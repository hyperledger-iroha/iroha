---
lang: ur
direction: rtl
source: docs/source/soranet_spec_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9a9f47e4230c3a3f46b3e98691303f326701a8f6b695a1cc6dbeba34b3cf7791
source_last_modified: "2026-01-03T18:08:01.827023+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: SoraNet Specification Plan (Draft)
summary: Outline for SNNet-1 specification and threat model.
---

# SoraNet Specification Plan (Draft)

## Deliverables
- RFC-style document (`docs/source/soranet/spec.md`) covering:
  - Relay roles (entry/middle/exit), circuit length rules.
  - QUIC + Noise (X25519 + Kyber) handshake.
  - 1024-byte cell framing.
  - CID blinding (`blinded_cid = H(salt ∥ CID)`).
  - Capability advertisement (PQ KEM/sig, role flags).
  - Dual-slot KDF mixing classical + PQ inputs.
  - Directory publication via SoraNS/Norito.
  - Padding policy and compliance considerations.
- Threat model appendix and migration plan.

## Structure
1. Introduction & Goals
2. Terminology & Roles
3. Protocol Overview
4. Handshake Sequence (QUIC vs Noise variations)
5. Cell Framing & Flow Control
6. Capability Negotiation
7. CID Blinding & Salt Rotation
8. Directory Service & Discovery
9. Threat Model
10. Migration / Rollout Plan

## Handshake Layering Decision

- **Transport choice.** The RFC will standardise on QUIC v1 carrying a Noise XX handshake inside a
  dedicated control stream. QUIC/TLS remains available as an interop fallback but is not the normative
  path.
  - QUIC provides loss recovery, multiplexing, and connection IDs required for circuit padding without
    re-implementing congestion control.
  - Noise XX keeps capability TLVs in the transcript and allows us to exclude TLS ciphersuite negotiation
    complexity while still binding PQ and classical inputs.
- **Why not “pure Noise over UDP”?** We evaluated that approach in SNNet-1a:
  - UDP alone lacks built-in congestion control and requires re-implementing PTO; risk of ossification.
  - Network traversal (firewalls, middleboxes) favours QUIC/TLS-looking traffic; Noise-in-QUIC maintains
    indistinguishability.
- **Fallback behaviour.** The spec will require relays to expose a QUIC/TLS 1.3 endpoint with the same
  Noise payloads for audit/testing environments. Clients MAY use it when QUIC/Noise fails, but MUST log a
  downgrade event. This flow is documented in `docs/source/soranet_handshake_plan.md`.
- **Spec impact.** Section 4 of the RFC will:
  - Define stream layout (`stream 0xCAFE` for handshake, `0xCB00` for control).
  - Normatively describe payload framing, padding, and retransmission policy for both QUIC/Noise and the
    QUIC/TLS fallback.
  - Delegate implementation details to SNNet-1a (handshake) and SNNet-1b (salt) plans via references.

## Diagrams & Test Vector Deliverables

- **Sequence diagrams.** Produce Mermaid diagrams for:
  1. Client ↔ Relay handshake (QUIC/Noise baseline).
  2. Retry path when capability mismatch occurs.
  3. Salt update acknowledgement during circuit resume.
  These diagrams will live under `docs/assets/soranet/diagrams/` and be embedded in sections 4–7.
- **State machine diagrams.** Define entry/middle/exit relay state transitions (circuit creation, padding
  timers, teardown) and client recovery states (missed salt, downgrade alarm).
- **Test vectors.**
  - Handshake transcripts: JSON + hex dumps matching the fixtures enumerated in `soranet_handshake_rfc.md`
    (baseline success, downgrade rejection, GREASE preservation).
  - Capability negotiation vectors: TLV payloads for classical-only (expected failure), hybrid success, and
    GREASE extension sets. Each vector includes the transcript hash, derived traffic keys, and expected
    telemetry record.
  - Salt rotation vectors: sample `SaltAnnouncementV1` payloads plus the recovery batch used when a client
    misses two epochs.
  - Padding flow traces: capture of cover cells over a 60 s window demonstrating compliance with the 1024-byte
    framing and target cover ratio.
  A new appendix (“Appendix B — Test Vectors”) will enumerate these artifacts and link to the canonical
  files in `docs/assets/soranet/fixtures/v2/`.
- **Generation tooling.** `cargo xtask soranet-fixtures` must be extended to emit all vectors and verify
  signatures, with CI invoking it during documentation builds.

## Coordination with SNNet Subplans

- **SNNet-1a (Handshake Plan).** Ensure Section 4 references the detailed transcript binding and ticket
  policies captured in `docs/source/soranet_handshake_plan.md`. Updates to capability hashing or dual-KDF
  rules must land in both documents simultaneously. A shared checklist will track:
  - Capability TLV registry alignment (types, required flags).
  - Transcript hash formula and downgrade telemetry hooks.
- **SNNet-1b (Salt Rotation Plan).** Section 7 will cite the salt rotation procedures in
  `docs/source/soranet_salt_plan.md`. The specification will embed the final `SaltAnnouncementV1` schema,
  including governance signature requirements, and cross-link the recovery flow documented in the salt plan.
- **SNNet-1c (Capabilities Plan).** Capability TLV table in Section 6 must be derived from
  `docs/source/soranet_capabilities_plan.md`. Any new TLV types or GREASE behaviours require updating the
  plan before they appear in the spec. Establish a versioned `capability_registry.md` appendix maintained by
  the SNNet-1c team and referenced from the RFC.
- **Working group cadence.** Weekly sync between protocol authors and the SNNet subteams will review:
  - Open action items (handshake comparisons, salt recovery edge cases, GREASE telemetry).
  - Test vector status and harness coverage.
  Notes from these syncs become part of the spec changelog (`docs/source/soranet/spec.md#changelog`) so
  downstream teams can track evolved requirements.
