//! Regression guards for the desktop viewer's native window.
//!
//! The `bevy_winit` feature was not enabled on the `bevy` dependency, so
//! `DefaultPlugins` never installed `WinitPlugin`. The viewer therefore had no
//! event loop, `App::run` fell back to Bevy's single-tick runner, and no
//! native window was ever created.
//!
//! The window probe also has to observe windows from inside the schedule:
//! `App::run` moves the world into the event loop and leaves the caller's
//! `App` empty, so a post-run query over that `App` always reports zero.

use std::error::Error;

use bevy::prelude::{Entity, Window};
use vortex_bevy::cli::{CliReport, viewer_window_report};
use vortex_bevy::runtime::{WindowProbe, build_runtime_app};
use vortex_core::domain::ConversionId;

/// The runtime app is headless: it must not install the windowing plugin, so
/// it never needs a desktop session to build.
#[test]
fn runtime_app_does_not_install_the_windowing_plugin() {
    let app = build_runtime_app("artifacts");
    assert!(
        !app.is_plugin_added::<bevy_winit::WinitPlugin>(),
        "the runtime app must stay headless so it builds without a display"
    );
}

/// The window probe observes windows from inside the schedule, because the
/// caller's `App` is empty once `App::run` has returned.
#[test]
fn window_probe_counts_window_entities_from_inside_the_schedule() {
    let probe = WindowProbe::new();
    let mut app = build_runtime_app("artifacts");
    probe.install(&mut app);

    app.update();
    let baseline = probe.observed_window_count();
    assert!(baseline >= 1, "the primary window must be counted, observed {baseline}");

    let window: Entity = app.world_mut().spawn(Window::default()).id();
    app.update();
    assert_eq!(
        probe.observed_window_count(),
        baseline + 1,
        "the probe must observe the spawned window"
    );

    app.world_mut().entity_mut(window).despawn();
    app.update();
    assert_eq!(
        probe.observed_window_count(),
        baseline + 1,
        "the probe keeps the highest count so a closed window still reports as created"
    );
}

/// A viewer that opened a native window reports success, and one that did
/// not reports a typed diagnostic that points at `verify`.
#[test]
fn viewer_report_distinguishes_an_opened_window_from_a_missing_one() -> Result<(), Box<dyn Error>> {
    let conversion_id =
        ConversionId::parse("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef")?;
    let artifact_root = std::path::Path::new("/tmp/vortex-artifacts");

    let opened = viewer_window_report(&conversion_id, artifact_root, 1);
    let CliReport::Convert { reason, remediation, .. } = opened else {
        return Err("a completed viewer session reports a conversion outcome".into());
    };
    assert!(reason.contains("viewer opened in Bevy window"));
    assert!(!remediation.contains("could not be created"));

    let missing = viewer_window_report(&conversion_id, artifact_root, 0);
    let CliReport::Convert { reason, remediation, .. } = missing else {
        return Err("a failed viewer session reports a conversion outcome".into());
    };
    assert!(reason.contains("without creating a native window"));
    assert!(remediation.contains("vortex-bevy verify --conversion-id"));

    Ok(())
}
