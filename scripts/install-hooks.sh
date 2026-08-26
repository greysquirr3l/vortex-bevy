#!/usr/bin/env bash
set -euo pipefail

readonly hooks_dir=".git/hooks"
mkdir -p "${hooks_dir}"

# .gitattributes assigns this driver to generated task contracts and pinned
# bridge tooling.  It keeps the checked-out branch's reviewed file on merges.
git config merge.vortex-preserve.name "Preserve Vortex project contracts"
git config merge.vortex-preserve.driver true

cat > "${hooks_dir}/pre-commit" <<'HOOK'
#!/usr/bin/env bash
set -euo pipefail
cargo fmt --all -- --check
bash scripts/check-preserved-contracts.sh
if command -v gitleaks >/dev/null 2>&1; then
  gitleaks git --config .gitleaks.toml
fi
if command -v cargo-audit >/dev/null 2>&1; then
  cargo audit
fi
HOOK

cat > "${hooks_dir}/commit-msg" <<'HOOK'
#!/usr/bin/env bash
set -euo pipefail
exec bash scripts/check-conventional-commit.sh "$1"
HOOK

cat > "${hooks_dir}/pre-push" <<'HOOK'
#!/usr/bin/env bash
set -euo pipefail
bash scripts/check-preserved-contracts.sh
cargo test --workspace --all-features
cargo l
bash scripts/check-bridge-quality.sh
cargo build --workspace --release --all-targets --all-features
cargo doc --workspace --no-deps
HOOK

chmod +x "${hooks_dir}/pre-commit" "${hooks_dir}/commit-msg" "${hooks_dir}/pre-push"
printf 'Installed Vortex-Bevy Git hooks and merge drivers.\n'
