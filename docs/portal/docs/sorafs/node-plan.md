---
id: node-plan
title: SoraFS Node Implementation Plan
sidebar_label: Node Implementation Plan
description: Track the SF-3 storage implementation status, remaining rollout evidence, and operator-facing SoraFS node surfaces.
---

:::note Canonical Source
:::

SF-3 delivers the first runnable `sorafs-node` crate that turns an Iroha/Torii process into a SoraFS storage provider. Use this plan alongside the [node storage guide](node-storage.md), [provider admission policy](provider-admission-policy.md), and [storage capacity marketplace roadmap](storage-capacity-marketplace.md) when sequencing deliverables.

## Target Scope (Milestone M1)

1. **Chunk store integration.** Wrap `sorafs_car::ChunkStore` with a persistent backend that stores chunk bytes, manifests, and PoR trees in the configured data directory.
2. **Gateway endpoints.** Expose Norito HTTP endpoints for pin submission, chunk fetch, PoR sampling, and storage telemetry within the Torii process.
3. **Configuration plumbing.** Add a `SoraFsStorage` config struct (enabled flag, capacity, directories, concurrency limits) wired through `iroha_config`, `iroha_core`, and `iroha_torii`.
4. **Quota/scheduling.** Enforce operator-defined disk/parallelism limits and queue requests with back-pressure.
5. **Telemetry.** Emit metrics/logs for pin success, chunk fetch latency, capacity utilisation, and PoR sampling results.

## Work Breakdown

### A. Crate & Module Structure

| Task | Owner(s) | Notes |
|------|----------|-------|
| Create `crates/sorafs_node` with modules: `config`, `store`, `gateway`, `scheduler`, `telemetry`. | Storage Team | Re-export reusable types for Torii integration. |
| Implement `StorageConfig` mapped from `SoraFsStorage` (user → actual → defaults). | Storage Team / Config WG | Ensure the Norito/`iroha_config` layers remain deterministic. |
| Provide a `NodeHandle` facade Torii uses to submit pins/fetches. | Storage Team | Encapsulate storage internals and async plumbing. |

### B. Persistent Chunk Store

| Task | Owner(s) | Notes |
|------|----------|-------|
| Build a disk backend wrapping `sorafs_car::ChunkStore` with an on-disk manifest index (`sled`/`sqlite`). | Storage Team | Deterministic layout: `<data_dir>/<manifest_cid>/chunk_{idx}.bin`. |
| Maintain PoR metadata (64 KiB/4 KiB trees) using `ChunkStore::sample_leaves`. | Storage Team | Support replay after restart; fail fast on corruption. |
| Implement integrity replay on startup (rehash manifests, prune incomplete pins). | Storage Team | Block Torii start until replay completes. |

### C. Gateway Endpoints

| Endpoint | Behaviour | Tasks |
|----------|-----------|-------|
| `POST /sorafs/pin` | Accept `PinProposalV1`, validate manifests, queue ingestion, respond with manifest CID. | Validate chunk profile, enforce quotas, stream data via chunk store. |
| `GET /sorafs/chunks/{cid}` + range query | Serve chunk bytes with `Content-Chunker` headers; respect range capability spec. | Use scheduler + stream budgets (tie into SF-2d range capability). |
| `POST /sorafs/por/sample` | Run PoR sampling for a manifest and return proof bundle. | Reuse chunk store sampling, respond with Norito JSON payloads. |
| `GET /sorafs/telemetry` | Summaries: capacity, PoR success, fetch error counts. | Provide data for dashboards/operators. |

Runtime plumbing threads PoR interactions through `sorafs_node::por`: the tracker records every `PorChallengeV1`, `PorProofV1`, and `AuditVerdictV1` so the `CapacityMeter` metrics reflect governance verdicts without bespoke Torii logic.【crates/sorafs_node/src/scheduler.rs#L147】

Implementation notes:

- Use Torii’s Axum stack with `norito::json` payloads.
- Add Norito schemas for responses (`PinResultV1`, `FetchErrorV1`, telemetry structs).

- ✅ `/v1/sorafs/por/ingestion/{manifest_digest_hex}?limit=N` now exposes
  bounded backlog entries plus the oldest epoch/deadline and the most recent
  success/failure timestamps for each provider, powered by
  `sorafs_node::NodeHandle::por_ingestion_status`, while preserving total
  provider counts; Torii records the
  `torii_sorafs_por_ingest_backlog`/`torii_sorafs_por_ingest_failures_total` gauges for dashboards.【crates/sorafs_node/src/lib.rs:510】【crates/iroha_torii/src/sorafs/api.rs:1883】【crates/iroha_torii/src/routing.rs:7244】【crates/iroha_telemetry/src/metrics.rs:5390】

## M2 Integration Status

The M2 local implementation items are now represented by the runtime and CLI
surfaces below. Remaining SF-3 work is operational hardening: hosted rollout
evidence, governance policy tuning, and SDK management ergonomics.

| Capability | Status | References |
|------------|--------|------------|
| PoR ingestion worker and status endpoint | Implemented locally. | `crates/sorafs_node/src/lib.rs`, `crates/iroha_torii/src/sorafs/api.rs`, `crates/iroha_torii/src/routing.rs`. |
| Challenge queue and replay plumbing | Implemented locally through `PorCoordinatorRuntime` storage interactions and operator replay. | `crates/sorafs_node/src/por.rs`, `crates/sorafs_node/src/bin/sorafs-node.rs`. |
| Governance telemetry | Implemented locally for ingest backlog/failure counters and dashboard export. | `crates/iroha_telemetry/src/metrics.rs`, `docs/source/sorafs_observability_plan.md`. |
| Operator tooling | Implemented locally with `sorafs-node ingest por` and runbook coverage. | `crates/sorafs_node/src/bin/sorafs-node.rs`, `docs/source/sorafs/runbooks/sorafs_node_ops.md`. |

### D. Scheduler & Quota Enforcement

| Task | Details |
|------|---------|
| Disk quota | Track bytes on disk; reject new pins when exceeding `max_capacity_bytes`. Provide eviction hooks for future policies. |
| Fetch concurrency | Global semaphore (`max_parallel_fetches`) plus per-provider budgets sourced from SF-2d range caps. |
| Pin queue | Limit outstanding ingestion jobs; expose Norito status endpoints for queue depth. |
| PoR cadence | Background worker driven by `por_sample_interval_secs`. |

### E. Telemetry & Logging

Metrics (Prometheus):

- `sorafs_pin_success_total`, `sorafs_pin_failure_total`
- `sorafs_chunk_fetch_duration_seconds` (histogram with `result` labels)
- `torii_sorafs_storage_bytes_used`, `torii_sorafs_storage_bytes_capacity`
- `torii_sorafs_storage_pin_queue_depth`, `torii_sorafs_storage_fetch_inflight`
- `torii_sorafs_storage_fetch_bytes_per_sec`
- `torii_sorafs_storage_por_inflight`
- `torii_sorafs_storage_por_samples_success_total`, `torii_sorafs_storage_por_samples_failed_total`

Logs / events:

- Structured Norito telemetry for governance ingestion (`StorageTelemetryV1`).
- Alerts when utilisation > 90% or PoR failure streak exceeds threshold.

### F. Testing Strategy

1. **Unit tests.** Chunk store persistence, quota calculations, scheduler invariants (see `crates/sorafs_node/src/scheduler.rs`).  
2. **Integration tests** (`crates/sorafs_node/tests`). Pin → fetch round trip, restart recovery, quota rejection, PoR sampling proof verification.  
3. **Torii integration tests.** Run Torii with storage enabled, exercise HTTP endpoints via `assert_cmd`.  
4. **Chaos roadmap.** Future drills simulate disk exhaustion, slow IO, provider removal.

## Dependencies

- SF-2b admission policy — ensure nodes verify admission envelopes before advertising.  
- SF-2c capacity marketplace — tie telemetry back into capacity declarations.  
- SF-2d advert extensions — consume range capability + stream budgets once available.

## Milestone Exit Criteria

- `cargo run -p sorafs_node --example pin_fetch` works against local fixtures.  
- Torii builds with `--features sorafs-storage` and passes integration tests.  
- Documentation ([node storage guide](node-storage.md)) updated with configuration defaults + CLI examples; operator runbook available.  
- Telemetry visible in staging dashboards; alerts configured for capacity saturation and PoR failures.

## Documentation & Ops Deliverables

- Update the [node storage reference](node-storage.md) with configuration defaults, CLI usage, and troubleshooting steps.  
- Keep the [node operations runbook](node-operations.md) aligned with the implementation as SF-3 evolves.  
- Keep API references for `/sorafs/*` endpoints wired into the developer portal and OpenAPI manifest as Torii handlers change.
