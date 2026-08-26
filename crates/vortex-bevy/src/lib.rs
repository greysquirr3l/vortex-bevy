//! Bevy runtime composition for Vortex-Bevy.
//!
//! User-facing and runtime adapters live in this crate while conversion-domain
//! behavior remains in the Bevy-free `vortex-core` crate.

#![forbid(unsafe_code)]

use bevy::prelude::{App, MinimalPlugins};

pub mod cli;
pub mod runtime;

/// Builds the minimal Bevy application used by command-oriented workflows.
///
/// # Examples
///
/// ```no_run
/// let app = vortex_bevy::build_app();
/// drop(app);
/// ```
pub fn build_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app
}
