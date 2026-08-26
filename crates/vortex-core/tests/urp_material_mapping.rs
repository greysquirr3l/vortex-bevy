use std::{collections::BTreeSet, error::Error};

use vortex_core::{
    adapters::secondary::{
        texture::TextureRole,
        urp_material::{
            UrpMaterialError, UrpMaterialInput, UrpMaterialMapper, UrpShader, UrpSurface,
            UrpTextureBinding, UrpUnsupportedFeature,
        },
    },
    domain::{
        ClassificationKind, ProjectRoot, SourceAssetPath, SourceIdentity, UnityAssetGuid,
        UnityObjectId,
        scene_ir::{AlphaMode, MaterialId, TextureId, TextureTransform},
    },
};

fn unity_source() -> Result<SourceIdentity, Box<dyn Error>> {
    Ok(SourceIdentity::Unity {
        project_root: "fixture".parse::<ProjectRoot>()?,
        asset_path: "Assets/Crate.mat".parse::<SourceAssetPath>()?,
        asset_guid: "0123456789abcdef0123456789abcdef".parse::<UnityAssetGuid>()?,
        object_id: UnityObjectId::new(210),
    })
}

fn assert_close(actual: f32, expected: f32) {
    assert!((actual - expected).abs() <= f32::EPSILON);
}

fn baseline(shader: UrpShader) -> Result<UrpMaterialInput, Box<dyn Error>> {
    Ok(UrpMaterialInput {
        id: MaterialId::new(1),
        source: unity_source()?,
        shader,
        base_color: [0.5, 0.25, 0.75, 0.8],
        metallic: 0.4,
        smoothness: 0.25,
        normal_scale: 0.8,
        occlusion_strength: 0.7,
        emissive: [0.1, 0.2, 0.3],
        surface: UrpSurface::Transparent,
        alpha_clip: true,
        alpha_cutoff: Some(0.4),
        double_sided: true,
        texture_transform: TextureTransform { scale: [2.0, 3.0], offset: [0.25, 0.5] },
        textures: vec![
            UrpTextureBinding { role: TextureRole::BaseColor, texture: TextureId::new(1) },
            UrpTextureBinding { role: TextureRole::Normal, texture: TextureId::new(2) },
            UrpTextureBinding { role: TextureRole::MetallicRoughness, texture: TextureId::new(3) },
        ],
        unsupported_features: BTreeSet::new(),
    })
}

#[test]
fn lit_and_unlit_baselines_map_to_deterministic_source_neutral_pbr() -> Result<(), Box<dyn Error>> {
    let lit = UrpMaterialMapper.map(&baseline(UrpShader::Lit)?)?;
    let material = lit.material.ok_or("missing Lit material")?;
    assert_eq!(lit.classification.kind, ClassificationKind::Converted);
    assert_close(material.roughness, 0.75);
    assert_close(material.metallic, 0.4);
    assert_eq!(material.alpha_mode, AlphaMode::Mask);
    assert_eq!(material.alpha_cutoff, Some(0.4));
    assert!(material.double_sided);
    assert!(
        material
            .texture_transform
            .scale
            .iter()
            .zip([2.0_f32, 3.0])
            .all(|(actual, expected)| (*actual - expected).abs() <= f32::EPSILON)
    );
    assert_eq!(material.textures.get("base_color"), Some(&TextureId::new(1)));
    assert_eq!(
        lit.provenance.get("unity.urp.shader"),
        Some(&UrpShader::Lit.source_name().to_owned())
    );

    let mut unlit_input = baseline(UrpShader::Unlit)?;
    unlit_input.alpha_clip = false;
    unlit_input.alpha_cutoff = None;
    unlit_input.surface = UrpSurface::Opaque;
    let unlit = UrpMaterialMapper.map(&unlit_input)?;
    let unlit_material = unlit.material.ok_or("missing Unlit material")?;
    assert_close(unlit_material.metallic, 0.0);
    assert_close(unlit_material.roughness, 1.0);
    assert_eq!(unlit_material.alpha_mode, AlphaMode::Opaque);
    Ok(())
}

#[test]
fn unsupported_features_and_invalid_source_facts_are_never_approximated()
-> Result<(), Box<dyn Error>> {
    let mut unsupported = baseline(UrpShader::Lit)?;
    unsupported.unsupported_features.insert(UrpUnsupportedFeature::ShaderGraph);
    unsupported.unsupported_features.insert(UrpUnsupportedFeature::ClearCoat);
    let mapped = UrpMaterialMapper.map(&unsupported)?;
    assert!(mapped.material.is_none());
    assert_eq!(mapped.classification.kind, ClassificationKind::RequiresAdapter);
    assert!(mapped.classification.reason.contains("clear_coat"));
    assert!(mapped.classification.reason.contains("shader_graph"));

    let mut invalid = baseline(UrpShader::Lit)?;
    invalid.smoothness = f32::NAN;
    assert!(matches!(
        UrpMaterialMapper.map(&invalid),
        Err(UrpMaterialError::InvalidNumber { property: "smoothness", .. })
    ));

    let mut duplicate = baseline(UrpShader::Lit)?;
    duplicate
        .textures
        .push(UrpTextureBinding { role: TextureRole::BaseColor, texture: TextureId::new(8) });
    assert!(matches!(
        UrpMaterialMapper.map(&duplicate),
        Err(UrpMaterialError::DuplicateTextureRole { role: "base_color", .. })
    ));
    Ok(())
}
