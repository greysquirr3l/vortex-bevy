use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
};

use vortex_core::{
    adapters::secondary::unreal_bridge::{
        UnrealBridge, UnrealBridgeError, UnrealBridgeProcess, UnrealBridgeProcessOutput,
        UnrealBridgeRequest,
    },
    domain::{ClassificationKind, UnrealObjectPath},
};

const UNREAL_REPORT: &str = include_str!("../../../bridges/fixtures/unreal-report.json");

static NEXT_DIRECTORY: AtomicUsize = AtomicUsize::new(0);

struct TestDirectory {
    path: PathBuf,
}

impl TestDirectory {
    fn new(label: &str) -> Result<Self, std::io::Error> {
        let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir()
            .join(format!("vortex-unreal-bridge-{label}-{}-{sequence}", std::process::id()));
        fs::create_dir_all(&path)?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        drop(fs::remove_dir_all(&self.path));
    }
}

struct StaticProcess {
    output: UnrealBridgeProcessOutput,
    calls: AtomicUsize,
}

impl StaticProcess {
    const fn success() -> Self {
        Self {
            output: UnrealBridgeProcessOutput { exit_code: Some(0), stderr: String::new() },
            calls: AtomicUsize::new(0),
        }
    }

    fn failure() -> Self {
        Self {
            output: UnrealBridgeProcessOutput {
                exit_code: Some(17),
                stderr: "simulated exporter failure".to_owned(),
            },
            calls: AtomicUsize::new(0),
        }
    }
}

impl UnrealBridgeProcess for StaticProcess {
    type Error = std::io::Error;

    fn execute(
        &self,
        _command: &vortex_core::adapters::secondary::unreal_bridge::UnrealBridgeCommand,
    ) -> Result<UnrealBridgeProcessOutput, Self::Error> {
        let _previous = self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.output.clone())
    }
}

fn repository_root() -> Result<PathBuf, Box<dyn Error>> {
    Ok(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize()?)
}

fn object_path(value: &str) -> Result<UnrealObjectPath, Box<dyn Error>> {
    Ok(value.parse()?)
}

fn bridge() -> Result<UnrealBridge, Box<dyn Error>> {
    Ok(UnrealBridge::from_repository_root(repository_root()?)?)
}

fn request(directory: &TestDirectory) -> Result<UnrealBridgeRequest, Box<dyn Error>> {
    let project_file = directory.path().join("Fixture.uproject");
    let editor = directory.path().join("UnrealEditor-Cmd");
    let staging = directory.path().join("Staging");
    fs::write(&project_file, "{}")?;
    fs::write(&editor, "simulated-editor")?;
    fs::create_dir_all(&staging)?;
    Ok(UnrealBridgeRequest::new(
        directory.path(),
        project_file,
        editor,
        staging,
        [
            object_path("/Game/Meshes/SM_Crate.SM_Crate")?,
            object_path("/Game/Blueprints/BP_Controller.BP_Controller")?,
        ],
    )?)
}

fn with_replaced_report(original: &str, from: &str, to: &str) -> Result<String, Box<dyn Error>> {
    let replaced = original.replacen(from, to, 1);
    if replaced == original {
        return Err(format!("fixture did not contain expected text: {from}").into());
    }
    Ok(replaced)
}

#[test]
fn fixed_checked_in_script_command_is_deterministic_and_confined() -> Result<(), Box<dyn Error>> {
    let directory = TestDirectory::new("command")?;
    let request = request(&directory)?;
    let command = bridge()?.build_command(&request)?;
    let arguments = command
        .arguments()
        .iter()
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect::<Vec<_>>();

    assert!(command.program().ends_with("UnrealEditor-Cmd"));
    assert!(arguments.iter().any(|argument| argument == "-unattended"));
    // The script path is assembled with the platform separator, so the
    // assertion normalizes it rather than assuming a forward slash.
    let script_suffix = ["bridges", "unreal", "vortex_unreal_bridge.py"].join("/");
    assert!(arguments.iter().any(|argument| argument.replace('\\', "/").contains(&script_suffix)));
    assert!(arguments.iter().any(|argument| argument == "--assets"));
    assert!(arguments.iter().any(|argument| argument
        == "/Game/Blueprints/BP_Controller.BP_Controller,/Game/Meshes/SM_Crate.SM_Crate"));
    assert!(command.report_path().starts_with(request.staging_root()));
    Ok(())
}

#[test]
fn fixture_report_maps_every_record_to_a_typed_requested_source() -> Result<(), Box<dyn Error>> {
    let directory = TestDirectory::new("fixture")?;
    let request = request(&directory)?;
    let report = bridge()?.parse_report_text(&request, UNREAL_REPORT)?;

    assert_eq!(report.records.len(), 2);
    let blueprint = report.records.first().ok_or("missing Blueprint bridge record")?;
    let mesh = report.records.get(1).ok_or("missing mesh bridge record")?;
    assert!(blueprint.source.to_string().ends_with(
        ":Content/Blueprints/BP_Controller.uasset:/Game/Blueprints/BP_Controller.BP_Controller"
    ));
    assert_eq!(blueprint.classification.kind, ClassificationKind::RequiresAdapter);
    assert!(
        mesh.source
            .to_string()
            .ends_with(":Content/Meshes/SM_Crate.uasset:/Game/Meshes/SM_Crate.SM_Crate")
    );
    assert_eq!(mesh.classification.kind, ClassificationKind::Converted);
    assert_eq!(mesh.outputs, vec![PathBuf::from("exports/Game_Meshes_SM_Crate_SM_Crate.fbx")]);
    Ok(())
}

#[test]
fn malformed_and_incompatible_reports_are_rejected_without_editor() -> Result<(), Box<dyn Error>> {
    let directory = TestDirectory::new("protocol")?;
    let request = request(&directory)?;
    let bridge = bridge()?;

    let malformed = bridge.parse_report_text(&request, "{");
    assert!(matches!(malformed, Err(UnrealBridgeError::ReportMalformed { .. })));

    let version = with_replaced_report(
        UNREAL_REPORT,
        "vortex.editor-bridge/v1",
        "vortex.editor-bridge/v999",
    )?;
    let incompatible = bridge.parse_report_text(&request, &version);
    assert!(matches!(incompatible, Err(UnrealBridgeError::UnsupportedProtocol { .. })));
    Ok(())
}

#[test]
fn unsafe_report_outputs_and_unrequested_sources_are_rejected() -> Result<(), Box<dyn Error>> {
    let directory = TestDirectory::new("unsafe-report")?;
    let request = request(&directory)?;
    let bridge = bridge()?;

    let unsafe_output = with_replaced_report(
        UNREAL_REPORT,
        "exports/Game_Meshes_SM_Crate_SM_Crate.fbx",
        "../outside.fbx",
    )?;
    assert!(matches!(
        bridge.parse_report_text(&request, &unsafe_output),
        Err(UnrealBridgeError::UnsafeOutput { .. })
    ));

    let unrequested = with_replaced_report(
        UNREAL_REPORT,
        "/Game/Meshes/SM_Crate.SM_Crate",
        "/Game/Meshes/Unrequested.Unrequested",
    )?;
    assert!(matches!(
        bridge.parse_report_text(&request, &unrequested),
        Err(UnrealBridgeError::UnexpectedSource { .. })
    ));
    Ok(())
}

#[test]
fn unsafe_project_or_staging_paths_are_rejected_before_process_launch() -> Result<(), Box<dyn Error>>
{
    let directory = TestDirectory::new("unsafe-path")?;
    let project_file = directory.path().join("Fixture.uproject");
    let editor = directory.path().join("UnrealEditor-Cmd");
    let staging = directory.path().join("Staging");
    fs::write(&project_file, "{}")?;
    fs::write(&editor, "simulated-editor")?;
    fs::create_dir_all(&staging)?;
    let object = object_path("/Game/Props/Crate.Crate")?;

    let unsafe_project = UnrealBridgeRequest::new(
        directory.path(),
        directory.path().join("nested/../Fixture.uproject"),
        &editor,
        &staging,
        [object.clone()],
    );
    assert!(matches!(unsafe_project, Err(UnrealBridgeError::PathEscape { .. })));

    let unsafe_staging = UnrealBridgeRequest::new(
        directory.path(),
        &project_file,
        &editor,
        directory.path().join("Staging/../Staging"),
        [object],
    );
    assert!(matches!(unsafe_staging, Err(UnrealBridgeError::PathEscape { .. })));
    Ok(())
}

#[test]
fn subprocess_failure_is_mapped_without_reading_a_report() -> Result<(), Box<dyn Error>> {
    let directory = TestDirectory::new("process-failure")?;
    let request = request(&directory)?;
    let process = StaticProcess::failure();

    let result = bridge()?.invoke(&request, &process);

    assert!(matches!(result, Err(UnrealBridgeError::ProcessFailed { exit_code: Some(17), .. })));
    assert_eq!(process.calls.load(Ordering::SeqCst), 1);
    Ok(())
}

#[test]
fn simulated_export_success_reads_only_the_confined_report_path() -> Result<(), Box<dyn Error>> {
    let directory = TestDirectory::new("process-success")?;
    let request = request(&directory)?;
    fs::write(request.staging_root().join("vortex-unreal-bridge-report.json"), UNREAL_REPORT)?;
    let process = StaticProcess::success();

    let report = bridge()?.invoke(&request, &process)?;

    assert_eq!(process.calls.load(Ordering::SeqCst), 1);
    assert_eq!(report.records.len(), 2);
    Ok(())
}
