"""Canonical first-release native benchmark operation names and labels.

The scalar Poseidon GPU primitive has separate arithmetic diagnostics. Its
retired trace-commitment report IDs cannot identify six-lane measurements.
"""
from __future__ import annotations

import math
from types import MappingProxyType

OPERATION_LABELS = MappingProxyType({
    "fft": "FFT",
    "ifft": "IFFT",
    "lde": "LDE",
    "digest384_trace_columns": "Six-lane trace columns",
    "digest384_merkle_pairs": "Six-lane Merkle pairs",
    "bn254_poseidon_words": "BN254 Poseidon words",
})
CANONICAL_OPERATION_ORDER = tuple(OPERATION_LABELS)
CANONICAL_OPERATIONS = frozenset(CANONICAL_OPERATION_ORDER)
DIGEST384_OPERATIONS = frozenset({"digest384_trace_columns", "digest384_merkle_pairs"})
CANONICAL_FILTERS = CANONICAL_OPERATIONS | {"all"}


def require_operation(value: object) -> str:
    """Require one exact operation ID, rejecting obsolete IDs and aliases."""
    if not isinstance(value, str) or value not in CANONICAL_OPERATIONS:
        raise ValueError(f"unknown native benchmark operation: {value!r}")
    return value


def require_filter(value: object) -> str:
    """Require an exact operation filter or the complete-operation filter."""
    if not isinstance(value, str) or value not in CANONICAL_FILTERS:
        raise ValueError(f"unknown native benchmark operation filter: {value!r}")
    return value


def operation_label(value: object) -> str:
    """Return a label only for an operation with defined measurement semantics."""
    return OPERATION_LABELS[require_operation(value)]


_RETIRED_FIELDS = frozenset({
    "poseidon_microbench", "poseidon_profiles", "scalar_lane", "speedup_vs_scalar",
    "default_mean_ms", "scalar_mean_ms",
})

def checked_invocations(warmups: object, iterations: object) -> int:
    """Require two u64 counts and a representable total before any mode-specific work."""
    maximum = (1 << 64) - 1
    for name, value, minimum in (("warmups", warmups, 0), ("iterations", iterations, 1)):
        if type(value) is not int or not minimum <= value <= maximum:
            raise ValueError(f"{name} must be a u64 integer >= {minimum}")
    total = warmups + iterations
    if total > maximum:
        raise ValueError("warmups + iterations exceeds u64 invocation count")
    return total


def validate_column_staging(staging: object) -> None:
    """Validate the complete closed FFT/LDE telemetry emitted by the native owner."""
    metrics = {"flatten_ms", "wait_ms", "wait_ratio"}

    def closed(value: object, fields: set[str], label: str) -> dict:
        if not isinstance(value, dict) or set(value) != fields:
            names = "fft and lde" if fields == {"fft", "lde"} else ", ".join(sorted(fields))
            raise ValueError(f"{label} must contain exactly {names}")
        return value

    def measured(value: dict, count: str, label: str) -> None:
        counter = value[count]
        if type(counter) is not int or not 0 <= counter <= (1 << 64) - 1:
            raise ValueError(f"{label}.{count} must be a u64 integer")
        for field in metrics:
            number = value[field]
            try:
                finite = type(number) in (int, float) and math.isfinite(number)
            except OverflowError:
                finite = False
            if not finite or number < 0 or (field == "wait_ratio" and number > 1):
                raise ValueError(f"{label}.{field} must be finite and nonnegative, with wait_ratio <= 1")

    closed(staging, metrics | {"batches", "phases", "samples"}, "column_staging")
    measured(staging, "batches", "column_staging")
    for group in ("phases", "samples"):
        closed(staging[group], {"fft", "lde"}, f"column_staging.{group}")
    for phase in ("fft", "lde"):
        label = f"column_staging.phases.{phase}"
        measured(closed(staging["phases"][phase], metrics | {"batches"}, label), "batches", label)
        samples = staging["samples"][phase]
        if not isinstance(samples, list):
            raise ValueError(f"column_staging.samples.{phase} must be an array")
        for index, sample in enumerate(samples):
            label = f"column_staging.samples.{phase}[{index}]"
            measured(closed(sample, metrics | {"batch"}, label), "batch", label)


def reject_retired_fields(value: object) -> None:
    """Reject retired scalar report fields even beside otherwise valid evidence."""
    if isinstance(value, dict):
        retired = _RETIRED_FIELDS.intersection(value)
        if retired:
            raise ValueError(f"retired native benchmark report field: {sorted(retired)[0]}")
        queue = value.get("metal_dispatch_queue")
        if isinstance(queue, dict) and {"poseidon", "poseidon_pipeline"}.intersection(queue):
            raise ValueError("retired native benchmark scalar dispatch queue")
        heuristics = value.get("metal_heuristics")
        if isinstance(heuristics, dict):
            batches = heuristics.get("batch_columns")
            if "poseidon_batch_multiplier" in heuristics or (
                isinstance(batches, dict) and "poseidon" in batches
            ):
                raise ValueError("retired native benchmark scalar scheduling geometry")
        if "column_staging" in value:
            validate_column_staging(value["column_staging"])
        for child in value.values():
            reject_retired_fields(child)
    elif isinstance(value, list):
        for child in value:
            reject_retired_fields(child)
