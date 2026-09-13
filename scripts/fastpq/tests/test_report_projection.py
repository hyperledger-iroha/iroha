"""Lossless report projections and rejected scalar evidence; fixtures are synthetic."""
from copy import deepcopy
import json
from pathlib import Path

import pytest

from scripts.fastpq import report_projection as projection
from scripts.fastpq import rollout_manifest_summary as rollout
from scripts.fastpq import update_benchmark_history as history
from scripts.fastpq import update_dashboard_panel as dashboard
from scripts.fastpq import wrap_benchmark
from scripts.fastpq.tests.test_digest384_evidence import primitive_report
from scripts.fastpq.tests.report_fixtures import complete_flat_report, add_synthetic_raw_copy, column_staging, flat_report


def bundle(operation="digest384_trace_columns", backend="metal", *, raw_copy=True):
    report = primitive_report(operation, backend)
    schema = wrap_benchmark.CUDA_NESTED_SCHEMA if backend == "cuda" else wrap_benchmark.METAL_FLAT_SCHEMA
    report["producer_schema"] = schema
    operations, _ = wrap_benchmark.summarize_operations(report, schema)
    result = {
        "producer_schema": schema,
        "metadata": {"generated_at": "2026-09-13T00:00:00Z", "command": "canonical benchmark capture"},
        "benchmarks": {**report, "operations": operations},
    }
    if raw_copy:
        result["report"] = report
    return result


def manifest_entry(data, path):
    report = data["benchmarks"]
    return {
        "label": "test", "path": str(path),
        **{field: report[field] for field in ("operation_filter", "iterations", "warmups", "gpu_backend", "gpu_available")},
    }


@pytest.mark.parametrize("operation", ["digest384_trace_columns", "digest384_merkle_pairs"])
@pytest.mark.parametrize("backend", ["none", "metal", "cuda"])
def test_every_report_projection_preserves_and_revalidates_full_evidence(tmp_path, operation, backend):
    data = bundle(operation, backend, raw_copy=True)
    original = deepcopy(data)
    evidence = projection.project_bundle(data)
    assert evidence["flattened"] is True
    assert evidence["report"]["operations"] == data["benchmarks"]["operations"]
    for field in ("warmups", "iterations", "execution_mode", "gpu_available", "gpu_backend"):
        assert evidence["report"][field] == data["benchmarks"][field]
    assert projection.validate_projection(evidence) == evidence
    raw = projection.project_report(data["report"], flattened=False, producer_schema=data["producer_schema"])
    if data["producer_schema"] == "metal_flat":
        assert projection.project_bundle(data["report"]) == raw
    else:
        with pytest.raises(ValueError): projection.project_bundle(data["report"])
    assert raw["flattened"] is False
    assert raw["report"]["operations"] == data["report"]["operations"]
    assert data == original
    evidence["report"]["operations"][0]["digest384"]["digest_lanes"] = 5
    assert data == original
    with pytest.raises(ValueError): projection.render_evidence(evidence)

    path = tmp_path / "fastpq_metal_bench_probe.json"
    path.write_text(json.dumps(data))
    rows = history.collect_benchmark_rows(tmp_path)
    assert len(rows) == 1
    assert rows[0].operation_evidence["report"]["operations"] == data["benchmarks"]["operations"]
    rendered = history.render_document(rows, "", "")
    assert '"digest_lanes": 6' in rendered
    assert '"gpu_payload_buffer_bytes"' in rendered
    assert operation in rendered
    rendered = dashboard.build_markdown(path, data)
    fenced = rendered.split("```json\n", 1)[1].split("\n```", 1)[0]
    assert json.loads(fenced) == projection.project_bundle(data)
    assert "\n```json\n" in rendered
    assert '"digest_lanes": 6' in rendered
    assert '"canonical_words"' in rendered
    summary = rollout.summarize_bench_entry(manifest_entry(data, path), bundle_dir=tmp_path, repo_root=tmp_path)
    assert summary["source_command"] == "canonical benchmark capture"
    assert summary["operation_evidence"]["report"]["operations"] == data["benchmarks"]["operations"]
    rendered = rollout.render_markdown({"benches": [summary]})
    assert '"digest_lanes": 6' in rendered
    assert '"cpu_reference_verified": true' in rendered


@pytest.mark.parametrize("key", ["poseidon_microbench", "scalar_lane", "speedup_vs_scalar", "default_mean_ms", "scalar_mean_ms", "poseidon_profiles"])
@pytest.mark.parametrize("location", ["root", "benchmarks", "operation", "metadata"])
def test_retired_fields_reject_even_beside_valid_six_lane_report(tmp_path, key, location):
    data = bundle()
    target = {"root": data, "benchmarks": data["benchmarks"], "operation": data["benchmarks"]["operations"][0], "metadata": data["metadata"]}[location]
    target[key] = None
    with pytest.raises(ValueError, match="retired"):
        projection.project_bundle(data)
    with pytest.raises(ValueError, match="retired"):
        dashboard.build_markdown(Path("capture.json"), data)
    path = tmp_path / "fastpq_metal_bench_probe.json"
    path.write_text(json.dumps(data))
    with pytest.raises(ValueError, match="retired"):
        history.collect_benchmark_rows(tmp_path)
    with pytest.raises(ValueError, match="retired"):
        rollout.summarize_bench_entry(manifest_entry(data, path), bundle_dir=tmp_path, repo_root=tmp_path)


@pytest.mark.parametrize("operation", ["poseidon_hash_columns", "poseidon_merkle_pairs", "digest384_columns", "unknown"])
def test_retired_or_aliased_operation_ids_reject_in_every_ingress(tmp_path, operation):
    data = bundle()
    data["benchmarks"]["operations"][0]["operation"] = operation
    with pytest.raises(ValueError, match="operation"):
        dashboard.build_markdown(Path("capture.json"), data)
    path = tmp_path / "fastpq_metal_bench_probe.json"; path.write_text(json.dumps(data))
    with pytest.raises(ValueError, match="operation"): history.collect_benchmark_rows(tmp_path)
    with pytest.raises(ValueError, match="operation"):
        rollout.summarize_bench_entry(manifest_entry(data, path), bundle_dir=tmp_path, repo_root=tmp_path)


@pytest.mark.parametrize("mutation", ["missing", "lane", "words", "parity", "timed", "context"])
def test_malformed_digest_evidence_cannot_be_rendered_as_valid_timings(tmp_path, mutation):
    data = bundle()
    entry = data["benchmarks"]["operations"][0]
    if mutation == "missing": del entry["digest384"]
    elif mutation == "lane": entry["digest384"]["digest_lanes"] = 1
    elif mutation == "words": entry["digest384"]["canonical_words"] -= 2
    elif mutation == "parity": entry["digest384"]["gpu"]["parity_checked_lanes"] -= 1
    elif mutation == "timed": entry["digest384"]["gpu"]["timed_invocations"] -= 1
    else: data["benchmarks"]["iterations"] += 1
    with pytest.raises(ValueError): dashboard.build_markdown(Path("capture.json"), data)
    path = tmp_path / "fastpq_metal_bench_probe.json"; path.write_text(json.dumps(data))
    with pytest.raises(ValueError): history.collect_benchmark_rows(tmp_path)
    with pytest.raises(ValueError): rollout.summarize_bench_entry(manifest_entry(data, path), bundle_dir=tmp_path, repo_root=tmp_path)


def test_fft_ifft_lde_and_bn254_entries_remain_complete(tmp_path):
    entries = [{"operation": name, "cpu_mean_ms": 2.0, "gpu_mean_ms": 1.0, "speedup_ratio": 2.0, "measured_detail": {"sample_count": 7}} for name in ["fft", "ifft", "lde", "bn254_poseidon_words"]]
    report = complete_flat_report("cuda")
    extras = {entry["operation"]: entry for entry in entries}
    for complete in report["operations"]:
        if complete["operation"] in extras:
            complete.update(extras[complete["operation"]])
    entries = report["operations"]
    data = add_synthetic_raw_copy({"producer_schema": "cuda_nested", "benchmarks": report})
    evidence = projection.project_bundle(data)
    assert evidence["report"]["operations"] == entries
    path = tmp_path / "fastpq_cuda_bench_probe.json"; path.write_text(json.dumps(data))
    rows = history.collect_benchmark_rows(tmp_path)
    table = history.gpu_table(rows)
    for label in ["FFT", "IFFT", "LDE", "BN254 Poseidon words"]: assert label in table
    assert table.count("2.0/1.0/2.00") == 4


def test_duplicate_and_conflicting_report_copies_reject():
    data = bundle(raw_copy=True)
    data["report"]["operations"][0]["cpu"]["mean_ms"] = 21.0
    with pytest.raises(ValueError, match="differs"): projection.project_bundle(data)
    data = bundle(); data["benchmarks"]["operations"] *= 2
    with pytest.raises(ValueError, match="duplicate"): projection.project_bundle(data)
    data = bundle(); data["benchmarks"]["operation_filter"] = "lde"
    with pytest.raises(ValueError, match="disagrees"): projection.project_bundle(data)


def test_summary_manifest_scope_and_retired_fields_reject_without_local_capture(tmp_path):
    entry = {"label": "missing", "path": "absent.json", "operation_filter": "lde", "matrix_operation_filters": ["lde"]}
    assert "load_warning" in rollout.summarize_bench_entry(entry, bundle_dir=tmp_path, repo_root=tmp_path)
    for field, value in [("operation_filter", "poseidon_hash_columns"), ("matrix_operation_filters", ["poseidon_merkle_pairs"]), ("poseidon_microbench", None)]:
        changed = {**entry, field: value}
        with pytest.raises(ValueError): rollout.summarize_bench_entry(changed, bundle_dir=tmp_path, repo_root=tmp_path)
    for field in ["max_operation_ms", "min_operation_speedup"]:
        with pytest.raises(ValueError): rollout.render_constraints({field: {"poseidon_hash_columns": 1}})


def test_projection_rendering_rechecks_retained_operations_and_exact_format():
    evidence = projection.project_bundle(bundle())
    for value in [1, "true", None]:
        with pytest.raises(ValueError): projection.validate_projection({**evidence, "flattened": value})
        with pytest.raises(ValueError): projection.project_report(evidence["report"], flattened=value, producer_schema=evidence["producer_schema"])
    with pytest.raises(ValueError): projection.validate_projection({**evidence, "extra": 1})
    with pytest.raises(ValueError): projection.project_bundle({"report": primitive_report()})
    with pytest.raises(ValueError): projection.project_bundle(primitive_report(), require_wrapped=True)


def test_obsolete_scalar_export_commands_are_removed():
    scripts = Path(__file__).resolve().parent.parent
    assert not (scripts / "export_poseidon_microbench.py").exists()
    assert not (scripts / "aggregate_poseidon_microbench.py").exists()


def test_rollout_output_is_not_written_before_evidence_validation(tmp_path):
    data = bundle()
    path = tmp_path / "capture.json"; path.write_text(json.dumps(data))
    bench = rollout.summarize_bench_entry(manifest_entry(data, path), bundle_dir=tmp_path, repo_root=tmp_path)
    bench["operation_evidence"]["report"]["operations"][0]["digest384"]["gpu"]["parity_checked_lanes"] -= 1
    json_out, markdown_out = tmp_path / "summary.json", tmp_path / "summary.md"
    with pytest.raises(ValueError):
        rollout.write_summary({"benches": [bench]}, json_out=json_out, markdown_out=markdown_out)
    assert not json_out.exists()
    assert not markdown_out.exists()


def test_rendering_cannot_swap_scope_or_operation_projection(tmp_path):
    data = bundle()
    path = tmp_path / "fastpq_metal_bench_probe.json"; path.write_text(json.dumps(data))
    rows = history.collect_benchmark_rows(tmp_path)
    rows[0].operation_evidence = projection.project_bundle(bundle(raw_copy=True)["report"])
    with pytest.raises(ValueError, match="flattened"): history.gpu_table(rows)
    rows[0].operation_evidence = projection.project_bundle(data)
    rows[0].operation_filter = "lde"
    with pytest.raises(ValueError, match="filter"): history.gpu_table(rows)
    bench = rollout.summarize_bench_entry(manifest_entry(data, path), bundle_dir=tmp_path, repo_root=tmp_path)
    bench["available_operations"] = ["lde"]
    with pytest.raises(ValueError, match="operations"): rollout.render_markdown({"benches": [bench]})
    bench["available_operations"] = ["digest384_trace_columns"]
    bench["operation_filter"] = "lde"
    with pytest.raises(ValueError, match="filter"): rollout.render_markdown({"benches": [bench]})


@pytest.mark.parametrize("operation", ["fft", "lde", "bn254_poseidon_words"])
def test_six_lane_claims_cannot_be_relabelled_as_other_canonical_operations(operation):
    data = bundle()
    data["benchmarks"]["operation_filter"] = operation
    data["benchmarks"]["operations"][0]["operation"] = operation
    with pytest.raises(ValueError, match="cannot label another operation"):
        projection.project_bundle(data)


@pytest.mark.parametrize("mutation", [
    "no_schema", "alias_schema", "nested_schema", "no_rows", "bad_padding", "no_columns",
    "no_warmups", "bool_iterations", "no_mode", "bad_backend", "no_available",
    "no_cpu", "negative_cpu", "nan_cpu", "bool_gpu", "infinite_ratio", "missing_delta",
    "no_input", "bool_columns", "no_output", "raw_metrics", "orphan_speedup",
])
def test_non_digest_reports_require_complete_current_contract_in_every_ingress(tmp_path, mutation):
    data = add_synthetic_raw_copy({"producer_schema": "cuda_nested", "benchmarks": flat_report(("fft",))})
    report = data["benchmarks"]
    entry = report["operations"][0]
    if mutation == "no_schema": del data["producer_schema"]
    elif mutation == "alias_schema": data["producer_schema"] = "cuda"
    elif mutation == "nested_schema": report["producer_schema"] = "metal_flat"
    elif mutation == "no_rows": del report["rows"]
    elif mutation == "bad_padding": report["padded_rows"] = 9
    elif mutation == "no_columns": del report["column_count"]
    elif mutation == "no_warmups": del report["warmups"]
    elif mutation == "bool_iterations": report["iterations"] = True
    elif mutation == "no_mode": del report["execution_mode"]
    elif mutation == "bad_backend": report["gpu_backend"] = "metal"
    elif mutation == "no_available": del report["gpu_available"]
    elif mutation == "no_cpu": del entry["cpu_mean_ms"]
    elif mutation == "negative_cpu": entry["cpu_mean_ms"] = -1
    elif mutation == "nan_cpu": entry["cpu_mean_ms"] = float("nan")
    elif mutation == "bool_gpu": entry["gpu_mean_ms"] = True
    elif mutation == "infinite_ratio": entry["speedup_ratio"] = float("inf")
    elif mutation == "missing_delta": del entry["speedup_delta_ms"]
    elif mutation == "no_input": del entry["input_len"]
    elif mutation == "bool_columns": entry["columns"] = True
    elif mutation == "no_output": del entry["output_bytes"]
    elif mutation == "raw_metrics": entry["cpu"] = {"mean_ms": 2}
    else: entry["gpu_mean_ms"] = None
    with pytest.raises(ValueError): projection.project_bundle(data)
    with pytest.raises(ValueError): dashboard.build_markdown(Path("capture.json"), data)
    path = tmp_path / "fastpq_cuda_bench_probe.json"; path.write_text(json.dumps(data))
    with pytest.raises(ValueError): history.collect_benchmark_rows(tmp_path)
    with pytest.raises(ValueError):
        rollout.summarize_bench_entry({"label": "bad", "path": str(path), "operation_filter": "fft"}, bundle_dir=tmp_path, repo_root=tmp_path)


def test_explicit_cpu_producer_schema_preserves_distinct_raw_contracts():
    metal = primitive_report(backend="none")
    metal["producer_schema"] = "metal_flat"
    cuda = deepcopy(metal)
    cuda["producer_schema"] = "cuda_nested"
    del cuda["operations"][0]["gpu_recorded"]
    for field in ("min_ms", "max_ms"): del cuda["operations"][0]["cpu"][field]
    for report in (metal, cuda):
        evidence = projection.project_report(report, flattened=False, producer_schema=report["producer_schema"])
        assert projection.validate_projection(evidence) == evidence
        assert evidence["producer_schema"] == report["producer_schema"]
    with pytest.raises(ValueError, match="producer_schema"):
        projection.project_report(cuda, flattened=False, producer_schema="metal_flat")
    del cuda["producer_schema"]
    with pytest.raises(ValueError, match="gpu_recorded"):
        projection.project_report(cuda, flattened=False, producer_schema="metal_flat")
    with pytest.raises(ValueError, match="producer_schema"): projection.project_bundle(cuda)


@pytest.mark.parametrize("mutation", ["no_cpu", "negative_time", "invalid_range", "flattened_metrics", "bad_count"])
def test_raw_non_digest_report_uses_current_wrapper_validation(mutation):
    raw = primitive_report()
    raw["producer_schema"] = "metal_flat"
    raw["operation_filter"] = "fft"
    entry = raw["operations"][0]
    entry["operation"] = "fft"
    del entry["digest384"]
    del entry["gpu_payload_buffer_bytes"]
    if mutation == "no_cpu": del entry["cpu"]
    elif mutation == "negative_time": entry["gpu"]["mean_ms"] = -1
    elif mutation == "invalid_range": entry["cpu"]["min_ms"] = entry["cpu"]["max_ms"] + 1
    elif mutation == "flattened_metrics": entry["cpu_mean_ms"] = 1
    else: entry["columns"] = True
    with pytest.raises(ValueError): projection.project_bundle(raw)


@pytest.mark.parametrize("field", ["rows", "padded_rows", "column_count", "iterations", "warmups", "gpu_available", "producer_schema"])
def test_manifest_and_rendered_geometry_cannot_disagree_with_retained_evidence(tmp_path, field):
    data = bundle()
    path = tmp_path / "capture.json"; path.write_text(json.dumps(data))
    claims = manifest_entry(data, path)
    expected = data["producer_schema"] if field == "producer_schema" else data["benchmarks"][field]
    wrong = "cuda_nested" if field == "producer_schema" else int(expected) if field == "gpu_available" else expected + 1
    claims[field] = wrong
    with pytest.raises(ValueError, match=field):
        rollout.summarize_bench_entry(claims, bundle_dir=tmp_path, repo_root=tmp_path)
    summary = rollout.summarize_bench_entry(manifest_entry(data, path), bundle_dir=tmp_path, repo_root=tmp_path)
    summary[field] = wrong
    with pytest.raises(ValueError, match=field): rollout.render_markdown({"benches": [summary]})


def test_history_uses_explicit_producer_and_rechecks_displayed_execution_context(tmp_path):
    data = bundle(backend="cuda")
    path = tmp_path / "fastpq_metal_bench_filename_is_not_schema.json"
    path.write_text(json.dumps(data))
    rows = history.collect_benchmark_rows(tmp_path)
    assert rows[0].backend == "cuda"
    history.gpu_table(rows)
    for field, changed in [("backend", "metal"), ("execution_mode", "cpu"), ("gpu_backend", "none"), ("gpu_available", "no")]:
        before = getattr(rows[0], field)
        setattr(rows[0], field, changed)
        with pytest.raises(ValueError, match="context"): history.gpu_table(rows)
        setattr(rows[0], field, before)


def test_actual_fft_lde_staging_is_preserved_in_every_projection(tmp_path):
    data = {"producer_schema": "metal_flat", "benchmarks": complete_flat_report("metal")}
    staging = column_staging()
    data["benchmarks"]["column_staging"] = staging
    add_synthetic_raw_copy(data)
    evidence = projection.project_bundle(data)
    assert evidence["report"]["column_staging"] == staging
    assert projection.validate_projection(evidence) == evidence
    rendered = dashboard.build_markdown(Path("capture.json"), data)
    assert "Column staging overlap: 2 batches, flatten 2.000 ms, wait 1.000 ms" in rendered
    assert json.loads(rendered.split("```json\n", 1)[1].split("\n```", 1)[0])["report"]["column_staging"] == staging
    path = tmp_path / "fastpq_metal_bench_staging.json"; path.write_text(json.dumps(data))
    rows = history.collect_benchmark_rows(tmp_path)
    assert rows[0].operation_evidence["report"]["column_staging"] == staging
    summary = rollout.summarize_bench_entry(manifest_entry(data, path), bundle_dir=tmp_path, repo_root=tmp_path)
    assert summary["operation_evidence"]["report"]["column_staging"] == staging


@pytest.mark.parametrize("group", ["phases", "samples"])
@pytest.mark.parametrize("mutation", ["poseidon", "digest384", "missing", "not_object"])
def test_column_staging_rejects_retired_or_invented_phase_names(tmp_path, group, mutation):
    data = bundle()
    staging = column_staging()
    data["benchmarks"]["column_staging"] = staging
    if mutation == "not_object": staging[group] = []
    elif mutation == "missing": del staging[group]["lde"]
    else: staging[group][mutation] = staging[group]["fft"]
    with pytest.raises(ValueError, match="exactly fft and lde"): projection.project_bundle(data)
    with pytest.raises(ValueError, match="exactly fft and lde"): dashboard.build_markdown(Path("capture.json"), data)
    path = tmp_path / "fastpq_metal_bench_staging.json"; path.write_text(json.dumps(data))
    with pytest.raises(ValueError, match="exactly fft and lde"): history.collect_benchmark_rows(tmp_path)
    with pytest.raises(ValueError, match="exactly fft and lde"):
        rollout.summarize_bench_entry(manifest_entry(data, path), bundle_dir=tmp_path, repo_root=tmp_path)


@pytest.mark.parametrize("location", ["root", "report", "benchmarks"])
@pytest.mark.parametrize("field", ["queue_poseidon", "queue_pipeline", "multiplier", "batch_columns"])
def test_scalar_queue_and_scheduling_claims_reject_in_every_copy(location, field):
    data = bundle(raw_copy=True)
    selected = data if location == "root" else data[location]
    if field.startswith("queue_"):
        selected["metal_dispatch_queue"] = {"poseidon" if field == "queue_poseidon" else "poseidon_pipeline": None}
    else:
        selected["metal_heuristics"] = {"poseidon_batch_multiplier": None} if field == "multiplier" else {"batch_columns": {"poseidon": None}}
    with pytest.raises(ValueError, match="retired"):
        projection.project_bundle(data)
    with pytest.raises(SystemExit, match="retired"):
        wrap_benchmark.normalize_report(data)


@pytest.mark.parametrize("field", ["gpu_mean_ms", "speedup_ratio", "speedup_delta_ms"])
@pytest.mark.parametrize("backend", ["metal", "none", "cuda"])
def test_nullable_flat_metrics_are_explicit_in_both_report_ingresses(field, backend):
    data = bundle(backend=backend, raw_copy=True)
    del data["benchmarks"]["operations"][0][field]
    with pytest.raises(ValueError):
        projection.project_bundle(data)
    with pytest.raises(SystemExit, match=field):
        wrap_benchmark.normalize_report(data)


@pytest.mark.parametrize("field", ["producer_schema", "column_count"])
def test_nested_raw_header_cannot_be_backfilled_from_another_copy(field):
    data = bundle(raw_copy=True)
    del data["report"][field]
    with pytest.raises(ValueError, match=field):
        projection.project_bundle(data)
    with pytest.raises(SystemExit, match=field):
        wrap_benchmark.normalize_report(data)


def test_column_count_copies_cannot_alias_boolean_or_float_values():
    for value in [True, 2.0]:
        data = bundle(raw_copy=True)
        data["benchmarks"]["column_count"] = value
        with pytest.raises(SystemExit, match="column_count"):
            wrap_benchmark.normalize_report(data)


@pytest.mark.parametrize("field", ["column_count", "iterations", "warmups"])
def test_all_operation_headers_reject_counts_outside_native_u64(field):
    report = flat_report(["fft"], backend="metal")
    report[field] = 1 << 64
    with pytest.raises(ValueError, match="u64"):
        projection.project_report(report, flattened=True, producer_schema="metal_flat")


def test_fft_only_header_requires_exact_next_power_of_two_padding():
    report = flat_report(["fft"], backend="metal")
    report["padded_rows"] *= 2
    with pytest.raises(ValueError, match="next_power_of_two"):
        projection.project_report(report, flattened=True, producer_schema="metal_flat")


def test_generic_flat_gpu_mode_cannot_publish_only_cpu_timing():
    report = flat_report(["fft"], backend="metal")
    report["operations"][0].update(gpu_mean_ms=None, speedup_ratio=None, speedup_delta_ms=None)
    with pytest.raises(ValueError, match="availability"):
        projection.project_report(report, flattened=True, producer_schema="metal_flat")


@pytest.mark.parametrize("backend", ["metal", "cuda"])
def test_generic_raw_gpu_timing_must_match_header(backend):
    from scripts.fastpq.tests.test_wrap_benchmark import current_cuda_payload, current_metal_report
    schema = "metal_flat" if backend == "metal" else "cuda_nested"
    report = current_metal_report() if backend == "metal" else current_cuda_payload()["report"]
    report.update(execution_mode="gpu" if backend == "metal" else "cpu", gpu_available=backend == "metal", gpu_backend="metal" if backend == "metal" else "none")
    with pytest.raises(ValueError, match="availability"):
        projection.project_report(report, flattened=False, producer_schema=schema)


@pytest.mark.parametrize("backend", ["metal", "cuda"])
def test_wrapped_reports_require_both_current_copies(backend):
    complete = bundle(backend=backend)
    projection.project_bundle(complete)
    for field in ["report", "benchmarks"]:
        data = deepcopy(complete)
        del data[field]
        with pytest.raises(ValueError):
            projection.project_bundle(data)
        with pytest.raises(SystemExit):
            wrap_benchmark.normalize_report(data)


@pytest.mark.parametrize("mutation", ["missing", "reorder", "duplicate"])
def test_all_filter_requires_the_exact_ordered_six_operation_inventory(mutation):
    data = add_synthetic_raw_copy({"producer_schema": "metal_flat", "benchmarks": complete_flat_report("metal")})
    projection.project_bundle(data)
    for name in ["report", "benchmarks"]:
        entries = data[name]["operations"]
        if mutation == "missing": entries.pop()
        elif mutation == "reorder": entries[0], entries[1] = entries[1], entries[0]
        else: entries[-1] = entries[0]
    with pytest.raises(ValueError, match="inventory|duplicate"):
        projection.project_bundle(data)


@pytest.mark.parametrize("flattened", [False, True])
@pytest.mark.parametrize("field", ["columns", "input_len", "output_len", "input_bytes", "output_bytes", "estimated_gpu_transfer_bytes"])
def test_generic_operation_dimensions_cannot_exceed_native_u64(field, flattened):
    from scripts.fastpq.tests.test_wrap_benchmark import current_cuda_payload
    data = current_cuda_payload()
    report = data["benchmarks" if flattened else "report"]
    report["operations"][0][field] = 1 << 64
    with pytest.raises(ValueError):
        projection.project_report(report, flattened=flattened, producer_schema="cuda_nested")
