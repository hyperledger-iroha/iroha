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
| Production transfer prover (default) | _none_ | Canonical masked [DEEP profile](fastpq_deep_protocol_contract.md) with bounded ordinary/AXT verification and explicit source expectations. | Default for supported transfer statements in production binaries. |
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
   macOS builds require a genuinely admitted embedded Metal bundle before device discovery; ordinary builds do not probe or invoke the Metal toolchain. The current bundle is absent. See the explicit producer boundary below.

3. **Self-tests**
   ```bash
   cargo test -p fastpq_prover
   ```
   Run this once per release build to confirm the V1 path before packaging.
   The canonical verifier uses `offline_compact::VerificationLimits`, binds
   every expected public input and complete statement digest, and authenticates
   64 row/quotient-mask queries and five FRI folds. It checks all 923 AIR slots
   at the out-of-domain point and the complete degree-bounded terminal; it
   rebuilds no private witness, trace or full LDE. Roots and siblings retain all
   six canonical words of `GoldilocksDigest384V1`. The child frame ceiling is
   502,895 bytes, and complete wrappers still obey the one-MiB artifact cap.
   AXT adds independently expected binding, manifest, DA, amount and expiry;
   finalized-source verification additionally checks the authoritative anchor
   and ordered source transactions. Metadata-only effects are unsupported by
   the transfer AIR. Full-prover resource and cryptographic qualification remain
   explicit evidence obligations.

### Metal artifact production (macOS)

The sole offline producer is explicitly invoked after selecting an installed full
Xcode toolchain with `xcode-select` or `DEVELOPER_DIR`:

```bash
python3 scripts/build_fastpq_metal_bundle.py \
  --target aarch64-apple-darwin --output target/fastpq-metal-candidate
```

It captures eight ordered source inputs, compiles six modules, records actual
compiler/linker identities and flags, and checks natural terminal success and
currentness before create-only publication. It never installs components, clears
caches, cancels children or downloads tools. `--skip` performs no generation.
The requested target and generated metadata are unqualified observations; they
cannot authorize their own bytes. Genuine repeated generation, independent
admission pins/target review, sixteen real pipeline loads and full output parity
remain required.

Current production has no approved bundle (`None`). Discovery and every context's
common compiled-data loader require the same private immutable admission before
device setup or private staging. Runtime source compilation and library-path
selection are removed. `FASTPQ_SKIP_GPU_BUILD` remains a separate CUDA build
control; it does not provide a Metal source fallback. Explicit GPU policies retain
operational refusal; CPU policies retain deterministic scalar execution. Sticky
uncertain-completion refusal and retained private owners remain, with complete
calibration, native packaging, allocation backing and recovery qualification open.

### V1 release checklist
Keep the FASTPQ release ticket blocked until every item below is complete and attached.

1. **LDE primitive timing** — Inspect the freshly captured `fastpq_metal_bench_*.json` and
   confirm the `benchmarks.operations` entry where `operation = "lde"` (and the mirrored
   `report.operations` sample) reports `gpu_mean_ms ≤ 950` for the 20 000-row workload (32 768 padded
   rows). This measures the LDE primitive, not a complete masked Quantity proof.
   Complete facade timing, memory and artifact evidence remain separate requirements
   in the [production readiness record](fastpq_production_readiness.md).
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
Use the pinned container workflow to produce reproducible V1 artefacts. Install
the shared Python 3.10+ script requirements and select reviewed immutable image
references before running it. The variables below must contain actual
`registry/repository@sha256:<64 lowercase hex digits>` values supplied by the
operator; tags alone are rejected.

```bash
python3 -m pip install -r scripts/requirements.txt
: "${FASTPQ_RUST_IMAGE:?Set the reviewed digest-pinned Rust base image}"
scripts/fastpq/repro_build.sh --mode cpu --rust-image "$FASTPQ_RUST_IMAGE"

# GPU builds also require an independently reviewed CUDA base image.
: "${FASTPQ_CUDA_IMAGE:?Set the reviewed digest-pinned CUDA base image}"
scripts/fastpq/repro_build.sh --mode gpu \
  --rust-image "$FASTPQ_RUST_IMAGE" --cuda-image "$FASTPQ_CUDA_IMAGE" \
  --output artifacts/fastpq-repro-gpu

# Select an installed container runtime explicitly when needed.
scripts/fastpq/repro_build.sh --container-runtime podman \
  --rust-image "$FASTPQ_RUST_IMAGE"
```

The helper derives the exact Rust channel from `rust-toolchain.toml`, passes it to
the selected Docker build stage, and runs the build inside the container. It
writes `manifest.json`, `sha256s.txt`, and the compiled binaries to the target
output directory. The supplied Rust base must provide the expected rustup layout;
the CUDA base must support the Dockerfile's Ubuntu/Debian package setup.
The workflow never substitutes a mutable image tag for a missing digest.

Environment overrides:
- `FASTPQ_RUST_IMAGE`, `FASTPQ_CUDA_IMAGE` – explicit digest-pinned base images.
- `FASTPQ_RUST_TOOLCHAIN` – optional assertion of the checked-in Rust channel;
  a mismatch is rejected before container execution.
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
   - GPU-configured nodes surface `backend="metal"` only after the embedded compiled bundle is admitted, real `MTLDevice` discovery succeeds and original pipeline/parity preflight passes. The current absent bundle does not satisfy this gate.
   - If artifact admission, loading, or preflight fails for explicit `gpu`, the FASTPQ lane is disabled instead of silently using CPU.【crates/fastpq_prover/build.rs:29】【crates/iroha_core/src/fastpq/lane.rs:228】【crates/fastpq_prover/src/metal.rs:43】

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
- **Metal unavailable on Apple Silicon** — first verify the independently admitted embedded bundle. Current source supplies `None`; installing a compiler or supplying a runtime path cannot enable Metal. With an admitted bundle, debug/dev `FASTPQ_DEBUG_METAL_ENUM=1` can diagnose real device discovery. Treat compiled-library loading and parity refusal as operational failures; required GPU policy remains required, with no runtime source compiler.
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

## Hardware and complete-proof measurements

FFT/LDE benchmark rows and timings do not size the fixed masked Quantity relation.
Use the [source-bound readiness evidence](fastpq_production_readiness.md) for
complete ordinary/AXT artifact timing and memory, and qualify the actual deployment
hardware separately. CPU is the default; explicit device execution requires its
own availability, parity and failure checks. No full-proof throughput target is
established by the primitive benchmark above.

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
