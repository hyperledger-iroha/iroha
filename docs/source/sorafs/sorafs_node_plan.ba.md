---
lang: ba
direction: ltr
source: docs/source/sorafs/sorafs_node_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a42f39287fcb33f421b891f15ac9aef28a57826719ccfb388bd6eaa062a7a199
source_last_modified: "2025-12-29T18:16:36.130900+00:00"
translation_last_reviewed: 2026-02-07
---

# SoraFS Node Prototype Implementation Plan (SF-3)

SF-3 delivers the first runnable `sorafs-node` crate that turns an Iroha/Torii
process into a SoraFS storage provider. This plan translates the high-level
storage design into concrete engineering tasks, milestones, and test coverage.
It should be used alongside `sorafs_node_storage.md`, the provider admission
policy, and the capacity marketplace roadmap.

> **Portal:** Mirrored in `docs/portal/docs/sorafs/node-plan.md`. Update both
> copies to keep reviewers aligned.

## Target Scope (Milestone M1)

1. **Chunk store integration**: wrap `sorafs_car::ChunkStore` with a persistent
   backend that stores chunk bytes, manifests, and PoR trees in the configured
   data directory.
2. **Gateway endpoints**: expose Norito HTTP endpoints for pin submission,
   chunk fetch, PoR sampling, and storage telemetry within the Torii process.
3. **Configuration plumbing**: add `SoraFsStorage` config struct (enabled,
   capacity, directories, concurrency limits) wired through `iroha_config`,
   `iroha_core`, and `iroha_torii`.
4. **Quota/scheduling**: enforce operator-defined disk/parallelism limits and
   queue requests with back-pressure.
5. **Telemetry**: emit metrics/logs for pin success, chunk fetch latency,
   capacity utilisation, PoR sampling results.

## Work Breakdown

### A. Crate & Module Structure

| Task | Owner(s) | Notes |
|------|----------|-------|
| Create `crates/sorafs_node` with modules: `config`, `store`, `gateway`, `scheduler`, `telemetry`. | Storage Team | Re-export reusable types for Torii integration. |
| Implement `StorageConfig` mapped from `SoraFsStorage` (actual/default/user). | Storage Team / Config WG | Ensure serde/Norito parity and environment overrides. |
| Provide `NodeHandle` facade that Torii uses to submit pins/fetches. | Storage Team | Encapsulate storage internals. |

### B. Persistent Chunk Store

| Task | Owner(s) | Notes |
|------|----------|-------|
| Build disk backend wrapping `sorafs_car::ChunkStore` with an on-disk manifest index (sled/sqlite?). | Storage Team | Deterministic layout: `<data_dir>/<manifest_cid>/chunk_{idx}.bin`. |
| Maintain PoR metadata (64 KiB/4 KiB trees) using existing `ChunkStore::sample_leaves`. | Storage Team | Support resuming after restart. |
| Implement integrity replay on startup (rehash manifest entries, prune incomplete pins). | Storage Team | Fail fast if corruption detected. |

### C. Gateway Endpoints

| Endpoint | Behaviour | Tasks |
|----------|-----------|-------|
| `POST /sorafs/pin` | Accept `PinProposalV1`, validate manifests, queue ingestion, respond with manifest CID. | Validate chunk profile, enforce quotas, stream data via chunk store. |
| `GET /sorafs/chunks/{cid}` + range query | Serve chunk bytes with `Content-Chunker` headers; respect range capability spec. | Use scheduler + stream budgets (tie-in to SF-2d). |
| `POST /sorafs/por/sample` | Run PoR sampling for a manifest and return proof bundle. | Reuse chunk store sampling, respond with Norito JSON. |
| `GET /sorafs/telemetry` | Summaries: capacity, PoR success, fetch error counts. | Provide data for dashboards/operators. |

The runtime now threads these PoR interactions through `sorafs_node::por`: the tracker records every `PorChallengeV1`, `PorProofV1`, and `AuditVerdictV1` so the `CapacityMeter` and scheduler metrics reflect governance verdicts without bespoke plumbing in Torii.

Implementation Notes:
- Use Axum (Torii’s stack) with `norito::json` for payloads.
- Add Norito schemas for responses (e.g., `PinResultV1`, `FetchErrorV1`).

### D. Scheduler & Quota Enforcement

| Task | Details |
|------|---------|
| Disk quota | Track bytes on disk; reject new pins when exceeding `max_capacity_bytes`. Provide eviction hooks for future policies. |
| Fetch concurrency | Global semaphore (`max_parallel_fetches`) + per-provider budgets (from SF-2d). |
| Pin queue | Limit outstanding ingestion jobs; provide Norito status endpoint. |
| PoR cadence | Background worker triggered by `por_sample_interval_secs`. |

### E. Telemetry & Logging

Metrics (Prometheus):
- `sorafs_pin_success_total`, `sorafs_pin_failure_total`.
- `sorafs_chunk_fetch_duration_seconds` (histogram with labels `result`).
- `torii_sorafs_storage_bytes_used`, `torii_sorafs_storage_bytes_capacity`.
- `torii_sorafs_storage_pin_queue_depth`, `torii_sorafs_storage_fetch_inflight`.
- `torii_sorafs_storage_fetch_bytes_per_sec`.
- `torii_sorafs_storage_por_inflight`.
- `torii_sorafs_storage_por_samples_success_total`, `torii_sorafs_storage_por_samples_failed_total`.

The initial runtime implementation now backs these gauges via
`StorageSchedulersRuntime`, which enforces the pin/fetch/PoR concurrency
budgets and aggregates throughput/queue statistics for Torii to expose via
Prometheus.【crates/sorafs_node/src/scheduler.rs:147】

Logs / events:
- Structured Norito telemetry for governance ingestion (`StorageTelemetryV1`).
- Governance-enforced capacity telemetry ingress (authorised submitters + nonces) that caps windows to declared capacity, rejects zero-capacity payloads, enforces monotonic windows with bounded gaps/replay guards, and emits rejection metrics before fee/strike handling.【crates/iroha_core/src/smartcontracts/isi/sorafs.rs】【crates/iroha_config/src/parameters/{actual,user}.rs】【crates/iroha_telemetry/src/metrics.rs】
- Alerts when utilisation > 90% or PoR failure streak exceeds threshold.

### F. Testing Strategy

1. **Unit tests**: chunk store persistence, quota calculations, scheduler (see
   `crates/sorafs_node/src/scheduler.rs` for queue and rate-limit coverage).
2. **Integration tests** (new `crates/sorafs_node/tests`):
   - Pin → fetch round trip using fixture manifest/plan.
   - Restart recovery: pin, restart, verify manifest registry.
   - Quota rejection: set low capacity, attempt additional pin.
   - PoR sampling endpoint verifying proof matches chunk store root.
3. **Torii integration tests**: run Torii with storage enabled, exercise HTTP endpoints using `assert_cmd`.
4. **Chaos tests (future)**: simulate disk exhaustion, slow IO, provider removal (tracked in later milestones).

### Dependencies

- SF-2b admission policy (provider verifier) — ensure node checks admission envelopes before advertising.
- SF-2c capacity marketplace — later tie storage telemetry into capacity declarations.
- SF-2d advert extensions — consume range capability + stream budgets once available.

### Milestone Exit Criteria

- `cargo run -p sorafs_node --example pin_fetch` works against local fixtures.
- Torii builds with `--features sorafs-storage` and passes integration tests.
- Documentation (`sorafs_node_storage.md`) updated to match implementation; operator guide drafted.
- Telemetry visible in staging dashboards; alerts configured for capacity saturation and PoR failures.

## Remaining integration tasks (M2 focus)

| Task | Description | Dependencies |
|------|-------------|--------------|
| PoR ingestion worker | Expand `NodeHandle::ingest_por_proof` to accept streamed proofs, persist them under `PorCoordinatorRuntime::storage`, and expose a Norito status endpoint (`/v2/sorafs/por/ingestion`). | `crates/iroha_torii/src/sorafs/por.rs`, `crates/sorafs_node/src/por.rs`. |
| Challenge queue plumbing | Subscribe to the coordinator events emitted by `PorCoordinatorRuntime::run_epoch`, fan out challenges to storage workers, and ensure retries are idempotent across restarts. | Runtime wiring hooks introduced in `PorCoordinatorRuntime`. |
| Governance telemetry | Emit `sorafs_por_ingest_backlog` + `sorafs_por_ingest_failures_total` metrics, thread them into the gateway dashboards, and document alert thresholds in `docs/source/sorafs_observability_plan.md`. | Observability plan + `crates/iroha_telemetry`. |
| Operator tooling | Add a `sorafs-node ingest por --manifest <cid>` helper plus runbook updates so operators can replay proofs locally before submitting. | CLI additions under `crates/sorafs_node/src/bin/sorafs-node.rs`. |

- ✅ `/v2/sorafs/por/ingestion/{manifest_digest_hex}` now delegates to
  `sorafs_node::NodeHandle::por_ingestion_status`, returning backlog depth, the oldest epoch/deadline, and the
  most recent success/failure timestamps per provider while Torii updates
  `torii_sorafs_por_ingest_backlog`/`torii_sorafs_por_ingest_failures_total` so the dashboards track stalled manifests
  automatically.【crates/sorafs_node/src/lib.rs:510】【crates/iroha_torii/src/sorafs/api.rs:1883】【crates/iroha_torii/src/routing.rs:7244】【crates/iroha_telemetry/src/metrics.rs:5390】
- ✅ `sorafs-node ingest por` now replays PoR challenges, proofs, and optional verdicts against the embedded
  storage worker, emitting JSON summaries so operators can validate artefacts and archive evidence before calling
  the HTTP API. Regression tests cover the new flow and the runbooks/portal docs describe the workflow for SREs
  preparing governance tickets.【crates/sorafs_node/src/bin/sorafs-node.rs:184】【crates/sorafs_node/tests/cli.rs:103】【docs/source/sorafs/runbooks/sorafs_node_ops.md:57】【docs/portal/docs/sorafs/node-operations.md:59】

These items keep SF‑3 aligned with SF‑9 (PoR automation) and are being tracked in
`roadmap.md` as part of the “deliver SoraFS tasks” call-out for March.

## Documentation & Ops Deliverables

- Update `docs/source/sorafs/sorafs_node_storage.md` with configuration defaults, CLI examples.
- Create operator runbook (`docs/source/sorafs/runbooks/sorafs_node_ops.md`) covering deployment, monitoring, troubleshooting.
- Publish API reference for the new endpoints under the docs portal.

Progress should be reflected in the roadmap by checking off the items in the SF-3 section as features land.
