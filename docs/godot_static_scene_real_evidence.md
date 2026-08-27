# Godot 4.7.2 static-scene bridge evidence

## Scope and authorization

This evidence records a controlled real-Editor validation performed against the user-authorized `Test123` Godot project at `/Users/ncampbell/Documents/Godot/Test123` using Godot `4.7.2.stable.official.ed1daf0bf` on macOS. No user project asset, source scene, cache, report, GLB, or PNG is stored in this repository.

The launcher invoked the checked-in bridge from an external directory. The bridge created the validation scene in memory, wrote only a generated GLB and generated PNG to an external staging directory, and did not create or save a `res://` resource. The user confirmed that a post-run inventory found no Vortex or static-scene source path outside the normal `.godot/` cache directory.

## Controlled scene and bridge result

The generated scene contained one box mesh, a base-color PNG, a `Camera3D`, a `DirectionalLight3D`, and an inert `GPUParticles3D` node. The bridge report contains three controlled records under `vortex.editor-bridge/v1`:

| Record | Outcome | Boundary |
|---|---|---|
| Generated static scene | Converted to `exports/VortexStaticScene.glb` | Direct Godot GLB export |
| Generated base-color image | Converted to `textures/vortex-static-base-color.png` | Embedded `image/png` retained by the GLB |
| `GPUParticles3D` | `RequiresAdapter` | Particles are not converted or emulated |

The staged report SHA-256 was `f2a1de012182220b8deaf7c7ea080cdcf55dae1e025578dbfc438c4608fdccf6`. The staged GLB SHA-256 was `68a2865e9ed7f0ae8fc6c39973c9d75bc151a5674da751114b0965bdf9032eb6`; the staged PNG SHA-256 was `99bbd20b58a0a46c04a86cb5a41f59aaf96a2c13cb35e7a483fc57744cc34435`.

## Canonical publication and runtime validation

The ignored opt-in Rust test consumed explicit local paths for the received report, GLB, and PNG. It parsed the report through the bounded Godot adapter, retained the typed Godot provenance identity `godot:test123-godot-4.7.2:godot://temporary/VortexStaticScene`, and atomically published the report, GLB, and PNG with manifest hash verification.

> The direct Godot GLB path does not re-author the already-valid generated GLB. The validation verifies the external artifacts, ties them to typed provenance, and loads the published result through the Bevy runtime.

The transaction published three verified artifacts and deterministically replayed conversion ID `ad3e4e93fb96d22076a9a4aa433113d42133694c6ecfcab2e5061745f5354223`. Headless Bevy loaded the published GLB with its embedded PNG texture and instantiated one `StandardMaterial`, two cameras, and one directional light. Bevy's explicit `png` feature is enabled because default Bevy features are intentionally disabled in this project.

## Limits and remaining boundaries

This evidence validates only the controlled in-memory Godot static-scene export and direct GLB runtime path. It does not claim general conversion of Godot `.tscn` scenes, project scripts, plugins, particles, animations, shaders, or arbitrary imported resources. Those inputs remain explicitly adapter-gated until a separately scoped conversion contract and test suite exist.

The relevant Godot API and external-execution constraints are documented in [Godot static-scene API sources](godot_static_scene_api_sources.md).
