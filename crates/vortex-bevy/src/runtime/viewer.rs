//! `bevy_egui` panels for inspecting and controlling loaded Vortex scene roots.

use bevy::prelude::{App, Entity, Plugin, Resource, Result, Update, World};
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use vortex_core::domain::ClassificationKind;

use super::{
    RuntimeSceneInspection, ViewerFrameRequest, ViewerInputGate, ViewerPanelLayout,
    VortexSceneRoot, inspect_scene_in_world, reload_scene_in_world, unload_scene_in_world,
};

/// Data-owned viewer state that can be rendered by a UI without conversion-handler access.
#[derive(Resource, Clone, Debug, Default)]
pub struct VortexViewerState {
    /// Currently selected runtime root, if any.
    pub selected_root: Option<Entity>,
    /// Selected source row within the current inspection snapshot.
    pub selected_source: Option<usize>,
    /// Optional conversion classification filter for source rows.
    pub classification_filter: Option<ClassificationKind>,
    /// Stable diagnostic-code prefix used to filter visible diagnostics.
    pub diagnostic_prefix: String,
    /// Current detached inspection data for the selected root.
    pub inspection: Option<RuntimeSceneInspection>,
    /// Last recoverable lifecycle or inspection failure presented to the operator.
    pub last_error: Option<String>,
}

/// Explicit UI-to-runtime lifecycle requests, applied outside egui rendering.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewerCommand {
    /// Revalidates and replaces the selected manifest-backed root.
    Reload(Entity),
    /// Removes the selected manifest-backed root.
    Unload(Entity),
}

/// Queue of lifecycle requests produced by the viewer UI.
#[derive(Resource, Debug, Default)]
pub struct ViewerCommandQueue(Vec<ViewerCommand>);

impl ViewerCommandQueue {
    /// Queues a lifecycle request for application during the next Bevy update.
    pub fn push(&mut self, command: ViewerCommand) {
        self.0.push(command);
    }

    fn drain(&mut self) -> Vec<ViewerCommand> {
        std::mem::take(&mut self.0)
    }
}

/// Installs the data-driven Vortex inspector overlay and lifecycle command processing.
#[derive(Clone, Copy, Debug, Default)]
pub struct VortexViewerPlugin;

impl Plugin for VortexViewerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<VortexViewerState>();
        app.init_resource::<ViewerCommandQueue>();
        app.add_systems(Update, update_viewer_state);
        app.add_systems(EguiPrimaryContextPass, render_viewer_panels);
    }
}

/// Refreshes the selected root and applies explicit lifecycle commands.
pub fn update_viewer_state(world: &mut World) {
    let mut state = world.remove_resource::<VortexViewerState>().unwrap_or_default();
    let commands = world
        .get_resource_mut::<ViewerCommandQueue>()
        .map_or_else(Vec::new, |mut queue| queue.drain());

    for command in commands {
        match command {
            ViewerCommand::Reload(entity) => match reload_scene_in_world(world, entity) {
                Ok(reloaded) => {
                    state.selected_root = Some(reloaded);
                    state.selected_source = None;
                    state.last_error = None;
                }
                Err(error) => state.last_error = Some(error.to_string()),
            },
            ViewerCommand::Unload(entity) => match unload_scene_in_world(world, entity) {
                Ok(_) => {
                    if state.selected_root == Some(entity) {
                        state.selected_root = None;
                        state.selected_source = None;
                        state.inspection = None;
                    }
                    state.last_error = None;
                }
                Err(error) => state.last_error = Some(error.to_string()),
            },
        }
    }

    synchronize_selection(world, &mut state);
    world.insert_resource(state);
}

fn synchronize_selection(world: &World, state: &mut VortexViewerState) {
    let selected_is_live = state.selected_root.is_some_and(|entity| {
        world.get_entity(entity).is_ok_and(|root| root.contains::<VortexSceneRoot>())
    });
    if !selected_is_live {
        state.selected_root = world
            .iter_entities()
            .filter(bevy::ecs::world::EntityRef::contains::<VortexSceneRoot>)
            .map(|entity| entity.id())
            .min_by_key(|entity| entity.to_bits());
        state.selected_source = None;
    }

    state.inspection = match state.selected_root {
        Some(entity) => match inspect_scene_in_world(world, entity) {
            Ok(inspection) => {
                state.last_error = None;
                Some(inspection)
            }
            Err(error) => {
                state.last_error = Some(error.to_string());
                None
            }
        },
        None => None,
    };
}

fn render_viewer_panels(
    mut contexts: EguiContexts,
    mut state: bevy::prelude::ResMut<VortexViewerState>,
    mut commands: bevy::prelude::ResMut<ViewerCommandQueue>,
    mut frame_request: bevy::prelude::ResMut<ViewerFrameRequest>,
    mut input_gate: bevy::prelude::ResMut<ViewerInputGate>,
    mut layout: bevy::prelude::ResMut<ViewerPanelLayout>,
) -> Result<()> {
    let context = contexts.ctx_mut()?;
    let mut viewport_ui = egui::Ui::new(
        context.clone(),
        "vortex_viewer".into(),
        egui::UiBuilder::new().max_rect(context.viewport_rect()),
    );
    layout.top = render_toolbar(&mut viewport_ui, &state, &mut commands, &mut frame_request);
    layout.left = render_source_tree(&mut viewport_ui, &mut state);
    layout.right = render_inspector(&mut viewport_ui, &state);
    layout.bottom = render_diagnostics(&mut viewport_ui, &state);
    input_gate.pointer_owned_by_ui = context.egui_wants_pointer_input();
    Ok(())
}

fn render_toolbar(
    ui: &mut egui::Ui,
    state: &VortexViewerState,
    commands: &mut ViewerCommandQueue,
    frame_request: &mut ViewerFrameRequest,
) -> f32 {
    egui::Panel::top("vortex_toolbar")
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Vortex Viewer");
                ui.small("Right drag: orbit · Middle drag: pan · Wheel: zoom · F: frame");
                if let Some(root) = state.selected_root {
                    if ui.button("Reload").clicked() {
                        commands.push(ViewerCommand::Reload(root));
                    }
                    if ui.button("Frame").clicked() {
                        frame_request.0 = Some(root);
                    }
                    if ui.button("Unload").clicked() {
                        commands.push(ViewerCommand::Unload(root));
                    }
                } else {
                    ui.label("No verified Vortex scene is loaded.");
                }
                if let Some(error) = &state.last_error {
                    ui.colored_label(egui::Color32::RED, error);
                }
            });
        })
        .response
        .rect
        .height()
}

fn render_source_tree(ui: &mut egui::Ui, state: &mut VortexViewerState) -> f32 {
    egui::Panel::left("vortex_sources")
        .show(ui, |ui| {
            ui.heading("Sources");
            render_classification_filter(ui, &mut state.classification_filter);
            let Some(inspection) = &state.inspection else {
                return;
            };
            for (index, source) in inspection.sources.iter().enumerate() {
                if state
                    .classification_filter
                    .as_ref()
                    .is_some_and(|filter| filter != &source.classification)
                {
                    continue;
                }
                let selected = state.selected_source == Some(index);
                let label = format!("{:?}: {}", source.classification, source.asset_kind);
                if ui.selectable_label(selected, label).clicked() {
                    state.selected_source = Some(index);
                }
            }
        })
        .response
        .rect
        .width()
}

fn render_classification_filter(ui: &mut egui::Ui, filter: &mut Option<ClassificationKind>) {
    egui::ComboBox::from_label("Classification")
        .selected_text(filter.as_ref().map_or("All", |value| match value {
            ClassificationKind::Converted => "Converted",
            ClassificationKind::Baked => "Baked",
            ClassificationKind::RequiresAdapter => "Requires adapter",
            ClassificationKind::Unsupported => "Unsupported",
            ClassificationKind::Rejected => "Rejected",
        }))
        .show_ui(ui, |ui| {
            ui.selectable_value(filter, None, "All");
            for value in [
                ClassificationKind::Converted,
                ClassificationKind::Baked,
                ClassificationKind::RequiresAdapter,
                ClassificationKind::Unsupported,
                ClassificationKind::Rejected,
            ] {
                ui.selectable_value(filter, Some(value.clone()), format!("{value:?}"));
            }
        });
}

fn render_inspector(ui: &mut egui::Ui, state: &VortexViewerState) -> f32 {
    egui::Panel::right("vortex_inspector")
        .show(ui, |ui| {
            ui.heading("Inspector");
            let Some(inspection) = &state.inspection else {
                return;
            };
            ui.label(format!("Generation {}", inspection.generation));
            let Some(index) = state.selected_source else {
                ui.label("Select a source to inspect its provenance.");
                return;
            };
            let Some(source) = inspection.sources.get(index) else {
                ui.label("The selected source is no longer available.");
                return;
            };
            ui.monospace(source.identity.to_string());
            ui.label(format!("Asset kind: {}", source.asset_kind));
            ui.label(format!("Classification: {:?}", source.classification));
            ui.collapsing("Selected variants", |ui| {
                for (key, value) in &source.selected_values {
                    ui.label(format!("{key}: {value}"));
                }
            });
            ui.collapsing("GLB source labels", |ui| {
                for (label, source_path) in &source.glb_labels {
                    ui.label(format!("{label} → {source_path}"));
                }
            });
        })
        .response
        .rect
        .width()
}

fn render_diagnostics(ui: &mut egui::Ui, state: &VortexViewerState) -> f32 {
    egui::Panel::bottom("vortex_diagnostics")
        .show(ui, |ui| {
            ui.heading("Diagnostics");
            let Some(inspection) = &state.inspection else {
                return;
            };
            for diagnostic in inspection.diagnostics_with_code_prefix(&state.diagnostic_prefix) {
                ui.label(format!("{}: {}", diagnostic.code, diagnostic.message));
                ui.small(diagnostic.remediation.as_str());
            }
        })
        .response
        .rect
        .height()
}
