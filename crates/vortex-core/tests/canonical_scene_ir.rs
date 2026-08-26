use std::{collections::BTreeMap, error::Error};

use vortex_core::domain::{
    ProjectRoot, SourceAssetPath, SourceIdentity, StageSelection, UnrealObjectPath, UsdPrimPath,
    scene_ir::{
        AlphaMode, AnimationClip, AnimationClipId, AnimationKey, CanonicalScene, Channel,
        ChannelId, CoordinateNormalization, JointId, Material, MaterialId, Mesh, MeshId, Node,
        NodeId, Primitive, PrimitiveId, SceneIrError, Skin, SkinId, SourceLink, SourceLinkId,
        TextureTransform, Transform, VertexStream, VertexStreamId,
    },
};

fn source_identity() -> Result<SourceIdentity, Box<dyn Error>> {
    Ok(SourceIdentity::Unreal {
        project_root: "fixture".parse::<ProjectRoot>()?,
        asset_path: "Content/Props/Crate.uasset".parse::<SourceAssetPath>()?,
        object_path: "/Game/Props/Crate.Crate".parse::<UnrealObjectPath>()?,
    })
}

fn usd_source_identity() -> Result<SourceIdentity, Box<dyn Error>> {
    Ok(SourceIdentity::Usd {
        stage: "Scenes/Crate.usda".parse::<SourceAssetPath>()?,
        prim_path: "/Crate".parse::<UsdPrimPath>()?,
        selection: "context=universal;payload=forbid;purpose=render;time=stage;variants="
            .parse::<StageSelection>()?,
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

const fn source_link(id: SourceLinkId, source: SourceIdentity) -> SourceLink {
    SourceLink { id, source, facts: BTreeMap::new() }
}

fn node(id: NodeId, source_link: SourceLinkId) -> Node {
    Node {
        id,
        parent: None,
        transform: Transform::default(),
        mesh: Some(MeshId::new(1)),
        camera: None,
        light: None,
        instance: None,
        source_link,
    }
}

fn base_scene(source: SourceIdentity) -> Result<CanonicalScene, Box<dyn Error>> {
    Ok(CanonicalScene::new(CanonicalScene {
        normalization: normalization(),
        source_links: vec![source_link(SourceLinkId::new(1), source)],
        nodes: vec![node(NodeId::new(1), SourceLinkId::new(1))],
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
        materials: vec![Material {
            id: MaterialId::new(1),
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
        }],
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
fn canonical_json_is_stable_when_input_collections_arrive_in_different_order()
-> Result<(), Box<dyn Error>> {
    let mut first = base_scene(source_identity()?)?;
    first.nodes.push(Node {
        id: NodeId::new(2),
        parent: Some(NodeId::new(1)),
        transform: Transform::default(),
        mesh: None,
        camera: None,
        light: None,
        instance: None,
        source_link: SourceLinkId::new(1),
    });
    let first = CanonicalScene::new(first)?;
    let mut second = first.clone();
    second.nodes.reverse();
    let second = CanonicalScene::new(second)?;

    assert_eq!(first, second);
    assert_eq!(first.to_canonical_json()?, second.to_canonical_json()?);
    Ok(())
}

#[test]
fn dangling_references_cycles_nonfinite_transforms_and_invalid_vertex_indices_fail_closed()
-> Result<(), Box<dyn Error>> {
    let mut dangling = base_scene(source_identity()?)?;
    let dangling_node = dangling.nodes.first_mut().ok_or("missing node")?;
    dangling_node.source_link = SourceLinkId::new(99);
    assert!(matches!(
        CanonicalScene::new(dangling),
        Err(SceneIrError::DanglingReference { kind: "source link", .. })
    ));

    let mut cyclic = base_scene(source_identity()?)?;
    cyclic.nodes.push(Node {
        id: NodeId::new(2),
        parent: Some(NodeId::new(1)),
        transform: Transform::default(),
        mesh: None,
        camera: None,
        light: None,
        instance: None,
        source_link: SourceLinkId::new(1),
    });
    let first_node = cyclic.nodes.first_mut().ok_or("missing first node")?;
    first_node.parent = Some(NodeId::new(2));
    assert!(matches!(CanonicalScene::new(cyclic), Err(SceneIrError::ParentCycle { .. })));

    let mut nonfinite = base_scene(source_identity()?)?;
    let nonfinite_node = nonfinite.nodes.first_mut().ok_or("missing node")?;
    nonfinite_node.transform.translation[0] = f64::NAN;
    assert!(matches!(
        CanonicalScene::new(nonfinite),
        Err(SceneIrError::InvalidNumber { field: "node.transform" })
    ));

    let mut invalid_indices = base_scene(source_identity()?)?;
    let stream = invalid_indices.vertex_streams.first_mut().ok_or("missing stream")?;
    stream.indices = vec![0, 1, 9];
    assert!(matches!(
        CanonicalScene::new(invalid_indices),
        Err(SceneIrError::InvalidVertexIndex { .. })
    ));
    Ok(())
}

#[test]
fn invalid_skin_joint_material_slot_and_animation_times_are_typed_failures()
-> Result<(), Box<dyn Error>> {
    let mut invalid_skin = base_scene(source_identity()?)?;
    let mesh = invalid_skin.meshes.first_mut().ok_or("missing mesh")?;
    mesh.skin = Some(SkinId::new(1));
    invalid_skin.skins.push(Skin {
        id: SkinId::new(1),
        joints: vec![JointId::new(9)],
        inverse_bind_matrices: vec![[1.0; 16]],
    });
    assert!(matches!(
        CanonicalScene::new(invalid_skin),
        Err(SceneIrError::DanglingReference { kind: "skin joint", .. })
    ));

    let mut invalid_material = base_scene(source_identity()?)?;
    let primitive = invalid_material.primitives.first_mut().ok_or("missing primitive")?;
    primitive.material = Some(MaterialId::new(99));
    assert!(matches!(
        CanonicalScene::new(invalid_material),
        Err(SceneIrError::DanglingReference { kind: "material", .. })
    ));

    let mut invalid_animation = base_scene(source_identity()?)?;
    invalid_animation.channels.push(Channel {
        id: ChannelId::new(1),
        node: NodeId::new(1),
        property: "translation".to_owned(),
        keys: vec![
            AnimationKey { time_seconds: 1.0, value: vec![0.0, 0.0, 0.0] },
            AnimationKey { time_seconds: 0.0, value: vec![0.0, 0.0, 0.0] },
        ],
    });
    invalid_animation.animation_clips.push(AnimationClip {
        id: AnimationClipId::new(1),
        name: "invalid".to_owned(),
        channels: vec![ChannelId::new(1)],
    });
    assert!(matches!(
        CanonicalScene::new(invalid_animation),
        Err(SceneIrError::InvalidAnimationTimes { .. })
    ));
    Ok(())
}

#[test]
fn equivalent_unreal_and_usd_sources_produce_the_same_render_graph() -> Result<(), Box<dyn Error>> {
    let unreal = base_scene(source_identity()?)?;
    let usd = base_scene(usd_source_identity()?)?;

    assert_eq!(unreal.nodes, usd.nodes);
    assert_eq!(unreal.meshes, usd.meshes);
    assert_eq!(unreal.primitives, usd.primitives);
    assert_eq!(unreal.vertex_streams, usd.vertex_streams);
    assert_eq!(unreal.materials, usd.materials);
    assert_ne!(unreal.source_links, usd.source_links);
    Ok(())
}
