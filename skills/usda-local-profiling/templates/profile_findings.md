# USDA Stage-Resolution Profiling Findings

## Reproducibility record

| Field | Value |
|---|---|
| Commit | `<git SHA>` |
| `Cargo.lock` SHA-256 | `<hash>` |
| Fixture SHA-256 | `<hash>` |
| Host | `<redacted OS and CPU model>` |
| Physical memory | `<bytes or GiB>` |
| Rust / profiler | `<versions>` |
| Benchmark filter | `usd_stage/resolve_standard_shader_ball` |
| Exact command | `<command>` |

## Measured interval

| Measurement | Result |
|---|---:|
| Criterion 95% interval | `<milliseconds>` |
| RSS / allocator observation | `<repository benchmark output>` |
| Profiler duration and sample rate | `<value>` |

## Function-level attribution

| Rank | Function or stack | Inclusive % | Exclusive % | Evidence-based interpretation |
|---:|---|---:|---:|---|
| 1 | `<symbol>` | `<%>` | `<%>` | `<fact, not proposal>` |
| 2 | `<symbol>` | `<%>` | `<%>` | `<fact, not proposal>` |
| 3 | `<symbol>` | `<%>` | `<%>` | `<fact, not proposal>` |

## Interpretation

State whether the evidence isolates file read, UTF-8 conversion, line scanning, parser work, deterministic collection/sort work, or snapshot construction. Identify uncertainties, including sampling bias, system-library attribution, cache state, and filesystem state.

## Change decision

| Candidate | Evidence threshold | Decision |
|---|---|---|
| Bounded mapped evaluation | File-read cost must be material and the mapping lifetime must remain root-confined | `<accept / reject / defer>` |
| Parallel chunk parsing | Profile must show parallelizable work and a deterministic grammar-boundary design must exist | `<accept / reject / defer>` |
| Content-addressed snapshot cache | A repeated identical request must dominate the target workflow; cache key and invalidation tests must be specified | `<accept / reject / defer>` |

## Safety and provenance

Record only redacted host facts. Do not include user paths, hostnames, profile traces, raw USDA content, environment variables, credentials, or memory addresses. Preserve the fixture license and source hash; do not treat profile data as conversion evidence.
