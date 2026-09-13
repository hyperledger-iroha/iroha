"""Synthetic adversarial report checks; these are not measured device captures."""
import copy

import pytest

from scripts.fastpq import digest384_evidence as evidence
from scripts.fastpq import wrap_benchmark


def primitive_report(operation="digest384_trace_columns", backend="metal", *, rows=None, iterations=2, warmups=1, column_count=2):
    """Two public eight-row named columns, or two complete Merkle pairs."""
    columns = operation == "digest384_trace_columns"
    rows = (8 if columns else 2) if rows is None else rows
    padded = 1 << (rows - 1).bit_length()
    assert 1 <= column_count <= 100  # Synthetic names retain the two-digit width.
    frames = column_count if columns else rows
    unpadded_words = 59 + 4 + 3 + (padded * 8) // 7
    words_per_frame = unpadded_words + unpadded_words % 2 if columns else 78
    words = frames * words_per_frame
    input_bytes = column_count * (8 + padded * 8) if columns else frames * 96
    invocations = iterations + warmups
    max_batch_frames = min(frames, 1024)
    gpu = backend != "none"
    claim = {
        "schema": "fastpq-digest384-primitive-benchmark-v1",
        "catalog": "iroha-privacy-exact12-v1", "protocol": "fastpq-state-transition-stark-v1",
        "profile": "fastpq-state-transition-stark-v1", "role": "fastpq:v1:preprocessing-trace",
        "phase": "column-leaf" if columns else "binary-node", "level": 0 if columns else 1,
        "digest_lanes": 6, "output_encoding": "six-canonical-u64-le-words", "frame_count": frames,
        "input_field_bytes": input_bytes, "canonical_words": words,
        "sponge_permutations": words * 3, "output_bytes": frames * 48, "cpu_reference_verified": True,
    }
    entry = {
        "operation": operation, "columns": frames, "input_len": padded if columns else 12,
        "output_len": 6, "input_bytes": claim["input_field_bytes"], "output_bytes": frames * 48,
        "gpu_payload_buffer_bytes": words * 8 + 72 * frames, "digest384": claim,
        "cpu": {"mean_ms": 20.0, "min_ms": 19.0, "max_ms": 21.0},
    }
    if backend in {"none", "metal"}:
        entry["gpu_recorded"] = gpu
    if gpu:
        entry.update(gpu={"mean_ms": 10.0, "min_ms": 9.0, "max_ms": 11.0},
                     speedup={"ratio": 2.0, "delta_ms": 10.0})
        claim["gpu"] = {
            "backend": backend, "warmup_invocations": warmups, "timed_invocations": iterations,
            "invocations": invocations,
            "dispatches": ((frames + max_batch_frames - 1) // max_batch_frames) * invocations,
            "frames": frames * invocations, "canonical_words": words * invocations,
            "descriptor_words": 3 * frames * invocations, "output_words": 6 * frames * invocations,
            "max_batch_frames": max_batch_frames, "max_batch_words": max_batch_frames * words_per_frame,
            "parity_checked_digests": frames * invocations, "parity_checked_lanes": 6 * frames * invocations,
        }
    return {
        "producer_schema": "cuda_nested" if backend == "cuda" else "metal_flat",
        "rows": rows, "padded_rows": padded, "column_count": column_count, "iterations": iterations, "warmups": warmups,
        "execution_mode": "gpu" if gpu else "cpu", "gpu_backend": backend,
        "gpu_available": gpu, "operation_filter": operation, "operations": [entry],
    }


@pytest.mark.parametrize("operation", sorted(evidence.DIGEST384_OPERATIONS))
@pytest.mark.parametrize("backend", ["none", "metal", "cuda"])
def test_complete_primitive_preserves_evidence_in_each_report_projection(operation, backend):
    report = primitive_report(operation, backend)
    schema = wrap_benchmark.CUDA_NESTED_SCHEMA if backend == "cuda" else wrap_benchmark.METAL_FLAT_SCHEMA
    wrap_benchmark.validate_report_header(report, schema)
    projected, _ = wrap_benchmark.summarize_operations(report, schema)
    assert projected[0]["digest384"] == report["operations"][0]["digest384"]
    assert "estimated_gpu_transfer_bytes" not in projected[0]
    evidence.validate_digest384_operation(projected[0], report, flattened=True)
    if backend == "cuda":
        capture = {"producer_schema": report["producer_schema"], "report": report, "benchmarks": {**report, "operations": projected}}
        assert wrap_benchmark.normalize_report(capture)["operations"] == report["operations"]


@pytest.mark.parametrize("field", sorted(evidence._FIELDS))
def test_every_framing_field_is_required(field):
    report = primitive_report()
    del report["operations"][0]["digest384"][field]
    with pytest.raises(ValueError, match="canonical fields"):
        evidence.validate_digest384_operation(report["operations"][0], report)


@pytest.mark.parametrize("field", sorted(evidence._GPU_FIELDS))
def test_every_device_work_field_is_required(field):
    report = primitive_report()
    del report["operations"][0]["digest384"]["gpu"][field]
    with pytest.raises(ValueError, match="canonical fields"):
        evidence.validate_digest384_operation(report["operations"][0], report)


@pytest.mark.parametrize("path,value", [
    (("operation",), "poseidon_hash_columns"),
    (("operation",), "poseidon_merkle_pairs"),
    (("columns",), True), (("output_len",), 1), (("output_bytes",), 32),
    (("estimated_gpu_transfer_bytes",), 1360), (("gpu_payload_buffer_bytes",), 1),
    (("digest384", "schema"), "fastpq-digest384-primitive-benchmark-v2"),
    (("digest384", "catalog"), "old-catalog"), (("digest384", "profile"), "fastpq-prod"),
    (("digest384", "protocol"), "ivm"), (("digest384", "role"), "merkle"),
    (("digest384", "phase"), "binary-node"), (("digest384", "level"), False),
    (("digest384", "digest_lanes"), 1), (("digest384", "frame_count"), 1),
    (("digest384", "input_field_bytes"), 128), (("digest384", "canonical_words"), 151),
    (("digest384", "sponge_permutations"), 76), (("digest384", "output_bytes"), 32),
    (("digest384", "cpu_reference_verified"), False), (("digest384", "cpu_reference_verified"), 1),
    (("digest384", "fallback"), "scalar"), (("digest384", "gpu", "backend"), "cuda"),
    (("digest384", "gpu", "warmup_invocations"), 0), (("digest384", "gpu", "timed_invocations"), 3),
    (("digest384", "gpu", "invocations"), 2), (("digest384", "gpu", "frames"), 4),
    (("digest384", "gpu", "dispatches"), 0), (("digest384", "gpu", "dispatches"), 7),
    (("digest384", "gpu", "descriptor_words"), 6), (("digest384", "gpu", "output_words"), 6),
    (("digest384", "gpu", "canonical_words"), 152), (("digest384", "gpu", "max_batch_frames"), 1),
    (("digest384", "gpu", "max_batch_frames"), 65537), (("digest384", "gpu", "max_batch_words"), 100),
    (("digest384", "gpu", "parity_checked_digests"), 2), (("digest384", "gpu", "parity_checked_lanes"), 6),
    (("digest384", "gpu", "unknown"), 1),
])
def test_partial_parity_aliases_and_inconsistent_counts_are_rejected(path, value):
    report = primitive_report()
    target = report["operations"][0]
    for key in path[:-1]:
        target = target[key]
    target[path[-1]] = value
    with pytest.raises(ValueError):
        evidence.validate_digest384_operation(report["operations"][0], report)


def test_consistent_false_merkle_word_count_cannot_pass_arithmetic_projections():
    report = primitive_report("digest384_merkle_pairs", "none")
    entry = report["operations"][0]
    entry["digest384"].update(canonical_words=158, sponge_permutations=474)
    entry["gpu_payload_buffer_bytes"] = 158 * 8 + 144
    with pytest.raises(ValueError, match="Merkle canonical_words"):
        evidence.validate_digest384_operation(entry, report)


def test_cpu_capture_cannot_retain_device_claim_or_suppress_missing_gpu_timing():
    cpu = primitive_report(backend="none")
    entry = cpu["operations"][0]
    entry["digest384"]["gpu"] = primitive_report()["operations"][0]["digest384"]["gpu"]
    with pytest.raises(ValueError, match="omit device"):
        evidence.validate_digest384_operation(entry, cpu)
    gpu = primitive_report()
    del gpu["operations"][0]["gpu"]
    with pytest.raises(ValueError, match="timing presence"):
        evidence.validate_digest384_operation(gpu["operations"][0], gpu)


@pytest.mark.parametrize("field", ["canonical_words", "parity_checked_lanes", "backend"])
def test_nested_cuda_copies_must_preserve_exact_device_evidence(field):
    report = primitive_report(backend="cuda")
    projected, _ = wrap_benchmark.summarize_operations(report, wrap_benchmark.CUDA_NESTED_SCHEMA)
    capture = {"producer_schema": report["producer_schema"], "report": copy.deepcopy(report), "benchmarks": copy.deepcopy({**report, "operations": projected})}
    capture["benchmarks"]["operations"][0]["digest384"]["gpu"][field] = "divergent"
    with pytest.raises(SystemExit, match="differs between report and benchmarks"):
        wrap_benchmark.normalize_report(capture)


@pytest.mark.parametrize("section,field", [("benchmarks", "gpu_available"), ("operation", "columns"), ("evidence", "cpu_reference_verified")])
def test_nested_cuda_copies_reject_boolean_integer_aliases(section, field):
    report = primitive_report(backend="cuda")
    projected, _ = wrap_benchmark.summarize_operations(report, wrap_benchmark.CUDA_NESTED_SCHEMA)
    capture = {"producer_schema": report["producer_schema"], "report": copy.deepcopy(report), "benchmarks": copy.deepcopy({**report, "operations": projected})}
    if section == "benchmarks":
        capture["benchmarks"][field] = 1
    elif section == "evidence":
        capture["benchmarks"]["operations"][0]["digest384"][field] = 1
    else:
        # Equal numeric value with a different JSON number representation.
        capture["benchmarks"]["operations"][0][field] = 2.0
    with pytest.raises(SystemExit, match="between report and benchmarks"):
        wrap_benchmark.normalize_report(capture)


@pytest.mark.parametrize("operation", sorted(evidence.DIGEST384_OPERATIONS))
@pytest.mark.parametrize("field,value", [("rows", 20_000), ("padded_rows", 32_768), ("column_count", 3)])
def test_small_work_cannot_claim_larger_benchmark_geometry(operation, field, value):
    report = primitive_report(operation)
    report[field] = value
    if operation == "digest384_merkle_pairs" and field == "column_count":
        evidence.validate_digest384_operation(report["operations"][0], report)
    else:
        with pytest.raises(ValueError):
            evidence.validate_digest384_operation(report["operations"][0], report)


@pytest.mark.parametrize("backend", ["none", "metal", "cuda"])
def test_explicit_schema_survives_native_and_wrapped_primitive_report(backend):
    report = primitive_report(backend=backend)
    schema = report["producer_schema"]
    projected, _ = wrap_benchmark.summarize_operations(report, schema)
    bundle = {"producer_schema": schema, "report": report, "benchmarks": {**report, "operations": projected}}
    normalized = wrap_benchmark.normalize_report(bundle)
    assert normalized == report
    assert wrap_benchmark.producer_schema_for_payload(bundle) == schema
    del bundle["producer_schema"]
    with pytest.raises(SystemExit, match="producer_schema"):
        wrap_benchmark.normalize_report(bundle)
