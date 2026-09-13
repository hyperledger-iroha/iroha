"""Tests for the FASTPQ benchmark history generator."""

import json

import pytest

from scripts.fastpq import update_benchmark_history
from scripts.fastpq.tests.report_fixtures import add_synthetic_raw_copy, flat_report


def test_format_operation_filter_prefers_explicit_filter():
    bench = {"operation_filter": "lde"}
    result = update_benchmark_history.format_operation_filter(bench)

    assert result == "lde"


def test_format_operation_filter_rejects_missing_field():
    bench = {}
    with pytest.raises(ValueError, match="operation filter"):
        update_benchmark_history.format_operation_filter(bench)


def test_collect_benchmark_rows_carries_operation_filter(tmp_path):
    bundle = {
        "metadata": {
            "labels": {
                "device_class": "xeon-rtx",
                "gpu_model": "NVIDIA RTX 6000 Ada",
            }
        },
        "producer_schema": "cuda_nested",
        "benchmarks": flat_report(cpu=12.0, gpu=6.0),
    }
    add_synthetic_raw_copy(bundle)
    path = tmp_path / "fastpq_cuda_bench_probe.json"
    path.write_text(json.dumps(bundle), encoding="utf-8")

    rows = update_benchmark_history.collect_benchmark_rows(tmp_path)

    assert len(rows) == 1
    row = rows[0]
    assert row.operation_filter == "lde"
    assert row.gpu_backend == "cuda"
    assert update_benchmark_history.format_operation(row.operation_evidence["report"]["operations"][0]) == "12.0/6.0/2.00"


def test_gpu_table_mentions_filter_column():
    row = update_benchmark_history.BenchmarkRow(
        bundle=update_benchmark_history.Path("fastpq_cuda_bench_probe.json"),
        backend="cuda",
        execution_mode="gpu",
        gpu_backend="cuda",
        gpu_available="yes",
        operation_filter="lde",
        device_class="xeon-rtx",
        gpu_model="NVIDIA RTX 6000 Ada",
        operation_evidence={"flattened": True, "producer_schema": "cuda_nested", "report": flat_report(cpu=12.0, gpu=6.0)},
    )

    table = update_benchmark_history.gpu_table([row])

    assert "| Bundle | Backend | Mode | GPU backend | GPU available | Filter |" in table
    assert "| `fastpq_cuda_bench_probe.json` | cuda | gpu | cuda | yes | lde |" in table




def test_retired_manifest_cli_is_removed(monkeypatch):
    monkeypatch.setattr("sys.argv", ["update_benchmark_history.py", "--poseidon-manifest", "manifest.json"])
    with pytest.raises(SystemExit) as error:
        update_benchmark_history.parse_args()
    assert error.value.code == 2


def test_history_has_no_scalar_manifest_table_or_regeneration_hint():
    rendered = update_benchmark_history.render_document([], "merkle", "rows")
    assert "Poseidon Microbench" not in rendered
    assert "default-vs-scalar" not in rendered
    assert "export_poseidon_microbench" not in rendered
    for label in ["FFT", "IFFT", "LDE", "Six-lane trace columns", "Six-lane Merkle pairs", "BN254 Poseidon words"]:
        assert label in rendered
