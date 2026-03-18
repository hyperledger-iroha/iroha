---
lang: he
direction: rtl
source: docs/source/sorafs_proof_streaming_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6a3af98cf966b3fd9927c0029d145b7e50b84b289cf6d52dcf55a4b643f86acd
source_last_modified: "2026-01-03T18:07:57.219478+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: SoraFS Proof Streaming & Monitoring Plan (Draft)
summary: Outline for exposing PoR/PoTR streaming APIs and telemetry in CLI/SDK.
---

# SoraFS Proof Streaming & Monitoring Plan (Draft)

## Goals

- Provide streaming APIs in CLI/SDK to request/verify PoR samples and PoTR (deadline proofs).
- Emit observability data for proof success/failure, latency, and provider responses.
- Integrate with orchestrator and gateway telemetry.

> **Status (Mar 2026):** `sorafs_cli proof stream` streams PoR samples and replays cached
> PoTR receipts with structured metrics (`docs/source/sorafs_proof_streaming.md`).
> PDP streaming remains on the roadmap alongside the CDC commitment work (SF-13),
> and PoTR live probes arrive with the provider protocol extensions in SF-14.
> Prometheus export ships alongside the SF-7 telemetry deliverables. See
> `docs/source/sorafs_proof_streaming_minutes_20250305.md` for the latest design
> review decisions.

## API Concepts

- `ProofStreamRequest`:
  - `manifest_digest`
  - `provider_id`
  - `sample_count`
  - `nonce`
- `ProofStreamResponse` (streamed items):
  - `sample_index`, `chunk_index`, `proof`, `verification_status`, `latency_ms`.

CLI commands (draft):
- `sorafs stream-por --manifest manifest.to --provider <id> --samples 128`
- `sorafs stream-potr --manifest manifest.to --provider <id> --deadline 90s`

SDK features:
- Async iterator in Rust (`ProofStream`).
- Promise-based API in TS (`sorafsSDK.proof.streamPor({...})`).
- Context manager in Go.

## Telemetry

- Counters: `sorafs_proof_stream_success_total`, `sorafs_proof_stream_failure_total{reason}`
- Histograms: `sorafs_proof_stream_latency_ms_bucket`
- Gauges: `sorafs_proof_stream_inflight`

## Integration Points

- Gateway endpoints `POST /proof/{manifest_cid}` (PoR) and future PoTR API.
- Orchestrator to request proofs after chunk fetch.
- CI pipeline to run proof streaming smoke tests.

## Schema Alignment (SF-13 PDP & SF-14 PoTR)

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
  This schema allows the orchestrator and CLI to route requests for PoR (SF-9), PDP (SF-13), and PoTR
  (SF-14) without diverging code paths. PDP requests must set `proof_kind=Pdp`, `sample_count`, and
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
  - For PoTR, `sample_index` is `None` and `receipt` carries the signed deadline proof from SF-14.
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

- **Primary mechanism: HTTP/2 streaming.**
  - Gateways expose `POST /v1/proof/stream` accepting `ProofStreamRequestV1` and responding with a
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
- **CLI/SDK implementation.**
  - Rust async iterator reads NDJSON lines, verifies `trace_id`, and feeds verification pipeline.
  - TypeScript SDK uses `ReadableStream` or `AsyncGenerator` depending on runtime; Node/CDN builds rely on
    fetch streaming (WHATWG).
  - Go SDK wraps the HTTP response body in a decoder that delivers items on a channel, respecting context
    cancellation.

These decisions ensure proof streaming stays coherent with SF-13/SF-14 deliverables, exposes deterministic
error codes, and leverages transport already hardened in SoraFS gateways.
