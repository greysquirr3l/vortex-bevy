# Tutorial: Convert a USD stage end-to-end

This tutorial walks you through converting a self-authored OpenUSD
stage into a published Bevy-compatible GLB plus manifest. By the end you
will understand the full pipeline: stage selection, IR construction,
GLB packaging, atomic publication, and Bevy runtime loading.

The USD path is fully testable headlessly — no Editor required, no
network — so it is the right starting point for understanding the system.

## What you'll build

You will produce:

- A typed `scene.manifest.ron` sidecar describing the source, profile,
  artifacts, and compatibility requirements.
- A `scene.glb` Bevy-compatible container.
- A `conversion-report.json` summarizing the classification and
  diagnostics.

All three are atomically published under a conversion-id-named directory
inside your chosen output root.

## Step 1: Create a minimal USD stage

Create a directory anywhere on your filesystem to hold the stage:

```bash
mkdir -p /tmp/vortex-tutorial/stage
```

Write a minimal USDA file to that directory. The orchestrator requires
the canonical `#usda 1.0` header and validates metadata. A minimal
two-prim stage is enough to exercise the pipeline:

```text
#usda 1.0
(
    upAxis = "Y"
    metersPerUnit = 0.01
)

def Xform "Root" {
    def Mesh "Geometry" {
    }

    def Xform "Pivot" {
        double3 xformOp:translate = (1.0, 0.0, 0.0)
    }
}
```

Save this as `/tmp/vortex-tutorial/stage/Fixture.usda`. The file is
intentionally tiny — no mesh data, no materials. The orchestrator
records the prim hierarchy as `CanonicalScene` nodes; the GLB packaging
emits a valid glTF 2.0 container with that hierarchy.

## Step 2: Pick a conversion profile

The CLI requires an `--profile` argument that names a deterministic
conversion profile. The orchestrator derives the conversion ID from the
source identity and the profile hash, so the same source with the same
profile always lands at the same conversion-id directory.

For this tutorial, use `default`:

```bash
cargo run -p vortex-bevy -- plan \
    --project /tmp/vortex-tutorial \
    --engine usd \
    --asset stage/Fixture.usda \
    --object /Root \
    --profile default \
    --output /tmp/vortex-tutorial/out
```

The orchestrator parses the stage, computes the conversion ID, and checks
whether an existing compatible conversion is already published. Since
this is the first run, the action is `execute`.

## Step 3: Inspect the plan report

The CLI returns a JSON report on stdout:

```json
{
  "kind": "Plan",
  "conversion_id": "f4b1...7c",
  "source": "usd:/tmp/vortex-tutorial:stage/Fixture.usda:/Root:/",
  "action": "execute",
  "classification": "converted",
  "reason": "selected USD stage packaged through canonical IR + GLB",
  "remediation": "stage is converted; runtime loads via vortex-bevy view --artifact-root"
}
```

Note the key fields:

- `conversion_id` — the SHA-256 hash that identifies this exact source +
  profile combination.
- `source` — the typed `SourceIdentity::Usd` formatted as
  `usd:<project>:<stage>:<prim_path>:<selection>`.
- `action` — `execute` means no existing compatible conversion was found;
  the runner will perform full work. `reuse` means a compatible
  conversion already exists and will be returned unchanged.
- `classification` — the runner's classification. For a fully
  self-authored USD fixture, the orchestrator produces `converted`.

## Step 4: Run the conversion

The `convert` command does everything `plan` does, then runs the
orchestrator and publishes the artifacts:

```bash
cargo run -p vortex-bevy -- convert \
    --project /tmp/vortex-tutorial \
    --engine usd \
    --asset stage/Fixture.usda \
    --object /Root \
    --profile default \
    --output /tmp/vortex-tutorial/out
```

The CLI prints a similar JSON report with `kind: "Convert"`. Check the
exit code (`echo $?`) — `0` means success; non-zero means typed failure
(see [Exit codes and error surface](../reference/errors.md)).

## Step 5: Inspect the published artifacts

The conversion published into `/tmp/vortex-tutorial/out/<conversion_id>/`:

```bash
ls /tmp/vortex-tutorial/out/<conversion_id>/
```

You should see three files:

- `scene.glb` — the packaged GLB
- `scene.manifest.ron` — the versioned manifest
- `conversion-report.json` — the same classification as the CLI report

Read the manifest:

```bash
cat /tmp/vortex-tutorial/out/<conversion_id>/scene.manifest.ron
```

The manifest is RON, schema-versioned (`schema_version: 1`). The key
fields are:

- `converter_version` — the vortex-bevy version that produced the artifacts.
- `profile_hash` — SHA-256 of the named profile.
- `artifacts` — the typed artifact list (kind, path, hash). The GLB's
  hash is verified by Bevy at load time.
- `sources` — the typed `ManifestSource` records, including the asset
  kind, selected values, GLB label mappings, classification, and
  diagnostics.
- `compatibility` — the Bevy version and required capabilities. For a
  USD conversion through this orchestrator path, the capability is
  `BevyGltfPbr`.

## Step 6: Re-run to see idempotence

Run the same `convert` command again:

```bash
cargo run -p vortex-bevy -- convert \
    --project /tmp/vortex-tutorial \
    --engine usd \
    --asset stage/Fixture.usda \
    --object /Root \
    --profile default \
    --output /tmp/vortex-tutorial/out
```

The CLI returns `action: "reused"` this time. The atomic artifact store
detects the existing conversion, verifies that the profile hash still
matches, and returns the previously-published artifacts unchanged. The
GLB and manifest files are byte-identical to the first run.

The manifest and GLB contain no wall-clock timestamps, environment
variables, host paths, or random identifiers. Re-running on a different
host or at a different time produces byte-identical outputs.

## Step 7: Inspect the source classification and diagnostics

The manifest's `sources` array carries the classification emitted by the
runner. For a USD stage that exceeds the glTF `u32` `NodeId` capacity
(roughly 4 billion prims), the orchestrator surfaces a typed
`PrimCountExceedsGlb` error carrying remediation:

> glTF runtime u32 NodeId capacity reached. Options:
>
> 1. split the source stage into multiple selected configurations and convert them independently;
> 2. isolate the offending prim via a narrower prim-path selection;
> 3. apply upstream mesh decimation to reduce the prim count below the u32 limit.

The CLI exits non-zero with the typed error printed as the message body
plus the JSON `error` field on stderr (when `--format json` is set).

For the two-prim fixture you built, the capacity check is trivially
satisfied and the runner produces `classification: "converted"` with a
`reason` describing what the orchestrator did.

## Step 8: Trigger the capacity check (optional)

To exercise the capacity check, build a stage with more than `u32::MAX`
prims. This is impractical by hand — `PrimCountExceedsGlb` is typed and
unit-tested in the orchestrator's `tests` module. To see the path
without authoring a multi-billion-prim file, look at the unit test
`prim_count_exceeds_glb_returns_typed_remediation` (when it lands) or
the typed-error contract documented at
[`docs/canonical_scene_ir_contract.md`](../../../docs/canonical_scene_ir_contract.md).

## Where to go next

- [Inspect an Unreal asset](../how-to/inspect-unreal.md) — the bounded
  metadata preflight path for `.uasset` files
- [Convert an authorized source](../how-to/convert.md) — the engine
  variants (Unreal / Unity / Godot) and the `requires_adapter`
  classification each one produces today
- [Reference: Scene manifest schema](../reference/scene-manifest.md) —
  full field-by-field documentation
