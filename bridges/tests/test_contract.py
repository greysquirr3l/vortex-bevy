"""Deterministic contract checks for the three authorized Vortex Editor bridges."""

from __future__ import annotations

import copy
import json
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator

ROOT = Path(__file__).resolve().parents[1]
PROTOCOL_VERSION = "vortex.editor-bridge/v1"
STATIC_SCENE_PROTOCOL_VERSION = "vortex.unity-static-scene/v1"
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
        ROOT / "unity" / "Editor" / "VortexUnityStaticSceneBridge.cs": (
            "VORTEX_UNITY_STATIC_SCENE_STAGING_ROOT",
            "EditorSceneManager.NewScene",
            "GameObject.CreatePrimitive",
            "CleanupTemporaryAssets",
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


def test_static_scene_report_contract_is_strict_and_confined() -> None:
    """Validate the owned real-Editor static-scene protocol fixture and failure boundaries."""
    report = _static_scene_report()
    schema = _read_json(ROOT / "contract" / "vortex-unity-static-scene-v1.schema.json")
    validator = Draft202012Validator(schema)
    errors = sorted(validator.iter_errors(report), key=lambda error: list(error.path))
    assert not errors, errors[0].message
    assert report["protocol_version"] == STATIC_SCENE_PROTOCOL_VERSION
    assert report["scene"]["source_path"].startswith("Assets/")
    assert report["scene"]["material"]["base_color_texture"].startswith("textures/")
    assert report["unsupported_components"][0]["classification"] == "requires_adapter"

    escaped = copy.deepcopy(report)
    escaped["scene"]["material"]["base_color_texture"] = "../outside.png"
    assert list(validator.iter_errors(escaped))

    unexpected = copy.deepcopy(report)
    unexpected["scene"]["unexpected"] = True
    assert list(validator.iter_errors(unexpected))


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


def _static_scene_report() -> dict[str, Any]:
    """Return an owned, minimal report for the bounded static-scene protocol."""
    return {
        "engine": "unity",
        "protocol_version": STATIC_SCENE_PROTOCOL_VERSION,
        "editor_version": "6000.5.10f1",
        "urp_package_version": "17.5.0",
        "scene": {
            "source_guid": "0123456789abcdef0123456789abcdef",
            "source_path": "Assets/VortexBridgeTemporary/StaticScene.unity",
            "object_id": 1001,
            "mesh": {
                "positions": [
                    {"x": -0.5, "y": 0.0, "z": 0.0},
                    {"x": 0.5, "y": 0.0, "z": 0.0},
                    {"x": 0.0, "y": 1.0, "z": 0.0},
                ],
                "normals": [
                    {"x": 0.0, "y": 0.0, "z": 1.0},
                    {"x": 0.0, "y": 0.0, "z": 1.0},
                    {"x": 0.0, "y": 0.0, "z": 1.0},
                ],
                "uvs": [
                    {"x": 0.0, "y": 0.0},
                    {"x": 1.0, "y": 0.0},
                    {"x": 0.5, "y": 1.0},
                ],
                "indices": [0, 1, 2],
            },
            "material": {
                "shader": "Universal Render Pipeline/Lit",
                "base_color": {"r": 0.8, "g": 0.4, "b": 0.2, "a": 1.0},
                "metallic": 0.35,
                "smoothness": 0.6,
                "base_color_texture": "textures/vortex-static-base-color.png",
            },
            "camera": {
                "position": {"x": 0.0, "y": 1.0, "z": -5.0},
                "rotation": {"x": 0.0, "y": 0.0, "z": 0.0, "w": 1.0},
                "scale": {"x": 1.0, "y": 1.0, "z": 1.0},
                "vertical_fov_radians": 1.0471976,
            },
            "light": {
                "position": {"x": 0.0, "y": 3.0, "z": -1.0},
                "rotation": {"x": 0.0, "y": 0.0, "z": 0.0, "w": 1.0},
                "scale": {"x": 1.0, "y": 1.0, "z": 1.0},
                "kind": "directional",
                "intensity": 1.0,
            },
        },
        "unsupported_components": [
            {
                "component": "BoxCollider",
                "classification": "requires_adapter",
                "reason": (
                    "BoxCollider physics shape is outside this render-scene conversion subset."
                ),
            }
        ],
    }


def _read_json(path: Path) -> dict[str, Any]:
    """Read an owned JSON fixture with an assertion on malformed root data."""
    with path.open(encoding="utf-8") as fixture:
        loaded = json.load(fixture)
    assert isinstance(loaded, dict)
    return loaded
