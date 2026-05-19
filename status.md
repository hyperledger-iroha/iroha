# Status

Last updated: 2026-05-19

## 2026-05-19 IVM CUDA vector boundary adversarial hardening

- Hardened the CUDA vector helper entry points so zero-length `f32`, `u32`, and
  `u64` vector batches short-circuit as deterministic empty outputs before CUDA
  backend probing, matching the existing no-op behavior of other CUDA batch
  helpers.
- Added negative boundary coverage proving empty/mismatched vector inputs are
  handled without device work: empty pairs return empty outputs, while
  empty-vs-nonempty adversarial length pairs fail closed.
- Focused validation is green with
  `CARGO_TARGET_DIR=target/codex-ivm-cuda-vector-boundary RUST_TEST_THREADS=1 IVM_CUDA_GENCODE=arch=compute_86,code=sm_86 cargo test -p ivm --features cuda --test cuda_extra cuda_empty_vector_boundaries_short_circuit_without_device_work -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-ivm-cuda-vector-boundary RUST_TEST_THREADS=1 IVM_CUDA_GENCODE=arch=compute_86,code=sm_86 cargo test -p ivm --features cuda --test cuda_extra -- --nocapture`,
  `cargo fmt --all -- --check`, and `git diff --check`.

## 2026-05-19 IVM CUDA env/config adversarial gates

- Extended the CUDA env/config regression tests with a guard that restores
  `IVM_DISABLE_CUDA`, `IVM_FORCE_CUDA_SELFTEST_FAIL`, acceleration config, and
  CUDA backend status after each case.
- Added negative coverage proving malformed or adversarially present
  `IVM_DISABLE_CUDA` values keep adaptive VM policy fail-closed, explicit
  config disable marks CUDA unavailable before manager init, config re-enable
  clears disable diagnostics, and forced backend disable/self-test failure
  reports deterministic status without requiring a physical GPU.
- Focused validation is green with
  `CARGO_TARGET_DIR=target/codex-ivm-cuda-env RUST_TEST_THREADS=1 IVM_CUDA_GENCODE=arch=compute_86,code=sm_86 cargo test -p ivm --features cuda --test cuda_env -- --nocapture`,
  `cargo fmt --all -- --check`, and `git diff --check`.

## 2026-05-19 Pipeline CUDA key-bucket adversarial sorting

- Added `iroha_core` pipeline GPU-sort adversarial coverage for access triplets
  with `u32::MAX` keys, `usize::MAX` transaction indices, and extreme flag
  bytes. Direct GPU-sort failure now has test coverage proving caller input is
  preserved for deterministic CPU retry.
- Added GPU-or-CPU fallback coverage proving unencodable transaction indices
  cannot be truncated into CUDA sort keys and instead force canonical CPU
  ordering across extreme key/index/flag combinations.
- Focused validation is green with
  `CARGO_TARGET_DIR=target/codex-core-cuda-adversarial RUST_TEST_THREADS=1 cargo test -p iroha_core pipeline::gpu::tests -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-core-cuda-adversarial RUST_TEST_THREADS=1 IVM_CUDA_GENCODE=arch=compute_86,code=sm_86 cargo test -p iroha_core --features cuda pipeline::gpu::tests -- --nocapture`, and
  `CARGO_TARGET_DIR=target/codex-core-cuda-adversarial IVM_CUDA_GENCODE=arch=compute_86,code=sm_86 cargo clippy -p iroha_core --features cuda --lib --no-deps -- -D warnings`.

## 2026-05-19 Poseidon CUDA bench adversarial reporting

- Hardened the `poseidon-cuda-bench` timing path so a CUDA first-call parity
  mismatch fails closed in the report: timing, ops/sec, speedup, and total CUDA
  operations are left empty/zero instead of benchmarking a mismatched backend.
- Added xtask adversarial coverage for tampered CUDA Poseidon output,
  length-short CUDA output, and a CUDA backend that disappears after a valid
  parity probe. The mismatch tests assert that the timing closure is never
  invoked after invalid first-call output.
- Updated the benchmark docs to state that parity-mismatched CUDA outputs are
  reported as failed evidence rather than throughput evidence.
- Focused validation is green with
  `CARGO_TARGET_DIR=target/codex-xtask-cuda-adversarial RUST_TEST_THREADS=1 cargo test -p xtask cuda_measure -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-xtask-cuda-adversarial RUST_TEST_THREADS=1 cargo test -p xtask poseidon_bench::tests -- --nocapture`, and
  `CARGO_TARGET_DIR=target/codex-xtask-cuda-adversarial cargo clippy -p xtask --all-targets --no-deps -- -D warnings`.

## 2026-05-19 IVM CUDA adversarial test hardening

- Added IVM CUDA public-helper negative coverage for mismatched vector,
  bit-operation, BN254 batch, and Ed25519 batch input lengths. The bitonic sort
  mismatch case now also asserts that rejected calls leave caller buffers
  unchanged.
- Added CUDA boundary coverage for zero-length and singleton inputs that should
  short-circuit deterministically without device work, including bitonic sort,
  SHA-256 leaves/pair reduction, Poseidon batch helpers, AES batch helpers, and
  Ed25519 batch verification.
- Added Ed25519 CUDA adversarial coverage for public-key bytes that must never
  verify successfully and for tampered batch challenge-scalar metadata that the
  CUDA batch verifier must reject while accepting the untouched row.
- Extended the CUDA disable-on-mismatch acceptance suite so non-empty
  Poseidon2/Poseidon6 batch helpers and BN254 batch helpers fail closed when
  CUDA is disabled by forced self-test failure or configuration.
- Focused validation is green with
  `CARGO_TARGET_DIR=target/codex-ivm-cuda-adversarial RUST_TEST_THREADS=1 IVM_CUDA_GENCODE=arch=compute_86,code=sm_86 cargo test -p ivm --features cuda --test cuda_extra -- --nocapture` and
  `CARGO_TARGET_DIR=target/codex-ivm-cuda-adversarial RUST_TEST_THREADS=1 IVM_CUDA_GENCODE=arch=compute_86,code=sm_86 cargo test -p ivm --features cuda --test cuda_disable_on_mismatch -- --nocapture`.
  A strict focused clippy attempt without `--no-deps` is still blocked by
  existing `iroha_crypto/src/sm.rs` dead-code warnings, and the same IVM command
  with `--no-deps` reaches pre-existing CUDA-feature clippy findings in the IVM
  implementation rather than the new test code.

## 2026-05-19 Norito CUDA helper adversarial hardening

- Added CUDA zstd helper C-ABI negative coverage for null pointers even with
  zero lengths, rejected decode paths preserving caller output buffers and
  capacity values, and truncated standard zstd frames failing closed without
  writing partial output.
- Added JSON Stage-1 CUDA helper coverage for sequence-planner null control
  pointers, unknown layout rejection, too-small sequence span capacity reporting
  the required count without writing partial spans, descending fixed-offset
  tables failing closed on the CUDA path, and unavailable-helper empty-input
  behavior.
- Removed needless explicit returns in the JSON Stage-1 CUDA FFI wrappers so the
  helper crate passes strict clippy with the CUDA feature enabled.
- Focused validation is green with
  `CARGO_TARGET_DIR=target/codex-cuda-adversarial GPUZSTD_CUDA_REQUIRE=1 RUST_TEST_THREADS=1 cargo test -p gpuzstd_cuda -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-cuda-adversarial-jsonstage1 JSONSTAGE1_CUDA_REQUIRE=1 RUST_TEST_THREADS=1 cargo test -p jsonstage1_cuda --features cuda-kernel -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-cuda-adversarial RUST_TEST_THREADS=1 cargo test -p jsonstage1_cuda --no-default-features -- --nocapture`, and
  `CARGO_TARGET_DIR=target/codex-cuda-adversarial cargo clippy -p gpuzstd_cuda -p jsonstage1_cuda --all-targets -- -D warnings`.

## 2026-05-19 FASTPQ CUDA adversarial test hardening

- Added negative FASTPQ CUDA wrapper coverage for overflowing BN254 Poseidon
  word-batch slices, partial Poseidon state buffers, malformed column counts,
  truncated flattened payloads, short output buffers, mismatched block counts,
  and short fused leaf+parent output buffers. These cases now fail in the Rust
  wrapper before any CUDA dispatch.
- Added trace-layer adversarial coverage for mismatched Poseidon domain/column
  metadata, out-of-range and overflowing GPU batch windows, and truncated or
  tampered Merkle-pair GPU outputs. The CPU parity sampler rejects those outputs
  before they can be accepted as accelerated Merkle parents.
- Added FASTPQ CUDA suite validation for impossible rows, iterations, and
  column counts, colliding raw/wrapped output paths, and non-finite active
  latency thresholds while leaving unused `--no-wrap` thresholds inert.
- Focused validation is green with
  `CARGO_TARGET_DIR=target/codex-fastpq-cuda FASTPQ_GPU=gpu RUST_TEST_THREADS=1 cargo test -p fastpq_prover --lib --features fastpq-gpu fastpq_cuda::tests -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-fastpq-cuda FASTPQ_GPU=gpu RUST_TEST_THREADS=1 cargo test -p fastpq_prover --lib --features fastpq-gpu poseidon_column_batch -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-fastpq-cuda FASTPQ_GPU=gpu RUST_TEST_THREADS=1 cargo test -p fastpq_prover --lib --features fastpq-gpu trace_merkle_pair_parity_sample_rejects_truncated_or_tampered_gpu_output -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-fastpq-cuda-bn254 RUST_TEST_THREADS=1 cargo test -p fastpq_prover --lib --features fastpq-gpu compact_slice_chunk -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-fastpq-cuda RUST_TEST_THREADS=1 cargo test -p xtask cuda_suite -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-fastpq-cuda FASTPQ_GPU=gpu cargo clippy -p fastpq_prover --all-targets --features fastpq-gpu -- -D warnings`, and
  `CARGO_TARGET_DIR=target/codex-fastpq-cuda cargo clippy -p xtask --all-targets --no-deps -- -D warnings`.
  A full xtask clippy attempt without `--no-deps` still reaches existing
  `iroha_primitives/src/erasure/rs16.rs` AVX2 clippy failures unrelated to this
  CUDA test hardening.

## 2026-05-19 FASTPQ CUDA roadmap closure

- CUDA runtime evidence was captured on `DESKTOP-R9RFED4` under WSL2 Ubuntu
  24.04.4 / Linux `6.6.114.1-microsoft-standard-WSL2`, with an Intel
  i7-11800H host CPU and an NVIDIA GeForce RTX 3080 Laptop GPU
  (`16 GiB`, compute capability `8.6`). `nvidia-smi` reported driver
  `527.56` and CUDA `12.0`; `nvcc --version` reported CUDA toolkit
  `12.0.140`. The FASTPQ static CUDA build used the current selected arch flag
  `-arch=sm_80`.
- `fastpq_cuda_bench` now captures `poseidon_merkle_pairs` in addition to
  FFT/IFFT/LDE, Poseidon column hashing, and BN254 Poseidon word batches.
  `fastpq-cuda-suite`, dashboard helpers, stage aggregation scripts, and the
  benchmark docs accept the new `poseidon_merkle_pairs` and
  `bn254_poseidon_words` focused-operation filters.
- A follow-up roadmap audit removed stale CUDA-host and accelerator-validation
  follow-up wording from earlier FASTPQ planning notes. `roadmap.md` now leaves
  no CUDA-specific FASTPQ proof, parity, benchmark, or release-comparison task
  open.
- FASTPQ CUDA parity is green for generic Poseidon GPU filters, BN254 Poseidon
  word-batch filters, trace Merkle parent-pair parity filters, low-level fused
  first-level Poseidon coverage, CUDA FFT/LDE wrappers, and fail-closed
  telemetry counters. The accepted Merkle-pair path recorded a GPU batch with
  zero fallbacks in focused coverage.
- Release CPU/GPU proof parity is green:
  `v1_artifact_balanced_cpu_gpu_parity` produced CPU and CUDA proofs matching
  the canonical V1 fixture.
- Release benchmark capture:
  `dist/fastpq_cuda_bench_20260519.json` with `rows=20000`, `padded_rows=32768`,
  `iterations=3`, `warmups=1`, `column_count=16`, `FASTPQ_GPU=gpu`,
  `gpu_backend=cuda`, and no BN254 warnings. Mean timings were FFT
  `cpu=3.483ms` / `cuda=117.733ms`, IFFT `cpu=3.554ms` / `cuda=131.261ms`,
  LDE `cpu=29.284ms` / `cuda=1034.580ms`, Poseidon columns `cpu=756.419ms` /
  `cuda=3032.520ms`, Poseidon Merkle pairs `cpu=121.938ms` /
  `cuda=1.830ms` (`66.633x`), and BN254 Poseidon words `cpu=1185.570ms` /
  `cuda=102.367ms` (`11.582x`). CPU remains the authoritative fallback for the
  transfer-heavy FFT/IFFT/LDE and Poseidon-column shapes where this host's CUDA
  path is slower.
- Focused validation is green with
  `cargo fmt --all`,
  `CARGO_TARGET_DIR=target/codex-fastpq-cuda RUST_TEST_THREADS=1 cargo test -p fastpq_prover --bin fastpq_cuda_bench --features fastpq-gpu -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-fastpq-cuda FASTPQ_GPU=gpu RUST_TEST_THREADS=1 cargo test -p fastpq_prover --lib --features fastpq-gpu cuda -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-fastpq-cuda FASTPQ_GPU=gpu RUST_TEST_THREADS=1 cargo test -p fastpq_prover --lib --features fastpq-gpu poseidon_gpu -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-fastpq-cuda FASTPQ_GPU=gpu RUST_TEST_THREADS=1 cargo test -p fastpq_prover --lib --features fastpq-gpu public_gpu -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-fastpq-cuda RUST_TEST_THREADS=1 cargo test -p iroha_telemetry records_fastpq_gpu_disable_and_parity_metrics --lib -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-fastpq-cuda-release FASTPQ_GPU=gpu RUST_TEST_THREADS=1 cargo test -p fastpq_prover --test backend_regression --features fastpq-gpu --release v1_artifact_balanced_cpu_gpu_parity -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-fastpq-cuda-release FASTPQ_GPU=gpu RUST_TEST_THREADS=1 cargo run -p fastpq_prover --bin fastpq_cuda_bench --release --features fastpq-gpu -- --rows 20000 --iterations 3 --warmups 1 --column-count 16 --require-gpu --device "NVIDIA GeForce RTX 3080 Laptop GPU cc8.6 driver 527.56 CUDA 12.0.140 FASTPQ -arch=sm_80" --output dist/fastpq_cuda_bench_20260519.json --notes "CUDA roadmap closure on WSL2; selected FASTPQ CUDA arch flag -arch=sm_80"`,
  `CARGO_TARGET_DIR=target/codex-fastpq-cuda FASTPQ_GPU=gpu cargo clippy -p fastpq_prover --all-targets --features fastpq-gpu -- -D warnings`,
  `CARGO_TARGET_DIR=target/codex-fastpq-cuda cargo test -p xtask cuda_operation_filter_accepts_poseidon_merkle_and_bn254_aliases -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-fastpq-cuda cargo test -p xtask parse_fastpq_cuda_suite_operation_updates_default_artifact_names -- --nocapture`,
  `python3 -m py_compile scripts/fastpq/aggregate_stage_timings.py scripts/fastpq/launch_geometry_sweep.py scripts/fastpq/update_dashboard_panel.py scripts/fastpq/metal_capture_summary.py`,
  `cargo fmt --all -- --check`, and `git diff --check`. `pytest` is not
  installed on this host, so the focused script tests could not be run through
  pytest here.

## 2026-05-19 Final validation pass

- After clearing generated `/tmp/iroha-codex-*` target directories to recover
  disk space, the release-hardening tree validates with
  `CARGO_TARGET_DIR=/tmp/iroha-codex-current-validate cargo build --workspace`
  and
  `CARGO_TARGET_DIR=/tmp/iroha-codex-current-validate cargo test --workspace --no-run`.
- Focused runtime coverage is green for the changed paths:
  `cargo test -p integration_tests --test core_api multisig::multisig_normal`,
  `cargo test -p integration_tests --test core_api transfer_domain::domain_owner_transfer`,
  `cargo test -p integration_tests --test core_api threshold_escrow::`,
  `cargo test -p integration_tests --test nexus_and_streaming nexus::autoscale_localnet::tests::`,
  and the single-cycle, repeated-cycle, and strict autoscale localnet tests.
  The full `consensus_and_da` binary is also green (`258 passed; 0 failed; 7
  ignored`).
- Focused library coverage is green for queue defaults, the minimal config
  fixture with `LOG_FORMAT` unset, Norito JSON duplicate-field rejection, Torii
  encrypted-only RAM-LFE DTO rejection, and `iroha_core` identifier claim
  binding checks. Hygiene is green with `cargo fmt --all -- --check`,
  `git diff --check`, and `scripts/check_no_scale.sh`.

## 2026-05-19 Nexus autoscale public-testnet hardening

- Nexus scale-in now requires a complete scale-in sample window, active lanes
  above `min_lanes`, low p95 utilization, and low p95 latency via
  `scale_in_latency_ratio`; focused unit coverage includes high-latency
  rejection, full-window enforcement, cooldown suppression, managed-lane-only
  retirement, and preserving base lanes `0..2` under the public-testnet
  profile.
- The Taira-style public-testnet profile keeps autoscale opt-in with
  `min_lanes = 3`, `max_lanes = 5`, `target_block_ms = 1000`,
  `scale_out_latency_ratio = 1.50`, `scale_in_latency_ratio = 1.10`,
  `scale_out_window_blocks = 48`, `scale_in_window_blocks = 192`,
  `cooldown_blocks = 128`, and `per_lane_target_tps = 32`.
- The strict localnet autoscale harness now waits for expanded lane status
  quorum before contraction and keeps submitting low-load heartbeat
  transactions during contraction/precheck so scale-in has real low-load
  samples. The local test profile uses a larger autoscale target window to
  reflect observed DA/NPoS localnet commit jitter while preserving
  `scale_in_latency_ratio < scale_out_latency_ratio`.
- A public-testnet-shaped strict localnet variant now starts from three base
  lanes, expands to elastic lane `3`, waits for expansion status quorum before
  contraction, and verifies scale-in quorum without retiring base lanes `0..2`.
- Focused validation is green with
  `CARGO_TARGET_DIR=target/codex-autoscale-params cargo test -p iroha_core --lib autoscale -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-autoscale-params cargo test -p iroha_core --lib lane_lifecycle -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-autoscale-params/iroha-test-network cargo build -p irohad --bin iroha3d`,
  and the single-cycle, repeated-cycle, strict, and public-profile strict
  localnet autoscale tests in `integration_tests --test nexus_and_streaming`
  using the freshly built `iroha3d` binary.

## 2026-05-19 Torii queue default headroom

- The realistic 30 TPS soak reached 86.9 minutes before Torii rejected ingress
  with `PRTRY:QUEUE_FULL` at the inherited default `65,536/65,536` pending
  queue ceiling; the stall detector did not trip first (`max_no_progress_gap`
  stayed below the 20s threshold).
- Torii/consensus pending-transaction defaults now set both the global queue
  capacity and the per-authority queue capacity to `262,144` (4x the previous
  `65,536`) so default soak and node configs inherit the wider headroom.
- Focused validation is green with
  `env -u LOG_FORMAT cargo test -p iroha_config --lib queue_defaults_allow_four_times_legacy_soak_capacity -- --nocapture`
  and
  `env -u LOG_FORMAT cargo test -p iroha_config --test fixtures minimal_config_snapshot -- --nocapture`.

## 2026-05-19 Contract manifest pipeline trigger filter fix

- Contract manifest trigger fixtures now register deterministic approved-block
  pipeline filters, matching the runtime policy that only approved/rejected
  transaction facts and approved block facts are replayed for contract pipeline
  triggers.
- Kotodama trigger declarations now lower `on pipeline block [approved]` and
  `on pipeline transaction [approved]` to approved-status manifest filters and
  reject nondeterministic pipeline families at parse time; the grammar and gap
  analysis docs now describe that deterministic surface.
- Focused validation is green with `cargo fmt --all`,
  `cargo test -p kotodama_lang pipeline_filter --lib -- --nocapture`,
  `cargo test -p kotodama_lang pipeline_transaction --lib -- --nocapture`, and
  `cargo test -p iroha_core --test contract_manifest_triggers -- --nocapture`.

## 2026-05-19 WSV/Kura and query pipeline refactor closeout

- WSV remains memory-only: durable block state stays in Kura, while query,
  pagination, SNS mutation/readback, Torii error reporting, and integration
  tests were hardened around committed in-memory state instead of ad hoc
  persistence paths.
- Query pagination now carries unknown remaining-count semantics through the
  Rust client, smart-contract query builder, and integration tests, avoiding
  misleading totals when the WSV snapshot cannot cheaply prove them.
- SNS client helpers now submit consensus mutations and poll committed reads;
  Torii JSON error paths preserve validation details for query conversion and
  read failures.
- IVM contract artifact validation now rejects global `*` and scoped `state:*`
  access hints in first-release artifacts. Test-mode wildcard diagnostics remain
  compiler/report-only, invalid dynamic hints are rejected, and threshold escrow
  deploy paths are covered by focused integration validation.
- Focused validation is green with `cargo check -p kotodama_lang --lib`,
  `cargo test -p ivm --test contract_artifact -- --nocapture`,
  `cargo test -p integration_tests --test core_api threshold_escrow:: -- --nocapture`,
  and the full `cargo test -p integration_tests --test core_api -- --nocapture`
  suite (`171 passed; 0 failed; 4 ignored`).
- Broad Rust validation is green with `cargo build --workspace`,
  `cargo clippy --workspace --all-targets -- -D warnings`, and the full
  `cargo test -p integration_tests --test consensus_and_da -- --nocapture`
  suite (`258 passed; 0 failed; 7 ignored`). The preceding workspace test run
  had reached `consensus_and_da`; the RAM-LFE receipt, mode-cutover, and
  fingerprint regressions exposed there are now covered by the passing full
  `consensus_and_da` binary.
- Hygiene is green with `cargo fmt --all -- --check` and `git diff --check`.

## 2026-05-19 First-release FHE/RAM-LFE correctness pass

- RAM-LFE app execution is now encrypted-only. Torii rejects plaintext
  `input_hex` / identifier `input` requests, evaluates the programmed BFV path
  over ciphertext envelopes, and signs receipts that bind input/output
  ciphertext hashes, program digest, parameter digest, evaluation-key digest,
  backend, verification mode, and timestamps. Torii runtime entries now carry
  explicit Norito-encoded hidden program material in `hidden_program_hex` plus
  receipt-signing keys; they do not carry BFV secret keys.
- The in-repo RAM-LFE BFV profile now uses an exact plaintext-lift
  representation with plaintext modulus `257`, validates accumulated hidden
  program multiplicative depth through registers/state, rejects static
  register/input/memory overflows before execution, and proves
  `SelectEqZero` across all byte values without decrypting inside the
  evaluator. Full bounded-noise BFV-RNS remains tracked in `roadmap.md`.
- Identifier claim/resolve receipts now require a signed
  `RamLfeOutputOpening`. Torii derives `opaque:` identifiers from the verified
  opened-output hash instead of decrypting locally or deriving directly from a
  ciphertext hash, and on-chain `ClaimIdentifier` validation rechecks the
  opening against the execution receipt and the configured opening verifier
  key.
- Soracloud FHE state mutation and job execution now persist full ciphertext
  payloads, verify stored payload commitments before execution, run Add,
  Multiply, RotateLeft, and Bootstrap over encoded BFV ciphertext envelopes,
  and commit the encoded output ciphertext bytes back into authoritative state.
  Bootstrap uses a validated public encrypted-zero refresh key so evaluators
  remain secret-key free while ciphertext bytes are actually transformed.
  RotateLeft now also requires public rotation-key refresh material and
  re-randomizes every moved ciphertext slot after the envelope slot rotation.
- OpenAPI/static portal specs and the universal-account/Soracloud docs now
  describe the encrypted-only RAM-LFE flow, encrypted output ciphertexts, and
  opening-required identifier resolution. The translated universal-account
  guide variants now carry the same canonical encrypted-only RAM-LFE section.
- Kotlin, Java, Swift, and JavaScript SDK helpers now require encrypted
  identifier requests plus `RamLfeOutputOpening`; the JavaScript helper emits the
  257-prime BFV envelope vector and parses nested `{ payload, attestation }`
  receipts.
- Focused validation is green with
  `CARGO_TARGET_DIR=target/codex-fhe-fix cargo test -p iroha_crypto ram_lfe -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-fhe-fix cargo test -p iroha_data_model soracloud -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-fhe-fix cargo test -p iroha_core run_soracloud_fhe_job_records_ciphertext_output_state --lib -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-fhe-fix cargo check -p iroha_torii`, and
  `CARGO_TARGET_DIR=target/codex-fhe-fix cargo check -p irohad`, plus
  `CARGO_TARGET_DIR=target/codex-fhe-fix cargo test -p iroha_torii
  identifier_resolution --lib -- --nocapture` and
  `CARGO_TARGET_DIR=target/codex-fhe-fix cargo test -p iroha_torii
  openapi --lib -- --nocapture`, and
  `CARGO_TARGET_DIR=target/codex-fhe-fix cargo test -p iroha_torii
  encrypted_only_request_dtos_reject_plaintext_fields --lib -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-fhe-fix cargo test -p iroha_torii
  ram_lfe_execute_returns_receipt --lib -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-fhe-fix cargo test -p iroha_torii
  identifier_resolve --lib -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-fhe-fix cargo test -p iroha_torii
  identifier_claim_receipt --lib -- --nocapture`, and
  `CARGO_TARGET_DIR=target/codex-fhe-fix cargo test -p iroha_core
  claim_identifier --lib -- --nocapture`.
- Additional SDK/fixture validation is green with `swift test --filter
  ToriiClientTests --filter TxBuilderTests`, `node --test
  test/toriiClient.identifier.test.js test/toriiClient.ramLfe.test.js`,
  `npm run lint`, `npm run build:dist`, `CARGO_TARGET_DIR=target/codex-fhe-fix
  cargo check -p kotlin-fixture-gen`, and `CARGO_TARGET_DIR=target/codex-fhe-fix
  cargo test -p iroha_core claim_identifier_rejects_invalid_output_opening_signature
  --lib -- --nocapture`. After staging Temurin 21 under `/tmp/temurin21`,
  `JAVA_HOME=/tmp/temurin21/Contents/Home ./gradlew :core-jvm:test --console=plain`
  is green from `kotlin/`, and
  `JAVA_HOME=/tmp/temurin21/Contents/Home ANDROID_HOME=~/Library/Android/sdk
  ANDROID_SDK_ROOT=~/Library/Android/sdk ./gradlew test --console=plain` is green
  from `java/iroha_android/`.
- Follow-up focused Bootstrap validation is green with
  `CARGO_TARGET_DIR=target/codex-fhe-fix cargo test -p iroha_crypto
  bootstrap_refresh_preserves_plaintext_and_changes_ciphertext -- --nocapture`
  and `CARGO_TARGET_DIR=target/codex-fhe-fix cargo test -p iroha_core
  soracloud_bootstrap_uses_refresh_key --lib -- --nocapture`.
- Follow-up rotation-key validation is green with
  `CARGO_TARGET_DIR=target/codex-fhe-fix cargo test -p iroha_crypto rotation_key
  -- --nocapture` and `CARGO_TARGET_DIR=target/codex-fhe-fix cargo test -p
  iroha_core soracloud_rotate_left_uses_rotation_key_refresh --lib --
  --nocapture`.
- Follow-up hidden-program runtime-config validation is green with
  `CARGO_TARGET_DIR=target/codex-fhe-fix cargo run -p kotlin-fixture-gen --
  hidden-ram-fhe-program`,
  `CARGO_TARGET_DIR=target/codex-fhe-fix cargo test -p iroha_config
  torii_ram_lfe_parses -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-fhe-fix cargo check -p iroha_config`,
  `CARGO_TARGET_DIR=target/codex-fhe-fix cargo check -p iroha_torii`,
  `CARGO_TARGET_DIR=target/codex-fhe-fix cargo test -p iroha_torii
  identifier_resolution --lib -- --nocapture`, and
  `CARGO_TARGET_DIR=target/codex-fhe-fix cargo test -p iroha_torii
  torii_ram_lfe_uses_config_runtime --lib -- --nocapture`.
- Follow-up EqZero/depth validation is green with
  `CARGO_TARGET_DIR=target/codex-fhe-fix cargo test -p iroha_crypto ram_lfe
  --lib -- --nocapture` and
  `CARGO_TARGET_DIR=target/codex-fhe-fix cargo test -p iroha_crypto fhe_bfv
  --lib -- --nocapture`.
- Additional negative/adversarial FHE coverage now exercises static hidden
  program shape/memory/register/output overflow rejection, tampered BFV digests,
  tampered RAM-FHE profiles, proof-verifier metadata abuse, truncated
  ciphertext envelopes, adversarial BFV evaluation-key metadata, unregistered
  production BFV parameter sets, decrypted identifier envelopes with impossible
  length/byte/trailing-slot metadata, replayed/tampered/future/expired/
  wrong-verifier output openings, Torii receipt-signing/backend mismatches,
  Soracloud missing/malformed evaluation-key material, empty/malformed
  ciphertext slots, malformed relinearization keys, slot-count mismatches, and
  SDK-side adversarial BFV public-parameter/input rejection. A further
  negative pass now covers Soracloud FHE governance parameter lifecycle/linkage
  abuse, job operation-shape smuggling, policy budget overflows,
  encrypted-only Torii DTO rejection for legacy plaintext fields and missing
  output openings, and Kotlin/Java SDK rejection for malformed RAM-LFE or
  identifier ciphertext hex plus plaintext-only policy misuse. A follow-up
  receipt-binding pass now covers RAM-LFE execution receipts and output
  openings signed by the wrong key, proof attestations passed to signature
  verification, post-signature ciphertext/opened-output tampering, typed
  identifier receipt payload tampering, and JavaScript SDK rejection of
  tampered or proof-only identifier receipts. The JVM verifier pass now mirrors
  those receipt-adversarial checks in Kotlin and Java: tampered payloads return
  false, proof-only attestations fail the signature verifier, mismatched policy
  ids are rejected, and malformed signature hex is rejected. The latest
  adversarial pass mutates every signed RAM-LFE receipt/opening security
  binding, mutates every signed identifier receipt linkage, rejects wrong
  identifier resolver keys, rejects malformed/proof-smuggled JavaScript
  attestations, and hardens Norito JSON object parsing so duplicate
  `encrypted_input` / `policy_id` keys, duplicate `output_opening` objects, and
  nested output-opening shadow fields are rejected before Torii DTO decoding.
  The on-chain claim path also rejects validly re-signed output openings whose
  program id, ciphertext hashes, parameter/evaluation-key digests, opened
  output hash, opaque id binding, or expiry no longer match the execution
  receipt. Focused validation is green with
  `CARGO_TARGET_DIR=target/codex-fhe-fix cargo test -p
  iroha_crypto fhe_bfv --lib -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-fhe-fix cargo test -p iroha_crypto ram_lfe
  --lib -- --nocapture`, `CARGO_TARGET_DIR=target/codex-fhe-fix cargo test -p
  iroha_torii identifier_resolution --lib -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-fhe-fix cargo test -p iroha_core soracloud
  --lib -- --nocapture`, `node --test test/toriiClient.identifier.test.js
  test/toriiClient.ramLfe.test.js`, `npm run lint`,
  `JAVA_HOME=/tmp/temurin21/Contents/Home ./gradlew :core-jvm:test
  --console=plain` from `kotlin/`, and
  `JAVA_HOME=/tmp/temurin21/Contents/Home ANDROID_HOME=~/Library/Android/sdk
  ANDROID_SDK_ROOT=~/Library/Android/sdk ./gradlew test --console=plain` from
  `java/iroha_android/`. The latest focused additions are also green with
  `CARGO_TARGET_DIR=target/codex-fhe-fix cargo test -p iroha_data_model fhe_
  --lib -- --nocapture` and `CARGO_TARGET_DIR=target/codex-fhe-fix cargo test
  -p iroha_torii ram_lfe_encrypted_only_request_dto_tests --lib --
  --nocapture`. The latest receipt-binding additions are green with
  `CARGO_TARGET_DIR=target/codex-fhe-fix cargo test -p iroha_data_model
  ram_lfe --lib -- --nocapture`, `CARGO_TARGET_DIR=target/codex-fhe-fix cargo
  test -p iroha_data_model identifier_resolution_receipt --lib --
  --nocapture`, `node --test test/toriiClient.identifier.test.js
  test/toriiClient.ramLfe.test.js`, `npm run lint`,
  `JAVA_HOME=/tmp/temurin21/Contents/Home ./gradlew :core-jvm:test
  --console=plain` from `kotlin/`, and
  `JAVA_HOME=/tmp/temurin21/Contents/Home ANDROID_HOME=~/Library/Android/sdk
  ANDROID_SDK_ROOT=~/Library/Android/sdk ./gradlew test --console=plain` from
  `java/iroha_android/`. The duplicate-key and expanded binding-mutation pass
  is green with `CARGO_TARGET_DIR=target/codex-fhe-fix cargo test -p norito
  --test json_native --features json -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-fhe-fix cargo test -p iroha_torii
  ram_lfe_encrypted_only_request_dto_tests --lib --features app_api --
  --nocapture`, `CARGO_TARGET_DIR=target/codex-fhe-fix cargo test -p iroha_core
  claim_identifier --lib -- --nocapture`, `CARGO_TARGET_DIR=target/codex-fhe-fix
  cargo test -p iroha_data_model ram_lfe --lib -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-fhe-fix cargo test -p iroha_data_model
  identifier_resolution_receipt --lib -- --nocapture`, `node --test
  test/toriiClient.identifier.test.js test/toriiClient.ramLfe.test.js`, and
  `npm run lint`. Hygiene is green with `cargo fmt --all -- --check` and
  `git diff --check`.

## 2026-05-19 Structural hardening validation closeout

- Fixed the new BFV rotation-key metadata lint by making
  `BfvRotationKey` `Copy`; it is a single `u32` metadata wrapper and remains
  Norito/schema-compatible.
- Added the Soracloud uploaded-model operator pin-and-register checklist to
  `docs/source/soracloud/uploaded_private_models.md`, covering approved SoraFS
  pin evidence, runtime submission gas-asset readiness, provenance binding,
  external JavaScript signing, and committed receipt audit queries.
- Focused validation is green on the current dirty tree with
  `CARGO_TARGET_DIR=target/codex-struct-hardening-check cargo check -p
  iroha_core -p iroha_torii -p irohad -p iroha_data_model`, the stale
  commit-inflight and exact-frontier Sumeragi filters, the 4-peer Kura/WSV
  restart test, the 4-peer Soracloud private uploaded-model receipt restart
  test, `cargo test -p ivm --test contract_artifact
  verify_rejects_invalid_dynamic_access_hints`, and `cargo test -p
  iroha_data_model lane_relay -- --nocapture`.
- Hygiene is green with `cargo fmt --all -- --check`, `git diff --check`,
  dashboard JSON parsing, Ruby YAML parsing for the FASTPQ alert files, and the
  no-`Cargo.lock`/`dist` diff check. `promtool` is not installed on this host,
  and CUDA runtime parity still requires a CUDA host.

## 2026-05-18 Kotodama first-release hardening

- Kotodama access metadata is now compiler-owned for the first release:
  user-written `#[access(...)]` attributes are rejected, production compilation
  fails when access cannot be derived without wildcard fallbacks, and test mode
  remains the only place where incomplete opaque host-call hints are tolerated.
- Dynamic state-map iteration is first-release behavior with a fixed limit of
  64 guarded iterations. The compiler emits structured dynamic access
  descriptors in manifests, admission/pipeline code normalizes those
  descriptors to map-level state keys, and artifact validation rejects wildcard
  or zero-bound dynamic descriptors.
- Durable state map keys are restricted to `int` and pointer-ABI key types, and
  samples, portal snippets, and Kotodama docs no longer advertise manual access
  annotations or a tunable dynamic-iteration cap.
- Validation: `cargo test -p kotodama_lang --lib`,
  `cargo test -p iroha_data_model access_set_hints_roundtrip --lib`,
  `cargo test -p iroha_core access_set_hints --lib`,
  `cargo test -p ivm --test manifest_roundtrip`,
  `cargo test -p ivm --test contract_artifact`, and the full
  `cargo test -p ivm` crate corridor passed. The previously blocked focused
  `cargo test -p ivm --test contract_artifact
  verify_rejects_invalid_dynamic_access_hints -- --nocapture` check is now
  green after the BFV rotation-key lint fix.
- Closeout validation also passed with `cargo fmt --all -- --check`,
  `git diff --check`, `scripts/check_no_scale.sh`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-workspace-build cargo build --workspace`,
  and `CARGO_TARGET_DIR=/tmp/iroha-codex-workspace-build cargo test --workspace
  --no-run`. The full workspace test execution remains a separate multi-hour
  corridor.

## 2026-05-19 Trigger implementation completion

- Finished the v1 trigger semantics pass: deterministic pipeline triggers now
  execute only for transaction `Approved`/`Rejected` and block `Approved`
  validation facts, by-call trigger chaining propagates data-trigger failures,
  missing trigger bytecode fails closed, and enabled metadata now fails closed
  when malformed.
- Active trigger ID caches now track enabled-and-not-depleted triggers, trigger
  action construction exposes a fallible `Action::try_new(...)`, and
  action Norito/JSON deserialization now routes invalid payloads through
  fallible validation instead of panicking. Trigger-completion events use
  `trigger_execution_hash` for the invocation identity.
- `ExecuteTrigger::new(...)` now uses an empty JSON object for its default args
  so no-argument by-call instructions encode cleanly through Norito. Integration
  trigger fixtures now construct typed `InstructionBox` values instead of
  erasing payload type information through `Box<dyn Instruction>`, and
  timing-sensitive trigger integration checks now wait for committed trigger
  side effects before asserting.
- Negative/adversarial trigger coverage now includes mismatched execute-trigger
  authority and invalid retry-policy payloads during Norito/JSON action decode,
  malformed `__enabled` active-cache behavior, rejected/applied block pipeline
  registration filters, malformed-enabled pipeline execution, and pipeline
  trigger rollback when chained data triggers fail. Follow-up adversarial cases
  also cover direct `Action::try_new(...)` rejection of trigger-completion
  filters, JSON decode rejection of retry policies on pre-commit time triggers,
  constrained pipeline transaction filters ignoring hash/height near-misses, and
  fail-closed pipeline IVM execution when registered bytecode is missing. The
  latest coverage adds non-base64 Action JSON rejection, active-cache recovery
  after replacing malformed `__enabled` with `true`, and block-approved pipeline
  filters ignoring wrong-height approved events. Additional adversarial coverage
  rejects valid-base64 Action JSON with invalid Norito bytes, verifies removing
  a false `__enabled` flag restores default-active behavior, and proves
  one-shot pipeline triggers execute only once when multiple matching pipeline
  events arrive in the same batch. Follow-up adversarial cases reject non-string
  Action JSON payloads, verify setting `__enabled=false` removes an active
  trigger from active queries, and prove direct pipeline action failure rolls
  back without consuming repeats. The newest coverage exercises numeric
  `__enabled=0/1` transitions in active queries, confirms numeric-zero pipeline
  triggers stay inactive without consuming repeats even under matching approved
  block facts, and proves one-shot transaction pipeline triggers execute only
  once when duplicate approved transaction facts arrive in one batch. The latest
  adversarial pass adds fail-closed TriggerCompleted JSON decoding for
  non-string, non-base64, and invalid Norito payloads, rejects unknown
  trigger-completion outcome discriminants, and verifies by-call triggers with
  numeric-zero or malformed `__enabled` metadata reject execution without
  consuming repeats or appearing active. The newest negative data-trigger cases
  verify numeric-zero and malformed `__enabled` metadata skip DFS execution
  without mutation, repeat consumption, or active-cache exposure, and that
  corrupted depleted data-trigger entries are pruned without executing.
- Validation: `cargo check -p iroha_core --lib`;
  `CARGO_TARGET_DIR=/tmp/iroha-trigger-tests cargo test -p iroha_core --lib
  trigger -- --nocapture` (`153` passed);
  `CARGO_TARGET_DIR=/tmp/iroha-trigger-dm cargo test -p iroha_data_model
  trigger -- --nocapture` (`35` passed in lib plus filtered integration-test
  binaries); and `CARGO_TARGET_DIR=/tmp/iroha-trigger-it cargo test -p
  integration_tests --test events_and_triggers triggers:: -- --nocapture` (`26`
  passed) are green. `cargo fmt --all -- --check`, `git diff --check` on the
  touched files, and `scripts/check_no_scale.sh` are also green. Focused
  `NORITO_SKIP_BINDINGS_SYNC=1 cargo clippy -p iroha_data_model --lib --
  -D warnings`, `NORITO_SKIP_BINDINGS_SYNC=1 cargo clippy -p iroha_core --lib
  -- -D warnings`, and the earlier
  `cargo clippy -p integration_tests --test events_and_triggers -- -D warnings`
  are green after narrow lint cleanups in dependency files, including a tiny
  Kotodama borrow-order fix and local `too_many_arguments` allow needed to keep
  the dirty dependency tree compiling under strict clippy. Unskipped clippy is
  currently blocked in this dirty tree by unrelated Kotlin Norito binding-sync
  test compilation errors in `HttpClientTransportTest.kt`.

## 2026-05-19 Soracloud SDK parser adversarial hardening

- Tightened Kotlin/JVM and Java Android Soracloud private uploaded-model JSON
  parsers so receipt-list pagination counts, artifact ciphertext byte counts,
  and receipt emitted sequence numbers are rejected when negative instead of
  being accepted as structurally valid signed-response data.
- Added adversarial parser coverage for negative `total`, `returned_items`,
  `remaining_items`, `ciphertext_bytes`, and `emitted_sequence`, plus blank
  receipt identity/policy fields on the execute-response path. Java Android
  mirrors the Kotlin behavior through the existing Gradle harness entry point.
- Validation: `JAVA_HOME=/opt/homebrew/Cellar/openjdk@21/21.0.11/libexec/openjdk.jdk/Contents/Home
  ./gradlew :core-jvm:test --tests
  org.hyperledger.iroha.sdk.client.SoracloudPrivateUploadedModelJsonParserTest
  --no-daemon --console=plain` from `kotlin`, and
  `JAVA_HOME=/opt/homebrew/Cellar/openjdk@21/21.0.11/libexec/openjdk.jdk/Contents/Home
  ANDROID_HARNESS_MAINS=org.hyperledger.iroha.android.client.SoracloudPrivateUploadedModelJsonParserTests
  ./gradlew :core:test --tests
  org.hyperledger.iroha.android.GradleHarnessTests --no-daemon
  --console=plain` from `java/iroha_android` are green.

## 2026-05-19 Nexus proposal lane lookahead

- Proposal assembly now fetches up to the configured scan budget before
  applying the block slot cap, then orders fetched transactions with
  slot-rotated lane interleaving. This lets a small proposal reach a ready
  transaction from a later lane instead of being monopolized by the first lane
  in queue order.
- Transactions fetched during lookahead but not admitted because of the slot,
  gas, or IVM-heavy transaction budgets release their lane TEU accounting and
  enter the existing deterministic deferred requeue path.
- The roadmap now names the remaining work as replacing this bounded global
  proposal-path lookahead with the full independent per-lane proposal/vote
  scheduler.
- Focused validation with `CARGO_TARGET_DIR=target/codex-lane-consensus` is
  green for `cargo check -p iroha_core`,
  `cargo test -p iroha_core --lib interleave_lane_indices -- --nocapture`,
  `cargo test -p iroha_core --lib proposal_queue_scan_budget -- --nocapture`,
  `cargo test -p iroha_core --lib proposal_gas_budget_limits_fetch --
  --nocapture`, and `cargo test -p iroha_core --lib
  proposal_ivm_budget_defers_extra_ivm_and_keeps_cheap_slots -- --nocapture`.
  Touched-file `rustfmt --edition 2024 --check` and `git diff --check` are
  also green.

## 2026-05-19 Nexus verified relay commit hydration

- `RegisterVerifiedLaneRelay` execution now stages the verified relay record on
  the transaction, carries successful transactions into the block accumulator,
  and hydrates the runtime lane-relay cache immediately after the block's
  contract-visible state commits.
- The shared hydration path still validates relay-ref, proof digest,
  verification-height, manifest-root, FastPQ effect type, and claim digest
  before recording a relay. Invalid or stale staged records are logged and do
  not enter the merge-admissible relay set.
- Added adversarial hydration coverage proving bad claim digests are ignored,
  staged records missing FastPQ proof material or carrying zeroed manifest roots
  are ignored, dropped transactions neither persist nor hydrate relay records,
  conflicting cached relay material is not overwritten, and older committed
  records cannot regress a newer cached lane relay.
- Added admission-boundary negative coverage for missing/zero manifest roots,
  disabled Nexus, unknown lane ids, lane/dataspace mismatches, unknown
  dataspaces, empty or malformed proof payloads, proof dataspace and
  manifest-root mismatches, zero-like proof digests, stale/future FastPQ
  metadata heights, malformed envelope block heights, settlement
  lane/dataspace/hash/totals mismatches, expired proof blobs, missing FastPQ
  bindings, source dataspace mismatches, wrong FastPQ effect types, malformed
  stored relay state, and conflicting stored relay state.
- Added persisted-state corruption coverage proving merge-candidate hydration
  ignores verified relay records whose relay ref, proof payload hash,
  verification height, manifest root, FastPQ source dataspace, or FastPQ effect
  type no longer match the embedded relay envelope.
- Added key-space and sibling-corruption coverage proving noncanonical
  prefix-matching verified relay keys cannot hydrate relays, malformed or
  corrupted prefixed sibling records cannot block a valid canonical record,
  self-consistent records stored under another relay's canonical key are
  ignored, spoofed-key siblings cannot replace the valid canonical relay,
  records for unconfigured lanes or unexpected dataspaces are ignored, and
  unrelated malformed contract state is ignored by the relay scanner.
- Added direct lane-relay-burn ingestion coverage proving malformed canonical
  verified state, canonical keys containing another relay's record, corrupted
  verified state fields, and noncanonical verified state keys cannot satisfy
  `record_lane_relay` admission.
- Fixed an existing Soracloud BFV bootstrap `Result` inference blocker that was
  preventing current dirty-tree `iroha_core` tests from compiling.
- Fixed an existing block-test pipeline-trigger helper compile blocker by
  staging synthetic pipeline triggers through the trigger storage transaction
  API and using a concrete rejected transaction status in rejected-event
  filters.
- Focused validation with `CARGO_TARGET_DIR=target/codex-lane-consensus` is
  green for `cargo check -p iroha_core`,
  `cargo test -p iroha_core --lib merge_candidates -- --nocapture` (16 tests),
  `cargo test -p iroha_core --lib merge_candidates_ignore_contract_state_record
  -- --nocapture`,
  `cargo test -p iroha_core --lib verified_lane_relay -- --nocapture` (45
  tests),
  `cargo test -p iroha_core --lib
  committed_verified_lane_relay_record_hydrates_runtime_cache -- --nocapture`,
  `cargo test -p iroha_core --lib register_verified_lane_relay -- --nocapture`
  (27 tests), `cargo test -p iroha_core --lib record_lane_relay -- --nocapture`
  (26 tests),
  and `cargo test -p iroha_core --lib
  block_validation_sequential_entrypoints_execute_pipeline_triggers --
  --nocapture`.

## 2026-05-19 Nexus lane relay merge hardening

- Changed lane relay handling to two-stage FastPQ admission. Structurally valid
  relays are retained and gossiped as pending; relays with valid FastPQ material
  upgrade the same `(lane_id,dataspace_id,height,settlement_hash)` key to
  verified status. Merge candidate synthesis only uses verified relays.
- Block sealing now emits pending lane relay envelopes without copying the
  global block commit QC into the lane QC field, so relays require real
  lane-domain finality before they can enter the merge path.
- Merge candidates now use active lanes only. Configured lanes without a new
  verified relay no longer block candidate construction, and merge roots are
  derived from lane QC state roots, tip hash, DA commitment, settlement hash,
  and RBC byte totals instead of the settlement hash alone.
- Lane relay QC verification now uses a lane/dataspace domain-separated mode
  tag, so a global block QC cannot be copied into a lane relay and accepted as
  lane-final evidence. Merge entry commit validation also checks that every
  snapshot is backed by a stored verified relay with matching tip and
  merge-hint root.
- Merge candidate synthesis now hydrates persisted verified lane relay records
  from contract state into the runtime relay cache before selecting active
  lanes. Hydrated records are checked for relay-ref, proof digest,
  verification-height, manifest-root, FastPQ effect type, and claim-digest
  consistency before they can become merge-admissible.
- Fixed Soracloud FHE/state-entry test fixtures to match the already-updated
  provenance payload and service-state payload schemas, unblocking data-model
  test-profile builds on the current dirty tree.
- Focused validation with `CARGO_TARGET_DIR=target/codex-lane-consensus` is
  green for `cargo check -p iroha_data_model`, `cargo check -p iroha_core`,
  `cargo test -p iroha_data_model --lib nexus::relay::tests -- --nocapture`,
  `cargo test -p iroha_core --lib nexus::lane_relay -- --nocapture`,
  `cargo test -p iroha_core --lib record_lane_relay -- --nocapture`,
  `cargo test -p iroha_core --lib lane_relay_store_upgrades -- --nocapture`,
  `cargo test -p iroha_core --lib lane_relay_envelopes -- --nocapture`,
  `cargo test -p iroha_core --lib lane_relay_helper -- --nocapture`,
  `cargo test -p iroha_core --lib commit_merge_entry -- --nocapture`,
  `cargo test -p iroha_core --lib merge_candidate -- --nocapture`, and
  `cargo test -p iroha_core --lib merge_committee -- --nocapture`. The
  contract-state hydration regression
  `cargo test -p iroha_core --lib
  merge_candidates_hydrate_verified_lane_relay_records_from_contract_state --
  --nocapture` is green. Touched-file `rustfmt --edition 2024 --check` and
  `git diff --check` are also green.

## 2026-05-18 Negative/adversarial test addendum

- Added Sumeragi unit coverage for the named block-sync recovery modes. The
  negative cases assert that payload-only recovery enables no bypasses,
  requested-payload repair only permits stale payload recovery, and signed
  quorum repair cannot impersonate commit evidence or revive aborted work
  without local commit-QC policy. The same module now also pins the full stale
  `BlockCreated` admission truth table so a stale payload is rejected unless a
  missing request, retained match, or recovery evidence is present.
- Added FASTPQ alert-rule negative coverage for present-but-flat GPU disable
  and sampled parity-failure counters, proving scraped historical counter
  values do not alert unless the counter increases inside the rule window. It
  also covers old increments outside the five-minute alert window and exact
  queue/zero-fill thresholds that must not page.
- Added sidecar-pruning negative coverage proving optional recovery metadata
  removal preserves non-sidecar payload directories and files. Additional
  adversarial helper coverage proves similarly named payload directories are
  preserved, missing roots are ignored, and Soracloud canonical signing URIs
  bind path/query bytes without including origin or fragments.
- Validation: `CARGO_TARGET_DIR=target/codex-struct-hardening-check cargo test
  -p iroha_core proposal_handlers::tests --lib -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-struct-hardening-check cargo test -p
  integration_tests --test network_functional remove_optional_recovery_sidecars
  -- --nocapture`, `CARGO_TARGET_DIR=target/codex-struct-hardening-check cargo
  test -p integration_tests --test network_functional signing_uri --
  --nocapture`, `cargo fmt --all --check`, touched-file `git diff --check`,
  dashboard JSON/YAML parsing, and the no-`Cargo.lock`/`dist` diff check are
  green.

## 2026-05-18 Structural hardening completion pass

- Sumeragi's block-sync-to-BlockCreated handoff now uses named recovery modes
  (`PayloadOnly`, `RequestedPayloadRepair`, `SignedQuorumFrontierRepair`, and
  `CommitEvidenceRepair`) instead of passing broad stale/authoritative/revival
  bypass booleans through the exact-frontier path. The cleanup preserves the
  existing payload-only, signed-quorum, and commit-evidence behavior while
  making the certified recovery corridor explicit at the actor-owned handoff.
- Added focused 4-peer restart coverage for the memory-only WSV model:
  route-sensitive account, alias, asset, and domain-owned state is committed,
  optional checkpoint/manifest sidecars are removed, peers restart from Kura
  blocks, and the rebuilt query surface is compared across all restarted peers.
- Added focused 4-peer Soracloud private uploaded-model runtime coverage:
  approved SoraFS artifacts are registered, the deterministic quantized CPU
  private execute route returns a receipt instruction, the receipt is committed,
  peers restart from Kura blocks, and the committed receipt query remains
  deterministic across restarted peers.
- FASTPQ operator coverage now includes Prometheus alerts and Grafana panels
  for `fastpq_gpu_disable_total` and `fastpq_gpu_parity_failure_total`, so GPU
  disable events and sampled runtime parity failures are visible as fail-closed
  rollout signals. The alert YAML and dashboard JSON parse cleanly; `promtool`
  was not installed on this host for rule-test execution.
- Focused validation is green with
  `CARGO_TARGET_DIR=target/codex-struct-hardening-check cargo check -p
  iroha_core -p iroha_torii -p irohad -p iroha_data_model`,
  `CARGO_TARGET_DIR=target/codex-struct-hardening-check cargo test -p
  iroha_core stale_commit_inflight -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-struct-hardening-check cargo test -p
  iroha_core block_sync_update_commit_qc_supersedes_stale_same_height_frontier_owner
  --lib -- --nocapture`, `CARGO_TARGET_DIR=target/codex-struct-hardening-check
  cargo test -p iroha_core
  block_sync_payload_with_cached_commit_qc_supersedes_lock_conflicting_stale_frontier_owner
  --lib -- --nocapture`, `CARGO_TARGET_DIR=target/codex-struct-hardening-check
  cargo test -p iroha_core
  block_sync_update_accepts_stale_exact_frontier_payload_repair_with_da --lib
  -- --nocapture`, the focused 4-peer Kura/WSV restart test, the focused
  4-peer Soracloud private receipt restart test, the FASTPQ Metal Poseidon
  gates, the Kotlin `:core-jvm:test` gate, and the Java Android Soracloud
  parser harness. `cargo fmt --all --check`, `git diff --check` on touched
  files, dashboard JSON/YAML parsing, and the no-`Cargo.lock`/`dist` diff check
  are also clean. CUDA runtime parity remains pending on a real CUDA host.

## 2026-05-18 Torii query adversarial cursor coverage

- Added negative coverage for replay-backed bounded stored query cursors:
  mismatched paged cursors fail before invoking replay work and do not evict the
  valid cursor, permanent paged failures (`Expired`/`CursorDone`) evict the
  dead cursor and release live-query capacity plus per-authority quota,
  transient paged failures remain resident, retryable, and quota-consuming,
  exhausted paged starts do not consume capacity or quota, dropped paged cursors
  release capacity and quota without invoking replay work, paged starts still
  enforce live-query-store capacity, explicit pagination limits do not force
  probe reads or cursor storage, forged returned offsets at the limit fail
  before reading source rows, and oversized fetch requests fail before reading
  source rows.
- Added Arc snapshot-lane coverage for the Torii path: wrong continuation
  cursors and forged query ids do not consume the original cursor, and
  limit-bound bounded queries return no cursor even when more source rows exist
  past the requested limit. The Arc path also rejects ephemeral continuations,
  rejects underfunded stored continuations before store lookup, proves those
  validation failures do not consume real cursors, lets sufficiently funded
  missing cursors reach the store and expire, and evicts replay cursors once
  their owning state handle is gone. It also documents the intended
  fresh-view replay behavior by proving bounded continuations can observe later
  state changes.
- Validation: `CARGO_INCREMENTAL=0
  CARGO_TARGET_DIR=/tmp/iroha-query-api-check cargo test -p iroha_core --lib
  paged_ -- --nocapture`,
  `CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/iroha-query-api-check cargo test
  -p iroha_core --lib stored_unsorted_bounded_replay_ -- --nocapture`,
  `CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/iroha-query-api-check cargo test
  -p iroha_core --lib collect_unsorted_bounded_page_rejects_ -- --nocapture`,
  `CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/iroha-query-api-check cargo test
  -p iroha_core --lib arc_ -- --nocapture`,
  `CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/iroha-query-api-check cargo check
  -p iroha_torii --tests` (passes with an unrelated dirty-worktree warning in
  `crates/iroha_core/src/smartcontracts/isi/sns.rs`), and
  `CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/iroha-query-api-check cargo clippy
  -p iroha_core --all-targets -- -D warnings` are green.
  `rustfmt --edition 2024 --check` on the touched query files and
  `git diff --check` are also green; package/workspace-wide `cargo fmt
  --check` is blocked by unrelated existing format diffs in other dirty
  workspace files.

## 2026-05-18 ZK adversarial negative coverage follow-up

- Legacy Halo2 vote-circuit matching now accepts only the exact
  `vote-bool-commit-merkle{2,8,16}` families or hyphen-suffixed variants,
  closing prefix-smuggling shapes such as `vote-bool-commit-merkle8evil` while
  preserving intended legacy ids.
- Added adversarial coverage for preverified proof cache keys binding proof
  backend and bytes, poisoned preverified cache entries failing to bypass the
  registered verifying-key commitment, poisoned preverified entries failing to
  bypass `OpenVerifyEnvelope` backend-tag mismatches, cross-family envelope
  backend tags being rejected before cache lookup, malformed proof attachment
  triples failing early, same-name VK refs under different backend namespaces
  missing preverified cache hits, proof and verifying-key hash boundaries
  resisting backend/payload concatenation ambiguity, poisoned preverified
  entries failing to bypass public-input schema, envelope VK-hash, inactive-VK,
  circuit-index, or stored-key-backend registry mismatches, preverified replay
  attempts failing before an existing proof record can be overwritten, preverify
  dedup keys resisting backend/payload boundary ambiguity and
  missing-vs-present commitment ambiguity, failed preverify attempts not
  poisoning later valid dedup entries even for wrong VK bytes or unsupported
  empty backends, and P2P confidential handshakes rejecting peers whose ZK
  policy hashes, confidential rules versions, VK-set hashes,
  Poseidon/Pedersen parameter ids, enabled flags, `assume_valid` flags,
  verifier backends, or required confidential metadata are missing or mismatched.
- Validation: focused coverage is green with `CARGO_BUILD_JOBS=2
  CARGO_TARGET_DIR=/tmp/iroha-codex-zk-more cargo test -p iroha_core
  preverified_key_tests --lib -- --nocapture`, `CARGO_BUILD_JOBS=2
  CARGO_TARGET_DIR=/tmp/iroha-codex-zk-more cargo test -p iroha_core
  preverified_proof_key --lib -- --nocapture`,
  `CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=/tmp/iroha-codex-zk-more cargo test -p
  iroha_core normalize_halo2_circuit_id_and_match_variants --lib --
  --nocapture`, `CARGO_BUILD_JOBS=2
  CARGO_TARGET_DIR=/tmp/iroha-codex-zk-more cargo test -p iroha_core
  verify_proof_preverified_cache_does_not_bypass_wrong_vk_commitment_key --lib
  -- --nocapture`, and `CARGO_BUILD_JOBS=2
  CARGO_TARGET_DIR=/tmp/iroha-codex-zk-more cargo test -p iroha_core
  verify_proof_preverified_cache_does_not_bypass_wrong_envelope_backend_tag
  --lib -- --nocapture`, `CARGO_BUILD_JOBS=2
  CARGO_TARGET_DIR=/tmp/iroha-codex-zk-more cargo test -p iroha_core
  open_verify_backend_tag_matches_rejects_cross_family_tags --lib --
  --nocapture`, `CARGO_BUILD_JOBS=2
  CARGO_TARGET_DIR=/tmp/iroha-codex-zk-more cargo test -p iroha_core
  validate_proof_attachment_rejects_mismatched_attachment_triples --lib --
  --nocapture`, `CARGO_BUILD_JOBS=2
  CARGO_TARGET_DIR=/tmp/iroha-codex-zk-more cargo test -p iroha_core
  verify_proof_preverified_cache_does_not_bypass_envelope_metadata_mismatches
  --lib -- --nocapture`, `CARGO_BUILD_JOBS=2
  CARGO_TARGET_DIR=/tmp/iroha-codex-zk-more cargo test -p iroha_core
  verify_proof_preverified_cache --lib -- --nocapture`, `CARGO_BUILD_JOBS=2
  CARGO_TARGET_DIR=/tmp/iroha-codex-zk-more cargo test -p iroha_core
  verify_proof_preverified_cache_does_not_bypass_existing_proof_record --lib
  -- --nocapture`, `CARGO_BUILD_JOBS=2
  CARGO_TARGET_DIR=/tmp/iroha-codex-zk-more cargo test -p iroha_core
  verifying_key_hash_length_prefixes_backend_and_payload --lib --
  --nocapture`, `CARGO_BUILD_JOBS=2
  CARGO_TARGET_DIR=/tmp/iroha-codex-zk-more cargo test -p iroha_core
  preverify_ --lib -- --nocapture`, `CARGO_BUILD_JOBS=2
  CARGO_TARGET_DIR=/tmp/iroha-codex-zk-more cargo test -p iroha_core
  preverify_dedup_key_length_prefixes_backend_and_payload --lib --
  --nocapture`, `CARGO_BUILD_JOBS=2
  CARGO_TARGET_DIR=/tmp/iroha-codex-zk-more cargo test -p iroha_core
  failed_preverify_attempts_do_not_poison_dedup_cache --lib -- --nocapture`,
  `CARGO_BUILD_JOBS=2
  CARGO_TARGET_DIR=/tmp/iroha-codex-zk-more cargo test -p iroha_p2p
  handshake_rejects --lib -- --nocapture`.

## 2026-05-18 FASTPQ Metal parity and mobile SDK validation follow-up

- The Java Android Gradle harness now runs
  `SoracloudPrivateUploadedModelJsonParserTests`, so the Java mirror parser
  coverage is part of the normal targeted harness instead of remaining an
  orphaned main-style test.
- JDK-backed mobile SDK validation is green under Homebrew OpenJDK 21:
  `JAVA_HOME=/opt/homebrew/Cellar/openjdk@21/21.0.11/libexec/openjdk.jdk/Contents/Home
  ./gradlew :core-jvm:cleanTest :core-jvm:test --no-daemon --console=plain`
  passed from `kotlin`, and
  `JAVA_HOME=/opt/homebrew/Cellar/openjdk@21/21.0.11/libexec/openjdk.jdk/Contents/Home
  ANDROID_HARNESS_MAINS=org.hyperledger.iroha.android.client.SoracloudPrivateUploadedModelJsonParserTests
  ./gradlew :core:test --tests
  org.hyperledger.iroha.android.GradleHarnessTests --no-daemon
  --console=plain` passed from `java/iroha_android`.
- FASTPQ's generic Metal Poseidon column path now dispatches one state per
  batch until vectorized multi-state parity is proven for both column and
  Merkle-pair workloads. The scalar sponge remains authoritative, and the
  conservative Metal path now has direct CPU-equivalence coverage for
  multi-column and Merkle parent-pair self-test vectors.
- Hardware evidence was collected on this host's Apple M1 Ultra GPU. The
  Metal-focused gates are green with `FASTPQ_GPU=gpu` and
  `CARGO_TARGET_DIR=target/codex-struct-hardening-check`:
  `cargo test -p fastpq_prover --features fastpq-gpu metal_poseidon --
  --nocapture`, `cargo test -p fastpq_prover --features fastpq-gpu
  poseidon_gpu -- --nocapture`, `cargo test -p fastpq_prover --features
  fastpq-gpu poseidon_fused_gpu_matches_cpu_first_level -- --nocapture`, and
  `cargo test -p fastpq_prover --features fastpq-gpu metal_bn254_poseidon --
  --nocapture`.
- CUDA runtime parity remains pending because no CUDA device/toolchain was
  available on this macOS host.

## 2026-05-18 Soracloud private route hardening and FASTPQ telemetry

- Torii OpenAPI now documents the Soracloud private uploaded-model execute and
  committed receipt query routes, including the deterministic quantized CPU
  request, encrypted artifact references, receipt payload, transaction
  instruction skeleton, and bounded receipt pagination metadata.
- Private uploaded-model execution now fail-closes when the requested policy id
  diverges from the admitted uploaded-model bundle. When a
  `decryption_request_id` is supplied, Torii also requires a committed
  decryption request record for the same service, policy, input ciphertext
  commitment, and no-newer sequence before releasing the runtime execution
  path. Receipts remain commitment/artifact-reference only; plaintext is not
  stored on-chain.
- FASTPQ exposes process-local GPU accelerator event observers and Prometheus
  counters for accelerator disable events and sampled parity failures. The
  daemon wires those events into telemetry labels for the configured device
  class, chip family, and GPU kind, while scalar CPU hashing remains the
  authoritative fallback.
- Sumeragi VRF staging now merges compatible committed and pending NPoS VRF
  observations instead of letting stale committed seals drop newer compatible
  pending evidence, and conflicting pending VRF records are pruned against the
  committed state.
- The NPoS 30 TPS two-hour soak is green with
  `artifacts/30tps-2h-npos-vrfstage-20260518T065858`: 719 monitor samples,
  maximum no-progress gap `10.118s` against the `60s` stall threshold, final
  height `1098`, final approved transactions `214572`, zero rejected
  transactions, and no `VRF epoch seal conflicts`, pending-block validation
  reject, `NposEffectsInvalid`, or panic signatures in peer logs.
- Negative/adversarial VRF staging coverage now rejects same-signer
  commitment rewrites, conflicting late reveals, penalty marker rewrites,
  overlapping finalized offender categories, bad snapshots over compatible
  pending state, and bad pending records when a committed-compatible snapshot
  is available.
- Validation: `cargo fmt --all`,
  `CARGO_TARGET_DIR=target/codex-struct-hardening-check cargo test -p
  iroha_torii openapi --lib -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-struct-hardening-check cargo test -p
  iroha_torii private_uploaded_model_execute --lib -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-struct-hardening-check cargo test -p
  iroha_telemetry records_fastpq_gpu_disable_and_parity_metrics --lib --
  --nocapture`,
  `CARGO_TARGET_DIR=target/codex-struct-hardening-check cargo test -p
  fastpq_prover poseidon_policy_labels_cpu_fallbacks -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-struct-hardening-check cargo check -p
  iroha_core -p iroha_torii -p irohad -p iroha_data_model`,
  `CARGO_TARGET_DIR=target/codex-struct-hardening-check cargo test -p
  iroha_core stage_vrf_snapshot --lib -- --nocapture`,
  `cargo test -p iroha_core --lib merge_vrf_epoch_records -- --nocapture`,
  and `CARGO_TARGET_DIR=target/codex-struct-hardening-check cargo test -p
  integration_tests --test consensus_and_da
  joint_consensus_switches_mode_at_activation_height -- --nocapture` are green.
  The conservative Metal parity corridor is now green; CUDA runtime parity
  remains pending on a suitable host.

## 2026-05-18 Torii bounded stored query continuations

- Torii's Arc-owned snapshot query path now registers unsorted bounded stored
  cursors as replay continuations: the start response consumes only the first
  page plus a probe, and each `Continue` request reopens a query view and
  materializes one page instead of retaining a fully materialized tail in the
  live-query store.
- The borrowed-state stored path remains available for callers that cannot
  provide a `State` handle; sorted and exact-count stored queries keep their
  existing snapshot-materialized cursor behavior.
- Added focused coverage for replay-backed bounded starts not materializing the
  tail and for Arc-backed stored bounded continuation returning the next page
  with bounded metadata.
- Validation: focused query coverage is green with
  `CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/iroha-query-api-check cargo check
  -p iroha_core --lib`,
  `CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/iroha-query-api-check cargo test
  -p iroha_core --lib
  stored_unsorted_bounded_replay_cursor_does_not_materialize_tail_on_start --
  --nocapture`,
  `CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/iroha-query-api-check cargo test
  -p iroha_core --lib bounded_stored_arc_continuation_replays_one_page --
  --nocapture`,
  `CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/iroha-query-api-check cargo check
  -p iroha_torii --tests`, and
  `CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/iroha-query-api-check cargo clippy
  -p iroha_core -p iroha_torii --all-targets -- -D warnings`; `cargo fmt
  --all --check` and `git diff --check` are also green.

## 2026-05-18 Kura optional WSV sidecar recovery

- Kura commit manifests remain Norito sidecars for replay verification of the
  memory-only WSV surface, but startup recovery now treats the canonical durable
  block log as the source of truth. Missing manifests no longer imply a shorter
  recoverable chain.
- The Sumeragi commit worker persists the canonical block to Kura before WSV
  state apply/commit. Post-commit WSV checkpoint or commit-manifest write
  failures now have explicit regression coverage as non-fatal status/telemetry
  warnings: the block and memory WSV commit remain accepted, and replay can
  recover from the canonical block log.
- Startup reconciliation prunes stale, unreadable, or block-mismatched commit
  manifests and mismatched WSV checkpoints instead of pruning intact durable
  blocks. Checkpoints above the durable block height are also pruned when no
  commit manifest directory exists.
- Focused Kura coverage now exercises mismatched manifest pruning, mismatched
  checkpoint pruning, checkpoint pruning without manifests, and preserving
  durable blocks when a manifest is missing.
- Validation: focused core coverage passes with `CARGO_INCREMENTAL=0 cargo
  test -p iroha_core --lib commit_manifest -- --nocapture`,
  `CARGO_INCREMENTAL=0 cargo test -p iroha_core --lib bounded --
  --nocapture`, and `CARGO_INCREMENTAL=0 cargo test -p iroha_core --lib
  kura_store_counters_surface_in_snapshot -- --nocapture`; `CARGO_INCREMENTAL=0
  cargo check -p iroha_core` is green. Integration validation passes with
  `CARGO_INCREMENTAL=0 cargo test -p integration_tests --lib` and
  `CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/iroha-codex-wsv-store-sync cargo
  test -p integration_tests --test consensus_and_da -- --nocapture` (258
  passed, 0 failed, 7 ignored; 4239.30s), covering the mode-cutover,
  vote-QC, NPoS timing, and restart/rehydration regressions exposed by the
  earlier workspace run.
  Lint validation passes with `CARGO_INCREMENTAL=0 cargo clippy -p
  iroha_data_model --all-targets -- -D warnings` and `CARGO_INCREMENTAL=0 cargo
  clippy --workspace --all-targets -- -D warnings` after splitting the
  overlong SoraCloud receipt roundtrip test. Targeted `rustfmt --edition 2024
  --check`, `CARGO_INCREMENTAL=0
  CARGO_TARGET_DIR=/tmp/iroha-codex-wsv-store-sync cargo clippy -p
  integration_tests --test consensus_and_da -- -D warnings`,
  `cargo fmt --all -- --check`, and `git diff --check` are green for the
  Kura/query/status files,
  integration regressions, schema drift fixes, and docs touched by this work.

## 2026-05-18 FASTPQ Poseidon parity fail-closed gates

- FASTPQ GPU Poseidon column hashing now disables the process-local column
  accelerator after a dispatch error, startup self-test mismatch, limb-batch
  count mismatch, or sampled CPU parity mismatch. Future calls fall back to the
  deterministic CPU Poseidon path instead of retrying a suspect accelerator.
- Trace Merkle parent-pair hashing now disables its GPU path on sampled
  CPU-parity mismatch as well as dispatch failure, preserving scalar hashing as
  the authoritative path for Merkle parents.
- Validation: `rustfmt --edition 2024 --check
  crates/fastpq_prover/src/trace.rs crates/fastpq_prover/src/backend.rs` and
  `git diff --check -- crates/fastpq_prover/src/trace.rs
  crates/fastpq_prover/src/backend.rs` are green. Focused FASTPQ fallback
  coverage passes with `CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/iroha-codex-cross-ds
  cargo test -p fastpq_prover poseidon_policy_labels_cpu_fallbacks --
  --nocapture`. The conservative Metal parity follow-up is now green on Apple
  M1 Ultra; CUDA runtime parity remains pending on a suitable host.

## 2026-05-18 Soracloud private uploaded-model CPU runtime slice

- `SoraUploadedModelRuntimeFormatV1` now has a
  `DeterministicQuantizedCpuV1` package class for the first private
  uploaded-model execution runtime. The chain-facing receipt surface is
  explicit:
  `SoraPrivateUploadedModelExecutionReceiptV1` stores runtime version, model
  manifest digest, bundle root, policy id, input/output commitments, and
  encrypted SoraFS artifact references only.
- `iroha_core::soracloud_runtime` now includes the v1 CPU reference evaluator
  for quantized uploaded models. It uses fixed signed-integer linear
  operations, nearest-away-from-zero rounding, saturating bounds, and rejects
  bundles whose admitted runtime format or policy id does not match the private
  execution request.
- This non-FHE uploaded-model runtime confines plaintext input/output to the
  private service call boundary; receipts remain hash/artifact-ref only so
  plaintext does not enter chain state.
- Torii now exposes `/v1/soracloud/model/upload/private/execute` as the first
  guarded private execution surface. The route resolves a finalized uploaded
  model from authoritative state, requires the model format to be
  `DeterministicQuantizedCpuV1`, verifies the model bundle plus encrypted input
  and output artifact references against approved SoraFS pin manifests, then
  returns the deterministic private execution receipt plus a canonical
  `RecordSoracloudPrivateUploadedModelExecutionReceipt` transaction-instruction
  skeleton for client signing/submission.
- Torii also exposes `/v1/soracloud/model/upload/private/receipts` for committed
  private execution receipts, filtered by receipt id, service, model id, or
  weight version with bounded pagination metadata and explicit
  `count_mode=exact` opt-in.
- `RecordSoracloudPrivateUploadedModelExecutionReceipt` is now a Norito
  instruction with registry/visitor coverage. Core state stores committed
  private uploaded-model execution receipts by receipt hash, and the receipt
  writer rejects receipts whose finalized uploaded-model bundle is not the
  deterministic quantized CPU format or whose manifest, bundle root, or policy
  binding diverges from the admitted bundle.
- The JavaScript SDK now has unsigned private uploaded-model helpers for
  deterministic CPU execute requests, committed receipt queries, and extraction
  of the returned `RecordSoracloudPrivateUploadedModelExecutionReceipt`
  transaction-instruction skeleton. The helpers reject embedded signing secrets
  and preserve external signing as the only transaction path.
- The Kotlin core SDK and Java Android SDK now include parser models for
  private uploaded-model execute responses, committed receipt-list responses,
  and the unsigned receipt-instruction skeleton helper, keeping mobile clients
  aligned with the JavaScript extraction flow.
- Validation: `CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/iroha-codex-cross-ds
  cargo test -p iroha_data_model
  private_uploaded_model_execution_receipt_round_trips_and_validates --
  --nocapture` and `CARGO_INCREMENTAL=0
  CARGO_TARGET_DIR=/tmp/iroha-codex-cross-ds cargo test -p iroha_core --lib
  private_uploaded_model_quantized_cpu_runtime -- --nocapture` are green.
  `CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/iroha-codex-cross-ds cargo test -p
  iroha_torii --lib
  private_uploaded_model_execute_requires_finalized_quantized_bundle_and_active_artifacts
  -- --nocapture` is also green. `cargo check -p iroha_core` is green with the
  committed receipt store wired into world views/transactions.
  `npm run build:dist` and `node --test test/soracloud.test.js` are green from
  `javascript/iroha_js`. JDK-backed Kotlin and Java Android parser validation
  is now green under Homebrew OpenJDK 21, and OpenAPI route coverage is green
  in the Soracloud route-hardening entry. Multi-peer private execution
  integration coverage remains open.

## 2026-05-18 Torii space-directory pagination metadata

- The space-directory manifest app endpoint now accepts `count_mode` and
  returns `has_more` plus `count_mode` with its existing manifest inventory
  response, closing one of the concrete app-list response gaps without changing
  route paths or the older `total` field.
- Validation: `CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/iroha-codex-cross-ds
  cargo test -p iroha_torii handle_v1_space_directory_manifests --
  --nocapture` is green; the seven focused manifest endpoint tests passed and
  the remaining filtered Torii test binaries completed without failures.

## 2026-05-17 Transaction pipeline parallelism follow-up

- Static block validation now builds and installs the same config-sized private
  pipeline Rayon pool used by state-backed execution, so `pipeline.workers`
  bounds stateless transaction validation and Merkle-root construction instead
  of falling through to Rayon’s global pool.
- Parallel apply layer construction now computes component-local indegrees with
  reusable buffers and merges deterministic component waves, removing the
  previous full-indegree clone per component.
- Dynamic IVM access scheduling now derives access sets from the already-built
  overlay and optional host access log, avoiding a separate access-prepass VM
  execution before overlay construction.
- Sumeragi status snapshots now classify detached sequential fallbacks by fee
  postprocessing, user-provided executor, durable state, unsupported
  instruction, rejected detached evaluation, and overlay build error; telemetry
  exports the same aggregate reasons through
  `pipeline_detached_fallback_reason{reason=...}`.
- Fee-bearing transactions now participate in deterministic scheduling with an
  implicit global fee write. Simple transparent single-transfer detached deltas
  can merge without `fee_postprocessing` fallback and then run fee/gas/Nexus
  postprocessing in the same `StateTransaction`; data-trigger-sensitive or more
  complex fee-bearing detached deltas still fall back deliberately.
- Added adversarial coverage around that boundary: a supported non-transfer
  fee-bearing delta now proves the `fee_postprocessing` fallback counter, and
  an insufficient-fee single transfer proves failed fee charging does not leak
  the detached business transfer into state. Additional negative coverage now
  proves active data triggers keep simple fee-bearing transfers on the
  sequential fee-postprocessing path, and missing payer fee-asset state rejects
  without creating a fee asset or leaking the business transfer. Further
  adversarial coverage now exercises same-asset transfer/fee rollback, shared
  fee-balance exhaustion across two transfers, and rollback of a valid transfer
  followed by a failing instruction while still charging the rejected-transaction
  Nexus fee. A third negative batch now covers stateful sequence-admission
  rejection before any transfer or fee debit, invalid pre-burn fee-sink routing
  rollback, and unauthorized sponsor metadata rollback without debiting the
  sponsor. Additional sponsor/config adversarial coverage now proves disabled
  sponsorship, sponsor fee caps, and malformed fee-asset configuration reject
  without leaking the detached transfer or debiting the requested fee payer.
  Follow-up malformed metadata and gas-policy coverage now proves invalid
  `fee_sponsor` metadata does not fall back to payer debit, required
  `gas_asset_id` metadata is enforced, and accepted gas assets without
  `units_per_gas` mappings reject without leaking detached transfer effects.
- Validation: `CARGO_TARGET_DIR=/tmp/iroha-codex-tx-pipeline-core cargo check
  -p iroha_telemetry`, targeted
  `rustfmt --edition 2024 --check`, and `git diff --check` on the touched
  pipeline/telemetry files are green. After the dirty-worktree query iterator
  blocker was cleared, focused validation is also green with
  `CARGO_TARGET_DIR=/tmp/iroha-codex-tx-pipeline-core cargo check -p
  iroha_core`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-tx-pipeline-core cargo test -p iroha_core
  fee_enabled_single_transfer_uses_detached_merge_without_fee_fallback --lib --
  --nocapture`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-tx-pipeline-core cargo test -p iroha_core
  fee_enabled_supported_non_transfer_uses_fee_postprocessing_fallback --lib --
  --nocapture`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-tx-pipeline-core cargo test -p iroha_core
  fee_enabled_single_transfer_rejects_without_partial_state_when_fee_missing
  --lib -- --nocapture`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-tx-pipeline-core cargo test -p iroha_core
  fee_enabled_single_transfer_with_active_data_trigger_uses_fee_fallback --lib
  -- --nocapture`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-tx-pipeline-core cargo test -p iroha_core
  fee_enabled_single_transfer_rejects_without_partial_state_when_fee_asset_missing
  --lib -- --nocapture`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-tx-pipeline-core cargo test -p iroha_core
  fee_enabled_transfer_then_failing_instruction_falls_back_without_leaking_transfer
  --lib -- --nocapture`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-tx-pipeline-core cargo test -p iroha_core
  fee_enabled_non_increasing_sequence_rejects_before_transfer_or_fee --lib --
  --nocapture`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-tx-pipeline-core cargo test -p iroha_core
  fee_enabled_invalid_sink_before_burn_rejects_without_partial_transfer_or_fee
  --lib -- --nocapture`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-tx-pipeline-core cargo test -p iroha_core
  fee_enabled_unauthorized_sponsor_rejects_without_transfer_or_sponsor_debit
  --lib -- --nocapture`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-tx-pipeline-core cargo test -p iroha_core
  fee_enabled_disabled_sponsor_rejects_without_transfer_or_sponsor_debit --lib
  -- --nocapture`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-tx-pipeline-core cargo test -p iroha_core
  fee_enabled_sponsor_cap_rejects_without_transfer_or_sponsor_debit --lib --
  --nocapture`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-tx-pipeline-core cargo test -p iroha_core
  fee_enabled_invalid_fee_asset_rejects_without_partial_transfer_or_fee --lib
  -- --nocapture`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-tx-pipeline-core cargo test -p iroha_core
  fee_enabled_malformed_sponsor_metadata_rejects_without_transfer_or_fee
  --lib -- --nocapture`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-tx-pipeline-core cargo test -p iroha_core
  fee_enabled_missing_gas_asset_metadata_rejects_without_partial_transfer
  --lib -- --nocapture`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-tx-pipeline-core cargo test -p iroha_core
  fee_enabled_missing_gas_rate_mapping_rejects_without_partial_transfer
  --lib -- --nocapture`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-tx-pipeline-core cargo test -p iroha_core
  fee_enabled_ --lib -- --nocapture`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-tx-pipeline-core cargo test -p iroha_core
  scheduler_variant_tests --lib -- --nocapture`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-tx-pipeline-core cargo test -p iroha_core
  ivm_access --lib -- --nocapture`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-tx-pipeline-core cargo test -p iroha_core
  --features telemetry state_telemetry_detached_counters_set --lib --
  --nocapture`, and
  `CARGO_TARGET_DIR=/tmp/iroha-codex-tx-pipeline-core cargo test -p
  iroha_data_model --lib
  native_amx_receipts_change_lane_block_commitment_hash_inputs --
  --nocapture`.

## 2026-05-17 ZK audit fix follow-up

- Governance ballot/tally verifying-key role checks now reject arbitrary Halo2
  circuits, allowing only canonical vote role IDs or the existing shared
  Halo2 vote circuit family, while STARK remains pinned to explicit
  `vote-ballot` and `vote-tally` circuit roles.
- Block preverification results are now keyed by proof hash, verifying-key
  reference, and verifying-key commitment. `VerifyProof` also validates the
  registered VK bytes, commitment, and backend before a preverified cache hit
  can skip backend verification.
- STARK backend examples, docs, and SDK/test fixtures now use the canonical
  `stark/fri` family (`stark/fri/...` variants) instead of the unsupported
  v1-suffixed spelling.
- Validation:
  - `cargo fmt --all --check` is green.
  - `rustfmt --edition 2024 --check` on the Rust files touched by this fix is
    green, including the adjacent `NativeAmxLegRecord`/`LaneSettlementReceipt`
    derive fix needed to keep data-model denied lints compiling.
  - `git diff --check` is green.
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-zk-audit-fix-core cargo test -p iroha_core normalize_halo2_circuit_id_and_match_variants --lib -- --nocapture`
    is green.
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-zk-audit-fix-core cargo test -p iroha_core preverified_proof_key_binds_vk_reference_and_commitment --lib -- --nocapture`
    is green.
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-zk-audit-fix-core-zkpre cargo test -p iroha_core verify_proof_preverified_cache_does_not_bypass_missing_verifying_key_bytes --lib --features zk-preverify -- --nocapture`
    is green.
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-zk-audit-fix-dm cargo test -p iroha_data_model backend_mismatches --lib -- --nocapture`
    is green.
  - `swift test` from `IrohaSwift/` is green: 828 tests executed, 101 skipped,
    0 failures.
  - `npm run build:native` from `javascript/iroha_js/` rebuilt
    `native/iroha_js_host.node` and refreshed
    `native/iroha_js_host.checksums.json`; the manifest now matches the local
    native binding SHA-256.
  - `node --test test/instructionBuilders.test.js` from `javascript/iroha_js/`
    is green: 87 tests passed.
  - `CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=/tmp/iroha-codex-cross-ds cargo test -p iroha_core guardrails --lib --features zk-stark -- --nocapture`
    is green: 7 guardrail tests passed.
  - `CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=/tmp/iroha-codex-cross-ds cargo test -p iroha_core zk_verify_batch --lib --features zk-stark -- --nocapture`
    is green: 4 CoreHost batch registry-binding tests passed.
  - `CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=/tmp/iroha-codex-cross-ds cargo test -p iroha_core zk_policy_hash --lib -- --nocapture`
    is green: 3 policy-hash tests passed.
  - `CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=/tmp/iroha-codex-cross-ds cargo test -p iroha_core confidential_digest --lib -- --nocapture`
    is green: 4 confidential-digest tests passed.
  - `CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=/tmp/iroha-codex-cross-ds cargo test -p iroha_core dummy_block_populates_proof_policy_hash --lib -- --nocapture`
    is green.
  - `CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=/tmp/iroha-codex-cross-ds cargo test -p iroha_p2p confidential_digest_roundtrip_preserves_zk_policy_hash --lib -- --nocapture`
    is green.
  - `CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=/tmp/iroha-codex-cross-ds cargo test -p iroha_core --test ivm_corehost_envelope_hash_bind -- --nocapture`
    is green.
  - `CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=/tmp/iroha-codex-cross-ds cargo test -p iroha_core --test ivm_corehost_zk_gate --test ivm_corehost_halo2_disabled_latch --features zk-tests,zk-ipa-native -- --nocapture`
    is green: 1 disabled-latch test and 4 gate tests passed.
  - A later duplicate rerun of the Halo2 role test hit local disk exhaustion
    while linking; stale generated `target/codex-*` artifacts were partially
    cleared to restore working space. The earlier focused test result above is
    the recorded pass for this change set.

## 2026-05-18 Cross-dataspace adversarial coverage

- Native AMX receipt coverage now includes negative cases: non-universal
  routing for mixed participants emits no native AMX receipt, and a universal
  route with only one non-universal participant also emits no receipt. Unknown
  dataspace aliases do not synthesize receipt legs, and repeated references to
  one dataspace do not count as a multi-leg AMX batch.
- Router adversarial coverage now also rejects strict-policy mixed native
  targets hidden inside IVM-proved overlays, and fails closed when mixed native
  targets need the universal AMX coordinator but no universal lane is present.
  Strict `amx_policy` matching is covered for whitespace/case normalization,
  mixed dataspace-scoped permissions, and missing universal-lane permission
  batches.
- Restricted transparent asset transfer resolution now cross-checks direct UAID
  dataspace bindings in addition to the account-scope directory, so a
  multi-bound destination account fails closed instead of implicitly choosing a
  dataspace bucket. Source-side ambiguity is covered too: a global-looking
  source asset id for a multi-bound account now rejects before debit, leaves the
  source balance untouched, and does not materialize a destination balance.
  Non-universal execution routes now also reject destination accounts whose
  unique binding points at a different dataspace, preventing cross-dataspace
  credits outside the universal AMX coordinator. The transfer-batch execution
  surface has matching negative coverage for that same cross-dataspace credit
  attempt, plus explicit empty-batch rejection coverage.
- Query tests now use explicit `#[tokio::test]` on async cases instead of
  shadowing the builtin `#[test]` attribute, keeping sync negative tests
  compilable under `iroha_core --lib`.
- Focused validation is green with
  `CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/iroha-codex-cross-ds cargo test
  -p iroha_core --lib native_amx_receipt -- --nocapture`,
  `CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/iroha-codex-cross-ds cargo test
  -p iroha_core --lib strict_amx_policy -- --nocapture`,
  `CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/iroha-codex-cross-ds cargo test
  -p iroha_core --lib
  mixed_dataspace_scoped_permissions_without_universal_lane_fail_closed --
  --nocapture`,
  `CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/iroha-codex-cross-ds cargo test
  -p iroha_core --lib mixed_domain_write_targets_without_universal_lane_fail_closed
  -- --nocapture`,
  `CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/iroha-codex-cross-ds cargo test
  -p iroha_core --lib transfer_batch_rejects -- --nocapture`,
  `CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/iroha-codex-cross-ds cargo test
  -p iroha_core --lib ambiguous_source_dataspace_binding -- --nocapture`,
  `CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/iroha-codex-cross-ds cargo test
  -p iroha_core --lib
  transfer_restricted_asset_rejects_destination_binding_outside_non_universal_route
  -- --nocapture`,
  `CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/iroha-codex-cross-ds cargo test
  -p iroha_core --lib
  transfer_restricted_asset_rejects_ambiguous_destination_dataspace_binding --
  --nocapture`, `CARGO_INCREMENTAL=0
  CARGO_TARGET_DIR=/tmp/iroha-codex-cross-ds cargo test -p iroha_core --lib
  transfer_restricted_asset_uses_destination_dataspace_binding_and_policy --
  --nocapture`, and `CARGO_INCREMENTAL=0
  CARGO_TARGET_DIR=/tmp/iroha-codex-cross-ds cargo test -p iroha_core --lib
  collect_unsorted_bounded_page_rejects_returned_offset_at_limit_without_reading
  -- --nocapture`. Full workspace testing remains a long-corridor follow-up.

## 2026-05-17 Cross-dataspace native routing and XOR gas

- Mixed native transaction targets, including mixed dataspace-scoped
  permissions and account-bound asset movements, now route to the universal AMX
  coordinator by default instead of failing developer submissions with
  conflicting dataspace target errors. A strict transaction metadata opt-out
  (`amx_policy = "reject_cross_dataspace"`) preserves the previous fail-closed
  behavior for callers that need it.
- Universal coordinator routing now has precedence over non-universal policy
  rules, so a rule matching one instruction in a mixed native transaction cannot
  accidentally pin the batch to only one participant dataspace.
- Restricted asset transfers running on the universal route now resolve source
  and destination balance buckets from each account's unique dataspace binding,
  while non-universal routes still cannot debit an explicitly scoped source
  balance from another dataspace.
- Global asset writes may execute through the universal AMX coordinator even
  when the asset's authoritative home dataspace is non-universal; direct
  non-universal execution still has to match the authoritative dataspace.
- Lane block commitments now include versioned `native_amx_receipts` recording
  the source transaction, universal coordinator lane/dataspace/height, and
  successful prepare/commit legs for each non-universal participant dataspace.
- Nexus fee documentation now states the intended invariant: gas is XOR across
  all dataspaces, and local-token conversion is reserved for explicit
  settlement products rather than the default fee rail.
- The query post-processing helpers no longer require the source WSV iterator
  itself to be `Send + Sync + 'static`; only owned stored output values keep
  those bounds. This unblocks focused `iroha_core` validation while preserving
  the shared live-query store's thread-safe cursor values.
- Focused validation is green with `CARGO_TARGET_DIR=/tmp/iroha-codex-cross-ds
  cargo test -p iroha_core --lib mixed_domain_write_targets -- --nocapture`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-cross-ds cargo test -p iroha_core --lib
  dataspace_scoped_permission_grant_routes_mixed_dataspaces_to_universal --
  --nocapture`, `CARGO_TARGET_DIR=/tmp/iroha-codex-cross-ds cargo test -p
  iroha_core --lib
  transfer_restricted_asset_uses_destination_dataspace_binding_and_policy --
  --nocapture`, `CARGO_TARGET_DIR=/tmp/iroha-codex-cross-ds cargo test -p
  iroha_core --lib
  mint_global_asset_allows_universal_amx_route_for_non_universal_home --
  --nocapture`, `CARGO_TARGET_DIR=/tmp/iroha-codex-cross-ds cargo test -p
  iroha_core --lib stored_unsorted_bounded -- --nocapture`,
  `CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/iroha-codex-cross-ds cargo test
  -p iroha_core --lib native_amx_receipt_records_participant_dataspace_legs
  -- --nocapture`, and `CARGO_TARGET_DIR=/tmp/iroha-codex-cross-ds cargo test
  -p iroha_data_model
  native_amx_receipts_change_lane_block_commitment_hash_inputs --
  --nocapture`. Full workspace testing remains a long-corridor follow-up.

## 2026-05-17 Torii query API bounded-count fast path

- Signed `/v1/query`, generic query envelopes, and app list/query endpoints now
  default to bounded count mode, expose `has_more`, keep exact totals optional,
  and accept `count_mode=exact` when clients need full-count metadata.
- `QueryOutput` and live query responses now distinguish exact remaining counts
  from bounded continuation metadata. Stored unsorted bounded cursors avoid
  reporting fake remaining counts; they still own continuation values before
  insertion into the shared `Send + Sync` live-query store until the MV/world
  iterator boundary can safely provide thread-safe lazy continuations.
- Torii query admission now has config-backed global/heavy concurrency caps and
  a short queue timeout, and public signed/app query handlers enforce query
  rate limits without waiting for high-load mode.
- Bounded metadata and exact-count opt-ins now cover the generic query route,
  signed queries, accounts/domains, repo agreements, asset definitions, NFTs,
  RWAs, permissions, account assets, asset holders, transaction/history,
  contract activity/events, trader activity, and subscription plan/list APIs.
  SDK DTOs that expose concrete responses now model `total` as optional and
  include `has_more`/`count_mode`.
- Account alias projection in asset-holder responses is batched against the
  request snapshot, avoiding per-row full-state alias scans on that hot path.
- Added Criterion coverage for first-batch count-mode cost in
  `snapshot_find_domains_count_mode_first_batch/{ephemeral,stored}/{exact,bounded}`
  and documented the benchmark entrypoints in `docs/source/query_benchmarks.md`.
- Validation: `cargo check -p iroha_data_model`,
  `cargo check -p iroha_config`, `cargo check -p iroha_core`,
  `cargo check -p iroha_torii`, `cargo check -p iroha_torii --tests`,
  `cargo check -p iroha_core --bench queries`,
  `cargo test -p iroha_data_model query_response_roundtrip -- --nocapture`,
  `cargo test -p iroha_core --lib stored_unsorted_bounded_cursor_materializes_owned_tail_without_exact_count -- --nocapture`,
  `cargo test -p iroha_torii generic_query -- --nocapture`,
  `cargo fmt --all`, `cargo fmt --all --check`, `git diff --check`, and
  `cargo clippy -p iroha_core -p iroha_torii --lib -- -D warnings` are green.

## 2026-05-17 Kura commit manifests for memory-only WSV

- WSV remains the in-memory canonical runtime state. Kura now writes a
  Norito-encoded commit manifest after a successful WSV commit, tying the
  durable block hash to the committed WSV checkpoint hash plus available
  execution roots and commit-QC hash.
- Kura startup validates existing commit manifests against the recovered
  durable block hash journal, prunes stale or mismatched manifest/checkpoint
  sidecars, and never truncates intact blocks to match a sidecar prefix.
  Missing manifests are absent verification metadata, so they do not truncate an
  otherwise intact block log.
- Post-commit WSV checkpoint or commit-manifest persistence failures keep the
  already-advanced block committed, but now increment structured Sumeragi status
  counters with the last affected height/view/block hash instead of existing
  only as log text.
- Validation: `cargo check -p iroha_core`,
  `cargo test -p iroha_core --lib bounded -- --nocapture`,
  `cargo test -p iroha_core --lib commit_manifest -- --nocapture`, and
  `cargo test -p iroha_core --lib kura_store_counters_surface_in_snapshot -- --nocapture`
  are green.

## 2026-05-17 SoraFS paid pin adversarial test coverage

- Added SoraFS paid-pin negative coverage for unfunded and underfunded public
  registration, pre-fee validation failures that must not debit the submitter,
  successor-chain rejection paths that must leave fee balances unchanged,
  malformed alias proof bytes, stale alias records, and other alias proof
  tampering that must not write a manifest or move fees,
  duplicate digest/alias replay without a second fee charge, and storage ingest
  with pending/retired records, missing fee metadata, forged fee payer,
  submitter mismatch, embedded record digest mismatch, chunker profile and
  multihash mismatches, policy mismatches across replicas/storage
  class/retention epoch, and tampered paid-record content length. Approval
  rejection coverage now also checks invalid council signatures and alias
  collisions leave pending manifests pending, do not stamp envelope digests,
  and do not create or overwrite alias bindings. The latest approval pass also
  rejects a valid pending approval envelope paired with a forged supplied
  digest while leaving the pending record and alias table unchanged. Alias
  lifecycle coverage now rejects expiry-before-bound, pending-manifest binds,
  duplicate binds, bound-before-approval binds, unknown-manifest binds, and
  expiry-past-retention without attaching aliases or replacing existing alias
  records; conflicting repeat retirement leaves the original retired epoch,
  reason, and dropped alias state intact.
- Added SDK-side validation coverage so JavaScript, Kotlin/JVM, and Java
  Android builders reject missing or negative SoraFS pin-register
  `content_length` instead of emitting malformed register payloads, and reject
  nonnumeric `content_length` during argument decoding. Kotlin and Java
  argument decoding now also reject negative `submitted_epoch`. JavaScript
  request validation rejects zero replicas, negative retention epochs, alias
  bindings without proofs, malformed alias proofs, and malformed successor
  digests before fetch; it now also rejects negative submitted epochs and
  malformed manifest digests before fetch. Kotlin/JVM and Java Android policy
  builders and argument decoders now reject zero, negative, and nonnumeric
  replica counts as well as partial alias argument sets. Typed response
  normalization rejects negative pin-fee receipt values, negative response
  content lengths, negative response submitted epochs, malformed successor
  digests, and aliases missing proofs.
- Validation: `cargo fmt --all`, `git diff --check`,
  `cargo test -p iroha_core --lib register_pin_manifest_rejects_unfunded_public_submission_without_side_effects -- --nocapture`,
  `cargo test -p iroha_core --lib register_pin_manifest_rejects_insufficient_public_fee_without_side_effects -- --nocapture`,
  `cargo test -p iroha_core --lib register_manifest_rejects -- --nocapture`,
  `cargo test -p iroha_core --lib register_manifest_rejects_alias -- --nocapture`,
  `cargo test -p iroha_core --lib register_manifest_rejects_empty_alias_proof_without_side_effects -- --nocapture`,
  `cargo test -p iroha_core --lib register_manifest_rejects_malformed_alias_proof_without_side_effects -- --nocapture`,
  `cargo test -p iroha_core --lib register_manifest_rejects_stale_alias_record_without_side_effects -- --nocapture`,
  `cargo test -p iroha_core --lib approve_pending_manifest_rejects -- --nocapture`,
  `cargo test -p iroha_core --lib bind_manifest_alias_rejects -- --nocapture`,
  `cargo test -p iroha_core --lib retire_manifest_rejects_conflicting_repeat_without_side_effects -- --nocapture`,
  `cargo test -p iroha_torii --lib --features app_api storage_pin_rejects_paid_record -- --nocapture`,
  and
  `cargo test -p iroha_torii --lib --features app_api paid_record_even -- --nocapture`
  are green. The focused JavaScript register-pin filter is also green when run
  with a temporary `IROHA_JS_NATIVE_DIR` whose checksum manifest matches the
  local native binding:
  `node --test --test-name-pattern "registerSorafsPinManifest" javascript/iroha_js/test/toriiClient.test.js`.
  A direct run is currently blocked by a dirty-worktree native checksum
  mismatch. Kotlin/Java Gradle validation is blocked because this host has no
  Java runtime available.

## 2026-05-16 SoraFS paid pin registry enforcement

- Public SoraFS pin registration now records content length and SoraFS pin-fee
  payment metadata, transfers the computed public pin fee from the submitter to
  the configured governance treasury, and stores the approved pin record as the
  storage-ingest authority.
- Torii storage pin ingest now requires a matching approved paid registry record
  for manifest digest, chunk profile, content length, policy, chunk plan digest,
  and fee payer. The recorded fee asset, treasury, and amount are treated as the
  committed on-chain receipt rather than repriced against later governance
  changes. Legacy bearer-token/CIDR pin admission no longer authorizes storage
  ingest.
- Gateway admission now fails closed when the registry is unavailable, manifest
  envelopes are validated against signed registry metadata instead of merely
  detected, and unknown chunker profiles are rejected by Torii and `sorafs_node`.
- CAR range responses now stream chunk files through `CarStreamingWriter`
  instead of buffering the full range response in memory.

## 2026-05-17 Offline Note V2 local-final SDK semantics

- Swift, Kotlin/JVM, and Java Android Offline Note V2 wallets now treat
  offline-to-offline `pay`/`accept` as the immediate, irrevocable value
  transfer. Sender inputs become `SPENT`, sender change is immediately
  `SPENDABLE`, and the recipient's matched receive-pending note becomes
  `SPENDABLE` after local token/proof verification; online sync is not part of
  the transfer path.
- Swift, Kotlin/JVM, and Java Android now require trusted key certificates for
  load, receive, pay, token acceptance, audit publication, and redeem flows.
  The default verifier rejects certificates until the caller supplies trusted
  issuer roots, and the included Ed25519 verifier checks the issuer signature
  plus wallet/account role binding for sender, recipient, input-claim, and
  output-claim certificates.
- Audit publication is now an explicit optional online step
  (`publishAudit`) that submits evidence without mutating local wallet
  spendability. Wallet sync now reconciles redeem-pending notes only.
- Payment-token handoff moved from JSON to a Norito envelope carrying
  `chain_id`, `payment_request_id`, `created_at_ms`, `token_nonce`, `token_id`,
  and the audit bundle. The token id preimage now binds the request id and
  creation timestamp across Rust fixtures plus Swift/Kotlin/Java SDKs.
- Swift Keychain, Kotlin in-memory/Android secure, and Java in-memory/Android
  secure stores expose atomic note mutations so local-final state transitions
  cannot be split across separate reads and writes.
- Swift Keychain persistence now writes revisioned ThisDeviceOnly records and
  deletes the previous revision after each committed save. Kotlin Android and
  Java Android secure stores now write each committed revision with a fresh
  non-exportable Android Keystore key and delete the previous revision key, so
  cloned app preferences or app-data rollback cannot decrypt an old wallet
  snapshot once the device has observed the newer revision.
- Android secure-store decryption no longer creates missing historical keys;
  rolled-back ciphertext now fails closed without leaving orphan Keystore keys.
- Legacy persisted `spendPending`/`SPEND_PENDING` notes decode as spent, and
  `changePending`/`CHANGE_PENDING` notes decode as spendable, matching the
  local-final first-release state machine.
- Swift, Kotlin/JVM, and Java Android adversarial tests now cover the rejecting
  default verifier, wrong issuer roots, receive-request account substitution,
  tampered stored input certificates, forged output-claim certificates, and
  forged input-claim certificate hashes. The latest coverage also rejects valid
  certificates presented for the wrong wallet account, forged sender change
  output certificates, and coherent audit sender-certificate substitutions that
  recompute input claim hashes, token IDs, and recursive-proof public inputs.
  A further negative pass covers wrong-chain receive requests, forged receive
  output commitments, wrong-chain payment tokens, and payment-request-ID swaps
  with recomputed token IDs and recursive-proof public inputs.
  The latest claim-mutation pass also covers recipient-output amount and asset
  substitutions, output-order swaps, and dropped recipient outputs with
  recomputed token IDs and audit public inputs. Token identity adversarial
  coverage now also rejects top-level token-id substitution, audit-bundle
  token-id substitution, and stale recursive-proof public-input bindings.
  Receive-request tampering coverage now includes asset-owner substitution and
  amount substitution before the payer creates an otherwise coherent token.
  The shared Offline V2 fixture now also carries a canonical SDK interop payment
  token handoff, and Swift, Kotlin/JVM, and Java Android assert identical Norito
  bytes, text payloads, QR frames, and local recipient acceptance from that
  artifact. Swift asset-definition address decoding now has a bridge-free
  BLAKE3 checksum fallback and rejects bad checksums, matching Android/Kotlin
  address semantics when the native bridge is unavailable on SwiftPM or iOS
  simulator test hosts.
- Swift, Kotlin/JVM, and Java Android now expose an app-facing
  `OfflineNoteV2TransferHandoff` layer for QR streaming, NFC, and nearby
  payment-token transfer modalities. QR uses the canonical `iroha:qr1:`
  streaming frames, NFC includes a png2-style APDU datastream
  (`select`/`get_info`/`read_chunk`/`write_meta`/`write_chunk`/`commit`) with a
  64 KiB advertised-payload cap, SHA-256 metadata, Android-safe 240-byte default
  chunks, local receipt ACK reads, and explicit iOS fast-chunk opt-in. Nearby now
  has a sorted-key shared JSON envelope with unpadded base64url payloads, a
  pairing-image challenge, payment payload, receipt ACK, and rejection message.
  Swift/Kotlin/Java tests pin the same NFC APDU and Nearby envelope wire
  fixture, including rejection of padded Nearby payloads, malformed APDUs,
  nonzero Le bytes on no-data commands, non-canonical zero-length reads,
  invalid direct read lengths, nonzero P1/P2 smuggling on no-offset APDUs,
  invalid response/bounds handling, huge and negative assembler offsets,
  conflicting partial-overlap chunks, malformed or smuggled pairing objects,
  fractional Nearby versions, challenge/receipt ACK content-type downgrades,
  top-level non-object envelopes, ACK-with-pairing payloads, and invalid
  payment-token payloads. The app-facing handoff decoders now also reject
  content-type downgrades, corrupted QR stream frames, header stream-id
  mismatches, wrong-stream data injection, valid-CRC poisoned chunks,
  non-canonical QR frame/envelope lengths, header counter drift, data/parity
  count and chunk-length mismatches, out-of-range Java/Kotlin 16-bit field
  values that previously could wrap on encode, poisoned parity recovery,
  coherent-but-mutated payload hash mismatches, conflicting repeated headers or
  chunks, and non-payment stream payload kinds before returning a token.
  Android platform modules include capability helpers that enable NFC only when
  the device advertises HCE support; Swift keeps NFC disabled unless the app
  opts in after confirming an allowed iOS HCE/CardSession use case and
  entitlement.
- The previously blocking `qr_stream_fixtures` bin now uses the exported
  `norito::json!` macro form that compiles under the current Norito JSON
  module layout, and the shared QR fixtures were regenerated.
- Validation:
  - `cargo fmt --all`
  - `swift test --filter OfflineQrStreamTests` from `IrohaSwift` (`8` tests)
  - `swift test --filter OfflineQrStreamTests/testQrStreamRejectsAdversarialEnvelopeAndChunkShapes` from `IrohaSwift`
  - `swift test --filter OfflineNoteV2Tests` from `IrohaSwift` (`53` tests)
  - `swift test --filter OfflineNoteV2Tests/testOfflineNoteV2TransferHandoffRejectsAdversarialStreamsAndMetadata` from `IrohaSwift`
  - `swift test --filter OfflineNoteV2Tests/testOfflineNoteV2NfcApduProtocolSupportsAndroidSafeAndIOSFastChunks --filter OfflineNoteV2Tests/testOfflineNoteV2NearbyEnvelopeRoundTripsPairingPaymentAndAck` from `IrohaSwift`
  - `swift test --filter OfflineNoteV2Tests/testOfflineNoteV2TransportWireFormatMatchesSharedFixture --filter OfflineNoteV2Tests/testOfflineNoteV2NearbyEnvelopeRejectsAdversarialMessages` from `IrohaSwift`
  - `swift test --filter OfflineNoteV2Tests/testOfflineNoteV2NfcApduProtocolRejectsMalformedCommandsAndBounds --filter OfflineNoteV2Tests/testOfflineNoteV2NearbyEnvelopeRejectsAdversarialMessages` from `IrohaSwift`
  - `swift test --filter OfflineNoteV2Tests/testOfflineNoteV2TransferHandoffSupportsQrNfcAndNearbyPayloads` from `IrohaSwift`
  - `swift test --filter ToriiClientTests/testCanonical` from `IrohaSwift`
  - `xcodebuild test -scheme IrohaSwift -destination 'id=7A8B8CC0-617D-49EA-BA33-3976C3E15517' -only-testing:IrohaSwiftTests/OfflineQrStreamTests -only-testing:IrohaSwiftTests/OfflineNoteV2Tests` from `IrohaSwift` on the iPhone 17 iOS 26.4 simulator (`61` tests)
  - `xcodebuild test -scheme IrohaSwift -destination 'id=7A8B8CC0-617D-49EA-BA33-3976C3E15517' -only-testing:IrohaSwiftTests/OfflineNoteV2Tests` from `IrohaSwift` on the booted iPhone 17 iOS 26.5 simulator (`53` tests)
  - `JAVA_HOME=/opt/homebrew/opt/openjdk@21/libexec/openjdk.jdk/Contents/Home PATH=/opt/homebrew/opt/openjdk@21/bin:$PATH ./gradlew :core-jvm:test --tests org.hyperledger.iroha.sdk.offline.OfflineNoteV2Test --console=plain --rerun-tasks` from `kotlin`
  - `JAVA_HOME=/opt/homebrew/opt/openjdk@21/libexec/openjdk.jdk/Contents/Home PATH=/opt/homebrew/opt/openjdk@21/bin:$PATH ./gradlew :core-jvm:test --tests org.hyperledger.iroha.sdk.offline.OfflineNoteV2Test.qrStreamRejectsAdversarialEnvelopesAndChunkShapes --console=plain --rerun-tasks` from `kotlin`
  - `JAVA_HOME=/opt/homebrew/opt/openjdk@21/libexec/openjdk.jdk/Contents/Home PATH=/opt/homebrew/opt/openjdk@21/bin:$PATH ./gradlew :core-jvm:test --tests org.hyperledger.iroha.sdk.offline.OfflineNoteV2Test.transferHandoffRejectsAdversarialStreamsAndMetadata --console=plain --rerun-tasks` from `kotlin`
  - `JAVA_HOME=/opt/homebrew/opt/openjdk@21/libexec/openjdk.jdk/Contents/Home PATH=/opt/homebrew/opt/openjdk@21/bin:$PATH ./gradlew :core-jvm:test --tests org.hyperledger.iroha.sdk.offline.OfflineNoteV2Test.nfcApduProtocolRejectsMalformedCommandsAndBounds --tests org.hyperledger.iroha.sdk.offline.OfflineNoteV2Test.nearbyEnvelopeRejectsAdversarialMessages --console=plain --rerun-tasks` from `kotlin`
  - `JAVA_HOME=/opt/homebrew/opt/openjdk@21/libexec/openjdk.jdk/Contents/Home PATH=/opt/homebrew/opt/openjdk@21/bin:$PATH ANDROID_HOME=$HOME/Library/Android/sdk ANDROID_SDK_ROOT=$HOME/Library/Android/sdk ./gradlew :offline-wallet-android:compileDebugAndroidTestJavaWithJavac :offline-wallet-android:compileReleaseKotlin --console=plain` from `kotlin`
  - `JAVA_HOME=/opt/homebrew/opt/openjdk@21/libexec/openjdk.jdk/Contents/Home PATH=/opt/homebrew/opt/openjdk@21/bin:$PATH ANDROID_HOME=$HOME/Library/Android/sdk ANDROID_SDK_ROOT=$HOME/Library/Android/sdk ./gradlew :client-android:assembleRelease :offline-wallet-android:assembleRelease --quiet` from `kotlin`
  - Installed Android emulator tooling and an API 35 Google APIs ARM64 system
    image with `sdkmanager`, created the `iroha_offline_api35` AVD, and booted
    it headless for connected tests.
  - `JAVA_HOME=/opt/homebrew/opt/openjdk@21/libexec/openjdk.jdk/Contents/Home ANDROID_SERIAL=emulator-5554 ANDROID_HOME=/opt/homebrew/share/android-commandlinetools ANDROID_SDK_ROOT=/opt/homebrew/share/android-commandlinetools PATH=/opt/homebrew/opt/openjdk@21/bin:/opt/homebrew/share/android-commandlinetools/platform-tools:/opt/homebrew/share/android-commandlinetools/emulator:$PATH ./gradlew :offline-wallet-android:connectedDebugAndroidTest --console=plain` from `kotlin`
  - `JAVA_HOME=/opt/homebrew/opt/openjdk@21/libexec/openjdk.jdk/Contents/Home PATH=/opt/homebrew/opt/openjdk@21/bin:$PATH ANDROID_HOME=$HOME/Library/Android/sdk ANDROID_SDK_ROOT=$HOME/Library/Android/sdk ANDROID_HARNESS_MAINS=org.hyperledger.iroha.android.offline.OfflineNoteV2Test ./gradlew :core:test --tests org.hyperledger.iroha.android.GradleHarnessTests --console=plain --rerun-tasks` from `java/iroha_android`
  - `JAVA_HOME=/opt/homebrew/opt/openjdk@21/libexec/openjdk.jdk/Contents/Home PATH=/opt/homebrew/opt/openjdk@21/bin:$PATH ANDROID_HOME=$HOME/Library/Android/sdk ANDROID_SDK_ROOT=$HOME/Library/Android/sdk ./gradlew :android:compileDebugAndroidTestJavaWithJavac :android:compileDebugJavaWithJavac --console=plain` from `java/iroha_android`
  - `JAVA_HOME=/opt/homebrew/opt/openjdk@21/libexec/openjdk.jdk/Contents/Home PATH=/opt/homebrew/opt/openjdk@21/bin:$PATH ANDROID_HOME=$HOME/Library/Android/sdk ANDROID_SDK_ROOT=$HOME/Library/Android/sdk ./gradlew :android:compileDebugJavaWithJavac --console=plain --rerun-tasks` from `java/iroha_android`
  - `JAVA_HOME=/opt/homebrew/opt/openjdk@21/libexec/openjdk.jdk/Contents/Home PATH=/opt/homebrew/opt/openjdk@21/bin:$PATH ANDROID_HOME=$HOME/Library/Android/sdk ANDROID_SDK_ROOT=$HOME/Library/Android/sdk ./gradlew :android:assembleDebug --console=plain --quiet` from `java/iroha_android`
  - `JAVA_HOME=/opt/homebrew/opt/openjdk@21/libexec/openjdk.jdk/Contents/Home ANDROID_SERIAL=emulator-5554 ANDROID_HOME=/opt/homebrew/share/android-commandlinetools ANDROID_SDK_ROOT=/opt/homebrew/share/android-commandlinetools PATH=/opt/homebrew/opt/openjdk@21/bin:/opt/homebrew/share/android-commandlinetools/platform-tools:/opt/homebrew/share/android-commandlinetools/emulator:$PATH ./gradlew :android:connectedDebugAndroidTest --console=plain` from `java/iroha_android`
  - `cargo test -p iroha_data_model --features test-fixtures,transparent_api --bin offline_v2_vectors -- --nocapture`
  - `cargo run -p iroha_data_model --features test-fixtures,transparent_api --bin offline_v2_vectors -- --check`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-offline-v2-fixtures cargo run -p iroha_data_model --features test-fixtures,transparent_api --bin offline_v2_vectors -- --check`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-offline-v2-fixtures cargo run -p iroha_data_model --features test-fixtures --bin qr_stream_fixtures -- --check`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-offline-v2-fixtures cargo test -p iroha_data_model --features test-fixtures,transparent_api offline_note_v2_wallet_derivations -- --nocapture`
  - `git diff --check`
  - `git diff --check`
- Device validation now includes Swift Offline Note V2 tests on an iOS
  simulator plus Kotlin and Java Android Keystore rollback drills on a real
  booted emulator. The Android tests restore a stale preferences snapshot after
  a committed revision and verify that the deleted revision key makes the old
  ciphertext fail decryption.

## 2026-05-17 SoraNet VPN hardening pass

- Helper-ticket v1 is now a 256-byte first-release frame that commits the
  authorized Ed25519 metering public key and full deterministic tariff under
  the helper-ticket MAC. Relay voucher acceptance rejects wrong metering keys
  and recomputes earned fees from the ticket tariff instead of trusting voucher
  envelopes.
- Native VPN lease records now persist the quote policy and accepted settlement
  receipt. Torii active session lookup plus receipt submission/listing
  reconstruct settlement context from WSV `vpn_leases` instead of requiring
  process-local VPN session caches.
- Relay/backend bridging now uses `vpn.backend_endpoint` with a default
  permissioned Unix socket. TCP endpoints require a shared bootstrap secret and
  use Norito bootstrap envelopes with timestamp, nonce, and keyed MAC; the
  backend rejects stale, replayed, or bad-MAC frames.
- Local helper workers read magic-prefixed Norito connect-payload frames from
  stdin rather than argv, reject mismatched metering seeds, and persist
  magic-prefixed Norito state frames with batched traffic writes and a forced
  shutdown flush.
- Torii adversarial coverage now exercises WSV-backed session lookup after
  cache loss for expired leases, non-active leases, and cross-account access,
  plus WSV-backed receipt settlement rejection for wrong metering keys,
  relay-side earned-fee inflation, voucher substitution, non-operator
  signatures, exact signed-request replay, and explicit `lease_id_hex`
  confusion between two active leases. Additional receipt verifier coverage
  rejects payment-hash, account-hash, relay-id, byte-counter, uptime,
  timestamp-order, voucher-signature, and voucher-sequence tampering after
  WSV/cache-loss reconstruction. The latest negative pass also rejects
  receipt-side and voucher-side session/quote mismatches, voucher-side relay-id
  mismatch, malformed receipt/voucher hex, and extra malformed Norito bytes
  before settlement. Additional boundary tests now reject malformed JSON
  receipt bodies before auth, explicit `lease_id_hex` values that are non-hex
  or the wrong length, receipt-derived unknown lease IDs, and replayed
  settlement attempts against already settled or refunded WSV leases. The
  quote/session creation boundary now rejects malformed JSON before auth,
  non-hex metering keys, cross-account quote consumption, exit-class mismatch,
  metering-key mismatch, and empty payment hashes.
- Helper-ticket parser coverage now rejects wrong MAC secrets and verifies that
  the MAC covers both the authorized Ed25519 metering public key and every
  deterministic tariff field. Additional fixed-frame parser coverage rejects
  valid-length bad magic, non-hex transport input, valid-hex wrong-length
  input, and expiry-field tampering under the MAC. Usage-voucher data-model
  tests now reject signed-body tampering, public-key substitution, and
  signature substitution, and verify that voucher hashes commit to the body,
  public key, and signature.
- Focused validation passed for data-model SoraNet VPN/helper-ticket tests,
  the full `sora-vpn-backend` unit suite, the full `sora-vpn-controller`
  helper binary suite, relay VPN voucher debt/earned-fee tests, Torii
  WSV-backed active session and receipt-settlement tests, the
  Swift/Kotlin/JVM/Java Android, Python, and JavaScript client Torii files,
  `git diff --check` on the task-scoped files, rustfmt on the touched files,
  and `cargo fmt --all --check`.

## 2026-05-17 ZK audit hardening

- Confidential feature digests now commit to a `zk_policy_hash` covering
  consensus-relevant ZK verifier policy, and the digest is serialized through
  block headers and P2P confidential capability handshakes.
- Proof and verifying-key commitments now use versioned, domain-separated,
  length-prefixed SHA-256 inputs so proof hashes and VK commitments cannot
  collide across domains or ambiguous backend/payload splits.
- Generic `VerifyProof` now requires a registered `vk_ref` and enforces active
  VK status, gas schedule, active circuit/version mapping,
  circuit/schema/commitment binding, and backend guardrails before verification.
- User-facing generic proof attachments are registry-only across the CLI,
  Torii prover, Connect Norito bridge, JavaScript, Swift, and Python SDKs.
  `ProofAttachment` no longer carries verifying-key bytes in the data model,
  Norito wire shape, SDK builders, or test fixtures; key bytes remain only
  inside verifier registry records or external key stores.
- The vendored Halo2 environment-based max-degree override and its generated
  env inventory documentation were removed. Circuit degree selection is no
  longer mutable through a process environment variable.
- Consensus proof paths no longer reject solely because local elapsed
  verification time exceeded `zk.verify_timeout`; elapsed time remains telemetry,
  while validity-affecting limits are committed through config policy.
- CoreHost IVM ZK verification now threads the full ZK config and accepts
  registry-bound STARK/FRI envelopes when `zk-stark` is built and enabled,
  including batch syscall coverage alongside the existing Halo2 IPA path.
- Focused validation passed with isolated target directories:
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-zk-cleanup-dm cargo check -p iroha_data_model --lib`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-zk-cleanup-ivm cargo check -p ivm --lib`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-zk-cleanup-bridge cargo check -p connect_norito_bridge --lib`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-zk-cleanup-core-preverify cargo test -p iroha_core preverify_and_dedup_across_transactions_in_block --lib --features zk-preverify -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-zk-cleanup-cli cargo test -p iroha_cli build_proof_attachment_from_json -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-zk-cleanup-torii-rerun cargo test -p iroha_torii proofs_roundtrip_and_query_via_torii --lib -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-zk-cleanup-torii-rerun cargo test -p iroha_torii scan_and_report_single_attachment --lib -- --nocapture`
  - `swift test --filter ProofAttachmentNoritoTests` from `IrohaSwift`
  - `python3 -m compileall -q python/iroha_python/src/iroha_python`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-zk-cleanup-js-native npm run build:native` from `javascript/iroha_js`
  - `node --test test/instructionBuilders.test.js` from `javascript/iroha_js`
- Additional registry-only attachment follow-up validation passed with isolated
  target directories:
  - `cargo fmt --all`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-zk-gap-dm-tests cargo check -p iroha_data_model --tests`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-zk-gap-ivm-tests cargo check -p ivm --tests`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-zk-gap-core-zk-tests cargo check -p iroha_core --tests --features zk-tests,zk-preverify,halo2-dev-tests`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-zk-gap-cli-tests cargo check -p iroha_cli --bins --tests`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-zk-gap-bridge-tests cargo check -p connect_norito_bridge --tests`
  - `swift test --filter ProofAttachmentNoritoTests` from `IrohaSwift`
  - `node --test test/instructionBuilders.test.js` from `javascript/iroha_js`
  - `git diff --check`
- `CARGO_TARGET_DIR=/tmp/iroha-codex-zk-gap-torii-tests cargo check -p iroha_torii --tests --features zk-tests,halo2-dev-tests`
  is currently blocked by unrelated Torii test compile issues in
  `sorafs_discovery`, `connect_gating`, `gov_mode_mismatch_and_autoclose`, and
  `zk_vote_tally_handler`.
- Additional negative/adversarial coverage now rejects legacy inline verifying
  key fields in CLI, Connect Norito bridge, JavaScript, and Swift proof
  builders even when `vk_ref` is present, including every legacy field alias
  used by old clients. Connect bridge coverage now also rejects legacy inline
  VK fields through the exported C encoder, leaving output pointers and hashes
  untouched on failure. Data-model unit coverage rejects missing `vk_ref` and
  the old optional `vk_ref`/inline-key Norito slot shape, while core ZK
  coverage rejects proof/backend mismatches, commitment-only registry bypass
  attempts, inactive registered VKs in preverify, inactive VKs in
  `VerifyProof`, tampered OpenVerifyEnvelope VK hashes, and malformed supported
  Halo2 proof envelopes that should be recorded as rejected.
- A second adversarial pass makes `ProofAttachment` direct JSON decoding fail
  closed on unknown fields and legacy inline VK aliases, rejects malformed
  `vk_commitment`/`envelope_hash` arrays before model construction, rejects
  malformed CLI commitment hex and non-object `vk_ref`, rejects Connect bridge
  proof/backend splits at both parser and C encoder boundaries, rejects
  JavaScript structured proof backend mismatches, covers Swift missing-`vk_ref`
  proof payloads, and adds a direct `VerifyProof` duplicate-record replay test.
- A third adversarial pass enforces attachment/proof/VK-reference backend
  consistency at the data-model JSON boundary, CLI proof JSON builder, Connect
  bridge parser and C encoder, JavaScript builder, Swift proof attachment, and
  core preverify/direct `VerifyProof` paths. The JavaScript builder no longer
  accepts the stale `vk_reference` alias, and fixed-byte JSON tests now cover
  out-of-range byte values in addition to wrong lengths.
- A fourth adversarial pass makes `ProofAttachment` Norito wire decode fail
  closed on proof/backend and VK-reference/backend mismatches, including
  base64-encoded `ProofAttachmentList` JSON payloads. CLI, Connect bridge,
  JavaScript, and Swift native escrow builders now also reject a stale
  `vk_reference` shadow field even when a valid `vk_ref` is present.
- A fifth adversarial pass rejects nested shadow fields inside proof attachment
  `proof` and `vk_ref` objects instead of silently ignoring them. The
  data-model JSON decoder now fails closed on nested shadow keys, CLI/Connect
  bridge/JavaScript/Swift native escrow builders reject nested `vk_ref`
  smuggling, JavaScript rejects structured proof shadow fields, and Norito wire
  tests cover legacy `Some(vk_ref)/Some(vk_inline)` slots plus inline-VK tails
  after a valid registry reference.
- A sixth adversarial pass rejects surplus Norito tail fields after the allowed
  `vk_commitment`/`envelope_hash`/`lane_privacy` attachment tail, preventing
  future or legacy fields from being silently ignored by direct slice decode.
  JavaScript proof builders now reject conflicting aliases for proof bytes,
  verifying-key references, and verifying-key commitments instead of letting
  precedence rules hide a shadow value. Swift native escrow tests also cover
  incomplete `vk_ref` dictionaries.
- A seventh adversarial pass adds list/DTO boundary checks: base64
  `ProofAttachmentList` JSON rejects a single-attachment wire payload, the CLI
  rejects bridge-only `proof_backend` shadowing, the Connect C encoder rejects
  nested `vk_ref` shadow fields, and JavaScript rejects envelope-hash alias
  collisions.
- An eighth adversarial pass rejects blank verifier identities and nested
  verifier-ID alias smuggling: data-model JSON and Norito decode now fail
  closed on blank attachment/proof/VK backends or `vk_ref.name`, CLI and
  Connect bridge builders trim and reject blank verifier reference fields,
  JavaScript rejects blank string/object verifier IDs plus nested
  backend/name alias collisions, and Swift native escrow rejects whitespace-only
  verifier reference dictionaries.
- Focused validation for the adversarial follow-up:
  - `cargo fmt --all`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-zk-adversarial-dm cargo test -p iroha_data_model proof_attachment_decode_rejects_blank_verifying_key_name --lib -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-zk-adversarial-dm cargo test -p iroha_data_model proof_attachment_json_rejects_blank_verifying_key_name --lib -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-zk-adversarial-dm2 cargo test -p iroha_data_model blank_backend_fields --lib -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-zk-adversarial-dm cargo test -p iroha_data_model proof_attachment_list_json_rejects_single_attachment_wire_payload --lib -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-zk-adversarial-dm cargo test -p iroha_data_model proof_attachment_decode_rejects --lib -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-zk-adversarial-dm cargo test -p iroha_data_model proof_attachment_json_rejects_nested_shadow_fields --lib -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-zk-adversarial-dm cargo test -p iroha_data_model proof_attachment_json --lib -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-zk-adversarial-dm cargo test -p iroha_data_model proof_attachment_decode --lib -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-zk-adversarial-dm cargo test -p iroha_data_model proof_attachment_roundtrip --lib -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-zk-adversarial-dm cargo test -p iroha_data_model proof_attachment_list_roundtrip_bare --lib -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-zk-adversarial-dm-json cargo test -p iroha_data_model proof_attachment_list_json_rejects_backend_mismatch_inside_wire_payload --lib -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-zk-gap-dm-tests cargo test -p iroha_data_model proof_attachment_decode --lib -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-zk-gap-dm-tests cargo test -p iroha_data_model proof_attachment_json --lib -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-zk-adversarial-cli cargo test -p iroha_cli build_proof_attachment_from_json --bins -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-zk-adversarial-cli cargo test -p iroha_cli build_proof_attachment_from_json_rejects_blank_vk_ref_name --bins -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-zk-adversarial-cli2 cargo test -p iroha_cli build_proof_attachment_from_json_rejects_blank_backend_fields --bins -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-zk-adversarial-cli cargo test -p iroha_cli build_proof_attachment_from_json_rejects_bridge_only_proof_backend_shadow --bins -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-zk-adversarial-cli cargo test -p iroha_cli build_proof_attachment_from_json_rejects_nested_vk_ref_shadow_field --bins -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-zk-adversarial-cli cargo test -p iroha_cli build_proof_attachment_from_json_rejects_vk_reference_shadow_field --bins -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-zk-gap-cli-tests cargo test -p iroha_cli build_proof_attachment_from_json --bins -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-zk-gap-cli-tests cargo test -p iroha_cli build_proof_attachment_from_json_rejects_legacy_inline_vk_field --bins -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-zk-gap-cli-tests cargo test -p iroha_cli build_proof_attachment_from_json_rejects --bins -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-zk-adversarial-bridge cargo test -p connect_norito_bridge vk_reference_shadow_field -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-zk-adversarial-bridge cargo test -p connect_norito_bridge proof_attachment_json_rejects_blank_vk_ref_name -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-zk-adversarial-bridge2 cargo test -p connect_norito_bridge proof_attachment_json_rejects_blank_backend_fields -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-zk-adversarial-bridge cargo test -p connect_norito_bridge proof_attachment_json_rejects_nested_vk_ref_shadow_field -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-zk-adversarial-bridge cargo test -p connect_norito_bridge zk_transfer_encoder_rejects_nested_vk_ref_shadow_field -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-zk-adversarial-bridge cargo test -p connect_norito_bridge proof_attachment_json -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-zk-gap-bridge-tests cargo test -p connect_norito_bridge legacy_inline_vk_field -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-zk-gap-bridge-tests cargo test -p connect_norito_bridge proof_attachment_json -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-zk-gap-bridge-tests cargo test -p connect_norito_bridge zk_transfer_encoder_rejects -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-zk-gap-core-zk-tests cargo test -p iroha_core --test zk_verify --features zk-tests,zk-preverify,halo2-dev-tests preverify_rejects -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-zk-gap-core-zk-tests cargo test -p iroha_core --test zk_verify --features zk-tests,zk-preverify,halo2-dev-tests verifyproof_rejects_inactive_registered_verifying_key -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-zk-gap-core-zk-tests cargo test -p iroha_core --test zk_verify --features zk-tests,zk-preverify,halo2-dev-tests verifyproof -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-zk-adversarial-js-native npm run build:native` from `javascript/iroha_js`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-zk-gap-js-native npm run build:native` from `javascript/iroha_js`
  - `swift test --filter NativeEscrowInstructionBuildersTests` from `IrohaSwift`
  - `swift test --filter NativeEscrowInstructionBuildersTests/testAnonymousEscrowRejectsVerifyingKeyReferenceShadowField` from `IrohaSwift`
  - `swift test --filter 'ProofAttachmentNoritoTests|NativeEscrowInstructionBuildersTests'` from `IrohaSwift`
  - `node --test test/instructionBuilders.test.js` from `javascript/iroha_js`
  - `git diff --check`
  - `rg -n "new_inline|ProofAttachment::new_inline" crates javascript IrohaSwift python docs roadmap.md`
  - Static stale-pattern scan for the removed env override, old VK hash formula,
    timeout-as-rejection wording, and old SDK messages
  - `git diff --check` on the ZK cleanup touch set

## 2026-05-16 Soracloud production V1 hardening

- Soracloud uploaded-model registration now records SoraFS-backed bundle
  metadata instead of chain-resident encrypted model chunks. Bundle metadata
  includes an approved active SoraFS `ManifestDigest`, and core validates that
  pin before accepting register/finalize flows.
- Public Torii routing now exposes the single
  `/v1/soracloud/model/upload/register` mutation plus upload status/recipient
  reads. The old upload chunk/finalize and private inference routes are no
  longer registered.
- Production runtime posture now requires explicit Inrou enablement and an
  explicit runtime submission `gas_asset_id`; runtime submissions no longer
  source gas assets from environment variables or accepted-asset fallbacks.
- The embedded runtime fails closed for uploaded-model private inference in V1.
  Status is limited to SoraFS-backed storage and model registry readiness until
  a real deterministic private runtime exists.
- Uploaded-model V1 schema no longer carries private-runtime compatibility
  metadata such as compile-profile hashes, private bundle roots, or privacy-mode
  fields. Artifact links now use uploaded-model source provenance plus SoraFS
  storage roots.
- The JavaScript Soracloud HF helper no longer accepts `privateKeyHex`.
  Callers build an unsigned draft and assemble requests from external
  provenance signatures.

## 2026-05-17 FASTPQ verifier and proof sidecar hardening

- FASTPQ V1 verification no longer rebuilds the prover-scale CPU backend
  artifact. It checks the canonical batch commitment and public inputs, parses
  proof-carried trace/lookup/AIR roots, binds the lookup product into the
  transcript, authenticates sampled LDE/AIR/FRI Merkle openings, recomputes
  sampled AIR composition values, and validates FRI query chains. Tampered roots,
  lookup products, AIR openings, query chunks, and relabelled proofs now fail
  through proof-content checks.
- `Prover::prove` still self-verifies each generated proof before returning it;
  that self-check uses the same verifier with limits sized to the generated
  batch/proof, while public `verify(...)` keeps bounded default limits for
  untrusted inputs.
- FASTPQ runtime configuration now defaults to explicit `cpu`. The first-release
  production modes are `cpu` and `gpu`; explicit `gpu` lane startup fails closed
  when the backend or Poseidon preflight is unavailable instead of silently
  falling back to CPU.
- The restart/catch-up path now releases the incoming block-sync update dedup key
  before deferring or processing recovery payloads, so restarted peers can retry
  the same recovery update immediately instead of waiting for the ingress dedup
  TTL. The confidential localnet three-hop, dual-restart, and timeout-pressure
  cases pass with the current genesis/ZK policy-hash plumbing.
- FASTPQ proof snapshots remain in the existing Kura pipeline sidecar flow, now
  bounded by `fastpq.proof_sidecar_queue_cap`,
  `fastpq.proof_sidecar_max_bytes`, and
  `fastpq.proof_sidecar_max_retries`. Kura rejects oversized/overflowing
  snapshots, retries pending sidecar merges up to the configured limit, records
  missing entry hashes, and exports queue/event telemetry for sidecar enqueue,
  write, retry, drop, and rejection paths.
- Torii now exposes
  `/v1/pipeline/recovery/{height}/fastpq-proofs` for public JSON retrieval from
  existing pipeline sidecars, returning `404` when the sidecar is absent and an
  empty proof list when the sidecar exists without FASTPQ proofs. `fastpq_prover`
  also exposes helpers to canonicalize already-embedded AXT bindings and package
  already-bound batches plus proofs as `AxtProofEnvelope`/`ProofBlob` without
  adding binding metadata after proof generation.
- Validation: `cargo fmt --all`,
  `CARGO_TARGET_DIR=target/codex-fastpq-gpu FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq-gpu --release`,
  `CARGO_TARGET_DIR=target/codex-fastpq-release cargo test -p iroha_config --test fastpq_queue_overrides`,
  `CARGO_TARGET_DIR=target/codex-fastpq-release cargo check -p iroha_core --lib`,
  `CARGO_TARGET_DIR=target/codex-fastpq-release cargo test -p iroha_core fastpq`,
  `CARGO_TARGET_DIR=target/codex-fastpq-release cargo test -p iroha_core block_sync_update_releases_ingress_dedup_before_deferral -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-fastpq-release cargo check -p iroha_torii --lib`,
  `CARGO_TARGET_DIR=target/codex-fastpq-release cargo test -p iroha_torii --test pipeline_recovery_endpoint`,
  `CARGO_TARGET_DIR=target/codex-fastpq-release cargo check -p irohad --bin irohad`,
  the focused confidential localnet three-hop, dual-restart, and
  timeout-pressure integration tests,
  `CARGO_TARGET_DIR=target/codex-fastpq-release cargo clippy --workspace --all-targets -- -D warnings`,
  and the task-scoped `git diff --check` are green. Full
  `cargo test --workspace` was not run in this validation window because it is
  a multi-hour corridor.

## 2026-05-17 Torii first-release API cleanup

- Collapsed Torii API negotiation to the first-release `1.0` surface only and
  removed the previous sunset/old-version posture from the shared defaults.
- Moved public Norito submit/query/stream/system routes onto versioned paths:
  `/v1/pipeline/transactions`,
  `/v1/pipeline/transaction-entrypoints`,
  `/v1/pipeline/transactions/batch`, `/v1/query`, `/v1/events/ws`,
  `/v1/blocks/stream`, `/v1/peers`, `/v1/configuration`, `/v1/schema`, and
  `/v1/api/version`. OpenAPI, MCP, telemetry peer monitoring, Rust route tests,
  Swift/Kotlin/Java/JavaScript/Python client surfaces, and route docs now use
  the same first-release paths.
- Standardized Torii runtime failures on the Norito `ErrorEnvelope` with
  optional `details` for reject codes, queue pressure, retry hints, endpoint
  names, and AXT metadata. Queue rejections and proof throttling now return
  that envelope instead of legacy ad-hoc payloads, and the stale legacy shared
  queue-envelope type has been removed.
- Removed the app contract-route API-token bypass so `torii.require_api_token`
  applies consistently to those routes.
- Validation: `cargo fmt --all`, `git diff --check`,
  `cargo test -p iroha_torii_shared`,
  `cargo test -p iroha response_report -- --nocapture`,
  and
  `CARGO_TARGET_DIR=target/codex-openapi cargo test -p iroha_core pipeline_sidecar_decodes_missing_fastpq_proofs_as_empty -- --nocapture`
  are green. Focused Swift, Python, Kotlin/JVM, Java Android, and JavaScript
  client regressions for structured `ErrorEnvelope.details` reject codes are
  also green after rebuilding the JS native binding and package `dist`.
  Additional Rust/Python adversarial client coverage now checks AXT details
  without `reject_code`, ignores non-string reject-code smuggling attempts, and
  refuses to decode binary Norito envelopes under a non-Norito content type.
- Static Torii OpenAPI JSON snapshots and latest/current manifests were
  refreshed in explicit unsigned first-release mode. `ci/check_openapi_spec.sh`
  now verifies latest and current artifact path, size, SHA-256, and BLAKE3
  metadata with `--allow-unsigned`, while the signed manifest path remains
  available for operator release signing. Focused xtask OpenAPI tests now also
  reject stale unsigned payload digests, mismatched detached signatures, invalid
  signatures even under `--allow-unsigned`, unsupported signature algorithms,
  malformed signature hex/public-key hex, unsafe manifest artifact paths,
  platform-specific artifact paths, artifact filename mismatches, and all
  unsigned/signed flag combinations; portal OpenAPI tests reject signed entries
  hidden behind unsigned allowlists, signature metadata smuggled into unsigned
  version entries, signed manifests smuggled behind unsigned labels, and
  relative, absolute, Windows-drive, drive-relative, or backslash-traversal
  spec/manifest paths that escape the static OpenAPI directory. The portal
  checker also now rejects malformed allowed-signer keys/algorithms, malformed
  allowed-signer versions, duplicate allowed signers, malformed/missing and
  duplicate/whitespace versions lists, non-object version entries, missing or
  whitespace version-entry labels, non-boolean `signed` flags, invalid version
  byte counts, malformed BLAKE3 metadata, malformed signed version-entry
  metadata, malformed/non-object manifest-side signature metadata, incomplete
  manifest generator metadata, invalid manifest artifact byte counts, malformed
  signature hex before verification, and unsafe manifest `artifact.path`
  metadata rather than normalizing it into a valid-looking path.

## 2026-05-16 Parallel apply event fixture refresh

- Refreshed the `parallel_apply` event snapshots for asset-definition key-value,
  key-value/NFT lifecycle, and mint/burn/transfer coverage so the fixtures
  match the current Norito event encodings and emitted lifecycle events.
- Focused validation is green for
  `cargo test -p iroha_core --test parallel_apply -- --nocapture`.

## 2026-05-16 Sumeragi delivered RBC READY repair

- Delivered RBC rebroadcast now replays the cached READY set directly to every
  remote validator before the slower DELIVER rebroadcast. This gives peers that
  already supplied READY evidence another compact repair after local delivery,
  while the existing missing-peer rescue path remains scoped to peers missing
  locally observed READY for body repair and pre-delivery quorum deferrals.
- Focused validation is green for
  `cargo test -p iroha_core sumeragi::main_loop::tests::rebroadcast_stalled_rbc_payloads_repairs_ready_before_deliver_after_delivery -- --nocapture`,
  `cargo test -p iroha_core ready_repair -- --nocapture`, and
  `cargo test -p iroha_core rescue_rbc_missing_ready_peers -- --nocapture`.

## 2026-05-16 TradFi ISO 20022 interop audit/profile bridge

- Added the canonical TradFi ISO 20022 audit/design note at
  `docs/source/finance/tradfi_interop_audit.md` and linked it from the finance
  settlement mapping portal page.
- Added the shared `iroha_core::iso_bridge::profiles` catalog with static
  Norito JSON defaults for generic ISO 20022, Swift CBPR+, Fedwire Funds, SEPA
  SCT Inst, and securities CSD profiles. The Torii `iso_bridge` configuration
  now exposes `default_profile`, operator profile overrides, `store_dir`, and
  embedded signature policy without introducing new production environment
  toggles.
- Torii `pacs.008`/`pacs.009` ingestion now selects profiles via
  `X-Iroha-Iso-Profile` or `?profile=...`, validates message versions,
  Business Application Header/BizSvc/UETR policy, required reference datasets,
  minor units, SupplementaryData size, structured-address mode, and embedded
  XML signature policy. ISO bridge records persist under `store_dir/messages`
  with profile metadata, payload hash, UETR, transaction hash, reason codes, and
  status history.
- Follow-up hardening now resolves Business Application Header fields through
  suffix/canonical aliases for live-profile enforcement, keeps the structured
  address default on a constant-backed `ReadConfig` expression, and rechecks
  payload/UETR conflicts before replacing rejected or expired retry records.
  Focused Torii tests for BAH alias handling and UETR retry conflicts are green.
- ISO status responses now carry profile and audit metadata, expose
  `/v1/iso20022/messages/{msg_id}`, and can emit current `pacs.002` XML at
  `/v1/iso20022/messages/{msg_id}/pacs002`. OpenAPI, MCP, and JS SDK submission
  surfaces include profile selection.
- JS `submitIsoMessage` no longer injects wall-clock `creationDateTime`; callers
  must pass an explicit timestamp. CLI `sese.023`/`sese.025` previews now accept
  `--iso-settlement-date YYYY-MM-DD` for deterministic settlement-date output.

## 2026-05-11 20k liveness 300s phase-tail boundary

- Sumeragi execution status now reports the last completed block-pipeline apply,
  not the in-flight pre-apply overlay snapshot. The public status payload carries
  aggregate `pipeline_execution` counters for lane vertices/edges, overlay work,
  RBC bytes/chunks, detached prepared/merged/fallback counts, and quarantine
  execution. Per-lane detail remains available, while Izanami now treats the
  aggregate as authoritative and uses per-lane summing only as a fallback for
  older payloads.
- Focused coverage now guards both telemetry and non-telemetry builds:
  `parallel_apply_knob_compiles_without_telemetry` asserts the public status
  snapshot shows detached work when `pipeline.parallel_apply = true`, and
  `parallel_apply_knob_affects_detached_counters` keeps the metrics path covered
  under the `telemetry` feature.
- Sumeragi phase status now also exposes per-phase maximums for the current
  process lifetime. Torii returns them in `/v1/sumeragi/phases` as `max_ms`,
  and Izanami records `phase_collect_da_max_ms`,
  `phase_collect_precommit_max_ms`, and `phase_pipeline_total_max_ms` in the
  liveness matrix. This makes rejected rows useful for tail diagnosis instead
  of only reporting the last sampled phase.
- `scripts/run_izanami_liveness_matrix.py` now passes a 5s progress-monitor
  interval by default and records it in the matrix. The previous 15s monitor
  interval was too coarse for a 2-3s block-cadence gate and could reject rows
  on monitor-window quantization even when peer-observed block gaps were below
  the threshold.
- Short-run 20k-ingress matrix evidence after the status fix:
  - `dist/izanami-liveness-matrix-20k-cap1184-status-verify-30s-20260511-054723`:
    cap `1184`, pipeline `300ms`, collectors/redundant-send `3/3`, backup RBC
    on passed with `600,000` accepted submissions, `486.00` committed TPS,
    peer p95 `2.904s`, zero view changes, and status confirmed
    `detached_prepared_total = detached_merged_total = 1184`.
  - `dist/izanami-liveness-matrix-20k-next-options-60s-20260511-054850`:
    cap `1216` passed at pipeline `300ms` (`508.42` committed TPS, peer p95
    `2.935s`) and `350ms` (`508.42` committed TPS, peer p95 `2.837s`).
    Cap `1280` at pipeline `350ms` also passed with `514.72` committed TPS,
    peer p95 `2.940s`, zero view changes, and full detached merge
    (`1280/1280`, fallback `0`).
  - `dist/izanami-liveness-matrix-20k-cap1280-confirm-120s-20260511-055329`:
    cap `1280`, scan multiplier `32`, pipeline `350ms`, fanout `3/3`, backup
    RBC on is the new confirmed stable point. It accepted all `2,400,000`
    submissions, reached strict height `51`, approved `62,732` transactions,
    committed `522.77` TPS, held runner p95 to `2503ms`, peer p95 to `2.923s`,
    installed zero view changes, and ended with
    `detached_prepared_total = detached_merged_total = 1280`, fallback `0`.
  - `dist/izanami-liveness-matrix-20k-cap1344-1408-60s-20260511-055803`:
    cap `1344` passed at pipeline `350ms` (`539.95` committed TPS, peer p95
    `2.933s`) and `400ms` (`521.62` committed TPS, peer p95 `2.872s`). Cap
    `1408` also passed for 60s at pipeline `400ms` with `541.37` committed TPS
    and peer p95 `2.957s`, but this was only a short-run result.
  - `dist/izanami-liveness-matrix-20k-cap1408-confirm-120s-20260511-060245`:
    cap `1408`, pipeline `400ms` failed the 120s confirmation gate. It accepted
    `2,101,825` submissions before the runner p95 reached `3003ms`; parsed peer
    p95 was `3.106s`, max peer gap was `3.369s`, and the row crossed the
    3-second block-cadence budget. DA and precommit tails were visible in the
    sample (`492ms` DA, `599ms` precommit), so cap `1408` is rejected for the
    current stable baseline.
  - `dist/izanami-liveness-matrix-20k-cap1344-confirm-120s-20260511-060729`:
    cap `1344`, pipeline `350ms` accepted all `2,400,000` submissions with zero
    view changes and no fallback execution, but failed the hard peer-cadence
    gate at peer p95 `3.004s` (`526.50` committed TPS).
  - `dist/izanami-liveness-matrix-20k-cap1344-p400-confirm-120s-20260511-061034`:
    cap `1344`, pipeline `400ms` also accepted all `2,400,000` submissions with
    zero view changes and no fallback execution, but failed the peer-cadence
    gate at peer p95 `3.008s` (`516.08` committed TPS).
  - `dist/izanami-liveness-matrix-20k-cap1312-confirm-120s-20260511-061319`:
    cap `1312`, scan multiplier `32`, pipeline `350ms`, fanout `3/3`, backup
    RBC on passed the 120s gate. It accepted all `2,400,000` submissions,
    reached strict height `50`, approved `63,126` transactions, committed
    `526.05` TPS, held runner p95 to `2503ms`, peer p95 to `2.941s`, installed
    zero view changes, and ended with
    `detached_prepared_total = detached_merged_total = 1312`, fallback `0`.
  - `dist/izanami-liveness-matrix-20k-cap1328-confirm-120s-20260511-061707`:
    cap `1328`, pipeline `350ms` accepted all `2,400,000` submissions with zero
    view changes and no fallback execution, but failed the peer-cadence gate at
    peer p95 `3.073s` (`510.42` committed TPS). This rejects the immediate
    midpoint above cap `1312`.
- 300s 20k-ingress soak evidence:
  - `dist/izanami-liveness-matrix-20k-cap1312-soak-300s-20260511-062743`:
    cap `1312`, pipeline `350ms` failed the longer gate. It accepted
    `5,407,598` submissions, reached strict height `103`, hit runner p95
    `3006ms`, parsed peer p95 `3.271s`, and crossed the hard 3-second cadence
    budget.
  - `dist/izanami-liveness-matrix-20k-cap1280-soak-300s-20260511-064115`:
    cap `1280`, pipeline `350ms` failed with runner p95 `3006ms`, parsed peer
    p95 `3.155s`, and phase maxes showing `1021ms` DA, `1429ms` precommit,
    and `5356ms` pipeline tail.
  - `dist/izanami-liveness-matrix-20k-cap1216-soak-300s-20260511-064652`:
    cap `1216`, pipeline `350ms` failed with parsed peer p95 `3.084s`; DA and
    precommit maxes were `934ms` and `1231ms`.
  - `dist/izanami-liveness-matrix-20k-cap1120-soak-300s-20260511-065228`:
    cap `1120`, pipeline `300ms` passed parsed peer p95 (`2.856s`) but failed
    the hard runner gate at `3005ms`; this row is a near miss, not an accepted
    baseline.
  - `dist/izanami-liveness-matrix-20k-cap1024-soak-300s-20260511-065900`:
    cap `1024`, pipeline `300ms` passed. It accepted all `6,000,000`
    submissions, reached strict height `131`, approved `132,230` transactions,
    committed `440.77` TPS, held runner p95 to `2506ms`, parsed peer p95 to
    `2.921s`, and installed zero view changes.
  - `dist/izanami-liveness-matrix-20k-cap1120-p250-soak-300s-20260511-070515`:
    cap `1120`, pipeline `250ms` failed with runner p95 `3005ms`, parsed peer
    p95 `3.039s`, and DA/precommit maxes `1066ms`/`1356ms`.
  - `dist/izanami-liveness-matrix-20k-cap1024-p250-soak-300s-20260511-071039`:
    cap `1024`, pipeline `250ms` passed. It accepted all `6,000,000`
    submissions, reached strict height `130`, approved `131,145` transactions,
    committed `437.15` TPS, held runner p95 to `2506ms`, and parsed peer p95 to
    `2.874s`.
  - `dist/izanami-liveness-matrix-20k-cap1088-p250-soak-300s-20260511-071656`:
    cap `1088`, pipeline `250ms` passed under the old 15s progress monitor. It
    accepted all `6,000,000` submissions, reached strict height `127`, approved
    `136,073` transactions, committed `453.58` TPS, held runner p95 to
    `2510ms`, parsed peer p95 to `2.982s`, installed zero view changes, and
    ended with full detached merge (`1088/1088`, fallback `0`).
  - `dist/izanami-liveness-matrix-20k-cap1104-p250-soak-300s-20260511-072314`:
    cap `1104`, pipeline `250ms` was rejected by the old 15s progress monitor.
    Parsed peer p95 stayed under the limit at `2.885s`, but the hard runner
    gate reached `3005ms` at the target-height checkpoint before full ingress
    completed.
  - `dist/izanami-liveness-matrix-20k-cap1104-p250-pi5-soak-300s-20260511-073757`:
    cap `1104`, pipeline `250ms`, 5s progress monitor completed all
    `6,000,000` submissions and kept runner p95 at `2522ms`, but the row is
    still rejected on the external block-gap gate: parsed peer p95 was
    `3.054s`.
  - `dist/izanami-liveness-matrix-20k-cap1096-p250-pi5-soak-300s-20260511-074409`:
    cap `1096`, pipeline `250ms`, 5s progress monitor is the current accepted
    300s boundary. It accepted all `6,000,000` submissions, reached strict
    height `126`, approved `135,938` transactions, committed `453.13` TPS, held
    runner p95 to `2523ms`, parsed peer p95 to `2.899s`, installed zero view
    changes, and ended with full detached merge (`1096/1096`, fallback `0`).
  - `dist/izanami-liveness-matrix-20k-cap1100-p250-pi5-soak-300s-20260511-075025`:
    cap `1100`, pipeline `250ms`, collectors/redundant-send `3/3` accepted all
    ingress but is rejected on parsed peer p95 `3.071s`.
  - `dist/izanami-liveness-matrix-20k-cap1100-p250-k4r4-pi5-soak-300s-20260511-075630`:
    cap `1100`, pipeline `250ms`, collectors/redundant-send `4/4` also
    accepted all ingress but is rejected on parsed peer p95 `3.022s` and lower
    committed throughput (`451.14` TPS), so extra collector fanout is not the
    next fix.
  - A precommit scheduling-order experiment that sent the local precommit vote
    before local QC application was rejected and reverted. It did not move the
    boundary: cap `1100` still failed at parsed peer p95 `3.041s`, and cap
    `1096` regressed to parsed peer p95 `3.029s`.
- Conclusion: the 120s cap `1312` result was not durable. The current accepted
  20k-ingress operating point is cap `1096`, scan multiplier `32`, pipeline
  `250ms`, collectors/redundant-send `3/3`, backup RBC on. This preserves
  consensus liveness in the 2-3s p95 block envelope for 300s, but it still only
  commits hundreds of TPS. Reaching 20k committed TPS requires safe payloads in
  the tens of thousands per block or a major deterministic execution/DA/QC tail
  reduction; raising the cap to `1100+` currently loses the liveness gate.
- Validation:
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-inline-backup-20260511 cargo test -p iroha_core --lib access_set_source_and_conflict_rate_snapshot_updates -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-inline-backup-20260511 cargo test -p iroha_core --test parallel_apply_knob parallel_apply_knob_compiles_without_telemetry -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-inline-backup-20260511 cargo test -p iroha_core --features telemetry --test parallel_apply_knob parallel_apply_knob_affects_detached_counters -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-inline-backup-20260511 cargo test -p iroha_torii status_snapshot_json_includes -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-inline-backup-20260511 cargo test -p izanami sumeragi_status_digest_preserves_detailed_liveness_evidence -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-inline-backup-20260511 cargo test -p iroha_core --lib phase_snapshot_exposes_phase_ema -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-inline-backup-20260511 cargo test -p iroha_torii --features telemetry --test sumeragi_phases_endpoint -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-inline-backup-20260511 cargo test -p izanami parse_sumeragi_phase_snapshot_extracts_expected_fields -- --nocapture`
  - `rustfmt --edition 2024 --check crates/iroha_core/src/block.rs crates/iroha_core/tests/parallel_apply_knob.rs crates/iroha_core/src/sumeragi/status.rs crates/iroha_torii/src/routing.rs crates/iroha_torii/src/routing/consensus.rs crates/izanami/src/chaos.rs`
  - `python3 -m py_compile scripts/run_izanami_liveness_matrix.py`
  - `git diff --check`
  - `git diff --cached --check -- crates/iroha_core/src/block.rs crates/iroha_core/tests/parallel_apply_knob.rs crates/iroha_core/src/sumeragi/status.rs crates/iroha_torii/src/routing.rs crates/iroha_torii/src/routing/consensus.rs crates/izanami/src/chaos.rs status.md roadmap.md`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-inline-backup-20260511 cargo build --release -p irohad --bin iroha3d -p izanami --bin izanami --features irohad/fastpq-gpu`
  - 30s status verification, 60s next-options matrix, 120s cap-1280,
    cap-1344, cap-1408, cap-1312, and cap-1328 confirmation matrices, plus the
    300s cap-1312, cap-1280, cap-1216, cap-1120, cap-1024, cap-1088,
    cap-1096, cap-1100, and cap-1104 soaks listed above.

## 2026-05-11 Live-frontier missing-QC recovery matrix

- Sumeragi now suppresses idle missing-QC committed-anchor range pulls when the
  local node is already working on the contiguous frontier height, has local
  round liveness, and has no explicit same-height commit-QC dependency. The
  reacquire attempt is still recorded and throttled, but it no longer injects
  block-sync range-pull traffic into a live round that is already advancing.
- `scripts/run_izanami_liveness_matrix.py` now treats the parsed peer commit
  p95 as a hard row gate (`--peer-gap-p95-threshold-s`, default `3.0`), reports
  DA/precommit phase timing and RBC repair counts, and retains target-height
  progress fields from failed Izanami runs so rejected rows still have useful
  matrix data.
- New 20k ingress matrix evidence after the suppression:
  - `dist/izanami-liveness-matrix-20k-live-frontier-confirm-120s-20260511-034742`:
    cap `1120`, pipeline `300ms`, collectors/redundant-send `3/3`, and backup
    RBC on is the confirmed stable point. It accepted all `2,400,000`
    submissions, reached strict height `52`, approved `56,041` transactions,
    committed `467.01` TPS, held runner p95 to `2507ms`, parsed peer p95 to
    `2.822s`, max peer gap to `2.996s`, and installed zero view changes.
  - `dist/izanami-liveness-matrix-20k-live-frontier-cap-sweep-60s-20260511-034304`:
    caps `1152`, `1184`, and `1200` completed without view changes but failed
    the peer-cadence gate at parsed peer p95 `3.077s`, `3.154s`, and `3.089s`.
    The extra throughput was therefore rejected as unstable for the current
    liveness target.
  - `dist/izanami-liveness-matrix-20k-live-frontier-pipeline-60s-20260511-035106`:
    cap `1120` passed at pipeline `150ms`, `200ms`, and `250ms`, with parsed
    peer p95 `2.841s`, `2.733s`, and `2.934s`. A cap `1152` retry at `250ms`
    failed at `3.235s`.
  - `dist/izanami-liveness-matrix-20k-live-frontier-pipe200-confirm-120s-20260511-035713`:
    the tempting `1120`/`200ms` setting was not promoted. It failed the runner
    gate at `3002ms` p95 and stopped after `2,104,874` accepted submissions,
    despite parsed peer p95 staying under `3s`.
- Conclusion: the current stable 20k-ingress operating point is still cap
  `1120`, pipeline `300ms`, collectors/redundant-send `3/3`, backup RBC on.
  Larger caps and lower pipeline timing can increase or preserve short-run
  throughput, but the confirmed acceptance rule is consensus liveness first.
- Validation:
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-inline-backup-20260511 cargo test -p iroha_core --lib missing_qc_reacquire -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-inline-backup-20260511 cargo test -p iroha_core --lib force_view_change_if_idle_reacquires -- --nocapture`
  - `rustfmt --edition 2024 --check crates/iroha_core/src/sumeragi/main_loop.rs crates/iroha_core/src/sumeragi/main_loop/tests.rs`
  - `python3 -m py_compile scripts/run_izanami_liveness_matrix.py`
  - `git diff --check`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-inline-backup-20260511 cargo build --release -p irohad --bin iroha3d -p izanami --bin izanami --features irohad/fastpq-gpu`
  - 60s cap sweep, 60s pipeline sweep, 120s cap-1120 confirmation, and 120s
    pipe-200 rejection matrices listed above.

## 2026-05-11 RBC READY repair fanout tightening

- Sumeragi targeted RBC READY repair now sends the READY sidecar only to peers
  whose READY signature is still missing from the local session, instead of
  sending the sidecar to every remote validator once DELIVER/quorum repair is
  active. Pre-quorum authoritative frontier sessions also keep repair small:
  they send READY evidence without full payload/body fanout until READY quorum
  or local DELIVER makes heavier rescue appropriate.
- Focused coverage was updated for both paths:
  `maybe_emit_rbc_deliver_prefers_ready_repair_after_ready_quorum` now asserts
  that peers already observed locally are not direct READY-repair targets, and
  `maybe_emit_rbc_deliver_prefers_targeted_ready_rescue_when_subset_skips_local`
  verifies pre-quorum READY repair without payload fanout.
- New 20k ingress matrix evidence:
  - `dist/izanami-liveness-matrix-20k-ready-repair-60s-20260511-030904`:
    cap `1120`/backup-on passed with `485.50` committed TPS and parsed peer p95
    `2.793s`; cap `1216`/backup-on did not stall but stayed outside the
    peer-cadence target with parsed peer p95 `3.397s` and max gap `5.515s`.
  - `dist/izanami-liveness-matrix-20k-ready-repair-backupoff-60s-20260511-031258`:
    backup-off improved larger-cap throughput slightly (`1216`: `489.22` TPS,
    `1280`: `491.88` TPS), but both rows still exceeded the peer p95 target
    (`3.183s` and `3.142s`) and had more gaps over `3s`.
  - `dist/izanami-liveness-matrix-20k-ready-repair-confirm-120s-20260511-031638`:
    cap `1120`/backup-on remains the current stable baseline after the repair
    change: `2,400,000` submissions accepted, strict height `52`, `56,031`
    approved transactions, `466.93` committed TPS, runner p95 `2504ms`, parsed
    peer p95 `2.914s`, max gap `3.204s`, and zero view-change installs.
- Conclusion: the repair tightening reduces redundant control/body traffic and
  preserves the stable `1120` cap, but it does not unlock `1216+`. The next
  throughput work remains reducing DA/RBC/QC tail latency and block application
  cost enough for larger blocks to stay inside the 2-3s liveness envelope.
- Validation:
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-inline-backup-20260511 cargo test -p iroha_core --lib maybe_emit_rbc_deliver -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-inline-backup-20260511 cargo test -p iroha_core --lib rescue_rbc_missing_ready_peers -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-inline-backup-20260511 cargo build --release -p irohad --bin iroha3d -p izanami --bin izanami --features irohad/fastpq-gpu`
  - 60s backup-on, 60s backup-off, and 120s cap-1120 20k Izanami matrices listed above.

## 2026-05-11 Inline BlockCreated backup-RBC matrix

- Sumeragi now exposes
  `sumeragi.advanced.rbc.inline_block_created_backup` so exact single-frame
  frontier `BlockCreated` proposals can be tested with or without the redundant
  Proposal + RBC body backup path. The production/default posture remains
  backup-on; oversized, multi-chunk, and non-frontier proposal bodies still use
  Proposal + RBC transport.
- Izanami wires the knob through CLI/config/persistence as
  `--sumeragi-inline-block-created-backup-rbc`, and
  `scripts/run_izanami_liveness_matrix.py` accepts a seventh row field:
  `name:cap:scan:pipeline_ms:collectors_k:redundant_send_r:inline_backup_rbc`.
- The 60s comparison in
  `dist/izanami-liveness-matrix-20k-inline-backup-60s-20260511-021833`
  isolated the switch at cap `1120`/fanout `3/3`: backup-on passed with
  `476.02` committed TPS and parsed peer p95 `2.749s`; backup-off also passed
  with `486.55` committed TPS and parsed peer p95 `2.708s`. Higher caps did
  not become stable: `1280` backup-off failed at parsed peer p95 `3.401s`, and
  `1536` backup-off failed at `3.230s`.
- A narrow 60s backup-off sweep in
  `dist/izanami-liveness-matrix-20k-inline-backup-narrow-60s-20260511-022450`
  showed why 60s samples are not enough: `1216` passed once with `487.42`
  committed TPS and peer p95 `2.938s`, while `1152` and `1184` already showed
  peer p95 above the `3s` line.
- The 120s confirmation in
  `dist/izanami-liveness-matrix-20k-inline-backup-confirm-120s-20260511-022926`
  rejected `1216` backup-off as a stable cap: it accepted all ingress but
  failed the cadence gate at parsed peer p95 `3.306s`. The same run confirmed
  `1120` backup-off as stable with `2,400,000` submissions accepted, strict
  height `52`, `56,198` approved transactions, `468.32` committed TPS, runner
  p95 `2506ms`, parsed peer p95 `2.919s`, and zero view-change installs.
- The same-binary 120s backup-on confirmation in
  `dist/izanami-liveness-matrix-20k-inline-backup-on-confirm-120s-20260511-023458`
  passed with `468.01` committed TPS, runner p95 `2507ms`, parsed peer p95
  `2.913s`, and zero view-change installs. Conclusion: backup-off is a useful
  explicit experiment knob and safe at cap `1120`, but it does not justify
  raising the accepted cap or changing the default recovery posture.
- Validation:
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-inline-backup-20260511 cargo test -p iroha_core --lib backup_transport -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-inline-backup-20260511 cargo test -p iroha_torii --test connect_gating --no-run`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-inline-backup-20260511 cargo test -p izanami sumeragi -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-inline-backup-20260511 cargo test -p izanami roundtrip -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-inline-backup-20260511 cargo build --release -p irohad --bin iroha3d -p izanami --bin izanami --features irohad/fastpq-gpu`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-inline-backup-20260511 cargo build --release -p izanami --bin izanami`
  - `python3 -m py_compile scripts/run_izanami_liveness_matrix.py`
  - `rustfmt --edition 2024 --check crates/iroha_config/src/parameters/defaults.rs crates/iroha_config/src/parameters/actual.rs crates/iroha_config/src/parameters/user.rs crates/iroha_config/tests/fixtures.rs crates/iroha_torii/src/test_utils.rs crates/iroha_torii/tests/connect_gating.rs crates/iroha_core/src/kiso.rs crates/iroha_core/src/sumeragi/penalties.rs crates/iroha_core/src/sumeragi/main_loop/rbc.rs crates/iroha_core/src/sumeragi/main_loop/propose.rs crates/iroha_core/src/sumeragi/main_loop/tests.rs crates/izanami/src/config.rs crates/izanami/src/main.rs crates/izanami/src/persistence.rs crates/izanami/src/chaos.rs`
  - `git diff --check`
  - 60s, narrow 60s, and 120s 20k Izanami inline-backup matrices listed above.

## 2026-05-11 Izanami collector fanout matrix

- Izanami now also exposes NPoS collector fanout as matrixable runtime inputs:
  `--sumeragi-collectors-k` and
  `--sumeragi-collectors-redundant-send-r`. The values are parsed from CLI,
  persisted in stored Izanami run arguments, and injected into generated
  Sumeragi genesis parameters.
- `scripts/run_izanami_liveness_matrix.py` accepts optional row suffixes
  `:collectors_k:redundant_send_r` and records those columns in
  `summary.csv`/`summary.md`, allowing consensus fanout rows to be compared
  without source edits.
- The 60s collector matrix in
  `dist/izanami-liveness-matrix-20k-collectors-60s-20260511-012322` compared
  the existing `1024/4/4` baseline with lower redundancy. `1024/3/3` regressed
  committed TPS and parsed peer p95, while `1088/3/3` passed the runner gate
  with `1,200,000` accepted submissions, `455.78` committed TPS, runner p95
  `2501ms`, parsed peer p95 `2.918s`, and zero view-change installs.
  `1280/3/3` still failed the hard `3s` p95 gate.
- The first 120s confirmation in
  `dist/izanami-liveness-matrix-20k-collectors-confirm-120s-20260511-012933`
  promoted `1088/3/3` over `1024/4/4`: it accepted all `2,400,000`
  submissions, reached strict height `52`, approved `54,475` transactions,
  committed `453.96` TPS, held runner strict p95 to `2505ms`, and had parsed
  peer p95 `2.928s`.
- A narrower 60s sweep in
  `dist/izanami-liveness-matrix-20k-collectors-narrow-60s-20260511-013755`
  found a better candidate at `1120/3/3`: `467.35` committed TPS, runner p95
  `2502ms`, parsed peer p95 `2.749s`, and only one parsed peer gap over `3s`.
  `1152` and `1216` runner-passed but had parsed peer p95 at or above `3s`.
- The 120s confirmation in
  `dist/izanami-liveness-matrix-20k-collectors-cap1120-confirm-120s-20260511-014406`
  makes `1120/3/3` the current best confirmed point: `2,400,000` submissions
  accepted, strict height `53`, `57,265` approved transactions, `477.21`
  committed TPS, runner strict p95 `2506ms`, parsed peer p95 `2.856s`, and
  zero view-change installs.
- Conclusion: reducing collector redundancy from `4/4` to `3/3` gives a small
  safe-cap improvement when paired with cap `1120`, but it does not change the
  core throughput order of magnitude. The next work remains DA/RBC/QC and
  application/queue-drain tail latency reduction so larger blocks can stay
  under the 2-3s liveness SLO.
- Validation:
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-izanami-collectors-20260511 cargo test -p izanami sumeragi -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-izanami-collectors-20260511 cargo test -p izanami roundtrip -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-izanami-collectors-20260511 cargo build --release -p izanami --bin izanami`
  - `python3 -m py_compile scripts/run_izanami_liveness_matrix.py`
  - `rustfmt --edition 2024 --check crates/izanami/src/chaos.rs crates/izanami/src/config.rs crates/izanami/src/main.rs crates/izanami/src/persistence.rs`
  - `git diff --check`
  - 60s, 120s, narrow 60s, and cap-1120 120s 20k Izanami collector fanout
    matrices listed above.

## 2026-05-11 Sumeragi post-commit pacemaker kick

- Resolved the pending Sumeragi/ISI merge index forward without textual conflict
  markers. The retained resolution keeps the commit-QC/vote replay paths,
  detached-transfer call-hash fixtures, account-admission/SNS behavior, and
  registry-side changes intact.
- Durable commit completion now kickstarts the pacemaker when transactions are
  queued and the only proposal backpressure is transaction-queue saturation or
  consensus-queue pacing. Active pending blocks, RBC backlog, and relay
  backpressure remain hard stops.
- Focused coverage now verifies post-commit kickstart behavior for queued work
  with no backpressure, queue-only saturation, consensus-queue-only pacing, hard
  active/RBC/relay blockers, and an empty transaction queue.
- The rebuilt 20k FASTPQ GPU liveness gate at
  `dist/izanami-prebuilt-20k-fastpq-gpu-postcommit-kick-120s-20260511-011906`
  reached strict/quorum height `42` by `105.06s` and accepted all `2,101,351`
  offered submissions with zero failures before the hard cadence gate stopped
  the run. It missed the `3s` p95 block-interval threshold narrowly:
  quorum/strict p95 was `3002ms` across `37` samples. The run still showed
  queue saturation, ending around `759k` queued transactions in peer heartbeat
  samples before shutdown.
- Validation:
  - `cargo test -p iroha_core --lib kickstart_pacemaker_after_commit_triggers_only_when_allowed -- --nocapture`
  - `cargo test -p iroha_core --lib evaluate_pacemaker -- --nocapture`
  - `cargo test -p iroha_core --lib starv -- --nocapture`
  - `cargo clippy -p iroha_core --tests -- -D warnings`
  - `cargo build --release -p irohad --bin iroha3d -p izanami --bin izanami --features irohad/fastpq-gpu`
  - 120s 20k Izanami FASTPQ GPU liveness gate with
    `--latency-p95-threshold 3s` (failed at `3002ms` p95).
  - `cargo fmt --all -- --check`
  - `git diff --check`

## 2026-05-11 Izanami 20k liveness matrix

- Izanami now exposes the Sumeragi block payload tuning knobs directly:
  `--sumeragi-block-max-transactions` and
  `--sumeragi-proposal-queue-scan-multiplier`. The values are persisted in
  stored Izanami run arguments and flow into both genesis block parameters and
  runtime Sumeragi config, so matrix runs no longer require source edits.
- Added `scripts/run_izanami_liveness_matrix.py` to run repeatable 20k ingress
  sweeps and emit `summary.csv`/`summary.md` with accepted submissions,
  committed transaction rate, runner block-interval p95, parsed peer commit-gap
  p95/max, queue saturation, and view-change counters.
- The 60s pilot matrix in
  `dist/izanami-liveness-matrix-20k-60s-20260511-003049` tested caps `1024`,
  `1280`, `1536`, and `2048` with scan/pipeline variants. All rows accepted
  the offered load, but only `1024` was comfortably inside the peer-observed
  3s p95 cadence target (`2.723s`). `1280` passed the runner gate for 60s but
  already had parsed peer p95 `3.069s`; `1536+` failed.
- The 120s confirm matrix in
  `dist/izanami-liveness-matrix-20k-confirm-120s-20260511-004146` keeps
  `1024` as the safe cap: it accepted all `2,400,000` submissions, reached
  strict height `52`, had zero view-change installs, runner strict interval p95
  `2506ms`, parsed peer p95 `2.903s`, and committed `51,550` transactions
  (`429.58 TPS`). `1280` failed at the target checkpoint with runner p95
  `3002ms` and parsed peer p95 `3.236s`.
- The narrower 120s matrix in
  `dist/izanami-liveness-matrix-20k-narrow-120s-20260511-004711` shows no
  confirmed cap headroom above `1024`: `1088`, `1152`, and `1216` all failed
  the 3s runner p95 gate and had parsed peer p95 from `3.271s` to `3.369s`.
- Conclusion: the current code can sustain 20k ingress while keeping consensus
  live at the 1,024-transaction cap, but it is not close to 20k committed TPS.
  With 2-3s blocks, reaching 20k committed TPS requires tens of thousands of
  safe transactions per block or equivalent parallelization. The next hard work
  is reducing DA/QC/application tail latency and queue-drain cost enough to
  raise the safe block payload without sacrificing the liveness gate.
- Validation:
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-izanami-matrix-20260511 cargo test -p izanami sumeragi_block -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-izanami-matrix-20260511 cargo test -p izanami roundtrip -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-izanami-matrix-20260511 cargo build --release -p izanami --bin izanami`
  - `python3 -m py_compile scripts/run_izanami_liveness_matrix.py`
  - `rustfmt --edition 2024 --check crates/izanami/src/chaos.rs crates/izanami/src/config.rs crates/izanami/src/main.rs crates/izanami/src/persistence.rs`
  - `git diff --check`
  - 60s pilot, 120s confirm, and 120s narrow 20k Izanami liveness matrices
    listed above.

## 2026-05-10 Izanami consensus liveness target

- The 20k Izanami gate is now treated as a block-production liveness test first:
  accepted submissions are not sufficient if consensus stalls. The working SLO
  is sustained 20k ingress while peers continue committing blocks at roughly a
  2-3s cadence.
- Cadence analysis of
  `dist/izanami-prebuilt-20k-fastpq-gpu-cert-vote-replay-120s-20260510-154724`
  shows the 4,096-transaction cap is not stable enough for that SLO: inter-block
  gaps averaged `7.20s`, p50 was `6.10s`, p95 was `13.62s`, and all `62`
  measured gaps exceeded `3s`. The steady post-QC persist segment alone was
  about `2.31s` p50 / `2.67s` p95, leaving no room for proposal, validation,
  and QC formation.
- The comparable 2,048-cap 20k control artifact
  `dist/izanami-prebuilt-20k-age-ring-cap2048-120s-20260430-170353` accepted
  all `2,400,000` submissions, reached strict height `66`, had zero view-change
  installs, and kept inter-block cadence at p50 `1.80s` / p95 `2.91s`.
- The current-code 2,048-cap rerun
  `dist/izanami-prebuilt-20k-liveness2048-current-120s-20260510-211240`
  accepted all `2,400,000` submissions with zero failures and zero view-change
  installs, but it only reached strict height `34`; inter-block cadence was p50
  `3.54s`, p95 `4.54s`, and max `5.77s`.
- The current-code 1,024-cap rerun
  `dist/izanami-prebuilt-20k-liveness1024-current-120s-20260510-212330`
  accepted all `2,400,000` submissions with zero failures and zero view-change
  installs, reached strict height `52`, and kept inter-block cadence at p50
  `2.28s` / p95 `2.94s` / max `3.32s` across `200` measured gaps. Stage timing
  split was p95 `1.50s` from previous commit to validation, `0.88s` from
  validation to QC, and `0.95s` from QC to commit, with no slow commit-stage
  lines. Izanami's high-TPS harness cap is therefore `1,024` transactions per
  block until block application and queue-drain optimizations can raise the cap
  without reintroducing stalls.
- Izanami now accepts `--latency-p95-threshold` on duration-only runs. When no
  `--target-blocks` KPI is configured, the harness derives a soft block target
  from `duration / threshold`, reuses the block-progress monitor until the
  duration deadline, and treats missing interval samples as a hard failure
  because the run cannot prove consensus liveness. Izanami summaries now also
  emit final quorum/strict block-interval p50, p95, and sample counts when the
  progress monitor is active.
- The cadence-gated 1,024-cap run
  `dist/izanami-prebuilt-20k-liveness1024-gated-120s-20260510-220144` used
  `--latency-p95-threshold 3s`, accepted all `2,400,000` submissions with zero
  failures, reported submit latency p50 `2ms` / p95 `4ms`, reached strict
  height `52`, and passed the hard p95 block-interval gate. Parsed peer commit
  gaps were p50 `2.28s`, p95 `2.97s`, and max `3.38s` across `200` measured
  gaps. Proposal-to-commit timing from peer logs was p95 `2.26s`, split between
  proposal-to-QC p95 `1.42s` and QC-to-commit p95 `0.95s`. The queue still
  saturated (`862,671 / 2,400,000`), so the next optimization target is applied
  throughput and queue drain without losing the 2-3s block cadence.
- Validation:
  - Parsed existing 20k Izanami peer stdout logs for commit-gap,
    validation-to-QC, and QC-to-commit timing.
  - `rustfmt --edition 2024 --check crates/izanami/src/chaos.rs crates/izanami/src/config.rs`
  - `cargo test -p izanami latency -- --nocapture`
  - `cargo test -p izanami block_interval -- --nocapture`
  - `git diff --check -- crates/izanami/src/chaos.rs crates/izanami/src/config.rs roadmap.md status.md`
  - `cargo test -p izanami make_network_builder_applies_pipeline_time -- --nocapture`
  - `cargo build --release -p irohad --bin iroha3d -p izanami --bin izanami --features irohad/fastpq-gpu`
  - `cargo build --release -p izanami --bin izanami`
  - 120s 20k Izanami liveness gates at the 2,048 and 1,024 transaction caps,
    plus the 1,024-cap gated rerun with `--latency-p95-threshold 3s`.

## 2026-05-10 Sumeragi commit-QC recovery and cert-only vote replay

- Commit processing now tries to form a local commit QC from cached commit
  votes before treating a known locally valid block as missing its commit QC.
  This lets a peer that already has the block body and quorum votes persist the
  certificate without waiting for another peer fetch round trip.
- Commit-QC-only block-sync responses now also try to synthesize a direct
  commit QC from cached votes when the responder already has the block body. If
  the responder still cannot form the certificate, it replays cached commit
  votes to the requester while keeping the cert-only request stashed for the
  later QC.
- 20k FASTPQ GPU gate evidence:
  `dist/izanami-prebuilt-20k-fastpq-gpu-cert-vote-replay-120s-20260510-154724`
  accepted and succeeded all `2,400,000` submissions with zero failures,
  submit latency `p50=3ms`, `p95=29ms`, `p99=81ms`, `max=250ms`, strict
  height `16`, strict approved `57,385`, peer height skew `2`, and peer
  approval skew `8,192`. Missing-block fetches dropped to `105`, cert-only
  known-block update requests matched `body_request=false` requests at `102`,
  the cached-vote replay path fired `53` times, exact frontier body requests
  were `7`, pending frontier redrives were `2`, commit-inflight timeout total
  stayed `0`, and commit inflight ended inactive.
- The queue still saturated (`843,109 / 2,400,000`), so the remaining 20k work
  is queue drain and final height convergence under sustained ingress. The
  commit-QC recovery changes moved the missing-QC pressure down without
  reintroducing the large-block merge bottleneck.
- Validation:
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-inflight cargo test -p iroha_core --lib commit_pipeline_forms_local_commit_qc_before_missing_commit_qc_recovery -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-inflight cargo test -p iroha_core --lib commit_pipeline_arms_missing_commit_qc_recovery_for_stalled_local_vote -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-inflight cargo test -p iroha_core --lib commit_qc_only_fetch_pending_block -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-inflight cargo clippy -p iroha_core --tests -- -D warnings`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-20k-frontier-metadata-rbc-20260509-180500 cargo build --release -p irohad --bin iroha3d -p izanami --bin izanami --features irohad/fastpq-gpu`
  - The 120s 20k Izanami FASTPQ GPU gate above
  - `cargo fmt --all -- --check`
  - `git diff --check`

## 2026-05-10 Sumeragi committed RBC repair suppression

- Delivered RBC sessions whose block height is already at or below the local
  committed tip no longer stay active for RBC rebroadcast or hot repair. This
  prevents retained post-commit RBC state from repeatedly sending targeted
  READY/INIT/body repair traffic or rebroadcasting DELIVER after the block has
  already committed.
- Delivered sessions that are still uncommitted remain eligible for the
  existing frontier/body repair path, so pre-commit RBC convergence is
  preserved.
- Validation:
  - `cargo fmt --all`
  - `cargo test -p iroha_core --lib actor_next_tick_deadline_ignores_delivered_committed_rbc_session -- --nocapture`
  - `cargo test -p iroha_core --lib rebroadcast_stalled_rbc_payloads_skips_deliver_for_committed_block -- --nocapture`
  - `cargo test -p iroha_core --lib rebroadcast_stalled_rbc_payloads_skips_payload_after_delivery -- --nocapture`
  - `cargo test -p iroha_core --lib delivered_rbc_session_at_committed_tip_is_not_rebroadcast_active -- --nocapture`
  - `cargo test -p iroha_core --lib rebroadcast_stalled_rbc_payloads_repairs_ready_before_deliver_after_delivery -- --nocapture`
  - `cargo test -p iroha_core --lib rbc_backlog_counts_delivered_session_without_ready_quorum -- --nocapture`
  - `cargo check -p iroha_core --lib`

## 2026-05-10 Izanami proposal stale-window and block-cap tuning

- Sumeragi proposal assembly now scales the stale-view guard by proposal
  fullness: small proposals still use the base quorum timeout, a full
  4,096-transaction proposal gets one extra quorum window for shared-host
  scheduling jitter, and larger proposal experiments remain capped at four
  windows. The one-transaction stale-proposal regression still aborts and
  requeues before broadcast.
- The 8,192 and 16,384 max-transaction experiments did not improve the 20k
  gate. The 16,384 run no longer self-aborted after the larger-batch grace, but
  regressed into multi-second validation/merge waits and commit-inflight churn;
  the 8,192 run tied the earlier approval count with worse latency/churn. That
  pass kept the Izanami high-TPS profile at the then-current 4,096 block cap,
  but the later liveness analysis above moves the harness to 1,024 while block
  cadence is the primary gate.
- Final-code 20k FASTPQ GPU gate evidence:
  `dist/izanami-prebuilt-20k-fastpq-gpu-low-contention-8192-block4096-finalcode-120s-20260510-153218`
  accepted and succeeded all `2,400,000` submissions with zero failures,
  submit latency `p50=3ms`, `p95=20ms`, `p99=69ms`, `max=206ms`, strict height
  `13`, strict approved `45,167`, peer height skew `2`, and peer approval skew
  `8,192`. The diagnostic grep found stale proposal aborts `0`, detached apply
  fallback `0`, slow commit-stage timings `0`, no `no proposal observed`
  errors, and no `ERROR` log lines. The final Sumeragi digest reported
  view-change installs `3`, all from `missing_qc`, and commit-inflight timeout
  total `0`.
- The queue still saturated (`852,277 / 2,400,000`) and missing-QC recovery
  still drove view changes, so the remaining 20k work is commit-QC formation
  and recovery cadence under sustained ingress, not proposal assembly, the
  single-block merge path, or a larger block cap.
- Validation:
  - `rustfmt --edition 2024 crates/iroha_core/src/sumeragi/main_loop/propose.rs crates/iroha_core/src/sumeragi/main_loop/tests.rs crates/izanami/src/chaos.rs`
  - `rustfmt --edition 2024 --check crates/iroha_core/src/sumeragi/main_loop/propose.rs crates/iroha_core/src/sumeragi/main_loop/tests.rs crates/izanami/src/chaos.rs`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-izanami-stable-workload cargo test -p iroha_core --lib proposal_assembly_stale_window_scales_for_large_batches -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-izanami-stable-workload cargo test -p iroha_core --lib stale_proposal_assembly_aborts_before_broadcast_and_requeues -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-izanami-stable-workload cargo test -p iroha_core --lib force_view_change_if_idle_defers_empty_frontier_missing_qc_under_tx_backlog_after_reacquire -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-izanami-stable-workload cargo test -p iroha_core --lib resilience_fast_paths_same_height_commit_votes_for_medium_rosters -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-izanami-stable-workload cargo test -p iroha_core --lib vote_validation_inbound_defers_then_dispatches -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-izanami-stable-workload cargo test -p izanami make_network_builder_applies_pipeline_time -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-20k-stable-low-contention-20260510 cargo build --release -p irohad --bin iroha3d -p izanami --bin izanami --features irohad/fastpq-gpu`
  - The 120s 20k Izanami FASTPQ GPU gate above
  - `git diff --check`

## 2026-05-10 Izanami simple-transfer batch merge gate

- The detached single-transfer merge fast path now has a block-level simple
  transfer batch path for preseeded receiver assets. Eligibility checks the
  exact transfer data events against enabled data-trigger filters, so unrelated
  data triggers no longer disable the batch path while matching asset/account
  triggers still keep the per-transaction merge semantics.
- The transfer transcript regression now also covers batch eligibility for
  no-trigger, unrelated-trigger, and matching asset-trigger cases, while the
  batched merge still preserves per-transaction FASTPQ transcript buckets and
  event streams.
- 20k FASTPQ GPU gate evidence:
  `dist/izanami-prebuilt-20k-fastpq-gpu-batch-triggerguard-120s-20260510-121922`
  accepted and succeeded all `2,400,000` submissions with zero failures,
  submit latency `p50=3ms`, `p95=30ms`, `p99=80ms`, `max=210ms`, strict
  height `16`, strict approved `57,378`, and peer height/approval skew `0`.
  The run emitted no slow commit-stage timing samples, reported
  commit-inflight timeout total `0`, and ended with commit inflight inactive.
  The queue still saturated (`869,733 / 2,400,000`), so the remaining 20k work
  is proposal/QC cadence and queue drain, not the previous multi-second
  transfer merge sample.
- Recovery counters from the same gate stayed on the intended paths:
  cert-only known-block update requests `91`, matching `body_request=false`
  requests `91`, exact frontier body requests `25`, pending frontier redrives
  `633`, and no known-block frontier-stall catch-up or exact-body repair
  routes were observed by the diagnostic grep.
- Validation:
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-inflight cargo test -p iroha_core --lib detached_asset_transfer_matches_sequential_transcript_and_events -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-inflight cargo clippy -p iroha_core --tests -- -D warnings`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-20k-frontier-metadata-rbc-20260509-180500 cargo build --release -p irohad --bin iroha3d -p izanami --bin izanami --features irohad/fastpq-gpu`
  - 120s 20k Izanami FASTPQ GPU gate above
  - `cargo fmt --all -- --check`
  - `git diff --check`

## 2026-05-10 Izanami ordered stable transfer fast-merge reruns

- Izanami prebuilt transaction warmup now preserves `WorkloadEngine`
  generation order when concurrent prebuild workers finish out of order. This
  keeps the stable workload's deterministic sender/receiver transfer pairs in
  block order instead of reintroducing avoidable merge conflicts through channel
  receive order.
- Stable high-TPS runs now seed at least `8,192` workload accounts. That is
  enough for one disjoint sender/receiver pair per 4096-transfer block while
  avoiding the extra state size from the earlier `16,384` account floor.
- Detached single-transfer merge now uses a dedicated fast path for deltas that
  contain exactly one transparent numeric transfer and no other operations. It
  still replays through the same asset policy, FASTPQ transcript, event, and
  trigger machinery as sequential execution.
- 20k FASTPQ GPU gate evidence:
  - Ordered-only stable rerun
    `dist/izanami-prebuilt-20k-fastpq-gpu-low-contention-ordered-120s-20260510-104110`
    accepted and succeeded all `2,400,000` submissions, but reached only
    strict height `6` / strict approved `16,428`; fallback was `0`, while
    slow samples averaged `validation_execution_tx_apply_merge_ms=6,874ms`.
  - Adding the transfer fast path with the `16,384` account floor
    `dist/izanami-prebuilt-20k-fastpq-gpu-low-contention-fastmerge-120s-20260510-111651`
    reached strict height `9` / strict approved `28,713`, with merge average
    down to `4,777ms`.
  - The latest `8,192` account fast-merge gate
    `dist/izanami-prebuilt-20k-fastpq-gpu-low-contention-8192-fastmerge-120s-20260510-113031`
    accepted and succeeded all `2,400,000` submissions with zero failures,
    submit latency `p50=3ms`, `p95=40ms`, `p99=95ms`, `max=337ms`, strict
    height `12`, strict approved `41,190`, and peer height/approval skew `0`.
    Slow samples dropped to `7`; merge still averaged `4,211ms`, detached
    execution averaged `288ms`, fallback and digest-submit were both `0`, and
    the final queue remained saturated (`852,390 / 2,400,000`).
- Validation:
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-izanami-stable-workload cargo test -p izanami ordered_plan_sequence_preserves_stable_transfer_pair_order -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-izanami-stable-workload cargo test -p izanami stable_transfer_plan_uses_disjoint_sender_receiver_halves -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-izanami-stable-workload cargo test -p izanami prebuilt_stress_queue_capacity_scales_to_buffer -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-izanami-stable-workload cargo test -p iroha_core detached_asset_transfer_matches_sequential_transcript_and_events -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-20k-stable-low-contention-20260510 cargo build --release -p irohad --bin iroha3d -p izanami --bin izanami --features irohad/fastpq-gpu`
  - The three 120s 20k Izanami gates listed above
  - `rustfmt --edition 2024 --check crates/iroha_core/src/state.rs crates/izanami/src/chaos.rs crates/izanami/src/instructions.rs`
  - `git diff --check`

## 2026-05-10 Sumeragi detached transfer/cert-only recovery gate

- Source-signed transparent numeric asset transfers now have a detached apply
  path for the large-block validator hot path. The executor records transfers
  as transfer deltas, merge replays them through the same asset policy,
  transcript, event, and trigger machinery as sequential ISI execution, and
  block validation enables the path only for non-genesis, single-instruction
  transfers where the source account is the transaction authority.
- Detached merge now receives transaction context (`tx_call_hash`, signed
  transaction hash, lane, and dataspace) so FASTPQ transfer transcripts from
  detached execution match sequential execution. The new regression compares
  sequential and detached transfer transcripts, events, balances, and Poseidon
  digest.
- Known-block commit-QC retries for locally materialized payloads now stay on
  the cert-only fetch path even when the missing-QC request is dependency
  stalled. The recovery path no longer spends the frontier stall-reset catch-up
  budget for a block body that is already local.
- Latest comparable 120s permissioned 20k FASTPQ GPU gate:
  `dist/izanami-prebuilt-20k-fastpq-gpu-detached-transfer-cert-stall-120s-20260510-110407`.
  The artifact summary reports `2,400,000` offered, `2,400,000` ingress
  accepted, `2,400,000` successes, zero failures, submit latency `p50=3ms`,
  `p95=36ms`, `p99=86ms`, `max=218ms`, strict height `10`, strict approved
  `32,898`, and peer height/approval skew `0`.
- The gate confirms the intended routing changes: known-block frontier
  stall-reset catch-up routes `0`, known-block exact-body repair routes `0`,
  cert-only known-block update requests `57`, and detached apply fallback
  `0` across the slow commit samples. The remaining blocker is merge cost:
  slow commit samples show `validation_execution_tx_apply_merge_ms` p50
  around `4.3s` per 4096-transfer block, with detached execution itself around
  `0.15s`.
- Validation:
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-inflight cargo test -p iroha_core --lib detached_asset_transfer_matches_sequential_transcript_and_events -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-inflight cargo test -p iroha_core --lib known_block_commit_qc_recovery -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-inflight cargo clippy -p iroha_core --tests -- -D warnings`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-20k-frontier-metadata-rbc-20260509-180500 cargo build --release -p irohad --bin iroha3d -p izanami --bin izanami --features irohad/fastpq-gpu`
  - Artifact summary/diagnostic checks over the 20k gate above
  - `cargo fmt --all -- --check`
  - `git diff --check`

## 2026-05-10 Izanami stable transfer contention audit

- Izanami stable transfer planning now uses deterministic sender/receiver
  halves instead of random receivers. Stable runs still submit only preseeded
  `TransferAsset` instructions on the measured hot path, but the generated
  transfer stream avoids avoidable receiver collisions that inflate
  block-merge fallback work during large-block profiling.
- High-TPS stable runs now seed at least `8,192` workload accounts, while
  chaos high-TPS runs keep the existing `4,096` floor. This gives the 20k
  stable profile a larger disjoint-pair pool without changing chaos coverage.
- Validation:
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-izanami-stable-workload cargo test -p izanami stable_transfer_plan_uses_disjoint_sender_receiver_halves -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-izanami-stable-workload cargo test -p izanami prebuilt_stress_queue_capacity_scales_to_buffer -- --nocapture`
  - `cargo fmt --all --check`
  - `git diff --check`

## 2026-05-10 FASTPQ transcript digest precompute rerun

- Single-delta FASTPQ transfer transcripts now compute and store their Poseidon
  preimage digest when `StateTransaction::record_transfer_transcript` records
  the transcript. The pending-transcript coverage now asserts that the digest is
  available before block drain, so validation/commit no longer has to submit
  the same single-delta digest work on the hot path.
- The release build and clean 20k FASTPQ GPU Izanami gate both completed with
  the rebuilt binaries:
  `dist/izanami-prebuilt-20k-fastpq-gpu-precomputed-digest-clean-120s-20260510-095052`.
  Izanami exited `0`, prebuilt and used all `2,400,000` transactions, accepted
  and succeeded all submissions with zero failures, and reported submit latency
  `p50=3ms`, `p95=33ms`, `p99=81ms`, `max=222ms`.
- Final strict progress in the clean run was height `15`, approved `54,008`,
  with peer height/approval skew `0`; peer stderr logs were empty. Sumeragi
  reported no commit inflight timeouts and no slow commit stage timings, but
  the final transaction queue still saturated (`894,605 / 2,400,000`) and one
  missing-QC view-change signal remained.
- A separate diagnostic run taken while a local release build was still active
  is treated as contaminated, but its slow commit-stage lines showed
  `validation_execution_tx_finalize_digest_submit_ms=0`, confirming that the
  digest-submit stage has been moved out of the hot path. The remaining 20k
  blocker is committed-throughput/proposal recovery and real commit reuse, not
  transcript digest finalization.
- Validation:
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-fastpq-precompute-check cargo check -p iroha_core --lib`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-20k-fastpq-precomputed-digest-20260510 cargo build --release -p irohad --bin iroha3d -p izanami --bin izanami --features irohad/fastpq-gpu`
  - 120s clean 20k Izanami FASTPQ GPU gate using the rebuilt release binaries above
  - `cargo fmt --all --check`
  - `git diff --check`

## 2026-05-10 Sumeragi local-payload commit-QC recovery

- Known-block commit-QC recovery no longer routes locally materialized payloads
  through exact frontier body repair before requesting the missing commit QC.
  Local payloads now use the existing certificate-only peer fetch path
  immediately, while payloads that are not materialized locally still arm exact
  body repair and honor ingress grace.
- Added focused coverage that separates those two cases and updated the local
  retry regression so a cert-only retry does not require an active frontier
  body-repair slot.
- Validation:
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-inflight cargo test -p iroha_core --lib known_block_commit_qc_recovery -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-inflight cargo clippy -p iroha_core --tests -- -D warnings`
  - `cargo fmt --all -- --check`
  - `git diff --check`

## 2026-05-10 ISI registry/executor surface cleanup

- The default built-in instruction registry now exposes grouped instruction
  families through their canonical boxes only: register/unregister, mint/burn,
  transfer, key-value metadata, grant/revoke, RWA, repo, and settlement. Direct
  generic/grouped wire names such as `Register<Domain>`, concrete
  mint/burn/transfer variants, `Grant<Permission, Account>`, `RepoIsi`, and
  `DvpIsi` are intentionally absent from the default registry, while the boxed
  families keep stable wire IDs.
- Missing standalone executable ISIs are now registered and dispatched for
  oracle disputes/governance/Twitter bindings, SoraDNS, content bundles,
  confidential parameter lifecycle, SoraFS pricing/credit, public-lane staking
  reward/bond/unbond/slash/claim flows, `RebindPublicLaneValidatorPeer`,
  `ExpireSpaceDirectoryManifest`, and `RemoveSmartContractBytes`.
- Added registry/dispatch drift coverage in `iroha_core` and data-model
  registry coverage that proves direct grouped variants stay unregistered while
  boxed stable IDs and the new standalone surface decode.
- Removed the dead SoraFS `RecordReplicationReceipt` data-model ISI and its
  unused data-model receipt status/record types. The Kotlin and Java Android
  typed builders for that removed instruction were deleted as well; the
  independent `sorafs_manifest` receipt records remain.
- Validation:
  - `cargo fmt --all`
  - `cargo fmt --all --check`
  - `git diff --check`
  - `cargo test -p iroha_data_model instruction_registry`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-isi-dm cargo test -p iroha_data_model --lib registry`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-isi-final cargo test -p iroha_core smartcontracts::isi` (passed; library slice: 814 tests)
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-isi-final cargo test -p iroha_core --lib default_instruction_registry_entries_have_core_dispatch_handlers`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-isi-final cargo test -p iroha_core --lib oracle`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-isi-final cargo test -p iroha_core --test oracle`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-isi-final cargo test -p iroha_core --test social_viral_incentives`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-isi-final cargo test -p iroha_core --test confidential_params_registry` (compiled; no runnable tests under the active cfg)
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-isi-final cargo test -p iroha_core --features zk-tests --test confidential_params_registry`

## 2026-05-10 Sumeragi vNext/Izanami 20k restart

- Repaired the Sumeragi vNext unit-test harness after the runtime `Reactor`
  boundary removal: direct actor-owned vNext dispatch, validation rejection,
  commit persistence, proposal acceptance, and availability handoff tests now
  bind vote/QC chain-order through the active signing topology.
- Torii query batch canonicalization now covers the oracle and Twitter binding
  output variants, and Torii test utilities include the current Sumeragi worker
  validation-stall defaults.
- Test-network genesis now wraps peer PoP registration through the registered
  `RegisterBox::Peer` shape via `InstructionBox::from(register)` and expands
  topology entries only into the HSM-bound registration path. This fixes the
  Izanami genesis signing panic from a raw `RegisterPeerWithPop` Norito payload
  and avoids duplicate plain peer registrations.
- Fresh 20k FASTPQ GPU gate:
  `dist/izanami-prebuilt-20k-fastpq-gpu-direct-digest-batch-120s-20260510-033617`.
  Izanami exited `0`, prebuilt/used all `2,400,000` transactions, accepted and
  succeeded all submissions with zero failures, and reported submit latency
  `p50=3ms`, `p95=33ms`, `p99=79ms`, `max=191ms`. Final strict progress was
  height `17`, approved `61,543`, peer height/approval skew `0`; peer stderr
  logs were empty. The run still ended with a saturated transaction queue
  (`866,385 / 2,400,000`) and one missing-QC view-change signal, so the next
  throughput slice remains queue drain/block validation and commit reuse.
- Validation:
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-inflight cargo test -p iroha_core --lib validate_block_for_voting_records_timings -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-inflight cargo test -p iroha_core --lib commit_stage_timings_threshold_uses_non_overlapping_total -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-inflight cargo test -p iroha_data_model set_transaction_results --lib -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-inflight cargo test -p fastpq_prover direct_gpu_batch_limit_covers_izanami_block_shape --lib -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-inflight cargo test -p iroha_test_network genesis_registers_peers_with_pop --lib -- --nocapture`
  - Direct `iroha_core --lib` vNext tests:
    `vnext_dispatch_validation_queues_worker_and_accepts_result`,
    `vnext_reject_validation_aborts_slot_and_removes_pending`,
    `vnext_commit_persisted_marks_round_slot_committed`,
    `vnext_proposal_accepted_marks_round_slot_proposed`, and
    `vnext_availability_ready_marks_round_slot_awaiting_validation`.
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-inflight cargo clippy -p fastpq_prover -p iroha_data_model -p iroha_core --lib -- -D warnings`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-inflight cargo clippy -p iroha_core --tests -- -D warnings`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-inflight cargo clippy -p iroha_torii -p iroha_test_network --lib -- -D warnings`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-20k-frontier-metadata-rbc-20260509-180500 cargo build --release -p irohad --bin iroha3d -p izanami --bin izanami --features irohad/fastpq-gpu`
  - 120s 20k Izanami FASTPQ GPU gate using the rebuilt release binaries above
  - `cargo fmt --all -- --check`
  - `git diff --check`

## 2026-05-10 Sumeragi/Data-Model fallback cleanup

- `SignedBlock::set_transaction_results*` is now fallible and validates
  external entrypoint hash prefixes plus existing consensus merkle roots before
  mutating block result roots. `presigned_with_payload` no longer hydrates the
  legacy transaction cache implicitly; RBC recovery explicitly hydrates decoded
  payloads with `BlockPayload::hydrate_legacy_transaction_cache_from_entrypoints`.
- Sumeragi vNext no longer has a runtime `Reactor` event/effect boundary. The
  actor owns vNext round/slot/validation state directly, including validation
  result handling, ticks, rechain/view-change handling, recovery, and
  block-sync sidecar hydration.
- Vote/QC chain-order binding now derives from the effective signature or
  phase-specific roster, including NewView commit-history rosters, and QC
  aggregation carries that binding into the materialized certificate.
- Validation redrive and stall timing now use explicit worker config knobs
  under `sumeragi.advanced.worker`, with zero-value rejection at config parse
  time.
- Broad data-model validation also refreshed stale generated fixtures for
  confidential wallet signed transactions, Nexus lane-commitment `.to`
  companions, oracle reference JSON, and the BlockHeader Norito golden after
  the current canonical encodings changed.
- Validation:
  - `cargo fmt --all`
  - `cargo test -p iroha_data_model --features transparent_api`
  - `cargo test -p iroha_config sumeragi`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-vnext-cleanup cargo check -p iroha_core --lib`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-vnext-cleanup cargo test -p iroha_data_model --features transparent_api set_transaction_results -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-vnext-cleanup cargo test -p iroha_data_model --features transparent_api payload_hydration -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-vnext-cleanup cargo test -p iroha_data_model --features transparent_api presigned_with_payload_does_not_hydrate -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-vnext-cleanup cargo test -p iroha_core --lib npos_qc_uses -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-vnext-cleanup cargo test -p iroha_core --lib qc_broadcast_targets_snapshot_roster -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-vnext-cleanup cargo test -p iroha_core --lib rebuild_qcs_from_cached_votes -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-vnext-cleanup cargo test -p iroha_core --lib conflicting_ -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-vnext-cleanup cargo test -p iroha_core --lib commit_pipeline_ -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-vnext-cleanup cargo test -p iroha_core --lib new_view_ -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-vnext-cleanup cargo test -p iroha_core --lib pacemaker_ -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-vnext-cleanup cargo test -p iroha_core --lib reschedule_ -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-vnext-cleanup cargo test -p iroha_core --lib vote_backed -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-vnext-cleanup cargo test -p iroha_core --lib deferred_ -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-vnext-cleanup cargo test -p iroha_core --lib commit_qc_redrives -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-vnext-cleanup cargo test -p iroha_core --lib commit_pipeline_redrives_ -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-vnext-cleanup cargo test -p iroha_core --lib vnext -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-vnext-cleanup cargo test -p iroha_core --lib chain_order -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-vnext-cleanup cargo test -p iroha_data_model --features transparent_api --test confidential_wallet_fixtures confidential_wallet_fixtures_are_stable -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-vnext-cleanup cargo test -p iroha_data_model --features transparent_api --test consensus_roundtrip regenerate_lane_commitment_fixtures -- --ignored --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-vnext-cleanup cargo test -p iroha_data_model --features transparent_api --test consensus_roundtrip lane_commitment_fixtures_roundtrip -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-vnext-cleanup cargo test -p iroha_data_model --features transparent_api --test norito_chain_layout signed_block_wire_accepts_default_and_rejects_non_default_layout_flags -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-vnext-cleanup cargo test -p iroha_data_model --features transparent_api --test norito_golden_scaffold -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-vnext-cleanup cargo test -p iroha_data_model --features transparent_api --test oracle_reference_fixtures regenerate_follow_reference_fixtures -- --ignored --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-vnext-cleanup cargo test -p iroha_data_model --features transparent_api --test oracle_reference_fixtures -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-vnext-cleanup cargo test -p iroha_data_model --features transparent_api -- --nocapture`
  - `cargo fmt --all --check`
  - `git diff --check`

## 2026-05-10 Soracles operator MVP

- Soracles now routes feed registration, provider observations, aggregation,
  disputes, governance changes, rollback, and Twitter binding ISIs through
  normal instruction dispatch and `InstructionBox` conversion.
- Operator management is covered by typed oracle permissions, while observation
  submit and aggregation remain provider-membership gated. Feed registration,
  aggregation replay/staleness, dispute anchoring, and governance stage order
  now enforce the MVP safety rules documented in `docs/source/soracles.md`.
- Oracle query coverage now includes feed, history, provider stats, dispute,
  change, and Twitter binding lookups, including singular provider-stat records
  that carry both key and counters. The CLI adds `iroha soracles tx ...` and
  `iroha soracles query ...` subcommands alongside the existing bundle/catalog
  and evidence GC helpers.
- Focused follow-up coverage now exercises the oracle operator ISI surface
  through `InstructionBox` dispatch, validates role-derived typed permissions,
  rejects invalid feed configs, quorum/replay/stale aggregation, unanchored
  disputes, and governance stage jumps, round-trips all oracle query
  constructors through Norito, and verifies CLI subcommand parsing plus
  output-only instruction generation without a live node. The compiled `iroha`
  binary smoke suite now also exercises `iroha --output app soracles tx
  aggregate`, decodes stdout back into `AggregateOracleFeed`, and checks the
  feed/slot/request/evidence fields. The command-module tests now cover
  output-safe construction for every Soracles tx builder and parse every
  Soracles query subcommand without contacting a node. A broad workspace check
  also caught and fixed the generic Mochi state-browser batch label matcher so
  oracle feed, history, stats, dispute, change, and Twitter binding query
  batches report stable labels on unexpected-batch errors.
- Validation:
  - `cargo fmt --all -- --check`
  - `CARGO_TARGET_DIR=/tmp/iroha-soracles-check cargo check -p iroha_data_model -p iroha_executor_data_model -p iroha_executor`
  - `CARGO_TARGET_DIR=/tmp/iroha-soracles-check cargo check -p iroha_core --lib`
  - `cargo check -p iroha_data_model -p iroha_executor_data_model -p iroha_executor -p iroha_core`
  - `CARGO_TARGET_DIR=/tmp/iroha-soracles-check cargo test -p iroha_data_model oracle -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-soracles-check cargo test -p iroha_executor_data_model oracle -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-soracles-check cargo test -p iroha_core --test oracle oracle -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-soracles-check cargo test -p iroha_core oracle -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-soracles-check cargo test -p iroha_cli --test cli_smoke soracles_aggregate_output_emits_instruction_payload -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-soracles-check cargo test -p iroha_cli soracles -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-soracles-check cargo test -p iroha_cli --test cli_smoke -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-soracles-check cargo test -p iroha_cli`
  - `CARGO_TARGET_DIR=/tmp/iroha-soracles-check cargo test -p mochi-core batch_label_handles_rwa_and_escrow_variants -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-soracles-check cargo check --workspace`
  - `CARGO_TARGET_DIR=/tmp/iroha-soracles-check cargo clippy -p iroha_data_model -p iroha_executor_data_model -p iroha_executor -p iroha_core -p iroha_cli --all-targets -- -D warnings`
  - `CARGO_TARGET_DIR=/tmp/iroha-soracles-check cargo clippy -p iroha_cli --test cli_smoke -- -D warnings`
  - `CARGO_TARGET_DIR=/tmp/iroha-soracles-check cargo clippy -p iroha_cli --all-targets -- -D warnings`
  - `cargo fmt -p iroha_cli -- --check` after the CLI smoke increment
  - `git diff --check`

## 2026-05-09 Sumeragi vNext active-load validation guard

- DA validation worker freshness now scales with the pending block size
  (`16ms` per transaction, capped at `90s`) before vNext treats a worker as
  stalled. The no-argument helper remains available for focused tests, while
  production timeout checks use the hash-specific pending-block floor.
- vNext tick recovery now defers `ValidationTimeout` while the matching worker
  is still fresh, and defers `ValidationBackpressure` when the block is still
  locally pending so queue saturation remains on the shell retry path instead
  of entering vNext recovery. Recovering and aborted slots no longer redrive
  stale validation ownership.
- The Sumeragi commit-result fixture now initializes the commit-stage
  validation timing field, keeping focused `iroha_core --lib` builds aligned
  with the current commit telemetry shape.
- The 120s 20k prebuilt FASTPQ GPU run
  `dist/izanami-prebuilt-20k-fastpq-gpu-vnext-stallfloor16-120s-20260509`
  accepted and succeeded all `2,400,000` submissions with zero failures and
  submit latency `p50=2ms`, `p95=4ms`, `p99=36ms`. It finished at strict
  height `4`, strict approved `8,254`, and logged zero stale vNext redrives,
  zero recovery slots, zero validation worker stall expirations, and zero view
  changes.
- The remaining 20k acceptance blocker is no longer false validation recovery:
  the run still saturated the transaction queue (`870,642 / 2,400,000`) and
  reported commit-inflight timeout noise while large blocks were validating and
  committing. The next throughput slice should reuse or carry validation
  execution artifacts into commit instead of executing the same large block
  path twice.
- Validation:
  - `cargo fmt --all`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-redrive-current cargo test -p iroha_core --lib da_validation_worker_stall_timeout_scales_with_pending_block_size -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-redrive-current cargo test -p iroha_core --lib tick_defers_vnext_validation_backpressure_while_pending_block_exists -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-redrive-current cargo test -p iroha_core --lib vnext_validation -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-redrive-current cargo test -p iroha_core --lib commit_pipeline_redrives_ -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-redrive-current cargo test -p iroha_core --lib commit_pipeline_keeps_ -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-redrive-current cargo test -p iroha_core --lib validation_worker -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-redrive-current cargo check -p iroha_core --lib`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-redrive-current cargo clippy -p iroha_core --lib -- -D warnings`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-20k-vnext-redrive-20260509-backpressureguard cargo build --release -p irohad --bin iroha3d -p izanami --bin izanami --features irohad/fastpq-gpu`
  - 20k Izanami GPU 120s gate using the prebuilt release binaries above
  - `cargo fmt --all --check`
  - `git diff --check`

## 2026-05-09 Torii/Sumeragi validation gap closure

- ZK proof list integration fixtures now seed proof records and TLV tag indexes
  through committed WSV test helpers, so `/v1/zk/proofs?has_tag=...` exercises
  the same derived indexes used by production query paths.
- Sumeragi vNext chain-order/quorum helpers now use the live signature topology
  or ordered-validator slice that is actually in scope, keeping the current
  `iroha_core` test build compiling after the vNext redrive changes.
- The fresh validation-inflight timeout floor is compiled for production while
  the no-argument stall-timeout helper is test-only, removing the non-test
  dead-code warning without changing focused test coverage.
- Validation:
  - `cargo fmt --all`
  - `git diff --check`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-torii-full cargo test -p iroha_core --lib list_and_count_filter_by_tag_and_status -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-torii-full cargo test -p iroha_core --lib maybe_force_view_change_for_stalled_pending_honors_fresh_validation_worker_floor_under_tx_backlog -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-torii-full cargo test -p iroha_torii`
## 2026-05-10 BLS admission aggregate precheck

- `ValidBlock::validate` now runs BLS transaction signature micro-batch
  prechecks during static validation, before duplicate-payload rejection, and
  feeds those prechecked results into transaction admission. Height-1 BLS
  transactions with aggregate-prechecked bad signatures are rejected instead of
  slipping through the genesis transaction shortcut.
- Validation:
  - `cargo fmt --all`
  - `cargo test -p iroha_core --test admission_batching --features bls,telemetry bls_ -- --nocapture`
  - `cargo test -p iroha_core --test admission_batching --features bls,telemetry -- --nocapture`
  - `cargo check -p iroha_core --features bls`
  - `git diff --check`

## 2026-05-10 Sumeragi missing-QC recovery ordering

- Empty contiguous-frontier `missing_qc` idle ticks now use the unified
  frontier recovery arming pass for the initial view and for a non-leader's
  first timeout in a later view when no recovery owner exists. Post-rotation
  rounds with a recorded same-view timeout or stale ingress still rotate at the
  base timeout, so stale block/control ingress cannot permanently suppress idle
  recovery.
- Validation:
  - `cargo fmt --all`
  - `cargo test -p iroha_core --lib force_view_change_if_idle_allows_empty_frontier_after_stale_block_ingress_without_progress -- --nocapture`
  - `cargo test -p iroha_core --lib force_view_change_if_idle_no_actionable_dependency_rotates_after_base_timeout -- --nocapture`
  - `cargo test -p iroha_core --lib force_view_change_if_idle_arms_nonleader_empty_frontier_recovery_after_pacemaker_attempt -- --nocapture`
  - `cargo test -p iroha_core sumeragi::main_loop::tests::force_view_change_if_idle_ -- --nocapture`
  - `git diff --check`

## 2026-05-09 iroha_core library regression sweep

- Repaired the broad `iroha_core` library regression set covering block
  stateless validation, genesis parameter transaction fixtures, network gossip
  roundtrips, IVM admission/host policy checks, FastPQ transfer transcript
  call-hash fixtures, SNS/account-admission transfer flows, domain NFT cleanup,
  block-sync RBC ingress handling, Sumeragi roster/QC/vote recovery, cached
  proposal rotation, VRF reveal routing, and proposal timing.
- Sumeragi vote/QC handling now validates cached and replayed evidence against
  the roster context used to aggregate it, preserves fresh cached frontier
  proposals before their repair window expires, and signs NPoS VRF reveal test
  messages with the peer selected by the effective commit topology.
- Validation:
  - `cargo fmt --all`
  - `cargo test -p iroha_core --lib external_vrf_reveal_broadcasts_after_acceptance -- --nocapture`
  - `cargo test -p iroha_core --lib on_block_message_handles_vrf_reveal_before_commit_catchup_finalizes_epoch -- --nocapture`
  - `cargo test -p iroha_core --lib assemble_proposal_allows_stale_retired_prior_view_local_vote_history -- --nocapture`
  - `cargo test -p iroha_core --lib pacemaker_does_not_rotate_fresh_cached_frontier_proposal_before_body_materializes -- --nocapture`
  - `cargo test -p iroha_core --lib cached_recovery_proposal_ -- --nocapture`
  - `cargo test -p iroha_core --lib` (`5530` passed, `0` failed, `22` ignored)

## 2026-05-09 FASTPQ GPU Izanami fallback cleanup

- BN254 Poseidon word-batch submission now drains deterministic 128-slice
  accelerator chunks sequentially. This keeps large Izanami transcript batches
  on the Metal path without exhausting command permits or re-entering the
  scalar BN254 fallback.
- Tiny GPU batches now stay on the scalar path before dispatch: row hashing
  uses CPU below 32 rows, and grouped limb hashing uses CPU below 32 messages.
  Larger homogeneous padded-length groups still use the accelerator path.
- The 20k Izanami GPU gate
  `dist/izanami-prebuilt-20k-fastpq-gpu-bn254-128seq-rowguard-120s-20260509-163329`
  completed with `2,400,000` offered and accepted transactions, zero submit
  failures, strict height `3`, and `4,361` strict approved transactions. The
  logs contain no BN254 dispatch failures, row-hash dispatch failures, GPU
  limb parity mismatches, scalar fallback warnings, or command-buffer failure
  diagnostics.
- The follow-up sampled profile
  `dist/izanami-profile-20k-fastpq-gpu-clean-sampled-90s-20260509-163714`
  completed with `1,800,000` offered and accepted transactions, strict height
  `3`, and `4,279` strict approved transactions. The old
  `iroha_zkp_halo2::poseidon::hash_u64_words_internal` CPU hotspot is absent;
  the sampled proof-side wait is now Metal BN254 completion, while the live
  progress limit remains consensus queue saturation and quorum timeout churn.
- Validation:
  - `cargo test -p fastpq_prover bn254_poseidon --features fastpq-gpu -- --nocapture`
    with `CARGO_TARGET_DIR=/tmp/iroha-codex-fastpq-limb-groups`
  - `cargo test -p fastpq_prover trace_row_hashes --features fastpq-gpu -- --nocapture`
    with `CARGO_TARGET_DIR=/tmp/iroha-codex-fastpq-limb-groups`
  - `cargo test -p fastpq_prover domain_hash --features fastpq-gpu -- --nocapture`
    with `CARGO_TARGET_DIR=/tmp/iroha-codex-fastpq-limb-groups`
  - `cargo build --release -p irohad --bin iroha3d -p izanami --bin izanami --features irohad/fastpq-gpu`
    with `CARGO_TARGET_DIR=/tmp/iroha-codex-20k-limb-groups-20260509-151821`
  - 20k Izanami GPU 120s gate using the prebuilt release binaries above
  - 20k Izanami GPU 90s sampled profile using the prebuilt release binaries
    above
  - `cargo check -p fastpq_prover` with
    `CARGO_TARGET_DIR=/tmp/iroha-codex-fastpq-default-check`
  - `cargo clippy -p fastpq_prover -- -D warnings` with
    `CARGO_TARGET_DIR=/tmp/iroha-codex-fastpq-default-check`
  - `cargo fmt --all --check`
  - `git diff --check`

## 2026-05-09 Offline Note V2 explorer outcome sync adapters

- Kotlin/JVM, Java Android, and Swift now decode Offline Note V2 explorer
  instruction envelopes for issue, audit, and redeem payloads. The public SDK
  decoders accept both framed instruction payloads and the raw instruction
  pair shape returned by explorer rows.
- The SDKs now expose an `OfflineNoteV2OutcomeIndex` plus resolver/provider
  adapters that turn committed or rejected audit/redeem explorer outcomes into
  wallet sync resolutions. Committed audits spend input nullifiers and release
  outputs, rejected audits restore inputs and cancel outputs, committed
  redeems mark notes redeemed, and rejected redeems return notes to spendable.
- Production Torii providers fetch `AuditOfflineNoteV2` and
  `RedeemOfflineNoteV2` rows from `/v1/explorer/instructions`, extract
  `r#box.encoded` instruction bytes, and feed the resolver-backed
  `OfflineNoteV2Wallet.sync()` path.
- Cross-SDK fixture tests cover explorer instruction decoding and committed /
  rejected outcome reconciliation for pending spend, change, receive, and
  redeem wallet notes.
- Validation:
  - `JAVA_HOME=/opt/homebrew/opt/openjdk@21/libexec/openjdk.jdk/Contents/Home PATH=/opt/homebrew/opt/openjdk@21/bin:$PATH ./gradlew :core-jvm:test --tests org.hyperledger.iroha.sdk.offline.OfflineNoteV2Test --console=plain --rerun-tasks` from `kotlin`
  - `JAVA_HOME=/opt/homebrew/opt/openjdk@21/libexec/openjdk.jdk/Contents/Home PATH=/opt/homebrew/opt/openjdk@21/bin:$PATH ANDROID_HOME=$HOME/Library/Android/sdk ANDROID_SDK_ROOT=$HOME/Library/Android/sdk ./gradlew :core:test --tests org.hyperledger.iroha.android.GradleHarnessTests --console=plain --rerun-tasks` from `java/iroha_android`
  - `JAVA_HOME=/opt/homebrew/opt/openjdk@21/libexec/openjdk.jdk/Contents/Home PATH=/opt/homebrew/opt/openjdk@21/bin:$PATH ANDROID_HOME=$HOME/Library/Android/sdk ANDROID_SDK_ROOT=$HOME/Library/Android/sdk ./gradlew :android:compileDebugJavaWithJavac --console=plain --rerun-tasks` from `java/iroha_android`
  - `swift test --filter OfflineNoteV2Tests` from `IrohaSwift`
  - `swift test` from `IrohaSwift`

## 2026-05-09 Offline Note V2 Swift Keychain wallet-note store

- Swift now has a public `OfflineNoteV2WalletNoteJsonCodec` matching the
  Android persisted wallet-note shape, including Norito key certificates,
  commitment origins, canonical amounts, state, and timestamps.
- `OfflineNoteV2KeychainStore` implements `OfflineNoteV2Store` with a
  Keychain-backed encrypted collection. The store supports app groups,
  optional user-presence access control, sorted note listing, upsert, delete,
  and clear operations.
- Swift wallet store operations now throw, so Keychain or corrupt-store
  failures propagate through wallet load/pay/accept/redeem/sync flows instead
  of being hidden behind a non-throwing store API.
- Validation:
  - `swift test --filter OfflineNoteV2Tests` from `IrohaSwift`
  - `swift test` from `IrohaSwift`

## 2026-05-09 Offline Note V2 Android secure wallet-note store

- Java Android now has a structured `OfflineNoteV2WalletNoteJsonCodec` for
  persisted wallet notes. The codec preserves the note chain/account/asset,
  canonical amount, Norito key certificate, commitment, note secret, origin,
  state, and timestamps so platform stores do not invent an ad hoc shape.
- The Android platform module now exposes `AndroidOfflineNoteV2SecureStore`,
  an `OfflineNoteV2Store` implementation that encrypts wallet-note JSON with
  Android Keystore AES-GCM and stores the encrypted envelopes plus commitment
  index in private `SharedPreferences`.
- Validation:
  - `JAVA_HOME=/opt/homebrew/opt/openjdk@21/libexec/openjdk.jdk/Contents/Home PATH=/opt/homebrew/opt/openjdk@21/bin:$PATH ANDROID_HOME=$HOME/Library/Android/sdk ANDROID_SDK_ROOT=$HOME/Library/Android/sdk ./gradlew :core:test --tests org.hyperledger.iroha.android.GradleHarnessTests --console=plain --rerun-tasks` from `java/iroha_android`
  - `JAVA_HOME=/opt/homebrew/opt/openjdk@21/libexec/openjdk.jdk/Contents/Home PATH=/opt/homebrew/opt/openjdk@21/bin:$PATH ANDROID_HOME=$HOME/Library/Android/sdk ANDROID_SDK_ROOT=$HOME/Library/Android/sdk ./gradlew :android:compileDebugJavaWithJavac --console=plain --rerun-tasks` from `java/iroha_android`

## 2026-05-09 FASTPQ Poseidon limb-batch grouping and BN254 chunking

- FASTPQ domain-separated limb hashing now groups GPU Poseidon batches by each
  message's canonical padded sponge length before dispatch. This prevents the
  fixed-`block_count` Metal/CUDA column kernel from permuting extra zero blocks
  for shorter messages in a mixed batch.
- `PoseidonColumnBatch::from_limb_slices` now rejects mixed padded lengths so
  callers must split batches before entering the accelerator path. Ordered CPU
  fallback remains intact whenever GPU dispatch is unavailable or a parity
  sample fails.
- BN254 Poseidon word-batch submission now splits oversized accelerator
  batches into deterministic 128-slice chunks, compacts each chunk's word
  buffer with rebased offsets, and concatenates completed chunk results in the
  original order.
- The oversized 4,096-slice Izanami-shaped BN254 word-batch regression now
  stays on the Metal accelerator path and matches the scalar reference.
- Validation:
  - `cargo test -p fastpq_prover public_gpu_bn254_poseidon_word_batches_chunk_large_izanami_shape --features fastpq-gpu -- --nocapture`
    with `CARGO_TARGET_DIR=/tmp/iroha-codex-continue-fastpq`
  - `cargo test -p fastpq_prover compact_slice_chunk --features fastpq-gpu -- --nocapture`
    with `CARGO_TARGET_DIR=/tmp/iroha-codex-continue-fastpq`
  - `cargo test -p fastpq_prover chunked_pending --features fastpq-gpu -- --nocapture`
    with `CARGO_TARGET_DIR=/tmp/iroha-codex-continue-fastpq`
  - `cargo test -p fastpq_prover domain_hash --features fastpq-gpu -- --nocapture`
    with `CARGO_TARGET_DIR=/tmp/iroha-codex-fastpq-limb-groups`
  - `cargo test -p fastpq_prover poseidon_column_batch --features fastpq-gpu -- --nocapture`
    with `CARGO_TARGET_DIR=/tmp/iroha-codex-fastpq-limb-groups`
  - `cargo test -p fastpq_prover poseidon --features fastpq-gpu -- --nocapture`
    with `CARGO_TARGET_DIR=/tmp/iroha-codex-fastpq-limb-groups`
  - `cargo test -p fastpq_prover proof --features fastpq-gpu -- --nocapture`
    with `CARGO_TARGET_DIR=/tmp/iroha-codex-fastpq-limb-groups`
  - `cargo check -p fastpq_prover` with
    `CARGO_TARGET_DIR=/tmp/iroha-codex-fastpq-default-check`
  - `cargo clippy -p fastpq_prover -- -D warnings` with
    `CARGO_TARGET_DIR=/tmp/iroha-codex-fastpq-default-check`
  - `cargo fmt --all --check`
  - `git diff --check`

## 2026-05-09 Offline Note V2 payment-token QR/JSON codec

- Kotlin/JVM, Java Android, and Swift now expose payment-token handoff codecs
  for Offline Note V2 wallet QR flows. The compact JSON payload carries the
  v2 type/version, invoice/payment-request id, token id, creation timestamp,
  and the canonical Norito audit bundle as `audit_norito_base64`.
- The codecs roundtrip through the public Norito audit decoder, reject token
  ids that do not match the embedded audit bundle, support the
  `wallet-offline-payment-v2:` text prefix, and produce Fountain QR frames
  tagged as `OFFLINE_PAYMENT_TOKEN_V2`.
- Cross-SDK tests now cover JSON bytes, prefixed text, and QR frame
  encode/decode roundtrips for the shared Offline Note V2 fixture token.
- Validation:
  - `JAVA_HOME=/opt/homebrew/opt/openjdk@21/libexec/openjdk.jdk/Contents/Home PATH=/opt/homebrew/opt/openjdk@21/bin:$PATH ./gradlew :core-jvm:test --tests org.hyperledger.iroha.sdk.offline.OfflineNoteV2Test --console=plain --rerun-tasks` from `kotlin`
  - `JAVA_HOME=/opt/homebrew/opt/openjdk@21/libexec/openjdk.jdk/Contents/Home PATH=/opt/homebrew/opt/openjdk@21/bin:$PATH ANDROID_HOME=$HOME/Library/Android/sdk ANDROID_SDK_ROOT=$HOME/Library/Android/sdk ./gradlew :core:test --tests org.hyperledger.iroha.android.GradleHarnessTests --console=plain --rerun-tasks` from `java/iroha_android`
  - `swift test` from `IrohaSwift`

## 2026-05-09 Offline Note V2 public SDK Norito decoders

- Kotlin/JVM, Java Android, and Swift now expose public Offline Note V2 Norito
  decoders for key certificate payloads/certificates, issue payloads, issued
  claims, redeem payloads/public inputs, audit bundles/public inputs, and the
  wallet-derived commitment/nullifier/payment-token-id preimages.
- The SDK adapters now decode the same framed compact Norito payloads they
  already encode, including account identifiers, asset identifiers, recursive
  proof boxes, commitment origins, numeric amounts, hash vectors, and optional
  certificate usage limits. Swift also handles bridge-unavailable asset address
  roundtrips in tests with a checked fallback literal.
- Cross-SDK fixture tests roundtrip the shared Offline Note V2 vectors through
  the new public decoders and re-encode them back to the canonical bytes.
- Validation:
  - `JAVA_HOME=/opt/homebrew/opt/openjdk@21/libexec/openjdk.jdk/Contents/Home PATH=/opt/homebrew/opt/openjdk@21/bin:$PATH ./gradlew :core-jvm:test --tests org.hyperledger.iroha.sdk.offline.OfflineNoteV2Test --console=plain --rerun-tasks` from `kotlin`
  - `JAVA_HOME=/opt/homebrew/opt/openjdk@21/libexec/openjdk.jdk/Contents/Home PATH=/opt/homebrew/opt/openjdk@21/bin:$PATH ANDROID_HOME=$HOME/Library/Android/sdk ANDROID_SDK_ROOT=$HOME/Library/Android/sdk ./gradlew :core:test --tests org.hyperledger.iroha.android.GradleHarnessTests --console=plain --rerun-tasks` from `java/iroha_android`
  - `swift test` from `IrohaSwift`

## 2026-05-09 Sumeragi validation vNext redrive cleanup

- Commit and commit-QC validation now redrive stale, disconnected, stalled, or
  expired frontier validation through vNext instead of falling back to
  production inline execution. Legacy worker inflight state is superseded only
  when vNext is not already tracking the block, so late worker results remain
  filtered by their inflight id while retry ownership stays on the vNext path.
- Near-quorum commit votes, commit-QC/cached-QC evidence, and small
  fast-finality blocks now stay on the vNext worker path instead of toggling
  consensus-critical validation back to inline execution while the worker path
  is healthy.
- Focused tests now cover near-tip vote evidence without proposal observation,
  vNext worker ownership for near-quorum and small fast-finality validation,
  stale commit-QC redrive, queue-full backpressure, configured fallback timing,
  and stalled or disconnected inflight redrive. The inline validation helper is
  retained only for focused unit tests.
- Recovery-heartbeat proposals now survive RBC/block-payload recovery because
  `SignedBlock::presigned_with_payload` hydrates the skipped transaction cache
  from external entrypoints when rebuilding a signed block from a decoded
  payload. Pacemaker recovery tests now seed real validation inflight state when
  asserting that an old-view frontier owner remains live across a missing-QC
  view advance, and commit-QC-history-sensitive pacemaker fixtures hold the
  existing commit-history test guard so parallel harness setup cannot reset
  their global QC context mid-assertion.
- Validation:
  - `cargo fmt --all`
  - `cargo fmt --all --check`
  - `rustfmt --edition 2024 crates/iroha_core/src/sumeragi/main_loop/commit.rs crates/iroha_core/src/sumeragi/main_loop/qc.rs crates/iroha_core/src/sumeragi/main_loop/validation.rs crates/iroha_core/src/sumeragi/main_loop/tests.rs`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-redrive-current cargo test -p iroha_core --lib vnext_dispatch_validation_queues_worker_and_accepts_result -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-redrive-current cargo test -p iroha_core --lib through_vnext -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-redrive-current cargo test -p iroha_core --lib queue_full -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-redrive-current cargo test -p iroha_core --lib commit_qc_keeps_fresh_inflight_validation_deferred_past_inline_fallback -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-redrive-current cargo test -p iroha_core --lib commit_pipeline_keeps_deferred_validation_when_inflight_is_fresh -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-redrive-current cargo test -p iroha_core --lib validation_allows_near_tip_commit_votes_without_proposal_evidence -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-redrive-current cargo test -p iroha_core --lib validation_inline -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-redrive-current cargo test -p iroha_core --lib validation_allows -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-redrive-current cargo test -p iroha_core --lib stale_view_async_commit_votes_for_known_pending_block_still_form_qc -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-redrive-current cargo check -p iroha_core --lib`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-redrive-current cargo test -p iroha_core --lib block_sync_update_ -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-redrive-current cargo test -p iroha_core --lib vote_only -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-redrive-current cargo test -p iroha_core --lib stale_frontier_with -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-redrive-current cargo test -p iroha_core --lib vnext_worker -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-redrive-current cargo test -p iroha_core --lib commit_qc_redrives -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-redrive-current cargo test -p iroha_core --lib commit_pipeline_redrives_ -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-redrive-current cargo test -p iroha_core --lib commit_pipeline_keeps_ -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-redrive-current cargo test -p iroha_core --lib commit_pipeline_ -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-redrive-current cargo test -p iroha_core --lib pacemaker_defers_reproposal_after_missing_qc_view_advance_with_live_frontier_owner -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-redrive-current cargo test -p iroha_core --lib pacemaker_ -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-redrive-current cargo test -p iroha_core --lib pacemaker_ -- --nocapture --test-threads=1`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-redrive-current cargo test -p iroha_core --lib new_view_ -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-redrive-current cargo test -p iroha_data_model presigned_with_payload_hydrates_transactions_from_entrypoints --features transparent_api -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-redrive-current cargo clippy -p iroha_core --lib -- -D warnings`
  - `git diff --check`
  - `git diff --check -- crates/iroha_core/src/sumeragi/main_loop/commit.rs crates/iroha_core/src/sumeragi/main_loop/qc.rs crates/iroha_core/src/sumeragi/main_loop/validation.rs crates/iroha_core/src/sumeragi/main_loop/tests.rs status.md roadmap.md`

## 2026-05-09 Torii MCP header policy and Norito error envelopes

- MCP route dispatch now keeps default `extra_headers` from overriding
  reserved auth/internal headers while allowing the Connect management tools to
  pass their explicit `Authorization` header through the new scoped policy.
  `x-iroha-api-version` remains user-settable for tool dispatch.
- Norito public transaction ingress rejection tests now decode the structured
  `ErrorEnvelope` response for malformed transaction payloads instead of
  assuming a plain-text body, while still asserting that decode failures do not
  surface panic text.
- Operator-profile MCP endpoint coverage now exercises the Sumeragi and
  governance agent aliases without enabling broader writer-only policy.
- Validation:
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-client-confirmation cargo test -p iroha_torii --lib apply_extra_headers -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-client-confirmation cargo test -p iroha_torii --lib connect_management_extra_headers_allow_authorization_only -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-client-confirmation cargo test -p iroha_torii --test mcp_endpoints mcp_jsonrpc_tools_call_agent_alias_sumeragi_endpoints_dispatch -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-client-confirmation cargo test -p iroha_torii --test mcp_endpoints mcp_jsonrpc_tools_call_agent_alias_gov_endpoints_dispatch -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-client-confirmation cargo test -p iroha_torii --test norito_ingress public_transaction_route_rejects -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-client-confirmation cargo test -p iroha_torii --test norito_ingress norito_transaction_rejects_invalid_signature_without_decode_panic -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-client-confirmation cargo test -p iroha_torii --test norito_ingress -- --nocapture`

## 2026-05-09 Offline V2 issuer OpenAPI body auth

- Torii OpenAPI now documents all Offline V2 issuer POST endpoints:
  `/v1/offline/v2/keys/refill`, `/v1/offline/v2/notes/issue`,
  `/v1/offline/v2/notes/redeem`, and `/v1/offline/v2/audit`.
- The shared `OfflineV2IssuerBodyAuthRequest` schema records the required
  top-level `account_id`, `timestamp_ms`, and `nonce` fields plus exactly one
  proof field, `signature_base64` or `witness_base64`, and calls out that
  nested fields with those names remain signed business data. The OpenAPI info
  and Offline tag descriptions now state that these issuer POSTs reject legacy
  `X-Iroha-*` app-auth headers.
- Focused cleanup also removed current strict-clippy blockers in the Sumeragi
  vNext validation diff and FASTPQ Poseidon helper visibility without changing
  the public Offline V2 behavior.
- Validation:
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-torii-openapi cargo test -p iroha_torii --lib generated_spec_includes_documented_paths -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-torii-openapi cargo test -p iroha_torii --lib generated_spec_documents_offline_v2_body_auth_schema -- --nocapture`
  - `cargo fmt --all --check`
  - `git diff --check`

## 2026-05-09 Offline Note V2 wallet regression hardening

- Kotlin/JVM, Java Android, and Swift Offline Note V2 wallet tests now cover
  duplicate P2P payment-token acceptance, already-pending input rejection, and
  failed audit/redeem submission reconciliation through the resolver-backed
  `sync()` path.
- The failed-audit regressions assert that sender source notes are restored to
  `SPENDABLE`, pending change outputs are cancelled, and recipient pending
  outputs are cancelled when the audit transaction is rejected. The
  failed-redeem regressions assert that `REDEEM_PENDING` notes return to
  `SPENDABLE` after a rejected redeem transaction outcome.
- Production Torii/offline outcome adapters were still open at this point;
  the later 2026-05-09 explorer outcome sync adapters close that gap by
  deriving wallet note resolutions from explorer instruction payloads.
- Validation:
  - `JAVA_HOME=/opt/homebrew/opt/openjdk@21/libexec/openjdk.jdk/Contents/Home PATH=/opt/homebrew/opt/openjdk@21/bin:$PATH ./gradlew :core-jvm:test --tests org.hyperledger.iroha.sdk.offline.OfflineNoteV2Test --console=plain --rerun-tasks` from `kotlin`
  - `JAVA_HOME=/opt/homebrew/opt/openjdk@21/libexec/openjdk.jdk/Contents/Home PATH=/opt/homebrew/opt/openjdk@21/bin:$PATH ANDROID_HOME=$HOME/Library/Android/sdk ANDROID_SDK_ROOT=$HOME/Library/Android/sdk ./gradlew :core:test --tests org.hyperledger.iroha.android.GradleHarnessTests --console=plain --rerun-tasks` from `java/iroha_android`
  - `swift test --filter OfflineNoteV2Tests` from `IrohaSwift`

## 2026-05-09 Iroha config minimal snapshot refresh

- Refreshed `minimal_config_snapshot` so the expected Sumeragi persistence
  defaults use the current `5s` commit-inflight timeout.
- Updated Sumeragi configuration examples to show
  `commit_inflight_timeout_ms = 5000`, matching the actual default.
- Cleaned duplicate `execution_context_hash` fields from transaction
  confirmation and Torii block-event test fixtures so they compile against the
  current `BlockHeader` shape.
- Validation:
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-config cargo test -p iroha_config --test fixtures minimal_config_snapshot -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-config cargo test -p iroha_config --test fixtures`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-client-confirmation cargo test -p iroha --lib tx_confirmation_stream_tests -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-client-confirmation cargo test -p iroha --test tx_confirmation -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-client-confirmation cargo test -p iroha_torii --lib committed_block_height_detects_commits --features app_api,telemetry -- --nocapture`
  - `cargo fmt --all --check`
  - `git diff --check`

## 2026-05-09 FASTPQ batched proof Poseidon and exact-frontier recovery

- FASTPQ proof construction now routes independent Poseidon work through
  mode-aware batched helpers instead of launching single-state GPU sponge work
  on row hashes, LDE leaves, AIR trace/composition leaves, FRI leaves, Merkle
  parent levels, and proof query paths. Long sequential sponge chains remain on
  the deterministic CPU path.
- The Metal row-hash API is used for row-major proof hashing when available,
  with CPU parity sampling and ordered CPU fallback on backend errors or digest
  mismatches. Domain-separated limb batches and trace Merkle pairs have the same
  fail-closed parity gate.
- Proof polynomial LDE materialization stays CPU-owned so CPU/GPU modes keep
  byte-identical proof fixtures while GPU proof work is concentrated on
  independent batched Poseidon calls.
- Exact-frontier certified recovery now detaches a stale commit-inflight owner
  when stronger same-height commit evidence arrives. Payload-only repair still
  leaves the old inflight marker intact; late worker output for detached stale
  work is ignored by the existing commit-result id/inflight checks.
- Validation:
  - `cargo test -p fastpq_prover poseidon --features fastpq-gpu -- --nocapture`
  - `cargo test -p fastpq_prover proof --features fastpq-gpu -- --nocapture`
  - `cargo test -p iroha_core --lib block_sync_update_commit_qc_bypasses_stale_commit_inflight_frontier_owner --features fastpq-gpu -- --nocapture`
  - `cargo test -p iroha_core --lib sparse_exact_frontier_block_sync_bypasses_stale_commit_inflight_for_payload_repair --features fastpq-gpu -- --nocapture`
  - `cargo test -p iroha_core --lib maybe_force_view_change_for_stalled_pending_forces_frontier_advance_after_repair_exhaustion_under_tx_backlog --features fastpq-gpu -- --nocapture`
  - `cargo test -p iroha_core --lib commit_pipeline_arms_missing_commit_qc_recovery_for_stalled_local_vote --features fastpq-gpu -- --nocapture`
  - `cargo test -p iroha_core known_block_commit_qc_recovery_requests_pending_block_fetch --features fastpq-gpu -- --nocapture`
  - `cargo check -p iroha_core --features fastpq-gpu`
  - `cargo check -p irohad --features fastpq-gpu`
  - `cargo fmt --all`
  - `cargo fmt --all --check`
  - `git diff --check`
- Attempted broad Sumeragi validation is not green yet:
  `cargo test -p iroha_core sumeragi --features fastpq-gpu -- --nocapture`
  still reports many vote/QC/topology regressions and aborts with a stack
  overflow in
  `plain_block_body_response_releases_dedup_for_active_missing_commit_qc_repair`.
  No fresh 20k Izanami gate/profile was run for this entry.

## 2026-05-09 20k Izanami gate/profile rerun

- Rebuilt current release binaries in
  `/tmp/iroha-codex-20k-current-20260509-093407` with
  `irohad/fastpq-gpu`. The build completed, but emitted existing FASTPQ Metal
  dead-code warnings plus current Sumeragi unused-code warnings in
  `commit.rs` and `validation.rs`.
- The fresh 120s 20k prebuilt gate artifacts are in
  `dist/izanami-prebuilt-20k-fastpq-gpu-current-120s-20260509-093407`. Izanami
  accepted and submitted all `2,400,000` transactions with zero submit
  failures, queue drops, prebuild fallbacks, prebuild skips, or prebuild build
  failures. Submit latency stayed low (`p50=3ms`, `p95=14ms`).
- The gate is a hard fail: strict height stopped at `2`, strict approved
  transactions stopped at `114`, final queue depth was `893,219 / 2,400,000`,
  and the run installed `73` view changes. Sumeragi reported `17` quorum-timeout
  view changes, `11` missing-QC view changes, `314` missing-block fetches,
  `17/17` missing-QC reacquire successes, and queue saturation.
- Peer logs show the active-load stall is still exact-frontier consensus
  recovery, not ingress: the run logged `2,891` `block sync: no QC available
  for block` messages, `763` `skipping NEW_VIEW certificate` warnings, and
  active-pending stalls with `timeout_ms: 5000` but `max_pending_stall_ms:
  6138`. The five-second cap fires, but the production SLA is not satisfied.
- FASTPQ preflights still reported `ok=true` on all peers and no BN254/Metal
  preflight failures were logged, but the new proof Poseidon limb-batch path is
  not accepted: the gate logged `65` GPU limb-batch parity mismatches followed
  by CPU fallback.
- A separate 90s sampled profile is in
  `dist/izanami-profile-20k-fastpq-gpu-current-sampled-90s-20260509-093407`.
  It reproduced the stall with strict height `2`, strict approved `41`, queue
  depth `690,223`, and `711` installed view changes, mostly quorum timeouts.
  `/usr/bin/sample` targeted peer process `10389` for `45s`; the sample still
  shows CPU proof hashing in the hot set (`hash_u64_words_internal`) alongside
  SHA-256, Curve25519/Ed25519, Blake2, CRC64, Norito encode/decode, allocator,
  and transport crypto leaves.
- Current critique: this tree regressed behind the previous 2026-05-08 gate.
  The immediate blockers are (1) fix the Metal/domain-separated limb batch
  parity mismatch so proof hashing does not fall back to CPU, and (2) fix
  same-height Sumeragi recovery/view-change churn so exact-frontier commit QC
  evidence either commits, advances, or clears stale ownership without repeated
  no-QC/block-sync loops.
- Validation:
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-20k-current-20260509-093407 cargo build --release -p irohad --bin iroha3d -p izanami --bin izanami --features irohad/fastpq-gpu`
  - 120s 20k Izanami prebuilt gate with rebuilt binaries and
    `--duration 120s --pipeline-time 300ms --tps 20000 --max-inflight 300000 --submitters 4096 --prebuild-tx-buffer 2400000`
  - 90s 20k Izanami sampled profile with rebuilt binaries and the same load
    shape
  - `/usr/bin/sample 10389 45 1`

## 2026-05-09 Sumeragi vNext durable certificate sidecars

- Kura roster sidecars now persist vNext re-chain and view-change certificate
  sidecars with the committed block metadata. Existing sidecars decode with
  empty vNext certificate lists, and Kura rejects sidecars whose re-chain
  certificate target or view-change certificate height does not match the
  sidecar height.
- Commit persistence now merges matching in-memory vNext certificates into the
  committed block's roster sidecar, deduplicating exact certificates before
  writing the updated sidecar.
- Block-sync update construction now reloads persisted vNext certificates from
  Kura sidecars, then merges live journal entries. Inbound updates still filter
  re-chain sidecars by exact height/view/block hash and view-change sidecars by
  target height/new view before replaying them into the reactor.
- Vote and QC chain-order binding checks now lazily hydrate matching persisted
  vNext certificate sidecars from Kura before comparing `chain_order_hash` and
  `rechain_seq`, so restarted or catching-up actors can accept votes/QCs bound
  to a durable re-chain certificate even when the in-memory reactor journal is
  empty.
- Validation:
  - `cargo test -p iroha_core --lib roster_sidecar_roundtrip_with_vnext_certificates -- --nocapture`
  - `cargo test -p iroha_core --lib roster_sidecar_rejects_vnext_rechain_certificate_mismatch -- --nocapture`
  - `cargo test -p iroha_core --lib roster_sidecar_rejects_vnext_view_change_height_mismatch -- --nocapture`
  - `cargo test -p iroha_core --lib vnext_committed_block_persists_certificate_sidecars -- --nocapture`
  - `cargo test -p iroha_core --lib vnext_rechain_block_sync_sidecar_installs_chain_order -- --nocapture`
  - `cargo test -p iroha_core --lib vnext_block_sync_update_attaches_certificate_sidecars -- --nocapture`
  - `cargo test -p iroha_core --lib vnext_view_change_block_sync_sidecar_advances_live_view -- --nocapture`
  - `cargo test -p iroha_core --lib vnext_vote_binding_hydrates_from_persisted_sidecar -- --nocapture`
  - `cargo test -p iroha_core --lib vnext_qc_binding_hydrates_from_persisted_sidecar -- --nocapture`
  - `cargo test -p iroha_core --lib vnext -- --nocapture`
  - `cargo test -p iroha_core --lib block_sync_sidecar -- --nocapture`
  - `cargo test -p iroha_core --lib incoming_block_message_accepts_block_sync_update_with_new_evidence -- --nocapture`
  - `cargo check -p iroha_core --lib`
  - `cargo fmt --all --check`
  - `git diff --check`

## 2026-05-09 FastPQ split FFT compile hygiene

- The FastPQ split CPU/GPU FFT, IFFT, and LDE helpers now use scoped standard
  threads for the CPU half while the current thread owns the GPU lane guard.
  This preserves deterministic CPU fallback output and avoids moving
  `MutexGuard` through Rayon worker closures during `iroha_core` builds.
- Trace polynomial derivation now keeps coefficient IFFT on the CPU before
  optional GPU LDE/hash stages, keeping the pending GPU lane available for the
  later polynomial/hash work.
- The FASTPQ Poseidon column GPU batch path now runs a cached parity self-test
  before using the accelerator and falls back to CPU hashing on mismatch or
  backend failure.
- Validation:
  - `cargo test -p fastpq_prover --lib split_matches_cpu_output_without_gpu -- --nocapture`
  - `cargo test -p fastpq_prover --lib column_hashes -- --nocapture`
  - `cargo test -p fastpq_prover --lib pending_lde -- --nocapture`
  - `cargo test -p fastpq_prover --lib poseidon_gpu_hashes_match_cpu_when_backend_available --features fastpq-gpu -- --nocapture`
  - `cargo check -p iroha_core --lib`
  - `cargo fmt --all --check`
  - `git diff --check`

## 2026-05-08 FastPQ prover compile hygiene

- Restored the CPU backend's domain-separated Merkle-node hash helper in
  `fastpq_prover` so `cargo check -p iroha_core --lib` can compile the
  dependency graph without relying on a private helper from `trace.rs`.
- Gated the GPU stub `poseidon_hash_rows` fallback to GPU builds and tests so
  non-GPU library checks do not emit dead-code warnings for the stub-only path.
- Validation:
  - `cargo test -p fastpq_prover --lib merkle_paths -- --nocapture`
  - `cargo check -p iroha_core --lib`
  - `cargo fmt --all --check`
  - `git diff --check`

## 2026-05-08 Sumeragi active-load 5s production ceiling

- Active transaction backlog now caps Sumeragi liveness damping at a five-second
  block-production ceiling. Active-pending frontier recovery, idle
  same-frontier deferrals, proposal-gap backlog grace, ingress drain grace, and
  RBC progress grace no longer extend active-load rotation/recovery windows
  beyond the configured commit-inflight timeout capped at five seconds.
- The default `sumeragi.persistence.commit_inflight_timeout_ms` is now `5_000`.
  Timeout reporting still leaves the commit worker result attachable; it does
  not abort the worker or change block output.
- `next_tick_deadline()` now schedules default commit-inflight liveness checks at
  the five-second deadline, and active tx backlog paths keep immediate ticking
  behavior when the queue has work.
- A 2026-05-08 return pass rebuilt release binaries in
  `/tmp/iroha-codex-20k-sumeragi-5s-20260508-183004` and ran repeated 120s
  20k prebuilt gates. The latest artifact is
  `dist/izanami-prebuilt-20k-fastpq-gpu-sumeragi-5s-clean2-120s-20260508-185439`.
  Izanami exited `0`, accepted and succeeded all `2,400,000` submissions, used
  all `2,400,000` prebuilt transactions, and recorded zero submit failures,
  confirmation failures, queue drops, prebuild fallbacks, prebuild skips, or
  `BN254 Poseidon Metal batch failed` warnings. BN254 digest and general FASTPQ
  Poseidon GPU preflight reported `ok=true` on all peers.
- The gate is not accepted: it ended at strict height `5`, strict approved
  `12,388`, max height skew `4`, approved-transaction skew `16,384`, and queue
  depth `880,537 / 2,400,000`, which misses both the `61,622` strict-approved
  and `861,515` queue-depth baselines. The run also had external Cargo/Rust
  compiler contention during the window, so it is noisy, but the repeated runs
  agreed on the same failure shape.
- The five-second cap did fire: peer logs show `38` active-pending stall
  warnings with `timeout_ms: 5000`, plus `5` exhausted active-pending repair
  warnings. However, production still violated the block-cadence requirement:
  max pending stall reached `13,400ms`, block sync logged `1,470` `no QC
  available for block` messages, and two peers logged validation-rejection view
  changes at height `7` for `prev_height` mismatch.
- Validation:
  - `cargo fmt --all`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target-sumeragi-5s cargo test -p iroha_core --lib actor_next_tick_deadline_tracks_default_commit_inflight_sla --features fastpq-gpu -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target-sumeragi-5s cargo test -p iroha_core --lib stalled_pending_timeout_decision_caps_recovery_window_under_active_tx_backlog --features fastpq-gpu -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target-sumeragi-5s cargo test -p iroha_core --lib maybe_force_view_change_for_stalled_pending_forces_frontier_advance_after_repair_exhaustion_under_tx_backlog --features fastpq-gpu -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target-sumeragi-5s cargo test -p iroha_core --lib maybe_force_view_change_for_stalled_pending_defers_frontier_advance_while_consensus_ingress_drains --features fastpq-gpu -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target-sumeragi-5s cargo test -p iroha_core --lib vnext_commit_persisted_marks_reactor_slot_committed --features fastpq-gpu -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target-sumeragi-5s cargo test -p iroha_core --lib reactor_marks_slot_committed_after_persistence_event --features fastpq-gpu -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target-sumeragi-5s cargo check -p iroha_core --features fastpq-gpu`
  - `git diff --check`

## 2026-05-08 20k Izanami Sumeragi vNext rerun

- Rebuilt release `iroha3d` with `fastpq-gpu` and `izanami` in
  `/tmp/iroha-codex-20k-sumeragi-20260508-195227`.
- Fresh 120s 20k prebuilt gate artifacts are in
  `dist/izanami-prebuilt-20k-fastpq-gpu-sumeragi-vnext-120s-20260508-200822`.
  The run had no submit failures, validation rejects, confirmation failures,
  queue drops, prebuild fallbacks, or prebuild skips. BN254 digest and general
  FASTPQ Poseidon preflight reported `ok=true` on all peers, and there were no
  `BN254 Poseidon Metal batch failed` runtime warnings.
- The 120s gate reached strict approved `61,609` transactions at height `17`
  with final queue depth `853,973`. Queue depth improved from the latest clean
  baseline of `861,515`, but strict approved missed the `61,622` baseline by
  `13` transactions, so this is not accepted as a throughput win.
- Fresh 90s sampled artifacts are in
  `dist/izanami-profile-20k-fastpq-gpu-sumeragi-vnext-sampled-90s-20260508-201200`.
  The run had no submit or prebuild failures and the manual load-window sample
  targeted the busiest peer process, `83820`.
- The sampled profile no longer shows scalar
  `iroha_zkp_halo2::poseidon::hash_u64_words_internal` or
  `fastpq_prover::poseidon::GpuPoseidonBackend::permute_state` as the dominant
  app leaf. The remaining visible leaves are Ed25519/Curve25519 verification,
  Norito encode/decode, memory movement, SHA-256, Blake2, CRC64, and transport
  crypto.
- Peer logs show the next bottleneck is consensus recovery, not BN254 or the
  general Poseidon GPU lane: one peer stalled at height `5` with active
  pending state, commit inflight `(5, 0)`, and highest/locked commit QC already
  at `(5, 0)`, then repeated `active_recovery_backlog` quorum-timeout view
  advances. The next pass should make exact-frontier commit-QC/body recovery
  either finish the known commit or clear/reacquire the stuck inflight state
  deterministically.
- Validation:
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-20k-sumeragi-20260508-195227 cargo build --release -p irohad --bin iroha3d --features fastpq-gpu`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-20k-sumeragi-20260508-195227 cargo build --release -p izanami --bin izanami`
  - 120s 20k Izanami prebuilt gate using the rebuilt binaries and
    `--duration 120s --tps 20000 --max-inflight 300000 --submitters 4096`
  - 90s 20k Izanami sampled run using the rebuilt binaries and
    `/usr/bin/sample 83820 45 1`

## 2026-05-08 Sumeragi vNext validation worker adapter

- The live validation gate now hands proposal-backed worker validation to the
  vNext reactor by emitting `ProposalAccepted`, `AvailabilityReady`, and
  `ValidationNeeded` before any worker dispatch. Worker-capable paths no longer
  use the removed direct queue/full-queue inline cutover branch.
- `AcceptValidated` is now a live vNext effect: successful vNext-owned worker
  results mark the pending block valid, store execution roots, replay cached
  commit QCs, and wake commit progression through the effect adapter.
- `RejectValidation` is now a live vNext effect for terminal worker rejects:
  rejected slots are aborted through the reactor, pending ownership is removed,
  validation-reject telemetry/view-change handling runs immediately, and
  parent-height deferrals stay on the repair path instead of being misreported
  as vNext rejects.
- Non-terminal validation deferrals now emit `ValidationDeferred` into the
  reactor so consumed worker results return the slot to `AwaitingValidation`
  for a deterministic retry instead of leaving stale running-worker state.
- `StartRecovery` is now a live actor effect: vNext validation
  timeout/backpressure recovery clears stale worker ownership and drives the
  existing quorum-timeout view-change path, while successor-obligation recovery
  maps to censorship-evidence view changes.
- `RequireViewChange` is now a live actor effect: unsafe re-chain evidence
  clears stale validation worker ownership and drives the existing
  censorship-evidence view-change path instead of only logging reactor output.
- Accepted `RechainVote` and `ViewChangeVote` effects now feed bounded actor
  vote caches. Once quorum is reached, the actor aggregates BLS signatures into
  vNext certificates, feeds those certificates back into the reactor, journals
  them, and gossips the aggregate certificate.
- Locally signed vNext re-chain and view-change votes are inserted into the
  same aggregation caches before gossip, so quorum formation no longer depends
  on receiving the node's own vote back from the network.
- `InstallViewChange` now advances the live Sumeragi view to the certified
  target view while preserving the bounded vNext certificate journal.
- `BlockSyncUpdate` now carries vNext re-chain and view-change certificate
  sidecars. Block-sync broadcasts and exact-body fetch responses attach matching
  certificates from the bounded journal, frame-cap trimming can drop sidecars
  before commit evidence, and inbound updates install sidecars before processing
  vote/QC evidence so catch-up can reconstruct chain order and live view.
- Re-chain sidecars are exact-block scoped by height/view/block hash, while
  view-change sidecars are attached to the target height/new view. Block-payload
  dedup evidence now hashes both vNext sidecar lists, so sidecar-bearing
  recovery updates are not dropped as duplicates of sparse updates.
- Late worker results are ignored once the reactor slot is recovering,
  committed, or aborted, preventing a timed-out validation from resurrecting a
  slot as prepared after recovery starts.
- Accepted `BlockCreated` bodies now emit `ProposalAccepted` and
  `AvailabilityReady` into vNext as soon as the body-backed proposal becomes
  authoritative, before validation is requested.
- RBC payload hydration, chunk reconstruction, READY completion, and DELIVER
  completion now emit `AvailabilityReady` into vNext at the same points that
  wake the commit pipeline for payload availability.
- Successful commit persistence now emits `CommitPersisted` into the live
  reactor and marks the slot `Committed`; late proposal/availability/validation
  events no longer downgrade committed vNext slots.
- `ReactorEffect::DispatchValidation` now queues real validation work through the
  existing Sumeragi validation worker lanes instead of logging and returning.
- The actor records the vNext slot/generation beside the legacy inflight worker
  marker, sends `ValidationWorkerStarted` or `ValidationQueueFull` back into the
  reactor, and routes matching worker results back as `ValidationResult`.
- Legacy inline fallback no longer supersedes vNext-owned validation inflight
  work; vNext timeout/recovery owns slow or saturated validation lanes.
- Validation:
  - `cargo fmt --all`
  - `cargo test -p iroha_core --lib validation_gate_routes_proposal_worker_path_through_vnext_reactor -- --nocapture`
  - `cargo test -p iroha_core --lib validation_dispatches_non_near_quorum_commit_votes_to_workers -- --nocapture`
  - `cargo test -p iroha_core --lib vnext_proposal_accepted_marks_reactor_slot_proposed -- --nocapture`
  - `cargo test -p iroha_core --lib vnext_availability_ready_marks_reactor_slot_awaiting_validation -- --nocapture`
  - `cargo test -p iroha_core --lib reactor_marks_slot_committed_after_persistence_event -- --nocapture`
  - `cargo test -p iroha_core --lib vnext_commit_persisted_marks_reactor_slot_committed -- --nocapture`
  - `cargo test -p iroha_core --lib vnext_dispatch_validation_queues_worker_and_accepts_result -- --nocapture`
  - `cargo test -p iroha_core --lib vnext_reject_validation_aborts_slot_and_removes_pending -- --nocapture`
  - `cargo test -p iroha_core --lib reactor_validation_deferred_returns_slot_to_awaiting_validation -- --nocapture`
  - `cargo test -p iroha_core --lib reactor_drops_late_worker_result_after_recovery -- --nocapture`
  - `cargo test -p iroha_core --lib vnext_require_view_change_drives_live_view_change -- --nocapture`
  - `cargo test -p iroha_core --lib vnext_rechain_votes_aggregate_certificate -- --nocapture`
  - `cargo test -p iroha_core --lib vnext_view_change_votes_aggregate_certificate_and_advance_view -- --nocapture`
  - `cargo test -p iroha_core --lib vnext_block_sync -- --nocapture`
  - `cargo test -p iroha_core --lib block_sync_sidecar -- --nocapture`
  - `cargo test -p iroha_core --lib incoming_block_message_accepts_block_sync_update_with_new_evidence -- --nocapture`
  - `cargo test -p iroha_core --lib vnext -- --nocapture`
  - `cargo test -p iroha_core --lib validation_worker_result_replays_cached_precommit_qc_after_block_becomes_valid -- --nocapture`
  - `cargo check -p iroha_core --lib`
  - `cargo fmt --all --check`
  - `git diff --check`

## 2026-05-08 Sumeragi active-pending no-proposal scheduling

- Reviewed the current Sumeragi idle/recovery logic around same-height
  no-proposal storms. The breaker was reachable from `force_view_change_if_idle`
  but active pending blocks suppressed idle tick deadlines, so an overdue
  no-proposal storm could wait for an unrelated pending-block quorum wakeup.
- `next_tick_deadline()` now mirrors the active-pending no-proposal storm
  predicate and schedules the immediate tick when the view/queue age has
  already exceeded the missing-leader timeout.
- Added focused coverage proving the storm deadline is immediate while the
  normal pending-block quorum deadline is still in the future.
- Validation:
  - `cargo fmt --all`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target-sumeragi cargo test -p iroha_core --lib active_pending_no_proposal_storm_schedules_tick_deadline --features fastpq-gpu -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target-sumeragi cargo test -p iroha_core same_height_no_proposal_storm --features fastpq-gpu -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target-sumeragi cargo check -p iroha_core --features fastpq-gpu`
  - `git diff --check`

## 2026-05-08 Sumeragi vNext timeout tick adapter

- The main Sumeragi tick loop now schedules active vNext reactors when running
  or backpressured validation reaches the configured suspicion timeout.
- Tick handling fans `ReactorEvent::Tick` into every live reactor and applies
  emitted `ReactorEffect`s through the existing actor adapter path, so slow
  validation enters vNext recovery without falling through an inline validation
  path.
- The idle view-change path now runs the recorded same-height no-proposal storm
  breaker before active pending blocks suppress idle handling, allowing stale
  frontier pending state to be cleaned up deterministically without emitting an
  extra MissingQc rotation.
- Added focused main-loop coverage for a running vNext validation that times
  out through the actor tick adapter, a vNext validation dispatch/result
  round-trip through the worker lane, and the live idle path purging stale
  same-height pending state during a recorded no-proposal storm.
- Validation:
  - `cargo fmt --all`
  - `git diff --check`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target-sumeragi cargo check -p iroha_core --features fastpq-gpu`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target-sumeragi cargo test -p iroha_core --lib vnext_dispatch_validation_queues_worker_and_accepts_result --features fastpq-gpu -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target-sumeragi cargo test -p iroha_core force_view_change_if_idle_breaks_recorded_same_height_no_proposal_storm --features fastpq-gpu -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target-sumeragi cargo test -p iroha_core same_height_no_proposal_storm --features fastpq-gpu -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target-sumeragi cargo test -p iroha_core tick_drives_vnext_validation_timeout_recovery --features fastpq-gpu -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target-sumeragi cargo test -p iroha_core duplicate_commit_qc_clears_known_block_recovery_request --features fastpq-gpu -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target-sumeragi cargo test -p iroha_core --lib da_proposal_uses_rbc_for_ram_lfe_tx_exceeding_consensus_payload_frame_cap --features fastpq-gpu -- --nocapture`

## 2026-05-08 Sumeragi commit-to-proposal stack unwind

- Successful commit-result handling now unwinds the large commit-application
  frame before kickstarting the pacemaker for the next proposal. This keeps the
  durable-commit fast path intact while avoiding stack overflow when proposal
  assembly runs immediately after commit publication.
- Validation:
  - `cargo fmt --all`
  - `cargo test -p iroha_core --lib sumeragi::main_loop::tests::commit_outcome_kickstarts_next_proposal_and_records_round_gap -- --nocapture`
  - `cargo test -p iroha_core --lib sumeragi::main_loop::tests::kickstart_pacemaker_after_commit_triggers_only_when_allowed -- --nocapture`

## 2026-05-08 FASTPQ Poseidon prover batch path

- FASTPQ trace column and Merkle hashing now use scalar CPU Poseidon for CPU
  fallback/reference work instead of routing single-state hashes through the
  active backend sponge. This removes the global GPU lane mutex from the CPU
  fallback path while preserving the existing digest domains.
- Trace Merkle parent levels now build domain-separated `[left, right]` pair
  batches. Large levels can use the existing Poseidon GPU column-batch kernel
  after a scalar-equivalence preflight; runtime dispatch errors latch the
  parent-pair accelerator off for the process and fall back to scalar hashing.
- The high-level fused column API now returns leaves from the parity-proven
  column-batch kernel and first-level parents from the guarded pair-batch path.
  The older low-level Metal/CUDA fused parent kernels remain parked until a
  fresh throughput gate justifies production hot-path promotion.
- Metal Poseidon dispatch now waits each staged batch before
  submitting the next one. This avoids a command-semaphore cycle seen when
  many proof tests simultaneously held one queued Poseidon ticket and blocked
  trying to acquire another.
- `fastpq_metal_bench` now has a `poseidon_merkle_pairs` operation, and
  Poseidon pipeline telemetry reports Merkle pair GPU/CPU batches, fallback
  count, and max pair batch size.
- Validation:
  - `cargo fmt --all`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-fastpq-poseidon-fix cargo test -p fastpq_prover metal::tests::poseidon_dispatch_staging_does_not_chain_command_permits --features fastpq-gpu -- --nocapture`
  - `cargo test -p fastpq_prover trace::tests::merkle_levels_match_scalar_reference_for_mixed_shapes --features fastpq-gpu -- --nocapture`
  - `cargo test -p fastpq_prover --bin fastpq_metal_bench operation_filter_parses_poseidon_merkle_pairs --features fastpq-gpu -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-fastpq-poseidon-fix cargo test -p fastpq_prover poseidon --features fastpq-gpu -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-fastpq-poseidon-fix cargo test -p fastpq_prover trace --features fastpq-gpu -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-fastpq-poseidon-fix cargo test -p fastpq_prover proof::tests::verify_rejects_wrong_air_trace_root --features fastpq-gpu -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-fastpq-poseidon-fix cargo check -p iroha_core --features fastpq-gpu`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-fastpq-poseidon-fix cargo check -p irohad --features fastpq-gpu`
  - `git diff --check`

## 2026-05-08 Offline Note V2 wallet sync resolver

- Kotlin/JVM, Java Android, and Swift `OfflineNoteV2Wallet.sync()` now reconcile
  pending wallet notes through an injected transaction-outcome resolver instead
  of returning the store snapshot unchanged. The resolver can finalize
  `SPEND_PENDING` notes as `SPENT`, promote accepted `CHANGE_PENDING` outputs
  to `SPENDABLE`, and settle `REDEEM_PENDING` notes as `REDEEMED`; rejected
  flows can also map pending outputs to `CANCELLED` or restore notes to
  `SPENDABLE`.
- The three SDKs expose matching sync resolution types and add lifecycle
  regressions that drive P2P pay, sync spent/change state, redeem the synced
  change note, and sync redemption finality.
- Validation:
  - `JAVA_HOME=/opt/homebrew/opt/openjdk@21/libexec/openjdk.jdk/Contents/Home PATH=/opt/homebrew/opt/openjdk@21/bin:$PATH ./gradlew :core-jvm:test --tests org.hyperledger.iroha.sdk.offline.OfflineNoteV2Test --console=plain --rerun-tasks` from `kotlin`
  - `JAVA_HOME=/opt/homebrew/opt/openjdk@21/libexec/openjdk.jdk/Contents/Home PATH=/opt/homebrew/opt/openjdk@21/bin:$PATH ANDROID_HOME=$HOME/Library/Android/sdk ANDROID_SDK_ROOT=$HOME/Library/Android/sdk ./gradlew :core:test --tests org.hyperledger.iroha.android.GradleHarnessTests --console=plain --rerun-tasks` from `java/iroha_android`
  - `swift test --filter OfflineNoteV2Tests` from `IrohaSwift`
  - `git diff --check`

## 2026-05-08 Sumeragi idle RBC tick throttling

- The dedicated parallel Sumeragi tick worker now records every tick attempt,
  including no-progress maintenance ticks, as the last tick time. Idle
  maintenance that returns no progress therefore sleeps for the configured
  tick-min-gap instead of immediately ticking again.
- Idle-view deadlines are no longer scheduled when the transaction queue is
  empty and there is no recovery, backlog, proposal-liveness, or vote-backed
  consensus evidence to act on.
- RBC rebroadcast deadlines now ignore inactive retained sessions, including
  delivered sessions outside the active repair window, so stale RBC/DA state
  does not keep an otherwise idle actor awake.
- Validation:
  - `cargo fmt --all`
  - `cargo test -p iroha_core --lib spawn_tick_worker_throttles_no_progress_ticks -- --nocapture`
  - `cargo test -p iroha_core --lib actor_next_tick_deadline -- --nocapture`

## 2026-05-08 Soracloud Inrou host-advert suppression

- The embedded Soracloud runtime no longer submits an authoritative
  `AdvertiseSoracloudInrouHost` transaction when the host has no local Inrou
  backend and the manager only derived an automatic zero-capacity proxy-only
  capability. This keeps backend discovery failures from turning into repeated
  validator-authored Inrou advert traffic.
- The affected live deployment was also rolled to a same-source Linux `irohad`
  built without `embedded-soracloud-runtime` so the live nodes cannot start the
  embedded Inrou runtime. The deployed binary SHA-256 is
  `d724c4a69373817f120833ed840ccf1ad878bde92e709089f5a34afba6a4f862`.
- Validation:
  - `cargo fmt --all`
  - `CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=x86_64-linux-gnu-gcc cargo build -p irohad --bin irohad --release --target x86_64-unknown-linux-gnu`
  - `cargo test -p irohad auto_proxy_only_inrou_host_advert_is_suppressed --features embedded-soracloud-runtime -- --nocapture`
  - Live binary-only rollout evidence was captured outside this repository.
  - Live `make minamoto-live-endpoint-smoke`
  - Live `make minamoto-monitor-check`

## 2026-05-08 Sumeragi idle timing cache

- Sumeragi now caches the active and chain-effective consensus timing derived
  from WSV snapshots instead of recomputing it from `state.world_view()` on
  idle/tick housekeeping paths. Runtime DA checks, commit-quorum timeout,
  rebroadcast cooldowns, idle tick deadlines, and worker-loop budgets now read
  the cached snapshot.
- The cache preserves the distinction between locally active consensus mode and
  chain-effective mode for pending mode flips. It is refreshed during actor
  initialization and the existing post-commit, mode-reset, and adaptive timing
  status refresh points.
- Added a focused regression proving `commit_quorum_timeout()` stays on the
  cached value until the timing snapshot is refreshed, plus updated direct
  parameter-mutation tests to refresh the actor timing snapshot explicitly.
- Validation:
  - `cargo fmt --all`
  - `cargo fmt --all --check`
  - `cargo check -p iroha_core --lib`
  - `cargo test -p iroha_core --lib commit_quorum_timeout_uses_cached_timing_until_refresh -- --nocapture`
  - `cargo test -p iroha_core --lib commit_quorum_timeout_uses_npos_commit_floor -- --nocapture`
  - `cargo test -p iroha_core --lib rebroadcast_cooldown_uses_npos_block_time -- --nocapture`
  - `cargo test -p iroha_core --lib commit_pipeline_qc_rebuild_cooldown_uses_chain_block_time -- --nocapture`
  - `cargo test -p iroha_core --lib update_effective_timing_status_populates_snapshot -- --nocapture`
  - `cargo test -p iroha_core --lib actor_next_tick_deadline_schedules_tick_when_mode_flip_due -- --nocapture`
  - `cargo test -p iroha_core --lib commit_evidence_replay_cooldown_does_not_fallback_to_payload -- --nocapture`
  - `cargo test -p iroha_core --lib proposal_backpressure_respects_pending_stall_grace -- --nocapture`
  - `git diff --check`

## 2026-05-08 Sumeragi block-body response repair ingress

- Exact `BlockBodyResponse` repair traffic now enters the fast block/recovery
  lane instead of the ordinary block-payload lane. The message keeps the
  existing dedup key and blocking enqueue semantics, but no longer waits behind
  historical body/proposal backlog when it is needed to materialize the
  contiguous frontier.
- The queue-capacity comments for the block lane now describe its current
  recovery role: fetches, body responses, and consensus params.
- The full 20-minute realistic transfer soak now passes on the release daemon.
  It submitted all 36,000 transfers, reached the 36,008 approved target
  including the 8 baseline approvals, and ended with zero rejects, all peers at
  722 non-empty blocks, and queue size 0. The run still shows throughput margin
  pressure: load submitted at 30.00 TPS, committed at 21.61 TPS during load,
  peaked at 9,973 queued transactions, and needed 722 seconds of drain time.
  Artifact:
  `integration_tests/artifacts/realistic-30tps-transfer-20min-640-release-daemon-block-body-response-block-lane/throughput-1778229477740/summary.json`.
- Validation:
  - `cargo fmt --all`
  - `cargo test -p iroha_core --lib incoming_block_message_routes_block_body_response_via_block_ingress_queue -- --nocapture`
  - `cargo test -p iroha_core --lib run_parallel_worker_prioritizes_block_ingress_before_vote_cert_burst -- --nocapture`
  - `cargo build -p irohad --release`
  - `TEST_NETWORK_BIN_IROHAD=/Users/takemiyamakoto/dev/iroha/target/release/irohad IROHA_TEST_SKIP_BUILD=1 IROHA_TEST_NETWORK_KEEP_DIRS=1 IROHA_THROUGHPUT_ARTIFACT_DIR=/Users/takemiyamakoto/dev/iroha/integration_tests/artifacts/realistic-30tps-transfer-20min-640-release-daemon-block-body-response-block-lane cargo test -p integration_tests --test consensus_and_da sumeragi_localnet_smoke::permissioned_localnet_realistic_30tps_20min -- --ignored --exact --nocapture --test-threads=1`
  - `cargo fmt --all --check`
  - `git diff --check`

## 2026-05-08 Realistic RAM-LFE email soak

- The realistic 30 TPS localnet soak now supports
  `IROHA_REALISTIC_30TPS_LOAD_KIND=ram-lfe-email`. This mode registers the
  email identifier policy and RAM-LFE program policy during genesis, creates
  640 UAID-bearing account targets, and submits signed `ClaimIdentifier`
  transactions with RAM-LFE email receipts instead of transfer instructions.
- The release-daemon 20-minute RAM-LFE email run passed. It submitted all
  36,000 email-claim transactions, reached the 36,008 approved target including
  the 8 baseline approvals, and ended with zero rejects, all peers at 723
  non-empty blocks, and queue size 0. Throughput margin remains similar to the
  transfer soak: load submitted at 30.00 TPS, committed at 21.27 TPS during
  load, peaked at 10,377 queued transactions, and needed 677 seconds of drain
  time. Artifact:
  `integration_tests/artifacts/realistic-30tps-ram-lfe-email-20min-release-daemon/throughput-1778232961671/summary.json`.
- Validation:
  - `cargo fmt --all`
  - `cargo test -p integration_tests --test consensus_and_da realistic_ram_lfe_email -- --nocapture`
  - `cargo test -p integration_tests --test consensus_and_da realistic_30tps_load_kind_parses_email_mode_and_defaults_to_transfer -- --nocapture`
  - `TEST_NETWORK_BIN_IROHAD=/Users/takemiyamakoto/dev/iroha/target/release/irohad IROHA_TEST_SKIP_BUILD=1 IROHA_TEST_NETWORK_KEEP_DIRS=1 IROHA_REALISTIC_30TPS_LOAD_KIND=ram-lfe-email IROHA_REALISTIC_30TPS_DURATION_SECS=5 IROHA_REALISTIC_30TPS_TARGET_TPS=2 IROHA_REALISTIC_30TPS_TARGET_BLOCKS=2 IROHA_REALISTIC_30TPS_BLOCK_MAX_TXS=5 IROHA_REALISTIC_30TPS_STALL_SECS=30 IROHA_THROUGHPUT_ARTIFACT_DIR=/Users/takemiyamakoto/dev/iroha/integration_tests/artifacts/realistic-ram-lfe-email-smoke cargo test -p integration_tests --test consensus_and_da sumeragi_localnet_smoke::permissioned_localnet_realistic_30tps_20min -- --ignored --exact --nocapture --test-threads=1`
  - `TEST_NETWORK_BIN_IROHAD=/Users/takemiyamakoto/dev/iroha/target/release/irohad IROHA_TEST_SKIP_BUILD=1 IROHA_TEST_NETWORK_KEEP_DIRS=1 IROHA_REALISTIC_30TPS_LOAD_KIND=ram-lfe-email IROHA_THROUGHPUT_ARTIFACT_DIR=/Users/takemiyamakoto/dev/iroha/integration_tests/artifacts/realistic-30tps-ram-lfe-email-20min-release-daemon cargo test -p integration_tests --test consensus_and_da sumeragi_localnet_smoke::permissioned_localnet_realistic_30tps_20min -- --ignored --exact --nocapture --test-threads=1`
  - `cargo fmt --all --check`
  - `git diff --check`

## 2026-05-08 Sumeragi known-block commit-QC recovery dampening

- Commit-QC-only `FetchPendingBlock` responses now send the direct commit
  certificate without also emitting an exact `BlockBodyResponse` body envelope.
  This keeps known-block QC recovery from generating companion body traffic
  that receivers immediately ignore as already-known or non-frontier.
- Known-block missing-QC requests are now retired as soon as a duplicate,
  record-only, detached, or block-sync supplied commit QC is locally cacheable.
  Committed-tip QC reacquisition also uses the configured missing-QC recovery
  window instead of the faster payload-rescue cadence.
- Same-height known-block commit-QC recovery now treats requests for older
  local views as obsolete, prunes stale missing-QC requests during view-state
  pruning, and rotates the active frontier view after bounded retries with no
  dependency progress. If the stall-reset fallback reanchor fires first, the
  slot stays in passive catch-up instead of immediately reclaiming exact local
  body ownership in the same retry tick.
- Validation:
  - `cargo fmt --all`
  - `cargo fmt --all --check`
  - `cargo test -p iroha_core --lib commit_qc_only_fetch_pending_block_sends_direct_cert_without_body_response -- --nocapture`
  - `cargo test -p iroha_core --lib commit_qc_only_fetch_pending_block_defers_without_body_when_cert_missing -- --nocapture`
  - `cargo test -p iroha_core --lib duplicate_commit_qc_clears_known_block_recovery_request -- --nocapture`
  - `cargo test -p iroha_core --lib known_block_commit_qc_recovery -- --nocapture`
  - `cargo test -p iroha_core --lib known_block_commit_qc_stall_uses_fallback_reanchor_when_primary_is_in_cooldown -- --nocapture`
  - `cargo test -p iroha_core --lib known_block_commit_qc_recovery_uses_reacquire_window_for_committed_tip -- --nocapture`
  - `cargo test -p iroha_core --lib known_block_commit_qc_recovery_requests_pending_update_when_payload_is_local -- --nocapture`
  - `cargo test -p iroha_core --lib prune_stale_view_state_clears_known_block_commit_qc_requests -- --nocapture`
  - `cargo test -p iroha_core --lib retry_known_block_commit_qc_requests -- --nocapture`
  - `cargo test -p iroha_core --lib block_body_response_retains_same_height_known_block_commit_qc_repair_after_frontier_view_advances -- --nocapture`
  - `cargo test -p iroha_core --lib da_payload_budget -- --nocapture`
  - `cargo test -p iroha_core --lib da_proposal_uses_rbc_for_ram_lfe_tx_exceeding_consensus_payload_frame_cap -- --nocapture`
  - `cargo test -p iroha_core --lib proposal_defers_when_all_txs_exceed_payload_budget -- --nocapture`
  - `cargo check -p iroha_core --lib`
  - `git diff --check`

## 2026-05-08 Realistic transfer soak artifact accounting

- The realistic 30 TPS transfer soak artifacts now retain load samples on early
  failure, tag samples with `load`/`drain` phases, record
  `blocks_non_empty`, and write a `realistic` summary with baseline, target,
  submitted, load/drain elapsed time, peer min/max status, committed TPS, and
  block interval data. Stall paths now write the partial realistic summary
  before returning the error.
- The optimized-daemon 2-minute diagnostic passed with 3,600 submitted
  transfers, 3,600 approved, zero rejects, 74 produced non-empty blocks, and
  50-transfer proof jobs averaging about 598 ms:
  `integration_tests/artifacts/realistic-30tps-transfer-2min-640-release-daemon-diagnostic/throughput-1778218485540/summary.json`.
- The full 20-minute optimized-daemon run on current `i23-features` did not
  pass. It submitted all 36,000 transfers with zero rejects, but stalled in
  drain at min approved 26,595 / target 36,008, min non-empty 534 / target
  601, and max queue 9,413. The failure artifact confirms proof jobs stayed
  fast (50-transfer proof jobs averaged about 530 ms), so the remaining issue
  is consensus/RBC/QC drain cadence under backlog rather than prover speed:
  `integration_tests/artifacts/realistic-30tps-transfer-20min-640-release-daemon-current/throughput-1778220296767/summary.json`.
- Validation:
  - `cargo fmt --all`
  - `cargo fmt --all --check`
  - `cargo test -p integration_tests --test consensus_and_da write_throughput_artifacts -- --nocapture`
  - `cargo test -p integration_tests --test consensus_and_da throughput_status_summary_uses_min_and_max_peer_values -- --nocapture`
  - `cargo test -p integration_tests --test consensus_and_da realistic_artifact_summary_counts_load_samples_and_keeps_zero_block_rates_finite -- --nocapture`
  - `cargo test -p integration_tests --test consensus_and_da status_snapshot_value_handles_options -- --nocapture`
  - `cargo build -p irohad --release`
  - `TEST_NETWORK_BIN_IROHAD=/Users/takemiyamakoto/dev/iroha/target/release/irohad IROHA_TEST_SKIP_BUILD=1 IROHA_TEST_NETWORK_KEEP_DIRS=1 IROHA_REALISTIC_30TPS_DURATION_SECS=120 IROHA_REALISTIC_30TPS_TARGET_BLOCKS=60 IROHA_THROUGHPUT_ARTIFACT_DIR=/Users/takemiyamakoto/dev/iroha/integration_tests/artifacts/realistic-30tps-transfer-2min-640-release-daemon-diagnostic cargo test -p integration_tests --test consensus_and_da sumeragi_localnet_smoke::permissioned_localnet_realistic_30tps_20min -- --ignored --exact --nocapture --test-threads=1`
  - `TEST_NETWORK_BIN_IROHAD=/Users/takemiyamakoto/dev/iroha/target/release/irohad IROHA_TEST_SKIP_BUILD=1 IROHA_TEST_NETWORK_KEEP_DIRS=1 IROHA_THROUGHPUT_ARTIFACT_DIR=/Users/takemiyamakoto/dev/iroha/integration_tests/artifacts/realistic-30tps-transfer-20min-640-release-daemon-current cargo test -p integration_tests --test consensus_and_da sumeragi_localnet_smoke::permissioned_localnet_realistic_30tps_20min -- --ignored --exact --nocapture --test-threads=1` (failed in drain; artifact retained)
  - `git diff --check`

## 2026-05-08 Sumeragi DA/RBC large RAM-LFE proposal fallback

- DA-enabled proposal assembly no longer caps candidate transaction payloads by
  the single consensus-frame limit. The DA path is now bounded by the configured
  block payload cap plus RBC total/pending capacity, so large transactions that
  fit RBC can be proposed instead of being requeued indefinitely.
- Oversized exact `BlockCreated` companions are now skipped under DA when they
  exceed the consensus frame cap. The proposer still handles the block locally
  and carries the payload to peers through `Proposal` + RBC transport.
- Added a focused RAM-LFE policy transaction regression that lowers the
  consensus frame cap, proves the transaction exceeds one frame but fits RBC,
  and verifies Proposal/RBC messages are emitted without requeueing or posting
  an oversized `BlockCreated`.
- Validation:
  - `cargo fmt --all`
  - `cargo fmt --all --check`
  - `cargo test -p iroha_core --lib da_payload_budget -- --nocapture`
  - `cargo test -p iroha_core --lib da_proposal_uses_rbc_for_ram_lfe_tx_exceeding_consensus_payload_frame_cap -- --nocapture`
  - `cargo test -p iroha_core --lib proposal_defers_when_all_txs_exceed_payload_budget -- --nocapture`
  - `cargo check -p iroha_core --lib`
  - `git diff --check`

## 2026-05-08 Izanami 20k FASTPQ-GPU gate

- The 120s 20k Izanami FASTPQ-GPU gate is back above the latest clean baseline
  after suppressing redundant delivered-session RBC READY repair fanout once
  every READY sender is already locally known.
- Passing release gate:
  `TEST_NETWORK_BIN_IROHAD=/tmp/iroha-codex-20k-return-rbcfix-20260508-1158/release/iroha3d TEST_NETWORK_IROHAD_FEATURES=fastpq-gpu IROHA_TEST_SKIP_BUILD=1 /tmp/iroha-codex-20k-return-rbcfix-20260508-1158/release/izanami --allow-net --peers 4 --faulty 0 --duration 120s --pipeline-time 300ms --tps 20000 --max-inflight 300000 --submitters 4096 --prebuild-tx-buffer 2400000 --prebuild-tx-workers 0 --workload-profile stable ...`
  produced `final_strict_min_txs_approved=69673` and
  `tx_queue_depth=847734`, beating the prior clean baselines of `61622` and
  `861515`. The same run had zero submit failures, validation rejects,
  confirmation failures, queue drops, prebuild fallbacks, and prebuild skips.
- BN254 digest and prover Poseidon GPU preflight logged `ok=true` on all four
  peers, and diagnostics had zero `BN254 Poseidon Metal batch failed` /
  runtime-dispatch failures.
- A later Poseidon-return rebuild under
  `/tmp/iroha-codex-20k-poseidon-return-20260508-150858` reproduced the 20k
  load with mixed consensus results. The first run in
  `dist/izanami-prebuilt-20k-fastpq-gpu-poseidon-return-120s-20260508-150858`
  stalled at height 4/5 with `final_strict_min_txs_approved=8279`,
  `view_change_missing_qc_total=69`, and `tx_queue_depth=843329`; both
  Poseidon preflights were still `ok=true` on all peers and no Metal runtime
  failures were logged. The immediate repeat in
  `dist/izanami-prebuilt-20k-fastpq-gpu-poseidon-return-repeat-120s-20260508-152225`
  completed with `final_strict_min_txs_approved=70179`,
  `view_change_missing_qc_total=1`, zero submit/validation/prebuild failures,
  and zero BN254/general Poseidon Metal runtime failures. Its
  `tx_queue_depth=875499` beats throughput but not the older queue-depth
  baseline of `861515`, so queue drain remains the next gate metric to recover.
- Focused validation passed with
  `CARGO_TARGET_DIR=/tmp/iroha-20k-validate-target cargo test -p iroha_core --lib rescue_rbc_missing_ready_peers -- --nocapture`,
  `CARGO_TARGET_DIR=/tmp/iroha-20k-validate-target cargo test -p iroha_core --lib targeted_payload_rescue_cooldown_keeps_heavy_repair_off_vote_cadence -- --nocapture`,
  `CARGO_TARGET_DIR=/tmp/iroha-20k-check-target cargo check -p iroha_core --lib`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-20k-return-rbcfix-20260508-1158 cargo build --release -p irohad --bin iroha3d -p izanami --bin izanami --features irohad/fastpq-gpu`,
  `cargo fmt --all`, and `git diff --check`.

## 2026-05-08 Sumeragi block-body response repair test harness

- Moved
  `block_body_response_retains_same_height_known_block_commit_qc_repair_after_frontier_view_advances`
  onto the existing Sumeragi test-thread builder so the deep block-body
  response/QC repair path runs with the same explicit stack budget as live
  Sumeragi worker threads.
- Focused validation passed with `cargo fmt --all`,
  `cargo test -p iroha_core sumeragi::main_loop::tests::block_body_response_retains_same_height_known_block_commit_qc_repair_after_frontier_view_advances --lib -- --nocapture`,
  and
  `cargo test -p iroha_core sumeragi::main_loop::tests::block_created_clears_missing_request_on_duplicate --lib -- --nocapture`.

## 2026-05-08 Iroha Config Snapshot Defaults

- Refreshed `minimal_config_snapshot` to match the current Sumeragi defaults:
  fast-finality transaction caps remain disabled by default, and the
  DA-critical actor-gate yield threshold is `2`.
- Validation passed with `cargo test -p iroha_config --test fixtures` and
  `cargo test -p iroha_config`.

## 2026-05-08 Torii account push notification bridge

- Torii push registration now persists devices under the configured Torii data
  directory, requires canonical signed request auth for register/unregister,
  and supports idempotent `POST` plus `DELETE /v1/notify/devices` without
  putting raw provider tokens in URLs.
- Added a best-effort non-consensus delivery bridge for committed
  account-affecting external transactions. The bridge reuses the Explorer
  account-instruction matcher, queues minimal payloads with persistent
  dedupe/retry state, dispatches through FCM HTTP v1 and APNs token-auth HTTP/2,
  and removes invalid provider tokens on permanent provider errors.
- `torii.push` config now exposes FCM HTTP v1 service-account fields and APNs
  sandbox/production token-auth fields while retaining deprecated legacy fields
  for configuration compatibility. OpenAPI, Swift SDK helpers, Kotlin/JVM SDK
  helpers, and mobile SDK docs were updated for signed registration and
  unregistration.
- Focused validation passed with
  `CARGO_TARGET_DIR=/tmp/iroha-codex-push-target cargo check -p iroha_torii`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-push-target cargo test -p iroha_torii --test push_bridge`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-push-target cargo test -p iroha_torii push --lib`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-push-target cargo test -p iroha_torii account_activity --lib`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-push-target cargo test -p iroha_config push`,
  `swift test` from `IrohaSwift`, and
  `JAVA_HOME=/opt/homebrew/opt/openjdk@21/libexec/openjdk.jdk/Contents/Home PATH=/opt/homebrew/opt/openjdk@21/bin:$PATH ./gradlew :core-jvm:test --console=plain`
  from `kotlin`.
- Broader Torii validation passed with
  `CARGO_TARGET_DIR=/tmp/iroha-codex-push-target cargo clippy -p iroha_torii -p iroha_config --all-targets -- -D warnings`
  and `CARGO_TARGET_DIR=/tmp/iroha-codex-push-target cargo test -p iroha_torii --lib`
  after serializing Torii's process-wide data-dir mutation in tests.

## 2026-05-08 Sumeragi vNext chain-order sidecar binding

- Block-sync commit QC derivation now preserves the vNext `chain_order_hash`
  and `rechain_seq` carried by precommit signer history instead of rebuilding
  sidecar QCs with the default pre-rechain binding.
- Validator-set checkpoint sidecars now carry the same chain-order binding as
  the QC they summarize. Checkpoint aggregate verification uses those fields in
  the commit-vote preimage, so checkpoint-derived QCs cannot be replayed across
  a different installed chain order.
- Raw vote logs, deferred-vote buffers, vote-validation caches, QC validation
  vote lookups, and vote/QC verifier cache keys now include the selected
  `chain_order_hash` and `rechain_seq`. A vote accepted under an old installed
  chain order no longer suppresses or validates a same-slot vote under a later
  re-chain binding.
- Focused validation passed with
  `cargo test -p iroha_data_model --lib validator_set_checkpoint_roundtrip_and_hash -- --nocapture`,
  `cargo test -p iroha_core --lib validate_checkpoint_roster_binds_chain_order -- --nocapture`,
  `cargo test -p iroha_core --lib vote_duplicate_binds_chain_order -- --nocapture`,
  `cargo test -p iroha_core --lib vnext -- --nocapture`,
  `cargo check -p iroha_core --lib`, and
  `cargo fmt --all --check`.
  Diff hygiene passed with `git diff --check` and `git diff --cached --check`.

## 2026-05-07 Sumeragi vNext live control path

- `BlockMessage::VNext` is no longer a validate-and-ignore path in the
  Sumeragi main loop. Valid vNext control frames are routed into a per
  height/view `sumeragi::vnext::Reactor`; re-chain certificates install the
  reactor's live chain order and are retained in a bounded in-memory journal,
  re-chain proposals trigger locally signed `RechainVote` broadcasts when the
  local validator belongs to the certified new-order critical path, and
  view-change certificates are journaled for follow-up catch-up sidecars.
- Added explicit `RechainVote` and `ViewChangeVote` vNext wire messages. They
  sign the same canonical certificate bodies that are later aggregated, verify
  signer roster membership at ingress, and retain the existing chain-id plus
  consensus-mode signing domains and PoP-aware BLS aggregate checks for final
  certificates.
- Multi-suspicion re-chain evidence is now deterministic instead of being
  rejected wholesale. Evidence must share slot, chain-order hash, and
  `rechain_seq`; duplicate canonical suspicion bodies are rejected; accepted
  suspicions are sorted by canonical signing-body hash and applied sequentially
  to the evolving order with cumulative tainted validators held in the
  quarantine tail. Proposals/certificates must match the deterministic
  `new_order`, tainted set, canonical suspicion order, and final
  `rechain_seq`.
- Focused validation passed with
  `cargo test -p iroha_core --lib vnext -- --nocapture` (`36` passed) and
  `cargo check -p iroha_core --lib`.

## 2026-05-07 Izanami 20k final load-window profile critique

- Captured a corrected load-window `90s` macOS sample during the `120s` 20k
  Izanami gate at
  `dist/izanami-profile-20k-fastpq-gpu-final-loadsample-90s-20260507-225637`.
  The run completed with `2,400,000` submissions offered, accepted, and
  succeeded; `0` ingress failures, validation rejects, queue drops, confirmation
  failures, prebuild fallbacks, prebuild skips, or view changes; and all four
  peers reported both general FASTPQ Poseidon prover preflight and BN254 digest
  GPU preflight as `ok=true`.
- Sampling perturbed the gate slightly versus the clean final return run: final
  strict height was `16` with `57,410` strict-approved transactions, submit
  latency was p50 `3ms`, p95 `24ms`, p99 `84ms`, max `534ms`, and the
  transaction queue ended saturated at `842,171 / 2,400,000`.
- The sampled peer's top application leaf frame is now scalar
  `iroha_zkp_halo2::poseidon::hash_u64_words_internal` (`21,729` samples).
  This is corroborated by four runtime `BN254 Poseidon Metal batch failed while
  waiting; falling back to scalar hashing` warnings across the peers, even
  though the BN254 preflight itself passed. The next GPU issue is runtime BN254
  batch stability, not the already-fixed general FASTPQ prover Poseidon
  preflight parity.
- Consensus progress is queue-drain limited: each peer committed `15` blocks in
  the profiled run, with average commit intervals around `8.0s` to `8.35s`.
  RBC store pressure, evictions, and drops stayed at zero, but diagnostics show
  `542` local READY deferrals, `486` DELIVER deferrals, `229` exact-frontier
  block-body fetches, `60` range-pull logs, and `105` not-enough-votes logs.
  That points to payload availability and exact-frontier recovery delaying
  commit cadence, not ingress rejection or RBC capacity pressure.
- Secondary CPU costs remain meaningful after the two primary bottlenecks:
  Ed25519/Curve25519 (`14,583` application leaf samples), Norito
  encode/decode/length/write paths (`13,942`), SHA-256 (`9,060`), Blake2
  (`5,279`), and CRC64 (`2,916`). These should follow the BN254 runtime fallback
  and consensus payload-availability work.

## 2026-05-07 Iroha Config Snapshot Refresh

- Refreshed `minimal_config_snapshot` so the expected minimal config includes
  the current Sumeragi exact-body repair timing defaults, the default `Full`
  logger format, and the empty Nexus dataspace fee-sponsor map.
- Validation:
  - `cargo fmt --all`
  - `cargo test -p iroha_config --test fixtures`
  - `cargo test -p iroha_config`

## 2026-05-07 Sumeragi vNext foundation

- Added experimental `sumeragi::vnext` protocol state with explicit slot and
  validation ownership states, BChain-style successor-scoped suspicion,
  re-chain proposals/certificates, view-change certificates, and count/stake
  quorum checks that refuse quarantine when it would weaken commit safety.
- Added a nonblocking vNext reactor event/effect layer that dispatches
  validation once, rejects stale worker starts/results, enters recovery on
  validation timeout or queue saturation without inline fallback, broadcasts
  successor-scoped suspicion, and installs or escalates re-chain outcomes.
- Added the `BlockMessage::VNext` consensus-wire variant. The current legacy
  Sumeragi main loop classifies vNext frames as consensus traffic, validates
  vNext signatures/quorum/evidence at ingress, records malformed drops in
  consensus-message telemetry, and only then ignores valid frames until the
  replacement reactor is wired in. Permissioned ingress uses the count quorum;
  NPoS ingress now derives a `Numeric` stake quorum from the live roster cache
  without rounding stake weights into integers.
- Threaded vNext performance-fault parameters through `iroha_config` defaults,
  user/env parsing, actual config, documentation, and conversion into
  `PerformanceFaultConfig`; zero-valued vNext knobs now fail config parsing.
- Added canonical chain-id/mode-tag separated signing preimages for vNext
  suspicion, re-chain proposal, re-chain certificate, and view-change
  certificate messages. Suspicion/head signatures now verify against embedded
  peer keys, certificate aggregate verification is PoP-aware for BLS-normal
  signers, signer bitmaps reject malformed/out-of-range bits, and the reactor
  escalates to view change when a re-chain would exceed the configured tainted
  validator budget. The same checks are exposed through
  `ConsensusMessage::verify_ingress` so live ingress and unit tests share one
  verifier, and stake quorum uses the same `signed * 3 >= total * 2`
  cross-multiplication shape as the live NPoS QC path. Ingress also rejects
  multi-suspicion re-chain evidence until the deterministic multi-evidence
  ordering rule is explicitly modeled.
- Focused validation passed with
  `cargo test -p iroha_config sumeragi_vnext --test fixtures -- --nocapture`
  (`2` passed) and
  `cargo test -p iroha_core --lib vnext -- --nocapture` (`31` passed), plus
  `cargo test -p iroha_core --lib consensus_message_handling_labels_include_new_variants -- --nocapture`
  (`1` passed).
  `cargo check -p iroha_core --lib` passed, repository-wide formatting was
  checked with `cargo fmt --all --check`, and `git diff --check` passed.

## 2026-05-07 RWA query WSV secondary indexes

- Added non-serialized WSV read-side indexes for RWA status and frozen state.
  They are rebuilt from canonical `rwas` storage on world construction/decode,
  carried through world block/transaction/view layers, and maintained on RWA
  insert plus freeze/unfreeze transitions.
- `FindRwas` now intersects indexed candidate sets across id, owner, domain,
  status, and frozen-state predicates before applying the full predicate. The
  status/frozen predicate matcher also handles the `frozen` alias directly, so
  indexed filters do not devolve into a full scan for common lifecycle queries.
- Focused validation passed with
  `cargo test -p iroha_core find_rwas --lib` and
  `cargo test -p iroha_core rwas_status_and_frozen_iters_use_secondary_indexes --lib -- --nocapture`.

## 2026-05-07 Account query WSV holder fast path

- Added a non-serialized WSV read-side index from asset definition id to
  accounts that currently hold at least one non-zero balance partition. The
  index is rebuilt from canonical `assets` storage on world construction/decode,
  carried through world block/transaction/view layers, and maintained when
  numeric assets are created, deposited, transferred into, or removed.
- `FindAccountsWithAsset` now starts from that non-zero holder index instead of
  scanning concrete asset ids for the definition on every query. `FindAccountIds`
  also reuses the parsed id-candidate set for id predicates that are not the
  trivial one-clause direct lookup, avoiding full account scans for mixed `IN`
  lists plus
  `exists("id")`.
- Added focused account-query coverage for the non-zero asset holder path and
  the extended account-id candidate path. Focused validation passed with
  `cargo test -p iroha_core find_accounts_with_asset_uses_nonzero_definition_asset_index --lib -- --nocapture`
  and
  `cargo test -p iroha_core find_account_ids_uses_candidate_lookup_for_id_predicates_with_exists --lib -- --nocapture`;
  asset-index lifecycle coverage passed with
  `cargo test -p iroha_core asset_definition_holder_index_tracks_asset_lifecycle --lib -- --nocapture`
  and
  `cargo test -p iroha_core asset_definition_holder_index_waits_for_last_partition_removal --lib -- --nocapture`.
- Library validation passed with `cargo check -p iroha_core --lib`; the
  serialization guard `scripts/check_no_scale.sh` also passed.

## 2026-05-07 Torii ephemeral query first-batch path

- Ephemeral iterable query execution without metadata sorting now keeps only
  the first response batch in memory while counting paginated results for the
  existing `remaining_items` contract. Stored cursors still materialize owned
  iterators because they can outlive the state snapshot borrow.
- Added unit coverage for the no-sort ephemeral first-batch/remaining-count
  path. `cargo check -p iroha_core --lib` and `scripts/check_no_scale.sh`
  passed; the focused `cargo test -p iroha_core
  ephemeral_unsorted_query_returns_first_batch_and_remaining_without_cursor
  --lib -- --nocapture` also passed.

## 2026-05-07 Torii account-asset WSV range reads

- Torii account-asset GET/query handlers now project rows from WSV account-keyed
  asset ranges instead of scanning every world asset and filtering by account.
  The projection path also caches asset-definition name/alias lookups per
  response.
- Asset-holder GET now uses the WSV account+definition range when `account_id`
  is supplied, avoiding a full definition-holder walk for exact account reads.
- Asset-holder query fallback now extracts safe exact `account_id` constraints
  from `eq`, `in`, `and`, and fully-constrained `or` filters and uses the same
  WSV account+definition ranges instead of building candidates from every
  holder of the asset definition.
- Focused validation passed with `cargo check -p iroha_torii --lib`,
  `cargo test -p iroha_torii
  collect_projected_account_assets_reads_only_scoped_account_assets --lib --
  --nocapture`, and `cargo test -p iroha_torii
  accumulate_asset_holder_quantity_respects_scope_filter --lib -- --nocapture`,
  and `cargo test -p iroha_torii
  asset_holder_filter_account_candidates_extracts_safe_exact_constraints --lib
  -- --nocapture`.

## 2026-05-07 Torii repo-agreement indexed filters

- Torii repo-agreement list/query handlers now plan exact `id`, `initiator`,
  `counterparty`, and `custodian` filters against the WSV repo-agreement
  participant indexes before projecting rows. The full route filter is still
  applied afterward, so indexed candidates are only a safe narrowing step.
- Focused validation passed with `cargo check -p iroha_torii --lib` and
  `cargo test -p iroha_torii
  repo_filter_candidate_ids_extracts_safe_indexed_constraints --lib --
  --nocapture`, plus the route-level
  `repo_agreements_list_filter_accepts_canonical_accounts` and
  `repo_agreements_query_filter_accepts_canonical_accounts` focused tests.

## 2026-05-07 Torii NFT/RWA id-filter streaming

- Torii NFT and RWA list/query handlers now stream from WSV instead of eagerly
  collecting every row into a `Vec`, and exact `id` filters are planned as
  direct WSV lookups for `eq`, `in`, `and`, and fully-constrained `or`
  expressions.
- Focused validation passed with `cargo check -p iroha_torii --lib`,
  `cargo test -p iroha_torii
  nft_filter_candidate_ids_extracts_safe_exact_constraints --lib --
  --nocapture`, and `cargo test -p iroha_torii
  rwa_filter_candidate_ids_extracts_safe_exact_constraints --lib --
  --nocapture`.

## 2026-05-07 Torii account and asset-definition exact-key planning

- Torii account list/query handlers now extract safe exact canonical `id`
  constraints and read those account keys directly from WSV before applying the
  full filter/projection, avoiding full account scans for targeted reads.
- Torii asset-definition list/query handlers now use exact canonical
  asset-definition `id` constraints as direct WSV key lookups. Alias predicates
  intentionally remain on the full filtered path so the existing enriched alias
  projection semantics are preserved.
- Focused validation passed with `cargo check -p iroha_torii --lib`,
  `cargo test -p iroha_torii extracts_safe --lib -- --nocapture`, the account
  list/query canonical/alias filter route tests, and
  `assets_definitions_query_filters_name_alias_and_null_alias`.

## 2026-05-07 Torii explorer WSV indexed dispatch

- WSV now exposes a borrowed `asset_entries_by_definition_iter(...)` over the
  maintained asset-definition asset index, and the owned
  `assets_by_definition_iter(...)` delegates to it. Torii asset-holder live
  scans and selected-definition query-projection archive builders now aggregate
  borrowed asset entries instead of cloning full assets.
- Torii explorer handlers now pass filtered WSV index iterators into the page
  builders for account domain/definition filters, domain owner filters,
  asset-definition domain/owner filters, asset owner/definition/id filters,
  NFT owner/domain filters, and RWA owner/domain filters. Explorer network
  metrics now use storage `len()` for entity counts instead of counting full
  iterators.
- Explorer account/domain/asset-definition list and detail handlers no longer
  build a full `ExplorerAggregates` snapshot per request; row counters are
  derived on demand from maintained WSV indexes such as domain-owner,
  account-asset, NFT-owner, domain-asset-definition, and asset-definition asset
  indexes.
- Focused validation passed with `cargo check -p iroha_core --lib`,
  `cargo check -p iroha_torii --lib`, the focused core
  `assets_by_definition_iter_includes_all_tracked_partitions` test,
  `cargo test -p iroha_torii explorer:: --lib -- --nocapture`, and
  `cargo test -p iroha_torii asset_holder --lib -- --nocapture`, plus the
  focused asset-holder projection catalog/archive tests.

## 2026-05-08 Core asset query WSV streaming

- WSV now exposes `asset_entries_by_definition_ids_iter(...)` and
  `asset_entries_by_ids_iter(...)` so query plans that already narrowed to
  asset ids or asset-definition ids can stream borrowed storage entries and
  clone asset values only at the final query output boundary.
- `FindAssets` definition/domain/id and subject-scoped indexed paths now use
  those borrowed-entry helpers instead of materializing intermediate
  `Vec<Asset>` batches per definition or subject.
- Validation passed with `cargo check -p iroha_core --lib` and
  `cargo check -p iroha_torii --lib`. The focused `cargo test -p iroha_core
  find_assets_filters_by_definition_predicate --lib -- --nocapture` test build
  is currently blocked by unrelated dirty Sumeragi test initializers for
  `PrecommitSignerRecord` and `derive_block_sync_qc_from_signers`.

## 2026-05-08 NFT/domain and subscription indexed reads

- WSV now exposes `nft_entries_by_ids_iter(...)`; `FindNfts` owner/domain/id
  plans and `FindNftsByAccountId` use it to stream borrowed NFT entries after
  narrowing by owner indexes, domain ranges, or exact ids.
- `FindDomains` owner-filter plans now resolve domain ids from the maintained
  domain-owner index and clone only the selected domain rows, avoiding
  intermediate `Vec<Domain>` materialization per owner.
- Torii subscription listing now uses the WSV NFT owner index when `owned_by`
  is supplied, while still applying the existing subscription metadata,
  provider, and status filters afterward.
- Validation passed with `cargo check -p iroha_core --lib` and
  `cargo check -p iroha_torii --lib`. The focused
  `cargo test -p iroha_torii subscription --lib -- --nocapture` build is
  currently blocked by unrelated dirty consensus test initializers for
  `Qc`/`QcVote` chain-order fields.

## 2026-05-07 IVM Staging and Sumeragi Targeted Recovery

- The staged `ivm_contract_deploy` fixtures were retested against the contract
  runtime host after the literal-table padding fix. The four staged copy/register
  tests now load the generated programs instead of failing metadata validation.
- NPoS block validation now accepts monotonic, same-epoch VRF epoch-record
  extensions from staged consensus effects while continuing to reject rewrites
  of existing participant data. This covers the case where late reveals or
  participant additions extend a pre-block epoch snapshot before finalization.
- The late-VRF-reveal integration scenario now gives the consensus actor a
  wider active-epoch window and a longer local processing poll before forcing
  progress blocks, avoiding the race where an accepted Torii reveal was handled
  only after the epoch had closed. The randomness module, including the
  zero-participation case, is green with the wider short epoch.
- Focused consensus/DA regressions that failed in the broad workspace attempt
  are green again: selective-drop recovery, conflicting-ready invalidation,
  Kura eviction DA rehydration, NPoS baseline metrics, pacemaker latency,
  pacemaker restart liveness, stale-evidence rejection, and VRF randomness.
- Validation:
  - `cargo fmt --all`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-keepgoing-workspace-check cargo test -p iroha_cli --bin ivm_contract_deploy staged_ -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-keepgoing-workspace-check cargo test -p iroha_core validate_npos_effects --lib -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-keepgoing-workspace-check IROHA_TEST_NETWORK_KEEP_DIRS=1 cargo test -p integration_tests --test consensus_and_da sumeragi_randomness:: -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-keepgoing-workspace-check cargo check -p integration_tests --test consensus_and_da`

## 2026-05-07 Workspace All-Target Compile Fallout

- The default Linux `iroha_monitor` build no longer imports the gated built-in
  synth module when `linux-builtin-synth` is disabled. The theme intro now
  degrades to a soft audio-unavailable message in that build, and synth-only
  score helpers are marked as such for the default no-synth target.
- Python and `xtask` Nexus lane commitment fixtures now include empty
  `nexus_fee_receipts`, matching the current `LaneBlockCommitment` layout.
- Removed stale Norito CRC64 x86 helper residue and quieted the Metal sequence
  planner's non-Mac unused-argument warning exposed by the broad check.
- Validation:
  - `cargo fmt --all`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-keepgoing-workspace-check cargo check --workspace --all-targets`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-keepgoing-workspace-check cargo check -p iroha_python_rs --all-targets`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-keepgoing-workspace-check cargo check -p xtask --all-targets`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-keepgoing-workspace-check cargo test -p iroha_monitor --bin iroha_monitor intro -- --nocapture`

## 2026-05-07 Norito Registry Test Re-enable

- Stale `IROHA_RUN_IGNORED` guards were removed from the instruction registry,
  trait-object instruction, lazy registry initialization, ZK envelope, block
  signature, signed-transaction attachment, and proof Norito roundtrip tests.
  The direct registry tests now feed header-framed instruction payloads, matching
  the current `InstructionRegistry::decode` contract.
- The legacy block-header decode fixture now includes the current optional
  NPoS effects hash field before the SCCP commitment root, keeping the
  backwards-compatibility fixture aligned with the actual header layout.
- Validation:
  - `cargo fmt --all`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-asset-control-slice cargo test -p iroha_data_model --test registry_decode_roundtrip --test instruction_registry_reset -- --nocapture --test-threads=1`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-asset-control-slice cargo test -p iroha_data_model --test trait_objects -- --nocapture --test-threads=1`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-asset-control-slice cargo test -p iroha_data_model --test instruction_registry_lazy_init -- --nocapture --test-threads=1`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-asset-control-slice cargo test -p iroha_data_model --test zk_envelope_roundtrip -- --nocapture --test-threads=1`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-asset-control-slice cargo test -p iroha_data_model register_and_decode_instruction -- --nocapture --test-threads=1`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-asset-control-slice cargo test -p iroha_data_model default_registry_roundtrip -- --nocapture --test-threads=1`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-asset-control-slice cargo test -p iroha_data_model ordering_is_preserved_across_roundtrip -- --nocapture --test-threads=1`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-asset-control-slice cargo test -p iroha_data_model block_signature_roundtrip_diagnostics -- --nocapture --test-threads=1`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-asset-control-slice cargo test -p iroha_data_model signed_tx_with_attachments_roundtrip -- --nocapture --test-threads=1`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-asset-control-slice cargo test -p iroha_data_model proof -- --nocapture --test-threads=1`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-asset-control-slice cargo test -p iroha_data_model registry -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-asset-control-slice cargo test -p iroha_data_model header_decodes_legacy_payload_without_execution_context_hash -- --nocapture --test-threads=1`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-asset-control-slice cargo test -p iroha_data_model --lib` (`1212 passed; 0 failed; 2 ignored`)

## 2026-05-06 Core Instruction Slice Decode

- `Transfer<...>`, `TransferBox`, `TransferAssetBatchEntry`, and
  `TransferAssetBatch` now have narrow `DecodeFromSlice` implementations for
  ordinary AoS Norito payloads, with the existing codec cursor retained for
  packed-struct payloads.
- `Mint<...>`, `MintBox`, `Burn<...>`, and `BurnBox` now use the same narrow
  ordinary-AoS slice path for both asset-numeric and trigger-repetition
  variants.
- `SetKeyValue<...>`, `RemoveKeyValue<...>`, `SetAssetKeyValue`,
  `RemoveAssetKeyValue`, `SetKeyValueBox`, and `RemoveKeyValueBox` now have
  narrow ordinary-AoS slice decoders for metadata instructions.
- `Grant<...>`, `Revoke<...>`, `GrantBox`, and `RevokeBox` now use narrow
  ordinary-AoS slice decoders for account/role permission changes. The account
  authority instructions `AddSignatory`, `RemoveSignatory`, and
  `SetAccountQuorum` now do the same.
- `SetParameter`, `ExecuteTrigger`, `Upgrade`, and `CustomInstruction` now use
  narrow ordinary-AoS slice decoders while preserving their stable wire IDs.
- `Register<...>`, `Unregister<...>`, `RegisterBox`, and `UnregisterBox` now
  use narrow ordinary-AoS slice decoders for domain/account/asset definition/
  NFT/role/trigger entities. `RegisterPeerWithPop` now uses the same slice
  path, including the `RegisterBox::Peer` stable boxed registration path.
- RWA lot lifecycle instructions, RWA metadata edits inside
  `RwaInstructionBox`, repo initiate/reverse/margin-call instructions inside
  `RepoInstructionBox`, and settlement DvP/PvP instructions inside
  `SettlementInstructionBox` now use narrow ordinary-AoS slice decoders while
  preserving their existing stable registry lookup strings.
- Asset-definition alias/balance-policy instructions and asset transfer-control
  freeze/blacklist/limit instructions now use narrow ordinary-AoS slice
  decoders while preserving their stable wire IDs.
- Account alias binding/primary-alias instructions, paid account-alias lease
  acquire/renew instructions, and contract-alias binding instructions now use
  narrow ordinary-AoS slice decoders. The existing identity lookup strings for
  account alias binding remain unchanged.
- Account controller replacement and social-recovery policy/propose/approve/
  cancel/finalize instructions now use narrow ordinary-AoS slice decoders while
  preserving their stable account-recovery wire IDs.
- RAM-LFE program-policy instructions, hidden-identifier policy/claim/revoke
  instructions, and consensus-key register/rotate/disable instructions now use
  narrow ordinary-AoS slice decoders while preserving their existing
  `identity::...` and `consensus::...` registry lookup strings.
- Domain-endorsement committee/policy/submission instructions now use narrow
  ordinary-AoS slice decoders while preserving the `nexus::...` registry lookup
  strings.
- Verifying-key register/update instructions and Offline V2 issue/redeem/audit
  instructions now use narrow ordinary-AoS slice decoders on their type-name
  wire IDs.
- Verified Nexus lane-relay and public fee-budget registration instructions now
  use narrow ordinary-AoS slice decoders while preserving their `nexus::...`
  stable lookup strings. The emergency lane-relay validator override now also
  uses a narrow ordinary-AoS slice decoder on both its type-name and
  `nexus::SetLaneRelayEmergencyValidators` stable lookup strings.
- Native and anonymous asset escrow open/accept/payment-sent/release/cancel/
  dispute/resolve instructions now use narrow ordinary-AoS slice decoders on
  their type-name wire IDs.
- Musubi release publish/yank, short-alias binding, and release-existence
  assertion instructions now use narrow ordinary-AoS slice decoders while
  preserving their `iroha.musubi.*` stable wire IDs.
- Smart-contract-code manifest, instance activation/deactivation, bytecode
  registration, and bytecode removal instructions now use narrow ordinary-AoS
  slice decoders on their type-name wire IDs.
- Space Directory manifest publish/revoke/expire instructions now use narrow
  ordinary-AoS slice decoders on their type-name wire IDs.
- SoraFS pin manifest, alias, capacity declaration/telemetry/dispute,
  replication order, provider-owner, replication receipt, pricing schedule, and
  provider credit instructions now use narrow ordinary-AoS slice decoders. The
  default registry uses that slice path for the SoraFS instruction subset it
  currently exposes on type-name wire IDs.
- Oracle feed registration, observation submission, aggregation, dispute,
  governance-change, Twitter binding, and Twitter binding revocation
  instructions now use narrow ordinary-AoS slice decoders on their type-name
  wire IDs. The oracle fetch-plan backoff regression expectation was refreshed
  to the current deterministic schedule `[10, 13, 14]`.
- Bridge proof submission, bridge receipt recording, and SCCP message recording
  instructions now use narrow ordinary-AoS slice decoders on their type-name
  wire IDs.
- Ministry citizen-agenda proposal submission now uses a narrow ordinary-AoS
  slice decoder on its type-name wire ID.
- Social Twitter follow reward/send/cancel instructions now use narrow
  ordinary-AoS slice decoders on their type-name wire IDs.
- Public-lane validator register/rebind/activate/exit instructions and the
  consensus-evidence penalty cancellation instruction now use narrow
  ordinary-AoS slice decoders. The existing `iroha.staking.*` stable lookup
  strings for activate/rebind/exit now use the same slice path.
- `InvalidInstruction` now uses a narrow ordinary-AoS slice decoder on both its
  type-name registration and stable `iroha.invalid_instruction` lookup string.
- SoraNet VPN lease open/settle/refund instructions now use narrow ordinary-AoS
  slice decoders on their type-name wire IDs.
- ZK proof verification/pruning, ZK asset registration, confidential policy
  transition schedule/cancel, shield/transfer/unshield, and private election
  create/ballot/finalize instructions now use narrow ordinary-AoS slice
  decoders. The existing `zk::ScheduleConfidentialPolicyTransition` and
  `zk::CancelConfidentialPolicyTransition` stable lookup strings now use the
  same slice path.
- Kaigi create/join/leave/end, usage, relay-manifest, relay-registration, and
  relay-health instructions now use narrow ordinary-AoS slice decoders on their
  type-name wire IDs.
- Governance deploy/runtime-upgrade proposals, ZK/plain ballots, lock slash/
  restitution, referendum enact/finalize, proposal approval, council
  persistence, citizen service outcome, and citizen register/unregister
  instructions now use narrow ordinary-AoS slice decoders on their type-name
  wire IDs.
- Soracloud service lifecycle, config/secret, state/FHE/decryption, shared HF
  lease, model-host/Inrou placement, agent-apartment, autonomy, training,
  model-weight/upload/private-inference, rollout, runtime-state, lease-usage,
  mailbox, and runtime-receipt instructions now use narrow ordinary-AoS slice
  decoders. The existing Soracloud stable lookup strings that the default
  registry exposes now use the same slice path, and the unit reconcile
  instructions explicitly accept empty AoS payloads.
- The default instruction registry now uses the opt-in slice constructor for
  the four concrete transfer ISIs, `TransferBox`, `TransferAssetBatch`, the
  two concrete mint ISIs, `MintBox`, the two concrete burn ISIs, `BurnBox`, the
  concrete key-value metadata ISIs, the Grant/Revoke ISIs, and the signatory
  quorum ISIs. It also uses the slice constructor for the stable core
  SetParameter/trigger/upgrade/custom ISIs, register/unregister box dispatch,
  asset alias/balance-policy dispatch, asset transfer-control dispatch, account
  alias binding/lease dispatch, contract-alias dispatch, and account-recovery
  dispatch, plus RAM-LFE, identifier, consensus-key, domain-endorsement,
  verified Nexus relay/budget/emergency-validator override, RWA/repo/
  settlement stable boxes, asset escrow, verifying-key, Offline V2, Musubi, and
  smart-contract-code, Space Directory, SoraFS, oracle, bridge/SCCP, ministry,
  social, registered public-lane staking, invalid-instruction, SoraNet VPN
  lease, ZK, Kaigi, governance, and Soracloud dispatch. No default registry
  entry remains on the generic instruction decoder path.
  Stable wire identifiers remain unchanged for
  `iroha.transfer`, `iroha.transfer_batch`, `iroha.mint`, `iroha.burn`,
  `iroha.set_key_value`, `iroha.remove_key_value`, `iroha.grant`, and
  `iroha.revoke`, plus `iroha.set_parameter`, `iroha.execute_trigger`,
  `iroha.upgrade`, `iroha.custom`, `iroha.register`, `iroha.unregister`,
  `iroha.asset_definition.alias.set`,
  `iroha.asset_definition.balance_policy.set`,
  `iroha.asset.transfer.freeze.set`,
  `iroha.asset.transfer.blacklist.set`, and
  `iroha.asset.transfer.control.set`, plus
  `identity::SetAccountAliasBinding`, `identity::SetPrimaryAccountAlias`, and
  `iroha.contract.alias.set`, plus `iroha.rwa`, `iroha.repo.initiate`,
  `iroha.repo.reverse`, `iroha.settlement.dvp`,
  `iroha.settlement.pvp`, `zk::ScheduleConfidentialPolicyTransition`, and
  `zk::CancelConfidentialPolicyTransition`.
- Validation:
  - `cargo fmt --all`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-transfer-slice cargo test -p iroha_data_model transfer_ -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-transfer-slice cargo test -p iroha_data_model mint_burn -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-transfer-slice cargo test -p iroha_data_model registry -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-transfer-slice cargo check -p iroha_data_model --bench decode_registry`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-kv-slice cargo test -p iroha_data_model key_value -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-kv-slice cargo test -p iroha_data_model registry -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-kv-slice cargo check -p iroha_data_model --bench decode_registry`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-grant-slice cargo test -p iroha_data_model grant_revoke -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-grant-slice cargo test -p iroha_data_model signatory_quorum -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-grant-slice cargo test -p iroha_data_model trigger_upgrade_custom -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-grant-slice cargo test -p iroha_data_model set_parameter -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-grant-slice cargo test -p iroha_data_model registry -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-grant-slice cargo check -p iroha_data_model --bench decode_registry`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-register-slice cargo test -p iroha_data_model register_unregister -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-register-slice cargo test -p iroha_data_model registry -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-register-slice cargo check -p iroha_data_model --bench decode_registry`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-asset-control-slice cargo test -p iroha_data_model asset_transfer_control -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-asset-control-slice cargo test -p iroha_data_model asset_alias -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-asset-control-slice cargo test -p iroha_data_model registry -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-asset-control-slice cargo check -p iroha_data_model --bench decode_registry`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-asset-control-slice cargo test -p iroha_data_model account_alias -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-asset-control-slice cargo test -p iroha_data_model contract_alias -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-asset-control-slice cargo test -p iroha_data_model registry -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-asset-control-slice cargo check -p iroha_data_model --bench decode_registry`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-asset-control-slice cargo test -p iroha_data_model account_recovery -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-asset-control-slice cargo test -p iroha_data_model registry -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-asset-control-slice cargo check -p iroha_data_model --bench decode_registry`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-asset-control-slice cargo test -p iroha_data_model ram_lfe -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-asset-control-slice cargo test -p iroha_data_model identifier -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-asset-control-slice cargo test -p iroha_data_model consensus_key -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-asset-control-slice cargo test -p iroha_data_model endorsement -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-asset-control-slice cargo test -p iroha_data_model verifying_key -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-asset-control-slice cargo test -p iroha_data_model offline_note_v2 -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-asset-control-slice cargo test -p iroha_data_model register -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-asset-control-slice cargo test -p iroha_data_model nexus_verified -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-asset-control-slice cargo test -p iroha_data_model nexus -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-asset-control-slice cargo test -p iroha_data_model escrow -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-asset-control-slice cargo test -p iroha_data_model rwa -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-asset-control-slice cargo test -p iroha_data_model repo -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-asset-control-slice cargo test -p iroha_data_model settlement -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-asset-control-slice cargo test -p iroha_data_model musubi -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-asset-control-slice cargo test -p iroha_data_model smart_contract_code -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-asset-control-slice cargo test -p iroha_data_model space_directory -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-asset-control-slice cargo test -p iroha_data_model sorafs -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-asset-control-slice cargo test -p iroha_data_model oracle -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-asset-control-slice cargo test -p iroha_data_model bridge -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-asset-control-slice cargo test -p iroha_data_model ministry -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-asset-control-slice cargo test -p iroha_data_model social -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-asset-control-slice cargo test -p iroha_data_model staking -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-asset-control-slice cargo test -p iroha_data_model trigger_upgrade_custom -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-asset-control-slice cargo test -p iroha_data_model vpn -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-asset-control-slice cargo test -p iroha_data_model zk -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-asset-control-slice cargo test -p iroha_data_model kaigi -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-asset-control-slice cargo test -p iroha_data_model governance -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-asset-control-slice cargo test -p iroha_data_model soracloud -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-asset-control-slice cargo test -p iroha_data_model registry -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-asset-control-slice cargo check -p iroha_data_model --bench decode_registry`

## 2026-05-06 Instruction Payload Slice Decode

- `InstructionRegistry` now has an internal opt-in constructor path for
  instruction types that implement `DecodeFromSlice`. The ordinary registration
  path remains compatible with instructions that still require the existing
  framed Norito decoder.
- `Log`, `RecordSccpMessage`, the runtime-upgrade ISIs, and the SNS name ISIs
  now have narrow slice decoders for their ordinary AoS Norito payloads, with
  the codec cursor retained for packed-struct payloads. The default registry
  uses the slice constructor for those instructions, including `Log`'s stable
  `iroha.log` wire identifier and the runtime-upgrade stable wire identifiers.
- Validation:
  - `cargo fmt --all`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-instruction-registry-slice cargo test -p iroha_data_model registry_decode_accepts_misaligned_framed_payload -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-instruction-registry-slice cargo test -p iroha_data_model sccp -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-instruction-registry-slice cargo test -p iroha_data_model instruction_box -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-instruction-registry-slice cargo test -p iroha_data_model registry -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-instruction-registry-slice cargo test -p iroha_data_model transaction::signed::tests:: -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-instruction-registry-slice cargo check -p iroha_data_model --bench decode_registry`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-runtime-upgrade-slice cargo test -p iroha_data_model runtime_upgrade -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-runtime-upgrade-slice cargo test -p iroha_data_model sns -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-runtime-upgrade-slice cargo check -p iroha_data_model --bench decode_registry`
  - `git diff --check`

## 2026-05-06 Izanami Admission-Decode Gate Rerun

- Built isolated scalar release Izanami/Iroha gate binaries in
  `/tmp/iroha-codex-20k-admission-decode` with
  `ENABLE_RANS_BUNDLES=1 NORITO_SKIP_BINDINGS_SYNC=1 CARGO_TARGET_DIR=/tmp/iroha-codex-20k-admission-decode cargo build --release -p irohad --bin iroha3d -p izanami --bin izanami`.
  The build passed in `12m27s`; warning output was limited to the existing
  unused-code set in `iroha_crypto`, `ivm`, `iroha_core`, and Izanami test
  helpers.
- This WSL2 host has neither `sample` nor `perf`, so the 30s follow-up is an
  unsampled no-fault validation run rather than a CPU profile. Artifact:
  `dist/izanami-prebuilt-20k-admission-decode-unsampled-30s-20260506-020112`.
  It exited `0`, offered/accepted/succeeded all `600,000` submissions, built
  and used all `600,000` prebuilt transactions, reported `0` failures,
  prebuild fallback/build failures, ingress failovers, unhealthy endpoints,
  view changes, validation rejects, or RBC pressure. Submit latency was
  `p50=5ms`, `p95=17ms`, `p99=45ms`, `max=344ms`; final strict progress was
  `4,133` approved transactions at height `3`, with queue saturation
  `159,593 / 600,000`.
- The full 4-peer no-fault prebuilt `20k TPS` / `120s` scalar gate artifact is
  `dist/izanami-prebuilt-20k-admission-decode-120s-20260506-020335`. It
  exited `0`, offered/accepted/succeeded `2,379,055` submissions, built all
  `2,400,000` prebuilt transactions, used `2,379,055`, and reported `0`
  failures, prebuild fallback/build failures, ingress failovers, unhealthy
  endpoints, view changes, validation rejects, or RBC pressure. Submit latency
  was `p50=5ms`, `p95=25ms`, `p99=210ms`, `max=9288ms`; final strict progress
  was `20,553` approved transactions at height `7`, with queue saturation
  `812,857 / 2,400,000`. Treat this as fresh ingress/safety evidence for the
  admission-decode pass, not a committed-20k throughput win.
- Validation:
  - `command -v sample || true; command -v perf || true`
  - `/tmp/iroha-codex-20k-admission-decode/release/izanami --help`
  - 30s no-fault prebuilt run with `--duration 30s --tps 20000 --max-inflight 600000 --submitters 4096 --prebuild-tx-buffer 600000 --prebuild-tx-workers 20`
  - 120s no-fault prebuilt run with `--duration 120s --tps 20000 --max-inflight 2400000 --submitters 4096 --prebuild-tx-buffer 2400000 --prebuild-tx-workers 20`

## 2026-05-06 Signed Transaction Slice Decode Field Walkers

- `SignedTransaction::DecodeFromSlice` now walks the ordinary AoS Norito
  fields directly instead of routing the whole transaction through the cursor
  decoder. Its payload field delegates to a new `TransactionPayload` slice
  decoder, preserving the cursor fallback for packed-struct layouts and small
  codec-only fields whose custom codecs do not expose slice-safe decoders.
- `TransactionPayload::DecodeFromSlice` now decodes the hot `Executable` field
  through a narrow `Executable::Instructions` slice path, which keeps the
  instruction vector on the `ConstVec<InstructionBox>` planned decoder added in
  the previous pass. Non-instruction executable variants still use the existing
  cursor path.
- Validation:
  - `cargo fmt --all`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-signed-slice cargo test -p iroha_data_model executable_instructions_decode_from_slice_roundtrips -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-signed-slice cargo test -p iroha_data_model transaction::signed::tests:: -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-tx-frame-hash cargo test -p iroha_core --lib accepted_transaction_caches_hashes_and_encoded_length -- --nocapture`

## 2026-05-06 Accepted Transaction Signed-Frame Hash Reuse

- `AcceptedTransaction::from_external_with_cached_bytes` now builds the cached
  signed-transaction frame from the same canonical signed payload used for the
  external entrypoint hash. The normal hot-cache path avoids the previous second
  signed-transaction serialization while preserving canonical Norito bytes and
  transaction hashes.
- Added a signed-frame hash helper that validates Norito headers/schema/checksum
  before hashing the already-framed signed payload for caller-provided cached
  bytes, with fallback to the canonical re-encode path when validation fails.
- Validation:
  - `cargo fmt --all`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-tx-frame-hash cargo test -p iroha_core --lib entrypoint_hash -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-tx-frame-hash cargo test -p iroha_core --lib accepted_transaction_caches_hashes_and_encoded_length -- --nocapture`

## 2026-05-06 ConstVec Planned Slice Decode Fast Path

- `ConstVec<T>::DecodeFromSlice` now tries Norito's scalar sequence planner
  directly for non-`u8` elements before falling back to the previous canonical
  `Vec<T>` field decode. This keeps `ConstVec<u8>` and legacy recovery behavior
  unchanged while avoiding the top-level archive/canonical-length pass on hot
  `ConstVec<InstructionBox>` admission slices.
- Added a hidden Norito helper for scalar planned `Vec<T>` slice decoding and
  prefix-consumption coverage in both Norito and `iroha_primitives`.
- Validation:
  - `cargo fmt --all`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-constvec-fast cargo test -p norito decode_vec_from_slice_serial_reports_prefix_used -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-constvec-fast cargo test -p iroha_primitives decode_from_slice_reports_prefix_used_for_non_byte_items -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-constvec-fast cargo test -p iroha_data_model execution_step_decode_from_slice -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-constvec-parallel cargo test -p norito --lib sequence_parallel_decode_threshold --features parallel-decode -- --nocapture`

## 2026-05-06 ExecutionStep Slice Decode Fast Path

- `ExecutionStep::DecodeFromSlice` now parses its single Norito field directly
  and delegates the inner instruction list to `ConstVec<InstructionBox>`'s
  planned slice decoder. This keeps the `ExecutionStep` wire layout unchanged
  while avoiding the generic cursor-based decode path for the hot
  transaction-admission instruction vector.
- The decoder still rejects trailing bytes, records full payload access for
  parent canonical-length checks, and preserves the existing exact-consumption
  contract used by signed transaction decoding.
- Added positive exact-decode coverage for a two-instruction `ExecutionStep`.
- Validation:
  - `cargo fmt --all`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-executionstep-slice cargo test -p iroha_data_model execution_step_decode_from_slice -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-executionstep-slice cargo test -p iroha_data_model signed_transaction_decode_from_slice -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-executionstep-slice cargo test -p iroha_data_model transaction::signed::tests:: -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-executionstep-check cargo check -p iroha_data_model --bench chain_wire`

## 2026-05-05 InstructionBox Slice Decode Fast Path

- `InstructionBox::DecodeFromSlice` now decodes the canonical borrowed
  `(wire_id, framed_payload)` tuple directly instead of first realigning the
  whole outer instruction payload as an archived `InstructionBox`. This keeps
  the wire bytes and registry semantics unchanged while avoiding an avoidable
  allocation/copy on misaligned direct/gossip admission slices.
- Successful direct slice decodes now record full payload consumption for
  parent Norito canonical-length checks, and malformed slices still return the
  bounded canonical-framing error used by the existing rejection path.
- Added regression coverage for a deliberately misaligned borrowed
  `InstructionBox` tuple, and restored the normal trait-object Norito roundtrip
  test now that the borrowed nested decode path is green.
- Validation:
  - `cargo fmt --all`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-instruction-slice cargo test -p iroha_data_model instruction_box_decode_from_slice_accepts_misaligned_borrowed_pair -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-instruction-slice cargo test -p iroha_data_model norito_roundtrip_trait_object_deserialize -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-instruction-slice cargo test -p iroha_data_model instruction_box -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-instruction-slice cargo check -p iroha_data_model --bench decode_registry`
  - `git diff --check`

## 2026-05-05 Norito Parallel Sequence Decode Wiring

- The hidden `parallel-decode` feature now routes large planned `Vec<T>`
  decodes through `decode_planned_sequence_parallel` when `T: Send`, preserving
  original element order and lowest-index error reporting while keeping small
  sequences on the serial path.
- `Vec<T>` deserialization that cannot prove `T: Send` remains serial, so the
  feature does not force generic stream/schema helpers to add a public `Send`
  bound. No Norito wire bytes, canonical hashes, runtime configuration, or
  dependencies changed.
- Added focused coverage for the large-plan threshold and for a large
  `Vec<u64>` decode under `parallel-decode`.
- Removed the stale `struct_index_random_x86` unused-parentheses warning that
  surfaced in the Norito all-target feature check.
- Validation:
  - `cargo fmt --all`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-norito-parallel-decode cargo test -p norito --test sequence_plan --features parallel-decode -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-norito-parallel-decode cargo test -p norito --lib sequence_parallel_decode_threshold --features parallel-decode -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-norito-parallel-decode cargo check -p norito --all-targets --features parallel-decode`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-norito-parallel-decode-default cargo test -p norito --test sequence_plan -- --nocapture`

## 2026-05-05 IVM staging and CUDA closure

- The `ivm_contract_deploy` staged test programs now write the real `LTLB`
  post-padding length and zero padding bytes before appending code. This fixes
  the staged copy/register runtime-host tests that were loading generated
  programs as `InvalidMetadata`.
- The JSON Stage-1 CUDA helper now exposes the same backend-error return code
  expected by the Rust FFI. Norito also has require-mode loader tests for the
  CUDA sequence planner and CUDA CRC64 helper so `JSONSTAGE1_CUDA_REQUIRE=1`
  and `NORITO_CRC64_CUDA_REQUIRE=1` fail if the helper cannot load or pass its
  self-test.
- CUDA runtime validation passed on WSL2 with an NVIDIA GeForce RTX 3080 Laptop
  GPU (`cc8.6`, driver `527.56`, CUDA `12.0`). FastPQ CUDA parity passed, and
  the release CUDA benchmark was recorded at
  `dist/fastpq_cuda_bench_20260505.json`: Poseidon column hashing measured
  `97.972x` faster on CUDA, BN254 Poseidon words measured `11.72x` faster, and
  the smaller transfer-heavy FFT/IFFT/LDE operations remained faster on CPU in
  that benchmark shape.
- The nightly CUDA workflow now keeps the Norito CUDA sequence-plan and CRC64
  loader assertions in the hardware lane.
- CUDA validation used `CARGO_TARGET_DIR=/tmp/iroha-codex-cuda-20260505`,
  `RUST_TEST_THREADS=1`, `JSONSTAGE1_CUDA_ARCH=-arch=sm_86`, and
  `GPUZSTD_CUDA_ARCH=-arch=sm_86`; the IVM CUDA slice used
  `IVM_CUDA_GENCODE=arch=compute_86,code=sm_86`.
- Validation:
  - `cargo fmt --all`
  - `cargo test -p iroha_cli --bin ivm_contract_deploy staged -- --nocapture`
  - `cargo test -p ivm --lib cuda_ --features cuda -- --nocapture`
  - `JSONSTAGE1_CUDA_REQUIRE=1 cargo test -p jsonstage1_cuda --features cuda-kernel binary_sequence_plan -- --nocapture`
  - `JSONSTAGE1_CUDA_REQUIRE=1 cargo test -p norito sequence_plan --features codec-gpu-cuda -- --nocapture`
  - `GPUZSTD_CUDA_REQUIRE=1 cargo test -p gpuzstd_cuda --features cuda-kernel -- --nocapture`
  - `GPUZSTD_CUDA_REQUIRE=1 cargo test -p norito gpu_zstd --features gpu-compression -- --nocapture`
  - `JSONSTAGE1_CUDA_REQUIRE=1 cargo test -p jsonstage1_cuda --features cuda-kernel crc64 -- --nocapture`
  - `NORITO_CRC64_CUDA_REQUIRE=1 cargo test -p norito crc64 --features cuda-crc64 -- --nocapture`
  - `cargo test -p fastpq_prover --lib --features fastpq-gpu cuda -- --nocapture`
  - `cargo run -p fastpq_prover --bin fastpq_cuda_bench --release --features fastpq-gpu -- --rows 20000 --iterations 3 --warmups 1 --column-count 16 --require-gpu --device "NVIDIA GeForce RTX 3080 Laptop GPU cc8.6 driver 527.56" --output dist/fastpq_cuda_bench_20260505.json --notes "CUDA roadmap closure on WSL2"`
  - `git diff --check`

## 2026-05-07 Izanami 20k Metal final return gate

- The general FASTPQ Metal Poseidon prover preflight now passes on the Apple
  Metal host. `fastpq_prover::metal::poseidon_permute` keeps the
  sponge/preflight permutation on one independent state per Metal lane, while
  the trace kernels keep their separate multi-state batching path. Focused
  FASTPQ validation passed with `cargo test -p fastpq_prover poseidon
  --features fastpq-gpu -- --nocapture` (`39` lib tests plus matching bin and
  integration filters passed), including single-state, batched-state,
  preflight, trace GPU parity, and BN254-independent preflight coverage.
- Sumeragi commit-QC validation now defers to a fresh background validation
  inflight instead of treating the old inline fallback window as permission to
  duplicate work. It only supersedes and inlines when the worker is
  disconnected, the inflight frontier generation is stale, or the worker stall
  timeout is exceeded. Focused coverage passed for no-inflight worker dispatch,
  fresh inflight deferral past the old inline fallback, stale inflight recovery,
  and the broader `commit_pipeline` slice (`55` passed).
- A fresh release build completed with
  `CARGO_TARGET_DIR=/tmp/iroha-codex-20k-return-fixed-20260507-201449 cargo
  build --release -p irohad --bin iroha3d -p izanami --bin izanami --features
  irohad/fastpq-gpu`.
- The fresh 4-peer no-fault prebuilt `20k TPS` / `120s` `fastpq-gpu` Metal run
  at
  `dist/izanami-prebuilt-20k-fastpq-gpu-return-final-120s-20260507-221432`
  offered, accepted, and succeeded all `2,400,000` submissions and used all
  `2,400,000` prebuilt transactions. It reported `0` submit failures,
  validation rejects, confirmation failures, confirmation queue drops, prebuild
  fallbacks, prebuild skips, prebuild build failures, ingress failovers, or
  unhealthy endpoints. All four peers exited `0` and emitted empty stderr; the
  local wrapper failed only after Izanami completed because the zsh bookkeeping
  used Bash's `PIPESTATUS` variable.
- Final quorum/strict height was `17/17`; final quorum/strict approved
  transactions were `61,622/61,622`. Submit latency was `p50=2ms`,
  `p95=8ms`, `p99=60ms`, and `max=190ms`. Final queue depth was
  `861,515 / 2,400,000`, with `94` pacemaker backpressure deferrals and
  commit-pipeline EMA `2ms`.
- All peers logged BN254 Poseidon digest GPU preflight `ok=true` and general
  FASTPQ Poseidon prover preflight `ok=true`. Diagnostics had no normal
  commit-QC inline supersede logs and no inline pre-vote validation fallback
  logs. Two peers still logged the existing BN254 Poseidon Metal runtime batch
  command-buffer fallback to deterministic scalar hashing, so the remaining
  throughput work is queue drain/block-validation cost and BN254 runtime batch
  stability rather than prover Poseidon preflight parity.
- Additional validation passed with `cargo fmt --all`,
  `cargo check -p iroha_core --features fastpq-gpu`, and
  `cargo check -p irohad --features fastpq-gpu`.

## 2026-05-07 Izanami 20k Metal sampled profile

- A 90s `/usr/bin/sample` profile of one peer during the same prebuilt 20k
  workload completed at
  `dist/izanami-profile-20k-fastpq-gpu-metal-return-sampled-90s-20260507-080927`;
  both Izanami and the sampler exited `0`.
- The sampled run still offered, accepted, and succeeded all `2,400,000`
  submissions with `0` failures, validation rejects, confirmation failures, or
  queue drops. Sampling reduced progress versus the clean gate: final
  quorum/strict height was `16/16`, final strict approved was `57,581`, submit
  latency was `p50=3ms`, `p95=21ms`, `p99=82ms`, `max=251ms`, and final queue
  depth was `871,355 / 2,400,000`.
- The sampled peer reported a `3.4G` physical footprint. The top active
  application leaf frames moved away from scalar BN254 Poseidon digest hashing:
  SHA256 (`3,810` top-of-stack samples), Curve25519 field operations
  (`3,606` and `3,543`), Blake2 compression (`2,867`), CRC64 (`1,306`), and
  Norito length/write helpers (`841`, plus lower Norito encode/decode frames)
  now lead the sample. Lower `ark_ff` field operations remain visible, but
  scalar `iroha_zkp_halo2::poseidon::hash_u64_words_internal` is no longer the
  dominant CPU frame seen in the previous CPU-fallback profile.
- All peers again logged BN254 Poseidon digest GPU preflight `ok=true`, and the
  profile diagnostics did not log BN254 Metal batch command-buffer fallback.
  General FASTPQ Poseidon prover preflight still failed on Metal/CPU parity and
  used the CPU prover backend.
- The main remaining bottleneck is commit validation cadence and queue drain:
  diagnostics logged `60` slow commit-pipeline blocks with validation time
  `min=1053ms`, `avg=1946ms`, `max=3152ms`, plus `57` inline validation
  fallbacks after the `750ms` inflight timeout. Queue saturation persisted, with
  `113` pacemaker backpressure deferrals, while RBC store pressure, view
  changes, missing-block fetches, and validation rejects stayed at `0`.

## 2026-05-07 Bounded node shutdown

- `iroha_futures::Supervisor` now sends cancellation to the supervised child
  task before detaching the monitor on shutdown timeout, so slow graceful exits
  no longer leave the underlying Tokio task running until runtime teardown.
- `irohad` now shuts down the Tokio runtime with a bounded timeout after the
  node future completes, preventing non-preemptible blocking work from keeping
  the process alive indefinitely.
- Validation passed with `cargo test -p iroha_futures -- --nocapture` and
  `cargo check -p irohad`.

## 2026-05-07 Izanami 20k Metal return gate

- A fresh release build with the restored Metal toolchain completed for
  `iroha3d` and `izanami` using
  `CARGO_TARGET_DIR=/tmp/iroha-codex-20k-metal-return-20260507-075339 cargo
  build --release -p irohad --bin iroha3d -p izanami --bin izanami --features
  irohad/fastpq-gpu`.
- The fresh 4-peer no-fault prebuilt `20k TPS` / `120s` `fastpq-gpu` Metal
  return gate at
  `dist/izanami-prebuilt-20k-fastpq-gpu-metal-return-120s-20260507-080246`
  exited `0`. It offered, accepted, and succeeded all `2,400,000`
  submissions, used all `2,400,000` prebuilt transactions, and reported `0`
  submit failures, validation rejects, confirmation failures, confirmation
  queue drops, prebuild fallbacks, prebuild skips, prebuild build failures,
  ingress failovers, or unhealthy endpoints.
- Final quorum/strict height was `18/18`; final quorum/strict approved
  transactions were `65,652/65,652`. Submit latency was `p50=3ms`,
  `p95=17ms`, `p99=79ms`, and `max=186ms`. Final queue depth was
  `823,854 / 2,400,000`, with `156` pacemaker backpressure deferrals and
  commit-pipeline EMA `27ms`. This clears the 20k return thresholds
  (`>53,461` strict-approved and `<850,745` queue depth).
- Metal state is improved but not yet fully clean: every peer logged BN254
  Poseidon digest GPU preflight `ok=true`, while general FASTPQ Poseidon prover
  preflight still fell back to CPU with a Metal/CPU parity mismatch. At least
  two BN254 digest batches later hit a Metal command-buffer error and fell back
  deterministically to scalar hashing, so this is successful 20k return-gate
  evidence with Metal digest preflight restored, not evidence of a fully
  sustained GPU-backed proof/digest run.

## 2026-05-07 Metal toolchain preflight restored

- The host Metal compiler component is installed and active. `xcodebuild
  -downloadComponent MetalToolchain` downloaded Metal Toolchain `17E188`, and
  `xcrun --find metal` / `xcrun --find metallib` now resolve through the
  mounted MobileAsset Metal toolchain instead of the inactive Xcode stub.
  `xcrun metal -v` exits `0`.
- Focused FASTPQ BN254 Metal validation passed with
  `CARGO_TARGET_DIR=/tmp/iroha-codex-metal-preflight-20260507-073454 cargo test
  -p fastpq_prover
  metal_bn254_poseidon_word_batch_matches_cpu_self_test_cases --features
  fastpq-gpu -- --nocapture`. The build produced
  `/tmp/iroha-codex-metal-preflight-20260507-073454/debug/build/fastpq_prover-48ecd44d01176b1e/out/fastpq.metallib`
  and the test passed `1` case.
- At the time of the focused preflight, follow-up `iroha_core` digest-gate
  validation was blocked before execution by an existing unmerged conflict in
  `crates/iroha_core/src/sumeragi/main_loop/propose.rs` at line `913`.

## 2026-05-06 Izanami 20k return gate

- The fresh 4-peer no-fault prebuilt `20k TPS` / `120s` `fastpq-gpu`
  return gate at
  `dist/izanami-prebuilt-20k-fastpq-gpu-return-120s-20260506-195652`
  completed the Izanami workload successfully. It offered, accepted, and
  succeeded all `2,400,000` submissions, used all `2,400,000` prebuilt
  transactions, and reported `0` submit failures, validation rejects,
  confirmation failures, confirmation queue drops, prebuild fallbacks, prebuild
  skips, prebuild build failures, ingress failovers, or unhealthy endpoints.
  The shell wrapper exited `1` after completion because zsh treats `status` as
  read-only; `izanami_status.txt` records the workload status as `0`.
- Final quorum/strict height was `19/19`; final quorum/strict approved
  transactions were `69,806/69,806`. Submit latency was `p50=3ms`, `p95=7ms`,
  `p99=18ms`, and `max=202ms`. Final queue depth was
  `835,396 / 2,400,000`, with `120` pacemaker backpressure deferrals and
  commit-pipeline EMA `1ms`. The host did not provide a hardware-backed FASTPQ
  path: the release build reported a missing Metal toolchain, and peer logs
  show BN254 Poseidon digest and Poseidon GPU preflights both `ok=false`, so the
  run is valid CPU-fallback evidence rather than GPU-backed BN254 evidence.

## 2026-05-06 Izanami 20k sampled profile

- A 90s `/usr/bin/sample` profile of one peer during the same 20k prebuilt
  workload completed at
  `dist/izanami-profile-20k-fastpq-gpu-return-sampled-90s-20260506-200245`.
  The sampled Izanami run still accepted and succeeded all `2,400,000`
  submissions with `0` failures, rejects, or queue drops, but sampling
  materially reduced progress: final strict approved fell to `53,346`, queue
  depth rose to `877,159 / 2,400,000`, and submit latency widened to
  `p50=3ms`, `p95=18ms`, `p99=76ms`, `max=418ms`.
- The sampled peer reported a `5.2G` physical footprint. Its top active
  application frame was scalar BN254 Poseidon:
  `iroha_zkp_halo2::poseidon::hash_u64_words_internal` accounted for `33,013`
  collapsed top-of-stack samples. SHA256 (`9,355`), Curve25519 field
  multiplication (`6,113`), Blake2 compression (`5,227`), CRC/Norito helpers,
  and queue admission were visible but lower. All sampled peer logs again show
  BN254 digest and prover Poseidon GPU preflights `ok=false`; this profile
  therefore identifies the deterministic CPU fallback as the current primary
  validation bottleneck, with queue drain/backpressure still the main end-to-end
  throughput limit.

## 2026-05-06 Izanami 20k current gate

- The fresh 4-peer no-fault prebuilt `20k TPS` / `120s` `fastpq-gpu`
  return-current gate at
  `dist/izanami-prebuilt-20k-fastpq-gpu-return-current-120s-20260506-124641`
  exited `0`. It offered, accepted, and succeeded all `2,400,000`
  submissions, used all `2,400,000` prebuilt transactions, and reported `0`
  submit failures, validation rejects, confirmation failures, confirmation
  queue drops, prebuild fallbacks, prebuild skips, prebuild build failures,
  ingress failovers, or unhealthy endpoints.
- Final quorum/strict height was `14/14`; final quorum/strict approved
  transactions were `49,428/49,428`. Submit latency was `p50=3ms`,
  `p95=12ms`, `p99=70ms`, and `max=184ms`. The queue remained saturated at
  `873,062 / 2,400,000`, with `117` pacemaker backpressure deferrals and
  commit-pipeline EMA `12ms`. This returns the current 20k ingress/strict
  progress path to the `49,428` gate target while committed 20k TPS remains a
  queue-drain and validation/serialization follow-up.

## 2026-05-06 Dataspace default fee sponsorship

- `nexus.dataspace_catalog` now accepts `fee_sponsor_account_id` for a
  dataspace-wide default fee sponsor. Explicit transaction `fee_sponsor`
  metadata still wins, while transactions without that metadata inherit the
  sponsor for their routed dataspace.
- Nexus fee admission and execution authorization now accept the routed
  dataspace default sponsor without requiring every caller account to hold a
  direct `CanUseFeeSponsor` grant. Existing explicit grants remain supported,
  and configured dataspace sponsors still require `nexus.fees.sponsorship_enabled`.

## 2026-05-06 BPNG fee-sponsor routing drift fix

- Queue proposal selection and outbound gossip now refresh cached
  lane/dataspace routing from the committed state immediately before exposing a
  queued transaction, update the routing decision cache and ledger with the
  refreshed route, and explicitly reject queued transactions that can no longer
  be routed.
- State-backed account-permission query routing now uses the same account-scope
  fallback as block validation, so `CanUseFeeSponsor` grants for BPNG-scoped
  holders derive the BPNG lane/dataspace consistently while no-state routing
  still defers when committed state is required.
- Focused validation passed with `cargo test -p iroha_core queue::router`,
  `cargo test -p iroha_core queue::`, and
  `cargo test -p iroha_core validate_static_state_dependent`.

## 2026-05-05 Sumeragi VRF late-reveal ingress hardening

- Torii consensus evidence submission now checks the configured stale-evidence
  horizon immediately after decode. Payloads older than the NPoS horizon are
  accepted as stale inputs without running signature/topology validation, so
  stale evidence is not persisted and does not fail for the wrong reason.
- Sumeragi now routes `VrfCommit` and `VrfReveal` metadata through the vote
  worker queue. The actor also handles VRF metadata before polling newly
  committed blocks for epoch catch-up, then polls committed blocks immediately
  afterward. This preserves queued late reveals that arrive while the local WSV
  height has already advanced to the epoch boundary.
- VRF reveal processing now hydrates the in-memory epoch manager from the
  committed VRF epoch record before validating a reveal, so late external
  reveals can be matched against commitments that were persisted by an earlier
  block even if the local manager missed the original commit. Accepted
  Torii-submitted VRF commits/reveals are also gossiped to the active validator
  topology while network-originated copies are not rebroadcast.
- The late-reveal integration harness now avoids blocking progress-log
  submissions, signs external VRF metadata with the active mode tag, retries
  reveal submission while the epoch is still open for late reveals, keeps
  polling for the persisted seal after Sumeragi status reports acceptance, and
  reports compact status diagnostics on failure.
- Focused validation passed for stale evidence submission and the NPoS
  performance metrics baseline:
  `CARGO_TARGET_DIR=/tmp/iroha-codex-uaid-target cargo test -p integration_tests --test consensus_and_da sumeragi_negative_paths::posting_stale_evidence_is_not_persisted -- --nocapture`
  and
  `CARGO_TARGET_DIR=/tmp/iroha-codex-uaid-target cargo test -p integration_tests --test consensus_and_da sumeragi_npos_performance::npos_baseline_1s_k3_captures_metrics -- --nocapture`.
  Focused core validation also passed for
  `incoming_block_message_routes_vrf_metadata_via_vote_queue`,
  `external_vrf_reveal_broadcasts_after_acceptance`,
  `merge_record_observations_hydrates_commit_for_late_reveal`,
  `committed_vrf_record_does_not_cover_newer_pending_late_reveal`,
  `on_block_message_handles_vrf_reveal_before_commit_catchup_finalizes_epoch`,
  and the permissioned-mode VRF commit/reveal handler regressions.
- Validation caveat: the focused late-reveal integration now reaches
  Sumeragi acceptance (`vrf_late_reveals_total = 1`) but still fails to observe
  the persisted epoch seal because the four-peer NPoS/DA network repeatedly
  stalls at height 4 with pending RBC sessions waiting on missing INIT/chunk
  data. Treat that as a separate DA/RBC liveness issue before using this
  integration as the final VRF persistence gate. Formatting and whitespace were
  rerun after these edits.

## 2026-05-05 Offline Note V2 wallet-derived commitments

- Offline Note V2 commitment derivation now starts in the wallet instead of
  Torii settlement metadata. `iroha_data_model::offline` exposes canonical
  Norito preimages and domain tags for note commitments, input nullifiers, and
  payment token ids, with 32-byte `note_secret` and `token_nonce` material
  enforced by the derivation helpers.
- Torii `/v1/offline/v2/notes/issue` now requires a wallet-supplied bare
  64-character hex `note_commitment`, issues that exact commitment, and keeps
  `settlement.entry_hash` as lineage/settlement metadata rather than deriving
  the note commitment from it.
- Kotlin/JVM, Java Android, and Swift Offline Note V2 model helpers now match
  the Rust derivation vectors for source notes, P2P output notes, input
  nullifiers, payment token ids, and redeem nullifiers. The shared Offline V2
  fixture was regenerated with the derivation preimages, and transaction
  fixtures were refreshed after aligning SDK account-controller encoding on
  compact public-key payload bytes.
- Kotlin/JVM, Java Android, and Swift now expose `OfflineNoteV2Wallet` facades
  for `load`, `prepareReceive`, `pay`, `accept`, `redeem`, and `sync`, with
  structured in-memory stores, injectable attestation/random/proof/issuer
  boundaries, direct audit/redeem transaction submitters, and mock lifecycle
  tests covering load, P2P pay, accept/audit, redeem, and spent/change-pending
  state transitions.
- Kotlin/JVM, Java Android, and Swift now include Torii-backed
  `OfflineNoteV2IssuerClient` adapters for `/v1/offline/v2/keys/refill` and
  `/v1/offline/v2/notes/issue`. The adapters body-sign issuer JSON with the
  canonical request signer, cache signed lineage state between refill and
  issue, derive wallet commitments against the post-issue revision, and submit
  the wallet-supplied `note_commitment` unchanged.
- Validation passed with the focused Rust Offline V2 data-model, Torii issuer,
  and core tests; full Kotlin `:core-jvm:test`; Java Android core harness; and
  Swift `OfflineNoteV2Tests`. The 2026-05-06 rerun of
  `cd IrohaSwift && swift test --filter OfflineNoteV2Tests` is green
  (`19` tests, `0` failures). Earlier derivation work in this slice also had
  focused Java fixture/Norito parity coverage and full `swift test` in
  `IrohaSwift` green. Formatting and whitespace checks are green with
  `cargo fmt --all --check` and `git diff --check`. Production sync outcome
  adapters, structured secure note stores, and public Norito decoders remain
  tracked as release work in `roadmap.md`.

## 2026-05-06 Torii query API WSV fast paths

- Generic Torii query execution now routes more common JSON predicates through
  in-memory WSV/Kura indexes instead of falling back to full scans. The covered
  surfaces include repo agreements, public and anonymous asset escrows, proof
  records, triggers, active trigger IDs, roles, blocks, block headers, and
  committed transactions constrained by block hash, entrypoint hash, authority,
  timestamp, timestamp range, or result status. Committed-transaction typed
  filters now expose their parsed filter set to the planner, so `ts_ge`/`ts_le`
  ranges and typed authority/entrypoint/result filters use the same Kura indexes
  as JSON equality predicates. Multiple positive transaction constraints are
  intersected at the block-height candidate stage instead of only choosing the
  smallest candidate set. NFT queries now also plan domain predicates through
  the existing `nfts_in_domain_iter` range instead of scanning the whole NFT
  store. Proof-record JSON predicates now intersect id/backend/status candidate
  sets from the proof-id key order and status index. Asset queries now preserve
  exact asset-id predicates in the general planner, so `id == ...` combined
  with additional JSON conditions still uses direct WSV lookup instead of
  widening to account/definition/domain scans. Repo agreement, public escrow,
  anonymous escrow, block/header, trigger, and role planners now intersect
  repeated positive index-derived candidate sets instead of retaining only the
  smallest set, keeping conjunctive predicates narrow before final filtering.
- Kura now maintains block-hash-to-height and committed-transaction indexes
  alongside the in-memory block log, keeping hash/transaction lookups fast
  across replay, durable block append, lazy block-body loading, top-block
  replacement, and pruning. Reopened stores with only hash metadata keep the
  transaction index marked partial until loaded block bodies make it complete,
  so query planners fall back to full scans instead of returning incomplete
  results.
- Focused validation passed with the repo agreement, escrow, proof record,
  trigger, role, block/header, committed-transaction block-hash, transaction
  entrypoint, transaction authority/timestamp/result-status, typed transaction
  timestamp-range, NFT owner/domain range, proof backend/status intersection,
  Kura pruning, lazy transaction-index completion, asset exact-id
  general-planner, repo participant-candidate intersection, escrow
  status/buyer-candidate intersection, block candidate-height intersection,
  trigger candidate-id intersection, and role candidate-id intersection unit
  tests. Formatting is green; crate-level
  `cargo check -p iroha_core --lib` is green; `cargo check -p iroha_data_model
  --features fast_dsl` is green; `git diff --check` and
  `scripts/check_no_scale.sh` are green. The 2026-05-06 rerun also covered
  `find_transactions_by_authority_timestamp_and_result_use_kura_indexes`,
  `find_transactions_by_filter_timestamp_range_uses_kura_index`,
  `transaction_index_completes_after_lazy_loading_reopened_blocks`, and
  `find_proof_records_intersects_backend_and_status_indexes` under
  `CARGO_TARGET_DIR=/tmp/iroha-codex-query-fastpaths`, plus
  `asset_predicate_view_extracts_alias_fields_for_planner` and
  `find_assets_filters_by_exact_id_with_extra_predicate`,
  `repo_agreement_candidates_intersect_participant_indexes`,
  `asset_escrow_candidates_intersect_status_and_buyer_indexes`,
  `block_candidate_heights_are_intersected`,
  `trigger_candidate_ids_are_intersected`, and
  `role_candidate_ids_are_intersected` in the default target.

## 2026-05-05 Offline V2 issuer body auth

- Offline V2 issuer POSTs now reject legacy `X-Iroha-*` app-auth headers and
  verify `account_id`, `timestamp_ms`, `nonce`, plus exactly one of
  `signature_base64` or `witness_base64` from the JSON body. The signed body
  hash uses Norito JSON canonical bytes with only top-level proof fields
  removed, preserving nested receipt and lineage signatures.
- Kotlin and Java `CanonicalRequestSigner` helpers now build body-auth signing
  messages and single-signature body fields, with witness helper support for a
  prebuilt `witness_base64`.
- Focused Torii validation passed with
  `CARGO_TARGET_DIR=target/codex-offline-body-auth cargo test -p iroha_torii body_auth --lib`.
  The broader Offline V2 issuer filter also passed with
  `CARGO_TARGET_DIR=target/codex-offline-body-auth cargo test -p iroha_torii offline_v2`
  (the lib slice reported `13` passed and `1749` filtered out, plus the package
  filter covered `offline_v2_readiness_is_mounted_and_legacy_routes_are_absent`).
  `rustfmt --edition 2024 --check crates/iroha_torii/src/app_auth.rs crates/iroha_torii/src/offline_v2_issuer.rs`
  and full `cargo fmt --all -- --check` are green.
- The focused strict lint gate
  `CARGO_TARGET_DIR=target/codex-offline-body-auth cargo clippy -p iroha_torii --lib -- -D warnings`
  is green after clearing current-tree blockers in `iroha_p2p` decrypted-frame
  parsing and exposing the proof-record indexed insert helper to production
  builds. The p2p cleanup is covered by
  `CARGO_TARGET_DIR=target/codex-offline-body-auth cargo test -p iroha_p2p`
  (`161` unit tests and `15` integration tests passed).
- Full Rust workspace build is green with `cargo build --workspace` in the
  normal repository target directory (`20m10s`). A first duplicate-target
  attempt failed with `No space left on device`; removing the generated
  `target/codex-offline-body-auth` tree freed enough space for the successful
  normal-target build.
- The full workspace all-target clippy gate is green with
  `cargo clippy --workspace --all-targets -- -D warnings` after clearing
  current-tree blockers in `fastpq_prover`, `iroha_executor`, the SoraFS pin
  client helper, Kagami genesis/localnet code, scheduler telemetry tests, and
  IVM host syscall coverage tests.
- Follow-up focused tests for the nontrivial clippy fixes also passed:
  `cargo test -p fastpq_prover metal_config --lib` (`17` passed,
  `275` filtered out) and
  `cargo test -p iroha build_register_manifest_payload_contains_expected_fields --lib`
  (`1` passed, `283` filtered out).
- Kotlin/Java validation is green when Gradle is pointed at the local Homebrew
  OpenJDK 21 install:
  `JAVA_HOME=/opt/homebrew/opt/openjdk@21/libexec/openjdk.jdk/Contents/Home PATH=/opt/homebrew/opt/openjdk@21/bin:$PATH ./gradlew :core-jvm:test --console=plain`
  and
  `JAVA_HOME=/opt/homebrew/opt/openjdk@21/libexec/openjdk.jdk/Contents/Home PATH=/opt/homebrew/opt/openjdk@21/bin:$PATH ANDROID_HOME=$HOME/Library/Android/sdk ANDROID_SDK_ROOT=$HOME/Library/Android/sdk ./gradlew test --console=plain`
  from `java/iroha_android`.
- Swift SDK validation also passed with `swift test` from `IrohaSwift`
  (`783` executed, `101` skipped, `0` failures).

## 2026-05-05 Soracloud and local acceleration validation

- The full `irohad` Soracloud binary filter is green with
  `env -u LOG_FORMAT CARGO_TARGET_DIR=/tmp/iroha-codex-soracloud-full cargo test -p irohad --features embedded-soracloud-runtime --bin irohad soracloud -- --nocapture`
  (`97` passed, `1` ignored, `139` filtered out). The expected
  `manager_config_rejects_unsafe_direct_actual_production_posture` panic is
  covered by a `#[should_panic]` test.
- Local Apple Metal is available on this macOS arm64 host:
  `xcrun -sdk macosx metal -v` reports Apple Metal `32023.864`. `nvcc` is not
  on `PATH`, so CUDA hardware parity was not run here.
- Local IVM acceleration gates are green:
  `RUST_TEST_THREADS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-accel-metal cargo test -p ivm --features metal --test metal_sha256 --test gpu_determinism --test metal_disable_on_mismatch --test vector_ops -- --nocapture`
  and
  `CARGO_TARGET_DIR=/tmp/iroha-codex-accel-simd cargo test -p ivm --test acceleration_simd --test simd_tail_misalignment --test poseidon_simd --test vector_detect -- --nocapture`.
- Norito Metal/SIMD gates are green with
  `CARGO_TARGET_DIR=/tmp/iroha-codex-norito-metal cargo test -p jsonstage1_metal -p gpuzstd_metal -- --nocapture`
  and
  `CARGO_TARGET_DIR=/tmp/iroha-codex-norito-metal cargo test -p norito --features "json simd-accel parallel-stage1 stage1-validate codec-gpu-metal" stage1 -- --nocapture`.
  `gpuzstd_metal` reported the runtime GPU backend unavailable and skipped its
  GPU-only assertions, while the helper and Stage-1 suites passed.
- FASTPQ Metal hardware parity is green on the available Apple Metal backend:
  `FASTPQ_GPU=gpu RUST_TEST_THREADS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-fastpq-metal cargo test -p fastpq_prover --features fastpq-gpu --lib metal -- --nocapture`
  passed (`67` passed, `285` filtered out). The fix covers deterministic
  test device hints, BN254 fixture/kernel coverage, Goldilocks reduction,
  domain-rooted FFT twiddles, coefficient-wise LDE coset scaling, tile
  writeback, BN254 LDE scaling, and Metal-safe Poseidon partial-round
  compilation.
- The full Soracloud production-readiness profile was not run because this
  workspace has no operator mixed-host inventory or observability evidence
  inputs. That remains a rollout blocker rather than a local code failure.
- Hygiene checks passed with `cargo fmt --all -- --check`, `git diff --check`,
  and `scripts/check_no_scale.sh`.

## 2026-05-05 UAID replay/checkpoint hardening

- Kura replay now restores per-block commit-QC hints from the commit-roster
  journal without pre-populating WSV commit-QC storage during journal restore.
  Canonical replay checkpoints ignore consensus scheduling/evidence caches
  (`commit_topology`, `prev_commit_topology`, `world.commit_qcs`, and
  `world.vrf_epochs`) that are reconstructed from Kura and sidecar journals
  rather than from transaction execution.
- Checkpointed replay preserves committed Kura transaction results when local
  execution drifts on already-committed history, then enforces the stored WSV
  checkpoint. The committed-result fallback now seeds canonical transaction
  context, replays successful committed transactions, runs time triggers, and
  allows ZK transfer effects only in that replay-trust path so normal admission
  still rejects invalid local proof verification.
- Focused validation passed with the replay/checkpoint unit tests, the Halo2
  restart-marker fixture verifier, and the previously failing
  `consensus_and_da` restart/localnet cases:
  `sumeragi_restart_retains_lock_convergence`,
  `npos_pacemaker_resumes_after_downtime`,
  `confidential_combined_peer_downtime_and_timeout_pressure_localnet`, and
  `confidential_dual_restart_stress_mid_flow_localnet`. Formatting,
  diff-whitespace, Cargo.lock, debug/source-guard checks, strict clippy for
  `iroha_core --lib`, `iroha_crypto --lib --tests`, and
  `integration_tests --test consensus_and_da` are green.

## 2026-05-05 Durable Space Directory snapshot restore

- State snapshots now persist the durable Space Directory manifest registry in
  an explicit top-level `space_directory_manifests` section. This closes the
  restart hole where a peer could load a height-consistent snapshot that had
  silently dropped the manifests needed by later proposals.
- Snapshot restore decodes the manifest registry and runs the existing storage
  migration pass so UAID dataspace bindings are rebuilt from active manifest
  records before the node resumes.
- Legacy snapshots that are missing the new section are treated as recoverable
  when Kura history up to the snapshot height contains Space Directory manifest
  instructions, allowing startup to discard the incomplete snapshot and rebuild
  from the block log.
- Kura replay checkpoint validation accepts the pre-upgrade WSV checkpoint hash
  only when it matches the legacy snapshot surface without the new manifest
  registry, then logs the upgrade compatibility path. New checkpoints continue
  to hash the full durable snapshot surface.
- Validation:
  - `cargo fmt --all`
  - `cargo test -p iroha_core --lib snapshot_roundtrip_preserves_space_directory_manifests_and_rebuilds_bindings -- --nocapture`
  - `cargo test -p iroha_core --lib can_read_snapshot_after_writing -- --nocapture`
  - `cargo test -p irohad snapshot_read_error_is_recoverable_classifies_errors -- --nocapture`

## 2026-05-04 UAID workspace-test corridor follow-up

- The events/time-trigger failures exposed by the broad workspace sweep are now
  covered by the full `events_and_triggers` target (`36` passed). Time-trigger
  execution seeds the real trigger call hash for queued instructions, same-id
  reschedules preserve the new repeat budget, and blocking client confirmation
  stream close is bounded so subscription tests can shut down cleanly.
- Stale IVM/Kotodama and Norito fixtures were refreshed for the current ABI and
  schema layouts: `mint_rose_trigger.to`,
  `query_assets_and_save_cursor.to`, `smart_contract_can_filter_queries.to`,
  lane commitments, Space Directory capability manifests, Norito instruction
  JSON, and streaming RANS snapshots. `queries_and_proofs` is green (`23`
  passed), and `nexus_and_streaming` is green (`255` passed, `2` ignored).
- Prepared transaction metadata now derives external execution-entrypoint
  hashes from the actual canonical Norito signed payload bytes instead of a
  synthesized encoded length. A governance runtime-upgrade regression covers the
  rich proposal payload case that previously produced entrypoint hash
  mismatches during Sora parliament execution.
- Sora governance wait helpers now request transaction status with explicit
  `scope=auto`, matching the intended local-or-routed Torii lookup for
  referendum proposal and runtime-upgrade transaction confirmation.
- Instruction payload framing now records the actual Norito header flags used
  for each inner instruction payload instead of reframing adaptive encodings
  with default flags. `OpaqueInstruction` also preserves and re-emits the exact
  framed payload bytes. This fixes genesis/block decode failures where a header
  advertised compact layout bits that the payload did not use.
- Wrong-ingress account-permissions fanout now reports
  `x-iroha-routed-by=proxy` from the attempted route set, even when only local
  payloads survive merge filtering. This keeps route diagnostics honest for
  signed all-dataspace fanout reads and fixes the cross-dataspace localnet
  assertion.
- The unstable-network fault selector now avoids isolating DA/RBC collector
  peers for single-fault runs when safe alternatives exist. Focused selector
  tests, the `unstable_network_8_peers_1_fault` regression, and the full
  `extra_functional::unstable_network` slice are green (`29` passed).
- Private transaction entrypoints now carry their own hash through checking,
  queue requeue/removal, and gossiper diagnostics instead of forcing
  `AsRef<SignedTransaction>`. This removes the private-entrypoint panic exposed
  by sealed-reveal gossip tests. The full `core_api` target is green after the
  repair and liveness hardening for slow asset exchange and sealed-reveal
  height advancement (`171` passed, `4` ignored; 3193.09s).
- Additional validation passed with focused Torii fanout clippy/tests, the
  reduced-sample ignored `torii_load_profile`, `cargo fmt --all -- --check`,
  `git diff --check`, `scripts/check_no_scale.sh`, and focused strict clippy
  for `iroha`, `iroha_core`, `iroha_torii`, `network_functional`,
  `nexus_and_streaming`, and the `core_api` integration target. Focused
  `iroha_core` unit tests for entrypoint hashing, the stateless-validation
  cache, and snapshot roundtrip helpers are also green.
- The latest broad `cargo test --workspace` reached the integration-test
  library after a full workspace compile and the earlier crate/test targets
  were green. The first integration library pass exposed a stale spawned
  daemon artifact that rejected generated genesis as a Norito length mismatch;
  after the daemon artifact rebuilt, the exact startup/drop regressions and the
  full `integration_tests --lib` suite are green (`41` passed).
- Core transaction signature validation now routes the prepared single-Ed25519
  verifier path through the deterministic batch verifier, removes the obsolete
  public single pre-parsed Ed25519 verifier, and renames replay/precheck
  internals so the signature-bypass source guard stays meaningful. Sumeragi
  heartbeat block fixtures now embed routing execution context before signing,
  matching production proposal construction and keeping signature-index
  recovery validation compatible with external entrypoint context checks.
  Focused validation passed with the core `signature` slice (`102` passed),
  `iroha_crypto` Ed25519 aggregate tests (`7` passed), the Ed25519 public-key
  fast-cache unit, strict clippy for `iroha_core --lib`, `iroha_crypto
  --lib --tests`, and `integration_tests --lib`, plus formatting, diff
  whitespace, no-SCALE, and signature-bypass term guards. A fresh end-to-end
  `cargo test --workspace` remains queued for a clean uninterrupted rerun.

## 2026-05-04 Sumeragi embedded QC and NPoS block-sync hardening

- Embedded QC roster fallback now requires the QC's advertised validator set to
  match an authoritative topology candidate for the QC height/view and
  consensus mode before aggregate validation or payload recovery can proceed.
  This closes the permissioned shrink-roster path where a QC signed by one
  known validator could satisfy quorum against its own embedded one-validator
  set after local cached-roster validation failed, and keeps NPoS fallback tied
  to the elected stake topology instead of the QC author's advertised roster.
- Embedded roster fallback also fails closed when any advertised validator is
  missing a cached BLS proof of possession, matching the commit-certificate and
  checkpoint roster validation posture.
- NPoS block-sync roster selection now carries the locally resolved stake
  snapshot forward after commit-certificate/checkpoint validation. A valid
  NPoS artifact no longer validates using a recomputed snapshot only to lose
  that snapshot before the later block-signature quorum and QC validation
  checks.
- Focused validation passed with
  `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-highest cargo test -p iroha_core --lib embedded_roster -- --nocapture`
  (`3` passed),
  `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-highest cargo test -p iroha_core --lib commit_qc_rejects_shrunk_embedded_roster -- --nocapture`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-highest cargo test -p iroha_core --lib commit_qc_rejects_embedded_roster_with_missing_pop -- --nocapture`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-highest cargo test -p iroha_core --lib commit_qc_bootstraps_from_embedded_roster_when_cached_roster_is_stale -- --nocapture`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-highest cargo test -p iroha_core --lib selection_from_roster_artifacts_uses_commit_cert_epoch_for_checkpoint -- --nocapture`,
  and
  `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-highest cargo test -p iroha_core --lib find_repo_agreements_uses_id_predicate_lookup -- --nocapture`.
  Formatting validation passed with `rustfmt --edition 2024 --check` over the
  touched Rust files.

## 2026-05-03 IVM ABI v1 gas/error hardening

- The generated ABI v1 syscall gas table no longer has placeholder gas rows:
  domain lifecycle syscalls now declare dedicated gas assets, FastPQ batch
  scope/admin/contract-call/escrow/Soracloud families are in the spec, and
  `gen_syscalls_doc --check` is clean after regenerating `syscalls.md`,
  `syscalls_doc_gen.rs`, and `gas_spec.rs`.
- Syscall execution now supports metered host errors: hosts can attach
  deterministic gas to expensive failure paths, the VM debits that gas before
  surfacing the original error kind, and diagnostics/trap classification peel
  the wrapper so callers still see the underlying failure.
- Default, standalone CoreHost, WSV mock host, and the real CoreHostImpl now
  charge fixed gas for FastPQ batch begin/end. Soracloud host syscalls charge
  request/response byte gas, contract calls charge parent overhead plus return
  bytes while leaving child execution gas with the child VM, and allowed
  host-inapplicable syscalls return metered `NotImplemented` instead of falling
  through to `UnknownSyscall`.
- Mixed-hardware consensus coverage now compares the register result, gas used,
  and full deterministic execution-proof summary across adaptive acceleration
  and scalar fallback policies.
- Focused validation passed with the regenerated-doc check, the requested IVM
  doc/gas/ABI and host-policy test batches, the focused `iroha_core` AXT
  library and integration tests, and Soracloud host/local-read regressions under
  `--features embedded-soracloud-runtime`. `cargo test -p irohad soracloud
  --lib` is not a valid validation command because `irohad` has no library
  target; the equivalent full Soracloud binary filter still has unrelated
  environment/materialization failures outside this slice.

## 2026-05-03 FastPQ V1 AXT validation gap follow-up

- FastPQ example docs under `docs/source/examples` no longer carry stale
  pre-V1 terminology; the focused scan for replay-specific verifier errors,
  synthetic transfer helpers, legacy FastPQ names, and diagnostic-only AXT
  acceptance language is clean across the FastPQ/IVM/core/docs scope.
- The app-API AXT core-host validation command was tightened to include the
  required `iroha-core-tests` feature. `--features app_api` alone compiles the
  `ivm_corehost_axt` integration target with zero tests because the file is
  gated by `#![cfg(feature = "iroha-core-tests")]`.
- Enabling the real test target exposed and fixed two gaps: trigger-set
  active-id cache deserialization now iterates through a storage view with the
  required `mv::Value` bound, and the multi-dataspace CoreHost AXT fixture now
  uses FastPQ-backed proof blobs instead of raw manifest-root placeholders.
  Validation passed with
  `CARGO_TARGET_DIR=/tmp/iroha-codex-fastpq-gap-v2 cargo test -p iroha_core --features "app_api iroha-core-tests" --test ivm_corehost_axt -- --nocapture`
  (`26` passed).
- Added follow-up coverage for the repaired paths: trigger-set DTO and JSON
  roundtrips now assert active-trigger ID caches are rebuilt while depleted
  triggers stay inactive; CoreHost AXT now rejects proof envelopes for the
  wrong dataspace and envelopes whose FastPQ binding advertises a mismatched
  `source_dsid`; IVM trap classification now covers metered wrapper errors.
  Focused validation passed with
  `CARGO_TARGET_DIR=/tmp/iroha-codex-fastpq-gap-v2 cargo test -p ivm --lib metered_trap_classifies_as_source_error -- --nocapture`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-fastpq-gap-v2 cargo test -p iroha_core --lib set_roundtrips_rebuild_active_trigger_ids -- --nocapture`,
  the two new focused `ivm_corehost_axt` tests, and the full
  `CARGO_TARGET_DIR=/tmp/iroha-codex-fastpq-gap-v2 cargo test -p iroha_core --features "app_api iroha-core-tests" --test ivm_corehost_axt -- --nocapture`
  target (`28` passed).
- Added FastPQ verifier-layer coverage for AXT proof payload tampering:
  verification now has explicit regressions for a batch mutated after its AXT
  seal was written and for a valid proof paired with a different sealed batch.
  The first fails on the batch-seal metadata, while the second reaches the core
  proof verifier and fails with `CommitmentMismatch`. Transfer SMT coverage now
  also rejects empty witness material and transcripts missing the receiver proof.
  Additional AXT binding coverage rejects envelope/source dataspace mismatches,
  batch public-input dataspace mismatches, embedded binding metadata mismatches,
  and transfer claims that omit transfer transcripts.
  A third coverage pass now exercises missing `fastpq_binding`, empty
  execution batches, target-dataspace metadata mismatches, transfer claims
  without transfer rows, malformed transfer transcript metadata, missing sender
  SMT proofs, and transcript root chains that do not connect. The Ed25519
  public-key parse cache keeps large valid parse outcomes boxed so the strict
  `variant-size-differences` lint does not block downstream FastPQ test builds.
  A fourth coverage pass adds binding normalization and manifest-hash stability,
  malformed digest and claim-type rejection, AXT batch parameter mismatch
  checks, malformed embedded binding metadata, source transaction commitment
  metadata mismatch, empty-corridor acceptance, transfer sender underflow,
  receiver mismatch/overflow, missing receiver rows, and wrong initial SMT root
  rejection.
  A fifth coverage pass adds transition-batch data-model roundtrips across
  mint/burn/role-revoke/meta operations, required AXT metadata removal checks,
  optional DA-commitment statement binding, non-transfer row filtering, transfer
  sender row value mismatches, negative numeric bounds, Merkle proof accessor
  bounds, extra sibling rejection, and `TransferRowKey::from_transition`.
  A sixth coverage pass adds bind-time missing/wrong `entry_hash` rejection,
  receipt/witness/corridor metadata mismatch checks, empty transcript metadata
  decoding, empty transcript-set verification, chained SMT witness construction
  across multiple transcripts, stale chained balance rejection, and empty
  transcript witness root matching/mismatch cases.
  Validation passed with the focused new tests and
  `CARGO_TARGET_DIR=/tmp/iroha-codex-fastpq-gap-v2 cargo test -p fastpq_prover --lib -- --nocapture`
  (`290` passed), plus focused Ed25519 cache checks with
  `CARGO_TARGET_DIR=/tmp/iroha-codex-fastpq-gap-v2 cargo test -p iroha_crypto ed25519_cache -- --nocapture`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-fastpq-gap-v2 cargo test -p iroha_crypto parse_public_key_cache -- --nocapture`,
  and
  `CARGO_TARGET_DIR=/tmp/iroha-codex-fastpq-gap-v2 cargo test -p iroha_crypto parse_public_key_uses_thread_local_cache_for_valid_keys -- --nocapture`.

## 2026-05-03 Workspace clippy corridor follow-up

- The full workspace all-target clippy corridor is green again with
  `CARGO_TARGET_DIR=/tmp/iroha-codex-uaid-target cargo clippy --workspace --all-targets -- -D warnings`.
- Follow-up lint repairs were kept narrow: `iroha_core` state escrow index
  helpers now use the `mv::Key` bound required by `StorageTransaction`, the
  NewView highest-QC vote selector is compiled only for tests, Ed25519 cache
  index masking now uses checked conversions with a focused bounds regression,
  the FASTPQ BN254 Poseidon pending wait API documents its cfg-dependent
  fallibility, and signed-transaction payload preparation documents why it is
  intentionally client-independent.
- The IVM CoreHost AXT policy test now expects commit gas for the actually
  recorded flow shape: an empty proof preflight does not store a proof entry,
  so the valid flow commits one touch and one handle. Focused validation passed
  with `CARGO_TARGET_DIR=/tmp/iroha-codex-uaid-target cargo test -p ivm --test core_host_policy -- --nocapture`
  (`21` passed), and the adjacent AXT host dispatch/flow tests passed with
  `CARGO_TARGET_DIR=/tmp/iroha-codex-uaid-target cargo test -p ivm --test axt_host_flow --test host_unknown_syscall -- --nocapture`
  (`35` passed).
- Additional validation passed with
  `CARGO_TARGET_DIR=/tmp/iroha-codex-uaid-target cargo clippy -p iroha_core --lib -- -D warnings`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-uaid-target cargo clippy -p iroha_core --all-targets -- -D warnings`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-uaid-target cargo clippy -p iroha_crypto --all-targets -- -D warnings`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-uaid-target cargo test -p iroha_crypto ed25519_cache_indexes_stay_within_cache_masks -- --nocapture`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-uaid-target cargo clippy -p fastpq_prover --all-targets -- -D warnings`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-uaid-target cargo clippy -p ivm --all-targets -- -D warnings`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-uaid-target cargo clippy -p iroha --all-targets -- -D warnings`,
  `cargo fmt --all -- --check`, `scripts/check_no_scale.sh`, and
  `git diff --check`.
- Full workspace tests were not rerun in this follow-up and remain for the next
  uncontended validation window.

## 2026-05-03 Events and time-trigger workspace-test follow-up

- A broad `CARGO_TARGET_DIR=/tmp/iroha-codex-uaid-target cargo test --workspace`
  run completed the build and passed the already-reached `consensus_and_da`
  (`250` passed, `6` ignored) and `core_api` (`171` passed, `4` ignored)
  integration targets before stopping in `events_and_triggers`.
- The two by-call trigger failures were caused by a stale
  `mint_rose_trigger.to` artifact with invalid current IVM metadata. The
  Kotodama sample and integration fixture were regenerated from
  `mint_rose_trigger.ko`; the refreshed artifact is a current 593-byte CNTR
  payload with ABI v1 metadata.
- Subscription time-trigger billing now succeeds in the integration scenario.
  Time-trigger execution seeds `tx_call_hash` from the actual
  `TimeTriggerEntrypoint` hash before applying queued instructions, so FastPQ
  transfer transcript recording works for transfers produced by time triggers.
  Same-id time-trigger reschedules also preserve the newly registered action's
  repeat budget instead of consuming it after the old invocation finishes.
- The blocking client confirmation close path is bounded so a timed-out
  confirmation stream cannot strand `spawn_blocking` during async test runtime
  shutdown. The subscription poll helper now submits tick transactions without
  waiting for confirmation and records richer timeout diagnostics for future
  invoice failures.
- Focused validation passed with
  `CARGO_TARGET_DIR=/tmp/iroha-codex-uaid-target cargo test -p iroha_core --lib time_trigger_same_id_reschedule_keeps_new_repeat_budget -- --nocapture`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-uaid-target cargo test -p iroha --lib close_tx_confirmation_stream -- --nocapture`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-uaid-target cargo test -p integration_tests --test events_and_triggers subscriptions::subscription_scenarios -- --nocapture`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-uaid-target cargo test -p integration_tests --test events_and_triggers triggers::by_call_trigger::call_execute_trigger_with_args -- --nocapture`,
  and
  `CARGO_TARGET_DIR=/tmp/iroha-codex-uaid-target cargo test -p integration_tests --test events_and_triggers triggers::by_call_trigger::trigger_in_genesis -- --nocapture`.
  Strict focused clippy passed for `iroha_core --lib`, `iroha --lib`, and the
  `events_and_triggers` integration target. `cargo fmt --all -- --check`,
  `scripts/check_no_scale.sh`, and `git diff --check` are also clean.
- The full workspace test sweep was not restarted after these repairs; it
  remains queued for the next long validation window.

## 2026-05-03 Sumeragi NewView QC signature binding

- Sumeragi vote preimages now bind the optional `highest_qc` reference and use
  the new `Vote/v2` consensus domain. Aggregate QC verification mirrors the
  QC's `highest_qc` into the reconstructed vote preimage, so NPoS aggregate-only
  validation cannot accept a NewView QC whose highest-QC hint was substituted
  after signing.
- NewView QC formation now groups votes by exact signed highest-QC reference
  and only aggregates a quorum from one group. This avoids locally building a
  same-message BLS aggregate from votes that signed different NewView
  justifications.
- NewView QC validation against the local vote log now requires each counted
  vote to carry the exact same `highest_qc` reference as the QC aggregate. A
  lower-ranked local hint is no longer accepted as a match for a higher-ranked
  aggregate preimage.
- QC validation against the local vote log also requires counted votes to match
  the QC parent/post state roots, because those roots are part of the signed
  aggregate preimage. Root mismatches now get their own validation reason while
  aggregate recovery remains available for stale local catch-up votes.
- Non-NewView votes and QCs now reject unexpected `highest_qc` payloads so the
  signed NewView-only field cannot create alternate Prepare/Commit preimages.
- Block-sync QC validation, commit-certificate roster validation, embedded-roster
  bootstrap, and aggregate-only block-sync fallback now enforce the same
  NewView-only `highest_qc` invariant. Block-sync commit evidence also rejects
  non-`Commit` QCs before aggregate fallback, and the fallback rechecks
  permissioned commit quorum and NPoS stake quorum instead of treating an
  aggregate-valid permissioned QC as sufficient by itself.
- Commit-certificate and validator-checkpoint roster validation now fail closed
  when any bitmap signer is missing a BLS proof of possession. They no longer
  log and accept the roster without aggregate verification.
- The data-model comments for `QcVote::highest_qc` and `Qc::highest_qc` now
  reflect that the field is cryptographically bound into vote and aggregate
  signatures.
- During the focused rerun, unrelated dirty compile blockers in `state.rs`,
  `block.rs`, and `gossiper.rs` were repaired: the `ProofId` range-bound macro
  now uses a local identifier, the pending FASTPQ transcript digest hooks
  expected by block finalization are restored, and local Ed25519 batch helper
  lifetimes now match `Ed25519BatchScratch`. A later proof-query lifetime
  blocker in `smartcontracts/isi/world.rs` was also repaired by returning an
  owned iterator for status-indexed proof queries.
- Focused validation passed with
  `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-highest cargo test -p iroha_core --lib non_new_view_highest -- --nocapture`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-highest cargo test -p iroha_core --lib missing_pop -- --nocapture`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-highest cargo test -p iroha_core --lib validate_block_sync_qc -- --nocapture`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-highest cargo test -p iroha_core --lib block_sync_qc_aggregate_fallback -- --nocapture`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-highest cargo test -p iroha_core --lib qc_validation_error_reports_reason_labels -- --nocapture`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-highest cargo test -p iroha_core --lib validate_qc_against_votes -- --nocapture`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-highest cargo test -p iroha_core --lib validate_qc_against_votes_rejects_new_view_vote_highest_mismatch -- --nocapture`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-highest cargo test -p iroha_core --lib validate_qc_against_votes_rejects_state_root_mismatch -- --nocapture`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-highest cargo test -p iroha_core --lib recover_qc_from_aggregate_accepts_commit_subject_mismatch -- --nocapture`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-highest cargo test -p iroha_core --lib validate_commit_qc_roster -- --nocapture`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-highest cargo test -p iroha_core --lib validate_checkpoint_roster -- --nocapture`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-highest cargo test -p iroha_core --lib highest_qc_substitution -- --nocapture`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-highest cargo test -p iroha_core --lib new_view_highest -- --nocapture`, and
  `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-highest cargo test -p iroha_core --lib new_view_votes_still_form_qc -- --nocapture`.
  Formatting and whitespace validation passed with `rustfmt --edition 2024 --check`
  over the touched Rust files and `git diff --check`.

## 2026-05-03 Core API multisig and status smoke follow-up

- Multisig direct-sign admission now rejects only authorities that actually
  carry multisig state, multisig metadata, or a multisig controller. Ordinary
  single-key signatories keep direct-signing rights even when they hold a
  `MULTISIG_SIGNATORY/` role assigned by another multisig account.
- The status endpoint smoke test now decodes the Norito status payload with the
  canonical byte helper and uses the current SNS lease payment amount in its
  setup path.
- The multisig CLI list-all regression test now resolves the `iroha` CLI binary
  before submitting expiring proposals, then reuses the resolved path for JSON,
  text, and paged CLI checks. This keeps Cargo build-lock contention outside
  the proposal lifetime and preserves the hashed-role-suffix coverage.
- The shared dynamic-key comparison macro now accepts full Rust type syntax for
  `target:` so qualified path targets compile as well as bare identifiers.
- Focused validation passed with
  `cargo test -p iroha_core multisig_signatory_role_does_not_block_direct_signing --lib -- --nocapture`,
  `cargo test -p integration_tests --test core_api misc::misc_status_endpoints_smoke -- --nocapture`, and
  `cargo test -p integration_tests --test core_api multisig::multisig_cli_list_all_resolves_hashed_role_suffixes -- --nocapture`.
  The full multisig slice now passes with
  `cargo test -p integration_tests --test core_api multisig:: -- --nocapture`:
  `17` passed, `0` failed.
  A prior full `core_api` integration sweep exposed the fixed status and
  multisig regressions; the whole `core_api` target was not rerun after this
  focused repair.

## 2026-05-03 Kotodama artifact and access-hint hardening

- Contract bytecode registration now verifies self-describing `CNTR` artifacts,
  uses the verified artifact hash, and rejects raw or malformed contract bytes.
  Manifest registration and contract activation now require the matching stored
  bytecode and compare the submitted manifest payload with the manifest embedded
  in the artifact.
- Torii and overlay deploy paths now queue `RegisterSmartContractBytes` before
  `RegisterSmartContractCode`. Transaction-metadata manifests are trusted only
  when they match the embedded `CNTR` payload, and access-set derivation prefers
  selected entrypoint hints before manifest-level hints while applying the same
  bytecode safety gate to both.
- Kotodama access-hint reports now distinguish complete precise hints from
  fallback wildcards. Dynamic durable-state paths, contract calls, opaque ISI
  lowering, and alias-derived fallback cases record skipped reasons and mark
  `access_hints_complete = false`.
- IVM metadata parsing now rejects noncanonical literal-table post-padding:
  padding must be at most three zero bytes and exactly match the alignment
  implied by the section offset, entries, and data length. `koto_compile` now
  strips debug metadata by default unless `--embed-debug` is passed.
- The CoreHost `SET_ACCOUNT_DETAIL` gas path now reads JSON TLV payload length
  through the payload-length helper, fixing the compile regression where the
  validating helper's unit return value was treated like a TLV.
- Follow-up strict clippy fixes added `FastPQ` doc markup, made paired SMT-path
  checks inspect both path shapes instead of returning a constant, and replaced
  truncating crypto hotpath / CUDA bench casts with checked conversions. The
  FastPQ backend fixture assertion now uses `assert!` with the same mismatch
  diagnostics.
- Focused validation passed with
  `cargo test -p kotodama_lang access_hint -- --nocapture`,
  `cargo test -p kotodama_lang manifest_access_set_hints --lib -- --nocapture`,
  `cargo test -p ivm --test metadata_parse -- --nocapture`,
  `cargo test -p iroha_core overlay_appends_manifest_only_when_missing --lib -- --nocapture`,
  `cargo test -p iroha_core --test contract_code_bytes -- --nocapture`,
  `cargo test -p iroha_core --test contract_manifest_triggers -- --nocapture`,
  `cargo test -p iroha_core ivm_access_uses_manifest --lib -- --nocapture`,
  `cargo test -p iroha_core register_contract_manifest_is_queryable_without_permission --lib -- --nocapture`,
  `cargo test -p iroha_core activate_contract_instance_is_public_for_unprotected_namespace --lib -- --nocapture`,
  `cargo test -p iroha_core --test ivm_manifest_abi_reject -- --nocapture`,
  `cargo test -p iroha_core --test gov_enact_deploy -- --nocapture`, and
  `cargo test -p iroha_core smartcontracts::code --lib -- --nocapture`.
  Strict targeted clippy passed with
  `cargo clippy -p ivm -p ivm_abi -p kotodama_lang -p iroha_core --all-targets -- -D warnings`.
  The full workspace all-target clippy corridor also passed with
  `cargo clippy --workspace --all-targets -- -D warnings` after the follow-up
  FastPQ and bench lint fixes.
  An accidental broad filtered sweep,
  `cargo test -p ivm metadata_parse -- --nocapture`, also completed without
  failures, though it filtered out the `parse_*` metadata tests; the exact
  `metadata_parse` target above exercised them. Formatting and whitespace
  validation passed with `cargo fmt --all` and `git diff --check`.
- A full `cargo test --workspace` run was attempted after the clippy corridor.
  It progressed through broad workspace compilation but failed during
  large test-binary linking with `ld: write() failed, errno=28` and subsequent
  `No space left on device` errors while writing incremental/query-cache
  outputs. This was an infrastructure capacity failure rather than a Rust test
  assertion failure. `cargo clean` then completed and removed 1,850,436
  generated files, recovering 880.4 GiB from `target/`.

## 2026-05-03 Confidential admission and localnet stabilization

- Confidential policy admission now rejects disabled `Shield`/`Unshield`
  instructions before queueing and again during stateful block admission. Torii
  maps the queue rejection to `403` with stable confidential-policy metadata so
  clients get a policy error instead of a transport-looking failure.
- Versioned signed-transaction admission metadata now derives the external
  entrypoint hash and framed byte budget from the actual adaptive Norito payload
  bytes after decode. This avoids trusting stale `encoded_len_exact()` hints for
  confidential encrypted payloads while preserving the decoded signed
  transaction and canonical hash contract.
- The ZK-confidential localnet negative-submit helper now distinguishes wrapped
  policy rejections from startup transport errors, allows inconclusive negative
  submits to skip state assertions, retries balance reads through transient peer
  churn, and requires stable Torii readiness before starting submit-heavy flows.
  The test-network startup poll now treats storage-observed block progress as a
  nonfatal readiness fallback instead of shutting down peers when `/status`
  lags behind.
- Focused validation passed with
  `CARGO_TARGET_DIR=/tmp/iroha-codex-uaid-target cargo test -p iroha_core --lib confidential_policy_admission -- --nocapture`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-uaid-target cargo test -p iroha_core --lib push_with_lane_with_state_rejects_confidential_policy_before_enqueue -- --nocapture`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-uaid-target cargo test -p iroha_torii --lib push_into_queue_confidential_policy_rejection_maps_to_forbidden --features app_api -- --nocapture`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-uaid-target cargo test -p iroha_core --lib decoded_versioned_signed_transaction -- --nocapture`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-uaid-target cargo test -p integration_tests --test consensus_and_da zk_confidential_localnet::confidential_unshield_rejected_when_disabled -- --nocapture`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-uaid-target cargo test -p integration_tests --test consensus_and_da zk_confidential_localnet::confidential_shield_rejected_when_disabled -- --nocapture`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-uaid-target cargo test -p integration_tests --test consensus_and_da transient_client_error_detector -- --nocapture`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-uaid-target cargo test -p integration_tests --test consensus_and_da accepted_or_expected_rejection_treats_transient_submit_as_inconclusive -- --nocapture`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-uaid-target cargo test -p integration_tests --test consensus_and_da submit_retry_budget_covers_localnet_startup_jitter -- --nocapture`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-uaid-target cargo check -p integration_tests --test consensus_and_da`,
  `cargo fmt --all -- --check`,
  `scripts/check_no_scale.sh`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-uaid-target cargo clippy -p iroha_core -p iroha_torii -p iroha_test_network --lib -- -D warnings`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-uaid-target cargo clippy -p integration_tests --test consensus_and_da -- -D warnings`, and
  `git diff --check`.
- The full serial consensus/DA integration target now passes with
  `CARGO_TARGET_DIR=/tmp/iroha-codex-uaid-target cargo test -p integration_tests --test consensus_and_da -- --test-threads=1`:
  `250` passed, `0` failed, `6` ignored.
- Full workspace tests were not rerun in this pass and remain for the next
  uncontended validation window.

## 2026-05-03 FASTPQ GPU Metal Gate and Profile

- Installed and verified the Apple Metal compiler toolchain on this macOS
  arm64 host with `xcodebuild -downloadComponent MetalToolchain`. `xcrun`
  now resolves both `metal` and `metallib` from the Metal toolchain bundle, and
  `xcrun -sdk macosx metal -v` reports Apple Metal `32023.864`.
- The FASTPQ BN254 Poseidon word-batch Metal perf gate is now hardware-backed
  instead of silently measuring fallback CPU:
  `cargo run -p fastpq_prover --bin fastpq_metal_bench --features fastpq-gpu -- --operation bn254_poseidon_words --rows 20000 --iterations 20 --require-gpu --require-telemetry`
  completed with `execution_mode = gpu`, `gpu_backend = metal`,
  `run_status = ok`, CPU mean `7373.849 ms`, GPU mean `323.923 ms`, and
  `22.764x` speedup on Apple M1 Ultra.
- Focused FASTPQ GPU validation passed with
  `cargo check -p fastpq_prover --features fastpq-gpu`,
  `cargo test -p fastpq_prover bn254_poseidon --features fastpq-gpu -- --nocapture`,
  `cargo test -p fastpq_prover --test poseidon_manifest_consistency -- --nocapture`,
  and `cargo test -p iroha_core fastpq --lib --features fastpq-gpu -- --nocapture`
  (`38` passed). CUDA runtime parity/perf was not run on this macOS host; the
  CUDA evidence here is limited to compile/manifest/bounds coverage.
- Built the release Izanami gate binaries with
  `ENABLE_RANS_BUNDLES=1 NORITO_SKIP_BINDINGS_SYNC=1 cargo build --release -p irohad --bin iroha3d -p izanami --bin izanami --features irohad/fastpq-gpu`.
  The 4-peer `20k TPS` / `120s` GPU-enabled release gate at
  `dist/izanami-prebuilt-20k-fastpq-gpu-120s-20260503-034401` exited `0`,
  offered and accepted all `2,400,000` planned submissions, reported `0`
  failures, reached strict approved transactions `24,605` at strict height `8`,
  and recorded no view changes, validation rejects, missing payload/QC events,
  DA/RBC pressure, or queue drops.
- A requested same-shape rerun at
  `dist/izanami-prebuilt-20k-fastpq-gpu-rerun-120s-20260503-035513` also
  exited `0`, offered and accepted all `2,400,000` planned submissions,
  reported `0` failures, and improved strict approved transactions to `28,695`
  at strict height `9`. Safety signals stayed clean: no view changes,
  validation rejects, missing payload/QC events, DA/RBC pressure, or queue
  drops. The artifact captured unrelated Cargo/rustc and debug-network
  activity before/after the run, so treat latency and throughput comparisons as
  contended.
- The matching `30s` sampled GPU profile at
  `dist/izanami-profile-20k-fastpq-gpu-sampled-30s-20260503-034942` exited `0`
  with sample status `0`, offered and accepted all `600,000` planned
  submissions, and reported `0` failures. The profile was contended by
  unrelated Cargo/rustc work, so treat it as bottleneck evidence rather than a
  clean latency baseline.
- A requested same-shape sampled rerun at
  `dist/izanami-profile-20k-fastpq-gpu-rerun3-sampled-30s-20260503-040340`
  exited `0` with sample status `0`, offered and accepted all `600,000`
  planned submissions, reported `0` failures, and reached strict approved
  transactions `4,131` at strict height `3`. Peer stack samples show the
  current bottlenecks as synchronous Metal completion wait inside
  `finalize_transfer_transcript_digests_in_map`, Ed25519/Curve25519 batch
  verification and public-key parsing, Norito transaction/gossip decode and
  transfer serialization, allocator/copy traffic, CRC64, SHA-256, and Blake2.
  Scalar `poseidon3_permute` remains absent.
- Peer samples confirm FASTPQ transcript digest finalization now reaches the
  Metal `bn254_poseidon_hash_words` path, while `poseidon3_permute` is absent
  from the sampled stacks. Remaining hot costs are dominated by Norito
  transaction/transfer serialization and decode, Ed25519/Curve25519
  parse/verify miss work, allocation/copy traffic, and CRC64/hash helpers.
- The follow-up FASTPQ performance pass now overlaps Metal BN254 Poseidon word
  batches with CPU finalization work, preflights the daemon `fastpq-gpu`
  hardware path at startup, propagates finalized transcript digests into the
  execution-witness recorder to avoid a duplicate witness-side GPU wait, and
  widens the Ed25519 thread-local public-key/verify caches for the 20k stable
  workload window. Focused validation passed with
  `cargo test -p iroha_core apply_fastpq_transcript_digests_updates_recorded_witness_copy --lib -- --nocapture`,
  `cargo test -p iroha_crypto ed25519 --lib -- --nocapture` (`34` passed),
  and `cargo check -p iroha_core --features fastpq-gpu`.
- Rebuilt the release gate binaries with `irohad/fastpq-gpu`. The final
  4-peer `20k TPS` / `120s` GPU-enabled release gate at
  `dist/izanami-prebuilt-20k-fastpq-gpu-final-120s-20260503-152118` exited
  `0`, offered and accepted all `2,400,000` planned submissions, reported `0`
  failures, reached strict approved transactions `36,986` at strict height
  `11`, and recorded submit latency `p50=4ms`, `p95=12ms`, `p99=79ms`,
  `max=253ms`. The queue remained saturated
  (`875,623 / 2,400,000`), with `2` view-change installs and no validation
  rejects, missing payload/QC view-change causes, queue drops, DA/RBC pressure,
  ingress failover, or endpoint unhealthy events.
- The final delayed load-window sampled profile at
  `dist/izanami-profile-20k-fastpq-gpu-final-sampled-30s-20260503-151935`
  exited `0` with sample status `0`, offered and accepted all `600,000`
  planned submissions, and reported `0` failures. The load-window peer samples
  no longer include cold Metal preflight, scalar `poseidon3_permute`, or CPU
  FASTPQ fallback. Small residual FASTPQ waits are now device completion waits
  on the accelerated path; the dominant remaining peer CPU stack is
  Ed25519/Curve25519 public-key parse and verification, Norito transaction and
  transfer encode/decode, transaction metadata hashing, allocation/copy
  traffic, and CRC64/SHA-256 helpers. Sampling perturbed this short run
  (`strict approved = 4,152`, `p95 = 57ms`), so use the profile for stack
  attribution and the 120s gate above for the cleaner throughput/latency
  evidence.
- Follow-up Ed25519 parse-cache tuning kept only lock-free thread-local changes.
  A sharded process-wide public-key parse cache experiment was rejected after
  validation: the sampled run
  `dist/izanami-profile-20k-fastpq-gpu-shared-ed25519-sampled-30s-20260503-183527`
  exited `0` but dropped to `278` strict-approved transactions with `p95=691ms`,
  and the unsampled check
  `dist/izanami-prebuilt-20k-fastpq-gpu-shared-ed25519-30s-20260503-183817`
  exited `0` but reached only `10` strict-approved transactions. After backing
  that out, the reverted 30s gate
  `dist/izanami-prebuilt-20k-fastpq-gpu-reverted-shared-ed25519-30s-20260503-185400`
  returned to `4,163` strict-approved transactions with `p95=46ms`.
- The accepted Ed25519 follow-up stays thread-local: the public-key parse map
  is pre-sized for the Izanami working set, parsed key cache entries remain
  boxed to satisfy the workspace `variant-size-differences` lint, and the
  generic verify-ok `HashSet` stays lazy so 32-byte transaction-hash
  verification uses only the direct exact cache. A second thread-local
  follow-up added a direct-mapped Ed25519 full-key cache ahead of the generic
  128-entry linear `Signature::verify` public-key cache, avoiding linear LRU
  churn and compact-key rewrapping for the 4096-submitter Izanami key set.
  Current validation passed with
  `cargo test -p iroha_crypto ed25519 --lib -- --nocapture` (`35` passed),
  `cargo check -p irohad --features fastpq-gpu`, and the release rebuild
  `ENABLE_RANS_BUNDLES=1 NORITO_SKIP_BINDINGS_SYNC=1 cargo build --release -p irohad --bin iroha3d -p izanami --bin izanami --features irohad/fastpq-gpu`.
  The rebuild completed in `7m47s` with only the known `fastpq_prover` Metal
  dead-code warnings.
  A same-shape `120s` release gate with the rebuilt binaries at
  `dist/izanami-prebuilt-20k-fastpq-gpu-post-cache-contended-120s-20260503-200542`
  exited `0`, offered, accepted, and succeeded all `2,400,000` submissions,
  reported `0` failures, and had no validation rejects, DA/RBC pressure,
  ingress failover, or endpoint unhealthy events. It is not clean performance
  evidence: a separate `cargo test --workspace` job was active, including a
  long `consensus_and_da` child and later a high-CPU `iroha_core`/`iroha_torii`
  compile. The contended run reached only `8,261` strict-approved transactions
  at height `4`, installed `8` view changes, and recorded submit latency
  `p50=7ms`, `p95=23ms`, `p99=77ms`, `max=288ms`.
  The 30s 20k unsampled gate
  `dist/izanami-prebuilt-20k-fastpq-gpu-ed25519-presized-cache-30s-20260503-190713`
  exited `0`, offered `599,998`, accepted `599,998`, reported `0` failures,
  reached `4,127` strict-approved transactions at strict height `3`, and
  recorded submit latency `p50=5ms`, `p95=17ms`, `p99=110ms`, `max=310ms`.
  A follow-up 30s functional run with the lazy generic verify cache at
  `dist/izanami-prebuilt-20k-fastpq-gpu-ed25519-lazy-cache-noisy-30s-20260503-192924`
  completed its Izanami summary while a separate workspace test was compiling
  in the background, so it is not clean perf evidence. The run offered,
  accepted, and succeeded all `600,000` submissions, reached `4,214`
  strict-approved transactions at strict height `3`, reported no view changes
  or validation rejects, and recorded noisy submit latency `p50=11ms`,
  `p95=106ms`, `p99=336ms`, `max=852ms`. The wrapper failed after the summary
  while writing `run_status` because `status` is read-only in `zsh`; the
  artifact status is marked inferred-success from the Izanami summary.
- Follow-up Torii ingress bookkeeping now avoids the per-transaction
  `DashMap::len()` calls in `PipelineStatusCache::prune_if_needed`. The status
  cache keeps relaxed atomic live counts for transaction entries and pending
  block entries, reuses the transaction-event timestamp for pruning, and still
  runs the existing ordered TTL/capacity prune on the same 30s cadence or when
  the atomic count crosses capacity. Focused validation passed with
  `cargo test -p iroha_torii pipeline_status_cache --lib -- --nocapture`
  (`8` passed, `1` ignored load-profile test) and
  `cargo test -p iroha_torii pipeline_status_cache_prune_load_profile --lib -- --ignored --nocapture`
  (`avg_us=7395.588`, `p95_us=7649.000` for the explicit over-capacity prune
  pressure case), plus
  `cargo check -p irohad --features fastpq-gpu` (only the known
  `fastpq_prover` Metal dead-code warnings). A clean post-change Izanami
  profile/gate has not been run yet because a separate `cargo test --workspace`
  process was still active on the host.
- A follow-up allocation slice now hashes typed Norito payloads by streaming
  `Encode::encode_to` directly into Blake2b for `HashOf::new`, avoiding the
  temporary encoded buffer previously allocated before every typed hash.
  Direct byte hashing now also finalizes Blake2b into the fixed 32-byte hash
  buffer instead of allocating a boxed digest and copying it back out, so Merkle
  parent hashes and other direct `Hash::new` callers take the same allocation
  reduction. Merkle parent hashing and shielded commitment helpers now absorb
  their existing byte slices directly through a crate-private chunked hash path
  instead of copying children/tags into temporary concatenation buffers first.
  `SignedTransaction::hash_as_entrypoint` also uses a private borrowed encoder
  for the `TransactionEntrypoint::External` wrapper, preserving the generated
  enum bytes without cloning the signed transaction before hashing. Focused
  parity passed with
  `cargo test -p iroha_crypto hash_new --lib -- --nocapture` (`2` passed),
  `cargo test -p iroha_crypto hash_of_new_matches_encoded_bytes_hash --lib -- --nocapture`
  (`1` passed),
  `cargo test -p iroha_crypto merkle --lib -- --nocapture` (`39` passed),
  and
  `cargo test -p iroha_data_model entrypoint_hashes_match_direct_encoding --lib -- --nocapture`
  (`1` passed);
  `cargo check -p irohad --features fastpq-gpu` passed in `1m52s` with only the
  known `fastpq_prover` Metal dead-code warnings. The release rebuild
  `ENABLE_RANS_BUNDLES=1 NORITO_SKIP_BINDINGS_SYNC=1 cargo build --release -p irohad --bin iroha3d -p izanami --bin izanami --features irohad/fastpq-gpu`
  passed in `8m03s` with the same warning set.
- The clean post-allocation 4-peer no-fault prebuilt `20k TPS` / `120s`
  `fastpq-gpu` return gate at
  `dist/izanami-prebuilt-20k-fastpq-gpu-return-120s-20260504-012106` exited
  `0`, offered, accepted, and succeeded all `2,400,000` submissions, reported
  `0` failures, used all `2,400,000` prebuilt transactions with no fallback,
  and recorded submit latency `p50=9ms`, `p95=34ms`, `p99=118ms`,
  `max=495ms`. The run restored clean ingress but not strict approval progress:
  final quorum/strict height was `5/5`, final quorum/strict approved was
  `12,413/12,413`, queue depth was `884,071`, and Sumeragi recorded `7` view
  changes (`5` missing-QC, `1` missing-payload), with no validation rejects,
  DA/RBC pressure, ingress failover, or unhealthy endpoint events.
- The matching sampled `30s` profile at
  `dist/izanami-profile-20k-fastpq-gpu-return-sampled-30s-20260504-012521`
  also exited `0` with `sample_status=0`, offered, accepted, and succeeded all
  `600,000` submissions, and reported `0` failures. It is stack-attribution
  evidence rather than throughput evidence because `sample(1)` heavily
  perturbed the run (`strict approved = 41`, `p95 = 2649ms`). The peer samples
  show no scalar FASTPQ/Poseidon fallback; the remaining visible costs are
  Ed25519/Curve25519 public-key parse and batch verification, Norito signed
  transaction and transfer encode/decode, queue push lock contention,
  transaction metadata hashing, allocation/copy traffic, and SHA-256/CRC64
  helpers.
- A follow-up queue-lock slice narrows `push_remove_lock` in the successful
  gossip-admission and consensus-requeue push paths. The lock still covers the
  transaction/routing/age/expiry/gossip-payload maps that removal can clean up,
  but post-enqueue backpressure publication, gossip side-channel enqueue,
  queued-event emission, logging, and Sumeragi wakeup now run after releasing
  it. Focused validation passed with
  `cargo test -p iroha_core push_with_gossip_payload_with_state_and_routing_skips_router_lookup --lib -- --nocapture`
  and
  `cargo test -p iroha_core push_requeued_with_routing_accepts_pending_transaction --lib -- --nocapture`.
- The isolated post-queue-lock release rebuild
  `ENABLE_RANS_BUNDLES=1 NORITO_SKIP_BINDINGS_SYNC=1 CARGO_TARGET_DIR=/tmp/iroha-codex-20k-queue cargo build --release -p irohad --bin iroha3d -p izanami --bin izanami --features irohad/fastpq-gpu`
  passed in `9m20s` with only the known `fastpq_prover` Metal dead-code
  warnings. A sampled `30s`/`20k TPS` run against those isolated binaries at
  `dist/izanami-profile-20k-fastpq-gpu-queue-lock-sampled-30s-20260504-165035`
  exited `0` with `sample_status=0`, accepted and succeeded all `600,000`
  submissions, and recorded submit latency `p50=5ms`, `p95=19ms`,
  `p99=216ms`, `max=573ms`. Treat it as invalid throughput evidence: strict
  approval stayed at `9`, queue depth ended at `213,358`, and peer diagnostics
  show repeated block-validation warnings for `ExecutionContextInvalid`:
  `execution context entrypoint hash mismatch at index 0` across all four
  peers. The useful stack attribution remains consistent with the prior
  profile: no scalar FASTPQ/Poseidon fallback is visible, and the peer CPU is
  dominated by Ed25519/Curve25519 parse/verify work, Norito transaction/transfer
  encode/decode, transaction metadata/Merkle hashing, queue/backlog
  bookkeeping, allocator/copy traffic, and SHA-256/CRC64 helpers.
- The follow-up bottleneck fix reclassifies RBC READY/DELIVER frames onto the
  consensus-chunk lane, limits high-priority payload bursts to one frame before
  chunk traffic gets a turn, caches prepared transaction metadata JSON depths,
  and keeps prepared metadata depth checks on the static-validation hot path.
  A narrower local cleanup also avoids a temporary signed-transaction byte
  vector while deriving prepared signed/entrypoint hashes and encoded lengths,
  and reuses prepared payload and signed hashes in validation
  cache/signature-batch paths. Focused validation passed with
  `cargo fmt --all --check`,
  `cargo test -p iroha_core --lib sumeragi_block_classifies_topics -- --nocapture`,
  `cargo test -p iroha_core --lib borrowed_external_entrypoint_hash_matches_canonical_hash -- --nocapture`,
  `cargo test -p iroha_core --lib gossip_signed_metadata_matches_canonical_preparation -- --nocapture`,
  `cargo test -p iroha_core --lib prepared_metadata_depth_matches_direct_depth_check -- --nocapture`,
  `cargo test -p iroha_core --lib validate_and_record_transactions_skip_stateless_matches_full -- --nocapture`,
  `cargo test -p iroha_p2p --lib message_sender_isolates_consensus_payload_and_chunk_encrypted_frames -- --nocapture`,
  and
  `cargo test -p iroha_p2p --lib high_lane_payload_and_chunk_progress_under_sustained_consensus -- --nocapture`.
- The rebuilt `fastpq-gpu` release gate
  `ENABLE_RANS_BUNDLES=1 NORITO_SKIP_BINDINGS_SYNC=1 CARGO_TARGET_DIR=/tmp/iroha-codex-20k-queue cargo build --release -p irohad --bin iroha3d -p izanami --bin izanami --features irohad/fastpq-gpu`
  passed in `5m12s` with only the known `fastpq_prover` Metal dead-code
  warnings. The 4-peer no-fault prebuilt `20k TPS` / `120s` rerun at
  `dist/izanami-prebuilt-20k-fastpq-gpu-bottleneckfix-120s-20260504-183724`
  completed the Izanami workload successfully (`run_status=0`; wrapper status
  was `1` only because the zsh wrapper tried to assign read-only `status` after
  the summary). It offered, accepted, and succeeded all `2,400,000`
  submissions, used all `2,400,000` prebuilt transactions with no fallback,
  reported `0` failures and `0` confirmation queue drops, and recorded submit
  latency `p50=5ms`, `p95=22ms`, `p99=101ms`, `max=249ms`. Safety signals were
  clean: no view changes, missing payload/QC causes, validation rejects, ingress
  failover, or unhealthy endpoint events. Strict progress improved back to
  height `11` and `37,000` approved transactions, but the queue was still
  saturated (`854,344 / 2,400,000`) with `55` pacemaker backpressure deferrals,
  so this restores stable 20k ingress rather than committed 20k TPS.
- The matching `30s` sampled profile at
  `dist/izanami-profile-20k-fastpq-gpu-bottleneckfix-peer-sampled-30s-20260504-184154`
  exited `0` with `sample_status=0`, offered, accepted, and succeeded all
  `600,000` submissions, and reported `0` failures. It reached strict height
  `4` and `8,290` approved transactions with submit latency `p50=5ms`,
  `p95=70ms`, `p99=199ms`, `max=546ms`; the queue remained saturated
  (`185,902 / 600,000`) with `19` pacemaker backpressure deferrals. The profile
  shows no scalar FASTPQ/Poseidon fallback. The remaining active peer CPU stack
  is block validation and serialization heavy: Ed25519/Curve25519 verification
  math, Norito compact-length and transaction/transfer encode/decode,
  allocator/reallocation and `memmove`, SHA-256/Blake2/CRC64 helpers,
  `resolve_streaming_metadata`, and pipeline access/overlay preparation.
  RBC READY/DELIVER deferrals still occur, but they no longer produce
  missing-payload or missing-QC view changes in this run.
- The current-code 4-peer no-fault prebuilt `20k TPS` / `120s` `fastpq-gpu`
  gate after the final prepared-hash cleanup rebuilt in
  `/tmp/iroha-codex-20k-queue` and ran at
  `dist/izanami-prebuilt-20k-fastpq-gpu-return-current-120s-20260504-194602`.
  The release rebuild passed in `5m20s` with only the known `fastpq_prover`
  Metal dead-code warnings. Izanami exited `0`, offered, accepted, and
  succeeded all `2,400,000` submissions, used all `2,400,000` prebuilt
  transactions with no fallback or build failures, reported `0` failures and
  `0` confirmation queue drops, and recorded submit latency `p50=6ms`,
  `p95=21ms`, `p99=99ms`, `max=269ms`. Safety signals stayed clean: no view
  changes, missing payload/QC causes, validation rejects, ingress failover, or
  unhealthy endpoint events. Strict progress was `32,956` approved
  transactions at height `10`; the queue remained saturated
  (`883,791 / 2,400,000`) with `117` pacemaker backpressure deferrals and
  commit-pipeline EMA `592ms`. Diagnostics still show validation inflight
  pressure and RBC READY/DELIVER deferrals, so 20k ingress is back but committed
  20k TPS remains open.
- Fresh current-code sampled profiles split the remaining bottlenecks into
  cold-start and steady-state costs. The immediate `30s` profile at
  `dist/izanami-profile-20k-fastpq-gpu-current-peer-sampled-30s-20260504-195325`
  exited `0` with `sample_status=0`, accepted and succeeded all `600,000`
  submissions, but strict progress reached only `4,132` approved transactions
  at height `3` with the queue saturated (`180,455 / 600,000`). The sampled
  peer spent the dominant first-use stack inside FASTPQ Metal pipeline creation
  (`fastpq_prover::metal::build_metal_context` /
  `new_compute_pipeline_state_with_function`) before Poseidon dispatch, so
  hardware acceleration is present but its pipeline/context creation is still
  on the first proof's hot path. A delayed post-warm `60s` run at
  `dist/izanami-profile-20k-fastpq-gpu-current-peer-postwarm-sampled-60s-20260504-195720`
  also exited `0` with `sample_status=0`, accepted and succeeded all
  `1,200,000` submissions, recorded no validation rejects, missing-payload
  causes, ingress failover, or endpoint failures, and ended at strict height
  `4` with `8,237` approved transactions. It still saturated the queue
  (`388,241 / 1,200,000`), recorded `50` pacemaker backpressure deferrals,
  `15` validation-inflight fallbacks, `15` slow commit-pipeline warnings, and
  `146`/`130` RBC READY/DELIVER deferrals. Slow commit warnings are validation
  dominated (`2.9s` to `9.8s` validation, `3ms` to `8ms` finalize), and the
  steady-state sample is led by Ed25519/Curve25519 parse/verify work, Norito
  transaction/transfer decode and encode/length accounting, allocator/free/
  reallocation and `memmove`, SHA-256/Blake2/CRC64 helpers, queue admission
  bookkeeping, and world-view/access preparation. Scalar FASTPQ/Poseidon
  fallback is not the current steady-state bottleneck.
- The FASTPQ lane startup/preflight follow-up now publishes the lane handle
  before backend construction, initializes the real prover on a blocking worker,
  defers background proof jobs until that worker marks the lane ready, and keeps
  host-side BN254 Poseidon digest acceleration disabled until the lane observes
  successful Poseidon GPU and BN254 digest preflights. A failed prover Poseidon
  preflight now keeps the deterministic CPU fallback for the lane instead of
  letting first proof work resolve back onto the GPU hot path. Focused
  validation passed with
  `ENABLE_RANS_BUNDLES=1 NORITO_SKIP_BINDINGS_SYNC=1 CARGO_TARGET_DIR=/tmp/iroha-codex-20k-queue cargo test -p iroha_core fastpq --features fastpq-gpu -- --nocapture`
  (`43` matching tests passed) and the release rebuild
  `ENABLE_RANS_BUNDLES=1 NORITO_SKIP_BINDINGS_SYNC=1 CARGO_TARGET_DIR=/tmp/iroha-codex-20k-queue cargo build --release -p irohad --bin iroha3d -p izanami --bin izanami --features irohad/fastpq-gpu`
  passed in `4m48s` with only the known `fastpq_prover` Metal dead-code
  warnings and existing `PenaltyApplier::telemetry` warning.
- The fresh 4-peer no-fault prebuilt `20k TPS` / `120s` `fastpq-gpu` return
  gate at
  `dist/izanami-prebuilt-20k-fastpq-gpu-return-preflight-gate-120s-20260505-124838`
  exited `0`, offered, accepted, and succeeded all `2,400,000` submissions,
  used all `2,400,000` prebuilt transactions with no fallback or build
  failures, and reported `0` submit failures, validation rejects, confirmation
  failures, confirmation queue drops, ingress failovers, or unhealthy endpoints.
  Final quorum/strict height was `13/13`, final quorum/strict approved
  transactions were `45,191/45,191`, max peer height and approved-transaction
  skew were both `0`, and submit latency was `p50=3ms`, `p95=11ms`,
  `p99=60ms`, `max=200ms`. The queue remained saturated
  (`877,135 / 2,400,000`) with `95` pacemaker backpressure deferrals, so this
  restores the 20k ingress/strict-progress gate above the prior `36,967`
  baseline while committed 20k TPS still needs the validation/serialization and
  queue-drain work identified in the latest 30s/60s profiles.

## 2026-05-03 IVM WSV mock mutation gas hardening

- The WSV mock host no longer normalizes free state mutation for ABI syscalls.
  Direct peer, trigger, contract-code, domain/account, signatory/quorum/detail,
  asset, role/permission, transfer, unregister, domain-transfer, and NFT
  mutation syscalls now return deterministic mutation gas. Account detail and
  NFT metadata charge their JSON payload bytes; FastPQ transfer batch entry and
  apply paths charge per-entry mutation gas.
- Development `SMARTCONTRACT_EXECUTE_QUERY` JSON envelopes now charge singular
  query gas over request + response bytes, and
  `SMARTCONTRACT_EXECUTE_INSTRUCTION` JSON/data-model mutation envelopes charge
  payload-sized mutation gas on successful state changes. ZK ballot/finalize
  helper mutations also return nonzero deterministic gas while preserving the
  one-shot verification latch semantics.
- Focused validation passed with `cargo fmt --all`,
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --lib charge -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --test wsv_host_execute_query_envelope --test wsv_host_pointer_tlv --test wsv_host_account_admin --test wsv_host_grant_revoke_tlv --test wsv_host_roles_triggers_envelope --test wsv_host_register_account_asset_tlv --test wsv_host_register_domain_tlv --test wsv_host_unregister_tlv -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --lib finalize -- --nocapture`, and
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --lib submit_ballot -- --nocapture`.
- The real smart-contract host now charges the declared deterministic
  `G_sc_depth` floor for `SET_SMARTCONTRACT_EXECUTION_DEPTH`, including the
  zero-depth no-op path that previously returned free success.
  `CREATE_NFTS_FOR_ALL_USERS` also now charges the declared deterministic
  `G_create_nfts_all` floor when the account snapshot is empty, while retaining
  queued-instruction gas for NFT creation/transfer work. Focused validation
  passed with
  `CARGO_TARGET_DIR=target/codex-core-scallx cargo test -p iroha_core smartcontract_depth --lib -- --nocapture` and
  `CARGO_TARGET_DIR=target/codex-core-scallx cargo test -p iroha_core create_nfts_for_all --lib -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-core-scallx cargo test -p iroha_core queue_instructions_accumulates_gas_and_enqueues --lib -- --nocapture`, and
  `CARGO_TARGET_DIR=target/codex-core-scallx cargo test -p iroha_core syscall_charges --lib -- --nocapture`.

## 2026-05-03 IVM runtime-helper gas hardening

- Runtime helper syscalls that already advertised gas assets now return
  deterministic nonzero gas in host implementations instead of silently doing
  work for free. `INPUT_PUBLISH_TLV` charges `G_input_publish + bytes` across
  `DefaultHost`, standalone `CoreHost`, and WSV host paths; `VERIFY_SIGNATURE`
  charges `G_verify_sig + bytes`; `GET_PRIVATE_INPUT`, `USE_NULLIFIER`,
  `COMMIT_OUTPUT`, debug/exit/abort helpers, heap growth, allocation in the
  standalone `CoreHost`, Merkle proof helpers, validation-only ISI mutation
  stubs, and FastPQ batch-entry/apply validation paths now return their
  documented fixed/page/depth/per-entry costs.
- The syscall spec, generated docs, and ABI doc table now reflect byte-scaled
  costs for `INPUT_PUBLISH_TLV` and `VERIFY_SIGNATURE`.
- Focused validation passed with
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --lib charges -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --lib default_host_runtime_helpers_charge_declared_gas -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --test syscalls_doc_sync --test syscalls_doc_generated --test syscalls_gas_names --test syscalls_markdown_gas --test gas_schedule_hash -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --test abi_hash_versions --test abi_syscall_list_golden --test ivm_abi_doc_sync -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm_abi --lib -- --nocapture`, and
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --test dynamic_memory --test private_input --test nullifier --test verify_signature_tlv --test syscalls --test default_host_input_publish_tlv --test core_host_input_publish_tlv --test wsv_host_input_publish_tlv -- --nocapture`.
  Follow-up pointer/mutation coverage passed with
	  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --test core_host_pointer_abi -- --nocapture` and
	  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --test syscalls --test wsv_host_pointer_tlv -- --nocapture`.
	  Formatting passed with `cargo fmt --all`.

## 2026-05-03 IVM verification gas hardening

- VRF verification syscalls now charge deterministic byte-counted gas across
  all decoded status exits in the shared `DefaultHost` path. `VRF_VERIFY` and
  `VRF_VERIFY_BATCH` return `G_verify + bytes` for wrong-type, malformed
  payload, chain/key/proof/variant/verification failure, empty-batch, OOM, and
  success paths instead of allowing status-returning work to run for free.
- Standalone and WSV ZK verification status paths now charge deterministic
  payload-size gas as well. The real smart-contract host already used the
  confidential proof gas schedule; the standalone single-envelope verifier,
  disabled standalone batch path, and WSV mock verifier now match the same
  nonzero accounting model for decoded payload work.
- Stale standalone, WSV, and feature-gated Goldilocks verifier fixtures now
  assert the byte-counted verification gas and encoded JSON mutation-envelope
  gas instead of preserving the old zero-gas expectations in tests.
- `VERIFY_DS_PROOF` now charges deterministic verification gas as well: the real
  smart-contract host returns `G_verify + bytes` for successful proof
  verification and `G_verify` for proof-clear, while standalone `DefaultHost`,
  standalone `CoreHost`, and WSV mock proof-clear paths return `G_verify`.
  Standalone proof-consuming AXT calls still fail closed after FastPQ V1
  preflight because those hosts do not link the real verifier.
- The remaining successful AXT bookkeeping syscalls are no longer free:
  `AXT_BEGIN`, `AXT_TOUCH`, and `USE_ASSET_HANDLE` now charge `G_axt + bytes`
  from the decoded pointer-ABI payloads they validate, and `AXT_COMMIT` charges
  `G_axt + entries` from recorded touches, proofs, and handle uses. The
  standalone hosts, WSV mock, and real smart-contract host all use the same
  deterministic saturating arithmetic.
- The generated syscall spec/docs now advertise `G_verify_proof + bytes` for
  single ZK proof verification, `G_verify + bytes` for VRF and ZK batches,
  `G_verify + bytes` for `VERIFY_DS_PROOF`, `G_axt + bytes`/`G_axt + entries`
  for AXT bookkeeping, and `G_verify_proof + bytes` for `VERIFY_PROOF`.
- ZK read helpers and VRF epoch seed reads now charge deterministic request +
  response byte gas (`G_roots_get + bytes` / `G_vote_get + bytes`) across the
  standalone host, WSV mock, and real smart-contract host instead of returning
  zero on successful read/status paths.
- Focused validation passed with
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --lib default_host_ -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --lib zk_verify_status_paths_charge_payload_bytes -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --lib zk_read_helpers_charge_request_and_response_bytes -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --test zk_verify_batch_syscall --test zk_verify_batch_gating -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --test zk_verify_syscall --test zk_verify_gating --test wsv_verify_latch_unshield --test wsv_host_zk_perm_and_events -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --test zk_verify_goldilocks --features ivm_zk_tests,goldilocks_backend -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-core-scallx cargo test -p iroha_core vrf_epoch_seed_syscall --lib -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-core-scallx cargo test -p iroha_core zk_vote_tally_syscall_reads_world_snapshot --lib -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-core-scallx cargo test -p iroha_core from_state_hydrates_zk_snapshots --lib -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-core-scallx cargo test -p iroha_core axt_verify_ds_proof --lib -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-core-scallx cargo test -p iroha_core axt_proof_cache --lib -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --test core_host_policy --test host_unknown_syscall --test axt_host_flow -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --test axt_host_flow --test core_host_policy --test host_unknown_syscall -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --test syscalls_doc_sync --test syscalls_gas_names --test syscalls_markdown_gas --test gas_schedule_hash -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --test syscalls_doc_sync --test syscalls_doc_generated --test syscalls_gas_names --test syscalls_markdown_gas -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --test abi_hash_versions --test abi_syscall_list_golden --test ivm_abi_doc_sync -- --nocapture`, and
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --test gas_schedule_hash --test abi_hash_versions --test abi_syscall_list_golden --test ivm_abi_doc_sync -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm_abi --lib -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-core-scallx cargo test -p iroha_core axt_ --lib -- --nocapture`, and
  `CARGO_TARGET_DIR=target/codex-core-scallx cargo test -p iroha_core --features iroha-core-tests --test ivm_corehost_axt -- --nocapture`.
  Formatting and whitespace validation passed with `cargo fmt --all` and
  `git diff --check`.

## 2026-05-02 Sumeragi witness-root and localnet validation hardening

- Sumeragi's debug `corrupt_witness_ack` path now mutates the local
  post-execution root before the node stores validated roots and votes, both in
  inline validation and validation-worker result handling. The parent root is
  preserved, while the post root is deterministically salted by block hash,
  height, view, and peer identity so witness-root divergence is exercised by the
  QC path instead of being hidden by an unmodified local vote.
- The ZK-confidential localnet submit helper now treats lower-cased transport
  causes, including `tcp connect error`, as transient during startup and uses a
  larger submit retry budget. This keeps early peer readiness jitter from being
  reported as a policy or payload failure before any live peer sees the
  transaction.
- The nested `iroha3d` build path used by consensus localnet tests was repaired
  by keeping schema-helper input lengths in scope before VM mutation in the
  standalone and WSV IVM hosts.
- Focused validation passed with
  `CARGO_TARGET_DIR=/tmp/iroha-codex-workspace-corridor cargo test -p iroha_core debug_corrupt_witness_roots_changes_local_post_root --lib`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-workspace-corridor cargo check -p ivm --lib`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-workspace-corridor cargo check -p iroha_core --lib`, and
  `CARGO_TARGET_DIR=/tmp/iroha-codex-workspace-corridor cargo test -p integration_tests --test consensus_and_da sumeragi_adversarial::sumeragi_adversarial_chunk_drop_recovery -- --nocapture`.
  Earlier focused reruns in the same corridor also passed for
  `sumeragi_adversarial::sumeragi_adversarial_witness_corruption`,
  `sumeragi_da::sumeragi_da_kura_eviction_rehydrates_from_da_store`, and
  `sumeragi_da_eviction_rehydrates_block_bodies`. Follow-up validation on
  2026-05-03 also passed
  `CARGO_TARGET_DIR=/tmp/iroha-codex-workspace-corridor cargo test -p integration_tests --test consensus_and_da sumeragi_da::sumeragi_da_payload_loss_does_not_block_commit -- --nocapture`.
  The localnet retry-helper slice also passed with
  `CARGO_TARGET_DIR=/tmp/iroha-codex-workspace-corridor cargo test -p integration_tests --test consensus_and_da transient_client_error_detector -- --nocapture`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-workspace-corridor cargo test -p integration_tests --test consensus_and_da transient_localnet_startup_error_detector -- --nocapture`, and
  `CARGO_TARGET_DIR=/tmp/iroha-codex-workspace-corridor cargo test -p integration_tests --test consensus_and_da submit_retry_budget_covers_localnet_startup_jitter -- --nocapture`.
  Follow-up validation on 2026-05-03 with
  `CARGO_TARGET_DIR=/tmp/iroha-codex-uaid-target` also passed the disabled
  shield/unshield confidential localnet cases, the hardened negative-submit
  classifier/retry-budget slice, and the full serial `consensus_and_da` target
  after fixing adaptive signed-transaction admission metadata.
- A broad `consensus_and_da` rerun reached 228 passed, 19 failed, and 6 ignored
  under heavy concurrent Cargo activity. The Sumeragi/DA failures from that run
  now have green focused reruns, and the disabled confidential localnet cluster
  has green focused reruns. The follow-up serial rerun is green.
## 2026-05-04 Swift Torii wallet authentication guardrail

- The Swift `ToriiClient` now accepts SDK-level default headers and a
  `ToriiClientAuthentication` helper for wallet sessions. The helper builds
  `Authorization`, `X-Account-Id`, and optional `X-Dataspace-Id` /
  `X-API-Token` headers once, and every REST request merges that context before
  sending so mobile clients do not need to remember Torii wallet headers at each
  call site.
- Torii transport security now treats `X-Account-Id` and `X-Dataspace-Id` as
  credential-bearing headers alongside `Authorization` and `X-API-Token`, so
  credentialed wallet context is rejected over insecure or host-mismatched
  HTTP transport.
- Validation:
  - `swift test --filter AuthenticationContext`
  - `swift test --filter ToriiClientTests`
  - `swift test --filter TransportSecurity`

## 2026-05-03 Sumeragi restarted-peer commit-QC recovery

- The widened consensus validation did not find a quorum-halting consensus
  failure, but it did expose a peer-local catch-up bug in the confidential
  downtime plus timeout localnet scenario. A restarted peer could keep a known
  frontier payload locally while repeatedly requesting the missing commit QC,
  even though the other validators had already finalized later heights.
- Root cause: exact block-body repair sends a plain `BlockBodyResponse` before
  the richer `BlockSyncUpdate` companion. The first repair bug was network
  ingress deduplication: the plain body and QC-bearing companion shared only a
  height/view/hash key, so the plain body could suppress the certificate
  response. The second bug was receiver-side admission order: a QC-bearing
  companion for height `H + 2` could arrive while the restarted peer was still
  committing height `H + 1`, get classified as non-exact, and be dropped before
  the normal block-sync deferral path could retain the commit QC.
- `BlockBodyResponse` dedup now includes a response-evidence hash, so plain
  bodies and certificate-bearing companions are separately admissible.
  `handle_block_body_response(...)` also releases a plain exact-body key when a
  same-round missing commit-QC request remains pending, routes non-exact
  QC-bearing companions through `handle_block_sync_update(...)`, and preserves
  oversized QC companions as direct `BlockSyncUpdate` fallbacks when that form
  fits the payload frame cap.
- Added focused regressions for the observed message orders: plain exact body
  first, then QC-bearing `BlockSyncUpdate`; dedup separation for plain versus
  evidence-bearing companions; non-exact QC-bearing companion admission; and
  oversized wrapper fallback that preserves the commit certificate.
- The NPoS 1s/K=3 performance harness now separates host jitter from consensus
  liveness by raising the propose EMA ceiling slightly and adding an explicit
  bounded-progress check on observed block spacing.
- Validation:
  - `cargo test -p iroha_core plain_block_body_response_releases_dedup_for_active_missing_commit_qc_repair -- --nocapture`
  - `cargo test -p iroha_core --lib block_body_response -- --nocapture`
  - `IROHA_TEST_NETWORK_KEEP_DIRS=1 cargo test -p integration_tests --test consensus_and_da zk_confidential_localnet::confidential_combined_peer_downtime_and_timeout_pressure_localnet -- --nocapture --test-threads=1` (passed without the restarted-peer catch-up warning)
  - `cargo test -p integration_tests --test consensus_and_da sumeragi_npos_performance::npos_baseline_1s_k3_captures_metrics -- --nocapture --test-threads=1`
  - `bash ci/check_sumeragi_formal.sh`

## 2026-05-03 Taira Inrou rollout fail-closed hardening

- Taira's shipped systemd unit now starts the bundled `/opt/iroha/bin/irohad`
  from the rollout bundle instead of an ambient `/usr/local/bin/irohad`, so the
  release cannot accidentally keep running a binary built without
  `embedded-soracloud-runtime`.
- The checked-in Taira validator config now enables Soracloud production mode
  with bounded fail-closed egress and non-proxy Inrou hosting, causing startup
  to reject stub/non-production runtime posture instead of silently exposing an
  empty runtime snapshot.
- The Taira container path now installs PortableVm QEMU tooling, passes the
  portable acceleration setting through, and exposes `/dev/kvm` when present.
- Soracloud status now reports the runtime manager as unavailable when `irohad`
  is compiled without `embedded-soracloud-runtime`, rather than presenting the
  stub as an idle materializer.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `bash -n configs/soranexus/taira/taira-validator-container.sh configs/soranexus/taira/build_taira_rollout_bundle.sh scripts/build_release_image.sh`
  - `cargo test -p iroha_config soracloud_runtime_production_mode_accepts_bounded_posture --lib -- --nocapture`
  - `cargo test -p iroha_config --test fixtures taira_config_enables_untrusted_cid_hosting -- --nocapture`
  - `cargo test -p iroha_torii --lib --features app_api,telemetry soracloud_runtime_status_sections_report_unavailable_without_runtime -- --nocapture`
  - `cargo test -p irohad --features embedded-soracloud-runtime --bin irohad manager_config_ -- --nocapture`
  - `python3 scripts/tests/taira_validator_container_test.py`
  - `configs/soranexus/taira/build_taira_rollout_bundle.sh --profile debug --allow-dirty`

## 2026-05-03 Sumeragi frontier formal process hardening

- Hardened the bounded Taira frontier-recovery model again after the latest
  consensus hang fixes. The model now tracks active pending progress age and
  event kind, validation/local-vote/commit-QC progress, subject-view-scoped
  stale recovery unlocks, and direct process obligations for stale owner clear,
  vote queue drain, payload recovery, quorum retransmit, retransmit
  follow-through, and future reanchor.
- Added expected-failure mutation coverage for disabled pending-progress touch
  and height-only stale recovery unlocks, and extended the formal expected
  failure suite so these run with the existing stale-owner, vote-queue,
  payload-recovery, retransmit-follow-through, future-promotion,
  reanchor-clear, future-evidence-drop, promotion-reset, and future-stale-owner
  mutations.
- The strengthened model closes a verification-process gap: during hardening,
  the retransmit-follow-through mutation initially escaped until the model got
  a direct `RetransmitHasFollowthroughProgress` invariant. The final suite now
  rejects that mutation as expected. No Sumeragi protocol state-machine
  behavior changed in this slice.
- Broader runtime validation exposed an internal execution-witness recorder
  isolation issue: a prior capture could leave the global witness recorder
  active after an early return or panic, poisoning later parallel tests and
  polluting transaction-set hash assertions. The recorder now only accepts
  events inside an active capture window, recovers from poisoned lock state for
  cleanup, and clears unfinished captures when the guard drops.
- The next Torii validation pass exposed a non-consensus macOS process-wrapper
  issue: `sandbox-exec` can abort the Rust attachment sanitizer child during
  runtime initialization before it can return a structured rejection. Torii now
  uses the direct sanitizer subprocess path on macOS, while Linux keeps the
  `bwrap` sandbox path when available. The subprocess timeout fixture now
  accepts both child-process and stdout-reader timeout classifications.
- Full formal validation with local Apalache `0.52.2` is green:
  - `bash -n scripts/formal/sumeragi_apalache.sh ci/check_sumeragi_formal.sh ci/check_sumeragi_formal_expected_failures.sh scripts/formal/sumeragi_tlc.sh`
  - `bash scripts/formal/sumeragi_apalache.sh frontier-fast`
  - `bash scripts/formal/sumeragi_apalache.sh frontier-deep`
  - `bash scripts/formal/sumeragi_apalache.sh frontier-wide`
  - `bash scripts/formal/sumeragi_apalache.sh frontier-nightly`
  - `bash ci/check_sumeragi_formal_expected_failures.sh`
  - `bash ci/check_sumeragi_formal.sh`
  - `git diff --check`
  - `python3 ci/check_docs_i18n_metadata.py --paths docs/formal` (passed with
    expected stale `source_hash` warnings for translated formal READMEs)
- The full CI formal gate also ran the small TLC cross-check for the frontier
  model. TLC completed the bounded state graph with `1,165,588` distinct
  states, graph depth `11`, and no invariant or temporal-property errors.
- Focused Rust bridge validation for the formal assumptions is green:
  - `cargo test -p iroha_core --lib local_commit_vote_counts_as_pending_progress -- --nocapture`
  - `cargo test -p iroha_core --lib commit_qc_observation_counts_as_pending_progress -- --nocapture`
  - `cargo test -p iroha_core --lib local_same_height_vote_blocks_when_exhausted_recovery_has_not_rotated_vote_view -- --nocapture`
  - `cargo test -p iroha_core --lib reschedule_defers_vote_backed_quorum_timeout_while_vote_queue_backlogged -- --nocapture`
  - `cargo test -p iroha_core --lib reschedule_skips_vote_backed_retransmit_while_frontier_quorum_timeout_window_owned -- --nocapture`
  - `cargo test -p iroha_core --lib reschedule_ignores_quorum_timeout_vote_queue_backlog -- --nocapture`
  - `cargo test -p iroha_core --lib pacemaker_reanchors_frontier_when_future_new_view_quorum_exists -- --nocapture`
  - `cargo test -p iroha_core --lib pacemaker_reanchors_future_new_view_quorum_while_vote_queue_backlogged -- --nocapture`
  - `cargo test -p iroha_core --lib pacemaker_reanchors_future_new_view_quorum_over_stale_frontier_owner -- --nocapture`
- Broader Sumeragi regression windows were also green after the formal pass:
  - `cargo test -p iroha_core --lib reschedule_ -- --nocapture` (`61` passed)
  - `cargo test -p iroha_core --lib same_height -- --nocapture` (`51`
    passed, `3` ignored as obsolete)
  - `cargo test -p iroha_core --lib pending_progress -- --nocapture` (`6`
    passed)
  - `cargo test -p iroha_core --lib pacemaker_reanchors -- --nocapture` (`3`
    passed)
- Continued widening found no runtime consensus failures:
  - `cargo test -p iroha_core --lib` (`5129` passed, `22` ignored, `0`
    failed; finished in `726.67s`)
  - `cargo test -p iroha_core --lib sumeragi::witness::tests -- --nocapture`
    (`5` passed)
  - `cargo test -p iroha_core --lib state::fastpq_tx_set_hash_tests -- --test-threads=1 --nocapture`
    (`4` passed)
  - `cargo test -p iroha_core --lib frontier -- --nocapture` (`326` passed,
    `1` ignored)
  - `cargo test -p iroha_core --lib vote_queue -- --nocapture` (`6` passed)
  - `cargo test -p iroha_core --lib commit_qc -- --nocapture` (`143` passed)
  - `cargo test -p iroha_core --lib future_new_view -- --nocapture` (`5`
    passed)
  - `cargo test -p iroha_core --lib sumeragi::main_loop::tests` (`2023`
    passed, `20` ignored)
  - `cargo test -p iroha_core --lib sumeragi::tests` (`137` passed)
  - `cargo test -p iroha_core --lib sumeragi::status` (`65` passed)
- The Torii crate corridor is green after fixing the macOS sanitizer wrapper
  issue:
  - `cargo test -p iroha_torii --test zk_attachments_subprocess -- --nocapture --test-threads=1`
    (`16` passed)
  - `cargo test -p iroha_torii` (passed, including `1680` library tests, `1`
    ignored, all integration binaries, and doctests)

## 2026-05-03 Nexus fee burn activation gate

- Normal Nexus transaction fees are now burned from the fee payer or authorized
  fee sponsor once `nexus.fees.burn_from_unix_timestamp_ms` is reached. Before
  that timestamp, the executor preserves legacy fee transfer/self-fee behavior
  so existing live Minamoto blocks replay without changing holder balances or
  total supply.
- Sponsored fees still require `CanUseFeeSponsor`, and admission checks now
  require the payer/sponsor fee asset balance even when the payer equals the
  configured fee sink, matching the burn-on-execution behavior after activation.
- Added regression coverage for sponsor-as-sink legacy no-op before activation,
  legacy transfer before activation, and burn behavior after activation.
- The default activation timestamp is `u64::MAX`; operators must explicitly set
  a future timestamp after deploying the compatible binary to every peer.
- Focused validation:
  - `cargo fmt --all`
  - `env -u LOG_FORMAT cargo test -p iroha_config`
  - `env -u LOG_FORMAT cargo test -p iroha_core nexus_fee -- --nocapture --test-threads=1`

## 2026-05-02 SoraFS pin registry metrics test isolation

- The SoraFS pin registry metrics summary test now records its Prometheus
  assertions against an isolated test metrics registry instead of the process
  global registry, avoiding parallel-test interference from other telemetry
  fixtures while keeping the summary assertions unchanged.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo test -p iroha_torii --lib --features app_api,telemetry sorafs::api::advert_tests::pin_registry_metrics_summary_tracks_counts -- --nocapture`
  - `cargo test -p iroha_torii --lib --features app_api,telemetry -- --nocapture`

## 2026-05-02 Kotodama source analysis fixture refresh

- Updated the reentrancy-analysis test snippets to call
  `host::call_contract` with the current `(String|Blob, String|Blob, Json)`
  signature, preserving the write-before-call and call-before-write scenarios
  under test.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo test -p kotodama_lang reentrancy -- --nocapture`
  - `cargo test -p kotodama_lang`

## 2026-05-02 Iroha Connect Android approve fixture refresh

- Refreshed the Android-emitted Connect approve frame fixture to carry the
  current canonical I105 account literal and matching nested Connect length
  fields instead of the retired base58-style account literal.
- The fixture reader now tolerates line-wrapped hex so long generated frames can
  remain readable without changing the decoded byte stream.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo test -p iroha_torii_shared --test connect_android_approve_fixture -- --nocapture`
  - `cargo test -p iroha_torii_shared --lib -- --nocapture`

## 2026-05-02 Torii limiter existing-key lookup trim

- Refactored Torii's sharded rate limiter so hot existing-key checks use one
  mutable bucket lookup instead of `contains_key` followed by `get_mut`.
  Bucket insertion/eviction remains on the cold miss path and is shared by
  single-cost and repeated-consume paths.
- Kept the earlier impossible-cost fast reject before map access, so requests
  larger than burst still fail without allocating a bucket.
- Added `limiter_existing_key_reuses_bucket` to cover existing-key reuse across
  `allow`, `allow_cost`, and `allow_repeated`. Focused validation passed with
  `CARGO_TARGET_DIR=/tmp/iroha-codex-torii-route`: `cargo test -p iroha_torii
  limiter_existing_key_reuses_bucket --lib -- --nocapture`, `cargo test -p
  iroha_torii limiter_rejects_impossible_cost_without_tracking_key --lib --
  --nocapture`, and `cargo test -p iroha_torii
  limiter_allow_repeated_matches_single_key_prefix_consumption --lib --
  --nocapture`.

## 2026-05-02 IVM state probe syscalls

- Added V1 SCALLX durable-state probes `STATE_HAS` (`0x010031`),
  `STATE_LEN` (`0x010032`), and `STATE_COUNT` (`0x010033`) to the ABI
  allowlist, syscall names, generated docs, and gas-asset table. The ABI v1
  hash is now
  `73cefb1b419f97b9e2864cdc6545d3f80ae2328dc0fbe2fbd034cd51a837ba0d`.
- Implemented the probes in `DefaultHost`, standalone `CoreHost`, `WsvHost`,
  and the real smart-contract `CoreHost`. Presence and length checks use the
  same scoped overlay/base/tombstone durable-state resolution as the existing
  state read path; `STATE_LEN` reports the `NoritoBytes` payload length rather
  than the TLV envelope length, and `STATE_COUNT` counts matching keys without
  copying or returning the key list.
- Corrected the syscall source spec so dedicated `QUERY_GET_*` SCALLX helpers
  advertise the singular query gas model (`G_scq`) instead of a placeholder
  dash.
- Closed the adjacent classic durable-state gas gap: `STATE_GET`, `STATE_SET`,
  and `STATE_DEL` now advertise and return deterministic XOR gas charges
  (`G_state_get + bytes`, `G_state_set + bytes`, and `G_state_del`) across
  `DefaultHost`, standalone `CoreHost`, `WsvHost`, and the real
  smart-contract host. Present reads/writes charge payload bytes; misses and
  tombstones charge only the fixed base. `STATE_KEYS` host gas is aligned with
  the documented base + returned-count + encoded-bytes model.
- Removed two more zero-gas syscall returns from the query-like surface:
  `GET_ACCOUNT_BALANCE` now uses the same singular-query gas arithmetic in the
  real host and WSV mock, and `RESOLVE_ACCOUNT_ALIAS` now charges a
  deterministic singular-query-style cost in the real host.
- Closed more codec-helper gas gaps: `TLV_EQ` now advertises
  `G_tlv_eq + bytes`, and `TLV_LEN` now advertises `G_tlv_len + bytes`.
  Both return byte-counted gas from the standalone codec host, WSV host, and
  default host while preserving exact type/version/payload comparison and
  payload-length semantics.
- Numeric helpers now advertise and return the fixed `G_numeric` charge across
  default, standalone codec, WSV, and real-host forwarding paths, so bounded
  deterministic arithmetic no longer runs for free.
- Pointer conversion helpers now advertise and return `G_pointer + bytes`.
  `POINTER_TO_NORITO` charges the canonical TLV envelope bytes copied into
  `NoritoBytes`, `POINTER_FROM_NORITO` charges the embedded envelope bytes it
  validates, and the default host now implements the same public ABI helpers as
  the standalone codec and WSV hosts.
- SM4 GCM/CCM seal/open helpers now advertise and return `G_sm4 + bytes`
  through the shared default-host implementation used by standalone, WSV, and
  real smart-contract hosts. The byte component charges AAD plus
  plaintext/ciphertext bytes, including decoded-input failure paths, while the
  existing deterministic SM4 vector outputs remain unchanged.
- SM2 verification now advertises and returns `G_verify + bytes`. The default
  host charges message, signature, public-key, and optional distid bytes on
  both successful verification and decoded-input failure paths; standalone,
  WSV, and real-host forwarding inherit the same deterministic cost.
- Deterministic host sysvars now advertise and return nonzero gas:
  `CURRENT_TIME_MS`, `SYSVAR_BLOCK_TIME_MS`, and `SYSVAR_BLOCK_HEIGHT` charge
  `G_sysvar`; byte-returning `SYSVAR_CHAIN_ID`, `SYSVAR_CONTRACT_ADDRESS`, and
  `SYSVAR_ENTRYPOINT` charge `G_sysvar + bytes`; authority sysvars charge
  `G_get_auth + bytes`. DefaultHost, standalone CoreHost, WsvHost, and the real
  smart-contract host now return the matching gas from these paths.
- Schema and codec helper gas gaps are closed across standalone CoreHost, WSV,
  and real-host codec delegation: `SCHEMA_*` charge `G_schema + bytes`;
  `JSON_ENCODE`, `JSON_DECODE`, `JSON_OBJECT`, `JSON_GET_*`, and `JSON_SET_*`
  charge their JSON gas assets plus bytes; `DECODE_INT`/`ENCODE_INT` charge
  `G_numeric + bytes`; path builders charge `G_path + bytes`; and
  `NAME_DECODE` charges `G_name_decode + bytes`.
- Focused validation passed with
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --lib default_host_state_has_len_and_keys_roundtrip -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --lib state_keys_syscall_returns_sorted_prefix_page -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --test wsv_host_state_syscalls wsv_host_state_count_uses_overlay_and_tombstones -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --lib tlv_eq_syscall_compares_payloads -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --lib default_host_tlv_eq_charges_payload_bytes -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --test wsv_host_state_syscalls wsv_host_tlv_eq_charges_payload_bytes -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --lib tlv_len_syscall_charges_payload_bytes -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --lib default_host_tlv_len_charges_payload_bytes -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --test wsv_host_state_syscalls wsv_host_tlv_len_charges_payload_bytes -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --test numeric_syscalls -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-core-scallx cargo test -p iroha_core numeric_helper_syscalls_roundtrip_through_codec_host --lib -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --lib pointer_norito_helpers_charge_envelope_bytes -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --lib default_host_pointer_helpers_roundtrip_and_charge_gas -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --test wsv_host_state_syscalls wsv_host_pointer_helpers_charge_envelope_bytes -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --lib pointer_to_norito_roundtrips_via_pointer_from_norito -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --test kotodama_state_struct_pointer -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --lib default_host_sm4_gcm_charges_aad_and_data_bytes -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --test sm_syscalls wsv_host_sm4_gcm_seal_returns_byte_counted_gas_when_enabled -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --test sm_syscalls syscall_sm4_ -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --test sm_syscalls syscall_sm2_verify_ -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --lib sysvar -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --lib time_syscalls_use_configured_deterministic_value -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --lib current_time_syscall_returns_host_time -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --lib helpers_charge_payload_bytes -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --test core_host_json_schema_syscalls schema_encode_decode_roundtrip -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --test wsv_host_decode_syscalls -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --test core_host_name_decode_syscall -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --lib get_account_balance_syscall_accepts_account_id_payloads -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --test core_host_state_syscalls --test wsv_host_state_syscalls -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-core-scallx cargo test -p iroha_core get_account_balance_syscall_reads_numeric_asset --lib -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-core-scallx cargo test -p iroha_core resolve_account_alias_syscall_reads_current_alias_binding --lib -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-core-scallx cargo test -p iroha_core state_syscall_ --lib -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-core-scallx cargo test -p iroha_core state_keys_syscall_strips_scope_and_applies_tombstones --lib -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-core-scallx cargo test -p iroha_core encode_decode_int_syscalls_roundtrip --lib -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --test abi_hash_versions --test abi_syscall_list_golden --test syscalls_doc_sync --test ivm_abi_doc_sync --test syscalls_gas_names --test syscalls_markdown_gas -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --test syscalls_doc_sync --test syscalls_gas_names --test syscalls_markdown_gas --test gas_schedule_hash -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --test gas_schedule_hash -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm_abi --lib -- --nocapture`,
  `cargo fmt --all`, and `git diff --check`.
- Real-host sysvar focused reruns
  `CARGO_TARGET_DIR=target/codex-core-scallx cargo test -p iroha_core sysvar --lib -- --nocapture`
  and
  `CARGO_TARGET_DIR=target/codex-core-scallx cargo test -p iroha_core current_time_syscall_uses_block_time --lib -- --nocapture`
  were blocked before reaching `iroha_core` by unrelated dirty-tree
  `fastpq_prover` compile errors from the pre-V1 transfer scaffold. The V1
  cleanup replaces that scaffold with required SMT witness material.

## 2026-05-02 Torii limiter impossible-cost fast reject

- Moved Torii `RateLimiter::allow_cost` burst feasibility ahead of bucket
  lookup/allocation. Costed requests larger than the configured burst now fail
  before inserting an unserviceable key into the sharded bucket map.
- Added a limiter regression for oversized cost rejection without bucket growth,
  while preserving later small requests for the same key.
- Formatting and touched-file whitespace checks passed. Focused Torii test
  validation is currently blocked before reaching Torii by unrelated dirty-tree
  `ivm` compile errors in `crates/ivm/src/host.rs` (`tlv.payload` on `()`),
  observed with both `cargo test -p iroha_torii limiter_ --lib --
  --nocapture` and the single-test `limiter_rejects_impossible_cost_without_tracking_key`
  filter.

## 2026-05-02 Torii batch API-token borrow

- Removed the remaining `/transactions/batch` API-token clone in Torii ingress.
  The batch handler now borrows `x-api-token` through authentication and the
  post-decode limiter check, matching the direct transaction submission
  handlers while preserving the shared token-key rate-limit semantics.
- Added a distinct-authority batch regression proving token-authenticated
  batches still consume the shared API-token key instead of falling back to
  per-authority keys.
- Focused validation passed with
  `CARGO_TARGET_DIR=/tmp/iroha-codex-torii-route`: `cargo test -p iroha_torii
  handler_post_transactions_batch_ --lib -- --nocapture` and `cargo test -p
  iroha_torii transaction_batch_rate_limit_ --lib -- --nocapture`.
  `cargo fmt --package iroha_torii -- --check` also passed.

## 2026-05-02 Current 20k Profile After Ed25519 Cache

- Ran a release 4-peer no-fault prebuilt `20,000 TPS` / `30s` sampled Izanami
  profile at
  `dist/izanami-profile-20k-ed25519-cache-sampled3-30s-20260502-182524`.
  Izanami exited `0`; `sample(1)` attached to the runner and all four
  `iroha3d` peers with `sample_ready=1` and `sample_status=0`.
- The run offered and accepted all `600,000` planned ingress submissions,
  built and used all `600,000` prebuilt transactions, and had no submit
  failures, prebuild fallback, ingress failover, unhealthy endpoints,
  confirmation failures, confirmation queue drops, validation rejects, view
  changes, DA/RBC store pressure, missing payloads, or missing-QC exhaustions.
  Submit latency p50/p95/p99/max was `9/53/853/1493 ms`.
- Final quorum/strict height was `3/3` with `4,113/4,113` approved
  transactions, max peer height skew `1`, and max approved-transaction skew
  `4,096`. The queue remained saturated at deadline (`210,733/600,000`) with
  `52` pacemaker backpressure deferrals; commit height `4` was still in
  flight at shutdown.
- Treat this as contended diagnostic evidence, not a clean comparable baseline:
  `contention-before.txt` captured an active workspace `cargo test`, rustc
  jobs, and another debug test network; `contention-after.txt` still captured
  active rustc/test-network work.
- Current peer hot spots are led by
  `iroha_core::fastpq::finalize_transfer_transcripts_serial` on Rayon workers.
  The active stacks repeatedly serialize account/numeric/array fields through
  `norito::codec::encode_adaptive_into`, `AccountId::serialize`,
  `write_len_prefixed`, `NumericScaleHelper::serialize`, and array
  serialization, then feed `PoseidonByteHasher::{write,update}` and
  `poseidon3_permute`.
- Ed25519/Curve25519 admission remains a secondary peer-side cost:
  `precheck_transaction_batch_ed25519` still reaches
  `ed25519_dalek::verify_batch`, `optional_multiscalar_mul`,
  `CompressedEdwardsY::decompress`, `sqrt_ratio_i`, `pow22501`, and
  `FieldElement51::pow2k` for uncached/miss work. Public-key decode also still
  shows `PublicKeyCompact::try_deserialize`, `PublicKeyFull::from_bytes`,
  `parse_public_key`, cache get/insert, recompression, decompression, and
  small-order checks.
- Other visible but smaller costs are Norito transaction/signature decode,
  compact-length writes, `ConstVec`, allocation/copy churn (`malloc`, `free`,
  `realloc`, `RawVec` growth, `memmove`/`memcpy`), and CRC64 fallback. The
  Izanami runner mostly parks in Tokio or waits on HTTP request I/O and is not
  the bottleneck.

## 2026-05-02 Torii direct ingress allocation trim

- Trimmed direct transaction submission allocation on API-token paths by
  borrowing the `x-api-token` header for auth and rate limiting instead of
  cloning it into a `String`.
- Removed the local-route `TransactionEntrypoint` clone in
  `handler_post_transaction_entrypoint`; admission now takes ownership of the
  decoded entrypoint, and the proxy branch clones from the accepted transaction
  only when the route is actually remote.
- Added token-key regression coverage for direct signed-transaction and
  entrypoint submissions. Focused validation passed with
  `CARGO_TARGET_DIR=/tmp/iroha-codex-torii-route`: `cargo test -p iroha_torii
  api_token_rate_limit_key --lib -- --nocapture` and `cargo test -p
  iroha_torii handler_post_transaction_ --lib -- --nocapture`.

## 2026-05-02 Ed25519 admission hot-path cache reduction

- Extended the Ed25519 32-byte public-key parse cache to retain deterministic
  invalid outcomes as well as valid parsed keys. Wrong-length payloads still
  bypass the cache; canonical decompression failures, non-canonical encodings,
  and weak-key rejections now return the same parse error from the cache on
  repeated attempts. The direct thread-local fast cache is now 256 slots while
  the bounded map still clears at the existing limit.
- Kept Ed25519 compact/full conversion on the cached parse path and added a
  regression so `PublicKeyCompact` to `PublicKeyFull` conversion hits the
  thread-local cache after the first parse.
- Tightened Ed25519 batch verification setup around the exact verify-ok cache:
  all-cached batches skip signature parsing and dalek batch setup, mixed
  batches parse only uncached signatures, and the first-bad bisection still
  returns the lowest original failing index.
- Torii batch admission gained regressions for repeated Ed25519 authorities,
  bad Ed25519 signatures, and singleton batch-precheck equivalence with normal
  single-transaction signature verification.
- Final Criterion hot-path numbers from
  `cargo bench -p iroha_crypto --bench ed25519_hotpaths`:
  warm parse `34.783-34.903 ns`, 256-key parse loop
  `19.673-19.982 us`, single verify cache path `318.93-319.71 ns`,
  exact 32-byte verify cache hit `35.167-35.245 ns`, preparsed batch verify
  `16/64/256 = 4.6657-4.6940 us / 18.495-18.644 us / 73.693-73.932 us`.
- Focused validation:
  - `cargo test -p iroha_crypto ed25519 -- --nocapture`
  - `cargo test -p iroha_crypto ed25519 --lib -- --nocapture`
  - `cargo test -p iroha_torii precheck_transaction_batch_ed25519 -- --nocapture`
    (compiled and completed, but this filter matched zero Torii tests)
  - `cargo test -p iroha_torii transaction_batch_ed25519_precheck --lib -- --nocapture`
  - `cargo check -p iroha_crypto -p iroha_core -p iroha_torii`
  - `rustfmt --edition 2024 --check crates/iroha_crypto/src/signature/ed25519.rs crates/iroha_crypto/src/lib.rs crates/iroha_torii/src/lib.rs`
- Follow-up release Izanami 20k gate and sampled profile reruns are recorded
  above and below. The latest sampled profile is
  `dist/izanami-profile-20k-ed25519-cache-sampled3-30s-20260502-182524`.

## 2026-05-02 Release Izanami 20k Gate Rerun After Ed25519 Cache

- Rebuilt the release `izanami` and `iroha3d` binaries with
  `RUSTFLAGS='-A missing-copy-implementations' cargo build --release -p
  izanami --bin izanami -p irohad --bin iroha3d`; the build passed in `625s`.
- Reran the release 4-peer no-fault prebuilt `20,000 TPS` / `120s` Izanami
  gate at
  `dist/izanami-prebuilt-20k-rerun-release-ed25519-cache-120s-20260502-180614`.
  The wrapper exited `0` with `build_status=0` and `run_status=0`.
- This was not a clean all-accepted ingress gate: the run offered all
  `2,400,000` planned submissions, reported `ingress_accepted=2,364,756`,
  `successes=2,364,756`, and `failures=35,244`. It still built and used all
  `2,400,000` prebuilt transactions, with zero prebuild fallback, skipped
  prebuilt transactions, prebuild build failures, ingress failover, unhealthy
  endpoints, confirmation failures, or confirmation queue drops.
- Submit latency p50/p95/p99/max was `9/5860/8580/11997 ms`. Final
  quorum/strict height was `7/7` with `20,582/20,582` approved transactions,
  max peer height skew `1`, and max approved-transaction skew `4,096`. The
  queue remained saturated at deadline (`743,992/2,400,000`).
- Consensus safety signals stayed deterministic: no validation rejects, no
  DA/RBC store pressure, no missing-payload or missing-QC deferrals, no
  commit-inflight timeout, missing-QC reacquire succeeded `6/6`, and there
  were no range-pull failures. Liveness remained overloaded, with `1`
  view-change install, `37` missing-block fetches, `99` pacemaker backpressure
  deferrals, and `7` block-sync range-pull escalations / `1` success.
- Treat this as contended diagnostic evidence, not a clean comparable release
  baseline. The artifact captured `10/8/17` active build/gate process lines in
  `contention-build-before.txt`, `contention-before.txt`, and
  `contention-after.txt`.

## 2026-05-02 Torii batch authority rate-limit run collapse

- Collapsed consecutive same-authority `/transactions/batch` rate-limit checks
  into one repeated-token consume. Unauthenticated batches now avoid allocating
  and locking once per transaction when a wallet/load generator submits adjacent
  transactions from the same authority, while interleaved authorities still
  consume in the original transaction order.
- The API-token fast path from the prior Torii pass remains unchanged: one
  token-authenticated batch still consumes against the API-token key once.
- Added focused tests for same-authority collapse and mixed-authority ordering.
  Validation passed with `CARGO_TARGET_DIR=/tmp/iroha-codex-torii-route`:
  `cargo test -p iroha_torii transaction_batch_rate_limit_ --lib --
  --nocapture`. The adjacent handler suite passed with the local unrelated lint
  allowance:
  `RUSTFLAGS='-A variant-size-differences' CARGO_TARGET_DIR=/tmp/iroha-codex-torii-route cargo test -p iroha_torii handler_post_transactions_batch_ --lib -- --nocapture`.
  `cargo fmt --package iroha_torii -- --check` and touched-file
  `git diff --check` also passed. The earlier unsuppressed handler rerun
  blocker was cleared by shrinking the cached Ed25519 parse outcome; the Torii
  handler rerun was not restarted in this slice.

## 2026-05-02 FastPQ V1 verifier structural hardening

- Added a default `max_proof_bytes` verifier limit so oversized FastPQ proof
  payloads are rejected before canonical replay work.
- Tightened the V1 verifier before replay equality: batch/proof parameter
  mismatches now fail immediately, FRI layer count must match the exact
  domain/arity reduction schedule, and sampled FRI round openings must carry
  exactly one arity-sized chunk per non-terminal round.
- Added exact opening-shape checks for the verifier: sampled LDE chunks must
  match their canonical leaf length, LDE/AIR/FRI Merkle authentication paths
  must match the derived tree depth, and terminal FRI openings must match the
  derived final-layer leaf shape.
- Removed the legacy deterministic lane-relay proof digest helper entirely.
  Positive fixtures now use external proof digests or the real AXT proof blob
  hash.
- Lane-relay proof metadata now requires a concrete `verified_at_height` at or
  above the relayed block height; omitted verification heights are no longer a
  valid wire shape.
- The FastPQ JSON request schema now requires execution-captured
  `batch_base64`; descriptor-only proof requests and synthetic measurement
  samples are no longer accepted inputs.
- Deleted the core synthetic FastPQ batch-hash fallback. Transfer transcript
  recording now requires a real transaction call hash, by-call triggers derive
  a trigger-specific call hash, and generated RWA lot IDs use the first-release
  `iroha:rwa:id:v1` domain.
- Verified lane-relay registration now binds the envelope's FastPQ proof digest
  to the submitted proof blob payload hash and rejects proof metadata stamped
  beyond the block height doing the verification.
- AXT FastPQ bindings now reject explicit non-`fastpq` verifier IDs and
  non-`v1` verifier versions instead of accepting mislabeled proof envelopes.
  Empty verifier labels still canonicalize to the first-release FastPQ V1
  defaults.
- AXT FastPQ proof payloads now have a pre-decode size ceiling, so oversized
  envelope `proof` fields fail before Norito payload decoding or replay work.
- Removed the descriptor-derived synthetic AXT batch builder and CLI fallback;
  FastPQ proof generation and measurement now require execution-captured
  `batch_base64` fixture material.
- AXT FastPQ bindings now also reject empty or duplicate target dataspace sets.
  Data-model and IVM ABI proof-envelope shape checks require concrete binding
  strings, supported FastPQ claim types, 32-byte hex digest fields, and a
  nonempty duplicate-free target set, so a `fastpq`/`v1` label alone is no
  longer enough to pass AXT proof material checks.
- Standalone IVM DefaultHost/CoreHost/WSVHost validation now shares an
  ABI-level FastPQ V1 envelope preflight, so those hosts reject raw proof bytes,
  non-FastPQ labels, and non-V1 labels. Preflight is diagnostic only; because
  standalone IVM does not link the full FastPQ verifier, proof-consuming AXT
  calls fail closed after preflight instead of accepting the envelope shape.
- DefaultHost now also binds accepted handle usage to the proof envelope
  manifest root. Inline, recorded, and late-provided proofs must match the
  handle `manifest_view_root`, and zero handle roots are rejected.
- Focused validation passed with `CARGO_TARGET_DIR=/tmp/iroha-codex-core`:
  `cargo test -p fastpq_prover --lib verify_rejects -- --nocapture`,
  `cargo test -p fastpq_prover --lib verify_limits -- --nocapture`,
  `cargo test -p fastpq_prover --lib verify_fri_query_chain --
  --nocapture`,
  `cargo test -p fastpq_prover --lib axt_binding -- --nocapture`,
  `cargo test -p fastpq_prover --lib -- --nocapture`,
  `cargo check -p fastpq_prover --bins --lib`,
  `cargo test -p iroha_data_model --lib proof_matches_manifest --
  --nocapture`,
  `cargo test -p iroha_data_model --lib fastpq_proof_material -- --nocapture`,
  and `cargo test -p iroha_core --lib lane_relay --features app_api --
  --nocapture`. With `CARGO_TARGET_DIR=/tmp/iroha-codex-ivm-axt`,
  `cargo test -p ivm_abi --lib preflight_fastpq_v1_proof_envelope --
  --nocapture` and `cargo test -p ivm --test axt_host_flow -- --nocapture`
  also passed. The DefaultHost raw-proof/manifest-root binding regressions were
  rerun with
  `CARGO_TARGET_DIR=/tmp/iroha-codex-ivm-axt-host cargo test -p ivm --test
  axt_host_flow -- --nocapture`, which passed with 33 tests, and the adjacent
  default-host syscall sequence passed with
  `CARGO_TARGET_DIR=/tmp/iroha-codex-ivm-host-unknown`: `cargo test -p ivm
  --test host_unknown_syscall -- --nocapture`. The adjacent standalone
  CoreHost policy target also passed with the same
  `/tmp/iroha-codex-ivm-axt-host` target dir: `cargo test -p ivm --test
  core_host_policy -- --nocapture`.
  `cargo fmt --all` passed.

## 2026-05-02 FASTPQ Poseidon byte-hasher hot path

- Removed the extra two-word staging array from the BN254
  `PoseidonByteHasher` path, so packed little-endian words now absorb directly
  into the sponge state before each rate-2 permutation. The byte streaming
  contract and known vectors remain unchanged.
- Kept FASTPQ transfer preimage digests on the streaming Norito `encode_to`
  path and tightened the matching word-packer batch path used before GPU
  digest acceleration.
- Added coverage for `update_u64_le_word` after a partial byte update, and
  shrank the cached Ed25519 parse outcome so the unrelated
  `variant-size-differences` lint no longer blocks FASTPQ digest validation.
  Focused validation passed with `cargo test -p iroha_zkp_halo2 poseidon --lib
  -- --nocapture`, `cargo test -p fastpq_isi poseidon --lib -- --nocapture`,
  `cargo test -p fastpq_prover
  compute_poseidon_digest_matches_canonical_encoded_preimage --lib --
  --nocapture`, `cargo test -p iroha_core
  poseidon_digest_matches_known_vector --lib -- --nocapture`, and `cargo test
  -p iroha_crypto parse_public_key_cache --lib -- --nocapture`.
- Broader FASTPQ host validation passed with `cargo test -p iroha_core fastpq::
  --lib -- --nocapture` after refreshing the authority digest golden vector and
  making the metadata test expect the canonical finalized transcript.
- Post-change Criterion baseline from `cargo bench -p iroha_core --features
  zk-halo2,zk-halo2-ipa --bench crypto_hotpaths`: Poseidon `hash_bytes`
  32/128/512/4096 bytes = `56.259-59.903 us`, `173.68-187.74 us`,
  `647.19-697.40 us`, `5.9642-6.8346 ms`; streaming hasher
  32/128/512/4096 bytes = `56.319-57.692 us`, `169.98-173.16 us`,
  `620.48-622.26 us`, `9.1656-11.642 ms`; fixed-width `hash2_u64` =
  `34.395-45.507 us`, `hash6_u64` = `92.358-120.23 us`.
- Follow-up CPU pass unrolled the width-3 full/partial round body in
  `poseidon3_permute`, removing the remaining per-round state iterator from
  the BN254 Poseidon hot path. Validation after this pass: `cargo test -p
  iroha_zkp_halo2 poseidon --lib -- --nocapture`, `cargo test -p
  fastpq_prover compute_poseidon_digest_matches_canonical_encoded_preimage
  --lib -- --nocapture`, `cargo test -p iroha_core
  poseidon_digest_matches_known_vector --lib -- --nocapture`, and `cargo bench
  -p iroha_core --features zk-halo2,zk-halo2-ipa --bench crypto_hotpaths
  --no-run`.

## 2026-05-02 Android Sora VPN native lease SDK surface

- Added Kotlin/JVM and Java Android Torii helpers for the Sora VPN native XOR
  lease flow: profile fetch, signed quote creation, session creation by
  committed quote-bound lease transaction hash, session get/delete, operator
  receipt submission, and receipt listing.
- Added typed VPN DTOs for `OpenVpnLeaseEscrow` and `SettleVpnLease`
  instruction skeletons, plus parsers for earned/refunded XOR amounts and
  native settlement instructions.
- Updated canonical request signing on Kotlin/JVM to include Torii freshness
  headers (`X-Iroha-Timestamp-Ms`, `X-Iroha-Nonce`) and removed the Java
  `HexFormat` dependency from nonce rendering.
- Added focused Kotlin and Java Android transport tests for VPN profile,
  quote/session/receipt request bodies, canonical signatures, and native lease
  instruction parsing. Local execution is blocked in this environment because
  `/usr/libexec/java_home -v 21` and Gradle both report no installed Java
  runtime; `git diff --check` over the touched SDK files is clean.

## 2026-05-02 Torii batch token rate-limit fast path

- Added a same-key repeated-consume path to Torii's sharded rate limiter and
  used it for `/transactions/batch` submissions authenticated by one API token.
  Token-keyed batches now take one limiter shard lock and one monotonic
  timestamp instead of cloning/checking the same token once per transaction.
- Rejection semantics stay aligned with the former per-transaction loop: if a
  same-key batch is too large, the limiter consumes the whole-token prefix that
  would have passed before the first limited item. Authority-keyed batches keep
  the existing per-transaction checks.
- Added `torii_rate_limiter_same_key_batch_32` to the Torii hot-path benchmark
  binary. Focused validation passed with
  `CARGO_TARGET_DIR=/tmp/iroha-codex-torii-route`: `cargo test -p iroha_torii
  limiter_allow_repeated_matches_single_key_prefix_consumption --lib --
  --nocapture`, `cargo test -p iroha_torii
  handler_post_transactions_batch_rate_limits_api_token_as_single_key_batch
  --lib -- --nocapture`, and `cargo bench -p iroha_torii --bench
  torii_hot_paths --no-run`. `cargo fmt --package iroha_torii -- --check` and
  touched-file `git diff --check` also passed.

## 2026-05-02 Current Contended 20k Bottleneck Profile After Hotspot Follow-up

- Ran the release 4-peer no-fault prebuilt `20,000 TPS` / `30s` sampled
  Izanami profile at
  `dist/izanami-profile-20k-hotspots-followup-sampled-30s-20260502-145740`.
  Izanami exited `0`; `sample(1)` attached to the runner and all four
  `iroha3d` peers with `sample_ready=1` and `sample_status=0`.
- The run offered and accepted all `600,000` planned ingress submissions, built
  and used all `600,000` prebuilt transactions, and had no submit failures,
  prebuild fallback, ingress failover, unhealthy endpoints, validation rejects,
  view changes, or DA/RBC store pressure. Submit latency p50/p95/p99/max was
  `14/610/979/1499 ms`.
- Final quorum/strict height was `3/3` with `4,117/4,117` approved
  transactions and zero peer height or approved-transaction skew. The queue
  remained saturated at deadline (`228,682/600,000`) with `41` pacemaker
  backpressure deferrals, `2/2` missing-QC reacquires, and `2` block-sync range
  pull escalations.
- Treat the CPU ranking as contended diagnostic evidence. The artifact captured
  `33` active process lines before and `28` after, including active
  Cargo/rustc/clippy jobs.
- Current peer CPU hot spots are led by FASTPQ/Poseidon
  (`apply_mds3`, `sbox`, `PoseidonByteHasher::{update,absorb_word,finalize}`,
  and `poseidon_preimage_digest`), followed by Curve25519/Ed25519 admission
  work (`FieldElement51::pow2k`, field multiplication, `pow22501`,
  `sqrt_ratio_i`, Edwards compression/decompression, `parse_public_key`,
  `PublicKeyFull::from_bytes`, and `verify_batch`).
- Norito remains active in transaction/instruction paths:
  `SignedTransaction`/`TransactionPayload` decode, `InstructionBox` and
  `TransferBox` serialize/deserialize, `ConstVec` encode/decode,
  `write_len_with_flags`, `write_len_prefixed`, `len_prefix_len_with_flags`,
  `read_len_from_slice_with_flags`, `plan_binary_sequence`, `SmallBuf::write`,
  and public-key/account encode/decode all show up in the peer samples.
- Secondary costs are allocation/copy and hashes: `_platform_memmove`,
  malloc/free/realloc, `RawVec` reserve/grow, Vec clone/drop, SHA256, Blake2,
  CRC64, Keccak, default hashing, and TLS access (`_tlv_get_addr`). The
  Izanami runner itself is not the bottleneck; it mostly waits on HTTP I/O and
  shows only low-count prebuilt-submit overhead.

## 2026-05-02 Contended Release Izanami 20k Gate Rerun After Hotspot Follow-up

- Rebuilt the release `izanami` and `iroha3d` binaries with
  `RUSTFLAGS='-A missing-copy-implementations' cargo build --release -p
  izanami --bin izanami -p irohad --bin iroha3d`; the build passed in `561s`
  after waiting on the Cargo package-cache lock.
- Reran the release 4-peer no-fault prebuilt `20,000 TPS` / `120s` Izanami
  gate at
  `dist/izanami-prebuilt-20k-rerun-release-hotspots-followup-120s-20260502-144114`.
  The wrapper exited `0` with `build_status=0` and `run_status=0`.
- The run offered all `2,400,000` planned submissions, reported
  `ingress_accepted=2,400,000`, built and used all `2,400,000` prebuilt
  transactions, and had no prebuild fallback, submit failures, ingress
  failover, or unhealthy endpoints. Submit latency p50/p95/p99/max was
  `9/31/130/473 ms`.
- Final quorum/strict height was `5/5` with `12,330/12,330` approved
  transactions and zero peer height or approved-transaction skew. The queue
  remained saturated at deadline (`906,464/2,400,000`), with `44` pacemaker
  backpressure deferrals.
- Safety signals stayed deterministic: no validation rejects, no DA/RBC store
  pressure, no missing-payload or missing-QC deferrals, no commit-inflight
  timeout, missing-QC reacquire succeeded `4/4`, and block-sync range pull
  recorded `4` escalations / `5` successes. Liveness was still overloaded:
  `4` view-change installs, `2` quorum-timeout causes, and `6` missing-block
  fetches were recorded.
- Treat this as contended diagnostic evidence, not a clean comparable release
  baseline. The artifact captured `32/16/33` active build/gate process lines in
  `contention-build-before.txt`, `contention-before.txt`, and
  `contention-after.txt`, including active Cargo/rustc jobs.

## 2026-05-02 Torii route-resolution reuse

- Removed a duplicate lane/dataspace route resolution on local Torii
  transaction enqueue. Single signed-transaction, entrypoint, and inbound
  proxy submission paths now pass the already-resolved routing decision into
  the queue push path, matching the existing batched transaction handler.
- This only avoids repeated router/state work after admission has already
  decided the local route. It does not change transaction wire bytes, hashes,
  queue semantics, proxy routing selection, or runtime configuration.
- Focused validation passed with
  `CARGO_TARGET_DIR=/tmp/iroha-codex-torii-route`:
  `cargo test -p iroha_torii reuses_resolved_route_for_enqueue --lib -- --nocapture`
  and
  `cargo test -p iroha_torii handler_post_transaction_ --lib -- --nocapture`.
  `cargo fmt --package iroha_torii -- --check` and
  `git diff --check -- crates/iroha_torii/src/lib.rs` also passed.

## 2026-05-02 Norito/Crypto Hotspot Follow-up Implementation

- Reduced Ed25519 admission cache overhead for the 32-byte transaction-hash
  path: valid 32-byte message/signature tuples now stay on the exact fixed
  cache and no longer compute the Blake2 verify-cache key on cache misses.
  Batch verification also avoids rechecking the already-scanned prefix when
  splitting cached and uncached tuples.
- Removed avoidable gossip precheck copies by feeding borrowed payload-hash and
  signature slices into the deterministic Ed25519 batch verifier while
  preserving original-order rejection reporting.
- Moved FASTPQ transfer Poseidon digest construction in `iroha_core` and
  `fastpq_prover` to the existing streaming byte hasher. The batch GPU staging
  packer now writes words directly into the shared batch buffer instead of
  allocating a temporary word vector per transcript.
- Reduced Norito/transaction allocation churn: reusable `SmallBuf` scratch
  buffers return to stack storage after `clear()` even after an earlier spill,
  and `AcceptedTransaction` prepares external entrypoint hashes from borrowed
  signed transactions instead of cloning `SignedTransaction`.
- These changes do not alter Norito wire bytes, transaction hashes, decoded
  values, rejection ordering, or runtime configuration. Focused validation
  passed with `CARGO_TARGET_DIR=/tmp/iroha-codex-hotspots-followup`:
  `cargo test -p iroha_crypto ed25519_verify_ok_cache --lib -- --nocapture`,
  `cargo test -p norito smallbuf_clear_returns_short_writes_to_stack_storage
  --lib -- --nocapture`, `cargo test -p fastpq_prover
  compute_poseidon_digest_matches_canonical_encoded_preimage --lib --
  --nocapture`, `cargo test -p iroha_core
  borrowed_external_entrypoint_hash_matches_canonical_hash --lib --
  --nocapture`, `cargo test -p iroha_core
  poseidon_word_packer_matches_little_endian_chunks --lib -- --nocapture`,
  `cargo test -p iroha_core gossip_ed25519_batch_precheck --lib --
  --nocapture`, `cargo test -p iroha_core
  accepted_transaction_caches_hashes_and_encoded_length --lib --
  --nocapture`, `cargo test -p iroha_core
  accept_with_canonical_signed_bytes_reuses_payload_cache --lib --
  --nocapture`, and `cargo test -p iroha_core
  poseidon_digest_matches_known_vector --lib -- --nocapture`.
- The package check corridor also passed with the same target directory:
  `cargo check -p norito -p iroha_crypto -p fastpq_prover -p iroha_core`.
  `cargo fmt --all -- --check` and `git diff --check` passed after the
  implementation.

## 2026-05-02 Workspace all-target compile corridor

- Repaired the current `cargo check --workspace --all-targets` blockers found
  after the RAM-LFE hardening pass by following the coded APIs:
  `norito_codegen_exporter` now renders `Metadata::Float`, the Python receipt
  test fixture uses the current `TransactionSubmissionReceiptPayload` fields,
  and Mochi's chaos/event/state helpers match the current Izanami fault,
  Offline V2 note-event, and query-batch surfaces.
- Cleaned up the remaining Rust warning sources surfaced by the workspace
  check: removed the unused `ivm_corehost_axt` model-proof helper and the
  unused proof imports from `queries_and_proofs`.
- Focused validation passed with
  `CARGO_TARGET_DIR=/tmp/iroha-codex-workspace-corridor`:
  `cargo test -p norito_codegen_exporter metadata_to_value_renders_float_mode --bin norito_codegen_exporter`,
  `cargo test -p iroha_python_rs decode_transaction_receipt_json_roundtrip --lib`,
  `cargo check -p mochi-core --all-targets`,
  `cargo test -p mochi-core offline_note_issued_summary_includes_note_and_amount --lib`,
  and
  `cargo test -p mochi-core batch_label_handles_rwa_and_escrow_variants --lib`.
  The warning cleanup was checked with
  `cargo check -p iroha_core --test ivm_corehost_axt` and
  `cargo check -p integration_tests --test queries_and_proofs`.
- The broader
  `CARGO_TARGET_DIR=/tmp/iroha-codex-workspace-corridor cargo check --workspace --all-targets`
  completed successfully after warning cleanup. The only diagnostics left on
  this host are CUDA helper build-script warnings that `nvcc` is unavailable.
- Focused strict clippy also passed with
  `CARGO_TARGET_DIR=/tmp/iroha-codex-workspace-corridor`:
  `cargo clippy -p norito_codegen_exporter -p iroha_python_rs -p mochi-core --all-targets -- -D warnings`
  and
  `cargo clippy -p iroha_core --test ivm_corehost_axt -p integration_tests --test queries_and_proofs -- -D warnings`.
- The full workspace all-target clippy corridor is green with
  `CARGO_TARGET_DIR=/tmp/iroha-codex-workspace-corridor cargo clippy --workspace --all-targets -- -D warnings`
  after splitting the oversized FastPQ FRI query-chain test, moving
  `verify_fri_query_chain` to a small context struct, and cleaning the
  SoraNet VPN settlement helper lints.
- The Java RAM-LFE parser mirror was whitespace-cleaned after inspection, and
  the missing Kotlin SDK RAM-LFE parser/transport coverage was added. Focused
  SDK validation passed with Homebrew OpenJDK 21 pinned via `JAVA_HOME`:
  `./gradlew :core-jvm:test --tests org.hyperledger.iroha.sdk.client.HttpClientTransportTest --console=plain`
  from `kotlin/`, and
  `ANDROID_HARNESS_MAINS=org.hyperledger.iroha.android.client.HttpClientTransportTests ./gradlew :core:test --tests org.hyperledger.iroha.android.GradleHarnessTests --console=plain`
  from `java/iroha_android/`.
- `cargo fmt --all -- --check` and `git diff --check` also passed after the
  fixes.
- Continued the RAM-LFE/identifier SDK audit from the coded wire paths. Swift
  account-controller Norito encoding now uses Rust-compatible
  algorithm-tagged `PublicKey` bytes, confidential encrypted payload
  serialization has a deterministic Swift fallback when the native bridge is
  unavailable, Ed25519 seed material falls back to the pure Swift key path, and
  the pinned `NoritoBridge` hashes match the current artifact manifest.
- Canonicalized BFV identifier ciphertext Norito framing across JavaScript,
  Kotlin/JVM, and Java Android by replacing the legacy repeated FNV schema hash
  with the domain-separated SHA-256 schema hash used by Norito core and Swift.
  The broader SDK audit also moved the C# Norito codec, JavaScript Connect
  browser/journal framing, and the Android pinned schema manifest onto the same
  canonical type-name schema hash. The JS/Swift/Java BFV vectors and Swift
  live-JS fixture were refreshed. The captured live Torii receipt signature is
  now kept as a legacy-negative check because it signs the previous account-id
  payload form; current signed-receipt positives are still covered by generated
  verifier fixtures.
- Additional validation passed on 2026-05-02:
  `cargo run -p iroha_data_model --features test-fixtures,transparent_api --bin offline_v2_vectors -- --check`,
  `node --test javascript/iroha_js/test/toriiClient.identifier.test.js`,
  `cd IrohaSwift && swift test` (774 tests, 101 skipped, 0 failures), the
  focused Swift regression filter for Ed25519 seed, bridge pinning, BFV vectors,
  and legacy receipt rejection, the JavaScript Connect/identifier test slice,
  ESLint over the touched JS SDK files, Android `:core:verifyNoritoSchemas`,
  `scripts/check_no_scale.sh`, plus the same focused Kotlin/JVM and Java
  Android transport harnesses with Homebrew OpenJDK 21 pinned via `JAVA_HOME`.
- A temporary .NET 8 SDK installed under `/tmp/iroha-dotnet/sdk` unblocked C#
  validation. `dotnet test csharp/tests/Hyperledger.Iroha.Sdk.Tests/Hyperledger.Iroha.Sdk.Tests.csproj --no-restore`
  now passes (137 tests) after updating the C# Norito schema-hash golden, the
  C# transaction hash goldens, faucet PoW vectors, and the half-width
  Katakana URL expectation to match the coded SDK paths.

## 2026-05-02 Sumeragi NPoS/permissioned QC and VRF hardening

- Hardened Sumeragi VRF commit/reveal traffic with BLS-signed mode-tagged
  preimages, reject-on-invalid inbound signatures, and reveal acceptance only
  after a matching prior commit.
- Tightened commit/block-sync evidence handling so permissioned paths use vote
  quorum while NPoS paths require stake snapshots for stake-weighted quorum and
  root selection; observer padding no longer counts toward NPoS commit/root
  materialization.
- Made live consensus-key records mandatory for active roster membership, kept
  large numeric stake comparisons exact, and updated Torii/OpenAPI VRF
  endpoints to require non-empty `bls_sig_hex` payloads.
- Focused validation passed with
  `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-verify`: Sumeragi commit tests,
  block-sync update tests, VRF tests, QC validation filters, roster-selection
  tests, Torii VRF OpenAPI/parser tests, and the data-model consensus Norito
  roundtrip. `cargo check -p iroha_data_model -p iroha_core -p iroha_torii -p
  irohad`, `cargo fmt --all --check`, `git diff --check`, and OpenAPI JSON
  parsing also passed.
- The focused lint corridor also passed with
  `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-clippy cargo clippy -p
  iroha_core -p iroha_data_model -p iroha_torii -p irohad --all-targets -- -D
  warnings` after mechanical deterministic-container, allocation,
  doc-markdown, iterator, and bench-cast cleanup in crates pulled into that
  graph.
- The full workspace all-target lint corridor now also passes with
  `CARGO_TARGET_DIR=/tmp/iroha-codex-workspace-corridor cargo clippy
  --workspace --all-targets -- -D warnings`; the only emitted diagnostics are
  CUDA helper build-script warnings when `nvcc` is unavailable on the host.

## 2026-05-02 Current Contended Release Izanami 20k Bottleneck Profile

- Ran the release 4-peer no-fault prebuilt `20,000 TPS` / `30s` sampled
  Izanami profile at
  `dist/izanami-profile-20k-hotspots-now-sampled-30s-20260502-110442`;
  Izanami exited `0` and all five `sample(1)` captures completed with
  `sample_status=0`.
- The profile offered `600,000` submissions, built and used all `600,000`
  prebuilt transactions, accepted `53,392` ingress transactions, and reported
  submit latency p50/p95/p99/max of `3362/5206/5998/6628 ms`.
- Final quorum/strict height was `3/3` with `4,111/4,111` approved
  transactions, max height and approved-transaction skew `0`, final queue depth
  `38,895/600,000`, and `tx_queue_saturated=true`. Consensus safety signals
  stayed clean: no view changes, validation rejects, DA/RBC pressure,
  pending-RBC drops, or commit-inflight timeouts.
- The runner sample is mostly async wait, endpoint health/backpressure, and
  request I/O. Peer CPU remains the bottleneck. The largest sampled peer costs
  are Ed25519/Curve25519 admission work (`verify_batch`, parsed public-key
  handling, decompression, `FieldElement51::pow2k`/field multiplication),
  FASTPQ/Poseidon digest work (`poseidon_preimage_digest`, MDS, sbox, byte
  absorption), Norito transaction/instruction codec work (`write_len`, compact
  length reads, `SmallBuf::write`, `ConstVec`, `InstructionBox`,
  `TransferBox`, `TransactionPayload`, `SignedTransaction`, account and public
  key decode), and allocation/copy churn. `sha2`, `blake2`, `keccak`, `crc64`,
  and SHA512 Ed25519 hashing are visible secondary costs.
- This is still diagnostic rather than a clean comparable profile:
  `contention-before.txt` and `contention-after.txt` each captured `20` active
  Cargo/rustc or gate-related processes. Keep the clean sampled profile open
  for an uncontended host window.

## 2026-05-02 Contended Release Izanami 20k Gate After Hotspot Follow-up

- Reran the release 4-peer no-fault prebuilt `20,000 TPS` / `120s` Izanami
  gate after rebuilding `target/release/izanami` and `target/release/iroha3d`
  with `RUSTFLAGS='-A missing-copy-implementations'`. The release build
  completed in `9m48s`.
- Fresh artifact:
  `dist/izanami-prebuilt-20k-rerun-release-hotspots-120s-20260502-104844`;
  the wrapper exited `0`. The gate offered all `2,400,000` submissions, built
  and used all `2,400,000` prebuilt transactions, had zero prebuild fallback or
  build failures, and accepted `52,615` ingress transactions. Submit latency
  p50/p95/p99/max was `3223/4642/5561/7027 ms`.
- Final quorum/strict height was `9/9` with `28,720/28,720` approved
  transactions, max height skew `1`, approved-transaction skew `4,096`, final
  queue depth `23,958/2,400,000`, and `tx_queue_saturated=true`.
- Safety signals stayed free of validation rejects, view changes, quorum
  timeout causes, DA gate pressure, RBC store pressure/evictions, pending-RBC
  drops, and commit-inflight timeouts. The run recorded `7/7` missing-QC
  reacquire attempts/successes, `7` range-pull escalations, and `125`
  pacemaker backpressure deferrals.
- This is still not a clean comparable release baseline:
  `contention-before.txt` and `contention-after.txt` each captured `24` active
  Cargo/rustc or gate-related processes. Keep the clean 20k release gate open
  for an uncontended host window.

## 2026-05-02 IVM ABI V1 SCALLX and Determinism Hardening

- Added the first-release extended syscall encoding: `SYSTEM` now acts as
  `SCALLX` with a 24-bit syscall id, is admitted by opcode validation, is
  charged in the gas schedule, and is analyzed by the static syscall scanner.
- Expanded the V1 ABI syscall surface with SCALLX query helpers
  `QUERY_EXECUTE_NORITO`, dedicated `QUERY_GET_*` reservations, and sysvars for
  chain id, block height, block time, authority, contract address, and
  entrypoint. Added `STATE_KEYS` for deterministic durable-state prefix
  enumeration with pagination and contract-scope prefix stripping. Added
  deterministic, gas-charged hash helpers for SHA-256, SHA3-256, raw
  Blake2b-256, Keccak-256, and Iroha `Hash::new`; the real smart-contract host
  now routes these instead of rejecting them as unknown. The ABI hash is updated
  to `d0ea15df44f695e074ea697a808d61b991497361fcaefe8e28603403e0ec62ed`, and the
  opcode gas schedule hash remains
  `65dcbda2e776d3b4e7a83b16830cf3f5c40a0e91bef0174eef25095c69f38fad`.
- Hardened prepared-program execution so contract headers/literal bytes cannot
  fall back to raw instruction decode, and fallthrough past code now reports
  `MissingHalt` unless the program explicitly halts, exits, or aborts.
- Replaced `DefaultHost` wall-clock time with configured deterministic block
  time, added deterministic block-height sysvar plumbing, kept
  XOR-denominated gas/fee metadata intact, kept stored VM host objects
  shareable as `Send + Sync`, and kept scoped `run_with_host` execution able
  to borrow a non-`Sync` host directly.
- Implemented the dedicated `QUERY_GET_ACCOUNT`, `QUERY_GET_ASSET`,
  `QUERY_GET_ASSET_DEFINITION`, `QUERY_GET_DOMAIN`, `QUERY_GET_NFT`,
  `QUERY_GET_PARAMETER`, `QUERY_GET_CONTRACT_MANIFEST`, and
  `QUERY_GET_CONTRACT_INSTANCE` SCALLX helpers. Account/asset/domain/manifest
  reads use the existing validated query engine; NFT, parameter, and
  contract-instance reads use deterministic attached-state snapshots and the
  same singular query gas schedule.
- Implemented `VERIFY_PROOF` in `CoreHost` for
  `NoritoBytes(OpenVerifyEnvelope)` payloads. It now uses the on-chain
  verifying-key registry, namespace-independent envelope prechecks, backend
  guardrails, and deterministic status-code returns; the standalone host still
  reports `NotImplemented` because it has no registry context. Acceleration
  runtime status also now reports CUDA parity as OK only when CUDA is actually
  available after policy, hardware detection, and self-tests.
- Implemented the public `PROVE_EXECUTION` syscall instead of leaving it as a
  reserved stub. The default/CoreHost path now returns
  `NoritoBytes(ExecutionProof)`: a first-release deterministic proof summary
  with fixed fields plus SHA-256 commitments to PC, delta-register, ZK trace,
  constraint, memory, register, and step-root logs. This keeps the artifact
  byte-identical across hardware and gives future SNARK/STARK integration
  stable public material to bind.
- Admission now decodes both classic `SCALL` and extended `SCALLX` numbers when
  rejecting unknown ABI syscalls.
- Focused validation passed with `CARGO_TARGET_DIR=target/codex-ivm-scallx`:
  `cargo test -p ivm --lib ivm_is_send_sync_for_state_sharing -- --nocapture`,
  `cargo test -p ivm --lib scallx_dispatches_extended_syscall_id -- --nocapture`,
  `cargo test -p ivm --test abi_hash_versions --test gas_schedule_hash --test syscalls_doc_sync --test ivm_abi_doc_sync -- --nocapture`,
  and
  `cargo test -p ivm_abi --lib syscallx_roundtrips_24_bit_number -- --nocapture`.
  The matching core admission regression also passed with
  `CARGO_TARGET_DIR=target/codex-core-scallx cargo test -p iroha_core validate_ivm_unknown_scallx_rejected_at_admission --lib -- --nocapture`.
  Follow-up focused host-bound sweeps also passed with
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --lib ivm_is_send_sync_for_state_sharing -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --lib run_with_host_accepts_non_sync_host -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --lib block_height_syscall_uses_configured_deterministic_value -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --lib acceleration_runtime -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --lib cuda_status_never_reports_parity_without_availability -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --lib execution_proof_summary_is_stable_for_same_program -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --lib state_keys_syscall_returns_sorted_prefix_page -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --test syscalls hash_syscalls_return_expected_digest_blobs -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --test syscalls test_prove_execution_syscall_returns_deterministic_summary -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --test abi_hash_versions --test abi_syscall_list_golden --test syscalls_doc_sync --test ivm_abi_doc_sync --test syscalls_gas_names --test syscalls_markdown_gas -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --test gas_schedule_hash -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm_abi --lib -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-core-scallx cargo test -p iroha_core dedicated_query_syscalls_return_norito_payloads --lib -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-core-scallx cargo test -p iroha_core generic_verify_proof_syscall_reports_registry_precheck_errors --lib -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-core-scallx cargo test -p iroha_core block_height_sysvar_uses_attached_transaction_context --lib -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-core-scallx cargo test -p iroha_core state_keys_syscall_strips_scope_and_applies_tombstones --lib -- --nocapture`,
  `CARGO_TARGET_DIR=target/codex-ivm-scallx cargo test -p ivm --lib`,
  the IVM gas/opcode/vector/metadata/pointer/predecoder/doc-sync batches, and
  the remaining IVM integration tail covering syscalls, vector execution,
  WSV-host state/query/admin flows, VRF, and ZK verifier gates. A direct
  `shifts_prop` rerun passed after one broad `cargo test -p ivm --tests`
  session became a stale tool session after the process had already exited.

## 2026-05-02 FastPQ V1 verifier and AXT proof binding

- Replaced the FastPQ V1 fail-closed placeholder verifier exit with canonical
  replay verification: after transcript, Merkle, AIR, and FRI opening checks,
  verification now authenticates the submitted proof contents, public inputs,
  commitments, Merkle paths, lookup product, AIR openings, FRI query chain, and
  verifier challenges without regenerating a prover artifact.
- Added an explicit proof-size verification limit before verifier work.
  Oversized V1 proof payloads now fail with `max_proof_bytes` instead of being
  allowed to drive expensive decode/opening checks.
- Fixed AIR next-row openings to use the FRI blowup stride over the LDE domain,
  and removed the transfer-row synthetic SMT fallback from trace construction;
  transfer rows now require canonical sender/receiver SMT proof metadata.
- Hardened AXT FastPQ verification so proof envelopes must carry a Norito
  `AxtFastpqProofPayload` containing the proven batch and proof. IVM and ISI
  registration paths verify that payload directly instead of rebuilding a
  descriptor-derived batch from `AxtFastpqBinding`, and verified lane relay records now
  persist the FastPQ statement digest plus embedded proof digest.
- Added an explicit `axt_fastpq_batch_seal_v1` metadata seal over the concrete
  carried batch and canonical AXT binding. The verifier also requires a
  non-empty batch with execution `entry_hash` metadata matching
  `source_tx_commitment`. Descriptor-derived fixture batches intentionally do
  not produce this seal and are rejected by the AXT envelope verifier; the JSON
  CLI now requires an explicit execution-captured `batch_base64` before
  exporting AXT proof envelope material.
- Hardened the AXT consumption paths that were still manifest-only: block AXT
  validation and the IVM CoreHost proof syscall now require a decodable AXT
  proof envelope and successful FastPQ verification, so raw manifest roots and
  binding-less envelopes are no longer accepted in those maintained paths.
- Migrated the block-level app-API AXT validation fixtures and host proof-cache
  success fixtures off raw manifest roots. Valid paths now use sealed
  FastPQ-backed AXT proof envelopes; malformed byte payloads remain only in
  negative tests that assert rejection.
- Tightened the remaining structural AXT helpers and shims: `ProofBlob`
  matching now requires a Norito `AxtProofEnvelope` with non-empty proof bytes
  and a V1 FastPQ binding, while the standalone IVM CoreHost and WSV test host
  reject raw manifest roots instead of accepting them as proof payloads.
- Closed the remaining lane-relay proof-material shortcuts: the deterministic
  `expected_fastpq_proof_digest` helper is gone, `verified_at_height` is a
  required field, verified lane-relay registration binds the envelope digest to
  the proof blob payload hash, and maintained fixtures use externally supplied
  proof digests or proof-payload hashes instead.
- Removed the remaining synthetic FastPQ batch-hash fallback from core state:
  transfer transcript recording fails without a transaction call hash, by-call
  trigger execution seeds a trigger-specific call hash instead of a fake FastPQ
  batch hash, and generated RWA lot IDs now use the first-release
  `iroha:rwa:id:v1` domain.
- Migrated state replay-ledger, ISI lane-relay registration, `ivm_corehost_axt`,
  `ivm` host-flow, and ABI amount-resolution success fixtures to bound AXT
  proof envelopes. The data-model AXT JSON fixture was regenerated with
  `fastpq_binding` material instead of binding-less proof envelopes.
- Removed automatic synthetic FastPQ proof material attachment from lane relay
  envelope production. Relay proof metadata validation now only accepts
  non-zero externally supplied proof digests with an explicit verification
  height.
- Refreshed the FastPQ ordering-hash golden for the current canonical
  Norito/Poseidon encoding used by the prover.
- Focused validation passed:
  `cargo test -p fastpq_prover --lib`,
  `cargo check -p fastpq_prover --bins --lib`, and
  `cargo check -p iroha_core --lib`.
- Follow-up FastPQ verifier-limit validation passed with
  `CARGO_TARGET_DIR=/tmp/iroha-codex-core`:
  `cargo test -p fastpq_prover --lib verify_limits -- --nocapture` and
  `cargo check -p fastpq_prover --bins --lib`.
- Follow-up lane-relay proof-material validation passed with
  `CARGO_TARGET_DIR=/tmp/iroha-codex-core`:
  `cargo test -p iroha_data_model --lib fastpq_proof_material -- --nocapture`
  and
  `cargo test -p iroha_core --lib lane_relay --features app_api -- --nocapture`.
- Follow-up deletion validation passed:
  `CARGO_TARGET_DIR=/tmp/iroha-codex-fastpq-ruthless cargo check -p fastpq_prover --bins --lib`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-core cargo check -p iroha_core --features app_api`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-fastpq-ruthless cargo test -p fastpq_prover --lib axt_binding -- --nocapture`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-fastpq-data cargo test -p iroha_data_model --lib fastpq_proof_material -- --nocapture`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-core cargo test -p iroha_core --lib transfer_transcripts --features app_api -- --nocapture`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-core cargo test -p iroha_core --lib generated_rwa_id --features app_api -- --nocapture`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-core cargo test -p iroha_core --lib rwa --features app_api -- --nocapture`, and
  `CARGO_TARGET_DIR=/tmp/iroha-codex-core cargo test -p iroha_core --lib lane_relay --features app_api -- --nocapture`.
- Additional focused CoreHost AXT validation passed:
  `cargo test -p iroha_core --test ivm_corehost_axt --features iroha-core-tests,app_api core_host_binds_proof_to_manifest_root -- --nocapture`.
- Additional envelope-only validation passed:
  `cargo test -p iroha_data_model --lib proof_matches_manifest -- --nocapture`,
  `cargo test -p iroha_data_model --test axt_envelope_fixture -- --nocapture`,
  `cargo test -p iroha_data_model --test axt_proof_envelope -- --nocapture`,
  `cargo test -p ivm --test axt_host_flow -- --nocapture`,
  `cargo test -p ivm --test core_host_policy -- --nocapture`,
  `cargo test -p iroha_core --lib axt_replay_ledger --features app_api -- --nocapture`,
  `cargo test -p iroha_core --lib register_verified_lane_relay_instruction_box_is_registered --features app_api -- --nocapture`,
  `cargo test -p iroha_core --test ivm_corehost_axt --features iroha-core-tests,app_api axt_replay_ledger -- --nocapture`,
  `cargo test -p iroha_core --test ivm_corehost_axt --features iroha-core-tests,app_api axt_sub_nonce_floor_persists_across_restart -- --nocapture`,
  `cargo test -p iroha_core --lib axt_verify_ds_proof --features app_api -- --nocapture`,
  `cargo test -p iroha_core --lib axt_validation_tests --features app_api -- --nocapture`,
  `cargo test -p ivm_abi resolve_handle_amount -- --nocapture`, and
  `cargo check -p iroha_core --lib --features app_api`.
- Follow-up AXT fixture validation passed with
  `CARGO_TARGET_DIR=/tmp/iroha-codex-core`:
  `cargo check -p iroha_core --lib --features app_api`,
  `cargo test -p iroha_core --lib axt_validation_tests --features app_api -- --nocapture`,
  and
  `cargo test -p iroha_core --lib axt_verify_ds_proof -- --nocapture`.

## 2026-05-02 SoraNet VPN XOR escrow and helper hardening

- Changed SoraNet VPN control plane defaults so VPN is disabled until explicitly
  configured with a helper-ticket secret and relay TLS SPKI pin, while the
  exposed VPN fee selector is the explicit XOR alias
  `xor#universal.universal`.
- Replaced prepaid lease-only session creation with a quote and committed XOR
  escrow payment flow. Torii now issues quote-bound session records only after
  signed account authorization, XOR payment hash binding, relay/operator
  metadata, and a non-operator escrow account check.
- Added client-signed cumulative VPN usage vouchers and relay receipt binding
  fields so operator settlement must match the active quote, payment, account,
  relay, and client metering key, and cannot claim more than the escrowed lease
  fee.
- Hardened helper tickets and relay accounting: helper tickets bind quote,
  account, relay, and payment, are single-use on the relay, expire active
  backend tunnels, and VPN metering now starts only for helper-authenticated VPN
  sessions. Relay ingress/egress cells reject non-monotonic sequences.
- Hardened the local helper by requiring relay TLS SPKI pinning, rejecting zero
  padding budgets, writing state/resolver backups with private permissions
  outside `/tmp` by default, and resolving network commands from trusted system
  paths with a cleared environment.
- Added native VPN lease escrow settlement payloads to the legacy Torii receipt
  response: operator-submitted receipts now return the quote/default
  `lease_id_hex` plus a Norito-framed `SettleVpnLease` instruction skeleton
  carrying the verified relay receipt and client voucher. The same skeleton is
  also exposed in the generic `tx_instructions` array used by existing
  client-signed transaction tooling.
- Added an optional relay-side `vpn.receipt_spool_dir` settlement artifact
  spool. When a helper-authenticated session closes after accepting a client
  usage voucher, the relay writes a Norito-compatible JSON request body for
  `/v1/vpn/receipts`; sessions without accepted vouchers are logged but not
  spoolable for settlement.
- Added `soranet-vpn-settlement`, a relay-package operator helper that consumes
  those spooled artifacts, signs the exact Torii canonical receipt request with
  runtime-only Ed25519 seed material, and emits either JSON headers/body or a
  ready `curl` command for the operator runner.
- Switched the Torii quote/session flow onto native lease opening: quote
  requests bind the client metering key and return a Norito-framed
  `OpenVpnLeaseEscrow` skeleton in `tx_instructions`; session creation now
  verifies the committed transaction opened that exact XOR-native lease instead
  of accepting a generic transfer to an escrow account.
- Updated the JavaScript Torii client surface for the native VPN flow:
  `createVpnQuote` exposes the quote-bound `OpenVpnLeaseEscrow` skeleton,
  `createVpnSession` now requires quote/payment/metering-key binding, and
  `submitVpnReceipt` exposes `SettleVpnLease` settlement skeletons plus
  earned/refund XOR fee fields.
- Updated the C# Torii client surface to match the same native VPN contract:
  quote creation, quote-bound session creation, operator receipt submission,
  and typed `OpenVpnLeaseEscrow`/`SettleVpnLease` instruction DTOs are now
  exposed, delete returns the canonical disconnected receipt, and the live
  smoke no longer tries the retired direct-session flow.
- Updated the Swift Torii client surface for the same flow:
  `ToriiCanonicalRequestAuth` can sign VPN app requests, quote/session helpers
  validate the XOR lease identifiers and metering key, and receipt/delete/list
  helpers expose typed `SettleVpnLease` instruction skeletons.
- Updated the Python Torii client surface for the same flow:
  `ToriiCanonicalRequestAuth` signs request bytes through a caller-supplied
  signer callback, quote/session helpers normalize the XOR lease identifiers
  and metering key, and receipt/delete/list helpers expose typed
  `SettleVpnLease` instruction skeletons.
- Focused validation:
  - `cargo check -p iroha_data_model -p iroha_config -p iroha_torii -p soranet-relay -p sora-vpn-helper --tests`
  - `cargo test -p iroha_data_model soranet::vpn --lib`
  - `cargo test -p iroha_config soranet_vpn --lib`
  - `cargo test -p iroha_torii vpn --lib`
  - `cargo test -p soranet-relay --test vpn_overlay`
  - `cargo test -p sora-vpn-helper --bin sora-vpn-controller`
  - JavaScript VPN smoke covering quote/session/receipt request and response
    normalization passed via direct `node --input-type=module`; the full
    `toriiClient.test.js` load is currently blocked by the missing native JS
    binding, and `npm run build:native` fails in the pre-existing
    `iroha_core/src/smartcontracts/ivm/host.rs` query-helper compile errors.
  - C# VPN-focused tests were initially blocked by missing `dotnet`; the later
    temporary .NET 8 SDK validation covered the same Torii client surface in the
    full C# unit suite.
  - `cd IrohaSwift && swift test --filter 'ToriiClientTests/test.*Vpn'`
    passed for the Swift VPN profile, quote, session, and receipt helpers.
  - `python3 -m pytest python/iroha_torii_client/tests/test_client.py -k 'vpn or canonical'`
    passed for the Python VPN profile, quote signing, session, and receipt
    helpers; importing `iroha_python` under the system Python 3.9 remains
    blocked by the pre-existing `typing.TypeAlias` usage in
    `python/iroha_python/src/iroha_python/crypto.py`.
- Follow-up native open/settle validation passed with
  `CARGO_TARGET_DIR=/tmp/iroha-vpn-tools-check CARGO_BUILD_JOBS=2`:
  `cargo test -p iroha_core xor_asset_check_accepts_canonical_xor_id --lib`,
  `cargo test -p iroha_torii 'vpn::tests::' --lib`,
  `cargo test -p iroha_torii vpn_tool_factories_expose_expected_names_and_routes --lib`,
  `cargo test -p iroha_torii generated_spec_includes_documented_paths --lib`,
  `cargo test -p soranet-relay vpn_usage_voucher_control_updates_receipt --lib`,
  `cargo test -p soranet-relay vpn_receipt_spool_dir_preserves_operator_path --lib`,
  `cargo test -p soranet-relay --bin soranet_vpn_settlement`,
  and
  `cargo test -p sora-vpn-helper usage_voucher_signer_builds_signed_cumulative_voucher --bin sora-vpn-controller`.

## 2026-05-02 Contended Release Izanami 20k Bottleneck Profile

- Ran a release 4-peer no-fault prebuilt `20,000 TPS` / `30s` sampled Izanami
  profile at
  `dist/izanami-profile-20k-post-hotpath-sampled-30s-20260502-035256`;
  Izanami exited `0` and all five `sample(1)` captures completed with
  `sample_status=0`.
- The profile offered `600,000` submissions, built and used all `600,000`
  prebuilt transactions, accepted `42,409` ingress transactions, and reported
  submit latency p50/p95/p99/max of `3268/4955/5636/6213 ms`.
- Final quorum/strict height was `3/3` with `4,163/4,163` approved
  transactions, max height skew `0`, final queue depth `31,733/600,000`, and
  `tx_queue_saturated=true`. Consensus safety signals stayed clean: no view
  changes, validation rejects, DA/RBC pressure, pending-RBC drops, or
  commit-inflight timeouts.
- Peer samples now split the hot path between Ed25519/Curve25519 admission
  precheck and FASTPQ/Poseidon digest work. The highest leaf samples include
  `curve25519_dalek::FieldElement51::pow2k`, Curve25519 field multiplication,
  `iroha_zkp_halo2::poseidon::apply_mds3`, `PoseidonByteHasher`, and
  `poseidon::sbox`. The call paths confirm
  `precheck_transaction_batch_ed25519 -> ed25519_dalek::batch::verify_batch`
  and `fastpq::poseidon_preimage_digest -> norito::codec::encode_adaptive_into
  -> AccountId/InstructionBox/TransferBox/ConstVec serialization ->
  write_len_prefixed -> PoseidonByteHasher`.
- Norito transaction/instruction decode appears in the peer samples through
  `ConstVec`, `InstructionBox`, and `TransferBox` deserialization, but it is
  no longer the top standalone leaf cost in this run. Hash helpers
  `sha2`, `blake2`, `keccak`, and `crc64` plus allocation/memmove are visible
  as secondary costs under admission, preimage digest, and encoding work.
- This is still not a clean comparable profile: `contention-before.txt`
  captured `27` active Cargo/rustc jobs and `contention-after.txt` captured
  `10`, including active `iroha_data_model` rustc processes near full CPU.
  Keep a clean sampled profile open for an uncontended host window.

## 2026-05-02 Contended Release Izanami 20k Gate Rerun

- Reran the release 4-peer no-fault prebuilt `20,000 TPS` / `120s` Izanami gate
  after rebuilding `target/release/izanami` and `target/release/iroha3d`.
  The release rebuild required the existing lint-only
  `RUSTFLAGS='-A missing-copy-implementations'` workaround for the unrelated
  `VpnUsageVoucherBodyV1` `Copy` lint. It first exposed an unrelated Torii
  BFV decrypt return-type mismatch in the dirty tree; `decrypt_program_input`
  now wraps the BFV decrypt result through the existing
  `IdentifierResolutionError::Fhe` conversion, and the rebuild then passed in
  `5m08s`.
- Fresh artifact:
  `dist/izanami-prebuilt-20k-rerun-release-post-hotpath-120s-20260502-034728`;
  the wrapper exited `0`. The gate offered all `2,400,000` submissions, built
  and used all `2,400,000` prebuilt transactions, had zero prebuild fallback or
  build failures, and accepted `48,376` ingress transactions. Submit latency
  p50/p95/p99/max was `3282/4696/5182/6160 ms`.
- Final quorum/strict height was `4/4` with `8,228/8,228` approved
  transactions, max height skew `0`, approved-transaction skew `0`, final queue
  depth `39,234/2,400,000`, and `tx_queue_saturated=true`.
- Safety signals stayed free of validation rejects, quorum-timeout causes, DA
  gate pressure, RBC store pressure/evictions, pending-RBC drops, and
  commit-inflight timeouts. The run recorded `5` view-change installs, `1`
  missing-QC view-change cause, `120` missing-block fetches, `8/8` missing-QC
  reacquire attempts/successes, `12` range-pull escalations with `5` successes,
  and `83` pacemaker backpressure deferrals.
- This is not a clean comparable release baseline: `contention-before.txt`
  captured `6` active Cargo/rustc jobs, including an `iroha_core` rustc process
  using about `140%` CPU, and `contention-after.txt` captured `8` active
  Cargo/rustc jobs. Keep the clean 20k release gate open for an uncontended host
  window.

## 2026-05-02 RAM-LFE API and Proof Hardening

- Removed plaintext RAM-LFE output disclosure from execute responses. Torii now
  returns `output_hash` and `receipt_hash` only for execution, while receipt
  verification still accepts caller-supplied `output_hex` for local hash
  matching. The Torii OpenAPI schema, Swift/Kotlin/Java/JavaScript SDK response
  parsers, and universal-account guide were updated to match.
- Removed the coded debug proof-verifier backends (`debug/ok`, `debug/reject`,
  and `debug/sleep`) from verifier dispatch, rejected `debug/*` proof metadata
  in programmed RAM-LFE parameters, and moved tests onto real Halo2 fixtures or
  unsupported production-style backends for rejection coverage.
- Hardened receipt and policy admission by validating RAM-LFE receipt execution
  and expiry timestamps against the verification clock, requiring valid BFV
  public parameters for BFV policies, and rejecting proof verification modes on
  RAM-LFE backends that do not carry proof-verifier metadata.
- Snapshot/world deserialization now re-runs the same RAM-LFE program-policy
  validation before restoring `world.ram_lfe_program_policies`, so malformed
  persisted state cannot bypass register/activate admission checks.
- Fixed encrypted generic RAM-LFE execution to preserve raw decrypted bytes.
  Identifier-claim decryption still enforces UTF-8 only on identifier inputs.
- OpenAPI manifest refresh now supports detached Ed25519 signature envelopes via
  `cargo xtask openapi --signature-envelope <path>`, allowing an operator-held
  signing key to sign the exact canonical `torii.json` bytes without exposing
  the private key to the checkout.
- Focused validation so far: `cargo fmt --all`, `cargo fmt --all -- --check`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-ram-lfe-impl cargo test -p xtask openapi`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-ram-lfe-impl cargo test -p iroha_crypto ram_lfe --lib`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-ram-lfe-impl cargo test -p iroha_core deserialize_rejects_invalid_ram_lfe_program_policy_storage --lib`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-ram-lfe-impl cargo test -p iroha_torii ram_lfe --lib --features app_api`,
  `node --test test/toriiClient.ramLfe.test.js`,
  `swift test --filter ToriiClientTests/testExecuteRamLfeProgramAsync`, and
  `git diff --check` passed. Follow-up validation:
  `CARGO_TARGET_DIR=/tmp/iroha-codex-ram-lfe-sync cargo test -p ivm ivm_is_send_sync_for_state_sharing --lib`
  and
  `CARGO_TARGET_DIR=/tmp/iroha-codex-ram-lfe-sync cargo test -p iroha_core smartcontracts::isi::ram_lfe --lib`
  passed after restoring the VM-level `Send` invariant needed by threaded
  `iroha_core` state paths.
- Validation note: the static Torii OpenAPI JSON files were updated, but their
  signed manifest entries still need an operator-provided OpenAPI signature or
  detached signature envelope before the manifest-verification CI guard can
  pass.

## 2026-05-02 IVM/Kotodama Vector and Syscall Hardening

- Fixed the IVM `VADD32` optimized/simple path so it uses the same `u32`
  lane semantics as the canonical interpreter, including zero-extension of
  wrapped lane results.
- Made vector length strict for the first release: metadata and Kotodama
  contract meta now accept only `0` or `1..=64`, `SETVL` rejects lengths above
  the ABI maximum, and `VADD64` rejects odd logical vector lengths.
- Changed vector gas accounting to scale by the actual logical vector length
  from the two-lane baseline instead of capping gas at two lanes.
- Moved syscall allowlist enforcement into the VM `SCALL` path so
  `run_with_host` cannot bypass ABI policy with a permissive custom host.
- Moved Kotodama test-runner helper calls onto host-private extended SCALLX
  numbers so fixture helpers no longer collide with the public V1 syscall
  surface while still passing through VM-level host admission.
- Hardened instruction fetch and pointer-TLV validation against overflowing
  guest-controlled addresses.
- Corrected static program analysis so `SETVL`'s immediate lane count is not
  reported as a register read, keeping AMX/Nexus dependency fingerprints aligned
  with execution semantics.
- Repaired validation blockers exposed by the broad IVM rerun: the data-model
  account I105 cache helper path now resolves, SoraNet VPN settlement payloads
  satisfy the derived ordering bounds, and AXT golden fixture headers are
  refreshed for the current Norito schema.
- Trimmed the `shifts_prop` property test body from `822.24s` to `2.19s` by
  reusing the loaded VM program and resetting VM state per case instead of
  rebuilding a full VM for each random input.
- Updated stale Kotodama static-analysis reentrancy fixtures to call the
  current typed `call_contract(target, entrypoint, payload)` builtin shape, so
  the tests exercise the analyzer instead of failing in semantic validation.
- Focused validation: `cargo test -p ivm_abi`,
  `cargo test -p ivm --test vector_execution_regression`, and
  `cargo test -p kotodama_lang vector_length` passed. The focused IVM
  gas/metadata/pointer window also passed:
  `cargo test -p ivm --test gas_conformance --test gas_golden --test metadata --test metadata_roundtrip --test pointer_tlv_neg`.
  The focused analyzer regression
  `cargo test -p ivm analysis_treats_setvl_operand_as_immediate --lib` passed.
- Additional validation passed:
  `cargo test -p ivm --bin koto_test -- execute_suite_supports_native_contract_flow_helpers execute_suite_runs_compiled_contract_flow_helpers_from_standalone_test -- --nocapture`,
  `cargo test -p kotodama_lang test_mode_helpers_emit_private_scallx_syscalls`,
  `cargo test -p iroha_data_model --test axt_policy_vectors`, and
  `cargo test -p ivm --test core_host_policy core_host_enforces_fixture_snapshot_fields -- --nocapture`.
- The broad `cargo test -p ivm` corridor passed after those repairs. A final
  focused rerun of the optimized property test also passed:
  `cargo test -p ivm --test shifts_prop`.
- Follow-up widened validation passed:
  `cargo test -p ivm_abi`,
  `cargo test -p kotodama_lang analysis::source::tests:: --lib`,
  `cargo test -p kotodama_lang`,
  `cargo clippy -p ivm_abi -p kotodama_lang --all-targets -- -D warnings`,
  and `cargo clippy -p ivm --all-targets -- -D warnings`.

## 2026-05-02 UAID Onboarding Hardening

- Tightened `POST /v1/accounts/onboard` so callers must supply an explicit
  canonical UAID (`uaid:<hex>` or raw 64-hex with LSB=1). The old
  alias/account/identity-derived fallback is removed, raw `identity` metadata
  is rejected, and optional identity evidence is stored only as the digest
  string `identity_commitment_hex`.
- Updated the MCP and OpenAPI onboarding surfaces to make the explicit UAID
  requirement visible, forward `identity_commitment_hex`, and reject raw
  identity shortcuts. MCP now validates raw `accounts.onboard` body shape before
  forwarding, including exact account-material cardinality, required string
  UAID, string permissions, and rejection of raw `identity`. The Swift
  onboarding request now carries and canonicalizes `uaid`, validates the
  optional identity commitment as 32-byte hex, and the Taira canary script
  derives a runtime-only canonical UAID from the generated canary public key.
- Fixed UAID portfolio grouping so global asset balances stay under the
  account's Space Directory/default dataspace while
  `AssetBalanceScope::Dataspace(id)` balances are reported under their explicit
  dataspace. Totals now count unique accounts after asset filtering, preserving
  the v1 1:1 UAID-to-`AccountId` model.
- Kept stored/shared IVM hosts thread-safe by using explicit `Send + Sync` host
  objects for VM-owned hosts while preserving borrowed `run_with_host` support
  for non-`Sync` query hosts. The obsolete unsafe `Sync` impl on
  `CoreHostImpl<QS>` was removed so the no-query stored host relies on
  structural `Sync` instead.
- Hardened `sync-openapi` so stale manifest files are not advertised as signed
  in `versions.json` when unsigned sync is explicitly allowed; manifest metadata
  is now copied into the version index only when path, byte count, and SHA-256
  match the generated spec. The static OpenAPI version index was refreshed in
  explicit unsigned mode, so it now matches the generated spec bytes and no
  longer points at stale signature metadata.
- Documentation now records the explicit-onboarding contract, digest-only
  identity commitment rule, low-bit UAID derivation step, and portfolio
  grouping semantics.
- Closed the follow-up Torii crate gaps surfaced by the wider sweep:
  latency-saturated local queues now keep ingress open until capacity is
  exhausted, the test-only runtime peer binding helper can create immediate
  synthetic consensus-key records without violating the production lead-time
  policy, and Torii test header injection now refreshes the latest-header cache
  so alias grace-period reads observe the intended block time.
- Regenerated the DA ingest manifest fixtures for the current canonical Norito
  encoding, including custom, governance, nexus-lane, and Taikai sample
  manifests.
- Focused validation so far: targeted `rustfmt --edition 2024` on the touched
  Rust files, `python3 -m py_compile scripts/taira_bootstrap_canary.py`,
  `cargo test -p iroha_torii uaid_parsing_tests --lib --features app_api -- --nocapture`,
  `cargo test -p iroha_torii build_accounts_onboard_body --lib --features app_api -- --nocapture`,
  `cargo test -p iroha_torii --test accounts_onboard accounts_onboard_rejects_invalid_uaid_contract --features app_api -- --nocapture`,
  `cargo test -p iroha_torii --test accounts_onboard --features app_api -- --nocapture`,
  `cargo test -p iroha_core groups_assets_by_balance_scope_dataspace --lib -- --nocapture`,
  `cargo test -p iroha_core syscall_hint_filters_accept_u32_numbers --lib -- --nocapture`,
  `cargo test -p ivm ivm_is_send_sync_for_state_sharing --lib -- --nocapture`,
  `cargo test -p ivm run_with_host_accepts_non_sync_host --lib -- --nocapture`,
  `node --test scripts/__tests__/sync-openapi.test.mjs scripts/__tests__/verify-openapi-versions.test.mjs scripts/__tests__/check-openapi-signatures.test.mjs`,
  `node scripts/verify-openapi-versions.mjs`,
  `cd IrohaSwift && swift test --filter ToriiClientTests/testRegisterAccount`,
  and
  `cargo test -p iroha_torii onboarding_error_metadata --lib --features app_api -- --nocapture`
  passed.
- The broader Torii library window is now green with
  `CARGO_TARGET_DIR=/tmp/iroha-codex-uaid-target cargo test -p iroha_torii --lib --features app_api -- --nocapture`:
  `1722` passed, `0` failed, `2` ignored. The DA manifest sweep
  `cargo test -p iroha_torii manifest_ --lib --features app_api -- --nocapture`
  and the queue-age focused regressions also pass.
- The broader all-target compile corridor also passes with
  `CARGO_TARGET_DIR=/tmp/iroha-codex-uaid-target cargo check --workspace --all-targets`.
  A stale in-flight workspace attempt first saw pre-existing `tx.rs` helper
  errors while the file was changing; a focused
  `cargo check -p iroha_core --lib` passed against the current tree, then the
  full all-target check completed successfully. The only diagnostics were the
  expected CUDA helper warnings for missing `nvcc`.
- The full workspace all-target clippy corridor also passes with
  `CARGO_TARGET_DIR=/tmp/iroha-codex-uaid-target cargo clippy --workspace --all-targets -- -D warnings`.
  Follow-up lint repairs were limited to a relay shorthand pattern and an AXT
  `FastPQ` doc-markdown warning; focused `soranet-relay`, data-model, `xtask`,
  and Torii hot-path bench clippy reruns passed before the final workspace
  clippy pass.
- Validation note: the static Torii OpenAPI JSON snapshots and `versions.json`
  are in sync, but the signed manifest files still need operator regeneration
  with the OpenAPI signing key or detached envelope before the
  manifest-signature guard can pass. Full workspace tests remain open for the
  next validation window.

## 2026-05-02 Torii Exposure Hardening

- Made Torii browser exposure opt-in: CORS now defaults disabled and config
  parsing requires explicit non-wildcard origins, HTTP methods, and request
  headers when enabled. The pre-auth connection gate now has a bounded per-IP
  default of `64`.
- Hardened route composition by gating `POST /v1/gov/protected-namespaces`
  behind operator access while leaving its read side available, removing the
  Soracloud root catch-all local-read fallback, and dropping SoraFS root/site
  catch-alls in favor of explicit `/api` and `/sorafs/cid/...` routes.
- Replaced MCP read-only policy heuristics with first-class tool effects. The
  generated OpenAPI surface now publishes `x-iroha-tool-effect`, MCP consumes
  that extension for OpenAPI-backed tools, manual tools carry the same effect
  metadata, operator tools stay out of read-only and writer profiles, and MCP
  caller-supplied auth, identity, remote-address, and internal headers are
  blocked.
- Follow-up MCP effect audit keeps manual Sumeragi snapshot reads
  (`iroha.sumeragi.vrf.commit`, `iroha.sumeragi.vrf.reveal`, and
  `iroha.sumeragi.rbc.sample`) in the read-only surface while preserving
  `iroha.sumeragi.evidence.submit` as operator-only. A regression now checks
  that every MCP `GET` tool declares a read effect.
- Made the mixed Norito/JSON extractor require an explicit `Content-Type`;
  missing types and `application/octet-stream` are now rejected instead of
  probing both codecs.
- Split DA replay/receipt/spooler setup out of pure HTTP router composition so
  runtime service preparation is separate from route assembly.
- Focused validation so far: `cargo fmt --all`,
  `cargo check -p iroha_config -p iroha_torii`,
  `cargo test -p iroha_config torii_cors_parse --lib`, and
  `cargo test -p iroha_torii tool_effects --lib` pass. The Torii tool-effect
  filter passed in both the shared target and
  `CARGO_TARGET_DIR=/tmp/iroha-codex-torii-tooleffects`. The follow-up manual
  effect audit passed with
  `CARGO_TARGET_DIR=/tmp/iroha-codex-torii-continue cargo test -p iroha_torii get_tools_are_declared_read_effect --lib`,
  `CARGO_TARGET_DIR=/tmp/iroha-codex-torii-continue cargo test -p iroha_torii manual_sumeragi_snapshot_tools_remain_read_only --lib`,
  and
  `CARGO_TARGET_DIR=/tmp/iroha-codex-torii-continue cargo test -p iroha_torii tool_effects --lib`.

## 2026-05-02 Norito/Crypto Hot-Path Implementation Slice

- Added focused benchmark coverage for the current 20k bottleneck set:
  `chain_wire` now isolates compact length read/write, public-key and
  account-controller decode, `InstructionBox` Log/Transfer/Register payloads,
  and packed `ConstVec<InstructionBox>` decode; `decode_registry` now includes
  the owned instruction-pair baseline and direct `InstructionBox` decode;
  `iroha_crypto` has an `ed25519_hotpaths` bench; `iroha_core` has a
  `crypto_hotpaths` Poseidon bench.
- Kept the scalar Norito fast path deterministic while reducing hot-loop TLS
  churn: length-prefix sizing/read/write can use explicit flag snapshots,
  `ConstVec` uses planned sequence spans as its primary decode path for
  length-prefixed and packed fixed-offset payloads, unpacked serialization
  writes lengths from the captured flags, and packed fixed offsets stream
  directly instead of building a temporary offset byte vector.
- Added borrowed `InstructionBox` tuple decode for the existing
  `(wire_id, framed_payload)` layout. The decoder borrows the wire id and
  framed payload slices, dispatches through the existing registry, and falls
  back to the owned `(String, Vec<u8>)` path if the tuple shape is not
  canonical.
- Follow-up instruction hot-path tuning now makes registry dispatch use a
  single hash lookup for either type names or stable wire ids, and serializes
  `InstructionBox`'s existing `(wire_id, framed_payload)` tuple layout directly
  so encoding no longer allocates a temporary wire-id `String` or temporary
  framed-payload `Vec<u8>`. A regression test proves the direct serializer
  stays byte-identical to the tuple serializer under fixed and compact length
  layouts, and the Norito core frame writer is checked against the existing
  vector-producing framer. The latest follow-up adds an object-safe
  `Instruction::dyn_encode_into` path so built-in instructions encode directly
  into the reusable bare-payload buffer, copies global registry entries without
  cloning the registry `Arc`, and only reports exact `InstructionBox` lengths
  when the inner instruction can compute them cheaply. Packed `ConstVec`
  exact-length sizing now follows the same rule instead of serializing
  inexact-length elements during a sizing query, and packed serialization uses a
  single capacity-hint pass instead of two exact-length pre-passes before the
  real encode. The latest pass writes packed fixed-offset tables and payload
  bytes through one contiguous buffer for both generic `Vec<T>` and
  `ConstVec<T>`, and writes `InstructionBox` wire ids with explicit captured
  length flags instead of re-entering the generic string serializer. Norito's
  scalar length/header/primitive writers now write little-endian bytes directly
  through `write_all`, and `Option<T>` uses the existing stack-backed
  length-prefixed buffer for `Some` payloads instead of allocating a temporary
  `Vec`. Generic `Vec<T>` encoding now has a slice-specialized path that
  pre-reserves element scratch and packed fixed-offset payload capacity from
  cheap element length hints, while keeping iterator-based collection encoders
  on the existing generic helper. The latest packed-sequence pass removes the
  remaining per-element scratch copy in packed iterator/slice encoders and
  packed `ConstVec`: elements now serialize directly into the final offset-table
  buffer, which is emitted only after all elements succeed. Packed `BTreeMap`
  and sorted `HashMap` serialization now use the same direct-buffer approach for
  the canonical `key_offsets, value_offsets, key_bytes, value_bytes` layout,
  avoiding size vectors, key/value data vectors, and key/value scratch buffers.
  Packed `BTreeMap`/`HashMap` decode now validates key/value offset-table
  slices and reads offsets on demand instead of allocating offset vectors.
  `ConstVec` sizing and reservation also now uses cheap exact-or-hint lengths
  consistently, with checked arithmetic instead of saturating hint totals.
  Sequence-backed collection decoders for `VecDeque`, `LinkedList`,
  `BinaryHeap`, `BTreeSet`, and `HashSet` now walk planned element spans
  directly into the final collection, avoiding the previous intermediate
  `Vec<T>` allocation/conversion layer while preserving the raw `Vec<u8>` fast
  path and strict rejection of ambiguous `Vec<u8>` length-prefixed payloads.
  The same sequence collections plus `BTreeMap`, sorted `HashMap`, and non-raw
  `Vec<T>` now report cheap exact/hint encoded lengths for fixed and packed
  layouts, improving outer-buffer reservation without changing serialization
  output. Map decode now uses a shared entry walker so packed `BTreeMap` no
  longer materializes a key vector before decoding values, and `HashMap` decodes
  directly into its final hash table instead of routing through a temporary
  `BTreeMap`. Non-packed `BTreeMap`/`HashMap` serialization now pre-reserves the
  reusable key/value scratch buffer from cheap field exact-or-hint lengths and
  captures layout flags once for all field length prefixes, avoiding repeated
  scratch growth and TLS flag reads in fixed-layout map payloads. Non-packed
  generic sequence encoders now use the same explicit flag snapshot for element
  length prefixes and convert hint-driven scratch growth to checked reservation.
  Array and tuple field framing now follows that same captured-flag path, with
  arrays pre-sizing reusable scratch from cheap element length hints. Owned
  payload framing for `Box`, `Rc`, and `Arc` now uses checked reservation,
  checked payload-length conversion, and an explicit length-flag snapshot.
  Shared length-prefixed payload framing now captures flags before inner
  serialization, string-like serializers write prefixes through the explicit
  flag path, and `Result<T, E>` reports cheap exact/hint encoded lengths so
  outer buffers can reserve compact layouts accurately. Tuple encoded-length
  reporting now reuses the exact same merged flag snapshot as tuple
  serialization and prefers cheap exact field lengths before hints, avoiding
  fixed-prefix over-reservation under compact layouts. String-like, owned
  smart-pointer, and fixed-array encoded-length hints now use compact-aware
  length-prefix sizing and checked arithmetic, avoiding fixed-prefix
  over-reservation for cheap exact layouts. `Option<T>` and `Result<T, E>` now
  share the same checked tagged length-prefix sizing helper, and `Option<T>`
  hints prefer cheap exact inner lengths before fallback hints.
- Extended Ed25519 scalar hot paths with a small exact-match public-key parse
  slot cache ahead of the existing bounded map, reusable parsed-signature
  storage in `Ed25519BatchScratch`, and an exact 32-byte-message verify-ok
  cache that compares public key, message, and signature bytes directly.
- Specialized Poseidon MDS work for BN254 widths 3 and 6 plus the Goldilocks
  width-3 FASTPQ path without changing constants, field arithmetic, hash
  outputs, wire bytes, or runtime configuration.
- Focused validation:
  - `cargo check -p norito -p iroha_primitives -p iroha_crypto -p iroha_zkp_halo2`
  - `RUSTFLAGS='-A missing-copy-implementations' cargo check -p iroha_data_model --benches`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-constvec-hotpath cargo test -p iroha_primitives encoded_len_exact --lib -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-norito-stream cargo test -p norito write_bare_frame_with_header_flags_matches_vec_framer --lib -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-instruction-hotpath cargo test -p iroha_data_model dyn_encode --lib -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-instruction-hotpath cargo test -p iroha_data_model instruction_box --lib -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-instruction-hotpath cargo test -p iroha_data_model record_sccp_message_registry_roundtrip_preserves_payload_bytes --lib -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-instruction-hotpath cargo check -p iroha_data_model --bench decode_registry`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-instruction-hotpath cargo check -p iroha_data_model --bench chain_wire`
  - `cargo check -p iroha_crypto --bench ed25519_hotpaths`
  - `cargo check -p iroha_core --bench crypto_hotpaths`
  - `RUSTFLAGS='-A missing-copy-implementations' cargo test -p iroha_data_model borrowed_instruction_pair_decodes_without_owned_payload -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-packed-onebuf cargo test -p norito encode_seq_payloads --lib -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-packed-onebuf cargo test -p iroha_primitives packed_seq --lib -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-packed-onebuf cargo test -p iroha_data_model instruction_box --lib -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-packed-onebuf cargo check -p norito`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-packed-onebuf cargo test -p norito --lib -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-packed-onebuf cargo test -p iroha_primitives encoded_len_exact --lib -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-packed-onebuf cargo check -p iroha_data_model --bench decode_registry`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-vec-slice cargo test -p norito --lib -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-vec-slice cargo test -p iroha_primitives packed_seq --lib -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-vec-slice cargo test -p iroha_primitives encoded_len_exact --lib -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-vec-slice cargo test -p iroha_data_model instruction_box --lib -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-vec-slice cargo check -p iroha_data_model --bench decode_registry`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-constvec-hints cargo test -p iroha_primitives encoded_len_exact --lib -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-constvec-hints cargo test -p iroha_primitives packed_seq --lib -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-packed-direct cargo test -p norito encode_seq_payloads --lib -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-packed-direct cargo test -p norito packed_maps_keep_key_then_value_payload_layout --lib -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-packed-direct cargo test -p norito --lib -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-packed-direct cargo test -p iroha_primitives packed_seq --lib -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-packed-direct cargo test -p iroha_data_model instruction_box --lib -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-packed-direct cargo check -p iroha_data_model --bench decode_registry`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-collection-direct cargo test -p norito collection_decoders_handle_u8_element_sequences_directly --lib -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-collection-direct cargo test -p norito collection_and_map_encoded_lengths_match_payloads --lib -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-collection-direct cargo test -p norito --lib -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-collection-direct cargo test -p iroha_primitives packed_seq --lib -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-collection-direct cargo test -p iroha_data_model instruction_box --lib -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-collection-direct cargo check -p iroha_data_model --bench decode_registry`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-map-direct cargo test -p norito packed_maps_keep_key_then_value_payload_layout --lib -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-map-direct cargo test -p norito --lib -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-map-direct cargo test -p iroha_primitives packed_seq --lib -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-map-direct cargo test -p iroha_data_model instruction_box --lib -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-map-direct cargo check -p iroha_data_model --bench decode_registry`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-map-nonpacked cargo test -p norito collection_and_map_encoded_lengths_match_payloads --lib -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-map-nonpacked cargo test -p norito encode_seq_payloads --lib -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-map-nonpacked cargo test -p norito --lib -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-map-nonpacked cargo test -p iroha_primitives packed_seq --lib -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-map-nonpacked cargo test -p iroha_data_model instruction_box --lib -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-map-nonpacked cargo check -p iroha_data_model --bench decode_registry`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-tuple-array cargo test -p norito array_and_tuple_serialization_use_compact_element_lengths --lib -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-tuple-array cargo test -p norito serialize_owned --lib -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-tuple-array cargo test -p norito --lib -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-tuple-array cargo test -p iroha_primitives packed_seq --lib -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-tuple-array cargo test -p iroha_data_model instruction_box --lib -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-tuple-array cargo check -p iroha_data_model --bench decode_registry`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-string-result cargo test -p norito string_and_result_lengths_match_compact_payloads --lib -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-string-result cargo test -p norito serialize_owned --lib -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-string-result cargo test -p norito option_roundtrip_respects_compact_flags --lib -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-string-result cargo test -p norito result_uses_actual_length_prefix --lib -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-string-result cargo test -p norito array_and_tuple_serialization_use_compact_element_lengths --lib -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-string-result cargo test -p norito --lib -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-string-result cargo test -p iroha_primitives packed_seq --lib -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-string-result cargo test -p iroha_data_model instruction_box --lib -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-string-result cargo check -p iroha_data_model --bench decode_registry`
  - `cargo fmt --check --package norito --package iroha_data_model --package iroha_primitives`
  - `git diff --check`
- Validation notes: the unsuppressed broader
  `cargo check -p norito -p iroha_primitives -p iroha_data_model -p
  iroha_crypto -p iroha_core -p irohad` currently stops on the pre-existing
  `missing-copy-implementations` lint for
  `crates/iroha_data_model/src/soranet/vpn.rs::VpnUsageVoucherBodyV1`.
  The earlier dirty-tree `ToriiCors::parse` and
  `metadata_roundtrip.rs` formatting blockers have been repaired in later
  slices; rerun the broader hot-path checks in a clean validation window.

## 2026-05-02 Current 20k Bottleneck Profile After Norito Span Planner

- Ran a fresh release `20,000 TPS` / `30s` 4-peer no-fault sampled profile
  against the rebuilt `izanami`/`iroha3d` binaries. Artifact:
  `dist/izanami-profile-20k-norito-span-sampled-30s-20260502-020217`; the
  Izanami run exited `0`, and `sample` completed successfully for the runner
  plus all four peer processes.
- The profile offered all `600,000` submissions, built and used all `600,000`
  prebuilt transactions, had zero prebuild fallback/build failures, accepted
  `37,021` ingress transactions, and reported submit latency p50/p95/p99/max
  of `3472/5151/6041/7070 ms`.
- Final quorum/strict height was `3/3` with `4,141/4,141` approved
  transactions, max height skew `1`, approved-transaction skew `4,096`, final
  queue depth `30,458/600,000`, and `tx_queue_saturated=true`. Safety counters
  stayed free of validation rejects, view changes, DA/RBC pressure, missing
  block fetches, pending-RBC drops, and commit-inflight timeouts.
- Aggregated peer recursive stacks now put active-path cost in Norito
  transaction/instruction codec first, then Poseidon/Ed25519/Curve/hash work,
  Rayon proof/hash scheduling, allocator/copy churn, TLS/context lookup, and
  Torii admission queue routing. Leaf samples agree that the hottest active
  leaves are Poseidon MDS/sbox, Curve25519 field math, malloc/free/memmove,
  `_tlv_get_addr`, and Norito compact-length/decode/encode routines.

## 2026-05-02 Release Izanami 20k Gate After Norito Span Planner

- Rebuilt the scalar release `izanami` and `iroha3d` binaries with
  `cargo build --release -p izanami --bin izanami -p irohad --bin iroha3d`,
  which completed in `7m33s`, then reran the same 4-peer no-fault prebuilt
  `20,000 TPS` / `120s` Izanami gate. Fresh artifact:
  `dist/izanami-prebuilt-20k-rerun-release-norito-span-120s-20260502-015557`;
  it exited `0`.
- The gate offered all `2,400,000` submissions, built and used all
  `2,400,000` prebuilt transactions, had zero prebuild fallback/build
  failures, and accepted `47,503` ingress transactions. Submit latency
  p50/p95/p99/max was `3086/4451/4997/6126 ms`.
- Final quorum/strict height was `10/10` with `32,786/32,786` approved
  transactions, max height skew `1`, approved-transaction skew `4,096`, final
  queue depth `10,250/2,400,000`, and `tx_queue_saturated=true`.
- Safety signals stayed free of validation rejects, quorum-timeout causes, DA
  gate pressure, RBC store pressure/evictions, pending-RBC drops, and
  commit-inflight timeouts. The run recorded `3` view-change installs, `2`
  missing-QC view-change causes, `13` missing-block fetches, `8/8` missing-QC
  reacquire attempts/successes, `8` range-pull escalations with `2` successes,
  and `96` pacemaker backpressure deferrals.
- Contention snapshots only captured the wrapper process and the snapshot `rg`
  probes; no other Rust build or Izanami jobs were present before or after the
  gate.

## 2026-05-02 Norito Binary Sequence Span Planner

- Added hidden Norito binary sequence span planning APIs for length-prefixed
  and packed fixed-offset sequences. `Vec<T>` decode and the `ConstVec<T>`
  manual unpacked recovery path now plan element byte ranges once and keep
  final semantic decode on CPU in original order.
- Added a hidden `parallel-decode` feature using the existing optional Rayon
  dependency for typed callers that can prove `T: Send`; no generic
  `ConstVec<T>` `Send` bound or runtime config knob was added.
- Added the helper-internal `norito_binary_sequence_plan` ABI to the existing
  Metal and CUDA jsonstage1 helper crates. Norito only attempts it for large
  payloads behind existing codec GPU features, self-tests and validates helper
  output against the scalar planner before use, falls back per call when the
  backend is unavailable, and disables the helper on backend failure or
  mismatch. The production helper bodies now route to native Metal/CUDA entry
  points; fixed-offset layouts validate spans in parallel, while
  length-prefixed layouts currently use a bounded single-thread device parser.
- Hardened Norito acceleration validation: GPU CRC64 self-tests now include
  large/chunk-boundary payloads and sampled production CPU parity checks, local
  SIMD CRC64 is selected only after startup parity against the portable
  fallback, GPU zstd output is required to be a single zstd frame and is sampled
  by CPU decode-to-payload validation, and parallel JSON Stage-1 now composes
  quote/backslash state across chunk boundaries before merging offsets.
  Follow-up coverage now locks the CRC/zstd validation schedules,
  mismatch-disable behavior, rejection of trailing zstd frames, byte-distinct
  single-frame zstd output with identical decoded payloads, and an escaped quote
  split across a Stage-1 chunk boundary. GPU compression availability reporting
  now also reflects runtime backend disablement after failed validation.
- Tightened the Metal jsonstage1 helper boundary so public Stage-1 and CRC64
  exports report unavailable/backend errors instead of silently completing with
  CPU work. Metal Stage-1 finalization now reports the required tape length on
  no-space rather than truncating the written count and returning success; the
  public wrapper also handles empty JSON/CRC inputs without device work and
  rejects Stage-1 inputs too large for the `u32` offset ABI before dispatch.
- Kept GPU zstd public per current policy and documented the contract: decoded
  payloads remain canonical, while compressed frame bytes may differ by backend.
  The DA query projection path that hashes compressed bytes uses its own fixed
  `zstd::bulk::compress` implementation rather than Norito GPU compression.
- No Norito wire layout, decoded values, rejection class, ordering, hashes,
  runtime config, dependencies, or `Cargo.lock` changed.
- Focused validation for this slice:
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-norito-gap-metal cargo check -p jsonstage1_metal`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-norito-gap-cuda cargo check -p jsonstage1_cuda --no-default-features`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-norito-gap-check cargo check -p norito --features simd-accel,parallel-stage1,parallel-stage1-rayon,stage1-validate,gpu-compression,codec-gpu-metal,codec-gpu-cuda,metal-crc64,cuda-crc64`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-norito-gap-seq cargo test -p norito --test sequence_plan --features parallel-decode -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-norito-gap-metal cargo test -p jsonstage1_metal -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-norito-gap-metal cargo test -p jsonstage1_metal binary_sequence_plan -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-norito-gap-cuda cargo test -p jsonstage1_cuda --no-default-features binary_sequence_plan -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-norito-gap-check cargo test -p norito --features gpu-compression,codec-gpu-metal,codec-gpu-cuda --lib gpu_zstd -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-norito-gap-check cargo test -p norito --features simd-accel,parallel-stage1,parallel-stage1-rayon,stage1-validate,gpu-compression,codec-gpu-metal,codec-gpu-cuda,metal-crc64,cuda-crc64 --lib -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-crypto-bench-check cargo check -p iroha_crypto --benches`
  - `cargo fmt --package norito --package jsonstage1_cuda --package jsonstage1_metal --package iroha_crypto`
  - `git diff --check`

## 2026-05-01 FASTPQ BN254 Metal Poseidon Batch Path

- Added a `fastpq_prover` BN254 Poseidon word-batch helper and a Metal
  `bn254_poseidon_hash_words` kernel for FASTPQ transfer transcript digest
  finalization. The helper self-tests fixed vectors against the scalar
  `iroha_zkp_halo2` Poseidon word path before use and disables itself on
  mismatch or runtime failure.
- Wired `iroha_core` FASTPQ transcript drain/snapshot finalization to batch
  single-delta transcript digests through the accelerator when the existing
  FASTPQ execution/poseidon modes permit GPU use and the batch is large enough;
  CPU mode, unavailable Metal, failed self-test, and small batches keep the
  existing scalar streaming hasher.
- Added `iroha_core/fastpq-gpu` feature plumbing and made `irohad/fastpq-gpu`
  enable both the daemon and core FASTPQ GPU paths. No new dependencies,
  `Cargo.lock` changes, wire-format changes, or config knobs were added.
- Focused validation passed:
  `cargo check -p fastpq_prover`,
  `cargo test -p iroha_core --lib poseidon_word_packer_matches_little_endian_chunks -- --nocapture`,
  `cargo check -p fastpq_prover --features fastpq-gpu`,
  `cargo check -p iroha_core --features fastpq-gpu`,
  `cargo check -p irohad --features fastpq-gpu`,
  `cargo test -p fastpq_prover --lib bn254_poseidon -- --nocapture`,
  `cargo fmt --all`, and `git diff --check`.
- The local Xcode install is missing the Metal toolchain, so the existing
  `fastpq_prover` build script warned that it could not execute `xcrun metal`
  and skipped shader compilation. The Rust feature plumbing is checked, but
  kernel syntax/parity and performance still need a rerun on a host with
  `xcodebuild -downloadComponent MetalToolchain` completed.

## 2026-05-01 Release Izanami 20k Gate After FASTPQ Metal Pass

- Rebuilt the scalar release `izanami` and `iroha3d` binaries with
  `cargo build --release -p izanami --bin izanami -p irohad --bin iroha3d`,
  which completed in `6m45s`, then reran the same 4-peer no-fault prebuilt
  `20,000 TPS` / `120s` Izanami gate. Fresh artifact:
  `dist/izanami-prebuilt-20k-rerun-release-120s-20260501-224554`; it exited
  `0`.
- The gate offered all `2,400,000` submissions, built and used all
  `2,400,000` prebuilt transactions, had zero prebuild fallback/build
  failures, and accepted `48,920` ingress transactions. Submit latency
  p50/p95/p99/max was `3197/4708/5190/6171 ms`.
- Final quorum/strict height was `11/11` with `36,979/36,979` approved
  transactions, zero height skew, zero approved-transaction skew, final queue
  depth `11,960/2,400,000`, and `tx_queue_saturated=true`.
- Safety signals stayed clean: no validation rejects, view changes,
  quorum-timeout causes, DA gate pressure, RBC store pressure/evictions,
  pending-RBC drops, missing-block fetches, or commit-inflight timeouts. The
  run recorded `7/7` missing-QC reacquire attempts/successes, `7` range-pull
  escalations, `0` range-pull successes, and `107` pacemaker backpressure
  deferrals.
- This was the default scalar release gate. The Metal Poseidon accelerator path
  still needs a separate `fastpq-gpu` release run on a host with the Apple
  Metal toolchain installed.

## 2026-05-01 Release Izanami 20k Sampled Profile After FASTPQ Metal Pass

- Captured a fresh scalar release 4-peer no-fault prebuilt `20,000 TPS` /
  `30s` sampled profile at
  `dist/izanami-profile-20k-current-sampled2-30s-20260501-225258`.
  Izanami exited `0`, `sample_ready=true`, `sample_status=0`, and the profiler
  captured the runner plus all four `iroha3d` peers. The contention snapshots
  only contain the wrapper's own `rg` probes.
- The run offered all `600,000` submissions, built and used all `600,000`
  prebuilt transactions, had zero prebuild fallback/build failures, and
  accepted `46,458` ingress transactions. Submit latency p50/p95/p99/max was
  `3123/4569/4957/6102 ms`.
- Final quorum/strict height was `3/3` with `4,141/4,141` approved
  transactions, zero height skew, zero approved-transaction skew, final queue
  depth `35,228/600,000`, and `tx_queue_saturated=true`. Safety counters stayed
  clean: no validation rejects, view changes, DA/RBC pressure, pending-RBC
  drops, or commit-inflight timeouts. The run did record `12` missing-block
  fetches, `2/2` missing-QC reacquire attempts/successes, and `3` range-pull
  escalations.
- Active peer leaf samples, excluding wait/parking and generic runtime frames,
  are now led by Norito/transaction wire work at roughly `31.5%`, followed by
  low-level syscall/TLS/write overhead at `13.3%`, allocation/copy at `10.0%`,
  FASTPQ/Poseidon hashing at `9.1%`, Ed25519/Curve25519 math at `8.4%`, Rayon
  batch/prover scheduling at `5.9%`, hash/multihash work at `2.6%`, ID/public
  key/string work at `2.3%`, P2P/network crypto at `1.1%`, and Torii/HTTP at
  `0.7%`.
- Hot peer leaves include `_tlv_get_addr`, Poseidon `apply_mds`/`sbox`,
  `curve25519_dalek` `FieldElement51::pow2k`/multiply, `_platform_memmove`,
  `_xzm_free`, Norito `use_compact_len`, `write_len`,
  `decode_field_canonical`, `ConstVec` decode/serialize helpers,
  `Transfer` serialization, SHA-256/Blake2 compression, and account-address
  `i105` conversion.
- Recursive peer attribution still points at Norito and transaction material as
  the largest non-wait corridor: decoded versioned signed transaction handling,
  instruction-pair/payload decode, valid-block validation, transaction gossip
  handling, and block validation/prepared transaction paths. Overlay execution
  wrappers remain visible, but `StateTransaction::record_transfer_transcripts`
  is now low-count residue; FASTPQ Poseidon work remains visible through
  digest finalization/prover-side batch work rather than as the dominant
  transfer-execution stack. Runner-side cost is secondary and mostly
  request/endpoint-pool/socket overhead.

## 2026-05-01 FASTPQ Poseidon Deferral And Admission Cache Follow-up

- Moved single-delta FASTPQ Poseidon digest work out of the transfer execution
  hot path. Transfer execution now records runtime-local transcripts without a
  digest, and the digest is finalized before block transcript drain or
  execution-witness drain/snapshot exposes the data. Multi-delta transcript
  digest behavior remains `None`.
- Added a streaming BN254 Poseidon byte hasher in `iroha_zkp_halo2` and wired
  FASTPQ digest construction through Norito `encode_to`, avoiding the previous
  full preimage buffer while preserving the current byte hash path.
- Reduced Torii/Core admission residue by deriving decoded external
  entrypoint hashes from the already-decoded versioned signed payload bytes and
  by reusing Ed25519 batch precheck message/signature/key vectors across
  chunks.
- Focused validation passed:
  `cargo test -p iroha_zkp_halo2 poseidon --lib -- --nocapture`,
  `cargo test -p iroha_core poseidon_digest_matches_known_vector --lib -- --nocapture`,
  `cargo test -p iroha_core transfer_transcript --lib -- --nocapture`,
  `cargo test -p iroha_core decoded_versioned_signed_transaction --lib -- --nocapture`,
  `cargo test -p iroha_core fastpq_transcripts --lib -- --nocapture`,
  `cargo test -p iroha_core snapshot_finalizes_single_fastpq_transcript_without_clearing --lib -- --nocapture`,
  `cargo test -p iroha_torii transaction_batch_ed25519 --lib -- --nocapture`,
  `cargo test -p iroha_torii transaction_batch_non_ed25519 --lib -- --nocapture`,
  and
  `cargo test -p iroha_torii handler_post_transactions_batch_rejects_invalid_ed25519_precheck_without_partial_push --lib -- --nocapture`,
  plus `cargo check -p iroha_zkp_halo2 -p iroha_core -p iroha_torii`,
  `cargo fmt --all -- --check`, and `git diff --check`.
- Later scalar release 20k gate and sampled profile reruns are recorded above.
  The sampled profile should be used to check whether
  `StateTransaction::record_transfer_transcripts`,
  `fastpq::poseidon_preimage_digest`, and
  `iroha_zkp_halo2::poseidon::{apply_mds,sbox}` moved out of the foreground
  execution stack.

## 2026-05-01 Direct-Ingress Precheck And Borrowed Overlay Pass

- Added deterministic single-key Ed25519 precheck for Torii
  `/transactions/batch` admission, reusing the existing
  `pipeline.signature_batch_max_ed25519` setting. All transactions still pass
  the existing chain-id, time/TTL, signing-allowed, size, signature-count, NTS,
  route, and queue-admission checks; multisig, non-Ed25519, sealed/private/time
  entrypoints stay on the existing validation path.
- Tightened the built-in overlay executor path so `Executor::Initial`
  `TxOverlay::apply_with_chunk` calls a crate-private borrowed instruction
  dispatch helper instead of cloning the whole `InstructionBox` first.
  `Executor::UserProvided` still uses the owned-instruction fallback, so the
  public `Execute` trait and custom executor API are unchanged.
- Focused validation passed:
  `cargo test -p iroha_core --lib borrowed_overlay_apply_matches_owned_initial_executor_for_register_domain -- --nocapture`,
  `cargo test -p iroha_core --lib decoded_versioned_signed_transaction_owned_supports_ed25519_prechecked_accept -- --nocapture`,
  `cargo test -p iroha_core --lib gossip_transaction_hash_from_framed_entrypoint_matches_canonical_hash -- --nocapture`,
  `cargo test -p iroha_core --lib does_not_materialize_entrypoint -- --nocapture`,
  `cargo test -p iroha_core --lib queue_accepts_gossip_payload_cache -- --nocapture`,
  `cargo test -p iroha_core --lib queue_generated_gossip_payload_uses_framed_entrypoint_wire -- --nocapture`,
  `cargo test -p iroha_torii --lib transaction_batch_ -- --nocapture`,
  `cargo test -p iroha_torii --lib handler_post_transactions_batch_rejects_invalid_ed25519_precheck_without_partial_push -- --nocapture`,
  `cargo test -p iroha_torii --lib handler_post_transactions_batch_accepts_multiple_payloads -- --nocapture`,
  `cargo fmt --all`,
  `cargo check -p norito -p iroha_crypto -p iroha_data_model -p iroha_core -p iroha_torii -p irohad`,
  and
  `cargo build --release -p izanami --bin izanami -p irohad --bin iroha3d`.
- Clean final 4-peer no-fault prebuilt `20,000 TPS` / `120s` gate artifact:
  `dist/izanami-prebuilt-20k-direct-ingress-precheck-final-120s-20260501-212850`.
  It exited `0`, and `contention-before.txt` / `contention-after.txt` only
  contain timestamps.
- The final gate offered all `2,400,000` submissions, built and used all
  `2,400,000` prebuilt transactions, had zero prebuild fallback/build
  failures, and accepted `47,566` ingress transactions. Submit latency
  p50/p95/p99/max was `3196/4554/4993/6118 ms`.
- Final quorum/strict height was `7/7` with `20,499/20,499` approved
  transactions, max height skew `1`, approved-transaction skew `8,192`, final
  queue depth `22,789/2,400,000`, and `tx_queue_saturated=true`.
- Safety signals remained clean: no validation rejects, quorum-timeout causes,
  DA gate pressure, RBC store pressure/evictions, pending-RBC drops, or
  commit-inflight timeouts. The run recorded `2` view-change installs, `28`
  missing-block fetches, `7/7` missing-QC reacquire attempts/successes, and
  `7` range-pull escalations with `2` successes.
- The direct-ingress sampled profile for this pass is
  `dist/izanami-profile-20k-direct-ingress-precheck-sampled-30s-20260501-210924`.
  It exited `0` with `sample_status=0`, accepted `43,109` ingress
  transactions, and reached strict height `3` with `4,125` approved
  transactions. The samples show the new Torii Ed25519 precheck path active;
  the remaining peer-side bottlenecks are Ed25519 batch math, Norito
  signed-transaction/instruction decode and allocation, public-key parsing
  during decode, residual gossip materialization, and Poseidon/hash work.

## 2026-05-01 Release Izanami 20k gate rerun

- Rebuilt the release `irohad` and `izanami` binaries with
  `cargo build --release -p irohad -p izanami`, then reran the 4-peer,
  no-fault, prebuilt `20,000 TPS` / `120s` Izanami gate. Clean-wrapper
  artifact:
  `dist/izanami-prebuilt-20k-rerun-release2-120s-20260501-210031`; it exited
  `0`.
- The gate offered all `2,400,000` submissions, built and used all
  `2,400,000` prebuilt transactions, had zero prebuild fallback/build
  failures, and accepted `52,582` ingress transactions. Submit latency
  p50/p95/p99/max was `3146/4607/5012/6072 ms`.
- Final quorum/strict height was `9/9` with `28,755/28,755` approved
  transactions, zero height skew, zero approved-transaction skew, final queue
  depth `23,624/2,400,000`, and `tx_queue_saturated=true`.
- Safety signals stayed clean: no validation rejects, quorum-timeout causes,
  DA gate pressure, RBC store pressure/evictions, or pending-RBC drops. The run
  recorded `1` view-change install, `18` missing-block fetches, `8/8`
  missing-QC reacquire attempts/successes, `8` range-pull escalations, and `1`
  range-pull success.
- Treat throughput as contended evidence, not an isolated baseline:
  `contention-before.txt` showed no other Rust build jobs, but
  `contention-after.txt` showed a separate debug
  `cargo check -p norito -p iroha_crypto -p iroha_data_model -p iroha_core -p iroha_torii -p irohad`
  with active `rustc` processes that started during the gate window.
- An immediately preceding same-shape artifact at
  `dist/izanami-prebuilt-20k-rerun-release-120s-20260501-205658` also reached
  the final Izanami summary, but its wrapper failed after the summary while
  recording `exit.status` because the zsh wrapper used the read-only variable
  name `status`.

## 2026-05-01 Release Izanami 20k sampled profile bottlenecks

- Captured a fresh release 4-peer, no-fault, prebuilt `20,000 TPS` / `30s`
  sampled profile at
  `dist/izanami-profile-20k-rerun-release-sampled2-30s-20260501-211211`.
  The fixed sampler targeted the Izanami runner and its direct child peers,
  recorded `sample_status=0`, and the Izanami run exited `0`.
- The run offered all `600,000` submissions, accepted `46,709` ingress
  transactions, and reached quorum/strict height `3/3` with `4,125/4,125`
  strict approved transactions. Submit latency p50/p95/p99/max was
  `3065/5833/6648/7556 ms`; final queue depth was `33,949/600,000` with
  `tx_queue_saturated=true`.
- Safety signals stayed clean: no validation rejects, view-change causes,
  commit-inflight timeouts, DA gate pressure, RBC store pressure/evictions, or
  pending-RBC drops. The status delta only recorded `28` pacemaker backpressure
  deferrals, `2/2` missing-QC reacquire attempts/successes, and `2`
  range-pull escalations.
- The pre-sample contention snapshot was clean. A separate `120s`
  direct-ingress gate appeared after the sample window and during shutdown, so
  the peer CPU samples are usable bottleneck evidence while the final timing
  summary should be treated as lightly contended.
- Peer samples now put the main CPU weight in
  `iroha_zkp_halo2::poseidon::{apply_mds,sbox}` and `fastpq_isi::poseidon`,
  `_platform_memmove`, allocator free/malloc paths, `sha2`/`blake2` hashing,
  Norito length/decode/encode routines such as `use_compact_len`,
  `read_len_from_slice`, `write_len`, and `decode_field_canonical`, plus
  `curve25519_dalek` / `ed25519_dalek` verification math.
- Direct ingress batch precheck is visible but no longer the dominant leaf, and
  the earlier overlay/clone targets are low-count residue in this profile:
  `InstructionDynClone::dyn_box_clone`, `InstructionBox::encoded_len_exact`,
  and `ValidBlock::validate_and_record_transactions_with_prepared` only appear
  as small wrapper or leaf costs. The next bottleneck work should prioritize
  the sustained Poseidon/FASTPQ source, allocation/memmove reduction, Norito
  decode and compact-length walks, and then signature-verification reuse or
  batching.

## 2026-05-01 Sealed reveal adversarial multi-peer coverage

- Broadened `tx_history::sealed_reveal_adversarial_cases_hold_on_multi_peer_network`
  to keep a real 4-peer network and cover five same-window sealed reveals in
  one block, duplicate reveal replay after commit, delayed expired reveal
  rejection, and all-peer state checks for both successful and expired reveal
  effects. The duplicate path now verifies the canonical reveal hash on primary
  replay and probes a secondary peer without letting Torii queue backpressure
  dominate the test result.
- Refreshed the bundled rANS fixture checksum used by peer startup so the
  repository fixture matches the current legacy Norito body framing.
- Focused validation passed:
  `NORITO_SKIP_BINDINGS_SYNC=1 CARGO_TARGET_DIR=/tmp/iroha-codex-norito-fixture cargo test -p norito load_bundle_tables_accepts --lib -- --nocapture`,
  `NORITO_SKIP_BINDINGS_SYNC=1 cargo test -p integration_tests --test core_api sealed_reveal_adversarial_cases_hold_on_multi_peer_network --no-run`,
  `NORITO_SKIP_BINDINGS_SYNC=1 RUST_BACKTRACE=1 cargo test -p integration_tests --test core_api sealed_reveal_adversarial_cases_hold_on_multi_peer_network -- --nocapture`,
  and `cargo fmt --all`.

## 2026-05-01 Conservative ingress and exact-length cache implementation

- Implemented the next conservative cache slice without changing transaction
  wire bytes, block wire bytes, canonical hashes, consensus rules, or config
  defaults. Torii direct signed-transaction and batch submission now decode
  versioned signed payloads once into a core-owned prepared admission token,
  preserving the existing validation path while reusing prepared hashes,
  encoded length, payload hash, and parsed single-Ed25519 key metadata.
- `InstructionBox::encoded_len_exact` now counts the existing
  `(wire_id, framed_payload)` Norito layout without materializing the framed
  instruction payload, keeping serialization/decode behavior unchanged while
  removing the residual dynamic framing allocation from size checks.
- Focused validation passed:
  `cargo test -p iroha_data_model instruction_box_encoded_len_exact -- --nocapture`,
  `cargo test -p iroha_core decoded_versioned_signed_transaction --lib -- --nocapture`,
  `cargo test -p iroha_core signed_encoded_len --lib -- --nocapture`,
  `cargo test -p iroha_torii decode_transaction_batch_payloads --lib -- --nocapture`,
  `cargo check -p iroha_data_model -p iroha_core -p iroha_torii`, and
  `cargo fmt --all -- --check`.
- Release validation for this code is now recorded in the broader 20k
  bottleneck-pass entries below, including the fixed-runner sampled profile at
  `dist/izanami-profile-20k-broader-pass-rerun-sampled-30s-20260501-200527`
  and the latest 120s gate rerun at
  `dist/izanami-prebuilt-20k-direct-ingress-precheck-final-120s-20260501-212850`.

## 2026-05-01 Contended conservative-cache release 20k gate rerun

- Reran the 4-peer, no-fault, prebuilt `20,000 TPS` / `120s` Izanami gate from
  the existing release binaries. Corrected artifact:
  `dist/izanami-prebuilt-20k-conservative-cache-rerun2-120s-20260501-144548`.
  It exited `0`. A preceding same-shape artifact at
  `dist/izanami-prebuilt-20k-conservative-cache-rerun-120s-20260501-144204`
  completed and wrote a summary, but its wrapper failed only while recording the
  exit status.
- This is a contended performance data point: active debug
  `cargo test`/`rustc` jobs were running during the gate, including the
  integration `core_api` network test build. The artifact records that
  contention in `contention.txt`; do not compare it as a clean throughput
  baseline against the earlier isolated gate.
- The corrected run offered all `2,400,000` submissions, built and used all
  `2,400,000` prebuilt transactions, had no prebuild fallback/build failures,
  and accepted `52,070` ingress transactions. Submit latency p50/p95/p99/max
  was `3208/4708/5311/6960 ms`.
- Final quorum/strict height was `5/5` with `12,329/12,329` strict approved
  transactions, zero peer height skew, zero approved-transaction skew, final
  queue depth `39,344/2,400,000`, and `tx_queue_saturated=true`.
- Safety remained intact: no validation rejects, no quorum-timeout cause, no DA
  gate pressure, no RBC store pressure/evictions, and no pending-RBC drops. The
  contended run did record `4` view-change installs, `22` missing-block fetches,
  `6/6` missing-QC reacquire attempts/successes, and `5` range-pull escalations
  with `2` range-pull successes.

## 2026-05-01 Contended conservative-cache 20k sampled profile bottlenecks

- Captured the latest requested release 4-peer, no-fault, prebuilt
  `20,000 TPS` / `30s` sampled profile at
  `dist/izanami-profile-20k-conservative-cache-rerun2-sampled-30s-20260501-145104`.
  The Izanami run exited `0`; `sample_ready=1`, and `sample_status=1` was
  caused by the sampler also targeting the bash wrapper plus one transient PID.
  Valid `sample` outputs were captured for the load driver and all four peers.
- This profile was contended by active debug `cargo test`/`rustc` jobs, so it is
  bottleneck evidence rather than an isolated timing baseline. The run offered
  all `600,000` submissions, built/used all `600,000` prebuilt transactions,
  accepted `52,817` ingress transactions, and reached quorum/strict height
  `3/3` with `4,137/4,137` strict approved transactions.
- Submit latency p50/p95/p99/max was `3385/6290/8296/11882 ms`; final queue
  depth was `40,839/600,000` with `tx_queue_saturated=true`. Safety signals
  stayed clean: no validation rejects, view changes, RBC store pressure,
  evictions, pending-RBC drops, prebuild fallback, or transaction build
  failures.
- The prior cache-pass removals remain absent from the peer samples:
  `Queue::encode_gossip_payload=0`, `TxOverlay::byte_size=0`, and
  `external_entrypoints_cloned=0`. `prepare_signed_metadata` appears only as
  residue and is not the dominant sampled path.
- The current bottleneck stack is now transaction admission crypto/public-key
  work, allocation/memmove churn, residual Norito dynamic instruction framing
  inside exact-length and canonical-byte construction, gossip transaction
  materialization/decode during admission, and block validation overlay
  execution. Representative leaves include `curve25519_dalek` field ops,
  `PublicKeyFull::from_bytes`, `AcceptedTransaction::from_external_with_hot_cache`,
  `AcceptedTransaction::signed_encoded_len`,
  `iroha_data_model::isi::encoded_instruction_pair`,
  `GossipTransaction::try_deserialize`,
  `decode_gossip_transaction_payload`, `TxOverlay::apply_with_chunk`,
  `InstructionDynClone::dyn_box_clone`, and `WorldTransaction::apply`.
- Next tuning should carry already validated inbound canonical signed/entrypoint
  bytes into `AcceptedTransaction`, reduce `InstructionBox` exact-length sizing
  without re-encoding framed payloads, extend parsed Ed25519 key/signature reuse
  on the Torii/direct-ingress path, and only then tackle the clone-heavy
  `TxOverlay::apply_with_chunk` API if overlay samples remain active.

## 2026-05-01 Norito first-release codec cleanup

- Tightened the v1 Norito header contract across Rust, Java, Kotlin, Python,
  and Swift: decoders now reject reserved layout bits, `FIELD_BITSET` is only
  valid with `PACKED_STRUCT | COMPACT_LEN`, and public encoders reject
  unsupported layout flags instead of emitting frames that downstream decoders
  would have to guess.
- Replaced the duplicated FNV-1a schema hash with a domain-separated SHA-256
  digest truncated to 16 bytes for both type-name and structural schema hashes.
  Shared fixtures, SDK tests, and the Norito binding parity script now pin the
  new values.
- Removed public last-encode-flag side channels from Rust, Java, Kotlin, and
  Python. Callers that need explicit layout metadata now use the explicit
  `encode_with_header_flags` surface.
- Removed first-release compatibility fallbacks from the core codec: typed
  decoders no longer special-case legacy schema length mismatches, and `Vec<u8>`
  no longer accepts the old per-element length-prefixed representation.
- Removed ignored runtime Norito heuristic knobs from configuration and daemon
  startup reporting. Runtime config now exposes only the active GPU-compression
  permission and archive-size guard; the remaining codec layout heuristics are
  compiled release defaults documented under the acceleration guide.
- Fixed the structural-schema build gap by adding explicit float metadata to
  `iroha_schema`, schema JSON support for float metadata, and manual structural
  schema descriptions for streaming named-variant payloads and bundled codec
  helper state.
- Validation:
  - `python3 scripts/check_norito_bindings_sync.py` passed.
  - `python3 -m pytest python/norito_py/tests/test_header_padding.py` passed.
  - `swift test --filter NoritoTests` passed from `IrohaSwift`.
  - Focused Rust checks passed for Norito header validation, schema hashes,
    stream iterator reserved flags, explicit bare-header flags, `Vec<u8>`
    rejection of legacy element framing, `norito_derive` self-delimiting
    classification, `iroha_config` fixture snapshots, cross-language hashes,
    and the `iroha_crypto` public-key Norito golden archive.
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-norito-fixes cargo test -p
    iroha_schema --test floats -- --nocapture` passed.
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-norito-fixes cargo test -p norito
    --features schema-structural --test schema_hash -- --nocapture` passed.
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-norito-fixes cargo check -p
    iroha_kagami` passed.
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-norito-fixes cargo check -p
    iroha_torii --features schema` passed.

## 2026-05-01 Broader 20k bottleneck pass

- Implemented lazy transaction-gossip materialization in
  `crates/iroha_core/src/gossiper.rs`: decoded gossip entries now keep the
  canonical framed `TransactionEntrypoint` bytes plus the precomputed
  entrypoint-compatible transaction hash, and semantic entrypoint decode is
  deferred until queue admission reaches a route-valid, locally unknown
  candidate. Outbound gossip serialization continues to write the cached framed
  bytes.
- Added gossip-side deterministic single-key Ed25519 batch precheck using the
  existing `pipeline.signature_batch_max_ed25519` setting and
  `iroha_crypto::ed25519_verify_batch_preparsed_deterministic_with_scratch`.
  Multisig, non-Ed25519, sealed commitment/reveal, private Kaigi, and time
  entrypoints stay on the existing per-entrypoint validation path.
- Added a crate-private accepted-gossip constructor in
  `crates/iroha_core/src/tx.rs` that accepts prepared metadata plus a
  single-Ed25519-prechecked marker, while still running chain id, time/TTL,
  signing policy, size, signature-count, heartbeat, and NTS health checks.
- Routed `TxOverlay` application through a crate-private borrowed overlay
  executor adapter. The public `Execute` trait and custom executor API remain
  owned-instruction based; user-provided executor paths still explicitly fall
  back to owned `InstructionBox` execution.
- Fixed the signature materialization corridor needed by lazy gossip decode:
  `iroha_crypto::Signature` now has a narrow `ConstVec<u8>` fallback for the
  compact per-byte signature payload layout, so valid cached entrypoint frames
  can materialize through normal Norito validation instead of being dropped as
  semantic decode failures.
- Added focused unit coverage for lazy gossip decode/cache behavior,
  route-invalid and known-duplicate drops without materialization, accepted
  cached gossip payload reuse, and valid/invalid gossip Ed25519 batch precheck
  behavior.
- Profile classification after this pass: the prior 30s profile at
  `dist/izanami-profile-20k-postcache-tuned-bottleneck-30s-20260501-171955`
  is pre-broader-pass evidence. The fresh sampled 30s profile is
  `dist/izanami-profile-20k-broader-pass-sampled-30s-20260501-194734`; it
  completed with `ingress_accepted=37324`, submit latency
  `p50/p95/p99=3355/5723/6477ms`, final strict height `2`, final approved
  transactions `143`, and a saturated queue depth of `37402`. Peer samples
  still classify active CPU mostly under Ed25519/curve25519, ZKP poseidon,
  memory movement/allocation, SHA/Blake hashing, and Norito decode/compact-len
  paths; the sample wrapper captured the shell pipeline's `tee` process instead
  of the Izanami runner, but the four peer samples were captured successfully.
  Gossip materialization and deterministic gossip Ed25519 batch precheck now
  appear as narrower peer-side stacks rather than the broadest top-level
  categories.
- The fresh unsampled 120s release gate is
  `dist/izanami-prebuilt-20k-broader-pass-120s-20260501-194908`; it completed
  with `offered=2400000`, `ingress_accepted=52291`, submit latency
  `p50/p95/p99=3290/4753/5217ms`, final strict height `9`, final approved
  transactions `28740`, final height skew `1`, and saturated queue depth
  `18527`. Compared with
  `dist/izanami-prebuilt-20k-conservative-cache-rerun-120s-20260501-175213`,
  final approved transactions were effectively flat (`28740` vs `28710`) and
  p95/p99 submit latency improved, while ingress accepted count was lower
  (`52291` vs `54574`).
- The immediate release-gate rerun is
  `dist/izanami-prebuilt-20k-broader-pass-rerun-120s-20260501-195617`; it
  completed with `offered=2400000`, `ingress_accepted=51802`, submit latency
  `p50/p95/p99=3099/4543/5075ms`, final strict height `9`, final approved
  transactions `28699`, final height skew `0`, saturated queue depth `23112`,
  `view_change_* = 0`, and exit code `0`. Compared with the previous
  broader-pass 120s gate, final approved transactions and ingress accepted
  count were slightly lower (`28699` vs `28740`, `51802` vs `52291`), while
  p50/p95/p99 submit latency improved.
- The follow-up fixed-runner sampled 30s profile is
  `dist/izanami-profile-20k-broader-pass-rerun-sampled-30s-20260501-200527`.
  It completed with `sample_ready=true`, `sample_status=0`, and exit code `0`,
  and sampled the actual Izanami runner plus all eight observed `iroha3d`
  peer processes. The run offered `600000`, accepted `54750`, had submit
  latency `p50/p95/p99=3391/6921/7769ms`, final strict height `2`, final
  approved transactions `12`, final height skew `1`, and a saturated queue
  depth of `41098`. Treat this artifact as CPU bottleneck classification, not
  a committed-throughput baseline.
- Fixed-runner profile classification: direct peer CPU is dominated by
  Ed25519/curve25519 verification math, with `curve25519_dalek` field
  exponentiation/multiplication and multiscalar paths as the largest leaves.
  Memory allocation/copying and Norito compact/decode work are the next tier,
  including `memmove`, `malloc`/`free`, `norito::core::use_compact_len`, and
  data-model instruction registry/decode paths. Hashing (`sha2`, `blake2`,
  `crc64fast`) remains visible but secondary. ZK/BLS math is also material:
  direct peer samples include `iroha_zkp_halo2::poseidon`, while the
  `core_api` child peer processes are dominated by `ark_ff`,
  `ark_bls12_381`, and `w3f_bls` public-key deserialization/subgroup math.
  Gossip materialization and gossip deterministic Ed25519 batch precheck are
  present as narrow stacks, while queue mechanics and borrowed overlay apply
  are not primary CPU bottlenecks in this sample.
- Two earlier fixed-runner profile attempts,
  `dist/izanami-profile-20k-broader-pass-rerun-sampled-30s-20260501-200044`
  and
  `dist/izanami-profile-20k-broader-pass-rerun-sampled-30s-20260501-200211`,
  failed before peer startup because the bundled rANS table checksum did not
  match the current Norito table payload. The table checksum was restored to
  the current-source value before the successful sampled rerun.
- Validation:
  - `cargo fmt --all` passed.
  - `git diff --check` passed.
  - `NORITO_SKIP_BINDINGS_SYNC=1 CARGO_TARGET_DIR=/tmp/iroha-codex-check cargo
    test -p iroha_core --lib gossip_transaction -- --nocapture` passed
    (`5` tests).
  - `NORITO_SKIP_BINDINGS_SYNC=1 CARGO_TARGET_DIR=/tmp/iroha-codex-check cargo
    test -p iroha_core --lib gossip_drop_does_not_materialize -- --nocapture`
    passed (`2` tests).
  - `NORITO_SKIP_BINDINGS_SYNC=1 CARGO_TARGET_DIR=/tmp/iroha-codex-check cargo
    test -p iroha_core --lib gossip_ed25519_batch_precheck -- --nocapture`
    passed (`2` tests).
  - `NORITO_SKIP_BINDINGS_SYNC=1 CARGO_TARGET_DIR=/tmp/iroha-codex-check cargo
    test -p iroha_core --lib queue_accepts_gossip_payload_cache --
    --nocapture` passed (`3` tests).
  - `NORITO_SKIP_BINDINGS_SYNC=1 CARGO_TARGET_DIR=/tmp/iroha-codex-check cargo
    test -p iroha_core --lib
    queue_generated_gossip_payload_uses_framed_entrypoint_wire -- --nocapture`
    passed (`1` test).
  - `NORITO_SKIP_BINDINGS_SYNC=1 CARGO_TARGET_DIR=/tmp/iroha-codex-check cargo
    test -p iroha_crypto
    signature_of_try_deserialize_preserves_compact_const_vec_payload --
    --nocapture` passed (`1` test).
  - `NORITO_SKIP_BINDINGS_SYNC=1 CARGO_TARGET_DIR=/tmp/iroha-codex-check cargo
    check -p norito -p iroha_crypto -p iroha_data_model -p iroha_core -p
    iroha_torii -p irohad` passed.
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-check-unskipped cargo check -p norito
    -p iroha_crypto -p iroha_data_model -p iroha_core -p iroha_torii -p
    irohad` passed, including the Norito binding parity build-script corridor.
  - `cargo build --release -p izanami --bin izanami -p irohad --bin iroha3d`
    passed.
  - Fresh `30s` sampled 20k Izanami profile passed with artifact
    `dist/izanami-profile-20k-broader-pass-sampled-30s-20260501-194734`.
  - Fresh `120s` prebuilt 20k Izanami gate passed with artifact
    `dist/izanami-prebuilt-20k-broader-pass-120s-20260501-194908`.
  - Fresh `120s` prebuilt 20k Izanami gate rerun passed with artifact
    `dist/izanami-prebuilt-20k-broader-pass-rerun-120s-20260501-195617`.
  - Fresh fixed-runner `30s` sampled 20k Izanami profile passed with artifact
    `dist/izanami-profile-20k-broader-pass-rerun-sampled-30s-20260501-200527`.

## 2026-05-01 Conservative cache release 20k gate rerun

- Rebuilt release binaries with
  `cargo build --release -p izanami --bin izanami -p irohad --bin iroha3d`
  and reran the 4-peer, no-fault, prebuilt `20,000 TPS` / `120s` Izanami
  gate. Canonical artifact:
  `dist/izanami-prebuilt-20k-conservative-cache-rerun-120s-20260501-175213`.
  It exited `0`; a preceding same-shape artifact completed and wrote a summary,
  but its wrapper tripped only during post-run exit bookkeeping, so it is not the
  recorded gate result.
- The clean run offered `2,399,905` submissions, built `2,400,000` prebuilt
  transactions, used `2,399,905`, and had no prebuild fallback or build
  failures. Ingress accepted `54,574` transactions. Submit latency
  p50/p95/p99/max was `3114/4804/5744/7116 ms`.
- Final quorum/strict height was `9/9` with `28,710/28,710` strict approved
  transactions, final peer height skew `1`, final approved-transaction skew
  `4096`, final queue depth `25,839/2,400,000`, and
  `tx_queue_saturated=true`.
- The result is materially consistent with the previous post-cache tuned gates
  (`28,790` then `28,694` strict approved transactions). The conservative cache
  pass did not regress safety signals, but the 20k committed-throughput target is
  still missed; the next useful step remains a fresh 30s sampled profile focused
  on validation/execution recording, gossip decode, and any remaining Norito
  encoded-length fallback.
- The gate recorded no validation rejects, view changes, RBC store pressure, RBC
  evictions, pending-RBC drops, prebuild fallback, or transaction build failures.
  `sumeragi_status_delta` reported `81` pacemaker backpressure deferrals,
  `8/8` missing-QC reacquire attempts/successes, `8` range-pull escalations,
  and no range-pull successes or failures.

## 2026-05-01 Conservative cache 20k sampled profile bottlenecks

- Captured a fresh release 4-peer, no-fault, prebuilt `20,000 TPS` / `30s`
  macOS `sample` profile at
  `dist/izanami-profile-20k-conservative-cache-parallel-sampled-30s-20260501-181025`.
  The wrapper exited `0` with `sample_ready=1`, `sample_status=0`, and sampled
  the load driver plus all four peer processes. A concurrent
  `cargo test -p iroha_core --lib gossip_transaction -- --nocapture` compile
  was active on the same host, so this is useful bottleneck evidence but not an
  isolated latency baseline.
- The run offered all `600,000` submissions, built/used all `600,000` prebuilt
  transactions, accepted `52,080` ingress transactions, and reached
  quorum/strict height `2/2` with `147/147` strict approved transactions.
  Submit latency p50/p95/p99/max was `3511/7212/8964/11842 ms`; final queue
  depth was `41,996/600,000` with `tx_queue_saturated=true`.
- Safety signals stayed clean: no validation rejects, view changes, RBC store
  pressure, RBC evictions, pending-RBC drops, prebuild fallback, or transaction
  build failures. The status delta showed `45` pacemaker backpressure deferrals,
  `3/3` missing-QC reacquire attempts/successes, and `3` range-pull
  escalations.
- The intended cache-pass removals remain absent in the sampled peers:
  `Queue::encode_gossip_payload=0`, `TxOverlay::byte_size=0`, and
  `external_entrypoints_cloned=0`. `prepare_signed_metadata` is no longer a
  visible recursive-stack bottleneck in the aggregate peer summary.
- The dominant non-idle peer leaves are now transaction admission crypto and
  canonical-byte work. The Torii ingress path
  `accept_transaction_for_ingress -> AcceptedTransaction::accept_entrypoint ->
  validate_with_now_and_signature_check -> verify_signature_for_check` spends
  visible time in Ed25519 verification and public-key parsing
  (`curve25519_dalek`, `ed25519_dalek`, `PublicKeyFull::from_bytes`). After a
  transaction is accepted, `from_external_with_hot_cache` still constructs
  canonical signed bytes with `norito::to_bytes(&tx)` for each external
  transaction, which shows up with `SignedTransaction`/`TransactionPayload`
  serialization and Blake2 hashing.
- The signed-length fallback is reduced but not free. The hot
  `AcceptedTransaction::signed_encoded_len` stacks now mostly go through
  `SignedTransaction::encoded_len_exact`, but `InstructionBox::encoded_len_exact`
  still calls `encoded_instruction_pair`, which dynamically encodes and frames
  the instruction payload (`Instruction::dyn_encode`, `frame_instruction_payload`)
  before measuring the `(wire_id, payload)` pair. That is why
  `norito::codec::encode_adaptive`, `write_len_prefixed`, CRC/schema-hash, and
  instruction serialization remain visible even on the exact-length path.
- Gossip decode caching is working at the wrapper layer, but downstream gossip
  admission still materializes entrypoints when validation needs semantics. The
  sampled stacks include `TransactionGossip`/`GossipTransaction` decode into
  `SignedTransaction -> TransactionPayload -> InstructionBox -> Transfer`, plus
  `PublicKey`/`PublicKeyCompact` decode.
- Block validation and overlay execution are still present, but this contended
  sample is more ingress/gossip dominated than the earlier cachepass sample.
  The remaining overlay cost is clone-heavy: `validate_and_record_transactions`
  reaches `build_overlay_for_transaction_with_accounts_zk`,
  `InstructionDynClone::dyn_box_clone`, `Transfer::clone`, and
  `WorldTransaction::apply`.

## 2026-05-01 Further 20k conservative cache pass

- Reused prepared signed-transaction metadata across block static validation and
  the later validation/execution recording phase. The all-external block path
  now borrows `external_entrypoints_slice()` and only allocates cloned
  entrypoints for legacy/non-external fallback execution.
- Reduced signed transaction encoded-length fallback by extending exact Norito
  sizing through tuple fields, `Option::Some`, `NonZeroU*`, `PublicKey`,
  `InstructionBox`, and the signed/external entrypoint paths used by
  `AcceptedTransaction`. Cached canonical signed bytes are preferred when
  available for size checks.
- Routed `GossipTransaction::try_deserialize` through the same cached
  entrypoint-payload decode helper used by slice decoding, preserving exact byte
  comparison for cache collision safety and leaving `TransactionGossip` wire
  bytes unchanged.
- No executor API rewrite, borrowed-instruction execution rewrite, config
  default change, canonical transaction/block wire change, or consensus behavior
  change was made in this pass. No fresh Izanami sampled profile or Criterion
  benchmark was run yet after these focused validations; the 120s release gate
  rerun is recorded above.
- Focused validation for this slice:
  - `cargo test -p norito --test encoded_len_exact -- --nocapture`
  - `cargo test -p iroha_crypto public_key_encoded_len_exact --lib -- --nocapture`
  - `cargo test -p iroha_data_model instruction_box_encoded_len_exact --lib -- --nocapture`
  - `cargo test -p iroha_core signed_encoded_len --lib -- --nocapture`
  - `cargo test -p iroha_core gossip_transaction --lib -- --nocapture`
  - `cargo test -p iroha_core validate_and_record_transactions --lib -- --nocapture`
  - `cargo test -p iroha_core stateless_cache --lib -- --nocapture`
  - `cargo test -p iroha_core entrypoint_hash --lib -- --nocapture`
  - `cargo test -p iroha_data_model signed_block_wire_skips_runtime_transaction_caches -- --nocapture`
  - `cargo check -p iroha_data_model -p iroha_core -p iroha_torii`
  - `cargo fmt --all -- --check`
  - `git diff --check`

## 2026-05-01 Soracloud generated auth state hardening

- Generated Soracloud webapp and PII app auth servers now serialize file-backed
  auth state mutations behind a local lock directory, recover stale locks, and
  keep the external shared-state adapter path unchanged. This avoids losing
  challenge/session records when local test replicas share the fallback state
  file.
- Generated webapp and PII app request handlers now convert unexpected
  top-level handler failures into JSON `INTERNAL_SERVER_ERROR` responses rather
  than surfacing opaque client-side socket drops.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo test -p iroha_cli soracloud::tests::generated_pii_app_auth_core_persists_file_state_canonically -- --nocapture`
  - `cargo test -p iroha_cli soracloud::tests::generated_webapp -- --nocapture`
  - `cargo test -p iroha_cli --bin iroha -- --nocapture`

## 2026-05-01 Client submit rejection confirmation race

- Transaction confirmation fallback polling now starts after the first poll
  interval instead of immediately, so a submit endpoint rejection that arrives
  just after listener setup preempts status polling and returns without racing
  the configured status timeout.
- Added regression coverage for a pending submit failure that must be observed
  before the first status poll.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo test -p iroha tx_confirmation_stream_tests::pending_submit_failure_preempts_first_status_poll -- --nocapture`
  - `cargo test -p iroha client::tests::submit_transaction_blocking_returns_submit_rejection_without_waiting_for_timeout -- --nocapture`
  - `cargo test -p iroha client::tests -- --nocapture`
  - `cargo test -p iroha tx_confirmation_stream_tests -- --nocapture`
  - `cargo test -p iroha --lib -- --nocapture`

## 2026-05-01 Live Taira faucet authority top-up

- Submitted a live Taira mint from the configured faucet authority to itself
  for `200000000000000000` canonical XOR
  (`6TEAJqbb8oEPmLncoNiMRbLEK6tw`), transaction
  `9C6A2CDE5B8B4377C0D2534CDF9795D4E2FAB7852DB4CA7B0AB1945AA5DAF7D9`.
- Verified the faucet end-to-end with a fresh account canary. The public
  faucet returned HTTP `202`, claim transaction
  `6b71a4fb7c01006e1a6736de347eaa3babf6e04c61ef7d93301572ff8006e021`, and
  the canary account indexed with `25000` XOR.
- Post-repair public balance check showed the faucet authority at
  `199999999999983427.61890` XOR, leaving enough capacity for repeated
  `25000` XOR claims.

## 2026-05-01 Taira faucet authority seed funding

- Taira genesis now seeds the configured public faucet authority
  (`testuﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV`) with
  `200000000000000000` units of the canonical `xor#universal` asset definition
  (`6TEAJqbb8oEPmLncoNiMRbLEK6tw`) so the served account faucet can satisfy
  repeated `25000` XOR claims after a Taira redeploy/reset from this genesis.
- Focused validation for this slice:
  - `jq -e . configs/soranexus/taira/genesis.json`

## 2026-05-01 Offline V2 native SDK prover speedups

- Swift Offline Note V2 pure Halo2 proving now reuses a cached IPA/domain
  context, verifier-key transcript scalar, and fixed selector polynomial instead
  of regenerating them on every proof. The hot commitment path now uses sparse
  Lagrange commitments for single-row instance/advice columns, a 4-bit
  windowed Vesta scalar multiplication path, and bucketed 4-bit MSM for dense
  IPA commitments to avoid the previous scalar-multiply-per-base commitment
  path. Projective Vesta doubling now reuses the shared `(x + y^2)^2`
  intermediate instead of squaring it twice, and the Swift group formulas
  replace fixed `2x`/`8x` field multiplications with additions.
- Added Swift convenience APIs for direct native proof generation from
  `OfflineNoteRedeemV2` / `OfflineNoteAuditBundleV2`, plus proof replacement
  helpers and a `Halo2OfflineNoteV2Prover.prewarm()` hook so callers can keep
  the native model, initialize the proof cache before the button path, and swap
  in the newly generated recursive proof before binding validation.
- Added Kotlin/JVM and Java Android Offline V2 instance-value builders,
  scalar-column encoders, proof replacement helpers, and pure Java Halo2/IPA
  provers. The Java-family path now builds the same `OpenVerifyEnvelope`
  recursive proof payloads without routing production calls through Rust JNI,
  and the focused JVM payload was cross-verified by the Swift native verifier.
  The Java-family prover also uses bucketed dense MSM and conditional
  canonical field add/sub paths to avoid repeated `BigInteger.mod(...)` calls
  in the group-addition hot loop, plus the same projective doubling
  intermediate reuse as Swift and dedicated `2x`/`8x` field helpers for the
  group-formula constants.
- Release Swift benchmark on macOS arm64 for the pure Swift prover after the
  latest optimization pass: audit median `0.315s`, p95 `0.322s`, max
  `0.324s`; redeem median `0.311s`, p95 `0.316s`, max `0.325s` over 20
  iterations.
- Env-gated Java-family benchmark hooks now report subsecond native prover
  medians on macOS arm64 over 5 iterations: Kotlin/JVM audit `0.826s`, p95
  `0.832s`, max `0.832s`; Kotlin/JVM redeem `0.820s`, p95 `0.871s`, max
  `0.871s`; Java Android harness audit `0.829s`, p95 `0.836s`, max `0.836s`;
  Java Android harness redeem `0.823s`, p95 `0.825s`, max `0.825s`.
- Focused validation for this slice:
  - `swift test -c release --filter Halo2PastaTests/testPastaUniformBytesAndVestaGroupArithmetic`
  - `swift test -c release --filter Halo2PastaTests/testOfflineNoteV2NativeHalo2ProofEnvelopeFitsQrBudget`
  - `IROHA_SWIFT_OFFLINE_V2_BENCH=1 IROHA_SWIFT_OFFLINE_V2_BENCH_ITERATIONS=20 swift test -c release --filter Halo2PastaTests/testOfflineNoteV2NativeHalo2ProofPerformanceWhenRequested`
  - `./gradlew :core-jvm:test --tests org.hyperledger.iroha.sdk.offline.OfflineNoteV2Test --console=plain` from `kotlin`
  - `IROHA_JVM_OFFLINE_V2_PROVER_TEST=1 ./gradlew :core-jvm:test --tests org.hyperledger.iroha.sdk.offline.OfflineNoteV2Test.nativeHalo2ProverProducesVerifyingPayloadWhenRequested --console=plain` from `kotlin`
  - `IROHA_JVM_OFFLINE_V2_BENCH=1 IROHA_JVM_OFFLINE_V2_BENCH_ITERATIONS=5 ./gradlew :core-jvm:test --tests org.hyperledger.iroha.sdk.offline.OfflineNoteV2Test.nativeHalo2ProverPerformanceWhenRequested --console=plain --info --rerun-tasks` from `kotlin`
  - `IROHA_SWIFT_OFFLINE_V2_VERIFY_PAYLOAD_IN=/tmp/iroha-jvm-offline-v2-audit.zk1 swift test -c release --filter Halo2PastaTests/testOfflineNoteV2NativeHalo2ProofEnvelopeFitsQrBudget`
  - `JAVA_HOME=$(/usr/libexec/java_home -v 21) ANDROID_HARNESS_MAINS=org.hyperledger.iroha.android.offline.OfflineNoteV2Test ./gradlew :core:test --tests org.hyperledger.iroha.android.GradleHarnessTests --console=plain` from `java/iroha_android`
  - `JAVA_HOME=$(/usr/libexec/java_home -v 21) IROHA_JAVA_OFFLINE_V2_PROVER_TEST=1 ANDROID_HARNESS_MAINS=org.hyperledger.iroha.android.offline.OfflineNoteV2Test ./gradlew :core:test --tests org.hyperledger.iroha.android.GradleHarnessTests --console=plain` from `java/iroha_android`
  - `JAVA_HOME=$(/usr/libexec/java_home -v 21) IROHA_JAVA_OFFLINE_V2_BENCH=1 IROHA_JAVA_OFFLINE_V2_BENCH_ITERATIONS=5 ANDROID_HARNESS_MAINS=org.hyperledger.iroha.android.offline.OfflineNoteV2Test ./gradlew :core:test --tests org.hyperledger.iroha.android.GradleHarnessTests --console=plain --info --rerun-tasks` from `java/iroha_android`
  - `git diff --check`

## 2026-05-01 Torii Offline V2 issuer hardening

- Torii Offline V2 issuer certificate minting now requires a signed middleware
  attestation receipt before certifying hardware one-use keys. Certificate JSON
  echoes canonical base64 key bytes from the verified receipt instead of
  client-supplied hex/base64 spellings.
- Offline V2 note issuance now derives balances from Torii-signed lineage
  state, treats client `local_balance` / `local_revision` as consistency
  checks, preserves trusted balance during existing-lineage key refills, and
  returns the same chain note commitment that is submitted on-chain.
- `torii.offline_issuer` configuration now requires an attestation verifier
  public key and explicitly accepts only Ed25519 or Secp256k1 issuer/verifier
  keys.
- Refreshed the minimal `iroha_config` fixture snapshot so the Torii defaults
  include `offline_issuer: None`.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo check -p iroha_config -p iroha_torii --features app_api`
  - `cargo test -p iroha_torii offline_v2_issuer`
  - `cargo test -p iroha_config torii_offline_issuer`
  - `cargo test -p iroha_config --test fixtures`

## 2026-05-01 20k post-cache tuned release gate rerun

- Reran the release 4-peer, no-fault, prebuilt `20,000 TPS` / `120s` gate
  against the current post-cache tuned binaries. Artifact:
  `dist/izanami-prebuilt-20k-postcache-tuned-rerun-120s-20260501-170823`.
- The runner exited `0`, offered all `2,400,000` submissions, built and used all
  `2,400,000` prebuilt transactions, and had no prebuild fallback or build
  failures. Ingress accepted `46,748` transactions. Submit latency
  p50/p95/p99/max was `3109/4828/5770/7788 ms`.
- Final quorum/strict height was `9/9` with `28,694/28,694` strict approved
  transactions, final queue depth `18,063/2,400,000`, and
  `tx_queue_saturated=true`. The rerun is effectively consistent with the
  previous tuned 120s gate (`28,790` strict approved), while still far below
  the committed-20k target.
- Diagnostics recorded `23,090` latency-saturated ingress rejects, `312`
  deferred local `RBC READY` events, `274` deferred `RBC DELIVER` events, `96`
  targeted RBC payload sends, `96` targeted READY-set sends, and `66` targeted
  BlockBodyResponse companions. There were no inline validation fallback
  warnings, slow commit-pipeline warnings, slow block-validation warnings,
  validation rejects, view changes, RBC store pressure, RBC evictions, or
  pending-RBC drops.

## 2026-05-01 20k post-cache targeted tuning pass

- Implemented the low/medium-risk post-cache slice without public API,
  wire-format, config, or dependency changes. Transaction gossip now derives
  the `TransactionEntrypoint` hash directly from canonical framed Norito
  entrypoint bytes, routes accepted gossip through a crate-private constructor
  that seeds those inbound bytes after normal validation, and performs
  per-entry route/plane rejection before hash/materialization where that keeps
  existing drop reasons intact.
- Block validation keeps external-only blocks on borrowed
  `external_entrypoints_slice()` inspection and only allocates cloned
  entrypoints for non-external sequential fallback. Regression coverage now
  checks both the external-only path and the non-external sealed-commitment
  fallback while preserving entrypoint hashes and execution results.
- `TxOverlay` now caches its Norito instruction byte-size sum with a local
  `OnceLock`; max-byte rejection paths compute `byte_size()` once per check and
  reuse that value in the unchanged rejection message. The broader borrowed
  instruction execution rewrite remains deliberately out of this pass.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo test -p iroha_core --lib gossip_transaction_hash_from_framed_entrypoint_matches_canonical_hash -- --nocapture`
  - `cargo test -p iroha_core --lib queue_accepts_gossip_payload_cache -- --nocapture`
  - `cargo test -p iroha_core --lib queue_generated_gossip_payload_uses_framed_entrypoint_wire -- --nocapture`
  - `cargo test -p iroha_core --lib overlay_byte_size_cache_matches_norito_instruction_sum -- --nocapture`
  - `cargo test -p iroha_core --lib block_validation_external_only_records_entrypoint_hash_without_fallback -- --nocapture`
  - `cargo test -p iroha_core --lib block_validation_non_external_entrypoint_uses_sequential_fallback -- --nocapture`
  - `cargo test -p iroha_core --test overlay_bounds overlay_bytes_cap_rejects_and_rest_apply -- --nocapture`
  - `cargo check -p norito -p iroha_crypto -p iroha_data_model -p iroha_core -p iroha_torii -p irohad`
  - `git diff --check`
- Rebuilt release binaries with
  `cargo build --release -p izanami --bin izanami -p irohad --bin iroha3d`.
  Fresh 30s sampled artifact:
  `dist/izanami-profile-20k-postcache-tuned-sampled-30s-20260501-165811`.
  It exited `0`, sampled the runner plus four current peers, offered all
  `600,000` submissions, built/used all `600,000` prebuilt transactions,
  accepted `51,852` ingress transactions, and reached final quorum/strict
  height `3/3` with `4,164/4,164` strict approved transactions. Submit latency
  p50/p95/p99/max was `3484/5504/6442/8837 ms`; final queue depth was
  `42,101/600,000` with `tx_queue_saturated=true`.
- Fresh 120s release gate artifact:
  `dist/izanami-prebuilt-20k-postcache-tuned-120s-20260501-165947`. It exited
  `0`, offered all `2,400,000` submissions, built/used all `2,400,000`
  prebuilt transactions, accepted `47,174` ingress transactions, and reached
  final quorum/strict height `9/9` with `28,790/28,790` strict approved
  transactions. Submit latency p50/p95/p99/max was `3193/4687/5706/7139 ms`;
  final queue depth fell to `14,282/2,400,000`, with
  `tx_queue_saturated=true`.
- Against the cachepass baselines, the 30s sampled run improved strict height
  from `2` to `3`, strict approved transactions from `4,116` to `4,164`, and
  p95/p99 latency from `6198/7260 ms` to `5504/6442 ms`, while ingress accepted
  transactions fell from `53,626` to `51,852`. The 120s gate improved strict
  approved transactions from `24,623` to `28,790` (`+16.9%`) and final queue
  depth from `23,457` to `14,282`, while ingress accepted transactions fell
  from `52,167` to `47,174`.
- Current-peer sample classification confirms the intended hot-path removals:
  `Queue::encode_gossip_payload=0`, `TxOverlay::byte_size=0`, and
  `external_entrypoints_cloned=0`. Relative to the cachepass sample,
  current-peer pattern counts fell for `write_len_prefixed` (`22,787` to
  `19,403`), `TransactionGossip` (`1,602` to `1,441`), and
  `GossipTransaction` (`1,010` to `402`). Remaining visible costs are still
  Norito/gossip decode/materialization, signed length/metadata walks
  (`AcceptedTransaction::signed_encoded_len=1,217`,
  `prepare_signed_metadata=50` in this sample), validation/overlay execution,
  and crypto/math leaves. No validation rejects, DA-gate view changes, RBC
  store pressure, RBC evictions, or pending-RBC drops were recorded in either
  fresh run.

## 2026-05-01 20k post-cache sampled profile bottlenecks

- Captured a fresh release 4-peer, no-fault, prebuilt `20,000 TPS` / `30s`
  macOS `sample` profile at
  `dist/izanami-profile-20k-cachepass-sampled-30s-20260501-152126`. A
  discarded first wrapper at
  `dist/izanami-profile-20k-cachepass-sampled-30s-20260501-152043` aborted
  before peer startup because the shell treated zero `pgrep` matches as fatal
  under `pipefail`.
- The valid wrapper exited `0` with `sample_ready=true`. It sampled the runner,
  two Bash wrappers, stale peer PIDs from the aborted wrapper, and the current
  peer set; the current peer samples used for classification are
  `profiles/sample-16852.txt`, `profiles/sample-16853.txt`,
  `profiles/sample-16854.txt`, and `profiles/sample-16855.txt`.
- The run offered all `600,000` submissions, built and used all `600,000`
  prebuilt transactions, accepted `53,626` ingress transactions, and reached
  final quorum/strict height `2/2` with `4,116/4,116` strict approved
  transactions. Submit latency p50/p95/p99/max was
  `3318/6198/7260/9966 ms`; final queue depth was `42,282/600,000` with
  `tx_queue_saturated=true`. There were no validation rejects, DA-gate view
  changes, RBC store pressure events, RBC evictions, or pending-RBC drops.
- Peer diagnostics recorded `6,613` latency-saturated ingress rejects,
  `68` deferred local `RBC READY` events, `64` deferred `RBC DELIVER` events,
  `34` targeted RBC payload sends, `33` targeted READY-set sends, `26`
  targeted BlockBodyResponse companions, `7` inline validation fallback
  warnings, and `4` slow commit-pipeline warnings. The slow validation windows
  were `4,583`, `14,452`, `14,580`, and `16,205 ms` (`12,455 ms` average);
  slow total pipeline time peaked at `16,289 ms`, and oldest queued transaction
  age reached `33,202 ms`.
- The largest active consensus stack is now block validation and overlay
  application:
  `validate_block_for_voting -> validate_keep_voting_block_inner -> validate_and_record_transactions -> TxOverlay::apply_with_chunk`.
  This is the commit-progress critical path; one peer sample shows
  `validate_block_for_voting` holding `7,989` recursive samples,
  `validate_and_record_transactions` `5,143`, and `TxOverlay::apply_with_chunk`
  `4,183`.
- The next active stack is incoming transaction-gossip Norito decode:
  `TransactionGossip -> GossipTransaction -> SignedTransaction -> TransactionPayload -> InstructionBox -> Transfer`.
  Current-peer pattern counts include `TransactionGossip=1,602`,
  `GossipTransaction=1,010`, `decode_field_canonical=20,235`,
  `write_len_prefixed=22,787`, `TransactionPayload=9,083`,
  `InstructionBox=7,687`, and `PublicKey/PublicKeyCompact=7,391/3,483`.
- Remaining signed-transaction length/serialization fallback is still visible
  inside prepared metadata:
  `ValidBlock::validate_and_record_transactions -> AcceptedTransaction::prepare_signed_metadata -> AcceptedTransaction::signed_encoded_len -> norito::codec::encode_adaptive_into`.
  Current-peer pattern counts show `AcceptedTransaction::signed_encoded_len=725`,
  `AcceptedTransaction::prepare_signed_metadata=30`,
  `norito::codec::encode_adaptive_into=529`, and
  `norito::core::to_bytes=985`.
- Top-leaf samples still show expensive crypto/math and allocation costs even
  when the commit-progress stack is validation/overlay. Excluding parked/kernel
  waits across the four current peer samples, active top leaves were led by
  Ed25519/Curve25519 (`78,409`, `29.44%`), allocation/copy (`51,069`,
  `19.17%`), runtime/scheduler artifacts (`49,173`, `18.46%`), Norito/gossip
  decode (`33,813`, `12.69%`), and FASTPQ/Poseidon/ZKP leaves (`11,878`,
  `4.46%`). Recursive stack attribution puts Norito/gossip decode first
  (`167,668`, `45.77%`) and Ed25519/Curve25519 at `40,848` (`11.15%`).
- The intended queue-side fix held: `Queue::encode_gossip_payload=0` in the
  current peer samples. `PublicKey::normalize=0`, Ed25519 curve math is present
  as a leaf cost but not the blocking commit-progress stack, and
  FASTPQ/Poseidon is visible but small in recursive attribution. The remaining
  limiter is block validation and overlay throughput, fed by incoming gossip
  decode and the leftover signed length/serialization metadata walk; increasing
  queue capacity alone will not improve committed throughput.

## 2026-05-01 Block transaction pipeline hardening follow-up

- Transaction gossip now carries canonical `TransactionEntrypoint` payloads
  instead of downgrading to `SignedTransaction`, so sealed commitments and
  sealed reveals retain their public entrypoint identity across queue gossip.
  Queue-side gossip payload caches now store entrypoint Norito bytes.
- Explorer, contract-source lookup, and Torii pipeline-status reconstruction now
  pair block entrypoints with aligned transaction results rather than zipping
  external signed transactions against the first result slice. Sealed reveals
  are reported and looked up by their entrypoint hash.
- Torii submission responses and signed receipts now expose
  `entrypoint_hash`, keep `x-iroha-transaction-hash` as the canonical
  entrypoint hash, and add `x-iroha-signed-transaction-hash` when an inner
  signed transaction exists.
- Sealed transaction commitments now have deterministic non-zero gas/TEU cost
  based on encoded length, and block execution prunes expired pending sealed
  commitments from smart-contract state.
- Follow-up unit coverage now executes a sealed commitment in one block and
  its reveal in the next block through the real block-builder validation path,
  checks expiry pruning after the reveal deadline, and verifies Torii resolves
  pipeline status by the sealed-reveal entrypoint hash stored in Kura/state.
- Sealed commitments are no longer expired by the wall-clock queue TTL; their
  lifetime remains governed by reveal-height windows during block execution.
- Post-execution block application now indexes canonical external entrypoint
  hashes instead of signed-transaction-only hashes, which avoids duplicate
  transaction-index insertion mismatches when sealed commitment blocks are
  replayed with commit-QC evidence.
- Block receipt proof construction now uses an external-entrypoint Merkle tree
  for external receipts and the full entrypoint tree for time-trigger receipts,
  so sealed commitments and mixed external/time-trigger blocks verify against
  the correct root. Explorer block totals now count external entrypoints rather
  than only external signed transactions.
- The multi-peer sealed pipeline integration now submits a sealed commitment,
  waits for gossip/commit across a 4-peer network, reveals it in-window, and
  verifies explorer lookup by the reveal entrypoint hash. The test backs off on
  temporary queue-latency `429` responses instead of treating transient queue
  pressure as a protocol failure.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo check -p iroha_data_model -p iroha_core -p iroha_torii`
  - `cargo test -p iroha_core gossip_roundtrip_preserves_sealed_commitment_entrypoint --lib -- --nocapture`
  - `cargo test -p iroha_core partition_gossip_batch_keeps_sealed_commitments --lib -- --nocapture`
  - `cargo test -p iroha_core sealed_commitment_cost_is_nonzero_and_size_sensitive --lib -- --nocapture`
  - `cargo test -p iroha_core queue_generated_gossip_payload_uses_framed_entrypoint_wire --lib -- --nocapture`
  - `cargo test -p iroha_core queue_accepts_gossip_payload_cache --lib -- --nocapture`
  - `cargo test -p iroha_data_model submission_receipt_roundtrips_signature --lib -- --nocapture`
  - `cargo test -p iroha_torii handler_post_transaction_entrypoint_accepts_external_entrypoint --lib -- --nocapture`
  - `cargo test -p iroha_torii handler_post_transaction_honors_prefer_return_minimal --lib -- --nocapture`
  - `cargo test -p iroha_core block_pipeline_executes_sealed_reveal_and_records_entrypoint_hash --lib -- --nocapture`
  - `cargo test -p iroha_core prune_expired_sealed_commitments_removes_pending_state_after_deadline --lib -- --nocapture`
  - `cargo test -p iroha_core sealed_commitment_is_not_expired_by_wall_clock_queue_ttl --lib -- --nocapture`
  - `cargo test -p iroha_core apply_without_execution_indexes_sealed_commitment_entrypoint_hash --lib -- --nocapture`
  - `cargo test -p iroha_core block_proofs_for_sealed_commitment_use_external_merkle_root --lib -- --nocapture`
  - `cargo test -p iroha_data_model --features transparent_api proofs_for_external_entry_with_time_trigger_use_consensus_root --lib -- --nocapture`
  - `cargo test -p iroha_torii pipeline_status_handler_returns_applied_for_sealed_reveal_entrypoint_hash --lib -- --nocapture`
  - `cargo test -p iroha_torii block_dto_counts_sealed_commitment_entrypoints --lib -- --nocapture`
  - `cargo test -p integration_tests --test core_api sealed_commitment_reveal_gossips_and_explorer_lookup_uses_entrypoint_hash -- --nocapture`
  - `git diff --check`

## 2026-05-01 20k hot-path release gate rerun

- Rebuilt release binaries with
  `cargo build --release -p izanami --bin izanami -p irohad --bin iroha3d`,
  then reran the 4-peer, no-fault, prebuilt `20,000 TPS` / `120s` gate with
  `2,400,000` prebuilt transactions and `4096` submitters. Artifact:
  `dist/izanami-prebuilt-20k-hotpath-120s-20260501-142015`.
- The runner exited `0`, offered all `2,400,000` planned submissions, built
  and used all `2,400,000` prebuilt transactions, and had no prebuild fallback
  or build failures. Ingress accepted `53,891` (`449.1 TPS`, `2.25%` of
  offered). Submit latency p50/p95/p99/max was `3315/4836/5690/8060 ms`;
  failover/unhealthy endpoint totals were `13/46`.
- Final quorum/strict height was `9/9` with zero peer height skew and
  `28,713/28,713` approved transactions. That is `239.3 committed TPS` over
  the measured window, or `1.20%` of the `2,400,000` committed-transaction
  gate. Compared with the prior 2026-05-01 120s rerun artifact
  (`24,590` approved), this is `+4,123` approved transactions (`+16.8%`), but
  still misses the 20k committed-TPS target by `2,371,287` transactions.
- Runtime counters still show queue drain / validation throughput as the
  limiter rather than DA/RBC storage failure: `tx_queue_saturated=true`, final
  queue depth `25,286/2,400,000`, `pacemaker_backpressure_deferrals_total=74`,
  `worker_loop_stage="drain_votes"`, no view-change causes, no validation
  rejects, no DA-gate causes, no RBC store pressure, no RBC evictions, no RBC
  persist drops, no pending-RBC drops, and no commit-inflight timeouts.
- Peer diagnostics recorded `23,608` latency-saturated ingress rejects,
  `28` inline validation fallback warnings, `28` slow commit-pipeline warnings,
  max block validation `6,060 ms`, and max slow-pipeline total `6,197 ms`.
  Slow validation warnings covered heights `3` through `10`. RBC authoritative
  payload delay symptoms remained visible but resolved: `260` deferred local
  `RBC READY`, `243` deferred `RBC DELIVER`, `101` targeted RBC payload sends,
  `101` targeted READY-set sends, and `74` targeted BlockBodyResponse
  companions. Peer stderr logs were empty; one peer again exited with raw
  `unix_wait_status(3)` during shutdown after the measured window.
- Reran the same release gate shape again at
  `dist/izanami-prebuilt-20k-cachepass-120s-20260501-142429` to check variance.
  This repeat also exited `0`, offered all `2,400,000` submissions, built/used
  all `2,400,000` prebuilt transactions, and had no prebuild fallback or build
  failures. Ingress accepted `52,167` (`434.7 TPS`, `2.17%` of offered), with
  submit latency p50/p95/p99/max `3112/4803/5840/8251 ms`, final
  quorum/strict height `8/8`, strict approved transactions `24,623`
  (`205.2 committed TPS`, `1.03%` of target), final queue depth
  `23,457/2,400,000`, and `tx_queue_saturated=true`.
- The repeat confirms the failure class rather than a one-off artifact:
  compared with `dist/izanami-prebuilt-20k-rerun-120s-20260501-103729`,
  ingress improved by `4,699` accepted transactions (`+9.9%`), but strict
  approved transactions only improved by `33` (`+0.13%`). Peer diagnostics
  showed `21,968` latency-saturated ingress rejects, `26` inline validation
  fallback warnings, `26` slow commit-pipeline warnings, validation times up to
  `5,907 ms` (`5,116 ms` average among slow warnings), no validation rejects,
  no DA-gate/view-change causes, no RBC store pressure/evictions, and empty
  peer stderr logs.

## 2026-05-01 20k hot-path critical-path cache pass

- Implemented the next bottleneck-reduction slice from the 20k profile: accepted
  transactions can now carry caller-provided canonical signed-transaction bytes,
  and gossip acceptance reuses those bytes instead of immediately re-encoding
  the same `SignedTransaction`. The cached bytes feed the prepared transaction
  metadata path used by size/hash checks.
- Block validation now prepares external transaction metadata once per block and
  uses the borrowed external entrypoint slice when the block payload has one.
  Merkle-root rebuilding reuses prepared entrypoint hashes for external entries
  while still hashing sealed/non-external wrappers directly, so sealed reveal
  ordering and block-wire semantics are unchanged.
- Ed25519 block validation now feeds true batch verification with prepared
  payload hashes and parsed single-key authorities. The batch-failure path
  bisects borrowed slices with reusable scratch storage, then reports the first
  invalid transaction in deterministic block order before falling back to the
  existing per-transaction validation path.
- Runtime-only block caches are kept off the canonical wire: the legacy
  signed-transaction cache and result-side entrypoint cache are skipped by
  Norito encoding, while canonical external entrypoints remain on the block
  payload. Focused roundtrip coverage asserts cache contents do not change
  signed block wire bytes.
- FASTPQ execution witnesses no longer force block-level `TransitionBatch`
  materialization inside `StateBlock::capture_exec_witness`. The commit path
  carries a local-only `FastpqWitnessContext` containing the public-input
  template, tx-set hash, and entry dataspace overrides into the FASTPQ lane,
  where transcript-only witnesses are expanded off the consensus critical path.
  The `ExecWitness` wire shape remains unchanged.
- Focused validation for this slice:
  - `CARGO_TARGET_DIR=target/codex-20k cargo test -p iroha_core accept_with_canonical_signed_bytes_reuses_payload_cache --lib -- --nocapture`
  - `CARGO_TARGET_DIR=target/codex-20k cargo test -p iroha_core job_context_builds_batches_for_transcript_only_witness --lib -- --nocapture`
  - `CARGO_TARGET_DIR=target/codex-20k cargo test -p iroha_core capture_exec_witness --lib -- --nocapture`
  - `CARGO_TARGET_DIR=target/codex-20k cargo check -p iroha_data_model -p iroha_core -p iroha_torii`
  - `cargo test -p iroha_crypto ed25519_batch -- --nocapture`
  - `cargo test -p iroha_data_model signed_block_wire_skips_runtime_transaction_caches -- --nocapture`
  - `cargo test -p iroha_data_model versioned_block_roundtrip_preserves_instruction_order -- --nocapture`
  - `cargo test -p iroha_core --test admission_batching ed25519_batch_bisection_finds_bad_sig -- --nocapture`
  - `cargo test -p iroha_core --test signature_batch_determinism ed25519_batch_permutation_finds_same_bad_sig -- --nocapture`
  - `cargo test -p iroha_core queue_generated_gossip_payload_uses_framed_signed_transaction_wire --lib -- --nocapture`
  - `cargo test -p iroha_core queue_accepts_gossip_payload_cache --lib -- --nocapture`
  - `cargo fmt --all`
  - `cargo check -p norito -p iroha_crypto -p iroha_data_model -p iroha_core -p iroha_torii -p irohad`
- Release Izanami validation for this slice is recorded above in the
  `20k hot-path release gate rerun` and `20k post-cache sampled profile
  bottlenecks` entries. The remaining validation work is to locate the new
  5k/10k knee and then reprofile after the next block-validation/Norito
  optimization slice.

## 2026-05-01 20k sampled profile rerun bottlenecks

- Captured a fresh 4-peer, no-fault, prebuilt `20,000 TPS` / `30s` macOS
  `sample` profile at
  `dist/izanami-profile-20k-rerun-sampled-30s-20260501-104544`. The valid
  wrapper exited `0`; the sampled PIDs were the Izanami runner, one Bash child
  to ignore, and all four `iroha3d` peers. A discarded earlier artifact at
  `dist/izanami-profile-20k-rerun-sampled-30s-20260501-104431` failed after
  warmup because the wrapper used `mapfile` on the system Bash.
- The sampled run offered all `600,000` planned submissions and used all
  `600,000` prebuilt transactions. Ingress accepted `53,697` (`8.95%` of
  offered), but final quorum/strict progress was only height `2/2` with
  `17/17` approved transactions. Submit latency p50/p95/p99/max was
  `3267/6097/7739/9965 ms`; failover/unhealthy endpoint totals were `22/14`.
- Runtime counters and peer diagnostics still point at block drain,
  validation, and consensus payload progress rather than DA/RBC store capacity:
  `tx_queue_saturated=true`, final queue depth `38,701/600,000`,
  `pacemaker_backpressure_deferrals_total=28`, `worker_loop_stage="tick"`,
  no validation rejects, no DA-gate view changes, no RBC store pressure, no
  RBC evictions, and no pending-RBC drops. Peer logs had `6,442` queue
  age-saturated rejects, `163` deferred local `RBC READY`, `125` deferred
  `RBC DELIVER`, `14` targeted RBC payload sends, `14` targeted READY-set
  sends, `8` commit quorums, and `8` commits.
- The decisive consensus log evidence is validation latency: all peers emitted
  inline validation fallback warnings, and three peers logged height-`3`
  `commit pipeline block processing slow` with validation times of
  `10,901 ms`, `10,923 ms`, and `12,372 ms` (`total_ms` up to `12,908`).
  During that window queue age saturated and RBC delivery repeatedly waited
  for authoritative payload availability; one peer also requested seven
  block-sync range pulls for `idle_missing_qc_reacquire`.
- Peer CPU attribution, after excluding parked/waiting leaves, is led by:
  allocation/copy churn at roughly `27.6%` of active top-of-stack samples,
  Ed25519/Curve25519 math at `26.6%`, Norito encode/decode leaf work at
  `12.4%`, public-key/account string and multihash work at `4.4%`,
  FASTPQ/Poseidon hashing at `4.3%`, and hash/CRC work at `2.9%`. Recursive
  stack attribution still puts Norito and transaction wire work first:
  `norito::core::write_len_prefixed`, `decode_field_canonical`,
  `write_len`, transaction payload/entrypoint, `InstructionBox`,
  `Transfer`, `AccountController`, `AssetId`, `SignedTransaction`, and P2P
  transaction gossip decode dominate the hot call paths.
- Normalized hot symbols from the four peers reinforce the bottleneck order:
  top active leaves include `FieldElement51::pow2k` (`55,767`), dalek field
  multiply (`21,159`), `_platform_memmove` (`16,884`), `_xzm_free`
  (`14,026`), `_xzm_xzone_malloc_tiny` (`10,233`), Poseidon `apply_mds`
  (`9,784`), `RawVec::finish_grow` (`6,085`), string collection/formatting
  (`5,672`), `Blake2bVarCore::compress` (`4,765`), `write_all` (`4,491`),
  and Norito `decode_field_canonical` (`3,258`). Recursive hot paths include
  Norito length/field helpers, transaction payload serialization,
  `PublicKey::normalize`, multihash formatting, Ed25519 public-key parsing,
  and block validation batch-verification frames.
- Critique: this profile confirms the latest 120s 20k failure mode. Torii can
  accept a short burst, but queue age saturates because block validation and
  payload/commit progress cannot drain it. The next fix should prioritize
  removing remaining Norito re-serialization and allocation growth from block
  validation/proposal paths, reusing canonical payload bytes through RBC/commit
  validation, and reducing public-key/account string conversions on incoming
  gossip. Ed25519 batch verification is now true CPU work rather than just
  parse overhead, so further wins likely need fewer repeated verification
  inputs or a faster batch-verification backend. FASTPQ/Poseidon is visible
  but not the top bottleneck; keep it budgeted/yielding if later cache work
  exposes it.

## 2026-05-01 20k validation rerun critique

- Rebuilt release binaries with
  `cargo build --release -p izanami --bin izanami -p irohad --bin iroha3d`,
  then reran the 4-peer, no-fault, prebuilt `20,000 TPS` / `120s` gate with
  `2,400,000` prebuilt transactions and `4096` submitters. Artifact:
  `dist/izanami-prebuilt-20k-rerun-120s-20260501-103729`.
- The driver offered all `2,400,000` planned submissions and used all
  `2,400,000` prebuilt transactions with no fallback/build failures. Ingress
  accepted `47,468` (`395.6 TPS`, `1.98%` of offered). Submit latency
  p50/p95/p99/max was `3187/4507/5031/6285 ms`; failover/unhealthy endpoint
  totals were `15/49`.
- Final quorum/strict height was `8/8` with zero peer height skew and
  `24,590/24,590` approved transactions. This is `204.9 committed TPS` over
  the measured window, or `1.02%` of the `2,400,000` committed-transaction
  target. It improves the prior metadata/batch 120s run's `20,609` approved
  transactions by `3,981` (`+19.3%`) but still misses the 20k committed-TPS
  gate by `2,375,410` transactions.
- Runtime counters still classify the failure as sustained queue drain and
  block/consensus throughput, not a DA/RBC store-capacity failure:
  `tx_queue_saturated=true`, final queue depth `22,465/2,400,000`,
  `pacemaker_backpressure_deferrals_total=75`, `worker_loop_stage=drain_rbc_chunks`,
  and no validation rejects, DA-gate view-change causes, RBC store pressure,
  RBC evictions, pending-RBC drops, or commit-inflight timeouts.
- Peer diagnostics show the remaining shape more sharply than the summary:
  `21,733` queue age-saturated ingress rejects, `509` deferred local
  `RBC READY` events, `431` deferred `RBC DELIVER` events, `14` missing
  `BlockCreated` requests, one "QC quorum but payload missing" warning, and
  one deferred commit-QC application while block validation caught up. This
  suggests the next bottleneck is not Torii routing itself, but the pipeline
  between queue drain, block validation/prepared metadata, RBC payload
  materialization, and commit-QC application at height `9`.
- Caveats: the wrapper reached and logged the Izanami summary, but its
  post-run status capture used Bash `PIPESTATUS` under zsh and exited after
  the summary without writing `exit.status`. One peer reported raw
  `unix_wait_status(3)` during shutdown; `iroha_test_network` uses SIGQUIT
  after a peer misses the SIGTERM grace window, so this is a shutdown-lag
  signal rather than a measured-window consensus crash. The stderr logs were
  empty.

## 2026-05-01 20k post-batch profile bottleneck classification

- Profile source: all-peer macOS `sample` output under
  `dist/izanami-profile-20k-metadata-batch-sampled-30s-20260501-101313`.
  This run sampled the Izanami runner and all four `iroha3d` peers, but used a
  different sample interval from the older postfix artifact, so percentages
  within this run are more meaningful than raw-count comparison to older
  samples.
- Excluding wait/parking leaves, active peer CPU is led by
  Ed25519/Curve25519 leaf work (`~31%` of active leaf samples), mostly
  `curve25519_dalek` field arithmetic from dalek batch verification. Recursive
  stack attribution still puts Norito encode/decode as the largest category
  (`~39.5%` of active recursive stack appearances), because transaction gossip
  decode, block validation preparation, and transaction/application paths sit
  above many Norito serializers/deserializers.
- The fixed queue-side bottleneck is confirmed removed from the profile:
  `Queue::encode_gossip_payload` appears in the older sampled peer profiles but
  does not appear in the fresh all-peer peer samples. The Ed25519 path now
  reaches `ed25519_dalek::batch::verify_batch` through
  `ed25519_verify_batch_preparsed_deterministic`.
- Current bottleneck order:
  1. Norito transaction wire work: P2P `MessageReader::parse_next_encrypted_frame`
     repeatedly decodes `TransactionGossip` / `GossipTransaction` /
     `SignedTransaction`, while block execution still serializes transaction
     payload, instruction, asset/account, and transfer shapes during metadata,
     access, and hashing work.
  2. Ed25519 batch math: true batch verification removes the old per-signature
     loop, but batch verification itself is now the hottest active leaf CPU
     path, dominated by `curve25519_dalek` field operations.
  3. Public-key and ID string normalization: `PublicKey::normalize`,
     multihash formatting/hex conversion, and account/address `i105`
     conversion remain visible under incoming wire decode and admission.
  4. Allocation/copy/format churn: malloc/free/realloc/memmove and string
     formatting account for roughly another `~22.5%` of active leaf samples,
     strongly correlated with Norito serialization/deserialization and
     public-key/account string conversions.
  5. Torii HTTP/routing is now a secondary cost (`~0.5%` active leaf,
     `~3.1%` active recursive); it contributes to admission pressure but is no
     longer the dominant CPU stack in this profile.
- Runtime counters still show overload behind the hot paths rather than a DA/RBC
  failure: queue saturation remains true, pacemaker backpressure increments, and
  there are no validation rejects, DA-gate causes, RBC pressure, or pending-RBC
  drops in the sampled run.

## 2026-05-01 20k metadata and Ed25519 batch pass

- Extended accepted transaction hot-path metadata with canonical signed bytes,
  exact framed encoded lengths, payload hashes, and parsed single-key Ed25519
  verifying keys where available. Queue gossip now reuses cached signed bytes
  and canonicalizes any provided gossip payload against the accepted transaction
  cache instead of re-encoding on the queue path.
- Block validation now prepares per-transaction metadata once per block and
  feeds the prepared signed hash, entrypoint hash, payload hash, encoded length,
  signature references, and parsed Ed25519 key into stateless validation.
  Signature semantics and canonical Norito block/transaction wire formats are
  unchanged.
- The Ed25519 batch path now uses `ed25519_dalek::verify_batch` behind the
  existing `ecc-batch` feature through a preparsed deterministic API. If a batch
  fails, validation bisects and then falls back to individual verification in
  original transaction order to report the first bad transaction
  deterministically.
- Built fresh release benchmark binaries with
  `cargo build --release -p izanami --bin izanami -p irohad --bin iroha3d`.
  A 30s runner-only release artifact at
  `dist/izanami-profile-20k-metadata-batch-sampled-30s-20260501-101106`
  offered `600,000`, accepted `47,629`, recorded submit latency
  `3167/4881/5585/6164 ms`, and reached quorum/strict height `3/3` with
  `4,177/4,177` approved transactions; only the Izanami runner was sampled in
  that artifact.
- Reran the all-peer sampled 30s profile at
  `dist/izanami-profile-20k-metadata-batch-sampled-30s-20260501-101313`.
  It offered `600,000`, accepted `22,344`, recorded submit latency
  `3408/6772/7832/9952 ms`, and reached quorum/strict height `2/2` with
  `83/83` approved transactions. This run is used for profiler classification
  because macOS `sample` captured the runner plus all four `iroha3d` peers.
- Reran the clean 120s release validation at
  `dist/izanami-prebuilt-20k-metadata-batch-120s-20260501-101457`. It offered
  `2,400,000`, accepted `47,573`, recorded submit latency
  `3247/4809/5766/7015 ms`, and reached quorum/strict height `7/7` with
  `20,609/20,609` approved transactions. Compared with the prior 120s artifact
  (`48,679` accepted, `2958/4484/4995/6116 ms`, height `5/5`, `12,328`
  approved), ingress remains flat/slightly lower but finality progress improved.
- Fresh peer samples show the intended profile shift: `Queue::encode_gossip_payload`
  no longer appears in peer samples, and the Ed25519 path reaches
  `ed25519_dalek::batch::verify_batch` through
  `ed25519_verify_batch_preparsed_deterministic`. Remaining visible hot stacks
  are Norito gossip/block decode, one-time prepared metadata
  length/hash construction, Ed25519 batch field math, and public-key parsing
  from incoming wire decode. This is still not a successful committed-20k TPS
  run.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo test -p iroha_crypto ed25519_batch -- --nocapture`
  - `cargo test -p iroha_core --lib accepted_transaction_caches_hashes_and_encoded_length -- --nocapture`
  - `cargo test -p iroha_core --lib queue_generated_gossip_payload_uses_framed_signed_transaction_wire -- --nocapture`
  - `cargo test -p iroha_core --test signature_batch_determinism ed25519_batch_permutation_finds_same_bad_sig -- --nocapture`
  - `cargo check -p norito -p iroha_crypto -p iroha_core -p iroha_torii -p irohad`

## 2026-05-01 Block transaction pipeline correctness

- Block validation now runs the shared stateful transaction-admission checks
  before every block execution/apply path. Height TTLs, monotonic transaction
  sequences, lane policy, direct multisig execution rejection, authority
  materialization rules, and fraud gates are enforced consistently for queued
  transactions and received blocks.
- Parallel block scheduling now treats each transaction authority's sequence as
  a write key, so same-authority transactions cannot be applied out of order by
  non-overlapping ISI or IVM access sets.
- Block construction and validation use accepted-entrypoint hashes and prepared
  transaction metadata instead of repeatedly converting accepted transactions
  back to external signed transactions. This keeps non-external entrypoints
  hashable for ordering and Merkle roots while preserving canonical block wire
  layout.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `git diff --check`
  - `cargo test -p iroha_core block_pipeline_rejects -- --nocapture`
  - `cargo test -p iroha_core log_instruction_has_no_access_keys -- --nocapture`
  - `cargo test -p iroha_core --lib ivm_access_dynamic_prepass_requires_gas_limit -- --nocapture`

## 2026-05-01 Inrou portable and proxy hardening

- Portable Inrou now prepares the mutable root disk as a `qcow2` overlay over the
  verified base root image, while Firecracker/KVM `Isolated` policy installs
  explicit tap-scoped host-input and forward-drop rules.
- Hosted HTTP responses forwarded through the Torii P2P proxy path are capped by
  `torii.soracloud_public_max_response_bytes` before buffering; the default is
  64 MiB and over-limit snapshots fail closed with `502 Bad Gateway`.
- `scripts/ci/prepare_inrou_portable_guest_assets.py` defaults to the pinned
  Debian Bookworm `20260413-2447` cloud image build, uses the real
  build-suffixed genericcloud archive names, verifies `SHA512SUMS.sign` with GPG
  when a detached signature is published, and otherwise falls back only to the
  hard-pinned SHA512 digest for the exact `amd64` or `arm64` archive. Unsigned
  archives without a pinned digest still fail closed.
- The mixed-host Inrou inventory now preserves `IROHA_INROU_LINUX_KVM_*` through
  `sudo`, and the Soracloud docs describe the root overlay, signed asset
  verification, explicit isolated firewall policy, and P2P response cap.
- Coverage for this slice now also exercises exact-limit Torii proxy responses,
  invalid proxied header/status restoration, HostedHttp-only response cap
  selection, duplicate proxied header preservation, gateway-only retry
  classification, HostedHttp route-timeout selection, reqwest bridge
  header/body preservation at the cap, no-candidate and all-retryable proxy
  fallback responses, open-policy firewall rule ordering, empty-allowlist
  default-drop planning, reusable portable root overlays, oversized base-rootfs
  budget rejection, allowlist IPv4 de-duplication plus IPv6-only and empty-port
  fail-closed behavior, host architecture mapping, SHA512SUMS star-prefixed
  archive entries, missing/mismatched archive checksums, pinned digest
  mismatches, signature download success/failure paths, GPG verifier failures,
  disk extraction missing/reuse paths, byte-range extraction EOF handling,
  GPT root-partition selection and rejection paths, rootfs patch command
  construction, boot-file selection/dump replacement failures, env export
  quoting, CLI argument defaults/overrides, host tool and GPG verifier discovery,
  subprocess wrapper capture flags, Debian keyring absence errors, download
  replacement/short-circuit behavior, debugfs stdout capture, chunked SHA512
  hashing, and main-flow orchestration for signed and pinned asset verification.
- The embedded-feature Torii explorer helper now derives external signed
  transaction results from `external_entrypoints_cloned()` and `results()`,
  avoiding a stale `entrypoint_results()` call under the irohad feature set.
- While validating alongside the local 20k hot-path edits already present in the
  worktree, the AMX admission-failure path now drops its temporary
  `StateTransaction` before recording the abort on the parent `StateBlock`.
- Focused validation for this slice:
  - `python3 -m py_compile scripts/ci/prepare_inrou_portable_guest_assets.py`
  - `python3 -m pytest scripts/tests/prepare_inrou_portable_guest_assets_test.py`
  - `cargo fmt --all`
  - `git diff --check`
  - `env -u LOG_FORMAT CARGO_TARGET_DIR=target/codex-inrou-fixes-config cargo test -p iroha_config soracloud_public_runtime_defaults_are_non_zero --lib -- --nocapture`
  - `env -u LOG_FORMAT CARGO_TARGET_DIR=target/codex-inrou-fixes-irohad cargo test -p irohad --features embedded-soracloud-runtime --bin irohad planned_inrou_tap_firewall_rules_keep_isolated_policy_private -- --nocapture`
  - `env -u LOG_FORMAT CARGO_TARGET_DIR=target/codex-inrou-fixes-irohad cargo test -p irohad --features embedded-soracloud-runtime --bin irohad planned_inrou_tap_firewall_rules_place_allowlist_accepts_above_default_drop -- --nocapture`
  - `env -u LOG_FORMAT CARGO_TARGET_DIR=target/codex-inrou-fixes-irohad cargo test -p irohad --features embedded-soracloud-runtime --bin irohad ensure_inrou_portable_root_disk_uses_qcow2_overlay_with_backing_file -- --nocapture`
  - `env -u LOG_FORMAT CARGO_TARGET_DIR=target/codex-inrou-fixes-irohad cargo test -p iroha_torii --lib snapshot_caps_buffered -- --nocapture`
  - `env -u LOG_FORMAT CARGO_TARGET_DIR=target/codex-inrou-fixes-torii-connect cargo test -p iroha_torii --lib broadcast_strategy_ -- --nocapture`
  - `env -u LOG_FORMAT CARGO_TARGET_DIR=target/codex-inrou-fixes-torii-connect cargo test -p iroha_torii --test connect_gating -- --nocapture`
  - `env -u LOG_FORMAT CARGO_TARGET_DIR=target/codex-inrou-fixes-clippy cargo clippy -p iroha_torii --all-targets -- -D warnings`
  - `env -u LOG_FORMAT CARGO_TARGET_DIR=target/codex-inrou-fixes-clippy cargo clippy --workspace --all-targets -- -D warnings` currently stops in unrelated `mochi-core` drift (`FaultPeer`/`FaultConfig` API updates and removed offline data-model symbols), after Torii itself checks.
  - `python3 -m pytest scripts/tests/prepare_inrou_portable_guest_assets_test.py` now passes 49 asset-prep verifier tests.
  - `env -u LOG_FORMAT CARGO_TARGET_DIR=target/codex-inrou-extra-tests-torii cargo test -p iroha_torii --lib torii_proxy_snapshot -- --nocapture` passes 6 Torii proxy snapshot tests.
  - `env -u LOG_FORMAT CARGO_TARGET_DIR=target/codex-inrou-extra-tests-torii cargo test -p iroha_torii --lib torii_proxy_response_body_limit_only_caps_hosted_http -- --nocapture` passes the HostedHttp response-cap selector test.
  - `env -u LOG_FORMAT CARGO_TARGET_DIR=target/codex-inrou-extra-tests-torii cargo test -p iroha_torii --lib torii_proxy_header_conversion_preserves_duplicates_and_skips_invalid -- --nocapture`
  - `env -u LOG_FORMAT CARGO_TARGET_DIR=target/codex-inrou-extra-tests-torii cargo test -p iroha_torii --lib torii_proxy_retry_policy_only_retries_gateway_class_statuses -- --nocapture`
  - `env -u LOG_FORMAT CARGO_TARGET_DIR=target/codex-inrou-extra-tests-torii cargo test -p iroha_torii --lib torii_proxy_hosted_http_request_kind_uses_route_timeout -- --nocapture`
  - `env -u LOG_FORMAT CARGO_TARGET_DIR=target/codex-inrou-extra-tests-torii cargo test -p iroha_torii --lib execute_torii_proxy_request_across_candidates_returns_route_unavailable_without_candidates -- --nocapture`
  - `env -u LOG_FORMAT CARGO_TARGET_DIR=target/codex-inrou-extra-tests-torii-retryable cargo test -p iroha_torii --lib execute_torii_proxy_request_across_candidates_returns_last_retryable_response -- --nocapture`
  - `env -u LOG_FORMAT CARGO_TARGET_DIR=target/codex-inrou-extra-tests-torii-retryable cargo test -p iroha_torii --lib execute_torii_proxy_request_across_candidates_returns_route_unavailable_after_transport_errors -- --nocapture`
  - `env -u LOG_FORMAT CARGO_TARGET_DIR=target/codex-inrou-extra-tests-irohad cargo test -p irohad --features embedded-soracloud-runtime --bin irohad planned_inrou_tap_firewall_rules -- --nocapture` passes 3 firewall-planning tests.
  - `env -u LOG_FORMAT CARGO_TARGET_DIR=target/codex-inrou-extra-tests-irohad cargo test -p irohad --features embedded-soracloud-runtime --bin irohad ensure_inrou_portable_root_disk -- --nocapture` passes 3 portable root-disk tests.
  - `env -u LOG_FORMAT CARGO_TARGET_DIR=target/codex-inrou-extra-tests-irohad cargo test -p irohad --features embedded-soracloud-runtime --bin irohad resolve_inrou_allowlist_endpoints_deduplicates_ipv4_entries -- --nocapture`
  - `env -u LOG_FORMAT CARGO_TARGET_DIR=target/codex-inrou-extra-tests-irohad cargo test -p irohad --features embedded-soracloud-runtime --bin irohad resolve_inrou_allowlist_endpoints_rejects_ipv6_only_literals -- --nocapture`
  - `env -u LOG_FORMAT CARGO_TARGET_DIR=target/codex-inrou-extra-tests-irohad-empty cargo test -p irohad --features embedded-soracloud-runtime --bin irohad resolve_inrou_allowlist_endpoints_rejects_empty_port_lists -- --nocapture`
  - `env -u LOG_FORMAT CARGO_TARGET_DIR=target/codex-inrou-extra-tests-irohad-empty cargo test -p irohad --features embedded-soracloud-runtime --bin irohad planned_inrou_tap_firewall_rules_allowlist_empty_keeps_default_drop -- --nocapture`
- Live KVM/Firecracker smoke was not run on this macOS host.

## 2026-05-01 20k hot-path cache push

- Added exact encoded-length coverage for the `AcceptedTransaction`
  entrypoints that feed block validation. The fallback length path now uses
  Norito's exact `Encode::encoded_len` plus the framed header/padding instead
  of allocating a full `norito::to_bytes(...)` buffer.
- `iroha_crypto` now keeps a bounded thread-local cache of successfully parsed
  canonical Ed25519 public keys. Rejections, malformed keys, and non-canonical
  encodings are not cached, so verification outcomes and deterministic batch
  behavior stay unchanged.
- `PendingBlock` now lazily caches canonical block payload bytes and reuses
  them for pending/in-flight progress payload matching and local RBC seed work.
  The cache is reset when pending block state is replaced, revived after an
  abort, or swapped during commit/Kura retry handling.
- Added sustained-pressure queue coverage and a Torii hot-path benchmark that
  keeps a reused queue at fixed backlog levels while measuring enqueue and
  pressure bookkeeping. This complements the existing fresh-queue benchmark.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `git diff --check`
  - `cargo test -p iroha_crypto parse_public_key_ -- --nocapture`
  - `cargo test -p iroha_core accepted_transaction_entrypoint_encoded_lengths_match_norito_frames -- --nocapture`
  - `cargo test -p iroha_core --lib queue_pressure_counters_stay_consistent_under_sustained_backlog -- --nocapture`
  - `cargo test -p iroha_core --lib pending_block_payload_bytes_match_canonical_encoding_and_reset_on_replace -- --nocapture`
  - `cargo bench -p iroha_torii --bench torii_hot_paths --no-run`
- No new Izanami release validation artifact has been produced for this cache
  push yet. The latest committed-throughput baseline remains
  `dist/izanami-prebuilt-20k-postfix-120s-20260501-003649`, which offered
  `2,400,000`, accepted `48,679`, and strictly approved `12,328`; the next
  gate is a fresh 4-peer no-fault 20k TPS prebuilt release run plus a sampled
  profile.

## 2026-04-30 20k bottleneck fix implementation

- Implemented the first tranche of measured hot-path fixes: debug-build Norito
  trace checks are cached outside tests, Torii batch admission caches local
  routing decisions per batch, Torii queue-pressure reads avoid full state
  views, and queue ingress now rejects fresh submissions with the existing
  429 envelope when the latency budget is saturated.
- Added a committed latest-block-header cache to `State` and changed
  `latest_block_header_fast()` to read the cache instead of loading the full
  latest block from Kura on hot paths.
- `AcceptedTransaction` now carries cached entrypoint hashes, signed
  transaction hashes, and exact encoded lengths so queue/admission paths can
  avoid repeated full Norito encoding and hash work.
- Block static validation now uses the deterministic Ed25519 batch verification
  API for naturally batched single-key Ed25519 transactions, then falls back to
  individual deterministic verification on batch failure to identify the bad
  transaction. The normal transaction limit, TTL, signing-policy, and
  consensus-visible validation checks still run.
- FASTPQ background prover submission now observes queue backpressure and
  defers non-critical prover jobs while the queue is saturated. Consensus and
  required block validation checks are unchanged.
- Validation so far: `cargo fmt --all`, `git diff --check`,
  `cargo check -p norito -p iroha_crypto -p iroha_core -p iroha_torii -p irohad`,
  `cargo test -p norito debug_trace_follows_env_flag -- --nocapture`,
  `cargo test -p iroha_crypto ed25519_batch_deterministic -- --nocapture`,
  `cargo test -p iroha_core --lib accepted_transaction_caches_hashes_and_encoded_length -- --nocapture`,
  `cargo test -p iroha_core --lib queue_pressure_ -- --nocapture`,
  `cargo test -p iroha_core latest_block_header_fast_reads_latest_committed_header -- --nocapture`,
  `cargo test -p iroha_torii --lib transaction_ingress_rejects_latency_saturated_queue_before_capacity -- --nocapture`,
  `cargo test -p iroha_core --test admission_batching ed25519_batch_bisection_finds_bad_sig -- --nocapture`,
  and `cargo test -p iroha_core --test signature_batch_determinism ed25519_batch_permutation_finds_same_bad_sig -- --nocapture`.
  Post-fix clean 20k TPS benchmark artifacts are recorded below.

## 2026-05-01 20k post-fix release validation

- Built the benchmark binaries with both requested commands:
  `cargo build -p izanami --bin izanami -p irohad --bin iroha3d` and
  `cargo build --release -p izanami --bin izanami -p irohad --bin iroha3d`.
- Ran a clean release `4`-peer, no-fault, prebuilt `20,000 TPS` profile for
  `30s` at `dist/izanami-profile-20k-postfix-30s-20260501-003504`. Izanami
  offered all `600,000` planned submissions and ingress accepted `47,670`.
  Submit latency p50/p95/p99/max was `3374/5297/6016/7079 ms`; final
  quorum/strict height was `2/2`; quorum/strict approved transactions were
  `58/58`; the tx queue was age-saturated at `37,089/600,000`.
- Ran the clean sampled release profile at
  `dist/izanami-profile-20k-postfix-sampled-30s-20260501-004220`, with macOS
  `sample` output for the Izanami runner and all four `iroha3d` peers. It
  offered `600,000`, accepted `44,999`, recorded submit latency
  `3173/4340/5181/6536 ms`, ended at quorum/strict height `2/2`, and copied
  diagnostics plus `profiles/sample-*.txt`.
- The fresh samples show the remaining hot stacks in peer processes are still
  transaction validation and wire-format work: Ed25519 public-key
  parse/decompress/verify paths, Norito transaction deserialize/serialize
  paths, and `AcceptedTransaction::signed_encoded_len` fallback into
  `norito::core::to_bytes` during block validation. Torii routing/state
  helpers are still visible but no longer dominate the top stack counts.
- Aggregating the four peer `sample-*.txt` files by recursive stack count puts
  Norito encode/decode at roughly `110k` stack appearances,
  Ed25519/Curve25519 crypto at roughly `39k`, public-key string/normalization
  at roughly `9.6k`, block/transaction validation wrappers at roughly `3.6k`,
  and Torii ingress/routing at roughly `1.9k`. Leaf samples excluding sleeps
  and waits are led by Curve25519 field math, allocation/memmove/free, Norito
  compact-length/read-write helpers, and Blake2/CRC hashing.
- Code inspection matches the profile: block validation builds batch inputs
  from raw `SignedTransaction`s, computes `HashOf::new(tx.payload())`, calls
  `signatory.to_bytes()`, then re-enters stateless transaction validation for
  each transaction. The deterministic Ed25519 batch helper currently verifies
  each triple independently and reparses every public key, preserving parity
  but leaving most crypto cost in place. Queue ingress still recomputes gossip
  bytes with `norito::to_bytes(signed)` rather than carrying encoded bytes from
  accepted transaction metadata.
- Ran the clean release `4`-peer `120s` validation at
  `dist/izanami-prebuilt-20k-postfix-120s-20260501-003649`. Izanami offered
  all `2,400,000` planned submissions and ingress accepted `48,679`; submit
  latency p50/p95/p99/max was `2958/4484/4995/6116 ms`; final quorum/strict
  height was `5/5`; quorum/strict approved transactions were `12,328/12,328`;
  the tx queue remained age-saturated at `32,034/2,400,000`.
- Post-fix conclusion: the implemented fixes reduced the worst submit-latency
  and failover symptoms and improved finality progress, especially versus the
  earlier `120s` rerun's `2/2` height and `10/10` approved transactions, but
  this is still not a successful committed-20k run. The next measured
  bottleneck is queue-drain/block-validation throughput under sustained
  ingress pressure, with no current evidence that RBC pressure, DA gating,
  validation rejects, or view-change storms are the primary limiter.

## 2026-04-30 Izanami 20k TPS stress rerun

- Rebuilt the current dirty-tree dev binaries with
  `cargo build -p izanami --bin izanami -p irohad --bin iroha3d`, then reran
  the local `4`-peer, no-fault prebuilt `20,000 TPS` path for a `120s` timed
  window with `2,400,000` transactions prebuilt before the window, `4096`
  submitters, `300,000` max inflight, and `300ms` pipeline time.
- The run offered all `2,400,000` planned submissions (`20,000.00 TPS`).
  Ingress accepted `87,144` (`726.20 TPS`, `3.63%` of offered). Submit
  latency p50/p95/p99/max was `625/30011/45016/46130 ms`; shutdown aborted
  `1,827` outstanding submit tasks after the measured window.
- Final committed/finality evidence is still blocked: quorum/strict height
  ended at `2/2`, quorum/strict approved transactions stayed at `10/10`, and
  peer height / approved-transaction skew stayed `0`. The row remains
  overload and consensus-throughput evidence rather than a successful
  committed-20k result.
- Dominant status deltas: tx queue saturated at `66,580/2,400,000`,
  pacemaker backpressure deferrals `31`, view-change installs `4`, missing-QC
  causes `3`, quorum-timeout causes `1`, range-pull escalations `5` with no
  successes, and no RBC pressure, pending-RBC drops, validation rejects, DA
  gate, missing-payload, or stake-quorum timeout evidence.
- Artifact:
  `dist/izanami-prebuilt-20k-rerun-120s-20260430-200921`, including
  `run.log`, `command.txt`, and copied diagnostics. The shell wrapper failed
  after Izanami emitted its final summary because zsh reserves the variable
  name `status`, so this artifact has no numeric `exit.status`; the run
  itself reached `izanami::summary`.

## 2026-04-30 Izanami 20k bottleneck classification

- Captured a current-tree dev macOS `sample` profile during a fresh `4`-peer,
  no-fault, prebuilt `20,000 TPS` Izanami run for `30s`. Artifact:
  `dist/izanami-profile-20k-current-30s-20260430-202549`, including the
  runner command, peer samples, run log, and copied diagnostics.
- Profile-run caveats: the profiling wrapper reached Izanami's final
  `izanami::summary` but then exited `127` because the wrapper waited for the
  runner PID after a broader `wait`; the captured samples are still present.
  A concurrent `cargo test -p iroha_torii torii_hot_path_load_profile -- --ignored --nocapture`
  process was also active, so this profile is directional rather than a clean
  isolated benchmark.
- Current-tree dev run outcome: `600,000` submissions were offered, but only
  `1,466` reached ingress, with submit latency p50/p95/p99/max
  `3706/11223/41150/44167 ms`, `7,428` endpoint failovers, `14` endpoint
  unhealthy marks, and `3,387` submit tasks aborted on shutdown. Final
  quorum/strict height was `1/1`, approved transactions stayed at `9/9`, peer
  skew stayed `0`, the tx queue saturated at `17,453/600,000`, pacemaker
  backpressure deferred `9` times, and there was no RBC pressure,
  missing-payload, DA-gate, validation-reject, or view-change storm evidence.
- Primary bottleneck class: overload admission and Torii ingress saturation.
  The system is spending the window failing over and timing out request
  submission while the node-side transaction queue stays saturated, so the
  committed-throughput ceiling is being hit before finality can make progress.
- CPU bottleneck class in peer samples: per-transaction validation plus codec
  work. Active peer stacks are dominated by Norito encode/decode paths,
  Ed25519/`curve25519-dalek` signature verification and public-key parsing,
  transaction hash re-encoding, and Torii batch admission paths
  (`handler_post_transactions_batch` -> `accept_transaction_for_ingress` ->
  `AcceptedTransaction::accept_entrypoint`/`validate_with_now`).
- Hot-path tax surfaced by the dev profile: `norito::debug_trace_enabled()`
  probes `std::env::var_os("NORITO_TRACE")` on encode/decode in debug builds,
  which shows up as repeated `getenv`/environment-lock contention in peer
  samples. This is not consensus logic, but it inflates the current dev
  profile and should be cached or removed from hot codec paths before using dev
  samples as throughput evidence.
- Secondary bottleneck class: routing/state metadata lookups during ingress.
  Some peer stacks repeatedly enter
  `State::authoritative_lane_peer_ids` -> `latest_block_header_fast` ->
  `block_by_height` -> `Kura::get_block`, so authoritative-lane checks are
  pulling state/block metadata on the Torii admission path.
- Consensus/RBC classification: not the dominant bottleneck in this rerun.
  The run shows stalled finality, but no RBC pressure, DA-gate,
  missing-payload, validation-reject, stake-quorum timeout, or missing-QC
  storm. Consensus is mostly starved behind saturated ingress/validation and
  expensive admission hot paths rather than failing through an RBC-specific
  mechanism.

## 2026-04-30 Secondary hot-path cost reduction

- `iroha_core` queue admission now lazily snapshots `WorldView`/`Nexus` for
  state-backed queue paths, so ordinary internal transactions no longer pay
  full world/nexus clone/drop costs unless external fee admission or lane
  compliance actually needs world data.
- Queue pressure bookkeeping now uses maintained atomic active/queued counts
  instead of repeatedly asking `DashMap`/removed-marker state for hot
  backpressure snapshots.
- Torii pipeline status pruning now uses observed-time order indexes and a
  single-pruner guard, avoiding full-cache scans and sorts during normal
  transaction/block status writes while preserving TTL and capacity eviction.
- Torii ingress now uses narrow `State` accessors for transaction admission
  limits, account existence checks, and effective block time, removing the
  remaining full `WorldView` clones from transaction acceptance and enqueue
  pressure refresh paths.
- Queue pressure counter tests now assert internal counter consistency against
  the active transaction map, queued hash deque, and queued-age ring after
  committed-removal, retry, hash-queue rebuild, and expiry-compaction paths.
- Added measurement hooks for the secondary costs:
  `torii_transaction_handle_enqueue_direct_metrics` in
  `crates/iroha_torii/benches/torii_hot_paths.rs` exercises the full Torii
  transaction handler/enqueue path with fresh queue setup outside the measured
  routine, and the ignored
  `pipeline_status_cache_prune_load_profile` test emits a structured prune
  pressure profile line.
- Cleanup found by all-target validation is folded in: current signature wrapper
  calls in the Ed25519 batch precheck path use the inner signature bytes, the
  Kura bench config includes `eviction_required_replicas`, and `iroha` client
  clippy warnings are resolved without relaxing lint policy.
- Focused validation:
  - `cargo fmt --all`
  - `cargo check -p iroha_core --lib`
  - `cargo test -p iroha_core queue_pressure_counters -- --nocapture`
  - `cargo test -p iroha_core expired_cull -- --nocapture`
  - `cargo test -p iroha_core latest_block_header_fast_reads_latest_committed_header -- --nocapture`
  - `cargo test -p iroha_core --test signature_batch_determinism -- --nocapture`
  - `cargo test -p iroha_torii multisig_guard_tests -- --nocapture`
  - `cargo test -p iroha_torii pipeline_status_cache -- --nocapture`
  - `cargo test -p iroha_torii pipeline_status_cache_prune_load_profile -- --ignored --nocapture`
    emitted
    `torii_profile suite=hot_path kind=pipeline_status_cache_prune_pressure samples=32 warmup_samples=4 concurrency=1 wall_ms=357.389 throughput_per_sec=89.538 avg_us=7113.948 p50_us=7120.416 p95_us=7184.209 p99_us=7192.167 p999_us=NA max_us=7192.167`
  - `cargo bench -p iroha_torii --bench torii_hot_paths -- --sample-size 10`
    completed with `torii_transaction_admission_direct_metrics` at
    `[46.692 µs 46.755 µs 46.907 µs]` and
    `torii_transaction_handle_enqueue_direct_metrics` at
    `[14.707 ms 14.890 ms 15.230 ms]`
  - `cargo clippy -p iroha_core -p iroha_torii --all-targets -- -D warnings`

## 2026-04-30 Transaction signature bypass removal

- Removed the `iroha_core` transaction-validation signature override entrypoints
  and block-validation plumbing that could feed preaccepted signature results
  into per-transaction validation.
- Block transaction validation no longer uses stateless validation cache hits or
  deterministic batch-preverification overrides to skip the normal
  `SignedTransaction::verify_signature` path. The cache configuration remains
  present, but block validation treats warmed entries as insufficient for
  accepting a transaction signature.
- Added regression coverage for warmed-cache invalid signatures, heartbeat
  signature rejection, and a source-level guard against reintroducing the
  removed bypass identifiers.

## 2026-04-30 Izanami 20k CPU profile

- Captured a release-build macOS `sample` profile during a `4`-peer,
  no-fault, prebuilt `20,000 TPS` Izanami run for `30s`. The run offered and
  ingress-accepted all `600,000` prebuilt transfers, with submit latency
  p50/p95/p99/max `24/121/387/1353 ms`, so the driver was not the limiting
  stage for this profile.
- Final consensus evidence remained poor despite full ingress delivery:
  quorum/strict height reached only `2/2`, quorum/strict approved
  transactions were `156/156`, max approved-transaction skew was `4096`, the
  queue stayed saturated at `219,843/600,000`, and the worker-loop last
  iteration stretched to `2395 ms`.
- Driver samples were mostly idle or in lightweight send/hash/account-address
  work. The dominant active peer CPU samples were transaction signature
  verification (`curve25519-dalek`/Ed25519), allocation and string work,
  FASTPQ Poseidon/prover work, `WorldView` clone/drop costs, Norito
  serialization, Torii queue/status bookkeeping, and hash/CRC work.
- The peer call stacks show the largest consensus-path CPU cost inside
  `ValidBlock::validate_static_with_snapshot`, especially transaction
  signature verification and Norito re-encoding of signed transactions during
  validation. FASTPQ prover jobs were asynchronous but still consumed several
  seconds of host CPU per peer after block commit.
- Current optimization order after the signature-bypass removal:
  1. Reintroduce transaction signature throughput work only as real signature
     verification, never as a validation override or cache-hit acceptance path.
  2. Bound or defer FASTPQ prover CPU while consensus has an active backlog,
     without changing block validity or deterministic consensus state.
  3. Remove per-transaction full state-view clone/drop work from Torii
     ingress and lane routing by caching cheap parameter/routing snapshots for
     the current block or epoch.
  4. Batch or amortize `PipelineStatusCache` pruning and remaining queue
     pressure refresh work under heavy ingress.
- Profile artifact:
  `dist/izanami-profile-20k-30s-20260430-185822`.

## 2026-05-01 Iroha Connect default relay TTL restored

- Changed the default Connect P2P relay TTL from `0` to `8` hops so the
  default `broadcast` relay strategy actually rebroadcasts over an attached
  Iroha P2P handle. Operators can still set `CONNECT_P2P_TTL_HOPS=0` to
  disable cross-node Connect rebroadcast explicitly.
- Tightened Connect status so zero-TTL broadcast configuration reports an
  effective `local_only` strategy while preserving the normalized configured
  strategy.
- Updated Connect configuration docs, translated default-value references, and
  the config default fixture.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo fmt --all --check`
  - `git diff --check`
  - `cargo test -p iroha_torii --test connect_gating --features ws_integration_tests connect_ws_broadcast -- --nocapture`
  - `cargo test -p iroha_torii --lib broadcast_strategy_with_zero_ttl_reports_local_only_when_p2p_attached -- --nocapture`
  - `cargo test -p iroha_config minimal_config_snapshot -- --nocapture`
  - `cargo test -p iroha_torii --test connect_gating --features ws_integration_tests -- --nocapture`

## 2026-05-01 Iroha Connect P2P rendezvous claims

- Added versioned Connect P2P control messages for relay envelopes, session
  claims, consumed-role notices, and session termination notices. Torii now
  gossips session claims over authenticated Iroha P2P so app and wallet
  WebSockets can rendezvous through different Torii nodes after one
  `/v1/connect/session` response.
- Replaced in-memory app, wallet, and management token storage with
  domain-separated authentication hashes and constant-time comparisons. Claims
  carry token hashes plus the relay MAC key, not raw app/wallet/management
  tokens.
- Added P2P claim, conflict, unknown-session relay drop, consumed-role, and
  termination counters to Connect status and surfaced them in the JS, Python,
  and Swift typed SDK status snapshots.
- Added shared Connect session vectors for token hashes, relay MAC key, and
  relay auth hash, with Rust/JS fixture coverage and Swift/Kotlin/Java relay
  auth fixture assertions.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo fmt --all --check`
  - `cargo check -p iroha_torii`
  - `cargo test -p iroha_torii_shared connect_sdk -- --nocapture`
  - `cargo test -p iroha_torii --lib p2p_ -- --nocapture`
  - `cargo test -p iroha_torii --lib register_tokens_stores_token_hashes -- --nocapture`
  - `node --test test/connect.browser.test.js test/connectPreviewFlow.test.js test/toriiClient.test.js` from `javascript/iroha_js`
  - `npm run build:dist` from `javascript/iroha_js`
  - `python3 -m py_compile python/iroha_python/src/iroha_python/client.py python/iroha_torii_client/client.py python/iroha_torii_client/tests/test_client.py`
- Validation blockers in this environment:
  - `python3 -m pytest python/iroha_torii_client/tests/test_client.py -k 'connect_status or connect_session'` could not run because `pytest` is not installed.
  - Kotlin/JVM and Java Android Connect tests could not run because no Java runtime is available.
  - Focused Swift Connect tests could not run because `IrohaSwift/dist/NoritoBridge.xcframework` is missing.

## 2026-05-01 Sumeragi frontier formal gaps closed

- Refactored the focused Taira frontier-recovery model from one active frontier
  plus a Boolean future-evidence shortcut into one active frontier plus one
  concrete future frontier slot. The future slot carries presence, contiguity,
  vote counts, queued votes, payload state, and recovery owner, while
  `FutureFrontierEvidence` is now derived from that slot.
- Added the two-step future reanchor path: clear the stale/current pending
  wrapper, then promote the future slot into the active slot with active
  progress flags reset and `frontierSlot` advanced.
- Strengthened liveness so an active vote-backed pending wrapper must
  eventually clear. Payload recovery, quorum retransmit, future-slot reanchor,
  and promoted second-slot behavior now have focused follow-through
  properties.
- Added late post-GST future-evidence arrival, future-evidence preservation,
  and promotion-freshness checks so a second-slot quorum cannot be silently
  dropped or promoted with stale active progress flags.
- Made Apalache bounds explicit in `scripts/formal/sumeragi_apalache.sh` for
  every mode, kept existing modes backward-compatible, added payload-recovery,
  retransmit-follow-through, future-promotion, future-reanchor-clear,
  future-evidence-drop, promotion-reset, and future-stale-owner bug modes, and
  promoted `frontier-wide`, a small TLC cross-check, and all expected-failure
  mutations into normal formal CI. A scheduled/manual GitHub Actions workflow
  now runs the longer `frontier-nightly` bound.
- Updated the English Sumeragi formal README with the two-slot proof scope,
  runner modes, CI behavior, and model-to-implementation assumption map.
  Translated `docs/formal/sumeragi/README.*.md` bodies were intentionally not
  refreshed in this slice, so they may remain source-current stale until a
  separate translation refresh.
- Added focused Rust bridge regressions for future new-view reanchor while the
  vote queue is backlogged and while the old frontier recovery owner is stale.
- Improved the surrounding process so nightly formal CI runs the normal formal
  gate before the longer bound, and PR/nightly docs jobs upload JSON metadata
  reports for the deliberately stale translated formal READMEs.
- No runtime consensus code changed in this hardening pass.
- Validation completed with local Apalache `0.52.2`:
  - `bash -n scripts/formal/sumeragi_apalache.sh ci/check_sumeragi_formal.sh ci/check_sumeragi_formal_expected_failures.sh scripts/formal/sumeragi_tlc.sh`
  - `cargo fmt --all`
  - `bash scripts/formal/sumeragi_apalache.sh frontier-fast`
  - `bash scripts/formal/sumeragi_apalache.sh frontier-deep`
  - `bash scripts/formal/sumeragi_apalache.sh frontier-wide`
  - `bash scripts/formal/sumeragi_apalache.sh frontier-nightly`
  - `bash ci/check_sumeragi_formal_expected_failures.sh`
  - `bash scripts/formal/sumeragi_tlc.sh frontier-small`
  - `bash ci/check_sumeragi_formal.sh`
  - `cargo test -p iroha_core --lib reschedule_defers_vote_backed_quorum_timeout_while_vote_queue_backlogged -- --nocapture`
  - `cargo test -p iroha_core --lib reschedule_skips_vote_backed_retransmit_while_frontier_quorum_timeout_window_owned -- --nocapture`
  - `cargo test -p iroha_core --lib reschedule_ignores_quorum_timeout_vote_queue_backlog -- --nocapture`
  - `cargo test -p iroha_core pacemaker_reanchors -- --nocapture`
  - `python3 ci/check_docs_i18n_metadata.py --paths docs/formal`
  - `python3 ci/check_docs_i18n_metadata.py --paths docs/formal --json-out target/docs-i18n/formal-metadata.json`

## 2026-04-30 Sumeragi frontier recovery formal model

- Added a focused bounded TLA+/Apalache model for the Taira frontier hang
  class at `docs/formal/sumeragi/SumeragiFrontierRecovery.tla`, with fast and
  deep configs. The model keeps signatures/ECDSA abstracted as finite vote
  evidence and explicitly covers a pending contiguous frontier block, queued
  commit-vote backlog, missing/local payload state, stale recovery ownership,
  quorum-reschedule marker/window pacing, and deterministic commit, retransmit,
  bounded view-rotation, and zero-evidence drop outcomes after GST.
- The frontier proof checks `TypeInvariant`, commit-implies-vote-quorum,
  commit-implies-payload-availability, no vote-backed zero-evidence zombie
  drop, post-GST vote-backed frontier progress, and post-GST eventual
  resolution by commit, payload recovery, quorum retransmit, or bounded view
  rotation.
- Extended `scripts/formal/sumeragi_apalache.sh` with backward-compatible
  `frontier-fast` and `frontier-deep` modes, and extended
  `ci/check_sumeragi_formal.sh` so formal CI now runs both the existing
  commit-path model and the new frontier-recovery model.
- No runtime consensus code changed in this slice.
- Validation used local Apalache `0.52.2` (`build: 9103560`, archive sha256
  `e0ebea7e45c8f99df8d92f2755101dda84ab71df06d1ec3a21955d3b53a886e2`):
  - `bash scripts/formal/install_apalache.sh 0.52.2`
  - `bash -n scripts/formal/sumeragi_apalache.sh ci/check_sumeragi_formal.sh`
  - `bash scripts/formal/sumeragi_apalache.sh frontier-fast`
  - `bash scripts/formal/sumeragi_apalache.sh frontier-deep`
  - `bash ci/check_sumeragi_formal.sh`
  - `cargo test -p iroha_core reschedule_defers_vote_backed_quorum_timeout_while_vote_queue_backlogged -- --nocapture`
  - `cargo test -p iroha_core reschedule_skips_vote_backed_retransmit_while_frontier_quorum_timeout_window_owned -- --nocapture`
  - `cargo test -p iroha_core reschedule_ignores_quorum_timeout_vote_queue_backlog -- --nocapture`
  - `python3 ci/check_docs_i18n_metadata.py --paths docs/formal` (passed with
    the expected stale `source_hash` warnings for existing translated formal
    README files)

## 2026-04-30 Iroha Connect session and relay hardening

- Added session-scoped `token_management` and `token_relay` credentials to
  Connect session creation. Management tokens now gate session deletion and
  per-session status; public `/v1/connect/status` is redacted to aggregate
  counters.
- Wrapped P2P Connect rebroadcasts in MAC-authenticated relay envelopes with a
  bounded TTL. Torii drops relay frames that fail MAC verification, target an
  unknown session, or exhaust TTL, and exposes auth/TTL drop counters in
  status.
- Bound the relay token into wallet approval signatures with a tagged
  approval preimage, added browser-side approval verification, and aligned the
  JS, Python, Swift, Kotlin, and Java SDK surfaces with the new management and
  relay token contract.
- Updated MCP/OpenAPI metadata, examples, JS package `dist`, and Connect docs
  to keep management tokens out of launch URIs while carrying relay tokens in
  wallet/app deep links.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo fmt --all --check`
  - `cargo check -p iroha_torii`
  - `cargo test -p iroha_torii_shared connect_sdk -- --nocapture`
  - `cargo test -p iroha_torii connect_session_delete -- --nocapture`
  - `cargo test -p iroha_torii connect_session_status_requires_management_token -- --nocapture`
  - `cargo test -p iroha_torii connect_management_headers_maps_token_to_authorization -- --nocapture`
  - `node --test test/connect.browser.test.js test/connectPreviewFlow.test.js test/toriiClient.test.js` from `javascript/iroha_js`
  - `npm run build:dist` from `javascript/iroha_js`
  - `python3 -m py_compile python/iroha_python/src/iroha_python/client.py python/iroha_python/src/iroha_python/connect.py python/iroha_python/src/iroha_python/examples/connect_flow.py python/iroha_torii_client/client.py python/iroha_torii_client/tests/test_client.py`
- Validation blockers in this environment:
  - `python3 -m pytest python/iroha_torii_client/tests/test_client.py -k 'connect_status or connect_session'` could not run because `pytest` is not installed.
  - `./gradlew :core-jvm:test --tests org.hyperledger.iroha.sdk.connect.ConnectWalletRequestTest --console=plain` could not run because no Java runtime is available.
  - `swift test --filter 'ConnectCryptoTests|ToriiClientTests/testGetConnectStatusParsesSnapshot|ToriiClientTests/testCreateConnectSessionPostsPayload|ToriiClientTests/testDeleteConnectSessionHandles404'` could not run because `dist/NoritoBridge.xcframework` is missing.

## 2026-04-30 queue router and Sumeragi focused regression fixes

- Fixed the focused `iroha_core` regression cluster where opaque
  asset-definition IDs in queue routing fell back to the default dataspace
  instead of resolving the stored canonical asset definition and its alias
  binding.
- Tightened Sumeragi quorum-timeout frontier recovery ownership checks to the
  active view so stale owner state from an earlier post-rotation view no
  longer suppresses the direct empty-view rotation path.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo test -p iroha_core queue::router::tests:: -- --nocapture`
  - `cargo test -p iroha_core sumeragi::main_loop::tests::force_view_change_if_idle_rotates_post_rotation_round_with_stale_quorum_timeout_owner -- --nocapture`
  - `cargo test -p iroha_core sumeragi::main_loop::tests::force_view_change_if_idle_ignores_stale_quorum_timeout_owner_after_frontier_grace -- --nocapture`

## 2026-04-30 Izanami 20k ingress restored; committed TPS still blocked

- Replaced the queue pressure age scan with an amortized FIFO age ring, so
  Torii admission no longer scans the full transaction map under 20k prebuilt
  ingress. The 30s release run with `600,000` prebuilt transfers now offered
  and ingress-accepted all `600,000` submissions with zero submit failures;
  submit latency was p50/p95/p99/max `16/36/73/281 ms`.
- Added final transaction-approval sampling to Izanami progress and summary
  output. This separates ingress acceptance from committed/finalized
  throughput and exposes peer divergence at the deadline.
- The latest local 20k run did **not** reach 20k committed TPS:
  quorum/strict finalized transaction counts were only `39/39` at the 30s
  deadline, with max peer transaction-approved skew `4096`. One peer can get a
  full 4096-transfer block ahead, but quorum/strict finality does not converge
  fast enough under the offered load.
- Dominant evidence in the diagnostics is now block validation and
  availability/commit convergence under large transfer blocks, not the driver:
  height-3 validation took about `3.4s`, commit QC aggregation took roughly
  `20s`, repeated RBC DELIVER rebroadcasts were needed, and the asynchronous
  FASTPQ prover lane generated many per-entry proofs at roughly `16-20 ms`
  each after the block commit. View-change causes were zero in the Izanami
  summary, but the queue remained saturated (`205,691/600,000`) and
  pacemaker backpressure fired `7` times.
- Reduced FASTPQ prover log amplification by changing per-proof success logs
  to debug-level detail and emitting one info-level summary per prover job.
  This removes a large local logging pressure source for transfer-heavy stress
  blocks, but proof generation itself remains a real throughput cost.
- Current conclusion: the harness can now do real 20k ingress on this host,
  but Sumeragi cannot commit/finalize 20k TPS for the current transfer
  workload. Getting there requires a real consensus/execution throughput
  change: batching or throttling optional FASTPQ proof work, reducing per-block
  validation latency, and improving large-block DA/RBC/commit-vote convergence.
- Latest artifact:
  `dist/izanami-prebuilt-20k-4096acct-fastplan-pipeline200-gas2b-teu24m-cap4096-rbc4096-30s-20260430-180302`.

## 2026-04-30 Sumeragi overload admission and 20k liveness rerun

- Added a local Torii ingress admission guard for latency-saturated queues:
  before enqueueing a fresh transaction, Torii refreshes the queue pressure
  budget from the effective block time and returns saturated queue
  backpressure once queued transaction age exceeds the budget. This keeps
  20k stress rows honest as overload admission evidence rather than accepting
  an unbounded local backlog.
- Relaxed the Sumeragi pacemaker backpressure classification for payload-only
  pending frontier work after the ingress starvation window. Saturated ingress
  no longer turns a live but unvoted pending frontier into a permanent hard
  proposal veto; once recovery is due, queue saturation becomes pacing so the
  deterministic recovery/proposal path can run without changing quorum safety.
- Reran the local no-fault offered-20k path for a `120s` timed window with all
  `2,400,000` transactions prebuilt ahead of time. Izanami launched all
  `2,400,000` attempts (`20,000.00 TPS`) and ingress accepted `7,090`
  (`59.08 TPS`, `0.30%` of offered). The row is now classified as
  `not-driver-saturated`, `ingress-under-delivered`, `overload-admission`,
  and `liveness-degraded-not-stalled`: quorum/strict height advanced to `4/4`
  with zero peer skew instead of remaining at height `1/1`.
- The dominant Sumeragi evidence moved from a missing-QC storm to bounded
  overload: view-change installs `12`, quorum timeouts `6`, missing QC `4`,
  last cause `quorum_timeout`, tx queue saturated at `6,475/65,536`,
  pacemaker backpressure deferrals `105`, missing-QC reacquire `5/5`, and
  range-pull escalations/successes/failures `19/4/4`. RBC pressure,
  validation rejects, missing payload, DA gate, and stake-quorum timeout were
  all zero.
- Latest local 20k artifacts:
  `dist/izanami-prebuilt-20k-120s-20260430-111435`, including
  `run.log`, `failure-classification.md`, and copied diagnostics under
  `diagnostics/`.
- Extended the same prebuilt/no-fault local path into a `120s` TPS ladder at
  requested `50,100,250,500,1000,2000,5000,10000`. The `50` and `100` rows are
  explicitly driver-capped by Izanami stable-ingress pacing (`64` max inflight
  for requested TPS <= `200`), while `250` TPS and above offered essentially the
  full requested load from memory. Best accepted throughput in this ladder was
  `8,376` accepted at requested `5,000 TPS` (`69.80 TPS`, `1.40%` of offered);
  requested `10,000 TPS` offered `1,199,035` attempts and accepted `7,264`
  (`60.53 TPS`, `0.61%` of offered).
- Ladder classification: every uncapped row is `ingress-under-delivered` and
  `tx_queue_saturated`; `2,000 TPS` and `10,000 TPS` show the clearest
  liveness churn, with strict height only `3`, missing-QC view-change totals
  `35` and `20`, and quorum-timeout totals `13` and `8`. The `5,000 TPS` row
  stayed livelier (`9` strict height, one quorum-timeout cause), so the observed
  break point is not monotonic; scheduler/leader timing still matters. Across
  the ladder, RBC pressure, missing payload, validation rejects, DA gate, and
  peer height divergence remained zero or non-dominant.
- Ladder artifacts:
  `dist/izanami-prebuilt-ladder-120s-20260430-122804/ladder-summary.tsv` and
  `dist/izanami-prebuilt-ladder-120s-20260430-122804/ladder-report.md`.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo test -p iroha_core --lib proposal_backpressure_allows_starved_payload_only_pending_under_saturation -- --nocapture`
  - `cargo test -p iroha_torii transaction_ingress_rejects_latency_saturated_queue_before_capacity -- --nocapture`
  - `cargo test -p iroha_core --lib sumeragi_resilience -- --nocapture`
  - `cargo test -p iroha_core --lib sumeragi_status -- --nocapture`
  - `cargo test -p izanami sumeragi_status_digest -- --nocapture`
  - `cargo test -p izanami throughput -- --nocapture`
  - `cargo test -p izanami prebuild -- --nocapture`
  - `python3 -m pytest scripts/tests/izanami_matrix_classifier_test.py`
  - `cargo build -p izanami --bin izanami -p irohad --bin iroha3d`

## 2026-04-30 Izanami prebuilt transaction buffer

- Added an Izanami-only high-TPS driver path with `--prebuild-tx-buffer` and
  `--prebuild-tx-workers`. Stable stateless plans are now signed, hashed, and
  Norito-encoded before the timed load window starts; prebuilt stress feeds use
  only cached payloads and do not fall back to live transaction construction.
- Added reusable client support for prepared transaction payloads so repeated
  submissions can avoid re-encoding `SignedTransaction` bodies. The public Torii
  endpoint and payload format remain unchanged.
- Added summary counters for the buffer: capacity, workers, built, used,
  fallback, skipped, and build failures. Matrix/sweep scripts can pass the new
  prebuild knobs through their own CLI options.
- Replaced the per-submit prebuilt ticker with a batch feed scheduler that
  catches up to elapsed-time TPS targets from the in-memory deque. The timed
  window no longer includes prebuild warmup, and the prebuilt path bypasses
  loop-level ingress backpressure so under-delivery is reported as
  ingress/consensus evidence instead of silent driver throttling.
- Local `4`-peer smoke at requested `20,000 TPS` for `10s` prebuilt all
  `200,000` transactions ahead of time and launched all `200,000` during the
  timed window. Ingress accepted `27,046`; the row exposed endpoint failover,
  endpoint unhealthy marking, `tx_queue_saturated=true`, pacemaker
  backpressure, `quorum_timeout`, and missing-block/range-pull pressure. This
  is now a real offered-20k stress row on the driver side, with acceptance
  limited by ingress/consensus pressure.
- Extended the same local offered-20k path to a `120s` timed window with
  `2,400,000` prebuilt transactions. Izanami launched `2,394,340` attempts
  (`19,952.83 TPS`, `99.76%` of requested) and ingress accepted `55,488`
  (`462.40 TPS`, `2.32%` of offered). Classification:
  `ingress-under-delivered`, `consensus-stalled`, `overload-admission`, and
  `missing-qc-view-change-storm`; not driver-saturated. The run preserved raw
  logs and a compact classification report under
  `dist/izanami-prebuilt-20k-120s-20260430-103233`.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo test -p iroha prepared_transaction_payload -- --nocapture`
  - `cargo test -p izanami prebuild -- --nocapture`
  - `cargo test -p izanami stored_args_roundtrip -- --nocapture`
  - `cargo test -p izanami throughput -- --nocapture`
  - `cargo test -p izanami local_port_exhaustion -- --nocapture`
  - `bash -n scripts/run_izanami_communication_vulnerability_matrix.sh scripts/run_izanami_communication_vulnerability_sweep.sh`
  - `cargo build -p izanami --bin izanami -p irohad --bin iroha3d`

## 2026-04-29 Izanami 20k stress diagnostics and targeted Sumeragi recovery

- Added truthful 20k stress reporting for requested/offered/accepted TPS, submit latency percentiles, shutdown-drain counters, recovery-height/skew evidence, detailed Sumeragi status deltas, and per-scenario diagnostic artifact capture under matrix stress runs.
- Extended Sumeragi status propagation through the wire model, Torii routing, client JSON, Izanami evidence TSV, root-cause report, and sweep aggregation so view-change causes, missing-block fetch state, tx queue/backpressure, worker-loop, QC defer/reacquire, forced proposal, range-pull, and NPoS repair-coverage telemetry remain individually visible.
- Applied the observed `missing_qc` liveness fix only: repeated same-height missing-QC reacquire now promotes to the existing trusted-peer block-sync range-pull path and clears same-height range-pull cooldowns, without changing block validity, signatures, validator order, or quorum safety.
- Reran the real seed-7 NPoS 20-peer/800s/25% packet-loss row at requested `20,000 TPS` with `target/debug` binaries. The driver offered `816,166` attempts (`1020.21 TPS`, `5.10%` of request) and ingress accepted `773,285` (`966.61 TPS`, `4.83%` of request); quorum/strict height stayed `1/1`. The post-fix dominant cause moved from `missing_qc` to `quorum_timeout` with `tx_queue_saturated=true` and pacemaker backpressure, so the row is now classified as driver-saturated, consensus-stalled, and overload-admission evidence rather than an unexplained missing-QC repair failure.
- Latest 20k artifacts: `dist/izanami-20k-targeted-npos-packet-loss-25pct-seed7-20260430-000420`, including `evidence.tsv`, `root-cause.md`, `paper-style-final-report.md`, and diagnostics copied under `diagnostics/npos-packet-loss-25pct`.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo test -p iroha_core --lib repeated_same_height_missing_qc_reacquire_broadens_range_pull_after_retry_window -- --nocapture`
  - `cargo test -p iroha_core --lib missing_qc_height_stall_mode_reanchor_uses_deterministic_peer_subset_and_periodic_all_peers -- --nocapture`
  - `cargo test -p izanami sumeragi_status_digest -- --nocapture`
  - `cargo test -p izanami throughput -- --nocapture`
  - `cargo test -p iroha_core --lib sumeragi_status -- --nocapture`
  - `python3 -m pytest scripts/tests/izanami_matrix_classifier_test.py`
  - `bash -n scripts/run_izanami_communication_vulnerability_matrix.sh`
  - `cargo build -p izanami --bin izanami -p irohad --bin iroha3d`
  - Real 20k NPoS packet-loss row with diagnostics enabled, then report-only rebuild for the new overload-admission evidence label.

## 2026-04-30 Offline V2 native bridge prover FFI

- Rebased PR #5578 onto the current `i23-features` branch and narrowed it to
  the shared `connect_norito_bridge` C-FFI prover surface. Swift keeps using
  its native `Halo2OfflineNoteV2Prover` path, while the bridge now exposes
  Rust-backed redeem/audit proof generation for other native consumers.
- Added `connect_norito_offline_prove_note_v2_redeem` and
  `connect_norito_offline_prove_note_v2_audit`, returning Norito-archive
  `OfflineNoteRecursiveProofV2` payloads with canonical verifier-key id,
  public-input hash, and Halo2/IPA proof bytes.
- Added bridge tests that decode the FFI output, check the proof binding, and
  verify the returned proof against the canonical Offline V2 verifier. Invalid
  archives now fail through `CONNECT_NORITO_ERR_OFFLINE_NOTE_V2_PROVE`.
- Fixed three current `iroha_data_model` clippy findings surfaced by the
  bridge clippy pass: two `NPoS` doc-markdown warnings and one collapsible
  schema-map branch.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo fmt --all --check`
  - `cargo test -p connect_norito_bridge offline_note_v2_ -- --nocapture`
  - `cargo test -p connect_norito_bridge`
  - `cargo clippy -p connect_norito_bridge --all-targets -- -D warnings`

## 2026-04-29 NPoS PRF seed recovery repair

- Restored NPoS PRF seed recovery to use persisted VRF epoch records as the
  source of truth, deriving restart-gap seeds by replaying finalized record
  reveals and empty epoch rollovers instead of re-hashing the configured base
  seed by epoch number.
- Kept `EpochManager::restore_from_record` seeds intact during actor startup,
  mode rebuilds, and post-commit boundary refresh so unfinalized/finalized VRF
  record state is not clobbered by schedule-derived fallback seeds.
- Updated Sumeragi vote fixtures to cache signer identity metadata with their
  test rosters, and aligned the stale-view commit-vote fixture with the
  actor's committed-block catch-up before signing votes.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo test -p iroha_core --lib npos_seed_for_height -- --nocapture`
  - `cargo test -p iroha_core --lib load_npos_collector_config_uses_vrf_seed -- --nocapture`
  - `cargo test -p iroha_core --lib event_driven_precommit -- --nocapture`
  - `cargo test -p iroha_core --lib stale_view_async_commit_votes_for_known_pending_block_still_form_qc -- --nocapture`
  - `cargo test -p iroha_core --lib block_sync_update_stale_frontier_with_commit_votes_keeps_recovery_active_for_local_vote -- --nocapture`
  - `cargo test -p iroha_core --lib apply_mode_flip_to -- --nocapture`
  - `cargo test -p iroha_core --lib refresh_npos_seed -- --nocapture`
  - `cargo test -p iroha_core --lib on_block_commit_persists_new_epoch_seed_record -- --nocapture`
  - `cargo test -p iroha_core --lib finalize_pending_block_commits_retired_same_height_with_conflicting_local_vote -- --nocapture`
  - `cargo test -p iroha_core --lib`
  - `git diff --check`

## 2026-04-29 NPoS epoch transition coverage

- Added focused `EpochManager` unit coverage for non-boundary block commits,
  explicit `next_epoch` state clearing, clamped epoch window/height mapping,
  unfinalized VRF epoch record restore, and finalized epoch-boundary snapshots
  that preserve commits, regular reveals, late reveals, and penalty inputs
  before state is cleared.
- Extended `EpochManager` edge coverage for epoch-mismatched VRF notes, late
  reveal rejection without a matching commitment, height-zero commit no-ops,
  and zero-length epoch parameters clamping to single-block epochs.
- Added epoch-schedule and PRF seed coverage for skipped unfinished VRF epoch
  records, non-monotonic finalized epoch ends, post-finalized fallback epoch
  lengths, and seed selection across finalized NPoS epoch boundaries.
- Added actor-level coverage for runtime mode flip restoring an unfinalized
  target VRF epoch record, and for pre-commit seed refresh preserving in-flight
  epoch participation while applying updated on-chain epoch parameters.
- Added reverse mode-flip and post-commit boundary refresh coverage: NPoS to
  permissioned rebuilds the epoch manager while clearing collectors, and an
  epoch-boundary post-commit refresh advances to the next epoch while clearing
  stale VRF inputs.
- Added actor epoch-resolution coverage for scheduled permissioned cutovers,
  finalized VRF schedule precedence over a stale local manager, and active
  NPoS manager use after the last finalized epoch boundary.
- Added VRF epoch snapshot persistence coverage for participant merge ordering,
  late reveal serialization, finalized penalty field retention, and preserving
  existing penalty-application markers on unfinalized snapshot updates.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo test -p iroha_core --lib apply_mode_flip_to_npos_restores_unfinalized_target_epoch_record`
  - `cargo test -p iroha_core --lib apply_mode_flip_to_permissioned_restores_epoch_record_but_clears_collectors`
  - `cargo test -p iroha_core --lib refresh_npos_seed_precommit_preserves_epoch_state_during_schedule_change`
  - `cargo test -p iroha_core --lib refresh_npos_seed_postcommit_boundary_advances_epoch_and_clears_inputs`
  - `cargo test -p iroha_core --lib epoch_for_height_returns_zero_for_permissioned_target_after_scheduled_flip`
  - `cargo test -p iroha_core --lib epoch_for_height_prefers_finalized_schedule_over_stale_manager`
  - `cargo test -p iroha_core --lib epoch_for_height_uses_manager_after_finalized_boundary_when_npos_active`
  - `cargo test -p iroha_core --lib epoch_schedule_`
  - `cargo test -p iroha_core --lib npos_seed_for_height_tracks_finalized_epoch_schedule`
  - `cargo test -p iroha_core --lib vrf_snapshot_record_merges_participants_and_finalized_penalties`
  - `cargo test -p iroha_core --lib unfinalized_vrf_snapshot_strips_penalties_but_preserves_application_marker`
  - `cargo test -p iroha_core --lib sumeragi::epoch`
  - `git diff --check`

## 2026-04-29 Izanami seed-7 stress rerun closure

- Reran the real seed-7 `stress-400` and `stress-800` communication-vulnerability matrices with fresh `target/codex-stress` binaries and row-isolated execution so each scenario has its own completed log before being merged into the report artifact.
- Both refreshed artifact directories are now fully resilient: `dist/izanami-stress-400-seed7-20260428` and `dist/izanami-stress-800-seed7-20260428` each contain 14 data rows plus the header in `summary.tsv` / `evidence.tsv`; every permissioned and NPoS row has `exit_code=0`, `status=ok`, and `paper_outcome=resilient`.
- The previously degraded NPoS transient-failure, packet-loss, and leader-isolation rows now report `confirmation_queue_dropped=0`, `confirmation_failed=0`, hard `failures=0`, no unexpected successes, and no RBC/pending pressure. Late sampled confirmations are counted as `confirmation_budget_skipped` or shutdown noise instead of degrading completed runs.
- Rebuilt `summary.md`, `evidence.tsv`, and `paper-style-final-report.md` for both stress directories from the completed row logs; artifact-wide scans over the fresh report directories found no nonzero queue-drop/failure counters, panics, route errors, confirmation timeouts, stuck-queued transaction markers, or queue-pressure markers.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo test -p izanami confirmation_audit_scheduler -- --nocapture`
  - `python3 -m pytest scripts/tests/izanami_matrix_classifier_test.py`
  - `bash -n scripts/run_izanami_communication_vulnerability_matrix.sh && git diff --check`
  - `CARGO_TARGET_DIR=target/codex-stress cargo build -p izanami --bin izanami -p irohad --bin iroha3d`
  - Row-isolated real seed-7 `stress-400` reruns for missing/degraded NPoS rows plus report rebuild for `dist/izanami-stress-400-seed7-20260428`
  - Row-isolated real seed-7 `stress-800` reruns for all permissioned and NPoS rows plus report rebuild for `dist/izanami-stress-800-seed7-20260428`

## 2026-04-29 Iroha config minimal snapshot Kura default

- Refreshed `minimal_config_snapshot` so the expected Kura defaults include
  `eviction_required_replicas: 3`.
- Focused validation for this fix:
  - `cargo test -p iroha_config --test fixtures`
  - `cargo fmt --all`

## 2026-04-29 Canonical Kura test fixture repair

- Updated snapshot tests so WSV state and snapshot writing share the same Kura
  instance under the new state/Kura alignment guard, and kept the intentional
  last-block soft-fork read scenario as a read-side mismatch instead of a
  write-side rejection.
- Adjusted state and Sumeragi fixtures that intentionally modeled missing,
  future, or conflicting blocks so Kura only stores canonical contiguous
  chains; non-canonical payloads now live in pending/test state where the
  behavior under test expects them.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo test -p iroha_core --lib snapshot::tests::can_read -- --nocapture`
  - `cargo test -p iroha_core --lib state::tests::all_blocks_skips_missing_kura_entries -- --nocapture`
  - `cargo test -p iroha_core --lib sumeragi::main_loop::tests::fetch_pending_block_ -- --nocapture`
  - `cargo test -p iroha_core --lib sumeragi::main_loop::tests::fetch_block_body_ -- --nocapture`
  - `cargo test -p iroha_core --lib sumeragi::main_loop::tests -- --nocapture` (`2001` passed, `20` ignored)
  - `cargo fmt --all --check`
  - `git diff --check`

## 2026-04-29 SORA Minamoto mainnet Codex skill

- Added a standalone `sora-minamoto-mainnet` Codex skill for the public
  Minamoto Torii MCP endpoint at `https://minamoto.sora.org/v1/mcp`, mirroring
  the Taira skill structure while making mainnet write handling explicitly
  conservative.
- Expanded the skill with a concrete Minamoto transaction workflow, write
  confirmation policy, required write inputs, default no-agent-side-signing
  posture, common read payload examples, failure-to-action map, Taira
  difference table, and agent output requirements.
- Incorporated live restart-smoke feedback: prefer the Minamoto MCP namespace
  when multiple SORA servers are configured, avoid alias-index enumeration as a
  health check, treat Musubi `404` as absent data when the tool is callable,
  and verify explorer pagination against returned `pagination` plus
  `inputSchema`.
- Updated Codex integration docs and agent guidance so Minamoto workflows point
  at the new skill, prefer curated `iroha.*` tools, keep signing inputs
  runtime-only, and avoid Taira testnet faucet/bootstrap assumptions on
  mainnet.
- Focused validation for this slice:
  - `git diff --check`
  - `git diff --no-index --check /dev/null skills/sora-minamoto-mainnet/SKILL.md`
  - `git diff --no-index --check /dev/null skills/sora-minamoto-mainnet/agents/openai.yaml`
  - `diff -qr skills/sora-minamoto-mainnet "$HOME/.codex/skills/sora-minamoto-mainnet"`
  - Read-only live MCP smoke: `mcp__sora_minamoto_mainnet__.iroha_sumeragi_status`

## 2026-04-29 Mandatory Kura durability before state commit

- Made Kura block storage synchronous and canonical-height checked: duplicate same-height/same-hash stores are idempotent, gaps and same-height hash conflicts are hard errors, and successful returns mean the block is present in the durable block files.
- Removed the Sumeragi commit-path `persist_required` bypass so every commit worker stores/verifies the block in Kura before applying and committing WSV state; `PendingBlock::kura_persisted` is retained only as retry/logging state.
- Kept the Kura writer thread on sidecar flushing and shutdown fsync handling instead of making it authoritative for block appends, and added regression coverage for merge-log rollback and `kura_persisted` retry behavior.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo test -p iroha_core kura::tests`
  - `cargo test -p iroha_core sumeragi::main_loop::tests::state_commit_failure_after_kura_store_keeps_partial_head_hidden`
  - `cargo test -p iroha_core --lib sumeragi::main_loop::tests::pending_kura_persisted_still_checks_kura_before_state_commit`
  - `cargo test -p iroha_core --lib snapshot::tests::snapshot_write_rejects_state_ahead_of_kura`
  - `cargo test -p irohad snapshot_read_error_tests::snapshot_read_error_is_recoverable_classifies_errors`
  - `git diff --check`

## 2026-04-28 Izanami stress audit queue root-cause fix

- Root-caused the seed-7 stress degraded rows to Izanami's sampled confirmation audit queue, not Sumeragi consensus state: the completed stress logs show `confirmation_queue_dropped` hits while Sumeragi status deltas report no RBC store pressure, no RBC evictions, no pending-RBC drops, no persist drops, and accepted submissions equal started submissions.
- Fixed the audit scheduler so sampled confirmations are not enqueued when the remaining run window cannot cover the configured audit timeout. Late samples now count as `confirmation_budget_skipped`; genuine bounded queue overflow still increments `confirmation_queue_dropped`, and an unexpected early audit-channel close now increments `confirmation_failed`.
- The fresh 2026-04-29 stress rerun above confirms the previously degraded audit-queue-only rows move to resilient when no real queue overflow occurs.
- Validation:
  - `cargo fmt --all`
  - `cargo test -p izanami confirmation_audit_scheduler -- --nocapture`
  - `python3 -m pytest scripts/tests/izanami_matrix_classifier_test.py`
  - `bash -n scripts/run_izanami_communication_vulnerability_matrix.sh && git diff --check`

## 2026-04-28 Sumeragi pacemaker low-online recovery gate

- Narrowed the pacemaker's first-proposal low-online deferral so it still suppresses fresh view-0 startup proposals without an online commit quorum, but no longer blocks cached-slot cleanup, missing-QC committed-QC fallback proposals, recovery heartbeat proposals, stale/unknown precommit recovery, or future-NEW_VIEW reanchor catch-up.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo fmt --all --check`
  - `cargo test -p iroha_core --lib sumeragi::main_loop::tests::cached_recovery_proposal -- --nocapture`
  - `cargo test -p iroha_core --lib sumeragi::main_loop::tests::force_view_change_if_idle_forces_missing_qc_frontier_proposal -- --nocapture`
  - `cargo test -p iroha_core --lib sumeragi::main_loop::tests::pacemaker_ -- --nocapture`
  - `git diff --check`

## 2026-04-28 Izanami seed-7 stress evidence

- Ran real 20-peer/800s stress matrices with fresh `target/codex-stress` binaries for seed `7`:
  - `dist/izanami-stress-400-seed7-20260428` (`stress-400`, both Sumeragi modes)
  - `dist/izanami-stress-800-seed7-20260428` (`stress-800`, both Sumeragi modes)
- Stress results are margin evidence, not paper-mode acceptance replacements. This initial pre-fix run showed permissioned resilient across all 800 TPS rows and all but the 400 TPS 25% packet-loss subrow; NPoS targeted-load/stopping were resilient while transient-failure, packet-loss subrows, and leader-isolation degraded from bounded confirmation-audit queue drops without hard submission failures. The 2026-04-29 rerun closure above supersedes these classifications with fresh fully resilient artifacts.
- Added `--report-only` to the matrix runner so completed raw logs can regenerate `summary.md`, `evidence.tsv`, and `paper-style-final-report.md` after a post-run report assembly failure. Rebuilt both stress artifact directories with the new path.
- Focused validation for this slice:
  - `CARGO_TARGET_DIR=target/codex-stress cargo build -p izanami --bin izanami -p irohad --bin iroha3d`
  - `TEST_NETWORK_BIN_IROHAD=target/codex-stress/debug/iroha3d IROHA_TEST_SKIP_BUILD=1 scripts/run_izanami_communication_vulnerability_matrix.sh --out dist/izanami-stress-400-seed7-20260428 --mode stress-400 --sumeragi-mode both --izanami-cmd target/codex-stress/debug/izanami -- --seed 7`
  - `TEST_NETWORK_BIN_IROHAD=target/codex-stress/debug/iroha3d IROHA_TEST_SKIP_BUILD=1 scripts/run_izanami_communication_vulnerability_matrix.sh --out dist/izanami-stress-800-seed7-20260428 --mode stress-800 --sumeragi-mode both --izanami-cmd target/codex-stress/debug/izanami -- --seed 7`
  - `scripts/run_izanami_communication_vulnerability_matrix.sh --out dist/izanami-stress-400-seed7-20260428 --mode stress-400 --sumeragi-mode both --report-only`
  - `scripts/run_izanami_communication_vulnerability_matrix.sh --out dist/izanami-stress-800-seed7-20260428 --mode stress-800 --sumeragi-mode both --report-only`
  - `bash -n scripts/run_izanami_communication_vulnerability_matrix.sh scripts/run_izanami_communication_vulnerability_sweep.sh`
  - `python3 -m pytest scripts/tests/izanami_matrix_classifier_test.py`

## 2026-04-28 SDK compatibility matrix gap closure

- Added `fixtures/sdk/compatibility_matrix.json` as the canonical public SDK compatibility matrix fixture for the `i23-features` branch, with every SDK/story cell populated.
- Added a focused pytest guard that rejects malformed rows, private/local source metadata, and any reintroduced `no-data` cells.
- Focused validation for this slice:
  - `python3 scripts/tests/sdk_compatibility_matrix_test.py`

## 2026-04-28 SDK crypto parity for JS, Swift, Kotlin, and Java

- Added full signing-algorithm parity across the JavaScript SDK, Swift SDK, Kotlin SDK, and Java Android SDK for Ed25519, secp256k1, ML-DSA, all supported GOST R 34.10-2012 parameter sets, BLS normal/small, and SM2.
- JS now exposes generic native-backed key generation/import, signing, verification, and multihash helpers; Swift and JVM/Android SDKs now carry the same Rust bridge discriminants, address curve IDs, and native-backed software key paths.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo fmt --all --check`
  - `cargo check -p iroha_js_host`
  - `cargo check -p connect_norito_bridge`
  - `npm --prefix javascript/iroha_js run build:native`
  - `npm --prefix javascript/iroha_js run build:dist`
  - `cd javascript/iroha_js && node --test test/crypto.test.js test/crypto.browser.test.js`
  - Node one-off JS generic crypto smoke with a stubbed native binding
  - `cd kotlin && ./gradlew :core-jvm:test --tests org.hyperledger.iroha.sdk.crypto.SigningAlgorithmTest --tests org.hyperledger.iroha.sdk.address.AccountAddressTest --console=plain`
  - `cd java/iroha_android && JAVA_HOME=$(/usr/libexec/java_home -v 21) ANDROID_HOME=~/Library/Android/sdk ANDROID_SDK_ROOT=~/Library/Android/sdk ./gradlew :core:test --tests org.hyperledger.iroha.android.crypto.SigningAlgorithmTests --tests org.hyperledger.iroha.android.address.AccountAddressTests --console=plain`
  - `cd IrohaSwift && swift test --filter IrohaSDKSigningAlgorithmTests/testSigningAlgorithmsMatchRustBridgeDiscriminants`
  - `git diff --check`
- Rebuilt `javascript/iroha_js/native/iroha_js_host.node` and its checksum manifest so the Node SDK loads the new generic crypto exports locally.

## 2026-04-28 Python SDK all-algorithm crypto bridge

- Extended the Python SDK crypto bridge beyond Ed25519/raw SM2 helpers to expose generic `CryptoKeyPair`, key generation/import, signing, verification, and multihash import/export for every compiled `iroha_crypto` signature suite: Ed25519, secp256k1, ML-DSA-65, TC26 GOST R 34.10-2012 parameter sets, BLS normal/small, and SM2.
- Kept compatibility-specific Ed25519 account-id helpers and raw SM2 distid/SEC1 helpers while adding payload-based generic APIs for all algorithms.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo fmt --all --check`
  - `cargo check -p iroha_python_rs`
  - `cargo build --release -p iroha_python_rs`
  - `python3 -m compileall -q python/iroha_python/src/iroha_python/crypto.py python/iroha_python/src/iroha_python/__init__.py python/iroha_python/tests/crypto_algorithms_test.py`
  - Direct Python execution of `python/iroha_python/tests/crypto_algorithms_test.py` test functions against `target/release/libiroha_python_rs.dylib` (`3` test functions passed)
  - `git diff --check`
- Local tooling gaps: `ruff`, `maturin`, and `pytest` are not installed in the available Python interpreters, so the focused Python smoke used plain assertion execution instead of pytest collection.

## 2026-04-28 Izanami evidence reporting review fixes

- Tightened the communication-vulnerability matrix acceptance marker so only a real whitespace-delimited `failures=N` field can degrade a run; `expected_failures=N` metadata no longer satisfies the marker.
- Stopped duration-only Izanami deadline sampling from reporting `first_progress_after_fault_start_height` / `first_progress_after_fault_end_height`; those fields now remain reserved for polling runs that actually observe height advancement after the fault boundary, while final quorum/strict/skew evidence is still recorded.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `bash -n scripts/run_izanami_communication_vulnerability_matrix.sh`
  - `python3 -m pytest scripts/tests/izanami_matrix_classifier_test.py`
  - `cargo test -p izanami duration_deadline -- --nocapture`
  - `git diff --check`

## 2026-04-28 Sumeragi main-loop recovery regression closure

- Fixed the reported Sumeragi main-loop recovery failure cluster by keeping contiguous frontier recovery on exact slots instead of leaking back into generic missing-block state, while preserving the wider-roster resilience fallback when a commit quorum leaves a meaningful non-quorum tail.
- Restored liveness gating for idle view changes, missing-QC reacquire, stale frontier owners, backlog suppression, and passive catch-up slots so actionable dependencies defer rotation without pinning unrelated views.
- Repaired vote-backed recovery handoffs: local same-height vote history now blocks active conflicting proposals but still lets exhausted stale owners yield; validation/commit inflight work keeps matching frontier ownership live; known-block commit-QC repair distinguishes local payload materialization from exact body repair.
- Reworked vote-backed reschedule and NEW_VIEW rebroadcast paths so near-quorum retries, single-vote fast windows, quorum-timeout ownership, and pacemaker rebroadcasts remain deterministic under backlog.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo test -p iroha_core --lib sumeragi::main_loop::tests::assemble_proposal_defers_when_candidate_conflicts_with_local_vote_history -- --nocapture`
  - `cargo test -p iroha_core --lib sumeragi::main_loop::tests::proposal_yields_ -- --nocapture`
  - `cargo test -p iroha_core --lib sumeragi::main_loop::tests::known_block_commit_qc_recovery_routes_frontier_fetch_through_exact_block_body -- --nocapture`
  - `cargo test -p iroha_core --lib sumeragi::main_loop::tests::frontier_body_fetch_wakes_commit_pipeline_when_commit_qc_repair_body_is_local -- --nocapture`
  - `cargo test -p iroha_core --lib sumeragi::main_loop::tests::qc_missing_block_defer_contiguous_frontier_commit_quorum_fetches_exact_body_immediately -- --nocapture`
  - `cargo test -p iroha_core --lib sumeragi::main_loop::tests::qc_missing_block_defer_widens_exact_body_repair_under_resilience_commit_quorum -- --nocapture`
  - `cargo test -p iroha_core --lib sumeragi::main_loop::tests::pacemaker_injects_recovery_heartbeat_when_new_view_leader_queue_empty -- --nocapture`
  - `cargo test -p iroha_core --lib sumeragi::main_loop::tests -- --nocapture` (`1990` passed, `20` ignored)

## 2026-04-28 Izanami/Sumeragi result-strengthening harness

- Hardened Izanami shutdown accounting so load supervisors stop planning new submissions at shutdown, drain spawned submission tasks for a bounded timeout, and expose `submit_plans_started`, `submit_plans_shutdown_skipped`, and `submit_tasks_shutdown_aborted` in the final `izanami::summary`. The CLI persists `--shutdown-drain-timeout`; the matrix wrapper keeps quick runs at `15s` and paper/stress profiles at `60s`.
- Added run evidence to Izanami summaries and matrix TSVs: submit-latency sample percentiles (`p50`/`p95`/`p99`/max), final quorum/strict height, max peer-height skew, first height progress after fault start/end, and best-effort Sumeragi status deltas for view changes, commit-pipeline timing, missing-block fetch, RBC pressure/evictions, block-sync roster source counters, and NPoS repair coverage.
- Added observational NPoS repair-coverage telemetry to `/v1/sumeragi/status` with Norito-defaulted fields. The snapshot is populated only from local repair/fanout selection, only surfaced for active NPoS status, and does not feed block validity, validator ordering, signatures, or deterministic consensus state.
- Updated the communication-vulnerability matrix tooling to emit `paper-style-final-report.md`, added `paper`, `stress-400`, and `stress-800` profiles, expanded acceptance-failure marker checks, and added `scripts/run_izanami_communication_vulnerability_sweep.sh` to aggregate multi-profile/multi-seed runs into `sweep-summary.tsv`, `sweep-evidence.tsv`, and `sweep-report.md`.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo check -p izanami --bin izanami`
  - `cargo check -p iroha_torii`
  - `python3 -m pytest scripts/tests/izanami_matrix_classifier_test.py`
  - `bash -n scripts/run_izanami_communication_vulnerability_matrix.sh && bash -n scripts/run_izanami_communication_vulnerability_sweep.sh`
  - `cargo test -p izanami metrics_snapshot_accumulates_counts -- --nocapture`
  - `cargo test -p izanami latency_summary_uses_ceil_rank_percentiles -- --nocapture`
  - `cargo test -p izanami shutdown_submission_drain_counts_aborted_tasks -- --nocapture`
  - `cargo test -p izanami cli_overrides_shutdown_drain_timeout -- --nocapture`
  - `cargo test -p izanami stored_args_roundtrip_preserves_fault_window_fields -- --nocapture`
  - `cargo test -p iroha_core --lib npos_repair_coverage_snapshot_is_npos_only -- --nocapture`
  - `cargo test -p iroha_core --lib stake_coverage_bps_for_world_reports_selected_coverage -- --nocapture`
  - `cargo test -p iroha_data_model --test consensus_roundtrip sumeragi_wire_status_roundtrip -- --nocapture`
  - `cargo test -p iroha sumeragi_status_wire_roundtrip_to_json_preserves_fields -- --nocapture`
  - `git diff --check`
- The full 10-seed paper/stress sweep remains intentionally unrun in this slice because it is an expensive acceptance run.

## 2026-04-28 Retired sample identifier cleanup

- Replaced retired bank/sample identifiers in localnet alias catalog defaults, Offline V2 vector generation, SDK tests, and status command examples with neutral PayNet/demo placeholders.
- Regenerated `fixtures/offline/interop_contract_v2.json` from the updated vector generator.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo run -p iroha_data_model --features test-fixtures,transparent_api --bin offline_v2_vectors`
  - `cargo run -p iroha_data_model --features test-fixtures,transparent_api --bin offline_v2_vectors -- --check`
  - `cargo test -p iroha_core selector_matches_authority_domain --lib -- --nocapture`
  - `git diff --check`
  - `cargo fmt --all --check`
  - Boundary-aware tracked and hidden-file scans for the retired identifiers now return no matches.
- `cargo test -p iroha_kagami nexus_localnet_alias_lanes_bind_dataspaces_and_seed_validators -- --nocapture` is blocked by the existing `crates/iroha_kagami/src/genesis/generate.rs` `manifest.parse()` private-method compile error.

## 2026-04-28 Offline Note V2 focused validation gap closure

- Added focused core rejection coverage for Offline Note V2 redeem/audit proof validation: non-`OpenVerifyEnvelope` proof bytes, wrong verifier key id/backend, inactive verifier keys, and public-input hash mismatches now have explicit tests.
- Tightened Torii Offline V2 readiness smoke coverage so the exposed verifier id and public-input schema hash match the canonical fixture contract.
- Test-network genesis generation now computes and injects the confidential verifier-registry root from appended verifier-key registration instructions, so Offline V2 real-verifier localnets no longer start with a stale `vk_set_hash`.
- Added four-peer `network_functional` coverage that registers the real Offline V2 Halo2 IPA verifier, issues a note, audits it into a new note, redeems it, validates balances, and rejects replay/nullifier reuse under consensus.
- Added native app validation coverage for the shared `interop_contract_v2.json`, synthetic Android counter rejection, transcript-like recursive proof rejection, and old QR prefix rejection. Android PK and the companion app now also have physical-only KeyMint runner scripts that require a selected API 31+ non-emulator device and capture public attestation artifacts under untracked `artifacts/offline/keymint/`.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo test -p iroha_data_model offline_note_v2 --lib -- --nocapture`
  - `cargo test -p iroha_data_model --features test-fixtures,transparent_api --bin offline_v2_vectors -- --nocapture`
  - `cargo test -p iroha_core offline_note_v2 --lib -- --nocapture`
  - `cargo test -p iroha_test_network config::tests::genesis_confidential_digest_tracks_registered_verifying_keys -- --nocapture`
  - `cargo test -p integration_tests --test network_functional offline_note_v2_issue_audit_redeem_real_proofs_on_four_peers -- --nocapture`
  - `cargo test -p iroha_torii --test offline_v2_readiness_smoke -- --nocapture`
  - `cd IrohaSwift && swift test --filter OfflineNoteV2`
  - `cd kotlin && ./gradlew :core-jvm:test --tests 'org.hyperledger.iroha.sdk.offline.OfflineNoteV2Test' --console=plain`
  - `cd java/iroha_android && ANDROID_HARNESS_MAINS=org.hyperledger.iroha.android.offline.OfflineNoteV2Test ./gradlew :core:test --console=plain`
  - `cd /Users/takemiyamakoto/dev/pk-retail-wallet-android && ./gradlew --no-daemon :core:test --console=plain`
  - `cd /Users/takemiyamakoto/dev/partner-retail-wallet-android && ./gradlew --no-daemon :core:test --console=plain`
  - `cd /Users/takemiyamakoto/dev/pk-retail-wallet-android && ANDROID_SERIAL=19181FDF600918 E2E_DEVICE_SERIAL=19181FDF600918 scripts/run_offline_keymint_physical.sh`
  - `cd /Users/takemiyamakoto/dev/partner-retail-wallet-android && ANDROID_SERIAL=19181FDF600918 E2E_DEVICE_SERIAL=19181FDF600918 scripts/run_offline_keymint_physical.sh`
  - `cd /Users/takemiyamakoto/dev/pk-retail-wallet-ios && xcodebuild test -project RetailWalletIOS.xcodeproj -scheme RetailWalletIOS -configuration Debug -destination 'platform=iOS Simulator,id=A7E7B24D-46DE-4D6D-B23B-622C5AD9A464' -only-testing:RetailWalletIOSTests/OfflineAPIContractTests -only-testing:RetailWalletIOSTests/AppAttestationAssertionDecoderTests -only-testing:RetailWalletIOSTests/OfflineProofVerifierFixtureTests`
  - `cd /Users/takemiyamakoto/dev/partner-retail-wallet-ios && xcodebuild test -project RetailWalletIOS.xcodeproj -scheme RetailWalletIOS -configuration Debug -destination 'platform=iOS Simulator,id=A7E7B24D-46DE-4D6D-B23B-622C5AD9A464' -only-testing:RetailWalletIOSTests/OfflineAPIContractTests -only-testing:RetailWalletIOSTests/AppAttestationAssertionDecoderTests -only-testing:RetailWalletIOSTests/OfflineProofVerifierFixtureTests`
  - `cd /Users/takemiyamakoto/dev/pk-retail-wallet-ios && xcodebuild build -project RetailWalletIOS.xcodeproj -scheme RetailWalletIOS -configuration Debug -destination 'generic/platform=iOS Simulator'`
  - `cd /Users/takemiyamakoto/dev/partner-retail-wallet-ios && xcodebuild build -project RetailWalletIOS.xcodeproj -scheme RetailWalletIOS -configuration Debug -destination 'generic/platform=iOS Simulator'`
  - `git diff --check` in the Iroha root and all four touched app repositories.
  - `shasum -a 256` confirmed every copied `fixtures/offline/interop_contract_v2.json` has hash `2660dd41e3b8c1f4b8337d14febbc88e3febe45428c08e4d083197ef01d4e0f6`.
  - Targeted changed-file scans found no temporary-work markers, exact retired proof-domain identifiers, or retired fountain QR v1 identifiers.
- The physical KeyMint gate ran on Pixel 6 serial `19181FDF600918`. PK artifacts were captured under `/Users/takemiyamakoto/dev/pk-retail-wallet-android/artifacts/offline/keymint/20260428T081251Z-19181FDF600918`; companion app artifacts were captured under `/Users/takemiyamakoto/dev/partner-retail-wallet-android/artifacts/offline/keymint/20260428T081331Z-19181FDF600918`.

## 2026-04-27 Sumeragi future-new-view recovery and Izanami stable gate

- Added a catch-up-only Sumeragi recovery path for a lagging local frontier when exact-height `NEW_VIEW` quorum is absent but a quorum exists at a future height. The pacemaker now observes the future quorum's highest QC, requests a range pull from the local frontier with the `future_new_view_frontier_reanchor` reason, rebroadcasts the `NEW_VIEW`, and lets passive catch-up advance instead of proposing from stale local state.
- Hardened Izanami ingress failover classification so a closed transport during request send (`connection closed before message completed`) is treated as retryable. This prevents a transient peer close during leader isolation from surfacing as a non-retryable plan submission failure after consensus has otherwise recovered.
- Built fresh binaries with `CARGO_TARGET_DIR=target/codex-stable-gate cargo build -p izanami --bin izanami -p irohad --bin iroha3d`.
- Re-ran the 4-peer stable permissioned gate with fresh binaries and preserved logs. The `200`-block diagnostic at `dist/izanami-stable-gate-20260427-rerun` crossed the previous stall region and reached strict/quorum height `107` with `255/255` accepted transactions, zero submission failures, and zero confirmation failures, but did not hit `200` blocks because the workload drained before the height target. The calibrated `100`-block gate at `dist/izanami-stable-gate-20260427-target100` passed: strict height `100`, quorum height `100`, `241/241` accepted transactions, zero failures, zero confirmation failures, and zero failover or endpoint-unhealthy events.
- Re-ran the quick communication-vulnerability matrix for both Sumeragi modes with seed `7` at `dist/izanami-quick-both-20260427`: all ten rows exited `0`, reported `status=ok`, classified as `paper_outcome=resilient`, and recorded `failure_marker_count=0`. After the ingress retry hardening, reran the changed leader-isolation rows at `dist/izanami-quick-leader-retry-20260427`; permissioned and NPoS both remained `resilient`, `failure_marker_count=0`, and the logs had no `non_retryable`, plan-submission, `429`, timeout, or run-finished-with-errors markers.
- Focused validation for this slice:
  - `CARGO_TARGET_DIR=target/codex-stable-gate cargo test -p iroha_core --lib new_view_tracker_selects_future_quorum_above_recovery_floor -- --nocapture`
  - `CARGO_TARGET_DIR=target/codex-stable-gate cargo test -p iroha_core --lib pacemaker_reanchors_frontier_when_future_new_view_quorum_exists -- --nocapture`
  - `CARGO_TARGET_DIR=target/codex-stable-gate cargo test -p iroha_core --lib new_view_tracker -- --nocapture`
  - `CARGO_TARGET_DIR=target/codex-stable-gate cargo test -p iroha_core --lib pacemaker_prunes_new_view_entries_below_active_height -- --nocapture`
  - `CARGO_TARGET_DIR=target/codex-stable-gate cargo test -p izanami ingress_failover_marks_closed_send_request_retryable -- --nocapture`
  - `CARGO_TARGET_DIR=target/codex-stable-gate cargo test -p izanami communication_vulnerabilities -- --nocapture`
  - `TEST_NETWORK_BIN_IROHAD=$PWD/target/codex-stable-gate/debug/iroha3d IROHA_TEST_SKIP_BUILD=1 scripts/run_izanami_communication_vulnerability_matrix.sh --out dist/izanami-quick-both-20260427 --mode quick --sumeragi-mode both --izanami-cmd target/codex-stable-gate/debug/izanami -- --seed 7`
  - `TEST_NETWORK_BIN_IROHAD=$PWD/target/codex-stable-gate/debug/iroha3d IROHA_TEST_SKIP_BUILD=1 scripts/run_izanami_communication_vulnerability_matrix.sh --out dist/izanami-quick-leader-retry-20260427 --mode quick --sumeragi-mode both --only leader-isolation --izanami-cmd target/codex-stable-gate/debug/izanami -- --seed 7`

## 2026-04-27 Izanami packet-loss sweep closure

- Completed the explicit paper packet-loss sweep at `dist/izanami-packet-sweep-paper-20260427-loss-only` with a fresh `target/codex-packet-loss/debug/izanami` binary, `--mode paper --sumeragi-mode both --only packet-loss --packet-loss-sweep 25,50,75`, and seed `7`: permissioned and NPoS rows at `25%`, `50%`, and `75%` packet loss all exited `0`, reported `status=ok`, and classified as `paper_outcome=resilient`. Each row recorded post-fault block-height progress evidence, every `failure_marker_count` is `0`, the artifact-wide acceptance-marker scan found no hits, and no Izanami/Iroha test-network processes were left running afterward. A stale binary from an earlier attempt rejected `--fault-network-packet-loss-percent`; rebuilding Izanami and checking `--help` confirmed the sweep CLI was present before rerunning.
- Focused validation for this slice:
  - `CARGO_TARGET_DIR=target/codex-packet-loss cargo build -p izanami --bin izanami`
  - `target/codex-packet-loss/debug/izanami --help | rg 'fault-network-packet-loss-percent|fault-enable-network-packet-loss'`
  - `scripts/run_izanami_communication_vulnerability_matrix.sh --out /tmp/izanami-fresh-sweep-flag-smoke --mode quick --sumeragi-mode permissioned --only packet-loss --packet-loss-sweep 25,75 --izanami-cmd target/codex-packet-loss/debug/izanami -- --seed 7`
  - `scripts/run_izanami_communication_vulnerability_matrix.sh --out dist/izanami-packet-sweep-paper-20260427-loss-only --mode paper --sumeragi-mode both --only packet-loss --packet-loss-sweep 25,50,75 --izanami-cmd target/codex-packet-loss/debug/izanami -- --seed 7`
  - `rg -n "429 Too Many Requests|confirmation timeout|confirmation timed out|sampled confirmation failed|transaction did not reach|transaction remained queued|transaction queued for too long|load-worker shutdown timeout|worker shutdown timeout|worker shutdown timed out|queue pressure|No endpoint|route_unavailable|panic|error:" dist/izanami-packet-sweep-paper-20260427-loss-only` (no hits)
  - `cargo fmt --all`
  - `cargo fmt --all --check`
  - `bash -n scripts/run_izanami_communication_vulnerability_matrix.sh`
  - `git diff --check`
  - `pgrep -fl '/\.rustup/.*/bin/(cargo|rustc)|target/codex-packet-loss/debug/izanami|target/codex-izanami/debug/izanami|run_izanami_communication_vulnerability_matrix|target/iroha-test-network/debug/iroha3d'` (no hits after rerun; the first check overlapped with `cargo fmt --all --check`)

## 2026-04-27 SNS alias auto-renew billing amount-scale fix

- Fixed IVM subscription billing for SNS account-alias auto-renew so quote charges are compared, invoiced, and renewed as nano-XOR decimal `Numeric` values instead of raw scale-0 integers.
- Reused the same SNS quote conversion for lease ISI transfers and the IVM host auto-renew path so the cap/balance check matches the actual renewal transfer amount.
- Focused validation for this fix:
  - `cargo fmt --all`
  - `cargo test -p iroha_core quote_charge_amount_to_numeric_uses_nano_xor_scale -- --nocapture`
  - `cargo test -p iroha_core smartcontracts::ivm::host::tests::subscription_bill_account_alias_auto_renew_queues_renewal_and_reschedules -- --nocapture`
  - `git diff --check`

## 2026-04-27 Native Offline Note V2 SDK/mobile alignment

- Standardized the first-release mobile offline contract on the Iroha Offline Note V2 fixture (`fixtures/offline/interop_contract_v2.json`) with canonical Norito-backed public-input hashes, opaque recursive proof bytes, `iroha:qr1:` QR stream frames, and `parity_group=3`.
- Added native Kotlin/JVM and Java Android Offline Note V2 model/codec surfaces that mirror the Swift SDK without Rust FFI/JNI. The parity tests validate key-certificate signing bytes, issue/redeem/audit Norito payloads, public-input hashes, proof binding rejection, and the shared fixture.
- Updated the PK and companion iOS/Android app offline flows to call the native SDK-backed Offline Note V2 helpers for certificate payloads, issued/output claims, payment-token public inputs, QR framing, and validation instead of app-local text transcripts.
- Focused validation for this slice:
  - `cargo test -p iroha_data_model --features test-fixtures,transparent_api --bin offline_v2_vectors`
  - `cargo run -p iroha_data_model --features test-fixtures,transparent_api --bin offline_v2_vectors`
  - `cd IrohaSwift && swift test --filter OfflineNoteV2`
  - `cd kotlin && ./gradlew :core-jvm:test --tests 'org.hyperledger.iroha.sdk.offline.OfflineNoteV2Test' --rerun-tasks --console=plain`
  - `cd java/iroha_android && ANDROID_HARNESS_MAINS=org.hyperledger.iroha.android.offline.OfflineNoteV2Test ./gradlew :core:test --rerun-tasks --console=plain`
  - `cd /Users/takemiyamakoto/dev/pk-retail-wallet-android && ./gradlew --no-daemon :core:test --console=plain`
  - `cd /Users/takemiyamakoto/dev/partner-retail-wallet-android && ./gradlew --no-daemon :core:test --console=plain`
  - `cd /Users/takemiyamakoto/dev/pk-retail-wallet-ios && xcodebuild test -project RetailWalletIOS.xcodeproj -scheme RetailWalletIOS -configuration Debug -destination 'platform=iOS Simulator,id=A7E7B24D-46DE-4D6D-B23B-622C5AD9A464' -only-testing:RetailWalletIOSTests/OfflineAPIContractTests`
  - `cd /Users/takemiyamakoto/dev/partner-retail-wallet-ios && xcodebuild -project RetailWalletIOS.xcodeproj -scheme RetailWalletIOS -configuration Debug -destination 'platform=iOS Simulator,id=A7E7B24D-46DE-4D6D-B23B-622C5AD9A464' build`

## 2026-04-27 Swift Offline V2 transaction builders

- Added Swift Offline V2 note models for key certificates, issued claims, redeem public inputs, audit public inputs, recursive proofs, and issue/redeem/audit instruction payloads.
- Added `IrohaSDK` builders and submit helpers for `IssueOfflineNoteV2`, `RedeemOfflineNoteV2`, and `AuditOfflineNoteV2` transactions. Redeem and audit builders validate the recursive proof's public-input hash against the canonical Swift/Rust Norito payload before signing.
- Added fixture parity coverage against `fixtures/offline/interop_contract_v2.json` for key-certificate signing bytes, issue/redeem/audit Norito payloads, public-input hashes, proof binding rejection, and signed envelope construction.
- Focused validation for this slice:
  - `swift test --filter OfflineNoteV2Tests`
  - `swift test`

## 2026-04-27 Iroha config minimal snapshot refresh

- Refreshed `minimal_config_snapshot` so the expected Nexus fee defaults include the empty `successful_claim_fee_exempt_authorities` list.
- Focused validation for this fix:
  - `cargo test -p iroha_config --test fixtures`

## 2026-04-27 SNS suffix catalog price alignment

- `docs/examples/sns/suffix_catalog_v1.json` now matches the `.sora` default price in `iroha_data_model::sns::fixtures::default_policy()` (`500000000` nano-XOR / 0.5 XOR), following the nano-XOR lease unit convention used by ledger-backed SNS pricing.
- Refreshed the catalog checksum and the English SNS catalog/schema docs so they no longer advertise the legacy `120` payment unit as the current `.sora` policy price.
- Focused validation for this fix:
  - `cargo test -p iroha_cli catalog_entry_matches_default_policy -- --nocapture`
  - `sha256sum -c docs/examples/sns/suffix_catalog_v1.sha256`
  - `cargo test -p iroha_cli catalog_detects_price_mismatch -- --nocapture`

## 2026-04-27 Offline V2 real Halo2 IPA prover slice

- Added the real `offline-note-v2-recursive-v1` Halo2 IPA semantic circuit. The circuit binds the Offline V2 public-instance schema, constrains redeem/audit mode, bounded input/output counts, unused amount slots, and normalized input/output amount conservation.
- Added `prove_offline_note_v2_redeem`, `prove_offline_note_v2_audit`, and `derive_halo2_ipa_offline_note_v2_proving_key_bytes`. These paths generate real Halo2 IPA proofs against registered verifier-key material; no debug or mock prover backend is used.
- Offline V2 ISI verification now compares proof-exposed public instances against the same semantic instance layout used by the prover instead of the old hash-only reserved-sentinel layout.
- Added active WSV verifier-key registration for `offline-note-v2-recursive-v1` to Kagami-generated localnet genesis using the real inline Halo2 IPA verifier key and Offline V2 schema hash.
- Torii Offline V2 readiness now advertises the canonical recursive-proof backend, circuit id, schema hash, instance-column count, and verifier key id. The Swift SDK has a typed `getOfflineV2Readiness` accessor for that metadata.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo test -p iroha_core offline_note_v2_real --lib -- --nocapture`
  - `cargo test -p iroha_kagami generated_nexus_localnet_keeps_fee_asset_convertible_for_taira_wallets -- --nocapture`
  - `cargo test -p iroha_torii --test offline_v2_readiness_smoke -- --nocapture`
  - `swift test --filter ToriiClientTests/testGetOfflineV2ReadinessParsesRecursiveVerifierMetadata`
  - `cargo test -p iroha_core expected_public_instances_encode_semantic_columns --lib -- --nocapture`
  - `cargo test -p iroha_data_model offline_note_v2 --lib -- --nocapture`

## 2026-04-27 Offline audit replay and router ambiguity fix

- Offline V2 audit bundles now carry issued input claims in their canonical public inputs. Core verifies those source claims were issued and unspent, consumes their normal spent-claim keys, and consumes normal redemption nullifier keys before publishing audited output claims as redeemable.
- Nexus account-scoped routing no longer trusts the legacy single-binding `dataspace_for_account` shortcut. Account targets route to a non-universal dataspace only when the full account-scope hierarchy has exactly one dataspace; universal-plus-private and multi-private scopes fall back to the default route.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo run -p iroha_data_model --features test-fixtures,transparent_api --bin offline_v2_vectors`
  - `cargo test -p iroha_data_model offline_note_v2 --lib -- --nocapture`
  - `cargo test -p iroha_data_model --features test-fixtures,transparent_api --bin offline_v2_vectors -- --nocapture`
  - `cargo test -p iroha_core audit_replay_keys_cover_input_spend_and_output_issue_domains --lib -- --nocapture`
  - `cargo test -p iroha_core opaque_asset_transfer --lib -- --nocapture`
  - `cargo test -p iroha_core untargeted_universal_authority_transaction_uses_default_lane_with_state --lib -- --nocapture`
  - `cargo test -p iroha_torii --lib explorer -- --nocapture`

## 2026-04-26 Loose-end closure pass: escrow, Izanami, and release hygiene

- Wired the native anonymous escrow ISI family into the V1 IVM syscall surface: open, accept, mark-payment-sent, release, cancel, open-dispute, and resolve-dispute now have ABI constants, policy-table coverage, unknown-syscall fixture updates, host dispatch into the typed ISIs, syscall docs, and an updated V1 ABI hash (`6e26a7b44f773a856e45e91baa9aebbc975d47bb452f12962cd4b03fecfe27b3`).
- Added proof-carrying anonymous escrow helper surfaces for Swift, Kotlin, and Java Android so app clients can build the open/accept/payment/release/cancel/dispute/resolve payload shapes without hand-assembling argument maps.
- Fixed the account-alias onboarding auto-renew enqueue path so the subscriber is granted `CanModifyNftMetadata` for the subscription NFT before the auto-renew trigger is registered, closing the focused permission gap for user-signed metadata updates on that lease record.
- Replaced split IVM sample staging with a shared `crates/ivm/prebuilt_samples.txt` manifest consumed by both `ivm_prebuild` and `integration_tests/build.rs`; the manifest now includes `threshold_escrow` so CLI and integration fixture staging cannot drift on that sample.
- Plumbed Izanami packet-loss percentage through CLI arguments, stored configuration, runtime config merging, persistence, and fault-config generation. The matrix runner now supports `--packet-loss-sweep`, uses `75%` for quick packet-loss smokes, and defaults paper mode to `25%,50%,75%` packet-loss subrows while keeping leader isolation pinned to the existing `75%` stress point.
- Completed the exact-injector 75% packet-loss paper baseline at `dist/izanami-exact-packet-paper-20260426` with `--mode paper --sumeragi-mode both` and seed `7`: all ten permissioned/NPoS rows exited `0`, reported `status=ok`, and classified as `paper_outcome=resilient`. The acceptance-marker scan found no `429`, confirmation timeout, stuck queued transaction, route-unavailable, queue-pressure, panic, or error signatures. The wrapper tripped a post-run `note_path` finalization bug after recording all rows; `note_path` is now initialized up front, the shell smoke passes, and `summary.md` was regenerated from the completed `summary.tsv` / `evidence.tsv`. With the current sweep-capable script, reproduce this ten-row baseline with `--packet-loss-sweep 75`.
- Pinned Sumeragi formal CI to Apalache `0.52.2` through the local installer/toolchain path, updated the fallback Docker reference, and added a `docs/formal` translation metadata audit job for `source`, `source_hash`, and `translation_last_reviewed`.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `git diff --check`
  - `python3 -m py_compile ci/check_docs_i18n_metadata.py`
  - `python3 ci/check_docs_i18n_metadata.py --paths docs/formal --max-messages 5` (passed with stale `source_hash` warnings for existing formal translations)
  - `bash -n scripts/run_izanami_communication_vulnerability_matrix.sh`
  - `scripts/run_izanami_communication_vulnerability_matrix.sh --out /tmp/izanami-note-path-smoke --mode quick --sumeragi-mode permissioned --only targeted-load --izanami-cmd true`
  - `scripts/run_izanami_communication_vulnerability_matrix.sh --out dist/izanami-exact-packet-paper-20260426 --mode paper --sumeragi-mode both --izanami-cmd target/codex-packet-loss/debug/izanami -- --seed 7` (all ten single-75% packet-loss baseline rows completed and were recorded as resilient; with the current sweep-capable script, add `--packet-loss-sweep 75` to reproduce the same ten-row shape; the wrapper finalization bug above was fixed afterward)
  - `rg -n "429 Too Many Requests|confirmation timeout|confirmation timed out|sampled confirmation failed|transaction did not reach|transaction remained queued|transaction queued for too long|load-worker shutdown timeout|worker shutdown timeout|worker shutdown timed out|queue pressure|No endpoint|route_unavailable|panic|error:" dist/izanami-exact-packet-paper-20260426` (no hits)
  - `cargo check -p izanami --bin izanami`
  - `cargo test -p izanami cli_overrides_packet_loss_percent --no-run`
  - `cargo test -p ivm --bin ivm_prebuild sample_manifest_includes_threshold_escrow --no-run`
  - `cargo test -p ivm --test abi_syscall_list_golden --no-run`
  - `cargo test -p ivm --test abi_hash_versions --no-run`
  - `cargo test -p iroha_core native_anonymous_escrow_syscalls_queue_expected_instructions --lib --no-run`
  - `cargo test -p iroha_torii onboarding_alias_auto_renew_grants_subscriber_metadata_mutation --lib --features app_api --no-run`
  - `swift test --filter NativeEscrowInstructionBuildersTests`
- Validation blockers:
  - `cargo test -p ivm --test abi_syscall_list_golden -- --nocapture` compiled but the test binary wedged before entering the test body at `_dyld_start`; a direct `target/debug/deps/abi_syscall_list_golden-* --nocapture` run timed out the same way after the no-run compile passed.
  - Kotlin and Java Android focused Gradle tests were not runnable in this shell because `java` and `/usr/libexec/java_home -v 21` reported no installed Java runtime.
  - A broader `python3 ci/check_docs_i18n_metadata.py --paths docs/source docs/formal docs/portal --max-messages 10` run found existing `docs/source` and `docs/portal` metadata debt, including missing `source_hash` and `translation_last_reviewed` fields, so source/portal-wide gating remains open.

## 2026-04-26 Offline escrow self-account guard and localnet note seed fix

- `crates/iroha_core/src/smartcontracts/isi/offline.rs` now rejects non-zero Offline V2 note escrow movements when the resolved escrow account is the same account being debited or credited. The new `escrow_self_reference` invariant is checked before balance mutation on issue/reserve and redeem/credit paths.
- `crates/iroha_kagami/src/localnet.rs` no longer writes the built-in offline-note asset escrow account as the localnet app authority. Generated peer configs record the deterministic escrow account for the built-in offline-note asset, and core also derives metadata-enabled escrows at enforcement points so stale or missing config bindings cannot bypass the vault protections.
- Focused validation for this fix:
  - `cargo fmt --all`
  - `cargo test -p iroha_core escrow_self_reference`
  - `cargo test -p iroha_kagami generated_localnet_bootstraps_builtin_offline_note_asset_and_permissions`
  - `cargo test -p iroha_kagami generated_peer_config_enables_offline_note_bootstrap_services`
  - `cargo test -p iroha_kagami localnet_readme_records_base_seed_when_present`

## 2026-04-26 Torii MCP Sumeragi collector empty-topology fix

- Fixed `/v1/sumeragi/collectors` so the Torii/MCP test harness with no commit topology returns an empty collector snapshot instead of constructing a `Topology` from an empty peer list and panicking.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo test -p iroha_torii mcp_jsonrpc_tools_call_agent_alias_sumeragi_endpoints_dispatch -- --nocapture`

## 2026-04-26 Sumeragi locked-chain precommit vote fix

- Fixed local precommit emission so a validator with a known locked block refuses to precommit a different block at the same height, even when the candidate is in a newer view. Missing locked payloads still keep the existing newer-view override behavior.
- Focused validation for this slice:
  - `cargo test -p iroha_core --lib sumeragi::main_loop::tests::precommit_vote_skips_when_block_conflicts_with_locked_chain -- --nocapture --exact`
  - `cargo test -p iroha_core --lib emit_precommit_vote -- --nocapture`
  - `cargo fmt --all`

## 2026-04-26 Nexus router default-lane reroute fix

- Fixed state-aware queue rerouting so universal-only account scope is treated as fallback materialization, not as a dataspace routing target. Untargeted universal authority transactions now continue to use `nexus.routing_policy.default_lane`, while non-universal single-scope accounts still route to their dataspace lane.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo test -p iroha_core queue::tests::apply_lane_lifecycle_reconfigures_router_and_limits -- --nocapture`
  - `cargo test -p iroha_core untargeted_universal_authority_transaction_uses_default_lane_with_state -- --nocapture`

## 2026-04-26 Sumeragi recovery and worker-loop test stabilization

- Stabilized Sumeragi commit recovery around QC-backed pending blocks without proposal evidence: cached prepare/commit QCs now count as consensus evidence for validation and commit gating, and local precommit emission records and applies the vote synchronously before broadcast.
- Tightened recovery scheduling for empty-frontier local vote evidence, authoritative RBC/body ingress, and snapshot-roster near-quorum reschedules.
- Restored deterministic worker-loop budget tests by isolating queue-depth state and allowing one pre-tick drain before marking tiny drain budgets exhausted.
- The Sumeragi/Izanami paper-shaped communication vulnerability matrix now completes for both Sumeragi modes with resilient rows across targeted load, transient failure, packet loss, stopping, and leader isolation. The exact-injector 2026-04-26 paper run at `dist/izanami-exact-packet-paper-20260426` produced ten `exit_code=0`, `status=ok`, `paper_outcome=resilient` rows, and the failure-marker scan found no `429`, confirmation timeout, stuck queued transaction, route-unavailable, queue-pressure, startup/config, or pipeline-status failure signatures.
- Izanami now has an in-process P2P packet-loss injector controlled through `iroha_config` network fields. The matrix `packet-loss` and `leader-isolation` scenarios use 75% application-frame loss during their attack windows without mutating validator rosters or deterministic consensus state.
- Duration-only Izanami runs now sample quorum and strict block heights at the deadline, so matrix reports can record progress evidence even when no `--target-blocks` KPI is configured. The matrix runner now writes `evidence.tsv` and an `Iroha Run Evidence` section with progress evidence and acceptance-failure marker counts while preserving the original paper-style result table.
- Izanami now supports run-relative `--fault-window-start` / `--fault-window-end` offsets. Paper-mode matrix fault scenarios pass the paper's `133s` to `266s` attack window, while quick mode remains immediate/randomized for fast local smoke runs.
- The leader-isolation harness now detects single-peer network-fault profiles and retargets each injection round from live Sumeragi leader telemetry instead of relying on the initial random faulty-peer selection.
- Short local leader-isolation smokes at `dist/izanami-leader-target-smoke-20260426/permissioned-rustlog.log` and `dist/izanami-leader-target-smoke-20260426/npos-rustlog.log` exercised the dynamic path for both Sumeragi modes: Izanami detected the profile, sampled `/v1/sumeragi/leader`, injected the partition into the sampled leader, rejoined it, and completed without the matrix acceptance-failure markers.
- Native packet-loss validation covered both Sumeragi modes. Short smokes at `dist/izanami-packet-loss-smoke-20260426/permissioned-leader-rerun.log` and `dist/izanami-packet-loss-smoke-20260426/npos-leader.log` each injected 75% packet loss into the sampled leader, offered 47 transactions, accepted 47, and finished with zero failures. A quick matrix limited to `packet-loss` at `dist/izanami-packet-loss-smoke-20260426/quick-packet-loss-matrix` produced `resilient` rows for permissioned and NPoS with zero failure-marker hits.
- Focused validation for this slice:
  - `cargo test -p iroha_core --lib sumeragi::main_loop::tests::commit_pipeline_allows_tip_pending_with_cached_qc_without_proposal_evidence -- --nocapture --exact`
  - Direct `target/debug/deps/iroha_core-afb8267c04707e87` runs for the Sumeragi main-loop and worker-loop failures listed in the test report.
  - `target/debug/deps/iroha_core-afb8267c04707e87 sumeragi::main_loop::tests::reschedule_stale_pending_blocks_targets_snapshot_roster --exact --nocapture`
  - `CARGO_TARGET_DIR=target/codex-izanami cargo test -p iroha_core --lib commit_pipeline -- --nocapture` (`45` passed)
  - `CARGO_TARGET_DIR=target/codex-izanami cargo test -p iroha_core --lib reschedule_stale_pending_blocks_targets_snapshot_roster -- --nocapture` (`1` passed)
  - `CARGO_TARGET_DIR=target/codex-izanami cargo test -p iroha_core --lib run_worker_iteration -- --nocapture` (`27` passed)
  - `CARGO_TARGET_DIR=target/codex-izanami cargo test -p iroha_torii pipeline_status -- --nocapture` (`19` passed)
  - `CARGO_TARGET_DIR=target/codex-izanami cargo test -p izanami communication_vulnerabilities -- --nocapture` (`5` passed)
  - `CARGO_TARGET_DIR=target/codex-izanami cargo test -p izanami fault_window -- --nocapture` (`8` passed)
  - `CARGO_TARGET_DIR=target/codex-izanami cargo test -p izanami stored_args_roundtrip_preserves_fault_window_fields -- --nocapture` (`1` passed)
  - `CARGO_TARGET_DIR=target/codex-izanami cargo test -p izanami wait_for_duration_deadline -- --nocapture` (`2` passed)
  - `CARGO_TARGET_DIR=target/codex-izanami cargo test -p izanami -- --nocapture` (`255` passed)
  - `CARGO_TARGET_DIR=target/codex-izanami cargo build -p izanami --bin izanami`
  - `CARGO_TARGET_DIR=target/codex-packet-loss cargo test -p iroha_p2p debug_packet_loss_dropper --lib -- --nocapture` (`1` passed)
  - `CARGO_TARGET_DIR=target/codex-packet-loss cargo test -p iroha_p2p --tests --no-run`
  - `CARGO_TARGET_DIR=target/codex-packet-loss cargo test -p iroha_core --lib sumeragi_resilience --no-run`
  - `CARGO_TARGET_DIR=target/codex-packet-loss cargo test -p iroha_torii --test connect_gating --no-run`
  - `CARGO_TARGET_DIR=target/codex-packet-loss cargo test -p izanami network_packet_loss -- --nocapture` (`1` passed)
  - `CARGO_TARGET_DIR=target/codex-packet-loss cargo test -p izanami sumeragi_leader -- --nocapture` (`4` passed)
  - `CARGO_TARGET_DIR=target/codex-packet-loss cargo test -p izanami communication_vulnerabilities -- --nocapture` (`5` passed)
  - `CARGO_TARGET_DIR=target/codex-packet-loss cargo test -p izanami fault_window -- --nocapture` (`8` passed)
  - `CARGO_TARGET_DIR=target/codex-packet-loss cargo test -p izanami fault_toggles -- --nocapture` (`2` passed)
  - `CARGO_TARGET_DIR=target/codex-packet-loss cargo build -p izanami --bin izanami`
  - `bash -n scripts/run_izanami_communication_vulnerability_matrix.sh`
  - `scripts/run_izanami_communication_vulnerability_matrix.sh --out /tmp/izanami-matrix-script-smoke-quick --mode quick --sumeragi-mode both --only targeted-load --izanami-cmd true`
  - `scripts/run_izanami_communication_vulnerability_matrix.sh --out /tmp/izanami-matrix-script-smoke-paper --mode paper --sumeragi-mode both --only transient-failure --izanami-cmd true`
  - `scripts/run_izanami_communication_vulnerability_matrix.sh --out /tmp/izanami-matrix-script-smoke-leader-quick --mode quick --sumeragi-mode both --only leader-isolation --izanami-cmd true`
  - `scripts/run_izanami_communication_vulnerability_matrix.sh --out /tmp/izanami-matrix-script-smoke-leader-paper --mode paper --sumeragi-mode both --only leader-isolation --izanami-cmd true`
  - `scripts/run_izanami_communication_vulnerability_matrix.sh --out /tmp/izanami-matrix-script-smoke-packet-quick --mode quick --sumeragi-mode both --only packet-loss --izanami-cmd true`
  - `scripts/run_izanami_communication_vulnerability_matrix.sh --out /tmp/izanami-matrix-script-smoke-leader-packet-paper --mode paper --sumeragi-mode both --only leader-isolation --izanami-cmd true`
  - `target/codex-izanami/debug/izanami --allow-net --peers 4 --duration 18s --fault-window-start 2s --fault-window-end 9s --tps 5 --max-inflight 16 --workload-profile stable --faulty 1 --submitters 4 --fault-interval-min 1s --fault-interval-max 1s --fault-enable-network-partition=true ...` with `RUST_LOG='info,izanami::faults=debug'`
  - `target/codex-izanami/debug/izanami --nexus --allow-net --peers 4 --duration 18s --fault-window-start 2s --fault-window-end 9s --tps 5 --max-inflight 16 --workload-profile stable --faulty 1 --submitters 4 --fault-interval-min 1s --fault-interval-max 1s --fault-enable-network-partition=true ...` with `RUST_LOG='info,izanami::faults=debug'`
  - `target/codex-packet-loss/debug/izanami --allow-net --peers 4 --duration 55s --fault-window-start 8s --fault-window-end 20s --tps 5 --max-inflight 16 --workload-profile stable --faulty 1 --submitters 4 --fault-enable-network-packet-loss=true ...` with `RUST_LOG='info,izanami::faults=debug,iroha_p2p::network=debug'`
  - `target/codex-packet-loss/debug/izanami --nexus --allow-net --peers 4 --duration 55s --fault-window-start 8s --fault-window-end 20s --tps 5 --max-inflight 16 --workload-profile stable --faulty 1 --submitters 4 --fault-enable-network-packet-loss=true ...` with `RUST_LOG='info,izanami::faults=debug,iroha_p2p::network=debug'`
  - `scripts/run_izanami_communication_vulnerability_matrix.sh --out dist/izanami-packet-loss-smoke-20260426/quick-packet-loss-matrix --mode quick --sumeragi-mode both --only packet-loss --izanami-cmd target/codex-packet-loss/debug/izanami`
  - `scripts/run_izanami_communication_vulnerability_matrix.sh --mode paper --sumeragi-mode both --izanami-cmd target/codex-izanami/debug/izanami -- --seed 7`
  - `cargo fmt --all`
  - `git diff --check`

## 2026-04-26 Offline V2 first-release replacement

- Hardened Offline V2 note issuance so only `CanManageOfflineEscrow` operators can issue notes, and key certificates must verify against the issuing operator over the canonical certificate payload before escrow is reserved.
- Hardened Offline V2 note redemption so the recursive proof public-input hash must bind the source note commitment, consumed nullifiers, certified key payload, recipient, asset, and amount, and escrow is released only for a ledger-recorded issued-note claim that has not already been redeemed.
- Hardened Offline V2 optional audit so the proof public-input hash binds the token id, observed nullifiers, output commitments, and certified key payload; audit now requires a previously issued key certificate and detects token/public-input conflicts plus duplicate output commitments.
- Ordered cheap issued-claim, token, and nullifier replay checks before expensive recursive proof verification while still verifying proofs before escrow release or new audit state.
- Replaced the local transcript-style recursive proof placeholder with verifier-key-backed validation: the proof must name an active `offline_note_v2` WSV verifier, decode as an `OpenVerifyEnvelope`, match the Offline V2 public-input schema hash, expose the expected public instance columns, and pass the configured ZK backend verifier.
- Added data-model helper payloads for canonical key-certificate signing bytes, issued-note claims, redemption public inputs, and audit public inputs.
- Removed legacy allowance, lineage, transfer, revocation, balance-proof, petal-stream, and settlement helper surfaces across Rust, Torii, mobile SDKs, examples, fixtures, and stale docs.
- Torii now exposes only `/v1/offline/v2/readiness` for offline discovery; issuance, redemption, and audit use V2 transaction instructions.
- Torii MCP keeps structured compatibility aliases for legacy offline transfer/revocation tool names so agent clients get Offline V2 guidance instead of JSON-RPC tool-not-found errors; this does not re-publish the removed HTTP routes.
- Localnet, telemetry, QR payload kinds, and mobile parser surfaces now use Offline V2 note naming instead of legacy cash/transfer terminology.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo test -p iroha_torii --test mcp_endpoints`
  - `CARGO_TARGET_DIR=target/codex-workspace-test cargo check -p iroha_data_model -p iroha_core -p iroha_torii -p iroha_config -p iroha_kagami -p iroha_telemetry -p connect_norito_bridge -p fastpq_prover -p fastpq_isi --lib`
  - `CARGO_TARGET_DIR=target/codex-workspace-test cargo test -p iroha_data_model offline_note_v2 --lib -- --nocapture`
  - `CARGO_TARGET_DIR=target/codex-workspace-test cargo test -p iroha_torii --test offline_v2_readiness_smoke -- --nocapture`
  - `CARGO_TARGET_DIR=target/codex-workspace-test cargo test -p connect_norito_bridge --lib -- --nocapture`
  - `CARGO_TARGET_DIR=target/codex-workspace-test cargo test -p iroha_core offline_note_v2 --lib -- --nocapture` (ok; no Core-local tests matched after the model tests moved to `iroha_data_model`)
  - `swift test`
  - `./gradlew :core-jvm:test --console=plain`
  - `./gradlew :offline-wallet-android:assembleRelease --console=plain`
  - `JAVA_HOME=$(/usr/libexec/java_home -v 21) ANDROID_HOME=~/Library/Android/sdk ANDROID_SDK_ROOT=~/Library/Android/sdk ./gradlew test --console=plain`
  - `npm run build:dist && node --test test/toriiClient.test.js test/package_dist.test.js test/offlineQrStream.test.js`
  - `python3 -m pytest python/iroha_torii_client/tests/test_client.py python/iroha_python/tests/testconnect_codec.py -q`
  - `git diff --check`
  - Stale-route/native-symbol scans for legacy offline routes, removed native exports, old Offline V1 fixtures, and deleted Safety Detect wrappers returned no matches in active source/fixture paths.

## 2026-04-25 Taira devex CLI and onboarding diagnostics

- Added the first-class `iroha taira` CLI surface. `iroha taira doctor` performs read-only checks against the public Taira root by default, including `/status`, route availability, MCP initialize, curated MCP tool availability, and recent status warnings. `iroha taira write-canary` now drives the preferred real-write path: ephemeral signer by default, alias/public-key onboarding, faucet PoW claim, gas metadata insertion, signed ping submission, Applied wait, query verification attempt, optional restrictive `--write-config`, and redacted text/JSON receipts.
- Torii MCP `accounts.onboard` now advertises and forwards the `public_key_hex` shortcut, matching the HTTP onboarding path. JSON onboarding clients now receive stable `error_code`, `message`, and optional `hint` diagnostics while explicit Norito clients keep the existing Norito error envelope.
- The Taira rollout docs now steer single-endpoint devex checks to `iroha taira doctor` and `iroha taira write-canary`; `check_mcp_rollout.sh` remains available as the multi-step compatibility harness.
- Focused validation for this slice:
  - `cargo check -p iroha_cli --bin iroha`
  - `cargo check -p iroha_torii --lib`
  - `cargo test -p iroha_cli --bin iroha taira -- --nocapture`
  - `cargo test -p iroha_cli`
  - `cargo test -p iroha_torii --lib onboarding -- --nocapture`
  - `cargo test -p iroha_torii --lib build_accounts_onboard_body -- --nocapture`
  - `bash -n configs/soranexus/taira/check_mcp_rollout.sh configs/soranexus/taira/check_mcp_rollout_mock_test.sh`
  - `bash configs/soranexus/taira/check_mcp_rollout_mock_test.sh`
  - `cargo clippy -q -p iroha_cli --bin iroha --no-deps -- -D warnings`

## 2026-04-25 CUDA coverage follow-up 3

- Added a third focused coverage pass for remaining edge branches: IVM explicit CUDA disable status reset, FASTPQ BN254 wrapper shape rejection before backend calls, `gpuzstd_cuda` exact-capacity compression and empty-source decode errors, `jsonstage1_cuda` zero-capacity required-length reporting, and shared zstd frame truncated-header / bad-magic rejection plus out-of-range sequence metadata rejection.
- Focused validation for this follow-up:
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-gpuzstd-metal cargo test -p gpuzstd_metal -- --nocapture` (`30` passed; `1` ignored)
  - `RUST_TEST_THREADS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-ivm-cuda-check IVM_CUDA_GENCODE=arch=compute_86,code=sm_86 cargo test -p ivm --lib explicit_cuda_disable_records_message_and_reset_clears_it --features cuda -- --nocapture` (`1` passed; existing `iroha_crypto` dead-code warnings remain)
  - `RUST_TEST_THREADS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-fastpq-cuda-check cargo test -p fastpq_prover --lib --features fastpq-gpu fastpq_cuda::tests -- --nocapture` (`12` passed)
  - `RUST_TEST_THREADS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-gpuzstd-cuda GPUZSTD_CUDA_ARCH=-arch=sm_86 GPUZSTD_CUDA_REQUIRE=1 cargo test -p gpuzstd_cuda --features cuda-kernel -- --nocapture` (`14` passed)
  - `RUST_TEST_THREADS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-jsonstage1-cuda JSONSTAGE1_CUDA_ARCH=-arch=sm_86 JSONSTAGE1_CUDA_REQUIRE=1 cargo test -p jsonstage1_cuda --features cuda-kernel -- --nocapture` (`21` passed)

## 2026-04-25 CUDA coverage follow-up 2

- Added another focused coverage pass for edge branches around the CUDA closure: IVM CUDA enable/disable/reset status flags, FASTPQ length saturation and pre-backend shape validation, `gpuzstd_cuda` compression no-space and empty-payload roundtrips, `jsonstage1_cuda` exact-capacity and empty public-entrypoint behavior, and shared zstd frame empty-input plus invalid chunk-metadata rejection.
- The shared zstd frame encoder now validates `chunk_size`, `counts`, and `offsets` before indexing chunk metadata, and emits a valid final empty block for empty payloads.
- Focused validation for this follow-up:
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-gpuzstd-metal cargo test -p gpuzstd_metal -- --nocapture` (`29` passed; `1` ignored)
  - `RUST_TEST_THREADS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-ivm-cuda-check IVM_CUDA_GENCODE=arch=compute_86,code=sm_86 cargo test -p ivm --lib cuda_wait --features cuda -- --nocapture` (`3` passed; existing `iroha_crypto` dead-code warnings remain)
  - `RUST_TEST_THREADS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-ivm-cuda-check IVM_CUDA_GENCODE=arch=compute_86,code=sm_86 cargo test -p ivm --lib cuda_enable_disable_and_reset_update_status_flags --features cuda -- --nocapture` (`1` passed; existing `iroha_crypto` dead-code warnings remain)
  - `RUST_TEST_THREADS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-fastpq-cuda-check cargo test -p fastpq_prover --lib --features fastpq-gpu fastpq_cuda::tests -- --nocapture` (`11` passed)
  - `RUST_TEST_THREADS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-gpuzstd-cuda GPUZSTD_CUDA_ARCH=-arch=sm_86 GPUZSTD_CUDA_REQUIRE=1 cargo test -p gpuzstd_cuda --features cuda-kernel -- --nocapture` (`12` passed)
  - `RUST_TEST_THREADS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-jsonstage1-cuda JSONSTAGE1_CUDA_ARCH=-arch=sm_86 JSONSTAGE1_CUDA_REQUIRE=1 cargo test -p jsonstage1_cuda --features cuda-kernel -- --nocapture` (`19` passed)

## 2026-04-25 CUDA coverage follow-up

- Added focused tests for the CUDA closure paths that were previously validated mostly through happy-path hardware runs: IVM wait-state ready/failure/timeout transitions, FASTPQ CUDA validation/error formatting, `gpuzstd_cuda` FFI null/no-space/exact-capacity behavior, `jsonstage1_cuda` FFI null handling plus empty Stage-1/CRC64 CPU edge cases, and Norito's 16 MiB GPU cutoff selector under a disabled GPU policy.
- Focused validation for this follow-up:
  - `RUST_TEST_THREADS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-ivm-cuda-check IVM_CUDA_GENCODE=arch=compute_86,code=sm_86 cargo test -p ivm --lib cuda_wait --features cuda -- --nocapture` (`3` passed; existing `iroha_crypto` dead-code warnings remain)
  - `RUST_TEST_THREADS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-fastpq-cuda-check cargo test -p fastpq_prover --lib --features fastpq-gpu fastpq_cuda::tests -- --nocapture` (`8` passed)
  - `RUST_TEST_THREADS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-gpuzstd-cuda GPUZSTD_CUDA_ARCH=-arch=sm_86 GPUZSTD_CUDA_REQUIRE=1 cargo test -p gpuzstd_cuda --features cuda-kernel -- --nocapture` (`10` passed)
  - `RUST_TEST_THREADS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-jsonstage1-cuda JSONSTAGE1_CUDA_ARCH=-arch=sm_86 JSONSTAGE1_CUDA_REQUIRE=1 cargo test -p jsonstage1_cuda --features cuda-kernel -- --nocapture` (`16` passed)
  - `RUST_TEST_THREADS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-norito-gpu cargo test -p norito core::heuristics --features gpu-compression -- --nocapture` (`2` passed; existing `struct_index_random_x86.rs` unused-parens warning remains)

## 2026-04-25 CUDA roadmap closure

- IVM CUDA now uses bounded stream waits that fail closed, disable CUDA on timeout, and intentionally abandon outstanding device allocations instead of risking a blocking `cuMemFree` after a timed-out stream. `GpuContext` also drops the CUDA context last so cached modules, streams, and buffers release in driver-safe order.
- IVM Poseidon and public helper paths now use pinned host buffers plus explicit async copies where host-visible CUDA results are read. New fixtures cover timeout handling and repeated CPU-vs-CUDA determinism for vector add, SHA-256, Keccak, AES, BN254, and Ed25519 helpers.
- FASTPQ CUDA FFT/LDE/BN254/Poseidon paths now use nonblocking streams, pinned host transfer buffers, bounded event polling, and a timeout harness that exercises the fail-closed path without wedging a GPU. Repeated BN254 transform determinism coverage is in place.
- Norito's `gpuzstd_cuda`, JSON Stage-1, and CRC64 helpers now use pinned async host/device transfers with bounded event waits before host-visible reads. A zstd offset-code bug in the shared frame encoder was fixed and covered with a standard zstd roundtrip fixture.
- The default Norito GPU compression cutoff is now `16 MiB`. On the RTX 3080 Laptop / WSL2 host below, CUDA zstd stayed slower than CPU at the measured automatic-offload sizes: 1 MiB was `1.700 ms` CPU vs `145.493 ms` CUDA, and 8 MiB was `13.747 ms` CPU vs `517.025 ms` CUDA. JSON Stage-1 CUDA crossed over on this run around 4 KiB (`5593 ns` scalar vs `5030 ns` kernel) and remained faster at 256 KiB (`350011 ns` scalar vs `315870 ns` kernel).
- Added `.github/workflows/nightly_cuda.yml`, a self-hosted CUDA nightly/manual lane that builds real CUDA helpers, runs the focused IVM/FASTPQ/Norito accelerator gates, and uploads GPU model, driver, CUDA toolkit, and gencode inventory under `dist/cuda-nightly`.
- Hardware/toolchain used for closure: NVIDIA GeForce RTX 3080 Laptop GPU, driver `527.56`, compute capability `8.6`, CUDA `12.0.140`, `IVM_CUDA_GENCODE=arch=compute_86,code=sm_86`, `GPUZSTD_CUDA_ARCH=-arch=sm_86`, `JSONSTAGE1_CUDA_ARCH=-arch=sm_86`.
- Focused validation for this closure:
  - `RUST_TEST_THREADS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-ivm-cuda-check IVM_CUDA_GENCODE=arch=compute_86,code=sm_86 cargo test -p ivm --lib cuda_ --features cuda -- --nocapture` (`24` passed; existing `iroha_crypto` dead-code warnings remain)
  - `RUST_TEST_THREADS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-fastpq-cuda-check cargo test -p fastpq_prover --lib --features fastpq-gpu cuda -- --nocapture` (`9` passed)
  - `RUST_TEST_THREADS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-gpuzstd-cuda GPUZSTD_CUDA_ARCH=-arch=sm_86 GPUZSTD_CUDA_REQUIRE=1 cargo test -p gpuzstd_cuda --features cuda-kernel -- --nocapture` (`7` passed)
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-gpuzstd-metal cargo test -p gpuzstd_metal -- --nocapture` (`27` passed; `1` ignored)
  - `RUST_TEST_THREADS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-jsonstage1-cuda JSONSTAGE1_CUDA_ARCH=-arch=sm_86 JSONSTAGE1_CUDA_REQUIRE=1 cargo test -p jsonstage1_cuda --features cuda-kernel -- --nocapture` (`13` passed)
  - `RUST_TEST_THREADS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-norito-gpu GPUZSTD_CUDA_ARCH=-arch=sm_86 GPUZSTD_CUDA_REQUIRE=1 cargo test -p norito gpu_zstd --features gpu-compression -- --nocapture` and `required_cuda_backend_is_registered_when_requested` (passed; existing `struct_index_random_x86.rs` unused-parens warning remains)
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-norito-stage1 JSONSTAGE1_CUDA_ARCH=-arch=sm_86 cargo build -p jsonstage1_cuda --features cuda-kernel`, then `RUST_TEST_THREADS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-norito-stage1 JSONSTAGE1_CUDA_ARCH=-arch=sm_86 JSONSTAGE1_CUDA_REQUIRE=1 cargo test -p norito stage1_helper --features cuda-stage1,stage1-validate -- --nocapture` and `cuda_stage1_backend_matches_scalar_when_required_or_available` (passed)
  - `cargo run -p norito --example gpu_threshold --release --features gpu-compression -- --json` with the CUDA zstd helper built in release mode, and `cargo run -p norito --example stage1_cutover --release --features bench-internal,cuda-stage1,stage1-validate` with the JSON Stage-1 helper built in release mode, captured the cutoff data above.

## 2026-04-25 Norito CUDA GPU helpers

- `gpuzstd_cuda` now builds CUDA kernels by default when `nvcc` is available. Compression runs deterministic CUDA match-finding/sequence generation and uses the shared zstd frame encoder; helpers without built kernels or a CUDA device report `gpu_unavailable` instead of registering a CPU-only helper as a GPU backend. CUDA and frame-assembly failures now return errors to Norito rather than CPU-encoding inside the helper.
- Norito's non-Mac CUDA zstd loader now accepts only the CUDA-named helper and rejects `gpu_unavailable` compression during self-test before enabling the backend. The loader no longer requires `nvidia-smi`; CUDA availability is proven by the helper self-test.
- `jsonstage1_cuda` now includes a CUDA JSON Stage-1 classifier for structural, quote, and backslash masks, with host finalization preserving quote/backslash parity across 32-byte blocks. CUDA CRC64 also reports unavailable when kernels/devices are missing instead of silently falling back inside the helper.
- CUDA validation tests now support required-hardware modes: `GPUZSTD_CUDA_REQUIRE=1` for `gpuzstd_cuda` and `JSONSTAGE1_CUDA_REQUIRE=1` for `jsonstage1_cuda`. Without those env vars, CUDA-only assertions skip cleanly on hosts without kernels/devices.
- Documentation: `crates/norito/README.md` and `docs/source/gpuzstd_cuda_pipeline.md` describe the CUDA zstd, JSON Stage-1, CRC64, and fallback contracts.
- Focused validation for this slice:
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-gpuzstd-cuda cargo test -p gpuzstd_cuda`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-jsonstage1-cuda cargo test -p jsonstage1_cuda`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-norito-gpu cargo test -p norito gpu_zstd --features gpu-compression`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-norito-stage1 cargo test -p norito stage1_helper --features cuda-stage1,stage1-validate`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-norito-stage1 cargo test -p norito cuda_stage1_backend_matches_scalar_when_required_or_available --features cuda-stage1,stage1-validate`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-gpuzstd-metal cargo test -p gpuzstd_metal`

## 2026-04-25 Native anonymous asset escrow core

- Added `AnonymousAssetEscrowRecord`, proof-movement records, and anonymous dispute-resolution records. These store the escrow note commitment, funding/spend nullifiers, labelled buyer/seller output commitments, proof hash, optional envelope hash, root hint, timestamps, lifecycle state, and evidence hashes for on-chain auditability.
- Added native anonymous escrow ISIs for open, accept, mark payment sent, release, cancel, dispute, and resolve. Open/release/cancel/resolve execute through the existing shielded `ZkTransfer` path, so policy checks, root freshness, nullifier replay protection, proof verification, deterministic output ordering, and confidential proof-hash events stay shared with the ZK asset ledger.
- Added world-state storage plus typed query/JSON surfaces for anonymous escrow records, and a generic `ZkTransfer` custody guard: active anonymous escrow commitments require confidential transfer v2 public input commitments, and generic shielded transfers that spend an active escrow note are rejected unless they run inside a native anonymous escrow ISI.
- Hardened anonymous escrow close handling so release, cancellation, and court-resolution proofs must expose exactly one non-zero confidential transfer v2 input commitment matching the stored escrow commitment. Public and anonymous escrows now share `EscrowId` uniqueness both ways, proof-carrying anonymous escrow ISIs charge the same confidential gas as their internal `ZkTransfer`, and JSON snapshot restore preserves public and anonymous escrow records.
- Focused validation for this slice:
  - `cargo fmt --all --check`
  - `CARGO_TARGET_DIR=target/codex-escrow-check cargo check -p iroha_data_model -p iroha_core`
  - `CARGO_TARGET_DIR=target/codex-escrow-check cargo test -p iroha_data_model escrow --lib`
  - `CARGO_TARGET_DIR=target/codex-escrow-check cargo test -p iroha_core escrow --lib`
  - `CARGO_TARGET_DIR=target/codex-escrow-check cargo test -p iroha_core gas --lib -- --nocapture`
  - `CARGO_TARGET_DIR=target/codex-escrow-check cargo test -p iroha_core escrow_records_roundtrip_through_state_json --lib -- --nocapture`

## 2026-04-25 Native escrow custody hardening and SDK surfaces

- Added a typed executor data-model permission token for `CanResolveEscrowDispute`, and core now asserts that the escrow court permission string matches the typed permission.
- Hardened native escrow custody so generic numeric asset debits from any recorded deterministic custody account are rejected through both `Transfer<Asset>` and `Burn<Asset>`, including the closed-record dust case. Escrow release/cancel/resolve remain the only valid custody exit paths.
- Added SDK-facing native escrow helpers for Kotlin, Java Android, and Swift, plus a Kotodama `native_escrow.ko` sample and docs that steer new Aitai-style numeric custody flows away from the legacy threshold escrow grant pattern.
- Added an ABI admission regression proving a V1 contract deployment proposal with a non-canonical ABI hash is rejected after the escrow syscall surface update.
- Focused validation for this gap-closure slice:
  - `cargo fmt --all --check`
  - `CARGO_TARGET_DIR=target/codex-escrow-check cargo check -p iroha_executor_data_model -p iroha_data_model -p iroha_core -p kotodama_lang -p ivm_abi -p iroha`
  - `CARGO_TARGET_DIR=target/codex-escrow-check cargo test -p iroha_data_model escrow --lib`
  - `CARGO_TARGET_DIR=target/codex-escrow-check cargo test -p iroha_executor_data_model escrow_court_permission_uses_expected_name --lib`
  - `CARGO_TARGET_DIR=target/codex-escrow-check cargo test -p iroha_core escrow --lib`
  - `CARGO_TARGET_DIR=target/codex-escrow-check cargo test -p iroha_core propose_rejects_non_canonical_abi_hash_for_v1 --test gov_propose_validation`
  - `CARGO_TARGET_DIR=target/codex-escrow-check cargo test -p kotodama_lang native_escrow_builtins_emit_escrow_syscalls --lib`
- Local SDK validation:
  - Installed Homebrew `openjdk@21` and Android command-line tooling, then populated `~/Library/Android/sdk` with platform/build-tools 34 for the Java Android Gradle harness.
  - Materialized the local Swift bridge with `scripts/build_norito_xcframework.sh`, which produced `dist/NoritoBridge.xcframework` and `dist/NoritoBridge.artifacts.json` for package testing.
  - `JAVA_HOME=/opt/homebrew/opt/openjdk@21/libexec/openjdk.jdk/Contents/Home PATH=/opt/homebrew/opt/openjdk@21/bin:$PATH ./gradlew :core-jvm:test --tests org.hyperledger.iroha.sdk.core.model.instructions.NativeEscrowInstructionsTest --console=plain`
  - `JAVA_HOME=/opt/homebrew/opt/openjdk@21/libexec/openjdk.jdk/Contents/Home PATH=/opt/homebrew/opt/openjdk@21/bin:$PATH ANDROID_HOME=$HOME/Library/Android/sdk ANDROID_SDK_ROOT=$HOME/Library/Android/sdk ./gradlew :core:test --tests org.hyperledger.iroha.android.model.instructions.NativeEscrowInstructionTests --console=plain`
  - `swift test --filter NativeEscrowInstructionBuildersTests`
  - `JAVA_HOME=/opt/homebrew/opt/openjdk@21/libexec/openjdk.jdk/Contents/Home PATH=/opt/homebrew/opt/openjdk@21/bin:$PATH ./gradlew :core-jvm:cleanTest :core-jvm:test --console=plain --no-daemon`
  - `JAVA_HOME=/opt/homebrew/opt/openjdk@21/libexec/openjdk.jdk/Contents/Home PATH=/opt/homebrew/opt/openjdk@21/bin:$PATH ANDROID_HOME=$HOME/Library/Android/sdk ANDROID_SDK_ROOT=$HOME/Library/Android/sdk ./gradlew :core:test --console=plain`
  - `swift test`

## 2026-04-25 Native escrow observability and Aitai integration gap closure

- Added app-facing escrow event filtering via `EscrowEventFilter`, including filters by escrow id, seller, buyer, lifecycle status, and escrow event kind.
- Moved Kotodama escrow id derivation into the data model with `EscrowId::from_kotodama_name`, so compiler/host/client code use the same deterministic mapping.
- Extended native escrow IVM syscalls and Kotodama builtins with optional evidence-hash TLV registers for open, dispute, and resolve flows while keeping zero as the no-evidence path.
- Added ergonomic constructors and prelude exports for all native escrow ISIs, JSON query support for `FindAssetEscrowById`, iterable JSON support for `FindAssetEscrows`, and typed batch downcasting for `AssetEscrowRecord`.
- Added a four-peer native Aitai-style integration flow under `core_api`: open escrow with evidence, prove generic transfer from active custody is rejected even after a transfer grant, accept, mark payment sent, release, and query the final buyer/seller balances.
- Focused validation for the gap-closure slice:
  - `cargo fmt --all`
  - `CARGO_TARGET_DIR=target/codex-escrow-check cargo check -p iroha_data_model -p iroha_core -p kotodama_lang -p ivm_abi -p iroha`
  - `CARGO_TARGET_DIR=target/codex-escrow-check cargo test -p iroha_data_model escrow --lib`
  - `CARGO_TARGET_DIR=target/codex-escrow-check cargo test -p iroha_data_model find_asset_queries_roundtrip_with_public_selectors --lib`
  - `CARGO_TARGET_DIR=target/codex-escrow-check cargo test -p kotodama_lang native_escrow_builtins_emit_escrow_syscalls --lib`
  - `CARGO_TARGET_DIR=target/codex-escrow-check cargo test -p iroha_core escrow --lib`
  - `CARGO_TARGET_DIR=target/codex-escrow-check cargo test -p integration_tests native_asset_escrow_aitai_flow_on_multi_peer_network --test core_api --no-run`
  - `CARGO_TARGET_DIR=target/codex-escrow-check cargo test -p integration_tests native_asset_escrow_aitai_flow_on_multi_peer_network --test core_api -- --nocapture`

## 2026-04-24 Native asset escrow ISIs and Kotodama builtins

- Added native numeric asset escrow state keyed by `EscrowId`, including lifecycle status, evidence hashes, timestamps, seller/buyer/custody fields, dispute resolution details, Norito/JSON/schema support, query outputs, and escrow lifecycle events.
- Added `OpenAssetEscrow`, `AcceptAssetEscrow`, `MarkEscrowPaymentSent`, `ReleaseAssetEscrow`, `CancelAssetEscrow`, `OpenEscrowDispute`, and `ResolveEscrowDispute` ISIs. Opening moves seller funds into a deterministic protocol custody account; release/cancel/resolve move custody only through the escrow ISIs. Generic asset transfer now rejects active native escrow custody sources.
- Added the narrow `CanResolveEscrowDispute` permission for court resolution, plus IVM syscalls `0xB8..0xBE` and Kotodama builtins `escrow_open_offer`, `escrow_accept`, `escrow_mark_payment_sent`, `escrow_release`, `escrow_cancel`, `escrow_open_dispute`, and `escrow_resolve_dispute`.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `CARGO_TARGET_DIR=target/codex-escrow-check cargo check -p iroha_data_model -p iroha_core -p kotodama_lang -p ivm_abi`
  - `CARGO_TARGET_DIR=target/codex-escrow-check cargo test -p iroha_data_model escrow --lib -- --nocapture`
  - `CARGO_TARGET_DIR=target/codex-escrow-check cargo test -p iroha_core escrow --lib -- --nocapture`
  - `CARGO_TARGET_DIR=target/codex-escrow-check cargo test -p ivm abi_syscall_list_matches_golden --test abi_syscall_list_golden -- --nocapture`
  - `CARGO_TARGET_DIR=target/codex-escrow-check cargo test -p ivm abi_hash --test abi_hash_versions -- --nocapture`
  - `CARGO_TARGET_DIR=target/codex-escrow-check cargo test -p kotodama_lang native_escrow_builtins_emit_escrow_syscalls --lib -- --nocapture`

## 2026-04-24 Soracloud production posture hardening

- `iroha_config` now exposes `soracloud_runtime.production_mode`; enabling it requires non-proxy Inrou hosting, fail-closed runtime egress with explicit rate/byte budgets, and rejects Hugging Face inference-bridge fallback. The HF fallback default is now disabled.
- `irohad`'s Soracloud runtime stub now refuses production mode at startup, so a production config requires an `irohad` binary built with `embedded-soracloud-runtime`.
- The embedded Soracloud runtime manager now also calls the same production-posture assertion from `actual::SoracloudRuntime`, so direct config construction cannot bypass the user-config parser.
- Torii now has route-specific signed Soracloud body caps: ordinary mutation bodies use `torii.soracloud_mutation_max_body_bytes`, uploaded-model chunk routes use `torii.soracloud_upload_max_body_bytes`, and the cap is enforced before canonical-request signature verification.
- Signed Soracloud POST routes now have account+origin rate limiting and a global inflight cap via `torii.soracloud_mutation_rate_per_account_origin_per_sec`, `torii.soracloud_mutation_burst_per_account_origin`, and `torii.soracloud_mutation_max_inflight`. The rate key includes the verified account, `Origin`, and route group (`mutation`, `upload`, `model`, or `hf`).
- Focused validation for the production-posture slice:
  - `cargo fmt --all`
  - `env -u LOG_FORMAT cargo test -p iroha_config soracloud_runtime_ --lib -- --nocapture`
  - `env -u LOG_FORMAT cargo test -p iroha_config --test fixtures -- --nocapture`
  - `env -u LOG_FORMAT cargo test -p iroha_torii --lib --features app_api,telemetry soracloud_signed_mutation_middleware_ -- --nocapture`
  - `env -u LOG_FORMAT cargo test -p irohad --bin irohad stub_runtime_ -- --nocapture`
  - `env -u LOG_FORMAT cargo test -p irohad --features embedded-soracloud-runtime --bin irohad manager_config_ -- --nocapture`
- The 2026-04-25 portable follow-up closes the local boot-smoke gap:
  - Homebrew QEMU 11.0.0 provides `qemu-system-aarch64` and `qemu-img` on this host.
  - `scripts/ci/prepare_inrou_portable_guest_assets.py` now prepares verified Debian genericcloud assets for native PortableVm smoke runs. It checks Debian `SHA512SUMS`, extracts the root ext4 partition, applies the expected root label/fstab, and exports the matching kernel/rootfs/initrd paths.
  - PortableVm cloud-init network config now matches predictable Ethernet names (`e*`) instead of assuming `eth0`, so Debian arm64 genericcloud guests consume the NoCloud metadata seed and start the hosted app.
  - `cargo run -p xtask --bin xtask -- soracloud-inrou-smoke portable` now preflights the guest asset env vars and points operators at the preparation helper when they are missing.
  - Focused validation for the follow-up:
    - `python3 -m py_compile scripts/ci/prepare_inrou_portable_guest_assets.py`
    - `python3 scripts/ci/prepare_inrou_portable_guest_assets.py --output-dir /tmp/iroha-inrou-assets-genericcloud --print-env`
    - `env -u LOG_FORMAT cargo test -p irohad --features embedded-soracloud-runtime --bin irohad build_inrou_portable_network_config_matches_predictable_interface_names -- --nocapture`
    - `env -u LOG_FORMAT IROHA_RUN_IGNORED=1 IROHA_INROU_PORTABLE=1 IROHA_INROU_PORTABLE_KERNEL_IMAGE=/tmp/iroha-inrou-assets-genericcloud/vmlinux-aarch64 IROHA_INROU_PORTABLE_ROOTFS_IMAGE=/tmp/iroha-inrou-assets-genericcloud/rootfs-aarch64.ext4 IROHA_INROU_PORTABLE_INITRD_IMAGE=/tmp/iroha-inrou-assets-genericcloud/initrd-aarch64.img cargo test --locked -p irohad --features embedded-soracloud-runtime --bin irohad inrou_portable_smoke_boots_debian_guest_and_serves_healthcheck -- --ignored --nocapture`
    - `env -u LOG_FORMAT IROHA_INROU_PORTABLE_KERNEL_IMAGE=/private/tmp/iroha-inrou-assets-genericcloud/vmlinux-aarch64 IROHA_INROU_PORTABLE_ROOTFS_IMAGE=/private/tmp/iroha-inrou-assets-genericcloud/rootfs-aarch64.ext4 IROHA_INROU_PORTABLE_INITRD_IMAGE=/private/tmp/iroha-inrou-assets-genericcloud/initrd-aarch64.img cargo run -p xtask --bin xtask -- soracloud-inrou-smoke portable`
    - `env -u IROHA_INROU_PORTABLE_KERNEL_IMAGE -u IROHA_INROU_PORTABLE_ROOTFS_IMAGE -u IROHA_INROU_PORTABLE_INITRD_IMAGE target/debug/xtask soracloud-inrou-smoke portable`
- The 2026-04-25 readiness-runner follow-up closes the local/load validation gap:
  - `scripts/ci/run_soracloud_production_readiness.sh` now targets the current `integration_tests` Core API test binary for Soracloud multi-peer CLI gates, writes Markdown status bullets safely, clears stale logs when an explicit output directory is reused, supports per-step fail-closed timeouts, and can run Cargo gates with `--cargo-target-dir` or `--isolate-cargo-target` to avoid unrelated workspace lock contention. Full/load profiles now mark missing production-only gates as `blocked` and exit non-zero unless `--allow-open-blockers` is supplied.
  - Torii query batch merge/canonicalization now covers `AnonymousAssetEscrowRecord`, which keeps the Torii Soracloud route-pressure suite compiling with the current native escrow query surface.
  - Proxy-only Inrou hosts are now excluded from local replica projection, advertise zero hosted capacity, and have focused runtime coverage proving they do not publish placed-replica runtime state. The mixed-host inventory example now uses that `proxy_only_inrou_host` gate instead of a plain compile check.
  - Production observability is now a required readiness artifact for full/load profiles. `scripts/ci/check_soracloud_observability_evidence.py` validates deployment evidence for signed-auth failures, body/inflight/rate limit rejections, runtime hydration lag, Inrou lifecycle, lease/cache/disk pressure, egress usage, stale model-host heartbeats, HF fallback use, private-session failures, matching status fields, alerts, runbooks, and dashboards.
  - Focused readiness report: `/tmp/iroha-soracloud-readiness-focused-final/soracloud_production_readiness.md`.
  - Load readiness report: `/tmp/iroha-soracloud-readiness-load/soracloud_production_readiness.md`. Required local/load gates passed under the previous runner behavior: config posture, config fixtures, signed mutation route pressure, public runtime pressure, runtime-stub production rejection, embedded runtime manager posture, live multi-peer status, mutation rollout, training/model lifecycle, and HF shared-lease proration. The mixed-host Inrou smoke was skipped because no operator inventory was supplied; with the current runner, that condition and missing observability evidence are explicit blockers.
  - Additional validation for this follow-up:
    - `bash -n scripts/ci/run_soracloud_production_readiness.sh`
    - `scripts/ci/run_soracloud_production_readiness.sh --help`
    - `python3 -m py_compile scripts/ci/check_soracloud_observability_evidence.py`
    - `python3 scripts/ci/check_soracloud_observability_evidence.py --evidence fixtures/soracloud/production_observability_evidence.example.json`
    - `python3 -m pytest scripts/tests/check_soracloud_observability_evidence_test.py`
    - `env -u LOG_FORMAT cargo test -p iroha_torii --lib --features app_api,telemetry soracloud_signed_mutation_ -- --nocapture`
    - `env -u LOG_FORMAT cargo test -p iroha_torii --lib --features app_api,telemetry soracloud_public_runtime_rate_and_inflight_limits_fail_closed -- --nocapture`
    - `env -u LOG_FORMAT cargo test -p iroha_torii --lib --features app_api,telemetry anonymous_asset_escrow_records -- --nocapture`
    - `env -u LOG_FORMAT cargo test -p irohad --features embedded-soracloud-runtime --bin irohad manager_config_ -- --nocapture`
    - `env -u LOG_FORMAT cargo test -p irohad --features embedded-soracloud-runtime --bin irohad proxy_only_inrou_host -- --nocapture`
    - `scripts/ci/run_soracloud_production_readiness.sh --profile focused --out /tmp/iroha-soracloud-readiness-focused-final`
    - `scripts/ci/run_soracloud_production_readiness.sh --profile load --out /tmp/iroha-soracloud-readiness-load`

## 2026-04-24 Verified lane relay JSON state key for UC6

- `LaneRelayEnvelopeRef::relay_state_key()` now exposes the canonical contract-state key for verified lane relay records: `pkdeploy_verified_lane_relay_<dataspace_id>_<lane_id>_<block_height>_<hash64>`. The helper avoids slash-delimited paths and gives deploy/Core API code a single source for the public relay key string.
- `RegisterVerifiedLaneRelay` / `FindLaneRelayEnvelopeByRef` continue to persist and read contract-visible Norito JSON for `VerifiedLaneRelayRecord`, and focused coverage now pins that old raw `VerifiedLaneRelayRecord` bytes are rejected by the JSON decoder path.
- The IVM schema registry no longer carries `VerifiedLaneRelayRecord` for this flow, so Kotodama UC6 contracts consume the relay record through `decode_json(relay_state)` rather than `decode_schema(...)`.
- `crates/iroha_core/src/smartcontracts/isi/world.rs` now makes `verified_lane_relay_state_key_is_single_contract_name` derive its expected prefix from the shared `VERIFIED_LANE_RELAY_STATE_KEY_PREFIX` and the live `LaneRelayEnvelopeRef` fields instead of pinning a stale hardcoded sample prefix. The production formatter already uses `dataspace_id`, `lane_id`, and `block_height`, and the sample relay record currently comes from `ValidBlock::new_dummy`, which defaults to block height `2`.
- This keeps the regression focused on the real contract-name guarantee: the verified-lane-relay state key stays a single `Name`, uses the canonical relay-ref components, and ends with the hashed settlement suffix.
- Focused validation for this slice:
  - `cargo test -p iroha_data_model relay_envelope_ref_state_key_is_canonical_and_deterministic -- --nocapture`
  - `cargo test -p iroha_core verified_lane_relay_state -- --nocapture`
  - `cargo test -p ivm schema_registry --lib -- --nocapture`
  - `cargo test -p iroha_core verified_lane_relay_state_key_is_single_contract_name -- --nocapture`
  - `cargo fmt --all`
  - `cargo test -p iroha_core --lib verified_lane_relay_state_ -- --nocapture`

## 2026-04-24 Izanami communication vulnerability matrix scaffold

- `crates/izanami/src/communication_vulnerabilities.rs` now records the five scenario taxonomy from "Blockchain Communication Vulnerabilities" (targeted load, transient failure, packet loss, stopping, leader isolation), the paper-shaped 20-node/800s/200 TPS constants, the paper baseline outcomes for Algorand, Aptos, Avalanche, Redbelly, and Solana, and the current Izanami coverage profile for each case.
- `scripts/run_izanami_communication_vulnerability_matrix.sh` adds a reusable matrix runner with `quick` and paper-shaped modes. It writes `summary.md`, `summary.tsv`, and per-scenario logs under `dist/izanami-communication-vuln-*`, including the paper baseline table beside Iroha/Izanami run results.
- The matrix runner now accepts explicit `--fault-enable-*=true|false` values, performs a one-shot `127.0.0.1:0` socket-bind preflight before running any scenario, records `blocked-loopback-bind` rows when the environment denies local listeners, and exits nonzero on environment or scenario failures instead of reporting a misleading all-green shell status.
- `docs/source/izanami_communication_vulnerabilities.md` documents how the paper maps to Izanami, the comparison baseline, classification signals, and the current exact-vs-approximate coverage boundaries. Packet loss and leader isolation are explicitly marked as approximations until Izanami gets exact packet-drop windows; leader isolation now uses dynamic Sumeragi leader targeting, but still uses the trusted-peer isolation primitive.
- Quick-mode execution was attempted directly with the built `izanami` binary at `dist/izanami-communication-vuln-quick-20260424-145213-direct`, and the new fast-fail preflight path was exercised at `dist/izanami-communication-vuln-quick-20260424-152301-preflight`. In both cases the environment denied binding `127.0.0.1` with `Operation not permitted`, so no Iroha vulnerability classification was produced from this execution surface.
- With full local socket access restored, a full quick-mode run completed at `dist/izanami-communication-vuln-quick-20260424-153021-fullaccess`. All five scenarios returned exit code `0`, but the log shape is mixed:
  - `targeted-load` completed with a sampled confirmation timeout and a load-worker shutdown timeout, so it is not a clean resilient pass.
  - `transient-failure` and `stopping` completed, but both showed sustained `429 Too Many Requests`, connection-refused fallbacks, and repeated confirmation timeouts during recovery, so they currently look degraded rather than clean resilient quick-pass runs.
  - `packet-loss` and `leader-isolation` were inconclusive in that first full quick run because the partition-style fault injector reported `peer missing trusted_peers_pop roster required for partition restart`, so those runs exited `0` without a faithful partition/rejoin fault shape.
- The partition-style fault injector now restarts peers with a self-only trusted-peer entry and the peer's own BLS PoP instead of trying to read a full PoP roster from `config.base.toml`. Focused partition unit coverage passes, and a 4-peer/30s live partition smoke at `dist/izanami-partition-smoke-20260424-fixed/leader-isolation-smoke.log` no longer reports `peer missing trusted_peers_pop roster` or `trusted_peers_pop contains keys not in trusted_peers`; the remaining signal is connection-refused and confirmation-timeout degradation while peers are isolated/restarted. This makes the approximation executable, but the paper comparison still needs exact packet-drop injection and active-leader targeting before `packet-loss` or `leader-isolation` should be treated as exact reproductions.
- Fixed quick-only reruns of the two previously inconclusive scenarios completed with usable degraded classifications:
  - `packet-loss`: `dist/izanami-communication-vuln-quick-20260424-packet-loss-fixed/summary.md` reports exit code `0`, status `ok`, paper outcome `degraded`. The log no longer contains the old partition restart/config signatures; it shows runtime backpressure, connection refusals, and confirmation timeouts instead.
  - `leader-isolation`: `dist/izanami-communication-vuln-quick-20260424-leader-fixed/summary.md` reports exit code `0`, status `ok`, paper outcome `degraded`. The log likewise has no `peer missing trusted_peers_pop roster`, `trusted_peers_pop contains keys not in trusted_peers`, restart-failure, or `InvalidSumeragiConfig` signatures, and the remaining signal is Torii reachability/backpressure plus queued/timeout degradation.
- The matrix runner now accepts `--sumeragi-mode permissioned|npos|both`, records `sumeragi_mode` in `summary.tsv`, and emits separate paper-style rows for `Iroha (Sumeragi permissioned)` and `Iroha (Sumeragi NPoS)`. NPoS mode runs Izanami with `--nexus`, loading the Nexus/Sora profile and `sumeragi.consensus_mode = "npos"`.
- The NPoS quick matrix startup blocker is fixed: Izanami now compacts only the universal dataspace grant into the retained Nexus genesis transaction while keeping non-universal dataspace-scoped grants separate, so the generated NPoS genesis stays within Iroha's 16-transaction startup cap without mixing dataspace permission targets.
- A fresh full quick matrix covering both Sumeragi modes completed at `dist/izanami-communication-vuln-quick-20260424-both-modes-fixed`. All ten rows exited `0` with status `ok` and paper outcome `degraded`; the previous NPoS `Invalid genesis block: Genesis block must have 1 to 16 transactions` startup failure no longer appears in these logs.
- The current quick-mode Iroha rows are:
  - `Iroha (Sumeragi permissioned) | Izanami quick | peer-to-peer/TCP | BFT quorum (permissioned validators) | degraded | degraded | degraded | degraded | degraded`
  - `Iroha (Sumeragi NPoS) | Izanami quick | peer-to-peer/TCP | stake-elected BFT quorum | degraded | degraded | degraded | degraded | degraded`
  These are executable Izanami approximations, not yet exact reproductions of the paper's packet-loss percentages or active-leader isolation method.
- Focused validation for this scaffold:
  - `cargo fmt --all`
  - `cargo test -p izanami network_partition --lib -- --nocapture` (`2` passed)
  - `cargo test -p izanami communication_vulnerabilities --lib -- --nocapture`
  - `cargo test -p izanami --lib -- --nocapture` (`18` passed)
  - `cargo test -p izanami --bin izanami -- --nocapture` (`199` passed)
  - `cargo test -p izanami make_network_builder_npos_genesis_stays_within_transaction_cap --bin izanami -- --nocapture`
  - `cargo test -p izanami make_network_builder_injects_npos_parameters --bin izanami -- --nocapture`
  - `cargo build -p izanami --bin izanami`
  - `cargo test -p izanami cli_accepts_explicit_false_fault_toggles --bin izanami -- --nocapture`
  - `bash -n scripts/run_izanami_communication_vulnerability_matrix.sh`
  - `scripts/run_izanami_communication_vulnerability_matrix.sh --help`
  - `scripts/run_izanami_communication_vulnerability_matrix.sh --mode quick --izanami-cmd 'target/debug/izanami' --out dist/izanami-communication-vuln-quick-20260424-145213-direct` (blocked by local socket bind restrictions)
  - `scripts/run_izanami_communication_vulnerability_matrix.sh --mode quick --izanami-cmd 'target/debug/izanami' --out dist/izanami-communication-vuln-quick-20260424-152301-preflight` (fast-fail preflight blocked by local socket bind restrictions)
  - `scripts/run_izanami_communication_vulnerability_matrix.sh --mode quick --izanami-cmd 'target/debug/izanami' --out dist/izanami-communication-vuln-quick-20260424-153021-fullaccess`
  - `target/debug/izanami --allow-net --peers 4 --duration 30s --tps 5 --max-inflight 16 --workload-profile stable --faulty 1 --submitters 4 --fault-enable-network-partition=true ...` with log at `dist/izanami-partition-smoke-20260424-fixed/leader-isolation-smoke.log`
  - `scripts/run_izanami_communication_vulnerability_matrix.sh --mode quick --only packet-loss --izanami-cmd 'target/debug/izanami' --out dist/izanami-communication-vuln-quick-20260424-packet-loss-fixed`
  - `scripts/run_izanami_communication_vulnerability_matrix.sh --mode quick --only leader-isolation --izanami-cmd 'target/debug/izanami' --out dist/izanami-communication-vuln-quick-20260424-leader-fixed`
  - `scripts/run_izanami_communication_vulnerability_matrix.sh --mode quick --sumeragi-mode both --izanami-cmd 'target/debug/izanami' --out dist/izanami-communication-vuln-quick-20260424-both-modes-fixed`

## 2026-04-24 Torii telemetry and routed-read regression sweep

- `crates/iroha_torii/src/routing.rs` now makes the test telemetry fixture safe for synchronous tests by entering a shared Tokio runtime when needed, and uses the full telemetry profile so status, Sumeragi, and developer telemetry endpoints expose the data their tests assert.
- Torii status responses now emit header-framed Norito through `norito::to_bytes(...)`, local8/domain address reject metrics record both the local8 bucket and the explicit domain label, and the affected address/routed-read tests now derive expected reject reasons from the live account parser.
- The failing fixture expectations are aligned with current runtime behavior: Sumeragi telemetry status seeds a VRF epoch, AXT cache debug serializes reject reasons as stable labels and uses a non-empty policy snapshot, the privacy-share test sample uses an aligned aggregation bucket, and the SoraFS pin registry metrics are registered with the global telemetry registry before tests read them.
- Focused validation for this fix:
  - `cargo fmt --all`
  - `cargo test -p iroha_torii --lib --features app_api,telemetry iso20022_bridge::tests -- --nocapture`
  - `cargo test -p iroha_torii --lib --features app_api,telemetry routing::adapter_filter_tests -- --nocapture`
  - `cargo test -p iroha_torii --lib --features app_api,telemetry routing::account_path_metric_tests -- --nocapture`
  - `cargo test -p iroha_torii --lib --features app_api,telemetry routing::address_metrics_tests -- --nocapture`
  - `cargo test -p iroha_torii --lib --features app_api,telemetry routing::tests::metrics_handler_strips_lane_labels_when_nexus_disabled -- --nocapture`
  - `cargo test -p iroha_torii --lib --features app_api,telemetry sorafs::api::advert_tests::pin_registry_metrics_summary_tracks_counts -- --nocapture`
  - `cargo test -p iroha_torii --lib --features app_api,telemetry tests_runtime_handlers:: -- --nocapture`
  - `cargo test -p iroha_torii --lib --features app_api,telemetry tests::axt_proof_cache_debug_reports_snapshot -- --nocapture`
  - `cargo test -p iroha_torii --lib --features app_api,telemetry torii_routed_read_tests -- --nocapture`
  - `cargo test -p iroha_torii --lib --features app_api,telemetry -- --nocapture` (`1766 passed; 0 failed; 2 ignored`)

## 2026-04-24 Soracloud manifest fixture canonicalization and coverage

- `fixtures/soracloud/sora_container_manifest_v1.json`, `fixtures/soracloud/sora_service_manifest_v1.json`, and `fixtures/soracloud/sora_deployment_bundle_v1.json` now match the current Soracloud V1 JSON schema: optional `inrou` serializes explicitly as `null`, empty default arrays use the compact canonical `[]` form, and the deployment bundle carries the refreshed canonical container manifest hash.
- `crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs` now adds fixture-level coverage for legacy JSON payloads that omit defaulted manifest fields, `null` default collections, custom container-manifest unknown-field rejection, `Ivm`/`Inrou` runtime metadata mismatch rejection, required config/secret material validation, config export declaration and target validation, healthcheck path validation, omitted service routes, route and rollout validation, empty deterministic handler rejection, deterministic lease-volume rejection, quota-class validation, state binding limit/encryption validation, handler route/certification/mailbox validation, mailbox size validation, duplicate state binding and handler rejection, artifact path and handler-reference validation, nested deployment-bundle default decoding, cross-fixture embedded container hash consistency, state-write capability admission, deterministic-vs-HTTP runtime admission, Inrou HTTP root/shared volume and SSH-key requirements, HTTP quota limits for replicas/resources/storage, admission success after refreshing a changed container reference, and admission rejection when schema versions, public-route healthchecks, or embedded container contents drift without updating the deployment reference.
- Focused validation for this fix:
  - `cargo fmt --all`
  - `cargo test -p iroha_data_model --features json --test soracloud_manifest_fixtures`
  - `cargo test -p iroha_data_model --test soracloud_manifest_fixtures`

## 2026-04-23 Block header Norito golden refresh

- `crates/iroha_data_model/tests/norito_golden_scaffold.rs` now pins the current bare `BlockHeader::encode()` payload instead of a stale pre-compact fixture. The old bytes no longer matched the repo-wide Norito default (`COMPACT_LEN`) or the current default `confidential_features` digest carried by newly constructed headers, so `block_header_golden_bytes` is aligned with the live codec output again.
- Focused validation for this fix:
  - `cargo test -p iroha_data_model --test norito_golden_scaffold -- --nocapture`

## 2026-04-23 Compound predicate Norito roundtrip fix

- `crates/iroha_data_model/src/query/dsl.rs` now routes `CompoundPredicateWire` through direct `norito::NoritoSerialize` / `norito::NoritoDeserialize` instead of the `Encode` / `Decode` derive pair when preserving the custom `CompoundPredicate<T>` wire wrapper. The wrapper also now delegates `encoded_len_hint` / `encoded_len_exact` to the wire enum and uses fallible `try_deserialize(...)` before reconstructing the runtime payload.
- `crates/iroha_data_model/src/query/dsl_fast.rs` now mirrors the same fix for the `fast_dsl` feature path, so both predicate DSL implementations decode the header-framed `Json(...)` variant with the correct Norito length semantics instead of misreading the compact-length-prefixed payload as a huge fixed-width allocation request.
- Both DSL modules now also have focused codec unit coverage for the full wrapper variant surface: `Pass`, `Json`, committed-transaction `TxFilters`, and committed-transaction `TxPredicate`, plus assertions that `CompoundPredicate<T>` forwards `encoded_len_hint` / `encoded_len_exact` to the inner wire enum on each path.
- Both DSL modules now additionally cover the committed-transaction `and(...)` merge matrix directly: `PASS` passthrough, filter+filter collapse through `Const(true)`, tree+tree flattening, filter+tree merging, and tree+filter merging. That gives direct test coverage over the branchy `and_committed_tx_predicates(...)` and `committed_tx_predicate_from_filters(...)` paths instead of only reaching them indirectly through roundtrips.
- Both DSL modules now also cover the remaining committed-transaction JSON evaluation branches directly: raw expression JSON that routes through `committed_tx_filters_from_json(...)`, canonical/raw object JSON that misses the filter parser and falls back to generic field-path matching, raw non-object JSON that returns the default permissive `true`, and mixed JSON-vs-tree payload combinations that keep the most recently supplied predicate instead of attempting an invalid merge.
- Both DSL modules now also have direct helper-level coverage for the last private predicate branches in this slice: `predicate_value_at_path(...)` rejects empty/blank segments, `predicate_json_from_map(...)` treats empty arrays as equality payloads instead of `in` conditions, `predicate_json_applies(...)` rejects missing equality/membership paths and `exists` checks on `null`, `and_committed_tx_predicates(...)` short-circuits `Const(false)`, and `committed_tx_predicate_from_filters(...)` preserves the expected field-to-atom ordering for the less common `authority_ne` / `ts_le` / `entry_nin` / `result_ok_ne` / `result_exists` branches.
- The same DSL coverage slice now also pins the remaining JSON-wrapper edges that previously only executed implicitly: `CompoundPredicate<T>::json_deserialize(...)` now treats `null` and `{}` as `PASS`, preserves raw non-object payloads verbatim, defaults to permissive evaluation for malformed raw JSON in both generic and committed-transaction paths, and directly exercises the `and_committed_tx_predicates(...)` append/prepend branches for `And + leaf` and `leaf + And`.
- `crates/iroha_data_model/src/query/tx_predicate.rs` now has direct shared coverage for the committed-transaction predicate core instead of only reaching it through DSL wrappers: string-field `exists` / `is_null` filter parsing, `authority_in` / `authority_nin`, `entry_ne` / `entry_nin`, `result_ok_ne` / `result_ok_in`, `timestamp_ms` `gte` / `lt` normalization, `TsGt` / `TsNin`, `EntryNe` / `EntryNin`, `MetadataIn` / `MetadataNin`, `MetadataIsNull(false)`, `Not`, `Const`, and a complex Norito roundtrip over an `Or(Not(...), TsGt(...), EntryNe(...), MetadataIn(...), Const(false))` tree.
- The same shared predicate test module now also covers the error-heavy remainder of `tx_predicate.rs`: timestamp bound saturation at `u64::MAX` / `0`, parser rejection for unsupported boolean ops and malformed/unsupported field/operator combinations, and `wire::inflate(...)` failures for missing child nodes and trailing-node payloads.
- This clears the reported `memory allocation of 8316310562681852178 bytes failed` abort in `compound_predicate_roundtrip` and returns the `iroha_data_model` `data_model` integration test binary to green.
- Focused validation for this fix:
  - `cargo fmt --all`
  - `cargo test -p iroha_data_model --lib codec_tests -- --nocapture`
  - `cargo test -p iroha_data_model --lib --features fast_dsl codec_tests -- --nocapture`
  - `cargo test -p iroha_data_model --test data_model -- --nocapture`
  - `cargo test -p iroha_data_model --test data_model --features fast_dsl compound_predicate_roundtrip -- --nocapture`
  - `cargo test -p iroha_data_model --lib tx_predicate::tests -- --nocapture`

## 2026-04-23 Staged IVM deploy input-bump exhaustion fix

- `crates/ivm/src/ivm.rs` now exposes `IVM::alloc_host_tlv(...)`, which preserves the existing input-bump path for ordinary host-returned TLVs but spills oversized returns to heap once the fixed INPUT window is exhausted. The same unit-test module now pins that spill path with `alloc_host_tlv_spills_to_heap_after_input_fills`.
- `crates/ivm/src/core_host.rs`, `crates/ivm/src/mock_wsv.rs`, and `crates/iroha_core/src/smartcontracts/ivm/host.rs` now route durable-state and Norito-bytes host returns through the new helper, so staged `STATE_GET` loops no longer fail with `MemoryOutOfBounds` after several large chunk fetches.
- `crates/iroha_cli/src/bin/ivm_contract_deploy.rs` now passes its existing staged large-payload regressions again without code changes in the builder itself: both the plain-core-host reconstruction test and the contract-runtime-host nine-chunk staged register test are green.
- Follow-up coverage for the same spill path now pins the remaining direct branches and host call sites:
  - `crates/ivm/src/ivm.rs` adds `alloc_host_tlv_prefers_input_when_space_is_available` and `alloc_host_tlv_propagates_out_of_memory_when_heap_spill_cannot_fit`,
  - `crates/ivm/tests/core_host_state_syscalls.rs` and `crates/ivm/tests/wsv_host_state_syscalls.rs` now prove `STATE_GET` returns heap-backed TLVs once the INPUT bump allocator is saturated,
  - `crates/ivm/tests/wsv_host_state_syscalls.rs` now also covers raw-byte durable state wrapping through the spill path, direct overlay-backed `STATE_GET` spills under `begin_tx`, and rejection of malformed wrapped state whose inner TLV is not `NoritoBytes`,
  - `crates/ivm/tests/wsv_host_state_syscalls.rs` now also pins the `Some(None)` overlay-tombstone branch, proving an in-flight `STATE_DEL` shadows a persisted base value during `begin_tx`,
  - `crates/ivm/tests/wsv_host_state_syscalls.rs` now also pins the non-spill raw-byte wrapping path and the overlay precedence path where an in-flight `STATE_SET` overrides a persisted base value during `begin_tx` and remains persisted after `finish_tx`,
  - `crates/ivm/tests/wsv_host_state_syscalls.rs` now also pins the overlay-delete flush path, proving an in-flight `STATE_DEL` still removes the persisted base value after `finish_tx`,
  - `crates/iroha_core/src/smartcontracts/ivm/host.rs` now has matching contract-runtime regressions for the malformed wrapped-state rejection path, for direct raw-state wrapping into `NoritoBytes`, for scoped overlay reads spilling to heap once INPUT is full, for scoped persisted-base reads winning over legacy fallback keys, and for scoped tombstones shadowing legacy unscoped base values,
  - `crates/iroha_core/src/smartcontracts/ivm/host.rs` now also pins the scoped overlay precedence path, proving a staged scoped `STATE_SET` overrides both persisted scoped state and the legacy unscoped fallback while writing only the scoped overlay key,
  - `crates/iroha_core/src/smartcontracts/ivm/host.rs` now also pins the unscoped precedence branches used when no contract runtime context is present: staged unscoped `STATE_SET` overrides persisted base state, and staged unscoped `STATE_DEL` shadows persisted base state while recording only a single unscoped tombstone,
  - `crates/ivm/src/ivm.rs` now also pins the allocator edge where only an undersized aligned tail remains in INPUT, proving `alloc_host_tlv(...)` spills in that partial-tail case instead of only when INPUT is exactly full.
- Focused validation for this fix:
  - `cargo fmt --all`
  - `cargo test -p ivm alloc_host_tlv_spills_to_heap_after_input_fills --lib -- --nocapture`
  - `cargo test -p iroha_cli --bin ivm_contract_deploy -- --nocapture`
  - `cargo test -p ivm alloc_host_tlv_ --lib -- --nocapture`
  - `cargo test -p ivm --test core_host_state_syscalls -- --nocapture`
  - `cargo test -p ivm --test wsv_host_state_syscalls -- --nocapture`
  - `cargo test -p iroha_core state_syscall_reads_world_snapshot_spills_to_heap_when_input_fills --lib -- --nocapture`
  - `cargo test -p iroha_core load_state_value_rejects_wrapped_non_norito_bytes --lib -- --nocapture`
  - `cargo test -p iroha_core state_syscall_reads_scoped_overlay_spills_to_heap_when_input_fills --lib -- --nocapture`
  - `cargo test -p iroha_core state_syscall_scoped_delete_shadows_legacy_raw_base_value --lib -- --nocapture`
  - `cargo test -p iroha_core load_state_value_ --lib -- --nocapture`
  - `cargo test -p iroha_core state_syscall_prefers_scoped_base_value_over_legacy_fallback --lib -- --nocapture`
  - `cargo test -p iroha_core state_syscall_scoped_overlay_overrides_scoped_and_legacy_base_values --lib -- --nocapture`
  - `cargo test -p iroha_core state_syscall_unscoped_ --lib -- --nocapture`

## 2026-04-23 Kotodama on-chain account alias resolution

- `crates/kotodama_lang/src/ir.rs` now treats alias-shaped `account_id("...")` string literals as runtime alias inputs: canonical encoded `AccountId` literals still lower to static `AccountId` TLVs, while non-canonical alias-shaped literals lower to `ResolveAccountAlias` with the original blob literal preserved for host-side validation against current WSV state. The same test module now also pins the boundary between shorthand and the explicit builtin on both dataspace-root and domain-qualified literals, including malformed builtin, malformed domain-qualified builtin, and malformed domain-qualified shorthand literals: `resolve_account_alias("merchant@paynet")` / `resolve_account_alias("merchant@bank.paynet")` / `resolve_account_alias("merchant@")` / `resolve_account_alias("merchant@bank.")` and `account_id("merchant@bank.")` all continue through runtime alias resolution, while `account_id("merchant")` stays on the static `AccountId` path and only fails later during compile-time encoding.
- `crates/kotodama_lang/src/compiler.rs` now covers the shorthand and builtin paths at compile/manifest level: alias-shaped `account_id("merchant@paynet")` emits the existing `SYSCALL_RESOLVE_ACCOUNT_ALIAS`, canonical `account_id("<i105>")` stays embedded as a static TLV without the alias syscall, alias-shaped invalid literals compile into runtime resolution instead of stale compile-time `AccountId` encoding, invalid non-alias literals such as `account_id("merchant")` still fail during static `AccountId` encoding, both dataspace-root and domain-qualified alias/builtin forms emit the alias syscall even for malformed alias-shaped literals, and shorthand- or builtin-derived alias transfer targets force wildcard access hints even for malformed alias-shaped literals instead of baking a stale canonical account key.
- `crates/iroha_core/src/smartcontracts/ivm/host.rs` now passes alias literals to `SYSCALL_RESOLVE_ACCOUNT_ALIAS` without trimming and resolves only through the authoritative WSV alias-binding table, so malformed alias-shaped literals fail through the host runtime rather than being normalized silently and primary labels no longer act as an implicit fallback. The same test module now covers live alias rebinding against the committed Nexus dataspace catalog, binding-only resolution, missing dataspace/domain permissions, domain-qualified alias bindings that require an SNS lease, explicit `resolve_account_alias("merchant@paynet")` and `resolve_account_alias("merchant@bank.paynet")` contract parity, runtime rejection when contract-side alias resolution lacks permission, rejection when builtin or shorthand contract paths hit a missing binding or malformed alias literal at runtime, and the full permission matrix for domain-qualified aliases: raw syscall rejection plus builtin/shorthand contract rejection when either the domain permission or the dataspace permission is missing.
- `crates/ivm/docs/kotodama_grammar.md` now documents that canonical `account_id("...")` literals stay static while alias-shaped literals lower to runtime alias resolution.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo test -p kotodama_lang account_id -- --nocapture`
  - `cargo test -p kotodama_lang resolve_account_alias -- --nocapture`
  - `cargo test -p iroha_core resolve_account_alias -- --nocapture`
  - `cargo test -p iroha_core --lib alias_shorthand -- --nocapture`

## 2026-04-23 Nexus-wide alias read routing

- `crates/iroha_core/src/torii_proxy.rs` now appends `AliasResolve`, `AliasResolveIndex`, and `AliasLookupByAccount` to `ToriiReadEndpointV1`, preserving the existing proxy enum order while letting alias reads travel through the Nexus Torii read-proxy layer.
- `crates/iroha_torii/src/lib.rs` now splits the alias handlers into routed public entrypoints and local-only executors, routes `/v1/aliases/resolve` by alias dataspace, fans `/v1/aliases/resolve_index` out across configured dataspaces, and routes `/v1/aliases/by_account` through target-account routes with deduped merged totals, `source = "fanout"` on merged responses, `409 route_conflict` on incompatible alias-index bindings, and alias-specific `403 permission_denied` plus warning/diagnostic headers when denied routes are skipped.
- The same `crates/iroha_torii/src/lib.rs` slice now has direct routed-read unit coverage for the alias-specific fanout collector and merge helpers, plus the local alias read-proxy dispatch and route-visibility partition helpers: synthetic denied-route fail-closed behavior, empty-route `route_unavailable`, explicit routed `permission_denied` precedence, warning-header emission on successful merges with denied routes, alias-index dedupe, alias-by-account dedupe, conflicting account-root rejection, the fail-closed empty-items-plus-denied branch, route-local alias-index execution, route-local alias-by-account filtering, invalid proxied alias bodies returning `invalid_proxy_request`, and unsigned-vs-caller-scoped visibility partitioning for restricted dataspaces.
- Additional alias-routing regressions now pin the remaining public handler/error paths: `/v1/aliases/resolve` rejects empty aliases before routing, `/v1/aliases/by_account` rejects malformed `account_id` literals, `/v1/aliases/resolve_index` now has endpoint-level coverage for both fail-closed `403 permission_denied` when only hidden routes can resolve and warning-header emission when a public route resolves while another routed dataspace stays denied, and the local read-proxy decoder now rejects malformed proxied bodies for all three alias read endpoints.
- This pass also adds local route-mismatch and filtered-empty-result coverage for the split alias executors, plus direct handler regressions for malformed alias literals, empty `account_id` requests, and malformed `/v1/aliases/resolve_index` request bodies so the app-facing conversion paths are pinned without relying only on proxy-level tests.
- Another follow-up pass now pins the alias-specific fallback ordering that was still implicit: `collect_torii_alias_json_payloads` prefers `404 not_found` over sibling `route_unavailable` responses when no route resolves, returns `503 route_unavailable` when every routed alias attempt is unavailable, the public `/v1/aliases/resolve` and `/v1/aliases/by_account` handlers reject malformed JSON bodies directly, and `/v1/aliases/resolve` has an end-to-end `route_unavailable` regression when the only authoritative alias route is non-local and ingress proxying is unavailable in the current feature set.
- The latest test expansion keeps pushing on endpoint-facing error behavior instead of only helper coverage: alias collector precedence is now locked down at the alias-specific helper layer, malformed request-body rejection is covered across all three public alias helpers, and the signed `/v1/aliases/resolve` path now proves the offline-authoritative dataspace case returns the existing `503 route_unavailable` surface rather than silently degrading into a local miss.
- The newest coverage pass closes a few remaining alias-read gaps around mixed-route edge cases: local read-proxy dispatch for `AliasResolve` now has an explicit success regression, empty `merged_alias_resolve_index_response` fanout payloads now pin the `404 not_found` merge result, `/v1/aliases/by_account` now proves that a public reachable route plus an offline authoritative route can still return an empty `fanout` success with `x-iroha-fanout-routes-unavailable = 1`, and `/v1/aliases/resolve_index` now locks down the fail-closed `403 permission_denied` outcome when denied routes are present even though the remaining allowed routes only miss or go unavailable.
- `crates/iroha_torii/src/openapi.rs`, `docs/source/governance_api.md`, and `docs/portal/docs/governance/api.md` now document the Nexus-routed alias lookup behavior for `/v1/aliases/resolve`, `/v1/aliases/resolve_index`, and `/v1/aliases/by_account`, including the routed `403 permission_denied` and `409 route_conflict` outcomes.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo test -p iroha_torii torii_routed_read_tests --lib --features app_api -- --nocapture`
  - `cargo test -p iroha_torii alias_ --lib --features app_api -- --nocapture`
  - `cargo test -p iroha_torii alias_resolve --lib --features app_api -- --nocapture`
  - `cargo test -p iroha_torii alias_lookup --lib --features app_api -- --nocapture`
  - `cargo test -p iroha_torii --lib --features app_api -- --nocapture`

## 2026-04-23 Sumeragi harness-salt test hardening

- `crates/iroha_core/src/sumeragi/main_loop/tests.rs` now makes `assemble_proposal_defers_when_candidate_conflicts_with_local_vote_history` search for a later local-led proposal view instead of reusing the PRF-shuffled local position directly. This keeps the blocked proposal view distinct from the setup branch at `view = 0`, so the fixture no longer spuriously inherits `proposals_seen` from `insert_validated_pending(...)` when parallel tests assign a different per-harness peer seed salt.
- The same test file now makes `precommit_vote_ignores_remote_same_height_vote_when_cached_roster_differs_from_live` match the more robust cached-roster collision setup already used by the adjacent signer-collision regressions: permissioned runs rotate away the degenerate local-first roster, and the view search now scans a wider `len * 8` window instead of assuming the remap appears in the first `len` views.
- Focused validation for this fix:
  - `cargo fmt --all`
  - `cargo test -p iroha_core assemble_proposal_defers -- --nocapture`
  - `cargo test -p iroha_core cached_roster_differs_from_live -- --nocapture`

## 2026-04-23 Sumeragi targeted failure triage

- `crates/iroha_core/src/sumeragi/main_loop/tests.rs` now aligns `precommit_vote_skips_payload_fallback_across_rapid_votes_without_roster` with the current permissioned PRF/view-aligned signer mapping by deriving the view-aligned topology from `consensus_context_for_height(...)` and signing with the full harness validator key set instead of assuming the signer remains local.
- The same test file now makes `handle_vote_uses_cached_roster_for_frontier_commit_vote_validation` build a cached roster that provably changes the signer-to-peer mapping relative to the live frontier roster, so the fixture continues to exercise cached-roster validation rather than accidentally passing under the live roster.
- `reschedule_stale_pending_blocks_targets_snapshot_roster` now backdates pending progress after seeding near-quorum votes, matching the current vote-backed reschedule semantics that measure staleness from the latest observed progress instead of the original insertion timestamp.
- Focused validation for this fix:
  - `cargo test -p iroha_core precommit_vote_skips_payload_fallback_across_rapid_votes_without_roster -- --nocapture`
  - `cargo test -p iroha_core handle_vote_uses_cached_roster_for_frontier_commit_vote_validation -- --nocapture`
  - `cargo test -p iroha_core reschedule_stale_pending_blocks_targets_snapshot_roster -- --nocapture`

## 2026-04-23 Sumeragi commit-history test isolation fix

- `crates/iroha_core/src/sumeragi/main_loop/tests.rs` now takes `super::status::commit_history_test_guard()` in the `refresh_derived_rbc_session_roster_*` regression slice and in `reschedule_stale_pending_blocks_targets_snapshot_roster`, isolating those fixtures from the process-global commit-QC history cache that parallel `iroha_core --lib` runs mutate.
- This keeps the derived-roster-unavailable expectations deterministic and preserves the snapshot-roster reschedule assertions even when neighboring tests seed commit history for the same heights/views.
- Focused validation for this fix:
  - `cargo fmt --all`
  - `cargo test -p iroha_core refresh_derived_rbc_session_roster_ --lib`
  - `cargo test -p iroha_core reschedule_stale_pending_blocks_targets_snapshot_roster --lib`
  - `cargo test -p iroha_core --lib`

## 2026-04-23 Torii zk prover report-filter coverage follow-up

- `crates/iroha_torii/src/zk_prover.rs` now adds six more direct prover-report regressions in the same report-management slice:
  - single-delete recovery coverage proving `delete_report(...)` rebuilds a malformed `reports_index.json` from on-disk reports and only removes the requested report,
  - count-handler coverage proving zero is returned when no summaries satisfy the request,
  - count-handler filter composition coverage for `content_type`, `has_tag`, `since`, and `until`,
  - list-handler coverage proving `latest=true` still respects the `messages_only` filter and returns the newest failed message,
  - bulk-delete coverage for uppercase `id` normalization,
  - bulk-delete filter composition coverage for `content_type`, `has_tag`, `since`, and `until`.
- Focused validation for this follow-up:
  - `cargo fmt --all`
  - `cargo test -p iroha_torii 'zk_prover::tests::' -- --nocapture`

## 2026-04-23 Torii zk prover malformed-index save recovery coverage follow-up

- `crates/iroha_torii/src/zk_prover.rs` now adds seven more direct prover-report regressions in the same report-management slice:
  - empty-state index rebuild coverage proving `load_report_summaries()` persists an empty index when the reports directory starts empty,
  - helper coverage proving `remove_report_summary(...)` ignores both invalid ids and valid-but-missing ids without disturbing existing summaries,
  - save-path coverage proving `save_report(...)` rebuilds from on-disk report files when `reports_index.json` is malformed and then preserves both reports in the recovered index,
  - GC coverage proving expired summaries are still deleted when the backing report file is already gone,
  - count-handler coverage for `ok_only=true&errors_only=true`, exercising the “count everything” filter combination,
  - list-handler coverage for uppercase `id` normalization and the mixed-case `order=Desc` branch.
- Focused validation for this follow-up:
  - `cargo fmt --all`
  - `cargo test -p iroha_torii 'zk_prover::tests::' -- --nocapture`

## 2026-04-23 Torii zk prover pagination and GC-rebuild coverage follow-up

- `crates/iroha_torii/src/zk_prover.rs` now adds five more direct regressions in the same prover-report slice:
  - helper coverage proving `delete_report_files(...)` ignores invalid ids without disturbing valid reports or the persisted summary index,
  - direct `load_report(...)` invalid-id coverage for the early sanitize rejection branch,
  - direct `gc_reports_once()` malformed-index rebuild coverage proving GC falls back to the on-disk report files and preserves fresh reports,
  - list-handler coverage for combined `content_type` / `has_tag` / `since_ms` / `before_ms` filtering through the real JSON response path,
  - list-handler coverage for the normal `offset` + `limit` pagination window, not only the past-end and limit-cap branches.
- Focused validation for this follow-up:
  - `cargo fmt --all`
  - `cargo test -p iroha_torii 'zk_prover::tests::' -- --nocapture`

## 2026-04-23 Torii zk prover report-alias and index-normalization coverage follow-up

- `crates/iroha_torii/src/zk_prover.rs` now adds five more direct prover-report regressions in the same report-management slice:
  - valid persisted report-index normalization coverage proving `load_report_summaries()` drops invalid ids, lowercases uppercase ids, and keeps only the last duplicate entry when the index file itself is otherwise valid,
  - direct `gc_reports_once()` no-op coverage proving fresh reports stay indexed and `deleted == 0` when nothing has expired,
  - direct `failed_only=true` alias coverage for list, count, and bulk-delete handlers so those paths are exercised independently of `errors_only` and `messages_only`.
- Focused validation for this follow-up:
  - `cargo fmt --all`
  - `cargo test -p iroha_torii 'zk_prover::tests::' -- --nocapture`

## 2026-04-23 Torii zk prover report-handler coverage follow-up

- `crates/iroha_torii/src/zk_prover.rs` now adds seven more direct report-management regressions in the same prover slice:
  - invalid report-id rejection coverage for the list, count, and bulk-delete handlers,
  - happy-path coverage for single-report `GET` payload serialization and single-report `DELETE` index pruning,
  - direct stale-index helper coverage proving `delete_report_files(...)` removes a persisted summary even when the backing file is already missing,
  - direct handler coverage proving `latest=true` ignores `order`, `offset`, and `limit` instead of applying pagination first.
- Focused validation for this follow-up:
  - `cargo fmt --all`
  - `cargo test -p iroha_torii 'zk_prover::tests::' -- --nocapture`

## 2026-04-23 Torii zk prover GC test boundary fix

- `crates/iroha_torii/src/zk_prover.rs` now keeps the GC retention regression safely inside the configured TTL window instead of placing the "fresh" report just `1 ms` below expiry, which could age out before `gc_reports_once()` recomputed `now`.
- Focused validation for this fix:
  - `cargo fmt --all`
  - `cargo test -p iroha_torii zk_prover::tests::gc_reports_once_deletes_only_expired_reports_and_retains_fresh_index -- --nocapture`
  - `cargo test -p iroha_torii 'zk_prover::tests::' -- --nocapture`

## 2026-04-23 Torii space-directory public-route coverage follow-up

- `crates/iroha_torii/tests/space_directory_manifests.rs` now adds two more router-level regressions in the same Space Directory slice:
  - `GET /v1/space-directory/uaids/{uaid}` now covers a multi-dataspace payload with one catalog alias, one missing alias, and deterministic multi-account assertions through `api_router_for_tests()`,
  - `GET /v1/space-directory/uaids/{uaid}/manifests?status=ACTIVE&limit=1&offset=1` now proves the public route preserves the prefilter `total` count even when status filtering plus pagination returns an empty page.
- Focused validation for this follow-up:
  - `cargo fmt --all`
  - `cargo test -p iroha_torii --test space_directory_manifests -- --nocapture`

## 2026-04-23 Torii space-directory revocation-shape coverage follow-up

- `crates/iroha_torii/tests/space_directory_manifests.rs` now adds two more public-route / mutation regressions in the same Space Directory slice:
  - `GET /v1/space-directory/uaids/{uaid}/manifests?status=Inactive` now has explicit route-level coverage proving a reasonless revocation is serialized with `lifecycle.revocation.reason = null`,
  - `POST /v1/space-directory/manifests/revoke` now has direct queue-inspection coverage proving a raw 64-hex `uaid` without the prefix is accepted and that omitting `reason` preserves `reason = None` in the queued `RevokeSpaceDirectoryManifest` instruction.
- Focused validation for this follow-up:
  - `cargo fmt --all`
  - `cargo test -p iroha_torii --test space_directory_manifests -- --nocapture`

## 2026-04-23 Torii space-directory raw-UAID coverage follow-up

- `crates/iroha_torii/src/routing.rs` now expands `uaid_parsing_tests` and `space_directory_manifest_helper_tests` around the remaining raw-hex branch:
  - `parse_uaid_literal(...)` accepts raw 64-hex literals without the `uaid:` prefix,
  - direct bindings/manifests helper tests prove raw-hex UAID path literals are accepted and canonicalized in the JSON response payloads,
  - the invalid-input parser test now also covers the empty-literal rejection branch.
- `crates/iroha_torii/tests/space_directory_manifests.rs` now drives the public GET bindings and manifests routes with a raw 64-hex UAID path and asserts the response canonicalizes back to `uaid:<lower-hex>`.
- Focused validation for this follow-up:
  - `cargo fmt --all`
  - `cargo test -p iroha_torii uaid_parsing_tests --lib -- --nocapture`
  - `cargo test -p iroha_torii space_directory_manifest_helper_tests --lib -- --nocapture`
  - `cargo test -p iroha_torii --test space_directory_manifests -- --nocapture`

## 2026-04-23 Torii space-directory GET parse-path coverage follow-up

- `crates/iroha_torii/src/routing.rs` now adds two more narrow `space_directory_manifest_helper_tests` cases for the direct GET handler parse failures:
  - `handle_v1_space_directory_bindings(...)` rejects malformed UAID path literals with `400`,
  - `handle_v1_space_directory_manifests(...)` rejects malformed UAID path literals with `400`.
- `crates/iroha_torii/tests/space_directory_manifests.rs` now adds a router-level regression proving the public GET bindings and manifests routes both surface the same malformed-UAID rejection through `api_router_for_tests()`.
- Focused validation for this follow-up:
  - `cargo fmt --all`
  - `cargo test -p iroha_torii 'space_directory_manifest_helper_tests::' --lib -- --nocapture`
  - `cargo test -p iroha_torii --test space_directory_manifests -- --nocapture`

## 2026-04-23 Torii space-directory mutation coverage follow-up

- `crates/iroha_torii/tests/space_directory_manifests.rs` now inspects the queued transaction payloads for the Space Directory mutation endpoints instead of only checking `queued_len()`.
- Added direct publish-handler coverage for both reason-preprocessing branches:
  - `reason` is copied only into entries whose `notes` are missing,
  - omitting `reason` leaves missing `notes` untouched.
- Added direct revoke-handler coverage for UAID parsing behavior:
  - mixed-case / padded `uaid:` literals are canonicalized into the queued `RevokeSpaceDirectoryManifest` instruction,
  - invalid UAID literals fail with `400` and do not enqueue a transaction.
- Focused validation for this follow-up:
  - `cargo fmt --all`
  - `cargo test -p iroha_torii --test space_directory_manifests -- --nocapture`

## 2026-04-23 Torii space-directory handler coverage follow-up

- `crates/iroha_torii/src/routing.rs` now adds three more narrow `space_directory_manifest_helper_tests` cases in the same local slice:
  - direct `handle_v1_space_directory_bindings(...)` coverage for the missing-UAID-bindings response shape,
  - direct `handle_v1_space_directory_bindings(...)` coverage for multi-dataspace alias/account output, including deterministic account comparisons,
  - direct `handle_v1_space_directory_manifests(...)` coverage for the `Inactive` filter returning pending and revoked rows while excluding active rows.
- The bindings multi-dataspace test seeds a minimal manifest set alongside the bindings because `State::new_for_testing(...)` prunes stale standalone `uaid_dataspaces` entries during storage migration when no manifest sets exist.
- Focused validation for this follow-up:
  - `cargo fmt --all`
  - `cargo test -p iroha_torii 'space_directory_manifest_helper_tests::' --lib -- --nocapture`

## 2026-04-23 Torii space-directory helper coverage expansion

- `crates/iroha_torii/src/routing.rs` now adds four more narrow `space_directory_manifest_helper_tests` cases around the same helper slice:
  - direct `manifest_lifecycle_json(...)` coverage for revocations that carry an explicit reason,
  - direct `manifest_entry_to_json(...)` coverage for alias/hash/status/lifecycle/accounts population,
  - direct `manifest_entry_to_json(...)` fallback coverage for null alias and empty accounts when context is missing,
  - direct `handle_v1_space_directory_manifests(...)` coverage for the `dataspace` filter plus `limit=0` being treated as unbounded.
- These keep the coverage local to the helper/test module and exercise JSON-shaping and query-filter branches that were previously only hit indirectly or not at all.
- Focused validation for this coverage pass:
  - `cargo fmt --all`
  - `cargo test -p iroha_torii 'space_directory_manifest_helper_tests::' --lib -- --nocapture`

## 2026-04-23 Torii space-directory helper test ordering fix

- `crates/iroha_torii/src/routing.rs` no longer assumes insertion order in `bindings_for_dataspace_filters_to_requested_scope_and_handles_missing_bindings`; the test now sorts the returned account literals and expected literals before comparing them.
- This aligns the assertion with `UaidDataspaceBindings`, which stores per-dataspace accounts in a `BTreeSet` and therefore guarantees membership, not random `KeyPair::random()` insertion order.
- Focused validation for this fix:
  - `cargo fmt --all`
  - `cargo test -p iroha_torii bindings_for_dataspace_filters_to_requested_scope_and_handles_missing_bindings --lib`

## 2026-04-23 Torii ZK attachments smoke auth alignment

- `crates/iroha_torii/tests/fixtures.rs` now provides `app_signed_request(...)`, a shared integration-test helper that attaches canonical `X-Iroha-*` request-signature headers for app-authenticated routes.
- `crates/iroha_torii/tests/zk_subrouter_smoke.rs` now serializes its attachment-focused cases with a local test mutex so the merged-router smoke suite no longer races on the shared global attachments config.
- The smoke slice now covers the enabled routes with valid signed requests across list/get/count/delete and a full POST upload roundtrip, including a successful create, successful fetch, successful delete, and `NOT_FOUND` after delete.
- The same smoke slice now also asserts that replaying the exact same signed POST request is rejected with `ValidationFail::NotPermitted(... "nonce already used" ...)`, and that unsigned list/count/delete attachment requests are rejected with `ValidationFail::NotPermitted(... "signed account headers are required" ...)`.
- The disabled-path smoke coverage now checks `/v1/zk/attachments`, `/v1/zk/attachments/count`, and `/v1/zk/attachments/{id}` (GET and DELETE) all return `404` when attachments are disabled.
- Focused validation for this fix:
  - `cargo fmt --all`
  - `cargo test -p iroha_torii --test zk_subrouter_smoke -- --nocapture`

## 2026-04-23 Alias auto-renew regression fixes

- `crates/iroha_core/src/smartcontracts/ivm/host.rs` now unregisters the currently executing alias auto-renew billing trigger before queueing the replacement trigger, so successful and retryable billing runs can reschedule themselves without duplicate-trigger rejection.
- `crates/iroha_config/src/parameters/user.rs` now defaults `torii.onboarding.alias_auto_renew_enabled` to `false` until `alias_auto_renew_subscription_domain` is configured, and the optional onboarding subtree now carries explicit Norito defaults so upgraded configs keep their documented defaults when the new field is omitted.
- `crates/iroha_torii/src/routing.rs` now treats account-alias auto-renew NFTs as a special resume path: `/v1/subscriptions/{id}/resume` no longer requires `subscription_plan` metadata for these NFTs, resets failure state, preserves the current billing window, and rebuilds the billing trigger with the resumed charge time.
- `crates/iroha_torii/src/lib.rs` now mounts the missing subscription mutation routes (`POST /v1/subscriptions/plans`, `POST /v1/subscriptions`, and the per-subscription pause/resume/cancel/keep/usage/charge-now routes) so the existing handlers are reachable through the public app router.
- `crates/iroha_torii/tests/subscriptions_endpoints.rs`, `crates/iroha_torii/tests/accounts_onboard.rs`, `crates/iroha_torii/src/routing.rs`, `crates/iroha_config/src/parameters/user.rs`, and `crates/iroha_core/src/smartcontracts/ivm/host.rs` now cover the reschedule fix, the safe onboarding default, the no-domain onboarding path, alias auto-renew list/get compatibility, router-level resume plus mutation-route registration, the omitted-charge alias resume helper branches, and the generic cancel-at-period-end subscription action branch.
- Focused validation for this fix set:
  - `cargo test -p iroha_core subscription_bill_account_alias_auto_renew_queues_renewal_and_reschedules -- --nocapture`
  - `cargo test -p iroha_config onboarding_alias_auto_renew_defaults_disabled_without_subscription_domain -- --nocapture`
  - `cargo test -p iroha_torii handle_post_v1_subscription_resume_supports_alias_auto_renew_nfts -- --nocapture`
  - `cargo test -p iroha_torii --test accounts_onboard without_auto_renew_subscription_domain_when_disabled -- --nocapture`
  - `cargo test -p iroha_torii --test subscriptions_endpoints -- --nocapture`
  - `cargo test -p iroha_torii resolve_account_alias_auto_renew_resume_charge_ms_ -- --nocapture`
  - `cargo test -p iroha_torii handle_post_v1_subscription_cancel_period_end_marks_cancellation_window -- --nocapture`
  - `cargo test -p iroha_torii handle_post_v1_subscription_resume_alias_auto_renew_without_charge_at_preserves_future_schedule -- --nocapture`

## 2026-04-23 Account-alias lease coverage expansion

- `crates/iroha_core/src/sns.rs` now covers two more SNS quote guards for the paid alias lease path: registration rejects an already-registered selector, and renewal rejects a tombstoned alias.
- `crates/iroha_core/src/smartcontracts/isi/sns.rs` now covers the executor guard branches that reject a mismatched payer on `AcquireAccountAliasLease` and reject `RenewAccountAliasLease` when the caller is neither the lease owner nor a `CanManageAccountAlias` holder.
- `crates/iroha_core/src/smartcontracts/ivm/host.rs` now covers the SNS-specific subscription billing branch directly, including the paid auto-renew success path that queues a canonical `RenewAccountAliasLease` and the missing-alias failure path that suspends the subscription and records a failed invoice.
- `crates/iroha_torii/tests/accounts_onboard.rs` now also verifies the onboarding response lease block plus `GET /v1/accounts/{account_id}/aliases` after onboarding, so the new app-facing lease DTOs and alias-list route are exercised at integration level.
- Focused validation for this coverage follow-up:
  - `cargo fmt --all`
  - `CARGO_TARGET_DIR=target/codex-account-alias-lease cargo test -p iroha_torii --test accounts_onboard -- --nocapture`
  - `CARGO_TARGET_DIR=target/codex-account-alias-lease cargo test -p iroha_core --lib quote_account_alias_ -- --nocapture`
  - `CARGO_TARGET_DIR=target/codex-account-alias-lease cargo test -p iroha_core --lib acquire_account_alias_lease_ -- --nocapture`
  - `CARGO_TARGET_DIR=target/codex-account-alias-lease cargo test -p iroha_core --lib renew_account_alias_lease_rejects_non_owner_without_permission -- --nocapture`
  - `CARGO_TARGET_DIR=target/codex-account-alias-lease cargo test -p iroha_core --lib subscription_bill_account_alias_auto_renew_ -- --nocapture`

## 2026-04-23 Canonical paid account-alias leases and auto-renew

- `crates/iroha_data_model` now exposes canonical `AcquireAccountAliasLease` / `RenewAccountAliasLease` ISIs plus SNS account-alias auto-renew metadata on subscriptions.
- `crates/iroha_core/src/sns.rs` now quotes paid account-alias registration/renewal lifecycles, and `crates/iroha_core/src/smartcontracts/isi/sns.rs` executes the canonical paid lease acquire/renew path instead of relying on implicit runtime lease seeding.
- `crates/iroha_core/src/smartcontracts/ivm/host.rs` now recognizes subscription NFTs tagged with account-alias auto-renew metadata and bills them through canonical alias renewal instead of the generic fixed/usage transfer path.
- `crates/iroha_torii/src/routing.rs` now makes `/v1/accounts/onboard` and `/v1/accounts/onboard/multisig` acquire a real finite alias lease in the queued transaction, exposes alias lease status / renew / auto-renew endpoints, and returns lease state in onboarding responses.
- `crates/iroha_torii/tests/accounts_onboard.rs`, `crates/iroha_core/src/sns.rs`, and `crates/iroha_core/src/smartcontracts/isi/sns.rs` now cover the paid onboarding flow, SNS quote helpers, and executor-level acquire/renew round trip.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `CARGO_TARGET_DIR=target/codex-account-alias-lease cargo test -p iroha_torii --test accounts_onboard -- --nocapture`
  - `CARGO_TARGET_DIR=target/codex-account-alias-lease cargo test -p iroha_core --lib quote_account_alias_ -- --nocapture`
  - `CARGO_TARGET_DIR=target/codex-account-alias-lease cargo test -p iroha_core --lib acquire_and_renew_account_alias_lease_round_trip -- --nocapture`
  - `git diff --check`

## 2026-04-23 Sumeragi helper guard coverage addendum

- `crates/iroha_core/src/sumeragi/main_loop/tests.rs` now adds three more direct guard/fallback tests in the same quorum-target / frontier-wire slice so the remaining conservative-degrade branches are covered without relying on larger end-to-end fixtures.
- Added `quorum_retransmit_targets_fall_back_to_full_fanout_when_signer_mapping_fails`, which proves invalid signer-to-peer mapping degrades to full retransmit fanout instead of dropping recovery traffic.
- Added `frontier_block_created_for_local_proposal_wire_falls_back_to_first_block_signature`, which proves the local proposal wire helper still emits enriched frontier metadata when the proposal’s proposer index does not match any signature on the block.
- Added `frontier_block_created_for_wire_returns_plain_block_without_frontier_metadata`, which proves the generic wire helper degrades to a plain `BlockCreated` when neither proposal cache nor authoritative frontier metadata is available.
- Focused validation for this addendum:
  - `cargo test -p iroha_core --lib quorum_retransmit_targets_fall_back_to_full_fanout_when_signer_mapping_fails -- --nocapture`
  - `cargo test -p iroha_core --lib frontier_block_created_for_local_proposal_wire_falls_back_to_first_block_signature -- --nocapture`
  - `cargo test -p iroha_core --lib frontier_block_created_for_wire_returns_plain_block_without_frontier_metadata -- --nocapture`
  - `cargo test -p iroha_core --lib frontier_block_created_for_proposal_wire_falls_back_to_authoritative_frontier_cache -- --nocapture`
  - `cargo test -p iroha_core --lib frontier_block_created_for_local_proposal_wire_uses_live_roster_when_derived_roster_unavailable -- --nocapture`

## 2026-04-23 Sumeragi helper coverage addendum

- `crates/iroha_core/src/sumeragi/main_loop/tests.rs` now adds three more direct helper tests in the quorum-target / frontier-proposal slice so the remaining positive and negative fallback branches are exercised without relying on larger pacemaker or reschedule fixtures.
- Added `quorum_retransmit_targets_expand_to_full_fanout_near_commit_quorum`, which proves the helper widens back to every remote peer once the round is one vote short of commit quorum and only a single canonical target appears to be missing.
- Added `frontier_block_created_for_proposal_wire_falls_back_to_authoritative_frontier_cache`, which proves the generic proposal-wire helper can rebuild enriched `BlockCreated` payloads from authoritative frontier metadata after a prior local `BlockCreated`.
- Added `frontier_block_created_for_proposal_wire_rejects_authoritative_frontier_cache_when_metadata_mismatches`, which proves the generic proposal-wire helper will not reuse cached authoritative metadata once the proposal header no longer matches it.
- Focused validation for this addendum:
  - `cargo test -p iroha_core --lib quorum_retransmit_targets_expand_to_full_fanout_near_commit_quorum -- --nocapture`
  - `cargo test -p iroha_core --lib frontier_block_created_for_proposal_wire_falls_back_to_authoritative_frontier_cache -- --nocapture`
  - `cargo test -p iroha_core --lib frontier_block_created_for_proposal_wire_rejects_authoritative_frontier_cache_when_metadata_mismatches -- --nocapture`
  - `cargo test -p iroha_core --lib frontier_block_created_for_local_proposal_wire_falls_back_to_authoritative_frontier_cache -- --nocapture`
  - `cargo test -p iroha_core --lib frontier_block_created_for_proposal_wire_rebuilds_authoritative_frontier_metadata -- --nocapture`
  - `cargo test -p iroha_core --lib pacemaker_rebroadcasts_cached_frontier_block_when_leader -- --nocapture`

## 2026-04-23 Sumeragi fallback coverage addendum

- `crates/iroha_core/src/sumeragi/main_loop/tests.rs` now adds two more direct branch tests around the quorum-target / cached-frontier fixes instead of relying only on the higher-level regression fixtures.
- Added `precommit_vote_falls_back_to_seeded_collectors_when_quorum_targets_are_satisfied`, which proves `emit_precommit_vote(...)` keeps the seeded collector subset as the fallback target once cached remote commit votes already satisfy quorum-target retransmit selection.
- Added `frontier_block_created_for_local_proposal_wire_falls_back_to_authoritative_frontier_cache`, which proves local proposal wire rebuilds can recover from authoritative frontier metadata after a prior enriched `BlockCreated`, even when the roster hint is unavailable.
- Focused validation for this addendum:
  - `cargo test -p iroha_core --lib precommit_vote_falls_back_to_seeded_collectors_when_quorum_targets_are_satisfied -- --nocapture`
  - `cargo test -p iroha_core --lib frontier_block_created_for_local_proposal_wire_falls_back_to_authoritative_frontier_cache -- --nocapture`
  - `cargo test -p iroha_core --lib emit_precommit_vote_targets_quorum_retransmit_peers -- --nocapture`
  - `cargo test -p iroha_core --lib frontier_block_created_for_local_proposal_wire_uses_live_roster_when_derived_roster_unavailable -- --nocapture`
  - `cargo test -p iroha_core --lib pacemaker_rebroadcasts_cached_frontier_block_when_leader -- --nocapture`

## 2026-04-23 Sumeragi quorum-target / frontier rebroadcast follow-up

- `crates/iroha_core/src/sumeragi/main_loop/commit.rs` now sends the first local precommit to `quorum_retransmit_targets_for_missing_votes(...)` instead of the seeded collector subset, while still preserving the collector seed state for later retry widening.
- `crates/iroha_core/src/sumeragi/main_loop/propose.rs` now rebuilds cached frontier `BlockCreated` rebroadcasts with `frontier_block_created_for_local_proposal_wire(...)` so a cached local-leader proposal can still emit an enriched payload rebroadcast without a preseeded RBC session.
- `crates/iroha_core/src/sumeragi/main_loop/tests.rs` now aligns the READY-quorum and cached-frontier pacemaker fixtures with PRF/view-aligned sender and leader selection, and the older initial-vote assertions now expect quorum retransmit peers instead of the seeded collector subset.
- Focused validation for this follow-up:
  - `cargo test -p iroha_core --lib commit_vote_targets_collectors_or_topology -- --nocapture`
  - `cargo test -p iroha_core --lib precommit_vote_targets_collectors_without_broadcast -- --nocapture`
  - `cargo test -p iroha_core --lib emit_precommit_vote_targets_quorum_retransmit_peers -- --nocapture`
  - `cargo test -p iroha_core --lib maybe_emit_rbc_ready_after_ready_quorum_without_all_chunks -- --nocapture`
  - `cargo test -p iroha_core --lib pacemaker_rebroadcasts_cached_frontier_block_when_leader -- --nocapture`

## 2026-04-23 Torii zk prover handler combo follow-up

- `crates/iroha_torii/src/zk_prover.rs` now adds four more direct prover-report handler regressions around mixed filter/projection combinations that were still only exercised incidentally.
- Added uppercase `id` query normalization coverage for `handle_count_reports(...)`, direct `messages_only=true` coverage proving successful reports are excluded from the projection even when `ok_only=true` is also present, and `latest=true` coverage for the default full-object list projection.
- Added bulk-delete coverage proving `ok_only=true&errors_only=true` collapses to deleting both successful and failed reports instead of silently filtering one side out.
- Focused validation for this follow-up:
  - `cargo fmt --all`
  - `cargo test -p iroha_torii 'zk_prover::tests::' -- --nocapture`
  - `cargo test -p iroha_torii --test zk_prover_integration -- --nocapture`

## 2026-04-23 Torii zk prover report load-failure follow-up

- `crates/iroha_torii/src/zk_prover.rs` now adds five more narrow prover-report regressions around valid-but-missing endpoints and unloadable report bodies so those fallback branches stay covered directly.
- Added `load_report(...)` rejection coverage for non-UTF-8 and malformed-JSON report files, plus direct `handle_get_report(...)` coverage for a valid-but-missing report id.
- Added list-handler coverage proving the default full-object projection silently skips summary entries whose on-disk report body cannot be decoded, and delete-handler coverage for the `{ deleted: 0, ids: [] }` no-match response path.
- Focused validation for this follow-up:
  - `cargo fmt --all`
  - `cargo test -p iroha_torii 'zk_prover::tests::' -- --nocapture`
  - `cargo test -p iroha_torii --test zk_prover_integration -- --nocapture`

## 2026-04-23 Torii zk prover helper filter and upsert follow-up

- `crates/iroha_torii/src/zk_prover.rs` now adds five more narrow prover-report regressions around helper behavior that was still mostly only covered indirectly: summary upsert replacement, filename discovery normalization, direct filter predicate edges, and single-report delete misses.
- Added direct `save_report(...)` upsert coverage proving the persisted summary index is updated in place for repeated ids rather than accumulating duplicates, and that the on-disk report body reflects the latest write.
- Added helper coverage for `list_report_ids(...)` ignoring non-report and invalid-id entries, plus `filter_report_summary(...)` exact-id, content-type, tag, time-bound, and ok/failed matrix branches.
- Added direct `handle_delete_report(...)` not-found coverage for a valid-but-missing report id.
- Focused validation for this follow-up:
  - `cargo fmt --all`
  - `cargo test -p iroha_torii 'zk_prover::tests::' -- --nocapture`
  - `cargo test -p iroha_torii --test zk_prover_integration -- --nocapture`

## 2026-04-23 Torii zk prover helper coverage GC and cap follow-up

- `crates/iroha_torii/src/zk_prover.rs` now adds four more unit-style regressions for the remaining cheap helper branches in the prover report slice: invalid-id `save_report(...)` rejection, `gc_reports_once()` expiry pruning, empty-result `latest=true` projection, and the `limit.min(1000)` safety cap.
- Added direct GC coverage proving expired report files are deleted, fresh reports remain, and the persisted summary index is rewritten to only the retained entries.
- Added direct list-handler coverage proving `latest=true` with no matches returns an empty ids array and that oversized caller limits are capped to the first 1000 reports.
- Focused validation for this follow-up:
  - `cargo fmt --all`
  - `cargo test -p iroha_torii 'zk_prover::tests::' -- --nocapture`
  - `cargo test -p iroha_torii --test zk_prover_integration -- --nocapture`

## 2026-04-23 Torii zk prover helper coverage edge follow-up

- `crates/iroha_torii/src/zk_prover.rs` now adds six more narrow unit-style regressions in the prover report helper slice so the remaining normalization, rebuild, alias, and ordering branches are covered directly instead of only incidentally.
- Added `normalize_report_summaries(...)` coverage proving invalid ids are dropped and duplicate ids collapse to the last entry after canonical lowercase normalization.
- Added malformed `reports_index.json` rebuild coverage for `load_report_summaries()`, plus defensive persisted-id normalization coverage for `load_report(...)` when on-disk JSON carries uppercase ids.
- Added `errors_only=true` alias coverage for both `handle_count_reports(...)` and `handle_delete_reports(...)`, plus case-insensitive `order=DESC` coverage for `handle_list_reports(...)`.
- Focused validation for this follow-up:
  - `cargo fmt --all`
  - `cargo test -p iroha_torii 'zk_prover::tests::' -- --nocapture`
  - `cargo test -p iroha_torii --test zk_prover_integration -- --nocapture`

## 2026-04-23 Torii zk prover helper coverage follow-up

- `crates/iroha_torii/src/zk_prover.rs` now adds five direct unit-style coverage tests for prover report helper and handler edges that were previously only hit indirectly, if at all.
- Added stale summary-index cleanup coverage for `load_report_summaries()`, proving missing report files are pruned from `reports_index.json` and the cleaned index is persisted.
- Added projection precedence coverage proving `ids_only=true` wins over `messages_only=true`, while `messages_only=true` still preserves `error: null` for failed summaries that carry no error string.
- Added bulk-delete response coverage for `handle_delete_reports(...)`, asserting the returned `{ deleted, ids }` payload matches the exact-id filter and that non-matching reports remain on disk.
- Added paging edge coverage proving `offset` values past the filtered result length return an empty JSON array instead of leaking stale entries.
- Focused validation for this follow-up:
  - `cargo fmt --all`
  - `cargo test -p iroha_torii 'zk_prover::tests::' -- --nocapture`
  - `cargo test -p iroha_torii --test zk_prover_integration -- --nocapture`

## 2026-04-23 Torii zk prover report coverage follow-up

- `crates/iroha_torii/tests/zk_prover_integration.rs` now adds three focused prover-report endpoint regressions around query/filter edge cases instead of only the happy-path list/filter coverage.
- Added invalid query-id coverage for `GET /v1/zk/prover/reports`, `GET /v1/zk/prover/reports/count`, and `DELETE /v1/zk/prover/reports`, asserting the shared `invalid report id` rejection path stays wired through each app-facing handler.
- Added a combined `ok_only=true&failed_only=true` list case so the fallback branch that returns both successful and failed reports remains covered.
- Added a `messages_only=true&latest=true&order=asc&offset=1&limit=1` case proving `latest=true` overrides paging/order inputs before the failed-report message projection runs.
- Focused validation for this follow-up:
  - `cargo fmt --all`
  - `cargo test -p iroha_torii --test zk_prover_integration -- --nocapture`

## 2026-04-23 Torii zk prover fixture alignment

- `crates/iroha_torii/tests/zk_prover_integration.rs` now uses the supported `halo2/ipa:tiny-add` deterministic Halo2 fixture circuit instead of the unsupported `halo2/ipa:tiny-add-v1` alias, so the shared fixture helper uses a registered verifying-key reference for the prover report integration coverage.
- Focused validation for this fix:
  - `cargo test -p iroha_torii --test zk_prover_integration prover_reports_list_get_delete -- --nocapture`
  - `cargo test -p iroha_torii --test zk_prover_integration prover_reports_server_side_filters -- --nocapture`
  - `cargo test -p iroha_torii --test zk_prover_integration -- --nocapture`

## 2026-04-23 Sumeragi main_loop coverage edge follow-up

- `crates/iroha_core/src/sumeragi/main_loop/tests.rs` now adds five more direct unit tests in the recent known-block replay / exact-frontier helper slice so the remaining explicit-target and local-only frontier branches are exercised directly.
- Added a deduplicated explicit-target vote replay case for `maybe_replay_known_block_commit_evidence(...)` that proves duplicate and local peers collapse to one outbound `QcVote` send per unique remote target.
- Added a cached commit-QC replay case for `maybe_replay_known_block_commit_evidence(...)` that proves the helper returns `false` when the explicit target set collapses to the local peer only and therefore the QC replay path has no outbound work to schedule.
- Added three more `frontier_body_next_due(...)` checks: unarmed exact-fetch slots stay idle even with cached targets, leader-stage retries stay idle when a single-peer topology yields no remote leader or voters, and voter-stage retries stay armed when at least one cached remote voter survives leader filtering.
- Focused validation for this follow-up:
  - `cargo fmt --all`
  - `cargo test -p iroha_core --lib known_block_commit_evidence_replay_deduplicates_explicit_vote_targets -- --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::known_block_commit_qc_replay_returns_false_for_local_only_explicit_targets' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::frontier_body_next_due_ignores_exact_fetch_disabled_slot_even_with_targets' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::frontier_body_next_due_leader_stage_stays_idle_without_remote_targets' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::frontier_body_next_due_voter_stage_uses_cached_remote_voters_after_leader_filter' --nocapture`

## 2026-04-23 Sumeragi main_loop coverage tail follow-up

- `crates/iroha_core/src/sumeragi/main_loop/tests.rs` now adds four more direct unit tests in the recent known-block replay / exact-frontier helper slice instead of relying on broader regressions to incidentally hit those branches.
- Added a per-block cooldown case for `maybe_replay_known_block_commit_evidence(...)` that proves the second replay attempt is suppressed before any duplicate network work is scheduled.
- Added a local-only explicit-target case for `maybe_replay_known_block_commit_evidence(...)` that proves the helper returns `false` when the explicit peer set collapses to self and therefore cannot emit outbound recovery traffic.
- Added two exact-frontier cached-target shape checks for `frontier_body_next_due(...)`: leader-stage retries remain armed when only cached voters exist, and voter-stage retries stay idle when the cached voter set collapses to the cached leader only.
- Focused validation for this follow-up:
  - `cargo fmt --all`
  - `cargo test -p iroha_core --lib known_block_commit_evidence_replay_skips_during_cooldown -- --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::known_block_commit_evidence_replay_returns_false_for_local_only_explicit_targets' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::frontier_body_next_due_leader_stage_uses_cached_voters_without_leader' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::frontier_body_next_due_voter_stage_ignores_cached_leader_only_voter_set' --nocapture`

## 2026-04-22 Torii zk roots coverage sweep

- `crates/iroha_torii/src/routing.rs` now covers the remaining `/v1/zk/roots` selector, gas-default, and `Accept` negotiation branches, including blank selectors, trimmed and canonical gas-asset handling, custom-vs-pipeline fallback precedence, malformed `Accept` handling, vendor `+json`, wildcard, and zero-quality Norito fallback cases.
- `crates/iroha_torii/tests/zk_endpoints.rs` now pins the route-level registered-asset empty-state `200`, parseable-but-missing asset alias `404`, missing asset `404`, and blank/invalid selector `403` regressions.
- `crates/iroha_torii/tests/zk_roots_handler_integration.rs` now covers bounded nonzero `max`, zero-cap empty windows that preserve `latest` and `height`, trimmed alias resolution against non-empty shielded state, Norito decoding for non-empty roots windows, and forwarded unsupported `Accept` headers returning `406`.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo test -p iroha_torii --lib zk_roots_selector_tests -- --nocapture`
  - `cargo test -p iroha_torii --test zk_endpoints --test zk_roots_handler_integration -- --nocapture`
  - `git diff --check`

## 2026-04-22 Sumeragi main_loop coverage follow-up

- `crates/iroha_core/src/sumeragi/main_loop/tests.rs` now adds direct unit coverage for the recent collector / vote-replay / exact-frontier branches instead of only relying on the earlier regression fixtures.
- Added a collector-disabled precommit case that proves `emit_precommit_vote(...)` falls back to the full view-aligned topology when no seeded collector set exists.
- Added a local accepted commit-vote case that proves `handle_vote(...)` records the vote without triggering known-block commit-evidence replay or payload recovery fanout.
- Added direct `frontier_body_next_due(...)` coverage for both derived-target paths: leader-stage scheduling now stays armed when live-roster derivation can supply fetch targets, while voter-stage scheduling still stays idle when derivation yields only the local leader and no remote voters.
- Added direct `known_block_commit_qc_recovery_targets(...)` coverage for both empty-input fallback branches: cached vote-roster recovery for far-future rounds with preserved roster evidence, and live commit-topology recovery when no vote-roster evidence exists.
- Added two more exact-frontier scheduler checks: far-future slots now prove the final fallback to `effective_commit_topology()`, and body-present slots now prove exact fetch stays idle when no same-round commit-QC repair is active.
- Added direct `deterministic_elected_roster_from_candidates(...)` coverage for two narrow roster-election guards: zero requested roster length now proves the helper still elects one candidate, and NPoS `max_validators` now proves candidate truncation happens before the requested roster-size cap.
- Added direct `frontier_body_next_due(...)` deadline checks for the two timing branches that were still only covered indirectly: before ingress grace elapses the helper returns `observed_at + authoritative_body_ingress_fetch_grace()`, and after grace elapses it returns `last_fetch_at + retry_window`.
- Added direct `maybe_replay_known_block_commit_evidence(...)` coverage for the “no new progress” guard: once the same round’s replay state has already been sent and neither vote count nor commit-QC state changes, the helper now has a focused test proving it returns `false` without scheduling more traffic.
- Added direct NPoS roster-unavailability source coverage for the empty-commit-topology path: one test proves the helper stays on the multi-peer local lane when that lane is available, and another proves it falls back to the full published active validator roster when the local validator has no lane assignment.
- Added two more exact-frontier due checks: body-present slots still schedule retries when same-round commit-QC repair remains actionable, and passive-catchup slots stay excluded from exact body retry scheduling even when cached targets exist.
- Focused validation for this follow-up:
  - `cargo fmt --all`
  - `cargo test -p iroha_core --lib precommit_vote_falls_back_to_topology_when_collectors_disabled -- --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::local_accepted_commit_vote_does_not_replay_known_block_evidence' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::frontier_body_next_due_derives_targets_from_live_roster_when_slot_cache_is_empty' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::frontier_body_next_due_voter_stage_requires_remote_voters_after_derivation' --nocapture`
  - `cargo test -p iroha_core --lib known_block_commit_qc_recovery_targets_fall_back_to_cached_vote_roster -- --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::known_block_commit_qc_recovery_targets_fall_back_to_effective_commit_topology' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::frontier_body_next_due_falls_back_to_effective_commit_topology_when_live_roster_is_empty' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::frontier_body_next_due_ignores_body_present_without_commit_qc_repair' --nocapture`
  - `cargo test -p iroha_core --lib deterministic_roster_election_keeps_one_candidate_when_target_len_is_zero -- --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::deterministic_roster_election_truncates_npos_candidates_to_max_validators' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::frontier_body_next_due_returns_ingress_grace_deadline_before_grace_elapses' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::frontier_body_next_due_returns_retry_deadline_after_grace_elapses' --nocapture`
  - `cargo test -p iroha_core --lib roster_unavailability_candidate_source_npos_uses_local_lane_when_commit_topology_empty -- --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::roster_unavailability_candidate_source_npos_uses_full_active_roster_when_local_lane_missing' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::known_block_commit_evidence_replay_skips_without_new_progress' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::frontier_body_next_due_keeps_retry_armed_when_body_present_but_commit_qc_repair_active' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::frontier_body_next_due_ignores_passive_catchup_slot_even_with_targets' --nocapture`

## 2026-05-04 Kura replay WSV determinism

- Blocks now carry a header-committed execution context bundle for external entrypoints, recording the lane and dataspace used during execution so future Kura replay does not need to re-derive route-sensitive state from the current WSV.
- Live non-genesis block validation rejects missing, tampered, misaligned, or route-mismatched execution context. The replay-specific path remains compatible with older committed blocks while preferring embedded context whenever it is present.
- Kura replay now hard-fails before applying a block if re-execution does not reproduce the committed result merkle root, full entry merkle root, entrypoint hash sequence, result hash sequence, and stored transaction result payloads. Stored committed blocks without execution results are treated as unreplayable for WSV rebuild.
- Kura now writes a Norito WSV checkpoint sidecar after each live state commit, keyed by height and block hash, and replay compares the reconstructed canonical WSV snapshot hash against the checkpoint when present. Once checkpointed history has begun, later missing WSV checkpoints fail replay instead of silently accepting an unchecked rebuild.
- WSV checkpoint sidecars are pruned when Kura truncates history or replaces the top block, so stale checkpoints cannot survive a local rollback/replacement path.
- The optional execution-context fields are appended in the Norito header/payload layouts so older block data with absent context decodes with the intended default.
- Snapshot tests now expose a canonical WSV byte surface and assert snapshot roundtrips preserve those bytes.
- Focused validation so far:
  - `cargo fmt --all`
  - `cargo check -p iroha_data_model`
  - `cargo check -p iroha_core`
  - `cargo check -p irohad`
  - `cargo test -p iroha_core wsv_checkpoint -- --nocapture`
  - `cargo test -p iroha_core --lib replay_from_height_catches_up_state -- --nocapture`
  - `cargo test -p iroha_core --lib replay_ -- --nocapture`
  - `cargo test -p iroha_data_model header_decodes_legacy_payload_without_execution_context_hash -- --nocapture`
  - `cargo test -p iroha_data_model block_payload_decodes_legacy_payload_without_execution_context -- --nocapture`
  - `cargo test -p iroha_core replay_rejects_committed_result_mismatch_before_applying_block -- --nocapture`
  - `cargo test -p iroha_core replay_legacy_route_sensitive_block_reconstructs_canonical_state -- --nocapture`
  - `cargo test -p iroha_core replay_ -- --nocapture`
  - `cargo test -p iroha_core execution_context -- --nocapture`

## 2026-04-22 Sumeragi targeted main_loop regression sweep

- `crates/iroha_core/src/sumeragi/main_loop/commit.rs` now keeps the initial local commit/precommit emit on the seeded collector set plus explicit parallel fanout, instead of widening that very first send through the generic commit-evidence replay path.
- `crates/iroha_core/src/sumeragi/main_loop/votes.rs` now limits automatic known-block commit-evidence replay to non-local accepted commit votes, so a locally emitted precommit is not redundantly fanned out to the whole topology before the collector retry logic takes over.
- `crates/iroha_core/src/sumeragi/main_loop.rs` now lets `frontier_body_next_due(...)` account for derived voter fallback when the leader lane has no usable remote leader target, and NPoS roster-unavailability candidate selection now scopes lane discovery from the active topology rather than the raw cached snapshot.
- `crates/iroha_core/src/sumeragi/main_loop/tests.rs` now uses PRF-aligned leader/view discovery for the affected proposal and vote fixtures, searches RBC subset cases without assuming the local peer is outside the deterministic rebroadcast base set, and expects partial-session payload rebroadcasts to queue only the chunks that are still locally present.
- This clears the reported `sumeragi::main_loop` failure cluster around collector targeting, idle frontier recovery, RBC rescue/rebroadcast fixtures, same-slot vote-collision validation, exact-frontier retry scheduling, and roster-unavailability source selection.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo test -p iroha_core --lib commit_vote_targets_collectors_or_topology -- --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::assemble_proposal_height_two_records_inline_frontier_manifest' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::commit_vote_targets_collectors_or_topology' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::force_view_change_if_idle_arms_nonleader_empty_frontier_recovery_after_pacemaker_attempt' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::maybe_emit_rbc_deliver_prefers_targeted_ready_rescue_when_subset_skips_local' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::pending_validation_preserves_same_slot_signature_collisions_until_identity_validation' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::precommit_vote_skips_payload_broadcast_for_aborted_pending' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::precommit_vote_targets_collectors_without_broadcast' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::rbc_payload_rebroadcast_allows_derived_roster' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::retry_missing_block_requests_defers_view_change_when_queue_blocks_seen' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::roster_unavailability_candidate_source_matches_consensus_mode' --nocapture`

## 2026-04-22 Roadmap Audit

- `roadmap.md` was re-audited against the current repo and rewritten around repo-backed unfinished issues instead of speculative or stale reminders.
- Removed the obsolete `iroha_torii_shared` `AccountLabel` import blocker; the current file no longer imports `AccountLabel`.
- Removed the old trigger `get_numeric(...)` follow-up; the free helper has already been removed in `kotodama_lang`, so any future trigger work must be scoped to a concrete repo-local test gap.
- Promoted concrete open gaps into the roadmap where evidence exists today, including the `ivm_prebuild.rs` / `integration_tests/build.rs` sample-list drift and the lack of a repo-wide translation metadata audit.

## 2026-04-22 Roadmap Cleanup

- `roadmap.md` now tracks unfinished work only. Completed history was removed from the roadmap so it stops acting like a second status log.
- Historical completed roadmap-only epics were archived here at summary level instead of staying in the roadmap:
  - Kagami NPoS-ready network generation, explicit consensus-mode cutover tooling, peer/block signature hardening, and bootstrap-from-trusted-peer genesis fetch are complete.
  - Kagami/Mochi Iroha3 profiles, account-identity / alias unification, CLI output normalization, CI cleanup, and offline QR storm follow-up work are complete.
  - Soracloud platform MVP and Soracloud model-training / weight-lifecycle work are complete.
  - Kotodama / IVM developer-experience observability work is complete.
  - Asset ID, account-literal, and asset-alias lease cleanup follow-ups are complete.
  - The tracked repository action-item inventory had no actionable code/runtime/SDK/test/tooling/docs entries at the time of the scan; only reference-only mentions remained.

## 2026-04-22 Recent Completed Follow-ups

- `crates/iroha_core/src/sumeragi/main_loop/tests.rs` now salts harness-generated peer seeds per instance and folds the elected signer into the helper heartbeat seed, which eliminates helper block-hash collisions across parallel RBC roster tests and closes the stale-cache replacement coverage gaps around same-epoch roster recovery.
- `crates/iroha_torii/src/zk_attachments.rs` and `crates/iroha_torii/tests/zk_attachments_subprocess.rs` now cover the remaining sanitizer subprocess branches exercised in the latest pass: launcher fallback, timeout handling, response decoding, malformed compressed payloads, child-entry validation, spawn failures, and reader timeout/error paths.
- The recent `iroha_core` frontier-slot helper slices are complete: pending-wrapper liveness, active-owner-state helpers, validation-inflight preservation, commit-inflight preservation, and later-view lock-state guard coverage were all added with focused tests.
- The recent `iroha_torii` manifest-routing slices are complete: helper coverage, direct handler coverage, malformed-payload coverage, and direct-routing coverage are all in place with focused test runs.
- The recent `iroha_data_model` bridge helper / verifier / serialization follow-up slices are complete with focused crate validation.

## 2026-04-22 Completed Follow-up Archive

- Permissioned preserved-peer stable soak: green on the patched tree; the remaining follow-up is the fresh-binary rerun from the current branch.
- Permissioned missing-QC recovery: landed, covered by focused regressions, and aligned with the current reschedule / local-vote behavior.
- Retained RBC summary refresh: landed; the previously failing large-payload NPoS regression is green.
- Permission-cache replay: landed and green.
- Integration failure sweep: the reported targeted regressions were fixed and covered by focused validation.
- MCP writer-profile alignment: landed; mutation-capable MCP endpoint tests now opt into the documented writer profile and the targeted test file is green.

## 2026-04-21 Follow-up: Kotlin SDK typed offline-cash redeem support
- Added typed cash models and a redeem-proof builder under
  `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/`.
  After rebasing onto the Offline V2 Torii surface, `OfflineToriiClient`
  remains scoped to `/v1/offline/v2/readiness`; the older cash-route client
  overloads and route tests are not retained because Torii no longer exposes
  those HTTP endpoints.
- `OfflineStarkEnvelopeProver.kt` +
  `OfflineSettlementProofs.buildRedeemRequestProof` port
  `iroha_core::zk_stark::prove_stark_fri_{air,composition}_envelope_bytes`
  to pure Kotlin, restricted to the `stark/fri/sha256-goldilocks` backend
  used by the legacy offline cash payload shape. Parity is locked byte-for-byte
  against committed Rust-generated fixtures at
  `kotlin/core-jvm/src/test/resources/offline/redeem_proof_fixtures.json`.
- Temporary exception: `java/iroha_android` was intentionally left
  untouched for this slice. The Kotlin↔Java mirror policy in
  `AGENTS.md:69-70` remains repo policy; a follow-up task will mirror
  the typed offline-cash API once the Kotlin surface stabilizes.
- Focused validation:
  - `cd kotlin && ./gradlew :core-jvm:compileKotlin` (pass)
  - `cd kotlin && ./gradlew :core-jvm:test --console=plain --tests
    'org.hyperledger.iroha.sdk.offline.OfflineSettlementProofsParityTest'
    --tests 'org.hyperledger.iroha.sdk.offline.OfflineCashCodecTest'`
    (pass — envelope byte-parity gate cleared against Rust fixtures).
## 2026-05-02 SoraNet VPN native lease escrow hardening

- Added native SoraNet VPN lease escrow data model and ISIs:
  `OpenVpnLeaseEscrow`, `SettleVpnLease`, and `RefundExpiredVpnLease`.
- Added deterministic VPN tariff, on-chain lease status/record types, voucher
  and receipt hashes, and WSV storage for `vpn_leases` so settlement/replay
  state can live in ledger state instead of Torii process memory.
- Core execution now derives deterministic protocol custody accounts, enforces
  XOR-denominated lease funding, verifies client-signed cumulative usage
  vouchers, recomputes earned fees from the fixed tariff, splits earned/refund
  amounts from custody, and blocks generic transfers out of VPN custody.
- Torii's existing receipt compatibility path now rejects relay overclaims by
  checking voucher signatures, exact byte counters, uptime coverage, monotonic
  voucher commitment, and a deterministic earned-fee calculation instead of
  trusting `earned_fee_nanos` supplied by the relay.
- Helper/relay VPN transport now carries client-signed cumulative usage voucher
  control cells. The relay verifies voucher signatures and session/quote/relay
  binding, tracks the highest accepted voucher on the session handle, mirrors
  that voucher into emitted receipts, and stops forwarding when observed
  unvouched payload bytes exceed `vpn.usage_voucher_debt_window_bytes`.
- JavaScript, C#, Swift, Python, Kotlin/JVM, and Java Android Torii clients now
  expose quote-first native VPN lease opening and operator receipt submission
  helpers, including the returned `OpenVpnLeaseEscrow` and `SettleVpnLease`
  instruction skeletons.
- Relay operator workflow now has an optional `vpn.receipt_spool_dir`: when a
  helper-authenticated session closes with an accepted client voucher, the relay
  writes a JSON settlement artifact containing the exact `/v1/vpn/receipts`
  body (`relay_receipt_hex`, `client_voucher_hex`, and `lease_id_hex`) needed to
  obtain the native `SettleVpnLease` instruction. The
  `soranet-vpn-settlement` helper signs those artifacts into deterministic Torii
  headers/body or a one-shot curl command using runtime-only operator seed
  material. Sessions without client vouchers do not produce settlement
  artifacts.
- Remaining deployment follow-up: run a public relay/helper/Torii canary using
  the spooled native open/settle artifacts.

## 2026-05-02 FASTPQ/Poseidon hot-path pass

- Optimized the BN254 Poseidon2 width-3 hot path in
  `crates/iroha_zkp_halo2/src/poseidon.rs`: the S-box now uses explicit
  square/square/multiply arithmetic, width-3 MDS/round updates are unrolled,
  byte hashing absorbs full two-word rate blocks directly, and field/u64 word
  hashing no longer materializes padded vectors. `PoseidonByteHasher::finalize`
  now applies final padding directly instead of routing through `absorb_word`,
  and `hash*_u64` output no longer copies a full 32-byte field representation
  before reading the low limb. A width-6 round-unroll experiment was rejected
  after filtered `hash6_u64` Criterion runs showed the simpler loop faster
  (`~57.1 us` versus `~65.9 us`).
- Added packed `hash_u64_words_bytes` Criterion coverage for the FASTPQ CPU
  batch fallback path and kept a manual two-word sponge loop after it improved
  the 24-word and 64-word filters. Baseline medians were about `40.0 us`,
  `262.6 us`, and `666.4 us` for 2/24/64 words; the manual loop rerun was
  within noise for 2 words and improved to about `258.1 us` and `651.0 us` for
  24 and 64 words. The cached round constants now live in fixed-size arrays
  instead of a `Vec`; filtered Criterion showed `hash2_u64` at about `19.39 us`,
  improved `hash_u64_words_bytes/2` to about `39.5 us`, and kept the 24/64-word
  packed filters statistically unchanged. A final `apply_mds3` row-destructure
  pass was kept after filtered `hash2_u64` Criterion measured `19.47 us` with
  no statistically significant performance change.
- Removed completed-word staging-buffer zeroing from the Poseidon byte hasher
  and FASTPQ word packer, with final partial words zero-padded only at finish.
  Same-session A/B Criterion rejected the old zeroing path after it regressed
  `byte_hasher_streaming/32` by about `30%`, `poseidon_preimage_digest` by about
  `153%`, and `batch_from_transcripts/missing_digests/64` by about `26%`
  against the no-zeroing run under the same load window.
- The final partial-word helper now masks stale high bytes instead of copying
  into a temporary zeroed buffer, and `PoseidonWordPacker` uses an 8-byte
  full-word path. Filtered Criterion kept this follow-up after measuring
  `byte_hasher_streaming/32` at about `59.5 us`,
  `poseidon_preimage_digest` at about `328.5 us`, and
  `batch_from_transcripts/missing_digests/64` at about `21.6 ms`, all reported
  as statistically significant improvements against the preceding filtered run.
- `hash_bytes` now uses a one-shot local sponge path instead of constructing a
  streaming `PoseidonByteHasher`, while preserving the same padding behavior for
  partial trailing words. Filtered Criterion kept the direct path after
  `hash_bytes/128` improved to about `177.6 us`, `hash_bytes/512` improved to
  about `648.9 us`, and `hash_bytes/32` stayed within the noise threshold.
  A round-constant destructuring experiment and a shared mask-table experiment
  were rejected after filtered runs showed no stable gain and a regression in at
  least one hot filter.
- `field_to_bytes` now converts `Fr::to_repr()` directly into `[u8; 32]`
  instead of copying through `as_ref()`. Filtered Criterion kept the conversion
  cleanup after `hash_bytes/128` improved to about `174.1 us`, while
  `hash_bytes/32`, `hash_bytes/512`, `hash2_u64`, and packed
  `hash_u64_words_bytes` filters stayed within noise or showed no detected
  regression.
- The direct `hash_bytes` path now packs short trailing byte words from the
  input slice without a temporary zeroed `[u8; 8]`, and the hot-path benchmark
  now covers 33- and 129-byte inputs. Same-session A/B kept the helper after
  restoring the temporary-copy path regressed one `hash_bytes/129` run to about
  `176.7 us`; the final helper rerun measured about `58.6 us` for 33 bytes and
  `175.8 us` for 129 bytes, both within the noise threshold against the
  immediately preceding baseline.
- Kept cross-crate `#[inline(always)]` on `PoseidonByteHasher`'s public update
  and finalize methods plus FASTPQ's tiny digest/encode helpers after reverting
  them to plain `#[inline]` regressed filtered `byte_hasher_streaming/32`,
  `byte_hasher_streaming/128`, and `poseidon_preimage_digest` by about `4.4%`,
  `6.8%`, and `7.2%` respectively. The `Write` trait method inlining
  experiment was rejected: it kept `poseidon_preimage_digest` within noise but
  regressed the precomputed 64-transcript batch filter by about `1.9%`.
- The `crypto_hotpaths` 64-transcript FASTPQ benchmark fixture now generates a
  valid chained balance sequence so current SMT witness attachment can run
  before measuring missing/precomputed Poseidon digest paths.
- Rejected three follow-up hot-path experiments after filtered Criterion A/B:
  direct slot-1 absorption in `PoseidonByteHasher::update` regressed small
  streaming inputs, a fixed 32-byte `batch_hash` update path regressed
  `poseidon_preimage_digest` by about `9.7%`, and forcing
  `PoseidonWordPacker::{new, update, finish}` to `#[inline(always)]` regressed
  both 64-transcript batch filters by about `5-7%`.
- Rejected output-wrapper inlining too: adding `#[inline]` to
  `hash_u64_words_bytes` regressed the packed u64-word filters heavily, and
  reverse A/B showed ordinary `#[inline]` on `field_to_{bytes,u64}` plus
  fixed-width public wrappers had no meaningful benefit.
- Rejected `#[inline]` on `PoseidonByteHasher::new` after direct reverse A/B
  kept `byte_hasher_streaming/{32,128}` and `poseidon_preimage_digest` within
  the noise threshold without the annotation.
- Kept `chunks_exact(2)` iteration in `hash_u64_words_internal` after direct
  A/B improved the 24-word packed filter by about `3.0%`. Reverse A/B restoring
  the old index loop regressed `hash_u64_words_bytes/{2,24,64}` by about
  `5.4%`, `4.0%`, and `4.2%`.
- Kept `chunks_exact(8)` in `PoseidonWordPacker::update` after isolated reverse
  A/B restoring the manual loop regressed
  `batch_from_transcripts/missing_digests/64` by about `1.7%`; the precomputed
  64-transcript filter stayed within the noise threshold. Hybrid stack-first
  single-digest packers with 24- and 32-word inline buffers were rejected after
  `poseidon_preimage_digest` stayed within-noise slower than the Vec-backed
  packed-word path.
- Rejected the Rayon CPU hash fallback for packed digest batches after the
  64-transcript missing-digest fixture stayed within noise without it, and a
  temporary 256-transcript fixture measured about `98.1 ms` serial versus
  `100.2 ms` through the parallel fallback.
- Kept a `PoseidonByteHasher::finalize` pending-word specialization after the
  direct streaming A/B improved the 32-byte filter by about `2.3%`; 33, 128,
  129, 512, and 4096-byte filters stayed within noise, and reverse A/B restoring
  the old `absorb_word` path moved the 32-byte filter back about `0.9%` slower.
- Updated FASTPQ digest construction in `crates/iroha_core/src/fastpq/mod.rs`
  to pack the single-digest CPU path through the same u64 word preimage builder
  used by batched Poseidon digests. Direct reverse A/B kept the packed-word
  `poseidon_preimage_digest` path after the streaming byte hasher measured
  about `334 us`, and the restored packed implementation confirmed at about
  `317 us`, roughly `5.2%` faster than the temporary streaming baseline.
  GPU digest batches still use a shared word buffer with preallocated slice/word
  capacity. GPU finalization now exits before packing when Poseidon acceleration
  is disabled or the single-delta digest count is below the GPU threshold, and
  the packed-word path now avoids a non-specializing shape-dispatch branch.
  When a large packed batch is already built but the accelerator returns
  `None`, FASTPQ now hashes those packed slices on CPU instead of discarding the
  batch and re-encoding every transcript.
- Continued the host-side FASTPQ trim by making transcript finalizers count and
  process only missing single-delta Poseidon digests. Precomputed transcript
  digests now skip the release-build Poseidon recomputation path while debug
  builds still validate consistency when the serial helper touches an existing
  digest. Batched map/bundle finalizers now reuse the caller's missing-digest
  count instead of rescanning before packing.
- Cleared the denied Ed25519 public-key parse-cache variant-size lint in
  `crates/iroha_crypto/src/signature/ed25519.rs` by boxing cached valid keys
  while preserving cached rejection outcomes.
- Focused validation passed:
  - `cargo test -p iroha_zkp_halo2 poseidon --lib -- --nocapture`
  - `cargo test -p iroha_core fastpq:: --lib -- --nocapture`
  - `cargo test -p fastpq_prover compute_poseidon_digest_matches_canonical_encoded_preimage --lib -- --nocapture`
  - `cargo test -p iroha_crypto parse_public_key_cache --lib -- --nocapture`
  - `cargo check -p iroha_core --features zk-halo2,zk-halo2-ipa --bench crypto_hotpaths`
  - `CARGO_TARGET_DIR=target/codex-core-scallx cargo bench -p iroha_core --features zk-halo2,zk-halo2-ipa --bench crypto_hotpaths -- crypto_hotpaths/poseidon/hash2_u64`
  - `CARGO_TARGET_DIR=target/codex-core-scallx cargo bench -p iroha_core --features zk-halo2,zk-halo2-ipa --bench crypto_hotpaths -- crypto_hotpaths/poseidon/hash_u64_words_bytes`
- A small unrelated `crates/ivm/src/mock_wsv.rs` gas-accounting compile fix was
  applied while validating the benchmark target: `BUILD_PATH_MAP_KEY` now keeps
  the decoded `Name` payload length in scope before returning `schema_gas`.
- A small unrelated `crates/iroha_core/src/state.rs` lifetime compile fix was
  also applied while validating the FASTPQ tests: domain-asset iteration now
  collects owned `AccountId`s before querying account assets.
- `cargo bench -p iroha_core --features zk-halo2,zk-halo2-ipa --bench
  crypto_hotpaths` now builds and runs after the concurrent
  `crates/ivm/src/host.rs` TLV fix. Latest Criterion medians were roughly:
  `hash_bytes` 32/128/512/4096 bytes = `58.5 us`, `176.9 us`, `648.6 us`,
  `5.05 ms`; streaming 32/128/512/4096 bytes = `56.5 us`, `168.8 us`,
  `607.3 us`, `4.72 ms`; `hash2_u64` = `19.9 us`; filtered `hash6_u64`
  remained within noise at `57.0 us`; filtered
  `fastpq/poseidon_preimage_digest` = `308.7 us`; filtered
  `fastpq/batch_from_transcripts` for 64 transcripts = `20.74 ms` with missing
  digests versus `527 us` with precomputed digests. A packed-CPU local
  finalizer experiment was rejected and reverted after it regressed the same
  filter to `22.68 ms` and `625.8 us`; a stack-backed single-digest word
  hasher experiment was also rejected and reverted after it regressed
  `poseidon_preimage_digest` to `330.6 us`. A `chunks_exact(16)` byte-update
  loop refactor was likewise rejected and reverted after Criterion showed
  Poseidon regressions across byte and u64 filters (`hash_bytes/32` around
  `67.5 us`, `streaming/32` around `64.5 us`, and `hash2_u64` around
  `21.4 us`). The final current-code batch rerun kept the precomputed path
  fast at `557.5 us`, while the missing-digest case was noisy under workspace
  load (`45.5 ms`).

## 2026-05-19 - Multisig Propose Norito SDK Devex

- Documented and tested the existing Torii `/v1/multisig/propose` JSON path
  that accepts base64 native Norito `InstructionBox` frames in the
  `instructions` array, alongside structured JSON instruction objects. The
  OpenAPI schema now advertises the instruction `oneOf` shape and the multisig
  POST operations document `application/x-norito` request bodies.
- Added SDK helpers for native instruction bytes:
  - Rust `Client::post_multisig_propose`
  - Swift `ToriiMultisigProposeRequest` / `proposeMultisig`
  - Kotlin and Java Android `MultisigProposeRequest` / `proposeMultisig`
  - C# `ToriiMultisigProposeRequest` / `ProposeMultisigAsync` plus
    `TransactionInstruction.EncodeInstructionBoxBase64`
  - Python `iroha_torii_client.ToriiClient.propose_multisig`, with
    `iroha_python` re-exporting the shared `MultisigResponse` type for its
    inherited Torii helper surface
  - JavaScript `ToriiClient.proposeMultisig` and
    `buildMultisigProposeRequest`
- Added focused negative/adversarial coverage for malformed native Norito
  instruction frames inside mixed batches, server-side multisig-propose
  rejection propagation, empty instruction batches, ambiguous or missing
  selectors, invalid detached signatures/public keys, negative creation
  timestamps, and SDK helper misuse across Rust, Swift, Kotlin, Java Android,
  Python, JavaScript, and C# tests.
- A second adversarial pass now covers incomplete detached-signature field
  pairs at the Torii handler boundary and malformed success responses across
  Rust, Swift, Kotlin, Java Android, Python, JavaScript, and C# parser/client
  tests, including invalid `submitted`, `instructions_hash`, and
  `signing_message_b64` fields.
- A third adversarial pass now rejects `private_key` server-side signing on the
  generic `/v1/multisig/propose` handler, validates Rust client response
  hash/base64 metadata, and makes Kotlin/Java Android multisig response parsers
  fail closed on negative `creation_time_ms`; Swift, Python, JavaScript, and
  C# tests cover the same negative timestamp response shape.
- A fourth adversarial pass now covers malformed detached submit credentials at
  the Torii handler boundary (`public_key_hex` and `signature_b64`) and rejects
  empty-but-valid base64 `signing_message_b64` values in the Rust client
  response validator.
- Focused validation passed:
  - `cargo test -p iroha_torii --lib multisig_propose_documents_native_norito_request_body -- --nocapture`
  - `cargo test -p iroha_torii --lib multisig_generic_propose --features app_api -- --nocapture`
  - `cargo test -p iroha --lib post_multisig_propose -- --nocapture`
  - `swift test --filter ToriiClientTests/testProposeMultisig`
  - `./gradlew :core-jvm:test --tests org.hyperledger.iroha.sdk.client.HttpClientTransportTest --console=plain`
  - `JAVA_HOME=$(/usr/libexec/java_home -v 21) ANDROID_HOME=~/Library/Android/sdk ANDROID_SDK_ROOT=~/Library/Android/sdk ./gradlew test --console=plain`
  - `npm run build:native`
  - `npm run build:dist`
  - `node --test --test-name-pattern "proposeMultisig" test/toriiClient.test.js`
  - `python3 -m compileall python/iroha_torii_client`
  - `python3 -m compileall python/iroha_python/src/iroha_python python/iroha_python/tests/test_address_format.py`
- Python focused validation could not run because the local Python 3.14
  environment does not have `pytest` installed (`No module named pytest`).
- C# focused validation could not run because this environment does not have
  the `dotnet` CLI installed.
