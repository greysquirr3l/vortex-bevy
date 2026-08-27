use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const HASH_HEX_LENGTH: usize = 64;

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum IdentityError {
    #[error("{kind} cannot be empty")]
    Empty { kind: &'static str },
    #[error("{kind} contains a forbidden character or path component: {value}")]
    Invalid { kind: &'static str, value: String },
    #[error(
        "artifact hash must be a {HASH_HEX_LENGTH}-character lowercase hexadecimal SHA-256 value"
    )]
    InvalidHash,
}

macro_rules! validated_string {
    ($name:ident, $kind:literal, $validator:ident) => {
        #[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(String);

        impl $name {
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl FromStr for $name {
            type Err = IdentityError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                $validator(value, $kind)?;
                Ok(Self(value.to_owned()))
            }
        }

        impl TryFrom<String> for $name {
            type Error = IdentityError;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                value.parse()
            }
        }

        impl From<$name> for String {
            fn from(value: $name) -> Self {
                value.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(formatter)
            }
        }
    };
}

validated_string!(ProjectRoot, "project root", validate_relative_path);
validated_string!(OutputRoot, "output root", validate_relative_path);
validated_string!(SourceAssetPath, "source asset path", validate_relative_path);
validated_string!(ExportProfileName, "export profile name", validate_profile_name);
validated_string!(UnrealObjectPath, "Unreal object path", validate_object_path);
validated_string!(UsdPrimPath, "USD prim path", validate_object_path);
validated_string!(GodotResourcePath, "Godot resource path", validate_godot_resource_path);
validated_string!(StageSelection, "stage selection", validate_selection);
validated_string!(UnityAssetGuid, "Unity asset GUID", validate_guid);

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct UnityObjectId(u64);

impl UnityObjectId {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceIdentity {
    Unreal {
        project_root: ProjectRoot,
        asset_path: SourceAssetPath,
        object_path: UnrealObjectPath,
    },
    Usd {
        stage: SourceAssetPath,
        prim_path: UsdPrimPath,
        selection: StageSelection,
    },
    Unity {
        project_root: ProjectRoot,
        asset_path: SourceAssetPath,
        asset_guid: UnityAssetGuid,
        object_id: UnityObjectId,
    },
    Godot {
        project_root: ProjectRoot,
        resource_path: GodotResourcePath,
    },
}

impl fmt::Display for SourceIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unreal { project_root, asset_path, object_path } => {
                write!(formatter, "unreal:{project_root}:{asset_path}:{object_path}")
            }
            Self::Usd { stage, prim_path, selection } => {
                write!(formatter, "usd:{stage}:{prim_path}:{selection}")
            }
            Self::Unity { project_root, asset_path, asset_guid, object_id } => write!(
                formatter,
                "unity:{project_root}:{asset_path}:{asset_guid}:{}",
                object_id.get()
            ),
            Self::Godot { project_root, resource_path } => {
                write!(formatter, "godot:{project_root}:{resource_path}")
            }
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(try_from = "String", into = "String")]
pub struct ArtifactHash(String);

impl ArtifactHash {
    #[must_use]
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let digest = Sha256::digest(bytes);
        Self(format!("{digest:x}"))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for ArtifactHash {
    type Err = IdentityError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.len() != HASH_HEX_LENGTH
            || !value.bytes().all(|byte| {
                byte.is_ascii_digit() || (byte.is_ascii_lowercase() && byte.is_ascii_hexdigit())
            })
        {
            return Err(IdentityError::InvalidHash);
        }
        Ok(Self(value.to_owned()))
    }
}

impl TryFrom<String> for ArtifactHash {
    type Error = IdentityError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl From<ArtifactHash> for String {
    fn from(value: ArtifactHash) -> Self {
        value.0
    }
}

impl fmt::Display for ArtifactHash {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct ConversionId(String);

impl ConversionId {
    #[must_use]
    pub fn derive(source: &SourceIdentity, profile_hash: &ArtifactHash) -> Self {
        let input = format!("vortex-conversion-v1:{source}:{profile_hash}");
        let digest = Sha256::digest(input.as_bytes());
        Self(format!("{digest:x}"))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ConversionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

fn validate_relative_path(value: &str, kind: &'static str) -> Result<(), IdentityError> {
    if value.is_empty() {
        return Err(IdentityError::Empty { kind });
    }

    if value.starts_with('/')
        || value.starts_with('\\')
        || value.split('/').any(|part| part.is_empty() || part == "." || part == "..")
        || value.split('\\').any(|part| part.is_empty() || part == "." || part == "..")
        || value.contains('\0')
    {
        return Err(IdentityError::Invalid { kind, value: value.to_owned() });
    }

    Ok(())
}

fn validate_profile_name(value: &str, kind: &'static str) -> Result<(), IdentityError> {
    if value.is_empty() {
        return Err(IdentityError::Empty { kind });
    }

    if !value.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_') {
        return Err(IdentityError::Invalid { kind, value: value.to_owned() });
    }

    Ok(())
}

fn validate_object_path(value: &str, kind: &'static str) -> Result<(), IdentityError> {
    if value.is_empty() {
        return Err(IdentityError::Empty { kind });
    }

    if !value.starts_with('/')
        || value.contains("//")
        || value.contains("..")
        || value.contains('\0')
    {
        return Err(IdentityError::Invalid { kind, value: value.to_owned() });
    }

    Ok(())
}

fn validate_godot_resource_path(value: &str, kind: &'static str) -> Result<(), IdentityError> {
    if value.is_empty() {
        return Err(IdentityError::Empty { kind });
    }

    let valid_prefix = value.starts_with("res://") || value.starts_with("godot://temporary/");
    if !valid_prefix
        || value.contains('\\')
        || value.contains('\0')
        || value.split('/').any(|part| part == "." || part == "..")
    {
        return Err(IdentityError::Invalid { kind, value: value.to_owned() });
    }
    Ok(())
}

fn validate_selection(value: &str, kind: &'static str) -> Result<(), IdentityError> {
    if value.is_empty() {
        return Err(IdentityError::Empty { kind });
    }

    if value.contains('\0') || value.contains('\n') || value.contains('\r') {
        return Err(IdentityError::Invalid { kind, value: value.to_owned() });
    }

    Ok(())
}

fn validate_guid(value: &str, kind: &'static str) -> Result<(), IdentityError> {
    if value.len() != 32
        || !value.bytes().all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
    {
        return Err(IdentityError::Invalid { kind, value: value.to_owned() });
    }

    Ok(())
}
