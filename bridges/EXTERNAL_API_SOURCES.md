# Editor Bridge API Sources

The bridge implementations are confined to authorized, installed Editor APIs. These references informed their supported automation boundaries.

| Engine | Official source | Applied contract decision |
|---|---|---|
| Unreal Engine | [AssetExportTask Python API](https://dev.epicgames.com/documentation/en-us/unreal-engine/python-api/class/AssetExportTask) | Uses an unattended `AssetExportTask` with explicit asset object, output filename, selected exporter, disabled prompts, and surfaced exporter errors. |
| Unity | [AssetDatabase scripting API](https://docs.unity3d.com/6000.5/Documentation/ScriptReference/AssetDatabase.html) | Uses `GUIDToAssetPath` and `GetMainAssetTypeAtPath` from an Editor C# script to resolve explicit selected GUIDs. |
| Godot | [EditorScript](https://docs.godotengine.org/en/stable/classes/class_editorscript.html) and [command-line tutorial](https://docs.godotengine.org/en/stable/tutorials/editor/command_line_tutorial.html) | Uses an explicit `res://` selection, Godot resource loading, and the documented headless command-line mode. |

The official API documents were accessed on 2026-08-25. Real Editor execution remains opt-in and requires an authorized editable project, matching Editor version, and a confined staging root.
