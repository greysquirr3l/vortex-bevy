#!/usr/bin/env bash
set -euo pipefail

readonly bridge_root="bridges"
readonly unity_bridge="${bridge_root}/unity/Editor/VortexUnityBridge.cs"
readonly unity_static_scene_bridge="${bridge_root}/unity/Editor/VortexUnityStaticSceneBridge.cs"
readonly unity_lint_project="${bridge_root}/unity/Vortex.UnityBridge.Lint.csproj"
readonly unity_lint_stubs="${bridge_root}/unity/lint/UnityApiStubs.cs"
readonly godot_bridge="${bridge_root}/godot/vortex_godot_bridge.gd"

require_command() {
  local command_name="$1"
  if ! command -v "${command_name}" >/dev/null 2>&1; then
    printf 'Required bridge quality command is unavailable: %s\n' "${command_name}" >&2
    exit 1
  fi
}

reject_markers() {
  local pattern='TODO|FIXME|HACK|XXX'
  if grep --recursive --line-number --extended-regexp \
    --include='*.py' --include='*.cs' --include='*.gd' "${pattern}" "${bridge_root}"; then
    printf 'Unresolved bridge quality markers are forbidden.\n' >&2
    exit 1
  fi
}

command_present() {
  command -v "$1" >/dev/null 2>&1
}

require_command ruff
require_command pytest

ruff check --config "${bridge_root}/ruff.toml" "${bridge_root}/unreal" "${bridge_root}/tests"
ruff format --check "${bridge_root}/unreal" "${bridge_root}/tests"
pytest "${bridge_root}/tests"
reject_markers

# Optional: Godot bridge lint + format via gdtoolkit (gdlint, gdformat).
# The toolchain is host-specific (pip install gdtoolkit) and not every
# contributor has Python tooling on PATH. When absent, surface a warning
# and skip so the pre-push hook does not wedge local development.
run_godot_lint() {
  if ! command_present gdlint || ! command_present gdformat; then
    printf 'gdlint/gdformat not installed; skipping Godot bridge lint/format. Restore via `pip install gdtoolkit` once.\n' >&2
    return 0
  fi
  gdlint "${godot_bridge}"
  gdformat --check "${godot_bridge}"
}

run_godot_lint

# Optional: Unity C# lint + build via dotnet-csharpier. The Unity bridge
# tooling is host-specific (requires the .NET SDK plus the pinned
# csharpier global tool). On hosts without dotnet or csharpier installed,
# surface a warning and skip rather than aborting the pre-push hook.
run_unity_lint() {
  if ! command_present dotnet; then
    printf 'dotnet not installed; skipping Unity bridge lint/build.\n' >&2
    return 0
  fi
  if ! dotnet csharpier --version >/dev/null 2>&1; then
    printf 'dotnet csharpier not installed; skipping Unity bridge lint/build. Restore via `dotnet tool restore` once.\n' >&2
    return 0
  fi
  dotnet csharpier check "${unity_bridge}" "${unity_static_scene_bridge}" "${unity_lint_stubs}"
  dotnet build "${unity_lint_project}" --nologo --warnaserror
}

run_unity_lint
