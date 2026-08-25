# bevy_egui Setup Notes

The current `bevy_egui` README documents version **0.42.0** with Bevy **0.19.0** and shows the intended Bevy 0.19 setup: `EguiPlugin::default()` plus UI systems in `EguiPrimaryContextPass`. The dependency will be pinned to `0.42.0` for the T21 viewer layer.

The viewer will retain the existing Bevy asset/runtime lifecycle. `bevy_egui` is limited to rendering artifact and source trees, typed inspection data, diagnostic/classification filters, and explicit lifecycle controls. UI code will emit typed viewer commands; it will not contain conversion handlers.

On Linux, the upstream project documents XCB development libraries for desktop builds. The existing Vortex runtime already enables Bevy desktop rendering features, and UI setup will be validated by the normal Rust 1.97.1 preflight.

Sources:

- https://docs.rs/crate/bevy_egui/latest/source/README.md
- https://bevy.org/news/bevy-0-19/
