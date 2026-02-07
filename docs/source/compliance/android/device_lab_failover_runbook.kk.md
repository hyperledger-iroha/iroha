---
lang: kk
direction: ltr
source: docs/source/compliance/android/device_lab_failover_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 473b2b49d32c32d2b884b670ba35e9aa3d0606cfd451d441a7ca927c1160311d
source_last_modified: "2025-12-29T18:16:35.923579+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android Device Lab Failover Drill Runbook (AND6/AND7)

This runbook captures the procedure, evidence requirements, and contact matrix
used when exercising the **device-lab contingency plan** referenced in
`roadmap.md` (§“Regulatory artefact approvals & lab contingency”). It complements
the reservation workflow (`device_lab_reservation.md`) and the incident log
(`device_lab_contingency.md`) so compliance reviewers, legal counsel, and SRE
have a single source of truth for how we validate failover readiness.

## Purpose & Cadence

- Demonstrate that the Android StrongBox + general device pools can fail over
  to the fallback Pixel lanes, shared pool, Firebase Test Lab burst queue, and
  external StrongBox retainer without missing AND6/AND7 SLAs.
- Produce an evidence bundle that legal can attach to ETSI/FISC submissions
  ahead of the Feb compliance review.
- Run at least once per quarter, plus any time the lab hardware roster changes
  (new devices, retirement, or maintenance longer than 24 h).

| Drill ID | Date | Scenario | Evidence Bundle | Status |
|----------|------|----------|-----------------|--------|
| DR-2026-02-Q1 | 2026-02-20 | Simulated Pixel 8 Pro lane outage + attestation backlog with AND7 telemetry rehearsal | `artifacts/android/device_lab_contingency/20260220-failover-drill/` | ✅ Completed — bundle hashes recorded in `docs/source/compliance/android/evidence_log.csv`. |
| DR-2026-05-Q2 | 2026-05-22 (scheduled) | StrongBox maintenance overlap + Nexus rehearsal | `artifacts/android/device_lab_contingency/20260522-failover-drill/` *(pending)* — `_android-device-lab` ticket **AND6-DR-202605** holds the reservations; bundle will be populated post-drill. | 🗓 Scheduled — calendar block added to “Android Device Lab – Reservations” per AND6 cadence. |

## Procedure

### 1. Pre-drill preparation

1. Confirm baseline capacity in `docs/source/sdk/android/android_strongbox_capture_status.md`.
2. Export the reservation calendar for the target ISO week via
   `python3 scripts/android_device_lab_export.py --week <ISO week>`.
3. File `_android-device-lab` ticket
   `AND6-DR-<YYYYMM>` with scope (“failover drill”), planned slots, and affected
   workloads (attestation, CI smoke, telemetry chaos).
4. Update the contingency log template in `device_lab_contingency.md` with a
   placeholder row for the drill date.

### 2. Simulate failure conditions

1. Disable or depool the primary lane (`pixel8pro-strongbox-a`) inside the lab
   scheduler and tag the reservation entry as “drill”.
2. Trigger a mock outage alert in PagerDuty (`AND6-device-lab` service) and
   capture the notification export for the evidence bundle.
3. Annotate the Buildkite jobs that normally consume the lane
   (`android-strongbox-attestation`, `android-ci-e2e`) with the drill ID.

### 3. Failover execution

1. Promote the fallback Pixel 7 lane to primary CI target and schedule the
   planned workloads against it.
2. Trigger the Firebase Test Lab burst suite via the `firebase-burst` lane for
   the retail-wallet smoke tests while StrongBox coverage moves to the shared
   lane. Capture the CLI invocation (or console export) in the ticket for audit
   parity.
3. Engage the external StrongBox lab retainer for a short attestation sweep;
   log contact acknowledgement as described below.
4. Record all Buildkite run IDs, Firebase job URLs, and retainer transcripts in
   the `_android-device-lab` ticket and the evidence bundle manifest.

### 4. Validation & rollback

1. Compare attestation/CI runtimes against baseline; flag deltas >10 % to the
   Hardware Lab Lead.
2. Restore the primary lane and update the capacity snapshot plus the readiness
   matrix once validation passes.
3. Append the final row to `device_lab_contingency.md` with trigger, actions,
   and follow-ups.
4. Update `docs/source/compliance/android/evidence_log.csv` with:
   bundle path, SHA-256 manifest, Buildkite run IDs, PagerDuty export hash, and
   reviewer sign-off.

## Evidence Bundle Layout

| File | Description |
|------|-------------|
| `README.md` | Summary (drill ID, scope, owners, timeline). |
| `bundle-manifest.json` | SHA-256 map for every file in the bundle. |
| `calendar-export.{ics,json}` | ISO-week reservation calendar from the export script. |
| `pagerduty/incident_<id>.json` | PagerDuty incident export showing alert + acknowledgement timeline. |
| `buildkite/<job>.txt` | Buildkite run URLs & logs for affected jobs. |
| `firebase/burst_report.json` | Firebase Test Lab burst execution summary. |
| `retainer/acknowledgement.eml` | Confirmation from the external StrongBox lab. |
| `photos/` | Optional photos/screenshots of lab topology if hardware was re-cabled. |

Store the bundle at
`artifacts/android/device_lab_contingency/<YYYYMMDD>-failover-drill/` and record
the manifest checksum inside the evidence log plus the AND6 compliance checklist.

## Contact & Escalation Matrix

| Role | Primary Contact | Channel(s) | Notes |
|------|-----------------|------------|-------|
| Hardware Lab Lead | Priya Ramanathan | `@android-lab` Slack · +81-3-5550-1234 | Owns on-site actions and calendar updates. |
| Device Lab Ops | Mateo Cruz | `_android-device-lab` queue | Coordinates reservation tickets + bundle uploads. |
| Release Engineering | Alexei Morozov | Release Eng Slack · `release-eng@iroha.org` | Validates Buildkite evidence + publishes hashes. |
| External StrongBox Lab | Sakura Instruments NOC | `noc@sakura.example` · +81-3-5550-9876 | Retainer contact; confirm availability within 6 h. |
| Firebase Burst Coordinator | Tessa Wright | `@android-ci` Slack | Triggers Firebase Test Lab automation when fallback is needed. |

Escalate in the following order if a drill uncovers blocking issues:
1. Hardware Lab Lead
2. Android Foundations TL
3. Program Lead / Release Engineering
4. Compliance Lead + Legal Counsel (if drill reveals regulatory risk)

## Reporting & Follow-Ups

- Link this runbook alongside the reservation procedure whenever referencing
  failover readiness in `roadmap.md`, `status.md`, and governance packets.
- Email the quarterly drill recap to Compliance + Legal with the evidence bundle
  hash table and attach the `_android-device-lab` ticket export.
- Mirror key metrics (time-to-failover, workloads restored, outstanding actions)
  inside `status.md` and the AND7 hot-list tracker so reviewers can trace the
  dependency to a concrete rehearsal.
