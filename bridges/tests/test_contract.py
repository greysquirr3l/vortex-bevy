"""Deterministic contract checks for the three authorized Vortex Editor bridges."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator

ROOT = Path(__file__).resolve().parents[1]
PROTOCOL_VERSION = "vortex.editor-bridge/v1"
CLASSIFICATIONS = frozenset(
    {"converted", "baked", "requires_adapter", "unsupported", "rejected"},
)
ENGINES = frozenset({"unreal", "unity", "godot"})


def test_fixture_reports_match_the_shared_contract() -> None:
    """Ensure every synthetic engine report is structurally conformant and confined."""
    for fixture_path in sorted((ROOT / "fixtures").glob("*-report.json")):
        report = _read_json(fixture_path)
        assert report["engine"] in ENGINES
        assert report["protocol_version"] == PROTOCOL_VERSION
        assert report["records"]
        _assert_schema_valid(report)
        for record in report["records"]:
            _assert_record(record, report["engine"])


def test_schema_declares_the_current_protocol_and_engine_set() -> None:
    """Ensure the published schema encodes the same strict protocol choices."""
    schema = _read_json(ROOT / "contract" / "vortex-editor-bridge-v1.schema.json")
    assert schema["properties"]["protocol_version"]["const"] == PROTOCOL_VERSION
    assert set(schema["properties"]["engine"]["enum"]) == ENGINES


def test_bridge_sources_keep_the_required_authorization_boundaries() -> None:
    """Ensure bridges use explicit input selection and confined staging primitives."""
    required_markers = {
        ROOT / "unreal" / "vortex_unreal_bridge.py": (
            "unreal.AssetExportTask()",
            "task.automated = True",
            "task.prompt = False",
            "_confined",
        ),
        ROOT / "unity" / "Editor" / "VortexUnityBridge.cs": (
            "AssetDatabase.GUIDToAssetPath",
            "VORTEX_UNITY_GUIDS",
            "ConfinedPath",
            "EditorApplication.Exit",
        ),
        ROOT / "godot" / "vortex_godot_bridge.gd": (
            "OS.get_cmdline_user_args()",
            "ResourceLoader.load",
            "_is_confined",
            "--staging-root",
        ),
    }
    for path, markers in required_markers.items():
        source = path.read_text(encoding="utf-8")
        for marker in markers:
            assert marker in source, f"{path} omitted required marker {marker!r}"


def _assert_schema_valid(report: dict[str, Any]) -> None:
    """Validate an owned bridge report against the full versioned JSON Schema."""
    schema = _read_json(ROOT / "contract" / "vortex-editor-bridge-v1.schema.json")
    validator = Draft202012Validator(schema)
    errors = sorted(validator.iter_errors(report), key=lambda error: list(error.path))
    assert not errors, errors[0].message


def _assert_record(record: dict[str, Any], engine: str) -> None:
    """Assert mandatory shared-record fields and relative output confinement."""
    required_fields = {
        "asset_kind",
        "classification",
        "errors",
        "exporter",
        "outputs",
        "source_identity",
        "warnings",
    }
    assert set(record) == required_fields
    assert record["classification"] in CLASSIFICATIONS
    assert record["source_identity"]["engine"] == engine
    assert record["source_identity"]["id"]
    for output in record["outputs"]:
        assert not Path(output).is_absolute()
        assert ".." not in Path(output).parts


def _read_json(path: Path) -> dict[str, Any]:
    """Read an owned JSON fixture with an assertion on malformed root data."""
    with path.open(encoding="utf-8") as fixture:
        loaded = json.load(fixture)
    assert isinstance(loaded, dict)
    return loaded
