#! FASTPQ V1 Operations Guide

This runbook describes how to build, validate, and operate the sole V1 FASTPQ
prover. V1 has no legacy or placeholder backend. It complements the
implementation boundary in `specs/fastpq_plan.md` and assumes you already track
workspace status in `status.md`.

## Audience & Scope
- Validator operators rolling out the production prover in staging or mainnet environments.
- Release engineers creating binaries or containers that will ship with the production backend.
- SRE/observability teams wiring new telemetry signals and alerting.

Out of scope: Kotodama contract authoring and IVM ABI changes (see `specs/nexus.md` for the
execution model).

## Feature Matrix
| Path | Cargo features to enable | Result | When to use |
| ---- | ----------------------- | ------ | ----------- |
| Production prover (default) | _none_ | V1 FASTPQ backend with FFT/LDE planner and DEEP-FRI pipeline.【crates/fastpq_prover/src/backend.rs:1】 | Default for all production binaries. |
| Optional GPU acceleration | `fastpq_prover/fastpq-gpu` | Enables CUDA/Metal kernels. Production `gpu` mode fails closed when kernels or preflight are unavailable; `cpu` remains the default.【crates/fastpq_prover/Cargo.toml:9】【crates/iroha_core/src/fastpq/lane.rs:228】 | Hosts with supported accelerators. |

## Build Procedure
1. **CPU-only build**
   ```bash
   cargo build --release -p irohad --bin iroha3d
   cargo build --release -p iroha_cli
   ```
   The production backend is compiled in by default; no extra features are required.

2. **GPU-enabled build (optional)**
   ```bash
   cargo build --release -p irohad --bin iroha3d --features fastpq-gpu
   ```
   Linux/NVIDIA builds require an SM80+ CUDA toolkit with `nvcc` available during the build.
   macOS builds use Metal and probe Xcode's optional MetalToolchain as described below.【crates/fastpq_prover/Cargo.toml:11】【crates/fastpq_prover/build.rs:30】

3. **Self-tests**
   ```bash
   cargo test -p fastpq_prover
   ```
   Run this once per release build to confirm the V1 path before packaging.
   The public V1 verifier applies `fastpq_prover::VerifyLimits`, checks the
   canonical batch commitment and public inputs, authenticates sampled LDE query
   chunks against Merkle paths rooted at `lde_root`, and uses the proof's
   `lde_domain_size` when deriving query indices. It rebuilds the canonical
   trace, LDE, and AIR commitments before accepting proof-carried
   openings. Proof roots, FRI layer commitments, and Merkle siblings use the
   canonical `GoldilocksDigest384V1` carrier, so malformed field representatives
   are rejected during construction or decoding. Keep node-facing proof batches
   within the transition-count, payload, query, and path caps. V1 proofs now carry
   exactly 16 AIR composition challenges, sampled AIR trace rows, sampled AIR
   composition openings, and per-round FRI
   openings; the verifier recomputes the sampled AIR composition value from
   opened adjacent rows and requires that value to match the FRI base-layer
   opening.

### Metal toolchain preparation (macOS)
1. Install full Xcode and select it with `xcode-select` (or `DEVELOPER_DIR`); the standalone Command Line Tools package is not sufficient for the offline Metal compiler. The macOS build probes both `metal -v` and `metallib -v` but never installs components or clears system caches. If either tool is unavailable, install it explicitly with `xcodebuild -downloadComponent MetalToolchain`; the build warns and falls back to runtime source compilation in the meantime.【crates/fastpq_prover/build.rs:107】【crates/fastpq_prover/src/backend.rs:716】【crates/fastpq_prover/src/metal.rs:2331】
2. To validate the pipeline ahead of CI, you can mirror the build script locally:
   ```bash
   export OUT_DIR=$PWD/target/metal && mkdir -p "$OUT_DIR"
   xcrun metal -std=macos-metal2.4 -O3 -c -I crates/fastpq_prover/metal/include -I crates/fastpq_prover/metal/kernels crates/fastpq_prover/metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"
   xcrun metal -std=macos-metal2.4 -O3 -c -I crates/fastpq_prover/metal/include -I crates/fastpq_prover/metal/kernels crates/fastpq_prover/metal/kernels/poseidon.metal -o "$OUT_DIR/poseidon.air"
   xcrun metal -std=macos-metal2.4 -O3 -c -I crates/fastpq_prover/metal/include -I crates/fastpq_prover/metal/kernels crates/fastpq_prover/metal/kernels/bn254.metal -o "$OUT_DIR/bn254.air"
   xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon.air" "$OUT_DIR/bn254.air" -o "$OUT_DIR/fastpq.metallib"
   ```
   Release builds should let `build.rs` generate the library and embed its Cargo `OUT_DIR` path at compile time. `FASTPQ_METAL_LIB` is only a debug/dev override, not production configuration; a relocated release whose embedded path is stale compiles the embedded source instead.【crates/fastpq_prover/build.rs:210】【crates/fastpq_prover/src/metal.rs:2475】
3. Set `FASTPQ_SKIP_GPU_BUILD=1` to skip the offline shader build. On macOS this does not disable visible Metal hardware: the runtime compiles the embedded, self-contained source through `MTLDevice`.【crates/fastpq_prover/build.rs:32】【crates/fastpq_prover/src/metal.rs:2348】
4. Nodes configured with `zk.fastpq.execution_mode = "gpu"` fail closed if no usable `MTLDevice` exists, the preferred build-time library cannot load, embedded source/pipeline compilation fails, or parity preflight fails. Nodes configured with `cpu` stay on the deterministic scalar path.【crates/iroha_core/src/fastpq/lane.rs:283】【crates/fastpq_prover/src/metal.rs:2334】

### V1 release checklist
Keep the FASTPQ release ticket blocked until every item below is complete and attached.

1. **Sub-second proof metrics** — Inspect the freshly captured `fastpq_metal_bench_*.json` and
   confirm the `benchmarks.operations` entry where `operation = "lde"` (and the mirrored
   `report.operations` sample) reports `gpu_mean_ms ≤ 950` for the 20 000-row workload (32 768 padded
   rows). Captures outside the ceiling require reruns before the checklist can be signed.
2. **Signed manifest** — Run
   `cargo xtask fastpq-bench-manifest --bench metal=<json> --bench cuda=<json> --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key <path> --out artifacts/fastpq_bench_manifest.json`
   so the release ticket carries both the manifest and its detached signature
   (`artifacts/fastpq_bench_manifest.sig`). Reviewers verify the digest/signature pair before
   promoting a release.【xtask/src/fastpq.rs:128】【xtask/src/main.rs:845】 The matrix manifest,
   built via `scripts/fastpq/capture_matrix.sh`, already encodes the 20 k row floor used by the
   gate.
3. **Evidence attachments** — Upload the Metal benchmark JSON, stdout log (or Instruments trace),
   CUDA/Metal manifest outputs, and the detached signature to the release ticket. The checklist entry
   should link to all artefacts plus the public key fingerprint used for signing so downstream audits
   can replay the verification step.

### Metal validation workflow

Run the native GPU parity suite on the actual device, then capture the complete
V1 workload with `fastpq_metal_bench --require-gpu --rows 20000 --iterations 5`.
Use the maintained Cargo build, `--output <raw.json>` and optional `--trace-dir
<traces>` arguments. Record the actual library/toolchain path and device.
The benchmark aborts when the selected GPU cannot complete the work.

The exact operation selectors, six-lane frame geometry, CPU/device parity
checks and report schema are specified in [the V1 benchmark contract](fastpq_benchmark_v1.md).
`digest384_trace_columns` selects complete named-column commitments;
`digest384_merkle_pairs` selects pairs of complete six-lane children. Generic
FFT/LDE staging and BN254 arithmetic keep their own telemetry.

Wrap the raw capture with `scripts/fastpq/wrap_benchmark.py`, preserving its
explicit producer tag, duplicate report consistency and all six-lane counters.
Use `--require-lde-mean-ms 950` for the existing LDE target and
`--require-digest384-columns-mean-ms <reviewed-limit-ms>` for an explicitly
reviewed trace-column target. Earlier scalar timings cannot qualify that target.
Attach actual decoded row-usage evidence with `--row-usage <snapshot.json>`;
`--sign-output` produces the detached signature through the configured signer.
Repeat on the actual CUDA host and validate both device captures with
`cargo xtask fastpq-bench-manifest`. No synthetic report or CPU measurement
qualifies GPU execution.

### Evidence to archive

Retain the raw and wrapped JSON, exact source/build identity, native parity log,
actual toolchain/device metadata, trace files, decoded row-usage input, signed
matrix/manifest and detached signatures. Preserve failed attempts with their
original identities. Report consistency and primitive parity do not establish
full proof performance, soundness, admission or deployment qualification.


## Reproducible Builds
Use the pinned container workflow to produce reproducible V1 artefacts:

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
   execution_mode = "cpu"   # or "gpu"
   poseidon_mode = "cpu"    # or "gpu"
   proof_sidecar_queue_cap = 1024
   proof_sidecar_max_bytes = "1 MiB"
   proof_sidecar_max_retries = 16
   ```
   The values are parsed through `FastpqExecutionMode`/`FastpqPoseidonMode` and thread into the backend at startup. The sidecar knobs bound local FASTPQ proof persistence in Kura.【crates/iroha_config/src/parameters/user.rs:3964】【crates/iroha_core/src/kura.rs:90】【crates/irohad/src/main.rs:1733】

2. Override at launch if needed:
   ```bash
   iroha3d --fastpq-execution-mode gpu ...
   ```
   CLI overrides mutate the resolved config before the node boots.【crates/irohad/src/main.rs:270】【crates/irohad/src/main.rs:1733】

3. Production node behavior is config/CLI sourced. Low-level prover benches can
   still use `FASTPQ_GPU={auto,cpu,gpu}` for developer diagnostics, but do not rely
   on that environment variable for shipped FASTPQ lane policy.【crates/fastpq_prover/src/backend.rs:208】【crates/iroha_config/src/parameters/user.rs:3964】

## Verification Checklist
1. **Startup logs**
   - Expect `FASTPQ execution mode resolved` from target `telemetry::fastpq.execution_mode` with
     `requested`, `resolved`, and `backend` labels.【crates/fastpq_prover/src/backend.rs:208】
   - GPU-configured nodes surface `backend="metal"` when `MTLDevice` discovery succeeds and either the offline library or embedded source pipelines pass preflight.
   - If compilation, loading, or preflight fails for explicit `gpu`, the FASTPQ lane is disabled instead of silently using CPU.【crates/fastpq_prover/build.rs:29】【crates/iroha_core/src/fastpq/lane.rs:228】【crates/fastpq_prover/src/metal.rs:43】

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
   - macOS `iroha3d` nodes compiled with `--features fastpq-gpu` additionally expose
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
- **Resolved mode stays CPU on GPU hosts** — check that the daemon was built with
  `irohad/fastpq-gpu`, CUDA libraries are on the loader path, and `FASTPQ_GPU` is not forcing
  `cpu`.
- **Metal unavailable on Apple Silicon** — in a debug/dev diagnostic build, set `FASTPQ_DEBUG_METAL_ENUM=1` and verify `MTLCreateSystemDefaultDevice` or `MTLCopyAllDevices` sees the GPU. A missing offline compiler produces a build warning and uses embedded source compilation; install the optional Xcode component explicitly when an offline library is required. Treat a runtime compiler or parity-preflight error as a library/pipeline failure rather than a hardware-discovery failure; `FASTPQ_METAL_LIB` is only a debug/dev override.【crates/fastpq_prover/build.rs:107】【crates/fastpq_prover/src/backend.rs:745】【crates/fastpq_prover/src/metal.rs:2334】
- **`Unknown parameter` errors** — ensure both prover and verifier use the same canonical catalogue
  emitted by `fastpq_isi`; mismatches surface as `Error::UnknownParameter`.【crates/fastpq_prover/src/proof.rs:133】
- **GPU mode disabled at startup** — inspect `cargo tree -p fastpq_prover --features` and
  confirm `fastpq_prover/fastpq-gpu` is present in GPU builds; verify `nvcc`/CUDA libraries or the Metal library are available and that the preflight succeeds.
- **CUDA asynchronous completion failure** — a CUDA stream or event wait is bounded at 120 seconds,
  and every non-success completion status (not only a timeout) quarantines the CUDA backend for the
  rest of the process. Direct CUDA calls return an error, while proof hashing records the dispatch
  failure and uses the deterministic CPU fallback rather than allocating more device buffers or
  freeing resources whose ownership is uncertain.
  This runtime fallback is distinct from mandatory-GPU startup preflight; restart the process only
  after diagnosing the device or driver stall.【crates/fastpq_prover/cuda/fastpq_cuda.cu:22】【crates/fastpq_prover/src/trace.rs:1252】
- **Telemetry counter missing** — verify the node was started with `--features telemetry` (default)
  and that OTEL export (if enabled) includes the metric pipeline.【crates/iroha_telemetry/src/metrics.rs:8887】

## Failure Procedure
There is no compatibility backend. If a regression requires a deployment
rollback, redeploy previously qualified V1 release artefacts and investigate
before issuing a replacement. Confirm telemetry reports the configured CPU or
GPU backend; `backend="none"` is a failure signal, not a usable execution path.

## Hardware Baseline
| Profile | CPU | GPU | Notes |
| ------- | --- | --- | ----- |
| Reference (V1) | AMD EPYC 7B12 (32 cores), 256 GiB RAM | NVIDIA A100 40 GB (CUDA 12.2) | 20 000 row execution-captured V1 batches must complete ≤1 000 ms.【specs/fastpq_plan.md:131】 |
| CPU-only | ≥32 physical cores, AVX2 | – | Expect ~0.9–1.2 s for 20 000 rows; keep `execution_mode = "cpu"` for determinism. |

## Regression Tests
- `cargo test -p fastpq_prover --release`
- `cargo test -p fastpq_prover --release --features fastpq-gpu` (on GPU hosts)
- Optional canonical 64-row raw-transcript fixture check
  (`tests/fixtures/v1_raw_transcript_64.bin`):
  ```bash
  cargo test -p fastpq_prover --features dev-tools --test transcript_replay --release -- --nocapture
  ```

Document any deviations from this checklist in your ops runbook and update
`status.md` after validation completes.
