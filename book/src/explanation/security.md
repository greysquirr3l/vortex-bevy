# Security model

Vortex-Bevy is an **offline CLI plus an optional Bevy 0.19 desktop
viewer**. It does not expose an HTTP server, connect to a database, or
fetch URLs on the user's behalf. The attack surface is local-file
ingestion, local subprocess invocation, and the Bevy desktop window.

This page summarizes the security model. The full audit is at
[`docs/security_audit_t30.md`](../../docs/security_audit_t30.md).

## Threat model

The threat model is deliberately narrow:

- The user runs the CLI on their own machine.
- The user supplies an explicitly authorized editable project root,
  asset path, output root, and (where applicable) Unreal object path.
- The user invokes the matching authorized editor for semantic export
  through one of the shipped bridge scripts.

The threat model does **not** include:

- Untrusted network input (no HTTP server, no URL fetching).
- Untrusted subprocess input (every subprocess call uses
  `Command::new(program).args(...)` — no shell, no string
  concatenation).
- Untrusted user accounts (the CLI runs as the user invoking it).
- Untrusted Editor installations (the bridges require the matching
  authorized Editor version).

## Asset-surface enumeration

The attack surface, in priority order:

1. **Local-file ingestion.** `.uasset`, `.usda`/`.usdc`/`.usd`,
   `.unity`, `.tscn`, manifest RON. Every adapter enforces:
   - Root canonicalization and confinement
     (`canonicalize + starts_with(root)`).
   - Symlink escape rejection.
   - Per-format size caps (4 MiB for USDA, 32 MiB for `.uasset`).
   - Validated typed source identities
     (`SourceAssetPath`, `UnrealObjectPath`, `UsdPrimPath`,
     `UnityAssetGuid`, etc.) before any filesystem work.
2. **Local subprocess invocation.** The matching authorized
   Unreal/Unity/Godot Editor through typed adapters that use
   `Command::new(program).args(...)`. No shell. No string
   concatenation. The bridge script path is canonicalized and
   validated before invocation.
3. **Bevy desktop window.** Native window via `bevy_winit` +
   `bevy_egui` overlay. The UI only renders typed state and queues
   explicit lifecycle commands; it never embeds conversion handlers
   or source-format decoders.

The CLI does not:

- accept credentials as arguments,
- write to arbitrary filesystem locations,
- execute downloaded code,
- modify the user's editor installation.

## Path traversal

The 22 source files that read from disk use a consistent pattern:

```rust,ignore
let root = canonicalize(root)?;
let candidate = canonicalize(candidate)?;
if !candidate.starts_with(&root) {
    return Err(PathEscape { ... });
}
```

The Python bridge mirrors this pattern with `_normalized(path).relative_to(_normalized(root))`.
The manifest parser enforces that artifact paths are non-empty,
relative, contain no traversal segments, and contain no NUL bytes.

A failure at any step surfaces as a typed error (`PathEscape`,
`UnsafeRelativePath`, etc.) before any adapter runs.

## Cryptography

Vortex-Bevy uses only SHA-256 content hashing via the `sha2` crate.
There is no encryption, no RNG, no networking. SHA-256 is used for:

- `ArtifactHash::from_bytes` — content addressable artifact
  identification.
- `ConversionId::derive` — conversion IDs are derived from
  `SHA-256("vortex-conversion-v1" || source || profile_hash)`.
- Profile hashes derived from profile names.

The dependency graph deliberately avoids MD5, SHA-1, DES, AES, and
ECB-mode ciphers. The project carries no RNG (`rand`/`getrandom` are
not in the dependency graph) because all outputs are deterministic
from typed inputs.

## Subprocess injection

The only production subprocess invocation is
`crates/vortex-core/src/adapters/secondary/unreal_bridge.rs:218`:

```rust,ignore
Command::new(command.program()).args(command.arguments()).output()?
```

`command.program()` is the configured matching Unreal Editor
executable path, not a shell string. `command.arguments()` is
`Vec<OsString>` produced by `UnrealBridge::build_command`, which
composes fixed `-unattended` / `-nop4` /
`-ExecutePythonScript=...` flags plus the report path and an explicit
asset-path list. No string concatenation, no `format!` into the
command line, no shell expansion.

The bridge script path is resolved by a fixed relative path from a
canonicalized repository root
(`BRIDGE_SCRIPT_COMPONENTS = ["bridges", "unreal",
"vortex_unreal_bridge.py"]`), then validated with
`fs::canonicalize(&script_path)` plus
`if !script_path.starts_with(&repository_root) || !script_path.is_file()`.

## Logging

`tracing` events are `ConversionId`-correlated and carry no PII or
credentials. The fields are typed `ConversionId`, `SourceEngine`,
`SubmissionId`, `conversion_id`, and success booleans. No project
roots, asset paths, object paths, or report bodies are emitted in
trace events.

T27 RSS and allocation observations contain process-wide counters
only: they do not capture asset paths, project roots, environment
variables, text content, credentials, or payload bytes. The benchmark
allocator is a system-allocator wrapper, not a per-thread production
allocator, and must not be interpreted as exact per-thread or
production accounting.

## Bevy runtime invariants

The runtime also enforces the security boundary at load time:

- The manifest's `schema_version` must be the supported version. Unknown
  versions are rejected before any artifact is touched.
- The `minimum_bevy_version` and `required_capabilities` are
  validated. Mismatches reject the manifest.
- The artifact checksum is verified before spawning
  `WorldAssetRoot`. A mismatch is rejected.
- Artifact paths must resolve beneath the manifest directory after
  canonicalization. Traversal is rejected.

A failure at any step short-circuits to
`load_published_conversion_and_spawn` returning `Err`, and the Bevy
runtime falls back to a `VortexRuntimeDiagnostic` entity with the
stable code `vortex.runtime.artifact_rejected`.

## Audit findings

The full audit at
[`docs/security_audit_t30.md`](../../docs/security_audit_t30.md)
covers 20 categories with no critical or high-severity findings.
Notable items:

- **F-11 (path traversal)** — Covered by systematic `canonicalize +
starts_with(root)` in 22 source files plus the Python bridge
  equivalent.
- **F-15 (subprocess injection)** — Covered by typed
  `Command::new(...).args(...)` with no shell.
- **F-19 (dependency advisories)** — `cargo audit` clean against 1,226
  RustSec advisories. `cargo deny` clean on bans/licenses/sources.
- **F-20 (Windows crate fan-out)** — documented as a known transitive
  divergence in `windows-sys` (0.52 / 0.59 / 0.60). No runtime impact
  on macOS/Linux.

## Where to go next

- [Scope and authorization](scope.md) — what is in and out of scope
- [Performance and observability](performance.md) — what the project
  measures and what it deliberately does not measure
- [Architecture overview](architecture.md) — the collapsed DDD/Hex
  layout
