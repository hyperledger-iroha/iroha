# SoraFS Node V1 Implementation Record (SF-3)

SF-3 defines the canonical `sorafs-node` provider service embedded in the
Iroha/Torii runtime. This record captures the implemented V1 storage contract,
its verification surface, and the genuine deployment evidence that remains.
Use it alongside `sorafs_node_storage.md`, the provider admission policy, and
the capacity marketplace contract.

> **Portal:** Mirrored in `docs/portal/docs/sorafs/node-plan.md`. Update both
> copies to keep reviewers aligned.

## V1 Provider Scope

1. **Chunk store integration**: wrap `sorafs_car::ChunkStore` with a persistent
   backend that stores chunk bytes, manifests, and PoR trees in the configured
   data directory.
2. **Gateway endpoints**: expose Norito HTTP endpoints for pin submission,
   chunk fetch, authenticated proof streaming, and storage telemetry within the
   Torii process.
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
| Implement `StorageConfig` mapped from `SoraFsStorage` (actual/default/user). | Storage Team / Config WG | Ensure Norito/config snapshot parity without production environment overrides. |
| Provide `NodeHandle` read facade for Torii and an internal finalized-ledger ingest worker. | Storage Team | Torii never accepts payload uploads; only the provider outbox may call storage mutation after exact cursor, manifest, and provider-assignment validation. |

### B. Persistent Chunk Store

| Task | Owner(s) | Notes |
|------|----------|-------|
| Persist `sorafs_car::ChunkStore` data through the canonical filesystem backend and Norito manifest index. | Storage Team | Deterministic layout: `<data_dir>/<manifest_cid>/chunk_{idx}.bin`; unsafe path and corrupt-index inputs fail closed. |
| Maintain PoR metadata (64 KiB/4 KiB trees) using existing `ChunkStore::sample_leaves`. | Storage Team | Support resuming after restart. |
| Implement integrity replay on startup (rehash manifest entries, prune incomplete pins). | Storage Team | Fail fast if corruption detected. |

### C. Gateway Endpoints

| Endpoint | Behaviour | Tasks |
|----------|-----------|-------|
| `GET /v1/sorafs/pin`, `POST /v1/sorafs/pin/register`, `GET /v1/sorafs/pin/{digest_hex}` | Read the pin registry, register paid manifest pins, and fetch one exact native manifest record at a finalized cursor. | Validate chunker profiles and canonical manifest payloads before registration; for detail reads return `PinManifestFinalizedRecordV1` and accept only the paired expected finalized height/hash precondition. |
| `POST /v1/sorafs/storage/fetch`, `POST /v1/sorafs/storage/token` | Fetch content ranges and issue storage access tokens. Storage ingest has no public route. | Enforce token policy, provider capability checks, and scheduler/back-pressure limits. Provider bytes arrive only through the durable finalized-ledger outbox. |
| `GET /v1/sorafs/storage/manifest/{manifest_id}`, `GET /v1/sorafs/storage/plan/{manifest_id}`, `GET /v1/sorafs/storage/car/{manifest_id}`, `GET /v1/sorafs/storage/chunk/{manifest_id}/{chunk_digest}` | Serve bounded manifest metadata, deterministic chunk plans, CAR bytes, and individual chunk bytes. | Keep readback arrays bounded while preserving total counts and verify digest/path bindings before streaming bytes. |
| `GET /v1/sorafs/storage/peers`, `GET /v1/sorafs/storage/state`, `POST /v1/sorafs/proof/stream` | Report peer/storage state and request authenticated, finalized-pin-bound PoR witnesses. The unauthenticated local PoR sampling route is retired; proof and verdict admission use the governed capacity lifecycle. | Reuse chunk-store sampling behind the authenticated proof stream, verify generated witnesses against the committed manifest root, update telemetry, and preserve governance-verdict replay state. |

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
- `sorafs_provider_ingest_inflight`, `torii_sorafs_storage_fetch_inflight`.
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
   - Authenticated proof-stream PoR sampling verifies every witness against the
     committed chunk-store root.
3. **Torii integration tests**: run Torii with storage enabled, exercise HTTP endpoints using `assert_cmd`.
4. **Adversarial deployment rehearsal**: simulate disk exhaustion, slow IO,
   restart, and provider removal in the reviewed reference deployment.

### Dependencies

- SF-2b admission policy — nodes verify canonical admission envelopes before advertising.
- SF-2c capacity marketplace — committed capacity state and storage telemetry reconcile at finalized cursors.
- SF-2d advert extensions — range capability and stream budgets are enforced at admission and serving.

### Milestone Exit Criteria

- `cargo test -p sorafs_node --test pin_workflows pin_fetch_roundtrip` passes
  against canonical fixtures.
- Torii exposes the documented `/v1/sorafs/pin*` and
  `/v1/sorafs/storage/*` routes and passes integration tests.
- Documentation (`sorafs_node_storage.md`) updated to match implementation; operator guide drafted.
- Telemetry visible in staging dashboards; alerts configured for capacity saturation and PoR failures.

## M2 Integration Status

The M2 local implementation items are now represented by the runtime and CLI
surfaces below. Remaining SF‑3 work is operational hardening: hosted rollout
evidence, governance policy tuning, and SDK management ergonomics.

| Capability | Status | References |
|------------|--------|------------|
| PoR ingestion worker and status endpoint | Implemented locally. | `crates/sorafs_node/src/lib.rs`, `crates/iroha_torii/src/sorafs/api.rs`, `crates/iroha_torii/src/routing.rs`. |
| Challenge queue and replay plumbing | Implemented locally through `PorCoordinatorRuntime` storage interactions and operator replay. | `crates/sorafs_node/src/por.rs`, `crates/sorafs_node/src/bin/sorafs-node.rs`. |
| Governance telemetry | Implemented locally for ingest backlog/failure counters and dashboard export. | `crates/iroha_telemetry/src/metrics.rs`, `specs/sorafs_observability_plan.md`. |
| Operator tooling | Implemented locally with `sorafs-node ingest por` and runbook coverage. | `crates/sorafs_node/src/bin/sorafs-node.rs`, `specs/sorafs/runbooks/sorafs_node_ops.md`. |

- ✅ `/v1/sorafs/por/ingestion/{manifest_digest_hex}?limit=N` now delegates to
  `sorafs_node::NodeHandle::por_ingestion_status`, returning bounded backlog
  entries, the oldest epoch/deadline, and the most recent success/failure
  timestamps per provider while preserving total provider counts and Torii updates
  `torii_sorafs_por_ingest_backlog`/`torii_sorafs_por_ingest_failures_total` so the dashboards track stalled manifests
  automatically.【crates/sorafs_node/src/lib.rs:510】【crates/iroha_torii/src/sorafs/api.rs:1883】【crates/iroha_torii/src/routing.rs:7244】【crates/iroha_telemetry/src/metrics.rs:5390】
- ✅ `sorafs-node ingest por` now replays PoR challenges, proofs, and optional verdicts against the embedded
  storage worker, emitting JSON summaries so operators can validate artefacts and archive evidence before calling
  the HTTP API. Regression tests cover the new flow and the runbooks/portal docs describe the workflow for SREs
  preparing governance tickets.【crates/sorafs_node/src/bin/sorafs-node.rs:184】【crates/sorafs_node/tests/cli.rs:103】【specs/sorafs/runbooks/sorafs_node_ops.md:57】【docs/portal/docs/sorafs/node-operations.md:59】

These shipped items keep SF‑3 aligned with SF‑9 (PoR automation). Live rollout
evidence and hosted governance archive hand-offs remain tracked in `roadmap.md`.

## Documentation & Ops Deliverables

- Keep `specs/sorafs/sorafs_node_storage.md` aligned with configuration
  defaults and CLI examples.
- Keep the operator runbook
  (`specs/sorafs/runbooks/sorafs_node_ops.md`) aligned with deployment,
  monitoring, and troubleshooting behavior.
- Keep the API reference for the `/v1/sorafs/pin*` and
  `/v1/sorafs/storage/*` endpoints aligned with the OpenAPI manifest.

Local implementation and external-evidence state are tracked in
`specs/sorafs/v1_closure_ledger.md`; this document does not create a
second readiness authority.
