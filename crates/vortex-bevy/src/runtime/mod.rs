//! Bevy 0.19 runtime loading for validated Vortex conversion artifacts.

mod components;
mod desktop;
mod inspection;
mod lifecycle;
mod manifest;
mod viewer;

use std::{collections::BTreeMap, path::Path};

use anyhow::Result;
use bevy::{
    asset::{AssetPlugin, AssetServer},
    camera::visibility::{InheritedVisibility, ViewVisibility},
    ecs::hierarchy::{ChildOf, Children},
    gltf::GltfAssetLabel,
    prelude::{
        App, DefaultPlugins, Entity, GlobalTransform, Name, PluginGroup, Transform, Visibility,
        World, WorldAssetRoot,
    },
    transform::components::TransformTreeChanged,
    window::{Window, WindowPlugin, WindowResolution},
};
use bevy_egui::EguiPlugin;
use vortex_core::domain::ManifestSource;

pub use components::{
    VortexArtifactHash, VortexConversionStatus, VortexDiagnosticSet, VortexRuntimeDiagnostic,
    VortexSceneRoot, VortexSourceLink, VortexSourceRecord, VortexVariantSelection,
};
pub use desktop::{
    ViewerFrameRequest, ViewerInputGate, ViewerPanelLayout, VortexDesktopViewerPlugin,
    VortexViewerCamera, apply_camera_pose, apply_input_delta, apply_viewport_state,
    calculate_viewport, frame_points,
};
pub use inspection::{
    RuntimeSceneInspection, RuntimeSourceInspection, inspect_scene, inspect_scene_in_world,
};
pub use lifecycle::{
    VortexSceneLifecycle, reload_scene, reload_scene_in_world, unload_scene, unload_scene_in_world,
};
pub use manifest::{VerifiedVortexArtifact, verify_manifest};
pub use viewer::{
    ViewerCommand, ViewerCommandQueue, VortexViewerPlugin, VortexViewerState, update_viewer_state,
};

/// Builds a headless-capable Bevy 0.19 application configured to load assets from one directory.
///
/// # Examples
///
/// ```no_run
/// let app = vortex_bevy::runtime::build_runtime_app("artifacts");
/// drop(app);
/// ```
pub fn build_runtime_app(asset_root: impl Into<String>) -> App {
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins.set(AssetPlugin { file_path: asset_root.into(), ..Default::default() }).set(
            WindowPlugin {
                primary_window: Some(Window {
                    title: "Vortex-Bevy Viewer".to_owned(),
                    resolution: WindowResolution::new(1_440, 900),
                    resizable: true,
                    ..Default::default()
                }),
                ..Default::default()
            },
        ),
    );
    app.register_type::<Transform>();
    app.register_type::<GlobalTransform>();
    app.register_type::<TransformTreeChanged>();
    app.register_type::<Visibility>();
    app.register_type::<InheritedVisibility>();
    app.register_type::<ViewVisibility>();
    app.register_type::<ChildOf>();
    app.register_type::<Children>();
    app.register_type::<Name>();
    app.add_plugins(EguiPlugin::default());
    app.add_plugins(VortexDesktopViewerPlugin);
    app.add_plugins(VortexViewerPlugin);
    app.finish();
    app.cleanup();
    app
}

/// Validates a Vortex manifest and spawns a Bevy world asset root with provenance components.
///
/// # Errors
///
/// Returns an error when manifest or checksum validation fails, when the supplied runtime lacks an
/// asset server, or when the declared GLB cannot be submitted to the asset pipeline.
pub fn load_manifest_and_spawn(app: &mut App, manifest_path: &Path) -> Result<Entity> {
    load_manifest_and_spawn_in_world(app.world_mut(), manifest_path)
}

/// Validates a Vortex manifest and spawns a Bevy world asset root in an existing world.
///
/// # Errors
///
/// Returns an error when manifest or checksum validation fails, when the world lacks an asset
/// server, or when the declared GLB cannot be submitted to the asset pipeline.
pub fn load_manifest_and_spawn_in_world(world: &mut World, manifest_path: &Path) -> Result<Entity> {
    let verified = verify_manifest(manifest_path)?;
    let root = {
        let asset_server = world
            .get_resource::<AssetServer>()
            .ok_or_else(|| anyhow::anyhow!("Bevy runtime world has no AssetServer"))?;
        asset_server.load(GltfAssetLabel::Scene(0).from_asset(verified.glb_asset_path.clone()))
    };
    let sources = verified.manifest.sources.iter().map(source_record).collect::<Vec<_>>();
    let statuses = verified
        .manifest
        .sources
        .iter()
        .map(|source| source.classification.kind.clone())
        .collect::<Vec<_>>();
    let diagnostics = verified
        .manifest
        .sources
        .iter()
        .flat_map(|source| source.diagnostics.iter().cloned())
        .collect::<Vec<_>>();
    let variants = verified
        .manifest
        .sources
        .iter()
        .map(|source| (source.identity_key(), source.selected_values.clone()))
        .collect::<BTreeMap<_, _>>();

    Ok(world
        .spawn((
            WorldAssetRoot(root),
            VortexSceneRoot,
            VortexSourceLink(sources),
            VortexConversionStatus(statuses),
            VortexArtifactHash(verified.glb_hash),
            VortexDiagnosticSet(diagnostics),
            VortexVariantSelection(variants),
            VortexSceneLifecycle::initial(manifest_path.to_path_buf()),
        ))
        .id())
}

/// Spawns a visible runtime diagnostic instead of a scene for a failed artifact request.
///
/// # Examples
///
/// ```no_run
/// let mut app = vortex_bevy::runtime::build_runtime_app("artifacts");
/// let _entity = vortex_bevy::runtime::spawn_runtime_diagnostic(
///     &mut app,
///     "vortex.runtime_invalid",
///     "fixture request rejected",
/// );
/// ```
#[must_use]
pub fn spawn_runtime_diagnostic(
    app: &mut App,
    code: impl Into<String>,
    message: impl Into<String>,
) -> Entity {
    app.world_mut()
        .spawn(VortexRuntimeDiagnostic { code: code.into(), message: message.into() })
        .id()
}

fn source_record(source: &ManifestSource) -> VortexSourceRecord {
    VortexSourceRecord {
        identity: source.identity.clone(),
        asset_kind: source.asset_kind.clone(),
        glb_labels: source.glb_labels.clone(),
    }
}
