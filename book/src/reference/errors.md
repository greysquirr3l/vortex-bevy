# Exit codes and error surface

Vortex-Bevy is fail-closed by design. Every failure is a typed error
with a stable shape — exit code, JSON `error` field, manifest
diagnostic code, or typed remediation in the support matrix.

This page documents the exit code matrix and the JSON error surface so
tooling can depend on stable contracts.

## Exit codes

| Code | Meaning                          | When                                                                                                            |
| ---- | -------------------------------- | --------------------------------------------------------------------------------------------------------------- |
| `0`  | Success                          | The subcommand completed its work without failure.                                                              |
| `2`  | Argument parse error             | Clap rejected one or more arguments; the CLI printed usage to stderr.                                           |
| `3`  | Capability `Unavailable`         | The subcommand completed but its downstream capability is `Unavailable`. The remediation is in the JSON report. |
| `4`  | Inspection or conversion failure | A typed error from an adapter or the orchestrator.                                                              |

The exit codes are stable. Tooling that depends on them does not need to
parse stderr text — the JSON mode puts every failure into the same
`error` field shape.

## JSON error shape

When `--format json` is set, every CLI invocation that produces a
typed error writes a JSON document on stderr with the following shape:

```json
{
  "error": "<stable error string>",
  "source": "<typed SourceIdentity>",
  "capability": "<support-matrix capability key>",
  "stage": "<inspect|plan|convert|verify|view|support-matrix|version>",
  "diagnostics": [
    {
      "code": "<stable diagnostic code>",
      "message": "<human-readable message>"
    }
  ]
}
```

The fields are stable. The CLI never emits a free-form error message
that does not also appear in the structured fields. The text mode prints
the same information in a terminal-friendly format.

## Diagnostic codes

Diagnostic codes are stable lower-case tokens. They appear in:

- The manifest's `diagnostics` array on each `ManifestSource`.
- The CLI's JSON `diagnostics` array on every error response.
- The runtime's `VortexRuntimeDiagnostic` entity when a load fails.

The `code` field is what tooling should depend on. The `message` field
is for humans.

## Common diagnostic codes

| Code                                                          | Meaning                                                                                                                                                  |
| ------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `vortex.runtime.artifact_rejected`                            | The Bevy runtime could not load the published artifacts. The root cause is in the diagnostic `message` field.                                            |
| `vortex.usd.stage_composition_requires_authoritative_runtime` | The bounded USDA adapter cannot resolve the stage's composition (payloads, recursive references, unselected variants).                                   |
| `vortex.usd.prim_count_exceeds_glb_u32_capacity`              | The USD source has more prims than the glTF u32 NodeId capacity allows. The diagnostic carries typed remediation.                                        |
| `vortex.unreal.cooked_sidecar_rejected`                       | The bounded preflight detected a cooked-style sidecar (`.pak`, `.utoc`, `.ucas`, `.uexp`, `.ubulk`).                                                     |
| `vortex.unreal.path_escape`                                   | The supplied project root resolved outside its canonical parent after symlink resolution.                                                                |
| `vortex.glb.json_serialization_failed`                        | The generated GLB JSON chunk could not be serialized. This is a deterministic internal error and should be filed as an issue.                            |
| `vortex.glb.structural_parse_failed`                          | The generated GLB bytes did not parse as glTF 2.0 after the deterministic build. This is a deterministic internal error and should be filed as an issue. |

## Failure classification

A failed invocation is one of:

- **Argument parse failure** — exit code `2`. The CLI prints usage. The
  JSON form includes the offending argument name.
- **Capability unavailable** — exit code `3`. The subcommand completed
  but reported `Unavailable`. The remediation is in the JSON report.
- **Typed error** — exit code `4`. The subcommand encountered a typed
  error from an adapter or the orchestrator. The JSON report carries
  the diagnostic code and message.

A successful invocation returns exit code `0`. There are no other
exit codes.

## Where the error surfaces

The same five classifications appear in every error surface:

- The CLI's JSON `error` field.
- The manifest's `classification` field on each `ManifestSource`.
- The `ConversionClassification` carried by the
  `ConversionAggregate`.
- The support matrix's `classification` field on each row.

Downstream tooling that handles one surface correctly will handle the
others correctly with the same code path.

## Going further

- [Understand classification outcomes](../how-to/classification-outcomes.md)
- [Troubleshoot conversion failures](../how-to/troubleshoot.md)
- [Reference: Scene manifest schema](scene-manifest.md)
