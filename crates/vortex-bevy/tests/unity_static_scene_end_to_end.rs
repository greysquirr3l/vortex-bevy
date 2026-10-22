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
    VortexSceneRoot, build_runtime_app, load_published_conversion_and_spawn,
};
use vortex_core::{
    adapters::secondary::{
        artifact_store::{
            ArtifactPayload, AtomicArtifactStore, PublicationOutcome, PublicationRequest,
        },
        glb::GlbWriter,
        unity_static_scene::{
            MAX_STATIC_SCENE_REPORT_BYTES, UnityStaticSceneAdapter, UnityStaticSceneError,
        },
    },
    domain::{
        ArtifactHash, ArtifactKind, ArtifactRecord, ClassificationKind, CompatibilityCapability,
        CompatibilityRequirements, ConversionAggregate, ConversionClassification,
        ConversionContext, ManifestArtifact, ManifestArtifactKind, ManifestSource, OutputRoot,
        ProjectRoot, SceneManifest,
    },
};

static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn test_root(label: &str) -> Result<PathBuf, Box<dyn Error>> {
    let sequence = TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!("vortex-unity-static-scene-{label}-{sequence}"));
    if root.exists() {
        fs::remove_dir_all(&root)?;
    }
    fs::create_dir_all(root.join("staging/textures"))?;
    Ok(root)
}

fn base_color_png() -> Vec<u8> {
    vec![
        137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6,
        0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 120, 156, 99, 248, 207, 192, 240,
        31, 0, 5, 0, 1, 255, 137, 153, 61, 29, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
    ]
}

fn report() -> String {
    r#"{
  "engine":"unity",
  "protocol_version":"vortex.unity-static-scene/v1",
  "editor_version":"6000.5.10f1",
  "urp_package_version":"17.5.0",
  "scene":{
    "source_guid":"0123456789abcdef0123456789abcdef",
    "source_path":"Assets/VortexBridgeTemporary/StaticScene.unity",
    "object_id":1001,
    "mesh":{
      "positions":[{"x":-0.5,"y":0.0,"z":0.0},{"x":0.5,"y":0.0,"z":0.0},{"x":0.0,"y":1.0,"z":0.0}],
      "normals":[{"x":0.0,"y":0.0,"z":1.0},{"x":0.0,"y":0.0,"z":1.0},{"x":0.0,"y":0.0,"z":1.0}],
      "uvs":[{"x":0.0,"y":0.0},{"x":1.0,"y":0.0},{"x":0.5,"y":1.0}],
      "indices":[0,1,2]
    },
    "material":{
      "shader":"Universal Render Pipeline/Lit",
      "base_color":{"r":0.8,"g":0.4,"b":0.2,"a":1.0},
      "metallic":0.35,
      "smoothness":0.6,
      "base_color_texture":"textures/vortex-static-base-color.png"
    },
    "camera":{
      "position":{"x":0.0,"y":0.5,"z":-3.0},
      "rotation":{"x":0.0,"y":0.0,"z":0.0,"w":1.0},
      "scale":{"x":1.0,"y":1.0,"z":1.0},
      "vertical_fov_radians":1.0471976
    },
    "light":{
      "position":{"x":0.0,"y":3.0,"z":-1.0},
      "rotation":{"x":0.0,"y":0.0,"z":0.0,"w":1.0},
      "scale":{"x":1.0,"y":1.0,"z":1.0},
      "kind":"directional",
      "intensity":1.0
    }
  },
  "unsupported_components":[{
    "component":"BoxCollider",
    "classification":"requires_adapter",
    "reason":"BoxCollider physics shape is outside this render-scene conversion subset."
  }]
}"#
    .to_owned()
}

fn tick_until_loaded(app: &mut App, entity: bevy::prelude::Entity) -> Result<(), Box<dyn Error>> {
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
    Err(format!(
        "published Unity static-scene GLB did not load in bounded headless ticks; states: {:?}",
        server.get_load_states(root.0.id())
    )
    .into())
}

fn scene_source(
    source: vortex_core::domain::SourceIdentity,
    classification: ConversionClassification,
) -> ManifestSource {
    ManifestSource {
        identity: source,
        asset_kind: "unity_editor_generated_static_scene".to_owned(),
        selected_values: BTreeMap::from([
            ("unity.editor_version".to_owned(), "6000.5.10f1".to_owned()),
            ("unity.urp_package_version".to_owned(), "17.5.0".to_owned()),
            ("unity.material.shader".to_owned(), "Universal Render Pipeline/Lit".to_owned()),
            ("unity.unsupported_components".to_owned(), "BoxCollider:requires_adapter".to_owned()),
        ]),
        glb_labels: BTreeMap::from([
            ("node:1".to_owned(), "StaticMesh".to_owned()),
            ("node:2".to_owned(), "Camera".to_owned()),
            ("node:3".to_owned(), "DirectionalLight".to_owned()),
        ]),
        classification,
        diagnostics: Vec::new(),
    }
}

fn write_staged_png(root: &Path) -> Result<(), Box<dyn Error>> {
    fs::write(root.join("staging/textures/vortex-static-base-color.png"), base_color_png())?;
    Ok(())
}

#[test]
fn self_authored_static_scene_report_is_deterministic_published_and_loaded()
-> Result<(), Box<dyn Error>> {
    let root = test_root("success")?;
    write_staged_png(&root)?;
    let project_root: ProjectRoot = "authorized-test123".parse()?;
    let adapter = UnityStaticSceneAdapter;
    let first = adapter.load_and_convert(
        project_root.clone(),
        &root.join("staging"),
        report().as_bytes(),
    )?;
    let second =
        adapter.load_and_convert(project_root, &root.join("staging"), report().as_bytes())?;
    assert_eq!(first, second);
    assert_eq!(first.material_classification.kind, ClassificationKind::Converted);
    assert_eq!(first.scene.unsupported_features.len(), 1);
    let unsupported_feature =
        first.scene.unsupported_features.first().ok_or("missing explicit unsupported component")?;
    assert_eq!(unsupported_feature.classification.kind, ClassificationKind::RequiresAdapter);
    assert_eq!(first.scene.cameras.len(), 1);
    assert_eq!(first.scene.lights.len(), 1);
    assert_eq!(first.scene.textures.len(), 1);

    let first_package = GlbWriter.package(&first.scene)?;
    let second_package = GlbWriter.package(&second.scene)?;
    assert_eq!(first_package.bytes, second_package.bytes);
    let glb_payload = ArtifactPayload::new("scene.glb", first_package.bytes)?;
    let texture_payload = ArtifactPayload::new(
        first.base_color_texture.texture.image.clone(),
        first.base_color_texture.bytes.clone(),
    )?;
    let profile_hash = ArtifactHash::from_bytes(b"unity-static-scene-v1");
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
        ],
        vec![scene_source(first.source.clone(), first.material_classification.clone())],
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
    let mut conversion = ConversionAggregate::begin(
        first.source.clone(),
        ConversionContext::new("default".parse()?, profile_hash, "0.1.0")?,
    );
    conversion.record_artifact(ArtifactRecord::new(
        ArtifactKind::Glb,
        glb_payload.hash(),
        "scene.glb",
    )?)?;
    conversion.record_artifact(ArtifactRecord::new(
        ArtifactKind::Texture,
        texture_payload.hash(),
        texture_payload.relative_path.clone(),
    )?)?;
    conversion.record_artifact(ArtifactRecord::new(
        ArtifactKind::Manifest,
        ArtifactHash::from_bytes(manifest_ron.as_bytes()),
        "scene.manifest.ron",
    )?)?;
    conversion.classify(first.material_classification)?;
    let conversion = conversion.finalize()?;
    let id = conversion.id().clone();
    let store = AtomicArtifactStore::new(&root, "published".parse::<OutputRoot>()?)?;
    let request =
        PublicationRequest { conversion, manifest, payloads: vec![glb_payload, texture_payload] };
    assert_eq!(store.publish(&request)?, PublicationOutcome::Published);
    assert_eq!(store.publish(&request)?, PublicationOutcome::Reused);
    assert_eq!(store.verify_published(&id)?.verified_hashes.len(), 2);

    let asset_root = root.join(id.as_str());
    let mut app = build_runtime_app(asset_root.to_string_lossy().into_owned());
    let entity = load_published_conversion_and_spawn(&mut app, &store, &id)?;
    tick_until_loaded(&mut app, entity)?;
    let world = app.world_mut();
    assert!(world.entity(entity).contains::<VortexSceneRoot>());
    let mut materials = world.query::<&MeshMaterial3d<StandardMaterial>>();
    assert!(materials.iter(world).next().is_some());
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn static_scene_report_rejects_oversize_and_path_escape() -> Result<(), Box<dyn Error>> {
    let root = test_root("failures")?;
    write_staged_png(&root)?;
    let adapter = UnityStaticSceneAdapter;
    let project_root: ProjectRoot = "authorized-test123".parse()?;
    let oversized = vec![b'x'; MAX_STATIC_SCENE_REPORT_BYTES + 1];
    assert!(matches!(
        adapter.convert(project_root.clone(), &oversized, base_color_png()),
        Err(UnityStaticSceneError::ReportTooLarge { .. })
    ));
    let escaping_report =
        report().replace("textures/vortex-static-base-color.png", "textures/../outside.png");
    assert!(matches!(
        adapter.load_and_convert(project_root, &root.join("staging"), escaping_report.as_bytes()),
        Err(UnityStaticSceneError::UnsafeTexturePath { .. })
    ));
    fs::remove_dir_all(root)?;
    Ok(())
}
