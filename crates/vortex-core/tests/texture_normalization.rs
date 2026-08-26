use std::error::Error;

use vortex_core::{
    adapters::secondary::texture::{
        TextureInput, TextureNormalizationError, TextureNormalizer, TextureRole,
        TextureSamplerInput,
    },
    domain::{
        SourceAssetPath,
        scene_ir::{TextureColorSpace, TextureId},
    },
};

fn png_chunk(kind: [u8; 4], payload: &[u8]) -> Vec<u8> {
    let length = u32::try_from(payload.len()).ok().map_or([0_u8; 4], u32::to_be_bytes);
    let mut chunk = Vec::new();
    chunk.extend(length);
    chunk.extend(kind);
    chunk.extend(payload);
    chunk.extend([0_u8; 4]);
    chunk
}

fn png(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = vec![137, 80, 78, 71, 13, 10, 26, 10];
    let mut ihdr = Vec::new();
    ihdr.extend(width.to_be_bytes());
    ihdr.extend(height.to_be_bytes());
    ihdr.extend([8, 6, 0, 0, 0]);
    bytes.extend(png_chunk(*b"IHDR", &ihdr));
    bytes.extend(png_chunk(*b"IDAT", &[0]));
    bytes.extend(png_chunk(*b"IEND", &[]));
    bytes
}

fn input(
    id: u32,
    source: &str,
    role: TextureRole,
    bytes: Vec<u8>,
) -> Result<TextureInput, Box<dyn Error>> {
    Ok(TextureInput {
        id: TextureId::new(id),
        source: source.parse::<SourceAssetPath>()?,
        role,
        sampler: TextureSamplerInput::linear_repeat(),
        bytes,
    })
}

#[test]
fn content_addressed_pngs_deduplicate_and_preserve_role_color_space() -> Result<(), Box<dyn Error>>
{
    let bytes = png(2, 3);
    let result = TextureNormalizer.normalize([
        input(2, "Assets/normal.png", TextureRole::Normal, bytes.clone())?,
        input(1, "Assets/albedo.png", TextureRole::BaseColor, bytes)?,
    ])?;

    assert_eq!(result.staged.len(), 1);
    assert_eq!(result.textures.len(), 2);
    let first = result.textures.first().ok_or("missing first texture")?;
    let second = result.textures.get(1).ok_or("missing second texture")?;
    assert_eq!(first.id, TextureId::new(1));
    assert_eq!(first.color_space, TextureColorSpace::Srgb);
    assert_eq!(second.id, TextureId::new(2));
    assert_eq!(second.color_space, TextureColorSpace::Linear);
    assert!(first.image.starts_with("textures/"));
    assert!(
        std::path::Path::new(&first.image)
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("png"))
    );
    assert!(!first.image.contains("albedo"));
    let staged = result.staged.values().next().ok_or("missing staged texture")?;
    assert_eq!(staged.width, 2);
    assert_eq!(staged.height, 3);
    assert_eq!(staged.bytes, png(2, 3));
    Ok(())
}

#[test]
fn duplicate_ids_and_malformed_or_truncated_pngs_fail_without_staging() -> Result<(), Box<dyn Error>>
{
    let bytes = png(1, 1);
    assert!(matches!(
        TextureNormalizer.normalize([
            input(1, "Assets/a.png", TextureRole::BaseColor, bytes.clone())?,
            input(1, "Assets/b.png", TextureRole::Emissive, bytes.clone())?,
        ]),
        Err(TextureNormalizationError::DuplicateTextureId { id: 1 })
    ));

    assert!(matches!(
        TextureNormalizer.normalize([input(
            2,
            "Assets/bad.png",
            TextureRole::BaseColor,
            vec![1, 2, 3]
        )?]),
        Err(TextureNormalizationError::UnsupportedImage { .. })
    ));
    let mut truncated = bytes;
    truncated.pop();
    assert!(matches!(
        TextureNormalizer.normalize([input(
            3,
            "Assets/truncated.png",
            TextureRole::BaseColor,
            truncated
        )?]),
        Err(TextureNormalizationError::InvalidPngHeader { .. })
    ));
    assert!(matches!(
        TextureNormalizer.normalize([input(
            4,
            "Assets/huge.png",
            TextureRole::BaseColor,
            png(16_385, 1)
        )?]),
        Err(TextureNormalizationError::DimensionsExceeded { .. })
    ));
    Ok(())
}
