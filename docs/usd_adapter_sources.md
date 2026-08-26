# OpenUSD Adapter Sources

The initial USDA adapter is deliberately bounded and does not claim to replicate the authoritative OpenUSD composition engine. These sources informed its selection facts and normalization boundaries.

| Topic | Source | Adapter consequence |
|---|---|---|
| USDA encoding and composition | [OpenUSD FAQ](https://openusd.org/release/usdfaq.html) | `.usda` is UTF-8 text; USD composition includes references and variants, so the initial parser records rather than resolves arcs. |
| Stage linear units | [Encoding Stage Linear Units](https://openusd.org/24.08/api/group___usd_geom_linear_units__group.html) | `metersPerUnit` is stage-wide and defaults to 0.01 when unauthored; the adapter records normalized stage-level units rather than scaling raw authored transform values. |
| Stage up axis | [Encoding Stage UpAxis](https://openusd.org/release/api/group___usd_geom_up_axis__group.html) | `upAxis` is stage-level with legal `Y` or `Z` values; the adapter validates and records the pending `Z`-to-`Y` normalization decision for later IR processing. |
| References and variants | [Referencing Layers tutorial](https://openusd.org/dev/tut_referencing_layers.html) | References, payloads, sublayers, and variant sets are composition arcs; full composed semantics require an approved OpenUSD runtime adapter. |

The adapter validates self-authored USDA fixtures only. Real OpenUSD runtime composition remains a separate, explicitly gated capability.
