#!/usr/bin/env python3
"""
Check Norito fixture manifest alignment across SDK copies.

This helper compares the canonical Norito-RPC manifest with the SDK-local
copies (Android/Python/Swift by default), reports any drift, and can emit both
JSON and Markdown summaries for governance evidence.
"""

from __future__ import annotations

import argparse
import base64
import binascii
import datetime as dt
import hashlib
import json
import re
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Dict, Iterable, List, Mapping, MutableMapping, Optional, Sequence

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from norito_fixture_frame import (
    SIGNED_TRANSACTION_SCHEMA,
    TRANSACTION_PAYLOAD_SCHEMA,
    decode_canonical_norito_frame,
    iroha_hash_hex,
    signed_transaction_entrypoint_hash_hex,
    signed_transaction_payload,
)


DEFAULT_CANONICAL = Path("fixtures/norito_rpc/transaction_fixtures.manifest.json")
DEFAULT_TARGETS: Mapping[str, Path] = {
    "android": Path(
        "java/iroha_android/src/test/resources/transaction_fixtures.manifest.json"
    ),
    "python": Path(
        "python/iroha_python/tests/fixtures/transaction_fixtures.manifest.json"
    ),
    "swift": Path("IrohaSwift/Fixtures/transaction_fixtures.manifest.json"),
}
MANIFEST_ROOT_FIELDS = frozenset({"fixtures"})
MANIFEST_FIXTURE_FIELDS = frozenset(
    {
        "authority",
        "creation_time_ms",
        "encoded_file",
        "encoded_len",
        "name",
        "network_id",
        "nonce",
        "payload_base64",
        "payload_hash",
        "signed_base64",
        "signed_hash",
        "signed_len",
        "time_to_live_ms",
    }
)
NETWORK_ID_LITERAL = re.compile(r"hash:([0-9A-F]{64})#([0-9A-F]{4})")
TRANSACTION_PAYLOAD_FIELDS = (
    "domain",
    "authority",
    "creation_time_ms",
    "instructions",
    "time_to_live_ms",
    "nonce",
    "fee_payment",
    "admission_intent",
    "metadata",
    "attachments",
)


@dataclass(frozen=True)
class FixtureDigest:
    name: str
    encoded_file: str
    network_id: str
    authority: str
    payload_base64: str
    payload_hash: str
    signed_base64: str
    signed_hash: str
    encoded_len: int
    signed_len: int
    creation_time_ms: int
    time_to_live_ms: int
    nonce: Optional[int]


@dataclass(frozen=True)
class ManifestSnapshot:
    path: Path
    fingerprint: str
    fixtures: Mapping[str, FixtureDigest]
    age_hours: float


@dataclass(frozen=True)
class FixtureMismatch:
    name: str
    differences: Mapping[str, str]


@dataclass(frozen=True)
class AlignmentResult:
    label: str
    snapshot: ManifestSnapshot
    missing: Sequence[str]
    extra: Sequence[str]
    mismatched: Sequence[FixtureMismatch]

    @property
    def ok(self) -> bool:
        return not (self.missing or self.extra or self.mismatched)


def _fingerprint_manifest(payload: MutableMapping[str, object]) -> str:
    normalized = json.dumps(payload, sort_keys=True, separators=(",", ":")).encode(
        "utf-8"
    )
    return hashlib.blake2b(normalized, digest_size=16).hexdigest()


def _require_exact_fields(
    record: Mapping[str, object], expected: frozenset[str], context: str
) -> None:
    actual = set(record)
    missing = sorted(expected - actual)
    unexpected = sorted(actual - expected)
    if missing or unexpected:
        raise SystemExit(
            f"[error] {context} has invalid fields: "
            f"missing={missing}, unexpected={unexpected}"
        )


def _crc16_ccitt_false(payload: bytes) -> int:
    crc = 0xFFFF
    for byte in payload:
        crc ^= byte << 8
        for _ in range(8):
            crc = (
                ((crc << 1) ^ 0x1021) & 0xFFFF if crc & 0x8000 else (crc << 1) & 0xFFFF
            )
    return crc


def _require_network_id(value: str, context: str) -> str:
    matched = NETWORK_ID_LITERAL.fullmatch(value)
    if matched is None:
        raise SystemExit(f"[error] {context} is not a canonical network_id")
    body, checksum = matched.groups()
    expected_checksum = _crc16_ccitt_false(f"hash:{body}".encode("ascii"))
    if checksum != f"{expected_checksum:04X}" or bytes.fromhex(body)[-1] & 1 != 1:
        raise SystemExit(f"[error] {context} is not a canonical network_id")
    return value


def _fixture_digest(entry: Mapping[str, object]) -> FixtureDigest:
    _require_exact_fields(entry, MANIFEST_FIXTURE_FIELDS, "fixture manifest entry")

    def required_string(field: str) -> str:
        value = entry.get(field)
        if not isinstance(value, str) or not value:
            raise SystemExit(
                f"[error] malformed fixture entry field {field!r}: {entry}"
            )
        return value

    def required_nonnegative_int(field: str) -> int:
        value = entry.get(field)
        if not isinstance(value, int) or isinstance(value, bool) or value < 0:
            raise SystemExit(
                f"[error] malformed fixture entry field {field!r}: {entry}"
            )
        return value

    def required_positive_int(field: str) -> int:
        value = entry.get(field)
        if not isinstance(value, int) or isinstance(value, bool) or value <= 0:
            raise SystemExit(
                f"[error] malformed fixture entry field {field!r}: {entry}"
            )
        return value

    name = required_string("name")
    encoded_file = required_string("encoded_file")
    network_id = _require_network_id(required_string("network_id"), "network_id")
    authority = required_string("authority")
    payload_base64 = required_string("payload_base64")
    payload_hash = required_string("payload_hash")
    signed_base64 = required_string("signed_base64")
    signed_hash = required_string("signed_hash")
    encoded_len = required_nonnegative_int("encoded_len")
    signed_len = required_nonnegative_int("signed_len")
    creation_time_ms = required_nonnegative_int("creation_time_ms")
    time_to_live_ms = required_positive_int("time_to_live_ms")
    nonce = entry.get("nonce")
    if (
        not isinstance(nonce, (int, type(None)))
        or isinstance(nonce, bool)
        or (isinstance(nonce, int) and nonce < 0)
    ):
        raise SystemExit(f"[error] malformed fixture entry: {entry}")
    return FixtureDigest(
        name=name,
        encoded_file=encoded_file,
        network_id=network_id,
        authority=authority,
        payload_base64=payload_base64,
        payload_hash=payload_hash,
        signed_base64=signed_base64,
        signed_hash=signed_hash,
        encoded_len=encoded_len,
        signed_len=signed_len,
        creation_time_ms=creation_time_ms,
        time_to_live_ms=time_to_live_ms,
        nonce=nonce,
    )


def _decode_canonical_base64(value: str, context: str) -> bytes:
    try:
        decoded = base64.b64decode(value, validate=True)
    except (ValueError, binascii.Error) as exc:
        raise SystemExit(f"[error] invalid base64 for {context}: {exc}") from exc
    if base64.b64encode(decoded).decode("ascii") != value:
        raise SystemExit(f"[error] non-canonical base64 for {context}")
    return decoded


def _compact_length(value: int) -> bytes:
    output = bytearray()
    remaining = value
    while True:
        byte = remaining & 0x7F
        remaining >>= 7
        if remaining:
            byte |= 0x80
        output.append(byte)
        if not remaining:
            return bytes(output)


def _decode_compact_length(data: bytes, offset: int) -> tuple[int, int]:
    start = offset
    value = 0
    shift = 0
    while offset < len(data) and shift <= 63:
        byte = data[offset]
        offset += 1
        value |= (byte & 0x7F) << shift
        if byte & 0x80 == 0:
            if data[start:offset] != _compact_length(value):
                raise ValueError("non-canonical compact field length")
            return value, offset
        shift += 7
    raise ValueError("truncated or overflowing compact field length")


def _read_field(data: bytes, offset: int, context: str) -> tuple[bytes, int]:
    length, payload_offset = _decode_compact_length(data, offset)
    end = payload_offset + length
    if end > len(data):
        raise ValueError(f"truncated {context}")
    return data[payload_offset:end], end


def _read_exact_fields(
    data: bytes, names: Sequence[str], context: str
) -> Mapping[str, bytes]:
    """Read one exact ordered adaptive-Norito struct without fallback fields."""

    fields: Dict[str, bytes] = {}
    offset = 0
    for name in names:
        if offset == len(data):
            raise ValueError(f"{context} is missing required field {name}")
        value, offset = _read_field(data, offset, f"{context}.{name}")
        fields[name] = value
    if offset != len(data):
        raise ValueError(f"{context} has trailing or legacy fields")
    return fields


def _read_option(data: bytes, context: str, expected_size: Optional[int]) -> Optional[bytes]:
    """Read the one canonical adaptive-Norito Option representation."""

    if data == b"\x00":
        return None
    if not data or data[0] != 1:
        raise ValueError(f"{context} has an invalid Option tag")
    value, offset = _read_field(data, 1, f"{context}.value")
    if offset != len(data):
        raise ValueError(f"{context} has trailing Option bytes")
    if expected_size is not None and len(value) != expected_size:
        raise ValueError(f"{context} has the wrong fixed-width value")
    return value


def _validate_fee_payment(data: bytes, context: str) -> None:
    """Require the current fee-payer record, including its gas-limit field."""

    if len(data) < 4:
        raise ValueError(f"{context} has a truncated payer tag")
    payer = int.from_bytes(data[:4], "little")
    payment, offset = _read_field(data, 4, f"{context}.value")
    if offset != len(data):
        raise ValueError(f"{context} has trailing payer bytes")
    if payer == 0:
        fields = _read_exact_fields(
            payment,
            ("charge_limits", "gas_limit"),
            f"{context}.authority",
        )
    elif payer == 1:
        fields = _read_exact_fields(
            payment,
            ("program_id", "program_revision", "charge_limits", "gas_limit"),
            f"{context}.sponsor",
        )
        if len(fields["program_revision"]) != 8:
            raise ValueError(f"{context}.sponsor.program_revision must be u64")
    else:
        raise ValueError(f"{context} has an unknown payer tag {payer}")
    gas_limit = _read_option(fields["gas_limit"], f"{context}.gas_limit", 8)
    if gas_limit is not None and int.from_bytes(gas_limit, "little") == 0:
        raise ValueError(f"{context}.gas_limit must be non-zero")


def _transaction_payload_fields(data: bytes, context: str) -> Mapping[str, bytes]:
    """Decode the exact current TransactionPayload field sequence."""

    fields = _read_exact_fields(data, TRANSACTION_PAYLOAD_FIELDS, context)
    if len(fields["creation_time_ms"]) != 8:
        raise ValueError(f"{context}.creation_time_ms must be u64")
    executable = fields["instructions"]
    if len(executable) < 4 or int.from_bytes(executable[:4], "little") not in range(5):
        raise ValueError(f"{context}.instructions has an unknown executable tag")
    ttl = _read_option(fields["time_to_live_ms"], f"{context}.time_to_live_ms", 8)
    if ttl is None or int.from_bytes(ttl, "little") == 0:
        raise ValueError(f"{context}.time_to_live_ms must be signature-bound and non-zero")
    nonce = _read_option(fields["nonce"], f"{context}.nonce", 4)
    if nonce is not None and int.from_bytes(nonce, "little") == 0:
        raise ValueError(f"{context}.nonce must be non-zero when present")
    _validate_fee_payment(fields["fee_payment"], f"{context}.fee_payment")
    admission = fields["admission_intent"]
    if len(admission) != 4 or int.from_bytes(admission, "little") not in (0, 1):
        raise ValueError(f"{context}.admission_intent has an unknown tag")
    if len(fields["metadata"]) < 8:
        raise ValueError(f"{context}.metadata has a truncated entry count")
    _read_option(fields["attachments"], f"{context}.attachments", None)
    return fields


def _transaction_payload_network_id(data: bytes, context: str) -> bytes:
    domain = _transaction_payload_fields(data, context)["domain"]
    if len(domain) < 4:
        raise ValueError(f"{context} has a truncated transaction domain")
    tag = int.from_bytes(domain[:4], "little")
    if tag == 1:
        raise ValueError(f"{context} uses the genesis-only transaction domain")
    if tag != 0:
        raise ValueError(f"{context} has an unknown transaction domain tag {tag}")
    network_id, offset = _read_field(domain, 4, f"{context}.domain.network_id")
    if offset != len(domain) or len(network_id) != 32:
        raise ValueError(f"{context} has a malformed transaction network_id")
    return network_id


def _require_transaction_network_id(
    payload: bytes, network_id: str, context: str
) -> None:
    expected = bytes.fromhex(network_id[5:69])
    if _transaction_payload_network_id(payload, context) != expected:
        raise ValueError(f"{context} network_id does not match its manifest")


def load_manifest(path: Path) -> ManifestSnapshot:
    if not path.exists():
        raise SystemExit(f"[error] manifest not found: {path}")
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:  # pragma: no cover - defensive guard
        raise SystemExit(f"[error] failed to parse JSON from {path}: {exc}") from exc
    if not isinstance(payload, dict):
        raise SystemExit(f"[error] manifest {path} must contain a JSON object")
    _require_exact_fields(payload, MANIFEST_ROOT_FIELDS, f"manifest {path}")
    fixtures_raw = payload.get("fixtures")
    if not isinstance(fixtures_raw, list):
        raise SystemExit(f"[error] manifest {path} missing fixtures array")
    fixtures: Dict[str, FixtureDigest] = {}
    encoded_files: Dict[str, str] = {}
    payload_hashes: Dict[str, str] = {}
    signed_hashes: Dict[str, str] = {}
    payload_bytes: Dict[bytes, str] = {}
    signed_bytes: Dict[bytes, str] = {}
    for entry in fixtures_raw:
        if not isinstance(entry, dict):
            raise SystemExit(
                f"[error] fixture entry in {path} was not an object: {entry!r}"
            )
        digest = _fixture_digest(entry)
        if digest.name in fixtures:
            raise SystemExit(
                f"[error] duplicate fixture name {digest.name!r} in {path}"
            )
        if digest.encoded_file in encoded_files:
            raise SystemExit(
                f"[error] duplicate encoded_file {digest.encoded_file!r} in {path}: "
                f"{encoded_files[digest.encoded_file]!r} and {digest.name!r}"
            )
        if digest.payload_hash in payload_hashes:
            raise SystemExit(
                f"[error] duplicate payload_hash {digest.payload_hash!r} in {path}: "
                f"{payload_hashes[digest.payload_hash]!r} and {digest.name!r}"
            )
        if digest.signed_hash in signed_hashes:
            raise SystemExit(
                f"[error] duplicate signed_hash {digest.signed_hash!r} in {path}: "
                f"{signed_hashes[digest.signed_hash]!r} and {digest.name!r}"
            )
        payload_frame = _decode_canonical_base64(
            digest.payload_base64, f"{path}:{digest.name}.payload_base64"
        )
        signed_frame = _decode_canonical_base64(
            digest.signed_base64, f"{path}:{digest.name}.signed_base64"
        )
        try:
            decoded_payload = decode_canonical_norito_frame(
                payload_frame,
                f"{path}:{digest.name}.payload",
                expected_schema=TRANSACTION_PAYLOAD_SCHEMA,
            )
            decoded_signed = decode_canonical_norito_frame(
                signed_frame,
                f"{path}:{digest.name}.signed",
                expected_schema=SIGNED_TRANSACTION_SCHEMA,
            )
            embedded_payload = signed_transaction_payload(decoded_signed)
            if embedded_payload != decoded_payload:
                raise ValueError(
                    f"{path}:{digest.name} signed transaction contains a different payload"
                )
            _require_transaction_network_id(
                decoded_payload,
                digest.network_id,
                f"{path}:{digest.name}.payload",
            )
            _require_transaction_network_id(
                embedded_payload,
                digest.network_id,
                f"{path}:{digest.name}.signed_payload",
            )
        except ValueError as exc:
            raise SystemExit(f"[error] {exc}") from exc
        if len(payload_frame) != digest.encoded_len:
            raise SystemExit(
                f"[error] {path}:{digest.name} encoded_len mismatch: "
                f"manifest={digest.encoded_len} decoded={len(payload_frame)}"
            )
        if len(signed_frame) != digest.signed_len:
            raise SystemExit(
                f"[error] {path}:{digest.name} signed_len mismatch: "
                f"manifest={digest.signed_len} decoded={len(signed_frame)}"
            )
        computed_payload_hash = iroha_hash_hex(payload_frame)
        if computed_payload_hash != digest.payload_hash:
            raise SystemExit(
                f"[error] {path}:{digest.name} payload_hash mismatch: "
                f"manifest={digest.payload_hash} computed={computed_payload_hash}"
            )
        computed_signed_hash = signed_transaction_entrypoint_hash_hex(decoded_signed)
        if computed_signed_hash != digest.signed_hash:
            raise SystemExit(
                f"[error] {path}:{digest.name} signed_hash mismatch: "
                f"manifest={digest.signed_hash} computed={computed_signed_hash}"
            )
        if payload_frame in payload_bytes:
            raise SystemExit(
                f"[error] duplicate payload bytes in {path}: "
                f"{payload_bytes[payload_frame]!r} and {digest.name!r}"
            )
        if signed_frame in signed_bytes:
            raise SystemExit(
                f"[error] duplicate signed bytes in {path}: "
                f"{signed_bytes[signed_frame]!r} and {digest.name!r}"
            )
        fixtures[digest.name] = digest
        encoded_files[digest.encoded_file] = digest.name
        payload_hashes[digest.payload_hash] = digest.name
        signed_hashes[digest.signed_hash] = digest.name
        payload_bytes[payload_frame] = digest.name
        signed_bytes[signed_frame] = digest.name
    fingerprint = _fingerprint_manifest(payload)
    age_hours = (
        dt.datetime.now(dt.timezone.utc) - _stat_mtime(path)
    ).total_seconds() / 3600.0
    return ManifestSnapshot(
        path=path, fingerprint=fingerprint, fixtures=fixtures, age_hours=age_hours
    )


def _stat_mtime(path: Path) -> dt.datetime:
    stat = path.stat()
    return dt.datetime.fromtimestamp(stat.st_mtime, tz=dt.timezone.utc)


def compare_manifests(
    label: str, canonical: ManifestSnapshot, target: ManifestSnapshot
) -> AlignmentResult:
    missing = sorted(set(canonical.fixtures) - set(target.fixtures))
    extra = sorted(set(target.fixtures) - set(canonical.fixtures))
    mismatched: List[FixtureMismatch] = []
    for name, expected in canonical.fixtures.items():
        if name not in target.fixtures:
            continue
        found = target.fixtures[name]
        differences: Dict[str, str] = {}
        if expected.encoded_file != found.encoded_file:
            differences["encoded_file"] = (
                f"{found.encoded_file} != {expected.encoded_file}"
            )
        if expected.payload_base64 != found.payload_base64:
            differences["payload_base64"] = "decoded payload bytes differ"
        if expected.payload_hash != found.payload_hash:
            differences["payload_hash"] = (
                f"{found.payload_hash} != {expected.payload_hash}"
            )
        if expected.signed_hash != found.signed_hash:
            differences["signed_hash"] = (
                f"{found.signed_hash} != {expected.signed_hash}"
            )
        if expected.signed_base64 != found.signed_base64:
            differences["signed_base64"] = "decoded signed bytes differ"
        if expected.encoded_len != found.encoded_len:
            differences["encoded_len"] = (
                f"{found.encoded_len} != {expected.encoded_len}"
            )
        if expected.signed_len != found.signed_len:
            differences["signed_len"] = f"{found.signed_len} != {expected.signed_len}"
        if expected.creation_time_ms != found.creation_time_ms:
            differences["creation_time_ms"] = (
                f"{found.creation_time_ms} != {expected.creation_time_ms}"
            )
        if expected.network_id != found.network_id:
            differences["network_id"] = f"{found.network_id} != {expected.network_id}"
        if expected.authority != found.authority:
            differences["authority"] = f"{found.authority} != {expected.authority}"
        if expected.time_to_live_ms != found.time_to_live_ms:
            differences["time_to_live_ms"] = (
                f"{found.time_to_live_ms} != {expected.time_to_live_ms}"
            )
        if expected.nonce != found.nonce:
            differences["nonce"] = f"{found.nonce} != {expected.nonce}"
        if differences:
            mismatched.append(FixtureMismatch(name=name, differences=differences))
    return AlignmentResult(
        label=label,
        snapshot=target,
        missing=missing,
        extra=extra,
        mismatched=sorted(mismatched, key=lambda m: m.name),
    )


def build_alignment_report(
    canonical: ManifestSnapshot, targets: Mapping[str, ManifestSnapshot]
) -> Mapping[str, object]:
    results = [
        compare_manifests(label, canonical, snapshot)
        for label, snapshot in targets.items()
    ]
    return {
        "canonical": {
            "path": str(canonical.path),
            "fingerprint": canonical.fingerprint,
            "fixtures": len(canonical.fixtures),
            "age_hours": round(canonical.age_hours, 2),
        },
        "targets": [
            {
                "label": result.label,
                "path": str(result.snapshot.path),
                "fingerprint": result.snapshot.fingerprint,
                "fixtures": len(result.snapshot.fixtures),
                "age_hours": round(result.snapshot.age_hours, 2),
                "missing": result.missing,
                "extra": result.extra,
                "mismatched": [
                    {"name": mismatch.name, "differences": mismatch.differences}
                    for mismatch in result.mismatched
                ],
                "status": "ok" if result.ok else "drift",
            }
            for result in results
        ],
        "generated_at": dt.datetime.now(dt.timezone.utc).isoformat(),
    }


def render_markdown(report: Mapping[str, object]) -> str:
    canonical = report.get("canonical", {})
    targets = report.get("targets", [])
    lines = [
        "# Norito fixture alignment",
        "",
        f"- Canonical: `{canonical.get('path', '')}` (fixtures: {canonical.get('fixtures', '?')}, "
        f"fingerprint: `{canonical.get('fingerprint', '')}`, "
        f"age_hours: {canonical.get('age_hours', '?')})",
        "",
        "| SDK | Status | Missing | Extra | Mismatched | Age (h) | Fingerprint |",
        "|-----|--------|---------|-------|------------|---------|-------------|",
    ]
    for target in targets:
        status = target.get("status", "?")
        missing = target.get("missing", [])
        extra = target.get("extra", [])
        mismatched = target.get("mismatched", [])
        fingerprint = target.get("fingerprint", "")
        age = target.get("age_hours", "?")
        lines.append(
            f"| {target.get('label', '?')} | {status} | "
            f"{_fmt_list(missing)} | {_fmt_list(extra)} | {_fmt_mismatch(mismatched)} | {age} | `{fingerprint}` |"
        )
    return "\n".join(lines) + "\n"


def _fmt_list(entries: Iterable[str]) -> str:
    items = list(entries)
    if not items:
        return "—"
    return "<br>".join(items)


def _fmt_mismatch(entries: Iterable[Mapping[str, object]]) -> str:
    rows: List[str] = []
    for entry in entries:
        name = entry.get("name", "?")
        diffs = entry.get("differences", {})
        if isinstance(diffs, dict):
            joined = "; ".join(f"{k}: {v}" for k, v in diffs.items())
        else:
            joined = str(diffs)
        rows.append(f"{name} ({joined})")
    return "<br>".join(rows) if rows else "—"


def parse_target_overrides(raw_targets: Sequence[str]) -> Mapping[str, Path]:
    targets: Dict[str, Path] = {}
    for raw in raw_targets:
        if "=" not in raw:
            raise SystemExit(
                f"[error] expected --target entries in label=path form, got {raw!r}"
            )
        label, path = raw.split("=", 1)
        label = label.strip()
        resolved = Path(path.strip())
        if not label:
            raise SystemExit("[error] target label may not be empty")
        targets[label] = resolved
    return targets


def build_targets(
    defaults: Mapping[str, Path], overrides: Mapping[str, Path], drop_defaults: bool
) -> Mapping[str, Path]:
    if drop_defaults:
        return overrides
    merged = dict(defaults)
    merged.update(overrides)
    return merged


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Verify Norito fixture manifest alignment across SDK copies.",
        formatter_class=argparse.ArgumentDefaultsHelpFormatter,
    )
    parser.add_argument(
        "--canonical",
        type=Path,
        default=DEFAULT_CANONICAL,
        help="Path to the canonical Norito fixture manifest.",
    )
    parser.add_argument(
        "--target",
        action="append",
        default=[],
        help="SDK manifest in label=path form (e.g., android=java/.../transaction_fixtures.manifest.json).",
    )
    parser.add_argument(
        "--no-default-targets",
        action="store_true",
        help="Ignore built-in target paths and rely solely on --target values.",
    )
    parser.add_argument(
        "--json-out",
        type=Path,
        help="Write a machine-readable JSON report to the given path.",
    )
    parser.add_argument(
        "--markdown-out",
        type=Path,
        help="Write a Markdown summary to the given path instead of stdout.",
    )
    parser.add_argument(
        "--allow-drift",
        action="store_true",
        help="Exit with status 0 even when drift is detected.",
    )
    args = parser.parse_args(argv)

    override_targets = parse_target_overrides(args.target)
    target_paths = build_targets(
        DEFAULT_TARGETS, override_targets, args.no_default_targets
    )
    canonical_snapshot = load_manifest(args.canonical)
    target_snapshots = {
        label: load_manifest(path) for label, path in target_paths.items()
    }
    report = build_alignment_report(canonical_snapshot, target_snapshots)
    markdown = render_markdown(report)

    if args.json_out:
        args.json_out.parent.mkdir(parents=True, exist_ok=True)
        args.json_out.write_text(json.dumps(report, indent=2), encoding="utf-8")
    if args.markdown_out:
        args.markdown_out.parent.mkdir(parents=True, exist_ok=True)
        args.markdown_out.write_text(markdown, encoding="utf-8")
    if not args.json_out and not args.markdown_out:
        sys.stdout.write(markdown)

    has_drift = any(
        target.get("status") != "ok" for target in report.get("targets", [])
    )
    return 0 if (args.allow_drift or not has_drift) else 1


if __name__ == "__main__":  # pragma: no cover - CLI entrypoint
    raise SystemExit(main())
