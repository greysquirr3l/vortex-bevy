//! Pure conversion-domain contracts for Vortex-Bevy.
//!
//! This crate deliberately excludes Bevy, filesystem, subprocess, and source-SDK
//! dependencies. Primary and secondary adapters are added only when a concrete
//! application handler requires them.

#![forbid(unsafe_code)]

pub mod adapters;
pub mod application;
pub mod domain;
