# Fixture contract

Every fixture in this directory is self-authored and may be used for automated tests. Each fixture family records its purpose, source format, expected classification, and checksum in a sibling JSON manifest. Fixture manifests use only normalized forward-slash paths and must not contain wall-clock timestamps.

| Directory | Purpose | Expected outcome |
|---|---|---|
| `minimal_usda/` | Readable OpenUSD scene with geometry, PBR factors, transform, camera, light, and animation samples. | Converted after USD ingestion exists. |
| `minimal_usdc/` | Reserved output location for reproducibly generated binary USD from a checked-in source. | Not committed until a reproducible generator task adds it. |
| `malformed/` | Deliberately invalid source documents. | Typed diagnostic; no panic or partial publication. |
| `unsupported/` | Valid source features outside the supported subset. | `unsupported` or `requires_adapter` diagnostic. |
| `expected/` | Golden expected metadata and future artifact checksums. | Byte-stable comparison after canonicalization. |

Do not add Unreal project data, Marketplace assets, game assets, `.uasset`, `.uexp`, `.ubulk`, `.pak`, `.utoc`, `.ucas`, keys, certificates, or files larger than 5 MiB. Synthetic Unreal package-header cases should use unit-level byte builders or documented `.bin` files only after the inspection task defines their supported header contract.
