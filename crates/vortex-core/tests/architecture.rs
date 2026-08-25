//! Architecture boundary checks for the reusable core crate.

#[test]
fn core_manifest_does_not_depend_on_bevy() {
    let manifest = include_str!("../Cargo.toml");

    assert!(
        !manifest.lines().any(|line| line.trim_start().starts_with("bevy")),
        "vortex-core must remain independent of Bevy"
    );
}
