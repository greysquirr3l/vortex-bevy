#!/usr/bin/env bash
set -euo pipefail

readonly fixture_root="fixtures"

# Pick the sha256 tool the host has. `sha256sum` is GNU coreutils (Linux,
# homebrew coreutils on macOS); `shasum -a 256` is BSD/macOS userland.
# The resolved command is invoked via "${hash_cmd[@]}" so the hot loop
# stays portable across both without branching at the call site.
if command -v sha256sum >/dev/null 2>&1; then
  readonly hash_cmd=(sha256sum)
elif command -v shasum >/dev/null 2>&1; then
  readonly hash_cmd=(shasum -a 256)
else
  printf 'Neither sha256sum nor shasum is available; cannot verify fixtures.\n' >&2
  exit 1
fi

while IFS= read -r -d '' manifest; do
  source_name=$(sed -n 's/^[[:space:]]*"source": "\([^"]*\)".*/\1/p' "${manifest}")
  expected_hash=$(sed -n 's/^[[:space:]]*"sha256": "\([a-f0-9]*\)".*/\1/p' "${manifest}")

  if [[ -z "${source_name}" || -z "${expected_hash}" ]]; then
    printf 'Invalid fixture manifest: %s\n' "${manifest}" >&2
    exit 1
  fi

  source_path="$(dirname "${manifest}")/${source_name}"
  if [[ ! -f "${source_path}" ]]; then
    printf 'Fixture source is missing: %s\n' "${source_path}" >&2
    exit 1
  fi

  actual_hash=$("${hash_cmd[@]}" "${source_path}" | awk '{print $1}')
  if [[ "${actual_hash}" != "${expected_hash}" ]]; then
    printf 'Fixture hash mismatch for %s\n' "${source_path}" >&2
    exit 1
  fi
done < <(find "${fixture_root}" -name '*.fixture.json' -print0)

printf 'Fixture manifests are deterministic and verified.\n'
