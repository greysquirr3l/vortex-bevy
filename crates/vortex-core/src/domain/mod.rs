//! Pure conversion types, invariants, and deterministic state.
//!
//! This module deliberately remains independent of filesystem, subprocess, SDK,
//! and runtime framework concerns.

pub mod conversion;
pub mod identity;
pub mod manifest;
pub mod scene_ir;

pub use conversion::{
    ArtifactKind, ArtifactRecord, ClassificationKind, ConversionAggregate,
    ConversionClassification, ConversionContext, ConversionState, Diagnostic, DiagnosticCode,
    DomainError, Remediation,
};
pub use identity::{
    ArtifactHash, ConversionId, ExportProfileName, OutputRoot, ProjectRoot, SourceAssetPath,
    SourceIdentity, StageSelection, UnityAssetGuid, UnityObjectId, UnrealObjectPath, UsdPrimPath,
};
pub use manifest::{
    CompatibilityCapability, CompatibilityRequirements, DEFAULT_MANIFEST_LIMIT_BYTES,
    ManifestArtifact, ManifestArtifactKind, ManifestError, ManifestSource, SceneManifest,
    SchemaVersion,
};
pub use scene_ir::{CanonicalScene, CoordinateNormalization, SceneIrError};
