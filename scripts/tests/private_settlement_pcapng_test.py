"""Tests for strict private-settlement capture conversion."""

from __future__ import annotations

import importlib.util
import struct
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).resolve().parents[1] / "private_settlement_pcapng.py"
SPEC = importlib.util.spec_from_file_location("private_settlement_pcapng", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


def classic_pcap(
    packet: bytes = b"atomic-private-settlement",
    *,
    byte_order: str = "<",
    nanoseconds: bool = False,
) -> bytes:
    """Build one valid single-packet classic-pcap fixture."""

    magic = {
        ("<", False): b"\xd4\xc3\xb2\xa1",
        (">", False): b"\xa1\xb2\xc3\xd4",
        ("<", True): b"\x4d\x3c\xb2\xa1",
        (">", True): b"\xa1\xb2\x3c\x4d",
    }[(byte_order, nanoseconds)]
    fraction = 123_456_789 if nanoseconds else 123_456
    return b"".join(
        [
            magic,
            struct.pack(f"{byte_order}HHiIII", 2, 4, 0, 0, 65535, 1),
            struct.pack(
                f"{byte_order}IIII",
                42,
                fraction,
                len(packet),
                len(packet),
            ),
            packet,
        ]
    )


def pcapng_blocks(data: bytes) -> list[tuple[int, bytes]]:
    """Decode little-endian block framing from a generated pcapng fixture."""

    blocks: list[tuple[int, bytes]] = []
    offset = 0
    while offset < len(data):
        block_type, length = struct.unpack_from("<II", data, offset)
        if length < 12 or offset + length > len(data):
            raise AssertionError("invalid generated pcapng block length")
        (trailing,) = struct.unpack_from("<I", data, offset + length - 4)
        if trailing != length:
            raise AssertionError("generated pcapng block trailer mismatch")
        blocks.append((block_type, data[offset : offset + length]))
        offset += length
    return blocks


class PrivateSettlementPcapngTests(unittest.TestCase):
    def convert(self, source_bytes: bytes) -> tuple[int, bytes]:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "capture.pcap"
            destination = root / "capture.pcapng"
            source.write_bytes(source_bytes)
            packets = MODULE.convert_pcap_to_pcapng(source, destination)
            return packets, destination.read_bytes()

    def test_little_endian_microsecond_capture_preserves_packet(self) -> None:
        packet = b"\x00\x01\x02private-settlement-packet"
        count, output = self.convert(classic_pcap(packet))
        self.assertEqual(count, 1)
        blocks = pcapng_blocks(output)
        self.assertEqual([row[0] for row in blocks], [0x0A0D0D0A, 1, 6])
        interface = blocks[1][1]
        self.assertEqual(interface[16:21], struct.pack("<HHB", 9, 1, 6))
        enhanced = blocks[2][1]
        captured = struct.unpack_from("<I", enhanced, 20)[0]
        self.assertEqual(captured, len(packet))
        self.assertEqual(enhanced[28 : 28 + captured], packet)

    def test_big_endian_nanosecond_capture_uses_explicit_resolution(self) -> None:
        packet = b"nanosecond-packet"
        count, output = self.convert(
            classic_pcap(packet, byte_order=">", nanoseconds=True)
        )
        self.assertEqual(count, 1)
        blocks = pcapng_blocks(output)
        self.assertEqual(blocks[1][1][16:21], struct.pack("<HHB", 9, 1, 9))
        timestamp_high, timestamp_low = struct.unpack_from("<II", blocks[2][1], 12)
        self.assertEqual(
            (timestamp_high << 32) | timestamp_low,
            42_123_456_789,
        )

    def test_truncation_and_inconsistent_lengths_fail_without_output(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "capture.pcap"
            destination = root / "capture.pcapng"
            source.write_bytes(classic_pcap()[:-1])
            with self.assertRaises(MODULE.CaptureFormatError):
                MODULE.convert_pcap_to_pcapng(source, destination)
            self.assertFalse(destination.exists())

            malformed = bytearray(classic_pcap(b"1234"))
            struct.pack_into("<I", malformed, 24 + 8, 70_000)
            source.write_bytes(malformed)
            with self.assertRaises(MODULE.CaptureFormatError):
                MODULE.convert_pcap_to_pcapng(source, destination)
            self.assertFalse(destination.exists())

    def test_destination_replacement_and_source_symlink_fail_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            real_source = root / "real.pcap"
            real_source.write_bytes(classic_pcap())
            linked_source = root / "linked.pcap"
            linked_source.symlink_to(real_source)
            destination = root / "capture.pcapng"
            destination.write_bytes(b"keep")
            with self.assertRaises(FileExistsError):
                MODULE.convert_pcap_to_pcapng(real_source, destination)
            self.assertEqual(destination.read_bytes(), b"keep")
            destination.unlink()
            with self.assertRaises(MODULE.CaptureFormatError):
                MODULE.convert_pcap_to_pcapng(linked_source, destination)
            self.assertFalse(destination.exists())


if __name__ == "__main__":
    unittest.main()
