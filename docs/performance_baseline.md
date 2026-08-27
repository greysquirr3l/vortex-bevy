# Performance and Observability Baseline

This document records **measured observations**, not production performance guarantees. The measurements below use only repository-owned fixture definitions, a single Cargo job, and the pinned Rust toolchain. They are not comparable across host classes, operating systems, storage devices, or CPU power policies.

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

The redaction and bounded-admission contracts are covered by repository tests. See the T27 benchmark targets under `crates/*/benches/` for reruns and [`README.md`](../README.md) for the supported conversion boundary.
