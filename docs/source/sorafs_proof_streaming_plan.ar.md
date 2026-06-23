---
lang: ar
direction: rtl
source: docs/source/sorafs_proof_streaming_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6a3af98cf966b3fd9927c0029d145b7e50b84b289cf6d52dcf55a4b643f86acd
source_last_modified: "2026-01-03T18:07:57.219478+00:00"
translation_last_reviewed: 2026-01-30
---

# SoraFS Proof Streaming & Monitoring Status

## Goals

- Provide streaming APIs in CLI/SDK to request/verify PoR samples and PoTR (deadline proofs).
- Emit observability data for proof success/failure, latency, and provider responses.
- Integrate with orchestrator and gateway telemetry.

> **Status (Jun 2026):** `sorafs_cli proof stream` streams PoR samples, replays
> cached PoTR receipts, writes deterministic governance-evidence bundles, and
> fails closed on gateway or local verification failures. Torii exposes the
> `/v1/sorafs/proof/stream` NDJSON endpoint with Prometheus counters,
> histograms, and in-flight gauges. PDP request construction and committed
> `fixtures/sorafs_manifest/pdp/` validator fixtures are schema-ready, but
> Torii intentionally rejects `proof_kind=pdp` until the SF-13 provider
> protocol, live provider signatures, and CDC commitment verification land.

## API Concepts

- `ProofStreamRequest`:
  - `manifest_digest`
  - `provider_id`
  - `sample_count`
  - `nonce`
- `ProofStreamResponse` (streamed items):
  - `sample_index`, `chunk_index`, `proof`, `verification_status`, `latency_ms`.

CLI commands:
- `sorafs_cli proof stream --manifest=manifest.to --provider-id-hex=<hex32> --proof-kind=por --samples=128`
- `sorafs_cli proof stream --manifest=manifest.to --provider-id-hex=<hex32> --proof-kind=potr --deadline-ms=90000`

Operator features:
- `sorafs_cli proof stream` reads NDJSON, verifies PoR samples locally, and
  records a summary JSON blob for CI/governance archives.
- `--governance-evidence-dir` copies the manifest, metadata, and proof summary
  into a deterministic evidence directory.
- `--max-failures` and `--max-verification-failures` let rehearsals tolerate a
  bounded number of expected failures; defaults remain fail-closed.

## Telemetry

- Counters: `torii_sorafs_proof_stream_events_total{kind,result,reason}`
- Histograms: `torii_sorafs_proof_stream_latency_ms_bucket{kind}`
- Gauges: `torii_sorafs_proof_stream_inflight{kind}`

## Integration Points

- Gateway endpoint `POST /v1/sorafs/proof/stream` handles PoR streams and PoTR receipt replay; PDP requests return unsupported until SF-13.
- Orchestrator and CLI request proofs after chunk fetch and archive the summary
  alongside manifests, signatures, and CAR summaries.
- CI pipelines can gate on the CLI summary blob when Prometheus export is not
  available.

## Schema Alignment (PoR/PoTR and SF-13 PDP)

- **Unified request envelope.**
  ```norito
  struct ProofStreamRequestV1 {
      manifest_digest: Hash,
      provider_id: ProviderId,
      proof_kind: ProofKind,         // Por | Pdp | Potr
      sample_count: Option<u32>,     // Required for PoR/PDP
      deadline_ms: Option<u32>,      // Required for PoTR
      nonce: [u8; 16],               // Client-supplied to prevent replay
      orchestrator_job_id: Option<Uuid>,
      tier: Option<ProofTier>,       // hot | warm | archive (maps to PDP/PoTR tiers)
  }
  enum ProofKind { Por, Pdp, Potr }
  enum ProofTier { Hot, Warm, Archive }
  ```
  This schema allows the orchestrator and CLI to route requests for PoR, PoTR, and PDP
  (SF-13) without diverging code paths. PDP requests must set `proof_kind=Pdp`, `sample_count`, and
  `tier`. PoTR requests MUST set `deadline_ms` and omit `sample_count`.
- **Streaming response items.**
  ```norito
  struct ProofStreamItemV1 {
      manifest_digest: Hash,
      provider_id: ProviderId,
      proof_kind: ProofKind,
      sample_index: Option<u32>,
      chunk_index: Option<u32>,
      receipt: ProofReceiptV1,
      verification_status: VerificationStatus,
      latency_ms: u32,
      failure_reason: Option<FailureReason>,
      trace_id: Option<Uuid>,
  }
  ```
  - For PoTR, `sample_index` is `None` and `receipt` carries the signed deadline proof (`PotrReceiptV1`).
  - For PDP, `receipt` references the CDC-based commitment proof defined in the PDP plan and includes the
    `Sora-PDP-Proof` fields (commitment root, challenge salt).
  - PoR items encode standard chunk proofs with Merkle path.
- **Telemetry hooks.** Each streamed item feeds into the counters/histograms previously listed. PDP
  failures propagate to the SF-13 slashing pipeline via the shared `FailureReason`.

## Failure Reason Taxonomy

- `timeout` — provider failed to respond within the orchestrator deadline (PoR/PDP) or breached PoTR SLA.
- `invalid_proof` — verification failed (hash mismatch, invalid Merkle path, PDP commitment mismatch).
- `admission_mismatch` — provider rejected request due to manifest/admission inconsistency.
- `token_exhausted` — stream token quota exceeded mid-stream.
- `provider_unreachable` — transport errors (connection refused, TLS failure).
- `orchestrator_aborted` — client/orchestrator cancelled the stream.
- `unsupported_capability` — provider lacks requested proof kind/tier.

These enumerations are shared with the orchestration telemetry (`failure_reason` label) so dashboards and
alerting remain consistent. The CLI/SDK map them to user-facing error messages and exit codes.

## Transport Decision

- **Primary mechanism: HTTP/2-friendly NDJSON streaming.**
  - Gateways expose `POST /v1/sorafs/proof/stream` accepting `ProofStreamRequestV1` and responding with a
    `application/x-ndjson` body (`ProofStreamItemV1` per line). HTTP/2 allows multiplexing alongside chunk
    fetches and integrates with existing gateway infrastructure.
  - Back-pressure is handled via flow control; gateways MUST not buffer more than 64 items before blocking.
  - Each response includes `Sora-Trace-Id` header so the orchestrator can correlate with OpenTelemetry spans.
- **Optional gRPC endpoint.**
  - `sorafs.proof.v1.ProofStreamService/StreamProofs` returning a bidirectional stream for environments that
    already use gRPC (internal testing, SDK integration). This mirrors the HTTP semantics and reuses the same
    Norito payloads under the hood.
- **Non-goals.** WebSocket transport is deemed unnecessary; HTTP/2 streaming satisfies bidirectional needs
  and keeps security posture aligned with existing MTLS gateways.
- **CLI implementation.**
  - The Rust CLI reads NDJSON lines, emits or suppresses per-item events,
    verifies PoR samples, enforces failure budgets, and writes the final
    metrics summary.
  - SDK streaming helpers should preserve the same request/response schema and
    failure taxonomy as they are added.

These decisions keep proof streaming coherent with the current PoR/PoTR surface
and SF-13 PDP deliverables, expose deterministic error codes, and reuse
transport already hardened in SoraFS gateways.
