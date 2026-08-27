# UE 5.8.2 `UTexture2D` Metadata Contract

T26 adds a **bounded direct decoder** for exactly one editable-package capability: the legacy tagged Asset Registry record for one `UTexture2D` saved by **Unreal Engine 5.8.2**. It provides a quick, non-mutating metadata preflight when the facts are useful but launching the Editor solely to inspect them would be unnecessary. It does not decode texture pixels, deserialize `UTexture2D` object properties, consume `.ubulk` bytes, or convert an asset to GLB.

> The authoritative Unreal Editor bridge remains the only approved route for semantic export or baking. A direct metadata success is deliberately classified as `requires_adapter`.

## Exact capability

| Dimension | Supported boundary | Rejected boundary |
|---|---|---|
| Source identity | One explicitly supplied typed `/Game/...` Unreal object identity | Non-Unreal identity or a tag-table object path that differs from the request |
| Package serialization | Little-endian package tag; legacy format `-9`; UE4 file version `522`; UE5 object version `1018`; zero licensee version; saved-by engine `5.8.2` | Unknown, older, newer, swapped, custom-licensee, or unversioned serialization |
| Asset Registry table | Exactly one legacy tagged record beginning at the header’s `AssetRegistryDataOffset` | Absent table, zero/multiple records, malformed offsets, compressed chunks, extra package references, or unsupported tags |
| Asset class | `Texture2D` | Any other class, including subclasses or Blueprint-created assets |
| Semantic tags | `Dimensions`, `PixelFormat`, `CompressionSettings`; optional `Filter` and lowercase-normalized 64-hex `SourceFileHash` | Missing required facts, duplicate/unknown tags, unbounded dimensions, or unrecognized enum values |
| Bulk reference | Internal bulk offset, or a nonempty external companion supplied only as an existence signal | No companion for an external offset; `.ubulk` content is never read |
| Output | Typed source identity, dimensions, selected format/compression/filter facts, safe bulk-reference state, and `requires_adapter` classification | Pixels, mip bytes, source image paths, native `UObject` fields, conversion result, or inferred semantics |

The decoder is intentionally version-locked rather than guessing how a later package header or tag representation should be read. The legacy Asset Registry table is a package-header data region; Unreal’s Asset Registry documentation describes header tags as the data used to populate an asset’s `TagsAndValues` map.[1]

## Independent oracle

`bridges/unreal/vortex_unreal_texture2d_metadata_bridge.py` is a separate, non-mutating UE 5.8.2 oracle. It requests exactly one on-disk Asset Registry `AssetData` record and reads only the explicit selected tags through the documented `get_tag_value` API. It does not call `get_asset`, which is the documented method that may load an unloaded asset.[2] The bridge writes a single JSON report atomically beneath a caller-owned staging directory.

```json
{
  "engine": "unreal",
  "engine_version": "5.8.2",
  "protocol_version": "vortex.unreal-texture-metadata/v1",
  "records": [
    {
      "asset_kind": "Texture2D",
      "cooked": false,
      "object_path": "/Game/Textures/T_Fixture.T_Fixture",
      "tags": {
        "CompressionSettings": "TC_Default",
        "Dimensions": "256x128",
        "Filter": "TF_Trilinear",
        "PixelFormat": "PF_B8G8R8A8"
      }
    }
  ]
}
```

The Rust decoder parses this report as a strict, unknown-field-rejecting protocol and compares its typed semantic facts to independently decoded package facts. A match validates the direct subset only; it does not establish binary compatibility for arbitrary packages or replace an Editor export.

## Authorized validation procedure

Run the oracle only in an authorized editable UE 5.8.2 project, with a caller-owned staging location outside the project. The command does not create or modify assets, but normal Editor cache behavior remains the project owner’s responsibility.

```bash
UNREAL_EDITOR="/path/to/UnrealEditor-Cmd"
PROJECT="/authorized/Project/Project.uproject"
STAGING="$HOME/Desktop/vortex-unreal-texture-metadata"
mkdir -p "$STAGING"
"$UNREAL_EDITOR" "$PROJECT" -unattended -nop4 \
  -script="$(pwd)/bridges/unreal/vortex_unreal_texture2d_metadata_bridge.py" -- \
  --asset /Game/Textures/T_Fixture.T_Fixture \
  --staging-root "$STAGING" \
  --report "$STAGING/vortex-unreal-texture-metadata-report.json"
```

Do not upload or commit the selected `.uasset`, `.ubulk`, `.uexp`, source project, caches, logs, or texture pixels. For any future authorized comparison, retain only the exact engine version, chosen object identity, decoder result, oracle result, and hashes/metadata sufficient to reproduce the decision.

The ignored opt-in test reads only explicitly provided external paths and writes nothing. Run it only after the external report exists:

```bash
VORTEX_UNREAL_TEXTURE_PROJECT_ROOT="/authorized/Project" \
VORTEX_UNREAL_TEXTURE_UASSET="/authorized/Project/Content/Textures/T_Fixture.uasset" \
VORTEX_UNREAL_TEXTURE_ORACLE="$STAGING/vortex-unreal-texture-metadata-report.json" \
VORTEX_UNREAL_TEXTURE_OBJECT="/Game/Textures/T_Fixture.T_Fixture" \
cargo test -p vortex-core --test real_unreal_texture_metadata_opt_in -- --ignored
```

If the header indicates external bulk data, set `VORTEX_UNREAL_TEXTURE_UBULK` to the corresponding explicit companion path. The test uses it only as a nonempty existence signal and does not inspect its bytes.

## Verification coverage

The owned, synthetic test suite builds a minimal UE 5.8.2-shaped package header and tag table in memory. It verifies successful typed facts and oracle equivalence; malformed byte fuzz smoke handling; unsupported version rejection before tag reads; unknown/missing tag rejection; and required external-companion handling. No proprietary package, image, or external-bulk payload is stored in the repository.

## References

[1] [Epic Games, *Asset Registry in Unreal Engine*](https://dev.epicgames.com/documentation/en-us/unreal-engine/asset-registry-in-unreal-engine)

[2] [Epic Games, *unreal.AssetData Python API 5.8*](https://dev.epicgames.com/documentation/en-us/unreal-engine/python-api/class/AssetData?application_version=5.8)

[3] [CUE4Parse, `FPackageFileSummary` source reference](https://github.com/FabianFG/CUE4Parse/blob/master/CUE4Parse/UE4/Objects/UObject/FPackageFileSummary.cs)

[4] [UAssetAPI project reference](https://github.com/atenfyr/UAssetAPI)
