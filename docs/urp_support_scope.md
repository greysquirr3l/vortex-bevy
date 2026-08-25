# Unity URP support scope

The initial Unity support track targets the **Universal Render Pipeline (URP)** as a prebuilt Scriptable Render Pipeline. The converter imports only authorized editable Unity project content and produces deterministic, flattened Bevy-compatible artifacts; it does not reproduce URP’s runtime renderer, shader graph execution, custom renderer features, or live volume system.

## Initial material mapping

The initial supported subset is the simple `Universal Render Pipeline/Lit` and `Universal Render Pipeline/Unlit` material contract. The adapter maps base color/map, metallic workflow, smoothness-to-roughness conversion, normal map, occlusion, emission, UV tiling/offset, alpha mode, and alpha cutoff into the canonical PBR/GLB contract where representable. It records the Unity shader and all source properties in provenance.

## Explicit capability boundaries

The adapter reports rather than approximates unsupported semantics: specular workflow when no approved mapping exists, height/parallax maps, clear coat, detail layers, shader graph, custom shaders, custom renderer features, decals, 2D renderer/light effects, volume/post-processing effects, reflection/light probe behavior, baked lighting, light cookies, platform quality settings, and renderer feature ordering. A later approved task may add a bounded material baking path for specific effects.

## Architectural placement

Unity YAML/serialized-asset extraction and optional Editor bridge code belong in a secondary adapter. They return owned typed source records; Unity APIs, serialized object types, and stringly property bags do not cross into domain or application code. The shared canonical scene IR remains source-neutral and preserves Unity object/file GUID, URP shader identifier, pipeline-asset version, and conversion classification only in `SourceLink`/manifest provenance.

## Sources

- Unity Manual, [Introduction to the Universal Render Pipeline](https://docs.unity3d.com/Manual/urp/urp-introduction.html), accessed 2026-08-25.
- Unity URP 12 Manual, [Lit Shader](https://docs.unity3d.com/Packages/com.unity.render-pipelines.universal@12.0/manual/lit-shader.html), accessed 2026-08-25.
