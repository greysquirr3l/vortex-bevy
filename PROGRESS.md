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
| T07 — Build a typed CLI for inspect, plan, convert, verify, and view | `[x]` | Typed CLI commands, structured reports, and pre-dispatch malformed-input rejection are validated. |
| T08 — Implement Rust-first Unreal package inspection and preflight | `[x]` | Metadata-only bounded inspection is implemented; payload decoding remains deliberately excluded. |
| T09 — T09a — Implement Unreal Editor bridge protocol and synthetic tests | `[x]` | Versioned bridge protocol and synthetic report/staging tests are implemented without an Editor installation. |
| T10 — T09b — Validate authorized Unreal Editor export integration | `[ ]` | |
| T11 — Resolve selected OpenUSD stage configurations into typed source data | `[x]` | Bounded USDA stage facts and canonical convergence are implemented; advanced composition remains bridge-gated. |
| T12 — Build the canonical scene intermediate representation | `[x]` | Typed deterministic IR validates graph, transforms, render payloads, deformation, provenance, and source convergence. |
| T13 — Ingest authorized Unity URP project source into typed canonical records | `[x]` | Bounded text-only Unity project records and constrained URP source facts are implemented and tested. |
| T14 — Validate the Unity URP adapter against an authorized Editor project | `[ ]` | |

---


## Phase 4 — Artifact Conversion and Packaging

> Depends on: Phase 3 all complete


| Task | Status | Notes |
|---|---|---|
| T15 — Emit deterministic Bevy-compatible GLB mesh and scene assets | `[x]` | Deterministic GLB mesh, hierarchy, image, material, skin, clip, and morph emission is structurally validated. |
| T16 — Normalize textures and map constrained PBR materials | `[x]` | Bounded PNG staging, linear ORM packing, color-space validation, and embedded GLB texture binding are implemented. |
| T17 — Map constrained Unity URP Lit and Unlit materials to Bevy PBR artifacts | `[x]` | Approved Lit/Unlit slots, UV metadata, and typed canonical texture binding are implemented; unsupported semantics remain explicit. |
| T18 — Add validated skins, conventional clips, and morph targets | `[x]` | Validated skin palettes, inverse binds, conventional clips, morph deltas, and deterministic GLB payloads are implemented. |
| T19 — Atomically publish artifacts, reports, and replay-safe conversion state | `[x]` | Atomic owned-path publication, replay reuse, mismatch rejection, contention handling, and cleanup are implemented. |

---


## Phase 5 — Bevy 0.19 Runtime and End-to-End Experience

> Depends on: Phase 4 all complete


| Task | Status | Notes |
|---|---|---|
| T20 — Load Vortex artifacts into Bevy 0.19 with provenance components | `[x]` | Bounded manifest/GLB validation, `WorldAssetRoot` spawning, child-node instantiation, provenance components, and fail-closed rejection are validated headlessly. |
| T21 — Ship the Bevy viewer with scene, source, and diagnostic inspection | `[x]` | Data-driven lifecycle/provenance inspection, filtering, reload/unload, and `bevy_egui` panels are complete with one native camera: right-drag orbit, middle-drag pan, bounded wheel zoom, selected-root framing, pointer gating, scale-aware viewport clipping, and camera disablement when panels exhaust the viewport. Headless validation is complete; visual desktop smoke testing remains a release-platform activity. |
| T22 — Prove the complete conversion pipeline with golden end-to-end suites | `[ ]` | |
| T23 — Prove Unity URP fixture conversion end to end | `[ ]` | |
| T24 — Validate Unity URP conversion against an authorized Editor project | `[ ]` | |

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

`vortex-core` contains a pure deterministic conversion domain, consumer-owned application contracts, bounded source adapters, canonical scene IR, deterministic GLB/image/deformation packaging, constrained URP mapping, and atomic publication. `vortex-bevy` contains typed CLI boundaries and a Bevy 0.19 runtime that validates a bounded manifest and GLB checksum before spawning `WorldAssetRoot`; it retains Vortex provenance components and is verified headlessly through real Bevy asset loading and child-node instantiation. T21 adds root lifecycle generations, controlled unload/reload, detached inspection snapshots, and a Bevy 0.19-compatible `bevy_egui` panel shell with typed reload/unload commands, classification filtering, provenance details, variants, GLB labels, and diagnostics. Its native desktop viewer now has a single orbit camera, selected-root framing, pointer-ownership gating, scale-aware panel clipping, and safe camera disablement when panels consume the viewport. T10 and T14 remain human-gated authorized Editor integrations; T22–T24 and advanced Phase 6 work remain open.
