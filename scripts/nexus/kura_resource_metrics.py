"""Strict exact parser for the first-release Kura resource Prometheus projection.

This parses a retained response; it does not authenticate an endpoint, sample
RSS, align clocks, or qualify a scaling trial. Those belong to the probe owner.
The source-owned projection has 23 fixed families and 125 success samples, or
exactly two unavailable samples. No previous observation is accepted as input.
"""
from __future__ import annotations

import hashlib
import io
import re
from dataclasses import dataclass
from enum import Enum
from typing import Literal, NoReturn

PREFIX = "iroha_kura_resource_"
MAX_EXACT_INTEGER = 1 << 53
MAX_RESPONSE_BYTES = 16 * 1024 * 1024
MAX_RESPONSE_LINES = 131_072
MAX_LINE_BYTES = 16 * 1024
MAX_METRIC_NAME_BYTES = 128
MAX_NUMERIC_TOKEN_BYTES = 128
MAX_LABEL_TOKEN_BYTES = 256
MAX_LABEL_VALUE_BYTES = 128
MAX_DECIMAL_EXPONENT = 1024
MAX_TARGET_ROWS = 125
REVIEWED_PROJECTION_SHA256 = "94cf69104620a7efaecbd860bd735cd88192e2dd30d6ed03c6aa9ac3141448c6"

FAMILIES = (
    "resident_canonical", "resident_transaction", "resident_merge", "resident_carrier",
    "resident_replica", "resident_verification", "resident_frontier", "resident_queue",
    "canonical_index", "canonical_hashes", "pipeline_index", "ownership_index",
    "certified_index", "execution_input_index", "execution_preflight_index",
    "application_receipt_index", "merge_bundle_index", "canonical_replica_index",
    "merge_carrier_record", "native_latest_record", "query_marker_records",
    "evidence_key_records", "storage_bytes",
)
USAGE_FIELDS = (
    "resident_associations", "persisted_entries", "index_bytes", "temporary_index_bytes", "storage_bytes",
)
SCALARS = (
    "available", "generation", "fault_count", *(field + "_sum" for field in USAGE_FIELDS),
    "represented_entries",
)
METRIC_NAMES = frozenset(PREFIX + name for name in (*SCALARS, *USAGE_FIELDS, "status"))
_NAME = re.compile(r"[A-Za-z_:][A-Za-z0-9_:]*")
_LABEL_NAME = re.compile(r"[A-Za-z_][A-Za-z0-9_]*")
_DECIMAL = re.compile(r"\+?(?:(\d+)(?:\.(\d*))?|\.(\d+))(?:[eE]([+-]?\d+))?", re.ASCII)


class ProjectionError(ValueError):
    """An invalid, ambiguous or over-bound resource response; no observation exists."""


class UnavailableReason(str, Enum):
    """Closed exporter failures, without paths or arbitrary diagnostic text."""

    OWNER_UNAVAILABLE = "owner_unavailable"
    UNREGISTERED = "unregistered"
    BUSY = "busy"
    INTERRUPTED = "interrupted"
    ARITHMETIC = "arithmetic"
    GENERATION_CHANGED = "generation_changed"
    OWNER_MISMATCH = "owner_mismatch"
    INVALID_INVENTORY = "invalid_inventory"
    NUMERIC_RANGE = "numeric_range"


@dataclass(frozen=True, slots=True)
class Usage:
    """Separate represented counts and actual logical byte units."""

    resident_associations: int
    persisted_entries: int
    index_bytes: int
    temporary_index_bytes: int
    storage_bytes: int


@dataclass(frozen=True, slots=True)
class FamilyUsage:
    """One member of the fixed complete family vector."""

    family: str
    usage: Usage


@dataclass(frozen=True, slots=True)
class AvailableObservation:
    """A complete exact projection from one response, not a trial qualification."""

    raw_sha256: str
    response_bytes: int
    generation: int
    fault_count: int
    components: tuple[FamilyUsage, ...]
    total: Usage
    represented_entries: int

    @property
    def available(self) -> Literal[True]:
        return True


@dataclass(frozen=True, slots=True)
class UnavailableObservation:
    """An explicit failed observation, with no count or generation fields."""

    raw_sha256: str
    response_bytes: int
    reason: UnavailableReason

    @property
    def available(self) -> Literal[False]:
        return False


Observation = AvailableObservation | UnavailableObservation


def _fail(line: int, reason: str) -> NoReturn:
    # Reasons are source constants. Do not echo untrusted lines, paths or labels.
    raise ProjectionError(f"resource projection line {line}: {reason}")


def _integer(token: str, line: int) -> int:
    if len(token.encode("utf-8")) > MAX_NUMERIC_TOKEN_BYTES:
        _fail(line, "numeric token exceeds bound")
    if token.startswith("-"):
        _fail(line, "negative numeric spelling")
    match = _DECIMAL.fullmatch(token)
    if match is None:
        _fail(line, "invalid finite decimal or extra sample fields")
    whole, fraction, leading_fraction, exponent = match.groups()
    fraction = fraction if fraction is not None else (leading_fraction or "")
    whole = whole or "0"
    exponent_value = int(exponent or "0")
    if abs(exponent_value) > MAX_DECIMAL_EXPONENT:
        _fail(line, "decimal exponent exceeds bound")
    digits = whole + fraction
    coefficient = int(digits)
    if coefficient == 0:
        return 0
    shift = exponent_value - len(fraction)
    if shift >= 0:
        # Do not allocate a huge power or decimal integer before checking range.
        if len(str(coefficient)) + shift > len(str(MAX_EXACT_INTEGER)):
            _fail(line, "integer exceeds exact export range")
        value = coefficient * 10 ** shift
    else:
        if -shift > len(digits):
            _fail(line, "nonintegral decimal")
        value, remainder = divmod(coefficient, 10 ** -shift)
        if remainder:
            _fail(line, "nonintegral decimal")
    if value > MAX_EXACT_INTEGER:
        _fail(line, "integer exceeds exact export range")
    return value


def _quoted(text: str, offset: int, line: int) -> tuple[str, int]:
    if offset >= len(text) or text[offset] != '"':
        _fail(line, "label value must be quoted")
    start = offset
    offset += 1
    value: list[str] = []
    while offset < len(text):
        if len(text[start:offset].encode("utf-8")) > MAX_LABEL_TOKEN_BYTES:
            _fail(line, "label token exceeds bound")
        char = text[offset]
        offset += 1
        if char == '"':
            if len(text[start:offset].encode("utf-8")) > MAX_LABEL_TOKEN_BYTES:
                _fail(line, "label token exceeds bound")
            decoded = "".join(value)
            if len(decoded.encode("utf-8")) > MAX_LABEL_VALUE_BYTES:
                _fail(line, "label value exceeds bound")
            return decoded, offset
        if char == "\\":
            if offset >= len(text):
                _fail(line, "unterminated label escape")
            escaped = text[offset]
            offset += 1
            if escaped not in ('\\', '"', 'n'):
                _fail(line, "invalid Prometheus label escape")
            value.append({'\\': '\\', '"': '"', 'n': '\n'}[escaped])
        elif ord(char) < 32 or ord(char) == 127:
            _fail(line, "control character in label")
        else:
            value.append(char)
    _fail(line, "unterminated quoted label")


def _labels(text: str, offset: int, line: int) -> tuple[dict[str, str], int]:
    labels: dict[str, str] = {}
    offset += 1  # opening brace
    while True:
        while offset < len(text) and text[offset] in " \t":
            offset += 1
        if offset < len(text) and text[offset] == "}":
            return labels, offset + 1
        match = _LABEL_NAME.match(text, offset)
        if match is None:
            _fail(line, "malformed label name")
        name = match.group()
        if len(name) > MAX_LABEL_TOKEN_BYTES:
            _fail(line, "label name exceeds bound")
        if name in labels:
            _fail(line, "duplicate label name")
        offset = match.end()
        while offset < len(text) and text[offset] in " \t":
            offset += 1
        if offset >= len(text) or text[offset] != "=":
            _fail(line, "missing label assignment")
        offset += 1
        while offset < len(text) and text[offset] in " \t":
            offset += 1
        value, offset = _quoted(text, offset, line)
        labels[name] = value
        if len(labels) > 1:
            _fail(line, "unexpected additional target label")
        while offset < len(text) and text[offset] in " \t":
            offset += 1
        if offset >= len(text):
            _fail(line, "unterminated labels")
        if text[offset] == "}":
            return labels, offset + 1
        if text[offset] != ",":
            _fail(line, "malformed label separator")
        offset += 1


def _target_sample(text: str, line: int) -> tuple[tuple[str, str | None], int]:
    match = _NAME.match(text)
    if match is None:
        _fail(line, "unknown target metric")
    name = match.group()
    if len(name) > MAX_METRIC_NAME_BYTES:
        _fail(line, "metric name exceeds bound")
    if name not in METRIC_NAMES:
        _fail(line, "unknown target metric")
    offset = match.end()
    labels: dict[str, str] = {}
    if offset < len(text) and text[offset] == "{":
        labels, offset = _labels(text, offset, line)
    if offset >= len(text) or text[offset] not in " \t":
        _fail(line, "missing sample value separator")
    token = text[offset:].strip(" \t")
    value = _integer(token, line)
    suffix = name.removeprefix(PREFIX)
    if suffix in USAGE_FIELDS:
        if set(labels) != {"family"} or labels["family"] not in FAMILIES:
            _fail(line, "unknown or missing fixed family label")
        label = labels["family"]
    elif suffix == "status":
        if set(labels) != {"reason"} or labels["reason"] not in {"available", *(reason.value for reason in UnavailableReason)}:
            _fail(line, "unknown or missing status reason")
        label = labels["reason"]
    else:
        if labels:
            _fail(line, "unexpected scalar labels")
        label = None
    return (suffix, label), value


def _metadata(text: str, line: int, seen: set[tuple[str, str]]) -> None:
    parts = re.split(r"[ \t]+", text[1:].strip(" \t"), maxsplit=2)
    if not parts:
        return
    if parts[0].startswith(PREFIX):
        _fail(line, "malformed target metadata")
    if len(parts) < 2 or not parts[1].startswith(PREFIX):
        return
    if parts[0] not in ("HELP", "TYPE"):
        _fail(line, "unsupported target metadata directive")
    if len(parts[1]) > MAX_METRIC_NAME_BYTES:
        _fail(line, "metadata metric name exceeds bound")
    if parts[1] not in METRIC_NAMES:
        _fail(line, "unknown target metadata name")
    if len(parts) != 3 or not parts[2]:
        _fail(line, "incomplete target metadata")
    identity = (parts[0], parts[1])
    if identity in seen:
        _fail(line, "duplicate target metadata")
    seen.add(identity)
    if parts[0] == "TYPE" and parts[2].strip(" \t") != "gauge":
        _fail(line, "target metric type must be gauge")


def parse_kura_resource_metrics(raw: bytes) -> Observation:
    """Parse one bounded response; raise ProjectionError on ambiguity or invalidity.

    The input must be bytes retained by the caller's bounded HTTP probe. All
    response framing/UTF-8 bounds apply even to unrelated lines. Unrelated metric
    semantics are ignored. Target samples may not carry timestamps or exemplars.
    Optional target HELP/TYPE metadata is checked, but cannot create a sample.
    """
    if type(raw) is not bytes:
        raise ProjectionError("resource projection requires immutable response bytes")
    if len(raw) > MAX_RESPONSE_BYTES:
        raise ProjectionError("resource projection response exceeds byte bound")
    samples: dict[tuple[str, str | None], int] = {}
    metadata: set[tuple[str, str]] = set()
    stream = io.BytesIO(raw)
    line = 0
    while chunk := stream.readline(MAX_LINE_BYTES + 2):
        line += 1
        if line > MAX_RESPONSE_LINES:
            _fail(line, "response exceeds line count bound")
        chunk = chunk.removesuffix(b"\n").removesuffix(b"\r")
        if len(chunk) > MAX_LINE_BYTES:
            _fail(line, "line exceeds byte bound")
        try:
            text = chunk.decode("utf-8", errors="strict").strip(" \t")
        except UnicodeDecodeError:
            _fail(line, "invalid UTF-8 response")
        if not text:
            continue
        if text.startswith("#"):
            _metadata(text, line, metadata)
            continue
        if not text.startswith(PREFIX):
            continue
        key, value = _target_sample(text, line)
        if key in samples:
            _fail(line, "duplicate target sample")
        samples[key] = value
        if len(samples) > MAX_TARGET_ROWS:
            _fail(line, "target sample count exceeds bound")
    digest = hashlib.sha256(raw).hexdigest()
    status = [(label, value) for (name, label), value in samples.items() if name == "status"]
    available = samples.get(("available", None))
    if available not in (0, 1) or len(status) != 1 or status[0][1] != 1:
        _fail(0, "missing or inconsistent availability/status discriminant")
    reason = status[0][0]
    if available == 0:
        if len(samples) != 2 or reason == "available":
            _fail(0, "unavailable observation contains stale values or contradictory status")
        return UnavailableObservation(digest, len(raw), UnavailableReason(reason))
    if reason != "available":
        _fail(0, "available observation has an unavailable reason")
    expected = {(name, None) for name in SCALARS}
    expected.add(("status", "available"))
    expected.update((field, family) for field in USAGE_FIELDS for family in FAMILIES)
    if set(samples) != expected or len(samples) != 125:
        _fail(0, "incomplete success vector or unexpected target samples")
    components = tuple(FamilyUsage(family, Usage(*(samples[(field, family)] for field in USAGE_FIELDS))) for family in FAMILIES)
    totals = tuple(samples[(field + "_sum", None)] for field in USAGE_FIELDS)
    for field, total in zip(USAGE_FIELDS, totals, strict=True):
        if sum(samples[(field, family)] for family in FAMILIES) != total:
            _fail(0, "inconsistent component subtotal")
    represented = samples[("represented_entries", None)]
    if totals[0] + totals[1] != represented:
        _fail(0, "inconsistent represented-entry reduction")
    return AvailableObservation(digest, len(raw), samples[("generation", None)],
        samples[("fault_count", None)], components, Usage(*totals), represented)
