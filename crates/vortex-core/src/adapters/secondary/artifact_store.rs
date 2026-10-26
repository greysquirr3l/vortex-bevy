use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    application::{
        command::{
            clean_output::OutputCleaner,
            execute_conversion::ArtifactStore,
            plan_conversion::ExistingConversionReader,
            verify_artifacts::{ArtifactVerification, ArtifactVerifier},
        },
        query::conversion_report::ConversionReportReader,
    },
    domain::{
        ArtifactHash, ConversionAggregate, ConversionId, Diagnostic, OutputRoot, SceneManifest,
    },
};

const MANIFEST_FILE: &str = "scene.manifest.ron";
const REPORT_FILE: &str = "conversion-report.json";
const STAGING_DIRECTORY: &str = ".staging";
const LOCK_DIRECTORY: &str = ".locks";
static STAGING_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// One verified byte payload staged beside a published scene manifest.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ArtifactPayload {
    pub relative_path: String,
    pub bytes: Vec<u8>,
}

impl ArtifactPayload {
    /// Creates a payload after validating its portable relative path.
    ///
    /// # Errors
    ///
    /// Returns [`ArtifactStoreError::UnsafeRelativePath`] for absolute, traversing, or empty paths.
    pub fn new(
        relative_path: impl Into<String>,
        bytes: Vec<u8>,
    ) -> Result<Self, ArtifactStoreError> {
        let relative_path = relative_path.into();
        validate_relative_path(&relative_path)?;
        Ok(Self { relative_path, bytes })
    }

    #[must_use]
    pub fn hash(&self) -> ArtifactHash {
        ArtifactHash::from_bytes(&self.bytes)
    }
}

/// All data required to publish one complete conversion directory.
#[derive(Clone, Debug)]
pub struct PublicationRequest {
    pub conversion: ConversionAggregate,
    pub manifest: SceneManifest,
    pub payloads: Vec<ArtifactPayload>,
}

impl PublicationRequest {
    /// Validates deterministic path and hash relationships before filesystem mutation.
    ///
    /// # Errors
    ///
    /// Returns a typed error if payload paths conflict, hashes mismatch, or the finalized
    /// conversion does not agree with the manifest profile hash.
    pub fn validate(&self) -> Result<(), ArtifactStoreError> {
        if self.conversion.context().profile_hash != self.manifest.profile_hash {
            return Err(ArtifactStoreError::ProfileHashMismatch);
        }
        let mut payloads = BTreeMap::new();
        for payload in &self.payloads {
            validate_relative_path(&payload.relative_path)?;
            if payloads.insert(payload.relative_path.as_str(), payload.hash()).is_some() {
                return Err(ArtifactStoreError::DuplicatePayload {
                    path: payload.relative_path.clone(),
                });
            }
        }
        for artifact in &self.manifest.artifacts {
            let Some(actual) = payloads.get(artifact.path.as_str()) else {
                return Err(ArtifactStoreError::MissingPayload { path: artifact.path.clone() });
            };
            if actual != &artifact.hash {
                return Err(ArtifactStoreError::ChecksumMismatch {
                    path: artifact.path.clone(),
                    expected: artifact.hash.clone(),
                    actual: actual.clone(),
                });
            }
        }
        Ok(())
    }
}

/// Outcome of a deterministic publication attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublicationOutcome {
    Published,
    Reused,
}

/// Fail-closed filesystem artifact publication rooted in one canonical output directory.
#[derive(Debug)]
pub struct AtomicArtifactStore {
    root: PathBuf,
    _logical_root: OutputRoot,
    fail_after_payloads: Option<usize>,
}

impl AtomicArtifactStore {
    /// Opens one typed output root and canonicalizes its physical directory.
    ///
    /// # Errors
    ///
    /// Returns [`ArtifactStoreError::Io`] when the root cannot be created or canonicalized.
    pub fn new(
        root: impl AsRef<Path>,
        logical_root: OutputRoot,
    ) -> Result<Self, ArtifactStoreError> {
        let root = root.as_ref();
        fs::create_dir_all(root).map_err(|source| ArtifactStoreError::Io {
            operation: "create output root",
            path: root.to_path_buf(),
            source,
        })?;
        let root = root.canonicalize().map_err(|source| ArtifactStoreError::Io {
            operation: "canonicalize output root",
            path: root.to_path_buf(),
            source,
        })?;
        Ok(Self { root, _logical_root: logical_root, fail_after_payloads: None })
    }

    /// Configures a deterministic test-only write interruption without exposing partial finals.
    #[must_use]
    pub const fn with_failure_after_payloads(mut self, payload_count: usize) -> Self {
        self.fail_after_payloads = Some(payload_count);
        self
    }

    /// Stages, verifies, fsyncs, and atomically publishes one conversion directory.
    ///
    /// # Errors
    ///
    /// Returns a typed path, checksum, lock, filesystem, or report error. Failed attempts clean
    /// their staging directory and never replace an existing final conversion directory.
    pub fn publish(
        &self,
        request: &PublicationRequest,
    ) -> Result<PublicationOutcome, ArtifactStoreError> {
        request.validate()?;
        let conversion_id = request.conversion.id();
        let _lock = self.acquire_lock(conversion_id)?;
        let final_directory = self.conversion_directory(conversion_id)?;
        if final_directory.exists() {
            return self.reuse_if_equivalent(&final_directory, request);
        }
        let staging_directory = self.staging_directory(conversion_id)?;
        let result = self.write_staging(&staging_directory, request).and_then(|()| {
            fs::rename(&staging_directory, &final_directory).map_err(|source| {
                ArtifactStoreError::Io {
                    operation: "rename staged conversion",
                    path: final_directory.clone(),
                    source,
                }
            })?;
            sync_directory(&self.root)?;
            Ok(PublicationOutcome::Published)
        });
        if result.is_err() {
            remove_directory_if_exists(&staging_directory)?;
        }
        result
    }

    /// Returns the root-confined manifest path for one published conversion.
    ///
    /// # Errors
    ///
    /// Returns a typed error if the conversion ID cannot be resolved beneath the configured root.
    pub fn published_manifest_path(
        &self,
        conversion_id: &ConversionId,
    ) -> Result<PathBuf, ArtifactStoreError> {
        let directory = self.conversion_directory(conversion_id)?;
        confined_join(&directory, MANIFEST_FILE)
    }

    /// Lists published conversion IDs beneath the configured root.
    ///
    /// Returns lexically sorted IDs for every subdirectory that contains the
    /// `scene.manifest.ron` sidecar. Unknown entries (no manifest) are
    /// silently skipped — they are treated as partial leftovers, not
    /// loadable conversions. Returns an empty vector if the root has no
    /// published conversions yet.
    pub fn list_published_conversions(&self) -> Result<Vec<ConversionId>, ArtifactStoreError> {
        let mut ids = Vec::new();
        for entry in fs::read_dir(&self.root).map_err(|source| ArtifactStoreError::Io {
            operation: "read artifact root for listing",
            path: self.root.clone(),
            source,
        })? {
            let entry = entry.map_err(|source| ArtifactStoreError::Io {
                operation: "read artifact root entry",
                path: self.root.clone(),
                source,
            })?;
            let name = entry.file_name();
            let name_str = name.to_string_lossy().into_owned();
            let Ok(id) = ConversionId::parse(&name_str) else {
                continue;
            };
            let Ok(manifest_path) = self.published_manifest_path(&id) else {
                continue;
            };
            if manifest_path.is_file() {
                ids.push(id);
            }
        }
        ids.sort();
        Ok(ids)
    }

    /// Verifies all hashes declared by one published scene manifest.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the conversion is absent, a manifest is invalid, or an artifact
    /// differs from its declared hash.
    pub fn verify_published(
        &self,
        conversion_id: &ConversionId,
    ) -> Result<ArtifactVerification, ArtifactStoreError> {
        let directory = self.conversion_directory(conversion_id)?;
        let manifest_path = directory.join(MANIFEST_FILE);
        let bytes = read_file(&manifest_path, "read scene manifest")?;
        let manifest = SceneManifest::from_ron(&bytes, crate::domain::DEFAULT_MANIFEST_LIMIT_BYTES)
            .map_err(|source| ArtifactStoreError::Manifest { source })?;
        let mut verified_hashes = Vec::with_capacity(manifest.artifacts.len());
        for artifact in manifest.artifacts {
            let path = confined_join(&directory, &artifact.path)?;
            let bytes = read_file(&path, "read published artifact")?;
            let actual = ArtifactHash::from_bytes(&bytes);
            if actual != artifact.hash {
                return Err(ArtifactStoreError::ChecksumMismatch {
                    path: artifact.path,
                    expected: artifact.hash,
                    actual,
                });
            }
            verified_hashes.push(actual);
        }
        Ok(ArtifactVerification {
            conversion_id: conversion_id.clone(),
            verified_hashes,
            diagnostics: Vec::<Diagnostic>::new(),
        })
    }

    /// Removes only an owned conversion directory and its residual staging directories.
    ///
    /// # Errors
    ///
    /// Returns a typed filesystem error if an owned directory cannot be removed.
    pub fn clean_conversion(&self, conversion_id: &ConversionId) -> Result<(), ArtifactStoreError> {
        let final_directory = self.conversion_directory(conversion_id)?;
        remove_directory_if_exists(&final_directory)?;
        self.clean_staging(conversion_id)
    }

    fn reuse_if_equivalent(
        &self,
        final_directory: &Path,
        request: &PublicationRequest,
    ) -> Result<PublicationOutcome, ArtifactStoreError> {
        let report = Self::read_report_at(final_directory)?;
        if report.id() != request.conversion.id()
            || report.context().profile_hash != request.conversion.context().profile_hash
        {
            return Err(ArtifactStoreError::ExistingConversionConflict {
                conversion_id: request.conversion.id().clone(),
            });
        }
        self.verify_published(request.conversion.id())?;
        Ok(PublicationOutcome::Reused)
    }

    fn write_staging(
        &self,
        staging_directory: &Path,
        request: &PublicationRequest,
    ) -> Result<(), ArtifactStoreError> {
        fs::create_dir(staging_directory).map_err(|source| ArtifactStoreError::Io {
            operation: "create staging directory",
            path: staging_directory.to_path_buf(),
            source,
        })?;
        for (index, payload) in request.payloads.iter().enumerate() {
            if self.fail_after_payloads.is_some_and(|limit| index >= limit) {
                return Err(ArtifactStoreError::InjectedWriteFailure);
            }
            let path = confined_join(staging_directory, &payload.relative_path)?;
            write_file(&path, &payload.bytes)?;
        }
        let manifest =
            request.manifest.to_ron().map_err(|source| ArtifactStoreError::Manifest { source })?;
        write_file(&staging_directory.join(MANIFEST_FILE), manifest.as_bytes())?;
        let report = serde_json::to_vec_pretty(&request.conversion)
            .map_err(|source| ArtifactStoreError::ReportSerialization { source })?;
        write_file(&staging_directory.join(REPORT_FILE), &report)?;
        sync_directory(staging_directory)
    }

    fn acquire_lock(
        &self,
        conversion_id: &ConversionId,
    ) -> Result<PublicationLock, ArtifactStoreError> {
        let locks = self.root.join(LOCK_DIRECTORY);
        fs::create_dir_all(&locks).map_err(|source| ArtifactStoreError::Io {
            operation: "create lock directory",
            path: locks.clone(),
            source,
        })?;
        let path = locks.join(format!("{}.lock", conversion_id.as_str()));
        match OpenOptions::new().create_new(true).write(true).open(&path) {
            Ok(file) => Ok(PublicationLock { path, _file: file }),
            Err(source) if source.kind() == io::ErrorKind::AlreadyExists => {
                Err(ArtifactStoreError::LockContended { conversion_id: conversion_id.clone() })
            }
            Err(source) => {
                Err(ArtifactStoreError::Io { operation: "acquire conversion lock", path, source })
            }
        }
    }

    fn conversion_directory(
        &self,
        conversion_id: &ConversionId,
    ) -> Result<PathBuf, ArtifactStoreError> {
        confined_join(&self.root, conversion_id.as_str())
    }

    fn staging_directory(
        &self,
        conversion_id: &ConversionId,
    ) -> Result<PathBuf, ArtifactStoreError> {
        let staging_root = self.root.join(STAGING_DIRECTORY);
        fs::create_dir_all(&staging_root).map_err(|source| ArtifactStoreError::Io {
            operation: "create staging root",
            path: staging_root.clone(),
            source,
        })?;
        let sequence = STAGING_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        confined_join(&staging_root, &format!("{}-{sequence}", conversion_id.as_str()))
    }

    fn read_report_at(directory: &Path) -> Result<ConversionAggregate, ArtifactStoreError> {
        let bytes = read_file(&directory.join(REPORT_FILE), "read conversion report")?;
        serde_json::from_slice(&bytes)
            .map_err(|source| ArtifactStoreError::ReportDeserialization { source })
    }
}

impl ArtifactStore for AtomicArtifactStore {
    type Error = ArtifactStoreError;

    fn persist(&self, conversion: &ConversionAggregate) -> Result<(), Self::Error> {
        let _lock = self.acquire_lock(conversion.id())?;
        let directory = self.conversion_directory(conversion.id())?;
        fs::create_dir_all(&directory).map_err(|source| ArtifactStoreError::Io {
            operation: "create conversion directory for persist",
            path: directory.clone(),
            source,
        })?;
        let report = serde_json::to_vec_pretty(conversion)
            .map_err(|source| ArtifactStoreError::ReportSerialization { source })?;
        write_file(&directory.join(REPORT_FILE), &report)?;
        sync_directory(&directory)
    }
}

impl ArtifactVerifier for AtomicArtifactStore {
    type Error = ArtifactStoreError;

    fn verify(&self, id: &ConversionId) -> Result<ArtifactVerification, Self::Error> {
        self.verify_published(id)
    }
}

impl OutputCleaner for AtomicArtifactStore {
    type Error = ArtifactStoreError;

    fn clean_staging(&self, id: &ConversionId) -> Result<(), Self::Error> {
        let staging_root = self.root.join(STAGING_DIRECTORY);
        if !staging_root.exists() {
            return Ok(());
        }
        let prefix = format!("{}-", id.as_str());
        for entry in fs::read_dir(&staging_root).map_err(|source| ArtifactStoreError::Io {
            operation: "read staging root",
            path: staging_root.clone(),
            source,
        })? {
            let entry = entry.map_err(|source| ArtifactStoreError::Io {
                operation: "read staging entry",
                path: staging_root.clone(),
                source,
            })?;
            let name = entry.file_name();
            if name.to_string_lossy().starts_with(&prefix) {
                remove_directory_if_exists(&entry.path())?;
            }
        }
        Ok(())
    }
}

impl ConversionReportReader for AtomicArtifactStore {
    type Error = ArtifactStoreError;

    fn read(&self, id: &ConversionId) -> Result<Option<ConversionAggregate>, Self::Error> {
        let directory = self.conversion_directory(id)?;
        if !directory.exists() {
            return Ok(None);
        }
        Self::read_report_at(&directory).map(Some)
    }
}

impl ExistingConversionReader for AtomicArtifactStore {
    type Error = ArtifactStoreError;

    fn find_compatible(
        &self,
        id: &ConversionId,
    ) -> Result<Option<ConversionAggregate>, Self::Error> {
        // ConversionId is deterministically derived from (source, profile_hash), so the
        // directory presence alone is the compatibility check: same source + same profile
        // produces the same ID, and the serialized report carries both.
        self.read(id)
    }
}

#[derive(Debug)]
struct PublicationLock {
    path: PathBuf,
    _file: File,
}

impl Drop for PublicationLock {
    fn drop(&mut self) {
        let _ignored = fs::remove_file(&self.path);
    }
}

#[derive(Debug, Error)]
pub enum ArtifactStoreError {
    #[error("artifact path is not a safe relative path: {path}")]
    UnsafeRelativePath { path: String },
    #[error("artifact payload path is duplicated: {path}")]
    DuplicatePayload { path: String },
    #[error("manifest declares a missing payload: {path}")]
    MissingPayload { path: String },
    #[error("artifact checksum mismatch for {path}: expected {expected}, got {actual}")]
    ChecksumMismatch { path: String, expected: ArtifactHash, actual: ArtifactHash },
    #[error("conversion and manifest profile hashes differ")]
    ProfileHashMismatch,
    #[error("conversion lock is contended: {conversion_id}")]
    LockContended { conversion_id: ConversionId },
    #[error("a different valid conversion already owns {conversion_id}")]
    ExistingConversionConflict { conversion_id: ConversionId },
    #[error("an injected staging write failure occurred")]
    InjectedWriteFailure,
    #[error("filesystem operation {operation} failed at {path}: {source}")]
    Io { operation: &'static str, path: PathBuf, source: io::Error },
    #[error("scene manifest is invalid: {source}")]
    Manifest { source: crate::domain::ManifestError },
    #[error("conversion report serialization failed: {source}")]
    ReportSerialization { source: serde_json::Error },
    #[error("conversion report deserialization failed: {source}")]
    ReportDeserialization { source: serde_json::Error },
}

fn validate_relative_path(value: &str) -> Result<(), ArtifactStoreError> {
    let path = Path::new(value);
    if value.is_empty()
        || path.is_absolute()
        || path.components().any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(ArtifactStoreError::UnsafeRelativePath { path: value.to_owned() });
    }
    Ok(())
}

fn confined_join(root: &Path, relative: &str) -> Result<PathBuf, ArtifactStoreError> {
    validate_relative_path(relative)?;
    let path = root.join(relative);
    if !path.starts_with(root) {
        return Err(ArtifactStoreError::UnsafeRelativePath { path: relative.to_owned() });
    }
    Ok(path)
}

fn write_file(path: &Path, bytes: &[u8]) -> Result<(), ArtifactStoreError> {
    let parent = path.parent().ok_or_else(|| ArtifactStoreError::UnsafeRelativePath {
        path: path.display().to_string(),
    })?;
    fs::create_dir_all(parent).map_err(|source| ArtifactStoreError::Io {
        operation: "create artifact parent",
        path: parent.to_path_buf(),
        source,
    })?;
    let mut file = File::create(path).map_err(|source| ArtifactStoreError::Io {
        operation: "create artifact",
        path: path.to_path_buf(),
        source,
    })?;
    file.write_all(bytes).map_err(|source| ArtifactStoreError::Io {
        operation: "write artifact",
        path: path.to_path_buf(),
        source,
    })?;
    file.sync_all().map_err(|source| ArtifactStoreError::Io {
        operation: "sync artifact",
        path: path.to_path_buf(),
        source,
    })
}

fn read_file(path: &Path, operation: &'static str) -> Result<Vec<u8>, ArtifactStoreError> {
    fs::read(path).map_err(|source| ArtifactStoreError::Io {
        operation,
        path: path.to_path_buf(),
        source,
    })
}

fn remove_directory_if_exists(path: &Path) -> Result<(), ArtifactStoreError> {
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(ArtifactStoreError::Io {
            operation: "remove owned directory",
            path: path.to_path_buf(),
            source,
        }),
    }
}

/// Syncs a directory so that a rename into it survives a power loss.
///
/// Windows cannot open a directory as a file, so the fsync that makes a
/// rename durable is not available there. The rename is still atomic and
/// the payload files are synced individually, so durability holds at file
/// granularity.
#[cfg(not(windows))]
fn sync_directory(path: &Path) -> Result<(), ArtifactStoreError> {
    let directory = File::open(path).map_err(|source| ArtifactStoreError::Io {
        operation: "open directory for sync",
        path: path.to_path_buf(),
        source,
    })?;
    directory.sync_all().map_err(|source| ArtifactStoreError::Io {
        operation: "sync directory",
        path: path.to_path_buf(),
        source,
    })
}

/// Confirms that a directory is reachable, in place of the fsync.
///
/// Opening a directory as a file fails on Windows, so the directory cannot
/// be flushed there. The check keeps the guarantee the Unix path gets for
/// free from opening the directory: the path is a real directory at the
/// moment the caller claims to have synced it.
#[cfg(windows)]
fn sync_directory(path: &Path) -> Result<(), ArtifactStoreError> {
    let metadata = fs::metadata(path).map_err(|source| ArtifactStoreError::Io {
        operation: "inspect directory for sync",
        path: path.to_path_buf(),
        source,
    })?;
    if !metadata.is_dir() {
        return Err(ArtifactStoreError::Io {
            operation: "inspect directory for sync",
            path: path.to_path_buf(),
            source: io::Error::other("path is not a directory"),
        });
    }
    Ok(())
}
