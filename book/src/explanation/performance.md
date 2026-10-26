# Performance and observability

Vortex-Bevy is deterministic. Every output depends only on typed
source inputs and the export profile — never on wall-clock time,
unordered map iteration, environment variables, or process-global
mutable state. This page documents what the project measures for
performance and observability, and what it deliberately does not
measure.

For the observability contracts, see
[`docs/security_audit_t30.md`](../../../docs/security_audit_t30.md).

## Performance baseline

The performance baseline is established by owned-fixture Criterion
benchmarks in `crates/vortex-core/benches/core_pipeline.rs` and
`crates/vortex-bevy/benches/runtime_spawn.rs`. The benchmarks cover:

- Direct `UTexture2D` metadata parsing (T26 path).
- USDA stage resolution (T11 path).
- Canonical IR validation.
- Texture normalization.
- GLB packaging.
- Manifest parsing.
- Headless Bevy 0.20 app construction.
- Headless Bevy manifest verification.
- Headless Bevy scene spawn.

To reproduce the baseline:

```bash
cargo bench --workspace --all-features
```

The benchmarks use owned, self-authored fixtures only. They do not
depend on licensed Unreal Marketplace assets, game assets, encryption
keys, or proprietary payloads.

## Determinism

Conversion outputs are deterministic from typed inputs. The
`ConversionId` is derived from `SHA-256("vortex-conversion-v1" ||
source || profile_hash)` and is stable across hosts and across time.
Re-running the same conversion on a different host or at a different
wall-clock time produces byte-identical GLB and manifest outputs.

There are no wall-clock timestamps in the manifest, no environment
variables, no random identifiers, and no process-global mutable state
in the conversion path. Unordered `HashMap` iteration is avoided;
every collection in the domain is sorted deterministically before
serialization.

## Resource limits

T29 introduces bounded resource admission for the conversion pipeline.
The `ConversionResourceGovernor` enforces:

- A bounded concurrency limit.
- A bounded memory budget.

The governor acquires per-conversion permits and releases them before
returning. Failed admission returns `CapacityExhausted` or
`MemoryBudgetExceeded` typed errors before any adapter runs.

The `StreamingUsdStageQueue` adds a bounded worker queue for selected
USDA parsing. The queue length, worker count, and per-job memory
budget are all bounded and documented in this chapter.

The queue deliberately does not:

- add network streaming,
- (add) general USD composition,
- (add) output publication,
- retain user assets past the queue's lifetime.

## What the project measures

`tracing` events are emitted at typed boundaries:

- `ConversionId` — the typed identifier for one conversion.
- `SourceEngine` — which engine produced the source.
- `ConversionId`-correlated spans for `vortex.conversion.execute`.
- `SubmissionId` for streaming USDA submissions.

The events are emitted only at the conversion boundary. They do not
contain source paths, asset paths, project roots, environment
variables, text content, credentials, or payload bytes.

## What the project deliberately does not measure

Per the security audit:

- **Allocation observations are benchmark-only.** The
  `LinuxProcessMemoryReader` (`crates/vortex-core/src/adapters/secondary/linux_process_memory.rs`)
  reads `/proc/self/status` and returns aggregate RSS + peak RSS. It
  does not read heap allocations, source identities, open files,
  environment variables, or other process metadata. It is host-scoped
  (Linux only) and is intended for benchmark introspection, not
  production allocation accounting.
- **RSS remains elevated after result drop.** The allocator / C
  runtime retains pages or unrelated process work. The Linux process
  sampler is a snapshot, not a leak detector. Compare allocator
  counters, repeat on a quiet host, and profile representative
  authorized data before diagnosing a leak.
- **Benchmarks vary across runs.** CPU governor, cache, sharing,
  compiler/LTO configuration, or host load affect single-point
  timings. Record toolchain / host details, repeat the same command,
  and compare intervals.

## The Linux process sampler is not a leak detector

The `LinuxProcessMemoryReader` is intentionally narrow:

```rust,ignore
let resident_bytes = parse_metric(status, "VmRSS:")?;
let peak_resident_bytes = parse_metric(status, "VmHWM:")?;
```

It reads only the aggregate and high-water-mark RSS values. It does
not:

- map allocations to source identities,
- track per-thread or per-allocation-site memory,
- count open file descriptors,
- enumerate environment variables,
- record command-line arguments,
- read process credentials or capabilities.

The benchmark wrapper `stats_alloc` is a system-allocator wrapper,
not a per-thread production allocator. The sampler is documented as
"host-scoped observability", and benchmarks that use it must
explicitly acknowledge the limitation.

## Troubleshooting benchmark variance

If a benchmark result diverges from the documented baseline:

1. **Record the toolchain.** `rustc --version`, `cargo --version`,
   and any non-default RUSTFLAGS.
2. **Record the host.** CPU governor mode (`performance` / `powersave`),
   cache size, sharing, and load.
3. **Repeat the same command** three times and compare intervals,
   not single-point timings.
4. **Profile representative authorized data** before diagnosing a
   regression. Owned self-authored fixtures are not a representative
   workload; the production-scale fixtures under
   `fixtures/production_scale/` are documented in
   [`fixtures/production_scale/NOTICE.md`](../../../fixtures/production_scale/NOTICE.md).

## Where to go next

- [Architecture overview](architecture.md) — the collapsed DDD/Hex
  layout
- [Security model](security.md) — the offline threat model and audit
- [Reference: Scene manifest schema](../reference/scene-manifest.md)
