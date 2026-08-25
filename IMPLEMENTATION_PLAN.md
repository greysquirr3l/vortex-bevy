# vortex-bevy — Implementation Plan

## Overview

Vortex-Bevy is a Rust 1.97.1 / Bevy 0.19 asset-conversion and runtime-viewing
workspace for authorized Unreal Engine source projects and OpenUSD stages. It
inspects Unreal package metadata, invokes a minimal Unreal Editor automation
bridge for authoritative export, resolves configured USD stage views, compiles
a typed intermediate representation, packages Bevy-compatible GLB and textures,
and loads the result with source-traceable metadata in Bevy.

The product explicitly distinguishes editable source-project assets from loose,
cooked, encrypted, or proprietary payloads. It never promises functional
Blueprint/C++ conversion. It emits an actionable per-asset classification:
converted, baked, requires_adapter, unsupported, or rejected.


**Architecture**: collapsed-ddd-hexagonal
**Language**: rust

---

## Phases


### Phase 1 — Repository and Toolchain Foundation

1. **T01 — Pin the Rust 1.97.1 workspace and collapsed architecture**
   Create the Vortex-Bevy Cargo workspace, exact stable toolchain pin, minimal crate topology, cross-platform repository hygiene, and a compiling no-feature baseline.
2. **T02 — Install quality gates, CI, security checks, and conventional commits**
   Make every change reproducibly enforce Rust quality, fixture safety, licensing checks, and cross-platform compatibility before feature work begins.
   _Depends on: workspace-foundation_
3. **T03 — Define deterministic owned fixture contracts and golden artifact rules**
   Create synthetic and self-authored fixture families that support the entire converter without depending on proprietary Unreal or game content.
   _Depends on: quality-ci-and-hooks_


### Phase 2 — Core Domain and Application Contracts

1. **T04 — Model conversion identities, profiles, classifications, and diagnostics**
   Implement a pure domain model for source identity, conversion policy, output identity, classification, diagnostics, and deterministic conversion state transitions.
   _Depends on: fixture-contract_
2. **T05 — Create narrow consumer-owned conversion use cases**
   Implement typed command/query handlers for inspection, planning, conversion, report retrieval, and deterministic artifact verification using narrow handler-local ports.
   _Depends on: conversion-domain_
3. **T06 — Specify the versioned scene manifest and compatibility contract**
   Define the stable scene.manifest.ron contract that carries provenance and conversion behavior beyond glTF while remaining safe and forward-compatible.
   _Depends on: conversion-domain, application-commands-and-queries_


### Phase 3 — Input Adapters and Canonical Representation

1. **T07 — Build a typed CLI for inspect, plan, convert, verify, and view**
   Expose the product through a clear clap CLI that parses all user input into typed commands and gives actionable non-panicking errors.
   _Depends on: application-commands-and-queries, manifest-contract_
2. **T08 — Implement Rust-first Unreal package inspection and preflight**
   Use a version-pinned Rust package reader behind an adapter to inspect authorized .uasset metadata, identify dependencies and companion-file requirements, classify input eligibility, and produce a deterministic inspection report.
   _Depends on: cli-and-input-boundaries_
3. **T09 — T09a — Implement Unreal Editor bridge protocol and synthetic tests**
   Implement the isolated, versioned Unreal Editor bridge protocol, Rust process adapter, typed bridge-report parser, and synthetic test suite without requiring a local Unreal Editor installation.
   _Depends on: uasset-inspection_
4. **T10 — T09b — Validate authorized Unreal Editor export integration**
   Run the versioned bridge against an authorized editable Unreal project and matching Editor, then verify that authoritative AssetExportTask output satisfies the bridge contract.
   _Depends on: unreal-editor-bridge-protocol_
5. **T11 — Resolve selected OpenUSD stage configurations into typed source data**
   Add a source adapter for USD stage inspection and deterministic selection of layers, references, payload policy, variants, purpose, units, axis, instancing, and time range.
   _Depends on: cli-and-input-boundaries, manifest-contract_
6. **T12 — Build the canonical scene intermediate representation**
   Translate Editor bridge and USD source facts into one typed, engine-neutral scene IR that cleanly feeds GLB packaging and Bevy runtime metadata.
   _Depends on: unreal-editor-bridge-protocol, usd-stage-ingestion_
7. **T13 — Ingest authorized Unity URP project source into typed canonical records**
   Add a secondary adapter that inspects authorized editable Unity project scene, prefab, material, texture, and metadata files, then produces owned typed Unity/URP source facts for the existing canonical scene IR.
   _Depends on: canonical-scene-ir, cli-and-input-boundaries, manifest-contract_
8. **T14 — Validate the Unity URP adapter against an authorized Editor project**
   Validate the URP source adapter and optional versioned Unity Editor export bridge against an authorized editable Unity project with a known Editor and URP package version.
   _Depends on: unity-urp-source-ingestion_


### Phase 4 — Artifact Conversion and Packaging

1. **T15 — Emit deterministic Bevy-compatible GLB mesh and scene assets**
   Serialize validated canonical scene IR into a deterministic GLB that uses Bevy 0.19’s supported glTF subset and carries stable labels for runtime lookup.
   _Depends on: canonical-scene-ir_
2. **T16 — Normalize textures and map constrained PBR materials**
   Convert supported source textures and simple Preview Surface/material-instance data into Bevy-compatible GLB PBR material references with explicit baking and unsupported-feature reporting.
   _Depends on: mesh-and-gltf-packaging_
3. **T17 — Map constrained Unity URP Lit and Unlit materials to Bevy PBR artifacts**
   Map approved Unity URP Lit and Unlit source facts into the existing canonical material and Bevy-compatible GLB PBR contracts while preserving exact provenance and feature classifications.
   _Depends on: texture-and-material-normalization, unity-urp-source-ingestion_
4. **T18 — Add validated skins, conventional clips, and morph targets**
   Convert renderable skeletal data, animation clips, and blend shapes when the source adapter supplies conventional data; otherwise report precise limitations.
   _Depends on: mesh-and-gltf-packaging, texture-and-material-normalization_
5. **T19 — Atomically publish artifacts, reports, and replay-safe conversion state**
   Implement secondary adapters that safely stage, verify, atomically publish, clean, and report conversion artifacts with idempotent behavior.
   _Depends on: manifest-contract, mesh-and-gltf-packaging, texture-and-material-normalization, skinning-animation-and-morphs_


### Phase 5 — Bevy 0.19 Runtime and End-to-End Experience

1. **T20 — Load Vortex artifacts into Bevy 0.19 with provenance components**
   Implement the Bevy 0.19 runtime adapter that validates the manifest, loads GLB scenes through current Bevy APIs, spawns them with WorldAssetRoot, and attaches Vortex provenance/classification components.
   _Depends on: artifact-store-and-reports_
2. **T21 — Ship the Bevy viewer with scene, source, and diagnostic inspection**
   Provide a practical desktop viewer mode that opens Vortex artifact directories, frames a scene, exposes source provenance and conversion diagnostics, and supports safe variant/selection inspection.
   _Depends on: bevy-manifest-loader_
3. **T22 — Prove the complete conversion pipeline with golden end-to-end suites**
   Exercise inspect → plan → source adapter → canonical IR → package → verify → Bevy load across owned fixtures and a gated authorized Unreal sample project.
   _Depends on: viewer-and-inspector_
4. **T23 — Prove Unity URP fixture conversion end to end**
   Exercise the self-authored Unity URP path from deterministic source inspection through canonical IR, GLB packaging, manifest provenance, and headless Bevy loading in normal CI without Unity Editor access.
   _Depends on: end-to-end-conversion-suite, unity-urp-lit-material-mapping_
5. **T24 — Validate Unity URP conversion against an authorized Editor project**
   Validate an approved Unity URP project through the same conversion, packaging, provenance, and Bevy-loading contract proven by the ungated fixture suite.
   _Depends on: unity-urp-fixture-end-to-end-suite, unity-urp-editor-integration_


### Phase 6 — Full Capability Completion

1. **T25 — Complete selected USD variants, instances, payload policy, and scene chunking**
   Turn selected USD composition facts into explicit Bevy runtime behavior where useful, while preserving a deterministic flattened artifact as the fallback.
   _Depends on: end-to-end-conversion-suite_
2. **T26 — Expand direct authorized asset decoding only where it beats Editor export**
   Add direct Rust decoding for selected well-tested Unreal asset classes only when it provides a material advantage over the authoritative Editor bridge, while maintaining exact scope reporting.
   _Depends on: end-to-end-conversion-suite_
3. **T27 — Measure, optimize, and observe full conversion/runtime behavior**
   Add evidence-driven performance benchmarks, structured tracing, memory limits, and reproducible diagnostics for the complete pipeline.
   _Depends on: usd-variants-instancing-and-streaming, direct-asset-decoder-expansion_


### Phase 7 — Release, Documentation, and Independent Audits

1. **T28 — Publish complete architecture, support, operations, and migration documentation**
   Produce the documentation required to use, extend, validate, and operate Vortex-Bevy without reading implementation details.
   _Depends on: performance-and-observability_
2. **T29 — Perform release hardening, reproducibility verification, and supply-chain review**
   Audit the complete workspace for release readiness across correctness, provenance, reproducibility, security, licensing, and platform behavior.
   _Depends on: documentation-and-compatibility-matrix_
3. **T30 — Security hardening and vulnerability review**
   Verify and enforce the six OWASP baseline security properties across the entire codebase before declaring the project complete.
   _Depends on: release-hardening_
4. **T31 — Integration wiring audit**
   Verify all components are properly connected and wired together. AI-generated code often creates modules that compile but aren't actually integrated into the application — this task catches those gaps.
   _Depends on: security-hardening_
5. **T32 — Stub and placeholder cleanup**
   Find and replace all stub implementations, placeholder code, TODO markers, and unimplemented functions. AI-generated code frequently leaves behind placeholder implementations that compile but don't actually work.
   _Depends on: integration-wiring_


---

## Preflight Commands

```bash
cargo build --workspace --all-targets --all-features
cargo test --workspace --all-features
cargo fmt --all -- --check && cargo l
```

---

_Generated by [wiggum](https://github.com/greysquirr3l/wiggum)._
