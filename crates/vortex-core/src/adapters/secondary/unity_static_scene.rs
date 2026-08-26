//! Bounded Unity Editor static-scene report conversion for the real-Editor validation lane.
//!
//! The adapter accepts only the narrow `vortex.unity-static-scene/v1` protocol. It deliberately
//! does not parse Unity project files or resolve arbitrary paths: the Unity Editor bridge resolves
//! the selected temporary scene, and this module converts its bounded facts to canonical IR.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path},
    str::FromStr,
};

use serde::Deserialize;
use thiserror::Error;

use crate::{
    adapters::secondary::{
        texture::{
            StagedTexture, TextureInput, TextureNormalizationError, TextureNormalizer, TextureRole,
            TextureSamplerInput,
        },
        urp_material::{
            UrpMaterialError, UrpMaterialInput, UrpMaterialMapper, UrpShader, UrpSurface,
            UrpTextureBinding,
        },
    },
    domain::{
        ClassificationKind, ConversionClassification, DomainError, ProjectRoot, Remediation,
        SourceAssetPath, SourceIdentity, UnityAssetGuid, UnityObjectId,
        identity::IdentityError,
        scene_ir::{
            Camera, CameraId, CanonicalScene, CoordinateNormalization, Light, LightId, MaterialId,
            Mesh, MeshId, Node, NodeId, Primitive, PrimitiveId, SourceLink, SourceLinkId,
            TextureId, TextureTransform, Transform, UnsupportedFeature, UnsupportedFeatureId,
            VertexStream, VertexStreamId,
        },
    },
};

/// Current Unity static-scene extraction protocol.
pub const STATIC_SCENE_PROTOCOL: &str = "vortex.unity-static-scene/v1";
/// Largest accepted JSON report in the real-Editor validation lane.
pub const MAX_STATIC_SCENE_REPORT_BYTES: usize = 1_048_576;
/// Largest supported static-mesh vertex count in this deliberately small protocol.
pub const MAX_STATIC_SCENE_VERTICES: usize = 16_384;
/// Largest supported static-mesh triangle-index count in this deliberately small protocol.
pub const MAX_STATIC_SCENE_INDICES: usize = 49_152;
/// Largest staged base-color PNG read before handing bytes to the texture normalizer.
pub const MAX_STATIC_SCENE_TEXTURE_BYTES: u64 = 8 * 1024 * 1024;

/// Validated real-Editor scene conversion result, ready for GLB packaging and atomic publication.
#[derive(Clone, Debug, PartialEq)]
pub struct UnityStaticSceneConversion {
    /// Fully validated engine-neutral scene graph.
    pub scene: CanonicalScene,
    /// Canonicalized staged base-color texture and its exact publication bytes.
    pub base_color_texture: StagedTexture,
    /// Main selected Unity scene source identity retained for manifests and provenance.
    pub source: SourceIdentity,
    /// Approved baseline URP material conversion classification.
    pub material_classification: ConversionClassification,
}

/// Boundary failures for the narrow Unity static-scene report protocol.
#[derive(Debug, Error)]
pub enum UnityStaticSceneError {
    /// The JSON report exceeded a bounded parsing limit.
    #[error("Unity static-scene report is {actual} bytes, exceeding the {limit}-byte limit")]
    ReportTooLarge { actual: usize, limit: usize },
    /// The bridge did not emit valid JSON for the protocol.
    #[error("Unity static-scene report JSON is invalid: {source}")]
    Json { source: serde_json::Error },
    /// The bridge used another source engine or protocol version.
    #[error("Unity static-scene report has unsupported {field}: {value}")]
    UnsupportedProtocol { field: &'static str, value: String },
    /// A typed canonical identity could not be constructed at the boundary.
    #[error("Unity static-scene report has an invalid source identity: {source}")]
    Identity { source: IdentityError },
    /// The report supplied malformed or out-of-scope mesh data.
    #[error("Unity static-scene report has invalid mesh data: {reason}")]
    Mesh { reason: &'static str },
    /// A required numeric fact was nonfinite or outside the deliberately small subset.
    #[error("Unity static-scene report has an invalid numeric value in {field}")]
    Numeric { field: &'static str },
    /// The bridge output path was not a normal staging-relative PNG location.
    #[error("Unity static-scene report has an unsafe base-color texture path: {path}")]
    UnsafeTexturePath { path: String },
    /// The staging directory or requested texture could not be resolved safely.
    #[error("Unity static-scene staged texture could not be read: {source}")]
    StagedTextureRead { source: std::io::Error },
    /// The resolved texture was not under the supplied staging root.
    #[error("Unity static-scene staged texture escapes its configured staging root")]
    StagedTextureEscapesRoot,
    /// The resolved staged texture was too large to acquire.
    #[error(
        "Unity static-scene staged texture is {actual} bytes, exceeding the {limit}-byte limit"
    )]
    StagedTextureTooLarge { actual: u64, limit: u64 },
    /// Texture normalization rejected the staged base-color PNG.
    #[error("Unity static-scene texture normalization failed: {source}")]
    Texture { source: TextureNormalizationError },
    /// The constrained URP material mapper rejected or deferred a claimed baseline material.
    #[error("Unity static-scene URP material mapping failed: {source}")]
    Material { source: UrpMaterialError },
    /// A claimed baseline material did not produce portable PBR output.
    #[error("Unity static-scene URP Lit material unexpectedly required an adapter")]
    MaterialRequiresAdapter,
    /// An explicit unsupported component could not receive an actionable classification.
    #[error("Unity static-scene unsupported component classification failed: {source}")]
    Classification { source: DomainError },
    /// Canonical scene construction rejected a reference or source fact.
    #[error("Unity static-scene canonical IR construction failed: {source}")]
    Scene { source: crate::domain::scene_ir::SceneIrError },
}

/// Converts a tightly scoped Unity Editor static-scene report to canonical IR.
#[derive(Clone, Copy, Debug, Default)]
pub struct UnityStaticSceneAdapter;

impl UnityStaticSceneAdapter {
    /// Parses and converts report bytes plus the already-acquired base-color PNG.
    ///
    /// # Errors
    ///
    /// Returns a typed error for protocol, identity, geometry, numeric, material, texture, or
    /// canonical-scene violations. This method never reads an arbitrary filesystem path.
    pub fn convert(
        &self,
        project_root: ProjectRoot,
        report_bytes: &[u8],
        base_color_png: Vec<u8>,
    ) -> Result<UnityStaticSceneConversion, UnityStaticSceneError> {
        let report = parse_report(report_bytes, project_root)?;
        convert_report(report, base_color_png)
    }

    /// Reads a report-selected PNG only from a confined staging root, then converts it.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the report is invalid, the output path is unsafe, the resolved
    /// file escapes the supplied root, or canonical conversion fails.
    pub fn load_and_convert(
        &self,
        project_root: ProjectRoot,
        staging_root: &Path,
        report_bytes: &[u8],
    ) -> Result<UnityStaticSceneConversion, UnityStaticSceneError> {
        let report = parse_report(report_bytes, project_root)?;
        let texture = read_confined_texture(staging_root, &report.base_color_texture)?;
        convert_report(report, texture)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawReport {
    engine: String,
    protocol_version: String,
    editor_version: String,
    urp_package_version: String,
    scene: RawScene,
    unsupported_components: Vec<RawUnsupportedComponent>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawScene {
    source_guid: String,
    source_path: String,
    object_id: u64,
    mesh: RawMesh,
    material: RawMaterial,
    camera: RawCamera,
    light: RawLight,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawMesh {
    positions: Vec<RawVector3>,
    normals: Vec<RawVector3>,
    uvs: Vec<RawVector2>,
    indices: Vec<u32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawMaterial {
    shader: String,
    base_color: RawColor,
    metallic: f32,
    smoothness: f32,
    base_color_texture: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCamera {
    #[serde(flatten)]
    transform: RawTransform,
    vertical_fov_radians: f32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLight {
    #[serde(flatten)]
    transform: RawTransform,
    kind: String,
    intensity: f32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTransform {
    position: RawVector3,
    rotation: RawQuaternion,
    scale: RawVector3,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawVector2 {
    x: f32,
    y: f32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawVector3 {
    x: f32,
    y: f32,
    z: f32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawQuaternion {
    x: f32,
    y: f32,
    z: f32,
    w: f32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawColor {
    r: f32,
    g: f32,
    b: f32,
    a: f32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawUnsupportedComponent {
    component: String,
    classification: String,
    reason: String,
}

struct ValidatedReport {
    editor_version: String,
    urp_package_version: String,
    source: SourceIdentity,
    base_color_texture: String,
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    indices: Vec<u32>,
    base_color: [f32; 4],
    metallic: f32,
    smoothness: f32,
    camera: Transform,
    vertical_fov_radians: f32,
    light: Transform,
    light_kind: String,
    light_intensity: f32,
    unsupported_components: Vec<RawUnsupportedComponent>,
}

#[expect(
    clippy::too_many_lines,
    reason = "The versioned wire-contract boundary validates every required v1 fact before any canonical scene is constructed."
)]
fn parse_report(
    report_bytes: &[u8],
    project_root: ProjectRoot,
) -> Result<ValidatedReport, UnityStaticSceneError> {
    if report_bytes.len() > MAX_STATIC_SCENE_REPORT_BYTES {
        return Err(UnityStaticSceneError::ReportTooLarge {
            actual: report_bytes.len(),
            limit: MAX_STATIC_SCENE_REPORT_BYTES,
        });
    }
    let report: RawReport = serde_json::from_slice(report_bytes)
        .map_err(|source| UnityStaticSceneError::Json { source })?;
    if report.engine != "unity" {
        return Err(UnityStaticSceneError::UnsupportedProtocol {
            field: "engine",
            value: report.engine,
        });
    }
    if report.protocol_version != STATIC_SCENE_PROTOCOL {
        return Err(UnityStaticSceneError::UnsupportedProtocol {
            field: "protocol_version",
            value: report.protocol_version,
        });
    }
    if report.editor_version.trim().is_empty() {
        return Err(UnityStaticSceneError::UnsupportedProtocol {
            field: "editor_version",
            value: report.editor_version,
        });
    }
    if report.urp_package_version.trim().is_empty() {
        return Err(UnityStaticSceneError::UnsupportedProtocol {
            field: "urp_package_version",
            value: report.urp_package_version,
        });
    }
    if report.scene.material.shader != UrpShader::Lit.source_name() {
        return Err(UnityStaticSceneError::UnsupportedProtocol {
            field: "scene.material.shader",
            value: report.scene.material.shader,
        });
    }
    let asset_path = SourceAssetPath::from_str(&report.scene.source_path)
        .map_err(|source| UnityStaticSceneError::Identity { source })?;
    if !report.scene.source_path.starts_with("Assets/") {
        return Err(UnityStaticSceneError::Mesh {
            reason: "scene source path must be under Assets",
        });
    }
    let source = SourceIdentity::Unity {
        project_root,
        asset_path,
        asset_guid: UnityAssetGuid::from_str(&report.scene.source_guid)
            .map_err(|source| UnityStaticSceneError::Identity { source })?,
        object_id: UnityObjectId::new(report.scene.object_id),
    };
    let positions = vec3_values(&report.scene.mesh.positions, "mesh.positions")?;
    let normals = vec3_values(&report.scene.mesh.normals, "mesh.normals")?;
    let uvs = vec2_values(&report.scene.mesh.uvs, "mesh.uvs")?;
    let indices = report.scene.mesh.indices;
    if positions.len() > MAX_STATIC_SCENE_VERTICES {
        return Err(UnityStaticSceneError::Mesh {
            reason: "vertex count exceeds static-scene limit",
        });
    }
    if positions.len() < 3 || normals.len() != positions.len() || uvs.len() != positions.len() {
        return Err(UnityStaticSceneError::Mesh {
            reason: "positions, normals, and UVs must be aligned nonempty streams",
        });
    }
    if indices.len() > MAX_STATIC_SCENE_INDICES || !indices.len().is_multiple_of(3) {
        return Err(UnityStaticSceneError::Mesh {
            reason: "indices must be a bounded triangle list",
        });
    }
    if indices
        .iter()
        .any(|index| usize::try_from(*index).map_or(true, |value| value >= positions.len()))
    {
        return Err(UnityStaticSceneError::Mesh {
            reason: "index refers outside the vertex stream",
        });
    }
    let base_color = color_value(&report.scene.material.base_color)?;
    bounded_unit(report.scene.material.metallic, "scene.material.metallic")?;
    bounded_unit(report.scene.material.smoothness, "scene.material.smoothness")?;
    let base_color_texture = validated_texture_path(&report.scene.material.base_color_texture)?;
    let camera = transform_value(&report.scene.camera.transform, "scene.camera")?;
    let vertical_fov_radians = report.scene.camera.vertical_fov_radians;
    if !vertical_fov_radians.is_finite()
        || !(0.0..std::f32::consts::PI).contains(&vertical_fov_radians)
    {
        return Err(UnityStaticSceneError::Numeric { field: "scene.camera.vertical_fov_radians" });
    }
    let light = transform_value(&report.scene.light.transform, "scene.light")?;
    let light_kind = report.scene.light.kind;
    if !matches!(light_kind.as_str(), "directional" | "point") {
        return Err(UnityStaticSceneError::UnsupportedProtocol {
            field: "scene.light.kind",
            value: light_kind,
        });
    }
    let light_intensity = report.scene.light.intensity;
    if !light_intensity.is_finite() || light_intensity <= 0.0 {
        return Err(UnityStaticSceneError::Numeric { field: "scene.light.intensity" });
    }
    if report.unsupported_components.is_empty()
        || report.unsupported_components.iter().any(|component| {
            component.classification != "requires_adapter"
                || component.component.trim().is_empty()
                || component.reason.trim().is_empty()
        })
    {
        return Err(UnityStaticSceneError::Mesh {
            reason: "at least one explicit requires_adapter component is required",
        });
    }
    Ok(ValidatedReport {
        editor_version: report.editor_version,
        urp_package_version: report.urp_package_version,
        source,
        base_color_texture,
        positions,
        normals,
        uvs,
        indices,
        base_color,
        metallic: report.scene.material.metallic,
        smoothness: report.scene.material.smoothness,
        camera,
        vertical_fov_radians,
        light,
        light_kind,
        light_intensity,
        unsupported_components: report.unsupported_components,
    })
}

#[expect(
    clippy::too_many_lines,
    reason = "The conversion transaction keeps texture normalization, URP mapping, provenance, and canonical scene construction in one fail-closed boundary."
)]
fn convert_report(
    report: ValidatedReport,
    base_color_png: Vec<u8>,
) -> Result<UnityStaticSceneConversion, UnityStaticSceneError> {
    let texture_source = SourceAssetPath::from_str(&report.base_color_texture)
        .map_err(|source| UnityStaticSceneError::Identity { source })?;
    let normalization = TextureNormalizer
        .normalize([TextureInput {
            id: TextureId::new(1),
            source: texture_source,
            role: TextureRole::BaseColor,
            sampler: TextureSamplerInput::linear_repeat(),
            bytes: base_color_png,
        }])
        .map_err(|source| UnityStaticSceneError::Texture { source })?;
    let base_color_texture = normalization
        .staged
        .into_values()
        .next()
        .ok_or(UnityStaticSceneError::MaterialRequiresAdapter)?;
    let material_mapping = UrpMaterialMapper
        .map(&UrpMaterialInput {
            id: MaterialId::new(1),
            source: report.source.clone(),
            shader: UrpShader::Lit,
            base_color: report.base_color,
            metallic: report.metallic,
            smoothness: report.smoothness,
            normal_scale: 1.0,
            occlusion_strength: 1.0,
            emissive: [0.0, 0.0, 0.0],
            surface: UrpSurface::Opaque,
            alpha_clip: false,
            alpha_cutoff: None,
            double_sided: false,
            texture_transform: TextureTransform::default(),
            textures: vec![UrpTextureBinding {
                role: TextureRole::BaseColor,
                texture: TextureId::new(1),
            }],
            unsupported_features: BTreeSet::default(),
        })
        .map_err(|source| UnityStaticSceneError::Material { source })?;
    let material =
        material_mapping.material.ok_or(UnityStaticSceneError::MaterialRequiresAdapter)?;
    let source_link_id = SourceLinkId::new(1);
    let unsupported_features = report
        .unsupported_components
        .iter()
        .enumerate()
        .map(|(index, component)| unsupported_feature(index, source_link_id, component))
        .collect::<Result<Vec<_>, _>>()?;
    let mut source_facts = BTreeMap::new();
    source_facts.insert("unity.editor.version".to_owned(), report.editor_version);
    source_facts.insert("unity.urp.package_version".to_owned(), report.urp_package_version);
    source_facts.insert("unity.static_scene.texture_output".to_owned(), report.base_color_texture);
    source_facts.insert(
        "unity.static_scene.material_shader".to_owned(),
        UrpShader::Lit.source_name().to_owned(),
    );
    let scene = CanonicalScene::new(CanonicalScene {
        normalization: CoordinateNormalization {
            meters_per_unit: "1.000000000000".to_owned(),
            source_up_axis: "Y".to_owned(),
            target_up_axis: "Y".to_owned(),
            winding: "counter_clockwise".to_owned(),
            uv_origin: "bottom_left".to_owned(),
            color_space: "linear_srgb".to_owned(),
        },
        source_links: vec![SourceLink {
            id: source_link_id,
            source: report.source.clone(),
            facts: source_facts,
        }],
        nodes: vec![
            Node {
                id: NodeId::new(1),
                parent: None,
                transform: Transform::default(),
                mesh: Some(MeshId::new(1)),
                camera: None,
                light: None,
                instance: None,
                source_link: source_link_id,
            },
            Node {
                id: NodeId::new(2),
                parent: None,
                transform: report.camera,
                mesh: None,
                camera: Some(CameraId::new(1)),
                light: None,
                instance: None,
                source_link: source_link_id,
            },
            Node {
                id: NodeId::new(3),
                parent: None,
                transform: report.light,
                mesh: None,
                camera: None,
                light: Some(LightId::new(1)),
                instance: None,
                source_link: source_link_id,
            },
        ],
        meshes: vec![Mesh {
            id: MeshId::new(1),
            primitives: vec![PrimitiveId::new(1)],
            skin: None,
        }],
        primitives: vec![Primitive {
            id: PrimitiveId::new(1),
            vertex_stream: VertexStreamId::new(1),
            material: Some(MaterialId::new(1)),
            morph_targets: Vec::new(),
        }],
        vertex_streams: vec![VertexStream {
            id: VertexStreamId::new(1),
            positions: report.positions,
            normals: Some(report.normals),
            tangents: None,
            tex_coords_0: Some(report.uvs),
            colors_0: None,
            joints_0: None,
            weights_0: None,
            indices: report.indices,
        }],
        materials: vec![material],
        textures: normalization.textures,
        samplers: normalization.samplers,
        skins: Vec::new(),
        joints: Vec::new(),
        animation_clips: Vec::new(),
        channels: Vec::new(),
        morph_targets: Vec::new(),
        cameras: vec![Camera {
            id: CameraId::new(1),
            projection: "perspective".to_owned(),
            primary_extent: f64::from(report.vertical_fov_radians),
        }],
        lights: vec![Light {
            id: LightId::new(1),
            kind: report.light_kind,
            intensity: f64::from(report.light_intensity),
        }],
        instances: Vec::new(),
        unsupported_features,
    })
    .map_err(|source| UnityStaticSceneError::Scene { source })?;
    Ok(UnityStaticSceneConversion {
        scene,
        base_color_texture,
        source: report.source,
        material_classification: material_mapping.classification,
    })
}

fn vec3_values(
    values: &[RawVector3],
    field: &'static str,
) -> Result<Vec<[f32; 3]>, UnityStaticSceneError> {
    values
        .iter()
        .map(|value| {
            finite(value.x, field)?;
            finite(value.y, field)?;
            finite(value.z, field)?;
            Ok([value.x, value.y, value.z])
        })
        .collect()
}

fn vec2_values(
    values: &[RawVector2],
    field: &'static str,
) -> Result<Vec<[f32; 2]>, UnityStaticSceneError> {
    values
        .iter()
        .map(|value| {
            finite(value.x, field)?;
            finite(value.y, field)?;
            Ok([value.x, value.y])
        })
        .collect()
}

fn color_value(value: &RawColor) -> Result<[f32; 4], UnityStaticSceneError> {
    for component in [value.r, value.g, value.b, value.a] {
        bounded_unit(component, "scene.material.base_color")?;
    }
    Ok([value.r, value.g, value.b, value.a])
}

fn transform_value(
    value: &RawTransform,
    field: &'static str,
) -> Result<Transform, UnityStaticSceneError> {
    for component in [
        value.position.x,
        value.position.y,
        value.position.z,
        value.rotation.x,
        value.rotation.y,
        value.rotation.z,
        value.rotation.w,
        value.scale.x,
        value.scale.y,
        value.scale.z,
    ] {
        finite(component, field)?;
    }
    if value.scale.x == 0.0 || value.scale.y == 0.0 || value.scale.z == 0.0 {
        return Err(UnityStaticSceneError::Numeric { field });
    }
    Ok(Transform {
        translation: [
            f64::from(value.position.x),
            f64::from(value.position.y),
            f64::from(value.position.z),
        ],
        rotation: [
            f64::from(value.rotation.x),
            f64::from(value.rotation.y),
            f64::from(value.rotation.z),
            f64::from(value.rotation.w),
        ],
        scale: [f64::from(value.scale.x), f64::from(value.scale.y), f64::from(value.scale.z)],
    })
}

const fn finite(value: f32, field: &'static str) -> Result<(), UnityStaticSceneError> {
    if value.is_finite() { Ok(()) } else { Err(UnityStaticSceneError::Numeric { field }) }
}

fn bounded_unit(value: f32, field: &'static str) -> Result<(), UnityStaticSceneError> {
    if value.is_finite() && (0.0..=1.0).contains(&value) {
        Ok(())
    } else {
        Err(UnityStaticSceneError::Numeric { field })
    }
}

fn validated_texture_path(value: &str) -> Result<String, UnityStaticSceneError> {
    let path = Path::new(value);
    if !value.starts_with("textures/")
        || path.extension().is_none_or(|extension| extension != "png")
        || !is_normal_relative_path(path)
    {
        return Err(UnityStaticSceneError::UnsafeTexturePath { path: value.to_owned() });
    }
    Ok(value.to_owned())
}

fn read_confined_texture(
    staging_root: &Path,
    relative_path: &str,
) -> Result<Vec<u8>, UnityStaticSceneError> {
    let root = staging_root
        .canonicalize()
        .map_err(|source| UnityStaticSceneError::StagedTextureRead { source })?;
    let candidate = root.join(relative_path);
    let resolved = candidate
        .canonicalize()
        .map_err(|source| UnityStaticSceneError::StagedTextureRead { source })?;
    if !resolved.starts_with(&root) {
        return Err(UnityStaticSceneError::StagedTextureEscapesRoot);
    }
    let metadata = fs::metadata(&resolved)
        .map_err(|source| UnityStaticSceneError::StagedTextureRead { source })?;
    if !metadata.is_file() {
        return Err(UnityStaticSceneError::StagedTextureEscapesRoot);
    }
    if metadata.len() > MAX_STATIC_SCENE_TEXTURE_BYTES {
        return Err(UnityStaticSceneError::StagedTextureTooLarge {
            actual: metadata.len(),
            limit: MAX_STATIC_SCENE_TEXTURE_BYTES,
        });
    }
    fs::read(resolved).map_err(|source| UnityStaticSceneError::StagedTextureRead { source })
}

fn is_normal_relative_path(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && path.components().all(|component| matches!(component, Component::Normal(_)))
}

fn unsupported_feature(
    index: usize,
    source_link: SourceLinkId,
    component: &RawUnsupportedComponent,
) -> Result<UnsupportedFeature, UnityStaticSceneError> {
    let identifier = u32::try_from(index).ok().and_then(|value| value.checked_add(1)).ok_or(
        UnityStaticSceneError::Mesh { reason: "unsupported component count overflows IDs" },
    )?;
    let classification = ConversionClassification::new(
        ClassificationKind::RequiresAdapter,
        component.reason.clone(),
        Remediation::new(
            "Add a dedicated Unity component adapter or omit this component explicitly.",
        )
        .map_err(|source| UnityStaticSceneError::Classification { source })?,
    )
    .map_err(|source| UnityStaticSceneError::Classification { source })?;
    Ok(UnsupportedFeature {
        id: UnsupportedFeatureId::new(identifier),
        source_link,
        feature: format!("unity.component.{}", component.component),
        classification,
    })
}

#[cfg(test)]
mod tests {
    use super::{is_normal_relative_path, validated_texture_path};
    use std::path::Path;

    #[test]
    fn relative_texture_paths_are_confined() {
        assert!(is_normal_relative_path(Path::new("textures/base.png")));
        assert!(validated_texture_path("textures/base.png").is_ok());
        assert!(validated_texture_path("../textures/base.png").is_err());
        assert!(validated_texture_path("textures/base.jpg").is_err());
    }
}
