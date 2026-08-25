#!/usr/bin/env bash
set -euo pipefail

if (( $# != 1 )); then
  printf 'Usage: %s <commit-message-file>\n' "$0" >&2
  exit 2
fi

readonly message_file="$1"
readonly subject=$(head -n 1 "${message_file}")
readonly pattern='^(build|chore|ci|docs|feat|fix|perf|refactor|revert|security|test)(\([a-z0-9][a-z0-9-]*\))?!?: .+$'

if [[ ! "${subject}" =~ ${pattern} ]]; then
  printf 'Commit subject is not Conventional Commits compliant: %s\n' "${subject}" >&2
  exit 1
fi

if (( ${#subject} > 72 )); then
  printf 'Commit subject exceeds 72 characters: %s\n' "${subject}" >&2
  exit 1
fi
