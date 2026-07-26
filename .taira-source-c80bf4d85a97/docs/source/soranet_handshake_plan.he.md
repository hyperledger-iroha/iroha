---
lang: he
direction: rtl
source: docs/source/soranet_handshake_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cb35136774a013bfdfac4d33c6d51b6b4648f20c958c3b3a7d98e3186d0a3782
source_last_modified: "2026-01-03T18:08:01.337514+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: SoraNet QUIC/Noise Handshake Plan (Draft)
summary: Outline for SNNet-1a handshake design and review.
---

# SoraNet Handshake Plan (Draft)

## Goals
- Decide whether to layer Noise over QUIC or use QUIC/TLS 1.3 with PQ suites.
- Document transcript binding, resumption behaviour, downgrade detection.
- Publish audit notes for relay/client implementers.

## Topics
- Capability vector hashing into Finished message.
- Dual-KDF extraction for classical + PQ inputs.
- Ticket/resumption policies.
- Grease capability TLVs.

## Transport Comparison Matrix

| Aspect | QUIC + Noise XX | QUIC + TLS 1.3 (PQ suites) |
|--------|-----------------|-----------------------------|
| PQ readiness | Native ML-KEM-768/1024 and Dilithium3 integration; no dependency on TLS library roadmaps. | Requires PQ-capable TLS stacks; hybrid suites still experimental in OpenSSL/BoringSSL. |
| Transcript control | Transcript hash defined by us, includes capability TLVs/GREASE for downgrade resistance. | TLS Finished covers standard extensions; custom TLVs require additional extensions and complex validation. |
| Implementation effort | Custom Noise framing inside a QUIC stream; deterministic behaviour under our control. | Uses existing TLS libs but needs patches for PQ suites, ECH, and deterministic padding. |
| Interop & testing | Harness can generate canonical vectors; fewer library differences. | Behaviour varies by TLS implementation, challenging for reproducible fixtures. |
| Ossification risk | GREASE + custom TLVs maintained by spec. | TLS middleboxes may block new extensions; GREASE limited to what TLS allows. |
| Decision | Adopt as primary transport (WG resolution 2026‑02‑12). | Retained as audit fallback until PQ TLS matures. |

## Crypto Review Feedback (Feb 2026)

- Reviewers: Crypto WG, Networking WG, external PQ consultant.
- Outcomes:
  1. Dual-KDF (`HKDF(classical || pq, transcript_hash)`) confirmed secure; transcript hash must include TLV ordering and GREASE bytes (already documented).
  2. ML-KEM-768 mandatory; ML-KEM-1024 optional. Classical-only clients must trigger downgrade alarms and rejection pre-KDF.
  3. Dilithium3 signatures required for descriptors/salt announcements; Ed25519 witness retained for tooling.
  4. TLS fallback permitted only for audit paths with session resumption disabled until PQ TLS stabilises.
  5. Action items: publish handshake test vectors (baseline/downgrade/GREASE), document TLS fallback audit procedure, ensure harness covers downgrade telemetry.

Feedback recorded in WG minutes; this plan now reflects the agreed baseline.
