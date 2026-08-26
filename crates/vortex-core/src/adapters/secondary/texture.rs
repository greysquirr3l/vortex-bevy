//! Bounded, deterministic texture normalization for portable PBR package inputs.

use std::collections::{BTreeMap, BTreeSet};

use thiserror::Error;

use crate::domain::{
    ArtifactHash, SourceAssetPath,
    scene_ir::{Sampler, Texture, TextureColorSpace, TextureFilter, TextureId, TextureWrap},
};

const MAX_IMAGE_BYTES: usize = 8 * 1024 * 1024;
const MAX_IMAGE_DIMENSION: u32 = 16_384;
const MAX_IMAGE_PIXELS: u64 = 64 * 1024 * 1024;
const PNG_SIGNATURE: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];

/// Portable material texture role used to derive a required color-space intent.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum TextureRole {
    /// Base-color albedo data.
    BaseColor,
    /// Emissive color data.
    Emissive,
    /// Normal-vector data.
    Normal,
    /// Metallic-roughness packed data.
    MetallicRoughness,
    /// Ambient-occlusion data.
    Occlusion,
}

impl TextureRole {
    /// Returns the portable color-space interpretation for this texture role.
    #[must_use]
    pub const fn color_space(self) -> TextureColorSpace {
        match self {
            Self::BaseColor | Self::Emissive => TextureColorSpace::Srgb,
            Self::Normal | Self::MetallicRoughness | Self::Occlusion => TextureColorSpace::Linear,
        }
    }

    /// Returns the deterministic canonical material-slot token.
    #[must_use]
    pub const fn slot(self) -> &'static str {
        match self {
            Self::BaseColor => "base_color",
            Self::Emissive => "emissive",
            Self::Normal => "normal",
            Self::MetallicRoughness => "metallic_roughness",
            Self::Occlusion => "occlusion",
        }
    }
}

/// Explicit sampler choices supplied by a source adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TextureSamplerInput {
    /// Magnification filter.
    pub mag_filter: TextureFilter,
    /// Optional minification filter.
    pub min_filter: Option<TextureFilter>,
    /// U-axis wrapping.
    pub wrap_s: TextureWrap,
    /// V-axis wrapping.
    pub wrap_t: TextureWrap,
}

impl TextureSamplerInput {
    /// Returns a deterministic standard linear-repeat sampler.
    #[must_use]
    pub const fn linear_repeat() -> Self {
        Self {
            mag_filter: TextureFilter::Linear,
            min_filter: Some(TextureFilter::Linear),
            wrap_s: TextureWrap::Repeat,
            wrap_t: TextureWrap::Repeat,
        }
    }
}

/// One bounded source image supplied by an engine adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextureInput {
    /// Canonical texture ID selected by the adapter.
    pub id: TextureId,
    /// Typed logical source location retained only for deterministic ordering/provenance.
    pub source: SourceAssetPath,
    /// Intended material role.
    pub role: TextureRole,
    /// Explicit portable sampler choice.
    pub sampler: TextureSamplerInput,
    /// Already acquired image bytes; the normalizer does not read arbitrary filesystem paths.
    pub bytes: Vec<u8>,
}

/// Content-addressed staged texture result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StagedTexture {
    /// Canonical IR texture descriptor.
    pub texture: Texture,
    /// Canonical IR sampler descriptor.
    pub sampler: Sampler,
    /// Validated PNG width.
    pub width: u32,
    /// Validated PNG height.
    pub height: u32,
    /// SHA-256 hash of the original PNG bytes.
    pub checksum: ArtifactHash,
    /// Exact source PNG payload to be atomically published by the artifact adapter.
    pub bytes: Vec<u8>,
}

/// Deterministic normalized texture collection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextureNormalization {
    /// Unique staged images keyed by content checksum.
    pub staged: BTreeMap<ArtifactHash, StagedTexture>,
    /// Typed texture descriptors in ascending texture-ID order.
    pub textures: Vec<Texture>,
    /// Typed samplers in ascending texture-ID order.
    pub samplers: Vec<Sampler>,
}

/// Failures from bounded image preflight and deterministic texture normalization.
#[derive(Debug, Error)]
pub enum TextureNormalizationError {
    /// Source payload exceeded the fixed input bound.
    #[error("texture input {asset} exceeds {limit_bytes} bytes: {actual_bytes}")]
    InputTooLarge { asset: SourceAssetPath, actual_bytes: usize, limit_bytes: usize },
    /// The image type is outside the intentionally small initial portable subset.
    #[error("texture input {asset} is not a supported PNG payload")]
    UnsupportedImage { asset: SourceAssetPath },
    /// The PNG header did not provide a bounded IHDR dimensions record.
    #[error("texture input {asset} has a malformed PNG IHDR header")]
    InvalidPngHeader { asset: SourceAssetPath },
    /// Image dimensions exceeded a deterministic resource limit.
    #[error("texture input {asset} has unsupported dimensions {width}x{height}")]
    DimensionsExceeded { asset: SourceAssetPath, width: u32, height: u32 },
    /// Texture IDs must be unique before package publication.
    #[error("texture normalization has a duplicate texture ID: {id}")]
    DuplicateTextureId { id: u32 },
    /// Generated staging naming overflowed a typed package field.
    #[error("texture normalization could not construct a stable staging name")]
    StagingName,
}

/// Isolated adapter that validates and normalizes source-acquired PNG texture bytes.
#[derive(Clone, Copy, Debug, Default)]
pub struct TextureNormalizer;

impl TextureNormalizer {
    /// Normalizes deterministic PNG texture staging records.
    ///
    /// # Errors
    ///
    /// Returns a typed error for duplicate IDs, invalid/oversized PNG inputs, or resource-limit
    /// violations. No files are written by this pure staging preparation method.
    pub fn normalize(
        &self,
        inputs: impl IntoIterator<Item = TextureInput>,
    ) -> Result<TextureNormalization, TextureNormalizationError> {
        let mut inputs = inputs.into_iter().collect::<Vec<_>>();
        inputs.sort_by(|left, right| {
            left.id.cmp(&right.id).then_with(|| left.source.cmp(&right.source))
        });
        let mut ids = BTreeSet::new();
        let mut staged = BTreeMap::new();
        let mut textures = Vec::new();
        let mut samplers = Vec::new();
        for input in inputs {
            if !ids.insert(input.id) {
                return Err(TextureNormalizationError::DuplicateTextureId { id: input.id.get() });
            }
            let (width, height) = png_dimensions(&input.source, &input.bytes)?;
            let checksum = ArtifactHash::from_bytes(&input.bytes);
            let staged_path = format!("textures/{}.png", checksum.as_str());
            if staged_path.len() > 160 {
                return Err(TextureNormalizationError::StagingName);
            }
            let sampler = Sampler {
                id: crate::domain::scene_ir::SamplerId::new(input.id.get()),
                mag_filter: input.sampler.mag_filter,
                min_filter: input.sampler.min_filter,
                wrap_s: input.sampler.wrap_s,
                wrap_t: input.sampler.wrap_t,
            };
            let texture = Texture {
                id: input.id,
                image: staged_path,
                color_space: input.role.color_space(),
                sampler: Some(sampler.id),
            };
            staged.entry(checksum.clone()).or_insert_with(|| StagedTexture {
                texture: texture.clone(),
                sampler: sampler.clone(),
                width,
                height,
                checksum: checksum.clone(),
                bytes: input.bytes,
            });
            textures.push(texture);
            samplers.push(sampler);
        }
        Ok(TextureNormalization { staged, textures, samplers })
    }
}

fn png_dimensions(
    source: &SourceAssetPath,
    bytes: &[u8],
) -> Result<(u32, u32), TextureNormalizationError> {
    if bytes.len() > MAX_IMAGE_BYTES {
        return Err(TextureNormalizationError::InputTooLarge {
            asset: source.clone(),
            actual_bytes: bytes.len(),
            limit_bytes: MAX_IMAGE_BYTES,
        });
    }
    if bytes.get(..PNG_SIGNATURE.len()) != Some(PNG_SIGNATURE.as_slice()) {
        return Err(TextureNormalizationError::UnsupportedImage { asset: source.clone() });
    }
    let mut cursor = PNG_SIGNATURE.len();
    let mut dimensions = None;
    let mut first_chunk = true;
    let mut saw_image_data = false;
    while cursor < bytes.len() {
        let chunk_header = bytes
            .get(cursor..cursor.saturating_add(8))
            .ok_or_else(|| TextureNormalizationError::InvalidPngHeader { asset: source.clone() })?;
        let length = png_u32(chunk_header.get(..4), source)?;
        let chunk_type = chunk_header
            .get(4..)
            .ok_or_else(|| TextureNormalizationError::InvalidPngHeader { asset: source.clone() })?;
        let payload_start = cursor
            .checked_add(8)
            .ok_or_else(|| TextureNormalizationError::InvalidPngHeader { asset: source.clone() })?;
        let payload_end = payload_start
            .checked_add(usize::try_from(length).map_err(|_| {
                TextureNormalizationError::InvalidPngHeader { asset: source.clone() }
            })?)
            .ok_or_else(|| TextureNormalizationError::InvalidPngHeader { asset: source.clone() })?;
        let chunk_end = payload_end
            .checked_add(4)
            .ok_or_else(|| TextureNormalizationError::InvalidPngHeader { asset: source.clone() })?;
        let payload = bytes
            .get(payload_start..payload_end)
            .ok_or_else(|| TextureNormalizationError::InvalidPngHeader { asset: source.clone() })?;
        if bytes.get(payload_end..chunk_end).is_none() {
            return Err(TextureNormalizationError::InvalidPngHeader { asset: source.clone() });
        }
        if first_chunk {
            if chunk_type != b"IHDR" || length != 13 {
                return Err(TextureNormalizationError::InvalidPngHeader { asset: source.clone() });
            }
            let width = png_u32(payload.get(..4), source)?;
            let height = png_u32(payload.get(4..8), source)?;
            let pixels = u64::from(width).saturating_mul(u64::from(height));
            if width == 0
                || height == 0
                || width > MAX_IMAGE_DIMENSION
                || height > MAX_IMAGE_DIMENSION
                || pixels > MAX_IMAGE_PIXELS
            {
                return Err(TextureNormalizationError::DimensionsExceeded {
                    asset: source.clone(),
                    width,
                    height,
                });
            }
            dimensions = Some((width, height));
            first_chunk = false;
        }
        if chunk_type == b"IDAT" {
            saw_image_data = true;
        }
        if chunk_type == b"IEND" {
            if !saw_image_data || length != 0 || chunk_end != bytes.len() {
                return Err(TextureNormalizationError::InvalidPngHeader { asset: source.clone() });
            }
            return dimensions.ok_or_else(|| TextureNormalizationError::InvalidPngHeader {
                asset: source.clone(),
            });
        }
        cursor = chunk_end;
    }
    Err(TextureNormalizationError::InvalidPngHeader { asset: source.clone() })
}

fn png_u32(
    bytes: Option<&[u8]>,
    source: &SourceAssetPath,
) -> Result<u32, TextureNormalizationError> {
    let bytes = bytes
        .ok_or_else(|| TextureNormalizationError::InvalidPngHeader { asset: source.clone() })?;
    let fixed = bytes
        .try_into()
        .map_err(|_| TextureNormalizationError::InvalidPngHeader { asset: source.clone() })?;
    Ok(u32::from_be_bytes(fixed))
}
