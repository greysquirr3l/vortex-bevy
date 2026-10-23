//! Observation of Bevy window creation from outside the running app.
//!
//! `App::run` moves the world into the event loop and leaves the caller's
//! `App` empty, so a post-run query over the caller's world always reports
//! zero windows. This probe counts `Window` entities from inside the
//! schedule instead, so the CLI can report accurately whether a native
//! window was created.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use bevy::prelude::{App, Query, Update, With};
use bevy::window::Window;

/// Shared counter recording the highest number of live `Window` entities
/// observed while the app was running.
///
/// The counter is monotonic: it keeps the maximum seen count so that a
/// window which is created and later closed still reports as created.
#[derive(Clone, Debug, Default)]
pub struct WindowProbe(Arc<AtomicUsize>);

impl WindowProbe {
    /// Creates a probe that has observed nothing yet.
    #[must_use]
    pub fn new() -> Self {
        Self(Arc::new(AtomicUsize::new(0)))
    }

    /// Installs the observing system into an app.
    pub fn install(&self, app: &mut App) {
        let counter = Arc::clone(&self.0);
        app.add_systems(Update, move |windows: Query<(), With<Window>>| {
            counter.fetch_max(windows.iter().count(), Ordering::Relaxed);
        });
    }

    /// Returns the highest number of `Window` entities observed so far.
    pub fn observed_window_count(&self) -> usize {
        self.0.load(Ordering::Relaxed)
    }
}
