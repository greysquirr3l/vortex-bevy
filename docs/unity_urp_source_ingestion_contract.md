# Unity URP Text-Project Ingestion Contract

## Scope

`vortex-core::adapters::secondary::unity` inspects an **authorized editable Unity project** that uses visible `.meta` files and Force Text serialization. It accepts caller-selected files under `Assets/` and returns owned `UnitySourceRecord` values plus project provenance. It does not load Unity’s Library cache, packages cache, serialized binary assets, custom runtime code, or an installed Unity Editor.

The adapter is a bounded source-inspection path. The committed Unity Editor C# bridge remains the authoritative path for importer-resolved meshes, textures, and other semantic export data.

## Required project boundary

| Requirement | Enforced behavior |
|---|---|
| Project root | Canonicalized directory; selected paths may not leave the root after symlink resolution. |
| Selected paths | Explicit relative `Assets/` paths only; traversal and absolute/external references are rejected. |
| Editor version | The documented initial layout accepts Unity `2022.3.*` and Unity `6000.*` project versions. Unknown layouts fail closed. |
| Serialization | `ProjectSettings/EditorSettings.asset` must contain `m_SerializationMode: 2` (Force Text). |
| URP | `Packages/manifest.json` must declare `com.unity.render-pipelines.universal`. |
| Identity | Every selected asset requires a visible sibling `.meta` file with one typed GUID. Duplicate selected GUIDs are rejected. |
| Source size | Text inputs are limited to 2 MiB. Binary/unknown text layouts are rejected before records are published. |

Unity documents Force Text serialization for version-control-friendly scenes and assets, and documents the relationship between local YAML File IDs and sibling `.meta` GUIDs for cross-file identity. [1] [2]

## Accepted records and classification

The text parser recognizes Unity YAML object headers and a deliberately small static-3D subset. It stores no YAML AST or arbitrary property bag outside the adapter.

| Source | Initial accepted facts | Result |
|---|---|---|
| `.unity`, `.prefab` | `GameObject`, `Transform`, `MeshRenderer`, `SkinnedMeshRenderer`, `Camera`, `Light`, local object ID, name, local-position marker, GUID references | `requires_adapter`: importer/mesh payload still needs matching Editor or later adapter |
| `.mat` | Material name and `Universal Render Pipeline/Lit` or `Universal Render Pipeline/Unlit` identity | `requires_adapter`: later material-to-IR/package mapping owns portable PBR conversion |
| Texture/mesh import sources | Typed asset and `.meta` identity only | `requires_adapter`: matching Unity importer is authoritative |
| Shader Graph, custom shaders, renderer features, unsupported components | Explicit source fact/classification | `unsupported` or `requires_adapter`, never silently converted |

The URP Lit documentation identifies the standard Lit material family and its portable surface inputs; Vortex limits its initial text material identity to standard URP Lit/Unlit rather than guessing Shader Graph or custom shader behavior. [3]

## Provenance and deterministic output

Each record carries `SourceIdentity::Unity { project_root, asset_path, asset_guid, object_id }`, a typed source kind, classification, and sorted facts. `UnityProjectSnapshot::source_links` adds the Unity Editor version, URP package version, Force Text decision, and classification to canonical IR `SourceLink` records. Asset selection and record output are sorted by typed source identity, making repeated equivalent selections stable.

The self-authored suite in `crates/vortex-core/tests/unity_urp_source_ingestion.rs` covers deterministic scene/material/texture output, visible-meta enforcement, duplicate GUIDs, symlink escape, unsupported Editor version, serialization mode, missing URP, binary/unknown YAML, malformed object headers, unsafe references, and Shader Graph classification. No third-party Unity project, Library cache, or package cache is committed.

## References

[1]: https://docs.unity3d.com/2018.2/Documentation/Manual/class-EditorManager.html "Unity Editor Settings — Asset Serialization Mode"
[2]: https://unity.com/blog/engine-platform/understanding-unitys-serialization-language-yaml "Unity — Understanding Unity's serialization language YAML"
[3]: https://docs.unity3d.com/Packages/com.unity.render-pipelines.universal@12.0/manual/lit-shader.html "Unity URP Lit Shader"
