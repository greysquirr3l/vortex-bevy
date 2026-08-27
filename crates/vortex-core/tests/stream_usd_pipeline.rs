use std::{
    error::Error,
    fs,
    io::Write,
    path::{Path, PathBuf},
    process,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use flate2::{Compression, write::GzEncoder};
use vortex_core::{
    adapters::secondary::usd_stage::{UsdStageInspector, UsdStageRequest},
    application::command::{
        resource_limits::{ConversionResourceGovernor, ConversionResourceLimits},
        stream_usd::{
            StreamingUsdStageEncoding, StreamingUsdStageError, StreamingUsdStageLimits,
            StreamingUsdStageQueue, StreamingUsdStageRequest,
        },
    },
    domain::SourceAssetPath,
};

const PRODUCTION_STAGE: &[u8] =
    include_bytes!("../../../fixtures/production_scale/standard_shader_ball/shaderball.usda");

const SMALL_STAGE: &str = r#"#usda 1.0
(
    defaultPrim = "Root"
)
def Xform "Root"
{
    def Mesh "Triangle"
    {
    }
}
"#;

static DIRECTORY_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[test]
fn queued_plain_usda_matches_direct_bounded_resolution() -> Result<(), Box<dyn Error>> {
    let root = TestDirectory::new("plain")?;
    write_file(root.path().join("scene.usda"), SMALL_STAGE.as_bytes())?;
    let request = stage_request(root.path(), "scene.usda")?;
    let direct = UsdStageInspector.resolve(&request)?;
    let queue = queue(4 * 1024 * 1024, 1, 1)?;
    let job = queue.submit(StreamingUsdStageRequest::plain_usda(request))?;
    assert_eq!(job.submission_id(), 1);
    let streamed = job.wait()?;
    assert_eq!(streamed, direct);
    queue.shutdown()?;
    Ok(())
}

#[test]
fn queued_gzip_usda_preserves_bounded_stage_facts() -> Result<(), Box<dyn Error>> {
    let root = TestDirectory::new("gzip")?;
    write_gzip(root.path().join("scene.usda.gz"), SMALL_STAGE.as_bytes())?;
    let queue = queue(4 * 1024 * 1024, 1, 1)?;
    let request = stage_request(root.path(), "scene.usda.gz")?;
    let job = queue.submit(StreamingUsdStageRequest::gzip_usda(request))?;
    let snapshot = job.wait()?;
    assert_eq!(snapshot.prims.len(), 2);
    assert!(
        snapshot.facts.iter().any(|fact| fact == &format!("stage.bytes={}", SMALL_STAGE.len()))
    );
    queue.shutdown()?;
    Ok(())
}

#[test]
fn queued_production_gzip_stage_retains_requires_adapter_boundary() -> Result<(), Box<dyn Error>> {
    let root = TestDirectory::new("production-gzip")?;
    write_gzip(root.path().join("shaderball.usda.gz"), PRODUCTION_STAGE)?;
    let queue = queue(4 * 1024 * 1024, 1, 1)?;
    let request = stage_request(root.path(), "shaderball.usda.gz")?;
    let job = queue.submit(StreamingUsdStageRequest::gzip_usda(request))?;
    let snapshot = job.wait()?;
    assert_eq!(
        snapshot.classification.kind,
        vortex_core::domain::ClassificationKind::RequiresAdapter
    );
    assert!(snapshot.prims.len() > 100);
    assert!(
        snapshot
            .facts
            .iter()
            .any(|fact| fact == &format!("stage.bytes={}", PRODUCTION_STAGE.len()))
    );
    queue.shutdown()?;
    Ok(())
}

#[test]
fn queued_plain_usda_rejects_configured_byte_limit_before_stage_parsing()
-> Result<(), Box<dyn Error>> {
    let root = TestDirectory::new("plain-limit")?;
    let limit = 128_u64;
    let payload = format!("#usda 1.0\n{}", "x".repeat(256));
    write_file(root.path().join("oversized.usda"), payload.as_bytes())?;
    let queue = queue(limit, 1, 1)?;
    let request = stage_request(root.path(), "oversized.usda")?;
    let job = queue.submit(StreamingUsdStageRequest::plain_usda(request))?;
    assert!(matches!(
        job.wait(),
        Err(StreamingUsdStageError::ByteLimitExceeded {
            actual,
            limit: 128
        }) if actual > 128
    ));
    queue.shutdown()?;
    Ok(())
}

#[test]
fn queued_gzip_rejects_expansion_before_stage_parsing() -> Result<(), Box<dyn Error>> {
    let root = TestDirectory::new("expansion")?;
    let limit = 128_u64;
    let payload = format!("#usda 1.0\n{}", "x".repeat(256));
    write_gzip(root.path().join("oversized.usda.gz"), payload.as_bytes())?;
    let queue = queue(limit, 1, 1)?;
    let request = stage_request(root.path(), "oversized.usda.gz")?;
    let job = queue.submit(StreamingUsdStageRequest::gzip_usda(request))?;
    assert!(matches!(
        job.wait(),
        Err(StreamingUsdStageError::ByteLimitExceeded { actual: 129, limit: 128 })
    ));
    queue.shutdown()?;
    Ok(())
}

#[test]
fn queued_gzip_rejects_malformed_stream_and_encoding_mismatch() -> Result<(), Box<dyn Error>> {
    let root = TestDirectory::new("invalid")?;
    write_file(root.path().join("invalid.usda.gz"), SMALL_STAGE.as_bytes())?;
    let queue = queue(4 * 1024 * 1024, 1, 1)?;
    let malformed = stage_request(root.path(), "invalid.usda.gz")?;
    let malformed_job = queue.submit(StreamingUsdStageRequest::gzip_usda(malformed))?;
    assert!(matches!(malformed_job.wait(), Err(StreamingUsdStageError::Decompression { .. })));
    let mismatch = stage_request(root.path(), "invalid.usda.gz")?;
    let mismatch_job = queue.submit(StreamingUsdStageRequest::plain_usda(mismatch))?;
    assert!(matches!(mismatch_job.wait(), Err(StreamingUsdStageError::EncodingMismatch)));
    queue.shutdown()?;
    Ok(())
}

#[test]
fn queue_configuration_and_encoding_are_explicitly_bounded() -> Result<(), Box<dyn Error>> {
    let governor = Arc::new(ConversionResourceGovernor::new(ConversionResourceLimits::new(
        1,
        4 * 1024 * 1024,
    )?));
    let invalid = StreamingUsdStageLimits::new(2, 1, 4 * 1024 * 1024)?;
    assert!(StreamingUsdStageQueue::new(invalid, governor).is_err());
    let root = TestDirectory::new("encoding")?;
    write_file(root.path().join("scene.usda"), SMALL_STAGE.as_bytes())?;
    let request = stage_request(root.path(), "scene.usda")?;
    let streaming = StreamingUsdStageRequest::plain_usda(request);
    assert_eq!(streaming.encoding(), StreamingUsdStageEncoding::PlainUsda);
    Ok(())
}

fn queue(
    bytes: u64,
    workers: usize,
    depth: usize,
) -> Result<StreamingUsdStageQueue, Box<dyn Error>> {
    let limits = StreamingUsdStageLimits::new(workers, depth, bytes)?;
    let governor = Arc::new(ConversionResourceGovernor::new(ConversionResourceLimits::new(
        workers,
        usize::try_from(bytes)?,
    )?));
    Ok(StreamingUsdStageQueue::new(limits, governor)?)
}

fn stage_request(root: &Path, stage: &str) -> Result<UsdStageRequest, Box<dyn Error>> {
    Ok(UsdStageRequest::new(root, stage.parse::<SourceAssetPath>()?, "")?)
}

fn write_file(path: PathBuf, contents: &[u8]) -> Result<(), Box<dyn Error>> {
    fs::write(path, contents)?;
    Ok(())
}

fn write_gzip(path: PathBuf, contents: &[u8]) -> Result<(), Box<dyn Error>> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(contents)?;
    write_file(path, &encoder.finish()?)
}

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new(label: &str) -> Result<Self, Box<dyn Error>> {
        let sequence = DIRECTORY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join(format!("t29-streaming-{label}-{}-{sequence}", process::id()));
        fs::create_dir_all(&directory)?;
        Ok(Self(directory))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            eprintln!("failed to remove T29 test directory: {error}");
        }
    }
}
