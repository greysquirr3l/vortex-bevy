use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::Duration,
};

use bevy::{
    asset::AssetServer,
    prelude::{App, MeshMaterial3d, StandardMaterial, WorldAssetRoot},
};
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
        unity::{UnityProjectError, UnityProjectInspector, UnityProjectRequest, UnitySourceKind},
        urp_material::{
            UrpMaterialInput, UrpMaterialMapper, UrpMaterialMapping, UrpShader, UrpSurface,
        },
    },
    application::query::conversion_report::ConversionReportReader,
    domain::{
        ArtifactHash, ArtifactKind, ArtifactRecord, CanonicalScene, ClassificationKind,
        CompatibilityCapability, CompatibilityRequirements, ConversionAggregate,
        ConversionClassification, ConversionContext, CoordinateNormalization, ManifestArtifact,
        ManifestArtifactKind, ManifestSource, OutputRoot, SceneManifest, SourceAssetPath,
        scene_ir::{
            AlphaMode, Material, MaterialId, Mesh, MeshId, Node, NodeId, Primitive, PrimitiveId,
            SourceLink, SourceLinkId, TextureTransform, Transform, VertexStream, VertexStreamId,
        },
    },
};

static FIXTURE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

type PublishedFixture = (
    AtomicArtifactStore,
    vortex_core::domain::ConversionId,
    ArtifactHash,
    String,
    PublicationOutcome,
);

fn project_root(label: &str) -> Result<PathBuf, Box<dyn Error>> {
    let sequence = FIXTURE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!("vortex-t23-{label}-{sequence}"));
    if root.exists() {
        fs::remove_dir_all(&root)?;
    }
    fs::create_dir_all(root.join("Assets"))?;
    fs::create_dir_all(root.join("Packages"))?;
    fs::create_dir_all(root.join("ProjectSettings"))?;
    fs::write(root.join("ProjectSettings/ProjectVersion.txt"), "m_EditorVersion: 2022.3.5f1\n")?;
    fs::write(root.join("ProjectSettings/EditorSettings.asset"), "m_SerializationMode: 2\n")?;
    fs::write(
        root.join("Packages/manifest.json"),
        "{\"dependencies\": {\"com.unity.render-pipelines.universal\": \"12.1.11\"}}",
    )?;
    Ok(root)
}

fn write_meta(path: &Path, guid: &str) -> Result<(), Box<dyn Error>> {
    fs::write(
        PathBuf::from(format!("{}.meta", path.display())),
        format!("fileFormatVersion: 2\nguid: {guid}\n"),
    )?;
    Ok(())
}

fn write_supported_project(root: &Path) -> Result<(), Box<dyn Error>> {
    let scene = root.join("Assets/Fixture.unity");
    fs::write(
        &scene,
        "%YAML 1.1\n--- !u!1 &100\nGameObject:\n  m_Name: Fixture\n--- !u!4 &101\nTransform:\n  m_LocalPosition: {x: 0, y: 0, z: 0}\n--- !u!23 &102\nMeshRenderer:\n",
    )?;
    write_meta(&scene, "0123456789abcdef0123456789abcdef")?;

    let lit = root.join("Assets/Lit.mat");
    fs::write(
        &lit,
        "%YAML 1.1\n--- !u!21 &210\nMaterial:\n  m_Name: Lit\n  m_ShaderName: Universal Render Pipeline/Lit\n",
    )?;
    write_meta(&lit, "fedcba9876543210fedcba9876543210")?;

    let unlit = root.join("Assets/Unlit.mat");
    fs::write(
        &unlit,
        "%YAML 1.1\n--- !u!21 &220\nMaterial:\n  m_Name: Unlit\n  m_ShaderName: Universal Render Pipeline/Unlit\n",
    )?;
    write_meta(&unlit, "00112233445566778899aabbccddeeff")?;
    Ok(())
}

fn selected(paths: &[&str]) -> Result<Vec<SourceAssetPath>, Box<dyn Error>> {
    paths.iter().map(|path| (*path).parse::<SourceAssetPath>().map_err(Into::into)).collect()
}

fn material_input(
    id: u32,
    source: vortex_core::domain::SourceIdentity,
    shader: UrpShader,
) -> UrpMaterialInput {
    UrpMaterialInput {
        id: MaterialId::new(id),
        source,
        shader,
        base_color: if shader == UrpShader::Lit {
            [0.8, 0.4, 0.2, 1.0]
        } else {
            [0.1, 0.8, 0.3, 1.0]
        },
        metallic: 0.35,
        smoothness: 0.6,
        normal_scale: 1.0,
        occlusion_strength: 1.0,
        emissive: [0.0; 3],
        surface: UrpSurface::Opaque,
        alpha_clip: false,
        alpha_cutoff: None,
        double_sided: false,
        texture_transform: TextureTransform::default(),
        textures: Vec::new(),
        unsupported_features: BTreeSet::new(),
    }
}

fn canonical_scene(
    links: Vec<SourceLink>,
    lit: Material,
    unlit: Material,
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
        source_links: links,
        nodes: vec![Node {
            id: NodeId::new(0),
            parent: None,
            transform: Transform::default(),
            mesh: Some(MeshId::new(0)),
            camera: None,
            light: None,
            instance: None,
            source_link: SourceLinkId::new(1),
        }],
        meshes: vec![Mesh {
            id: MeshId::new(0),
            primitives: vec![PrimitiveId::new(0), PrimitiveId::new(1)],
            skin: None,
        }],
        primitives: vec![
            Primitive {
                id: PrimitiveId::new(0),
                vertex_stream: VertexStreamId::new(0),
                material: Some(lit.id),
                morph_targets: Vec::new(),
            },
            Primitive {
                id: PrimitiveId::new(1),
                vertex_stream: VertexStreamId::new(0),
                material: Some(unlit.id),
                morph_targets: Vec::new(),
            },
        ],
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
        materials: vec![lit, unlit],
        textures: Vec::new(),
        samplers: Vec::new(),
        skins: Vec::new(),
        joints: Vec::new(),
        animation_clips: Vec::new(),
        channels: Vec::new(),
        morph_targets: Vec::new(),
        cameras: Vec::new(),
        lights: Vec::new(),
        instances: Vec::new(),
        unsupported_features: Vec::new(),
    })?)
}

fn source_record(
    source: vortex_core::domain::SourceIdentity,
    asset_kind: &str,
    classification: ConversionClassification,
    values: BTreeMap<String, String>,
) -> ManifestSource {
    ManifestSource {
        identity: source,
        asset_kind: asset_kind.to_owned(),
        selected_values: values,
        glb_labels: BTreeMap::from([("node:0".to_owned(), "Fixture".to_owned())]),
        classification,
        diagnostics: Vec::new(),
    }
}

fn publish(
    root: &Path,
    snapshot: &vortex_core::adapters::secondary::unity::UnityProjectSnapshot,
) -> Result<PublishedFixture, Box<dyn Error>> {
    let records = snapshot
        .records
        .iter()
        .filter(|record| record.kind == UnitySourceKind::Material)
        .collect::<Vec<_>>();
    let lit_record = records
        .iter()
        .find(|record| {
            record
                .facts
                .get("unity.shader_name")
                .is_some_and(|shader| shader == UrpShader::Lit.source_name())
        })
        .ok_or("missing Lit material record")?;
    let unlit_record = records
        .iter()
        .find(|record| {
            record
                .facts
                .get("unity.shader_name")
                .is_some_and(|shader| shader == UrpShader::Unlit.source_name())
        })
        .ok_or("missing Unlit material record")?;
    let mapper = UrpMaterialMapper;
    let UrpMaterialMapping { material: lit_material, classification: lit_classification, .. } =
        mapper.map(&material_input(0, lit_record.source.clone(), UrpShader::Lit))?;
    let UrpMaterialMapping {
        material: unlit_material, classification: unlit_classification, ..
    } = mapper.map(&material_input(1, unlit_record.source.clone(), UrpShader::Unlit))?;
    let lit = lit_material.ok_or("Lit fixture did not map to canonical material")?;
    let unlit = unlit_material.ok_or("Unlit fixture did not map to canonical material")?;
    assert_eq!(lit.alpha_mode, AlphaMode::Opaque);
    assert!((unlit.roughness - 1.0).abs() <= f32::EPSILON);

    let scene = canonical_scene(snapshot.source_links()?, lit, unlit)?;
    let first_package = GlbWriter.package(&scene)?;
    let second_package = GlbWriter.package(&scene)?;
    assert_eq!(first_package.bytes, second_package.bytes);
    let profile_hash = ArtifactHash::from_bytes(b"t23-unity-urp-fixture-profile-v1");
    let payload = ArtifactPayload::new("scene.glb", first_package.bytes.clone())?;
    let selection_values = BTreeMap::from([
        ("unity.editor_version".to_owned(), snapshot.provenance.editor_version.clone()),
        ("unity.urp_package_version".to_owned(), snapshot.provenance.urp_package_version.clone()),
        ("unity.serialization_mode".to_owned(), snapshot.provenance.serialization_mode.clone()),
        ("unity.urp.material_support".to_owned(), "lit,unlit".to_owned()),
    ]);
    let manifest = SceneManifest::new(
        "0.1.0",
        profile_hash.clone(),
        vec![ManifestArtifact {
            kind: ManifestArtifactKind::Glb,
            path: payload.relative_path.clone(),
            hash: payload.hash(),
        }],
        vec![
            source_record(
                lit_record.source.clone(),
                "urp_lit_material",
                lit_classification.clone(),
                selection_values.clone(),
            ),
            source_record(
                unlit_record.source.clone(),
                "urp_unlit_material",
                unlit_classification,
                selection_values,
            ),
        ],
        CompatibilityRequirements {
            minimum_bevy_version: "0.19".to_owned(),
            required_capabilities: BTreeSet::from([CompatibilityCapability::BevyGltfPbr]),
        },
    )?;
    let canonical_manifest = manifest.to_ron()?;
    let source = lit_record.source.clone();
    let mut conversion = ConversionAggregate::begin(
        source,
        ConversionContext::new("default".parse()?, profile_hash, "0.1.0")?,
    );
    conversion.record_artifact(ArtifactRecord::new(
        ArtifactKind::Glb,
        payload.hash(),
        "scene.glb",
    )?)?;
    conversion.record_artifact(ArtifactRecord::new(
        ArtifactKind::Manifest,
        ArtifactHash::from_bytes(canonical_manifest.as_bytes()),
        "scene.manifest.ron",
    )?)?;
    conversion.classify(lit_classification)?;
    let conversion = conversion.finalize()?;
    let id = conversion.id().clone();
    let store = AtomicArtifactStore::new(root, "t23-output".parse::<OutputRoot>()?)?;
    let request = PublicationRequest { conversion, manifest, payloads: vec![payload] };
    let outcome = store.publish(&request)?;
    assert_eq!(store.publish(&request)?, PublicationOutcome::Reused);
    Ok((store, id, first_package.checksum, canonical_manifest, outcome))
}

fn tick_until_loaded(app: &mut App, entity: bevy::prelude::Entity) -> Result<(), Box<dyn Error>> {
    for _ in 0..80 {
        app.update();
        let world = app.world();
        let root =
            world.entity(entity).get::<WorldAssetRoot>().ok_or("missing world asset root")?;
        let server = world.get_resource::<AssetServer>().ok_or("missing asset server")?;
        if server.is_loaded_with_dependencies(root.0.id()) {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(5));
    }
    Err("Unity fixture GLB did not resolve in bounded headless ticks".into())
}

#[test]
fn owned_urp_lit_and_unlit_fixture_is_deterministic_published_and_loaded()
-> Result<(), Box<dyn Error>> {
    let root = project_root("success")?;
    write_supported_project(&root)?;
    let forward = UnityProjectRequest::new(
        &root,
        selected(&["Assets/Fixture.unity", "Assets/Lit.mat", "Assets/Unlit.mat"])?,
    )?;
    let reverse = UnityProjectRequest::new(
        &root,
        selected(&["Assets/Unlit.mat", "Assets/Lit.mat", "Assets/Fixture.unity"])?,
    )?;
    let inspector = UnityProjectInspector;
    let first = inspector.inspect(&forward)?;
    let second = inspector.inspect(&reverse)?;
    assert_eq!(first, second);
    let (store, id, checksum, manifest, first_outcome) = publish(&root, &first)?;
    let (_, replay_id, replay_checksum, replay_manifest, replay_outcome) = publish(&root, &second)?;
    assert_eq!(first_outcome, PublicationOutcome::Published);
    assert_eq!(replay_outcome, PublicationOutcome::Reused);
    assert_eq!(id, replay_id);
    assert_eq!(checksum, replay_checksum);
    assert_eq!(manifest, replay_manifest);
    assert_eq!(store.verify_published(&id)?.verified_hashes.len(), 1);
    assert!(store.read(&id)?.is_some());

    let asset_root = root.join(id.as_str());
    let mut app = build_runtime_app(asset_root.to_string_lossy().into_owned());
    let entity = load_published_conversion_and_spawn(&mut app, &store, &id)?;
    tick_until_loaded(&mut app, entity)?;
    let world = app.world_mut();
    assert!(world.entity(entity).contains::<VortexSceneRoot>());
    let mut materials = world.query::<&MeshMaterial3d<StandardMaterial>>();
    assert!(materials.iter(world).count() >= 2);
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn malformed_and_shader_graph_unity_fixtures_are_visible_failures() -> Result<(), Box<dyn Error>> {
    let root = project_root("failures")?;
    let graph = root.join("Assets/Graph.mat");
    fs::write(
        &graph,
        "%YAML 1.1\n--- !u!21 &300\nMaterial:\n  m_ShaderName: Shader Graphs/Custom\n",
    )?;
    write_meta(&graph, "11112222333344445555666677778888")?;
    let graph_request = UnityProjectRequest::new(&root, selected(&["Assets/Graph.mat"])?)?;
    let graph_snapshot = UnityProjectInspector.inspect(&graph_request)?;
    let graph_record = graph_snapshot.records.first().ok_or("missing Shader Graph record")?;
    assert_eq!(graph_record.classification.kind, ClassificationKind::Unsupported);

    let malformed = root.join("Assets/Malformed.unity");
    fs::write(&malformed, [0_u8, 1, 2])?;
    write_meta(&malformed, "9999aaaabbbbccccddddeeeeffff0000")?;
    let malformed_request =
        UnityProjectRequest::new(&root, selected(&["Assets/Malformed.unity"])?)?;
    assert!(matches!(
        UnityProjectInspector.inspect(&malformed_request),
        Err(UnityProjectError::BinaryOrUnknownSerialization { .. })
    ));

    write_supported_project(&root)?;
    let valid = UnityProjectInspector.inspect(&UnityProjectRequest::new(
        &root,
        selected(&["Assets/Fixture.unity", "Assets/Lit.mat", "Assets/Unlit.mat"])?,
    )?)?;
    let (store, id, _, _, outcome) = publish(&root, &valid)?;
    assert_eq!(outcome, PublicationOutcome::Published);
    let asset_root = root.join(id.as_str());
    fs::write(asset_root.join("scene.glb"), b"tampered Unity fixture artifact")?;
    let mut app = build_runtime_app(asset_root.to_string_lossy().into_owned());
    let diagnostic = load_published_conversion_or_diagnostic(&mut app, &store, &id);
    assert!(app.world().entity(diagnostic).contains::<VortexRuntimeDiagnostic>());
    let mut roots = app.world_mut().query::<&VortexSceneRoot>();
    assert!(roots.iter(app.world()).next().is_none());
    fs::remove_dir_all(root)?;
    Ok(())
}
