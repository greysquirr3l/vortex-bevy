//! Application use cases and their consumer-owned dependencies.
//!
//! Each handler defines only the narrow trait surface it consumes. Secondary
//! adapters implement those traits without moving them into a shared port layer.

pub mod command;
pub mod query;
