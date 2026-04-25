# Roadmap (Open Work Only)

Last updated: 2026-04-25

Completed history lives in `status.md`. This file should only track unfinished work.

## Validation corridor

- Carry native asset escrow through the remaining Aitai application corridor.
  - Wire the Sora Aitai application UI/backend onto the native numeric escrow ISIs and the new Kotlin/Java/Swift helper surfaces, then subscribe through the numeric escrow query/event APIs.
  - Add Kotodama/IVM pointer-ABI wrappers, SDK builders, and app-facing lifecycle events for the proof-carrying anonymous escrow ISIs now that the native core/query path is in place.
  - Add end-to-end UI/client smoke coverage once the Sora Aitai application replaces the old contract escrow account path for both transparent XOR and shielded anonymous-asset offers.
  - Rerun the full Kotlin, Java Android, and Swift SDK suites after the Aitai app wiring lands.
  - Keep NFT/RWA escrow and court fee/payout generalization as separate follow-ups; the v1 primitive intentionally resolves only between the escrow seller and accepted buyer.
- Carry the Soracloud production posture hardening through the operator-host rollout corridor.
  - Local focused, portable QEMU, and prior multi-peer load gates are green as of 2026-04-25; the readiness runner now reports missing operator inventory and missing observability evidence as production blockers. Before public rollout, run the mixed-host Inrou smoke with the real operator inventory, attach the real metrics/status/alert/dashboard evidence, and archive a blocker-free readiness report.
- Carry the verified lane relay JSON-state/key change through the next UC6 integration corridor.
  - The focused crate checks are green as of 2026-04-24, but no live UC6 settlement-smoke run or topology reset has been performed from this tree.
  - Before any live deployment, confirm the deploy/Core API smoke path still uses `relay_state_key`, JSON relay state, and the simulation gate against the exact finalization payload.
  - If a topology plan selects reset mode while validating this change, stop before approval and reassess the rollout scope.
- Carry the Torii routed-read and telemetry fixes through the next workspace validation corridor.
  - The crate-local sweep is green as of 2026-04-24 with `cargo test -p iroha_torii --lib --features app_api,telemetry -- --nocapture`.
  - When validation budget allows, carry the alias-routing and Torii telemetry slices through the next `cargo test --workspace` / `cargo clippy --workspace --all-targets -- -D warnings` corridor and record the result in `status.md`.
- Broaden validation for the new canonical account-alias lease flow beyond the focused onboarding and executor checks.
  - Rerun a wider `cargo test -p iroha_torii` window with the new `/v1/accounts/{account_id}/aliases`, `/renew`, and `/auto-renew` handlers enabled.
  - Add or rerun focused coverage for the SNS subscription auto-renew billing path in `crates/iroha_core/src/smartcontracts/ivm/host.rs`, not just the onboarding enqueue path.
  - Once the alias lease slice is stable under those focused reruns, fold it into the next broader `cargo test --workspace` / `cargo clippy --workspace --all-targets -- -D warnings` corridor.
- Broaden validation after the 2026-04-22 targeted `sumeragi::main_loop` regression sweep and follow-up unit coverage additions.
  - Rerun a wider `cargo test -p iroha_core --lib` window now that the reported 10-case failure cluster is green under focused verification.
  - Keep the new collector-disabled fallback, seeded-collector precommit fallback, near-quorum full-fanout retransmit helper test, signer-mapping failure full-fanout fallback test, local-vote replay suppression, direct `known_block_commit_qc_recovery_targets(...)` fallbacks, direct `frontier_body_next_due(...)` deadline/fallback coverage, the direct deterministic roster-election guard tests, the direct NPoS empty-commit-topology candidate-source tests, the direct "no new progress" replay-suppression test, the direct replay-cooldown suppression test, the direct local-only explicit-target replay test, the direct deduplicated explicit-target vote replay test, the direct local-only explicit-target commit-QC replay test, the local-proposal authoritative-frontier fallback test, the local-proposal first-signature fallback test, the generic proposal-wire authoritative-frontier fallback and mismatch-guard tests, the plain `frontier_block_created_for_wire(...)` no-metadata fallback test, and the expanded cached-target / unarmed / local-only `frontier_body_next_due(...)` tests in the rerun window so the next pass exercises both the regression fixtures and the new narrow branch tests together.
  - If that broader rerun exposes follow-up regressions outside the patched collector / frontier / roster slice, capture the first failing test names and keep the next fix narrowly scoped.
- Reopen the wider validation corridor after the recent focused `iroha_core`, `iroha_torii`, and `iroha_data_model` test additions.
  - Rerun `cargo test -p iroha_core --lib`, including `quorum_reschedule_rebroadcasts_block_created_while_skipping_block_sync_without_roster_proof` in a fresh cargo process.
  - Rerun `cargo test -p iroha_torii` and `cargo test -p integration_tests -- --nocapture` once the current tree is stable enough for network suites.
  - When validation budget allows, rerun `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings`, then capture failures or green status in `status.md`.

## Consensus and Izanami

- Close the exact-reproduction gaps in the Izanami communication vulnerability matrix.
  - Add a timed fault scheduler that can reproduce the paper's 133s-266s injection window instead of relying only on randomized Izanami fault-loop timing.
  - Add an OS `netem` or in-process P2P packet-drop injector so the `packet-loss` scenario can run 25%, 50%, and 75% loss between selected peer groups.
  - Replace the current self-only trusted-peer restart approximation with a network-level relay/proxy partition so packet loss and isolation do not mutate the peer's validator view while simulating communication loss.
  - Wire Sumeragi proposer/leader telemetry into the `leader-isolation` scenario so Izanami isolates the active proposer rather than a fixed selected peer.
  - Keep any future quick-mode publication reruns split with `--sumeragi-mode both` so permissioned and NPoS Sumeragi classifications are not collapsed.
  - After those exact injectors land, run `scripts/run_izanami_communication_vulnerability_matrix.sh --mode paper --sumeragi-mode both` and record both Iroha classifications against the paper's Algorand/Aptos/Avalanche/Redbelly/Solana baseline in `status.md`.
- Rerun the permissioned preserved-peer stable envelope with fresh binaries from the current tree.
  - Build fresh `izanami` / `iroha3d` binaries instead of reusing prior artifacts.
  - Confirm the previously observed height-533 zero-local-vote `missing_qc` loop does not recur on the current branch.
  - If the run is otherwise green but teardown still panics, preserve dirs/logs and split that into a separate reproducible follow-up.
- Close the remaining Izanami stable-profile acceptance gates.
  - Rerun the 4-peer `1 TPS`, `300s`, `200 blocks` gate with preserved artifacts.
  - Once the short gate is green, rerun the longer `3600s` / `2000+` block acceptance pass.
  - If a run fails, classify the first cause as execution-root divergence, consensus stall, endpoint instability, or workload-plan timeout before tuning again.
- Root-cause the remaining NPoS soak/localnet collapse instead of keeping it as a log-only symptom.
  - Reproduce with preserved peer dirs and `iroha_futures::supervisor=debug`.
  - Identify the first exiting supervised child before investigating downstream connection refusals.
  - Cross-check peer logs with `/v1/sumeragi/status` counters so the fix targets the actual failing layer.

## Throughput and query performance

- Re-establish current throughput knees for the de-amplified harness and shared-host localnet.
  - Rerun the stepped single-host sweep.
  - Repeat permissioned and NPoS passes on the same hardware envelope and compare against the archived `25-50 TPS` / `75-100 TPS` baselines.
  - Record the new knee points and any regressions in `status.md`.
- Turn the proposal-gap / queue-pressure investigation into a reproducible measurement pass.
  - Rerun the 7-peer load that previously advanced slowly or stalled under backlog.
  - Sample `/v1/sumeragi/status`, pending-block / commit-inflight metrics, and queue depths throughout the run.
  - Use a load generator that can actually sustain the target rate before changing worker/backlog tuning again.
- Rebaseline sorted asset-definition query performance.
  - Rerun `snapshot_ephemeral_sorted_asset_defs_first_batch` and `snapshot_stored_sorted_asset_defs_first_batch` on an isolated host.
  - If stored-mode still regresses, tune `stored_sorted_fast_start_params` / first-batch thresholds and keep the matching query tests aligned.
  - Restore a green `cargo test -p iroha_core` baseline for the query-performance branch after any tuning.

## Targeted follow-ups

- Capture Norito CUDA helper validation on a CUDA host.
  - Run `GPUZSTD_CUDA_REQUIRE=1 cargo test -p gpuzstd_cuda --features cuda-kernel -- --nocapture`, `JSONSTAGE1_CUDA_REQUIRE=1 cargo test -p jsonstage1_cuda --features cuda-kernel -- --nocapture`, and the Norito required-loader tests on an SM80+ host with `nvcc` available.
  - Record encode/decode throughput and Stage-1 tape latency against CPU SIMD baselines, then adjust the GPU cutoff only with benchmark evidence.
  - Keep the current `gpu_unavailable` contract intact: helpers without built kernels or a CUDA device must not register as accelerated backends.
- Close the remaining CUDA hardening gaps on real NVIDIA hardware.
  - Run `cargo test -p ivm --features cuda -- --nocapture` and the FASTPQ CUDA-focused tests on an SM80+ host with `nvcc`, confirming the new bounded stream/event waits fail closed instead of hanging.
  - Add focused timeout-path tests or a small CUDA fault harness that can exercise stream/event timeout handling without requiring a wedged GPU.
  - Audit IVM CUDA drop paths after timeout: `cust::DeviceBuffer` drops call `cuMemFree`, so timeout exits should either use stream-ordered async frees or intentionally abandon device allocations instead of risking a second blocking driver call.
  - Move remaining synchronous CUDA host transfers in FASTPQ, Norito JSON/CRC, and GPU zstd to explicit non-blocking streams with pinned host buffers where practical, with the same bounded event polling before host-visible results are read.
  - Add a CUDA CI lane or nightly hardware job that builds real PTX, runs the IVM/FASTPQ/Norito accelerator suites, and records GPU model, driver, CUDA toolkit, and `IVM_CUDA_GENCODE` in `status.md`.
  - Add CPU-vs-CUDA determinism fixtures for IVM vector/hash/AES/BN254/Ed25519 helpers and FASTPQ transforms, including repeated runs on the same input to catch nondeterministic reductions or stale-buffer reuse.
- Reconcile the app-facing alias auto-renew mutation endpoint with the on-chain NFT/domain permission model.
  - The new coverage pass confirmed the read path, but a user-signed disable/update flow still hits `Can't modify NFT from domain owned by another account` when the subscription NFT lives in the operator-owned subscription domain.
  - Decide whether alias auto-renew mutations should be operator-submitted, whether the subscription asset should live in a user-controlled domain, or whether a narrower on-chain permission needs to be granted for this subscription NFT class.
  - Add an integration test for the chosen enable/disable path once the permission model is settled.
- Add a live multi-peer multisig test for previously unregistered signatories.
  - Start from the existing materialization coverage in `integration_tests/tests/multisig.rs`.
  - Add a case where a signatory is materialized by registration and then successfully authors `MultisigPropose` / `MultisigApprove` on the network.
  - Assert transaction-authority shape and final instruction execution, not only account materialization.
- Unify IVM prebuilt sample staging so CLI and integration tests cannot drift.
  - Move the sample manifest to one shared source of truth.
  - Keep `crates/ivm/src/bin/ivm_prebuild.rs` and `integration_tests/build.rs` consuming the same sample list and fixture semantics.
  - Cover a real drift case such as `threshold_escrow`, which is staged by `integration_tests/build.rs` but not listed in `ivm_prebuild.rs` today.
- Make the Sumeragi formal CI source of truth deterministic.
  - Decide whether CI should pin the container digest or use the locally pinned `0.52.2` toolchain path as the canonical version.
  - Update `.github/workflows/pr.yml`, `scripts/formal/sumeragi_apalache.sh`, and `docs/formal/sumeragi/README.md` together.
  - Rerun `bash ci/check_sumeragi_formal.sh` and record the exact Apalache version or digest in `status.md`.
- Replace the vague translation refresh backlog with a real metadata audit.
  - Generalize the existing `ci/check_android_docs_i18n.sh` logic or add a new repo-wide checker for `source_hash` / `translation_last_reviewed`.
  - Run it against repo-root docs, `docs/source`, and `docs/portal` to produce an actual mismatch list.
  - Refresh only the files the checker flags, then document or gate the audit command.
- Add a recorded capture gate for the default `sora-temple` petal styles.
  - Use `petal score-styles` with a published style set, profile, seed, and minimum success ratio.
  - Record the JSON baseline in `status.md` and keep the default style honest under aggressive capture.
  - Only add a stronger default variant if the current `sora-temple` family cannot meet the agreed gate.
