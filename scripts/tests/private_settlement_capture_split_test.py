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


def null_fragmented_ipv6_udp(source_port: int, destination_port: int) -> bytes:
    packet = bytearray(null_ipv6_udp(source_port, destination_port))
    packet[4 + 6] = 44
    packet[4 + 4 : 4 + 6] = (16).to_bytes(2, "big")
    fragment = bytearray(8)
    fragment[0] = 17
    return bytes(packet[: 4 + 40] + fragment + packet[4 + 40 :])


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
            self.assertEqual(MODULE.derive_split_counts(output, groups), counts)
            explicit_packet_counts = MODULE.derive_split_packet_counts(output, groups)
            self.assertEqual(
                set(explicit_packet_counts),
                set(MODULE.PACKET_COUNT_FIELDS),
            )
            self.assertEqual(MODULE.legacy_split_counts(explicit_packet_counts), counts)
            for channel, filename in MODULE.OUTPUT_NAMES.items():
                self.assertEqual(pcapng_packet_count(output / filename), counts[channel])

            sanitized_path = output / MODULE.OUTPUT_NAMES["sanitized"]
            original_sanitized = sanitized_path.read_bytes()
            duplicate = MODULE.pcapng._enhanced_packet(
                8_000_000,
                len(packets[0]),
                len(packets[0]),
                packets[0],
            )
            with sanitized_path.open("ab") as stream:
                stream.write(duplicate)
            with self.assertRaisesRegex(MODULE.CaptureSplitError, "exact union"):
                MODULE.derive_split_counts(output, groups)
            sanitized_path.write_bytes(original_sanitized)

            torii_path = output / MODULE.OUTPUT_NAMES["torii"]
            original_torii = torii_path.read_bytes()
            truncated = MODULE.pcapng._enhanced_packet(
                8_500_000,
                len(packets[0]),
                len(packets[0]) + 1,
                packets[0],
            )
            with torii_path.open("ab") as stream:
                stream.write(truncated)
            with self.assertRaisesRegex(MODULE.CaptureSplitError, "lengths"):
                MODULE.derive_split_counts(output, groups)
            torii_path.write_bytes(original_torii)

            unrelated = MODULE.pcapng._enhanced_packet(
                9_000_000,
                len(packets[-1]),
                len(packets[-1]),
                packets[-1],
            )
            with torii_path.open("ab") as stream:
                stream.write(unrelated)
            with self.assertRaisesRegex(MODULE.CaptureSplitError, "outside"):
                MODULE.derive_split_counts(output, groups)

    def test_loopback_ipv6_udp_ports_are_decoded(self) -> None:
        packet = null_ipv6_udp(44_000, 8080)
        self.assertEqual(MODULE.packet_transport_ports(packet, 0), (44_000, 8080))

    def test_fragmented_ipv4_and_ipv6_fail_closed(self) -> None:
        ipv4 = bytearray(ethernet_ipv4_tcp(50_000, 8080))
        ipv4[14 + 6 : 14 + 8] = (0x2000).to_bytes(2, "big")
        with self.assertRaisesRegex(MODULE.CaptureSplitError, "fragmented IPv4"):
            MODULE.packet_transport_ports(bytes(ipv4), 1)
        ipv4[14 + 6 : 14 + 8] = (1).to_bytes(2, "big")
        with self.assertRaisesRegex(MODULE.CaptureSplitError, "fragmented IPv4"):
            MODULE.packet_transport_ports(bytes(ipv4), 1)
        with self.assertRaisesRegex(MODULE.CaptureSplitError, "fragmented IPv6"):
            MODULE.packet_transport_ports(null_fragmented_ipv6_udp(44_000, 8080), 0)

    def test_split_rejects_snaplen_truncation_and_statistic_mismatch(self) -> None:
        packets = [
            ethernet_ipv4_tcp(50_000, 8080),
            ethernet_ipv4_tcp(8080, 50_000),
            ethernet_ipv4_tcp(1337, 50_001),
            ethernet_ipv4_tcp(50_002, 1448),
        ]
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            groups = MODULE.load_port_manifest(self.write_manifest(root))
            truncated = bytearray(classic_pcap(packets))
            truncated[24 + 12 : 24 + 16] = (len(packets[0]) + 1).to_bytes(
                4, "little"
            )
            source = root / "truncated.pcap"
            source.write_bytes(truncated)
            output = root / "truncated-out"
            output.mkdir()
            with self.assertRaisesRegex(MODULE.CaptureSplitError, "lengths"):
                MODULE.split_capture(source, output, groups)
            self.assertEqual(list(output.iterdir()), [])

            source = root / "complete.pcap"
            source.write_bytes(classic_pcap(packets))
            output = root / "mismatch-out"
            output.mkdir()
            with self.assertRaisesRegex(MODULE.CaptureSplitError, "statistic differs"):
                MODULE.split_capture(
                    source,
                    output,
                    groups,
                    expected_source_packets=len(packets) + 1,
                )
            self.assertEqual(list(output.iterdir()), [])

    def test_bound_manifest_is_canonical_and_reproducible(self) -> None:
        groups = {
            "torii": (8080,),
            "public_p2p": (1337,),
            "restricted_p2p": (1448,),
        }
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest = root / "ports.json"
            canonical = MODULE.canonical_port_manifest_bytes(groups)
            manifest.write_bytes(canonical)
            document, loaded, binding = MODULE.load_bound_port_manifest(manifest)
            self.assertEqual(loaded, groups)
            self.assertEqual(
                document,
                MODULE.canonical_port_manifest_document(groups),
            )
            self.assertEqual(
                binding,
                MODULE.canonical_port_manifest_binding(loaded),
            )
            manifest.write_bytes(canonical + b"\n")
            with self.assertRaisesRegex(MODULE.CaptureSplitError, "not canonical"):
                MODULE.load_bound_port_manifest(manifest)

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
