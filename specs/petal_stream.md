---
title: Petal Stream Transport
---

Petal Stream is the animated optical transport of Iroha. A sender shows a sequence of square
frames; a phone camera reads them; a payload (for KAGEMUSHA, an `IPM1` peer message of up to
about 12 KB) is reassembled on the other side. It is the "Sakura-storm" alternative to a stream
of QR codes: a storm of sakura petals that a plain phone camera reads faster than an animated QR
code and that still reads where the QR code does not.

This document is normative for the wire format, the frame geometry and the receiver rules. The
reference implementation is the Rust crate `crates/iroha_petal`; the golden vectors in
`fixtures/petal/` are the cross-SDK conformance suite (§10). Where prose and code disagree, the
code and the fixtures win and this document is wrong.

## 1. Overview

A frame is a black square carrying three kinds of marks. Two of them carry data, one locates the
code:

| Mark | What it is | Role |
| --- | --- | --- |
| **Sakura finders** | Four solid five-petal blossoms in the canvas corners | Locate the code in a camera frame and give the perspective; any three suffice when a thumb, a glare or the frame edge hides one. |
| **Petal field** | 3,376 petals on 34 concentric bands, each lit or dark | One bit per petal, in four sectors (lanes `F0`–`F3`). Petals inside the SORA `天` are drawn white, the others pink, over a faint `天`, so the character shows through the storm. |
| **Three dotted rings** | 88, 100 and 108 dot slots at radii 405, 440 and 475 | Which slots are lit carries 240 bits (lane `D`). Fixed *gates* at 3, 6 and 9 o'clock (none at 12) are always lit. |

Each lane is exactly one Reed–Solomon codeword over GF(256), so the lanes fail independently:

* lane `D` (big isolated dots) is the most robust: it survives defocus of 4 px at 720p and carries
  the stream beacon;
* the four field lanes carry 96 % of the payload. A petal is read by its brightness alone, with no
  shape to recognise, so the field survives blur that would defeat a QR code of the same size.

Every lane carries fountain-coded payload atoms, so **any one readable lane is useful by itself**
and the stream completes from whatever mixture of lanes and frames the camera manages to read.
Lost, torn or blended frames cost time, never correctness. The field sectors are pie slices, so a
capture torn by a frame change still yields the sectors on either side of the tear.

Petal Stream does not carry `QrStreamFrame` or `IRQR` frames: it is a complete transport with its
own framing, forward error correction and payload integrity check. KAGEMUSHA payloads use the
`kind` byte of §5.3 with the same values as `IrohaPeerWireKindV1`, which are the KAGEMUSHA wallet V1
envelope message tags (1 Offer, 2 Request, 3 Payment, 4 Credited, 5 SessionControl, 6 PolicyData,
7 Lineage; [wallet wire record](kagemusha_wallet_wire_v1.md) §3.4), and carry the encoded `IPM1`
message as the payload. A Payment carrier is at most 12,182 bytes (84-byte `IPM1` header plus the
12,098-byte carrier), which the measurements in §8 use; Petal carries any such message whole (§5.5:
the default receiver limit is 65 536 bytes).

This is format 2. Format 1 (a `天`-shaped field of 256 katakana tiles) is retired; its beacon
version `0x10` is refused (§5.3).

## 2. Frame geometry (normative)

All coordinates are *design units* on a square canvas of 1024 units, origin at the top-left, `y`
growing downward. A renderer scales the canvas to any pixel size (recommended at least 512 px;
768–1080 px on phones). Angles are measured from the positive `x` axis and grow clockwise on the
screen.

### 2.1 Palette

| Name | RGB |
| --- | --- |
| background | `(0, 0, 0)` |
| light (finders, petals inside the `天`) | `(250, 235, 244)` |
| pink (ring dots, petals outside the `天`) | `(245, 175, 208)` |
| backdrop (the faint `天`) | `(25, 24, 24)` |

Decoders assume that a lit petal and a dot are much brighter than the black canvas; they threshold
the light and the pink petals apart (§7 step 5), so the two colours need not match a fixed ratio. Do
not re-theme frames that are meant to be scanned.

### 2.2 Finders

Centres, clockwise from the top-left: `(72, 72)`, `(952, 72)`, `(952, 952)`, `(72, 952)`.
A finder is the union of a centre disc of radius 12 and five petal discs of radius 26 whose
centres lie 34 units from the finder centre at angles `-90° + 72°·k` (`k = 0..4`, the first petal
points up), drawn in the light colour. A notch of radius 6 centred 60 units from the finder centre
along each petal axis is cut out of the petal tips. The notches are cosmetic: a decoder may only
rely on the finder being one large, isolated, roughly round blob of diameter 120 units. Keep every
other bright object at least one finder diameter away from the finders (hide status bars and
similar UI).

### 2.3 The petal field

The field is 34 concentric **bands**. Band `j` (`0..33`) has radius `r_j = 16 + 11·j` and `n_j`
slots, from the table

```
n = 8, 12, 20, 24, 32, 36, 40, 48, 52, 56, 64, 68, 76, 80, 84, 92, 96, 104, 108, 112, 120,
    124, 128, 136, 140, 148, 152, 156, 164, 168, 172, 180, 184, 192
```

(`4 · round(2π r_j / 50)`: about one petal every 12.5 units; the table, not the formula, is
normative). Every `n_j` is a multiple of four. Slot `k` of band `j` is centred at angle
`θ = 2π (k + φ_j) / n_j` on the circle of radius `r_j` around `(512, 512)`, with `φ_j = 0` for even
and `½` for odd bands, so neighbouring bands interleave. Field cells are numbered band by band from
the centre out, slot by slot: cell `c` is slot `c − o_j` of the band `j` with `o_j ≤ c < o_{j+1}`,
where `o_j` is the sum of the slot counts of the bands before `j`. There are 3,376 cells.

**Sectors.** Sector `s` (`0..3`) holds slots `s·n_j/4 … (s+1)·n_j/4 − 1` of every band, so each
sector is a quarter turn of the field (sector 0 from 3 o'clock clockwise to 6 o'clock) and holds
844 cells. A sector's *bit order* is band by band from the centre out and, within a band, ascending
slot. The first 840 cells of a sector carry the 105 bytes of its lane (§3); the last four are spare
and always dark. The field, the sectors and the bit order map onto themselves under every quarter
turn.

**Petals.** A lit cell is drawn as a petal: an ellipse of half-length 5.2 along the outward radial
direction `θ` and half-width 4.3, centred on the cell, minus a cosmetic notch (a disc of radius 1.3
centred 5.4 units outward from the cell centre). Decoders sample inside the disc of radius 4.6
around the cell centre and rely on nothing else; renderers may omit the notch. A dark cell is not
drawn.

**The `天`.** The 20 × 20 lattice whose cell `(col, row)` spans `[182 + 33·col, 182 + 33·(col+1))`
horizontally and likewise vertically carries the `天` mask, listed row 0 (top) first:

```
....############....
...##############...
..################..
.##################.
###..............###
###..............###
#########..#########
#########..#########
###..............###
###..............###
########....########
########....########
#######......#######
#######..##..#######
######..####..######
#####...####...#####
.###...######...###.
..###.########.###..
......########......
.....##########.....
```

A field cell whose centre lies in a `#` lattice cell is *light*: lit, it is drawn in the light
colour; every other lit petal is pink. The `#` lattice cells, clipped to the disc of radius 390
around the canvas centre, are drawn in the backdrop colour under the petals. The mask is
mirror-symmetric left to right and not top to bottom.

### 2.4 Rings, gates and guards

Ring `r ∈ {0, 1, 2}` has radius `405, 440, 475` and `N_r = 88, 100, 108` slots; slot `k` is centred
at angle `2π·k / N_r`, so slot 0 is at 3 o'clock, slot `N_r/4` at 6 o'clock, slot `N_r/2` at 9
o'clock. A lit slot is a pink dot of radius 9.

Three gates are always lit, with these dot counts for rings `(0, 1, 2)`:

| Gate | Base slot `b` | Dots on rings 0, 1, 2 |
| --- | --- | --- |
| right (3 o'clock) | `0` | 1, 1, 3 |
| bottom (6 o'clock) | `N_r/4` | 2, 2, 2 |
| left (9 o'clock) | `N_r/2` | 1, 2, 2 |

One dot occupies slot `b`; two dots occupy `b, b+1`; three dots occupy `b-1, b, b+1` (indices modulo
`N_r`). The slot on either side of a gate group is a *guard* and is always dark. There is no gate at
12 o'clock. The 16 gate dots, 18 guards and the remaining 262 slots partition the 296 slots; the
first 240 remaining slots in flat order (ring 0 slots ascending, then ring 1, then ring 2) are the
**data slots**, numbered `0..239`; the last 22 remaining slots are spare and dark.

## 3. Lanes (normative)

### 3.1 Cells to bytes

* **Lane `D`**, 30 bytes: bit `b` is `1` iff data slot `b` is lit. Byte `j` holds bits `8j … 8j+7`,
  most significant bit first.
* **Lanes `F0`–`F3`**, 105 bytes each: bit `b` of lane `Fs` is `1` iff cell `b` of sector `s`'s bit
  order is lit. Byte `j` holds bits `8j … 8j+7`, most significant bit first.

### 3.2 Codewords

The transmitted bytes of a lane are `RS(data ‖ parity) XOR whitening`.

| Lane | Codeword | Data | Parity | Corrects |
| --- | --- | --- | --- | --- |
| `D` | 30 B | 19 B | 11 B | 5 wrong bytes, or 11 erased |
| `F0`–`F3` | 105 B | 83 B | 22 B | 11 wrong bytes, or 22 erased |

**Reed–Solomon.** GF(256) with primitive polynomial `x⁸+x⁴+x³+x²+1` (`0x11D`), `α = 2`. The generator is
`∏_{i=0}^{nsym-1}(x − αⁱ)` (first root `α⁰`), the code is systematic with the data bytes first and the
highest-degree coefficient first (the same field and conventions as QR Code). Decoders must support
errors and erasures (`2·errors + erasures ≤ nsym`) and must verify that the corrected word has zero
syndromes before accepting it.

**Whitening.** A fixed pseudo-random byte string is XORed over the codeword so that every frame is
statistically balanced whatever the payload is (about half the petals are lit). The generator is
xorshift32 (`x ^= x << 13; x ^= x >> 17; x ^= x << 5`, a zero seed becomes `0xDEADBEEF`); each output
byte is the top byte (`x >> 24`) of the state after an update. Seeds: `D = 0x444F5453`,
`Fs = 0x50455430 + s`. The first `word length` bytes of the sequence are used; the sequences are in the
fixture. A codeword read under the wrong sector's whitening fails its check, so a decoder that reads
the field in a wrong orientation reads nothing.

## 4. Atoms and the fountain code (normative)

A payload of `len` bytes (1 ≤ `len` ≤ 2²⁴−1) is cut into `K = ⌈len / 16⌉` **source atoms** of 16 bytes
(the last zero padded). The code is a rateless random linear code over GF(2). Encoded atom `id`
(`u32`):

* `id < K`: source atom `id` (systematic);
* otherwise the XOR of the source atoms selected by a pseudo-random mask:

  ```
  mix32(x): x ^= x >> 16; x *= 0x85EBCA6B; x ^= x >> 13; x *= 0xC2B2AE35; x ^= x >> 16   (u32)
  seed     = mix32((id * 0x9E3779B1) ^ crc ^ 0xA5A5A5A5)
  word[w]  = mix32(seed + (w + 1) * 0x9E3779B9)            (all arithmetic modulo 2³²)
  ```

  Mask bit `j` is bit `j mod 32` of `word[⌊j/32⌋]`; bits `≥ K` are cleared; an all-zero mask is
  replaced by the single bit `id mod K`. `crc` is the payload CRC-32C (§5.1). The mixer must not be
  replaced by a GF(2)-linear generator such as xorshift: all masks would then lie in a subspace of
  dimension ≤ 32 and the code could never complete for `K > 32`.

A receiver recovers the payload by Gaussian elimination over GF(2) once it holds `K` linearly
independent atoms; about `K + 2` random atoms suffice. Verification is by the payload CRC-32C.

## 5. Stream layer (normative)

### 5.1 Identity

`crc` is CRC-32C (Castagnoli, reflected, init and final xor `0xFFFFFFFF`) of the payload. The one-byte
`tag` is `crc & 0xFF`. `kind` is an application byte (0 = unspecified).

### 5.2 Frames and atom ids

Frames carry a 16-bit counter `f` (wrapping). Frame `f` carries atoms in this order: field lanes
`F0`, `F1`, `F2`, `F3` five atoms each, then lane `D` one atom (omitted on beacon frames, which are
those with `f mod 4 = 0`). Atom ids are contiguous over the atoms actually sent: the first atom of
frame `f` has id `21·f − ⌈f / 4⌉`; lane `Fs` starts `5s` later and lane `D` 20 later. (65536 is
divisible by 4, so ids repeat cleanly when the counter wraps.)

### 5.3 Lane data layouts (big-endian)

Every lane starts with a 3-byte header `tag:u8, frame:u16`. Then:

* lanes `F0`–`F3`: five atoms (80 B) → 83 B;
* lane `D`, non-beacon frame: one atom → 19 B;
* lane `D`, beacon frame (`f mod 4 = 0`): `version:u8 (=0x20), kind:u8, len:u24, crc:u32` followed by 7 zero bytes → 19 B.

`version` = `0x20` means format version 2, layout profile 0; anything else is rejected (format 1's
`0x10` included).

### 5.4 Sender

Send frames `f = f₀, f₀+1, …` at 8–15 frames per second; **12 is the default**: a camera reads practically every frame at
12 fps, and a phone's slowest cores keep up with its 30 fps camera (§8). The sender repeats until the user stops it; the first `⌈K / 20.75⌉` frames carry every source atom once and the rest are repair
atoms. There is no back-channel; if the receiver needs an acknowledgement it uses its own stream.

### 5.5 Receiver (assembler)

* A beacon locks the receiver to a stream `(kind, len, crc)`; lane headers carry only the tag. A
  different beacon replaces the active stream only after two consecutive sightings.
* Atoms that arrive before the first beacon are buffered (bounded, default 512) and replayed once
  the stream is known; atoms whose tag differs from the active stream are ignored.
* Beacons with `len = 0` or above the receiver's limit (default 65 536 bytes) are ignored. Memory and
  work are bounded by that limit (`K ≤ 4096` at the default).
* When the payload is solved its CRC-32C must match the beacon; if it does not, the elimination state
  is discarded and counted as an integrity failure while the stream continues.
* Only lane `D` carries the beacon, so a receiver needs to read lane `D` of at least one beacon frame
  to start; atoms read earlier are buffered, not lost. Lane `D` is the most robust lane.
* A completed stream is delivered exactly once; further frames of the same stream are ignored.
* Sessions forget a partial stream after an idle timeout (default 30 s) or an absolute timeout (default 180 s).

## 6. Rendering

Any 2D API may draw a frame as long as geometry, polarity and colours match §2. The reference
software renderer (`render.rs`) shades each sample in this order: finders, ring dots, petals, the
backdrop `天`, background, with 2×2 or 3×3 supersampling. SDKs additionally expose a platform-neutral
*draw list* (finder discs and notches, backdrop cells, lit petals with centre, angle and colour,
lit dots) so a native canvas can play a stream without per-pixel work.

Display guidance: full screen brightness, a black surround, the whole square visible, no overlays
within one finder diameter of the finders, animation at 12 fps (§5.4). A frame must stay up for at least
two camera frames: 15 fps is the most a 30 fps camera reads (§8).

## 7. Decoding

A conforming decoder takes an 8-bit luma plane (the Y plane of an NV21/YUV camera frame, or luminance
computed as `(299R + 587G + 114B + 500) / 1000`) and must: locate the four finders, find the pose,
resolve orientation (four rotations, optionally mirrored), read the lanes and Reed–Solomon-decode
them. It must never report a lane whose Reed–Solomon check failed, and it must reject garbage
(noise, blank frames) without crashing.

The reference algorithm, which the golden captures are calibrated to:

1. **Finders.** Adaptive threshold against the local mean (integral image; window radius `clamp(short side / 8, 12, 64)`).
   With `range = max(P99.5 − P2, 8)` over the luma percentiles, a pixel is lit when it exceeds the local mean by
   `max(s·range, 5)` and also exceeds `P2 + 0.2·range`, for sensitivities `s = 0.12, 0.22, 0.34`.
   Label 4-connected components. A finder candidate is a component with `size ≥ 14 px` (larger bounding-box side), area ≥ 100,
   fill (area / bounding box) 0.45–0.90 and principal-axis ratio ≥ 0.5, with no *other component of substance* (area ≥ 8 and
   at least 1.5 % of the candidate's area) whose centroid lies within 0.8 × its size. The *large* candidates are those of at
   least 0.55 × the largest; any set of candidates is ordered by size, largest first (ties in discovery order), and at most the
   first ten are combined. A *quad* is a combination of four that forms a convex quadrilateral with similar sizes
   (largest/smallest ≤ 1.9, longest/shortest side ≤ 2.6, mean side / mean size between 4.8 and 10.5); the one with the lowest
   score `(size ratio − 1) + (side ratio − 1) + |mean side / mean size − 7.33| / 7.33` wins. A *corner* is three candidates with
   sizes within 1.9, one of them (the vertex) with two legs whose lengths are within a factor 2, whose angle has
   `|cos| ≤ 0.5`, and whose mean length is 4.8–10.5 mean sizes; the lowest
   `(size ratio − 1) + (leg ratio − 1) + |cos| + |mean leg / mean size − 7.33| / 7.33` wins, and its fourth corner is the
   parallelogram point `a + c − vertex`. Finder centres are refined with an intensity-weighted centroid inside the blob
   (inferred corners are not).

   The decoder tries **candidate finder sets** in this order and keeps the first under which a lane decodes:
   1. for each sensitivity in turn, the quad of the large candidates;
   2. from the first sensitivity that has one, the corner of the large candidates completed by the nearest candidate of at least
      0.3 × the mean size within 0.3 mean legs of its parallelogram point (steep tilt makes the far finder small);
   3. from the first sensitivity that has one, the quad of all candidates;
   4. the corner of step 2 with its fourth finder **inferred** — hidden by a thumb, a glare or the edge of the frame.

   Only the sensitivities that are needed are computed: a clean frame costs one binarisation.
2. **Pose.** A pose maps the canvas onto the image in two steps: a homography onto undistorted pixels, then
   radial lens distortion about the image centre `c = (w/2, h/2)`: a pixel an undistorted camera sees at `u`
   is seen at `c + (u − c)·(1 + k·|u − c|² / s²)`, with `s = 0.8·max(w, h)` and distortion `k` (negative is
   barrel). The pose with distortion `k` through four image points is found by undistorting the points (six
   fixed-point rounds of `u = c + (p − c) / (1 + k·|u − c|²/s²)`, starting from `u = p`) and fitting the
   homography from the four canonical finder centres to them. Every orientation hypothesis (4 rotations, and 4
   mirrored) starts from `k = 0`.
   An inferred corner is first moved to where the three dotted rings line up best: the brightness summed over all 296 ring
   slots (five-point samples as in step 4) is maximised over a 13 × 13 grid in steps of 2 % of the mean leg around the
   parallelogram point, then over a 9 × 9 grid in steps of 0.5 % around the best point, under plain (`k = 0`) poses. The ring
   slots map onto themselves under every quarter turn and mirror (88, 100 and 108 are multiples of four), so this search fixes the
   geometry once for all orientations. Reference levels come from the finders (lit, nine samples of the solid core) and the
   black canvas 100 units inward of them (dark); a corner whose lit level is less than 12 above its dark level refuses the
   candidate. An inferred corner has no levels of its own: its lit and dark levels are the parallelogram extrapolation of the
   other three, clamped to their range, and must differ by 12 as well.
3. **Orientation.** Each hypothesis gets a *gate score*: the mean normalised sample of the gate dots minus that of the guards.
   Hypotheses are ranked by gate score. Try lane `D` under the best three whose gate score is at least 0.2; the first that
   decodes fixes the orientation, and the field is read under it (steps 5–6). Otherwise try the field under the best four (a
   wrong orientation reads permuted petals that no field codeword accepts), each first under its plain pose (step 5 without
   the local read). When that reads every field lane it wins; when it reads some, the distortion is searched (step 6). When it
   reads none, the first of the four still searches the distortion if its gate score is at least 0.2 (strong distortion can
   hide both lane `D` and the field), and otherwise runs the local read under its plain pose; the other three are skipped. The
   first hypothesis that reads a field lane wins, with lane `D` read under its final pose. The decoded frame reports the
   inferred corner, if any, as its canonical index (0 top-left, 1 top-right, 2 bottom-right, 3 bottom-left of the upright
   code).
4. **Dots.** Sample each data slot (the mean of five points, the centre and four at ±2.88 units), normalise with the local
   lit/dark levels, and threshold per ring at the midpoint of that ring's gate and guard samples when they are more than 0.2
   apart (0.5 otherwise).
5. **Petals.** Sample each field cell (the mean of five points, the centre and four at ±1.84 units) and normalise it as
   `(v − dark) / (lit − dark)` with the lit/dark levels interpolated bilinearly between the four finders. For each field lane,
   take the levels of its 840 cells and compute a *two-means threshold* separately for its light cells and its pink cells (a
   class with fewer than 16 cells uses the threshold of all 840): start at the mean of the values, then move to the midpoint of
   the mean of the values above and the mean of the values at or below the threshold, at most eight times, stopping when it
   moves less than 10⁻⁴ or one side is empty (sums run in cell order). A cell above its class threshold is lit; a byte's
   confidence is the least `|level − threshold|` of its eight cells. *Local read*: a field lane the levels leave unread is read
   again from **local levels**: every cell's level rescaled so that the mean of the five lowest and the mean of the five highest
   levels of its neighbourhood (the cells whose centres lie within 32 units, itself included) become 0 and 1 (a span below 0.05
   counts as 0.05). About half the petals around any cell are lit, so shadows, glare, banding and veiling light, which the
   finder levels cannot describe, cancel.
6. **Lens distortion.** When the plain pose leaves field lanes unread, the field is read again (step 5, without the local
   read) under the poses with distortion `k₀ + Δ` for `Δ = −0.03, −0.06, −0.09, −0.12, −0.16, +0.03` in turn and then the best
   `k ± 0.015`, where `k₀` is the starting distortion (0, or the previous frame's when tracking). A pose wins over the best so far
   when it decodes more field lanes, or as many with a larger mean `|level − threshold|` over the cells read; the search stops
   once all four lanes decode. The local read of step 5 then runs once, under the winning pose. A cheap lens moves the outer
   petals of a 480p capture by a third of their pitch while the finders that fix the homography sit far outside the field.
7. **Reed–Solomon with erasures.** Rank bytes by confidence, least confident first, with ties in position order (a stable sort),
   and retry with 0, ⅛, ¼, ⅓ and ½ of the parity bytes as erasures (integer division: lane `D` 0, 1, 2, 3, 5; a field lane
   0, 2, 5, 7, 11). Lane `D` stops at ½: with 11 parity bytes, a further erasure step lets wrong codewords through (§11).
8. **Merging the captures of one shown frame.** A camera sees each shown frame two to four times (30 fps against 8–15 fps), and
   noise, display banding or hand shake can cost every one of those captures a field lane. A scan session therefore keeps the
   *field evidence* of each capture that reads some lane but not all four field lanes of its frame: the normalised level of every
   field cell (step 5) under that capture's pose. The capture's shown frame is the `(tag, frame)` header (§5.3) that every lane
   decoded from it names; a capture whose lanes name different frames (torn by a frame change) is not kept. Evidence is kept for
   the last 6 shown frames and at most 4 captures each (a further capture replaces the oldest; about 27 KB per capture), and is
   dropped once all four field lanes of the frame are read. Replacing an already active stream identity clears held evidence and
   completed-lane flags, even when the new stream shares its 8-bit tag and frame counter. Accepting the first beacon keeps valid
   pre-beacon evidence. From the second capture of a frame on, the session averages the held levels cell by cell (summed in
   capture order, then divided by the number of captures) and reads the field lanes not yet read from the mean as in steps 5 and 7,
   local read included. The session offers a merged lane only when its own header names the frame it was merged for, and counts
   merges and merged lanes apart from single-capture lanes. Merging one capture reads exactly what that capture read alone.

**Tracking.** A scan session that decoded a frame reads the next ones by following that pose instead of searching the
whole image, as long as the pose is at most 500 ms old. Each corner finder is re-found from where the previous pose puts it:
an intensity-weighted centroid over a disc of twice the finder's expected diameter (nothing else bright is that close to a
corner finder), then centroids over the finder's own disc, at most five, until the centre moves less than ¼ px; a finder
with less than 20 levels of contrast in its disc, or ending more than 0.75 diameters from where it was expected, is lost.
The mean movement of the corners that were followed predicts the others. A corner inferred in the previous frame is seen
again only when its blossom is re-found within ¼ diameter of that prediction (a bright thumb beside it must not count).
With one corner missing, it is placed at the prediction and refined against the rings as in step 2; with two missing, or
when no lane decodes, the frame goes through the full search. The orientation is kept; the distortion search of step 6 starts
from the previous frame's distortion.

The constants above are what the ports use; they may be tuned together with the fixtures. Decoding is not bit-reproducible
across platforms (it uses the platform's `sin`, `cos`, `exp` and `atan2`): conformance means the golden captures decode to the
recorded lanes, never that two platforms produce identical floating-point intermediates.

## 8. Qualification

All numbers below are from the camera simulator in `crates/iroha_petal/src/sim.rs`
(perspective and rotation, lens distortion, area integration, defocus blur, bloom, auto-exposure clipping,
ambient light and gradients, vignetting, sensor noise, sharpening, 8-bit quantisation, frame blending and tearing).
They are evidence from a model, **not** device measurements. A modern phone at 720p sees about 7.5 px per petal
pitch, a 480p preview about 4.7.

Per-frame decode rates (`qualify`, 40 random poses per row, rotation 0–360°, tilt up to 12° unless stated). "Field" is the
share of the four field lanes read; "B/frame" the payload bytes a capture yields (80 per field lane, 16 for lane `D` on three
frames in four); format 1 for comparison:

| Condition | Any lane | `D` | Field | B/frame | Format 1 B/frame |
| --- | --- | --- | --- | --- | --- |
| modern 720p | 100 % | 100 % | 100 % | 332 | 109 |
| legacy 720p, σ 1.3 px, noise 6, tilt 15°, barrel distortion | 100 % | 100 % | 100 % | 332 | 109 |
| 480p, fill 0.5–0.9, σ 1.2 px (2.9–4.2 px per petal) | 100 % | 100 % | 100 % | 332 | 29–67 |
| 720p blur σ 0.7 / 1.4 / 1.8 / 2.2 px | 100 % | 100 % | 100 % | 332 | 109 / 109 / 71 / 29 |
| 720p defocus σ 2.6 / 3.2 / 4.0 px | 100 % | 100 % | 98 / 41 / 0 % | 324 / 142 / 12 | 29 |
| 480p defocus σ 2.0 / 2.6 / 3.2 px | 100 % | 100 % | 48 / 0 / 0 % | 164 / 12 / 12 | 29 / 29 / 22 |
| 720p noise 3–13 levels | 100 % | 100 % | 100 % | 332 | 109 |
| 720p tilt ≤ 25° / 35° / 45° | 100 / 100 / 82 % | same | 100 / 100 / 82 % | 332 / 332 / 274 | 109 / 107 / 94 |
| harshest preset (480p, σ 1.7, hand shake, tilt 22°, bloom, ambient 10 %, barrel) | 100 % | 100 % | 42 % | 148 | 29 |

The field reads wherever the camera resolves about 3 px per petal and the blur stays below about a third of the petal pitch;
the rings carry on alone beyond that (deep defocus), at 12 B per frame against format 1's 29. A modern or legacy camera
reads every lane of every capture, three times format 1's payload per frame.

Light, motion and framing that are not nominal (`stress` example, 24 poses per cell; each cell lists a modern 720p / a
legacy 720p / the harshest 480p camera; light levels are fractions of the lit level; hand shake is a linear blur of the given
length). The last column is format 1's katakana lane `K`, which carried most of its payload:

| Condition | Any lane | `D` | Field | B/frame | Format 1 `K` |
| --- | --- | --- | --- | --- | --- |
| veiling light 0.35 / 0.5 | 100 / 100 / 100 % | 100 / 100 / 100 % | 100 / 100 / 30–5 % | 332 / 332 / 109–29 | 100 / 98–100 / 0 % |
| auto-exposure gain 1.5× / 2× too high | 100 / 100 / 100 % | 100 / 100 / 100 % | 100 / 100 / 46–47 % | 332 / 332 / 159–162 | 100 / 78–100 / 0 % |
| auto-exposure gain 3× too high | 100 / 100 / 100 % | 100 / 100 / 100 % | 100 / 100 / 0 % | 332 / 332 / 12 | 100 / 0 / 0 % |
| illumination gradient equal to the lit level | 100 / 100 / 46 % | 100 / 100 / 42 % | 100 / 100 / 4 % | 332 / 332 / 18 | 100 / 90 / 0 % |
| glare 0.5 × lit, σ 120 px | 100 / 100 / 88 % | 100 / 100 / 83 % | 100 / 100 / 17 % | 332 / 332 / 63 | 100 / 100 / 0 % |
| glare 1.0 × lit, σ 120 px | 100 / 92 / 12 % | 75 / 71 / 8 % | 100 / 75 / 1 % | 329 / 248 / 4 | 100 / 60 / 0 % |
| banding (display PWM) 30 % deep, 30 px period | 100 / 100 / 100 % | 100 / 100 / 100 % | 100 / 100 / 14 % | 332 / 332 / 55 | 100 / 98 / 0 % |
| banding 50 % deep, 80 px period | 100 / 96 / 25 % | 100 / 75 / 8 % | 59 / 52 / 7 % | 202 / 176 / 24 | 8 / 0 / 0 % |
| hand shake 4 px | 100 / 100 / 100 % | 100 / 100 / 100 % | 100 / 100 / 15 % | 332 / 332 / 59 | 60 / 12 / 0 % |
| hand shake 9 px | 100 / 100 / 100 % | 100 / 100 / 100 % | 84 / 83 / 0 % | 282 / 279 / 12 | 0 / 0 / 0 % |
| hand shake 14 px | 100 / 100 / 79 % | 100 / 100 / 79 % | 0 / 0 / 0 % | 12 / 12 / 10 | not measured |
| a thumb over one corner blossom | 100 / 100 / 58 % | 100 / 100 / 58 % | 100 / 100 / 29 % | 332 / 332 / 100 | 100 / 95 / 0 % |
| a glare spot on one corner blossom | 96 / 100 / 83 % | 96 / 100 / 83 % | 96 / 100 / 30 % | 318 / 332 / 107 | 95 / 5 / 0 % |
| one corner outside the frame | 100 / 100 / 71 % | 100 / 100 / 71 % | 100 / 100 / 15 % | 332 / 332 / 55 | 98 / 95 / 0 % |

The field survives the lighting problems old cameras actually have, through the normalised read and the local read of §7 step
5: a legacy camera three times over-exposed reads every field lane where format 1 lost lane `K` entirely, and veiling light, a
gradient as strong as the lit level or a 0.5 glare cost it nothing. A covered, glared or cut-off corner blossom costs at most
4 % of the field at 720p, and the harshest 480p preset reads a share of the field in 11 of these 14 conditions, where format 1
read only lane `D`. Hand shake costs the field from about 9 px of blur on (83–84 % of lanes, where `K` was gone) and all of it
at 14 px; display banding 50 % deep with an 80 px period costs about half the field lanes (`K` read 0–8 %). Lane `D` is still
the lane that holds longest, but its dots are smaller than format 1's (radius 9 against 11), and under deep banding it reads
less: 75 % against 98 % on the legacy camera, 8 % against 28 % on the 480p preset. A deep glare or wide, deep banding defeats
every lane in the frames it covers: that is why the stream is rateless and frames are shown for several camera frames.

End to end (`stream_sim`: 30 fps camera, 1/60 s exposure with blending/tearing, random pose, 8 trials per row, received
through a scan session that tracks the pose; seconds to complete, mean) for the largest KAGEMUSHA Payment (12,182 bytes) and,
after the slash, a Request-sized message (2,132 bytes). The slower decoders see only that many camera frames a second; the
rolling shutters expose the rows one after another over the readout time:

| Camera | 8 fps | 10 fps | 12 fps | 15 fps |
| --- | --- | --- | --- | --- |
| modern, legacy and soft 720p | 4.5 / 0.7 s | 3.6 / 0.6 s | 3.0 / 0.5 s | 2.4 / 0.4 s |
| harshest 480p preset | 6.0 / 1.0 s | 5.2 / 0.9 s | 4.9 / 0.9 s | 5.3 / 1.1 s |
| 720p, decoder keeping up with 20 camera frames a second | not run | 3.6 s | 3.0 s | 2.4 s |
| 720p, decoder keeping up with 15 camera frames a second | 4.5 s | 3.6 s | 3.0 s | 2.4 s |
| 720p, decoder keeping up with 10 camera frames a second | 4.5 s | 3.6 s | 3.7 s | not run |
| 720p, rolling shutter, 20 ms readout, 8 ms exposure | 4.5 s | not run | 3.0 s | 2.4 s |
| 720p, rolling shutter, 30 ms readout, 16 ms exposure | 4.5 s | not run | 3.0 s | 2.4 s |
| animated QR (Apple Vision), 720p cameras | 6.4 / 1.2 s | 5.1 s | 4.3 s | 3.4 s |

Every trial completed and none delivered a wrong payload. A 720p camera reads practically every lane of every frame, so a
payload completes after the frames its atoms need, 37 for 12,182 bytes (762 atoms at 20.75 per frame), at any display rate up
to 15 fps, under both rolling shutters, and with a decoder that keeps up with only 15 camera frames a second. A decoder slower
than the display skips frames and loses nothing else: keeping up with 10 camera frames a second it still completes in 3.7 s at
12 fps, faster than the 4.5 s of 8 fps. On the harshest 480p preset 12 fps is the fastest rate; 15 fps leaves fewer captures
per shown frame and takes 5.3 s.

The QR row is the same cameras and timeline watching the apps' animated QR stream: `IRQR` frames as ZXing draws them at ECC M,
version 13, 256-byte shards with Cauchy Reed–Solomon parity, the QR box 1.25 times the petal canvas, carrying the message body
without its 84-byte header (12,098 and 2,048 bytes), read by Apple Vision's barcode detector (`VNDetectBarcodesRequest`;
modern and legacy cameras, 6 trials per rate above 8 fps; the soft camera matches at 8 fps). On the harshest 480p preset one
QR trial in eight completed within 90 s. Format 1 needed 14.1 s and 2.4 s at 8 fps (20.3 s and 3.5 s on the soft camera, 54.5 s
and 9.6 s on the 480p preset). At every display rate format 2 takes about 30 % less time than animated QR: its frames carry
332 payload bytes against 256, and the camera reads practically all of them against 90–96 % of the QR frames. At the default
12 frames a second a Payment takes 3.0 s, against 6.4 s for the apps' animated QR at its 8 frames a second (4.5 s for petals
at the same 8 fps).

Decode cost (`decode_cost` example; one thread of an Apple M4 Max, the fastest of 18 runs while other work kept the machine
busy): at 1280×720 a full search takes 5.2 ms and tracking the pose of the previous frame 0.6 ms (the legacy camera with
barrel distortion 4.4 and 0.6 ms; 1920×1080 9.9 and 0.8 ms; 640×480 1.9 and 0.4 ms). A frame whose corner blossom a thumb
covers costs 15.2 ms the first time (the corner search against the rings) and 2.6 ms per tracked frame. A merge of three
captures costs 0.7 ms. Through a scan session the simulated 30 fps camera costs 0.8–2.9 ms per camera frame on the 720p
cameras and 6–10 ms on the 480p preset. Format 1 needed about 7.5 ms for a full search, 2 ms for a tracked frame and 21 ms
with a hidden corner. Ports are expected to stay within a factor of 20 of the reference.

On a phone: a Pixel 6 held to its four Cortex-A55 cores at 1.8 GHz (`taskset 0f`; the core type that budget 4 GB phones pair
with a few faster ones), running the Kotlin SDK on ART, decoded the camera frames of simulated trials (12,182 bytes, one pose)
through a scan session that, as CameraX's analyser does, takes only the newest camera frame whenever it is free. The older
720p camera costs 16 ms per camera frame after warm-up (the first full search 85 ms), so all 30 camera frames a second are
decoded and the Payment completes in 2.98 s at 12 fps and 4.48 s at 8 fps, on the same camera frame as the reference decoder
fed the same frames. The harshest 480p preset costs 59 ms per frame, because its field lanes need the distortion search, the
local read and merges; about 17 camera frames a second are decoded, and 12 fps still completes first: 5.0–5.2 s against
5.5–6.1 s at 8 fps. One of the phone's X1 cores takes 1.4 ms per frame. The frames came from the simulator: no camera or
display was in this loop.

Run the evidence yourself:

```
cargo run --release -p iroha_petal --example qualify -- 40            # per-frame matrix
cargo run --release -p iroha_petal --example stress -- 24             # light, motion and hidden-corner matrix
cargo run --release -p iroha_petal --example stream_sim -- 8 12182 12 # end to end at the default 12 fps
cargo run --release -p iroha_petal --example stream_sim -- 8 12182 12 modern,legacy,soft --camera-fps 10
cargo run --release -p iroha_petal --example stream_sim -- 8 12182 12 modern --readout-ms 30 --exposure-ms 16
cargo run --release -p iroha_petal --example decode_cost              # decode time
iroha offline petal simulate --camera legacy --bytes 12182 --fps 12   # end to end, through the CLI
```

## 9. CLI

```
iroha offline petal encode   --input payload.bin --output out/ --kind 3 [--frames N] [--size 1024] [--fps 12] [--format png|gif]
iroha offline petal decode   --input-dir out/ --output payload.out
iroha offline petal inspect  --input out/frame_0000.png
iroha offline petal simulate --camera modern|legacy|soft|worst --bytes 12182 --fps 12 --trials 4
```

`encode` writes `frame_0000.png …` (or one looping `stream.gif` with the `offline-visual-codecs` feature) and
`manifest.json` (`iroha.offline.petal.encode.v2`). `decode` reads PNG frames in file-name order from a
directory — frames extracted from a screen recording work. `inspect` prints which lanes decode, the orientation, the lens
distortion and the beacon of one frame.

## 10. Fixtures and conformance

* `fixtures/petal/petal_stream_v2.json` — constants, layout tables (finders, band slots, every sector's cell order, the
  `天` mask and the light cells, rings, data, gate and guard slots), palette, CRC-32C, xorshift32/`mix32`, whitening,
  Reed–Solomon vectors, fountain masks, the atom-id schedule, and for three streams (`tiny`, `one-pass`, `wrap`) the lane
  data, transmitted words, lit dots and lit petals of every listed frame. Encoders must reproduce all of it bit for bit.
* `fixtures/petal/petal_captures_v2.json` — twelve degraded camera luma planes (zlib + base64, row-major 8-bit) of the
  `one-pass` stream with `must_decode` lanes, the lanes the reference decoder reads (`reference_decoded`), the expected lane
  data, a `mirrored` flag and the `inferred_corner` (null, or the canonical index of a hidden corner blossom); two tracking
  pairs (`tracks`: a first frame to decode and a second frame to read by tracking its pose, with `must_track`,
  `reference_tracked` and `inferred_corner`); one merge set (`merges`: four 480p captures of one shown frame under one pose,
  each with its own sensor noise, with per-capture `must_decode` and `reference_decoded`, `must_merge` for the field lanes the
  merge of §7 step 8 must recover although no capture reads them alone, and `reference_merged`); one mixed set
  (`mixed_merges`: four captures whose rings show `frame` and whose petal field shows `petals_frame`, with `reference_merged`
  for what the merge of their field evidence reads); and two negatives. Lanes are named by letters: `D` and the sector digits
  `0`–`3`. A decoder must read the required lanes, must never return a lane with wrong data, must reject the negatives, and
  conforming ports reproduce `reference_decoded`, `reference_tracked`, `reference_merged` and the inferred corners exactly. A
  scan session fed the captures of a merge set in order (30 fps) must recover its `must_merge` lanes, and fed a mixed set it
  must merge but never offer a lane, since the merged lanes name `petals_frame`. The captures cover a sharp and a rotated
  camera, an older soft camera with barrel distortion, two blurred 480p cameras, a mirrored front camera, over-exposure,
  veiling light, a shadow band that only the local read gets through, a covered blossom and a corner outside the frame (both
  inferred), and a cheap lens whose barrel distortion the decoder must search for.
* Regenerate with `cargo run -p iroha_petal --example gen_fixtures -- fixtures/petal/petal_stream_v2.json` and
  `cargo run --release -p iroha_petal --example gen_captures -- fixtures/petal/petal_captures_v2.json`; `cargo test -p iroha_petal`
  fails if the files and the reference drift apart.

Implementations: Rust (`crates/iroha_petal`, `iroha offline petal`), Swift (`IrohaSwift`), Kotlin/Java (`kotlin/core-jvm`,
`kotlin/client-android`), JavaScript (`javascript/iroha_js`), Python (`python/iroha_petal`), C# (`csharp`).

## 11. Security considerations

* Frames are untrusted input. Decoders bound their work (`max_pixels` 12 MP, payload limit, bounded buffers) and never panic
  on malformed images; garbage fixtures are part of the suite.
* Reed–Solomon decoding with erasures can accept a word that is not a transmission: measured on uniformly random words with
  random byte confidences, one decoding of lane `D` accepts 3 in 40 000 (0.0075 %) and of a field lane none in 40 000. (Lane `D`
  used to try a seventh erasure; that accepted 0.4 % of random words.) The stream layer absorbs what is left: lane headers carry
  the stream tag (a stranger passes with probability 1/256) and the payload CRC-32C is verified before delivery, so a false
  accept costs at most an integrity reset, never a wrong payload.
* The payload CRC-32C and the Reed–Solomon codes detect corruption, not forgery. A screen in front of the camera can always show
  garbage or a different stream; the application must authenticate the payload (KAGEMUSHA messages carry their own proofs and
  signatures) and must not treat a completed Petal stream as authentic.
* A visible stream is readable by anyone who films it. Encrypt payloads that must stay private before streaming them.
* A stream that never completes (jamming) only wastes the user's time; sessions time out.

## 12. Open work

* TODO: physical-device qualification of the matrices in §8, including the OLED/PWM and rolling-shutter effects, the
  exposure-compensation guidance, and the 12 fps default with a real camera on an older 4 GB Android phone.
