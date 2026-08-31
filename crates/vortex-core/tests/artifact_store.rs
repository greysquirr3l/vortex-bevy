use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use vortex_core::{
    adapters::secondary::artifact_store::{
        ArtifactPayload, ArtifactStoreError, AtomicArtifactStore, PublicationOutcome,
        PublicationRequest,
    },
    application::{
        command::{clean_output::OutputCleaner, verify_artifacts::ArtifactVerifier},
        query::conversion_report::ConversionReportReader,
    },
    domain::{
        ArtifactHash, ArtifactKind, ArtifactRecord, ClassificationKind, CompatibilityRequirements,
        ConversionAggregate, ConversionClassification, ConversionContext, ConversionId,
        DiagnosticCode, ManifestArtifact, ManifestArtifactKind, ManifestSource, OutputRoot,
        ProjectRoot, Remediation, SceneManifest, SourceAssetPath, SourceIdentity, UnrealObjectPath,
    },
};

static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn root(name: &str) -> Result<PathBuf, Box<dyn Error>> {
    let sequence = TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!("vortex-artifact-store-{name}-{sequence}"));
    fs::create_dir_all(&root)?;
    Ok(root)
}

fn source() -> Result<SourceIdentity, Box<dyn Error>> {
    Ok(SourceIdentity::Unreal {
        project_root: "fixture".parse::<ProjectRoot>()?,
        asset_path: "Content/Crate.uasset".parse::<SourceAssetPath>()?,
        object_path: "/Game/Crate.Crate".parse::<UnrealObjectPath>()?,
    })
}

fn request() -> Result<PublicationRequest, Box<dyn Error>> {
    let source = source()?;
    let profile_hash = ArtifactHash::from_bytes(b"profile");
    let context = ConversionContext::new("default".parse()?, profile_hash.clone(), "test")?;
    let classification = ConversionClassification::new(
        ClassificationKind::Converted,
        "self-authored fixture converted",
        Remediation::new("inspect the deterministic fixture")?,
    )?;
    let glb = ArtifactPayload::new("scene.glb", b"glb-bytes".to_vec())?;
    let manifest = SceneManifest::new(
        "test",
        profile_hash,
        vec![ManifestArtifact {
            kind: ManifestArtifactKind::Glb,
            path: glb.relative_path.clone(),
            hash: glb.hash(),
        }],
        vec![ManifestSource {
            identity: source.clone(),
            asset_kind: "static_mesh".to_owned(),
            selected_values: BTreeMap::new(),
            glb_labels: BTreeMap::new(),
            classification: classification.clone(),
            diagnostics: Vec::new(),
        }],
        CompatibilityRequirements {
            minimum_bevy_version: "0.19".to_owned(),
            required_capabilities: BTreeSet::new(),
        },
    )?;
    let mut conversion = ConversionAggregate::begin(source, context);
    conversion.record_artifact(ArtifactRecord::new(ArtifactKind::Glb, glb.hash(), "scene.glb")?)?;
    conversion.record_artifact(ArtifactRecord::new(
        ArtifactKind::Manifest,
        ArtifactHash::from_bytes(b"manifest-record"),
        "scene.manifest.ron",
    )?)?;
    conversion.classify(classification)?;
    let conversion = conversion.finalize()?;
    Ok(PublicationRequest { conversion, manifest, payloads: vec![glb] })
}

fn store(root: &PathBuf) -> Result<AtomicArtifactStore, Box<dyn Error>> {
    Ok(AtomicArtifactStore::new(root, "artifacts".parse::<OutputRoot>()?)?)
}

#[test]
fn publish_verify_report_replay_and_clean_are_idempotent() -> Result<(), Box<dyn Error>> {
    let root = root("success")?;
    let store = store(&root)?;
    let request = request()?;
    let id = request.conversion.id().clone();

    assert_eq!(store.publish(&request)?, PublicationOutcome::Published);
    assert_eq!(store.publish(&request)?, PublicationOutcome::Reused);
    let verification = store.verify(&id)?;
    let expected_hash = request.payloads.first().ok_or("missing payload")?.hash();
    assert_eq!(verification.verified_hashes, vec![expected_hash]);
    assert_eq!(store.read(&id)?, Some(request.conversion));
    store.clean_conversion(&id)?;
    store.clean_conversion(&id)?;
    assert_eq!(store.read(&id)?, None);
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn failed_write_and_checksum_mismatch_never_publish_a_partial_final_directory()
-> Result<(), Box<dyn Error>> {
    let root = root("failure")?;
    let request = request()?;
    let id = request.conversion.id().clone();
    let failing = store(&root)?.with_failure_after_payloads(0);
    assert!(matches!(failing.publish(&request), Err(ArtifactStoreError::InjectedWriteFailure)));
    assert!(!root.join(id.as_str()).exists());

    let mut mismatched = request;
    let payload = mismatched.payloads.first_mut().ok_or("missing payload")?;
    payload.bytes = b"tampered".to_vec();
    assert!(matches!(
        store(&root)?.publish(&mismatched),
        Err(ArtifactStoreError::ChecksumMismatch { .. })
    ));
    assert!(!root.join(id.as_str()).exists());
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn lock_contention_checksum_failure_and_external_cleanup_refusal_are_fail_closed()
-> Result<(), Box<dyn Error>> {
    let root = root("contention")?;
    let request = request()?;
    let id = request.conversion.id().clone();
    let locks = root.join(".locks");
    fs::create_dir_all(&locks)?;
    fs::write(locks.join(format!("{}.lock", id.as_str())), b"held")?;
    assert!(matches!(
        store(&root)?.publish(&request),
        Err(ArtifactStoreError::LockContended { .. })
    ));
    fs::remove_file(locks.join(format!("{}.lock", id.as_str())))?;

    let store = store(&root)?;
    assert_eq!(store.publish(&request)?, PublicationOutcome::Published);
    fs::write(root.join(id.as_str()).join("scene.glb"), b"corrupt")?;
    assert!(matches!(store.verify(&id), Err(ArtifactStoreError::ChecksumMismatch { .. })));
    let external = root.with_extension("external");
    fs::create_dir_all(&external)?;
    fs::write(external.join("keep.txt"), b"outside")?;
    store.clean_staging(&id)?;
    assert!(external.join("keep.txt").exists());
    fs::remove_dir_all(root)?;
    fs::remove_dir_all(external)?;
    Ok(())
}

#[test]
fn malformed_report_is_not_returned_as_a_valid_conversion() -> Result<(), Box<dyn Error>> {
    let root = root("report")?;
    let request = request()?;
    let id = request.conversion.id().clone();
    let store = store(&root)?;
    assert_eq!(store.publish(&request)?, PublicationOutcome::Published);
    fs::write(root.join(id.as_str()).join("conversion-report.json"), b"not-json")?;
    assert!(matches!(store.read(&id), Err(ArtifactStoreError::ReportDeserialization { .. })));
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn diagnostic_code_fixture_remains_typed() -> Result<(), Box<dyn Error>> {
    let code = DiagnosticCode::parse("artifact.verified")?;
    assert_eq!(code.as_str(), "artifact.verified");
    Ok(())
}

#[test]
fn list_published_conversions_enumerates_only_directories_with_a_manifest()
-> Result<(), Box<dyn Error>> {
    let root = root("list")?;
    let store = store(&root)?;

    // Empty root returns an empty list.
    assert!(store.list_published_conversions()?.is_empty());

    // Publish one conversion and verify it appears.
    let request = request()?;
    let id = request.conversion.id().clone();
    assert_eq!(store.publish(&request)?, PublicationOutcome::Published);
    let listed = store.list_published_conversions()?;
    assert_eq!(listed, vec![id.clone()]);

    // Foreign directories (no manifest sidecar) are skipped, not errored.
    let foreign = "not-a-real-conversion";
    fs::create_dir_all(root.join(foreign))?;
    let after_foreign = store.list_published_conversions()?;
    assert_eq!(after_foreign, vec![id.clone()]);

    // A non-64-char directory name fails to parse and is skipped.
    let short = "short";
    fs::create_dir_all(root.join(short))?;
    let after_short = store.list_published_conversions()?;
    assert_eq!(after_short, vec![id.clone()]);

    // ConversionId::parse round-trips through a hex string and rejects
    // non-hex or wrong-length input.
    assert_eq!(ConversionId::parse(id.as_str())?.as_str(), id.as_str());
    assert!(ConversionId::parse("not-hex").is_err());
    assert!(ConversionId::parse(&"f".repeat(63)).is_err());
    assert!(ConversionId::parse(&"g".repeat(64)).is_err());

    fs::remove_dir_all(root)?;
    Ok(())
}
