# Supported capabilities

This page documents the capabilities the project has tested, with the
typed classification each one carries and the remediation the runtime
surfaces when the classification is not `converted` or `baked`.

For the at-rest JSON form, see
[Compatibility matrix](compatibility-matrix.md) or run
`vortex-bevy support-matrix --format json`.

## Engine-specific capabilities

### Unreal

| Capability                                          | Classification     | Notes                                                                                                                                                                                                                                                      |
| --------------------------------------------------- | ------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `unreal.uasset.metadata_preflight`                  | `requires_adapter` | Direct `.uasset` decoding is metadata-only by product design (T08). The bounded preflight extracts stable facts from the package header and a small set of known tag tables. Authoritative export requires the matching authorized Unreal Editor bridge.   |
| `unreal.utexture2d.ue5_8_2_asset_registry_metadata` | `requires_adapter` | One legacy Asset Registry record, bounded dimensions / format / compression / filter / hash / bulk-reference state. Never reads pixels, object payloads, or bulk data. Still `requires_adapter` for authoritative export.                                  |
| `unreal.editor_bridge.protocol`                     | `requires_adapter` | The checked-in bridge (`bridges/unreal/vortex_unreal_bridge.py`) is invoked only against an authorized matching Unreal Editor project. The CLI builds the bridge command, validates the versioned report, and maps each record to a typed source identity. |
| `unreal.cooked_or_encrypted_content`                | `rejected`         | Cooked, encrypted, unowned, DRM-protected, or otherwise unauthorized content is rejected without bypassing access controls.                                                                                                                                |

### USD

| Capability                             | Classification     | Notes                                                                                                                                                                   |
| -------------------------------------- | ------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `usd.usda.selected_stage`              | `requires_adapter` | The bounded adapter resolves selected, root-confined USDA text. Binary/plugin-backed layers (`.usdc`, non-text `.usd`) require an OpenUSD runtime for full composition. |
| `usd.package.flattended`               | `converted`        | The deterministic flattened fallback packages a USDA stage as a single GLB with a versioned manifest.                                                                   |
| `usd.package.prepackaged_variants`     | `converted`        | Explicit prepackaged named variants each publish atomically under one manifest. The Bevy runtime can switch only among verified prebuilt alternatives.                  |
| `usd.package.expanded_instances`       | `converted`        | Expanded and shared instance batches publish atomically under one manifest.                                                                                             |
| `usd.package.quantized_bounds_chunks`  | `converted`        | Quantized-bounds chunks publish atomically. The runtime loads bounded chunks with retained provenance.                                                                  |
| `usd.composition.payload_arcs`         | `rejected`         | Payload arcs are detected and recorded as unselected facts; payload evaluation is out of scope for the bounded adapter.                                                 |
| `usd.composition.recursive_references` | `rejected`         | Recursive references fail closed; the bounded adapter does not attempt recursive resolution.                                                                            |

### Unity

| Capability               | Classification     | Notes                                                                                                                                                            |
| ------------------------ | ------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `unity.urp_text_project` | `requires_adapter` | Force Text source inspection covers metadata only. Authoritative static-scene export requires the matching Unity Editor's batch-mode bridge (`bridges/unity/*`). |
| `unity.urp.lit_subset`   | `converted`        | The URP Lit subset (base color, metallic, smoothness, normal scale, occlusion strength, emissive) maps deterministically to a portable PBR representation.       |

### Godot

| Capability            | Classification     | Notes                                                                                                                                                   |
| --------------------- | ------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `godot.tscn_metadata` | `requires_adapter` | Direct `.tscn` parsing covers metadata only. Authoritative static-scene export requires a Godot adapter expansion that is currently tracked separately. |

## Cross-cutting capabilities

| Capability                        | Classification | Notes                                                                                                                                                                                                        |
| --------------------------------- | -------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `glb.deterministic_packaging`     | `converted`    | The validated canonical scene IR packs to a deterministic glTF 2.0 core subset GLB.                                                                                                                          |
| `texture.normalization`           | `converted`    | Owned-fixture texture normalization handles PNG and TGA inputs and emits normalized GLB-embeddable textures.                                                                                                 |
| `artifact.atomic_publication`     | `converted`    | The atomic artifact store stages, verifies, fsyncs, and renames; failed attempts clean their staging directory and never replace an existing final conversion directory.                                     |
| `runtime.bevy_glb_load`           | `converted`    | The Bevy 0.19 runtime verifies the manifest and GLB checksum before spawning `WorldAssetRoot`, then retains source provenance, classifications, diagnostics, variants, and artifact hash on the root entity. |
| `runtime.lifecycle.reload_unload` | `converted`    | Runtime roots expose explicit reload and unload operations that preserve the data-driven viewer boundary.                                                                                                    |
| `runtime.bevy_egui_viewer`        | `converted`    | The local `bevy_egui` inspector queues lifecycle actions and shares a clipped native 3D viewport.                                                                                                            |

## Going further

- [How-to: Inspect the support matrix](../how-to/support-matrix.md)
- [Reference: Compatibility matrix](compatibility-matrix.md)
- [Reference: Scene manifest schema](scene-manifest.md)
