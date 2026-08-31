#!/usr/bin/env bash
set -euo pipefail

# Verifies that the merge-driver-protected paths in .gitattributes are present
# and tracked. The merge.vortex-preserve driver installed by
# scripts/install-hooks.sh keeps the current branch's reviewed copy on a
# merge, but that driver is only meaningful when the path actually exists in
# the working tree.

readonly required_paths=(
  "bridges"
)

failures=0
for path in "${required_paths[@]}"; do
  if [[ ! -e "${path}" ]]; then
    printf 'error: required Vortex project contract path is missing: %s\n' "${path}" >&2
    failures=1
    continue
  fi

  if ! git ls-files --error-unmatch -- "${path}" >/dev/null 2>&1; then
    printf 'error: required Vortex project contract path is not tracked: %s\n' "${path}" >&2
    failures=1
  fi
done

if (( failures != 0 )); then
  printf 'Restore the required contracts before committing or pushing.\n' >&2
  exit 1
fi

printf 'Preserved Vortex project contracts are present and tracked.\n'