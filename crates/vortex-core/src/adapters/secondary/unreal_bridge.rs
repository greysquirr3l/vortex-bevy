//! Versioned, authorized Unreal Editor bridge protocol handling.
//!
//! The adapter invokes only the checked-in Vortex Python bridge and validates its
//! bounded JSON report. It does not discover project scripts, decrypt assets, or
//! treat a report as semantic conversion output without the matching Editor.

use std::{
    collections::BTreeSet,
    ffi::OsString,
    fs,
    path::{Component, Path, PathBuf},
    process::Command,
};

use serde::Deserialize;
use thiserror::Error;

use crate::domain::{
    ClassificationKind, ConversionClassification, DomainError, ProjectRoot, Remediation,
    SourceAssetPath, SourceIdentity, UnrealObjectPath, identity::IdentityError,
};

const PROTOCOL_VERSION: &str = "vortex.editor-bridge/v1";
const REPORT_FILE_NAME: &str = "vortex-unreal-bridge-report.json";
const MAX_REPORT_BYTES: u64 = 1024 * 1024;
const BRIDGE_SCRIPT_COMPONENTS: [&str; 3] = ["bridges", "unreal", "vortex_unreal_bridge.py"];

/// Typed failures emitted by the isolated Unreal Editor bridge adapter.
#[derive(Debug, Error)]
pub enum UnrealBridgeError {
    /// A configured path cannot be resolved as an existing filesystem object.
    #[error("bridge path is unavailable: {source}")]
    PathUnavailable { source: std::io::Error },
    /// A configured filesystem path had an incompatible type.
    #[error("bridge path has an incompatible type: {path}")]
    InvalidPathType { path: PathBuf },
    /// A path escaped its approved canonical root.
    #[error("bridge path escapes its approved root: {path}")]
    PathEscape { path: PathBuf },
    /// The bridge request did not name any explicit Unreal object paths.
    #[error("bridge request must include at least one explicit Unreal object path")]
    EmptyAssetRequest,
    /// The project file does not have the required Unreal project extension.
    #[error("authorized project file must use the .uproject extension: {path}")]
    InvalidProjectFile { path: PathBuf },
    /// The checked-in bridge script could not be resolved at its fixed repository location.
    #[error("checked-in Unreal bridge script is unavailable: {path}")]
    BridgeScriptUnavailable { path: PathBuf },
    /// The bridge process itself could not be started.
    #[error("could not execute the authorized Unreal bridge: {message}")]
    Process { message: String },
    /// The bridge process returned a nonzero status.
    #[error("authorized Unreal bridge exited with {exit_code:?}: {stderr}")]
    ProcessFailed { exit_code: Option<i32>, stderr: String },
    /// The bridge did not write its required report.
    #[error("authorized Unreal bridge report is unavailable: {source}")]
    ReportUnavailable { source: std::io::Error },
    /// The bridge report exceeds the bounded parser limit.
    #[error("bridge report exceeds bounded size limit: {actual_bytes} bytes")]
    ReportTooLarge { actual_bytes: u64 },
    /// The bridge report could not be decoded as its explicit JSON protocol.
    #[error("bridge report is malformed: {source}")]
    ReportMalformed { source: serde_json::Error },
    /// The report named a different engine family.
    #[error("bridge report engine must be unreal, received {engine}")]
    UnexpectedEngine { engine: String },
    /// The report version does not match the supported bridge protocol.
    #[error("unsupported bridge protocol version: {version}")]
    UnsupportedProtocol { version: String },
    /// A record did not correspond to an explicitly requested source object.
    #[error("bridge report referenced an unrequested Unreal object: {object_path}")]
    UnexpectedSource { object_path: String },
    /// A record used a source object mount that cannot be mapped to a source project asset path.
    #[error("bridge report source is outside the supported /Game mount: {object_path}")]
    UnsupportedObjectMount { object_path: String },
    /// A record did not obey classification and output invariants.
    #[error("bridge report record is inconsistent: {reason}")]
    InconsistentRecord { reason: String },
    /// A report output was not a safe staging-relative path.
    #[error("bridge report output is not staging-relative: {output}")]
    UnsafeOutput { output: String },
    /// A typed source identity could not be constructed from bridge data.
    #[error("bridge report could not create a typed source identity: {source}")]
    Identity { source: IdentityError },
    /// A domain classification could not be constructed from bridge data.
    #[error("bridge report could not create a typed domain value: {source}")]
    Domain { source: DomainError },
}

/// Explicit, canonicalized inputs accepted by the authorized Unreal bridge.
#[derive(Clone, Debug)]
pub struct UnrealBridgeRequest {
    project_root: ProjectRoot,
    project_file: PathBuf,
    editor_executable: PathBuf,
    staging_root: PathBuf,
    asset_paths: BTreeSet<UnrealObjectPath>,
}

impl UnrealBridgeRequest {
    /// Creates a confined request for a matching authorized Unreal Editor project.
    ///
    /// # Errors
    ///
    /// Returns a typed error when paths do not exist, escape the project root, use
    /// an invalid project extension, or the request has no explicit source object.
    pub fn new(
        project_root_path: impl AsRef<Path>,
        project_file: impl AsRef<Path>,
        editor_executable: impl AsRef<Path>,
        staging_root: impl AsRef<Path>,
        asset_paths: impl IntoIterator<Item = UnrealObjectPath>,
    ) -> Result<Self, UnrealBridgeError> {
        let project_root_path = canonical_directory(project_root_path.as_ref())?;
        let project_root = logical_project_root(&project_root_path)?;
        if has_parent_component(project_file.as_ref()) {
            return Err(UnrealBridgeError::PathEscape {
                path: project_file.as_ref().to_path_buf(),
            });
        }
        if has_parent_component(staging_root.as_ref()) {
            return Err(UnrealBridgeError::PathEscape {
                path: staging_root.as_ref().to_path_buf(),
            });
        }
        let project_file = canonical_file(project_file.as_ref())?;
        if !project_file.starts_with(&project_root_path) {
            return Err(UnrealBridgeError::PathEscape { path: project_file });
        }
        let has_uproject_extension = project_file
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("uproject"));
        if !has_uproject_extension {
            return Err(UnrealBridgeError::InvalidProjectFile { path: project_file });
        }
        let editor_executable = canonical_file(editor_executable.as_ref())?;
        let staging_root = canonical_directory(staging_root.as_ref())?;
        let asset_paths = asset_paths.into_iter().collect::<BTreeSet<_>>();
        if asset_paths.is_empty() {
            return Err(UnrealBridgeError::EmptyAssetRequest);
        }
        Ok(Self { project_root, project_file, editor_executable, staging_root, asset_paths })
    }

    /// Returns the canonical staging directory owned by this bridge request.
    #[must_use]
    pub fn staging_root(&self) -> &Path {
        &self.staging_root
    }
}

/// A fully resolved command that can invoke only the checked-in Unreal bridge.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnrealBridgeCommand {
    program: PathBuf,
    arguments: Vec<OsString>,
    report_path: PathBuf,
}

impl UnrealBridgeCommand {
    /// Returns the canonical matching Unreal Editor executable.
    #[must_use]
    pub fn program(&self) -> &Path {
        &self.program
    }

    /// Returns the complete fixed-script argument vector.
    #[must_use]
    pub fn arguments(&self) -> &[OsString] {
        &self.arguments
    }

    /// Returns the report path confined beneath the request staging root.
    #[must_use]
    pub fn report_path(&self) -> &Path {
        &self.report_path
    }
}

/// A testable result from one bridge process invocation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnrealBridgeProcessOutput {
    /// The child exit status, if the operating system reported one.
    pub exit_code: Option<i32>,
    /// Stderr preserved for an actionable operator error.
    pub stderr: String,
}

/// Consumer-owned process port used by the Unreal bridge runner.
pub trait UnrealBridgeProcess {
    /// Adapter-specific process failure.
    type Error: std::error::Error + Send + Sync + 'static;

    /// Runs exactly one prepared fixed-script bridge command.
    ///
    /// # Errors
    ///
    /// Returns the concrete process adapter error when the command cannot be
    /// executed or its output cannot be collected.
    fn execute(
        &self,
        command: &UnrealBridgeCommand,
    ) -> Result<UnrealBridgeProcessOutput, Self::Error>;
}

/// Native subprocess implementation for a configured matching Unreal Editor.
#[derive(Clone, Copy, Debug, Default)]
pub struct NativeUnrealBridgeProcess;

impl UnrealBridgeProcess for NativeUnrealBridgeProcess {
    type Error = std::io::Error;

    fn execute(
        &self,
        command: &UnrealBridgeCommand,
    ) -> Result<UnrealBridgeProcessOutput, Self::Error> {
        let output = Command::new(command.program()).args(command.arguments()).output()?;
        Ok(UnrealBridgeProcessOutput {
            exit_code: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

/// One typed, source-traceable record emitted by the Unreal bridge report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnrealBridgeRecord {
    /// The requested Unreal object represented by the record.
    pub source: SourceIdentity,
    /// Unreal’s reported object class.
    pub asset_kind: String,
    /// The explicit exporter selected by the authorized Editor bridge.
    pub exporter: String,
    /// Staging-relative output files, in lexical order.
    pub outputs: Vec<PathBuf>,
    /// Editor warnings retained without interpretation.
    pub warnings: Vec<String>,
    /// Editor errors retained without interpretation.
    pub errors: Vec<String>,
    /// Explicit typed conversion eligibility.
    pub classification: ConversionClassification,
}

/// Parsed report from one successful authorized Unreal bridge process.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnrealBridgeReport {
    /// Typed records in stable source-object order.
    pub records: Vec<UnrealBridgeRecord>,
}

/// Fixed-script adapter for the checked-in authorized Unreal bridge.
#[derive(Clone, Debug)]
pub struct UnrealBridge {
    script_path: PathBuf,
}

impl UnrealBridge {
    /// Resolves the only approved Python bridge script beneath a repository root.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the repository root or fixed bridge script is
    /// unavailable or not a regular file.
    pub fn from_repository_root(
        repository_root: impl AsRef<Path>,
    ) -> Result<Self, UnrealBridgeError> {
        let repository_root = canonical_directory(repository_root.as_ref())?;
        let script_path = BRIDGE_SCRIPT_COMPONENTS
            .iter()
            .fold(repository_root.clone(), |path, component| path.join(component));
        let script_path = fs::canonicalize(&script_path).map_err(|_| {
            UnrealBridgeError::BridgeScriptUnavailable { path: script_path.clone() }
        })?;
        if !script_path.starts_with(&repository_root) || !script_path.is_file() {
            return Err(UnrealBridgeError::BridgeScriptUnavailable { path: script_path });
        }
        Ok(Self { script_path })
    }

    /// Builds a deterministic fixed-script command without starting an Editor.
    ///
    /// # Errors
    ///
    /// Propagates invalid request state and rejects a report path that would escape
    /// the request’s canonical staging directory.
    pub fn build_command(
        &self,
        request: &UnrealBridgeRequest,
    ) -> Result<UnrealBridgeCommand, UnrealBridgeError> {
        let report_path = request.staging_root.join(REPORT_FILE_NAME);
        if !report_path.starts_with(&request.staging_root) {
            return Err(UnrealBridgeError::PathEscape { path: report_path });
        }
        let asset_paths =
            request.asset_paths.iter().map(UnrealObjectPath::as_str).collect::<Vec<_>>().join(",");
        let arguments = vec![
            request.project_file.clone().into_os_string(),
            OsString::from("-unattended"),
            OsString::from("-nop4"),
            OsString::from(format!("-script={}", self.script_path.display())),
            OsString::from("--"),
            OsString::from("--assets"),
            OsString::from(asset_paths),
            OsString::from("--staging-root"),
            request.staging_root.clone().into_os_string(),
            OsString::from("--report"),
            report_path.clone().into_os_string(),
        ];
        Ok(UnrealBridgeCommand {
            program: request.editor_executable.clone(),
            arguments,
            report_path,
        })
    }

    /// Invokes the bridge through a consumer-owned process port and parses its report.
    ///
    /// # Errors
    ///
    /// Returns a typed error before report parsing when command construction or the
    /// process outcome fails, and a typed protocol error for invalid report data.
    pub fn invoke<Process>(
        &self,
        request: &UnrealBridgeRequest,
        process: &Process,
    ) -> Result<UnrealBridgeReport, UnrealBridgeError>
    where
        Process: UnrealBridgeProcess,
    {
        let command = self.build_command(request)?;
        let output = process
            .execute(&command)
            .map_err(|source| UnrealBridgeError::Process { message: source.to_string() })?;
        if output.exit_code != Some(0) {
            return Err(UnrealBridgeError::ProcessFailed {
                exit_code: output.exit_code,
                stderr: output.stderr,
            });
        }
        self.parse_report_file(request, &command)
    }

    /// Invokes a configured matching Unreal Editor directly.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`Self::invoke`] for command, process, and report
    /// failures.
    pub fn invoke_native(
        &self,
        request: &UnrealBridgeRequest,
    ) -> Result<UnrealBridgeReport, UnrealBridgeError> {
        self.invoke(request, &NativeUnrealBridgeProcess)
    }

    /// Parses report text for a previously validated request without launching an Editor.
    ///
    /// # Errors
    ///
    /// Returns typed protocol and confinement errors for malformed, incompatible, or
    /// unsafe report content.
    pub fn parse_report_text(
        &self,
        request: &UnrealBridgeRequest,
        text: &str,
    ) -> Result<UnrealBridgeReport, UnrealBridgeError> {
        let raw: RawBridgeReport = serde_json::from_str(text)
            .map_err(|source| UnrealBridgeError::ReportMalformed { source })?;
        if raw.engine != "unreal" {
            return Err(UnrealBridgeError::UnexpectedEngine { engine: raw.engine });
        }
        if raw.protocol_version != PROTOCOL_VERSION {
            return Err(UnrealBridgeError::UnsupportedProtocol { version: raw.protocol_version });
        }

        let mut records = raw
            .records
            .into_iter()
            .map(|record| Self::map_record(request, record))
            .collect::<Result<Vec<_>, _>>()?;
        records.sort_by_key(|record| record.source.to_string());
        let unique_sources =
            records.iter().map(|record| record.source.to_string()).collect::<BTreeSet<_>>();
        if unique_sources.len() != records.len() {
            return Err(UnrealBridgeError::InconsistentRecord {
                reason: "bridge report contains duplicate source identities".to_owned(),
            });
        }
        Ok(UnrealBridgeReport { records })
    }

    fn parse_report_file(
        &self,
        request: &UnrealBridgeRequest,
        command: &UnrealBridgeCommand,
    ) -> Result<UnrealBridgeReport, UnrealBridgeError> {
        let metadata = fs::metadata(command.report_path())
            .map_err(|source| UnrealBridgeError::ReportUnavailable { source })?;
        if metadata.len() > MAX_REPORT_BYTES {
            return Err(UnrealBridgeError::ReportTooLarge { actual_bytes: metadata.len() });
        }
        let text = fs::read_to_string(command.report_path())
            .map_err(|source| UnrealBridgeError::ReportUnavailable { source })?;
        self.parse_report_text(request, &text)
    }

    fn map_record(
        request: &UnrealBridgeRequest,
        raw: RawBridgeRecord,
    ) -> Result<UnrealBridgeRecord, UnrealBridgeError> {
        if raw.source_identity.engine != "unreal" {
            return Err(UnrealBridgeError::UnexpectedEngine { engine: raw.source_identity.engine });
        }
        let object_path = raw
            .source_identity
            .id
            .parse::<UnrealObjectPath>()
            .map_err(|source| UnrealBridgeError::Identity { source })?;
        if !request.asset_paths.contains(&object_path) {
            return Err(UnrealBridgeError::UnexpectedSource {
                object_path: object_path.as_str().to_owned(),
            });
        }
        let asset_path = asset_path_for_object(&object_path)?;
        let source = SourceIdentity::Unreal {
            project_root: request.project_root.clone(),
            asset_path,
            object_path,
        };
        let outputs = raw
            .outputs
            .iter()
            .map(|output| confined_output(&request.staging_root, output))
            .collect::<Result<Vec<_>, _>>()?;
        let converted =
            matches!(raw.classification, RawClassification::Converted | RawClassification::Baked);
        if converted && outputs.is_empty() {
            return Err(UnrealBridgeError::InconsistentRecord {
                reason: "a converted or baked bridge record must contain an output".to_owned(),
            });
        }
        if !converted && !outputs.is_empty() {
            return Err(UnrealBridgeError::InconsistentRecord {
                reason: "a non-exportable bridge record must not contain outputs".to_owned(),
            });
        }
        if raw.asset_kind.trim().is_empty() || raw.exporter.trim().is_empty() {
            return Err(UnrealBridgeError::InconsistentRecord {
                reason: "asset kind and exporter must be nonempty".to_owned(),
            });
        }
        let classification = classification_for_record(&raw)?;
        Ok(UnrealBridgeRecord {
            source,
            asset_kind: raw.asset_kind,
            exporter: raw.exporter,
            outputs,
            warnings: sorted_strings(raw.warnings),
            errors: sorted_strings(raw.errors),
            classification,
        })
    }
}

fn has_parent_component(path: &Path) -> bool {
    path.components().any(|component| matches!(component, Component::ParentDir))
}

fn canonical_directory(path: &Path) -> Result<PathBuf, UnrealBridgeError> {
    let path =
        fs::canonicalize(path).map_err(|source| UnrealBridgeError::PathUnavailable { source })?;
    if !path.is_dir() {
        return Err(UnrealBridgeError::InvalidPathType { path });
    }
    Ok(path)
}

fn canonical_file(path: &Path) -> Result<PathBuf, UnrealBridgeError> {
    let path =
        fs::canonicalize(path).map_err(|source| UnrealBridgeError::PathUnavailable { source })?;
    if !path.is_file() {
        return Err(UnrealBridgeError::InvalidPathType { path });
    }
    Ok(path)
}

fn logical_project_root(path: &Path) -> Result<ProjectRoot, UnrealBridgeError> {
    let name = path
        .file_name()
        .and_then(|component| component.to_str())
        .ok_or_else(|| UnrealBridgeError::InvalidPathType { path: path.to_path_buf() })?;
    name.parse::<ProjectRoot>().map_err(|source| UnrealBridgeError::Identity { source })
}

fn asset_path_for_object(
    object_path: &UnrealObjectPath,
) -> Result<SourceAssetPath, UnrealBridgeError> {
    let object = object_path.as_str();
    let relative = object.strip_prefix("/Game/").ok_or_else(|| {
        UnrealBridgeError::UnsupportedObjectMount { object_path: object.to_owned() }
    })?;
    let package = relative.split_once('.').map_or(relative, |(package, _)| package);
    let asset_path = format!("Content/{package}.uasset");
    asset_path.parse::<SourceAssetPath>().map_err(|source| UnrealBridgeError::Identity { source })
}

fn confined_output(staging_root: &Path, output: &str) -> Result<PathBuf, UnrealBridgeError> {
    let output_path = Path::new(output);
    let safe = !output.is_empty()
        && !output.contains('\\')
        && !output_path.is_absolute()
        && output_path.components().all(|component| matches!(component, Component::Normal(_)));
    if !safe {
        return Err(UnrealBridgeError::UnsafeOutput { output: output.to_owned() });
    }
    let full_path = staging_root.join(output_path);
    if !full_path.starts_with(staging_root) {
        return Err(UnrealBridgeError::UnsafeOutput { output: output.to_owned() });
    }
    Ok(output_path.to_path_buf())
}

fn classification_for_record(
    raw: &RawBridgeRecord,
) -> Result<ConversionClassification, UnrealBridgeError> {
    let kind = match raw.classification {
        RawClassification::Converted => ClassificationKind::Converted,
        RawClassification::Baked => ClassificationKind::Baked,
        RawClassification::RequiresAdapter => ClassificationKind::RequiresAdapter,
        RawClassification::Unsupported => ClassificationKind::Unsupported,
        RawClassification::Rejected => ClassificationKind::Rejected,
    };
    let reason = raw.errors.first().or_else(|| raw.warnings.first()).map_or_else(
        || format!("Unreal Editor bridge classified {} as {:?}.", raw.asset_kind, kind),
        Clone::clone,
    );
    let remediation = match kind {
        ClassificationKind::Converted | ClassificationKind::Baked => {
            "Validate the staged output before packaging it into a Vortex artifact."
        }
        ClassificationKind::RequiresAdapter => {
            "Use a dedicated adapter or bake the non-portable Unreal feature into an approved interchange form."
        }
        ClassificationKind::Unsupported => {
            "Select a portable source asset or provide an approved engine-specific adapter."
        }
        ClassificationKind::Rejected => {
            "Repair the authorized source or inspect the matching Unreal Editor diagnostics."
        }
    };
    ConversionClassification::new(
        kind,
        reason,
        Remediation::new(remediation).map_err(|source| UnrealBridgeError::Domain { source })?,
    )
    .map_err(|source| UnrealBridgeError::Domain { source })
}

fn sorted_strings(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values.dedup();
    values
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

#[derive(Clone, Copy, Debug, Deserialize)]
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
