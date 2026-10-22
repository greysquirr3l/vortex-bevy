# Unity URP Fixture End-to-End Contract

T23 verifies the owned Unity URP fixture path in normal CI without invoking Unity Editor. The integration test creates a fresh Force Text project with declared Unity `2022.3.5f1`, URP `12.1.11`, a text scene, and exact URP Lit and Unlit material records. The files, GUIDs, and project metadata are self-authored during the test.

## Covered Flow

| Stage | Verified behavior |
|---|---|
| Source inspection | `UnityProjectInspector` receives explicitly selected `Assets/` paths, validates project metadata and `.meta` GUIDs, and returns typed source records with Unity/URP/serialization provenance. Reversing input traversal yields the same snapshot and source links. |
| Material mapping | `UrpMaterialMapper` maps exact Lit and Unlit shader identities into canonical PBR materials. The fixture asserts opaque alpha behavior and the unlit roughness convention. |
| Canonical package | The source links retain typed Unity object identity and project facts. A self-authored canonical mesh references both mapped material records; repeated package calls yield equal GLB bytes and checksum. |
| Publication | The root-confined atomic store publishes the manifest and GLB, then reuses the same conversion on deterministic replay. The manifest records the Unity Editor version, URP package version, Force Text mode, and supported material matrix. |
| Runtime | The Bevy 0.20 runtime verifies the published conversion before loading. Headless glTF spawning retains `VortexSceneRoot` and yields at least two `MeshMaterial3d<StandardMaterial>` resources. |
| Failure behavior | A Shader Graph material remains an explicit `unsupported` source record, malformed serialized input returns `BinaryOrUnknownSerialization`, and post-publication GLB tampering produces `VortexRuntimeDiagnostic` without a Vortex scene root. |

## Compatibility Matrix

The JSON compatibility report is regenerated from the CLI’s typed support-matrix query:

```bash
cargo run -q -p vortex-bevy -- support-matrix --format json \
  > docs/compatibility_matrix.json
```

It contains `e2e.synthetic_unity_urp_fixture` as `converted`. That classification applies only to the self-authored Force Text Lit/Unlit fixture subset exercised above.

> This suite does not validate a Unity import database, arbitrary serialized layouts, Shader Graph, custom renderer features, production textures, or runtime behavior. Those paths remain explicit `unsupported` or `requires_adapter` outcomes until an authorized matching Unity Editor project is supplied for T24.
