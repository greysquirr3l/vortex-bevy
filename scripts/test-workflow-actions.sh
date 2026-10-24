#!/usr/bin/env bash
# Exercises the workflow action contract against synthetic action manifests.
#
# The real check resolves action.yml from GitHub, which needs network access.
# These cases use a pre-populated cache directory so the parsing rules are
# verified offline and cannot silently regress.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

readonly script="scripts/check-workflow-actions.sh"
readonly work_dir="$(mktemp -d)"
trap 'rm -rf "${work_dir}"' EXIT

readonly cache_dir="${work_dir}/cache"
readonly sha="0123456789abcdef0123456789abcdef01234567"

failures=0

# Writes a synthetic action.yml into the manifest cache. The cache key matches
# the one the check computes for `owner/name@<sha>`.
write_manifest() {
  local owner="$1"
  local name="$2"
  local manifest_sha="$3"
  local subpath="$4"
  local body="$5"
  local cached="${cache_dir}/${owner}-${name}-${manifest_sha}-${subpath//\//-}.yml"
  mkdir -p "$(dirname "${cached}")"
  printf '%s' "${body}" > "${cached}"
}

# Runs the check over a single synthetic workflow and asserts the exit status.
run_case() {
  local name="$1"
  local workflow="$2"
  local expected_status="$3"

  local workflow_dir="${work_dir}/${name}/workflows"
  mkdir -p "${workflow_dir}"
  printf '%s' "${workflow}" > "${workflow_dir}/case.yml"

  local output
  local status=0
  output="$(VORTEX_WORKFLOW_ROOT="${workflow_dir}" \
    VORTEX_WORKFLOW_ACTION_CACHE="${cache_dir}" \
    bash "${script}" 2>&1)" || status=$?

  if [[ "${status}" != "${expected_status}" ]]; then
    printf 'FAIL %s: expected exit %s, got %s\n' "${name}" "${expected_status}" "${status}" >&2
    printf '%s\n' "${output}" >&2
    failures=$((failures + 1))
    return
  fi
  printf 'ok   %s\n' "${name}"
}

write_manifest "example" "action" "${sha}" "" 'name: Example
inputs:
  first:
    description: A required input
    required: true
  second:
    description: An optional input
    default: fallback
'

write_manifest "example" "defaulted" "${sha}" "" 'name: Example
inputs:
  only:
    description: Has a default
    required: true
    default: value
'

write_manifest "example" "crlf" "${sha}" "" $'name: Example\r\ninputs:\r\n  only:\r\n    description: Windows line endings\r\n    required: true\r\n'

# A step that supplies every required input and only declared inputs.
run_case "satisfied" \
  'name: Case
on: [push]
permissions:
  contents: read
jobs:
  job:
    runs-on: ubuntu-latest
    steps:
      - uses: example/action@0123456789abcdef0123456789abcdef01234567
        with:
          first: value
' 0

# A step that passes an input the action does not declare.
run_case "unknown_input" \
  'name: Case
on: [push]
permissions:
  contents: read
jobs:
  job:
    runs-on: ubuntu-latest
    steps:
      - uses: example/action@0123456789abcdef0123456789abcdef01234567
        with:
          first: value
          bogus: value
' 1

# A step that omits a required input.
run_case "missing_required" \
  'name: Case
on: [push]
permissions:
  contents: read
jobs:
  job:
    runs-on: ubuntu-latest
    steps:
      - uses: example/action@0123456789abcdef0123456789abcdef01234567
        with:
          second: value
' 1

# A required input that has a default is satisfied without `with:`.
run_case "required_with_default" \
  'name: Case
on: [push]
permissions:
  contents: read
jobs:
  job:
    runs-on: ubuntu-latest
    steps:
      - uses: example/defaulted@0123456789abcdef0123456789abcdef01234567
' 0

# An action manifest that uses CRLF line endings parses the same way.
run_case "crlf_manifest" \
  'name: Case
on: [push]
permissions:
  contents: read
jobs:
  job:
    runs-on: ubuntu-latest
    steps:
      - uses: example/crlf@0123456789abcdef0123456789abcdef01234567
' 1

# An action reference that is not pinned to a commit SHA.
run_case "unpinned" \
  'name: Case
on: [push]
permissions:
  contents: read
jobs:
  job:
    runs-on: ubuntu-latest
    steps:
      - uses: example/action@v1
' 1

# `uses:` is not the first key of the step.
run_case "uses_after_name" \
  'name: Case
on: [push]
permissions:
  contents: read
jobs:
  job:
    runs-on: ubuntu-latest
    steps:
      - name: Resolve
        uses: example/action@0123456789abcdef0123456789abcdef01234567
        with:
          first: value
' 0

# A `run:` body after the `with:` block must not be read as an input.
run_case "run_body_is_not_an_input" \
  'name: Case
on: [push]
permissions:
  contents: read
jobs:
  job:
    runs-on: ubuntu-latest
    steps:
      - uses: example/action@0123456789abcdef0123456789abcdef01234567
        with:
          first: value
        run: |
          echo "not-an-input: value"
' 0

# A version comment after the SHA must not become part of the pinned ref.
run_case "version_comment" \
  'name: Case
on: [push]
permissions:
  contents: read
jobs:
  job:
    runs-on: ubuntu-latest
    steps:
      - uses: example/action@0123456789abcdef0123456789abcdef01234567 # v1.2.3
        with:
          first: value
' 0

if (( failures > 0 )); then
  printf 'Workflow action contract tests failed: %d case(s).\n' "${failures}" >&2
  exit 1
fi
printf 'Workflow action contract tests passed.\n'
