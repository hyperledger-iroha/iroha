---
title: SoraFS Proof Streaming & Monitoring Status
summary: Implemented PoR/PDP/PoTR streaming APIs, challenge-bound CLI requests, evidence capture, and telemetry status.
---

# SoraFS Proof Streaming & Monitoring Status

## Goals

- Provide streaming APIs in CLI/SDK to request PoR samples, governed PDP
  challenge status, and PoTR deadline receipts.
- Emit observability data for proof success/failure, latency, and provider responses.
- Integrate with orchestrator and gateway telemetry.

> **Status (Jul 2026):** `sorafs_cli proof stream` streams PoR samples, reads
> the durable status of an explicit governed PDP challenge, replays cached
> PoTR receipts, validates the exact approved native pin projection before
> every request, writes payload-free governance-evidence bundles, and fails
> closed on transport, projection, sequence, gateway, or local verification
> failures. Torii exposes the
> `/v1/sorafs/proof/stream` NDJSON endpoint plus the authenticated five-route
> PDP provider protocol with Prometheus counters, histograms, and in-flight
> gauges. PDP sampling inputs come only from the recorded challenge; clients
> must supply its non-zero `challenge_id` and cannot choose a sample count or
> seed. Proof failures use the exact-chain durable native repair-transaction
> handoff and finalized-lease-gated storage execution. Promotion remains
> blocked on removing the residual local repair manager/checkpoint consumers,
> proving one cross-peer terminal outcome, and genuine multi-provider
> deployment evidence.

## API Concepts

- `ProofStreamRequest`:
  - `manifest_digest`
  - `provider_id`
  - `challenge_id` (PDP only)
  - `sample_count` (PoR only)
  - `deadline_ms` (PoTR only)
  - `expected_finalized_height`
  - `expected_finalized_block_hash`
  - `nonce`
- `ProofStreamResponse` (streamed items):
  - `request_digest`, finalized cursor, canonical proof-kind projection, and
    the kind-specific proof or signed receipt payload.

CLI commands:
- `sorafs_cli proof stream --manifest=manifest.to --torii-url=https://torii.example --provider-id-hex=<hex32> --bearer-token-env=SORAFS_PROOF_BEARER_TOKEN --proof-kind=por --samples=128`
- `sorafs_cli proof stream --manifest=manifest.to --torii-url=https://torii.example --provider-id-hex=<hex32> --bearer-token-env=SORAFS_PROOF_BEARER_TOKEN --proof-kind=pdp --challenge-id-hex=<hex32>`
- `sorafs_cli proof stream --manifest=manifest.to --torii-url=https://torii.example --provider-id-hex=<hex32> --bearer-token-env=SORAFS_PROOF_BEARER_TOKEN --proof-kind=potr --deadline-ms=90000 --orchestrator-job-id-hex=<hex16>`

Operator features:
- `sorafs_cli proof stream` first authenticates exact native
  `GET /v1/sorafs/pin/{digest_hex}` state from the same HTTPS origin. It
  requires `Approved`, equality with the local canonical manifest, and a
  non-zero finalized cursor, then reads NDJSON and verifies the exact
  request-bound sequence through EOF.
- Torii bounds PoR `sample_count` to `1..=500`, requires a non-zero PDP
  `challenge_id`, rejects PDP client-selected sampling inputs, and requires a
  non-zero PoTR deadline before manifest lookup.
- `--governance-evidence-dir` copies the manifest, redacted origin/path
  metadata, and payload-free proof summary into an evidence directory.
- Authentication comes only from the runtime environment named by
  `--bearer-token-env`. HTTP, userinfo, queries, fragments, redirects,
  `--stream-token`, and failure-budget overrides are rejected.

## Telemetry

- Counters: `torii_sorafs_proof_stream_events_total{kind,result,reason}`
- Histograms: `torii_sorafs_proof_stream_latency_ms_bucket{kind}`
- Gauges: `torii_sorafs_proof_stream_inflight{kind}`

## Integration Points

- Gateway endpoint `POST /v1/sorafs/proof/stream` handles PoR streams,
  challenge-bound PDP status, and PoTR receipt replay. The authenticated
  `/v1/sorafs/pdp/{challenge,next,proof,status,export}` family owns challenge
  admission, work pickup, proof submission, status, and bounded export.
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
      challenge_id: Option<Hash>,    // Required for PDP; governed and non-zero
      sample_count: Option<u32>,     // Required for PoR only; 1..=500
      deadline_ms: Option<u32>,      // Required for PoTR
      sample_seed: Option<u64>,      // PoR only; folded into request-bound sampling
      expected_finalized_height: Option<u64>,
      expected_finalized_block_hash: Option<Hash>,
      nonce: [u8; 16],               // Client-supplied to prevent replay
      orchestrator_job_id: Option<Uuid>,
      tier: Option<ProofTier>,       // hot | warm | archive (maps to PDP/PoTR tiers)
  }
  enum ProofKind { Por, Pdp, Potr }
  enum ProofTier { Hot, Warm, Archive }
  ```
  The CLI always populates both finalized-cursor fields from an authenticated,
  approved native pin readback. This schema allows the orchestrator and CLI to
  route requests for PoR, PoTR, and PDP without divergent wire formats. PDP requests must set
  `proof_kind=Pdp` and `challenge_id`, and must omit `sample_count`,
  `sample_seed`, and `deadline_ms`. PoTR requests must set `deadline_ms` and
  omit challenge and sampling fields.
- **Streaming response items.**
  ```norito
  struct ProofStreamItemV1 {
      request_digest: Hash,
      manifest_digest: Hash,
      provider_id: ProviderId,
      finalized_block_height: u64,
      finalized_block_hash: Hash,
      proof_kind: ProofKind,
      leaf_index_flat: Option<u64>,
      chunk_index: Option<u32>,
      proof: Option<PorProof>,
      receipt: Option<PotrReceiptV1>,
      result: VerificationStatus,
      latency_ms: Option<u32>,
      failure_reason: Option<FailureReason>,
      trace_id: Option<Uuid>,
  }
  ```
  - For PoTR, `leaf_index_flat` is `None` and `receipt` carries the signed deadline proof (`PotrReceiptV1`).
  - For PDP, the streamed item reports the durable lifecycle and terminal
    decision for the exact recorded challenge. Canonical proof bytes enter
    through `/v1/sorafs/pdp/proof`.
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
  - The Rust CLI uses an HTTPS-only, no-redirect, no-proxy, identity-encoding
    client for the authenticated pin readback and stream on one origin. It
    consumes bounded canonical NDJSON through EOF, verifies the exact sequence,
    emits only payload-free projections when requested, requires zero
    failures, and writes the final metrics summary.
  - SDK streaming helpers should preserve the same request/response schema and
    failure taxonomy as they are added.

These decisions keep proof streaming coherent with the current PoR/PoTR surface
and SF-13 PDP deliverables, expose deterministic error codes, and reuse
transport already hardened in SoraFS gateways.
