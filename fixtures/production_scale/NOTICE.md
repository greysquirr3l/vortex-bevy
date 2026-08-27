# Production-scale fixture provenance

This directory contains two externally authored fixtures retained under the user’s explicit authorization for integration testing and profiling. They are not user project assets, Marketplace content, or Editor-generated outputs. Each fixture has a sibling `.fixture.json` manifest that pins its source and SHA-256 checksum.

| Fixture | Upstream source | License | Retained scope |
|---|---|---|---|
| `standard_shader_ball/shaderball.usda` | [`usd-wg/assets` StandardShaderBall](https://github.com/usd-wg/assets/tree/3b75c2dad6a494897557dcca0098257bcf42a8c6/full_assets/StandardShaderBall) at commit `3b75c2dad6a494897557dcca0098257bcf42a8c6` | [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/) | One 3,354,071-byte USDA layer; no referenced layers, texture maps, media, or authoring files. |
| `polyhaven_wooden_planks/wooden_planks_diff_1k.png` | [Poly Haven `wooden_planks`](https://polyhaven.com/a/wooden_planks) | [CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/) | One 5,016,652-byte 1024×1024, 16-bit RGB diffuse PNG; no other PBR maps or models. |

The StandardShaderBall source identifies **Chris Rydalch** for geometry and textures, **André Mazzone** for specification and validation, and **Thomas Anagnostou** for the original scene inspiration and consultation. This retained derivative fixture preserves that attribution, the CC BY 4.0 license URL, and an immutable upstream revision in its sibling manifest. [1]

Poly Haven states that its assets are licensed under CC0 and may be used and redistributed for any purpose. The manifest retains the published source URL, published MD5, published byte count, and a local SHA-256 integrity check for reproducibility. [2]

## References

[1]: https://github.com/usd-wg/assets/blob/3b75c2dad6a494897557dcca0098257bcf42a8c6/full_assets/StandardShaderBall/README.md "StandardShaderBall README"
[2]: https://polyhaven.com/license "Poly Haven Asset License"
