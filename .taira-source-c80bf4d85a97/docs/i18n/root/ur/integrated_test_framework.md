---
lang: ur
direction: rtl
source: integrated_test_framework.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ff9e1108802fdd57703749069f87270c4195f4037a32aa65c28cde9a67b63e98
source_last_modified: "2025-11-02T04:40:28+00:00"
translation_last_reviewed: 2026-01-30
---

# Integration Test Framework for Hyperledger Iroha (7‑Node Network)

## Introduction

Hyperledger Iroha v2 provides a rich set of Iroha Special Instructions (ISI) for domain, asset, permission, and peer management. This document specifies an integration test framework for a 7‑peer network to validate correctness, consensus, and cross‑peer consistency under normal and faulty conditions (tolerating up to 2 faulty peers).

These guidelines reflect the recent genesis schema migration to multi-transaction blocks.

Note on API: In this codebase, Torii is an HTTP/WebSocket API (Axum). Tests should use HTTP endpoints (commonly ports 8080+). No additional RPC service listens on 50051.

## Objectives

- Automated 7‑peer orchestration: Start and manage peers programmatically for fast CI (primary), or via Docker Compose (optional).
- Genesis configuration: Start from a common Norito genesis with deterministic accounts/keys and required permissions.
- Exercise ISI: Systematically cover ISI via transactions and verify resulting state.
- Cross‑peer consistency: Query multiple peers after each step to ensure identical ledger state.
- Fault tolerance checks: With up to 2 peers offline or partitioned, remaining peers continue; returning peers catch up without divergence.

## Orchestration Modes

- Rust harness (recommended): `crates/iroha_test_network` spawns `irohad` processes locally, allocates ports, writes per‑run configs and Norito `.nrt` genesis, monitors readiness and block heights, and provides utilities for shutdown/restart.
- Docker Compose (optional): `crates/iroha_swarm` generates a Compose file for N peers. Use when containerized networking or external orchestration is desired.

## Test Network Setup

**Genesis Block:**

- Source: `defaults/genesis.json` which now groups instructions into a `transactions` array. Tests may append instructions with `NetworkBuilder::with_genesis_instruction` and start a new transaction via `.next_genesis_transaction()`. The resulting block is serialized to Norito `.nrt`.
- Topology: Stored in the first transaction (`transactions[0].topology`) and includes all 7 peers (public key + address), so each peer knows the network from the start.
- Accounts/Permissions: Prefer standardized accounts from `crates/iroha_test_samples` (`ALICE_ID`, `BOB_ID`, `SAMPLE_GENESIS_ACCOUNT_KEYPAIR`) with explicit grants for test scenarios, e.g. `CanManagePeers`, `CanManageRoles`, `CanMintAssetWithDefinition`.
- Injection strategy: With the harness, genesis is typically provided to one peer (the “genesis submitter”); other peers catch up via block sync. With Compose, point all peers to the same genesis path.

**Networking and Ports:**

- Torii HTTP API: `API_ADDRESS` (Axum). For Compose, map `8080`–`8086` for 7 peers to host. The harness auto‑allocates loopback ports.
- P2P: Internal peer‑to‑peer address is configured in `trusted_peers` and gossiped. The harness auto‑sets `trusted_peers` per run.

**Data Storage:**

- Kura: Iroha v2 block storage (no RocksDB/Postgres container needed). Configure via `[kura]` (e.g., `store_dir`). Disable snapshots for tests via `[snapshot]`.

**Harness flow:**

1) Build network: `NetworkBuilder::new().with_peers(7)`; optionally `.with_pipeline_time(...)` and `.with_config_layer(...)` for overrides; choose IVM fuel via `IvmFuelConfig`.
2) Start peers: `.start()` or `.start_blocking()`; writes config layers, sets `trusted_peers`, injects genesis for one peer, and waits for readiness.
3) Readiness: `Network::ensure_blocks(height)` or `once_blocks_sync(...)` ensures non‑empty block heights reach expectations across peers. Alternatively, poll `Client::get_status()`.

**Docker Compose flow (optional):**

1) Generate compose: Use `iroha_swarm::Swarm` to emit a compose file with N peers. Map API ports to host and set env vars (CHAIN, keys, TRUSTED_PEERS, GENESIS).
2) Start: `docker compose up`.
3) Readiness: Poll Torii HTTP endpoints until status healthy and block height ≥ 1.

## Test Harness Implementation (Rust)

**Client & Transactions:**

- Use `iroha::client::Client` (HTTP/WebSocket) to submit transactions and queries to Torii.
- Build transactions from ISI `InstructionBox` sequences or IVM bytecode; sign with deterministic `KeyPair` values from `iroha_test_samples`.
- Useful calls: `submit_blocking`, `submit_all_blocking`, `query(...).execute()/execute_all()`, `get_status()`, block and event streams over WebSocket.

**Cross‑Peer Consistency:**

- After each operation, query every running peer (`Network::peers()` → `peer.client()`) and compare results (e.g., balances, definitions, peer lists). This ensures strong consistency checks beyond single‑peer verification.

**Fault Injection (no containers):**

- Use the relay/proxy utilities in `integration_tests/tests/extra_functional/unstable_network.rs` to rewrite `trusted_peers` to TCP proxies and selectively suspend links. This enables targeted partitions, drops, and reconnections.

**IVM prerequisites:**

- Some tests require prebuilt IVM samples; ensure `crates/ivm/target/prebuilt/build_config.toml` exists. Where missing, tests can be skipped (current integration tests do this check).

## 7‑Node Scenario Outline

1) Start a 7‑peer network (harness or Compose) and wait for genesis commit across peers.
2) Execute a suite of ISI:
   - Register domains/accounts/assets; grant/revoke permissions; mint/burn/transfer assets; set/remove key‑values; register triggers; upgrade executor.
3) After each logical step, run cross‑peer queries to verify identical state.
4) Stop 1–2 peers, continue submitting transactions with remaining 5 peers; ensure progress and consistency among running peers.
5) Restart stopped peers and verify catch‑up and cross‑peer equality.

## CI Usage

- Build: `cargo build --workspace`
- Prebuild IVM samples (as required by tests)
- Test: `cargo test --workspace`
- Optional stricter lint: `cargo clippy --workspace --all-targets -- -D warnings`

## Conclusion

This framework leverages the in‑repo Rust harness (`iroha_test_network`) and HTTP‑based client (`iroha::client::Client`) to validate a 7‑peer Iroha v2 network. It emphasizes cross‑peer consistency, realistic fault scenarios, and repeatable setup/teardown suitable for CI. Docker Compose via `iroha_swarm` is available when containerization is preferred.

## Explorer

- Any blockchain explorer that speaks Torii HTTP/WebSocket can connect to each peer independently. Each peer exposes a Torii endpoint (host:port) suitable for status, blocks, queries, and events.
- With the Rust harness: after building a network you can derive the URLs for all peers:

  ```rust
  use iroha_test_network::NetworkBuilder;

  let network = NetworkBuilder::new().with_peers(7).build();
  let urls = network.torii_urls();
  // e.g. ["http://127.0.0.1:8080", ..., "http://127.0.0.1:8086"]
  ```

  Per‑peer helpers are also available: `peer.api_address()` and `peer.torii_url()`.

- With Docker Compose (`iroha_swarm`): generate a 7‑peer compose file and map `8080..8086` to the host; point your explorer to each of those addresses. If your explorer supports multiple endpoints, configure all 7; otherwise run one explorer instance per peer.
