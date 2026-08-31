# Security Policy

## Threat model

Vortex-Bevy is an **offline** CLI plus an optional Bevy 0.19 desktop viewer.
It does not expose an HTTP server, connect to a database, or fetch URLs on
the user's behalf. The attack surface is:

1. **Local filesystem ingestion** of `.uasset`, `.usda`/`.usdc`/`.usd`,
   Force Text Unity projects, `.tscn`, and manifest RON.
2. **Local subprocess invocation** of the authorized Unreal/Unity/Godot
   Editor through typed `Command::new(...).args(...)` adapters.
3. **Bevy desktop window** via `bevy_winit` + `bevy_egui` overlay.

The threat model does **not** include untrusted network input, untrusted
subprocess input, multi-user access, or arbitrary editor installations.

## Supported versions

Only the **latest commit on `main`** receives security updates. We do
not maintain a release-tagged backports branch.

## Reporting a vulnerability

Report private vulnerabilities to the maintainers via GitHub's
[private vulnerability reporting](https://github.com/greysquirr3l/vortex-bevy/security/advisories/new)
flow. This routes the report to maintainers without disclosing it publicly
until a fix is available.

Please include:

- A clear description of the issue and impact.
- Reproduction steps (stage / asset path / command line).
- The commit SHA or tag you reproduced against.
- Any proposed mitigation.

We aim to acknowledge new reports within **5 business days** and to
issue a fix or disclose within **30 days**, depending on severity and
whether the report requires changes outside the project's scope (for
example, fixes to upstream `UnrealEngine` or `Unity` Editor behavior).

## Coordinated disclosure

We follow a 90-day coordinated disclosure window. After 90 days from
the date of an acknowledgement, the reporter is free to disclose the
issue publicly regardless of fix status.

## Out of scope

The following are **explicitly out of scope** for the security policy:

- **Vulnerabilities in upstream engines** (Unreal Editor, Unity Editor,
  Godot Editor, `uasset` crate, Bevy, glTF runtime). Report those to
  the upstream maintainers.
- **Vulnerabilities in cooked, encrypted, DRM-protected, or
  key-dependent content**. Vortex-Bevy detects and rejects these inputs
  (`rejected` classification); it does not bypass access controls.
  Reports about bypasses of those protections will not be accepted as
  valid security findings — they describe inputs the tool is designed
  not to support.
- **Feature requests**. "I want the tool to do X with encrypted Z" is
  not a vulnerability.
- **Performance issues**. The performance baseline documents host-scoped
  observations, not runtime guarantees; benchmarking anomalies are
  filed as performance issues, not security.

## Project-level security posture

The full audit at [`docs/security_audit_t30.md`](docs/security_audit_t30.md)
covers 20 categories (hardcoded secrets, SQL injection, HTTP security
headers, rate limiting, file upload validation, SSRF, weak cryptography,
TLS, RNG, deserialization, path traversal, log redaction, authz TODO/FIXME,
IV/nonce reuse, subprocess injection, cooked/encrypted content,
bridge script provenance, unsafe Rust, dependency vulnerabilities,
duplicate transitive Windows crates) with no critical or high-severity
findings.

Notable items the audit verifies:

- **No encryption or RNG.** SHA-256 content hashing is the only crypto.
  No MD5/SHA-1/AES. No `rand`/`getrandom` usage. Outputs are deterministic
  from typed inputs.
- **No shell.** Subprocess invocation uses `Command::new(...).args(...)`
  with `Vec<OsString>` arguments; no `format!` into a command line.
- **Systematic path confinement.** 22 source files use
  `canonicalize + starts_with(root)`. The Python bridge uses
  `_normalized + relative_to`. Cooked-sidecar rejection at the
  `inspect` boundary. The manifest parser enforces valid relative paths
- **No PII in logs.** `tracing` events carry only `ConversionId`,
  `SourceEngine`, `SubmissionId`, and success booleans. No source paths,
  no project roots, no environment variables, no credentials.
- **Dependency advisories.** `cargo audit` is clean against the latest
  RustSec database; `cargo deny` is clean on bans, licenses, and
  sources.

## Security-relevant build configuration

- **Unsafe Rust is forbidden** at the workspace level
  (`workspace.lints.rust] unsafe_code = "forbid"`).
- **`clippy::unwrap_used`, `expect_used`, `panic`, `indexing_slicing`,
  `cast_ptr_alignment`, `suspicious` are all denied** at the workspace
  level.
- **No network crates** in the dependency graph (verified by `cargo deny`).
- **Bridge tooling is pinned** via `scripts/check-bridge-quality.sh`
  (ruff 0.14.3, gdtoolkit 4.2.2, csharpier 1.3.0, pytest 8.3.5).

## Hardening the build:

`bash scripts/install-hooks.sh` installs three git hooks:

- `pre-commit` runs `cargo fmt --check`, the preserved-contracts check,
  `gitleaks` (if installed), and `cargo audit` (if installed).
- `commit-msg` enforces Conventional Commits format via
  `scripts/check-conventional-commit.sh`.
- `pre-push` runs the full quality gate: `cargo test`, `cargo l`,
  bridge-quality check, release build, and `cargo doc`.

The CI workflow at
[`.github/workflows/ci.yml`](.github/workflows/ci.yml) runs the same
gates across Linux, macOS, and Windows runners, plus separate
`bridge-quality`, `supply-chain`, and `secret-scan` jobs.

## Acknowledgements

We thank the reporters who have privately disclosed vulnerabilities to
this project. (None on record at the time of writing.)

## License

This security policy is part of the vortex-bevy repository and is
licensed under the same terms as the project (MIT OR Apache-2.0).