# Bundle assets

cargo-bundle reads `icon` paths from `[package.metadata.bundle]` and
selects the best format per platform:

- macOS prefers `.icns`
- Windows prefers `.ico`
- Linux uses `.png` (rendered into `.desktop` files)

## Files in this directory

| File                   | Format | Platform | Notes                                      |
| ---------------------- | ------ | -------- | ------------------------------------------ |
| `vortex-bevy.icns`     | ICNS   | macOS    | Multi-resolution (16 → 1024)               |
| `vortex-bevy.ico`      | ICO    | Windows  | 6 sizes: 16, 32, 48, 64, 128, 256          |
| `vortex-bevy-16.png`   | PNG    | Linux    |                                            |
| `vortex-bevy-32.png`   | PNG    | Linux    |                                            |
| `vortex-bevy-48.png`   | PNG    | Linux    |                                            |
| `vortex-bevy-64.png`   | PNG    | Linux    |                                            |
| `vortex-bevy-128.png`  | PNG    | Linux    |                                            |
| `vortex-bevy-256.png`  | PNG    | Linux    |                                            |
| `vortex-bevy-512.png`  | PNG    | Linux    | Canonical XDG hi-res size                  |
| `vortex-bevy-1024.png` | PNG    | master   | Canonical master for future re-derivations |

`cargo-bundle` globs the `icon = [...]` paths in
`crates/vortex-bevy/Cargo.toml`, so any size of the same format is
picked up automatically — the size-suffixed filenames are not a
contract; only the `.icns` / `.ico` / `.png` extension-prefix matters.

## Regenerating the assets

The canonical source is the 1024×1024 PNG master
(`vortex-bevy-1024.png`). All smaller outputs are derived from it. To
re-derive after a redesign:

```bash
# Re-derive every PNG size from the 1024 master.
for size in 16 32 48 64 128 256 512 1024; do
    magick vortex-bevy-1024.png -resize ${size}x${size} vortex-bevy-${size}.png
done

# Rebuild the multi-resolution ICO.
magick vortex-bevy-16.png vortex-bevy-32.png vortex-bevy-48.png \
      vortex-bevy-64.png vortex-bevy-128.png vortex-bevy-256.png \
      vortex-bevy.ico

# Rebuild the ICNS via iconutil.
rm -rf vortex-bevy.iconset && mkdir vortex-bevy.iconset
cp vortex-bevy-16.png   vortex-bevy.iconset/icon_16x16.png
cp vortex-bevy-32.png   vortex-bevy.iconset/icon_16x16@2x.png
cp vortex-bevy-32.png   vortex-bevy.iconset/icon_32x32.png
cp vortex-bevy-64.png   vortex-bevy.iconset/icon_32x32@2x.png
cp vortex-bevy-128.png  vortex-bevy.iconset/icon_128x128.png
cp vortex-bevy-256.png  vortex-bevy.iconset/icon_128x128@2x.png
cp vortex-bevy-256.png  vortex-bevy.iconset/icon_256x256.png
cp vortex-bevy-512.png  vortex-bevy.iconset/icon_256x256@2x.png
cp vortex-bevy-512.png  vortex-bevy.iconset/icon_512x512.png
cp vortex-bevy-1024.png vortex-bevy.iconset/icon_512x512@2x.png
cp vortex-bevy-1024.png vortex-bevy.iconset/icon_1024x1024.png
iconutil -c icns vortex-bevy.iconset
rm -rf vortex-bevy.iconset
```

## Cropping the white margin from a new source render

If the master render has a non-transparent border (e.g. an export
from a 3D tool with a white background), crop it before re-deriving:

```bash
magick source.png -fuzz 1% -trim +repage trimmed.png
magick trimmed.png -bordercolor none -border 32 trimmed.png
mv trimmed.png vortex-bevy-1024.png
```

The `-fuzz 1%` threshold catches near-white edge pixels; the 32px
transparent border prevents the icon from bleeding into adjacent UI
pixels when rendered at small sizes.

## Licensing

The icon source and the raster outputs are licensed under the same
terms as the project (MIT OR Apache-2.0). When you swap or redesign
the icon, record the change in `CHANGELOG.md` under the Unreleased
section.
