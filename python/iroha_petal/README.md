# iroha-petal

`iroha-petal` is the pure-Python implementation of **Petal Stream**, Iroha's
animated optical transport (a "streaming QR code" in the Sakura-storm look).
A sender shows a sequence of square frames; a camera reads them; a payload
(for example, a peer message of up to about 10 KB) is reassembled.

The package is a function-by-function port of the Rust reference crate
[`crates/iroha_petal`](../../crates/iroha_petal) and depends only on the Python
standard library (no NumPy, no native extensions, no `iroha_python`). It
declares Python 3.10+ like the rest of the SDK and also runs on 3.9.

## The frame

A frame (format 3) is black with:

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

| lane | carrier | codeword | data | parity | atoms |
|------|---------|----------|------|--------|-------|
| `D` | ring dots, 240 bits | 30 B | 19 B | 11 B | 1, or the beacon on every 4th frame |
| `F0`-`F3` | the petals of one sector, 840 bits | 105 B | 83 B | 22 B | 5 |
| `T0`-`T3` | the turns of 360 lit petals of one sector, 720 bits | 90 B | 68 B | 22 B | 4, then a 4-byte check |

Each lane is one Reed-Solomon codeword over GF(256) (polynomial `0x11D`,
first root `alpha^0`), XOR-whitened with a fixed xorshift32 stream, carrying
16-byte atoms of a random linear fountain code over GF(2) (systematic first).
A turn lane rides on the field lane of its sector: once that field lane has
said which petals are lit, the turns of its carriers show the turn lane's
codeword (Gray-coded, two bits a petal). Its four atoms are repair atoms with ids
from 2³¹ (`turn_first_id`), and its CRC-32C check binds them to the tag, frame
and sector of the field lane they ride on (`turn_check`), so a turn lane read
under the wrong field lane is refused. A frame carries up to 37 atoms.
Any single readable lane is useful; the payload is bound to the beacon by its
CRC-32C. The beacon's version byte is `0x30`; format 2 (the same field with
rounder, unturned petals, version `0x20`) and format 1 (the katakana tile field,
lanes `P` and `K`, version `0x10`) are retired and their beacons are refused.
Reports name the lanes `D` and `0`-`3` (the field sectors; a turn lane by the
digit of its field lane).

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
    rgb = render_frame(cells, RenderOptions(size=768, supersample=2))  # Rgb, 768x768
    shapes = draw_list(cells)  # finders, backdrop, lit petals, lit dots for vector backends
    svg = shapes.to_svg(768)
```

`StreamEncoder.cells` returns the `FrameCells` of a frame, lit and turned:
`dots`, `petals` and `turns` (the turn position, 0-3, of every field cell; 0 for
dark petals and for lit petals that carry no turn bits). `FrameCells.from_words`
builds unturned cells from the lane codewords, `with_turns(words)` returns them
with the carriers turned to show the four turn codewords, and `turn_word(sector)`
packs the turns back. `lane_data` and `words` return a `LaneData` with `d`,
`fields` and `turns`.

`render_frame` reproduces the reference software renderer pixel for pixel.
`draw_list` describes the same frame for vector backends: finders (centre
disc, five petal circles, then five notch circles in the background colour),
the backdrop (every `#` cell of the `天` mask lattice as a 33-unit square,
clipped to the disc of radius 390 about the centre), the lit petals (field cell,
centre, axis angle and light or pink colour; each an ellipse of half-length 6
along its axis, the outward direction plus its turn, and half-width 3.73, minus a
notch of radius 1.12 centred 6.24 units along the axis) and lit dots (radius 9).
The notch must be drawn: it tells a petal from one turned by half a turn, and the
turn reader's templates include it. `to_svg` draws every petal as one notched
outline (`DrawList.petal_outline`), placed and turned with `<use>`; petals are
small (12 x 7.5 units), so draw them antialiased, at 768 pixels or more. The
default palette is background `(0, 0, 0)`, light `(250, 235, 244)`, pink
`(245, 175, 208)` and backdrop `(25, 24, 24)`. Show the frames at 12 frames a
second, the default display rate.

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

## Command line

```bash
python3 -m iroha_petal encode --input payload.bin --output frames/ [--kind N] [--frames N] [--size 768] [--format png|pgm|svg]
python3 -m iroha_petal decode --input-dir frames/ --output payload.out   # or repeated --input FILE, any order
python3 -m iroha_petal inspect frames/frame_0004.png
```

`encode` writes `frame_0000.png`, ... (default: twice the systematic frame
count). `decode` reads PNG (8-bit grey, grey + alpha, RGB, RGBA, palette;
non-interlaced) and binary PGM/PPM frames, downscales frames larger than
`--max-side` (default 1280), timestamps them `--frame-interval-ms` apart
(default 83, the 12 fps display rate), reports the lanes each frame read (`D` and
the sector digits `0`-`3`), marks frames read by tracking and frames with a
hidden corner blossom, recovered field lanes and the turn lanes read, prints the
session counters (lane `D` frames, field lanes, turn lanes, merges and merged
field lanes), and exits with status 1 if the stream is incomplete. `inspect`
prints the orientation, a hidden (inferred) corner, the lens distortion, the
homography, every lane and turn lane that decodes and the blur the turn reader
fits. There is no network or camera access.

## Modules

| module | contents |
|--------|----------|
| `crc`, `prng`, `rs` | CRC-32C, xorshift32, GF(256) Reed-Solomon with errors-and-erasures decoding |
| `layout` | canvas geometry: petal bands and sectors, the `天` mask and light petals, ring slot roles, finder and petal shapes |
| `lanes`, `fountain`, `stream` | lane codecs and `FrameCells`, fountain code, `StreamEncoder`, `StreamAssembler` |
| `turns` | turn lanes: carrier order, the turn lane codec, `read_turns`, `observed_turns`, `turn_template` |
| `image`, `locate`, `decode` | `Luma`, `Rgb`, `Homography`, `Pose`, finder candidates and following, `decode_frame`, `decode_frame_at`, `track_frame`, `field_evidence`, `merge_fields` |
| `render`, `session`, `pngio`, `cli` | renderer and draw list, `ScanSession`, PNG/PGM I/O, command line |

## Conformance

The tests check every section of the shared golden vectors
`fixtures/petal/petal_stream_v3.json` (the carrier orders, the `T0`-`T3`
whitening, the turn atom ids, the turn reader's template probes, which the
tables match exactly, and every listed frame's turn lane data, codewords and
turn positions included) and decode every golden capture in
`fixtures/petal/petal_captures_v3.json`. All twelve captures decode with exactly
the lanes the reference read (`reference_decoded`) and the recorded inferred
corner (`hidden-corner-540p`: 3, `cut-corner-720p`: 2), never with wrong data;
both tracking pairs follow the first frame's pose into the second with exactly
the lanes and inferred corner the reference tracked (`reference_tracked`); every
capture and both tracking pairs read exactly the turn lanes the reference read
(`reference_turned`): all four on the sharp, rotated, mirrored, covered and
cut-off captures and both tracking pairs, all four on the shadow band with 3 to 9
corrections, two on the over-exposed capture at the limit of their Reed-Solomon
code, and none on the soft, blurred, veiled and distorted captures; and the
negatives are rejected. Field lane 3 of `shadow-band-540p` is read only
through the local read, and the field lanes of `barrel-480p` only under the
distorted pose the search finds (`k = -0.12`). The renderer reproduces the
`clean-512` capture byte for byte. The noisy merge set recovers exactly the
field lanes the reference merged (`reference_merged`), which no capture reads
alone, and a session offers each of them once; the mixed-frame set recovers lanes
whose header names another shown frame and verifies that a session refuses to
offer their atoms. Fed the reference's 40 000 random words per lane, the
Reed-Solomon erasure schedule accepts exactly the same three words of lane `D`
and none of a field lane (the tests run the first 10 000 and 1 000).

The decoder keeps the reference's constants, order of operations and IEEE
double arithmetic, including the order of every floating-point sum, the
single-precision ring-slot centres and petal constants and Rust's `total_cmp`
ordering, so it reads the same lanes from the same pixels, in the level read and
in the local read alike. The fast paths (box sums over rows, run-based component
labelling, the bucketed neighbourhood table, the shape-by-shape renderer) are
tested against literal ports of the reference.

On CPython 3.9 and 3.14 (Apple Silicon, measured on a busy machine) a golden
capture decodes in 0.15-1.0 s (1280x720 included; a hidden corner costs about
0.35 s more for the ring search), a tracked frame takes 0.02-0.5 s (the longer
with a hidden corner), and a 768-pixel frame (2x2 supersampling) renders in
about 0.6-1.1 s. Reading the turn lanes of a golden capture takes 0.5-1.0 s more
(0.2 s when the camera is too soft and the blur fit stops the reader); the
reader's tables are built once, on first use, in about 0.3 s, and a carrier's
window is sampled only when the reader first needs it.

## Tests

```bash
cd python/iroha_petal
python3 -m unittest discover -s tests
python3 -m pytest tests          # when pytest is installed
```
