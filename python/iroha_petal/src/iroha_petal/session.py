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
"""

from __future__ import annotations

from collections import deque
from dataclasses import dataclass, field, replace
from typing import Deque, List, Optional, Tuple

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
from .lanes import DECODE_ORDER, FIELD_LANES
from .layout import SECTORS
from .stream import (
    AssemblerLimits,
    Completed,
    LaneHeader,
    Progress,
    StreamAssembler,
    parse_atom_lane,
)

__all__ = [
    "TRACK_WINDOW_MS",
    "MERGE_WINDOW",
    "MERGE_CAPTURES",
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
    """Decodes camera frames and reassembles the stream they carry."""

    def __init__(self, limits: Optional[ScanLimits] = None) -> None:
        self._limits = limits if limits is not None else ScanLimits()
        self._assembler = StreamAssembler(self._limits.assembler)
        self._stats = ScanStats()
        self._started_ms: Optional[int] = None
        self._progress_ms = 0
        self._last_rank = 0
        self._last_pose: Optional[Tuple[DecodedFrame, int]] = None
        self._held: Deque[_Held] = deque()

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
        """Drop all partial state."""
        self._assembler.reset()
        self._started_ms = None
        self._last_rank = 0
        self._last_pose = None
        self._held.clear()

    def push(self, image: Luma, now_ms: int) -> ScanOutcome:
        """Offer one camera luma plane captured at monotonic time ``now_ms``.

        While the last decoded pose is at most :data:`TRACK_WINDOW_MS` old the frame
        is first read by tracking that pose; a full decode runs when tracking fails.
        """
        start = self._started_ms
        if start is not None and (
            max(now_ms - self._progress_ms, 0) > self._limits.idle_timeout_ms
            or max(now_ms - start, 0) > self._limits.absolute_timeout_ms
        ):
            self.reset()
        self._stats = replace(self._stats, frames=self._stats.frames + 1)
        error: Optional[DecodeErrorKind] = None
        lanes = ""
        merged = ""
        frame: Optional[DecodedFrame] = None
        last_pose = self._last_pose
        if last_pose is not None and max(now_ms - last_pose[1], 0) <= TRACK_WINDOW_MS:
            frame = track_frame(image, last_pose[0], self._limits.decode)
            if frame is not None:
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
            merged = self._merge(image, frame)
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
        # Tags are only eight bits: held evidence and done flags belong to the full stream
        # identity. Captures that preceded the first beacon remain useful.
        if previous_meta is not None and self._assembler.progress().meta != previous_meta:
            self._held.clear()
        return lanes

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
