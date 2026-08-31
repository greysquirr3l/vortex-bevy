# Build a platform bundle

The Bevy viewer window needs a foreground GUI session to open. On
macOS that's an `.app` bundle (so AppKit grants a foreground window
policy); on Windows that's an MSI with a SxS manifest; on Linux it's
`.desktop` + icon at the XDG paths. A bare `cargo run` from a
non-foreground shell can't open a Bevy window — you'll see the typed
report printed and the process exits silently.

This page covers building the platform-native bundle that gives the
viewer the foreground session it needs.

## Prerequisites

```bash
cargo install cargo-bundle --locked
```

`cargo-bundle` reads `[package.metadata.bundle]` from
`crates/vortex-bevy/Cargo.toml` and produces platform-native bundles
under `target/release/bundle/`.

For Linux bundles you also need the corresponding build tooling on
PATH (only the format you want):

| Format      | Tool           | Notes                                           |
| ----------- | -------------- | ----------------------------------------------- |
| `.deb`      | `dpkg-deb`     | Installed by default on Debian/Ubuntu           |
| `.rpm`      | `rpmbuild`     | Installed via `dnf install rpm-build` on Fedora |
| `.AppImage` | `appimagetool` | Download from AppImageKit; optional             |

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
| macOS            | `target/release/bundle/osx/Vortex-Bevy.app`                                                                   |
| Windows          | `target/release/bundle/windows/Vortex-Bevy.msi` (and the bundled `.exe` + SxS manifest in the same directory) |
| Linux (Debian)   | `target/release/bundle/deb/vortex-bevy_0.1.0_<arch>.deb`                                                      |
| Linux (Fedora)   | `target/release/bundle/rpm/vortex-bevy-0.1.0.<arch>.rpm`                                                      |
| Linux (AppImage) | `target/release/bundle/AppImage/Vortex-Bevy_0.1.0_<arch>.AppImage`                                            |

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

Install the MSI (double-click it, or `msiexec /i Vortex-Bevy.msi /qb`). Launch from Start Menu, or directly:

```text
"C:\Program Files\Vortex-Bevy\vortex-bevy.exe" view --artifact-root C:\path\to\output
```

### Linux

```bash
# .deb
sudo dpkg -i target/release/bundle/deb/vortex-bevy_*.deb
vortex-bevy view --artifact-root /path/to/output

# .rpm
sudo rpm -i target/release/bundle/rpm/vortex-bevy-*.rpm
vortex-bevy view --artifact-root /path/to/output

# AppImage
chmod +x target/release/bundle/AppImage/Vortex-Bevy-*.AppImage
./target/release/bundle/AppImage/Vortex-Bevy-*.AppImage view \
    --artifact-root /path/to/output
```

The `.desktop` file (installed by the `.deb` / `.rpm`) lets you launch
from the application menu.

## What the bundle contains

The bundle metadata in `crates/vortex-bevy/Cargo.toml` declares:

- `name = "Vortex-Bevy"` — display name on every platform
- `identifier = "dev.greysquirr3l.vortex-bevy"` — bundle id (used for macOS code signing, Linux .desktop file naming)
- `category = "DeveloperTool"` — macOS app-store-style categorization
- `short_description` / `long_description` — used in `.desktop` files and macOS Get Info pane
- `homepage` — links the bundled app to the project
- `license = "MIT OR Apache-2.0"` — bundled Info.plist / installer metadata

Add an icon by dropping a `.icns` (macOS), `.ico` (Windows), and
`.png` (Linux) into `crates/vortex-bevy/assets/bundle/` and pointing
`macos_icon_path` / `windows_icon_path` at them.

## Why not a cross-platform CI bundle job?

Cross-platform `cargo bundle` runs are real (Ubuntu + macOS-14 +
windows-latest in CI), but each matrix needs the platform-specific
packaging tools (appimagetool, MSI bundler, codesign on macOS) and the
artifact-upload steps. That job belongs in a release workflow, gated
on tags. For local development, the `just bundle` recipe on each
developer's host is faster and avoids the ~30 minutes of CI bundling
on every push.

## Going further

- [Tutorial: Convert a USD stage end-to-end](../tutorial/usd-end-to-end.md) — produces a published conversion that the bundled viewer can load.
- [Reference: CLI](./reference/cli.md) — the full CLI flag matrix the bundle exposes.
