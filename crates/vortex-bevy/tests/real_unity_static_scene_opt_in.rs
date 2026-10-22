use std::{
    collections::{BTreeMap, BTreeSet},
    env,
    error::Error,
    fs,
    path::PathBuf,
    thread,
    time::Duration,
};

use bevy::{
    asset::AssetServer,
    prelude::{App, Camera, DirectionalLight, MeshMaterial3d, StandardMaterial, WorldAssetRoot},
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
        unity_static_scene::UnityStaticSceneAdapter,
    },
    domain::{
        ArtifactHash, ArtifactKind, ArtifactRecord, ClassificationKind, CompatibilityCapability,
        CompatibilityRequirements, ConversionAggregate, ConversionContext, ManifestArtifact,
        ManifestArtifactKind, ManifestSource, OutputRoot, ProjectRoot, SceneManifest,
    },
};

const REPORT_VARIABLE: &str = "VORTEX_REAL_UNITY_STATIC_SCENE_REPORT";
const STAGING_VARIABLE: &str = "VORTEX_REAL_UNITY_STATIC_SCENE_STAGING_ROOT";
const OUTPUT_VARIABLE: &str = "VORTEX_REAL_UNITY_STATIC_SCENE_OUTPUT_ROOT";
const PROJECT_ROOT_LABEL: &str = "test123-unity-6000.5.10f1";

#[derive(Serialize)]
struct RealUnityValidationSummary {
    source_identity: String,
    conversion_id: String,
    publication: String,
    report_hash: String,
    glb_hash: String,
    texture_hash: String,
    verified_artifact_count: usize,
    mesh_vertex_count: usize,
    mesh_index_count: usize,
    camera_count: usize,
    directional_light_count: usize,
    standard_material_count: usize,
    unsupported_features: Vec<String>,
}

fn required_path(variable: &str) -> Result<PathBuf, Box<dyn Error>> {
    let value =
        env::var_os(variable).ok_or_else(|| format!("missing environment variable: {variable}"))?;
    Ok(PathBuf::from(value))
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
    Err("published real Unity static-scene GLB did not load in bounded headless ticks".into())
}

fn report_source(
    conversion: &vortex_core::adapters::secondary::unity_static_scene::UnityStaticSceneConversion,
    labels: BTreeMap<String, u32>,
) -> ManifestSource {
    let unsupported = conversion
        .scene
        .unsupported_features
        .iter()
        .map(|feature| format!("{}:{:?}", feature.feature, feature.classification.kind))
        .collect::<Vec<_>>()
        .join(",");
    ManifestSource {
        identity: conversion.source.clone(),
        asset_kind: "unity_editor_generated_static_scene".to_owned(),
        selected_values: BTreeMap::from([
            ("unity.editor_version".to_owned(), "6000.5.10f1".to_owned()),
            ("unity.urp_package_version".to_owned(), "17.5.0".to_owned()),
            ("unity.material.shader".to_owned(), "Universal Render Pipeline/Lit".to_owned()),
            ("unity.unsupported_components".to_owned(), unsupported),
        ]),
        glb_labels: labels.into_iter().map(|(label, value)| (label, value.to_string())).collect(),
        classification: conversion.material_classification.clone(),
        diagnostics: Vec::new(),
    }
}

#[test]
#[ignore = "requires an authorized real Unity static-scene report and PNG supplied through explicit environment paths"]
#[expect(
    clippy::too_many_lines,
    reason = "This ignored evidence test intentionally keeps its report-to-publication-to-runtime assertion as one auditable real-Editor transaction."
)]
fn authorized_real_unity_static_scene_is_published_and_loaded() -> Result<(), Box<dyn Error>> {
    let report_path = required_path(REPORT_VARIABLE)?;
    let staging_root = required_path(STAGING_VARIABLE)?;
    let output_root = required_path(OUTPUT_VARIABLE)?;
    if output_root.exists() {
        return Err(format!(
            "refusing to reuse existing validation output root: {}",
            output_root.display()
        )
        .into());
    }
    let report_bytes = fs::read(&report_path)?;
    let project_root: ProjectRoot = PROJECT_ROOT_LABEL.parse()?;
    let conversion =
        UnityStaticSceneAdapter.load_and_convert(project_root, &staging_root, &report_bytes)?;
    assert_eq!(conversion.material_classification.kind, ClassificationKind::Converted);
    assert_eq!(conversion.scene.cameras.len(), 1);
    assert_eq!(conversion.scene.lights.len(), 1);
    assert_eq!(conversion.scene.unsupported_features.len(), 1);
    let unsupported_feature = conversion
        .scene
        .unsupported_features
        .first()
        .ok_or("missing explicit unsupported component")?;
    assert_eq!(unsupported_feature.classification.kind, ClassificationKind::RequiresAdapter);

    let package = GlbWriter.package(&conversion.scene)?;
    let replay_package = GlbWriter.package(&conversion.scene)?;
    assert_eq!(package.bytes, replay_package.bytes);
    let glb_payload = ArtifactPayload::new("scene.glb", package.bytes.clone())?;
    let texture_payload = ArtifactPayload::new(
        conversion.base_color_texture.texture.image.clone(),
        conversion.base_color_texture.bytes.clone(),
    )?;
    let report_payload =
        ArtifactPayload::new("source/vortex-unity-static-scene-report.json", report_bytes)?;
    let profile_hash = ArtifactHash::from_bytes(b"real-unity-static-scene-validation-v1");
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
        vec![report_source(&conversion, package.labels.clone())],
        CompatibilityRequirements {
            minimum_bevy_version: "0.20".to_owned(),
            required_capabilities: BTreeSet::from([
                CompatibilityCapability::BevyGltfPbr,
                CompatibilityCapability::BevyPunctualLights,
                CompatibilityCapability::UrpLitSubset,
            ]),
        },
    )?;
    let manifest_ron = manifest.to_ron()?;
    let mut aggregate = ConversionAggregate::begin(
        conversion.source.clone(),
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
    aggregate.classify(conversion.material_classification.clone())?;
    let aggregate = aggregate.finalize()?;
    let conversion_id = aggregate.id().clone();
    let store =
        AtomicArtifactStore::new(&output_root, "real-unity-static-scene".parse::<OutputRoot>()?)?;
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
    let mut lights = world.query::<&DirectionalLight>();
    let directional_light_count = lights.iter(world).count();
    assert!(directional_light_count >= 1);
    let mut cameras = world.query::<&Camera>();
    assert!(cameras.iter(world).count() >= 1);

    let vertex_stream =
        conversion.scene.vertex_streams.first().ok_or("missing mesh vertex stream")?;
    let summary = RealUnityValidationSummary {
        source_identity: conversion.source.to_string(),
        conversion_id: conversion_id.to_string(),
        publication: format!("{:?}", PublicationOutcome::Published),
        report_hash: ArtifactHash::from_bytes(&fs::read(&report_path)?).to_string(),
        glb_hash: package.checksum.to_string(),
        texture_hash: conversion.base_color_texture.checksum.to_string(),
        verified_artifact_count: verification.verified_hashes.len(),
        mesh_vertex_count: vertex_stream.positions.len(),
        mesh_index_count: vertex_stream.indices.len(),
        camera_count: conversion.scene.cameras.len(),
        directional_light_count,
        standard_material_count,
        unsupported_features: conversion
            .scene
            .unsupported_features
            .iter()
            .map(|feature| format!("{}:{:?}", feature.feature, feature.classification.kind))
            .collect(),
    };
    fs::write(
        output_root.join("real-unity-static-scene-validation-summary.json"),
        format!("{}\n", serde_json::to_string_pretty(&summary)?),
    )?;
    Ok(())
}
