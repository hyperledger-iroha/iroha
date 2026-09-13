"""Lossless operation evidence for current native benchmark report projections.

This validates schema consistency, not device provenance or proof qualification.
Raw producer reports and wrapped flattened reports keep their explicit format;
there is no scalar report adapter or synthesized six-lane measurement.
"""
from __future__ import annotations

from copy import deepcopy
import json
import math
from typing import Any

try:
    from .benchmark_operations import DIGEST384_OPERATIONS, reject_retired_fields, require_filter, require_operation
    from .digest384_evidence import validate_digest384_operation
    from . import wrap_benchmark
except ImportError:  # Direct script invocation.
    from benchmark_operations import DIGEST384_OPERATIONS, reject_retired_fields, require_filter, require_operation
    from digest384_evidence import validate_digest384_operation
    import wrap_benchmark

_REPORT_FIELDS = (
    "rows", "padded_rows", "iterations", "warmups", "column_count",
    "execution_mode", "gpu_backend", "gpu_available", "operation_filter", "column_staging",
)


def _flattened_operation(entry: dict[str, Any], report: dict[str, Any], schema: str) -> None:
    """Validate the exact timing/count fields emitted by the current wrapper."""
    if any(field in entry for field in ("cpu", "gpu", "speedup", "gpu_recorded")):
        raise ValueError("flattened benchmark cannot contain raw operation metrics")
    counts = {"input_len", "columns"}
    if schema == wrap_benchmark.CUDA_NESTED_SCHEMA or entry["operation"] in DIGEST384_OPERATIONS:
        counts.update(("output_len", "input_bytes", "output_bytes"))
        counts.add("gpu_payload_buffer_bytes" if entry["operation"] in DIGEST384_OPERATIONS else "estimated_gpu_transfer_bytes")
    for field in sorted(counts):
        value = entry.get(field)
        minimum = 1 if field == "columns" else 0
        if type(value) is not int or not minimum <= value <= (1 << 64) - 1:
            raise ValueError(f"flattened benchmark requires u64 integer {field} >= {minimum}")
    for field in ("cpu_mean_ms", "gpu_mean_ms", "speedup_ratio", "speedup_delta_ms"):
        if field not in entry:
            raise ValueError(f"flattened benchmark requires {field}")
        value = entry[field]
        if value is None and field != "cpu_mean_ms":
            continue
        try:
            finite = type(value) in (int, float) and math.isfinite(value)
        except OverflowError:
            finite = False
        if not finite or (field != "speedup_delta_ms" and value < 0):
            raise ValueError(f"flattened benchmark requires finite valid {field}")
    gpu = entry["gpu_mean_ms"]
    if (gpu is not None) != report["gpu_available"]:
        raise ValueError("flattened benchmark GPU timing disagrees with availability")
    if gpu is None:
        if entry["speedup_ratio"] is not None or entry["speedup_delta_ms"] is not None:
            raise ValueError("flattened benchmark without GPU timing cannot contain speedup")
    elif not report["gpu_available"] or entry["speedup_ratio"] is None:
        raise ValueError("flattened benchmark GPU timing disagrees with availability or speedup")
    elif schema == wrap_benchmark.METAL_FLAT_SCHEMA and entry["speedup_delta_ms"] is None:
        raise ValueError("flattened Metal benchmark requires speedup_delta_ms")


def project_report(report: dict[str, Any], *, flattened: bool, producer_schema: str) -> dict[str, Any]:
    """Validate exact operation IDs/evidence and retain full entries plus context."""
    if type(flattened) is not bool:
        raise ValueError("operation evidence flattened must be boolean")
    if not isinstance(report, dict):
        raise ValueError("benchmark report must be an object")
    reject_retired_fields(report)
    operation_filter = require_filter(report.get("operation_filter"))
    if producer_schema not in (wrap_benchmark.CUDA_NESTED_SCHEMA, wrap_benchmark.METAL_FLAT_SCHEMA):
        raise ValueError("benchmark requires an explicit canonical producer_schema")
    if "producer_schema" in report and report["producer_schema"] != producer_schema:
        raise ValueError("benchmark report producer_schema disagrees with envelope")
    report = {**report, "producer_schema": producer_schema}

    operations = report.get("operations")
    if not isinstance(operations, list) or not operations:
        raise ValueError("benchmark report requires a non-empty operations array")
    seen = set()
    for entry in operations:
        if not isinstance(entry, dict):
            raise ValueError("benchmark operation must be an object")
        name = require_operation(entry.get("operation"))
        if name in seen:
            raise ValueError(f"duplicate benchmark operation: {name}")
        seen.add(name)
        validate_digest384_operation(entry, report, flattened=flattened)
    if operation_filter != "all" and seen != {operation_filter}:
        raise ValueError("focused operation_filter disagrees with measured operations")
    try:
        wrap_benchmark.validate_report_header(report, producer_schema)
        if flattened:
            for entry in operations:
                _flattened_operation(entry, report, producer_schema)
        else:
            if any(any(field in entry for field in ("cpu_mean_ms", "gpu_mean_ms", "speedup_ratio", "speedup_delta_ms")) for entry in operations):
                raise ValueError("raw benchmark cannot contain flattened operation metrics")
            wrap_benchmark.summarize_operations(report, producer_schema)
    except SystemExit as error:
        raise ValueError(str(error)) from error
    result = {field: deepcopy(report[field]) for field in _REPORT_FIELDS if field in report}
    result["operations"] = deepcopy(operations)
    return {"flattened": flattened, "producer_schema": producer_schema, "report": result}


def project_bundle(bundle: dict[str, Any], *, require_wrapped: bool = False) -> dict[str, Any]:
    """Validate every report copy before projecting its explicitly represented format."""
    if not isinstance(bundle, dict):
        raise ValueError("benchmark bundle must be an object")
    reject_retired_fields(bundle)
    schema = bundle.get("producer_schema")
    if schema not in (wrap_benchmark.CUDA_NESTED_SCHEMA, wrap_benchmark.METAL_FLAT_SCHEMA):
        raise ValueError("benchmark requires an explicit canonical producer_schema")
    if "benchmarks" not in bundle:
        if require_wrapped or "report" in bundle or schema == wrap_benchmark.CUDA_NESTED_SCHEMA:
            raise ValueError("wrapped benchmark requires its benchmarks object")
        return project_report(bundle, flattened=False, producer_schema=schema)
    if "operations" in bundle:
        raise ValueError("wrapped benchmark operations belong in benchmarks or report")
    if "report" not in bundle:
        raise ValueError("wrapped benchmark requires its raw report copy")
    projection = project_report(bundle["benchmarks"], flattened=True, producer_schema=schema)
    if "report" in bundle:
        project_report(bundle["report"], flattened=False, producer_schema=schema)
        try:
            # This existing publication owner compares raw/flattened operation
            # entries, full six-lane evidence and execution counters exactly.
            wrap_benchmark.normalize_report(bundle)
        except SystemExit as error:
            raise ValueError(str(error)) from error
    return projection


def validate_projection(projection: dict[str, Any]) -> dict[str, Any]:
    """Revalidate projected evidence at each maintained rendering boundary."""
    if not isinstance(projection, dict) or set(projection) != {"flattened", "producer_schema", "report"}:
        raise ValueError("operation evidence must contain its exact format and report")
    if type(projection["flattened"]) is not bool:
        raise ValueError("operation evidence flattened must be boolean")
    return project_report(projection["report"], flattened=projection["flattened"], producer_schema=projection["producer_schema"])


def require_matching_report_claims(claims: dict[str, Any], projection: dict[str, Any]) -> None:
    """Reject any displayed or manifest claim that contradicts retained evidence."""
    evidence = validate_projection(projection)
    measured = {**evidence["report"], "producer_schema": evidence["producer_schema"]}
    for field in (*_REPORT_FIELDS, "producer_schema"):
        if field in claims and (field not in measured or not wrap_benchmark._same_json_value(claims[field], measured[field])):
            raise ValueError(f"benchmark {field} disagrees with retained measurement evidence")


def render_evidence(projection: dict[str, Any]) -> str:
    """Retain every operation field and validating counter in readable JSON."""
    validated = validate_projection(projection)
    return "```json\n" + json.dumps(validated, indent=2, sort_keys=True) + "\n```"
