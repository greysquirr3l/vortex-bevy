# Architecture overview

Vortex-Bevy is a two-crate Rust 1.97.1 workspace. The two crates
deliberately split along a boundary that keeps the conversion pipeline
free of Bevy, filesystem, subprocess, and source-SDK dependencies.

For the contract-level architecture, see
[`docs/operations_and_architecture_guide.md`](../../docs/operations_and_architecture_guide.md).
For the architecture audit, see
[`docs/integration_wiring_audit_t31.md`](../../docs/integration_wiring_audit_t31.md).

## Workspace layout

```text
crates/
├── vortex-core/  # Pure domain, application commands and queries,
│                  # and concrete adapter boundaries.
└── vortex-bevy/  # CLI and Bevy 0.19 runtime composition.
```

`vortex-core` contains the conversion domain (typed value objects,
identity derivation, scene IR, manifest schema, classification), the
application use cases (commands and queries), and the adapter
boundaries that consume source formats and publish artifacts. It has no
dependency on Bevy, the filesystem, subprocess management, or any
source SDK.

`vortex-bevy` contains the typed CLI surface and the Bevy 0.19 runtime
composition: `bevy_egui` overlay, asset loader, scene lifecycle, and
viewer camera. It consumes `vortex-core` through its public API only.

## Collapsed DDD/Hex layout

Inside `vortex-core`, the source tree follows a collapsed
Domain-Driven Design / Hexagonal architecture:

```text
crates/vortex-core/src/
├── domain/             # Pure domain values, invariants, deterministic state.
│   ├── conversion.rs   # ConversionAggregate, ClassificationKind, Diagnostic
│   ├── identity.rs     # SourceIdentity, ArtifactHash, ConversionId, ...
│   ├── manifest.rs     # SceneManifest, ManifestSource, CompatibilityRequirements
│   ├── scene_ir.rs     # CanonicalScene, Node, Mesh, Material, VertexStream, ...
│   ├── observability.rs
│   └── packaged_scene.rs
├── application/        # Use cases and their consumer-owned dependencies.
│   ├── command/        # Stateful commands
│   │   ├── inspect_input.rs       # InputInspector trait + handle fn
│   │   ├── plan_conversion.rs     # ExistingConversionReader trait + handle fn
│   │   ├── execute_conversion.rs  # ConversionRunner + ArtifactStore traits + handle fn
│   │   ├── verify_artifacts.rs    # ArtifactVerifier trait + handle fn
│   │   ├── clean_output.rs        # OutputCleaner trait + handle fn
│   │   └── stream_usd.rs
│   └── query/          # Read-only queries
│       ├── support_matrix.rs
│       └── conversion_report.rs
└── adapters/           # Concrete inbound and outbound boundaries.
    └── secondary/      # All current adapters live here; primary lives
                         # in the vortex-bevy binary crate (cli.rs).
        ├── artifact_store.rs         # AtomicArtifactStore
        ├── conversion_pipeline.rs    # ConversionPipeline (orchestrator)
        ├── glb.rs                     # GlbWriter
        ├── texture.rs                 # TextureNormalizer
        ├── usd_stage.rs               # UsdStageInspector
        ├── usd_packages.rs            # UsdPackagePlanner
        ├── unreal_bridge.rs           # UnrealBridge + UnrealBridgeProcess
        ├── unreal_package.rs          # UnrealPackageInspector
        ├── unreal_static_scene.rs     # Unreal static-scene adapter
        ├── unreal_texture2d_metadata.rs
        ├── unity.rs                   # Unity Force Text adapter
        ├── unity_static_scene.rs      # Unity static-scene adapter
        ├── godot_static_scene.rs
        ├── urp_material.rs            # UrpMaterialMapper
        └── linux_process_memory.rs    # T27 observability
```

The key invariant is that **every domain module imports nothing from
Bevy, the filesystem, a subprocess layer, an SDK, or any source format**.
Domain logic receives time, platform facts, and conversion context
explicitly.

## Port traits are consumer-owned

The Hexagonal / Ports-and-Adapters tradition puts port traits in a
shared `ports/` module shared by both the application and the adapters.
The collapsed layout used here keeps the port trait beside the
command or query that consumes it. There is no shared `ports/` directory
and no shared ports crate.

The eight application port traits:

| Trait                      | Defined in                      | Adapter                     |
| -------------------------- | ------------------------------- | --------------------------- |
| `InputInspector`           | `command/inspect_input.rs`      | `UnrealPackageInspector`    |
| `SupportMatrixReader`      | `query/support_matrix.rs`       | `StaticSupportMatrixReader` |
| `ArtifactStore`            | `command/execute_conversion.rs` | `AtomicArtifactStore`       |
| `ArtifactVerifier`         | `command/verify_artifacts.rs`   | `AtomicArtifactStore`       |
| `OutputCleaner`            | `command/clean_output.rs`       | `AtomicArtifactStore`       |
| `ConversionReportReader`   | `query/conversion_report.rs`    | `AtomicArtifactStore`       |
| `ExistingConversionReader` | `command/plan_conversion.rs`    | `AtomicArtifactStore`       |
| `ConversionRunner`         | `command/execute_conversion.rs` | `ConversionPipeline`        |

Plus one consumer-owned port declared inside the bridge itself:

| Trait                 | Defined in         | Adapter                     |
| --------------------- | ------------------ | --------------------------- |
| `UnrealBridgeProcess` | `unreal_bridge.rs` | `NativeUnrealBridgeProcess` |

The collapsed layout has three consequences worth knowing:

1. **Adapter replacement is local.** Replacing `AtomicArtifactStore`
   with another artifact store does not require changing any domain
   code — only the command/query handler that consumes the trait.
2. **Tests use trait-bound fakes.** Unit tests construct typed fakes
   that implement the trait under test. The integration suite also
   uses trait-bound fakes for deterministic replay.
3. **Domain code does not know about engines.** A future addition
   that adds a new engine does not require touching any domain module —
   it adds a new adapter and wires it into the CLI.

## The conversion pipeline (orchestrator)

`ConversionPipeline` in `adapters/secondary/conversion_pipeline.rs` is
the consumer of `ConversionRunner`. For each engine, it:

1. **USD** — parses the selected USDA stage via `UsdStageInspector`,
   builds a `CanonicalScene` from the prim hierarchy, packages it as a
   Bevy-compatible GLB via `GlbWriter`, builds a version-1
   `SceneManifest`, and publishes the conversion through
   `AtomicArtifactStore::publish`. The returned
   `ConversionAggregate` is classified `converted` and records both the
   GLB and the manifest.
2. **Unreal / Unity / Godot** — classify `RequiresAdapter` with
   explicit remediation pointing at the authorized editor bridge.
   Bridge reports are produced by `bridges/*` scripts running in the
   engine process and consumed by
   `unreal_static_scene` / `unity_static_scene` / `godot_static_scene`.
   Wiring those bridge reports through the same orchestrator is the
   natural extension point once CLI flags for the bridge report path
   land.

The orchestrator fails loudly when a source exceeds the glTF
`u32` `NodeId` capacity (~4 billion prims), carrying a typed
remediation that names the next step rather than silently truncating.

## Atomic publication

`AtomicArtifactStore::publish` is the only path that creates a
published conversion directory. It stages payloads into a per-conversion
staging directory under `.staging/<conversion_id>-<sequence>`, writes
the manifest RON, writes the report JSON, fsyncs the staging directory,
renames it to the final `<conversion_id>/` directory, and fsyncs the
output root. Failed attempts clean their staging directory and never
replace an existing final conversion directory.

The store also implements `ExistingConversionReader` (the lookup key
is the conversion ID, which is deterministically derived from
`SHA-256(source || profile_hash)`). Same source plus same profile
equals same ID, so compatibility is unambiguous.

## Bevy runtime composition

`vortex-bevy/src/runtime/mod.rs` builds a headless-capable Bevy 0.19
application configured to load assets from one directory. The
runtime registers the standard reflected components emitted by
dynamic glTF worlds (transform, visibility, hierarchy, name components,
camera, light, mesh, material) before asset loading. This is required
for current `WorldAssetRoot` instantiation in a headless-capable
application.

The runtime side is composed from six focused modules:

- `components.rs` — `VortexSceneRoot`, `VortexSourceLink`,
  `VortexConversionStatus`, `VortexArtifactHash`,
  `VortexDiagnosticSet`, `VortexVariantSelection`,
  `VortexPackagedScenes`, `VortexSceneChunk`,
  `VortexPackagedVariant`, `VortexSourceRecord`,
  `VortexRuntimeDiagnostic`.
- `lifecycle.rs` — `VortexSceneLifecycle`, `reload_scene`, `unload_scene`.
- `packages.rs` — `load_packaged_chunk`, `switch_packaged_variant`.
- `inspection.rs` — detached `RuntimeSceneInspection` snapshots.
- `viewer.rs` — `VortexViewerPlugin`, `VortexViewerState`,
  `ViewerCommandQueue`, `update_viewer_state`.
- `desktop.rs` — `VortexDesktopViewerPlugin`, native camera,
  `ViewerInputGate`, `apply_input_delta`, `apply_viewport_state`,
  `frame_points`, `ViewerPanelLayout`.

The UI only renders typed state and queues explicit lifecycle
commands; it never embeds conversion handlers or source-format decoders.

## Where to go next

- [Scope and authorization](scope.md) — what is in and out of scope
- [Security model](security.md) — the offline threat model and audit
- [Performance and observability](performance.md) — what the project
  measures
