# Authorized Editor Bridges

`bridges/` contains the only checked-in Editor automation required to obtain semantic asset facts from **authorized, editable source projects**. Each bridge accepts an explicit asset selection and writes a deterministic report conforming to [`contract/vortex-editor-bridge-v1.schema.json`](contract/vortex-editor-bridge-v1.schema.json). The bridge does not search arbitrary project content, execute project-provided code, transmit data, or circumvent encryption, DRM, access controls, or licensing.

> These are **Editor-gated integrations**. They are statically linted and contract-tested in normal CI, but actual execution requires a user-approved project, a matching installed Editor, and a staging directory controlled by the caller.

| Engine | Implementation | Explicit identity | Current approved output | Engine-gated command |
|---|---|---|---|---|
| Unreal Engine | [`unreal/vortex_unreal_bridge.py`](unreal/vortex_unreal_bridge.py) | Unreal object path | Static/Skeletal Mesh → FBX; `Texture2D` → TGA | `UnrealEditor-Cmd <project>.uproject -run=pythonscript -script=<absolute bridge path> -- --assets <object-paths> --staging-root <dir> --report <file>` |
| Unity | [`unity/Editor/VortexUnityBridge.cs`](unity/Editor/VortexUnityBridge.cs) | Unity GUID | Approved direct-source mesh, glTF, image, and audio files | `VORTEX_UNITY_GUIDS=<guids> VORTEX_UNITY_STAGING_ROOT=<dir> VORTEX_UNITY_REPORT=<file> Unity -batchmode -nographics -quit -projectPath <project> -executeMethod Vortex.UnityBridge.VortexUnityBridge.RunFromCommandLine` |
| Godot | [`godot/vortex_godot_bridge.gd`](godot/vortex_godot_bridge.gd) | `res://` resource path | Approved direct-source glTF, image, and audio files | `godot --headless --path <project> --script <absolute bridge path> -- --assets <res-paths> --staging-root <dir> --report <file>` |

## Shared contract

Each report has these top-level fields:

```json
{
  "engine": "unreal | unity | godot",
  "protocol_version": "vortex.editor-bridge/v1",
  "records": []
}
```

Every record carries its engine-native source identity, the selected exporter, only staging-relative outputs, sorted diagnostics, and one explicit classification: `converted`, `baked`, `requires_adapter`, `unsupported`, or `rejected`. Complex behavior, graphs, scenes, and components that lack a verified portable mapping must be reported as `requires_adapter` or `unsupported`; they must never be silently omitted.

The Unreal bridge uses the official `AssetExportTask` properties for unattended export and disables prompts. [1] The Unity bridge uses `AssetDatabase` to resolve only supplied GUIDs to project assets. [2] The Godot bridge uses documented headless command-line invocation and explicit project resource paths. [3] [4]

## Strict quality gate

Run the same bridge checks locally and in CI:

```bash
bash scripts/check-bridge-quality.sh
```

The gate runs Ruff’s full configured rule set and formatting check on Python, deterministic fixture tests, `gdlint` plus `gdformat --check` for GDScript, and a repository-pinned CSharpier check for C#. It also rejects unresolved `TODO`, `FIXME`, `HACK`, and `XXX` markers below `bridges/`. The Rust pre-push hook invokes this gate in addition to `cargo test`, strict Clippy, release build, and documentation generation.

## Explicit non-goals

These scripts do not validate a live Unreal, Unity, or Godot project in this environment. They do not decode cooked, encrypted, unowned, leaked, or DRM-protected packages. They do not convert gameplay code, Blueprints, complex material/shader graphs, custom engine plug-ins, or arbitrary runtime behavior. Those capabilities require a separate approved adapter and a matching real-Editor acceptance task.

## References

[1]: https://dev.epicgames.com/documentation/en-us/unreal-engine/python-api/class/AssetExportTask
[2]: https://docs.unity3d.com/6000.5/Documentation/ScriptReference/AssetDatabase.html
[3]: https://docs.godotengine.org/en/stable/classes/class_editorscript.html
[4]: https://docs.godotengine.org/en/stable/tutorials/editor/command_line_tutorial.html
