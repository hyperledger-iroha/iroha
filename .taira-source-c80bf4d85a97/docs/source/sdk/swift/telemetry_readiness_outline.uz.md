---
lang: uz
direction: ltr
source: docs/source/sdk/swift/telemetry_readiness_outline.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3ee0ad3ff2e9d3f032aa785a4c7dc40827f23cdea1e38138fab1ed89522c5672
source_last_modified: "2025-12-29T18:16:36.083827+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Swift Telemetry Redaction Readiness Outline (IOS7/IOS8)

This outline fulfils the roadmap mitigation for the **“Telemetry redaction &
readiness plan”** risk tracked under IOS7/IOS8. It mirrors the Android AND7
enablement package so the SRE governance review scheduled for **2026-03-05
16:00 UTC** has a single place to confirm curriculum, evidence, and follow-up
expectations.

## Session Logistics

- **Title:** Swift Telemetry Redaction & Observability Readiness
- **Date/Time:** 2026-03-05 16:00–17:00 UTC (60 minutes + optional 15 minute
  Q&A hangback)
- **Delivery:** Zoom — `https://meet.sora.dev/swift-telemetry-readiness`
- **Presenters:** Mei Nakamura (Swift Observability TL), Elias Ortega
  (Docs/Support Manager), LLM (acting IOS7/IOS8 DRI)
- **Audience:** SRE governance rota, Support Engineering, SDK Program PMs,
  Release Engineering, Swift QA, Compliance/privacy reviewers
- **Pre-work:** Review `docs/source/sdk/swift/telemetry_redaction.md`,
  `docs/source/sdk/swift/support_playbook.md#telemetry-and-readiness`, and
  `docs/source/sdk/swift/telemetry_chaos_checklist.md`.
- **Availability:** Calendar invites accepted (Docs/SRE sync 2026-02-20); dry-run
  pencilled for 2026-02-28 with the same presenters.

## Agenda (60 minutes)

| Segment | Duration | Presenter | Content |
|---------|----------|-----------|---------|
| Welcome & objectives | 5 min | LLM | Goals, roadmap tie-ins, success criteria |
| Policy & schema diff deep dive | 15 min | Mei Nakamura | Signal inventory, authority hashing, device/profile buckets, override guardrails |
| Runbook + override workflow | 15 min | Elias Ortega | Support playbook walkthrough, Norito override ledger, escalation flow |
| Dashboards & exporter health | 10 min | Mei Nakamura | `dashboards/mobile_parity.swift`, exporter status metrics, salt rotation alerts |
| Chaos/lab rehearsal | 10 min | LLM | Scenario sampling from `readiness/labs/swift_telemetry_lab_01.md`, validation artefacts |
| Knowledge check & Q&A | 5 min | Elias Ortega | Quiz instructions, feedback form, logistics |

## Session Materials

- **Slide deck storyboard:** `docs/source/sdk/swift/readiness/deck/telemetry_redaction_ios7.md`
  (export to slides before the live run; tracks speaker notes + demo cues).
- **Quick reference card:** `docs/source/sdk/swift/readiness/cards/swift_telemetry_redaction_qrc.md`
  (override workflow, dashboard URLs, alert checklist).
- **Lab guide:** `docs/source/sdk/swift/readiness/labs/swift_telemetry_lab_01.md`
  (maps chaos checklist scenarios to Swift tooling, Torii staging assets, and
  evidence requirements).
- **Knowledge check:** `docs/source/sdk/swift/readiness/forms/swift_telemetry_quiz_2026-03.md`
  (Google Form export + Markdown source used for sign-off).
- **Policy references:** `docs/source/sdk/swift/telemetry_redaction.md`,
  `docs/source/sdk/swift/support_playbook.md`, and
  `docs/source/sdk/swift/telemetry_chaos_checklist.md`.
- **Exporter & digest tooling:** `scripts/swift_status_export.py`,
  `scripts/swift_collect_redaction_status.py`,
  `dashboards/mobile_parity.swift`, and `ci/swift_status_export.sh`.

## Communications Log

| Date | Channel | Summary | Artefact / Notes |
|------|---------|---------|------------------|
| 2026-02-20 | Email — SRE governance list | Shared readiness plan overview, linked policy docs, requested agenda slot confirmation. | `docs/source/sdk/swift/readiness/archive/2026-03/sre_governance_brief_20260220.md` |

## Rust ↔ Swift Telemetry Briefing

| Dimension | Rust Baseline | Swift SDK Policy | Talking Points / Evidence |
|-----------|---------------|------------------|---------------------------|
| Authority identifiers | Plain Torii authorities logged in spans and metrics. | Blake2b-256 hash salted via `iroha_config.telemetry.redaction_salt`; salt rotates quarterly with override audit hooks. | Demo `swift.torii.http.request` vs `torii.http.request` records using schema diff JSON (`dashboards/data/swift_schema.sample.json`). |
| Connect aliases & sessions | Rust nodes do not hash Connect session metadata. | `session_alias_hash` + `torii_domain_hash` emitted; alias secrets stored only in Norito overrides. | Show `swift.connect.session_event` entries and reference §“Connect telemetry” in `telemetry_redaction.md`. |
| Device/profile metadata | Rust exports raw model identifiers. | Buckets (`simulator`, `iphone_small`, `iphone_large`, `ipad`, `mac_catalyst`) with SDK major version only. | Cite device profile mapping in `docs/source/sdk/mobile_device_profile_alignment.md` and screenshots captured during Lab Scenario C. |
| Overrides & redaction failures | Rust lacks override ledger. | Overrides Norito-signed via `scripts/swift_status_export.py telemetry-override …`; failures recorded as `swift.telemetry.redaction.failure`. | Walk through override log sample + support playbook guidance. |
| Offline queue telemetry | Rust nodes expose pending queue depth + Torii retries. | `swift.offline.queue_depth` gauges per queue kind (`connect`, `pipeline`) plus hashed authority for correlation. | Replay Scenario D to show parity between Android + Swift queue dashboards. |

## Lab Rehearsal Plan

Lab scenarios reuse `docs/source/sdk/swift/telemetry_chaos_checklist.md`; the
Swift-specific instructions live in
`docs/source/sdk/swift/readiness/labs/swift_telemetry_lab_01.md`.

1. **Scenario A — Salt drift injection:** Use
   `scripts/swift_collect_redaction_status.py --inject-drift` to trigger alerts,
   verify dashboards recover after salt roll.
2. **Scenario B — Override workflow:** Create/revoke overrides via
   `scripts/swift_status_export.py telemetry-override create …` and audit the log
   entries plus exporter counters.
3. **Scenario C — Exporter outage:** Pause the local OTEL collector, capture
   buffered queue telemetry, and verify alert suppression/resolution flow.
4. **Scenario D — Offline queue replay:** Run `IrohaSwift` pending queue replay
   tests, submit demo transfers, enable airplane mode on simulator, then confirm
   queue drains and metrics update when connectivity returns.
5. **Scenario E — Connect session fault:** Inject `connect.frame_latency`
   regressions using the mock dApp harness, observe alerting, and capture
   dashboards/screenshots under `readiness/screenshots/`.

## Evidence Capture & Dashboards

- Exporter health, salt gauges, override counts: `dashboards/mobile_parity.swift`
  (telemetry block) and `dashboards/mobile_ci.swift`.
- Weekly digest + Slack update: `ci/swift_status_export.sh` calling
  `scripts/swift_status_export.py telemetry`.
- Schema diffs: `scripts/telemetry/run_schema_diff.sh` (reuse Android helper) ↔
  JSON stored in `dashboards/data/swift_schema.sample.json`.
- Incident artefacts: archive logs/screenshots in
  `docs/source/sdk/swift/readiness/archive/2026-03/` and
  `docs/source/sdk/swift/readiness/screenshots/`.

## 2026-03 Session Outcomes

- Response export: `docs/source/sdk/swift/readiness/forms/responses/2026-03-and7.csv`
  (session date 2026-03-05).
- Quiz summary: `docs/source/sdk/swift/readiness/reports/202603_and7_quiz.md`
  with JSON companion `artifacts/swift/telemetry/and7_quiz/202603-summary.json`.
- Attendance log: `docs/source/sdk/swift/readiness/archive/2026-03/attendance.md`
  records roster plus pass/fail outcomes; remediation is scheduled for the lone
  <90% score.

## Post-Session Follow-Ups

1. Upload recording to `s3://sora-readiness/swift/telemetry/2026-03-05/` and add
   link to the archive README.
2. Email slides, quick reference card, lab guide, and quiz link to attendees
   within 24 h.
3. Collect quiz responses by 2026-03-10; follow up with anyone scoring <90 %;
   log outcomes in `readiness/forms/responses/2026-03.csv` (exported nightly).
4. Update `status.md` (Swift section) and the roadmap risk notes with readiness
   completion data.
5. File action items from Q&A in the IOS7/IOS8 board within two business days.

## Open Tasks

- [x] Draft readiness outline (`telemetry_readiness_outline.md`).
- [x] Create slide storyboard stub (`readiness/deck/telemetry_redaction_ios7.md`).
- [x] Draft quick reference card (`readiness/cards/swift_telemetry_redaction_qrc.md`).
- [x] Document lab scenarios (`readiness/labs/swift_telemetry_lab_01.md`).
- [x] Capture knowledge-check questions (`readiness/forms/swift_telemetry_quiz_2026-03.md`).
- [x] Record dry-run + archive notes (`readiness/archive/2026-03/dry_run_notes.md`, due 2026-02-29) — completed with timeline + action items captured from the 2026-02-28 rehearsal.
- [x] Collect rehearsal screenshots/logs (`readiness/screenshots/2026-02-28/` + `salt_drift.log`, due 2026-02-28) — Scenario A–E screenshots stored in S3 and indexed in the README.
- [x] Publish support digest update referencing readiness session (`readiness/archive/2026-03/support_digest_20260306.md`, due 2026-03-06).
