# Unity Adapter Sources

The bounded Unity source adapter accepts self-authored, text-serialized editable project data only. The following official Unity materials define its safety and scope constraints.

| Topic | Source | Adapter consequence |
|---|---|---|
| Asset serialization | [Unity Editor Settings — Asset Serialization Mode](https://docs.unity3d.com/2018.2/Documentation/Manual/class-EditorManager.html) | The adapter requires `m_SerializationMode: 2` (Force Text) and rejects binary/unknown layouts. |
| YAML object identity and meta GUIDs | [Understanding Unity's serialization language YAML](https://unity.com/blog/engine-platform/understanding-unitys-serialization-language-yaml) | YAML local File IDs identify objects inside one file; visible sibling `.meta` GUIDs identify the file. The adapter records both in typed provenance. |
| Cross-asset references | [Direct reference asset management](https://docs.unity3d.com/6000.3/Documentation/Manual/assets-direct-reference.html) | Direct references require GUID/local-ID interpretation; unsupported or unsafe external references fail closed rather than being resolved from Library/package cache. |
| URP Lit material subset | [URP Lit Shader](https://docs.unity3d.com/Packages/com.unity.render-pipelines.universal@12.0/manual/lit-shader.html) | The initial material subset recognizes only `Universal Render Pipeline/Lit` and `Universal Render Pipeline/Unlit`; Shader Graph and custom material behavior remain explicit unsupported/adapter-needed classifications. |

The adapter does not edit Unity YAML or replace the matching Unity Editor. It only inspects authorized source facts and leaves actual import semantics to the human-gated Editor bridge.
