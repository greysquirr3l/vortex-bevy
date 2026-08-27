# Unreal Metadata Preflight Contract

Vortex-Bevy performs **metadata-only** inspection of an explicitly selected, authorized `.uasset` beneath a caller-supplied project root. This is a preflight boundary; it does not decode arbitrary `UObject` payloads and does not replace the matching Unreal Editor for semantic export.

## Invocation

```bash
cargo run -p vortex-bevy -- inspect \
  --project /authorized/Project \
  --engine unreal \
  --asset Content/Props/Fixture.uasset \
  --object /Game/Props/Fixture.Fixture \
  --format json
```

The CLI parses `--asset` and `--object` into validated domain identities before any filesystem work. `--project` establishes the physical filesystem root used exclusively for confinement. A syntactically valid source path still fails if canonicalization resolves it outside that root.

## Safety limits

The adapter canonicalizes the project root and candidate package path, rejects symlink escapes, requires a `.uasset` extension, and applies a 32 MiB default file-size limit before invoking the pinned `uasset` 0.6.0 reader. The package prefix must match the Unreal package magic. A `.pak`, `.utoc`, `.ucas`, `.uexp`, or `.ubulk` sidecar causes a fail-closed `rejected` classification because that context is treated as cooked-style content.

> Vortex-Bevy does not decrypt packages, guess keys, bypass access controls, or infer semantic equivalence for cooked, encrypted, Blueprint, C++, or unknown package payloads.

## Result contract

A successful bounded header parse returns stable, sorted facts. Current fact families include package byte count and logical path, package flags/name/version, engine-version clues, name/import/export counts, imported package dependencies, and exported class names. When no class can be resolved from the metadata table, the report emits `export.class=<unknown>` rather than inventing an asset type. The result remains `requires_adapter`: users must run the versioned matching Unreal Editor bridge to obtain exportable semantic content.

Malformed headers, invalid magic, parser incompatibility, cooked context, and a package that declares external bulk data beyond its own file but lacks the required `.ubulk` companion return an explicit `rejected` classification with a stable `diagnostic.code` fact and remediation. Transport failures such as a missing source, an oversized file, or a root escape return a typed adapter error before metadata parsing.

## Testing boundary

Repository policy forbids committing real Unreal package payloads. Tests therefore use self-authored in-memory or temporary synthetic bytes to verify successful header facts, declared package-dependency reporting, unknown-class reporting, incompatible-version rejection, missing external-bulk companion rejection, malformed-header handling, cooked-sidecar rejection, size-limit enforcement, symlink escape rejection, and a bounded malformed-byte fuzz smoke corpus. A real authorized Editor/project validation remains human-gated under T10; it is not implied by this metadata contract.

## T26 selected direct decoder

The optional selected direct decoder for a legacy tagged `UTexture2D` Asset Registry record is separately version-locked to UE 5.8.2. It reads no texture pixels and remains `requires_adapter`; its strict binary, oracle, and companion-data boundaries are documented in [`unreal_texture2d_metadata_contract.md`](unreal_texture2d_metadata_contract.md).

## CLI recovery status

The typed Clap surface accepts `inspect`, `plan`, `convert`, `verify`, `view`, `support-matrix`, and `version`, with text or JSON reports. During recovery, only Unreal `inspect`, `support-matrix`, and `version` execute a concrete adapter/query. The remaining commands parse and validate their typed inputs but report an explicit unavailable capability until their downstream canonical-IR, packaging, publication, and artifact-discovery tasks are rebuilt.
