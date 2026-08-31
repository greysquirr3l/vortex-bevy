#!/usr/bin/env bash
# Builds the vortex-bevy release binary and wraps it in a platform-native
# application bundle. The bundle is the launchable artifact on macOS,
# Windows, and Linux; the binary alone is not (it lacks the foreground
# GUI session AppKit/win32/X11 require to open a winit window).
#
# Requires:
#   - cargo install cargo-bundle --locked
#   - On Linux: dpkg-deb (deb), rpmbuild (rpm), or appimagetool (AppImage)
#              depending on the format you want. cargo-bundle picks whatever
#              is on PATH.
#   - On macOS: nothing extra; cargo-bundle produces a self-contained .app.
#   - On Windows: nothing extra; cargo-bundle produces an MSI + the
#                  bundled .exe + SxS manifest.
#
# Output (relative to repo root):
#   - macOS:   target/release/bundle/osx/Vortex-Bevy.app
#   - Windows: target/release/bundle/windows/Vortex-Bevy.msi
#   - Linux:   target/release/bundle/deb/vortex-bevy_<version>_<arch>.deb
#              (and .rpm / .AppImage when their build tools are on PATH)
#
# After running this script, on macOS:
#   open target/release/bundle/osx/Vortex-Bevy.app --args view \
#        --artifact-root /path/to/published/output
# On Windows, double-click the MSI to install, then run from Start Menu.

set -euo pipefail

readonly ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly PACKAGE="vortex-bevy"
readonly BUNDLE_ROOT="${ROOT}/target/release/bundle"

cd "${ROOT}"

if ! command -v cargo-bundle >/dev/null 2>&1; then
  printf 'cargo-bundle is not installed. Install with:\n  cargo install cargo-bundle --locked\n' >&2
  exit 1
fi

printf 'Building release binary...\n'
cargo build --release --bin "${PACKAGE}" -p "${PACKAGE}"

printf 'Packaging platform bundle...\n'
# cargo-bundle reads [package.metadata.bundle] from the crate manifest and
# produces the platform-native bundle under target/release/bundle/.
cargo bundle --release --bin "${PACKAGE}" -p "${PACKAGE}"

case "$(uname -s)" in
  Darwin)
    printf '\nmacOS bundle:\n  %s/osx/Vortex-Bevy.app\n' "${BUNDLE_ROOT}"
    printf '\nLaunch with:\n  open %s/osx/Vortex-Bevy.app --args view \\\n      --artifact-root /path/to/output\n' "${BUNDLE_ROOT}"
    ;;
  Linux)
    printf '\nLinux bundles:\n'
    for ext in deb rpm AppImage; do
      if [[ -d "${BUNDLE_ROOT}/${ext}" ]]; then
        printf '  %s/%s/\n' "${BUNDLE_ROOT}" "${ext}"
      fi
    done
    ;;
  MINGW*|CYGWIN*|MSYS*)
    printf '\nWindows bundles:\n'
    if [[ -d "${BUNDLE_ROOT}/windows" ]]; then
      printf '  %s/windows/\n' "${BUNDLE_ROOT}"
    fi
    ;;
  *)
    printf '\nUnrecognized host. Inspect %s for the produced artifact.\n' "${BUNDLE_ROOT}"
    ;;
esac
