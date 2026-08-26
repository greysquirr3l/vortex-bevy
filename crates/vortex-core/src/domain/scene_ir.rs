//! Engine-neutral canonical scene representation for deterministic packaging.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use thiserror::Error;

use super::{ConversionClassification, SourceIdentity};

macro_rules! ir_id {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
        #[serde(transparent)]
        pub struct $name(u32);

        impl $name {
            #[must_use]
            pub const fn new(value: u32) -> Self {
                Self(value)
            }

            #[must_use]
            pub const fn get(self) -> u32 {
                self.0
            }
        }
    };
}

ir_id!(NodeId);
ir_id!(MeshId);
ir_id!(PrimitiveId);
ir_id!(VertexStreamId);
ir_id!(MaterialId);
ir_id!(TextureId);
ir_id!(SamplerId);
ir_id!(SkinId);
ir_id!(JointId);
ir_id!(AnimationClipId);
ir_id!(ChannelId);
ir_id!(MorphTargetId);
ir_id!(CameraId);
ir_id!(LightId);
ir_id!(InstanceId);
ir_id!(SourceLinkId);
ir_id!(UnsupportedFeatureId);

/// Coordinate conventions normalized before artifact packaging.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CoordinateNormalization {
    /// Source metric in meters per source unit.
    pub meters_per_unit: String,
    /// Source up-axis token.
    pub source_up_axis: String,
    /// Target up-axis token.
    pub target_up_axis: String,
    /// Winding conversion decision retained in provenance.
    pub winding: String,
    /// UV origin conversion decision retained in provenance.
    pub uv_origin: String,
    /// Source texture color-space decision retained in provenance.
    pub color_space: String,
}

/// Affine node transform in normalized scene space.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Transform {
    /// Translation components.
    pub translation: [f64; 3],
    /// Unit-quaternion rotation components.
    pub rotation: [f64; 4],
    /// Nonzero scale components.
    pub scale: [f64; 3],
}

impl Default for Transform {
    fn default() -> Self {
        Self { translation: [0.0; 3], rotation: [0.0, 0.0, 0.0, 1.0], scale: [1.0; 3] }
    }
}

/// Canonical render graph node.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Node {
    /// Stable numeric node identity.
    pub id: NodeId,
    /// Optional parent node.
    pub parent: Option<NodeId>,
    /// Normalized local transform.
    pub transform: Transform,
    /// Optional render mesh instance.
    pub mesh: Option<MeshId>,
    /// Optional camera attachment.
    pub camera: Option<CameraId>,
    /// Optional light attachment.
    pub light: Option<LightId>,
    /// Optional shared instance attachment.
    pub instance: Option<InstanceId>,
    /// Provenance for source-specific facts.
    pub source_link: SourceLinkId,
}

/// Mesh grouped by primitive references.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Mesh {
    /// Stable mesh identity.
    pub id: MeshId,
    /// Ordered primitive references.
    pub primitives: Vec<PrimitiveId>,
    /// Optional skin binding.
    pub skin: Option<SkinId>,
}

/// One material-bound geometry section.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Primitive {
    /// Stable primitive identity.
    pub id: PrimitiveId,
    /// Vertex/index stream reference.
    pub vertex_stream: VertexStreamId,
    /// Optional material slot.
    pub material: Option<MaterialId>,
    /// Optional morph targets.
    pub morph_targets: Vec<MorphTargetId>,
}

/// Indexed geometric source retained before GLB binary encoding.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct VertexStream {
    /// Stable stream identity.
    pub id: VertexStreamId,
    /// Position tuples in normalized coordinates.
    pub positions: Vec<[f32; 3]>,
    /// Optional normalized surface normals aligned to positions.
    pub normals: Option<Vec<[f32; 3]>>,
    /// Optional tangent vectors aligned to positions.
    pub tangents: Option<Vec<[f32; 4]>>,
    /// Optional primary UV coordinates aligned to positions.
    pub tex_coords_0: Option<Vec<[f32; 2]>>,
    /// Optional primary vertex colors aligned to positions.
    pub colors_0: Option<Vec<[f32; 4]>>,
    /// Optional four-joint influence indices aligned to positions.
    pub joints_0: Option<Vec<[u16; 4]>>,
    /// Optional four-joint influence weights aligned to positions.
    pub weights_0: Option<Vec<[f32; 4]>>,
    /// Triangle indices.
    pub indices: Vec<u32>,
}

/// Portable glTF alpha behavior.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum AlphaMode {
    /// Fully opaque output.
    Opaque,
    /// Alpha cutoff output.
    Mask,
    /// Alpha blending output.
    Blend,
}

/// Texture color-space intent retained through source normalization.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum TextureColorSpace {
    /// sRGB data such as base color or emissive textures.
    Srgb,
    /// Linear data such as normal, occlusion, metallic, or roughness textures.
    Linear,
}

/// Portable magnification/minification filtering decision.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum TextureFilter {
    /// Nearest-neighbor filtering.
    Nearest,
    /// Linear filtering.
    Linear,
}

/// Portable texture-coordinate wrapping decision.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum TextureWrap {
    /// Repeat outside the normalized coordinate interval.
    Repeat,
    /// Clamp outside the normalized coordinate interval.
    ClampToEdge,
    /// Mirror-and-repeat outside the normalized coordinate interval.
    MirroredRepeat,
}

/// Source-neutral UV tiling and offset retained for a texture binding.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TextureTransform {
    /// U/V scale values.
    pub scale: [f32; 2],
    /// U/V offset values.
    pub offset: [f32; 2],
}

impl Default for TextureTransform {
    fn default() -> Self {
        Self { scale: [1.0, 1.0], offset: [0.0, 0.0] }
    }
}

/// Portable PBR material identity, factors, and texture-slot references.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Material {
    /// Stable material identity.
    pub id: MaterialId,
    /// Canonical material family label such as `pbr_metallic_roughness`.
    pub family: String,
    /// Base color RGBA factor.
    pub base_color: [f32; 4],
    /// Metallic factor.
    pub metallic: f32,
    /// Roughness factor.
    pub roughness: f32,
    /// Normal-map scaling factor.
    pub normal_scale: f32,
    /// Occlusion-strength factor.
    pub occlusion_strength: f32,
    /// Linear emissive RGB factor.
    pub emissive: [f32; 3],
    /// Portable alpha behavior.
    pub alpha_mode: AlphaMode,
    /// Optional alpha threshold when `alpha_mode` is `Mask`.
    pub alpha_cutoff: Option<f32>,
    /// Whether back-face culling is disabled.
    pub double_sided: bool,
    /// Source-neutral UV transform retained for portable texture bindings.
    pub texture_transform: TextureTransform,
    /// Deterministically named texture slots.
    pub textures: BTreeMap<String, TextureId>,
}

/// Texture image identity, color-space intent, and sampler relationship.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Texture {
    /// Stable texture identity.
    pub id: TextureId,
    /// Staging-relative image location.
    pub image: String,
    /// Required source color-space intent.
    pub color_space: TextureColorSpace,
    /// Optional sampler.
    pub sampler: Option<SamplerId>,
}

/// Portable texture sampling parameters.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Sampler {
    /// Stable sampler identity.
    pub id: SamplerId,
    /// Magnification filter.
    pub mag_filter: TextureFilter,
    /// Optional minification filter.
    pub min_filter: Option<TextureFilter>,
    /// U-axis wrapping mode.
    pub wrap_s: TextureWrap,
    /// V-axis wrapping mode.
    pub wrap_t: TextureWrap,
}

/// Joint declaration owned by a skin.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Joint {
    /// Stable joint identity.
    pub id: JointId,
    /// Node that drives this joint.
    pub node: NodeId,
}

/// Skin mapping mesh deformation to ordered joints.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Skin {
    /// Stable skin identity.
    pub id: SkinId,
    /// Ordered joints.
    pub joints: Vec<JointId>,
    /// Inverse bind matrices aligned one-for-one with joints.
    pub inverse_bind_matrices: Vec<[f32; 16]>,
}

/// Morph target position deltas associated with one primitive.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct MorphTarget {
    /// Stable morph target identity.
    pub id: MorphTargetId,
    /// Name retained for user inspection.
    pub name: String,
    /// Primitive receiving this target.
    pub primitive: PrimitiveId,
    /// Position deltas aligned to the primitive vertex stream.
    pub position_deltas: Vec<[f32; 3]>,
}

/// Animation keyframe time/value fact.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AnimationKey {
    /// Time in seconds.
    pub time_seconds: f64,
    /// Owned value tuple.
    pub value: Vec<f32>,
}

/// Animation channel targeting a node transform or morph weights.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Channel {
    /// Stable channel identity.
    pub id: ChannelId,
    /// Target node.
    pub node: NodeId,
    /// `translation`, `rotation`, `scale`, or `weights`.
    pub property: String,
    /// Ordered keyframes.
    pub keys: Vec<AnimationKey>,
}

/// Animation clip grouped by channel references.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AnimationClip {
    /// Stable clip identity.
    pub id: AnimationClipId,
    /// Deterministic clip label.
    pub name: String,
    /// Ordered channel references.
    pub channels: Vec<ChannelId>,
}

/// Camera attachment facts.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Camera {
    /// Stable camera identity.
    pub id: CameraId,
    /// `perspective` or `orthographic`.
    pub projection: String,
    /// Vertical field of view or orthographic scale.
    pub primary_extent: f64,
}

/// Light attachment facts.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Light {
    /// Stable light identity.
    pub id: LightId,
    /// Portable light kind token.
    pub kind: String,
    /// Normalized intensity.
    pub intensity: f64,
}

/// Explicit shared-instance linkage.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Instance {
    /// Stable instance identity.
    pub id: InstanceId,
    /// Mesh shared by the instance.
    pub mesh: MeshId,
}

/// Source-specific provenance held outside render primitives.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SourceLink {
    /// Stable source-link identity.
    pub id: SourceLinkId,
    /// Typed originating source identity.
    pub source: SourceIdentity,
    /// Deterministically keyed engine-source facts.
    pub facts: BTreeMap<String, String>,
}

/// Explicit nonportable capability retained with actionable classification.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct UnsupportedFeature {
    /// Stable unsupported-feature identity.
    pub id: UnsupportedFeatureId,
    /// Source-link reporting the feature.
    pub source_link: SourceLinkId,
    /// Stable source feature label.
    pub feature: String,
    /// Actionable classification.
    pub classification: ConversionClassification,
}

/// Fully validated, engine-neutral canonical scene.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CanonicalScene {
    /// Normalization decisions applied at this boundary.
    pub normalization: CoordinateNormalization,
    /// Source provenance ordered by stable ID.
    pub source_links: Vec<SourceLink>,
    /// Scene graph nodes ordered by stable ID.
    pub nodes: Vec<Node>,
    /// Render meshes ordered by stable ID.
    pub meshes: Vec<Mesh>,
    /// Mesh primitives ordered by stable ID.
    pub primitives: Vec<Primitive>,
    /// Vertex streams ordered by stable ID.
    pub vertex_streams: Vec<VertexStream>,
    /// Materials ordered by stable ID.
    pub materials: Vec<Material>,
    /// Textures ordered by stable ID.
    pub textures: Vec<Texture>,
    /// Samplers ordered by stable ID.
    pub samplers: Vec<Sampler>,
    /// Skins ordered by stable ID.
    pub skins: Vec<Skin>,
    /// Joints ordered by stable ID.
    pub joints: Vec<Joint>,
    /// Animation clips ordered by stable ID.
    pub animation_clips: Vec<AnimationClip>,
    /// Animation channels ordered by stable ID.
    pub channels: Vec<Channel>,
    /// Morph targets ordered by stable ID.
    pub morph_targets: Vec<MorphTarget>,
    /// Cameras ordered by stable ID.
    pub cameras: Vec<Camera>,
    /// Lights ordered by stable ID.
    pub lights: Vec<Light>,
    /// Shared instances ordered by stable ID.
    pub instances: Vec<Instance>,
    /// Explicit unsupported source features ordered by stable ID.
    pub unsupported_features: Vec<UnsupportedFeature>,
}

/// Validation failures for the canonical graph.
#[derive(Debug, Error)]
pub enum SceneIrError {
    /// An ID appeared more than once in an IR collection.
    #[error("canonical scene contains a duplicate {kind} ID: {id}")]
    DuplicateId { kind: &'static str, id: u32 },
    /// An ID reference did not resolve to a graph object.
    #[error("canonical scene has a dangling {kind} reference: {id}")]
    DanglingReference { kind: &'static str, id: u32 },
    /// Parent references formed a cycle.
    #[error("canonical scene node parent graph contains a cycle at node {node}")]
    ParentCycle { node: u32 },
    /// A floating-point render fact was nonfinite or otherwise invalid.
    #[error("canonical scene has an invalid numeric value in {field}")]
    InvalidNumber { field: &'static str },
    /// Animation keys are not monotonic and finite.
    #[error("canonical scene animation channel {channel} has invalid key times")]
    InvalidAnimationTimes { channel: u32 },
    /// A primitive index or aligned vertex attribute is invalid.
    #[error("canonical scene primitive {primitive} has invalid vertex data")]
    InvalidVertexIndex { primitive: u32 },
    /// A skin has invalid joints, matrices, or a joint count beyond the supported subset.
    #[error("canonical scene skin {skin} has invalid deformation data")]
    InvalidSkin { skin: u32 },
    /// A morph target is not aligned to its owning primitive stream.
    #[error("canonical scene morph target {morph} has invalid position deltas")]
    InvalidMorphTarget { morph: u32 },
    /// An animation clip has no stable label or unresolved channel semantics.
    #[error("canonical scene animation clip {clip} is invalid")]
    InvalidAnimationClip { clip: u32 },
}

impl CanonicalScene {
    /// Canonicalizes and validates all typed graph references.
    ///
    /// # Errors
    ///
    /// Returns a typed diagnostic for duplicate IDs, dangling references, cycles,
    /// nonfinite numeric values, invalid animation keys, or invalid vertex indices.
    pub fn new(mut scene: Self) -> Result<Self, SceneIrError> {
        scene.sort_collections();
        scene.validate()?;
        Ok(scene)
    }

    /// Serializes the validated scene in deterministic JSON field and collection order.
    ///
    /// # Errors
    ///
    /// Returns a JSON serialization error if an upstream type gains an incompatible
    /// serialization implementation.
    pub fn to_canonical_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    fn sort_collections(&mut self) {
        self.source_links.sort_by_key(|value| value.id);
        self.nodes.sort_by_key(|value| value.id);
        self.meshes.sort_by_key(|value| value.id);
        self.primitives.sort_by_key(|value| value.id);
        self.vertex_streams.sort_by_key(|value| value.id);
        self.materials.sort_by_key(|value| value.id);
        self.textures.sort_by_key(|value| value.id);
        self.samplers.sort_by_key(|value| value.id);
        self.skins.sort_by_key(|value| value.id);
        self.joints.sort_by_key(|value| value.id);
        self.animation_clips.sort_by_key(|value| value.id);
        self.channels.sort_by_key(|value| value.id);
        self.morph_targets.sort_by_key(|value| value.id);
        self.cameras.sort_by_key(|value| value.id);
        self.lights.sort_by_key(|value| value.id);
        self.instances.sort_by_key(|value| value.id);
        self.unsupported_features.sort_by_key(|value| value.id);
    }

    fn validate(&self) -> Result<(), SceneIrError> {
        let source_links =
            unique_ids("source link", self.source_links.iter().map(|value| value.id.get()))?;
        let nodes = unique_ids("node", self.nodes.iter().map(|value| value.id.get()))?;
        let meshes = unique_ids("mesh", self.meshes.iter().map(|value| value.id.get()))?;
        let primitives =
            unique_ids("primitive", self.primitives.iter().map(|value| value.id.get()))?;
        let vertex_streams =
            unique_ids("vertex stream", self.vertex_streams.iter().map(|value| value.id.get()))?;
        let materials = unique_ids("material", self.materials.iter().map(|value| value.id.get()))?;
        let textures = unique_ids("texture", self.textures.iter().map(|value| value.id.get()))?;
        let samplers = unique_ids("sampler", self.samplers.iter().map(|value| value.id.get()))?;
        let skins = unique_ids("skin", self.skins.iter().map(|value| value.id.get()))?;
        let joints = unique_ids("joint", self.joints.iter().map(|value| value.id.get()))?;
        let channels = unique_ids("channel", self.channels.iter().map(|value| value.id.get()))?;
        let cameras = unique_ids("camera", self.cameras.iter().map(|value| value.id.get()))?;
        let lights = unique_ids("light", self.lights.iter().map(|value| value.id.get()))?;
        let instances = unique_ids("instance", self.instances.iter().map(|value| value.id.get()))?;
        let morphs =
            unique_ids("morph target", self.morph_targets.iter().map(|value| value.id.get()))?;
        unique_ids("animation clip", self.animation_clips.iter().map(|value| value.id.get()))?;
        unique_ids(
            "unsupported feature",
            self.unsupported_features.iter().map(|value| value.id.get()),
        )?;

        for node in &self.nodes {
            validate_transform(&node.transform)?;
            validate_optional(node.parent.map(NodeId::get), &nodes, "parent node")?;
            validate_optional(node.mesh.map(MeshId::get), &meshes, "mesh")?;
            validate_optional(node.camera.map(CameraId::get), &cameras, "camera")?;
            validate_optional(node.light.map(LightId::get), &lights, "light")?;
            validate_optional(node.instance.map(InstanceId::get), &instances, "instance")?;
            validate_required(node.source_link.get(), &source_links, "source link")?;
        }
        validate_parent_cycles(&self.nodes)?;
        for mesh in &self.meshes {
            validate_optional(mesh.skin.map(SkinId::get), &skins, "skin")?;
            for primitive in &mesh.primitives {
                validate_required(primitive.get(), &primitives, "primitive")?;
            }
        }
        for primitive in &self.primitives {
            validate_required(primitive.vertex_stream.get(), &vertex_streams, "vertex stream")?;
            validate_optional(primitive.material.map(MaterialId::get), &materials, "material")?;
            for morph in &primitive.morph_targets {
                validate_required(morph.get(), &morphs, "morph target")?;
            }
            let stream = self
                .vertex_streams
                .iter()
                .find(|stream| stream.id == primitive.vertex_stream)
                .ok_or_else(|| SceneIrError::DanglingReference {
                    kind: "vertex stream",
                    id: primitive.vertex_stream.get(),
                })?;
            validate_vertex_stream(primitive.id, stream)?;
        }
        for material in &self.materials {
            validate_material(material)?;
            for texture in material.textures.values() {
                validate_required(texture.get(), &textures, "texture")?;
            }
        }
        for texture in &self.textures {
            validate_optional(texture.sampler.map(SamplerId::get), &samplers, "sampler")?;
        }
        validate_deformation(self, &nodes, &joints, &channels)?;
        for camera in &self.cameras {
            if !camera.primary_extent.is_finite() || camera.primary_extent <= 0.0 {
                return Err(SceneIrError::InvalidNumber { field: "camera.primary_extent" });
            }
        }
        for light in &self.lights {
            if !light.intensity.is_finite() || light.intensity < 0.0 {
                return Err(SceneIrError::InvalidNumber { field: "light.intensity" });
            }
        }
        for instance in &self.instances {
            validate_required(instance.mesh.get(), &meshes, "instance mesh")?;
        }
        for feature in &self.unsupported_features {
            validate_required(feature.source_link.get(), &source_links, "unsupported source link")?;
        }
        Ok(())
    }
}

fn unique_ids(
    kind: &'static str,
    ids: impl IntoIterator<Item = u32>,
) -> Result<BTreeSet<u32>, SceneIrError> {
    let mut values = BTreeSet::new();
    for id in ids {
        if !values.insert(id) {
            return Err(SceneIrError::DuplicateId { kind, id });
        }
    }
    Ok(values)
}

fn validate_required(
    id: u32,
    values: &BTreeSet<u32>,
    kind: &'static str,
) -> Result<(), SceneIrError> {
    if values.contains(&id) {
        return Ok(());
    }
    Err(SceneIrError::DanglingReference { kind, id })
}

fn validate_optional(
    id: Option<u32>,
    values: &BTreeSet<u32>,
    kind: &'static str,
) -> Result<(), SceneIrError> {
    if let Some(id) = id {
        validate_required(id, values, kind)?;
    }
    Ok(())
}

fn validate_transform(transform: &Transform) -> Result<(), SceneIrError> {
    if transform
        .translation
        .iter()
        .chain(transform.rotation.iter())
        .chain(transform.scale.iter())
        .all(|value| value.is_finite())
        && transform.scale.iter().all(|value| *value != 0.0)
    {
        return Ok(());
    }
    Err(SceneIrError::InvalidNumber { field: "node.transform" })
}

fn validate_material(material: &Material) -> Result<(), SceneIrError> {
    let factors = material
        .base_color
        .iter()
        .chain(material.emissive.iter())
        .chain(material.texture_transform.scale.iter())
        .chain(material.texture_transform.offset.iter())
        .all(|value| value.is_finite());
    let scalar_factors =
        [material.metallic, material.roughness, material.normal_scale, material.occlusion_strength]
            .into_iter()
            .all(f32::is_finite);
    let cutoff = material.alpha_cutoff.is_none_or(f32::is_finite);
    if factors && scalar_factors && cutoff {
        return Ok(());
    }
    Err(SceneIrError::InvalidNumber { field: "material" })
}

fn validate_vertex_stream(
    primitive: PrimitiveId,
    stream: &VertexStream,
) -> Result<(), SceneIrError> {
    let vertex_count = stream.positions.len();
    let finite_positions = stream.positions.iter().flatten().all(|value| value.is_finite());
    let indices = stream
        .indices
        .iter()
        .all(|index| usize::try_from(*index).is_ok_and(|index| index < vertex_count));
    let normals = optional_vec3_matches(stream.normals.as_ref(), vertex_count);
    let tangents = stream.tangents.as_ref().is_none_or(|values| {
        values.len() == vertex_count && values.iter().flatten().all(|value| value.is_finite())
    });
    let tex_coords = stream.tex_coords_0.as_ref().is_none_or(|values| {
        values.len() == vertex_count && values.iter().flatten().all(|value| value.is_finite())
    });
    let colors = stream.colors_0.as_ref().is_none_or(|values| {
        values.len() == vertex_count
            && values.iter().flatten().all(|value| value.is_finite() && (0.0..=1.0).contains(value))
    });
    let influences = validate_influences(stream, vertex_count);
    if finite_positions && indices && normals && tangents && tex_coords && colors && influences {
        return Ok(());
    }
    Err(SceneIrError::InvalidVertexIndex { primitive: primitive.get() })
}

fn optional_vec3_matches(values: Option<&Vec<[f32; 3]>>, vertex_count: usize) -> bool {
    values.is_none_or(|values| {
        values.len() == vertex_count && values.iter().flatten().all(|value| value.is_finite())
    })
}

fn validate_deformation(
    scene: &CanonicalScene,
    nodes: &BTreeSet<u32>,
    joints: &BTreeSet<u32>,
    channels: &BTreeSet<u32>,
) -> Result<(), SceneIrError> {
    for joint in &scene.joints {
        validate_required(joint.node.get(), nodes, "joint node")?;
    }
    for skin in &scene.skins {
        validate_skin(skin)?;
        for joint in &skin.joints {
            validate_required(joint.get(), joints, "skin joint")?;
        }
    }
    for morph in &scene.morph_targets {
        validate_morph_target(morph, &scene.primitives, &scene.vertex_streams)?;
    }
    for clip in &scene.animation_clips {
        if clip.name.trim().is_empty() {
            return Err(SceneIrError::InvalidAnimationClip { clip: clip.id.get() });
        }
        for channel in &clip.channels {
            validate_required(channel.get(), channels, "animation channel")?;
        }
    }
    for channel in &scene.channels {
        validate_required(channel.node.get(), nodes, "animation node")?;
        validate_channel(channel)?;
    }
    Ok(())
}

fn validate_influences(stream: &VertexStream, vertex_count: usize) -> bool {
    match (&stream.joints_0, &stream.weights_0) {
        (None, None) => true,
        (Some(joints), Some(weights)) => {
            joints.len() == vertex_count
                && weights.len() == vertex_count
                && weights.iter().all(|weight| {
                    let finite = weight.iter().all(|value| value.is_finite() && *value >= 0.0);
                    let sum = weight.iter().sum::<f32>();
                    finite && (sum - 1.0).abs() <= 0.000_1
                })
        }
        _ => false,
    }
}

fn validate_skin(skin: &Skin) -> Result<(), SceneIrError> {
    let supported_joint_count = 256;
    if skin.joints.is_empty()
        || skin.joints.len() > supported_joint_count
        || skin.joints.len() != skin.inverse_bind_matrices.len()
        || skin.inverse_bind_matrices.iter().flatten().any(|value| !value.is_finite())
    {
        return Err(SceneIrError::InvalidSkin { skin: skin.id.get() });
    }
    Ok(())
}

fn validate_morph_target(
    morph: &MorphTarget,
    primitives: &[Primitive],
    streams: &[VertexStream],
) -> Result<(), SceneIrError> {
    let primitive = primitives
        .iter()
        .find(|primitive| primitive.id == morph.primitive)
        .ok_or_else(|| SceneIrError::InvalidMorphTarget { morph: morph.id.get() })?;
    let stream = streams
        .iter()
        .find(|stream| stream.id == primitive.vertex_stream)
        .ok_or_else(|| SceneIrError::InvalidMorphTarget { morph: morph.id.get() })?;
    if morph.name.trim().is_empty()
        || morph.position_deltas.len() != stream.positions.len()
        || morph.position_deltas.iter().flatten().any(|value| !value.is_finite())
    {
        return Err(SceneIrError::InvalidMorphTarget { morph: morph.id.get() });
    }
    Ok(())
}

fn validate_channel(channel: &Channel) -> Result<(), SceneIrError> {
    let expected_values = match channel.property.as_str() {
        "translation" | "scale" => Some(3),
        "rotation" => Some(4),
        "weights" => None,
        _ => return Err(SceneIrError::InvalidAnimationTimes { channel: channel.id.get() }),
    };
    let mut previous = None;
    for key in &channel.keys {
        let value_count = expected_values.is_none_or(|expected| key.value.len() == expected)
            && (!key.value.is_empty() || channel.property != "weights");
        let valid = key.time_seconds.is_finite()
            && key.value.iter().all(|value| value.is_finite())
            && previous.is_none_or(|time| key.time_seconds >= time)
            && value_count;
        if !valid {
            return Err(SceneIrError::InvalidAnimationTimes { channel: channel.id.get() });
        }
        previous = Some(key.time_seconds);
    }
    Ok(())
}

fn validate_parent_cycles(nodes: &[Node]) -> Result<(), SceneIrError> {
    let parents = nodes.iter().map(|node| (node.id, node.parent)).collect::<BTreeMap<_, _>>();
    for node in nodes {
        let mut visited = BTreeSet::new();
        let mut current = Some(node.id);
        while let Some(id) = current {
            if !visited.insert(id) {
                return Err(SceneIrError::ParentCycle { node: id.get() });
            }
            current = parents.get(&id).copied().flatten();
        }
    }
    Ok(())
}
