# Scene manifest schema

Every published Vortex artifact set carries a versioned
`scene.manifest.ron` sidecar beside its GLB and related files. This
page documents the manifest schema, the determinism contract, and the
typed provenance model.

For the full field-by-field contract, see
[`docs/scene_manifest_contract.md`](../../../docs/scene_manifest_contract.md).
For the runtime's loader, see
[`docs/bevy_runtime_loading_contract.md`](../../../docs/bevy_runtime_loading_contract.md).

## Versioning

The initial schema version is **`1`**. Readers accept exactly this
version and reject any other version before consuming artifact
references. A breaking change increments `schema_version`; an additive
optional field keeps the schema version only when older readers can
safely ignore it.

The version is encoded in the manifest itself as
`schema_version: 1`. Older readers that don't recognize newer versions
reject the manifest with a typed error rather than guessing.

## Top-level shape

```ron
(
    schema_version: 1,
    converter_version: "<semver>",
    profile_hash: "<sha256>",
    artifacts: [
        (
            kind: Glb,
            path: "scene.glb",
            hash: "<sha256>",
        ),
        ...
    ],
    sources: [
        (
            identity: <SourceIdentity>,
            asset_kind: "<kind>",
            selected_values: { ... },
            glb_labels: { ... },
            classification: <ConversionClassification>,
            diagnostics: [...],
        ),
        ...
    ],
    packaged_scenes: [...],
    compatibility: (
        minimum_bevy_version: "0.20",
        required_capabilities: [BevyGltfPbr, ...],
    ),
)
```

The fields are:

- `schema_version` — always `1` for this contract.
- `converter_version` — the vortex-bevy version that produced the
  artifacts. Useful for regression analysis.
- `profile_hash` — SHA-256 of the named export profile. Combined with
  the source identity to derive the conversion ID.
- `artifacts` — typed `ManifestArtifact` records. Each has a `kind`
  (`Glb`, `Texture`, or `Report`), a relative `path`, and a `hash`.
- `sources` — typed `ManifestSource` records. One per input source.
- `packaged_scenes` — explicit prebuilt alternatives and chunks. Empty
  for legacy single-scene manifests.
- `compatibility` — typed `CompatibilityRequirements`. The Bevy
  runtime rejects the manifest if its declared capabilities are not
  satisfied by the loaded Bevy build.

## Determinism

The manifest contains no wall-clock timestamps, host paths,
environment values, or randomly generated identifiers. Repeating the
conversion for the same typed source and profile must produce
byte-identical manifest RON and matching checksums.

The serializer applies deterministic ordering:

- `artifacts` sorted by `(kind, path)`.
- `sources` sorted by `identity_key()`.
- `packaged_scenes` sorted by id, with each package's `source_links`
  sorted by their string form.

This makes the manifest diff-friendly across runs and across hosts.

## Safety boundary

The manifest parser enforces:

- A bounded size limit (1 MiB default via
  `DEFAULT_MANIFEST_LIMIT_BYTES`).
- RON deserialization only. The parser never executes code.
- Validated artifact paths: nonempty, relative, no traversal segments,
  no NUL bytes.
- Validated typed source identities. Malformed project paths, USD prim
  paths, Unreal object paths, Unity asset GUIDs, and artifact hashes
  are rejected at deserialization.

A parsed manifest that fails any of these checks is rejected with a
typed error before the runtime loads any artifact.

## Provenance model

Each `ManifestSource` carries the typed identity for the input:

- Unreal: `SourceIdentity::Unreal { project_root, asset_path,
object_path }`
- USD: `SourceIdentity::Usd { stage, prim_path, selection }`
- Unity: `SourceIdentity::Unity { project_root, asset_path,
asset_guid, object_id }`
- Godot: `SourceIdentity::Godot { project_root, resource_path }`

Plus the source asset kind, selected values (USD variants, URP
profile facts), mappings to GLB labels, classification, and
diagnostics. The adapter implementation types for Unreal, USD, and
Unity never appear in the manifest.

## Classification model

`converted`, `baked`, `requires_adapter`, `unsupported`, and `rejected`
are the five explicit outcomes. Every outcome includes a `reason` and
a `remediation`. The CLI's `--format json` output uses the same five
values, so the same tooling can drive UI copy off either the manifest
or the CLI report.

## What the runtime verifies at load time

The Bevy runtime, before submitting any artifact path to Bevy's loader,
applies:

1. **Schema and size validation.** Rejects unknown schema versions and
   oversized manifests.
2. **Bevy version compatibility.** Rejects the manifest if its
   declared `minimum_bevy_version` is newer than the loaded Bevy build.
3. **Capability compatibility.** Rejects the manifest if any
   `required_capability` is not satisfied.
4. **Artifact checksum verification.** SHA-256 verifies each declared
   artifact against its actual contents.
5. **Path confinement.** Each declared artifact path must resolve
   beneath the manifest directory after canonicalization.

A failure at any step short-circuits to `load_published_conversion_and_spawn`
returning `Err`, and the Bevy runtime falls back to a
`VortexRuntimeDiagnostic` entity with the stable code
`vortex.runtime.artifact_rejected`.
