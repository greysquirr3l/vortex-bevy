use std::{
    collections::BTreeSet,
    error::Error,
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};

use vortex_bevy::cli::{CliReport, run_from};

static NEXT_TEST_DIRECTORY: AtomicUsize = AtomicUsize::new(0);

struct TestDirectory {
    path: PathBuf,
}

impl TestDirectory {
    fn new(label: &str) -> Result<Self, std::io::Error> {
        let sequence = NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir()
            .join(format!("vortex-bevy-{label}-{}-{sequence}", std::process::id()));
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

#[test]
fn every_documented_subcommand_appears_in_help() -> Result<(), Box<dyn Error>> {
    let executable = std::env::var("CARGO_BIN_EXE_vortex-bevy")?;
    let output = Command::new(executable).arg("--help").output()?;
    let help = String::from_utf8(output.stdout)?;

    assert!(output.status.success());
    for command in ["inspect", "plan", "convert", "verify", "view", "support-matrix", "version"] {
        assert!(help.contains(command));
    }
    Ok(())
}

#[test]
fn support_matrix_returns_a_machine_readable_typed_report() -> Result<(), Box<dyn Error>> {
    let report = run_from(["vortex-bevy", "support-matrix", "--format", "json"])?;

    match report {
        CliReport::SupportMatrix { entries } => {
            let expected = BTreeSet::from([
                "artifact.atomic_publication",
                "bevy.0_19_verified_runtime",
                "e2e.authorized_unity_urp_static_scene",
                "e2e.authorized_unreal_static_scene",
                "e2e.synthetic_unity_urp_fixture",
                "e2e.synthetic_usda_fixture",
                "glb.deterministic_packaging",
                "unreal.cooked_or_encrypted_content",
                "unreal.editor_bridge.protocol",
                "unreal.fbx_tga_static_scene_canonical_glb",
                "unreal.uasset.metadata_preflight",
                "unity.urp_text_project",
                "usd.expanded_instances",
                "usd.instance_batches",
                "usd.packaged_flattened_stage",
                "usd.packaged_variant_alternatives",
                "usd.scene_chunks",
                "usd.usda.selected_stage",
            ]);
            assert_eq!(entries.len(), expected.len());
            let encoded = serde_json::to_string(&entries)?;
            for capability in expected {
                assert!(encoded.contains(capability));
            }
        }
        other => return Err(format!("unexpected report: {other:?}").into()),
    }
    Ok(())
}

#[test]
fn checked_in_compatibility_matrix_is_generated_from_the_typed_cli_model()
-> Result<(), Box<dyn Error>> {
    let report = run_from(["vortex-bevy", "support-matrix", "--format", "json"])?;
    let generated = serde_json::to_value(report)?;
    let checked_in = serde_json::from_str::<serde_json::Value>(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/compatibility_matrix.json"
    )))?;

    assert_eq!(generated, checked_in);
    Ok(())
}

#[test]
fn malformed_profile_is_rejected_before_conversion_dispatch() -> Result<(), Box<dyn Error>> {
    let executable = std::env::var("CARGO_BIN_EXE_vortex-bevy")?;
    let output = Command::new(executable)
        .args([
            "plan",
            "--project",
            "project",
            "--engine",
            "unreal",
            "--asset",
            "Content/Fixture.uasset",
            "--profile",
            "invalid/profile",
            "--output",
            "output",
        ])
        .output()?;
    let stderr = String::from_utf8(output.stderr)?;

    assert_eq!(output.status.code(), Some(2));
    assert!(stderr.contains("--profile"));
    assert!(stderr.contains("forbidden character"));
    Ok(())
}

#[test]
fn malformed_variant_is_rejected_before_conversion_dispatch() -> Result<(), Box<dyn Error>> {
    let executable = std::env::var("CARGO_BIN_EXE_vortex-bevy")?;
    let output = Command::new(executable)
        .args([
            "plan",
            "--project",
            "project",
            "--engine",
            "usd",
            "--asset",
            "Scenes/fixture.usda",
            "--profile",
            "default",
            "--output",
            "output",
            "--variant",
            "invalid\nvariant",
        ])
        .output()?;
    let stderr = String::from_utf8(output.stderr)?;

    assert_eq!(output.status.code(), Some(2));
    assert!(stderr.contains("--variant"));
    assert!(stderr.contains("forbidden character"));
    Ok(())
}

#[test]
fn inspect_dispatches_a_typed_unreal_source_to_bounded_preflight() -> Result<(), Box<dyn Error>> {
    let project = TestDirectory::new("cli-inspect")?;
    let asset = project.path().join("Content/Fixture.uasset");
    let parent = asset.parent().ok_or("fixture path has no parent")?;
    fs::create_dir_all(parent)?;
    fs::write(&asset, b"invalid-package")?;
    let project = project.path().display().to_string();

    let report = run_from([
        "vortex-bevy",
        "inspect",
        "--project",
        &project,
        "--engine",
        "unreal",
        "--asset",
        "Content/Fixture.uasset",
        "--object",
        "/Game/Fixture.Fixture",
    ])?;

    match report {
        CliReport::Inspection { classification, facts, .. } => {
            assert_eq!(classification, "rejected");
            assert!(
                facts.iter().any(|fact| fact == "diagnostic.code=unreal.invalid_package_magic")
            );
        }
        other => return Err(format!("unexpected report: {other:?}").into()),
    }
    Ok(())
}

#[test]
fn every_subcommand_has_its_own_help_contract() -> Result<(), Box<dyn Error>> {
    let executable = std::env::var("CARGO_BIN_EXE_vortex-bevy")?;
    for command in ["inspect", "plan", "convert", "verify", "view", "support-matrix", "version"] {
        let output = Command::new(&executable).args([command, "--help"]).output()?;
        let help = String::from_utf8(output.stdout)?;

        assert!(output.status.success());
        assert!(help.contains("Usage:"));
    }
    Ok(())
}

#[test]
fn valid_recovery_commands_return_explicit_typed_reports() -> Result<(), Box<dyn Error>> {
    let cases = [
        (
            vec![
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
            ],
            "plan",
        ),
        (
            vec![
                "vortex-bevy",
                "convert",
                "--project",
                "project",
                "--engine",
                "unity",
                "--asset",
                "Assets/Fixture.prefab",
                "--profile",
                "default",
                "--output",
                "output",
            ],
            "convert",
        ),
        (vec!["vortex-bevy", "verify", "--conversion-id", "stable-id"], "verify"),
        (vec!["vortex-bevy", "view", "--artifact-root", "artifacts"], "view"),
    ];

    for (arguments, command) in cases {
        let report = run_from(arguments)?;
        match report {
            CliReport::Unavailable { command: actual, remediation, .. } => {
                assert_eq!(actual, command);
                assert!(!remediation.is_empty());
            }
            other => return Err(format!("unexpected report: {other:?}").into()),
        }
    }

    let version = run_from(["vortex-bevy", "version", "--format", "json"])?;
    match version {
        CliReport::Version { version } => assert!(!version.is_empty()),
        other => return Err(format!("unexpected report: {other:?}").into()),
    }
    Ok(())
}

#[test]
fn missing_project_and_unsupported_engine_have_stable_nonzero_exit_codes()
-> Result<(), Box<dyn Error>> {
    let executable = std::env::var("CARGO_BIN_EXE_vortex-bevy")?;
    let missing = Command::new(&executable)
        .args([
            "inspect",
            "--project",
            "does-not-exist",
            "--engine",
            "unreal",
            "--asset",
            "Content/Fixture.uasset",
            "--object",
            "/Game/Fixture.Fixture",
        ])
        .output()?;
    let missing_stderr = String::from_utf8(missing.stderr)?;
    assert_eq!(missing.status.code(), Some(4));
    assert!(missing_stderr.contains("authorized Unreal project root"));

    let unsupported = Command::new(executable)
        .args([
            "inspect",
            "--project",
            "does-not-matter",
            "--engine",
            "unity",
            "--asset",
            "Assets/Fixture.prefab",
        ])
        .output()?;
    let unsupported_stderr = String::from_utf8(unsupported.stderr)?;
    assert_eq!(unsupported.status.code(), Some(3));
    assert!(unsupported_stderr.contains("Unity inspection is unavailable"));
    Ok(())
}
