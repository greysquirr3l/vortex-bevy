#![cfg(target_os = "linux")]

use std::{alloc::System, error::Error, path::PathBuf};

use stats_alloc::{INSTRUMENTED_SYSTEM, Region, StatsAlloc};
use vortex_core::{
    adapters::secondary::{
        linux_process_memory::LinuxProcessMemoryReader,
        texture::{TextureInput, TextureNormalizer, TextureRole, TextureSamplerInput},
        usd_stage::{UsdStageInspector, UsdStageRequest},
    },
    domain::{SourceAssetPath, scene_ir::TextureId},
};

const PRODUCTION_TEXTURE: &[u8] = include_bytes!(
    "../../../fixtures/production_scale/polyhaven_wooden_planks/wooden_planks_diff_1k.png"
);

#[global_allocator]
static GLOBAL: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

fn fixture_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/production_scale").join(relative)
}

#[test]
fn standard_shader_ball_stage_records_nonzero_observations() -> Result<(), Box<dyn Error>> {
    let reader = LinuxProcessMemoryReader;
    let before = reader.snapshot()?;
    let request = UsdStageRequest::new(
        fixture_path("standard_shader_ball"),
        "shaderball.usda".parse::<SourceAssetPath>()?,
        "",
    )?;
    let region = Region::new(GLOBAL);
    let snapshot = UsdStageInspector.resolve(&request)?;
    let allocation = region.change();
    let during = reader.snapshot()?;

    assert!(snapshot.prims.len() > 100);
    assert!(allocation.allocations > 0);
    assert!(allocation.bytes_allocated > 0);
    assert!(during.peak_resident_bytes() >= during.resident_bytes());
    assert!(before.peak_resident_bytes() <= during.peak_resident_bytes());
    Ok(())
}

#[test]
fn polyhaven_texture_records_nonzero_observations() -> Result<(), Box<dyn Error>> {
    let reader = LinuxProcessMemoryReader;
    let before = reader.snapshot()?;
    let region = Region::new(GLOBAL);
    let normalized = TextureNormalizer.normalize([TextureInput {
        id: TextureId::new(93),
        source: "Textures/PolyHaven/wooden_planks_diff_1k.png".parse()?,
        role: TextureRole::BaseColor,
        sampler: TextureSamplerInput::linear_repeat(),
        bytes: PRODUCTION_TEXTURE.to_vec(),
    }])?;
    let allocation = region.change();
    let during = reader.snapshot()?;

    assert_eq!(normalized.staged.len(), 1);
    assert!(allocation.allocations > 0);
    assert!(allocation.bytes_allocated >= PRODUCTION_TEXTURE.len());
    assert!(during.peak_resident_bytes() >= during.resident_bytes());
    assert!(before.peak_resident_bytes() <= during.peak_resident_bytes());
    Ok(())
}
