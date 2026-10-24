# Compatibility matrix

The compatibility matrix is the at-rest JSON form of the support matrix.
The CLI's `support-matrix --format json` command emits the same
document; the file at [`docs/compatibility_matrix.json`](../../../docs/compatibility_matrix.json)
is a snapshot of that document at every release cut.

This page is a curated, operator-oriented summary. For authoritative
machine-readable data, always run the CLI or read the JSON file.

## Engine summary

| Input route                               | Exact tested basis                                 | Output route                                                                              | Classification and limit                                                                                                                                                                   |
| ----------------------------------------- | -------------------------------------------------- | ----------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Authorized Unreal controlled static scene | Unreal Engine 5.8.2 Editor bridge evidence         | Bounded FBX/TGA ingestion → canonical IR → deterministic GLB → manifest → headless Bevy   | Static-scene subset can convert. Worlds, Blueprints/C++, broad material graphs, collision/physics, and unsupported FBX features remain explicit boundaries.                                |
| Direct Unreal `.uasset` metadata          | UE 5.8.2; one legacy Asset Registry record         | Typed metadata facts only                                                                 | `requires_adapter`; no pixel/object/bulk payload read, `.ubulk` decode, or semantic export.                                                                                                |
| Selected OpenUSD stage                    | Root-confined text USDA facts                      | Typed stage facts and package planning                                                    | `requires_adapter` for full composition. Binary/plugin-backed layers, payload evaluation, recursive references, and arbitrary live variants require an authorized OpenUSD runtime adapter. |
| USD package alternatives                  | Owned synthetic and documented package paths       | Prebuilt flattened, variant, instance, batch, and chunk GLBs                              | Verified prepackaged selections only; runtime does not author arbitrary USD variants or composition.                                                                                       |
| Selected Unity URP Force Text project     | Bounded project facts                              | Typed URP source facts and mapping                                                        | `requires_adapter` for authoritative import resolution; the URP Lit subset maps deterministically.                                                                                         |
| Selected Unity static scene               | Authorized Unity Editor batch-mode bridge evidence | Bounded static-scene report → canonical IR → deterministic GLB → manifest → headless Bevy | Static-scene subset can convert.                                                                                                                                                           |

## Capability matrix (curated)

| Capability                                          | Classification     |
| --------------------------------------------------- | ------------------ |
| `unreal.uasset.metadata_preflight`                  | `requires_adapter` |
| `unreal.utexture2d.ue5_8_2_asset_registry_metadata` | `requires_adapter` |
| `unreal.editor_bridge.protocol`                     | `requires_adapter` |
| `unreal.cooked_or_encrypted_content`                | `rejected`         |
| `usd.usda.selected_stage`                           | `requires_adapter` |
| `usd.package.flattended`                            | `converted`        |
| `usd.package.prepackaged_variants`                  | `converted`        |
| `usd.package.expanded_instances`                    | `converted`        |
| `usd.package.quantized_bounds_chunks`               | `converted`        |
| `usd.composition.payload_arcs`                      | `rejected`         |
| `usd.composition.recursive_references`              | `rejected`         |
| `unity.urp_text_project`                            | `requires_adapter` |
| `unity.urp.lit_subset`                              | `converted`        |
| `godot.tscn_metadata`                               | `requires_adapter` |
| `glb.deterministic_packaging`                       | `converted`        |
| `texture.normalization`                             | `converted`        |
| `artifact.atomic_publication`                       | `converted`        |
| `runtime.bevy_glb_load`                             | `converted`        |
| `runtime.lifecycle.reload_unload`                   | `converted`        |
| `runtime.bevy_egui_viewer`                          | `converted`        |

For the full machine-readable list and the remediation strings, see
[`docs/compatibility_matrix.json`](../../../docs/compatibility_matrix.json)
or run `vortex-bevy support-matrix --format json`.

## What the matrix does not tell you

- It does not list every input shape Vortex-Bevy would refuse. Inputs
  that fall outside the documented boundary are detected per-invocation
  and classified `rejected` with a typed diagnostic.
- It does not claim equivalence for unsupported engine systems.
  Blueprint/C++ behavior conversion, custom shader graphs, arbitrary
  USD composition, and nonrepresentable URP renderer effects remain
  explicit boundaries.
- It does not promise compatibility with Editor versions newer than
  what the bridge scripts have been tested against. The bridge
  protocols are versioned; new Editor versions may require bridge
  updates before the matrix is updated.

## Going further

- [How-to: Inspect the support matrix](../how-to/support-matrix.md)
- [Reference: Supported capabilities](capabilities.md)
- [Understand classification outcomes](../how-to/classification-outcomes.md)
