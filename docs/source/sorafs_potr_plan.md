---
title: PoTR-Lite Deadline Proofs Status
summary: SF-14 signed receipt capture and exact finalized proof-outcome lookup.
---

# PoTR-Lite Deadline Proofs Status

## Objectives

- Deliver timed retrieval probes for hot (≤90s) and warm (≤5min) tiers.
- Produce signed latency receipts for use in reputation and incentives.
- Surface PoTR status in routing and gateway headers.

> **Status (Jul 2026):** Torii captures PoTR receipts on ranged SoraFS gateway
> fetches and validates their invariants and signatures through
> `sorafs_manifest::potr`. The native `SubmitSorafsProofOutcome` instruction
> validates and commits an authorized final canonical receipt.
> `/v1/sorafs/proof/stream` with
> `proof_kind=potr` no longer scans an embedded receipt cache: it derives the
> exact request-scope identity from the requested manifest, provider, and
> mandatory orchestrator job ID, then returns one terminal outcome from the
> finalized chain view with complete commit provenance.
>
> A captured receipt does not become queryable until an authorized transaction
> forwarder commits it. Torii now builds `PotrFinalizedAdmissionReaderV1` from
> `PotrStateFinalizedPolicySourceV1` and its council-verified admission
> registry only after the enabled `[sorafs.por.potr_runtime]` public binding
> exactly matches the injected runtime signer roles. The binding independently
> pins both signer handles/identities/qualifications, the gateway key, the
> reader/source/resolver identities, and the complete baseline finalized
> admission anchor; it never contains credentials or private keys. Partial,
> disabled-stale, test-marked, shared, zero, non-canonical, or substituted
> bindings fail closed. This configuration and startup comparison are
> source-complete; focused and workspace Cargo validation remains pending.
> Remaining SF-14 work
> includes production forwarding/reconciliation across that boundary, genuine
> live multi-provider rollout evidence, deployment-owned HSM/KMS adapters for
> the shipped role-separated runtime signer interfaces, operator provisioning
> of the governed gateway/provider key roster and reputation-weight policy,
> focused/workspace Rust validation, and four-peer independent
> rotation/recovery/replay evidence. A process-local pending status or
> cached-receipt replay path is not a valid substitute.
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
> finalized lookup (the evidence schema retains its historical replay label),
> reputation integration, observability, and governance approval evidence. The
> builder requires reviewed deployment context, complete
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

1. Orchestrator/gateway issues a timed retrieval request with the
   `Sora-PoTR-Request` value
   `deadline=<millis>;tier=<hot|warm|archive>;request-id=<32-lowercase-hex>`
   plus an optional
   `trace-id=<32-lowercase-hex>` parameter. The non-zero 16-byte request ID is
   mandatory for a V1 final receipt and is the CLI's orchestrator job ID.
2. Gateway responds with the requested range plus `Sora-PoTR-Receipt` and
   `Sora-PoTR-Status` headers containing a base64 Norito `PotrReceiptV1` with:
   - `manifest_digest`
   - `provider_id`
   - `range_start`, `range_end`
   - `requested_at_ms`, `responded_at_ms`, `recorded_at_ms`
   - `latency_ms`
   - `deadline_ms`, `tier`, `status`
   - required `request_id`, Ed25519 gateway signature, and ML-DSA-65 provider
     signature, plus an optional `trace_id`
3. The gateway verifies the receipt, and an authorized retry-safe transaction
   forwarder submits its exact canonical bytes for ledger commitment. The
   native instruction rechecks both signatures and the active signer policy.
   Invalid, unsigned, revoked-key, or non-canonical receipts are rejected.
4. Operators read the exact finalized outcome with:

   ```bash
   sorafs_cli proof stream \
     --proof-kind=potr \
     --deadline-ms=90000 \
     --orchestrator-job-id-hex=<32-lowercase-hex> \
     --tier=hot
   ```

   The manifest and provider flags shown in the general proof-stream
   documentation are also required. The route returns one terminal row, never
   a local `pending` result or a receipt-history scan.

## Telemetry

- Proof-stream metrics:
  `torii_sorafs_proof_stream_events_total{kind="potr",result,reason}`,
  `torii_sorafs_proof_stream_latency_ms_bucket{kind="potr"}`, and
  `torii_sorafs_proof_stream_inflight{kind="potr"}`.
- Proof-health metrics:
  `torii_sorafs_proof_health_potr_breaches` and
  `torii_da_potr_bonus_micro_total`.
- Production reputation scoring must consume finalized proof-outcome events;
  live production weighting remains rollout evidence.

## Headers

- `Sora-PoTR-Request` example:
  `deadline=90000;tier=hot;request-id=<32-lowercase-hex>`
- `Sora-PoTR-Receipt: <base64 PotrReceiptV1>`
- `Sora-PoTR-Status` is one of `success`, `missed_deadline`,
  `provider_error`, `gateway_error`, or `client_cancelled`.

## Signature Scheme & Verification

- **Signature format:** `PotrReceiptV1` uses optional fields at the codec level
  so absence can be rejected explicitly, but a valid V1 final receipt requires
  both signatures: Ed25519 for the gateway and ML-DSA-65 for the provider.
  ```norito
  struct PotrReceiptV1 {
      manifest_digest: Hash,
      provider_id: ProviderId,
      tier: PotrTier,               // hot | warm | archive
      deadline_ms: u32,
      latency_ms: u32,
      status: PotrStatus,           // success | missed_deadline | provider_error
                                    // | gateway_error | client_cancelled
      requested_at_ms: Timestamp,
      responded_at_ms: Timestamp,
      recorded_at_ms: Timestamp,
      range_start: u64,
      range_end: u64,
      request_id: Option<[u8; 16]>, // Some(non-zero) required by V1 validation
      trace_id: Option<[u8; 16]>,
      gateway_signature: Option<PotrSignatureV1>,
      provider_signature: Option<PotrSignatureV1>,
  }
  ```
- Validation requires the exact algorithms and lengths, verifies both
  signatures over the same domain-separated canonical unsigned receipt, and
  binds the self-contained keys to the configured gateway trust anchor and the
  council-verified provider admission record. Self-advertised keys alone never
  authorize commitment.
- Torii no longer derives a provider ML-DSA key from the gateway stream-token
  Ed25519 seed. `PotrRuntimeSignerRolesV1` requires distinct gateway and
  provider runtime objects and stable non-zero administrative identities, plus
  separate reader/source/resolver identities and an exact non-zero baseline
  anchor. Those injected values are not self-authorizing: enabled startup also
  requires the independent `[sorafs.por.potr_runtime]` configuration and
  compares every public handle, identity, revision, digest, gateway key, and
  finalized-anchor field exactly. The provider signer revision/digest must
  equal the baseline admission sequence/digest. Configuration without injected
  roles, injected roles without enabled configuration, partial bindings,
  disabled stale fields, test-marked/shared handles, and identity collisions
  are rejected. The checked-in
  [`potr_runtime_binding.toml`](sorafs/snippets/potr_runtime_binding.toml)
  fragment shows the non-secret fields; deployment launchers keep all HSM/KMS
  credentials outside `iroha_config`.
  Torii binds `PotrStateFinalizedPolicySourceV1` to authoritative state and
  `PotrFinalizedAdmissionReaderV1` to the council-verified admission registry.
  The reader resolves the council admission before signing and
  rechecks the exact provider, policy identity/digest/sequence, finalized
  height/hash, and envelope after both signatures. A stale, revoked,
  unavailable, substituted, or mid-signature-changed policy fails closed.
  Gateway output is verified before the provider HSM is invoked, and the
  completed receipt is verified again against both governed keys. The generic
  Torii launcher supplies no gateway/provider signer roles and fails closed
  until the deployment injects those two independently administered providers.
- Validation also checks schema version, non-zero manifest/provider/request
  identifiers, range and timestamp ordering, latency/status consistency,
  bounded optional notes, and canonical encoding. The authoritative receipt
  digest covers the entire final signed receipt.

## Storage & Aggregation

- **Gateway tracking:** An embedded bounded receipt tracker may support local
  diagnostics, but it is not authoritative and is never the PoTR proof-stream
  source. It atomically persists the accepted policy identity/digest/sequence,
  finalized cursor, provider, and admission-envelope digest before any
  external handoff and restores that exact binding as the monotonic floor for
  the next live admission read.
- **Ledger identity:** The authoritative key is
  `BLAKE3("sorafs.potr.request-scope.v1\0" || manifest_digest || provider_id
  || request_id)`. A conflicting receipt for the same scope fails closed
  instead of creating a second outcome.
- **API:** `POST /v1/sorafs/proof/stream` with `proof_kind=potr` requires
  `orchestrator_job_id_hex` and performs one exact finalized lookup. Manifest,
  provider, deadline, optional tier, and signed receipt request ID must all
  match the requested scope.
- **Projection:** The NDJSON row carries `outcome_identity_hex`,
  `outcome_digest_hex`, `admission_envelope_digest_hex`,
  `finalized_block_height`, `finalized_block_hash_hex`, `committed_at_ms`, and
  the exact final `receipt_b64`. The parser rejects missing provenance and any
  JSON result/reason, timing, tier, trace, identity, or digest that disagrees
  with the signed receipt.
- **Security:** The required request ID prevents replay/correlation ambiguity;
  `trace_id` remains optional. Unknown, unfinalized, or mismatched scopes return
  no row and never fall back to embedded history.

## Reputation Oracle Integration

- Reputation plan consumes PoTR data:
  - `success_potr_i` metric = ratio of `status=success` receipts for provider `i` over rolling 7 days.
  - Missed deadlines (`status=missed_deadline`) contribute to penalty factors.
- Reputation ingestion must consume finalized proof-outcome events or another
  finalized ledger projection, not process-local receipt history:
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
multi-provider probes, receipt validation, proof-stream finalized lookup
(historically named replay in the evidence schema), reputation integration,
observability, and governance approval. It fails closed on stale
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

This status page is a reference for the native PoTR receipt and finalized-query
contract. Future updates must track the retry-safe transaction-forwarding and
reconciliation path, live rollout evidence, governed provider PQ keys, and
reputation-weight changes that pass the SF-14 gate with validation,
proof-stream, reputation, observability, and governance artifacts bound to the
same multi-provider probe receipt summary digest. Receipt-validation and
reputation artifacts must also bind to governance-approved PQ key-roster and
reputation-weight policy digests. Governance policy digests remain exposed as
`valid_policy_digests` readiness metadata from the same governed approval
artifacts.
