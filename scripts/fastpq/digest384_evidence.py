"""Validate the canonical six-lane primitive benchmark's measured work claims.

This checks report consistency, not device provenance, cryptographic soundness,
or complete-proof qualification. Historical scalar reports have no adapter.
"""
from __future__ import annotations

from typing import Any

try:
    from .benchmark_operations import DIGEST384_OPERATIONS, require_operation
except ImportError:  # Direct script invocation.
    from benchmark_operations import DIGEST384_OPERATIONS, require_operation

SCHEMA = "fastpq-digest384-primitive-benchmark-v1"
CATALOG = "iroha-privacy-exact12-v1"
PROTOCOL = "fastpq-state-transition-stark-v1"
ROLE = "fastpq:v1:preprocessing-trace"
_MAX_U64 = (1 << 64) - 1
_FIELDS = frozenset({
    "schema", "catalog", "protocol", "profile", "role", "phase", "level",
    "digest_lanes", "output_encoding", "frame_count", "input_field_bytes",
    "canonical_words", "sponge_permutations", "output_bytes", "cpu_reference_verified",
})
_GPU_FIELDS = frozenset({
    "backend", "warmup_invocations", "timed_invocations", "invocations", "dispatches",
    "frames", "canonical_words", "descriptor_words", "output_words", "max_batch_frames",
    "max_batch_words", "parity_checked_digests", "parity_checked_lanes",
})


def _count(value: Any, label: str, minimum: int = 1, maximum: int = _MAX_U64) -> int:
    if type(value) is not int or not minimum <= value <= maximum:
        raise ValueError(f"{label} must be an integer in {minimum}..{maximum}")
    return value


def _equal(actual: Any, expected: Any, label: str) -> None:
    if type(actual) is not type(expected) or actual != expected:
        raise ValueError(f"{label} must equal {expected!r}")


def _closed(value: Any, required: frozenset[str], optional: frozenset[str], label: str) -> dict:
    if not isinstance(value, dict) or not required <= value.keys() or value.keys() - required - optional:
        raise ValueError(f"{label} must contain exactly its canonical fields")
    return value


def _frame_base_words(phase: str) -> int:
    # The Rust frame emits tag/length plus seven-byte chunks and a terminated
    # remainder for every byte field, four u64 coordinate/lane fields, a
    # field-count tag/value, and one final terminator before even rate padding.
    fields = ("iroha:goldilocks-digest384:message-frame:v1", CATALOG, PROTOCOL,
              PROTOCOL, ROLE, phase)
    return sum(len(value.encode("utf-8")) // 7 + 3 for value in fields) + 4 * 4 + 2 + 1


def validate_digest384_operation(entry: dict, report: dict, *, flattened: bool = False) -> None:
    """Check one operation, its exact framing identity, and every invocation's parity counts.

    The maintained producers use bench_00, bench_01, ... for trace columns.
    Their generated shape and the fixed Merkle input lengths determine the
    exact reported field-byte and word counts. This is not input authentication.
    """
    name = require_operation(entry.get("operation"))
    if name not in DIGEST384_OPERATIONS:
        if "digest384" in entry or "gpu_payload_buffer_bytes" in entry:
            raise ValueError("six-lane evidence cannot label another operation")
        return
    if "estimated_gpu_transfer_bytes" in entry:
        raise ValueError("six-lane logical buffer bytes are not estimated_gpu_transfer_bytes")
    evidence = _closed(entry.get("digest384"), _FIELDS, frozenset({"gpu"}), "digest384")
    frames = _count(entry.get("columns"), "columns")
    length = _count(entry.get("input_len"), "input_len")
    input_bytes = _count(entry.get("input_bytes"), "input_bytes")
    words = _count(evidence.get("canonical_words"), "digest384.canonical_words")
    phase, level = ("column-leaf", 0) if name == "digest384_trace_columns" else ("binary-node", 1)
    for field, expected in {
        "schema": SCHEMA, "catalog": CATALOG, "protocol": PROTOCOL, "profile": PROTOCOL,
        "role": ROLE, "phase": phase, "level": level, "digest_lanes": 6,
        "output_encoding": "six-canonical-u64-le-words", "frame_count": frames,
        "input_field_bytes": input_bytes, "sponge_permutations": 3 * words,
        "output_bytes": 48 * frames, "cpu_reference_verified": True,
    }.items():
        _equal(evidence.get(field), expected, f"digest384.{field}")
    for field, expected in {
        "output_len": 6, "output_bytes": 48 * frames,
        "gpu_payload_buffer_bytes": 8 * words + 72 * frames,
    }.items():
        _count(entry.get(field), field)
        _equal(entry[field], expected, field)
    rows = _count(report.get("rows"), "rows", maximum=1 << 16)
    padded_rows = _count(report.get("padded_rows"), "padded_rows", maximum=1 << 16)
    _equal(padded_rows, 1 << (rows - 1).bit_length(), "padded_rows")
    column_count = _count(report.get("column_count"), "column_count")
    base = _frame_base_words(phase)
    if phase == "binary-node":
        _equal(frames, rows, "Merkle frame_count versus report rows")
        _equal(length, 12, "Merkle input_len")
        _equal(input_bytes, 96 * frames, "Merkle input_bytes")
        unpadded = base + 2 * (48 // 7 + 3)
        _equal(words, frames * (unpadded + unpadded % 2), "Merkle canonical_words")
    else:
        _equal(length, padded_rows, "trace input_len versus report padded_rows")
        _equal(frames, column_count, "trace frame_count versus report column_count")
        # Group generated names by decimal index width. This takes at most 20
        # steps for a u64 counter, avoiding allocation proportional to the claim.
        name_bytes = 0
        expected_words = 0
        first = 0
        end = 100
        digits = 2
        while first < frames:
            count = min(frames, end) - first
            name_length = 6 + digits
            name_bytes += count * name_length
            unpadded = base + 6 + name_length // 7 + (length * 8) // 7
            expected_words += count * (unpadded + unpadded % 2)
            first = end
            end *= 10
            digits += 1
        _equal(input_bytes, frames * length * 8 + name_bytes, "trace generated input_bytes")
        _equal(words, expected_words, "trace generated canonical_words")
    backend = report.get("gpu_backend")
    mode = report.get("execution_mode")
    _equal(report.get("gpu_available"), mode == "gpu", "gpu_available")
    if mode not in {"cpu", "gpu"} or backend not in ({"none"} if mode == "cpu" else {"metal", "cuda"}):
        raise ValueError("six-lane benchmark must declare an explicit CPU or device execution")
    has_gpu = entry.get("gpu_mean_ms") is not None if flattened else "gpu" in entry
    _equal(has_gpu, mode == "gpu", "six-lane GPU timing presence")
    if "gpu_recorded" in entry:
        _equal(entry["gpu_recorded"], has_gpu, "gpu_recorded")
    warmups = _count(report.get("warmups"), "warmups", minimum=0)
    iterations = _count(report.get("iterations"), "iterations")
    if not has_gpu:
        if "gpu" in evidence:
            raise ValueError("CPU six-lane benchmark must omit device evidence")
        return
    gpu = _closed(evidence.get("gpu"), _GPU_FIELDS, frozenset(), "digest384.gpu")
    invocations = warmups + iterations
    for field, expected in {
        "backend": backend, "warmup_invocations": warmups, "timed_invocations": iterations,
        "invocations": invocations, "frames": frames * invocations,
        "canonical_words": words * invocations, "descriptor_words": 3 * frames * invocations,
        "output_words": 6 * frames * invocations, "parity_checked_digests": frames * invocations,
        "parity_checked_lanes": 6 * frames * invocations,
    }.items():
        _equal(gpu.get(field), expected, f"digest384.gpu.{field}")
        if field != "backend":
            _count(gpu[field], f"digest384.gpu.{field}", minimum=0 if field == "warmup_invocations" else 1)
    dispatches = _count(gpu.get("dispatches"), "digest384.gpu.dispatches", minimum=invocations,
                        maximum=frames * invocations)
    maximum_frames = _count(gpu.get("max_batch_frames"), "digest384.gpu.max_batch_frames",
                            maximum=min(65_536, frames))
    maximum_words = _count(gpu.get("max_batch_words"), "digest384.gpu.max_batch_words",
                           maximum=min(4_194_304, words))
    if maximum_words % 2 or maximum_words < maximum_frames * 2:
        raise ValueError("device maximum batch cannot hold its complete even-padded frames")
    if maximum_frames * dispatches < gpu["frames"] or maximum_words * dispatches < gpu["canonical_words"]:
        raise ValueError("device dispatches do not cover the reported frames and words")
