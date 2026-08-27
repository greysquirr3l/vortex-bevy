use std::collections::{BTreeMap, BTreeSet};

use ron::{de::SpannedError, ser::PrettyConfig};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{
    ArtifactHash, ClassificationKind, ConversionClassification, Diagnostic, PackagedScene,
    PackagedSceneError, PackagedSceneId, PackagedSceneMode, SourceIdentity,
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
    UsdFlattenedStage,
    UsdPackagedVariants,
    UsdExpandedInstances,
    UsdInstanceBatches,
    UsdSceneChunks,
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
    #[error("manifest packaged scene declaration is invalid: {source}")]
    InvalidPackagedScene { source: PackagedSceneError },
    #[error("manifest packaged scene identifier is duplicated: {id}")]
    DuplicatePackagedScene { id: PackagedSceneId },
    #[error("manifest packaged scene {id} has an unsafe artifact path: {path}")]
    UnsafePackagedScenePath { id: PackagedSceneId, path: String },
    #[error("manifest packaged scene {id} does not declare a GLB artifact: {path}")]
    MissingPackagedSceneArtifact { id: PackagedSceneId, path: String },
    #[error("manifest packaged scenes cannot share an artifact path: {path}")]
    DuplicatePackagedSceneArtifact { path: String },
    #[error(
        "manifest packaged scene {id} source link is absent from manifest sources: {source_identity}"
    )]
    MissingPackagedSceneSource { id: PackagedSceneId, source_identity: String },
    #[error("manifest packaged scene {id} has no declared parent: {parent}")]
    MissingPackagedSceneParent { id: PackagedSceneId, parent: PackagedSceneId },
    #[error("manifest packaged scene {id} must directly descend from a flattened package")]
    InvalidPackagedSceneParent { id: PackagedSceneId },
    #[error("manifest packaged scene declarations require exactly one flattened fallback")]
    InvalidFlattenedPackagedSceneCount,
    #[error("manifest packaged scene {id} requires compatibility capability {capability:?}")]
    MissingPackagedSceneCapability { id: PackagedSceneId, capability: CompatibilityCapability },
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
    /// Explicit prebuilt alternatives and chunks. Empty for legacy single-scene manifests.
    #[serde(default)]
    pub packaged_scenes: Vec<PackagedScene>,
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
            packaged_scenes: Vec::new(),
            compatibility,
        }
        .canonicalize();
        manifest.validate()?;
        Ok(manifest)
    }

    /// Adds explicit runtime packages and validates their parent/artifact relationships.
    ///
    /// # Errors
    ///
    /// Returns [`ManifestError`] when package identities, paths, parents, or GLB artifacts are
    /// invalid.
    pub fn with_packaged_scenes(
        mut self,
        packaged_scenes: Vec<PackagedScene>,
    ) -> Result<Self, ManifestError> {
        self.packaged_scenes = packaged_scenes;
        self = self.canonicalize();
        self.validate()?;
        Ok(self)
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
        self.packaged_scenes.sort_by_key(|package| package.id.clone());
        for package in &mut self.packaged_scenes {
            package.source_links.sort_by_key(ToString::to_string);
        }
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
        self.validate_packaged_scenes()?;
        Ok(())
    }

    fn validate_packaged_scenes(&self) -> Result<(), ManifestError> {
        if self.packaged_scenes.is_empty() {
            return Ok(());
        }
        let flattened = self
            .packaged_scenes
            .iter()
            .filter(|package| package.mode == PackagedSceneMode::Flattened)
            .count();
        if flattened != 1 {
            return Err(ManifestError::InvalidFlattenedPackagedSceneCount);
        }
        let packages = self
            .packaged_scenes
            .iter()
            .map(|package| (package.id.clone(), package))
            .collect::<BTreeMap<_, _>>();
        let paths = self
            .packaged_scenes
            .iter()
            .map(|package| package.artifact_path.as_str())
            .collect::<BTreeSet<_>>();
        if paths.len() != self.packaged_scenes.len() {
            let duplicate = self
                .packaged_scenes
                .windows(2)
                .filter_map(|pair| pair.first().zip(pair.get(1)))
                .find_map(|(left, right)| {
                    (left.artifact_path == right.artifact_path).then(|| left.artifact_path.clone())
                })
                .ok_or(ManifestError::InvalidFlattenedPackagedSceneCount)?;
            return Err(ManifestError::DuplicatePackagedSceneArtifact { path: duplicate });
        }
        if packages.len() != self.packaged_scenes.len() {
            let duplicate = self
                .packaged_scenes
                .windows(2)
                .filter_map(|pair| pair.first().zip(pair.get(1)))
                .find_map(|(left, right)| (left.id == right.id).then(|| left.id.clone()))
                .ok_or(ManifestError::InvalidFlattenedPackagedSceneCount)?;
            return Err(ManifestError::DuplicatePackagedScene { id: duplicate });
        }
        for package in &self.packaged_scenes {
            package.validate().map_err(|source| ManifestError::InvalidPackagedScene { source })?;
            if !self.compatibility.required_capabilities.contains(&package_capability(package.mode))
            {
                return Err(ManifestError::MissingPackagedSceneCapability {
                    id: package.id.clone(),
                    capability: package_capability(package.mode),
                });
            }
            for source in &package.source_links {
                if !self.sources.iter().any(|manifest_source| manifest_source.identity == *source) {
                    return Err(ManifestError::MissingPackagedSceneSource {
                        id: package.id.clone(),
                        source_identity: source.to_string(),
                    });
                }
            }
            if !is_safe_relative_path(&package.artifact_path) {
                return Err(ManifestError::UnsafePackagedScenePath {
                    id: package.id.clone(),
                    path: package.artifact_path.clone(),
                });
            }
            let is_glb = self.artifacts.iter().any(|artifact| {
                artifact.kind == ManifestArtifactKind::Glb && artifact.path == package.artifact_path
            });
            if !is_glb {
                return Err(ManifestError::MissingPackagedSceneArtifact {
                    id: package.id.clone(),
                    path: package.artifact_path.clone(),
                });
            }
            if let Some(parent) = &package.parent {
                let parent_package = packages.get(parent).ok_or_else(|| {
                    ManifestError::MissingPackagedSceneParent {
                        id: package.id.clone(),
                        parent: parent.clone(),
                    }
                })?;
                if parent_package.mode != PackagedSceneMode::Flattened {
                    return Err(ManifestError::InvalidPackagedSceneParent {
                        id: package.id.clone(),
                    });
                }
            }
        }
        Ok(())
    }
}

const fn package_capability(mode: PackagedSceneMode) -> CompatibilityCapability {
    match mode {
        PackagedSceneMode::Flattened => CompatibilityCapability::UsdFlattenedStage,
        PackagedSceneMode::VariantAlternative => CompatibilityCapability::UsdPackagedVariants,
        PackagedSceneMode::ExpandedInstances => CompatibilityCapability::UsdExpandedInstances,
        PackagedSceneMode::InstanceBatch => CompatibilityCapability::UsdInstanceBatches,
        PackagedSceneMode::Chunk => CompatibilityCapability::UsdSceneChunks,
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
