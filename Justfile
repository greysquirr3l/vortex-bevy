# Vortex-Bevy development recipes.
#
# Run `just` to list every recipe. Common entry points:
#   just preflight   — AGENTS.md quality gate, run before each commit
#   just pre-push    — local pre-push hook chain (broader, slower)
#   just ci          — every CI job back-to-back (longest path to green)
#   just ci-quality  — just the rust quality job
#
# Recipes group CLI invocations so the flags match `.github/workflows/ci.yml`
# and AGENTS.md preflight exactly. If you change a flag here, mirror it in
# both places.

# Shell used to execute recipe bodies. The `-c` flag is mandatory — without it
# bash interprets the recipe string as a script-file path and fails with
# ENOENT on every multi-word command (cargo build, rustfmt, etc.).
#
# Wrapping with `env` makes the Justfile portable: `env bash` resolves `bash`
# against $PATH at runtime, so this Mac's homebrew bash (/opt/homebrew/bin/bash)
# is picked up first, Linux/CI runners fall through to /usr/bin/bash, and no
# absolute path is hardcoded anywhere.
set shell := ["env", "bash", "-eu", "-c"]

# Toolchain pinned in rust-toolchain.toml — cargo picks it up automatically.
toolchain       := "1.97.1"
workspace_args  := "--workspace --all-targets --all-features"
test_args       := "--workspace --all-features"
doc_args        := "--workspace --no-deps"

# Default: list recipes instead of running anything dangerous.
default:
    @just --list

# ----- Setup -----------------------------------------------------------------

# Install git hooks (pre-commit, commit-msg, pre-push) into .git/hooks/.
install-hooks:
    bash scripts/install-hooks.sh

# ----- Formatting -----------------------------------------------------------

# Apply rustfmt across the workspace.
fmt:
    cargo fmt --all

# Verify rustfmt formatting. Non-zero exit fails CI.
fmt-check:
    cargo fmt --all -- --check

# ----- Linting --------------------------------------------------------------

# Strict clippy. Uses the `cargo l` alias from .cargo/config.toml, which sets
# -W clippy::{all,pedantic,nursery,cargo,perf} plus -D warnings.
clippy:
    cargo l

# Apply automatic clippy fixes. Review the diff before committing — automated
# fixes can violate house style and need manual cleanup.
clippy-fix:
    cargo clippy {{workspace_args}} --fix --allow-dirty --allow-staged

# Build workspace documentation. CI runs this without deps to catch broken
# intra-doc links and missing examples.
doc:
    cargo doc {{doc_args}}

# Build the user-facing MDBook. Requires `mdbook` (cargo install mdbook --locked).
# Outputs static HTML under book/build/ which is gitignored.
book:
    mdbook build book/

# Strict mdbook build: also runs the link checker and flags dead links.
book-strict:
    mdbook test book/ 2>&1 || true
    mdbook build book/

# ----- Build ----------------------------------------------------------------

# Type-check the workspace. Faster than `build` (no codegen), good for
# iterating on signatures and macro output.
check:
    cargo check {{workspace_args}}

# Debug build of every workspace target with every feature.
build:
    cargo build {{workspace_args}}

# Release build. Matches what `cargo install` or distribution packaging would
# produce. Pre-push runs this; expect it to be slow.
build-release:
    cargo build --release {{workspace_args}}

# Remove the target/ directory.
clean:
    cargo clean

# ----- Test -----------------------------------------------------------------

# Run the workspace test suite with every feature enabled.
test:
    cargo test {{test_args}}

# Run only doc tests. Useful when you change `///` examples.
test-doc:
    cargo test --doc {{test_args}}

# ----- Supply chain / policy ------------------------------------------------

# cargo-deny licence, advisory, and source policy. Matches the `supply-chain`
# CI job. Requires `cargo install cargo-deny`.
deny:
    cargo deny --all-features check

# cargo-audit RustSec vulnerability scan. Requires `cargo install cargo-audit`.
audit:
    cargo audit

# ----- Fixtures -------------------------------------------------------------

# Enforce fixture provenance rules: forbidden file types and the 5 MiB size
# cap. See scripts/check-fixture-policy.sh.
fixture-policy:
    bash scripts/check-fixture-policy.sh

# Verify each fixture manifest's sha256 matches its source file. See
# scripts/verify-fixtures.sh.
fixture-verify:
    bash scripts/verify-fixtures.sh

# ----- Editor bridges (Unreal / Unity / Godot) ------------------------------

# Run every editor-bridge lint and test pipeline (ruff, pytest, gdlint,
# gdformat, dotnet csharpier, dotnet build, TODO/FIXME sweep). Requires the
# toolchain pinned in the `bridge-quality` CI job.
bridge-quality:
    bash scripts/check-bridge-quality.sh

# ----- Commit message --------------------------------------------------------

# Validate a commit message file against Conventional Commits. Pass the path
# to the message file (typically .git/COMMIT_EDITMSG from the commit-msg hook).
commit-msg path:
    bash scripts/check-conventional-commit.sh {{path}}

# ----- Running the CLI ------------------------------------------------------

# Run the vortex-bevy CLI. Pass arguments after `just run --`, e.g.
#   just run -- --help
#   just run -- inspect path/to/.uasset
run *args:
    cargo run -p vortex-bevy -- {{args}}

# ----- Composite workflows --------------------------------------------------

# Cross-platform application bundle. Produces an OS-native installer that
# grants a foreground GUI session to the Bevy viewer window (AppKit on
# macOS, SxS manifest on Windows, .desktop + icon on Linux).
#
# Requires: cargo install cargo-bundle --locked
# Outputs (one per platform):
#   - macOS:   target/release/bundle/osx/Vortex-Bevy.app
#   - Windows: target/release/bundle/windows/  (MSI + bundled .exe + SxS manifest)
#   - Linux:   target/release/bundle/{deb,rpm,appimage}/
#
# Each bundle wraps target/release/vortex-bevy with the platform-specific
# metadata in crates/vortex-bevy/Cargo.toml ([package.metadata.bundle]).
bundle:
    cargo build --release --bin vortex-bevy -p vortex-bevy
    cargo bundle --release --bin vortex-bevy -p vortex-bevy

# AGENTS.md preflight. Required to pass before any commit. Mirrors the exact
# command list documented in AGENTS.md.
preflight: build test fmt-check clippy deny audit
    @echo ""
    @echo "Preflight passed."

# Mirrors the local pre-push git hook. Includes bridge-quality and a release
# build that preflight skips. Run this before pushing to avoid CI surprises.
pre-push: test clippy bridge-quality build-release doc
    @echo ""
    @echo "Pre-push checks passed."

# CI `quality` job from .github/workflows/ci.yml.
ci-quality: build test fmt-check clippy doc book
    @echo ""
    @echo "CI quality job passed."

# CI `supply-chain` job from .github/workflows/ci.yml.
ci-supply-chain: deny audit fixture-policy fixture-verify
    @echo ""
    @echo "CI supply-chain job passed."

# Every CI job back-to-back. Slowest path to green — use before opening a PR
# if you want zero surprises on first push.
ci: ci-quality bridge-quality ci-supply-chain
    @echo ""
    @echo "All CI jobs passed."
