use std::{
    error::Error,
    fmt,
    io::{self, Write},
    str::FromStr,
    sync::{Arc, Mutex},
};

use tracing_subscriber::fmt::MakeWriter;
use vortex_core::{
    application::command::{
        execute_conversion::{self, ArtifactStore, ConversionRunner, ExecutionOutcome},
        plan_conversion::{ConversionPlan, PlanAction},
    },
    domain::{
        ArtifactHash, ClassificationKind, ConversionAggregate, ConversionClassification,
        ConversionContext, ExportProfileName, ProjectRoot, RedactedSourceIdentity, Remediation,
        SourceAssetPath, SourceIdentity, UnrealObjectPath,
    },
};

#[derive(Clone, Default)]
struct CaptureWriter(Arc<Mutex<Vec<u8>>>);

struct CaptureGuard(CaptureWriter);

impl Write for CaptureGuard {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0
            .0
            .lock()
            .map_err(|_| io::Error::other("trace capture unavailable"))?
            .extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for CaptureWriter {
    type Writer = CaptureGuard;

    fn make_writer(&'a self) -> Self::Writer {
        CaptureGuard(self.clone())
    }
}

impl CaptureWriter {
    fn text(&self) -> Result<String, Box<dyn Error>> {
        let bytes =
            self.0.lock().map_err(|_| io::Error::other("trace capture unavailable"))?.clone();
        Ok(String::from_utf8(bytes)?)
    }
}

#[derive(Debug)]
struct UnexpectedAdapterCall;

impl fmt::Display for UnexpectedAdapterCall {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("reuse plan unexpectedly called an adapter")
    }
}

impl Error for UnexpectedAdapterCall {}

struct NeverRunner;

impl ConversionRunner for NeverRunner {
    type Error = UnexpectedAdapterCall;

    fn execute(
        &self,
        _source: &SourceIdentity,
        _context: &ConversionContext,
    ) -> Result<ConversionAggregate, Self::Error> {
        Err(UnexpectedAdapterCall)
    }
}

struct NeverStore;

impl ArtifactStore for NeverStore {
    type Error = UnexpectedAdapterCall;

    fn persist(&self, _conversion: &ConversionAggregate) -> Result<(), Self::Error> {
        Err(UnexpectedAdapterCall)
    }
}

fn sensitive_source() -> Result<SourceIdentity, Box<dyn Error>> {
    Ok(SourceIdentity::Unreal {
        project_root: ProjectRoot::from_str("SensitiveProject")?,
        asset_path: SourceAssetPath::from_str("Content/TopSecret/Texture.uasset")?,
        object_path: UnrealObjectPath::from_str("/Game/TopSecret/Texture.Texture")?,
    })
}

fn finalized_reuse_plan() -> Result<(ConversionPlan, RedactedSourceIdentity), Box<dyn Error>> {
    let source = sensitive_source()?;
    let context = ConversionContext::new(
        ExportProfileName::from_str("bench")?,
        ArtifactHash::from_bytes(b"observability-profile"),
        "0.1.0",
    )?;
    let mut conversion = ConversionAggregate::begin(source.clone(), context.clone());
    conversion.classify(ConversionClassification::new(
        ClassificationKind::Rejected,
        "owned tracing fixture",
        Remediation::new("Use the owned fixture.")?,
    )?)?;
    let conversion = conversion.finalize()?;
    let redacted = RedactedSourceIdentity::from(&source);
    let plan = ConversionPlan {
        id: conversion.id().clone(),
        source,
        context,
        action: PlanAction::Reuse(Box::new(conversion)),
    };
    Ok((plan, redacted))
}

#[test]
fn structured_execution_trace_correlates_without_raw_source_identity() -> Result<(), Box<dyn Error>>
{
    let (plan, redacted) = finalized_reuse_plan()?;
    let conversion_id = plan.id.as_str().to_owned();
    let capture = CaptureWriter::default();
    let subscriber = tracing_subscriber::fmt()
        .with_ansi(false)
        .without_time()
        .with_writer(capture.clone())
        .finish();

    let outcome = tracing::subscriber::with_default(subscriber, || {
        execute_conversion::handle(plan, &NeverRunner, &NeverStore)
    })?;
    let output = capture.text()?;

    assert!(matches!(outcome, ExecutionOutcome::Reused(_)));
    assert!(output.contains("conversion execution admitted"));
    assert!(output.contains(&conversion_id));
    assert!(output.contains(redacted.engine.as_str()));
    assert!(output.contains(redacted.fingerprint.as_str()));
    assert!(!output.contains("SensitiveProject"));
    assert!(!output.contains("Content/TopSecret"));
    assert!(!output.contains("/Game/TopSecret"));
    Ok(())
}
