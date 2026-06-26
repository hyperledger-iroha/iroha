---
lang: zh-hant
direction: ltr
source: docs/source/sorafs_potr_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 99b52e440da8975c4d9d457046ce4cfc4526628f63daa9f9b8ca84e62da8d7c3
source_last_modified: "2026-06-25T17:46:08+00:00"
translation_last_reviewed: 2026-06-25
title: PoTR-Lite Deadline Proofs Status
summary: Implemented SF-14 timed-retrieval receipt capture, validation, and replay status.
---

# PoTR-Lite Deadline Proofs Status

## Objectives
- Deliver timed retrieval probes for hot (≤90s) and warm (≤5min) tiers.
- Produce signed latency receipts for use in reputation and incentives.
- Surface PoTR status in routing and gateway headers.

> **Status (Jun 2026):** Torii now captures PoTR receipts on ranged SoraFS
> gateway fetches, records them in the embedded SoraFS node, validates receipt
> invariants/signatures through `sorafs_manifest::potr`, and replays cached
> receipts through `/v1/sorafs/proof/stream` with `proof_kind=potr`. Remaining
> SF-14 work is live multi-provider rollout evidence and PQ provider-signature
> key distribution, not local receipt capture, validation, or replay wiring.
> `scripts/check_sorafs_potr_rollout_evidence.py` now provides the fail-closed
> SF-14 rollout evidence gate, and
> `scripts/run_sorafs_potr_rollout_evidence.py` provides the reviewed
> collection planner/runner.

## Workflow
1. Orchestrator/gateway issues a timed retrieval request with
   `Sora-PoTR-Request: deadline=<millis>;tier=<hot|warm|archive>` plus optional
   `request-id=<hex>` and `trace-id=<hex>` parameters.
2. Gateway responds with the requested range plus `Sora-PoTR-Receipt` and
   `Sora-PoTR-Status` headers containing a base64 Norito `PotrReceiptV1` with:
   - `manifest_digest`
   - `provider_id`
   - `range_start`, `range_end`
   - `requested_at_ms`, `responded_at_ms`, `recorded_at_ms`
   - `latency_ms`
   - `deadline_ms`, `tier`, `status`
   - optional `request_id`, `trace_id`, and gateway/provider signatures
3. The gateway validates the receipt before recording it. Invalid receipts are
   dropped instead of being exposed through proof streams.
4. Operators replay cached receipts with `sorafs_cli proof stream
   --proof-kind=potr` or the raw `/v1/sorafs/proof/stream` endpoint.

## Telemetry
- Proof-stream metrics:
  `torii_sorafs_proof_stream_events_total{kind="potr",result,reason}`,
  `torii_sorafs_proof_stream_latency_ms_bucket{kind="potr"}`, and
  `torii_sorafs_proof_stream_inflight{kind="potr"}`.
- Proof-health metrics:
  `torii_sorafs_proof_health_potr_breaches` and
  `torii_da_potr_bonus_micro_total`.
- Reputation scoring consumes validated receipt summaries through the local
  SoraFS reputation pipeline; live production weighting is rollout evidence.

## Headers
- `Sora-PoTR-Request: deadline=90000;tier=hot`
- `Sora-PoTR-Receipt: <base64 PotrReceiptV1>`
- `Sora-PoTR-Status: success|missed_deadline|provider_error`

## Signature Scheme & Verification

- **Signature format:** `PotrReceiptV1` carries optional gateway and provider
  signatures. Ed25519 is the current gateway default; ML-DSA/Dilithium3
  provider attestations are schema-supported and remain gated on operator key
  distribution.
  ```norito
  struct PotrReceiptV1 {
      manifest_digest: Hash,
      provider_id: ProviderId,
      tier: PotrTier,               // hot | warm
      deadline_ms: u32,
      latency_ms: u32,
      status: PotrStatus,           // success | missed_deadline | provider_error
      requested_at_ms: Timestamp,
      responded_at_ms: Timestamp,
      recorded_at_ms: Timestamp,
      range_start: u64,
      range_end: u64,
      request_id: Option<[u8; 16]>,
      trace_id: Option<[u8; 16]>,
      gateway_signature: Option<PotrSignatureV1>,
      provider_signature: Option<PotrSignatureV1>,
  }
  ```
- Clients verify the Ed25519 gateway signature today and may verify provider
  ML-DSA signatures once governed provider keys are distributed.
- Validation checks schema version, non-zero manifest/provider identifiers,
  range ordering, timestamp ordering, success latency bounds, optional
  signature lengths, and signature payload integrity.

## Storage & Aggregation

- **Gateway tracking:** The embedded SoraFS node retains a bounded recent receipt
  history for diagnostics and proof-stream replay.
- **API:** `POST /v1/sorafs/proof/stream` with `proof_kind=potr` streams cached
  receipts filtered by manifest, provider, and tier.
- **Security:** Receipts include optional `request_id` and `trace_id` values to
  prevent replay/correlation ambiguity. Live governance archives should retain
  the original fetch transcript and the proof-stream summary.

## Reputation Oracle Integration

- Reputation plan consumes PoTR data:
  - `success_potr_i` metric = ratio of `status=success` receipts for provider `i` over rolling 7 days.
  - Missed deadlines (`status=missed_deadline`) contribute to penalty factors. Raw receipt data stored for transparency.
- Reputation process fetches receipts via the API or directly from DAG batch export:
  - Aggregator job computes latency percentiles and success ratios, emitting `PotrStatsV1`.
  - These stats feed into the reputation scoring formula (`w_potr` weight in `sorafs_reputation_plan.md`).
- **Alerts:** When a provider’s hot-tier success rate drops below 95% in the last 24h, trigger `sorafs_potr_degradation` alert and link to the reputation engine for investigation.

## Rollout Evidence Gate

Operators should keep SF-14 promotion fail-closed until payload-free deployment
evidence passes the checked-in gate:

```bash
python3 scripts/check_sorafs_potr_rollout_evidence.py \
  @scripts/examples/sorafs_potr_rollout_evidence.args.example
```

For reviewed collection planning, use the runner in dry-run mode before
executing it against captured evidence paths:

```bash
python3 scripts/run_sorafs_potr_rollout_evidence.py \
  @scripts/examples/sorafs_potr_rollout_collection.args.example \
  --dry-run
```

The checker recognizes `sorafs.potr.*` SF-14 rollout schemas for
multi-provider probes, receipt validation, proof-stream replay, reputation
integration, observability, and governance approval. It fails closed on stale
evidence, raw receipts, raw fetch transcripts, response bodies, transactions,
tokens, secrets, under-sized provider or receipt samples, missing hot/warm tier
coverage, hot/warm latency above threshold, missing gateway or provider
signature validation, missing governed ML-DSA provider key evidence, non-Norito
proof-stream routes, missing proof-stream filters, missing reputation-weight
governance, missing PoTR metrics or deadline-breach alert checks, critical
alerts, receipt summary digest drift across validation/proof-stream/reputation/
observability/governance artifacts, and governance packets not bound to
`iroha_config`. Receipt summary binding failures are recorded on the offending
artifact before required-kind validity is computed, so the JSON summary matches
the fail-closed process result.

The rollout evidence scripts have focused Python coverage in:

- `scripts/tests/check_sorafs_potr_rollout_evidence_test.py`
- `scripts/tests/run_sorafs_potr_rollout_evidence_test.py`

This status page is now a reference for the shipped local PoTR surface. Future
updates should track live rollout evidence, governed provider PQ keys, and
reputation-weight changes that pass the SF-14 gate with validation,
proof-stream, reputation, observability, and governance artifacts bound to the
same multi-provider probe receipt summary digest rather than reintroducing draft
local wiring tasks.
