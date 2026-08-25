# Agent rules — vortex-bevy

Vortex-Bevy is a Rust 1.97.1 / Bevy 0.19 asset-conversion and runtime-viewing
workspace for authorized Unreal Engine source projects and OpenUSD stages. It
inspects Unreal package metadata, invokes a minimal Unreal Editor automation
bridge for authoritative export, resolves configured USD stage views, compiles
a typed intermediate representation, packages Bevy-compatible GLB and textures,
and loads the result with source-traceable metadata in Bevy.

The product explicitly distinguishes editable source-project assets from loose,
cooked, encrypted, or proprietary payloads. It never promises functional
Blueprint/C++ conversion. It emits an actionable per-asset classification:
converted, baked, requires_adapter, unsupported, or rejected.


Language: **rust**.

Architecture: **collapsed-ddd-hexagonal** (see rules below).

## Preflight

A preflight failure blocks the commit.

```bash
cargo build --workspace --all-targets --all-features && cargo test --workspace --all-features && cargo fmt --all -- --check && cargo l && cargo deny --all-features check && cargo audit
```


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

## Security (non-negotiable)

These rules apply to every file you touch — no exceptions.

These rules apply to every file you touch — no exceptions.
- Credentials, API keys, and secrets must only be read from environment variables or a secrets manager — never hardcoded in source files.
- All database queries must use parameterised queries (e.g. sqlx's `query!` macro) — never interpolate user input into SQL strings.
- Every HTTP server must set Content-Security-Policy, Strict-Transport-Security, and X-Frame-Options response headers.
- Any endpoint that accepts user input must enforce rate limiting — verify the middleware is wired to the router, not just defined.
- File upload handlers must validate MIME type server-side, reject executable extensions (.exe, .sh, .php, etc.), and enforce a maximum file size.
- Any feature that fetches a URL on behalf of the user must validate the target against an explicit allowlist — never fetch arbitrary user-supplied URLs (SSRF prevention).
- Never use weak/broken cryptography: avoid MD5, SHA-1, DES, 3DES, RC4, Blowfish, or ECB mode. Use SHA-256/SHA-3 for hashing, AES-256-GCM or ChaCha20-Poly1305 for encryption, Argon2id/bcrypt/scrypt for passwords.
- Never disable TLS certificate verification (e.g. danger_accept_invalid_certs) — fix root certificate issues instead of bypassing verification.
- For security-sensitive random values (tokens, nonces, keys), use `rand::rngs::OsRng` or `getrandom` — never `rand::thread_rng()` for cryptographic purposes.
- Never use unsafe deserialisation on untrusted data — prefer `serde_json` over formats that allow arbitrary code execution.
- Sanitise all file paths from user input — canonicalise paths and verify they stay within allowed directories (path traversal prevention).
- Never log sensitive data (passwords, tokens, API keys) — redact before logging or use structured logging with field allowlists.
- Never ship TODO/FIXME comments in authentication or authorisation code — implement security checks now, not later.
- Generate a fresh random IV/nonce for every encryption operation — never reuse or hardcode IVs.


## Strict project standards

This project enables `[style] strict = true`. The generated code must comply with every rule below. Treat them as hard requirements, not suggestions — `preflight` will block any commit that violates them.
- Never use `.expect()` or `.unwrap()` on `Result` or `Option` in production code — propagate with `?` or handle with explicit `match` / `if let` / `.unwrap_or_else(|e| ...)` where the failure mode is genuinely recoverable.
- Never use `.unwrap()` / `.expect()` inside helper closures either — they panic on the first unexpected input and turn recoverable errors into crashes. Use `?` or explicit match.
- Never use `panic!`, `unreachable!`, `todo!`, or `unimplemented!` in production code paths. Reserve them for tests and `match` arms where the invariant has already been statically proven.
- Never use index slicing that can panic (`&vec[i]`, `&s[0..n]`, `arr[0]`, `s[..n]`). Use `.get(i)`, `.get(..n)`, or split-by-pattern APIs and handle `None` explicitly.
- Never add `#[allow(clippy::...)]` suppression attributes on functions, impls, modules, or the crate root — fix the lint instead. Use `#[expect(clippy::lint, reason = "...")]` only for genuinely unavoidable cases, and document the reason.
- Prefer `.is_multiple_of(n)` over `% n == 0` and `% n != 0` (Rust 1.87+, suppresses `clippy::manual_is_multiple_of`).
- Prefer `?` propagation over `.unwrap_or_else(|_| default)` chains where the error type carries information; only collapse to a default when the missing value is semantically the same as the default.
- Use the full clippy profile: `-W clippy::all -W clippy::pedantic -W clippy::nursery -W clippy::cargo -W clippy::perf` and `-D warnings`. Hard-deny `clippy::unwrap_used`, `clippy::expect_used`, `clippy::panic`, `clippy::indexing_slicing`, `clippy::cast_ptr_alignment`, `clippy::suspicious`.
- Add a `.cargo/config.toml` alias `l` for `cargo clippy --workspace --all-targets --` followed by the flags above so lint runs are reproducible.
- Library crates use `thiserror` for typed errors; binary / app crates use `anyhow` with `Context` / `with_context` for rich error chains. Never mix the two across the same crate boundary.
- Parse at the boundary (`TryFrom` / `FromStr` / typed DTOs) — never let free-form `String` cross into domain internals once parsed. Don't validate, parse.
- Deterministic domain APIs: pass time / clock / RNG into domain functions explicitly rather than calling `Utc::now()`, `Instant::now()`, or `rand::thread_rng()` inside them.
- Prefer `LazyLock` (stable since 1.80) over `lazy_static!` for static initialisation.
- Doc comments (`///`) on public items must include `# Examples`, `# Errors`, and `# Panics` sections where the function can fail in any of those ways.



## Writing style (non-negotiable)

Avoid patterns that reveal AI authorship.
- Avoid slop vocabulary: never use 'robust', 'comprehensive', 'streamlined', 'leverage', 'utilize', 'facilitate', 'innovative', 'cutting-edge', 'pivotal', 'seamless', 'synergistic', 'transformative', 'harness', 'delve', 'embark'.
- Avoid filler phrases: never write 'it's worth noting that', 'at its core', 'let's break this down', 'in order to', 'from a broader perspective', 'a key takeaway is', 'this underscores the importance of'.
- Avoid AI prompt leakage: never include phrases like 'as an AI', 'I cannot help with', 'here's the updated', 'let me explain', 'analysis:'.
- Write naturally: prefer short, direct sentences. Vary sentence length. Use contractions where appropriate. Don't over-explain obvious things.
- Code should speak for itself: prefer meaningful names over comments. Only comment non-obvious decisions, safety invariants, and 'why' explanations.

### Commenting guidelines
- Only comment to explain WHY, not WHAT — the code shows what happens; comments explain non-obvious reasoning.
- Preserve safety/security comments: keep comments containing 'Safety:', 'SAFETY:', 'security', 'invariant', 'caveat', 'trade-off'.
- Delete tutorial-style comments that restate the code: '// This function adds two numbers' before `fn add(a, b)` is noise.
- Keep TODO/FIXME/HACK markers for genuine incomplete work, but never in production auth code.
- Doc comments (`///`) should describe contracts and edge cases, not repeat function signatures.



## File structure (non-negotiable)

- Do not create or expand "God" files. Keep each file focused on one primary responsibility.
- If a change introduces a new concern, create a focused module/file instead of extending an unrelated one.
- Avoid catch-all files (`utils`, `helpers`, `common`) containing unrelated logic.
- Keep domain logic, adapters, and orchestration in separate files that mirror the selected architecture.
- If an existing file is already overloaded, prefer extracting cohesive pieces into new files before adding more behavior.

## Strategy


**Complete** — ship root fix end-to-end with tests and docs in the same task. No workarounds; no "follow-up" stubs.

## Commit conventions

- Conventional commits: `feat:`, `fix:`, `refactor:`, `test:`, `docs:`
- Focus commit messages on user impact, not file counts or line numbers

---

_Generated by [wiggum](https://github.com/greysquirr3l/wiggum). This file is a rules document for IDE-side agents; it contains no orchestrator-loop directives. The receiving IDE drives its own agent loop._
