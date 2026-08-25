//! Native Bevy viewport camera controls for the Vortex desktop viewer.

use std::f32::consts::FRAC_PI_2;

use super::viewer::VortexViewerState;

use bevy::{
    camera::{Camera, Viewport},
    ecs::{hierarchy::Children, schedule::IntoScheduleConfigs},
    input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll},
    prelude::{
        App, ButtonInput, Camera3d, Component, EulerRot, GlobalTransform, KeyCode, MouseButton,
        Name, Plugin, Query, Res, ResMut, Resource, Startup, Transform, UVec2, Update, Vec3, With,
    },
    window::{PrimaryWindow, Window},
};

/// Requests one viewer camera framing operation for a verified Vortex scene root.
#[derive(Resource, Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ViewerFrameRequest(pub Option<bevy::prelude::Entity>);

/// Prevents camera input while the immediate-mode interface owns pointer input.
#[derive(Resource, Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ViewerInputGate {
    /// Whether the pointer is currently owned by an `egui` control.
    pub pointer_owned_by_ui: bool,
}

/// Configurable panel extents in logical viewport units.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct ViewerPanelLayout {
    /// Left-side source panel width.
    pub left: f32,
    /// Right-side inspector panel width.
    pub right: f32,
    /// Top toolbar height.
    pub top: f32,
    /// Bottom diagnostics height.
    pub bottom: f32,
}

/// Native Bevy camera state for the Vortex 3D viewport.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct VortexViewerCamera {
    /// Point the camera orbits and frames around.
    pub target: Vec3,
    /// Horizontal orbit angle in radians.
    pub yaw: f32,
    /// Vertical orbit angle in radians.
    pub pitch: f32,
    /// Camera distance from `target`.
    pub radius: f32,
    /// Lower zoom bound.
    pub min_radius: f32,
    /// Upper zoom bound.
    pub max_radius: f32,
}

impl Default for VortexViewerCamera {
    fn default() -> Self {
        Self {
            target: Vec3::ZERO,
            yaw: 0.75,
            pitch: -0.35,
            radius: 8.0,
            min_radius: 0.2,
            max_radius: 10_000.0,
        }
    }
}

/// Installs the desktop viewport camera and its input/frame systems.
#[derive(Clone, Copy, Debug, Default)]
pub struct VortexDesktopViewerPlugin;

impl Plugin for VortexDesktopViewerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ViewerFrameRequest>();
        app.init_resource::<ViewerInputGate>();
        app.init_resource::<ViewerPanelLayout>();
        app.add_systems(Startup, spawn_viewer_camera);
        app.add_systems(
            Update,
            (apply_orbit_input, apply_frame_request, apply_viewport_clipping).chain(),
        );
    }
}

/// Applies an orbit-camera state to a Bevy transform.
pub fn apply_camera_pose(camera: VortexViewerCamera, transform: &mut Transform) {
    transform.rotation = Quat::from_euler(EulerRot::YXZ, camera.yaw, camera.pitch, 0.0);
    transform.translation = camera.target - transform.forward() * camera.radius;
}

/// Updates a camera state from accumulated desktop mouse input.
pub fn apply_input_delta(
    camera: &mut VortexViewerCamera,
    motion: bevy::prelude::Vec2,
    scroll: bevy::prelude::Vec2,
    orbiting: bool,
    panning: bool,
) {
    if orbiting {
        camera.yaw = motion.x.mul_add(-0.006, camera.yaw);
        camera.pitch =
            motion.y.mul_add(-0.006, camera.pitch).clamp(-FRAC_PI_2 + 0.01, FRAC_PI_2 - 0.01);
    }
    if panning {
        let orientation = Quat::from_euler(EulerRot::YXZ, camera.yaw, camera.pitch, 0.0);
        camera.target +=
            orientation * Vec3::new(-motion.x, motion.y, 0.0) * (camera.radius * 0.002);
    }
    if scroll.y != 0.0 {
        camera.radius =
            (camera.radius * (-scroll.y * 0.1).exp()).clamp(camera.min_radius, camera.max_radius);
    }
}

fn spawn_viewer_camera(mut commands: bevy::prelude::Commands) {
    let camera = VortexViewerCamera::default();
    let mut transform = Transform::default();
    apply_camera_pose(camera, &mut transform);
    commands.spawn((
        Name::new("Vortex Viewer Camera"),
        Camera3d::default(),
        Camera::default(),
        camera,
        transform,
    ));
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "Bevy ECS system parameters must be owned wrappers so the scheduler can inject resources."
)]
#[expect(
    clippy::too_many_arguments,
    reason = "The controller consumes distinct Bevy resources and queries rather than a hidden aggregate resource."
)]
fn apply_orbit_input(
    gate: Res<ViewerInputGate>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    viewer_state: Res<VortexViewerState>,
    mut frame_request: ResMut<ViewerFrameRequest>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    mut cameras: Query<(&mut VortexViewerCamera, &mut Transform)>,
) {
    if gate.pointer_owned_by_ui {
        return;
    }
    let orbiting = mouse_buttons.pressed(MouseButton::Right);
    let panning = mouse_buttons.pressed(MouseButton::Middle);
    let camera_count = cameras.iter().count();
    if camera_count != 1 {
        return;
    }
    let Some((mut camera, mut transform)) = cameras.iter_mut().next() else {
        return;
    };
    apply_input_delta(&mut camera, motion.delta, scroll.delta, orbiting, panning);
    if keys.just_pressed(KeyCode::KeyF) {
        frame_request.0 = viewer_state.selected_root;
    }
    apply_camera_pose(*camera, &mut transform);
}

fn apply_frame_request(
    mut request: ResMut<ViewerFrameRequest>,
    roots: Query<&GlobalTransform>,
    children: Query<&Children>,
    mut cameras: Query<(&mut VortexViewerCamera, &mut Transform)>,
) {
    let Some(root) = request.0.take() else {
        return;
    };
    let camera_count = cameras.iter().count();
    if camera_count != 1 {
        return;
    }
    let Some((mut camera, mut transform)) = cameras.iter_mut().next() else {
        return;
    };
    let points = collect_scene_points(root, &roots, &children);
    frame_points(&mut camera, &points);
    apply_camera_pose(*camera, &mut transform);
}

fn collect_scene_points(
    root: bevy::prelude::Entity,
    transforms: &Query<&GlobalTransform>,
    children: &Query<&Children>,
) -> Vec<Vec3> {
    let mut points = Vec::new();
    let mut pending = vec![root];
    while let Some(entity) = pending.pop() {
        if let Ok(transform) = transforms.get(entity) {
            points.push(transform.translation());
        }
        if let Ok(entity_children) = children.get(entity) {
            pending.extend(entity_children.iter());
        }
    }
    points
}

/// Calculates the physical center viewport that remains after inspector panels consume logical space.
#[must_use]
pub fn calculate_viewport(
    physical_window_size: UVec2,
    scale_factor: f32,
    layout: ViewerPanelLayout,
) -> Option<Viewport> {
    if physical_window_size.x == 0 || physical_window_size.y == 0 || !scale_factor.is_finite() {
        return None;
    }
    let scale = scale_factor.max(1.0);
    let left = logical_extent_to_pixels(layout.left, scale, physical_window_size.x);
    let right = logical_extent_to_pixels(layout.right, scale, physical_window_size.x);
    let top = logical_extent_to_pixels(layout.top, scale, physical_window_size.y);
    let bottom = logical_extent_to_pixels(layout.bottom, scale, physical_window_size.y);
    let available_width = physical_window_size.x.saturating_sub(left.saturating_add(right));
    let available_height = physical_window_size.y.saturating_sub(top.saturating_add(bottom));
    if available_width == 0 || available_height == 0 {
        return None;
    }
    Some(Viewport {
        physical_position: UVec2::new(
            left.min(physical_window_size.x),
            top.min(physical_window_size.y),
        ),
        physical_size: UVec2::new(available_width, available_height),
        ..Default::default()
    })
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "Bevy ECS system parameters must be owned wrappers so the scheduler can inject resources."
)]
fn apply_viewport_clipping(
    layout: Res<ViewerPanelLayout>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut cameras: Query<&mut Camera, With<VortexViewerCamera>>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let viewport = calculate_viewport(window.physical_size(), window.scale_factor(), *layout);
    for mut camera in &mut cameras {
        apply_viewport_state(&mut camera, &viewport);
    }
}

/// Applies a clipped viewport and disables rendering when no unobscured viewport remains.
pub fn apply_viewport_state(camera: &mut Camera, viewport: &Option<Viewport>) {
    camera.is_active = viewport.is_some();
    camera.viewport.clone_from(viewport);
}

/// Converts a finite non-negative logical extent into a physical pixel count bounded by a window axis.
#[expect(
    clippy::cast_possible_truncation,
    reason = "The finite value is rounded and clamped to the target u32 window axis before conversion."
)]
#[expect(
    clippy::cast_precision_loss,
    reason = "The target u32 window axis is compared in f32 logical-layout arithmetic before final clamping."
)]
#[expect(
    clippy::cast_sign_loss,
    reason = "The finite value is clamped non-negative before conversion."
)]
fn logical_extent_to_pixels(logical: f32, scale: f32, maximum: u32) -> u32 {
    let bounded = (logical.max(0.0) * scale).round().min(maximum as f32);
    bounded as u32
}

/// Converts a scene point count to the finite scalar needed by the pure camera framing operation.
#[expect(
    clippy::cast_precision_loss,
    reason = "Bevy Vec3 arithmetic uses f32; scene point count only scales an already materialized in-memory collection."
)]
const fn point_count_as_f32(count: usize) -> f32 {
    count as f32
}

/// Frames points conservatively while retaining configured zoom limits.
pub fn frame_points(camera: &mut VortexViewerCamera, points: &[Vec3]) {
    if points.is_empty() {
        return;
    }
    let sum = points.iter().copied().sum::<Vec3>();
    camera.target = sum / point_count_as_f32(points.len());
    let radius = points.iter().map(|point| point.distance(camera.target)).fold(1.5_f32, f32::max);
    camera.radius = (radius * 2.5).clamp(camera.min_radius, camera.max_radius);
}

use bevy::prelude::Quat;
