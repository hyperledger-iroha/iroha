---
title: Proof Streaming
summary: Stream PoR samples from gateways and collect summary metrics.
---

# SoraFS Proof Streaming

The `sorafs_cli` binary requests Proof-of-Retrievability (PoR) samples, reads
the durable status of an existing governed Proof-of-Data-Possession (PDP)
challenge, or replays signed Proof-of-Timed Retrieval (PoTR) receipts from a
Torii gateway. The streaming interface follows the single
`ProofStreamRequestV1` contract. A PDP request must bind a non-zero challenge
identifier already admitted by the authenticated provider protocol; clients
cannot synthesize challenges or choose PDP sampling inputs.

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
  Norito JSON
  matching the `ProofStreamRequestV1` schema (digest, proof kind, nonce,
  and exactly the fields allowed by the selected proof kind). A full regional
  gateway route may instead be supplied with `--gateway-url`; the retired
  `--endpoint` and textual `--provider-id` aliases are rejected.
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
  and deadline options. For PoTR pass
  `--proof-kind=potr` with `--deadline-ms=<millis>`; the stream will return the
  recorded durable receipts for the requested manifest, provider, and tier.

### PoTR HTTP headers

- Clients issue ranged fetches (`GET /v1/sorafs/storage/car/{manifest}` or
  `GET /v1/sorafs/storage/chunk/{manifest}/{digest}`) with
  `Sora-PoTR-Request: deadline=<value>;tier=<hot|warm|archive>` alongside the
  existing gateway headers. Optional `request-id=<hex>` and `trace-id=<hex>`
  parameters allow orchestrators to correlate retries deterministically.
- Gateways respond with `Sora-PoTR-Receipt` (base64-encoded Norito
  `PotrReceiptV1`) and `Sora-PoTR-Status` so clients can verify signed latency
  receipts without issuing a separate API call. Receipts include the requested
  byte range, observed/request timestamps, deterministic request IDs, and the
  gateway’s Ed25519 signature and the admitted provider’s governed ML-DSA
  signature.
- Gateways validate both signatures and the governed provider key before
  atomically recording a receipt; invalid receipts are rejected rather than
  streamed. Every PoTR stream row includes `receipt_b64`, the canonical Norito
  bytes of that final signed receipt. The CLI verifies both signatures and
  rejects any JSON identity, result, timing, tier, or trace projection that
  differs from the signed object.

### Summary structure

The final JSON summary mirrors the following layout:

```json
{
  "proof_kind": "potr",
  "requested_deadline_ms": 90000,
  "metrics": {
    "item_total": 128,
    "success_total": 126,
    "failure_total": 2,
    "failure_by_reason": {
      "invalid_proof": 1,
      "missed_deadline": 1
    },
    "latency_ms": {
      "count": 128,
      "min_ms": 38,
      "max_ms": 120,
      "p50_ms": 55,
      "p95_ms": 83,
      "average_ms": 57.9
    }
  },
  "failure_samples": [
    {
      "proof_kind": "potr",
      "result": "failure",
      "latency_ms": 120,
      "deadline_ms": 90000,
      "tier": "hot",
      "failure_reason": "missed_deadline",
      "range_start": 0,
      "range_end": 4194303,
      "requested_at_ms": 1700000100000,
      "responded_at_ms": 1700000100120,
      "recorded_at_ms": 1700000500000
    }
  ]
}
```

This data maps directly onto the metrics documented in
`docs/source/sorafs_proof_streaming_plan.md`:

- `metrics.success_total` / `metrics.failure_total` feed into
  `torii_sorafs_proof_stream_events_total{result="success|failure"}`.
- `metrics.failure_by_reason` mirrors the `reason` label carried by the same
  counter and allows quick post-processing without scraping Prometheus.
- `metrics.latency_ms` reflects `torii_sorafs_proof_stream_latency_ms_bucket`
  for the requested proof kind.

Instrumentation in `iroha_telemetry` already exposes these Prometheus counters
and histograms (`kind` label distinguishes `por` vs `potr`). The CLI summary
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
- **Promotion boundary.** Protocol and local durability coverage do not by
  themselves close the readiness lane. Promotion still requires the
  chain-authoritative repair handoff and genuine multi-provider deployment
  evidence.
