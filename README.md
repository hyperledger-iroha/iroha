# Hyperledger Iroha

[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

Hyperledger Iroha is a simple and efficient blockchain ledger built on **distributed ledger technology (DLT)**. Its design embraces the Japanese Kaizen mindset of eliminating excess (*muri*) to deliver a robust implementation.

Iroha helps you manage accounts, assets, and on-chain data storage with efficient smart contracts while remaining both Byzantine- and crash-fault tolerant.

> _Documentation status:_ this README reflects the workspace state captured in
> `status.md` (last updated 2026-02-12). Refer to that file for component health,
> recent milestones, and outstanding follow-up items.

## Release Lines

This repository ships two deployment tracks from the same codebase:

- **Iroha 2** — for self-hosted permissioned or consortium networks. Operators control their own governance,
  genesis data, upgrade cadence, and feature toggles while reusing the shared crates.
- **Iroha 3 (SORA Nexus)** — the single global Nexus network built on the multi-lane pipeline and data-space
  architecture. Organisations onboard by registering data spaces that plug into the shared global ledger.

Both tracks use the same Iroha Virtual Machine (IVM) and Kotodama toolchain, so smart contracts and bytecode
artifacts run unchanged across self-hosted deployments and the SORA Nexus ledger.

---
For a Japanese overview, see [`README.ja.md`](./README.ja.md). Hebrew, Spanish,
Portuguese, French, Russian, Arabic, and Urdu overviews live at
[`README.he.md`](./README.he.md), [`README.es.md`](./README.es.md),
[`README.pt.md`](./README.pt.md), [`README.fr.md`](./README.fr.md),
[`README.ru.md`](./README.ru.md), [`README.ar.md`](./README.ar.md), and
[`README.ur.md`](./README.ur.md). Translation workflow and language coverage
guidelines live in [`docs/i18n/README.md`](./docs/i18n/README.md).

## Features

Iroha is a fully featured blockchain ledger. With it you can:

* Build custom fungible and non-fungible assets using deterministic Norito schemas
* Manage user accounts with hierarchical domains, multi-signature policies, and configurable metadata
* Execute smart contracts via the wide-opcode Kotodama compiler targeting the Iroha Virtual Machine (IVM), or fall back to built-in Special Instructions
* Operate the NPoS Sumeragi consensus pipeline with VRF commit/reveal randomness; SORA Nexus deployments enable RBC-backed data availability and governance-driven slashing evidence by default, while self-hosted networks can opt in as needed
* Protect confidential flows with the on-ledger confidential feature digest, verifier key registries, and deterministic proof timeouts
* Deploy in permissioned or permissionless configurations with telemetry, a CLI, and Norito toolchains included out of the box

Iroha also offers:

* Byzantine fault tolerance with up to 33% faulty validators under the NPoS Sumeragi pacemaker
* Deterministic execution with memory-resident world state and Norito-backed persistence
* Rich telemetry, evidence, and randomness reporting (see [Telemetry](#telemetry))
* Modular architecture with clearly separated crates and Norito codec reuse across components
* Strongly typed, event-driven design streaming over SSE/WebSocket alongside Norito JSON projections

## Overview

- Check the [system requirements](#system-requirements) and learn how to [build, test, and run Iroha](#build-test-and-run-iroha)
- Explore the provided [crates](#integration)
- Learn how to [configure and operate Iroha](#maintenance)
- Review the [genesis configuration](./docs/genesis.md) used to bootstrap a network
- Understand [P2P queue capacities and metrics](./docs/source/p2p.md)
- Walk through the end-to-end [transaction processing pipeline](./docs/source/pipeline.md) and the pacemaker/data-availability metrics summarised in [new_pipeline.md](./new_pipeline.md)
- Read the [Sumeragi consensus, randomness, and evidence guides](./docs/source/sumeragi.md) together with the governance API reference (`docs/source/governance_api.md`)
- Dive into Norito streaming and telemetry via [docs/source/norito_streaming.md](./docs/source/norito_streaming.md)
- Build Swift/iOS clients using the [IrohaSwift documentation bundle](./docs/README.md#swift--ios-sdk-references), including the Connect SDK quickstart, Xcode integration guide, and SwiftUI demo instructions
- Continue reading in the [further reading](#further-reading) section
  - ZK app API (attachments + prover reports): `docs/source/zk_app_api.md`
  - CI smoke test harness: `scripts/ci/zk_smoke.sh` (`register-asset` + `shield`)

Community resources:
- [Contribute](./CONTRIBUTING.md) to the repository
- [Contact us](./CONTRIBUTING.md#contact) for support

## System Requirements

RAM and storage requirements depend on whether you are building or operating a network, as well as the network size and transaction volume. Use this table as a guideline:

| Use case          | CPU               | RAM   | Storage |
|-------------------|-------------------|-------|-------------|
| Build (minimum)   | Dual-core CPU     | 4GB   | 20GB        |
| Build (recommended) | AMD Ryzen™ 5 1600 | 16GB | 40GB        |
| Deploy (small)    | Dual-core CPU     | 8GB+  | 20GB+       |
| Deploy (large)    | AMD Epyc™ 64-core | 128GB | 128GB+      |

RAM considerations:

* On average, plan for 5 KiB per account. A network with 1,000,000 accounts consumes about 5 GiB.
* Each `Transfer` or `Mint` instruction consumes roughly 1 KiB.
* Because transactions live in memory, RAM usage grows linearly with sustained throughput and uptime.

CPU considerations:

* Rust compilation favours multi-core CPUs such as Apple M1™, AMD Ryzen™/Threadripper™/Epyc™, and Intel Alder Lake™.
* On systems with limited memory and many cores, compilation may fail with `SIGKILL`. Use `cargo build -j <number>` (replace `<number>` with half your RAM capacity, rounded down) to limit parallelism.

## Build, Test, and Run Iroha

### Prerequisites

* [Rust](https://www.rust-lang.org/learn/get-started) (stable toolchain; profiling builds follow the [profiling](./CONTRIBUTING.md#profiling) section and require nightly)
* (Optional) [Docker](https://docs.docker.com/get-docker/)
* (Optional) [Docker Compose](https://docs.docker.com/compose/install/)
* (Optional) Pre-built IVM bytecode samples for tests that depend on them. The previous `scripts/build_ivm.sh` helper has been removed.

### Build Iroha

Build the full workspace with the stable toolchain:

```bash
cargo build --workspace
```

Optional helpers:

```bash
# Enable incremental compilation
CARGO_INCREMENTAL=1 cargo build

# Capture verbose diagnostics and timing
cargo build -vv --timings

# Build the latest Docker image
docker build . -t hyperledger/iroha:dev
```

If you skip the Docker build, the latest available image is used when starting the containerised peer.

### Regenerate codec samples

When the data model schema changes, regenerate the Norito codec samples used by `kagami`:

```bash
cargo run --manifest-path scripts/regenerate_codec_samples/Cargo.toml
```

Artifacts are written to `crates/iroha_kagami/samples/codec/`.

### Run UI tests manually

UI tests validate compile-time diagnostics and are skipped in CI environments. Run them locally when making UI-affecting changes:

```bash
cargo test -p iroha_data_model --test ui
```

### Test the workspace

Use the workspace test command to run unit, integration, and doc tests:

```bash
cargo test --workspace
```

### Run Iroha

After building, start a minimum viable network using the provided Compose bundle:

```bash
docker compose -f defaults/docker-compose.yml up
```

With the Docker Compose stack running, access it through the [Iroha Client CLI](crates/iroha_cli/README.md):

```bash
cargo run --bin iroha -- --config ./defaults/client.toml
```

### TUI monitor (`iroha_monitor`) — attach-first / spawn-lite

A synthwave-styled terminal UI is included for inspecting node status and P2P metrics.

- Attach to running nodes (recommended):

  ```bash
  # Single node
  cargo run -p iroha_monitor -- --attach http://127.0.0.1:8080 --use-alice

  # Multiple nodes
  cargo run -p iroha_monitor -- \
    --attach http://hostA:8080 http://hostB:8080 --use-alice

  # From a TOML file
  cargo run -p iroha_monitor -- --attach-config attach.toml
  # Minimal attach.toml example:
  # endpoints = ["http://127.0.0.1:8080"]
  # [account]
  # id = "ed...@wonderland"
  # private_key = "ed25519:..."
  ```

- Spawn a local test network:

  ```bash
  cargo run -p iroha_monitor -- --spawn --peers 1
  ```

  If building `irohad` fails, the monitor prints the captured `cargo` output. Fix the error or switch to attach mode.

- Spawn-lite stubs (no node required):

  ```bash
  cargo run -p iroha_monitor -- --spawn-lite --peers 2
  ```

  This launches lightweight HTTP stubs serving `/status` (JSON) and `/metrics` (Prometheus) so the TUI can be exercised without a blockchain node.

Keyboard shortcuts: `n/p` or `+/-` (focus), `0..N` (jump), `r` (auto-rotate), `m` (metrics), `c` (CRT), `x` (CRT MAX), `?` (help), `b` (beep), `q` (quit). `Ctrl+C` exits as well. `--crt` enables green phosphor styling; `--crt-max` adds raster noise and a starfield backdrop. The monitor auto-switches to a compact layout on narrow terminals.

### Torii JSON API

Torii exposes both the signed Norito API and convenience JSON endpoints by default (the `app_api` and `transparent_api` features on `iroha_torii` are enabled). `transparent_api` forwards the data-model’s “mutable structs” feature so Torii can project raw ledger fields directly into JSON responses without adding bespoke mirror types.

- `GET /v1/accounts/{accountId}/transactions`, `POST /v1/accounts/{accountId}/transactions/query`
- `GET /v1/accounts/{accountId}/permissions` (permission tokens granted directly to the account)
- `GET /v1/accounts/{accountId}/assets`, `GET /v1/assets/{assetDefinitionId}/holders`
- `GET /v1/accounts`, `GET /v1/assets/definitions`, `GET /v1/nfts`, `GET /v1/domains`
- `POST /v1/domains/query` (JSON envelope with `filter`, `sort`, `pagination`)
- `GET /v1/parameters`
- `POST /v1/contracts/code` (wraps `RegisterSmartContractCode` into a signed transaction)
- `GET /v1/contracts/code/{code_hash}` (fetch an on-chain `ContractManifest` by code hash)
- `GET /v1/events/sse` (stream events with URL-encoded Norito filters)

Quick examples:

```bash
TORII=http://127.0.0.1:8080
curl -s "$TORII/v1/parameters" | jq .
curl -N "$TORII/v1/events/sse?filter=%7B%22op%22%3A%22eq%22%2C%22args%22%3A%5B%22tx_status%22%2C%22Approved%22%5D%7D"
```

### Experimental: IDs-only projections

An experimental feature returns only resource identifiers in list queries. Enable it via the `ids_projection` feature flag:

```bash
cargo test -p iroha_core --features ids_projection --no-run
cargo test -p iroha_cli --features "cli_integration_harness ids_projection" --no-run

cargo run --bin iroha --features ids_projection -- \
  domain list all --select ids
```

Currently supported resource types: Domains, Accounts, AssetDefinitions, NFTs, Roles, and Triggers. The feature is opt-in and subject to change.

## Telemetry

Iroha exposes Prometheus-compatible metrics. In addition to P2P and consensus gauges, the transaction pipeline publishes per-stage timings and overlay statistics. Consult `docs/source/telemetry.md` (and the Japanese companion `docs/source/telemetry.ja.md`) for a complete metric catalogue and recommended queries.

Detached execution is always enabled; per-commit gauges surface how the latest block interacted with the detached path:

- `pipeline_detached_prepared`: transactions prepared for detached execution
- `pipeline_detached_merged`: detached deltas merged successfully into the block state
- `pipeline_detached_fallback`: transactions that fell back to sequential apply

These gauges reset for every committed block. See [new_pipeline.md](./new_pipeline.md) for a deeper walk-through.

Example calls:

```bash
TORII=http://127.0.0.1:8080
curl -s "$TORII/v1/accounts/alice@wonderland/assets" | jq .
curl -s "$TORII/v1/assets/rose#wonderland/holders/query" \
  -H 'Content-Type: application/json' \
  -d '{"sort":[{"key":"quantity","order":"desc"}],"pagination":{"limit":10}}' | jq .
```

For consensus troubleshooting, the helper script `scripts/sumeragi_backpressure_log_scraper.py`
correlates pacemaker backpressure deferrals with nearby RBC retry or abort log entries. It accepts
plain-text or JSON logs (pass `-` to read from stdin) and can enrich the report with counters from a
`/v1/sumeragi/status` snapshot (`--status status.json`). See `--help` for full usage and
`docs/source/telemetry.md` for the operator runbook.

## Integration

The project is composed of multiple Rust crates. Key crates include:

- [`iroha`](crates/iroha): client library for communicating with peers
- [`irohad`](crates/irohad): command-line application for deploying an Iroha peer
- [`iroha_cli`](crates/iroha_cli): reference CLI built on the client SDK
- [`iroha_core`](crates/iroha_core): primary library powering peer endpoints and execution
- [`iroha_config`](crates/iroha_config): configuration handling and documentation generation
- [`iroha_derive`](crates/iroha_derive): macros such as `ReadConfig` (enable the `config_base` feature)
- [`iroha_crypto`](crates/iroha_crypto): cryptographic primitives used across the stack
- [`iroha_kagami`](crates/iroha_kagami): generates cryptographic keys, default genesis, configuration references, and schemas
- [`iroha_data_model`](crates/iroha_data_model): common data models
- [`norito`](crates/norito): deterministic serialization codec, featuring CRC64-XZ acceleration (x86_64 CLMUL / aarch64 PMULL), slicing-by-8 fallback, optional zstd compression (CPU/GPU via feature flags), schema hashing, and streaming iterators (`StreamSeqIter`, `StreamMapIter`, finish-on-drop guards)
- [`iroha_futures`](crates/iroha_futures): async utilities
- [`iroha_logger`](crates/iroha_logger): logging built on `tracing`
- [`iroha_macro`](crates/iroha_macro): convenience macros
- [`iroha_p2p`](crates/iroha_p2p): peer creation and handshake logic
- [`ivm`](crates/ivm): implementation of the Iroha Virtual Machine
  - Kotodama smart contracts compile to IVM bytecode (`.to`) and do not target standalone RISC-V. RISC-V-like encodings are an IVM instruction-format detail.
  - Documentation: `ivm.md`, `docs/source/ivm_syscalls.md`, `docs/source/kotodama_grammar.md`
- [`iroha_telemetry`](crates/iroha_telemetry): metrics collection and analysis
- [`iroha_version`](crates/iroha_version): versioning for rolling upgrades

Examples:
- Kotodama + IVM examples live under `examples/`. If the external tools are available, run `make examples-run` (override with `KOTO=/path/to/koto_compile` and `IVM=/path/to/ivm_run`), or follow `examples/README.md`.

## Transaction Pipeline

Transactions flow through the network as follows:

1. **Construct and sign** — a client (SDK or `iroha_cli`) builds a transaction with instructions (built-ins or IVM calls), sets metadata (e.g., nonce/TTL), and signs it according to the account quorum.
2. **Submit via Torii** — the signed transaction is sent to a peer, deserialised using Norito, and lightly validated.
3. **Admission and stateless checks** — signatures, size, instruction counts, TTL/nonce (when enabled), and other lightweight checks run. Valid transactions enter the in-memory queue; telemetry updates `queue_size`.
4. **Proposal and ordering (Sumeragi)** — the leader assembles a proposal from the queue, rechecks basic validity, and broadcasts the proposal to the committee according to Sumeragi timing parameters.
5. **Consensus** — peers validate the proposal, vote, and aggregate signatures. Upon commitment, the ordered batch is final for execution.
6. **Execution and state transition** — each peer applies the ordered transactions to its World State View. Permissions are enforced, and both ISIs and IVM smart contracts execute deterministically. Events are emitted for observers.
7. **Block commit (Kura)** — the resulting block (headers, signatures, optional payload Merkle roots) is persisted by Kura. Committed blocks gossip to lagging peers.
8. **Observability** — telemetry and WebSocket endpoints expose metrics (`blocks`, `blocks_non_empty`, `commit_time_ms`, `queue_size`, etc.) and block streams.

Accelerated paths (SIMD/GPU) are guarded by feature flags with deterministic CPU fallbacks; outputs must remain identical across hardware. Sumeragi timing knobs such as `SumeragiParameter::{BlockTimeMs,CommitTimeMs}` can be tuned (see test-network helpers).

## Maintenance

An overview of how to configure and operate an Iroha instance:

- [Configuration](#configuration)
- [Endpoints](#endpoints)
- [Logging](#logging)
- [Monitoring](#monitoring)
- [Storage](#storage)
- [Scalability](#scalability)

### Configuration

Prefer passing configuration via TOML files:

```bash
irohad --config /path/to/config.toml
```

Consensus settings (`Sumeragi`) are summarised in `docs/source/sumeragi.md`. Example `[sumeragi]` snippet:

```toml
[sumeragi]
role = "validator"
allow_view0_slack = false
collectors_k = 2
collectors_redundant_send_r = 2
msg_channel_cap_votes = 8192
msg_channel_cap_block_payload = 128
msg_channel_cap_rbc_chunks = 1024
msg_channel_cap_blocks = 256
control_msg_channel_cap = 1024
```

See `docs/source/references/peer.template.toml` for a broader template. The detailed configuration reference is still evolving.

### Endpoints

Refer to the [Torii endpoint reference](https://docs.iroha.tech/reference/torii-endpoints.html) for the full list of endpoints, operations, and parameters.

### Logging

Logs default to human-readable output on `stdout`. Adjust the log level via configuration (`logger.level`) or at runtime using the `/configuration` endpoint:

```bash
curl -X POST \
  -H 'Content-Type: application/json' \
  http://127.0.0.1:8080/configuration \
  -d '{"logger":{"level":"DEBUG"}}' -i
```

`logger.format` accepts `full` (default), `compact`, `pretty`, or `json`. Redirecting output or log rotation is the operator’s responsibility.

### Monitoring

`/status` returns JSON; `/metrics` returns Prometheus-formatted metrics. To get started, install Prometheus and use the template at `docs/source/references/prometheus.template.yml`.

Key Sumeragi metrics:

- `sumeragi_collectors_k` / `sumeragi_redundant_send_r`: current multi-collector configuration
- `sumeragi_redundant_sends_total`: timeout-driven redundant sends (r > 1)
- `sumeragi_redundant_sends_by_collector{idx}` / `..._by_peer{peer}` / `..._by_role_idx{role_idx}`: redundant send breakdown by collector index, peer, and local validator role index
- `sumeragi_collectors_targeted_current`: collectors targeted for the current voting block
- `sumeragi_collectors_targeted_per_block`: histogram of collectors targeted per committed block
- `sumeragi_cert_size`: histogram of signature counts per finalisation certificate
- `sumeragi_dropped_block_messages_total` / `sumeragi_dropped_control_messages_total`: channel drops due to full Sumeragi channels

Prometheus query examples:

- `sum by (le) (rate(sumeragi_collectors_targeted_per_block_bucket[5m]))`
- `histogram_quantile(0.95, sum by (le) (rate(sumeragi_collectors_targeted_per_block_bucket[5m])))`
- `rate(sumeragi_redundant_sends_total[5m])`
- `topk(5, sum by (peer) (rate(sumeragi_redundant_sends_by_peer[5m])))`
- `rate(sumeragi_dropped_block_messages_total[5m])`

### Storage

Blocks and snapshots live in the `storage` directory created in the peer’s working directory. Override the location with `kura.block_store_path` (resolved relative to the configuration file).

### Scalability

Multiple peers and client binaries can run on the same machine, even within the same working directory. We still recommend allocating a clean working directory per instance. The provided `docker-compose` file demonstrates a minimum viable network and how to work with the `hyperledger/iroha:dev` image.

## Further Reading

Start with the [Iroha 2 Tutorial](https://docs.iroha.tech) for language-specific guides (Bash, Python, Rust, Kotlin/Java, JavaScript/TypeScript).

* [Iroha 2 Documentation](https://docs.iroha.tech)
  * [Glossary](https://docs.iroha.tech/reference/glossary.html)
  * [Iroha Special Instructions](https://docs.iroha.tech/blockchain/instructions.html)
  * [Torii API Reference](https://docs.iroha.tech/reference/torii-endpoints.html)
* [Iroha 2 Whitepaper](./docs/source/iroha_2_whitepaper.md)
* [Data Model & ISI Spec (implementation derived)](./docs/source/data_model_and_isi_spec.md)
* [Error Mapping Guide](./docs/source/error_mapping.md)

Iroha SDKs:

* [Iroha Python](https://github.com/hyperledger-iroha/iroha-python)
* [Iroha Java](https://github.com/hyperledger-iroha/iroha-java)
* [Iroha JavaScript/TypeScript](https://github.com/hyperledger-iroha/iroha-javascript)
* Iroha Swift/iOS: see the [IrohaSwift SDK and integration guides](./docs/README.md#swift--ios-sdk-references)

## How to Contribute

We welcome community contributions. Report bugs and suggest improvements via GitHub issues or pull requests. See [CONTRIBUTING.md](./CONTRIBUTING.md) for details.

## Get Help

Use the channels listed in [CONTRIBUTING.md#contact](./CONTRIBUTING.md#contact) to get help or engage with the community.

## License

Iroha is licensed under the Apache License, Version 2.0. See [LICENSE](./LICENSE) for details.

Documentation is provided under the Creative Commons Attribution 4.0 International (CC-BY-4.0) license: http://creativecommons.org/licenses/by/4.0/
