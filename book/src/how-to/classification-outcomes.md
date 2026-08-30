# Understand classification outcomes

Every Vortex-Bevy conversion emits exactly one classification. The
classifications are stable, machine-readable, and tied to a remediation
hint that names the next step.

This page documents the five outcomes, when each applies, and what to
do when you see them.

## The five classifications

### `converted`

Vortex-Bevy produced a runtime artifact from the input end-to-end. The
GLB was packaged, the manifest was emitted, and the conversion was
atomically published. The runtime can load the result via
`load_published_conversion_and_spawn`.

Typical for: a self-authored USD stage that parses within the bounded
adapter, an authorized Unreal controlled static-scene export produced by
the matching Editor bridge, a Force Text URP static-scene report
produced by the matching Unity Editor bridge.

### `baked`

An external adapter produced the input, and Vortex-Bevy converted the
baked form end-to-end. Same operational status as `converted`; the
classification distinguishes "the input was already preprocessed" from
"the input was native source".

Typical for: a USD stage that was authored through an OpenUSD runtime
adapter to a flattened, prepackaged, or quantized-bounds shape.

### `requires_adapter`

Vortex-Bevy detected the input shape and identified an external adapter
required to complete the conversion. The classification carries a typed
remediation naming the specific adapter to invoke — for example,
"Invoke the authorized Unreal Editor through
`bridges/unreal/vortex_unreal_bridge.py` and convert the staged FBX/TGA
outputs through the canonical IR pipeline."

`requires_adapter` is the correct classification for inputs that are
real and recognized but where direct decoding is intentionally out of
scope. The orchestrator does not silently reclassify these inputs as
`converted` or attempt to bypass the adapter.

Typical for: any `.uasset` (T08, T26 — direct `.uasset` decoding is
metadata-only), any Force Text Unity project, any Godot `.tscn`,
any USD stage beyond the bounded USDA parser, any USD binary layer
(`.usdc`).

### `unsupported`

Vortex-Bevy does not support this capability. The remediation names a
forward direction (track under T-XX family, file an issue, or use the
matching Editor's native export path).

Typical for: input shapes that fall outside any documented boundary —
non-OpenUSD formats, encrypted payloads with no matching key, BP-only
or C++-only Unreal assets that the bridge contract cannot export.

### `rejected`

The input is unauthorized — cooked, encrypted, unowned, DRM-protected,
key-dependent, or otherwise outside the documented authorization
boundary. Vortex-Bevy rejects it without attempting to bypass access
controls.

Typical for: `.pak`/`.utoc`/`.ucas`/`.uexp`/`.ubulk` sidecars, paths that
escape the project root after canonicalization, symlinks pointing
outside the project root, malformed package headers, missing companion
bulk files.

## Where classifications appear

Every classification site uses the same five values, so the same tooling
can drive UI copy off any one of them:

- **CLI report** — the `classification` field of every `CliReport`
  variant.
- **Scene manifest** — the `classification` field of each `ManifestSource`
  record.
- **Conversion aggregate** — the `classification` field of the typed
  `ConversionClassification`.
- **Support matrix** — the `classification` field of each
  `SupportMatrixEntry`.

A CLI consumer that handles `requires_adapter` correctly will handle
every other site correctly with the same code path.

## What to do with each outcome

| Classification        | Operator action                                                                                                                             |
| --------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| `converted` / `baked` | Hand the manifest + GLB to the Bevy runtime. The CLI's JSON `kind: "Convert"` with `action: "executed"` is the success signal.              |
| `requires_adapter`    | Read the `remediation` field. It names the specific adapter (Unreal Editor bridge script, Unity batch-mode bridge, OpenUSD runtime, etc.).  |
| `unsupported`         | File an issue with the input shape and the engine version. The CLI exit code is non-zero.                                                   |
| `rejected`            | Recheck the explicit project root, asset path, and authorization. Do not weaken confinement or provide keys. The CLI exit code is non-zero. |

## Going further

- [Tutorial: Convert a USD stage end-to-end](../tutorial/usd-end-to-end.md) —
  hands-on example of `converted` and the JSON report shape
- [Troubleshoot conversion failures](troubleshoot.md) — common failure
  shapes and how to read the CLI exit code
- [Reference: Scene manifest schema](../reference/scene-manifest.md)
