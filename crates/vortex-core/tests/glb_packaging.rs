use std::{collections::BTreeMap, error::Error};

use gltf::{Gltf, Semantic};
use vortex_core::{
    adapters::secondary::glb::{GlbPackageError, GlbWriter},
    domain::{
        ProjectRoot, SourceAssetPath, SourceIdentity, UnrealObjectPath,
        scene_ir::{
            AlphaMode, AnimationClip, AnimationClipId, AnimationKey, Camera, CameraId,
            CanonicalScene, Channel, ChannelId, CoordinateNormalization, Joint, JointId, Light,
            LightId, Material, MaterialId, Mesh, MeshId, MorphTarget, MorphTargetId, Node, NodeId,
            Primitive, PrimitiveId, Skin, SkinId, SourceLink, SourceLinkId, Texture,
            TextureColorSpace, TextureId, TextureTransform, Transform, VertexStream,
            VertexStreamId,
        },
    },
};

fn source() -> Result<SourceIdentity, Box<dyn Error>> {
    Ok(SourceIdentity::Unreal {
        project_root: "fixture".parse::<ProjectRoot>()?,
        asset_path: "Content/Crate.uasset".parse::<SourceAssetPath>()?,
        object_path: "/Game/Crate.Crate".parse::<UnrealObjectPath>()?,
    })
}

fn normalization() -> CoordinateNormalization {
    CoordinateNormalization {
        meters_per_unit: "1.000000000000".to_owned(),
        source_up_axis: "Y".to_owned(),
        target_up_axis: "Y".to_owned(),
        winding: "counter_clockwise".to_owned(),
        uv_origin: "bottom_left".to_owned(),
        color_space: "linear_srgb".to_owned(),
    }
}

fn material(id: u32) -> Material {
    Material {
        id: MaterialId::new(id),
        family: "pbr_metallic_roughness".to_owned(),
        base_color: [1.0, 1.0, 1.0, 1.0],
        metallic: 0.0,
        roughness: 1.0,
        normal_scale: 1.0,
        occlusion_strength: 1.0,
        emissive: [0.0, 0.0, 0.0],
        alpha_mode: AlphaMode::Opaque,
        alpha_cutoff: None,
        double_sided: false,
        texture_transform: TextureTransform::default(),
        textures: BTreeMap::new(),
    }
}

fn base_scene() -> Result<CanonicalScene, Box<dyn Error>> {
    Ok(CanonicalScene::new(CanonicalScene {
        normalization: normalization(),
        source_links: vec![SourceLink {
            id: SourceLinkId::new(1),
            source: source()?,
            facts: BTreeMap::new(),
        }],
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
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            normals: None,
            tangents: None,
            tex_coords_0: None,
            colors_0: None,
            joints_0: None,
            weights_0: None,
            indices: vec![0, 1, 2],
        }],
        materials: vec![material(1)],
        textures: Vec::new(),
        samplers: Vec::new(),
        skins: Vec::new(),
        joints: Vec::new(),
        animation_clips: Vec::new(),
        channels: Vec::new(),
        morph_targets: Vec::new(),
        cameras: Vec::new(),
        lights: Vec::new(),
        instances: Vec::new(),
        unsupported_features: Vec::new(),
    })?)
}

#[test]
fn repeated_minimal_package_is_byte_stable_and_structurally_parseable() -> Result<(), Box<dyn Error>>
{
    let scene = base_scene()?;
    let writer = GlbWriter;
    let first = writer.package(&scene)?;
    let second = writer.package(&scene)?;
    let document = Gltf::from_slice(&first.bytes)?;

    assert_eq!(first, second);
    assert_eq!(first.checksum.as_str().len(), 64);
    assert_eq!(document.scenes().count(), 1);
    assert_eq!(document.nodes().count(), 1);
    assert_eq!(document.meshes().count(), 1);
    assert_eq!(document.materials().count(), 1);
    assert_eq!(first.labels.get("Scene0"), Some(&0));
    assert_eq!(first.labels.get("Mesh1"), Some(&1));
    Ok(())
}

#[test]
fn multiple_primitives_materials_camera_and_light_are_emitted_deterministically()
-> Result<(), Box<dyn Error>> {
    let mut scene = base_scene()?;
    scene.materials.push(material(2));
    scene.vertex_streams.push(VertexStream {
        id: VertexStreamId::new(2),
        positions: vec![[0.0, 0.0, 1.0], [1.0, 0.0, 1.0], [0.0, 1.0, 1.0]],
        normals: None,
        tangents: None,
        tex_coords_0: None,
        colors_0: None,
        joints_0: None,
        weights_0: None,
        indices: vec![0, 1, 2],
    });
    scene.primitives.push(Primitive {
        id: PrimitiveId::new(2),
        vertex_stream: VertexStreamId::new(2),
        material: Some(MaterialId::new(2)),
        morph_targets: Vec::new(),
    });
    let mesh = scene.meshes.first_mut().ok_or("missing mesh")?;
    mesh.primitives.push(PrimitiveId::new(2));
    scene.cameras.push(Camera {
        id: CameraId::new(1),
        projection: "perspective".to_owned(),
        primary_extent: 1.0,
    });
    scene.lights.push(Light {
        id: LightId::new(1),
        kind: "directional".to_owned(),
        intensity: 1.0,
    });
    scene.nodes.push(Node {
        id: NodeId::new(2),
        parent: None,
        transform: Transform::default(),
        mesh: None,
        camera: Some(CameraId::new(1)),
        light: Some(LightId::new(1)),
        instance: None,
        source_link: SourceLinkId::new(1),
    });
    let scene = CanonicalScene::new(scene)?;
    let package = GlbWriter.package(&scene)?;
    let document = Gltf::from_slice(&package.bytes)?;
    let mesh = document.meshes().next().ok_or("missing packaged mesh")?;

    assert_eq!(mesh.primitives().count(), 2);
    assert_eq!(document.materials().count(), 2);
    assert_eq!(document.cameras().count(), 1);
    assert_eq!(document.nodes().count(), 2);
    assert_eq!(package.labels.get("Material2"), Some(&2));
    Ok(())
}

#[test]
fn malformed_ir_rejects_while_textures_skins_animations_and_morphs_package()
-> Result<(), Box<dyn Error>> {
    let mut malformed = base_scene()?;
    let node = malformed.nodes.first_mut().ok_or("missing node")?;
    node.source_link = SourceLinkId::new(99);
    assert!(matches!(GlbWriter.package(&malformed), Err(GlbPackageError::SceneInvalid { .. })));

    let mut textured = base_scene()?;
    let material = textured.materials.first_mut().ok_or("missing material")?;
    material.textures.insert("base_color".to_owned(), TextureId::new(1));
    textured.textures.push(Texture {
        id: TextureId::new(1),
        image: "crate.png".to_owned(),
        color_space: TextureColorSpace::Srgb,
        sampler: None,
    });
    let textured_package = GlbWriter.package(&textured)?;
    let textured_document = Gltf::from_slice(&textured_package.bytes)?;
    assert_eq!(textured_document.textures().count(), 1);

    let mut deformed = base_scene()?;
    let stream = deformed.vertex_streams.first_mut().ok_or("missing stream")?;
    stream.joints_0 = Some(vec![[0, 0, 0, 0]; 3]);
    stream.weights_0 = Some(vec![[1.0, 0.0, 0.0, 0.0]; 3]);
    let mesh = deformed.meshes.first_mut().ok_or("missing mesh")?;
    mesh.skin = Some(SkinId::new(1));
    let primitive = deformed.primitives.first_mut().ok_or("missing primitive")?;
    primitive.morph_targets.push(MorphTargetId::new(1));
    deformed.joints.push(Joint { id: JointId::new(1), node: NodeId::new(1) });
    deformed.skins.push(Skin {
        id: SkinId::new(1),
        joints: vec![JointId::new(1)],
        inverse_bind_matrices: vec![[
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ]],
    });
    deformed.morph_targets.push(MorphTarget {
        id: MorphTargetId::new(1),
        name: "Raise".to_owned(),
        primitive: PrimitiveId::new(1),
        position_deltas: vec![[0.0, 0.0, 0.1]; 3],
    });
    deformed.channels.push(Channel {
        id: ChannelId::new(1),
        node: NodeId::new(1),
        property: "translation".to_owned(),
        keys: vec![
            AnimationKey { time_seconds: 0.0, value: vec![0.0, 0.0, 0.0] },
            AnimationKey { time_seconds: 1.0, value: vec![1.0, 0.0, 0.0] },
        ],
    });
    deformed.animation_clips.push(AnimationClip {
        id: AnimationClipId::new(1),
        name: "Move".to_owned(),
        channels: vec![ChannelId::new(1)],
    });
    let deformed = CanonicalScene::new(deformed)?;
    let deformed_package = GlbWriter.package(&deformed)?;
    let deformed_document = Gltf::from_slice(&deformed_package.bytes)?;
    assert_eq!(deformed_document.skins().count(), 1);
    assert_eq!(deformed_document.animations().count(), 1);
    let packaged_mesh = deformed_document.meshes().next().ok_or("missing deformed mesh")?;
    let packaged_primitive = packaged_mesh.primitives().next().ok_or("missing primitive")?;
    assert!(packaged_primitive.get(&Semantic::Joints(0)).is_some());
    assert!(packaged_primitive.get(&Semantic::Weights(0)).is_some());
    assert_eq!(packaged_primitive.morph_targets().count(), 1);
    Ok(())
}
