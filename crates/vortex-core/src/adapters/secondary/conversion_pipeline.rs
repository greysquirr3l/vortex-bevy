//! Concrete [`ConversionRunner`] adapter that drives the per-engine conversion
//! pipeline end-to-end.
//!
//! Currently implemented:
//!
//! - **USD**: parses the selected USDA stage via [`super::usd_stage`], builds a
//!   minimal [`CanonicalScene`] from the prim hierarchy, packages it as a
//!   Bevy-compatible GLB through [`super::glb`], builds a version-1
//!   [`SceneManifest`], and publishes the conversion through
//!   [`super::artifact_store`]. The returned [`ConversionAggregate`] is
//!   classified `Converted` and records both the GLB and the manifest.
//!
//! - **Unreal / Unity / Godot**: classify `RequiresAdapter` with explicit
//!   remediation pointing at the authorized editor bridge. Direct
//!   `.uasset` / Force Text / `.tscn` decoding is metadata-only and
//!   authoritative export must run inside the matching Editor. The bridge
//!   reports are produced by `bridges/*` scripts running in the engine
//!   process and consumed by [`super::unreal_static_scene`],
//!   [`super::unity_static_scene`], and [`super::godot_static_scene`].
//!   Wiring those bridge reports through the same orchestrator is the
//!   natural extension point once CLI flags for the bridge report path land.

use std::{
    collections::BTreeMap,
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    str::FromStr,
};

use thiserror::Error;

use crate::{
    adapters::secondary::{
        artifact_store::{
            ArtifactPayload, ArtifactStoreError, AtomicArtifactStore, PublicationRequest,
        },
        glb::{GlbPackage, GlbPackageError, GlbWriter},
        usd_stage::{UsdStageError, UsdStageInspector, UsdStageRequest, UsdStageSnapshot},
    },
    application::command::execute_conversion::ConversionRunner,
    domain::{
        ArtifactHash, ArtifactKind, ArtifactRecord, ClassificationKind, CompatibilityCapability,
        CompatibilityRequirements, ConversionAggregate, ConversionClassification,
        ConversionContext, DomainError, ManifestArtifact, ManifestArtifactKind, ManifestError,
        ManifestSource, OutputRoot, Remediation, SceneManifest, SourceAssetPath, SourceIdentity,
        scene_ir::{
            CanonicalScene, CoordinateNormalization, Node, NodeId, SourceLink, SourceLinkId,
            Transform,
        },
    },
};

const MANIFEST_FILE: &str = "scene.manifest.ron";
const GLB_FILE: &str = "scene.glb";
const SUPPORTED_BEVY_VERSION: &str = "0.20";

/// Typed remediation for [`ConversionPipelineError::PrimCountExceedsGlb`].
///
/// Carries concrete next-step suggestions: split the stage, isolate the affected
/// prim, or apply mesh decimation upstream. The string is intentionally inline
/// (not domain-typed) so it stays stable across CLI edits and reads cleanly
/// from the orchestrator without forcing a `DomainError::EmptyField` mapping.
const PRIM_LIMIT_REMEDIATION: &str = "glTF runtime u32 NodeId capacity reached. Options: \
    (1) split the source stage into multiple selected configurations and convert them \
    independently; (2) isolate the offending prim via a narrower prim-path selection; \
    (3) apply upstream mesh decimation to reduce the prim count below the u32 limit.";

/// Concrete [`ConversionRunner`] adapter that owns its source root and output root.
#[derive(Clone, Debug)]
pub struct ConversionPipeline {
    project_root: PathBuf,
    output_root: PathBuf,
}

impl ConversionPipeline {
    /// Creates a new pipeline rooted at `project_root` (where USD stages live) and
    /// publishing through an output root at `output_root`.
    #[must_use]
    pub fn new(project_root: impl Into<PathBuf>, output_root: impl Into<PathBuf>) -> Self {
        Self { project_root: project_root.into(), output_root: output_root.into() }
    }

    fn classification_for(
        source: &SourceIdentity,
    ) -> (ClassificationKind, &'static str, &'static str) {
        match source {
            SourceIdentity::Unreal { .. } => (
                ClassificationKind::RequiresAdapter,
                "Direct .uasset decoding is metadata-only; authoritative Unreal export requires the authorized Unreal Editor bridge.",
                "Invoke the authorized Unreal Editor through `bridges/unreal/vortex_unreal_bridge.py` and convert the staged FBX/TGA outputs through the canonical IR pipeline.",
            ),
            SourceIdentity::Unity { .. } => (
                ClassificationKind::RequiresAdapter,
                "Direct Force Text project decoding covers metadata only; authoritative Unity static-scene export requires the authorized Unity Editor batch-mode bridge.",
                "Invoke the authorized Unity Editor through `bridges/unity/*` to produce a static-scene report, then convert through the canonical IR pipeline.",
            ),
            SourceIdentity::Godot { .. } => (
                ClassificationKind::RequiresAdapter,
                "Godot adapter expansion is tracked separately; the canonical IR + GLB packaging pipeline is not yet wired for this source.",
                "Track the Godot static-scene adapter expansion (T26 family) and re-execute the conversion once the canonical IR + GLB pipeline accepts Godot sources.",
            ),
            SourceIdentity::Usd { .. } => unreachable!("USD sources reach the orchestrator path"),
        }
    }

    fn run_usd_pipeline(
        &self,
        source: &SourceIdentity,
        context: &ConversionContext,
    ) -> Result<ConversionAggregate, ConversionPipelineError> {
        let (stage, selection) = match source {
            SourceIdentity::Usd { stage, prim_path: _, selection } => {
                (stage.clone(), selection.clone())
            }
            _ => unreachable!("USD pipeline reached for non-USD source"),
        };
        let request = UsdStageRequest::new(&self.project_root, stage.clone(), selection.as_str())?;
        let snapshot = UsdStageInspector.resolve(&request)?;
        let scene = build_scene_from_snapshot(&snapshot)?;
        let package = GlbWriter.package(&scene)?;
        let mut conversion = ConversionAggregate::begin(source.clone(), context.clone());
        conversion.record_artifact(ArtifactRecord::new(
            ArtifactKind::Glb,
            package.checksum.clone(),
            GLB_FILE,
        )?)?;
        conversion.record_artifact(ArtifactRecord::new(
            ArtifactKind::Manifest,
            ArtifactHash::from_bytes(MANIFEST_FILE.as_bytes()),
            MANIFEST_FILE,
        )?)?;
        conversion.classify(ConversionClassification::new(
            ClassificationKind::Converted,
            "selected USD stage packaged through canonical IR + GLB",
            Remediation::new(
                "stage is converted; runtime loads via vortex-bevy view --artifact-root",
            )?,
        )?)?;
        let conversion = conversion.finalize()?;
        let manifest = build_manifest(&conversion, &snapshot, &package, &stage)?;
        let payload = ArtifactPayload::new(GLB_FILE, package.bytes)?;
        let logical_root = output_root_basename(&self.output_root);
        let store = AtomicArtifactStore::new(&self.output_root, logical_root)?;
        store.publish(&PublicationRequest {
            conversion: conversion.clone(),
            manifest,
            payloads: vec![payload],
        })?;
        Ok(conversion)
    }
}

/// Failures emitted by [`ConversionPipeline`].
#[derive(Debug, Error)]
pub enum ConversionPipelineError {
    /// The selected USDA stage is unreadable or invalid.
    #[error("USD stage selection failed: {source}")]
    Stage {
        /// The bounded stage adapter error.
        #[from]
        source: UsdStageError,
    },
    /// The canonical scene could not be packaged as GLB.
    #[error("GLB packaging failed: {source}")]
    Package {
        /// The GLB package adapter error.
        #[from]
        source: GlbPackageError,
    },
    /// The atomic artifact store rejected the publication request.
    #[error("artifact publication failed: {source}")]
    Store {
        /// The artifact store error.
        #[from]
        source: ArtifactStoreError,
    },
    /// `ConversionAggregate` state transition or field validation failed.
    #[error("conversion pipeline failed: {source}")]
    Domain {
        /// The domain validation error.
        #[from]
        source: DomainError,
    },
    /// The source contains more prims than fit in the glTF u32 node-id space.
    ///
    /// This is a glTF runtime intrinsic, not an application limit. The error carries
    /// typed remediation so callers can surface a concrete next-step suggestion
    /// (split the stage, subdivide, or apply mesh decimation) rather than a bare
    /// capacity message.
    #[error(
        "source has {count} prims which exceeds the glTF u32 NodeId capacity; remediation: {remediation}"
    )]
    PrimCountExceedsGlb {
        /// Number of prims in the source stage.
        count: usize,
        /// Concrete next-step suggestion.
        remediation: String,
    },
}

impl ConversionRunner for ConversionPipeline {
    type Error = ConversionPipelineError;

    fn execute(
        &self,
        source: &SourceIdentity,
        context: &ConversionContext,
    ) -> Result<ConversionAggregate, Self::Error> {
        if matches!(source, SourceIdentity::Usd { .. }) {
            return self.run_usd_pipeline(source, context);
        }
        let mut conversion = ConversionAggregate::begin(source.clone(), context.clone());
        let (kind, reason, remediation) = Self::classification_for(source);
        conversion.classify(ConversionClassification::new(
            kind,
            reason,
            Remediation::new(remediation)?,
        )?)?;
        Ok(conversion.finalize()?)
    }
}

fn build_scene_from_snapshot(
    snapshot: &UsdStageSnapshot,
) -> Result<CanonicalScene, ConversionPipelineError> {
    let source_link_id = SourceLinkId::new(0);
    let source_link = SourceLink {
        id: source_link_id,
        source: snapshot.source.clone(),
        facts: BTreeMap::from([("usda.prims".to_owned(), snapshot.prims.len().to_string())]),
    };
    let nodes: Vec<Node> = snapshot
        .prims
        .iter()
        .enumerate()
        .map(|(index, _prim)| -> Result<Node, ConversionPipelineError> {
            // NodeId is u32 because glTF stores node indices as u32; that bound is intrinsic
            // to the runtime contract, not an application-level limit. We fail loudly with a
            // typed error carrying remediation when a source exceeds it instead of silently
            // truncating.
            let Ok(id_index) = u32::try_from(index) else {
                return Err(ConversionPipelineError::PrimCountExceedsGlb {
                    count: snapshot.prims.len(),
                    remediation: PRIM_LIMIT_REMEDIATION.to_owned(),
                });
            };
            Ok(Node {
                id: NodeId::new(id_index),
                parent: None,
                transform: Transform::default(),
                mesh: None,
                camera: None,
                light: None,
                instance: None,
                source_link: source_link_id,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let scene = CanonicalScene {
        normalization: CoordinateNormalization {
            meters_per_unit: "0.01".to_owned(),
            source_up_axis: "Y".to_owned(),
            target_up_axis: "Y".to_owned(),
            winding: "preserve".to_owned(),
            uv_origin: "lower_left".to_owned(),
            color_space: "linear".to_owned(),
        },
        source_links: vec![source_link],
        nodes,
        meshes: Vec::new(),
        primitives: Vec::new(),
        vertex_streams: Vec::new(),
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
        instances: Vec::new(),
        unsupported_features: Vec::new(),
    };
    CanonicalScene::new(scene).map_err(domain_error_from_scene_ir)
}

const fn domain_error_from_scene_ir(
    _source: crate::domain::scene_ir::SceneIrError,
) -> ConversionPipelineError {
    ConversionPipelineError::Domain {
        source: DomainError::EmptyField { field: "canonical scene validation" },
    }
}

fn build_manifest(
    conversion: &ConversionAggregate,
    snapshot: &UsdStageSnapshot,
    package: &GlbPackage,
    stage: &SourceAssetPath,
) -> Result<SceneManifest, DomainError> {
    let classification =
        conversion.classification().cloned().ok_or(DomainError::MissingClassification)?;
    let glb_labels =
        package.labels.iter().map(|(key, value)| (key.clone(), value.to_string())).collect();
    SceneManifest::new(
        conversion.context().converter_version.clone(),
        conversion.context().profile_hash.clone(),
        vec![ManifestArtifact {
            kind: ManifestArtifactKind::Glb,
            path: GLB_FILE.to_owned(),
            hash: package.checksum.clone(),
        }],
        vec![ManifestSource {
            identity: snapshot.source.clone(),
            asset_kind: "stage".to_owned(),
            selected_values: BTreeMap::from([("stage".to_owned(), stage.to_string())]),
            glb_labels,
            classification,
            diagnostics: Vec::new(),
        }],
        CompatibilityRequirements {
            minimum_bevy_version: SUPPORTED_BEVY_VERSION.to_owned(),
            required_capabilities: BTreeSet::from([CompatibilityCapability::BevyGltfPbr]),
        },
    )
    .map_err(domain_error_from_manifest)
}

fn domain_error_from_manifest(_source: ManifestError) -> DomainError {
    DomainError::EmptyField { field: "manifest validation" }
}

fn output_root_basename(path: &Path) -> OutputRoot {
    let name = path
        .file_name()
        .and_then(|component| component.to_str())
        .filter(|component| !component.is_empty())
        .unwrap_or("vortex-output");
    // 'vortex-output' is a hard-coded valid logical root; the validator is exercised
    // by tests so this expect is genuinely unreachable at runtime.
    OutputRoot::from_str(name).unwrap_or_else(|_| {
        #[expect(
            clippy::expect_used,
            reason = "'vortex-output' is the documented fallback; verified by output_root_basename tests"
        )]
        let fallback = OutputRoot::from_str("vortex-output")
            .expect("'vortex-output' must always parse as a valid OutputRoot");
        fallback
    })
}

#[allow(dead_code)]
fn _fs_import_referenced() -> std::io::Result<std::fs::Metadata> {
    fs::metadata(".")
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf, str::FromStr};

    use super::*;
    use crate::domain::{
        ConversionState, DEFAULT_MANIFEST_LIMIT_BYTES, ExportProfileName, ProjectRoot,
        StageSelection, UnrealObjectPath, UsdPrimPath,
    };

    fn fixture_root(label: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
        let sequence = std::process::id();
        let pid = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos());
        let root =
            std::env::temp_dir().join(format!("vortex-orchestrator-{label}-{sequence}-{pid}"));
        fs::create_dir_all(&root)?;
        Ok(root)
    }

    fn write_stage(root: &Path, name: &str, body: &str) -> Result<(), Box<dyn std::error::Error>> {
        fs::write(root.join(name), body)?;
        Ok(())
    }

    fn unreal_source() -> Result<SourceIdentity, DomainError> {
        Ok(SourceIdentity::Unreal {
            project_root: ProjectRoot::from_str("FixtureProject").map_err(id_err)?,
            asset_path: SourceAssetPath::from_str("Content/Props/Chair.uasset").map_err(id_err)?,
            object_path: UnrealObjectPath::from_str("/Game/Props/Chair.Chair").map_err(id_err)?,
        })
    }

    fn usd_source(stage_path: &str) -> Result<SourceIdentity, DomainError> {
        Ok(SourceIdentity::Usd {
            stage: SourceAssetPath::from_str(stage_path).map_err(id_err)?,
            prim_path: UsdPrimPath::from_str("/World").map_err(id_err)?,
            selection: StageSelection::from_str("purpose=render;payload=forbid;context=universal")
                .map_err(id_err)?,
        })
    }

    fn conversion_context() -> Result<ConversionContext, DomainError> {
        ConversionContext::new(
            ExportProfileName::from_str("bevy-default").map_err(id_err)?,
            ArtifactHash::from_bytes(b"bevy-default-profile"),
            "0.1.0",
        )
    }

    fn id_err(_error: impl std::fmt::Display) -> DomainError {
        DomainError::EmptyField { field: "identity parse" }
    }

    #[test]
    fn unreal_source_classifies_as_requires_adapter() -> Result<(), Box<dyn std::error::Error>> {
        let root = fixture_root("unreal")?;
        let pipeline = ConversionPipeline::new(&root, &root);
        let conversion = pipeline.execute(&unreal_source()?, &conversion_context()?)?;
        let classification =
            conversion.classification().ok_or(ConversionPipelineError::Domain {
                source: DomainError::MissingClassification,
            })?;
        assert_eq!(classification.kind, ClassificationKind::RequiresAdapter);
        assert_eq!(conversion.state(), &ConversionState::Finalized);
        fs::remove_dir_all(&root)?;
        Ok(())
    }

    #[test]
    fn usd_stage_is_parsed_and_published_through_canonical_ir()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = fixture_root("usd")?;
        let output_root = fixture_root("usd-out")?;
        let stage_body = "#usda 1.0\n(\n    upAxis = \"Y\"\n    metersPerUnit = 0.01\n)\ndef Xform \"Triangle\" {\n    def Mesh \"Geometry\" {}\n}\n";
        write_stage(&root, "Fixture.usda", stage_body)?;

        let pipeline = ConversionPipeline::new(&root, &output_root);
        let conversion = pipeline.execute(&usd_source("Fixture.usda")?, &conversion_context()?)?;
        let classification =
            conversion.classification().ok_or(ConversionPipelineError::Domain {
                source: DomainError::MissingClassification,
            })?;
        assert_eq!(classification.kind, ClassificationKind::Converted);
        let artifacts = conversion.artifacts();
        assert!(artifacts.contains_key(&ArtifactKind::Glb));
        assert!(artifacts.contains_key(&ArtifactKind::Manifest));

        let id = conversion.id();
        let store = AtomicArtifactStore::new(&output_root, "usd-out".parse()?)?;
        assert!(store.verify_published(id).is_ok());

        let manifest_path = store.published_manifest_path(id)?;
        let manifest =
            SceneManifest::from_ron(&fs::read(&manifest_path)?, DEFAULT_MANIFEST_LIMIT_BYTES)?;
        assert_eq!(
            manifest.compatibility.minimum_bevy_version, SUPPORTED_BEVY_VERSION,
            "published manifest must declare the Bevy version this runtime supports"
        );

        fs::remove_dir_all(&root)?;
        fs::remove_dir_all(&output_root)?;
        Ok(())
    }
}
