---
lang: am
direction: ltr
source: docs/portal/docs/sorafs/node-storage.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ffa884bf745ab5f79c20d4b20baaba842878301dc56a66463c4520275ce4fd0b
source_last_modified: "2026-01-05T09:28:11.899124+00:00"
translation_last_reviewed: 2026-02-07
id: node-storage
title: SoraFS Node Storage Design
sidebar_label: Node Storage Design
description: Storage architecture, quotas, and lifecycle hooks for Torii nodes hosting SoraFS data.
---

:::note Canonical Source
:::

## SoraFS Node Storage Design (Draft)

This note refines how an Iroha (Torii) node can opt-in to the SoraFS data
availability layer and dedicate a slice of local disk for storing and serving
chunks. It complements the `sorafs_node_client_protocol.md` discovery spec and
the SF-1b fixture work by outlining the storage-side architecture, resource
controls, and configuration plumbing that must land in the node and gateway
code paths. Practical operator drills live in the
[Node Operations Runbook](./node-operations).

### Goals

- Allow any validator or auxiliary Iroha process to expose spare disk as a
  SoraFS provider without affecting the core ledger responsibilities.
- Keep the storage module deterministic and Norito-driven: manifests,
  chunk plans, Proof-of-Retrievability (PoR) roots, and provider adverts are the
  source of truth.
- Enforce operator-defined quotas so a node cannot exhaust its own resources by
  accepting too many pin or fetch requests.
- Surface health/telemetry (PoR sampling, chunk fetch latency, disk pressure)
  back to governance and clients.

### High-level Architecture

```
┌──────────────────────────────────────────────────────────────────────┐
│                         Iroha/Torii Node                             │
│                                                                      │
│  ┌──────────────┐      ┌────────────────────┐                        │
│  │  Torii APIs  │◀────▶│   SoraFS Gateway   │◀───────────────┐       │
│  └──────────────┘      │ (Norito endpoints) │                │       │
│                        └────────┬───────────┘                │       │
│                                 │                            │       │
│                        ┌────────▼────────┐                   │       │
│                        │  Pin Registry   │◀───── manifests   │       │
│                        │ (State / DB)    │                   │       │
│                        └────────┬────────┘                   │       │
│                                 │                            │       │
│                        ┌────────▼────────┐                   │       │
│                        │  Chunk Storage  │◀──── chunk plans  │       │
│                        │  (ChunkStore)   │                   │       │
│                        └────────┬────────┘                   │       │
│                                 │                            │       │
│                        ┌────────▼────────┐                   │       │
│                        │  Disk Quota/IO  │─Pin/serve chunks─▶│ Fetch │
│                        │  Scheduler      │                   │ Clients│
│                        └─────────────────┘                   │       │
│                                                                      │
└──────────────────────────────────────────────────────────────────────┘
```

Key modules:

- **Gateway**: exposes Norito HTTP endpoints for pin proposals, chunk fetch
  requests, PoR sampling, and telemetry. It validates Norito payloads and
  marshals requests into the chunk store. Reuses the existing Torii HTTP stack
  to avoid a new daemon.
- **Pin Registry**: the manifest pin state tracked in `iroha_data_model::sorafs`
  and `iroha_core`. When a manifest is accepted the registry records the
  manifest digest, chunk plan digest, PoR root, and provider capability flags.
- **Chunk Storage**: disk-backed `ChunkStore` implementation that ingests
  signed manifests, materialises chunk plans using `ChunkProfile::DEFAULT`, and
  persists chunks under a deterministic layout. Each chunk is associated with a
  content fingerprint and PoR metadata so sampling can re-validate without
  re-reading the entire file.
- **Quota/Scheduler**: enforces operator-configured limits (maximum disk bytes,
  maximum outstanding pins, maximum parallel fetches, chunk TTL) and coordinates
  IO so the node's ledger duties are not starved. The scheduler is also
  responsible for serving PoR proofs and sampling requests with bounded CPU.

### Configuration

Add a new section to `iroha_config`:

```toml
[sorafs.storage]
enabled = false
data_dir = "/var/lib/iroha/sorafs"
max_capacity_bytes = "100 GiB"
max_parallel_fetches = 32
max_pins = 10_000
por_sample_interval_secs = 600
alias = "tenant.alpha"            # optional human friendly tag
adverts:
  stake_pointer = "stake.pool.v1:0x1234"
  availability = "hot"
  max_latency_ms = 500
  topics = ["sorafs.sf1.primary:global"]
```

- `enabled`: participation toggle. When false the gateway returns a 503 for
  storage endpoints and the node does not advertise in discovery.
- `data_dir`: root directory for chunk data, PoR trees, and fetch telemetry.
  Defaults to `<iroha.data_dir>/sorafs`.
- `max_capacity_bytes`: hard limit for pinned chunk data. A background task
  rejects new pins when the limit is reached.
- `max_parallel_fetches`: concurrency cap enforced by the scheduler to balance
  bandwidth/disk IO against validator workload.
- `max_pins`: maximum number of manifest pins the node accepts before applying
  eviction/back pressure.
- `por_sample_interval_secs`: cadence for automatic PoR sampling jobs. Each job
  samples `N` leaves (configurable per manifest) and emits telemetry events.
  Governance can scale `N` deterministically by setting the capacity metadata
  key `profile.sample_multiplier` (integer `1-4`). The value may be a single
  number/string or an object with per-profile overrides, e.g.
  `{"default":2,"sorafs.sf2@1.0.0":3}`.
- `adverts`: structure used by the provider advert generator to fill
  `ProviderAdvertV1` fields (stake pointer, QoS hints, topics). If omitted the
  node uses defaults from the governance registry.

Config plumbing:

- `[sorafs.storage]` is defined in `iroha_config` as `SorafsStorage` and is
  loaded from the node config file.
- `iroha_core` and `iroha_torii` thread the storage config into the gateway
  builder and chunk store at startup.
- Dev/test env overrides exist (`SORAFS_STORAGE_*`, `SORAFS_STORAGE_PIN_*`), but
  production deployments should rely on the config file.

### CLI Utilities

While Torii’s HTTP surface is still being wired, the `sorafs_node` crate ships a
thin CLI so operators can script ingestion/export drills against the persistent
backend.【crates/sorafs_node/src/bin/sorafs-node.rs:1】

```bash
cargo run -p sorafs_node --bin sorafs-node ingest \
  --data-dir ./storage/sorafs \
  --manifest ./fixtures/manifest.to \
  --payload ./fixtures/payload.bin \
  --plan-json-out ./plan.json
```

- `ingest` expects a Norito-encoded manifest `.to` file plus the matching payload
  bytes. It reconstructs the chunk plan from the manifest’s chunking profile,
  enforces digest parity, persists chunk files, and optionally emits a
  `chunk_fetch_specs` JSON blob so downstream tooling can sanity-check the
  layout.
- `export` accepts a manifest ID and writes the stored manifest/payload to disk
  (with optional plan JSON) so fixtures remain reproducible across environments.

Both commands print a Norito JSON summary to stdout, making it easy to pipe into
scripts. The CLI is covered by an integration test to ensure manifests and
payloads round-trip cleanly before the Torii APIs land.【crates/sorafs_node/tests/cli.rs:1】

> HTTP parity
>
> The Torii gateway now exposes read-only helpers backed by the same
> `NodeHandle`:
>
> - `GET /v1/sorafs/storage/manifest/{manifest_id_hex}` — returns the stored
>   Norito manifest (base64) alongside digest/metadata.【crates/iroha_torii/src/sorafs/api.rs:1207】
> - `GET /v1/sorafs/storage/plan/{manifest_id_hex}` — returns the deterministic
>   chunk plan JSON (`chunk_fetch_specs`) for downstream tooling.【crates/iroha_torii/src/sorafs/api.rs:1259】
>
> These endpoints mirror the CLI output so pipelines can switch from local
> scripts to HTTP probes without changing parsers.【crates/iroha_torii/src/sorafs/api.rs:1207】【crates/iroha_torii/src/sorafs/api.rs:1259】

### Node Lifecycle

1. **Startup**:
   - If storage is enabled the node initialises the chunk store with the
     configured directory and capacity. This includes verifying or creating the
     PoR manifest database and replaying pinned manifests to warm caches.
   - Register the SoraFS gateway routes (Norito JSON POST/GET endpoints for pin,
     fetch, PoR sample, telemetry).
   - Spawn the PoR sampling worker and quota monitor.
2. **Discovery / Adverts**:
   - Generate `ProviderAdvertV1` documents using current capacity/health, sign
     them with the council-approved key, and publish via the discovery channel.
     available.
3. **Pin Workflow**:
   - Gateway receives a signed manifest (including chunk plan, PoR root, council
     signatures). Validate the alias list (`sorafs.sf1@1.0.0` required) and
     ensure the chunk plan matches the manifest metadata.
   - Check quotas. If capacity/pin limits would be exceeded respond with a
     policy error (Norito structured).
   - Stream chunk data into the `ChunkStore`, verifying digests as we ingest.
     Update PoR trees and store manifest metadata in the registry.
4. **Fetch Workflow**:
   - Serve chunk range requests from disk. Scheduler enforces
     `max_parallel_fetches` and returns `429` when saturated.
   - Emit structured telemetry (Norito JSON) with latency, bytes served, and
     error counts for downstream monitoring.
5. **PoR Sampling**:
   - Worker selects manifests proportional to weight (e.g., bytes stored) and
     runs deterministic sampling using the chunk store's PoR tree.
   - Persist results for governance audits and include summaries in provider
     adverts / telemetry endpoints.
6. **Eviction / Quota Enforcement**:
   - When capacity is reached the node rejects new pins by default. Optionally,
     operators may configure eviction policies (e.g., TTL-based, LRU) once the
     governance model is agreed; for now the design assumes strict quotas and
     operator-initiated unpin operations.

### Capacity Declaration & Scheduling Integration

- Torii now relays `CapacityDeclarationRecord` updates from `/v1/sorafs/capacity/declare`
  to the embedded `CapacityManager`, so each node builds an in-memory view of its
  committed chunker and lane allocations. The manager exposes read-only snapshots
  for telemetry (`GET /v1/sorafs/capacity/state`) and enforces per-profile or per-lane
  reservations before new orders are accepted.【crates/sorafs_node/src/capacity.rs:1】【crates/sorafs_node/src/lib.rs:60】
- The `/v1/sorafs/capacity/schedule` endpoint accepts governance-issued `ReplicationOrderV1`
  payloads. When the order targets the local provider the manager checks for
  duplicate scheduling, verifies chunker/lane capacity, reserves the slice, and
  returns a `ReplicationPlan` describing remaining capacity so orchestration tools
  can proceed with ingestion. Orders for other providers are acknowledged with an
  `ignored` response to ease multi-operator workflows.【crates/iroha_torii/src/routing.rs:4845】
- Completion hooks (e.g., triggered after ingestion succeeds) hit
  `POST /v1/sorafs/capacity/complete` to release reservations via
  `CapacityManager::complete_order`. The response includes a `ReplicationRelease`
  snapshot (remaining totals, chunker/lane residuals) so orchestration tooling can
  queue the next order without polling. Follow-up work will wire this into the chunk
  store pipeline once ingestion logic lands.【crates/iroha_torii/src/routing.rs:4885】【crates/sorafs_node/src/capacity.rs:90】
- The embedded `TelemetryAccumulator` can be mutated through
  `NodeHandle::update_telemetry`, letting background workers record PoR/uptime samples
  and eventually derive canonical `CapacityTelemetryV1` payloads without touching the
  scheduler internals.【crates/sorafs_node/src/lib.rs:142】【crates/sorafs_node/src/telemetry.rs:1】

### Integrations & Future Work

- **Governance**: extend `sorafs_pin_registry_tracker.md` with storage telemetry
  (PoR success rate, disk utilisation). Admission policies can require minimum
  capacity or minimum PoR success rate before adverts are accepted.
- **Client SDKs**: expose the new storage config (disk limits, alias) so
  management tooling can bootstrap nodes programmatically.
- **Telemetry**: integrate with the existing metrics stack (Prometheus /
  OpenTelemetry) so storage metrics appear in observability dashboards.
- **Security**: run the storage module inside a dedicated async task pool with
  back-pressure and consider sandboxing chunk reads via io_uring or tokio's
  bounded pools to prevent malicious clients from exhausting resources.

This design keeps the storage module optional and deterministic while giving
operators the knobs they need to participate in the SoraFS data availability
layer. Implementing it will involve changes across `iroha_config`, `iroha_core`,
`iroha_torii`, and the Norito gateway, plus the provider advert tooling.
