---
lang: ka
direction: ltr
source: docs/source/compliance/android/eu/gdpr_dpia_summary.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8ef338a20104dc5d15094e28a1332a604b68bdcfef1ff82fea784d43fdbd10b5
source_last_modified: "2025-12-29T18:16:35.925474+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# GDPR DPIA Summary — Android SDK Telemetry (AND7)

| Field | Value |
|-------|-------|
| Assessment Date | 2026-02-12 |
| Processing Activity | Android SDK telemetry export to shared OTLP backends |
| Controllers / Processors | SORA Nexus Ops (controller), partner operators (joint controllers), Hyperledger Iroha Contributors (processor) |
| Reference Docs | `docs/source/sdk/android/telemetry_redaction.md`, `docs/source/android_support_playbook.md`, `docs/source/android_runbook.md` |

## 1. Processing Description

- **Purpose:** Provide operational telemetry needed to support AND7 observability (latency, retries, attestation health) while mirroring the Rust node schema (§2 of `telemetry_redaction.md`).
- **Systems:** Android SDK instrumentation -> OTLP exporter -> shared telemetry collector managed by SRE (see Support Playbook §8).
- **Data subjects:** Operator staff using Android SDK-based apps; downstream Torii endpoints (authority strings are hashed per telemetry policy).

## 2. Data Inventory & Mitigations

| Channel | Fields | PII Risk | Mitigation | Retention |
|---------|--------|----------|------------|-----------|
| Traces (`android.torii.http.request`, `android.torii.http.retry`) | Route, status, latency | Low (no PII) | Authority hashed with Blake2b + rotating salt; no payload bodies exported. | 7–30 days (per telemetry doc). |
| Events (`android.keystore.attestation.result`) | Alias label, security level, attestation digest | Medium (operational data) | Alias hashed (`alias_label`), `actor_role_masked` recorded for overrides with Norito audit tokens. | 90 days for success, 365 days for overrides/failures. |
| Metrics (`android.pending_queue.depth`, `android.telemetry.export.status`) | Queue counts, exporter status | Low | Aggregated counts only. | 90 days. |
| Device profile gauges (`android.telemetry.device_profile`) | SDK major, hardware tier | Medium | Bucketing (emulator/consumer/enterprise), no OEM or serial numbers. | 30 days. |
| Network context events (`android.telemetry.network_context`) | Network type, roaming flag | Medium | Carrier name dropped entirely; supports compliance requirement to avoid subscriber data. | 7 days. |

## 3. Lawful Basis & Rights

- **Lawful basis:** Legitimate interest (Art. 6(1)(f)) — ensuring reliable operation of regulated ledger clients.
- **Necessity test:** Metrics limited to operational health (no user content); hashed authority ensures parity with Rust nodes through reversible mapping available only to authorised support personnel (via override workflow).
- **Balancing test:** Telemetry is scoped to operator-controlled devices, not end-user data. Overrides require signed Norito artefacts reviewed by Support + Compliance (Support Playbook §3 + §9).
- **Data subject rights:** Operators contact Support Engineering (playbook §2) to request telemetry export/delete. Redaction overrides and logs (telemetry doc §Signal Inventory) enable fulfilment within 30 days.

## 4. Risk Assessment

| Risk | Likelihood | Impact | Residual Mitigation |
|------|------------|--------|---------------------|
| Re-identification via hashed authorities | Low | Medium | Salt rotation recorded through `android.telemetry.redaction.salt_version`; salts stored in secure vault; overrides audited quarterly. |
| Device fingerprinting via profile buckets | Low | Medium | Only tier + SDK major exported; Support Playbook prohibits escalation requests for OEM/serial data. |
| Override misuse leaking PII | Very Low | High | Norito override requests logged, expire within 24h, require SRE approval (`docs/source/android_runbook.md` §3). |
| Telemetry storage outside EU | Medium | Medium | Collectors deployed in EU + JP regions; retention policy enforced via OTLP backend configuration (documented in Support Playbook §8). |

Residual risk is deemed acceptable given the controls above and ongoing monitoring.

## 5. Actions & Follow-ups

1. **Quarterly review:** Validate telemetry schemas, salt rotations, and override logs; document in `docs/source/sdk/android/telemetry_redaction_minutes_YYYYMMDD.md`.
2. **Cross-SDK alignment:** Coordinate with Swift/JS maintainers to maintain consistent hashing/bucketing rules (tracked in roadmap AND7).
3. **Partner comms:** Include DPIA summary in partner onboarding kits (Support Playbook §9) and link to this document from `status.md`.
