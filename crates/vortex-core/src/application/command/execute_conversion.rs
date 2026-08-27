use std::error::Error;

use crate::{
    application::command::{
        plan_conversion::{ConversionPlan, PlanAction},
        resource_limits::{ConversionResourceError, ConversionResourceGovernor},
    },
    domain::{
        ConversionAggregate, ConversionContext, ConversionId, ConversionTraceContext,
        SourceIdentity,
    },
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

/// Errors emitted by resource-bounded conversion execution.
#[derive(Debug, thiserror::Error)]
pub enum BoundedExecuteConversionError<RunnerError, StoreError>
where
    RunnerError: Error + Send + Sync + 'static,
    StoreError: Error + Send + Sync + 'static,
{
    #[error("conversion resource admission failed: {source}")]
    Resource { source: ConversionResourceError },
    #[error(transparent)]
    Execute {
        #[from]
        source: ExecuteConversionError<RunnerError, StoreError>,
    },
}

/// Executes a plan once or returns its existing compatible result.
///
/// The emitted tracing span carries only a typed conversion ID, source-engine family, and opaque
/// source fingerprint. It never records a project root, asset path, object path, or report body.
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
    let trace = ConversionTraceContext::new(plan.id.clone(), &plan.source);
    let span = tracing::info_span!(
        "vortex.conversion.execute",
        conversion_id = trace.conversion_id.as_str(),
        source_engine = trace.source.engine.as_str(),
        source_fingerprint = trace.source.fingerprint.as_str(),
    );
    let _entered = span.enter();
    let action = match &plan.action {
        PlanAction::Execute => "execute",
        PlanAction::Reuse(_) => "reuse",
    };
    tracing::info!(action, "conversion execution admitted");
    execute_plan(plan, runner, store)
}

/// Executes a plan after reserving one bounded concurrency slot and estimated memory budget.
///
/// The governor permit is scoped to this synchronous function and released before it returns. The
/// function does not hold a mutex guard while calling an adapter and cannot span an await point.
///
/// # Errors
///
/// Returns [`BoundedExecuteConversionError::Resource`] when the configured concurrency or memory
/// limits deny admission. Propagates execution failures as [`BoundedExecuteConversionError::Execute`].
pub fn handle_bounded<Runner, Store>(
    plan: ConversionPlan,
    runner: &Runner,
    store: &Store,
    governor: &ConversionResourceGovernor,
    estimated_bytes: usize,
) -> Result<ExecutionOutcome, BoundedExecuteConversionError<Runner::Error, Store::Error>>
where
    Runner: ConversionRunner,
    Store: ArtifactStore,
{
    let reservation = governor
        .acquire(estimated_bytes)
        .map_err(|source| BoundedExecuteConversionError::Resource { source })?;
    let outcome = handle(plan, runner, store)?;
    drop(reservation);
    Ok(outcome)
}

fn execute_plan<Runner, Store>(
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
