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

require_command ruff
require_command pytest
require_command gdlint
require_command gdformat
require_command dotnet

ruff check --config "${bridge_root}/ruff.toml" "${bridge_root}/unreal" "${bridge_root}/tests"
ruff format --check "${bridge_root}/unreal" "${bridge_root}/tests"
pytest "${bridge_root}/tests"
gdlint "${godot_bridge}"
gdformat --check "${godot_bridge}"
dotnet tool restore
dotnet csharpier check "${unity_bridge}" "${unity_static_scene_bridge}" "${unity_lint_stubs}"
dotnet build "${unity_lint_project}" --nologo --warnaserror
reject_markers
