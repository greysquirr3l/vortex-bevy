use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use vortex_core::{
    adapters::secondary::unity::{
        UnityProjectError, UnityProjectInspector, UnityProjectRequest, UnitySourceKind,
    },
    domain::SourceAssetPath,
};

/// Creates a symbolic link at `link` that points at `target`.
///
/// Windows only grants symlink creation to an administrator or to a
/// developer-mode install, so this can fail there. The caller decides what
/// to assert when it does.
fn create_file_symlink(target: &Path, link: &Path) -> Result<(), std::io::Error> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link)
    }
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_file(target, link)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (target, link);
        Err(std::io::Error::from(std::io::ErrorKind::Unsupported))
    }
}

fn temporary_project(label: &str) -> Result<PathBuf, Box<dyn Error>> {
    let root = std::env::temp_dir().join(format!("vortex-unity-{label}-{}", std::process::id()));
    if root.exists() {
        fs::remove_dir_all(&root)?;
    }
    fs::create_dir_all(root.join("Assets"))?;
    fs::create_dir_all(root.join("Packages"))?;
    fs::create_dir_all(root.join("ProjectSettings"))?;
    write_project_settings(&root, "2022.3.5f1", "2", true)?;
    Ok(root)
}

fn write_project_settings(
    root: &Path,
    version: &str,
    serialization_mode: &str,
    include_urp: bool,
) -> Result<(), Box<dyn Error>> {
    fs::write(
        root.join("ProjectSettings/ProjectVersion.txt"),
        format!("m_EditorVersion: {version}\n"),
    )?;
    fs::write(
        root.join("ProjectSettings/EditorSettings.asset"),
        format!("m_SerializationMode: {serialization_mode}\n"),
    )?;
    let dependencies = if include_urp {
        "\"com.unity.render-pipelines.universal\": \"12.1.11\""
    } else {
        "\"com.unity.textmeshpro\": \"3.0.6\""
    };
    fs::write(
        root.join("Packages/manifest.json"),
        format!("{{\"dependencies\": {{{dependencies}}}}}"),
    )?;
    Ok(())
}

fn write_meta(path: &Path, guid: &str) -> Result<(), Box<dyn Error>> {
    fs::write(
        PathBuf::from(format!("{}.meta", path.display())),
        format!("fileFormatVersion: 2\nguid: {guid}\n"),
    )?;
    Ok(())
}

fn selected(paths: &[&str]) -> Result<Vec<SourceAssetPath>, Box<dyn Error>> {
    paths.iter().map(|path| (*path).parse::<SourceAssetPath>().map_err(Into::into)).collect()
}

fn write_supported_assets(root: &Path) -> Result<(), Box<dyn Error>> {
    let scene = root.join("Assets/Crate.unity");
    fs::write(
        &scene,
        "%YAML 1.1\n--- !u!1 &100\nGameObject:\n  m_Name: Crate\n--- !u!4 &101\nTransform:\n  m_LocalPosition: {x: 0, y: 0, z: 0}\n--- !u!23 &102\nMeshRenderer:\n",
    )?;
    write_meta(&scene, "0123456789abcdef0123456789abcdef")?;

    let material = root.join("Assets/Crate.mat");
    fs::write(
        &material,
        "%YAML 1.1\n--- !u!21 &210\nMaterial:\n  m_Name: CrateMaterial\n  m_ShaderName: Universal Render Pipeline/Lit\n",
    )?;
    write_meta(&material, "fedcba9876543210fedcba9876543210")?;

    let texture = root.join("Assets/Crate.png");
    fs::write(&texture, [0_u8, 1, 2, 3])?;
    write_meta(&texture, "00112233445566778899aabbccddeeff")?;
    Ok(())
}

#[test]
fn self_authored_text_project_yields_stable_typed_records_and_source_links()
-> Result<(), Box<dyn Error>> {
    let root = temporary_project("stable")?;
    write_supported_assets(&root)?;
    let first_request = UnityProjectRequest::new(
        &root,
        selected(&["Assets/Crate.unity", "Assets/Crate.mat", "Assets/Crate.png"])?,
    )?;
    let second_request = UnityProjectRequest::new(
        &root,
        selected(&["Assets/Crate.png", "Assets/Crate.mat", "Assets/Crate.unity"])?,
    )?;
    let inspector = UnityProjectInspector;
    let first = inspector.inspect(&first_request)?;
    let second = inspector.inspect(&second_request)?;

    assert_eq!(first, second);
    assert_eq!(first.provenance.editor_version, "2022.3.5f1");
    assert_eq!(first.provenance.urp_package_version, "12.1.11");
    assert_eq!(first.provenance.serialization_mode, "force_text");
    assert_eq!(first.records.len(), 5);
    assert!(first.records.iter().any(|record| record.kind == UnitySourceKind::Material));
    assert_eq!(first.source_links()?, second.source_links()?);
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn missing_meta_duplicate_guid_and_symlink_escape_fail_before_publication()
-> Result<(), Box<dyn Error>> {
    let missing_meta_root = temporary_project("missing-meta")?;
    let scene = missing_meta_root.join("Assets/Missing.unity");
    fs::write(&scene, "%YAML 1.1\n--- !u!1 &1\nGameObject:\n")?;
    let request =
        UnityProjectRequest::new(&missing_meta_root, selected(&["Assets/Missing.unity"])?)?;
    assert!(matches!(
        UnityProjectInspector.inspect(&request),
        Err(UnityProjectError::MissingMeta { .. })
    ));
    fs::remove_dir_all(missing_meta_root)?;

    let duplicate_root = temporary_project("duplicate-guid")?;
    write_supported_assets(&duplicate_root)?;
    let duplicate = duplicate_root.join("Assets/Other.prefab");
    fs::write(&duplicate, "%YAML 1.1\n--- !u!1 &2\nGameObject:\n")?;
    write_meta(&duplicate, "0123456789abcdef0123456789abcdef")?;
    let request = UnityProjectRequest::new(
        &duplicate_root,
        selected(&["Assets/Crate.unity", "Assets/Other.prefab"])?,
    )?;
    assert!(matches!(
        UnityProjectInspector.inspect(&request),
        Err(UnityProjectError::DuplicateGuid { .. })
    ));
    fs::remove_dir_all(duplicate_root)?;

    let escape_root = temporary_project("escape")?;
    let external = std::env::temp_dir().join(format!("vortex-external-{}", std::process::id()));
    fs::write(&external, "%YAML 1.1\n--- !u!1 &3\nGameObject:\n")?;
    let link = escape_root.join("Assets/Escape.prefab");
    let symlinked = create_file_symlink(&external, &link);
    if symlinked.is_err() {
        // Windows only grants symlink creation to an administrator or to a
        // developer-mode install. Without that privilege the escape cannot be
        // constructed, so a copy stands in for it and has to stay confined
        // to the project rather than escape it.
        fs::copy(&external, &link)?;
    }
    write_meta(&link, "11223344556677889900aabbccddeeff")?;
    let request = UnityProjectRequest::new(&escape_root, selected(&["Assets/Escape.prefab"])?)?;
    if symlinked.is_ok() {
        assert!(matches!(
            UnityProjectInspector.inspect(&request),
            Err(UnityProjectError::PathEscape { .. })
        ));
    } else {
        assert!(!matches!(
            UnityProjectInspector.inspect(&request),
            Err(UnityProjectError::PathEscape { .. })
        ));
    }
    fs::remove_file(external)?;
    fs::remove_dir_all(escape_root)?;
    Ok(())
}

#[test]
fn unsupported_project_and_text_layouts_produce_typed_diagnostics() -> Result<(), Box<dyn Error>> {
    let root = temporary_project("unsupported")?;
    write_supported_assets(&root)?;
    let request = UnityProjectRequest::new(&root, selected(&["Assets/Crate.unity"])?)?;
    write_project_settings(&root, "2019.4.0f1", "2", true)?;
    assert!(matches!(
        UnityProjectInspector.inspect(&request),
        Err(UnityProjectError::UnsupportedEditorVersion { .. })
    ));

    write_project_settings(&root, "2022.3.5f1", "1", true)?;
    assert!(matches!(
        UnityProjectInspector.inspect(&request),
        Err(UnityProjectError::SerializationMode)
    ));

    write_project_settings(&root, "2022.3.5f1", "2", false)?;
    assert!(matches!(
        UnityProjectInspector.inspect(&request),
        Err(UnityProjectError::MissingUrpPackage)
    ));

    write_project_settings(&root, "2022.3.5f1", "2", true)?;
    fs::write(root.join("Assets/Crate.unity"), [0_u8, 3, 4])?;
    assert!(matches!(
        UnityProjectInspector.inspect(&request),
        Err(UnityProjectError::BinaryOrUnknownSerialization { .. })
    ));

    fs::write(root.join("Assets/Crate.unity"), "%YAML 1.1\n--- !u!bad &1\nGameObject:\n")?;
    assert!(matches!(
        UnityProjectInspector.inspect(&request),
        Err(UnityProjectError::InvalidObjectHeader { .. })
    ));

    fs::write(
        root.join("Assets/Crate.unity"),
        "%YAML 1.1\n--- !u!1 &1\nGameObject:\n  m_Path: ../outside\n",
    )?;
    assert!(matches!(
        UnityProjectInspector.inspect(&request),
        Err(UnityProjectError::UnsafeReference { .. })
    ));
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn shader_graph_material_is_explicitly_unsupported() -> Result<(), Box<dyn Error>> {
    let root = temporary_project("shader-graph")?;
    let material = root.join("Assets/Graph.mat");
    fs::write(
        &material,
        "%YAML 1.1\n--- !u!21 &99\nMaterial:\n  m_ShaderName: Shader Graphs/Crate\n",
    )?;
    write_meta(&material, "abcdefabcdefabcdefabcdefabcdefab")?;
    let request = UnityProjectRequest::new(&root, selected(&["Assets/Graph.mat"])?)?;
    let snapshot = UnityProjectInspector.inspect(&request)?;
    let record = snapshot.records.first().ok_or("missing material record")?;
    assert_eq!(format!("{:?}", record.classification.kind), "Unsupported");
    fs::remove_dir_all(root)?;
    Ok(())
}
