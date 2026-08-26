//! Bounded USDA stage inspection with explicit composition limits.
//!
//! This initial adapter accepts only text USDA input and records selected stage
//! facts deterministically. It does not claim full `OpenUSD` composition semantics;
//! references, payloads, and unselected variants remain explicit adapter facts.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    str::FromStr,
};

use thiserror::Error;

use crate::domain::{
    ClassificationKind, ConversionClassification, DomainError, Remediation, SourceAssetPath,
    SourceIdentity, StageSelection, UsdPrimPath, identity::IdentityError,
};

const MAX_USDA_BYTES: u64 = 4 * 1024 * 1024;
const USDA_MAGIC: &str = "#usda 1.0";
const DEFAULT_METERS_PER_UNIT: &str = "0.01";
const DEFAULT_UP_AXIS: &str = "Y";

/// Payload behavior selected for the bounded initial USDA adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UsdPayloadPolicy {
    /// Do not resolve or load payload arcs; preserve them as unselected facts.
    Forbid,
    /// Preserve payload-arc metadata without resolving payload content.
    MetadataOnly,
}

impl UsdPayloadPolicy {
    const fn label(self) -> &'static str {
        match self {
            Self::Forbid => "forbid",
            Self::MetadataOnly => "metadata_only",
        }
    }
}

/// Explicit selected-stage options parsed at the source adapter boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsdStageOptions {
    /// Deterministically sorted variant-set selections.
    pub variants: BTreeMap<String, String>,
    /// Selected payload handling behavior.
    pub payload_policy: UsdPayloadPolicy,
    /// Requested imageable purpose.
    pub purpose: String,
    /// Optional inclusive integer time-code range.
    pub time_range: Option<(i64, i64)>,
    /// Requested render context token.
    pub render_context: String,
}

impl Default for UsdStageOptions {
    fn default() -> Self {
        Self {
            variants: BTreeMap::new(),
            payload_policy: UsdPayloadPolicy::Forbid,
            purpose: "render".to_owned(),
            time_range: None,
            render_context: "universal".to_owned(),
        }
    }
}

/// Failures emitted by bounded stage selection parsing and USDA resolution.
#[derive(Debug, Error)]
pub enum UsdStageError {
    /// The supplied selected-stage expression does not follow the bounded grammar.
    #[error("invalid USD stage selection: {reason}")]
    InvalidSelection { reason: String },
    /// The configured stage root is unavailable.
    #[error("USD stage root is unavailable: {source}")]
    RootUnavailable { source: std::io::Error },
    /// The configured root is not a directory.
    #[error("USD stage root is not a directory: {path}")]
    RootNotDirectory { path: PathBuf },
    /// The requested root-layer path is unsafe or leaves the authorized root.
    #[error("USD stage path escapes the authorized root: {path}")]
    PathEscape { path: PathBuf },
    /// The selected root-layer file is unavailable.
    #[error("USD root layer is unavailable: {source}")]
    StageUnavailable { source: std::io::Error },
    /// The selected layer has an unsupported extension.
    #[error("USD root layer must use .usda, .usdc, or .usd: {path}")]
    UnsupportedExtension { path: PathBuf },
    /// The selected USDA input exceeds the bounded text-reader size.
    #[error("USD root layer exceeds bounded size limit: {actual_bytes} bytes")]
    StageTooLarge { actual_bytes: u64 },
    /// The selected USDA input is not valid UTF-8.
    #[error("USDA root layer is not valid UTF-8: {source}")]
    InvalidUtf8 { source: std::string::FromUtf8Error },
    /// The stage does not start with the supported USDA header.
    #[error("USDA root layer does not start with the supported header")]
    InvalidHeader,
    /// Parentheses or braces are structurally unbalanced.
    #[error("USDA root layer has malformed structure: {reason}")]
    MalformedStructure { reason: String },
    /// Root metadata specified an invalid stage metric or axis.
    #[error("USDA root metadata is invalid: {reason}")]
    InvalidMetadata { reason: String },
    /// A typed stage or prim identity could not be created.
    #[error("USD stage resolution could not create a typed source identity: {source}")]
    Identity { source: IdentityError },
    /// A typed classification could not be created.
    #[error("USD stage resolution could not create a domain value: {source}")]
    Domain { source: DomainError },
}

/// A physical USDA root layer plus normalized selected-stage options.
#[derive(Clone, Debug)]
pub struct UsdStageRequest {
    root_directory: PathBuf,
    stage_path: SourceAssetPath,
    selection: StageSelection,
    options: UsdStageOptions,
}

impl UsdStageRequest {
    /// Creates a root-confined request for one explicit USDA/USDC/USD stage.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the root is invalid, the stage path escapes the
    /// root, or the selected-stage expression is malformed.
    pub fn new(
        root_directory: impl AsRef<Path>,
        stage_path: SourceAssetPath,
        selection: impl AsRef<str>,
    ) -> Result<Self, UsdStageError> {
        let root_directory = fs::canonicalize(root_directory)
            .map_err(|source| UsdStageError::RootUnavailable { source })?;
        if !root_directory.is_dir() {
            return Err(UsdStageError::RootNotDirectory { path: root_directory });
        }
        let options = selection.as_ref().parse::<UsdStageOptions>()?;
        let selection = canonical_selection(&options)?;
        Ok(Self { root_directory, stage_path, selection, options })
    }

    /// Returns the normalized typed source selection retained in resolved identities.
    #[must_use]
    pub const fn selection(&self) -> &StageSelection {
        &self.selection
    }
}

/// A bounded, owned prim fact copied from the initial USDA adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsdPrimSnapshot {
    /// Canonical absolute USD prim path.
    pub path: UsdPrimPath,
    /// Authored specifier such as `def`, `over`, or `class`.
    pub specifier: String,
    /// Optional authored schema type token.
    pub type_name: Option<String>,
    /// Imageable purpose or inherited selection default represented as an owned token.
    pub purpose: Option<String>,
    /// Explicit visibility token, if authored.
    pub visibility: Option<String>,
    /// Whether the prim was explicitly marked instancable.
    pub instancable: bool,
    /// Owned authored transform operation strings in source order.
    pub transforms: Vec<String>,
}

/// Deterministic source facts for one selected root-layer configuration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsdStageSnapshot {
    /// Typed root source identity with the normalized selected-stage expression.
    pub source: SourceIdentity,
    /// Explicit eligibility for the bounded adapter result.
    pub classification: ConversionClassification,
    /// Lexically sorted facts capturing selection, metadata, and unresolved arcs.
    pub facts: Vec<String>,
    /// Prim snapshots sorted by canonical prim path.
    pub prims: Vec<UsdPrimSnapshot>,
}

/// Isolated, bounded source adapter for initial USDA stage inspection.
#[derive(Clone, Copy, Debug, Default)]
pub struct UsdStageInspector;

impl UsdStageInspector {
    /// Resolves one selected stage into owned, deterministic USDA source facts.
    ///
    /// # Errors
    ///
    /// Returns typed errors for root escape, unsupported/malformed input, invalid
    /// metadata, or malformed selected-stage expressions. It never partially
    /// publishes a stage snapshot on error.
    pub fn resolve(&self, request: &UsdStageRequest) -> Result<UsdStageSnapshot, UsdStageError> {
        let stage_file = canonical_stage_file(request)?;
        let extension = stage_file
            .extension()
            .and_then(|value| value.to_str())
            .map(str::to_ascii_lowercase)
            .ok_or_else(|| UsdStageError::UnsupportedExtension { path: stage_file.clone() })?;
        if !matches!(extension.as_str(), "usda" | "usdc" | "usd") {
            return Err(UsdStageError::UnsupportedExtension { path: stage_file });
        }
        let metadata = fs::metadata(&stage_file)
            .map_err(|source| UsdStageError::StageUnavailable { source })?;
        if metadata.len() > MAX_USDA_BYTES {
            return Err(UsdStageError::StageTooLarge { actual_bytes: metadata.len() });
        }
        let bytes =
            fs::read(&stage_file).map_err(|source| UsdStageError::StageUnavailable { source })?;
        let is_text_usda = extension == "usda" || bytes.starts_with(USDA_MAGIC.as_bytes());
        if !is_text_usda {
            return unsupported_binary_snapshot(request, metadata.len(), &extension);
        }
        let text =
            String::from_utf8(bytes).map_err(|source| UsdStageError::InvalidUtf8 { source })?;
        parse_usda(request, metadata.len(), &text)
    }
}

impl FromStr for UsdStageOptions {
    type Err = UsdStageError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let mut options = Self::default();
        if value.trim().is_empty() {
            return Ok(options);
        }
        let mut seen = BTreeSet::new();
        for segment in value.split(';') {
            let (key, raw_value) =
                segment.split_once('=').ok_or_else(|| UsdStageError::InvalidSelection {
                    reason: format!("selection segment must use key=value: {segment}"),
                })?;
            if !seen.insert(key) {
                return Err(UsdStageError::InvalidSelection {
                    reason: format!("selection key appears more than once: {key}"),
                });
            }
            match key {
                "variants" => parse_variants(raw_value, &mut options.variants)?,
                "payload" => {
                    options.payload_policy = match raw_value {
                        "forbid" => UsdPayloadPolicy::Forbid,
                        "metadata_only" => UsdPayloadPolicy::MetadataOnly,
                        _ => {
                            return Err(UsdStageError::InvalidSelection {
                                reason: format!("unsupported payload policy: {raw_value}"),
                            });
                        }
                    };
                }
                "purpose" => options.purpose = parse_token("purpose", raw_value)?,
                "time" => options.time_range = parse_time_range(raw_value)?,
                "context" => options.render_context = parse_token("context", raw_value)?,
                _ => {
                    return Err(UsdStageError::InvalidSelection {
                        reason: format!("unsupported selection key: {key}"),
                    });
                }
            }
        }
        Ok(options)
    }
}

fn parse_variants(
    value: &str,
    variants: &mut BTreeMap<String, String>,
) -> Result<(), UsdStageError> {
    if value.is_empty() {
        return Ok(());
    }
    for entry in value.split(',') {
        let (name, selection) =
            entry.split_once(':').ok_or_else(|| UsdStageError::InvalidSelection {
                reason: format!("variant must use name:selection: {entry}"),
            })?;
        let name = parse_token("variant name", name)?;
        let selection = parse_token("variant selection", selection)?;
        if variants.insert(name.clone(), selection).is_some() {
            return Err(UsdStageError::InvalidSelection {
                reason: format!("variant appears more than once: {name}"),
            });
        }
    }
    Ok(())
}

fn parse_token(label: &str, value: &str) -> Result<String, UsdStageError> {
    let valid = !value.is_empty()
        && value.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-');
    if !valid {
        return Err(UsdStageError::InvalidSelection {
            reason: format!("{label} must be a nonempty ASCII token"),
        });
    }
    Ok(value.to_owned())
}

fn parse_time_range(value: &str) -> Result<Option<(i64, i64)>, UsdStageError> {
    if value == "stage" {
        return Ok(None);
    }
    let (start, end) = value.split_once(':').ok_or_else(|| UsdStageError::InvalidSelection {
        reason: "time must be stage or start:end".to_owned(),
    })?;
    let start = start.parse::<i64>().map_err(|_| UsdStageError::InvalidSelection {
        reason: "time start must be an integer".to_owned(),
    })?;
    let end = end.parse::<i64>().map_err(|_| UsdStageError::InvalidSelection {
        reason: "time end must be an integer".to_owned(),
    })?;
    if end < start {
        return Err(UsdStageError::InvalidSelection {
            reason: "time end must not precede time start".to_owned(),
        });
    }
    Ok(Some((start, end)))
}

fn canonical_selection(options: &UsdStageOptions) -> Result<StageSelection, UsdStageError> {
    let variants = options
        .variants
        .iter()
        .map(|(name, selection)| format!("{name}:{selection}"))
        .collect::<Vec<_>>()
        .join(",");
    let time = options
        .time_range
        .map_or_else(|| "stage".to_owned(), |(start, end)| format!("{start}:{end}"));
    format!(
        "context={};payload={};purpose={};time={time};variants={variants}",
        options.render_context,
        options.payload_policy.label(),
        options.purpose,
    )
    .parse::<StageSelection>()
    .map_err(|source| UsdStageError::Identity { source })
}

fn canonical_stage_file(request: &UsdStageRequest) -> Result<PathBuf, UsdStageError> {
    if request.stage_path.as_str().split('/').any(|component| component == "." || component == "..")
    {
        return Err(UsdStageError::PathEscape { path: PathBuf::from(request.stage_path.as_str()) });
    }
    let candidate = request.root_directory.join(request.stage_path.as_str());
    let stage_file =
        fs::canonicalize(candidate).map_err(|source| UsdStageError::StageUnavailable { source })?;
    if !stage_file.starts_with(&request.root_directory) {
        return Err(UsdStageError::PathEscape { path: stage_file });
    }
    Ok(stage_file)
}

fn unsupported_binary_snapshot(
    request: &UsdStageRequest,
    bytes: u64,
    extension: &str,
) -> Result<UsdStageSnapshot, UsdStageError> {
    let source = root_source_identity(request)?;
    let classification = ConversionClassification::new(
        ClassificationKind::RequiresAdapter,
        "The selected USD layer is binary or plugin-backed and requires an authorized OpenUSD runtime adapter.",
        Remediation::new(
            "Convert the approved stage to USDA with a matching OpenUSD tool or provide an approved runtime adapter.",
        )
        .map_err(|source| UsdStageError::Domain { source })?,
    )
    .map_err(|source| UsdStageError::Domain { source })?;
    let mut facts = selection_facts(&request.options);
    facts.extend([
        format!("stage.bytes={bytes}"),
        format!("stage.extension={extension}"),
        "stage.composition=requires_authoritative_runtime".to_owned(),
    ]);
    facts.sort();
    Ok(UsdStageSnapshot { source, classification, facts, prims: Vec::new() })
}

fn parse_usda(
    request: &UsdStageRequest,
    bytes: u64,
    text: &str,
) -> Result<UsdStageSnapshot, UsdStageError> {
    if !text.starts_with(USDA_MAGIC) {
        return Err(UsdStageError::InvalidHeader);
    }
    let mut parser = UsdaParser::new();
    for raw_line in text.lines() {
        parser.consume_line(raw_line)?;
    }
    parser.finish()?;

    let meters_per_unit = parser
        .meters_per_unit
        .as_deref()
        .unwrap_or(DEFAULT_METERS_PER_UNIT)
        .parse::<f64>()
        .map_err(|_| UsdStageError::InvalidMetadata {
            reason: "metersPerUnit must be a positive finite number".to_owned(),
        })?;
    if !meters_per_unit.is_finite() || meters_per_unit <= 0.0 {
        return Err(UsdStageError::InvalidMetadata {
            reason: "metersPerUnit must be a positive finite number".to_owned(),
        });
    }
    let up_axis = parser.up_axis.as_deref().unwrap_or(DEFAULT_UP_AXIS);
    if !matches!(up_axis, "Y" | "Z") {
        return Err(UsdStageError::InvalidMetadata { reason: "upAxis must be Y or Z".to_owned() });
    }

    let source = root_source_identity(request)?;
    let has_unresolved_composition = !parser.references.is_empty()
        || !parser.payloads.is_empty()
        || !parser.variant_sets.is_empty()
        || !parser.sub_layers.is_empty();
    let classification = ConversionClassification::new(
        ClassificationKind::RequiresAdapter,
        if has_unresolved_composition {
            "USDA metadata and selected configuration were resolved, while composition arcs remain explicit adapter facts."
        } else {
            "USDA metadata and selected configuration were resolved; canonical scene conversion requires the recovered IR adapter."
        },
        Remediation::new(
            "Use the next canonical IR adapter for portable stage content, or an approved OpenUSD runtime adapter for full composition semantics.",
        )
        .map_err(|source| UsdStageError::Domain { source })?,
    )
    .map_err(|source| UsdStageError::Domain { source })?;

    let mut facts = selection_facts(&request.options);
    facts.extend([
        format!("stage.bytes={bytes}"),
        "stage.extension=usda".to_owned(),
        format!("stage.meters_per_unit={meters_per_unit:.12}"),
        format!("stage.up_axis={up_axis}"),
        format!("normalization.meters_per_unit={meters_per_unit:.12}"),
        format!(
            "normalization.up_axis={}",
            if up_axis == "Y" { "identity" } else { "z_to_y_pending_ir" }
        ),
    ]);
    facts
        .extend(parser.sub_layers.into_iter().map(|layer| format!("composition.sublayer={layer}")));
    facts.extend(
        parser.references.into_iter().map(|reference| format!("composition.reference={reference}")),
    );
    facts.extend(
        parser
            .payloads
            .into_iter()
            .map(|payload| format!("composition.payload.unselected={payload}")),
    );
    facts.extend(
        parser
            .variant_sets
            .into_iter()
            .map(|variant| format!("composition.variant.unselected={variant}")),
    );
    facts.sort();
    facts.dedup();

    let mut prims = parser.prims;
    prims.sort_by_key(|prim| prim.path.as_str().to_owned());
    Ok(UsdStageSnapshot { source, classification, facts, prims })
}

fn root_source_identity(request: &UsdStageRequest) -> Result<SourceIdentity, UsdStageError> {
    Ok(SourceIdentity::Usd {
        stage: request.stage_path.clone(),
        prim_path: "/"
            .parse::<UsdPrimPath>()
            .map_err(|source| UsdStageError::Identity { source })?,
        selection: request.selection.clone(),
    })
}

fn selection_facts(options: &UsdStageOptions) -> Vec<String> {
    let mut facts = vec![
        format!("selection.payload={}", options.payload_policy.label()),
        format!("selection.purpose={}", options.purpose),
        format!("selection.render_context={}", options.render_context),
    ];
    match options.time_range {
        Some((start, end)) => facts.push(format!("selection.time={start}:{end}")),
        None => facts.push("selection.time=stage".to_owned()),
    }
    facts.extend(
        options
            .variants
            .iter()
            .map(|(name, selection)| format!("selection.variant.{name}={selection}")),
    );
    facts
}

#[derive(Default)]
struct UsdaParser {
    meters_per_unit: Option<String>,
    up_axis: Option<String>,
    sub_layers: BTreeSet<String>,
    references: BTreeSet<String>,
    payloads: BTreeSet<String>,
    variant_sets: BTreeSet<String>,
    prims: Vec<UsdPrimSnapshot>,
    open_scopes: Vec<Option<usize>>,
    pending_prim_scope: Option<usize>,
    parentheses_depth: i32,
}

impl UsdaParser {
    fn new() -> Self {
        Self::default()
    }

    fn consume_line(&mut self, raw_line: &str) -> Result<(), UsdStageError> {
        let line = raw_line.split('#').next().map_or("", str::trim);
        if line.is_empty() {
            return Ok(());
        }
        self.track_parentheses(line)?;
        self.record_stage_metadata(line);
        self.record_composition_facts(line);
        self.open_scope_for_line(line)?;
        self.record_active_prim_fields(line);
        self.close_scopes_for_line(line)
    }

    fn track_parentheses(&mut self, line: &str) -> Result<(), UsdStageError> {
        self.parentheses_depth += count_char(line, '(') - count_char(line, ')');
        if self.parentheses_depth < 0 {
            return Err(UsdStageError::MalformedStructure {
                reason: "closing metadata parenthesis without an opening parenthesis".to_owned(),
            });
        }
        Ok(())
    }

    fn record_stage_metadata(&mut self, line: &str) {
        if let Some(value) = metadata_value(line, "metersPerUnit") {
            self.meters_per_unit = Some(value.to_owned());
        }
        if let Some(value) = quoted_metadata_value(line, "upAxis") {
            self.up_axis = Some(value.to_owned());
        }
    }

    fn record_composition_facts(&mut self, line: &str) {
        self.sub_layers.extend(asset_paths_after(line, "subLayers"));
        self.references.extend(asset_paths_after(line, "references"));
        self.payloads.extend(asset_paths_after(line, "payload"));
        if let Some(variant) = quoted_after(line, "variantSet") {
            self.variant_sets.insert(variant.to_owned());
        }
    }

    fn open_scope_for_line(&mut self, line: &str) -> Result<(), UsdStageError> {
        if let Some((specifier, type_name, name)) = prim_declaration(line) {
            self.declare_prim(specifier, type_name, name, count_char(line, '{'))?;
        } else {
            self.open_nonprim_scopes(count_char(line, '{'));
        }
        Ok(())
    }

    fn declare_prim(
        &mut self,
        specifier: &str,
        type_name: Option<&str>,
        name: &str,
        opening_braces: i32,
    ) -> Result<(), UsdStageError> {
        let parent_path = self
            .open_scopes
            .iter()
            .rev()
            .find_map(|scope| scope.and_then(|index| self.prims.get(index)))
            .map_or("", |prim| prim.path.as_str());
        let path = format!("{parent_path}/{name}");
        let path =
            path.parse::<UsdPrimPath>().map_err(|source| UsdStageError::Identity { source })?;
        self.prims.push(UsdPrimSnapshot {
            path,
            specifier: specifier.to_owned(),
            type_name: type_name.map(str::to_owned),
            purpose: None,
            visibility: None,
            instancable: false,
            transforms: Vec::new(),
        });
        let index = self.prims.len() - 1;
        if opening_braces > 0 {
            self.open_scopes.push(Some(index));
            self.open_additional_scopes(opening_braces);
        } else {
            self.pending_prim_scope = Some(index);
        }
        Ok(())
    }

    fn open_nonprim_scopes(&mut self, opening_braces: i32) {
        if opening_braces <= 0 {
            return;
        }
        if let Some(index) = self.pending_prim_scope.take() {
            self.open_scopes.push(Some(index));
            self.open_additional_scopes(opening_braces);
        } else {
            for _ in 0..opening_braces {
                self.open_scopes.push(None);
            }
        }
    }

    fn open_additional_scopes(&mut self, opening_braces: i32) {
        for _ in 1..opening_braces {
            self.open_scopes.push(None);
        }
    }

    fn record_active_prim_fields(&mut self, line: &str) {
        let Some(index) = self.open_scopes.iter().rev().find_map(|scope| *scope) else {
            return;
        };
        let Some(prim) = self.prims.get_mut(index) else {
            return;
        };
        if let Some(value) = quoted_assignment_value(line, "purpose") {
            prim.purpose = Some(value.to_owned());
        }
        if let Some(value) = quoted_assignment_value(line, "visibility") {
            prim.visibility = Some(value.to_owned());
        }
        if line.contains("instanceable = true") {
            prim.instancable = true;
        }
        if line.contains("xformOp:") {
            prim.transforms.push(line.to_owned());
        }
    }

    fn close_scopes_for_line(&mut self, line: &str) -> Result<(), UsdStageError> {
        for _ in 0..count_char(line, '}') {
            if self.open_scopes.pop().is_none() {
                return Err(UsdStageError::MalformedStructure {
                    reason: "closing brace without an opening scope".to_owned(),
                });
            }
        }
        Ok(())
    }

    fn finish(&self) -> Result<(), UsdStageError> {
        if self.parentheses_depth != 0 {
            return Err(UsdStageError::MalformedStructure {
                reason: "unclosed metadata parentheses".to_owned(),
            });
        }
        if self.pending_prim_scope.is_some() || !self.open_scopes.is_empty() {
            return Err(UsdStageError::MalformedStructure {
                reason: "unclosed USDA prim or brace scope".to_owned(),
            });
        }
        Ok(())
    }
}

fn count_char(line: &str, target: char) -> i32 {
    i32::try_from(line.chars().filter(|character| *character == target).count())
        .map_or(i32::MAX, |count| count)
}

fn metadata_value<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    Some(line.strip_prefix(key)?.trim_start().strip_prefix('=')?.trim())
}

fn quoted_metadata_value<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let value = metadata_value(line, key)?;
    value.strip_prefix('"')?.strip_suffix('"')
}

fn quoted_assignment_value<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let (_, after_key) = line.split_once(key)?;
    let value = after_key.trim_start().strip_prefix('=')?.trim();
    value.strip_prefix('"')?.strip_suffix('"')
}

fn asset_paths_after(line: &str, key: &str) -> Vec<String> {
    if !line.contains(key) {
        return Vec::new();
    }
    let mut values = Vec::new();
    let mut remainder = line;
    while let Some(start) = remainder.find('@') {
        let Some(after_start) = remainder.get(start.saturating_add(1)..) else {
            break;
        };
        let Some(end) = after_start.find('@') else {
            break;
        };
        let Some(value) = after_start.get(..end) else {
            break;
        };
        if !value.is_empty() {
            values.push(value.to_owned());
        }
        let Some(next) = after_start.get(end.saturating_add(1)..) else {
            break;
        };
        remainder = next;
    }
    values
}

fn quoted_after<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let remainder = line.strip_prefix(key)?.trim_start();
    let value = remainder.strip_prefix('"')?;
    value.split_once('"').map(|(value, _)| value)
}

fn prim_declaration(line: &str) -> Option<(&str, Option<&str>, &str)> {
    let mut words = line.split_whitespace();
    let specifier = words.next()?;
    if !matches!(specifier, "def" | "over" | "class") {
        return None;
    }
    let next = words.next()?;
    let (type_name, quoted_name) =
        if next.starts_with('"') { (None, next) } else { (Some(next), words.next()?) };
    let name = quoted_name.strip_prefix('"')?.strip_suffix('"')?;
    if name.is_empty() {
        return None;
    }
    Some((specifier, type_name, name))
}
