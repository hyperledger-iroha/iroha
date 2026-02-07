---
lang: hy
direction: ltr
source: docs/source/compliance/android/eu/legal_signoff_memo_2026-02.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: eb92b77765ced36213a0bde55581f29d59c262f398c658f35a1fb43a182fe296
source_last_modified: "2025-12-29T18:16:35.926476+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# AND6 EU Legal Sign-off Memo — 2026.1 GA (Android SDK)

## Summary

- **Release / Train:** 2026.1 GA (Android SDK)
- **Review date:** 2026-04-15
- **Counsel / Reviewer:** Sofia Martins — Compliance & Legal
- **Scope:** ETSI EN 319 401 security target, GDPR DPIA summary, SBOM attestation, AND6 device-lab contingency evidence
- **Associated tickets:** `_android-device-lab` / AND6-DR-202602, AND6 governance tracker (`GOV-AND6-2026Q1`)

## Artefact Checklist

| Artefact | SHA-256 | Location / Link | Notes |
|----------|---------|-----------------|-------|
| `security_target.md` | `385d17a55579d2b0b365e21090ee081ded79e44655690b2abfbf54068c9b55b0` | `docs/source/compliance/android/eu/security_target.md` | Matches 2026.1 GA release identifiers and threat model deltas (Torii NRPC additions). |
| `gdpr_dpia_summary.md` | `8ef338a20104dc5d15094e28a1332a604b68bdcfef1ff82fea784d43fdbd10b5` | `docs/source/compliance/android/eu/gdpr_dpia_summary.md` | References AND7 telemetry policy (`docs/source/sdk/android/telemetry_redaction.md`). |
| `sbom_attestation.md` | `c2e0de176d4bb8c8e09329e2b9ee5dd93228d3f0def78225c1d8b777a5613f2d` | `docs/source/compliance/android/eu/sbom_attestation.md` + Sigstore bundle (`android-sdk-release#4821`). | CycloneDX + provenance reviewed; matches Buildkite job `android-sdk-release#4821`. |
| Evidence log | `0b2d2f9eddada06faa70620f608c3ad1ec38f378d2cbddc24b15d0a83fcc381d` | `docs/source/compliance/android/evidence_log.csv` (row `android-device-lab-failover-20260220`) | Confirms log captured bundle hashes + capacity snapshot + memo entry. |
| Device-lab contingency bundle | `faf32356dfc0bbca1459b14d75f3306ea1c10cb40f3180fe1758ac5105016f85` | `artifacts/android/device_lab_contingency/20260220-failover-drill/` | Hash taken from `bundle-manifest.json`; ticket AND6-DR-202602 recorded hand-off to Legal/Compliance. |

## Findings & Exceptions

- No blocking issues identified. Artefacts align with ETSI/GDPR requirements; AND7 telemetry parity noted in DPIA summary and no additional mitigations required.
- Recommendation: monitor scheduled DR-2026-05-Q2 drill (ticket AND6-DR-202605) and append resulting bundle to the evidence log before the next governance checkpoint.

## Approval

- **Decision:** Approved
- **Signature / Timestamp:** _Sofia Martins (digitally signed via governance portal, 2026-04-15 14:32 UTC)_
- **Follow-up owners:** Device Lab Ops (deliver DR-2026-05-Q2 evidence bundle before 2026-05-31)
