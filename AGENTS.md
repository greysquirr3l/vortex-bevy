# AGENTS.md

## Project

vortex-bevy — Vortex-Bevy is a Rust 1.97.1 / Bevy 0.19 asset-conversion and runtime-viewing
workspace for authorized Unreal Engine source projects and OpenUSD stages. It
inspects Unreal package metadata, invokes a minimal Unreal Editor automation
bridge for authoritative export, resolves configured USD stage views, compiles
a typed intermediate representation, packages Bevy-compatible GLB and textures,
and loads the result with source-traceable metadata in Bevy.

The product explicitly distinguishes editable source-project assets from loose,
cooked, encrypted, or proprietary payloads. It never promises functional
Blueprint/C++ conversion. It emits an actionable per-asset classification:
converted, baked, requires_adapter, unsupported, or rejected.


## Setup commands

- Build: `cargo build --workspace --all-targets --all-features`
- Test: `cargo test --workspace --all-features`
- Lint: `cargo fmt --all -- --check && cargo l`

## Code style

- Language: rust
- Strategy: Complete — ship root fix end-to-end with tests and docs in the same task


## Rules

- Pin Rust exactly to 1.97.1 in rust-toolchain.toml and use edition 2024; stable only, no nightly features.

- Use a Cargo workspace with the minimum practical crates: crates/vortex-core as the reusable library and crates/vortex-bevy as the CLI plus Bevy runtime application. Do not add a crate without a concrete dependency or build-boundary reason.

- Use collapsed DDD/Hex layout inside each cohesive crate: src/domain, src/application/{command,query}, and src/adapters/{primary,secondary}. Re-export focused public APIs from module roots; do not create a separate ports directory or shared ports crate.

- Port traits are consumer-owned: define each narrow trait in the command/query handler that calls it, with one to three methods. Concrete adapters implement the handler-local trait and never re-export it.

- The domain is pure: no Bevy, filesystem, subprocess, Tokio, OpenUSD, serde format, or Unreal Editor imports in domain modules. Domain logic receives time, platform facts, and conversion context explicitly.

- Parse untrusted CLI, filesystem, project, USD, package, and manifest inputs exactly once at primary adapter boundaries into typed value objects. Do not pass raw String values into domain or application handlers.

- Use thiserror for library/domain/application errors and anyhow with context only in binary entrypoints and primary adapters. Do not flatten domain errors into strings before an adapter boundary.

- Never use .unwrap(), .expect(), panic!, todo!(), unimplemented!(), unsafe code, or indexing/slicing that can panic in production code. Use ?, explicit matches, get, checked arithmetic, and typed errors. Clap’s own parse exit behavior is the sole accepted CLI exception.

- Run the full strict Clippy profile. Do not add #[allow(clippy::...)] suppressions. Use #[expect(...)] only when an unavoidable, documented lint is expected and retain a reason.

- Prefer .is_multiple_of(n) to modulo equality checks, LazyLock/LazyCell to lazy_static!, checked conversions over as casts, Arc::clone for genuine shared ownership, and owned data over contagious lifetime parameters. Clone small immutable values when that is clearer than lifetime gymnastics.

- Avoid async traits unless an actual asynchronous boundary requires one. If a port must be dynamically dispatched and async, use an explicit object-safe boxed future or a concrete adapter; do not add async-trait by reflex.

- Treat every conversion as deterministic and idempotent. Derive stable output paths and IDs from typed source identities and export profile. Do not depend on wall-clock time, unordered HashMap iteration, or process-global mutable state in generated artifacts.

- Use GLB plus a versioned scene.manifest.ron sidecar as the first runtime contract. Preserve source package path, Unreal object path, USD prim path, selected variant state, warnings, and conversion class; never silently drop unsupported source features.

- Editable .uproject input uses the matching Unreal Editor as the authoritative source decoder. The only permitted Python is a minimal, isolated Unreal Editor automation script because AssetExportTask is the official bridge; Rust remains the product core. Do not introduce Python elsewhere.

- Cooked, encrypted, unowned, or DRM-protected inputs are out of scope by default. Detect and report the condition; do not bypass encryption, keys, licensing, or access controls.

- Bevy 0.19 code uses the current APIs: WorldAssetRoot and GltfAssetLabel for GLB scene spawning; FontSize::Px and FontSource where text is present; resources-as-components semantics; and Bevy 0.19-compatible crate versions only.

- Validate every GLB against Bevy’s supported glTF subset. Reject or transform unsupported compression, instancing, variants, shader extensions, and unsupported texture-transform usage with explicit manifest diagnostics.

- Tests must use owned or synthetic fixtures only. Never commit licensed Unreal Marketplace assets, game assets, encryption keys, proprietary .uasset payloads, or generated binaries without explicit authorization.

- Use conventional commits, GitHub Actions CI, Coraline for code navigation, and direct safe edit operations. Never force-add ignored files and never use truncation-prone shell rewrite patterns.

- Before removing a cargo-deny ignore, run cargo deny --all-features check with a fresh network advisory database and confirm the advisory is absent from the resolved graph.

- Optimize only after measuring. Use concrete types in hot conversion/Bevy paths; do not introduce trait objects or Arc in tight per-vertex loops without evidence.

## Testing instructions

- Run `cargo test --workspace --all-features` before committing
- Every new public function needs at least one test
- Fix all test failures before marking a task complete

## Claude hooks

A `.claude/settings.json` is generated alongside these files. It registers a
`PreCompact` hook that blocks context compression while any task is in the `[~]`
(in-progress) state, preventing the loss of mid-task context.

- Do **not** delete or edit `.claude/settings.json` manually
- The hook exit-code controls: `0` = safe to compact, `1` = blocked
- If you need to override, close the `[~]` task first (mark it `[!]` or `[x]`)

## Commit conventions

- Use conventional commits: `feat:`, `fix:`, `refactor:`, `test:`, `docs:`
- Focus commit messages on user impact, not file counts or line numbers

---

_Generated by [wiggum](https://github.com/greysquirr3l/wiggum)._
