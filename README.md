# Vortex-Bevy

Vortex-Bevy converts **authorized** Unreal Engine source-project assets and selected OpenUSD stages into deterministic Bevy 0.19 runtime artifacts. The first runtime contract is a Bevy-compatible GLB plus a versioned `scene.manifest.ron` sidecar that preserves source identity, selected USD configuration, classification, and diagnostics.

## Support boundary

Editable Unreal source projects use the matching Unreal Editor as the authoritative semantic/export decoder. Direct `.uasset` support begins as metadata inspection only. Later direct decoders must be approved per asset class, Unreal version, fixture provenance, and acceptance criteria.

Cooked, encrypted, unowned, DRM-protected, or otherwise unauthorized inputs are rejected by default. Vortex-Bevy does not bypass encryption, guess keys, convert Blueprint/C++ behavior, or claim equivalence for unsupported engine systems.

## Workspace layout

```text
crates/
├── vortex-core/  # Pure domain, application use cases, and adapter boundaries.
└── vortex-bevy/  # CLI and Bevy 0.19 runtime composition.
```

The core crate uses a collapsed DDD/Hex layout:

```text
src/
├── domain/
├── application/{command,query}/
└── adapters/{primary,secondary}/
```

Ports are consumer-owned and live beside the command/query that needs them. There is no shared ports crate or `ports/` directory.

## Conversion and manifest contract

The core domain uses validated typed identities for authorized project roots, source paths, Unreal object paths, USD prim paths, Unity GUIDs, profiles, hashes, and conversion IDs. Conversion IDs are deterministically derived from the typed source and profile hash. The aggregate records artifacts, diagnostics, exactly one actionable classification, and legal state transitions without reading clocks, the environment, or storage.

Command/query handlers own only the narrow dependency trait they consume: inspection, existing-conversion lookup, execution, artifact storage, verification, cleanup, report retrieval, or support-matrix retrieval. This enables in-memory fakes and deterministic replay without a global repository or ports abstraction.

`scene.manifest.ron` is schema-versioned, bounded on read, canonicalized for stable ordering, and rejects unsafe artifact paths, duplicate source identities, malformed RON, and unknown schema versions. See [`docs/scene_manifest_contract.md`](docs/scene_manifest_contract.md).

The Bevy 0.19 runtime verifies the manifest and GLB checksum before spawning `WorldAssetRoot`, then retains source provenance, classifications, diagnostics, variants, and artifact hash on the root entity. See [`docs/bevy_runtime_loading_contract.md`](docs/bevy_runtime_loading_contract.md). Runtime roots have explicit inspection, unload, and reload operations that preserve a data-driven viewer boundary; see [`docs/runtime_scene_lifecycle_contract.md`](docs/runtime_scene_lifecycle_contract.md). The local `bevy_egui` inspector queues lifecycle actions and shares a clipped native 3D viewport: right drag orbits, middle drag pans, the wheel zooms, and **F** or **Frame** fits the selected verified root. Pointer-owned UI input is gated, and the camera disables if panels leave no usable viewport. See [`docs/bevy_egui_viewer_contract.md`](docs/bevy_egui_viewer_contract.md).

## Prerequisites

- Rust **1.97.1**, automatically selected by `rust-toolchain.toml`.
- A supported C compiler/linker for Cargo builds.
- `cargo-deny` and `cargo-audit` for the full preflight.

## Bootstrap and validation

```bash
cargo build --workspace --all-targets --all-features
cargo test --workspace --all-features
cargo fmt --all -- --check
cargo l
cargo deny --all-features check
cargo audit
```

The Wiggum implementation plan is `plan.toml`. Generated task state is maintained in `PROGRESS.md`; do not edit generated task filenames by hand without regenerating the scaffold.

## Fixture policy

Only self-authored or explicitly authorized fixtures belong in this repository. Do not commit Marketplace assets, game assets, Unreal projects, package payloads, encryption material, or generated large binaries without explicit written authorization. See [`fixtures/README.md`](fixtures/README.md).
