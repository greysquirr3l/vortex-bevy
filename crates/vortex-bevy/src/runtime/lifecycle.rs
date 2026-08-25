//! Explicit scene lifecycle operations for Vortex runtime roots.

use std::path::PathBuf;

use anyhow::{Result, anyhow};
use bevy::prelude::{App, Component, Entity, World};

use super::{VortexSceneRoot, load_manifest_and_spawn_in_world};

/// Runtime-owned reload state attached only to a validated Vortex scene root.
#[derive(Component, Clone, Debug, Eq, PartialEq)]
pub struct VortexSceneLifecycle {
    manifest_path: PathBuf,
    generation: u64,
}

impl VortexSceneLifecycle {
    /// Creates the first lifecycle state for a manifest-backed scene root.
    #[must_use]
    pub const fn initial(manifest_path: PathBuf) -> Self {
        Self { manifest_path, generation: 0 }
    }

    /// Returns the manifest path that can recreate this scene.
    #[must_use]
    pub const fn manifest_path(&self) -> &PathBuf {
        &self.manifest_path
    }

    /// Returns the monotonic in-process reload generation.
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    fn next_generation(&self) -> Result<u64> {
        self.generation.checked_add(1).ok_or_else(|| anyhow!("scene reload generation overflow"))
    }
}

/// Removes a validated Vortex root and lets Bevy reclaim its associated world instance.
///
/// # Errors
///
/// Returns an error when the entity is absent or was not created by the Vortex runtime.
pub fn unload_scene(app: &mut App, entity: Entity) -> Result<VortexSceneLifecycle> {
    unload_scene_in_world(app.world_mut(), entity)
}

/// Removes a validated Vortex root from an existing Bevy world.
///
/// # Errors
///
/// Returns an error when the entity is absent or was not created by the Vortex runtime.
pub fn unload_scene_in_world(world: &mut World, entity: Entity) -> Result<VortexSceneLifecycle> {
    let lifecycle = {
        let root = world
            .get_entity(entity)
            .map_err(|_| anyhow!("scene root {entity:?} does not exist"))?;
        if !root.contains::<VortexSceneRoot>() {
            return Err(anyhow!("entity {entity:?} is not a Vortex scene root"));
        }
        root.get::<VortexSceneLifecycle>()
            .cloned()
            .ok_or_else(|| anyhow!("Vortex scene root {entity:?} has no lifecycle state"))?
    };

    world.try_despawn(entity).map_err(|error| anyhow!(error))?;
    Ok(lifecycle)
}

/// Revalidates a scene's retained manifest and replaces its root with a new generation.
///
/// # Errors
///
/// Returns an error when the existing root cannot be unloaded or its manifest cannot be reloaded.
pub fn reload_scene(app: &mut App, entity: Entity) -> Result<Entity> {
    reload_scene_in_world(app.world_mut(), entity)
}

/// Revalidates a scene's retained manifest and replaces its root within an existing Bevy world.
///
/// # Errors
///
/// Returns an error when the existing root cannot be unloaded or its manifest cannot be reloaded.
pub fn reload_scene_in_world(world: &mut World, entity: Entity) -> Result<Entity> {
    let prior = unload_scene_in_world(world, entity)?;
    let next_generation = prior.next_generation()?;
    let next_entity = load_manifest_and_spawn_in_world(world, prior.manifest_path())?;
    let mut next_root = world
        .get_entity_mut(next_entity)
        .map_err(|_| anyhow!("reloaded Vortex root {next_entity:?} was not spawned"))?;
    next_root.insert(VortexSceneLifecycle {
        manifest_path: prior.manifest_path().clone(),
        generation: next_generation,
    });
    Ok(next_entity)
}
