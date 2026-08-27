# vortex-bevy — Implementation Progress

> Orchestrator reads this file at the start of each loop iteration.
> Subagents update this file after completing a task.

## Status Legend

- `[ ]` — Not started
- `[~]` — In progress (claimed by a subagent)
- `[x]` — Completed
- `[!]` — Blocked / needs human input

---


## Phase 1 — Repository and Toolchain Foundation


| Task | Status | Notes |
|---|---|---|
| T01 — Pin the Rust 1.97.1 workspace and collapsed architecture | `[x]` | Rust 1.97.1, edition 2024, resolver 3, two-crate workspace, and headless Bevy 0.19 baseline validated. |
| T02 — Install quality gates, CI, security checks, and conventional commits | `[x]` | Cross-platform CI, CodeQL, Dependabot, cargo-deny, cargo-audit, gitleaks config, hooks, and policy scripts added. |
| T03 — Define deterministic owned fixture contracts and golden artifact rules | `[x]` | Self-authored USDA, malformed, unsupported, manifest, checksum, and fail-closed fixture policy baseline added. |

---


## Phase 2 — Core Domain and Application Contracts

> Depends on: Phase 1 all complete


| Task | Status | Notes |
|---|---|---|
| T04 — Model conversion identities, profiles, classifications, and diagnostics | `[x]` | Validated source/profile/hash identities, deterministic IDs, classifications, diagnostics, and aggregate transitions are implemented and tested. |
| T05 — Create narrow consumer-owned conversion use cases | `[x]` | Typed inspection, planning, execution, verification, cleanup, report, and support-matrix handlers own narrow local traits; replay tests use fakes. |
| T06 — Specify the versioned scene manifest and compatibility contract | `[x]` | Schema-v1 bounded RON manifest, provenance, canonical ordering, compatibility requirements, and rejection tests are implemented and documented. |

---


## Phase 3 — Input Adapters and Canonical Representation

> Depends on: Phase 2 all complete


| Task | Status | Notes |
|---|---|---|
| T07 — Build a typed CLI for inspect, plan, convert, verify, and view | `[x]` | Typed Clap command boundary, text/JSON reports, malformed-input rejection before dispatch, and support-matrix reporting are validated. |
| T08 — Implement Rust-first Unreal package inspection and preflight | `[x]` | Bounded `uasset` 0.6 metadata preflight, root confinement, cooked-sidecar rejection, companion checks, typed facts, and malformed-byte coverage are validated. |
| T09 — T09a — Implement Unreal Editor bridge protocol and synthetic tests | `[x]` | Fixed checked-in bridge command construction, report/version validation, staging confinement, deterministic mapping, and simulated subprocess coverage are validated without an Editor. |
| T10 — T09b — Validate authorized Unreal Editor export integration | `[x]` | Authorized Unreal Engine `5.8.2` controlled static-scene commandlet produced a strict v1 report, FBX/TGA exports, stable `/Game/VortexBridgeTemporary/...` identities, and visible World/MaterialInstanceConstant `requires_adapter` records. Rust typed parsing verified the received artifacts; user-confirmed cleanup removed temporary project content and injected bridge files. |
| T11 — Resolve selected OpenUSD stage configurations into typed source data | `[x]` | Root-confined bounded USDA selection, deterministic prim/metadata facts, and explicit binary/full-composition limits are validated. |
| T12 — Build the canonical scene intermediate representation | `[x]` | Bevy-free deterministic canonical scene IR, typed graph references, normalization provenance, and fail-closed validation are implemented and tested. |
| T13 — Ingest authorized Unity URP project source into typed canonical records | `[x]` | Bounded Force Text project inspection, GUID/meta provenance, URP source facts, and explicit unsupported/Editor-required outcomes are validated. |
| T14 — Validate the Unity URP adapter against an authorized Editor project | `[x]` | Authorized Unity `6000.5.10f1` / URP `17.5.0` unattended batch validation produced a temporary controlled scene report with real GUID/object identity, static mesh, URP Lit material plus generated PNG, camera, directional light, and explicit `BoxCollider` `requires_adapter`. Staging was confined and user-confirmed restoration removed temporary scene/launcher folders. |

---


## Phase 4 — Artifact Conversion and Packaging

> Depends on: Phase 3 all complete


| Task | Status | Notes |
|---|---|---|
| T15 — Emit deterministic Bevy-compatible GLB mesh and scene assets | `[x]` | Contained deterministic GLB serializer validates canonical IR, alignment, structural glTF parsing, mesh/node/camera/light labels, and byte stability. |
| T16 — Normalize textures and map constrained PBR materials | `[x]` | Bounded content-addressed PNG staging, color-space/sampler preservation, deduplication, and deterministic PBR texture emission are validated. |
| T17 — Map constrained Unity URP Lit and Unlit materials to Bevy PBR artifacts | `[x]` | Unity-owned bounded Lit/Unlit mapping validates shader/property gates, factors, alpha/UV semantics, texture roles, and explicit unsupported cases. |
| T18 — Add validated skins, conventional clips, and morph targets | `[x]` | Canonical deformation validation and deterministic GLB skin, animation, morph, and vertex-influence emission are parser-tested. |
| T19 — Atomically publish artifacts, reports, and replay-safe conversion state | `[x]` | Root-confined staging, fsync/rename publication, locks, hash verification, replay, reports, and idempotent cleanup are validated. |

---


## Phase 5 — Bevy 0.19 Runtime and End-to-End Experience

> Depends on: Phase 4 all complete


| Task | Status | Notes |
|---|---|---|
| T20 — Load Vortex artifacts into Bevy 0.19 with provenance components | `[x]` | Bounded manifest/GLB validation, complete atomic-publication verification, reflected current glTF world components, `WorldAssetRoot` spawning, provenance components, and diagnostic fallback are validated headlessly. |
| T21 — Ship the Bevy viewer with scene, source, and diagnostic inspection | `[x]` | Data-driven lifecycle/provenance inspection, filtering, reload/unload, and `bevy_egui` panels are complete with one native camera: right-drag orbit, middle-drag pan, bounded wheel zoom, selected-root framing, pointer gating, scale-aware viewport clipping, and camera disablement when panels exhaust the viewport. Headless validation is complete; visual desktop smoke testing remains a release-platform activity. |
| T22 — Prove the complete conversion pipeline with golden end-to-end suites | `[~]` | Mandatory owned USDA → canonical IR → deterministic GLB → atomic publication/verification → headless Bevy suite and compatibility matrix are complete. T10 now provides real Unreal bridge-export evidence; this task's composed canonical-conversion suite remains intentionally owned and synthetic. |
| T23 — Prove Unity URP fixture conversion end to end | `[x]` | Owned Force Text Lit/Unlit fixture inspection, canonical provenance, deterministic GLB/manifest publication and replay, headless Bevy material loading, and malformed/Shader Graph/tamper failure paths are validated without Unity Editor. |
| T24 — Validate Unity URP conversion against an authorized Editor project | `[x]` | The authorized static-scene report traversed strict typed parsing, canonical IR, deterministic GLB replay, external PNG publication, manifest provenance, three-artifact hash verification/reuse, and headless Bevy spawning of one StandardMaterial, camera, and directional light. `BoxCollider` remains explicit `RequiresAdapter`; evidence is recorded without storing user artifacts. |

---


## Phase 6 — Full Capability Completion

> Depends on: Phase 5 all complete


| Task | Status | Notes |
|---|---|---|
| T25 — Complete selected USD variants, instances, payload policy, and scene chunking | `[ ]` | |
| T26 — Expand direct authorized asset decoding only where it beats Editor export | `[ ]` | |
| T27 — Measure, optimize, and observe full conversion/runtime behavior | `[ ]` | |

---


## Phase 7 — Release, Documentation, and Independent Audits

> Depends on: Phase 6 all complete


| Task | Status | Notes |
|---|---|---|
| T28 — Publish complete architecture, support, operations, and migration documentation | `[ ]` | |
| T29 — Perform release hardening, reproducibility verification, and supply-chain review | `[ ]` | |
| T30 — Security hardening and vulnerability review | `[ ]` | |
| T31 — Integration wiring audit | `[ ]` | |
| T32 — Stub and placeholder cleanup | `[ ]` | |

---


## Accumulated Learnings

> Subagents append discoveries here after each task.
> The orchestrator reads this section at the start of every iteration
> to avoid repeating past mistakes.

- Identity value objects use the same validation during RON deserialization as at input boundaries; domain code does not accept unchecked serialized path/hash strings.
- RON 0.10 does not round-trip internally tagged `SourceIdentity` enums as emitted by its serializer, so the stable manifest uses externally tagged source enum records.
- The strict lint profile requires application handlers to consume typed command/query fields explicitly, boxes disparate large enum payloads, and documents trait methods returning `Result`.

## Codebase State

> Subagents update this section after completing each task.
> Describe what now exists, what is wired up, and what key decisions were made.
> A fresh agent should be able to orient from this section alone.

**Recovery state (2026-08-26):** the previously absent T07–T19 implementations have been rebuilt and validated. `vortex-core` now contains bounded Unreal metadata and Editor-bridge adapters, USDA selection facts, canonical scene IR, Unity Force Text/URP source facts and mapping, deterministic GLB/texture/deformation packaging, and a root-confined atomic artifact store. `vortex-bevy` has typed CLI/report boundaries plus a Bevy 0.19 runtime that verifies complete published conversions before loading, registers the reflected dynamic-glTF components required by current Bevy world spawning, and preserves provenance and diagnostics. T21’s native `bevy_egui` viewer remains complete. T22’s mandatory self-authored synthetic USDA → canonical IR → GLB → publication → headless runtime path and compatibility matrix are complete, while its real Editor smoke test remains gated. T23 proves the owned Force Text URP Lit/Unlit fixture path through deterministic replay, atomic artifacts, and headless Bevy material loading. T14/T24 are now complete: an authorized real Unity `6000.5.10f1` / URP `17.5.0` temporary static scene carried real GUID/object provenance, mesh, generated URP Lit/PNG material, camera, directional light, and explicit `BoxCollider` adapter boundary through canonical IR, deterministic GLB, external texture publication, verified manifest replay, and headless Bevy. The user independently verified temporary project content and launcher working folders were restored/absent. T10 is now complete: authorized Unreal Engine `5.8.2` controlled static-scene commandlet evidence covers v1 report parsing, stable temporary `/Game` identities, FBX/TGA exports, explicit World/MaterialInstanceConstant adapter boundaries, and user-confirmed restoration. A separate opt-in bounded adapter also validates the received FBX/TGA through canonical IR, deterministic GLB, atomic publication, and headless Bevy without adding proprietary artifacts to normal CI.
