# OpenUSD Packaging Design Sources

T25 deliberately implements **prepackaged alternatives and bounded instance expansion** rather than claiming live or universal USD composition in Bevy.

The [OpenUSD `usdcat` tool documentation][1] states that `--flatten` composes a stage into one layer, bakes composition and value-resolution operators into the result, and preserves native USD instancing by flattening prototypes and adding references from instances. Consequently, a flattened package may differ structurally from participating source layers while evaluating equivalently. Vortex-Bevy retains the selected source facts and uses flattened output as a deterministic fallback; it does not expose the source composition graph as a live Bevy runtime.

The [OpenUSD variant tutorial][2] documents variants as authored named alternatives. T25 therefore exposes only **named alternatives that have been explicitly packaged**; a runtime request cannot evaluate arbitrary source variants. The [OpenUSD glossary][3] describes payloads and instancing as composition mechanisms. T25 treats payload loading, recursive references, and arbitrary composition as bounded source-adapter concerns, recording unsupported/unselected cases with explicit diagnostics rather than resolving them implicitly.

| Design decision | Source constraint |
|---|---|
| Flattened selected stage remains available | A flattened stage bakes composition operators and preserves runtime-independent content. |
| Variant switch targets a packaged named alternative | Variants are composition alternatives; this runtime does not compose source layers dynamically. |
| Instance batching is explicit and bounded | USD native instancing may retain a prototype/reference structure after flattening. |
| Payload/reference limits fail before allocation | Payloads can be loaded or unloaded and can recursively expand a stage; unbounded resolution is not a safe runtime default. |

[1]: https://openusd.org/dev/toolset.html "OpenUSD Toolset — usdcat flattening"
[2]: https://openusd.org/release/tut_authoring_variants.html "OpenUSD — Authoring Variants"
[3]: https://openusd.org/release/glossary.html "OpenUSD — Terms and Concepts"
