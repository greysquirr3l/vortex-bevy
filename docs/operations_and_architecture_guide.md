# Operations and architecture guide

Vortex-Bevy is an **authorized asset-conversion and runtime-viewing workspace** for Rust 1.97.1 and Bevy 0.19. It produces deterministic Bevy-compatible GLB artifacts plus a versioned `scene.manifest.ron` sidecar. The manifest, rather than native live OpenUSD composition, is the runtime contract: it records source provenance, selected configuration, artifact hashes, compatibility requirements, package declarations, classification, and diagnostics. [1] [2]

## Installation and prerequisites

Use a Rust 1.97.1 toolchain, a supported native C compiler/linker, and the repository’s pinned dependency graph. The standard verification path also requires `cargo-deny` and `cargo-audit`. The Editor bridges are opt-in: an authorized editable source project and its matching Editor version are required only for an actual Editor bridge invocation.

```bash
cargo build --workspace --all-targets --all-features
cargo test --workspace --all-features
cargo fmt --all -- --check
cargo l
cargo doc --workspace --no-deps
```

The project deliberately supports no global `ports/` package. `vortex-core` contains pure domain values, application commands and queries, and concrete adapter boundaries; `vortex-bevy` composes the CLI and Bevy 0.19 runtime. A command or query owns the narrow trait it consumes, so a new adapter is introduced beside its specific consumer after parsing external DTOs into typed domain values. SDK types, untyped external strings, and engine runtime objects do not cross that boundary. [3]

## Authorization and security boundary

An input is authorized only when the caller has the right to supply it and explicitly selects its editable project root, asset/stage path, output root, and—where applicable—Unreal object path. Root-confinement, canonicalization, symlink checks, size limits, manifest validation, checksum verification, and atomic publication prevent an accepted request from implicitly widening that authority. The tool rejects cooked, encrypted, unowned, DRM-protected, key-dependent, or otherwise unauthorized content; it neither extracts keys nor claims Blueprint/C++ behavior conversion. [4]

Structured tracing retains only conversion ID, engine family, and a SHA-256 source-identity fingerprint. T28 RSS and allocator observations deliberately contain process-wide counters only: they do not capture asset paths, project roots, environment variables, text content, credentials, or payload bytes. Allocation observations use a benchmark-only system-allocator wrapper and must not be interpreted as exact per-thread or production allocator accounting. [5]

## Tested support and classifications

The generated JSON support matrix is authoritative for individual capability rows and is emitted by `vortex-bevy support-matrix --format json`. The following condensed table is an operator-oriented summary; run the CLI for the complete generated data. [6]

| Input route | Exact tested basis | Output route | Classification and limit |
|---|---|---|---|
| Authorized Unreal controlled static scene | Unreal Engine 5.8.2 Editor bridge evidence | Bounded FBX/TGA ingestion → canonical IR → deterministic GLB → manifest → headless Bevy | Static-scene subset can convert. Worlds, Blueprints/C++, broad material graphs, collision/physics, and unsupported FBX features remain explicit boundaries. |
| Direct Unreal `UTexture2D` metadata | UE 5.8.2; one legacy Asset Registry record | Typed metadata facts only | `requires_adapter`; no pixel/object/bulk payload read, `.ubulk` decode, or semantic export. |
| Selected OpenUSD stage | Root-confined text USDA facts | Typed stage facts and package planning | `requires_adapter` for full composition. Binary/plugin-backed layers, payload evaluation, recursive references, and arbitrary live variants require an authorized OpenUSD runtime adapter. |
| USD package alternatives | Owned synthetic and documented package paths | Prebuilt flattened, variant, instance, batch, and chunk GLBs | Verified prepackaged selections only; runtime does not author arbitrary USD variants or composition. |
| Unity URP static source | Unity 6000.5.10f1 / URP 17.5.0 Editor evidence | Canonical IR → GLB/manifest → headless Bevy | The documented static scene/prefab/material subset can convert; Shader Graph and unresolved import semantics remain Editor-gated. |
| Godot controlled static scene | Godot 4.7.2 Editor evidence | External bridge GLB/PNG/report → manifest → headless Bevy | Controlled scene can convert; `GPUParticles3D` remains `requires_adapter`. |

Every result has exactly one classification. **Converted** means the documented bounded path succeeded. **Baked** means a static output intentionally replaces unsupported live behavior. **Requires adapter** means a typed fact was recovered but an authoritative runtime/bridge is required for the next semantic step. **Unsupported** identifies a recognized excluded feature. **Rejected** denies an unsafe, malformed, oversized, unauthorised, or incompatible request.

## CLI reference and documented command checks

The CLI accepts `inspect`, `plan`, `convert`, `verify`, `view`, `support-matrix`, and `version`. Every example below is non-mutating except `convert` without `--dry-run` and `view`; the documentation check executes the shown `--help`, `version`, and support-matrix commands against the built binary. Actual conversions must replace the illustrative values only after the caller has authorized the source and output roots.

| Purpose | Validated command | Operator outcome |
|---|---|---|
| Verify binary identity | `cargo run -q -p vortex-bevy -- version --format json` | Stable JSON version report. |
| Print complete support | `cargo run -q -p vortex-bevy -- support-matrix --format json` | Generated typed support report; compare with `docs/compatibility_matrix.json`. |
| Discover safe input arguments | `cargo run -q -p vortex-bevy -- inspect --help` | Shows the required `--project`, `--engine`, and `--asset` fields. |
| Plan with no writes | `cargo run -q -p vortex-bevy -- plan --help` | Shows `--profile`, output-root, selection, payload policy, and `--dry-run` controls. |
| Discover conversion controls | `cargo run -q -p vortex-bevy -- convert --help` | Shows the same typed authorization and output controls used for real publication. |
| Discover verification/view controls | `cargo run -q -p vortex-bevy -- verify --help` and `cargo run -q -p vortex-bevy -- view --help` | Shows verified conversion-ID and artifact-root boundaries. |

`--engine` accepts `unreal`, `usd`, or `unity`. `--profile` is a caller-supplied validated deterministic token, not an unbounded engine setting. USD selection uses `--variant`, `--payload-policy forbid|metadata-only`, purpose, time, and render-context facts where the selected adapter supports them. A production operator should run `inspect`, then `plan --dry-run`, then authorize a non-dry-run conversion only after reviewing its typed report.

## Output layout, verification, and Bevy runtime

A completed conversion publishes a directory owned by the selected Vortex output root. The directory contains one `scene.manifest.ron` and the GLB/texture artifacts named by that manifest. Publication stages complete content, writes metadata, verifies hashes, and renames the finished directory atomically; equivalent replay validates and reuses a compatible prior result. Never edit an artifact directory by hand. Use `verify --conversion-id` for an existing report’s conversion identity, and use `view --artifact-root` only on a verified artifact directory. [1]

Bevy 0.19 verifies the manifest and GLB checksum before spawning `WorldAssetRoot`. It retains the artifact hash, source provenance, classifications, diagnostics, package alternatives, and chunk state on the root entity. The optional native `bevy_egui` view has pointer-gated orbit/pan/zoom and a framed selection; it is a consumer of verified artifact data, not an OpenUSD evaluator or an Editor replacement. [2] [7]

## Performance, RSS, allocator, and admission operations

The T27 Criterion baseline is host-specific and uses repository-owned fixtures. T28 extends it with license-tracked production-shaped inputs: a 3,354,071-byte StandardShaderBall USDA layer and a 5,016,652-byte Poly Haven 1024×1024 16-bit RGB PNG. Both remain below the repository 5 MiB per-file fixture cap. [5] [8]

On GNU/Linux, `LinuxProcessMemoryReader` samples only `VmRSS` and `VmHWM` from `/proc/self/status`. Benchmark targets use the MIT-licensed `stats_alloc` wrapper around `System` to report aggregate allocation/deallocation/reallocation counts and byte requests while the tested result remains live. The subsequent post-drop RSS observation distinguishes retained allocator pages from logical result lifetime. These counters are **observations**, not allocation limits and not a memory-leak verdict.

Admission control is independent of profiling. `ConversionResourceGovernor` reserves a caller-declared estimated-byte budget plus one concurrency slot before adapter execution. It rejects a single request larger than the configured budget or any request that would exceed current slot/byte reservations; the RAII permit releases capacity before the synchronous handler returns. It does not allocate the requested bytes, sample process RSS, or guarantee that a caller’s estimate matches physical memory use. [5]

## Fixture policy and provenance

Fixtures are checked-in only when they are self-authored or explicitly authorized. The policy rejects Unreal package/payload/cooked formats, key/certificate material, and any individual fixture file above 5 MiB. A sibling `.fixture.json` pins source filename and SHA-256. The production-scale fixture notice and manifests additionally record upstream revision, license, attribution, upstream checksum when published, and deliberate omissions. [8]

The retained StandardShaderBall layer is sourced from the ASWF USD Working Group asset repository at an immutable revision and carries its required CC BY 4.0 attribution. The retained `wooden_planks` diffuse texture comes from Poly Haven under CC0 1.0. The complete upstream shader-ball bundle, its referenced images/layers, and every non-required PBR map are excluded to preserve the fixture-size and scope boundary. [9] [10]

## Migration and extension guidance

Existing consumers should continue to treat GLB plus `scene.manifest.ron` as the portable load contract. Adding an adapter must first define a typed bounded request and domain outcome, keep engine SDK calls/DTO parsing inside a focused secondary adapter, provide synthetic or license-tracked fixtures, and publish exact support-matrix wording. Do not add a shared port, expose a raw path/string beyond parsing, or represent a partial conversion as `converted`.

A new direct Unreal decoder requires human approval for the asset class, exact Unreal version, fixture provenance, independent oracle protocol, and acceptance conditions. A new real-Editor test remains ignored/opt-in until a user provides an authorized project, object path, and caller-owned staging root; its committed test must retain no user bytes. A new USD composition capability requires an explicit policy for payloads, references, recursion, variants, resources, and provenance—not an implicit parser extension.

## Troubleshooting

| Symptom | Likely boundary | Correct action |
|---|---|---|
| `requires_adapter` for a valid asset | Semantic export/composition is deliberately outside the direct reader | Use the matching authorized Editor bridge or approved OpenUSD runtime adapter; do not reclassify the asset. |
| `rejected` after selecting a path | Path escape, symlink, unauthorized root, size limit, malformed data, or cooked/encrypted boundary | Recheck the explicitly authorized root/path and report fields. Do not weaken confinement or provide keys. |
| Manifest/GLB checksum failure | Artifact directory was incomplete, altered, or does not match its manifest | Remove only the Vortex-owned output directory and re-run the authorized conversion; do not patch hashes. |
| Bevy cannot load a texture | The input/output encoding is outside the runtime feature set | Check the manifest and documented supported texture path; convert through an approved bounded adapter rather than guessing a decoder. |
| RSS remains elevated after result drop | Allocator/C runtime retained pages or unrelated process work | Compare allocator counters, repeat on a quiet host, and profile representative authorized data before diagnosing a leak. |
| Benchmarks vary across runs | CPU governor, cache, sharing, compiler/LTO configuration, or host load | Record toolchain/host details, repeat the same command, and compare intervals—not single-point timings. |

## References

[1]: artifact_publication_contract.md "Atomic artifact publication contract"
[2]: bevy_runtime_loading_contract.md "Bevy runtime loading contract"
[3]: ../README.md "Vortex-Bevy workspace architecture"
[4]: unreal_metadata_preflight_contract.md "Unreal metadata preflight and authorization contract"
[5]: performance_baseline.md "Performance baseline and observability limits"
[6]: compatibility_matrix.json "Generated compatibility matrix"
[7]: runtime_scene_lifecycle_contract.md "Runtime scene lifecycle contract"
[8]: ../fixtures/production_scale/NOTICE.md "Production-scale fixture provenance"
[9]: https://github.com/usd-wg/assets/blob/3b75c2dad6a494897557dcca0098257bcf42a8c6/full_assets/StandardShaderBall/README.md "StandardShaderBall README"
[10]: https://polyhaven.com/license "Poly Haven Asset License"
