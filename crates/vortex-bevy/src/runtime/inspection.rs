//! Data-driven Vortex runtime inspection models for a viewer or headless host.

use std::collections::BTreeMap;

use anyhow::{Result, anyhow};
use bevy::prelude::{App, Entity, World};
use vortex_core::domain::{ClassificationKind, Diagnostic, SourceIdentity};

use super::{
    VortexConversionStatus, VortexDiagnosticSet, VortexSceneLifecycle, VortexSceneRoot,
    VortexSourceLink, VortexVariantSelection,
};

/// A source row available to a viewer without exposing conversion-handler internals.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeSourceInspection {
    /// Typed source identity retained by the manifest loader.
    pub identity: SourceIdentity,
    /// Source-side asset class or kind.
    pub asset_kind: String,
    /// Final conversion classification for this source record.
    pub classification: ClassificationKind,
    /// GLB labels that retain source-to-artifact linkage.
    pub glb_labels: BTreeMap<String, String>,
    /// Selected source variants preserved as flattened artifact provenance.
    pub selected_values: BTreeMap<String, String>,
}

/// A detached scene-inspection snapshot safe for presentation code to own.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeSceneInspection {
    /// In-process reload generation for the selected root.
    pub generation: u64,
    /// Source records in deterministic manifest order.
    pub sources: Vec<RuntimeSourceInspection>,
    /// Actionable diagnostics retained on the scene root.
    pub diagnostics: Vec<Diagnostic>,
}

impl RuntimeSceneInspection {
    /// Returns diagnostics whose stable code begins with the supplied lower-case prefix.
    #[must_use]
    pub fn diagnostics_with_code_prefix(&self, prefix: &str) -> Vec<Diagnostic> {
        self.diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code.as_str().starts_with(prefix))
            .cloned()
            .collect()
    }

    /// Returns sources with the requested conversion classification.
    #[must_use]
    pub fn sources_with_classification(
        &self,
        classification: &ClassificationKind,
    ) -> Vec<RuntimeSourceInspection> {
        self.sources
            .iter()
            .filter(|source| &source.classification == classification)
            .cloned()
            .collect()
    }
}

/// Produces a presentation-safe snapshot for one Vortex runtime root held by an application.
///
/// # Errors
///
/// Returns an error when the entity is absent, is not a Vortex root, or has inconsistent retained
/// provenance components.
pub fn inspect_scene(app: &App, entity: Entity) -> Result<RuntimeSceneInspection> {
    inspect_scene_in_world(app.world(), entity)
}

/// Produces a presentation-safe snapshot for one Vortex runtime root held by a Bevy world.
///
/// # Errors
///
/// Returns an error when the entity is absent, is not a Vortex root, or has inconsistent retained
/// provenance components.
pub fn inspect_scene_in_world(world: &World, entity: Entity) -> Result<RuntimeSceneInspection> {
    let root =
        world.get_entity(entity).map_err(|_| anyhow!("scene root {entity:?} does not exist"))?;
    if !root.contains::<VortexSceneRoot>() {
        return Err(anyhow!("entity {entity:?} is not a Vortex scene root"));
    }

    let lifecycle = root
        .get::<VortexSceneLifecycle>()
        .ok_or_else(|| anyhow!("Vortex scene root {entity:?} has no lifecycle state"))?;
    let source_link = root
        .get::<VortexSourceLink>()
        .ok_or_else(|| anyhow!("Vortex scene root {entity:?} has no source links"))?;
    let statuses = root
        .get::<VortexConversionStatus>()
        .ok_or_else(|| anyhow!("Vortex scene root {entity:?} has no conversion statuses"))?;
    let diagnostics = root
        .get::<VortexDiagnosticSet>()
        .ok_or_else(|| anyhow!("Vortex scene root {entity:?} has no diagnostic set"))?;
    let variants = root
        .get::<VortexVariantSelection>()
        .ok_or_else(|| anyhow!("Vortex scene root {entity:?} has no variant selection"))?;

    if source_link.0.len() != statuses.0.len() {
        return Err(anyhow!("Vortex scene root {entity:?} has inconsistent source/status counts"));
    }

    let sources = source_link
        .0
        .iter()
        .zip(&statuses.0)
        .map(|(source, classification)| RuntimeSourceInspection {
            identity: source.identity.clone(),
            asset_kind: source.asset_kind.clone(),
            classification: classification.clone(),
            glb_labels: source.glb_labels.clone(),
            selected_values: variants
                .0
                .get(&source.identity.to_string())
                .cloned()
                .unwrap_or_default(),
        })
        .collect();

    Ok(RuntimeSceneInspection {
        generation: lifecycle.generation(),
        sources,
        diagnostics: diagnostics.0.clone(),
    })
}
