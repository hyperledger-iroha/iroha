# Mochi brand assets

The Mochi mark is a *kagami-mochi* (鏡餅), the two-tier New Year rice cake and a nod
to Mochi's sibling CLI, `kagami`. Two flat, glossy rice-cake tiers sit on red-edged
*shihōbeni* paper on a hinoki *sanbō* stand. The Hyperledger Iroha sakura and its
leaves crown the stack where the traditional bitter orange sits, and the SORA 天 mark
is the stand's crest.

| File | Use |
| --- | --- |
| `mochi-mark.svg` | Primary mark on light or dark backgrounds. |
| `mochi-logo.svg` | Horizontal lockup with the rounded `mochi` wordmark; the i-dot is SORA red. |
| `mochi-icon.svg` | Application icon: the mark under a SORA sky with *kasumi* cloud bands. |
| `mochi-mascot.svg` | Face-less rice cake, leaves and sakura: the animated mascot layer. |
| `mochi-stand.svg` | Paper, stand and SORA crest: the static mascot layer. |
| `mochi-icon-512.png` | Window and dock icon embedded by `mochi-ui`. |
| `mochi-mascot-512.png`, `mochi-stand-512.png` | Mascot layers embedded by `mochi-ui`. |

Both source marks keep their original geometry and colors: the Iroha sakura
(`#BB0000` with its white line figure, from the Iroha documentation icon) and the SORA
mark (`#E3242D`). Mochi's own colors are rice white `#FFFFFF` to `#E3E8F1`, ink
`#2E2230`, hinoki `#EDCB8E`, leaf `#5BA85A`, sakura pink `#CF356D` and sky `#5B8DEF`.

The PNG files are renders of the SVG sources. After changing an SVG, render the PNGs
at 512 × 512 with any SVG renderer, for example:

```sh
rsvg-convert -w 512 -h 512 mochi/assets/mochi-icon.svg -o mochi/assets/mochi-icon-512.png
rsvg-convert -w 512 -h 512 mochi/assets/mochi-mascot.svg -o mochi/assets/mochi-mascot-512.png
rsvg-convert -w 512 -h 512 mochi/assets/mochi-stand.svg -o mochi/assets/mochi-stand-512.png
```

The desktop paints the face in `mochi-ui-egui/src/gui/brand.rs` using the same 512-unit
coordinates as `mochi-mark.svg`; keep the two in step when the face changes.

## Izanami marks

The Izanami mark is a portrait of Izanami-no-Mikoto (伊邪那美命) in the manner of an
*uchiwa-e*, the round ukiyo-e woodblock prints made for fans. Izanami is the load and drill
engine planned for Mochi (`specs/izanami_managed_localnet.md`). She holds *Ame-no-nuboko*
(天沼矛), the jeweled spear with which she and Izanagi stirred the primordial sea, and faces
a beni sun. Behind her, her hair streams into the murasaki cloud bank of Yomi, the land of
the dead she came to rule, and spills into *seigaiha* (青海波) waves.

It is drawn as a woodblock print. Flat pigments sit under a sumi key block. A
Prussian-blue *ichimonji bokashi* shades the top of the sky and a dawn gradation the sea
line. Her vermilion robe carries the *asanoha* hemp-leaf pattern, her hair has carved
strands and *kewari* at the brow, and the grain of the block and the fibers of the washi
print through every color. Her face follows the *hikime-kagibana* convention of Yamato-e
scrolls: slit eyes, a hooked nose and high *hikimayu* brows. She wears layered collars in
white, green and vermilion, a magatama necklace and an ochre hairpin.

| File | Use |
| --- | --- |
| `izanami-mark.svg` | Portrait crest on light or dark backgrounds. |
| `izanami-logo.svg` | Horizontal lockup with the rounded `izanami` wordmark, drawn in the stroke grammar of the `mochi` wordmark; both i-dots are brine drops in SORA red, like the `mochi` i-dot. |
| `izanami-mark-512.png` | Render of the crest for the desktop. Not embedded yet; intended for the planned Lab view. |

The pigments are washi `#F2E6CC` with the face and hand in `#F8F0DE`, sumi `#231A1F`,
Prussian blue `#2C4F80` deepening to `#1B3359`, vermilion `#C9402E` with asanoha lines in
`#93281F`, the sun in `#D24A35`, yellow ochre `#D9A441` and pale ochre `#EBD39A`,
verdigris `#4E8A6A`, and murasaki `#57406F` with cloud scales in `#3A2850`. The seigaiha
scales have radius 24 in rows 12 units apart, with alternate rows offset by 24, flat
Prussian blue with washi lines under a deepening bokashi. The grain and fibers are vector
strokes and a tiled pattern, so the mark needs no raster texture or SVG filter. The motifs
are traditional and the subject comes from the *Kojiki*; the artwork itself is original.

The committed PNG was rendered on macOS with the stock `sips`:

```sh
sips -s format png -z 512 512 mochi/assets/izanami-mark.svg --out mochi/assets/izanami-mark-512.png
```

## macOS application icon

`mochi-icon.icns` is the macOS Finder icon. `cargo xtask mochi-bundle` copies it unchanged
to `Mochi.app/Contents/Resources/Mochi.icns`, so packaging needs no image tools. It packs
direct renders of `mochi-icon.svg` at 16, 32, 128, 256 and 512 points, each at 1x and 2x;
regenerate it whenever that SVG changes. The committed file was produced on macOS 27 with
the stock `sips` and `iconutil` tools:

```sh
iconset="$(mktemp -d)/mochi-icon.iconset"
mkdir "$iconset"
for size in 16 32 128 256 512; do
  sips -s format png -z "$size" "$size" mochi/assets/mochi-icon.svg \
    --out "$iconset/icon_${size}x${size}.png"
  sips -s format png -z "$((size * 2))" "$((size * 2))" mochi/assets/mochi-icon.svg \
    --out "$iconset/icon_${size}x${size}@2x.png"
done
iconutil -c icns "$iconset" -o mochi/assets/mochi-icon.icns
```
