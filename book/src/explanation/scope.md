# Scope and authorization

Vortex-Bevy is an **authorized asset-conversion and runtime-viewing
workspace**. This page documents what that means, what is in scope, and
what is explicitly out of scope.

The product description says "**authorized** Unreal Engine source-project
assets and selected OpenUSD stages". "Authorized" is the key word —
every conversion requires the caller to have the right to supply the
input and to explicitly select the editable project root, asset path,
output root, and (where applicable) Unreal object path. Vortex-Bevy does
not bypass that authority.

For the security boundary in detail, see
[Security model](security.md). For the architecture in detail, see
[Architecture overview](architecture.md).

## What Vortex-Bevy does

Vortex-Bevy:

- Parses and packages selected OpenUSD stage facts.
- Packages authorized Unreal controlled static-scene exports produced
  by the matching Unreal Editor.
- Packages authorized Unity URP static-scene exports produced by the
  matching Unity Editor.
- Loads and renders the resulting GLBs in a Bevy 0.19 desktop viewer.

The product emits a per-asset classification — `converted`, `baked`,
`requires_adapter`, `unsupported`, or `rejected` — and a typed
remediation hint that names the next step.

## What Vortex-Bevy does not do

Vortex-Bevy is **not** a general-purpose `.uasset` / `.unity` / `.tscn` /
`.usd` decoder. It:

- does not bypass encryption or guess keys,
- does not claim Blueprint/C++ equivalence,
- does not perform live OpenUSD composition (payloads, recursive
  references, unselected variants),
- does not expose an HTTP server, fetch URLs, or connect to a database,
- does not run async or networked code paths,
- does not author arbitrary USD variants at runtime.

Inputs that fall outside the documented support boundary are detected
and reported with a typed classification. They never silently
reclassify to `converted` and never silently coerce to a partial
result.

## Authorization model

An input is authorized only when the caller has the right to supply it
and explicitly selects its editable project root, asset/stage path,
output root, and (where applicable) Unreal object path. The CLI parses
each of these into a typed domain value object before any filesystem
work.

The bounded ingestion path enforces root confinement, canonicalization,
symlink checks, size limits, manifest validation, checksum verification,
and atomic publication. These prevent an accepted request from
implicitly widening that authority.

The project refuses to:

- bypass encryption or access controls,
- infer semantic equivalence for cooked, encrypted, Blueprint, C++, or
  unknown package payloads,
- treat key-dependent content as if it were open.

When in doubt, the orchestrator surfaces a typed `requires_adapter`
classification with a remediation hint that names the specific adapter
to invoke. The CLI does not silently proceed.

## Cooked / encrypted / unowned / DRM-protected inputs

These are detected and reported as `rejected`. Vortex-Bevy never
attempts to bypass access controls. The remediation in every `rejected`
report names the explicit authorization that must be supplied — usually
"recheck the explicitly authorized project root and asset path".

For Unreal, this means:

- `.pak`, `.utoc`, `.ucas`, `.uexp`, `.ubulk` sidecars cause a
  fail-closed `rejected` classification.
- Cooked `.uasset` files (without the editable project structure) are
  detected and rejected.
- Packages that declare external bulk data beyond their own file but
  lack the required `.ubulk` companion are rejected.

## Bevy runtime invariants

The Bevy runtime also enforces the authorization boundary at load
time:

- The manifest's `required_capabilities` array must be satisfied by the
  loaded Bevy build. A manifest that requires a capability the Bevy
  build does not have is rejected.
- The artifact checksum is verified before spawning
  `WorldAssetRoot`. A mismatch is rejected.
- Artifact paths must resolve beneath the manifest directory after
  canonicalization. A traversal is rejected.

These are documented at
[`docs/bevy_runtime_loading_contract.md`](../../docs/bevy_runtime_loading_contract.md)
and exercised by the integration suite in `crates/vortex-bevy/tests/`.

## Where to go next

- [Security model](security.md) — the offline threat model and the
  audit findings
- [Architecture overview](architecture.md) — the collapsed DDD/Hex
  layout and the port trait design
- [Performance and observability](performance.md) — what the project
  measures and what it deliberately does not measure
