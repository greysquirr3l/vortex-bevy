# Changelog

All notable repository changes are documented here. Versioning follows the support boundaries published in `docs/compatibility_matrix.json`; a capability is never implied by a changelog entry alone.

## Unreleased

### Changed

- Every GitHub Action is now pinned to a commit SHA on a Node 24 release. The previous pins targeted Node 20 actions, and GitHub removed the Node 20 runtime from hosted runners on 2026-09-16, so the workflows could not start. `actions/checkout` moves to v7.0.1, `actions/setup-python` to v7.1.0, `actions/setup-dotnet` to v6.0.0, `gitleaks/gitleaks-action` to v3.0.0, `github/codeql-action` to v4.39.1, `actions/upload-artifact` to v7.0.2, `actions/download-artifact` to v8.0.2, `actions/upload-pages-artifact` to v5.0.0, `actions/deploy-pages` to v5.0.1, and `actions/configure-pages` to v6.0.0. `softprops/action-gh-release` was already pinned to v3.0.3; only its version comment was wrong.
- The book deployment opts back into hidden files for the Pages artifact. `actions/upload-pages-artifact` v4 stopped shipping them by default, but mdbook writes `.nojekyll` into the build directory to tell GitHub Pages to skip Jekyll processing.
- The Dependabot open pull request limit is raised from five to ten for both the Cargo and GitHub Actions ecosystems. The previous limit saturated the queue, so the remaining outdated dependencies were never offered.

### Fixed

- `actions/checkout` in the book workflow was referenced by a floating version tag instead of a commit SHA, unlike every other action in the repository.
- The release workflow passed `tag` to `softprops/action-gh-release`, which the action does not declare. The action ignored the unknown input and fell back to `github.ref_name`, so a manually dispatched release would have been published against the branch instead of the requested tag. The input is now `tag_name`.
- The CodeQL workflow invoked `dtolnay/rust-toolchain` without its required `toolchain` input, so the job failed at that step with `'toolchain' is a required input` before it ever reached the build. The action does not read `rust-toolchain.toml`.
- `clipboard-win` 5.4.1 was yanked upstream, which failed `cargo deny --all-features check` and blocked the supply-chain job. It is updated to 5.4.3, which satisfies `arboard`'s `^5.3.1` requirement.
- The documentation contract failed on a clean checkout because four tracked files linked to `docs/performance_baseline.md`, `docs/release_hardening.md`, and `docs/operations_and_architecture_guide.md`. Those files are gitignored and untracked, so they are present on a developer machine and absent from CI. The links now point at the book chapters and the tracked contracts that replaced them. The contract also failed on eighteen book chapters whose relative links resolved one directory too high, and on one chapter that linked `./reference/cli.md` instead of `../reference/cli.md`.
- The branch protection rule on `main` required three status checks that no CI job produces. GitHub Actions names a check run after the job's `name` field, but the rule required the job ids `supply-chain`, `bridge-quality`, and `secret-scan` instead of the display names `Supply chain and fixture policy`, `Authorized Editor bridge quality`, and `Secret scan`. Those three checks could never report a result, so every pull request stayed blocked even once the jobs passed. The rule now requires the display names, and `scripts/check-branch-protection.sh` compares the rule against the CI workflow so a job rename cannot silently break it again.

- `vortex-bevy view` never opened a native window on any platform. The `bevy_winit` feature was not enabled on the `bevy` dependency, so Bevy's `DefaultPlugins` did not install `WinitPlugin` and the viewer had no event loop at all: `App::run` fell back to Bevy's single-tick runner and returned immediately. The `bevy_winit` feature is now enabled, and the viewer opens a real window. The macOS 26 / winit 0.31 explanation in [#7](https://github.com/greysquirr3l/vortex-bevy/issues/7) was a misdiagnosis: winit 0.30.13 opens windows normally on macOS 26, and the Bevy 0.20 release that pinned it was never the problem.
- `vortex-bevy view` reported `vortex.runtime.window_unavailable` for every host because the post-run window probe queried the caller's `App` after `App::run` had moved the world into the event loop, leaving that `App` empty. The probe now counts `Window` entities from inside the schedule and keeps the highest count it saw, so a window that was created and later closed still reports as created.
- `vortex-bevy view` failed to load the published scene because the asset server root was the artifact root rather than the conversion directory that owns the manifest, so the manifest's relative GLB path resolved to a file that does not exist. The viewer now serves assets from the conversion directory.
- The desktop viewer no longer fails to start when Bevy's default tonemapper is selected: the `tonemapping_luts` feature is enabled on the `bevy` dependency.
- The `bevy_egui` overlay no longer warns that `PickingPlugin` is missing: the `bevy_picking` feature is enabled on the `bevy` dependency.

### Added

- Bevy's logger is enabled for the desktop viewer, so renderer, asset, and windowing diagnostics reach the operator instead of failing silently.
- A typed `published_conversion_asset_root` helper that resolves the directory a published conversion's assets are served from.
- A workflow action contract check that resolves the `action.yml` of every pinned action and verifies that each `with:` key names a declared input and that no required input is missing. `actionlint` does not resolve remote actions, so it cannot catch either mistake. The check runs in the CI quality job and in the release readiness script, and `scripts/test-workflow-actions.sh` covers its parsing rules against synthetic manifests.

## 0.1.0 - 2026-10-22

The first full release. The runtime and CLI target Bevy 0.20, generated manifests declare `minimum_bevy_version: "0.20"`, and the runtime refuses a manifest that declares any other version.

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
- Multi-resolution bundle icon assets under `crates/vortex-bevy/assets/bundle/` (1024 master PNG, plus 16→512 sizes; macOS `.icns`; Windows multi-resolution `.ico`). Cargo-bundle picks the right format per platform from a single `icon` array in `[package.metadata.bundle.bin.vortex-bevy]`. The hexagon+swirl source render is cropped with a 1% fuzz threshold and a 32 px transparent border so it renders cleanly at 16 px without edge bleed.
- An MDBook user guide deployed via GitHub Pages and a repository `SECURITY.md` describing the threat model and reporting flow.

### Changed

- Stateless USDA parsing and root-confined stage-file resolution are now crate-private reusable operations. Their validated behavior and external support boundary are unchanged.
- CI action references use immutable commits rather than mutable version tags.
- `vortex-bevy view` is wired through a dedicated `build_viewer_app` builder so the runtime tests keep their headless path and the GUI path runs the Bevy event loop exactly once.
- `validate_selection` now accepts empty input as the documented default (root layer, no overrides) instead of rejecting it as `IdentityError::Empty`. The `default_stage_selection` API no longer panics.
- The runtime and CLI now target Bevy 0.20 (with `bevy_egui` 0.43.1) instead of Bevy 0.19. Bevy 0.20 requires Rust 1.97.1, which matches the toolchain this repository already pins. The conversion output contract is unchanged: generated manifests now declare `minimum_bevy_version: "0.20"`, and the runtime rejects manifests that declare any other version. The support-matrix capability id is now `bevy.0_20_verified_runtime`.
- The macOS 26 viewer-window report now names Bevy 0.20 as the runtime that still pins winit 0.30.13; the limitation itself is unchanged, because Bevy 0.20 has not adopted winit 0.31 yet.
- `RUSTSEC-2026-0344` (a decompression bomb in `ruzstd` 0.8.3) is carried as a documented exception in `deny.toml` and `.cargo/audit.toml`. `ruzstd` reaches the build through `bevy_egui`'s `bevy_image/zstd_rust` request, but its only call site is `bevy_image::ktx2`, which is compiled solely under the `bevy_image/ktx2` feature that nothing here enables — the built `libbevy_image` rlib and the linked `vortex-bevy` binary both contain zero `ruzstd` symbols. The advisory is denial-of-service only, and the fix (`ruzstd` 0.9.1) is semver-incompatible with `bevy_image` 0.20.0's `^0.8.0` requirement. Both entries must be removed once Bevy bumps `ruzstd` to `>=0.9.1`.

### Fixed

- `vortex-bevy view` exits cleanly with `classification: converted` on macOS 26 (Tahoe) without opening a native window because Bevy transitively pins winit 0.30.13 and winit 0.30.x's `-[NSApplication run]` returns immediately on hosts without an explicit foreground GUI session. The published GLB, manifest, and atomic publication are unaffected. The fix lands upstream in winit 0.31, which Bevy has not adopted yet. Tracked in [#7](https://github.com/greysquirr3l/vortex-bevy/issues/7). On Linux / Windows / macOS ≤ 15 the native viewer window opens normally.
- `valid_recovery_commands_return_explicit_typed_reports` boundary test panicked when the CLI requested the default stage selection; aligned with the `validate_selection` change above.
- The Quality CI job failed to compile on every platform because `docs/compatibility_matrix.json` was gitignored while `cli_boundary.rs` asserts against it with `include_str!`. A test cannot compile against an ignored path, so the file built locally and failed on any clean checkout. The file is the authoritative machine-readable snapshot of `vortex-bevy support-matrix --format json`, so it is tracked again; the book chapter remains the curated human summary.
- The Authorized Editor bridge CI job failed before running a single check because gdtoolkit 4.2.2 imports `pkg_resources`, which setuptools 81 removed and Python 3.12 no longer bundles. setuptools is now pinned below 81 in the CI lint-tool install. gdtoolkit stays on 4.2.2 because 4.5.0 also reformats `bridges/godot/vortex_godot_bridge.gd`, so moving to it would mean reformatting the bridge in the same change.

### Security

- The queue introduces no network ingress, arbitrary archive traversal, output publication, raw source telemetry, or `unsafe` code.
- The release guide records the third-party unsafe-code audit as a finding and requires re-evaluation when dependencies change.

### Compatibility

- The streaming queue does not add full OpenUSD composition, arbitrary binary USD support, recursive reference/payload traversal, pixel decode, GPU upload, or an Editor-free semantic export path.
- The macOS 26 viewer-window limitation does not change the support matrix for `convert` / `inspect` / `plan` / `verify` — those continue to work on macOS 26 and emit the same typed reports. Only the native viewer window requires reverting to macOS ≤ 15 (or waiting for winit 0.31 / bevy 0.20).
