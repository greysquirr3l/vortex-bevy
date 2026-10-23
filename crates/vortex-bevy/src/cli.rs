//! Typed command-line boundary for Vortex-Bevy workflows.
//!
//! Clap parses external input into validated domain value objects before dispatch.
//! Commands whose downstream recovery task is not yet implemented report an explicit
//! unavailable capability rather than guessing at a conversion result.

use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    process::ExitCode,
    str::FromStr,
};

use crate::runtime::{
    VortexRuntimeDiagnostic, WindowProbe, build_viewer_app,
    load_published_conversion_or_diagnostic, published_conversion_asset_root,
};
use anyhow::{Context, Result, anyhow};
use clap::{Args, Parser, Subcommand, ValueEnum};
use serde::Serialize;
use vortex_core::{
    adapters::secondary::{
        artifact_store::AtomicArtifactStore, conversion_pipeline::ConversionPipeline,
        unreal_package::UnrealPackageInspector,
    },
    application::{
        command::{
            execute_conversion::handle as execute_conversion,
            inspect_input::{InspectInput, handle as inspect_input},
            plan_conversion::{PlanConversion, handle as plan_conversion},
        },
        query::support_matrix::{
            SupportMatrixEntry as CoreSupportMatrixEntry, SupportMatrixReader,
            handle as read_support_matrix,
        },
    },
    domain::{
        ArtifactHash, ClassificationKind, ConversionContext, ConversionId, DomainError,
        ExportProfileName, OutputRoot, ProjectRoot, Remediation, SourceAssetPath, SourceIdentity,
        StageSelection, UnityAssetGuid, UnityObjectId, UnrealObjectPath, UsdPrimPath,
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
    /// Engine-specific secondary identifier: Unreal object path, USD prim path,
    /// Unity object id, or Godot resource path. Required for all engines except Godot.
    #[arg(long, value_parser = parse_optional_non_empty_string)]
    object: Option<String>,
    /// Unity asset GUID. Required only when `--engine unity`.
    #[arg(long, value_parser = parse_optional_non_empty_string)]
    guid: Option<String>,
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
    /// A deterministic conversion plan resolved without publishing artifacts.
    Plan {
        conversion_id: String,
        source: String,
        action: String,
        classification: String,
        reason: String,
        remediation: String,
    },
    /// A conversion executed or was reused from the artifact store.
    Convert {
        conversion_id: String,
        source: String,
        action: String,
        classification: String,
        reason: String,
        remediation: String,
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
        Command::Plan(arguments) => plan(&arguments),
        Command::Convert(arguments) => convert(&arguments),
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

fn plan(arguments: &ConversionArgs) -> Result<CliReport> {
    validate_conversion_arguments(arguments)?;
    let source = build_source_identity(arguments.engine, arguments)?;
    let context = build_conversion_context(arguments)?;
    let source_string = source.to_string();
    let store = open_artifact_store(&arguments.output)?;
    let plan = plan_conversion(PlanConversion { source, context }, &store)
        .with_context(|| format!("could not resolve conversion plan for {source_string}"))?;
    let (action, aggregate) = match &plan.action {
        vortex_core::application::command::plan_conversion::PlanAction::Execute => {
            ("execute".to_owned(), None)
        }
        vortex_core::application::command::plan_conversion::PlanAction::Reuse(existing) => {
            ("reuse".to_owned(), Some(existing.as_ref()))
        }
    };
    let classification = aggregate
        .and_then(|value| value.classification())
        .map_or_else(pending_classification, classification_record);
    Ok(CliReport::Plan {
        conversion_id: plan.id.as_str().to_owned(),
        source: source_string,
        action,
        classification: classification_label(&classification.kind).to_owned(),
        reason: classification.reason,
        remediation: classification.remediation,
    })
}

fn convert(arguments: &ConversionArgs) -> Result<CliReport> {
    validate_conversion_arguments(arguments)?;
    let source = build_source_identity(arguments.engine, arguments)?;
    let context = build_conversion_context(arguments)?;
    let source_string = source.to_string();
    let store = open_artifact_store(&arguments.output)?;
    let plan = plan_conversion(PlanConversion { source, context }, &store)
        .with_context(|| format!("could not resolve conversion plan for {source_string}"))?;
    let runner = ConversionPipeline::new(&arguments.project, &arguments.output);
    let outcome = execute_conversion(plan, &runner, &store)
        .with_context(|| format!("conversion execution failed for {source_string}"))?;
    let (action, aggregate) = match &outcome {
        vortex_core::application::command::execute_conversion::ExecutionOutcome::Executed(
            value,
        ) => ("executed".to_owned(), value.as_ref()),
        vortex_core::application::command::execute_conversion::ExecutionOutcome::Reused(value) => {
            ("reused".to_owned(), value.as_ref())
        }
    };
    let classification =
        aggregate.classification().map_or_else(pending_classification, classification_record);
    Ok(CliReport::Convert {
        conversion_id: aggregate.id().as_str().to_owned(),
        source: source_string,
        action,
        classification: classification_label(&classification.kind).to_owned(),
        reason: classification.reason,
        remediation: classification.remediation,
    })
}

struct ClassificationRecord {
    kind: ClassificationKind,
    reason: String,
    remediation: String,
}

fn classification_record(
    classification: &vortex_core::domain::ConversionClassification,
) -> ClassificationRecord {
    ClassificationRecord {
        kind: classification.kind.clone(),
        reason: classification.reason.clone(),
        remediation: classification.remediation.as_str().to_owned(),
    }
}

fn pending_classification() -> ClassificationRecord {
    ClassificationRecord {
        kind: ClassificationKind::RequiresAdapter,
        reason: "plan resolved without a classification; awaiting downstream adapter".to_owned(),
        remediation: "Re-run with a recognized engine or invoke the editor bridge for this source."
            .to_owned(),
    }
}

fn build_source_identity(engine: Engine, arguments: &ConversionArgs) -> Result<SourceIdentity> {
    let object = arguments.object.as_deref();
    match engine {
        Engine::Unreal => {
            let object_path =
                object.ok_or_else(|| anyhow!("--object is required for Unreal conversion"))?;
            Ok(SourceIdentity::Unreal {
                project_root: logical_project_root(&arguments.project)?,
                asset_path: arguments.asset.clone(),
                object_path: UnrealObjectPath::from_str(object_path).map_err(|error| {
                    anyhow!("--object is not a valid Unreal object path: {error}")
                })?,
            })
        }
        Engine::Usd => {
            let prim_path = object.ok_or_else(|| {
                anyhow!("--object is required for USD conversion (USD prim path)")
            })?;
            let selection = arguments.variant.clone().unwrap_or_else(default_stage_selection);
            Ok(SourceIdentity::Usd {
                stage: arguments.asset.clone(),
                prim_path: UsdPrimPath::from_str(prim_path)
                    .map_err(|error| anyhow!("--object is not a valid USD prim path: {error}"))?,
                selection,
            })
        }
        Engine::Unity => {
            let guid_string = arguments
                .guid
                .as_deref()
                .ok_or_else(|| anyhow!("--guid is required for Unity conversion"))?;
            let object_id_string = object.ok_or_else(|| {
                anyhow!("--object is required for Unity conversion (Unity object id)")
            })?;
            let parsed_id: u64 = object_id_string.parse().map_err(|error| {
                anyhow!("--object is not a valid Unity object id (expected u64): {error}")
            })?;
            Ok(SourceIdentity::Unity {
                project_root: logical_project_root(&arguments.project)?,
                asset_path: arguments.asset.clone(),
                asset_guid: UnityAssetGuid::from_str(guid_string)
                    .map_err(|error| anyhow!("--guid is not a valid Unity asset GUID: {error}"))?,
                object_id: UnityObjectId::new(parsed_id),
            })
        }
    }
}

fn build_conversion_context(arguments: &ConversionArgs) -> Result<ConversionContext> {
    let profile_hash = ArtifactHash::from_bytes(arguments.profile.as_str().as_bytes());
    Ok(ConversionContext::new(arguments.profile.clone(), profile_hash, env!("CARGO_PKG_VERSION"))?)
}

fn open_artifact_store(output: &std::path::Path) -> Result<AtomicArtifactStore> {
    let logical = logical_output_root(output)?;
    AtomicArtifactStore::new(output, logical)
        .with_context(|| format!("could not open atomic artifact store at {}", output.display()))
}

fn default_stage_selection() -> StageSelection {
    // Empty selection is valid (selects the root layer with default options).
    // StageSelection's validator accepts an empty input as "all defaults".
    StageSelection::from_str("").unwrap_or_else(|error| {
        #[expect(
            clippy::expect_used,
            reason = "empty stage selection is the documented default; validator must accept it"
        )]
        let fallback =
            StageSelection::from_str("").expect("empty stage selection must always parse");
        let _ = error;
        fallback
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
    let logical = logical_output_root(&arguments.artifact_root)?;
    let store = AtomicArtifactStore::new(&arguments.artifact_root, logical).with_context(|| {
        format!("could not open artifact store at {}", arguments.artifact_root.display())
    })?;
    let conversions = store.list_published_conversions().with_context(|| {
        format!("could not enumerate conversions under {}", arguments.artifact_root.display())
    })?;
    let Some(latest) = conversions.last().cloned() else {
        return Ok(CliReport::Unavailable {
            command: "view".to_owned(),
            capability: "published conversion discovery".to_owned(),
            remediation: "No published conversions found under --artifact-root. Run \
                 `vortex-bevy convert --project <project> --engine <engine> ...` first."
                .to_owned(),
        });
    };

    // The manifest declares its GLB relative to the conversion directory, and
    // `verify_manifest` resolves it that way, so the asset server root must be
    // the conversion directory rather than the artifact root that contains it.
    let conversion_directory = published_conversion_asset_root(&store, &latest)
        .with_context(|| format!("could not resolve the asset root for {}", latest.as_str()))?;

    let probe = WindowProbe::new();
    let mut app = build_viewer_app(conversion_directory.to_string_lossy().into_owned());
    probe.install(&mut app);
    let diagnostic_entity = load_published_conversion_or_diagnostic(&mut app, &store, &latest);
    if app.world().entity(diagnostic_entity).get::<VortexRuntimeDiagnostic>().is_some() {
        // Artifact failed to load. Run a short tick loop so the diagnostic
        // components settle, print the diagnostic code + message, and exit.
        for _ in 0..8 {
            app.update();
        }
        if let Some(diag) = app.world().entity(diagnostic_entity).get::<VortexRuntimeDiagnostic>() {
            return Err(anyhow!(
                "view failed for conversion {latest}: {} ({}); see runtime diagnostic entity {}",
                diag.message,
                diag.code,
                diagnostic_entity
            ));
        }
        return Err(anyhow!(
            "view failed for conversion {latest}; see runtime diagnostic entity {diagnostic_entity}"
        ));
    }

    // Print the typed report on stdout before entering the event loop so
    // operators see the load result even when the window is the focus.
    println!(
        "loaded conversion_id: {}\nsource: <artifact-root>\naction: executed\nclassification: converted",
        latest.as_str()
    );
    println!("opening Bevy viewer; close the window to exit");

    // Enter the Bevy event loop. App::run() blocks until the window closes.
    // The probe observes window creation from inside the schedule, because
    // App::run() moves the world into the event loop and leaves this `app`
    // empty, so a query over it after the fact would always report zero.
    app.run();

    let window_count = probe.observed_window_count();
    let report = viewer_window_report(&latest, &arguments.artifact_root, window_count);
    if let CliReport::Convert { reason, .. } = &report
        && window_count == 0
    {
        eprintln!("{reason}");
    }
    Ok(report)
}

/// Builds the typed report for a viewer session that has ended.
///
/// `window_count` is the number of native windows the viewer observed while
/// the event loop was running. A count of zero means the platform windowing
/// backend never produced a window.
pub fn viewer_window_report(
    conversion_id: &ConversionId,
    artifact_root: &Path,
    window_count: usize,
) -> CliReport {
    if window_count == 0 {
        return CliReport::Convert {
            conversion_id: conversion_id.as_str().to_owned(),
            source: "<artifact-root>".to_owned(),
            action: "executed".to_owned(),
            classification: "converted".to_owned(),
            reason: format!(
                "vortex-bevy: viewer exited without creating a native window. \
                 The published conversion {conversion_id} loaded, but the platform \
                 windowing backend did not produce a window before the event loop \
                 ended. The conversion, manifest, and atomic publication remain \
                 intact and can be inspected at {artifact_root} with \
                 `vortex-bevy verify --conversion-id {conversion_id}`.",
                artifact_root = artifact_root.display(),
            ),
            remediation: format!(
                "The native viewer window could not be created on this host. \
                 Use `vortex-bevy verify --conversion-id {conversion_id} \
                 --artifact-root {artifact_root}` to inspect the published GLB + \
                 manifest, or run the binary on a host with an available desktop \
                 session.",
                artifact_root = artifact_root.display(),
            ),
        };
    }

    CliReport::Convert {
        conversion_id: conversion_id.as_str().to_owned(),
        source: "<artifact-root>".to_owned(),
        action: "executed".to_owned(),
        classification: "converted".to_owned(),
        reason: "published conversion loaded; viewer opened in Bevy window".to_owned(),
        remediation:
            "Press F (or use the Frame toolbar action) to fit the camera; right-drag to orbit; \
             middle-drag to pan; wheel to zoom. Close the window to exit."
                .to_owned(),
    }
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
        let mut entries = base_support_entries()?;
        entries.extend(evidence_support_entries()?);
        entries.extend(packaged_usd_support_entries()?);
        Ok(entries)
    }
}

fn base_support_entries() -> Result<Vec<CoreSupportMatrixEntry>, DomainError> {
    entry_group(&[
        (
            "unreal.uasset.metadata_preflight",
            ClassificationKind::RequiresAdapter,
            "Use the matching authorized Unreal Editor bridge for semantic export.",
        ),
        (
            "unreal.utexture2d.ue5_8_2_asset_registry_metadata",
            ClassificationKind::RequiresAdapter,
            "Inspect only the bounded UE 5.8.2 Texture2D tag table; use the authorized Editor bridge for texture export.",
        ),
        (
            "unreal.editor_bridge.protocol",
            ClassificationKind::RequiresAdapter,
            "Run the checked-in bridge only with an authorized matching Unreal Editor project.",
        ),
        (
            "unreal.cooked_or_encrypted_content",
            ClassificationKind::Rejected,
            "Use authorized editable project source; Vortex does not bypass access controls.",
        ),
        (
            "usd.usda.selected_stage",
            ClassificationKind::RequiresAdapter,
            "Use the bounded USDA adapter for selected facts; use an OpenUSD runtime for full composition.",
        ),
        (
            "unity.urp_text_project",
            ClassificationKind::RequiresAdapter,
            "Use Force Text source inspection for the supported subset or a matching Unity Editor for import resolution.",
        ),
        (
            "glb.deterministic_packaging",
            ClassificationKind::Converted,
            "Use the validated canonical scene IR and deterministic GLB package boundary.",
        ),
        (
            "artifact.atomic_publication",
            ClassificationKind::Converted,
            "Publish through the root-confined atomic artifact store and verify before loading.",
        ),
        (
            "bevy.0_20_verified_runtime",
            ClassificationKind::Converted,
            "Load only verified published manifests through the Bevy 0.20 runtime adapter.",
        ),
    ])
}

fn evidence_support_entries() -> Result<Vec<CoreSupportMatrixEntry>, DomainError> {
    entry_group(&[
        (
            "e2e.synthetic_usda_fixture",
            ClassificationKind::Converted,
            "Use the self-authored synthetic fixture suite; real Editor validation remains opt-in.",
        ),
        (
            "e2e.synthetic_unity_urp_fixture",
            ClassificationKind::Converted,
            "Use the owned Force Text Lit/Unlit fixture suite; matching-Editor validation remains required for real projects.",
        ),
        (
            "e2e.authorized_unity_urp_static_scene",
            ClassificationKind::Converted,
            "Use the documented opt-in Unity static-scene bridge with a matching authorized Editor and URP version.",
        ),
        (
            "e2e.authorized_unreal_static_scene",
            ClassificationKind::Converted,
            "Use the documented opt-in Unreal static-scene bridge with a matching authorized Editor project.",
        ),
        (
            "unreal.fbx_tga_static_scene_canonical_glb",
            ClassificationKind::Converted,
            "Convert only report-confined authoritative FBX and TGA exports through the bounded canonical adapter.",
        ),
    ])
}

fn packaged_usd_support_entries() -> Result<Vec<CoreSupportMatrixEntry>, DomainError> {
    entry_group(&[
        (
            "usd.packaged_flattened_stage",
            ClassificationKind::Converted,
            "Use the deterministic flattened fallback packaged before Bevy runtime loading.",
        ),
        (
            "usd.packaged_variant_alternatives",
            ClassificationKind::Converted,
            "Pre-export named variant selections; do not evaluate live USD composition at runtime.",
        ),
        (
            "usd.expanded_instances",
            ClassificationKind::Converted,
            "Materialize bounded instance attachments before deterministic GLB packaging.",
        ),
        (
            "usd.instance_batches",
            ClassificationKind::Converted,
            "Lower bounded shared instances to ordinary GLB nodes referencing a shared mesh.",
        ),
        (
            "usd.scene_chunks",
            ClassificationKind::Converted,
            "Use explicit quantized-bounds chunks atomically published beside the parent manifest.",
        ),
    ])
}

fn entry_group(
    declarations: &[(&str, ClassificationKind, &str)],
) -> Result<Vec<CoreSupportMatrixEntry>, DomainError> {
    declarations
        .iter()
        .map(|(capability, classification, remediation)| {
            Ok(CoreSupportMatrixEntry {
                capability: (*capability).to_owned(),
                classification: classification.clone(),
                remediation: Remediation::new(*remediation)?,
            })
        })
        .collect()
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

fn logical_output_root(path: &std::path::Path) -> Result<OutputRoot> {
    let name = path
        .file_name()
        .and_then(|component| component.to_str())
        .filter(|component| !component.is_empty())
        .unwrap_or("output");
    name.parse::<OutputRoot>()
        .context("output root basename is not a valid logical output identity")
}

fn parse_optional_non_empty_string(value: &str) -> Result<String, String> {
    if value.trim().is_empty() {
        return Err("value cannot be empty".to_owned());
    }
    Ok(value.to_owned())
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
        CliReport::Plan { conversion_id, source, action, classification, reason, remediation }
        | CliReport::Convert {
            conversion_id,
            source,
            action,
            classification,
            reason,
            remediation,
        } => {
            println!("conversion_id: {conversion_id}");
            println!("source: {source}");
            println!("action: {action}");
            println!("classification: {classification}");
            println!("reason: {reason}");
            println!("remediation: {remediation}");
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
