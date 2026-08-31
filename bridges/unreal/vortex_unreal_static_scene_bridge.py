"""Controlled Unreal static-scene validation bridge for Vortex-Bevy.

This versioned Editor-gated script creates only a refused-if-pre-existing temporary
project folder, exports a duplicated Engine cube and texture through AssetExportTask,
reports the non-portable material-instance and level facts explicitly, then restores
the previously open editable level and deletes its temporary project assets.
"""

from __future__ import annotations

import json
import shlex
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import unreal

PROTOCOL_VERSION = "vortex.editor-bridge/v1"
TEMPORARY_PACKAGE = "/Game/VortexBridgeTemporary"
TEMPORARY_LEVEL = f"{TEMPORARY_PACKAGE}/LVL_VortexStaticScene"
TEMPORARY_MESH = f"{TEMPORARY_PACKAGE}/SM_VortexCube"
TEMPORARY_TEXTURE = f"{TEMPORARY_PACKAGE}/T_VortexBaseColor"
TEMPORARY_MATERIAL = f"{TEMPORARY_PACKAGE}/MI_VortexLit"
ENGINE_CUBE = "/Engine/BasicShapes/Cube.Cube"
ENGINE_TEXTURE = "/Engine/EngineResources/DefaultTexture.DefaultTexture"
ENGINE_MATERIAL = "/Engine/BasicShapes/BasicShapeMaterial.BasicShapeMaterial"
MESH_OUTPUT = "exports/SM_VortexCube.fbx"
TEXTURE_OUTPUT = "exports/T_VortexBaseColor.tga"


@dataclass(frozen=True)
class ScenarioRequest:
    """Validated, caller-owned output paths for one temporary validation."""

    report_path: Path
    staging_root: Path


@dataclass(frozen=True)
class RecordDetails:
    """Variable report fields for one controlled temporary source."""

    outputs: tuple[str, ...] = ()
    warnings: tuple[str, ...] = ()
    errors: tuple[str, ...] = ()


def _normalized(path: Path) -> Path:
    """Return an absolute normalized path without requiring it to exist."""
    return path.expanduser().resolve(strict=False)


def _confined(path: Path, root: Path) -> bool:
    """Return whether a path remains located at or below its approved root."""
    try:
        _normalized(path).relative_to(_normalized(root))
    except ValueError:
        return False
    return True


def _command_value(name: str) -> str | None:
    """Return an explicit argument from Python or the Editor process command line."""
    command_lines = (list(sys.argv), shlex.split(unreal.SystemLibrary.get_command_line()))
    for arguments in command_lines:
        for index, argument in enumerate(arguments):
            if argument == name and index + 1 < len(arguments):
                return arguments[index + 1]
    return None


def _parse_request() -> ScenarioRequest:
    """Parse and confine staging paths supplied by the fixed launcher.

    Raises:
        ValueError: If the required paths are absent or the report escapes staging.
    """
    raw_report = _command_value("--report")
    raw_staging = _command_value("--staging-root")
    if raw_report is None or raw_staging is None:
        message = "Expected --report and --staging-root arguments."
        raise ValueError(message)

    staging_root = _normalized(Path(raw_staging))
    report_path = _normalized(Path(raw_report))
    if not staging_root.is_dir():
        message = "Staging root must already exist as a directory."
        raise ValueError(message)
    if not _confined(report_path, staging_root):
        message = "Report path must remain confined to staging root."
        raise ValueError(message)
    return ScenarioRequest(report_path=report_path, staging_root=staging_root)


def _object_path(package_path: str) -> str:
    """Return the canonical Unreal object path for a single named package asset."""
    asset_name = package_path.rsplit("/", maxsplit=1)[-1]
    return f"{package_path}.{asset_name}"


def _asset_or_error(object_path: str) -> unreal.Object:
    """Load an explicitly named bridge-owned asset or raise a controlled error.

    Raises:
        RuntimeError: If Unreal cannot load the requested bridge-owned asset.
    """
    asset = unreal.EditorAssetLibrary.load_asset(object_path)
    if asset is None:
        message = f"Unable to load bridge-owned asset: {object_path}"
        raise RuntimeError(message)
    return asset


def _require_success(action: str, *, succeeded: bool) -> None:
    """Raise a controlled error when one authoritative Editor action fails.

    Raises:
        RuntimeError: If an Unreal API reports an unsuccessful action.
    """
    if not succeeded:
        message = f"Unreal Editor action failed: {action}"
        raise RuntimeError(message)


def _record(
    asset_path: str,
    asset_kind: str,
    classification: str,
    exporter: str,
    details: RecordDetails,
) -> dict[str, Any]:
    """Build one deterministic shared-protocol record for a controlled source."""
    return {
        "asset_kind": asset_kind,
        "classification": classification,
        "errors": sorted(details.errors),
        "exporter": exporter,
        "outputs": sorted(details.outputs),
        "source_identity": {"engine": "unreal", "id": asset_path},
        "warnings": sorted(details.warnings),
    }


def _export_asset(
    asset: unreal.Object,
    output_path: Path,
    exporter: unreal.Exporter,
    description: str,
) -> None:
    """Run one unattended authoritative export into an approved output path.

    Raises:
        RuntimeError: If Unreal's exporter reports failure or any task errors.
    """
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
        details = "; ".join(errors) if errors else "Exporter returned false."
        message = f"{description} export failed: {details}"
        raise RuntimeError(message)


def _duplicate_engine_asset(source_path: str, destination_path: str) -> unreal.Object:
    """Duplicate one fixed Engine asset into the refused-if-pre-existing folder.

    Raises:
        RuntimeError: If Unreal cannot duplicate or reload the fixed source asset.
    """
    copied = unreal.EditorAssetLibrary.duplicate_asset(source_path, destination_path)
    _require_success(f"duplicate {source_path}", succeeded=copied)
    return _asset_or_error(_object_path(destination_path))


def _create_material_instance() -> unreal.Object:
    """Create the controlled MaterialInstanceConstant assigned to the cube.

    Raises:
        RuntimeError: If the Engine parent material or generated instance is unavailable.
    """
    parent = _asset_or_error(ENGINE_MATERIAL)
    factory = unreal.MaterialInstanceConstantFactoryNew()
    material = unreal.AssetToolsHelpers.get_asset_tools().create_asset(
        "MI_VortexLit",
        TEMPORARY_PACKAGE,
        unreal.MaterialInstanceConstant,
        factory,
    )
    if material is None:
        message = "Unable to create bridge-owned material instance."
        raise RuntimeError(message)
    material.set_editor_property("parent", parent)
    saved = unreal.EditorAssetLibrary.save_loaded_asset(material)
    _require_success("save bridge-owned material instance", succeeded=saved)
    return material


def _spawn_static_scene(mesh: unreal.Object, material: unreal.Object) -> None:
    """Create cube, camera, and directional-light actors in the new level.

    Raises:
        RuntimeError: If any required controlled actor cannot be created or configured.
    """
    cube_actor = unreal.EditorLevelLibrary.spawn_actor_from_class(
        unreal.StaticMeshActor,
        unreal.Vector(0.0, 0.0, 0.0),
        unreal.Rotator(0.0, 0.0, 0.0),
    )
    if cube_actor is None:
        message = "Unable to spawn bridge-owned static-mesh actor."
        raise RuntimeError(message)
    cube_actor.set_actor_label("VortexStaticCube")
    mesh_component = cube_actor.get_editor_property("static_mesh_component")
    mesh_component.set_static_mesh(mesh)
    mesh_component.set_material(0, material)

    camera_actor = unreal.EditorLevelLibrary.spawn_actor_from_class(
        unreal.CameraActor,
        unreal.Vector(450.0, -650.0, 400.0),
        unreal.Rotator(-18.0, 34.0, 0.0),
    )
    if camera_actor is None:
        message = "Unable to spawn bridge-owned camera actor."
        raise RuntimeError(message)
    camera_actor.set_actor_label("VortexValidationCamera")

    light_actor = unreal.EditorLevelLibrary.spawn_actor_from_class(
        unreal.DirectionalLight,
        unreal.Vector(0.0, 0.0, 500.0),
        unreal.Rotator(-45.0, -30.0, 0.0),
    )
    if light_actor is None:
        message = "Unable to spawn bridge-owned directional-light actor."
        raise RuntimeError(message)
    light_actor.set_actor_label("VortexValidationDirectionalLight")


def _assert_temporary_package_absent() -> None:
    """Refuse to proceed when the temporary package is not exclusively bridge-owned.

    Raises:
        RuntimeError: If the fixed temporary package already exists in the project.
    """
    if unreal.EditorAssetLibrary.does_directory_exist(TEMPORARY_PACKAGE):
        message = "Refusing to touch pre-existing temporary package: /Game/VortexBridgeTemporary"
        raise RuntimeError(message)


def _create_scene() -> None:
    """Create and save the controlled static mesh level and its owned assets.

    Raises:
        RuntimeError: If Unreal cannot create the scenario.
    """
    created_level = unreal.EditorLevelLibrary.new_level(TEMPORARY_LEVEL)
    _require_success("create bridge-owned validation level", succeeded=created_level)

    mesh = _duplicate_engine_asset(ENGINE_CUBE, TEMPORARY_MESH)
    _duplicate_engine_asset(ENGINE_TEXTURE, TEMPORARY_TEXTURE)
    material = _create_material_instance()
    _spawn_static_scene(mesh, material)
    saved = unreal.EditorLevelLibrary.save_current_level()
    _require_success("save bridge-owned validation level", succeeded=saved)


def _original_level_path() -> str | None:
    """Return the currently open project level path when it is safe to reload later."""
    world = unreal.EditorLevelLibrary.get_editor_world()
    if world is None:
        return None
    package_name = str(world.get_outermost().get_name())
    if package_name.startswith("/Game/") and package_name != TEMPORARY_LEVEL:
        return package_name
    return None


def _cleanup_scene(original_level: str | None) -> None:
    """Restore the original level then delete only this run's created package.

    Raises:
        RuntimeError: If the owned package cannot be deleted after restoring the editor.
    """
    if original_level is not None:
        restored = unreal.EditorLevelLibrary.load_level(original_level)
        _require_success("restore original project level", succeeded=restored)
    if unreal.EditorAssetLibrary.does_directory_exist(TEMPORARY_PACKAGE):
        unreal.EditorAssetLibrary.delete_directory(TEMPORARY_PACKAGE)
        if unreal.EditorAssetLibrary.does_directory_exist(TEMPORARY_PACKAGE):
            message = "Unreal retained the bridge-owned temporary package after deletion."
            raise RuntimeError(message)


def _scene_records(request: ScenarioRequest) -> list[dict[str, Any]]:
    """Export controlled mesh and texture assets and classify non-portable scene facts.

    Raises:
        RuntimeError: If a controlled authoritative export fails.
    """
    mesh = _asset_or_error(_object_path(TEMPORARY_MESH))
    texture = _asset_or_error(_object_path(TEMPORARY_TEXTURE))
    mesh_output = request.staging_root / MESH_OUTPUT
    texture_output = request.staging_root / TEXTURE_OUTPUT
    _export_asset(mesh, mesh_output, unreal.StaticMeshExporterFBX(), "StaticMesh")
    _export_asset(texture, texture_output, unreal.TextureExporterTGA(), "Texture2D")

    return [
        _record(
            _object_path(TEMPORARY_MESH),
            "StaticMesh",
            "converted",
            "StaticMeshExporterFBX",
            RecordDetails(outputs=(MESH_OUTPUT,)),
        ),
        _record(
            _object_path(TEMPORARY_TEXTURE),
            "Texture2D",
            "converted",
            "TextureExporterTGA",
            RecordDetails(outputs=(TEXTURE_OUTPUT,)),
        ),
        _record(
            _object_path(TEMPORARY_MATERIAL),
            "MaterialInstanceConstant",
            "requires_adapter",
            "none",
            RecordDetails(
                warnings=(
                    "Material-instance parameters are reported explicitly; arbitrary Unreal "
                    "material graphs are outside the portable subset."
                ),
            ),
        ),
        _record(
            _object_path(TEMPORARY_LEVEL),
            "World",
            "requires_adapter",
            "none",
            RecordDetails(
                warnings=(
                    "Static level actor placement is evidence-only in bridge v1; full world "
                    "conversion requires a dedicated scene adapter."
                ),
            ),
        ),
    ]


def _write_report(report_path: Path, records: list[dict[str, Any]]) -> None:
    """Write a deterministic shared bridge report inside the caller's staging root."""
    report_path.parent.mkdir(parents=True, exist_ok=True)
    report = {
        "engine": "unreal",
        "protocol_version": PROTOCOL_VERSION,
        "records": sorted(records, key=lambda record: str(record["source_identity"]["id"])),
    }
    report_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def main() -> None:
    """Execute the bounded static-scene validation and always clean its owned package."""
    request = _parse_request()
    _assert_temporary_package_absent()
    original_level = _original_level_path()
    try:
        _create_scene()
        _write_report(request.report_path, _scene_records(request))
        unreal.log("Vortex Unreal static-scene bridge completed four report records.")
    finally:
        _cleanup_scene(original_level)


if __name__ == "__main__":
    try:
        main()
    except (RuntimeError, ValueError) as error:
        unreal.log_error(f"Vortex Unreal static-scene bridge failed: {error}")
        raise
