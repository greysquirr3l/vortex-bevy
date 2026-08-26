# GLB Serializer Sources

The contained deterministic GLB serializer is constrained by the Khronos glTF 2.0 specification.

| Requirement | Serializer consequence |
|---|---|
| A glTF asset specifies version `2.0` and combines JSON scene description with binary resources. | Every output writes `asset.version = "2.0"`, one JSON chunk, and one binary chunk. |
| A GLB is a single-file representation of glTF JSON and binary resources. | Vortex emits no external buffer URI and verifies the complete final byte sequence with the `gltf` parser. |
| Buffer views describe ranges of a buffer; accessors describe typed data elements. | Positions and indices have separately aligned buffer views and typed accessors. |
| Indexed geometry uses an index source to assemble primitives. | Vertex stream indices are validated by canonical IR before serialization and emitted as unsigned integer triangle-list accessors. |
| glTF can represent scenes, nodes, meshes, materials, cameras, animations, and skins. | The initial core subset emits scenes/nodes/meshes/PBR factors/cameras/punctual lights; texture staging, skins, clips, and morphs are explicit later-package boundaries. |
| Standard PBR uses metallic-roughness material semantics. | Only source-neutral `pbr_metallic_roughness` factors enter the initial material encoder. |

The adapter intentionally rejects unsupported compression and extension behavior rather than emitting unverified glTF extensions. The later texture, deformation, and artifact-publication steps expand only the data they can represent deterministically.

[1]: https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html "Khronos glTF 2.0 Specification"
