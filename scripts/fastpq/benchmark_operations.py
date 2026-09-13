"""Canonical first-release native benchmark operation names and labels.

The scalar Poseidon GPU primitive has separate arithmetic diagnostics. Its
retired trace-commitment report IDs cannot identify six-lane measurements.
"""
from __future__ import annotations

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
            staging = value["column_staging"]
            if not isinstance(staging, dict):
                raise ValueError("column_staging must contain current FFT/LDE telemetry")
            for group in ("phases", "samples"):
                phases = staging.get(group)
                if not isinstance(phases, dict) or set(phases) != {"fft", "lde"}:
                    raise ValueError(f"column_staging.{group} must contain exactly fft and lde")
        for child in value.values():
            reject_retired_fields(child)
    elif isinstance(value, list):
        for child in value:
            reject_retired_fields(child)
