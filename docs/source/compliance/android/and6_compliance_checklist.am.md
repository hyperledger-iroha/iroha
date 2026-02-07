---
lang: am
direction: ltr
source: docs/source/compliance/android/and6_compliance_checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2a0ce1be46f9c468915f50de5e38e2f34657b26bf4243fb5ea45dab175789393
source_last_modified: "2026-01-05T09:28:12.002460+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android AND6 Compliance Checklist

This checklist tracks the compliance deliverables that gate milestone **AND6 -
CI & Compliance Hardening**. It consolidates the regulatory artefacts requested
in `roadmap.md` and defines the storage layout under
`docs/source/compliance/android/` so Release Engineering, Support, and Legal
can reference the same evidence set before approving Android releases.

## Scope & Owners

| Area | Deliverables | Primary Owner | Backup / Reviewer |
|------|--------------|---------------|-------------------|
| EU regulatory bundle | ETSI EN 319 401 security target, GDPR DPIA summary, SBOM attestation, evidence log | Compliance & Legal (Sofia Martins) | Release Engineering (Alexei Morozov) |
| Japan regulatory bundle | FISC security controls checklist, bilingual StrongBox attestation bundles, evidence log | Compliance & Legal (Daniel Park) | Android Program Lead |
| Device lab readiness | Capacity tracking, contingency triggers, escalation log | Hardware Lab Lead | Android Observability TL |

## Artefact Matrix

| Artifact | Description | Storage Path | Refresh Cadence | Notes |
|----------|-------------|--------------|-----------------|-------|
| ETSI EN 319 401 security target | Narrative describing security objectives/assumptions for Android SDK binaries. | `docs/source/compliance/android/eu/security_target.md` | Revalidate every GA + LTS release. | Must cite build provenance hashes for the release train. |
| GDPR DPIA summary | Data protection impact assessment covering telemetry/logging. | `docs/source/compliance/android/eu/gdpr_dpia_summary.md` | Annual + before material telemetry changes. | Reference redaction policy in `sdk/android/telemetry_redaction.md`. |
| SBOM attestation | Signed SBOM plus SLSA provenance for the Gradle/Maven artifacts. | `docs/source/compliance/android/eu/sbom_attestation.md` | Every GA release. | Run `scripts/android_sbom_provenance.sh <version>` to generate CycloneDX reports, cosign bundles, and checksums. |
| FISC security controls checklist | Completed checklist mapping SDK controls to FISC requirements. | `docs/source/compliance/android/jp/fisc_controls_checklist.md` | Annual + before JP partner pilots. | Provide bilingual headings (EN/JP). |
| StrongBox attestation bundle (JP) | Per-device attestation summary + chain for JP regulators. | `docs/source/compliance/android/jp/strongbox_attestation.md` | When new hardware enters the pool. | Point to raw artefacts under `artifacts/android/attestation/<device>/`. |
| Legal sign-off memo | Counsel summary covering ETSI/GDPR/FISC scope, privacy posture, and chain of custody for attached artefacts. | `docs/source/compliance/android/eu/legal_signoff_memo.md` | Every time the artefact bundle changes or a new jurisdiction is added. | Memo references hashes from the evidence log and links to the device-lab contingency bundle. |
| Evidence log | Index of submitted artefacts with hash/timestamp metadata. | `docs/source/compliance/android/evidence_log.csv` | Updated whenever any entry above changes. | Add Buildkite link + reviewer sign-off. |
| Device-lab instrumentation bundle | Slot-specific telemetry, queue, and attestation evidence recorded with the process defined in `device_lab_instrumentation.md`. | `artifacts/android/device_lab/<slot>/` (see `docs/source/compliance/android/device_lab_instrumentation.md`) | Every reserved slot + failover drill. | Capture SHA-256 manifests and reference the slot ID in the evidence log + checklist. |
| Device-lab reservation log | Booking workflow, approvals, capacity snapshots, and escalation ladder used to keep StrongBox pools ≥80 % during freezes. | `docs/source/compliance/android/device_lab_reservation.md` | Update whenever reservations are created/changed. | Reference the `_android-device-lab` ticket IDs and weekly calendar export noted in the procedure. |
| Device-lab failover runbook & drill bundle | Quarterly rehearsal plan and artefact manifest demonstrating fallback lanes, Firebase burst queue, and external StrongBox retainer readiness. | `docs/source/compliance/android/device_lab_failover_runbook.md` + `artifacts/android/device_lab_contingency/<YYYYMMDD>-failover-drill/` | Quarterly (or after hardware roster changes). | Log drill IDs in the evidence log and attach the manifest hash + PagerDuty export noted in the runbook. |

> **Tip:** When attaching PDFs or externally signed artefacts, store a short
> Markdown wrapper in the tabled path that links to the immutable artefact in
> the governance share. This keeps the repo lightweight while preserving the
> audit trail.

## EU Regulatory Packet (ETSI/GDPR)

The EU packet ties together the three artefacts above plus the legal memo:

- Update `security_target.md` with the release identifier, Torii manifest hash,
  and SBOM digest so auditors can match binaries to the declared scope.
- Keep the DPIA summary aligned with the latest telemetry redaction policy and
  attach the Norito diff excerpt referenced in `docs/source/sdk/android/telemetry_redaction.md`.
- The SBOM attestation entry should include: CycloneDX JSON hash, provenance
  bundle hash, cosign statement, and the Buildkite job URL that generated them.
- `legal_signoff_memo.md` must capture the counsel/date, list every artefact +
  SHA-256, outline any compensating controls, and link to the evidence log row
  plus the PagerDuty ticket ID that tracked the approval.

## Japan Regulatory Packet (FISC/StrongBox)

Japan’s regulators expect a parallel bundle with bilingual documentation:

- `fisc_controls_checklist.md` mirrors the official spreadsheet; fill both the
  EN and JA columns and reference the specific section of `sdk/android/security.md`
  or the StrongBox attestation bundle that satisfies each control.
- `strongbox_attestation.md` summarises the latest runs of
  `docs/source/sdk/android/readiness/android_strongbox_attestation_bundle.md`
  (per-device JSON + Norito envelopes). Embed links to the immutable artefacts
  under `artifacts/android/attestation/<device>/` and note the rotation cadence.
- Record the bilingual cover letter template that ships with submissions inside
  `docs/source/compliance/android/jp/README.md` so Support can reuse it.
- Update the evidence log with a single row that references the checklist, the
  attestation bundle hash, and any JP partner ticket IDs tied to the delivery.

## Submission Workflow

1. **Draft** - Owner prepares the artefact, records the planned filename from
   the table above, and opens a PR containing the updated Markdown stub plus a
   checksum of the external attachment.
2. **Review** - Release Engineering confirms provenance hashes match the staged
   binaries; Compliance verifies regulatory language; Support ensures SLAs and
   telemetry policies are referenced correctly.
3. **Sign-off** - Approvers add their names and dates to the `Sign-off` table
   below. The evidence log is updated with the PR URL and Buildkite run.
4. **Publish** - After SRE governance sign-off, link the artefact in
   `status.md` and update the Android Support Playbook references.

### Sign-off Log

| Artefact | Reviewed By | Date | PR / Evidence |
|----------|-------------|------|---------------|
| *(pending)* | - | - | - |

## Device Lab Reservation & Contingency Plan

To mitigate the **device lab availability** risk called out in the roadmap:

- Track weekly capacity in `docs/source/compliance/android/evidence_log.csv`
  (column `device_lab_capacity_pct`). Alert Release Engineering if availability
  falls below 70 % for two consecutive weeks.
- Reserve StrongBox/general lanes following
  `docs/source/compliance/android/device_lab_reservation.md` ahead of every
  freeze, rehearsal, or compliance sweep so requests, approvals, and artefacts
  are captured in the `_android-device-lab` queue. Link the resulting ticket IDs
  in the evidence log when recording capacity snapshots.
- **Fallback pools:** burst to the shared Pixel pool first; if still saturated,
  schedule Firebase Test Lab smoke runs for CI validation.
- **External lab retainer:** maintain the retainer with the StrongBox partner
  lab so we can reserve hardware during freeze windows (minimum 7-day lead).
- **Escalation:** raise `AND6-device-lab` incident in PagerDuty when both the
  primary and fallback pools drop below 50 % capacity. The Hardware Lab Lead
  coordinates with SRE to re-prioritise devices.
- **Failover evidence bundles:** store every rehearsal under
  `artifacts/android/device_lab_contingency/<YYYYMMDD>/` with the reservation
  request, PagerDuty export, hardware manifest, and recovery transcript. Reference
  the bundle from `device_lab_contingency.md` and add the SHA-256 to the evidence log
  so Legal can prove the contingency workflow was exercised.
- **Quarterly drills:** exercise the runbook in
  `docs/source/compliance/android/device_lab_failover_runbook.md`, attach the
  resulting bundle path + manifest hash to the `_android-device-lab` ticket, and
  mirror the drill ID in both the contingency log and evidence log.

Document every activation of the contingency plan in
`docs/source/compliance/android/device_lab_contingency.md` (include date,
trigger, actions, and follow-ups).

## Static-Analysis Prototype

- `make android-lint` wraps `ci/check_android_javac_lint.sh`, compiling
  `java/iroha_android` and the shared `java/norito_java` sources with
  `javac --release 21 -Xlint:all -Werror` (with the flagged categories noted in
- After compilation, the script enforces the AND6 dependency policy with
  `jdeps --summary`, failing if any module outside the approved allowlist
  (`java.base`, `java.net.http`, `jdk.httpserver`) appears. This keeps the
  Android surface aligned with the SDK council’s “no hidden JDK dependencies”
  requirement before StrongBox compliance reviews.
- CI now runs the same gate via
  `.github/workflows/android-lint.yml`, which invokes
  `ci/check_android_javac_lint.sh` on every push/PR that touches the Android or
  shared Norito Java sources and uploads `artifacts/android/lint/jdeps-summary.txt`
  so compliance reviews can reference a signed module list without rerunning the
  script locally.
- Set `ANDROID_LINT_KEEP_WORKDIR=1` when you need to retain the temporary
  workspace. The script already copies the generated module summary into
  `artifacts/android/lint/jdeps-summary.txt`; set
  `ANDROID_LINT_SUMMARY_OUT=docs/source/compliance/android/evidence/android_lint_jdeps.txt`
  (or similar) when you require an additional, versioned artefact for audits.
  Engineers should still run the command locally before submitting Android PRs
  that touch Java sources and attach the recorded summary/log to compliance
  reviews. Reference it from release notes as “Android javac lint + dependency
  scan”.

## CI Evidence (Lint, Tests, Attestation)

- `.github/workflows/android-and6.yml` now runs all AND6 gates (javac lint +
  dependency scan, Android test suite, StrongBox attestation verifier, and
  device-lab slot validation) on every PR/push touching the Android surface.
- `ci/run_android_tests.sh` wraps `ci/run_android_tests.sh` and emits
  a deterministic summary at `artifacts/android/tests/test-summary.json` while
  persisting the console log to `artifacts/android/tests/test.log`. Attach both
  files to compliance packets when referencing CI runs.
- `scripts/android_strongbox_attestation_ci.sh --summary-out` produces
  `artifacts/android/attestation/ci-summary.json`, validating the bundled
  attestation chains under `artifacts/android/attestation/**` for StrongBox and
  TEE pools.
- `scripts/check_android_device_lab_slot.py --root fixtures/android/device_lab`
  verifies the sample slot (`slot-sample/`) used in CI and can be pointed at
  real runs under `artifacts/android/device_lab/<slot-id>/` with
  `--require-slot --json-out <dest>` to prove instrumentation bundles follow
  the documented layout. CI writes the validation summary to
  `artifacts/android/device_lab/summary.json`; the sample slot includes
  placeholder telemetry/attestation/queue/log extracts plus a recorded
  `sha256sum.txt` for reproducible hashes.

## Device-Lab Instrumentation Workflow

Every reservation or failover rehearsal must follow the
`device_lab_instrumentation.md` guide so telemetry, queue, and attestation
artefacts line up with the booking log:

1. **Seed slot artefacts.** Create
   `artifacts/android/device_lab/<slot>/` with the standard sub-folders and run
   `shasum` after the slot closes (see the “Artifact Layout” section of the new
   guide).
2. **Run instrumentation commands.** Execute the telemetry/queue capture,
   override digest, StrongBox harness, and lint/dependency scan exactly as
   documented so the outputs mirror CI.
3. **File evidence.** Update
   `docs/source/compliance/android/evidence_log.csv` and the reservation ticket
   with the slot ID, SHA-256 manifest path, and corresponding dashboard/Buildkite
   links.

Attach the artefact folder and the hash manifest to the AND6 release packet for
the affected freeze window. Governance reviewers will reject checklists that do
not cite a slot identifier plus the instrumentation guide.

### Reservation & Failover Readiness Evidence

Roadmap item “Regulatory artefact approvals & lab contingency” requires more
than instrumentation. Every AND6 packet must also reference the proactive
reservation workflow and quarterly failover rehearsal:

- **Reservation playbook (`device_lab_reservation.md`).** Follow the booking
  table (lead times, owners, slot lengths), export the shared calendar via
  `scripts/android_device_lab_export.py`, and record `_android-device-lab`
  ticket IDs alongside capacity snapshots in `evidence_log.csv`. The playbook
  spells out the escalation ladder and contingency triggers; copy those details
  into the checklist entry when reservations move or capacity drops below the
  80 % roadmap target.
- **Failover drill runbook (`device_lab_failover_runbook.md`).** Execute the
  quarterly rehearsal (simulate outage → promote fallback lanes → engage
  Firebase burst + external StrongBox partner) and store the artefacts under
  `artifacts/android/device_lab_contingency/<drill-id>/`. Each bundle must
  contain the manifest, PagerDuty export, Buildkite run links, Firebase burst
  report, and retainer acknowledgement noted in the runbook. Reference the
  drill ID, SHA-256 manifest, and follow-up ticket in both the evidence log and
  this checklist.

Together these documents prove that device capacity planning, outage rehearsals,
and the instrumentation bundles share the same audited trail demanded by the
roadmap and legal reviewers.

## Review Cadence

- **Quarterly** - Validate that EU/JP artefacts are up to date; refresh
  evidence log hashes; rehearse provenance capture.
- **Pre-release** - Run this checklist during every GA/LTS cutover and attach
  the completed log to the release RFC.
- **Post-incident** - If a Sev 1/2 incident touches telemetry, signing, or
  attestation, update the relevant artefact stubs with remediation notes and
  capture the reference in the evidence log.
