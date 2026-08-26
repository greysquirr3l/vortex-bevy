"""Authorized Unreal Editor export bridge for Vortex-Bevy.

Run this module only through a matching, authorized Unreal Editor installation. The
bridge exports explicit requested assets into a caller-provided staging directory and
emits the shared Vortex Editor Bridge Report v1. It never discovers or executes
project-provided scripts.
"""

from __future__ import annotations

import json
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import unreal

PROTOCOL_VERSION = "vortex.editor-bridge/v1"
ALLOWED_EXPORT_SUFFIXES = frozenset({".fbx", ".gltf", ".glb", ".png", ".tga", ".exr"})


@dataclass(frozen=True)
class BridgeRequest:
    """A validated bridge request received from Unreal command-line arguments."""

    asset_paths: tuple[str, ...]
    report_path: Path
    staging_root: Path


@dataclass(frozen=True)
class RecordDetails:
    """The variable collection fields for one shared bridge report record."""

    outputs: tuple[str, ...] = ()
    warnings: tuple[str, ...] = ()
    errors: tuple[str, ...] = ()


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
    """Return the explicit value following a named command-line argument."""
    arguments = list(sys.argv)
    for index, argument in enumerate(arguments):
        if argument == name and index + 1 < len(arguments):
            return arguments[index + 1]
    return None


def _parse_request() -> BridgeRequest:
    """Parse and confine explicit command-line bridge inputs.

    Raises:
        ValueError: If arguments are incomplete, unsafe, or semantically invalid.
    """
    raw_assets = _command_value("--assets")
    raw_report = _command_value("--report")
    raw_staging = _command_value("--staging-root")
    if raw_assets is None or raw_report is None or raw_staging is None:
        message = "Expected --assets, --report, and --staging-root arguments."
        raise ValueError(message)

    staging_root = _normalized(Path(raw_staging))
    report_path = _normalized(Path(raw_report))
    if not _confined(report_path, staging_root):
        message = "Report path must be confined to staging root."
        raise ValueError(message)

    asset_paths = tuple(sorted({path.strip() for path in raw_assets.split(",") if path.strip()}))
    if not asset_paths or any(not path.startswith("/") for path in asset_paths):
        message = "Assets must be one or more Unreal object paths beginning with '/'."
        raise ValueError(message)
    return BridgeRequest(asset_paths, report_path, staging_root)


def _safe_output_path(staging_root: Path, asset_path: str, suffix: str) -> Path:
    """Derive a deterministic and confined output path for one Unreal object path."""
    if suffix.casefold() not in ALLOWED_EXPORT_SUFFIXES:
        message = f"Unsupported exporter suffix: {suffix}"
        raise ValueError(message)
    safe_stem = asset_path.strip("/").replace("/", "_").replace(".", "_")
    output_path = staging_root / "exports" / f"{safe_stem}{suffix.casefold()}"
    if not _confined(output_path, staging_root):
        message = "Derived export path escaped staging root."
        raise ValueError(message)
    return output_path


def _asset_kind(asset: unreal.Object) -> str:
    """Return the stable Unreal class name for report classification."""
    return str(asset.get_class().get_name())


def _select_exporter(asset: unreal.Object) -> tuple[unreal.Exporter | None, str, str | None]:
    """Select a conservative supported exporter and suffix for an Unreal asset."""
    asset_class = _asset_kind(asset)
    if asset_class in {"StaticMesh", "SkeletalMesh"}:
        return unreal.FbxExporter(), ".fbx", None
    if asset_class == "Texture2D":
        return unreal.TextureExporterTGA(), ".tga", None
    return None, "", f"Unsupported authoritative export class: {asset_class}"


def _record(
    asset_path: str,
    asset_kind: str,
    classification: str,
    exporter: str,
    details: RecordDetails,
) -> dict[str, Any]:
    """Build one deterministic shared-protocol report record."""
    return {
        "asset_kind": asset_kind,
        "classification": classification,
        "errors": sorted(details.errors),
        "exporter": exporter,
        "outputs": sorted(details.outputs),
        "source_identity": {"engine": "unreal", "id": asset_path},
        "warnings": sorted(details.warnings),
    }


def _export_asset(request: BridgeRequest, asset_path: str) -> dict[str, Any]:
    """Export one explicitly requested, loadable asset through Unreal's exporter API."""
    asset = unreal.EditorAssetLibrary.load_asset(asset_path)
    if asset is None:
        return _record(
            asset_path,
            "Unknown",
            "rejected",
            "none",
            RecordDetails(errors=("Asset could not be loaded.",)),
        )

    exporter, suffix, selection_error = _select_exporter(asset)
    asset_kind = _asset_kind(asset)
    if selection_error is not None or exporter is None:
        warning = selection_error or "No exporter was selected."
        return _record(
            asset_path,
            asset_kind,
            "requires_adapter",
            "none",
            RecordDetails(warnings=(warning,)),
        )

    output_path = _safe_output_path(request.staging_root, asset_path, suffix)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    task = unreal.AssetExportTask()
    task.automated = True
    task.exporter = exporter
    task.filename = str(output_path)
    task.object = asset
    task.prompt = False
    task.replace_identical = True
    succeeded = unreal.Exporter.run_asset_export_task(task)
    errors = tuple(str(error) for error in task.errors)
    if not succeeded or errors:
        return _record(
            asset_path,
            asset_kind,
            "rejected",
            asset_kind,
            RecordDetails(errors=errors or ("Exporter failed.",)),
        )
    relative_output = output_path.relative_to(request.staging_root).as_posix()
    return _record(
        asset_path,
        asset_kind,
        "converted",
        asset_kind,
        RecordDetails(outputs=(relative_output,)),
    )


def _write_report(report_path: Path, records: list[dict[str, Any]]) -> None:
    """Write a stable shared-protocol report inside the validated staging root."""
    report_path.parent.mkdir(parents=True, exist_ok=True)
    report = {
        "engine": "unreal",
        "protocol_version": PROTOCOL_VERSION,
        "records": sorted(records, key=lambda record: str(record["source_identity"]["id"])),
    }
    report_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def main() -> None:
    """Execute the explicitly requested Unreal bridge operation and emit a report."""
    try:
        request = _parse_request()
    except ValueError as error:
        unreal.log_error(f"Vortex Unreal bridge rejected input: {error}")
        return

    records = [_export_asset(request, asset_path) for asset_path in request.asset_paths]
    _write_report(request.report_path, records)
    unreal.log(f"Vortex Unreal bridge wrote {len(records)} record(s).")


if __name__ == "__main__":
    main()
