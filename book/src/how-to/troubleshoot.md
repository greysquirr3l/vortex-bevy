# Troubleshoot conversion failures

This page walks through the common failure shapes you will see when
running `inspect`, `plan`, or `convert`, and what each one means.

The CLI is fail-closed by design. Every failure is a typed error with
a stable shape — the exit code, the JSON `error` field, the diagnostic
code in the manifest, or the typed remediation in the support matrix
all use the same five classifications.

## Reading the CLI output

Always check the **exit code first**. Stable exit codes:

| Exit code | Meaning                                                               |
| --------- | --------------------------------------------------------------------- |
| `0`       | Success.                                                              |
| `2`       | Argument parse error (Clap). The CLI printed usage.                   |
| `3`       | The command completed but its downstream capability is `Unavailable`. |
| `4`       | Inspection or conversion failed with a typed error.                   |

When `--format json` is set, the CLI prints a JSON document on stdout
or stderr. Inspect the `error` field for a stable error string and the
`source` field for the typed `SourceIdentity` that the CLI was working
with.

## Common failure shapes

### "Path escapes the authorized root"

**Where it appears:** every CLI subcommand that takes `--project` or
`--output`.

**Cause:** the canonicalized path resolves outside the supplied
project root. Most often this is a symlink that points outside the
project directory, or a relative path that traverses with `..`.

**Action:** recheck the explicit project root. Do not weaken
confinement by changing the root to a parent directory. Resolve the
symlink at its source so it stays inside the project.

### "--object is required for Unreal conversion (Unreal object path)"

**Where it appears:** `convert --engine unreal` without `--object`.

**Cause:** the typed `SourceIdentity::Unreal` requires a stable
`UnrealObjectPath` reference inside the `.uasset`. The CLI cannot
construct the identity without it.

**Action:** pass `--object /Game/Path/Asset.Asset` matching the typed
form. The CLI rejects malformed paths at parse time — see
[Reference: CLI](../reference/cli.md) for the validation rules.

### "--object is required for Unity conversion (Unity object id)"

**Where it appears:** `convert --engine unity` without `--object` or
`--guid`.

**Cause:** `SourceIdentity::Unity` requires both an asset GUID and an
object id. Both are typed values the CLI parses into the identity.

**Action:** pass `--guid <32-char hex>` and `--object <u64>`. The CLI
validates both — GUID must be 32 lowercase hex characters; object id
must parse as a `u64`.

### "Classification is `requires_adapter` and that is correct"

This is not a failure. The CLI returns exit code `0` and the JSON report
carries the `requires_adapter` classification with a typed remediation.

**Where it appears:** `convert --engine unreal`, `convert --engine
unity`, `convert --engine godot`, `convert --engine usd` for binary
layers, and any USD stage the bounded adapter cannot fully resolve.

**Action:** read the `remediation` field of the CLI report. It names
the specific adapter to invoke (Unreal Editor bridge script,
Unity batch-mode bridge, OpenUSD runtime, etc.). The classification
itself is the success signal for these inputs.

### "Cooked sidecar is rejected before metadata parsing"

**Where it appears:** `inspect --engine unreal` when the package has
`.pak`, `.utoc`, `.ucas`, `.uexp`, or `.ubulk` siblings, or the package
is itself one of those.

**Cause:** the bounded preflight rejects cooked-style content. The
adapter's design contract is that Vortex-Bevy does not bypass access
controls.

**Action:** confirm the package is the editable `.uasset` source and not
a cooked artifact. If the source is intentionally cooked, use the
matching Editor's normal export pipeline — Vortex-Bevy is not a
cooked-asset tool.

### "Manifest exceeds the runtime limit"

**Where it appears:** Bevy runtime load with an oversized manifest.

**Cause:** the manifest RON exceeds `DEFAULT_MANIFEST_LIMIT_BYTES`
(1 MiB). Most often this is a manifest with thousands of
diagnostics or many thousands of artifacts.

**Action:** check the manifest's `diagnostics` array. If the orchestrator
emitted many diagnostics, fix the upstream issues so the conversion
emits fewer. If the manifest legitimately needs to be larger, raise the
`DEFAULT_MANIFEST_LIMIT_BYTES` constant in the runtime and rebuild.

### "Source has N prims which exceeds the glTF u32 NodeId capacity"

**Where it appears:** `convert --engine usd` against a stage with more
than roughly 4 billion prims.

**Cause:** the glTF 2.0 specification uses `u32` for node indices. This
is an intrinsic runtime limit, not an application limit.

**Action:** the typed error carries explicit remediation:

> glTF runtime u32 NodeId capacity reached. Options:
>
> 1. split the source stage into multiple selected configurations and convert them independently;
> 2. isolate the offending prim via a narrower prim-path selection;
> 3. apply upstream mesh decimation to reduce the prim count below the u32 limit.

Do not weaken this check; the orchestrator surfaces it as a typed
failure rather than silently truncating because silent truncation
hides data loss.

### "Conversion lock is contended"

**Where it appears:** two concurrent `convert` invocations against the
same output root for the same conversion ID.

**Cause:** the atomic artifact store uses an exclusive lock per
conversion ID. The second invocation waits; if the first takes longer
than the lock's TTL, the second times out.

**Action:** serialize concurrent conversions against the same output
root. If two conversions legitimately need to share an output root,
they must use different conversion profiles so the conversion IDs
differ.

## Going further

- [Understand classification outcomes](classification-outcomes.md) — the
  five classifications and when each is correct
- [Reference: Exit codes and error surface](../reference/errors.md) —
  the full exit-code matrix and JSON error shape
- [Reference: Scene manifest schema](../reference/scene-manifest.md)
