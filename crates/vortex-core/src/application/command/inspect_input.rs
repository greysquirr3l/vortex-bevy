use std::error::Error;

use crate::domain::{ConversionClassification, SourceIdentity};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InspectInput {
    pub source: SourceIdentity,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InputInspection {
    pub source: SourceIdentity,
    pub classification: ConversionClassification,
    pub facts: Vec<String>,
}

pub trait InputInspector {
    type Error: Error + Send + Sync + 'static;

    /// Inspects one typed source and returns its stable facts.
    ///
    /// # Errors
    ///
    /// Returns the adapter-specific error when source inspection fails.
    fn inspect(&self, source: &SourceIdentity) -> Result<InputInspection, Self::Error>;
}

/// Inspects one typed source through the consumer-owned reader contract.
///
/// # Errors
///
/// Propagates the adapter error returned by [`InputInspector`].
pub fn handle<Inspector>(
    command: InspectInput,
    inspector: &Inspector,
) -> Result<InputInspection, Inspector::Error>
where
    Inspector: InputInspector,
{
    let InspectInput { source } = command;
    inspector.inspect(&source)
}
