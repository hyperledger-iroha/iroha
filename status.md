# Status

Last update: 2026-01-19

- Sumeragi pacemaker: idle view changes now pause when consensus block-payload/RBC-chunk queues are saturated to avoid churn under payload backlog; added `force_view_change_if_idle_skips_when_consensus_queue_backpressure`.
- Tests: `cargo test -p iroha_core force_view_change_if_idle_skips_when_consensus_queue_backpressure -- --nocapture` (timed out waiting for Cargo build dir lock).
- Sumeragi pacemaker: defer proposal assembly while block-payload/RBC-chunk queues are saturated; added `consensus_queue_backpressure_trips_on_payload_or_rbc_queue` + `proposal_backpressure_defers_on_consensus_queue_backpressure`. Docs updated (`docs/source/sumeragi.md`, `docs/source/references/configuration.md`, `docs/source/references/peer.template.toml`).
- Tests: `cargo test -p iroha_core consensus_queue_backpressure -- --nocapture` (ok).
- DA/RBC: default RBC chunk fanout now targets the full roster (minus local) so READY quorum can form even with f faulty peers; unit coverage updated.
- DA/RBC: unresolved backlog detection now gates on incomplete READY quorum/chunk coverage (even when payload is local) and pending RBC stashes; idle view changes stay suppressed while RBC backlog or relay backpressure is active; tests added/updated (not run).
- DA/RBC: treat blocks present in Kura as committed only when their stored height is <= committed height, and keep accepting RBC messages for committed heights when payload is missing; added `rbc_message_stale_ignores_uncommitted_kura_blocks` + `rbc_message_stale_allows_missing_payload_for_committed_height_with_da` (not run).
- DA/RBC: pending RBC stashes at the tip height now gate view changes (alongside tip+1) to prevent premature leader rotation; added `rbc_backlog_counts_pending_stash_for_tip_height` (not run).
- Format: `cargo fmt --all` (warns about nightly-only rustfmt options in config).
- Sumeragi pacemaker: rebroadcast cached proposals + BlockCreated payloads when the local leader already has the proposal cached, avoiding stalls when proposal messages are dropped; added `pacemaker_rebroadcasts_cached_proposal_when_leader` coverage.
- Tests: `cargo test -p iroha_core pacemaker_rebroadcasts_cached_proposal_when_leader -- --nocapture` (ok).
- Tests: `cargo test --workspace` failed during compile (missing `connect_startup_delay` in `iroha_p2p` integration test Network configs).
- Integration tests (asset): submit helpers now broadcast signed transactions across peers and tolerate duplicate-enqueue responses to avoid queued timeouts; added duplicate-error detection coverage. Tests not re-run yet.
- Integration tests: NetworkBuilder::new now defaults to a 1s pipeline time for faster test networks; `with_default_pipeline_time` keeps Sumeragi defaults. Tests not run yet.
- Torii/client: queue rejections now include `x-iroha-reject-code`; client ResponseReport decodes Norito error envelopes to surface codes/messages. Tests: `cargo test -p iroha --lib response_report::with_msg_decodes_norito_error_envelope`, `cargo test -p iroha_torii --lib push_into_queue_error_sets_reject_code_header` (ok).
- Integration tests: consolidated asset mint quantity cases into one network, tightened polling delays, and moved most integration tests to a 2s pipeline time for faster runs (unstable_network and NPoS performance timing left unchanged). Tests not run yet.
- Integration tests: collapsed misc, pagination, non-mintable, NFT, subscription, telemetry (permissioned), and time-trigger suites to reuse a single network per file and set 2s pipeline time where safe. Tests not run yet.
- Genesis: consensus handshake metadata now refreshes during manifest parse/normalize to keep fingerprints aligned with effective parameters (added `build_and_sign_refreshes_stale_consensus_fingerprint` test).
- Config: fixed `connect_startup_delay` initialization in torii test utils and p2p start wiring.
- Tests: `cargo test -p integration_tests genesis_asset_minted_across_peers -- --nocapture` (ok).
- Tests: `cargo test -p integration_tests genesis_norito_bytes_roundtrip_network -- --nocapture` (ok).
- NPoS bootstrap: RBC roster derivation now falls back to the active topology near the tip to avoid empty-session snapshots; added `rbc_roster_for_session_uses_active_topology_in_npos_bootstrap` (tests not run).
- RBC ingress/backlog: treat RbcInit as blocking ingress to avoid non-blocking drops under load, and count pending RBC stashes for the next height as unresolved backlog so pacemaker won’t trigger view changes while BlockCreated/INIT is missing; added `rbc_backlog_counts_pending_stash_for_next_height` + updated relay blocking-variant test (tests not run).
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
