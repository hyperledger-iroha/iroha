"""Synthetic report validation and canonical cross-device matrix aggregation."""
import json
from pathlib import Path
import subprocess
import sys

import pytest

from scripts.fastpq import capture_matrix
from scripts.fastpq.benchmark_operations import CANONICAL_FILTER_ORDER, ordered_filters, require_filter_array
from scripts.fastpq.tests.report_fixtures import add_synthetic_raw_copy, complete_flat_report, flat_report
from scripts.fastpq.tests.test_report_projection import bundle as digest_bundle


def report_bundle(operation="fft", backend="cuda", *, rows=8, cpu=2.0, gpu=1.0):
    report = (complete_flat_report(backend, rows=rows) if operation == "all" else
              flat_report((operation,), backend, rows=rows, cpu=cpu, gpu=gpu))
    return add_synthetic_raw_copy({
        "producer_schema": "cuda_nested" if backend == "cuda" else "metal_flat",
        "metadata": {"platform": "linux", "machine": "x86_64"},
        "benchmarks": report,
    })


def write_bundle(tmp_path, data, name="capture.json"):
    path = tmp_path / name
    path.write_text(json.dumps(data), encoding="utf-8")
    return path


def test_format_operation_filter_prefers_explicit_value():
    benchmarks = {"operation_filter": "lde", "operations": [{"operation": "lde"}]}
    assert capture_matrix.format_operation_filter(benchmarks) == "lde"


@pytest.mark.parametrize("value", [None, "", "ALL", " fft", "poseidon_hash_columns", "digest384-columns", 1])
def test_format_operation_filter_rejects_missing_or_retired_inference(value):
    benchmarks = {"operations": [{"operation": "fft"}, {"operation": "lde"}]}
    if value is not None:
        benchmarks["operation_filter"] = value
    with pytest.raises(ValueError, match="operation filter"):
        capture_matrix.format_operation_filter(benchmarks)


def test_summarize_device_records_operation_filters_in_manifest(tmp_path):
    fft_bundle = report_bundle("fft", rows=8, cpu=2.0, gpu=1.0)
    lde_bundle = report_bundle("lde", rows=16, cpu=4.5, gpu=3.0)
    fft_path = write_bundle(tmp_path, fft_bundle, "fft.json")
    lde_path = write_bundle(tmp_path, lde_bundle, "lde.json")
    summary = capture_matrix.summarize_device("xeon-rtx", [fft_path, lde_path])
    entry = summary.to_manifest_entry(tmp_path, max_ms_slack=5.0, min_speedup_slack=5.0)
    assert summary.operation_filters == {"fft", "lde"}
    assert entry["operation_filters"] == ["fft", "lde"]
    assert entry["rows"] == {"min": 8, "max": 16, "padded": [8, 16]}
    assert entry["backend"] == "cuda"
    assert entry["platforms"] == ["linux"]
    assert entry["machines"] == ["x86_64"]
    assert entry["operations"]["fft"] == {"sample_count": 1, "median_gpu_mean_ms": 1.0, "median_speedup_ratio": 2.0}
    assert entry["operations"]["lde"] == {"sample_count": 1, "median_gpu_mean_ms": 3.0, "median_speedup_ratio": 1.5}
    assert entry["max_operation_ms"] == {"fft": 1.05, "lde": 3.15}
    assert entry["min_operation_speedup"] == {"fft": 1.9, "lde": 1.425}


@pytest.mark.parametrize("backend", ["metal", "cuda"])
def test_complete_and_focused_captures_emit_canonical_filter_order(tmp_path, backend):
    paths = []
    for index, name in enumerate(reversed(CANONICAL_FILTER_ORDER)):
        if name.startswith("digest384"):
            data = digest_bundle(name, backend)
        else:
            data = report_bundle(name, backend)
        paths.append(write_bundle(tmp_path, data, f"{index}.json"))
    entry = capture_matrix.summarize_device("device", paths).to_manifest_entry(tmp_path, 0, 0)
    assert entry["operation_filters"] == list(CANONICAL_FILTER_ORDER)
    assert len(entry["operations"]) == 6
    assert all(row["sample_count"] == 2 for row in entry["operations"].values())
    assert require_filter_array(entry["operation_filters"]) == entry["operation_filters"]


@pytest.mark.parametrize("filters", [[], ["lde", "fft"], ["fft", "fft"], ["fft", "all"], ["ALL"], ["poseidon_merkle_pairs"], ["digest384_merkle_pairs", "digest384_trace_columns"], "fft", [True]])
def test_encoded_filter_arrays_reject_empty_duplicate_noncanonical_or_retired(filters):
    with pytest.raises(ValueError):
        require_filter_array(filters)


def test_writer_normalizes_only_valid_filter_sets():
    assert ordered_filters({"lde", "all", "fft", "digest384_merkle_pairs", "digest384_trace_columns"}) == ["all", "fft", "lde", "digest384_trace_columns", "digest384_merkle_pairs"]
    for values in (set(), {"fft", "unknown"}):
        with pytest.raises(ValueError):
            ordered_filters(values)


@pytest.mark.parametrize("mutation", ["missing_filter", "focused_mismatch", "partial_all", "alias", "missing_raw", "missing_tag", "missing_nullable", "raw_timing_mismatch", "float_rows", "bad_padding", "overflow_invocations", "flat_raw_metric", "retired_queue", "partial_parity"])
def test_invalid_reports_reject_before_any_sample_aggregation(tmp_path, monkeypatch, mutation):
    data = digest_bundle()
    report = data["benchmarks"]
    if mutation == "missing_filter": del report["operation_filter"]
    elif mutation == "focused_mismatch": report["operation_filter"] = "fft"
    elif mutation == "partial_all":
        data = report_bundle("all")
        for key in ("benchmarks", "report"): data[key]["operations"] = data[key]["operations"][:2]
    elif mutation == "alias": report["operations"][0]["operation"] = "poseidon_hash_columns"
    elif mutation == "missing_raw": del data["report"]
    elif mutation == "missing_tag": del data["producer_schema"]
    elif mutation == "missing_nullable": del report["operations"][0]["speedup_delta_ms"]
    elif mutation == "raw_timing_mismatch": data["report"]["operations"][0]["cpu"]["mean_ms"] += 1
    elif mutation == "float_rows": report["rows"] = float(report["rows"])
    elif mutation == "bad_padding": report["padded_rows"] *= 2
    elif mutation == "overflow_invocations":
        for key in ("benchmarks", "report"):
            data[key]["warmups"] = (1 << 64) - 1
            data[key]["iterations"] = 1
    elif mutation == "flat_raw_metric": report["operations"][0]["gpu_recorded"] = True
    elif mutation == "retired_queue": data["report"]["metal_dispatch_queue"] = {"poseidon": {}}
    else: report["operations"][0]["digest384"]["gpu"]["parity_checked_lanes"] = 5
    path = write_bundle(tmp_path, data)
    calls = []
    monkeypatch.setattr(capture_matrix.OperationSummary, "add", lambda *args: calls.append(args))
    with pytest.raises(ValueError):
        capture_matrix.summarize_device("device", [path])
    assert calls == []


def test_cpu_capture_keeps_absent_gpu_measurements(tmp_path):
    data = report_bundle("fft", "none")
    summary = capture_matrix.summarize_device("cpu", [write_bundle(tmp_path, data)])
    entry = summary.to_manifest_entry(tmp_path, 0, 0)
    assert entry["backend"] == "none"
    assert entry["operations"]["fft"] == {"sample_count": 0, "median_gpu_mean_ms": None, "median_speedup_ratio": None}
    assert entry["max_operation_ms"] == {}
    assert entry["min_operation_speedup"] == {}


def test_direct_matrix_cli_validates_before_writing_manifest(tmp_path):
    repo = tmp_path / "repo"
    repo.mkdir()
    matrix = tmp_path / "matrix"
    (matrix / "devices").mkdir(parents=True)
    capture = write_bundle(tmp_path, report_bundle("all"))
    (matrix / "devices/device.txt").write_text(str(capture) + "\n")
    output = tmp_path / "matrix.json"
    command = [sys.executable, str(Path(capture_matrix.__file__)), "--repo-root", str(repo), "--matrix-dir", str(matrix), "--output", str(output), "--skip-acceleration-matrix"]
    result = subprocess.run(command, text=True, capture_output=True, check=False)
    assert result.returncode == 0, result.stdout + result.stderr
    assert json.loads(output.read_text())["devices"][0]["operation_filters"] == ["all"]
    output.unlink()
    malformed = report_bundle("fft")
    del malformed["benchmarks"]["operation_filter"]
    capture.write_text(json.dumps(malformed))
    result = subprocess.run(command, text=True, capture_output=True, check=False)
    assert result.returncode != 0
    assert not output.exists()


@pytest.mark.parametrize("backends", [("metal", "cuda"), ("cuda", "metal"), ("none", "metal"), ("metal", "none"), ("none", "cuda"), ("cuda", "none")])
def test_device_label_rejects_mixed_canonical_backends_in_both_input_orders(tmp_path, backends):
    paths = [write_bundle(tmp_path, report_bundle("fft", backend), f"{index}.json") for index, backend in enumerate(backends)]
    with pytest.raises(ValueError, match="device 'mixed' contains conflicting gpu_backend"):
        capture_matrix.summarize_device("mixed", paths)


@pytest.mark.parametrize("backend", ["none", "metal", "cuda"])
def test_same_backend_captures_aggregate_without_changing_device_label(tmp_path, backend):
    paths = [write_bundle(tmp_path, report_bundle("fft", backend, cpu=2.0 * gpu, gpu=gpu), f"{index}.json") for index, gpu in enumerate((1.0, 3.0))]
    entry = capture_matrix.summarize_device("same", paths).to_manifest_entry(tmp_path, 0, 0)
    assert entry["backend"] == backend
    assert entry["operation_filters"] == ["fft"]
    assert entry["operations"]["fft"] == {
        "sample_count": 0 if backend == "none" else 2,
        "median_gpu_mean_ms": None if backend == "none" else 2.0,
        "median_speedup_ratio": None if backend == "none" else 2.0,
    }
