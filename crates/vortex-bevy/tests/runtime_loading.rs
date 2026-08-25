use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fs,
    path::{Path, PathBuf},
    str::FromStr,
    thread,
    time::Duration,
};

use bevy::{
    asset::AssetServer,
    prelude::{App, Entity, Name, WorldAssetRoot},
};
use vortex_bevy::runtime::{
    ViewerCommand, ViewerCommandQueue, VortexArtifactHash, VortexConversionStatus,
    VortexDiagnosticSet, VortexSceneLifecycle, VortexSceneRoot, VortexSourceLink,
    VortexVariantSelection, VortexViewerState, build_runtime_app, inspect_scene,
    load_manifest_and_spawn, reload_scene, unload_scene, verify_manifest,
};
use vortex_core::domain::{
    ArtifactHash, ClassificationKind, CompatibilityCapability, CompatibilityRequirements,
    ConversionClassification, Diagnostic, DiagnosticCode, ManifestArtifact, ManifestArtifactKind,
    ManifestSource, ProjectRoot, Remediation, SceneManifest, SourceAssetPath, SourceIdentity,
    UnrealObjectPath,
};

fn source_identity() -> Result<SourceIdentity, Box<dyn Error>> {
    Ok(SourceIdentity::Unreal {
        project_root: ProjectRoot::from_str("FixtureProject")?,
        asset_path: SourceAssetPath::from_str("Content/RuntimeFixture.uasset")?,
        object_path: UnrealObjectPath::from_str("/Game/RuntimeFixture.RuntimeFixture")?,
    })
}

fn fixture_root(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target").join(name)
}

fn reset_root(root: &Path) -> Result<(), Box<dyn Error>> {
    if root.exists() {
        fs::remove_dir_all(root)?;
    }
    fs::create_dir_all(root)?;
    Ok(())
}

fn minimal_glb() -> Result<Vec<u8>, Box<dyn Error>> {
    let mut json = br#"{"asset":{"version":"2.0"},"scene":0,"scenes":[{"nodes":[0]}],"nodes":[{"name":"RuntimeTriangle"}]}"#.to_vec();
    while !json.len().is_multiple_of(4) {
        json.push(b' ');
    }
    let json_len = u32::try_from(json.len())?;
    let total_len = 20_u32.checked_add(json_len).ok_or("fixture GLB length overflow")?;
    let mut glb = Vec::new();
    for header in [0x4654_6c67_u32, 2, total_len, json_len, 0x4e4f_534a] {
        glb.extend_from_slice(&header.to_le_bytes());
    }
    glb.extend_from_slice(&json);
    Ok(glb)
}

fn write_manifest_fixture(
    root: &Path,
    capability: CompatibilityCapability,
) -> Result<PathBuf, Box<dyn Error>> {
    let glb = minimal_glb()?;
    let glb_path = root.join("scene.glb");
    fs::write(&glb_path, &glb)?;
    let manifest = SceneManifest::new(
        "0.1.0",
        ArtifactHash::from_bytes(b"t20-profile"),
        vec![ManifestArtifact {
            kind: ManifestArtifactKind::Glb,
            path: "scene.glb".to_owned(),
            hash: ArtifactHash::from_bytes(&glb),
        }],
        vec![ManifestSource {
            identity: source_identity()?,
            asset_kind: "static_mesh".to_owned(),
            selected_values: BTreeMap::from([("variant".to_owned(), "Runtime".to_owned())]),
            glb_labels: BTreeMap::from([(
                "node:RuntimeTriangle".to_owned(),
                "/Game/RuntimeFixture.RuntimeFixture".to_owned(),
            )]),
            classification: ConversionClassification::new(
                ClassificationKind::Converted,
                "fixture is supported by the runtime subset",
                Remediation::new("Use the runtime fixture profile.")?,
            )?,
            diagnostics: Vec::new(),
        }],
        CompatibilityRequirements {
            minimum_bevy_version: "0.19".to_owned(),
            required_capabilities: BTreeSet::from([capability]),
        },
    )?;
    let manifest_path = root.join("scene.manifest.ron");
    fs::write(&manifest_path, manifest.to_ron()?)?;
    Ok(manifest_path)
}

fn tick_until_world_asset_loaded(app: &mut App, entity: Entity) -> Result<(), String> {
    for _ in 0..80 {
        app.update();
        let loaded = {
            let world = app.world();
            let Some(root) = world.entity(entity).get::<WorldAssetRoot>() else {
                return Err("runtime root was unexpectedly despawned".to_owned());
            };
            let Some(asset_server) = world.get_resource::<AssetServer>() else {
                return Err("runtime app lost its AssetServer".to_owned());
            };
            asset_server.is_loaded_with_dependencies(root.0.id())
        };
        if loaded {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(5));
    }
    let world = app.world();
    let Some(root) = world.entity(entity).get::<WorldAssetRoot>() else {
        return Err("runtime root was unexpectedly despawned".to_owned());
    };
    let Some(asset_server) = world.get_resource::<AssetServer>() else {
        return Err("runtime app lost its AssetServer".to_owned());
    };
    Err(format!("world asset never resolved: {:?}", asset_server.get_load_states(root.0.id())))
}

#[test]
fn verified_manifest_spawns_world_asset_root_with_provenance_components()
-> Result<(), Box<dyn Error>> {
    let root = fixture_root("t20-runtime-valid");
    reset_root(&root)?;
    let manifest = write_manifest_fixture(&root, CompatibilityCapability::BevyGltfPbr)?;
    let mut app = build_runtime_app(root.to_string_lossy().into_owned());
    let entity = load_manifest_and_spawn(&mut app, &manifest)?;

    let world = app.world();
    let entity_ref = world.entity(entity);
    assert!(entity_ref.contains::<WorldAssetRoot>());
    assert!(entity_ref.contains::<VortexSceneRoot>());
    assert!(entity_ref.contains::<VortexSourceLink>());
    assert!(entity_ref.contains::<VortexConversionStatus>());
    assert!(entity_ref.contains::<VortexArtifactHash>());
    assert!(entity_ref.contains::<VortexDiagnosticSet>());
    assert!(entity_ref.contains::<VortexVariantSelection>());
    let Some(links) = entity_ref.get::<VortexSourceLink>() else {
        return Err("spawned runtime root should retain source links".into());
    };
    assert_eq!(links.0.len(), 1);
    let Some(variants) = entity_ref.get::<VortexVariantSelection>() else {
        return Err("spawned runtime root should retain selected values".into());
    };
    assert_eq!(variants.0.len(), 1);
    if let Err(error) = tick_until_world_asset_loaded(&mut app, entity) {
        return Err(Box::<dyn Error>::from(error));
    }
    let world = app.world_mut();
    let mut names = world.query::<&Name>();
    assert!(names.iter(world).any(|name| name.as_str() == "RuntimeTriangle"));

    Ok(())
}

#[test]
fn checksum_mismatch_and_unsupported_capability_fail_closed() -> Result<(), Box<dyn Error>> {
    let mismatch_root = fixture_root("t20-runtime-mismatch");
    reset_root(&mismatch_root)?;
    let mismatch_manifest =
        write_manifest_fixture(&mismatch_root, CompatibilityCapability::BevyGltfPbr)?;
    fs::write(mismatch_root.join("scene.glb"), b"modified artifact")?;
    assert!(verify_manifest(&mismatch_manifest).is_err());

    let capability_root = fixture_root("t20-runtime-capability");
    reset_root(&capability_root)?;
    let capability_manifest =
        write_manifest_fixture(&capability_root, CompatibilityCapability::UnrealEditorExport)?;
    assert!(verify_manifest(&capability_manifest).is_err());

    Ok(())
}

#[test]
fn lifecycle_inspection_unloads_and_reloads_a_verified_scene() -> Result<(), Box<dyn Error>> {
    let root = fixture_root("t21-lifecycle");
    reset_root(&root)?;
    let manifest = write_manifest_fixture(&root, CompatibilityCapability::BevyGltfPbr)?;
    let mut app = build_runtime_app(root.to_string_lossy().into_owned());
    let first = load_manifest_and_spawn(&mut app, &manifest)?;
    if let Err(error) = tick_until_world_asset_loaded(&mut app, first) {
        return Err(Box::<dyn Error>::from(error));
    }

    let diagnostic = Diagnostic::new(
        DiagnosticCode::parse("vortex.runtime.unsupported_fixture")?,
        "fixture keeps unsupported source behavior visible",
        Remediation::new("Review the source classification.")?,
    )?;
    let mut first_root =
        app.world_mut().get_entity_mut(first).map_err(|_| "verified root should exist")?;
    first_root.insert((
        VortexDiagnosticSet(vec![diagnostic]),
        VortexConversionStatus(vec![ClassificationKind::Unsupported]),
    ));

    let inspection = inspect_scene(&app, first)?;
    assert_eq!(inspection.generation, 0);
    assert_eq!(inspection.sources_with_classification(&ClassificationKind::Unsupported).len(), 1);
    assert_eq!(inspection.diagnostics_with_code_prefix("vortex.runtime").len(), 1);

    let second = reload_scene(&mut app, first)?;
    assert_ne!(first, second);
    if let Err(error) = tick_until_world_asset_loaded(&mut app, second) {
        return Err(Box::<dyn Error>::from(error));
    }
    let second_inspection = inspect_scene(&app, second)?;
    assert_eq!(second_inspection.generation, 1);
    let world = app.world();
    assert!(world.get_entity(first).is_err());
    let second_root = world.get_entity(second).map_err(|_| "reloaded root should exist")?;
    assert!(second_root.contains::<VortexSceneLifecycle>());

    let foreign = app.world_mut().spawn_empty().id();
    assert!(unload_scene(&mut app, foreign).is_err());
    let unloaded = unload_scene(&mut app, second)?;
    assert_eq!(unloaded.generation(), 1);
    app.update();
    let world = app.world_mut();
    assert!(world.get_entity(second).is_err());
    let mut roots = world.query::<&VortexSceneRoot>();
    assert!(roots.iter(world).next().is_none());

    Ok(())
}

#[test]
fn viewer_state_queues_typed_lifecycle_operations() -> Result<(), Box<dyn Error>> {
    let root = fixture_root("t21-egui-viewer");
    reset_root(&root)?;
    let manifest = write_manifest_fixture(&root, CompatibilityCapability::BevyGltfPbr)?;
    let mut app = build_runtime_app(root.to_string_lossy().into_owned());
    let first = load_manifest_and_spawn(&mut app, &manifest)?;
    if let Err(error) = tick_until_world_asset_loaded(&mut app, first) {
        return Err(Box::<dyn Error>::from(error));
    }

    app.world_mut().resource_mut::<VortexViewerState>().selected_root = Some(first);
    app.update();
    let initial_state = app.world().resource::<VortexViewerState>();
    assert_eq!(initial_state.selected_root, Some(first));
    assert!(initial_state.inspection.is_some());
    assert_eq!(initial_state.inspection.as_ref().map_or(0, |value| value.sources.len()), 1);

    app.world_mut().resource_mut::<ViewerCommandQueue>().push(ViewerCommand::Reload(first));
    app.update();
    let second = app
        .world()
        .resource::<VortexViewerState>()
        .selected_root
        .ok_or("viewer should select the reloaded root")?;
    assert_ne!(first, second);
    if let Err(error) = tick_until_world_asset_loaded(&mut app, second) {
        return Err(Box::<dyn Error>::from(error));
    }

    app.world_mut().resource_mut::<ViewerCommandQueue>().push(ViewerCommand::Unload(second));
    app.update();
    let state = app.world().resource::<VortexViewerState>();
    assert!(state.selected_root.is_none());
    assert!(state.inspection.is_none());
    assert!(app.world().get_entity(second).is_err());

    Ok(())
}

#[test]
fn missing_and_malformed_manifests_never_spawn_assets() -> Result<(), Box<dyn Error>> {
    let root = fixture_root("t20-runtime-invalid");
    reset_root(&root)?;
    let mut app = build_runtime_app(root.to_string_lossy().into_owned());
    assert!(load_manifest_and_spawn(&mut app, &root.join("missing.ron")).is_err());
    let malformed = root.join("malformed.ron");
    fs::write(&malformed, "(not valid")?;
    assert!(load_manifest_and_spawn(&mut app, &malformed).is_err());
    let world = app.world_mut();
    let mut roots = world.query::<&VortexSceneRoot>();
    assert!(roots.iter(world).next().is_none());

    Ok(())
}
