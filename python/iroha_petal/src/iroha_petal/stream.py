# Copyright 2026 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

"""Payload streams: the sender-side :class:`StreamEncoder` and the
receiver-side :class:`StreamAssembler`.

Every frame carries a handful of fountain atoms, one lane at a time: lane ``P``
one atom, lane ``K`` five, and lane ``D`` one atom, except on every fourth
frame (``frame % 4 == 0``), when lane ``D`` carries the stream *beacon*
instead, so a receiver can join at any frame within a fraction of a second.
Atom ids run contiguously over the atoms actually sent (see
:func:`first_atom_id`). Any single readable lane is useful on its own.

Lane data layouts (all big-endian):

* every lane starts with ``tag:u8, frame:u16``; ``tag`` is the low byte of the
  payload CRC-32C.
* lanes ``P``, ``K`` and non-beacon ``D``: ``atoms...`` (16 bytes each).
* beacon ``D``: ``version:u8, kind:u8, len:u24, crc:u32``, zero padded.
"""

from __future__ import annotations

import collections
import enum
from dataclasses import dataclass
from typing import Deque, Optional, Tuple, Union

from .crc import crc32c
from .fountain import FountainDecoder, encode_atom, split_payload
from .lanes import (
    ATOM_LEN,
    D_ATOMS,
    D_DATA,
    K_ATOMS,
    LANE_HEADER_LEN,
    P_ATOMS,
    FrameCells,
    Lane,
    encode_lane,
)

__all__ = [
    "FORMAT_VERSION",
    "MAX_PAYLOAD_LEN",
    "DEFAULT_MAX_PAYLOAD_LEN",
    "BEACON_INTERVAL",
    "is_beacon_frame",
    "atoms_in_frame",
    "first_atom_id",
    "StreamErrorKind",
    "StreamError",
    "StreamMeta",
    "LaneHeader",
    "Beacon",
    "AtomPacket",
    "DLane",
    "parse_atom_lane",
    "parse_d_lane",
    "StreamEncoder",
    "AssemblerLimits",
    "Completed",
    "Progress",
    "StreamAssembler",
]

#: Version/profile byte of the beacon: format version 1, layout profile 0.
FORMAT_VERSION = 0x10
#: Largest payload a beacon can describe (``u24``).
MAX_PAYLOAD_LEN = (1 << 24) - 1
#: Default receiver payload limit; override with :class:`AssemblerLimits`.
DEFAULT_MAX_PAYLOAD_LEN = 65_536
#: A beacon replaces the lane-``D`` atom on frames divisible by this interval.
BEACON_INTERVAL = 4

_BEACON_BODY_LEN = 9
_U32 = 0xFFFFFFFF


def _check_frame(frame: int) -> int:
    if not 0 <= frame <= 0xFFFF:
        raise ValueError("frame counters are 16-bit unsigned integers")
    return frame


def is_beacon_frame(frame: int) -> bool:
    """Whether ``frame`` carries the beacon in lane ``D``."""
    return frame % BEACON_INTERVAL == 0


def atoms_in_frame(frame: int) -> int:
    """Fountain atoms carried by ``frame``."""
    if is_beacon_frame(frame):
        return P_ATOMS + K_ATOMS
    return P_ATOMS + D_ATOMS + K_ATOMS


def first_atom_id(frame: int) -> int:
    """Fountain id of the first atom of ``frame``.

    Frame ``f`` follows ``f`` earlier frames, ``ceil(f / 4)`` of which were
    beacon frames with one atom fewer.
    """
    f = _check_frame(frame)
    return f * (P_ATOMS + D_ATOMS + K_ATOMS) - (f + BEACON_INTERVAL - 1) // BEACON_INTERVAL


def _lane_first_id(lane: Lane, frame: int) -> int:
    base = first_atom_id(frame)
    if lane is Lane.P:
        return base
    if lane is Lane.D:
        return base + P_ATOMS
    return base + P_ATOMS + (0 if is_beacon_frame(frame) else D_ATOMS)


class StreamErrorKind(enum.Enum):
    """Reasons the sender rejects a payload."""

    #: The payload is empty.
    EMPTY_PAYLOAD = "petal stream payload is empty"
    #: The payload exceeds :data:`MAX_PAYLOAD_LEN`.
    PAYLOAD_TOO_LARGE = "petal stream payload exceeds the 24-bit length field"


class StreamError(ValueError):
    """The sender rejected a payload; :attr:`kind` says why."""

    def __init__(self, kind: StreamErrorKind) -> None:
        super().__init__(kind.value)
        self.kind = kind


@dataclass(frozen=True)
class StreamMeta:
    """Identity of a stream, as carried by every beacon."""

    #: Application payload kind.
    kind: int
    #: Payload length in bytes.
    length: int
    #: CRC-32C of the payload.
    crc: int

    @property
    def tag(self) -> int:
        """The one-byte stream tag repeated in every lane header."""
        return self.crc & 0xFF

    @property
    def source_atoms(self) -> int:
        """Number of fountain source atoms."""
        return (self.length + ATOM_LEN - 1) // ATOM_LEN


@dataclass(frozen=True)
class LaneHeader:
    """The common three-byte header of every lane."""

    #: Stream tag.
    tag: int
    #: Frame counter (wraps at 65536).
    frame: int


@dataclass(frozen=True)
class Beacon:
    """A decoded beacon."""

    #: Lane header.
    header: LaneHeader
    #: Stream identity.
    meta: StreamMeta


@dataclass(frozen=True)
class AtomPacket:
    """Atoms read from one lane."""

    #: Lane header.
    header: LaneHeader
    #: Fountain id of the first atom; the rest follow consecutively.
    first_id: int
    #: The 16-byte atoms.
    atoms: Tuple[bytes, ...]


#: What lane ``D`` carried: the stream beacon or a payload atom.
DLane = Union[Beacon, AtomPacket]


def _parse_header(data: bytes) -> LaneHeader:
    return LaneHeader(tag=data[0], frame=(data[1] << 8) | data[2])


def _parse_atoms(lane: Lane, data: bytes, header: LaneHeader, count: int) -> AtomPacket:
    atoms = tuple(
        bytes(data[LANE_HEADER_LEN + i * ATOM_LEN : LANE_HEADER_LEN + (i + 1) * ATOM_LEN])
        for i in range(count)
    )
    return AtomPacket(header=header, first_id=_lane_first_id(lane, header.frame), atoms=atoms)


def parse_atom_lane(lane: Lane, data: bytes) -> Optional[AtomPacket]:
    """Parse the data bytes of lane ``P`` or lane ``K``."""
    if lane is Lane.P:
        count = P_ATOMS
    elif lane is Lane.K:
        count = K_ATOMS
    else:
        return None
    if len(data) != lane.data_len:
        return None
    return _parse_atoms(lane, data, _parse_header(data), count)


def parse_d_lane(data: bytes) -> Optional[DLane]:
    """Parse the data bytes of lane ``D``: a :class:`Beacon` or an :class:`AtomPacket`."""
    if len(data) != D_DATA:
        return None
    header = _parse_header(data)
    if not is_beacon_frame(header.frame):
        return _parse_atoms(Lane.D, data, header, D_ATOMS)
    body = data[LANE_HEADER_LEN:]
    if body[0] != FORMAT_VERSION:
        return None
    length = (body[2] << 16) | (body[3] << 8) | body[4]
    if length == 0:
        return None
    crc = int.from_bytes(bytes(body[5:9]), "big")
    return Beacon(header=header, meta=StreamMeta(kind=body[1], length=length, crc=crc))


class StreamEncoder:
    """Sender side: turns one payload into an endless sequence of frames."""

    __slots__ = ("_meta", "_source")

    def __init__(self, payload: bytes, kind: int) -> None:
        """Prepare ``payload`` of application kind ``kind`` (``0..=255``) for streaming.

        Raises :class:`StreamError` for empty or oversized payloads.
        """
        payload = bytes(payload)
        if not 0 <= kind <= 0xFF:
            raise ValueError("payload kind is an 8-bit unsigned integer")
        if not payload:
            raise StreamError(StreamErrorKind.EMPTY_PAYLOAD)
        if len(payload) > MAX_PAYLOAD_LEN:
            raise StreamError(StreamErrorKind.PAYLOAD_TOO_LARGE)
        self._meta = StreamMeta(kind=kind, length=len(payload), crc=crc32c(payload))
        self._source = split_payload(payload)

    @property
    def meta(self) -> StreamMeta:
        """Stream identity."""
        return self._meta

    def systematic_frames(self) -> int:
        """Frames needed to send every source atom once (no losses, no repair)."""
        frames = 0
        atoms = 0
        while atoms < len(self._source):
            atoms += atoms_in_frame(frames & 0xFFFF)
            frames += 1
        return frames

    def _atoms(self, first_id: int, count: int) -> bytes:
        crc = self._meta.crc
        return b"".join(encode_atom(self._source, crc, (first_id + i) & _U32) for i in range(count))

    def lane_data(self, frame: int) -> Tuple[bytes, bytes, bytes]:
        """The data bytes of every lane of ``frame``: ``(P, K, D)``."""
        _check_frame(frame)
        header = bytes((self._meta.tag, frame >> 8, frame & 0xFF))
        p = header + self._atoms(_lane_first_id(Lane.P, frame), P_ATOMS)
        k = header + self._atoms(_lane_first_id(Lane.K, frame), K_ATOMS)
        if is_beacon_frame(frame):
            meta = self._meta
            body = (
                bytes((FORMAT_VERSION, meta.kind))
                + meta.length.to_bytes(3, "big")
                + meta.crc.to_bytes(4, "big")
            )
            body += bytes(D_DATA - LANE_HEADER_LEN - len(body))
            d = header + body
        else:
            d = header + self._atoms(_lane_first_id(Lane.D, frame), D_ATOMS)
        return p, k, d

    def words(self, frame: int) -> Tuple[bytes, bytes, bytes]:
        """The transmitted codewords of ``frame``: ``(P, K, D)``."""
        p, k, d = self.lane_data(frame)
        return encode_lane(Lane.P, p), encode_lane(Lane.K, k), encode_lane(Lane.D, d)

    def cells(self, frame: int) -> FrameCells:
        """Every cell of ``frame``, ready to render."""
        return FrameCells.from_words(*self.words(frame))


@dataclass(frozen=True)
class AssemblerLimits:
    """Receiver limits that bound memory and work."""

    #: Largest payload the receiver accepts.
    max_payload_len: int = DEFAULT_MAX_PAYLOAD_LEN
    #: Atoms buffered while waiting for the first beacon (0 drops them).
    max_pending_atoms: int = 128


@dataclass(frozen=True)
class Completed:
    """A reassembled, CRC-verified payload."""

    #: Stream identity.
    meta: StreamMeta
    #: The payload bytes.
    payload: bytes


@dataclass(frozen=True)
class Progress:
    """Snapshot of receive progress for a UI."""

    #: Stream identity once a beacon was accepted.
    meta: Optional[StreamMeta] = None
    #: Source atoms of the active stream.
    source_atoms: int = 0
    #: Independent atoms collected so far.
    rank: int = 0
    #: Atoms offered to the decoder (including duplicates).
    atoms_received: int = 0
    #: Reassembled payloads that failed the CRC check and were discarded
    #: (cumulative over the assembler's lifetime, not cleared by ``reset``).
    integrity_failures: int = 0
    #: Whether the payload is complete and verified.
    complete: bool = False


class _Active:
    __slots__ = ("meta", "decoder", "done")

    def __init__(self, meta: StreamMeta) -> None:
        self.meta = meta
        self.decoder = FountainDecoder(meta.source_atoms)
        self.done = False


class StreamAssembler:
    """Receiver side: collects atoms from any lane of any frame."""

    def __init__(self, limits: Optional[AssemblerLimits] = None) -> None:
        self._limits = limits if limits is not None else AssemblerLimits()
        self._active: Optional[_Active] = None
        self._pending: Deque[Tuple[int, int, bytes]] = collections.deque()
        self._conflicting: Optional[Tuple[StreamMeta, int]] = None
        self._completed: Optional[Completed] = None
        self._atoms_received = 0
        self._integrity_failures = 0

    @property
    def limits(self) -> AssemblerLimits:
        """The limits this assembler enforces."""
        return self._limits

    def reset(self) -> None:
        """Forget the active stream and any completed payload.

        The integrity-failure counter is cumulative and survives a reset.
        """
        self._active = None
        self._pending.clear()
        self._conflicting = None
        self._completed = None
        self._atoms_received = 0

    def progress(self) -> Progress:
        """Current progress."""
        active = self._active
        if active is None:
            return Progress(
                atoms_received=self._atoms_received,
                integrity_failures=self._integrity_failures,
            )
        return Progress(
            meta=active.meta,
            source_atoms=active.decoder.source_atoms,
            rank=active.decoder.rank,
            atoms_received=self._atoms_received,
            integrity_failures=self._integrity_failures,
            complete=active.done,
        )

    def take_completed(self) -> Optional[Completed]:
        """Take the completed payload, if any."""
        completed = self._completed
        self._completed = None
        return completed

    def _start(self, meta: StreamMeta) -> None:
        self._active = _Active(meta)
        self._conflicting = None
        self._completed = None
        self._atoms_received = 0
        tag = meta.tag
        pending = self._pending
        self._pending = collections.deque()
        for pending_tag, atom_id, atom in pending:
            if pending_tag == tag:
                self._add_atom(atom_id, atom)

    def push_beacon(self, beacon: Beacon) -> None:
        """Offer a beacon read from lane ``D``."""
        meta = beacon.meta
        if meta.length == 0 or meta.length > self._limits.max_payload_len:
            return
        active = self._active
        if active is None:
            self._start(meta)
        elif active.meta == meta:
            self._conflicting = None
        else:
            # A different stream: switch only after two consecutive sightings.
            conflicting = self._conflicting
            seen = conflicting[1] + 1 if conflicting is not None and conflicting[0] == meta else 1
            if seen >= 2:
                self._start(meta)
            else:
                self._conflicting = (meta, seen)

    def push_atoms(self, packet: AtomPacket) -> None:
        """Offer atoms read from a lane."""
        for index, atom in enumerate(packet.atoms):
            atom_id = (packet.first_id + index) & _U32
            active = self._active
            if active is not None:
                if active.meta.tag == packet.header.tag:
                    self._add_atom(atom_id, atom)
            else:
                limit = self._limits.max_pending_atoms
                if limit == 0:
                    continue
                if len(self._pending) >= limit:
                    self._pending.popleft()
                self._pending.append((packet.header.tag, atom_id, atom))

    def push_d_lane(self, lane: DLane) -> None:
        """Offer whatever lane ``D`` carried."""
        if isinstance(lane, Beacon):
            self.push_beacon(lane)
        else:
            self.push_atoms(lane)

    def _add_atom(self, atom_id: int, atom: bytes) -> None:
        active = self._active
        if active is None or active.done:
            return
        self._atoms_received = min(self._atoms_received + 1, _U32)
        decoder = active.decoder
        decoder.add_encoded(active.meta.crc, atom_id, atom)
        if not decoder.is_complete:
            return
        source = decoder.solve()
        if source is None:
            return
        payload = b"".join(source)[: active.meta.length]
        if crc32c(payload) == active.meta.crc:
            active.done = True
            self._completed = Completed(meta=active.meta, payload=payload)
        else:
            # Corrupt atoms slipped through: start the elimination over.
            self._integrity_failures = min(self._integrity_failures + 1, _U32)
            active.decoder = FountainDecoder(active.meta.source_atoms)
