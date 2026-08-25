# T08 — Implement Rust-first Unreal package inspection and preflight

> **Depends on**: T-cli-and-input-boundaries.

## Goal

Use a version-pinned Rust package reader behind an adapter to inspect authorized .uasset metadata, identify dependencies and companion-file requirements, classify input eligibility, and produce a deterministic inspection report.

## Project Context

- Project: `vortex-bevy` — Vortex-Bevy is a Rust 1.97.1 / Bevy 0.19 asset-conversion and runtime-viewing
workspace for authorized Unreal Engine source projects and OpenUSD stages. It
inspects Unreal package metadata, invokes a minimal Unreal Editor automation
bridge for authoritative export, resolves configured USD stage views, compiles
a typed intermediate representation, packages Bevy-compatible GLB and textures,
and loads the result with source-traceable metadata in Bevy.

The product explicitly distinguishes editable source-project assets from loose,
cooked, encrypted, or proprietary payloads. It never promises functional
Blueprint/C++ conversion. It emits an actionable per-asset classification:
converted, baked, requires_adapter, unsupported, or rejected.

- Language: rust
- Architecture: collapsed-ddd-hexagonal



## Strategy: Complete (End-to-End)

### Completion contract

- Implement the root fix for this task end-to-end (avoid temporary workaround paths)
- Add/update tests for behavior changes, including at least one edge/failure case
- Update relevant documentation in this task before marking complete

### Implementation

- Evaluate and pin jorgenpt/uasset-rs behind the secondary adapter boundary. Keep its types out of domain/application public contracts.
- Inspect package header, names, imports, exports, object classes, engine/custom versions, package flags, expected companion paths, and source identity without attempting to decode every UObject payload.
- Detect loose package, editable source project context, cooked-style companion files, .pak/.utoc/.ucas context, missing companions, incompatible versions, and unknown asset classes. Emit explicit classifications and remediation.
- Never decrypt, guess keys, bypass access controls, or claim that a cooked/encrypted file is safely convertible. Detect and reject by default.
- Use safe path handling: canonicalize within the authorized project root, reject traversal and symlink escape, enforce size limits, and avoid loading huge payloads during metadata preflight.


### Tests

- Test self-authored synthetic package headers for supported, missing-companion, incompatible-version, malformed, unknown-class, and rejected-cooked scenarios.
- Property-test or fuzz the metadata boundary for malformed bytes; it must return errors without panic or unbounded allocation.
- Test project-root escape attempts and symlink traversal handling on supported platforms.


### Docs updates

- Update README/docs/notes that are affected by this task's behavior or public contract changes




## File Structure (Anti-Godfile)

- Keep each changed file focused on one primary responsibility.
- If this task introduces a new concern, create a focused module/file instead of extending an unrelated catch-all file.
- Do not expand `utils`, `helpers`, or `common` into multi-purpose dumping grounds.
- If a file is already overloaded, extract cohesive pieces before adding new behavior.



## Strict Standards (Project-wide)

This task is part of a project that opts in to `[style] strict = true`.
Generated code must comply with every rule below — `preflight` will block
the commit otherwise.

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



## Housekeeping: TODO / FIXME Sweep

Before running preflight, scan all files you created or modified in this task for
`TODO`, `FIXME`, `HACK`, `XXX`, and similar markers.

- **Resolve** any that fall within the scope of this task's goal.
- **Leave in place** any that reference work belonging to a later task or phase — but ensure they include a task reference (e.g. `// TODO(T07): wire up auth adapter`).
- **Remove** any placeholder markers that are no longer relevant after your implementation.

If none are found, move on.

## Preflight

```bash
cargo build --workspace --all-targets --all-features && cargo test --workspace --all-features && cargo fmt --all -- --check && cargo l && cargo deny --all-features check && cargo audit
```

## Exit Criteria

- [ ] All code compiles without errors or warnings
- [ ] All tests pass
- [ ] Linter passes with no warnings
- [ ] `cargo deny --all-features check && cargo audit` reports no vulnerabilities
- [ ] Implementation matches the goal described above
- [ ] No unresolved TODO/FIXME/HACK markers that belong to this task's scope

- [ ] Malformed package fixtures never panic under unit and fuzz smoke tests
- [ ] A preflight report includes engine/version clues, class, dependencies, and conversion eligibility
- [ ] Path traversal and missing companion tests fail closed


## After Completion

Update PROGRESS.md row for T08 to `[x]`.
Commit: `feat(uasset-inspection): implement implement rust-first unreal package inspection and preflight`
