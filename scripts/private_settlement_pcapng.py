#!/usr/bin/env python3
"""Convert one bounded classic-pcap capture to canonical little-endian pcapng.

The AtomicPrivateSettlementV1 leakage campaign captures real loopback packets
with the host ``tcpdump``.  Some supported hosts emit classic pcap rather than
pcapng.  This converter preserves every captured packet and timestamp while
writing the exact pcapng container named by the release inventory.  It rejects
truncation, trailing partial records, symlinks, replacement races, and invalid
lengths instead of publishing a renamed or synthetic capture.
"""

from __future__ import annotations

import argparse
import os
import stat
import struct
import sys
from pathlib import Path
from typing import BinaryIO, Sequence

MAX_CAPTURE_BYTES = 2 * 1024 * 1024 * 1024
MAX_PACKET_BYTES = 16 * 1024 * 1024
PCAP_HEADER_BYTES = 24
PCAP_PACKET_HEADER_BYTES = 16
PCAPNG_SECTION_HEADER = struct.pack(
    "<IIIHHqI",
    0x0A0D0D0A,
    28,
    0x1A2B3C4D,
    1,
    0,
    -1,
    28,
)


class CaptureFormatError(ValueError):
    """Raised when a capture cannot be converted without guessing."""


def _read_exact(stream: BinaryIO, size: int, label: str) -> bytes:
    data = stream.read(size)
    if len(data) != size:
        raise CaptureFormatError(f"truncated {label}")
    return data


def _open_stable_source(path: Path) -> tuple[BinaryIO, os.stat_result]:
    before = path.lstat()
    if not stat.S_ISREG(before.st_mode) or stat.S_ISLNK(before.st_mode):
        raise CaptureFormatError("capture source must be a regular non-symlink file")
    if before.st_size < PCAP_HEADER_BYTES or before.st_size > MAX_CAPTURE_BYTES:
        raise CaptureFormatError("capture source size is outside the bounded range")
    flags = os.O_RDONLY
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    descriptor = os.open(path, flags)
    try:
        opened = os.fstat(descriptor)
        if (before.st_dev, before.st_ino, before.st_size) != (
            opened.st_dev,
            opened.st_ino,
            opened.st_size,
        ):
            raise CaptureFormatError("capture source changed while opening")
        return os.fdopen(descriptor, "rb", closefd=True), before
    except BaseException:
        os.close(descriptor)
        raise


def _open_exclusive_destination(path: Path) -> BinaryIO:
    parent = path.parent
    if not parent.is_dir() or parent.is_symlink():
        raise CaptureFormatError("capture destination parent must be a real directory")
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    descriptor = os.open(path, flags, 0o600)
    return os.fdopen(descriptor, "wb", closefd=True)


def _pcap_layout(magic: bytes) -> tuple[str, int]:
    layouts = {
        b"\xd4\xc3\xb2\xa1": ("<", 6),
        b"\xa1\xb2\xc3\xd4": (">", 6),
        b"\x4d\x3c\xb2\xa1": ("<", 9),
        b"\xa1\xb2\x3c\x4d": (">", 9),
    }
    try:
        return layouts[magic]
    except KeyError as error:
        raise CaptureFormatError("source is not a supported classic-pcap capture") from error


def _interface_description(link_type: int, snaplen: int, timestamp_power: int) -> bytes:
    if link_type > 0xFFFF:
        raise CaptureFormatError("pcap link type does not fit the pcapng interface field")
    # if_tsresol is explicit for both microsecond and nanosecond pcaps. Options
    # are four-byte aligned and terminated by opt_endofopt.
    options = struct.pack("<HHB3xHH", 9, 1, timestamp_power, 0, 0)
    total = 20 + len(options)
    return b"".join(
        [
            struct.pack("<IIHHI", 1, total, link_type, 0, snaplen),
            options,
            struct.pack("<I", total),
        ]
    )


def _enhanced_packet(
    timestamp: int,
    captured_length: int,
    original_length: int,
    packet: bytes,
) -> bytes:
    padding = (-captured_length) % 4
    total = 32 + captured_length + padding
    return b"".join(
        [
            struct.pack(
                "<IIIIIII",
                6,
                total,
                0,
                timestamp >> 32,
                timestamp & 0xFFFFFFFF,
                captured_length,
                original_length,
            ),
            packet,
            b"\0" * padding,
            struct.pack("<I", total),
        ]
    )


def convert_pcap_to_pcapng(source: Path, destination: Path) -> int:
    """Convert ``source`` to ``destination`` and return the packet count.

    ``destination`` must not exist. The completed file and its parent directory
    are fsynced before success is returned.
    """

    source = source.absolute()
    destination = destination.absolute()
    if source == destination:
        raise CaptureFormatError("source and destination must differ")
    source_stream, source_identity = _open_stable_source(source)
    output: BinaryIO | None = None
    destination_created = False
    completed = False
    try:
        output = _open_exclusive_destination(destination)
        destination_created = True
        header = _read_exact(source_stream, PCAP_HEADER_BYTES, "pcap global header")
        byte_order, timestamp_power = _pcap_layout(header[:4])
        major, minor, _zone, _sigfigs, snaplen, link_type = struct.unpack(
            f"{byte_order}HHiIII", header[4:]
        )
        if (major, minor) != (2, 4):
            raise CaptureFormatError("unsupported classic-pcap version")
        if snaplen == 0 or snaplen > MAX_PACKET_BYTES:
            raise CaptureFormatError("pcap snap length is outside the bounded range")
        output.write(PCAPNG_SECTION_HEADER)
        output.write(_interface_description(link_type, snaplen, timestamp_power))

        packets = 0
        while True:
            packet_header = source_stream.read(PCAP_PACKET_HEADER_BYTES)
            if not packet_header:
                break
            if len(packet_header) != PCAP_PACKET_HEADER_BYTES:
                raise CaptureFormatError("truncated pcap packet header")
            seconds, fractional, captured, original = struct.unpack(
                f"{byte_order}IIII", packet_header
            )
            resolution = 10**timestamp_power
            if fractional >= resolution:
                raise CaptureFormatError("pcap timestamp fraction exceeds its resolution")
            if captured > snaplen or original < captured:
                raise CaptureFormatError("pcap packet lengths are inconsistent")
            packet = _read_exact(source_stream, captured, "pcap packet payload")
            timestamp = seconds * resolution + fractional
            output.write(_enhanced_packet(timestamp, captured, original, packet))
            packets += 1

        output.flush()
        os.fsync(output.fileno())
        output.close()
        output = None
        parent_descriptor = os.open(destination.parent, os.O_RDONLY)
        try:
            os.fsync(parent_descriptor)
        finally:
            os.close(parent_descriptor)
        after = source.lstat()
        if (
            not stat.S_ISREG(after.st_mode)
            or stat.S_ISLNK(after.st_mode)
            or (
                source_identity.st_dev,
                source_identity.st_ino,
                source_identity.st_size,
                source_identity.st_mtime_ns,
                source_identity.st_ctime_ns,
            )
            != (
                after.st_dev,
                after.st_ino,
                after.st_size,
                after.st_mtime_ns,
                after.st_ctime_ns,
            )
        ):
            raise CaptureFormatError("capture source changed during conversion")
        completed = True
        return packets
    finally:
        source_stream.close()
        if output is not None:
            output.close()
        if destination_created and not completed:
            try:
                destination.unlink()
            except FileNotFoundError:
                pass


def parse_args(argv: Sequence[str]) -> argparse.Namespace:
    """Parse the standalone converter arguments."""

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("destination", type=Path)
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    """Run the converter CLI."""

    arguments = parse_args(sys.argv[1:] if argv is None else argv)
    try:
        packets = convert_pcap_to_pcapng(arguments.source, arguments.destination)
    except (CaptureFormatError, OSError) as error:
        print(f"private-settlement pcapng conversion failed: {error}", file=sys.stderr)
        return 1
    print(f"converted {packets} captured packets")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
