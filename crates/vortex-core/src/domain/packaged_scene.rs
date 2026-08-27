//! Explicit, source-neutral packaged scene alternatives for runtime selection and chunk loading.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    str::FromStr,
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::SourceIdentity;

const MAX_PACKAGED_SCENE_ID_BYTES: usize = 96;

/// Stable identifier for one explicitly packaged scene artifact.
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct PackagedSceneId(String);

impl PackagedSceneId {
    /// Returns the canonical package identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PackagedSceneId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl FromStr for PackagedSceneId {
    type Err = PackagedSceneError;

    /// Parses a portable package identity.
    ///
    /// # Errors
    ///
    /// Returns [`PackagedSceneError::InvalidId`] when the value is empty, too long, or contains a
    /// nonportable character.
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let valid = !value.is_empty()
            && value.len() <= MAX_PACKAGED_SCENE_ID_BYTES
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'));
        if !valid {
            return Err(PackagedSceneError::InvalidId { value: value.to_owned() });
        }
        Ok(Self(value.to_owned()))
    }
}

/// Explicit runtime behavior represented by one package artifact.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PackagedSceneMode {
    /// A deterministic selected-stage fallback with source composition already resolved upstream.
    Flattened,
    /// A named source selection exported before runtime and switchable without live composition.
    VariantAlternative,
    /// Per-instance nodes materialized into the package.
    ExpandedInstances,
    /// Nodes sharing one mesh in a supported batch package.
    InstanceBatch,
    /// A spatially bounded scene portion loaded separately from its parent package.
    Chunk,
}

/// Quantized spatial bounds for a separately packaged scene chunk.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SceneChunkBounds {
    /// Inclusive lower bound in normalized millimeters.
    pub minimum_millimeters: [i64; 3],
    /// Inclusive upper bound in normalized millimeters.
    pub maximum_millimeters: [i64; 3],
}

impl SceneChunkBounds {
    /// Creates validated deterministic chunk bounds.
    ///
    /// # Errors
    ///
    /// Returns [`PackagedSceneError::InvalidBounds`] when a lower component exceeds its matching
    /// upper component.
    pub fn new(
        minimum_millimeters: [i64; 3],
        maximum_millimeters: [i64; 3],
    ) -> Result<Self, PackagedSceneError> {
        if minimum_millimeters
            .iter()
            .zip(maximum_millimeters)
            .any(|(minimum, maximum)| minimum > &maximum)
        {
            return Err(PackagedSceneError::InvalidBounds);
        }
        Ok(Self { minimum_millimeters, maximum_millimeters })
    }
}

/// One prebuilt artifact that may be selected or loaded without source composition at runtime.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PackagedScene {
    /// Stable package identity.
    pub id: PackagedSceneId,
    /// Explicit packaged behavior.
    pub mode: PackagedSceneMode,
    /// Root-confined artifact path relative to the containing manifest.
    pub artifact_path: String,
    /// Optional immediate parent package identity.
    pub parent: Option<PackagedSceneId>,
    /// Source identities preserved by this package or chunk.
    pub source_links: Vec<SourceIdentity>,
    /// Explicit selected source values, such as `variant.look=blue`.
    pub selected_values: BTreeMap<String, String>,
    /// Required stable bounds for independently loadable chunks.
    pub bounds: Option<SceneChunkBounds>,
}

impl PackagedScene {
    /// Creates one validated explicit package declaration.
    ///
    /// # Errors
    ///
    /// Returns [`PackagedSceneError`] when local package invariants are violated.
    pub fn new(
        id: PackagedSceneId,
        mode: PackagedSceneMode,
        artifact_path: impl Into<String>,
        parent: Option<PackagedSceneId>,
        mut source_links: Vec<SourceIdentity>,
        selected_values: BTreeMap<String, String>,
        bounds: Option<SceneChunkBounds>,
    ) -> Result<Self, PackagedSceneError> {
        source_links.sort_by_key(ToString::to_string);
        let package = Self {
            id,
            mode,
            artifact_path: artifact_path.into(),
            parent,
            source_links,
            selected_values,
            bounds,
        };
        package.validate()?;
        Ok(package)
    }

    /// Validates local package invariants after deserialization or construction.
    ///
    /// # Errors
    ///
    /// Returns [`PackagedSceneError`] when artifact, provenance, parent, selection, or bounds
    /// requirements for the declared mode are violated.
    pub fn validate(&self) -> Result<(), PackagedSceneError> {
        self.validate_local()
    }

    /// Returns whether this package represents a runtime-selectable variant alternative.
    #[must_use]
    pub const fn is_variant(&self) -> bool {
        matches!(self.mode, PackagedSceneMode::VariantAlternative)
    }

    fn validate_local(&self) -> Result<(), PackagedSceneError> {
        if self.artifact_path.is_empty() {
            return Err(PackagedSceneError::EmptyArtifactPath { id: self.id.clone() });
        }
        if self.source_links.is_empty() {
            return Err(PackagedSceneError::MissingSourceLinks { id: self.id.clone() });
        }
        if self.source_links.iter().map(ToString::to_string).collect::<BTreeSet<_>>().len()
            != self.source_links.len()
        {
            return Err(PackagedSceneError::DuplicateSourceLink { id: self.id.clone() });
        }
        match self.mode {
            PackagedSceneMode::Flattened => {
                if self.parent.is_some() || self.bounds.is_some() {
                    return Err(PackagedSceneError::InvalidFlattenedRelationship {
                        id: self.id.clone(),
                    });
                }
            }
            PackagedSceneMode::VariantAlternative => {
                if self.parent.is_none() || self.selected_values.is_empty() || self.bounds.is_some()
                {
                    return Err(PackagedSceneError::InvalidVariantAlternative {
                        id: self.id.clone(),
                    });
                }
            }
            PackagedSceneMode::ExpandedInstances | PackagedSceneMode::InstanceBatch => {
                if self.parent.is_none() || self.bounds.is_some() {
                    return Err(PackagedSceneError::InvalidInstancePackage { id: self.id.clone() });
                }
            }
            PackagedSceneMode::Chunk => {
                if self.parent.is_none() || self.bounds.is_none() {
                    return Err(PackagedSceneError::InvalidChunk { id: self.id.clone() });
                }
            }
        }
        Ok(())
    }
}

/// Bounded package declaration errors.
#[derive(Debug, Error)]
pub enum PackagedSceneError {
    /// The package identifier does not use the portable grammar.
    #[error("invalid packaged scene identifier: {value}")]
    InvalidId { value: String },
    /// The package does not identify an artifact.
    #[error("packaged scene {id} has no artifact path")]
    EmptyArtifactPath { id: PackagedSceneId },
    /// A package loses its source provenance.
    #[error("packaged scene {id} has no source links")]
    MissingSourceLinks { id: PackagedSceneId },
    /// A package repeats a source identity.
    #[error("packaged scene {id} duplicates a source link")]
    DuplicateSourceLink { id: PackagedSceneId },
    /// A flattened package has child-only metadata.
    #[error("flattened packaged scene {id} cannot declare a parent or bounds")]
    InvalidFlattenedRelationship { id: PackagedSceneId },
    /// A named alternative lacks the data needed for safe selection.
    #[error("variant alternative {id} must have a parent, selected values, and no chunk bounds")]
    InvalidVariantAlternative { id: PackagedSceneId },
    /// An instance package lacks its flattened parent or declares chunk-only bounds.
    #[error("instance package {id} must have a parent and cannot declare chunk bounds")]
    InvalidInstancePackage { id: PackagedSceneId },
    /// A chunk lacks its parent or stable bounds.
    #[error("chunk package {id} must have a parent and bounds")]
    InvalidChunk { id: PackagedSceneId },
    /// The quantized chunk bounds are inverted.
    #[error("chunk bounds minimum components must not exceed maximum components")]
    InvalidBounds,
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, str::FromStr};

    use super::{PackagedScene, PackagedSceneError, PackagedSceneMode, SceneChunkBounds};
    use crate::domain::{SourceAssetPath, SourceIdentity, StageSelection, UsdPrimPath};

    fn source() -> Result<SourceIdentity, Box<dyn std::error::Error>> {
        Ok(SourceIdentity::Usd {
            stage: SourceAssetPath::from_str("fixtures/scene.usda")?,
            prim_path: UsdPrimPath::from_str("/World")?,
            selection: StageSelection::from_str(
                "context=universal;payload=forbid;purpose=render;time=stage;variants=",
            )?,
        })
    }

    #[test]
    fn package_invariants_preserve_source_links_and_bounds()
    -> Result<(), Box<dyn std::error::Error>> {
        let flattened = PackagedScene::new(
            "base".parse()?,
            PackagedSceneMode::Flattened,
            "scenes/base.glb",
            None,
            vec![source()?],
            BTreeMap::new(),
            None,
        )?;
        assert_eq!(flattened.source_links.len(), 1);
        let bounds = SceneChunkBounds::new([0, 0, 0], [1_000, 1_000, 1_000])?;
        let chunk = PackagedScene::new(
            "west".parse()?,
            PackagedSceneMode::Chunk,
            "chunks/west.glb",
            Some("base".parse()?),
            vec![source()?],
            BTreeMap::new(),
            Some(bounds),
        )?;
        assert_eq!(chunk.parent, Some("base".parse()?));
        Ok(())
    }

    #[test]
    fn variant_without_explicit_selection_is_rejected() -> Result<(), Box<dyn std::error::Error>> {
        let Err(error) = PackagedScene::new(
            "look-blue".parse()?,
            PackagedSceneMode::VariantAlternative,
            "variants/look-blue.glb",
            Some("base".parse()?),
            vec![source()?],
            BTreeMap::new(),
            None,
        ) else {
            return Err("variant package unexpectedly accepted empty selection".into());
        };
        assert!(matches!(error, PackagedSceneError::InvalidVariantAlternative { .. }));
        Ok(())
    }
}
