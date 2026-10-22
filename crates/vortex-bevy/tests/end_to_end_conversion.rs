use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::Duration,
};

use bevy::{asset::AssetServer, prelude::WorldAssetRoot};
use vortex_bevy::runtime::{
    VortexRuntimeDiagnostic, VortexSceneRoot, build_runtime_app,
    load_published_conversion_and_spawn, load_published_conversion_or_diagnostic,
};
use vortex_core::{
    adapters::secondary::{
        artifact_store::{
            ArtifactPayload, AtomicArtifactStore, PublicationOutcome, PublicationRequest,
        },
        glb::GlbWriter,
        usd_stage::{UsdStageInspector, UsdStageRequest},
    },
    application::query::conversion_report::ConversionReportReader,
    domain::{
        ArtifactHash, ArtifactKind, ArtifactRecord, CanonicalScene, ClassificationKind,
        CompatibilityCapability, CompatibilityRequirements, ConversionAggregate,
        ConversionClassification, ConversionContext, CoordinateNormalization, ManifestArtifact,
        ManifestArtifactKind, ManifestSource, OutputRoot, ProjectRoot, SceneManifest,
        SourceAssetPath, UsdPrimPath,
        scene_ir::{
            Camera, CameraId, Light, LightId, Mesh, MeshId, Node, NodeId, Primitive, PrimitiveId,
            SourceLink, SourceLinkId, Transform, VertexStream, VertexStreamId,
        },
    },
};

static FIXTURE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn fixture_root(name: &str) -> Result<PathBuf, Box<dyn Error>> {
    let sequence = FIXTURE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!("vortex-t22-{name}-{sequence}"));
    if root.exists() {
        fs::remove_dir_all(&root)?;
    }
    fs::create_dir_all(&root)?;
    Ok(root)
}

fn write_stage(root: &Path, name: &str, content: &str) -> Result<(), Box<dyn Error>> {
    fs::write(root.join(name), content)?;
    Ok(())
}

fn scene_from_stage(
    source: vortex_core::domain::SourceIdentity,
) -> Result<CanonicalScene, Box<dyn Error>> {
    Ok(CanonicalScene::new(CanonicalScene {
        normalization: CoordinateNormalization {
            meters_per_unit: "0.01".to_owned(),
            source_up_axis: "Y".to_owned(),
            target_up_axis: "Y".to_owned(),
            winding: "preserve".to_owned(),
            uv_origin: "lower_left".to_owned(),
            color_space: "linear".to_owned(),
        },
        source_links: vec![SourceLink {
            id: SourceLinkId::new(0),
            source,
            facts: BTreeMap::from([("fixture".to_owned(), "t22_usda_triangle".to_owned())]),
        }],
        nodes: vec![
            Node {
                id: NodeId::new(0),
                parent: None,
                transform: Transform::default(),
                mesh: Some(MeshId::new(0)),
                camera: None,
                light: None,
                instance: None,
                source_link: SourceLinkId::new(0),
            },
            Node {
                id: NodeId::new(1),
                parent: None,
                transform: Transform { translation: [0.0, 0.0, 2.0], ..Transform::default() },
                mesh: None,
                camera: Some(CameraId::new(0)),
                light: None,
                instance: None,
                source_link: SourceLinkId::new(0),
            },
            Node {
                id: NodeId::new(2),
                parent: None,
                transform: Transform::default(),
                mesh: None,
                camera: None,
                light: Some(LightId::new(0)),
                instance: None,
                source_link: SourceLinkId::new(0),
            },
        ],
        meshes: vec![Mesh {
            id: MeshId::new(0),
            primitives: vec![PrimitiveId::new(0)],
            skin: None,
        }],
        primitives: vec![Primitive {
            id: PrimitiveId::new(0),
            vertex_stream: VertexStreamId::new(0),
            material: None,
            morph_targets: Vec::new(),
        }],
        vertex_streams: vec![VertexStream {
            id: VertexStreamId::new(0),
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            normals: None,
            tangents: None,
            tex_coords_0: None,
            colors_0: None,
            joints_0: None,
            weights_0: None,
            indices: vec![0, 1, 2],
        }],
        materials: Vec::new(),
        textures: Vec::new(),
        samplers: Vec::new(),
        skins: Vec::new(),
        joints: Vec::new(),
        animation_clips: Vec::new(),
        channels: Vec::new(),
        morph_targets: Vec::new(),
        cameras: vec![Camera {
            id: CameraId::new(0),
            projection: "perspective".to_owned(),
            primary_extent: 1.0,
        }],
        lights: vec![Light { id: LightId::new(0), kind: "point".to_owned(), intensity: 25.0 }],
        instances: Vec::new(),
        unsupported_features: Vec::new(),
    })?)
}

fn publish_stage_scene(
    output_root: &Path,
    stage_source: vortex_core::domain::SourceIdentity,
    classification: ConversionClassification,
) -> Result<(AtomicArtifactStore, vortex_core::domain::ConversionId, Vec<u8>), Box<dyn Error>> {
    let scene = scene_from_stage(stage_source.clone())?;
    let package = GlbWriter.package(&scene)?;
    let second = GlbWriter.package(&scene)?;
    assert_eq!(package.bytes, second.bytes);
    assert_eq!(package.checksum, second.checksum);
    let payload = ArtifactPayload::new("scene.glb", package.bytes.clone())?;
    let profile_hash = ArtifactHash::from_bytes(b"t22-synthetic-profile-v1");
    let manifest = SceneManifest::new(
        "0.1.0",
        profile_hash.clone(),
        vec![ManifestArtifact {
            kind: ManifestArtifactKind::Glb,
            path: payload.relative_path.clone(),
            hash: payload.hash(),
        }],
        vec![ManifestSource {
            identity: stage_source.clone(),
            asset_kind: "mesh".to_owned(),
            selected_values: BTreeMap::from([("stage".to_owned(), "fixture.usda".to_owned())]),
            glb_labels: BTreeMap::from([("node:0".to_owned(), "/Triangle".to_owned())]),
            classification: classification.clone(),
            diagnostics: Vec::new(),
        }],
        CompatibilityRequirements {
            minimum_bevy_version: "0.20".to_owned(),
            required_capabilities: BTreeSet::from([
                CompatibilityCapability::BevyGltfPbr,
                CompatibilityCapability::BevyPunctualLights,
            ]),
        },
    )?;
    let context = ConversionContext::new("default".parse()?, profile_hash, "0.1.0")?;
    let mut conversion = ConversionAggregate::begin(stage_source, context);
    conversion.record_artifact(ArtifactRecord::new(
        ArtifactKind::Glb,
        payload.hash(),
        "scene.glb",
    )?)?;
    conversion.record_artifact(ArtifactRecord::new(
        ArtifactKind::Manifest,
        ArtifactHash::from_bytes(b"t22-manifest-record"),
        "scene.manifest.ron",
    )?)?;
    conversion.classify(classification)?;
    let conversion = conversion.finalize()?;
    let id = conversion.id().clone();
    let store = AtomicArtifactStore::new(output_root, "t22-output".parse::<OutputRoot>()?)?;
    let request = PublicationRequest { conversion, manifest, payloads: vec![payload] };
    assert_eq!(store.publish(&request)?, PublicationOutcome::Published);
    assert_eq!(store.publish(&request)?, PublicationOutcome::Reused);
    Ok((store, id, package.bytes))
}

fn tick_until_loaded(
    app: &mut bevy::prelude::App,
    entity: bevy::prelude::Entity,
) -> Result<(), Box<dyn Error>> {
    for _ in 0..80 {
        app.update();
        let world = app.world();
        let root = world
            .entity(entity)
            .get::<WorldAssetRoot>()
            .ok_or("published scene root is missing WorldAssetRoot")?;
        let asset_server = world
            .get_resource::<AssetServer>()
            .ok_or("published scene runtime has no AssetServer")?;
        if asset_server.is_loaded_with_dependencies(root.0.id()) {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(5));
    }
    Err("published scene did not resolve within bounded headless ticks".into())
}

#[test]
fn synthetic_usda_to_canonical_glb_atomic_artifacts_and_headless_bevy_is_deterministic()
-> Result<(), Box<dyn Error>> {
    let root = fixture_root("success")?;
    write_stage(
        &root,
        "fixture.usda",
        "#usda 1.0\n(\n    upAxis = \"Y\"\n    metersPerUnit = 0.01\n)\ndef Xform \"Triangle\" {\n    def Mesh \"Geometry\" { }\n}\n",
    )?;
    let request = UsdStageRequest::new(
        &root,
        "fixture.usda".parse::<SourceAssetPath>()?,
        "purpose=render;payload=forbid;context=universal",
    )?;
    let inspector = UsdStageInspector;
    let first = inspector.resolve(&request)?;
    let second = inspector.resolve(&request)?;
    assert_eq!(first, second);
    assert_eq!(first.prims.len(), 2);
    assert_eq!(first.prims.last().map(|prim| prim.path.as_str()), Some("/Triangle/Geometry"));
    assert_eq!(first.classification.kind, ClassificationKind::RequiresAdapter);

    let (store, id, package) = publish_stage_scene(&root, first.source, first.classification)?;
    assert!(!package.is_empty());
    assert_eq!(store.verify_published(&id)?.verified_hashes.len(), 1);
    assert!(store.read(&id)?.is_some());

    let asset_root = root.join(id.as_str());
    let mut app = build_runtime_app(asset_root.to_string_lossy().into_owned());
    let entity = load_published_conversion_and_spawn(&mut app, &store, &id)?;
    assert!(app.world().entity(entity).contains::<VortexSceneRoot>());
    tick_until_loaded(&mut app, entity)?;
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn synthetic_e2e_failures_are_specific_and_never_spawn_a_scene() -> Result<(), Box<dyn Error>> {
    let root = fixture_root("failures")?;
    write_stage(&root, "broken.usda", "not a USDA stage")?;
    let malformed = UsdStageRequest::new(
        &root,
        "broken.usda".parse::<SourceAssetPath>()?,
        "purpose=render;payload=forbid;context=universal",
    )?;
    assert!(UsdStageInspector.resolve(&malformed).is_err());
    assert!(ArtifactPayload::new("../escape.glb", Vec::new()).is_err());

    write_stage(
        &root,
        "fixture.usda",
        "#usda 1.0\ndef Xform \"Triangle\" { def Mesh \"Geometry\" { } }\n",
    )?;
    let valid = UsdStageInspector.resolve(&UsdStageRequest::new(
        &root,
        "fixture.usda".parse::<SourceAssetPath>()?,
        "purpose=render;payload=forbid;context=universal",
    )?)?;
    let (store, id, _) = publish_stage_scene(&root, valid.source, valid.classification)?;
    let asset_root = root.join(id.as_str());
    fs::write(asset_root.join("scene.glb"), b"tampered")?;
    let mut app = build_runtime_app(asset_root.to_string_lossy().into_owned());
    let diagnostic = load_published_conversion_or_diagnostic(&mut app, &store, &id);
    assert!(app.world().entity(diagnostic).contains::<VortexRuntimeDiagnostic>());
    let mut scenes = app.world_mut().query::<&VortexSceneRoot>();
    assert!(scenes.iter(app.world()).next().is_none());
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn t22_fixture_identity_remains_authorized_and_typed() -> Result<(), Box<dyn Error>> {
    let project_root: ProjectRoot = "owned-fixture".parse()?;
    let prim: UsdPrimPath = "/Triangle".parse()?;
    assert_eq!(project_root.as_str(), "owned-fixture");
    assert_eq!(prim.as_str(), "/Triangle");
    Ok(())
}
