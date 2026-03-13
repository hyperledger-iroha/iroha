---
lang: uz
direction: ltr
source: docs/source/sorafs/storage_capacity_marketplace.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2f830d4a4549c49d0c9649b728ab6ec00b0b7bcfcbd5a50230686ff0cd0648d7
source_last_modified: "2026-01-22T14:35:37.719643+00:00"
translation_last_reviewed: 2026-02-07
---

# SoraFS Storage Capacity Marketplace (SF-2c Draft)

The SF-2c roadmap item introduces a governed marketplace where storage
providers declare committed capacity, receive replication orders, and earn fees
proportional to delivered availability. This document scopes the deliverables
required for the first release and breaks them into actionable tracks.

## Objectives

- Express provider capacity commitments (total bytes, per-lane limits, expiry)
  in a verifiable form consumable by governance, SoraNet transport, and Torii.
- Allocate pins across providers according to declared capacity, stake, and
  policy constraints while maintaining deterministic behaviour.
- Meter storage delivery (replication success, uptime, integrity proofs) and
  export telemetry for fee distribution.
- Provide revocation and dispute processes so dishonest providers can be
  penalised or removed.

## Domain Concepts

| Concept | Description | Initial Deliverable |
|---------|-------------|---------------------|
| `CapacityDeclarationV1` | Norito payload describing provider ID, chunker profile support, committed GiB, lane-specific limits, pricing hints, staking commitment, and expiry. | Schema + validator in `sorafs_manifest::capacity`. |
| `ReplicationOrder` | Governance-issued instruction assigning a manifest CID to one or more providers, including redundancy level and SLA metrics. | Norito schema shared with Torii + smart contract API. |
| `CapacityLedger` | On-chain/off-chain registry tracking active capacity declarations, replication orders, performance metrics, and fee accrual. | Smart contract module or off-chain service stub with deterministic snapshot. |
| `MarketplacePolicy` | Governance policy defining minimum stake, audit requirements, and penalty curves. | Config struct in `sorafs_manifest` + governance document. |

### Implemented Schemas (Status)

## Work Breakdown

### 1. Schema & Registry Layer

| Task | Owner(s) | Notes |
|------|----------|-------|
| Define `CapacityDeclarationV1`, `ReplicationOrderV1`, `CapacityTelemetryV1`. | Storage Team / Governance | Use Norito; include semantic versioning and capability references. |
| Implement parser + validator modules in `sorafs_manifest`. | Storage Team | Enforce monotonic IDs, capacity bounds, stake requirements. |
| Extend chunker registry metadata with `min_capacity_gib` per profile. | Tooling WG | Helps clients enforce per-profile minimum hardware requirements. |
| Draft `MarketplacePolicy` document capturing admission guardrails and penalty schedule. | Governance Council | Publish in docs alongside policy defaults. |

#### Schema Definitions (Implemented)

- `CapacityDeclarationV1` captures signed capacity commitments per provider, including canonical chunker handles, capability references, optional lane caps, pricing hints, validity windows, and metadata. Validation ensures non-zero stake, canonical handles, deduplicated aliases, per-lane caps within the declared total, and monotonic GiB accounting.【crates/sorafs_manifest/src/capacity.rs:28】
- `ReplicationOrderV1` binds manifests to governance-issued assignments with redundancy targets, SLA thresholds, and per-assignment guarantees; validators enforce canonical chunker handles, unique providers, and deadline constraints before Torii or the registry ingest the order.【crates/sorafs_manifest/src/capacity.rs:301】
- `CapacityTelemetryV1` expresses epoch snapshots (declared vs utilised GiB, replication counters, uptime/PoR percentages) that feed fee distribution. Bounds checks keep utilisation within declarations and percentages within 0 – 100 %.【crates/sorafs_manifest/src/capacity.rs:476】
- Shared helpers (`CapacityMetadataEntry`, `PricingScheduleV1`, lane/assignment/SLA validators) provide deterministic key validation and error reporting that CI and downstream tooling can reuse.【crates/sorafs_manifest/src/capacity.rs:230】
- `PinProviderRegistry` now surfaces the on-chain snapshot via `/v2/sorafs/capacity/state`, combining provider declarations and fee ledger entries behind deterministic Norito JSON.【crates/iroha_torii/src/sorafs/registry.rs:17】【crates/iroha_torii/src/sorafs/api.rs:64】
- Validation coverage exercises canonical handle enforcement, duplicate detection, per-lane bounds, replication assignment guards, and telemetry range checks so regressions surface immediately in CI.【crates/sorafs_manifest/src/capacity.rs:792】
- Operator tooling: `sorafs_manifest_stub capacity {declaration, telemetry, replication-order}` converts human-readable specs into canonical Norito payloads, base64 blobs, and JSON summaries so operators can stage `/v2/sorafs/capacity/declare`, `/v2/sorafs/capacity/telemetry`, and replication order fixtures with local validation.【crates/sorafs_car/src/bin/sorafs_manifest_stub/capacity.rs:1】 Reference fixtures live in `fixtures/sorafs_manifest/replication_order/` (`order_v1.json`, `order_v1.to`) and are generated via `cargo run -p sorafs_manifest --bin generate_replication_order_fixture`.

### 2. Smart Contract / Control Plane

| Task | Owner(s) | Notes |
|------|----------|-------|
| Prototype registry contract (`PinProviderRegistry`) with CRUD for capacity declarations and replication orders. | Core Infra / Smart Contract Team | Ensure deterministic hashing and Norito encoding parity. |
| Expose gRPC/REST service (`/v2/sorafs/capacity`) mirroring contract state for Torii/gateways. | Core Infra | Provide pagination + attestation (block hash, proof). |
| Implement fee accrual ledger with basic rate card (GiB · hour * price). | Economics WG / Core Infra | Export ledger snapshots for billing integration. |
| Add dispute/resolution hooks (challenge window, evidence submission). | Governance Council | Determine default timeouts and penalties. |

### 3. Torii & SoraFS Node Integration

| Task | Owner(s) | Notes |
|------|----------|-------|
| Torii: ingest `CapacityDeclarationV1` and expose via discovery API. | Networking TL | Align with existing provider advert flows. |
| `sorafs-node`: persist replication assignments, schedule downloads, enforce per-provider quotas. | Storage Team | Build on top of multi-source fetch orchestrator. |
| CLI updates: `sorafs_manifest_stub capacity {declaration, telemetry, replication-order}`, `sorafs_fetch --capacity-plan`. | Tooling WG | Provide JSON reports for operators. |
| Telemetry: publish `capacity_commitment_bytes`, `capacity_utilisation_percent`, `replication_order_backlog`. | Observability | Feed dashboards + alerts. |

- Torii app API now accepts capacity registry submissions via dedicated endpoints:
  - `POST /v2/sorafs/capacity/declare` wraps a signed `CapacityDeclarationV1` and queues the
    corresponding `RegisterCapacityDeclaration` instruction.【crates/iroha_torii/src/routing.rs:4390】【crates/iroha_torii/src/lib.rs:3175】
  - `POST /v2/sorafs/capacity/telemetry` records per-epoch utilisation snapshots through
    `RecordCapacityTelemetry`, enforcing sanity bounds before dispatch.【crates/iroha_torii/src/routing.rs:4744】【crates/iroha_torii/src/lib.rs:3248】
- Telemetry payloads now include PDP/PoTR counters so governance can correlate proof failures with
  billing and enforcement. Submitters must provide `pdp_challenges`/`pdp_failures` and
  `potr_windows`/`potr_breaches`; Torii validates that failures never exceed the total window and
  surfaces descriptive errors when probes are missing. These counters feed the new
  `SorafsPenaltyPolicy.max_pdp_failures` and `.max_potr_breaches` knobs so any proof failure can
  trigger an immediate strike/slash without waiting for utilisation/uptime caps.
- Proof failure governance evidence is now emitted automatically. Whenever `RecordCapacityTelemetry`
  receives a snapshot whose PDP or PoTR counters exceed the configured limits, the runtime files a
  `proof_failure` `CapacityDisputeRecord` with a canonical `CapacityDisputeV1` payload. The evidence
  digest references the Norito-encoded telemetry payload (retrievable via the URI
  `norito://sorafs/capacity_telemetry/<provider_hex>/<start_epoch>-<end_epoch>`), so governance,
  Taikai/CDN reviewers, and auditors can fetch the exact snapshot that triggered the strike without
  relying on ad hoc bundles.
- `POST /v2/sorafs/capacity/schedule` allows operators to submit governance-issued `ReplicationOrderV1`
  payloads; the embedded `sorafs-node` manager validates the order, tracks outstanding assignments,
  and returns a scheduling summary with remaining capacity so orchestration tooling can act on the
  result. `POST /v2/sorafs/capacity/complete` releases reservations once ingestion finishes, feeding
  release telemetry back into local capacity snapshots. The node seeds a `TelemetryAccumulator`
  alongside the scheduler so operators (or background workers) can derive canonical
  `CapacityTelemetryV1` payloads capturing GiB·hour, uptime, and PoR success metrics before posting
  through Torii.【crates/iroha_torii/src/routing.rs:4806】【crates/sorafs_node/src/lib.rs:110】【crates/sorafs_node/src/telemetry.rs:1】
- Local metering now surfaces dedicated observation endpoints. `POST /v2/sorafs/capacity/uptime`,
  `POST /v2/sorafs/capacity/por`, and `POST /v2/sorafs/capacity/failure` update the embedded
  `CapacityMeter`, telemetry accumulator, and Prometheus gauges without issuing transactions,
  ensuring probe data and replication failures feed dashboards and fee accrual logic immediately.【crates/iroha_torii/src/routing.rs:5023】【crates/iroha_torii/src/lib.rs:5301】
- The trustless gateway profile draft enumerates the HTTP request/response matrix, proof formats, and
  telemetry expectations that gateways must satisfy before joining the SF-5 conformance suite. See
  `docs/source/sorafs_gateway_profile.md` for the normative specification.
- `GET /v2/sorafs/capacity/state` now includes a `local_usage` projection that reports the node’s
  committed/allocated GiB, per-chunker reserves, lane utilisation, outstanding orders, and live
  metering counters (GiB·hour, uptime/PoR samples, replication counts) sourced from the embedded
  meter. A `telemetry_preview` payload mirrors the canonical `CapacityTelemetryV1` submission so
  operators can compare dashboard values against the Norito snapshot before broadcasting new
  adverts.【crates/iroha_torii/src/sorafs/api.rs:144】
- Credit ledgers are exported alongside fee ledgers in the same response. Each entry reports
  available credit, bonded collateral, strike counters, penalty totals, and low-balance timestamps so
  treasury automation and dashboards can gate payouts before settlement windows close.【crates/iroha_torii/src/sorafs/registry.rs:123】【crates/iroha_torii/src/sorafs/api.rs:5096】
- Capacity disputes are first-class in the capacity registry: `/v2/sorafs/capacity/state`
  now emits a `disputes` array (with base64 payloads, evidence digests, and status metadata) while
  `/v2/sorafs/capacity/dispute` accepts governance-signed submissions. Use the CLI helper to craft
  requests and note the response’s `dispute_id_hex` for revocation and audit tracking.【crates/iroha_torii/src/sorafs/api.rs:520】【crates/iroha_torii/src/routing.rs:4889】【docs/source/sorafs/dispute_revocation_runbook.md:45】
- `sorafs_manifest_stub capacity dispute` accepts a declarative spec when filing governance disputes.
  Required fields: `provider_id_hex`, `complainant_id_hex`, `kind` (`replication_shortfall`, `uptime_breach`,
  `proof_failure`, `fee_dispute`, or `other`), `submitted_epoch`, `description`, and an `evidence` object with
  `digest_hex` (BLAKE3-256). Optional fields include `replication_order_id_hex`, `requested_remedy`, `evidence.media_type`,
  `evidence.uri`, and `evidence.size_bytes`. The CLI emits canonical Norito bytes, base64 payloads, and a Torii-ready
  request body so operators can lodge disputes or archive evidence deterministically. See
  `docs/source/sorafs/dispute_revocation_runbook.md` for the end-to-end governance playbook.【crates/sorafs_car/src/bin/sorafs_manifest_stub/capacity.rs:35】
- `sorafs_manifest_stub capacity {declaration, telemetry, replication-order, complete}` gained `--request-out`
  helpers (with `--authority`/`--private-key` for declarations and telemetry) so operators can emit
  ready-to-post JSON payloads for the Torii endpoints without hand-assembling request
  bodies.【crates/sorafs_car/src/bin/sorafs_manifest_stub/capacity.rs:20】

### 4. Metering & Fee Distribution

| Task | Owner(s) | Notes |
|------|----------|-------|
| Define proof types (PoR success, uptime intervals, ticket acknowledgements). | Storage Team / Observability | Reuse existing PoR tree metadata. |
| Build metering pipeline that aggregates per-provider metrics per epoch. | Observability / Economics WG | Output JSON snapshots consumed by billing. |
| Implement reward calculation (baseline share + performance multiplier). | Economics WG | Document formulas; ensure deterministic rounding. |
| Governance workflow for payout approval and penalty enforcement. | Governance Council | Provide CLI + docs for treasury review. |

- `sorafs_node` now ships with a lightweight `CapacityMeter` that tracks scheduled/completed
  orders, declared GiB, and outstanding slices so telemetry windows can be populated directly
  from the embedded worker without re-deriving utilisation off-chain.【crates/sorafs_node/src/metering.rs:1】【crates/sorafs_node/src/lib.rs:27】
- `/v2/sorafs/capacity/state` now emits a deterministic `fee_projection` payload alongside the live
  `metering` snapshot, and Prometheus exports feed the ready-to-import Grafana board at
  `docs/source/grafana_sorafs_metering.json` so billing teams can monitor GiB·hour accrual,
  projected nano-SORA fees, and SLA compliance in real time.【crates/iroha_torii/src/sorafs/api.rs:268】【crates/iroha_torii/src/routing.rs:5143】【docs/source/grafana_sorafs_metering.json:1】
- When metering smoothing is enabled, the snapshot includes `smoothed_gib_hours` and
  `smoothed_por_success_bps` so operators can compare EMA-trended values against the raw
  counters that governance uses for payouts.【crates/sorafs_node/src/metering.rs:401】【crates/iroha_torii/src/sorafs/api.rs:816】

### 5. Dispute & Revocation Handling

| Task | Owner(s) | Notes |
|------|----------|-------|
| Define `CapacityDisputeV1` payload (complainant, evidence, target provider). | Governance Council | Norito schema + validator. |
| CLI support to file disputes and respond (with evidence attachments). | Tooling WG | Ensure deterministic hashing of evidence bundle. |
| Add automated checks for repeated SLA breaches (auto-escalate to dispute). | Observability | Alert thresholds and governance hooks. |
| Document revocation playbook (grace period, evacuation of pinned data). | Docs / Storage Team | Link to policy doc and operator runbook. |

## Testing & CI Requirements

- Unit tests for all new schema validators (`sorafs_manifest`).
- Integration tests that simulate: declaration → replication order → metering → payout.
- CI workflow to regenerate sample capacity declarations/telemetry and ensure signatures remain in sync (extend `ci/check_sorafs_fixtures.sh`).
- Load tests for the registry API (simulate 10k providers, 100k orders).

## Telemetry & Dashboards

- Dashboard panels:
  - Capacity declared vs utilised per provider.
  - Replication order backlog and average assignment delay.
  - SLA compliance (uptime %, PoR success rate).
  - Fee accrual and penalties per epoch.
- Alerts:
  - Provider below minimum committed capacity.
  - Replication order stuck > SLA.
  - Metering pipeline failures.

## Documentation Deliverables

- Operator guide for declaring capacity, renewing commitments, and monitoring utilisation.
- Governance guide for approving declarations, issuing orders, handling disputes.
- API reference for the capacity endpoints and replication order format.
- Marketplace FAQ for developers.
- Provider onboarding & exit runbook covering artefact storage, role-based approvals,
  and Torii verification commands (`docs/source/sorafs/capacity_onboarding_runbook.md`).

## GA Readiness Checklist

Roadmap item **SF-2c** gates production rollout on concrete evidence across accounting,
dispute handling, and onboarding. Use the artefacts below to keep the acceptance criteria
in-sync with the implementation.

### Nightly accounting & XOR reconciliation

1. Export the XOR transfers that treasury executed during the previous window and the
   matching `/v2/sorafs/capacity/state` snapshot. Reconcile them with the new helper:
   ```bash
   python3 scripts/telemetry/capacity_reconcile.py \
     --snapshot artifacts/sorafs/capacity/state_$(date +%F).json \
     --ledger /var/lib/iroha/exports/sorafs-capacity-ledger-$(date +%F).ndjson \
     --label nightly-capacity \
     --json-out artifacts/sorafs/capacity/reconcile_$(date +%F).json \
     --prom-out "${SORAFS_CAPACITY_RECONCILE_TEXTFILE:-artifacts/sorafs/capacity/reconcile.prom}"
   ```
   The script exits non-zero when settlements/penalties are missing or overpaid and emits a
   Prometheus textfile mirroring the JSON summary so governance can replay evidence.
2. Triangulate the reconciliation output with `dashboards/grafana/sorafs_capacity_penalties.json`
   and Alertmanager rule `SoraFSCapacityReconciliationMismatch`
   (`dashboards/alerts/sorafs_capacity_rules.yml`). The rule fires whenever reconciliation
   metrics report missing/overpaid settlements or unexpected provider transfers.
3. Archive the nightly reconciliation digest alongside the governance packets in
   `docs/examples/sorafs_capacity_marketplace_validation/` so treasury approvals can
   reference deterministic evidence. The validation checklist in
   `docs/source/sorafs/reports/capacity_marketplace_validation.md` links to the latest
   sign-off bundle—extend that table when new exports are reviewed.

### Dispute & slashing evidence

1. Generate dispute payloads with the CLI harness
   (`sorafs_manifest_stub capacity dispute` and
   `cargo test -p sorafs_car --test capacity_cli`) so every `CapacityDisputeV1` bundle has
   canonical JSON/Norito artefacts.
2. Exercise the deterministic ledger hooks by running
   `cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic` plus the penalty
   suites the roadmap references (`record_capacity_telemetry_penalises_persistent_under_delivery`).
   These tests live in `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` and guarantee
   that disputes, penalties, and slashes replay faithfully across nodes.
3. Follow the escalation workflow in `docs/source/sorafs/dispute_revocation_runbook.md`—
   the runbook covers evidence capture, council ballots, and revocation/failover procedures.
   Link the resulting meeting minutes or strike approvals back into
   `docs/source/sorafs/reports/capacity_marketplace_validation.md` so reviewers can trace
   individual disputes from CLI payload → ledger mutation → governance approval.

### Provider onboarding & exit smoke tests

1. Stage declarations with `sorafs_manifest_stub capacity declaration --spec <file>` and
   replay the CLI regression (`cargo test -p sorafs_car --test capacity_cli -- capacity_declaration`)
   before handing submissions to Torii. The helper emits Norito `.to`, JSON, and Base64
   outputs plus the chunk-plan metadata described earlier in this guide.
2. Verify Torii behaviour by calling `POST /v2/sorafs/capacity/declare` and
   `GET /v2/sorafs/capacity/state`—records should match the governance defaults captured in
   `docs/source/sorafs/provider_admission_policy.md` and the runbook at
   `docs/source/sorafs/runbooks/multi_source_rollout.md`.
3. Exercise the exit path once per release candidate:
   - Drain pending assignments by replaying `iroha app sorafs replication list --status pending`
     (filters live in `crates/iroha/src/client.rs:294`) and queuing reassignment ballots for
     any manifest digests that still reference the retiring provider.
   - Use `sorafs_manifest_stub capacity telemetry` to publish the final telemetry snapshot
     and confirm Torii marks the provider inactive.
   - Capture the operator checklist (access revocation, manifest cache cleanup, treasury
     sign-off) in `docs/source/sorafs/runbooks/sorafs_node_ops.md` and update the validation
     evidence bundle with the exit ticket ID.
4. Archive the signed artefact bundle described in
   `docs/source/sorafs/capacity_onboarding_runbook.md` so governance, treasury, and SRE
   reviewers can trace the onboarding/exit through a single runbook entry.

Completing the steps above provides the auditors referenced in roadmap item SF-2c with the
same artefacts the acceptance checklist expects—nightly XOR reconciliations, deterministic
dispute handling, and repeatable onboarding/exit smoke tests.

## Dependencies & Sequencing

1. Finish SF-2b (admission policy) — marketplace relies on vetted providers.
2. Implement schema + registry layer (this doc) before Torii integration.
3. Complete metering pipeline before enabling payouts.
4. Final step: enable governance-controlled fee distribution once metering data is verified in staging.

Progress should be tracked in the roadmap with references to this document. Update the roadmap once each major section (schema, control plane, integration, metering, dispute handling) reaches feature complete status.
