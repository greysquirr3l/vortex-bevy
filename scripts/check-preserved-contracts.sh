#!/usr/bin/env bash
set -euo pipefail

readonly required_files=(
  ".config/dotnet-tools.json"
  "PROGRESS.md"
  "docs/compatibility_matrix.json"
  "features.json"
  "plan.toml"
)

failures=0
for path in "${required_files[@]}"; do
  if [[ ! -f "${path}" ]]; then
    printf 'error: required Vortex project contract is missing: %s\n' "${path}" >&2
    failures=1
    continue
  fi

  if ! git ls-files --error-unmatch -- "${path}" >/dev/null 2>&1; then
    printf 'error: required Vortex project contract is not tracked: %s\n' "${path}" >&2
    failures=1
  fi
done

shopt -s nullglob
readonly task_files=(tasks/*.md)
if (( ${#task_files[@]} == 0 )); then
  printf 'error: no generated Wiggum task files are present under tasks/.\n' >&2
  failures=1
fi

for path in "${task_files[@]}"; do
  if ! git ls-files --error-unmatch -- "${path}" >/dev/null 2>&1; then
    printf 'error: generated task file is not tracked: %s\n' "${path}" >&2
    failures=1
  fi
done

if (( failures != 0 )); then
  printf 'Restore the required contracts before committing or pushing.\n' >&2
  exit 1
fi

printf 'Preserved Vortex project contracts are present and tracked.\n'
