#!/usr/bin/env python3
"""Split one real loopback pcap into bound APS leakage-channel pcapng files.

The real-process harness captures the loopback interface once with ``tcpdump``.
After the network has published its exact Torii and lane P2P ports, this tool
classifies each IPv4/IPv6 TCP or UDP packet and writes the four canonical
capture surfaces without reconstructing packet payloads or timestamps.
"""

from __future__ import annotations

import argparse
import json
import os
import struct
import sys
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any, BinaryIO

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import private_settlement_pcapng as pcapng

VERSION = 1
MAX_MANIFEST_BYTES = 1024 * 1024
OUTPUT_NAMES = {
    "sanitized": "sanitized-capture.pcapng",
    "torii": "torii.pcapng",
    "public_p2p": "public-p2p.pcapng",
    "restricted_p2p": "restricted-p2p.pcapng",
}
TORII_DIRECTION_COUNT_NAMES = ("torii_requests", "torii_responses")
PORT_MANIFEST_FIELDS = {
    "version",
    "torii_ports",
    "public_p2p_ports",
    "restricted_p2p_ports",
}


class CaptureSplitError(ValueError):
    """Raised when a capture or its port binding cannot be split exactly."""


def _strict_json_loads(raw: bytes, label: str) -> Any:
    def object_from_pairs(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
        result: dict[str, Any] = {}
        for key, value in pairs:
            if key in result:
                raise CaptureSplitError(f"{label} contains duplicate key {key!r}")
            result[key] = value
        return result

    def reject_constant(value: str) -> Any:
        raise CaptureSplitError(f"{label} contains non-JSON constant {value}")

    try:
        text = raw.decode("utf-8")
    except UnicodeError as error:
        raise CaptureSplitError(f"{label} is not UTF-8") from error
    try:
        return json.loads(
            text,
            object_pairs_hook=object_from_pairs,
            parse_constant=reject_constant,
        )
    except json.JSONDecodeError as error:
        raise CaptureSplitError(f"{label} is not valid JSON: {error}") from error


def _read_manifest(path: Path) -> bytes:
    try:
        before = path.lstat()
    except OSError as error:
        raise CaptureSplitError("port manifest is unavailable") from error
    if path.is_symlink() or not path.is_file() or before.st_size > MAX_MANIFEST_BYTES:
        raise CaptureSplitError("port manifest must be a bounded regular non-symlink file")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise CaptureSplitError("port manifest cannot be opened safely") from error
    try:
        opened = os.fstat(descriptor)
        if (before.st_dev, before.st_ino, before.st_size) != (
            opened.st_dev,
            opened.st_ino,
            opened.st_size,
        ):
            raise CaptureSplitError("port manifest changed while opening")
        with os.fdopen(descriptor, "rb", closefd=False) as stream:
            raw = stream.read(MAX_MANIFEST_BYTES + 1)
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    identity = ("st_dev", "st_ino", "st_size", "st_mtime_ns", "st_ctime_ns")
    if len(raw) != before.st_size or any(
        getattr(before, field) != getattr(after, field) for field in identity
    ):
        raise CaptureSplitError("port manifest changed while reading")
    return raw


def _ports(value: Any, label: str) -> tuple[int, ...]:
    if not isinstance(value, list) or not value:
        raise CaptureSplitError(f"{label} must be a non-empty list")
    if any(
        isinstance(port, bool)
        or not isinstance(port, int)
        or not 1 <= port <= 65_535
        for port in value
    ):
        raise CaptureSplitError(f"{label} contains an invalid port")
    ports = tuple(value)
    if ports != tuple(sorted(set(ports))):
        raise CaptureSplitError(f"{label} must be sorted and duplicate-free")
    return ports


def load_port_manifest(path: Path) -> dict[str, tuple[int, ...]]:
    """Load the exact per-run endpoint binding emitted by the Rust harness."""

    document = _strict_json_loads(_read_manifest(path), "port manifest")
    return validate_port_manifest(document)


def validate_port_manifest(document: Any) -> dict[str, tuple[int, ...]]:
    """Validate an in-memory copy of the exact capture port manifest."""

    if not isinstance(document, dict) or set(document) != PORT_MANIFEST_FIELDS:
        raise CaptureSplitError("port manifest field inventory is invalid")
    if document["version"] != VERSION:
        raise CaptureSplitError("port manifest version must be 1")
    groups = {
        "torii": _ports(document["torii_ports"], "torii_ports"),
        "public_p2p": _ports(document["public_p2p_ports"], "public_p2p_ports"),
        "restricted_p2p": _ports(
            document["restricted_p2p_ports"], "restricted_p2p_ports"
        ),
    }
    all_ports = [port for ports in groups.values() for port in ports]
    if len(all_ports) != len(set(all_ports)):
        raise CaptureSplitError("capture port groups must be pairwise disjoint")
    return groups


def _pcapng_packets(path: Path) -> tuple[int, list[bytes]]:
    """Read the canonical pcapng emitted by this module without guessing layouts."""

    stream, identity = pcapng._open_stable_source(path.absolute())
    try:
        section = pcapng._read_exact(
            stream, len(pcapng.PCAPNG_SECTION_HEADER), "pcapng section header"
        )
        if section != pcapng.PCAPNG_SECTION_HEADER:
            raise CaptureSplitError("capture lacks the canonical pcapng section header")
        header = pcapng._read_exact(stream, 8, "pcapng interface block header")
        block_type, block_length = struct.unpack("<II", header)
        if block_type != 1 or block_length < 20 or block_length % 4 != 0:
            raise CaptureSplitError("capture lacks one canonical interface block")
        interface_tail = pcapng._read_exact(
            stream, block_length - 8, "pcapng interface block"
        )
        if struct.unpack_from("<I", interface_tail, len(interface_tail) - 4)[0] != block_length:
            raise CaptureSplitError("pcapng interface block trailer is invalid")
        link_type = struct.unpack_from("<H", interface_tail, 0)[0]
        packets: list[bytes] = []
        while True:
            header = stream.read(8)
            if not header:
                break
            if len(header) != 8:
                raise CaptureSplitError("truncated pcapng block header")
            block_type, block_length = struct.unpack("<II", header)
            if block_type != 6 or block_length < 32 or block_length % 4 != 0:
                raise CaptureSplitError("capture contains a non-canonical packet block")
            tail = pcapng._read_exact(stream, block_length - 8, "pcapng packet block")
            if struct.unpack_from("<I", tail, len(tail) - 4)[0] != block_length:
                raise CaptureSplitError("pcapng packet block trailer is invalid")
            interface_id, _high, _low, captured, original = struct.unpack_from(
                "<IIIII", tail, 0
            )
            if (
                interface_id != 0
                or captured > pcapng.MAX_PACKET_BYTES
                or original < captured
                or block_length != 32 + captured + (-captured) % 4
            ):
                raise CaptureSplitError("pcapng packet block lengths are inconsistent")
            packets.append(tail[20 : 20 + captured])
        after = path.absolute().lstat()
        stable = ("st_dev", "st_ino", "st_size", "st_mtime_ns", "st_ctime_ns")
        if any(getattr(identity, field) != getattr(after, field) for field in stable):
            raise CaptureSplitError("capture changed while its packets were replayed")
        return link_type, packets
    finally:
        stream.close()


def derive_split_counts(
    output_dir: Path, groups: Mapping[str, Sequence[int]]
) -> dict[str, int]:
    """Independently replay final split files into their source-backed counts."""

    if set(groups) != {"torii", "public_p2p", "restricted_p2p"}:
        raise CaptureSplitError("capture count replay requires the exact port groups")
    normalized = {name: frozenset(ports) for name, ports in groups.items()}
    if any(not ports for ports in normalized.values()):
        raise CaptureSplitError("capture count replay port groups must not be empty")
    paths = {name: output_dir / filename for name, filename in OUTPUT_NAMES.items()}
    decoded = {name: _pcapng_packets(path) for name, path in paths.items()}
    counts = {name: len(packets) for name, (_link, packets) in decoded.items()}
    counts.update({name: 0 for name in TORII_DIRECTION_COUNT_NAMES})
    torii_link, torii_packets = decoded["torii"]
    for packet in torii_packets:
        ports = packet_transport_ports(packet, torii_link)
        if ports is None:
            raise CaptureSplitError("Torii capture contains a packet without transport ports")
        source_port, destination_port = ports
        if source_port not in normalized["torii"] and destination_port not in normalized["torii"]:
            raise CaptureSplitError("Torii capture contains a packet outside its port manifest")
        counts["torii_requests"] += destination_port in normalized["torii"]
        counts["torii_responses"] += source_port in normalized["torii"]
    for name in ("public_p2p", "restricted_p2p"):
        link_type, packets = decoded[name]
        for packet in packets:
            ports = packet_transport_ports(packet, link_type)
            if ports is None or not any(port in normalized[name] for port in ports):
                raise CaptureSplitError(
                    f"{name} capture contains a packet outside its port manifest"
                )
    if any(counts[name] == 0 for name in (*OUTPUT_NAMES, *TORII_DIRECTION_COUNT_NAMES)):
        raise CaptureSplitError(f"one or more replayed capture channels are empty: {counts}")
    return counts


def _network_offset(packet: bytes, link_type: int) -> int | None:
    if link_type == 0:  # DLT_NULL/DLT_LOOP on supported loopback captures.
        return 4 if len(packet) >= 5 and packet[4] >> 4 in (4, 6) else None
    if link_type == 1:  # Ethernet, optionally with one or more VLAN tags.
        if len(packet) < 14:
            return None
        offset = 14
        protocol = int.from_bytes(packet[12:14], "big")
        while protocol in (0x8100, 0x88A8, 0x9100):
            if len(packet) < offset + 4:
                return None
            protocol = int.from_bytes(packet[offset + 2 : offset + 4], "big")
            offset += 4
        return offset if protocol in (0x0800, 0x86DD) else None
    if link_type == 101:  # DLT_RAW.
        return 0 if packet and packet[0] >> 4 in (4, 6) else None
    if link_type == 113:  # Linux cooked capture v1.
        if len(packet) < 16:
            return None
        return 16 if int.from_bytes(packet[14:16], "big") in (0x0800, 0x86DD) else None
    if link_type == 276:  # Linux cooked capture v2.
        if len(packet) < 20:
            return None
        return 20 if int.from_bytes(packet[0:2], "big") in (0x0800, 0x86DD) else None
    return None


def _ipv6_transport_offset(packet: bytes, offset: int) -> tuple[int, int] | None:
    if len(packet) < offset + 40:
        return None
    next_header = packet[offset + 6]
    cursor = offset + 40
    for _ in range(16):
        if next_header in (6, 17):
            return next_header, cursor
        if next_header in (0, 43, 60):
            if len(packet) < cursor + 2:
                return None
            following = packet[cursor]
            length = (packet[cursor + 1] + 1) * 8
        elif next_header == 44:
            if len(packet) < cursor + 8:
                return None
            following = packet[cursor]
            fragment = int.from_bytes(packet[cursor + 2 : cursor + 4], "big")
            if fragment & 0xFFF8:
                return None
            length = 8
        elif next_header == 51:
            if len(packet) < cursor + 2:
                return None
            following = packet[cursor]
            length = (packet[cursor + 1] + 2) * 4
        else:
            return None
        if length <= 0 or len(packet) < cursor + length:
            return None
        next_header = following
        cursor += length
    return None


def packet_transport_ports(packet: bytes, link_type: int) -> tuple[int, int] | None:
    """Return TCP/UDP source and destination ports for a supported packet."""

    network = _network_offset(packet, link_type)
    if network is None or len(packet) <= network:
        return None
    version = packet[network] >> 4
    if version == 4:
        if len(packet) < network + 20:
            return None
        header_bytes = (packet[network] & 0x0F) * 4
        if header_bytes < 20 or len(packet) < network + header_bytes + 4:
            return None
        fragment = int.from_bytes(packet[network + 6 : network + 8], "big")
        if fragment & 0x1FFF:
            return None
        protocol = packet[network + 9]
        transport = network + header_bytes
    elif version == 6:
        resolved = _ipv6_transport_offset(packet, network)
        if resolved is None:
            return None
        protocol, transport = resolved
    else:
        return None
    if protocol not in (6, 17) or len(packet) < transport + 4:
        return None
    return (
        int.from_bytes(packet[transport : transport + 2], "big"),
        int.from_bytes(packet[transport + 2 : transport + 4], "big"),
    )


def _close_and_sync(outputs: Mapping[str, BinaryIO], parent: Path) -> None:
    for output in outputs.values():
        output.flush()
        os.fsync(output.fileno())
        output.close()
    parent_descriptor = os.open(parent, os.O_RDONLY)
    try:
        os.fsync(parent_descriptor)
    finally:
        os.close(parent_descriptor)


def split_capture(
    source: Path,
    output_dir: Path,
    groups: Mapping[str, Sequence[int]],
) -> dict[str, int]:
    """Split a pcap and return per-surface plus Torii-direction packet counts."""

    if set(groups) != {"torii", "public_p2p", "restricted_p2p"}:
        raise CaptureSplitError("capture split requires the exact three port groups")
    normalized = {name: frozenset(ports) for name, ports in groups.items()}
    if any(not ports for ports in normalized.values()):
        raise CaptureSplitError("capture split port groups must not be empty")
    if output_dir.is_symlink() or not output_dir.is_dir():
        raise CaptureSplitError("capture output directory must be a real directory")
    source_stream, source_identity = pcapng._open_stable_source(source.absolute())
    outputs: dict[str, BinaryIO] = {}
    paths = {name: output_dir / filename for name, filename in OUTPUT_NAMES.items()}
    completed = False
    try:
        header = pcapng._read_exact(
            source_stream, pcapng.PCAP_HEADER_BYTES, "pcap global header"
        )
        byte_order, timestamp_power = pcapng._pcap_layout(header[:4])
        major, minor, _zone, _sigfigs, snaplen, link_type = struct.unpack(
            f"{byte_order}HHiIII", header[4:]
        )
        if (major, minor) != (2, 4):
            raise CaptureSplitError("unsupported classic-pcap version")
        if snaplen == 0 or snaplen > pcapng.MAX_PACKET_BYTES:
            raise CaptureSplitError("pcap snap length is outside the bounded range")
        for name, path in paths.items():
            output = pcapng._open_exclusive_destination(path)
            outputs[name] = output
            output.write(pcapng.PCAPNG_SECTION_HEADER)
            output.write(
                pcapng._interface_description(link_type, snaplen, timestamp_power)
            )
        counts = {
            **{name: 0 for name in OUTPUT_NAMES},
            **{name: 0 for name in TORII_DIRECTION_COUNT_NAMES},
        }
        while True:
            packet_header = source_stream.read(pcapng.PCAP_PACKET_HEADER_BYTES)
            if not packet_header:
                break
            if len(packet_header) != pcapng.PCAP_PACKET_HEADER_BYTES:
                raise CaptureSplitError("truncated pcap packet header")
            seconds, fractional, captured, original = struct.unpack(
                f"{byte_order}IIII", packet_header
            )
            resolution = 10**timestamp_power
            if fractional >= resolution:
                raise CaptureSplitError("pcap timestamp fraction exceeds its resolution")
            if captured > snaplen or original < captured:
                raise CaptureSplitError("pcap packet lengths are inconsistent")
            packet = pcapng._read_exact(source_stream, captured, "pcap packet payload")
            encoded = pcapng._enhanced_packet(
                seconds * resolution + fractional,
                captured,
                original,
                packet,
            )
            selected: set[str] = set()
            ports = packet_transport_ports(packet, link_type)
            if ports is not None:
                source_port, destination_port = ports
                for name, allowed in normalized.items():
                    if source_port in allowed or destination_port in allowed:
                        selected.add(name)
                if destination_port in normalized["torii"]:
                    counts["torii_requests"] += 1
                if source_port in normalized["torii"]:
                    counts["torii_responses"] += 1
            if selected:
                # The sanitized capture is the exact union of the manifest-bound APS
                # channels.  Keeping unrelated loopback traffic would make the paired
                # secret-only experiment depend on arbitrary host activity.
                selected.add("sanitized")
            for name in selected:
                outputs[name].write(encoded)
                counts[name] += 1
        _close_and_sync(outputs, output_dir)
        outputs = {}
        after = source.absolute().lstat()
        identity = ("st_dev", "st_ino", "st_size", "st_mtime_ns", "st_ctime_ns")
        if any(
            getattr(source_identity, field) != getattr(after, field)
            for field in identity
        ):
            raise CaptureSplitError("capture source changed during split")
        if any(
            counts[name] == 0
            for name in (*OUTPUT_NAMES, *TORII_DIRECTION_COUNT_NAMES)
        ):
            raise CaptureSplitError(
                f"one or more capture channels are empty: {counts}"
            )
        completed = True
        return counts
    finally:
        source_stream.close()
        for output in outputs.values():
            output.close()
        if not completed:
            for path in paths.values():
                try:
                    path.unlink()
                except FileNotFoundError:
                    pass


def parse_args(argv: Sequence[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", required=True, type=Path)
    parser.add_argument("--port-manifest", required=True, type=Path)
    parser.add_argument("--output-dir", required=True, type=Path)
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    arguments = parse_args(sys.argv[1:] if argv is None else argv)
    try:
        groups = load_port_manifest(arguments.port_manifest)
        counts = split_capture(arguments.source, arguments.output_dir, groups)
    except (CaptureSplitError, pcapng.CaptureFormatError, OSError) as error:
        print(f"private-settlement capture split failed: {error}", file=sys.stderr)
        return 1
    print(json.dumps({"version": VERSION, "packet_counts": counts}, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
