# Vortex-Bevy

Vortex-Bevy converts **authorized** Unreal Engine source-project assets and selected OpenUSD stages into deterministic Bevy 0.19 runtime artifacts. The first runtime contract is a Bevy-compatible GLB plus a versioned `scene.manifest.ron` sidecar that preserves source identity, selected USD configuration, classification, and diagnostics.

## Support boundary

Editable Unreal source projects use the matching Unreal Editor as the authoritative semantic/export decoder. Direct `.uasset` support begins as metadata inspection only. Later direct decoders must be approved per asset class, Unreal version, fixture provenance, and acceptance criteria.

Cooked, encrypted, unowned, DRM-protected, or otherwise unauthorized inputs are rejected by default. Vortex-Bevy does not bypass encryption, guess keys, convert Blueprint/C++ behavior, or claim equivalence for unsupported engine systems.

## Security scope

Vortex-Bevy is an offline CLI plus an optional Bevy 0.19 desktop viewer. It does not expose an HTTP server, connect to a database, or fetch URLs on the user's behalf. The attack surface is local-file ingestion (bounded per-format size caps, typed deserialization), local subprocess invocation (the authorized Unreal/Unity/Godot Editor through typed `Command::new(...).args(...)` adapters — no shell, no string concatenation), and the Bevy desktop window. Cryptography is limited to SHA-256 content hashing via `sha2`; there is no encryption, no RNG, and no network code path. Path traversal is prevented by systematic `canonicalize + starts_with(root)` confinement in 22 source files and the equivalent `_normalized + relative_to` check in the Python bridge. See [`docs/security_audit_t30.md`](docs/security_audit_t30.md) for the full audit.

## Workspace layout

```text
crates/
├── vortex-core/  # Pure domain, application use cases, and adapter boundaries.
└── vortex-bevy/  # CLI and Bevy 0.19 runtime composition.
bridges/          # Authorized Unreal, Unity, and Godot Editor export bridges.
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

## Typed CLI and Unreal metadata preflight

The recovery CLI uses typed Clap parsers for `inspect`, `plan`, `convert`, `verify`, `view`, `support-matrix`, and `version`. It supports deterministic text and JSON reports. The concrete first adapter is `inspect --engine unreal`: it confines one explicit `.uasset` to an authorized physical project root, enforces a bounded package size, rejects cooked-style sidecars and path/symlink escapes, then extracts only header metadata through pinned `uasset` 0.6.0. A successful inspection remains `requires_adapter`; semantic export must use the matching authorized Unreal Editor bridge. See [`docs/unreal_metadata_preflight_contract.md`](docs/unreal_metadata_preflight_contract.md).

The matching-Editor hand-off is represented by a typed Rust adapter: it builds only the checked-in Unreal bridge command, confines project/staging/report paths, validates the versioned report, and maps every explicit record to a typed source identity without requiring Unreal in synthetic tests. An authorized Unreal Engine `5.8.2` controlled static-scene export has produced source-traceable FBX/TGA outputs and explicit material/level `requires_adapter` records; the received exports also passed the bounded canonical IR → deterministic GLB → atomic publication → headless Bevy path. See [`docs/unreal_static_scene_real_evidence.md`](docs/unreal_static_scene_real_evidence.md). Arbitrary world semantics, material graphs, Blueprint/C++ behavior, collision/physics, and broad FBX feature support remain separate boundaries.

T26 adds a separate, fail-closed direct **UE 5.8.2 `UTexture2D` metadata** reader for exactly one legacy asset-registry record. It reads bounded dimensions, format, compression, optional filter/hash, and bulk-reference state; it never reads pixels, object payloads, or bulk data, and still returns `requires_adapter` for authoritative export. See [`docs/unreal_texture2d_metadata_contract.md`](docs/unreal_texture2d_metadata_contract.md).

The initial USD adapter now resolves selected, root-confined **USDA** stage facts deterministically: selection, variants, payload policy, purpose, time range, render context, root units/up-axis, hierarchy, transforms, visibility, instancing, and unresolved composition arcs. Binary/plugin-backed layers remain explicit authoritative-runtime requirements; this adapter does not claim full OpenUSD composition. See [`docs/usd_stage_ingestion_contract.md`](docs/usd_stage_ingestion_contract.md).

T25 adds a deterministic flattened fallback plus explicitly prepackaged named variants, expanded instances, shared instance batches, and quantized-bounds chunks. All package GLBs publish atomically under the same manifest; Bevy can switch only among verified prebuilt alternatives and load bounded chunks with retained provenance. Payload arcs, unresolved live composition, and recursive references fail closed rather than being evaluated at runtime. See [`docs/usd_packaged_runtime_contract.md`](docs/usd_packaged_runtime_contract.md).

Source adapters now feed a Bevy-free engine-neutral canonical scene IR with typed cross-object IDs, deterministic sorting, explicit coordinate/units provenance, and one fail-closed graph-validation boundary. See [`docs/canonical_scene_ir_contract.md`](docs/canonical_scene_ir_contract.md).

The bounded Unity URP adapter inspects selected Force Text source-project records with visible `.meta` GUIDs, root-constrained paths, Unity/URP provenance, stable typed source identities, and explicit unsupported/adapter-needed cases. It accepts only the documented static scene/prefab/material subset and does not replace matching-Editor import resolution. See [`docs/unity_urp_source_ingestion_contract.md`](docs/unity_urp_source_ingestion_contract.md).

The recovered deterministic GLB package boundary emits validated indexed geometry, PBR texture/image/sampler records, baseline Unity URP Lit/Unlit material mappings, skins, conventional animation clips, and position morph targets. It rejects unknown texture/material slots and malformed deformation facts rather than producing partial artifacts. See [`docs/glb_texture_material_deformation_contract.md`](docs/glb_texture_material_deformation_contract.md).

The atomic artifact store now stages complete payloads under the typed output root, fsyncs practical boundaries, verifies manifest hashes, acquires a single-writer conversion lock, and renames only complete directories into place. Replay verifies and reuses an equivalent final conversion; verify, report retrieval, and cleanup remain fail-closed and root-confined. See [`docs/artifact_publication_contract.md`](docs/artifact_publication_contract.md).

The mandatory synthetic T22 suite resolves owned USDA facts, validates a canonical scene, packages byte-stable GLB output, atomically publishes and verifies it, then loads it through headless Bevy. Its generated [`docs/compatibility_matrix.json`](docs/compatibility_matrix.json) is the JSON form of the same typed support matrix exposed by the CLI. A separate opt-in real Unreal static-mesh FBX/TGA path now traverses the same canonical packaging/publication/headless-loading contract without introducing proprietary CI fixtures. See [`docs/unreal_static_scene_real_evidence.md`](docs/unreal_static_scene_real_evidence.md).

T27 adds Criterion benchmarks for the owned Unreal metadata decoder, selected USDA resolution, canonical IR validation, bounded texture normalization, GLB writing, manifest parsing, and headless runtime root spawn. Conversion tracing records only deterministic conversion correlation fields and uses an opaque source fingerprint; synchronous admission controls support bounded in-flight work and caller-provided memory reservations. Measurements are host-specific observations, not runtime guarantees. See [`docs/performance_baseline.md`](docs/performance_baseline.md).

T28 adds a Linux `/proc/self/status` RSS/high-water sampler and benchmark-only `System` allocation counters, plus license-tracked production-scale USDA and PNG integration fixtures. These are aggregate profiling observations rather than allocation enforcement or memory-leak conclusions. The complete installation, support, authorization, CLI, output, runtime, performance, fixture, migration, and troubleshooting guide is [`docs/operations_and_architecture_guide.md`](docs/operations_and_architecture_guide.md).

T29 adds an optional caller-owned, in-process selected-USDA queue. It accepts only root-confined local `.usda` and `.usda.gz` requests, has a bounded non-blocking backlog, limits plain and decompressed stage bytes, reserves capacity through the existing governor before reading, and reports one typed result per request. It is for isolation and bounded throughput, not a faster single-stage parser, network streaming, archive traversal, or general OpenUSD composition. The measured bottleneck analysis is [`docs/t29_streaming_bottleneck_analysis.md`](docs/t29_streaming_bottleneck_analysis.md); release and recovery procedures are in [`docs/release_hardening.md`](docs/release_hardening.md).

The T23 owned Unity URP fixture suite verifies Force Text source inspection, Lit/Unlit material mapping, canonical source links, deterministic GLB and manifest publication, and headless Bevy material spawning. A separate authorized real Unity `6000.5.10f1` / URP `17.5.0` batch validation has now traversed a controlled static mesh, generated URP Lit material and PNG, camera, directional light, and explicit `BoxCollider` `requires_adapter` result through typed canonical IR, deterministic GLB, atomic publication, and headless Bevy component instantiation. It preserves the temporary scene GUID/object identity and was independently verified to restore the user project. See [`docs/unity_static_scene_real_evidence.md`](docs/unity_static_scene_real_evidence.md). Broader imported-asset import resolution and Shader Graph behavior remain Editor-gated or unsupported. The earlier selected-FBX and complex-VFX smoke boundary is recorded in [`docs/unity_editor_bridge_smoke_evidence.md`](docs/unity_editor_bridge_smoke_evidence.md).

The remaining conversion commands are deliberately explicit about their recovery status: they parse and validate typed inputs but report unavailable downstream capability until canonical IR, packaging, publication, and artifact discovery are rebuilt. They do not pretend to convert an asset.

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
cargo audit --no-fetch
bash scripts/check-bridge-quality.sh
bash scripts/check-release-readiness.sh
```

The Wiggum implementation plan is `plan.toml`. Generated task state is maintained in `PROGRESS.md`; do not edit generated task filenames by hand without regenerating the scaffold.

## User guide

The user-focused MDBook lives under [`book/`](book/src/SUMMARY.md) and
covers install, build, the end-to-end USD tutorial, per-engine
classification outcomes, the CLI flag reference, the scene manifest
schema, the security and architecture explanations, and the
performance baseline. Build it locally with `mdbook build book/`
(requires `cargo install mdbook --locked`) or read it on the
project site.

## Authorized Editor bridges

The repository provides isolated Unreal Python, Unity Editor C#, and Godot headless GDScript bridges in [`bridges/`](bridges/README.md). They only process caller-selected assets from authorized editable projects, confine all reports and staged files to a caller-provided root, and emit the versioned `vortex.editor-bridge/v1` contract. The bridge quality gate applies strict Python, C#, and GDScript formatting/linting plus deterministic contract tests without requiring any Editor in normal CI. Actual Editor execution remains opt-in and requires the matching authorized project and Editor version.

## Fixture policy

Only self-authored or explicitly authorized fixtures belong in this repository. Do not commit Marketplace assets, game assets, Unreal projects, package payloads, encryption material, or generated large binaries without explicit written authorization. See [`fixtures/README.md`](fixtures/README.md).
