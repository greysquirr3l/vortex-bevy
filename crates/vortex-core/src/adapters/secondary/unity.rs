//! Bounded Unity text-project and URP source inspection.
//!
//! The adapter accepts caller-selected editable project files with visible `.meta`
//! identities and text serialization. It records owned source facts only; binary
//! assets and behavior-rich engine features remain explicit adapter requirements.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use serde_json::Value;
use thiserror::Error;

use crate::domain::{
    ClassificationKind, ConversionClassification, DomainError, ProjectRoot, Remediation,
    SourceAssetPath, SourceIdentity, UnityAssetGuid, UnityObjectId,
    identity::IdentityError,
    scene_ir::{SourceLink, SourceLinkId},
};

const MAX_TEXT_BYTES: u64 = 2 * 1024 * 1024;
const TEXT_HEADER: &str = "%YAML";
const SUPPORTED_EDITOR_PREFIXES: [&str; 2] = ["2022.3.", "6000."];

/// A source type accepted by the bounded Unity adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnitySourceKind {
    /// A text-serialized Unity scene.
    Scene,
    /// A text-serialized Unity prefab.
    Prefab,
    /// A text-serialized Unity material.
    Material,
    /// An imported texture asset with visible meta identity.
    Texture,
    /// An imported mesh asset with visible meta identity.
    Mesh,
}

impl UnitySourceKind {
    const fn label(self) -> &'static str {
        match self {
            Self::Scene => "scene",
            Self::Prefab => "prefab",
            Self::Material => "material",
            Self::Texture => "texture",
            Self::Mesh => "mesh",
        }
    }
}

/// Versioned project facts retained with every resolved Unity source record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnityProjectProvenance {
    /// Matching Unity Editor version from `ProjectVersion.txt`.
    pub editor_version: String,
    /// URP package version from `Packages/manifest.json`.
    pub urp_package_version: String,
    /// Required text serialization mode from `EditorSettings.asset`.
    pub serialization_mode: String,
}

/// One owned Unity source record with its classification and parsed facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnitySourceRecord {
    /// Typed Unity source identity.
    pub source: SourceIdentity,
    /// Bounded source kind.
    pub kind: UnitySourceKind,
    /// Actionable source eligibility.
    pub classification: ConversionClassification,
    /// Deterministically ordered parser facts.
    pub facts: BTreeMap<String, String>,
}

/// Deterministic result of inspecting an authorized selected Unity source set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnityProjectSnapshot {
    /// Editor, URP, and serialization facts shared by all selected records.
    pub provenance: UnityProjectProvenance,
    /// Source records sorted by typed source identity.
    pub records: Vec<UnitySourceRecord>,
}

impl UnityProjectSnapshot {
    /// Converts source records into the canonical scene IR provenance boundary.
    ///
    /// # Errors
    ///
    /// Returns a typed error only if the selected record count cannot fit into the
    /// IR’s stable `u32` source-link identifier space.
    pub fn source_links(&self) -> Result<Vec<SourceLink>, UnityProjectError> {
        self.records
            .iter()
            .enumerate()
            .map(|(index, record)| {
                let raw_id = u32::try_from(index.saturating_add(1))
                    .map_err(|_| UnityProjectError::TooManyRecords { count: self.records.len() })?;
                let mut facts = record.facts.clone();
                facts.insert(
                    "unity.editor_version".to_owned(),
                    self.provenance.editor_version.clone(),
                );
                facts.insert(
                    "unity.urp_package_version".to_owned(),
                    self.provenance.urp_package_version.clone(),
                );
                facts.insert(
                    "unity.serialization_mode".to_owned(),
                    self.provenance.serialization_mode.clone(),
                );
                facts.insert(
                    "unity.classification".to_owned(),
                    format!("{:?}", record.classification.kind).to_ascii_lowercase(),
                );
                Ok(SourceLink {
                    id: SourceLinkId::new(raw_id),
                    source: record.source.clone(),
                    facts,
                })
            })
            .collect()
    }
}

/// Explicit selected Unity source paths rooted under `Assets/`.
#[derive(Clone, Debug)]
pub struct UnityProjectRequest {
    project_directory: PathBuf,
    project_root: ProjectRoot,
    selected_assets: BTreeSet<SourceAssetPath>,
}

impl UnityProjectRequest {
    /// Creates a root-confined request for caller-selected Unity `Assets/` files.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the project root is invalid or selection is empty.
    pub fn new(
        project_directory: impl AsRef<Path>,
        selected_assets: impl IntoIterator<Item = SourceAssetPath>,
    ) -> Result<Self, UnityProjectError> {
        let project_directory = fs::canonicalize(project_directory)
            .map_err(|source| UnityProjectError::ProjectUnavailable { source })?;
        if !project_directory.is_dir() {
            return Err(UnityProjectError::ProjectNotDirectory { path: project_directory });
        }
        let project_name =
            project_directory.file_name().and_then(|value| value.to_str()).ok_or_else(|| {
                UnityProjectError::ProjectNotDirectory { path: project_directory.clone() }
            })?;
        let project_root = project_name
            .parse::<ProjectRoot>()
            .map_err(|source| UnityProjectError::Identity { source })?;
        let selected_assets = selected_assets.into_iter().collect::<BTreeSet<_>>();
        if selected_assets.is_empty() {
            return Err(UnityProjectError::EmptySelection);
        }
        if selected_assets.iter().any(|path| !path.as_str().starts_with("Assets/")) {
            return Err(UnityProjectError::AssetOutsideAssets);
        }
        Ok(Self { project_directory, project_root, selected_assets })
    }
}

/// Typed failures emitted by the bounded Unity source adapter.
#[derive(Debug, Error)]
pub enum UnityProjectError {
    /// The configured project root does not exist or cannot be canonicalized.
    #[error("Unity project is unavailable: {source}")]
    ProjectUnavailable { source: std::io::Error },
    /// The configured project root is not a directory.
    #[error("Unity project root is not a directory: {path}")]
    ProjectNotDirectory { path: PathBuf },
    /// No selected input assets were provided.
    #[error("Unity source inspection requires at least one explicit selected asset")]
    EmptySelection,
    /// A selected source was not inside the editable project `Assets/` tree.
    #[error("Unity source inspection accepts only selected Assets/ paths")]
    AssetOutsideAssets,
    /// A selected source escaped the canonical authorized project root.
    #[error("Unity source path escapes the authorized project root: {path}")]
    PathEscape { path: PathBuf },
    /// A selected asset or required project setting is unavailable.
    #[error("Unity project input is unavailable: {source}")]
    InputUnavailable { source: std::io::Error },
    /// The selected source type is outside the bounded text-project subset.
    #[error("Unity source type is unsupported: {path}")]
    UnsupportedExtension { path: SourceAssetPath },
    /// A selected source lacks its required visible sibling `.meta` file.
    #[error("Unity source lacks visible meta identity: {asset}")]
    MissingMeta { asset: SourceAssetPath },
    /// A `.meta` file did not contain a typed GUID.
    #[error("Unity meta identity is invalid for {asset}: {reason}")]
    InvalidMeta { asset: SourceAssetPath, reason: String },
    /// Two selected assets resolved to the same GUID.
    #[error("Unity project contains duplicate selected GUID: {guid}")]
    DuplicateGuid { guid: UnityAssetGuid },
    /// A selected text source exceeds the bounded parser size.
    #[error("Unity text source exceeds bounded size limit: {actual_bytes} bytes")]
    TextTooLarge { actual_bytes: u64 },
    /// A selected text source was not UTF-8 text.
    #[error("Unity source is not valid UTF-8 text: {source}")]
    InvalidUtf8 { source: std::string::FromUtf8Error },
    /// Text serialization did not expose the required Unity YAML header.
    #[error("Unity source is binary or uses an unknown serialization layout: {asset}")]
    BinaryOrUnknownSerialization { asset: SourceAssetPath },
    /// The project did not select the required Force Text serialization mode.
    #[error("Unity project must use Force Text serialization mode")]
    SerializationMode,
    /// The supported source parser does not recognize the Editor version.
    #[error("Unity Editor version is unsupported by the bounded source adapter: {version}")]
    UnsupportedEditorVersion { version: String },
    /// The project does not declare Universal Render Pipeline.
    #[error("Unity project does not declare com.unity.render-pipelines.universal")]
    MissingUrpPackage,
    /// The project manifest is invalid JSON.
    #[error("Unity package manifest is malformed: {source}")]
    InvalidPackageManifest { source: serde_json::Error },
    /// A text source referenced a disallowed external/traversal path.
    #[error("Unity text source contains an unsafe external reference: {reference}")]
    UnsafeReference { reference: String },
    /// A YAML object header had an invalid local object ID.
    #[error("Unity YAML object header is malformed: {header}")]
    InvalidObjectHeader { header: String },
    /// The selected source count cannot fit in canonical IR source-link IDs.
    #[error("Unity source record count exceeds canonical IR ID capacity: {count}")]
    TooManyRecords { count: usize },
    /// A typed project or asset identity could not be constructed.
    #[error("Unity source adapter could not create a typed identity: {source}")]
    Identity { source: IdentityError },
    /// A typed classification could not be constructed.
    #[error("Unity source adapter could not create a domain value: {source}")]
    Domain { source: DomainError },
}

/// Isolated secondary adapter for a bounded, text-serialized Unity/URP project.
#[derive(Clone, Copy, Debug, Default)]
pub struct UnityProjectInspector;

impl UnityProjectInspector {
    /// Resolves selected text-project source facts in deterministic asset and local-ID order.
    ///
    /// # Errors
    ///
    /// Returns typed errors for unsafe paths, missing meta identities, duplicate GUIDs,
    /// binary/unknown serialization, unsupported Editor version, missing URP, malformed
    /// package metadata, or invalid object headers.
    pub fn inspect(
        &self,
        request: &UnityProjectRequest,
    ) -> Result<UnityProjectSnapshot, UnityProjectError> {
        let provenance = load_project_provenance(&request.project_directory)?;
        let mut guids = BTreeSet::new();
        let mut records = Vec::new();
        for asset_path in &request.selected_assets {
            let asset_file = canonical_asset_file(request, asset_path)?;
            let guid = load_meta_guid(request, asset_path, &asset_file)?;
            if !guids.insert(guid.clone()) {
                return Err(UnityProjectError::DuplicateGuid { guid });
            }
            let kind = source_kind(asset_path)?;
            let mut asset_records = inspect_asset(request, asset_path, &asset_file, &guid, kind)?;
            records.append(&mut asset_records);
        }
        records.sort_by_key(|record| record.source.to_string());
        Ok(UnityProjectSnapshot { provenance, records })
    }
}

fn load_project_provenance(root: &Path) -> Result<UnityProjectProvenance, UnityProjectError> {
    let version_path = root.join("ProjectSettings/ProjectVersion.txt");
    let version_text = read_bounded_text(&version_path, None)?;
    let editor_version = version_text
        .lines()
        .find_map(|line| line.strip_prefix("m_EditorVersion:").map(str::trim))
        .filter(|version| {
            SUPPORTED_EDITOR_PREFIXES.iter().any(|prefix| version.starts_with(prefix))
        })
        .map(str::to_owned)
        .ok_or_else(|| UnityProjectError::UnsupportedEditorVersion {
            version: version_text
                .lines()
                .find_map(|line| line.strip_prefix("m_EditorVersion:").map(str::trim))
                .map_or_else(|| "missing".to_owned(), str::to_owned),
        })?;
    let settings_path = root.join("ProjectSettings/EditorSettings.asset");
    let settings_text = read_bounded_text(&settings_path, None)?;
    let serialization_mode = settings_text
        .lines()
        .find_map(|line| line.strip_prefix("m_SerializationMode:").map(str::trim))
        .ok_or(UnityProjectError::SerializationMode)?;
    if serialization_mode != "2" {
        return Err(UnityProjectError::SerializationMode);
    }
    let package_path = root.join("Packages/manifest.json");
    let package_text = read_bounded_text(&package_path, None)?;
    let package_json: Value = serde_json::from_str(&package_text)
        .map_err(|source| UnityProjectError::InvalidPackageManifest { source })?;
    let urp_package_version = package_json
        .get("dependencies")
        .and_then(Value::as_object)
        .and_then(|dependencies| dependencies.get("com.unity.render-pipelines.universal"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or(UnityProjectError::MissingUrpPackage)?;
    Ok(UnityProjectProvenance {
        editor_version,
        urp_package_version,
        serialization_mode: "force_text".to_owned(),
    })
}

fn canonical_asset_file(
    request: &UnityProjectRequest,
    asset_path: &SourceAssetPath,
) -> Result<PathBuf, UnityProjectError> {
    if has_parent_component(asset_path.as_str()) {
        return Err(UnityProjectError::PathEscape { path: PathBuf::from(asset_path.as_str()) });
    }
    let candidate = request.project_directory.join(asset_path.as_str());
    let canonical = fs::canonicalize(candidate)
        .map_err(|source| UnityProjectError::InputUnavailable { source })?;
    if !canonical.starts_with(&request.project_directory) {
        return Err(UnityProjectError::PathEscape { path: canonical });
    }
    if !canonical.is_file() {
        return Err(UnityProjectError::InputUnavailable {
            source: std::io::Error::from(std::io::ErrorKind::InvalidInput),
        });
    }
    Ok(canonical)
}

fn load_meta_guid(
    request: &UnityProjectRequest,
    asset_path: &SourceAssetPath,
    asset_file: &Path,
) -> Result<UnityAssetGuid, UnityProjectError> {
    let meta_candidate = PathBuf::from(format!("{}.meta", asset_file.display()));
    let meta_file = fs::canonicalize(meta_candidate)
        .map_err(|_| UnityProjectError::MissingMeta { asset: asset_path.clone() })?;
    if !meta_file.starts_with(&request.project_directory) || !meta_file.is_file() {
        return Err(UnityProjectError::MissingMeta { asset: asset_path.clone() });
    }
    let text = read_bounded_text(&meta_file, Some(asset_path))?;
    let raw_guid = text
        .lines()
        .find_map(|line| line.strip_prefix("guid:").map(str::trim))
        .ok_or_else(|| UnityProjectError::InvalidMeta {
            asset: asset_path.clone(),
            reason: "missing guid property".to_owned(),
        })?;
    raw_guid.parse::<UnityAssetGuid>().map_err(|source| UnityProjectError::InvalidMeta {
        asset: asset_path.clone(),
        reason: source.to_string(),
    })
}

fn source_kind(asset_path: &SourceAssetPath) -> Result<UnitySourceKind, UnityProjectError> {
    let extension = Path::new(asset_path.as_str())
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase);
    match extension.as_deref() {
        Some("unity") => Ok(UnitySourceKind::Scene),
        Some("prefab") => Ok(UnitySourceKind::Prefab),
        Some("mat") => Ok(UnitySourceKind::Material),
        Some("png" | "jpg" | "jpeg" | "tga") => Ok(UnitySourceKind::Texture),
        Some("fbx" | "gltf" | "glb") => Ok(UnitySourceKind::Mesh),
        _ => Err(UnityProjectError::UnsupportedExtension { path: asset_path.clone() }),
    }
}

fn inspect_asset(
    request: &UnityProjectRequest,
    asset_path: &SourceAssetPath,
    asset_file: &Path,
    guid: &UnityAssetGuid,
    kind: UnitySourceKind,
) -> Result<Vec<UnitySourceRecord>, UnityProjectError> {
    match kind {
        UnitySourceKind::Scene | UnitySourceKind::Prefab | UnitySourceKind::Material => {
            inspect_text_asset(request, asset_path, asset_file, guid, kind)
        }
        UnitySourceKind::Texture | UnitySourceKind::Mesh => {
            let classification = explicit_adapter_classification(
                kind,
                "Imported source content requires the matching Unity Editor importer.",
            )?;
            Ok(vec![UnitySourceRecord {
                source: source_identity(request, asset_path, guid, UnityObjectId::new(0)),
                kind,
                classification,
                facts: BTreeMap::from([(
                    "unity.imported_asset".to_owned(),
                    kind.label().to_owned(),
                )]),
            }])
        }
    }
}

fn inspect_text_asset(
    request: &UnityProjectRequest,
    asset_path: &SourceAssetPath,
    asset_file: &Path,
    guid: &UnityAssetGuid,
    kind: UnitySourceKind,
) -> Result<Vec<UnitySourceRecord>, UnityProjectError> {
    let text = read_bounded_text(asset_file, Some(asset_path))?;
    if !text.starts_with(TEXT_HEADER) {
        return Err(UnityProjectError::BinaryOrUnknownSerialization { asset: asset_path.clone() });
    }
    reject_unsafe_references(&text)?;
    let objects = parse_yaml_objects(&text)?;
    if objects.is_empty() {
        return Err(UnityProjectError::BinaryOrUnknownSerialization { asset: asset_path.clone() });
    }
    let mut records = objects
        .into_iter()
        .map(|object| {
            let classification = classify_object(kind, &object)?;
            let mut facts = object.facts;
            facts.insert("unity.kind".to_owned(), kind.label().to_owned());
            facts.insert("unity.class_id".to_owned(), object.class_id.to_string());
            facts.insert("unity.object_type".to_owned(), object.type_name);
            Ok(UnitySourceRecord {
                source: source_identity(request, asset_path, guid, object.local_id),
                kind,
                classification,
                facts,
            })
        })
        .collect::<Result<Vec<_>, UnityProjectError>>()?;
    records.sort_by_key(|record| record.source.to_string());
    Ok(records)
}

fn source_identity(
    request: &UnityProjectRequest,
    asset_path: &SourceAssetPath,
    guid: &UnityAssetGuid,
    object_id: UnityObjectId,
) -> SourceIdentity {
    SourceIdentity::Unity {
        project_root: request.project_root.clone(),
        asset_path: asset_path.clone(),
        asset_guid: guid.clone(),
        object_id,
    }
}

fn explicit_adapter_classification(
    kind: UnitySourceKind,
    reason: &str,
) -> Result<ConversionClassification, UnityProjectError> {
    ConversionClassification::new(
        ClassificationKind::RequiresAdapter,
        reason,
        Remediation::new(format!(
            "Use a matching authorized Unity Editor to import the {} source.",
            kind.label()
        ))
        .map_err(|source| UnityProjectError::Domain { source })?,
    )
    .map_err(|source| UnityProjectError::Domain { source })
}

fn classify_object(
    kind: UnitySourceKind,
    object: &RawUnityObject,
) -> Result<ConversionClassification, UnityProjectError> {
    let type_name = object.type_name.as_str();
    let shader_name = object.facts.get("unity.shader_name").map(String::as_str);
    let supported = match kind {
        UnitySourceKind::Scene | UnitySourceKind::Prefab => matches!(
            type_name,
            "GameObject"
                | "Transform"
                | "MeshRenderer"
                | "SkinnedMeshRenderer"
                | "Camera"
                | "Light"
        ),
        UnitySourceKind::Material => matches!(
            shader_name,
            Some("Universal Render Pipeline/Lit" | "Universal Render Pipeline/Unlit")
        ),
        UnitySourceKind::Texture | UnitySourceKind::Mesh => false,
    };
    let (kind, reason, remediation) = if supported {
        (
            ClassificationKind::RequiresAdapter,
            "Unity text source fact is portable, while mesh and importer output require the matching Unity Editor.",
            "Use the recovered canonical IR adapter after matching-Editor mesh import data is available.",
        )
    } else if type_name.contains("ShaderGraph")
        || shader_name.is_some_and(|name| name.contains("Shader Graph"))
    {
        (
            ClassificationKind::Unsupported,
            "Unity Shader Graph is outside the bounded URP Lit/Unlit source contract.",
            "Bake the material to approved portable textures or provide a dedicated Shader Graph adapter.",
        )
    } else {
        (
            ClassificationKind::RequiresAdapter,
            "Unity component or material is outside the bounded static URP source contract.",
            "Use an approved Unity Editor bridge or a dedicated adapter for this source feature.",
        )
    };
    ConversionClassification::new(
        kind,
        reason,
        Remediation::new(remediation).map_err(|source| UnityProjectError::Domain { source })?,
    )
    .map_err(|source| UnityProjectError::Domain { source })
}

fn read_bounded_text(
    path: &Path,
    asset_path: Option<&SourceAssetPath>,
) -> Result<String, UnityProjectError> {
    let metadata =
        fs::metadata(path).map_err(|source| UnityProjectError::InputUnavailable { source })?;
    if metadata.len() > MAX_TEXT_BYTES {
        return Err(UnityProjectError::TextTooLarge { actual_bytes: metadata.len() });
    }
    let bytes = fs::read(path).map_err(|source| UnityProjectError::InputUnavailable { source })?;
    String::from_utf8(bytes).map_err(|source| {
        asset_path.map_or(UnityProjectError::InvalidUtf8 { source }, |asset| {
            UnityProjectError::BinaryOrUnknownSerialization { asset: asset.clone() }
        })
    })
}

fn has_parent_component(path: &str) -> bool {
    path.split('/').any(|component| component == "." || component == "..")
}

fn reject_unsafe_references(text: &str) -> Result<(), UnityProjectError> {
    for line in text.lines() {
        let reference = line.trim();
        if reference.contains("../") || reference.contains("file://") || reference.starts_with('/')
        {
            return Err(UnityProjectError::UnsafeReference { reference: reference.to_owned() });
        }
    }
    Ok(())
}

#[derive(Debug)]
struct RawUnityObject {
    class_id: u32,
    local_id: UnityObjectId,
    type_name: String,
    facts: BTreeMap<String, String>,
}

fn parse_yaml_objects(text: &str) -> Result<Vec<RawUnityObject>, UnityProjectError> {
    let mut objects = Vec::new();
    let mut current: Option<RawUnityObject> = None;
    for line in text.lines() {
        if line.starts_with("--- !u!") {
            if let Some(object) = current.take() {
                objects.push(object);
            }
            current = Some(parse_object_header(line)?);
            continue;
        }
        let Some(object) = current.as_mut() else {
            continue;
        };
        if object.type_name.is_empty() && !line.starts_with(' ') && line.ends_with(':') {
            line.trim_end_matches(':').clone_into(&mut object.type_name);
        }
        record_object_fact(object, line);
    }
    if let Some(object) = current {
        objects.push(object);
    }
    Ok(objects)
}

fn parse_object_header(line: &str) -> Result<RawUnityObject, UnityProjectError> {
    let remainder = line
        .strip_prefix("--- !u!")
        .ok_or_else(|| UnityProjectError::InvalidObjectHeader { header: line.to_owned() })?;
    let (class_id, local_id) = remainder
        .split_once(" &")
        .ok_or_else(|| UnityProjectError::InvalidObjectHeader { header: line.to_owned() })?;
    let class_id = class_id
        .parse::<u32>()
        .map_err(|_| UnityProjectError::InvalidObjectHeader { header: line.to_owned() })?;
    let local_id = local_id
        .parse::<u64>()
        .map_err(|_| UnityProjectError::InvalidObjectHeader { header: line.to_owned() })?;
    Ok(RawUnityObject {
        class_id,
        local_id: UnityObjectId::new(local_id),
        type_name: String::new(),
        facts: BTreeMap::new(),
    })
}

fn record_object_fact(object: &mut RawUnityObject, line: &str) {
    let trimmed = line.trim();
    if let Some(name) = trimmed.strip_prefix("m_Name:").map(str::trim) {
        object.facts.insert("unity.name".to_owned(), name.to_owned());
    }
    if let Some(shader) = trimmed.strip_prefix("m_ShaderName:").map(str::trim) {
        object.facts.insert("unity.shader_name".to_owned(), shader.to_owned());
    }
    if let Some(guid) = guid_from_reference(trimmed) {
        object.facts.insert("unity.reference_guid".to_owned(), guid);
    }
    if trimmed.starts_with("m_LocalPosition:") {
        object.facts.insert("unity.has_local_position".to_owned(), "true".to_owned());
    }
}

fn guid_from_reference(line: &str) -> Option<String> {
    let (_, after_guid) = line.split_once("guid:")?;
    let guid = after_guid.trim_start().split(',').next()?.trim();
    (!guid.is_empty()).then(|| guid.to_owned())
}
