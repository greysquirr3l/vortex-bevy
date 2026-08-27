# Packaged USD Runtime Contract

T25 converts **explicitly selected and prebuilt** USD scene views into independently loadable GLB artifacts. It preserves a deterministic flattened GLB as the required fallback. It does not evaluate OpenUSD composition inside the Bevy runtime.

## Supported package modes

| Mode | Package relationship | Runtime behavior | Capability |
|---|---|---|---|
| `Flattened` | Exactly one root package | Initial verified scene loaded from the parent manifest | `usd.packaged_flattened_stage` |
| `VariantAlternative` | Child of the flattened package | A typed command replaces only the prior selected child scene after the next package is spawned | `usd.packaged_variant_alternatives` |
| `ExpandedInstances` | Child of the flattened package | Instance attachments are materialized before GLB packaging | `usd.expanded_instances` |
| `InstanceBatch` | Child of the flattened package | Canonical shared instances lower to ordinary glTF nodes that reference a shared mesh | `usd.instance_batches` |
| `Chunk` | Child of the flattened package with quantized bounds | A typed command loads the chunk without replacing the parent root or selected variant | `usd.scene_chunks` |

Every package has a typed package ID, a safe relative GLB path, retained source links, and an explicit relationship to the flattened package. Package IDs, ordering, chunk IDs, and serialized manifest ordering are deterministic.

> Runtime switching is a selection operation over verified artifacts, not a request to evaluate a live USD variant set.

## Publication and runtime boundaries

The package planner emits a GLB per declared mode. The caller includes all package GLBs in one `PublicationRequest`; the atomic artifact store publishes, hashes, and verifies the full package set together. The root manifest carries `PackagedScene` declarations and requires the matching compatibility capability for each mode.

At runtime, manifest verification selects the declared flattened package rather than choosing an artifact by path order. The root entity retains `VortexPackagedScenes`, `VortexVariantSelection`, normal source links, conversion status, and diagnostics. `switch_packaged_variant` is idempotent for an already loaded selection; otherwise it spawns the new package first, records the selection, and only then despawns the previous variant child. `load_packaged_chunk` is also idempotent and retains `VortexSceneChunk` provenance.

## Composition and resource policy

The stage inspector continues to record unresolved composition arcs as source facts. The prepackager accepts only materialized `CanonicalScene` inputs for a specific selected configuration. It therefore fails closed when source facts expose a payload arc, an unselected variant set, or a direct recursive reference. A cached payload is not implemented: payload content must be authoritatively resolved and flattened before it reaches this boundary.

| Guard | Default limit | Failure behavior |
|---|---:|---|
| Package declarations | 64 | Reject before GLB allocation |
| Variant alternatives | 32 | Reject before GLB allocation |
| Explicit instances per package | 16,384 | Reject before GLB allocation |
| Aggregate estimated scene bytes | 256 MiB | Reject before GLB allocation |
| Payload arcs | 0 accepted | Require authoritative preflattening |
| Direct recursive references | 0 accepted | Reject with a source-path diagnostic |

Chunk bounds use signed quantized millimetres with non-empty axes. Their stable IDs are derived from the typed parent package ID, USD source identity, and quantized bounds. This is a packaging boundary rather than camera-driven streaming or arbitrary spatial partitioning.

## Intentional non-goals

The implementation does not claim universal OpenUSD composition, live variant evaluation, dynamic payload fetching, arbitrary recursive-reference resolution, per-instance dynamic dispatch, or automatic authoring of chunk layouts. Unsupported source composition remains visible as a classification and remediation rather than being silently flattened or discarded.

The official composition terminology and authoring model that constrain this boundary are recorded in [`usd_packaging_api_sources.md`](usd_packaging_api_sources.md).
