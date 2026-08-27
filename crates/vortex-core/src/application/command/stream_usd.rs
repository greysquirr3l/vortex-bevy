//! Resource-governed asynchronous selected-USDA inspection.
//!
//! This in-process queue accepts only root-confined local `.usda` or `.usda.gz`
//! files. It never fetches a layer, resolves arbitrary `OpenUSD` composition, or
//! publishes conversion artifacts. A caller owns the queue lifecycle and receives
//! one typed result for every accepted submission.

use std::{
    fs::{self, File},
    io::{BufReader, Read},
    num::{NonZeroU64, NonZeroUsize},
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
        mpsc::{self, Receiver, SyncSender, TrySendError},
    },
    thread::{self, JoinHandle},
};

use flate2::read::GzDecoder;
use thiserror::Error;

use crate::{
    adapters::secondary::usd_stage::{
        UsdStageError, UsdStageInspector, UsdStageRequest, UsdStageSnapshot,
    },
    application::command::resource_limits::{ConversionResourceError, ConversionResourceGovernor},
};

const MAX_STREAMED_USDA_BYTES: u64 = 4 * 1024 * 1024;

/// Explicit on-disk encoding accepted by the bounded streaming adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StreamingUsdStageEncoding {
    /// A local, root-confined uncompressed `.usda` source file.
    PlainUsda,
    /// A local, root-confined `.usda.gz` source file decompressed within a fixed bound.
    GzipUsda,
}

impl StreamingUsdStageEncoding {
    const fn label(self) -> &'static str {
        match self {
            Self::PlainUsda => "plain_usda",
            Self::GzipUsda => "gzip_usda",
        }
    }
}

/// Immutable bounds for one in-process streaming USDA queue.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StreamingUsdStageLimits {
    worker_count: NonZeroUsize,
    queue_depth: NonZeroUsize,
    max_decompressed_bytes: NonZeroU64,
}

impl StreamingUsdStageLimits {
    /// Creates bounded queue limits.
    ///
    /// # Errors
    ///
    /// Returns a typed error when a count is zero or the decompressed-byte limit is
    /// zero, exceeds the bounded USDA adapter limit, or cannot be represented for an
    /// in-memory buffer on the current target.
    pub fn new(
        worker_count: usize,
        queue_depth: usize,
        max_decompressed_bytes: u64,
    ) -> Result<Self, StreamingUsdStageConfigError> {
        let worker_count = NonZeroUsize::new(worker_count)
            .ok_or(StreamingUsdStageConfigError::ZeroLimit { field: "worker_count" })?;
        let queue_depth = NonZeroUsize::new(queue_depth)
            .ok_or(StreamingUsdStageConfigError::ZeroLimit { field: "queue_depth" })?;
        let max_decompressed_bytes = NonZeroU64::new(max_decompressed_bytes)
            .ok_or(StreamingUsdStageConfigError::ZeroLimit { field: "max_decompressed_bytes" })?;
        if max_decompressed_bytes.get() > MAX_STREAMED_USDA_BYTES {
            return Err(StreamingUsdStageConfigError::DecompressedLimitTooLarge {
                requested: max_decompressed_bytes.get(),
                limit: MAX_STREAMED_USDA_BYTES,
            });
        }
        if usize::try_from(max_decompressed_bytes.get()).is_err() {
            return Err(StreamingUsdStageConfigError::DecompressedLimitUnrepresentable {
                requested: max_decompressed_bytes.get(),
            });
        }
        Ok(Self { worker_count, queue_depth, max_decompressed_bytes })
    }

    /// Returns the number of worker threads owned by the queue.
    #[must_use]
    pub const fn worker_count(self) -> NonZeroUsize {
        self.worker_count
    }

    /// Returns the number of submissions allowed to wait in the channel.
    #[must_use]
    pub const fn queue_depth(self) -> NonZeroUsize {
        self.queue_depth
    }

    /// Returns the maximum accepted compressed and decompressed byte count.
    #[must_use]
    pub const fn max_decompressed_bytes(self) -> NonZeroU64 {
        self.max_decompressed_bytes
    }
}

/// Invalid queue configuration errors.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum StreamingUsdStageConfigError {
    /// One required positive limit was zero.
    #[error("{field} must be greater than zero")]
    ZeroLimit {
        /// The invalid configuration field.
        field: &'static str,
    },
    /// The requested decompressed bound would widen the existing USDA adapter limit.
    #[error("streaming USDA decompressed limit of {requested} bytes exceeds {limit} bytes")]
    DecompressedLimitTooLarge {
        /// The requested output limit.
        requested: u64,
        /// The fixed adapter limit.
        limit: u64,
    },
    /// The configured decompressed limit cannot initialize a bounded in-memory buffer.
    #[error("streaming USDA decompressed limit of {requested} bytes is unsupported on this target")]
    DecompressedLimitUnrepresentable {
        /// The unsupported byte count.
        requested: u64,
    },
    /// More worker threads were requested than the supplied governor can admit.
    #[error("streaming worker count of {workers} exceeds governor in-flight limit of {limit}")]
    WorkerCountExceedsGovernor {
        /// The requested worker count.
        workers: usize,
        /// The governor's in-flight limit.
        limit: usize,
    },
}

/// A selected USDA request plus its explicit input encoding.
#[derive(Clone, Debug)]
pub struct StreamingUsdStageRequest {
    stage: UsdStageRequest,
    encoding: StreamingUsdStageEncoding,
}

impl StreamingUsdStageRequest {
    /// Wraps one validated selected-stage request for uncompressed streaming resolution.
    #[must_use]
    pub const fn plain_usda(stage: UsdStageRequest) -> Self {
        Self { stage, encoding: StreamingUsdStageEncoding::PlainUsda }
    }

    /// Wraps one validated selected-stage request for bounded local gzip decompression.
    #[must_use]
    pub const fn gzip_usda(stage: UsdStageRequest) -> Self {
        Self { stage, encoding: StreamingUsdStageEncoding::GzipUsda }
    }

    /// Returns the explicit encoding selected at the input boundary.
    #[must_use]
    pub const fn encoding(&self) -> StreamingUsdStageEncoding {
        self.encoding
    }
}

/// Typed failures from streaming submission, worker execution, or shutdown.
#[derive(Debug, Error)]
pub enum StreamingUsdStageError {
    /// A job cannot reserve required bounded conversion capacity before any input is read.
    #[error("streaming USDA resource admission failed: {source}")]
    Resource {
        /// The existing governor error.
        source: ConversionResourceError,
    },
    /// All bounded queued-submission slots are occupied.
    #[error("streaming USDA submission queue is full")]
    QueueFull,
    /// The queue no longer has workers able to accept a submission.
    #[error("streaming USDA worker queue is unavailable")]
    WorkerUnavailable,
    /// The submission sequence cannot safely produce another local identifier.
    #[error("streaming USDA submission identifier space is exhausted")]
    SubmissionIdExhausted,
    /// A worker ended before sending the accepted job's result.
    #[error("streaming USDA worker ended before returning a result")]
    WorkerResultUnavailable,
    /// A worker panicked while owned queue shutdown joined it.
    #[error("streaming USDA worker did not shut down cleanly")]
    WorkerShutdownFailed,
    /// The selected compressed file is unavailable after root confinement.
    #[error("streaming USDA input is unavailable: {source}")]
    InputUnavailable {
        /// The underlying operating-system error.
        source: std::io::Error,
    },
    /// The selected local file does not match the requested bounded encoding.
    #[error("streaming USDA input encoding does not match the selected file")]
    EncodingMismatch,
    /// The compressed stream is malformed or cannot be read safely.
    #[error("streaming USDA decompression failed: {source}")]
    Decompression {
        /// The underlying bounded gzip-reader error.
        source: std::io::Error,
    },
    /// Compressed or decompressed bytes exceed the configured safe bound.
    #[error("streaming USDA bytes exceed the {limit}-byte bound: {actual}")]
    ByteLimitExceeded {
        /// The observed byte count.
        actual: u64,
        /// The configured byte limit.
        limit: u64,
    },
    /// Decompressed bytes are not valid UTF-8 USDA text.
    #[error("streaming USDA decompressed bytes are not valid UTF-8: {source}")]
    InvalidUtf8 {
        /// The UTF-8 conversion error.
        source: std::string::FromUtf8Error,
    },
    /// Canonical bounded selected-stage parsing failed.
    #[error("streaming USDA stage resolution failed: {source}")]
    Stage {
        /// The wrapped typed stage error.
        source: UsdStageError,
    },
}

/// An accepted asynchronous streaming USDA submission.
#[derive(Debug)]
pub struct StreamingUsdStageJob {
    submission_id: u64,
    receiver: Receiver<Result<UsdStageSnapshot, StreamingUsdStageError>>,
}

impl StreamingUsdStageJob {
    /// Returns the queue-local monotonic submission identifier.
    #[must_use]
    pub const fn submission_id(&self) -> u64 {
        self.submission_id
    }

    /// Waits for the worker's single typed result.
    ///
    /// # Errors
    ///
    /// Returns worker, decompression, resource-admission, or canonical stage errors.
    pub fn wait(self) -> Result<UsdStageSnapshot, StreamingUsdStageError> {
        self.receiver.recv().map_err(|_| StreamingUsdStageError::WorkerResultUnavailable)?
    }
}

/// Explicitly owned in-process workers for bounded selected-USDA inspection.
#[derive(Debug)]
pub struct StreamingUsdStageQueue {
    sender: SyncSender<QueuedStage>,
    workers: Vec<JoinHandle<()>>,
    governor: Arc<ConversionResourceGovernor>,
    sequence: AtomicU64,
}

impl StreamingUsdStageQueue {
    /// Starts the configured bounded streaming USDA workers.
    ///
    /// # Errors
    ///
    /// Returns a typed configuration error when workers exceed the supplied resource
    /// governor's in-flight bound.
    pub fn new(
        limits: StreamingUsdStageLimits,
        governor: Arc<ConversionResourceGovernor>,
    ) -> Result<Self, StreamingUsdStageConfigError> {
        let governor_limit = governor.limits().max_in_flight().get();
        if limits.worker_count().get() > governor_limit {
            return Err(StreamingUsdStageConfigError::WorkerCountExceedsGovernor {
                workers: limits.worker_count().get(),
                limit: governor_limit,
            });
        }
        let (sender, receiver) = mpsc::sync_channel(limits.queue_depth().get());
        let receiver = Arc::new(Mutex::new(receiver));
        let workers = spawn_workers(&receiver, &governor, limits);
        Ok(Self { sender, workers, governor, sequence: AtomicU64::new(1) })
    }

    /// Attempts to submit one selected local USDA request without blocking.
    ///
    /// # Errors
    ///
    /// Returns a typed queue-full or worker-unavailable error before a new request is
    /// queued. The worker later returns resource, decompression, or parser errors.
    pub fn submit(
        &self,
        request: StreamingUsdStageRequest,
    ) -> Result<StreamingUsdStageJob, StreamingUsdStageError> {
        let submission_id = self.next_submission_id()?;
        let encoding = request.encoding().label();
        let (reply, receiver) = mpsc::channel();
        let queued = QueuedStage { submission_id, request, reply };
        match self.sender.try_send(queued) {
            Ok(()) => {}
            Err(error) => return Err(map_submission_error(&error)),
        }
        tracing::info!(submission_id, encoding, "streaming USDA stage queued");
        Ok(StreamingUsdStageJob { submission_id, receiver })
    }

    /// Closes submission, drains accepted work, and joins every owned worker.
    ///
    /// # Errors
    ///
    /// Returns an error only when a worker panicked before it could be joined.
    pub fn shutdown(self) -> Result<(), StreamingUsdStageError> {
        let Self { sender, workers, governor, sequence: _ } = self;
        drop(sender);
        join_workers(workers)?;
        drop(governor);
        Ok(())
    }

    fn next_submission_id(&self) -> Result<u64, StreamingUsdStageError> {
        self.sequence
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| current.checked_add(1))
            .map_err(|_| StreamingUsdStageError::SubmissionIdExhausted)
    }
}

#[derive(Debug)]
struct QueuedStage {
    submission_id: u64,
    request: StreamingUsdStageRequest,
    reply: mpsc::Sender<Result<UsdStageSnapshot, StreamingUsdStageError>>,
}

impl QueuedStage {
    fn resolve(self, governor: &ConversionResourceGovernor, limits: StreamingUsdStageLimits) {
        let Self { submission_id, request, reply } = self;
        let result = resolve_queued_stage(&request, governor, limits);
        tracing::info!(submission_id, succeeded = result.is_ok(), "streaming USDA stage completed");
        drop(reply.send(result));
    }
}

fn spawn_workers(
    receiver: &Arc<Mutex<Receiver<QueuedStage>>>,
    governor: &Arc<ConversionResourceGovernor>,
    limits: StreamingUsdStageLimits,
) -> Vec<JoinHandle<()>> {
    (0..limits.worker_count().get())
        .map(|_| {
            let receiver = Arc::clone(receiver);
            let governor = Arc::clone(governor);
            thread::spawn(move || worker_loop(&receiver, &governor, limits))
        })
        .collect()
}

fn worker_loop(
    receiver: &Mutex<Receiver<QueuedStage>>,
    governor: &ConversionResourceGovernor,
    limits: StreamingUsdStageLimits,
) {
    loop {
        match receive_next(receiver) {
            Ok(queued) => queued.resolve(governor, limits),
            Err(()) => return,
        }
    }
}

fn receive_next(receiver: &Mutex<Receiver<QueuedStage>>) -> Result<QueuedStage, ()> {
    let message = match receiver.lock() {
        Ok(receiver) => receiver.recv(),
        Err(_) => return Err(()),
    };
    message.map_err(|_| ())
}

fn join_workers(workers: Vec<JoinHandle<()>>) -> Result<(), StreamingUsdStageError> {
    for worker in workers {
        worker.join().map_err(|_| StreamingUsdStageError::WorkerShutdownFailed)?;
    }
    Ok(())
}

const fn map_submission_error(error: &TrySendError<QueuedStage>) -> StreamingUsdStageError {
    match error {
        TrySendError::Full(_) => StreamingUsdStageError::QueueFull,
        TrySendError::Disconnected(_) => StreamingUsdStageError::WorkerUnavailable,
    }
}

fn resolve_queued_stage(
    request: &StreamingUsdStageRequest,
    governor: &ConversionResourceGovernor,
    limits: StreamingUsdStageLimits,
) -> Result<UsdStageSnapshot, StreamingUsdStageError> {
    let reservation = governor
        .acquire(reservation_bytes(limits)?)
        .map_err(|source| StreamingUsdStageError::Resource { source })?;
    let result = match request.encoding {
        StreamingUsdStageEncoding::PlainUsda => resolve_plain_usda(&request.stage, limits),
        StreamingUsdStageEncoding::GzipUsda => resolve_gzip_usda(&request.stage, limits),
    };
    drop(reservation);
    result
}

fn reservation_bytes(limits: StreamingUsdStageLimits) -> Result<usize, StreamingUsdStageError> {
    let maximum = limits.max_decompressed_bytes().get();
    usize::try_from(maximum).map_err(|_| StreamingUsdStageError::ByteLimitExceeded {
        actual: maximum,
        limit: MAX_STREAMED_USDA_BYTES,
    })
}

fn resolve_plain_usda(
    request: &UsdStageRequest,
    limits: StreamingUsdStageLimits,
) -> Result<UsdStageSnapshot, StreamingUsdStageError> {
    let inspector = UsdStageInspector;
    let stage_file = UsdStageInspector::resolve_stage_file(request)
        .map_err(|source| StreamingUsdStageError::Stage { source })?;
    if !has_extension(&stage_file, "usda") {
        return Err(StreamingUsdStageError::EncodingMismatch);
    }
    let maximum = limits.max_decompressed_bytes().get();
    let actual = fs::metadata(&stage_file)
        .map_err(|source| StreamingUsdStageError::InputUnavailable { source })?
        .len();
    if actual > maximum {
        return Err(StreamingUsdStageError::ByteLimitExceeded { actual, limit: maximum });
    }
    inspector.resolve(request).map_err(|source| StreamingUsdStageError::Stage { source })
}

fn resolve_gzip_usda(
    request: &UsdStageRequest,
    limits: StreamingUsdStageLimits,
) -> Result<UsdStageSnapshot, StreamingUsdStageError> {
    let stage_file = UsdStageInspector::resolve_stage_file(request)
        .map_err(|source| StreamingUsdStageError::Stage { source })?;
    if !is_gzip_usda_path(&stage_file) {
        return Err(StreamingUsdStageError::EncodingMismatch);
    }
    let maximum = limits.max_decompressed_bytes().get();
    let compressed_bytes = fs::metadata(&stage_file)
        .map_err(|source| StreamingUsdStageError::InputUnavailable { source })?
        .len();
    if compressed_bytes > maximum {
        return Err(StreamingUsdStageError::ByteLimitExceeded {
            actual: compressed_bytes,
            limit: maximum,
        });
    }
    let text = read_bounded_gzip_text(&stage_file, maximum)?;
    let text_bytes = u64::try_from(text.len()).map_err(|_| {
        StreamingUsdStageError::ByteLimitExceeded { actual: u64::MAX, limit: maximum }
    })?;
    UsdStageInspector::resolve_usda_text(request, text_bytes, &text)
        .map_err(|source| StreamingUsdStageError::Stage { source })
}

fn has_extension(path: &Path, expected: &str) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case(expected))
}

fn is_gzip_usda_path(path: &Path) -> bool {
    if !has_extension(path, "gz") {
        return false;
    }
    path.file_stem()
        .and_then(|value| Path::new(value).extension())
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("usda"))
}

fn read_bounded_gzip_text(
    path: &Path,
    maximum_bytes: u64,
) -> Result<String, StreamingUsdStageError> {
    let file =
        File::open(path).map_err(|source| StreamingUsdStageError::InputUnavailable { source })?;
    let decoder = GzDecoder::new(BufReader::new(file));
    let mut bounded = decoder.take(maximum_bytes.saturating_add(1));
    let capacity =
        usize::try_from(maximum_bytes).map_err(|_| StreamingUsdStageError::ByteLimitExceeded {
            actual: maximum_bytes,
            limit: MAX_STREAMED_USDA_BYTES,
        })?;
    let mut bytes = Vec::with_capacity(capacity);
    bounded
        .read_to_end(&mut bytes)
        .map_err(|source| StreamingUsdStageError::Decompression { source })?;
    let actual_bytes = u64::try_from(bytes.len()).map_err(|_| {
        StreamingUsdStageError::ByteLimitExceeded { actual: u64::MAX, limit: maximum_bytes }
    })?;
    if actual_bytes > maximum_bytes {
        return Err(StreamingUsdStageError::ByteLimitExceeded {
            actual: actual_bytes,
            limit: maximum_bytes,
        });
    }
    String::from_utf8(bytes).map_err(|source| StreamingUsdStageError::InvalidUtf8 { source })
}

#[cfg(test)]
mod tests {
    use std::{error::Error, sync::mpsc};

    use crate::{
        adapters::secondary::usd_stage::UsdStageRequest,
        application::command::{
            resource_limits::{ConversionResourceGovernor, ConversionResourceLimits},
            stream_usd::{
                QueuedStage, StreamingUsdStageConfigError, StreamingUsdStageLimits,
                StreamingUsdStageQueue, StreamingUsdStageRequest,
            },
        },
        domain::SourceAssetPath,
    };

    #[test]
    fn queue_limits_reject_zero_and_excessive_values() {
        assert!(matches!(
            StreamingUsdStageLimits::new(0, 1, 1),
            Err(StreamingUsdStageConfigError::ZeroLimit { field: "worker_count" })
        ));
        assert!(matches!(
            StreamingUsdStageLimits::new(1, 1, 4 * 1024 * 1024 + 1),
            Err(StreamingUsdStageConfigError::DecompressedLimitTooLarge { .. })
        ));
    }

    #[test]
    fn full_submission_channel_is_rejected_without_backlog_growth() -> Result<(), Box<dyn Error>> {
        let (sender, _receiver) = mpsc::sync_channel(1);
        sender.try_send(queued_stage(1)?)?;
        assert!(matches!(
            sender.try_send(queued_stage(2)?),
            Err(std::sync::mpsc::TrySendError::Full(_))
        ));
        Ok(())
    }

    #[test]
    fn queue_rejects_worker_counts_beyond_resource_admission() -> Result<(), Box<dyn Error>> {
        let limits = StreamingUsdStageLimits::new(2, 1, 1024)?;
        let governor = std::sync::Arc::new(ConversionResourceGovernor::new(
            ConversionResourceLimits::new(1, 1024)?,
        ));
        assert!(matches!(
            StreamingUsdStageQueue::new(limits, governor),
            Err(StreamingUsdStageConfigError::WorkerCountExceedsGovernor { .. })
        ));
        Ok(())
    }

    fn queued_stage(submission_id: u64) -> Result<QueuedStage, Box<dyn Error>> {
        let stage = UsdStageRequest::new(
            env!("CARGO_MANIFEST_DIR"),
            "Cargo.toml".parse::<SourceAssetPath>()?,
            "",
        )?;
        let (reply, _receiver) = mpsc::channel();
        Ok(QueuedStage {
            submission_id,
            request: StreamingUsdStageRequest::plain_usda(stage),
            reply,
        })
    }
}
