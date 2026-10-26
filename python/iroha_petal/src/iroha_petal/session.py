# Copyright 2026 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

"""The receive-side object an app holds while its camera is open.

After a frame decodes, the next frames are first read by tracking the code from
its last pose (:func:`~iroha_petal.decode.track_frame`), which skips the finder
search; a full :func:`~iroha_petal.decode.decode_frame` runs when tracking fails
or the last pose is older than :data:`TRACK_WINDOW_MS`.

A camera sees each shown frame two to four times, and noise, display banding or
hand shake can cost every one of those captures a field lane. When a capture
reads some lane but not every field lane, the session keeps its
:class:`~iroha_petal.decode.FieldEvidence` under the shown frame its decoded
lane headers name, for the last :data:`MERGE_WINDOW` shown frames. From the
second such capture on, it merges them
(:func:`~iroha_petal.decode.merge_fields`) and offers a recovered lane only
when that lane's own header names the same frame.

Every field lane a capture decodes also lets the session read the turn lane on
top of it (:func:`~iroha_petal.turns.read_turns`). A turn lane is tried once per
shown frame: the session remembers the last :data:`TURN_MEMORY` it tried and does
not look at those petals again. Reading turns costs a slow phone core several
times a frame's decode, so after a try that reads no turn lane (a camera too
soft or too shaky for the turns) the session passes over the next captures that
would try one, one more after every such try, at most :data:`TURN_BACKOFF`,
until a try reads a lane again.

The katakana bands above and below the square are read the same way
(:func:`~iroha_petal.kana.read_kana`): each band once per shown frame (the last
:data:`KANA_MEMORY` tried are remembered), with its own back-off of at most
:data:`KANA_BACKOFF` captures after tries that read no kana lane (a camera that
does not see the bands, or too blurred for their glyphs and polarity).

The bands go first. A capture whose bands read a kana lane tries the turn lanes
too only when it has time to spare: when the work it has done (finding or
following the code, a merge, the bands) and what the session's last turn try
took fit together in :data:`CAPTURE_BUDGET_MS`, a 30 fps camera's frame time
(before the first turn try, twice the work done stands in for it). Otherwise the
turn lanes of that shown frame wait for its next capture. On a slow phone core a
turn try costs five tracked decodes and a band try one, so reading both on one
capture overran the time a frame is shown at 12 fps and the session missed the
next shown frames; a fast decoder reads both at once, which matters when its
camera gives it few frames a second. A band try that reads nothing (bands out of
view, too blurred, or a torn capture) leaves the turn lanes to the same capture.

The session times its work on the monotonic clock (:func:`time.perf_counter`); a
``work_cost`` given to :class:`ScanSession` puts a model's costs in place of the
measured times, as tests do.
"""

from __future__ import annotations

import time
from collections import deque
from dataclasses import dataclass, field, replace
from enum import Enum
from typing import Callable, Deque, List, Optional, Tuple

from .decode import (
    DecodedFrame,
    DecodeError,
    DecodeErrorKind,
    DecodeOptions,
    FieldEvidence,
    decode_frame,
    field_evidence,
    merge_fields,
    track_frame,
)
from .image import Luma
from .kana import KANA_LANES, kana_packet, read_kana
from .lanes import DECODE_ORDER, FIELD_LANES
from .layout import KANA_BANDS, SECTORS
from .stream import (
    AssemblerLimits,
    Completed,
    LaneHeader,
    Progress,
    StreamAssembler,
    parse_atom_lane,
)
from .turns import read_turns, turn_packet

__all__ = [
    "TRACK_WINDOW_MS",
    "MERGE_WINDOW",
    "MERGE_CAPTURES",
    "TURN_MEMORY",
    "TURN_BACKOFF",
    "KANA_MEMORY",
    "KANA_BACKOFF",
    "CAPTURE_BUDGET_MS",
    "WorkKind",
    "Work",
    "ScanLimits",
    "ScanStats",
    "ScanOutcome",
    "ScanSession",
]

#: How long a decoded pose stays usable for tracking the next frames.
TRACK_WINDOW_MS = 500
#: Shown frames whose captures are held for merging.
MERGE_WINDOW = 6
#: Captures held per shown frame; a further capture replaces the oldest.
MERGE_CAPTURES = 4
#: Turn lanes remembered as tried (the four of each of the last :data:`MERGE_WINDOW` shown
#: frames).
TURN_MEMORY = MERGE_WINDOW * SECTORS
#: Most captures passed over after tries that read no turn lane.
TURN_BACKOFF = 8
#: Bands remembered as tried (the two of each of the last :data:`MERGE_WINDOW` shown frames).
KANA_MEMORY = MERGE_WINDOW * KANA_BANDS
#: Most captures passed over after tries that read no kana lane.
KANA_BACKOFF = 8
#: The time a capture's work may take in milliseconds, turn lanes included, for a capture
#: whose bands read a lane to try the turn lanes too: a 30 fps camera's frame time, so a
#: decoder that keeps to it misses no camera frame.
CAPTURE_BUDGET_MS = 1000 / 30


class WorkKind(Enum):
    """The kinds of a capture's work a :class:`ScanSession` times (see :class:`Work`)."""

    #: Finding the code by a full search and taking in the lanes read.
    SEARCH = "search"
    #: Following the code from the previous capture's pose and taking in the lanes read.
    TRACK = "track"
    #: Merging the held captures of a shown frame.
    MERGE = "merge"
    #: Reading the kana lanes of :attr:`Work.count` bands.
    BANDS = "bands"
    #: Reading the turn lanes.
    TURNS = "turns"


@dataclass(frozen=True)
class Work:
    """A part of a capture's work, as a :class:`ScanSession` times it to tell whether a
    capture has time to spare for the turn lanes (see the module documentation)."""

    #: What the work is.
    kind: WorkKind
    #: The bands read for :attr:`WorkKind.BANDS`; one otherwise.
    count: int = 1


def _measured(_work: Work, measured_ms: float) -> float:
    """The measured time, as a session on a camera judges its spare time."""
    return measured_ms


@dataclass(frozen=True)
class ScanLimits:
    """Limits of a scan session."""

    #: Forget a half-received stream after this long without progress.
    idle_timeout_ms: int = 30_000
    #: Forget a stream that has not finished this long after it started.
    absolute_timeout_ms: int = 180_000
    #: Assembler memory and size limits.
    assembler: AssemblerLimits = field(default_factory=AssemblerLimits)
    #: Image decoder options.
    decode: DecodeOptions = field(default_factory=DecodeOptions)


@dataclass(frozen=True)
class ScanStats:
    """Counters for diagnostics and UI hints."""

    #: Camera frames offered.
    frames: int = 0
    #: Frames in which a code was located, whether or not a lane could be read.
    located: int = 0
    #: Frames in which at least one lane decoded.
    readable: int = 0
    #: Lane ``D`` successes.
    lane_d: int = 0
    #: Field lane successes (up to four per frame).
    field_lanes: int = 0
    #: Frames read by tracking the previous pose instead of a full search.
    tracked: int = 0
    #: Frames read with one corner finder hidden and inferred.
    inferred: int = 0
    #: Merges of two or more captures of one shown frame.
    merges: int = 0
    #: Field lanes recovered by merging captures.
    merged_lanes: int = 0
    #: Turn lanes tried (each at most once per shown frame).
    turn_tries: int = 0
    #: Turn lanes read.
    turn_lanes: int = 0
    #: Bands tried (each at most once per shown frame).
    kana_tries: int = 0
    #: Kana lanes read (up to four per shown frame).
    kana_lanes: int = 0
    #: Captures whose bands read a lane that also tried the turn lanes, having time to spare.
    spare_turns: int = 0


@dataclass(frozen=True)
class ScanOutcome:
    """The result of offering one camera frame."""

    #: Why the frame produced nothing, when it did not. ``NO_ORIENTATION``
    #: means a code was located but no lane could be read (too far, too blurry).
    error: Optional[DecodeErrorKind]
    #: Lanes that decoded in this capture alone, as letters: ``D`` and the sector
    #: digits ``0``-``3``.
    lanes: str
    #: Receive progress after this frame.
    progress: Progress
    #: The finished payload, delivered exactly once.
    completed: Optional[Completed]
    #: Field lanes recovered by merging this capture with earlier captures of its
    #: shown frame, as sector digits.
    merged: str = ""
    #: Turn lanes read from this capture, as the sector digits of the field lanes they
    #: ride on.
    turned: str = ""
    #: Kana lanes read from this capture, as their names (``P0``, ``K0``, ``P1``, ``K1``).
    kana: str = ""


@dataclass
class _Held:
    """The captures of one shown frame held for merging."""

    shown: LaneHeader
    evidence: Deque[FieldEvidence] = field(default_factory=deque)
    done: List[bool] = field(default_factory=lambda: [False] * SECTORS)


def _lane_header(data: bytes) -> Optional[LaneHeader]:
    """The header every lane starts with."""
    if len(data) < 3:
        return None
    return LaneHeader(data[0], (data[1] << 8) | data[2])


def _shown_frame(frame: DecodedFrame) -> Optional[LaneHeader]:
    """The shown frame every decoded lane header of ``frame`` names; ``None`` when no
    lane decoded or the lanes of a torn capture name different frames."""
    shown = None
    for lane in DECODE_ORDER:
        result = frame.lane(lane)
        if result is None:
            continue
        header = _lane_header(result.data)
        if header is None:
            return None
        if shown is not None and shown != header:
            return None
        shown = header
    return shown


class ScanSession:
    """Decodes camera frames and reassembles the stream they carry.

    ``work_cost`` says what a part of a capture costs in milliseconds, given the
    milliseconds it took; by default the measured time.
    """

    def __init__(
        self,
        limits: Optional[ScanLimits] = None,
        work_cost: Optional[Callable[[Work, float], float]] = None,
    ) -> None:
        self._limits = limits if limits is not None else ScanLimits()
        self._work_cost = work_cost if work_cost is not None else _measured
        #: What the last turn try cost, in milliseconds.
        self._turn_cost_ms: Optional[float] = None
        self._assembler = StreamAssembler(self._limits.assembler)
        self._stats = ScanStats()
        self._started_ms: Optional[int] = None
        self._progress_ms = 0
        self._last_rank = 0
        self._last_pose: Optional[Tuple[DecodedFrame, int]] = None
        self._held: Deque[_Held] = deque()
        #: The turn lanes tried, oldest first: ``(header, sector)``.
        self._tried: Deque[Tuple[LaneHeader, int]] = deque()
        #: Tries in a row that read no turn lane (at most :data:`TURN_BACKOFF`).
        self._turn_misses = 0
        #: Captures that would try a turn lane still to pass over.
        self._turn_wait = 0
        #: The bands tried, oldest first: ``(shown frame, band)``.
        self._kana_tried: Deque[Tuple[LaneHeader, int]] = deque()
        #: Tries in a row that read no kana lane (at most :data:`KANA_BACKOFF`).
        self._kana_misses = 0
        #: Captures that would try a band still to pass over.
        self._kana_wait = 0

    @property
    def limits(self) -> ScanLimits:
        """The limits of this session."""
        return self._limits

    def stats(self) -> ScanStats:
        """Diagnostic counters."""
        return self._stats

    def progress(self) -> Progress:
        """Current progress."""
        return self._assembler.progress()

    def reset(self) -> None:
        """Drop all partial state, including the pose used for tracking, held captures, the
        turn lanes and bands tried and any back-off."""
        self._assembler.reset()
        self._started_ms = None
        self._last_rank = 0
        self._last_pose = None
        self._held.clear()
        self._forget_turns()

    def _forget_turns(self) -> None:
        """Forget the turn lanes and bands tried and any back-off."""
        self._tried.clear()
        self._turn_misses = 0
        self._turn_wait = 0
        self._kana_tried.clear()
        self._kana_misses = 0
        self._kana_wait = 0

    def push(self, image: Luma, now_ms: int) -> ScanOutcome:
        """Offer one camera luma plane captured at monotonic time ``now_ms``.

        While the last decoded pose is at most :data:`TRACK_WINDOW_MS` old the frame
        is first read by tracking that pose; a full decode runs when tracking fails. The
        bands are tried before the turn lanes, and a capture whose bands read a kana lane
        tries the turn lanes only with time to spare (see the module documentation).
        """
        start = self._started_ms
        if start is not None and (
            max(now_ms - self._progress_ms, 0) > self._limits.idle_timeout_ms
            or max(now_ms - start, 0) > self._limits.absolute_timeout_ms
        ):
            self.reset()
        self._stats = replace(self._stats, frames=self._stats.frames + 1)
        push_started = time.perf_counter()
        error: Optional[DecodeErrorKind] = None
        lanes = ""
        merged = ""
        turned = ""
        kana = ""
        frame: Optional[DecodedFrame] = None
        last_pose = self._last_pose
        followed = False
        if last_pose is not None and max(now_ms - last_pose[1], 0) <= TRACK_WINDOW_MS:
            frame = track_frame(image, last_pose[0], self._limits.decode)
            if frame is not None:
                followed = True
                self._stats = replace(self._stats, tracked=self._stats.tracked + 1)
        if frame is None:
            try:
                frame = decode_frame(image, self._limits.decode)
            except DecodeError as failure:
                error = failure.kind
        if frame is not None:
            if frame.inferred_corner is not None:
                self._stats = replace(self._stats, inferred=self._stats.inferred + 1)
            self._last_pose = (frame, now_ms)
            lanes = self._absorb(frame)
            spent = self._cost(Work(WorkKind.TRACK if followed else WorkKind.SEARCH), push_started)
            merges = self._stats.merges
            part_started = time.perf_counter()
            merged = self._merge(image, frame)
            if self._stats.merges > merges:
                spent += self._cost(Work(WorkKind.MERGE), part_started)
            kana_tries = self._stats.kana_tries
            part_started = time.perf_counter()
            kana = self._kana(image, frame)
            bands = self._stats.kana_tries - kana_tries
            if bands > 0:
                spent += self._cost(Work(WorkKind.BANDS, bands), part_started)
            # a capture whose bands read a lane tries the turn lanes only with time to spare
            # (see the module docs)
            turn_cost = self._turn_cost_ms if self._turn_cost_ms is not None else 2 * spent
            if not kana:
                turned = self._turn(image, frame)
            elif spent + turn_cost <= CAPTURE_BUDGET_MS:
                turn_tries = self._stats.turn_tries
                turned = self._turn(image, frame)
                if self._stats.turn_tries > turn_tries:
                    self._stats = replace(self._stats, spare_turns=self._stats.spare_turns + 1)
        if error not in (DecodeErrorKind.NO_FINDERS, DecodeErrorKind.UNSUPPORTED_IMAGE):
            self._stats = replace(self._stats, located=self._stats.located + 1)
        progress = self._assembler.progress()
        if progress.rank > self._last_rank or (
            progress.meta is not None and self._started_ms is None
        ):
            self._progress_ms = now_ms
            if self._started_ms is None:
                self._started_ms = now_ms
        self._last_rank = progress.rank
        return ScanOutcome(
            error=error,
            lanes=lanes,
            merged=merged,
            turned=turned,
            kana=kana,
            progress=progress,
            completed=self._assembler.take_completed(),
        )

    def push_plane(
        self, width: int, height: int, stride: int, plane: bytes, now_ms: int
    ) -> ScanOutcome:
        """Offer the luma (Y) plane of a camera frame, rows ``stride`` bytes apart.

        This is the camera-analyzer entry point: NV12/NV21/I420 frames carry
        their luma first, so pass that plane and its row stride directly.
        """
        return self.push(Luma.from_strided(width, height, stride, plane), now_ms)

    def _cost(self, work: Work, started: float) -> float:
        """What ``work`` cost, given that it started at ``started`` (:func:`time.perf_counter`)."""
        return self._work_cost(work, (time.perf_counter() - started) * 1000)

    def _absorb(self, frame: DecodedFrame) -> str:
        lanes = frame.lanes
        stats = self._stats
        self._stats = replace(
            stats,
            lane_d=stats.lane_d + (frame.d is not None),
            field_lanes=stats.field_lanes + sum(1 for f in frame.fields if f is not None),
            readable=stats.readable + (1 if lanes else 0),
        )
        previous_meta = self._assembler.progress().meta
        frame.feed(self._assembler)
        # Tags are only eight bits: held evidence, done flags and the turn lanes tried belong
        # to the full stream identity. Captures that preceded the first beacon remain useful.
        if previous_meta is not None and self._assembler.progress().meta != previous_meta:
            self._held.clear()
            self._forget_turns()
        return lanes

    def _turn(self, image: Luma, frame: DecodedFrame) -> str:
        """Tries the turn lanes on top of the field lanes this capture decoded, except those
        already tried for their shown frame, and offers the atoms of those it reads; after
        tries that read nothing it passes over the next captures that would try one (see
        the module documentation)."""
        skip = [True] * SECTORS
        headers: List[Optional[LaneHeader]] = [None] * SECTORS
        for sector, result in enumerate(frame.fields):
            header = None if result is None else _lane_header(result.data)
            if header is None:
                continue
            headers[sector] = header
            skip[sector] = (header, sector) in self._tried
        if all(skip):
            return ""
        if self._turn_wait > 0:
            self._turn_wait -= 1
            return ""
        tried = self._tried
        for sector, header in enumerate(headers):
            if not skip[sector] and header is not None:
                if len(tried) == TURN_MEMORY:
                    tried.popleft()
                tried.append((header, sector))
                self._stats = replace(self._stats, turn_tries=self._stats.turn_tries + 1)
        read_started = time.perf_counter()
        results = read_turns(image, frame, skip)
        self._turn_cost_ms = self._cost(Work(WorkKind.TURNS), read_started)
        turned = ""
        for sector, result in enumerate(results):
            header = headers[sector]
            if result is None or header is None:
                continue
            packet = turn_packet(header, sector, result.data)
            if packet is None:
                continue
            self._assembler.push_atoms(packet)
            self._stats = replace(self._stats, turn_lanes=self._stats.turn_lanes + 1)
            turned += FIELD_LANES[sector].letter
        if turned:
            self._turn_misses = 0
        else:
            self._turn_misses = min(self._turn_misses + 1, TURN_BACKOFF)
            self._turn_wait = self._turn_misses
        return turned

    def _kana(self, image: Luma, frame: DecodedFrame) -> str:
        """Tries the bands not yet tried for this capture's shown frame and offers the atoms
        of the kana lanes they read; after tries that read nothing it passes over the next
        captures that would try one (see the module documentation)."""
        header = _shown_frame(frame)
        if header is None:
            return ""
        tried = self._kana_tried
        skip = [(header, band) in tried for band in range(KANA_BANDS)]
        if all(skip):
            return ""
        if self._kana_wait > 0:
            self._kana_wait -= 1
            return ""
        for band in range(KANA_BANDS):
            if not skip[band]:
                if len(tried) == KANA_MEMORY:
                    tried.popleft()
                tried.append((header, band))
                self._stats = replace(self._stats, kana_tries=self._stats.kana_tries + 1)
        read = ""
        for lane, result in zip(KANA_LANES, read_kana(image, frame, header, skip)):
            if result is None:
                continue
            packet = kana_packet(result.header, lane, result.result.data)
            if packet is None:
                continue
            self._assembler.push_atoms(packet)
            self._stats = replace(self._stats, kana_lanes=self._stats.kana_lanes + 1)
            read += lane.value
        if read:
            self._kana_misses = 0
        else:
            self._kana_misses = min(self._kana_misses + 1, KANA_BACKOFF)
            self._kana_wait = self._kana_misses
        return read

    def _merge(self, image: Luma, frame: DecodedFrame) -> str:
        """Holds this capture's field evidence under its shown frame and, from the second
        capture on, merges the held captures of that frame."""
        shown = _shown_frame(frame)
        if shown is None:
            return ""
        held = next((h for h in self._held if h.shown == shown), None)
        if held is None:
            if len(self._held) == MERGE_WINDOW:
                self._held.popleft()
            held = _Held(shown)
            self._held.append(held)
        done = held.done
        for sector, result in enumerate(frame.fields):
            if result is not None:
                done[sector] = True
        if all(done):
            held.evidence.clear()
            return ""
        evidence = field_evidence(image, frame, self._limits.decode)
        if evidence is None:
            return ""
        if len(held.evidence) == MERGE_CAPTURES:
            held.evidence.popleft()
        held.evidence.append(evidence)
        if len(held.evidence) < 2:
            return ""
        self._stats = replace(self._stats, merges=self._stats.merges + 1)
        results = merge_fields(list(held.evidence), tuple(done))
        merged = ""
        for sector, (lane, result) in enumerate(zip(FIELD_LANES, results)):
            if result is None:
                continue
            packet = parse_atom_lane(lane, result.data)
            # a merge offers only atoms of the shown frame it was made for
            if packet is None or packet.header != shown:
                continue
            done[sector] = True
            self._assembler.push_atoms(packet)
            self._stats = replace(self._stats, merged_lanes=self._stats.merged_lanes + 1)
            merged += lane.letter
        if all(done):
            held.evidence.clear()
        return merged
