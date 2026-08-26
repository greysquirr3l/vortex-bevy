//! Vortex-Bevy application entrypoint.

#![forbid(unsafe_code)]

use std::process::ExitCode;

fn main() -> ExitCode {
    vortex_bevy::cli::run_from_environment()
}
