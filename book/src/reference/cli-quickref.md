# CLI quick reference

A condensed one-page summary of every subcommand and its required flags.
For the full flag-by-flag reference, see [CLI](cli.md).

## Subcommands

```text
vortex-bevy inspect     --project --engine --asset [--object] [--format]
vortex-bevy plan       --project --engine --asset --object --profile --output [--guid] [--variant] [--payload-policy] [--format]
vortex-bevy convert    --project --engine --asset --object --profile --output [--guid] [--variant] [--payload-policy] [--dry-run] [--overwrite] [--format]
vortex-bevy verify     --conversion-id [--format]
vortex-bevy view       --artifact-root [--format]
vortex-bevy support-matrix [--format]
vortex-bevy version     [--format]
```

## Engine-specific required flags

| Engine | `--object`               |
| ------ | ------------------------ |
| Unreal | `/Game/Path/Asset.Asset` |
| USD    | `/Prim/Path`             |
| Unity  | `<u64>`                  |
| Godot  | (defaults to `--asset`)  |

| Engine | additional required    |
| ------ | ---------------------- |
| Unity  | `--guid <32-char hex>` |

## Exit codes

| Code | Meaning                  |
| ---- | ------------------------ |
| `0`  | Success                  |
| `2`  | Argument parse error     |
| `3`  | Capability `Unavailable` |
| `4`  | Typed error              |

## Classifications

`converted`, `baked`, `requires_adapter`, `unsupported`, `rejected`.

See [Understand classification outcomes](../how-to/classification-outcomes.md)
for the full taxonomy.
