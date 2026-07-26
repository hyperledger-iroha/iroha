---
title: SoraFS Provider Reputation Oracle
summary: SFM-3 implementation status for the native committed journal, reputation V1 scoring, proofs, committed-derived APIs, and remaining service rollout.
---

# SoraFS Provider Reputation Oracle

## Status

SFM-3 has two local foundations: the deterministic reputation V1
snapshot/proof core and a native committed input journal. The journal uses a
governed, predecessor-bound recorder policy, one global contiguous sequence,
source-specific predecessor/revision rules, exact provider/policy/authority/
block-time binding, typed committed events, and a fixed-view finalized query.
PoR terminal outcomes and stream-token validation outcomes have dedicated
append instructions. Capacity-dispute registration appends `Opened`
atomically with the canonical dispute record, and
`ResolveSorafsCapacityDispute` atomically updates that record and appends the
exact revision-two `Resolved` event.

This is source implementation, not a readiness claim. The deterministic
multi-feed projector in `crates/sorafs_node/src/reputation.rs` is exported into
the crate graph. It consumes the existing finalized proof, unified reputation
journal, repair, orderbook, reserve-event, and reserve-provider projections;
persists five canonical restart-safe physical feed cursors; derives
byte-identical unsigned material; and retains a bounded idempotent
retry/dead-letter/acknowledgement outbox without signing keys. Torii's
local-authoritative snapshot POST and the matching CLI publication command are
removed. Latest, provider, weights, and event reads now consume only the fresh
committed-derived projection after signed snapshot validation and authenticated
Governance DAG readback. Snapshot-id reads resolve the exact authenticated
snapshot from the durable immutable suffix capped at 1,024 entries and the
publication-checkpoint byte ceiling; unknown or evicted ids return `404`.

Strict non-secret `iroha_config` policy construction and the supervised
finalized-query/threshold-signing/publication worker are implemented. The
current-head `State` adapter was removed because it cannot satisfy an immutable
historical exact-anchor query: enabling the runtime now requires a
deployment-injected `ReputationFinalizedQueryV1`. The queue-backed journal
submitter signs and enqueues PoR and counted stream-token append transactions,
while the committed publication projection is exposed to Torii only after a
fresh successful reconciliation and authenticated Governance DAG readback.
Production immutable historical-query, threshold-signer, authenticated DAG
publication/readback/head-inclusion, PoR-terminal-owner, and
stream-token-owner adapters remain open. The integrated Rust build, lint, and
test matrix for these changes is also pending.

The existing snapshot foundation includes canonical Norito/JSON schemas,
fixed-point scoring, trust-edge iteration, degradation flags, Merkle proofs,
Governance DAG payload validation, read-only Torii handlers, CLI verification
and read helpers, SDK convenience clients, and
observability assets. `scripts/build_sorafs_reputation_canary.py` builds
individual payload-free SFM-3 canary artifacts for publish/latest snapshots,
provider proofs, events, proof verification, metrics, transport, and
routing/incentive consumption evidence. The builder requires reviewed
deployment context, snapshot id/root bindings, reviewed lowercase production
provider IDs, provider proof inputs where applicable, reviewed provider names
using the same `provider-*` production shape whose unique inventory matches
`provider_count`, unique provider proof sibling hashes, non-negative integer
snapshot-age and ingest-lag threshold facts, duplicate/unknown metric rejection before writes,
bounded event-watch `limit`/`count` facts with duplicate-free sequence
inventories, reviewed `reputation-sse-event-*` and
`reputation-websocket-event-*` transport labels without non-production markers,
the governance-approved `--weights-digest-hex` input for publish/latest
snapshot anchors,
and validates every generated artifact through
`scripts/check_sorafs_reputation_rollout_evidence.py` before writing.
Checked-in response-file examples cover provider and metrics canaries.

## Goals & Scope
- Produce deterministic, governance-auditable reputation scores for each SoraFS provider to inform routing, incentives, staking, and compliance decisions.
- Combine operational metrics (PoR, PDP, PoTR, latency, disputes, settlement
  breaches) into one score on the governance-approved publication cycle. V1
  does not define a separate daily-delta format or a hard-coded weekly
  schedule.
- Provide Merkle-verifiable snapshots, public APIs, and SDK tooling, while ensuring privacy and resilience.

## Target Architecture
| Component | Responsibility | Notes |
|-----------|----------------|-------|
| Metrics ingest pipeline (`reputation_ingest`) | Deterministically consumes fixed-view proof, unified journal, repair, orderbook, reserve-event, and reserve-provider pages. | Exported projector persists only rebuildable projections, five physical finalized cursors, exact replay receipts, and a bounded unsigned-material outbox. Strict configuration, the queue-backed journal submitter, exact historical-query injection boundary, and supervised scheduling/reconciliation are wired; the production historical adapter, integrated validation, and reviewed deployment evidence remain open. |
| Scoring engine (`reputation_engine`) | Aggregates finalized projections, runs the fixed-point EigenTrust-style algorithm, applies policy penalties, and generates canonical snapshot material. | Runs on the configured supervised interval and writes only the bounded durable checkpoint/outbox; publication becomes visible through the authenticated Governance DAG and committed-derived projection. |
| Snapshot publisher (`reputation_publisher`) | Independently threshold-signs exact projector outbox material, publishes it to the Governance DAG/committed projection, and acknowledges the canonical result. | The supervised keyless worker is wired; production threshold-signer and authenticated DAG publication/readback adapters remain open. |
| API gateway (`sorafs_reputation_api`) | Exposes read-only REST, SSE, and WebSocket committed projections. | The obsolete local POST is removed. Latest/provider/weights/event reads use the ready committed projection; snapshot-id reads return the exact retained authenticated snapshot or `404` after bounded eviction, and the runtime cannot start in production until all required injected adapters exist. |
| CLI/SDK modules | `sorafs reputation` commands; SDK helper functions for verification and weighting. | Integrates with orchestrator, indexer, orderbook, incentives. |

### Data Flow
1. Governance activates an exact recorder-policy revision in committed state.
2. Governed authorities submit source-bound native transactions. V1 core
   currently admits PoR terminals, stream-token validation outcomes, and
   capacity-dispute `Opened`/`Resolved` transitions.
3. `reputation_ingest` reads all five physical finalized feeds, advances only
   through their exact contiguous sequences (with PoR/dispute/token sharing one
   journal cursor), and reconciles crash-staged projection state after restart.
4. The service normalises committed entries and the additional typed domain
   events into rebuildable metric projections (`raw_por`, `raw_pdp`,
   `raw_potr`, `raw_latency`, `raw_disputes`, `raw_tokens`).
5. `reputation_engine` processes those projections, calculates rolling
   windows, applies weights/penalties, and runs the deterministic trust
   iteration.
6. The projector durably enqueues exact unsigned material. An external
   threshold signer binds the governed policy, publishes the canonical signed
   result, and acknowledges the outbox item.
7. Public APIs and SDKs serve committed-derived scores with cryptographic
   proofs; downstream systems consume them for routing and incentives.

## Data Sources & Normalisation
- **PoR/PDP/PoTR**: success ratios are derived from the native PoR journal and
  finalized proof-outcome feed. The queue-backed governed PoR and counted
  stream-token transaction submitter is present; the actual PoR/token owners
  still need to invoke its durable callbacks.
- **Latency**: P95 latency from PoTR receipts (hot/warm tiers). Normalise to `[0,1]` by mapping 0 ms→1.0, 90 s→0 (hot) / 5 min→0 (warm).
- **Disputes**: count governance disputes resolved against provider per 1k
  orders. Native capacity-dispute `Opened` and `Resolved` journal transitions
  are authoritative; capacity telemetry never creates a dispute implicitly.
- **Token violations**: rate of throttle breaches and unauthorized access
  attempts. Native stream-token validation admission is implemented; the
  regional gateway transaction forwarder remains open.
- **Repair escalations**: projected from the existing finalized native repair
  event feed.
- **Orderbook and settlement**: projected from the existing finalized native
  orderbook event feed.
- **Stake & Reserve status**: Reserve+Rent lifecycle stage
  (Active/Warning/Grace/Delinquent/Default) is resolved from the same-anchor
  finalized reserve event and complete provider-account projections.
- Native journal entries and query pages use canonical Norito. A local
  database, telemetry exporter, or Governance DAG mirror may cache or project
  them, but cannot replace the committed journal as input authority.

The V1 service does not define a PostgreSQL authority or an object-storage
schema. Its durable local state is the canonical bounded checkpoint containing
the five physical finalized cursors, rebuildable metric projections, exact
replay receipts, and unsigned publication outbox. The authenticated publication
projection retains an immutable suffix capped at 1,024 snapshots and the
configured checkpoint byte ceiling. Operators may maintain additional caches
or archives, but those stores are never accepted as journal, signer-policy, or
publication authority.

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
  R_bps = (α_bps * C_bps * R_bps + (10000 - α_bps) * t_bps) / 10000
  ```
  where `α_bps = 8500`, `t_bps` is the canonical baseline trust vector, and
  `C_bps` is built from settlement-satisfaction edges. Every multiply, divide,
  remainder assignment, and convergence comparison uses checked integers.
  Iteration stops when the L1 delta is at most one basis point or after 100
  iterations.
- **Degradation penalties**:
  - Reserve lifecycle `Warning`, `Grace`, `Delinquent` multiply by `[0.9, 0.75, 0.5]`.
  - PoR/PDP success <90% 7-day → ×0.8; <80% → ×0.6.
  - Active dispute or slashing event sets `degradation_flag = "probation"` and clamps score ≤0.20.
- **Smoothing**: `R_final = 0.7 * R_current + 0.3 * R_prev`.
- **Bounds**: `0.05 ≤ R_final ≤ 0.99`. Providers below 0.15 flagged.
- **Transparency**: the canonical provider record carries the exact bounded raw
  metrics and degradation flags; the signed snapshot binds weights and scoring
  parameters.

## Publication & Verification
- Each governance-scheduled cycle produces a canonical
  `ReputationSnapshotV1`:
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
- Exact unsigned material enters the durable outbox, is threshold-signed
  externally, and is published to the authenticated Governance DAG. Only
  reconciled readback/head inclusion may update the committed-derived public
  projection.
- V1 publishes complete snapshots and sequenced snapshot events. It has no
  authoritative S3 layout or `ReputationDeltaV1` daily-diff fallback.
- Torii broadcasts `ReputationSnapshotEvent` with `snapshot_id`, `merkle_root`, `generated_at`.
  The local implementation records this as sequenced `ReputationSnapshotEventV1`
  rows that can be listed through `GET /v1/sorafs/reputation/events`.
- CLI `sorafs_cli reputation verify --snapshot <file> --provider-id <id>
  --proof <file>` replays Merkle proof for archived Norito artifacts.

### Current implementation surface

- `crates/iroha_data_model::sorafs::reputation` defines the native
  `ReputationJournalAuthorityPolicyV1`, policy activation record, typed journal
  entries, source heads, globally sequenced committed-event records, finalized
  cursors, and bounded event pages. V1 source families are PoR terminals,
  provider-capacity disputes, and stream-token validation outcomes.
- Native instructions provide governed policy activation, exact PoR and
  stream-token appends, and atomic capacity-dispute resolution.
  `RegisterCapacityDispute` is the only capacity-dispute intake path and
  appends the revision-one `Opened` journal event in the same transaction.
  Telemetry penalties and alerts do not create disputes.
- `FindSorafsReputationJournalEvents` returns an exclusive-cursor page from one
  immutable finalized view. Core persistence cross-checks the active and
  historical policy records, global head, sequence keys, event-id index,
  source-head index, block/index continuity, and exact source predecessor
  before returning a page. Hard item, object, decode-depth, event, and page
  bounds apply.
- `CanManageSorafsReputationJournalPolicy`,
  `CanRecordSorafsReputationJournal`, and
  `CanResolveSorafsCapacityDispute` are exact unit permissions wired through
  the default executor. Recorder identity is additionally pinned by the active
  policy; holding the generic record permission does not make an account the
  governed recorder.
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
  accept `GovernanceLogPayloadV1::SignedReputationSnapshot` and retain the full
  threshold-signed envelope and deterministic scoring evidence. Intrinsic
  envelope validation runs again when the DAG is read.
- `sorafs_cli reputation verify --snapshot=PATH [--provider-id=ID
  --proof=PATH] [--summary-out=PATH]` validates canonical Norito snapshots and
  optional provider Merkle proofs, then emits a JSON summary for operator logs.
- `sorafs_cli reputation snapshot --torii-url=URL` and `sorafs_cli reputation
  fetch --torii-url=URL --provider-id=ID [--format=table|json]` provide the
  read-only Torii consumption workflow. `sorafs_cli reputation watch
  --torii-url=URL [--since=N] [--limit=N]` polls the implemented reputation
  event list and advances by `next_since`. There is deliberately no
  `reputation publish` command or Torii POST fallback.
- `crates/sorafs_node/src/reputation.rs` contains a deterministic
  `ReputationIngestService` that joins proof, journal, repair, orderbook, reserve,
  and provider pages at one finalized height/hash/timestamp. It is exported,
  exposes restart-safe cursors for the five physical feeds, derives canonical
  unsigned signing material, and persists a bounded idempotent outbox with
  retry, dead-letter, exact signed-envelope binding, acknowledgement, canonical
  corruption checks, full public trust-policy/quorum/revocation/freshness
  verification, and payload-free status/metrics. Standard-daemon config,
  trust-policy injection, state-backed query scheduling, queue-backed journal
  submission/finality reconciliation, supervision, and payload-free status are
  wired. External threshold-signing and authenticated Governance DAG adapters,
  the committed read projection, integrated Rust validation, and reviewed
  deployment evidence remain open.
- Torii exposes only read handlers at `GET /v1/sorafs/reputation/latest` and
  `GET /v1/sorafs/reputation/providers/{provider_id}`. The provider endpoint
  returns the latest provider record with a Merkle proof. Historical lookup and
  configuration discovery are available through
  `GET /v1/sorafs/reputation/snapshots/{snapshot_id_hex}` and
  `GET /v1/sorafs/reputation/weights`, and bounded event polling is available
  through `GET /v1/sorafs/reputation/events`. Live server-sent event streaming
  is available through `GET /v1/sorafs/reputation/events/stream`, seeded by the
  same optional `since`/`limit` backlog cursor. WebSocket parity is available at
  `/v1/sorafs/reputation/events/ws` with JSON text frames backed by the same event broadcaster.
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
  evidence bundle before routing/incentive enforcement. The gate accepts an
  externally produced threshold-signing/publication artifact plus the
  read-only `sorafs_cli reputation snapshot|fetch|watch|verify` JSON artifacts,
  deployed metrics, SSE/WebSocket transport, and
  routing/incentive consumption evidence; it requires one fresh publish/latest
  snapshot anchor and requires provider, event, proof replay, metrics,
  transport, and routing/incentive consumption artifacts to bind to that
  `snapshot_id_hex`/`merkle_root_hex` tuple. It also requires bounded ingest
  lag, non-negative integer snapshot-age and ingest-lag evidence, provider
  proof coverage, proof replay, transport event delivery, and downstream
  consumption. Snapshot binding failures are recorded on the
  offending artifact before required-kind validity is finalized, so the JSON
  summary matches the fail-closed rollout decision. The aggregate
  production-readiness gate also requires `valid_snapshot_bindings` to match the
  top-level `snapshot_id_hex` and `merkle_root_hex` pair before final promotion
  can report ready, and rechecks snapshot-bound artifact fingerprints against
  `valid_snapshot_bindings` so downstream provider, event, proof, metrics,
  transport, and consumption artifacts cannot drift from the lane-proven
  publish/latest snapshot binding. It rejects raw snapshot/proof
  bytes, raw provider records, request or
  response bodies, bearer tokens, signed transactions, private keys, and other
  payload-bearing fields. The checker exports its required top-level payload
  fields as `EVIDENCE_REQUIRED_FIELDS`, allowing dry-run collection plans and
  downstream automation to inspect the exact SFM-3 evidence contract before
  live collection. Publish/latest snapshot, proof verification, metrics, and
  routing/incentive consumption artifacts must bind `provider_count` to the
  unique canonical `providers[].name` inventory and reject duplicate provider
  entries before promotion can report ready. Those `providers[].name` entries
  must use reviewed lowercase `provider-*` IDs without non-production markers,
  matching the provider-proof and verification `provider_id` policy. Metrics
  artifacts also bind `metric_count` to the unique canonical `metrics`
  inventory, require the reviewed reputation metrics set, and reject duplicate
  or unknown metric entries before promotion can report ready. Metrics and
  transport artifacts must explicitly set
  `response_bodies_included` to `false`, and routing/incentive consumption
  artifacts must explicitly set `raw_provider_records_included` to `false`,
  before promotion can report ready. The summary
  exports the sorted reviewed `metrics` inventory plus `metric_count_values`,
  and the aggregate production-readiness gate requires those fields to match
  the metrics artifact fingerprint before final promotion can report ready.
  Provider proof and verification artifacts must use reviewed lowercase
  `provider-*` IDs and must not contain non-production markers in
  provider/proof/verify `provider_id` fields. Required providers must have
  both matching provider-proof evidence and matching proof-verification
  evidence before promotion can report ready, and the default gate requires at
  least one provider ID to appear in both proof and verification evidence.
  Event-watch evidence must carry a positive `limit`, keep `count` equal to the
  `events[]` length, bind `count` to duplicate-free `events[].sequence` values,
  reject `count` values above that limit, and require every `events[]` row to
  be V1 with the same snapshot id, Merkle root, and provider count before
  transport evidence can report ready.
  Transport evidence must bind `sse_event_count` and `websocket_event_count` to
  the unique canonical `sse_events[].name` and `websocket_events[].name`
  inventories, require reviewed `reputation-sse-event-*` and
  `reputation-websocket-event-*` labels without non-production markers, and
  reject duplicate or malformed transport-event entries before promotion can
  report ready.
  It supports shell-style `@ARGFILE` inputs for direct replay of reviewed
  evidence directories and explicit artifacts.
- `scripts/run_sorafs_reputation_rollout_evidence.py` requires reviewed
  payload-free external publication evidence, then collects the deployed
  read-only bundle with bounded `sorafs_cli reputation
  snapshot|fetch|watch|verify` commands. It supports shell-style `@ARGFILE`
  response files, checks provider proof coverage before touching a live Torii
  endpoint, and then runs the evidence gate. Its `--dry-run` output includes
  the checker-backed
  `evidence_contract` map for publish/latest, provider, events, verify,
  metrics, transport, and consumption artifacts, and the runner validates the
  schema-closed collection plan, external evidence map, evidence contract, and
  command steps before dry-run output or live collection.
  `scripts/examples/sorafs_reputation_rollout_collection.args.example`
  provides a payload-free operator template.
- Operator workflow notes live in
  `docs/source/sorafs/reputation_operator.md`.

## APIs & SDK
- Native ledger contract:
  - Implemented in the data model/core: governed journal policy activation,
    PoR-terminal and stream-token append instructions, capacity-dispute
    `Opened` integration, `ResolveSorafsCapacityDispute`, typed committed
    events, and `FindSorafsReputationJournalEvents`.
  - Generic authenticated signed-transaction and typed-query transport plus
    committed read projections are present. Dedicated authenticated SDK
    journal transaction/query builders remain open; no local snapshot route is
    an authoritative journal mutation API.
- Standard-daemon runtime:
  - Implemented: strict non-secret `iroha_config` construction for the release
    window, governed weights, resource bounds, checkpoint roots, external
    adapter identities, and Governance DAG publisher identity.
  - Implemented: `IrohaRuntimeDeps` injection plus supervised
    startup/shutdown, restart-safe checkpoint opening, payload-free
    readiness/status, and bounded Prometheus metrics for the committed
    projector/publication reconciler. Missing, null/test-marked, or
    identity-substituted finalized-query, threshold-signer, and Governance DAG
    adapters fail startup.
  - Implemented: `QueuedReputationJournalTransactionSubmitterV1` signs and
    submits typed PoR/token append transactions through the normal queue.
    `IrohaRuntimeDeps` accepts an identity-pinned immutable historical
    `ReputationFinalizedQueryV1`; the unsound current-head state adapter and
    fallback were removed. Missing exact historical capability now fails
    startup instead of fabricating a fixed view.
  - Open under `V1-BLOCK-REPUTATION-RUNTIME-01`: deployment-owned
    immutable historical finalized-query,
    `ReputationThresholdSignerClientV1` and
    `ReputationGovernanceDagClientV1` adapters; concrete PoR-terminal and
    stream-token callback-owner wiring; current DAG head/inclusion proof;
    integrated Rust validation; and reviewed four-peer rotation, recovery,
    retry, and failover evidence remain outstanding. No ledger page,
    credential, signature, or acknowledgement may be synthesized as a
    fallback.
- REST endpoints:
  - Removed: the local-authoritative `POST /v1/sorafs/reputation/latest`
    descriptor, router mount, handler, and OpenAPI operation. Signed publication
    must come from the verified projector outbox/external threshold-signing
    flow.
  - Implemented locally: `GET /v1/sorafs/reputation/latest` returns latest
    snapshot metadata plus a `limit`-bounded provider-score array while
    preserving the total `provider_count`. Latest/provider/weights/event
    handlers read only the ready committed publication projection. Torii
    returns unavailable before the supervised runtime is fresh and fully
    reconciled.
  - Implemented locally: `GET /v1/sorafs/reputation/providers/{provider_id}`
    returns the provider entry with Merkle proof.
  - Implemented locally: `GET /v1/sorafs/reputation/snapshots/{snapshot_id_hex}`
    resolves the requested 16-byte id against the durable immutable suffix of
    authenticated committed snapshots and returns that exact snapshot with the
    same `limit`-bounded provider-score readback. The suffix is capped at 1,024
    entries and by the publication checkpoint byte ceiling; unknown or evicted
    ids return `404` rather than falling back to the latest snapshot.
  - Implemented locally: `GET /v1/sorafs/reputation/weights` returns the
    weights and smoothing parameters from the latest snapshot.
  - Implemented locally: `GET /v1/sorafs/reputation/events` returns sequenced
    snapshot events, with `since` and `limit` cursor parameters.
  - Implemented locally: reputation `GET` responses include deterministic
    `ETag` validators plus `Cache-Control`, and honor `If-None-Match` with
    `304 Not Modified`.
  - Implemented locally: `GET /v1/sorafs/reputation/events/stream` provides
    live server-sent events for reputation snapshot publications.
  - The repository ships reputation Prometheus/Grafana contracts. Remaining
    rollout: connect them to the supervised committed projector/publisher and
    capture live run evidence.
- Implemented locally: WebSocket `/v1/sorafs/reputation/events/ws` emits `reputation_snapshot`
  JSON text frames for the optional `since`/`limit` backlog and for live
  snapshot publications. Lag notifications use `event = "lagged"` frames so
  clients can resynchronize through `GET /v1/sorafs/reputation/events`.
- SDK helpers:
  - Rust currently has generic signed native transaction/query submission for
    the journal ISIs and `FindSorafsReputationJournalEvents`; there is no
    dedicated `ReputationClient`. Dedicated committed-projection builders remain
    release work.
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
- Required deployed logs are payload-free: bounded cursor/sequence, counts,
  policy/snapshot digests, lag, and iteration metadata only. Journal entries,
  raw provider records, token material, signatures, request/response bodies,
  credentials, and signing material must never be logged.
- Alerts:
  - Snapshot age > 7 days (`SoraFSReputationSnapshotStale`).
  - Ingest lag > 15 minutes (`SoraFSReputationIngestLagHigh`).
  - Low-score provider presence or fresh low-score crossings (`SoraFSReputationLowScoreProviders`, `SoraFSReputationLowScoreCrossing`).
  - Planned engine alert: score computation failure or non-convergence within 100 iterations.
  - Unexpected score jumps >0.25 within 24h (sanity check).

## Security & Compliance
- Native journal admission verifies the governed recorder identity, exact unit
  permissions, active policy digest, provider binding, committing block time,
  global/source continuity, and exact historical replay. The ingest service
  must consume the finalized typed query and must not trust telemetry,
  Governance DAG mirrors, or a local database as the event authority.
- Reputation config changes require governance multi-sig; config hashed and stored in DAG.
- Deployment requirement: mTLS for internal consumers; public API access
  requires the governed authentication policy and `reputation.read` scope.
- Rate limiting: 120 requests/min per client, bursts allowed for internal services.
- Privacy: Raw consumer feedback aggregated before inclusion; no PII stored.
- Retention is not hard-coded as a 12-month/5-year service promise. Committed
  journal history follows ledger/governance retention, while the online
  authenticated snapshot projection uses the bounded 1,024-entry immutable
  suffix and checkpoint byte ceiling. Any longer operational archive is
  deployment policy and cannot become authority.

## Testing Strategy
- Native journal tests cover strict policy rotation/predecessor forks, recorder
  authority and exact permission payloads, source/provider/policy/block-time
  binding, global sequence and per-block index continuity, historical replay,
  stale policy rejection, forged/orphan indexes and tails, bounded fixed-view
  pagination, and atomic capacity-dispute open/resolve lifecycle.
- Unit tests for metric aggregation, penalty application, Merkle tree construction.
- Property tests ensuring scores remain within bounds and respond correctly to input extremes.
- Remaining integration tests must drive committed transactions through
  multiple peers, rebuild the service from finalized journal pages, and verify
  journal ingest → deterministic signing material → externally signed snapshot
  publication without a competing local authority.
- Regression tests verifying CLI/API outputs match expected proofs using fixtures.
- Chaos tests: simulate ingest lag, snapshot publishing failure, config mismatch; ensure alerts trigger and system recovers.
- Benchmark tests for EigenTrust iteration (target < 2 s for 5k providers).

## Rollout Plan
1. Complete integrated source validation, then add the remaining authenticated
   journal transaction/query SDK builders.
2. Supply and exercise the immutable historical finalized-query adapter plus
   the queue-backed governed PoR/regional counted stream-token transaction
   submitter; wire the actual PoR and token owners to the durable callbacks.
   The projector already consumes the committed proof, unified journal,
   repair, orderbook, and reserve query families.
3. Supply external threshold signer and authenticated Governance DAG
   publication/readback/head-inclusion adapters to the already-supervised
   runtime. Reconcile canonical publication acknowledgements without
   introducing a local signing-key fallback.
4. Deploy the supervised publisher and API against the committed projection,
   exercise exact retained snapshot-id lookup and bounded eviction, and run
   four-peer end-to-end tests with orchestrator/indexer consumers.
5. Staging bake: run for 2 weeks, comparing manual calculations to engine outputs.
6. Governance approval for initial weights (`ReputationConfigV1`) and publication schedule.
7. Production rollout:
   - Stage 0: generate snapshots without publishing (shadow mode).
   - Stage 1: publish on the approved governance schedule, with routing usage
     optional.
   - Stage 2: enforce routing/incentive integration (threshold alerts active).
8. Update documentation (`docs/source/sorafs/reputation_operator.md`, portal page, dashboards). Record status/roadmap update.

## Rollout Status

Completed local foundations:

- Native chain-authoritative journal model, governed policy/history, one global
  sequence, event/source indexes, exact replay rules, payload-free typed
  events, and fixed-view finalized query.
- Native PoR and stream-token append instructions, atomic
  `RegisterCapacityDispute` → `Opened` integration, and atomic
  `ResolveSorafsCapacityDispute` → revision-two `Resolved` integration. Proof
  telemetry remains penalty/alert input and cannot mutate the dispute map.
- Canonical reputation schemas, scoring, penalties, smoothing, trust-edge
  iteration, snapshot validation, Merkle roots, and provider proofs.
- Governance DAG payload validation and the keyless finalized multi-feed
  projector/outbox contract.
- Read-only Torii latest, provider, snapshot, weights, events, SSE, and
  WebSocket handlers consume the fresh committed projection; snapshot-id reads
  return the exact retained authenticated snapshot and reject unknown or
  evicted ids.
- CLI `reputation verify`, `snapshot`, `fetch`, and `watch`; the obsolete local
  publication command is retired.
- The rollout evidence gate now emits aggregate-compatible required-kind
  metadata, evidence counts, required-row `present`/`artifact_count` fields,
  threshold metadata, and reviewed deployment-context fingerprints so the final
  production readiness aggregate can consume real reputation summaries directly.
- JavaScript/TypeScript and Python convenience clients for reads, event polling,
  and SSE consumption.
- Scheduler consumption through `reputation_score_bps`.
- Grafana dashboard and Prometheus alert rules for accepted snapshot health.
- Rollout evidence summary gate for fresh snapshot, publisher, provider proof,
  event delivery, metrics, SSE/WebSocket, and routing/incentive consumption
  artifacts. The gate now fails closed when any recognized artifact is invalid,
  including stale duplicate artifacts or optional artifacts outside the required
  subset.
- `scripts/build_sorafs_reputation_canary.py` provides checked-in payload-free
  canary generation for the local SFM-3 rollout gate. Count-bearing reputation
  canaries bind `provider_count` to reviewed `provider-*` `providers[].name`
  inventory before writing, publish/latest canaries require the
  governance-approved `weights_digest_hex`, and metrics canaries derive
  `metric_count` from the reviewed required metric inventory before writing.
  Publish/latest artifacts must carry the governance-approved
  `weights_digest_hex`, keep the digest consistent across both snapshot anchors,
  and export `valid_reputation_weight_digests` for aggregate promotion. The
  rollout summary now
  carries the reviewed metric inventory and `metric_count_values` as
  aggregate-gate metadata so final production readiness cannot promote a
  reputation summary whose metrics evidence is absent, truncated, unknown,
  unsorted, or untethered from the
  metrics artifact fingerprint. The builder rejects non-canonical or
  non-production `--provider-id` values before writing. Provider proof canaries
  reject duplicate Merkle sibling hashes before writing, and the rollout
  checker enforces the same uniqueness on externally
  supplied provider proof evidence. Required provider canary inputs are checked
  against both provider-proof and proof-verification evidence when the full
  rollout gate is evaluated, preventing verification-only or proof-only
  coverage from satisfying readiness. Event-watch canaries and externally
  supplied event artifacts must also prove a positive polling `limit`, exact
  `count`/`events[]` length agreement, duplicate-free `events[].sequence`
  inventory binding, `count <= limit`, and V1 event rows with consistent
  snapshot id, Merkle root, and provider count before readiness is reported. Metrics
  canaries and externally supplied metrics artifacts must present
  `snapshot_age_seconds` and `ingest_lag_seconds` as non-negative integer
  seconds before the SFM-3 freshness/lag ceilings can pass.
  Transport canaries also require reviewed `reputation-sse-event-*` and
  `reputation-websocket-event-*` labels without non-production markers matching
  `sse_event_count` and `websocket_event_count` before writing.
  The aggregate production-readiness gate also requires
  `valid_snapshot_bindings` to match the top-level `snapshot_id_hex` and
  `merkle_root_hex` pair before final promotion can report ready, and rechecks
  snapshot-bound artifact fingerprints against `valid_snapshot_bindings` so
  downstream provider, event, proof, metrics, transport, and consumption
  artifacts cannot drift from the lane-proven publish/latest snapshot binding.
  Aggregate promotion also requires `valid_reputation_weight_digests` to match
  publish/latest artifact fingerprints and rechecks every publish/latest
  artifact against that metadata before final promotion can report ready.
- Rollout evidence collection harness that requires reviewed external
  publication evidence, then reads back, watches, proof-replays, and verifies
  the deployed reputation evidence bundle from one response-file driven
  operator command.

Remaining production gates:

- Complete integrated source validation; expose dedicated authenticated SDK
  journal transaction/query builders; validate the wired fixed-view
  active-policy query and durable queue-backed PoR/countable-token submission
  and finality reconciler; validate the existing committed-derived GET/event
  reads under restart and failover; deploy external threshold-signing and authenticated
  Governance DAG publication/readback adapters; and complete reviewed
  four-peer rotation, retry/failover, and recovery evidence.
- Capture live run evidence for snapshot freshness, ingest lag, low-score
  handling, SSE/WebSocket event delivery, and routing/incentive consumption,
  then publish a `ready` summary from
  `scripts/run_sorafs_reputation_rollout_evidence.py` or the direct
  `scripts/check_sorafs_reputation_rollout_evidence.py` gate.
- Publish governance-approved weights with the governed `weights_digest_hex`
  carried by publish/latest rollout evidence, then archive the first production
  snapshot `.to`/JSON artifacts and proof replay evidence.
- Exercise rollback/stale-snapshot procedures before routing or incentives rely
  on scores in production.
