# Install

Vortex-Bevy is a Rust workspace. You need a recent stable Rust toolchain
plus a native C toolchain to link Bevy and its windowing dependencies.

## Prerequisites

- **Rust toolchain.** The project pins Rust 1.97.1 in
  `rust-toolchain.toml`. The standard `rustup` installation handles this
  automatically — clone the repository and `cargo` will pick the right
  version.
- **A native C compiler and linker.** On macOS, install Xcode Command Line
  Tools (`xcode-select --install`). On Debian/Ubuntu, install
  `build-essential`. On Windows, install Visual Studio Build Tools with the
  "Desktop development with C++" workload.
- **Optional: `cargo-deny` and `cargo-audit`** for supply-chain checks
  (used by the quality recipes).
- **Optional: an authorized editor installation** if you intend to run
  Editor-bridge evidence tests. These are off by default; nothing in the
  install path pulls them in.

## Clone and build

```bash
git clone https://github.com/greysquirr3l/vortex-bevy
cd vortex-bevy
cargo build --workspace --all-targets --all-features
```

The first build pulls the locked dependency graph (Bevy 0.20, glTF, etc.)
and compiles both crates. Expect a few minutes on a clean machine.

## Verify the toolchain

```bash
cargo --version
rustc --version
```

You should see Rust 1.97.1 reported by `rustc`. If `rustc` reports a
different version, check that `rust-toolchain.toml` is present and that you
do not have a global `rust-toolchain` override in the parent directory.

## Optional: install the development recipes

The repository ships a `Justfile` with named recipes matching the
`AGENTS.md` preflight contract. To use them:

```bash
# Requires just: cargo install just
just --list
just preflight
```

`just preflight` runs the full quality gate (build, test, format, clippy,
deny, audit) in the order documented in `AGENTS.md`.

## What's installed

After `cargo build`, the CLI lives at
`target/debug/vortex-bevy` (or `target/release/vortex-bevy` after
`cargo build --release`). All examples in this book use the development
binary path. To install it on `PATH`:

```bash
cargo install --path crates/vortex-bevy --locked
```

The `--locked` flag pins the dependency graph to the versions the project
was tested against.
