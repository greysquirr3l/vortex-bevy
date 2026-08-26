//! Unity URP-specific material interpretation that maps only approved baseline facts.

use std::collections::{BTreeMap, BTreeSet};

use thiserror::Error;

use crate::{
    adapters::secondary::texture::TextureRole,
    domain::{
        ClassificationKind, ConversionClassification, DomainError, Remediation, SourceIdentity,
        scene_ir::{AlphaMode, Material, MaterialId, TextureId, TextureTransform},
    },
};

/// Baseline URP shader identity accepted by the initial mapper.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UrpShader {
    /// `Universal Render Pipeline/Lit` metallic workflow.
    Lit,
    /// `Universal Render Pipeline/Unlit` surface.
    Unlit,
}

impl UrpShader {
    /// Parses the exact approved Unity shader identity.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "Universal Render Pipeline/Lit" => Some(Self::Lit),
            "Universal Render Pipeline/Unlit" => Some(Self::Unlit),
            _ => None,
        }
    }

    /// Returns the exact Unity source shader identity.
    #[must_use]
    pub const fn source_name(self) -> &'static str {
        match self {
            Self::Lit => "Universal Render Pipeline/Lit",
            Self::Unlit => "Universal Render Pipeline/Unlit",
        }
    }
}

/// URP surface blend mode with a direct portable alpha equivalent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UrpSurface {
    /// Opaque surface output.
    Opaque,
    /// Alpha-blended surface output.
    Transparent,
}

/// Explicit source feature outside the bounded URP baseline.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum UrpUnsupportedFeature {
    /// Specular workflow rather than metallic workflow.
    SpecularWorkflow,
    /// Height or parallax mapping.
    HeightMap,
    /// Detail-map layering.
    DetailMap,
    /// Shader Graph behavior.
    ShaderGraph,
    /// Custom shader code or shader family.
    CustomShader,
    /// Clear-coat behavior.
    ClearCoat,
    /// Renderer feature/pass ordering dependency.
    RendererFeature,
}

impl UrpUnsupportedFeature {
    const fn label(self) -> &'static str {
        match self {
            Self::SpecularWorkflow => "specular_workflow",
            Self::HeightMap => "height_map",
            Self::DetailMap => "detail_map",
            Self::ShaderGraph => "shader_graph",
            Self::CustomShader => "custom_shader",
            Self::ClearCoat => "clear_coat",
            Self::RendererFeature => "renderer_feature",
        }
    }
}

/// One resolved texture binding supplied by the normalized texture layer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UrpTextureBinding {
    /// Portable material role.
    pub role: TextureRole,
    /// Canonical texture identity.
    pub texture: TextureId,
}

/// Bounded Unity-owned URP material facts.
#[derive(Clone, Debug, PartialEq)]
pub struct UrpMaterialInput {
    /// Stable canonical material identity.
    pub id: MaterialId,
    /// Typed Unity material source identity.
    pub source: SourceIdentity,
    /// Exact approved URP shader identity.
    pub shader: UrpShader,
    /// Base map/color factor.
    pub base_color: [f32; 4],
    /// Metallic factor in the metallic workflow.
    pub metallic: f32,
    /// Unity smoothness factor, converted deterministically to roughness.
    pub smoothness: f32,
    /// Normal map scale.
    pub normal_scale: f32,
    /// Occlusion strength.
    pub occlusion_strength: f32,
    /// Emission color factor.
    pub emissive: [f32; 3],
    /// Opaque or transparent surface mode.
    pub surface: UrpSurface,
    /// Whether alpha clipping is enabled.
    pub alpha_clip: bool,
    /// Alpha clip threshold when clipping is enabled.
    pub alpha_cutoff: Option<f32>,
    /// Whether URP culling is disabled.
    pub double_sided: bool,
    /// Main texture tiling and offset.
    pub texture_transform: TextureTransform,
    /// Resolved portable texture bindings.
    pub textures: Vec<UrpTextureBinding>,
    /// Explicit source features that require another adapter.
    pub unsupported_features: BTreeSet<UrpUnsupportedFeature>,
}

/// Result of mapping one URP source material without leaking Unity keys into canonical IR.
#[derive(Clone, Debug, PartialEq)]
pub struct UrpMaterialMapping {
    /// Canonical PBR material when all supplied facts are representable.
    pub material: Option<Material>,
    /// Actionable conversion outcome.
    pub classification: ConversionClassification,
    /// Exact source provenance retained outside canonical material properties.
    pub provenance: BTreeMap<String, String>,
}

/// Failures from malformed bounded URP material facts.
#[derive(Debug, Error)]
pub enum UrpMaterialError {
    /// A numeric source factor could not be represented safely.
    #[error("URP material {material} has an invalid numeric property: {property}")]
    InvalidNumber { material: u32, property: &'static str },
    /// Alpha-cut behavior was incomplete or incompatible with source surface mode.
    #[error("URP material {material} has an invalid alpha clip configuration")]
    InvalidAlphaClip { material: u32 },
    /// A texture role was supplied more than once.
    #[error("URP material {material} has duplicate texture role: {role}")]
    DuplicateTextureRole { material: u32, role: &'static str },
    /// A typed domain classification could not be constructed.
    #[error("URP material mapping could not construct classification: {source}")]
    Domain { source: DomainError },
}

/// Isolated Unity-specific baseline URP mapper.
#[derive(Clone, Copy, Debug, Default)]
pub struct UrpMaterialMapper;

impl UrpMaterialMapper {
    /// Maps approved URP Lit/Unlit facts to source-neutral canonical PBR data.
    ///
    /// # Errors
    ///
    /// Returns typed errors for nonfinite/out-of-range factors, invalid alpha clipping, duplicate
    /// texture roles, or a domain classification construction failure. Unsupported source features
    /// return a normal actionable mapping result and no canonical material.
    pub fn map(&self, input: &UrpMaterialInput) -> Result<UrpMaterialMapping, UrpMaterialError> {
        validate(input)?;
        let provenance = provenance(input);
        if !input.unsupported_features.is_empty() {
            let features = input
                .unsupported_features
                .iter()
                .map(|feature| feature.label())
                .collect::<Vec<_>>()
                .join(",");
            return Ok(UrpMaterialMapping {
                material: None,
                classification: classification(
                    ClassificationKind::RequiresAdapter,
                    format!("URP source uses unsupported feature set: {features}"),
                    "Bake the material through an authorized Unity Editor path or add a dedicated feature mapper.",
                )?,
                provenance,
            });
        }
        let alpha_mode = if input.alpha_clip {
            AlphaMode::Mask
        } else if input.surface == UrpSurface::Transparent {
            AlphaMode::Blend
        } else {
            AlphaMode::Opaque
        };
        let textures = input
            .textures
            .iter()
            .map(|binding| (binding.role.slot().to_owned(), binding.texture))
            .collect();
        let material = Material {
            id: input.id,
            family: "pbr_metallic_roughness".to_owned(),
            base_color: input.base_color,
            metallic: if input.shader == UrpShader::Lit { input.metallic } else { 0.0 },
            roughness: if input.shader == UrpShader::Lit { 1.0 - input.smoothness } else { 1.0 },
            normal_scale: input.normal_scale,
            occlusion_strength: input.occlusion_strength,
            emissive: input.emissive,
            alpha_mode,
            alpha_cutoff: input.alpha_cutoff,
            double_sided: input.double_sided,
            texture_transform: input.texture_transform.clone(),
            textures,
        };
        Ok(UrpMaterialMapping {
            material: Some(material),
            classification: classification(
                ClassificationKind::Converted,
                "Approved URP baseline material maps directly to portable metallic-roughness PBR.",
                "No additional material adapter is required for the supplied baseline facts.",
            )?,
            provenance,
        })
    }
}

fn validate(input: &UrpMaterialInput) -> Result<(), UrpMaterialError> {
    let bounded = input
        .base_color
        .iter()
        .chain(input.emissive.iter())
        .chain(input.texture_transform.scale.iter())
        .chain(input.texture_transform.offset.iter())
        .all(|value| value.is_finite());
    if !bounded {
        return Err(UrpMaterialError::InvalidNumber {
            material: input.id.get(),
            property: "vector",
        });
    }
    for (property, value) in [
        ("metallic", input.metallic),
        ("smoothness", input.smoothness),
        ("normal_scale", input.normal_scale),
        ("occlusion_strength", input.occlusion_strength),
    ] {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(UrpMaterialError::InvalidNumber { material: input.id.get(), property });
        }
    }
    if input.alpha_clip != input.alpha_cutoff.is_some() {
        return Err(UrpMaterialError::InvalidAlphaClip { material: input.id.get() });
    }
    if let Some(cutoff) = input.alpha_cutoff
        && (!cutoff.is_finite() || !(0.0..=1.0).contains(&cutoff))
    {
        return Err(UrpMaterialError::InvalidAlphaClip { material: input.id.get() });
    }
    let mut roles = BTreeSet::new();
    for binding in &input.textures {
        if !roles.insert(binding.role) {
            return Err(UrpMaterialError::DuplicateTextureRole {
                material: input.id.get(),
                role: binding.role.slot(),
            });
        }
    }
    Ok(())
}

fn provenance(input: &UrpMaterialInput) -> BTreeMap<String, String> {
    let mut facts = BTreeMap::new();
    facts.insert("unity.urp.shader".to_owned(), input.shader.source_name().to_owned());
    facts.insert("unity.urp.smoothness".to_owned(), input.smoothness.to_string());
    facts.insert("unity.urp.roughness".to_owned(), (1.0 - input.smoothness).to_string());
    facts.insert(
        "unity.urp.surface".to_owned(),
        format!("{:?}", input.surface).to_ascii_lowercase(),
    );
    facts.insert("unity.urp.alpha_clip".to_owned(), input.alpha_clip.to_string());
    facts.insert("unity.urp.double_sided".to_owned(), input.double_sided.to_string());
    facts.insert("unity.source".to_owned(), input.source.to_string());
    facts
}

fn classification(
    kind: ClassificationKind,
    reason: impl Into<String>,
    remediation: &str,
) -> Result<ConversionClassification, UrpMaterialError> {
    ConversionClassification::new(
        kind,
        reason,
        Remediation::new(remediation).map_err(|source| UrpMaterialError::Domain { source })?,
    )
    .map_err(|source| UrpMaterialError::Domain { source })
}
