---
title: Petal Stream Transport
---

Petal Stream is the animated optical transport of Iroha. A sender shows a sequence of frames, each
a card made of a square code with a band of katakana tiles above and below it; a phone camera
reads them; a payload (for KAGEMUSHA, an `IPM1` peer message of up to about 12 KB) is reassembled
on the other side. It is the "Sakura-storm" alternative to a stream of QR codes: a storm of sakura
petals that a plain phone camera reads faster than an animated QR code and that still reads where
the QR code does not.

This document is normative for the wire format, the frame geometry and the receiver rules. The
reference implementation is the Rust crate `crates/iroha_petal`; the golden vectors in
`fixtures/petal/` are the cross-SDK conformance suite (§10). Where prose and code disagree, the
code and the fixtures win and this document is wrong.

## 1. Overview

A frame is a black card of 1024 × 1668 units: a square code of 1024 × 1024 with a band of katakana
tiles above and below it. The square carries three kinds of marks, two of which carry data and one
locates the code; the bands carry data too:

| Mark | What it is | Role |
| --- | --- | --- |
| **Sakura finders** | Four solid five-petal blossoms in the canvas corners | Locate the code in a camera frame and give the perspective; any three suffice when a thumb, a glare or the frame edge hides one. |
| **Petal field** | 3,376 petals on 34 concentric bands, each lit or dark, and turned | One bit per petal, in four sectors (lanes `F0`–`F3`). On top of each sector, the first 360 of its lit petals are turned by 0°, 45°, 90° or 135° about their centres: two more bits each, one *turn lane* per sector (`T0`–`T3`). Petals inside the SORA `天` are drawn white, the others pink, over a faint `天`, so the character shows through the storm. |
| **Three dotted rings** | 88, 100 and 108 dot slots at radii 405, 440 and 475 | Which slots are lit carries 240 bits (lane `D`). Fixed *gates* at 3, 6 and 9 o'clock (none at 12) are always lit. |
| **Katakana bands** | 306 tiles above the square and 306 below it, each a rounded square showing one of sixteen katakana, light or dark | Eight tiles of each band are pilots with fixed symbols. Each of the other 298 shows one bit as its polarity (lanes `P0` above, `P1` below) and four bits as its katakana (lanes `K0`, `K1`). The bands lie in the part of an upright phone's camera frame that the square leaves unused. |

Each lane is exactly one Reed–Solomon codeword over GF(256), so the lanes fail independently:

* lane `D` (big isolated dots) is the most robust: it survives defocus of 4 px at 720p and carries
  the stream beacon;
* the four field lanes carry most of the payload. A petal is read by its brightness alone, with no
  shape to recognise, so the field survives blur that would defeat a QR code of the same size;
* the four turn lanes carry 64 more payload bytes per field lane, read from the shape of the lit
  petals once their field lane has said which petals are lit. A turned petal stays inside its own
  cell, so the turns cost the field nothing; a camera that resolves the petals' shapes (a 720p
  camera at arm's length) reads them as well, and a blurrier one reads the rest of the frame as
  before;
* the four kana lanes carry 128 more payload bytes per band, read from the katakana tiles where the
  square's pose, extended beyond the square and corrected by each band's pilots, places them. A
  phone held upright sees both bands; a camera that frames only the square reads the square as
  before.

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

This is format 5: format 3's square with the katakana bands (4 was a laboratory study that was not
adopted). Format 3 (the square alone), format 2 (rounder, unturned petals) and format 1 (a `天`-shaped
field of 256 katakana tiles) are retired; their beacon versions `0x30`, `0x20` and `0x10` are refused
(§5.3).

## 2. Frame geometry (normative)

All coordinates are *design units* in the square's coordinates: the square spans `0 … 1024` on both
axes, origin at its top-left, `y` growing downward. The card adds a band of 322 units above the square
(`y = −322 … 0`) and one below it (`y = 1024 … 1346`): it is 1024 × 1668 units, and the square starts
322 units below its top edge. A renderer scales the card to any pixel width (recommended at least
512 px; 768–1080 px on phones). Angles are measured from the positive `x` axis and grow clockwise on
the screen.

### 2.1 Palette

| Name | RGB |
| --- | --- |
| background | `(0, 0, 0)` |
| light (finders, petals inside the `天`, light tiles) | `(250, 235, 244)` |
| pink (ring dots, petals outside the `天`, glyphs on dark tiles) | `(245, 175, 208)` |
| backdrop (the faint `天`) | `(25, 24, 24)` |
| ink (glyphs on light tiles) | `(20, 4, 14)` |

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

**Petals.** A lit cell is drawn as a petal: an ellipse of half-length 6.0 along its *axis* and
half-width 3.73, centred on the cell, minus a notch (a disc of radius 1.12 centred 6.24 units along
the axis from the cell centre). The axis is the outward radial direction `θ` turned clockwise by the
cell's turn (§3.3), which is 0 for every petal that carries no turn bits. Renderers must draw the
notch: the turn reader's templates include it. A petal reaches at most 6.0 units from its centre
whichever way it is turned, and no two cell centres are closer than 11 units, so no petal reaches
into another cell's disc of radius 4.6 around its centre, which is all the field lanes sample. A
dark cell is not drawn.

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

### 2.5 Katakana bands

Each band is a lattice of 10 rows by 35 columns with pitch 29, its columns starting at `x = 4.5`
(centred on the card); band 0's rows start at `y = −314` (8 units below the card's top edge) and band
1's at `y = 1048` (24 units below the square). The cell of row `r` and column `c` holds a **tile**: a
25 × 25 square with corners rounded to radius 3, inset 2 units into its cell, whose top-left corner is
`(6.5 + 29c, −312 + 29r)` in band 0 and `(6.5 + 29c, 1050 + 29r)` in band 1. A cell whose tile centre
lies closer than 198 units to a finder centre has no tile (a finder diameter of clearance from the
blossom's edge, plus the tile's half-diagonal), which leaves 306 tiles in each band. Tiles are
numbered band 0 first, each band in reading order (rows top first, columns left first); the list is
in the fixture.

Eight tiles of each band are **pilots** with fixed symbols; band 1's pilots mirror band 0's rows:

| Pilot | Band 0 (row, column) | Band 1 (row, column) | Glyph | Polarity |
| --- | --- | --- | --- | --- |
| 0 | (0, 0) | (9, 0) | ロ | light |
| 1 | (0, 17) | (9, 17) | ケ | dark |
| 2 | (0, 34) | (9, 34) | ヒ | light |
| 3 | (4, 0) | (5, 0) | ム | dark |
| 4 | (4, 34) | (5, 34) | ス | dark |
| 5 | (9, 8) | (0, 8) | ワ | light |
| 6 | (9, 17) | (0, 17) | ア | dark |
| 7 | (9, 26) | (0, 26) | カ | light |

The other 298 tiles of a band are its **data tiles**, numbered `0 … 297` in reading order.

**Glyphs.** A tile shows one of sixteen katakana, symbols `0 … 15`: イ ロ ハ ニ ヘ ト ワ カ レ ム ノ ケ ア
ヒ ス ン (members of the Iroha order that stay distinct under blur). Each glyph is a set of stroke
polylines on a 32 × 32 design grid (`y` down; the polylines are in the fixture) drawn with round caps
and joins of width 6.5: a design point is *inked* when it lies within 3.25 of a stroke segment. The
glyph box is the tile inset by 1 unit (23 × 23), so the tile-local point `(u, v)` (`0 … 25` from the
tile's top-left corner) is inked when it lies inside the rounded tile and the design point
`((u − 1)·32/23, (v − 1)·32/23)` is inked.

**Polarity.** A *light* tile is filled with the light colour and its glyph is drawn in ink; a *dark*
tile is not filled (black) and its glyph is drawn in pink. The gutters between tiles are black.

## 3. Lanes (normative)

### 3.1 Cells to bytes

* **Lane `D`**, 30 bytes: bit `b` is `1` iff data slot `b` is lit. Byte `j` holds bits `8j … 8j+7`,
  most significant bit first.
* **Lanes `F0`–`F3`**, 105 bytes each: bit `b` of lane `Fs` is `1` iff cell `b` of sector `s`'s bit
  order is lit. Byte `j` holds bits `8j … 8j+7`, most significant bit first.
* **Lanes `T0`–`T3`**, 90 bytes each: see §3.3.
* **Lanes `P0`, `P1`** (polarity), 37 bytes each: bit `b` (`0 … 295`) of band `n`'s lane is `1` iff
  data tile `b` of band `n` is light; data tiles 296 and 297 are always light. Byte `j` holds bits
  `8j … 8j+7`, most significant bit first.
* **Lanes `K0`, `K1`** (glyphs), 149 bytes each: data tile `t` of band `n` shows the glyph whose
  symbol is nibble `t` of the lane: the high nibble of byte `⌊t/2⌋` for even `t`, the low nibble for
  odd `t`.

### 3.2 Codewords

The transmitted bytes of a lane are `RS(data ‖ parity) XOR whitening`.

| Lane | Codeword | Data | Parity | Corrects |
| --- | --- | --- | --- | --- |
| `D` | 30 B | 19 B | 11 B | 5 wrong bytes, or 11 erased |
| `F0`–`F3` | 105 B | 83 B | 22 B | 11 wrong bytes, or 22 erased |
| `T0`–`T3` | 90 B | 68 B | 22 B | 11 wrong bytes, or 22 erased |
| `P0`, `P1` | 37 B | 20 B | 17 B | 8 wrong bytes, or 17 erased |
| `K0`, `K1` | 149 B | 116 B | 33 B | 16 wrong bytes, or 33 erased |

**Reed–Solomon.** GF(256) with primitive polynomial `x⁸+x⁴+x³+x²+1` (`0x11D`), `α = 2`. The generator is
`∏_{i=0}^{nsym-1}(x − αⁱ)` (first root `α⁰`), the code is systematic with the data bytes first and the
highest-degree coefficient first (the same field and conventions as QR Code). Decoders must support
errors and erasures (`2·errors + erasures ≤ nsym`) and must verify that the corrected word has zero
syndromes before accepting it.

**Whitening.** A fixed pseudo-random byte string is XORed over the codeword so that every frame is
statistically balanced whatever the payload is (about half the petals are lit). The generator is
xorshift32 (`x ^= x << 13; x ^= x >> 17; x ^= x << 5`, a zero seed becomes `0xDEADBEEF`); each output
byte is the top byte (`x >> 24`) of the state after an update. Seeds: `D = 0x444F5453`,
`Fs = 0x50455430 + s`, `Ts = 0x54524E30 + s`, `Pn = 0x504F4C30 + n` ("POL0", "POL1"),
`Kn = 0x4B414E30 + n` ("KAN0", "KAN1"). The first `word length` bytes of the sequence are used;
the sequences are in the fixture. A codeword read under the wrong sector's whitening fails its
check, so a decoder that reads the field in a wrong orientation reads nothing.

### 3.3 Turn lanes

Once a field lane is read, the decoder knows exactly which petals of its sector are lit, and the
turn lane on top of it uses them:

* **Carrier order.** Sector `s`'s carrier order is its 840 codeword cells (the first 840 of its bit
  order) shuffled by Fisher–Yates with xorshift32 seeded `0x4F524430 + s` ("ORD0" … "ORD3"): for `i`
  from 839 down to 1, positions `i` and `next_u32() mod (i + 1)` swap, where `next_u32()` is the
  state after an update (§3.2). The orders are in the fixture.
* **Carriers.** The carriers of sector `s` are the first 360 lit cells (per lane `Fs`) in its carrier
  order. A sector with fewer than 360 lit cells (about one sector in 60 000) has no carriers and no
  turn lane.
* **Turns.** Carrier `j` (`0 … 359`) shows bits `2j` and `2j + 1` of the transmitted turn codeword
  (byte `⌊j/4⌋`, most significant bits first) as a two-bit label `L`. Its *turn position* is
  `p = L ^ (L >> 1)` (so `L` is the Gray code of `p`: neighbouring turns differ in one bit, 135° and
  0° included), and its petal is turned clockwise by `π·p/4` (0°, 45°, 90° or 135°) from its outward
  direction. Every other petal is unturned.
* **Codeword.** The turn lane is one Reed–Solomon codeword of 90 bytes, 68 data and 22 parity
  (§3.2), XOR-whitened with the sequence `Ts`; its data layout is in §5.3.

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

The turn lane `Ts` of frame `f` carries four atoms with ids `2³¹ + 16·f + 4·s` to `2³¹ + 16·f + 4·s + 3`:
repair atoms only, in a range the field lanes and lane `D` never reach (their ids stay below
`21 · 65 536`). The square thus carries up to 37 atoms (592 bytes), 36.75 on average.

The kana lanes of frame `f` carry repair atoms from their own range above the turn lanes': lane `Kn`
seven atoms with ids `2³¹ + 2²⁰ + 16·f + 8·n` to `… + 6`, lane `Pn` one atom with id
`2³¹ + 2²⁰ + 16·f + 8·n + 7`. A card thus carries up to 53 atoms (848 bytes), 52.75 on average.

### 5.3 Lane data layouts (big-endian)

Every lane but the turn lanes starts with a 3-byte header `tag:u8, frame:u16`. Then:

* lanes `F0`–`F3`: five atoms (80 B) → 83 B;
* lane `D`, non-beacon frame: one atom → 19 B;
* lane `D`, beacon frame (`f mod 4 = 0`): `version:u8 (=0x50), kind:u8, len:u24, crc:u32` followed by 7 zero bytes → 19 B;
* lanes `T0`–`T3`: four atoms (64 B), then `check:u32` → 68 B. A turn lane has no header of its own: it
  rides on the field lane of its sector, whose header names the stream and the frame, and `check` is
  the CRC-32C of `tag:u8, frame:u16, sector:u8` (that header and the sector) followed by the four
  atoms. A decoder accepts a turn lane only when the check matches the header of the field lane it was
  read on;
* lanes `P0`, `P1`: one atom, then `check:u32` → 20 B; lanes `K0`, `K1`: seven atoms (112 B), then
  `check:u32` → 116 B. A kana lane has no header: `check` is the CRC-32C of `tag:u8, frame:u16, lane:u8`
  followed by the atoms, where `lane` is `0x10 | n << 1 | kind` for band `n` (`kind` 0 for polarity, 1
  for glyphs). A decoder takes `tag` and `frame` from the square's lanes and accepts a kana lane under
  that frame, the one after it or the one before it (a rolling shutter or a stale display can show a
  band from a neighbouring frame); the frame whose check matches gives the atoms' ids.

`version` = `0x50` means format version 5, layout profile 0; anything else is rejected (format 3's
`0x30`, format 2's `0x20` and format 1's `0x10` included).

### 5.4 Sender

Send frames `f = f₀, f₀+1, …` at 8–15 frames per second; **12 is the default**: a camera reads practically every frame at
12 fps, and a phone's slowest cores decode 14 to 19 of its camera frames a second, more than one capture per shown frame
(§8). The sender repeats until the user stops it; the first
`⌈K / 20.75⌉` frames carry every source atom once in their field lanes and lane `D`, and everything else, the turn and kana
lanes included, is repair atoms. There is no back-channel; if the receiver needs an acknowledgement it uses its own stream.

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
* Turn lane atoms enter only through a turn lane whose check matches its field lane's header (§5.3).
* Kana lane atoms enter only through a kana lane whose check matches the stream tag and one of the
  frames tried (§5.3).
* A completed stream is delivered exactly once; further frames of the same stream are ignored.
* Sessions forget a partial stream after an idle timeout (default 30 s) or an absolute timeout (default 180 s).

## 6. Rendering

Any 2D API may draw a frame as long as geometry, polarity and colours match §2. The reference
software renderer (`render.rs`) draws the whole card with 2×2 or 3×3 supersampling: in the square it
shades each sample in this order: finders, ring dots, petals, the backdrop `天`, background; in the
bands each sample takes its tile's colour (§2.5) or black. SDKs additionally expose a platform-neutral
*draw list* (finder discs and notches, backdrop cells, lit petals with centre, angle (the outward
direction plus the turn) and colour, lit dots, and tiles with their glyph and polarity over the sixteen
glyphs' stroke polylines) so a native canvas can play a stream without per-pixel work. Petals are
small (12 × 7.5 units) and tiles 25 units: draw them antialiased, notches included, at 768 px or more.

Display guidance: full screen brightness, a black surround, the whole card visible (on a phone the
card's width fills the screen's width), no overlays within one finder diameter of the finders,
animation at 12 fps (§5.4). A frame must stay up for at least
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
9. **Turn lanes.** The turn lanes are read from a capture whose field lanes decoded on their own (merged lanes are not used)
   and whose bands read no kana lane (step 10). A scan session tries a turn lane once per shown frame: it remembers the last 24 `(tag, frame, sector)` it tried, read or
   not (the four sectors of the last 6 shown frames, cleared with the held evidence when the stream changes), and passes
   those sectors over. Reading turns costs a slow phone core several times a frame's decode (§8), so after a try that reads
   no turn lane the session passes over the next captures that would try one: one capture after the first such try in a row,
   two after the second, and so on up to 8, until a try reads a lane (which resets the count). The order within a capture:
   sectors already tried are passed over; when none is left nothing happens; when captures are still to be passed over, one
   is used up and nothing happens; otherwise every remaining decoded sector is recorded as tried and then read.
   1. *Carriers.* The lit cells of every decoded sector come from re-encoding its field lane (§3), and its carriers from §3.3.
      The carriers of every decoded sector are decided below, so that each carrier's neighbours are modelled at their own
      turns; only the sectors not passed over are decoded in step 8.
   2. *Windows.* A carrier's window is 13 × 13 samples one canvas unit apart in its petal's outward frame: `a` along the outward
      direction `θ`, `b` across it, `a, b = −6 … 6`, stored by rows of `b` and columns of `a`. With `o` the image point of the
      window's centre and `u = pose(centre + (cos θ, sin θ)) − o`, `v = pose(centre + (−sin θ, cos θ)) − o`, sample `(a, b)` is the
      bilinear luma at `(o + b·v) + a·u`. A window is *normalised*: its 17th smallest sample (the 10th percentile of 169)
      becomes 0 and its 166th smallest (the 98th) becomes 1 (a span below one grey level counts as one), clamped to −0.2 … 1.2.
   3. *Warp.* The pose is good enough for brightness but not for shapes: a lens model or homography error of a unit moves a
      petal against its window. For every 17th carrier (carriers `0, 17, 34, …` in order, sector by sector) the normalised
      window centred on the cell gives a *centroid*: every sample weighs `clamp(level, 0, 1) · g(a − eₐ) · g(b − e_b)` with
      `g(t) = exp(−t² / (2·2.6²))`, from `(eₐ, e_b) = (0, 0)` recomputed three times (stopping when the weights sum to zero).
      The centroid's canvas offset `(eₐ cos θ − e_b sin θ, eₐ sin θ + e_b cos θ)` is fitted, per axis, by the least-squares
      cubic in `x = (X − 512)/384`, `y = (Y − 512)/384` at the cell centres `(X, Y)`, with the monomials
      `1, x, y, x², xy, y², x³, x²y, xy², y³` (normal equations summed in sample order, 10⁻⁶ added to the diagonal,
      Gauss–Jordan elimination with partial pivoting, a pivot below 10⁻¹² skipping its column). Every carrier's window is then
      sampled centred on its cell centre plus the fitted offset, and normalised.
   4. *Templates and kernels.* The template of blur `σ` (`σ = 1.0, 1.4, 1.9, 2.5, 3.2, 4.0` canvas units) is the unturned
      petal of §2.3 tabulated on 73 × 73 cells: cell `(i, j)` covers `along = −9 + i/4`, `across = −9 + j/4`, and its coverage is
      the share of the 4 × 4 points `(along + (m − 1.5)/16, across + (n − 1.5)/16)` inside the petal. The coverage is blurred by
      a Gaussian truncated at `⌈3σ / 0.25⌉` cells (weights `exp(−(k/4)² / (2σ²))` normalised by their sum, cells outside the
      table counting as zero), first along `i`, then along `j`. A template value is the bilinear interpolation of the table, and
      0 outside `−9 ≤ along, across < 9`. The *kernel* of turn position `p` is the template on the window grid turned by
      `φ = π·p/4`, weighted: `w(a, b) · T(a cos φ + b sin φ, −a sin φ + b cos φ)` with `w(a, b) = exp(−(a² + b²) / (2·3.6²))`.
      A window's *score* for `p` is the sum of window × kernel; its best position is the highest score (ties to the lower `p`),
      and its *lead* is the best score minus the next highest.
   5. *Neighbours.* The lit cells (of decoded sectors) within 19 units of a carrier, itself excluded, are its neighbours. A
      neighbour is modelled by the template turned to its own axis (its outward direction plus its turn, 0 when it carries
      none), added to or subtracted from a window at the grid points within 9.5 units of the neighbour's centre.
   6. *Blur.* For every 17th carrier and each blur in turn, the model is the carrier's template at its best position plus its
      neighbours at theirs (each carrier neighbour at the best position of its own window, under the same blur), and the
      sample scores `Σ w·x·m / max(√(Σ w·m² · Σ w·x²), 10⁻⁹)` over the window `x` and the model `m`. The blur with the largest
      sum fits best (ties to the sharper). From a fitted blur of 3.2 on, the camera cannot resolve the turns and the reader
      stops.
   7. *Turns.* Under the next wider blur than the fitted one (4.0 stays 4.0), every carrier takes the best position of its own
      window; then, carrier by carrier in order, it takes the best position of its window minus its neighbours at their latest
      positions, with that window's lead as its confidence.
   8. *Codeword.* The positions give the labels `p ^ (p >> 1)`, packed as in §3.3, and a byte's confidence is the least of its
      four carriers'. Reed–Solomon with erasures as in step 7 (0, 2, 5, 7 and 11 erasures), accepting the first word whose
      check (§5.3) matches the sector's field lane header.
10. **Kana lanes.** The bands are read from a capture whose square decoded, under the `(tag, frame)` header its lanes name.
   A scan session reads each band once per shown frame: it remembers the last 12 `(tag, frame, band)` it tried and passes
   those bands over, and after a try that reads no kana lane it passes over the next captures that would try one, as for the
   turn lanes (step 9) but with its own count (at most 8). The bands go before the turn lanes: a capture first tries the
   bands, and only when that reads no kana lane (no band is left to try for its shown frame, the capture is one the
   back-off passes over, or the try reads nothing) does it try the turn lanes on the same capture. A capture whose bands
   read a kana lane tries no turn lane, and that shown frame's turn lanes wait for its next capture. On a slow phone core a
   turn try costs about five tracked decodes and a band try about one (§8): both on one capture took longer than a frame is
   shown at 12 fps, and the session missed the next shown frames.
   1. *Placing.* The square's pose, applied to points beyond the square, places every tile. A tile one of whose corners maps
      outside the image is not read.
   2. *Samples.* A tile is sampled at a grid of points, moved by the tile's shift: the pose maps the tile's centre `o` (its
      top-left corner plus 12.5 units on both axes, plus the shift) and the points one unit right of it and one unit below it,
      `o + u` and `o + v` in the image, and grid point `(a, b)`, measured from the tile's centre, is the bilinear luma at
      `(o + b·v) + a·u`. The fine grid has 12 × 12 points `a, b = (i + ½)·25/12 − 12.5`, the coarse grid 6 × 6 points
      `(i + ½)·25/6 − 12.5` (rows `b` outer, columns `a` inner). The samples are taken minus their mean; that mean is the
      tile's *level* and the sum of their squares its *energy*.
   3. *Templates.* For every blur, two coverage rasters of a tile: points `−4 + a/2` units (`a = 0 … 66`) on both axes, each
      the share of its 4 × 4 sub-points `(m − 1.5)/8` units around it that lie inside the rounded tile (the *inside*) or on the
      glyph's strokes (the glyph's *ink*), blurred by a Gaussian of `σ` (taps every ½ unit out to `⌈3σ / ½⌉`, weights
      `exp(−d² / (2σ²))` normalised by their sum, points outside counting as zero; rows first), sampled bilinearly at the
      fine grid points and mean-centred. A dark tile's template is the ink, a light tile's is the inside less the ink, point
      by point; each also has a unit-length copy (divided by its length, at least 10⁻¹²). The blurs are
      `σ = 1.0, 1.6, 2.4, 3.4, 4.8` units. The *coarse templates* are the same under `σ = 3.4` at the coarse grid points,
      unit length. Probes of both are in the fixture.
   4. *Pilots.* The *correlation* of samples `x` with a unit template `t` is `Σ x·t / √energy` (0 when the energy is at most
      10⁻⁹). Every pilot in view is searched on the coarse grid under its coarse template at the shifts `(2·dx, 2·dy)` units
      within its reach: 4 units plus 10 for every 290 units its centre lies beyond the square's edge (above `y = 0` or below
      `y = 1024`), at most 12 more, for the lens distortion the pose extends beyond the square. Shifts run `dy` outer and `dx`
      inner, both ascending, and a later shift wins only with a larger correlation; a pilot is kept when its best is at least
      0.35, and with fewer than two kept the band is not read. On the fine grid, the band's blur is the one under which the
      kept pilots, each at the best of its shift and the eight shifts one unit around it, correlate best in sum (ties to the
      sharper). Each kept pilot is then refined under that blur from its coarse shift to the best of the eight shifts around
      it at steps of 1, ½ and ¼ unit in turn; a pilot whose final correlation is below 0.5 is dropped, and with fewer than two
      left the band is not read.
   5. *Shift field.* The band's shift at a tile centre `(X, Y)` is, per axis, `a₀ + a₁·x + a₂·y` with `x = (X − 512)/512` and
      `y = (Y − ȳ)/145` around the pilots' mean `ȳ`, fitted to the pilots' shifts by least squares (normal equations,
      Gauss–Jordan elimination with partial pivoting); with fewer than three pilots, or a pivot below 10⁻⁹, it is the pilots'
      mean shift.
   6. *Polarity.* A tile's level is the mean of its samples. The light pilots' levels and the dark pilots' levels are each
      fitted by a least-squares line in `x` (a constant when they share one `x`; a band without a light or without a dark
      pilot is not read). The threshold at a tile is the midpoint of the two lines there and half their difference is the
      half-contrast; a tile where the half-contrast is at most one grey level is not read. A tile is light when its level
      exceeds the threshold, with `|level − threshold| / half-contrast` as its confidence.
   7. *Glyphs.* Under the band's blur, with `A = Σ x·inside` and `B = Σ x·ink` over the fine samples `x` and the mean-centred
      templates, a glyph's light-tile correlation is `(A − B) / |inside − ink| / √energy` and its dark-tile correlation
      `B / |ink| / √energy` (both 0 when the energy is at most 10⁻⁹). A tile's glyph is the one whose better polarity
      correlates best (ties to the lower symbol), with that correlation minus the next glyph's as its confidence.
   8. *Codewords.* The polarity bits and glyph nibbles form the lanes' words (§3.1); a byte's confidence is the least of its
      tiles' (a tile that is not read has confidence 0 and shows 0 bits). Reed–Solomon with 0, ⅛, ¼, ⅓ and ½ of the parity
      bytes erased (polarity 0, 2, 4, 5 and 8; glyphs 0, 4, 8, 11 and 16), accepting the first word whose check (§5.3) matches
      the square's frame, the frame after it or the frame before it, in that order.

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
recorded lanes, never that two platforms produce identical floating-point intermediates. The turn and kana readers' templates
are in the fixture as probes (§10) so that a port can check its tables.

## 8. Qualification

All numbers below are from the camera simulator in `crates/iroha_petal/src/sim.rs`
(perspective and rotation, lens distortion, area integration, defocus blur, bloom, auto-exposure clipping,
ambient light and gradients, vignetting, sensor noise, sharpening, 8-bit quantisation, frame blending and tearing).
They are evidence from a model, **not** device measurements. It captures luma only, with in-camera sharpening but no
denoising, display subpixels or video compression, which the turn and kana lanes depend on more than the field lanes do.

**The phone held upright.** The sending phone shows the card upright and the receiving phone is held upright too, so the
card's long side runs along the long side of the camera sensor: in the sensor's landscape frame the card is turned by
about 90° or 270°, the square fills most of the frame's height and the bands lie beside it along the frame's width. The
simulated camera frames the square as it did for format 3. Rows marked *upright* turn the card by 90° or 270° ± 15°; there
a modern 720p camera sees about 7.0 px per petal pitch (12.5 canvas units) and 14 px per tile, the harshest 480p preset 4.4
and 8.8. The other rows turn the camera by any angle, as format 3 was measured (6.4 and 4.0 px per petal pitch); at angles
where a band lies across the frame's short side it is cut off, and about four kana lanes in ten are out of view.

Per-frame decode rates (`qualify`, 40 random poses per row, tilt up to 12°, the modern and legacy cameras' up to 15° and the
harshest preset's up to 22°). "Field", "Turn" and "Kana" are the shares of the four field lanes, the four turn lanes and the
four kana lanes read; "B/frame" the payload bytes a capture yields (80 per field lane, 64 per turn lane, 16 for lane `D` on
three frames in four, 16 per polarity lane and 112 per glyph lane), and "Bands" the part of them the kana lanes carry.
"B/frame" less "Bands" is what the square alone yields, format 3's figure for the same captures:

| Condition | Any lane | `D` | Field | Turn | Kana | B/frame | Bands |
| --- | --- | --- | --- | --- | --- | --- | --- |
| upright, modern 720p | 100 % | 100 % | 100 % | 100 % | 100 % | 844 | 256 |
| upright, legacy 720p (σ 1.3 px, noise 6, tilt 15°, barrel distortion) | 100 % | 100 % | 100 % | 85 % | 98 % | 797 | 248 |
| upright, soft 720p (σ 1.8 px, noise 6) | 100 % | 100 % | 100 % | 0 % | 100 % | 588 | 256 |
| upright, harshest preset (480p, σ 1.7, hand shake, tilt 22°, bloom, ambient 10 %, barrel) | 100 % | 100 % | 73 % | 0 % | 31 % | 266 | 20 |
| modern 720p | 100 % | 100 % | 100 % | 100 % | 63 % | 736 | 148 |
| legacy 720p | 100 % | 100 % | 100 % | 54 % | 64 % | 620 | 149 |
| 480p, fill 0.5–0.9, σ 1.2 px (2.9–4.2 px per petal) | 100 % | 100 % | 100 % | 0–1 % | 42–53 % | 362–424 | 30–92 |
| 720p blur σ 0.7 / 1.0 / 1.4 / 1.8 / 2.2 px | 100 % | 100 % | 100 % | 100 / 99 / 42 / 0 / 0 % | 62 / 62 / 62 / 57 / 41 % | 736 / 734 / 589 / 455 / 389 | 148 / 148 / 148 / 123 / 57 |
| 720p defocus σ 2.6 / 3.2 / 4.0 px | 100 % | 100 % | 96 / 38 / 0 % | 0 % | 31 / 21 / 4 % | 345 / 145 / 15 | 27 / 13 / 3 |
| 480p defocus σ 2.0 / 2.6 / 3.2 px | 100 % | 100 % | 44 / 0 / 0 % | 0 % | 18 / 3 / 0 % | 163 / 14 / 12 | 11 / 2 / 0 |
| 720p noise 3 / 6 / 9 / 13 levels, σ 1.2 px | 100 % | 100 % | 100 % | 87 / 81 / 68 / 51 % | 62 % | 702 / 686 / 653 / 611 | 148 |
| 720p tilt ≤ 0° / 15° / 25° / 35° / 45°, σ 1.2 px | 100 / 100 / 100 / 100 / 82 % | same | same | 92 / 79 / 58 / 38 / 23 % | 62 / 64 / 66 / 65 / 50 % | 717 / 684 / 636 / 580 / 444 | 148 / 149 / 155 / 152 / 111 |
| harshest preset | 100 % | 100 % | 40 % | 0 % | 16 % | 150 | 10 |

Upright, the bands add 256 bytes to every frame of the sharp and the soft 720p camera and 248 to the older one's: 44 % more
than the 588 bytes the sharp camera reads from the square, and 77 % more than the soft camera's 332, since it cannot read the
turns. A tile spans two petal pitches, so the kana lanes read where the turns do not: through 1.8 px of blur, 13 levels of
noise and 35° of tilt, as far as the bands are in view. Like the field, they give way to defocus, and the harshest 480p
preset reads only polarity lanes, about three in five of them (20 bytes a frame). At other angles the bands add about 150
bytes a frame on a 720p camera and 30–90 on a 480p one.

Light, motion and framing that are not nominal (`stress` example, 24 upright poses per cell; each cell lists a modern 720p /
a legacy 720p / the harshest 480p camera, a range spanning the row's conditions in order; light levels are fractions of the
lit level; hand shake is a linear blur of the given length):

| Condition | Any lane | `D` | Field | Turn | Kana | B/frame | Bands |
| --- | --- | --- | --- | --- | --- | --- | --- |
| veiling light 0.2 / 0.35 / 0.5 | 100 / 100 / 100 % | 100 / 100 / 100 % | 100 / 100 / 72–12 % | 100 / 85–23 / 0 % | 100 / 95–91 / 29–4 % | 844 / 783–605 / 261–55 | 256 / 233–214 / 19–3 |
| auto-exposure gain half the right one | 100 / 100 / 100 % | 100 / 100 / 100 % | 100 / 100 / 61 % | 100 / 82 / 0 % | 100 / 96 / 22 % | 844 / 780 / 223 | 256 / 237 / 14 |
| auto-exposure gain 1.5× / 2× too high | 100 / 100 / 100 % | 100 / 100 / 100 % | 100 / 100 / 79–80 % | 100 / 88–30 / 0 % | 100 / 96–88 / 33–22 % | 844 / 793–609 / 287–283 | 256 / 237–200 / 21–14 |
| auto-exposure gain 3× too high | 100 / 100 / 100 % | 100 / 100 / 100 % | 100 / 100 / 0 % | 100 / 0 / 0 % | 100 / 34 / 0 % | 844 / 354 / 12 | 256 / 22 / 0 |
| illumination gradient half the lit level | 100 / 100 / 100 % | 100 / 100 / 100 % | 100 / 100 / 69 % | 100 / 83 / 0 % | 100 / 95 / 28 % | 844 / 778 / 250 | 256 / 233 / 18 |
| illumination gradient equal to the lit level | 100 / 100 / 33 % | 100 / 100 / 29 % | 100 / 100 / 6 % | 100 / 27 / 0 % | 100 / 67 / 0 % | 844 / 532 / 24 | 256 / 131 / 0 |
| glare 0.5 × lit, σ 120 px | 100 / 100 / 92 % | 100 / 100 / 71 % | 100 / 100 / 33 % | 100 / 83 / 0 % | 94 / 85 / 11 % | 836 / 772 / 122 | 248 / 227 / 7 |
| glare 1.0 × lit, σ 120 px | 100 / 100 / 29 % | 79 / 71 / 0 % | 100 / 82 / 7 % | 100 / 60 / 0 % | 85 / 79 / 6 % | 828 / 649 / 27 | 243 / 223 / 4 |
| glare 0.8 × lit, σ 40 px | 100 / 100 / 100 % | 100 / 100 / 100 % | 100 / 100 / 48 % | 100 / 90 / 0 % | 98 / 96 / 30 % | 843 / 803 / 185 | 255 / 241 / 19 |
| banding (display PWM) 30 % deep, 30 px period | 100 / 100 / 100 % | 100 / 100 / 100 % | 100 / 100 / 20 % | 100 / 85 / 0 % | 100 / 86 / 3 % | 844 / 746 / 77 | 256 / 195 / 2 |
| banding 50 % deep, 30 px period | 100 / 100 / 29 % | 100 / 100 / 29 % | 100 / 100 / 0 % | 100 / 65 / 0 % | 98 / 30 / 0 % | 839 / 553 / 4 | 251 / 55 / 0 |
| banding 50 % deep, 80 px period | 100 / 96 / 17 % | 100 / 83 / 4 % | 52 / 47 / 5 % | 28 / 4 / 0 % | 41 / 9 / 0 % | 377 / 213 / 17 | 126 / 42 / 0 |
| hand shake 2 px | 100 / 100 / 100 % | 100 / 100 / 100 % | 100 / 100 / 61 % | 36 / 0 / 0 % | 100 / 77 / 28 % | 681 / 485 / 227 | 256 / 153 / 18 |
| hand shake 4 px | 100 / 100 / 100 % | 100 / 100 / 100 % | 100 / 100 / 40 % | 23 / 0 / 0 % | 98 / 67 / 20 % | 637 / 439 / 151 | 247 / 107 / 13 |
| hand shake 9 px | 100 / 100 / 100 % | 100 / 100 / 100 % | 95 / 92 / 0 % | 0 % | 50 / 48 / 1 % | 351 / 336 / 13 | 36 / 31 / 1 |
| hand shake 14 px | 100 / 100 / 96 % | 100 / 100 / 96 % | 0 / 0 / 0 % | 0 % | 30 / 22 / 0 % | 31 / 26 / 12 | 19 / 14 / 0 |
| a thumb over one corner blossom | 100 / 100 / 46 % | 100 / 100 / 46 % | 100 / 100 / 38 % | 100 / 93 / 0 % | 100 / 92 / 9 % | 844 / 788 / 132 | 256 / 219 / 6 |
| a glare spot on one corner blossom | 100 / 100 / 96 % | 100 / 100 / 96 % | 100 / 100 / 48 % | 96 / 42 / 0 % | 100 / 95 / 26 % | 833 / 671 / 182 | 256 / 233 / 17 |
| one corner outside the frame (the card turned about 45°) | 100 / 100 / 58 % | 100 / 100 / 58 % | 100 / 100 / 10 % | 100 / 65 / 0 % | 50 / 50 / 10 % | 716 / 625 / 47 | 128 / 128 / 7 |

The kana lanes stand up to the light as the field does. Veiling light, an auto-exposure gain from half to twice the right one,
a gradient half the lit level, glare and a covered blossom leave the sharp camera 85–100 % of them and the older camera
79–96 %, while its turns read in 23–93 %; the harshest 480p preset reads only polarity lanes, about half of them. Three times
over-exposure leaves the older camera a third of its kana lanes and none of its turns; a gradient as strong as the lit level
leaves it two thirds of them and a quarter of its turns. Banding is the bands' weak spot: 50 % deep at a 30 px period it
leaves the older camera 30 % of its kana lanes but 65 % of its turns, and at an 80 px period it takes about half or more of
every kind of lane on every camera. Hand shake is where the bands do most: 2 and 4 px of motion blur leave the sharp camera
all or almost all of its kana lanes against a third and a quarter of its turns, and the older camera three in four and two
in three against none; at 9 px both polarity lanes still read in almost every frame. With one corner outside the frame the
card is turned by about 45° in these poses, and one band is out of view.

End to end (`stream_sim`: 30 fps camera, 1/60 s exposure with blending/tearing, the phone held upright, 8 trials per row,
received through a scan session that tracks the pose; seconds to complete, mean) for the largest KAGEMUSHA Payment (12,182
bytes) and, after the slash, a Request-sized message (2,132 bytes). The slower cameras deliver only that many frames a second
to a decoder that reads each of them whatever it reads (a slow decoder's own cost is modelled below); the rolling shutters
expose the rows one after another over the readout time, and upright the rows run along the card, so a frame change sweeps
across its width. Rows with three figures list the sharp, older and soft 720p cameras:

| Camera | 8 fps | 10 fps | 12 fps | 15 fps |
| --- | --- | --- | --- | --- |
| sharp 720p | 1.78 / 0.25 s | 1.40 / 0.20 s | 1.20 / 0.18 s | 0.94 / 0.15 s |
| older 720p | 1.81 / 0.27 s | 1.47 / 0.22 s | 1.30 / 0.19 s | 1.02 / 0.15 s |
| soft 720p | 2.64 / 0.40 s | 2.05 / 0.29 s | 1.76 / 0.26 s | 1.32 / 0.19 s |
| harshest 480p preset | 4.55 / 0.75 s | 3.74 / 0.62 s | 3.20 / 0.52 s | 2.55 / 0.42 s |
| 720p, a camera giving the decoder only 20 frames a second | 1.78 / 1.95 / 2.58 s | 1.46 / 1.56 / 2.05 s | 1.36 / 1.46 / 1.74 s | 1.23 / 1.24 / 1.40 s |
| 720p, a camera giving the decoder only 15 frames a second | 1.87 / 1.97 / 2.55 s | 1.67 / 1.73 / 2.02 s | 1.58 / 1.62 / 1.75 s | 1.33 / 1.39 / 1.33 s |
| 720p, a camera giving the decoder only 10 frames a second | 2.34 / 2.45 / 2.61 s | 2.00 / 2.05 / 2.00 s | 2.12 / 2.19 / 2.26 s | 2.02 / 2.09 / 2.01 s |
| 720p, rolling shutter, 20 ms readout, 8 ms exposure | 1.82 / 1.83 / 2.73 s | not run | 1.25 / 1.32 / 1.85 s | 0.96 / 1.02 / 1.33 s |
| 720p, rolling shutter, 30 ms readout, 16 ms exposure | 1.85 / 1.89 / 2.80 s | not run | 1.28 / 1.32 / 1.89 s | 1.03 / 1.17 / 1.40 s |
| 720p held at any angle | not run | not run | 1.37 / 1.57 / 2.34 s | not run |
| format 3, the three 720p cameras at any angle | 2.46 / 2.73 / 4.46 s | 1.99 / 2.39 / 3.57 s | 1.65 / 1.87 / 2.98 s | 1.32 / 1.53 / 2.38 s |
| format 3, the three 720p cameras, Request | 0.33 / 0.40 / 0.71 s | 0.26 / 0.34 / 0.56 s | 0.23 / 0.29 / 0.48 s | 0.18 / 0.22 / 0.38 s |
| format 3, harshest 480p preset | 6.32 / 1.08 s | 5.70 / 1.04 s | 5.76 / 1.02 s | 6.31 / 1.29 s |
| animated QR (Apple Vision), 720p cameras | 6.4 / 1.2 s | 5.1 s | 4.3 s | 3.4 s |

Every trial completed and none delivered a wrong payload. At the default 12 fps a Payment takes 1.20 s on a sharp camera,
1.30 s on an older one and 1.76 s on a soft one, against 1.65, 1.87 and 2.98 s for format 3 and 6.4 s for the apps' animated
QR at its 8 fps; a Request takes 0.18–0.26 s. The three 720p cameras read 44–46 kana lanes a second of the 48 that 12 fps
shows; the harshest 480p preset reads 12 and completes in 3.20 s. The bands keep their lead under both rolling shutters
(1.25–1.28 s on the sharp camera, 1.85–1.89 s on the soft one and 3.22–3.27 s on the harshest preset at 12 fps). Held at any
angle, the harshest preset took 5.20 s on average and 3.37 s in the median, against 5.76 s on average for format 3.

A camera that gives the decoder fewer frames a second costs the turn lanes most. The scan session reads a shown frame's
bands on its first capture and its turn lanes only on a later one (§7 step 10), and at 15 camera frames a second most frames
shown at 12 fps are captured once, so the sharp and older cameras complete in 1.58 and 1.62 s at 12 fps, where a session that
read both on every capture took 1.20 and 1.25 s. That model charges a capture nothing for what it reads. On a phone a capture
that reads both costs the time that makes its decoder slow, and there the bands first are the faster schedule (below).

The QR row is the same cameras and timeline watching the apps' animated QR stream: `IRQR` frames as ZXing draws them at ECC M,
version 13, 256-byte shards with Cauchy Reed–Solomon parity, the QR box 1.25 times the petal canvas, carrying the message body
without its 84-byte header (12,098 and 2,048 bytes), read by Apple Vision's barcode detector (`VNDetectBarcodesRequest`; modern
and legacy cameras, 6 trials per rate above 8 fps; the soft camera matches at 8 fps). On the harshest 480p preset one QR trial
in eight completed within 90 s.

Paired trials measure what the bands add: the same trials (payloads, poses, display phases and noise) with the cards' bands
shown and blacked out (`stream_sim --hide-bands`, what the square alone yields, as format 3 does). Each cell gives the mean
seconds to complete a Payment at 12 fps with the bands / with them hidden, then how many trials the bands made faster,
left the same or made slower:

| Camera | Upright (24 trials) | Upright, rolling shutter 30 ms, 16 ms exposure (16) | Any angle (16) |
| --- | --- | --- | --- |
| sharp 720p | 1.18 / 1.66 s, 24–0–0 | 1.27 / 1.67 s, 16–0–0 | 1.38 / 1.66 s, 11–5–0 |
| older 720p | 1.28 / 1.77 s, 24–0–0 | 1.32 / 1.75 s, 16–0–0 | 1.63 / 2.03 s, 12–4–0 |
| soft 720p | 1.75 / 2.98 s, 24–0–0 | 1.89 / 2.99 s, 16–0–0 | 2.37 / 2.98 s, 12–4–0 |
| harshest 480p preset | 3.30 / 3.47 s, 23–1–0 | 3.55 / 3.70 s, 13–3–0 | 11.7 / 13.5 s (median 4.07 both), 11–5–0 |

No trial was slower with the bands, and none delivered a wrong payload. Held upright, the bands cut a Payment's transfer by
28–29 % on the sharp and older cameras (about 0.5 s) and by 41 % on the soft one (1.2 s), under the 30 ms rolling shutter by
24–37 %, and on the harshest 480p preset by 4–5 %. At any angle they cut it by about a fifth, and a quarter to a third of the
trials, whose bands the frame cut off, took the same time.

Decode cost (`decode_cost` example; one thread of an Apple M4 Max, the fastest of 18 runs while other work kept the machine
busy; upright captures, kana lanes of both bands):

| Capture | Full search | Tracked | Turn lanes | Kana lanes | Merge of 3 captures |
| --- | --- | --- | --- | --- | --- |
| modern 720p | 5.1 ms | 0.5 ms | 4.7 ms | 2.0 ms | 0.6 ms |
| legacy 720p, barrel distortion | 4.9 ms | 0.5 ms | 5.0 ms | 2.1 ms | 0.7 ms |
| modern 1080p | 11.9 ms | 0.8 ms | 4.9 ms | 2.2 ms | 0.7 ms |
| 480p, blur 1.2 px | 2.1 ms | 0.4 ms | 3.5 ms | 1.9 ms | 0.6 ms |
| modern 720p, a thumb over a blossom | 15.3 ms | 2.5 ms | 4.8 ms | 2.0 ms | 0.7 ms |

Reading the turn lanes costs about ten times a tracked frame's decode, and the kana lanes of both bands about four times. A
scan session therefore tries each turn lane and each band once per shown frame, reads the bands first, and backs off on a
camera whose turns or bands it cannot read (§7 steps 9 and 10). On a desktop the Kotlin and Swift ports read a capture's turn
lanes in 5–9 ms, and the Swift port its bands in 1–4 ms; ports are expected to stay within a factor of 20 of the reference.

On a phone, format 3 was measured on a Pixel 6 held to its four Cortex-A55 cores at 1.8 GHz (`taskset 0f`; the core type that
budget 4 GB phones pair with a few faster ones), running the Kotlin SDK on ART, which decoded the camera frames of simulated
trials (12,182 bytes, one pose) through a scan session that, as CameraX's analyser does, takes only the newest camera frame
whenever it is free. A capture cost about 16 ms, and about 80 ms when it tried turn lanes. The Payment completed in 1.82 s on
the sharp camera (13–15 camera frames a second decoded), 2.50 s on the older one (13 a second) and 3.08 s on the soft one
(26 a second, the back-off passing most of its tries over; 3.36 s without it). The frames came from the simulator: no camera
or display was in this loop.

Format 5 was measured the same way, on one upright trial of each camera (three replays each). On those cores a tracked
capture costs about 14 ms, a full search about 100 ms, the turn lanes 75 ms and the kana lanes of both bands 35–37 ms with the
reader of §7 step 10, whose first version took 75–83 ms. A session that tried the turn lanes and the bands on the same capture
spent about 165 ms on it, twice the 83 ms a frame is shown at 12 fps: it decoded only 6–9 camera frames a second, missed shown
frames, and completed the Payment in 2.51, 2.61 and 2.65 s on the sharp, older and soft cameras, slower than format 3 on the
sharp one. With the bands first (§7 step 10) the session decodes 14–15 camera frames a second on the sharp and older cameras
and 18–20 on the soft one, and the Payment completes in 1.67–1.87 s on the sharp camera, 1.90–2.07 s on the older one and
1.79–1.94 s on the soft one, against 1.82, 2.50 and 3.08 s with format 3. A decoder that keeps up with every camera frame
completes the same trials in 1.17, 1.27 and 1.73 s.

The simulator models those cores (`stream_sim --phone a55`: a full search 105 ms, a tracked capture 14.5 ms, the turn lanes
75 ms, a band 15 ms and a merge 10 ms, the newest camera frame taken whenever the decoder is free; 24 upright trials of each
camera). The Payment takes 1.74, 1.83, 1.90 and 3.94 s on the sharp, older, soft and harshest cameras with the bands first;
2.03, 2.13, 2.00 and 3.92 s when every capture reads both turns and bands; 2.04, 2.19, 3.09 and 3.84 s with format 3's session,
which reads no band; and 2.25, 2.39, 3.33 and 4.15 s with the bands blacked out, the band tries that read nothing costing their
time until the back-off passes them over.

Run the evidence yourself:

```
cargo run --release -p iroha_petal --example qualify -- 40            # per-frame matrix
cargo run --release -p iroha_petal --example stress -- 24             # light, motion and hidden-corner matrix
cargo run --release -p iroha_petal --example stream_sim -- 8 12182 12 # end to end at the default 12 fps, phone upright
cargo run --release -p iroha_petal --example stream_sim -- 24 12182 12 --hide-bands   # the same trials, square alone
cargo run --release -p iroha_petal --example stream_sim -- 8 12182 12 modern,legacy,soft --camera-fps 10
cargo run --release -p iroha_petal --example stream_sim -- 8 12182 12 --readout-ms 30 --exposure-ms 16
cargo run --release -p iroha_petal --example stream_sim -- 8 12182 12 --any-rotation
cargo run --release -p iroha_petal --example stream_sim -- 24 12182 12 --phone a55   # a slow phone core's decode costs
cargo run --release -p iroha_petal --example decode_cost              # decode time, turn and kana lanes included
iroha offline petal simulate --camera legacy --bytes 12182 --fps 12   # end to end, through the CLI
```

## 9. CLI

```
iroha offline petal encode   --input payload.bin --output out/ --kind 3 [--frames N] [--size 1024] [--fps 12] [--format png|gif]
iroha offline petal decode   --input-dir out/ --output payload.out
iroha offline petal inspect  --input out/frame_0000.png
iroha offline petal simulate --camera modern|legacy|soft|worst --bytes 12182 --fps 12 --trials 4
```

`encode` writes cards `--size` pixels wide (1024 × 1668 at the default) as `frame_0000.png …` (or one looping `stream.gif`
with the `offline-visual-codecs` feature) and `manifest.json` (`iroha.offline.petal.encode.v5`). `decode` reads PNG frames in
file-name order from a directory — frames extracted from a screen recording work — and reports the turn and kana lanes it
read. `inspect` prints which lanes, turn lanes and kana lanes decode, the blur the turn reader fits, the orientation, the
lens distortion and the beacon of one frame. `simulate` holds the receiving phone upright (§8).

## 10. Fixtures and conformance

* `fixtures/petal/petal_stream_v5.json` — constants, layout tables (finders, band slots, every sector's cell order and
  carrier order, the `天` mask and the light cells, rings, data, gate and guard slots, every katakana tile as
  `[band, row, column, x, y, pilot]` with −1 for a data tile, the pilots' symbols), the sixteen glyphs' characters and stroke
  polylines, palette, CRC-32C, xorshift32/`mix32`, whitening (lanes `D`, `F0`–`F3`, `T0`–`T3`, `P0`, `K0`, `P1`, `K1`),
  Reed–Solomon vectors, fountain masks, the atom-id schedules of the field, turn and kana lanes, probes of the turn
  reader's templates (`turn_reader`: every blur at six points) and of the kana reader's templates (`kana_reader`: every
  blur at six grid points), and for three streams (`tiny`, `one-pass`, `wrap`) the lane data and transmitted words of every
  lane (turn and kana lanes included, the kana lanes in the order `P0`, `K0`, `P1`, `K1`), the lit dots, the lit petals, the
  turn position of every cell (`turns`, one digit per cell), and every tile's glyph (`kana_glyphs`, one hex digit per tile)
  and polarity (`kana_light_hex`, one bit per tile) of every listed frame. Encoders must reproduce all of it bit for bit,
  and a port's templates must match the probes to 10⁻¹².
* `fixtures/petal/petal_captures_v5.json` — sixteen degraded camera luma planes (zlib + base64, row-major 8-bit) of cards
  of the `one-pass` stream with `must_decode` lanes, the lanes the reference decoder reads (`reference_decoded`),
  `must_turn` and `reference_turned` for the turn lanes (§7 step 9, read with no sector passed over), `must_kana` and
  `reference_kana` for the kana lanes (§7 step 10, both bands read, under the header the capture's lanes name), the expected
  lane data (`turn_data` for the turn lanes, `kana_data` for the kana lanes in the order `P0`, `K0`, `P1`, `K1`), a
  `mirrored` flag and the `inferred_corner` (null, or the canonical index of a hidden corner blossom); two tracking pairs
  (`tracks`: a first frame to decode and a second frame to read by tracking its pose, with `must_track`,
  `reference_tracked`, `must_turn`, `reference_turned`, `reference_kana` and `inferred_corner`); one merge set (`merges`:
  four 480p captures of one shown frame under one pose, each with its own sensor noise, with per-capture `must_decode` and
  `reference_decoded`, `must_merge` for the field lanes the merge of §7 step 8 must recover although no capture reads them
  alone, and `reference_merged`); one mixed set (`mixed_merges`: four captures whose rings show `frame` and whose petal
  field shows `petals_frame`, with `reference_merged` for what the merge of their field evidence reads); and two negatives.
  Lanes are named by letters: `D` and the sector digits `0`–`3` (a turn lane by the digit of its field lane), and the kana
  lanes by `P0`, `K0`, `P1`, `K1`. A decoder must read the required lanes, must never return a lane with wrong data, must
  reject the negatives, and conforming ports reproduce `reference_decoded`, `reference_turned`, `reference_kana`,
  `reference_tracked`, `reference_merged` and the inferred corners exactly. A scan session fed the captures of a merge set
  in order (30 fps) must recover its `must_merge` lanes, and fed a mixed set it must merge but never offer a lane, since the
  merged lanes name `petals_frame`. The captures cover a sharp and a rotated camera, an older soft camera with barrel
  distortion, two blurred 480p cameras, a mirrored front camera, over-exposure, veiling light, a shadow band that only the
  local read gets through, a covered blossom and a corner outside the frame (both inferred), a cheap lens whose barrel
  distortion the decoder must search for, and four phones held upright (a sharp, an older and a soft 720p camera and a
  480p one), which see both bands: the sharp, older and soft ones read all four kana lanes and the 480p one `P0`, `P1` and
  `K1`. The clean card and the veiled capture read all four kana lanes too; the tilted captures read the band in view.
* Regenerate with `cargo run -p iroha_petal --example gen_fixtures -- fixtures/petal/petal_stream_v5.json` and
  `cargo run --release -p iroha_petal --example gen_captures -- fixtures/petal/petal_captures_v5.json`; `cargo test -p iroha_petal`
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
  accept costs at most an integrity reset, never a wrong payload. A turn lane has the field lanes' 22 parity bytes on a shorter
  word and must also match its 32-bit check, which binds it to the stream tag, frame and sector of the field lane it was read
  on, so a misread turn lane (or one read under a wrong field lane) passes with probability about 2⁻³² on top of the
  Reed–Solomon odds. A kana lane must match its 32-bit check under one of three frames, which binds it to the stream tag, a
  frame and the lane: a misread passes with probability about 3 · 2⁻³² on top of the Reed–Solomon odds.
* The payload CRC-32C and the Reed–Solomon codes detect corruption, not forgery. A screen in front of the camera can always show
  garbage or a different stream; the application must authenticate the payload (KAGEMUSHA messages carry their own proofs and
  signatures) and must not treat a completed Petal stream as authentic.
* A visible stream is readable by anyone who films it. Encrypt payloads that must stay private before streaming them.
* A stream that never completes (jamming) only wastes the user's time; sessions time out.

## 12. Open work

* TODO: read the turn lanes on the first capture of a shown frame when the decoder has the time. The scan session reads
  a shown frame's bands first and its turns on a later capture (§7 step 10), which a slow phone core needs; a fast decoder
  whose camera gives it few frames a second (dim light) then reads most frames' turns not at all (§8: 1.58 s instead of
  1.20 s at 15 camera frames a second). A session that knew its spare time per capture could read both there.
* TODO: physical-device qualification of the matrices in §8, including the OLED/PWM and rolling-shutter effects, the
  exposure-compensation guidance, and the 12 fps default with a real camera on an older 4 GB Android phone.
* TODO: check the katakana bands with real cameras and screens: how people frame the card (the square or the whole card),
  the bands' exposure on OLED and LCD screens, and banding, the bands' weak spot in simulation (§8).
* TODO: check the turn lanes with real cameras and screens. The simulator has no denoising, display subpixels or video
  compression, and a phone's noise reduction and sharpening may smear or ring the petal outlines the turn reader matches;
  hand shake of 2 px already costs two thirds of them in simulation. A camera that cannot read turns still reads the field
  and the bands, as the soft camera of §8 does, with the scan session's back-off limiting the cost of failed tries.
