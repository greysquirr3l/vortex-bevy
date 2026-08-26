# Atomic Artifact Publication Contract

The T19 filesystem adapter publishes one completed conversion only through a **typed output root** and a conversion ID derived from the typed source identity and profile hash. It is a storage boundary: it receives a finalized conversion aggregate, a validated scene manifest, and byte payloads whose relative paths are checked before any filesystem mutation.

| Concern | Contract |
|---|---|
| Output confinement | `AtomicArtifactStore` canonicalizes its physical root. Payload paths must be non-empty normal relative path components; absolute paths, parent traversal, and ambiguous components are rejected. |
| Staging | Each attempt writes below `<output>/.staging/<conversion-id>-<sequence>`. Payload files are created, synchronized, and checked against manifest hashes before publication. |
| Publication | A complete staged directory is renamed once to `<output>/<conversion-id>`. Failed writes remove their owned staging directory and never replace an existing valid final directory. |
| Single writer | `<output>/.locks/<conversion-id>.lock` is acquired with `create_new`; contention returns a typed error without waiting, stealing, or deleting another writer’s state. |
| Replay | An existing directory is reused only when its persisted aggregate has the same conversion ID and profile hash and every declared manifest artifact verifies. |
| Verification | `verify` reads the bounded RON manifest, recomputes SHA-256 artifact hashes, and rejects missing or altered bytes. |
| Reporting | `conversion-report.json` serializes the typed conversion aggregate. It contains classifications, diagnostics, source identity, profile hash, version, and artifact hashes, but no credentials or environment variables. |
| Cleanup | Cleanup is idempotent and only targets a conversion directory or staging directories derived beneath the configured output root. It does not accept caller-controlled external paths. |

> Artifact equivalence is based on SHA-256 bytes and the explicit profile hash. It never uses a timestamp, current clock, or random value.

The generic `ArtifactStore` application port remains intentionally narrow because a completed domain aggregate contains artifact **hashes**, not the payload bytes required for physical publication. The atomic adapter’s `publish(PublicationRequest)` is therefore the complete publication boundary used by packaging/orchestration work; it rejects incomplete aggregate-only persistence rather than fabricating partial output.

## Verified Failure Semantics

The self-authored T19 suite demonstrates successful publish/replay/report retrieval, staged-write interruption, checksum mismatch, lock contention, altered final bytes, malformed reports, idempotent cleanup, and an external-directory preservation check. A failure leaves either the prior verified final conversion or no final conversion directory. It never publishes a partial final directory.

## Scope Limit

The adapter supplies durable local filesystem semantics where the underlying filesystem supports rename and file synchronization. It does not claim distributed locking, remote object-store transactions, or cross-device rename support. Those require a later, explicitly selected storage adapter.
