# Introduction

Vortex-Bevy converts **authorized** Unreal Engine source-project assets and
selected OpenUSD stages into deterministic Bevy 0.20 runtime artifacts. The
output is a Bevy-compatible GLB plus a versioned `scene.manifest.ron` sidecar
that records source identity, selected configuration, conversion
classification, and diagnostics.

If you are evaluating the project for the first time, the
[Getting started](./getting-started/install.md) section walks you through
installation and your first conversion in under ten minutes. The
[Tutorial](./tutorial/usd-end-to-end.md) goes deeper on a single end-to-end
USD conversion. The [How-to](./how-to/inspect-unreal.md) section covers
specific tasks like inspecting an asset or opening the Bevy viewer.

The latest published version of this guide is rendered at
<https://greysquirr3l.github.io/vortex-bevy/> by the
[Deploy book](../../.github/workflows/book.yml) workflow on every push
to `main` that touches `book/`.

For contract-level detail — manifest schema fields, the full CLI flag
matrix, supported capabilities — see the
[Reference](./reference/cli.md) section.

## What Vortex-Bevy is and is not

Vortex-Bevy is a deterministic asset-conversion and runtime-viewing
workspace for authorized source content. It:

- parses and packages selected OpenUSD stage facts,
- packages authorized Unreal controlled static-scene exports produced by
  the matching Unreal Editor,
- packages authorized Unity URP static-scene exports produced by the
  matching Unity Editor,
- loads and renders the resulting GLBs in a Bevy 0.20 desktop viewer.

It is **not** a general-purpose `.uasset` / `.unity` / `.tscn` /
`.usd` decoder. It does not bypass encryption, guess keys, or claim
Blueprint/C++ equivalence. Inputs that fall outside the documented support
boundary are detected and reported with a typed classification rather than
silently approximated.

## Support boundary

Editable Unreal source projects use the matching Unreal Editor as the
authoritative semantic/export decoder. Direct `.uasset` support begins as
metadata inspection only. New direct decoders must be approved per asset
class, engine version, fixture provenance, and acceptance criteria.

Cooked, encrypted, unowned, DRM-protected, or otherwise unauthorized
inputs are rejected by default. The tool refuses to bypass access controls.

## Security scope

Vortex-Bevy is an offline CLI plus an optional Bevy 0.20 desktop viewer. It
does not expose an HTTP server, connect to a database, or fetch URLs on the
user's behalf. The attack surface is local-file ingestion (with per-format
size caps and typed deserialization), local subprocess invocation (the
authorized Unreal/Unity/Godot Editor through typed `Command::new(...).args(...)`
adapters — no shell, no string concatenation), and the Bevy desktop window.

Cryptography is limited to SHA-256 content hashing via the `sha2` crate;
there is no encryption, no RNG, and no network code path. Path traversal
is prevented by systematic `canonicalize + starts_with(root)` confinement
in every adapter that reads from disk.

The full audit is at [`docs/security_audit_t30.md`](../docs/security_audit_t30.md).

## How to read this book

If you are new, start with [Install](./getting-started/install.md) and the
[Tutorial](./tutorial/usd-end-to-end.md). Use the [How-to](./how-to/inspect-unreal.md)
section for recipes. The [Reference](./reference/cli.md) section is for
looking up a specific flag or schema field. The
[Explanation](./explanation/architecture.md) section is for understanding
_why_ the system is shaped the way it is.

If something does not match what you see in your copy of the tool, the
CLI emits `vortex-bevy: <version>` on `--version` — please file an issue
with that version string.
