# Your first conversion

This page walks you through running the CLI end-to-end against a
self-authored fixture that ships with the repository. By the end you will
have inspected a synthetic USD stage, planned a conversion, executed the
conversion, and inspected the published manifest.

## Prerequisites

- [Built the project](./build.md) at least once
- Approximately five minutes

## Step 1: Inspect the support matrix

The first thing to confirm is what the tool can do. The support matrix
lists every capability the project has tested, with a typed classification
and a remediation hint.

```bash
cargo run -p vortex-bevy -- support-matrix --format json
```

You will see a JSON document listing capabilities such as
`usd.usda.selected_stage`, `glb.deterministic_packaging`, and
`artifact.atomic_publication`. Each row has a `classification` field —
`converted`, `baked`, `requires_adapter`, `unsupported`, or `rejected` —
and a `remediation` field that tells you what to do next.

The single most important thing to read here is the `requires_adapter`
classification for engine-specific items like `unreal.uasset.metadata_preflight`:
direct `.uasset` decoding is metadata-only; authoritative export requires
the matching authorized Unreal Editor bridge.

For an authoritative view of every row, run the command above — the
generated matrix is the single source of truth.

## Step 2: Run `--help`

Every subcommand prints its own help. Start with the top-level help:

```bash
cargo run -p vortex-bevy -- --help
```

The output lists seven subcommands: `inspect`, `plan`, `convert`, `verify`,
`view`, `support-matrix`, `version`. Each prints its own help:

```bash
cargo run -p vortex-bevy -- inspect --help
cargo run -p vortex-bevy -- plan --help
cargo run -p vortex-bevy -- convert --help
```

The CLI parses every flag into a typed domain value object before any
filesystem work. A syntactically valid path that fails canonicalization
(symlink escape, root escape) is rejected with a typed error before any
adapter runs.

## Step 3: Look at the synthetic fixtures

The repository ships self-authored fixtures under `fixtures/` and
`bridges/fixtures/`. The minimum USD stage that the orchestrator can
convert end-to-end is a single `.usda` file with the required header:

```text
#usda 1.0
(
    upAxis = "Y"
    metersPerUnit = 0.01
)
def Xform "Triangle" {
    def Mesh "Geometry" {
    }
}
```

You will find this exact pattern (with slight variations) across the test
suite under `crates/vortex-core/tests/`.

## Step 4: Run a real `convert`

The end-to-end USD pipeline is now wired in the orchestrator. Pick any
self-authored USDA fixture (the test suite writes one under a tempdir at
runtime; for a real file, the simplest path is to use the project root and
output root that point at any directory you control):

```bash
cargo run -p vortex-bevy -- convert \
    --project "$(pwd)/crates/vortex-core/tests" \
    --engine usd \
    --asset t20-runtime-valid/scene.usda \
    --object /World \
    --profile default \
    --output "$(pwd)/target/manual-out" \
    --format json
```

On success the CLI prints a JSON report:

```json
{
    "kind": "Convert",
    "conversion_id": "<sha256>",
    "source": "usd:...",
    "action": "executed",
    "classification": "requires_adapter",
    ...
}
```

A successful `requires_adapter` outcome means the typed SourceIdentity was
constructed, the planning step ran, and the runner reached the engine
dispatch. The exact classification depends on the source — for a fully
self-authored USD fixture the orchestrator drives the pipeline to a
`Converted` outcome with GLB + manifest artifacts.

If the CLI prints a JSON error (`{"error": "..."}` style) the conversion
itself failed; the JSON shape and exit code are stable. See
[Exit codes and error surface](../reference/errors.md).

## Step 5: Inspect the published artifacts

The conversion publishes into the directory you passed to `--output`.
Inside that directory, each conversion gets its own subdirectory named
after the conversion id (a SHA-256 of source + profile hash). Inside that:

```text
<output>/<conversion_id>/
├── scene.glb                # The packaged GLB
├── scene.manifest.ron       # Versioned manifest
└── conversion-report.json   # The same classification as the CLI report
```

You can read the manifest:

```bash
cat "$(pwd)/target/manual-out/<conversion_id>/scene.manifest.ron"
```

The manifest is RON, schema-versioned (currently `1`), and contains the
typed source identity, profile hash, GLB artifact hash, and the explicit
`BevyGltfPbr` capability requirement. See
[Scene manifest schema](../reference/scene-manifest.md).

## Step 6: View it in Bevy

The `view` subcommand is reserved for authorized full-conversion flows.
Until then, the integration test suites in `crates/vortex-bevy/tests/`
exercise the headless Bevy runtime end-to-end against the same fixtures.

## Where to go next

- [Tutorial: Convert a USD stage end-to-end](../tutorial/usd-end-to-end.md) —
  a deeper walk-through of the same flow with diagnostics and edge cases
- [Convert an authorized source](../how-to/convert.md) — the engine
  variants and what `requires_adapter` means in each case
- [Reference: CLI](../reference/cli.md) — every flag in one place
