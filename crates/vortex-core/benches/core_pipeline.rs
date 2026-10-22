//! Owned-fixture benchmarks for the bounded core conversion pipeline.

use std::{
    alloc::System,
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fs,
    io::Write,
    path::PathBuf,
    process,
    sync::Arc,
};

use criterion::{Criterion, black_box};
use flate2::{Compression, write::GzEncoder};
use stats_alloc::{INSTRUMENTED_SYSTEM, Region, Stats, StatsAlloc};
use vortex_core::{
    adapters::secondary::{
        glb::GlbWriter,
        texture::{TextureInput, TextureNormalizer, TextureRole, TextureSamplerInput},
        unreal_texture2d_metadata::UnrealTexture2DMetadataDecoder,
        usd_stage::{UsdStageInspector, UsdStageRequest},
    },
    application::command::{
        resource_limits::{ConversionResourceGovernor, ConversionResourceLimits},
        stream_usd::{
            StreamingUsdStageJob, StreamingUsdStageLimits, StreamingUsdStageQueue,
            StreamingUsdStageRequest,
        },
    },
    domain::{
        ArtifactHash, CompatibilityCapability, CompatibilityRequirements, ManifestArtifact,
        ManifestArtifactKind, ManifestSource, ProjectRoot, Remediation, SceneManifest,
        SourceAssetPath, SourceIdentity, UnrealObjectPath,
        scene_ir::{
            AlphaMode, CanonicalScene, CoordinateNormalization, Material, MaterialId, Mesh, MeshId,
            Node, NodeId, Primitive, PrimitiveId, SourceLink, SourceLinkId, TextureId,
            TextureTransform, Transform, VertexStream, VertexStreamId,
        },
    },
};

const STAGE_FIXTURE: &str = include_str!("../../../fixtures/minimal_usda/triangle_scene.usda");
const PRODUCTION_TEXTURE: &[u8] = include_bytes!(
    "../../../fixtures/production_scale/polyhaven_wooden_planks/wooden_planks_diff_1k.png"
);
const PRODUCTION_STAGE: &[u8] =
    include_bytes!("../../../fixtures/production_scale/standard_shader_ball/shaderball.usda");

#[global_allocator]
static GLOBAL: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

fn main() {
    if let Err(error) = run() {
        eprintln!("T28 core benchmark setup failed: {error}");
        process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let fixtures = CoreFixtures::new()?;
    record_memory_observation("usd_stage/resolve_owned_triangle", || {
        fixtures.stage_inspector.resolve(&fixtures.stage_request)
    })?;
    record_memory_observation("usd_stage/resolve_standard_shader_ball", || {
        fixtures.stage_inspector.resolve(&fixtures.production_stage_request)
    })?;
    record_memory_observation("texture/normalize_polyhaven_wooden_planks_1k", || {
        TextureNormalizer.normalize([fixtures.production_texture_input.clone()])
    })?;
    measure_core_pipeline(&fixtures);
    fixtures.shutdown()?;
    Ok(())
}

fn record_memory_observation<ResultValue, OperationError, Operation>(
    operation_name: &str,
    operation: Operation,
) -> Result<(), Box<dyn Error>>
where
    OperationError: Error + 'static,
    Operation: FnOnce() -> Result<ResultValue, OperationError>,
{
    use vortex_core::adapters::secondary::linux_process_memory::LinuxProcessMemoryReader;

    let reader = LinuxProcessMemoryReader;
    let before = reader.snapshot()?;
    let region = Region::new(GLOBAL);
    let result = operation()?;
    let during_operation = reader.snapshot()?;
    let allocation = region.change();
    drop(result);
    let after_release = reader.snapshot()?;
    print_memory_observation(
        operation_name,
        before.resident_bytes(),
        during_operation.resident_bytes(),
        after_release.resident_bytes(),
        during_operation.peak_resident_bytes(),
        allocation,
    );
    Ok(())
}

fn print_memory_observation(
    operation_name: &str,
    rss_before: u64,
    rss_during: u64,
    rss_after_release: u64,
    peak_rss: u64,
    allocation: Stats,
) {
    eprintln!(
        "T28 memory observation: operation={operation_name} rss_before_bytes={rss_before} rss_during_bytes={rss_during} rss_after_release_bytes={rss_after_release} rss_during_delta_bytes={} rss_after_release_delta_bytes={} peak_rss_bytes={peak_rss} allocations={} deallocations={} reallocations={} bytes_allocated={} bytes_deallocated={} bytes_reallocated={}",
        i128::from(rss_during) - i128::from(rss_before),
        i128::from(rss_after_release) - i128::from(rss_before),
        allocation.allocations,
        allocation.deallocations,
        allocation.reallocations,
        allocation.bytes_allocated,
        allocation.bytes_deallocated,
        allocation.bytes_reallocated,
    );
}

fn measure_core_pipeline(fixtures: &CoreFixtures) {
    let mut criterion = Criterion::default();
    criterion.bench_function("unreal_texture2d_metadata/decode", |bencher| {
        bencher.iter(|| {
            black_box(fixtures.texture_decoder.decode(
                black_box(&fixtures.source),
                black_box(&fixtures.texture_package),
                None,
            ))
        });
    });
    criterion.bench_function("usd_stage/resolve_owned_triangle", |bencher| {
        bencher.iter(|| {
            black_box(fixtures.stage_inspector.resolve(black_box(&fixtures.stage_request)))
        });
    });
    criterion.bench_function("usd_stage/resolve_standard_shader_ball", |bencher| {
        bencher.iter(|| {
            black_box(
                fixtures.stage_inspector.resolve(black_box(&fixtures.production_stage_request)),
            )
        });
    });
    criterion.bench_function("usd_stage/queue_standard_shader_ball_plain", |bencher| {
        bencher.iter(|| {
            let submitted = fixtures
                .streaming_stage_queue
                .submit(black_box(fixtures.production_streaming_stage_request.clone()));
            black_box(submitted.and_then(StreamingUsdStageJob::wait))
        });
    });
    criterion.bench_function("usd_stage/queue_standard_shader_ball_gzip", |bencher| {
        bencher.iter(|| {
            let submitted = fixtures
                .streaming_stage_queue
                .submit(black_box(fixtures.production_gzip_stage_request.clone()));
            black_box(submitted.and_then(StreamingUsdStageJob::wait))
        });
    });
    criterion.bench_function("canonical_ir/validate_triangle", |bencher| {
        bencher.iter(|| black_box(CanonicalScene::new(black_box(fixtures.scene_template.clone()))));
    });
    criterion.bench_function("texture/normalize_owned_png", |bencher| {
        bencher.iter(|| black_box(TextureNormalizer.normalize([fixtures.texture_input.clone()])));
    });
    criterion.bench_function("texture/normalize_polyhaven_wooden_planks_1k", |bencher| {
        bencher.iter(|| {
            black_box(TextureNormalizer.normalize([fixtures.production_texture_input.clone()]))
        });
    });
    criterion.bench_function("glb/package_triangle", |bencher| {
        bencher.iter(|| black_box(GlbWriter.package(black_box(&fixtures.scene))));
    });
    criterion.bench_function("manifest/parse_owned_fixture", |bencher| {
        bencher.iter(|| {
            black_box(SceneManifest::from_ron(
                black_box(fixtures.manifest_ron.as_bytes()),
                1024 * 1024,
            ))
        });
    });
    criterion.final_summary();
}

struct CoreFixtures {
    source: SourceIdentity,
    texture_decoder: UnrealTexture2DMetadataDecoder,
    texture_package: Vec<u8>,
    stage_inspector: UsdStageInspector,
    stage_request: UsdStageRequest,
    production_stage_request: UsdStageRequest,
    production_streaming_stage_request: StreamingUsdStageRequest,
    production_gzip_stage_request: StreamingUsdStageRequest,
    streaming_stage_queue: StreamingUsdStageQueue,
    production_texture_input: TextureInput,
    scene_template: CanonicalScene,
    scene: CanonicalScene,
    texture_input: TextureInput,
    manifest_ron: String,
}

impl CoreFixtures {
    fn new() -> Result<Self, Box<dyn Error>> {
        let source = source_identity()?;
        let scene_template = triangle_scene(source.clone())?;
        let scene = scene_template.clone();
        let root = owned_stage_root()?;
        let stage_request = UsdStageRequest::new(&root, "triangle_scene.usda".parse()?, "")?;
        let production_stage_request = UsdStageRequest::new(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../fixtures/production_scale/standard_shader_ball"),
            "shaderball.usda".parse()?,
            "",
        )?;
        let stream_limit_bytes = 4_u64 * 1024 * 1024;
        let streaming_governor = Arc::new(ConversionResourceGovernor::new(
            ConversionResourceLimits::new(1, usize::try_from(stream_limit_bytes)?)?,
        ));
        let streaming_stage_queue = StreamingUsdStageQueue::new(
            StreamingUsdStageLimits::new(1, 1, stream_limit_bytes)?,
            streaming_governor,
        )?;
        let production_streaming_stage_request =
            StreamingUsdStageRequest::plain_usda(production_stage_request.clone());
        let production_gzip_stage_request =
            StreamingUsdStageRequest::gzip_usda(production_gzip_stage_request()?);
        let texture_input = TextureInput {
            id: TextureId::new(1),
            source: "Textures/owned.png".parse::<SourceAssetPath>()?,
            role: TextureRole::BaseColor,
            sampler: TextureSamplerInput::linear_repeat(),
            bytes: owned_png(2, 2)?,
        };
        let production_texture_input = TextureInput {
            id: TextureId::new(92),
            source: "Textures/PolyHaven/wooden_planks_diff_1k.png".parse::<SourceAssetPath>()?,
            role: TextureRole::BaseColor,
            sampler: TextureSamplerInput::linear_repeat(),
            bytes: PRODUCTION_TEXTURE.to_vec(),
        };
        let manifest_ron = owned_manifest(&source)?.to_ron()?;
        Ok(Self {
            texture_package: texture_package()?,
            texture_decoder: UnrealTexture2DMetadataDecoder,
            stage_inspector: UsdStageInspector,
            source,
            stage_request,
            production_stage_request,
            production_streaming_stage_request,
            production_gzip_stage_request,
            streaming_stage_queue,
            production_texture_input,
            scene_template,
            scene,
            texture_input,
            manifest_ron,
        })
    }

    fn shutdown(self) -> Result<(), Box<dyn Error>> {
        self.streaming_stage_queue.shutdown()?;
        Ok(())
    }
}

fn source_identity() -> Result<SourceIdentity, Box<dyn Error>> {
    Ok(SourceIdentity::Unreal {
        project_root: "OwnedBenchProject".parse::<ProjectRoot>()?,
        asset_path: "Content/Textures/T_Bench.uasset".parse::<SourceAssetPath>()?,
        object_path: "/Game/Textures/T_Bench.T_Bench".parse::<UnrealObjectPath>()?,
    })
}

fn owned_stage_root() -> Result<PathBuf, Box<dyn Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/t27-owned-stage-fixture");
    fs::create_dir_all(&root)?;
    fs::write(root.join("triangle_scene.usda"), STAGE_FIXTURE)?;
    Ok(root)
}

fn production_gzip_stage_request() -> Result<UsdStageRequest, Box<dyn Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/t29-production-gzip-stage");
    fs::create_dir_all(&root)?;
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(PRODUCTION_STAGE)?;
    fs::write(root.join("shaderball.usda.gz"), encoder.finish()?)?;
    Ok(UsdStageRequest::new(&root, "shaderball.usda.gz".parse()?, "")?)
}

fn triangle_scene(source: SourceIdentity) -> Result<CanonicalScene, Box<dyn Error>> {
    CanonicalScene::new(CanonicalScene {
        normalization: CoordinateNormalization {
            meters_per_unit: "1.000000000000".to_owned(),
            source_up_axis: "Y".to_owned(),
            target_up_axis: "Y".to_owned(),
            winding: "counter_clockwise".to_owned(),
            uv_origin: "bottom_left".to_owned(),
            color_space: "linear_srgb".to_owned(),
        },
        source_links: vec![SourceLink { id: SourceLinkId::new(1), source, facts: BTreeMap::new() }],
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
        materials: vec![Material {
            id: MaterialId::new(1),
            family: "pbr_metallic_roughness".to_owned(),
            base_color: [1.0; 4],
            metallic: 0.0,
            roughness: 1.0,
            normal_scale: 1.0,
            occlusion_strength: 1.0,
            emissive: [0.0; 3],
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
    })
    .map_err(Into::into)
}

fn owned_manifest(source: &SourceIdentity) -> Result<SceneManifest, Box<dyn Error>> {
    SceneManifest::new(
        "0.1.0",
        ArtifactHash::from_bytes(b"t27-owned-profile"),
        vec![ManifestArtifact {
            kind: ManifestArtifactKind::Glb,
            path: "scene.glb".to_owned(),
            hash: ArtifactHash::from_bytes(b"t27-owned-glb"),
        }],
        vec![ManifestSource {
            identity: source.clone(),
            asset_kind: "static_mesh".to_owned(),
            selected_values: BTreeMap::new(),
            glb_labels: BTreeMap::new(),
            classification: vortex_core::domain::ConversionClassification::new(
                vortex_core::domain::ClassificationKind::Converted,
                "owned benchmark fixture",
                Remediation::new("Use the owned fixture.")?,
            )?,
            diagnostics: Vec::new(),
        }],
        CompatibilityRequirements {
            minimum_bevy_version: "0.20".to_owned(),
            required_capabilities: BTreeSet::from([CompatibilityCapability::BevyGltfPbr]),
        },
    )
    .map_err(Into::into)
}

fn owned_png(width: u32, height: u32) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut png = vec![137, 80, 78, 71, 13, 10, 26, 10];
    let mut header = Vec::with_capacity(13);
    header.extend_from_slice(&width.to_be_bytes());
    header.extend_from_slice(&height.to_be_bytes());
    header.extend_from_slice(&[8, 6, 0, 0, 0]);
    append_png_chunk(&mut png, *b"IHDR", &header)?;
    append_png_chunk(&mut png, *b"IDAT", &[0])?;
    append_png_chunk(&mut png, *b"IEND", &[])?;
    Ok(png)
}

fn append_png_chunk(
    png: &mut Vec<u8>,
    kind: [u8; 4],
    payload: &[u8],
) -> Result<(), Box<dyn Error>> {
    let length = u32::try_from(payload.len())?;
    png.extend_from_slice(&length.to_be_bytes());
    png.extend_from_slice(&kind);
    png.extend_from_slice(payload);
    png.extend_from_slice(&[0_u8; 4]);
    Ok(())
}

fn texture_package() -> Result<Vec<u8>, Box<dyn Error>> {
    let mut bytes = Vec::new();
    push_u32(&mut bytes, 0x9e2a_83c1);
    push_i32(&mut bytes, -9);
    push_i32(&mut bytes, 0);
    push_i32(&mut bytes, 522);
    push_i32(&mut bytes, 1018);
    push_i32(&mut bytes, 0);
    bytes.extend_from_slice(&[0_u8; 20]);
    let header_size_position = bytes.len();
    push_i32(&mut bytes, 0);
    push_i32(&mut bytes, 0);
    push_string(&mut bytes, "/Game/Textures/T_Bench")?;
    push_u32(&mut bytes, 0);
    bytes.extend_from_slice(&[0_u8; 16]);
    push_string(&mut bytes, "")?;
    bytes.extend_from_slice(&[0_u8; 76]);
    push_i32(&mut bytes, 0);
    push_engine_version(&mut bytes)?;
    push_engine_version(&mut bytes)?;
    bytes.extend_from_slice(&[0_u8; 4]);
    push_i32(&mut bytes, 0);
    bytes.extend_from_slice(&[0_u8; 4]);
    push_i32(&mut bytes, 0);
    let registry_offset_position = bytes.len();
    push_i32(&mut bytes, 0);
    let bulk_offset_position = bytes.len();
    push_i64(&mut bytes, 0);
    bytes.extend_from_slice(&[0_u8; 4]);
    push_i32(&mut bytes, 0);
    bytes.extend_from_slice(&[0_u8; 24]);
    let registry_offset = i32::try_from(bytes.len())?;
    push_i32(&mut bytes, 1);
    push_string(&mut bytes, "/Game/Textures/T_Bench.T_Bench")?;
    push_string(&mut bytes, "Texture2D")?;
    let tags = [
        ("Dimensions", "256x128"),
        ("PixelFormat", "PF_B8G8R8A8"),
        ("CompressionSettings", "TC_Default"),
    ];
    push_i32(&mut bytes, i32::try_from(tags.len())?);
    for (key, value) in tags {
        push_string(&mut bytes, key)?;
        push_string(&mut bytes, value)?;
    }
    overwrite_i32(&mut bytes, header_size_position, registry_offset)?;
    overwrite_i32(&mut bytes, registry_offset_position, registry_offset)?;
    let bulk_offset = i64::try_from(bytes.len())?;
    overwrite_i64(&mut bytes, bulk_offset_position, bulk_offset)?;
    Ok(bytes)
}

fn push_engine_version(bytes: &mut Vec<u8>) -> Result<(), Box<dyn Error>> {
    for value in [5_u16, 8, 2] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    push_u32(bytes, 0);
    push_string(bytes, "++UE5+Release-5.8")
}

fn push_string(bytes: &mut Vec<u8>, value: &str) -> Result<(), Box<dyn Error>> {
    push_i32(bytes, i32::try_from(value.len().saturating_add(1))?);
    bytes.extend_from_slice(value.as_bytes());
    bytes.push(0);
    Ok(())
}

fn push_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}
fn push_i32(bytes: &mut Vec<u8>, value: i32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}
fn push_i64(bytes: &mut Vec<u8>, value: i64) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn overwrite_i32(bytes: &mut [u8], offset: usize, value: i32) -> Result<(), Box<dyn Error>> {
    let end = offset.checked_add(4).ok_or("i32 offset overflow")?;
    let destination = bytes.get_mut(offset..end).ok_or("i32 offset outside fixture")?;
    destination.copy_from_slice(&value.to_le_bytes());
    Ok(())
}

fn overwrite_i64(bytes: &mut [u8], offset: usize, value: i64) -> Result<(), Box<dyn Error>> {
    let end = offset.checked_add(8).ok_or("i64 offset overflow")?;
    let destination = bytes.get_mut(offset..end).ok_or("i64 offset outside fixture")?;
    destination.copy_from_slice(&value.to_le_bytes());
    Ok(())
}
