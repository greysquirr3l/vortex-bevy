use std::error::Error;

use crate::domain::{ArtifactHash, ConversionId, Diagnostic};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifyArtifacts {
    pub conversion_id: ConversionId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactVerification {
    pub conversion_id: ConversionId,
    pub verified_hashes: Vec<ArtifactHash>,
    pub diagnostics: Vec<Diagnostic>,
}

pub trait ArtifactVerifier {
    type Error: Error + Send + Sync + 'static;

    /// Verifies all published artifacts for one conversion identity.
    ///
    /// # Errors
    ///
    /// Returns the adapter-specific error when artifact verification fails.
    fn verify(&self, id: &ConversionId) -> Result<ArtifactVerification, Self::Error>;
}

/// Verifies published artifacts for one conversion identity.
///
/// # Errors
///
/// Propagates the adapter error returned by [`ArtifactVerifier`].
pub fn handle<Verifier>(
    command: VerifyArtifacts,
    verifier: &Verifier,
) -> Result<ArtifactVerification, Verifier::Error>
where
    Verifier: ArtifactVerifier,
{
    let VerifyArtifacts { conversion_id } = command;
    verifier.verify(&conversion_id)
}
