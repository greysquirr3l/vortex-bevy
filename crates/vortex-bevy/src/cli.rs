//! Typed command-line boundary for Vortex-Bevy workflows.
//!
//! Clap parses external input into validated domain value objects before dispatch.
//! Commands whose downstream recovery task is not yet implemented report an explicit
//! unavailable capability rather than guessing at a conversion result.

use std::{ffi::OsString, path::PathBuf, process::ExitCode};

use anyhow::{Context, Result, anyhow};
use clap::{Args, Parser, Subcommand, ValueEnum};
use serde::Serialize;
use vortex_core::{
    adapters::secondary::unreal_package::UnrealPackageInspector,
    application::{
        command::inspect_input::{InspectInput, handle as inspect_input},
        query::support_matrix::{
            SupportMatrixEntry as CoreSupportMatrixEntry, SupportMatrixReader,
            handle as read_support_matrix,
        },
    },
    domain::{
        ClassificationKind, DomainError, ExportProfileName, ProjectRoot, Remediation,
        SourceAssetPath, SourceIdentity, StageSelection, UnrealObjectPath,
    },
};

const EXIT_SUCCESS: u8 = 0;
const EXIT_USAGE: u8 = 2;
const EXIT_UNAVAILABLE: u8 = 3;
const EXIT_INSPECTION_FAILURE: u8 = 4;

/// Runs the typed CLI using the process environment and returns a stable process code.
#[must_use]
pub fn run_from_environment() -> ExitCode {
    match Cli::try_parse() {
        Ok(cli) => {
            let output_mode = cli.output_mode();
            match run(cli) {
                Ok(report) => {
                    print_report(&report, output_mode);
                    ExitCode::from(EXIT_SUCCESS)
                }
                Err(error) => {
                    eprintln!("vortex-bevy: {error:#}");
                    ExitCode::from(exit_code_for(&error))
                }
            }
        }
        Err(error) => {
            let _ignored = error.print();
            let code = u8::try_from(error.exit_code()).map_or(EXIT_USAGE, |code| code);
            ExitCode::from(code)
        }
    }
}

/// Runs the typed CLI from supplied arguments for integration tests and embedding.
///
/// # Errors
///
/// Returns a contextual error when argument parsing or command execution fails.
pub fn run_from<I, T>(arguments: I) -> Result<CliReport>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    parse_and_dispatch(arguments, run)
}

#[derive(Clone, Debug, Parser)]
#[command(name = "vortex-bevy", version, about = "Authorized asset conversion for Bevy")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

impl Cli {
    const fn output_mode(&self) -> OutputMode {
        match &self.command {
            Command::Inspect(arguments) => arguments.output.format,
            Command::Plan(arguments) | Command::Convert(arguments) => arguments.report.format,
            Command::Verify(arguments) => arguments.output.format,
            Command::View(arguments) => arguments.output.format,
            Command::SupportMatrix(arguments) | Command::Version(arguments) => arguments.format,
        }
    }
}

#[derive(Clone, Debug, Subcommand)]
enum Command {
    /// Inspect one explicit authorized source without conversion.
    Inspect(InspectArgs),
    /// Plan a conversion without writing artifacts.
    Plan(ConversionArgs),
    /// Convert an explicit authorized source into owned artifacts.
    Convert(ConversionArgs),
    /// Verify artifacts for a conversion identifier.
    Verify(ConversionIdArgs),
    /// Open a Bevy viewer for verified artifacts.
    View(ViewArgs),
    /// Print the tested support classifications.
    SupportMatrix(OutputArgs),
    /// Print the product version in the selected output mode.
    Version(OutputArgs),
}

#[derive(Clone, Debug, Args)]
struct InspectArgs {
    /// Authorized source-project root used for filesystem confinement.
    #[arg(long)]
    project: PathBuf,
    /// Engine family that owns the selected source.
    #[arg(long, value_enum)]
    engine: Engine,
    /// Project-relative source asset path.
    #[arg(long, value_parser = parse_asset_path)]
    asset: SourceAssetPath,
    /// Unreal object path identifying the selected asset inside a `.uasset`.
    #[arg(long, value_parser = parse_unreal_object_path)]
    object: Option<UnrealObjectPath>,
    #[command(flatten)]
    output: OutputArgs,
}

#[derive(Clone, Debug, Args)]
struct ConversionArgs {
    /// Authorized source-project root used for filesystem confinement.
    #[arg(long)]
    project: PathBuf,
    /// Engine family that owns the selected source.
    #[arg(long, value_enum)]
    engine: Engine,
    /// Project-relative source asset path.
    #[arg(long, value_parser = parse_asset_path)]
    asset: SourceAssetPath,
    /// Named deterministic conversion profile.
    #[arg(long, value_parser = parse_profile_name)]
    profile: ExportProfileName,
    /// Output root owned by Vortex-Bevy.
    #[arg(long)]
    output: PathBuf,
    /// Optional selected USD variant expression.
    #[arg(long, value_parser = parse_stage_selection)]
    variant: Option<StageSelection>,
    /// Explicit payload policy for supported future USD composition modes.
    #[arg(long, value_enum, default_value_t = PayloadPolicy::Forbid)]
    payload_policy: PayloadPolicy,
    /// Compute the conversion plan without publishing artifacts.
    #[arg(long)]
    dry_run: bool,
    /// Permit replacement only inside the configured Vortex output root.
    #[arg(long)]
    overwrite: bool,
    #[command(flatten)]
    report: OutputArgs,
}

#[derive(Clone, Debug, Args)]
struct ConversionIdArgs {
    /// Stable conversion identifier returned by a prior conversion report.
    #[arg(long)]
    conversion_id: String,
    #[command(flatten)]
    output: OutputArgs,
}

#[derive(Clone, Debug, Args)]
struct ViewArgs {
    /// Directory containing a verified scene manifest and GLB artifact.
    #[arg(long)]
    artifact_root: PathBuf,
    #[command(flatten)]
    output: OutputArgs,
}

#[derive(Clone, Debug, Args)]
struct OutputArgs {
    /// Emit a stable JSON report rather than human-readable text.
    #[arg(long, value_enum, default_value_t = OutputMode::Text)]
    format: OutputMode,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum Engine {
    Unreal,
    Usd,
    Unity,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum OutputMode {
    Text,
    Json,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum PayloadPolicy {
    Forbid,
    MetadataOnly,
}

/// Stable, serializable result emitted by the recovered CLI boundary.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CliReport {
    /// A metadata-only source inspection completed successfully.
    Inspection {
        source: String,
        classification: String,
        reason: String,
        remediation: String,
        facts: Vec<String>,
    },
    /// A command was parsed safely but its downstream recovery task is unavailable.
    Unavailable { command: String, capability: String, remediation: String },
    /// A static support-matrix row represents a tested boundary.
    SupportMatrix { entries: Vec<SupportMatrixReportEntry> },
    /// Product version details.
    Version { version: String },
}

/// Machine-readable support-matrix entry emitted by [`CliReport::SupportMatrix`].
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SupportMatrixReportEntry {
    capability: String,
    classification: String,
    remediation: String,
}

fn parse_and_dispatch<I, T, Dispatch>(arguments: I, dispatch: Dispatch) -> Result<CliReport>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
    Dispatch: FnOnce(Cli) -> Result<CliReport>,
{
    let cli = Cli::try_parse_from(arguments).context("invalid Vortex-Bevy command line")?;
    dispatch(cli)
}

fn run(cli: Cli) -> Result<CliReport> {
    match cli.command {
        Command::Inspect(arguments) => inspect(arguments),
        Command::SupportMatrix(arguments) => support_matrix(arguments.format),
        Command::Version(arguments) => Ok(version_report(arguments.format)),
        Command::Plan(arguments) => unavailable_conversion("plan", &arguments),
        Command::Convert(arguments) => unavailable_conversion("convert", &arguments),
        Command::Verify(arguments) => unavailable_identifier_command("verify", &arguments),
        Command::View(arguments) => unavailable_view(&arguments),
    }
}

fn inspect(arguments: InspectArgs) -> Result<CliReport> {
    if arguments.engine != Engine::Unreal {
        return Err(anyhow!(
            "{} inspection is unavailable until its recovered source adapter is implemented",
            engine_label(arguments.engine)
        ));
    }
    let object_path = arguments
        .object
        .ok_or_else(|| anyhow!("--object is required for Unreal metadata inspection"))?;
    let source = SourceIdentity::Unreal {
        project_root: logical_project_root(&arguments.project)?,
        asset_path: arguments.asset,
        object_path,
    };
    let inspector = UnrealPackageInspector::new(&arguments.project).with_context(|| {
        format!(
            "could not establish authorized Unreal project root {}",
            arguments.project.display()
        )
    })?;
    let inspection = inspect_input(InspectInput { source }, &inspector)
        .context("Unreal metadata inspection failed")?;
    Ok(CliReport::Inspection {
        source: inspection.source.to_string(),
        classification: classification_label(&inspection.classification.kind).to_owned(),
        reason: inspection.classification.reason,
        remediation: inspection.classification.remediation.as_str().to_owned(),
        facts: inspection.facts,
    })
}

fn unavailable_conversion(command: &str, arguments: &ConversionArgs) -> Result<CliReport> {
    validate_conversion_arguments(arguments)?;
    Ok(CliReport::Unavailable {
        command: command.to_owned(),
        capability: "conversion orchestration".to_owned(),
        remediation: "Recovery must complete canonical IR, packaging, and publication before this command can execute.".to_owned(),
    })
}

fn unavailable_identifier_command(
    command: &str,
    arguments: &ConversionIdArgs,
) -> Result<CliReport> {
    if arguments.conversion_id.trim().is_empty() {
        return Err(anyhow!("--conversion-id cannot be empty"));
    }
    Ok(CliReport::Unavailable {
        command: command.to_owned(),
        capability: "artifact publication and verification".to_owned(),
        remediation:
            "Recovery must complete atomic publication before artifact verification can execute."
                .to_owned(),
    })
}

fn unavailable_view(arguments: &ViewArgs) -> Result<CliReport> {
    if arguments.artifact_root.as_os_str().is_empty() {
        return Err(anyhow!("--artifact-root cannot be empty"));
    }
    Ok(CliReport::Unavailable {
        command: "view".to_owned(),
        capability: "artifact discovery".to_owned(),
        remediation:
            "Pass a verified published artifact root after recovery completes publication."
                .to_owned(),
    })
}

fn validate_conversion_arguments(arguments: &ConversionArgs) -> Result<()> {
    if arguments.output.as_os_str().is_empty() {
        return Err(anyhow!("--output cannot be empty"));
    }
    if arguments.engine == Engine::Unreal && arguments.variant.is_some() {
        return Err(anyhow!("--variant is only meaningful for USD stage inputs"));
    }
    if arguments.payload_policy != PayloadPolicy::Forbid && arguments.engine != Engine::Usd {
        return Err(anyhow!("--payload-policy is only meaningful for USD stage inputs"));
    }
    Ok(())
}

struct StaticSupportMatrixReader;

impl SupportMatrixReader for StaticSupportMatrixReader {
    type Error = DomainError;

    fn read(&self) -> Result<Vec<CoreSupportMatrixEntry>, Self::Error> {
        Ok(vec![
            CoreSupportMatrixEntry {
                capability: "unreal.uasset.metadata_preflight".to_owned(),
                classification: ClassificationKind::RequiresAdapter,
                remediation: Remediation::new(
                    "Use the matching authorized Unreal Editor bridge for semantic export.",
                )?,
            },
            CoreSupportMatrixEntry {
                capability: "unreal.editor_bridge.protocol".to_owned(),
                classification: ClassificationKind::RequiresAdapter,
                remediation: Remediation::new(
                    "Run the checked-in bridge only with an authorized matching Unreal Editor project.",
                )?,
            },
            CoreSupportMatrixEntry {
                capability: "unreal.cooked_or_encrypted_content".to_owned(),
                classification: ClassificationKind::Rejected,
                remediation: Remediation::new(
                    "Use authorized editable project source; Vortex does not bypass access controls.",
                )?,
            },
            CoreSupportMatrixEntry {
                capability: "usd.usda.selected_stage".to_owned(),
                classification: ClassificationKind::RequiresAdapter,
                remediation: Remediation::new(
                    "Use the bounded USDA adapter for selected facts; use an OpenUSD runtime for full composition.",
                )?,
            },
            CoreSupportMatrixEntry {
                capability: "unity.urp_text_project".to_owned(),
                classification: ClassificationKind::RequiresAdapter,
                remediation: Remediation::new(
                    "Use Force Text source inspection for the supported subset or a matching Unity Editor for import resolution.",
                )?,
            },
            CoreSupportMatrixEntry {
                capability: "glb.deterministic_packaging".to_owned(),
                classification: ClassificationKind::Converted,
                remediation: Remediation::new(
                    "Use the validated canonical scene IR and deterministic GLB package boundary.",
                )?,
            },
            CoreSupportMatrixEntry {
                capability: "artifact.atomic_publication".to_owned(),
                classification: ClassificationKind::Converted,
                remediation: Remediation::new(
                    "Publish through the root-confined atomic artifact store and verify before loading.",
                )?,
            },
            CoreSupportMatrixEntry {
                capability: "bevy.0_19_verified_runtime".to_owned(),
                classification: ClassificationKind::Converted,
                remediation: Remediation::new(
                    "Load only verified published manifests through the Bevy 0.19 runtime adapter.",
                )?,
            },
            CoreSupportMatrixEntry {
                capability: "e2e.synthetic_usda_fixture".to_owned(),
                classification: ClassificationKind::Converted,
                remediation: Remediation::new(
                    "Use the self-authored synthetic fixture suite; real Editor validation remains opt-in.",
                )?,
            },
            CoreSupportMatrixEntry {
                capability: "e2e.synthetic_unity_urp_fixture".to_owned(),
                classification: ClassificationKind::Converted,
                remediation: Remediation::new(
                    "Use the owned Force Text Lit/Unlit fixture suite; matching-Editor validation remains required for real projects.",
                )?,
            },
        ])
    }
}

fn support_matrix(_mode: OutputMode) -> Result<CliReport> {
    let entries = read_support_matrix(&StaticSupportMatrixReader)
        .context("could not read the Vortex-Bevy support matrix")?;
    Ok(CliReport::SupportMatrix {
        entries: entries
            .into_iter()
            .map(|entry| SupportMatrixReportEntry {
                capability: entry.capability,
                classification: classification_label(&entry.classification).to_owned(),
                remediation: entry.remediation.as_str().to_owned(),
            })
            .collect(),
    })
}

fn version_report(_mode: OutputMode) -> CliReport {
    CliReport::Version { version: env!("CARGO_PKG_VERSION").to_owned() }
}

fn logical_project_root(path: &std::path::Path) -> Result<ProjectRoot> {
    let name = path
        .file_name()
        .and_then(|component| component.to_str())
        .filter(|component| !component.is_empty())
        .unwrap_or("project");
    name.parse::<ProjectRoot>()
        .context("project root basename is not a valid logical project identity")
}

fn parse_asset_path(value: &str) -> Result<SourceAssetPath, String> {
    value.parse::<SourceAssetPath>().map_err(|error| error.to_string())
}

fn parse_unreal_object_path(value: &str) -> Result<UnrealObjectPath, String> {
    value.parse::<UnrealObjectPath>().map_err(|error| error.to_string())
}

fn parse_profile_name(value: &str) -> Result<ExportProfileName, String> {
    value.parse::<ExportProfileName>().map_err(|error| error.to_string())
}

fn parse_stage_selection(value: &str) -> Result<StageSelection, String> {
    value.parse::<StageSelection>().map_err(|error| error.to_string())
}

const fn engine_label(engine: Engine) -> &'static str {
    match engine {
        Engine::Unreal => "Unreal",
        Engine::Usd => "USD",
        Engine::Unity => "Unity",
    }
}

const fn classification_label(kind: &ClassificationKind) -> &'static str {
    match kind {
        ClassificationKind::Converted => "converted",
        ClassificationKind::Baked => "baked",
        ClassificationKind::RequiresAdapter => "requires_adapter",
        ClassificationKind::Unsupported => "unsupported",
        ClassificationKind::Rejected => "rejected",
    }
}

fn exit_code_for(error: &anyhow::Error) -> u8 {
    if error.to_string().contains("unavailable") {
        EXIT_UNAVAILABLE
    } else {
        EXIT_INSPECTION_FAILURE
    }
}

fn print_report(report: &CliReport, output_mode: OutputMode) {
    if output_mode == OutputMode::Json {
        match serde_json::to_string_pretty(report) {
            Ok(json) => println!("{json}"),
            Err(error) => eprintln!("vortex-bevy: could not serialize machine report: {error}"),
        }
        return;
    }
    match report {
        CliReport::Inspection { source, classification, reason, remediation, facts } => {
            println!("source: {source}");
            println!("classification: {classification}");
            println!("reason: {reason}");
            println!("remediation: {remediation}");
            for fact in facts {
                println!("fact: {fact}");
            }
        }
        CliReport::Unavailable { command, capability, remediation } => {
            println!("command: {command}");
            println!("unavailable capability: {capability}");
            println!("remediation: {remediation}");
        }
        CliReport::SupportMatrix { entries } => {
            for entry in entries {
                println!(
                    "capability: {}\nclassification: {}\nremediation: {}",
                    entry.capability, entry.classification, entry.remediation
                );
            }
        }
        CliReport::Version { version } => println!("vortex-bevy {version}"),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    #[test]
    fn malformed_variant_never_reaches_dispatch() {
        let dispatches = AtomicUsize::new(0);
        let result = parse_and_dispatch(
            [
                "vortex-bevy",
                "plan",
                "--project",
                "project",
                "--engine",
                "usd",
                "--asset",
                "Scenes/Fixture.usda",
                "--profile",
                "default",
                "--output",
                "output",
                "--variant",
                "invalid\nvariant",
            ],
            |_| {
                let _previous = dispatches.fetch_add(1, Ordering::SeqCst);
                Ok(CliReport::Version { version: "unreachable".to_owned() })
            },
        );

        assert!(result.is_err());
        assert_eq!(dispatches.load(Ordering::SeqCst), 0);
    }
}
