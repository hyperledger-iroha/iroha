---
title: Proof Streaming
summary: Stream live PoR witnesses or read exact finalized PDP and PoTR outcomes.
---

# SoraFS Proof Streaming

The `sorafs_cli` binary requests live Proof-of-Retrievability (PoR) witnesses or
reads one exact terminal Proof-of-Data-Possession (PDP) or Proof-of-Timed
Retrieval (PoTR) outcome from finalized ledger state. PDP and PoTR responses
are not process-local scheduler status, receipt-cache scans, or pending work:
Torii executes an exact `FindSorafsProofOutcome` query against its finalized
view and returns one canonical NDJSON row.

The HTTP interface uses the schema-closed `ProofStreamHttpRequestV1` envelope.
A PDP request binds the non-zero challenge identifier admitted by the provider
protocol. A PoTR request binds a non-zero 16-byte orchestrator job identifier;
Torii derives the authoritative request-scope identity from the manifest
digest, provider ID, and job ID. Clients cannot synthesize PDP sampling inputs
or request a broad PoTR receipt history.

## CLI Usage

```bash
export TORII_URL="https://gateway.local/"
export PROVIDER_ID_HEX="1111111111111111111111111111111111111111111111111111111111111111"

sorafs_cli proof stream \
  --manifest=artifacts/manifest.to \
  --torii-url="${TORII_URL}" \
  --provider-id-hex="${PROVIDER_ID_HEX}" \
  --proof-kind=por \
  --samples=128 \
  --stream-token="$(cat stream.token)" \
  --summary-out=artifacts/proof_stream_summary.json \
  --governance-evidence-dir=artifacts/proof_stream_evidence
```

- The command POSTs to `--torii-url/v1/sorafs/proof/stream` with canonical
  Norito JSON matching `ProofStreamHttpRequestV1`: canonical lowercase
  `manifest_digest_hex` and `provider_id_hex`, the proof kind, a padded
  `nonce_b64`, and exactly the fields allowed by that proof kind. A full
  regional gateway route may instead be supplied with `--gateway-url`; the
  retired `--endpoint`, textual `--provider-id`, and `nonce_hex` forms are
  rejected.
- PoR `sample_count` is bounded to `1..=500`; oversized requests fail before
  manifest lookup so gateways do not perform unbounded sampling work.
- The request body supplies `manifest_digest_hex` (BLAKE3-256 of the canonical
  manifest) and `provider_id_hex` so gateways can resolve the stored manifest
  deterministically.
- Each streamed NDJSON line is re-emitted to STDOUT unless you pass
  `--emit-events=false`. Use this flag when piping to tooling that expects a
  single JSON summary.
- `--summary-out` writes the aggregated metrics to disk so CI pipelines can
  archive results alongside manifests, signatures, and CAR summaries.
- `--governance-evidence-dir=<dir>` copies the manifest, writes `metadata.json`
  (CLI version, resolved endpoint, manifest digest, capture timestamp), and persists the
  summary JSON in the supplied directory so release packets have ready-to-archive
  evidence for governance reviews.
- Streams now fail when any gateway item reports `result: failure` or when local
  PoR verification rejects a proof. Tune the budgets via `--max-failures=N` and
  `--max-verification-failures=N` (defaults: `0` for both) when you need to
  allow a small number of retries during rehearsals.
- `--samples` defaults to `32` for PoR and must not exceed `500`. For PDP pass
  `--proof-kind=pdp --challenge-id-hex=<64-lowercase-hex>` and omit sampling
  and deadline options. For PoTR pass `--proof-kind=potr`,
  `--deadline-ms=<millis>`, and
  `--orchestrator-job-id-hex=<32-lowercase-hex>`; Torii returns only the exact
  finalized outcome whose signed receipt carries that job ID.

## V1 response semantics

- **PoR is generated live.** The current Torii PoR branch samples the requested
  manifest from the configured local `sorafs_node` storage and emits one
  successful row per witness. Each row includes `leaf_index_flat`,
  `chunk_index`, `segment_index`, `leaf_index`, and `proof`. The client rejects
  a row unless the outer indices equal the witness indices and the witness is
  internally valid against its derived root. Supply `--por-root-hex` when the
  client must also verify against an independently trusted PoR root. PoR rows
  never carry finalized-outcome provenance.
- **PDP is an exact finalized lookup.** The lookup key is
  `(pdp, challenge_id)`. The response is one terminal `success` or `failure`
  row; there is no `pending` result. `outcome_identity_hex` equals
  `challenge_id_hex`, and the row includes the outcome digest, admission
  envelope digest, finalized block height/hash, and commit timestamp.
- **PoTR is an exact finalized lookup.** The lookup key is
  `(potr, BLAKE3("sorafs.potr.request-scope.v1\0" || manifest_digest ||
  provider_id || orchestrator_job_id))`. The single terminal row contains the
  exact canonical final signed receipt in `receipt_b64` plus the same complete
  committed provenance. The JSON manifest, provider, identity, digest,
  result/reason, deadline, latency, tier, trace, and recorded timestamp must
  agree with that receipt.

Unknown, unfinalized, or mismatched PDP/PoTR scopes return `404`; corrupt or
unavailable authoritative state returns `503`. Manifest/provider/deadline/tier/
job mismatches never fall back to a local cache or a broader scan.

### PoTR HTTP headers

- Clients issue ranged fetches (`GET /v1/sorafs/storage/car/{manifest}` or
  `GET /v1/sorafs/storage/chunk/{manifest}/{digest}`) with
  `Sora-PoTR-Request: deadline=<value>;tier=<hot|warm|archive>` alongside the
  existing gateway headers. The final V1 receipt requires a non-zero
  `request-id=<32-lowercase-hex>`; `trace-id=<32-lowercase-hex>` remains
  optional.
- Gateways respond with `Sora-PoTR-Receipt` (base64-encoded Norito
  `PotrReceiptV1`) and `Sora-PoTR-Status` so clients can verify signed latency
  receipts without issuing a separate API call. Receipts include the requested
  byte range, observed/request timestamps, deterministic request IDs, and the
  gateway’s Ed25519 signature and the admitted provider’s governed ML-DSA
  signature.
- The capture path validates the Ed25519 gateway signature and ML-DSA-65
  provider signature. An authorized retry-safe transaction forwarder must
  submit the exact receipt for ledger commitment, where the native instruction
  rechecks the signatures against the active governed policy. Invalid,
  unsigned, or self-advertised-key receipts are rejected. Every finalized PoTR
  row includes `receipt_b64`, the canonical Norito bytes of that final signed
  receipt. The CLI verifies both signatures and rejects any JSON identity,
  result, timing, tier, trace, or provenance projection that differs from the
  signed and committed object.

### Summary structure

The final JSON summary mirrors the following layout. This PDP failure example
shows the provenance present on every finalized PDP/PoTR row:

```json
{
  "proof_kind": "pdp",
  "requested_challenge_id_hex": "3333333333333333333333333333333333333333333333333333333333333333",
  "metrics": {
    "item_total": 1,
    "success_total": 0,
    "failure_total": 1,
    "failure_by_reason": {
      "deadline_expired": 1
    }
  },
  "failure_samples": [
    {
      "manifest_digest_hex": "1111111111111111111111111111111111111111111111111111111111111111",
      "provider_id_hex": "2222222222222222222222222222222222222222222222222222222222222222",
      "outcome_identity_hex": "3333333333333333333333333333333333333333333333333333333333333333",
      "outcome_digest_hex": "4444444444444444444444444444444444444444444444444444444444444444",
      "admission_envelope_digest_hex": "5555555555555555555555555555555555555555555555555555555555555555",
      "finalized_block_height": 12345,
      "finalized_block_hash_hex": "6666666666666666666666666666666666666666666666666666666666666666",
      "committed_at_ms": 1700000500000,
      "challenge_id_hex": "3333333333333333333333333333333333333333333333333333333333333333",
      "proof_kind": "pdp",
      "result": "failure",
      "failure_reason": "deadline_expired"
    }
  ]
}
```

PoTR failure samples additionally carry `receipt_b64`, `latency_ms`,
`deadline_ms`, `tier`, `recorded_at_ms`, and optional `trace_id`; those fields
must exactly project the decoded final receipt. PoR summaries contain live
witness rows and latency statistics but no committed-outcome fields.

This data maps directly onto the metrics documented in
`docs/source/sorafs_proof_streaming_plan.md`:

- `metrics.success_total` / `metrics.failure_total` feed into
  `torii_sorafs_proof_stream_events_total{result="success|failure"}`.
- `metrics.failure_by_reason` mirrors the `reason` label carried by the same
  counter and allows quick post-processing without scraping Prometheus.
- `metrics.latency_ms` reflects `torii_sorafs_proof_stream_latency_ms_bucket`
  for the requested proof kind.

Instrumentation in `iroha_telemetry` already exposes these Prometheus counters
and histograms (`kind` distinguishes `por`, `pdp`, and `potr`). The CLI summary
provides a deterministic blob for CI gating when metrics export is disabled.

## Dashboard Example

A Grafana skeleton that tracks outcome totals and latency quantiles is
available under
`docs/examples/sorafs_proof_streaming_dashboard.json`. It assumes the metrics
names listed above and can be imported directly with the Prometheus exporter enabled. The main panels include:

1. Proof outcomes (`success_total` vs `failure_total`)
2. Failure reasons split by taxonomy (timeout, invalid_proof, etc.)
3. Latency p50/p95 derived from `sorafs_proof_stream_latency_ms_bucket`

## Operational note

- **Event volume.** The CLI prints per-item NDJSON locally; set
  `--emit-events=false` when you only need the final summary blob for CI.
- **Promotion boundary.** Finalized proof-outcome queries and local protocol
  coverage do not by themselves close the readiness lane. Promotion still
  requires retry-safe terminal-outcome transaction forwarding/reconciliation,
  the chain-authoritative repair handoff, and genuine multi-provider deployment
  evidence.
