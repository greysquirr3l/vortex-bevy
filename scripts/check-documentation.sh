#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

check_local_links() {
  local document link target directory
  while IFS= read -r -d '' document; do
    # Only tracked documents are scanned. A gitignored file is absent from a
    # clean checkout, so its links cannot be validated here or in CI.
    git ls-files --error-unmatch -- "$document" >/dev/null 2>&1 || continue
    directory="$(dirname "$document")"
    while IFS= read -r link; do
      target="${link%%#*}"
      case "$target" in
        '' | http://* | https://* | mailto:*) continue ;;
      esac
      if [[ ! -e "$directory/$target" ]]; then
        printf 'Broken local Markdown link in %s: %s\n' "$document" "$link" >&2
        return 1
      fi
      # The target must be tracked by git. A gitignored file can be present on
      # a developer machine and absent from a clean checkout, so a link to one
      # passes here and breaks in CI.
      if ! git ls-files --error-unmatch -- "$directory/$target" >/dev/null 2>&1; then
        printf 'Local Markdown link in %s targets an untracked file: %s\n' \
          "$document" "$link" >&2
        return 1
      fi
    done < <(grep -oE '\]\([^)]*\)' "$document" | sed -E 's/^\]\(//; s/\)$//' || true)
  done < <(find README.md docs fixtures bridges book/src -type f -name '*.md' -print0)
}

run_documented_cli_examples() {
  local binary="${CARGO_TARGET_DIR:-$ROOT/target}/debug/vortex-bevy"
  cargo build -q -p vortex-bevy
  "$binary" --help >/dev/null
  "$binary" version --format json >/dev/null
  "$binary" support-matrix --format json >/dev/null
  "$binary" inspect --help >/dev/null
  "$binary" plan --help >/dev/null
  "$binary" convert --help >/dev/null
  "$binary" verify --help >/dev/null
  "$binary" view --help >/dev/null
}

check_local_links
run_documented_cli_examples
cargo doc --workspace --no-deps

if command -v mdbook >/dev/null 2>&1; then
  mdbook build book/
  if ! mdbook test book/; then
    printf 'mdbook test reported failures.\n' >&2
    exit 1
  fi
else
  printf 'mdbook not installed; skipping user-guide build (cargo install mdbook --locked).\n' >&2
fi

printf '%s\n' 'Documentation contract passed.'
