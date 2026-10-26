#!/usr/bin/env bash
# Verifies that the status checks the branch protection rule requires are
# exactly the checks the CI workflow produces.
#
# GitHub Actions reports each job as a check run named after the job's
# `name` field, not its id. A required check that names a job id never
# matches a result, so the pull request stays blocked forever even when
# every job passes.
#
# The expected contexts are recorded in .github/branch-protection.json.
# That file is the source of truth for the rule and can be compared
# against the live rule with `--live`, which needs a token with admin
# access to the repository. The default check is offline, so it runs in
# CI without any credential.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

readonly workflow=".github/workflows/ci.yml"
readonly expectations=".github/branch-protection.json"

usage() {
  printf 'Usage: %s [--live]\n' "${0##*/}" >&2
  printf '  --live  also compare the recorded contexts against the live rule\n' >&2
}

# Emits the check-run names the workflow produces.
#
# A matrix job reports one check per matrix entry, so `Quality (${{
# matrix.os }})` is expanded against the matrix values. The `runs-on`
# list is not the matrix, so the matrix is read from the strategy block.
workflow_check_names() {
  tr -d '\r' < "${workflow}" | awk '
    function flush() {
      if (job_name != "") {
        if (job_name ~ /\$\{\{ *matrix\.os *\}\}/) {
          for (i = 1; i <= os_count; i++) {
            printf "%s (%s)\n", job_prefix, os_values[i]
          }
        } else {
          print job_name
        }
      }
      job_name = ""
      job_prefix = ""
      os_count = 0
    }
    # A job id at two-space indent starts a new job.
    /^  [A-Za-z0-9_-]+:$/ {
      flush()
      next
    }
    /^    name: / {
      value = $0
      sub(/^    name: /, "", value)
      sub(/[ ]*#.*$/, "", value)
      gsub(/^"|"$/, "", value)
      job_name = value
      prefix = value
      sub(/ *\$\{\{ *matrix\.os *\}\}.*$/, "", prefix)
      sub(/ *\($/, "", prefix)
      job_prefix = prefix
      next
    }
    # The matrix may be an inline array on the `os:` line or a block list.
    /^        os: *\[/ {
      os_count = 0
      line = $0
      sub(/^        os: *\[/, "", line)
      sub(/\].*$/, "", line)
      n = split(line, parts, ",")
      for (i = 1; i <= n; i++) {
        value = parts[i]
        gsub(/[ \t]/, "", value)
        gsub(/^"|"$/, "", value)
        if (value != "") {
          os_count++
          os_values[os_count] = value
        }
      }
      next
    }
    /^        os: *$/ { in_os_block = 1; next }
    in_os_block && /^        - / {
      value = $0
      sub(/^        - /, "", value)
      gsub(/[ \t]/, "", value)
      gsub(/^"|"$/, "", value)
      if (value != "") {
        os_count++
        os_values[os_count] = value
      }
      next
    }
    in_os_block && /^        [A-Za-z0-9_-]+:/ { in_os_block = 0 }
    END { flush() }
  '
}

recorded_check_contexts() {
  sed -n 's/.*"\([^"]*\)".*/\1/p' "${expectations}" \
    | grep -v '^main$' \
    | grep -v '^required_status_checks$'
}

live_check_contexts() {
  local branch
  branch="$(sed -n 's/.*"branch": "\([^"]*\)".*/\1/p' "${expectations}")"
  gh api "repos/${GITHUB_REPOSITORY:-greysquirr3l/vortex-bevy}/branches/${branch}/protection" \
    --jq '.required_status_checks.contexts[]' 2>/dev/null
}

require_command() {
  local command_name="$1"
  if ! command -v "${command_name}" >/dev/null 2>&1; then
    printf 'Required command is unavailable: %s\n' "${command_name}" >&2
    exit 1
  fi
}

# Prints the names present on one side but missing on the other.
#
# The report goes to stdout so the caller can capture it; the caller
# redirects it to stderr.
report_differences() {
  local left_label="$1"
  local left="$2"
  local right_label="$3"
  local right="$4"
  local name

  while IFS= read -r name; do
    [[ -n "${name}" ]] || continue
    if ! printf '%s\n' "${right}" | grep -qxF -- "${name}"; then
      printf '%s has %s, which %s does not list\n' \
        "${left_label}" "${name}" "${right_label}"
    fi
  done <<< "${left}"

  while IFS= read -r name; do
    [[ -n "${name}" ]] || continue
    if ! printf '%s\n' "${left}" | grep -qxF -- "${name}"; then
      printf '%s has %s, which %s does not list\n' \
        "${right_label}" "${name}" "${left_label}"
    fi
  done <<< "${right}"
}

main() {
  local check_live=0
  while (( $# > 0 )); do
    case "$1" in
      --live) check_live=1 ;;
      -h | --help) usage; exit 0 ;;
      *)
        printf 'Unknown argument: %s\n' "$1" >&2
        usage
        exit 2
        ;;
    esac
    shift
  done

  require_command awk
  require_command tr
  require_command sed

  if [[ ! -f "${expectations}" ]]; then
    printf 'Missing %s\n' "${expectations}" >&2
    exit 1
  fi

  local workflow_names recorded_names
  workflow_names="$(workflow_check_names)"
  recorded_names="$(recorded_check_contexts)"

  if [[ -z "${recorded_names}" ]]; then
    printf 'No required status checks recorded in %s\n' "${expectations}" >&2
    exit 1
  fi

  local failures=0
  local differences
  differences="$(report_differences "the CI workflow" "${workflow_names}" \
    "${expectations}" "${recorded_names}" || true)"
  if [[ -n "${differences}" ]]; then
    printf '%s\n' "${differences}" >&2
    failures=$((failures + $(printf '%s\n' "${differences}" | grep -c .)))
  fi
  if (( check_live == 1 )); then
    require_command gh
    local live_names
    live_names="$(live_check_contexts)"
    if [[ -z "${live_names}" ]]; then
      printf 'Unable to read the live branch protection rule.\n' >&2
      printf 'The token needs admin access to the repository.\n' >&2
      exit 1
    fi
    differences="$(report_differences "the live rule" "${live_names}" \
      "${expectations}" "${recorded_names}" || true)"
    if [[ -n "${differences}" ]]; then
      printf '%s\n' "${differences}" >&2
      failures=$((failures + $(printf '%s\n' "${differences}" | grep -c .)))
    fi
  fi

  if (( failures > 0 )); then
    printf 'Branch protection contract failed with %d problem(s).\n' "${failures}" >&2
    exit 1
  fi
  printf 'Branch protection contract passed (%d checks).\n' \
    "$(printf '%s\n' "${recorded_names}" | grep -c .)"
}

main "$@"
