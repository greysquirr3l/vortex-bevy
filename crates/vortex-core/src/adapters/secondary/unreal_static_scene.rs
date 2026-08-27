//! Bounded canonical ingestion for the authorized Unreal static-scene bridge exports.
//!
//! This adapter accepts only the exact controlled `vortex.editor-bridge/v1` records
//! already validated by [`super::unreal_bridge::UnrealBridge`]. It parses one binary
//! FBX mesh and one TGA base-color image, preserves the report's explicit World and
//! `MaterialInstance` boundaries, and produces engine-neutral canonical IR.

use std::{collections::BTreeMap, io::Cursor, str::FromStr};

use fbxcel_dom::{
    any::AnyDocument,
    v7400::{
        data::mesh::{PolygonVertexIndex, PolygonVertices, layer::TypedLayerElementHandle},
        object::{TypedObjectHandle, geometry::TypedGeometryHandle},
    },
};
use image::ImageFormat;
use thiserror::Error;

use crate::{
    adapters::secondary::{
        texture::{
            StagedTexture, TextureInput, TextureNormalization, TextureNormalizationError,
            TextureNormalizer, TextureRole, TextureSamplerInput,
        },
        unreal_bridge::{UnrealBridgeRecord, UnrealBridgeReport},
    },
    domain::{
        ClassificationKind, ConversionClassification, SourceAssetPath, SourceIdentity,
        identity::IdentityError,
        scene_ir::{
            AlphaMode, CanonicalScene, CoordinateNormalization, Material, MaterialId, Mesh, MeshId,
            Node, NodeId, Primitive, PrimitiveId, SourceLink, SourceLinkId, TextureId,
            TextureTransform, Transform, UnsupportedFeature, UnsupportedFeatureId, VertexStream,
            VertexStreamId,
        },
    },
};

/// Maximum received FBX payload accepted by the controlled static-scene subset.
pub const MAX_UNREAL_STATIC_FBX_BYTES: usize = 8 * 1024 * 1024;
/// Maximum received TGA payload accepted before decoding to PNG.
pub const MAX_UNREAL_STATIC_TGA_BYTES: usize = 8 * 1024 * 1024;
/// Maximum flattened triangle-vertex count accepted from the controlled FBX subset.
pub const MAX_UNREAL_STATIC_VERTICES: usize = 16_384;
/// Maximum triangle-index count accepted from the controlled FBX subset.
pub const MAX_UNREAL_STATIC_INDICES: usize = 49_152;
/// Largest accepted dimension for the controlled base-color TGA.
pub const MAX_UNREAL_STATIC_TEXTURE_DIMENSION: u32 = 4_096;

const MESH_OUTPUT: &str = "exports/SM_VortexCube.fbx";
const TEXTURE_OUTPUT: &str = "exports/T_VortexBaseColor.tga";
const MESH_OBJECT: &str = "/Game/VortexBridgeTemporary/SM_VortexCube.SM_VortexCube";
const TEXTURE_OBJECT: &str = "/Game/VortexBridgeTemporary/T_VortexBaseColor.T_VortexBaseColor";
const MATERIAL_OBJECT: &str = "/Game/VortexBridgeTemporary/MI_VortexLit.MI_VortexLit";
const WORLD_OBJECT: &str =
    "/Game/VortexBridgeTemporary/LVL_VortexStaticScene.LVL_VortexStaticScene";

/// A validated canonical conversion of the controlled Unreal bridge exports.
#[derive(Clone, Debug, PartialEq)]
pub struct UnrealStaticSceneConversion {
    /// Fully validated canonical scene graph.
    pub scene: CanonicalScene,
    /// Content-addressed PNG bytes to publish at the URI referenced by the GLB.
    pub base_color_texture: StagedTexture,
    /// Mesh source retained for conversion and manifest provenance.
    pub mesh_source: SourceIdentity,
    /// Texture source retained for conversion and manifest provenance.
    pub texture_source: SourceIdentity,
    /// Explicit material-graph boundary from the authoritative Editor report.
    pub material_classification: ConversionClassification,
    /// Explicit world/actor boundary from the authoritative Editor report.
    pub world_classification: ConversionClassification,
}

/// Errors emitted by the narrow Unreal FBX/TGA canonical ingestion boundary.
#[derive(Debug, Error)]
pub enum UnrealStaticSceneError {
    /// A required, uniquely classified bridge record was unavailable.
    #[error("Unreal static-scene report does not contain the required {kind} record: {reason}")]
    Report { kind: &'static str, reason: &'static str },
    /// A received artifact exceeded its bounded acquisition limit.
    #[error("Unreal static-scene {kind} is {actual} bytes, exceeding the {limit}-byte limit")]
    ArtifactTooLarge { kind: &'static str, actual: usize, limit: usize },
    /// Binary FBX parsing or the controlled geometry subset rejected the mesh.
    #[error("Unreal static-scene FBX ingestion failed: {message}")]
    Fbx { message: String },
    /// The TGA payload could not be decoded or represented as canonical PNG bytes.
    #[error("Unreal static-scene TGA normalization failed: {message}")]
    Tga { message: String },
    /// Existing PNG normalization rejected the transcoded base-color image.
    #[error("Unreal static-scene PNG normalization failed: {source}")]
    Texture { source: TextureNormalizationError },
    /// A staged source path could not be represented as a typed domain identity.
    #[error("Unreal static-scene source identity is invalid: {source}")]
    Identity { source: IdentityError },
    /// Canonical graph construction rejected a source fact or render reference.
    #[error("Unreal static-scene canonical IR construction failed: {source}")]
    Scene { source: crate::domain::scene_ir::SceneIrError },
}

/// Converts the exact controlled Unreal static-scene bridge output to canonical IR.
#[derive(Clone, Copy, Debug, Default)]
pub struct UnrealStaticSceneAdapter;

type ParsedMesh = (Vec<[f32; 3]>, Vec<[f32; 3]>, Vec<[f32; 2]>, Vec<u32>);

struct ApprovedRecords<'a> {
    mesh: &'a UnrealBridgeRecord,
    texture: &'a UnrealBridgeRecord,
    material: &'a UnrealBridgeRecord,
    world: &'a UnrealBridgeRecord,
}

impl<'a> ApprovedRecords<'a> {
    fn select(report: &'a UnrealBridgeReport) -> Result<Self, UnrealStaticSceneError> {
        Ok(Self {
            mesh: required_record(
                report,
                "StaticMesh",
                MESH_OBJECT,
                "StaticMeshExporterFBX",
                MESH_OUTPUT,
                &ClassificationKind::Converted,
            )?,
            texture: required_record(
                report,
                "Texture2D",
                TEXTURE_OBJECT,
                "TextureExporterTGA",
                TEXTURE_OUTPUT,
                &ClassificationKind::Converted,
            )?,
            material: required_record(
                report,
                "MaterialInstanceConstant",
                MATERIAL_OBJECT,
                "none",
                "",
                &ClassificationKind::RequiresAdapter,
            )?,
            world: required_record(
                report,
                "World",
                WORLD_OBJECT,
                "none",
                "",
                &ClassificationKind::RequiresAdapter,
            )?,
        })
    }
}

impl UnrealStaticSceneAdapter {
    /// Converts an already typed bridge report plus its exact staged FBX and TGA bytes.
    ///
    /// # Errors
    ///
    /// Returns a typed error for an unexpected report shape, malformed or unsupported
    /// binary FBX/TGA data, nonfinite geometry, or canonical graph violation.
    pub fn convert(
        &self,
        report: &UnrealBridgeReport,
        fbx_bytes: &[u8],
        tga_bytes: &[u8],
    ) -> Result<UnrealStaticSceneConversion, UnrealStaticSceneError> {
        let records = ApprovedRecords::select(report)?;
        let mesh = parse_mesh(fbx_bytes)?;
        let (normalization, base_color_texture) = normalize_texture(tga_bytes)?;
        let scene = build_scene(&records, mesh, &normalization)?;
        Ok(UnrealStaticSceneConversion {
            scene,
            base_color_texture,
            mesh_source: records.mesh.source.clone(),
            texture_source: records.texture.source.clone(),
            material_classification: records.material.classification.clone(),
            world_classification: records.world.classification.clone(),
        })
    }
}

fn normalize_texture(
    tga_bytes: &[u8],
) -> Result<(TextureNormalization, StagedTexture), UnrealStaticSceneError> {
    let png_bytes = tga_to_png(tga_bytes)?;
    let texture_source =
        SourceAssetPath::from_str("Content/VortexBridgeTemporary/T_VortexBaseColor.tga")
            .map_err(|source| UnrealStaticSceneError::Identity { source })?;
    let normalization = TextureNormalizer
        .normalize([TextureInput {
            id: TextureId::new(1),
            source: texture_source,
            role: TextureRole::BaseColor,
            sampler: TextureSamplerInput::linear_repeat(),
            bytes: png_bytes,
        }])
        .map_err(|source| UnrealStaticSceneError::Texture { source })?;
    let base_color_texture =
        normalization.staged.values().next().cloned().ok_or(UnrealStaticSceneError::Report {
            kind: "Texture2D",
            reason: "normalization produced no staged texture",
        })?;
    Ok((normalization, base_color_texture))
}

fn build_scene(
    records: &ApprovedRecords<'_>,
    (positions, normals, uvs, indices): ParsedMesh,
    normalization: &TextureNormalization,
) -> Result<CanonicalScene, UnrealStaticSceneError> {
    CanonicalScene::new(CanonicalScene {
        normalization: CoordinateNormalization {
            meters_per_unit: "0.010000000000".to_owned(),
            source_up_axis: "Z".to_owned(),
            target_up_axis: "Y".to_owned(),
            winding: "counter_clockwise".to_owned(),
            uv_origin: "bottom_left".to_owned(),
            color_space: "linear_srgb".to_owned(),
        },
        source_links: source_links(records.mesh, records.texture, records.material, records.world),
        nodes: vec![Node {
            id: NodeId::new(1),
            parent: None,
            transform: Transform::default(),
            mesh: Some(MeshId::new(1)),
            camera: None,
            light: None,
            instance: None,
            source_link: SourceLinkId::new(1),
        }],
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
            positions,
            normals: Some(normals),
            tangents: None,
            tex_coords_0: Some(uvs),
            colors_0: None,
            joints_0: None,
            weights_0: None,
            indices,
        }],
        materials: vec![base_color_material()],
        textures: normalization.textures.clone(),
        samplers: normalization.samplers.clone(),
        skins: Vec::new(),
        joints: Vec::new(),
        animation_clips: Vec::new(),
        channels: Vec::new(),
        morph_targets: Vec::new(),
        cameras: Vec::new(),
        lights: Vec::new(),
        instances: Vec::new(),
        unsupported_features: vec![
            unsupported_feature(
                UnsupportedFeatureId::new(1),
                SourceLinkId::new(3),
                "unreal.material_instance",
                records.material.classification.clone(),
            ),
            unsupported_feature(
                UnsupportedFeatureId::new(2),
                SourceLinkId::new(4),
                "unreal.world",
                records.world.classification.clone(),
            ),
        ],
    })
    .map_err(|source| UnrealStaticSceneError::Scene { source })
}

fn required_record<'a>(
    report: &'a UnrealBridgeReport,
    asset_kind: &'static str,
    object_path: &'static str,
    exporter: &'static str,
    output: &'static str,
    classification: &ClassificationKind,
) -> Result<&'a UnrealBridgeRecord, UnrealStaticSceneError> {
    let mut matches = report.records.iter().filter(|record| {
        record.asset_kind == asset_kind && record.source.to_string().ends_with(object_path)
    });
    let record = matches.next().ok_or(UnrealStaticSceneError::Report {
        kind: asset_kind,
        reason: "missing approved source identity",
    })?;
    if matches.next().is_some() {
        return Err(UnrealStaticSceneError::Report {
            kind: asset_kind,
            reason: "duplicate source record",
        });
    }
    if &record.classification.kind != classification {
        return Err(UnrealStaticSceneError::Report {
            kind: asset_kind,
            reason: "unexpected conversion classification",
        });
    }
    if record.exporter != exporter {
        return Err(UnrealStaticSceneError::Report {
            kind: asset_kind,
            reason: "unexpected exporter",
        });
    }
    if output.is_empty() {
        if !record.outputs.is_empty() {
            return Err(UnrealStaticSceneError::Report {
                kind: asset_kind,
                reason: "adapter record emitted output",
            });
        }
    } else if record.outputs.as_slice() != [std::path::PathBuf::from(output)] {
        return Err(UnrealStaticSceneError::Report {
            kind: asset_kind,
            reason: "unexpected staged output",
        });
    }
    Ok(record)
}

fn parse_mesh(bytes: &[u8]) -> Result<ParsedMesh, UnrealStaticSceneError> {
    if bytes.len() > MAX_UNREAL_STATIC_FBX_BYTES {
        return Err(UnrealStaticSceneError::ArtifactTooLarge {
            kind: "FBX",
            actual: bytes.len(),
            limit: MAX_UNREAL_STATIC_FBX_BYTES,
        });
    }
    let document = AnyDocument::from_seekable_reader(Cursor::new(bytes))
        .map_err(|source| UnrealStaticSceneError::Fbx { message: source.to_string() })?;
    let AnyDocument::V7400(_, document) = document else {
        return Err(UnrealStaticSceneError::Fbx {
            message: "unsupported FBX document version".to_owned(),
        });
    };
    let mut meshes = document
        .objects()
        .filter_map(|object| match object.get_typed() {
            TypedObjectHandle::Geometry(TypedGeometryHandle::Mesh(mesh)) => Some(mesh),
            _ => None,
        })
        .filter(|mesh| mesh.models().next().is_some())
        .filter(|mesh| !mesh.name().is_some_and(|name| name.contains("UCX_")));
    let mesh = meshes.next().ok_or_else(|| UnrealStaticSceneError::Fbx {
        message: "controlled export contains no noncollision model-connected mesh geometry"
            .to_owned(),
    })?;
    if meshes.next().is_some() {
        return Err(UnrealStaticSceneError::Fbx {
            message: "controlled export contains more than one noncollision model-connected mesh geometry"
                .to_owned(),
        });
    }
    let polygons = mesh
        .polygon_vertices()
        .map_err(|source| UnrealStaticSceneError::Fbx { message: source.to_string() })?;
    let triangles = polygons
        .triangulate_each(triangulate_fan)
        .map_err(|source| UnrealStaticSceneError::Fbx { message: source.to_string() })?;
    if triangles.is_empty() || !triangles.len().is_multiple_of(3) {
        return Err(UnrealStaticSceneError::Fbx {
            message: "controlled export does not contain a nonempty triangle list".to_owned(),
        });
    }
    if triangles.len() > MAX_UNREAL_STATIC_VERTICES || triangles.len() > MAX_UNREAL_STATIC_INDICES {
        return Err(UnrealStaticSceneError::Fbx {
            message: "controlled export exceeds static-scene vertex/index limits".to_owned(),
        });
    }
    let (normal_layer, uv_layer) = mesh_layers(mesh)?;
    let mut positions = Vec::with_capacity(triangles.len());
    let mut normals = Vec::with_capacity(triangles.len());
    let mut uvs = Vec::with_capacity(triangles.len());
    let mut indices = Vec::with_capacity(triangles.len());
    for triangle_vertex in triangles.triangle_vertex_indices() {
        let position = triangles.control_point(triangle_vertex).ok_or_else(|| {
            UnrealStaticSceneError::Fbx {
                message: "triangle vertex references a missing control point".to_owned(),
            }
        })?;
        let normal = normal_layer
            .normal(&triangles, triangle_vertex)
            .map_err(|source| UnrealStaticSceneError::Fbx { message: source.to_string() })?;
        let uv = uv_layer
            .uv(&triangles, triangle_vertex)
            .map_err(|source| UnrealStaticSceneError::Fbx { message: source.to_string() })?;
        positions.push(unreal_position(position.x, position.y, position.z)?);
        normals.push(unreal_normal(normal.x, normal.y, normal.z)?);
        uvs.push(unreal_uv(uv.x, uv.y)?);
        indices.push(u32::try_from(indices.len()).map_err(|_| UnrealStaticSceneError::Fbx {
            message: "triangle index exceeds u32 range".to_owned(),
        })?);
    }
    let indices = reverse_winding(&indices)?;
    Ok((positions, normals, uvs, indices))
}

fn triangulate_fan(
    _polygons: &PolygonVertices<'_>,
    polygon: &[PolygonVertexIndex],
    output: &mut Vec<[PolygonVertexIndex; 3]>,
) -> Result<(), anyhow::Error> {
    let first = *polygon.first().ok_or_else(|| anyhow::anyhow!("polygon has no vertices"))?;
    if polygon.len() < 3 {
        return Err(anyhow::anyhow!("polygon has fewer than three vertices"));
    }
    for offset in 1..polygon.len() - 1 {
        let second =
            *polygon.get(offset).ok_or_else(|| anyhow::anyhow!("polygon fan is malformed"))?;
        let third =
            *polygon.get(offset + 1).ok_or_else(|| anyhow::anyhow!("polygon fan is malformed"))?;
        output.push([first, second, third]);
    }
    Ok(())
}

fn mesh_layers(
    mesh: fbxcel_dom::v7400::object::geometry::MeshHandle<'_>,
) -> Result<
    (
        fbxcel_dom::v7400::data::mesh::layer::normal::Normals<'_>,
        fbxcel_dom::v7400::data::mesh::layer::uv::Uv<'_>,
    ),
    UnrealStaticSceneError,
> {
    let mut normals = None;
    let mut uvs = None;
    for layer in mesh.layers() {
        for entry in layer.layer_element_entries() {
            let type_name = entry
                .type_str()
                .map_err(|source| UnrealStaticSceneError::Fbx { message: source.to_string() })?;
            let typed_index = entry
                .typed_index()
                .map_err(|source| UnrealStaticSceneError::Fbx { message: source.to_string() })?;
            if typed_index.to_u32() != 0 {
                continue;
            }
            if type_name == "LayerElementNormal" {
                let TypedLayerElementHandle::Normal(handle) =
                    entry.typed_layer_element().map_err(|source| UnrealStaticSceneError::Fbx {
                        message: source.to_string(),
                    })?
                else {
                    return Err(UnrealStaticSceneError::Fbx {
                        message: "normal layer had an unexpected typed handle".to_owned(),
                    });
                };
                if normals.is_some() {
                    return Err(UnrealStaticSceneError::Fbx {
                        message: "controlled export has multiple normal layers".to_owned(),
                    });
                }
                normals = Some(handle.normals().map_err(|source| UnrealStaticSceneError::Fbx {
                    message: source.to_string(),
                })?);
            } else if type_name == "LayerElementUV" {
                let TypedLayerElementHandle::Uv(handle) =
                    entry.typed_layer_element().map_err(|source| UnrealStaticSceneError::Fbx {
                        message: source.to_string(),
                    })?
                else {
                    return Err(UnrealStaticSceneError::Fbx {
                        message: "UV layer had an unexpected typed handle".to_owned(),
                    });
                };
                if uvs.is_some() {
                    return Err(UnrealStaticSceneError::Fbx {
                        message: "controlled export has multiple UV layers".to_owned(),
                    });
                }
                uvs = Some(handle.uv().map_err(|source| UnrealStaticSceneError::Fbx {
                    message: source.to_string(),
                })?);
            }
        }
    }
    let normals = normals.ok_or_else(|| UnrealStaticSceneError::Fbx {
        message: "controlled export lacks a normal layer".to_owned(),
    })?;
    let uvs = uvs.ok_or_else(|| UnrealStaticSceneError::Fbx {
        message: "controlled export lacks a UV layer".to_owned(),
    })?;
    Ok((normals, uvs))
}

fn unreal_position(x: f64, y: f64, z: f64) -> Result<[f32; 3], UnrealStaticSceneError> {
    finite_f32([x, z, -y], "position").map(|[x, y, z]| [x * 0.01, y * 0.01, z * 0.01])
}

fn unreal_normal(x: f64, y: f64, z: f64) -> Result<[f32; 3], UnrealStaticSceneError> {
    finite_f32([x, z, -y], "normal")
}

fn unreal_uv(u: f64, v: f64) -> Result<[f32; 2], UnrealStaticSceneError> {
    let [u, v] = finite_f32([u, v], "UV")?;
    Ok([u, 1.0 - v])
}

fn finite_f32<const N: usize>(
    values: [f64; N],
    field: &'static str,
) -> Result<[f32; N], UnrealStaticSceneError> {
    if values.iter().any(|value| !value.is_finite() || value.abs() > f64::from(f32::MAX)) {
        return Err(UnrealStaticSceneError::Fbx { message: format!("nonfinite {field}") });
    }
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the preceding finite range check proves every f64 is representable as a finite f32; precision reduction is the canonical IR contract"
    )]
    let converted = values.map(|value| value as f32);
    Ok(converted)
}

fn reverse_winding(indices: &[u32]) -> Result<Vec<u32>, UnrealStaticSceneError> {
    let mut canonical = Vec::with_capacity(indices.len());
    for triangle in indices.chunks_exact(3) {
        let first = *triangle.first().ok_or_else(|| UnrealStaticSceneError::Fbx {
            message: "triangle index list is malformed".to_owned(),
        })?;
        let second = *triangle.get(1).ok_or_else(|| UnrealStaticSceneError::Fbx {
            message: "triangle index list is malformed".to_owned(),
        })?;
        let third = *triangle.get(2).ok_or_else(|| UnrealStaticSceneError::Fbx {
            message: "triangle index list is malformed".to_owned(),
        })?;
        canonical.extend([first, third, second]);
    }
    if canonical.len() != indices.len() {
        return Err(UnrealStaticSceneError::Fbx {
            message: "triangle index list is not divisible by three".to_owned(),
        });
    }
    Ok(canonical)
}

fn tga_to_png(tga: &[u8]) -> Result<Vec<u8>, UnrealStaticSceneError> {
    if tga.len() > MAX_UNREAL_STATIC_TGA_BYTES {
        return Err(UnrealStaticSceneError::ArtifactTooLarge {
            kind: "TGA",
            actual: tga.len(),
            limit: MAX_UNREAL_STATIC_TGA_BYTES,
        });
    }
    let image = image::load_from_memory_with_format(tga, ImageFormat::Tga)
        .map_err(|source| UnrealStaticSceneError::Tga { message: source.to_string() })?;
    let (width, height) = (image.width(), image.height());
    if width == 0
        || height == 0
        || width > MAX_UNREAL_STATIC_TEXTURE_DIMENSION
        || height > MAX_UNREAL_STATIC_TEXTURE_DIMENSION
    {
        return Err(UnrealStaticSceneError::Tga {
            message: "decoded TGA dimensions exceed the controlled static-scene limit".to_owned(),
        });
    }
    let mut output = Cursor::new(Vec::new());
    image
        .write_to(&mut output, ImageFormat::Png)
        .map_err(|source| UnrealStaticSceneError::Tga { message: source.to_string() })?;
    Ok(output.into_inner())
}

fn source_links(
    mesh: &UnrealBridgeRecord,
    texture: &UnrealBridgeRecord,
    material: &UnrealBridgeRecord,
    world: &UnrealBridgeRecord,
) -> Vec<SourceLink> {
    vec![
        source_link(SourceLinkId::new(1), mesh, "unreal.static_scene.mesh_output"),
        source_link(SourceLinkId::new(2), texture, "unreal.static_scene.texture_output"),
        source_link(SourceLinkId::new(3), material, "unreal.static_scene.material_boundary"),
        source_link(SourceLinkId::new(4), world, "unreal.static_scene.world_boundary"),
    ]
}

fn source_link(id: SourceLinkId, record: &UnrealBridgeRecord, fact_key: &str) -> SourceLink {
    let mut facts = BTreeMap::new();
    facts.insert("unreal.asset_kind".to_owned(), record.asset_kind.clone());
    facts.insert("unreal.exporter".to_owned(), record.exporter.clone());
    facts.insert(
        fact_key.to_owned(),
        record
            .outputs
            .first()
            .map_or_else(|| "none".to_owned(), |output| output.display().to_string()),
    );
    SourceLink { id, source: record.source.clone(), facts }
}

fn base_color_material() -> Material {
    let mut textures = BTreeMap::new();
    textures.insert("base_color".to_owned(), TextureId::new(1));
    Material {
        id: MaterialId::new(1),
        family: "pbr_metallic_roughness".to_owned(),
        base_color: [1.0, 1.0, 1.0, 1.0],
        metallic: 0.0,
        roughness: 0.5,
        normal_scale: 1.0,
        occlusion_strength: 1.0,
        emissive: [0.0, 0.0, 0.0],
        alpha_mode: AlphaMode::Opaque,
        alpha_cutoff: None,
        double_sided: false,
        texture_transform: TextureTransform::default(),
        textures,
    }
}

fn unsupported_feature(
    id: UnsupportedFeatureId,
    source_link: SourceLinkId,
    feature: &str,
    classification: ConversionClassification,
) -> UnsupportedFeature {
    UnsupportedFeature { id, source_link, feature: feature.to_owned(), classification }
}

#[cfg(test)]
mod tests {
    use super::{UnrealStaticSceneError, reverse_winding, tga_to_png};

    #[test]
    fn malformed_tga_fails_at_the_binary_boundary() {
        let result = tga_to_png(b"not-a-tga");
        assert!(matches!(result, Err(UnrealStaticSceneError::Tga { .. })));
    }

    #[test]
    fn winding_reversal_is_triangle_local_and_rejects_partial_triangles() -> Result<(), String> {
        let reversed = reverse_winding(&[0, 1, 2, 3, 4, 5])
            .map_err(|error| format!("winding reversal failed unexpectedly: {error}"))?;
        assert_eq!(reversed, vec![0, 2, 1, 3, 5, 4]);
        assert!(matches!(reverse_winding(&[0, 1]), Err(UnrealStaticSceneError::Fbx { .. })));
        Ok(())
    }
}
