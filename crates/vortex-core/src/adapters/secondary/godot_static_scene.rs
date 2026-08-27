//! Typed boundary for the controlled Godot 4.7 static-scene bridge report.
//!
//! This adapter accepts only the three in-memory source IDs produced by the checked-in
//! bridge. It does not discover project resources or treat arbitrary Godot scene
//! serialization as a portable scene conversion.

use std::{
    collections::BTreeMap,
    fs,
    path::{Component, Path, PathBuf},
};

use serde::Deserialize;
use thiserror::Error;

use crate::domain::{
    ClassificationKind, ConversionClassification, DomainError, ProjectRoot, Remediation,
    SourceIdentity,
    identity::{GodotResourcePath, IdentityError},
};

const PROTOCOL_VERSION: &str = "vortex.editor-bridge/v1";
const MAX_REPORT_BYTES: u64 = 1024 * 1024;
const SCENE_SOURCE_ID: &str = "godot://temporary/VortexStaticScene";
const TEXTURE_SOURCE_ID: &str = "godot://temporary/VortexStaticScene/BaseColor";
const PARTICLE_SOURCE_ID: &str = "godot://temporary/VortexStaticScene/UnsupportedParticles";
const SCENE_OUTPUT: &str = "exports/VortexStaticScene.glb";
const TEXTURE_OUTPUT: &str = "textures/vortex-static-base-color.png";

/// Typed failures emitted by the controlled Godot static-scene report adapter.
#[derive(Debug, Error)]
pub enum GodotStaticSceneError {
    /// The report file could not be read.
    #[error("Godot static-scene report is unavailable: {source}")]
    ReportUnavailable { source: std::io::Error },
    /// The report file exceeds the strict decoding boundary.
    #[error("Godot static-scene report exceeds bounded size limit: {actual_bytes} bytes")]
    ReportTooLarge { actual_bytes: u64 },
    /// The report does not conform to the supported JSON shape.
    #[error("Godot static-scene report is malformed: {source}")]
    ReportMalformed { source: serde_json::Error },
    /// The report named a different engine family.
    #[error("Godot static-scene report engine must be godot, received {engine}")]
    UnexpectedEngine { engine: String },
    /// The report protocol version is not supported by this adapter.
    #[error("unsupported Godot static-scene protocol version: {version}")]
    UnsupportedProtocol { version: String },
    /// A report record did not belong to the exact controlled scene scope.
    #[error("Godot static-scene report referenced an unexpected source: {source_id}")]
    UnexpectedSource { source_id: String },
    /// A required controlled source was absent from the report.
    #[error("Godot static-scene report omitted required source: {source_id}")]
    MissingSource { source_id: &'static str },
    /// A reported output was not a safe path relative to the controlled staging root.
    #[error("Godot static-scene report output is not staging-relative: {output}")]
    UnsafeOutput { output: String },
    /// A controlled source did not match its fixed class, exporter, and output contract.
    #[error("Godot static-scene report record is inconsistent: {reason}")]
    InconsistentRecord { reason: String },
    /// A typed Godot source identity could not be constructed.
    #[error("Godot static-scene report could not create a typed source identity: {source}")]
    Identity { source: IdentityError },
    /// A typed domain classification could not be constructed.
    #[error("Godot static-scene report could not create a typed domain value: {source}")]
    Domain { source: DomainError },
}

/// Controlled source and staging facts extracted from a validated Godot report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GodotStaticSceneReport {
    /// The generated scene source identity.
    pub scene_source: SourceIdentity,
    /// The generated base-color texture source identity.
    pub texture_source: SourceIdentity,
    /// The generated particle marker source identity.
    pub particle_source: SourceIdentity,
    /// Confined path to the direct GLB export.
    pub glb_output: PathBuf,
    /// Confined path to the generated PNG export.
    pub texture_output: PathBuf,
    /// Explicit particle conversion boundary retained from the Editor report.
    pub particle_classification: ConversionClassification,
}

/// Parser for only the checked-in Godot static-scene protocol shape.
#[derive(Clone, Copy, Debug, Default)]
pub struct GodotStaticSceneAdapter;

impl GodotStaticSceneAdapter {
    /// Parses a size-bounded report file under a caller-owned staging root.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the report is unreadable, exceeds the input limit,
    /// is malformed, or does not match the exact controlled bridge contract.
    pub fn parse_report_file(
        self,
        project_root: &ProjectRoot,
        staging_root: &Path,
        report_path: &Path,
    ) -> Result<GodotStaticSceneReport, GodotStaticSceneError> {
        let metadata = fs::metadata(report_path)
            .map_err(|source| GodotStaticSceneError::ReportUnavailable { source })?;
        if metadata.len() > MAX_REPORT_BYTES {
            return Err(GodotStaticSceneError::ReportTooLarge { actual_bytes: metadata.len() });
        }
        let text = fs::read_to_string(report_path)
            .map_err(|source| GodotStaticSceneError::ReportUnavailable { source })?;
        self.parse_report_text(project_root, staging_root, &text)
    }

    /// Parses report text after the caller has established its source boundary.
    ///
    /// # Errors
    ///
    /// Returns a typed protocol or confinement error for incompatible report records.
    pub fn parse_report_text(
        self,
        project_root: &ProjectRoot,
        staging_root: &Path,
        text: &str,
    ) -> Result<GodotStaticSceneReport, GodotStaticSceneError> {
        let raw: RawBridgeReport = serde_json::from_str(text)
            .map_err(|source| GodotStaticSceneError::ReportMalformed { source })?;
        if raw.engine != "godot" {
            return Err(GodotStaticSceneError::UnexpectedEngine { engine: raw.engine });
        }
        if raw.protocol_version != PROTOCOL_VERSION {
            return Err(GodotStaticSceneError::UnsupportedProtocol {
                version: raw.protocol_version,
            });
        }

        let mut records = BTreeMap::new();
        for record in raw.records {
            let source = record.source_identity.id.clone();
            if !matches!(source.as_str(), SCENE_SOURCE_ID | TEXTURE_SOURCE_ID | PARTICLE_SOURCE_ID)
            {
                return Err(GodotStaticSceneError::UnexpectedSource { source_id: source });
            }
            if record.source_identity.engine != "godot" {
                return Err(GodotStaticSceneError::UnexpectedEngine {
                    engine: record.source_identity.engine,
                });
            }
            if records.insert(source.clone(), record).is_some() {
                return Err(GodotStaticSceneError::InconsistentRecord {
                    reason: format!("duplicate controlled source record: {source}"),
                });
            }
        }

        let scene = required_record(&mut records, SCENE_SOURCE_ID)?;
        let texture = required_record(&mut records, TEXTURE_SOURCE_ID)?;
        let particle = required_record(&mut records, PARTICLE_SOURCE_ID)?;
        if !records.is_empty() {
            return Err(GodotStaticSceneError::InconsistentRecord {
                reason: "report retained records outside the controlled source scope".to_owned(),
            });
        }

        let scene_source = typed_source(project_root, SCENE_SOURCE_ID)?;
        let texture_source = typed_source(project_root, TEXTURE_SOURCE_ID)?;
        let particle_source = typed_source(project_root, PARTICLE_SOURCE_ID)?;
        validate_converted(&scene, "Node3D", "godot.gltf-document", SCENE_OUTPUT, staging_root)?;
        validate_converted(
            &texture,
            "ImageTexture",
            "godot.generated-png",
            TEXTURE_OUTPUT,
            staging_root,
        )?;
        let particle_classification = validate_particle(&particle)?;

        Ok(GodotStaticSceneReport {
            scene_source,
            texture_source,
            particle_source,
            glb_output: confined_output(staging_root, SCENE_OUTPUT)?,
            texture_output: confined_output(staging_root, TEXTURE_OUTPUT)?,
            particle_classification,
        })
    }
}

fn required_record(
    records: &mut BTreeMap<String, RawBridgeRecord>,
    source_id: &'static str,
) -> Result<RawBridgeRecord, GodotStaticSceneError> {
    records.remove(source_id).ok_or(GodotStaticSceneError::MissingSource { source_id })
}

fn typed_source(
    project_root: &ProjectRoot,
    source_id: &str,
) -> Result<SourceIdentity, GodotStaticSceneError> {
    let resource_path = source_id
        .parse::<GodotResourcePath>()
        .map_err(|source| GodotStaticSceneError::Identity { source })?;
    Ok(SourceIdentity::Godot { project_root: project_root.clone(), resource_path })
}

fn validate_converted(
    record: &RawBridgeRecord,
    asset_kind: &str,
    exporter: &str,
    output: &str,
    staging_root: &Path,
) -> Result<(), GodotStaticSceneError> {
    if record.asset_kind != asset_kind
        || record.exporter != exporter
        || record.classification != RawClassification::Converted
        || record.outputs != [output]
        || !record.errors.is_empty()
    {
        return Err(GodotStaticSceneError::InconsistentRecord {
            reason: format!("expected a converted {asset_kind} record with output {output}"),
        });
    }
    let _ = confined_output(staging_root, output)?;
    Ok(())
}

fn validate_particle(
    record: &RawBridgeRecord,
) -> Result<ConversionClassification, GodotStaticSceneError> {
    if record.asset_kind != "GPUParticles3D"
        || record.exporter != "none"
        || record.classification != RawClassification::RequiresAdapter
        || !record.outputs.is_empty()
        || !record.errors.is_empty()
        || record.warnings.is_empty()
    {
        return Err(GodotStaticSceneError::InconsistentRecord {
            reason: "expected an explicit unsupported GPUParticles3D record".to_owned(),
        });
    }
    ConversionClassification::new(
        ClassificationKind::RequiresAdapter,
        record.warnings.iter().min().cloned().ok_or_else(|| {
            GodotStaticSceneError::InconsistentRecord {
                reason: "particle boundary is missing an actionable warning".to_owned(),
            }
        })?,
        Remediation::new(
            "Bake the particle effect into an approved portable interchange artifact or add a dedicated Godot particle adapter.",
        )
        .map_err(|source| GodotStaticSceneError::Domain { source })?,
    )
    .map_err(|source| GodotStaticSceneError::Domain { source })
}

fn confined_output(staging_root: &Path, output: &str) -> Result<PathBuf, GodotStaticSceneError> {
    let output_path = Path::new(output);
    let safe = !output.is_empty()
        && !output.contains('\\')
        && !output_path.is_absolute()
        && output_path.components().all(|component| matches!(component, Component::Normal(_)));
    if !safe || !staging_root.join(output_path).starts_with(staging_root) {
        return Err(GodotStaticSceneError::UnsafeOutput { output: output.to_owned() });
    }
    Ok(output_path.to_path_buf())
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBridgeReport {
    engine: String,
    protocol_version: String,
    records: Vec<RawBridgeRecord>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBridgeRecord {
    asset_kind: String,
    classification: RawClassification,
    errors: Vec<String>,
    exporter: String,
    outputs: Vec<String>,
    source_identity: RawSourceIdentity,
    warnings: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
enum RawClassification {
    Converted,
    Baked,
    RequiresAdapter,
    Unsupported,
    Rejected,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSourceIdentity {
    engine: String,
    id: String,
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::GodotStaticSceneAdapter;
    use crate::domain::ProjectRoot;

    #[test]
    fn controlled_report_is_typed_and_rejects_extra_sources()
    -> Result<(), Box<dyn std::error::Error>> {
        let report = r#"{
          "engine":"godot",
          "protocol_version":"vortex.editor-bridge/v1",
          "records":[
            {"asset_kind":"Node3D","classification":"converted","errors":[],"exporter":"godot.gltf-document","outputs":["exports/VortexStaticScene.glb"],"source_identity":{"engine":"godot","id":"godot://temporary/VortexStaticScene"},"warnings":[]},
            {"asset_kind":"ImageTexture","classification":"converted","errors":[],"exporter":"godot.generated-png","outputs":["textures/vortex-static-base-color.png"],"source_identity":{"engine":"godot","id":"godot://temporary/VortexStaticScene/BaseColor"},"warnings":[]},
            {"asset_kind":"GPUParticles3D","classification":"requires_adapter","errors":[],"exporter":"none","outputs":[],"source_identity":{"engine":"godot","id":"godot://temporary/VortexStaticScene/UnsupportedParticles"},"warnings":["particle boundary"]}
          ]
        }"#;
        let project_root: ProjectRoot = "Test123".parse()?;
        let adapter = GodotStaticSceneAdapter;
        let parsed = adapter.parse_report_text(&project_root, Path::new("/tmp/staging"), report)?;
        assert_eq!(parsed.glb_output, Path::new("exports/VortexStaticScene.glb"));

        let unsafe_report = report.replace(
            "godot://temporary/VortexStaticScene/UnsupportedParticles",
            "res://outside.tscn",
        );
        assert!(
            adapter
                .parse_report_text(&project_root, Path::new("/tmp/staging"), &unsafe_report)
                .is_err()
        );
        Ok(())
    }
}
