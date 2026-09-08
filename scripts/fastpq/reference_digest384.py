#!/usr/bin/env python3
"""Independent FASTPQ Digest384 conformance oracle (Python 3.10+, stdlib only).

Uses hashlib's SHAKE256 and Python arbitrary-precision modular arithmetic;
does not call Rust, load native code, or reuse the optimized reducer. The
public parameter seed and MDS matrix define the protocol being reproduced.
No environment variables are required. By default this checks the pinned
repository fixture; only explicit --write rewrites the selected fixture.
This is implementation parity evidence, not a cryptographic security review.
"""

from __future__ import annotations

import argparse
import hashlib
import struct
from dataclasses import dataclass, replace
from functools import lru_cache
from pathlib import Path
from typing import Iterable

MODULUS = 2**64 - 2**32 + 1
LANES = 6
WIDTH = 3
RATE = 2
ROUNDS = 65
MAX_FIELD_BYTES = 2**32 - 1
PARAMETER_GENERATOR = b"shake256-rejection-sampling-u64le-below-goldilocks-v1"
PARAMETER_SEED = b"iroha:first-release:native-stark:goldilocks-digest384:2026-08-28:v1"
PARAMETER_DOMAIN = b"iroha:goldilocks-digest384:poseidon-x7:parameter-generator:v1"
ASSET_DOMAIN = b"iroha:goldilocks-digest384:parameter-asset:v1"
FRAME_DOMAIN = b"iroha:goldilocks-digest384:message-frame:v1"
PARAMETER_SHA3_256 = "84c5055b47cc7289835e0a5f31d4563849244ffddbf51f5d67b1db95222ce3e6"
MDS = (
    (0x982513A23D22B592, 0xA3115DB8CF1D9C90, 0x46BA684B9EEE84B7),
    (0xBE3DCE25491DB768, 0xFB0A6F731943519F, 0xFCE5BD953CDE1896),
    (0xE624719C41EB1A09, 0xD2221B0F1AA2EBC4, 0x1AB5E60D03AD44BC),
)
FIXTURE = (
    Path(__file__).resolve().parents[2]
    / "crates/fastpq_isi/src/assets/digest384_reference_v1.tsv"
)


@dataclass(frozen=True)
class Domain:
    """Complete typed digest context; all integer coordinates are unsigned u64."""

    catalog: bytes = b"iroha-privacy-exact12-v1"
    protocol: bytes = b"fastpq-state-transition-stark-v1"
    profile: bytes = b"fastpq-state-transition-stark-v1"
    role: bytes = b"fastpq:v1:air-trace"
    phase: bytes = b"leaf"
    level: int = 0
    index: int = 7
    counter: int = 0


def _u64(value: int) -> bytes:
    if type(value) is not int or not 0 <= value < 2**64:
        raise ValueError("coordinate must be an unsigned u64")
    return struct.pack("<Q", value)


@lru_cache(maxsize=LANES)
def lane_parameters(lane: int) -> tuple[tuple[int, ...], tuple[tuple[int, ...], ...]]:
    """Derive a lane's IV and constants using the standard-library SHAKE256."""
    if type(lane) is not int or not 0 <= lane < LANES:
        raise ValueError("lane must be in 0..6")
    seed = (
        PARAMETER_DOMAIN
        + _u64(len(PARAMETER_GENERATOR))
        + PARAMETER_GENERATOR
        + _u64(len(PARAMETER_SEED))
        + PARAMETER_SEED
        + _u64(lane)
    )
    shake = hashlib.shake_256(seed)
    required = WIDTH * (1 + ROUNDS)
    byte_count = required * 8
    while True:
        words = tuple(
            candidate
            for (candidate,) in struct.iter_unpack("<Q", shake.digest(byte_count))
            if candidate < MODULUS
        )
        if len(words) >= required:
            words = words[:required]
            return words[:WIDTH], tuple(
                words[offset : offset + WIDTH]
                for offset in range(WIDTH, required, WIDTH)
            )
        byte_count *= 2


def parameter_asset_digest() -> bytes:
    """Reproduce the SHA3 identity of every lane IV, constant, and MDS entry."""
    encoded = bytearray(ASSET_DOMAIN + PARAMETER_GENERATOR + PARAMETER_SEED)
    for lane in range(LANES):
        initial, constants = lane_parameters(lane)
        encoded.extend(_u64(lane))
        for word in (*initial, *(word for row in constants for word in row)):
            encoded.extend(_u64(word))
    for row in MDS:
        for word in row:
            encoded.extend(_u64(word))
    return hashlib.sha3_256(encoded).digest()


def _permutation(state: list[int], constants: tuple[tuple[int, ...], ...]) -> list[int]:
    for round_index, row in enumerate(constants):
        added = [(word + constant) % MODULUS for word, constant in zip(state, row)]
        active = WIDTH if round_index < 4 or round_index >= 61 else 1
        powered = [pow(word, 7, MODULUS) if i < active else word for i, word in enumerate(added)]
        state = [sum(weight * word for weight, word in zip(row, powered)) % MODULUS for row in MDS]
    return state


def _byte_field(tag: int, payload: bytes) -> list[int]:
    if not isinstance(payload, bytes) or len(payload) > MAX_FIELD_BYTES:
        raise ValueError("field must be bytes within the u32 framing ceiling")
    whole = len(payload) // 7 * 7
    return [tag, len(payload)] + [
        int.from_bytes(payload[offset : offset + 7], "little")
        for offset in range(0, whole, 7)
    ] + [int.from_bytes(payload[whole:] + b"\x01", "little")]


def digest(domain: Domain, fields: Iterable[bytes]) -> bytes:
    """Hash a typed frame with independent full-integer arithmetic in six lanes."""
    fields = tuple(fields)
    if len(fields) > MAX_FIELD_BYTES:
        raise ValueError("field count exceeds the u32 framing ceiling")
    domain_fields = (
        FRAME_DOMAIN, domain.catalog, domain.protocol, domain.profile, domain.role,
        domain.phase, _u64(domain.level), _u64(domain.index), _u64(domain.counter),
    )
    common_prefix = [
        word
        for tag, payload in enumerate(domain_fields, 1)
        for word in _byte_field(tag, payload)
    ]
    common_suffix = [11, len(fields)] + [
        word for tag, payload in enumerate(fields, 12) for word in _byte_field(tag, payload)
    ] + [1]
    outputs = []
    for lane in range(LANES):
        initial, constants = lane_parameters(lane)
        words = common_prefix + _byte_field(10, _u64(lane)) + common_suffix
        if len(words) % RATE:
            words.append(0)
        state = list(initial)
        for offset in range(0, len(words), RATE):
            state = [(state[i] + words[offset + i]) % MODULUS for i in range(RATE)] + [state[2]]
            state = _permutation(state, constants)
        outputs.append(state[0])
    return struct.pack("<6Q", *outputs)


def cases() -> list[tuple[str, Domain, tuple[bytes, ...]]]:
    """Fixed boundary and typed-domain cases shared with the Rust fixture reader."""
    domain = Domain()
    output = [
        ("no-fields", domain, ()),
        ("empty-field", domain, (b"",)),
        ("split-fields", domain, (b"abc", b"def")),
        ("joined-fields", domain, (b"abcdef",)),
        ("trailing-zero", domain, (b"abcdef\0",)),
    ]
    for length in (1, 6, 7, 8, 13, 14, 15, 16, 27, 28, 29, 55, 56, 57, 135, 136, 137):
        payload = bytes(((index * 73 + length * 19) ^ 0xA5) & 0xFF for index in range(length))
        output.append((f"length-{length}", domain, (b"", b"fixed-prefix", payload)))
    for field in ("catalog", "protocol", "profile", "role", "phase"):
        output.append((f"domain-{field}", replace(domain, **{field: b"changed\0"}), (b"abc",)))
    for field in ("level", "index", "counter"):
        output.append((f"domain-{field}", replace(domain, **{field: 2**64 - 1}), (b"abc",)))
    output.append(("empty-domain", Domain(b"", b"", b"", b"", b"", 0, 0, 0), (b"abc",)))
    return output


def fixture_text() -> str:
    """Emit deterministic TSV conformance data, not a protocol serialization format."""
    lines = [
        "# FASTPQ Digest384 independent Python bigint/hashlib reference v1",
        f"# parameter_sha3_256={parameter_asset_digest().hex()}",
        "# name\tcatalog_hex\tprotocol_hex\tprofile_hex\trole_hex\tphase_hex\tlevel\tindex\tcounter\tfields_hex_comma_or_dash\tdigest_hex",
    ]
    for name, domain, fields in cases():
        lines.append("\t".join((
            name, domain.catalog.hex(), domain.protocol.hex(), domain.profile.hex(),
            domain.role.hex(), domain.phase.hex(), str(domain.level), str(domain.index),
            str(domain.counter), ",".join(field.hex() for field in fields) if fields else "-",
            digest(domain, fields).hex(),
        )))
    return "\n".join(lines) + "\n"


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fixture", type=Path, default=FIXTURE, help="TSV fixture to check or explicitly rewrite")
    parser.add_argument("--write", action="store_true", help="explicitly regenerate the selected fixture")
    args = parser.parse_args(argv)
    if parameter_asset_digest().hex() != PARAMETER_SHA3_256:
        parser.error("independently generated parameter identity differs from the pinned digest")
    expected = fixture_text()
    if args.write:
        args.fixture.write_text(expected, encoding="ascii")
        print(f"wrote {len(cases())} independent Digest384 vectors to {args.fixture}")
        return 0
    if not args.fixture.is_file() or args.fixture.read_text(encoding="ascii") != expected:
        parser.error("Digest384 fixture differs from the independent reference")
    print(f"verified {len(cases())} independent Digest384 vectors and parameter identity")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
