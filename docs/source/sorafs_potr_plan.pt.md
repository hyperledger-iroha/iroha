---
lang: pt
direction: ltr
source: docs/source/sorafs_potr_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 68ae2ee5f5cfb3b72dfeca266122af1ac4cb0bc24bc596068247acb26b8e3e2a
source_last_modified: "2026-07-06T19:52:04.112822+00:00"
translation_last_reviewed: 2026-07-05
source_mtime: 2026-07-04T22:57:30.469522+00:00
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
> key distribution, now represented by governance-bound key-roster and
> reputation-weight policy digests in rollout evidence, not local receipt
> capture, validation, or replay wiring.
> `scripts/check_sorafs_potr_rollout_evidence.py` now provides the fail-closed
> SF-14 rollout evidence gate, and
> `scripts/run_sorafs_potr_rollout_evidence.py` provides the reviewed
> collection planner/runner. The checker exports its required top-level
> payload fields as `EVIDENCE_REQUIRED_FIELDS`, and the planner includes the
> checker-backed `evidence_contract` map in dry-run output for the selected
> required kinds, and validates the schema-closed collection plan, required
> kinds, thresholds, external evidence map, evidence contract, and command steps
> before dry-run output or verifier execution.
> The shared runner plan guard also rejects non-canonical nested required-kind,
> threshold, external-evidence, evidence-contract, and command-step shapes before
> dry-run output or verifier execution.
> PoTR payload-safety artifacts must explicitly set `raw_receipts_included`,
> `fetch_transcripts_included`, `raw_receipt_bytes_included`,
> `response_bodies_included`, `raw_reputation_inputs_included`, and
> `critical_alerts_firing` to `false` before promotion can report ready.
> `scripts/build_sorafs_potr_canary.py` builds individual payload-free SF-14
> canary artifacts for multi-provider probes, receipt validation, proof-stream
> replay, reputation integration, observability, and governance approval
> evidence. The builder requires reviewed deployment context, complete
> hot/warm tier, proof-stream route, and metric coverage where applicable,
> rejects duplicate or unknown tier, route, and metric inputs before writing,
> derived `tier_count` for the reviewed hot/warm tier inventory,
> proof-stream `route_count` binding to the unique canonical `routes[].name`
> inventory, duplicate or unknown route rejection, receipt-summary digest bindings,
> provider and receipt minimum counts, reviewed lowercase `provider-*` provider
> labels without non-production markers, per-route `body_blake3_hex` response
> digest evidence for proof-stream routes,
> route and hot/warm latency threshold facts, governed PQ key-roster and
> reputation-weight policy digest bindings, config-backed governance metadata,
> reviewed governance policy digests surfaced as `valid_policy_digests`,
> and validates every generated artifact through
> `scripts/check_sorafs_potr_rollout_evidence.py` before writing. Checked-in
> response-file examples cover multi-provider-probe and proof-stream canaries.

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
observability/governance artifacts, PQ key-roster digest drift between receipt
validation and governance approval, reputation-weight policy digest drift
between reputation integration and governance approval, and governance packets
not bound to `iroha_config`. Valid governance approval artifacts publish their
reviewed `policy_digest_hex` values as `valid_policy_digests` for the aggregate
production-readiness gate. Route latency and hot/warm deadline latency evidence
must be non-negative integer-unit values before satisfying rollout ceilings.
Receipt summary, PQ key-roster, and reputation-weight
policy binding failures are recorded on the offending artifact before
required-kind validity is computed, so the JSON summary matches the fail-closed
process result. Multi-provider probes require `tier_count`, bind it to the
unique canonical `tiers_observed` inventory, and reject missing, inflated,
duplicate, or unknown hot/warm tier evidence before promotion. They also bind
`provider_count` to the unique canonical `providers[].name` inventory and
`receipt_count` to the unique canonical `receipts[].name` inventory, rejecting
duplicate provider or receipt labels before promotion. Provider inventory labels
must use reviewed lowercase `provider-*` IDs without non-production markers, and
receipt inventory labels must use reviewed lowercase `potr-receipt-*` labels
without non-production markers. The
proof-stream gate applies the same proof-stream `route_count` binding to the
unique canonical `routes[].name` inventory, duplicate or unknown route
rejection, and per-route
status/latency/Norito checks; every route response must include a
`body_blake3_hex` digest before proof-stream readiness can report ready.
Observability artifacts also bind `metric_count`
to the unique canonical `metrics` inventory, require the reviewed PoTR metric
set, and reject duplicate or unknown metric labels before promotion can report
ready. The summary exports the sorted reviewed `metrics` inventory plus
`metric_count_values`, and the aggregate production-readiness gate requires
those fields to match the observability artifact fingerprint before final
promotion can report ready. The PoTR gate fail-closes when more than one valid
receipt summary, PQ key roster, reputation weight policy, or governance policy
anchor appears, and clears the mixed `valid_receipt_summary_digests`,
`valid_pq_key_roster_digests`, `valid_reputation_weight_policy_digests`, or
`valid_policy_digests` set before aggregate promotion can report ready.
Aggregate promotion also rechecks the lane-proven PoTR digest relationships:
receipt-summary-bound artifact fingerprints must match
`valid_receipt_summary_digests`, PQ-key-roster-bound artifact fingerprints must
match `valid_pq_key_roster_digests`, and reputation-weight-bound artifact
fingerprints must match `valid_reputation_weight_policy_digests` before final
promotion can report ready.
The collection planner exposes those exact required payload fields
through `--dry-run` and validates the schema-closed collection plan, required
kinds, thresholds, external evidence map, evidence contract, and command steps
before contacting live PoTR services. The shared runner plan guard rejects
non-canonical nested required-kind, threshold, external-evidence,
evidence-contract, and command-step shapes before any live PoTR contact.

The rollout evidence scripts have focused Python coverage in:

- `scripts/tests/build_sorafs_potr_canary_test.py`
- `scripts/tests/check_sorafs_potr_rollout_evidence_test.py`
- `scripts/tests/run_sorafs_potr_rollout_evidence_test.py`

This status page is now a reference for the shipped local PoTR surface. Future
updates should track live rollout evidence, governed provider PQ keys, and
reputation-weight changes that pass the SF-14 gate with validation,
proof-stream, reputation, observability, and governance artifacts bound to the
same multi-provider probe receipt summary digest, plus receipt-validation and
reputation artifacts bound to governance-approved PQ key-roster and
reputation-weight policy digests, rather than reintroducing draft local wiring
tasks. Governance policy digests remain exposed as `valid_policy_digests`
readiness metadata from the same governed approval artifacts.
