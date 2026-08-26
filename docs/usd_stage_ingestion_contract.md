# Bounded USDA Stage Ingestion Contract

## Scope

`vortex-core::adapters::secondary::usd_stage` resolves a single caller-selected, root-confined stage file into owned, deterministic facts. The initial adapter accepts UTF-8 USDA text and identifies binary/plugin-backed `.usdc` or non-text `.usd` inputs as `requires_adapter`; it does not parse their contents. The implementation is intentionally not a replacement for the authoritative OpenUSD composition engine.

> OpenUSD composition can combine layers, references, payloads, and variants into a composed scenegraph. The initial Vortex adapter records those arcs and the caller’s selection rather than claiming live composition semantics in Bevy. [1]

## Stage selection

`UsdStageRequest` accepts a semicolon-delimited selection expression with these keys. Every token is parsed and normalized before it crosses the adapter boundary.

| Key | Values | Canonical fact |
|---|---|---|
| `variants` | Empty or comma-separated `name:selection` pairs | One sorted `selection.variant.<name>=<selection>` fact per selection |
| `payload` | `forbid` or `metadata_only` | `selection.payload=<policy>` |
| `purpose` | ASCII token | `selection.purpose=<token>` |
| `time` | `stage` or inclusive integer `start:end` | `selection.time=<range>` |
| `context` | ASCII token | `selection.render_context=<token>` |

The normalized key order makes semantically identical selections produce the same `StageSelection` identity and snapshot output even when callers provide a different input order.

## Bounded USDA facts

The adapter enforces a 4 MiB source bound, valid UTF-8, a `#usda 1.0` header, balanced metadata parentheses/braces, root-constrained canonical paths, and a root `upAxis` of `Y` or `Z`. It owns all prim facts and never exposes parser/FFI lifetimes across the adapter boundary.

| USDA authoring | Result |
|---|---|
| `metersPerUnit` | Parsed as a positive finite value; unauthored input records the OpenUSD centimeter fallback of `0.01`. [2] |
| `upAxis` | Validated as `Y` or `Z`; `Z` records `normalization.up_axis=z_to_y_pending_ir`, while actual coordinate transformation waits for canonical IR. [3] |
| `subLayers`, `references`, `payload` | Emitted as sorted `composition.*` facts; payload content is never loaded by this adapter. |
| `variantSet` | Emitted as `composition.variant.unselected` facts. Caller selection is separately preserved. |
| Prim declarations | Emits canonical hierarchy paths, specifier/type, authored purpose/visibility, `instanceable`, and source-order transform operation lines. |

The snapshot classification is `requires_adapter`: stage inspection is complete, but portable canonical-scene conversion belongs to the next recovery stage. Unsupported/binary inputs are also explicit `requires_adapter` outcomes with `stage.composition=requires_authoritative_runtime`.

## Safety and determinism

The root layer must remain beneath the authorized canonical root after symlink resolution. A malformed stage produces a typed error and no partial snapshot. Sorting of selection facts, composition facts, and prim paths makes the same selected source deterministic independent of traversal order.

The self-authored suite in `crates/vortex-core/tests/usd_stage_ingestion.rs` covers hierarchy, metadata, transforms, variants, payload/reference/sublayer facts, selection normalization, malformed metadata/structure, binary fallback, and symlink escape. It does **not** execute an OpenUSD runtime or claim equivalent composed rendering.

## References

[1]: https://openusd.org/release/usdfaq.html "OpenUSD FAQ — composition and USDA"
[2]: https://openusd.org/24.08/api/group___usd_geom_linear_units__group.html "OpenUSD — Encoding Stage Linear Units"
[3]: https://openusd.org/release/api/group___usd_geom_up_axis__group.html "OpenUSD — Encoding Stage UpAxis"
