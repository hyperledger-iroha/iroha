---
lang: ur
direction: rtl
source: docs/source/fastpq_migration_guide.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 99e435a831d793035e71915ca567d3e61cb28b89627e0cf0ebdec72aa57a981d
source_last_modified: "2026-01-04T10:50:53.613193+00:00"
translation_last_reviewed: 2026-01-30
---

#! FASTPQ Production Migration Guide

This runbook describes how to validate the Stage 6 production FASTPQ prover.
The deterministic placeholder backend was removed as part of this migration plan.
It complements the staged plan in `docs/source/fastpq_plan.md` and assumes you already track
workspace status in `status.md`.

## Audience & Scope
- Validator operators rolling out the production prover in staging or mainnet environments.
- Release engineers creating binaries or containers that will ship with the production backend.
- SRE/observability teams wiring new telemetry signals and alerting.

Out of scope: Kotodama contract authoring and IVM ABI changes (see `docs/source/nexus.md` for the
execution model).

## Feature Matrix
| Path | Cargo features to enable | Result | When to use |
| ---- | ----------------------- | ------ | ----------- |
| Production prover (default) | _none_ | Stage 6 FASTPQ backend with FFT/LDE planner and DEEP-FRI pipeline.【crates/fastpq_prover/src/backend.rs:1144】 | Default for all production binaries. |
| Optional GPU acceleration | `fastpq_prover/fastpq-gpu` | Enables CUDA/Metal kernels with automatic CPU fallback.【crates/fastpq_prover/Cargo.toml:9】【crates/fastpq_prover/src/fft.rs:124】 | Hosts with supported accelerators. |

## Build Procedure
1. **CPU-only build**
   ```bash
   cargo build --release -p irohad
   cargo build --release -p iroha_cli
   ```
   The production backend is compiled in by default; no extra features are required.

2. **GPU-enabled build (optional)**
   ```bash
   export FASTPQ_GPU=auto        # honour GPU detection at build-time helpers
   cargo build --release -p irohad --features fastpq_prover/fastpq-gpu
   ```
   GPU support requires an SM80+ CUDA toolkit with `nvcc` available during the build.【crates/fastpq_prover/Cargo.toml:11】

3. **Self-tests**
   ```bash
   cargo test -p fastpq_prover
   ```
   Run this once per release build to confirm the Stage 6 path before packaging.

### Metal toolchain preparation (macOS)
1. Install the Metal command-line tools before building: `xcode-select --install` (if the CLI tools are missing) and `xcodebuild -downloadComponent MetalToolchain` to fetch the GPU toolchain. The build script invokes `xcrun metal`/`xcrun metallib` directly and will fail fast if the binaries are absent.【crates/fastpq_prover/build.rs:98】【crates/fastpq_prover/build.rs:121】
2. To validate the pipeline ahead of CI, you can mirror the build script locally:
   ```bash
   export OUT_DIR=$PWD/target/metal && mkdir -p "$OUT_DIR"
   xcrun metal -std=metal3.0 -O3 -c metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"
   xcrun metal -std=metal3.0 -O3 -c metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"
   xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"
   export FASTPQ_METAL_LIB="$OUT_DIR/fastpq.metallib"
   ```
   When this succeeds the build emits `FASTPQ_METAL_LIB=<path>`; the runtime reads that value to load the metallib deterministically.【crates/fastpq_prover/build.rs:188】【crates/fastpq_prover/src/metal.rs:43】
3. Set `FASTPQ_SKIP_GPU_BUILD=1` when cross-compiling without the Metal toolchain; the build prints a warning and the planner remains on the CPU path.【crates/fastpq_prover/build.rs:45】【crates/fastpq_prover/src/backend.rs:195】
4. Nodes fall back to CPU automatically if Metal is unavailable (missing framework, unsupported GPU, or empty `FASTPQ_METAL_LIB`); the build script clears the env var and the planner logs the downgrade.【crates/fastpq_prover/build.rs:29】【crates/fastpq_prover/src/backend.rs:293】【crates/fastpq_prover/src/backend.rs:605】

### Release checklist (Stage 6)
Keep the FASTPQ release ticket blocked until every item below is complete and attached.

1. **Sub-second proof metrics** — Inspect the freshly captured `fastpq_metal_bench_*.json` and
   confirm the `benchmarks.operations` entry where `operation = "lde"` (and the mirrored
   `report.operations` sample) reports `gpu_mean_ms ≤ 950` for the 20 000-row workload (32 768 padded
   rows). Captures outside the ceiling require reruns before the checklist can be signed.
2. **Signed manifest** — Run
   `cargo xtask fastpq-bench-manifest --bench metal=<json> --bench cuda=<json> --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key <path> --out artifacts/fastpq_bench_manifest.json`
   so the release ticket carries both the manifest and its detached signature
   (`artifacts/fastpq_bench_manifest.sig`). Reviewers verify the digest/signature pair before
   promoting a release.【xtask/src/fastpq.rs:128】【xtask/src/main.rs:845】 The matrix manifest (built
   via `scripts/fastpq/capture_matrix.sh`) already encodes the 20 k row floor and
   debugging a regression.
3. **Evidence attachments** — Upload the Metal benchmark JSON, stdout log (or Instruments trace),
   CUDA/Metal manifest outputs, and the detached signature to the release ticket. The checklist entry
   should link to all artefacts plus the public key fingerprint used for signing so downstream audits
   can replay the verification step.【artifacts/fastpq_benchmarks/README.md:65】

### Metal validation workflow
1. After a GPU-enabled build, confirm `FASTPQ_METAL_LIB` points at a `.metallib` (`echo $FASTPQ_METAL_LIB`) so the runtime can load it deterministically.【crates/fastpq_prover/build.rs:188】
2. Run the parity suite with GPU lanes forced on:\
   `FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq_prover/fastpq-gpu --release`. The backend will exercise the Metal kernels and log a deterministic CPU fallback if detection fails.【crates/fastpq_prover/src/backend.rs:114】【crates/fastpq_prover/src/metal.rs:418】
3. Capture a benchmark sample for dashboards:\
   locate the compiled Metal library (`fd -g 'fastpq.metallib' target/release/build | head -n1`),
   export it via `FASTPQ_METAL_LIB`, and run\
  `cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release -- --rows 20000 --iterations 5 --output fastpq_metal_bench.json --trace-dir traces`.
  The canonical `fastpq-lane-balanced` profile now pads every capture to 32,768 rows (2¹⁵), so the JSON carries both `rows` and `padded_rows` along with the Metal LDE latency; rerun the capture if `zero_fill` or queue settings push the GPU LDE beyond the 950 ms (<1 s) target on Apple M-series hosts. Archive the resulting JSON/log alongside other release evidence; the nightly macOS workflow performs the same run and uploads its artefacts for comparison.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:697】【.github/workflows/fastpq-metal-nightly.yml:1】
  When you need Poseidon-only telemetry (e.g., to record an Instruments trace), add `--operation poseidon_hash_columns` to the command above; the bench will still respect `FASTPQ_GPU=gpu`, emit `metal_dispatch_queue.poseidon`, and include the new `poseidon_profiles` block so the release bundle documents the Poseidon bottleneck explicitly.
  Evidence now includes `zero_fill.{bytes,ms,queue_delta}` plus `kernel_profiles` (per-kernel
  occupancy, estimated GB/s, and duration stats) so GPU efficiency can be graphed without
  reprocessing raw traces, and a `twiddle_cache` block (hits/misses + `before_ms`/`after_ms`) that
  proves the cached twiddle uploads are in effect. `--trace-dir` relaunches the harness under
  `xcrun xctrace record` and
  stores a timestamped `.trace` file alongside the JSON; you can still provide a bespoke
  `--trace-output` (with optional `--trace-template` / `--trace-seconds`) when capturing to a
  custom location/template. The JSON records `metal_trace_{template,seconds,output}` for auditing.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:177】
  After each capture run `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 fastpq_metal_bench.json artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json` so the publication carries host metadata (now including `metadata.metal_trace`) for the Grafana board/alerting bundle (`dashboards/grafana/fastpq_acceleration.json`, `dashboards/alerts/fastpq_acceleration_rules.yml`). The report now carries a `speedup` object per operation (`speedup.ratio`, `speedup.delta_ms`), the wrapper hoists `zero_fill_hotspots` (bytes, latency, derived GB/s, and the Metal queue delta counters), flattens `kernel_profiles` into `benchmarks.kernel_summary`, keeps the `twiddle_cache` block intact, copies the new `post_tile_dispatches` block/summary so reviewers can prove the multi-pass kernel ran during the capture, and now summarizes the Poseidon microbench evidence into `benchmarks.poseidon_microbench` so dashboards can quote the scalar-vs-default latency without reparsing the raw report. The manifest gate reads the same block and rejects GPU evidence bundles that omit it, forcing operators to refresh captures whenever the post-tiling path is skipped or misconfigured.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1048】【scripts/fastpq/wrap_benchmark.py:714】【scripts/fastpq/wrap_benchmark.py:732】【xtask/src/fastpq.rs:280】
  The Poseidon2 Metal kernel shares the same knobs: `FASTPQ_METAL_POSEIDON_LANES` (32–256, powers of two) and `FASTPQ_METAL_POSEIDON_BATCH` (1–32 states per lane) let you pin the launch width and per-lane work without rebuilding; the host threads those values through `PoseidonArgs` before every dispatch. By default the runtime inspects `MTLDevice::{is_low_power,is_headless,location}` to bias discrete GPUs toward VRAM-tiered launches (`256×24` when ≥48 GiB is reported, `256×20` at 32 GiB, `256×16` otherwise) while low-power SoCs remain on `256×8` (and older 128/64 lane parts stick to 8/6 states per lane), so most operators never need to set the env vars manually.【crates/fastpq_prover/src/metal_config.rs:78】【crates/fastpq_prover/src/metal.rs:1971】 `fastpq_metal_bench` now re-executes itself under `FASTPQ_METAL_POSEIDON_MICRO_MODE={default,scalar}` and emits a `poseidon_microbench` block that records both launch profiles plus the measured speedup versus the scalar lane so release bundles can prove the new kernel actually shrinks `poseidon_hash_columns`, and it includes the `poseidon_pipeline` block so Stage 7 evidence captures the chunk depth/overlap knobs alongside the new occupancy tiers. Leave the env unset for normal runs; the harness manages the re-execution automatically, logs failures if the child capture cannot run, and exits immediately when `FASTPQ_GPU=gpu` is set but no GPU backend is available so silent CPU fallbacks never sneak into perf artefacts.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1691】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1988】
  The wrapper rejects Poseidon captures that are missing the `metal_dispatch_queue.poseidon` delta, the shared `column_staging` counters, or the `poseidon_profiles`/`poseidon_microbench` evidence blocks so operators must refresh any capture that fails to prove overlapping staging or the scalar-vs-default speedup.【scripts/fastpq/wrap_benchmark.py:732】 When you need a standalone JSON for dashboards or CI deltas, run `python3 scripts/fastpq/export_poseidon_microbench.py --bundle <bundle>`; the helper accepts both wrapped artifacts and raw `fastpq_metal_bench*.json` captures, emitting `benchmarks/poseidon/poseidon_microbench_<timestamp>.json` with the default/scalar timings, tuning metadata, and recorded speedup.【scripts/fastpq/export_poseidon_microbench.py:1】
  Finish the run by executing `cargo xtask fastpq-bench-manifest --bench metal=artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --bench cuda=artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key secrets/fastpq_bench.ed25519 --out artifacts/fastpq_bench_manifest.json` so the Stage 6 release checklist enforces the `<1 s` LDE ceiling and emits a signed manifest/digest bundle that ships with the release ticket.【xtask/src/fastpq.rs:1】【artifacts/fastpq_benchmarks/README.md:65】
4. Verify telemetry before rollout: curl the Prometheus endpoint (`fastpq_execution_mode_total{device_class="<matrix>", backend="metal"}`) and inspect `telemetry::fastpq.execution_mode` logs for unexpected `resolved="cpu"` entries.【crates/iroha_telemetry/src/metrics.rs:8887】【crates/fastpq_prover/src/backend.rs:174】
5. Document the CPU fallback path by forcing it intentionally (`FASTPQ_GPU=cpu` or `zk.fastpq.execution_mode = "cpu"`) so SRE playbooks stay aligned with the deterministic behaviour.【crates/fastpq_prover/src/backend.rs:308】【crates/iroha_config/src/parameters/user.rs:1357】
6. Optional tuning: by default the host selects 16 lanes for short traces, 32 for medium, and 64/128 once `log_len ≥ 10/14`, landing at 256 when `log_len ≥ 18`, and it now keeps the shared-memory tile at five stages for small traces, four once `log_len ≥ 12`, and 12/14/16 stages for `log_len ≥ 18/20/22` before kicking work to the post-tiling kernel. Export `FASTPQ_METAL_FFT_LANES` (power-of-two between 8 and 256) and/or `FASTPQ_METAL_FFT_TILE_STAGES` (1–16) before running the steps above to override those heuristics. Both the FFT/IFFT and LDE column batch sizes derive from the resolved threadgroup width (≈2 048 logical threads per dispatch, capped at 32 columns, and now ratcheting down through 32→16→8→4→2→1 as the domain grows) while the LDE path still enforces its domain caps; set `FASTPQ_METAL_FFT_COLUMNS` (1–32) to pin a deterministic FFT batch size and `FASTPQ_METAL_LDE_COLUMNS` (1–32) to apply the same override to the LDE dispatcher when you need bit-for-bit comparisons across hosts. The LDE tile depth mirrors the FFT heuristics as well—traces with `log₂ ≥ 18/20/22` only run 12/10/8 shared-memory stages before handing the wide butterflies to the post-tiling kernel—and you can override that limit via `FASTPQ_METAL_LDE_TILE_STAGES` (1–32). The runtime threads all values through the Metal kernel args, clamps unsupported overrides, and logs the resolved values so experiments remain reproducible without rebuilding the metallib; the benchmark JSON surfaces both the resolved tuning and the host zero-fill budget (`zero_fill.{bytes,ms,queue_delta}`) captured via LDE stats so queue deltas are tied directly to each capture, and now adds a `column_staging` block (batches flattened, flatten_ms, wait_ms, wait_ratio) so reviewers can verify the host/device overlap introduced by the double-buffered pipeline. When the GPU refuses to report zero-fill telemetry, the harness now synthesizes a deterministic timing from host-side buffer clears and injects it into the `zero_fill` block so release evidence never ships without the field.【crates/fastpq_prover/src/metal_config.rs:15】【crates/fastpq_prover/src/metal.rs:742】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:575】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1609】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1860】
7. Multi-queue dispatch is automatic on discrete Macs: when `Device::is_low_power()` returns false or the Metal device reports a Slot/External location the host instantiates two `MTLCommandQueue`s, only fans out once the workload carries ≥16 columns (scaled by the fan-out), and round-robins the column batches across queues so long traces keep both GPU lanes busy without compromising determinism. Override the policy with `FASTPQ_METAL_QUEUE_FANOUT` (1–4 queues) and `FASTPQ_METAL_COLUMN_THRESHOLD` (minimum total columns before fan-out) whenever you need reproducible captures across machines; the parity tests force those overrides so multi-GPU Macs stay covered and the resolved fan-out/threshold are logged next to the queue-depth telemetry.【crates/fastpq_prover/src/metal.rs:620】【crates/fastpq_prover/src/metal.rs:900】【crates/fastpq_prover/src/metal.rs:2254】

### Evidence to archive
| Artefact | Capture | Notes |
|----------|---------|-------|
| `.metallib` bundle | `xcrun metal -std=metal3.0 -O3 -c metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"` and `xcrun metal -std=metal3.0 -O3 -c metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"` followed by `xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"` and `export FASTPQ_METAL_LIB=$OUT_DIR/fastpq.metallib`. | Proves the Metal CLI/toolchain was installed and produced a deterministic library for this commit.【crates/fastpq_prover/build.rs:98】【crates/fastpq_prover/build.rs:188】 |
| Environment snapshot | `echo $FASTPQ_METAL_LIB` after the build; keep the absolute path with your release ticket. | Empty output means Metal was disabled; recording the value documents that GPU lanes remain available on the shipping artefact.【crates/fastpq_prover/build.rs:188】【crates/fastpq_prover/src/metal.rs:43】 |
| GPU parity log | `FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq-gpu --release` and archive the snippet that contains `backend="metal"` or the downgrade warning. | Demonstrates that kernels run (or fall back deterministically) before you promote the build.【crates/fastpq_prover/src/backend.rs:114】【crates/fastpq_prover/src/backend.rs:195】 |
| Benchmark output | `FASTPQ_METAL_LIB=$(fd -g 'fastpq.metallib' target/release/build | head -n1) cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release -- --rows 20000 --iterations 5 --output fastpq_metal_bench.json --trace-dir traces`; wrap and sign via `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 fastpq_metal_bench.json artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --sign-output [--gpg-key <fingerprint>]`. | The wrapped JSON records `speedup.ratio`, `speedup.delta_ms`, FFT tuning, padded rows (32,768), enriched `zero_fill`/`kernel_profiles`, the flattened `kernel_summary`, the verified `metal_dispatch_queue.poseidon`/`poseidon_profiles` blocks (when `--operation poseidon_hash_columns` is used), and the trace metadata so the GPU LDE mean stays ≤950 ms and Poseidon stays <1 s; keep both the bundle and the generated `.json.asc` signature with the release ticket so dashboards and auditors can verify the artefact without rerunning workloads.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:697】【scripts/fastpq/wrap_benchmark.py:714】【scripts/fastpq/wrap_benchmark.py:732】 |
| Bench manifest | `cargo xtask fastpq-bench-manifest --bench metal=artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --bench cuda=artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key secrets/fastpq_bench.ed25519 --out artifacts/fastpq_bench_manifest.json`. | Validates both GPU artefacts, fails if the LDE mean breaks the `<1 s` ceiling, records BLAKE3/SHA-256 digests, and emits a signed manifest so the release checklist cannot advance without verifiable metrics.【xtask/src/fastpq.rs:1】【artifacts/fastpq_benchmarks/README.md:65】 |
| CUDA bundle | Run `FASTPQ_GPU=gpu cargo run -p fastpq_prover --bin fastpq_cuda_bench --release -- --rows 20000 --iterations 5 --column-count 16 --device 0 --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json` on the SM80 lab host, wrap/sign the JSON into `artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json` (use `--label device_class=xeon-rtx-sm80` so dashboards pick up the correct class), add the path to `artifacts/fastpq_benchmarks/matrix/devices/xeon-rtx-sm80.txt`, and keep the `.json`/`.asc` pair with the Metal artefact before regenerating the manifest. The checked-in `fastpq_cuda_bench_2025-11-12T090501Z_ubuntu24_x86_64.json` illustrates the exact bundle format auditors expect.【scripts/fastpq/wrap_benchmark.py:714】【artifacts/fastpq_benchmarks/matrix/devices/xeon-rtx-sm80.txt:1】 |
| Telemetry proof | `curl -s http://<host>:8180/metrics | rg 'fastpq_execution_mode_total{device_class'` plus the `telemetry::fastpq.execution_mode` log emitted at startup. | Confirms Prometheus/OTEL expose `device_class="<matrix>", backend="metal"` (or a downgrade log) before enabling traffic.【crates/iroha_telemetry/src/metrics.rs:8887】【crates/fastpq_prover/src/backend.rs:174】 |
| Forced CPU drill | Run a short batch with `FASTPQ_GPU=cpu` or `zk.fastpq.execution_mode = "cpu"` and capture the downgrade log. | Keeps SRE runbooks aligned with the deterministic fallback path in case a rollback is needed mid-release.【crates/fastpq_prover/src/backend.rs:308】【crates/iroha_config/src/parameters/user.rs:1357】 |
| Trace capture (optional) | Repeat a parity test with `FASTPQ_METAL_TRACE=1 FASTPQ_GPU=gpu …` and save the emitted dispatch trace. | Preserves occupancy/threadgroup evidence for later profiling reviews without rerunning benchmarks.【crates/fastpq_prover/src/metal.rs:346】【crates/fastpq_prover/src/backend.rs:208】 |

The multilingual `fastpq_plan.*` files reference this checklist so staging and production operators follow the same evidence trail.【docs/source/fastpq_plan.md:1】

## Reproducible Builds
Use the pinned container workflow to produce reproducible Stage 6 artefacts:

```bash
scripts/fastpq/repro_build.sh --mode cpu                     # CPU-only toolchain
scripts/fastpq/repro_build.sh --mode gpu --output artifacts/fastpq-repro-gpu
scripts/fastpq/repro_build.sh --container-runtime podman     # Explicit runtime override
```

The helper script builds the `rust:1.88.0-slim-bookworm` toolchain image (and `nvidia/cuda:12.2.2-devel-ubuntu22.04` for GPU), runs the build inside the container, and writes `manifest.json`, `sha256s.txt`, and the compiled binaries to the target output directory.【scripts/fastpq/repro_build.sh:1】【scripts/fastpq/run_inside_repro_build.sh:1】【scripts/fastpq/docker/Dockerfile.gpu:1】

Environment overrides:
- `FASTPQ_RUST_IMAGE`, `FASTPQ_RUST_TOOLCHAIN` – pin an explicit Rust base/tag.
- `FASTPQ_CUDA_IMAGE` – swap the CUDA base when producing GPU artefacts.
- `FASTPQ_CONTAINER_RUNTIME` – force a specific runtime; default `auto` tries `FASTPQ_CONTAINER_RUNTIME_FALLBACKS`.
- `FASTPQ_CONTAINER_RUNTIME_FALLBACKS` – comma-separated preference order for runtime auto-detection (defaults to `docker,podman,nerdctl`).

## Configuration Updates
1. Set the runtime execution mode in your TOML:
   ```toml
   [zk.fastpq]
   execution_mode = "auto"   # or "cpu"/"gpu"
   ```
   The value is parsed through `FastpqExecutionMode` and threads into the backend at startup.【crates/iroha_config/src/parameters/user.rs:1357】【crates/irohad/src/main.rs:1733】

2. Override at launch if needed:
   ```bash
   irohad --fastpq-execution-mode gpu ...
   ```
   CLI overrides mutate the resolved config before the node boots.【crates/irohad/src/main.rs:270】【crates/irohad/src/main.rs:1733】

3. Developers can temporarily coerce detection without touching configs by exporting
   `FASTPQ_GPU={auto,cpu,gpu}` before launching the binary; the override is logged and the pipeline
   still surfaces the resolved mode.【crates/fastpq_prover/src/backend.rs:208】【crates/fastpq_prover/src/backend.rs:401】

## Verification Checklist
1. **Startup logs**
   - Expect `FASTPQ execution mode resolved` from target `telemetry::fastpq.execution_mode` with
     `requested`, `resolved`, and `backend` labels.【crates/fastpq_prover/src/backend.rs:208】
   - On automatic GPU detection a secondary log from `fastpq::planner` reports the final lane.
   - Metal hosts surface `backend="metal"` when the metallib loads successfully; if compilation or loading fails the build script emits a warning, clears `FASTPQ_METAL_LIB`, and the planner records `GPU acceleration unavailable` before staying on CPU.【crates/fastpq_prover/build.rs:29】【crates/fastpq_prover/src/backend.rs:174】【crates/fastpq_prover/src/backend.rs:195】【crates/fastpq_prover/src/metal.rs:43】

2. **Prometheus metrics**
   ```bash
   curl -s http://localhost:8180/metrics | rg 'fastpq_execution_mode_total{device_class'
   ```
   The counter is incremented via `record_fastpq_execution_mode` (now labeled by
   `{device_class,chip_family,gpu_kind}`) whenever a node resolves its execution
   mode.【crates/iroha_telemetry/src/metrics.rs:8887】
   - For Metal coverage confirm
     `fastpq_execution_mode_total{device_class="<matrix>",chip_family="<family>",gpu_kind="<kind>", backend="metal"}`
     increments alongside your deployment dashboards.【crates/iroha_telemetry/src/metrics.rs:5397】
   - macOS nodes compiled with `irohad --features fastpq-gpu` additionally expose
     `fastpq_metal_queue_ratio{device_class="<matrix>",chip_family="<family>",gpu_kind="<kind>",queue="global",metric="busy"}`
     and
     `fastpq_metal_queue_depth{device_class="<matrix>",chip_family="<family>",gpu_kind="<kind>",metric="limit"}` so the Stage 7 dashboards
     can track duty-cycle and queue headroom from live Prometheus scrapes.【crates/iroha_telemetry/src/metrics.rs:4436】【crates/irohad/src/main.rs:2345】

3. **Telemetry export**
   - OTEL builds emit `fastpq.execution_mode_resolutions_total` with the same labels; ensure your
     dashboards or alerts watch for unexpected `resolved="cpu"` when GPUs should be active.

4. **Sanity prove/verify**
   - Run a small batch through `iroha_cli` or an integration harness and confirm proofs verify on a
     peer compiled with the same parameters.

## Troubleshooting
- **Resolved mode stays CPU on GPU hosts** — check that the binary was built with
  `fastpq_prover/fastpq-gpu`, CUDA libraries are on the loader path, and `FASTPQ_GPU` is not forcing
  `cpu`.
- **Metal unavailable on Apple Silicon** — verify the CLI tools are installed (`xcode-select --install`), rerun `xcodebuild -downloadComponent MetalToolchain`, and ensure the build produced a non-empty `FASTPQ_METAL_LIB` path; an empty or missing value disables the backend by design.【crates/fastpq_prover/build.rs:166】【crates/fastpq_prover/src/metal.rs:43】
- **`Unknown parameter` errors** — ensure both prover and verifier use the same canonical catalogue
  emitted by `fastpq_isi`; mismatches surface as `Error::UnknownParameter`.【crates/fastpq_prover/src/proof.rs:133】
- **Unexpected CPU fallback** — inspect `cargo tree -p fastpq_prover --features` and
  confirm `fastpq_prover/fastpq-gpu` is present in GPU builds; verify `nvcc`/CUDA libraries are on the search path.
- **Telemetry counter missing** — verify the node was started with `--features telemetry` (default)
  and that OTEL export (if enabled) includes the metric pipeline.【crates/iroha_telemetry/src/metrics.rs:8887】

## Fallback Procedure
The deterministic placeholder backend has been removed. If a regression requires rollback,
redeploy the previously known-good release artefacts and investigate before reissuing Stage 6
binaries. Document the change management decision and ensure forward roll completes only after the
regression is understood.

3. Monitor telemetry to ensure `fastpq_execution_mode_total{device_class="<matrix>", backend="none"}` reflects the expected
   placeholder execution.

## Hardware Baseline
| Profile | CPU | GPU | Notes |
| ------- | --- | --- | ----- |
| Reference (Stage 6) | AMD EPYC 7B12 (32 cores), 256 GiB RAM | NVIDIA A100 40 GB (CUDA 12.2) | 20 000 row synthetic batches must complete ≤1 000 ms.【docs/source/fastpq_plan.md:131】 |
| CPU-only | ≥32 physical cores, AVX2 | – | Expect ~0.9–1.2 s for 20 000 rows; keep `execution_mode = "cpu"` for determinism. |

## Regression Tests
- `cargo test -p fastpq_prover --release`
- `cargo test -p fastpq_prover --release --features fastpq_prover/fastpq-gpu` (on GPU hosts)
- Optional golden fixture check:
  ```bash
  cargo test -p fastpq_prover --test backend_regression --release -- --ignored
  ```

Document any deviations from this checklist in your ops runbook and update `status.md` after the
migration window completes.
