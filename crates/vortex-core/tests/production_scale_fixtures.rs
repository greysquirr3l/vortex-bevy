use std::{error::Error, fs, path::PathBuf};

use vortex_core::{
    adapters::secondary::{
        texture::{TextureInput, TextureNormalizer, TextureRole, TextureSamplerInput},
        usd_stage::{UsdStageInspector, UsdStageRequest},
    },
    domain::{ArtifactHash, ClassificationKind, SourceAssetPath, scene_ir::TextureId},
};

const STANDARD_SHADER_BALL_SHA256: &str =
    "fafad41722900d71dd562fa49c226cf05c9650a8c1bc2cabfa0d99717eb4d109";
const WOODEN_PLANKS_SHA256: &str =
    "6b08165dfa7a1a5d662436cefc4566d1161540d17b5fc1314a7bfb4ee985ab2f";

fn fixture_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/production_scale").join(relative)
}

#[test]
fn production_usd_layer_is_deterministic_and_preserves_requires_adapter_boundary()
-> Result<(), Box<dyn Error>> {
    let root = fixture_path("standard_shader_ball");
    let bytes = fs::read(root.join("shaderball.usda"))?;
    assert_eq!(ArtifactHash::from_bytes(&bytes).as_str(), STANDARD_SHADER_BALL_SHA256);
    assert!(bytes.len() > 3 * 1024 * 1024);

    let first = UsdStageRequest::new(
        &root,
        "shaderball.usda".parse::<SourceAssetPath>()?,
        "context=universal;payload=forbid;purpose=render;time=stage",
    )?;
    let second = UsdStageRequest::new(&root, "shaderball.usda".parse::<SourceAssetPath>()?, "")?;
    let inspector = UsdStageInspector;
    let first_snapshot = inspector.resolve(&first)?;
    let second_snapshot = inspector.resolve(&second)?;

    assert_eq!(first_snapshot, second_snapshot);
    assert_eq!(first_snapshot.classification.kind, ClassificationKind::RequiresAdapter);
    assert!(first_snapshot.facts.iter().any(|fact| fact == "stage.bytes=3354071"));
    assert!(first_snapshot.prims.len() > 100);
    assert!(
        first_snapshot.facts.iter().any(|fact| fact.starts_with("composition.variant.unselected="))
    );
    Ok(())
}

#[test]
fn production_texture_is_normalized_with_stable_metadata_and_content_hash()
-> Result<(), Box<dyn Error>> {
    let bytes = fs::read(fixture_path("polyhaven_wooden_planks/wooden_planks_diff_1k.png"))?;
    assert_eq!(ArtifactHash::from_bytes(&bytes).as_str(), WOODEN_PLANKS_SHA256);
    assert!(bytes.len() > 4 * 1024 * 1024);

    let normalized = TextureNormalizer.normalize([TextureInput {
        id: TextureId::new(91),
        source: "Textures/PolyHaven/wooden_planks_diff_1k.png".parse()?,
        role: TextureRole::BaseColor,
        sampler: TextureSamplerInput::linear_repeat(),
        bytes,
    }])?;

    assert_eq!(normalized.staged.len(), 1);
    let staged = normalized.staged.values().next().ok_or("missing normalized texture")?;
    assert_eq!((staged.width, staged.height), (1024, 1024));
    assert_eq!(staged.checksum.as_str(), WOODEN_PLANKS_SHA256);
    assert_eq!(staged.bytes.len(), 5_016_652);
    assert_eq!(normalized.textures.len(), 1);
    assert_eq!(normalized.samplers.len(), 1);
    Ok(())
}
