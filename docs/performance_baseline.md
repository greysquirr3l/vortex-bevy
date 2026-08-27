# Performance and Observability Baseline

This document records **measured observations**, not production performance guarantees. The measurements below use repository-owned fixtures plus two license-tracked external fixtures retained under explicit authorization, a single Cargo job, and the pinned Rust toolchain. They are not comparable across host classes, operating systems, storage devices, allocators, or CPU power policies.

## Reproducibility

| Field | Value |
|---|---|
| Git base revision | `49375be03b6beadfccf21e4a6a5c59406d366d9f` before the uncommitted T27 changes |
| Rust compiler | `rustc 1.97.1 (8bab26f4f 2026-07-14)` |
| Cargo | `cargo 1.97.1 (c980f4866 2026-06-30)` |
| Host | Linux `6.1.102`, x86_64, Intel Xeon 2.50 GHz, 3.8 GiB reported memory |
| Build concurrency | `CARGO_BUILD_JOBS=1` |
| Benchmark framework | Criterion 0.5.1, 3-second warm-up, 100 samples, 5-second target measurement time |
| Plot backend | Plotters; Gnuplot was unavailable |

The original `cargo bench --workspace` baseline completed successfully but exposed **zero timed benchmark targets**. T27 adds the owned-fixture targets below. The commands used for the recorded measurements were:

```bash
CARGO_BUILD_JOBS=1 cargo bench -p vortex-core --bench core_pipeline
CARGO_BUILD_JOBS=1 cargo bench -p vortex-bevy --bench runtime_spawn
```

## Owned benchmark fixtures

| Target | Fixture boundary |
|---|---|
| `unreal_texture2d_metadata/decode` | One in-memory, self-authored UE 5.8.2-shaped `UTexture2D` asset-registry metadata record with three required tags, no pixels, and no bulk-data traversal. |
| `usd_stage/resolve_owned_triangle` | The repository-owned `fixtures/minimal_usda/triangle_scene.usda` selected USDA triangle stage. |
| `canonical_ir/validate_triangle` | One source link, one node, one mesh, one primitive, three vertices, one triangle, and one PBR material. |
| `texture/normalize_owned_png` | One generated 2×2 RGBA PNG declaration; no external image is read. |
| `glb/package_triangle` | The same three-vertex, one-triangle canonical scene. |
| `manifest/parse_owned_fixture` | One versioned manifest with one GLB record and one source record. |
| `runtime/build_app_verify_manifest_and_spawn_scene_root` | One temporary, generated minimal GLB plus its checksum-verified manifest. The fixture is preflighted before timing and deleted on process cleanup. |

No user project, `.uasset`, staged Editor report, GLB, image, package payload, or source path is benchmarked or retained.

## Production-scale fixtures and T28 measurements

T28 retains one production-shaped USD layer and one real-world diffuse PNG. Each file is below the enforced 5 MiB per-file fixture cap, has a sibling checksum/provenance manifest, and is covered by the fixture-policy and checksum gates. The USD input intentionally terminates at the existing `requires_adapter` boundary because it contains variants and complex shader-network facts; this test does not misrepresent incomplete live USD composition as GLB conversion. [1] [2] [3]

| Target | Fixture and source | Exact size | 95% timing interval | Result boundary |
|---|---|---:|---:|---|
| `usd_stage/resolve_standard_shader_ball` | ASWF USD Working Group StandardShaderBall `shaderball.usda`, CC BY 4.0, pinned to `3b75c2dad6a494897557dcca0098257bcf42a8c6` | 3,354,071 bytes | 22.391–22.636 ms | Deterministic typed stage facts and `requires_adapter`; no referenced-layer or MaterialX/EXR evaluation. |
| `texture/normalize_polyhaven_wooden_planks_1k` | Poly Haven `wooden_planks` diffuse 1k PNG, CC0 1.0 | 5,016,652 bytes; 1024×1024 16-bit RGB | 27.440–27.523 ms | Bounded PNG header validation, SHA-256 staging identity, typed sampler, and canonical texture metadata; it is not pixel decoding or GPU upload. |

The full extended core run used Criterion’s observed default configuration of a 3-second warm-up, 100 samples, and a 5-second target measurement interval. The command line attempted to request a shorter run, but Criterion reported and executed the default sample configuration, so only the parameters it actually reported are documented here. The measured intervals are therefore reproducible only when the same reported configuration and host constraints are retained.

## T28 RSS and allocator observations

`LinuxProcessMemoryReader` reads only `VmRSS` and `VmHWM` from `/proc/self/status`. `stats_alloc` wraps `System` in the core benchmark executable and reports allocation calls and byte requests for the process-wide benchmark region. The result is held during the “during” sample and dropped before the “after release” sample. This is a bounded measurement design: it identifies material allocation and RSS changes without adding unsafe code, a production global allocator replacement, raw source telemetry, or a claim that the counters belong exclusively to one thread. [4] [5]

| One-time operation | RSS before | RSS while result live | RSS after result drop | Peak RSS while sampled | Allocations / deallocations / reallocations | Bytes allocated / deallocated / reallocated |
|---|---:|---:|---:|---:|---:|---:|
| `usd_stage/resolve_owned_triangle` | 13,524,992 | 13,524,992 | 13,524,992 | 13,524,992 | 97 / 53 / 21 | 8,131 / 5,304 / 3,106 |
| `usd_stage/resolve_standard_shader_ball` | 13,524,992 | 13,983,744 | 13,983,744 | 17,256,448 | 2,473 / 2,106 / 171 | 3,480,902 / 3,434,823 / 43,815 |
| `texture/normalize_polyhaven_wooden_planks_1k` | 13,983,744 | 18,845,696 | 13,983,744 | 18,845,696 | 13 / 5 / 8 | 5,020,895 / 2,276 / 2,136 |

The production USD stage increased the observed RSS by 458,752 bytes while its snapshot stayed live, with a 17,256,448-byte high-water mark. The large PNG normalization increased RSS by 4,861,952 bytes while its staged result was live and returned to its pre-operation RSS after that result was dropped. These values are host observations, not ceiling guarantees. RSS can remain elevated because the operating system, C runtime, or allocator retains pages; conversely, allocator request counts do not account for all non-allocator memory or resident-page effects.

## Measured observations

| Target | 95% interval | Interpretation |
|---|---:|---|
| `unreal_texture2d_metadata/decode` | 275.67–277.69 ns | Metadata-only parsing of the small owned record. This does not measure texture decoding. |
| `usd_stage/resolve_owned_triangle` | 40.115–42.193 µs | Root-confined selected USDA stage resolution. |
| `canonical_ir/validate_triangle` | 1.3616–1.3822 µs | Typed canonical scene construction and validation. |
| `texture/normalize_owned_png` | 1.4380–1.4503 µs | Bounded generated PNG normalization. |
| `glb/package_triangle` | 26.994–27.286 µs | Deterministic GLB packaging for the owned triangle. |
| `manifest/parse_owned_fixture` | 11.214–11.314 µs | Bounded manifest parse of the owned record. |
| `runtime/build_app_verify_manifest_and_spawn_scene_root` | 5.1815–5.4583 ms | Fresh headless app construction, manifest verification, checksum validation, and root spawn together. It is not GPU render time. |

The runtime target required approximately 13.2 seconds for 100 samples on this host, so Criterion expanded its collection estimate beyond the nominal 5-second target. Outlier reports are retained in the Criterion output and are expected on shared sandbox hardware; the intervals above are descriptive rather than gating commitments.

## Structured observations and resource admission

Conversion execution now emits the `vortex.conversion.execute` span and one admission event. The only correlation fields are the typed deterministic `ConversionId`, source-engine family, opaque SHA-256 source fingerprint, and plan action. The raw project root, asset path, object path, environment data, secrets, report text, and untrusted documents are never recorded by this instrumentation.

`ConversionResourceGovernor` provides opt-in synchronous admission at the application boundary. It rejects a request larger than the configured aggregate byte ceiling and rejects work when the in-flight slot or reserved-byte capacity is exhausted. Its RAII permit releases capacity at scope exit; it does not expose a mutex guard, so a guard cannot cross an asynchronous boundary.

> Resource estimates are caller-provided admission budgets, not measured resident-memory accounting. They are a fail-closed safety control rather than a claim of exact allocator usage.

The redaction and bounded-admission contracts are covered by repository tests. T28 additionally runs end-to-end bounded-adapter tests over both production fixtures and verifies nonzero RSS/allocation observations without asserting hardware-specific values. See [`operations_and_architecture_guide.md`](operations_and_architecture_guide.md) for operator guidance and [`fixtures/production_scale/NOTICE.md`](../fixtures/production_scale/NOTICE.md) for source provenance.

## References

[1]: ../fixtures/production_scale/NOTICE.md "Production-scale fixture provenance"
[2]: https://github.com/usd-wg/assets/blob/3b75c2dad6a494897557dcca0098257bcf42a8c6/full_assets/StandardShaderBall/README.md "StandardShaderBall README"
[3]: https://polyhaven.com/license "Poly Haven Asset License"
[4]: https://man7.org/linux/man-pages/man5/proc_pid_status.5.html "/proc/pid/status manual"
[5]: https://docs.rs/stats_alloc/0.1.10/stats_alloc/ "stats_alloc 0.1.10 documentation"
