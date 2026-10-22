//! Owned-fixture benchmark for verified manifest loading and headless Bevy scene-root spawn.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fs,
    path::PathBuf,
    process,
};

use criterion::{BatchSize, Criterion, black_box};
use vortex_bevy::runtime::{build_runtime_app, load_manifest_and_spawn};
use vortex_core::domain::{
    ArtifactHash, ClassificationKind, CompatibilityCapability, CompatibilityRequirements,
    ConversionClassification, ManifestArtifact, ManifestArtifactKind, ManifestSource, ProjectRoot,
    Remediation, SceneManifest, SourceAssetPath, SourceIdentity, UnrealObjectPath,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("T27 runtime benchmark setup failed: {error}");
        process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new()?;
    let mut preflight_app = build_runtime_app(fixture.asset_root.to_string_lossy().into_owned());
    let _root = load_manifest_and_spawn(&mut preflight_app, &fixture.manifest_path)?;
    measure_runtime_fixture(&fixture);
    Ok(())
}

fn measure_runtime_fixture(fixture: &RuntimeFixture) {
    let mut criterion = Criterion::default();
    criterion.bench_function("runtime/build_app_verify_manifest_and_spawn_scene_root", |bencher| {
        bencher.iter_batched(
            || build_runtime_app(fixture.asset_root.to_string_lossy().into_owned()),
            |mut app| black_box(load_manifest_and_spawn(&mut app, &fixture.manifest_path)),
            BatchSize::SmallInput,
        );
    });
    criterion.final_summary();
}

struct RuntimeFixture {
    asset_root: PathBuf,
    manifest_path: PathBuf,
}

impl Drop for RuntimeFixture {
    fn drop(&mut self) {
        let _ignored = fs::remove_dir_all(&self.asset_root);
    }
}

impl RuntimeFixture {
    fn new() -> Result<Self, Box<dyn Error>> {
        let asset_root =
            std::env::temp_dir().join(format!("vortex-t27-runtime-bench-{}", process::id()));
        if asset_root.exists() {
            fs::remove_dir_all(&asset_root)?;
        }
        fs::create_dir_all(&asset_root)?;
        let glb = minimal_glb()?;
        fs::write(asset_root.join("scene.glb"), &glb)?;
        let manifest_path = asset_root.join("scene.manifest.ron");
        fs::write(&manifest_path, manifest(&glb)?.to_ron()?)?;
        Ok(Self { asset_root, manifest_path })
    }
}

fn minimal_glb() -> Result<Vec<u8>, Box<dyn Error>> {
    let mut json = br#"{"asset":{"version":"2.0"},"scene":0,"scenes":[{"nodes":[0]}],"nodes":[{"name":"OwnedBenchTriangle"}]}"#.to_vec();
    while !json.len().is_multiple_of(4) {
        json.push(b' ');
    }
    let json_len = u32::try_from(json.len())?;
    let total_len = 20_u32.checked_add(json_len).ok_or("owned GLB length overflow")?;
    let mut glb = Vec::with_capacity(usize::try_from(total_len)?);
    for field in [0x4654_6c67_u32, 2, total_len, json_len, 0x4e4f_534a] {
        glb.extend_from_slice(&field.to_le_bytes());
    }
    glb.extend_from_slice(&json);
    Ok(glb)
}

fn manifest(glb: &[u8]) -> Result<SceneManifest, Box<dyn Error>> {
    let source = SourceIdentity::Unreal {
        project_root: "OwnedBenchProject".parse::<ProjectRoot>()?,
        asset_path: "Content/OwnedBench.uasset".parse::<SourceAssetPath>()?,
        object_path: "/Game/OwnedBench.OwnedBench".parse::<UnrealObjectPath>()?,
    };
    SceneManifest::new(
        "0.1.0",
        ArtifactHash::from_bytes(b"t27-runtime-profile"),
        vec![ManifestArtifact {
            kind: ManifestArtifactKind::Glb,
            path: "scene.glb".to_owned(),
            hash: ArtifactHash::from_bytes(glb),
        }],
        vec![ManifestSource {
            identity: source,
            asset_kind: "static_mesh".to_owned(),
            selected_values: BTreeMap::new(),
            glb_labels: BTreeMap::new(),
            classification: ConversionClassification::new(
                ClassificationKind::Converted,
                "owned runtime benchmark fixture",
                Remediation::new("Use the owned benchmark fixture.")?,
            )?,
            diagnostics: Vec::new(),
        }],
        CompatibilityRequirements {
            minimum_bevy_version: "0.20".to_owned(),
            required_capabilities: BTreeSet::from([CompatibilityCapability::BevyGltfPbr]),
        },
    )
    .map_err(Into::into)
}
