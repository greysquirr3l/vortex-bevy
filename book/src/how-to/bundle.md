# Build a platform bundle

The Bevy viewer window needs a windowing session to open. On macOS
that's an `.app` bundle (so AppKit grants a foreground window
policy); on Windows that's a self-contained `.exe` built directly by `cargo build --release` (cargo-bundle's Windows MSI emitter has known hyphen-binary bugs and its exe emitter requires a resource directory rustc doesn't emit — both intentionally avoided; see GitHub burtonageo/cargo-bundle#77 and #221); on Linux it's
`.desktop` + icon at the XDG paths. On Linux the process also needs a
reachable X11 or Wayland display; without one the windowing backend
cannot start at all.

This page covers building the platform-native bundle that gives the
viewer the session it needs.

## Prerequisites

```bash
cargo install cargo-bundle --version 0.12.0 --locked
```

`cargo-bundle` reads `[package.metadata.bundle]` from
`crates/vortex-bevy/Cargo.toml` and produces platform-native bundles
under `target/release/bundle/`.

For Linux bundles you also need the corresponding build tooling on
PATH (only the format you want):

| Format      | Tool           | Notes                                           |
| ----------- | -------------- | ----------------------------------------------- |
| `.deb`      | `dpkg-deb`     | Installed by default on Debian/Ubuntu           |
| `.AppImage` | `appimagetool` | Download from AppImageKit; optional             |

There is no `.rpm` output. cargo-bundle's RPM emitter is still
unimplemented upstream, so the crate's Linux package types are `.deb` and
`.AppImage` only, and asking for `--format rpm` panics.

macOS and Windows need nothing extra — `cargo-bundle` produces
self-contained bundles on both.

## Build a bundle

```bash
# from the repo root
just bundle
```

or directly:

```bash
cargo build --release --bin vortex-bevy -p vortex-bevy
cargo bundle --release --bin vortex-bevy -p vortex-bevy
```

The wrapper script `scripts/build-bundle.sh` runs the same commands
and prints the produced bundle's location.

## Output locations

| Platform         | Bundle path                                                                                                   |
| ---------------- | ------------------------------------------------------------------------------------------------------------- |
| macOS            | `target/release/bundle/osx/Vortex-Bevy.app` and `target/release/bundle/dmg/Vortex-Bevy.dmg`                   |
| Windows          | `target/release/vortex-bevy.exe` (raw release binary; cargo-bundle skipped on Windows, see burtonageo/cargo-bundle#77 / #221 / `WIN32_ERROR(1812)`) |
| Linux (Debian)   | `target/release/bundle/deb/vortex-bevy_0.2.0_<arch>.deb`                                                      |
| Linux (AppImage) | `target/release/bundle/appimage/vortex-bevy_0.2.0_<arch>.AppImage`                                            |

cargo-bundle does not build an `.rpm`: its RPM emitter is still
unimplemented upstream, so the crate's Linux package types are `.deb` and
`.AppImage` only. Requesting `--format rpm` panics rather than
producing an artifact.

The exact format depends on which Linux build tools are on `PATH` at
build time.

## Launching the bundled viewer

After the bundle is built, the binary needs to be invoked through
the bundle's entry point so AppKit / win32 / X11 grants the foreground
GUI session.

### macOS

```bash
open target/release/bundle/osx/Vortex-Bevy.app --args view \
    --artifact-root /path/to/published/output
```

Or double-click the `.app` in Finder. The window opens in the foreground
session.

### Windows

Download the `.exe` from the GitHub Release, add it to `PATH`, or pin it in the Start Menu. Launch from a terminal or via a desktop shortcut:

```text
"C:\Program Files\Vortex-Bevy\vortex-bevy.exe" view --artifact-root C:\path\to\output
```

### Linux

```bash
# .deb
sudo dpkg -i target/release/bundle/deb/vortex-bevy_*.deb
vortex-bevy view --artifact-root /path/to/output

# AppImage
chmod +x target/release/bundle/appimage/vortex-bevy-*.AppImage
./target/release/bundle/appimage/vortex-bevy-*.AppImage view \
    --artifact-root /path/to/output
```

The `.desktop` file (installed by the `.deb`) lets you launch from the
application menu.

## What the bundle contains

The bundle metadata in `crates/vortex-bevy/Cargo.toml` declares:

- `name = "Vortex-Bevy"` — display name on every platform
- `identifier = "dev.greysquirr3l.vortex-bevy"` — bundle id (used for macOS code signing, Linux .desktop file naming)
- `category = "DeveloperTool"` — macOS app-store-style categorization
- `short_description` / `long_description` — used in `.desktop` files and macOS Get Info pane
- `icon` — the platform icon array
- `[osx]` — macOS-specific settings (`frameworks`, `minimum_system_version`)

cargo-bundle v0.12 parses this table with `deny_unknown_fields`, so a
key it does not declare is a hard error rather than a warning. The
project's homepage and license are not bundle keys; cargo-bundle reads
those from the package's own Cargo metadata, which already declares
both.

Add an icon by dropping the three formats into
`crates/vortex-bevy/assets/bundle/`:

| File                  | Format | Platform |
| --------------------- | ------ | -------- |
| `vortex-bevy.icns`    | ICNS   | macOS    |
| `vortex-bevy.ico`     | ICO    | Windows  |
| `vortex-bevy-512.png` | PNG    | Linux    |

cargo-bundle v0.12 picks the right format per platform from a single
`icon` array in `[package.metadata.bundle]`. The metadata block already
declares these paths; just drop the files in. Regeneration commands
from a single SVG source live in
[`crates/vortex-bevy/assets/bundle/README.md`](../../../crates/vortex-bevy/assets/bundle/README.md).

## Why not a cross-platform CI bundle job?

It exists — `.github/workflows/bundle.yml` builds per-platform
bundles on every `v*` semver tag and attaches them to a draft
GitHub Release named after the tag. The workflow is independent of
`ci.yml` so a release build can be reproduced without paying for the
full quality matrix on every push.

For local development, `just bundle` on each developer's host is
faster than waiting for the ~30 minute CI matrix on every push.

## Distribution channels

The CI workflow publishes **unsigned** bundles as release assets. What
each channel needs beyond that:

| Channel                        | Status                  | Notes                                                                                                                                                                                                                                                                                                                |
| ------------------------------ | ----------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| GitHub Releases                | ✅ workflow lands here  | `.deb`, `.AppImage`, `.dmg`, and `.exe` attached to the tag's release. MSI is a local-developer-only format (deliberately avoided due to cargo-bundle#77 / #221) |
| Homebrew tap                   | Out of scope            | A tap and formula are out of scope until macOS code signing lands                                                                                                                                                                                                                                                    |
| Windows `.exe` (direct download)| ✅                     | Add to PATH or pin to Start Menu. MSI deliberately not shipped — see burtonageo/cargo-bundle#77 and #221                                                                                                                                                                                                                                                         |
| `winget` / `scoop` manifests   | Out of scope            | Submission requires a published release + an authorized manifest review process                                                                                                                                                                                                                                      |
| Debian / Fedora apt/yum repos  | Out of scope            | Repo publication is a separate concern from CI bundling. The unsigned `.deb` / `.rpm` artifacts install locally via `dpkg -i` / `rpm -i`                                                                                                                                                                             |
| Arch Linux AUR                 | **Explicitly excluded** | The AUR ecosystem is currently compromised by fake packages impersonating legitimate projects. Vortex-Bevy will not publish there until the upstream AUR maintainers resolve the supply-chain incident. Track this in [#14](https://github.com/greysquirr3l/vortex-bevy/issues/14) if/when the decision is revisited |

## Going further

- [Tutorial: Convert a USD stage end-to-end](../tutorial/usd-end-to-end.md) — produces a published conversion that the bundled viewer can load.
- [Reference: CLI](../reference/cli.md) — the full CLI flag matrix the bundle exposes.
- [Explanation: Deployment](../explanation/deployment.md) — how the bundle workflow ties into GitHub Releases.
