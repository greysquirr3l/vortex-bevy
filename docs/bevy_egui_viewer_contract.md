# bevy_egui Viewer Contract

The Vortex runtime uses `bevy_egui` **0.43.1** with Bevy **0.20** for its local scene-lifecycle inspector. Bevy remains responsible for the viewport, asset pipeline, dynamic glTF worlds, and runtime lifecycle. The UI only renders typed state and queues explicit lifecycle commands; it never embeds conversion handlers or source-format decoders.

## Viewer layout

The overlay is composed from four panels around the native Bevy viewport. The top toolbar exposes reload and unload for the selected verified Vortex root. The left source panel lists retained manifest sources and filters them by conversion classification. The right inspector displays the selected source identity, kind, classification, selected variants, and GLB source labels. The bottom panel displays actionable diagnostics filtered by their stable code prefix.

## State and command boundary

`VortexViewerState` owns a detached `RuntimeSceneInspection` snapshot, selected root, selected source row, optional classification filter, diagnostic prefix, and the last recoverable lifecycle or inspection error. `ViewerCommandQueue` carries only `Reload(Entity)` and `Unload(Entity)` requests. The runtime applies queued commands during `Update` through its existing bounded manifest/GLB lifecycle operations, then refreshes the inspection snapshot from the selected root.

This preserves the collapsed DDD/Hex boundary: the Bevy UI has access to presentation-ready provenance and status data but not to conversion commands, format SDK types, or shared-port abstractions.

## Safety and verification

A viewer reload delegates to the same manifest compatibility, canonical path, and SHA-256 checks used for initial scene loading. An unload succeeds only for a Vortex-marked root. Foreign entities, malformed manifests, checksum mismatches, and unsupported capability requirements remain fail-closed.

The headless runtime integration suite proves selection refresh, detached inspection availability, command-queued reload replacement, post-reload generation progression, unload reclamation, and the absence of a selected inspection after removal. The focused desktop suite additionally proves camera installation, bounded input behavior, framing, scale-aware clipping, and rendering disablement when no unobscured viewport remains.

## References

- [bevy_egui 0.43.1 README](https://docs.rs/crate/bevy_egui/latest/source/README.md)
- [bevy_egui side panel example](https://github.com/vladbat00/bevy_egui/blob/v0.42.0/examples/side_panel.rs)

## Native desktop viewport

The viewer installs exactly one `VortexViewerCamera` beside the `bevy_egui` overlay. **Right drag** orbits around the target, **middle drag** pans in the camera orientation plane, and the mouse wheel applies exponential bounded zoom. The `Frame` toolbar action and **F** key frame the selected verified root by traversing its loaded hierarchy, averaging valid global-transform positions, and applying a conservative radius.

The `ViewerInputGate` prevents native camera interaction while an `egui` control owns pointer input. The camera viewport is recomputed from measured top, bottom, left, and right panel extents, converted through the window scale factor, and applied as a physical Bevy camera viewport. This preserves the central 3D viewport without drawing beneath inspector panels. When panels consume the entire window, the camera is explicitly disabled rather than falling back to Bevy's unrestricted full-window viewport. The desktop window is titled `Vortex-Bevy Viewer`, starts at a resizable 1440×900 logical workflow size, and retains native operating-system window behavior.

Headless tests cover one-camera runtime installation, bounded orbit/pan/zoom state, framing, scaled viewport calculation, and panel-exhaustion camera disablement. Visual interaction should additionally be smoke-tested on each supported desktop platform during release verification.
