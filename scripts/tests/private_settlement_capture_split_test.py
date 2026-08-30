"""Tests for real private-settlement packet capture channel splitting."""

from __future__ import annotations

import importlib.util
import json
import struct
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parents[1]
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))
SCRIPT = SCRIPT_DIR / "private_settlement_capture_split.py"
SPEC = importlib.util.spec_from_file_location(
    "private_settlement_capture_split", SCRIPT
)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


def ethernet_ipv4_tcp(source_port: int, destination_port: int) -> bytes:
    ethernet = bytes.fromhex("00112233445566778899aabb0800")
    ipv4 = bytearray(20)
    ipv4[0] = 0x45
    ipv4[2:4] = (40).to_bytes(2, "big")
    ipv4[8] = 64
    ipv4[9] = 6
    ipv4[12:16] = bytes((127, 0, 0, 1))
    ipv4[16:20] = bytes((127, 0, 0, 1))
    tcp = bytearray(20)
    tcp[:2] = source_port.to_bytes(2, "big")
    tcp[2:4] = destination_port.to_bytes(2, "big")
    tcp[12] = 5 << 4
    return ethernet + bytes(ipv4) + bytes(tcp)


def null_ipv6_udp(source_port: int, destination_port: int) -> bytes:
    ipv6 = bytearray(40)
    ipv6[0] = 0x60
    ipv6[4:6] = (8).to_bytes(2, "big")
    ipv6[6] = 17
    ipv6[7] = 64
    ipv6[23] = 1
    ipv6[39] = 1
    udp = bytearray(8)
    udp[:2] = source_port.to_bytes(2, "big")
    udp[2:4] = destination_port.to_bytes(2, "big")
    udp[4:6] = (8).to_bytes(2, "big")
    return (30).to_bytes(4, sys.byteorder) + bytes(ipv6) + bytes(udp)


def classic_pcap(packets: list[bytes], *, link_type: int = 1) -> bytes:
    rows = [
        b"\xd4\xc3\xb2\xa1",
        struct.pack("<HHiIII", 2, 4, 0, 0, 65_535, link_type),
    ]
    for index, packet in enumerate(packets, 1):
        rows.extend(
            (
                struct.pack("<IIII", index, index * 100, len(packet), len(packet)),
                packet,
            )
        )
    return b"".join(rows)


def pcapng_packet_count(path: Path) -> int:
    raw = path.read_bytes()
    offset = 0
    packets = 0
    while offset < len(raw):
        block_type, length = struct.unpack_from("<II", raw, offset)
        if length < 12 or offset + length > len(raw):
            raise AssertionError("invalid pcapng block")
        if struct.unpack_from("<I", raw, offset + length - 4)[0] != length:
            raise AssertionError("invalid pcapng block trailer")
        packets += block_type == 6
        offset += length
    return packets


class PrivateSettlementCaptureSplitTests(unittest.TestCase):
    def write_manifest(self, root: Path) -> Path:
        path = root / "ports.json"
        path.write_text(
            json.dumps(
                {
                    "version": 1,
                    "torii_ports": [8080],
                    "public_p2p_ports": [1337],
                    "restricted_p2p_ports": [1448],
                }
            ),
            encoding="utf-8",
        )
        return path

    def test_split_preserves_packets_in_exact_port_channels(self) -> None:
        packets = [
            ethernet_ipv4_tcp(50_000, 8080),
            ethernet_ipv4_tcp(8080, 50_000),
            ethernet_ipv4_tcp(1337, 50_001),
            ethernet_ipv4_tcp(50_002, 1448),
            ethernet_ipv4_tcp(50_003, 6553),
        ]
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "all.pcap"
            output = root / "out"
            output.mkdir()
            source.write_bytes(classic_pcap(packets))
            groups = MODULE.load_port_manifest(self.write_manifest(root))
            counts = MODULE.split_capture(source, output, groups)
            self.assertEqual(
                counts,
                {
                    "sanitized": 4,
                    "torii": 2,
                    "public_p2p": 1,
                    "restricted_p2p": 1,
                    "torii_requests": 1,
                    "torii_responses": 1,
                },
            )
            self.assertEqual(
                {path.name for path in output.iterdir()},
                set(MODULE.OUTPUT_NAMES.values()),
            )
            for channel, filename in MODULE.OUTPUT_NAMES.items():
                self.assertEqual(pcapng_packet_count(output / filename), counts[channel])

    def test_loopback_ipv6_udp_ports_are_decoded(self) -> None:
        packet = null_ipv6_udp(44_000, 8080)
        self.assertEqual(MODULE.packet_transport_ports(packet, 0), (44_000, 8080))

    def test_manifest_and_empty_channel_fail_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            duplicate = root / "ports.json"
            duplicate.write_text(
                '{"version":1,"version":1,"torii_ports":[8080],'
                '"public_p2p_ports":[1337],"restricted_p2p_ports":[1448]}',
                encoding="utf-8",
            )
            with self.assertRaisesRegex(MODULE.CaptureSplitError, "duplicate key"):
                MODULE.load_port_manifest(duplicate)

            duplicate.write_text(
                json.dumps(
                    {
                        "version": 1,
                        "torii_ports": [8080],
                        "public_p2p_ports": [8080],
                        "restricted_p2p_ports": [1448],
                    }
                ),
                encoding="utf-8",
            )
            with self.assertRaisesRegex(MODULE.CaptureSplitError, "disjoint"):
                MODULE.load_port_manifest(duplicate)

            source = root / "all.pcap"
            source.write_bytes(classic_pcap([ethernet_ipv4_tcp(50_000, 8080)]))
            output = root / "out"
            output.mkdir()
            with self.assertRaisesRegex(MODULE.CaptureSplitError, "empty"):
                MODULE.split_capture(
                    source,
                    output,
                    {
                        "torii": (8080,),
                        "public_p2p": (1337,),
                        "restricted_p2p": (1448,),
                    },
                )
            self.assertEqual(list(output.iterdir()), [])


if __name__ == "__main__":
    unittest.main()
