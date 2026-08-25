# Scene manifest contract

Every published Vortex artifact set carries a versioned `scene.manifest.ron` sidecar beside its GLB and related files. The manifest preserves conversion facts that glTF cannot express without source-specific extensions: source identity, profile hash, converter version, source selections, conversion classifications, diagnostics, artifact hashes, and explicit Bevy compatibility requirements.

## Versioning and compatibility

The initial schema version is **1**. Readers accept exactly this version and reject future or unsupported versions before consuming artifact references. A breaking change increments `schema_version`; an additive optional field keeps the schema version only when older readers can safely ignore it. Any artifact requiring a capability unavailable in the selected Bevy/runtime profile is rejected with a typed diagnostic rather than silently approximated.

## Determinism

The manifest contains no wall-clock timestamps, host paths, environment values, or randomly generated identifiers. Artifacts are ordered by kind and relative path; sources are ordered by their typed source identity; maps use stable key ordering. Repeating conversion for the same typed source and profile must produce byte-identical manifest RON and matching checksums.

## Safety boundary

Manifest parsing is bounded to **1 MiB** by default. It accepts RON data only and never deserializes executable behavior. Artifact paths must be nonempty, relative, and free of absolute roots, traversal segments, empty components, and NUL bytes. Source identities use validated typed value objects, so malformed project paths, USD prim paths, Unreal object paths, Unity asset GUIDs, and artifact hashes are rejected at deserialization boundaries.

## Provenance model

Each source record holds a typed identity for Unreal, USD, or Unity; the source asset kind; selected values such as USD variants or URP profile facts; mappings to GLB labels; classification; and diagnostics. Unreal, USD, and Unity adapter implementation types do not appear in the canonical IR or manifest APIs.

## Classification model

`converted`, `baked`, `requires_adapter`, `unsupported`, and `rejected` are explicit outcomes. Every outcome includes a reason and actionable remediation. Features such as Blueprint/C++ behavior, custom shader graphs, arbitrary USD composition, or nonrepresentable URP renderer effects must remain visible in this model instead of being silently omitted.
