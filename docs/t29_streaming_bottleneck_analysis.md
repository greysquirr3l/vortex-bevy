# T29 Streaming Pipeline and USD Bottleneck Analysis

## Measured baseline

The T28 Criterion measurement resolves the 3,354,071-byte StandardShaderBall USDA
layer in a 95% interval of **22.391–22.636 ms**. The self-authored minimal USDA
triangle resolves in **40.115–42.193 µs**. Converting both intervals to microseconds
shows that the production-shaped stage is **530.68–564.27×** slower, with a midpoint
ratio of **547.10×**. These are host-specific observations, not service-level
performance guarantees. [1]

| Workload | Input | 95% interval | Relevant output boundary |
|---|---:|---:|---|
| Minimal selected USDA | Small, self-authored triangle | 40.115–42.193 µs | Typed stage facts and prim snapshots |
| StandardShaderBall USDA | 3,354,071-byte, CC BY 4.0 layer | 22.391–22.636 ms | Typed stage facts, more than 100 prims, and `requires_adapter` |

The production fixture remains intentionally outside supported live composition and
GLB conversion. Its benchmark measures bounded initial stage inspection only. [1]

## Bottleneck attribution

The measured path constructs its request before the benchmark iteration. Every
iteration of `UsdStageInspector::resolve` then canonicalizes the selected file,
reads the full layer, copies it into a UTF-8 `String`, walks every line, parses
stage/prim/composition facts, inserts many owned strings into ordered collections,
and sorts/deduplicates final facts and prims. The StandardShaderBall source also
contains large mesh-array lines, nested shader networks, many prim declarations,
and variant-heavy structure. None of its referenced layers, MaterialX/EXR assets,
payloads, or mesh data are evaluated by the current adapter. [1] [2]

The one-time production observation allocated 3,480,902 bytes across 2,473
allocations, while the resolved snapshot was live. Its live RSS increase was
458,752 bytes; the observed high-water mark was 17,256,448 bytes. These values
support treating full-file input and owned snapshot construction as the principal
costs. They do not isolate filesystem I/O, allocation, UTF-8 conversion, or
individual parser operations, and they must not be interpreted as per-stage memory
ceilings. [1]

> An asynchronous queue cannot make one synchronous full-file parser intrinsically
> faster. Its role is to keep callers responsive and make multiple optional
> decompression/inspection requests bounded, observable, deterministic at the
> per-request boundary, and failure-safe.

## T29 implementation requirements

The T29 pipeline is deliberately an **in-process worker queue**, not a persistent
service, scheduler, downloader, or arbitrary archive extractor. It will preserve
existing root confinement, provenance, stage-selection, classification, and
resource-admission rules.

| Requirement | T29 decision |
|---|---|
| Input formats | Existing bounded `.usda` passthrough plus explicitly selected local `.usda.gz` input only. `.usd`, `.usdc`, archives, network paths, symlinks escaping the authorized root, and unrecognized compression remain rejected or retain their existing adapter boundary. |
| Queue bounds | A non-zero queue depth, worker count, and aggregate caller-supplied reservation budget are required. A full queue rejects immediately; it never creates an unbounded backlog. |
| Decompression bounds | Both compressed input and produced UTF-8 USDA bytes have explicit limits. Decompression stops and fails before the configured output limit is exceeded, preventing expansion bombs. |
| Worker lifecycle | The caller creates, owns, and explicitly shuts down workers. Each job returns exactly one typed result. A disconnected receiver discards its result and releases its permit; no result is published to conversion state by the queue itself. |
| Ordering | Submission order is assigned a monotonic local identifier. Completion order is intentionally not promised. Each completed stage remains deterministic for the same input, selection, and adapter version. |
| Admission | A queued job reserves bounded estimated bytes through the existing resource governor before it can reach file read, decompression, or parsing. Rejection happens before those operations. |
| Security | No unsafe code, shell command, network access, archive traversal, raw source telemetry, or user-controlled output path is introduced. Error records expose typed limits and opaque identifiers only. |
| Scope | The queue is optional infrastructure around selected local USDA inspection. It does not add live OpenUSD composition, arbitrary payload/reference traversal, pixel decoding, GPU upload, or authored runtime variant mutation. |

## Benchmark comparison plan

T29 will measure the existing direct StandardShaderBall path and the queue's local
USDA passthrough path separately. The queue comparison will report submission-to-
result latency, not claim that a worker queue accelerates the parser. A bounded
owned gzip fixture will exercise decompression, output-size rejection, malformed
stream rejection, full-queue rejection, cancellation/disconnect cleanup, and
preservation of the original typed stage result.

## References

[1]: performance_baseline.md "T28 measured timing, allocation, and RSS observations"
[2]: ../fixtures/production_scale/standard_shader_ball/shaderball.fixture.json "Pinned StandardShaderBall fixture provenance and supported boundary"

## T29 Queue Measurement

A standalone release-mode Criterion run measured the retained StandardShaderBall layer with the same one-worker, 4 MiB, depth-one queue configuration used by the streaming benchmark. The all-target test run also executes benchmark harnesses under a debug test profile, so its timings are intentionally excluded from this comparison.

| Operation | 95% interval | Midpoint | Interpretation |
|---|---:|---:|---|
| Direct selected USDA resolution | 18.741–18.979 ms | 18.8600 ms | Baseline: file metadata/read, UTF-8 conversion, line-oriented selected-stage parsing, deterministic sorting, and snapshot construction. |
| One-worker queued plain USDA resolution | 18.971–19.182 ms | 19.0765 ms | Adds 0.2165 ms, or approximately 1.00%, to caller-visible completion latency. This is scheduling and one-shot response overhead, not a parsing-speed improvement. |
| One-worker queued gzip USDA resolution | 27.630–28.685 ms | 28.1575 ms | Adds 9.0810 ms, or approximately 47.00%, over queued plain input. The difference is bounded gzip decoding and temporary decompressed text allocation. |

The queue therefore is appropriate for isolating selected blocking file/decompression work from a caller thread, but it does not make an individual stage resolve faster. Parallel throughput should be evaluated only with a representative authorized workload and a separately approved worker-count/rate-limit configuration. The current default keeps one worker and one queued request, so worst-case reserved decompression capacity is 4 MiB and rejected input cannot expand that budget. [1] [2]

[1]: https://doc.rust-lang.org/std/sync/mpsc/fn.sync_channel.html "Rust standard library synchronous channel"
[2]: https://docs.rs/flate2/1.1.0/flate2/read/struct.GzDecoder.html "flate2 GzDecoder API"
