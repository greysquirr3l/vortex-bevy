# Authorized Unreal static-scene real-Editor evidence

## Scope and boundary

This record closes **T10 — T09b** with a deliberately small, user-approved Unreal Engine project validation. It records only operator-supplied facts, stable hashes, and validation outcomes. It does **not** store the user project, the generated level, exported FBX/TGA files, reports, derived cache, Intermediate, Saved, Binaries, or logs in this repository.

The source project was the authorized editable Test123 project. The operator confirmed Unreal Editor **5.8.2-56702186+++UE5+Release-5.8** on macOS arm64. The controlled bridge used Unreal's unattended `AssetExportTask` surface, which supports explicit exporter, filename, automation, prompt suppression, replacement, and error collection. [1]

## Controlled scenario

The bridge refused to run if `/Game/VortexBridgeTemporary` already existed. It then created only that temporary package with the following bridge-owned content:

| Item | Stable Unreal identity | Intended bridge result |
|---|---|---|
| Static mesh | `/Game/VortexBridgeTemporary/SM_VortexCube.SM_VortexCube` | Authoritative FBX export |
| Base-color texture | `/Game/VortexBridgeTemporary/T_VortexBaseColor.T_VortexBaseColor` | Authoritative TGA export |
| Material instance | `/Game/VortexBridgeTemporary/MI_VortexLit.MI_VortexLit` | `requires_adapter` |
| Level with static mesh, camera, and directional light | `/Game/VortexBridgeTemporary/LVL_VortexStaticScene.LVL_VortexStaticScene` | `requires_adapter` |

The material instance and level remain visible boundaries: arbitrary Unreal material graphs and world/actor semantics are not silently treated as portable scene conversions. The temporary level was created only for this validation and was removed after staging.

## Real run results

The final commandlet completed with **0 errors**. Its `vortex.editor-bridge/v1` report was independently parsed through the typed Rust `UnrealBridge` boundary with all four approved source object paths requested explicitly. The typed result contained exactly two `Converted` entries and two `RequiresAdapter` entries.

| Received artifact | Size | SHA-256 | Independent validation |
|---|---:|---|---|
| `vortex-unreal-bridge-report.json` | 1,762 bytes | `ded5b12e3574d3a51cc94df38249b5d19749e1dda213425029f756be7c22a8e8` | Strict v1 report parse; output paths confined to `exports/` |
| `exports/SM_VortexCube.fbx` | 23,424 bytes | `8fdc8ade53dc55533e58b58e61596fddff354c67dd59b69da0a58dc9d44590ef` | Binary FBX 7300 signature |
| `exports/T_VortexBaseColor.tga` | 65,554 bytes | `475dc63044f2b9a00fb04e63d3864fb45bef14a9bd4daca277219efca63d4a52` | 128×128 RGBA TGA header |

The opt-in test `real_unreal_bridge_opt_in` accepts all three paths only through explicit environment variables. It creates no Unreal process, validates the report through `UnrealBridge`, checks the exact expected classifications and identities, and validates the staged FBX/TGA headers. It remains ignored in normal CI.

## Report-to-canonical-GLB validation

A second ignored opt-in test, `real_unreal_static_scene_conversion_opt_in`, consumed the same typed report and staged bytes without launching Unreal. It accepted only one noncollision render mesh from the FBX, excluded the `UCX_` collision hull, used primary normal/UV layers only, converted Unreal centimeters/Z-up coordinates to canonical meters/Y-up coordinates, reversed winding, transcoded the base-color TGA to normalized PNG, and retained the report's material-instance and world records as `RequiresAdapter` features.

| Canonical conversion result | Verified value |
|---|---|
| Render mesh | 144 flattened triangle vertices and 144 indices |
| Canonical material | One neutral metallic-roughness material with normalized base-color PNG |
| Explicit unsupported features | `unreal.material_instance:RequiresAdapter`; `unreal.world:RequiresAdapter` |
| Deterministic GLB SHA-256 | `bf4ab256b69848cd1177b2e15878bf75932c89277886c0d237c6ce5f443990f5` |
| Normalized PNG SHA-256 | `982ee697540934188b235cea31bff2ddb3f4a7135b0bcb06c64576e80c6dfed2` |
| Atomic publication | Three artifacts published, replay-reused, and hash-verified |
| Headless Bevy | Published GLB loaded and instantiated one `StandardMaterial` |

The external publication directory has conversion ID `7f42cd6993b8cfda45308455a4a4d7dd4ef5f660315090f3ca2d813e6e84c72f`. It is outside the repository and contains only the generated GLB, content-addressed PNG, source-report payload, manifest, and validation summary.

## Restoration confirmation

The operator independently verified that the following were absent after the successful run:

```text
Content/VortexBridgeTemporary
.vortex-unreal-static-scene-bridge-backup
Content/Python/VortexUnrealStaticSceneBridge.py
```

An earlier failed run's generated level was moved outside the project to an operator-controlled recovery location before the successful rerun. It was never committed, staged, or treated as source input.

## Remaining boundary

This evidence proves an authorized Unreal Editor authoritative export bridge and a bounded static-mesh report-to-canonical-GLB path for the controlled FBX/TGA pair, with explicit material/level adapter classifications. It does **not** claim Blueprint/C++ behavior conversion, arbitrary material graph conversion, full World/actor semantic conversion, collision/physics conversion, arbitrary FBX feature support, cooked or encrypted package support, or visual renderer parity. Those need separate approved adapters and acceptance coverage.

## References

[1]: https://dev.epicgames.com/documentation/en-us/unreal-engine/python-api/class/AssetExportTask
