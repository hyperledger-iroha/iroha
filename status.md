# Status

Last updated: 2026-04-26

## 2026-04-26 Offline V2 certificate, redemption, and audit binding

- Hardened Offline V2 note issuance so only `CanManageOfflineEscrow` operators can issue notes, and key certificates must verify against the issuing operator over the canonical certificate payload before escrow is reserved.
- Hardened Offline V2 note redemption so the recursive proof public-input hash must bind the source note commitment, consumed nullifiers, certified key payload, recipient, asset, and amount, and escrow is released only for a ledger-recorded issued-note claim that has not already been redeemed.
- Hardened Offline V2 optional audit so the proof public-input hash binds the token id, observed nullifiers, output commitments, and certified key payload; audit now requires a previously issued key certificate and detects token/public-input conflicts plus duplicate output commitments.
- Ordered cheap issued-claim, token, and nullifier replay checks before expensive recursive proof verification while still verifying proofs before escrow release or new audit state.
- Replaced the local transcript-style recursive proof placeholder with verifier-key-backed validation: the proof must name an active `offline_note_v2` WSV verifier, decode as an `OpenVerifyEnvelope`, match the Offline V2 public-input schema hash, expose the expected public instance columns, and pass the configured ZK backend verifier.
- Added data-model helper payloads for canonical key-certificate signing bytes, issued-note claims, redemption public inputs, and audit public inputs.
- Kept the retired Offline V1 settlement helpers fail-closed while documenting the retained compatibility code path for later removal.
- The strict clippy follow-up also cleaned local warning blockers in the touched dependency graph, including ML-DSA context length conversion, QR rendering docs/casts, FASTPQ proof helper lint annotations, Soracloud/SCCP/config style issues, and data-model test/bench nits.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo check -p iroha_data_model -p iroha_core -p iroha_torii`
  - `cargo clippy -p iroha_data_model -p iroha_core -p iroha_torii -p connect_norito_bridge --all-targets -- -D warnings`
  - `cargo test -p iroha_core offline_note_v2 --lib -- --nocapture`
  - `cargo test -p iroha_data_model offline_note_v2 --lib -- --nocapture`
  - `cargo test -p iroha_torii --test offline_cash_router_smoke -- --nocapture`
  - `cargo test -p connect_norito_bridge offline --lib -- --nocapture`
  - `cargo test -p soranet_pq mldsa --lib -- --nocapture`
  - `cargo test -p iroha_torii_shared qr --lib -- --nocapture`
  - `cargo test -p fastpq_prover merkle --lib -- --nocapture`
  - `npm run lint && node --test test/toriiClient.test.js test/package_dist.test.js test/offlineCounterJournal.test.js test/offlineEnvelope.test.js test/offlineQrStream.test.js test/offlineReplay.test.js` from `javascript/iroha_js`
  - `python3 -m pytest python/iroha_torii_client/tests/test_client.py python/iroha_python/tests/testconnect_codec.py -q`
  - `git diff --check`
  - Targeted stale legacy offline route/helper scan across docs, examples, and SDK sources returned no matches.
  - Targeted removed native export/wrapper scan across the bridge and mobile SDK sources returned no matches.

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
- `docs/source/izanami_communication_vulnerabilities.md` documents how the paper maps to Izanami, the comparison baseline, classification signals, and the current exact-vs-approximate coverage boundaries. Packet loss and leader isolation are explicitly marked as approximations until Izanami gets exact packet-drop windows and dynamic proposer tracking.
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

- `crates/iroha_torii/tests/zk_prover_integration.rs` now uses the supported `halo2/ipa:tiny-add` deterministic Halo2 fixture circuit instead of the unsupported `halo2/ipa:tiny-add-v1` alias, so the shared fixture helper emits inline verifying-key bytes again for the prover report integration coverage.
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
  - Kagami/Mochi Iroha3 profiles, account-identity / alias unification, CLI output normalization, CI TODO cleanup, and offline QR storm follow-up work are complete.
  - Soracloud platform MVP and Soracloud model-training / weight-lifecycle work are complete.
  - Kotodama / IVM developer-experience observability work is complete.
  - Asset ID, account-literal, and asset-alias lease cleanup follow-ups are complete.
  - The tracked repository TODO inventory had no actionable code/runtime/SDK/test/tooling/docs TODOs at the time of the scan; only reference-only TODO mentions remained.

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
