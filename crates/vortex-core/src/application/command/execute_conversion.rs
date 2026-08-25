use std::error::Error;

use crate::{
    application::command::plan_conversion::{ConversionPlan, PlanAction},
    domain::{ConversionAggregate, ConversionContext, ConversionId, SourceIdentity},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExecutionOutcome {
    Executed(Box<ConversionAggregate>),
    Reused(Box<ConversionAggregate>),
}

impl ExecutionOutcome {
    #[must_use]
    pub const fn id(&self) -> &ConversionId {
        match self {
            Self::Executed(conversion) | Self::Reused(conversion) => conversion.id(),
        }
    }
}

pub trait ConversionRunner {
    type Error: Error + Send + Sync + 'static;

    /// Performs one planned conversion using already validated typed input.
    ///
    /// # Errors
    ///
    /// Returns the adapter-specific error when conversion work cannot complete.
    fn execute(
        &self,
        source: &SourceIdentity,
        context: &ConversionContext,
    ) -> Result<ConversionAggregate, Self::Error>;
}

pub trait ArtifactStore {
    type Error: Error + Send + Sync + 'static;

    /// Persists one completed conversion atomically through the adapter boundary.
    ///
    /// # Errors
    ///
    /// Returns the adapter-specific error when persistence fails.
    fn persist(&self, conversion: &ConversionAggregate) -> Result<(), Self::Error>;
}

#[derive(Debug, thiserror::Error)]
pub enum ExecuteConversionError<RunnerError, StoreError>
where
    RunnerError: Error + Send + Sync + 'static,
    StoreError: Error + Send + Sync + 'static,
{
    #[error("conversion runner failed: {source}")]
    Runner { source: RunnerError },
    #[error("artifact store failed: {source}")]
    Store { source: StoreError },
}

/// Executes a plan once or returns its existing compatible result.
///
/// # Errors
///
/// Returns [`ExecuteConversionError::Runner`] when the runner fails.
/// Returns [`ExecuteConversionError::Store`] when persistence fails.
pub fn handle<Runner, Store>(
    plan: ConversionPlan,
    runner: &Runner,
    store: &Store,
) -> Result<ExecutionOutcome, ExecuteConversionError<Runner::Error, Store::Error>>
where
    Runner: ConversionRunner,
    Store: ArtifactStore,
{
    match plan.action {
        PlanAction::Reuse(existing) => Ok(ExecutionOutcome::Reused(existing)),
        PlanAction::Execute => {
            let conversion = runner
                .execute(&plan.source, &plan.context)
                .map_err(|source| ExecuteConversionError::Runner { source })?;
            store
                .persist(&conversion)
                .map_err(|source| ExecuteConversionError::Store { source })?;
            Ok(ExecutionOutcome::Executed(Box::new(conversion)))
        }
    }
}
