# Inspect an Unreal asset

The `inspect` subcommand is the bounded metadata preflight path for
explicitly authorized `.uasset` files. It is **metadata-only**: it
extracts stable, sorted facts from the package header and a small set of
known tag tables. It never reads `.ubulk` / `.uexp` payloads, never
infers Blueprint/C++ semantics, and never invokes the Unreal Editor.

This page covers the invocation, the safety limits the adapter enforces,
and what the resulting report looks like.

## Invocation

```bash
cargo run -p vortex-bevy -- inspect \
    --project /authorized/Project \
    --engine unreal \
    --asset Content/Props/Fixture.uasset \
    --object /Game/Props/Fixture.Fixture \
    --format json
```

Four flags are required:

- `--project <path>` — physical filesystem root that the package must
  resolve beneath after canonicalization. The CLI canonicalizes this
  before any other work.
- `--engine unreal` — selects the Unreal preflight adapter. Other
  engines (`usd`, `unity`) currently return an explicit
  "unavailable" outcome from `inspect`; see [Convert an authorized source](convert.md)
  for the corresponding `convert` dispatch.
- `--asset <path>` — project-relative package path. The CLI parses
  this into a typed `SourceAssetPath` that rejects absolute paths,
  traversal segments, empty components, and NUL bytes.
- `--object <UnrealObjectPath>` — the typed object reference inside
  the package. The CLI parses this into a typed `UnrealObjectPath` that
  validates the `/Game/...` form.

## Safety limits

The adapter applies, in order:

1. **Root canonicalization.** The supplied `--project` is canonicalized.
2. **Confinement.** The package path is canonicalized and must resolve
   beneath the project root. Symlink escapes fail closed.
3. **Extension check.** The package must end in `.uasset`.
4. **Size cap.** The package must not exceed the bounded size limit
   (32 MiB default).
5. **Magic check.** The package prefix must match the Unreal package
   magic. Mismatched magic fails closed.
6. **Cooked-style sidecar rejection.** The presence of a `.pak`,
   `.utoc`, `.ucas`, `.uexp`, or `.ubulk` sidecar causes a fail-closed
   `rejected` classification. Vortex does not bypass access controls.

A failure at any step short-circuits to a typed error and never invokes
the package parser.

## Result contract

A successful bounded header parse returns a JSON report with stable,
sorted facts:

```json
{
    "kind": "Inspection",
    "source": "unreal:FixtureProject:Content/Props/Fixture.uasset:/Game/Props/Fixture.Fixture",
    "classification": "requires_adapter",
    "reason": "...",
    "remediation": "Use the matching authorized Unreal Editor bridge for semantic export.",
    "facts": [
        "package.byte_count=12345",
        "package.path=Content/Props/Fixture.uasset",
        "package.engine_version=.../.../...",
        "package.name=Fixture",
        "package.imports=3",
        "package.exports=2",
        "export.class=UStaticMesh",
        ...
    ]
}
```

`facts` is sorted by key. Each fact is a `key=value` token whose value is
bounded and known-shape. New fact families are added behind typed
extension; the inspector never invents a fact that the package header
does not support.

The classification is `requires_adapter` even on a successful inspect —
**this is by design**. The bounded metadata preflight answers "is this a
real package, what does its header say?" It does not produce exportable
semantic content. Authoritative export requires the matching authorized
Unreal Editor bridge. The remediation string in the report names the
bridge and the asset path you need to pass.

## Rejection outcomes

`inspect` returns one of three typed classifications:

| Classification                      | Cause                                                                                               | Remediation                                                                         |
| ----------------------------------- | --------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------- |
| `rejected`                          | Path escape, symlink escape, missing file, size limit, magic mismatch, cooked-style sidecar present | Recheck the project root and asset path; do not weaken confinement or provide keys. |
| `requires_adapter`                  | Bounded metadata parse succeeded                                                                    | Use the matching authorized Unreal Editor bridge for semantic export.               |
| `rejected` (parser incompatibility) | Header indicates a parser version newer than supported, or an unknown package shape                 | File an issue with the package metadata facts and the engine version.               |

A `rejected` outcome never silently coerces. Each carries a stable
`facts` line with a `diagnostic.code` token and a typed error from the
adapter.

## Going further

- For authoritative export after a successful `requires_adapter` inspect,
  invoke the checked-in Unreal Editor bridge at
  `bridges/unreal/vortex_unreal_bridge.py` and pass the result through
  [Convert an authorized source](convert.md).
- The bridge protocol is documented at
  [`docs/unreal_editor_bridge_protocol.md`](../../docs/unreal_editor_bridge_protocol.md).
