//! Bounded manifest and artifact verification for the Bevy runtime adapter.

use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, anyhow, bail};
use vortex_core::{
    adapters::secondary::artifact_store::AtomicArtifactStore,
    domain::{
        ArtifactHash, CompatibilityCapability, ConversionId, DEFAULT_MANIFEST_LIMIT_BYTES,
        ManifestArtifactKind, PackagedSceneMode, SceneManifest,
    },
};

/// Resolves the directory a published conversion's assets are served from.
///
/// A manifest declares its artifacts relative to the conversion directory
/// that owns it, so the Bevy asset server's root has to be that directory.
/// Pointing the asset server at the artifact root instead makes every
/// relative artifact path resolve to a file that does not exist.
///
/// # Errors
///
/// Returns an error when the conversion's published manifest cannot be
/// resolved beneath the store's root, or when that manifest has no parent
/// directory.
pub fn published_conversion_asset_root(
    store: &AtomicArtifactStore,
    conversion_id: &ConversionId,
) -> Result<PathBuf> {
    let manifest_path = store
        .published_manifest_path(conversion_id)
        .with_context(|| format!("cannot resolve published manifest for {conversion_id}"))?;
    manifest_path.parent().map(Path::to_path_buf).ok_or_else(|| {
        anyhow!("published manifest {} has no parent directory", manifest_path.display())
    })
}

/// A Vortex artifact verified before it is handed to Bevy's asset pipeline.
#[derive(Clone, Debug)]
pub struct VerifiedVortexArtifact {
    /// Parsed, schema-validated manifest.
    pub manifest: SceneManifest,
    /// Asset-server-relative GLB path.
    pub glb_asset_path: String,
    /// Canonical on-disk GLB path whose bytes were verified.
    pub glb_path: PathBuf,
    /// Declared checksum verified against GLB bytes.
    pub glb_hash: ArtifactHash,
}

/// Parses and verifies a manifest and its declared GLB without starting Bevy asset loading.
///
/// # Errors
///
/// Returns an error for an inaccessible, oversized, malformed, incompatible, or unsafe manifest;
/// a missing GLB; or a declared checksum mismatch.
pub fn verify_manifest(manifest_path: &Path) -> Result<VerifiedVortexArtifact> {
    let manifest_path = manifest_path
        .canonicalize()
        .with_context(|| format!("cannot canonicalize manifest {}", manifest_path.display()))?;
    let metadata = fs::metadata(&manifest_path)
        .with_context(|| format!("cannot inspect manifest {}", manifest_path.display()))?;
    let bytes =
        usize::try_from(metadata.len()).context("manifest size does not fit this platform")?;
    if bytes > DEFAULT_MANIFEST_LIMIT_BYTES {
        bail!("manifest exceeds the {DEFAULT_MANIFEST_LIMIT_BYTES}-byte runtime limit");
    }
    let manifest_bytes = fs::read(&manifest_path)
        .with_context(|| format!("cannot read manifest {}", manifest_path.display()))?;
    let manifest = SceneManifest::from_ron(&manifest_bytes, DEFAULT_MANIFEST_LIMIT_BYTES)
        .context("manifest contract validation failed")?;
    ensure_bevy_020(&manifest.compatibility.minimum_bevy_version)?;
    ensure_supported_capabilities(&manifest.compatibility.required_capabilities)?;

    let glb = declared_initial_glb(&manifest)?;
    let glb_asset_path = glb.path.clone();
    let glb_hash = glb.hash.clone();
    let Some(parent) = manifest_path.parent() else {
        bail!("manifest has no parent artifact directory");
    };
    let artifact_root = parent
        .canonicalize()
        .with_context(|| format!("cannot canonicalize artifact root {}", parent.display()))?;
    let glb_path = parent.join(&glb_asset_path);
    let glb_path = glb_path
        .canonicalize()
        .with_context(|| format!("cannot canonicalize declared GLB {glb_asset_path}"))?;
    if !glb_path.starts_with(&artifact_root) {
        bail!("declared GLB escapes the manifest artifact directory");
    }
    let glb_bytes = fs::read(&glb_path)
        .with_context(|| format!("cannot read declared GLB {}", glb_path.display()))?;
    let actual_hash = ArtifactHash::from_bytes(&glb_bytes);
    if actual_hash != glb_hash {
        bail!("declared GLB checksum does not match artifact bytes");
    }

    Ok(VerifiedVortexArtifact { manifest, glb_asset_path, glb_path, glb_hash })
}

fn declared_initial_glb(
    manifest: &SceneManifest,
) -> Result<&vortex_core::domain::ManifestArtifact> {
    if let Some(flattened) =
        manifest.packaged_scenes.iter().find(|package| package.mode == PackagedSceneMode::Flattened)
    {
        return manifest
            .artifacts
            .iter()
            .find(|artifact| {
                artifact.kind == ManifestArtifactKind::Glb
                    && artifact.path == flattened.artifact_path
            })
            .ok_or_else(|| {
                anyhow!("manifest flattened package declares no matching GLB artifact")
            });
    }
    manifest
        .artifacts
        .iter()
        .find(|artifact| artifact.kind == ManifestArtifactKind::Glb)
        .ok_or_else(|| anyhow!("manifest declares no GLB artifact"))
}

fn ensure_bevy_020(version: &str) -> Result<()> {
    let mut parts = version.trim().split('.');
    let major = parts.next().unwrap_or_default();
    let minor = parts.next().unwrap_or_default();
    if (major, minor) != ("0", "20") {
        bail!("manifest requires Bevy {version}, but this runtime supports Bevy 0.20");
    }
    Ok(())
}

fn ensure_supported_capabilities(
    capabilities: &std::collections::BTreeSet<CompatibilityCapability>,
) -> Result<()> {
    for capability in capabilities {
        match capability {
            CompatibilityCapability::BevyGltfPbr
            | CompatibilityCapability::BevyPunctualLights
            | CompatibilityCapability::BevyAnimations
            | CompatibilityCapability::UrpLitSubset
            | CompatibilityCapability::UsdPreviewSurface
            | CompatibilityCapability::UsdFlattenedStage
            | CompatibilityCapability::UsdPackagedVariants
            | CompatibilityCapability::UsdExpandedInstances
            | CompatibilityCapability::UsdInstanceBatches
            | CompatibilityCapability::UsdSceneChunks => {}
            CompatibilityCapability::UnrealEditorExport => {
                bail!(
                    "runtime artifacts must not require an Unreal Editor export capability at load time"
                );
            }
        }
    }
    Ok(())
}
