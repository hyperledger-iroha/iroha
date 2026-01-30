---
lang: he
direction: rtl
source: docs/source/compliance/android/eu/legal_signoff_memo.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8bb3e19ca5eb661d202b5e3b9cd118207ded277e8ff717e16a342b71e7a67857
source_last_modified: "2026-01-03T18:07:59.200257+00:00"
translation_last_reviewed: 2026-01-30
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# AND6 EU Legal Sign-off Memo Template

This memo records the legal review required by roadmap item **AND6** before the
EU (ETSI/GDPR) artefact packet is submitted to regulators. Counsel should clone
this template per release, populate the fields below, and store the signed copy
alongside the immutable artefacts referenced in the memo.

## Summary

- **Release / Train:** `<e.g., 2026.1 GA>`
- **Review date:** `<YYYY-MM-DD>`
- **Counsel / Reviewer:** `<name + organisation>`
- **Scope:** `ETSI EN 319 401 security target, GDPR DPIA summary, SBOM attestation`
- **Associated tickets:** `<governance or legal issue IDs>`

## Artefact Checklist

| Artefact | SHA-256 | Location / Link | Notes |
|----------|---------|-----------------|-------|
| `security_target.md` | `<hash>` | `docs/source/compliance/android/eu/security_target.md` + governance archive | Confirm release identifiers & threat model adjustments. |
| `gdpr_dpia_summary.md` | `<hash>` | Same directory / localization mirrors | Ensure redaction policy references match `sdk/android/telemetry_redaction.md`. |
| `sbom_attestation.md` | `<hash>` | Same directory + cosign bundle in evidence bucket | Verify CycloneDX + provenance signatures. |
| Evidence log row | `<hash>` | `docs/source/compliance/android/evidence_log.csv` | Row number `<n>` |
| Device-lab contingency bundle | `<hash>` | `artifacts/android/device_lab_contingency/<YYYYMMDD>/*.tgz` | Confirms failover rehearsal tied to this release. |

> Attach additional rows if the packet contains more files (for example, privacy
> appendices or DPIA translations). Every artefact must reference its immutable
> upload target and the Buildkite job that produced it.

## Findings & Exceptions

- `None.` *(Replace with bullet list covering residual risks, compensating
  controls, or required follow-up actions.)*

## Approval

- **Decision:** `<Approved / Approved with conditions / Blocked>`
- **Signature / Timestamp:** `<digital signature or email reference>`
- **Follow-up owners:** `<team + due date for any conditions>`

Upload the final memo to the governance evidence bucket, copy the SHA-256 into
`docs/source/compliance/android/evidence_log.csv`, and link the upload path in
`status.md`. If the decision is “Blocked,” escalate to the AND6 steering
committee and document remediation steps in both the roadmap hot-list and the
device-lab contingency log.
