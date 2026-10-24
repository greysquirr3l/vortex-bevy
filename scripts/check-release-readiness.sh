#!/usr/bin/env bash
set -euo pipefail

if [[ -n "$(git status --short)" ]]; then
  printf '%s\n' 'release readiness requires a clean working tree' >&2
  exit 1
fi

git diff --check
cargo fmt --all -- --check
cargo l
cargo build --workspace --all-targets --all-features
cargo test --workspace --all-targets --all-features
bash scripts/check-bridge-quality.sh
bash scripts/check-documentation.sh
bash scripts/check-fixture-policy.sh
bash scripts/check-workflow-actions.sh
bash scripts/test-workflow-actions.sh
bash scripts/verify-fixtures.sh
cargo deny --all-features check
cargo audit --no-fetch
cargo build --workspace --all-targets --all-features --release

printf '%s\n' 'release readiness checks passed'
