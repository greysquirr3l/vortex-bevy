# CLI

Vortex-Bevy ships a single binary, `vortex-bevy`, with seven
subcommands. Every flag is parsed into a typed domain value object
before any filesystem work, so malformed input is rejected at the CLI
boundary with a stable typed error.

This page is the flag-by-flag reference. See
[Install](../getting-started/install.md) and
[Your first conversion](../getting-started/first-conversion.md) for
narrative walk-throughs.

## Top-level

```text
vortex-bevy [--version] [--help] <subcommand> ...
```

The version string follows semver. The CLI never assumes a default
project root, asset path, or output root — every operation requires
explicit authorization through its flags.

## `inspect`

Inspect one explicit authorized source without conversion. Metadata-only
preflight for `.uasset`.

```text
vortex-bevy inspect \
    --project <path> \
    --engine <unreal|usd|unity> \
    --asset <path> \
    [--object <UnrealObjectPath>] \
    [--format <text|json>]
```

| Flag        | Type                     | Required            | Validation                                 |
| ----------- | ------------------------ | ------------------- | ------------------------------------------ |
| `--project` | filesystem path          | yes                 | canonicalizes before use; confinement root |
| `--engine`  | enum                     | yes                 | `unreal`, `usd`, `unity`                   |
| `--asset`   | typed `SourceAssetPath`  | yes                 | relative path, no traversal, no NUL        |
| `--object`  | typed `UnrealObjectPath` | for Unreal          | `/Game/...` form, validated at parse       |
| `--format`  | enum                     | no (default `text`) | `text` or `json`                           |

For USD and Unity the inspect dispatch currently reports
`requires_adapter` — the inspect surface is calibrated for Unreal
metadata preflight today.

## `plan`

Resolve a deterministic conversion plan without publishing artifacts.

```text
vortex-bevy plan \
    --project <path> \
    --engine <unreal|usd|unity> \
    --asset <path> \
    --object <id> \
    --profile <name> \
    --output <path> \
    [--guid <hex32>] \
    [--variant <expr>] \
    [--payload-policy <forbid|metadata_only>] \
    [--format <text|json>]
```

The flag set mirrors `convert` minus the side-effecting publication.
See the [convert reference](#convert) below for per-flag validation
and the JSON report shape.

## `convert`

Drive the conversion orchestrator end-to-end and publish the artifacts
to `--output`.

```text
vortex-bevy convert \
    --project <path> \
    --engine <unreal|usd|unity> \
    --asset <path> \
    --object <id> \
    --profile <name> \
    --output <path> \
    [--guid <hex32>] \
    [--variant <expr>] \
    [--payload-policy <forbid|metadata_only>] \
    [--dry-run] \
    [--overwrite] \
    [--format <text|json>]
```

| Flag               | Type                      | Required            | Validation                                                                               |
| ------------------ | ------------------------- | ------------------- | ---------------------------------------------------------------------------------------- |
| `--project`        | filesystem path           | yes                 | canonicalizes; confinement root                                                          |
| `--engine`         | enum                      | yes                 | `unreal`, `usd`, `unity`                                                                 |
| `--asset`          | typed `SourceAssetPath`   | yes                 | relative path, no traversal                                                              |
| `--object`         | string                    | engine-specific     | see below                                                                                |
| `--profile`        | typed `ExportProfileName` | yes                 | ASCII alphanumeric + `-` + `_`                                                           |
| `--output`         | filesystem path           | yes                 | canonicalizes; artifact store root                                                       |
| `--guid`           | string                    | Unity only          | 32 lowercase hex chars                                                                   |
| `--variant`        | typed `StageSelection`    | USD only            | see [`docs/usd_stage_ingestion_contract.md`](../../docs/usd_stage_ingestion_contract.md) |
| `--payload-policy` | enum                      | USD only            | `forbid` or `metadata_only`                                                              |
| `--dry-run`        | flag                      | no                  | compute plan without publishing                                                          |
| `--overwrite`      | flag                      | no                  | permit replacement only inside `--output`                                                |
| `--format`         | enum                      | no (default `text`) | `text` or `json`                                                                         |

The `--object` flag is engine-specific:

- **Unreal** — `--object /Game/Path/Asset.Asset` (must match the
  typed `UnrealObjectPath` form).
- **USD** — `--object /Prim/Path` (must match the typed `UsdPrimPath`
  form).
- **Unity** — `--object <u64>` (object id parsed as `u64`).

## `verify`

Verify the artifacts for a conversion identifier.

```text
vortex-bevy verify --conversion-id <id> [--format <text|json>]
```

The verify dispatch currently reports `Unavailable` — the artifact
verification surface is part of the recovery sequence. The
`AtomicArtifactStore::verify_published` method is the authoritative
underlying implementation; tooling that needs the underlying check can
use it directly.

## `view`

Open the Bevy 0.19 desktop viewer for verified artifacts.

```text
vortex-bevy view --artifact-root <path> [--format <text|json>]
```

The view dispatch currently reports `Unavailable` — the artifact
discovery boundary is part of the recovery sequence. The Bevy runtime
loader is fully wired; see [View a published conversion](../how-to/view.md)
for the runtime-side contract and programmatic usage.

## `support-matrix`

Print the tested capability classifications.

```text
vortex-bevy support-matrix [--format <text|json>]
```

JSON output is the canonical form. The same document is also emitted
to [`docs/compatibility_matrix.json`](../../docs/compatibility_matrix.json)
at every release cut.

## `version`

Print the product version.

```text
vortex-bevy version [--format <text|json>]
```

JSON output is `{ "version": "<semver>" }`. Useful in CI gating and
in issue reports when a CLI invocation does not match expected
behavior.

## Output modes

Every subcommand accepts `--format text` or `--format json`. The text
mode is for human reading at a terminal; the JSON mode is for tooling
and CI gating. The JSON shape is stable per subcommand and is the
contract that downstream tooling should depend on.

The same five classifications appear in every report that emits one:

- `converted`
- `baked`
- `requires_adapter`
- `unsupported`
- `rejected`

See [Understand classification outcomes](../how-to/classification-outcomes.md)
for the full taxonomy.

## Exit codes

| Code | Meaning                                                                  |
| ---- | ------------------------------------------------------------------------ |
| `0`  | Success.                                                                 |
| `2`  | Argument parse error (Clap). The CLI printed usage to stderr.            |
| `3`  | The subcommand completed but its downstream capability is `Unavailable`. |
| `4`  | Inspection or conversion failed with a typed error.                      |

See [Exit codes and error surface](errors.md) for the typed error
matrix and JSON shape.
