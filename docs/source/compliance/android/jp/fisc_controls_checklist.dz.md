---
lang: dz
direction: ltr
source: docs/source/compliance/android/jp/fisc_controls_checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2d8b4c90c94dddd8118fcb9c55f07c25000c6dab1f8d239570402023ab89e844
source_last_modified: "2025-12-29T18:16:35.928660+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# FISC Security Controls Checklist — Android SDK

| Field | Value |
|-------|-------|
| Version | 0.1 (2026-02-12) |
| Scope | Android SDK + operator tooling used in Japanese financial deployments |
| Owners | Compliance & Legal (Daniel Park), Android Program Lead |

## Control Matrix

| FISC Control | Implementation Detail | Evidence / References | Status |
|--------------|-----------------------|-----------------------|--------|
| **System configuration integrity** | `ClientConfig` enforces manifest hashing, schema validation, and read-only runtime access. Configuration reload failures emit `android.telemetry.config.reload` events documented in the runbook. | `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java`; `docs/source/android_runbook.md` §1–2. | ✅ Implemented |
| **Access control & authentication** | SDK honours Torii TLS policies and `/v1/pipeline` signed requests; operator workflows reference Support Playbook §4–5 for escalation plus override gating via signed Norito artefacts. | `docs/source/android_support_playbook.md`; `docs/source/sdk/android/telemetry_redaction.md` (override workflow). | ✅ Implemented |
| **Cryptographic key management** | StrongBox-preferred providers, attestation validation, and device matrix coverage ensure KMS compliance. Attestation harness outputs archived under `artifacts/android/attestation/` and tracked in the readiness matrix. | `docs/source/sdk/android/key_management.md`; `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`; `scripts/android_strongbox_attestation_ci.sh`. | ✅ Implemented |
| **Logging, monitoring, and retention** | Telemetry redaction policy hashes sensitive data, bucketises device attributes, and enforces retention (7/30/90/365-day windows). Support Playbook §8 describes dashboard thresholds; overrides recorded in `telemetry_override_log.md`. | `docs/source/sdk/android/telemetry_redaction.md`; `docs/source/android_support_playbook.md`; `docs/source/sdk/android/telemetry_override_log.md`. | ✅ Implemented |
| **Operations & change management** | GA cutover procedure (Support Playbook §7.2) plus `status.md` updates track release readiness. Release evidence (SBOM, Sigstore bundles) linked via `docs/source/compliance/android/eu/sbom_attestation.md`. | `docs/source/android_support_playbook.md`; `status.md`; `docs/source/compliance/android/eu/sbom_attestation.md`. | ✅ Implemented |
| **Incident response & reporting** | Playbook defines severity matrix, SLA response windows, and compliance notification steps; telemetry overrides + chaos rehearsals ensure reproducibility before pilots. | `docs/source/android_support_playbook.md` §§4–9; `docs/source/sdk/android/telemetry_chaos_checklist.md`. | ✅ Implemented |
| **Data residency / localisation** | Telemetry collectors for JP deployments run in approved Tokyo region; StrongBox attestation bundles stored in-region and referenced from partner tickets. Localization plan ensures docs available in Japanese before beta (AND5). | `docs/source/android_support_playbook.md` §9; `docs/source/sdk/android/developer_experience_plan.md` §5; `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`. | 🈺 In Progress (localisation ongoing) |

## Reviewer Notes

- Verify device-matrix entries for Galaxy S23/S24 before regulated partner onboarding (see readiness doc rows `s23-strongbox-a`, `s24-strongbox-a`).
- Ensure telemetry collectors in JP deployments enforce the same retention/override logic defined in the DPIA (`docs/source/compliance/android/eu/gdpr_dpia_summary.md`).
- Capture confirmation from external auditors once banking partners review this checklist.
