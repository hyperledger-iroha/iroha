# Poseidon Benchmarks

This directory stores exported Poseidon benchmark summaries captured on the GPU lanes.

## Capturing Criterion CPU/CUDA results

1. Run `cargo bench -p ivm bench_poseidon -- --save-baseline poseidon_cuda` on the GPU worker.
2. Copy the relevant JSON summaries from `target/criterion/**/new/benchmark.json` into this directory.
   Suggested naming: `criterion_poseidon2_many_cuda.json`, `criterion_poseidon6_many_cuda.json`.
3. Commit the files so downstream teams can compare CPU vs CUDA throughput.

## Automated CUDA parity + throughput (xtask)

Use the dedicated xtask helper to capture scalar vs CUDA timings and parity checks in one run:

```bash
cargo xtask poseidon-cuda-bench \
  --json-out benchmarks/poseidon/poseidon_cuda_latest.json \
  --markdown-out benchmarks/poseidon/poseidon_cuda_latest.md \
  --allow-overwrite
```

The command seeds deterministic inputs for Poseidon2 and Poseidon6, records CUDA health
(feature flag, availability, last error) plus Metal runtime status, checks parity against
the scalar path, and emits ops/sec plus speedups into JSON + Markdown under `benchmarks/poseidon/`. CUDA-free hosts
still write the scalar baseline and note the missing accelerator, making the artefact usable
in CI and on CPU-only machines.

## Exporting Poseidon microbench evidence

Wrapped FASTPQ bundles now contain a `benchmarks.poseidon_microbench` block (default lane,
scalar lane, tuning, and the measured speedup). Extract the summary with:

```bash
python3 scripts/fastpq/export_poseidon_microbench.py \
  --bundle artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json \
  --output benchmarks/poseidon/poseidon_microbench_<date>.json
```

Omit `--output` to have the script drop the file under `benchmarks/poseidon/` with a timestamped
name automatically. The helper can convert raw captures for archival analysis, but current release
evidence must come from a wrapped bundle and carry explicit `operation_filter` and `column_count`
metadata. The history updater rejects exports missing either field. The generated JSON keeps the
metadata (host labels, backend, capture time) so dashboards and reviewers can compare complete
microbench records without unpacking the full bundle.

## Aggregating manifests

To produce a single manifest summarizing every export (latest entry first), run:

```bash
python3 scripts/fastpq/aggregate_poseidon_microbench.py \
  --input benchmarks/poseidon \
  --output benchmarks/poseidon/manifest.json
```

The checked-in 2025 summaries and manifest are retained as archival captures; they predate the
current required metadata and are not inputs to release gates or the current history updater.
Regenerate the exports and manifest from current wrapped bundles before using them in CI or a
release report.

## Generating CSV summaries

Downstream dashboards expect CSV exports that mirror each JSON summary. Run:

```bash
python3 scripts/benchmarks/export_csv.py
```

to produce `poseidon_microbench_*.csv` files (default + scalar rows with shared metadata) alongside
the JSON captures. The helper also converts the Merkle sweep JSON artefacts under
`benchmarks/merkle_threshold/` so CPU-only baselines share the same format.
