//! Typed, redacted values for structured conversion observations.

use serde::Serialize;

use super::{ArtifactHash, ConversionId, SourceIdentity};

/// Source-engine family safe to include in structured telemetry.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceEngine {
    /// Unreal Engine source identity.
    Unreal,
    /// `OpenUSD` source identity.
    Usd,
    /// Unity source identity.
    Unity,
    /// Godot source identity.
    Godot,
}

impl SourceEngine {
    /// Returns the stable telemetry label for this source engine.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unreal => "unreal",
            Self::Usd => "usd",
            Self::Unity => "unity",
            Self::Godot => "godot",
        }
    }
}

/// A source identity reduced to engine family and an opaque stable fingerprint.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RedactedSourceIdentity {
    /// The source-engine family, without a project root or asset path.
    pub engine: SourceEngine,
    /// A SHA-256 fingerprint of the full typed identity; the raw identity is not retained.
    pub fingerprint: ArtifactHash,
}

impl From<&SourceIdentity> for RedactedSourceIdentity {
    fn from(source: &SourceIdentity) -> Self {
        let engine = match source {
            SourceIdentity::Unreal { .. } => SourceEngine::Unreal,
            SourceIdentity::Usd { .. } => SourceEngine::Usd,
            SourceIdentity::Unity { .. } => SourceEngine::Unity,
            SourceIdentity::Godot { .. } => SourceEngine::Godot,
        };
        Self { engine, fingerprint: ArtifactHash::from_bytes(source.to_string().as_bytes()) }
    }
}

/// Correlation fields permitted in a conversion tracing span or diagnostic report.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ConversionTraceContext {
    /// The deterministic typed conversion identifier.
    pub conversion_id: ConversionId,
    /// The redacted source identity used for correlation.
    pub source: RedactedSourceIdentity,
}

impl ConversionTraceContext {
    /// Constructs a trace context without retaining a raw source path or object identity.
    #[must_use]
    pub fn new(conversion_id: ConversionId, source: &SourceIdentity) -> Self {
        Self { conversion_id, source: source.into() }
    }
}

#[cfg(test)]
mod tests {
    use super::{ConversionTraceContext, SourceEngine};
    use crate::domain::{
        ArtifactHash, ConversionId, ProjectRoot, SourceAssetPath, SourceIdentity, UnrealObjectPath,
    };

    #[test]
    fn trace_context_retains_correlation_without_raw_source_text()
    -> Result<(), Box<dyn std::error::Error>> {
        let source = SourceIdentity::Unreal {
            project_root: "SensitiveProject".parse::<ProjectRoot>()?,
            asset_path: "Content/Private/Asset.uasset".parse::<SourceAssetPath>()?,
            object_path: "/Game/Private/Asset.Asset".parse::<UnrealObjectPath>()?,
        };
        let id = ConversionId::derive(&source, &ArtifactHash::from_bytes(b"profile"));
        let context = ConversionTraceContext::new(id, &source);
        let encoded = serde_json::to_string(&context)?;
        assert_eq!(context.source.engine, SourceEngine::Unreal);
        assert_eq!(context.source.fingerprint.as_str().len(), 64);
        assert!(!encoded.contains("SensitiveProject"));
        assert!(!encoded.contains("Private/Asset"));
        Ok(())
    }
}
