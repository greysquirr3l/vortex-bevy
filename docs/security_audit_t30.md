# Security Hardening & Vulnerability Review

**Status:** Complete — no critical or high-severity findings.

**Scope:** `vortex-bevy` workspace at commit `f308cec`. Both crates
(`crates/vortex-core`, `crates/vortex-bevy`), the `bridges/` Python editor
bridge, and the Rust dependency graph (574 crates against the RustSec advisory
database, 2026-08-30).

**Reference checklist:** OWASP Top 10 (2021/2025) and the project's
`Strict project standards` and `Security (non-negotiable)` blocks in
`AGENTS.md`.

---

## Threat-model summary

`vortex-bevy` is an **offline, single-binary CLI plus an optional Bevy 0.20
desktop viewer**. It does not expose an HTTP server, does not connect to a
database, and does not fetch arbitrary URLs. The attack surface is:

1. **Local filesystem ingestion** — `.uproject`, `.uasset`, `.usda`/`.usd`,
   `.unity` Force Text, `.tscn` (Godot), `.png`/`.tga`/`.exr`, manifest RON.
2. **Local subprocess invocation** — the authorized Unreal Editor, the
   authorized Unity Editor, and the checked-in Python bridge scripts. All
   invocation goes through typed `Command::new(...).args(...)` adapters; no
   shell is involved.
3. **Bevy desktop viewer** — receives only pre-validated GLB + scene manifest
   from disk; no network, no plug-ins loaded at runtime.

The product explicitly rejects cooked/encrypted/unowned/DRM-protected inputs by
classification. Everything that touches the outside world is a local file or a
caller-supplied editor executable path. This means most OWASP categories
(HTTP, SQL, session auth, SSRF) are **out of scope by design**; the audit
documents that intent rather than treating absence as a gap.

---

## Findings matrix

| ID   | Category                                         | Severity | Status     | Notes                                                                                                                                                                                                                                       |
| ---- | ------------------------------------------------ | -------- | ---------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----- | ---------------- | ----------- | ----------------------------------------------------------------------- |
| F-01 | Hardcoded secrets / credentials                  | —        | N/A        | None present.                                                                                                                                                                                                                               |
| F-02 | SQL injection                                    | —        | N/A        | No database in project.                                                                                                                                                                                                                     |
| F-03 | HTTP security headers (CSP/HSTS/X-Frame-Options) | —        | N/A        | No HTTP server in project.                                                                                                                                                                                                                  |
| F-04 | Rate limiting middleware                         | —        | N/A        | No HTTP server in project.                                                                                                                                                                                                                  |
| F-05 | File-upload MIME/extension/size validation       | Info     | Covered    | Local ingestion is bounded by per-format size caps; see §3.                                                                                                                                                                                 |
| F-06 | SSRF — URL fetch on user behalf                  | —        | N/A        | No URL fetching anywhere in the codebase.                                                                                                                                                                                                   |
| F-07 | Weak/broken cryptography                         | —        | N/A        | Only `sha2 = "0.11.1"` (SHA-256) for `ArtifactHash` checksums. No MD5, SHA-1, DES, AES, Blowfish, ECB.                                                                                                                                      |
| F-08 | Disabled TLS certificate verification            | —        | N/A        | No HTTP client in project.                                                                                                                                                                                                                  |
| F-09 | `rand::thread_rng()` for crypto                  | —        | N/A        | Project is deterministic; no `rand` or `getrandom` dependency, zero call sites.                                                                                                                                                             |
| F-10 | Unsafe deserialization                           | —        | N/A        | Only `serde_json`, `serde_derive`, `ron`, `gltf` — all safe-Rust crates. No `bincode`, `bson`, `postcard`.                                                                                                                                  |
| F-11 | Path traversal                                   | Info     | Covered    | Systematic `canonicalize + starts_with(root)` confinement in 22 files; see §4.                                                                                                                                                              |
| F-12 | Sensitive data in logs                           | Info     | Covered    | `tracing` events are `ConversionId`-correlated and field-allowlisted; no PII or credentials logged.                                                                                                                                         |
| F-13 | Auth/authz TODO/FIXME                            | —        | N/A        | No HTTP/auth subsystem.                                                                                                                                                                                                                     |
| F-14 | IV/nonce reuse                                   | —        | N/A        | No encryption code in project.                                                                                                                                                                                                              |
| F-15 | Subprocess command injection                     | Info     | Covered    | `Command::new(program).args(...)` uses `OsString` arrays — no shell, no string concatenation into a command line. See §5.                                                                                                                   |
| F-16 | Cooked/encrypted/DRM input handling              | Info     | Covered    | Per-asset classification `converted                                                                                                                                                                                                         | baked | requires_adapter | unsupported | rejected`; encryption/license gates are detection-only, never bypassed. |
| F-17 | Subprocess script provenance                     | Info     | Covered    | Only the checked-in `bridges/unreal/vortex_unreal_*.py` scripts are launched; resolved by canonical path relative to a fixed repository root.                                                                                               |
| F-18 | Unsafe Rust                                      | —        | N/A        | `unsafe_code = "forbid"` at workspace level; no production occurrences.                                                                                                                                                                     |
| F-19 | Transitive dependency vulnerabilities            | Info     | Documented | `cargo audit` exit 0 against 1,349 RustSec advisories (587 crates scanned). `cargo deny --all-features check` reports `advisories ok, bans ok, licenses ok, sources ok`. One advisory is carried as a documented, unreachable-code exception — see §8. |
| F-20 | Duplicate transitive Windows crates              | Low      | Documented | `windows-sys` (0.52 / 0.59 / 0.60) and `windows_x86_64_msvc` (0.52.6 / 0.53.1) coexist due to `winit`/`bevy_egui`/`arboard` divergence. Windows-only targets; no runtime impact on Linux/macOS or in the conversion pipeline. Not blocking. |

---

## 1. Hardcoded secrets (F-01)

Workspace-wide grep across `crates/**/*.rs` for credential-shape tokens
(`api_key`, `apikey`, `API_KEY`, `password`, `passwd`, `secret`, `token`,
`secrets.toml`, `.env`):

- **Result:** zero production occurrences. The 132 grep matches are all
  false positives — `Vortex`/`vortex_*` module identifiers and the env-var
  names listed below.
- **Env-var usage** is restricted to two patterns:
  - `CARGO_BIN_EXE_vortex-bevy` — Cargo's own test infrastructure for locating
    the built binary in integration tests.
  - `VORTEX_REAL_UNREAL_*`, `VORTEX_REAL_UNITY_*`, `VORTEX_REAL_GODOT_*` —
    file paths for opt-in real-Editor evidence tests. These are file
    locations, not credentials.
- **Verdict:** no hardcoded credentials. Any future secret must come from
  `env::var` at the primary adapter boundary; no `lazy_static!`/const-string
  for sensitive values.

## 2. SQL / HTTP / rate-limit / SSRF (F-02, F-03, F-04, F-06)

Out of scope by design. Documented so reviewers do not flag absence as a gap:

- No `sqlx`, `diesel`, `rusqlite`, or any DB driver in `Cargo.toml`.
- No `reqwest`, `hyper`, `ureq`, `attohttpc`, `surf` (verified across both
  crates — 22 grep matches are all type names like `UrpSurface`,
  `UsdPreviewSurface`, false positives).
- No `axum`, `actix-web`, `warp`, `rocket`, `tide` server.
- The Bevy desktop viewer uses `bevy_winit` for windowing only; it does not
  open sockets.

## 3. File ingestion size caps and extension allowlists (F-05)

Every local-file reader is bounded:

| Reader                        | Bound                                                       | Source                                             |
| ----------------------------- | ----------------------------------------------------------- | -------------------------------------------------- |
| Unreal bridge report          | 1 MiB (`MAX_REPORT_BYTES`)                                  | `unreal_bridge.rs:21`                              |
| Static-scene bridge reports   | per-adapter cap                                             | `MAX_STATIC_SCENE_REPORT_BYTES` etc.               |
| Artifact manifest             | 1 MiB (`DEFAULT_MANIFEST_LIMIT_BYTES`)                      | `crates/vortex-bevy/src/runtime/manifest.rs:41-42` |
| Python bridge export suffixes | `.fbx`, `.gltf`, `.glb`, `.png`, `.tga`, `.exr` (frozenset) | `bridges/unreal/vortex_unreal_bridge.py:18`        |

All `serde_json::from_*` and `ron::from_*` callsites are preceded by a
`fs::metadata(...).len()` size check, so an oversized file is rejected
without ever invoking the deserializer.

MIME types are validated implicitly by the typed deserializer schema (every
record must match its expected `engine`, `protocol_version`, and field set —
e.g. `RawBridgeReport` requires `engine: "unreal"` and `protocol_version:
"vortex.editor-bridge/v1"`). Off-schema input is rejected with typed errors,
not silently coerced.

## 4. Path traversal (F-11)

Pattern across the codebase: **canonicalize → `starts_with(root)` → reject
otherwise**. 91 grep matches across 22 files demonstrate consistent
application:

- `unreal_package.rs:225-227` — `package_path.canonicalize() + if !
package_path.starts_with(&self.project_root)`.
- `unity.rs:325-327` — `fs::canonicalize(candidate) + if !canonical.starts_with(&request.project_directory)`.
- `unreal_bridge.rs:272-275, 292-295, 518-520` — bridge script, report path,
  output paths all confined to their declared root.
- `artifact_store.rs:135-136, 457-459` — output root and per-artifact path
  confinement.
- `unity_static_scene.rs:689-695` — staged texture root confinement.
- `godot_static_scene.rs:259` — staging root confinement for outputs.

The Python bridge mirrors this with `_normalized(path).relative_to(_normalized(root))`
(`bridges/unreal/vortex_unreal_bridge.py:_confined`).

External-reference rejection in `unity.rs:548` actively blocks `..`, `file://`,
and absolute paths inside Unity Force Text references — these are fail-closed
detectors, not usage.

## 5. Subprocess command construction (F-15, F-17)

The only production subprocess invocation outside of tests is
`crates/vortex-core/src/adapters/secondary/unreal_bridge.rs:218`:

```rust
Command::new(command.program()).args(command.arguments()).output()?
```

`command.program()` is the **configured matching Unreal Editor executable
path**, not a shell string. `command.arguments()` is `Vec<OsString>` produced
by `UnrealBridge::build_command`, which composes fixed
`-unattended`/`-nop4`/`-ExecutePythonScript=...` flags plus the report path
and an explicit asset-path list. No string concatenation, no `format!` into
the command line, no shell expansion.

The Python bridge script itself is resolved by a **fixed relative path** from
a canonicalized repository root
(`BRIDGE_SCRIPT_COMPONENTS = ["bridges", "unreal",
"vortex_unreal_bridge.py"]`), then validated with
`fs::canonicalize(&script_path) + if !script_path.starts_with(&repository_root)
|| !script_path.is_file()`. The bridge never discovers or executes
project-provided scripts (`vortex_unreal_bridge.py` docstring:
"It never discovers or executes project-provided scripts.").

## 6. Cryptography, RNG, deserialization (F-07, F-09, F-10)

- **Hashing:** `sha2 = "0.11.1"` only — used by `ArtifactHash::from_bytes`
  for content-addressable manifest verification. SHA-256, no MD5 or SHA-1.
- **Encryption:** zero. No `aes`, `chacha20poly1305`, `argon2`, `bcrypt`,
  or `scrypt` dependencies. No IV/nonce generation, because no encryption.
- **RNG:** zero. No `rand`, `getrandom`, `OsRng`, `thread_rng`. Project
  determinism rule is enforced: all outputs depend only on typed source
  identities and the export profile.
- **Deserialization formats:** `serde_json`, `ron`, `serde_derive`, `gltf`'s
  typed binary parser. No `bincode`, `bson`, `postcard`, `rmp`, or
  `MessagePack`. `serde_json::from_*` and `ron::from_*` callsites are bounded
  by per-format size caps (see §3) and validated against explicit protocol
  version + engine fields.

## 7. Logging (F-12)

`tracing` is used for structured observation. Audit summary:

- 5 production callsites, all in `crates/vortex-core/src/application/command/`:
  - `execute_conversion.rs:102,113` — `info_span!` and `info!` carrying
    `ConversionId`, `action`.
  - `stream_usd.rs:307,342` — `info!` with `submission_id`, `encoding`,
    `succeeded`.
- All fields are typed `ConversionId`/`SubmissionId` identifiers or boolean
  success flags. No source file paths, no source object paths, no PII, no
  credentials.
- T27 ("redacted `ConversionId`-correlated tracing") is in scope; the audit
  verifies the contract holds at every emit site.

## 8. Dependency vulnerability scan (F-19)

```
$ cargo audit
Loaded 1349 security advisories
Scanning Cargo.lock for vulnerabilities (587 crate dependencies)
EXIT=0
```

```
$ cargo deny --all-features check
advisories ok, bans ok, licenses ok, sources ok
```

Both pass, with one shared exception recorded in `deny.toml` and
`.cargo/audit.toml`:

**RUSTSEC-2026-0344 — decompression bomb in `ruzstd` 0.8.3.**
`bevy_egui` requests `bevy_image/zstd_rust`, so `ruzstd` is compiled into the
build. It is never *called*: its only call site is `bevy_image::ktx2`, which is
compiled solely under the `bevy_image/ktx2` feature, and no crate in this
workspace enables that feature. `nm` over the built `libbevy_image-*.rlib` and
over the linked `vortex-bevy` binary both report zero `ruzstd` symbols, so the
vulnerable decoder is not present in the shipped artifact. This workspace also
never decodes zstd input — textures are normalized to PNG.

The advisory is a denial-of-service only (memory stays in bounds), and the fix
(`ruzstd` 0.9.1) is semver-incompatible with `bevy_image` 0.20.0's `^0.8.0`
requirement, so it cannot be selected today. Both exception entries carry the
same rationale and must be removed once Bevy bumps `ruzstd` to `>=0.9.1`; they
should be re-verified on every dependency bump.

## 9. Workspace lint posture (F-18)

`[workspace.lints.rust] unsafe_code = "forbid"` is enforced across the
workspace. A grep for `unsafe (fn|impl|static|trait|extern)` finds one match,
in an `Error::error` message string in `crates/vortex-core/src/adapters/secondary/unity.rs:224`
("Unity text source contains an unsafe external reference: {reference}")
— that's a runtime safety message, not `unsafe` code.

Production-code lint posture:

- `clippy::unwrap_used = "deny"`, `clippy::expect_used = "deny"`,
  `clippy::panic = "deny"`, `clippy::indexing_slicing = "deny"`,
  `clippy::cast_ptr_alignment = "deny"`, `clippy::suspicious = "deny"`.
- `cargo l` (the project's reproducible lint alias) passes clean.
- `#[expect(...)]` suppressions are used sparingly and only for
  cross-platform `const fn` cfg gating (e.g. `linux_process_memory.rs:87`,
  `desktop.rs:132`). Each carries a documented reason.

## 10. Out-of-scope inputs (F-16)

Cooked/encrypted/unowned/DRM-protected inputs are detected and classified
`unsupported` or `rejected` — never silently bypassed. The audit verifies:

- `uasset` 0.6 metadata preflight rejects cooked sidecars with a typed error.
- `UStaticMesh` / `USkeletalMesh` / `UTexture2D` payloads are read from
  Editor-exported staged files only, not raw `.uasset` bulk data.
- `UTexture2D` direct decoder (T26) is metadata-only — it reads tag tables,
  not pixels or bulk payloads. Authoritative export still routes through the
  Editor.

## 11. Recommended follow-up

1. **F-20 (Windows crate fan-out):** track `winit`/`bevy_egui`/`arboard`
   upstream alignment on `windows-sys` 0.59+. Not blocking — Windows
   compilation already works; this is purely a `cargo tree` noise issue.
2. **Documentation:** add a one-paragraph "security scope" note to
   `README.md` stating the offline-CLI + Bevy-viewer threat model, so future
   contributors do not mistake the absence of HTTP auth as a gap.

---

## Cross-reference with project rules

| Project rule                                       | Evidence                             |
| -------------------------------------------------- | ------------------------------------ |
| "Credentials only from env vars / secrets manager" | §1                                   |
| "Never use weak/broken cryptography"               | §6 (SHA-256 only)                    |
| "Never disable TLS certificate verification"       | §2 (no TLS)                          |
| "Never use `rand::thread_rng()` for crypto"        | §6 (no RNG)                          |
| "Never use unsafe deserialization"                 | §6 (safe formats only)               |
| "Sanitize file paths from user input"              | §4 (canonicalize + root confinement) |
| "Never log sensitive data"                         | §7 (`ConversionId`-only fields)      |
| "No TODO/FIXME in auth code"                       | §2 (no auth subsystem)               |
| "Generate fresh IV/nonce for every encryption op"  | §6 (no encryption)                   |

---

## Deliverable

This document at `docs/security_audit_t30.md` is the findings document
("The deliverable is the findings document, not a code feature"). No
source-code changes were required by the audit.
