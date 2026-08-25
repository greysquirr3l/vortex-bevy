use std::collections::{BTreeMap, BTreeSet};

use ron::{de::SpannedError, ser::PrettyConfig};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{
    ArtifactHash, ClassificationKind, ConversionClassification, Diagnostic, SourceIdentity,
};

pub const CURRENT_SCHEMA_VERSION: u16 = 1;
pub const DEFAULT_MANIFEST_LIMIT_BYTES: usize = 1_048_576;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct SchemaVersion(u16);

impl SchemaVersion {
    #[must_use]
    pub const fn current() -> Self {
        Self(CURRENT_SCHEMA_VERSION)
    }

    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ManifestArtifactKind {
    Glb,
    Texture,
    Report,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ManifestArtifact {
    pub kind: ManifestArtifactKind,
    pub path: String,
    pub hash: ArtifactHash,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ManifestSource {
    pub identity: SourceIdentity,
    pub asset_kind: String,
    pub selected_values: BTreeMap<String, String>,
    pub glb_labels: BTreeMap<String, String>,
    pub classification: ConversionClassification,
    pub diagnostics: Vec<Diagnostic>,
}

impl ManifestSource {
    #[must_use]
    pub fn identity_key(&self) -> String {
        self.identity.to_string()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CompatibilityCapability {
    BevyGltfPbr,
    BevyPunctualLights,
    BevyAnimations,
    UrpLitSubset,
    UsdPreviewSurface,
    UnrealEditorExport,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CompatibilityRequirements {
    pub minimum_bevy_version: String,
    pub required_capabilities: BTreeSet<CompatibilityCapability>,
}

#[derive(Debug, Error)]
pub enum ManifestError {
    #[error("manifest is {actual} bytes, exceeding the {limit}-byte limit")]
    TooLarge { actual: usize, limit: usize },
    #[error("manifest RON is invalid: {source}")]
    InvalidRon { source: SpannedError },
    #[error("manifest schema version {actual} is not supported; expected {expected}")]
    UnsupportedSchema { actual: u16, expected: u16 },
    #[error("manifest source identity is duplicated: {identity}")]
    DuplicateSource { identity: String },
    #[error("manifest artifact path is unsafe: {path}")]
    UnsafeArtifactPath { path: String },
    #[error("manifest source asset kind cannot be empty")]
    EmptyAssetKind,
    #[error("manifest compatibility version cannot be empty")]
    EmptyBevyVersion,
    #[error("manifest source diagnostic classification must match its parent source")]
    ClassificationMismatch,
    #[error("manifest serialization failed: {source}")]
    Serialization { source: ron::Error },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SceneManifest {
    pub schema_version: SchemaVersion,
    pub converter_version: String,
    pub profile_hash: ArtifactHash,
    pub artifacts: Vec<ManifestArtifact>,
    pub sources: Vec<ManifestSource>,
    pub compatibility: CompatibilityRequirements,
}

impl SceneManifest {
    /// Creates and canonicalizes a version-one scene manifest.
    ///
    /// # Errors
    ///
    /// Returns [`ManifestError`] if an invariant is violated.
    pub fn new(
        converter_version: impl Into<String>,
        profile_hash: ArtifactHash,
        artifacts: Vec<ManifestArtifact>,
        sources: Vec<ManifestSource>,
        compatibility: CompatibilityRequirements,
    ) -> Result<Self, ManifestError> {
        let converter_version = converter_version.into();
        let manifest = Self {
            schema_version: SchemaVersion::current(),
            converter_version,
            profile_hash,
            artifacts,
            sources,
            compatibility,
        }
        .canonicalize();
        manifest.validate()?;
        Ok(manifest)
    }

    /// Parses a bounded RON manifest and normalizes deterministic ordering.
    ///
    /// # Errors
    ///
    /// Returns [`ManifestError::TooLarge`] when input exceeds `limit_bytes`.
    /// Returns [`ManifestError::InvalidRon`] when RON cannot be parsed.
    /// Returns another [`ManifestError`] when manifest invariants are violated.
    pub fn from_ron(input: &[u8], limit_bytes: usize) -> Result<Self, ManifestError> {
        if input.len() > limit_bytes {
            return Err(ManifestError::TooLarge { actual: input.len(), limit: limit_bytes });
        }
        let manifest: Self =
            ron::de::from_bytes(input).map_err(|source| ManifestError::InvalidRon { source })?;
        let manifest = manifest.canonicalize();
        manifest.validate()?;
        Ok(manifest)
    }

    /// Serializes canonical manifest RON without time-dependent data.
    ///
    /// # Errors
    ///
    /// Returns [`ManifestError::Serialization`] when RON serialization fails.
    pub fn to_ron(&self) -> Result<String, ManifestError> {
        ron::ser::to_string_pretty(self, PrettyConfig::new())
            .map_err(|source| ManifestError::Serialization { source })
    }

    #[must_use]
    pub fn canonicalize(mut self) -> Self {
        self.artifacts.sort_by(|left, right| {
            left.kind.cmp(&right.kind).then_with(|| left.path.cmp(&right.path))
        });
        self.sources.sort_by_key(ManifestSource::identity_key);
        self
    }

    fn validate(&self) -> Result<(), ManifestError> {
        if self.schema_version != SchemaVersion::current() {
            return Err(ManifestError::UnsupportedSchema {
                actual: self.schema_version.get(),
                expected: CURRENT_SCHEMA_VERSION,
            });
        }
        if self.converter_version.trim().is_empty() {
            return Err(ManifestError::EmptyBevyVersion);
        }
        if self.compatibility.minimum_bevy_version.trim().is_empty() {
            return Err(ManifestError::EmptyBevyVersion);
        }
        for artifact in &self.artifacts {
            if !is_safe_relative_path(&artifact.path) {
                return Err(ManifestError::UnsafeArtifactPath { path: artifact.path.clone() });
            }
        }
        for pair in self.sources.windows(2) {
            let [current, next] = pair else {
                continue;
            };
            if current.identity == next.identity {
                return Err(ManifestError::DuplicateSource { identity: current.identity_key() });
            }
        }
        for source in &self.sources {
            if source.asset_kind.trim().is_empty() {
                return Err(ManifestError::EmptyAssetKind);
            }
            if source.classification.kind == ClassificationKind::Converted
                && source
                    .diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.code.as_str().starts_with("source.rejected"))
            {
                return Err(ManifestError::ClassificationMismatch);
            }
        }
        Ok(())
    }
}

fn is_safe_relative_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.starts_with('\\')
        && !path.contains('\0')
        && path
            .split(['/', '\\'])
            .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
}
