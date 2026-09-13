"""Complete synthetic non-cryptographic operation reports for projection tests."""


def flat_report(names=("lde",), backend="cuda", *, rows=8, columns=2, iterations=1, warmups=0, cpu=2.0, gpu=1.0):
    """Return a current wrapper-shaped report with explicit measured counters."""
    padded = 1 << (rows - 1).bit_length()
    entries = []
    for name in names:
        output = padded * (8 if name == "lde" else 1)
        entries.append({
            "operation": name, "columns": columns, "input_len": padded,
            "output_len": output, "input_bytes": padded * columns * 8,
            "output_bytes": output * columns * 8,
            "estimated_gpu_transfer_bytes": (padded + output) * columns * 8,
            "cpu_mean_ms": cpu, "gpu_mean_ms": gpu if backend != "none" else None,
            "speedup_ratio": cpu / gpu if backend != "none" else None,
            "speedup_delta_ms": cpu - gpu if backend != "none" else None,
        })
    return {
        "rows": rows, "padded_rows": padded, "column_count": columns,
        "warmups": warmups, "iterations": iterations,
        "operation_filter": names[0] if len(names) == 1 else "all",
        "execution_mode": "cpu" if backend == "none" else "gpu",
        "gpu_backend": backend, "gpu_available": backend != "none", "operations": entries,
    }


def column_staging():
    """Return the actual two-phase telemetry shape with synthetic timings."""
    phase = {"batches": 1, "flatten_ms": 1.0, "wait_ms": 0.5, "wait_ratio": 0.333}
    sample = {"batch": 0, "flatten_ms": 1.0, "wait_ms": 0.5, "wait_ratio": 0.333}
    return {
        "batches": 2, "flatten_ms": 2.0, "wait_ms": 1.0, "wait_ratio": 0.333,
        "phases": {"fft": dict(phase), "lde": dict(phase)},
        "samples": {"fft": [dict(sample)], "lde": [dict(sample)]},
    }


def add_synthetic_raw_copy(bundle):
    """Give synthetic flat fixtures both required copies before applying mutants."""
    from copy import deepcopy
    assert "report" not in bundle
    report = deepcopy(bundle["benchmarks"])
    report["producer_schema"] = bundle["producer_schema"]
    metal = bundle["producer_schema"] == "metal_flat"
    # Repeated references in a synthetic Python list still serialize as
    # independent JSON objects; retain duplicate-row adversaries as such.
    report["operations"] = [deepcopy(entry) for entry in report["operations"]]
    for entry in report["operations"]:
        cpu = entry.pop("cpu_mean_ms")
        gpu = entry.pop("gpu_mean_ms")
        ratio = entry.pop("speedup_ratio")
        delta = entry.pop("speedup_delta_ms")
        entry["cpu"] = {"mean_ms": cpu}
        if metal:
            entry["cpu"].update(min_ms=cpu, max_ms=cpu)
            entry["gpu_recorded"] = gpu is not None
        if gpu is not None:
            entry["gpu"] = {"mean_ms": gpu}
            entry["speedup"] = {"ratio": ratio}
            if delta is not None:
                entry["speedup"]["delta_ms"] = delta
            if metal:
                entry["gpu"].update(min_ms=gpu, max_ms=gpu)
    bundle["report"] = report
    return bundle


def complete_flat_report(backend="metal", *, rows=8, columns=2, iterations=1, warmups=0):
    """Return all six synthetic operations in their sole maintained order."""
    from scripts.fastpq.benchmark_operations import CANONICAL_OPERATION_ORDER
    from scripts.fastpq.tests.test_digest384_evidence import primitive_report
    from scripts.fastpq import wrap_benchmark
    result = flat_report(("fft", "ifft", "lde", "bn254_poseidon_words"), backend,
                         rows=rows, columns=columns, iterations=iterations, warmups=warmups)
    operations = {entry["operation"]: entry for entry in result["operations"]}
    schema = "cuda_nested" if backend == "cuda" else "metal_flat"
    for name in ("digest384_trace_columns", "digest384_merkle_pairs"):
        report = primitive_report(name, backend, rows=rows, column_count=columns,
                                  iterations=iterations, warmups=warmups)
        entries, _ = wrap_benchmark.summarize_operations(report, schema)
        operations[name] = entries[0]
    result["operations"] = [operations[name] for name in CANONICAL_OPERATION_ORDER]
    return result
