"""Tests for the FASTPQ stage timing aggregator."""

import json

from scripts.fastpq import aggregate_stage_timings
from scripts.fastpq.tests.test_wrap_benchmark import current_cuda_payload
from scripts.fastpq.tests.test_digest384_evidence import primitive_report
import pytest


def test_normalize_report_accepts_wrapped_bundle_shape():
    payload = current_cuda_payload()

    report = aggregate_stage_timings.normalize_report(payload)

    assert report["operation_filter"] == "lde"
    assert report["operations"][0]["operation"] == "lde"


def test_load_samples_reports_bundle_filter(tmp_path):
    payload = current_cuda_payload()
    path = tmp_path / "wrapped_cuda.json"
    path.write_text(json.dumps(payload), encoding="utf-8")

    samples = aggregate_stage_timings.load_samples([str(path)], None)

    assert len(samples) == 1
    sample = samples[0]
    assert sample.bundle_filter == "lde"
    assert sample.operation == "lde"
    assert sample.cpu_ms == 0.2
    assert sample.gpu_ms == 0.1


def test_render_table_includes_filter_column():
    sample = aggregate_stage_timings.StageSample(
        path=aggregate_stage_timings.Path("wrapped_cuda.json"),
        bundle_filter="fft",
        operation="fft",
        gpu_ms=1.0,
        cpu_ms=2.0,
    )

    table = aggregate_stage_timings.render_table([sample])

    assert "| Report | Filter | Operation | GPU mean (ms) | CPU mean (ms) | Speedup |" in table
    assert "| `wrapped_cuda.json` | fft | fft | 1.000 | 2.000 | 2.000 |" in table


@pytest.mark.parametrize("mutation", ["alias", "missing_filter", "partial_parity"])
def test_stage_aggregator_rejects_retired_or_incomplete_reports(tmp_path, mutation):
    payload = primitive_report()
    if mutation == "alias":
        payload["operations"][0]["operation"] = "poseidon_hash_columns"
    elif mutation == "missing_filter":
        del payload["operation_filter"]
    else:
        payload["operations"][0]["digest384"]["gpu"]["parity_checked_lanes"] = 6
    path = tmp_path / "malformed.json"
    path.write_text(json.dumps(payload))
    with pytest.raises((SystemExit, ValueError)):
        aggregate_stage_timings.load_samples([str(path)], None)


def test_aggregator_rejects_divergent_nested_operation_copies(tmp_path):
    payload = current_cuda_payload()
    payload["benchmarks"]["operations"][0]["gpu_mean_ms"] = 100
    path = tmp_path / "divergent.json"
    path.write_text(json.dumps(payload))
    with pytest.raises(SystemExit, match="differs between report and benchmarks"):
        aggregate_stage_timings.load_samples([str(path)], None)
