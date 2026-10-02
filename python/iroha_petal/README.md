# iroha-petal

`iroha-petal` is the pure-Python implementation of **Petal Stream**, Iroha's
animated optical transport (a "streaming QR code" in the Sakura-storm look).
A sender shows a sequence of square frames; a camera reads them; a payload
(for KAGEMUSHA, an `IPM1` peer message of up to about 10 KB) is reassembled.

The package is a function-by-function port of the Rust reference crate
[`crates/iroha_petal`](../../crates/iroha_petal) and depends only on the Python
standard library (no NumPy, no native extensions, no `iroha_python`). It
declares Python 3.10+ like the rest of the SDK and also runs on 3.9.

## The frame

A frame is black with:

* four **sakura-blossom finders** in the canvas corners (solid five-petal
  blobs used for localisation);
* a **`天`-shaped field of 256 tiles**; each tile has a light/dark polarity and
  one of 16 katakana (`イロハニヘトワカレムノケアヒスン`);
* **three dotted rings** (80/92/104 slots) whose lit dots carry data, plus
  fixed gate dots at 3, 6 and 9 o'clock.

| lane | carrier | codeword | data | parity | atoms |
|------|---------|----------|------|--------|-------|
| `P` | tile polarity, 256 bits | 32 B | 19 B | 13 B | 1 |
| `K` | tile glyphs, 256 x 4 bits | 128 B | 83 B | 45 B | 5 |
| `D` | ring dots, 240 bits | 30 B | 19 B | 11 B | 1, or the beacon on every 4th frame |

Each lane is one Reed-Solomon codeword over GF(256) (polynomial `0x11D`,
first root `alpha^0`), XOR-whitened with a fixed xorshift32 stream, carrying
16-byte atoms of a random linear fountain code over GF(2) (systematic first).
Any single readable lane is useful; the payload is bound to the beacon by its
CRC-32C.

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
    shapes = draw_list(cells)  # finders, tiles, glyph strokes, lit dots for vector backends
    svg = shapes.to_svg(768)
```

`render_frame` reproduces the reference software renderer pixel for pixel.
`draw_list` describes the same frame for vector backends: finders (centre
disc, five petal circles, then five notch circles in the background colour),
tiles (25-unit rounded squares, corner radius 3, light flag, glyph index and
colours), glyph strokes (canvas-unit polylines clipped to the 23-unit glyph box,
stroke width 6.5/32 of the box, round caps and joins) and lit dots (radius 11).
The default palette is background `(0, 0, 0)`, light `(250, 235, 244)`, pink
`(245, 175, 208)` and ink `(20, 4, 14)`.

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
Lower-level entry points: `decode_frame(luma)` raises `DecodeError` with a
`DecodeErrorKind`, `decode_frame_at(luma, homography)` reads a frame with a
known pose, and `DecodedFrame.feed(assembler)` offers one frame's atoms to an
assembler. `decode_frame(..., max_side=N)` optionally box-filters large images
first (off by default) and still reports the homography in original pixels.

The tile lanes `P` and `K` are read in up to two ways. The *level read* judges
every 8x8 tile patch against the light and dark levels measured at the four
finders. For any lane it leaves unreadable, the *normalised read* rescales each
patch (and each template) by its own contrast, so over-exposure, veiling light,
glare, shadows and gradients cancel out, and tiles that lost their contrast
become the first Reed-Solomon erasures. Frames the level read already decodes
never pay for the second read.

## Command line

```bash
python3 -m iroha_petal encode --input payload.bin --output frames/ [--kind N] [--frames N] [--size 768] [--format png|pgm|svg]
python3 -m iroha_petal decode --input-dir frames/ --output payload.out   # or repeated --input FILE, any order
python3 -m iroha_petal inspect frames/frame_0004.png
```

`encode` writes `frame_0000.png`, ... (default: twice the systematic frame
count). `decode` reads PNG (8-bit grey, grey + alpha, RGB, RGBA, palette;
non-interlaced) and binary PGM/PPM frames, downscales frames larger than
`--max-side` (default 1280), and exits with status 1 if the stream is
incomplete. `inspect` prints the orientation, homography and every lane that
decodes. There is no network or camera access.

## Modules

| module | contents |
|--------|----------|
| `crc`, `prng`, `rs` | CRC-32C, xorshift32, GF(256) Reed-Solomon with errors-and-erasures decoding |
| `layout`, `glyphs` | canvas geometry, tile mask, ring slot roles, finder shape, katakana strokes and 8x8 templates |
| `lanes`, `fountain`, `stream` | lane codecs and `FrameCells`, fountain code, `StreamEncoder`, `StreamAssembler` |
| `image`, `locate`, `decode` | `Luma`, `Rgb`, `Homography`, finder location, `decode_frame`, `decode_frame_at` |
| `render`, `session`, `pngio`, `cli` | renderer and draw list, `ScanSession`, PNG/PGM I/O, command line |

## Conformance

The tests check every section of the shared golden vectors
`fixtures/petal/petal_stream_v1.json` and decode every golden capture in
`fixtures/petal/petal_captures_v1.json`. All nine captures decode with exactly
the lanes the reference read (`reference_decoded`), never with wrong data, and
the negatives are rejected; the tile lanes of `overexposed-540p` and
`shadow-band-540p`, and lane `K` of `veiled-720p`, are read only through the
normalised read. The renderer reproduces the `clean-512` capture byte for byte.

The decoder keeps the reference's constants, order of operations and IEEE
double arithmetic, including the order of every floating-point sum, the
single-precision ring-slot centres and Rust's `total_cmp` ordering, so it reads
the same lanes from the same pixels, in the level read and in the normalised
read alike. The fast paths (box sums over rows, run-based component labelling,
cell-major template matching) are tested against literal pixel-by-pixel ports
of the reference.

On CPython 3.9 (Apple Silicon) a golden capture decodes in 0.4-1.0 s
(1280x720 included; a frame whose level read leaves lane `P` or `K` unreadable
pays about 0.25 s more for the normalised read) and a 768-pixel frame (2x2
supersampling) renders in about 0.6 s.

## Tests

```bash
cd python/iroha_petal
python3 -m unittest discover -s tests
python3 -m pytest tests          # when pytest is installed
```
