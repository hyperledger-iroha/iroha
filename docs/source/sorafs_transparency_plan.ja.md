---
lang: ja
direction: ltr
source: docs/source/sorafs_transparency_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 72c55f773aae184a828641c347fa9ff1863f9568ecb48078c6dde767e2a036f7
source_last_modified: "2026-01-03T18:07:58.806660+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: Transparency Dashboards & Enforcement Receipts
summary: Final specification for SFM-4c covering moderation ledgers, proof APIs, dashboards, privacy-preserving metrics, and rollout.
---

# Transparency Dashboards & Enforcement Receipts

## Goals & Scope
- Publish deterministic transparency ledgers summarising moderation/compliance activity with cryptographic proofs so clients can verify opaque moderation tokens.
- Operate dashboards and APIs that report moderation stats while preserving privacy via differential privacy and aggregation.
- Provide a public receipt explorer enabling external auditors to validate enforcement outcomes.

This specification completes **SFM-4c — Transparency dashboards & enforcement receipts**, with subcomponents SFM-4c1 (privacy metrics) and SFM-4c2 (receipt explorer) addressed within.

## Architecture Overview
| Component | Responsibility | Notes |
|-----------|----------------|-------|
| Event ingestor (`transparency_ingest`) | Consumes moderation events (AI screening, compliance blocks, appeals, overrides) from Governance DAG and compliance pipelines. | Ensures deduplication and lag monitoring. |
| Ledger builder (`transparency_builder`) | Aggregates events per cycle, constructs Merkle-backed ledger blocks, stores in IPFS/S3. | Runs hourly; publishes weekly cycles. |
| Proof API (`transparency_api`) | Serves proof requests, ledger metadata, and summary statistics. | REST/GraphQL + WebSocket updates. |
| Dashboard backend | Pushes metrics to Grafana/Looker; exposes panel JSON. | Aggregates anonymised stats. |
| Receipt explorer UI | Web app for browsing ledger cycles, entry summaries, and verifying proof tokens. | Hosted at `transparency.sora.net`. |

## Ledger & Proof Design
- Cycle: ISO week (`YYYY-WW`) by default; configuration allows daily cycles if required.
- Ledger block (`ModerationLedgerBlockV1`):
  ```norito
  struct ModerationLedgerBlockV1 {
      version: u8,
      cycle_id: String,
      period_start: Timestamp,
      period_end: Timestamp,
      prev_root: Digest32,
      entries_root: Digest32,
      entry_count: u64,
      publishers: Vec<PublisherSignatureV1>,
  }
  ```
- Entry enum (`ModerationLedgerEntryV1`) includes:
  - `ActionSummaryV1` (policy, action, count, token_root)
  - `AppealOutcomeV1` (case hash, outcome, stake totals)
  - `OverrideNoticeV1`
  - `EvidenceAccessSummaryV1`
  - `TokenIssuanceLogV1`
  - `ComplianceDashboardSnapshotV1`
- Merkle tree built over sorted entry hashes (`blake3("SORA-MOD-ENTRY" || canonical(entry))`).
- Proof struct:
  ```norito
  struct ModerationLedgerProofV1 {
      cycle_id: String,
      entry_hash: Digest32,
      siblings: Vec<Digest32>,
      index: u64,
      root: Digest32,
  }
  ```
- Ledger blocks stored in IPFS (CAR files) + S3; root recorded in Governance DAG (`TransparencyLedgerNode`).
- Publication SLA: within 2 hours of cycle close; builder emits `TransparencyLedgerPublishedV1`.

## Privacy-Preserving Metrics (SFM-4c1)
- Differential privacy pipeline:
  - Metrics (counts of content types, appeals, evidence access) aggregated per class, then noise added using Laplace mechanism (`ε=1.0`, `δ=1e-6`).
  - Bucketing by jurisdiction ensures per-bucket counts ≥5 before release.
  - Pipeline ensures contributions per account limited via clipping.
- Metrics pipeline architecture:
  - `metrics_ingest` → `dp_aggregator` (Rust) → `dp_sanitizer` (applies noise + policy).
  - Output `DPMetricsBlockV1` stored alongside ledger block.
- API `/v2/transparency/metrics?cycle=` returns sanitized metrics with metadata (`epsilon`, `delta`, `timestamp`).
- Observability: `sorafs_transparency_dp_epsilon`, `..._lag_seconds`; alerts when sanitization fails.

## Receipt Explorer (SFM-4c2)
- Web UI features:
  - Cycle list with status (published, signed).
  - Entry explorer filtered by type (appeal outcome, compliance action).
  - Proof verification widget: paste token or entry data to verify inclusion.
  - Download ledger block (Norito JSON) + proof files.
- Backend routes:
  - `GET /v2/transparency/ledger/{cycle_id}` → block metadata + signatures.
  - `GET /v2/transparency/entry/{cycle_id}/{entry_hash}` → entry payload + proof.
  - `POST /v2/transparency/token/verify` → validate moderation token against ledger.
  - `GET /v2/transparency/cycles` → list available cycles, status.
- Authentication: read-only public endpoints; optional API key for high-rate usage.
- Rate limits to prevent scraping: default 60 req/min per IP.

## Dashboards & Observability
- Grafana panels:
  - Ingest lag, entry counts, proof requests, DP noise budgets.
  - Top-level compliance metrics (blocks, appeals resolved).
- Events pushing to Slack/alerts:
  - Publication missed SLA.
  - Proof request errors.
  - Differential privacy failure.
  - Ledger signature mismatch.
- Metrics:
  - `sorafs_transparency_ingest_events_total{type}`
  - `sorafs_transparency_publish_latency_seconds`
  - `sorafs_transparency_proof_requests_total{result}`
  - `sorafs_transparency_token_verification_total{result}`
  - `sorafs_transparency_dp_budget_remaining`

## Security & Governance
- Ledger blocks co-signed by ≥2/3 validators; signatures stored in block.
- Proof tokens verification uses hashed tokens; sensitive details not exposed.
- DP pipeline audited; parameters change requires governance approval.
- Access logs for explorer maintained; IP hashed/truncated for privacy.
- Compliance with legal requests: ability to omit sensitive entries while providing explanation in ledger with placeholder entry referencing legal hold.

## Testing & Rollout
- Unit tests: entry canonicalization, Merkle hashing, proof verification.
- Integration tests: ingest → ledger build → publication → proof API; executed in CI.
- DP pipeline tests verifying noise addition and privacy budget tracking.
- Load testing for explorer (target 500 req/s).
- Rollout:
  1. Implement ingest/build pipeline, run staging cycles.
  2. Validate DP pipeline with synthetic data; governance approves `ε/δ`.
  3. Deploy explorer UI; run closed beta.
  4. Publish first production cycle (shadow mode), compare with manual stats.
  5. Enable public API & dashboards; announce to community.

## Implementation Checklist
- [x] Define ledger schema, Merkle proofs, and publication workflow.
- [x] Document privacy-preserving metric pipeline and parameters.
- [x] Specify explorer API/UI, rate limits, and proof validation.
- [x] Capture observability metrics, alerts, and governance controls.
- [x] Outline testing and rollout steps.

With this specification, transparency, compliance, and observability teams can deliver verifiable moderation reporting that balances openness with user privacy.
