---
title: Proof Streaming
summary: Stream PoR samples from gateways and collect summary metrics.
---

# SoraFS Proof Streaming

The `sorafs_cli` binary can now request Proof-of-Retrievability (PoR) samples
or replay recorded Proof-of-Timed Retrieval (PoTR) receipts from a Torii gateway
and emit structured metrics so operators can monitor proof quality alongside the
rest of their pipeline. The streaming interface aligns with the unified
`ProofStreamRequestV1` schema and will extend to PDP once SF-13 lands.

## CLI Usage

```bash
export TORII_URL="https://gateway.local/"

sorafs_cli proof stream \
  --manifest artifacts/manifest.to \
  --torii-url "${TORII_URL}" \
  --provider-id provider::alpha \
  --samples=128 \
  --stream-token="$(cat stream.token)" \
  --summary-out artifacts/proof_stream_summary.json \
  --governance-evidence-dir artifacts/proof_stream_evidence
```

- The command POSTs to `--torii-url/v2/sorafs/proof/stream` with a Norito payload
  matching the `ProofStreamRequestV1` schema (digest, proof kind, nonce,
  and either `sample_count` or `deadline_ms` depending on the proof kind).
- The request body supplies `manifest_digest_hex` (BLAKE3-256 of the canonical
  manifest) and `provider_id_hex` so gateways can resolve the stored manifest
  deterministically.
- Each streamed NDJSON line is re-emitted to STDOUT unless you pass
  `--emit-events=false`. Use this flag when piping to tooling that expects a
  single JSON summary.
- `--summary-out` writes the aggregated metrics to disk so CI pipelines can
  archive results alongside manifests, signatures, and CAR summaries.
- `--governance-evidence-dir=<dir>` copies the manifest, writes `metadata.json`
  (CLI version, Torii URL, manifest digest, capture timestamp), and persists the
  summary JSON in the supplied directory so release packets have ready-to-archive
  evidence for governance reviews.
- Streams now fail when any gateway item reports `result: failure` or when local
  PoR verification rejects a proof. Tune the budgets via `--max-failures=N` and
  `--max-verification-failures=N` (defaults: `0` for both) when you need to
  allow a small number of retries during rehearsals.
- `--samples` defaults to `32` for PoR. For PoTR pass `--proof-kind=potr`
  with `--deadline-ms=<millis>`; the stream will return the recorded receipts
  currently cached by the gateway.

### PoTR HTTP headers

- Clients issue ranged fetches (`GET /v2/sorafs/storage/car/{manifest}` or
  `GET /v2/sorafs/storage/chunk/{manifest}/{digest}`) with
  `Sora-PoTR-Request: deadline=<value>;tier=<hot|warm|archive>` alongside the
  existing gateway headers. Optional `request-id=<hex>` and `trace-id=<hex>`
  parameters allow orchestrators to correlate retries deterministically.
- Gateways respond with `Sora-PoTR-Receipt` (base64-encoded Norito
  `PotrReceiptV1`) and `Sora-PoTR-Status` so clients can verify signed latency
  receipts without issuing a separate API call. Receipts include the requested
  byte range, observed/request timestamps, deterministic request IDs, and the
  gateway’s Ed25519 signature. Provider signatures remain optional until
  Dilithium key distribution lands.
- Gateways validate receipt signatures before caching them; receipts with invalid
  signatures are dropped instead of being streamed to clients.

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

This data maps directly onto the metrics planned in
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
names listed above and can be imported directly once the Prometheus exporter
lands. The main panels include:

1. Proof outcomes (`success_total` vs `failure_total`)
2. Failure reasons split by taxonomy (timeout, invalid_proof, etc.)
3. Latency p50/p95 derived from `sorafs_proof_stream_latency_ms_bucket`

## Limitations & Roadmap

- **Provider signatures.** Gateways attach Ed25519 receipts today; Dilithium3
  provider attestations remain on the roadmap and will be threaded once council
  distributes PQ keys to operators (tracked under SF-14 follow-ups).
- **PDP streaming.** Proof-of-Data-Possession support remains on the roadmap
  alongside the CDC commitment work (SF-13). The CLI will add `proof_kind=pdp`
  once the provider protocol and CDC commitments ship.
- **Event volume.** The CLI prints per-item NDJSON locally; set
  `--emit-events=false` when you only need the final summary blob for CI.
