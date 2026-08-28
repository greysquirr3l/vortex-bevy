---
name: usda-local-profiling
description: Profile vortex-bevy's production USDA stage-resolution benchmark on a developer-owned macOS or Linux machine. Use when collecting a function-level Instruments or cargo-flamegraph breakdown before changing USDA parsing, memory mapping, parallelization, or snapshot caching.
---

# Local USDA Profiling

Profile only the repository-owned `StandardShaderBall` fixture by default. Treat any user project, USDA, trace, screenshot, and profile export as private; keep them outside the repository unless the user explicitly authorizes a redacted derived summary.

## Scope and invariant

Measure `usd_stage/resolve_standard_shader_ball` from `crates/vortex-core/benches/core_pipeline.rs`. This benchmark performs bounded selected-stage inspection of the checked-in production fixture. It does **not** measure OpenUSD composition, referenced layers, payloads, MaterialX/EXR, mesh conversion, GLB packaging, or GPU work.

Before profiling, confirm the checkout is clean and retain these fields in the findings record: commit SHA, `Cargo.lock` hash, fixture SHA-256, operating-system version, CPU model, physical-memory size, Rust version, profiler version, and exact command.

```bash
git status --short
git rev-parse HEAD
shasum -a 256 Cargo.lock \
  fixtures/production_scale/standard_shader_ball/shaderball.usda
rustc --version
```

Use `CARGO_BUILD_JOBS=1` for reproducible low-contention local runs unless the goal explicitly measures parallel throughput. Do not run multiple profilers or benchmark processes concurrently.

## macOS: Instruments Time Profiler

Install Xcode Command Line Tools if `xcrun` is unavailable. Build the benchmark first with release symbols; do not include compilation in the recording.

```bash
export CARGO_BUILD_JOBS=1
export CARGO_PROFILE_RELEASE_DEBUG=true
cargo bench -p vortex-core --bench core_pipeline --no-run
```

Identify the single executable created for the benchmark, then record only the production stage filter. Put trace output in a user-owned directory outside the checkout.

```bash
PROFILE_ROOT="$HOME/vortex-profiles/usda-$(git rev-parse --short HEAD)"
mkdir -p "$PROFILE_ROOT"
BENCH_BIN="$(find target/release/deps -maxdepth 1 -type f -name 'core_pipeline-*' -perm -111 | head -n 1)"
test -n "$BENCH_BIN"
xcrun xctrace record \
  --template 'Time Profiler' \
  --output "$PROFILE_ROOT/usda-stage.trace" \
  --launch -- "$BENCH_BIN" usd_stage/resolve_standard_shader_ball
```

Open the trace in Instruments. Use the Time Profiler call tree with **Invert Call Tree** and **Hide System Libraries** enabled. Record both inclusive and exclusive percentages for the top functions. Separate `fs::read`, UTF-8 creation, line splitting, parser helpers, ordered-collection insertion, sorting/deduplication, and snapshot construction when they appear. Export only a redacted summary; a raw trace may contain absolute local paths.

If the benchmark executable cannot be identified uniquely, list matching files and select the executable explicitly. Do not profile `cargo` itself or the entire Criterion suite.

## Linux: cargo flamegraph

Use `cargo flamegraph` only on a developer-owned Linux machine with `perf` sampling permission. Install the maintained wrapper and kernel-matched performance tools using the local platform's package manager. Build symbols and write all output outside the checkout.

```bash
cargo install flamegraph --locked
export CARGO_BUILD_JOBS=1
export CARGO_PROFILE_RELEASE_DEBUG=true
PROFILE_ROOT="$HOME/vortex-profiles/usda-$(git rev-parse --short HEAD)"
mkdir -p "$PROFILE_ROOT"
cargo flamegraph \
  --output "$PROFILE_ROOT/usda-stage.svg" \
  --bench core_pipeline -- \
  usd_stage/resolve_standard_shader_ball
```

If `perf` rejects access, do **not** weaken system policy from an automated script. Have the machine owner grant the minimum temporary sampling permission, then rerun the identical command. Record the value of `kernel.perf_event_paranoid` in the findings. Do not use root merely to avoid understanding the local performance-counter policy.

Inspect the SVG with a browser and record the top inclusive stacks. Flamegraphs visualize sampled CPU time, not allocation count, wall-clock latency, I/O wait, or total memory use. Pair the profile with the existing Criterion interval and the benchmark's RSS/allocation observation.

## Analysis workflow

1. Capture one direct profile with the repository-owned fixture.
2. Validate the profiler target: the top stack must include `UsdStageInspector::resolve` and `parse_usda`.
3. Attribute work before proposing a change. Full-file read, UTF-8 conversion, line iteration, parsing, `BTreeSet`/`BTreeMap` insertion, `Vec` growth, sorting, deduplication, and typed identity creation are distinct costs.
4. Compare call-tree percentages with the independent Criterion interval. Do not add threads, memory mapping, or caching solely because a phase feels expensive.
5. For a prospective cache, key entries by canonical root-confined file identity, immutable content hash, normalized selection, parser/schema version, and relevant bounds. Invalidate on any key change; never cache an error as success.
6. For prospective chunking, preserve parser order and brace/parenthesis state at chunk boundaries. Reject malformed input before publication, cap workers/queue depth/bytes with the resource governor, and retain a deterministic single-thread fallback.
7. Record the outcome using `templates/profile_findings.md`. Never commit raw traces, user paths, hostnames, or CPU serial details.

## Interpretation guardrails

| Observation | Allowed conclusion | Not allowed |
|---|---|---|
| File-read stack is dominant | Investigate bounded mapped reads or read-buffer reuse | Claim that parsing is the bottleneck |
| Parser and allocation stacks dominate | Investigate borrowed scanning, bounded reuse, or a validated cache | Claim that memory mapping will improve CPU parsing |
| Ordered collections/sorting dominate | Measure deterministic alternatives while preserving output order | Replace ordering with nondeterministic maps |
| Cache hit is materially faster | Consider a content-addressed snapshot cache with strict invalidation | Reuse a snapshot after content or selection changes |
| One core is saturated | Evaluate bounded parallelism with representative authorized inputs | Assume parallel parsing preserves grammar correctness |

## Required report

Create a redacted Markdown findings report from `templates/profile_findings.md`. Include failed or unsupported profiling attempts as limitations. Make the report explicit that all numbers are machine-specific observations, not service-level guarantees.

Do not modify parser code, memory-mapping behavior, thread counts, queue limits, cache semantics, or public support claims until the captured profile and a deterministic test plan have been reviewed.
