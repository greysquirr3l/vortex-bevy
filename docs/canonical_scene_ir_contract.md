# Canonical Scene IR Contract

## Purpose

`vortex-core::domain::scene_ir` is the engine-neutral boundary between source adapters and deterministic packaging. It has no filesystem, subprocess, Bevy, Unreal SDK, OpenUSD SDK, Unity API, or parser dependency. Source-specific details remain in `SourceLink`; renderable facts use typed numeric IDs instead of string-keyed graph references.

## Model

The model contains `CanonicalScene`, `Node`, `Transform`, `Mesh`, `Primitive`, `VertexStream`, `Material`, `Texture`, `Sampler`, `Skin`, `Joint`, `AnimationClip`, `Channel`, `MorphTarget`, `Camera`, `Light`, `Instance`, `SourceLink`, and `UnsupportedFeature`. Every cross-object relationship uses a dedicated ID newtype such as `NodeId`, `MeshId`, or `MaterialId`.

| Concern | Contract |
|---|---|
| Identity | IDs are stable `u32` newtypes; duplicate IDs are rejected. |
| Render hierarchy | Nodes may name an optional parent, mesh, camera, light, or instance. Parent cycles are rejected. |
| Geometry | Every primitive must reference a vertex stream. Indices must be within the stream’s position range. |
| Materials | Primitive material slots and material texture slots must resolve to typed IDs. |
| Deformation | Mesh skin references, skin joints, and joint nodes must all resolve. |
| Animation | Clips reference channels; channels target nodes; every key time/value must be finite and nondecreasing. |
| Provenance | Every node has a typed `SourceLink`; engine-specific source facts are a deterministic `BTreeMap`. |
| Unsupported features | Remain explicit objects carrying an actionable conversion classification. |

## Determinism and normalization

`CanonicalScene::new` sorts all collections by typed ID and validates referential integrity in one construction step. `to_canonical_json` then uses fixed struct field order, sorted vectors, and `BTreeMap` source/material facts to produce byte-stable JSON for semantically identical graphs.

The `CoordinateNormalization` record preserves source metric, source/target up axes, winding, UV origin, and color-space decisions. Adapters record normalization rather than hiding it in source-specific render data. The subsequent GLB packaging stage is responsible for consuming these owned normalized values.

> Source adapters may describe equivalent render graphs while retaining distinct provenance. The IR deliberately makes that distinction visible: graph geometry/material/node values can be equivalent while their `SourceLink` values differ.

## Validation guarantees

The constructor rejects dangling IDs, duplicate IDs, parent cycles, invalid skin joints, absent material slots, nonfinite transforms/material/camera/light values, invalid animation times, and invalid vertex indices. A failed constructor returns no partially validated scene.

`crates/vortex-core/tests/canonical_scene_ir.rs` verifies deterministic canonical JSON, graph failure diagnostics, and render-graph equivalence between a synthetic Unreal source identity and a synthetic USD source identity.
