#!/usr/bin/env bash
# Checks that the build artifacts fit inside the runner's disk budget.
#
# The ubuntu runner has roughly 14 GB of free disk. A default debug build
# of this workspace is 12 GB, and `cargo test` and `cargo l` add their own
# artifacts on top, so the runner runs out of disk partway through the link
# step and fails with "ld terminated with signal 7 [Bus error]". That
# message points at the linker rather than at the disk, which makes it
# expensive to diagnose.
#
# This check runs after the build and fails with a message that names the
# actual cause.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

# The budget leaves headroom for the toolchain, the installed mdbook, the
# runner image, and the temporary files Cargo writes during a build.
readonly budget_mb="${VORTEX_DISK_BUDGET_MB:-10240}"

target_dir="${CARGO_TARGET_DIR:-${ROOT}/target}"

if [[ ! -d "${target_dir}" ]]; then
  printf 'No build directory at %s\n' "${target_dir}" >&2
  exit 1
fi

# `du -sk` is the portable spelling; macOS and Linux both accept it.
size_kb="$(du -sk "${target_dir}" 2>/dev/null | awk '{print $1}')"
if [[ -z "${size_kb}" ]]; then
  printf 'Unable to measure the build directory at %s\n' "${target_dir}" >&2
  exit 1
fi

size_mb=$(( size_kb / 1024 ))
printf 'Build directory: %s MB (budget %s MB)\n' "${size_mb}" "${budget_mb}"

if (( size_mb > budget_mb )); then
  printf 'The build exceeds the runner disk budget.\n' >&2
  printf 'The runner will run out of disk during the link step and fail with\n' >&2
  printf 'a bus error rather than a disk error. Reduce the artifact size, or\n' >&2
  printf 'raise VORTEX_DISK_BUDGET_MB if the runner has more room.\n' >&2
  exit 1
fi

if command -v df >/dev/null 2>&1; then
  df -Pk "${target_dir}" | awk 'NR == 2 { printf "Free space on the build volume: %.1f GB\n", $4 / 1048576 }'
fi
