use std::{collections::BTreeMap, fmt};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::identity::{ArtifactHash, ConversionId, ExportProfileName, SourceIdentity};

#[derive(Clone, Debug, Deserialize, Eq, Error, PartialEq, Serialize)]
pub enum DomainError {
    #[error("{field} cannot be empty")]
    EmptyField { field: &'static str },
    #[error("illegal conversion transition from {current:?} while attempting {attempted}")]
    IllegalTransition { current: ConversionState, attempted: &'static str },
    #[error("artifact kind {kind:?} is already recorded for this conversion")]
    DuplicateArtifact { kind: ArtifactKind },
    #[error("a classification is required before finalizing a conversion")]
    MissingClassification,
    #[error("a converted conversion requires both GLB and manifest artifacts")]
    MissingConvertedArtifacts,
    #[error("conversion classification can only be recorded once")]
    DuplicateClassification,
    #[error("invalid diagnostic code: {value}")]
    InvalidDiagnosticCode { value: String },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct Remediation(String);

impl Remediation {
    /// Creates explicit operator-facing remediation text.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::EmptyField`] when the remediation text is empty.
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(DomainError::EmptyField { field: "remediation" });
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ClassificationKind {
    Converted,
    Baked,
    RequiresAdapter,
    Unsupported,
    Rejected,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ConversionClassification {
    pub kind: ClassificationKind,
    pub reason: String,
    pub remediation: Remediation,
}

impl ConversionClassification {
    /// Creates a classification with an actionable reason and remediation.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::EmptyField`] when the reason is empty.
    pub fn new(
        kind: ClassificationKind,
        reason: impl Into<String>,
        remediation: Remediation,
    ) -> Result<Self, DomainError> {
        let reason = reason.into();
        if reason.trim().is_empty() {
            return Err(DomainError::EmptyField { field: "reason" });
        }
        Ok(Self { kind, reason, remediation })
    }
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(try_from = "String", into = "String")]
pub struct DiagnosticCode(String);

impl DiagnosticCode {
    /// Parses a stable lower-case diagnostic code.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::EmptyField`] when the code is empty.
    /// Returns [`DomainError::InvalidDiagnosticCode`] when the code contains an
    /// unsupported character.
    pub fn parse(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        if value.is_empty() {
            return Err(DomainError::EmptyField { field: "diagnostic code" });
        }
        if !value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'.' || byte == b'_'
        }) {
            return Err(DomainError::InvalidDiagnosticCode { value });
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for DiagnosticCode {
    type Error = DomainError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl From<DiagnosticCode> for String {
    fn from(value: DiagnosticCode) -> Self {
        value.0
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Diagnostic {
    pub code: DiagnosticCode,
    pub message: String,
    pub remediation: Remediation,
}

impl Diagnostic {
    /// Creates a structured, actionable diagnostic.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::EmptyField`] when the message is empty.
    pub fn new(
        code: DiagnosticCode,
        message: impl Into<String>,
        remediation: Remediation,
    ) -> Result<Self, DomainError> {
        let message = message.into();
        if message.trim().is_empty() {
            return Err(DomainError::EmptyField { field: "message" });
        }
        Ok(Self { code, message, remediation })
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    Glb,
    Manifest,
    Report,
    Texture,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ArtifactRecord {
    pub kind: ArtifactKind,
    pub hash: ArtifactHash,
    pub label: String,
}

impl ArtifactRecord {
    /// Creates a typed artifact record.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::EmptyField`] when the stable artifact label is empty.
    pub fn new(
        kind: ArtifactKind,
        hash: ArtifactHash,
        label: impl Into<String>,
    ) -> Result<Self, DomainError> {
        let label = label.into();
        if label.trim().is_empty() {
            return Err(DomainError::EmptyField { field: "artifact label" });
        }
        Ok(Self { kind, hash, label })
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversionState {
    InProgress,
    Classified,
    Finalized,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ConversionContext {
    pub profile: ExportProfileName,
    pub profile_hash: ArtifactHash,
    pub converter_version: String,
}

impl ConversionContext {
    /// Creates deterministic conversion context passed explicitly by callers.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::EmptyField`] when the converter version is empty.
    pub fn new(
        profile: ExportProfileName,
        profile_hash: ArtifactHash,
        converter_version: impl Into<String>,
    ) -> Result<Self, DomainError> {
        let converter_version = converter_version.into();
        if converter_version.trim().is_empty() {
            return Err(DomainError::EmptyField { field: "converter version" });
        }
        Ok(Self { profile, profile_hash, converter_version })
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ConversionAggregate {
    id: ConversionId,
    source: SourceIdentity,
    context: ConversionContext,
    state: ConversionState,
    artifacts: BTreeMap<ArtifactKind, ArtifactRecord>,
    diagnostics: Vec<Diagnostic>,
    classification: Option<ConversionClassification>,
}

impl ConversionAggregate {
    /// Begins a conversion with explicit source and context.
    #[must_use]
    pub fn begin(source: SourceIdentity, context: ConversionContext) -> Self {
        let id = ConversionId::derive(&source, &context.profile_hash);
        Self {
            id,
            source,
            context,
            state: ConversionState::InProgress,
            artifacts: BTreeMap::new(),
            diagnostics: Vec::new(),
            classification: None,
        }
    }

    #[must_use]
    pub const fn id(&self) -> &ConversionId {
        &self.id
    }

    #[must_use]
    pub const fn source(&self) -> &SourceIdentity {
        &self.source
    }

    #[must_use]
    pub const fn context(&self) -> &ConversionContext {
        &self.context
    }

    #[must_use]
    pub const fn state(&self) -> &ConversionState {
        &self.state
    }

    #[must_use]
    pub const fn artifacts(&self) -> &BTreeMap<ArtifactKind, ArtifactRecord> {
        &self.artifacts
    }

    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    #[must_use]
    pub const fn classification(&self) -> Option<&ConversionClassification> {
        self.classification.as_ref()
    }

    /// Records a unique artifact while conversion work is in progress.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::IllegalTransition`] after classification or finalization.
    /// Returns [`DomainError::DuplicateArtifact`] when an artifact kind is recorded twice.
    pub fn record_artifact(&mut self, artifact: ArtifactRecord) -> Result<(), DomainError> {
        self.require_state(ConversionState::InProgress, "record artifact")?;
        if self.artifacts.contains_key(&artifact.kind) {
            return Err(DomainError::DuplicateArtifact { kind: artifact.kind });
        }
        self.artifacts.insert(artifact.kind, artifact);
        Ok(())
    }

    /// Attaches a diagnostic before finalization.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::IllegalTransition`] after finalization.
    pub fn attach_diagnostic(&mut self, diagnostic: Diagnostic) -> Result<(), DomainError> {
        if self.state == ConversionState::Finalized {
            return Err(DomainError::IllegalTransition {
                current: self.state,
                attempted: "attach diagnostic",
            });
        }
        self.diagnostics.push(diagnostic);
        Ok(())
    }

    /// Records exactly one conversion classification.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::IllegalTransition`] after finalization.
    /// Returns [`DomainError::DuplicateClassification`] when classification already exists.
    pub fn classify(
        &mut self,
        classification: ConversionClassification,
    ) -> Result<(), DomainError> {
        self.require_state(ConversionState::InProgress, "classify conversion")?;
        if self.classification.is_some() {
            return Err(DomainError::DuplicateClassification);
        }
        self.classification = Some(classification);
        self.state = ConversionState::Classified;
        Ok(())
    }

    /// Finalizes a valid classified conversion.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::MissingClassification`] when no outcome is present.
    /// Returns [`DomainError::MissingConvertedArtifacts`] when a converted outcome
    /// lacks required GLB and manifest records.
    /// Returns [`DomainError::IllegalTransition`] when already finalized.
    pub fn finalize(mut self) -> Result<Self, DomainError> {
        if self.state == ConversionState::Finalized {
            return Err(DomainError::IllegalTransition {
                current: self.state,
                attempted: "finalize conversion",
            });
        }
        let Some(classification) = self.classification.as_ref() else {
            return Err(DomainError::MissingClassification);
        };
        if classification.kind == ClassificationKind::Converted
            && (!self.artifacts.contains_key(&ArtifactKind::Glb)
                || !self.artifacts.contains_key(&ArtifactKind::Manifest))
        {
            return Err(DomainError::MissingConvertedArtifacts);
        }
        self.state = ConversionState::Finalized;
        Ok(self)
    }

    fn require_state(
        &self,
        expected: ConversionState,
        attempted: &'static str,
    ) -> Result<(), DomainError> {
        if self.state != expected {
            return Err(DomainError::IllegalTransition { current: self.state, attempted });
        }
        Ok(())
    }
}

impl fmt::Display for DiagnosticCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}
