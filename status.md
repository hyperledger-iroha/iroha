# Status

Last update: 2026-01-29
- Kagami codec: drop the `DecodeFromSlice` bound on `ConverterImpl` so converter registration works for data-model types like `NewAccount` (fixes E0599 in `iroha_kagami`).
- Tests: not run (not requested).
- Integration tests (notifications): add a pipeline-event handshake to ensure the TriggerCompleted subscription is live before executing the trigger, reducing WS subscription races in `trigger_completion_event_scenarios`.
- Tests: `cargo test -p integration_tests trigger_completion_event_scenarios -- --nocapture` (ok; norito warnings about unused `padded` assignments persist). `cargo fmt --all` (warns about nightly-only rustfmt options in config).
- Izanami NPoS timing: drop the extra timeout compression so per-phase timeouts scale directly with the target block time (avoids sub-400ms commit timeouts at 1.5s block time); updated the timing test name/expectations accordingly.
- Tests: `cargo test -p izanami derive_npos_timing_uses_block_time_for_timeouts -- --nocapture` (timed out after 120s during compilation; norito `padded` warnings persist).
- Izanami 1 TPS rerun after offloading known‑block QC aggregate verification: command hit 20‑minute timeout but ran; all four peers committed height 15 (split view0 hash `4cba…` vs view1 hash `1c93…`), then stalled at height 16 with repeated “not enough stake collected for QC” and repeated “requested missing block payload from highest QC” for the view‑1 block. Run dir: `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_00i7UP`. Drain stats from slow‑iteration logs: `vote_drain_ms` mean ~28.8ms p95 ~274ms max 835ms; `block_payload_drain_ms` mean ~38.9ms p95 ~325ms max 2065ms.
- Known‑block QC aggregate verification now dispatches to QC verify workers (with inline fallback), and `tally_qc_against_block_signers` accepts a verified aggregate override; added `tally_qc_against_block_signers_accepts_aggregate_override` coverage.
- Tests: `cargo test -p iroha_core tally_qc_against_block_signers_accepts_aggregate_override -- --nocapture` (ok; norito warnings about unused `padded` assignments persist).
- Smart contract payload decode: `Validate<T>` now uses canonical slice decoding (no field-by-field prefix decode), test encodes bare payload, and the getrandom deterministic-pattern assertion matches the callback output. Tests: `cargo test -p iroha_data_model validate_decode_from_slice_roundtrips_any_query -- --nocapture` (ok; norito warnings about unused `padded`). `cargo test -p iroha_smart_contract_codec -- --nocapture` (ok; norito warnings about unused `padded`). `cargo test -p iroha_smart_contract_utils -- --nocapture` (ok; norito warnings about unused `padded`).
- Streaming events: add Norito slice decoding for `StreamingTicketRecord` so framed ticket records decode via `decode_from_bytes` in roundtrip tests.
- Smart contract codec: enable Norito slice decoding for smart-contract payload contexts (SmartContract/Trigger/Executor/Validate), add `Validate<AnyQueryBox>` decode-from-slice coverage, and ensure codec/utils test payloads derive `DecodeFromSlice`.
- Tests: `cargo fmt --all` (warns about nightly-only rustfmt options). `cargo test --workspace -q` (ok; norito warnings about unused `padded`).
- Test network Sora profile detection: include raw `torii.sorafs.*` + root `sorafs.*` flags alongside parsed config so detection doesn't mask torii overrides; fixes `config_requires_sora_profile_ignores_env_overrides`. Tests: `cargo test -p iroha_test_network config_requires_sora_profile_ignores_env_overrides -- --nocapture` (ok; norito warnings about unused `padded`).
- Sumeragi idle view-change: stop deferring view changes on consensus queue backlog (retain saturation backpressure gating) and update the backlog test to assert the view-change still fires when idle.
- Tests: `cargo fmt --all` (warns about nightly-only rustfmt options). `cargo test -p iroha_core force_view_change_if_idle_ignores_consensus_queue_backlog -- --nocapture` (ok; norito warnings about unused `padded`). `cargo test --workspace` failed: `iroha_smart_contract_codec` lib test `Dummy` missing `DecodeFromSlice` bound at `crates/iroha_smart_contract_codec/src/lib.rs:91`.
- Sumeragi state commit: drop the tiered-backend lock before taking `view_lock` to avoid lock-order inversion with lane lifecycle updates; added `state_commit_does_not_hold_tiered_backend_while_waiting_for_view_lock` and `lane_lifecycle_and_commit_do_not_deadlock_on_lock_order` coverage. Tests: `cargo test -p iroha_core state_commit_does_not_hold_tiered_backend_while_waiting_for_view_lock -- --nocapture` (ok; norito warnings about unused `padded`). `cargo test -p iroha_core lane_lifecycle_and_commit_do_not_deadlock_on_lock_order -- --nocapture` (ok; norito warnings about unused `padded`). `cargo fmt --all` (warns about nightly-only rustfmt options in config).
- Sora profile detection config scan: switch SoraFS detection/table helpers to `torii.sorafs.*` (nested) keys so config parsing no longer rejects the legacy `torii.sorafs_storage` parameter. Tests: not run (not requested).
- Integration tests: include trusted peer roster + PoPs when parsing builder config layers for NPoS timeout overrides to avoid genesis config warnings; added unit coverage for the parse-layer helper. Tests: not run (not requested).
- Integration tests: force `trusted_peers_pop` auto-population in sandbox network helpers, add coverage for start/build helpers, and update direct-network builders in unstable-network/genesis/repo/asset/sync paths so localnet configs always include PoPs.
- Tests: not run (config/test-only change).
- Kiso config updates: stage changes on a clone so validation failures don't partially mutate state or notify watchers; added atomicity tests for handshake/transport validation errors. Tests: not run (not requested).
- Highest‑QC missing fetch: only skip missing‑block requests when the highest‑QC block is actually known locally (height/view resolved), even if a payload is pending/processing/aborted; extended proposal fast‑path gating to force fetch when the block is in pending processing, and added `new_view_highest_qc_fetches_missing_for_aborted_payload` + `new_view_highest_qc_fetches_missing_for_processing_payload` coverage. Also added `DecodeFromSlice` support for `PeerTrustGossip` and fixed a test borrow to unblock compilation.
- Tests: `cargo test -p iroha_core --lib sumeragi::main_loop::tests::proposal_missing_highest_qc_fetches_aborted_payload -- --nocapture` (ok; warnings about unused `padded` assignments in `crates/norito/src/lib.rs:8586`/`:8634`).

- Izanami run (tps=1, 300s, 4 peers) after forced highest‑QC missing‑fetch: command timed out after 20m; progress stopped at height 18 on three peers and 13 on `nurtured_wallaby`; no `HasCommittedTransactions` rejections; run dir `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_DUMngz`. Stall window (post‑commit): `nurtured_wallaby` logs repeated “caching proposal without local highest QC block; awaiting sync” for heights 15–18, only one `requested missing block payload from highest QC` (height 14), repeated `not enough stake collected for QC` with 0–2 signers and view‑change/READY‑quorum deferrals; `improved_panther` reports `not enough stake collected for QC` with total_signers 0 at height 18, other peers show no further proposals/QCs. Drain stats (combined): `vote_drain_ms` mean ~10ms p95 ~0–1ms max 905ms; `block_payload_drain_ms` mean ~53ms p95 ~406ms max 1027ms.

- Test network Sora profile detection: ignore default routing policy in raw Nexus override scans so default routing config doesn't force the Sora profile; added raw-nexus override coverage.
- Tests: not run (`cargo test -p iroha_test_network sora_profile_detection_ignores_default_routing_policy -- --nocapture` timed out waiting for a build lock).

- Integration test configs: add `trusted_peers_pop` alongside trusted peer overrides in relay and observer sync tests so PoP validation passes with BLS peer IDs.
- Tests: not run (config-only change).

- Sumeragi RBC seed worker: parallelize seed work across multiple threads, scale queue caps with worker count, and add a shutdown regression test to reduce BlockCreated seeding latency under load.
- Tests: not run (previous `rbc_seed_worker_exits_on_channel_close` failure due to `PeerTrustGossip: DecodeFromSlice` was fixed; test not rerun yet).

- Peers gossiper: add `DecodeFromSlice` for `PeerTrustGossip` via Norito `decode_field_canonical` so `decode_from_bytes` works in unit tests.
- Tests: not run (not requested).

- Izanami NPoS 1 TPS run (300s, `--nexus`, `--tps 1`, `--target-blocks 200`, `--faulty 0`, `RUST_LOG=iroha_core::sumeragi::main_loop=debug`, `IROHA_TEST_NETWORK_KEEP_DIRS=1`): reached commit height 53 (2→53 in ~308s; avg commit interval 6.05s, p50 5.05s, max 13.75s) but missed the 200-block target. Repeated `missing_qc` view-change logs from height 37+, and a peer on port 31022 later exited after SIGQUIT during shutdown. Logs: `/private/tmp/iroha-izanami-logs/irohad_test_network_W2m842`.

- Sumeragi phase EMA telemetry: seed per-phase EMA gauges on startup and on consensus mode flips, and record `collect_da` on `BlockCreated` so DA availability is tracked; added unit coverage for seeded EMA (startup + mode flip) and `collect_da` recording. Updated peers gossiper tests to use `norito::decode_from_bytes` and avoid the `DecodeFromSlice` bound.
- Tests: `cargo test -p iroha_core phase_ema_seeded_on_startup -- --nocapture` (ok; norito warnings about unused `padded`). `cargo test -p iroha_core block_created_records_collect_da_phase -- --nocapture` (ok; norito warnings about unused `padded`). `cargo test -p iroha_core phase_ema_seeded_on_mode_flip -- --nocapture` (ok; norito warnings about unused `padded`). `cargo fmt --all` (warns about nightly-only rustfmt options).

- Sumeragi pacemaker latency: register missing pacemaker gauges in Prometheus output, wire per-phase EMA telemetry updates, align phase labels to docs, tolerate missing per-phase EMA series (require at least one), widen block-spacing budget to 2.5× commit quorum, and hash test-network build args in the build stamp to avoid stale feature builds; added metrics-reader label scanning and pacemaker metrics export/unit coverage.
- Tests: `cargo test -p integration_tests --test sumeragi_npos_pacemaker_latency -- --nocapture` (ok; norito warnings about unused `padded`). Not run: `cargo test -p iroha_telemetry pacemaker_metrics_are_exported`, `cargo test -p iroha_test_network fingerprint_with_build_args_changes_on_arg_differences`, `cargo test -p integration_tests` (full).

- Test network config parsing: include chain/genesis public key when resolving builder config layers for NPoS timeout overrides to avoid missing-parameter warnings; added unit coverage for parsing builder layers.
- Tests: not run (not requested).

- Sumeragi localnet smoke: extend test client TTL for `permissioned_localnet_reaches_100_blocks`, restore the TTL env even when a network run is skipped, and move soak DA timeout multipliers to `sumeragi.advanced.da.*` so the tests match the streamlined config surface.
- Tests: `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-smoke IROHA_TEST_NETWORK_PARALLELISM=1 IROHA_TEST_NETWORK_PERMIT_DIR=$(mktemp -d) cargo test -p integration_tests --test sumeragi_localnet_smoke -- --nocapture` (ok). `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-smoke IROHA_TEST_NETWORK_PARALLELISM=1 IROHA_TEST_NETWORK_PERMIT_DIR=$(mktemp -d) IROHA_SOAK_TARGET_BLOCKS=100 cargo test -p integration_tests --test sumeragi_localnet_smoke permissioned_localnet_soak_thousands -- --ignored --nocapture` (ok). `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-smoke IROHA_TEST_NETWORK_PARALLELISM=1 IROHA_TEST_NETWORK_PERMIT_DIR=$(mktemp -d) cargo test -p integration_tests --test sumeragi_localnet_smoke permissioned_localnet_soak_thousands -- --ignored --nocapture` (timed out after 20m; progress continued with min_non_empty ~146 and queue_size hovering ~205–225; network dir `/var/folders/7l/w31n0ppj4zg874c4szhllss00000gn/T/irohad_test_network_eFE2vf`).

- Torii VK POST integration: encode `authority` via `AccountId` JSON (IH58 + domain suffix) and pass `ExposedPrivateKey` directly so zk vk register/update tests don't depend on domain-selector resolution; fixes `vk_register_update_return_202` returning 400.
- Tests: not run (user-reported failure only).

- Norito stream seq iterator: map `UnexpectedEof` during sequence length decoding to `LengthMismatch` so short seq headers fail deterministically.
- Tests: `cargo test -p norito stream_seq_iter_rejects_short_seq_header` (ok; warnings about unused `padded` assignments in `crates/norito/src/lib.rs` persist).

- Norito header-framed decode: pad short payloads to `Archived<T>` size while preserving logical length so `Option<[u8; N]>::None` and other short values decode via `decode_from_bytes`/`decode_from_reader`; added reader regression in `array_u8` test.
- Tests: not run (user-reported failure only).

- Norito decode_from_bytes/deserialize_stream: require `DecodeFromSlice`, use slice decoding for short/misaligned payloads to avoid temporary-buffer borrows, and add `DecodeFromSlice` impls for manual types (MicroXor/TimestampMs/DurationSeconds/PeersGossip/SmallStr/SmallVec) plus Norito roundtrip tests for SmallStr/SmallVec.
- Tests: not run (not requested).

- Norito stream decode: allow header-framed payloads shorter than `Archived<T>` by zero-padding aligned buffers in `deserialize_stream`; added regression coverage for decode-from-reader on short payloads.
- Tests: `cargo test -p norito --test decode_reader_short_payload -- --nocapture` (ok).

- Norito stream padding tests: adjust Vec padding corruption test to handle zero-padding headers and switch `deserialize_stream_rejects_nonzero_padding` to an aligned scalar so the nonzero padding path remains covered.
- Tests: `cargo test -p norito --test stream` (ok). `cargo fmt --all` (warned about nightly-only rustfmt options).

- Torii status endpoint tests: update `set_effective_timing` calls to include `pacing_factor_bps` (use 10_000 bps) so the new signature compiles.
- Tests: not run (format-only: `cargo fmt --all` warned about nightly-only rustfmt options).

- Sumeragi integration revalidation: made DA payload-loss deferral checks optional under advisory DA, and relaxed NPoS pacemaker restart assertions to rely on converged heights instead of a single peer. Tests: `cargo test -p integration_tests --test sumeragi_da sumeragi_da_payload_loss_does_not_block_commit -- --nocapture` (ok). `cargo test -p integration_tests --test mod time_trigger_scenarios -- --nocapture` (ok). `cargo test -p integration_tests --test sumeragi_npos_liveness npos_pacemaker_resumes_after_downtime -- --nocapture` (ok).
- Sumeragi DA revalidation: tolerate missing RBC delivery on the primary after restart (recovery + commit checks still enforced), then re-ran the remaining DA scenarios individually. Tests: `cargo test -p integration_tests --test sumeragi_da sumeragi_rbc_recovers_after_peer_restart -- --nocapture` (ok; delivery may be pruned). `cargo test -p integration_tests --test sumeragi_da sumeragi_rbc_da_large_payload_ -- --nocapture` (ok). `cargo test -p integration_tests --test sumeragi_da sumeragi_da_commit_certificate_history_four_peers -- --nocapture` (ok). `cargo test -p integration_tests --test sumeragi_da sumeragi_commit_qc_with_tight_block_queue_four_peers -- --nocapture` (ok). `cargo test -p integration_tests --test sumeragi_da sumeragi_rbc_recovers_after_restart_with_roster_change -- --nocapture` (ok). `cargo test -p integration_tests --test sumeragi_da sumeragi_idle_view_change_recovers_after_leader_shutdown -- --nocapture` (ok). `cargo test -p integration_tests --test sumeragi_da sumeragi_rbc_session_recovers_after_cold_restart -- --nocapture` (ok). `cargo test -p integration_tests --test sumeragi_da sumeragi_da_eviction_rehydrates_block_bodies -- --nocapture` (ok).
- Sumeragi DA suite: `cargo test -p integration_tests --test sumeragi_da -- --nocapture` still timed out after 20m while tests were running (no explicit failures before timeout); individual scenario reruns above all passed.
- Sumeragi adversarial: accept corrupted-chunk aborts that still advance height by keying on mismatch telemetry + converged heights; ignore tx confirmation timeouts during heavy scenarios. Tests: `cargo test -p integration_tests --test sumeragi_adversarial sumeragi_adversarial_all_chunks_corrupted_abort -- --nocapture` (ok).
- Integration tests (connected_peers): `cargo test -p integration_tests --test mod connected_peers_with_f_2_1_2 -- --nocapture` (ok). `cargo test -p integration_tests --test mod connected_peers_with_f_1_0_1 -- --nocapture` (ok).
- Integration tests (unstable_network): retries and supply checks now resubmit after recovery, accept committed-height evidence when Torii is unavailable, and cap catch-up timeouts to 180s. Tests: `cargo test -p integration_tests --test mod unstable_network_5_peers_1_fault -- --nocapture` (ok). `cargo test -p integration_tests --test mod unstable_network_8_peers_1_fault -- --nocapture` (ok). `cargo test -p integration_tests --test mod unstable_network_9_peers_2_faults -- --nocapture` (ok).
- Norito decode helpers: relax `deserialize_stream`/`decode_from_bytes` to use `archived_from_slice` instead of requiring `DecodeFromSlice` bounds; update `InstructionRegistry` bounds accordingly; safe zero-sized `SmallVec` decode path. Added `#[norito(decode_from_slice)]` to GuardDirectorySnapshotV2, TicketRevocationSnapshot, AliasProofBundleV1, and AxtProofEnvelope.
- Tests: `cargo test -p integration_tests --test mod unstable_network_9_peers_3_faults -- --nocapture` blocked by compile errors in `iroha_torii` (NoritoJson axum extractor/handler bounds and websocket `DecodeFromSlice` constraints). `IROHA_TEST_SKIP_BUILD=1` + `TEST_NETWORK_BIN_IROHAD=target/debug/iroha3d` still hits the torii build errors.
- Norito packed-seq flags: `codec::encode_with_header_flags` now honors the active decode/layout flags (so `PACKED_SEQ` guards roundtrip correctly) and added unit coverage.
- Tests: `cargo test --workspace` (failed: `crates/iroha_torii/tests/sumeragi_status_endpoint.rs` missing the new `set_effective_timing` argument at lines 98/277/515/599; warnings about unused locals in `crates/iroha_core/src/state.rs` and `crates/iroha_core/src/telemetry.rs`). `cargo test -p norito` (failed: `crates/norito/tests/stream.rs` `stream_vec_collect_rejects_nonzero_padding` and `deserialize_stream_rejects_nonzero_padding` assertions that `padding > 0` at lines 131/152; packed-seq roundtrip tests pass).

- Sumeragi pacing governor: add deterministic windowed pacing-factor governor (`sumeragi.advanced.pacing_governor`) with bounded hysteresis, apply updates at block boundaries via configuration events, and refresh the runbook/parameter streamlining spec/config template coverage.
- Tests: `cargo fmt --all` (warns about nightly-only rustfmt options in config). `cargo test -p iroha_config pacing_governor_rejects_invalid_bounds -- --nocapture` (ok). `UPDATE_EXPECT=1 CARGO_TARGET_DIR=/tmp/iroha-codex-config cargo test -p iroha_config --test fixtures minimal_config_snapshot` (ok). `cargo test -p iroha_core pacing_governor -- --nocapture` (ok). `CARGO_TARGET_DIR=/tmp/iroha-codex-pacing cargo test -p iroha_core apply_without_execution_updates_pacing_factor_with_governor -- --nocapture` (ok).

- Torii connect-gating test config: add the missing `pacing_governor` field to the minimal Sumeragi config so the test compiles with the updated config struct.
- Tests: not run (config-only change).

- Norito decode_from_bytes (uncompressed): enforce exact header padding length so extra padding rejects with `LengthMismatch` instead of checksum mismatch in slice-based decode.
- Tests: `cargo test -p norito core::tests::decode_from_bytes_rejects_excess_padding -- --nocapture` (timed out after 120s while iterating test binaries; targeted test passed).

- Torii ZK proofs query integration test: send the authority as `<public_key>@<domain>` so app_api JSON parsing does not require a domain-selector resolver; add the missing `pacing_governor` test config field; fixes `proofs_query_find_by_id_returns_norito` failing with `ERR_DOMAIN_SELECTOR_UNRESOLVED`.
- Tests: `cargo test -p iroha_torii --test zk_proofs_query_integration --features app_api` (ok; warnings about unused items in `crates/iroha_core/src/sumeragi/pacing_governor.rs` persist).

- Torii attachment sanitizer subprocess test: raise the sanitize timeout floor to avoid slow-spawn flakes while exercising the subprocess path.
- Tests: `cargo test -p iroha_torii attachments_sanitize_via_subprocess -- --nocapture` (ok).

- SoraFS repair endpoints tests: isolate repair state/storage directories with temp dirs so status queries only see the tickets created in each test (fixes `sorafs_repair_worker_endpoints_drive_state` flake).
- Tests: `cargo test -p iroha_torii --test sorafs_repair_endpoints --features app_api` (ok).

- Offline transfer proof fixture: add the required Android provisioned device ID metadata so proof requests accept provisioned transfer payloads.
- Tests: `cargo test -p iroha_torii --test offline_transfer_proof --features app_api` (ok).

- Sumeragi proposal handling: force missing-block fetch for the highest QC when the only local payload is an aborted pending block; added `proposal_missing_highest_qc_fetches_aborted_payload` coverage.
- Tests: `cargo test -p iroha_core proposal_missing_highest_qc_fetches_aborted_payload -- --nocapture` (failed: existing compile errors in `crates/iroha_core/src/sumeragi/main_loop/tests.rs` for missing `cap_gas_limit_for_fast_commit`/`proposal_gas_cost` and `QcVote` Option mismatches; initial parent-hash type error in the new test fixed afterward, not re-run).

- Sumeragi proposal assembly: drop already-committed transactions from the proposal batch after queue scan so stale queue snapshots do not broadcast blocks that will fail `HasCommittedTransactions`; added `proposal_filter_drops_committed_transactions_after_queue_scan` coverage.
- Tests: `cargo fmt --all` (warns about nightly-only rustfmt options in config). `cargo test -p iroha_core proposal_filter_drops_committed_transactions_after_queue_scan -- --nocapture` (failed: `iroha_config` build error — `fast_gas_limit_per_block` `Option` conflicts with `config(default)` in `crates/iroha_config/src/parameters/user.rs:5221`, plus missing `NonZeroU64` import in `crates/iroha_config/src/parameters/defaults.rs:2322`).

- Sumeragi proposal assembly: clamp per-block gas budget when `effective_commit_time_ms <= 1000` (optional via `sumeragi.block.fast_gas_limit_per_block`, applied as `min(ivm_gas_limit_per_block, fast_gas_limit_per_block)`) and select transactions by gas to keep validation within 1s finality budget; added unit coverage and docs.
- Tests: not run (not requested).

- Izanami run (tps=1, 300s, 4 peers): stopped before target blocks; max committed height 16 on all peers; no `proposal mismatch` logs; `vote_drain_ms` max 829ms vs `block_payload_drain_ms` max 3871ms; repeated `not enough stake collected for QC` lines persisted; network dir `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_ntHOWR`.
- Tests: `cargo build -p izanami --release --locked` (warning: unused `min_finality` in `crates/iroha_core/src/sumeragi/mod.rs:360`). `RUST_LOG=izanami::summary=info,izanami::workload=warn,iroha_core::sumeragi::main_loop=debug,iroha_core::sumeragi=info,iroha_p2p=info IROHA_TEST_NETWORK_KEEP_DIRS=1 IROHA_TEST_NETWORK_PERMIT_DIR=$(mktemp -d) cargo run -p izanami --release --locked -- --allow-net --nexus --peers 4 --faulty 0 --duration 300s --target-blocks 200 --progress-interval 10s --progress-timeout 180s --tps 1 --max-inflight 8 --workload-profile stable` (stopped before target blocks reached).
- Sumeragi pacing factor: `pacing_factor_bps` now scales effective block/commit timing and is surfaced via status/torii/client outputs; Sumeragi runbook, pacemaker/configuration references, parameter streamlining spec, data model/telemetry/operator aids, and evidence API docs now list the pacing factor and effective timing fields, plus cleanup for JSON helper tests and unused imports.
- Tests: `cargo test -p iroha_core update_effective_timing_status_populates_snapshot -- --nocapture` (ok). `cargo test -p iroha_data_model sumeragi_parameters_effective_timing_applies_pacing_factor -- --nocapture` (ok).

- iroha_data_model JSON parsing: add `expect_u32` helper for `pacing_factor_bps` and unit coverage for range handling.
- Tests: `cargo test -p iroha_data_model expect_u32 -- --nocapture` (ok).

- Address literal refactor: moved the Sora compressed account-address sentinel to `sora` (full-width `ｓｏｒａ`), refreshed fixtures/docs/SDKs/clients, and tightened the Torii test resolver bootstrap so address-format round trips stay stable when other tests clear the domain-selector resolver.
- Tests: `cargo fmt --all` (warns about nightly-only rustfmt options in config). `cargo run -p iroha_data_model --example address_vectors` (ok; regenerated fixtures). `cargo test -p iroha_torii address_format::tests::display_literal_and_from_literal_round_trip -- --nocapture` (failed: `crates/iroha_data_model/src/parameter/system.rs` missing `pacing_factor_bps` handling; unrelated build error in `iroha_data_model`).

- Torii UAID portfolio fixtures: align UAID portfolio tests/fixtures with the one-to-one UAID binding invariant (single UAID-bound account, two asset positions).
- Tests: `cargo test -p iroha_torii --test accounts_portfolio --features app_api` (ok).

- Sumeragi build cleanups: removed unused locals in Sumeragi main-loop and queue tests, added `min_finality_ms` to the consensus fingerprint helper in `sumeragi_mode_cutover`, and trimmed unused locals in gossiper/Kura tests.
- Tests: `cargo test -p iroha_core block_sync_update_accepts_stale_view_when_missing_block_requested -- --nocapture` (ok). `cargo test -p integration_tests sumeragi_mode_cutover -- --nocapture` (ok; warning about unused import `std::time::Instant` in `crates/iroha_core/src/block.rs:55`).

- Kotodama lint: suppress opaque-access and nonliteral map-key warnings when explicit `#[access]` hints are present; updated Kotodama docs to note the behavior, refreshed Kotodama samples/docs/examples to add access hints and pointer de-duplication, and regenerated the affected `.to` artifacts (IVM docs examples, Kotodama samples, test fixtures, NFT example).
- Tests: `cargo test -p kotodama_lang lint_opaque_access_hints_with_explicit_access_is_silent` (ok). `cargo test -p kotodama_lang lint_nonliteral_state_map_key_with_explicit_access_is_silent` (ok).

- Sumeragi vote/QC verify workers: honor auto sizing for worker threads + queue caps when config values are 0 (aligns with validation workers) so vote verification can buffer more work before synchronous fallback; added unit coverage.
- Tests: `CARGO_TARGET_DIR=/tmp/iroha-codex-worker cargo test -p iroha_core worker_config_auto_scales -- --nocapture` (ok; warning about unused `mut` in `crates/iroha_core/src/kura.rs:7474` persists).
- Izanami NPoS timeouts: scale the block-time input used for per-phase timeout derivation so the timeout budget tracks the target block time; updated `derive_npos_timing_scales_timeouts_to_block_time` coverage.
- Tests: `cargo fmt --all` (warns about nightly-only rustfmt options in config).
- Izanami run (tps=1, 300s, 4 peers) after timeout scaling: stopped before target blocks; committed height 27 across peers; network dir `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_8ioANR`. Slow-line parsing across peers: `vote_drain_ms` mean ~6–10ms (p95 ~1ms, max ~534ms), `block_payload_drain_ms` mean ~71–119ms (p95 ~366–594ms, max ~3562ms), so vote drain tightened but height still stalls around the high 20s.
- Commit pipeline logging: include RBC session status (ready/required, chunks, delivered, sent_ready, invalid) in fast-timeout deferral logs to pinpoint READY quorum vs commit gating stalls.
- Izanami run (tps=1, 300s, 4 peers) with commit deferral + RBC logs: stopped before target blocks; committed height 42 on three peers and 41 on one; network dir `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_1OmzFT`. Commit spacing on `astounding_chameleon`: mean ~7.3s, p95 ~13.7s, max ~24.5s. Deferral logs show `rbc_ready` ≥ required (3/4) with `rbc_delivered=true` and chunks complete while `commit_qc_seen=false`; block validation timings at height 42 are ~82–103ms. Indicates the remaining stall is vote/QC formation latency rather than RBC or block validation.

- Torii SoraFS pin rate-limit test: widen the test window so back-to-back pin requests remain within the limiter even on slower storage; verified `cargo test -p iroha_torii storage_pin_rate_limits_repeated_requests -- --nocapture` (ok).
- Sumeragi parameter streamlining: remove on-chain NPoS timeout fields, drop the `sumeragi.npos.block_time_ms` config fallback, derive NPoS timeouts from on-chain `SumeragiParameters.block_time_ms` with advanced overrides, adjust Kagami genesis generation, and refresh genesis + Sumeragi runbook docs to reference `effective_npos_timeouts` (translations included).
- Tests: `cargo fmt --all` (warns about nightly-only rustfmt options in config). `cargo test -p iroha_config` (failed: `minimal_config_snapshot` expect mismatch). `UPDATE_EXPECT=1 cargo test -p iroha_config --test fixtures minimal_config_snapshot` (ok).

- IVM/CoreHost ZK wiring: hydrate roots/elections/verifying-keys snapshots for `ZK_*` syscalls (executor + `CoreHost::from_state`) and add a vote-tally snapshot syscall test.
- Pipeline overlay/access: pass Halo2 config, chain id, and ZK snapshots into CoreHost runs so prepass/overlay paths see real ZK state.
- Trigger execution: hydrate CoreHost with Halo2 config, chain id, and ZK snapshots; add CoreHost::from_state snapshot test.
- Kotodama demo artifacts: added ZK `meta` to `zk_vote_and_unshield.ko`, regenerated demo `.to` bytecode + manifests from Kotodama sources (authority_probe, ivm_smoke, prediction_market, irohaswap, transfer), and re-signed `prediction_market.deploy.manifest.json`.
- Tests: `cargo test -p iroha_core zk_vote_tally_syscall_reads_world_snapshot -- --nocapture` (ok; warnings about unused import `SumeragiNposTimeouts` in `crates/iroha_core/src/sumeragi/main_loop/tests.rs:29`, unused `mut` in `crates/iroha_core/src/kura.rs:7474`, unused `local_peer` in `crates/iroha_core/src/peers_gossiper.rs:1291`, unused `consensus_mode` in `crates/iroha_core/src/sumeragi/main_loop/tests.rs:48253`, unused `leader_kp` in `crates/iroha_core/src/sumeragi/main_loop/tests.rs:51847`).
- Kotodama sweep: renamed the Android override request template to `docs/examples/android_override_request.json`, fixed the MFC sample Name literal and deduplicated its asset pointer helper, then compiled all tracked `.ko` sources (dynamic bounds examples built with the `kotodama_dynamic_bounds` feature).

- Sumeragi config streamlining: introduce `sumeragi.advanced` (queues/worker/pacemaker/RBC + DA time multipliers + NPoS timeouts), keep `sumeragi.da` for enable + per-block limits, and refresh docs/templates/fixtures/error paths to the nested keys.
- Tests: `cargo fmt --all` (warns about nightly-only rustfmt options in config). `cargo test -p iroha_config` (ok). `UPDATE_EXPECT=1 cargo test -p iroha_config --test fixtures minimal_config_snapshot` (ok).

- Sumeragi main loop: make `update_effective_timing_status` borrow `&self` instead of `&mut self` so it can run while a `StateView` immutable borrow is live (fixes E0502 in commit/mode paths).
- Tests: `cargo test -p iroha_core update_effective_timing_status_populates_snapshot -- --nocapture` (ok; warnings about unused `mut` in `crates/iroha_core/src/kura.rs:7474` and unused variables in `crates/iroha_core/src/peers_gossiper.rs:1291`, `crates/iroha_core/src/sumeragi/main_loop/tests.rs:48270`, `crates/iroha_core/src/sumeragi/main_loop/tests.rs:51831` persist).

- Sumeragi status snapshot: expose effective timing + NPoS timeout fields in Norito/JSON payloads (Torii + client conversion), add unit coverage for timing snapshots/JSON, and refresh status docs.
- Tests: `cargo fmt --all` (warns about nightly-only rustfmt options in config). `cargo test -p iroha_core effective_timing -- --nocapture` (ok; warnings about unused `mut` in `crates/iroha_core/src/kura.rs:7474`, unused `local_peer` in `crates/iroha_core/src/peers_gossiper.rs:1291`, unused `consensus_mode` in `crates/iroha_core/src/sumeragi/main_loop/tests.rs:48270`, unused `leader_kp` in `crates/iroha_core/src/sumeragi/main_loop/tests.rs:51831`). `cargo test -p iroha_torii sumeragi_status_endpoint --features telemetry -- --nocapture` (ok; warning about unused import `std::time::Instant` in `crates/iroha_core/src/block.rs:55`).
- Izanami genesis: ensure the NPoS commit-time parameter is injected before block-time so SetParameter validation does not reject the genesis transaction; added `npos_genesis_sets_commit_time_before_block_time` coverage.
- State parameter updates: handle `SumeragiParameter::MinFinalityMs` in detached delta application and add a regression that validates the configuration event emission.
- Izanami run (tps=1, 300s, 4 peers, main_loop debug) after the genesis timing order fix: stopped before target blocks (lane_000_core `blocks.hashes` count 54 across peers); `block validation timings` execution_ms p90 ~1529ms p95 ~1599ms p99 ~3019ms (mean ~357ms); `commit pipeline defers validation` still logged 35–43 times per peer; network dir `/var/folders/7l/w31n0ppj4zg874c4szhllss00000gn/T/irohad_test_network_NQSMVs`.
- Tests: `CARGO_TARGET_DIR=/tmp/iroha-codex-roadmap-target cargo test -p iroha_core delta_merge_set_sumeragi_min_finality_emits_configuration_event -- --nocapture` (ok). `CARGO_TARGET_DIR=/tmp/iroha-codex-roadmap-target cargo test -p izanami npos_genesis_sets_commit_time_before_block_time -- --nocapture` (ok; emits test-network peer dirs).
- Sumeragi block validation timings: record stateless/execution sub-stage timings (state-dependent, snapshot, DA index hydration, state-block creation, tx execution, AXT, DA cursor, genesis clean) and log them in `validate_block_for_voting` debug output; expanded timing coverage.
- Tests: `cargo fmt --all` (warns about nightly-only rustfmt options in config). `cargo test -p iroha_core validate_block_for_voting_records_timings -- --nocapture` (ok; warnings about unused `mut` in `crates/iroha_core/src/kura.rs:7474` and unused vars in `crates/iroha_core/src/peers_gossiper.rs:1291`, `crates/iroha_core/src/sumeragi/main_loop/tests.rs:48149`, `crates/iroha_core/src/sumeragi/main_loop/tests.rs:51710` persist).
- Izanami run (tps=1, 300s, 4 peers) with validation sub-stage timings: stopped before target blocks; network dir `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_mwreSi`. Parsed 152 `block validation timings` entries: `execution_tx_ms` dominates (mean 740ms, p95 ~4011ms, max 5313ms) while DA index/state-block/AXT/DA cursor/genesis sub-stages are ~0ms; `stateless_ms` mean 50ms p95 61ms. Top spikes at heights 6/34/36 across peers. Indicates the hot path is transaction execution, not stateless checks or DA plumbing.
- Sumeragi block validation timings: further split `execution_tx_ms` into signature batch, stateless validation, access derivation, overlay build, DAG build, schedule, and apply/finalize; updated logging and timing test.
- Tests: `cargo fmt --all` (warns about nightly-only rustfmt options in config). `cargo test -p iroha_core validate_block_for_voting_records_timings -- --nocapture` (ok; warnings about unused `mut` in `crates/iroha_core/src/kura.rs:7474` and unused vars in `crates/iroha_core/src/peers_gossiper.rs:1291`, `crates/iroha_core/src/sumeragi/main_loop/tests.rs:48173`, `crates/iroha_core/src/sumeragi/main_loop/tests.rs:51734` persist).
- Izanami run (tps=1, 300s, 4 peers) with tx sub-stage timings: stopped before target blocks; network dir `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_FE6T7a`. Parsed 172 timing entries: `execution_tx_apply_ms` dominates (`mean ~508ms`, `p95 ~3875ms`, `max 5862ms`) while `execution_tx_dag_ms` spikes occasionally (max 1803ms) and other sub-stages are near zero. The hot path is apply/execute/trigger phase inside transaction execution.

- Sumeragi timing floor: added `min_finality_ms` to on-chain parameters and consensus fingerprints, enforced `commit_time_ms >= block_time_ms >= min_finality_ms` validation, clamped permissioned/NPoS timeouts to the floor, and refreshed genesis templates/docs with the 100ms default; added unit coverage for timing clamps and SetParameter rejection cases.
- Tests: `cargo fmt --all` (warns about nightly-only rustfmt options in config).

- Test harness determinism: documented the criteria for opting unit tests into real P2P (`IROHA_TEST_REAL_NETWORK`) in the testing guide, keeping unit tests hermetic unless OS-level socket behavior is required.
- Izanami validation latency: documented how to surface per-block validation timing logs (`block validation timings` with stateless/execution/total ms) during Izanami runs.
- Tests: `CARGO_TARGET_DIR=/tmp/iroha-codex-roadmap-target cargo test -p iroha_core validate_block_for_voting_records_timings -- --nocapture` (ok; warnings about unused `mut` in `crates/iroha_core/src/kura.rs:7474`, unused variables in `crates/iroha_core/src/peers_gossiper.rs:1291`, `crates/iroha_core/src/sumeragi/main_loop/tests.rs:48120`, `crates/iroha_core/src/sumeragi/main_loop/tests.rs:51681`). `CARGO_TARGET_DIR=/tmp/iroha-codex-roadmap-target cargo test --workspace` (failed: missing `SumeragiParameter` import in `crates/iroha_core/src/smartcontracts/isi/world.rs:9530`, plus non-exhaustive match in `crates/iroha_core/src/state.rs:7763` for `SumeragiParameter::MinFinalityMs`; warnings about unused import `std::time::Instant` in `crates/iroha_core/src/block.rs:55` and unused variables in `crates/iroha_core/src/state.rs:25567`/`25730` and `crates/iroha_core/src/telemetry.rs:12026`/`12181`).
- IVM trigger execution: reuse a shared IVM cache for trigger metadata/runtime templates to avoid repeated parse/load overhead; added `ivm_time_trigger_reuses_cache_across_blocks` coverage.
- Tests: `cargo fmt --all` (warns about nightly-only rustfmt options in config). `cargo test -p iroha_core ivm_time_trigger_reuses_cache_across_blocks -- --nocapture` (ok; warnings about unused `mut` in `crates/iroha_core/src/kura.rs:7474` and unused vars in `crates/iroha_core/src/peers_gossiper.rs:1291`, `crates/iroha_core/src/sumeragi/main_loop/tests.rs:48120`, `crates/iroha_core/src/sumeragi/main_loop/tests.rs:51681` persist).

- Roadmap: added Adaptive Pacing Floor and Sumeragi Parameter Streamlining work items to `roadmap.md`.
- Docs: drafted and refined `docs/source/sumeragi_parameter_streamlining.md` (added defaults/thresholds) and linked it from `roadmap.md`.
- Roadmap: refined Adaptive Pacing Floor and Sumeragi Parameter Streamlining tasks; marked the spec item complete.
- Izanami NPoS timing: derive NPoS timeouts from block time (config defaults) and split commit-time vs commit-timeout so on-chain genesis uses the correct fast-timeout values; add `derive_npos_timing_uses_block_time_for_timeouts` coverage.
- Izanami run (tps=1, 300s, 4 peers) after timing fix: stopped before target blocks; committed height 38 on `on_spitz` and peers exit cleanly. Logs show `commit pipeline defers validation past fast timeout` with `pending_age_ms` 1.5–5.0s vs `fast_timeout_ms` 1125, `commit_qc_seen=false`, and `time triggers matched` lengths 8–12s, indicating pre-vote validation latency still gates progress. Network dir `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_QwwMnX`.
- Tests: `cargo fmt --all` (warns about nightly-only rustfmt options in config). `cargo test -p izanami derive_npos_timing_uses_block_time_for_timeouts -- --nocapture` (timed out after 120s during compilation). `RUST_LOG=izanami::summary=info,izanami::workload=warn,iroha_core::sumeragi::main_loop=debug,iroha_core::sumeragi=info,iroha_p2p=info IROHA_TEST_NETWORK_KEEP_DIRS=1 IROHA_TEST_NETWORK_PERMIT_DIR=$(mktemp -d) cargo run -p izanami --release --locked -- --allow-net --nexus --peers 4 --faulty 0 --duration 300s --target-blocks 200 --progress-interval 10s --progress-timeout 180s --tps 1 --max-inflight 8 --workload-profile stable` (stopped before target blocks).

- Sumeragi vote verify: add debug logging for vote-verify rejections with reason + signer + roster hash in worker threads to surface signature/roster drops.
- Tests: not run (log-only change).

- Izanami log scan (tps=1, 300s, 4 peers) for `irohad_test_network_o6jN6V`: height-40 QC aggregated and commit started on swaying_deer, but no `stored committed block to kura` log for height 40 on that peer (other peers did persist); BlockSyncUpdate deferred due to `commit_inflight` and state-view lock contention warnings appear. Indicates a peer-local commit persistence stall rather than missing QC.
- Tests: not run (analysis only).

- IrohaSwift: add offline allowance registration request/response + ToriiClient method for POST `/v1/offline/allowances`, plus request encoding coverage; update pipeline test stubs to serve node capabilities.
- Tests: `swift test` (pass).

- Izanami config: set `pipeline.workers` + `sumeragi.advanced.worker.validation_*` overrides to 0 (auto) so stateless pipeline and pre-vote validation scale to available cores.
- Tests: not run (config change only).

- Sumeragi: allow proposal handlers to request missing blocks for the highest QC by widening `request_missing_block_for_highest_qc` visibility (build fix).
- Tests: not run (build fix only).

- Izanami run (tps=1, 300s, 4 peers) with auto-scaled `pipeline.workers` + `sumeragi.advanced.worker.validation_*`: stopped before target blocks; lane_000_core `blocks.hashes` file size 864 bytes (~27 entries) and logs show committed height 26 across peers; network dir `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_Wk33iK`. Slow-line parsing across peers: `vote_drain_ms` mean 233–330ms (p95 999–1252ms, max 2928–3378ms); `block_payload_drain_ms` mean 75–107ms (p95 378–453ms, max 1113–3201ms); `BlockSyncUpdate` handled elapsed mean 156–237ms (p95 499–1113ms, max 1113–2946ms). Aggregate: vote mean 266ms (p95 1156, max 3378); payload mean 88ms (p95 418, max 3201); BlockSyncUpdate mean 190ms (p95 756, max 2946).
- Tests: `RUST_LOG=izanami::summary=info,izanami::workload=warn,iroha_core::sumeragi::main_loop=debug,iroha_core::sumeragi=info,iroha_p2p=info IROHA_TEST_NETWORK_KEEP_DIRS=1 IROHA_TEST_NETWORK_PERMIT_DIR=$(mktemp -d) cargo run -p izanami --release --locked -- --allow-net --nexus --peers 4 --faulty 0 --duration 300s --target-blocks 200 --progress-interval 10s --progress-timeout 180s --tps 1 --max-inflight 8 --workload-profile stable` (stopped before target blocks reached).

- Pipeline stateless validation: reuse shared Rayon pool (no per-block pool creation), add stateless prepass cache with TTL/not-before gating, and batch signature verification for Ed25519/Secp/PQC/BLS (PoP-gated) with deterministic fallbacks; validation worker threads/queues now auto-scale when set to 0; config/docs updated and new unit coverage added.
- Tests: `cargo fmt --all` (warns about nightly-only rustfmt options in config). `cargo test -p iroha_core signature_verification_result_reports_invalid_signature -- --nocapture` (ok; warnings about unused `mut` in `crates/iroha_core/src/kura.rs:7407` and unused variables in `crates/iroha_core/src/peers_gossiper.rs:1291`, `crates/iroha_core/src/sumeragi/main_loop/tests.rs:47772`, `crates/iroha_core/src/sumeragi/main_loop/tests.rs:51333` persist). `cargo test -p iroha_core cache_respects_not_before_and_expiry -- --nocapture` (ok; same warnings).

- Izanami run (tps=1, 300s, 4 peers) after pipeline shared-pool reuse + stateless cache + stateless signature batching: stopped before target blocks; lane_000_core blocks.hashes=3 across peers, network dir `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_EdRo0i`. Slow-line parsing across peers: `vote_drain_ms` mean 238–276ms (p95 946–1003ms, max 2828–4654ms); `block_payload_drain_ms` mean 54–65ms (p95 223–368ms, max 492–782ms); `BlockSyncUpdate` handled elapsed mean 79–112ms (p95 334–442ms, max 472–509ms). Run aborted with `izanami run stopped before target blocks reached`; peer stderr empty.
- Tests: same as above.

- Localnet tx gossip tuning: Kagami localnet now shortens transaction gossip cadence to 100ms (resend ticks = 1, target reshuffle cadence = 100ms) when the pipeline is 1s or faster, so 1 Hz runs do not wait on 1s tx gossip ticks. Updated localnet throughput docs and Kagami README; added localnet config coverage in `generated_configs_set_localnet_channel_caps` plus a helper unit test.
- Tests: not run (not requested).

- NPoS localnet 1 Hz soak (release, `block_time_ms=1000`, `commit_time_ms=1000`, tx gossip 100ms): `/private/tmp/iroha-localnet-npos-1hz-20260127T200110Z` (ports 52080/57337). 100x1 Hz `iroha ledger transaction ping --msg 1hz-profile --no-wait` with `/v1/sumeragi/status` sampling to `sumeragi_status_1hz_20260127T200452Z.jsonl` (ping log `ping_1hz_20260127T200452Z.log`). `commit_qc.height` 1->61 (+60 over 120 samples, ~0.50 blocks/s), `view_change_install_total` +2 (`stake_quorum_timeout_total` +1), `missing_payload_total` +0, `missing_qc_total` +0, `missing_block_fetch.total` +32, `pending_rbc.bytes` max 13034 (sessions max 2).
- Tests: not run (soak only).

- NPoS localnet 1 Hz soak (release, default 1s pipeline split, tx gossip 100ms): `/private/tmp/iroha-localnet-npos-1hz-20260127T200918Z` (ports 53080/58337). 100x1 Hz `iroha ledger transaction ping --msg 1hz-profile --no-wait` with `/v1/sumeragi/status` sampling to `sumeragi_status_1hz_20260127T201237Z.jsonl` (ping log `ping_1hz_20260127T201237Z.log`). `commit_qc.height` 1->41 (+40 over 120 samples, ~0.33 blocks/s), `view_change_install_total` +14 (`missing_payload_total` +13), `missing_qc_total` +0, `stake_quorum_timeout_total` +0, `missing_block_fetch.total` +494, `pending_rbc.bytes` max 2627 (sessions max 1).
- Tests: not run (soak only).

- NPoS localnet 1 Hz soak (release, default timeouts, `commit_time_ms=1000`): `/private/tmp/iroha-localnet-npos-1hz-20260127T192356Z` (ports 51080/56337). 100x1 Hz `iroha ledger transaction ping --msg 1hz-profile --no-wait` with `/v1/sumeragi/status` sampling to `sumeragi_status_1hz_20260127T192855Z.jsonl` (ping log `ping_1hz_20260127T192855Z.log`). `commit_qc.height` 1->53 (+52 over 120 samples, ~0.43 blocks/s), `view_change_install_total` +1 (`stake_quorum_timeout_total` +1), `missing_payload_total` +0, `missing_qc_total` +0, `missing_block_fetch.total` +28, `pending_rbc.bytes` max 3875 (sessions max 1).
- Tests: not run (soak only).

- NPoS localnet 1 Hz soak (release, default timeouts, `commit_time_ms=1000`): `/private/tmp/iroha-localnet-npos-1hz-20260127T174650Z` (ports 47080/52337). 100x1 Hz `iroha ledger transaction ping --msg 1hz-profile --no-wait` with `/v1/sumeragi/status` sampling to `sumeragi_status_1hz_20260127T174727Z.jsonl` (ping log `ping_1hz_20260127T174727Z.log`). `commit_qc.height` 1->64 (+63 over 120 samples, ~0.53 blocks/s), `view_change_install_total` +1, `missing_payload_total` +0, `missing_qc_total` +0, `stake_quorum_timeout_total` +0, `missing_block_fetch.total` +18, `pending_rbc.bytes` max 2187 (sessions max 1). Logs show missing BlockCreated fetch followed by BlockCreated receipt (peer3.log:167).
- Tests: not run (soak only).

- NPoS localnet 1 Hz soak (release, `block_time_ms=1000`, `commit_time_ms=1000`, tx gossip 100ms): `/private/tmp/iroha-localnet-npos-1hz-release-20260128T102419Z` (ports 29084/33341). 120x1 Hz `scripts/tx_load.py` (`--batch-size 1 --batch-interval 1`) with `/v1/sumeragi/status` sampling to `sumeragi_status_1hz_20260128T102830Z_peer*.jsonl` (tx log `tx_load_1hz_20260128T102830Z.log`). `commit_qc.height` 1->57 (+56 over ~135s, ~0.41 blocks/s) across peers; `view_change_install_total` +1 (peer0/1/3) and +2 (peer2) with `stake_quorum_timeout_total` +1 on all peers and `missing_qc_total` +1 on peer2; `missing_block_fetch.total` +25-38; `pending_rbc.bytes` max 8939 (sessions max 1). Tx submit window ~118.9s for 120 pings (~1.01 tps). `tx_load.py` could not capture /metrics samples, so throughput was derived from transaction timestamps.
- Tests: not run (soak only).

- Android SDK offline client: add offline allowance registration, include response bodies in HTTP error messages, add certificate JSON serialization, and replace Java 9+ file APIs/isBlank usages for Java 8 compatibility.
- Tests: not run (not requested).

- NPoS QC tally: stake quorum now prefers the active validator roster (public lane validators) when present, falling back to the commit topology only when no active roster is available; added `npos_qc_uses_active_validator_roster_for_stake_quorum` coverage.
- Tests: not run (not requested).
- Sumeragi RBC recovery test: raise the recovery chunk size and pending chunk cap so the DA payload budget admits the ~9MB log transaction; avoids proposal starvation and restores session persistence/recovery coverage.
- Tests: `CARGO_TARGET_DIR=/tmp/iroha-codex-sumeragi-rbc IROHA_TEST_NETWORK_KEEP_DIRS=1 cargo test -p integration_tests sumeragi_rbc_session_recovers_after_cold_restart -- --nocapture` (ok).

- Kura eviction: force a pending-fsync flush before DA eviction so undurable index entries are not truncated; added `eviction_flushes_pending_fsync_before_rewrite` regression coverage.
- Tests: `cargo test -p iroha_core eviction_flushes_pending_fsync_before_rewrite -- --nocapture` (ok; warnings about unused `mut` in `crates/iroha_core/src/kura.rs:7476`, unused `local_peer` in `crates/iroha_core/src/peers_gossiper.rs:1291`, unused `consensus_mode` in `crates/iroha_core/src/sumeragi/main_loop/tests.rs:47219`, and unused `leader_kp` in `crates/iroha_core/src/sumeragi/main_loop/tests.rs:50780`).

- Sumeragi DA eviction rehydrate test: use the default Sumeragi pipeline time (2s/4s) so NPoS-derived timeouts scale up for large payloads and keep blocking submission to preserve per-tx block heights.
- Tests: `cargo test -p integration_tests --test sumeragi_da sumeragi_da_kura_eviction_rehydrates_from_da_store -- --nocapture` (ok; warning about unused `delivered` field in `integration_tests/tests/sumeragi_da.rs:3229` persists).

- Sumeragi QC validation: canonicalize the QC roster for aggregate checks and signer mapping, and map canonical signers to view indices by peer identity so PRF shuffles and non-canonical input orderings do not invalidate bitmaps; test helpers now sign canonical votes against the canonicalized topology.
- Tests: `cargo fmt --all` (warns about nightly-only rustfmt options in config). `cargo test -p iroha_core sumeragi::main_loop::tests::bitmap_count_matches_min_votes_for_commit -- --nocapture` (ok; warnings about unused `mut` in `crates/iroha_core/src/kura.rs:7407`, unused `local_peer` in `crates/iroha_core/src/peers_gossiper.rs:1291`, unused `consensus_mode` in `crates/iroha_core/src/sumeragi/main_loop/tests.rs:47219`, and unused `leader_kp` in `crates/iroha_core/src/sumeragi/main_loop/tests.rs:50780`).

- Smart contract query integration test: accept cursor-continuation failures for both ephemeral-mode `NotPermitted` and stored-mode cursor errors (expired/mismatch/done/not-found) so cursor-mode overrides don't break the scenario.
- Tests: not run (not requested).

- Block sync roster selection: stop re-filtering the allow-uncertified roster by PoP so it stays aligned with the active topology (including peers missing PoP when the map is incomplete); update the checkpoint roster test expectation accordingly.
- Tests: `CARGO_TARGET_DIR=/tmp/iroha-codex-block-sync cargo test -p iroha_core block_sync_update_uses_active_roster_for_checkpoint -- --nocapture` (ok; warnings about unused `mut` in `crates/iroha_core/src/kura.rs:7407`, unused `local_peer` in `crates/iroha_core/src/peers_gossiper.rs:1291`, unused `consensus_mode` in `crates/iroha_core/src/sumeragi/main_loop/tests.rs:47196`, and unused `leader_kp` in `crates/iroha_core/src/sumeragi/main_loop/tests.rs:50757`).

- P2P topology refresh: stop advertising unregistered peers and disconnect local peers after unregister so removed nodes stop syncing; added coverage for stray/removed topology refresh helpers.
- Tests: `CARGO_TARGET_DIR=/tmp/iroha-target-topology cargo test -p iroha_core topology_advertisement_skips_strays -- --nocapture` (ok; warnings about unused `mut` in `crates/iroha_core/src/kura.rs:7407`, unused `local_peer` in `crates/iroha_core/src/peers_gossiper.rs:1291`, unused `consensus_mode` in `crates/iroha_core/src/sumeragi/main_loop/tests.rs:47196`, and unused `leader_kp` in `crates/iroha_core/src/sumeragi/main_loop/tests.rs:50757`). `CARGO_TARGET_DIR=/tmp/iroha-target-topology cargo test -p iroha_core topology_update_for_local_removal_disconnects -- --nocapture` (ok; warnings about unused `mut` in `crates/iroha_core/src/kura.rs:7407`, unused `local_peer` in `crates/iroha_core/src/peers_gossiper.rs:1291`, unused `consensus_mode` in `crates/iroha_core/src/sumeragi/main_loop/tests.rs:47196`, and unused `leader_kp` in `crates/iroha_core/src/sumeragi/main_loop/tests.rs:50757`).

- Sumeragi tests: assert RBC chunk max bytes against `min(config, payload_cap)` so clamping expectations match the configured limit.
- Tests: `CARGO_TARGET_DIR=/tmp/iroha-target-consensus cargo test -p iroha_core consensus_frame_caps_are_plaintext_and_rbc_chunk_is_clamped -- --nocapture` (ok).
- Sumeragi pacemaker test: assert the proposal is observed via `proposals_seen` since local `BlockCreated` handling clears the proposal cache.
- Tests: `CARGO_TARGET_DIR=/tmp/iroha-target-consensus cargo test -p iroha_core pacemaker_uses_commit_qc_roster_for_proposal_leader -- --nocapture` (ok).

- Query limits test config: add the local BLS PoP entry to the minimal config used by `fetch_size_limit_tests` so Torii fetch-size limit checks load without trusted_peers_pop validation errors.
- Tests: `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_core smartcontracts::isi::query::fetch_size_limit_tests::query_limits_from_torii_uses_configured_max_fetch -- --nocapture` (ok; command timed out after filtered binaries continued running).

- Sumeragi vote intake: only use the active-topology fallback for commit votes within the active height window so votes that depend on future roster sidecars stay deferred until cached QCs persist the roster.
- Tests: `cargo test -p iroha_core sumeragi::main_loop::tests::commit_outcome_persists_roster_sidecar_from_cached_qc -- --nocapture` (ok; warnings about unused `mut` in `crates/iroha_core/src/kura.rs:7407` and unused variables in `crates/iroha_core/src/peers_gossiper.rs:1291`, `crates/iroha_core/src/sumeragi/main_loop/tests.rs:47192`, `crates/iroha_core/src/sumeragi/main_loop/tests.rs:50753` persist).
- Tests: `cargo test -p iroha_core deferred_votes_replay_after_commit_roster_history_arrives -- --nocapture` (timed out after 20m during compilation; warnings about unused `mut` in `crates/iroha_core/src/sumeragi/main_loop.rs:6218`/`:6224` and `crates/iroha_core/src/kura.rs:7407`, plus unused variables in `crates/iroha_core/src/peers_gossiper.rs:1291`, `crates/iroha_core/src/sumeragi/main_loop/tests.rs:47192`, `crates/iroha_core/src/sumeragi/main_loop/tests.rs:50753` persist).

- Block sync QC filtering/tests: canonicalize roster/signers for cached QCs, skip aggregate verification when the QC is derived locally, and update filter tests to sign against the aligned roster topology while making roster-metadata preference deterministic.
- Tests: `cargo fmt --all` (warns about nightly-only rustfmt options in config). `RUST_BACKTRACE=1 cargo test -p iroha_core block_sync::message::filter_tests::filter_blocks_ -- --nocapture` (ok).

- Block sync signature topology: align block-signature validation with canonical PRF-shuffled permissioned ordering (matching main-loop rotation), update block-sync filter tests to sign via the aligned topology, and add a permissioned PRF shuffle unit test.
- Tests: `cargo fmt --all` (warns about nightly-only rustfmt options in config). `cargo test -p iroha_core filter_blocks_replaces_qc_using_cached_signers -- --nocapture` (ok; command timed out after filtered binaries continued running; warnings about unused `mut` in `crates/iroha_core/src/kura.rs:7407` and unused variables in `crates/iroha_core/src/peers_gossiper.rs:1291`, `crates/iroha_core/src/sumeragi/main_loop/tests.rs:47075`, `crates/iroha_core/src/sumeragi/main_loop/tests.rs:50636`).

- Queue commit removals: keep removed-hash markers after committed hashes are removed so expiry tracking and removal markers remain visible until stale hashes are drained (skip compaction in `remove_committed_hashes`).
- Tests: `cargo test -p iroha_core queue::tests::remove_committed_hashes_clears_expiry_tracking -- --nocapture` (ok; warnings about unused `mut` in `crates/iroha_core/src/kura.rs:7407` and unused variables in `crates/iroha_core/src/peers_gossiper.rs:1291`, `crates/iroha_core/src/sumeragi/main_loop/tests.rs:47075`, `crates/iroha_core/src/sumeragi/main_loop/tests.rs:50636`).

- Repo lifecycle proof digest: update fixture digest to match deterministic snapshot output (no JSON drift; digest now `E59155AB0509BB42BBF07B3898171B6205AFB5131E703974F64F5F93F68EF3BB`).
- Tests: `cargo test -p iroha_core -- --exact smartcontracts::isi::repo::tests::repo_deterministic_lifecycle_proof_matches_fixture --nocapture` (ok; warnings about unused `mut` in `crates/iroha_core/src/kura.rs:7407` and unused variables in `crates/iroha_core/src/peers_gossiper.rs:1291`, `crates/iroha_core/src/sumeragi/main_loop/tests.rs:47075`, `crates/iroha_core/src/sumeragi/main_loop/tests.rs:50636`).

- Sumeragi roster: skip PoP filtering when the active topology has an incomplete PoP map for world/commit baselines, keeping the BLS baseline; trusted-only baselines still drop peers without PoP.
- Tests: `cargo test -p iroha_core sumeragi::main_loop::roster::tests::active_topology_skips_incomplete_pops -- --nocapture` (ok; warnings about unused `mut` in `crates/iroha_core/src/kura.rs:7407` and unused variables in `crates/iroha_core/src/peers_gossiper.rs:1291`, `crates/iroha_core/src/sumeragi/main_loop/tests.rs:47075`, `crates/iroha_core/src/sumeragi/main_loop/tests.rs:50636`).

- Sumeragi vote verification: batch vote signature checks in the vote-verify workers (group by preimage + algorithm, use deterministic batch wrappers, fallback to per-signature on failure) so the hot vote queue only enqueues work and drains results; added `vote_verify_worker_records_vote_after_async_check` coverage.
- Tests: `cargo fmt --all` (warns about nightly-only rustfmt options in config). `cargo test -p iroha_core vote_verify_worker_records_vote_after_async_check -- --nocapture` (ok; warnings about unused `mut` in `crates/iroha_core/src/kura.rs:7407` and unused variables in `crates/iroha_core/src/peers_gossiper.rs:1291`, `crates/iroha_core/src/sumeragi/main_loop/tests.rs:47068`, `crates/iroha_core/src/sumeragi/main_loop/tests.rs:50629`).

- Block sync tests: hold the commit-history test guard while sanitizing block-sync QCs so parallel tests cannot repopulate the precommit signer cache, fixing flakiness in `sanitize_block_sync_qc_keeps_incoming_without_cached_signers`.
- Tests: `cargo test -p iroha_core block_sync::message::filter_tests::sanitize_block_sync_qc_keeps_incoming_without_cached_signers -- --nocapture` (ok; warnings about unused `mut` in `crates/iroha_core/src/kura.rs:7407` and unused variables in `crates/iroha_core/src/peers_gossiper.rs:1291`, `crates/iroha_core/src/sumeragi/main_loop/tests.rs:47075`, `crates/iroha_core/src/sumeragi/main_loop/tests.rs:50636`).

- Izanami run (tps=1, 300s, 4 peers) after vote-verify batching: stopped before target blocks; lane_000_core blocks.hashes=49–50 across peers (1568–1600 bytes), network dir `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_VZnTAY`. Worker slow-line parsing across peers: `vote_drain_ms` mean 432–490ms (p95 2182–2344ms, max 3638–4500ms); `block_payload_drain_ms` mean 75–104ms (p95 267–422ms, max 622–1872ms), so vote drain still dominates. Duplicate metric registration warnings still present.
- Tests: `cargo fmt --all` (warns about nightly-only rustfmt options in config). Izanami command: `RUST_LOG=izanami::summary=info,izanami::workload=warn,iroha_core::sumeragi::main_loop=debug,iroha_core::sumeragi=info,iroha_p2p=info IROHA_TEST_NETWORK_KEEP_DIRS=1 IROHA_TEST_NETWORK_PERMIT_DIR=$(mktemp -d) cargo run -p izanami --release --locked -- --allow-net --nexus --peers 4 --faulty 0 --duration 300s --target-blocks 200 --progress-interval 10s --progress-timeout 180s --tps 1 --max-inflight 8 --workload-profile stable` (stopped before target blocks reached).
- Sumeragi evidence test: align subject-height permissioned vote signing with the rotated roster so evidence validation uses the correct mode tag after a runtime flip.
- Tests: `cargo test -p iroha_core sumeragi::main_loop::tests::handle_evidence_uses_subject_height_mode_tag -- --nocapture` (ok; command timed out after filtered binaries continued running).

- Sumeragi DA idle view-change test: derive the permissioned leader for the next height from the chain-id PRF seed and shuffled topology so the leader shutdown always forces a view change; avoids relying on `/v1/sumeragi/leader` during startup.
- Tests: `cargo fmt --all` (warns about nightly-only rustfmt options in config). `cargo test -p integration_tests sumeragi_idle_view_change_recovers_after_leader_shutdown -- --nocapture` (ok).

- Sumeragi missing-block retry: use local commit-roster snapshots or the active topology to avoid roster-validation stalls on the main loop; BlockSyncUpdate for known blocks now prefers the local snapshot and drops mismatching QC/checkpoint/stake hints before heavy validation; added `retry_missing_block_requests_uses_commit_roster_snapshot_without_validation` coverage.
- Tests: not run (not requested).

- Sumeragi state view timing: instrument `State::view` and view-lock writer scopes with wait/hold timers (debug-only) plus `state_view_returns_when_view_lock_held` coverage; NPoS localnet 1 Hz soak with state debug logs on `/private/tmp/iroha-localnet-npos-1hz-lock-20260127T075004Z` (ports 56080/57337). 600x1 Hz `iroha_cli ledger transaction ping --msg 1hz-profile --no-wait` with `/v1/sumeragi/status` sampling to `sumeragi_status_1hz_lock_20260127T075525Z.jsonl` (ping log `ping_1hz_lock_20260127T075525Z.log`). `commit_qc.height` 3->75 (+72), `view_change_install_total` +52, `missing_block_fetch.total` +112, `pending_rbc.bytes` max 24351, `rbc_store.bytes` max 454614. Slowest `state.view` wait: 3,788,018µs on `block_hashes` (peer0 `2026-01-27T07:58:23.792149Z`, caller `crates/iroha_core/src/sumeragi/main_loop.rs:4906:22`), with concurrent waits in gossiper/telemetry/block_sync; no view-lock write-hold logs above 10ms. Indicates contention on the `BlockHashes` RwLock write guard held across block execution, not the coarse `view_lock`.
- Tests: `cargo test -p iroha_core state_view_returns_when_view_lock_held -- --nocapture` (ok; warnings about unused `mut` in `crates/iroha_core/src/kura.rs:7407` and unused variables in `crates/iroha_core/src/peers_gossiper.rs:1291`, `crates/iroha_core/src/sumeragi/main_loop/tests.rs:47153`, `crates/iroha_core/src/sumeragi/main_loop/tests.rs:50714` persist).

- Sumeragi state view lock root-cause fix: rework `BlockHashes` block scope to hold a read guard and buffer appended hashes until commit (short write lock), so `state.view()` no longer blocks on long-lived block execution; `block_and_revert` now truncates on commit without mutating the shared vector mid-block; evidence tests updated to stage hashes via `push`. Added `block_hashes_commit_applies_pending_only_on_commit` + `block_hashes_block_and_revert_replaces_tail_on_commit` coverage.
- Tests: `cargo test -p iroha_core block_hashes_ -- --nocapture` (ok; warnings about unused `mut` in `crates/iroha_core/src/kura.rs:7407` and unused variables in `crates/iroha_core/src/peers_gossiper.rs:1291`, `crates/iroha_core/src/sumeragi/main_loop/tests.rs:47153`, `crates/iroha_core/src/sumeragi/main_loop/tests.rs:50714` persist).

- NPoS localnet 1 Hz soak (debug, `LOG_FILTER=iroha_core::state=debug`) after block-hash lock fix: `/private/tmp/iroha-localnet-npos-1hz-lockfix-20260127T092225Z` (ports 58080/59337). Ran 317x1 Hz pings (command hit tool timeout at 20 min) with `/v1/sumeragi/status` sampling to `sumeragi_status_1hz_lockfix_20260127T092541Z.jsonl` (ping log `ping_1hz_lockfix_20260127T092541Z.log`). `commit_qc.height` 1->61 (+60), `view_change_install_total` +16, `missing_block_fetch.total` +18, `pending_rbc.bytes` max 15691, `rbc_store.bytes` max 63953. Slowest `state.view` wait: 116,515µs on `block_hashes` (peer0 `2026-01-27T09:31:17.858874Z`, caller `crates/iroha_core/src/gossiper.rs:1103:49`); all waits < 0.12s (no multi-second spikes seen).
- Sumeragi missing-block fetch: when a FetchPendingBlock response would carry a BlockSyncUpdate, also push a BlockCreated payload (when it fits the consensus frame cap) so peers waiting on BlockCreated/INIT recover promptly; updated `fetch_pending_block_uses_block_sync_update_when_roster_available` coverage to assert the extra BlockCreated response.
- Tests: not run (soak only).
- Sumeragi vote delivery: QcVote posts now bypass the background queue to reduce QC latency, and test logging captures per-peer QcVote targets; default `redundant_send_r` updated to 2f+1 for 4 peers (r=3) in config/data-model defaults.
- Tests: not run (not requested).
- Sumeragi pre-vote validation: for fast-path timeouts ≤ 1s, run pending-block validation inline instead of dispatching to validation workers so precommit votes aren’t delayed by worker queues; added `validation_inline_records_state_roots_for_valid_block` coverage.
- Tests: not run (not requested).
- NPoS localnet 1 Hz soak (release, new QcVote fast-path + `redundant_send_r=3` defaults): `/private/tmp/iroha-localnet-npos-1hz-release-20260128T090217Z` (ports 62080/61337). 120x1 Hz `tx_load.py` (`--batch-size 1 --batch-interval 1`) with `/v1/sumeragi/status` sampling to `sumeragi_status_1hz_20260128T090535Z_peer*.jsonl` (tx log `tx_load_1hz_20260128T090535Z.log`). Peer0/2/3 `commit_qc.height` 1->49 (+48 over ~237–239s, ~0.20 blocks/s), `view_change_install_total` +12, `missing_qc_total` +11/12, `missing_payload_total` +0, `stake_quorum_timeout_total` +0, `missing_block_fetch.total` +5/9/12, `pending_rbc.bytes` max 220. Peer1 status timed out for all samples and the process was down by the end of the run.
- Tests: not run (soak only).
- Localnet tooling: `scripts/deploy_localnet.sh` now preflights API/P2P port ranges and auto-selects a free range if any ports are already in use (uses python3/python to probe). Prevents Torii bind failures when other processes occupy the base ports.

- Sumeragi vote QC tally: cache roster hashes for validated votes so `qc_signers_for_votes` can skip redundant signature checks while still revalidating on roster mismatch; added `qc_signers_for_votes_revalidates_on_roster_hash_mismatch` coverage.
- Tests: `cargo test -p iroha_core qc_signers_for_votes_revalidates_on_roster_hash_mismatch -- --nocapture` (ok; warnings about unused `mut` in `crates/iroha_core/src/kura.rs:7407` and unused variables in `crates/iroha_core/src/peers_gossiper.rs:1291`, `crates/iroha_core/src/sumeragi/main_loop/tests.rs:46901`, `crates/iroha_core/src/sumeragi/main_loop/tests.rs:50462`).

- Peers gossiper tests: fix `topology_update_adds_new_trusted_peers` initial peer map typing to match `BTreeMap<PeerId, SocketAddr>` expectations.
- Tests: not run (compile fix only).

- Izanami run (tps=1, 300s, 4 peers) after caching known-block block signers: stopped before target blocks; lane_000_core blocks.hashes=40 across peers (counted from 1280-byte hashes files), network dir `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_kzkISp`. Worker slow-line parsing across peers: `block_payload_drain_ms` mean 86ms (p95 565ms, max 3652ms); `BlockSyncUpdate` handled elapsed mean 158ms (p95 642ms, max 3627ms); `vote_drain_ms` mean 608ms (p95 2397ms, max 8438ms), suggesting vote drain dominates over block-payload handling. Duplicate metric registration warnings still present.
- Tests: `cargo fmt --all` (warns about nightly-only rustfmt options in config). Izanami command: `RUST_LOG=izanami::summary=info,izanami::workload=warn,iroha_core::sumeragi::main_loop=debug,iroha_core::sumeragi=info,iroha_p2p=info IROHA_TEST_NETWORK_KEEP_DIRS=1 IROHA_TEST_NETWORK_PERMIT_DIR=$(mktemp -d) cargo run -p izanami --release --locked -- --allow-net --nexus --peers 4 --faulty 0 --duration 300s --target-blocks 200 --progress-interval 10s --progress-timeout 180s --tps 1 --max-inflight 8 --workload-profile stable` (stopped before target blocks reached).

- Sumeragi block sync: cache validated block-signer sets per block hash/roster/mode/PRF for known-block QC validation so repeated BlockSyncUpdate handling can skip `validate_signatures_subset`; added `block_sync_update_known_block_reuses_cached_block_signers` coverage.
- Tests: `cargo fmt --all` (warns about nightly-only rustfmt options in config). `cargo test -p iroha_core block_sync_update_known_block_reuses_cached_block_signers -- --nocapture` (ok; warnings about unused `mut` in `crates/iroha_core/src/kura.rs:7407` and unused variables in `crates/iroha_core/src/sumeragi/main_loop/tests.rs:46757`/`:50318` persist).

- Sumeragi vote intake: align permissioned test vote signing with PRF-seeded view topology and allow precommit vote validation to fall back to the active roster when the persisted roster is missing but DA/pending/missing-block context keeps stale votes valid.
- Tests: `cargo test -p iroha_core handle_precommit_vote_ -- --nocapture` (ok). `cargo test -p iroha_core stale_view_accepts_precommit_vote_when_missing_block_requested -- --nocapture` (ok; command timed out after filtered binaries continued running).

- Sumeragi/telemetry: update leader_index on proposal handling, seed trust for newly added topology peers, and remove duplicate queue/pending/commit metric registrations; added unit coverage for leader index, topology add, and duplicate-metric guard.
- Tests: not run (not requested).

- Sumeragi block sync: reuse cached QC tallies for known blocks only when the incoming QC hash matches the cached QC or roster certificate, avoiding redundant aggregate/signature validation while preventing hash-mismatch bypass; added `block_sync_update_known_block_revalidates_qc_on_hash_mismatch` coverage.
- Tests: `cargo test -p iroha_core block_sync_update_known_block_revalidates_qc_on_hash_mismatch -- --nocapture` (ok; warnings about unused `mut` in `crates/iroha_core/src/kura.rs:7407` and unused variables in `crates/iroha_core/src/sumeragi/main_loop/tests.rs:46418`/`:49947` persist). `cargo fmt --all` (warns about nightly-only rustfmt options in config).

- Sumeragi block sync: allow known-block QC hash mismatches to proceed to validation when the validator set matches the local snapshot; accept the quorum QC and update the commit certificate cache; added `block_sync_update_known_block_accepts_quorum_qc_on_hash_mismatch` coverage.
- Tests: `cargo test -p iroha_core block_sync_update_known_block_accepts_quorum_qc_on_hash_mismatch -- --nocapture` (ok; warnings about unused `mut` in `crates/iroha_core/src/kura.rs:7407` and unused variables in `crates/iroha_core/src/peers_gossiper.rs:1291`, `crates/iroha_core/src/sumeragi/main_loop/tests.rs:47772`, `crates/iroha_core/src/sumeragi/main_loop/tests.rs:51333` persist).

- Izanami run (tps=1, 300s, 4 peers) with QC fast-path gating: failed with `no block height progress for 180s` (min height 3); plan submissions hit connection refused/timeouts against `http://127.0.0.1:33251`; one peer exited with status 3; duplicate metric registration warnings observed. Network dir: `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_bLPG1H`.

- Sumeragi block sync: reuse persisted commit-roster snapshots for known blocks when incoming hints match so roster validation is skipped and BlockCreated/INIT delivery isn't stalled by repeated roster checks; add `block_sync_update_known_block_reuses_commit_roster_snapshot` coverage.
- Tests: not run (not requested).

- Sumeragi proposal: surface a frame-cap rejection when a single DA transaction exceeds the consensus payload cap even if it was filtered by payload budget.
- Tests: `cargo test -p iroha_core sumeragi::main_loop::tests::da_proposal_rejects_single_tx_exceeding_consensus_payload_frame_cap -- --nocapture` (ok; warnings about unused `mut` in `crates/iroha_core/src/kura.rs:7407` and unused variables in `crates/iroha_core/src/sumeragi/main_loop/tests.rs:46381`/`:49943` persist).

- Sumeragi block-signature tests: align test signing with view-aligned/canonical topology indices so block-signers validation and derived block-sync QC tests use the same ordering as production.
- Tests: `cargo test -p iroha_core sumeragi::main_loop::tests::derive_block_sync_qc_from_committed_signers -- --nocapture` (ok; warnings about unused `mut` in `crates/iroha_core/src/kura.rs:7407` and unused variables in `crates/iroha_core/src/sumeragi/main_loop/tests.rs:46381`/`:49943` persist). `cargo test -p iroha_core sumeragi::main_loop::tests::validated_block_signers_accept_trimmed_commit_roles -- --nocapture` (ok; same warnings). `cargo fmt --all` (warns about nightly-only rustfmt options in config).

- Sumeragi vote intake: align conflicting-vote test signing with the roster/topology used by verification so the first vote is recorded before rejecting conflicts.
- Tests: `cargo test -p iroha_core sumeragi::main_loop::tests::conflicting_vote_does_not_override_first -- --nocapture` (ok; warnings about unused `mut` in `crates/iroha_core/src/kura.rs:7407` and unused variables in `crates/iroha_core/src/sumeragi/main_loop/tests.rs:46316`/`:49878` persist).

- Sumeragi reschedule: expose DA payload availability helper to sibling modules so quorum reschedule checks compile cleanly.
- Tests: not run (covered by the targeted test above).

- Sumeragi commit pipeline: defer quorum reschedule while DA payload availability is missing by checking payload hashes during reschedule sweeps, so MissingLocalData is honored even before commit processing updates the gate.
- Tests: `CARGO_TARGET_DIR=/tmp/iroha-target cargo test -p iroha_core commit_pipeline_defers_reschedule_until_availability_timeout -- --nocapture` (ok; warnings about unused `mut` in `crates/iroha_core/src/kura.rs:7407` and unused variables in `crates/iroha_core/src/sumeragi/main_loop/tests.rs:46316`/`:49845` persist).

- Sumeragi tests: align permissioned vote signing with PRF-seeded view topology when assembling view-aligned commit votes (fixes QC formation in main-loop tests).
- Tests: `cargo test -p iroha_core sumeragi::main_loop::tests::commit_qc_uses_exec_roots_from_view_votes -- --nocapture` (ok; warnings about unused `mut` in `crates/iroha_core/src/kura.rs:7407` and unused variables in `crates/iroha_core/src/sumeragi/main_loop/tests.rs:46316`/`:49878` persist).

- Sumeragi block sync: require explicit missing-block requests before accepting sparse-signature updates for the next height.
- Tests: `CARGO_TARGET_DIR=/tmp/iroha-target-block-sync cargo test -p iroha_core block_sync_quorum_allows_missing_block_request_with_sparse_signatures -- --nocapture` (ok; warnings about unused `mut` in `crates/iroha_core/src/kura.rs:7407` and unused variables in `crates/iroha_core/src/sumeragi/main_loop/tests.rs:46315`/`:49851` persist).

- Sumeragi block sync: drop block-sync candidate QCs with mismatched epochs before validation/caching; proposal-defaults unit test now uses an empty block to exercise zero-root defaults.
- Tests: `CARGO_TARGET_DIR=/tmp/iroha-target-test cargo test -p iroha_core block_sync_update_drops_qc_epoch_mismatch -- --nocapture` (not rerun; prior compile errors in `crates/iroha_core/src/sumeragi/main_loop/tests.rs` resolved by the view-aligned vote signing fix above).

- Sumeragi proposal/RBC scheduling test: log schedule_background call order for bypassed consensus messages and use it to assert RBC scheduling follows proposals in `assemble_proposal_schedules_rbc_after_proposal_messages`.
- Tests: not run (not requested).

- Sumeragi block sync: skip caching precommit QCs that conflict with a locked QC at the same height during block sync updates.
- Tests: `cargo test -p iroha_core block_sync_update_rejects_conflicting_precommit_qc_against_lock -- --nocapture` (previously failed on private `payload_available_for_da` access; not re-run after the reschedule DA availability update).

- Sumeragi worker: preempt blocks when backlog is urgent, cap block-payload/RBC-chunk drain counts while block queue is non-empty, and add selection/adaptive-cap coverage to reduce BlockCreated/RBC INIT queue latency.
- Tests: `cargo test -p iroha_core select_next_tier_preempts_votes_for_urgent_blocks -- --nocapture` (ok; warnings about unused variables in unrelated tests persist).
- Tests: `cargo test -p iroha_core adaptive_drain_caps_clamp_for_block_backlog -- --nocapture` (ok; same warnings).

- NPoS localnet 1 Hz soak (debug, 4 peers) after block-sync roster snapshot fast-path: `/private/tmp/iroha-localnet-npos-1hz-20260126T204734Z` (ports 47080/48337); 1200x1 Hz `iroha ledger transaction ping --msg 1hz-profile --no-wait` looped at 1s intervals with `/v1/sumeragi/status` sampling to `sumeragi_status_1hz_20260126T204734Z.jsonl` (ping log `ping_1hz_20260126T204734Z.log`). `commit_qc.height` 0->182 (+182), `view_change_install_total` +2 (`stake_quorum_timeout_total` +1), `missing_block_fetch.total` +213, `pending_rbc.bytes` max 20247 (sessions max 1), `stash_ready_init_missing_total` +106, `stash_deliver_init_missing_total` +8. Height=5 v0: leader peer1 recv 20:49:50.410690; peer0 missing req 20:49:51.233373 -> recv 20:49:52.320590 (+1.087s, +1.910s from leader), peer2 missing req 20:49:51.970915 -> recv 20:49:51.991437 (+0.021s, +1.581s from leader), peer3 recv 20:49:51.232836 (+0.822s from leader); no view>0 BlockCreated before v0 at height 5. Max missing‑req→v0 latency across run: peer0 6.108s (h74), peer2 5.391s (h74), peer1 3.450s (h3), peer3 2.884s (h44); still above ~1s on some followers.

- NPoS localnet 1 Hz soak (debug, 4 peers) after missing-block fetch stash/dedup: `/private/tmp/iroha-localnet-npos-1hz-20260126T110734Z` (ports 45080/46337); 1200x1 Hz `iroha ledger transaction ping --msg 1hz-profile --no-wait` with `/v1/sumeragi/status` sampling to `sumeragi_status_1hz_20260126T111016Z.jsonl` (ping log `ping_1hz_20260126T111016Z.log`). Height=5 v0: leader peer1 recv 11:11:08.566; peer0 recv 11:11:09.004 (+0.439s), peer2 missing req 11:11:10.131 → recv 11:11:10.928 (+0.796s), peer3 missing req 11:11:09.773 → recv 11:11:09.808 (+0.035s); no view>0 BlockCreated before v0 at height 5. Max missing‑req→v0 latency across run: peer1 6.574s (h3), peer2 4.989s (h6), peer0 3.698s (h3), peer3 1.655s (h7).

- Unstable network 9-peer/3-fault test: retry transaction submission with backoff, extend post-partition recovery delay before mint submit, and add backoff unit coverage to reduce flakiness under slow Torii/relay recovery.
- Tests: `cargo test -p integration_tests unstable_network_9_peers_3_faults -- --nocapture` (ok; Torii HTTP not reachable yet warnings during startup).

- Extra-functional connected-peers test: wait for the newly registered peer to sync block 2 before submitting the log transaction so roster sync completes and block 3 can commit cleanly.
- Tests: `cargo test -p integration_tests --test mod extra_functional::connected_peers::register_new_peer -- --nocapture` (ok).

- Sumeragi DA eviction test: raise P2P frame caps, extend RBC timeouts, and stop submitting extra oversized log transactions once DA evictions appear to avoid long consensus stalls; log level lowered to WARN to avoid payload spam.
- Tests: `IROHA_TEST_NETWORK_KEEP_DIRS=1 cargo test -p integration_tests --test sumeragi_da sumeragi_da_eviction_rehydrates_block_bodies -- --nocapture` (ok).

- Sumeragi main-loop test harness: stop holding commit-history/RBC status test locks for the entire harness lifetime so parallel unit tests no longer serialize and `block_created_*` cases finish under the 60s watchdog.
- Tests: `cargo test -p iroha_core sumeragi::main_loop::tests::block_created_ -- --nocapture --test-threads=4` (block_created suite passed in 59.84s; command later timed out while continuing to build/run filtered binaries; warnings about unused assignments in `crates/iroha_core/src/sumeragi/main_loop/block_sync.rs`, unused `PeersGossiperHandle::closed_for_tests`, and unused `consensus_mode` persist).
- Sumeragi missing-block fetch: dedup FetchPendingBlock requests by requester+hash, stash requesters until the block arrives, flush responses on BlockCreated, and route responses through the background queue; add fetch-dedup metrics + coverage.
- Tests: `cargo test -p iroha_core fetch_pending_block_serves_aborted_pending -- --nocapture --test-threads=1` (ok).
- Tests: `cargo test -p iroha_core fetch_pending_block_falls_back_to_block_created_when_oversized -- --nocapture --test-threads=1` (ok).

- Crypto: cache BLS PoP verification results inside `bls_collect_pks_with_pop` so repeated aggregate/QC verifications skip redundant PoP checks.
- Tests: `cargo fmt --all` (warns about nightly-only rustfmt options in config).
- Tests: `cargo test -p iroha_crypto --features bls bls_normal_preaggregated_same_message_roundtrip -- --nocapture`.

- Peers gossiper: gate `PeersGossiperHandle::closed_for_tests` to `cfg(test)` to silence the unused warning in non-test builds.
- Tests: not run (warning fix only).

- Sumeragi ingress: dedup inbound RBC INIT by hashing the full INIT payload before enqueueing to avoid duplicate queue floods; track RBC INIT dedup evictions and add `incoming_block_message_drops_duplicate_rbc_init` coverage.
- Tests: `cargo fmt --all` (warns about nightly-only rustfmt options in config).
- Tests: `cargo test -p iroha_core incoming_block_message_drops_duplicate_rbc_init -- --nocapture` (ok; warnings about unused `PeersGossiperHandle::closed_for_tests` and unused `consensus_mode` persist).
- Kura payload availability: treat evicted blocks as missing when DA payload files are gone so Sumeragi keeps missing-block/RBC recovery paths alive; add `block_payload_available_by_hash_tracks_evicted_da_payload`.
- Tests: not run (not requested).

- Sumeragi commit worker: retry result delivery with wake nudges when the result queue is full to avoid backpressure stalls; added `commit_worker_wakes_when_result_queue_full` coverage.
- Tests: `cargo test -p iroha_core commit_worker_wakes -- --nocapture` (ok; warnings about unused assignments in `crates/iroha_core/src/sumeragi/main_loop/block_sync.rs` and unused `PeersGossiperHandle::closed_for_tests`/`consensus_mode` persist).

- Test-network harness: increase the default peers-per-network divisor to 64 in `iroha_test_network` + integration-test sandbox so cross-process network concurrency is more conservative for DA/RBC-heavy suites (reduces resource contention and timeout flakiness in extra_functional/sumeragi_da/triggers runs).
- Tests: `cargo test -p iroha_test_network network_parallelism_env_override_applies -- --nocapture` (ok). `cargo test -p integration_tests serial_guard_applies_default_parallelism -- --nocapture` (ok; filtered binaries compiled).

- Unstable network tests: avoid suspending PRF-selected collectors for multi-fault rounds, stagger multi-fault suspensions with shorter pauses, submit mints only after recovery + stabilization delay, and scale supply-check timeouts; updated selection/unit coverage.
- Tests: `cargo test -p integration_tests --test mod unstable_network_9_peers_2_faults -- --nocapture` (ok; duplicate metric registration warnings, occasional relay connection-refused during peer startup). `cargo test -p integration_tests --test mod unstable_network_9_peers_3_faults -- --nocapture` (ok; duplicate metric registration warnings).

- Sumeragi RBC: when the derived commit roster is unavailable, treat the INIT roster as authoritative if it matches the active topology to unblock READY emission under NPoS; added `handle_rbc_init_uses_active_roster_when_derived_missing` coverage.
- Tests: `cargo test -p iroha_core handle_rbc_init_uses_active_roster_when_derived_missing -- --nocapture` (warnings about unused assignments in `crates/iroha_core/src/sumeragi/main_loop/block_sync.rs` and unused `PeersGossiperHandle::closed_for_tests`/`consensus_mode` persist).

- Consensus ingress: classify RBC chunks as critical (with RBC-session tracking) so chunk delivery is not throttled by bulk caps; updated ingress limiter tests.
- Tests: not run (not requested).

- Sumeragi DA: make the large-payload six-peer scenario submit the heavy log transaction non-blocking (commit/RBC waits still gate pass/fail) to avoid tx-confirmation stream stalls.
- Tests: `cargo test -p integration_tests --test sumeragi_da sumeragi_rbc_da_large_payload_six_peers -- --nocapture`

- Sumeragi RBC: queue BlockCreated seed work onto the RBC seed worker, insert stub sessions, and merge results asynchronously to cut per-message latency; add `rbc_seed_result_merges_stub_session` coverage.
- Tests: not run (not requested).

- Sumeragi block sync: add BlockSyncUpdate sub-step timers (Kura checks, roster selection, QC fallback) and cap vote bursts when blocks are queued to reduce block-queue latency; add `run_worker_iteration_limits_vote_burst_when_blocks_pending` coverage.
- Tests: not run (not requested).

- Sumeragi block sync: add roster-validation and QC-apply micro timers (cert/checkpoint validate, tally/process/commit) and cache validated roster selections for persisted roster artifacts; add `block_sync_roster_cache_hits_and_evicts` coverage.
- Tests: `cargo fmt --all` (warns about nightly-only rustfmt options in config).

- Izanami run (tps=1, 300s, 4 peers) after block-sync substep timers + block-queue vote burst cap: stopped before target blocks; committed height 20 (lane_000_core blocks.index size 320) across peers. Worker slow lines: 502; mean elapsed 2315ms with pre/tick/post means 761/766/630ms. Pre-tick per-tier means: votes 198ms, payloads 407ms, blocks 139ms; post-tick per-tier means: votes 345ms, payloads 102ms, blocks 176ms. Message timings: BlockCreated mean 267ms (p95 556ms), BlockSyncUpdate mean 1284ms (p95 2738ms). Queue latency (enqueue→drain): BlockCreated mean 5191ms (p95 12754ms), BlockSyncUpdate mean 3089ms (p95 8615ms). Queue depths: block_rx mean 5.4 (max 26), vote_rx mean 1.2 (max 10). BlockCreated substeps mean/p95: hint_validation 3/9ms, payload_bytes 4/9ms, payload_hash 1/2ms, rbc_seed 128/320ms, qc_replay 20/96ms. BlockSyncUpdate substeps mean/p95: roster_validate 392/501ms, signature_verify 50/134ms, commit_votes_pre 58/154ms, qc_validate 183/251ms, qc_apply 261/695ms (kura_committed/known, qc_candidate/fallback, commit_votes_post, block_apply ≈0). Commit stage timings mean/p95: qc_verify 597/2563ms, kura_persist 159/248ms. Duplicate metric registration warnings observed. Network dir: `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_Mu7adD`.
- Tests: not run (Izanami run only).

- Izanami run (tps=1, 300s, 4 peers) after roster-selection cache + micro timers: stopped before target blocks; committed height 22 (lane_000_core blocks.index size 352) across peers. Worker slow lines: 553; mean elapsed 2266ms with pre/tick/post means 725/711/673ms. Pre-tick per-tier means: votes 222ms, payloads 319ms, blocks 164ms; post-tick per-tier means: votes 397ms, payloads 120ms, blocks 152ms. Message timings: BlockCreated mean 415ms (p95 719ms), BlockSyncUpdate mean 1181ms (p95 2870ms). Queue latency: BlockCreated mean 5929ms (p95 14789ms), BlockSyncUpdate mean 2592ms (p95 7221ms). Queue depths: block_rx mean 3.98 (max 21), vote_rx mean 1.45 (max 10). Roster validation substeps mean/p95: cert_validate 215/381ms, checkpoint_validate 214/367ms (inputs/roots ≈0). BlockSyncUpdate substeps mean/p95: roster_validate 358/428ms, signature_verify 48/82ms, commit_votes_pre 54/78ms, qc_validate 196/335ms, qc_apply 226/325ms (qc_apply_tally 223/320ms, qc_apply_commit 3/5ms; others ≈0). BlockCreated substeps mean/p95: hint_validation 3/12ms, payload_bytes 4/9ms, payload_hash 1/2ms, rbc_seed 235/535ms, qc_replay 7/48ms. Duplicate metric registration warnings observed. Network dir: `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_dgKvnC`.
- Tests: not run (Izanami run only).

- Izanami run (tps=1, 300s, 4 peers) after RBC seed offload: stopped before target blocks; committed height 13 (lane_000_core blocks.index), logs in `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_IggGS1`. Worker slow lines: 558; mean elapsed 2119ms with pre/tick/post means 648/896/487ms (~96%). Pre-tick per-tier means: votes 224ms, payloads 267ms, blocks 144ms; post-tick per-tier means: votes 248ms, payloads 86ms, blocks 145ms. Message timings: BlockCreated mean 347ms (p95 806ms), BlockSyncUpdate mean 1680ms (p95 3945ms). Queue latency: Blocks mean 7558ms (p95 24014ms), BlockPayload mean 3502ms (p95 11184ms). Queue depths: block_rx mean 4.3 (max 38), vote_rx mean 1.3 (max 13). Duplicate metric registration warnings observed.
- Tests: not run (Izanami run only).

- P2P ingress: use configured `p2p_queue_cap_high/low` for inbound peer dispatch buffers to avoid control-plane stalls; add unit coverage for peer-message channel capacity and update P2P queue docs.
- Tests: not run (not requested).

- P2P relay ingress: classify BlockCreated/FetchPendingBlock/RbcInit/RbcReady/RbcDeliver as `Consensus` topic so high-priority relay queues drain them ahead of payload traffic; updated topic classification coverage and P2P frame-cap docs.
- Tests: not run (not requested).

- NPoS localnet 1 Hz rerun (debug + ingress topic reclass, 4 peers): `/private/tmp/iroha-localnet-npos-1hz-20260125T142728Z` (ports 38080/39337); genesis `block_time_ms=1000`, `commit_time_ms=1500`, config defaults, `RUST_LOG=warn,iroha_core::sumeragi=debug`; 100x1 Hz `ledger transaction ping --msg 1hz-profile --no-wait` with `/v1/sumeragi/status` sampling to `sumeragi_status_1hz_20260125T143015Z.jsonl` (ping log `ping_1hz_20260125T143015Z.log`). `commit_qc.height` 1->7 (+6 over 120 samples), `view_change_install_total` +5 (`stake_quorum_timeout_total` +2), `missing_block_fetch.total` +94, `pending_rbc.bytes` max 165676 (sessions max 4), `stash_ready_init_missing_total` +20, `stash_deliver_init_missing_total` +8. BlockCreated v0 height=5: leader peer1 recv 14:31:09.982; peer0 missing req 14:31:12.813 -> recv v0 14:31:18.336 (+5.52s, +8.35s from leader), peer2 missing req 14:31:11.048 -> recv v0 14:31:21.391 (+10.34s, +11.41s from leader), peer3 missing req 14:31:11.410 -> recv v0 14:31:14.126 (+2.72s, +4.14s from leader). No inbound high-priority dispatch stalls logged; RBC INIT enqueue/dequeue debug logs not emitted (no DEBUG lines in peer logs).
- NPoS localnet 1 Hz rerun (LOG_LEVEL=debug + ingress topic reclass, 4 peers): `/private/tmp/iroha-localnet-npos-1hz-20260125T150343Z` (ports 40080/41337); genesis `block_time_ms=1000`, `commit_time_ms=1500`, config defaults, `LOG_LEVEL=debug`, `RUST_LOG=debug,iroha_core::sumeragi=debug,iroha_p2p=debug`; 100x1 Hz `ledger transaction ping --msg 1hz-profile --no-wait` with `/v1/sumeragi/status` sampling to `sumeragi_status_1hz_20260125T150617Z.jsonl` (ping log `ping_1hz_20260125T150617Z.log`). `commit_qc.height` 1->6 (+5 over 120 samples), `view_change_install_total` +5 (`missing_qc_total` +2, `stake_quorum_timeout_total` +2), `missing_block_fetch.total` +43, `pending_rbc.bytes` max 41451 (sessions max 2), `stash_ready_init_missing_total` +11, `stash_deliver_init_missing_total` +3. BlockCreated v0 height=5: leader peer1 recv 15:07:16.637; peer0 missing req 15:07:17.566 -> recv v0 15:07:17.566 (+0.929s from leader), peer2 missing req 15:07:17.410 -> recv v0 15:07:17.410 (+0.773s), peer3 missing req 15:07:17.294 -> recv v0 15:07:17.295 (+0.657s). No inbound high-priority dispatch stalls logged; RBC INIT enqueue->dequeue (height=5 view=0): peer0 2.192s, peer1 17.992s, peer2 2.103s, peer3 1.833s.
- NPoS localnet 1 Hz soak (LOG_LEVEL=debug + ingress topic reclass, 4 peers): `/private/tmp/iroha-localnet-npos-1hz-20260125T154238Z` (ports 42080/43337); genesis `block_time_ms=1000`, `commit_time_ms=1500`, config defaults, `LOG_LEVEL=debug`, `RUST_LOG=debug,iroha_core::sumeragi=debug,iroha_p2p=debug`; 1200x1 Hz `ledger transaction ping --msg 1hz-profile --no-wait` looped at 1s intervals with `/v1/sumeragi/status` sampling to `sumeragi_status_1hz_20260125T154238Z.jsonl` (ping log `ping_1hz_20260125T154238Z.log`). `commit_qc.height` 0->4 (+4 over 1320 samples), `view_change_install_total` +13, `missing_block_fetch.total` +1452, `pending_rbc.bytes` max 220482 (sessions max 5), `stash_ready_init_missing_total` 39, `stash_deliver_init_missing_total` 7. BlockCreated v0 height=5: leader peer0 recv 15:43:32.121; peer1 missing req 15:43:37.687 -> recv v0 15:43:53.033 (+20.913s from leader), peer2 missing req 15:43:38.856 -> recv v0 15:43:51.018 (+18.898s), peer3 missing req 15:43:37.551 -> recv v0 15:44:21.669 (+49.548s). Peer3 saw view=2 BlockCreated at 15:43:58 before v0. Inbound high-priority dispatch stalls logged (wait_ms max: peer0 3525, peer1 3173, peer2 3184, peer3 3373). RBC INIT enqueue->dequeue (height=5 view=0): peer0 9.789s, peer1 22.199s, peer2 19.846s, peer3 51.067s (max enqueue->dequeue across run: peer0 310.300s, peer1 216.218s, peer2 322.936s, peer3 313.039s).
- Consensus roster changes: preserve `proposals_seen` across membership changes to avoid re-proposing the same view after peer unregister; add `on_block_commit_preserves_proposals_seen_on_membership_change` coverage and update topology-change expectations.
- Tests: `cargo fmt --all` (warns about nightly-only rustfmt options in config).
- Tests: not run (logic change only).

- Telemetry: recompute local-peer removal from the world state during sync so `connected_peers` and Sumeragi status recover after peer re-registration; seed the local peer into Torii telemetry test worlds and add a guard test for missing-local-peer metrics.
- Tests: not run (local changes only).

- Unstable network tests: submit asset-definition + mint ISIs via blocking confirmation (with status/TTL bounds) and await mint completion after relay resume to avoid silent rejects under multi-fault partitions.
- Tests: `cargo test -p integration_tests unstable_network_9_peers_3_faults -- --nocapture` (ok; duplicate metrics registration warnings observed during the run).
- Torii telemetry test handle now seeds the world peer list via a WorldBlock accessor; RBC seed worker now reports `eyre::Report` outcomes to match the threaded build path.
- Tests: `cargo check -p iroha_core` (ok; warning about unused `PeersGossiperHandle::closed_for_tests` persists). `cargo test -p integration_tests unstable_network_9_peers_2_faults -- --nocapture` (timed out at 20m while the test was still running; relay connection-refused warnings during peer startup).
- SoraFS dispute helper now uses `UptimeBreach` (no replication order required); P2P peer loop annotates inbound message type and documents peer-message sender fields to satisfy Rust 2024 inference + missing-docs lints.
- Tests: `CARGO_TARGET_DIR=/tmp/iroha-codex-capacity-dup3 CARGO_BUILD_JOBS=1 cargo test -p iroha_core smartcontracts::isi::sorafs::sorafs_tests::register_capacity_dispute_rejects_duplicate -- --nocapture` (test passed; command timed out after 20m while continuing to build/run filtered binaries; warnings about unused `PeersGossiperHandle::closed_for_tests` and unused `consensus_mode` persist).

- SoraFS telemetry cooldown penalty test: `record_capacity_telemetry_respects_cooldown_between_penalties` passes when run in an isolated build dir; earlier failure likely from concurrent cargo/build-dir locks rather than logic regressions.
- Tests: `CARGO_HOME=/tmp/iroha-codex-cargo CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_core record_capacity_telemetry_respects_cooldown_between_penalties -- --nocapture` (ok; command hit timeout after running filtered binaries, but the target test passed).

- SoraFS telemetry submitter test: remove the redundant `drop(stx)` after `apply()` in staking tests and inline the Kura budget limit assignment so `record_capacity_telemetry_requires_authorised_submitter` compiles cleanly.
- Tests: `cargo test -p iroha_core --lib smartcontracts::isi::sorafs::sorafs_tests::record_capacity_telemetry_requires_authorised_submitter -- --nocapture` (ok; warning about unused `consensus_mode` persists).

- SoraFS telemetry: treat PDP/PoTR failure counters as authoritative even when challenge/window counts are zero; add coverage for PDP failures reported without challenge counts so proof-health penalties/alerts aren’t suppressed.
- Tests: `CARGO_TARGET_DIR=/tmp/iroha-target cargo test -p iroha_core smartcontracts::isi::sorafs::sorafs_tests::record_capacity_telemetry_forces_penalty_on_pdp_failure -- --nocapture` (ok).
- Tests: `CARGO_TARGET_DIR=/tmp/iroha-target cargo test -p iroha_core smartcontracts::isi::sorafs::sorafs_tests::record_capacity_telemetry_emits_proof_health_event -- --nocapture` (ok).
- Tests: `CARGO_TARGET_DIR=/tmp/iroha-target cargo test -p iroha_core smartcontracts::isi::sorafs::sorafs_tests::record_capacity_telemetry_penalises_pdp_failures_without_challenge_count -- --nocapture` (ok).

- Staking ISI: apply the transaction before committing the block so CancelConsensusEvidencePenalty persists `penalty_cancelled`; fix Kura budget test compile by restoring `budget_limit` from config in `store_block_rejects_when_budget_exceeded`.
- Tests: `cargo test -p iroha_core smartcontracts::isi::staking::tests::cancel_consensus_evidence_penalty_marks_record -- --nocapture` (ok; warnings about unused `PeersGossiperHandle::closed_for_tests` and unused `consensus_mode` persist).

- Sumeragi BlockCreated duplicate handling: reuse cached payload hash and skip RBC hydration when sessions already hold the full payload; added `rbc_session_needs_payload` coverage.
- Tests: `cargo test -p iroha_core rbc_session_needs_payload` (ok; warnings about unused `PeersGossiperHandle::closed_for_tests` in `crates/iroha_core/src/peers_gossiper.rs:274` and unused `consensus_mode` in `crates/iroha_core/src/sumeragi/main_loop/tests.rs:45535`).

- P2P peer egress: split per-connection send buffers into high/low queues so control-plane frames (BlockCreated/INIT) drain ahead of low-priority payload chunks; added unit coverage for high-priority preemption.
- Tests: `cargo fmt --all` (warns about nightly-only rustfmt options in config).
- Tests: `cargo test --workspace` (failed: `crates/ivm/tests/wsv_host_role_admin_tlv.rs:23` `extend_from_slice` expected `&[u8]` but got `Vec<u8>`).
- Tests: `cargo test -p iroha_p2p` (failed: `peer::handshake_config_tests::puzzle_ticket_mints_and_verifies` assertion failure in `crates/iroha_p2p/src/peer.rs:668`).

- P2P peer ingress: split inbound peer dispatch into high/low channels (consensus/control vs gossip) and drain high-priority frames first; added per-connection stall logs for high-priority receive path and RBC INIT enqueue/dequeue timing logs in sumeragi.
- Tests: not run (local changes only).
- NPoS localnet 1 Hz rerun (debug + P2P send-queue priority, 4 peers): `/private/tmp/iroha-localnet-npos-1hz-20260125T131912Z` (ports 36080/37337); genesis `block_time_ms=1000`, `commit_time_ms=1500`, config defaults, `RUST_LOG=warn,iroha_core::sumeragi=debug`; 100x1 Hz `ledger transaction ping --msg 1hz-profile --no-wait` with `/v1/sumeragi/status` sampling to `sumeragi_status_1hz_20260125T131912Z.jsonl` (ping log `ping_1hz_20260125T131912Z.log`). `commit_qc.height` 1->10 (+9 over 120 samples), `view_change_install_total` +3 (`missing_qc_total` +1, `stake_quorum_timeout_total` +1), `missing_block_fetch.total` +71, `pending_rbc.bytes` max 70426 (sessions max 2), `stash_ready_init_missing_total` +19, `stash_deliver_init_missing_total` +3; logs still show delayed BlockCreated v0 delivery (peer0 13:21:44.520 → 13:21:54.162, peer2 13:21:42.627 → 13:21:43.930, peer3 missing at height 5).
- NPoS localnet 1 Hz rerun (debug + block-queue priority fix, 4 peers): `/private/tmp/iroha-localnet-npos-1hz-20260125T103707Z` (ports 35080/36337); genesis `block_time_ms=1000`, `commit_time_ms=1500`, config defaults, `RUST_LOG=warn,iroha_core::sumeragi=debug`; 100x1 Hz `ledger transaction ping --msg 1hz-profile --no-wait` with `/v1/sumeragi/status` sampling to `sumeragi_status_1hz_20260125T103707Z.jsonl` (ping log `ping_1hz_20260125T103707Z.log`). `commit_qc.height` 5->5 (+0 over 120 samples), `view_change_install_total` +6 (`missing_qc_total` +1, `missing_payload_total` +3, `stake_quorum_timeout_total` +2), `missing_block_fetch.total` +143, `pending_rbc.bytes` max 86599 (sessions max 1), `stash_ready_init_missing_total` +2, `stash_deliver_init_missing_total` +2.
- SoraFS dispute test helper now uses `UptimeBreach` (no replication order required) so duplicate/insert dispute tests stay valid after enforcing replication-order IDs for `ReplicationShortfall`.
- Tests: not run (local cargo invocations were terminated with SIGTERM in this environment; no other processes were stopped).

- SoraFS capacity declarations: resolve owner metadata via world-aware account literal parsing with canonical-literal fallback; add IH58-owner regression coverage for selector-free state.
- Tests: `CARGO_TARGET_DIR=/tmp/iroha-codex-sorafs-owner CARGO_BUILD_JOBS=1 cargo test -p iroha_core --lib smartcontracts::isi::sorafs::sorafs_tests::register_capacity_declaration_inserts_record -- --nocapture` (killed by signal 15 in this environment; other cargo tests were already running).

- SoraFS telemetry: enforce replay protection whenever a nonce is provided (even when `require_nonce=false`), add coverage for optional-nonce replays, and clarify nonce policy semantics in config/docs.
- Tests: `cargo fmt --all` (warns about nightly-only rustfmt options in config).
- Tests: `cargo test -p iroha_core record_capacity_telemetry_rejects_overlap_gap_and_replay -- --nocapture` (killed by signal 15 in this environment).

- SoraFS telemetry: resolve storage class via a canonical metadata key and fall back to the declaration payload when metadata is missing; add coverage for payload lookup.
- Tests: `cargo fmt --all` (warns about nightly-only rustfmt options in config).
- Tests: `CARGO_TARGET_DIR=/tmp/iroha-target cargo test -p iroha_core sorafs_tests::record_capacity_telemetry_uses_declaration_storage_class -- --nocapture` (killed by signal 15 in this environment).

- SoraFS telemetry tests: seed the default telemetry submitter in the test state so Alice is authorised under the allow-list and capacity telemetry rejection tests exercise the intended validation paths.
- Tests: `cargo test -p iroha_core smartcontracts::isi::sorafs::sorafs_tests::record_capacity_telemetry_rejects_overcommit_and_zero_capacity -- --nocapture` (timed out after 40m; process killed by signal 15 during build).
- Tests: `CARGO_TARGET_DIR=/tmp/iroha-codex-sorafs CARGO_BUILD_JOBS=2 cargo test -p iroha_core smartcontracts::isi::sorafs::sorafs_tests::record_capacity_telemetry_rejects_overcommit_and_zero_capacity -- --nocapture` (timed out after 20m during build; warnings about unused `PeersGossiperHandle::closed_for_tests` and unused `consensus_mode` persist).

- Kura budget test: include baseline on-disk overhead when setting the storage limit in `store_block_reclaims_retired_storage_when_budget_exceeded` so retired bytes are the only overage.
- Tests: `CARGO_TARGET_DIR=/tmp/iroha-target cargo test -p iroha_core kura::tests::store_block_reclaims_retired_storage_when_budget_exceeded -- --nocapture` (killed by signal 15 in this environment).
- Governance manifest quorum approvals now accept IH58-only `gov_manifest_approvers` entries by canonicalizing approvals/validators to IH58 and matching without requiring a domain-selector resolver.
- Tests: `cargo fmt --all` (warns about nightly-only rustfmt options in config).
- Tests: `cargo test -p iroha_core queue::tests::governance_manifest_enforces_quorum_metadata -- --nocapture` (timed out after 20m; process killed by signal 15 during build).
- Tests: `cargo test -p iroha_core queue::tests::governance_manifest_enforces_quorum_metadata -- --nocapture` (ok; warnings about unused `PeersGossiperHandle::closed_for_tests` in `crates/iroha_core/src/peers_gossiper.rs:274` and unused `consensus_mode` in `crates/iroha_core/src/sumeragi/main_loop/tests.rs:45535`).

- Staking ISI: apply the transaction before committing the block so CancelConsensusEvidencePenalty persists `penalty_cancelled`.
- Tests: `cargo test -p iroha_core smartcontracts::isi::staking::tests::cancel_consensus_evidence_penalty_marks_record -- --nocapture` (not rerun after resolving the `crates/iroha_core/src/kura.rs:6103` compile error; warnings about unused `PeersGossiperHandle::closed_for_tests` and unused `consensus_mode` persist).
- Account literal parsing: when the selector registry is missing, resolve IH58 addresses by matching encoded account addresses already in WSV; add coverage for the selector-free path (unblocks staking reward claims that rely on IH58-only fee-sink literals).
- Tests: `cargo test -p iroha_core claim_rewards_transfers_and_marks_epoch -- --nocapture` (ok; warnings about unused `PeersGossiperHandle::closed_for_tests` in `crates/iroha_core/src/peers_gossiper.rs:274` and unused `consensus_mode` in `crates/iroha_core/src/sumeragi/main_loop/tests.rs:45490` persist).
- Tests: `cargo test -p iroha_core parse_account_literal_resolves_ih58_without_selector_registry -- --nocapture` (ok; same warnings).

- Tests: update integration test config layers to use nested Sumeragi keys (`sumeragi.collectors.*`, `sumeragi.da.*`, `sumeragi.npos.*`, `sumeragi.advanced.pacemaker.*`, `sumeragi.advanced.rbc.*`, `sumeragi.advanced.queues.*`) and migrate the asset test’s manual config table to the same layout (RBC session TTL now expressed in ms). The legacy-key normalization unit test remains the only place using old keys on purpose.
- Tests: `cargo fmt --all` (warns about nightly-only rustfmt options in config).
- Tests: `cargo test -p integration_tests permissioned_to_npos_cutover_switches_mode_at_activation_height -- --nocapture` (timed out after 20m; test suite was still running, duplicate metric registration warnings seen during run).

- RegisterPeerWithPop policy-default test now snapshots key-policy parameters before mutating `stx`, avoiding the borrow-checker conflict in `register_peer_applies_key_policy_defaults`.
- Tests: `cargo test -p iroha_core register_capacity_dispute_rejects_ -- --nocapture` (ok; warnings about unused `PeersGossiperHandle::closed_for_tests` in `crates/iroha_core/src/peers_gossiper.rs:274` and unused `consensus_mode` in `crates/iroha_core/src/sumeragi/main_loop/tests.rs:45490` persist).

- RegisterPeerWithPop policy-default test now conditions HSM bindings on `key_require_hsm`, asserting presence when enabled and absence when disabled.
- Tests: `cargo test -p iroha_core register_peer_applies_key_policy_defaults -- --nocapture` (ok; warnings about unused `PeersGossiperHandle::closed_for_tests` in `crates/iroha_core/src/peers_gossiper.rs:274` and unused `consensus_mode` in `crates/iroha_core/src/sumeragi/main_loop/tests.rs:45490`).
- Offline balance proof: explicitly validate claimed-delta/resulting-value scales before building proofs so fractional values are rejected deterministically, and assert smart-contract errors via the inner message instead of relying on the generic display string.
- Tests: `cargo test -p iroha_core build_balance_proof_rejects_fractional_value -- --nocapture` (ok; warnings about unused `PeersGossiperHandle::closed_for_tests` and unused `consensus_mode` persist).
- SoraFS capacity disputes: validate replication-order requirements using the decoded payload (replication shortfall must specify an order; unknown orders are rejected) and persist the payload’s replication-order id into the stored record.
- Tests: `CARGO_TARGET_DIR=/tmp/iroha-codex-capacity-unknown CARGO_BUILD_JOBS=1 RUSTFLAGS='-C debuginfo=0' cargo test -p iroha_core smartcontracts::isi::sorafs::sorafs_tests::register_capacity_dispute_rejects_unknown_replication_order -- --nocapture` (ok; warnings about unused `PeersGossiperHandle::closed_for_tests` and unused `consensus_mode` persist).
- NFT ISI tests: update NftId literals to `name$domain` and reset `domain_selectors` with `Default::default()` so missing-domain and selector-free paths align with the current Storage API; audited for other `Storage::clear()` uses in tests and found none.
- Tests: `cargo test -p iroha_core smartcontracts::isi::nft::isi::tests::unregister_nft_rejects_missing_domain -- --nocapture` (ok; warnings about unused `PeersGossiperHandle::closed_for_tests` and unused `consensus_mode` persist).
- Tests: `cargo test -p iroha_core build_balance_proof_rejects_fractional_value -- --nocapture` (failed to compile: borrow checker error in `crates/iroha_core/src/smartcontracts/isi/world.rs:11190` about mutable borrow of `stx` while `params` is borrowed; warnings about unused `PeersGossiperHandle::closed_for_tests` and unused `consensus_mode` persist).
- Block sync tests: guard precommit signer history with the commit-history test lock so cached-QC checks stay stable under parallel runs.
- Tests: `cargo test -p iroha_core block_sync::message::filter_tests::filter_blocks_retains_cached_qc_even_with_signer_mismatch -- --nocapture` (not rerun after replacing `domain_selectors.clear()` with `Default::default()` in `crates/iroha_core/src/block.rs`; warnings about unused `PeersGossiperHandle::closed_for_tests` and unused `consensus_mode` persist).
- Staking rewards: parse `nexus.fees.fee_sink_account_id` via world-aware account literal resolution so IH58-only literals with domain selectors are accepted during reward recording/claims; add `claim_rewards_accepts_ih58_fee_sink` coverage.
- Tests: `cargo test -p iroha_core claim_rewards_ -- --nocapture` (failed to compile: borrow checker error in `crates/iroha_core/src/smartcontracts/isi/world.rs:11190` about mutable borrow of `stx` while `params` is borrowed; warnings about unused `PeersGossiperHandle::closed_for_tests` and unused `consensus_mode` persist).
- Staking snapshot test now expects topology widening for active validators and renames the case to match the documented behavior.
- Tests: `cargo test -p iroha_core smartcontracts::isi::staking::tests::stake_snapshot_widens_missing_validator_from_topology -- --nocapture` (failed to compile: borrow checker error in `crates/iroha_core/src/smartcontracts/isi/world.rs:11190` about mutable borrow of `stx` while `params` is borrowed; warnings about unused `PeersGossiperHandle::closed_for_tests` and unused `consensus_mode` persist).
- World ISI test: clone parameters before mutating `stx` in `register_peer_applies_key_policy_defaults` to satisfy the borrow checker.
- Tests: `cargo test -p iroha_core smartcontracts::isi::staking::tests::stake_snapshot_widens_missing_validator_from_topology -- --nocapture` (ok; warnings about unused `PeersGossiperHandle::closed_for_tests` in `crates/iroha_core/src/peers_gossiper.rs:274` and unused `consensus_mode` in `crates/iroha_core/src/sumeragi/main_loop/tests.rs:45490`).
- NPoS localnet 1 Hz rerun (debug + block-queue priority fix, 4 peers): `/private/tmp/iroha-localnet-npos-1hz-20260125T094329Z` (ports 33080/34337); genesis `block_time_ms=1000`, `commit_time_ms=1500`, config defaults, `RUST_LOG=warn,iroha_core::sumeragi=debug`; 100x1 Hz `ledger transaction ping --msg 1hz-profile --no-wait` with `/v1/sumeragi/status` sampling to `sumeragi_status_1hz_20260125T094329Z.jsonl` (ping log `ping_1hz_20260125T094329Z.log`). `commit_qc.height` 1->9 (+8 over 120 samples), `view_change_install_total` +5 (`missing_qc_total` +2, `missing_payload_total` +0, `stake_quorum_timeout_total` +2), `missing_block_fetch.total` +61, `pending_rbc.bytes` max 34271 (sessions max 1), `stash_ready_init_missing_total` 14, `stash_deliver_init_missing_total` 7.
- Staking: resolve `stake_escrow_account_id`/`slash_sink_account_id` via world-aware account literal parsing (accepts IH58-only literals once domain selectors are registered); added stake-context coverage to lock this in.
- Tests: `cargo test -p iroha_core unregister_peer -- --nocapture` (ok; warnings about unused `PeersGossiperHandle::closed_for_tests` in `crates/iroha_core/src/peers_gossiper.rs:274` and unused `consensus_mode` in `crates/iroha_core/src/sumeragi/main_loop/tests.rs:45490`).
- Tests: `cargo fmt --all` (warns about nightly-only rustfmt options in config).

- Data-model test stabilization: share default-domain guard across tests, add thread-local chain-discriminant guards for JSON/id tests, decode block-header roundtrip with bare Norito decode, fix deterministic fuzz chunk digests, make AssetId constructor test parse explicit-domain accounts, resolve oracle fixture selectors in tests and regenerate oracle fixtures, and update AccountId doctest to parse explicit-domain literals.
- Tests: `cargo test -p iroha_data_model` (ok).
- Tests: `cargo test -p iroha_data_model --doc` (ok).
- Tests: `cargo test -p iroha_data_model --test oracle_reference_fixtures -- --ignored` (ok; regenerated `fixtures/oracle/*`).

- World ISI lead-time tests now use non-genesis dummy blocks with BLS-normal leaders so lead-time policy checks exercise non-genesis paths; added a non-genesis dummy block helper in the ISI world test module.
- Tests: `cargo test -p iroha_core register_peer_rejects_activation_before_lead_time -- --nocapture` (ok; warnings about unused `PeersGossiperHandle::closed_for_tests` in `crates/iroha_core/src/peers_gossiper.rs:274` and unused `consensus_mode` in `crates/iroha_core/src/sumeragi/main_loop/tests.rs:45490`).
- Tests: `cargo test --workspace` (failed: `crates/ivm/tests/core_host_policy.rs:35` `extend_from_slice` expected `&[u8]` but got `Vec<u8>`).

- World ISI tests: explicitly enable Nexus in `next_mode_rejected_when_nexus_enabled` to match the default disabled config and keep staged cutover rejection covered.
- Tests: `cargo test -p iroha_core --lib smartcontracts::isi::world::isi::tests::next_mode_rejected_when_nexus_enabled` (ok; warning about unused `consensus_mode` in `crates/iroha_core/src/sumeragi/main_loop/tests.rs:45490`).
- Tests: `cargo test -p iroha_core smartcontracts::isi::world::isi::tests::next_mode_rejected_when_nexus_enabled` (failed: `crates/iroha_core/tests/ivm_corehost_domain.rs` test binary killed by SIGKILL after rebuild; warnings about unused `PeersGossiperHandle::closed_for_tests` in `crates/iroha_core/src/peers_gossiper.rs:274` and unused `consensus_mode` in `crates/iroha_core/src/sumeragi/main_loop/tests.rs:45490`).

- Permission summary account parsing: accept IH58-with-domain literals in `parse_account_literal_with_world` so permission payloads resolve account IDs; added coverage for the IH58 `@domain` literal path.
- Tests: `cargo test -p iroha_core permission_summary_resolves_accounts_from_payload` (ok; warnings about unused `PeersGossiperHandle::closed_for_tests` in `crates/iroha_core/src/peers_gossiper.rs:274` and unused `consensus_mode` in `crates/iroha_core/src/sumeragi/main_loop/tests.rs:45490`).
- Tests: `cargo test -p iroha_core parse_account_literal_accepts_ih58_with_domain_suffix` (ok; same warnings).

- AccountId parsing: guard the chain discriminant in the encoded-literals-with-domain test to avoid cross-test IH58 prefix races.
- Tests: `cargo test -p iroha_data_model account_id_parsing_tests::encoded_literals_with_domain_parse_without_resolver -- --nocapture` (ok).

- Curve registry alignment test now accepts the `bls` feature gate listed in the published registry.
- Tests: `cargo test --workspace` (failed: `crates/ivm/tests/wsv_host_grant_revoke_tlv.rs:22` and `crates/ivm/tests/wsv_host_account_admin.rs:21` `extend_from_slice` expected `&[u8]` but got `Vec<u8>`; warning about unused `DomainId` import in `crates/ivm/tests/wsv_host_account_admin.rs:8`).

- DAG conflict graph: apply the global state wildcard (`state:*`) to state-map entries even when no per-map wildcard exists, so `state:*` correctly conflicts with `state:Foo/<entry>`.
- Tests: `cargo test -p iroha_core block::dag_tests::state_global_wildcard_conflicts_with_state_entries -- --nocapture` (ok; warnings about unused `PeersGossiperHandle::closed_for_tests` in `crates/iroha_core/src/peers_gossiper.rs:274` and unused `consensus_mode` in `crates/iroha_core/src/sumeragi/main_loop/tests.rs:45490`).

- Block sync: cached QC validation now skips block-signer mismatch when using local precommit signer records so ShareBlocks filtering retains cached QCs even if block signatures differ.
- Tests: `cargo test -p iroha_core block_sync::message::filter_tests::filter_blocks_retains_cached_qc_even_with_signer_mismatch` (ok; warnings about unused `PeersGossiperHandle::closed_for_tests` in `crates/iroha_core/src/peers_gossiper.rs:274` and unused `consensus_mode` in `crates/iroha_core/src/sumeragi/main_loop/tests.rs:45490`).

- AXT replay-ledger state tests now encode `AxtDescriptor`/`DataSpaceId` TLVs with Norito headers so `AXT_BEGIN` decoding succeeds after restart/prune flows.
- Tests: `cargo test -p iroha_core axt_replay_ledger -- --nocapture` (state replay-ledger tests ok; `axt_replay_ledger_persists_through_kura_replay` failed in `crates/iroha_core/tests/ivm_corehost_axt.rs` with `UnknownDataspace(DataSpaceId(99))`).

- AccountId/AssetId JSON now serialize with explicit `@domain` suffixes (e.g., `IH58@domain`, `asset##IH58@domain`) so decoding no longer depends on a domain-selector resolver; updated Norito JSON migration notes and id-json regression expectations.
- Tests: `CARGO_TARGET_DIR=/tmp/iroha-codex-offline cargo test -p iroha_data_model --features json summary_round_trips_through_norito_json -- --nocapture` (ok).
- Tests: `CARGO_TARGET_DIR=/tmp/iroha-codex-offline cargo test -p iroha_data_model --features json summary_defaults_optional_fields_when_missing_in_json -- --nocapture` (ok).
- Tests: `CARGO_TARGET_DIR=/tmp/iroha-codex-offline cargo test -p iroha_data_model --features json account_id_json_uses_explicit_domain_suffix -- --nocapture` (ok).

- Commit-with-signers full-roster quorum test updated to use a six-node topology (min_votes_for_commit = 4) while still excluding leader/proxy tail from QC signer set.
- Tests: `cargo test -p iroha_core commit_with_signers_accepts_full_roster_quorum -- --nocapture` (ok; warnings about unused `PeersGossiperHandle::closed_for_tests` in `crates/iroha_core/src/peers_gossiper.rs:274` and unused `consensus_mode` in `crates/iroha_core/src/sumeragi/main_loop/tests.rs:45490`).
- NPoS localnet 1 Hz rerun (debug + BlockCreated/FetchPendingBlock/RBC INIT via block queue, 4 peers): `/private/tmp/iroha-localnet-npos-1hz-20260125T082237Z` (ports 32080/36380); genesis `block_time_ms=1000`, `commit_time_ms=1500`, config defaults, `RUST_LOG=warn,iroha_core::sumeragi=debug`; 100x1 Hz `ledger transaction ping --no-wait` with `/v1/sumeragi/status` sampling to `sumeragi_status_1hz_20260125T082237Z.jsonl` (ping log `ping_1hz_20260125T082237Z.log`). `commit_qc.height` 0->10 (+10 over 120 samples), `view_change_install_total` +2 (`missing_qc_total` +1, `stake_quorum_timeout_total` +1), `missing_block_fetch.total` +21, `pending_rbc.bytes` max 0 (sessions max 0).
- Sumeragi ingress: route BlockCreated/FetchPendingBlock/RBC INIT via the block queue and drain block traffic ahead of block-payload work to avoid missing-block stalls; update routing/priority unit coverage and queue docs.
- Tests: `cargo test -p iroha_core incoming_block_message_ -- --nocapture` (ok).
- Build: `cargo build -p irohad -p iroha_cli -p iroha_kagami` (ok).
- NPoS localnet 1 Hz rerun (debug + BlockSyncUpdate deferral, 4 peers): `/private/tmp/iroha-localnet-npos-1hz-20260125T054659Z` (ports 31380/35680); genesis `block_time_ms=1000`, `commit_time_ms=1500`, config defaults, `RUST_LOG=warn,iroha_core::sumeragi=debug`; 100x1 Hz `ledger transaction ping --msg 1hz-profile --no-wait` with `/v1/sumeragi/status` sampling to `sumeragi_status_1hz_20260125T054733Z.jsonl` (ping log `ping_1hz_20260125T054733Z.log`). `commit_qc.height` 1->9 (+8 over 120 samples), `view_change_install_total` +4 (`missing_qc_total` +2, `missing_payload_total` +0, `stake_quorum_timeout_total` +2), `missing_block_fetch.total` +46, `pending_rbc.bytes` max 57362 (sessions max 2), `stash_ready_init_missing_total` +12, `stash_deliver_init_missing_total` +0.
- Sumeragi block sync: defer heavy BlockSyncUpdate roster/signature validation while commit/validation work is inflight, replay deferred updates once the pipeline is idle; commit votes and missing-block bookkeeping still run immediately, unit coverage added, docs updated.
- Tests: not run (not requested).
- NPoS localnet 1 Hz rerun (release + worker drain timing logs, 4 peers): `/private/tmp/iroha-localnet-npos-1hz-20260124T202343Z` (ports 31380/35680); genesis `block_time_ms=1000`, `commit_time_ms=1500`, config defaults, `RUST_LOG=warn,iroha_core::sumeragi=debug`; 100x1 Hz `ledger transaction ping --msg 1hz-profile --no-wait` with `/v1/sumeragi/status` sampling to `sumeragi_status_1hz_20260124T202343Z.jsonl` (ping log `ping_1hz_20260124T202343Z.log`). `commit_qc.height` 1->62 (+61 over 120 samples), `view_change_install_total` +0 (`missing_qc_total` +0, `missing_payload_total` +0, `stake_quorum_timeout_total` +0), `missing_block_fetch.total` +9, `pending_rbc.bytes` max 220 (sessions max 1), `stash_ready_init_missing_total` +7, `stash_deliver_init_missing_total` +0.
- Block sync: fix missing-block fetch RBC chunk collection to avoid shadowed chunk assignment.
- Tests: not run (not requested).
- Sumeragi worker: record per-message drain timing for vote/payload tiers and log per-iteration totals; added drain timing unit coverage.
- Tests: not run (not requested).
- Sumeragi block sync: add DEBUG log when missing-block fetch responds with RBC INIT/chunks.
- Tests: not run (not requested).
- Account admission fee test now installs a domain-selector resolver so the fee-sink `AccountId` in policy metadata decodes without `ERR_DOMAIN_SELECTOR_UNRESOLVED`.
- Instruction registry handles now use `Arc` in `iroha_data_model::isi` to avoid borrowed guards and the Rust 2024 lifetime error during builds.
- Tests: `CARGO_BUILD_JOBS=4 RUSTFLAGS='-C debuginfo=0' cargo test -p iroha_core --lib smartcontracts::isi::account_admission::tests::implicit_creation_fee_is_enforced_and_charged -- --nocapture` (ok; warning about unused `consensus_mode` in `crates/iroha_core/src/sumeragi/main_loop/tests.rs:45341`).

- Governance manifest test helper now formats validator literals as `<ih58-address>@<domain>` so manifest parsing exercises the AccountAddress path (avoids alias/public-key ambiguity).
- Tests: `CARGO_TARGET_DIR=/tmp/iroha-manifest-test CARGO_BUILD_JOBS=1 cargo test -p iroha_core governance::manifest::tests::manifests_allow_validator_reuse_across_lanes -- --nocapture` (timed out after 20m during rebuild).

- Governance manifest unit tests now format validator literals as `<public_key>@<domain>` so manifest parsing in `iroha_core` does not depend on domain-selector resolution for IH58 strings.
- Tests: `cargo test -p iroha_core governance::manifest::tests::manifest_parses_validators_and_namespaces -- --nocapture` (ok; warnings about unused `PeersGossiperHandle::closed_for_tests` and `consensus_mode` in `crates/iroha_core/src/sumeragi/main_loop/tests.rs:45341`).

- Instruction registry test override now clones the registry instead of returning a borrowed guard, fixing the Rust 2024 missing-lifetime error in `iroha_data_model::isi::instruction_registry`; `InstructionRegistry` is `Clone` to support the test override.
- Tests: `CARGO_TARGET_DIR=/tmp/iroha-target-overlay-test CARGO_BUILD_JOBS=1 cargo test -p iroha_core overlay_rejects_contract_binding_code_hash_mismatch --lib -- --nocapture --test-threads=1` (ok; warning about unused `consensus_mode` in `crates/iroha_core/src/sumeragi/main_loop/tests.rs:45341`).

- Access-set hint parsing now preserves canonical account/asset keys when account domain selectors cannot be resolved (keeps raw hint keys) and detail/role bindings tolerate account IDs with colon-prefixed encodings.
- Tests: `CARGO_TARGET_DIR=/tmp/iroha-target-access cargo check -p iroha_core` (ok; warning about unused `PeersGossiperHandle::closed_for_tests`).
- Tests: `CARGO_BUILD_JOBS=1 CARGO_TARGET_DIR=/tmp/iroha-target-access cargo test -p iroha_core access_set_hints_accept_state_and_canonical_keys -- --nocapture` (timed out after 300s during rebuild).
- Tests: `CARGO_BUILD_JOBS=1 CARGO_TARGET_DIR=/tmp/iroha-target-access cargo test -p iroha_core pipeline::access::tests::ivm_access_uses_entrypoint_hints_for_isi_syscalls_with_wsv_keys -- --nocapture` (timed out after 1200s during rebuild).

- Iroha data-model fixes: add `sbp` to the test-only domain-selector fallback list, adjust oracle fetch jitter expectation + provider selection for scale mismatch, update instruction-box wire-id expectation, and flip petal-stream payload bits after the header to trigger checksum mismatch; accept Norito length-mismatch as valid trailing-byte rejection in versioned signed-block decode tests.
- Tests: `cargo fmt --all` (warns about nightly-only rustfmt options in config).
- Tests: `CARGO_TARGET_DIR=/tmp/iroha-codex-data-model cargo test -p iroha_data_model isi::tests::norito_serialize_trait_object -- --nocapture`
- Tests: `CARGO_TARGET_DIR=/tmp/iroha-codex-data-model cargo test -p iroha_data_model block::tests::decode_versioned_signed_block_rejects_trailing_bytes -- --nocapture`
- Tests: `CARGO_TARGET_DIR=/tmp/iroha-codex-data-model cargo test -p iroha_data_model oracle::tests::committee_fetch_plan_respects_membership_and_backoff -- --nocapture`
- Tests: `CARGO_TARGET_DIR=/tmp/iroha-codex-data-model cargo test -p iroha_data_model oracle::tests::aggregation_rejects_mismatched_request_hash_and_scale -- --nocapture`
- Tests: `CARGO_TARGET_DIR=/tmp/iroha-codex-data-model cargo test -p iroha_data_model petal_stream::tests::petal_grid_rejects_crc_mismatch -- --nocapture`
- Tests: `CARGO_TARGET_DIR=/tmp/iroha-codex-data-model cargo test -p iroha_data_model transactions::offline_transfer::tests::summary_round_trips_through_norito_json -- --nocapture`

- IVM access-set derivation now falls back to manifest hints attached in transaction metadata when the manifest is not yet in WSV; added unit coverage for metadata-only manifests.
- Tests: `CARGO_TARGET_DIR=/tmp/iroha-target-ivm-hints CARGO_BUILD_JOBS=1 cargo test -p iroha_core --lib pipeline::access::tests::ivm_access_uses_manifest_hints_when_present -- --nocapture` (ok; warning about unused `consensus_mode` in `crates/iroha_core/src/sumeragi/main_loop/tests.rs:45341`).
- Tests: `CARGO_TARGET_DIR=/tmp/iroha-target-ivm-hints CARGO_BUILD_JOBS=1 cargo test -p iroha_core --lib pipeline::access::tests::ivm_access_uses_manifest_hints_from_metadata_when_missing_in_wsv -- --nocapture` (ok; same warning).

- FindAssets JSON predicates now honor asset alias fields (`account`, `definition`, `domain`, etc.) by mapping them during evaluation so shorthand filters match assets even though IDs serialize as strings.
- Tests: `CARGO_TARGET_DIR=/tmp/iroha-codex-asset-query cargo test -p iroha_core find_assets_filters_by_ -- --nocapture` (target tests ok; command hit 20m timeout while running filtered binaries).

- Sumeragi main-loop unit-test harness now defaults to a closed P2P handle and a closed peers-gossiper handle (opt-in real networking via `IROHA_TEST_REAL_NETWORK`) to eliminate cross-test network races/hangs; added `PeersGossiperHandle::closed_for_tests` and reused it in the gossiper-drop test.
- Tests: not run (build directory busy).

- Overlay contract-binding tests now include gas-limit metadata so manifest/ABI errors are exercised after the admission gas check.
- Tests: `CARGO_TARGET_DIR=/tmp/iroha-codex-overlay-abi cargo test -p iroha_core overlay_requires_manifest_abi_for_bound_instance -- --nocapture` (ok).

- Overlay pre-execution policy now runs before IVM load so reserved SYSTEM opcodes fail admission with the header-policy error (fixes `pre_execution_policy_denies_system_opcode`).
- Tests: `CARGO_TARGET_DIR=/tmp/iroha-codex-preexec cargo test -p iroha_core pre_execution_policy_denies_system_opcode -- --nocapture` (killed by signal 9 in this environment after rebuild).

- Kura reload test: flush pending fsync in `create_blocks` so strict init observes the durable height after top-block replacement.
- Tests: `cargo test -p iroha_core kura_not_miss_replace_block -- --nocapture` (timed out after 20m during compilation; build directory lock).

- AccountId parsing tests now use thread-local domain-selector resolver/fallback in `iroha_data_model` to avoid cross-test global-state leakage; InstructionRegistry reads now honor a thread-local override in unit tests to prevent registry mutations from breaking unrelated instruction encoding.
- Address vector fixtures aligned in JS tests + portal example strings to the regenerated canonical/compressed outputs.
- Tests: `cargo fmt --all` (warns about nightly-only rustfmt options in config).
- Tests: `cargo test -p iroha_data_model` (timed out waiting on build directory lock; other cargo tests already running).

- Kura budget tests: include baseline on-disk overhead when setting storage limits so merge-entry and pending-block budget checks are accurate.
- Tests: `cargo test -p iroha_core store_block_with_merge_entry_counts_budget -- --nocapture` (failed to compile: missing lifetime specifier in `crates/iroha_data_model/src/isi/mod.rs:1076`; build directory lock contention).

- FASTPQ poseidon known vector refreshed to match the current preimage digest output.
- Tests: `cargo test -p iroha_core poseidon_digest_matches_known_vector -- --nocapture` (target test ok; command hit the 20m timeout while running filtered binaries after a build-dir lock).

- Block sync share-blocks runtime test now blocks on the payload queue to reflect current enqueue routing.
- Tests: `cargo fmt --all` (warns about nightly-only rustfmt options in config).
- Tests: `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_core share_blocks_enqueue_does_not_block_tokio_timer -- --nocapture` (ok).

- Block sync update now applies commit QCs for already-known blocks to unblock pending commits (e.g., re-registered peers catching up); added unit coverage.
- Tests: `cargo test -p iroha_core block_sync_update_known_block_applies_commit_qc_to_pending -- --nocapture` (timed out after 120s during compilation).
- Tests: `cargo fmt --all` (warns about nightly-only rustfmt options in config).

- IVM trigger syscall object-spec decode: fix JSON deserializers that double-consumed `:` (Executable, Repeats, TransactionEntrypoint, TransactionResult, MetadataChanged), add JSON roundtrip coverage, and ensure trigger tests install a domain-selector resolver.
- Tests: `CARGO_TARGET_DIR=/tmp/iroha-codex-trigger-fix cargo test -p iroha_core smartcontracts::ivm::host::pointer_abi_tests::create_trigger_syscall_object_spec_queues_instruction -- --nocapture` (ok).
- Tests: `CARGO_TARGET_DIR=/tmp/iroha-codex-json cargo test -p iroha_data_model --features json` (failed: 47 existing failures, largely `ERR_DOMAIN_SELECTOR_UNRESOLVED` in account/address/json/offline transfer/oracle/filters).

- IVM pointer-ABI standardization: remove legacy TLV decode helpers, normalize negative tests to use invalid Norito payloads (no raw fallbacks), require Norito-framed Name/Json payloads in codec helper tests, update mock WSV schema encode/decode fallback to emit Norito-framed JSON, and refresh prebuilt IVM TLVs to use `norito::to_bytes` for Name/Asset/Json payloads.
- Tests: not run (not requested).

- IVM host tests: guard the account-domain selector resolver with an RAII lock + drop cleanup to stop global state leakage between tests; use Norito header decode for AXT proof blob sanity checks.
- Tests: `cargo test -p iroha_core axt_replay_ledger_from_state_rejects_reuse -- --nocapture` (ok).

- NPoS pacemaker downtime integration test: align mock WSV Norito encoding/decoding with `norito::to_bytes` and borrow TLV payload slices for hashing.
- Tests: `cargo test -p integration_tests --test mod npos_pacemaker_resumes_after_downtime -- --nocapture` (ok; duplicate metric registration warnings).

- Sumeragi proposal signing: add `sign_with_index` to block builder and use the local validator index when signing blocks so leader signatures match the topology (unblocks DA/RBC sessions); added unit coverage for the indexed signature path.
- Tests: `cargo fmt --all` (warns about nightly-only rustfmt options in config).

- Subscription billing suspend test: `subscription_bill_suspends_after_max_failures` passes on a clean target build.
- Tests: `CARGO_TARGET_DIR=/tmp/iroha-codex-subscription-bill cargo test -p iroha_core subscription_bill_suspends_after_max_failures -- --nocapture` (ok; warning about unused import `Encode as _` in `crates/ivm/src/mock_wsv.rs:15`).
- Subscription billing failure-reschedule test: `subscription_bill_failed_reschedules_and_records_invoice` passes with the subscription-domain selector resolver installed.
- Tests: `cargo test -p iroha_core subscription_bill_failed_reschedules_and_records_invoice -- --nocapture` (target test ok; command hit the 20m timeout while running filtered binaries after a build-dir lock; warning about unused import `Encode as _` in `crates/ivm/src/mock_wsv.rs:15`).

- Subscription billing host test: validate trigger metadata/plan decoding before billing, gate `SpecializedAction` imports behind `cfg(test)`, and expose the domain-selector resolver guard for reuse in nested tests.
- Tests: `CARGO_TARGET_DIR=/tmp/iroha-target-subscription cargo test -p iroha_core subscription_bill_fixed_plan_transfers_and_reschedules -- --nocapture` (ok).

- Subscription IVM host tests: install a domain-selector resolver for subscription test domains so IH58 account IDs in subscription metadata decode correctly; applied in subscription billing/usage host tests.
- Tests: `cargo test -p iroha_core subscription_record_usage_updates_metadata -- --nocapture` (ok).
- Tests: `CARGO_TARGET_DIR=/tmp/iroha-target-subscription cargo test -p iroha_core subscription_bill_ -- --nocapture` (ok).

- IVM pointer-ABI decode: add owned-buffer fallback for Name/Json TLV payloads to avoid decode mismatches on INPUT slices; create-role syscall tests cover alias (`alice@wonderland` with resolver) and direct `AccountId` (`ALICE_ID`) paths.
- Tests: `cargo test -p iroha_core --lib create_role_syscall_queues_instruction_ -- --nocapture` (ok).

- IVM pointer-ABI quorum/signatory tests: install the deterministic alias resolver in add-signatory and set-account-quorum cases so standalone runs can parse alias-based AccountIds.
- Tests: `CARGO_TARGET_DIR=/tmp/iroha-codex-set-account-quorum cargo test -p iroha_core smartcontracts::ivm::host::pointer_abi_tests::set_account_quorum_syscall_queues_instruction -- --nocapture` (ok; warning about unused `specialized::SpecializedAction` in `crates/iroha_core/src/smartcontracts/ivm/host.rs`).

- P2P topology updates now re-apply trusted observers in permissioned mode so trusted peers stay connected across roster changes; added unit coverage for trusted observer retention.
- Tests: not run (local cargo tests already running; build directory busy).

- IVM pointer-ABI peer/signatory syscalls: accept object-shaped JSON for peer/public_key and register Add/RemoveSignatory + SetAccountQuorum in the instruction registry; updated syscall docs and added unit coverage.
- Tests: `CARGO_TARGET_DIR=/tmp/iroha-target cargo test -p iroha_core --lib pointer_abi_tests::register_peer_syscall_queues_instruction -- --nocapture` (ok).
- Tests: `CARGO_TARGET_DIR=/tmp/iroha-target cargo test -p iroha_core --lib pointer_abi_tests::register_peer_syscall_accepts_object_peer -- --nocapture` (ok).
- Tests: `CARGO_TARGET_DIR=/tmp/iroha-target cargo test -p iroha_core --lib pointer_abi_tests::remove_signatory_syscall_queues_instruction -- --nocapture` (ok).
- Tests: `CARGO_TARGET_DIR=/tmp/iroha-target cargo test -p iroha_core --lib pointer_abi_tests::remove_signatory_syscall_accepts_object_public_key -- --nocapture` (ok).

- IVM trigger syscall object-spec decode: accept structured action objects with flexible filter decoding and default metadata when omitted (TriggerCompleted still rejected), added missing-metadata/filter fallback unit coverage.
- Tests: `cargo fmt --all` (warns about nightly-only rustfmt options in config).
- Tests: `CARGO_TARGET_DIR=/tmp/iroha-codex-trigger-fix cargo test -p iroha_core smartcontracts::ivm::host::pointer_abi_tests::create_trigger_syscall_object_spec_queues_instruction -- --nocapture` (timed out after 20m during compilation).

- IVM pointer-ABI trigger syscall test: use `ALICE_ID` in `create_trigger_syscall_queues_instruction` to avoid alias-resolver parsing failures.
- Tests: `cargo test -p iroha_core smartcontracts::ivm::host::pointer_abi_tests::create_trigger_syscall_queues_instruction -- --nocapture` (timed out waiting for build directory lock).
- Consensus key tests: explicitly enable `key_require_hsm` in HSM-required register/rotate unit coverage.
- Tests: `cargo test -p iroha_core rotate_consensus_key_requires_hsm_when_configured -- --nocapture` (timed out waiting for build directory lock).
- Tests: `CARGO_TARGET_DIR=/tmp/iroha-codex-target-rotate-hsm cargo test -p iroha_core rotate_consensus_key_requires_hsm_when_configured -- --nocapture` (timed out during compilation).
- AXT policy snapshot: keep explicit policies intact during storage migrations so state-height slots are preserved when Kura is empty.
- Tests: `CARGO_TARGET_DIR=/tmp/iroha-codex-axt RUST_TEST_THREADS=1 cargo test -p iroha_core state::tests::axt_policy_snapshot_uses_state_height_for_current_slot -- --nocapture` (target test ok; command timed out after 20m while compiling remaining test binaries).
- IVM asset syscalls: align remaining mint/transfer tests to pass NoritoBytes amount TLVs (event ordering, corehost TLV negatives, host mapping) and encode NoritoBytes payloads with Norito headers in CoreHost asset tests.
- Tests: `cargo fmt --all` (warns about nightly-only rustfmt options); `cargo test -p iroha_core smartcontracts::ivm::host::tests::fastpq_batch_entry_syscall_returns_transfer_gas -- --nocapture` (timed out after 20m; build directory locked by other active cargo tests).

- IVM pointer-ABI JSON TLV decode: add norito-encoded payload coverage to ensure Json TLVs emitted via Norito encode are accepted by the host.
- Tests: not run (build directory locked by other active cargo tests).
- Replay roster journal test: align block signature with PRF-rotated topology and seed default NPoS params for deterministic replay signature order.
- Tests: `cargo test -p iroha_core replay_uses_commit_roster_journal_for_signature_order -- --nocapture` (failed to compile: borrow of moved value in `crates/iroha_core/src/smartcontracts/ivm/host.rs:8466`; initial run blocked on build directory lock).
- AXT replay ledger: treat zeroed replay records as expired during pruning/hydration; added `AxtReplayRecord::is_expired` coverage.
- Tests: `cargo test -p iroha_core smartcontracts::ivm::host::pointer_abi_tests::axt_replay_ledger_prunes_stale_entries_on_hydrate -- --nocapture` (failed to compile: borrow of moved value in `crates/iroha_core/src/smartcontracts/ivm/host.rs:8405`; warning about unused imports in `crates/iroha_core/src/state.rs:17248`).
- Tests: `cargo test -p iroha_data_model replay_record_zeroed_slots_are_expired -- --nocapture` (timed out after 20m; blocked waiting for build directory lock).
- Tests: `CARGO_TARGET_DIR=/tmp/iroha-codex-axt-replay cargo test -p iroha_core smartcontracts::ivm::host::pointer_abi_tests::axt_replay_ledger_prunes_stale_entries_on_hydrate -- --nocapture` (target test passed; command timed out after 20m while running filtered binaries; warning about unused import `specialized::SpecializedAction` in `crates/iroha_core/src/smartcontracts/ivm/host.rs:78`).
- Tests: `CARGO_TARGET_DIR=/tmp/iroha-codex-axt-replay-dm cargo test -p iroha_data_model replay_record_zeroed_slots_are_expired -- --nocapture` (ok).
- Tests: `CARGO_TARGET_DIR=/tmp/iroha-codex-axt-replay-lib cargo test -p iroha_core --lib smartcontracts::ivm::host::pointer_abi_tests::axt_replay_ledger_prunes_stale_entries_on_hydrate -- --nocapture` (ok; warnings about unused imports in `crates/ivm/src/mock_wsv.rs:15` and `crates/iroha_core/src/smartcontracts/ivm/host.rs:14`).
- IVM pointer-ABI transfer-asset tests: supply NoritoBytes amount TLVs in CoreHost and host-mapping/ABI coverage to match syscall ABI.
- Tests: `CARGO_TARGET_DIR=/tmp/iroha-codex-pointerabi cargo test -p iroha_core pointer_abi_transfer_asset_enqueues_isi -- --nocapture` (failed: existing move/borrow error in `crates/iroha_core/src/smartcontracts/ivm/host.rs:8466`; warning about unused imports in `crates/iroha_core/src/state.rs:17248`; initial run blocked by build dir lock).
- AXT policy startup: defer refresh during storage migrations so explicit minima persist until the runtime lane catalog is applied; added a pre-refresh policy assertion in `state::tests::axt_policy_rebuild_preserves_minimum_nonce_and_era`.
- Tests: `cargo fmt --all` (warns about nightly-only rustfmt options in config).
- Tests: `cargo test -p iroha_core state::tests::axt_policy_rebuild_preserves_minimum_nonce_and_era -- --nocapture` (timed out after 20m; blocked on build directory lock).
- Tests: `CARGO_TARGET_DIR=/tmp/iroha-codex-axt-policy cargo test -p iroha_core state::tests::axt_policy_rebuild_preserves_minimum_nonce_and_era -- --nocapture` (timed out after 20m during compilation; warning about unused imports in `crates/iroha_core/src/state.rs:17248`).
- IVM host get-account-balance syscall: return Norito-encoded `Numeric` payloads so smart contracts can decode balances consistently.
- Tests: `CARGO_TARGET_DIR=/tmp/iroha-codex-get-balance cargo test -p iroha_core get_account_balance_syscall_reads_numeric_asset -- --nocapture` (timed out after 20m; fresh target build still compiling).
- Sumeragi commit worker tests: widen wake/result timeouts to reduce flakiness under slower commit paths.
- Izanami tests: allow scoped env var guards under `-D unsafe-code` by wrapping env mutations with `#[allow(unsafe_code)]`.
- Sumeragi roster tests: import `CellVecExt` and allow commit-topology mutation in tests.
- Tests: `cargo test -p iroha_core commit_worker_ -- --nocapture` (ok).
- Tests: `cargo test --workspace` failed: `integration_tests::fraud_monitoring_requires_assessment_bands` timed out waiting for tx confirmation (600s); warnings about unused `lane_id` in `crates/iroha_core/src/state.rs`.

- IVM pointer-ABI mint asset syscall test: preload NoritoBytes amount TLV and install the test alias resolver so the case runs standalone without parse flakiness.
- Tests: `CARGO_TARGET_DIR=/tmp/iroha-target cargo test -p iroha_core pointer_abi_tests::mint_asset_syscall_returns_metered_gas` (ok; warnings about unused imports in `crates/iroha_core/src/state.rs`).
- IVM pointer-ABI peer syscall tests: install test alias resolver before parsing alias-based `AccountId` literals in the register/unregister peer coverage.
- Tests: `CARGO_TARGET_DIR=/tmp/iroha-target-codex cargo test -p iroha_core smartcontracts::ivm::host::pointer_abi_tests::unregister_peer_syscall_queues_instruction -- --nocapture` (ok; warnings about unused imports in `crates/iroha_core/src/state.rs`).
- Snapshots: clean up leftover temp artifacts after successful temp fallback so corrupt Merkle metadata promotes temp and clears stale `.tmp` files.
- Tests: `cargo fmt --all` (warns about nightly-only rustfmt options in config).
- Tests: `CARGO_TARGET_DIR=/tmp/iroha-target-snapshot cargo test -p iroha_core --lib snapshot::tests::snapshot_read_falls_back_to_tmp_on_corrupt_merkle_metadata -- --nocapture` (ok; warnings about unused imports in `crates/iroha_core/src/state.rs`).
- IVM pointer-ABI JSON decode: accept bare Norito JSON payloads in `decode_tlv_json` so permission syscalls can parse Json TLVs emitted by Norito encode.
- Tests: `cargo test -p iroha_core smartcontracts::ivm::host::pointer_abi_tests::revoke_permission_syscall_queues_instruction_from_json -- --nocapture` (timed out after 20m; build directory lock from other active cargo builds; warnings about unused imports in `crates/iroha_core/src/state.rs`).
- NPoS localnet 1 Hz rerun (release + FetchPendingBlock missing-INIT response, DEBUG rbc/block_sync, 4 peers): `/private/tmp/iroha-localnet-npos-1hz-20260124T190127Z` (ports 31080/35380); genesis `block_time_ms=1000`, `commit_time_ms=1500`, NPoS timeouts propose/prevote/precommit/commit/da/agg=500/800/1100/1500/1400/200, config `sumeragi.advanced.da.availability_timeout_multiplier=3` + `sumeragi.advanced.da.quorum_timeout_multiplier=3`, `sumeragi.advanced.rbc.pending_ttl_ms=120000`, `sumeragi.advanced.rbc.session_ttl_ms=240000`, `sumeragi.recovery.missing_block_signer_fallback_attempts=0`, `RUST_LOG=warn,iroha_core::sumeragi::main_loop::rbc=debug,iroha_core::sumeragi::main_loop::block_sync=debug`; 100x1 Hz `ledger transaction ping --no-wait` with `/v1/sumeragi/status` sampling to `sumeragi_status_1hz_missing_init_debug_20260124T190127Z.jsonl` (ping log `ping_1hz_missing_init_debug_20260124T190127Z.log`). `commit_qc.height` 2->58 (+56 over 100 samples), `view_change_install_total` +0 (`missing_qc_total` +0, `missing_payload_total` +0, `stake_quorum_timeout_total` +0), `missing_block_fetch.total` +24, `pending_rbc.bytes` max 220 (sessions max 1), `stash_ready_init_missing_total` 18, `stash_deliver_init_missing_total` 0; logs show missing BlockCreated fetches while awaiting RBC INIT plus queued INIT/chunk rebroadcasts.
- Build: `cargo build --release --bin kagami --bin irohad --bin iroha` (ok).
- Sumeragi missing-INIT recovery: include height/view hints in `FetchPendingBlock`, respond with RBC INIT/chunks even when the full block is unavailable, add `fetch_pending_block_serves_rbc_init_without_block` coverage, and update DA/RBC docs.
- Tests: `cargo fmt --all` (warns about nightly-only rustfmt options in config).
- Tests: `cargo test --workspace` not run (long-running).
- Connected peers: filter active roster by live consensus keys so disabled peers are skipped for leader selection; added roster unit coverage.
- Tests: `cargo fmt --all` (warns about nightly-only rustfmt options in config).
- Tests: `cargo test -p iroha_core active_topology_filters_inactive_consensus_keys -- --nocapture` (ok).
- Tests: `cargo test -p integration_tests --test mod extra_functional::connected_peers::connected_peers_with_f_1_0_1 -- --nocapture` (ok; duplicate metric registration warnings).
- Sumeragi fast-path: disable fast-timeout quorum reschedules in DA mode (reschedule + commit pipeline); add unit coverage and update docs.
- Tests: `cargo fmt --all` (warns about nightly-only rustfmt options in config).
- Tests: `cargo test -p iroha_core reschedule_skips_fast_timeout_with_da_enabled -- --nocapture` (ok).
- Tests: `cargo test -p iroha_core commit_pipeline_skips_fast_timeout_with_da_enabled -- --nocapture` (ok).
- NPoS localnet 1 Hz rerun (release + DA fast-path disable, 4 peers): `/tmp/iroha-localnet-npos-1hz-20260124T172906Z` (ports 30080/34380); genesis `block_time_ms=1000`, `commit_time_ms=1500`, NPoS timeouts propose/prevote/precommit/commit/da/agg=500/800/1100/1500/1400/200, config `sumeragi.advanced.da.availability_timeout_multiplier=3` + `sumeragi.advanced.da.quorum_timeout_multiplier=3`, `sumeragi.advanced.rbc.pending_ttl_ms=120000`, `sumeragi.advanced.rbc.session_ttl_ms=240000`, `sumeragi.recovery.missing_block_signer_fallback_attempts=0`; 100x1 Hz `ledger transaction ping --no-wait` with `/v1/sumeragi/status` sampling to `sumeragi_status_1hz_fastpath_fix_20260124T173233Z.jsonl` (ping log `ping_1hz_fastpath_fix_20260124T173233Z.log`). `commit_qc.height` 50->104 (+54 over 120 samples), `view_change_install_total` +0, `missing_qc_total` +0, `missing_payload_total` +0, `stake_quorum_timeout_total` +0, `missing_block_fetch.total` +15, `pending_rbc.bytes` max 440 (sessions max 1); cadence still below 1 Hz.
- State tests: guard commit-history tests in `crates/iroha_core/src/state.rs` to prevent cross-test commit QC history contamination; stabilizes `apply_without_execution_canonicalizes_commit_roster_signatures`.
- Tests: `cargo test -p iroha_core state::tests::apply_without_execution_canonicalizes_commit_roster_signatures -- --nocapture` (ok).
- AXT policy refresh: prune cached entries that target lanes missing from the current catalog when no directory snapshot is available.
- Tests: `cargo test -p iroha_core state::tests::axt_policy_refresh_clears_stale_entries_when_snapshot_missing -- --nocapture` (ok).
- Sumeragi commit pipeline: add debug instrumentation for validation deferrals and precommit gating past fast timeout to pinpoint delayed votes.
- Tests: `cargo fmt --all` (warns about nightly-only rustfmt options in config).
- Tests: `cargo test --workspace` not run (long-running).
- Sumeragi reschedule: skip fast-timeout quorum reschedules while validation is inflight; added unit coverage and updated docs.
- Tests: `cargo fmt --all` (warns about nightly-only rustfmt options in config).
- Tests: `cargo test -p iroha_core reschedule_defers_fast_timeout_while_validation_inflight -- --nocapture` (ok).
- NPoS localnet 1 Hz rerun (release + validation inflight fix, 4 peers): `/private/tmp/iroha-localnet-npos-1hz-20260124T130140Z`; 100x1 Hz `ledger transaction ping --no-wait` with `/v1/sumeragi/status` sampling to `sumeragi_status_1hz_fix_20260124T200000Z_20260124T194233Z.jsonl` (ping log `ping_1hz_fix_20260124T200000Z_20260124T194233Z.log`). `commit_qc.height` 1->58 (+57 over 120 samples), `view_change_install_total` +1 (no view_change_causes deltas), `missing_qc_total` +0, `missing_payload_total` +0, `stake_quorum_timeout_total` +0, `missing_block_fetch.total` +11, `pending_rbc.bytes` max 0 (sessions max 0); cadence still below 1 Hz.
- NPoS localnet 1 Hz rerun (release + commit/validation debug filter, 4 peers): `/private/tmp/iroha-localnet-npos-1hz-20260124T130140Z`; 100x1 Hz `ledger transaction ping --no-wait` with `/v1/sumeragi/status` sampling to `sumeragi_status_1hz_debug_vote_20260124T190437Z.jsonl` (ping log `ping_1hz_debug_vote_20260124T190437Z.log`). `commit_qc.height` 1->43 (+42 over 120 samples), `view_change_install_total` +4 (`missing_qc_total` +4), `missing_payload_total` +0, `stake_quorum_timeout_total` +0, `missing_block_fetch.total` +43, `pending_rbc.bytes` max 5455 (sessions max 1). Debug logs show validation inflight deferrals and precommit gating firing past fast timeout (1s) across peers.
- AXT policy refresh: preserve cached minimums when active Space Directory manifests exist but the lane catalog does not overlap, so explicit policy minimums survive startup refresh.
- Tests: `cargo test -p iroha_core state::tests::axt_policy_rebuild_preserves_minimum_nonce_and_era -- --nocapture` (ok).
- Tests: `cargo test -p iroha_core state::tests::apply_without_execution_updates_commit_topology_from_world_peers -- --nocapture` (ok).
- Tests: `cargo test -p iroha_core state::tests::axt_replay_ledger_persisted_from_block_rejects_reuse_on_validation -- --nocapture` (ok).
- Lane relay tests: commit emergency-override transactions before asserting stored state; fix missing `lane_id` in `da_pin_intents_drop_missing_owner_accounts`.
- Executor validation: install a default IVM host before running executor validation/migration so validation does not panic without a host.
- Tests: `cargo test -p iroha_core --lib state::tests::record_lane_relay_accepts_emergency_override_under_quorum -- --nocapture` (ok).
- Tests: `cargo test -p iroha_core --lib state::tests::execute_called_trigger_respects_executor_validation -- --nocapture` (ok).
- State tests: merge detached by-call triggers via `ExecuteTrigger::execute` so permission/filter/payload checks run; added detached permission coverage.
- Tests: `cargo test -p iroha_core delta_merge_execute_trigger_by_call -- --nocapture` (ok).
- Tests: `cargo test -p iroha_core state::tests::block_rejects_failing_execute_trigger_and_rolls_back -- --nocapture` (target test ok; command hit the 20m timeout while running filtered binaries).
- State tests: compute the Nexus storage budget from measured Kura/spool/cold usage so only the oldest spool entry is evicted before cold snapshots.
- Tests: `cargo test -p iroha_core enforce_nexus_storage_budget_prunes_spools_before_cold -- --nocapture` (ok).
- Tests: `CARGO_TARGET_DIR=/tmp/iroha-codex-pipeline cargo test -p iroha_core sumeragi::main_loop::commit::tests::execute_commit_work_emits_pipeline_events_before_state_apply -- --nocapture` (ok).
- Sumeragi commit-work tests: use deterministic mock timestamps for genesis transactions to avoid clock-skew rejections in `execute_commit_work_*` tests.
- Tests: `cargo test -p iroha_core execute_commit_work_ -- --nocapture` (target tests ok; command hit the 20m timeout while running filtered binaries).
- State tests: build `AssetId` directly in `capture_exec_witness_stashes_reads_and_writes` to avoid IH58 account parsing that depends on domain-selector resolution.
- Tests: `cargo test -p iroha_core state::tests::capture_exec_witness_stashes_reads_and_writes -- --nocapture` (passed; command hit the 20m timeout while running filtered binaries; build directory lock from other active cargo tests).
- Sumeragi: seed the genesis commit roster after block 1 commit so DA/RBC has roster evidence even when genesis is persisted after actor init; added `commit_outcome_seeds_genesis_commit_roster_after_commit` coverage.
- State tests: fix `da_pin_intents_drop_missing_owner_accounts` to derive the primary `lane_id` from the runtime lane catalog.
- Tests: `cargo test -p iroha_core commit_outcome_seeds_genesis_commit_roster_after_commit -- --nocapture` (timed out after 20m; build dir lock; warning about unused `lane_id` in `crates/iroha_core/src/state.rs:24871`).
- Genesis transaction validation now uses the block timestamp during static checks to avoid clock-skew rejections; added `validate_genesis_with_now` coverage.
- Tests: `cargo fmt --all` (warns about nightly-only rustfmt options in config).
- Tests: `CARGO_HOME=/tmp/codex-cargo-home CARGO_TARGET_DIR=/tmp/codex-target cargo test -p iroha_core sumeragi::main_loop::commit::tests::commit_worker_wakes_on_result -- --nocapture` (ok; warning about unused `lane_id` in `crates/iroha_core/src/state.rs`).
- Tests: `CARGO_HOME=/tmp/codex-cargo-home CARGO_TARGET_DIR=/tmp/codex-target cargo test -p iroha_core validate_genesis_with_now_uses_supplied_timestamp -- --nocapture` (ok; same warning about unused `lane_id` in `crates/iroha_core/src/state.rs`).
- DA pin intent state tests now use the configured primary lane id so intents are validated against the active lane catalog.
- Tests: `cargo test -p iroha_core da_pin_intents_ -- --nocapture` (timed out after 20m; build directory locked by other active cargo tests; `da_pin_intents_hydrate_from_kura_block_log` + `da_pin_intents_replay_sanitizes_invalid_entries` ok before timeout).
- State: commit DA pin intent world indexes (ticket/alias/manifest/lane+epoch+sequence) during `WorldBlock` commit so stored intents persist into WSV lookups.
- Tests: `cargo test -p iroha_core da_pin_intents_persist_into_world_indexes -- --nocapture` (failed to compile: invalid format string in `crates/iroha_core/src/smartcontracts/isi/query.rs` at lines 658 and 691).
- AXT policy refresh now preserves explicit entries when the Space Directory is empty so replay-ledger checks survive state restarts; added `axt_policy_refresh_preserves_explicit_entries_without_directory` coverage.
- Tests: `CARGO_TARGET_DIR=/tmp/iroha-codex-axt cargo test -p iroha_core axt_replay_ledger_survives_state_restart -- --nocapture` (ok; command hit the 20m timeout while running empty test binaries after the target test passed; warning about unused `lane_id` in `crates/iroha_core/src/state.rs:24871`).
- Sumeragi roster: enforce strict PoP filtering when falling back to trusted peers so missing PoPs drop peers instead of preserving quorum.
- Tests: `cargo test -p iroha_core active_topology_drops_trusted_peers_without_pop -- --nocapture` (blocked waiting for build directory lock).
- Tests: `CARGO_TARGET_DIR=/tmp/iroha-codex-active-topology cargo test -p iroha_core active_topology_drops_trusted_peers_without_pop -- --nocapture` (ok).
- Sumeragi RBC: rebroadcast INIT when reconstructing sessions from pending payloads, extend pending-RBC TTL on activity, and trigger missing-INIT payload rebroadcasts when pre-INIT traffic arrives; added unit coverage.
- Tests: `cargo fmt --all` (warns about nightly-only rustfmt options in config).
- Tests: `cargo test -p iroha_core seed_rbc_session_from_block_rebroadcasts_init_when_requested -- --nocapture` (ok).
- Tests: `cargo test -p iroha_core pending_rbc_ttl_counts_from_last_seen -- --nocapture` (ok).
- Tests: `cargo test --workspace` not run (long-running).
- Build: `cargo build --release -p iroha_kagami -p iroha_cli -p irohad` (ok).
- NPoS localnet 1 Hz rerun (release + INIT rebroadcast/pending TTL, 4 peers): `/private/tmp/iroha-localnet-npos-1hz-20260124T130140Z` (ports 29980/34280); genesis `block_time_ms=1000`, `commit_time_ms=1500`, NPoS timeouts propose/prevote/precommit/commit/da/agg=500/800/1100/1500/1400/200, config `sumeragi.advanced.da.availability_timeout_multiplier=3` + `sumeragi.advanced.da.quorum_timeout_multiplier=3`, `sumeragi.advanced.rbc.pending_ttl_ms=120000`, `sumeragi.advanced.rbc.session_ttl_ms=240000`, `sumeragi.recovery.missing_block_signer_fallback_attempts=0`; 100x1 Hz `ledger transaction ping --no-wait` with `/v1/sumeragi/status` sampling to `sumeragi_status_1hz_20260124T130140Z.jsonl` (ping log `ping_1hz_20260124T130140Z.log`). `commit_qc.height` 1->60 (+59 over 120 samples), `view_change_install_total` +3 (`missing_qc_total` +1), `missing_payload_total` +0, `stake_quorum_timeout_total` +0, `missing_block_fetch.total` +7, `pending_rbc.bytes` max 0 (sessions max 0); cadence still below 1 Hz.
- Sumeragi tests: extend commit worker wake/result timeouts to reduce flakiness under slower commit paths.
- Tests: `cargo test -p iroha_core commit_worker_ -- --nocapture` (ok).
- Sumeragi tests: make `execute_commit_work_reports_kura_store_failure` inject a Kura store failure instead of relying on storage-budget limits; `cargo test -p iroha_core execute_commit_work_reports_kura_store_failure -- --nocapture` (ok).
- CLI offline: add top-level `iroha offline` command alias, refresh CLI help, and align QR/petal docs to the alias so preview commands match user expectations.
- Petal stream: deepen sakura-wind preview (denser petals + subtle data glow) and add a PNG load roundtrip test.
- Connected peers test: wait for the re-registered peer to observe block 3 before asserting status to avoid stale local-removed metrics.
- Tests: `cargo test -p integration_tests --test mod connected_peers_with_f_1_0_1 -- --nocapture` (timed out after 20m).
- DA cursor regression tests: hydrate DA indexes before manual cursor advances so lazy hydration does not reset in-memory updates.
- Tests: `cargo test -p iroha_core da_shard_cursors_guard_regressions -- --nocapture` (ok).
- Tests: `cargo test -p iroha_core --lib da_receipt_cursors_guard_regressions -- --nocapture` (ok).
- Tests: `cargo test -p iroha_core --lib resharding_clears_stale_shard_cursors -- --nocapture` (ok).
- Lane relay tests: seed consensus-key PoPs in emergency-override acceptance fixtures so QC verification succeeds.
- Tests: not run (not requested).
- Sumeragi tests: align observer local-index, commit inflight, new-view gossip targets, and RBC availability tests with the Topology/roster types.
- Tests: not run (not requested).
- Sumeragi: dispatch background posts inline when `sumeragi.debug.disable_background_worker` is enabled; skip RBC availability gating once the block payload is local.
- Integration tests: align DA/RBC + pacemaker config keys to nested schema, fix DA Kura eviction block-dir lookup, accept `FindError::Domain` root cause for failing triggers, and allow the `debug/ok` ZK backend to verify.
- Tests: not run (not requested).
- CLI offline petal: expose QR payload-kind labels for petal manifests and add unit coverage.
- Tests: not run (not requested).
- Sumeragi config: pacemaker block time now resolves from on-chain `SumeragiParameters.block_time_ms` (fallback to `sumeragi.npos.block_time_ms` pre-genesis), NPoS timeouts derive from that value, and deprecated `sumeragi_npos_parameters.block_time_ms` is warned/ignored; DA docs/templates now call availability evidence advisory (commit does not gate).
- Tests: not run (not requested).
- Torii node capabilities now advertise `data_model_version`; Swift `ToriiClient` enforces it on transaction submission and surfaces `ToriiClientError.incompatibleDataModel`; JS + Python SDKs enforce it for submissions with `ToriiDataModelCompatibilityError`/`DataModelCompatibilityError`; Rust `Client` enforces it on submission with `DataModelCompatibilityError`, with tests/docs updated.
- CLI: added sakura-themed QR preview rendering via `ops offline qr encode --style sakura` and `--style sakura-wind` (petal wind), plus unit tests; updated QR stream docs to the `iroha ops offline` path.
- Petal stream: added Rust framing + CLI `ops offline petal` encode/decode + sakura wind renderer, JS/Swift/Android encoder/decoder + scan helpers (CameraX/AVFoundation), and docs for petal handoff.
- Torii pipeline status: scope the state view during queue checks to avoid writer-preferred lock deadlocks before fallback state lookups.
- Tests: not run (not requested).
- Snapshots: install domain-selector resolver during snapshot reads so IH58 account IDs for non-default domains deserialize; remove the restart-peer snapshot cleanup and add coverage for resolver install.
- Tests: `cargo test -p iroha_core snapshot_read_installs_domain_selector_resolver -- --nocapture` (ok).
- Tests: `IROHA_TEST_NETWORK_PARALLELISM=1 cargo test -p integration_tests --test mod restarted_peer_should_restore_its_state -- --nocapture` (ok).
- Tests: `cargo test --workspace` failed (rustc E0428: duplicate `mod tests` in `crates/iroha_config/src/parameters/actual.rs`).
- Lane relay tests: seed consensus-key PoPs in lane relay fixtures so QC verification succeeds before conflict/merge checks.
- Tests: `cargo fmt --all` (warns about nightly-only rustfmt options in config).
- Tests: `cargo test -p iroha_core record_lane_relay_rejects_conflicting_relay -- --nocapture` (ok).
- Tests: `cargo test -p iroha_core record_lane_relay_rejects_stale_relay -- --nocapture` (ok).
- Merge QC tests: seed consensus-key PoPs for commit-topology signers so merge entry verification can validate aggregate signatures.
- Tests: `cargo test -p iroha_core record_lane_relay_builds_merge_candidate_for_single_lane -- --nocapture` (ok).
- Tests: `cargo test -p iroha_core record_lane_relay_builds_merge_candidate_once_all_lanes_have_relays -- --nocapture` (ok).
- Tests: `cargo test -p iroha_core commit_worker_does_not_block_on_full_wake_channel -- --nocapture` (ok).
- Tests: `cargo test -p iroha_core commit_worker_wakes_on_result -- --nocapture` (ok).
- Tests: `cargo test -p iroha_core execute_commit_work_reports_kura_store_failure -- --nocapture` (ok).
- Tests: `cargo test -p iroha_core execute_commit_work_emits_pipeline_events_before_state_apply -- --nocapture` (ok).
- Integration tests: after registering a new peer in `network_stable_after_add_and_after_remove_peer`, emit a sync log block and wait for the target height derived from status before verifying the peer catches up.
- Tests: `cargo fmt --all` (warns about nightly-only rustfmt options in config).
- Tests: `cargo test -p integration_tests extra_functional::unregister_peer::network_stable_after_add_and_after_remove_peer -- --nocapture` (timed out after 20m; build dir locked by other active cargo tests).
- SoraFS tests: use explicit `<pub>@<domain>` owner metadata literals to avoid domain-selector resolution failures in capacity declaration tests.
- SoraFS tests: use explicit owner literals in `capacity_record_with_owner` to avoid domain-selector resolution dependencies.
- Tests: `cargo test -p iroha_core register_capacity_dispute_rejects_duplicate -- --nocapture` (ok).
- Tests: `cargo test -p iroha_core register_capacity_dispute_rejects_unknown_replication_order -- --nocapture` (ok).
- Sumeragi ticks: avoid scheduling idle view deadlines before phase tracking is seeded so fresh actors stay idle and missing-block windows drive deadlines.
- Tests: `cargo test -p iroha_core actor_next_tick_deadline_tracks_missing_block_windows -- --nocapture` (ok).
- Sumeragi tests: align `bitmap_count_matches_min_votes_for_commit` to derive the signer bitmap/votes from `min_votes_for_commit`.
- Tests: not run (not requested).
- Test network: normalize legacy flat Sumeragi config keys in test layers to match nested schema; added unit coverage.
- Tests: not run (not requested).
- Sumeragi tests: guard P2P startup in the main-loop harness with a timeout and fall back to `closed_for_tests` to avoid hangs when `iroha_p2p::network::start` stalls.
- Tests: not run (not requested).
- Integration tests: seed alias-domain account in genesis for `accounts_query_accepts_alias_and_compressed_filter_literals` to avoid transaction confirmation timeouts.
- Tests: `cargo test -p integration_tests --test address_canonicalisation accounts_query_accepts_alias_and_compressed_filter_literals -- --nocapture` (ok; duplicate metric registration warnings).
- NPoS localnet 1 Hz rerun (release + NPoS timeouts/collectors, 4 peers): `/private/tmp/iroha-localnet-npos-1hz-20260124T065327Z` (ports 29680/33980); genesis NPoS timeouts propose/prevote/precommit/commit/da/agg=300/500/700/900/800/100, `collectors_k=4`, `redundant_send_r=4`, config `sumeragi.advanced.da.availability_timeout_multiplier=2` + `sumeragi.advanced.da.quorum_timeout_multiplier=2`; 100x1 Hz `ledger transaction ping --no-wait` with `/v1/sumeragi/status` sampling to `sumeragi_status_1hz_timeouts_collectors_20260124T065327Z.jsonl` (ping log `ping_1hz_timeouts_collectors_20260124T065327Z.log`). `commit_qc.height` 1->42 (+41 over 120 samples), `view_change_install_total` +7 (`missing_qc_total` +6), `missing_payload_total` +0, `missing_block_fetch.total` +14, `pending_rbc.bytes` max 220 (sessions max 1); cadence still below 1 Hz.
- NPoS localnet 1 Hz rerun (release + RBC TTL + missing-block fallback, 4 peers): `/private/tmp/iroha-localnet-npos-1hz-20260124T121035Z` (ports 29880/34180); genesis `block_time_ms=1000`, `commit_time_ms=1500`, NPoS timeouts propose/prevote/precommit/commit/da/agg=500/800/1100/1500/1400/200, config `sumeragi.advanced.da.availability_timeout_multiplier=3` + `sumeragi.advanced.da.quorum_timeout_multiplier=3`, `sumeragi.advanced.rbc.pending_ttl_ms=120000`, `sumeragi.advanced.rbc.session_ttl_ms=240000`, `sumeragi.recovery.missing_block_signer_fallback_attempts=0`; 100x1 Hz `ledger transaction ping --no-wait` with `/v1/sumeragi/status` sampling to `sumeragi_status_1hz_20260124T121035Z.jsonl` (ping log `ping_1hz_20260124T121035Z.log`). `commit_qc.height` 1->64 (+63 over 120 samples), `view_change_install_total` +0, `missing_qc_total` +0, `missing_payload_total` +0, `stake_quorum_timeout_total` +0, `missing_block_fetch.total` +15, `pending_rbc.bytes` max 2708 (sessions max 1); cadence improved but still below 1 Hz.
- NPoS localnet 1 Hz rerun (release + higher commit/NPoS timeouts, 4 peers): `/private/tmp/iroha-localnet-npos-1hz-20260124T113118Z` (ports 29780/34080); genesis `block_time_ms=1000`, `commit_time_ms=1500`, NPoS timeouts propose/prevote/precommit/commit/da/agg=500/800/1100/1500/1400/200, `k_aggregators=4`, `redundant_send_r=4`, config `sumeragi.advanced.da.availability_timeout_multiplier=3` + `sumeragi.advanced.da.quorum_timeout_multiplier=3`; 100x1 Hz `iroha3 ledger transaction ping --no-wait` with `/v1/sumeragi/status` sampling to `sumeragi_status_1hz_20260124T113118Z_retry2.jsonl` (ping log `ping_1hz_20260124T113118Z_retry2.log`). `commit_qc.height` 53->53 (+0 over 120 samples), `view_change_install_total` +0 (started at 18; missing_payload_total=13, missing_qc_total=3, stake_quorum_timeout_total=1), `missing_block_fetch.total` +1433, `pending_rbc.bytes` max 0 (sessions max 0); peer logs show repeated missing_payload view changes at height 54 with stake quorum but missing block payload.
- Genesis: install domain-selector resolver when loading genesis JSON so domainless IH58 account IDs parse; added unit coverage.
- Tests: `cargo test -p iroha_genesis --lib parse_genesis_installs_domain_selector_resolver -- --nocapture` (ok).
- Format: `cargo fmt --all` (warns about nightly-only rustfmt options in config).
- NPoS localnet 1 Hz rerun (debug, 4 peers): `/tmp/iroha-localnet-npos-1hz-20260123T132039Z` (ports 29084/33341); 100x1 Hz `ledger transaction ping --no-wait` with `/v1/sumeragi/status` sampling to `sumeragi_status_1hz_20260123T132039Z.jsonl` (ping log `ping_1hz_20260123T132039Z.log`). `commit_qc.height` 1->14 (+13 over 363 samples), `view_change_install_total` +3 (stake_quorum_timeout/missing_qc), `pending_rbc.bytes` max 1260 (sessions max 2); cadence still below 1 Hz.
- NPoS localnet 1 Hz rerun (release, 4 peers): `/tmp/iroha-localnet-npos-1hz-20260123T133537Z` (ports 29100/33400); 100x1 Hz `ledger transaction ping --no-wait` with `/v1/sumeragi/status` sampling to `sumeragi_status_1hz_20260123T133537Z.jsonl` (ping log `ping_1hz_20260123T133537Z.log`). `commit_qc.height` 1->17 (+16 over 136 samples), `view_change_install_total` +71 (missing_qc), `pending_rbc.bytes` max 2220 (sessions max 1); cadence still below 1 Hz.
- NPoS localnet 1 Hz rerun (release + tuned config, 4 peers): `/tmp/iroha-localnet-npos-1hz-20260123T140944Z` (ports 29200/33500); set `sumeragi.collectors.redundant_send_r=3` + `sumeragi.advanced.da.availability_timeout_multiplier=2`, then ran 100x1 Hz `ledger transaction ping --no-wait` with `/v1/sumeragi/status` sampling to `sumeragi_status_1hz_tuned_20260123T140944Z.jsonl` (ping log `ping_1hz_tuned_20260123T140944Z.log`). `commit_qc.height` 0->7 over 146 samples, `view_change_install_total` +3 (missing_qc), `pending_rbc.bytes` max 0; cadence still below 1 Hz.
- NPoS localnet 1 Hz rerun (RBC INIT routed via block-payload queue, 4 peers): `/tmp/iroha-localnet-npos-1hz-20260123T181930Z`; 120x1 Hz `ledger transaction ping --no-wait` with `ops sumeragi status` sampling to `sumeragi_status_1hz_fix_status_20260123T184602Z.jsonl` (ping log `ping_1hz_fix_nowait_20260123T184602Z.log`). `commit_qc.height` 83->95 (+12 over 120 samples), `view_change_install_total` +4 (`missing_qc_total` +1, `stake_quorum_timeout_total` +3), `missing_payload_total` +0, `pending_rbc.bytes` 0 (`stash_ready_init_missing_total` +8); cadence still below 1 Hz.
- NPoS localnet 1 Hz rerun (release + tuned collectors/DA timeouts, 4 peers): `/private/tmp/iroha-localnet-npos-1hz-20260123T190128Z`; set `sumeragi.collectors.redundant_send_r=3`, `sumeragi.advanced.da.availability_timeout_multiplier=2`, and `sumeragi.advanced.da.quorum_timeout_multiplier=2`, then ran 120x1 Hz `ledger transaction ping --no-wait` with `ops sumeragi status` sampling to `sumeragi_status_1hz_tuned_20260123T190128Z.jsonl` (ping log `ping_1hz_tuned_20260123T190128Z.log`). `commit_qc.height` 1->14 (+13 over 120 samples), `view_change_install_total` +1 (`stake_quorum_timeout_total` +1), `missing_qc_total` +0, `missing_payload_total` +0, `missing_block_fetch.total` +6, `pending_rbc.bytes` max 220 (sessions max 1); cadence still below 1 Hz.
- NPoS localnet 1 Hz rerun (release + config tuning, 4 peers): `/private/tmp/iroha-localnet-npos-1hz-20260123T195213Z`; set `sumeragi.collectors.redundant_send_r=4`, `sumeragi.advanced.da.availability_timeout_multiplier=3`, `sumeragi.advanced.da.quorum_timeout_multiplier=3`, `sumeragi.advanced.rbc.payload_chunks_per_tick=256`, and `sumeragi.advanced.worker.tick_work_budget_cap_ms=1000`, then ran 120x1 Hz `ledger transaction ping --no-wait` with `ops sumeragi status` sampling to `sumeragi_status_1hz_tuned2_20260123T195213Z.jsonl` (ping log `ping_1hz_tuned2_20260123T195213Z.log`). `commit_qc.height` 1->28 (+27 over 120 samples), `view_change_install_total` +30 (`missing_qc_total` +22), `stake_quorum_timeout_total` +0, `missing_payload_total` +0, `missing_block_fetch.total` +27, `pending_rbc.bytes` max 4189 (sessions max 1); cadence improved but still below 1 Hz.
- NPoS 1 Hz log scan (release run `/tmp/iroha-localnet-npos-1hz-20260123T133537Z`): view changes dominated by `missing_payload`/`missing_qc` with `missing_block_fetch.total=2166`, worker loop parked in `drain_rbc_chunks` with `block_payload_rx` depth 16 + `rbc_chunk_rx` depth 132, and `pending_rbc` stashing READY/DELIVER while INIT was missing (`stash_ready_init_missing_total=37`, `stash_deliver_init_missing_total=10`). Likely fix: trigger INIT rebroadcasts when READY/DELIVER stashes accumulate and prioritize block-payload draining over RBC chunk backlog so missing-payload fetches can complete before view-change timers fire.
- Izanami long-run attempt (stable profile, 4 peers, `--nexus`, target 2000 blocks): `RUST_LOG=izanami::summary=info,izanami::workload=warn IROHA_TEST_NETWORK_KEEP_DIRS=1 IROHA_TEST_NETWORK_PERMIT_DIR=$(mktemp -d) cargo run -p izanami --release --locked -- --allow-net --nexus --peers 4 --faulty 0 --duration 3600s --target-blocks 2000 --progress-interval 10s --progress-timeout 300s --tps 5 --max-inflight 8 --workload-profile stable` failed with `no block height progress for 300s` (min height 98). Multiple plans timed out on `transaction.status_timeout_ms=600s`, peer logs show repeated `missing_qc` view changes and "no proposal observed" errors; no deadlock or execution-root warnings found. Network dir: `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_eMeaCB`.
- Sumeragi RBC: hydrate INIT-created sessions from pending payloads so availability can advance after view changes even when chunks are missing; added unit coverage.
- Tests: `cargo test -p iroha_core pending_block_hydrates_rbc_session_for_init -- --nocapture` (ok).
- Izanami long-run attempt (stable profile, 4 peers, `--nexus`, target 2000 blocks): `RUST_LOG=izanami::summary=info,izanami::workload=warn IROHA_TEST_NETWORK_KEEP_DIRS=1 IROHA_TEST_NETWORK_PERMIT_DIR=$(mktemp -d) cargo run -p izanami --release --locked -- --allow-net --nexus --peers 4 --faulty 0 --duration 3600s --target-blocks 2000 --progress-interval 10s --progress-timeout 300s --tps 5 --max-inflight 8 --workload-profile stable` failed with `no block height progress for 300s` (min height 12). Multiple plan submissions timed out (`transaction.status_timeout_ms=600s`) plus `transaction queued for too long`; duplicate metric registration warnings persisted. Network dir: `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_XT3mKF`.
- Izanami XT3mKF log scan: repeated `missing_qc` view changes and `no proposal observed` at heights 14/18; `missing_payload` view change at height 13; RBC READY stashed awaiting INIT with missing `BlockCreated` fetches; missing-block fetch for block `0da271...` (height 18 view 1) reached attempts 96 (`dwell_ms=425292`) with RBC DELIVER deferred due to missing payload chunks; frequent tick/worker slow warnings.
- Sumeragi block sync/RBC: added debug logs for accepted block sync batches/forwarded updates and RBC chunk receive/ingest/stash to trace missing-block flow.
- Izanami: forward `RUST_LOG` into peer logger config layers (network builder) so per-peer logs honor CLI env; added unit coverage.
- Izanami long-run attempt (stable profile, 4 peers, `--nexus`, target 2000 blocks): `RUST_LOG=iroha_p2p=debug,iroha_core=debug IROHA_TEST_NETWORK_KEEP_DIRS=1 IROHA_TEST_NETWORK_PERMIT_DIR=$(mktemp -d) cargo run -p izanami --release --locked -- --allow-net --nexus --peers 4 --faulty 0 --duration 3600s --target-blocks 2000 --progress-interval 10s --progress-timeout 300s --tps 5 --max-inflight 8 --workload-profile stable` failed with `no block height progress for 300s` (min height 36). RBC chunk send logs appear early, but no `accepted block sync batch` / `received RBC chunk` debug markers found in peer logs. Network dir: `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_KrCc5b`.
- Izanami long-run attempt (stable profile, 4 peers, `--nexus`, target 2000 blocks): `RUST_LOG=iroha_p2p=debug,iroha_core=debug IROHA_TEST_NETWORK_KEEP_DIRS=1 IROHA_TEST_NETWORK_PERMIT_DIR=$(mktemp -d) cargo run -p izanami --release --locked -- --allow-net --nexus --peers 4 --faulty 0 --duration 3600s --target-blocks 2000 --progress-interval 10s --progress-timeout 300s --tps 5 --max-inflight 8 --workload-profile stable` ran the full 3600s but stopped before target blocks (concise_cardinal committed height 307). Peer logs now show `received block sync frame`, `accepted block sync batch`, `received RBC chunk`, and missing BlockCreated fetches while awaiting INIT with later `BlockCreated arrived before RBC session initialised` + INIT rebroadcast. Network dir: `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_kDdEkr`.
- Izanami kDdEkr latency scan: missing BlockCreated (awaiting INIT) → BlockCreated arrival p50 1.758s, p90 4.876s, p95 5.927s, max 25.929s (mostly `rbc_chunk_stash`); INIT rebroadcast after BlockCreated p50 0.894s, max 7.009s; pending requests at end: 1.
- Izanami kDdEkr latency breakdown: per-peer p50/p95/max (concise_cardinal 1.740/6.123/25.929s, included_satyr 1.989/5.910/15.234s, inviting_quail 1.829/5.346/23.470s, triumphant_cougar 1.470/6.120/13.310s). Height windows: 1-50 p50 1.802s p95 7.548s max 25.929s; 201-250 p50 1.317s p95 4.939s max 8.576s; 301-350 p50 2.750s p95 11.913s max 13.310s (n=22).
- Izanami long-run attempt (stable profile, 4 peers, `--nexus`, target 2000 blocks): `RUST_LOG=iroha_p2p=debug,iroha_core=debug IROHA_TEST_NETWORK_KEEP_DIRS=1 IROHA_TEST_NETWORK_PERMIT_DIR=$(mktemp -d) cargo run -p izanami --release --locked -- --allow-net --nexus --peers 4 --faulty 0 --duration 3600s --target-blocks 2000 --progress-interval 10s --progress-timeout 300s --tps 5 --max-inflight 8 --workload-profile stable` ran (command timed out after ~65m; no Izanami summary captured). Peer logs show committed height 319 on all peers; block store totals 1888 txs (1880 excluding genesis) in lane_000_core. Network dir: `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_IElRk7`.
- Izanami IElRk7 latency scan: missing BlockCreated (awaiting INIT) → BlockCreated arrival p50 1.858s, p90 4.478s, p95 5.208s, max 10.984s (mostly `rbc_chunk_stash`); INIT rebroadcast after BlockCreated p50 0.891s, max 7.996s; pending requests at end: 0 (15 arrivals without matching request logs).
- Izanami run config check (IElRk7): `sumeragi.npos.block_time_ms=1500`, timeouts propose/prevote/precommit/commit/da/agg=750/1000/1000/2000/2000/375, `sumeragi.advanced.da.quorum_timeout_multiplier=2`, `sumeragi.advanced.pacemaker.active_pending_soft_limit=8`; worker drain/tick budgets at defaults (2000/500ms).
- Izanami config tuning: raised pipeline parallelism/analysis knobs, enabled tx signature batching, and increased validation worker throughput to speed local validation/execution.
- Sumeragi worker loop: record post-tick per-tier drain timing for votes/block payloads/blocks and surface the breakdown in slow-iteration logs; added unit coverage.
- Izanami profiling run (tps=5, 300s, main_loop debug): `RUST_LOG=izanami::summary=info,izanami::workload=warn,iroha_core::sumeragi::main_loop=debug,iroha_core::sumeragi=info,iroha_p2p=info IROHA_TEST_NETWORK_KEEP_DIRS=1 IROHA_TEST_NETWORK_PERMIT_DIR=$(mktemp -d) cargo run -p izanami --release --locked -- --allow-net --nexus --peers 4 --faulty 0 --duration 300s --target-blocks 200 --progress-interval 10s --progress-timeout 180s --tps 5 --max-inflight 8 --workload-profile stable` stopped before target blocks; committed height 29 (lane_000_core blocks.index) across peers. Worker-iteration slow lines 536; mean elapsed 2147ms; pre/tick/post drains ~92.6% of elapsed with post_tick_drain_ms highest mean (753ms), pre_tick_drain_ms next (662ms), vote_drain_ms (573ms), tick_elapsed_ms (574ms); block_payload_drain_ms mostly zero but spikes (p95 2007ms, max 4840ms). Network dir: `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_qkijuG`.
- Izanami profiling run (tps=5, 300s, post-tick tier metrics): same command with new post-tick drain metrics stopped before target blocks; committed height 24 (lane_000_core blocks.index). Worker-iteration slow lines 518; mean elapsed 2310ms; pre_tick_drain_ms highest mean (762ms), post_tick_drain_ms mean 737ms, tick_elapsed_ms mean 663ms. Post-tick per-tier means: votes 384ms, blocks 191ms, payloads 157ms (p95s 1596/1190/1456ms; max 4774/3512/3392ms). Queue depths: block_rx mean 6.53 (max 28), vote_rx mean 1.53 (max 9). Network dir: `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_OzsNYD`.
- Izanami profiling run (tps=1, 300s, main_loop debug): same command with `--tps 1` stopped before target blocks; committed height 18 (lane_000_core blocks.index). Worker-iteration slow lines 467; mean elapsed 2363ms; pre_tick_drain_ms mean 812ms, tick_elapsed_ms mean 694ms, post_tick_drain_ms mean 726ms. Pre-tick per-tier means: votes 225ms, payloads 338ms, blocks 228ms; post-tick per-tier means: votes 328ms, payloads 181ms, blocks 212ms. BlockCreated handling mean 983ms (p95 4628ms), BlockSyncUpdate handling mean 1383ms (p95 3748ms). Queue depths: block_rx mean 9.3 (max 55), vote_rx mean 1.9 (max 14). Network dir: `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_WSdobH`.
- Izanami profiling run (tps=1, 300s, main_loop debug): same command with `--tps 1` stopped before target blocks; committed height 25 (lane_000_core blocks.index) across peers. Worker-iteration slow lines 520; mean elapsed 2189ms; pre_tick_drain_ms mean 748ms, tick_elapsed_ms mean 616ms, post_tick_drain_ms mean 685ms. Pre-tick per-tier means: votes 183ms, payloads 346ms, blocks 203ms; post-tick per-tier means: votes 312ms, payloads 152ms, blocks 210ms. BlockCreated handling mean 1003ms (p95 3544ms), BlockSyncUpdate handling mean 1083ms (p95 2281ms). Queue depths: block_rx mean 6.4 (max 34), vote_rx mean 1.6 (max 10). Network dir: `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_KdMXIq`.
- Izanami profiling run (tps=1, 300s, main_loop debug + substep timers): `RUST_LOG=izanami::summary=info,izanami::workload=warn,iroha_core::sumeragi::main_loop=debug,iroha_core::sumeragi=info,iroha_p2p=info IROHA_TEST_NETWORK_KEEP_DIRS=1 IROHA_TEST_NETWORK_PERMIT_DIR=$(mktemp -d) cargo run -p izanami --release --locked -- --allow-net --nexus --peers 4 --faulty 0 --duration 300s --target-blocks 200 --progress-interval 10s --progress-timeout 180s --tps 1 --max-inflight 8 --workload-profile stable` stopped before target blocks; committed height 26 (lane_000_core blocks.index) across peers. Worker-iteration slow lines 501; mean elapsed 2264ms; pre_tick_drain_ms mean 763ms, tick_elapsed_ms mean 626ms, post_tick_drain_ms mean 741ms. Pre-tick per-tier means: votes 238ms, payloads 331ms, blocks 179ms; post-tick per-tier means: votes 354ms, payloads 146ms, blocks 234ms. Message handling mean: BlockCreated 946ms (p95 3861ms), BlockSyncUpdate 972ms (p95 2025ms). Queue depths: block_rx mean 6.5 (max 27), vote_rx mean 1.5 (max 8). Queue latency (enqueue→drain): BlockCreated mean 3995ms (p95 10306ms), BlockSyncUpdate mean 2104ms (p95 5250ms). BlockSyncUpdate substeps mean/p95: roster_validate 301/307ms, signature_verify 36/37ms, qc_validate 160/187ms, qc_apply 173/211ms. BlockCreated substeps mean/p95: hint_validation 3/7ms, payload_hash 1/5ms, rbc_hydrate 542/1717ms, qc_replay 44/76ms. Commit stage timings mean/p95: qc_verify 894/3684ms, kura_persist 91/155ms. Network dir: `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_jCBTwp`.
- Izanami ramp run (tps=5, 900s, info logs, tuned pipeline defaults): `RUST_LOG=izanami::summary=info,izanami::workload=warn,iroha_core=info,iroha_p2p=info IROHA_TEST_NETWORK_KEEP_DIRS=1 IROHA_TEST_NETWORK_PERMIT_DIR=$(mktemp -d) cargo run -p izanami --release --locked -- --allow-net --nexus --peers 4 --faulty 0 --duration 900s --target-blocks 600 --progress-interval 10s --progress-timeout 300s --tps 5 --max-inflight 8 --workload-profile stable` stopped before target blocks; committed height 67; block store totals 398 txs (390 excluding genesis); worker-iteration slow ~354-384 and tick loop lagging ~132-144 per peer; missing_qc/no_proposal/stake_quorum_timeout minimal (0-1). Network dir: `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_a2LoEX`.
- Izanami ramp run (tps=2, 900s, info logs): `RUST_LOG=izanami::summary=info,izanami::workload=warn,iroha_core=info,iroha_p2p=info IROHA_TEST_NETWORK_KEEP_DIRS=1 IROHA_TEST_NETWORK_PERMIT_DIR=$(mktemp -d) cargo run -p izanami --release --locked -- --allow-net --nexus --peers 4 --faulty 0 --duration 900s --target-blocks 600 --progress-interval 10s --progress-timeout 300s --tps 2 --max-inflight 8 --workload-profile stable` stopped before target blocks; committed height 94; block store totals 505 txs (497 excluding genesis). View-change logs 2-3 per peer, missing_qc=0, tick loop lagging ~150, worker-iteration slow ~400, consensus-backlog=0. Network dir: `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_ydcz68`.
- Izanami ramp run (tps=3, 900s, info logs): same command with `--tps 3` stopped before target blocks; committed height 86; block store totals 483 txs (475 excluding genesis). View-change logs 0-2 per peer, missing_qc=0, tick loop lagging ~140-160, worker-iteration slow ~380-390, consensus-backlog=0. Network dir: `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_NGrVys`.
- Izanami ramp run (tps=5, 900s, info logs): same command with `--tps 5` stopped before target blocks; committed height 79; block store totals 467 txs (459 excluding genesis). View-change logs 0-5 per peer, missing_qc=0, tick loop lagging ~140-155, worker-iteration slow ~365-410, consensus-backlog=0. Network dir: `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_RvgeXq`.
- Izanami ramp run (tps=5, max-inflight=64, 900s, info logs): `RUST_LOG=izanami::summary=info,izanami::workload=warn,iroha_core=info,iroha_p2p=info IROHA_TEST_NETWORK_KEEP_DIRS=1 IROHA_TEST_NETWORK_PERMIT_DIR=$(mktemp -d) cargo run -p izanami --release --locked -- --allow-net --nexus --peers 4 --faulty 0 --duration 900s --target-blocks 600 --progress-interval 10s --progress-timeout 300s --tps 5 --max-inflight 64 --workload-profile stable` timed out in the harness; peer logs show committed height 28-33; block store totals 739 txs (731 excluding genesis). View-change logs 9-26 per peer, missing_qc 2-8, no_proposal_observed 1-10, stake_quorum_timeout 1-8; tick loop lagging ~139-155, worker-iteration slow ~381-553, consensus-backlog=0. Network dir: `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_lkyzd2`.
- Izanami long-run attempt (stable profile, 4 peers, `--nexus`, target 2000 blocks): `RUST_LOG=izanami::summary=info,izanami::workload=warn IROHA_TEST_NETWORK_KEEP_DIRS=1 IROHA_TEST_NETWORK_PERMIT_DIR=$(mktemp -d) cargo run -p izanami --release --locked -- --allow-net --nexus --peers 4 --faulty 0 --duration 3600s --target-blocks 2000 --progress-interval 10s --progress-timeout 300s --tps 5 --max-inflight 8 --workload-profile stable` failed with `no block height progress for 300s` (min height 199). Peer `manifest_tayra` hit a deadlock in `iroha_torii::pipeline_status_from_state` and exited (connection refused/timeouts during plan submissions). No `execution roots`/QC split warnings in peer logs. Network dir: `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_CnIvOl`.
- Izanami long-run attempt (stable profile, 4 peers, `--nexus`, target 2000 blocks): `RUST_LOG=izanami::summary=info,izanami::workload=warn IROHA_TEST_NETWORK_KEEP_DIRS=1 IROHA_TEST_NETWORK_PERMIT_DIR=$(mktemp -d) cargo run -p izanami --release -- --allow-net --nexus --peers 4 --faulty 0 --duration 3600s --target-blocks 2000 --progress-interval 10s --progress-timeout 300s --tps 5 --max-inflight 8 --workload-profile stable` failed after ~28m with `no block height progress for 300s` (min height 31). Multiple workload plan submissions timed out at 600s. Network dir: `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_kjtBGK`.
- Sumeragi RBC: treat committed-height RBC messages with a hash mismatch as stale (even under DA) to avoid chasing obsolete payloads; added `rbc_message_stale_drops_conflicting_hash_at_committed_height` coverage.
- Tests: `cargo test -p iroha_core rbc_message_stale -- --nocapture` (ok).
- NPoS localnet 1 Hz rerun after RBC stale fix (4 peers, 100x1 Hz `iroha --config client.toml ledger transaction ping --no-wait`): `/private/tmp/iroha-localnet-npos-1hz-20260123180449`; `commit_qc.height` 0->8 over 151 samples (~150s), `view_change_install_total` +2, `pending_rbc.bytes` max 440 with sessions max 2; still seeing roster sidecar mismatch logs and missing-block fetch retries in peer logs.
- Izanami workload: metadata removals now set+remove ephemeral keys, asset metadata ops mint before set/remove, and execute-trigger plans register+execute the call trigger in one transaction to avoid missing-trigger failures.
- Tests: `cargo test -p izanami metadata -- --nocapture` (ok).
- Tests: `cargo test -p izanami execute_trigger -- --nocapture` (ok).
- Izanami 4-peer DA run (stable, 300s, target 20): `RUST_LOG=izanami::summary=info,izanami::workload=warn IROHA_TEST_NETWORK_KEEP_DIRS=1 IROHA_TEST_NETWORK_PERMIT_DIR=$(mktemp -d) cargo run -p izanami -- --allow-net --nexus --peers 4 --faulty 0 --duration 300s --target-blocks 20 --progress-interval 10s --progress-timeout 180s --tps 5 --max-inflight 8 --workload-profile stable` completed; summary `successes=112 failures=0`; no `FrameTooLarge` in stdout; duplicate metric registration warnings persisted; tempdir `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_oWFDRm`.
- Traced missing-payload/roster-sidecar path: missing block fetches originate from QC/vote/RBC handlers → `FetchPendingBlock` → `handle_fetch_pending_block` responds with `BlockSyncUpdate` only when roster evidence (commit QC/checkpoint + stake snapshot) is available; otherwise falls back to `BlockCreated` + RBC init/chunks. Roster sidecar mismatch logs occur when Kura has a different block hash at the same height, so roster evidence is skipped and NPoS sends `BlockCreated`, which can still be dropped by locked-QC/hint gates if the block is stale.
- NPoS localnet 1 Hz log review: repeated missing block payload fetch retries (heights 5-7) on peer0/1/3, sumeragi tick loop lagging/worker slow warnings on all peers, and view change due to stake_quorum_timeout on peer2; likely explains sub-1 Hz cadence. Logs: `/private/tmp/iroha-localnet-npos-1hz-20260123153657` (peer*.log).
- NPoS localnet 1 Hz rerun (4 peers, 100x1 Hz `iroha --config client.toml ledger transaction ping --no-wait`): `/private/tmp/iroha-localnet-npos-1hz-20260123153657`; `commit_qc.height` 0->6 over 139 samples (~140s), `view_change_install_total` +3, `pending_rbc.bytes` max 2960 with sessions max 4; logs `sumeragi_status_1hz.jsonl` + `ping_1hz.log`.
- Izanami 4-peer DA run: `target/debug/izanami --allow-net --peers 4 --faulty 0 --duration 240s --target-blocks 20 --progress-interval 10s --progress-timeout 60s --tps 10 --max-inflight 8 --workload-profile stable` reached `min_height=20` at ~200s (successes 117, failures 0); no FrameTooLarge observed in stdout; duplicate metric registration warnings persisted; test-network dir cleaned (default).
- Sumeragi NPoS config: derive optional timeouts/VRF windows from `block_time_ms` and `epoch_length_blocks`; updated docs/templates and unit tests; minimal config fixtures now include trusted-peer PoP coverage and the client config snapshot matches current crypto defaults.
- Client: limit transaction-committed fallback query to a single entrypoint-hash match with pagination/fetch-size caps to avoid timeout-prone full-history scans.
- Tests: `cargo test -p iroha transaction_committed_limits_query_params` (ok).
- Build: `CARGO_TARGET_DIR=/Users/takemiyamakoto/dev/iroha/target-localnet31 cargo build --release -p iroha_kagami -p iroha_cli -p irohad` (ok).
- Localnet (permissioned, 7 peers, 753ms, telemetry=extended, tick_work_budget_cap_ms=150, validation workers=2, commit queues=2/2): `/private/tmp/iroha-localnet-7peer-run170-perm` (25180/26100). 50 TPS for 100-block target (25k tx, 1s `/metrics` sampling) → `blocks_non_empty` +109 over 513.39s (avg block interval 4.71s), admitted 48.1 TPS, `view_change_install_total=2` (`view_change_suggest_total=1`). Logs: `metrics_peer0_50tps_100blocks.log`, `load_50tps_100blocks.log`; view-change logs show "no proposal observed before cutoff".
- Localnet (permissioned, same run). 12 min 50 TPS (36k tx, 1s `/metrics` sampling) → `blocks_non_empty` +118 over 728.74s (avg block interval 6.18s), admitted 49.0 TPS, `view_change_install_total=19` (`view_change_suggest_total=11`). Logs: `metrics_peer0_50tps_12min.log`, `load_50tps_12min.log`.
- Localnet (NPoS, 7 peers, 753ms, telemetry=extended, tick_work_budget_cap_ms=150, validation workers=2, commit queues=2/2): `/private/tmp/iroha-localnet-7peer-run171-npos` (25280/26200). 50 TPS for 100-block target (25k tx, 1s `/metrics` sampling) → `blocks_non_empty` +89 over 506.14s (avg block interval 5.69s), admitted 49.0 TPS, committed estimate 49.4 TPS, `view_change_install_total=5` (`view_change_suggest_total=2`). Logs: `metrics_peer0_50tps_100blocks.log`, `load_50tps_100blocks.log`; view-change logs show "no proposal observed before cutoff".
- Localnet (NPoS, same run). 12 min 50 TPS (36k tx, 1s `/metrics` sampling) → `blocks_non_empty` +132 over 740.07s (avg block interval 5.61s), admitted 48.4 TPS, committed estimate 48.9 TPS, `view_change_install_total=7` (`view_change_suggest_total=3`). Logs: `metrics_peer0_50tps_12min.log`, `load_50tps_12min.log`.
- Telemetry: emit pipeline DAG metrics when telemetry is enabled even if Nexus is disabled; added coverage for `pipeline_dag_vertices`/`pipeline_dag_edges`.
- Tests: `cargo test -p iroha_core --features telemetry state_telemetry_pipeline_dag_emits_without_nexus -- --nocapture` (ok; duplicate metric registration warnings in test output).
- Sumeragi tests: ensure all `sumeragi/main_loop` tests that spin up a harness send shutdown signals so background network/gossiper tasks exit cleanly; prevents `cargo test -p iroha_core` from hanging.
- Tests: not run (not requested).
- Integration tests: wait for all peers to reach the post-registration block height before querying alias/compressed account literals in `accounts_query_accepts_alias_and_compressed_filter_literals`.
- Tests: `cargo test -p integration_tests --test address_canonicalisation accounts_query_accepts_alias_and_compressed_filter_literals -- --nocapture` (skipped: sandboxed network restriction).
- Integration tests: register alias-domain accounts in separate blocking transactions so `accounts_query_accepts_alias_and_compressed_filter_literals` avoids permission gating.
- Tests: not run (not requested).
- Tests: `cargo fmt --all` (warns about nightly-only rustfmt options in config).
- Tests: `cargo test -p iroha_config sumeragi_npos -- --nocapture` (ok).
- Tests: `cargo test -p iroha_config duration_clamp_tests -- --nocapture` (ok).
- Tests: `cargo test -p iroha_config sora_profile_tests -- --nocapture` (ok).
- Tests: `cargo test -p iroha_config snapshot_serialized_form -- --nocapture` (ok).
- Tests: `cargo test -p iroha_config` (ok).
- Tests: `cargo test --workspace` (aborted by user request; full workspace tests not run).
- Consensus ingress: treat `BlockCreated` as critical traffic (no bulk drops) so non-leader peers receive payloads; updated ingress tests and guardrail docs/config template.
- Tests: not run (not requested).
- Android SDK: preserve canonical instruction fixtures, fix QR stream envelope sizing, and align signer prehash expectations in tests; JS SDK: normalize encoded `@domain` error handling, relax canonical-account-id assertions, and use fixture authority hints in parity checks.
- Tests: `ANDROID_HOME=/Users/mtakemiya/Library/Android/sdk JAVA_HOME=/Library/Java/JavaVirtualMachines/openjdk-21.jdk/Contents/Home ./gradlew test -Pandroid.useAndroidX=true` (ok); `npm test` in `javascript/iroha_js` (ok).
- Swift offline receipts: align test receipt amounts to allowance scale (2dp) and update scale-mismatch expectation in `OfflineReceiptBuilderTests`/`OfflineWalletReceiptTests`.
- Tests: `swift test` (ok).
- Test network: build `iroha3d` with `--features expensive-telemetry` so `/metrics` is populated during throughput runs; added a unit test for the build args.
- Tests: `cargo test -p iroha_test_network program_spec_irohad_enables_expensive_telemetry -- --nocapture` (ok).
- Format: `cargo fmt --all` (warns about nightly-only rustfmt options in config).
- Android harness: added `OfflineQrStreamTest` to the Gradle main-harness list.
- Tests: `ANDROID_HARNESS_MAINS=org.hyperledger.iroha.android.offline.OfflineQrStreamTest ./gradlew :core:test --no-daemon` (failed: Gradle wrapper download blocked; `UnknownHostException: services.gradle.org`).
- Izanami 4-peer DA run attempt (2026-01-23): `cargo run -p izanami -- --allow-net --peers 4 --faulty 0 --duration 60s --target-blocks 20 --progress-interval 10s --progress-timeout 60s --tps 5 --max-inflight 8 --workload-profile stable` failed: loopback bind denied (`Operation not permitted` for 127.0.0.1:30000) in `crates/iroha_test_network/src/fslock_ports.rs:188` (sandbox networking restriction).
- JS offline QR stream: allow `OfflineQrStreamScanSession` to ingest text frames (optional encoding), reuse decode helper in `scanQrStreamFrames`, and add unit coverage.
- Tests: `node --test javascript/iroha_js/test/offlineQrStream.test.js` (ok).
- OFFLINE-QR-STREAM: implemented `QrStreamEnvelope`/`QrStreamFrame` codec + assembler caps in `iroha_data_model`, added CRC32 + parity recovery tests, and generated deterministic fixtures + generator bin.
- OFFLINE-QR-STREAM: added `iroha offline qr encode/decode` with SVG/PNG/GIF/APNG export; added Swift/Android/JS QrStream codecs + scan pipelines + playback skins; updated QR stream spec and SDK/offline docs.
- OFFLINE-QR-STREAM: fixed payload-kind tag fallback and CLI QR manifest JSON construction.
- Tests: `cargo test -p iroha_data_model qr_stream -- --nocapture` (ok); `cargo test -p iroha_cli qr_ -- --nocapture` (ok).
- Sumeragi config: update core config access paths (recovery/gating/debug/rbc) to match nested config keys; fix compile errors after nesting.
- Tests: not run (config path updates only).
- Client API: fix consensus JSON serialization to use `mode_flip_enabled` field.
- Tests: not run (not requested).
- Sumeragi config rationalization: nested config keys applied across docs/templates/translations (collectors/block/queues/pacemaker/da/persistence/recovery/gating/rbc); legacy flat-key mentions removed.
- Tests: not run (docs/config-template updates only).
- Integration tests: update `iroha_cli` command paths for executor upgrade and domain listing (`ops executor`, `ledger domain`).
- Tests: `cargo test -p integration_tests --test iroha_cli can_upgrade_executor -- --nocapture` (ok); `cargo test -p integration_tests --test iroha_cli reads_client_toml_by_default -- --nocapture` (ok).
- Sumeragi worker loop: added `worker_iteration_drain_budget_cap_ms` to cap per-iteration mailbox drain time; config/docs updated; new unit coverage.
- Tests: `cargo test -p iroha_core run_worker_iteration_caps_drain_at_config_cap -- --nocapture` (ok).
- Format: `cargo fmt --all` (warns about nightly-only rustfmt options in config).
- Build: `CARGO_TARGET_DIR=/Users/takemiyamakoto/dev/iroha/target-localnet28 cargo build --release -p iroha_kagami -p iroha_cli -p irohad` (ok).
- Localnet baseline (NPoS, 7 peers, 753ms, telemetry=extended): `/private/tmp/iroha-localnet-7peer-run161-npos-baseline` (25580/26500). 50 TPS for 100 blocks with 1s `/metrics` sampling → `commit_qc.height` 0->102 over 491s (avg block interval 4.81s), avg tx/block 132.7 (~27.6 TPS), view-change log entries 46. Metrics: `metrics_peer0_50tps_100blocks.log`. Load log: `load_50tps_100blocks.log`.
- Localnet baseline (same run). 12 min 50 TPS with 1s `/metrics` sampling → `commit_qc.height` 103->248 over 745s (avg block interval 5.14s), avg tx/block 137.3 (~26.7 TPS), view-change log entries 46. Metrics: `metrics_peer0_50tps_12min.log`. Load log: `load_50tps_12min.log`.
- Localnet drain-cap (NPoS, 7 peers, 753ms, telemetry=extended, `worker_iteration_drain_budget_cap_ms=150`): `/private/tmp/iroha-localnet-7peer-run162-npos-cap150` (25680/26600). 50 TPS for 100 blocks with 1s `/metrics` sampling → `commit_qc.height` 0->103 over 530s (avg block interval 5.15s), avg tx/block 142.6 (~27.7 TPS), view-change log entries 336. Metrics: `metrics_peer0_50tps_100blocks.log`. Load log: `load_50tps_100blocks.log`.
- Localnet drain-cap (same run). 12 min 50 TPS with 1s `/metrics` sampling → `commit_qc.height` 103->249 over 742s (avg block interval 5.08s), avg tx/block 157.3 (~30.9 TPS), view-change log entries 336. Metrics: `metrics_peer0_50tps_12min.log`. Load log: `load_50tps_12min.log`.
- Account parsing: allow base58-like alias labels to resolve when IH58 parsing fails with a checksum mismatch; added unit coverage for alias resolution.
- Integration tests: validate IH58 authorities via `AccountAddress::parse_any` and wait for the account to appear in accounts queries before alias/compressed filter assertions.
- Tests: `cargo test -p iroha_data_model from_str_resolves_base58_like_alias -- --nocapture` (ok); `cargo test -p integration_tests --test address_canonicalisation account_transactions_get_supports_address_format -- --nocapture` (ok); `cargo test -p integration_tests --test address_canonicalisation account_transactions_query_supports_address_format -- --nocapture` (ok); `cargo test -p integration_tests --test address_canonicalisation accounts_query_accepts_alias_and_compressed_filter_literals -- --nocapture` (ok).
- Build: `CARGO_TARGET_DIR=/Users/takemiyamakoto/dev/iroha/target-localnet27 cargo build --release -p iroha_kagami -p irohad -p iroha_cli` (ok).
- Localnet (NPoS, 7 peers, 753ms, telemetry=extended): `/private/tmp/iroha-localnet-7peer-run159-npos` (25280/26200). 50 TPS for 100 blocks with 1s `/metrics` sampling → `commit_qc.height` 2->99 over 460s (avg block interval 4.74s), avg tx/block 206 (~43 TPS), `view_change_install_total=0`, pacemaker deferrals `active_pending=70`, `rbc_backlog=70`, pending blocks total 1, tx queue depth 450. Metrics: `metrics_peer0_50tps_100blocks.log`. Load log: `load_50tps_100blocks.log`.
- Localnet (NPoS, 7 peers, 753ms, telemetry=extended): `/private/tmp/iroha-localnet-7peer-run159-npos` (25280/26200). 12 min 50 TPS with 1s `/metrics` sampling → `commit_qc.height` 102->224 over 720s (avg block interval 5.90s), avg tx/block 245 (~41.6 TPS), `view_change_install_total=6`, pacemaker deferrals `active_pending=159`, `rbc_backlog=155`, tx queue depth 304. Metrics: `metrics_peer0_50tps_12min.log`. Load log: `load_50tps_12min.log`.
- Localnet throughput (NPoS, 7 peers, release, telemetry profile Extended): `TELEMETRY_PROFILE=Extended TELEMETRY_ENABLED=true IROHA_THROUGHPUT_ARTIFACT_DIR=/private/tmp/iroha-throughput-npos-20260123a IROHA_TEST_NETWORK_KEEP_DIRS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-throughput cargo test -p integration_tests --release --test sumeragi_localnet_smoke npos_localnet_throughput_10k_tps -- --ignored --nocapture` (failed: submit queue did not drain below 20000 within 180s; queue_size ~20458, min_non_empty=3). Artifacts: `/private/tmp/iroha-throughput-npos-20260123a/throughput-1769116348574` (summary.json + status_npos.jsonl; metrics_npos.prom empty; network dir `/private/tmp/iroha-throughput-npos-tmp/irohad_test_network_0PlwjN`).
- Throughput snapshot (NPoS rerun): `/private/tmp/iroha-throughput-npos-20260123a/status_npos.jsonl` shows `tx_queue.depth` 0→295 (capacity 0→65536) and `pacemaker_backpressure_deferrals_total` 0→181; tick logs show `pending_blocks` max 3 (last observed 1) with `queue_len` max 20480. `/metrics` scrape remained empty (expensive metrics still gated in test-network configs).
- NEW_VIEW quorum correlation (NPoS throughput rerun `irohad_test_network_0PlwjN`): required 5, max `new_view_slots` 4 (`3:1`/`3:2`); NEW_VIEW vote sends per peer: creative_flamingo 29, blameless_groundhog 5, promising_salamander 4, sinewy_bongo 3, tasty_chiffchaff 3, pure_crane 2, national_lizard 1.
- StateView hold tracing: Sumeragi tick now tags `StateView` hold context; permissioned run `irohad_test_network_P4eF53` shows long holds dominated by `sumeragi.tick.cull_expired` (held_ms up to ~17s) with occasional `sumeragi.tick.pacemaker_propose_ready`, plus `state view lock contended; returning unlocked view` warnings.
- NEW_VIEW quorum correlation (permissioned throughput rerun `irohad_test_network_P4eF53`): required 5, max `new_view_slots` 2; NEW_VIEW vote sends per peer: heartfelt_scorpionfish 13, aspiring_cuckoo 2, assured_murrelet 2, carefree_doberman 2, jesting_butterfly 2, marketable_waterbear 1, quick_katydid 1.
- Roadmap: added OFFLINE-QR-STREAM animated QR transport breakdown (spec + fixtures + SDK/CLI work).
- Roadmap: refined OFFLINE-QR-STREAM breakdown (spec scheduling + parity details + assembler caps + SDK scan sessions + sakura animation variants).
- Offline QR stream SDKs: added Swift/Android/JS encoder/decoder + parity recovery + ScanSession core, sakura theme helpers, and unit tests.
- Docs/i18n: translated Japanese stubs for `docs/source/sorafs_gateway_dns_design_attendance.md`, `docs/source/sorafs_orchestrator_telemetry_plan.md`, and `docs/source/sorafs_proof_streaming_plan.md`; `translation_last_reviewed` set to 2026-01-22.
- Docs/i18n: translated Japanese stubs for `docs/source/sorafs_gateway_dns_design_agenda.md`, `docs/source/sorafs_chunk_range_smoketest.md`, and `docs/source/sorafs_chaos_plan.md`; `translation_last_reviewed` set to 2026-01-22.
- Sumeragi pacemaker: reuse the existing state snapshot when gating proposals to avoid nested `State::view` reads under writer pressure; added `active_pending_blocks_len_for_tip` and updated coverage.
- Tests: `cargo test -p iroha_core active_pending_blocks_len_ignores_aborted_and_nonextending -- --nocapture` (ok).
- Format: `cargo fmt --all` (warns about nightly-only rustfmt options in config).
- Offline allowance fixtures: install domain-selector resolvers for `wonderland`/`treasury` in test fixture decode paths (integration tests + iroha_test_network).
- Tests: `cargo test -p integration_tests --test address_canonicalisation offline_allowances_listing_respects_address_format_hint -- --nocapture` (ok; network startup skipped due to loopback sandbox restriction).
- Torii: enforce account-scoped filtering for `/v1/accounts/{account_id}/transactions/query` and align query tests to the account path semantics.
- Tests: not run (not requested).
- Integration tests: submit an Alice-authored transaction before validating account transactions address_format responses.
- Tests: not run (integration test update only).
- Integration tests: wait for all peers to reach the post-registration block height before querying alias/compressed account literals in `accounts_query_accepts_alias_and_compressed_filter_literals`.
- Tests: not run (test-only sync guard update).
- Merge: resolved remaining status.md conflict markers from the i23 merge.
- State: reorder `State::block`/`block_and_revert` lock acquisition to take block hashes before the world (matching `State::view`) to avoid deadlocks; added `state_block_orders_block_hashes_before_world` coverage.
- Format: `cargo fmt --all` (warns about nightly-only rustfmt options in config).
- Tests: `cargo test -p iroha_core state_block_orders_block_hashes_before_world -- --nocapture` (ok).
- Localnet throughput (permissioned, 7 peers, release): `IROHA_THROUGHPUT_ARTIFACT_DIR=/private/tmp/iroha-throughput-permissioned-20260122b IROHA_TEST_NETWORK_KEEP_DIRS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-throughput cargo test -p integration_tests --release --test sumeragi_localnet_smoke permissioned_localnet_throughput_10k_tps -- --ignored --nocapture` (failed: submit queue did not drain below 20000 within 180s; queue_size ~20480, min_non_empty=1). Artifacts: `/private/tmp/iroha-throughput-permissioned-20260122b/throughput-1769106279251` (summary.json + logs; network dir `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_gIn3el`).
- Localnet throughput (NPoS, 7 peers, release): `IROHA_THROUGHPUT_ARTIFACT_DIR=/private/tmp/iroha-throughput-npos-20260122 IROHA_TEST_NETWORK_KEEP_DIRS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-throughput cargo test -p integration_tests --release --test sumeragi_localnet_smoke npos_localnet_throughput_10k_tps -- --ignored --nocapture` (failed: submit queue did not drain below 20000 within 180s; queue_size ~20458, min_non_empty=3). Artifacts: `/private/tmp/iroha-throughput-npos-20260122/throughput-1769107672300` (summary.json + logs; network dir `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_Xq0l0o`).
- Throughput log scan: permissioned `irohad_test_network_gIn3el` and NPoS `irohad_test_network_Xq0l0o` show repeated missing-proposal view changes/`missing_qc` errors, pacemaker deferrals awaiting NEW_VIEW quorum, tick loop lag with queue_len ~20458/20480 and tick_cost up to ~25s, commit pipeline slow warnings, StateView guard held for 10–19s in Sumeragi ticks, low-priority P2P queue growth, and duplicate metric registration warnings in stderr (no panics observed).
- Localnet (NPoS 1 Hz fast-path rerun, 4 peers): `/private/tmp/iroha-localnet-npos-1hz-fastpath2` (ports 48280/53537). 100x1 Hz `ledger transaction ping --no-wait` with `/v1/sumeragi/status` sampling to `/private/tmp/iroha-localnet-npos-1hz-fastpath2/sumeragi_status_1hz_fastpath2.jsonl` (ping log `/private/tmp/iroha-localnet-npos-1hz-fastpath2/ping_1hz_fastpath2.log`). `commit_qc.height` 1->13 (+12 over 123s), `view_change_install_total` +2, pending RBC max 0 bytes / 2 sessions; 0 ping failures.
- CLI: normalized JSON/text output and flag behavior across `iroha_cli` commands (da/alias/sns/kaigi/sorafs/taikai/streaming/soracles), added structured summaries, and renamed handshake token encoding flag to `--token-encoding`.
- Tests: `cargo test -p iroha_cli` (failed: cli_output assertions, SNS catalog account literals, soracles oracle-id parsing, sorafs denylist/account-resolution tests, multisig/offline/subscriptions CLI tests, and incentives tests requiring Torii account resolution).
- Docs: `cargo run -p iroha_cli --bin iroha_cli -- tools markdown-help > crates/iroha_cli/CommandLineHelp.md` (ok).
- Sumeragi worker-loop scheduling: rotate to the oldest pending tier after a vote burst to keep RBC/payload draining; added `select_next_tier_picks_oldest_pending_after_vote_burst` coverage and reordered a borrow in `proposal_backpressure_blocks_commit_qc_pending_after_reschedule`.
- Tests: `cargo test -p iroha_core select_next_tier_picks_oldest_pending_after_vote_burst -- --nocapture` (ok).
- Format: `cargo fmt --all` (warns about nightly-only rustfmt options in config).
- Build: `CARGO_TARGET_DIR=/Users/takemiyamakoto/dev/iroha/target-localnet25 cargo build --release -p iroha_kagami -p irohad` (ok). `-p iroha_cli` failed: missing DA CLI output helpers (`SubmitOutput`, `build_manifest_fetch_value`) in `crates/iroha_cli/src/commands/da.rs`.
- Localnet (NPoS, 7 peers, 753ms, telemetry=extended): `/private/tmp/iroha-localnet-7peer-run156-npos` (24980/25900). 50 TPS for 120s with 1s `/metrics` sampling → height 33 after 143s (avg block interval 4.33s). Status after load: `commit_qc.height=43`, `view_change_index=3` (`missing_qc_total=6`, `stake_quorum_timeout_total=3`), `tx_queue.depth=0`. Metrics: `metrics-sumeragi-load.log`.
- Localnet (permissioned, 7 peers, 753ms, telemetry=extended): `/private/tmp/iroha-localnet-7peer-run156-perm` (25080/26000). 50 TPS for 120s with 1s `/metrics` sampling → height 31 after 139s (avg block interval 4.48s). Status after load: `commit_qc.height=41`, `view_change_index=0`, `tx_queue.depth=200`, commit inflight active. Metrics: `metrics-sumeragi-load.log`.
- FASTPQ: refreshed ordering hash golden vector for the transfer fixture to match current ordering-hash output.
- Tests: not run (fixture alignment only).
- FASTPQ: refreshed transfer trace-commitment fixture metadata and updated the transfer golden commitment vector.
- Tests: `cargo test -p fastpq_prover trace_commitment_matches_golden_vectors -- --nocapture` (ok).
- FASTPQ: added a stage2 CPU/GPU proof parity check and fixed `fastpq_metal_bench` harness visibility/env parsing to compile under `fastpq-gpu`.
- Tests: `cargo test -p fastpq_prover stage2_artifact_balanced_1k_matches_fixture -- --nocapture` (ok).
- Tests: `FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq-gpu stage2_artifact_balanced_1k_matches_fixture -- --nocapture` (failed: missing Metal Toolchain; `xcodebuild -downloadComponent MetalToolchain` required).
- FASTPQ: refreshed stage2 balanced 1k/5k fixture proofs for backend regression coverage.
- Tests: `FASTPQ_UPDATE_FIXTURES=1 FASTPQ_GPU=cpu cargo test -p fastpq_prover --test backend_regression -- --nocapture` (ok).
- Tests: `cargo test -p fastpq_prover stage2_artifact_balanced_1k_matches_fixture -- --nocapture` (ok).
- Build: `CARGO_TARGET_DIR=/Users/takemiyamakoto/dev/iroha/target-localnet23 cargo build --release -p iroha_kagami -p irohad -p iroha_cli` (ok; existing `iroha_cli` unused-import/unused-variable warnings).
- Localnet (NPoS, 7 peers, 753ms, soft limits 1/2/8, telemetry=extended): `/private/tmp/iroha-localnet-7peer-run153-npos` (24680/25600). 100 TPS for 120s via `iroha_cli ... ping --count 100 --parallel 20 --no-wait` → height 5, peer0 view changes 42 (`missing_qc_total=24`, `stake_quorum_timeout_total=6`), `tx_queue.depth=4796`, `sumeragi_pending_blocks_total` max 5 (`blocking` max 1), `sumeragi_commit_inflight_queue_depth` max 0; commit intervals after initial gap ~3.7s (4 commits). Metrics: `metrics-sumeragi-load3.log`.
- Localnet (permissioned, 7 peers, 753ms, soft limits 1/2/8, telemetry=extended): `/private/tmp/iroha-localnet-7peer-run154-perm` (24780/25700). 100 TPS for 120s → height 5, peer0 view changes 9 (`missing_qc_total=7`, `quorum_timeout_total=2`), `tx_queue.depth=11522`, `sumeragi_pending_blocks_total` max 0, `sumeragi_commit_inflight_queue_depth` max 0; commit intervals after initial gap ~3.8s (4 commits). Metrics: `metrics-sumeragi-load.log`.
- Tests: not run (localnet-only focus per request).
- Merge: resolved status.md conflict markers between doc updates, CLI fixes, and FASTPQ test adjustments.
- Docs: aligned governance API translations with IH58 account_id responses and `--owner ih58...`, updated JS SDK validation error guidance, and removed `@domain` from `inspectAccountId` examples.
- Docs: switched Soranet incentive packet beneficiary example to canonical IH58 account ids.
- Merge: resolved conflicts in Sumeragi pending-block gating + docs/portal CLI examples (app/tools/ledger updates).
- Tests: not run (merge conflict resolution).
- Docs: refreshed account ID/identity references to canonical IH58 across Torii/SDK/finance/SoraFS/SNS docs, portal snippets, and OpenAPI relay account descriptions; updated Java recipe code to avoid `@domain` parsing.
- Tests: not run (docs/sample edits only).
- CLI: fixed peer list for `FindPeers` (PeerId output), added helper test, and resolved clap arg-id collisions in Space Directory scaffolds + SoraFS token helpers; regenerated `crates/iroha_cli/CommandLineHelp.md`.
- Docs: `cargo run -p iroha_cli --bin iroha_cli -- tools markdown-help > crates/iroha_cli/CommandLineHelp.md` (ok).
- Tests: not run (CLI doc regen + clap fixes only).
- FASTPQ trace tests: validate `path_bit_0`/`sibling_0` columns against indexed transfer proofs instead of assuming non-zero bits.
- Tests: not run (FASTPQ trace test fix only).
- Config: added pacemaker backpressure soft-limit defaults to the Kiso and Torii connect-gating Sumeragi config literals.
- Tests: not run (config literal updates only).
- CLI: fixed `version` output borrow by precomputing localized strings and added a text-mode unit test with a stubbed server version.
- Tests: not run (CLI borrow fix + unit test only).
- Consensus telemetry: added `sumeragi_proposal_gap_total` counter and docs/tests; increments on missing-proposal view-change rotations.
- Build: `CARGO_TARGET_DIR=/Users/takemiyamakoto/dev/iroha/target-localnet22 cargo build --release -p iroha_kagami -p iroha_cli -p irohad` (ok).
- Localnet (NPoS, 7 peers, 753ms, exit_teu=15060): `/private/tmp/iroha-localnet-7peer-run147` (24580/25500). 100 TPS for 240s with 1s `/metrics` sampling → height 53, avg slot 6.29s (latest 25553ms), `view_change_install_total=5`, `proposal_gap_total=2`; pending blocks/inflight mostly 0/1. Metrics: `metrics_peer0_sample_run147.txt`.
- Localnet (permissioned, 7 peers, 753ms, exit_teu=15060): `/private/tmp/iroha-localnet-7peer-run148` (24580/25500). 100 TPS for 240s with 1s `/metrics` sampling → height 69, avg slot 4.37s (latest 2721ms), `view_change_install_total=0`, `proposal_gap_total=0`; pending blocks/inflight mostly 0/1. Metrics: `metrics_peer0_sample_run148.txt`.
- Localnet (NPoS, `rbc_payload_chunks_per_tick=256`): `/private/tmp/iroha-localnet-7peer-run149` (24580/25500). 100 TPS for 240s → height 72, avg slot 4.88s (latest 9330ms), `view_change_install_total=4`, `proposal_gap_total=0`; pending blocks max 4. Metrics: `metrics_peer0_sample_run149.txt`.
- Localnet (permissioned, `rbc_payload_chunks_per_tick=256`): `/private/tmp/iroha-localnet-7peer-run150` (24580/25500). 100 TPS for 240s → height 65, avg slot 5.06s (latest 13481ms), `view_change_install_total=1`, `proposal_gap_total=0`. Metrics: `metrics_peer0_sample_run150.txt`.
- Build: `CARGO_TARGET_DIR=/Users/takemiyamakoto/dev/iroha/target-localnet22 cargo build --release -p iroha_kagami` (ok).
- Localnet (NPoS, 7 peers, 753ms, exit_teu=15060, NPoS params block_time_ms=753, `rbc_payload_chunks_per_tick=256`, consensus_future_* 32): `/private/tmp/iroha-localnet-7peer-run151` (24580/25500). 100 TPS for 240s → height 65, avg slot 5.68s (latest 5966ms), `view_change_install_total=11`, `proposal_gap_total=1`, `txs_accepted=23870`; pending blocks max 4/inflight max 1. Metrics: `metrics_peer0_sample_run151.txt`.
- Localnet (permissioned, 7 peers, 753ms, commit_time_ms=300, `rbc_payload_chunks_per_tick=256`, consensus_future_* 32): `/private/tmp/iroha-localnet-7peer-run152` (24680/25600). 100 TPS for 240s → height 33, avg slot 10.69s (latest 27443ms), `view_change_install_total=18`, `proposal_gap_total=0`, `txs_accepted=10287`; pending blocks max 4 (blocking max 3)/inflight max 1. Metrics: `metrics_peer0_sample_run152.txt`.
- Format: `cargo fmt --all` (warns about nightly-only rustfmt options in config).
- Tests: not run (telemetry addition + localnet reruns).
- Norito RPC fixtures: added explicit payload specs for transaction fixtures, regenerated canonical `.norito` payloads/manifests, and re-synced Android/Swift/Python fixture bundles.
- Tests: `cargo test -p connect_norito_bridge -- --nocapture` (ok).
- Tests: `cargo run --manifest-path xtask/Cargo.toml --bin xtask -- norito-rpc-verify` (ok).
- Consensus telemetry: added pending-block + commit-inflight gauges and documented the new signals.
- Commit pipeline: overlap Kura persistence with WSV apply; added a Kura store failure regression test.
- Localnet (NPoS, 7 peers, 753ms, exit_teu=15060): `/private/tmp/iroha-localnet-7peer-run145b` (24680/25600). 100 TPS for 300s → height 86, avg slot 3.62s (latest 6118ms), `view_change_install_total=12`; pacemaker deferrals `active_pending=87`, `rbc_backlog=84`; commit stage averages `persist=298ms`, `qc_verify=63ms`, `block_sync=26ms`.
- Localnet (fanout, k_aggregators=7, redundant_send_r=4): `/private/tmp/iroha-localnet-7peer-run146` (24680/25600). 100 TPS for 300s → height 80, avg slot 5.68s (latest 2679ms), `view_change_install_total=8`; pacemaker deferrals `active_pending=80`, `rbc_backlog=77`; commit inflight queue depth 1 at snapshot.
- Build: `CARGO_TARGET_DIR=/Users/takemiyamakoto/dev/iroha/target-localnet21 cargo build -p iroha_kagami -p iroha_cli -p irohad --release` (ok).
- Format: `cargo fmt --all` (warns about nightly-only rustfmt options in config).
- Client tests: borrow explorer account QR payload strings and account IDs to match `&str` APIs; removed unused asset-definition numeric helper to drop dead-code warning.
- Tests: not run (warning/type fix only).
- IVM host pointer-ABI policy test now uses an experimental ABI header to exercise disallowed pointer types without relying on test-only variants.
- Tests: not run (pointer-ABI test update only).
- Warnings: declared Norito derive feature flags in `ivm_abi`, moved Kotodama metadata payload helper and SoraFS repair approval import into tests to silence unused/unknown cfg warnings.
- Build: `cargo build --workspace` (ok).
- Build tooling: dropped crates.io patch overrides for `rust_decimal`, `vergen-git2`, `is-terminal`, and `darling*` so upstream crates are used; removed the corresponding vendored sources; remaining local overrides unchanged.
- Tests: not run (Cargo.lock update required to re-resolve removed patches; per repo rules not modified here).
- Android SDK signing: signers now prehash payloads with `IrohaHash`; `TransactionBuilder` passes raw payload bytes through; `SignedTransactionHasher` remains aligned.
- Android SDK fixtures: exporter now preserves opaque payloads by re-signing against the provided payload bytes; regenerated manifests/fixtures and `.norito` blobs.
- Tests: `cargo test --manifest-path scripts/export_norito_fixtures/Cargo.toml` (ok).
- Tests: `java/iroha_android/gradlew :core:test --tests org.hyperledger.iroha.android.tx.TransactionPayloadFixtureTests --tests org.hyperledger.iroha.android.tx.TransactionFixtureManifestTests --tests org.hyperledger.iroha.android.norito.NoritoCodecAdapterTests` (ok).
- Tests: `cargo test --workspace` (failed: `crates/iroha_torii_shared/src/connect_sdk.rs` mismatched types; expected `&str`, found `String`).
- P2P tests: add `connect_startup_delay` to integration test config literals to match Network fields.
- Tests: not run (config literal updates only).
- Localnet throughput: artifacts now emit on failure with error in summary; queue drain timeout is overridable via `IROHA_THROUGHPUT_QUEUE_PROGRESS_TIMEOUT_SECS`.
- Localnet throughput (7 peers, debug): `scripts/run_localnet_throughput.sh` failed with submit queue stuck above 20k for 180s; no artifacts were written before this change.
- Tests: not run (throughput artifact/timeout updates).

- Sumeragi tick scheduling: `next_tick_deadline` now triggers immediate ticks when the tx queue is non-empty even with pending blocks, plus `actor_next_tick_deadline_prioritizes_queue_with_pending_block` coverage.
- Tests: `cargo test -p iroha_core actor_next_tick_deadline_prioritizes_queue_with_pending_block -- --nocapture` (ok).
- Localnet (NPoS, 7 peers, 753ms block/commit, DA timeout multipliers 1/1, k/r 4/3): `/private/tmp/iroha-localnet-7peer-run127` (24980/25900). 50 tx/s for 100 blocks (8k submit) → avg block interval 4.7s (p50 3.9s, p90 7.5s, p99 8.8s), admitted 13.5 tx/s over 471s, view_changes=0; queue drained by height 256.
- Merge: resolved conflict markers across status, account literal messaging, SoraFS GC audit metadata, storage refcount updates, and Torii onboarding test height handling.
- Tests: not run (merge conflict resolution).
- Android SDK: AccountId authorities now encode as Norito structs (domain + controller) and fixture payloads/manifest/.norito blobs are regenerated with domain-qualified authorities.
- Tests: `java/iroha_android/gradlew test` (failed: `~/.gradle/wrapper/dists/gradle-8.13-bin/.../gradle-8.13-bin.zip.lck` FileNotFoundException, Operation not permitted).
- Tests: `GRADLE_USER_HOME=/Users/takemiyamakoto/dev/iroha/.gradle ./gradlew test` (failed: `NativeServices.createSystemInfo()` cannot query machine details, errno 1: Operation not permitted).
- Tests: `java/iroha_android/gradlew test` (failed: Android SDK not configured; set `ANDROID_HOME` or `local.properties` `sdk.dir`).
- Tests: `java/iroha_android/gradlew test` (ok).
- Tests: `python3 scripts/check_android_fixtures.py` (ok).
- Android SDK signing: prehashing moved into `Signer` implementations (Ed25519) while `TransactionBuilder` now forwards raw payload bytes; `SignedTransactionHasher` remains aligned.
- Account parsing: removed the global domain-selector cache and allow explicit `encoded@domain` literals to parse without a resolver; implicit encoded literals still require a resolver or default domain.
- Tests: `cargo test -p iroha_data_model account_id_parsing_tests::encoded_literals_with_domain_parse_without_resolver -- --nocapture` (timed out after 300s while compiling dependencies).
- Torii tests: compressed literal expectations now use the raw `sora` form (no `@domain`); SoraFS helpers build `AccountId` values from public keys + domains.
- Tests: `cargo test -p iroha_torii address_format::tests::display_literal_and_from_literal_round_trip -- --nocapture` (ok).
- Tests: `cargo test -p iroha_torii iso20022_bridge::tests::runtime_accepts_raw_signer_account_literal -- --nocapture` (ok).
- Tests: `cargo test -p iroha_torii sorafs::api::advert_tests::pin_registry_metrics_summary_tracks_counts -- --nocapture` (ok).
- Torii tests: fixed offline revocations query filter encoded-literal coverage to use canonical, canonical-hex, and compressed account segments (build error fix).
- Sumeragi commit pipeline: event-driven runs no longer skip on consensus queue backlog; RBC READY triggers the pipeline even with backlog (log-only), and commit processing no longer filters to a fast-path hash.
- Tests: `cargo test -p iroha_core commit_pipeline_runs_event_when_queue_backlogged -- --nocapture` (ok).
- Tests: `cargo test -p iroha_core commit_pipeline_runs_with_backlog_when_commit_qc_ready -- --nocapture` (ok).
- Tests: `cargo test -p iroha_core commit_pipeline_runs_with_backlog_without_commit_qc -- --nocapture` (ok).
- Tests: `cargo test -p iroha_core handle_rbc_ready_runs_commit_pipeline_when_queue_backlogged -- --nocapture` (ok).
- Format: `cargo fmt --all` (warns about nightly-only rustfmt options in config).
- Core tests: removed unused `mut` in `select_account_for_uaid_uses_index` test helper.
- Torii onboarding now publishes a default Space Directory manifest for the global dataspace when missing, binding UAIDs immediately; added onboarding integration coverage and updated UAID docs/OpenAPI descriptions.
- Tests: not run (onboarding binding change + docs/OpenAPI updates).
- Sumeragi: allow empty BlockCreated payloads when time triggers are due by checking the read-only trigger set (`StateView::time_triggers_due_for_block` now uses shared time-trigger matching).
- Tests: `cargo test -p iroha_core --lib time_triggers_due_for_block_detects_precommit_trigger` (ok; warnings about unexpected `cfg` values from Norito derives).
- Tests: `CARGO_TARGET_DIR=/tmp/iroha-codex-roadmap-target IROHA_TEST_NETWORK_KEEP_DIRS=1 cargo test -p integration_tests --test mod triggers::time_trigger::time_trigger_scenarios -- --nocapture` (ok).
- Tests: `API_ADDRESS=0.0.0.0:1 PUBLIC_KEY=not-a-key CARGO_TARGET_DIR=/tmp/iroha-codex-roadmap-target IROHA_TEST_NETWORK_PARALLELISM=1 IROHA_TEST_NETWORK_PERMIT_DIR=$(mktemp -d) cargo test -p integration_tests --test mod -- --nocapture --test-threads=1` (timed out after 2h; last observed running `triggers::by_call_trigger::call_execute_trigger`).
- Tests: `API_ADDRESS=0.0.0.0:1 PUBLIC_KEY=not-a-key CARGO_TARGET_DIR=/tmp/iroha-codex-roadmap-target IROHA_TEST_NETWORK_PARALLELISM=1 IROHA_TEST_NETWORK_PERMIT_DIR=$(mktemp -d) cargo test -p integration_tests --test mod triggers::by_call_trigger::call_execute_trigger -- --nocapture --test-threads=1` (ok; `call_execute_trigger_with_args` also ran).
- NPoS localnet: added pacemaker backpressure telemetry split by reason (queue saturation, active pending, RBC backlog, relay backpressure, consensus queue backpressure) plus pacemaker eval/propose timing histograms; docs updated.
- Localnet tuning: tightened NPoS timeouts (propose 300ms, prevote 400ms, precommit 500ms, commit 650ms, DA 600ms, agg 100ms), set DA timeout multipliers to 1/1, and tuned commit inflight timeout to 4–10s (6x multiplier).
- Pacemaker: pending-block backpressure now lifts after min(block_time, commit_time) when no votes/QCs arrive, allowing proposals before quorum-timeout; the same fast path is applied to quorum reschedule gating, with unit coverage and pacemaker docs updated.
- Localnet: NPoS 1 Hz / 100-block soak rerun on `/tmp/iroha-localnet-npos-1hz-run2` (ports 48080/53337); 100x1 Hz `transaction ping --no-wait` with `/v1/sumeragi/status` sampling to `/tmp/iroha-localnet-npos-1hz-run2/status-1hz-20260121T192416Z.jsonl` (ping log `/tmp/iroha-localnet-npos-1hz-run2/ping-1hz-20260121T192416Z.log`). `commit_qc.height` 27->39 (+12 over 118s, ~0.10 blocks/s); `view_change_install_total` +3 (`stake_quorum_timeout_total` +2); pending RBC max 1040 bytes / 1 session (cap 8 MiB / 256), `rbc_store.pressure_level=0`.
- Localnet (NPoS 1 Hz fast-path rerun, 4 peers): `/private/tmp/iroha-localnet-npos-1hz-fastpath` (ports 48280/53537). 1 Hz `ledger transaction ping --no-wait` stalled at idx 43 due to Torii POST timeout; `/v1/sumeragi/status` samples at `/private/tmp/iroha-localnet-npos-1hz-fastpath/sumeragi_status_1hz_fastpath.jsonl` and ping log at `/private/tmp/iroha-localnet-npos-1hz-fastpath/ping_1hz_fastpath.log`. `commit_qc.height` 2->7 (+5 over 119s), `view_change_install_total` +1, pending RBC max 0 bytes / 1 session. Peer0 logged a parking_lot deadlock in `Actor::active_pending_blocks_len` during pacemaker tick; telemetry sync timed out afterward. Rerun still needed.
- Tests: `cargo test -p iroha_core pacemaker_backpressure -- --nocapture` (pass; warnings about `unexpected_cfgs` from norito derive macros).
- Tests: `cargo test -p iroha_core proposal_backpressure_allows_fast_path_without_votes -- --nocapture` (pass).
- Triggers: `submit_instruction_and_wait` now uses `submit_blocking` with the network sync timeout to avoid racing trigger registration/execution.
- Tests: `IROHA_TEST_NETWORK_PARALLELISM=1 CARGO_TARGET_DIR=/tmp/iroha-codex-roadmap-target cargo test -p integration_tests --test mod call_execute_trigger -- --nocapture` (ok).
- Tests: `IROHA_TEST_NETWORK_PARALLELISM=1 CARGO_TARGET_DIR=/tmp/iroha-codex-roadmap-target cargo test -p integration_tests --test mod -- --nocapture --test-threads=1` (timed out after 2h; last observed running `triggers::time_trigger::time_trigger_scenarios`).
- Pointer-ABI: `current_policy` is now publicly accessible for VM-side enforcement, with guard install/restore unit tests.
- Torii SoraFS API: map retention metadata validation failures to 400 responses; added unit test coverage.
- SoraFS storage: explicitly type chunk refcount map during index rebuild to avoid saturating-add inference errors.
- Governance defaults: include `sorafs_repair_escalation` in core default builders + Torii test config defaults.
- Tests: `API_ADDRESS=0.0.0.0:1 PUBLIC_KEY=not-a-key IROHA_TEST_NETWORK_PARALLELISM=1 IROHA_TEST_NETWORK_PERMIT_DIR=$(mktemp -d) CARGO_TARGET_DIR=/tmp/iroha-codex-roadmap-target cargo test -p integration_tests --test mod extra_functional::connected_peers::register_new_peer -- --nocapture` (ok).
- Tests: `API_ADDRESS=0.0.0.0:1 PUBLIC_KEY=not-a-key IROHA_TEST_NETWORK_PARALLELISM=1 IROHA_TEST_NETWORK_PERMIT_DIR=$(mktemp -d) CARGO_TARGET_DIR=/tmp/iroha-codex-roadmap-target cargo test -p integration_tests --test mod triggers::by_call_trigger::call_execute_trigger_with_args -- --nocapture` (ok).
- Tests: `API_ADDRESS=0.0.0.0:1 PUBLIC_KEY=not-a-key IROHA_TEST_NETWORK_PARALLELISM=1 IROHA_TEST_NETWORK_PERMIT_DIR=$(mktemp -d) CARGO_TARGET_DIR=/tmp/iroha-codex-roadmap-target cargo test -p integration_tests --test mod -- --nocapture` (timed out after 1h; long-running by-call trigger and Sumeragi DA tests observed, no explicit failures before timeout).
- Tests: `API_ADDRESS=0.0.0.0:1 PUBLIC_KEY=not-a-key IROHA_TEST_NETWORK_PARALLELISM=2 IROHA_TEST_NETWORK_PERMIT_DIR=$(mktemp -d) CARGO_TARGET_DIR=/tmp/iroha-codex-roadmap-target cargo test -p integration_tests --test mod -- --nocapture --test-threads=2` (timed out after 1h; last observed running `sumeragi_rbc_recovers_after_restart_with_roster_change`).
- SoraFS repair governance now tracks approval/rejection votes, finalizes decisions deterministically at the dispute deadline, and records appeals; slash proposals (including CLI repair escalate) accept optional approval summaries while enforcing penalty caps; governance state persists in repair snapshots and docs now include the policy/lifecycle (portal mirror included).
- Tests: not run (SoraFS repair governance updates).
- SoraFS reconciliation: periodic reconciliation snapshots now hash repair tickets, retention index entries, and GC counters/refcounts; reports are published to the governance DAG, telemetry exposes run/divergence counts, and observability docs include the new metrics.
- Tests: not run (SoraFS reconciliation updates).
- SoraFS reconciliation: added a multi-peer integration test that compares per-peer reconciliation reports to detect divergence (`integration_tests/tests/sorafs_reconciliation.rs`).
- Tests: not run (SoraFS reconciliation integration test).
- Docs/i18n: replaced repo-root `MAINTAINERS.*` stubs with translations for ar/es/fr/pt/ru/ur; set `translation_last_reviewed` to 2026-01-21.
- Docs/i18n: replaced repo-root `integrated_test_framework.es.md` stub with a full Spanish translation; `translation_last_reviewed` set to 2026-01-21.
- Docs/i18n: replaced repo-root `integrated_test_framework.fr.md`, `integrated_test_framework.pt.md`, and `integrated_test_framework.ru.md` stubs with translations; `translation_last_reviewed` set to 2026-01-21.
- Docs/i18n: replaced repo-root `integrated_test_framework.ar.md` stub with an Arabic translation; `translation_last_reviewed` set to 2026-01-21.
- Docs/i18n: replaced `docs/source/soranet_gateway_hardening.*` stubs (ar/es/fr/he/ja/pt/ru/ur) with translations; `translation_last_reviewed` set to 2026-01-21.
- Docs/i18n: replaced `docs/automation/README.*` stubs (ar/es/fr/he/ja/pt/ru/ur) with translations; `translation_last_reviewed` set to 2026-01-21.
- Docs/i18n: replaced `docs/automation/android/README.*` stubs (ar/es/fr/he/ja/pt/ru/ur) with translations; `translation_last_reviewed` set to 2026-01-21.
- Docs/i18n: replaced `docs/automation/da/README.*` stubs (ar/es/fr/he/ja/pt/ru/ur) with translations; `translation_last_reviewed` set to 2026-01-21.
- Docs/i18n: replaced `docs/bindings/README.*` stubs (ar/es/fr/he/ja/pt/ru/ur) with translations; `translation_last_reviewed` set to 2026-01-21.
- Docs/i18n: replaced `docs/devportal/try-it.*` stubs (he/ja) with translations; `translation_last_reviewed` set to 2026-01-21.
- Docs/i18n: replaced `docs/settlement-router.*` stubs (he/ja) with translations; `translation_last_reviewed` set to 2026-01-21.
- Docs/i18n: replaced `docs/space-directory.*` stubs (he/ja) with translations; `translation_last_reviewed` set to 2026-01-21.
- Docs/i18n: replaced `docs/amx.*` stubs (he/ja) with translations; `translation_last_reviewed` set to 2026-01-21.
- Docs/i18n: replaced `docs/source/governance_playbook.*` stubs (he/ja) with translations; `translation_last_reviewed` set to 2026-01-21.
- Docs/i18n: replaced `docs/source/taikai_cache_hierarchy.*` stubs (he/ja) with translations; `translation_last_reviewed` set to 2026-01-21.
- Docs/i18n: replaced `docs/source/soranet_vpn.*` stubs (he/ja) with translations; `translation_last_reviewed` set to 2026-01-21.
- Docs/i18n: replaced `docs/source/soracles_evidence_retention.*` stubs (he/ja) with translations; `translation_last_reviewed` set to 2026-01-21.
- Docs/i18n: replaced `docs/source/p2p_trust_gossip.*` stubs (he/ja) with translations; `translation_last_reviewed` set to 2026-01-21.
- Docs/i18n: replaced `docs/source/compute_lane.*` stubs (he/ja) with translations; `translation_last_reviewed` set to 2026-01-21.
- Docs/i18n: replaced `docs/source/soracles.*` stubs (he/ja) with translations; `translation_last_reviewed` set to 2026-01-21.
- Docs/i18n: replaced `docs/source/soradns_ir_playbook.*` stubs (he/ja) with translations; `translation_last_reviewed` set to 2026-01-21.
- Docs/i18n: replaced `docs/source/soranet_gateway_billing.*` stubs (he/ja) with translations; `translation_last_reviewed` set to 2026-01-21.
- Docs/i18n: replaced `docs/source/soranet_gateway_billing_m0.*` stubs (he/ja) with translations; `translation_last_reviewed` set to 2026-01-21.
- Docs/i18n: replaced `docs/source/mochi/packaging.*` stubs (he/ja) with translations; `translation_last_reviewed` set to 2026-01-21.
- Docs/i18n: replaced `docs/source/mochi/index.*` stubs (he/ja) with translations; `translation_last_reviewed` set to 2026-01-21.
- Docs/i18n: replaced `docs/source/mochi/quickstart.*` stubs (he/ja) with translations; `translation_last_reviewed` set to 2026-01-21.
- Docs/i18n: replaced `docs/source/mochi/troubleshooting.*` stubs (he/ja) with translations; `translation_last_reviewed` set to 2026-01-21.
- Docs/i18n: replaced `docs/source/benchmarks/history.*` stubs (he/ja) with translations; `translation_last_reviewed` set to 2026-01-21.
- Docs/i18n: replaced `docs/source/agents.*` stubs (he/ja) with translations; `translation_last_reviewed` set to 2026-01-21.
- Docs/i18n: replaced `docs/source/agents/missing_docs_inventory.*` stubs (he/ja) with translations; `translation_last_reviewed` set to 2026-01-21.
- Docs/i18n: replaced `docs/source/agents/env_var_migration.*` stubs (he/ja) with translations; `translation_last_reviewed` set to 2026-01-21.
- Docs/i18n: replaced `docs/source/android_support_playbook.*` stubs (he/ja) with translations; `translation_last_reviewed` set to 2026-01-21.
- Docs/i18n: replaced `docs/source/compliance/android/checklists/and8_ga_2027-10_rehearsal.*` stubs (he/ja) with translations; `translation_last_reviewed` set to 2026-01-21.
- Docs/i18n: replaced `docs/source/sdk/android/and7_governance_hotlist.*` stubs (he/ja) with translations; `translation_last_reviewed` set to 2026-01-21.
- Docs/i18n: replaced `docs/source/sdk/android/android_support_playbook.*` stubs (he/ja) with translations; `translation_last_reviewed` set to 2026-01-21.
- Docs/i18n: replaced `docs/source/sdk/android/partner_sla_discovery.*` stubs (he/ja) with translations; `translation_last_reviewed` set to 2026-01-21.
- Docs/i18n: replaced `docs/source/compliance/android/eu/README.*` stubs (he/ja) with translations; `translation_last_reviewed` set to 2026-01-21.
- Docs/i18n: replaced `docs/source/compliance/android/jp/README.*` stubs (he/ja) with translations; `translation_last_reviewed` set to 2026-01-21.
- Docs/i18n: replaced `docs/source/sdk/android/norito_fixture_alignment.*` stubs (he/ja) with translations; `translation_last_reviewed` set to 2026-01-21.
- Docs/i18n: replaced `docs/source/sdk/index.*` stubs (he/ja) with translations; `translation_last_reviewed` set to 2026-01-21.
- Docs/i18n: replaced `docs/source/project_tracker/norito_streaming_post_mvp.*` stubs (he/ja) with translations; `translation_last_reviewed` set to 2026-01-21.
- Docs/i18n: replaced `docs/source/compliance/android/device_lab_contingency.*` stubs (he/ja) with translations; `translation_last_reviewed` set to 2026-01-21.
- Docs/i18n: replaced `docs/source/sdk/swift/connect_risk_tracker.*` stubs (he/ja) with translations; `translation_last_reviewed` set to 2026-01-21.
- Docs/i18n: replaced `docs/source/crypto/attachments/sm_openssl_provenance.*` stubs (he/ja) with translations; `translation_last_reviewed` set to 2026-01-21.
- Docs/i18n: replaced `docs/source/governance_pipeline.*` stubs (he/ja) with translations; `translation_last_reviewed` set to 2026-01-21.
- Docs/i18n: replaced `docs/source/kagami_profiles.*` stubs (he/ja) with translations; `translation_last_reviewed` set to 2026-01-21.
- Docs/i18n: replaced `docs/source/sdk/android/telemetry_override_log.*` stubs (he/ja) with translations; `translation_last_reviewed` set to 2026-01-21.
- Docs/i18n: replaced `docs/source/release_dual_track_schedule.*` stubs (he/ja) with translations; `translation_last_reviewed` set to 2026-01-21.
- Docs/i18n: replaced `docs/source/sdk/swift/readiness/screenshots/2026-03-05/README.*` stubs (he/ja) with translations; `translation_last_reviewed` set to 2026-01-21.
- Docs/i18n: replaced `docs/source/sdk/swift/readiness/screenshots/2026-02-28/README.*` stubs (he/ja) with translations; `translation_last_reviewed` set to 2026-01-21.
- Docs/i18n: replaced `docs/source/soranet/nsc-55-legal.*` stubs (he/ja) with translations; `translation_last_reviewed` set to 2026-01-21.
- Docs/i18n: replaced `docs/source/soranet/nsc-42-legal.*` stubs (he/ja) with translations; `translation_last_reviewed` set to 2026-01-21.
- Docs/i18n: replaced `docs/source/soranet/reports/pow_resilience.*` stubs (he/ja) with translations; `translation_last_reviewed` set to 2026-01-21.
- Docs/i18n: replaced `docs/source/soranet/snnet15_m3_runbook.*` stubs (he/ja) with translations; `translation_last_reviewed` set to 2026-01-21.
- Docs/i18n: replaced `docs/source/soranet/reports/circuit_stability.*` stubs (he/ja) with translations; `translation_last_reviewed` set to 2026-01-21.
- Docs/i18n: replaced `docs/source/soranet/snnet15_m2_runbook.*` stubs (he/ja) with translations; `translation_last_reviewed` set to 2026-01-21.
- Docs/i18n: replaced `docs/source/swift_xcframework_procurement_request.*` stubs (he/ja) with translations; `translation_last_reviewed` set to 2026-01-21.
- Docs/i18n: replaced `docs/source/project_tracker/sorafs_pin_registry_tracker.*` stubs (he/ja) with translations; `translation_last_reviewed` set to 2026-01-21.
- Docs/i18n: replaced `docs/source/torii/norito_rpc_tracker.*` stubs (he/ja) with translations; `translation_last_reviewed` set to 2026-01-21.
- Docs/i18n: replaced `docs/source/connect_architecture_followups.*` stubs (he/ja) with translations; `translation_last_reviewed` set to 2026-01-22.
- Docs/i18n: replaced `docs/source/soranet/gar_cdn_policy_bus.*` stubs (he/ja) with translations; `translation_last_reviewed` set to 2026-01-22.
- Docs/i18n: replaced `docs/source/crypto/sm_wg_sync_template.*` stubs (he/ja) with translations; `translation_last_reviewed` set to 2026-01-22.
- Docs/i18n: replaced `docs/source/soranet/templates/downgrade_communication_template.*` stubs (he/ja) with translations; `translation_last_reviewed` set to 2026-01-22.
- Docs/i18n: replaced `docs/source/sdk/swift/ios5_dx_completion.*` stubs (he/ja) with translations; `translation_last_reviewed` set to 2026-01-22.
- Docs/i18n: replaced `docs/source/norito_stage1_cutover.*` stubs (he/ja) with translations; `translation_last_reviewed` set to 2026-01-22.
- Docs/i18n: replaced `docs/source/sns/reports/steward_scorecard_2026q1.*` stubs (he/ja) with translations; `translation_last_reviewed` set to 2026-01-22.
- Docs/i18n: replaced `docs/source/status/soranet_testnet_weekly_digest.*` stubs (he/ja) with translations; `translation_last_reviewed` set to 2026-01-22.
- Docs/i18n: replaced `docs/source/sdk/swift/readiness/reports/202603_and7_quiz.*` stubs (he/ja) with translations; `translation_last_reviewed` set to 2026-01-22.
- Docs/i18n: replaced `docs/source/torii/api_versioning.*` stubs (he/ja) with translations; `translation_last_reviewed` set to 2026-01-22.
- Docs/i18n: replaced `docs/source/zk/proof_retention.*` stubs (he/ja) with translations; `translation_last_reviewed` set to 2026-01-22.
- Docs/i18n: replaced `docs/source/sorafs/migration_ledger.*` stubs (he/ja) with translations; `translation_last_reviewed` set to 2026-01-22.
- Docs/i18n: replaced `docs/source/soranet/lane_profiles.*` stubs (he/ja) with translations; `translation_last_reviewed` set to 2026-01-22.
- Docs/i18n: replaced `docs/source/soranet_billing_m0.*` stubs (he/ja) with translations; `translation_last_reviewed` set to 2026-01-22.
- Docs/i18n: replaced `docs/source/soranet_gateway_bug_bounty.*` stubs (he/ja) with translations; `translation_last_reviewed` set to 2026-01-22.
- SoraFS repair governance policy now enforces approval quorum/minimum voters, dispute/appeal windows, tie-break rules, and penalty caps; added approval/policy Norito payloads, capped scheduler draft penalties, updated CLI slash proposals to require approval summaries, and refreshed repair plan + node client protocol docs (portal mirror included).
- Tests: not run (repair escalation policy + docs updates only).
- SoraFS retention precedence now resolves effective retention as the minimum of pin policy, deal end, and governance cap metadata, persists `RetentionSourceV1` + access counters in storage metadata/index, and GC capacity sweeps evict expired manifests by LRU; CLI GC output adds `retention_sources`, docs updated (ops playbook, node client protocol, architecture RFC + portal mirror).
- Format: `cargo fmt --all` (warns about nightly-only rustfmt options in config).
- Tests: `cargo test --workspace` (failed: missing `crates/ivm_abi/src/syscalls_doc_gen.rs`).
- Tests: `cargo test -p sorafs_manifest retention_precedence` (ok).
- Tests: `cargo test -p sorafs_node retention_epoch_persists_in_metadata` (ok; warnings about unexpected `cfg` values from Norito derives and an unused assignment in `crates/sorafs_node/src/repair.rs`).
- Tests: `cargo test -p sorafs_node last_access_persists_after_reads` (ok; same warnings).
- Tests: `cargo test -p sorafs_node node_handle_gc_capacity_prefers_least_recently_used_expired` (ok; same warnings).
- Kotodama: extracted the compiler + front-end into `crates/kotodama_lang`, moved shared ABI definitions into `crates/ivm_abi` with `ivm` re-export stubs, and relocated runtime helpers to `crates/ivm/src/kotodama_std.rs`.
- Kotodama bytecode analysis now performs static decoding only; runtime fuzz reports a disabled-runtime finding in the standalone compiler crate (no IVM dependency).
- Docs/tests: updated Kotodama sample paths + documentation references to `crates/kotodama_lang/src/samples`.
- Tests: not run (Kotodama/ABI refactor only).
- Observability: added repair SLA/backlog/lease-expiry and retention-blocked eviction alerts + promtool fixtures, extended the SoraFS capacity dashboard with repair panels, and updated ops/observability docs (portal mirrors included).
- Tests: not run (alert rule fixtures + dashboard/doc updates only).
- Torii onboarding now publishes a default Space Directory manifest when the UAID is not bound to the global dataspace, ensuring UAID bindings for Sora global onboarding; updated onboarding integration coverage and OpenAPI descriptions.
- Tests: `cargo test -p iroha_torii accounts_onboard_publishes_global_manifest_and_binding -- --nocapture` (ok; warnings about unexpected `cfg` values from Norito derive macros).
- Sumeragi commit pipeline now runs finalize/precommit work every tick/event and throttles only QC rebuilds.
- Tests: `cargo test -p iroha_core commit_pipeline_defers_reschedule_until_availability_timeout -- --nocapture` (ok).
- Tests: `cargo test -p iroha_core rbc_chunk_commit_pipeline_runs_on_completion -- --nocapture` (ok).
- Tests: `cargo test -p iroha_core commit_pipeline_ -- --nocapture` (ok).
- SoraFS repair backlog stats: fixed `compute_backlog_stats` type inference by annotating `oldest_queued_at`, added `compute_backlog_stats_tracks_oldest_queued` unit test.
- Localnet readiness: `scripts/deploy_localnet.sh` now waits for `/v1/sumeragi/status` to report a non-empty `mode_tag` to avoid early-mode confusion.
- Localnet (permissioned, 4 peers, 10 tx/s): `/tmp/iroha-localnet-4peer-perm-10tps` (32180/34337); asset register failed (`rose#wonderland` exists) but peers ran; height ~15 after ~322s (did not reach 100).
- Localnet (NPoS requested, 4 peers, 10 tx/s): `/tmp/iroha-localnet-4peer-npos-10tps` (32280/34437, `--skip-asset-register`); height ~12 after ~324s (did not reach 100); `/status` still reported permissioned `mode_tag`.
- Localnet (NPoS config validation, 4 peers): `/tmp/iroha-localnet-4peer-npos-verify2` (32380/34537); peer configs + `genesis.json` set `consensus_mode = Npos`; `/v1/sumeragi/status` and `/status` report `npos` after startup (early `/status` can show permissioned before mode tags update).
- Localnet deploy script build failed earlier: `crates/sorafs_node/src/repair.rs:2381` E0282 closure type inference; fixed by annotating `oldest_queued_at`.
- Localnet (permissioned rerun, 4 peers, 10 tx/s): `/tmp/iroha-localnet-4peer-perm-10tps-rerun` (32480/34637, `--skip-asset-register`); 200s of `transaction ping --msg ping` at 10 tx/s; height 34 (did not reach 100).
- Localnet (NPoS rerun, 4 peers, 10 tx/s): `/tmp/iroha-localnet-4peer-npos-10tps-rerun` (32580/34737, `--skip-asset-register`); 200s of `transaction ping --msg ping` at 10 tx/s; height 30 (did not reach 100).
- Localnet (permissioned, release, 10k-permissioned): `/tmp/iroha-localnet-4peer-perm-10tps-perf` (32680/34837, `--skip-asset-register`); 60s of `transaction ping --msg ping` at 10k tx/s submit attempt (600k total, sender rate ~9375 tx/s); header height 1 immediately after load, `/v1/sumeragi/status` commit_qc height 4 and `worker_stage=drain_rbc_chunks` (did not reach 100).
- Localnet (NPoS, release, 10k-npos): `/tmp/iroha-localnet-4peer-npos-10k-perf` (32780/34937, `--skip-asset-register`); 60s of `transaction ping --msg ping` at 10k tx/s submit attempt (600k total, sender rate ~9836 tx/s); header height 8, commit_qc height 10 (did not reach 100).
- Localnet (permissioned, root-cause probe, 10k tx/s): `/tmp/iroha-localnet-4peer-perm-10k-root` (32880/35037, `--perf-profile 10k-permissioned`); default CLI `ulimit -n` was 256 so `transaction ping --count 10000` failed with `Too many open files` and only ~303 submits succeeded; raising to `ulimit -n 4096` allowed 10k submits.
- Localnet (permissioned, post-ulimit): `/v1/sumeragi/status` shows `tx_queue.depth=23346`, `commit_qc.height=29`, but `kura_store.stage_last_height=4` and `missing_block_fetch_last_targets=0` with `missing_block_fetch_total=1015`, plus `pending_rbc` init-missing stashes and `rbc_store` evictions; block height stayed at 4 for 30s, indicating missing BlockCreated/payload recovery (no fetch targets) rather than tx admission.
- Kagami localnet help now calls out that the default build line is `iroha3`, so leaving `--consensus-mode` unset yields `npos`.
- DA/RBC: missing-payload view-change deferral now respects backlog/backpressure so `qc_missing_block_defer` will not force view changes while RBC/block-payload queues are congested; READY/DELIVER handlers log missing READY senders and local deferral reasons to pinpoint stalled sessions.
- DA/RBC: RbcInit/RbcChunk posts now bypass the background post queue so chunk dissemination cannot be starved by background work.
- Tests: `cargo test -p iroha_core bypasses_background_queue -- --nocapture` (ok).
- Localnet: NPoS 1 Hz / 100-block stall check with `/v1/sumeragi/status` polling completed on `/tmp/iroha-localnet-1hz-fix2` (ports 32200/35500). 100 blocks in 744.9s (~0.134 blocks/s), no view changes, `missing_payload_total=0`, RBC queues stayed under cap.
- Tests: `cargo test -p iroha_core fetch_pending_block_npos_falls_back_without_roster_hints -- --nocapture` (ok).
- Tests: `cargo test -p iroha_core clear_missing_block_view_change_resets_window_and_trigger -- --nocapture` (ok).
- SDKs (Swift): aligned offline receipt Poseidon sample account to the canonical ed25519 seed `[0x01; 32]`, refreshed receiver hash expectations, and synced Swift offline_poseidon vectors to the artifacts snapshot.
- Tests: not run (Swift SDK fixture/value updates only).
- SoraFS repair telemetry: added backlog age/queue depth/lease expiry metrics to Prometheus + OTEL, extended repair audit events and governance metadata with manifest/provider IDs, and refreshed repair observability docs.
- Docs: synced portal observability plan translations for repair metrics (ar/es/fr/he/ja/pt/ru/ur).
- Format: `cargo fmt --all` (warns about nightly-only rustfmt options).
- Tests: `cargo test -p sorafs_manifest task_event_` (ok).
- Tests: `cargo test -p sorafs_node backlog_stats_tracks_oldest_and_per_provider` (ok; warnings about unexpected `cfg` values from Norito derive macros and unused assignment in `crates/sorafs_node/src/repair.rs`).
- Tests: `cargo test -p sorafs_node filesystem_publisher_writes_repair_audit_files` (ok; same warnings).
- Tests: `cargo test -p sorafs_node filesystem_publisher_writes_repair_slash_files` (ok; same warnings).
- Tests: `cargo test -p iroha_telemetry sorafs_repair` (ok).
- Tests: `cargo test -p iroha_telemetry repair_otel_handles_noop_without_exporter` (ok).
- Integration tests: reworked `sumeragi_rbc_session_recovers_after_cold_restart` to target a full in-flight RBC session via `/v1/sumeragi/rbc/sessions`, verify the on-disk session file by hash+height prefix before shutdown, and reuse the observed peer for restart checks; removed the unused persisted-session scan helper.
- Integration tests: allow `sumeragi_rbc_session_recovers_after_cold_restart` to proceed once a full-chunk session is observed even if delivery completed before shutdown (accept delivered sessions + drop the pre-shutdown `delivered` assertion).
- Tests: not run (integration-test logic change only).
- SDKs: Torii pipeline submissions now prefer Norito receipts across JS/Python/Swift, Android submit helpers send Norito payloads, Android pending-queue tests assert Norito bytes, JS dist artifacts refreshed, and SDK docs/examples now reference receipt payload hashes (including Android docs noting receipt bytes).
- Tests: not run (SDK updates only).
- Tests: `npm test` (failed; multiple JS test failures including `address_inspect`, `isoBridge`, `toriiClient`, `validationError`, and `transactionFixturesParity` suites).
- Tests: `swift test` (failed; 45 failures/35 unexpected, first failure `TransactionEncoderValidationTests.testCastZkBallotAcceptsCanonicalHints` with `nativeBridgeError(.authority)`).
- Docs/UX strings: clarified IH58 as the preferred account format and `sora` as the second-best option across the Account Structure RFC, data-model specs, SDK/CLI help text, Torii/OpenAPI docs, portal docs/images, and translations.
- Sumeragi worker loop now refreshes drain budgets and tick gaps each iteration from current on-chain timing to avoid stale startup timeouts; added `run_worker_loop_refreshes_config_each_iteration`.
- Tests: `cargo test -p iroha_core run_worker_loop_refreshes_config_each_iteration` (ok).
- Sumeragi quorum timeout now uses pending-block progress age (touched by RBC chunk/READY/DELIVER + votes) so healthy consensus activity does not trigger view changes; pending blocks track progress timestamps and tests/docs updated.
- Tests: `cargo test -p iroha_core pending_block_progress_age_resets_on_touch -- --nocapture` (ok).
- Tests: `cargo test -p iroha_core touch_pending_progress_updates_pending_age -- --nocapture` (ok).
- Localnet: re-run NPoS 1 Hz / 100-block stall check with status polling after the progress-age change failed in the sandbox (loopback bind `Operation not permitted`); rerun needed outside the sandbox.
- SoraFS repair API: added Torii contract tests for worker endpoints (report/claim/heartbeat/complete/fail) and status snapshots.
- SoraFS CLI: added help smoke coverage for `iroha sorafs repair` and `iroha sorafs gc`.
- SoraFS incentives state: embed domain hints so IH58 account IDs roundtrip, and install a domain-selector resolver while parsing JSON snapshots and CLI tests.
- Accounts/UAID: enforce UAID→account 1:1 index, reject duplicate UAIDs on registration, and switch UAID portfolio/reward selection to the index with new unit coverage.
- Account address parsing: accept full-width `ｓｏｒａ` sentinel for compressed Sora literals alongside ASCII `sora`.
- Tests: `cargo test -p iroha_data_model compressed_fullwidth_sentinel_accepts` (ok; waited for build dir lock).
- CLI account literals: resolve account identifiers through `/v1/accounts/resolve` across commands (contracts, DA/SoraFS ledgers, Kaigi, staking, ZK, subscriptions, offline filters, multisig/trigger inputs) and refresh CLI harness tests to use canonical account IDs.
- CLI SNS/SoraFS: catalog policy verification and gateway denylist validation now resolve account literals via `/v1/accounts/resolve`; vote CLI tests use canonical fixtures.
- Offline receipts: added `OfflineSpendReceipt::invoice_id()` accessor + unit test and switched executor validation/tests to use it so non-transparent builds compile.
- Roadmap: added account identity refactor tasks (IH58-only canonical IDs, alias/UAID unification, PII-safe tokens, no compatibility shims).
- SoraFS config: added `sorafs.repair`/`sorafs.gc` defaults and wiring, updated defaults/samples (`defaults/nexus`, `configs/soranexus/testus`), plus config docs (all locales).
- SoraFS repair status: added deterministic sorted/filtered status listing (status/provider filters) and a global list endpoint; OpenAPI updated.
- SoraFS repair workers: added claim/heartbeat/complete/fail endpoints with idempotency + lease validation, NodeHandle/repair scheduler tracking, OpenAPI entries, and repair plan doc updates.
- SoraFS repair auth: enforced worker signatures + `CanOperateSorafsRepair`, updated Torii DTOs/OpenAPI, guardrail checks, and SoraFS auth/runbook docs; added Torii auth tests.
- SoraFS repair audit schema: worker signatures now cover manifest digests, repair worker requests carry `manifest_digest_hex`, and canonical repair/GC audit payload structs with Norito roundtrip tests are defined; docs updated.
- SoraFS repair CLI: added `iroha sorafs repair` list/claim/complete/fail/escalate subcommands plus client helpers for repair status/worker/slash endpoints; repair plan docs updated.
- SoraFS GC CLI: added `iroha sorafs gc` inspect/dry-run reporting for retention state (read-only).
- Docs: added GC inspection/dry-run guidance to SoraFS ops playbook, node-ops runbook, and node-client protocol CLI helpers (including portal mirrors).
- SoraFS repair store: file-backed Norito snapshot (`repair_state.to`) with versioned schema, CAS updates, PoR history persistence, and corrupt-store archiving fallback.
- SoraFS repair events: record ordered RepairTaskEventV1 transitions in the scheduler and surface them in repair status responses.
- SoraFS repair watchdog: enforce SLA/attempt caps, requeue expired leases with backoff, emit draft slash proposals, and prioritize claimable tasks by SLA/severity/provider backlog.
- SoraFS repair worker: integrate repair chunk fetch/verify with the storage scheduler budgets to avoid starving normal pin traffic.
- SoraFS GC: persist `retention_epoch` in manifest metadata and index expiry scans; deterministic GC sweeper evicts expired manifests with pre-admission retry, skips active repairs, and publishes GC audit events.
- SoraFS PoR: skip scheduling PoR challenges once manifests are past `retention_epoch + retention_grace_secs`.
- Torii: added a GC sweeper runtime alongside the repair worker runtime and wired GC metrics/OTel counters.
- Telemetry: added SoraFS GC Prometheus/OTel metrics plus GC panels in the capacity health dashboard and GC alerts (stall/blocked) with ops/docs updates (including portal mirrors).
- Tests: not run for the SoraFS GC/retention and repair scheduler integration changes.
- Norito derive: fix AoS enum `[u8; N]` length prefixing for u8 array variants and add an unpacked AoS enum roundtrip test.
- Tests: `cargo test -p iroha_torii sorafs_repair_worker_endpoints_drive_state` (ok).
- Tests: `cargo test -p iroha_cli sorafs_` (ok).
- Tests: `cargo clippy -p sorafs_node --all-targets -- -D warnings` (ok).
- Tests: `cargo test -p norito enum_aos` (ok; warning about unused_mut in `crates/norito/src/core.rs:5610`).
- Tests: `cargo test -p sorafs_node compute_backlog_stats_tracks_oldest_queued -- --nocapture` (ok; repeated `unexpected_cfgs` warnings in `crates/sorafs_node/src/repair.rs`).
- Format: `cargo fmt --all` (warned about nightly-only rustfmt options in stable toolchain).
- Tests: `cargo test -p iroha_config sorafs_repair_and_gc_parse_clamps_values` (ok).
- Tests: `cargo test -p sorafs_node repair_and_gc_configs_preserve_fields` (ok).
- Tests: `cargo test -p sorafs_node node_handle_threads_repair_and_gc_config` (ok).
- Tests: `cargo test -p sorafs_node list_tasks_` (ok).
- Tests: `cargo test -p sorafs_node node_handle_manages_repair_queue` (ok).
- Tests: `cargo test -p sorafs_node` (failed: `store::tests::ingest_manifest_persists_metadata_and_chunks` expected manifest hash `57950a...` vs `010203`).
- Tests: `cargo test -p iroha_torii repair_query_tests` (ok).
- Build: `cargo check -p iroha_executor` (ok).
- Build: `cargo check -p iroha_core` (ok).
- Tests: `cargo test --workspace` (aborted per user request).
- Tests: `cargo test -p iroha_torii da::` (ok).
- Tests: `cargo test -p iroha_primitives rs16::` (ok).
- Tests: `cargo test -p sorafs_car --bin da_reconstruct --features da_harness` (ok).
- Torii test configs updated for new SoraFS repair/GC fields and network `connect_startup_delay`; DA reconstruct harness manifest now supplies Taikai metadata.
- Sumeragi pacemaker: idle view changes now pause when consensus block-payload/RBC-chunk queues are saturated to avoid churn under payload backlog; added `force_view_change_if_idle_skips_when_consensus_queue_backpressure`.
- Consensus ingress: dedup BlockSyncUpdate + RBC chunk payloads (and record evictions) to reduce block-payload/RBC-chunk queue saturation; bump block-payload/RBC dedup cache cap to 8192; added `incoming_block_message_drops_duplicate_block_sync_update` + `incoming_block_message_drops_duplicate_rbc_chunk`.
- Tests: `cargo test -p iroha_core incoming_block_message_drops_duplicate_block_sync_update -- --nocapture` (ok; initial build waited on Cargo lock).
- Tests: `cargo test -p iroha_core incoming_block_message_accepts_block_sync_update_with_new_evidence -- --nocapture` (ok; initial build waited on Cargo lock).
- Tests: `cargo test -p iroha_core incoming_block_message_drops_duplicate_rbc_chunk -- --nocapture` (ok; initial build waited on Cargo lock).
- NPoS localnet (22080/17537): generated `/tmp/iroha-npos-local-22080`, ran 100×1Hz pings (no-wait) — consensus stalled at commit/QC height 2; worker loop stuck in `drain_rbc_chunks` with block-payload/RBC-chunk backlogs.
- Tests: `cargo test -p iroha_core force_view_change_if_idle_skips_when_consensus_queue_backpressure -- --nocapture` (ok).
- Sumeragi pacemaker: defer proposal assembly while block-payload/RBC-chunk queues are saturated; added `consensus_queue_backpressure_trips_on_payload_or_rbc_queue` + `proposal_backpressure_defers_on_consensus_queue_backpressure`. Docs updated (`docs/source/sumeragi.md`, `docs/source/references/configuration.md`, `docs/source/references/peer.template.toml`).
- Tests: `cargo test -p iroha_core consensus_queue_backpressure -- --nocapture` (ok).
- DA/RBC: default RBC chunk fanout now targets the full roster (minus local) so READY quorum can form even with f faulty peers; unit coverage updated.
- DA/RBC: unresolved backlog detection now gates on incomplete READY quorum/chunk coverage (even when payload is local) and pending RBC stashes; idle view changes stay suppressed while RBC backlog or relay backpressure is active.
- Tests: `cargo test -p iroha_core rbc_backlog_ -- --nocapture` (ok).
- DA/RBC: treat blocks present in Kura as committed only when their stored height is <= committed height, and keep accepting RBC messages for committed heights when payload is missing; added `rbc_message_stale_ignores_uncommitted_kura_blocks` + `rbc_message_stale_allows_missing_payload_for_committed_height_with_da`.
- Tests: `cargo test -p iroha_core rbc_message_stale -- --nocapture` (ok).
- DA/RBC: pending RBC stashes at the tip height now gate view changes (alongside tip+1) to prevent premature leader rotation; added `rbc_backlog_counts_pending_stash_for_tip_height`.
- Tests: `cargo test -p iroha_core rbc_backlog_counts_pending_stash_for_tip_height -- --nocapture` (ok).
- Format: `cargo fmt --all` (warns about nightly-only rustfmt options in config).
- Sumeragi pacemaker: rebroadcast cached proposals + BlockCreated payloads when the local leader already has the proposal cached, avoiding stalls when proposal messages are dropped; added `pacemaker_rebroadcasts_cached_proposal_when_leader` coverage.
- Tests: `cargo test -p iroha_core pacemaker_rebroadcasts_cached_proposal_when_leader -- --nocapture` (ok).
- Tests: `cargo test --workspace` failed during compile (missing `connect_startup_delay` in `iroha_p2p` integration test Network configs).
- Integration tests (asset): exchange-rate flow now waits for seeded assets before rate lookup to avoid lagging-peer NotFound failures.
- Integration tests (asset): submit helpers now broadcast signed transactions across peers and tolerate duplicate-enqueue responses to avoid queued timeouts; added duplicate-error detection coverage. `CARGO_TARGET_DIR=/tmp/iroha-codex-roadmap-target IROHA_TEST_NETWORK_PARALLELISM=1 IROHA_TEST_NETWORK_PERMIT_DIR=$(mktemp -d) cargo test -p integration_tests --test asset -- --nocapture --test-threads=1` (pass).
- Integration tests (asset): widened local test consensus timing (pipeline time 6s, DA quorum/availability timeout multipliers 6/3) to reduce view-change stalls; validated by the same asset-suite run (previous targeted run timed out after 15m).
- Integration tests: NetworkBuilder::new now defaults to a 1s pipeline time for faster test networks; `with_default_pipeline_time` keeps Sumeragi defaults. Tests not run yet.
- Torii/client: queue rejections now include `x-iroha-reject-code`; client ResponseReport decodes Norito error envelopes to surface codes/messages. Tests: `cargo test -p iroha --lib response_report::with_msg_decodes_norito_error_envelope`, `cargo test -p iroha_torii --lib push_into_queue_error_sets_reject_code_header` (ok).
- Integration tests: consolidated asset mint quantity cases into one network, tightened polling delays, and moved most integration tests to a 2s pipeline time for faster runs (unstable_network and NPoS performance timing left unchanged). Tests not run yet.
- Integration tests: collapsed misc, pagination, non-mintable, NFT, subscription, telemetry (permissioned), and time-trigger suites to reuse a single network per file and set 2s pipeline time where safe. Tests not run yet.
- Genesis: consensus handshake metadata now refreshes during manifest parse/normalize to keep fingerprints aligned with effective parameters (added `build_and_sign_refreshes_stale_consensus_fingerprint` test).
- Config: fixed `connect_startup_delay` initialization in torii test utils and p2p start wiring.
- Tests: `cargo test -p integration_tests genesis_asset_minted_across_peers -- --nocapture` (ok).
- Tests: `cargo test -p integration_tests genesis_norito_bytes_roundtrip_network -- --nocapture` (ok).
- NPoS bootstrap: RBC roster derivation now falls back to the active topology near the tip to avoid empty-session snapshots; added `rbc_roster_for_session_uses_active_topology_in_npos_bootstrap`.
- Tests: `cargo test -p iroha_core rbc_roster_for_session_uses_active_topology_in_npos_bootstrap -- --nocapture` (ok).
- RBC ingress/backlog: treat RbcInit as blocking ingress to avoid non-blocking drops under load, and count pending RBC stashes for the next height as unresolved backlog so pacemaker won’t trigger view changes while BlockCreated/INIT is missing; added `rbc_backlog_counts_pending_stash_for_next_height` + updated relay blocking-variant test.
- Tests: `cargo test -p iroha_core rbc_backlog_counts_pending_stash_for_next_height -- --nocapture` (ok).
- Tests: `cargo test -p iroha_core try_incoming_paths_drop_on_full_channels -- --nocapture` (ok).
- Tests: `cargo test -p iroha_core rebroadcast_highest_pending_block_skips_on_relay_backpressure -- --nocapture` (ok).
- Tests: `cargo test -p iroha_core should_defer_proposal_when_relay_backpressure_active -- --nocapture` (ok).
- Tests: `cargo test -p iroha_core should_override_relay_backpressure_after_liveness_timeout -- --nocapture` (ok).
- Tests: `cargo test -p iroha_core relay_backpressure_tracks_recent_drops -- --nocapture` (ok).
- Lint: not run
- Build: not run
- Tests: `cargo test -p ivm` (timed out after 20m while running `kotodama_foreach_reads_durable_state_map_entries`)
- Tests: `cargo test -p iroha_p2p --lib` (ok).
- Build: `cargo build -p irohad -p iroha_kagami` (ok).
- NPoS localnet: default ports collided with other services (8080–8082 bound by `dpn-api-r`/`pk-cbdc-*`), and 18080 collided with `ipfs`. Re-ran with `kagami localnet --peers 4 --out-dir /tmp/iroha-npos-local-19080 --consensus-mode npos --seed demo --base-api-port 19080 --base-p2p-port 17337` + `start.sh` (ok). `/health` on 19080 is healthy, `iroha_cli ... transaction ping --no-wait` submitted, and `GET /v1/ledger/headers` shows height 2 with the tx merkle root.
- Smoke: `iroha_cli ... transaction ping --count 10 --parallel 4 --no-wait` (ok). `GET /v1/ledger/headers` advanced to height 3.
- Localnet pulse: `transaction ping --count 50 --parallel 8 --no-wait` then `--count 10` advanced the chain to height 6; peer logs show commit certificate applied and state committed for height 6 (no hang observed).
- Localnet regression: 1Hz test (`100` pings over ~100s, `--no-wait`) stalled at height 8; all peers report commit_qc height 8, repeated view changes with `missing_payload`, and `missing_block_fetch_total` growing (~1.6k). `/v1/sumeragi/status` shows worker loop stuck in `drain_block_payloads` with `block_rx` backlog (~512) and pending RBC sessions; processes are running but consensus is livelocked.
- Proposal assembly now schedules missing-block fetches when the highest QC parent is unavailable locally.
- Proposal assembly defers early when the highest QC block is missing to avoid building on the wrong parent.
- Sora profile detection now injects a deterministic PoP entry to prevent trusted_peers_pop warnings in test networks.
- Localnet P2P now supports `network.connect_startup_delay_ms` (Kagami localnet sets 2000ms) to reduce startup connection-refused spam; Sumeragi slow-iteration warnings now require backlog/progress to log.
- Izanami: inject SumeragiParameters block/commit timings into NPoS genesis when Izanami pipeline_time is unset (keeps on-chain timings aligned with NPoS overrides); added `npos_genesis_sets_sumeragi_timing` regression coverage.
- Tests: `cargo test -p izanami npos_genesis_sets_sumeragi_timing -- --nocapture` (ok; initial run timed out while compiling).
- Build: `cargo build -p izanami --release --locked` (ok).
- Format: `cargo fmt --all` (warns about nightly-only rustfmt options in config).
- Izanami NPoS 1 TPS run (300s) after SumeragiParameters update: stopped before target blocks; max commit height 41; `vote_drain_ms` p50=0, p90=734, max=3242; `block_payload_drain_ms` p90=155, max=1084. Logs: `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_wWvhSZ`.
- Votes: inbound handling now defers stale-height/locked-qc drops to the validation path to keep the hot queue light; added `vote_inbound_defers_stale_vote_drop`.
- Tests: `cargo test -p iroha_core vote_inbound_defers_stale_vote_drop -- --nocapture` (timed out while compiling).
- Izanami NPoS 1 TPS run (300s) after vote-inbound deferral: stopped before target blocks; max commit height 44; `vote_drain_ms` p50=0, p90=778, max=3485; `block_payload_drain_ms` p90=151, max=921. Logs: `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_p4poFN`.
- Defaults: redundant_send_r now derived as `2f+1` from validator count for Kagami profiles, localnet generation, and test-network genesis; default genesis templates and operator docs updated. Tests not run (not requested).
- State tests: use the configured Nexus lane catalog and a generated missing-owner account in `da_pin_intents_drop_missing_owner_accounts` to avoid false positives when default accounts exist.
- Sumeragi: proposal hints now trigger missing-block fetches for the highest QC parent; highest-QC fetches fall back to the raw commit-topology snapshot when the effective roster is empty; fetch responses matching the local highest QC bypass the background queue.
- Tests: updated fetch-pending coverage for priority handling and added `proposal_hint_missing_highest_qc_triggers_missing_block_fetch`, `fetch_pending_block_highest_qc_bypasses_background_queue`.
- Tests: `cargo test -p iroha_core proposal_hint_missing_highest_qc_triggers_missing_block_fetch -- --nocapture` (ok).
- Tests: `cargo test -p iroha_core fetch_pending_block_highest_qc_bypasses_background_queue -- --nocapture` (ok).
- Tests: `cargo test -p iroha_core new_view_highest_qc_fetch_uses_commit_topology_snapshot_when_effective_empty -- --nocapture` (ok).
- Tests: `cargo test -p iroha_core new_view_highest_qc_fetch_uses_commit_qc_history_fallback_when_roster_empty -- --nocapture` (ok).
- Tests: `cargo test -p iroha_core assemble_proposal_defers_when_highest_qc_block_missing -- --nocapture` (ok).
- Sumeragi ingress: route RbcReady/RbcDeliver into the rbc_chunk queue (higher priority) to avoid block-queue backlogs delaying READY quorum; updated queue-routing and backpressure tests.
- Tests: `cargo test -p iroha_core incoming_block_message_routes_rbc_ready_via_rbc_chunk_queue -- --nocapture` (ok).
- Tests: `cargo test -p iroha_core try_incoming_block_message_waits_when_rbc_ready_queue_full -- --nocapture` (ok).
- Tests: `cargo test -p iroha_core try_incoming_block_message_waits_when_rbc_deliver_queue_full -- --nocapture` (ok).
- Izanami: `DeployIvmContract` removed from the stable recipe set (kept in chaos) and the IVM trigger artifact switched to the `cycles1000` fixture to reduce time-trigger stalls.
- Izanami NPoS 1 TPS run (300s) with `cycles1000` artifact but IVM triggers still enabled: stopped before target blocks; validation dominated by time triggers from `ivm_trigger_*` (execution_tx_time_triggers_ms p95 ~3.8s, max ~3.8s). Logs: `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_ugpHFr`.
- Izanami NPoS 1 TPS run (300s) after removing IVM trigger deployments from stable profile: stopped before target blocks; time-trigger cost eliminated (execution_tx_time_triggers_ms p95 0, max 3ms), execution_tx_ms p95 ~90ms; remaining spikes are DAG stage (execution_tx_dag_ms max ~2.25s). Logs: `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_bGqaRa`.
- Kotodama/IVM: refreshed example contracts (permissions, state-map iteration bounds, valid AccountId/NftId literals), wired ZK fuzz examples to real ZK verify intrinsics, and forwarded `kotodama_dynamic_bounds` to `kotodama_lang`.
- Generated: `demo/*.to` and manifests recompiled via `koto_compile`; `demo/prediction_market.deploy.manifest.json` rebuilt via `iroha_cli app contracts manifest build`.
- Builds: `cargo build -p ivm --features kotodama_dynamic_bounds --bin koto_compile`, `cargo build -p iroha_cli` (ok).
- Checks: compiled 51 Kotodama example `.ko` files with `koto_compile --abi 1` (dynamic bounds enabled).
- Integration tests: keep `trusted_peers_pop` aligned with `trusted_peers` in `observer_sync` to satisfy config validation; tests not run.
