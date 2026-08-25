use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    str::FromStr,
};

use vortex_core::domain::{
    ArtifactHash, ClassificationKind, CompatibilityCapability, CompatibilityRequirements,
    ConversionClassification, ManifestArtifact, ManifestArtifactKind, ManifestError,
    ManifestSource, ProjectRoot, Remediation, SceneManifest, SourceAssetPath, SourceIdentity,
    UnrealObjectPath,
};

fn source(suffix: &str) -> Result<ManifestSource, Box<dyn Error>> {
    Ok(ManifestSource {
        identity: SourceIdentity::Unreal {
            project_root: ProjectRoot::from_str("FixtureProject")?,
            asset_path: SourceAssetPath::from_str(&format!("Content/Props/{suffix}.uasset"))?,
            object_path: UnrealObjectPath::from_str(&format!("/Game/Props/{suffix}.{suffix}"))?,
        },
        asset_kind: "static_mesh".to_owned(),
        selected_values: BTreeMap::new(),
        glb_labels: BTreeMap::from([("node".to_owned(), suffix.to_owned())]),
        classification: ConversionClassification::new(
            ClassificationKind::Converted,
            "fixture source is representable",
            Remediation::new("Use the supported fixture profile.")?,
        )?,
        diagnostics: Vec::new(),
    })
}

fn compatibility() -> CompatibilityRequirements {
    CompatibilityRequirements {
        minimum_bevy_version: "0.19".to_owned(),
        required_capabilities: BTreeSet::from([CompatibilityCapability::BevyGltfPbr]),
    }
}

fn artifact(path: &str) -> ManifestArtifact {
    ManifestArtifact {
        kind: ManifestArtifactKind::Glb,
        path: path.to_owned(),
        hash: ArtifactHash::from_bytes(path.as_bytes()),
    }
}

#[test]
fn manifest_round_trip_is_canonical_and_stable() -> Result<(), Box<dyn Error>> {
    let first = SceneManifest::new(
        "0.1.0",
        ArtifactHash::from_bytes(b"profile"),
        vec![artifact("assets/scene.glb")],
        vec![source("B")?, source("A")?],
        compatibility(),
    )?;
    let second = SceneManifest::new(
        "0.1.0",
        ArtifactHash::from_bytes(b"profile"),
        vec![artifact("assets/scene.glb")],
        vec![source("A")?, source("B")?],
        compatibility(),
    )?;

    let first_ron = first.to_ron()?;
    let second_ron = second.to_ron()?;
    assert_eq!(first_ron, second_ron);
    let round_tripped = SceneManifest::from_ron(first_ron.as_bytes(), 4_096)?;
    assert_eq!(round_tripped, first);

    Ok(())
}

#[test]
fn manifest_rejects_unsafe_paths_and_duplicate_sources() -> Result<(), Box<dyn Error>> {
    let unsafe_path = SceneManifest::new(
        "0.1.0",
        ArtifactHash::from_bytes(b"profile"),
        vec![artifact("../escape.glb")],
        vec![source("Chair")?],
        compatibility(),
    );
    assert!(matches!(unsafe_path, Err(ManifestError::UnsafeArtifactPath { .. })));

    let duplicate = source("Chair")?;
    let duplicate_source = SceneManifest::new(
        "0.1.0",
        ArtifactHash::from_bytes(b"profile"),
        vec![artifact("assets/scene.glb")],
        vec![duplicate.clone(), duplicate],
        compatibility(),
    );
    assert!(matches!(duplicate_source, Err(ManifestError::DuplicateSource { .. })));

    Ok(())
}

#[test]
fn manifest_rejects_oversized_future_and_malformed_input() -> Result<(), Box<dyn Error>> {
    let oversized = SceneManifest::from_ron(b"abc", 2);
    assert!(matches!(oversized, Err(ManifestError::TooLarge { .. })));

    let manifest = SceneManifest::new(
        "0.1.0",
        ArtifactHash::from_bytes(b"profile"),
        vec![artifact("assets/scene.glb")],
        vec![source("Chair")?],
        compatibility(),
    )?;
    let future = manifest.to_ron()?.replace("schema_version: 1", "schema_version: 2");
    let future_version = SceneManifest::from_ron(future.as_bytes(), 4_096);
    assert!(matches!(future_version, Err(ManifestError::UnsupportedSchema { actual: 2, .. })));

    let malformed = SceneManifest::from_ron(b"(not valid", 4_096);
    assert!(matches!(malformed, Err(ManifestError::InvalidRon { .. })));

    Ok(())
}
