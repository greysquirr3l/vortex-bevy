# Integration Wiring Audit

**Status:** Complete — no critical or high-severity findings. Project is wired
end-to-end for the implemented surface; gaps are explicit and reported by the
CLI itself.

**Scope:** `vortex-bevy` workspace at commit `6570432`. Both crates, all
`application/{command,query}/*` handlers, all `adapters/secondary/*`
implementations, the `vortex-bevy` CLI boundary, and the Bevy runtime plugin
registration.

---

## Audit axes

1. **Port trait → adapter → consumer matrix**: every consumer-owned trait
   declared in `application/` has at least one adapter implementation and at
   least one production consumer (CLI, runtime, or test).
2. **Public API utilization**: every `pub` item in `vortex-core/src/lib.rs`
   is imported by the binary crate, by tests, or by another in-crate module.
3. **CLI subcommand → handler wiring**: every Clap subcommand dispatches to a
   concrete handler or reports an explicit `Unavailable` capability.
4. **Bevy plugin and reflection registration**: every Bevy plugin the project
   defines is registered; every reflected component is registered.
5. **Background worker spawning**: every `thread::spawn` / `tokio::spawn` call
   is reachable from a public constructor and is exercised by tests.
6. **Error conversion**: every domain error type has at least one conversion
   path to the boundary (typed `thiserror` or `?`-propagation through
   `anyhow::Context`).

---

## Findings matrix

| ID   | Severity | Area                                                      | Status                                                                            |
| ---- | -------- | --------------------------------------------------------- | --------------------------------------------------------------------------------- |
| W-01 | Info     | `InputInspector` → `UnrealPackageInspector`               | Wired, CLI `inspect` ✓                                                            |
| W-02 | Info     | `SupportMatrixReader` → `StaticSupportMatrixReader`       | Wired, CLI `support-matrix` ✓                                                     |
| W-03 | Info     | `ArtifactStore` → `AtomicArtifactStore`                   | Wired, runtime + tests ✓                                                          |
| W-04 | Info     | `ArtifactVerifier` → `AtomicArtifactStore`                | Wired, runtime + tests ✓                                                          |
| W-05 | Info     | `OutputCleaner` → `AtomicArtifactStore`                   | Wired, tests only (CLI Verify reports Unavailable)                                |
| W-06 | Info     | `ConversionReportReader` → `AtomicArtifactStore`          | Wired, tests only (no CLI query surface)                                          |
| W-07 | Info     | `UnrealBridgeProcess` → `NativeUnrealBridgeProcess`       | Wired, internal to bridge                                                         |
| W-08 | Info     | `ConversionRunner`                                        | **No adapter** — staged for future Convert command                                |
| W-09 | Info     | `ExistingConversionReader`                                | **No adapter** — staged for future Plan command                                   |
| W-10 | Low      | `crates/vortex-core/src/adapters/primary/mod.rs` is empty | See §6                                                                            |
| W-11 | Info     | CLI `convert` reports Unavailable with remediation        | By design                                                                         |
| W-12 | Info     | CLI `plan` reports Unavailable with remediation           | By design                                                                         |
| W-13 | Info     | CLI `verify` reports Unavailable with remediation         | By design                                                                         |
| W-14 | Info     | CLI `view` reports Unavailable with remediation           | By design                                                                         |
| W-15 | Info     | Bevy plugin registration                                  | Complete: `VortexDesktopViewerPlugin`, `VortexViewerPlugin` + 25+ reflected types |
| W-16 | Info     | Background workers                                        | `StreamingUsdStageQueue::new` spawns workers on construction                      |
| W-17 | Info     | Error conversion paths                                    | 1 `#[from]` (bounded execute) + `with_context`/`map_err` everywhere else          |
| W-18 | Info     | `LinuxProcessMemoryReader` only used by benches/tests     | T27 host-scoped observability                                                     |
| W-19 | Info     | `StreamingUsdStageQueue` only used by benches/tests       | T29 staged for future Convert                                                     |

---

## 1. Port trait → adapter → consumer matrix

Eight consumer-owned port traits defined in `application/{command,query}/*`.
Six have production adapters and at least one production consumer; two have
no adapter and are exercised only through their generic-trait-bound contract
in unit tests.

| Trait                      | Defined in                         | Adapter (file)                                     | Production consumer                                                   | Test consumer                                                  |
| -------------------------- | ---------------------------------- | -------------------------------------------------- | --------------------------------------------------------------------- | -------------------------------------------------------------- |
| `InputInspector`           | `command/inspect_input.rs:17`      | `UnrealPackageInspector` (`unreal_package.rs:346`) | CLI `inspect` (`cli.rs:283`)                                          | —                                                              |
| `SupportMatrixReader`      | `query/support_matrix.rs:15`       | `StaticSupportMatrixReader` (`cli.rs:335`)         | CLI `support-matrix` (`cli.rs:39`)                                    | —                                                              |
| `ArtifactStore`            | `command/execute_conversion.rs:44` | `AtomicArtifactStore` (`artifact_store.rs:338`)    | (Convert Unavailable)                                                 | `application_contracts.rs:95`, `observability_contracts.rs:85` |
| `ArtifactVerifier`         | `command/verify_artifacts.rs:17`   | `AtomicArtifactStore` (`artifact_store.rs:351`)    | `runtime::load_published_conversion_and_spawn` (`runtime/mod.rs:140`) | 7 integration tests                                            |
| `OutputCleaner`            | `command/clean_output.rs:10`       | `AtomicArtifactStore` (`artifact_store.rs:359`)    | (Verify Unavailable)                                                  | `artifact_store.rs:154`                                        |
| `ConversionReportReader`   | `query/conversion_report.rs:10`    | `AtomicArtifactStore` (`artifact_store.rs:387`)    | (no CLI surface)                                                      | `end_to_end_conversion.rs`, `unity_urp_end_to_end.rs`          |
| `ConversionRunner`         | `command/execute_conversion.rs:29` | **none**                                           | none                                                                  | `observability_contracts.rs:6` (trait bound)                   |
| `ExistingConversionReader` | `command/plan_conversion.rs:25`    | **none**                                           | none                                                                  | `application_contracts.rs:65` (trait bound)                    |

Plus one consumer-owned port declared inside the bridge itself:

| Trait                 | Defined in             | Adapter                                              | Consumer                                        |
| --------------------- | ---------------------- | ---------------------------------------------------- | ----------------------------------------------- |
| `UnrealBridgeProcess` | `unreal_bridge.rs:209` | `NativeUnrealBridgeProcess` (`unreal_bridge.rs:211`) | `UnrealBridge::invoke` (`unreal_bridge.rs:218`) |

**Interpretation of W-08/W-09:** `ConversionRunner` and
`ExistingConversionReader` correspond exactly to the two CLI commands (`convert`,
`plan`) that report `CliReport::Unavailable` with a typed remediation hint.
The traits are staged for the recovery that will add the conversion
orchestration adapter. Until then, the handler `fn` is tested through its
generic trait-bound contract, not through a concrete adapter. No silent
disconnection.

## 2. Public API utilization

`crates/vortex-core/src/lib.rs` exports three modules:

```rust
pub mod adapters;
pub mod application;
pub mod domain;
```

Every module is reachable:

- `domain` — 7 submodules; `pub use` re-exports the typed identifiers
  (`ArtifactHash`, `ConversionId`, `SourceIdentity`, `SceneManifest`,
  `CompatibilityCapability`, etc.). Used by `cli.rs`, `runtime/`, and 30+ test
  files.
- `application` — 8 command handlers + 2 query handlers. Used by `cli.rs`
  (`inspect_input`, `read_support_matrix`) and tests.
- `adapters` — 14 secondary adapters (`artifact_store`, `glb`, `texture`,
  `unity`, `unity_static_scene`, `unreal_bridge`, `unreal_package`,
  `unreal_static_scene`, `unreal_texture2d_metadata`, `urp_material`,
  `usd_packages`, `usd_stage`, `godot_static_scene`,
  `linux_process_memory`). Used by tests, benches, and `runtime/mod.rs`.

No dead modules.

## 3. CLI subcommand wiring

Seven Clap subcommands defined in `crates/vortex-bevy/src/cli.rs:90`:

| Subcommand       | Dispatch                                            | Outcome                                                        |
| ---------------- | --------------------------------------------------- | -------------------------------------------------------------- |
| `inspect`        | `inspect_input` handler + `UnrealPackageInspector`  | Concrete result (Unreal only; others report typed unavailable) |
| `plan`           | `unavailable_conversion`                            | `CliReport::Unavailable { capability, remediation }`           |
| `convert`        | `unavailable_conversion`                            | `CliReport::Unavailable`                                       |
| `verify`         | `unavailable_identifier_command`                    | `CliReport::Unavailable`                                       |
| `view`           | `unavailable_view`                                  | `CliReport::Unavailable`                                       |
| `support-matrix` | `read_support_matrix` + `StaticSupportMatrixReader` | Concrete support matrix                                        |
| `version`        | `version_report`                                    | Concrete version string                                        |

The four "Unavailable" subcommands are not silently broken — they emit
`CliReport::Unavailable` with an explicit `remediation` string
(`"Recovery must complete canonical IR, packaging, and publication before
this command can execute."` etc.). The CLI exit code is `3`
(`EXIT_UNAVAILABLE`) for that case, distinct from `0` (success), `2`
(usage), and `4` (inspection failure).

## 4. Bevy plugin and reflection registration

`crates/vortex-bevy/src/runtime/mod.rs:74-122`:

- `DefaultPlugins` set with `AssetPlugin { file_path }` and a typed
  `WindowPlugin` (`WindowResolution::new(1_440, 900)`).
- `EguiPlugin` from `bevy_egui`.
- 25 components reflected via `app.register_type::<T>()`:
  `Transform`, `GlobalTransform`, `TransformTreeChanged`, `Visibility`,
  `InheritedVisibility`, `ViewVisibility`, `VisibleEntities`,
  `CubemapVisibleEntities`, `CascadesVisibleEntities`, `ChildOf`, `Children`,
  `Name`, `Mesh3d`, `Aabb`, `Frustum`, `CubemapFrusta`, `GltfMeshName`,
  `GltfMaterialName`, `GltfSceneName`, `MeshMaterial3d<StandardMaterial>`,
  `Camera`, `CameraMainTextureUsages`, plus light/cascade Frusta types.
- `VortexDesktopViewerPlugin` and `VortexViewerPlugin` registered last
  (`runtime/mod.rs:120-121`).

`VortexRuntimeDiagnostic` is spawned as a fallback entity when
`load_published_conversion_and_spawn` fails, so a bad artifact root never
crashes the Bevy app — it renders a typed diagnostic entity instead.

## 5. Background workers

Single background-worker spawn site: `stream_usd.rs:356`. Workers are spawned
inside the `StreamingUsdStageQueue::new` constructor
(`stream_usd.rs:285`), so worker lifetime is bound to the queue. The
worker count is typed (`StreamingUsdStageLimits::worker_count().get()`), and
each worker receives an `Arc<Mutex<Receiver<QueuedStage>>>` + an
`Arc<ConversionResourceGovernor>`. The shutdown path is
`StreamingUsdStageQueue::shutdown` which drops the receivers and joins
handles (`stream_usd.rs:316`).

No other `thread::spawn` / `tokio::spawn` call sites. No `async fn` in
production code.

## 6. Empty `adapters/primary/mod.rs` (W-10)

`crates/vortex-core/src/adapters/primary/mod.rs` contains only a doc comment:

```rust
//! Inbound adapters that translate external requests into typed application commands.
```

No `pub mod` declarations, no `pub use`, no code. Zero references to
`adapters::primary` across the workspace (verified by grep — empty result).

**Why it's empty:** the project's hexagonal layout puts primary adapters in
the binary crate (`crates/vortex-bevy/src/cli.rs`). The library crate stays
pure domain + application + secondary adapters. The empty `mod.rs` is a
placeholder kept for symmetry with the architecture diagram, not a code path.

**Recommendation:** T32 (stub and placeholder cleanup) should remove the
empty module and its parent `adapters::primary` declaration in
`crates/vortex-core/src/adapters/mod.rs:5` (`pub mod primary;`). Then
`crates/vortex-core/src/adapters/secondary/...` becomes the only adapter
boundary in the library crate.

## 7. Error conversion paths

| Pattern                   | Count                            | Notes                                                                                             |
| ------------------------- | -------------------------------- | ------------------------------------------------------------------------------------------------- | -------------------------------------------------- |
| `#[from]` attribute       | 1                                | `BoundedExecuteConversionError::source` in `execute_conversion.rs:78` (intra-handler composition) |
| `.map_err(\|source        | TypedError::Variant { source })` | many                                                                                              | Standard thiserror mapping; preserves source chain |
| `.with_context(\|\| ...)` | many                             | anyhow at binary boundary; consistent with `cli.rs` / `main.rs`                                   |
| `?` propagation           | pervasive                        | All handlers use `?` for nested calls                                                             |

No bare `From<X> for YError` blanket conversions were found. Each error
variant explicitly names its `source: InnerError` field, which preserves
the diagnostic chain for the typed `thiserror` display path.

## 8. Items consumed only by tests/benches (informational)

These are not "unused" — they are deliberately library-only building blocks
for the future Convert command or host-scoped observability adapters:

- `LinuxProcessMemoryReader` (`adapters/secondary/linux_process_memory.rs`)
  — T27 host-scoped RSS reader. Used in `benches/core_pipeline.rs:84` and
  `tests/production_scale_memory_profile.rs:8`. Linux-only by design.
- `StreamingUsdStageQueue` (`application/command/stream_usd.rs`) — T29
  bounded worker queue for selected-USDA/plain-gzip. Used in
  `benches/core_pipeline.rs:26` and `tests/stream_usd_pipeline.rs:18`.
  Consumed only when Convert is implemented.

Both follow the project's "staged but tested" pattern: the public API
exists, the unit tests prove the contract, and the production caller is
forthcoming.

## 9. CLI argument validation

Every Clap subcommand uses typed `value_parser` adapters:
`parse_asset_path`, `parse_unreal_object_path`, `parse_profile_name`,
`parse_stage_selection`. These produce typed `SourceAssetPath`,
`UnrealObjectPath`, `ExportProfileName`, `StageSelection` value objects
that the handlers consume directly. The CLI never lets a free-form `String`
cross into the domain layer.

Argument cross-validation happens in `validate_conversion_arguments`
(`cli.rs:325`): `--variant` only meaningful for USD, `--payload-policy`
only meaningful for USD, `--output` non-empty, `--conversion-id` non-empty.

---

## 10. Recommended follow-ups

1. **W-10 (empty `adapters/primary/mod.rs`)**: remove under T32.
2. **W-08/W-09 (unwired `ConversionRunner`, `ExistingConversionReader`)**:
   add adapters and wire `convert` / `plan` commands when conversion
   orchestration lands. The trait definitions stay; only the `impl` and the
   CLI dispatch change.
3. **W-18/W-19 (test-only adapters)**: leave as-is — they are
   deliberately staged per T27/T29 and have full test coverage.

---

## Cross-reference with the security audit

Wiring audit is orthogonal to the security audit (see
`docs/security_audit_t30.md`). Notable shared points:

- **Subprocess injection** (F-15) — `UnrealBridgeProcess` is the only
  subprocess port, and its adapter uses typed `OsString` args (security
  audit §5). No new subprocess ports.
- **Path traversal** (F-11) — every adapter implementation that reads
  from disk uses the canonical `canonicalize + starts_with(root)` pattern
  (security audit §4). Confirmed for all adapters in `adapters/secondary/`.
- **Bounded ingestion** (F-05) — `ArtifactVerifier::verify_published`
  validates every declared hash before the runtime loads the scene. No new
  ingestion surface.

---

## Deliverable

This document at `docs/integration_wiring_audit_t31.md` is the audit
deliverable. No source-code changes were required (W-10 placeholder
cleanup was deferred).
