---
lang: ur
direction: rtl
source: docs/source/sorafs_reputation_plan.md
status: needs-update
source_hash: e90824639bc20cd535be2f2037b825c934b591957995c2b83a434855eaa551d7
source_last_modified: "2026-06-25T18:04:39+00:00"
translation_last_reviewed: 2026-06-25
---

# SoraFS Provider Reputation Oracle

## Status

SFM-3 has a deterministic local reputation V1 core: canonical Norito/JSON
schemas, fixed-point scoring, trust-edge iteration, degradation flags, Merkle
proofs, Governance DAG payload validation, local Torii publication/read APIs,
CLI verification and publication helpers, SDK convenience clients, and
observability assets. Remaining rollout work is deploying the live
ingest/publisher service and archiving production evidence that passes the
rollout evidence gate, not the local scoring, proof, API, CLI, SDK, dashboard,
or verifier foundations.

## Goals & Scope
- Produce deterministic, governance-auditable reputation scores for each SoraFS provider to inform routing, incentives, staking, and compliance decisions.
- Combine operational metrics (PoR, PDP, PoTR, latency, disputes, settlement breaches) into a single score published weekly, with daily incremental updates.
- Provide Merkle-verifiable snapshots, public APIs, and SDK tooling, while ensuring privacy and resilience.

## Target Architecture
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
  The local implementation records this as sequenced `ReputationSnapshotEventV1`
  rows that can be listed through `GET /v1/sorafs/reputation/events`.
- CLI `sorafs_cli reputation verify --snapshot <file> --provider-id <id>
  --proof <file>` replays Merkle proof for archived Norito artifacts.

### Current implementation surface

- `crates/sorafs_manifest::reputation` defines the canonical V1 Norito/JSON
  schemas for `ReputationWeightsV1`, `ReputationProviderMetricsV1`,
  `ReputationProviderInputV1`, `ProviderReputationV1`,
  `ReputationSnapshotV1`, `ReputationSnapshotEventV1`, and
  `ReputationMerkleProofV1`.
- `build_reputation_snapshot` scores providers with fixed-point basis-point
  arithmetic, applies Reserve+Rent, proof-success, dispute, and slashing
  penalties, smooths against a previous score when supplied, bounds scores to
  `0.05..=0.99`, sorts providers by id, and computes the Merkle root.
  `build_reputation_snapshot_with_trust_edges` adds canonical
  `ReputationTrustEdgeV1` settlement-satisfaction inputs and runs the fixed-point
  EigenTrust-style iteration with `alpha_bps=8500`. The direct metric score
  remains an upper bound, so pairwise trust cannot lift a provider above
  objective proof/penalty evidence.
- `ReputationSnapshotV1::merkle_proof` and `ReputationMerkleProofV1::verify`
  provide the proof replay path over the published root. Governance DAG payloads
  accept `GovernanceLogPayloadV1::ReputationSnapshot` and validate the embedded
  snapshot before publication.
- `sorafs_cli reputation verify --snapshot=PATH [--provider-id=ID
  --proof=PATH] [--summary-out=PATH]` validates canonical Norito snapshots and
  optional provider Merkle proofs, then emits a JSON summary for operator logs.
- `sorafs_cli reputation publish --torii-url=URL --snapshot=PATH`,
  `sorafs_cli reputation snapshot --torii-url=URL`, and
  `sorafs_cli reputation fetch --torii-url=URL --provider-id=ID
  [--format=table|json]` provide the local Torii publication and consumption
  workflow around the implemented endpoints. `sorafs_cli reputation watch
  --torii-url=URL [--since=N] [--limit=N]` polls the implemented reputation
  event list and advances by `next_since`.
- `sorafs_node::NodeHandle::publish_reputation_snapshot` validates a snapshot,
  writes governance publisher artifacts when configured, records a sequenced
  reputation snapshot event, and caches it as the latest local snapshot. The
  filesystem publisher writes immutable
  `reputation/snapshots/<snapshot_id>/` `.to`/`.json` artifacts plus
  `reputation/latest.to` and `reputation/latest.json` pointers with BLAKE3
  sidecars.
- Torii exposes the local reputation surface at
  `POST /v1/sorafs/reputation/latest`,
  `GET /v1/sorafs/reputation/latest`, and
  `GET /v1/sorafs/reputation/providers/{provider_id}`. The provider endpoint
  returns the latest provider record with a Merkle proof. Historical lookup and
  configuration discovery are available through
  `GET /v1/sorafs/reputation/snapshots/{snapshot_id_hex}` and
  `GET /v1/sorafs/reputation/weights`, and bounded event polling is available
  through `GET /v1/sorafs/reputation/events`. Live server-sent event streaming
  is available through `GET /v1/sorafs/reputation/events/stream`, seeded by the
  same optional `since`/`limit` backlog cursor. WebSocket parity is available at
  `/ws/reputation` with JSON text frames backed by the same event broadcaster.
- JavaScript/TypeScript and Python Torii clients expose convenience helpers for
  the local reputation latest/provider/snapshot/weights/events endpoints and
  the SSE stream, including cache-validator options for `If-None-Match` polling.
- `sorafs_car::scoreboard::TelemetrySnapshot::from_reputation_snapshot` converts
  a validated reputation snapshot into scheduler telemetry, and
  `ProviderTelemetry::reputation_score_bps` reduces routing weight without
  hard-excluding low-score providers. The `sorafs_fetch --telemetry-json` parser
  accepts the same `reputation_score_bps` field and rejects values outside
  `0..=10000`.
- `scripts/check_sorafs_reputation_rollout_evidence.py` verifies the production
  evidence bundle before routing/incentive enforcement. The gate accepts the
  existing `sorafs_cli reputation publish|snapshot|fetch|watch|verify` JSON
  artifacts plus deployed metrics, SSE/WebSocket transport, and
  routing/incentive consumption evidence; it requires one fresh publish/latest
  snapshot anchor and requires provider, event, proof replay, metrics,
  transport, and routing/incentive consumption artifacts to bind to that
  `snapshot_id_hex`/`merkle_root_hex` tuple. It also requires bounded ingest
  lag, provider proof coverage, proof replay, transport event delivery, and
  downstream consumption. Snapshot binding failures are recorded on the
  offending artifact before required-kind validity is finalized, so the JSON
  summary matches the fail-closed rollout decision. It rejects raw snapshot/proof
  bytes, raw provider records, request or
  response bodies, bearer tokens, signed transactions, private keys, and other
  payload-bearing fields. The checker supports shell-style `@ARGFILE` inputs
  for direct replay of reviewed evidence directories and explicit artifacts.
- `scripts/run_sorafs_reputation_rollout_evidence.py` collects the deployed
  rollout bundle with bounded `sorafs_cli reputation publish|snapshot|fetch|watch|verify`
  commands, supports shell-style `@ARGFILE` response files, checks provider
  proof coverage before touching a live Torii endpoint, and then runs the
  evidence gate. `scripts/examples/sorafs_reputation_rollout_evidence.args.example`
  provides a payload-free operator template.
- Operator workflow notes live in
  `docs/source/sorafs/reputation_operator.md`.

## APIs & SDK
- REST endpoints:
  - Implemented locally: `POST /v1/sorafs/reputation/latest` accepts a
    canonical `ReputationSnapshotV1`, validates it, persists configured
    governance artifacts, and caches it as latest.
  - Implemented locally: `GET /v1/sorafs/reputation/latest` returns latest
    snapshot metadata plus a `limit`-bounded provider-score array while
    preserving the total `provider_count`.
  - Implemented locally: `GET /v1/sorafs/reputation/providers/{provider_id}`
    returns the provider entry with Merkle proof.
  - Implemented locally: `GET /v1/sorafs/reputation/snapshots/{snapshot_id_hex}`
    returns a previously accepted snapshot by 16-byte id with the same
    `limit`-bounded provider-score readback.
  - Implemented locally: `GET /v1/sorafs/reputation/weights` returns the
    weights and smoothing parameters from the latest snapshot.
  - Implemented locally: `GET /v1/sorafs/reputation/events` returns sequenced
    snapshot events, with `since` and `limit` cursor parameters.
  - Implemented locally: reputation `GET` responses include deterministic
    `ETag` validators plus `Cache-Control`, and honor `If-None-Match` with
    `304 Not Modified`.
  - Implemented locally: `GET /v1/sorafs/reputation/events/stream` provides
    live server-sent events for reputation snapshot publications.
  - Implemented locally: accepted snapshots export Prometheus gauges/counters,
    and the repository ships Grafana/alert assets for deployed publisher
    health. Remaining rollout: deploy the ingest/publisher service and capture
    live run evidence.
- Implemented locally: WebSocket `/ws/reputation` emits `reputation_snapshot`
  JSON text frames for the optional `since`/`limit` backlog and for live
  snapshot publications. Lag notifications use `event = "lagged"` frames so
  clients can resynchronize through `GET /v1/sorafs/reputation/events`.
- SDK helpers:
  - Rust: `ReputationClient::latest()`, `::provider(provider_id)`, `verify_provider_record`.
  - Implemented locally in JS/TS:
    `getSorafsReputationLatest`, `getSorafsReputationProvider`,
    `getSorafsReputationSnapshot`, `getSorafsReputationWeights`,
    `listSorafsReputationEvents`, and `streamSorafsReputationEvents`.
  - Implemented locally in Python:
    `get_sorafs_reputation_latest`, `get_sorafs_reputation_provider`,
    `get_sorafs_reputation_snapshot`, `get_sorafs_reputation_weights`,
    `list_sorafs_reputation_events`, and
    `stream_sorafs_reputation_events`.
- CLI commands:
  - Implemented locally as `sorafs_cli reputation fetch --torii-url=URL
    --provider-id=ID [--format=table|json] [--summary-out=PATH]`.
  - Implemented locally as `sorafs_cli reputation snapshot --torii-url=URL
    [--output=PATH] [--summary-out=PATH]`.
  - Implemented locally as `sorafs_cli reputation publish --torii-url=URL
    --snapshot=PATH [--summary-out=PATH]`.
  - Implemented locally as `sorafs_cli reputation verify --snapshot=PATH
    [--provider-id=ID --proof=PATH] [--summary-out=PATH]`.
  - Implemented locally as `sorafs_cli reputation watch --torii-url=URL
    [--since=N] [--limit=N] [--max-polls=N] [--poll-interval-ms=N]
    [--summary-out=PATH]`.

## Integration Points
- **Routing/Indexer**: Use scores as weights when ranking providers; degrade selection of low-score providers.
- **Orderbook**: Apply penalties or restrict order volume for providers below threshold.
- **Reserve+Rent**: Feed stage changes into degradation pipeline; send alerts to operations.
- **Governance**: DAG stores snapshots; slashing proposals reference reputation history.
- **Transparency dashboards**: Display scores, trend lines, and flags.

## Observability & Alerts
- Torii publisher metrics implemented locally:
  - `sorafs_reputation_ingest_lag_seconds`
  - `sorafs_reputation_snapshot_age_seconds`
  - `sorafs_reputation_snapshot_generated_at_unix`
  - `sorafs_reputation_provider_count`
  - `sorafs_reputation_low_score_providers`
  - `sorafs_reputation_score{provider_id}` (exported via Prometheus gauge with cardinality guard using top-N tracking)
  - `sorafs_reputation_threshold_crossings_total{level}`
- Deployed scorer/publisher rollout metrics:
  - `sorafs_reputation_iteration_count`
  - `sorafs_reputation_penalty_applied_total{type}`
- Logs: Structured `reputation_engine` logs with fields `snapshot_id`, `provider_id`, `score`, `penalties`, `iteration_count`.
- Alerts:
  - Snapshot age > 7 days (`SoraFSReputationSnapshotStale`).
  - Ingest lag > 15 minutes (`SoraFSReputationIngestLagHigh`).
  - Low-score provider presence or fresh low-score crossings (`SoraFSReputationLowScoreProviders`, `SoraFSReputationLowScoreCrossing`).
  - Planned engine alert: score computation failure or non-convergence within 100 iterations.
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

## Rollout Status

Completed local foundations:

- Canonical reputation schemas, scoring, penalties, smoothing, trust-edge
  iteration, snapshot validation, Merkle roots, and provider proofs.
- Governance DAG payload validation and local filesystem publisher artifacts.
- Torii latest, provider, snapshot, weights, events, SSE, and WebSocket surfaces.
- CLI `reputation verify`, `publish`, `snapshot`, `fetch`, and `watch`.
- JavaScript/TypeScript and Python convenience clients for reads, event polling,
  and SSE consumption.
- Scheduler consumption through `reputation_score_bps`.
- Grafana dashboard and Prometheus alert rules for accepted snapshot health.
- Rollout evidence summary gate for fresh snapshot, publisher, provider proof,
  event delivery, metrics, SSE/WebSocket, and routing/incentive consumption
  artifacts. The gate now fails closed when any recognized artifact is invalid,
  including stale duplicate artifacts or optional artifacts outside the required
  subset.
- Rollout evidence collection harness that publishes, reads back, watches, proof
  replays, and verifies the deployed reputation evidence bundle from one
  response-file driven operator command.

Remaining production gates:

- Deploy the live ingest/publisher service against production proof, dispute,
  settlement, and reserve/rent event sources.
- Capture live run evidence for snapshot freshness, ingest lag, low-score
  handling, SSE/WebSocket event delivery, and routing/incentive consumption,
  then publish a `ready` summary from
  `scripts/run_sorafs_reputation_rollout_evidence.py` or the direct
  `scripts/check_sorafs_reputation_rollout_evidence.py` gate.
- Publish governance-approved weights and the first production snapshot with
  archived `.to`/JSON artifacts and proof replay evidence.
- Exercise rollback/stale-snapshot procedures before routing or incentives rely
  on scores in production.
