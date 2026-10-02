# Copyright 2026 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

"""The receive-side object an app holds while its camera is open."""

from __future__ import annotations

from dataclasses import dataclass, field, replace
from typing import Optional

from .decode import DecodedFrame, DecodeError, DecodeErrorKind, DecodeOptions, decode_frame
from .image import Luma
from .stream import AssemblerLimits, Completed, Progress, StreamAssembler

__all__ = ["ScanLimits", "ScanStats", "ScanOutcome", "ScanSession"]


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
    #: Lane ``P`` successes.
    lane_p: int = 0
    #: Lane ``K`` successes.
    lane_k: int = 0
    #: Lane ``D`` successes.
    lane_d: int = 0


@dataclass(frozen=True)
class ScanOutcome:
    """The result of offering one camera frame."""

    #: Why the frame produced nothing, when it did not. ``NO_ORIENTATION``
    #: means a code was located but no lane could be read (too far, too blurry).
    error: Optional[DecodeErrorKind]
    #: Lanes that decoded, as letters from ``"PKD"``.
    lanes: str
    #: Receive progress after this frame.
    progress: Progress
    #: The finished payload, delivered exactly once.
    completed: Optional[Completed]


class ScanSession:
    """Decodes camera frames and reassembles the stream they carry."""

    def __init__(self, limits: Optional[ScanLimits] = None) -> None:
        self._limits = limits if limits is not None else ScanLimits()
        self._assembler = StreamAssembler(self._limits.assembler)
        self._stats = ScanStats()
        self._started_ms: Optional[int] = None
        self._progress_ms = 0
        self._last_rank = 0

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

    def push(self, image: Luma, now_ms: int) -> ScanOutcome:
        """Offer one camera luma plane captured at monotonic time ``now_ms``."""
        start = self._started_ms
        if start is not None and (
            max(now_ms - self._progress_ms, 0) > self._limits.idle_timeout_ms
            or max(now_ms - start, 0) > self._limits.absolute_timeout_ms
        ):
            self.reset()
        self._stats = replace(self._stats, frames=self._stats.frames + 1)
        error: Optional[DecodeErrorKind] = None
        lanes = ""
        try:
            frame = decode_frame(image, self._limits.decode)
        except DecodeError as failure:
            error = failure.kind
        else:
            lanes = self._absorb(frame)
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
            lane_p=stats.lane_p + (frame.p is not None),
            lane_k=stats.lane_k + (frame.k is not None),
            lane_d=stats.lane_d + (frame.d is not None),
            readable=stats.readable + (1 if lanes else 0),
        )
        frame.feed(self._assembler)
        return lanes
