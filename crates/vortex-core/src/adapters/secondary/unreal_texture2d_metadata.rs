//! Bounded direct decoding of UE 5.8.2 `UTexture2D` asset-registry metadata.
//!
//! This adapter reads only the legacy tagged asset-registry section recorded in an
//! editable `.uasset` header. It does not deserialize `UTexture2D` payloads, read
//! pixels, interpret a `.ubulk` companion, or replace the matching Unreal Editor
//! bridge for authoritative texture export.

use std::collections::BTreeMap;

use serde::Deserialize;
use thiserror::Error;

use crate::domain::{
    ClassificationKind, ConversionClassification, DomainError, Remediation, SourceIdentity,
};

/// Maximum package size accepted for direct texture metadata inspection.
pub const MAX_UNREAL_TEXTURE_METADATA_BYTES: usize = 32 * 1024 * 1024;
/// Maximum strings accepted in the fixed UE 5.8.2 header and tag table.
pub const MAX_UNREAL_TEXTURE_METADATA_STRING_BYTES: usize = 4 * 1024;
/// Maximum custom-version entries accepted while skipping the UE package header.
pub const MAX_UNREAL_TEXTURE_METADATA_CUSTOM_VERSIONS: usize = 128;
/// Maximum asset-registry records accepted by the selected `UTexture2D` subset.
pub const MAX_UNREAL_TEXTURE_METADATA_RECORDS: usize = 1;
/// Maximum tags accepted for the selected `UTexture2D` subset.
pub const MAX_UNREAL_TEXTURE_METADATA_TAGS: usize = 8;
/// Largest accepted source-image dimension reported by the selected subset.
pub const MAX_UNREAL_TEXTURE_METADATA_DIMENSION: u32 = 16_384;

const PACKAGE_MAGIC: u32 = 0x9e2a_83c1;
const LEGACY_FILE_VERSION: i32 = -9;
const UE4_FILE_VERSION: i32 = 522;
const UE5_FILE_VERSION: i32 = 1018;
const UE5_MAJOR: u16 = 5;
const UE5_MINOR: u16 = 8;
const UE5_PATCH: u16 = 2;
const PACKAGE_FLAG_COOKED: u32 = 0x0000_0200;
const PACKAGE_FLAG_FILTER_EDITOR_ONLY: u32 = 0x8000_0000;
const SAVED_HASH_BYTES: usize = 20;
const GUID_BYTES: usize = 16;
const CUSTOM_VERSION_BYTES: usize = 20;
const ORACLE_PROTOCOL_VERSION: &str = "vortex.unreal-texture-metadata/v1";

/// The only Unreal release accepted by this direct metadata decoder.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnrealTexture2DVersion {
    /// A versioned editable package produced by Unreal Engine 5.8.2.
    Unreal5_8_2,
}

/// Source dimensions reported by the selected `UTexture2D` metadata subset.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnrealTextureDimensions {
    /// Horizontal source-image pixel count.
    pub width: u32,
    /// Vertical source-image pixel count.
    pub height: u32,
}

/// Supported pixel-format facts for the selected texture metadata subset.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnrealTexturePixelFormat {
    /// BGRA 8-bit pixels.
    B8G8R8A8,
    /// Block-compressed DXT1 pixels.
    Dxt1,
    /// Block-compressed DXT5 pixels.
    Dxt5,
    /// Two-channel block-compressed normal-map pixels.
    Bc5,
    /// Block-compressed BC7 pixels.
    Bc7,
}

/// Supported texture compression-setting facts for the selected metadata subset.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnrealTextureCompression {
    /// Unreal's default texture compression setting.
    Default,
    /// Unreal's normal-map compression setting.
    NormalMap,
    /// Unreal's mask compression setting.
    Masks,
    /// Unreal's grayscale compression setting.
    Grayscale,
}

/// Supported texture filter facts for the selected metadata subset.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnrealTextureFilter {
    /// Use the project or texture-group default filter.
    Default,
    /// Nearest-neighbor filtering.
    Nearest,
    /// Bilinear filtering.
    Bilinear,
    /// Trilinear filtering.
    Trilinear,
}

/// Whether the package header identifies texture bulk data as internal or external.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnrealTextureBulkReference {
    /// The package's bulk-data start offset is inside the supplied package bytes.
    Inline,
    /// The bulk-data start offset is outside the package; a nonempty companion was supplied.
    ExternalCompanion,
}

/// Typed direct metadata extracted from one selected `UTexture2D` record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnrealTexture2DMetadata {
    /// The caller-provided, source-traceable Unreal identity.
    pub source: SourceIdentity,
    /// The only supported engine-version capability record.
    pub version: UnrealTexture2DVersion,
    /// Source-image dimensions reported by the asset-registry tag table.
    pub dimensions: UnrealTextureDimensions,
    /// Pixel-format fact reported by the asset-registry tag table.
    pub pixel_format: UnrealTexturePixelFormat,
    /// Compression-setting fact reported by the asset-registry tag table.
    pub compression: UnrealTextureCompression,
    /// Optional filter fact reported by the asset-registry tag table.
    pub filter: Option<UnrealTextureFilter>,
    /// Optional hexadecimal source-file hash reported by the asset-registry tag table.
    pub source_file_hash: Option<String>,
    /// Whether bulk data is internal to the package or requires an unread companion.
    pub bulk_reference: UnrealTextureBulkReference,
    /// Explicit boundary: metadata succeeds but authoritative texture export remains editor-owned.
    pub classification: ConversionClassification,
}

/// An independent, typed Editor asset-registry observation used for semantic comparison.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnrealTexture2DMetadataOracle {
    facts: TextureFacts,
}

/// Bounded failures emitted by direct texture metadata decoding and oracle comparison.
#[derive(Debug, Error)]
pub enum UnrealTexture2DMetadataError {
    /// The received package exceeds the configured direct-metadata limit.
    #[error(
        "Unreal texture metadata package is {actual_bytes} bytes, exceeding the {max_bytes}-byte limit"
    )]
    PackageTooLarge { actual_bytes: usize, max_bytes: usize },
    /// The caller did not provide an Unreal source identity.
    #[error("direct texture metadata decoding requires an Unreal source identity")]
    NonUnrealSource,
    /// The package did not start with the expected Unreal package tag.
    #[error("direct texture metadata decoder received an invalid package magic")]
    InvalidPackageMagic,
    /// The package's legacy version cannot be interpreted by the selected decoder.
    #[error(
        "direct texture metadata decoder supports legacy package version {expected}, received {actual}"
    )]
    UnsupportedLegacyVersion { expected: i32, actual: i32 },
    /// The package's UE4/UE5 file-version pair is outside the single approved capability.
    #[error(
        "direct texture metadata decoder supports UE4/UE5 file versions {expected_ue4}/{expected_ue5}, received {actual_ue4}/{actual_ue5}"
    )]
    UnsupportedPackageVersion {
        expected_ue4: i32,
        expected_ue5: i32,
        actual_ue4: i32,
        actual_ue5: i32,
    },
    /// The package has a nonzero licensee version that is outside the approved capability.
    #[error("direct texture metadata decoder rejects nonzero Unreal licensee version {actual}")]
    UnsupportedLicenseeVersion { actual: i32 },
    /// The package is marked as cooked and is not an editable source package.
    #[error("direct texture metadata decoder rejects cooked packages")]
    CookedPackage,
    /// The exact UE 5.8.2 saved-by engine identity was not present.
    #[error(
        "direct texture metadata decoder requires saved-by Unreal Engine 5.8.2, received {major}.{minor}.{patch}"
    )]
    UnsupportedEngineVersion { major: u16, minor: u16, patch: u16 },
    /// A bounded string length is invalid or exceeds the selected decoder limit.
    #[error("Unreal texture metadata string is invalid or exceeds the {max_bytes}-byte limit")]
    InvalidStringLength { max_bytes: usize },
    /// A string could not be decoded as the selected UTF-8 or UTF-16 representation.
    #[error("Unreal texture metadata string is not valid text")]
    InvalidString,
    /// A byte read could not be satisfied without reading beyond the package bytes.
    #[error("Unreal texture metadata package is truncated while reading {field}")]
    Truncated { field: &'static str },
    /// A signed serialized count is negative or exceeds its selected bound.
    #[error("Unreal texture metadata {field} count {actual} exceeds the {max}-entry limit")]
    CountLimit { field: &'static str, actual: i32, max: usize },
    /// An offset cannot be represented within the received package bytes.
    #[error("Unreal texture metadata {field} offset {offset} is outside the package")]
    InvalidOffset { field: &'static str, offset: i32 },
    /// The fixed package header contains compressed chunks outside this metadata-only subset.
    #[error("Unreal texture metadata decoder rejects compressed package chunks")]
    CompressedPackageChunks,
    /// The selected package has no readable legacy tagged asset-registry table.
    #[error("Unreal texture metadata package has no supported asset-registry tag table")]
    MissingAssetRegistryData,
    /// The selected package does not contain exactly one asset-registry record.
    #[error(
        "Unreal texture metadata decoder requires exactly one asset-registry record, received {actual}"
    )]
    AssetRegistryRecordCount { actual: i32 },
    /// The decoded asset-registry object path differs from the requested source identity.
    #[error("Unreal texture metadata object path differs from the requested source: {actual}")]
    UnexpectedObjectPath { actual: String },
    /// The asset-registry record is not the approved `Texture2D` class.
    #[error("Unreal texture metadata decoder requires Texture2D, received {actual}")]
    UnexpectedAssetClass { actual: String },
    /// The tag table duplicated a tag whose value would be ambiguous.
    #[error("Unreal texture metadata tag was duplicated: {tag}")]
    DuplicateTag { tag: String },
    /// A required tag was absent from the selected metadata record.
    #[error("Unreal texture metadata required tag is absent: {tag}")]
    MissingTag { tag: &'static str },
    /// A tag lies outside the approved selected metadata subset.
    #[error("Unreal texture metadata tag is unsupported: {tag}")]
    UnsupportedTag { tag: String },
    /// The selected `Dimensions` tag is invalid or outside its bounded range.
    #[error("Unreal texture metadata dimensions are invalid: {value}")]
    InvalidDimensions { value: String },
    /// The selected `PixelFormat` tag is not an approved metadata fact.
    #[error("Unreal texture metadata pixel format is unsupported: {value}")]
    UnsupportedPixelFormat { value: String },
    /// The selected `CompressionSettings` tag is not an approved metadata fact.
    #[error("Unreal texture metadata compression setting is unsupported: {value}")]
    UnsupportedCompression { value: String },
    /// The selected `Filter` tag is not an approved metadata fact.
    #[error("Unreal texture metadata filter is unsupported: {value}")]
    UnsupportedFilter { value: String },
    /// The source-file hash tag is not a 64-character hexadecimal digest.
    #[error("Unreal texture metadata source-file hash is invalid")]
    InvalidSourceFileHash,
    /// Header metadata points outside the package but no approved companion was supplied.
    #[error("Unreal texture metadata package requires a nonempty external bulk-data companion")]
    MissingExternalBulkCompanion,
    /// The independent Editor oracle report is not valid JSON.
    #[error("Unreal texture metadata oracle report is malformed: {source}")]
    OracleMalformed { source: serde_json::Error },
    /// The independent oracle report used a different protocol version.
    #[error("Unreal texture metadata oracle protocol is unsupported: {actual}")]
    OracleProtocol { actual: String },
    /// The independent oracle report named a different engine.
    #[error("Unreal texture metadata oracle engine is unsupported: {actual}")]
    OracleEngine { actual: String },
    /// The independent oracle report did not identify Unreal Engine 5.8.2.
    #[error("Unreal texture metadata oracle engine version is unsupported: {actual}")]
    OracleVersion { actual: String },
    /// The independent oracle report did not contain exactly one observation.
    #[error("Unreal texture metadata oracle requires exactly one record, received {actual}")]
    OracleRecordCount { actual: usize },
    /// The independent oracle reported cooked content, which the selected decoder rejects.
    #[error("Unreal texture metadata oracle reported cooked content")]
    OracleCooked,
    /// The independent oracle did not describe the requested source object.
    #[error(
        "Unreal texture metadata oracle object path differs from the requested source: {actual}"
    )]
    OracleUnexpectedObjectPath { actual: String },
    /// The independent oracle did not report the selected `UTexture2D` class.
    #[error("Unreal texture metadata oracle requires Texture2D, received {actual}")]
    OracleUnexpectedAssetClass { actual: String },
    /// A domain classification could not be constructed at the adapter boundary.
    #[error("Unreal texture metadata classification could not be constructed: {source}")]
    Classification { source: DomainError },
}

/// Direct, fail-closed decoder for one UE 5.8.2 `UTexture2D` tag record.
#[derive(Clone, Copy, Debug, Default)]
pub struct UnrealTexture2DMetadataDecoder;

impl UnrealTexture2DMetadataDecoder {
    /// Decodes one bounded `UTexture2D` legacy asset-registry record from editable package bytes.
    ///
    /// The optional companion is checked only for non-emptiness when the package header places
    /// bulk data outside the supplied `.uasset`; it is never opened or interpreted.
    ///
    /// # Errors
    ///
    /// Returns a typed error for an unsupported version, cooked package, malformed or truncated
    /// package, unexpected source/class/tag, invalid metadata value, or missing external companion.
    pub fn decode(
        &self,
        source: &SourceIdentity,
        package_bytes: &[u8],
        external_bulk_companion: Option<&[u8]>,
    ) -> Result<UnrealTexture2DMetadata, UnrealTexture2DMetadataError> {
        if package_bytes.len() > MAX_UNREAL_TEXTURE_METADATA_BYTES {
            return Err(UnrealTexture2DMetadataError::PackageTooLarge {
                actual_bytes: package_bytes.len(),
                max_bytes: MAX_UNREAL_TEXTURE_METADATA_BYTES,
            });
        }
        let expected_object_path = unreal_object_path(source)?;
        let header = parse_header(package_bytes)?;
        let mut table =
            Reader::at(package_bytes, header.asset_registry_data_offset, "asset registry")?;
        let facts = parse_asset_registry_record(&mut table, expected_object_path)?;
        let bulk_reference = if header.bulk_data_start_offset > package_bytes.len() {
            let companion_is_present =
                external_bulk_companion.is_some_and(|bytes| !bytes.is_empty());
            if !companion_is_present {
                return Err(UnrealTexture2DMetadataError::MissingExternalBulkCompanion);
            }
            UnrealTextureBulkReference::ExternalCompanion
        } else {
            UnrealTextureBulkReference::Inline
        };
        let classification = ConversionClassification::new(
            ClassificationKind::RequiresAdapter,
            "Direct UTexture2D metadata inspection succeeded; use the matching Unreal Editor bridge for authoritative texture export.",
            Remediation::new(
                "Run the authorized UE 5.8.2 Unreal Editor bridge to export or bake texture pixels.",
            )
            .map_err(|source| UnrealTexture2DMetadataError::Classification { source })?,
        )
        .map_err(|source| UnrealTexture2DMetadataError::Classification { source })?;
        Ok(UnrealTexture2DMetadata {
            source: source.clone(),
            version: UnrealTexture2DVersion::Unreal5_8_2,
            dimensions: facts.dimensions,
            pixel_format: facts.pixel_format,
            compression: facts.compression,
            filter: facts.filter,
            source_file_hash: facts.source_file_hash,
            bulk_reference,
            classification,
        })
    }

    /// Parses an independent non-mutating UE 5.8.2 Editor asset-registry observation.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the report is malformed, outside the approved protocol/version,
    /// identifies a different source or class, reports cooked content, or contains invalid metadata.
    pub fn parse_oracle_text(
        &self,
        source: &SourceIdentity,
        text: &str,
    ) -> Result<UnrealTexture2DMetadataOracle, UnrealTexture2DMetadataError> {
        let expected_object_path = unreal_object_path(source)?;
        let raw = serde_json::from_str::<RawOracleReport>(text)
            .map_err(|source| UnrealTexture2DMetadataError::OracleMalformed { source })?;
        if raw.protocol_version != ORACLE_PROTOCOL_VERSION {
            return Err(UnrealTexture2DMetadataError::OracleProtocol {
                actual: raw.protocol_version,
            });
        }
        if raw.engine != "unreal" {
            return Err(UnrealTexture2DMetadataError::OracleEngine { actual: raw.engine });
        }
        if raw.engine_version != "5.8.2" {
            return Err(UnrealTexture2DMetadataError::OracleVersion { actual: raw.engine_version });
        }
        if raw.records.len() != MAX_UNREAL_TEXTURE_METADATA_RECORDS {
            return Err(UnrealTexture2DMetadataError::OracleRecordCount {
                actual: raw.records.len(),
            });
        }
        let record = raw
            .records
            .into_iter()
            .next()
            .ok_or(UnrealTexture2DMetadataError::OracleRecordCount { actual: 0 })?;
        if record.cooked {
            return Err(UnrealTexture2DMetadataError::OracleCooked);
        }
        if record.object_path != expected_object_path {
            return Err(UnrealTexture2DMetadataError::OracleUnexpectedObjectPath {
                actual: record.object_path,
            });
        }
        if record.asset_kind != "Texture2D" {
            return Err(UnrealTexture2DMetadataError::OracleUnexpectedAssetClass {
                actual: record.asset_kind,
            });
        }
        Ok(UnrealTexture2DMetadataOracle { facts: TextureFacts::from_tags(&record.tags)? })
    }
}

impl UnrealTexture2DMetadataOracle {
    /// Returns whether direct decoding and the independent Editor observation agree on semantic facts.
    #[must_use]
    pub fn matches(&self, metadata: &UnrealTexture2DMetadata) -> bool {
        self.facts
            == TextureFacts {
                dimensions: metadata.dimensions,
                pixel_format: metadata.pixel_format,
                compression: metadata.compression,
                filter: metadata.filter,
                source_file_hash: metadata.source_file_hash.clone(),
            }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TextureFacts {
    dimensions: UnrealTextureDimensions,
    pixel_format: UnrealTexturePixelFormat,
    compression: UnrealTextureCompression,
    filter: Option<UnrealTextureFilter>,
    source_file_hash: Option<String>,
}

impl TextureFacts {
    fn from_tags(tags: &BTreeMap<String, String>) -> Result<Self, UnrealTexture2DMetadataError> {
        let dimensions = tags
            .get("Dimensions")
            .ok_or(UnrealTexture2DMetadataError::MissingTag { tag: "Dimensions" })
            .and_then(|value| parse_dimensions(value))?;
        let pixel_format = tags
            .get("PixelFormat")
            .ok_or(UnrealTexture2DMetadataError::MissingTag { tag: "PixelFormat" })
            .and_then(|value| parse_pixel_format(value))?;
        let compression = tags
            .get("CompressionSettings")
            .ok_or(UnrealTexture2DMetadataError::MissingTag { tag: "CompressionSettings" })
            .and_then(|value| parse_compression(value))?;
        let filter = tags.get("Filter").map(|value| parse_filter(value)).transpose()?;
        let source_file_hash =
            tags.get("SourceFileHash").map(|value| parse_source_file_hash(value)).transpose()?;
        Ok(Self { dimensions, pixel_format, compression, filter, source_file_hash })
    }
}

struct Header {
    asset_registry_data_offset: i32,
    bulk_data_start_offset: usize,
}

fn parse_header(package_bytes: &[u8]) -> Result<Header, UnrealTexture2DMetadataError> {
    let mut reader = Reader::new(package_bytes);
    read_versioned_prefix(&mut reader, package_bytes.len())?;
    read_table_summaries(&mut reader)?;
    let header = read_tail(&mut reader)?;
    Reader::at(package_bytes, header.asset_registry_data_offset, "asset registry")?;
    Ok(header)
}

fn read_versioned_prefix(
    reader: &mut Reader<'_>,
    package_byte_count: usize,
) -> Result<(), UnrealTexture2DMetadataError> {
    if reader.read_u32("package magic")? != PACKAGE_MAGIC {
        return Err(UnrealTexture2DMetadataError::InvalidPackageMagic);
    }
    let legacy_version = reader.read_i32("legacy package version")?;
    if legacy_version != LEGACY_FILE_VERSION {
        return Err(UnrealTexture2DMetadataError::UnsupportedLegacyVersion {
            expected: LEGACY_FILE_VERSION,
            actual: legacy_version,
        });
    }
    reader.skip(4, "legacy UE3 version")?;
    let ue4_version = reader.read_i32("UE4 package version")?;
    let ue5_version = reader.read_i32("UE5 package version")?;
    if ue4_version != UE4_FILE_VERSION || ue5_version != UE5_FILE_VERSION {
        return Err(UnrealTexture2DMetadataError::UnsupportedPackageVersion {
            expected_ue4: UE4_FILE_VERSION,
            expected_ue5: UE5_FILE_VERSION,
            actual_ue4: ue4_version,
            actual_ue5: ue5_version,
        });
    }
    let licensee_version = reader.read_i32("licensee package version")?;
    if licensee_version != 0 {
        return Err(UnrealTexture2DMetadataError::UnsupportedLicenseeVersion {
            actual: licensee_version,
        });
    }
    reader.skip(SAVED_HASH_BYTES, "package saved hash")?;
    let header_size = reader.read_i32("total header size")?;
    validate_header_size(header_size, package_byte_count)?;
    let custom_version_count =
        reader.read_count("custom version", MAX_UNREAL_TEXTURE_METADATA_CUSTOM_VERSIONS)?;
    let custom_version_bytes = custom_version_count.checked_mul(CUSTOM_VERSION_BYTES).ok_or(
        UnrealTexture2DMetadataError::InvalidStringLength {
            max_bytes: MAX_UNREAL_TEXTURE_METADATA_STRING_BYTES,
        },
    )?;
    reader.skip(custom_version_bytes, "custom versions")
}

fn validate_header_size(
    header_size: i32,
    package_byte_count: usize,
) -> Result<(), UnrealTexture2DMetadataError> {
    let header_size =
        usize::try_from(header_size).map_err(|_| UnrealTexture2DMetadataError::InvalidOffset {
            field: "total header size",
            offset: header_size,
        })?;
    if header_size > package_byte_count {
        let offset = i32::try_from(header_size).unwrap_or(i32::MAX);
        return Err(UnrealTexture2DMetadataError::InvalidOffset {
            field: "total header size",
            offset,
        });
    }
    Ok(())
}

fn read_table_summaries(reader: &mut Reader<'_>) -> Result<bool, UnrealTexture2DMetadataError> {
    let package_name = reader.read_string("package name")?;
    if package_name.is_empty() {
        return Err(UnrealTexture2DMetadataError::InvalidString);
    }
    let package_flags = reader.read_u32("package flags")?;
    if package_flags & PACKAGE_FLAG_COOKED != 0 {
        return Err(UnrealTexture2DMetadataError::CookedPackage);
    }
    reader.skip(8, "name map summary")?;
    reader.skip(8, "soft object path summary")?;
    let filter_editor_only = package_flags & PACKAGE_FLAG_FILTER_EDITOR_ONLY != 0;
    if !filter_editor_only {
        let _localization_id = reader.read_string("localization id")?;
    }
    reader.skip(8, "gatherable text summary")?;
    reader.skip(8, "export summary")?;
    reader.skip(8, "import summary")?;
    reader.skip(16, "Verse cell summary")?;
    reader.skip(4, "metadata summary offset")?;
    reader.skip(4, "dependency summary offset")?;
    reader.skip(8, "soft package reference summary")?;
    reader.skip(4, "searchable names offset")?;
    reader.skip(4, "thumbnail summary offset")?;
    reader.skip(8, "import type hierarchy summary")?;
    if !filter_editor_only {
        reader.skip(GUID_BYTES, "persistent package guid")?;
    }
    Ok(filter_editor_only)
}

fn read_tail(reader: &mut Reader<'_>) -> Result<Header, UnrealTexture2DMetadataError> {
    let generation_count = reader.read_count("generation", MAX_UNREAL_TEXTURE_METADATA_RECORDS)?;
    let generation_bytes = generation_count.checked_mul(8).ok_or(
        UnrealTexture2DMetadataError::InvalidStringLength {
            max_bytes: MAX_UNREAL_TEXTURE_METADATA_STRING_BYTES,
        },
    )?;
    reader.skip(generation_bytes, "generation records")?;
    validate_saved_by_version(reader)?;
    let _compatible_version = reader.read_engine_version("compatible engine version")?;
    reader.skip(4, "compression flags")?;
    reject_nonempty_count(
        reader,
        "compressed package chunk",
        UnrealTexture2DMetadataError::CompressedPackageChunks,
    )?;
    reader.skip(4, "package source")?;
    reject_nonempty_count(
        reader,
        "additional package",
        UnrealTexture2DMetadataError::CompressedPackageChunks,
    )?;
    let asset_registry_data_offset = reader.read_i32("asset registry data offset")?;
    if asset_registry_data_offset <= 0 {
        return Err(UnrealTexture2DMetadataError::MissingAssetRegistryData);
    }
    let bulk_data_start_offset = reader.read_i64("bulk data start offset")?;
    let bulk_data_start_offset = usize::try_from(bulk_data_start_offset).map_err(|_| {
        UnrealTexture2DMetadataError::InvalidOffset { field: "bulk data start", offset: i32::MAX }
    })?;
    reader.skip(4, "world tile info offset")?;
    reject_nonempty_count(
        reader,
        "chunk identifier",
        UnrealTexture2DMetadataError::CompressedPackageChunks,
    )?;
    reader.skip(8, "preload dependency summary")?;
    reader.skip(4, "names referenced from export data count")?;
    reader.skip(8, "payload toc offset")?;
    reader.skip(4, "data resource offset")?;
    Ok(Header { asset_registry_data_offset, bulk_data_start_offset })
}

fn validate_saved_by_version(reader: &mut Reader<'_>) -> Result<(), UnrealTexture2DMetadataError> {
    let saved_by_version = reader.read_engine_version("saved-by engine version")?;
    if saved_by_version != (UE5_MAJOR, UE5_MINOR, UE5_PATCH) {
        return Err(UnrealTexture2DMetadataError::UnsupportedEngineVersion {
            major: saved_by_version.0,
            minor: saved_by_version.1,
            patch: saved_by_version.2,
        });
    }
    Ok(())
}

fn reject_nonempty_count(
    reader: &mut Reader<'_>,
    field: &'static str,
    error: UnrealTexture2DMetadataError,
) -> Result<(), UnrealTexture2DMetadataError> {
    if reader.read_count(field, 0)? != 0 {
        return Err(error);
    }
    Ok(())
}

fn parse_asset_registry_record(
    reader: &mut Reader<'_>,
    expected_object_path: &str,
) -> Result<TextureFacts, UnrealTexture2DMetadataError> {
    let record_count =
        reader.read_count("asset registry record", MAX_UNREAL_TEXTURE_METADATA_RECORDS)?;
    if record_count != 1 {
        return Err(UnrealTexture2DMetadataError::AssetRegistryRecordCount {
            actual: i32::try_from(record_count).unwrap_or(i32::MAX),
        });
    }
    let object_path = reader.read_string("asset registry object path")?;
    if object_path != expected_object_path {
        return Err(UnrealTexture2DMetadataError::UnexpectedObjectPath { actual: object_path });
    }
    let asset_class = reader.read_string("asset registry class")?;
    if asset_class != "Texture2D" {
        return Err(UnrealTexture2DMetadataError::UnexpectedAssetClass { actual: asset_class });
    }
    let tag_count = reader.read_count("asset registry tag", MAX_UNREAL_TEXTURE_METADATA_TAGS)?;
    let mut tags = BTreeMap::new();
    for _ in 0..tag_count {
        let key = reader.read_string("asset registry tag name")?;
        let value = reader.read_string("asset registry tag value")?;
        if !matches!(
            key.as_str(),
            "Dimensions" | "PixelFormat" | "CompressionSettings" | "Filter" | "SourceFileHash"
        ) {
            return Err(UnrealTexture2DMetadataError::UnsupportedTag { tag: key });
        }
        if tags.insert(key.clone(), value).is_some() {
            return Err(UnrealTexture2DMetadataError::DuplicateTag { tag: key });
        }
    }
    TextureFacts::from_tags(&tags)
}

fn unreal_object_path(source: &SourceIdentity) -> Result<&str, UnrealTexture2DMetadataError> {
    let SourceIdentity::Unreal { object_path, .. } = source else {
        return Err(UnrealTexture2DMetadataError::NonUnrealSource);
    };
    Ok(object_path.as_str())
}

fn parse_dimensions(value: &str) -> Result<UnrealTextureDimensions, UnrealTexture2DMetadataError> {
    let (width, height) = value.split_once('x').ok_or_else(|| {
        UnrealTexture2DMetadataError::InvalidDimensions { value: value.to_owned() }
    })?;
    if height.contains('x') {
        return Err(UnrealTexture2DMetadataError::InvalidDimensions { value: value.to_owned() });
    }
    let width = width
        .parse::<u32>()
        .map_err(|_| UnrealTexture2DMetadataError::InvalidDimensions { value: value.to_owned() })?;
    let height = height
        .parse::<u32>()
        .map_err(|_| UnrealTexture2DMetadataError::InvalidDimensions { value: value.to_owned() })?;
    if width == 0
        || height == 0
        || width > MAX_UNREAL_TEXTURE_METADATA_DIMENSION
        || height > MAX_UNREAL_TEXTURE_METADATA_DIMENSION
    {
        return Err(UnrealTexture2DMetadataError::InvalidDimensions { value: value.to_owned() });
    }
    Ok(UnrealTextureDimensions { width, height })
}

fn parse_pixel_format(
    value: &str,
) -> Result<UnrealTexturePixelFormat, UnrealTexture2DMetadataError> {
    match value {
        "PF_B8G8R8A8" => Ok(UnrealTexturePixelFormat::B8G8R8A8),
        "PF_DXT1" => Ok(UnrealTexturePixelFormat::Dxt1),
        "PF_DXT5" => Ok(UnrealTexturePixelFormat::Dxt5),
        "PF_BC5" => Ok(UnrealTexturePixelFormat::Bc5),
        "PF_BC7" => Ok(UnrealTexturePixelFormat::Bc7),
        _ => Err(UnrealTexture2DMetadataError::UnsupportedPixelFormat { value: value.to_owned() }),
    }
}

fn parse_compression(
    value: &str,
) -> Result<UnrealTextureCompression, UnrealTexture2DMetadataError> {
    match value {
        "TC_Default" => Ok(UnrealTextureCompression::Default),
        "TC_Normalmap" => Ok(UnrealTextureCompression::NormalMap),
        "TC_Masks" => Ok(UnrealTextureCompression::Masks),
        "TC_Grayscale" => Ok(UnrealTextureCompression::Grayscale),
        _ => Err(UnrealTexture2DMetadataError::UnsupportedCompression { value: value.to_owned() }),
    }
}

fn parse_filter(value: &str) -> Result<UnrealTextureFilter, UnrealTexture2DMetadataError> {
    match value {
        "TF_Default" => Ok(UnrealTextureFilter::Default),
        "TF_Nearest" => Ok(UnrealTextureFilter::Nearest),
        "TF_Bilinear" => Ok(UnrealTextureFilter::Bilinear),
        "TF_Trilinear" => Ok(UnrealTextureFilter::Trilinear),
        _ => Err(UnrealTexture2DMetadataError::UnsupportedFilter { value: value.to_owned() }),
    }
}

fn parse_source_file_hash(value: &str) -> Result<String, UnrealTexture2DMetadataError> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(UnrealTexture2DMetadataError::InvalidSourceFileHash);
    }
    Ok(value.to_ascii_lowercase())
}

struct Reader<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    fn at(
        bytes: &'a [u8],
        offset: i32,
        field: &'static str,
    ) -> Result<Self, UnrealTexture2DMetadataError> {
        let position = usize::try_from(offset)
            .ok()
            .filter(|position| *position < bytes.len())
            .ok_or(UnrealTexture2DMetadataError::InvalidOffset { field, offset })?;
        Ok(Self { bytes, position })
    }

    fn read_u16(&mut self, field: &'static str) -> Result<u16, UnrealTexture2DMetadataError> {
        let bytes = self.take(2, field)?;
        let array = <[u8; 2]>::try_from(bytes)
            .map_err(|_| UnrealTexture2DMetadataError::Truncated { field })?;
        Ok(u16::from_le_bytes(array))
    }

    fn read_u32(&mut self, field: &'static str) -> Result<u32, UnrealTexture2DMetadataError> {
        let bytes = self.take(4, field)?;
        let array = <[u8; 4]>::try_from(bytes)
            .map_err(|_| UnrealTexture2DMetadataError::Truncated { field })?;
        Ok(u32::from_le_bytes(array))
    }

    fn read_i32(&mut self, field: &'static str) -> Result<i32, UnrealTexture2DMetadataError> {
        let bytes = self.take(4, field)?;
        let array = <[u8; 4]>::try_from(bytes)
            .map_err(|_| UnrealTexture2DMetadataError::Truncated { field })?;
        Ok(i32::from_le_bytes(array))
    }

    fn read_i64(&mut self, field: &'static str) -> Result<i64, UnrealTexture2DMetadataError> {
        let bytes = self.take(8, field)?;
        let array = <[u8; 8]>::try_from(bytes)
            .map_err(|_| UnrealTexture2DMetadataError::Truncated { field })?;
        Ok(i64::from_le_bytes(array))
    }

    fn read_count(
        &mut self,
        field: &'static str,
        max: usize,
    ) -> Result<usize, UnrealTexture2DMetadataError> {
        let actual = self.read_i32(field)?;
        let count = usize::try_from(actual)
            .map_err(|_| UnrealTexture2DMetadataError::CountLimit { field, actual, max })?;
        if count > max {
            return Err(UnrealTexture2DMetadataError::CountLimit { field, actual, max });
        }
        Ok(count)
    }

    fn read_engine_version(
        &mut self,
        field: &'static str,
    ) -> Result<(u16, u16, u16), UnrealTexture2DMetadataError> {
        let major = self.read_u16(field)?;
        let minor = self.read_u16(field)?;
        let patch = self.read_u16(field)?;
        self.skip(4, field)?;
        let _branch = self.read_string(field)?;
        Ok((major, minor, patch))
    }

    fn read_string(&mut self, field: &'static str) -> Result<String, UnrealTexture2DMetadataError> {
        let encoded_length = self.read_i32(field)?;
        if encoded_length == 0 {
            return Ok(String::new());
        }
        if encoded_length > 0 {
            let byte_length = usize::try_from(encoded_length).map_err(|_| {
                UnrealTexture2DMetadataError::InvalidStringLength {
                    max_bytes: MAX_UNREAL_TEXTURE_METADATA_STRING_BYTES,
                }
            })?;
            if byte_length > MAX_UNREAL_TEXTURE_METADATA_STRING_BYTES {
                return Err(UnrealTexture2DMetadataError::InvalidStringLength {
                    max_bytes: MAX_UNREAL_TEXTURE_METADATA_STRING_BYTES,
                });
            }
            let bytes = self.take(byte_length, field)?;
            let content = bytes
                .get(..byte_length.saturating_sub(1))
                .ok_or(UnrealTexture2DMetadataError::Truncated { field })?;
            if bytes.last() != Some(&0) {
                return Err(UnrealTexture2DMetadataError::InvalidString);
            }
            return std::str::from_utf8(content)
                .map(str::to_owned)
                .map_err(|_| UnrealTexture2DMetadataError::InvalidString);
        }
        let code_unit_count = encoded_length
            .checked_abs()
            .and_then(|length| usize::try_from(length).ok())
            .ok_or(UnrealTexture2DMetadataError::InvalidStringLength {
                max_bytes: MAX_UNREAL_TEXTURE_METADATA_STRING_BYTES,
            })?;
        let byte_length = code_unit_count.checked_mul(2).ok_or(
            UnrealTexture2DMetadataError::InvalidStringLength {
                max_bytes: MAX_UNREAL_TEXTURE_METADATA_STRING_BYTES,
            },
        )?;
        if byte_length > MAX_UNREAL_TEXTURE_METADATA_STRING_BYTES || code_unit_count == 0 {
            return Err(UnrealTexture2DMetadataError::InvalidStringLength {
                max_bytes: MAX_UNREAL_TEXTURE_METADATA_STRING_BYTES,
            });
        }
        let bytes = self.take(byte_length, field)?;
        let mut code_units = Vec::with_capacity(code_unit_count.saturating_sub(1));
        for chunk in bytes.chunks_exact(2) {
            let array = <[u8; 2]>::try_from(chunk)
                .map_err(|_| UnrealTexture2DMetadataError::Truncated { field })?;
            code_units.push(u16::from_le_bytes(array));
        }
        if code_units.last() != Some(&0) {
            return Err(UnrealTexture2DMetadataError::InvalidString);
        }
        let _terminator = code_units.pop();
        String::from_utf16(&code_units).map_err(|_| UnrealTexture2DMetadataError::InvalidString)
    }

    fn skip(
        &mut self,
        byte_count: usize,
        field: &'static str,
    ) -> Result<(), UnrealTexture2DMetadataError> {
        let _bytes = self.take(byte_count, field)?;
        Ok(())
    }

    fn take(
        &mut self,
        byte_count: usize,
        field: &'static str,
    ) -> Result<&'a [u8], UnrealTexture2DMetadataError> {
        let end = self
            .position
            .checked_add(byte_count)
            .ok_or(UnrealTexture2DMetadataError::Truncated { field })?;
        let bytes = self
            .bytes
            .get(self.position..end)
            .ok_or(UnrealTexture2DMetadataError::Truncated { field })?;
        self.position = end;
        Ok(bytes)
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawOracleReport {
    engine: String,
    engine_version: String,
    protocol_version: String,
    records: Vec<RawOracleRecord>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawOracleRecord {
    asset_kind: String,
    cooked: bool,
    object_path: String,
    tags: BTreeMap<String, String>,
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_UNREAL_TEXTURE_METADATA_BYTES, UnrealTexture2DMetadataDecoder,
        UnrealTexture2DMetadataError, UnrealTexturePixelFormat,
    };
    use crate::domain::{ProjectRoot, SourceAssetPath, SourceIdentity, UnrealObjectPath};

    fn source() -> Result<SourceIdentity, Box<dyn std::error::Error>> {
        Ok(SourceIdentity::Unreal {
            project_root: "texture-project".parse::<ProjectRoot>()?,
            asset_path: "Content/Textures/T_Fixture.uasset".parse::<SourceAssetPath>()?,
            object_path: "/Game/Textures/T_Fixture.T_Fixture".parse::<UnrealObjectPath>()?,
        })
    }

    #[test]
    fn malformed_package_bytes_fail_without_panicking() -> Result<(), Box<dyn std::error::Error>> {
        let decoder = UnrealTexture2DMetadataDecoder;
        let source = source()?;
        for length in 0_u8..=128 {
            let bytes = (0..usize::from(length))
                .map(|index| length.wrapping_add(u8::try_from(index).unwrap_or(0)))
                .collect::<Vec<_>>();
            let result = std::panic::catch_unwind(|| decoder.decode(&source, &bytes, None));
            assert!(result.is_ok());
        }
        Ok(())
    }

    #[test]
    fn unknown_or_missing_selected_metadata_is_rejected() -> Result<(), Box<dyn std::error::Error>>
    {
        let decoder = UnrealTexture2DMetadataDecoder;
        let source = source()?;
        let unknown_tag = fixture_package(&[("Unexpected", "value")], None)?;
        assert!(matches!(
            decoder.decode(&source, &unknown_tag, None),
            Err(UnrealTexture2DMetadataError::UnsupportedTag { .. })
        ));
        let missing_tag = fixture_package(&[("Dimensions", "32x32")], None)?;
        assert!(matches!(
            decoder.decode(&source, &missing_tag, None),
            Err(UnrealTexture2DMetadataError::MissingTag { tag: "PixelFormat" })
        ));
        Ok(())
    }

    fn fixture_package(
        tags: &[(&str, &str)],
        bulk_data_start_offset: Option<i64>,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let mut bytes = Vec::new();
        write_u32(&mut bytes, 0x9e2a_83c1);
        write_i32(&mut bytes, -9);
        write_i32(&mut bytes, 0);
        write_i32(&mut bytes, 522);
        write_i32(&mut bytes, 1018);
        write_i32(&mut bytes, 0);
        bytes.extend_from_slice(&[0_u8; 20]);
        let total_header_size_offset = bytes.len();
        write_i32(&mut bytes, 0);
        write_i32(&mut bytes, 0);
        write_string(&mut bytes, "/Game/Textures/T_Fixture")?;
        write_u32(&mut bytes, 0);
        bytes.extend_from_slice(&[0_u8; 8 + 8]);
        write_string(&mut bytes, "")?;
        bytes.extend_from_slice(&[0_u8; 8 + 8 + 8 + 16 + 4 + 4 + 8 + 4 + 4 + 8 + 16]);
        write_i32(&mut bytes, 0);
        write_engine_version(&mut bytes, 5, 8, 2)?;
        write_engine_version(&mut bytes, 5, 8, 2)?;
        bytes.extend_from_slice(&[0_u8; 4]);
        write_i32(&mut bytes, 0);
        bytes.extend_from_slice(&[0_u8; 4]);
        write_i32(&mut bytes, 0);
        let registry_offset_position = bytes.len();
        write_i32(&mut bytes, 0);
        let bulk_offset_position = bytes.len();
        write_i64(&mut bytes, 0);
        bytes.extend_from_slice(&[0_u8; 4]);
        write_i32(&mut bytes, 0);
        bytes.extend_from_slice(&[0_u8; 8 + 4 + 8 + 4]);
        let registry_offset = i32::try_from(bytes.len())?;
        write_i32(&mut bytes, 1);
        write_string(&mut bytes, "/Game/Textures/T_Fixture.T_Fixture")?;
        write_string(&mut bytes, "Texture2D")?;
        write_i32(&mut bytes, i32::try_from(tags.len())?);
        for (key, value) in tags {
            write_string(&mut bytes, key)?;
            write_string(&mut bytes, value)?;
        }
        let total_header_size = registry_offset;
        write_i32_at(&mut bytes, total_header_size_offset, total_header_size);
        write_i32_at(&mut bytes, registry_offset_position, registry_offset);
        let default_bulk_offset = i64::try_from(bytes.len())?;
        write_i64_at(
            &mut bytes,
            bulk_offset_position,
            bulk_data_start_offset.unwrap_or(default_bulk_offset),
        );
        Ok(bytes)
    }

    fn write_engine_version(
        bytes: &mut Vec<u8>,
        major: u16,
        minor: u16,
        patch: u16,
    ) -> Result<(), Box<dyn std::error::Error>> {
        bytes.extend_from_slice(&major.to_le_bytes());
        bytes.extend_from_slice(&minor.to_le_bytes());
        bytes.extend_from_slice(&patch.to_le_bytes());
        write_u32(bytes, 0);
        write_string(bytes, "++UE5+Release-5.8")
    }

    fn write_string(bytes: &mut Vec<u8>, value: &str) -> Result<(), Box<dyn std::error::Error>> {
        let length = i32::try_from(value.len() + 1)?;
        write_i32(bytes, length);
        bytes.extend_from_slice(value.as_bytes());
        bytes.push(0);
        Ok(())
    }

    fn write_u32(bytes: &mut Vec<u8>, value: u32) {
        bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn write_i32(bytes: &mut Vec<u8>, value: i32) {
        bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn write_i64(bytes: &mut Vec<u8>, value: i64) {
        bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn write_u32_at(bytes: &mut [u8], offset: usize, value: u32) {
        for (slot, byte) in bytes.iter_mut().skip(offset).zip(value.to_le_bytes()) {
            *slot = byte;
        }
    }

    fn write_i32_at(bytes: &mut [u8], offset: usize, value: i32) {
        for (slot, byte) in bytes.iter_mut().skip(offset).zip(value.to_le_bytes()) {
            *slot = byte;
        }
    }

    fn write_i64_at(bytes: &mut [u8], offset: usize, value: i64) {
        for (slot, byte) in bytes.iter_mut().skip(offset).zip(value.to_le_bytes()) {
            *slot = byte;
        }
    }

    #[test]
    fn decoder_matches_independent_authorized_oracle() -> Result<(), Box<dyn std::error::Error>> {
        let source = source()?;
        let tags = [
            ("Dimensions", "256x128"),
            ("PixelFormat", "PF_B8G8R8A8"),
            ("CompressionSettings", "TC_Default"),
            ("Filter", "TF_Trilinear"),
            ("SourceFileHash", "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"),
        ];
        let package = fixture_package(&tags, None)?;
        let decoder = UnrealTexture2DMetadataDecoder;
        let metadata = decoder.decode(&source, &package, None)?;
        let oracle = decoder.parse_oracle_text(&source, &oracle_text())?;
        assert_eq!(metadata.pixel_format, UnrealTexturePixelFormat::B8G8R8A8);
        assert!(oracle.matches(&metadata));
        Ok(())
    }

    #[test]
    fn external_bulk_requires_a_nonempty_companion() -> Result<(), Box<dyn std::error::Error>> {
        let source = source()?;
        let tags = required_tags();
        let package = fixture_package(&tags, Some(1_000_000))?;
        let decoder = UnrealTexture2DMetadataDecoder;
        assert!(matches!(
            decoder.decode(&source, &package, None),
            Err(UnrealTexture2DMetadataError::MissingExternalBulkCompanion)
        ));
        assert!(decoder.decode(&source, &package, Some(b"metadata-only")).is_ok());
        Ok(())
    }

    #[test]
    fn unsupported_ue5_version_is_rejected_before_tag_reading()
    -> Result<(), Box<dyn std::error::Error>> {
        let source = source()?;
        let mut package = fixture_package(&required_tags(), None)?;
        write_i32_at(&mut package, 16, 1017);
        let decoder = UnrealTexture2DMetadataDecoder;
        assert!(matches!(
            decoder.decode(&source, &package, None),
            Err(UnrealTexture2DMetadataError::UnsupportedPackageVersion { .. })
        ));
        Ok(())
    }

    #[test]
    fn package_size_limit_accepts_the_boundary_and_rejects_one_extra_byte()
    -> Result<(), Box<dyn std::error::Error>> {
        let source = source()?;
        let mut boundary = fixture_package(&required_tags(), None)?;
        boundary.resize(MAX_UNREAL_TEXTURE_METADATA_BYTES, 0);
        let decoder = UnrealTexture2DMetadataDecoder;
        assert!(decoder.decode(&source, &boundary, None).is_ok());
        boundary.push(0);
        assert!(matches!(
            decoder.decode(&source, &boundary, None),
            Err(UnrealTexture2DMetadataError::PackageTooLarge { .. })
        ));
        Ok(())
    }

    #[test]
    fn cooked_package_is_rejected_before_asset_registry_decode()
    -> Result<(), Box<dyn std::error::Error>> {
        let source = source()?;
        let mut package = fixture_package(&required_tags(), None)?;
        let package_flags_offset = 52 + 4 + "/Game/Textures/T_Fixture".len() + 1;
        write_u32_at(&mut package, package_flags_offset, 0x0000_0200);
        let decoder = UnrealTexture2DMetadataDecoder;
        assert!(matches!(
            decoder.decode(&source, &package, None),
            Err(UnrealTexture2DMetadataError::CookedPackage)
        ));
        Ok(())
    }

    fn required_tags() -> [(&'static str, &'static str); 3] {
        [("Dimensions", "64x64"), ("PixelFormat", "PF_DXT5"), ("CompressionSettings", "TC_Masks")]
    }

    fn oracle_text() -> String {
        r#"{
  "engine": "unreal",
  "engine_version": "5.8.2",
  "protocol_version": "vortex.unreal-texture-metadata/v1",
  "records": [
    {
      "asset_kind": "Texture2D",
      "cooked": false,
      "object_path": "/Game/Textures/T_Fixture.T_Fixture",
      "tags": {
        "CompressionSettings": "TC_Default",
        "Dimensions": "256x128",
        "Filter": "TF_Trilinear",
        "PixelFormat": "PF_B8G8R8A8",
        "SourceFileHash": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
      }
    }
  ]
}"#
        .to_owned()
    }
}
