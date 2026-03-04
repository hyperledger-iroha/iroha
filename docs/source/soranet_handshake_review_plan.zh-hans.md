---
lang: zh-hans
direction: ltr
source: docs/source/soranet_handshake_review_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 94ab415bdfea089c91bb166fc3820ceced589a2d2d89d3725ea70a3f15bca2da
source_last_modified: "2025-12-29T18:16:36.209612+00:00"
translation_last_reviewed: 2026-02-07
---

# SoraNet Handshake Working Group Prep

This note packages the current draft (`docs/source/soranet_handshake.md`) into a
meeting-ready agenda.

## Goals
- Lock the transport selection (QUIC+Noise XX vs PQ-ready TLS 1.3).
- Approve transcript hashing inputs, downgrade policy, and logging requirements.
- Finalise capability TLV registry (type IDs, semantics, GREASE policy).
- Ratify SaltAnnouncementV1 governance workflow and signature envelope.
- Agree on circuit padding parameters and acceptable overhead.
- Confirm directory consensus publishing cadence / diff format.
- Define telemetry schema, alert thresholds, and governance escalation.
- Identify fixtures required for CI (cap vectors, salt announcements, transcript hashes).

## Suggested Agenda (60 min)
1. Recap draft highlights (10 min)
2. Transport decision & transcript binding (15 min)
3. Capability TLV registry review (10 min)
4. Salt rotation & governance policy (10 min)
5. Circuit padding + telemetry hooks (10 min)
6. Next steps / owners / timelines (5 min)

## Pre-reading
- `docs/source/soranet_handshake.md`
- `roadmap.md` SNNet-1 section

## Proposed Outcomes
- Documented decisions captured back in `soranet_handshake.md`
- Action items assigned with owners and target dates (tracked in roadmap/status)
- Green light to draft full RFC text and generate signed fixtures

## Outcomes (Feb 2026)
- Chosen transport: QUIC + Noise XX with mandatory ML-KEM-768 key shares.
- Capability TLV registry finalised (0x0101 pqkem, 0x0102 pqsig, 0x0103 descriptor commit, 0x0201 role, 0x0202 padding).
- Dual Dilithium3 + Ed25519 signatures required for governance artefacts.
- Salt governance: 3-of-5 Dilithium3 council with emergency ticket logging.
- Padding/telemetry targets accepted (1024-byte cells, 18% cover-traffic budget, hourly SoraNetTelemetryV1 reports).
- Action items: draft RFC by Mar 15, 2026; reference harness by Apr 1; fixture bundle by Apr 15.
