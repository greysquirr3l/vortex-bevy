use std::error::Error;

use crate::domain::{ConversionAggregate, ConversionContext, ConversionId, SourceIdentity};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanConversion {
    pub source: SourceIdentity,
    pub context: ConversionContext,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlanAction {
    Execute,
    Reuse(Box<ConversionAggregate>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConversionPlan {
    pub id: ConversionId,
    pub source: SourceIdentity,
    pub context: ConversionContext,
    pub action: PlanAction,
}

pub trait ExistingConversionReader {
    type Error: Error + Send + Sync + 'static;

    /// Finds a completed artifact contract compatible with one conversion identity.
    ///
    /// # Errors
    ///
    /// Returns the adapter-specific error when compatible-result lookup fails.
    fn find_compatible(
        &self,
        id: &ConversionId,
    ) -> Result<Option<ConversionAggregate>, Self::Error>;
}

/// Produces a deterministic conversion plan without starting external work.
///
/// # Errors
///
/// Propagates the adapter error returned by [`ExistingConversionReader`].
pub fn handle<Reader>(
    command: PlanConversion,
    reader: &Reader,
) -> Result<ConversionPlan, Reader::Error>
where
    Reader: ExistingConversionReader,
{
    let draft = ConversionAggregate::begin(command.source.clone(), command.context.clone());
    let action = reader
        .find_compatible(draft.id())?
        .map_or(PlanAction::Execute, |existing| PlanAction::Reuse(Box::new(existing)));

    Ok(ConversionPlan {
        id: draft.id().clone(),
        source: command.source,
        context: command.context,
        action,
    })
}
