---
lang: pt
direction: ltr
source: docs/source/norito_streaming_transport_design.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 450da0d2cdd97c6b4b74f5df6cd0e2f964eabbebb0ce2505f111bbae5e5fe991
source_last_modified: "2026-01-04T15:31:22.952710+00:00"
translation_last_reviewed: 2026-01-30
---

# Norito Streaming Transport & Control Plane Design

## Purpose

This design note consolidates the transport, crypto, and observability decisions
required to unlock Roadmap Milestones D2–D7 (Norito streaming). It does not
repeat the normative codec specification in `norito_streaming.md`; instead it
anchors the concrete choices implementers must follow when wiring the control
plane, QUIC transport, and telemetry surfaces in the Rust workspace.

## Scope

- Hybrid public-key encryption (HPKE) profile selection, key lifetimes, and
  handshake framing in `StreamingSession`.
- QUIC capability negotiation covering stream/DATAGRAM usage, priority, and
  deterministic fallbacks.
- Forward-error-correction (FEC) feedback equations, timers, and invariants
  that guarantee identical redundancy decisions across hardware.
- Telemetry emission rules that preserve per-tenant privacy while supplying the
  counters referenced by operator tooling and roadmap acceptance criteria.

The choices below are binding for `iroha_p2p`, `iroha_crypto::streaming`,
`norito::streaming`, and the operator-facing documentation shipped in Milestones
D3–D7.

## HPKE Profile Choices

### Goals

- Provide post-quantum forward secrecy without sacrificing deterministic
  behaviour across nodes.
- Keep the on-wire format identical to the current
  `StreamingSession::snapshot_state` payload.
- Avoid suite negotiation in the critical path; all peers must converge on the
  same cipher suite list.

### Selected Profiles

Publishers and viewers MUST implement two HPKE suites in the order listed:

1. `mode = AuthPsk`, `kem = Kyber768`, `kdf = HKDF-SHA3-256`,
   `aead = ChaCha20Poly1305`.
2. `mode = AuthPsk`, `kem = Kyber1024`, `kdf = HKDF-SHA3-256`,
   `aead = ChaCha20Poly1305`.

Suite #1 is mandatory-to-use; suite #2 is reserved for future hardware where
larger ciphertexts are acceptable. Nodes advertise support via the capability
bitset defined in [QUIC Capability Negotiation](#quic-capability-negotiation);
however the negotiation is degenerate today because every deployment ships
Suite #1. Implementations MAY enable Suite #2 only when both peers set
`streaming_capabilities.hpke_kyber1024 = true`, otherwise the publisher MUST
fall back to Suite #1.

### Pre-Shared Key (PSK) Source

The PSK is derived deterministically from the Norito manifest commit hash:

```
psk = BLAKE3("nsc-hpke-psk" || manifest.commitment_hash)
psk_id = manifest.commitment_hash[0..15]
```

The `psk_id` is embedded in the control frame so viewers can verify they derived
the same PSK; mismatches trigger `StreamingProtocolError::PskMismatch`. Because
the manifest hash is immutable, this derivation guarantees that all peers for
the same broadcast share the PSK without out-of-band exchange.

### Rotation & Counters

- `StreamingSession::key_counter` increments per signed `KeyUpdate`. The cadence
  policy remains unchanged from `norito_streaming.md` section 5.4: rotate every 64 MiB
  of encrypted payload or 5 minutes, whichever comes first.
- Publishers MUST regenerate the HPKE ephemeral key pair per update and encode
  it in the control frame's `HpkePayload`.
- Viewers MUST reject any `KeyUpdate` whose counter is <= the last accepted
  counter to keep the stream monotonic.
- Sessions snapshot the negotiated suite identifier (`0x0001` for Suite #1,
  `0x0002` for Suite #2). Restoring a snapshot replays the counter and suite so
  resumed viewers cannot accidentally select a different profile.

### Determinism Notes

- Kyber encapsulation uses the deterministic variant described in FIPS 203
  Section 8.5: the random coins are derived from
  `BLAKE3("nsc-kyber-coins" || session_id || key_counter || role)`. This keeps
  signatures reproducible across restarts and hardware.
- The ChaCha20Poly1305 nonce maps to `sequence_number` emitted by the QUIC
  DATAGRAM layer; both peers zero-pad the 96-bit nonce and XOR with a shared
  salt derived from the HPKE context, yielding deterministic AEAD inputs.

## QUIC Capability Negotiation

### Control Stream Layout

- Stream `0x0` carries a Norito-encoded `TransportCapabilities` frame during the
  QUIC handshake. The frame enumerates:
  - `hpke_suite_mask` (bit 0 = Kyber768, bit 1 = Kyber1024).
  - `supports_datagram` (boolean).
  - `max_segment_datagram_size` (bytes, deterministic upper bound).
  - `fec_feedback_interval_ms`.
  - `privacy_bucket_granularity` (mirrors telemetry rules below).
- Peers MUST exchange the frame exactly once per direction. The first frame from
  the publisher MUST precede any media DATAGRAM; viewers MUST NOT emit feedback
  before processing this frame.

### Capability Resolution

1. Both peers parse `TransportCapabilities`.
2. The resolved HPKE suite equals the lowest-index bit set in both masks. If no
   bits intersect, the viewer rejects the connection with
   `TransportError::MissingHpkeSuite`.
3. `supports_datagram` resolves to true only when both endpoints set the flag.
   When false, publishers send segment chunks on ordered bidirectional streams
   with the priority weights described below.
4. `max_segment_datagram_size` resolves to the minimum value advertised. The
   publisher MUST fragment chunks accordingly so all viewers see identical
   packetization.
5. `fec_feedback_interval_ms` resolves to the maximum value advertised. Slower
   feedback keeps compute budgets consistent even when viewers run on slow
   hardware.

Baseline deployments ship `streaming.feature_bits = 0b11`, advertising feedback
hints (bit 0) alongside privacy-overlay provider support (bit 1). Operators can
lower the mask when disabling privacy overlay or experimental features; viewers
requesting bits outside the advertised mask MUST be rejected by the handshake.

### Stream & DATAGRAM Usage

- When DATAGRAM is enabled, each segment window maps to a contiguous range of
  DATAGRAM sequence numbers. Chunks are sent first, followed by parity shards.
  Publishers MUST NOT skip sequence numbers; padding DATAGRAMs containing only
  the session ID are inserted when needed to preserve determinism.
- When falling back to streams, publishers open a single unidirectional stream
  per segment window. Chunks and parity shards are framed in-order with a
  deterministic header (`chunk_id`, `is_parity`, `payload_len`). Stream offsets
  remain strictly increasing so retransmissions are QUIC-managed.
- Control messages (`KeyUpdate`, `FeedbackHint`, `ReceiverReport`) travel on a
  dedicated bidirectional stream with priority weight 256. Media streams use
  weight 128, and archival/manifest streams use weight 64. This hierarchy keeps
  rekey and feedback deterministic under scheduler pressure.

### Capability Hashing

To guarantee manifest reproducibility, the resolved capability tuple
`(suite_id, datagram, max_dgram, fec_interval)` feeds the manifest's
`transport_capabilities_hash`, computed as
`BLAKE3("nsc-transport-capabilities" || le_u16(suite_id) || u8(datagram) || le_u16(max_dgram) || le_u16(fec_interval))`.
Genesis manifests and on-chain validators compare this hash when admitting new
publishers.

## Deterministic FEC Feedback

### Feedback Messages

- Viewers emit `FeedbackHint` frames every `fec_feedback_interval_ms`, aligned
  to multiples of that interval from the start of the session. Each hint
  contains `(loss_ewma, latency_gradient, observed_rtt_ms)`.
- Every third hint (i.e., every `3 * fec_feedback_interval_ms`) viewers also
  emit a `ReceiverReport` that adds `delivered_sequence`, `parity_applied`, and
  `fec_budget`.
- Publishers persist the aggregated loss EWMA and viewer counters inside
  `StreamingSession::feedback_state`. The `ManifestPublisher`
  (`iroha_core::streaming::ManifestPublisher`) wraps
  `StreamingHandle::populate_manifest` when building outbound manifests, first
  injecting the negotiated capability hash and then deriving the monotonic
  parity plus cadence for the accompanying `FeedbackHintFrame`, while falling
  back gracefully when no viewer feedback is available yet.
- Real-time conferencing uses the `Kaigi` ISIs (`CreateKaigi`,
  `JoinKaigi`, `LeaveKaigi`, `EndKaigi`, `RecordKaigiUsage`) to
  orchestrate Nexus-hosted rooms. Call state lives in domain metadata under the
  `kaigi__` prefix so manifests, billing, and participant updates remain
  deterministic and discoverable by standard metadata subscribers.

### Loss Estimation

- The exponential moving average uses `alpha = 0.2`. Viewers maintain the EWMA
  over packet loss measured per interval, derived from DATAGRAM sequence gaps or
  QUIC ACK ranges when using streams.
- To avoid floating-point divergence, the EWMA is computed in fixed-point Q16.16
  space:

```
loss_ewma_fp = loss_ewma_fp + ((sample_fp - loss_ewma_fp) * alpha_fp) >> 16
```

where `alpha_fp = round(0.2 * 2^16) = 13107`. Publishers mirror the same math
when integrating the feedback.

### Redundancy Equation

Publishers compute parity for the next window of `w = 12` chunks using:

```
parity = clamp(ceil((loss_ewma * 1.25 + 0.005) * w), 0, 6)
```

All arithmetic executes in Q16.16; the `ceil` is implemented as
`(value_fp + 0xFFFF) >> 16`. The floor constant `0.005` equals `327` in Q16.16.
If the parity result differs from the previous window, publishers log the change
to the telemetry sink before applying it. Publishers NEVER decrease parity
within the same session; reductions take effect only when the session ID (or
manifest) changes.

### Timer Alignment

- Publishers sample feedback on their local wall clock but align window
  boundaries to the first `FeedbackHint` arrival. If a hint is delayed beyond
  `2 * fec_feedback_interval_ms`, the publisher reuses the last known values and
  increments an `feedback_timeout_total` counter.
- Viewers use the publisher's transport epoch (first `KeyUpdate` timestamp) to
  align their hint schedule. This ensures hints line up even if viewers join
  late.

## Telemetry Redaction Rules

### Aggregation Requirements

- All per-session identifiers are hashed with
  `BLAKE3("nsc-telemetry-session" || session_id)` before they leave the node.
- Counters exposed via Prometheus or OTLP MUST group data into the following
  buckets:
  - Loss EWMA buckets: `[0, 0.01)`, `[0.01, 0.05)`, `[0.05, 0.1)`, `[0.1, 0.2)`,
    `[0.2, 0.4)`, `[0.4, 1.0]`.
  - Latency gradient buckets: `< -5 ms`, `[-5, -1)`, `[-1, 1)`, `[1, 5)`,
    `[5, 10)`, `≥ 10`.
  - FEC budget buckets: `0`, `1`, `2`, `3`, `4`, `≥ 5`.
- No raw viewer addresses, QUIC connection IDs, or per-tenant manifest hashes
  appear in telemetry outputs. Nodes instead report aggregate counts per
  manifest by hashing the manifest ID with the same BLAKE3 domain separator as
  above.

### Operator-Facing Metrics

- `streaming_hpke_rekeys_total{suite_id}` increments per accepted `KeyUpdate`.
- `streaming_quic_datagrams_sent_total` and
  `streaming_quic_datagrams_dropped_total` track publisher egress/drop counts,
  regardless of DATAGRAM support.
- `streaming_fec_parity_current` exports the latest parity decision per session
  in the bucketed form above.
- `streaming_feedback_timeout_total` increments whenever the publisher doubles
  a feedback interval without receiving a hint.
- `streaming_privacy_redaction_fail_total` increments when a telemetry payload
  cannot be emitted because the redaction rules would leak raw identifiers. The
  counter MUST remain at zero during normal operation and is a CI guardrail.

### Storage & Retention

- Nodes retain raw feedback samples only in the encrypted session snapshot
  (`StreamingSession::snapshot_state`). Snapshots are deleted when a manifest
  expires or after 36 hours, whichever comes first.
- Long-term observability relies on aggregated telemetry exported at most once
  per minute to avoid deanonymising low-volume streams.

## Compliance Checklist

Implementations targeting Milestones D2-D7 MUST:

- Use the HPKE suites and deterministic derivations defined above.
- Implement QUIC capability negotiation with the exact resolution rules.
- Apply the fixed-point FEC feedback loop and never diverge from the parity
  equation.
- Emit telemetry only through the redacted counters and buckets listed above.
- Document deviations under the appropriate roadmap milestone; do not ship features with outstanding blockers (e.g., log items in the D8 backlog).

## Implementation Anchors

- `crates/iroha_crypto/src/streaming.rs` implements `StreamingSession`,
  deterministic cadence tracking, HPKE suite handling, and the feedback state
  machine described in this note.
- `crates/iroha_p2p/src/streaming/quic.rs` carries the `TransportCapabilities`
  negotiation, DATAGRAM fallbacks, and priority rules mandated above.
- `integration_tests/tests/norito_streaming_feedback.rs` exercises the fixed
  EWMA/parity equations and the viewer→publisher control-flow invariants.
- `docs/source/norito_streaming.md` summarises the operator guidance and links
  back to this design note for normative requirements.
- `crates/iroha_audio` centralises the deterministic audio encoder/decoder used
  by the streaming pipeline and hosts the golden vectors that Milestone D4
  requires.
- `crates/iroha_core/src/telemetry.rs` exposes `StreamingTelemetry`, wiring
  HPKE rekeys, content-key rotations, and parity buckets into Prometheus.

Keep these references synchronized with future behaviour changes.
