//! Bevy-facing provenance and diagnostic components for validated Vortex artifacts.

use std::collections::BTreeMap;

use bevy::prelude::Component;
use vortex_core::domain::{
    ArtifactHash, ClassificationKind, Diagnostic, PackagedScene, PackagedSceneId, SourceIdentity,
};

/// Source linkage retained on a spawned Vortex scene root.
#[derive(Clone, Debug)]
pub struct VortexSourceRecord {
    /// Typed source identity from the manifest.
    pub identity: SourceIdentity,
    /// Source-side asset class or kind.
    pub asset_kind: String,
    /// Stable GLB labels mapped back to source locators.
    pub glb_labels: BTreeMap<String, String>,
}

/// Source provenance attached to a Vortex scene root.
#[derive(Component, Clone, Debug)]
pub struct VortexSourceLink(pub Vec<VortexSourceRecord>);

/// Conversion classifications represented by a spawned Vortex scene root.
#[derive(Component, Clone, Debug)]
pub struct VortexConversionStatus(pub Vec<ClassificationKind>);

/// Declared and verified artifact hash attached to a Vortex scene root.
#[derive(Component, Clone, Debug)]
pub struct VortexArtifactHash(pub ArtifactHash);

/// All manifest diagnostics retained for visible runtime reporting.
#[derive(Component, Clone, Debug)]
pub struct VortexDiagnosticSet(pub Vec<Diagnostic>);

/// Selected source variants retained for runtime inspection.
#[derive(Component, Clone, Debug)]
pub struct VortexVariantSelection(pub BTreeMap<String, BTreeMap<String, String>>);

/// Explicit prebuilt scene alternatives and chunks retained on a validated scene root.
#[derive(Component, Clone, Debug)]
pub struct VortexPackagedScenes(pub Vec<PackagedScene>);

/// Provenance retained on a scene instance selected from a packaged variant alternative.
#[derive(Component, Clone, Debug, Eq, PartialEq)]
pub struct VortexPackagedVariant {
    /// Parent root whose selection owns this package instance.
    pub parent: bevy::prelude::Entity,
    /// Stable package identity.
    pub id: PackagedSceneId,
}

/// Provenance retained on a separately loaded scene chunk.
#[derive(Component, Clone, Debug, Eq, PartialEq)]
pub struct VortexSceneChunk {
    /// Parent root that owns this independently loadable chunk.
    pub parent: bevy::prelude::Entity,
    /// Stable package identity for the chunk.
    pub id: PackagedSceneId,
}

/// Marker for a scene root created from a validated Vortex manifest.
#[derive(Component, Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct VortexSceneRoot;

/// A visible runtime failure record created instead of spawning an invalid scene.
#[derive(Component, Clone, Debug)]
pub struct VortexRuntimeDiagnostic {
    /// Stable diagnostic code owned by the runtime adapter.
    pub code: String,
    /// Human-readable failure summary.
    pub message: String,
}
