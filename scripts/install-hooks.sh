#!/usr/bin/env bash
set -euo pipefail

readonly hooks_dir=".git/hooks"
mkdir -p "${hooks_dir}"

cat > "${hooks_dir}/pre-commit" <<'HOOK'
#!/usr/bin/env bash
set -euo pipefail
cargo fmt --all -- --check
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
cargo test --workspace --all-features
cargo l
bash scripts/check-bridge-quality.sh
cargo build --workspace --release --all-targets --all-features
cargo doc --workspace --no-deps
HOOK

chmod +x "${hooks_dir}/pre-commit" "${hooks_dir}/commit-msg" "${hooks_dir}/pre-push"
printf 'Installed Vortex-Bevy Git hooks.\n'
