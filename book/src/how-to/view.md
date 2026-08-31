# View a published conversion

The `view` subcommand is reserved for the Bevy 0.19 desktop viewer. It
opens the published manifest + GLB in a native window with a
`bevy_egui` overlay for source inspection, scene lifecycle, and
diagnostics.

This page covers the viewer's contract and what it shows. The runtime
side of the loading path is documented at
[`docs/bevy_runtime_loading_contract.md`](../../docs/bevy_runtime_loading_contract.md).

## Status

The viewer is implemented at the runtime level — the headless Bevy
loader, manifest verifier, source-provenance component set, lifecycle
operations, and the egui inspector panel — and is exercised by the
integration suite in `crates/vortex-bevy/tests/`.

The `view` CLI dispatch currently returns `Unavailable` with the
remediation "Pass a verified published artifact root after recovery
completes publication." This is intentional: the CLI command wires
through the same dispatch path as `convert`, and the artifact discovery
boundary is part of the recovery sequence. Until that boundary lands,
the headless Bevy tests in the integration suite are the authoritative
proof that the runtime can load a published manifest.

## The runtime contract (works today)

Even without the CLI dispatch, the runtime side is fully wired. To load
a published conversion programmatically:

```rust,ignore
use vortex_bevy::runtime::{
    build_runtime_app,
    load_published_conversion_and_spawn,
    spawn_runtime_diagnostic,
};
use vortex_core::adapters::secondary::artifact_store::AtomicArtifactStore;
use vortex_core::domain::ConversionId;

let mut app = build_runtime_app("/path/to/output_root");
let store = AtomicArtifactStore::new("/path/to/output_root", "output".parse()?)?;
let conversion_id = ConversionId::from_str("...")?;

// Loads the manifest, verifies the GLB hash, spawns WorldAssetRoot.
// Returns the root entity on success.
let root = load_published_conversion_and_spawn(&mut app, &store, &conversion_id)?;

// On failure, spawns a VortexRuntimeDiagnostic instead of a misleading scene.
let _entity = vortex_bevy::runtime::load_published_conversion_or_diagnostic(
    &mut app,
    &store,
    &conversion_id,
);
```

`load_published_conversion_and_spawn` returns `Result<Entity>` — the
runtime fails closed rather than spawning a misconfigured scene. The
diagnostic variant renders a typed error entity with a stable
`vortex.runtime.artifact_rejected` code so the inspector panel can show
the user what went wrong without leaving the app in a broken state.

## What the runtime preserves

For a successful load, the root entity has `WorldAssetRoot` plus the
following Vortex-owned components:

| Component                | Retained information                                                        |
| ------------------------ | --------------------------------------------------------------------------- |
| `VortexSceneRoot`        | Marks a Vortex-instantiated scene root.                                     |
| `VortexSourceLink`       | Typed source identities, source asset kinds, and stable GLB-label mappings. |
| `VortexConversionStatus` | Per-source conversion classifications.                                      |
| `VortexArtifactHash`     | The checksum verified before loading.                                       |
| `VortexDiagnosticSet`    | Manifest diagnostics preserved without loss.                                |
| `VortexVariantSelection` | Typed source-side variant selections.                                       |

The runtime registers the standard reflected Bevy components emitted by
dynamic glTF worlds (transform, visibility, hierarchy, name components)
before asset loading. This is necessary for current `WorldAssetRoot`
instantiation in a headless-capable application.

## What the viewer shows

The `bevy_egui` overlay (when wired through the desktop viewer) is
composed from four panels around the native Bevy viewport:

- **Top toolbar** — reload and unload for the selected verified Vortex
  root, plus a Frame action that traverses the loaded hierarchy and
  fits the camera to the bounding sphere.
- **Left source panel** — the retained manifest sources, filtered by
  classification.
- **Right inspector** — selected source identity, kind, classification,
  selected variants, and GLB source labels.
- **Bottom diagnostics panel** — actionable diagnostics filtered by
  their stable code prefix.

Pointer-owned UI input is gated: native camera interaction is suppressed
while an `egui` control owns pointer input. The camera viewport is
recomputed from measured panel extents and converted through the window
scale factor before being applied as a physical Bevy camera viewport.

The native desktop window installs exactly one `VortexViewerCamera`.
**Right drag** orbits around the target, **middle drag** pans in the
camera orientation plane, and the mouse wheel applies exponential
bounded zoom. **F** (or the Frame toolbar action) frames the selected
verified root by averaging valid global-transform positions and applying
a conservative radius.

## Going further

- [Build a platform bundle](./bundle.md) — wraps the binary in a
  platform-native `.app` / MSI / `.deb` so AppKit / win32 / X11 grant
  the foreground GUI session the winit window needs.
- [Reference: Bevy runtime loading contract](../../docs/bevy_runtime_loading_contract.md)
- [Reference: bevy_egui viewer contract](../../docs/bevy_egui_viewer_contract.md)
- [Reference: Runtime scene lifecycle contract](../../docs/runtime_scene_lifecycle_contract.md)
