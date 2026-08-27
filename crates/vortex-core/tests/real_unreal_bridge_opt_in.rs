use std::{
    env,
    error::Error,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
};

use vortex_core::{
    adapters::secondary::unreal_bridge::{UnrealBridge, UnrealBridgeRequest},
    domain::{ClassificationKind, UnrealObjectPath},
};

static NEXT_DIRECTORY: AtomicUsize = AtomicUsize::new(0);

struct TestDirectory {
    path: PathBuf,
}

impl TestDirectory {
    fn new(label: &str) -> Result<Self, std::io::Error> {
        let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir()
            .join(format!("vortex-real-unreal-{label}-{}-{sequence}", std::process::id()));
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

fn environment_path(name: &str) -> Result<PathBuf, Box<dyn Error>> {
    let value = env::var_os(name).ok_or_else(|| format!("missing required {name}"))?;
    let path = PathBuf::from(value).canonicalize()?;
    if !path.is_file() {
        return Err(format!("{name} must name a regular file: {}", path.display()).into());
    }
    Ok(path)
}

fn object_path(value: &str) -> Result<UnrealObjectPath, Box<dyn Error>> {
    Ok(value.parse()?)
}

/// Validates one received, authorized Unreal Editor static-scene bridge result.
///
/// This is deliberately opt-in because it reads real Editor output supplied outside
/// the repository. Invoke it with the report, FBX, and TGA paths explicitly set.
#[test]
#[ignore = "requires explicit paths to authorized received Unreal Editor artifacts"]
fn received_unreal_static_scene_report_and_exports_are_typed_and_bounded()
-> Result<(), Box<dyn Error>> {
    let report_path = environment_path("VORTEX_UNREAL_REPORT")?;
    let mesh_path = environment_path("VORTEX_UNREAL_MESH")?;
    let texture_path = environment_path("VORTEX_UNREAL_TEXTURE")?;
    let staging_root = report_path
        .parent()
        .ok_or_else(|| format!("report lacks parent directory: {}", report_path.display()))?
        .canonicalize()?;
    let directory = TestDirectory::new("bridge-validation")?;
    let project_file = directory.path().join("Test123.uproject");
    let editor = directory.path().join("UnrealEditor");
    fs::write(&project_file, "{}")?;
    fs::write(&editor, "received-artifact-validation-only")?;

    let request = UnrealBridgeRequest::new(
        directory.path(),
        &project_file,
        &editor,
        &staging_root,
        [
            object_path("/Game/VortexBridgeTemporary/LVL_VortexStaticScene.LVL_VortexStaticScene")?,
            object_path("/Game/VortexBridgeTemporary/MI_VortexLit.MI_VortexLit")?,
            object_path("/Game/VortexBridgeTemporary/SM_VortexCube.SM_VortexCube")?,
            object_path("/Game/VortexBridgeTemporary/T_VortexBaseColor.T_VortexBaseColor")?,
        ],
    )?;
    let repository_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize()?;
    let bridge = UnrealBridge::from_repository_root(repository_root)?;
    let report_text = fs::read_to_string(&report_path)?;
    let report = bridge.parse_report_text(&request, &report_text)?;

    assert_eq!(report.records.len(), 4);
    let converted = report
        .records
        .iter()
        .filter(|record| record.classification.kind == ClassificationKind::Converted)
        .collect::<Vec<_>>();
    assert_eq!(converted.len(), 2);
    assert!(converted.iter().any(|record| record.asset_kind == "StaticMesh"));
    assert!(converted.iter().any(|record| record.asset_kind == "Texture2D"));
    let requires_adapter = report
        .records
        .iter()
        .filter(|record| record.classification.kind == ClassificationKind::RequiresAdapter)
        .collect::<Vec<_>>();
    assert_eq!(requires_adapter.len(), 2);
    assert!(requires_adapter.iter().any(|record| record.asset_kind == "World"));
    assert!(requires_adapter.iter().any(|record| record.asset_kind == "MaterialInstanceConstant"));

    let mesh_bytes = fs::read(mesh_path)?;
    assert!(mesh_bytes.starts_with(b"Kaydara FBX Binary"));
    let texture_bytes = fs::read(texture_path)?;
    assert_eq!(texture_bytes.get(2), Some(&2));
    assert_eq!(texture_bytes.get(12..14), Some(&128_u16.to_le_bytes()[..]));
    assert_eq!(texture_bytes.get(14..16), Some(&128_u16.to_le_bytes()[..]));
    assert_eq!(texture_bytes.get(16), Some(&32));
    Ok(())
}
