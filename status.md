# Status

Last update: 2026-01-19

- Integration tests (asset): submit helpers now broadcast signed transactions across peers and tolerate duplicate-enqueue responses to avoid queued timeouts; added duplicate-error detection coverage. Tests not re-run yet.
- Genesis: consensus handshake metadata now refreshes during manifest parse/normalize to keep fingerprints aligned with effective parameters (added `build_and_sign_refreshes_stale_consensus_fingerprint` test).
- Config: fixed `connect_startup_delay` initialization in torii test utils and p2p start wiring.
- Tests: `cargo test -p integration_tests genesis_asset_minted_across_peers -- --nocapture` (ok).
- Tests: `cargo test -p integration_tests genesis_norito_bytes_roundtrip_network -- --nocapture` (ok).
- NPoS bootstrap: RBC roster derivation now falls back to the active topology near the tip to avoid empty-session snapshots; added `rbc_roster_for_session_uses_active_topology_in_npos_bootstrap` (tests not run).
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
