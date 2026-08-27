use std::{
    collections::{BTreeMap, BTreeSet},
    env,
    error::Error,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
    thread,
    time::Duration,
};

use bevy::{
    asset::AssetServer,
    prelude::{App, MeshMaterial3d, StandardMaterial, WorldAssetRoot},
};
use serde::Serialize;
use vortex_bevy::runtime::{
    VortexSceneRoot, build_runtime_app, load_published_conversion_and_spawn,
};
use vortex_core::{
    adapters::secondary::{
        artifact_store::{
            ArtifactPayload, AtomicArtifactStore, PublicationOutcome, PublicationRequest,
        },
        glb::GlbWriter,
        unreal_bridge::{UnrealBridge, UnrealBridgeRequest},
        unreal_static_scene::UnrealStaticSceneAdapter,
    },
    domain::{
        ArtifactHash, ArtifactKind, ArtifactRecord, ClassificationKind, CompatibilityCapability,
        CompatibilityRequirements, ConversionAggregate, ConversionClassification,
        ConversionContext, ManifestArtifact, ManifestArtifactKind, ManifestSource, OutputRoot,
        Remediation, SceneManifest, UnrealObjectPath,
    },
};

const REPORT_VARIABLE: &str = "VORTEX_REAL_UNREAL_REPORT";
const MESH_VARIABLE: &str = "VORTEX_REAL_UNREAL_MESH";
const TEXTURE_VARIABLE: &str = "VORTEX_REAL_UNREAL_TEXTURE";
const OUTPUT_VARIABLE: &str = "VORTEX_REAL_UNREAL_OUTPUT_ROOT";
static NEXT_DIRECTORY: AtomicUsize = AtomicUsize::new(0);

struct TestDirectory {
    path: PathBuf,
}

impl TestDirectory {
    fn new(label: &str) -> Result<Self, std::io::Error> {
        let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "vortex-real-unreal-static-scene-{label}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir_all(&path)?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        drop(fs::remove_dir_all(&self.path));
    }
}

#[derive(Serialize)]
struct RealUnrealValidationSummary {
    mesh_source_identity: String,
    texture_source_identity: String,
    conversion_id: String,
    publication: String,
    report_hash: String,
    input_fbx_hash: String,
    input_tga_hash: String,
    glb_hash: String,
    texture_png_hash: String,
    verified_artifact_count: usize,
    mesh_vertex_count: usize,
    mesh_index_count: usize,
    standard_material_count: usize,
    unsupported_features: Vec<String>,
}

fn required_path(variable: &str) -> Result<PathBuf, Box<dyn Error>> {
    let value =
        env::var_os(variable).ok_or_else(|| format!("missing environment variable: {variable}"))?;
    let path = PathBuf::from(value).canonicalize()?;
    if !path.is_file() {
        return Err(format!("{variable} must name a regular file: {}", path.display()).into());
    }
    Ok(path)
}

fn object_path(value: &str) -> Result<UnrealObjectPath, Box<dyn Error>> {
    Ok(value.parse()?)
}

fn tick_until_glb_loaded(
    app: &mut App,
    entity: bevy::prelude::Entity,
) -> Result<(), Box<dyn Error>> {
    for _ in 0..80 {
        app.update();
        let world = app.world();
        let root =
            world.entity(entity).get::<WorldAssetRoot>().ok_or("missing world asset root")?;
        let server = world.get_resource::<AssetServer>().ok_or("missing asset server")?;
        if server.is_loaded(root.0.id()) {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(5));
    }
    Err("published real Unreal static-scene GLB did not load in bounded headless ticks".into())
}

fn report_source(
    identity: vortex_core::domain::SourceIdentity,
    asset_kind: &str,
    classification: ConversionClassification,
    labels: BTreeMap<String, u32>,
) -> ManifestSource {
    ManifestSource {
        identity,
        asset_kind: asset_kind.to_owned(),
        selected_values: BTreeMap::from([
            ("unreal.editor_version".to_owned(), "5.8.2".to_owned()),
            ("unreal.bridge_protocol".to_owned(), "vortex.editor-bridge/v1".to_owned()),
            ("unreal.static_scene.material".to_owned(), "requires_adapter".to_owned()),
            ("unreal.static_scene.world".to_owned(), "requires_adapter".to_owned()),
        ]),
        glb_labels: labels.into_iter().map(|(label, value)| (label, value.to_string())).collect(),
        classification,
        diagnostics: Vec::new(),
    }
}

#[test]
#[ignore = "requires explicit paths to authorized received Unreal report, FBX, and TGA artifacts"]
#[expect(
    clippy::too_many_lines,
    reason = "This ignored evidence test intentionally retains one auditable real-Editor transaction from typed report to headless runtime."
)]
fn authorized_real_unreal_static_scene_is_published_and_loaded() -> Result<(), Box<dyn Error>> {
    let report_path = required_path(REPORT_VARIABLE)?;
    let mesh_path = required_path(MESH_VARIABLE)?;
    let texture_path = required_path(TEXTURE_VARIABLE)?;
    let output_root = PathBuf::from(
        env::var_os(OUTPUT_VARIABLE)
            .ok_or_else(|| format!("missing environment variable: {OUTPUT_VARIABLE}"))?,
    );
    if output_root.exists() {
        return Err(format!(
            "refusing to reuse existing validation output root: {}",
            output_root.display()
        )
        .into());
    }
    let staging_root =
        report_path.parent().ok_or("received report has no staging parent")?.canonicalize()?;
    let temporary = TestDirectory::new("report")?;
    let project_file = temporary.path().join("Test123.uproject");
    let editor = temporary.path().join("UnrealEditor");
    fs::write(&project_file, "{}")?;
    fs::write(&editor, "real-artifact-validation-only")?;
    let request = UnrealBridgeRequest::new(
        temporary.path(),
        &project_file,
        &editor,
        &staging_root,
        [
            object_path("/Game/VortexBridgeTemporary/LVL_VortexStaticScene.LVL_VortexStaticScene")?,
            object_path("/Game/VortexBridgeTemporary/MI_VortexLit.MI_VortexLit")?,
            object_path("/Game/VortexBridgeTemporary/SM_VortexCube.SM_VortexCube")?,
            object_path("/Game/VortexBridgeTemporary/T_VortexBaseColor.T_VortexBaseColor")?,
        ],
    )?;
    let repository_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize()?;
    let bridge = UnrealBridge::from_repository_root(repository_root)?;
    let report_bytes = fs::read(&report_path)?;
    let report = bridge.parse_report_text(&request, &String::from_utf8(report_bytes.clone())?)?;
    let conversion = UnrealStaticSceneAdapter.convert(
        &report,
        &fs::read(&mesh_path)?,
        &fs::read(&texture_path)?,
    )?;
    assert_eq!(conversion.scene.unsupported_features.len(), 2);
    assert!(
        conversion
            .scene
            .unsupported_features
            .iter()
            .all(|feature| feature.classification.kind == ClassificationKind::RequiresAdapter)
    );
    let vertex_stream =
        conversion.scene.vertex_streams.first().ok_or("missing canonical mesh vertex stream")?;
    assert!(vertex_stream.positions.len() >= 3);
    assert!(vertex_stream.indices.len().is_multiple_of(3));

    let package = GlbWriter.package(&conversion.scene)?;
    assert_eq!(package.bytes, GlbWriter.package(&conversion.scene)?.bytes);
    let glb_payload = ArtifactPayload::new("scene.glb", package.bytes.clone())?;
    let texture_payload = ArtifactPayload::new(
        conversion.base_color_texture.texture.image.clone(),
        conversion.base_color_texture.bytes.clone(),
    )?;
    let report_payload =
        ArtifactPayload::new("source/vortex-unreal-bridge-report.json", report_bytes.clone())?;
    let profile_hash =
        ArtifactHash::from_bytes(b"real-unreal-static-scene-canonical-validation-v1");
    let mesh_classification = ConversionClassification::new(
        ClassificationKind::Converted,
        "Authorized Unreal StaticMeshExporterFBX output converted through the bounded static-scene subset.",
        Remediation::new("Use a dedicated Unreal adapter for material graphs or world semantics.")?,
    )?;
    let manifest = SceneManifest::new(
        "0.1.0",
        profile_hash.clone(),
        vec![
            ManifestArtifact {
                kind: ManifestArtifactKind::Glb,
                path: glb_payload.relative_path.clone(),
                hash: glb_payload.hash(),
            },
            ManifestArtifact {
                kind: ManifestArtifactKind::Texture,
                path: texture_payload.relative_path.clone(),
                hash: texture_payload.hash(),
            },
            ManifestArtifact {
                kind: ManifestArtifactKind::Report,
                path: report_payload.relative_path.clone(),
                hash: report_payload.hash(),
            },
        ],
        vec![report_source(
            conversion.mesh_source.clone(),
            "StaticMesh",
            mesh_classification.clone(),
            package.labels.clone(),
        )],
        CompatibilityRequirements {
            minimum_bevy_version: "0.19".to_owned(),
            required_capabilities: BTreeSet::from([CompatibilityCapability::BevyGltfPbr]),
        },
    )?;
    let manifest_ron = manifest.to_ron()?;
    let mut aggregate = ConversionAggregate::begin(
        conversion.mesh_source.clone(),
        ConversionContext::new("default".parse()?, profile_hash, "0.1.0")?,
    );
    aggregate.record_artifact(ArtifactRecord::new(
        ArtifactKind::Glb,
        glb_payload.hash(),
        "scene.glb",
    )?)?;
    aggregate.record_artifact(ArtifactRecord::new(
        ArtifactKind::Texture,
        texture_payload.hash(),
        texture_payload.relative_path.clone(),
    )?)?;
    aggregate.record_artifact(ArtifactRecord::new(
        ArtifactKind::Report,
        report_payload.hash(),
        report_payload.relative_path.clone(),
    )?)?;
    aggregate.record_artifact(ArtifactRecord::new(
        ArtifactKind::Manifest,
        ArtifactHash::from_bytes(manifest_ron.as_bytes()),
        "scene.manifest.ron",
    )?)?;
    aggregate.classify(mesh_classification)?;
    let aggregate = aggregate.finalize()?;
    let conversion_id = aggregate.id().clone();
    let store =
        AtomicArtifactStore::new(&output_root, "real-unreal-static-scene".parse::<OutputRoot>()?)?;
    let request = PublicationRequest {
        conversion: aggregate,
        manifest,
        payloads: vec![glb_payload, texture_payload, report_payload],
    };
    assert_eq!(store.publish(&request)?, PublicationOutcome::Published);
    assert_eq!(store.publish(&request)?, PublicationOutcome::Reused);
    let verification = store.verify_published(&conversion_id)?;
    assert_eq!(verification.verified_hashes.len(), 3);

    let mut app =
        build_runtime_app(output_root.join(conversion_id.as_str()).to_string_lossy().into_owned());
    let entity = load_published_conversion_and_spawn(&mut app, &store, &conversion_id)?;
    tick_until_glb_loaded(&mut app, entity)?;
    let world = app.world_mut();
    assert!(world.entity(entity).contains::<VortexSceneRoot>());
    let mut materials = world.query::<&MeshMaterial3d<StandardMaterial>>();
    let standard_material_count = materials.iter(world).count();
    assert!(standard_material_count >= 1);

    let summary = RealUnrealValidationSummary {
        mesh_source_identity: conversion.mesh_source.to_string(),
        texture_source_identity: conversion.texture_source.to_string(),
        conversion_id: conversion_id.to_string(),
        publication: format!("{:?}", PublicationOutcome::Published),
        report_hash: ArtifactHash::from_bytes(&report_bytes).to_string(),
        input_fbx_hash: ArtifactHash::from_bytes(&fs::read(mesh_path)?).to_string(),
        input_tga_hash: ArtifactHash::from_bytes(&fs::read(texture_path)?).to_string(),
        glb_hash: package.checksum.to_string(),
        texture_png_hash: conversion.base_color_texture.checksum.to_string(),
        verified_artifact_count: verification.verified_hashes.len(),
        mesh_vertex_count: vertex_stream.positions.len(),
        mesh_index_count: vertex_stream.indices.len(),
        standard_material_count,
        unsupported_features: conversion
            .scene
            .unsupported_features
            .iter()
            .map(|feature| format!("{}:{:?}", feature.feature, feature.classification.kind))
            .collect(),
    };
    fs::create_dir_all(&output_root)?;
    fs::write(
        output_root.join("real-unreal-static-scene-canonical-validation-summary.json"),
        format!("{}\n", serde_json::to_string_pretty(&summary)?),
    )?;
    Ok(())
}
