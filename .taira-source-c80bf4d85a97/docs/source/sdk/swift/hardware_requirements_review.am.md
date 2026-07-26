---
lang: am
direction: ltr
source: docs/source/sdk/swift/hardware_requirements_review.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 63a907154945a6492c7a3418b4d19a5a6f6a4a8d4c9baeb6684577ad9d803f1c
source_last_modified: "2025-12-29T18:16:36.070790+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Swift Hardware Requirements Review (IOS4)

This brief schedules the hardware requirements review called out in the IOS4
roadmap. The session confirms Secure Enclave / Keychain coverage, attestation
export requirements, and dependency alignment with the Android AND2 StrongBox
initiative before the multi-algorithm signing work lands.

## Session Overview

| Field | Details |
|-------|---------|
| Date | **2026-05-15 (Friday)** |
| Time | 15:00–16:30 UTC (90 minutes) |
| Location | Zoom — `https://meet.sora.dev/ios4-hw-review` |
| Facilitator | Mei Nakamura (Swift Observability TL) |
| Co-chairs | LLM (IOS4 acting DRI), Kenji Obata (Android AND2 TL) |
| Required attendees | Swift SDK team, Android AND2 team, `iroha_crypto` maintainer, Docs/Support, Compliance, Release Engineering |
| Optional attendees | SRE governance liaison, Hardware Lab operations |
| Pre-read deadline | 2026-05-13 23:00 UTC |

## Objectives

1. Inventory supported hardware tiers (Secure Enclave, simulator fallback,
   macOS Catalyst) and map them to the `SigningKey` storage abstractions defined
   in `docs/source/sdk/swift/ios4_crypto_api_rfc.md`.
2. Align attestation evidence requirements with the Android AND2 StrongBox
   programme so governance reviews can reuse artefacts.
3. Approve the rollout matrix (devices × signing algorithms × multisig flows)
   and the automation hooks needed for IOS4 readiness gates.
4. Capture action items (CI coverage, device lab scheduling, doc updates) with
   explicit owners and due dates.

## Agenda

| Segment | Duration | Owner | Notes |
|---------|----------|-------|-------|
| Welcome / objectives | 10 min | LLM | Recap IOS4 scope, success criteria, and dependencies. |
| Hardware inventory review | 20 min | Mei Nakamura | Present Secure Enclave + macOS coverage, simulator fallbacks, and device gap analysis. |
| StrongBox lessons & attestation | 15 min | Kenji Obata | Share AND2 attestation cadence, evidence templates, and Norito metadata expectations. |
| `SigningKey` storage/telemetry walkthrough | 15 min | `iroha_crypto` maintainer | Ensure discriminants, Norito envelopes, and telemetry schemas line up. |
| Compliance & docs | 10 min | Docs/Support Manager | Confirm operator-facing requirements, update runbooks/reference docs. |
| Action item review | 10 min | LLM | Assign owners/dates, agree on follow-ups before IOS4 code freeze. |
| Q&A / buffer | 10 min | All | Holds spillover + risk review. |

## Pre-Reads & Inputs

- `docs/source/sdk/swift/ios4_crypto_api_rfc.md` (multi-algorithm API plan).
- `docs/source/sdk/swift/connect_risk_tracker.md` (Connect dependencies).
- Android AND2 StrongBox readiness deck (`docs/source/sdk/android/security.md`) and device matrix (`docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`).
- Latest device inventory / attestation evidence in `status.md` (Android section).
- Draft telemetry schema updates for `swift.crypto.signing` and
  `swift.multisig.session`.
- Swift hardware matrix + Secure Enclave attestation sample:
  - `docs/source/sdk/swift/hardware_matrix.md`
  - `docs/source/sdk/swift/attestation_sample.md`

## Required Outputs

- Updated hardware matrix & attestation checklist (to be recorded in
  `docs/source/sdk/swift/hardware_matrix.md` after the session).
- Confirmed CI device allocations (M-series Macs, iPhone/iPad variants,
  simulators) with owners for automation follow-ups.
- Action item log delivering:
  - Secure Enclave attestation export helper in `IrohaSwift`.
  - Device lab scheduling for nightly multisig smoke tests.
  - Compliance doc updates referencing the new signing/backups guidance.
- Meeting minutes + recording archived under
  `docs/source/sdk/swift/readiness/archive/2026-05/`.

## Logistics & Tracking

- Calendar invites sent 2026-04-25 to all required attendees; RSVP tracking lives
  in `docs/source/sdk/swift/readiness/archive/2026-05/hw_review_rsvp.md`.
- Notes owner: Elias Ortega (Docs/Support Manager).
- Follow-up checkpoint: include progress in the 2026-05-22 Swift status export
  and add any blockers to `roadmap.md` (IOS4 section).
