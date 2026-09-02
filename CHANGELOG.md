# Changelog

All notable repository changes are documented here. Versioning follows the support boundaries published in `docs/compatibility_matrix.json`; a capability is never implied by a changelog entry alone.

## Unreleased

### Added

- An optional caller-owned in-process queue for explicitly selected local `.usda` and `.usda.gz` stage requests.
- Bounded non-blocking queue admission, monotonic local submission IDs, per-worker conversion-governor reservations, explicit worker shutdown, and one typed result channel per request.
- Bounded gzip decoding that rejects malformed streams and both compressed and decompressed inputs that exceed the configured limit before stage parsing.
- Deterministic integration tests covering local plain/gzip parity, production-fixture gzip classification, queue configuration, expansion rejection, malformed compression, mismatched encoding, and channel capacity.
- Direct-versus-queued production USD timing evidence and a documented bottleneck analysis.
- A repeatable release-readiness script, action pins, release recovery guidance, and cargo-geiger audit evidence.
- A post-run window-probe in `vortex-bevy view` that surfaces a typed `CliReport` (`reason` + `remediation`) when no `bevy::window::Window` was spawned on the host, so the macOS 26 viewer-window limitation is no longer silent.
- Cross-platform bundle metadata, a `just bundle` recipe, and a `scripts/build-bundle.sh` launcher driven by `cargo-bundle` for `.app`, `.msi`, `.deb`, and `.rpm` outputs.
- A tag-driven `.github/workflows/bundle.yml` that builds per-platform bundles on every `v*` semver push and attaches them to a draft GitHub Release. Distribution channels (GitHub Releases, direct `.app` / `.msi` download, local `.deb` / `.rpm` install) are documented in the book; Homebrew / `winget` / `scoop` / apt+yum repos are out of scope; the Arch Linux AUR is explicitly excluded.
- An MDBook user guide deployed via GitHub Pages and a repository `SECURITY.md` describing the threat model and reporting flow.

### Changed

- Stateless USDA parsing and root-confined stage-file resolution are now crate-private reusable operations. Their validated behavior and external support boundary are unchanged.
- CI action references use immutable commits rather than mutable version tags.
- `vortex-bevy view` is wired through a dedicated `build_viewer_app` builder so the runtime tests keep their headless path and the GUI path runs the Bevy event loop exactly once.
- `validate_selection` now accepts empty input as the documented default (root layer, no overrides) instead of rejecting it as `IdentityError::Empty`. The `default_stage_selection` API no longer panics.

### Fixed

- `vortex-bevy view` exits cleanly with `classification: converted` on macOS 26 (Tahoe) without opening a native window because Bevy 0.19 transitively pins winit 0.30.13 and winit 0.30.x's `-[NSApplication run]` returns immediately on hosts without an explicit foreground GUI session. The published GLB, manifest, and atomic publication are unaffected. The fix lands upstream in winit 0.31 and bevy 0.20 (neither stable yet). Tracked in [#7](https://github.com/greysquirr3l/vortex-bevy/issues/7). On Linux / Windows / macOS ≤ 15 the native viewer window opens normally.
- `valid_recovery_commands_return_explicit_typed_reports` boundary test panicked when the CLI requested the default stage selection; aligned with the `validate_selection` change above.

### Security

- The queue introduces no network ingress, arbitrary archive traversal, output publication, raw source telemetry, or `unsafe` code.
- The release guide records the third-party unsafe-code audit as a finding and requires re-evaluation when dependencies change.

### Compatibility

- The streaming queue does not add full OpenUSD composition, arbitrary binary USD support, recursive reference/payload traversal, pixel decode, GPU upload, or an Editor-free semantic export path.
- The macOS 26 viewer-window limitation does not change the support matrix for `convert` / `inspect` / `plan` / `verify` — those continue to work on macOS 26 and emit the same typed reports. Only the native viewer window requires reverting to macOS ≤ 15 (or waiting for winit 0.31 / bevy 0.20).
