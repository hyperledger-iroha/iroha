# Copyright 2026 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

"""The kana layer: data in the katakana tiles of the bands above and below the square.

The card (:data:`~iroha_petal.layout.CARD_HEIGHT`) is the square code with a band
of katakana tiles above and below it, in the part of an upright phone's camera
frame that the square leaves unused. Each band holds 306 tiles
(:func:`~iroha_petal.layout.kana_tiles`); eight are pilots with fixed symbols
(:data:`~iroha_petal.layout.KANA_PILOT_SYMBOLS`) and the other 298 carry two
lanes:

==================  ==========================================  ========  =====  ======
lane                cells                                       codeword  data   parity
==================  ==========================================  ========  =====  ======
P0, P1 (polarity)   296 data tiles x 1 bit (light or dark)      37 B      20 B   17 B
K0, K1 (glyph)      298 data tiles x 4 bits (one of sixteen)    149 B     116 B  33 B
==================  ==========================================  ========  =====  ======

A light tile is filled light with its glyph in ink; a dark tile is black with its
glyph in pink. Data tile ``i`` of a band (its tiles that are not pilots, in
reading order) shows bit ``i`` of the band's polarity codeword (most significant
bit first; the last two data tiles, beyond the codeword, are light) and nibble
``i`` of its glyph codeword (the high nibble of byte ``i // 2`` for even ``i``).

Every kana lane is one Reed-Solomon codeword, XOR-whitened: fountain atoms from
their own repair range (:data:`KANA_ID_BASE`, see :func:`kana_first_id`) and a
CRC-32C check that binds them to the stream tag, the frame and the lane
(:func:`kana_check`). The lanes have no header: a reader takes the frame from the
square's lanes and also tries the neighbouring frames, because a rolling shutter
or a stale frame can show a band from the frame before or after the square.

:func:`read_kana` reads the bands of a decoded capture. The square's pose,
extended beyond the square, places every tile; each band's pilots are found by a
small search around their places, and an affine correction fitted to their
offsets moves every tile of the band. The pilots also pick the blur of the glyph
templates and the levels that tell light tiles from dark ones. Each data tile
then takes the polarity its mean level shows and the glyph whose template
correlates best with its samples.

Every floating-point expression follows the reference operation by operation in
IEEE double precision, including the order of every sum (sums run through
:func:`functools.reduce` or explicit loops: Python's built-in ``sum`` of floats
is compensated from 3.12 on). The reader's templates are built once, on first
use.
"""

from __future__ import annotations

import enum
import math
import threading
from dataclasses import dataclass
from functools import reduce
from operator import add, mul
from typing import List, Optional, Sequence, Tuple

from ._numeric import F64_MAX, f64_max, f64_min, total_key
from .crc import crc32c
from .decode import DecodedFrame, LaneResult
from .glyphs import GLYPH_COUNT, GLYPH_GRID, inked_flags
from .image import Luma, Pose
from .lanes import ATOM_LEN, KanaSymbol
from .layout import (
    CANVAS,
    KANA_BANDS,
    KANA_GLYPH_BOX,
    KANA_PILOT_SYMBOLS,
    KANA_PILOTS,
    KANA_TILE,
    kana_data_tiles,
    kana_inside,
    kana_pilot_tiles,
    kana_tiles,
)
from .prng import Xorshift32
from .rs import ReedSolomon, RsError, RsErrorKind
from .stream import AtomPacket, LaneHeader
from .turns import TURN_ID_BASE

__all__ = [
    "KANA_DATA_TILES",
    "KANA_GLYPH_WORD",
    "KANA_GLYPH_PARITY",
    "KANA_GLYPH_ATOMS",
    "KANA_POLARITY_WORD",
    "KANA_POLARITY_PARITY",
    "KANA_POLARITY_ATOMS",
    "KANA_CHECK_LEN",
    "KANA_ID_BASE",
    "KANA_TILE_COUNT",
    "KANA_BLURS",
    "KANA_GRID",
    "KANA_COARSE_GRID",
    "KanaKind",
    "KanaLane",
    "KANA_LANES",
    "KanaSymbol",
    "kana_first_id",
    "kana_check",
    "kana_data",
    "kana_packet",
    "encode_kana_lane",
    "decode_kana_lane_counted",
    "pilot_symbols",
    "set_kana_symbols",
    "polarity_word",
    "glyph_word",
    "template_probe",
    "coarse_template_probe",
    "TileReading",
    "BandReading",
    "observe_band",
    "KanaResult",
    "read_kana",
]

#: Data tiles of each band.
KANA_DATA_TILES = 298
#: Codeword length of a glyph lane in bytes (two data tiles per byte).
KANA_GLYPH_WORD = KANA_DATA_TILES // 2
#: Parity bytes of a glyph lane.
KANA_GLYPH_PARITY = 33
#: Atoms carried by each glyph lane.
KANA_GLYPH_ATOMS = 7
#: Codeword length of a polarity lane in bytes (eight data tiles per byte).
KANA_POLARITY_WORD = KANA_DATA_TILES // 8
#: Parity bytes of a polarity lane.
KANA_POLARITY_PARITY = 17
#: Atoms carried by each polarity lane.
KANA_POLARITY_ATOMS = 1
#: Bytes of the check that closes a kana lane's data.
KANA_CHECK_LEN = 4
#: Fountain id of the first kana atom of frame 0, just above the turn lanes' range
#: (``2^31 + 16 * 65536``).
KANA_ID_BASE = TURN_ID_BASE + (1 << 20)
#: Kana atom ids reserved per frame (eight per band).
_KANA_IDS_PER_FRAME = 16
#: Tiles of both bands.
KANA_TILE_COUNT = 612

assert KANA_GLYPH_WORD - KANA_GLYPH_PARITY == KANA_GLYPH_ATOMS * ATOM_LEN + KANA_CHECK_LEN
assert KANA_POLARITY_WORD - KANA_POLARITY_PARITY == KANA_POLARITY_ATOMS * ATOM_LEN + KANA_CHECK_LEN
assert KANA_GLYPH_ATOMS + KANA_POLARITY_ATOMS <= _KANA_IDS_PER_FRAME // KANA_BANDS
assert KANA_TILE_COUNT == KANA_BANDS * (KANA_DATA_TILES + KANA_PILOTS) == len(kana_tiles())


class KanaKind(enum.Enum):
    """What a kana lane carries."""

    #: Light or dark tiles.
    POLARITY = "polarity"
    #: The tiles' katakana.
    GLYPH = "glyph"


class KanaLane(enum.Enum):
    """One of the four kana lanes: the polarity lane (``P0``, ``P1``) and the glyph lane
    (``K0``, ``K1``) of the band above the square (0) and of the band below it (1).
    Iteration yields them in the order ``P0``, ``K0``, ``P1``, ``K1``."""

    #: The light or dark tiles of the band above the square.
    P0 = "P0"
    #: The katakana of the band above the square.
    K0 = "K0"
    #: The light or dark tiles of the band below the square.
    P1 = "P1"
    #: The katakana of the band below the square.
    K1 = "K1"

    @classmethod
    def of(cls, band: int, kind: KanaKind) -> "KanaLane":
        """The lane of ``band`` (``0..2``) that carries ``kind``."""
        if not 0 <= band < KANA_BANDS:
            raise ValueError("band out of range")
        return KANA_LANES[2 * band + (kind is KanaKind.GLYPH)]

    @property
    def band(self) -> int:
        """Band: 0 above the square, 1 below it."""
        return int(self.value[1])

    @property
    def kind(self) -> KanaKind:
        """What the lane carries."""
        return KanaKind.GLYPH if self.value[0] == "K" else KanaKind.POLARITY

    @property
    def word_len(self) -> int:
        """Codeword length in bytes."""
        return KANA_GLYPH_WORD if self.kind is KanaKind.GLYPH else KANA_POLARITY_WORD

    @property
    def parity_len(self) -> int:
        """Parity bytes."""
        return KANA_GLYPH_PARITY if self.kind is KanaKind.GLYPH else KANA_POLARITY_PARITY

    @property
    def data_len(self) -> int:
        """Data bytes: the atoms, then their check."""
        return self.word_len - self.parity_len

    @property
    def atoms(self) -> int:
        """Atoms carried."""
        return KANA_GLYPH_ATOMS if self.kind is KanaKind.GLYPH else KANA_POLARITY_ATOMS

    @property
    def code(self) -> int:
        """The byte that names the lane in its check: ``0x10 | band << 1 | kind`` (polarity
        0, glyph 1)."""
        return 0x10 | self.band << 1 | (1 if self.kind is KanaKind.GLYPH else 0)

    def whitening(self) -> bytes:
        """The whitening sequence: one byte per codeword byte from a
        :class:`~iroha_petal.prng.Xorshift32` seeded "POL0", "POL1" for polarity, "KAN0",
        "KAN1" for glyphs (``0x504F4C30 + band``, ``0x4B414E30 + band``)."""
        return _WHITENING[self]


#: The four lanes: ``P0``, ``K0``, ``P1``, ``K1``.
KANA_LANES = (KanaLane.P0, KanaLane.K0, KanaLane.P1, KanaLane.K1)


def _whitening(lane: KanaLane) -> bytes:
    base = 0x4B414E30 if lane.kind is KanaKind.GLYPH else 0x504F4C30
    rng = Xorshift32(base + lane.band)
    return bytes(rng.next_byte() for _ in range(lane.word_len))


_WHITENING = {lane: _whitening(lane) for lane in KanaLane}
_CODES = {
    KANA_POLARITY_PARITY: ReedSolomon(KANA_POLARITY_PARITY),
    KANA_GLYPH_PARITY: ReedSolomon(KANA_GLYPH_PARITY),
}


def kana_first_id(frame: int, lane: KanaLane) -> int:
    """Fountain id of the first atom ``lane`` carries in ``frame``: ``KANA_ID_BASE + 16 *
    frame + 8 * band``, plus 7 for the polarity lane (the glyph lane's seven atoms come
    first)."""
    if not 0 <= frame <= 0xFFFF:
        raise ValueError("frame counters are 16-bit unsigned integers")
    slot = 0 if lane.kind is KanaKind.GLYPH else KANA_GLYPH_ATOMS
    return KANA_ID_BASE + frame * _KANA_IDS_PER_FRAME + lane.band * 8 + slot


def kana_check(header: LaneHeader, lane: KanaLane, atoms: bytes) -> int:
    """The check of a kana lane: CRC-32C over the stream tag, the frame (big-endian), the
    lane's :attr:`KanaLane.code` and the atoms."""
    frame = header.frame
    return crc32c(bytes((header.tag, frame >> 8, frame & 0xFF, lane.code)) + bytes(atoms))


def kana_data(header: LaneHeader, lane: KanaLane, atoms: bytes) -> bytes:
    """The data of a kana lane: the atoms followed by their :func:`kana_check`
    (big-endian).

    Raises :class:`ValueError` when ``atoms`` is not the lane's atom count long.
    """
    atoms = bytes(atoms)
    if len(atoms) != lane.atoms * ATOM_LEN:
        raise ValueError("kana atoms length mismatch")
    return atoms + kana_check(header, lane, atoms).to_bytes(KANA_CHECK_LEN, "big")


def kana_packet(header: LaneHeader, lane: KanaLane, data: bytes) -> Optional[AtomPacket]:
    """The atoms of kana lane data whose check matches ``header`` and ``lane``; ``None`` for
    data of the wrong length or a check that does not match."""
    if len(data) != lane.data_len:
        return None
    count = lane.atoms * ATOM_LEN
    atoms = bytes(data[:count])
    if kana_check(header, lane, atoms).to_bytes(KANA_CHECK_LEN, "big") != bytes(data[count:]):
        return None
    return AtomPacket(
        header=header,
        first_id=kana_first_id(header.frame, lane),
        atoms=tuple(atoms[i * ATOM_LEN : (i + 1) * ATOM_LEN] for i in range(lane.atoms)),
    )


def encode_kana_lane(lane: KanaLane, data: bytes) -> bytes:
    """Encode kana lane data into the transmitted (whitened) codeword.

    Raises :class:`ValueError` when ``data`` is not exactly :attr:`KanaLane.data_len` bytes.
    """
    if len(data) != lane.data_len:
        raise ValueError("kana data length mismatch")
    word = _CODES[lane.parity_len].encode(bytes(data))
    return bytes(a ^ b for a, b in zip(word, _WHITENING[lane]))


def decode_kana_lane_counted(
    lane: KanaLane, transmitted: bytes, erasures: Sequence[int] = ()
) -> Tuple[bytes, int]:
    """Decode a transmitted kana codeword, returning its data and how many byte positions
    the Reed-Solomon decoder rewrote. The check is not verified here (see
    :func:`kana_packet`).

    Raises :class:`~iroha_petal.rs.RsError` when the word is uncorrectable.
    """
    if len(transmitted) != lane.word_len:
        raise RsError(RsErrorKind.INVALID_SHAPE)
    word = bytearray(a ^ b for a, b in zip(transmitted, _WHITENING[lane]))
    corrected = _CODES[lane.parity_len].decode(word, erasures)
    return bytes(word[: lane.data_len]), corrected


def _check_band(band: int) -> int:
    if not 0 <= band < KANA_BANDS:
        raise IndexError("band index out of range")
    return band


def pilot_symbols() -> List[KanaSymbol]:
    """The symbols of every tile (pilots fixed, data tiles light with glyph 0)."""
    symbols = [KanaSymbol(0, True)] * KANA_TILE_COUNT
    for index, tile in enumerate(kana_tiles()):
        if tile.pilot is not None:
            glyph, light = KANA_PILOT_SYMBOLS[tile.pilot]
            symbols[index] = KanaSymbol(glyph, light)
    return symbols


def set_kana_symbols(symbols: List[KanaSymbol], band: int, polarity: bytes, glyphs: bytes) -> None:
    """Show the transmitted codewords of ``band`` on its data tiles of ``symbols`` (one
    entry per tile, changed in place): ``polarity`` (37 bytes) and ``glyphs`` (149 bytes).

    Raises :class:`ValueError` when a codeword has the wrong length.
    """
    if len(polarity) != KANA_POLARITY_WORD or len(glyphs) != KANA_GLYPH_WORD:
        raise ValueError("kana codeword length mismatch")
    if len(symbols) != KANA_TILE_COUNT:
        raise ValueError("a frame has exactly 612 katakana tiles")
    bits = KANA_POLARITY_WORD * 8
    for i, tile in enumerate(kana_data_tiles(_check_band(band))):
        light = (polarity[i >> 3] >> (7 - (i & 7))) & 1 == 1 if i < bits else True
        byte = glyphs[i >> 1]
        symbols[tile] = KanaSymbol(byte >> 4 if i % 2 == 0 else byte & 0x0F, light)


def polarity_word(symbols: Sequence[KanaSymbol], band: int) -> bytes:
    """Pack the polarity codeword ``band``'s data tiles show."""
    word = bytearray(KANA_POLARITY_WORD)
    for i, tile in enumerate(kana_data_tiles(_check_band(band))[: KANA_POLARITY_WORD * 8]):
        if symbols[tile].light:
            word[i >> 3] |= 1 << (7 - (i & 7))
    return bytes(word)


def glyph_word(symbols: Sequence[KanaSymbol], band: int) -> bytes:
    """Pack the glyph codeword ``band``'s data tiles show."""
    word = bytearray(KANA_GLYPH_WORD)
    for i, tile in enumerate(kana_data_tiles(_check_band(band))):
        nibble = symbols[tile].glyph & 0x0F
        word[i >> 1] |= nibble << 4 if i % 2 == 0 else nibble
    return bytes(word)


# ---- reading ------------------------------------------------------------------------------

#: Gaussian blurs (sigma in canvas units) of the glyph templates; the pilots pick one per band.
KANA_BLURS = (1.0, 1.6, 2.4, 3.4, 4.8)
#: The blur of the template the pilots are first searched with (3.4 units): coarse enough to
#: find a pilot on a 2-unit grid under any blur.
_COARSE_BLUR = 3
#: Step of the coarse pilot search in canvas units.
_COARSE_STEP = 2.0
#: Samples per tile side: points ``(i + 1/2) * 25 / 12`` units from the tile's top-left corner.
KANA_GRID = 12
#: Samples per tile side in the coarse pilot search: points ``(i + 1/2) * 25 / 6``.
KANA_COARSE_GRID = 6
#: Template raster step in canvas units.
_RASTER_STEP = 0.5
#: The raster starts this far outside the tile square, so blur reaches in from the gutter.
_RASTER_MARGIN = 4.0
#: Raster side in steps.
_RASTER_SIDE = 67
#: A pilot whose coarse correlation is below this is not followed further.
_PILOT_COARSE_MIN = 0.35
#: A pilot whose final correlation is below this is not used.
_PILOT_MIN = 0.5

_POINTS = KANA_GRID * KANA_GRID
_COARSE_POINTS = KANA_COARSE_GRID * KANA_COARSE_GRID
_HALF_TILE = KANA_TILE / 2.0


def _grid_point(i: int, side: int) -> float:
    """Tile-local coordinate of grid index ``i`` on either axis of a grid of ``side``
    points."""
    return (i + 0.5) * KANA_TILE / side


#: The fine and coarse grids' offsets from the tile centre, on either axis.
_FINE_STEPS = tuple(_grid_point(i, KANA_GRID) - _HALF_TILE for i in range(KANA_GRID))
_COARSE_STEPS = tuple(
    _grid_point(i, KANA_COARSE_GRID) - _HALF_TILE for i in range(KANA_COARSE_GRID)
)

#: The raster's sub-points along either axis, in order: raster point ``a`` is at
#: ``-4 + a / 2`` units, its sub-points ``(m - 1.5) / 8`` units around it.
_SUB_POINTS = tuple(
    -_RASTER_MARGIN + a * _RASTER_STEP + (m - 1.5) / 8.0
    for a in range(_RASTER_SIDE)
    for m in range(4)
)


def _coverage(rows: Sequence[bytes]) -> List[float]:
    """The raster from the flags of its sub-points (one row of flags per sub-row): each
    value is the share of its 4 x 4 sub-points that are covered."""
    table: List[float] = []
    for b in range(_RASTER_SIDE):
        r0, r1, r2, r3 = rows[4 * b : 4 * b + 4]
        columns = list(map(add, map(add, r0, r1), map(add, r2, r3)))
        for k in range(0, 4 * _RASTER_SIDE, 4):
            hits = columns[k] + columns[k + 1] + columns[k + 2] + columns[k + 3]
            table.append(hits / 16.0)
    return table


def _inside_rows() -> List[bytes]:
    """The flags of every sub-row of the tile's inside raster."""
    return [bytes(kana_inside(u, v) for u in _SUB_POINTS) for v in _SUB_POINTS]


def _ink_rows(glyph: int, inside: Sequence[bytes]) -> List[bytes]:
    """The flags of every sub-row of ``glyph``'s ink raster: ``kana_inked`` at every
    sub-point, the strokes classified a row at a time (:func:`~iroha_petal.glyphs.inked_flags`)
    and cut to the rounded tile."""
    inset = (KANA_TILE - KANA_GLYPH_BOX) / 2.0
    scale = GLYPH_GRID / KANA_GLYPH_BOX
    xs = [(u - inset) * scale for u in _SUB_POINTS]
    rows = []
    for v, inside_row in zip(_SUB_POINTS, inside):
        flags = inked_flags(glyph, (v - inset) * scale, xs)
        rows.append(bytes(map(mul, flags, inside_row)))
    return rows


def _blur_raster(table: Sequence[float], sigma: float) -> List[float]:
    """Separable Gaussian blur of a raster: taps every :data:`_RASTER_STEP` out to
    ``ceil(3 sigma / step)``, weights ``exp(-d^2 / (2 sigma^2))`` normalised by their sum,
    points outside counting as zero; rows first. Every output sums its taps in order from
    zero, as the reference does."""
    side = _RASTER_SIDE
    reach = math.ceil(3.0 * sigma / _RASTER_STEP)
    weights = []
    for k in range(-reach, reach + 1):
        d = k * _RASTER_STEP
        weights.append(math.exp(-(d * d) / (2.0 * sigma * sigma)))
    total = reduce(add, weights, -0.0)
    weights = [w / total for w in weights]
    last_tap = 2 * reach

    def one_pass(source: Sequence[float], rows: bool) -> List[float]:
        out = [0.0] * (side * side)
        for b in range(side):
            for a in range(side):
                # the taps that land inside the raster, in tap order
                at = a if rows else b
                first = max(reach - at, 0)
                last = min(reach + side - 1 - at, last_tap)
                if rows:
                    start = b * side + a - reach + first
                    values = source[start : start + last - first + 1]
                else:
                    start = (b - reach + first) * side + a
                    values = source[start : start + (last - first) * side + 1 : side]
                out[b * side + a] = reduce(add, map(mul, weights[first : last + 1], values), 0.0)
        return out

    return one_pass(one_pass(table, True), False)


def _sample_raster(table: Sequence[float], side: int) -> List[float]:
    """A blurred raster at the ``side x side`` grid points (bilinear), mean-centred."""
    raster = _RASTER_SIDE
    out = []
    for j in range(side):
        fb = (_grid_point(j, side) + _RASTER_MARGIN) / _RASTER_STEP
        b0 = math.floor(fb)
        tb = fb - b0
        for i in range(side):
            fa = (_grid_point(i, side) + _RASTER_MARGIN) / _RASTER_STEP
            a0 = math.floor(fa)
            ta = fa - a0
            at = b0 * raster + a0
            top = table[at] * (1.0 - ta) + table[at + 1] * ta
            bottom = table[at + raster] * (1.0 - ta) + table[at + raster + 1] * ta
            out.append(top * (1.0 - tb) + bottom * tb)
    mean = reduce(add, out, -0.0) / (side * side)
    return [value - mean for value in out]


def _norm(values: Sequence[float]) -> float:
    """The length of a vector, at least ``1e-12``."""
    return f64_max(math.sqrt(reduce(add, map(mul, values, values), -0.0)), 1e-12)


def _scaled(values: Sequence[float], length: float) -> Tuple[float, ...]:
    """A vector scaled by ``1 / length``."""
    return tuple(value / length for value in values)


class _Templates:
    """The templates of one blur at the :data:`KANA_GRID` points, each mean-centred: the
    tile's inside, every glyph's ink (a dark tile's template), and every glyph's light-tile
    template (inside less ink), with the lengths of the dark and light templates and both as
    unit vectors."""

    __slots__ = ("inside", "ink", "light_norm", "dark_norm", "light", "dark")

    def __init__(self) -> None:
        self.inside: Tuple[float, ...] = ()
        self.ink: List[Tuple[float, ...]] = []
        self.light_norm: List[float] = []
        self.dark_norm: List[float] = []
        self.light: List[Tuple[float, ...]] = []
        self.dark: List[Tuple[float, ...]] = []


class _Tables:
    """Every blur's templates and the coarse templates of every glyph under
    :data:`_COARSE_BLUR` at the :data:`KANA_COARSE_GRID` points, mean-centred and scaled to
    unit length: light tiles (inside less ink) and dark tiles (ink)."""

    __slots__ = ("blurs", "coarse_light", "coarse_dark")

    def __init__(self) -> None:
        inside_rows = _inside_rows()
        inside = _coverage(inside_rows)
        inks = [_coverage(_ink_rows(glyph, inside_rows)) for glyph in range(GLYPH_COUNT)]
        self.coarse_light: List[Tuple[float, ...]] = []
        self.coarse_dark: List[Tuple[float, ...]] = []
        blurs = []
        for index, sigma in enumerate(KANA_BLURS):
            inside_blurred = _blur_raster(inside, sigma)
            inside_points = _sample_raster(inside_blurred, KANA_GRID)
            templates = _Templates()
            templates.inside = tuple(inside_points)
            for ink in inks:
                ink_blurred = _blur_raster(ink, sigma)
                ink_points = _sample_raster(ink_blurred, KANA_GRID)
                light = [a - b for a, b in zip(inside_points, ink_points)]
                light_norm = _norm(light)
                dark_norm = _norm(ink_points)
                templates.light_norm.append(light_norm)
                templates.dark_norm.append(dark_norm)
                templates.light.append(_scaled(light, light_norm))
                templates.dark.append(_scaled(ink_points, dark_norm))
                templates.ink.append(tuple(ink_points))
                if index == _COARSE_BLUR:
                    coarse_inside = _sample_raster(inside_blurred, KANA_COARSE_GRID)
                    coarse_ink = _sample_raster(ink_blurred, KANA_COARSE_GRID)
                    coarse = [a - b for a, b in zip(coarse_inside, coarse_ink)]
                    self.coarse_light.append(_scaled(coarse, _norm(coarse)))
                    self.coarse_dark.append(_scaled(coarse_ink, _norm(coarse_ink)))
            blurs.append(templates)
        self.blurs: Tuple[_Templates, ...] = tuple(blurs)


_lock = threading.Lock()
_tables_built: Optional[_Tables] = None


def _tables() -> _Tables:
    """The reader's templates, built on first use."""
    global _tables_built
    tables = _tables_built
    if tables is None:
        with _lock:
            if _tables_built is None:
                _tables_built = _Tables()
            tables = _tables_built
    return tables


def _check_probe(glyph: int, i: int, j: int, side: int) -> None:
    if not 0 <= glyph < GLYPH_COUNT:
        raise IndexError("glyph index out of range")
    if not (0 <= i < side and 0 <= j < side):
        raise IndexError("grid point out of range")


def template_probe(blur: int, glyph: int, light: bool, i: int, j: int) -> float:
    """The template value of ``glyph`` (light or dark tile) under blur ``blur`` (an index into
    :data:`KANA_BLURS`) at grid point ``(i, j)``, mean-centred and scaled to unit length:
    what conformance tests compare an SDK's tables with."""
    if not 0 <= blur < len(KANA_BLURS):
        raise IndexError("blur index out of range")
    _check_probe(glyph, i, j, KANA_GRID)
    templates = _tables().blurs[blur]
    table = templates.light if light else templates.dark
    return table[glyph][j * KANA_GRID + i]


def coarse_template_probe(glyph: int, light: bool, i: int, j: int) -> float:
    """The coarse template value of ``glyph`` (light or dark tile) at coarse grid point
    ``(i, j)``: what conformance tests compare an SDK's tables with."""
    _check_probe(glyph, i, j, KANA_COARSE_GRID)
    tables = _tables()
    table = tables.coarse_light if light else tables.coarse_dark
    return table[glyph][j * KANA_COARSE_GRID + i]


def _sample_tile(
    image: Luma,
    pose: Pose,
    x: float,
    y: float,
    shift_x: float,
    shift_y: float,
    steps: Sequence[float],
) -> Tuple[List[float], float, float]:
    """Samples the tile whose top-left corner is ``(x, y)`` (square coordinates), moved by
    the shift, at the grid points ``steps`` (offsets from the tile centre, rows ``b`` outer
    and columns ``a`` inner): the pose maps the tile's centre ``o`` and the points one unit
    to its right and below it (``o + u``, ``o + v``), and grid point ``(a, b)`` is sampled
    at ``(o + b v) + a u`` (bilinearly, clamped to the border, as
    :meth:`~iroha_petal.image.Luma.sample`). Returns the samples minus their mean, that mean
    and their energy (the sum of their squares)."""
    half = _HALF_TILE
    cx = x + half + shift_x
    cy = y + half + shift_y
    ox, oy = pose.apply(cx, cy)
    rx, ry = pose.apply(cx + 1.0, cy)
    dx, dy = pose.apply(cx, cy + 1.0)
    ux = rx - ox
    uy = ry - oy
    vx = dx - ox
    vy = dy - oy
    data = image.data
    w = image.width
    samples: List[float] = []
    append = samples.append
    # The samples are an affine image of the grid, so when the four corner samples lie a
    # pixel or more inside the image, every sample does: no clamp acts and the right and
    # lower neighbours always exist. The same arithmetic then runs without the border cases.
    first = steps[0]
    last = steps[-1]
    fwm2 = float(w - 2)
    fhm2 = float(image.height - 2)
    inside = True
    for b in (first, last):
        for a in (first, last):
            fx = ox + b * vx + a * ux - 0.5
            fy = oy + b * vy + a * uy - 0.5
            if not (1.0 <= fx <= fwm2 and 1.0 <= fy <= fhm2):
                inside = False
    if inside:
        for b in steps:
            row_x = ox + b * vx
            row_y = oy + b * vy
            for a in steps:
                fx = row_x + a * ux - 0.5
                fy = row_y + a * uy - 0.5
                x0 = int(fx)
                y0 = int(fy)
                tx = fx - x0
                ty = fy - y0
                r0 = y0 * w + x0
                r1 = r0 + w
                left = 1.0 - tx
                top = data[r0] * left + data[r0 + 1] * tx
                bottom = data[r1] * left + data[r1 + 1] * tx
                append(top * (1.0 - ty) + bottom * ty)
    else:
        sample = image.sample
        for b in steps:
            row_x = ox + b * vx
            row_y = oy + b * vy
            for a in steps:
                append(sample(row_x + a * ux, row_y + a * uy))
    mean = reduce(add, samples, 0.0) / len(samples)
    centred = [value - mean for value in samples]
    energy = reduce(add, map(mul, centred, centred), 0.0)
    return centred, mean, energy


def _correlate(samples: Sequence[float], energy: float, template: Sequence[float]) -> float:
    """Correlation of mean-centred samples of energy ``energy`` with a unit, mean-centred
    template."""
    if energy <= 1e-9:
        return 0.0
    return reduce(add, map(mul, samples, template), -0.0) / math.sqrt(energy)


def _tile_in_image(image: Luma, pose: Pose, x: float, y: float) -> bool:
    """Whether every corner of the tile at ``(x, y)`` maps inside the image."""
    side = KANA_TILE
    width = float(image.width)
    height = float(image.height)
    for u, v in ((0.0, 0.0), (side, 0.0), (0.0, side), (side, side)):
        px, py = pose.apply(x + u, y + v)
        if not (px >= 0.0 and py >= 0.0 and px < width and py < height):
            return False
    return True


@dataclass(frozen=True)
class _FoundPilot:
    """A pilot as found: its tile centre, the shift that fits it best, its level and its
    polarity."""

    centre: Tuple[float, float]
    shift: Tuple[float, float]
    level: float
    light: bool


def _pilot_reach(y: float) -> float:
    """How far (canvas units) the pose can miss a pilot centred ``y`` units down the square
    (negative above it): 4 units at the square's edge, growing by 10 over a band's 290-unit
    height, because a lens's distortion grows away from the square the pose was fitted to."""
    beyond = -y if y < 0.0 else y - CANVAS
    return 4.0 + 10.0 * f64_min(f64_max(beyond, 0.0) / 290.0, 1.2)


_TILES = kana_tiles()


def _pilot_template(
    pilot: int,
    light_templates: Sequence[Sequence[float]],
    dark_templates: Sequence[Sequence[float]],
) -> Sequence[float]:
    """The template of what pilot ``pilot`` shows, from a light and a dark table."""
    glyph, light = KANA_PILOT_SYMBOLS[pilot]
    return light_templates[glyph] if light else dark_templates[glyph]


def _pilot_score(
    image: Luma, pose: Pose, tile: int, blur: int, shift: Tuple[float, float]
) -> float:
    """The correlation of a pilot tile's samples, moved by ``shift``, with its template under
    ``blur``."""
    t = _TILES[tile]
    if t.pilot is None:
        return -F64_MAX
    templates = _tables().blurs[blur]
    template = _pilot_template(t.pilot, templates.light, templates.dark)
    samples, _, energy = _sample_tile(image, pose, t.x, t.y, shift[0], shift[1], _FINE_STEPS)
    return _correlate(samples, energy, template)


def _coarse_pilot(image: Luma, pose: Pose, tile: int) -> Tuple[Tuple[float, float], float]:
    """Coarse search of a pilot: shifts on a :data:`_COARSE_STEP` grid within its
    :func:`_pilot_reach`, sampled on the coarse grid under the coarse template; returns the
    best shift and its correlation."""
    t = _TILES[tile]
    if t.pilot is None:
        return (0.0, 0.0), -F64_MAX
    tables = _tables()
    template = _pilot_template(t.pilot, tables.coarse_light, tables.coarse_dark)
    steps = math.floor(_pilot_reach(t.centre()[1]) / _COARSE_STEP)
    best_shift = (0.0, 0.0)
    best = -F64_MAX
    for dy in range(-steps, steps + 1):
        for dx in range(-steps, steps + 1):
            shift_x = _COARSE_STEP * dx
            shift_y = _COARSE_STEP * dy
            samples, _, energy = _sample_tile(
                image, pose, t.x, t.y, shift_x, shift_y, _COARSE_STEPS
            )
            score = _correlate(samples, energy, template)
            if score > best:
                best_shift = (shift_x, shift_y)
                best = score
    return best_shift, best


def _refine_pilot(
    image: Luma,
    pose: Pose,
    tile: int,
    blur: int,
    start: Tuple[float, float],
    steps: Sequence[float],
) -> Tuple[Tuple[float, float], float]:
    """Refines a pilot's shift under ``blur``: the best of the shifts one step around it, for
    steps of ``steps`` in turn (ties keep the earlier)."""
    best_shift = start
    best = _pilot_score(image, pose, tile, blur, start)
    for step in steps:
        cx, cy = best_shift
        for dy in (-1, 0, 1):
            for dx in (-1, 0, 1):
                if dx == 0 and dy == 0:
                    continue
                shift = (cx + step * dx, cy + step * dy)
                score = _pilot_score(image, pose, tile, blur, shift)
                if score > best:
                    best_shift = shift
                    best = score
    return best_shift, best


def _solve3(
    matrix: Sequence[Sequence[float]], right: Sequence[float]
) -> Optional[Tuple[float, float, float]]:
    """Solves the 3 x 3 system ``m x = r`` by Gauss-Jordan elimination with partial pivoting
    (of equal pivots the last, as Rust's ``max_by`` picks); ``None`` when a pivot is below
    ``1e-9``."""
    m = [list(row) for row in matrix]
    r = list(right)
    for col in range(3):
        pivot = col
        best = total_key(abs(m[col][col]))
        for row in range(col + 1, 3):
            key = total_key(abs(m[row][col]))
            if key >= best:
                best = key
                pivot = row
        if abs(m[pivot][col]) < 1e-9:
            return None
        m[col], m[pivot] = m[pivot], m[col]
        r[col], r[pivot] = r[pivot], r[col]
        pivot_row = m[col]
        for row in range(3):
            if row == col:
                continue
            target = m[row]
            factor = target[col] / pivot_row[col]
            for k in range(col, 3):
                target[k] -= factor * pivot_row[k]
            r[row] -= factor * r[col]
    return r[0] / m[0][0], r[1] / m[1][1], r[2] / m[2][2]


class _ShiftField:
    """The shift of every point of a band: an affine function of the tile centre fitted to
    the found pilots' shifts by least squares (``x`` scaled by 512 around the square's
    middle, ``y`` by 145 around the pilots' mean), or their mean shift when fewer than three
    pilots fit one."""

    __slots__ = ("x", "y", "y_centre")

    def __init__(self, pilots: Sequence[_FoundPilot]) -> None:
        n = len(pilots)
        y_centre = reduce(add, (p.centre[1] for p in pilots), -0.0) / n
        self.y_centre = y_centre
        self.x: Tuple[float, ...] = (reduce(add, (p.shift[0] for p in pilots), -0.0) / n, 0.0, 0.0)
        self.y: Tuple[float, ...] = (reduce(add, (p.shift[1] for p in pilots), -0.0) / n, 0.0, 0.0)
        if n < 3:
            return
        m = [[0.0] * 3 for _ in range(3)]
        rx = [0.0] * 3
        ry = [0.0] * 3
        half = CANVAS / 2.0
        for p in pilots:
            basis = (1.0, (p.centre[0] - half) / 512.0, (p.centre[1] - y_centre) / 145.0)
            for i in range(3):
                row = m[i]
                bi = basis[i]
                for j in range(3):
                    row[j] += bi * basis[j]
                rx[i] += bi * p.shift[0]
                ry[i] += bi * p.shift[1]
        x = _solve3(m, rx)
        y = _solve3(m, ry)
        if x is not None and y is not None:
            self.x = x
            self.y = y

    def at(self, centre: Tuple[float, float]) -> Tuple[float, float]:
        b1 = (centre[0] - CANVAS / 2.0) / 512.0
        b2 = (centre[1] - self.y_centre) / 145.0
        x = self.x
        y = self.y
        return (
            x[0] * 1.0 + x[1] * b1 + x[2] * b2,
            y[0] * 1.0 + y[1] * b1 + y[2] * b2,
        )


def _level_line(points: Sequence[Tuple[float, float]]) -> Optional[Tuple[float, float]]:
    """A level that varies linearly across the band, fitted by least squares to pilot levels
    against ``(x - 512) / 512``; a constant when the pilots do not spread."""
    if not points:
        return None
    n = len(points)
    mx = reduce(add, (p[0] for p in points), -0.0) / n
    my = reduce(add, (p[1] for p in points), -0.0) / n
    sxx = reduce(add, ((p[0] - mx) * (p[0] - mx) for p in points), -0.0)
    if sxx < 1e-6:
        return my, 0.0
    sxy = reduce(add, ((p[0] - mx) * (p[1] - my) for p in points), -0.0)
    slope = sxy / sxx
    return my - slope * mx, slope


@dataclass(frozen=True)
class TileReading:
    """What a band showed one data tile as."""

    #: Index into :func:`~iroha_petal.layout.kana_tiles`.
    tile: int
    #: Decided polarity.
    light: bool
    #: How far the tile's mean level lay from the threshold, in half-contrasts.
    light_margin: float
    #: Decided glyph.
    glyph: int
    #: How far the glyph's correlation led the runner-up's.
    glyph_lead: float


@dataclass(frozen=True)
class BandReading:
    """What a reader saw of one band."""

    #: The blur (sigma in canvas units) the pilots chose.
    blur: float
    #: Pilots found.
    pilots: int
    #: Every data tile of the band in data order, or ``None`` for tiles outside the image or
    #: where the pilots' levels show no contrast.
    tiles: Tuple[Optional[TileReading], ...]


def observe_band(image: Luma, frame: DecodedFrame, band: int) -> Optional[BandReading]:
    """Read one band's tiles, before any Reed-Solomon correction (for diagnostics); ``None``
    when fewer than two pilots are found or the pilots show no contrast between light and
    dark tiles (or the plane's buffer does not match its size)."""
    _check_band(band)
    if len(image.data) != image.width * image.height:
        return None
    pose = frame.pose
    tiles = _TILES
    # coarse search of every pilot in view
    coarse: List[Tuple[int, Tuple[float, float]]] = []
    for tile in kana_pilot_tiles(band):
        t = tiles[tile]
        if not _tile_in_image(image, pose, t.x, t.y):
            continue
        shift, score = _coarse_pilot(image, pose, tile)
        if score >= _PILOT_COARSE_MIN:
            coarse.append((tile, shift))
    if len(coarse) < 2:
        return None
    # the blur under which the pilots, each at its best shift within a unit, correlate best
    blur = 0
    best = -F64_MAX
    for index in range(len(KANA_BLURS)):
        total = reduce(
            add,
            (_refine_pilot(image, pose, tile, index, shift, (1.0,))[1] for tile, shift in coarse),
            -0.0,
        )
        if total > best:
            best = total
            blur = index
    pilots: List[_FoundPilot] = []
    for tile, start in coarse:
        shift, score = _refine_pilot(image, pose, tile, blur, start, (1.0, 0.5, 0.25))
        if score < _PILOT_MIN:
            continue
        t = tiles[tile]
        _, level, _ = _sample_tile(image, pose, t.x, t.y, shift[0], shift[1], _FINE_STEPS)
        light = KANA_PILOT_SYMBOLS[t.pilot][1]  # type: ignore[index]
        pilots.append(_FoundPilot(t.centre(), shift, level, light))
    if len(pilots) < 2:
        return None
    field = _ShiftField(pilots)
    half = CANVAS / 2.0
    light_line = _level_line([((p.centre[0] - half) / 512.0, p.level) for p in pilots if p.light])
    dark_line = _level_line(
        [((p.centre[0] - half) / 512.0, p.level) for p in pilots if not p.light]
    )
    if light_line is None or dark_line is None:
        return None
    templates = _tables().blurs[blur]
    inside_template = templates.inside
    inks = templates.ink
    light_norms = templates.light_norm
    dark_norms = templates.dark_norm
    readings: List[Optional[TileReading]] = []
    for tile in kana_data_tiles(band):
        t = tiles[tile]
        if not _tile_in_image(image, pose, t.x, t.y):
            readings.append(None)
            continue
        centre = t.centre()
        shift_x, shift_y = field.at(centre)
        samples, mean, energy = _sample_tile(image, pose, t.x, t.y, shift_x, shift_y, _FINE_STEPS)
        x = (centre[0] - half) / 512.0
        high = light_line[0] + light_line[1] * x
        low = dark_line[0] + dark_line[1] * x
        contrast = (high - low) / 2.0
        if contrast <= 1.0:
            readings.append(None)
            continue
        threshold = (high + low) / 2.0
        # a light template is the inside less the ink, so one dot product with the inside
        # and one with each glyph's ink give both polarities' correlations
        scores = [0.0] * GLYPH_COUNT
        if energy > 1e-9:
            root = math.sqrt(energy)
            inside = reduce(add, map(mul, samples, inside_template), -0.0)
            for g in range(GLYPH_COUNT):
                ink = reduce(add, map(mul, samples, inks[g]), -0.0)
                light = (inside - ink) / light_norms[g] / root
                dark = ink / dark_norms[g] / root
                scores[g] = f64_max(light, dark)
        first = 0
        for g in range(1, GLYPH_COUNT):
            if scores[g] > scores[first]:
                first = g
        second = -F64_MAX
        for g in range(GLYPH_COUNT):
            if g != first:
                second = f64_max(second, scores[g])
        readings.append(
            TileReading(
                tile=tile,
                light=mean > threshold,
                light_margin=abs(mean - threshold) / contrast,
                glyph=first,
                glyph_lead=scores[first] - second,
            )
        )
    return BandReading(blur=KANA_BLURS[blur], pilots=len(pilots), tiles=tuple(readings))


@dataclass(frozen=True)
class KanaResult:
    """A kana lane read from a capture, with the header (stream tag and frame) its check
    matched."""

    #: The lane data (atoms and check).
    result: LaneResult
    #: The header the check matched: the square's frame or a neighbouring one.
    header: LaneHeader


def _decode_kana_word(
    lane: KanaLane, header: LaneHeader, word: bytes, confidence: Sequence[float]
) -> Optional[KanaResult]:
    """Tries Reed-Solomon with 0, n/8, n/4, n/3 and n/2 of the least confident bytes erased
    (n the parity bytes; ties keep byte order) and accepts the first result whose check
    matches the square's header or the frame after or before it."""
    order = sorted(range(len(word)), key=lambda i: total_key(confidence[i]))
    nsym = lane.parity_len
    schedule: List[int] = []
    for count in (0, nsym // 8, nsym // 4, nsym // 3, nsym // 2):
        if not schedule or schedule[-1] != count:
            schedule.append(count)
    frames = (header.frame, (header.frame + 1) & 0xFFFF, (header.frame - 1) & 0xFFFF)
    for erasures in schedule:
        positions = order[:erasures]
        trial = bytearray(word)
        for p in positions:
            trial[p] = 0
        try:
            data, corrected = decode_kana_lane_counted(lane, trial, positions)
        except RsError:
            continue
        for frame in frames:
            candidate = LaneHeader(header.tag, frame)
            if kana_packet(candidate, lane, data) is not None:
                return KanaResult(
                    result=LaneResult(data=data, corrected=corrected, erasures=len(positions)),
                    header=candidate,
                )
    return None


_NO_SKIP = (False,) * KANA_BANDS


def read_kana(
    image: Luma,
    frame: DecodedFrame,
    header: LaneHeader,
    skip: Sequence[bool] = _NO_SKIP,
) -> Tuple[Optional[KanaResult], ...]:
    """Read the kana lanes of the bands not in ``skip`` (one flag per band, band 0 first) of
    a capture whose square ``frame`` decoded under ``header`` (the stream tag and frame its
    lanes name).

    A lane passes when its Reed-Solomon codeword decodes, with 0, 1/8, 1/4, 1/3 or 1/2 of
    its parity bytes' worth of its least confident bytes erased (polarity 0, 2, 4, 5 and 8;
    glyphs 0, 4, 8, 11 and 16), into data whose :func:`kana_check` matches ``header``'s
    frame, the frame after it or the frame before it. A byte's confidence is the least of
    its tiles' (a tile that is not read has confidence 0 and shows 0 bits). Returns four
    entries in the order ``P0``, ``K0``, ``P1``, ``K1``, ``None`` where no lane was read;
    :func:`kana_packet` turns a result into atoms under its :attr:`KanaResult.header`.

    ``image`` is the luma plane ``frame`` was decoded from (``frame.pose`` maps into its
    pixel coordinates).
    """
    skip = tuple(bool(s) for s in skip)
    if len(skip) != KANA_BANDS:
        raise ValueError("skip names the two bands")
    results: List[Optional[KanaResult]] = [None] * len(KANA_LANES)
    for band in range(KANA_BANDS):
        if skip[band]:
            continue
        reading = observe_band(image, frame, band)
        if reading is None:
            continue
        polarity = bytearray(KANA_POLARITY_WORD)
        polarity_confidence = [F64_MAX] * KANA_POLARITY_WORD
        glyphs = bytearray(KANA_GLYPH_WORD)
        glyph_confidence = [F64_MAX] * KANA_GLYPH_WORD
        bits = KANA_POLARITY_WORD * 8
        for i, tile in enumerate(reading.tiles):
            if tile is None:
                light, margin, glyph, lead = False, 0.0, 0, 0.0
            else:
                light, margin, glyph, lead = (
                    tile.light,
                    tile.light_margin,
                    tile.glyph,
                    tile.glyph_lead,
                )
            if i < bits:
                if light:
                    polarity[i >> 3] |= 1 << (7 - (i & 7))
                polarity_confidence[i >> 3] = f64_min(polarity_confidence[i >> 3], margin)
            glyphs[i >> 1] |= glyph << 4 if i % 2 == 0 else glyph
            glyph_confidence[i >> 1] = f64_min(glyph_confidence[i >> 1], lead)
        results[2 * band] = _decode_kana_word(
            KANA_LANES[2 * band], header, bytes(polarity), polarity_confidence
        )
        results[2 * band + 1] = _decode_kana_word(
            KANA_LANES[2 * band + 1], header, bytes(glyphs), glyph_confidence
        )
    return tuple(results)
