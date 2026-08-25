# Runtime Scene Lifecycle Contract

## Scope

A validated Vortex manifest creates one `WorldAssetRoot` with Vortex-owned provenance components and `VortexSceneLifecycle`. The lifecycle record retains only the validated manifest path and an in-process generation counter. It does not retain conversion handlers or mutable conversion state.

## Operations

| Operation | Behavior |
|---|---|
| `inspect_scene` | Reads a detached snapshot of source identities, asset kinds, GLB labels, classifications, selected variants, diagnostics, and reload generation. Presentation code can filter diagnostics by stable code prefix and sources by classification. |
| `unload_scene` | Requires a live `VortexSceneRoot`, removes that root through Bevy’s entity API, and returns its retained manifest path/generation. Bevy’s `WorldAssetRoot` lifecycle then reclaims the associated dynamic world instance. |
| `reload_scene` | Unloads a validated root, revalidates its original manifest and GLB checksum through the same loader, spawns a replacement root, and increments the in-process generation. |

A non-Vortex entity cannot be unloaded through this boundary. A malformed or changed artifact still fails before a replacement root is spawned, because reload calls the manifest loader rather than bypassing validation.

## Viewer Boundary

The inspection structures are plain owned models rather than UI state or conversion systems. A desktop viewer can display source tree rows, conversion classification filters, selected variants, and diagnostics without importing conversion handlers into Bevy systems. The selected source variant is shown as flattened artifact provenance; this contract does not expose a nonfunctional runtime variant-switching control.

## Test Contract

Headless runtime tests load a self-authored GLB through Bevy 0.19, verify the named child node and Vortex provenance root, inspect an intentionally unsupported classification and visible diagnostic, reload to generation one, reject an unrelated entity, and finally unload without leaving a Vortex root in the world.
