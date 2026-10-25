# Copyright 2026 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

"""Lane codecs: bytes to and from transmitted cell states.

A frame carries five lanes, and four turn lanes on top of the field lanes (see
:mod:`iroha_petal.turns`). Each lane is exactly one Reed-Solomon codeword,
XOR-whitened with a fixed pseudo-random sequence so the picture is
statistically balanced whatever the payload is:

=========  =======================================  ========  =====  ======
lane       cells                                    codeword  data   parity
=========  =======================================  ========  =====  ======
D          240 ring slots x 1 bit                   30 B      19 B   11 B
F0 ... F3  840 field cells of one sector x 1 bit    105 B     83 B   22 B
T0 ... T3  360 lit petals of one sector x 2 bits    90 B      68 B   22 B
=========  =======================================  ========  =====  ======

Bit order is most-significant-bit first.
"""

from __future__ import annotations

import enum
from dataclasses import dataclass
from typing import Iterable, Optional, Sequence, Tuple

from .layout import (
    D_BITS,
    FIELD_CELLS,
    SECTOR_CELLS,
    SECTORS,
    TOTAL_SLOTS,
    SlotKind,
    data_slots,
    sector_cells,
    slot_roles,
)
from .prng import Xorshift32
from .rs import ReedSolomon, RsError, RsErrorKind

__all__ = [
    "ATOM_LEN",
    "LANE_HEADER_LEN",
    "D_ATOMS",
    "FIELD_ATOMS",
    "ATOMS_PER_FRAME",
    "D_WORD",
    "D_PARITY",
    "D_DATA",
    "FIELD_WORD",
    "FIELD_PARITY",
    "FIELD_DATA",
    "SPARE_SECTOR_CELLS",
    "Lane",
    "DECODE_ORDER",
    "FIELD_LANES",
    "encode_lane",
    "decode_lane",
    "decode_lane_counted",
    "FrameCells",
]

#: Length of an atom in bytes.
ATOM_LEN = 16
#: Bytes of the per-lane header (``tag``, ``frame`` high, ``frame`` low).
LANE_HEADER_LEN = 3
#: Atoms carried by lane ``D`` on frames that do not carry a beacon.
D_ATOMS = 1
#: Atoms carried by each field lane.
FIELD_ATOMS = 5
#: Most atoms one frame can carry (the four field lanes and lane ``D``).
ATOMS_PER_FRAME = SECTORS * FIELD_ATOMS + D_ATOMS

#: Codeword length of lane ``D`` in bytes.
D_WORD = D_BITS // 8
#: Parity bytes of lane ``D``.
D_PARITY = 11
#: Data bytes of lane ``D``.
D_DATA = D_WORD - D_PARITY
#: Codeword length of a field lane in bytes.
FIELD_WORD = 105
#: Parity bytes of a field lane.
FIELD_PARITY = 22
#: Data bytes of a field lane.
FIELD_DATA = FIELD_WORD - FIELD_PARITY
#: Field cells of a sector that carry no bit (they stay dark).
SPARE_SECTOR_CELLS = SECTOR_CELLS - FIELD_WORD * 8

assert D_DATA == LANE_HEADER_LEN + D_ATOMS * ATOM_LEN
assert FIELD_DATA == LANE_HEADER_LEN + FIELD_ATOMS * ATOM_LEN
assert FIELD_WORD * 8 <= SECTOR_CELLS

_FIELD_BITS = FIELD_WORD * 8


class Lane(enum.Enum):
    """One of the five data lanes of a frame: ``D`` (the ring dots) or the field lane
    ``F0`` ... ``F3`` of one petal sector. Iteration yields them in decode order."""

    #: Dots on the three rings.
    D = "D"
    #: The petals of field sector 0.
    F0 = "F0"
    #: The petals of field sector 1.
    F1 = "F1"
    #: The petals of field sector 2.
    F2 = "F2"
    #: The petals of field sector 3.
    F3 = "F3"

    @classmethod
    def field(cls, sector: int) -> "Lane":
        """The field lane of sector ``sector`` (``0..4``)."""
        if not 0 <= sector < SECTORS:
            raise ValueError("sector out of range")
        return FIELD_LANES[sector]

    @property
    def sector(self) -> Optional[int]:
        """The petal sector of a field lane; ``None`` for lane ``D``."""
        return None if self is Lane.D else int(self.value[1])

    @property
    def letter(self) -> str:
        """The letter that names the lane in reports: ``D``, or the sector digit ``0``-``3``."""
        return self.value[-1]

    @property
    def word_len(self) -> int:
        """Codeword length in bytes."""
        return D_WORD if self is Lane.D else FIELD_WORD

    @property
    def parity_len(self) -> int:
        """Parity bytes."""
        return D_PARITY if self is Lane.D else FIELD_PARITY

    @property
    def data_len(self) -> int:
        """Data bytes."""
        return self.word_len - self.parity_len

    @property
    def atoms(self) -> int:
        """Atoms carried (lane ``D``: on frames without a beacon)."""
        return D_ATOMS if self is Lane.D else FIELD_ATOMS

    def whitening(self) -> bytes:
        """The fixed whitening sequence of the lane."""
        return _WHITENING[self]


#: The four field lanes, sector 0 first.
FIELD_LANES = (Lane.F0, Lane.F1, Lane.F2, Lane.F3)
#: All lanes in decode order: lane ``D``, then the field lanes.
DECODE_ORDER = (Lane.D,) + FIELD_LANES


def _whitening_seed(lane: Lane) -> int:
    if lane is Lane.D:
        return 0x444F5453  # "DOTS"
    return 0x50455430 + FIELD_LANES.index(lane)  # "PET0" ... "PET3"


def _whitening(lane: Lane) -> bytes:
    rng = Xorshift32(_whitening_seed(lane))
    return bytes(rng.next_byte() for _ in range(lane.word_len))


_WHITENING = {lane: _whitening(lane) for lane in Lane}
_CODES = {D_PARITY: ReedSolomon(D_PARITY), FIELD_PARITY: ReedSolomon(FIELD_PARITY)}


def encode_lane(lane: Lane, data: bytes) -> bytes:
    """Encode lane data into the transmitted (whitened) codeword."""
    if len(data) != lane.data_len:
        raise ValueError("lane data length mismatch")
    word = _CODES[lane.parity_len].encode(data)
    return bytes(a ^ b for a, b in zip(word, _WHITENING[lane]))


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
    word = bytearray(a ^ b for a, b in zip(transmitted, _WHITENING[lane]))
    corrected = _CODES[lane.parity_len].decode(word, erasures)
    return bytes(word[: lane.data_len]), corrected


_ROLE_KINDS = tuple(role.kind for role in slot_roles())
_ROLE_BITS = tuple(role.bit for role in slot_roles())
#: The bit-carrying cells of every sector, in bit order.
_SECTOR_BITS = tuple(sector_cells(sector)[:_FIELD_BITS] for sector in range(SECTORS))
#: Every field cell unturned.
_UNTURNED = (0,) * FIELD_CELLS


@dataclass(frozen=True)
class FrameCells:
    """Every cell of one frame: what a renderer draws and a decoder samples."""

    #: Lit state of every ring slot, gate dots included.
    dots: Tuple[bool, ...]
    #: Lit state of every field cell (spare cells are dark).
    petals: Tuple[bool, ...]
    #: Turn position (0-3, see :func:`~iroha_petal.turns.turn_angle`) of every field cell:
    #: zero for dark petals and for lit petals that carry no turn bits. Empty (the
    #: default) means every petal is unturned.
    turns: Tuple[int, ...] = ()

    def __post_init__(self) -> None:
        dots = tuple(bool(v) for v in self.dots)
        petals = tuple(bool(v) for v in self.petals)
        turns = tuple(int(v) for v in self.turns) or _UNTURNED
        if len(dots) != TOTAL_SLOTS:
            raise ValueError("a frame has exactly 296 ring slots")
        if len(petals) != FIELD_CELLS:
            raise ValueError("a frame has exactly 3376 field cells")
        if len(turns) != FIELD_CELLS:
            raise ValueError("a frame turns exactly 3376 field cells")
        if min(turns) < 0 or max(turns) > 3:
            raise ValueError("a turn position is within 0..=3")
        object.__setattr__(self, "dots", dots)
        object.__setattr__(self, "petals", petals)
        object.__setattr__(self, "turns", turns)

    @classmethod
    def from_words(cls, d: bytes, fields: Sequence[bytes]) -> "FrameCells":
        """Build the cells from the transmitted codewords of lane ``D`` and the four
        field lanes (sector 0 first), with every petal unturned (see
        :meth:`with_turns`)."""
        if len(d) != D_WORD:
            raise ValueError("lane D codeword length mismatch")
        if len(fields) != SECTORS or any(len(word) != FIELD_WORD for word in fields):
            raise ValueError("a frame has four field codewords of 105 bytes")
        dots = []
        for kind, bit in zip(_ROLE_KINDS, _ROLE_BITS):
            if kind is SlotKind.GATE:
                dots.append(True)
            elif kind is SlotKind.DATA:
                dots.append((d[bit >> 3] >> (7 - (bit & 7))) & 1 == 1)
            else:
                dots.append(False)
        petals = [False] * FIELD_CELLS
        for cells, word in zip(_SECTOR_BITS, fields):
            for bit, cell in enumerate(cells):
                petals[cell] = (word[bit >> 3] >> (7 - (bit & 7))) & 1 == 1
        return cls(tuple(dots), tuple(petals))

    def d_word(self) -> bytes:
        """Pack the data dots into a lane ``D`` codeword."""
        word = bytearray(D_WORD)
        for bit, slot in enumerate(data_slots()):
            if self.dots[slot]:
                word[bit >> 3] |= 1 << (7 - (bit & 7))
        return bytes(word)

    def field_word(self, sector: int) -> bytes:
        """Pack the petals of sector ``sector`` into its field codeword."""
        if not 0 <= sector < SECTORS:
            raise IndexError("sector out of range")
        word = bytearray(FIELD_WORD)
        petals = self.petals
        for bit, cell in enumerate(_SECTOR_BITS[sector]):
            if petals[cell]:
                word[bit >> 3] |= 1 << (7 - (bit & 7))
        return bytes(word)

    def with_turns(self, words: Sequence[bytes]) -> "FrameCells":
        """The same cells with the carriers of every sector turned to show its transmitted
        turn codeword (sector 0 first, see :mod:`iroha_petal.turns`); a sector with too
        few lit petals stays unturned.

        Raises :class:`ValueError` when a codeword has the wrong length.
        """
        # imported here: the turns module builds on this one
        from .turns import TURN_WORD, carrier_order, carrier_position, carriers

        if len(words) != SECTORS or any(len(word) != TURN_WORD for word in words):
            raise ValueError("a frame has four turn codewords of 90 bytes")
        turns = list(self.turns)
        for sector, word in enumerate(words):
            for cell in carrier_order(sector):
                turns[cell] = 0
            chosen = carriers(self.petals, sector)
            if chosen is None:
                continue
            for index, cell in enumerate(chosen):
                turns[cell] = carrier_position(word, index)
        return FrameCells(self.dots, self.petals, tuple(turns))

    def turn_word(self, sector: int) -> Optional[bytes]:
        """Pack the turns of the carriers of sector ``sector`` into its turn codeword, or
        ``None`` when the sector has too few lit petals to carry one."""
        from .turns import TURN_WORD, carriers, turn_label

        if not 0 <= sector < SECTORS:
            raise IndexError("sector out of range")
        chosen = carriers(self.petals, sector)
        if chosen is None:
            return None
        word = bytearray(TURN_WORD)
        turns = self.turns
        for index, cell in enumerate(chosen):
            word[index >> 2] |= turn_label(turns[cell]) << (6 - 2 * (index & 3))
        return bytes(word)
