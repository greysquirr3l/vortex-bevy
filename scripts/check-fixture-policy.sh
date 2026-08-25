#!/usr/bin/env bash
set -euo pipefail

readonly fixture_root="fixtures"
readonly max_bytes=5242880

if [[ ! -d "${fixture_root}" ]]; then
  printf 'Fixture root %s is absent; nothing to validate.\n' "${fixture_root}"
  exit 0
fi

while IFS= read -r -d '' file; do
  case "${file}" in
    *.pak|*.utoc|*.ucas|*.uexp|*.ubulk|*.uasset|*.key|*.pem|*.p12|*.pfx)
      printf 'Prohibited fixture format: %s\n' "${file}" >&2
      exit 1
      ;;
  esac

  size=$(wc -c < "${file}")
  if (( size > max_bytes )); then
    printf 'Fixture exceeds %s bytes: %s\n' "${max_bytes}" "${file}" >&2
    exit 1
  fi
done < <(find "${fixture_root}" -type f -print0)

printf 'Fixture policy passed.\n'
