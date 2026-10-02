# Copyright 2026 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

"""Lane codecs: bytes to and from transmitted cell states.

A frame carries three lanes. Each lane is exactly one Reed-Solomon codeword,
XOR-whitened with a fixed pseudo-random sequence so the picture is
statistically balanced whatever the payload is:

====  ======================  ========  =====  ======
lane  cells                   codeword  data   parity
====  ======================  ========  =====  ======
P     256 tiles x 1 bit       32 B      19 B   13 B
K     256 tiles x 4 bits      128 B     83 B   45 B
D     240 ring slots x 1 bit  30 B      19 B   11 B
====  ======================  ========  =====  ======

Bit order is most-significant-bit first; lane ``K`` packs the first tile of a
pair into the high nibble.
"""

from __future__ import annotations

import enum
from dataclasses import dataclass
from typing import Iterable, Tuple

from .layout import D_BITS, TILE_COUNT, TOTAL_SLOTS, SlotKind, data_slots, slot_roles
from .prng import Xorshift32
from .rs import ReedSolomon, RsError, RsErrorKind

__all__ = [
    "ATOM_LEN",
    "LANE_HEADER_LEN",
    "P_ATOMS",
    "D_ATOMS",
    "K_ATOMS",
    "ATOMS_PER_FRAME",
    "P_WORD",
    "K_WORD",
    "D_WORD",
    "P_PARITY",
    "K_PARITY",
    "D_PARITY",
    "P_DATA",
    "K_DATA",
    "D_DATA",
    "Lane",
    "DECODE_ORDER",
    "encode_lane",
    "decode_lane",
    "decode_lane_counted",
    "FrameCells",
]

#: Length of an atom in bytes.
ATOM_LEN = 16
#: Bytes of the per-lane header (``tag``, ``frame`` high, ``frame`` low).
LANE_HEADER_LEN = 3
#: Atoms carried by lane ``P``.
P_ATOMS = 1
#: Atoms carried by lane ``D`` on frames that do not carry a beacon.
D_ATOMS = 1
#: Atoms carried by lane ``K``.
K_ATOMS = 5
#: Most atoms one frame can carry (lanes ``P``, ``D`` and ``K``).
ATOMS_PER_FRAME = P_ATOMS + D_ATOMS + K_ATOMS

#: Codeword length of lane ``P`` in bytes.
P_WORD = TILE_COUNT // 8
#: Codeword length of lane ``K`` in bytes.
K_WORD = TILE_COUNT // 2
#: Codeword length of lane ``D`` in bytes.
D_WORD = D_BITS // 8
#: Parity bytes of lane ``P``.
P_PARITY = 13
#: Parity bytes of lane ``K``.
K_PARITY = 45
#: Parity bytes of lane ``D``.
D_PARITY = 11
#: Data bytes of lane ``P``.
P_DATA = P_WORD - P_PARITY
#: Data bytes of lane ``K``.
K_DATA = K_WORD - K_PARITY
#: Data bytes of lane ``D``.
D_DATA = D_WORD - D_PARITY

assert P_DATA == LANE_HEADER_LEN + P_ATOMS * ATOM_LEN
assert K_DATA == LANE_HEADER_LEN + K_ATOMS * ATOM_LEN
assert D_DATA == LANE_HEADER_LEN + D_ATOMS * ATOM_LEN


class Lane(enum.Enum):
    """One of the three data lanes of a frame."""

    #: Light/dark polarity of the tiles.
    P = "P"
    #: Katakana glyph of each tile.
    K = "K"
    #: Dots on the three rings.
    D = "D"

    @property
    def word_len(self) -> int:
        """Codeword length in bytes."""
        return _WORD_LEN[self.value]

    @property
    def parity_len(self) -> int:
        """Parity bytes."""
        return _PARITY_LEN[self.value]

    @property
    def data_len(self) -> int:
        """Data bytes."""
        return _WORD_LEN[self.value] - _PARITY_LEN[self.value]

    def whitening(self) -> bytes:
        """The fixed whitening sequence of the lane."""
        return _WHITENING[self.value]


_WORD_LEN = {"P": P_WORD, "K": K_WORD, "D": D_WORD}
_PARITY_LEN = {"P": P_PARITY, "K": K_PARITY, "D": D_PARITY}
_WHITENING_SEED = {
    "P": 0x50455441,  # "PETA"
    "K": 0x4B414E41,  # "KANA"
    "D": 0x444F5453,  # "DOTS"
}


def _whitening(name: str) -> bytes:
    rng = Xorshift32(_WHITENING_SEED[name])
    return bytes(rng.next_byte() for _ in range(_WORD_LEN[name]))


_WHITENING = {name: _whitening(name) for name in ("P", "K", "D")}
_CODES = {name: ReedSolomon(_PARITY_LEN[name]) for name in ("P", "K", "D")}

#: All lanes in decode order.
DECODE_ORDER = (Lane.P, Lane.D, Lane.K)


def encode_lane(lane: Lane, data: bytes) -> bytes:
    """Encode lane data into the transmitted (whitened) codeword."""
    if len(data) != lane.data_len:
        raise ValueError("lane data length mismatch")
    word = _CODES[lane.value].encode(data)
    return bytes(a ^ b for a, b in zip(word, _WHITENING[lane.value]))


def decode_lane(lane: Lane, transmitted: bytes, erasures: Iterable[int] = ()) -> bytes:
    """Decode a transmitted codeword, returning the lane data bytes.

    ``erasures`` lists byte positions the caller distrusts. Raises
    :class:`~iroha_petal.rs.RsError` when the word is uncorrectable.
    """
    return decode_lane_counted(lane, transmitted, erasures)[0]


def decode_lane_counted(
    lane: Lane, transmitted: bytes, erasures: Iterable[int] = ()
) -> Tuple[bytes, int]:
    """Like :func:`decode_lane`, also returning how many byte positions the
    Reed-Solomon decoder rewrote (erased bytes plus unflagged errors).

    Raises :class:`~iroha_petal.rs.RsError` when the word is uncorrectable.
    """
    if len(transmitted) != lane.word_len:
        raise RsError(RsErrorKind.INVALID_SHAPE)
    word = bytearray(a ^ b for a, b in zip(transmitted, _WHITENING[lane.value]))
    corrected = _CODES[lane.value].decode(word, erasures)
    return bytes(word[: lane.data_len]), corrected


_ROLE_KINDS = tuple(role.kind for role in slot_roles())
_ROLE_BITS = tuple(role.bit for role in slot_roles())


@dataclass(frozen=True)
class FrameCells:
    """Every cell of one frame: what a renderer draws and a decoder samples."""

    #: Polarity of each tile; ``True`` is a light tile.
    light: Tuple[bool, ...]
    #: Glyph symbol (``0..16``) of each tile.
    glyph: Tuple[int, ...]
    #: Lit state of every ring slot, gate dots included.
    dots: Tuple[bool, ...]

    def __post_init__(self) -> None:
        light = tuple(bool(v) for v in self.light)
        glyph = tuple(int(v) for v in self.glyph)
        dots = tuple(bool(v) for v in self.dots)
        if len(light) != TILE_COUNT or len(glyph) != TILE_COUNT:
            raise ValueError("a frame has exactly 256 tiles")
        if len(dots) != TOTAL_SLOTS:
            raise ValueError("a frame has exactly 276 ring slots")
        if any(not 0 <= g < 16 for g in glyph):
            raise ValueError("glyph symbols are 4-bit values")
        object.__setattr__(self, "light", light)
        object.__setattr__(self, "glyph", glyph)
        object.__setattr__(self, "dots", dots)

    @classmethod
    def from_words(cls, p: bytes, k: bytes, d: bytes) -> "FrameCells":
        """Build the cells from the three transmitted codewords."""
        if len(p) != P_WORD or len(k) != K_WORD or len(d) != D_WORD:
            raise ValueError("lane codeword length mismatch")
        light = tuple((p[tile >> 3] >> (7 - (tile & 7))) & 1 == 1 for tile in range(TILE_COUNT))
        glyph = tuple(
            k[tile >> 1] >> 4 if tile % 2 == 0 else k[tile >> 1] & 0x0F
            for tile in range(TILE_COUNT)
        )
        dots = []
        for kind, bit in zip(_ROLE_KINDS, _ROLE_BITS):
            if kind is SlotKind.GATE:
                dots.append(True)
            elif kind is SlotKind.DATA:
                dots.append((d[bit >> 3] >> (7 - (bit & 7))) & 1 == 1)
            else:
                dots.append(False)
        return cls(light, glyph, tuple(dots))

    def p_word(self) -> bytes:
        """Pack the polarity cells into a lane ``P`` codeword."""
        word = bytearray(P_WORD)
        for tile, light in enumerate(self.light):
            if light:
                word[tile >> 3] |= 1 << (7 - (tile & 7))
        return bytes(word)

    def k_word(self) -> bytes:
        """Pack the glyph cells into a lane ``K`` codeword."""
        word = bytearray(K_WORD)
        for tile, glyph in enumerate(self.glyph):
            nibble = glyph & 0x0F
            word[tile >> 1] |= nibble << 4 if tile % 2 == 0 else nibble
        return bytes(word)

    def d_word(self) -> bytes:
        """Pack the data dots into a lane ``D`` codeword."""
        word = bytearray(D_WORD)
        for bit, slot in enumerate(data_slots()):
            if self.dots[slot]:
                word[bit >> 3] |= 1 << (7 - (bit & 7))
        return bytes(word)
