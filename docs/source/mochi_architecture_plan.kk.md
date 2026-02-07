---
lang: kk
direction: ltr
source: docs/source/mochi_architecture_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ffca282b5a2bb2506f46ac2c7a8985ff2f7d10a46bc999a002277956c9f452b0
source_last_modified: "2026-01-05T09:28:12.023255+00:00"
translation_last_reviewed: 2026-02-07
title: MOCHI Architecture Plan
description: High-level design for the MOCHI local-network GUI supervisor.
---

# MOCHI Architecture Plan


## Goals

- Bootstrap single-peer or multi-peer (four-node BFT) local networks quickly.
- Wrap `kagami`, `irohad`, and supporting binaries in a friendly GUI workflow.
- Surface live block, event, and state data through Torii HTTP/WebSocket endpoints.
- Provide structured builders for transactions and Iroha Special Instructions (ISI), with local signing and submission.
- Manage snapshots, re-genesis flows, and configuration tweaks without editing files manually.
- Ship as a single cross-platform Rust binary with no webview or Docker dependency.

## Architecture Overview

MOCHI is split into two primary crates housed in a new `/mochi` directory (see the
[MOCHI Quickstart](mochi/quickstart.md) for build and usage instructions):

1. `mochi-core`: a headless library responsible for configuration templating, key and genesis material generation, supervising child processes, driving Torii clients, and managing filesystem state.
2. `mochi-ui-egui`: a desktop application built on `egui`/`eframe` that renders the user interface and delegates all orchestration through the `mochi-core` API.

Additional front ends (for example a Tauri shell) can hook into `mochi-core` later without reworking the supervisor logic.

## Process Model

- Peer nodes run as separate `irohad` child processes. MOCHI never links the peer as a library, avoiding unstable internal APIs and matching production deployment topologies.
- Genesis and key material are created through `kagami` invocations with user provided inputs (chain ID, initial accounts, assets).
- Configuration files are generated from TOML templates, filling in Torii and P2P ports, storage paths, snapshot settings, and trusted peer lists. Generated configs are stored beneath a per-network workspace directory.
- The supervisor tracks process lifecycles, streams stdout/stderr for log surfaces, and polls `/status`, `/metrics`, and `/configuration` endpoints for health.
- A thin Torii client layer wraps HTTP and WebSocket calls, leaning on the Iroha Rust client crates where possible to avoid reimplementing SCALE encoding/decoding.

## User Flows Backed by `mochi-core`

- **Network Creation Wizard**: choose single or four-peer profile, pick directories, and call `kagami` to generate identities plus genesis.
- **Lifecycle Controls**: start, stop, restart peers; surface live metrics; expose log tails; toggle runtime configuration endpoints (e.g., log levels).
- **Block and Event Streams**: subscribe to `/block/stream` and `/events`, storing an in-memory rolling buffer for UI panels.
- **State Explorer**: run Norito-backed `/query` calls to list domains, accounts, assets, and asset definitions with pagination helpers and metadata summaries.
- **Transaction Composer**: stage mint/transfer instruction drafts, batch them into signed transactions, preview the Norito payload, submit via `/transaction`, and monitor the resulting events; vault signing hooks remain a future iteration.
- **Snapshots and Re-Genesis**: orchestrate Kura snapshot export/import, wipe stores, and regenerate genesis material for quick resets.

## UI Layer (`mochi-ui-egui`)

- Uses `egui`/`eframe` to ship a single native executable without external runtimes.
- Layout includes:
  - **Network Dashboard** with peer cards, health indicators, and quick actions.
  - **Blocks** panel streaming recent commits and allowing height search.
  - **Events** panel filtering transaction statuses by hash or account.
  - **State Explorer** tabs for domains, accounts, assets, and asset definitions with paginated Norito results plus raw dumps for inspection.
- **Composer** form with batchable mint/transfer palettes, queue management (add/remove/clear), raw Norito preview, and submission feedback backed by the signer vault so operators can swap between dev and real authorities.
- **Genesis & Snapshots** management view.
- **Settings** for runtime toggles and data directory shortcuts.
- UI subscribes to asynchronous updates from `mochi-core` via channels; the core exposes a `SupervisorHandle` that streams structured events (peer status, block headers, transaction updates).

## Local Development Notes

- The workspace config sets `ZSTD_SYS_USE_PKG_CONFIG=1` so `zstd-sys` links against the host `libzstd` instead of fetching vendored archives. This keeps pqcrypto-dependent builds (and MOCHI tests) running inside offline or sandboxed environments.

## Packaging and Distribution

- MOCHI bundles (or discovers on `PATH`) the `irohad`, `iroha_cli`, and `kagami` binaries.
- Uses `rustls` for outbound HTTPS to avoid OpenSSL dependencies.
- Stores all generated artifacts in a dedicated application data root (e.g., `~/.local/share/mochi` or platform equivalent) with per-network subdirectories. GUI provides “reveal in Finder/Explorer” helpers.
- Auto-detects and reserves Torii (8080+) and P2P (1337+) ports before launching peers to prevent conflicts.

## Future Extensions (Out of Scope for MVP)

- Alternate front ends (Tauri, CLI headless mode) sharing `mochi-core`.
- Multi-host orchestration for distributed testing clusters.
- Visualizers for consensus internals (Sumeragi round states, gossip timings).
- Integration with CI pipelines for automated ephemeral network snapshots.
- Plug-in system for custom dashboards or domain-specific inspectors.

## References

- [Torii Endpoints](https://docs.iroha.tech/reference/torii-endpoints.html)
- [Peer Configuration Parameters](https://docs.iroha.tech/reference/peer-config/params.html)
- [`kagami` repository documentation](https://github.com/hyperledger-iroha/iroha)
- [Iroha Special Instructions](https://iroha-test.readthedocs.io/en/iroha2-dev/references/isi/)
