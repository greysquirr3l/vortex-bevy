# Build and verify

This page covers the everyday build and verification recipes. Every
recipe matches a step in `AGENTS.md`'s preflight contract.

## Build

Build every workspace target with every feature flag enabled:

```bash
cargo build --workspace --all-targets --all-features
```

For an optimized release build (slower compile, faster runtime):

```bash
cargo build --workspace --all-targets --all-features --release
```

## Run the tests

The workspace has unit, integration, and benchmark test suites. The
integration suites include both headless self-authored fixtures and
opt-in real-Editor evidence tests gated behind environment variables.

```bash
cargo test --workspace --all-features
```

The opt-in real-Editor tests are skipped unless you set
`VORTEX_REAL_UNREAL_REPORT`, `VORTEX_REAL_UNITY_*`, or
`VORTEX_REAL_GODOT_*` environment variables. See
[Inspect an Unreal asset](../how-to/inspect-unreal.md) and
[Convert an authorized source](../how-to/convert.md) for the variable
names.

## Format

```bash
cargo fmt --all -- --check
```

Apply formatting in place with:

```bash
cargo fmt --all
```

## Strict clippy

The project runs the full clippy profile (all, pedantic, nursery, cargo,
perf) and denies the standard panics:

```bash
cargo l
```

`cargo l` is an alias defined in `.cargo/config.toml` that mirrors the exact
flag set used in CI. Treat any lint failure as a hard error.

## Supply-chain checks

```bash
cargo deny --all-features check
cargo audit
```

`cargo-deny` checks license compliance, advisory database, and source
policy. `cargo-audit` checks the RustSec advisory database directly.
Both must exit zero before a release.

## Rustdoc

```bash
cargo doc --workspace --no-deps
```

This builds the API documentation. Doctests are run separately via
`cargo test --doc`.

## Preflight (one command)

If you have [`just`](https://github.com/casey/just) installed, the full
quality gate is one recipe:

```bash
just preflight
```

This runs build, test, fmt-check, clippy, deny, and audit in that order.
If any step fails, the recipe stops and prints the failing step.
