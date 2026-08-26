# Real Unity Editor Bridge Smoke Evidence

## Scope and authorization boundary

This record summarizes an **opt-in real Editor smoke run** performed by the authorized project owner against the editable `Test123` Unity project. The project declared Unity `6000.5.10f1` with URP `17.5.0`. The user explicitly approved the two selected assets below.

No Unity project assets, generated Library data, PackageCache, credentials, build output, bridge report, or staged binary are included in this repository. The received report and FBX were used only for validation.

| Approved source identity | Source asset | Observed classification | Staged output |
|---|---|---|---|
| `885EAC34FD0D577439E6670A7CA3777E` | `Assets/3D Characters Zombie City Streets Lowpoly Pack - Lite/Models/Characters_Zombie_SuitMan_1.fbx` | `converted` | `exports/885EAC34FD0D577439E6670A7CA3777E.fbx` |
| `EAE1624132DA00D428C5D34F366B20DD` | `Assets/100BestEffectPack/Effects/PortalEffect/PortalEffect.prefab` | `requires_adapter` | None |

The VFX prefab is deliberately not treated as a converted render artifact. Its report warning states that `.prefab` has no approved direct-source staging policy. This preserves the product boundary: a Unity ParticleSystem/prefab graph is visible to users as requiring an adapter rather than silently staged or misrepresented as converted.

## Validation results

The real batch-mode run logged `Vortex Unity bridge wrote 2 record(s)`. The returned report validated against `vortex.editor-bridge/v1`, contained exactly the two approved Unity GUIDs, and used a portable root-confined output path. The direct staged FBX was byte-for-byte identical to the authorized source FBX.

| Check | Result |
|---|---|
| Report schema and protocol | Passed: `vortex.editor-bridge/v1` |
| Selected source identities | Passed: exactly both approved uppercase Unity GUIDs |
| Direct-source FBX staging | Passed: byte-identical 230,252-byte binary FBX, SHA-256 `1b80335e4bd4a7d4f3d7e029b61a4b5ee07f6581a117102ff5acbbf93eee2490` |
| VFX compatibility classification | Passed: `requires_adapter`, no staged output, visible `.prefab` policy warning |
| Batch-mode completion | Passed: Unity logged two bridge records and a confined report/staging root |

## Unity 6000 compatibility repair

The first real run exposed unrelated legacy package compile blockers before the bridge could execute. The checked-in bridge was also updated to use `private set` request properties rather than C# `init` accessors, because the Editor’s `Assembly-CSharp-Editor` profile did not supply `System.Runtime.CompilerServices.IsExternalInit`.

The external launcher temporarily moves only the unrelated legacy demo script and legacy image-effects script directory **outside** the Unity `Assets/` tree during the batch run, then restores both on success or failure. Keeping the temporary quarantine outside `Assets/` is essential because Unity compiles C# files anywhere inside `Assets`, even when a folder is renamed. The launcher restoration path is tested without modifying the approved mesh or VFX prefab.

## Remaining scope

This smoke evidence satisfies the real-Editor bridge identity, confinement, explicit-classification, and staged-source checks. It does **not** complete the broader T14/T24 scene requirement: a deliberately selected small scene containing a static mesh, URP Lit material, texture, camera, light, and unsupported component must still traverse the full conversion, published-manifest, and headless Bevy-loading path. The current evidence therefore keeps T14 and T24 in progress rather than marking either task complete.
