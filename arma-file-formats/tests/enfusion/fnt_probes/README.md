# Font probes

Black box fixtures for `fnt_test.rs`, saved by the Font Editor of Arma Reforger Workbench
(1.8.0.13) from `Roboto-Regular.ttf` (googlefonts/roboto, `src/hinted/`, Apache License 2.0, see
`ROBOTO_LICENSE.txt`). Settings: font size 24, ranges Ascii, SDF radius 8 for the distance field
fonts. The files were renamed after saving; `HEAD` keeps the names they were saved under.

| File                    | Font type      | Saved as                   |
| ----------------------- | -------------- | -------------------------- |
| `roboto_bitmap.fnt`     | Bitmap         | `roboto_bitmap_3224.fnt`   |
| `roboto_sdf.fnt`        | Distance field | `roboto_sdf_32.fnt`        |
| `roboto_sdf_bold.fnt`   | + Bold         | `roboto_sdf_32_bold.fnt`   |
| `roboto_sdf_italic.fnt` | + Italic       | `roboto_sdf_32_italic.fnt` |

The editor also writes the atlas next to the `.fnt` (a `.tga` and the `.edds` built from it), not
kept here. A kerning probe from Arial (`arial_sdf_32.fnt`, same settings) and a colour glyph probe
from Segoe UI Emoji (`emoji2_sdf.fnt`, plus the custom range 128512 to 128515, the field takes
decimal) are in the encrypted test data as `test-data/fnt_probes_in/`, since those fonts are not
free to redistribute.

Workbench 1.8.0.13 crashes on "Generate Bitmap" because the editor's fallback font
`system/wbdata/system_unicode.ttf` is not shipped. Any TrueType font at that path in the addon
fixes it.

Findings:

- The bitmap font is `FNT3` with `HEAD` first, the distance field fonts are `FNT5`; chunks
  `HEAD GLPS TCRD KERN`. Both get `U+0020..U+007E` plus `U+25A1`.
- Bold and Italic set the first and second flag byte of `HEAD`; atlas and glyphs stay the same.
- The 8 unused bytes of `GLPS` are `40 bc 73 42 fa 7f 00 00` (a pointer) in the distance field
  fonts and zeros in the bitmap font; the range padding is `0` here but random in other runs.
- Bitmap glyphs are a full line (26 px) high, the space has width 0 and 6 px of space after it,
  the advance the SDF font gives it.
- Roboto has kerning only in `GPOS`, so `KERN` is empty. Arial's `KERN` has every pair of its
  legacy `kern` table between ASCII characters except the one with value 0, as code points, in
  pixels: about `round(value * 24 / 2048)` (92 of 96, `-264` gives `-4`).
- Segoe UI Emoji's `COLR` glyphs get a `LYRS` entry each (chunks `HEAD GLPS TCRD LYRS KERN`):
  per layer a code point and an `0xAARRGGBB` colour, all 26 colours equal to `CPAL`. The layer
  shapes are extra glyphs at `0x110000 + glyph id - 1` with advance 0, layers with the same shape
  (every emoji's face) share one.
