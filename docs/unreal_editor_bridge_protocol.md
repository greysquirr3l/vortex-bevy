# Unreal Editor Bridge Protocol

## Purpose and authorization boundary

The Rust adapter at `vortex-core::adapters::secondary::unreal_bridge` accepts semantic export data **only** from the checked-in bridge script at [`bridges/unreal/vortex_unreal_bridge.py`](../bridges/unreal/vortex_unreal_bridge.py). It is the controlled hand-off between an authorized editable Unreal project and the Vortex conversion pipeline. The adapter does not discover project scripts, upload project data, decrypt packages, guess keys, or interpret Blueprint/C++, material graphs, Niagara, Chaos, Landscape, Nanite, Lumen, or other non-portable systems.

> A successful report proves only that a matching authorized Unreal Editor bridge staged the reported files. It does not itself prove packaging, rendering, or behavioral equivalence.

## Fixed invocation contract

A caller constructs `UnrealBridgeRequest` with an existing authorized project root, a `.uproject` located beneath that root, a configured matching Unreal Editor executable, an existing staging root, and one or more explicit Unreal object paths. The adapter canonicalizes the paths, rejects raw parent traversal before canonicalization, rejects project-file escape, and refuses an empty asset selection.

The command is deterministic and uses only the committed script:

```text
<UnrealEditor-Cmd> <Project.uproject> -unattended -nop4 \
  -script=<repository>/bridges/unreal/vortex_unreal_bridge.py -- \
  --assets <sorted-object-paths> \
  --staging-root <canonical-staging-root> \
  --report <canonical-staging-root>/vortex-unreal-bridge-report.json
```

The report is bounded to 1 MiB and must be emitted at the fixed path beneath the request’s staging root. A nonzero process outcome is surfaced as a typed `ProcessFailed` error before the adapter reads any report. The process dependency is consumer-owned through `UnrealBridgeProcess`, which allows tests to simulate process outcomes without installing Unreal.

## Report contract

The shared schema is [`bridges/contract/vortex-editor-bridge-v1.schema.json`](../bridges/contract/vortex-editor-bridge-v1.schema.json). The Rust adapter requires `engine: "unreal"` and `protocol_version: "vortex.editor-bridge/v1"`, rejects unknown JSON fields, sorts records by typed source identity, and rejects duplicate sources.

| Report field | Rust validation |
|---|---|
| `source_identity` | Must be Unreal, must be one of the explicitly requested object paths, and must map from `/Game/...` to a typed `Content/...uasset` source identity. |
| `asset_kind`, `exporter` | Must be nonempty. |
| `classification` | Maps to the existing typed Vortex classification enum. |
| `outputs` | Must be nonempty only for `converted`/`baked`; every path must be nonempty, relative, slash-separated, and free of `.` or `..` components. |
| `warnings`, `errors` | Preserved as sorted, deduplicated operator evidence. |

Records for Blueprint and other non-portable classes remain explicit `requires_adapter`, `unsupported`, or `rejected` results. They are not silently converted or dropped.

## Synthetic validation

`crates/vortex-core/tests/unreal_bridge_protocol.rs` validates command construction, source mapping from the committed shared fixture, malformed and future protocol rejection, staging-output traversal rejection, project/staging traversal rejection before process launch, simulated process failure mapping, and simulated successful report ingestion. These tests do not start Unreal.

The actual matching-Editor/project exercise remains **T10**, a human-gated validation step. Run it only with content you own or are expressly authorized to export.

## Related documents

The bridge workspace and its language-specific quality gate are documented in [`bridges/README.md`](../bridges/README.md). Bounded direct `.uasset` metadata inspection is a separate fail-closed preflight described in [`unreal_metadata_preflight_contract.md`](unreal_metadata_preflight_contract.md).
