# Changelog

All notable repository changes are documented here. Versioning follows the support boundaries published in `docs/compatibility_matrix.json`; a capability is never implied by a changelog entry alone.

## Unreleased

### Added

- An optional caller-owned in-process queue for explicitly selected local `.usda` and `.usda.gz` stage requests.
- Bounded non-blocking queue admission, monotonic local submission IDs, per-worker conversion-governor reservations, explicit worker shutdown, and one typed result channel per request.
- Bounded gzip decoding that rejects malformed streams and both compressed and decompressed inputs that exceed the configured limit before stage parsing.
- Deterministic integration tests covering local plain/gzip parity, production-fixture gzip classification, queue configuration, expansion rejection, malformed compression, mismatched encoding, and channel capacity.
- Direct-versus-queued production USD timing evidence and a documented bottleneck analysis.
- A repeatable release-readiness script, action pins, release recovery guidance, and cargo-geiger audit evidence.

### Changed

- Stateless USDA parsing and root-confined stage-file resolution are now crate-private reusable operations. Their validated behavior and external support boundary are unchanged.
- CI action references use immutable commits rather than mutable version tags.

### Security

- The queue introduces no network ingress, arbitrary archive traversal, output publication, raw source telemetry, or `unsafe` code.
- The release guide records the third-party unsafe-code audit as a finding and requires re-evaluation when dependencies change.

### Compatibility

- The streaming queue does not add full OpenUSD composition, arbitrary binary USD support, recursive reference/payload traversal, pixel decode, GPU upload, or an Editor-free semantic export path.
