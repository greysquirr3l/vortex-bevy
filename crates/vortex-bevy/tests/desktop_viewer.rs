use std::error::Error;

use bevy::camera::Camera;
use bevy::prelude::{Transform, UVec2, Vec2, Vec3};
use vortex_bevy::runtime::{
    ViewerPanelLayout, VortexViewerCamera, apply_camera_pose, apply_input_delta,
    apply_viewport_state, build_runtime_app, calculate_viewport, frame_points,
};

const EPSILON: f32 = 0.000_1;

#[test]
fn runtime_installs_one_native_vortex_viewer_camera() {
    let mut app = build_runtime_app("target/desktop-viewer-test");
    app.update();
    let world = app.world_mut();
    let mut cameras = world.query::<&VortexViewerCamera>();
    assert_eq!(cameras.iter(world).count(), 1);
}

#[test]
fn orbit_pan_and_zoom_are_bounded_and_update_pose() {
    let mut camera = VortexViewerCamera::default();
    let original = camera;
    apply_input_delta(&mut camera, Vec2::new(64.0, -32.0), Vec2::new(0.0, 4.0), true, true);

    assert!((camera.yaw - original.yaw).abs() > EPSILON);
    assert!((camera.pitch - original.pitch).abs() > EPSILON);
    assert_ne!(camera.target, original.target);
    assert!(camera.radius >= camera.min_radius);
    assert!(camera.radius <= camera.max_radius);

    let mut transform = Transform::default();
    apply_camera_pose(camera, &mut transform);
    let towards_target = (camera.target - transform.translation).normalize();
    assert!(transform.forward().dot(towards_target) > 1.0 - EPSILON);
}

#[test]
fn framing_centers_points_and_expands_to_contain_them() {
    let mut camera = VortexViewerCamera::default();
    let points = [Vec3::new(-4.0, 0.0, 0.0), Vec3::new(4.0, 2.0, 0.0)];
    frame_points(&mut camera, &points);

    assert_eq!(camera.target, Vec3::new(0.0, 1.0, 0.0));
    assert!(camera.radius >= 10.0 - EPSILON);
}

#[test]
fn viewport_clipping_accounts_for_logical_panel_extents_and_scale_factor()
-> Result<(), Box<dyn Error>> {
    let layout = ViewerPanelLayout { left: 200.0, right: 300.0, top: 50.0, bottom: 100.0 };
    let viewport = calculate_viewport(UVec2::new(1_440, 900), 1.5, layout)
        .ok_or_else(|| std::io::Error::other("the center viewport should remain visible"))?;
    assert_eq!(viewport.physical_position, UVec2::new(300, 75));
    assert_eq!(viewport.physical_size, UVec2::new(690, 675));
    Ok(())
}

#[test]
fn viewport_clipping_disables_the_camera_when_panels_exhaust_the_window() {
    let layout = ViewerPanelLayout { left: 800.0, right: 800.0, top: 0.0, bottom: 0.0 };
    let viewport = calculate_viewport(UVec2::new(1_000, 700), 1.0, layout);
    let mut camera = Camera::default();

    apply_viewport_state(&mut camera, &viewport);

    assert!(viewport.is_none());
    assert!(!camera.is_active);
    assert!(camera.viewport.is_none());
}
