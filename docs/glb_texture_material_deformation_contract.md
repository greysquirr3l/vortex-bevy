# Deterministic GLB Texture, Material, and Deformation Contract

The GLB package boundary consumes a validated, Bevy-free `CanonicalScene` and emits deterministic glTF 2.0 JSON/BIN chunks. The implementation is intentionally constrained to features with typed source-neutral representations and explicit failure paths.

| Area | Implemented subset | Boundary behavior |
|---|---|---|
| Image normalization | Bounded PNG payloads, content-addressed staging labels, dimensions preflight, role-derived sRGB/linear intent | Non-PNG, truncated, oversized, zero-sized, and duplicate-ID inputs are rejected before staging. |
| Samplers and textures | Linear/nearest filtering, repeat/clamp/mirror wrap modes, deterministic `images` and `textures` records | The GLB references generated staging-relative image URIs; later artifact publication owns the actual atomic file write. |
| PBR materials | Metallic-roughness factors, base/metallic-roughness/normal/occlusion/emissive texture slots, alpha mode/cutoff, double-sided, emissive/normal/occlusion factors | Unknown material families and texture slots fail closed. |
| UV transforms | Offset/scale are retained in canonical IR and emitted with `KHR_texture_transform` when non-identity | The extension is declared only when used. Runtime engine compatibility still needs a real Bevy visual smoke test. |
| Unity URP | Exact baseline `Universal Render Pipeline/Lit` and `/Unlit` mapping; smoothness maps deterministically to `1 - roughness` | Shader Graph, clear coat, specular workflow, detail/height maps, custom shaders, and renderer features return `requires_adapter`; they are never silently approximated. |
| Geometry | Positions, normals, tangents, UVs, colors, JOINTS_0, WEIGHTS_0, indexed triangle primitives | Attributes must be aligned, finite, and influence weights normalized before packaging. |
| Deformation | Skins with inverse bind matrices, transform/weight animation channels, position-delta morph targets | Invalid joint graphs, nonfinite matrices, unaligned deltas, and malformed clips fail at canonical-IR validation. Shared-instance extensions remain explicitly deferred. |

> Texture bytes are normalized but are not embedded into the GLB in this recovery layer. Atomic copy/publish is owned by the later artifact-publication adapter, where staged paths and manifest hashes are verified together.

## Validation

Focused package tests prove byte-stable output and parse the emitted GLB with the pinned `gltf` parser. They cover textures, PBR factors, skins, JOINTS_0/WEIGHTS_0, conventional transform animation, and position morph targets. The source implementation also runs the workspace’s strict lint profile without global suppressions.

The serializer follows Khronos glTF 2.0 structural requirements. The associated specification findings are recorded in [`glb_serializer_sources.md`](glb_serializer_sources.md). Unity format and URP constraints are recorded in [`unity_adapter_sources.md`](unity_adapter_sources.md).
