"""Independent cross-consumer schema regressions using synthetic report records."""
from copy import deepcopy

import pytest

from scripts.fastpq import benchmark_operations as operations
from scripts.fastpq import geometry_matrix, report_projection, wrap_benchmark
from scripts.fastpq.tests.report_fixtures import add_synthetic_raw_copy, column_staging, complete_flat_report


def bundle(backend="cuda", *, gpu=True):
    schema = "cuda_nested" if backend == "cuda" else "metal_flat"
    report = complete_flat_report(backend if gpu else "none")
    return add_synthetic_raw_copy({"producer_schema": schema, "benchmarks": report})


@pytest.mark.parametrize("backend", ["metal", "cuda"])
@pytest.mark.parametrize("gpu", [True, False])
def test_all_raw_flat_baselines_remain_valid(backend, gpu):
    data = bundle(backend, gpu=gpu)
    assert report_projection.project_bundle(data)["report"]["operations"] == data["benchmarks"]["operations"]


@pytest.mark.parametrize("missing", [True, False])
def test_cuda_gpu_delta_requires_numeric_raw_and_flat_values(missing):
    data = bundle()
    speedup = data["report"]["operations"][0]["speedup"]
    if missing:
        speedup.pop("delta_ms")
    else:
        speedup["delta_ms"] = None
    data["benchmarks"]["operations"][0]["speedup_delta_ms"] = None
    with pytest.raises(ValueError, match="speedup_delta_ms"):
        report_projection.project_bundle(data)
    with pytest.raises(SystemExit, match="speedup.delta_ms"):
        wrap_benchmark.summarize_operations(data["report"], "cuda_nested")


@pytest.mark.parametrize("fields", [("gpu",), ("speedup",), ("gpu", "speedup")])
def test_cpu_cuda_rejects_each_explicit_null_raw_gpu_field(fields):
    data = bundle(gpu=False)
    for field in fields:
        data["report"]["operations"][0][field] = None
    with pytest.raises(ValueError):
        report_projection.project_bundle(data)


@pytest.mark.parametrize("backend", ["metal", "cuda"])
@pytest.mark.parametrize("operation", ["fft", "digest384_trace_columns"])
def test_cpu_invocation_overflow_rejects_before_any_device_requirement(backend, operation):
    data = bundle(backend, gpu=False)
    for report in (data["report"], data["benchmarks"]):
        report.update(warmups=(1 << 64)-1, iterations=1, operation_filter=operation)
        report["operations"] = [entry for entry in report["operations"] if entry["operation"] == operation]
    with pytest.raises(ValueError, match="invocation"):
        report_projection.project_bundle(data)
    assert operations.checked_invocations((1 << 64)-2, 1) == (1 << 64)-1


@pytest.mark.parametrize("backend", ["metal", "cuda"])
def test_negative_ratio_and_oversized_metrics_reject_but_negative_delta_is_valid(backend):
    data = bundle(backend)
    raw = data["report"]
    row = raw["operations"][0]
    row["speedup"]["ratio"] = -1
    with pytest.raises(ValueError, match="speedup.ratio"):
        report_projection.project_report(raw, flattened=False, producer_schema=data["producer_schema"])
    row["speedup"]["ratio"] = 1
    row["cpu"]["mean_ms"] = 10**1000
    with pytest.raises(ValueError, match="cpu.mean_ms"):
        report_projection.project_report(raw, flattened=False, producer_schema=data["producer_schema"])
    data = bundle(backend)
    for entry in data["benchmarks"]["operations"]:
        entry.update(cpu_mean_ms=1.0, gpu_mean_ms=2.0, speedup_ratio=0.5, speedup_delta_ms=-1.0)
    data.pop("report")
    add_synthetic_raw_copy(data)
    report_projection.project_bundle(data)


@pytest.mark.parametrize("location", ["aggregate", "phase", "sample"])
@pytest.mark.parametrize("field,value", [
    ("count", -1), ("count", 1 << 64), ("count", True), ("count", 1.0),
    ("flatten_ms", -1), ("wait_ms", float("inf")), ("wait_ratio", 1.1),
    ("wait_ratio", True), ("flatten_ms", 10**1000), ("wait_ms", "1"),
])
def test_staging_rejects_invalid_metrics_at_every_level(location, field, value):
    staging = column_staging()
    target = staging if location == "aggregate" else staging["phases"]["fft"] if location == "phase" else staging["samples"]["lde"][0]
    target[("batch" if location == "sample" else "batches") if field == "count" else field] = value
    with pytest.raises(ValueError, match="column_staging"):
        operations.validate_column_staging(staging)


@pytest.mark.parametrize("location", ["aggregate", "phase", "sample"])
def test_staging_requires_closed_fields_and_retains_valid_zero_boundaries(location):
    staging = column_staging()
    operations.validate_column_staging(staging)
    target = staging if location == "aggregate" else staging["phases"]["fft"] if location == "phase" else staging["samples"]["lde"][0]
    for field in tuple(target):
        mutated = deepcopy(staging)
        affected = mutated if location == "aggregate" else mutated["phases"]["fft"] if location == "phase" else mutated["samples"]["lde"][0]
        del affected[field]
        with pytest.raises(ValueError): operations.validate_column_staging(mutated)
    target["extra"] = 0
    with pytest.raises(ValueError): operations.validate_column_staging(staging)
    del target["extra"]
    for field in target:
        if field not in {"phases", "samples"}: target[field] = 0
    operations.validate_column_staging(staging)


@pytest.mark.parametrize("group,value", [("phases", None), ("samples", False)])
def test_projection_rejects_malformed_staging_even_with_exact_phase_names(group, value):
    data = bundle("metal", gpu=False)
    for report in (data["report"], data["benchmarks"]):
        report["column_staging"] = column_staging()
        report["column_staging"][group]["fft"] = value
    with pytest.raises(ValueError): report_projection.project_bundle(data)


def summary():
    report = bundle("metal")["benchmarks"]
    return {**report, "producer_schema": "metal_flat", "status": "ok",
            "operations": {entry["operation"]: entry for entry in report["operations"]}}


@pytest.mark.parametrize("field", ["producer_schema", "operation_filter", "rows", "padded_rows", "iterations", "warmups", "column_count", "execution_mode"])
def test_geometry_cannot_classify_missing_context_as_stable(field):
    report = summary()
    del report[field]
    report["classification"] = {"stable": True, "reasons": []}
    assert geometry_matrix.build_matrix_entries([report])[0]["classification"] == "unstable"


@pytest.mark.parametrize("field", ["cpu_mean_ms", "speedup_ratio", "speedup_delta_ms", "input_len", "columns", "operation"])
def test_geometry_cannot_classify_incomplete_generic_operation_as_stable(field):
    report = summary()
    del report["operations"]["fft"][field]
    assert geometry_matrix.classify_entry(report)[0] == "unstable"


def test_geometry_full_inventory_has_no_object_order_dependency_and_focus_is_not_complete():
    report = summary()
    report["operations"] = dict(reversed(tuple(report["operations"].items())))
    assert geometry_matrix.classify_entry(report) == ("stable", [])
    focused = {**report, "operation_filter": "fft", "operations": {"fft": report["operations"]["fft"]}}
    evidence = geometry_matrix.project_summary(focused)
    assert evidence["report"]["operation_filter"] == "fft"
    assert geometry_matrix.classify_entry(focused)[0] == "unstable"
    row = geometry_matrix.build_matrix_entries([focused])[0]
    assert row["operation"] == "fft"
    assert report_projection.validate_projection(row["operation_evidence"]) == evidence
    partial = {**report, "operations": focused["operations"]}
    assert geometry_matrix.classify_entry(partial)[0] == "unstable"
