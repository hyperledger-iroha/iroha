# FASTPQ V1 native primitive benchmark contract

The maintained producers are `fastpq_metal_bench` and `fastpq_cuda_bench`.
Both call `fastpq_prover::digest384_benchmark` for complete six-lane trace-column
and Merkle-pair measurements. Benchmark schema checks establish internal
consistency. Hardware provenance, proof soundness, admission, full prover
performance and four-validator qualification require their own actual evidence.

## Operations and inputs

The exact operation names are `fft`, `ifft`, `lde`,
`digest384_trace_columns`, `digest384_merkle_pairs`, and `bn254_poseidon_words`.
A filter is exactly `all` or one operation name; aliases are rejected.
`all` requires all six entries in the listed order; partial or reordered
inventories cannot claim complete coverage.
Matrix filter sets are emitted with `all` first when present, followed by the
selected operation names in that same inventory order. Signed manifests reject
empty, duplicate or reordered filter arrays.
BN254 and FFT/LDE arithmetic retain their separate owners.

The report contains `rows`, `padded_rows`, `column_count`, `iterations` and
`warmups`. Counts are native u64 integers, never booleans. Rows are in
1..=65,536; padding is exactly the next power of two, iterations are positive,
and warmups may be zero. Their sum must also fit u64 in every execution mode.
Both native entry points validate these bounds before tracing, device probes or
input allocation. The shared preflight derives padded/LDE extents from the sole
V1 parameter set, checks native count arithmetic, and verifies sample, column,
flattened BN254 and twiddle allocation layouts. CUDA accepts only the exact
canonical parameter name. Representable layouts do not promise available memory.
For trace commitments, exactly `column_count` columns
named `bench_00`, `bench_01`, … contain `padded_rows` canonical Goldilocks values
each. For pairs, exactly `rows` ordered pairs contain two complete six-lane
children each. The benchmark hashes actual canonical frames for the selected
FASTPQ V1 parameter pack and rejects a different parameter pack.

Each six-lane operation records `columns` (digest frames per invocation),
`input_len` (values per column or 12 words per pair), `output_len: 6`,
`input_bytes`, `output_bytes`, and `gpu_payload_buffer_bytes`.
Output size is exactly 48 bytes per frame. The logical GPU buffer size is
`8 * canonical_words + 72 * frames`: canonical input words, three descriptor
words per frame, and all six output words per frame. It excludes allocation
rounding, arguments, constants, readiness checks and erasure traffic. It is not
measured device transfer traffic; these operations reject the retired estimated
transfer field.

## Explicit report representation

The outer `producer_schema` is mandatory: `metal_flat` for Metal and
`cuda_nested` for CUDA. Metal emits a raw report. CUDA emits both `report`
(raw timing objects) and `benchmarks` (flattened metrics). The wrapper retains
both copies for either producer. The nested raw report carries its own matching
`producer_schema` and complete header; fields are never inferred or backfilled.
An optional tag in `benchmarks` must match the outer tag.

Every flattened operation contains `cpu_mean_ms`, `gpu_mean_ms`,
`speedup_ratio` and `speedup_delta_ms`. Absent optional metrics are explicit
JSON nulls only when GPU execution is absent. GPU captures require numeric
values for all three GPU/speedup fields. Ratios are nonnegative; time deltas may
be negative. Raw absent GPU metrics are omitted. Header, operation order, counts,
projected timings and full six-lane evidence agree exactly across copies.
JSON booleans, integers and floating-point values do not alias one another.
The GPU identities are `metal_flat`/`metal` and `cuda_nested`/`cuda`.
Explicit CPU reports use `execution_mode: cpu`, `gpu_backend: none` and
`gpu_available: false`; they contain no GPU timings or digest dispatch claims.
A failed GPU request aborts the capture.

## Six-lane evidence

Each `digest384` object carries exactly the canonical schema, catalog, protocol,
profile, role, phase, level, lane count and output encoding, plus `frame_count`,
`input_field_bytes`, `canonical_words`, `sponge_permutations`, `output_bytes`,
`cpu_reference_verified` and, for successful GPU measurement, `gpu`.
The schema is `fastpq-digest384-primitive-benchmark-v1`, catalog is
`iroha-privacy-exact12-v1`, protocol/profile is
`fastpq-state-transition-stark-v1`, and role is
`fastpq:v1:preprocessing-trace`. The phase/level pairs are `column-leaf`/0
and `binary-node`/1. Encoding is `six-canonical-u64-le-words`, with six lanes.
Canonical word counts come from the actual frame owner; the total permutation
count is three times the word count. Every CPU invocation is checked against
the initial canonical reference.

GPU evidence binds the selected `backend`, `warmup_invocations`,
`timed_invocations`, total `invocations`, successful `dispatches`, `frames`,
`canonical_words`, `descriptor_words`, `output_words`, `max_batch_frames`,
`max_batch_words`, `parity_checked_digests` and `parity_checked_lanes`.
Counts cover every warmup and timed invocation. All outputs and all six lanes
must match CPU results. Batch maxima are observed successful payload callbacks,
bounded by 65,536 frames and 4,194,304 words. Readiness checks and CPU calls
are excluded from device counts. Dispatch failure, noncanonical output,
length/order disagreement or any lane mismatch aborts the operation.

Actual FFT/LDE `column_staging` remains: aggregate batches/flatten/wait metrics,
`phases` and `samples`, each containing exactly `fft` and `lde`. Aggregate
staging includes only those two phases. Each aggregate, phase and sample has
exact fields, bounded u64 counters and finite nonnegative timing values; wait
ratios lie in 0..=1. Six-lane work uses its own dispatch
counters. Scalar microbench, scalar comparison/profile, scalar queue and
unused scalar scheduling projections are rejected. The optional Prometheus
scrape retains its distinct runtime telemetry meaning; it cannot establish
six-lane dispatch or parity.

## Capture and review

Use `--require-gpu` for Metal qualification captures and run CUDA captures on
a CUDA host. `launch_geometry_sweep.py` always requires GPU execution and
accepts FFT/LDE/queue geometry only. A complete geometry matrix requires the full six-operation inventory and GPU
timings for FFT, LDE and both six-lane operations. A focused capture remains
identified by its exact operation filter.
The matrix builder requires wrapped captures with explicit filters and validates
both report copies before collecting samples or deriving thresholds. Every
capture under a device label must name the same backend; CPU, Metal and CUDA
samples cannot be combined under one backend identity.

Wrap actual captures with `scripts/fastpq/wrap_benchmark.py` and retain the raw
report, wrapped JSON, source/build identity, device/toolchain metadata and
signatures. `--require-digest384-columns-mean-ms <reviewed-limit-ms>` checks a
chosen trace-column limit; the historical scalar timing is not such a limit
or a measurement of the new construction. Existing LDE and release-matrix
limits still apply to their named operations.

Rust manifest/profile readers share `scripts/fastpq/src/digest384_report.rs`.
Python wrapper, rollout validation, stage aggregation, geometry, history,
dashboard and release summary consumers validate before rendering or publishing.
History/dashboard summaries retain the full validating operation evidence.
The isolated release runner authenticates these Python module dependencies
before importing them. No retired scalar exporter or report adapter exists.
Historical capture bytes, operation names and measured timings remain historical;
a fresh capture is required for this contract.
