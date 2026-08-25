use std::{cell::Cell, error::Error, fmt, str::FromStr};

use vortex_core::{
    application::command::{
        execute_conversion::{self, ArtifactStore, ConversionRunner, ExecutionOutcome},
        plan_conversion::{self, ExistingConversionReader, PlanAction, PlanConversion},
    },
    domain::{
        ArtifactHash, ClassificationKind, ConversionAggregate, ConversionClassification,
        ConversionContext, ExportProfileName, ProjectRoot, Remediation, SourceAssetPath,
        SourceIdentity, UnrealObjectPath,
    },
};

#[derive(Debug)]
struct FakeError;

impl fmt::Display for FakeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("fake adapter error")
    }
}

impl Error for FakeError {}

fn source() -> Result<SourceIdentity, Box<dyn Error>> {
    Ok(SourceIdentity::Unreal {
        project_root: ProjectRoot::from_str("FixtureProject")?,
        asset_path: SourceAssetPath::from_str("Content/Props/Chair.uasset")?,
        object_path: UnrealObjectPath::from_str("/Game/Props/Chair.Chair")?,
    })
}

fn context() -> Result<ConversionContext, Box<dyn Error>> {
    Ok(ConversionContext::new(
        ExportProfileName::from_str("bevy-default")?,
        ArtifactHash::from_bytes(b"bevy-default-profile"),
        "0.1.0",
    )?)
}

fn finalized_rejection() -> Result<ConversionAggregate, Box<dyn Error>> {
    let mut conversion = ConversionAggregate::begin(source()?, context()?);
    conversion.classify(ConversionClassification::new(
        ClassificationKind::Rejected,
        "fixture rejection",
        Remediation::new("Select an approved fixture.")?,
    )?)?;
    Ok(conversion.finalize()?)
}

struct Reader {
    existing: Option<ConversionAggregate>,
    calls: Cell<usize>,
}

impl ExistingConversionReader for Reader {
    type Error = FakeError;

    fn find_compatible(
        &self,
        _id: &vortex_core::domain::ConversionId,
    ) -> Result<Option<ConversionAggregate>, Self::Error> {
        self.calls.set(self.calls.get() + 1);
        Ok(self.existing.clone())
    }
}

struct Runner {
    calls: Cell<usize>,
}

impl ConversionRunner for Runner {
    type Error = FakeError;

    fn execute(
        &self,
        _source: &SourceIdentity,
        _context: &ConversionContext,
    ) -> Result<ConversionAggregate, Self::Error> {
        self.calls.set(self.calls.get() + 1);
        finalized_rejection().map_err(|_| FakeError)
    }
}

struct Store {
    calls: Cell<usize>,
}

impl ArtifactStore for Store {
    type Error = FakeError;

    fn persist(&self, _conversion: &ConversionAggregate) -> Result<(), Self::Error> {
        self.calls.set(self.calls.get() + 1);
        Ok(())
    }
}

#[test]
fn existing_compatible_conversion_is_reused_without_execution() -> Result<(), Box<dyn Error>> {
    let existing = finalized_rejection()?;
    let reader = Reader { existing: Some(existing.clone()), calls: Cell::new(0) };
    let plan = plan_conversion::handle(
        PlanConversion { source: source()?, context: context()? },
        &reader,
    )?;
    assert!(matches!(plan.action, PlanAction::Reuse(_)));

    let runner = Runner { calls: Cell::new(0) };
    let store = Store { calls: Cell::new(0) };
    let outcome = execute_conversion::handle(plan, &runner, &store)?;
    assert!(matches!(outcome, ExecutionOutcome::Reused(ref value) if **value == existing));
    assert_eq!(reader.calls.get(), 1);
    assert_eq!(runner.calls.get(), 0);
    assert_eq!(store.calls.get(), 0);

    Ok(())
}

#[test]
fn missing_compatible_conversion_executes_and_persists_once() -> Result<(), Box<dyn Error>> {
    let reader = Reader { existing: None, calls: Cell::new(0) };
    let plan = plan_conversion::handle(
        PlanConversion { source: source()?, context: context()? },
        &reader,
    )?;
    assert!(matches!(plan.action, PlanAction::Execute));

    let runner = Runner { calls: Cell::new(0) };
    let store = Store { calls: Cell::new(0) };
    let outcome = execute_conversion::handle(plan, &runner, &store)?;
    assert!(matches!(outcome, ExecutionOutcome::Executed(_)));
    assert_eq!(reader.calls.get(), 1);
    assert_eq!(runner.calls.get(), 1);
    assert_eq!(store.calls.get(), 1);

    Ok(())
}
