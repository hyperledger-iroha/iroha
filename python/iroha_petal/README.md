# iroha-petal

`iroha-petal` is the pure-Python implementation of **Petal Stream**, Iroha's
animated optical transport (a "streaming QR code" in the Sakura-storm look).
A sender shows a sequence of frames, each a portrait card (a square code with a
band of katakana tiles above and below it); a camera reads them; a payload (for
example, a peer message of up to about 12 KB) is reassembled.

The package is a function-by-function port of the Rust reference crate
[`crates/iroha_petal`](../../crates/iroha_petal) and depends only on the Python
standard library (no NumPy, no native extensions, no `iroha_python`). It
declares Python 3.10+ like the rest of the SDK and also runs on 3.9.

## The frame

A frame (format 5) is a black card of 1024 x 1668 design units: the square of
1024 x 1024 with a band of 322 units above and below it. The square has:

* four **sakura-blossom finders** in the canvas corners (solid five-petal
  blobs used for localisation);
* a **storm of 3,376 petals** on 34 concentric bands, each petal lit or dark,
  cut into four quarter-turn sectors; lit petals inside the SORA `天` are drawn
  white and the others pink, over a faint `天`, so the character shows through.
  On top of each sector, the first 360 of its lit petals (in a fixed shuffled
  order) are turned by 0°, 45°, 90° or 135° about their centres: two more bits
  each;
* **three dotted rings** (88/100/108 slots at radii 405/440/475) whose lit dots
  carry data, plus fixed gate dots at 3, 6 and 9 o'clock.

Each band is a lattice of 10 rows by 35 columns of **katakana tiles** (rounded
squares of 25 units, corner radius 3, on a pitch of 29), less the cells near the
finders: 306 tiles a band (`kana_tiles`, band 0 above the square first, in
reading order). Each tile shows one of sixteen katakana, イロハニヘトワカレムノケアヒスン
(stroke polylines on a 32-unit grid, width 6.5, in a 23-unit glyph box), and is
light (filled light, its glyph in ink `(20, 4, 14)`) or dark (black, its glyph in
pink). Eight tiles of each band are pilots with fixed symbols; each of the other
298 data tiles shows one bit by its polarity and four by its katakana.

| lane | carrier | codeword | data | parity | atoms |
|------|---------|----------|------|--------|-------|
| `D` | ring dots, 240 bits | 30 B | 19 B | 11 B | 1, or the beacon on every 4th frame |
| `F0`-`F3` | the petals of one sector, 840 bits | 105 B | 83 B | 22 B | 5 |
| `T0`-`T3` | the turns of 360 lit petals of one sector, 720 bits | 90 B | 68 B | 22 B | 4, then a 4-byte check |
| `P0`, `P1` | the polarity of 296 data tiles of one band | 37 B | 20 B | 17 B | 1, then a 4-byte check |
| `K0`, `K1` | the katakana of 298 data tiles of one band, 4 bits each | 149 B | 116 B | 33 B | 7, then a 4-byte check |

Each lane is one Reed-Solomon codeword over GF(256) (polynomial `0x11D`,
first root `alpha^0`), XOR-whitened with a fixed xorshift32 stream, carrying
16-byte atoms of a random linear fountain code over GF(2) (systematic first).
A turn lane rides on the field lane of its sector: once that field lane has
said which petals are lit, the turns of its carriers show the turn lane's
codeword (Gray-coded, two bits a petal). Its four atoms are repair atoms with ids
from 2³¹ (`turn_first_id`), and its CRC-32C check binds them to the tag, frame
and sector of the field lane they ride on (`turn_check`), so a turn lane read
under the wrong field lane is refused. The kana lanes have no header either:
their atoms are repair atoms with ids from 2³¹ + 2²⁰ (`kana_first_id`: 16 a
frame, the glyph lane's seven of a band before its polarity lane's one), and
their CRC-32C check binds them to the stream tag, the frame and the lane
(`kana_check`, with the lane code `0x10 | band << 1 | kind`). A frame carries up
to 53 atoms (848 bytes). Any single readable lane is useful; the payload is
bound to the beacon by its CRC-32C. The beacon's version byte is `0x50`;
format 3 (the square alone, version `0x30`), format 2 (rounder, unturned
petals, version `0x20`) and format 1 (a field of katakana tiles, version `0x10`)
are retired and their beacons are refused. Reports name the lanes `D` and
`0`-`3` (the field sectors; a turn lane by the digit of its field lane) and the
kana lanes `P0`, `K0`, `P1`, `K1`.

## Install

```bash
python3 -m pip install ./python/iroha_petal
```

## Sending

```python
from iroha_petal import RenderOptions, StreamEncoder, draw_list, render_frame

encoder = StreamEncoder(payload, kind=1)
for frame in range(2 * encoder.systematic_frames()):
    cells = encoder.cells(frame)
    rgb = render_frame(cells, RenderOptions(size=768, supersample=2))  # Rgb, the card, 768x1251
    shapes = draw_list(cells)  # finders, backdrop, lit petals, lit dots, tiles for vector backends
    svg = shapes.to_svg(768)
```

`StreamEncoder.cells` returns the `FrameCells` of a frame, lit, turned and with
its katakana tiles: `dots`, `petals`, `turns` (the turn position, 0-3, of every
field cell; 0 for dark petals and for lit petals that carry no turn bits) and
`kana` (a `KanaSymbol(glyph, light)` for every tile, in `kana_tiles` order).
`FrameCells.from_words` builds unturned cells from the lane codewords, with only
the pilots set among the tiles; `with_turns(words)` returns them with the
carriers turned to show the four turn codewords and `with_kana(words)` with the
data tiles showing the four kana codewords (`P0`, `K0`, `P1`, `K1`), and
`turn_word(sector)`, `polarity_word(cells.kana, band)` and
`glyph_word(cells.kana, band)` pack them back. `lane_data` and `words` return a
`LaneData` with `d`, `fields`, `turns` and `kana`.

`render_frame` reproduces the reference software renderer pixel for pixel: the
card, `size` pixels wide and `card_height_px(size)` (`size * 1668 / 1024`,
rounded) tall, its square `SQUARE_TOP` (322) units below the top edge.
`render_square` renders the square alone (what tests on bare squares use).
`draw_list` describes the same card for vector backends, in square coordinates
(the band above the square has negative `y`): finders (centre disc, five petal
circles, then five notch circles in the background colour), the backdrop (every
`#` cell of the `天` mask lattice as a 33-unit square, clipped to the disc of
radius 390 about the centre), the lit petals (field cell, centre, axis angle and
light or pink colour; each an ellipse of half-length 6 along its axis, the
outward direction plus its turn, and half-width 3.73, minus a notch of radius
1.12 centred 6.24 units along the axis), lit dots (radius 9) and every katakana
tile (its top-left corner, glyph, polarity and glyph colour: a light tile is a
rounded square of 25 units, corner radius 3, filled light; its glyph's strokes,
`kana_glyph_strokes(glyph)`, are polylines in tile units drawn with round caps
and joins of width `6.5 * 23 / 32`, in ink on a light tile and in pink on a dark
one; every stroke lies inside its rounded tile, so no clip is needed). The notch
must be drawn: it tells a petal from one turned by half a turn, and the turn
reader's templates include it. `to_svg` draws the whole card: every petal as
one notched outline (`DrawList.petal_outline`), placed and turned with `<use>`,
and every glyph once in `<defs>`, stroked over its tile with `<use>`; petals are
small (12 x 7.5 units) and tiles 25 units, so draw them antialiased, at 768
pixels or more. The default palette is background `(0, 0, 0)`, light
`(250, 235, 244)`, pink `(245, 175, 208)`, backdrop `(25, 24, 24)` and ink
`(20, 4, 14)`. Show the frames at 12 frames a second, the default display rate;
on a phone, the card's width fills the screen's width.

## Receiving

```python
from iroha_petal import ScanSession

session = ScanSession()
# for every camera frame: the luma (Y) plane with its row stride
outcome = session.push_plane(width, height, stride, y_plane, now_ms)
if outcome.completed is not None:
    payload = outcome.completed.payload
```

`ScanSession.push(Luma(...), now_ms)` accepts a ready `Luma`. The session
decodes the frame (`decode_frame`), feeds every readable lane into a
`StreamAssembler`, forgets half-received streams after the idle (30 s) and
absolute (180 s) timeouts, and delivers the CRC-verified payload exactly once.
After a frame decodes, the next frames are first read by tracking its pose
(`track_frame`) while it is at most `TRACK_WINDOW_MS` (500 ms) old, which skips
the finder search; a full decode runs when tracking fails. `session.stats()`
counts frames read, located and readable, lane `D` successes (`lane_d`), field
lanes read (`field_lanes`, up to four per frame), frames
`tracked`, and frames read with a hidden corner blossom (`inferred`, which an
app can turn into the hint "one corner blossom is hidden").
Lower-level entry points: `decode_frame(luma)` raises `DecodeError` with a
`DecodeErrorKind`, `decode_frame_at(luma, pose)` reads a frame with a
known `Pose`, `track_frame(luma, previous)` follows a previous `DecodedFrame` (or
returns `None`), and `DecodedFrame.feed(assembler)` offers one frame's atoms to
an assembler. `DecodedFrame.lanes` names the lanes that decoded (`"D0123"`) and
`DecodedFrame.lane(Lane.F2)` returns one lane's result. `decode_frame(..., max_side=N)`
optionally box-filters large images first (off by default) and still reports the
pose in original pixels.

A `Pose` maps the canvas onto the image in two steps: a homography onto
undistorted pixels, then radial lens distortion `k` about the image centre
(`Pose.distortion`; negative is barrel). Every orientation hypothesis starts from
a plain pose (`k = 0`), and tracking starts from the previous frame's distortion.

When a thumb, a glare or the edge of the frame hides one corner blossom, three
large blossoms that form a corner still locate the code: the fourth corner is
inferred as a parallelogram point, moved to where the three dotted rings line up
best, and its light and dark levels are extrapolated from the other three
corners. `DecodedFrame.inferred_corner` reports it as the canonical index
(0 top-left, 1 top-right, 2 bottom-right, 3 bottom-left of the upright code), and
`locate_candidates(luma)` lists the finder sets a decoder tries, in order. The
orientation hypotheses are ranked by the ring gates; lane `D` fixes the
orientation, and where the rings are unreadable the field does (a wrong quarter
turn reads permuted petals that no field codeword accepts). Tracking re-finds each
blossom near where the previous pose puts it; a
blossom inferred in the previous frame counts as seen again only when it is found
within a quarter diameter of where the other three say it is.

Every petal is one bit, read by its brightness alone: the mean of five samples
across the petal, normalised by the light and dark levels measured at the four
finders. Each field lane takes a two-means threshold of its own, separately for
the light petals inside the `天` and the pink ones, and the bytes whose bits sit
closest to their threshold become the first Reed-Solomon erasures. A field lane
this read leaves unreadable is read again from *local levels* (every petal
rescaled by the darkest and brightest petals within 32 units of it), so shadows,
glare, banding and veiling light, which the finder levels cannot describe,
cancel. When the plain pose leaves a field lane unread, the lens distortion is
searched (`k` from -0.16 to +0.03, then refined): a cheap lens's barrel
distortion moves the outer petals of a 480p capture by a third of their pitch.

A scan session also averages repeated captures of one shown frame when a field
lane is missing. Every decoded lane must agree on its tag and frame counter.
It holds at most six shown frames and four captures per frame, keeps each
capture's field evidence (`field_evidence`: every petal's level under that
capture's own pose and finder levels), reads the missing field lanes from the
cell-by-cell mean of the held captures (`merge_fields`), and offers a recovered
lane only when its header matches that frame. `ScanOutcome.merged` names newly
recovered lanes; `ScanStats.merges` and `merged_lanes` count attempts and recovery.
Reset and receive timeouts clear the held evidence.

Every field lane a capture decodes also lets the session read the turn lane on
top of it (`read_turns`), once per shown frame: the session remembers the last
24 turn lanes it tried (`TURN_MEMORY`), read or not, and does not look at those
petals again. The turn reader costs a slow phone core several times a frame's
decode, so after a try that reads no turn lane (a camera too soft or too shaky
for the turns) the session passes over the next captures that would try one,
one more after every such try, at most `TURN_BACKOFF` (8), until a try reads a
lane again. `ScanOutcome.turned` lists the turn lanes read from a capture (the
sector digits of their field lanes); `ScanStats.turn_tries` and `turn_lanes`
count the turn lanes tried and read. Reset and a stream switch clear the turn
lanes tried and the back-off.

The session reads the katakana bands of every capture whose square decoded
(`read_kana`), under the frame its lanes name: each band once per shown frame
(it remembers the last 12 bands it tried, `KANA_MEMORY`), with its own back-off
of at most `KANA_BACKOFF` (8) captures after tries that read no kana lane (a
camera that does not see the bands, or too blurred for their glyphs). The bands
go first: a capture whose bands read a kana lane tries no turn lane, and the
turn lanes of that shown frame wait for its next capture. On a slow phone core
a turn try costs five tracked decodes and a band try one, so reading both on one
capture overran the time a frame is shown at 12 fps; a camera that sees each
shown frame two or three times still reads both, one that sees it once reads
the cheaper bands and every frame's field lanes, and a band try that reads
nothing (bands out of view, too blurred, or a torn capture) leaves the turn
lanes to the same capture. `ScanOutcome.kana` names the kana lanes read from a
capture (`P0K0P1K1` for all four), and `ScanStats.kana_tries` and `kana_lanes`
count the bands tried and the kana lanes read.

The turn reader samples every carrier petal on a 13 x 13 window in its own
outward frame, after correcting the pose by a cubic fitted to the carriers'
centroids, fits the camera's blur (the Gaussian under which the carriers and their
lit neighbours explain the windows best), and decides each turn against blurred
petal templates with the lit neighbours subtracted at their own turns; a turn lane
passes only when its Reed-Solomon codeword decodes (with 0, 2, 5, 7 or 11 of its
least confident bytes erased) into data whose check matches the header of the
field lane it rides on. From a fitted blur of 3.2 canvas units on (a soft or
distant camera) it reads no turn lanes, and the field lanes and lane `D` carry
the stream as before. `observed_turns` reports the fitted blur and every
carrier's decision for diagnostics, and `turn_template` the reader's templates.
The turn lane codec is exported as well: `carrier_order`, `carriers`,
`turn_data`, `turn_packet`, `turn_check`, `turn_first_id`, `turn_whitening`,
`encode_turn_lane`, `decode_turn_lane_counted`, `carrier_position`, `turn_angle`,
`turn_label` and `turn_position`.

The band reader (`read_kana(luma, frame, header, skip)`) places every tile with
the square's pose, extended beyond the square. Each band's pilots are found by a
coarse search on a 6 x 6 grid within their reach (4 units at the square's edge,
growing away from it), then refined on the tile's 12 x 12 grid; they pick the
blur of the glyph templates (σ 1.0-4.8 units), an affine shift field fitted to
their offsets moves every tile, and their levels give the threshold between
light and dark tiles. Each data tile then takes the polarity its mean level
shows and the katakana whose light or dark template correlates best, the
margins become the bytes' confidences, and a lane passes when its Reed-Solomon
codeword decodes (with 0, 1/8, 1/4, 1/3 or 1/2 of its parity bytes' worth of its
least confident bytes erased) into data whose check matches the square's frame,
the frame after it or the frame before it (a rolling shutter or a stale display
can show a band of a neighbouring frame). It returns a `KanaResult` (the lane
data and the header the check matched) or `None` per lane, in the order `P0`,
`K0`, `P1`, `K1`; `kana_packet(result.header, lane, result.result.data)` turns
one into atoms. `observe_band` reports every tile's decision for diagnostics, and
`template_probe` and `coarse_template_probe` the reader's templates. The kana
lane codec is exported as well: `KanaLane` (`band`, `kind`, `code`, sizes and
`whitening`), `kana_data`, `kana_packet`, `kana_check`, `kana_first_id`,
`encode_kana_lane`, `decode_kana_lane_counted`, `pilot_symbols`,
`set_kana_symbols`, `polarity_word` and `glyph_word`; `glyphs` holds the
katakana (`GLYPH_CHARS`, `STROKES`, `is_inked`) and `layout` the card
(`CARD_HEIGHT`, `SQUARE_TOP`, `kana_tiles`, `kana_tile_at`, `kana_inside`,
`kana_inked` and the pilots).

## Command line

```bash
python3 -m iroha_petal encode --input payload.bin --output frames/ [--kind N] [--frames N] [--size 768] [--format png|pgm|svg]
python3 -m iroha_petal decode --input-dir frames/ --output payload.out   # or repeated --input FILE, any order
python3 -m iroha_petal inspect frames/frame_0004.png
```

`encode` writes cards `--size` pixels wide (768 x 1251 by default) as
`frame_0000.png`, ... (default: twice the systematic frame count). `decode` reads
PNG (8-bit grey, grey + alpha, RGB, RGBA, palette; non-interlaced) and binary
PGM/PPM frames, downscales frames larger than `--max-side` (default 1280),
timestamps them `--frame-interval-ms` apart (default 83, the 12 fps display
rate), reports the lanes each frame read (`D` and the sector digits `0`-`3`),
marks frames read by tracking and frames with a hidden corner blossom, recovered
field lanes, the turn lanes and the kana lanes read, prints the session counters
(lane `D` frames, field lanes, turn lanes, kana lanes, merges and merged field
lanes), and exits with status 1 if the stream is incomplete. `inspect` prints
the orientation, a hidden (inferred) corner, the lens distortion, the
homography, every lane, turn lane and kana lane that decodes (a kana lane with
the frame its check matched) and the blur the turn reader fits. There is no
network or camera access.

## Modules

| module | contents |
|--------|----------|
| `crc`, `prng`, `rs` | CRC-32C, xorshift32, GF(256) Reed-Solomon with errors-and-erasures decoding |
| `layout` | card geometry: petal bands and sectors, the `天` mask and light petals, ring slot roles, finder and petal shapes, the katakana tiles and pilots of the bands |
| `glyphs` | the sixteen katakana: stroke polylines, `is_inked`, `inked_flags` |
| `lanes`, `fountain`, `stream` | lane codecs and `FrameCells`, fountain code, `StreamEncoder`, `StreamAssembler` |
| `turns` | turn lanes: carrier order, the turn lane codec, `read_turns`, `observed_turns`, `turn_template` |
| `kana` | kana lanes: the lane codec, tile symbols, `read_kana`, `observe_band`, `template_probe`, `coarse_template_probe` |
| `image`, `locate`, `decode` | `Luma`, `Rgb`, `Homography`, `Pose`, finder candidates and following, `decode_frame`, `decode_frame_at`, `track_frame`, `field_evidence`, `merge_fields` |
| `render`, `session`, `pngio`, `cli` | renderer and draw list, `ScanSession`, PNG/PGM I/O, command line |

## Conformance

The tests check every section of the shared golden vectors
`fixtures/petal/petal_stream_v5.json` (the carrier orders, the katakana tiles,
pilots and glyph strokes, the `T0`-`T3` and `P0`, `K0`, `P1`, `K1` whitening, the
turn and kana atom ids, both readers' template probes, which the tables match
exactly, and every listed frame's turn and kana lane data, codewords, turn
positions and tile glyphs and polarities included) and decode every golden
capture in `fixtures/petal/petal_captures_v5.json`. All sixteen captures decode
with exactly the lanes the reference read (`reference_decoded`) and the
recorded inferred corner (`hidden-corner-540p`: 3, `cut-corner-720p`: 2), never
with wrong data; both tracking pairs follow the first frame's pose into the
second with exactly the lanes and inferred corner the reference tracked
(`reference_tracked`); every capture and both tracking pairs read exactly the
turn lanes the reference read (`reference_turned`): all four on the sharp,
rotated, mirrored, covered and cut-off captures, the sharp upright phone and both
tracking pairs, all four on the shadow band with 3 to 9 corrections, three on
the older upright phone, one on the over-exposed capture, and none on the soft,
blurred, veiled and distorted captures; every capture reads exactly the kana
lanes the reference read (`reference_kana`, with their data and the
corrections the reference makes): all four on the three upright 720p phones,
the clean card and the veiled capture, `P0`, `P1` and `K1` on the upright 480p
phone and the band in view on the tilted captures; and the negatives are
rejected. Field lane 3 of `shadow-band-540p` is read only through the local
read, and the field lanes of `barrel-480p` only under the distorted pose the
search finds (`k = -0.12`). The renderer reproduces the `clean-512` capture (a
whole card) byte for byte and the reference's CRC-32C of three cards at 512, 300
and 777 pixels. The noisy merge set recovers exactly the field lanes the
reference merged (`reference_merged`, `013`), which no capture reads alone, and
a session offers each of them once; the mixed-frame set recovers lanes whose
header names another shown frame and verifies that a session refuses to offer
their atoms. Fed the reference's 40 000 random words per lane, the Reed-Solomon
erasure schedule accepts exactly the same three words of lane `D` and none of a
field lane (the tests run the first 10 000 and 1 000).

The decoder keeps the reference's constants, order of operations and IEEE
double arithmetic, including the order of every floating-point sum, the
single-precision ring-slot centres and petal constants and Rust's `total_cmp`
ordering, so it reads the same lanes from the same pixels, in the level read and
in the local read alike, and the turn and band readers decide the same turns,
polarities and katakana. The fast paths (box sums over rows, run-based component
labelling, the bucketed neighbourhood table, the shape-by-shape renderer, the
row-at-a-time glyph classification) are tested against literal ports of the
reference.

On CPython 3.9 and 3.14 (Apple Silicon, measured on a busy machine) a golden
capture decodes in 0.15-1.0 s (1280x720 included; a hidden corner costs about
0.35 s more for the ring search), a tracked frame takes 0.02-0.5 s (the longer
with a hidden corner), and a 768-pixel card (2x2 supersampling) renders in
about 0.4-0.6 s. Reading the turn lanes of a golden capture takes 0.4-1.0 s more
(0.2 s when the camera is too soft and the blur fit stops the reader), and
reading both bands 0.05-0.3 s; the turn reader's tables are built once, on first
use, in about 0.3 s, and the band reader's templates in about 0.8 s, and a
carrier's window is sampled only when the reader first needs it.

## Tests

```bash
cd python/iroha_petal
python3 -m unittest discover -s tests
python3 -m pytest tests          # when pytest is installed
```
