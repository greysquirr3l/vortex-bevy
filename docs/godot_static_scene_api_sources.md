# Godot Static-Scene Validation API Sources

The authorized Godot `Test123` validation uses a headless script that creates no `res://` resources. It builds a small `Node3D` graph only in memory, exports its rendered subset to an external GLB, writes a generated base-color PNG to the same external staging root, emits the shared bridge report, and frees the in-memory root before exit.

| Need | Godot 4.7 API | Boundary applied |
|---|---|---|
| Scene-to-glTF export | `GLTFDocument.append_from_scene(node, state)` followed by `write_to_filesystem(state, path)` | Export an in-memory root only to a caller-provided absolute staging path. The bridge uses `ROOT_NODE_MODE_KEEP_ROOT` to avoid a Godot-specific single-root extension. |
| Transient render texture | `Image.create`, `set_pixel`, `save_png` and `ImageTexture.create_from_image` | Build a 2×2 RGBA texture in memory, stage only the PNG beneath the confined output root, and bind its in-memory texture to a `StandardMaterial3D`. |
| Scene ownership | `PackedScene.pack` and `ResourceSaver.save` exist for serializing a scene resource | Intentionally not used. The validation must not add or mutate project `.tscn`, `.tres`, `.import`, or `.godot` state. |
| Unsupported component | `GPUParticles3D` | Add an inert particle node only in memory and declare it `requires_adapter`; particle processing/material semantics are not approximated by the static GLB subset. |

> `GLTFDocument` is a scene-data processor rather than a retained scene container: it accepts scene nodes through `append_from_scene`, stores exported state in `GLTFState`, and writes the result to a filesystem path. The static validator therefore needs no temporary saved Godot scene.[1]

The output report uses the shared `vortex.editor-bridge/v1` protocol. Its controlled source IDs are `godot://temporary/...`, distinguishing generated in-memory validation objects from user-owned `res://` project resources. The parent shell launcher copies this checked-in script into a temporary project-local path only for the invocation, then restores or removes that copy using a guarded exit trap.

## References

[1]: https://docs.godotengine.org/en/4.7/classes/class_gltfdocument.html "Godot Engine 4.7 — GLTFDocument"
[2]: https://docs.godotengine.org/en/4.7/classes/class_image.html "Godot Engine 4.7 — Image"
[3]: https://docs.godotengine.org/en/4.7/classes/class_packedscene.html "Godot Engine 4.7 — PackedScene"
