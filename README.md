# mkdmg

Build a Finder-styled disk image without driving the Finder.

`mkdmg` writes the volume's `.DS_Store` itself instead of asking Finder to
produce one through AppleScript, so it needs no GUI session, no Finder
automation permission, and no wait-for-the-window-to-settle delays.

```sh
# a typical drag-to-install layout
mkdmg MyApp.app \
    --output MyApp-1.0.dmg \
    --background art/background.tiff \
    --window-size 660,432 \
    --icon MyApp.app 180 200 \
    --app-drop-link 480 200
```

## How the appearance is produced

Everything Finder draws comes from two things inside the volume:

| Piece | Where it lives |
| --- | --- |
| Window size, icon size, background image, icon positions | `.DS_Store` at the volume root |
| Background image | `.background/<image>` |
| Volume icon | `.VolumeIcon.icns` + `kHasCustomIcon` on the root directory |

`mkdmg` builds the first two in a staging directory and lets
`hdiutil create -srcfolder` copy them in. The background image is named by a
Finder *alias* record inside `.DS_Store`, and Finder resolves that record by
volume name and the path within the volume — so it can be written before the
image exists. Nothing has to be mounted.

The volume icon is the one exception. Finder only reads `.VolumeIcon.icns` when
the volume's root directory carries `kHasCustomIcon`, which belongs to the
directory the filesystem formatter created, not to the copied tree. `mkdmg`
mounts the writable image once and sets that attribute.

`--no-mount` keeps everything in the staging directory and skips the volume
icon. It cannot be combined with `--volume-icon`, and it is refused for APFS,
where the icon can only be reached through a mount.

## Notes

- `--window-position` uses Finder's own coordinates: `Y` counts upwards from
  the bottom of the screen. Converting from a top-down position would need the
  screen height of whichever machine later opens the image.
- The background image is drawn without scaling, so it should be as large as
  the window's content area: the window height minus its 32-point title bar.
  For Retina, pass a multi-representation TIFF (`tiffutil -cat 1x.tiff 2x.tiff
  -out background.tiff`).
- The default filesystem is HFS+, which older systems and third-party tools can
  still read. `--fs apfs` is available.
- Only one 4096-byte B-tree page of records is written, which is far more than a
  normal window needs; stores that would not fit are reported rather than
  silently truncated.
