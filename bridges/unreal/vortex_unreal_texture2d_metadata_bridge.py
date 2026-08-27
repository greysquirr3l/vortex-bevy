"""Authorized non-mutating UE 5.8.2 UTexture2D metadata oracle bridge.

Run only through an authorized Unreal Engine 5.8.2 Editor. The bridge queries one
explicit on-disk Asset Registry record, emits a confined JSON observation, never loads
the asset, and never writes project content. It is an oracle for the bounded native
metadata decoder; it is not an exporter.
"""

from __future__ import annotations

import json
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import unreal

PROTOCOL_VERSION = "vortex.unreal-texture-metadata/v1"
EXPECTED_ENGINE_VERSION = "5.8.2"
SELECTED_TAGS = (
    "CompressionSettings",
    "Dimensions",
    "Filter",
    "PixelFormat",
    "SourceFileHash",
)


@dataclass(frozen=True)
class BridgeRequest:
    """A validated explicit input and confined external output location."""

    asset_path: str
    report_path: Path
    staging_root: Path


def _normalized(path: Path) -> Path:
    """Return an absolute normalized path without requiring it to exist."""
    return path.expanduser().resolve(strict=False)


def _confined(path: Path, root: Path) -> bool:
    """Return whether ``path`` is located at or below ``root``."""
    try:
        _normalized(path).relative_to(_normalized(root))
    except ValueError:
        return False
    return True


def _command_value(name: str) -> str | None:
    """Return the explicit value following a bridge command-line argument."""
    arguments = list(sys.argv)
    for index, argument in enumerate(arguments):
        if argument == name and index + 1 < len(arguments):
            return arguments[index + 1]
    return None


def _parse_request() -> BridgeRequest:
    """Parse one explicit Unreal object path and one staging-confined report path."""
    raw_asset = _command_value("--asset")
    raw_report = _command_value("--report")
    raw_staging = _command_value("--staging-root")
    if raw_asset is None or raw_report is None or raw_staging is None:
        message = "Expected --asset, --report, and --staging-root arguments."
        raise ValueError(message)
    asset_path = raw_asset.strip()
    if not asset_path.startswith("/Game/") or "." not in asset_path:
        message = "Asset must be one explicit /Game/ Unreal object path."
        raise ValueError(message)
    staging_root = _normalized(Path(raw_staging))
    report_path = _normalized(Path(raw_report))
    if not _confined(report_path, staging_root):
        message = "Report path must be confined to staging root."
        raise ValueError(message)
    return BridgeRequest(asset_path, report_path, staging_root)


def _require_engine_version() -> str:
    """Return the exact UE 5.8.2 version or reject an unsupported Editor."""
    version = unreal.SystemLibrary.get_engine_version()
    if not version.startswith(EXPECTED_ENGINE_VERSION):
        message = f"Expected Unreal Engine {EXPECTED_ENGINE_VERSION}, received {version}."
        raise ValueError(message)
    return EXPECTED_ENGINE_VERSION


def _asset_data(request: BridgeRequest) -> unreal.AssetData:
    """Return exactly one on-disk Asset Registry record without loading its UObject."""
    registry = unreal.AssetRegistryHelpers.get_asset_registry()
    asset_data = registry.k2_get_asset_by_object_path(
        unreal.SoftObjectPath(request.asset_path),
        include_only_on_disk_assets=True,
        skip_ar_filtered_assets=True,
    )
    if not asset_data.is_valid():
        message = f"Asset Registry did not find {request.asset_path} on disk."
        raise ValueError(message)
    if asset_data.is_cooked():
        message = "Cooked Asset Registry records are outside the T26 metadata subset."
        raise ValueError(message)
    return asset_data


def _asset_kind(asset_data: unreal.AssetData) -> str:
    """Return the short native class name without loading the selected asset."""
    class_path = str(asset_data.asset_class_path)
    return class_path.rsplit(".", maxsplit=1)[-1]


def _tags(asset_data: unreal.AssetData) -> dict[str, str]:
    """Collect only the explicitly selected stable Asset Registry tags."""
    tags: dict[str, str] = {}
    for name in SELECTED_TAGS:
        value = asset_data.get_tag_value(name)
        if value is not None:
            tags[name] = value
    return tags


def _report(request: BridgeRequest) -> dict[str, Any]:
    """Build one deterministic non-mutating texture metadata observation."""
    version = _require_engine_version()
    asset_data = _asset_data(request)
    asset_kind = _asset_kind(asset_data)
    if asset_kind != "Texture2D":
        message = f"Expected Texture2D Asset Registry class, received {asset_kind}."
        raise ValueError(message)
    return {
        "engine": "unreal",
        "engine_version": version,
        "protocol_version": PROTOCOL_VERSION,
        "records": [
            {
                "asset_kind": asset_kind,
                "cooked": False,
                "object_path": request.asset_path,
                "tags": _tags(asset_data),
            }
        ],
    }


def _write_report(report_path: Path, report: dict[str, Any]) -> None:
    """Atomically write one deterministic report inside the caller-owned staging root."""
    report_path.parent.mkdir(parents=True, exist_ok=True)
    temporary = report_path.with_suffix(report_path.suffix + ".tmp")
    if not _confined(temporary, report_path.parent):
        message = "Temporary report path escaped report parent."
        raise ValueError(message)
    temporary.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    temporary.replace(report_path)


def main() -> None:
    """Run the selected query and report any rejection through the Unreal log."""
    try:
        request = _parse_request()
        _write_report(request.report_path, _report(request))
    except ValueError as error:
        unreal.log_error(f"Vortex Unreal texture metadata bridge rejected input: {error}")
        return
    unreal.log(f"Vortex Unreal texture metadata report: {request.report_path}")


if __name__ == "__main__":
    main()
