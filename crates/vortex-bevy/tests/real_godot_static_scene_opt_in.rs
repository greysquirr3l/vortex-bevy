use std::{
    collections::{BTreeMap, BTreeSet},
    env,
    error::Error,
    fs,
    path::{Path, PathBuf},
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
        godot_static_scene::{GodotStaticSceneAdapter, GodotStaticSceneReport},
    },
    domain::{
        ArtifactHash, ArtifactKind, ArtifactRecord, ClassificationKind, CompatibilityCapability,
        CompatibilityRequirements, ConversionAggregate, ConversionClassification,
        ConversionContext, ConversionId, ManifestArtifact, ManifestArtifactKind, ManifestSource,
        OutputRoot, ProjectRoot, Remediation, SceneManifest, SourceIdentity,
    },
};

const REPORT_VARIABLE: &str = "VORTEX_REAL_GODOT_STATIC_SCENE_REPORT";
const STAGING_VARIABLE: &str = "VORTEX_REAL_GODOT_STATIC_SCENE_STAGING_ROOT";
const OUTPUT_VARIABLE: &str = "VORTEX_REAL_GODOT_STATIC_SCENE_OUTPUT_ROOT";
const PROJECT_ROOT_LABEL: &str = "test123-godot-4.7.2";
const PROFILE_BYTES: &[u8] = b"real-godot-static-scene-validation-v1";

#[derive(Serialize)]
struct RealGodotValidationSummary {
    source_identity: String,
    conversion_id: String,
    publication: String,
    report_hash: String,
    glb_hash: String,
    texture_hash: String,
    verified_artifacts: usize,
    cameras: usize,
    directional_lights: usize,
    standard_materials: usize,
    particle_boundary: String,
}

struct ValidationInputs {
    report_path: PathBuf,
    staging_root: PathBuf,
    output_root: PathBuf,
}

struct ValidatedArtifacts {
    report: GodotStaticSceneReport,
    report_bytes: Vec<u8>,
    glb_bytes: Vec<u8>,
    texture_bytes: Vec<u8>,
}

struct ValidationPayloads {
    glb: ArtifactPayload,
    texture: ArtifactPayload,
    report: ArtifactPayload,
}

struct PublishedValidation {
    store: AtomicArtifactStore,
    conversion_id: ConversionId,
    verified_artifacts: usize,
}

fn required_path(variable: &str) -> Result<PathBuf, Box<dyn Error>> {
    let value =
        env::var_os(variable).ok_or_else(|| format!("missing environment variable: {variable}"))?;
    Ok(PathBuf::from(value))
}

fn inputs_from_environment() -> Result<ValidationInputs, Box<dyn Error>> {
    let inputs = ValidationInputs {
        report_path: required_path(REPORT_VARIABLE)?,
        staging_root: required_path(STAGING_VARIABLE)?,
        output_root: required_path(OUTPUT_VARIABLE)?,
    };
    if inputs.output_root.exists() {
        return Err(format!(
            "refusing to reuse existing validation output root: {}",
            inputs.output_root.display()
        )
        .into());
    }
    Ok(inputs)
}

fn load_artifacts(inputs: &ValidationInputs) -> Result<ValidatedArtifacts, Box<dyn Error>> {
    let report_bytes = fs::read(&inputs.report_path)?;
    let project_root: ProjectRoot = PROJECT_ROOT_LABEL.parse()?;
    let report = GodotStaticSceneAdapter.parse_report_file(
        &project_root,
        &inputs.staging_root,
        &inputs.report_path,
    )?;
    if report.particle_classification.kind != ClassificationKind::RequiresAdapter {
        return Err("Godot particle record must remain an explicit adapter boundary".into());
    }
    let glb_bytes = fs::read(inputs.staging_root.join(&report.glb_output))?;
    let texture_bytes = fs::read(inputs.staging_root.join(&report.texture_output))?;
    Ok(ValidatedArtifacts { report, report_bytes, glb_bytes, texture_bytes })
}

fn converted_classification(kind: &str) -> Result<ConversionClassification, Box<dyn Error>> {
    Ok(ConversionClassification::new(
        ClassificationKind::Converted,
        format!("Godot static-scene bridge converted the controlled {kind}."),
        Remediation::new("Validate the staged output before packaging it into a Vortex artifact.")?,
    )?)
}

fn source(
    identity: SourceIdentity,
    asset_kind: &str,
    selected_values: BTreeMap<String, String>,
    classification: ConversionClassification,
) -> ManifestSource {
    ManifestSource {
        identity,
        asset_kind: asset_kind.to_owned(),
        selected_values,
        glb_labels: BTreeMap::new(),
        classification,
        diagnostics: Vec::new(),
    }
}

fn manifest_for(
    artifacts: &ValidatedArtifacts,
    payloads: &ValidationPayloads,
    profile_hash: ArtifactHash,
) -> Result<(SceneManifest, ConversionClassification), Box<dyn Error>> {
    let scene_classification = converted_classification("Node3D scene")?;
    let texture_classification = converted_classification("ImageTexture base-color map")?;
    let manifest = SceneManifest::new(
        "0.1.0",
        profile_hash,
        vec![
            manifest_artifact(ManifestArtifactKind::Glb, &payloads.glb),
            manifest_artifact(ManifestArtifactKind::Texture, &payloads.texture),
            manifest_artifact(ManifestArtifactKind::Report, &payloads.report),
        ],
        vec![
            source(
                artifacts.report.scene_source.clone(),
                "godot_editor_generated_static_scene",
                BTreeMap::from([("godot.editor_version".to_owned(), "4.7.2".to_owned())]),
                scene_classification.clone(),
            ),
            source(
                artifacts.report.texture_source.clone(),
                "godot_editor_generated_base_color",
                BTreeMap::from([("godot.texture_format".to_owned(), "png".to_owned())]),
                texture_classification,
            ),
            source(
                artifacts.report.particle_source.clone(),
                "GPUParticles3D",
                BTreeMap::from([(
                    "godot.unsupported_component".to_owned(),
                    "GPUParticles3D".to_owned(),
                )]),
                artifacts.report.particle_classification.clone(),
            ),
        ],
        CompatibilityRequirements {
            minimum_bevy_version: "0.19".to_owned(),
            required_capabilities: BTreeSet::from([
                CompatibilityCapability::BevyGltfPbr,
                CompatibilityCapability::BevyPunctualLights,
            ]),
        },
    )?;
    Ok((manifest, scene_classification))
}

fn manifest_artifact(kind: ManifestArtifactKind, payload: &ArtifactPayload) -> ManifestArtifact {
    ManifestArtifact { kind, path: payload.relative_path.clone(), hash: payload.hash() }
}

fn payloads_for(artifacts: &ValidatedArtifacts) -> Result<ValidationPayloads, Box<dyn Error>> {
    Ok(ValidationPayloads {
        glb: ArtifactPayload::new("scene.glb", artifacts.glb_bytes.clone())?,
        texture: ArtifactPayload::new(
            artifacts.report.texture_output.to_string_lossy().into_owned(),
            artifacts.texture_bytes.clone(),
        )?,
        report: ArtifactPayload::new(
            "source/vortex-godot-static-scene-report.json",
            artifacts.report_bytes.clone(),
        )?,
    })
}

fn aggregate_for(
    artifacts: &ValidatedArtifacts,
    payloads: &ValidationPayloads,
    manifest_ron: &str,
    profile_hash: ArtifactHash,
    classification: ConversionClassification,
) -> Result<ConversionAggregate, Box<dyn Error>> {
    let mut aggregate = ConversionAggregate::begin(
        artifacts.report.scene_source.clone(),
        ConversionContext::new("default".parse()?, profile_hash, "0.1.0")?,
    );
    record(&mut aggregate, ArtifactKind::Glb, payloads.glb.hash(), "scene.glb")?;
    record(
        &mut aggregate,
        ArtifactKind::Texture,
        payloads.texture.hash(),
        payloads.texture.relative_path.clone(),
    )?;
    record(
        &mut aggregate,
        ArtifactKind::Report,
        payloads.report.hash(),
        payloads.report.relative_path.clone(),
    )?;
    record(
        &mut aggregate,
        ArtifactKind::Manifest,
        ArtifactHash::from_bytes(manifest_ron.as_bytes()),
        "scene.manifest.ron",
    )?;
    aggregate.classify(classification)?;
    Ok(aggregate.finalize()?)
}

fn publish_validation(
    inputs: &ValidationInputs,
    artifacts: &ValidatedArtifacts,
) -> Result<PublishedValidation, Box<dyn Error>> {
    let profile_hash = ArtifactHash::from_bytes(PROFILE_BYTES);
    let payloads = payloads_for(artifacts)?;
    let (manifest, scene_classification) =
        manifest_for(artifacts, &payloads, profile_hash.clone())?;
    let aggregate = aggregate_for(
        artifacts,
        &payloads,
        &manifest.to_ron()?,
        profile_hash,
        scene_classification,
    )?;
    let conversion_id = aggregate.id().clone();
    let store = AtomicArtifactStore::new(
        &inputs.output_root,
        "real-godot-static-scene".parse::<OutputRoot>()?,
    )?;
    let request = PublicationRequest {
        conversion: aggregate,
        manifest,
        payloads: vec![payloads.glb, payloads.texture, payloads.report],
    };
    if store.publish(&request)? != PublicationOutcome::Published
        || store.publish(&request)? != PublicationOutcome::Reused
    {
        return Err(
            "Godot static-scene publication did not produce deterministic publish/reuse outcomes"
                .into(),
        );
    }
    let verification = store.verify_published(&conversion_id)?;
    if verification.verified_hashes.len() != 3 {
        return Err("Godot static-scene publication must verify exactly three artifacts".into());
    }
    Ok(PublishedValidation {
        store,
        conversion_id,
        verified_artifacts: verification.verified_hashes.len(),
    })
}

fn assert_runtime(
    output_root: &Path,
    published: &PublishedValidation,
) -> Result<(usize, usize, usize), Box<dyn Error>> {
    let asset_root = output_root.join(published.conversion_id.as_str());
    let mut app = build_runtime_app(asset_root.to_string_lossy().into_owned());
    let entity =
        load_published_conversion_and_spawn(&mut app, &published.store, &published.conversion_id)?;
    tick_until_glb_loaded(&mut app, entity)?;
    let world = app.world_mut();
    if !world.entity(entity).contains::<VortexSceneRoot>() {
        return Err("published Godot scene root is missing Vortex provenance".into());
    }
    let materials = world.query::<&MeshMaterial3d<StandardMaterial>>().iter(world).count();
    let lights = world.query::<&DirectionalLight>().iter(world).count();
    let cameras = world.query::<&Camera>().iter(world).count();
    if materials == 0 || lights == 0 || cameras == 0 {
        return Err("published Godot scene did not instantiate material, camera, and light".into());
    }
    Ok((materials, lights, cameras))
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
    let world = app.world();
    let root = world.entity(entity).get::<WorldAssetRoot>().ok_or("missing world asset root")?;
    let server = world.get_resource::<AssetServer>().ok_or("missing asset server")?;
    let state = server
        .get_load_state(root.0.id())
        .map_or_else(|| "unknown".to_owned(), |state| format!("{state:?}"));
    Err(format!(
        "published real Godot static-scene GLB did not load in bounded headless ticks: {state}"
    )
    .into())
}

fn write_summary(
    inputs: &ValidationInputs,
    artifacts: &ValidatedArtifacts,
    published: &PublishedValidation,
    materials: usize,
    lights: usize,
    cameras: usize,
) -> Result<(), Box<dyn Error>> {
    let summary = RealGodotValidationSummary {
        source_identity: artifacts.report.scene_source.to_string(),
        conversion_id: published.conversion_id.to_string(),
        publication: format!("{:?}", PublicationOutcome::Published),
        report_hash: ArtifactHash::from_bytes(&artifacts.report_bytes).to_string(),
        glb_hash: ArtifactHash::from_bytes(&artifacts.glb_bytes).to_string(),
        texture_hash: ArtifactHash::from_bytes(&artifacts.texture_bytes).to_string(),
        verified_artifacts: published.verified_artifacts,
        cameras,
        directional_lights: lights,
        standard_materials: materials,
        particle_boundary: format!(
            "GPUParticles3D:{:?}",
            artifacts.report.particle_classification.kind
        ),
    };
    fs::write(
        inputs.output_root.join("real-godot-static-scene-validation-summary.json"),
        format!("{}\n", serde_json::to_string_pretty(&summary)?),
    )?;
    Ok(())
}

#[test]
#[ignore = "requires an authorized real Godot static-scene report, GLB, and PNG supplied through explicit environment paths"]
fn authorized_real_godot_static_scene_is_published_and_loaded() -> Result<(), Box<dyn Error>> {
    let inputs = inputs_from_environment()?;
    let artifacts = load_artifacts(&inputs)?;
    let published = publish_validation(&inputs, &artifacts)?;
    let (materials, lights, cameras) = assert_runtime(&inputs.output_root, &published)?;
    write_summary(&inputs, &artifacts, &published, materials, lights, cameras)
}

fn record(
    aggregate: &mut ConversionAggregate,
    kind: ArtifactKind,
    hash: ArtifactHash,
    path: impl Into<String>,
) -> Result<(), Box<dyn Error>> {
    aggregate.record_artifact(ArtifactRecord::new(kind, hash, path.into())?)?;
    Ok(())
}
