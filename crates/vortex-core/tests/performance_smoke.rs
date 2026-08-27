use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
    process,
    time::{Duration, Instant},
};

use vortex_core::adapters::secondary::usd_stage::{UsdStageInspector, UsdStageRequest};

const STAGE_FIXTURE: &str = include_str!("../../../fixtures/minimal_usda/triangle_scene.usda");
const RESOLUTION_ITERATIONS: usize = 64;
const GENEROUS_MAX_ELAPSED: Duration = Duration::from_secs(3);

struct OwnedFixtureDirectory(PathBuf);

impl OwnedFixtureDirectory {
    fn new() -> Result<Self, Box<dyn Error>> {
        let root = std::env::temp_dir().join(format!("vortex-t27-smoke-{}", process::id()));
        if root.exists() {
            fs::remove_dir_all(&root)?;
        }
        fs::create_dir_all(&root)?;
        fs::write(root.join("triangle_scene.usda"), STAGE_FIXTURE)?;
        Ok(Self(root))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for OwnedFixtureDirectory {
    fn drop(&mut self) {
        let _ignored = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn owned_usda_resolution_stays_within_generous_hardware_aware_smoke_budget()
-> Result<(), Box<dyn Error>> {
    let fixture = OwnedFixtureDirectory::new()?;
    let request = UsdStageRequest::new(fixture.path(), "triangle_scene.usda".parse()?, "")?;
    let inspector = UsdStageInspector;
    let started_at = Instant::now();

    for _ in 0..RESOLUTION_ITERATIONS {
        let facts = inspector.resolve(&request)?;
        assert!(!facts.prims.is_empty());
    }

    let elapsed = started_at.elapsed();
    assert!(
        elapsed <= GENEROUS_MAX_ELAPSED,
        "{RESOLUTION_ITERATIONS} owned USDA resolutions took {elapsed:?}, exceeding the generous {GENEROUS_MAX_ELAPSED:?} smoke budget"
    );
    Ok(())
}
