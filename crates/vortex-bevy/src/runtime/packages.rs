//! Runtime loading for explicitly prepackaged scene alternatives and chunks.

use std::collections::BTreeMap;

use anyhow::{Result, anyhow, bail};
use bevy::{
    asset::AssetServer,
    gltf::GltfAssetLabel,
    prelude::{App, Entity, World, WorldAssetRoot},
};
use vortex_core::domain::{PackagedScene, PackagedSceneId, PackagedSceneMode, SourceIdentity};

use super::{
    VortexConversionStatus, VortexDiagnosticSet, VortexPackagedScenes, VortexPackagedVariant,
    VortexSceneChunk, VortexSceneLifecycle, VortexSceneRoot, VortexSourceLink, VortexSourceRecord,
    VortexVariantSelection,
};

/// Switches one root to an explicitly prepackaged variant alternative.
///
/// Existing instances of a previously selected alternative for the same root are removed. The
/// root itself, its lifecycle, and unrelated scene roots remain intact.
///
/// # Errors
///
/// Returns an error when the root has no package catalog, the requested identifier is absent or
/// not a variant alternative, source provenance cannot be retained, or the runtime lacks an asset
/// server.
pub fn switch_packaged_variant(
    app: &mut App,
    root: Entity,
    package_id: &PackagedSceneId,
) -> Result<Entity> {
    switch_packaged_variant_in_world(app.world_mut(), root, package_id)
}

/// Switches one root to an explicitly prepackaged variant alternative in an existing world.
///
/// # Errors
///
/// Returns an error under the same conditions as [`switch_packaged_variant`].
pub fn switch_packaged_variant_in_world(
    world: &mut World,
    root: Entity,
    package_id: &PackagedSceneId,
) -> Result<Entity> {
    let context = root_context(world, root)?;
    let package = find_package(&context, package_id, PackagedSceneMode::VariantAlternative)?;
    if let Some(entity) = existing_variant(world, root, package_id) {
        return Ok(entity);
    }
    let next = spawn_package(world, root, package.clone(), PackageSpawnKind::Variant, &context)?;
    if let Err(error) =
        update_root_selection(world, root, &package.source_links, &package.selected_values)
    {
        world.try_despawn(next).map_err(|despawn| anyhow!(despawn))?;
        return Err(error);
    }
    let previous = world
        .query::<(Entity, &VortexPackagedVariant)>()
        .iter(world)
        .filter_map(|(entity, variant)| {
            (variant.parent == root && entity != next).then_some(entity)
        })
        .collect::<Vec<_>>();
    for entity in previous {
        world.try_despawn(entity).map_err(|error| anyhow!(error))?;
    }
    Ok(next)
}

/// Loads one explicitly packaged spatial chunk without replacing its parent root or variants.
///
/// # Errors
///
/// Returns an error when the root/package relationship is invalid, provenance cannot be retained,
/// or the runtime lacks an asset server.
pub fn load_packaged_chunk(
    app: &mut App,
    root: Entity,
    package_id: &PackagedSceneId,
) -> Result<Entity> {
    load_packaged_chunk_in_world(app.world_mut(), root, package_id)
}

/// Loads one explicitly packaged spatial chunk in an existing world.
///
/// # Errors
///
/// Returns an error under the same conditions as [`load_packaged_chunk`].
pub fn load_packaged_chunk_in_world(
    world: &mut World,
    root: Entity,
    package_id: &PackagedSceneId,
) -> Result<Entity> {
    let context = root_context(world, root)?;
    let package = find_package(&context, package_id, PackagedSceneMode::Chunk)?;
    if let Some(entity) = existing_chunk(world, root, package_id) {
        return Ok(entity);
    }
    spawn_package(world, root, package, PackageSpawnKind::Chunk, &context)
}

#[derive(Clone)]
struct PackageRootContext {
    packages: Vec<PackagedScene>,
    sources: VortexSourceLink,
    statuses: VortexConversionStatus,
    diagnostics: VortexDiagnosticSet,
}

fn root_context(world: &World, root: Entity) -> Result<PackageRootContext> {
    let entity =
        world.get_entity(root).map_err(|_| anyhow!("Vortex root {root:?} does not exist"))?;
    if !entity.contains::<VortexSceneRoot>() {
        bail!("entity {root:?} is not a Vortex scene root");
    }
    if entity.get::<VortexSceneLifecycle>().is_none() {
        bail!("Vortex root {root:?} has no lifecycle state");
    }
    let packages = entity
        .get::<VortexPackagedScenes>()
        .map(|catalog| catalog.0.clone())
        .ok_or_else(|| anyhow!("Vortex root {root:?} has no packaged-scene catalog"))?;
    let sources = entity
        .get::<VortexSourceLink>()
        .cloned()
        .ok_or_else(|| anyhow!("Vortex root {root:?} has no source provenance"))?;
    let statuses = entity
        .get::<VortexConversionStatus>()
        .cloned()
        .ok_or_else(|| anyhow!("Vortex root {root:?} has no conversion status"))?;
    let diagnostics = entity
        .get::<VortexDiagnosticSet>()
        .cloned()
        .ok_or_else(|| anyhow!("Vortex root {root:?} has no diagnostic set"))?;
    Ok(PackageRootContext { packages, sources, statuses, diagnostics })
}

fn existing_variant(
    world: &mut World,
    root: Entity,
    package_id: &PackagedSceneId,
) -> Option<Entity> {
    world.query::<(Entity, &VortexPackagedVariant)>().iter(world).find_map(|(entity, variant)| {
        (variant.parent == root && variant.id == *package_id).then_some(entity)
    })
}

fn existing_chunk(world: &mut World, root: Entity, package_id: &PackagedSceneId) -> Option<Entity> {
    world.query::<(Entity, &VortexSceneChunk)>().iter(world).find_map(|(entity, chunk)| {
        (chunk.parent == root && chunk.id == *package_id).then_some(entity)
    })
}

fn find_package(
    context: &PackageRootContext,
    package_id: &PackagedSceneId,
    mode: PackagedSceneMode,
) -> Result<PackagedScene> {
    let package = context
        .packages
        .iter()
        .find(|package| package.id == *package_id)
        .cloned()
        .ok_or_else(|| anyhow!("packaged scene {package_id} is not declared by this root"))?;
    if package.mode != mode {
        bail!("packaged scene {package_id} is not a {mode:?} package");
    }
    Ok(package)
}

fn update_root_selection(
    world: &mut World,
    root: Entity,
    source_links: &[SourceIdentity],
    selected_values: &BTreeMap<String, String>,
) -> Result<()> {
    let current = world
        .get_entity(root)
        .map_err(|_| anyhow!("Vortex root {root:?} disappeared during package selection"))?
        .get::<VortexVariantSelection>()
        .cloned()
        .ok_or_else(|| anyhow!("Vortex root {root:?} has no variant selection state"))?;
    let mut selections = current.0;
    for source in source_links {
        selections.insert(source.to_string(), selected_values.clone());
    }
    world
        .get_entity_mut(root)
        .map_err(|_| anyhow!("Vortex root {root:?} disappeared while updating selection"))?
        .insert(VortexVariantSelection(selections));
    Ok(())
}

#[derive(Clone, Copy)]
enum PackageSpawnKind {
    Variant,
    Chunk,
}

fn spawn_package(
    world: &mut World,
    root: Entity,
    package: PackagedScene,
    kind: PackageSpawnKind,
    context: &PackageRootContext,
) -> Result<Entity> {
    let records = selected_records(&context.sources.0, &package.source_links)?;
    let path = package.artifact_path.clone();
    let handle = {
        let asset_server = world
            .get_resource::<AssetServer>()
            .ok_or_else(|| anyhow!("Vortex runtime world has no AssetServer"))?;
        asset_server.load(GltfAssetLabel::Scene(0).from_asset(path))
    };
    let selections = package_selection(&package);
    let mut commands = world.spawn((
        WorldAssetRoot(handle),
        VortexSourceLink(records),
        context.statuses.clone(),
        context.diagnostics.clone(),
        VortexVariantSelection(selections),
    ));
    match kind {
        PackageSpawnKind::Variant => {
            commands.insert(VortexPackagedVariant { parent: root, id: package.id });
        }
        PackageSpawnKind::Chunk => {
            commands.insert(VortexSceneChunk { parent: root, id: package.id });
        }
    }
    Ok(commands.id())
}

fn selected_records(
    records: &[VortexSourceRecord],
    source_links: &[SourceIdentity],
) -> Result<Vec<VortexSourceRecord>> {
    source_links
        .iter()
        .map(|source| {
            records
                .iter()
                .find(|record| record.identity == *source)
                .cloned()
                .ok_or_else(|| anyhow!("package source {source} is absent from root provenance"))
        })
        .collect()
}

fn package_selection(package: &PackagedScene) -> BTreeMap<String, BTreeMap<String, String>> {
    package
        .source_links
        .iter()
        .map(|source| (source.to_string(), package.selected_values.clone()))
        .collect()
}
