use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
};

use vortex_core::{
    adapters::secondary::unreal_package::{UnrealPackageInspectionError, UnrealPackageInspector},
    domain::{ClassificationKind, ProjectRoot, SourceAssetPath, SourceIdentity, UnrealObjectPath},
};

static NEXT_TEST_DIRECTORY: AtomicUsize = AtomicUsize::new(0);

struct TestDirectory {
    path: PathBuf,
}

impl TestDirectory {
    fn new(label: &str) -> Result<Self, std::io::Error> {
        let sequence = NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir()
            .join(format!("vortex-core-{label}-{}-{sequence}", std::process::id()));
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

fn source(asset_path: &str) -> Result<SourceIdentity, Box<dyn Error>> {
    Ok(SourceIdentity::Unreal {
        project_root: "project".parse::<ProjectRoot>()?,
        asset_path: asset_path.parse::<SourceAssetPath>()?,
        object_path: "/Game/Fixture.Asset".parse::<UnrealObjectPath>()?,
    })
}

fn write_package(root: &Path, relative: &str, bytes: &[u8]) -> Result<PathBuf, std::io::Error> {
    let path = root.join(relative);
    let parent = path.parent().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "fixture path has no parent")
    })?;
    fs::create_dir_all(parent)?;
    fs::write(&path, bytes)?;
    Ok(path)
}

fn write_i32(bytes: &mut Vec<u8>, value: i32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn write_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn write_i64(bytes: &mut Vec<u8>, value: i64) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn minimal_uasset_header() -> Vec<u8> {
    minimal_uasset_header_with(214, 0)
}

fn write_i32_at(bytes: &mut [u8], offset: usize, value: i32) {
    for (slot, byte) in bytes.iter_mut().skip(offset).zip(value.to_le_bytes()) {
        *slot = byte;
    }
}

fn write_name(bytes: &mut Vec<u8>, value: &str) -> Result<(), Box<dyn Error>> {
    let length = i32::try_from(value.len() + 1)?;
    write_i32(bytes, length);
    bytes.extend_from_slice(value.as_bytes());
    bytes.push(0);
    Ok(())
}

fn write_name_reference(bytes: &mut Vec<u8>, index: i32) {
    write_i32(bytes, index);
    write_u32(bytes, 0);
}

fn dependency_uasset_header() -> Result<Vec<u8>, Box<dyn Error>> {
    let mut bytes = minimal_uasset_header();
    let names_offset = i32::try_from(bytes.len())?;
    write_i32_at(&mut bytes, 42, 3);
    write_i32_at(&mut bytes, 46, names_offset);
    write_name(&mut bytes, "/Script/CoreUObject")?;
    write_name(&mut bytes, "Package")?;
    write_name(&mut bytes, "/Game/Missing")?;

    let imports_offset = i32::try_from(bytes.len())?;
    write_i32_at(&mut bytes, 58, 1);
    write_i32_at(&mut bytes, 62, imports_offset);
    write_name_reference(&mut bytes, 0);
    write_name_reference(&mut bytes, 1);
    write_i32(&mut bytes, 0);
    write_name_reference(&mut bytes, 2);
    Ok(bytes)
}

fn minimal_uasset_header_with(file_version: i32, bulk_data_start_offset: i64) -> Vec<u8> {
    let mut bytes = Vec::new();
    write_u32(&mut bytes, 0x9e2a_83c1);
    write_i32(&mut bytes, -5);
    write_i32(&mut bytes, 0);
    write_i32(&mut bytes, file_version);
    write_i32(&mut bytes, 0);
    write_i32(&mut bytes, 0);
    write_i32(&mut bytes, 0);
    write_i32(&mut bytes, 6);
    bytes.extend_from_slice(b"/Game\0");
    write_u32(&mut bytes, 0);
    write_i32(&mut bytes, 0);
    write_i32(&mut bytes, 0);
    write_i32(&mut bytes, 0);
    write_i32(&mut bytes, 0);
    write_i32(&mut bytes, 0);
    write_i32(&mut bytes, 0);
    write_i32(&mut bytes, 0);
    write_i32(&mut bytes, 0);
    write_i32(&mut bytes, 0);
    bytes.extend_from_slice(&[0_u8; 16]);
    write_i32(&mut bytes, 0);
    write_u32(&mut bytes, 0);
    write_u32(&mut bytes, 0);
    write_i32(&mut bytes, 0);
    write_u32(&mut bytes, 0);
    write_i32(&mut bytes, 0);
    write_i32(&mut bytes, 0);
    write_i32(&mut bytes, 0);
    write_i64(&mut bytes, bulk_data_start_offset);
    bytes
}

#[test]
fn self_authored_header_yields_sorted_metadata_and_editor_bridge_classification()
-> Result<(), Box<dyn Error>> {
    let project = TestDirectory::new("minimal-header")?;
    write_package(project.path(), "Content/Fixture.uasset", &minimal_uasset_header())?;
    let inspector = UnrealPackageInspector::new(project.path())?;

    let report = inspector.preflight(&source("Content/Fixture.uasset")?)?;

    assert_eq!(report.classification.kind, ClassificationKind::RequiresAdapter);
    assert!(
        report.facts.windows(2).all(|facts| facts
            .first()
            .zip(facts.get(1))
            .is_some_and(|(left, right)| left <= right))
    );
    assert!(report.facts.iter().any(|fact| fact == "package.name=/Game"));
    assert!(report.facts.iter().any(|fact| fact == "table.names=0"));
    assert!(report.facts.iter().any(|fact| fact == "export.class=<unknown>"));
    assert!(report.facts.iter().any(|fact| fact == "package.version.legacy=-5"));
    Ok(())
}

#[test]
fn declared_package_dependency_is_reported_from_the_metadata_table() -> Result<(), Box<dyn Error>> {
    let project = TestDirectory::new("dependency-table")?;
    write_package(project.path(), "Content/Fixture.uasset", &dependency_uasset_header()?)?;
    let inspector = UnrealPackageInspector::new(project.path())?;

    let report = inspector.preflight(&source("Content/Fixture.uasset")?)?;

    assert_eq!(report.classification.kind, ClassificationKind::RequiresAdapter);
    assert!(report.facts.iter().any(|fact| fact == "dependency.package=/Game/Missing"));
    Ok(())
}

#[test]
fn missing_required_external_bulk_companion_is_rejected() -> Result<(), Box<dyn Error>> {
    let project = TestDirectory::new("missing-bulk")?;
    write_package(
        project.path(),
        "Content/Fixture.uasset",
        &minimal_uasset_header_with(214, 4_096),
    )?;
    let inspector = UnrealPackageInspector::new(project.path())?;

    let report = inspector.preflight(&source("Content/Fixture.uasset")?)?;

    assert_eq!(report.classification.kind, ClassificationKind::Rejected);
    assert!(report.facts.iter().any(|fact| fact == "companion.ubulk=missing"));
    assert!(
        report
            .facts
            .iter()
            .any(|fact| fact == "diagnostic.code=unreal.required_bulk_companion_missing")
    );
    Ok(())
}

#[test]
fn incompatible_header_version_is_rejected_without_payload_access() -> Result<(), Box<dyn Error>> {
    let project = TestDirectory::new("incompatible-version")?;
    write_package(
        project.path(),
        "Content/Fixture.uasset",
        &minimal_uasset_header_with(999_999, 0),
    )?;
    let inspector = UnrealPackageInspector::new(project.path())?;

    let report = inspector.preflight(&source("Content/Fixture.uasset")?)?;

    assert_eq!(report.classification.kind, ClassificationKind::Rejected);
    assert!(report.facts.iter().any(|fact| fact == "diagnostic.code=unreal.metadata_parse_failed"));
    Ok(())
}

#[test]
fn malformed_header_fuzz_smoke_never_panics() -> Result<(), Box<dyn Error>> {
    let project = TestDirectory::new("fuzz-smoke")?;
    let inspector = UnrealPackageInspector::new(project.path())?;
    let source = source("Content/Fixture.uasset")?;

    for length in 0_u8..=128 {
        let bytes = (0..usize::from(length))
            .map(|index| length.wrapping_add(u8::try_from(index).unwrap_or(0)))
            .collect::<Vec<_>>();
        write_package(project.path(), "Content/Fixture.uasset", &bytes)?;
        let _result = inspector.preflight(&source);
    }
    Ok(())
}

#[test]
fn malformed_prefix_is_rejected_with_a_stable_diagnostic() -> Result<(), Box<dyn Error>> {
    let project = TestDirectory::new("invalid-prefix")?;
    write_package(project.path(), "Content/Fixture.uasset", b"not-a-package")?;
    let inspector = UnrealPackageInspector::new(project.path())?;

    let report = inspector.preflight(&source("Content/Fixture.uasset")?)?;

    assert_eq!(report.classification.kind, ClassificationKind::Rejected);
    assert!(report.facts.iter().any(|fact| fact == "diagnostic.code=unreal.invalid_package_magic"));
    Ok(())
}

#[test]
fn cooked_sidecar_is_rejected_before_metadata_parsing() -> Result<(), Box<dyn Error>> {
    let project = TestDirectory::new("cooked-sidecar")?;
    let package =
        write_package(project.path(), "Content/Fixture.uasset", &[0xc1, 0x83, 0x2a, 0x9e])?;
    fs::write(package.with_extension("uexp"), b"cooked-sidecar")?;
    let inspector = UnrealPackageInspector::new(project.path())?;

    let report = inspector.preflight(&source("Content/Fixture.uasset")?)?;

    assert_eq!(report.classification.kind, ClassificationKind::Rejected);
    assert!(
        report.facts.iter().any(|fact| fact == "diagnostic.code=unreal.cooked_context_detected")
    );
    Ok(())
}

#[test]
fn configured_size_limit_fails_before_parser_invocation() -> Result<(), Box<dyn Error>> {
    let project = TestDirectory::new("size-limit")?;
    write_package(project.path(), "Content/Fixture.uasset", &[0_u8; 16])?;
    let inspector = UnrealPackageInspector::with_max_package_bytes(project.path(), 8)?;

    let error = inspector
        .preflight(&source("Content/Fixture.uasset")?)
        .err()
        .ok_or("bounded inspection unexpectedly succeeded")?;

    assert!(matches!(
        error,
        UnrealPackageInspectionError::TooLarge { actual_bytes: 16, max_bytes: 8, .. }
    ));
    Ok(())
}

#[cfg(unix)]
#[test]
fn symlink_escape_is_rejected_after_canonicalization() -> Result<(), Box<dyn Error>> {
    use std::os::unix::fs::symlink;

    let project = TestDirectory::new("project-root")?;
    let outside = TestDirectory::new("outside-root")?;
    let outside_package = write_package(outside.path(), "Outside.uasset", b"outside")?;
    let content = project.path().join("Content");
    fs::create_dir_all(&content)?;
    symlink(&outside_package, content.join("Escape.uasset"))?;
    let inspector = UnrealPackageInspector::new(project.path())?;

    let error = inspector
        .preflight(&source("Content/Escape.uasset")?)
        .err()
        .ok_or("symlink escape unexpectedly succeeded")?;

    assert!(matches!(error, UnrealPackageInspectionError::PathEscape { .. }));
    Ok(())
}
