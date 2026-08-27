//! Bounded prepackaging of explicitly selected USD scene alternatives.
//!
//! This adapter consumes a previously inspected USD source snapshot and caller-supplied canonical
//! scenes. It never evaluates live USD composition at runtime. Instead, it emits named GLB package
//! declarations whose source selections, limits, and chunk bounds remain explicit in the parent
//! manifest.

use std::collections::{BTreeMap, BTreeSet};

use thiserror::Error;

use crate::{
    adapters::secondary::{
        glb::GlbPackage, glb::GlbPackageError, glb::GlbWriter, usd_stage::UsdStageSnapshot,
    },
    domain::{
        CanonicalScene, PackagedScene, PackagedSceneError, PackagedSceneId, PackagedSceneMode,
        SceneChunkBounds, SourceIdentity,
    },
};

const MAX_DEFAULT_PACKAGES: usize = 32;
const MAX_DEFAULT_VARIANTS: usize = 16;
const MAX_DEFAULT_INSTANCES: usize = 4_096;
const MAX_DEFAULT_REFERENCES: usize = 16;
const MAX_DEFAULT_STAGE_BYTES: u64 = 4 * 1024 * 1024;
const MAX_DEFAULT_VERTEX_BYTES: usize = 64 * 1024 * 1024;

/// Resource bounds applied before caller-supplied canonical alternatives are packaged.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsdPackageLimits {
    /// Maximum total explicitly packaged scenes, including the flattened fallback.
    pub maximum_packages: usize,
    /// Maximum named alternatives in one parent package.
    pub maximum_variant_alternatives: usize,
    /// Maximum canonical shared-instance attachments in any one alternative.
    pub maximum_instances_per_package: usize,
    /// Maximum source reference facts accepted without an authoritative composition adapter.
    pub maximum_reference_arcs: usize,
    /// Maximum root-stage bytes accepted for this packaging transaction.
    pub maximum_stage_bytes: u64,
    /// Maximum estimated canonical vertex bytes across all supplied scenes.
    pub maximum_total_vertex_bytes: usize,
}

impl Default for UsdPackageLimits {
    fn default() -> Self {
        Self {
            maximum_packages: MAX_DEFAULT_PACKAGES,
            maximum_variant_alternatives: MAX_DEFAULT_VARIANTS,
            maximum_instances_per_package: MAX_DEFAULT_INSTANCES,
            maximum_reference_arcs: MAX_DEFAULT_REFERENCES,
            maximum_stage_bytes: MAX_DEFAULT_STAGE_BYTES,
            maximum_total_vertex_bytes: MAX_DEFAULT_VERTEX_BYTES,
        }
    }
}

/// One caller-supplied canonical scene placed into an explicit USD package mode.
#[derive(Clone, Debug)]
pub struct UsdPackageInput {
    /// Stable package identifier. Chunk IDs must be generated with [`UsdPackagePlanner::chunk_id`].
    pub id: PackagedSceneId,
    /// Explicit behavior exposed to the runtime.
    pub mode: PackagedSceneMode,
    /// Immediate flattened package parent for child package modes.
    pub parent: Option<PackagedSceneId>,
    /// Explicit source values selected before package creation.
    pub selected_values: BTreeMap<String, String>,
    /// Required bounds for chunk packages and absent for all other modes.
    pub bounds: Option<SceneChunkBounds>,
    /// Fully canonicalized scene materialized by an upstream adapter.
    pub scene: CanonicalScene,
}

/// One validated package declaration plus its deterministic GLB bytes.
#[derive(Clone, Debug)]
pub struct UsdPackagedArtifact {
    /// Manifest-ready package declaration.
    pub declaration: PackagedScene,
    /// Deterministic package GLB.
    pub glb: GlbPackage,
}

/// All explicit GLB alternatives belonging to one selected USD stage.
#[derive(Clone, Debug)]
pub struct UsdPackagePlan {
    /// Stable package artifacts sorted by package identifier.
    pub artifacts: Vec<UsdPackagedArtifact>,
}

impl UsdPackagePlan {
    /// Returns the package declarations in deterministic identifier order.
    #[must_use]
    pub fn declarations(&self) -> Vec<PackagedScene> {
        self.artifacts.iter().map(|artifact| artifact.declaration.clone()).collect()
    }
}

/// Bounded package planner for source snapshots and prebuilt canonical alternatives.
#[derive(Clone, Debug, Default)]
pub struct UsdPackagePlanner {
    limits: UsdPackageLimits,
}

impl UsdPackagePlanner {
    /// Creates a planner with explicit resource bounds.
    #[must_use]
    pub const fn with_limits(limits: UsdPackageLimits) -> Self {
        Self { limits }
    }

    /// Builds stable GLB alternatives for a selected USD source snapshot.
    ///
    /// # Errors
    ///
    /// Returns [`UsdPackageError`] before GLB allocation when source facts or package limits are
    /// exceeded, and wraps canonical package/GLB errors otherwise.
    pub fn package(
        &self,
        snapshot: &UsdStageSnapshot,
        inputs: Vec<UsdPackageInput>,
    ) -> Result<UsdPackagePlan, UsdPackageError> {
        self.validate_source(snapshot)?;
        self.validate_input_limits(&inputs)?;
        let source = require_usd_source(&snapshot.source)?;
        let mut artifacts = inputs
            .into_iter()
            .map(|input| Self::package_input(source, input))
            .collect::<Result<Vec<_>, _>>()?;
        artifacts.sort_by_key(|artifact| artifact.declaration.id.clone());
        validate_plan_relationships(&artifacts)?;
        Ok(UsdPackagePlan { artifacts })
    }

    /// Derives a deterministic chunk identity from one parent, source prim, and stable bounds.
    ///
    /// # Errors
    ///
    /// Returns [`UsdPackageError::NonUsdSource`] for a non-USD source and
    /// [`UsdPackageError::Package`] if the resulting portable identifier is invalid.
    pub fn chunk_id(
        parent: &PackagedSceneId,
        source: &SourceIdentity,
        bounds: &SceneChunkBounds,
    ) -> Result<PackagedSceneId, UsdPackageError> {
        let source = require_usd_source(source)?;
        let material = format!(
            "{}|{}|{:?}|{:?}",
            parent, source, bounds.minimum_millimeters, bounds.maximum_millimeters
        );
        let digest = crate::domain::ArtifactHash::from_bytes(material.as_bytes());
        let suffix = digest.as_str().get(..16).ok_or(UsdPackageError::ChunkIdUnavailable)?;
        format!("chunk-{suffix}").parse().map_err(UsdPackageError::Package)
    }

    fn validate_source(&self, snapshot: &UsdStageSnapshot) -> Result<(), UsdPackageError> {
        let facts = snapshot.facts.iter().collect::<BTreeSet<_>>();
        let stage = match require_usd_source(&snapshot.source)? {
            SourceIdentity::Usd { stage, .. } => stage.as_str(),
            SourceIdentity::Unreal { .. }
            | SourceIdentity::Unity { .. }
            | SourceIdentity::Godot { .. } => {
                return Err(UsdPackageError::NonUsdSource);
            }
        };
        if facts.iter().any(|fact| {
            fact.strip_prefix("composition.reference=").is_some_and(|reference| reference == stage)
        }) {
            return Err(UsdPackageError::RecursiveReference { stage: stage.to_owned() });
        }
        let stage_bytes = fact_value(&facts, "stage.bytes=")
            .ok_or(UsdPackageError::MissingStageBytes)?
            .parse::<u64>()
            .map_err(|_| UsdPackageError::InvalidStageBytes)?;
        if stage_bytes > self.limits.maximum_stage_bytes {
            return Err(UsdPackageError::StageBytesExceeded {
                actual: stage_bytes,
                limit: self.limits.maximum_stage_bytes,
            });
        }
        let references =
            facts.iter().filter(|fact| fact.starts_with("composition.reference=")).count();
        if references > self.limits.maximum_reference_arcs {
            return Err(UsdPackageError::ReferenceLimitExceeded {
                actual: references,
                limit: self.limits.maximum_reference_arcs,
            });
        }
        if facts.iter().any(|fact| fact.starts_with("composition.payload.unselected=")) {
            return Err(UsdPackageError::PayloadRequiresAuthoritativeAdapter);
        }
        Ok(())
    }

    fn validate_input_limits(&self, inputs: &[UsdPackageInput]) -> Result<(), UsdPackageError> {
        if inputs.is_empty() || inputs.len() > self.limits.maximum_packages {
            return Err(UsdPackageError::PackageLimitExceeded {
                actual: inputs.len(),
                limit: self.limits.maximum_packages,
            });
        }
        let variants = inputs
            .iter()
            .filter(|input| input.mode == PackagedSceneMode::VariantAlternative)
            .count();
        if variants > self.limits.maximum_variant_alternatives {
            return Err(UsdPackageError::VariantLimitExceeded {
                actual: variants,
                limit: self.limits.maximum_variant_alternatives,
            });
        }
        let mut vertex_bytes = 0_usize;
        for input in inputs {
            let instances = input.scene.nodes.iter().filter(|node| node.instance.is_some()).count();
            if instances > self.limits.maximum_instances_per_package {
                return Err(UsdPackageError::InstanceLimitExceeded {
                    actual: instances,
                    limit: self.limits.maximum_instances_per_package,
                });
            }
            let bytes = estimated_vertex_bytes(&input.scene)?;
            vertex_bytes =
                vertex_bytes.checked_add(bytes).ok_or(UsdPackageError::VertexBytesOverflow)?;
            if vertex_bytes > self.limits.maximum_total_vertex_bytes {
                return Err(UsdPackageError::VertexBytesExceeded {
                    actual: vertex_bytes,
                    limit: self.limits.maximum_total_vertex_bytes,
                });
            }
        }
        Ok(())
    }

    fn package_input(
        source: &SourceIdentity,
        input: UsdPackageInput,
    ) -> Result<UsdPackagedArtifact, UsdPackageError> {
        validate_mode_input(&input)?;
        let source_links = source_links(&input.scene, source)?;
        let path = artifact_path(&input.id, input.mode);
        let declaration = PackagedScene::new(
            input.id,
            input.mode,
            path,
            input.parent,
            source_links,
            input.selected_values,
            input.bounds,
        )
        .map_err(UsdPackageError::Package)?;
        let glb = GlbWriter.package(&input.scene).map_err(UsdPackageError::Glb)?;
        Ok(UsdPackagedArtifact { declaration, glb })
    }
}

/// T25 package-planning failures.
#[derive(Debug, Error)]
pub enum UsdPackageError {
    /// The supplied snapshot does not carry a USD source identity.
    #[error("USD package planning requires a USD source identity")]
    NonUsdSource,
    /// A snapshot omitted the stage-size fact required for bounded planning.
    #[error("USD package planning snapshot is missing stage.bytes")]
    MissingStageBytes,
    /// The stage-size fact was not an unsigned byte count.
    #[error("USD package planning stage.bytes is invalid")]
    InvalidStageBytes,
    /// The selected root layer exceeds the configured package limit.
    #[error("USD stage size {actual} exceeds the {limit}-byte package limit")]
    StageBytesExceeded { actual: u64, limit: u64 },
    /// The source snapshot directly references its own root layer.
    #[error("USD stage recursively references its own root layer: {stage}")]
    RecursiveReference { stage: String },
    /// The source snapshot has too many reference arcs for this bounded planner.
    #[error("USD reference count {actual} exceeds the configured limit {limit}")]
    ReferenceLimitExceeded { actual: usize, limit: usize },
    /// Payload content cannot be silently materialized without an authoritative adapter.
    #[error("USD payload content requires an authoritative preflattening adapter")]
    PayloadRequiresAuthoritativeAdapter,
    /// Package count would exceed the configured allocation bound.
    #[error("USD package count {actual} exceeds the configured limit {limit}")]
    PackageLimitExceeded { actual: usize, limit: usize },
    /// Variant alternatives would exceed the configured allocation bound.
    #[error("USD variant alternative count {actual} exceeds the configured limit {limit}")]
    VariantLimitExceeded { actual: usize, limit: usize },
    /// Materialized instances would exceed the configured allocation bound.
    #[error("USD instance count {actual} exceeds the configured limit {limit}")]
    InstanceLimitExceeded { actual: usize, limit: usize },
    /// Estimated vertex data overflowed the host address space.
    #[error("USD package vertex-byte accounting overflowed")]
    VertexBytesOverflow,
    /// Estimated vertex data would exceed the configured allocation bound.
    #[error("USD vertex bytes {actual} exceed the configured limit {limit}")]
    VertexBytesExceeded { actual: usize, limit: usize },
    /// One package does not declare the mode-specific facts required for safe packaging.
    #[error("USD package {id} has invalid {reason} mode input")]
    InvalidModeInput { id: PackagedSceneId, reason: &'static str },
    /// A canonical package scene loses the selected USD source identity.
    #[error("USD package {id} does not retain the selected source identity")]
    MissingSourceLink { id: PackagedSceneId },
    /// A deterministic chunk ID could not be derived from a fixed digest prefix.
    #[error("USD chunk identifier could not be derived")]
    ChunkIdUnavailable,
    /// Generic packaged-scene validation failed.
    #[error("USD package declaration is invalid: {0}")]
    Package(PackagedSceneError),
    /// Canonical GLB packaging failed.
    #[error("USD package GLB generation failed: {0}")]
    Glb(GlbPackageError),
    /// Multiple packages use the same identifier.
    #[error("USD package identifier is duplicated: {id}")]
    DuplicatePackage { id: PackagedSceneId },
    /// The package set needs exactly one flattened fallback.
    #[error("USD package set requires exactly one flattened fallback")]
    InvalidFlattenedCount,
    /// A child package parent was absent or was not the flattened fallback.
    #[error("USD package {id} has an invalid flattened parent")]
    InvalidParent { id: PackagedSceneId },
}

const fn require_usd_source(source: &SourceIdentity) -> Result<&SourceIdentity, UsdPackageError> {
    if matches!(source, SourceIdentity::Usd { .. }) {
        Ok(source)
    } else {
        Err(UsdPackageError::NonUsdSource)
    }
}

fn fact_value<'a>(facts: &BTreeSet<&'a String>, prefix: &str) -> Option<&'a str> {
    facts.iter().find_map(|fact| fact.strip_prefix(prefix))
}

fn estimated_vertex_bytes(scene: &CanonicalScene) -> Result<usize, UsdPackageError> {
    scene.vertex_streams.iter().try_fold(0_usize, |total, stream| {
        let attributes = 3_usize
            .saturating_add(usize::from(stream.normals.is_some()) * 3)
            .saturating_add(usize::from(stream.tangents.is_some()) * 4)
            .saturating_add(usize::from(stream.tex_coords_0.is_some()) * 2)
            .saturating_add(usize::from(stream.colors_0.is_some()) * 4)
            .saturating_add(usize::from(stream.joints_0.is_some()) * 2 * 4)
            .saturating_add(usize::from(stream.weights_0.is_some()) * 4);
        let vertices = stream
            .positions
            .len()
            .checked_mul(attributes)
            .ok_or(UsdPackageError::VertexBytesOverflow)?;
        let vertex_bytes = vertices
            .checked_mul(std::mem::size_of::<f32>())
            .ok_or(UsdPackageError::VertexBytesOverflow)?;
        let index_bytes = stream
            .indices
            .len()
            .checked_mul(std::mem::size_of::<u32>())
            .ok_or(UsdPackageError::VertexBytesOverflow)?;
        total
            .checked_add(vertex_bytes)
            .and_then(|value| value.checked_add(index_bytes))
            .ok_or(UsdPackageError::VertexBytesOverflow)
    })
}

fn validate_mode_input(input: &UsdPackageInput) -> Result<(), UsdPackageError> {
    let valid = match input.mode {
        PackagedSceneMode::Flattened => true,
        PackagedSceneMode::VariantAlternative => {
            !input.selected_values.is_empty()
                && input.selected_values.keys().all(|key| key.starts_with("variant."))
        }
        PackagedSceneMode::ExpandedInstances | PackagedSceneMode::InstanceBatch => {
            input.scene.nodes.iter().any(|node| node.instance.is_some())
        }
        PackagedSceneMode::Chunk => input.bounds.is_some(),
    };
    if valid {
        Ok(())
    } else {
        Err(UsdPackageError::InvalidModeInput { id: input.id.clone(), reason: "declared" })
    }
}

fn source_links(
    scene: &CanonicalScene,
    selected_source: &SourceIdentity,
) -> Result<Vec<SourceIdentity>, UsdPackageError> {
    let source_links =
        scene.source_links.iter().map(|link| link.source.clone()).collect::<Vec<_>>();
    if source_links.iter().any(|source| source == selected_source) {
        Ok(source_links)
    } else {
        Err(UsdPackageError::MissingSourceLink {
            id: "unknown".parse().map_err(UsdPackageError::Package)?,
        })
    }
}

fn artifact_path(id: &PackagedSceneId, mode: PackagedSceneMode) -> String {
    let directory = match mode {
        PackagedSceneMode::Flattened => "scenes",
        PackagedSceneMode::VariantAlternative => "variants",
        PackagedSceneMode::ExpandedInstances => "expanded-instances",
        PackagedSceneMode::InstanceBatch => "instance-batches",
        PackagedSceneMode::Chunk => "chunks",
    };
    format!("{directory}/{id}.glb")
}

fn validate_plan_relationships(artifacts: &[UsdPackagedArtifact]) -> Result<(), UsdPackageError> {
    let flattened = artifacts
        .iter()
        .filter(|artifact| artifact.declaration.mode == PackagedSceneMode::Flattened)
        .count();
    if flattened != 1 {
        return Err(UsdPackageError::InvalidFlattenedCount);
    }
    let declarations = artifacts
        .iter()
        .map(|artifact| (artifact.declaration.id.clone(), &artifact.declaration))
        .collect::<BTreeMap<_, _>>();
    if declarations.len() != artifacts.len() {
        let duplicate = artifacts
            .windows(2)
            .filter_map(|pair| pair.first().zip(pair.get(1)))
            .find_map(|(left, right)| {
                (left.declaration.id == right.declaration.id).then(|| left.declaration.id.clone())
            })
            .ok_or(UsdPackageError::InvalidFlattenedCount)?;
        return Err(UsdPackageError::DuplicatePackage { id: duplicate });
    }
    for artifact in artifacts {
        if let Some(parent) = &artifact.declaration.parent {
            let valid_parent = declarations
                .get(parent)
                .is_some_and(|candidate| candidate.mode == PackagedSceneMode::Flattened);
            if !valid_parent {
                return Err(UsdPackageError::InvalidParent { id: artifact.declaration.id.clone() });
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, str::FromStr};

    use super::{UsdPackageError, UsdPackageInput, UsdPackageLimits, UsdPackagePlanner};
    use crate::{
        adapters::secondary::usd_stage::{UsdPrimSnapshot, UsdStageSnapshot},
        domain::{
            CanonicalScene, ClassificationKind, ConversionClassification, CoordinateNormalization,
            PackagedSceneId, PackagedSceneMode, Remediation, SourceAssetPath, SourceIdentity,
            StageSelection, UsdPrimPath,
            scene_ir::{
                Instance, InstanceId, Mesh, MeshId, Node, NodeId, SourceLink, SourceLinkId,
                Transform, VertexStream, VertexStreamId,
            },
        },
    };

    fn source() -> Result<SourceIdentity, Box<dyn std::error::Error>> {
        Ok(SourceIdentity::Usd {
            stage: SourceAssetPath::from_str("fixtures/package.usda")?,
            prim_path: UsdPrimPath::from_str("/World")?,
            selection: StageSelection::from_str(
                "context=universal;payload=forbid;purpose=render;time=stage;variants=look:blue",
            )?,
        })
    }

    fn snapshot(facts: Vec<String>) -> Result<UsdStageSnapshot, Box<dyn std::error::Error>> {
        Ok(UsdStageSnapshot {
            source: source()?,
            classification: ConversionClassification::new(
                ClassificationKind::RequiresAdapter,
                "bounded selected stage",
                Remediation::new("use an approved packaging adapter")?,
            )?,
            facts,
            prims: vec![UsdPrimSnapshot {
                path: UsdPrimPath::from_str("/World")?,
                specifier: "def".to_owned(),
                type_name: Some("Xform".to_owned()),
                purpose: Some("render".to_owned()),
                visibility: Some("inherited".to_owned()),
                instancable: true,
                transforms: Vec::new(),
            }],
        })
    }

    fn scene(with_instance: bool) -> Result<CanonicalScene, Box<dyn std::error::Error>> {
        Ok(CanonicalScene::new(CanonicalScene {
            normalization: CoordinateNormalization {
                meters_per_unit: "1".to_owned(),
                source_up_axis: "Y".to_owned(),
                target_up_axis: "Y".to_owned(),
                winding: "preserve".to_owned(),
                uv_origin: "lower_left".to_owned(),
                color_space: "linear".to_owned(),
            },
            source_links: vec![SourceLink {
                id: SourceLinkId::new(0),
                source: source()?,
                facts: BTreeMap::new(),
            }],
            nodes: vec![Node {
                id: NodeId::new(0),
                parent: None,
                transform: Transform::default(),
                mesh: (!with_instance).then_some(MeshId::new(0)),
                camera: None,
                light: None,
                instance: with_instance.then_some(InstanceId::new(0)),
                source_link: SourceLinkId::new(0),
            }],
            meshes: vec![Mesh { id: MeshId::new(0), primitives: Vec::new(), skin: None }],
            primitives: Vec::new(),
            vertex_streams: vec![VertexStream {
                id: VertexStreamId::new(0),
                positions: Vec::new(),
                normals: None,
                tangents: None,
                tex_coords_0: None,
                colors_0: None,
                joints_0: None,
                weights_0: None,
                indices: Vec::new(),
            }],
            materials: Vec::new(),
            textures: Vec::new(),
            samplers: Vec::new(),
            skins: Vec::new(),
            joints: Vec::new(),
            animation_clips: Vec::new(),
            channels: Vec::new(),
            morph_targets: Vec::new(),
            cameras: Vec::new(),
            lights: Vec::new(),
            instances: with_instance
                .then_some(Instance { id: InstanceId::new(0), mesh: MeshId::new(0) })
                .into_iter()
                .collect(),
            unsupported_features: Vec::new(),
        })?)
    }

    fn input(
        id: &str,
        mode: PackagedSceneMode,
        parent: Option<&str>,
        selected_values: BTreeMap<String, String>,
        with_instance: bool,
    ) -> Result<UsdPackageInput, Box<dyn std::error::Error>> {
        Ok(UsdPackageInput {
            id: PackagedSceneId::from_str(id)?,
            mode,
            parent: parent.map(PackagedSceneId::from_str).transpose()?,
            selected_values,
            bounds: None,
            scene: scene(with_instance)?,
        })
    }

    #[test]
    fn named_variants_and_instance_batches_are_packaged_deterministically()
    -> Result<(), Box<dyn std::error::Error>> {
        let snapshot = snapshot(vec!["stage.bytes=42".to_owned()])?;
        let planner = UsdPackagePlanner::default();
        let plan = planner.package(
            &snapshot,
            vec![
                input("base", PackagedSceneMode::Flattened, None, BTreeMap::new(), false)?,
                input(
                    "look-blue",
                    PackagedSceneMode::VariantAlternative,
                    Some("base"),
                    BTreeMap::from([("variant.look".to_owned(), "blue".to_owned())]),
                    false,
                )?,
                input(
                    "batch-a",
                    PackagedSceneMode::InstanceBatch,
                    Some("base"),
                    BTreeMap::new(),
                    true,
                )?,
            ],
        )?;
        assert_eq!(plan.artifacts.len(), 3);
        assert!(plan.artifacts.iter().all(|artifact| !artifact.glb.bytes.is_empty()));
        let second = planner.package(
            &snapshot,
            vec![
                input("base", PackagedSceneMode::Flattened, None, BTreeMap::new(), false)?,
                input(
                    "look-blue",
                    PackagedSceneMode::VariantAlternative,
                    Some("base"),
                    BTreeMap::from([("variant.look".to_owned(), "blue".to_owned())]),
                    false,
                )?,
                input(
                    "batch-a",
                    PackagedSceneMode::InstanceBatch,
                    Some("base"),
                    BTreeMap::new(),
                    true,
                )?,
            ],
        )?;
        assert_eq!(
            plan.artifacts.iter().map(|artifact| &artifact.glb.bytes).collect::<Vec<_>>(),
            second.artifacts.iter().map(|artifact| &artifact.glb.bytes).collect::<Vec<_>>()
        );
        Ok(())
    }

    fn require_error<T>(
        result: Result<T, UsdPackageError>,
    ) -> Result<UsdPackageError, Box<dyn std::error::Error>> {
        match result {
            Ok(_) => Err("expected bounded USD package failure".into()),
            Err(error) => Ok(error),
        }
    }

    #[test]
    fn payload_and_variant_limits_fail_before_packaging() -> Result<(), Box<dyn std::error::Error>>
    {
        let planner = UsdPackagePlanner::with_limits(UsdPackageLimits {
            maximum_variant_alternatives: 0,
            ..UsdPackageLimits::default()
        });
        let variant = input(
            "look-blue",
            PackagedSceneMode::VariantAlternative,
            Some("base"),
            BTreeMap::from([("variant.look".to_owned(), "blue".to_owned())]),
            false,
        )?;
        let error = require_error(planner.package(
            &snapshot(vec!["stage.bytes=42".to_owned()])?,
            vec![
                input("base", PackagedSceneMode::Flattened, None, BTreeMap::new(), false)?,
                variant,
            ],
        ))?;
        assert!(matches!(error, UsdPackageError::VariantLimitExceeded { .. }));
        let payload = require_error(planner.package(
            &snapshot(vec![
                "stage.bytes=42".to_owned(),
                "composition.payload.unselected=large.usdc".to_owned(),
            ])?,
            vec![input("base", PackagedSceneMode::Flattened, None, BTreeMap::new(), false)?],
        ))?;
        assert!(matches!(payload, UsdPackageError::PayloadRequiresAuthoritativeAdapter));
        Ok(())
    }

    #[test]
    fn recursive_references_and_instance_limits_fail_before_glb_packaging()
    -> Result<(), Box<dyn std::error::Error>> {
        let recursive = require_error(UsdPackagePlanner::default().package(
            &snapshot(vec![
                "stage.bytes=42".to_owned(),
                "composition.reference=fixtures/package.usda".to_owned(),
            ])?,
            vec![input("base", PackagedSceneMode::Flattened, None, BTreeMap::new(), false)?],
        ))?;
        assert!(matches!(recursive, UsdPackageError::RecursiveReference { .. }));

        let limited = require_error(
            UsdPackagePlanner::with_limits(UsdPackageLimits {
                maximum_instances_per_package: 0,
                ..UsdPackageLimits::default()
            })
            .package(
                &snapshot(vec!["stage.bytes=42".to_owned()])?,
                vec![
                    input("base", PackagedSceneMode::Flattened, None, BTreeMap::new(), false)?,
                    input(
                        "batch-a",
                        PackagedSceneMode::InstanceBatch,
                        Some("base"),
                        BTreeMap::new(),
                        true,
                    )?,
                ],
            ),
        )?;
        assert!(matches!(limited, UsdPackageError::InstanceLimitExceeded { .. }));
        Ok(())
    }
}
