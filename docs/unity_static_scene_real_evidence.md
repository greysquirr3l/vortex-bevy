# Authorized Unity Static-Scene Canonical Conversion Evidence

This evidence records the opt-in real-Editor validation completed on **2026-08-26** against the user-authorized `Test123` Unity project. It supplements, rather than replaces, the earlier direct-FBX and complex-VFX smoke evidence in [`unity_editor_bridge_smoke_evidence.md`](unity_editor_bridge_smoke_evidence.md).

No Unity project asset, Unity `Library`, `Temp`, `Logs`, `PackageCache`, generated report, or generated PNG is stored in this repository. The report and PNG were supplied only as task attachments, copied into a private local validation root, and published only under an external local output root for hash verification.

## Controlled Editor Scenario

The unattended batch bridge used Unity **6000.5.10f1** and URP **17.5.0**. It created one temporary, self-generated scene under `Assets/VortexBridgeTemporary/`, saved it to obtain a genuine Unity scene GUID and object identity, extracted its resolved mesh and URP Lit facts, wrote the bounded report and generated PNG to a staging root outside the project, and then removed the temporary asset folder.

The scene intentionally contained a Unity primitive cube, a generated 2×2 base-color PNG, a generated `Universal Render Pipeline/Lit` material, a camera, a directional light, and the cube’s `BoxCollider`. The `BoxCollider` was deliberately retained as a visible non-rendering `requires_adapter` result; it was not silently omitted or converted.

| Evidence field | Verified value |
|---|---|
| Protocol | `vortex.unity-static-scene/v1` |
| Unity Editor | `6000.5.10f1` |
| URP package | `17.5.0` |
| Scene GUID | `544d0227b20a4450bb329a058d04f348` |
| Scene object ID | `753336472` |
| Reported scene path | `Assets/VortexBridgeTemporary/StaticScene.unity` |
| Static mesh | 24 vertices; 36 triangle-list indices |
| Material | `Universal Render Pipeline/Lit`; base color `[0.8, 0.4, 0.2, 1.0]`; metallic `0.35`; smoothness `0.6` |
| Texture output | `textures/vortex-static-base-color.png`; 2×2 generated PNG |
| Camera | One perspective camera; vertical FOV `1.0471975803375245` radians |
| Light | One directional light; intensity `1.0` |
| Explicit difference | `unity.component.BoxCollider:RequiresAdapter` |

## Input and Published Artifact Integrity

The uploaded report and generated PNG were staged only under a local validation input directory. The typed adapter accepted the report’s lower-case GUID, bounded mesh streams, finite transforms, strict URP Lit identity, safe PNG-relative output path, and explicit unsupported-component classification. It then normalized the PNG into its content-addressed publication location, mapped the material through `UrpMaterialMapper`, and built one `CanonicalScene` with source links, mesh, material, texture, sampler, camera, light, and unsupported-feature records.

| Artifact | SHA-256 |
|---|---|
| Editor report | `4b27c80e8a675785190e73466404a9944bcda64544a5b9f8e86ede9a9296a4ab` |
| Generated PNG | `0c15dad8079b1705da73229ffadad03f193422d11516a1713f70209646cb7992` |
| Deterministic GLB | `592ba3991964fc19a1af744bc264f4f27a58c35ba29562ac552c056aa5ede337` |
| Published conversion ID | `2c36a20a6275ab80b73977d3f2d6cf0a65a4e00ed295fe7c98ef17d8026db6c5` |

`GlbWriter` produced identical GLB bytes on a replay from the same validated canonical IR. `AtomicArtifactStore` atomically published `scene.glb`, the content-addressed external texture, and a copy of the report; it verified all **three** declared artifact hashes and returned `Reused` for the repeat publication request.

## Headless Bevy Result and Boundary

The verified published conversion was loaded through `build_runtime_app` and `load_published_conversion_and_spawn`. Bevy instantiated the scene root with **one** `StandardMaterial`, **one** camera, and **one** directional light. The manifest retains the typed Unity GUID/object source identity, Unity/URP version facts, URP Lit classification, and the explicit `BoxCollider:RequiresAdapter` compatibility difference.

The external base-color PNG is hash-verified, emitted at the URI referenced by the GLB, and retained in the published manifest. The headless assertion intentionally proves GLB world instantiation and material/component creation rather than GPU-complete recursive image readiness; a renderer-enabled visual smoke remains a separate capability from this T14/T24 validation.

## Restoration Check

After the real batch run, the user independently checked the project. `csDemoSceneCode.cs`, the legacy ImageEffects `Scripts` directory, and its `.meta` were present. The temporary `Assets/VortexBridgeTemporary/` folder and the launcher's `.vortex-static-scene-quarantine` and `.vortex-static-scene-bridge-backup` folders were absent. This confirms the test did not leave its generated scene assets or launcher temporary directories in the authorized project.

## Reproducibility Command

The repository contains an ignored opt-in test, `real_unity_static_scene_opt_in.rs`. It requires explicit paths for the authorized report, staging root, and a new output root; normal CI neither invokes Unity nor requires a real-Editor artifact. The local run used:

```bash
CARGO_BUILD_JOBS=1 \
VORTEX_REAL_UNITY_STATIC_SCENE_REPORT=/private/input/vortex-unity-static-scene-report.json \
VORTEX_REAL_UNITY_STATIC_SCENE_STAGING_ROOT=/private/input \
VORTEX_REAL_UNITY_STATIC_SCENE_OUTPUT_ROOT=/private/output \
cargo test -p vortex-bevy --test real_unity_static_scene_opt_in -- --ignored
```
