"""Regression tests for FASTPQ rollout evidence consistency and performance gates."""

import hashlib
import json
import os
from pathlib import Path
import subprocess

import blake3
import pytest

from scripts.fastpq import validate_rollout_manifest as validator
from scripts.fastpq import wrap_benchmark
from scripts.fastpq.tests.report_fixtures import complete_flat_report, add_synthetic_raw_copy


def make_evidence(tmp_path):
    """Build complete synthetic captures solely for exercising the validator."""
    benches = []
    for backend in ("metal", "cuda"):
        benchmarks = complete_flat_report(backend, rows=20_000, iterations=5, warmups=1)
        for operation in benchmarks["operations"]:
            operation.update(cpu_mean_ms=100.0, gpu_mean_ms=50.0,
                             speedup_ratio=2.0, speedup_delta_ms=50.0)
        if backend == "metal":
            benchmarks["metal_dispatch_queue"] = {
                "limit": 4, "max_in_flight": 3, "dispatch_count": 32,
            }
            benchmarks["zero_fill_hotspots"] = [{"operation": "lde", "mean_ms": 0.2}]
        capture = {
            "producer_schema": "metal_flat" if backend == "metal" else "cuda_nested",
            "metadata": {"labels": {"device_class": "test-only", "gpu_kind": "test-only"}},
            "benchmarks": benchmarks,
        }
        bench = {key: benchmarks[key] for key in (
            "rows", "padded_rows", "iterations", "warmups", "gpu_backend", "gpu_available",
        )}
        bench.update(label=backend, path=f"{backend}.json")
        benches.append(bench)
        write_capture(tmp_path, bench, capture)
    manifest = {
        "payload": {
            "version": 1,
            "benches": benches,
            "constraints": {
                "require_rows": 20_000,
                "max_operation_ms": {"lde": 950.0},
                "min_operation_speedup": {"fft": 1.0},
            },
        },
        "signature": None,
    }
    return manifest


def write_capture(tmp_path, bench, capture, *, rebuild_raw=True):
    """Reseal synthetic metrics in both copies to reach each intended policy guard."""
    if rebuild_raw:
        capture.pop("report", None)
        add_synthetic_raw_copy(capture)
    content = json.dumps(capture).encode()
    (tmp_path / bench["path"]).write_bytes(content)
    bench["hashes"] = {
        "sha256_hex": hashlib.sha256(content).hexdigest(),
        "blake3_hex": blake3.blake3(content).hexdigest(),
    }


def validate(tmp_path, manifest):
    path = tmp_path / "fastpq_bench_manifest.json"
    path.write_text(json.dumps(manifest))
    validator.validate_manifest(path, tmp_path)


def test_complete_captures_pass_consistency_gate(tmp_path):
    validate(tmp_path, make_evidence(tmp_path))


@pytest.mark.parametrize("field", ("sha256_hex", "blake3_hex"))
def test_capture_hash_mismatch_fails(tmp_path, field):
    manifest = make_evidence(tmp_path)
    manifest["payload"]["benches"][0]["hashes"][field] = "00" * 32
    with pytest.raises(ValueError, match="does not match"):
        validate(tmp_path, manifest)


@pytest.mark.parametrize("field,value", (
    ("rows", 1), ("padded_rows", 65536), ("iterations", 0),
    ("warmups", True), ("gpu_available", False), ("gpu_backend", "cuda"),
))
def test_manifest_cannot_misrepresent_capture(tmp_path, field, value):
    manifest = make_evidence(tmp_path)
    manifest["payload"]["benches"][0][field] = value
    with pytest.raises(ValueError, match="disagrees"):
        validate(tmp_path, manifest)


@pytest.mark.parametrize("mutation,message", (
    (lambda b: b.update(rows=100), "padded_rows|disagrees|rows"),
    (lambda b: b.update(iterations=0), "iterations"),
    (lambda b: b.update(gpu_available=False), "available"),
    (lambda b: b.update(gpu_backend="cuda"), "digest384.gpu.backend"),
    (lambda b: b.update(operation_filter="lde"), "operation_filter disagrees"),
    (lambda b: b["operations"].pop(), "exact six-operation ordered inventory"),
    (lambda b: b["operations"].append(b["operations"][0]), "duplicate benchmark operation"),
    (lambda b: b["operations"][0].update(gpu_mean_ms=-1), "finite valid gpu_mean_ms"),
    (lambda b: b["operations"][0].update(speedup_ratio=3), "disagrees"),
    (lambda b: b["metal_dispatch_queue"].update(dispatch_count=0), "dispatch_count"),
    (lambda b: b["metal_dispatch_queue"].update(max_in_flight=4), "headroom"),
    (lambda b: b["metal_dispatch_queue"].update(max_in_flight=-1), "max_in_flight"),
    (lambda b: b["zero_fill_hotspots"][0].update(mean_ms=0.41), "exceeds"),
    (lambda b: b["zero_fill_hotspots"][0].update(mean_ms=-1), "non-negative"),
))
def test_rehashed_invalid_capture_fails(tmp_path, mutation, message):
    manifest = make_evidence(tmp_path)
    bench = manifest["payload"]["benches"][0]
    capture = json.loads((tmp_path / bench["path"]).read_bytes())
    mutation(capture["benchmarks"])
    for key in ("rows", "padded_rows", "iterations", "warmups", "gpu_backend", "gpu_available"):
        bench[key] = capture["benchmarks"][key]
    write_capture(tmp_path, bench, capture)
    with pytest.raises(ValueError, match=message):
        validate(tmp_path, manifest)


@pytest.mark.parametrize("operation,cpu,gpu,speedup", (
    ("lde", 2000.0, 1000.0, 2.0),
    ("fft", 25.0, 50.0, 0.5),
))
def test_actual_performance_must_satisfy_claimed_limits(tmp_path, operation, cpu, gpu, speedup):
    manifest = make_evidence(tmp_path)
    bench = manifest["payload"]["benches"][0]
    capture = json.loads((tmp_path / bench["path"]).read_bytes())
    entry = next(item for item in capture["benchmarks"]["operations"] if item["operation"] == operation)
    entry.update(cpu_mean_ms=cpu, gpu_mean_ms=gpu, speedup_ratio=speedup, speedup_delta_ms=cpu-gpu)
    write_capture(tmp_path, bench, capture)
    with pytest.raises(ValueError, match="violates"):
        validate(tmp_path, manifest)


@pytest.mark.parametrize("value", (float("nan"), float("inf"), float("-inf"), -1, True, "0.2", 10**1000))
def test_numeric_gate_rejects_invalid_measurements(value):
    with pytest.raises(ValueError):
        validator.number(value, "metric")


@pytest.mark.parametrize("raw", (b'{"a":1,"a":2}', b'[]'))
def test_json_must_be_unambiguous_object(raw):
    with pytest.raises(ValueError):
        validator.load_json(raw)


def test_cli_reports_failure_and_success(tmp_path):
    manifest = make_evidence(tmp_path)
    validate(tmp_path, manifest)
    path = tmp_path / "fastpq_bench_manifest.json"
    assert validator.main([str(path), "--repo-root", str(tmp_path)]) == 0
    path.write_text("{}")
    with pytest.raises(SystemExit) as exc:
        validator.main([str(path), "--repo-root", str(tmp_path)])
    assert exc.value.code == 1


def test_rollout_shell_rechecks_captured_performance(tmp_path):
    manifest = make_evidence(tmp_path)
    for bench in manifest["payload"]["benches"]:
        bench["path"] = str(tmp_path / bench["path"])
    bench = manifest["payload"]["benches"][0]
    capture = json.loads(Path(bench["path"]).read_bytes())
    entry = next(item for item in capture["benchmarks"]["operations"] if item["operation"] == "lde")
    entry.update(cpu_mean_ms=2000, gpu_mean_ms=1000, speedup_ratio=2, speedup_delta_ms=1000)
    write_capture(tmp_path, bench, capture)
    path = tmp_path / "fastpq_bench_manifest.json"
    path.write_text(json.dumps(manifest))
    result = subprocess.run(
        ["bash", str(Path(__file__).resolve().parents[3] / "ci/check_fastpq_rollout.sh")],
        env={**os.environ, "FASTPQ_ROLLOUT_BUNDLE": str(path)},
        capture_output=True, text=True, check=False,
    )
    assert result.returncode == 1
    assert "lde.gpu_mean_ms=1000 violates" in result.stderr


def test_rollout_shell_requires_independent_signature_trust(tmp_path):
    manifest = make_evidence(tmp_path)
    for bench in manifest["payload"]["benches"]:
        bench["path"] = str(tmp_path / bench["path"])
    path = tmp_path / "fastpq_bench_manifest.json"
    path.write_text(json.dumps(manifest))
    env = {**os.environ, "FASTPQ_ROLLOUT_BUNDLE": str(path)}
    env.pop("FASTPQ_ROLLOUT_TRUSTED_PUBLIC_KEY", None)
    result = subprocess.run(
        ["bash", str(Path(__file__).resolve().parents[3] / "ci/check_fastpq_rollout.sh")],
        env=env, capture_output=True, text=True, check=False,
    )
    assert result.returncode == 1
    assert "independently trusted Ed25519 release key" in result.stderr


def test_rollout_shell_propagates_signature_verifier_rejection(tmp_path):
    manifest = make_evidence(tmp_path)
    for bench in manifest["payload"]["benches"]:
        bench["path"] = str(tmp_path / bench["path"])
    path = tmp_path / "fastpq_bench_manifest.json"
    path.write_text(json.dumps(manifest))
    verifier = tmp_path / "reject-signature"
    verifier.write_text(
        '#!/bin/sh\n'
        '[ "$1" = fastpq-verify-bench-manifest ] || exit 99\n'
        '[ "$3" != "$FASTPQ_ROLLOUT_BUNDLE" ] || exit 98\n'
        'cmp -s "$3" "$FASTPQ_ROLLOUT_BUNDLE" || exit 97\n'
        'printf "{}" > "$FASTPQ_ROLLOUT_BUNDLE"\n'
        'cmp -s "$3" "$FASTPQ_ROLLOUT_BUNDLE" && exit 96\n'
        'printf "%s" "$3" > "$FASTPQ_TEST_SNAPSHOT_PATH"\n'
        'exit 7\n'
    )
    verifier.chmod(0o700)
    result = subprocess.run(
        ["bash", str(Path(__file__).resolve().parents[3] / "ci/check_fastpq_rollout.sh")],
        env={**os.environ, "FASTPQ_ROLLOUT_BUNDLE": str(path),
             "FASTPQ_ROLLOUT_TRUSTED_PUBLIC_KEY": "externally-configured-key",
             "FASTPQ_XTASK_BIN": str(verifier),
             "FASTPQ_TEST_SNAPSHOT_PATH": str(tmp_path / "snapshot-path")},
        capture_output=True, text=True, check=False,
    )
    assert result.returncode == 7
    assert not Path((tmp_path / "snapshot-path").read_text()).exists()


@pytest.mark.parametrize("mutation", ["missing_schema", "wrong_schema", "retired_profiles", "retired_scalar_phase"])
def test_rollout_rejects_retired_fields_and_undeclared_producer_even_after_rehash(tmp_path, mutation):
    manifest = make_evidence(tmp_path)
    bench = manifest["payload"]["benches"][0]
    capture = json.loads((tmp_path / bench["path"]).read_text())
    if mutation == "missing_schema":
        del capture["producer_schema"]
    elif mutation == "wrong_schema":
        capture["producer_schema"] = "cuda_nested"
    elif mutation == "retired_profiles":
        capture["benchmarks"]["poseidon_profiles"] = None
    else:
        capture["benchmarks"]["column_staging"] = {"phases": {"fft": {}, "lde": {}, "poseidon": {}}, "samples": {"fft": [], "lde": []}}
    write_capture(tmp_path, bench, capture, rebuild_raw=False)
    with pytest.raises(ValueError):
        validate(tmp_path, manifest)


@pytest.mark.parametrize("mutation", [
    "missing_raw", "raw_tag", "raw_count", "missing_delta", "raw_metric",
    "generic_count", "wrong_all_order", "partial_all", "flat_raw_flag",
])
def test_rollout_rehash_cannot_bypass_shared_complete_report_owner(tmp_path, mutation):
    manifest = make_evidence(tmp_path)
    bench = manifest["payload"]["benches"][0]
    capture = json.loads((tmp_path / bench["path"]).read_bytes())
    if mutation == "missing_raw": capture.pop("report")
    elif mutation == "raw_tag": capture["report"].pop("producer_schema")
    elif mutation == "raw_count": capture["report"]["column_count"] += 1
    elif mutation == "missing_delta": capture["benchmarks"]["operations"][0].pop("speedup_delta_ms")
    elif mutation == "raw_metric": capture["report"]["operations"][0]["cpu"]["mean_ms"] += 1
    elif mutation == "generic_count":
        for report in (capture["report"], capture["benchmarks"]): report["operations"][0].pop("input_len")
    elif mutation == "wrong_all_order":
        for report in (capture["report"], capture["benchmarks"]): report["operations"].reverse()
    elif mutation == "partial_all":
        for report in (capture["report"], capture["benchmarks"]): report["operations"].pop()
    else: capture["benchmarks"]["operations"][0]["gpu_recorded"] = True
    write_capture(tmp_path, bench, capture, rebuild_raw=False)
    with pytest.raises(ValueError): validate(tmp_path, manifest)
