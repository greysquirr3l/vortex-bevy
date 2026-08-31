# Convert an authorized source

The `convert` subcommand drives the conversion orchestrator end-to-end
against an explicitly authorized source. It performs plan resolution,
runs the per-engine conversion pipeline, publishes the artifacts, and
returns a typed report.

This page covers what `convert` does, what its inputs are, and what
each per-engine classification means today.

## Invocation

The CLI takes nine flags total. Five are required for every
`convert` invocation; the remaining four are required only for specific
engines.

Always required:

- `--project <path>` — physical filesystem root for the source.
- `--engine <name>` — engine family (`unreal`, `usd`, `unity`).
- `--asset <path>` — project-relative source path.
- `--output <path>` — output root owned by Vortex-Bevy. The CLI
  canonicalizes this and opens an atomic artifact store rooted there.
- `--profile <name>` — deterministic conversion profile name. The
  profile hash is derived from the profile name (SHA-256 of its bytes)
  and combined with the source identity to form the conversion ID.

Engine-specific (required for the engines listed):

- `--object <id>` — engine-specific secondary identifier:
  - **Unreal**: `--object /Game/Path/Asset.Asset`
  - **USD**: `--object /Prim/Path`
  - **Unity**: `--object <u64>` (object id)
  - **Godot**: `--object res://path/to/scene.tscn` (defaults to `--asset` if omitted)
- `--guid <hex32>` — Unity asset GUID. Required only for `--engine unity`.
- `--variant <expr>` — USD selection expression. Defaults to a root
  selection if omitted. See
  [`docs/usd_stage_ingestion_contract.md`](../../docs/usd_stage_ingestion_contract.md)
  for the grammar.
- `--payload-policy <forbid|metadata_only>` — USD payload handling.
  Defaults to `forbid`.

## What happens

`convert` performs five steps:

1. **Validation.** Every flag is parsed into a typed domain value object
   before any filesystem work. An invalid path, an unknown engine, or
   a malformed object reference fails closed with a typed error.
2. **Plan resolution.** The CLI calls
   `plan_conversion::handle` with the typed `PlanConversion`. The
   `AtomicArtifactStore` (rooted at `--output`) looks up an existing
   compatible conversion. If found, the plan action is `Reuse`; otherwise
   it is `Execute`.
3. **Execution.** The CLI calls `execute_conversion::handle` with the
   `ConversionPipeline` runner and the same store. The runner dispatches
   by engine:
   - **USD**: full pipeline (parse → IR → GLB → manifest → publish).
   - **Unreal / Unity / Godot**: classify `RequiresAdapter` with explicit
     remediation pointing at the authorized Editor bridge.
4. **Persist.** The store persists the conversion report (`report.json`)
   beside the published artifacts.
5. **Report.** The CLI returns a typed `Convert` report with the
   conversion ID, the source identity, the action (`executed` /
   `reused`), the classification, and the typed remediation.

## Per-engine classifications today

| Engine                               | Classification     | Reason                                                                             | Remediation                                                                                                                                              |
| ------------------------------------ | ------------------ | ---------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `usd` (self-authored, parseable)     | `converted`        | Pipeline ran end-to-end and produced typed artifacts.                              | Runtime loads via `vortex-bevy view --artifact-root`.                                                                                                    |
| `usd` (binary/plugin-backed `.usdc`) | `requires_adapter` | The bounded adapter accepts text USDA only; binary layers need an OpenUSD runtime. | Use a bounded source-side adapter that hands the stage to OpenUSD for full composition.                                                                  |
| `unreal`                             | `requires_adapter` | Direct `.uasset` decoding is metadata-only by product design (T08, T26).           | Run `bridges/unreal/vortex_unreal_bridge.py` against a matching authorized Editor and feed the staged FBX/TGA exports through the canonical IR pipeline. |
| `unity`                              | `requires_adapter` | Direct Force Text project decoding covers metadata only.                           | Invoke the Unity Editor's batch-mode bridge at `bridges/unity/*` and convert the staged static-scene report through the canonical IR pipeline.           |
| `godot`                              | `requires_adapter` | Godot adapter expansion is tracked separately.                                     | Track the Godot static-scene adapter expansion (T26 family) and re-execute once the canonical IR + GLB pipeline accepts Godot sources.                   |

The classification is emitted through `Remediation::new` so it is always
typed and always present. The CLI surfaces the reason and remediation in
both text and JSON output modes.

## Reuse and idempotence

A `Reuse` plan returns the previously-published artifacts unchanged. The
atomic artifact store verifies that the profile hash still matches
before reusing, so a `convert` against a corrupted or modified output
directory returns a typed error rather than silently reusing broken
artifacts.

If a conversion with the same ID already exists with a different
profile hash, the store returns `ExistingConversionConflict` — a typed
error that surfaces in the CLI as a non-zero exit with the offending
conversion ID in the error body. This guards against the silent
reclassification of a previously-stable conversion.

## Capacity errors

The USD orchestrator enforces the glTF `u32` `NodeId` capacity. If a
source exceeds roughly 4 billion prims, the orchestrator returns
`PrimCountExceedsGlb { count, remediation }` with a typed remediation
suggesting concrete next steps:

> glTF runtime u32 NodeId capacity reached. Options:
>
> 1. split the source stage into multiple selected configurations and convert them independently;
> 2. isolate the offending prim via a narrower prim-path selection;
> 3. apply upstream mesh decimation to reduce the prim count below the u32 limit.

The CLI surfaces this as a non-zero exit with the error body printed.

## Going further

- [Tutorial: Convert a USD stage end-to-end](../tutorial/usd-end-to-end.md) —
  the USD pipeline in full
- [Understand classification outcomes](classification-outcomes.md) —
  when each classification is correct
- [Troubleshoot conversion failures](troubleshoot.md) — common failure
  shapes and how to read them
