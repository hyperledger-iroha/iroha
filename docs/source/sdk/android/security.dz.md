---
lang: dz
direction: ltr
source: docs/source/sdk/android/security.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 265e4f69a6239817117db045fdadc7e1f01d862347cc47c59e6eaf41103f1246
source_last_modified: "2025-12-29T18:16:36.051677+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android Security & Compliance Evidence

This document fulfils the hardware-security acceptance criteria called out in
**AND2** (Secure Key Management & StrongBox) and **AND6** (CI smoke harness &
compliance) within `roadmap.md`. It consolidates the attestation artefacts,
device-matrix requirements, tamper-proof logging plan, and disclosure playbooks
required before Android SDK releases can advance from preview to GA/LTS.

- Use this file as the canonical reference when preparing audit packets,
  partner disclosures, and SRE readiness reviews.
- Keep the companion guides up to date:
  [`Android Key Management & Attestation Guide`](key_management.md),
  [`Android Networking & Telemetry Guide`](networking.md), and the
  [`Android Support Playbook`](../../android_support_playbook.md).

## 1. Scope & Ownership

| Item | Owner(s) | Coverage |
|------|----------|----------|
| Attestation capture & verification | Android Crypto TL · Hardware Lab Lead | `scripts/android_keystore_attestation.sh`, `scripts/android_strongbox_attestation_ci.sh`, and the readiness artefacts under `docs/source/sdk/android/readiness/`. |
| Device inventory & loaners | Hardware Lab · Partner Engineering | `android_strongbox_device_matrix.md`, `android_strongbox_capture_status.md`, loaner rotation in the support playbook. |
| Tamper-proof logs & telemetry | Telemetry/SRE · Support Engineering | `android_strongbox_attestation_run_log.md`, `telemetry_redaction.md`, associated Grafana dashboards. |
| Disclosure packs & retention | Program Lead · Compliance/Legal | `android_support_playbook.md` Sections 5–9, partner advisory templates, incident retros stored under `docs/source/compliance/android/`. |

All updates MUST be reflected in `status.md` within 24 hours and referenced in
the weekly AND2/AND6 governance sync.

## 2. Release-Stage Evidence Matrix

| Stage | Attestation Reports | Device Matrix | Tamper-Proof Logs | Disclosure Pack |
|-------|--------------------|---------------|-------------------|-----------------|
| **Preview (Q2 2026)** | Fresh StrongBox bundle per device, mock bundles allowed only if tagged `mock=true`. | Pixels 6/7/8(+a/+Pro/Fold/Tablet) + Samsung S23/S24 allocated; capture status tracked in `android_strongbox_capture_status.md`. | Per-run JSONL in `readiness/android_strongbox_attestation_run_log.md`, hashes appended to `ops/drill-log.md`. | Draft comms + runbook delta filed in `android_support_playbook.md#7-release-readiness`. |
| **Beta (Q3 2026)** | Physical bundles only, verified by `scripts/android_strongbox_attestation_ci.sh` and archived under `artifacts/android/attestation/<fleet_tag>/YYYY-MM-DD/`. | Matrix signed off by Device Lab + Partner Eng; loaner rotation noted in the support playbook. | CI log digests plus Norito telemetry export stored under `docs/source/sdk/android/readiness/archive/`. | Disclosure outline translated (EN/JA) with attestation summary + opt-out instructions. |
| **GA/LTS (Q4 2026 →)** | Bundles + verification reports zipped into the release evidence pack referenced by `docs/source/compliance/android/`. | Same inventory, plus any partner-specific SKUs; deviations require Program Lead approval. | Signed SHA256 manifests for run logs retained 3 years, cross-linked in `status.md`. | Final playbook section published; incident contact + SLA tables validated. |

## 3. Attestation Report Pipeline

1. **Capture**
   - Run `scripts/android_keystore_attestation.sh --bundle-dir artifacts/android/attestation/<fleet_tag>/<date> --alias <alias> --challenge $(scripts/android_strongbox_attestation_report.py challenge)` on each device.
   - Follow the bundle layout defined in
     `readiness/android_strongbox_attestation_bundle.md` (`chain.pem`,
     `challenge.hex`, `alias.txt`, `result.json`, optional `trust_root_*.pem`).
   - Note the `fleet_tag` from `android_strongbox_device_matrix.md`; this value
     is referenced by CI and partner playbooks.
2. **Verification**
   - `scripts/android_strongbox_attestation_ci.sh --bundle-dir ...` is the
     offline-friendly guard used in Buildkite. It validates:
     - Certificate chain rooted in the vendor trust anchor.
     - Challenge binding and expiration.
     - Security level (`STRONGBOX`, `TEE`, `SOFTWARE`) surfaced to telemetry.
   - Results feed into
     `readiness/android_strongbox_attestation_run_log.md` and the `torii_request`
     telemetry counters documented in `telemetry_redaction.md`.
3. **Publication**
   - Archive bundles beneath `artifacts/android/attestation/` and attach the
     directory SHA256 to the weekly AND2 update in `status.md`.
   - Include the most recent digest + link in
     `readiness/android_strongbox_attestation_bundle.md`.
   - During releases, copy the relevant bundles into the compliance evidence set
     tracked by `docs/source/compliance/android/and6_compliance_checklist.md`
     and record the digest inside `docs/source/compliance/android/evidence_log.csv`.

## 4. Device-Matrix Coverage

- The authoritative matrix lives in
  `readiness/android_strongbox_device_matrix.md`; update the table whenever a
  device is provisioned, retired, or repurposed.
- `readiness/android_strongbox_capture_status.md` tracks per-device capture
  progress and is the checklist referenced during weekly readiness calls.
- Hardware additions/rotations require:
  1. Capture + verify attestation bundles as above.
  2. Log provisioning notes (firmware level, enrolment status) inside the matrix.
  3. Announce in the `#android-foundations` channel with a link to the diff.
- Partner loaners (AND8 dependency) must also appear in the `Loaner Rotation`
  table inside `android_support_playbook.md`.

## 5. Tamper-Proof Logs & Telemetry

- Every attestation capture (lab or CI) appends a JSONL entry to
  `readiness/android_strongbox_attestation_run_log.md` with the SHA256 of each
  artefact plus the Buildkite job URL.
- `scripts/android_strongbox_attestation_report.py` can emit signed manifests;
  store them under `readiness/archive/<year>/` and link the hash inside the run
  log so auditors can replay the evidence.
- Telemetry/SRE must export the following metrics (documented in
  `telemetry_redaction.md`):
  - `android_sdk_strongbox_success_rate`
  - `android_sdk_strongbox_attestation_age_seconds`
  - `android_sdk_strongbox_downgrade_total{reason}`
- Tamper-proof expectations:
  - CI jobs upload run logs to the immutable bucket managed by Ops, with server
    side encryption enabled and lifecycle rules set to ≥3 years.
  - Local captures sign the JSONL file via
    `scripts/android_strongbox_attestation_report.py manifest --input run_log.jsonl --signing-key <key>`
    before appending to `ops/drill-log.md`.

## 6. Disclosure Playbooks & Incident Flow

- Partner-facing procedures live in
  `../android_support_playbook.md` (Sections 4–8). This security doc summarises
  the specific hooks that must be updated when hardware issues occur:
  1. **Detection:** Alerts from `android_sdk_strongbox_success_rate` or manual
     capture failures trigger the Sev 2/Sev 1 paths defined in the playbook.
  2. **Containment:** Record the affected `fleet_tag`, disable automatic bundle
     promotion in `scripts/android_strongbox_attestation_ci.sh`, and file a
     redaction note per the override workflow in `telemetry_redaction.md`.
  3. **Disclosure:** Within 24 hours, publish a short advisory referencing the
     affected aliases, devices, and mitigation steps. Templates reside in
     `docs/source/sdk/android/readiness/cards/`.
  4. **Follow-up:** Append the incident summary to
     `docs/source/sdk/android/readiness/android_strongbox_capture_status.md`
     and record the remediation checklist in `status.md`.
- For regulatory filings, coordinate with Compliance to store the sealed report
  in `docs/source/compliance/android/` alongside the partner acknowledgement.

## 7. Evidence Review Cadence

- **Weekly (AND2/AND6 sync):**
  - Verify at least one attestation capture ran in the past seven days.
  - Confirm device matrix deltas and capture status checkboxes.
  - Update `status.md` with bundle hashes and outstanding risks.
- **Monthly (Governance review):**
  - Export redacted run logs + telemetry snapshots and attach to the meeting
    notes.
  - Reconcile disclosure templates with any new compliance findings.
  - Ensure the support playbook reflects the latest SLA, device rotation, and
    partner requirements.
- **Release (RC/GA):**
  - Snapshot this document (git tag) and freeze the evidence pack.
  - Attach the signed manifests + disclosure plan to the GA checklist referenced
    in `docs/source/sdk/android/developer_experience_plan.md`.

## 8. Related References

- `readiness/android_strongbox_attestation_bundle.md` — bundle layout and naming.
- `readiness/android_strongbox_device_matrix.md` — hardware coverage + owners.
- `readiness/android_strongbox_attestation_run_log.md` — tamper-proof logbook.
- `readiness/android_strongbox_capture_status.md` — progress checklist + TODOs.
- `networking.md` Section 5 — Torii telemetry hooks referenced by AND4/AND7.
- `telemetry_redaction.md` — override flow and metric retention policies.
- `../android_support_playbook.md` — incident response, partner comms, SLAs.
- `key_management.md` — alias lifecycle, deterministic export, StrongBox policy.

> **Reminder:** Updating any evidence source without touching this document
> violates the roadmap guard rail. Always amend the relevant section (scope,
> pipeline, or cadence) when processes change.
