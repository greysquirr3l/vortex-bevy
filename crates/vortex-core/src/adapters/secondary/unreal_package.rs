//! Bounded, metadata-only inspection for authorized editable Unreal packages.
//!
//! This adapter intentionally delegates semantic payload extraction to a matching
//! authorized Unreal Editor bridge. It reads only package metadata through the
//! pinned `uasset` crate and never decrypts or interprets cooked payload data.

use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
};

use thiserror::Error;
use uasset::{AssetHeader, ObjectReference};

use crate::{
    application::command::inspect_input::{InputInspection, InputInspector},
    domain::{
        ClassificationKind, ConversionClassification, DomainError, Remediation, SourceIdentity,
    },
};

/// Maximum file length accepted before invoking the third-party package parser.
pub const DEFAULT_MAX_PACKAGE_BYTES: u64 = 32 * 1024 * 1024;

const PACKAGE_FILE_MAGIC_LE: [u8; 4] = [0xc1, 0x83, 0x2a, 0x9e];
const COOKED_CONTEXT_EXTENSIONS: [&str; 3] = ["pak", "utoc", "ucas"];

/// Typed failures produced while preparing an authorized package for metadata inspection.
#[derive(Debug, Error)]
pub enum UnrealPackageInspectionError {
    /// The configured root could not be canonicalized as an existing directory.
    #[error("authorized Unreal project root is unavailable: {source}")]
    ProjectRoot { source: std::io::Error },
    /// The configured root resolved to a file rather than a directory.
    #[error("authorized Unreal project root is not a directory: {path}")]
    ProjectRootNotDirectory { path: PathBuf },
    /// The caller configured a zero-byte inspection limit.
    #[error("package inspection limit must be nonzero")]
    ZeroInspectionLimit,
    /// The source identity does not describe an Unreal package.
    #[error("metadata inspection requires an Unreal source identity")]
    NonUnrealSource,
    /// The source path could not be resolved inside the authorized root.
    #[error("package file is unavailable: {source}")]
    PackageUnavailable { source: std::io::Error },
    /// Canonicalization showed that a path escapes the authorized project root.
    #[error("package path escapes the authorized project root: {path}")]
    PathEscape { path: PathBuf },
    /// The requested source is not a `.uasset` package.
    #[error("metadata inspection requires a .uasset source: {path}")]
    InvalidExtension { path: PathBuf },
    /// The package exceeds the configured bounded inspection size.
    #[error(
        "package exceeds bounded inspection limit: {path} is {actual_bytes} bytes, limit is {max_bytes}"
    )]
    TooLarge { path: PathBuf, actual_bytes: u64, max_bytes: u64 },
    /// Prefix reads fail before the third-party parser receives the package bytes.
    #[error("could not read package prefix: {source}")]
    PrefixRead { source: std::io::Error },
    /// Static adapter invariants failed while constructing a domain classification.
    #[error("could not construct inspection classification: {source}")]
    Classification { source: DomainError },
}

/// Metadata-only result for one authorized Unreal package preflight.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnrealPackagePreflight {
    /// Source identity requested by the caller.
    pub source: SourceIdentity,
    /// Explicit conversion eligibility for the observed package context.
    pub classification: ConversionClassification,
    /// Canonically sorted stable metadata facts suitable for a report boundary.
    pub facts: Vec<String>,
}

/// Bounded metadata reader confined to one canonical authorized project root.
#[derive(Clone, Debug)]
pub struct UnrealPackageInspector {
    project_root: PathBuf,
    max_package_bytes: u64,
}

impl UnrealPackageInspector {
    /// Creates an inspector confined to an existing authorized project root.
    ///
    /// # Errors
    ///
    /// Returns [`UnrealPackageInspectionError::ProjectRoot`] when the root cannot
    /// be canonicalized or [`UnrealPackageInspectionError::ProjectRootNotDirectory`]
    /// when it is not a directory.
    pub fn new(project_root: impl AsRef<Path>) -> Result<Self, UnrealPackageInspectionError> {
        Self::with_max_package_bytes(project_root, DEFAULT_MAX_PACKAGE_BYTES)
    }

    /// Creates an inspector with a caller-supplied nonzero package-size limit.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`Self::new`] when the root is invalid or
    /// [`UnrealPackageInspectionError::ZeroInspectionLimit`] when the limit is zero.
    pub fn with_max_package_bytes(
        project_root: impl AsRef<Path>,
        max_package_bytes: u64,
    ) -> Result<Self, UnrealPackageInspectionError> {
        if max_package_bytes == 0 {
            return Err(UnrealPackageInspectionError::ZeroInspectionLimit);
        }
        let project_root = fs::canonicalize(project_root)
            .map_err(|source| UnrealPackageInspectionError::ProjectRoot { source })?;
        if !project_root.is_dir() {
            return Err(UnrealPackageInspectionError::ProjectRootNotDirectory {
                path: project_root,
            });
        }
        Ok(Self { project_root, max_package_bytes })
    }

    /// Inspects one explicit Unreal source without decoding its object payloads.
    ///
    /// # Errors
    ///
    /// Returns a typed error before parser invocation for invalid source kind,
    /// project confinement, file extension, prefix I/O, or size limits.
    pub fn preflight(
        &self,
        source: &SourceIdentity,
    ) -> Result<UnrealPackagePreflight, UnrealPackageInspectionError> {
        let SourceIdentity::Unreal { asset_path, .. } = source else {
            return Err(UnrealPackageInspectionError::NonUnrealSource);
        };
        let package_path = self.canonical_package_path(asset_path.as_str())?;
        let metadata = fs::metadata(&package_path)
            .map_err(|source| UnrealPackageInspectionError::PackageUnavailable { source })?;
        if metadata.len() > self.max_package_bytes {
            return Err(UnrealPackageInspectionError::TooLarge {
                path: package_path,
                actual_bytes: metadata.len(),
                max_bytes: self.max_package_bytes,
            });
        }

        let mut facts = vec![
            format!("package.bytes={}", metadata.len()),
            format!("package.path={}", asset_path.as_str()),
        ];
        if Self::has_cooked_context(&package_path) {
            return Self::rejected(
                source,
                facts,
                "unreal.cooked_context_detected",
                "A cooked container or sidecar was detected beside the package.",
                "Use the editable authorized source project and its matching Unreal Editor bridge.",
            );
        }

        if !Self::has_package_magic(&package_path)? {
            return Self::rejected(
                source,
                facts,
                "unreal.invalid_package_magic",
                "The selected .uasset does not contain the Unreal package magic prefix.",
                "Select an authorized editable .uasset from the project Content tree.",
            );
        }

        let header = match File::open(&package_path)
            .map_err(|source| UnrealPackageInspectionError::PackageUnavailable { source })
            .and_then(|file| {
                AssetHeader::new(file).map_err(|error| {
                    let message = error.to_string();
                    UnrealPackageInspectionError::PrefixRead {
                        source: std::io::Error::new(std::io::ErrorKind::InvalidData, message),
                    }
                })
            }) {
            Ok(header) => header,
            Err(UnrealPackageInspectionError::PrefixRead { source: parser_error }) => {
                return Self::rejected(
                    source,
                    facts,
                    "unreal.metadata_parse_failed",
                    &format!(
                        "The bounded package metadata parser rejected this header: {parser_error}"
                    ),
                    "Use the matching Unreal Editor bridge for unsupported package versions or repair the source package.",
                );
            }
            Err(error) => return Err(error),
        };

        if Self::requires_external_bulk_companion(&header, metadata.len())
            && !package_path.with_extension("ubulk").is_file()
        {
            facts.push("companion.ubulk=missing".to_owned());
            return Self::rejected(
                source,
                facts,
                "unreal.required_bulk_companion_missing",
                "The package header requires an external bulk-data companion that is absent.",
                "Restore the approved source package and its companion files before using the matching Unreal Editor bridge.",
            );
        }

        facts.extend(Self::header_facts(&header));
        facts.sort();
        let classification = ConversionClassification::new(
            ClassificationKind::RequiresAdapter,
            "Metadata inspection succeeded, but semantic export requires the authorized matching Unreal Editor bridge.",
            Remediation::new(
                "Run the versioned Unreal Editor bridge against the approved editable project.",
            )
            .map_err(|source| UnrealPackageInspectionError::Classification { source })?,
        )
        .map_err(|source| UnrealPackageInspectionError::Classification { source })?;
        Ok(UnrealPackagePreflight { source: source.clone(), classification, facts })
    }

    fn canonical_package_path(
        &self,
        raw_asset_path: &str,
    ) -> Result<PathBuf, UnrealPackageInspectionError> {
        let candidate = self.project_root.join(raw_asset_path);
        let package_path = fs::canonicalize(candidate)
            .map_err(|source| UnrealPackageInspectionError::PackageUnavailable { source })?;
        if !package_path.starts_with(&self.project_root) {
            return Err(UnrealPackageInspectionError::PathEscape { path: package_path });
        }
        let is_uasset = package_path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("uasset"));
        if !is_uasset {
            return Err(UnrealPackageInspectionError::InvalidExtension { path: package_path });
        }
        Ok(package_path)
    }

    fn has_cooked_context(package_path: &Path) -> bool {
        let has_container = COOKED_CONTEXT_EXTENSIONS
            .iter()
            .any(|extension| package_path.with_extension(extension).is_file());
        let has_export_sidecar = package_path.with_extension("uexp").is_file();
        let has_bulk_sidecar = package_path.with_extension("ubulk").is_file();
        has_container || has_export_sidecar || has_bulk_sidecar
    }

    fn has_package_magic(package_path: &Path) -> Result<bool, UnrealPackageInspectionError> {
        let mut file = File::open(package_path)
            .map_err(|source| UnrealPackageInspectionError::PackageUnavailable { source })?;
        let mut prefix = [0_u8; PACKAGE_FILE_MAGIC_LE.len()];
        match file.read_exact(&mut prefix) {
            Ok(()) => Ok(prefix == PACKAGE_FILE_MAGIC_LE),
            Err(source) if source.kind() == std::io::ErrorKind::UnexpectedEof => Ok(false),
            Err(source) => Err(UnrealPackageInspectionError::PrefixRead { source }),
        }
    }

    fn requires_external_bulk_companion<R>(header: &AssetHeader<R>, package_bytes: u64) -> bool
    where
        R: Read + std::io::Seek,
    {
        header.bulk_data_start_offset > 0
            && u64::try_from(header.bulk_data_start_offset)
                .is_ok_and(|offset| offset > package_bytes)
    }

    fn header_facts<R>(header: &AssetHeader<R>) -> Vec<String>
    where
        R: Read + std::io::Seek,
    {
        let mut facts = vec![
            format!("package.flags={}", header.package_flags),
            format!("package.name={}", header.package_name),
            format!("package.version.legacy={}", header.archive.legacy_version),
            format!("package.version.ue4={:?}", header.archive.file_version),
            format!("package.version.ue5={:?}", header.archive.file_version_ue5),
            format!(
                "package.version.engine={}.{}.{}",
                header.engine_version.major,
                header.engine_version.minor,
                header.engine_version.patch
            ),
            format!("table.exports={}", header.exports.len()),
            format!("table.imports={}", header.imports.len()),
            format!("table.names={}", header.names.len()),
        ];
        for dependency in header.package_import_iter().collect::<BTreeSet<_>>() {
            facts.push(format!("dependency.package={dependency}"));
        }
        let class_names = Self::export_class_names(header);
        if class_names.is_empty() {
            facts.push("export.class=<unknown>".to_owned());
        } else {
            for class_name in class_names {
                facts.push(format!("export.class={class_name}"));
            }
        }
        facts
    }

    fn export_class_names<R>(header: &AssetHeader<R>) -> BTreeSet<String>
    where
        R: Read + std::io::Seek,
    {
        header
            .exports
            .iter()
            .filter_map(|export| match export.class() {
                ObjectReference::Import { import_index } => header
                    .imports
                    .get(import_index)
                    .and_then(|import| header.resolve_name(&import.class_name).ok())
                    .map(std::borrow::Cow::into_owned),
                ObjectReference::Export { export_index } => header
                    .exports
                    .get(export_index)
                    .and_then(|class_export| header.resolve_name(&class_export.object_name).ok())
                    .map(std::borrow::Cow::into_owned),
                ObjectReference::None => None,
            })
            .collect()
    }

    fn rejected(
        source: &SourceIdentity,
        mut facts: Vec<String>,
        diagnostic_code: &str,
        reason: &str,
        remediation: &str,
    ) -> Result<UnrealPackagePreflight, UnrealPackageInspectionError> {
        facts.push(format!("diagnostic.code={diagnostic_code}"));
        facts.sort();
        let classification = ConversionClassification::new(
            ClassificationKind::Rejected,
            reason,
            Remediation::new(remediation)
                .map_err(|source| UnrealPackageInspectionError::Classification { source })?,
        )
        .map_err(|source| UnrealPackageInspectionError::Classification { source })?;
        Ok(UnrealPackagePreflight { source: source.clone(), classification, facts })
    }
}

impl InputInspector for UnrealPackageInspector {
    type Error = UnrealPackageInspectionError;

    fn inspect(&self, source: &SourceIdentity) -> Result<InputInspection, Self::Error> {
        let preflight = self.preflight(source)?;
        Ok(InputInspection {
            source: preflight.source,
            classification: preflight.classification,
            facts: preflight.facts,
        })
    }
}
