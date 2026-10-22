# Bevy Runtime Loading Contract

## Scope

The `vortex-bevy` runtime accepts a `scene.manifest.ron` sidecar and a declared GLB from one configured Bevy asset root. It validates the sidecar before submitting any asset path to Bevy’s loader, then uses Bevy 0.20 `GltfAssetLabel::Scene(0)` and `WorldAssetRoot` to instantiate the scene.

## Validation Boundary

The runtime applies the core manifest schema and size limit, requires a Bevy `0.20` compatibility declaration, rejects runtime requests that still require Unreal Editor export, and verifies the declared GLB SHA-256 checksum. The manifest artifact path is checked again after canonicalization to prevent traversal outside the manifest directory.

A failed request does not spawn `WorldAssetRoot`. `load_published_conversion_and_spawn` first invokes the atomic artifact store’s full manifest-artifact verification for one root-confined conversion ID, then passes its resolved manifest to Bevy. `load_published_conversion_or_diagnostic` converts any such rejection into `VortexRuntimeDiagnostic` with the stable `vortex.runtime.artifact_rejected` code instead of spawning a misleading scene.

## Runtime Provenance

For a successful request, the root entity has `WorldAssetRoot` plus the following Vortex-owned components:

| Component | Retained information |
|---|---|
| `VortexSceneRoot` | Marks a Vortex-instantiated scene root. |
| `VortexSourceLink` | Typed source identities, source asset kinds, and stable GLB-label mappings. |
| `VortexConversionStatus` | Per-source conversion classifications. |
| `VortexArtifactHash` | The checksum verified before loading. |
| `VortexDiagnosticSet` | Manifest diagnostics preserved without loss. |
| `VortexVariantSelection` | Typed source-side variant selections. |

The runtime registers the standard reflected Bevy components emitted by dynamic glTF worlds—transform, visibility, hierarchy, and name components—before asset loading. This is necessary for current `WorldAssetRoot` instantiation in a headless-capable application.

## Test Contract

The headless runtime integration suite creates both a self-authored direct manifest fixture and an atomically published conversion fixture. It verifies that Bevy resolves the scene, instantiates its named child node, and retains all root provenance components. Separate tests reject checksum mismatch, unsupported capability, absent manifest, malformed manifest, and corruption after atomic publication. The published-path failure produces a `VortexRuntimeDiagnostic` rather than a second scene root.
