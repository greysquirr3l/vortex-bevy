#!/usr/bin/env bash
# Validates every `uses:` step in .github/workflows against the action's own
# action.yml at the pinned commit SHA.
#
# actionlint checks workflow syntax and local action metadata, but it does not
# resolve remote actions, so it cannot tell that a `with:` key names an input
# the action does not have, or that a required input was never supplied. Both
# mistakes are silent at lint time and fail only when the job runs.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

readonly workflow_root="${VORTEX_WORKFLOW_ROOT:-.github/workflows}"
# The cache key is `owner-name-sha-subpath.yml`, so a pre-populated cache
# directory lets the check run without network access.
readonly cache_dir="${VORTEX_WORKFLOW_ACTION_CACHE:-${TMPDIR:-/tmp}/vortex-workflow-action-cache}"

require_command() {
  local command_name="$1"
  if ! command -v "${command_name}" >/dev/null 2>&1; then
    printf 'Required command is unavailable: %s\n' "${command_name}" >&2
    exit 1
  fi
}

# Emits `uses<TAB>input1,input2` for every action step in a workflow file.
# A step's `with:` keys are the lines indented exactly two spaces past the
# `with:` key itself; anything at a shallower indent ends the block.
collect_action_steps() {
  tr -d '\r' < "$1" | awk '
    function flush() {
      if (uses != "") {
        printf "%s\t%s\n", uses, (inputs == "" ? "-" : inputs)
      }
      uses = ""
      inputs = ""
      with_indent = -1
    }
    /^[ ]*-?[ ]*uses:[ ]/ {
      flush()
      value = $0
      sub(/^[ ]*-?[ ]*uses:[ ]*/, "", value)
      sub(/[ ]*#.*$/, "", value)
      sub(/[ ]+$/, "", value)
      uses = value
      next
    }
    /^[ ]*-?[ ]*with:[ ]*$/ {
      if (uses != "") {
        match($0, /^[ ]*/)
        with_indent = RLENGTH
      }
      next
    }
    {
      if (uses != "" && with_indent >= 0) {
        match($0, /^[ ]*/)
        indent = RLENGTH
        if (indent == with_indent + 2) {
          key = substr($0, indent + 1)
          sub(/:.*$/, "", key)
          if (key ~ /^[A-Za-z0-9_.-]+$/) {
            inputs = (inputs == "" ? key : inputs "," key)
          }
          next
        }
        if (indent <= with_indent) {
          with_indent = -1
        }
      }
    }
    END { flush() }
  '
}

# Emits `name<TAB>required|optional` per declared input of an action.yml.
# An input is required only when the action marks it required and gives it no
# default, because GitHub Actions resolves defaults before the step runs.
collect_declared_inputs() {
  tr -d '\r' < "$1" | awk '
    function flush() {
      if (name != "") {
        printf "%s\t%s\n", name, ((is_required && !has_default) ? "required" : "optional")
      }
      name = ""
      is_required = 0
      has_default = 0
    }
    /^inputs:[ ]*$/ { in_inputs = 1; next }
    {
      if (!in_inputs) { next }
      if (/^[A-Za-z_]/) {
        flush()
        in_inputs = 0
        next
      }
      if (/^[ ]+[A-Za-z0-9_.-]+:[ ]*$/) {
        flush()
        name = $0
        sub(/^[ ]+/, "", name)
        sub(/:$/, "", name)
        next
      }
      if (name != "") {
        if (/^[ ]+required:[ ]*true/) { is_required = 1 }
        if (/^[ ]+default:/) { has_default = 1 }
      }
    }
    END { flush() }
  '
}

# Prints the cached action.yml path for an action reference, fetching it once.
fetch_action_manifest() {
  local owner="$1"
  local name="$2"
  local sha="$3"
  local subpath="$4"
  local url="https://raw.githubusercontent.com/${owner}/${name}/${sha}/${subpath:+$subpath/}action.yml"
  local cached="${cache_dir}/${owner}-${name}-${sha}-${subpath//\//-}.yml"

  mkdir -p "${cache_dir}"
  if [[ -s "${cached}" ]]; then
    printf '%s' "${cached}"
    return 0
  fi

  if ! curl --fail --silent --show-error --location --retry 3 --retry-delay 2 \
    --output "${cached}.tmp" "${url}"; then
    rm -f "${cached}.tmp"
    printf 'Unable to fetch action manifest: %s\n' "${url}" >&2
    return 1
  fi
  mv "${cached}.tmp" "${cached}"
  printf '%s' "${cached}"
}

# Prints the input names declared by an action, one per line.
declared_inputs_of() {
  collect_declared_inputs "$1" | cut -f1
}

# Prints the required input names of an action, one per line.
required_inputs_of() {
  collect_declared_inputs "$1" | awk -F '\t' '$2 == "required" { print $1 }'
}

main() {
  require_command curl
  require_command awk
  require_command tr

  local failures=0
  local workflow uses supplied_csv owner name path subpath sha manifest
  local candidate
  local -a supplied unknown missing

  for workflow in "${workflow_root}"/*.yml; do
    [[ -e "${workflow}" ]] || continue
    while IFS=$'\t' read -r uses supplied_csv; do
      [[ -n "${uses}" ]] || continue
      case "${uses}" in
        ./* | docker://*) continue ;;
      esac
      if [[ "${uses}" != *@* ]]; then
        printf '%s: action reference is not pinned to a commit SHA: %s\n' \
          "${workflow}" "${uses}" >&2
        failures=$((failures + 1))
        continue
      fi

      # `uses` is `owner/name[/subpath]@sha`; split the SHA off first so the
      # path segments can be read positionally.
      sha="${uses##*@}"
      path="${uses%@*}"
      owner="${path%%/*}"
      path="${path#*/}"
      name="${path%%/*}"
      path="${path#*/}"
      if [[ "${path}" == "${name}" || -z "${path}" ]]; then
        subpath=""
      else
        subpath="${path}"
      fi

      manifest="$(fetch_action_manifest "${owner}" "${name}" "${sha}" "${subpath}")"

      supplied=()
      if [[ "${supplied_csv}" != "-" ]]; then
        IFS=',' read -r -a supplied <<< "${supplied_csv}"
      fi

      unknown=()
      for candidate in "${supplied[@]:-}"; do
        [[ -n "${candidate}" ]] || continue
        declared_inputs_of "${manifest}" | grep -qx -- "${candidate}" \
          || unknown+=("${candidate}")
      done

      missing=()
      while IFS= read -r candidate; do
        [[ -n "${candidate}" ]] || continue
        printf '%s\n' "${supplied[@]:-}" | grep -qx -- "${candidate}" \
          || missing+=("${candidate}")
      done < <(required_inputs_of "${manifest}")

      if (( ${#unknown[@]} > 0 )); then
        printf '%s: action %s does not declare input(s): %s\n' \
          "${workflow}" "${uses}" "${unknown[*]}" >&2
        failures=$((failures + 1))
      fi
      if (( ${#missing[@]} > 0 )); then
        printf '%s: action %s is missing required input(s): %s\n' \
          "${workflow}" "${uses}" "${missing[*]}" >&2
        failures=$((failures + 1))
      fi
    done < <(collect_action_steps "${workflow}")
  done

  if (( failures > 0 )); then
    printf 'Workflow action contract failed with %d problem(s).\n' "${failures}" >&2
    exit 1
  fi
  printf 'Workflow action contract passed.\n'
}

main "$@"
