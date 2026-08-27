use std::{
    env,
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use vortex_core::{
    adapters::secondary::unreal_texture2d_metadata::UnrealTexture2DMetadataDecoder,
    domain::{ProjectRoot, SourceAssetPath, SourceIdentity, UnrealObjectPath},
};

fn environment_path(name: &str) -> Result<PathBuf, Box<dyn Error>> {
    let value = env::var_os(name).ok_or_else(|| format!("missing required {name}"))?;
    let path = PathBuf::from(value).canonicalize()?;
    if !path.is_file() {
        return Err(format!("{name} must name a regular file: {}", path.display()).into());
    }
    Ok(path)
}

fn optional_environment_path(name: &str) -> Result<Option<PathBuf>, Box<dyn Error>> {
    let Some(value) = env::var_os(name) else {
        return Ok(None);
    };
    let path = PathBuf::from(value).canonicalize()?;
    if !path.is_file() {
        return Err(format!("{name} must name a regular file: {}", path.display()).into());
    }
    Ok(Some(path))
}

fn source_identity(project: &Path, package: &Path) -> Result<SourceIdentity, Box<dyn Error>> {
    let project = project.canonicalize()?;
    if !project.is_dir() {
        return Err(format!("project root must be a directory: {}", project.display()).into());
    }
    let package = package.canonicalize()?;
    let relative = package.strip_prefix(&project)?;
    let root_name = project
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or("project root has no UTF-8 terminal name")?;
    let object = env::var("VORTEX_UNREAL_TEXTURE_OBJECT")?;
    Ok(SourceIdentity::Unreal {
        project_root: root_name.parse::<ProjectRoot>()?,
        asset_path: relative.to_string_lossy().replace('\\', "/").parse::<SourceAssetPath>()?,
        object_path: object.parse::<UnrealObjectPath>()?,
    })
}

/// Compares a received authorized UE 5.8.2 `UTexture2D` package to the external Asset Registry oracle.
///
/// This remains ignored because the package, optional companion, and oracle report are supplied
/// outside the repository. It writes no project or staging files and retains no received bytes.
#[test]
#[ignore = "requires explicit paths to authorized UE 5.8.2 Texture2D package and oracle report"]
fn received_unreal_texture_metadata_matches_authorized_oracle() -> Result<(), Box<dyn Error>> {
    let project = env::var_os("VORTEX_UNREAL_TEXTURE_PROJECT_ROOT")
        .ok_or("missing required VORTEX_UNREAL_TEXTURE_PROJECT_ROOT")?;
    let package = environment_path("VORTEX_UNREAL_TEXTURE_UASSET")?;
    let oracle = environment_path("VORTEX_UNREAL_TEXTURE_ORACLE")?;
    let companion = optional_environment_path("VORTEX_UNREAL_TEXTURE_UBULK")?;
    let source = source_identity(Path::new(&project), &package)?;
    let package_bytes = fs::read(package)?;
    let companion_bytes = companion.map(fs::read).transpose()?;
    let oracle_text = fs::read_to_string(oracle)?;
    let decoder = UnrealTexture2DMetadataDecoder;
    let metadata = decoder.decode(&source, &package_bytes, companion_bytes.as_deref())?;
    let expected = decoder.parse_oracle_text(&source, &oracle_text)?;
    assert!(expected.matches(&metadata));
    Ok(())
}
