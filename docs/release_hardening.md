# Release Hardening Guide

**Status:** Release-candidate evidence for `vortex-bevy` 0.1.0.

This guide defines the reproducible release evidence for Vortex. It covers the repository-owned Rust workspace, deterministic fixtures, authorized Editor bridges, and the bounded in-process streaming USDA queue. It does not authorize the collection, retention, or publication of user projects, editor caches, reports, credentials, raw `.uasset` files, or arbitrary third-party inputs.

> A successful release gate establishes only the support claims documented in the compatibility matrix. In particular, direct selected-USDA inspection and queued gzip decompression do not claim general OpenUSD composition, arbitrary binary USD decoding, pixel decoding, or an Editor-free substitute for an authorized engine bridge.

## Release Inputs and Reproducibility

A release must originate from a clean commit on `main`, use the pinned Rust 1.97.1 toolchain, and retain the committed `Cargo.lock`. The normal build does not require a network connection after dependencies are available. The canonical local release command is:

```bash
PATH="$HOME/.rustup/toolchains/1.97.1-x86_64-unknown-linux-gnu/bin:$PATH" \
CARGO_BUILD_JOBS=1 \
bash scripts/check-release-readiness.sh
```

| Evidence | Required result | Reason |
|---|---|---|
| Working tree | `git status --short` is empty | Prevents accidental generated output or unreviewed inputs from entering the release. |
| Rust toolchain | Rust 1.97.1 | Matches the repository’s supported compiler and CI configuration. |
| Locked dependencies | `Cargo.lock` is unchanged and policy checks pass | Produces a reviewable dependency graph. |
| Tests and docs | Workspace tests, strict lint, bridges, fixtures, and documentation contracts pass | Preserves behavioral, provenance, and operator-facing contracts. |
| Release compile | All targets and features compile in release mode | Checks a production-equivalent build configuration. |

## Streaming Pipeline Recovery and Rollback

The selected-USDA queue has no persistent worker process, network ingress, or durable job state. A caller owns the queue, supplies each explicit local root and `SourceAssetPath`, and invokes `shutdown` to close the sender and join workers. If a worker rejects a request, its typed error is returned through that request’s one-shot receiver; no partial output, manifest, or source copy is created.

| Condition | Safe response |
|---|---|
| `QueueFull` | Apply caller-side backpressure and retry only after an existing job completes. Do not create an unbounded retry loop. |
| `WorkerUnavailable` or `WorkerShutdownFailed` | Discard the queue instance and construct a new one after recording only redacted operational context. |
| `ByteLimitExceeded` | Retain the configured limit; select a smaller authorized source or use the established adapter workflow. Do not raise limits based solely on untrusted input. |
| `Decompression` or `InvalidUtf8` | Treat the input as rejected. Do not fall back to a different parser or write decompressed bytes. |
| Source/root validation failure | Re-select an authorized root and path. Do not broaden the root boundary. |

The queue accepts only explicitly selected local `.usda` or `.usda.gz` paths. Its worker count must not exceed the existing conversion-governor concurrency limit. It uses a bounded synchronous channel, non-blocking submit, one worst-case reservation per active worker, a 4 MiB configured maximum decompressed stream size, and a typed rejection before parsing when either plain or decompressed input exceeds that bound. These rules keep streaming work inside the same admission-control model as synchronous conversion execution. [1] [2]

## Supply-Chain and Security Evidence

All CI actions are pinned to immutable reviewed commits, with the source version retained as an inline comment. The CI workflows expose least-privilege permissions: ordinary quality and supply-chain jobs have read-only contents access, the secret scan receives its scoped token, and CodeQL receives security-event write permission only. The scheduled CodeQL job remains the repository’s periodic analysis mechanism.

The release review includes `cargo deny --all-features check`, an advisory scan using `cargo audit --no-fetch` against the available RustSec database, fixture provenance and checksum checks, a source-level secret scan in CI, and `cargo geiger` audit evidence for both concrete workspace packages. The `cargo-geiger` 0.13.0 scan completed with parser warnings for upstream dependency test-only source files and reported unsafe-code use in the large third-party dependency graph. That result is an **audit finding**, not a clean-bill-of-health: upstream unsafe usage must continue to be reviewed when the dependency graph changes. Repository-owned production crates use `#![forbid(unsafe_code)]` and contain no `unsafe` operations; matches of the word “unsafe” are diagnostics and documentation only.

| Surface | Release position |
|---|---|
| Repository-owned Rust | `unsafe_code` is forbidden in core and runtime crates; release checks compile this policy. |
| Streaming dependency | `flate2` is the explicit local gzip decoder. It is restricted to the application streaming boundary and does not enter the domain layer. |
| Transitive unsafe code | Present in third-party dependencies according to the cargo-geiger audit; documented and re-audited on dependency changes. |
| Secrets | CI runs Gitleaks across full Git history. Release review must stop if it reports a real secret; rotate the credential and rewrite/remediate history under an approved incident process. |
| External fixtures | Only license- and checksum-tracked fixtures under `fixtures/` may be committed. User data and arbitrary downloads are prohibited. |

## Release Notes for 0.1.0 Candidate

The candidate provides deterministic canonical-scene and GLB workflows, bounded source adapters, atomic artifact publication, Bevy runtime loading, lifecycle inspection, controlled Unity/Unreal/Godot evidence boundaries, and typed support-matrix output. T25 adds prepackaged bounded USD variants, instance batches, and chunks. T26 adds narrowly scoped UE 5.8.2 `UTexture2D` metadata inspection while retaining the Editor bridge as authoritative. T27 and T28 add owned and provenance-tracked production-fixture benchmarks, redacted tracing, resource admission, and Linux RSS observations.

T29 adds a caller-owned in-process pipeline for selected local USDA stages. It supports explicit plain USDA and gzip-wrapped USDA input, bounded worker concurrency and backlog, non-blocking queue admission, decompression output limits, deterministic request-to-response correlation, and lifecycle shutdown. It does not stream from a network, recurse through references, traverse arbitrary payloads, retain decompressed content, write conversion artifacts, or bypass engine adapters.

Before publishing a tag, maintainers must confirm that every user-facing support claim matches `docs/compatibility_matrix.json`, that `docs/t29_streaming_bottleneck_analysis.md` is current for the selected production fixture, and that the current release has passed the canonical release script and CI. A failed pre-push, audit, fixture, provenance, or documentation gate blocks release publication.

## References

[1]: https://doc.rust-lang.org/std/sync/mpsc/fn.sync_channel.html "Rust standard library synchronous channel"
[2]: https://docs.rs/flate2/1.1.0/flate2/read/struct.GzDecoder.html "flate2 GzDecoder API"
[3]: https://docs.github.com/en/actions/security-for-github-actions/security-guides/security-hardening-your-deployments "GitHub Actions security hardening"
[4]: https://github.com/rustsec/rustsec/tree/main/cargo-audit "RustSec cargo-audit"
[5]: https://github.com/geiger-rs/cargo-geiger "cargo-geiger"
