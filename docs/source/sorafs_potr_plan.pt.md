---
lang: pt
direction: ltr
source: docs/source/sorafs_potr_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bee4dc2fcb6941387be4342337f29b7e256e44a518331f5cdfb61950f79612cd
source_last_modified: "2026-01-03T18:08:00.777598+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: PoTR-Lite Deadline Proofs Plan (Draft)
summary: Outline for SF-14 timed retrieval probes.
---

# PoTR-Lite Deadline Proofs Plan (Draft)

## Objectives
- Deliver timed retrieval probes for hot (≤90s) and warm (≤5min) tiers.
- Produce signed latency receipts for use in reputation and incentives.
- Surface PoTR status in routing and gateway headers.

> **Status (Feb 2026):** Torii now exposes `proof_kind=potr` via
> `/v1/sorafs/proof/stream`, replaying cached receipts and emitting metrics so
> operators can monitor deadline compliance while SF-14 finalises live probes.

## Workflow
1. Orchestrator/gateway issues a timed retrieval request.
2. Gateway responds with chunk + `Sora-PoTR-Receipt` header containing:
   - `manifest_digest`
   - `provider_id`
   - `range_start`, `range_end`
   - `request_timestamp`, `response_timestamp`
   - `latency_ms`
   - `signature`
3. Receipt stored for audit and scoring.

## Telemetry
- Metrics: `sorafs_potr_latency_ms_bucket`, `sorafs_potr_failures_total{reason}`.
- Routing uses receipts to adjust provider reputation.

## Headers
- `Sora-PoTR-Request: deadline=90s;tier=hot`
- `Sora-PoTR-Receipt: ...` (as above)

## Signature Scheme & Verification

- **Signature format:** Receipts are signed using the provider’s Dilithium3 key (same key as admission manifests) with an Ed25519 witness signature from the gateway. This dual signature allows clients to verify provider accountability and gateway attestation.
  ```norito
  struct PotrReceiptV1 {
      manifest_digest: Hash,
      provider_id: ProviderId,
      range_start: u64,
      range_end: u64,
      request_timestamp: Timestamp,
      response_timestamp: Timestamp,
      latency_ms: u32,
      tier: PotrTier,               // hot | warm
      status: PotrStatus,           // success | missed_deadline | provider_error
      provider_signature: DilithiumSignature,
      gateway_signature: Ed25519Signature,
  }
  ```
- Clients verify the Dilithium3 signature against the provider’s admission key stored in `sorafs_manifest`. The Ed25519 witness signature ensures the gateway delivered the receipt untouched.
- The orchestrator CLI (`sorafs potr verify`) validates both signatures, checks `response_timestamp - request_timestamp == latency_ms`, and confirms the deadline bound (`latency_ms <= tier_deadline`).

## Storage & Aggregation

- **Gateway persistence:** Gateways append receipts to a local log (`potr_receipts.log` in NDJSON). Log rotation occurs daily; older logs shipped to centralized storage (S3 bucket) with 90-day retention.
- **Central aggregation:** Orchestrator publishes validated receipts to the governance DAG as `PotrReceiptRecordV1` containing receipt hash and outcome. Additionally, metrics pipeline ingests receipts into a time-series database (Prometheus) with labels (`provider_id`, `tier`, `status`).
- **API:** Expose `GET /v1/potr/receipts?provider=<id>&since=<timestamp>` for auditors and reputation engine.
- **Security:** Receipts include nonces (`request_id`) to prevent replay; orchestrator ensures uniqueness per request.

## Reputation Oracle Integration

- Reputation plan consumes PoTR data:
  - `success_potr_i` metric = ratio of `status=success` receipts for provider `i` over rolling 7 days.
  - Missed deadlines (`status=missed_deadline`) contribute to penalty factors. Raw receipt data stored for transparency.
- Reputation process fetches receipts via the API or directly from DAG batch export:
  - Aggregator job runs hourly, computing latency percentiles and success ratios, emitting `PotrStatsV1`.
  - These stats feed into the reputation scoring formula (`w_potr` weight in `sorafs_reputation_plan.md`).
- **Alerts:** When a provider’s hot-tier success rate drops below 95% in the last 24h, trigger `sorafs_potr_degradation` alert and link to the reputation engine for investigation.

These details finalize the PoTR plan by specifying signatures, storage, and reputation integration.
