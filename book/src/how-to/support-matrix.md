# Inspect the support matrix

The `support-matrix` subcommand emits the authoritative list of every
capability the project has tested, with a typed classification and a
remediation hint for each row. It is the single source of truth for
"can vortex-bevy handle this input?".

## Invocation

```bash
cargo run -p vortex-bevy -- support-matrix --format json
```

JSON is the recommended format for tooling. The text format is for
human reading at a terminal.

The output is also emitted to [`docs/compatibility_matrix.json`](../../../docs/compatibility_matrix.json)
at every release cut, so any tooling that does not want to shell out to
the CLI can read the JSON file directly.

## What the rows mean

Each row has three fields:

| Field            | Type                                                                       | Meaning                                                                                                                                                    |
| ---------------- | -------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `capability`     | dotted identifier                                                          | What the row tests. Examples: `unreal.uasset.metadata_preflight`, `usd.usda.selected_stage`, `glb.deterministic_packaging`, `artifact.atomic_publication`. |
| `classification` | one of `converted`, `baked`, `requires_adapter`, `unsupported`, `rejected` | The product's documented status for this capability.                                                                                                       |
| `remediation`    | string                                                                     | What to do next when the classification is not `converted` or `baked`.                                                                                     |

The classifications are stable and machine-readable. Downstream tooling
can drive UI copy or CI gating off the classification field directly.

## Reading the classifications

| Classification     | Meaning                                                                                                                                                                                         |
| ------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `converted`        | Vortex-Bevy produces a runtime artifact from this capability end-to-end. No external adapter required.                                                                                          |
| `baked`            | An external adapter (the matching Editor, an OpenUSD runtime, etc.) is required to produce the input; once the input is in the documented shape, Vortex-Bevy converts it end-to-end.            |
| `requires_adapter` | An external adapter is required to complete the conversion. Vortex-Bevy reports the typed classification but does not bypass the adapter. The remediation names the specific adapter to invoke. |
| `unsupported`      | Vortex-Bevy does not support this capability. The remediation names a forward direction (track under T-XX family, file an issue with the input shape).                                          |
| `rejected`         | The input is unauthorized — cooked, encrypted, unowned, DRM-protected, or otherwise outside the documented boundary. Vortex-Bevy rejects it without attempting to bypass access controls.       |

The same five classifications appear in every classification site: the
CLI report, the `ConversionAggregate` in the orchestrator, and the
scene manifest. The support matrix is the at-rest view; the per-source
report is the per-invocation view.

## What the matrix does not tell you

The support matrix is a snapshot of _tested_ capabilities. It does not
list every input shape Vortex-Bevy would refuse — inputs that fall
outside the documented boundary are detected per-invocation and
classified `rejected` with a typed diagnostic. The matrix is the
positive list, not the negative list.

A `requires_adapter` row means the orchestrator knows the input is
real and the missing piece is an external adapter. It does not mean
"this will fail if you run it". For a runnable example, see the
[Tutorial: Convert a USD stage end-to-end](../tutorial/usd-end-to-end.md).

## Going further

- [Understand classification outcomes](classification-outcomes.md) — the
  full taxonomy and when each one applies
- [Reference: Supported capabilities](../reference/capabilities.md) —
  capability-level documentation
- [Reference: Compatibility matrix](../reference/compatibility-matrix.md)
