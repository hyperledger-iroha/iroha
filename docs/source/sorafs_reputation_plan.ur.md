---
lang: ur
direction: rtl
source: docs/source/sorafs_reputation_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4bcf9edfbf099f81a4459701b72dd9a44c87e9c3ea41abd22edb7041ce212bde
source_last_modified: "2026-01-03T18:07:58.449466+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: SoraFS Provider Reputation Oracle
summary: Final specification for SFM-3 covering scoring methodology, ingestion, publication, APIs, observability, and rollout.
---

# SoraFS Provider Reputation Oracle

## Goals & Scope
- Produce deterministic, governance-auditable reputation scores for each SoraFS provider to inform routing, incentives, staking, and compliance decisions.
- Combine operational metrics (PoR, PDP, PoTR, latency, disputes, settlement breaches) into a single score published weekly, with daily incremental updates.
- Provide Merkle-verifiable snapshots, public APIs, and SDK tooling, while ensuring privacy and resilience.

This specification completes **SFM-3 — Provider Reputation Oracle** and supersedes earlier drafts.

## Architecture Overview
| Component | Responsibility | Notes |
|-----------|----------------|-------|
| Metrics ingest pipeline (`reputation_ingest`) | Streams PoR/PDP/PoTR verdicts, settlement logs, disputes, token violations from Governance DAG + telemetry exporters. | Validates payload signatures, persists raw events. |
| Scoring engine (`reputation_engine`) | Aggregates metrics, runs scoring algorithm (EigenTrust-style), applies policy penalties, generates snapshots. | Runs hourly; writes outputs to database + object storage. |
| Snapshot publisher (`reputation_publisher`) | Builds Merkle tree, updates Governance DAG, pushes snapshots to IPFS/S3, broadcasts Torii events. | Weekly full snapshot + daily incremental diff. |
| API gateway (`sorafs_reputation_api`) | Exposes REST/GraphQL endpoints, WebSocket updates, CLI hooks. | Deployed regionally; uses caching with ETag. |
| CLI/SDK modules | `sorafs reputation` commands; SDK helper functions for verification and weighting. | Integrates with orchestrator, indexer, orderbook, incentives. |

### Data Flow
1. Governance DAG emits proof/verdict nodes (PoR/PDP/PoTR), repair events, settlement receipts, dispute outcomes.
2. `reputation_ingest` fetches blocks, validates signatures, normalises metrics into canonical tables (`raw_por`, `raw_pdp`, `raw_potr`, `raw_latency`, `raw_disputes`, `raw_tokens`).
3. `reputation_engine` processes metrics, calculates rolling windows, applies weights/penalties, and runs EigenTrust iteration.
4. Engine writes `reputation_scores` (current + historical) and supporting metadata to PostgreSQL/TimescaleDB.
5. Publisher builds Merkle tree over provider entries, stores snapshot in S3/IPFS, records root in Governance DAG (`ReputationSnapshotNode`).
6. API + CLI serve latest scores with cryptographic proofs; downstream systems fetch scores and update routing/incentive logic.

## Data Sources & Normalisation
- **PoR/PDP/PoTR**: success ratios computed over rolling 24h, 72h, 7d windows. Success defined as `verified` verdicts / total challenges.
- **Latency**: P95 latency from PoTR receipts (hot/warm tiers). Normalise to `[0,1]` by mapping 0 ms→1.0, 90 s→0 (hot) / 5 min→0 (warm).
- **Disputes**: count governance disputes resolved against provider per 1k orders.
- **Token violations**: rate of throttle breaches, unauthorized access attempts (from gateway telemetry).
- **Repair escalations**: number of PDP/PoR repair escalations not resolved within SLA.
- **Stake & Reserve status**: Reserve+Rent lifecycle stage (Active/Warning/Grace/Delinquent/Default) influences multipliers.
- All events stored in canonical Norito form; ingestion uses `sorafs_manifest` to decode.

Schema (PostgreSQL):
```sql
CREATE TABLE provider_metrics (
    provider_id TEXT,
    period_start TIMESTAMPTZ,
    period_end TIMESTAMPTZ,
    metric_kind TEXT,
    metric_value DOUBLE PRECISION,
    metadata JSONB,
    PRIMARY KEY (provider_id, period_start, metric_kind)
);
CREATE TABLE provider_scores (
    provider_id TEXT PRIMARY KEY,
    score NUMERIC(6,5),
    calculated_at TIMESTAMPTZ,
    degradation_flags TEXT[],
    details JSONB,
    snapshot_id UUID
);
CREATE TABLE reputation_snapshots (
    snapshot_id UUID PRIMARY KEY,
    generated_at TIMESTAMPTZ,
    alpha NUMERIC(4,2),
    weights JSONB,
    merkle_root BYTEA,
    cid TEXT,
    storage_uri TEXT
);
```

## Scoring Algorithm
- **Base scores** per provider `i`:
  ```
  s_i = w_por * success_por_i +
        w_pdp * success_pdp_i +
        w_potr * success_potr_i +
        w_latency * latency_factor_i -
        w_dispute * dispute_rate_i -
        w_token * token_violation_i -
        w_repair * repair_breach_rate_i
  ```
  Default weights: `w_por=0.22`, `w_pdp=0.20`, `w_potr=0.18`, `w_latency=0.15`, `w_dispute=0.10`, `w_token=0.05`, `w_repair=0.10`. Governance can update via `ReputationConfigUpdateV1`.
- **EigenTrust iteration**:
  ```
  R = α * C * R + (1 - α) * t
  ```
  where `α = 0.85`, `t` baseline trust vector derived from stake weight + historical reliability, `C` pairwise trust matrix built from settlement satisfaction (buyer feedback). Converges when `||R_{k+1} - R_k||_1 < 1e-6` or `k=100`.
- **Degradation penalties**:
  - Reserve lifecycle `Warning`, `Grace`, `Delinquent` multiply by `[0.9, 0.75, 0.5]`.
  - PoR/PDP success <90% 7-day → ×0.8; <80% → ×0.6.
  - Active dispute or slashing event sets `degradation_flag = "probation"` and clamps score ≤0.20.
- **Smoothing**: `R_final = 0.7 * R_current + 0.3 * R_prev`.
- **Bounds**: `0.05 ≤ R_final ≤ 0.99`. Providers below 0.15 flagged.
- **Transparency**: `details` JSON field includes metrics, weights, penalties applied.

## Publication & Verification
- Weekly snapshot (Monday 00:00 UTC) produced as `ReputationSnapshotV1`:
  ```norito
  struct ReputationSnapshotV1 {
      version: u8,
      snapshot_id: Uuid,
      generated_at: Timestamp,
      alpha: Decimal64,
      weights: ReputationWeightsV1,
      providers: Vec<ProviderReputationV1>,
      merkle_root: Digest32,
      previous_snapshot_id: Option<Uuid>,
  }
  struct ProviderReputationV1 {
      provider_id: ProviderId,
      score: Decimal64,
      degradation_flags: Vec<DegradationFlagV1>,
      raw_metrics: ProviderMetricsV1,
  }
  ```
- Merkle tree built over `H(provider_id || score || degradation_flags || raw_metrics_hash)`; leaves sorted lexicographically by provider ID.
- Snapshot stored in S3 (`s3://sorafs-reputation/<snapshot_id>.json`) and pinned to IPFS; root recorded in Governance DAG `ReputationSnapshotNode`.
- Daily incremental diffs (`ReputationDeltaV1`) capturing score deltas and new flags; clients can apply to previous snapshot.
- Torii broadcasts `ReputationSnapshotEvent` with `snapshot_id`, `merkle_root`, `generated_at`.
- CLI `sorafs reputation verify --snapshot <file> --root <cid>` replays Merkle proof.

## APIs & SDK
- REST endpoints:
  - `GET /v1/reputation/latest` → full snapshot metadata + provider scores.
  - `GET /v1/reputation/{provider_id}` → provider entry with Merkle proof.
  - `GET /v1/reputation/snapshots/{snapshot_id}` → historical snapshot.
  - `GET /v1/reputation/events?since=` → list incremental updates.
  - `GET /v1/reputation/weights` → current configuration.
- WebSocket `/ws/reputation` broadcasting score/delta events.
- SDK helpers:
  - Rust: `ReputationClient::latest()`, `::provider(provider_id)`, `verify_provider_record`.
  - JS/TS: `client.reputation.fetch({providerId})`.
  - Python: `client.reputation.get_provider(provider_id, verify=True)`.
- CLI commands:
  - `sorafs reputation fetch --provider <id> --format table|json`.
  - `sorafs reputation snapshot --output snapshot.json`.
  - `sorafs reputation verify --snapshot snapshot.json --proof proof.json`.
  - `sorafs reputation watch --threshold 0.2`.

## Integration Points
- **Routing/Indexer**: Use scores as weights when ranking providers; degrade selection of low-score providers.
- **Orderbook**: Apply penalties or restrict order volume for providers below threshold.
- **Reserve+Rent**: Feed stage changes into degradation pipeline; send alerts to operations.
- **Governance**: DAG stores snapshots; slashing proposals reference reputation history.
- **Transparency dashboards**: Display scores, trend lines, and flags.

## Observability & Alerts
- Metrics:
  - `sorafs_reputation_ingest_lag_seconds`
  - `sorafs_reputation_snapshot_age_seconds`
  - `sorafs_reputation_score{provider_id}` (exported via Prometheus gauge with cardinality guard using top-N tracking)
  - `sorafs_reputation_threshold_crossings_total{level}`
  - `sorafs_reputation_iteration_count`
  - `sorafs_reputation_penalty_applied_total{type}`
- Logs: Structured `reputation_engine` logs with fields `snapshot_id`, `provider_id`, `score`, `penalties`, `iteration_count`.
- Alerts:
  - Ingest lag > 5 min (warning) / 15 min (critical).
  - Snapshot age > 7 days.
  - Score computation failure or non-convergence within 100 iterations.
  - Unexpected score jumps >0.25 within 24h (sanity check).

## Security & Compliance
- Ingest verifies signatures from Governance DAG to prevent tampering.
- Reputation config changes require governance multi-sig; config hashed and stored in DAG.
- API signatures: mTLS for internal consumers; public API requires JWT tokens with `reputation.read` scope.
- Rate limiting: 120 requests/min per client, bursts allowed for internal services.
- Privacy: Raw consumer feedback aggregated before inclusion; no PII stored.
- Data retention: raw events retained 12 months (hot) + 5 years (cold archive).

## Testing Strategy
- Unit tests for metric aggregation, penalty application, Merkle tree construction.
- Property tests ensuring scores remain within bounds and respond correctly to input extremes.
- Integration tests with synthetic events to verify DAG ingestion → snapshot publication pipeline.
- Regression tests verifying CLI/API outputs match expected proofs using fixtures.
- Chaos tests: simulate ingest lag, snapshot publishing failure, config mismatch; ensure alerts trigger and system recovers.
- Benchmark tests for EigenTrust iteration (target < 2 s for 5k providers).

## Rollout Plan
1. Implement ingestion (DAG listeners) and data schema; deploy staging environment drawing from test governance DAG.
2. Build scoring engine and snapshot publisher; verify results with synthetic data.
3. Integrate APIs, CLI, and SDK; run end-to-end tests with orchestrator/indexer using staging scores.
4. Staging bake: run for 2 weeks, comparing manual calculations to engine outputs.
5. Governance approval for initial weights (`ReputationConfigV1`) and publication schedule.
6. Production rollout:
   - Stage 0: generate snapshots without publishing (shadow mode).
   - Stage 1: publish weekly snapshots, mark routing usage optional.
   - Stage 2: enforce routing/incentive integration (threshold alerts active).
7. Update documentation (`docs/source/sorafs/reputation_operator.md`, portal page, dashboards). Record status/roadmap update.

## Implementation Checklist
- [x] Specify ingestion sources, schemas, and verification.
- [x] Define scoring algorithm, penalties, and smoothing.
- [x] Document snapshot generation, Merkle proof, and publication.
- [x] Detail API endpoints, CLI/SDK integrations, and authentication.
- [x] Outline observability, metrics, alerts, and logs.
- [x] Capture testing plan and rollout stages.
- [x] Note governance/configuration controls and operator guides.

With this specification, teams can implement the SoraFS reputation oracle confidently, ensuring routing and governance decisions rely on transparent, auditable, and timely provider performance data.
