use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::Duration,
};

use bevy::{asset::AssetServer, prelude::WorldAssetRoot};
use vortex_bevy::runtime::{
    VortexPackagedVariant, VortexSceneChunk, VortexSceneRoot, VortexSourceLink,
    VortexVariantSelection, build_runtime_app, load_packaged_chunk,
    load_published_conversion_and_spawn, switch_packaged_variant,
};
use vortex_core::{
    adapters::secondary::{
        artifact_store::{
            ArtifactPayload, AtomicArtifactStore, PublicationOutcome, PublicationRequest,
        },
        usd_packages::{UsdPackageInput, UsdPackagePlanner},
        usd_stage::{UsdPrimSnapshot, UsdStageSnapshot},
    },
    domain::{
        ArtifactHash, ArtifactKind, ArtifactRecord, CanonicalScene, ClassificationKind,
        CompatibilityCapability, CompatibilityRequirements, ConversionAggregate,
        ConversionClassification, ConversionContext, CoordinateNormalization, ManifestArtifact,
        ManifestArtifactKind, ManifestSource, OutputRoot, PackagedSceneId, PackagedSceneMode,
        ProjectRoot, Remediation, SceneChunkBounds, SceneManifest, SourceAssetPath, SourceIdentity,
        StageSelection, UsdPrimPath,
        scene_ir::{
            Instance, InstanceId, Mesh, MeshId, Node, NodeId, Primitive, PrimitiveId, SourceLink,
            SourceLinkId, VertexStream, VertexStreamId,
        },
    },
};

static NEXT_TEST_ROOT: AtomicU64 = AtomicU64::new(0);

fn test_root() -> Result<PathBuf, Box<dyn Error>> {
    let sequence = NEXT_TEST_ROOT.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!("vortex-t25-{sequence}"));
    if root.exists() {
        fs::remove_dir_all(&root)?;
    }
    fs::create_dir_all(&root)?;
    Ok(root)
}

fn source() -> Result<SourceIdentity, Box<dyn Error>> {
    Ok(SourceIdentity::Usd {
        stage: "fixtures/t25.usda".parse::<SourceAssetPath>()?,
        prim_path: "/World".parse::<UsdPrimPath>()?,
        selection: "context=universal;payload=forbid;purpose=render;time=stage;variants=look:blue"
            .parse::<StageSelection>()?,
    })
}

fn snapshot(source: SourceIdentity) -> Result<UsdStageSnapshot, Box<dyn Error>> {
    Ok(UsdStageSnapshot {
        source,
        classification: ConversionClassification::new(
            ClassificationKind::RequiresAdapter,
            "explicit prepackaged alternatives avoid live source composition",
            Remediation::new("use an authoritative OpenUSD adapter for live composition")?,
        )?,
        facts: vec!["stage.bytes=128".to_owned()],
        prims: vec![UsdPrimSnapshot {
            path: "/World".parse::<UsdPrimPath>()?,
            specifier: "def".to_owned(),
            type_name: Some("Xform".to_owned()),
            purpose: Some("render".to_owned()),
            visibility: Some("inherited".to_owned()),
            instancable: true,
            transforms: Vec::new(),
        }],
    })
}

fn scene(
    source: SourceIdentity,
    translation: f32,
    instance: bool,
) -> Result<CanonicalScene, Box<dyn Error>> {
    Ok(CanonicalScene::new(CanonicalScene {
        normalization: CoordinateNormalization {
            meters_per_unit: "1".to_owned(),
            source_up_axis: "Y".to_owned(),
            target_up_axis: "Y".to_owned(),
            winding: "preserve".to_owned(),
            uv_origin: "lower_left".to_owned(),
            color_space: "linear".to_owned(),
        },
        source_links: vec![SourceLink { id: SourceLinkId::new(0), source, facts: BTreeMap::new() }],
        nodes: vec![Node {
            id: NodeId::new(0),
            parent: None,
            transform: vortex_core::domain::scene_ir::Transform {
                translation: [f64::from(translation), 0.0, 0.0],
                ..Default::default()
            },
            mesh: (!instance).then_some(MeshId::new(0)),
            camera: None,
            light: None,
            instance: instance.then_some(InstanceId::new(0)),
            source_link: SourceLinkId::new(0),
        }],
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
        cameras: Vec::new(),
        lights: Vec::new(),
        instances: instance
            .then_some(Instance { id: InstanceId::new(0), mesh: MeshId::new(0) })
            .into_iter()
            .collect(),
        unsupported_features: Vec::new(),
    })?)
}

fn input(
    id: &str,
    mode: PackagedSceneMode,
    parent: Option<&str>,
    selected_values: BTreeMap<String, String>,
    bounds: Option<SceneChunkBounds>,
    scene: CanonicalScene,
) -> Result<UsdPackageInput, Box<dyn Error>> {
    Ok(UsdPackageInput {
        id: id.parse()?,
        mode,
        parent: parent.map(str::parse).transpose()?,
        selected_values,
        bounds,
        scene,
    })
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
            .ok_or("packaged scene is missing WorldAssetRoot")?;
        let asset_server =
            world.get_resource::<AssetServer>().ok_or("packaged runtime has no AssetServer")?;
        if asset_server.is_loaded_with_dependencies(root.0.id()) {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(5));
    }
    Err("packaged scene did not resolve within bounded headless ticks".into())
}

fn package_plan(
    selected_source: &SourceIdentity,
) -> Result<vortex_core::adapters::secondary::usd_packages::UsdPackagePlan, Box<dyn Error>> {
    Ok(UsdPackagePlanner::default().package(
        &snapshot(selected_source.clone())?,
        vec![
            input(
                "base",
                PackagedSceneMode::Flattened,
                None,
                BTreeMap::new(),
                None,
                scene(selected_source.clone(), 0.0, false)?,
            )?,
            input(
                "look-blue",
                PackagedSceneMode::VariantAlternative,
                Some("base"),
                BTreeMap::from([("variant.look".to_owned(), "blue".to_owned())]),
                None,
                scene(selected_source.clone(), 2.0, false)?,
            )?,
            input(
                "expanded",
                PackagedSceneMode::ExpandedInstances,
                Some("base"),
                BTreeMap::new(),
                None,
                scene(selected_source.clone(), 0.0, true)?,
            )?,
            input(
                "instances",
                PackagedSceneMode::InstanceBatch,
                Some("base"),
                BTreeMap::new(),
                None,
                scene(selected_source.clone(), 0.0, true)?,
            )?,
            input(
                "west",
                PackagedSceneMode::Chunk,
                Some("base"),
                BTreeMap::new(),
                Some(SceneChunkBounds::new([-1_000, -1_000, -1_000], [0, 1_000, 1_000])?),
                scene(selected_source.clone(), -1.0, false)?,
            )?,
        ],
    )?)
}

fn publish_plan(
    root: &PathBuf,
    selected_source: SourceIdentity,
    plan: &vortex_core::adapters::secondary::usd_packages::UsdPackagePlan,
) -> Result<(AtomicArtifactStore, vortex_core::domain::ConversionId), Box<dyn Error>> {
    let payloads = plan
        .artifacts
        .iter()
        .map(|artifact| {
            ArtifactPayload::new(&artifact.declaration.artifact_path, artifact.glb.bytes.clone())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let profile_hash = ArtifactHash::from_bytes(b"t25-packaged-usd-profile-v1");
    let manifest =
        package_manifest(profile_hash.clone(), &selected_source, &payloads, plan.declarations())?;
    let context = ConversionContext::new("default".parse()?, profile_hash, "0.1.0")?;
    let primary = payloads
        .iter()
        .find(|payload| payload.relative_path == "scenes/base.glb")
        .ok_or("flattened package payload is missing")?;
    let mut conversion = ConversionAggregate::begin(selected_source, context);
    conversion.record_artifact(ArtifactRecord::new(
        ArtifactKind::Glb,
        primary.hash(),
        primary.relative_path.clone(),
    )?)?;
    conversion.record_artifact(ArtifactRecord::new(
        ArtifactKind::Manifest,
        ArtifactHash::from_bytes(b"t25-package-manifest"),
        "scene.manifest.ron",
    )?)?;
    conversion.classify(package_classification()?)?;
    let conversion = conversion.finalize()?;
    let conversion_id = conversion.id().clone();
    let store = AtomicArtifactStore::new(root, "t25-output".parse::<OutputRoot>()?)?;
    let request = PublicationRequest { conversion, manifest, payloads };
    assert_eq!(store.publish(&request)?, PublicationOutcome::Published);
    assert_eq!(store.publish(&request)?, PublicationOutcome::Reused);
    assert_eq!(store.verify_published(&conversion_id)?.verified_hashes.len(), 5);
    Ok((store, conversion_id))
}

fn package_manifest(
    profile_hash: ArtifactHash,
    selected_source: &SourceIdentity,
    payloads: &[ArtifactPayload],
    packages: Vec<vortex_core::domain::PackagedScene>,
) -> Result<SceneManifest, Box<dyn Error>> {
    Ok(SceneManifest::new(
        "0.1.0",
        profile_hash,
        payloads
            .iter()
            .map(|payload| ManifestArtifact {
                kind: ManifestArtifactKind::Glb,
                path: payload.relative_path.clone(),
                hash: payload.hash(),
            })
            .collect(),
        vec![ManifestSource {
            identity: selected_source.clone(),
            asset_kind: "usd.stage".to_owned(),
            selected_values: BTreeMap::new(),
            glb_labels: BTreeMap::new(),
            classification: package_classification()?,
            diagnostics: Vec::new(),
        }],
        CompatibilityRequirements {
            minimum_bevy_version: "0.20".to_owned(),
            required_capabilities: BTreeSet::from([
                CompatibilityCapability::BevyGltfPbr,
                CompatibilityCapability::UsdFlattenedStage,
                CompatibilityCapability::UsdPackagedVariants,
                CompatibilityCapability::UsdExpandedInstances,
                CompatibilityCapability::UsdInstanceBatches,
                CompatibilityCapability::UsdSceneChunks,
            ]),
        },
    )?
    .with_packaged_scenes(packages)?)
}

fn package_classification() -> Result<ConversionClassification, Box<dyn Error>> {
    Ok(ConversionClassification::new(
        ClassificationKind::RequiresAdapter,
        "prepackaged USD alternatives only",
        Remediation::new("use an authoritative OpenUSD adapter for live composition")?,
    )?)
}

fn assert_runtime_behavior(
    app: &mut bevy::prelude::App,
    parent: bevy::prelude::Entity,
    selected_source: &SourceIdentity,
) -> Result<(), Box<dyn Error>> {
    let missing_variant: PackagedSceneId = "look-red".parse()?;
    assert!(switch_packaged_variant(app, parent, &missing_variant).is_err());
    assert!(app.world().entity(parent).contains::<VortexSceneRoot>());
    let unrelated = app.world_mut().spawn(VortexSceneRoot).id();
    let variant_id: PackagedSceneId = "look-blue".parse()?;
    let variant = switch_packaged_variant(app, parent, &variant_id)?;
    tick_until_loaded(app, variant)?;
    assert_eq!(switch_packaged_variant(app, parent, &variant_id)?, variant);
    assert!(switch_packaged_variant(app, parent, &missing_variant).is_err());
    assert!(app.world().get_entity(variant).is_ok());
    assert!(app.world().get_entity(unrelated).is_ok());
    let chunk_id: PackagedSceneId = "west".parse()?;
    let chunk = load_packaged_chunk(app, parent, &chunk_id)?;
    tick_until_loaded(app, chunk)?;
    assert_eq!(load_packaged_chunk(app, parent, &chunk_id)?, chunk);
    assert_eq!(
        app.world()
            .entity(parent)
            .get::<VortexVariantSelection>()
            .ok_or("parent root lost variant selection")?
            .0
            .get(&selected_source.to_string()),
        Some(&BTreeMap::from([("variant.look".to_owned(), "blue".to_owned())]))
    );
    assert_eq!(
        app.world()
            .entity(variant)
            .get::<VortexPackagedVariant>()
            .ok_or("variant instance lost package provenance")?
            .id,
        variant_id
    );
    let chunk_provenance = app
        .world()
        .entity(chunk)
        .get::<VortexSceneChunk>()
        .ok_or("chunk instance lost package provenance")?;
    assert_eq!(chunk_provenance.parent, parent);
    assert_eq!(chunk_provenance.id, chunk_id);
    assert_eq!(
        app.world()
            .entity(chunk)
            .get::<VortexSourceLink>()
            .ok_or("chunk instance lost source provenance")?
            .0
            .len(),
        1
    );
    Ok(())
}

#[test]
fn packaged_usd_variant_and_chunk_preserve_provenance_without_replacing_the_parent_root()
-> Result<(), Box<dyn Error>> {
    let root = test_root()?;
    let selected_source = source()?;
    let plan = package_plan(&selected_source)?;
    let (store, conversion_id) = publish_plan(&root, selected_source.clone(), &plan)?;
    let asset_root = root.join(conversion_id.as_str());
    let mut app = build_runtime_app(asset_root.to_string_lossy().into_owned());
    let parent = load_published_conversion_and_spawn(&mut app, &store, &conversion_id)?;
    tick_until_loaded(&mut app, parent)?;
    assert_runtime_behavior(&mut app, parent, &selected_source)?;
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn packaged_usd_chunk_identifier_is_stable_for_matching_typed_source_and_bounds()
-> Result<(), Box<dyn Error>> {
    let source = source()?;
    let bounds = SceneChunkBounds::new([0, 0, 0], [1_000, 1_000, 1_000])?;
    let parent: PackagedSceneId = "base".parse()?;
    let first = UsdPackagePlanner::chunk_id(&parent, &source, &bounds)?;
    let second = UsdPackagePlanner::chunk_id(&parent, &source, &bounds)?;
    assert_eq!(first, second);
    Ok(())
}

#[test]
fn package_manifest_rejects_a_child_without_the_flattened_parent_artifact()
-> Result<(), Box<dyn Error>> {
    let source = source()?;
    let profile_hash = ArtifactHash::from_bytes(b"t25-invalid-parent");
    let glb = ArtifactHash::from_bytes(b"not-a-real-glb");
    let package = vortex_core::domain::PackagedScene::new(
        "variant".parse()?,
        PackagedSceneMode::VariantAlternative,
        "variants/variant.glb",
        Some("missing".parse()?),
        vec![source.clone()],
        BTreeMap::from([("variant.look".to_owned(), "blue".to_owned())]),
        None,
    )?;
    let manifest = SceneManifest::new(
        "0.1.0",
        profile_hash,
        vec![ManifestArtifact {
            kind: ManifestArtifactKind::Glb,
            path: "variants/variant.glb".to_owned(),
            hash: glb,
        }],
        vec![ManifestSource {
            identity: source,
            asset_kind: "usd.stage".to_owned(),
            selected_values: BTreeMap::new(),
            glb_labels: BTreeMap::new(),
            classification: ConversionClassification::new(
                ClassificationKind::RequiresAdapter,
                "prepackaged USD alternatives only",
                Remediation::new("use an authoritative OpenUSD adapter for live composition")?,
            )?,
            diagnostics: Vec::new(),
        }],
        CompatibilityRequirements {
            minimum_bevy_version: "0.20".to_owned(),
            required_capabilities: BTreeSet::new(),
        },
    )?
    .with_packaged_scenes(vec![package]);
    assert!(manifest.is_err());
    Ok(())
}

#[test]
fn root_identity_is_typed_without_a_project_fixture() -> Result<(), Box<dyn Error>> {
    let project: ProjectRoot = "owned-t25".parse()?;
    assert_eq!(project.as_str(), "owned-t25");
    Ok(())
}
