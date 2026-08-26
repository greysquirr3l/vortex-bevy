use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
};

use vortex_core::{
    adapters::secondary::usd_stage::{UsdStageError, UsdStageInspector, UsdStageRequest},
    domain::{ClassificationKind, SourceAssetPath},
};

static NEXT_DIRECTORY: AtomicUsize = AtomicUsize::new(0);

struct TestDirectory {
    path: PathBuf,
}

impl TestDirectory {
    fn new(label: &str) -> Result<Self, std::io::Error> {
        let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir()
            .join(format!("vortex-usd-stage-{label}-{}-{sequence}", std::process::id()));
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

const USDA_STAGE: &str = r#"#usda 1.0
(
    metersPerUnit = 1
    upAxis = "Z"
    subLayers = [@./base.usda@]
)
def Xform "World" (
    references = @./assembly.usda@</Assembly>
    payload = @./payload.usda@
) {
    uniform token purpose = "render"
    token visibility = "invisible"
    bool instanceable = true
    double3 xformOp:translate = (1, 2, 3)
    def Mesh "Cube" {
    }
    variantSet "look" = {
        "red" {
        }
    }
}
"#;

fn stage_path(value: &str) -> Result<SourceAssetPath, Box<dyn Error>> {
    Ok(value.parse()?)
}

fn write_stage(
    directory: &TestDirectory,
    relative: &str,
    contents: &[u8],
) -> Result<(), Box<dyn Error>> {
    let path = directory.path().join(relative);
    let parent = path.parent().ok_or("stage path has no parent")?;
    fs::create_dir_all(parent)?;
    fs::write(path, contents)?;
    Ok(())
}

#[test]
fn selected_usda_stage_is_deterministic_and_preserves_composition_facts()
-> Result<(), Box<dyn Error>> {
    let directory = TestDirectory::new("selected-stage")?;
    write_stage(&directory, "Scenes/root.usda", USDA_STAGE.as_bytes())?;
    let first = UsdStageRequest::new(
        directory.path(),
        stage_path("Scenes/root.usda")?,
        "variants=look:red,lod:high;payload=metadata_only;purpose=render;time=0:48;context=universal",
    )?;
    let second = UsdStageRequest::new(
        directory.path(),
        stage_path("Scenes/root.usda")?,
        "context=universal;time=0:48;purpose=render;payload=metadata_only;variants=lod:high,look:red",
    )?;
    let inspector = UsdStageInspector;
    let first_snapshot = inspector.resolve(&first)?;
    let second_snapshot = inspector.resolve(&second)?;

    assert_eq!(first.selection(), second.selection());
    assert_eq!(first_snapshot, second_snapshot);
    assert_eq!(first_snapshot.classification.kind, ClassificationKind::RequiresAdapter);
    assert!(first_snapshot.facts.iter().any(|fact| fact == "selection.variant.look=red"));
    assert!(first_snapshot.facts.iter().any(|fact| fact == "selection.variant.lod=high"));
    assert!(
        first_snapshot
            .facts
            .iter()
            .any(|fact| fact == "composition.payload.unselected=./payload.usda")
    );
    assert!(
        first_snapshot.facts.iter().any(|fact| fact == "composition.reference=./assembly.usda")
    );
    assert!(first_snapshot.facts.iter().any(|fact| fact == "composition.sublayer=./base.usda"));
    assert!(
        first_snapshot.facts.iter().any(|fact| fact == "normalization.up_axis=z_to_y_pending_ir")
    );
    assert_eq!(first_snapshot.prims.len(), 2);
    let world = first_snapshot.prims.first().ok_or("missing World prim")?;
    assert_eq!(world.path.as_str(), "/World");
    assert_eq!(world.purpose.as_deref(), Some("render"));
    assert_eq!(world.visibility.as_deref(), Some("invisible"));
    assert!(world.instancable);
    assert_eq!(world.transforms.len(), 1);
    let cube = first_snapshot.prims.get(1).ok_or("missing Cube prim")?;
    assert_eq!(cube.path.as_str(), "/World/Cube");
    Ok(())
}

#[test]
fn malformed_metadata_and_structure_return_typed_errors_without_snapshot()
-> Result<(), Box<dyn Error>> {
    let directory = TestDirectory::new("malformed")?;
    write_stage(
        &directory,
        "Scenes/axis.usda",
        b"#usda 1.0\n(\n upAxis = \"X\"\n)\ndef Xform \"World\" {\n}\n",
    )?;
    write_stage(&directory, "Scenes/brace.usda", b"#usda 1.0\ndef Xform \"World\" {\n")?;
    let inspector = UsdStageInspector;
    let invalid_axis = UsdStageRequest::new(directory.path(), stage_path("Scenes/axis.usda")?, "")?;
    let invalid_brace =
        UsdStageRequest::new(directory.path(), stage_path("Scenes/brace.usda")?, "")?;

    assert!(matches!(inspector.resolve(&invalid_axis), Err(UsdStageError::InvalidMetadata { .. })));
    assert!(matches!(
        inspector.resolve(&invalid_brace),
        Err(UsdStageError::MalformedStructure { .. })
    ));
    Ok(())
}

#[test]
fn binary_usdc_is_an_explicit_authoritative_runtime_requirement() -> Result<(), Box<dyn Error>> {
    let directory = TestDirectory::new("binary")?;
    write_stage(&directory, "Scenes/root.usdc", b"PXR-USDC-binary")?;
    let request = UsdStageRequest::new(directory.path(), stage_path("Scenes/root.usdc")?, "")?;

    let snapshot = UsdStageInspector.resolve(&request)?;

    assert_eq!(snapshot.classification.kind, ClassificationKind::RequiresAdapter);
    assert!(
        snapshot
            .facts
            .iter()
            .any(|fact| fact == "stage.composition=requires_authoritative_runtime")
    );
    assert!(snapshot.prims.is_empty());
    Ok(())
}

#[cfg(unix)]
#[test]
fn canonicalized_stage_symlink_escape_is_rejected() -> Result<(), Box<dyn Error>> {
    use std::os::unix::fs::symlink;

    let root = TestDirectory::new("symlink-root")?;
    let outside = TestDirectory::new("symlink-outside")?;
    write_stage(&outside, "outside.usda", USDA_STAGE.as_bytes())?;
    fs::create_dir_all(root.path().join("Scenes"))?;
    symlink(outside.path().join("outside.usda"), root.path().join("Scenes/escaped.usda"))?;
    let request = UsdStageRequest::new(root.path(), stage_path("Scenes/escaped.usda")?, "")?;

    assert!(matches!(UsdStageInspector.resolve(&request), Err(UsdStageError::PathEscape { .. })));
    Ok(())
}
