---
lang: ja
direction: ltr
source: docs/source/sdk/android/telemetry_readiness_outline.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8898b31b8f36df421db230a30d6a96f4d78081c1b0b9c8881f5b4dcb7d025c5b
source_last_modified: "2026-01-30T17:39:41.105733+00:00"
translation_last_reviewed: 2026-01-30
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android Telemetry Redaction Readiness Outline (AND7)

This outline fulfils the roadmap mitigation for the "Telemetry redaction and
readiness plan" risk tracked under AND7. It mirrors the enablement package
referenced in `telemetry_redaction.md` and the February 2026 SRE governance
pre-read so the session has a single place to confirm curriculum, evidence, and
follow-up expectations.

## Session Logistics

- Title: Android Telemetry Redaction and Observability Readiness
- Date/Time: 2026-02-18 15:00-16:00 UTC (60 minutes + optional 15 minute Q&A)
- Delivery: Zoom - https://meet.sora.dev/android-telemetry-readiness
- Presenters: LLM (acting AND7 DRI), Android Observability TL, Docs/Support Manager
- Audience: SRE governance rota, Support Engineering, SDK Program PMs,
  Release Engineering, Android QA, Compliance/privacy reviewers
- Pre-work: Review `docs/source/sdk/android/telemetry_redaction.md`,
  `docs/source/android_support_playbook.md`, and
  `docs/source/android_runbook.md` sections 2-5.
- Availability: Calendar invites accepted (Docs/SRE sync 2026-02-10);
  dry-run pencilled for 2026-02-16 with the same presenters.

## Agenda (60 minutes)

| Segment | Duration | Presenter | Content |
|---------|----------|-----------|---------|
| Welcome and objectives | 5 min | LLM | Goals, roadmap tie-ins, success criteria |
| Policy and schema diff deep dive | 15 min | Android Observability TL | Signal inventory, authority hashing, device/profile buckets, override guardrails |
| Runbook and override workflow | 15 min | Docs/Support Manager | Support playbook walkthrough, override workflow, escalation flow |
| Dashboards and exporter health | 10 min | Android Observability TL | Schema diff outputs, parity checks, salt rotation alerts |
| Chaos/lab rehearsal | 10 min | LLM | Scenario sampling from `readiness/labs/telemetry_lab_01.md`, validation artefacts |
| Knowledge check and Q&A | 5 min | Docs/Support Manager | Quiz instructions, feedback form, logistics |

## Session Materials

- Slide deck storyboard: `docs/source/sdk/android/readiness/deck/telemetry_redaction_v1.md`
  (export to slides before the live run; tracks speaker notes and demo cues).
- Quick reference card: `docs/source/sdk/android/telemetry_redaction_quick_reference.md`
  (override workflow, dashboard URLs, alert checklist).
- Lab guide: `docs/source/sdk/android/readiness/labs/telemetry_lab_01.md`
  (maps chaos checklist scenarios to Android tooling and evidence requirements).
- Knowledge check: `docs/source/sdk/android/readiness/forms/telemetry_quiz_2026-02.md`
  (Google Form export and Markdown source used for sign-off).
- Policy references: `docs/source/sdk/android/telemetry_redaction.md`,
  `docs/source/android_support_playbook.md`, and `docs/source/android_runbook.md`.
- Override tooling: `scripts/android_override_tool.sh`,
  `scripts/telemetry/validate_override_events.py`,
  `scripts/telemetry/run_override_event_validation.sh`.

## Communications Log

| Date | Channel | Summary | Artefact / Notes |
|------|---------|---------|------------------|
| 2026-02-05 | Email - SRE governance list | Shared readiness plan overview, linked policy docs, requested agenda slot confirmation. | `docs/source/sdk/android/readiness/archive/2026-02/pre-read-email.eml` |

## Rust vs Android Telemetry Briefing

| Dimension | Rust Baseline | Android SDK Policy | Talking Points / Evidence |
|-----------|---------------|--------------------|---------------------------|
| Authority identifiers | Plain Torii authorities logged in spans and metrics. | Blake2b-256 hash salted via `android.telemetry.redaction.salt_version`; salt rotates with override audit hooks. | Demo schema diff JSON outputs under `readiness/schema_diffs/` and the hashing checks in `telemetry_redaction.md`. |
| Connect aliases and sessions | Rust nodes do not hash Connect session metadata. | Emits hashed endpoint hints and session identifiers; drops raw hostnames. | Reference `telemetry_redaction.md` section 2 and the quick reference card. |
| Device/profile metadata | Rust exports raw model identifiers. | Buckets device profile and SDK level; strips model strings. | Show device-profile mapping and schema diff checklist. |
| Overrides and redaction failures | Rust lacks an override ledger. | Overrides Norito-signed via `scripts/android_override_tool.sh`; failures recorded as `android.telemetry.redaction.failure`. | Walk through override log sample in `telemetry_override_log.md`. |
| Offline queue telemetry | Rust nodes expose pending queue depth and Torii retries. | Android exports queue depth gauges per queue kind with hashed authority correlation. | Use `telemetry_redaction.md` and runbook section 4.3 for parity notes. |

## Lab Rehearsal Plan

Lab scenarios reuse `docs/source/sdk/android/telemetry_chaos_checklist.md`; the
Android-specific instructions live in
`docs/source/sdk/android/readiness/labs/telemetry_lab_01.md`.

1. Scenario A - Salt drift injection: trigger alerts, verify dashboards recover
   after salt roll.
2. Scenario B - Override workflow: create and revoke overrides, audit the log
   entries and exporter counters.
3. Scenario C - Exporter outage: pause the local collector, capture buffered
   queue telemetry, and verify alert suppression/resolution flow.
4. Scenario D - Offline queue replay: run the pending queue replay tests, submit
   demo transfers, enable airplane mode on emulator, confirm queue drains and
   metrics update when connectivity returns.
5. Scenario E - Connect session fault: inject Connect latency regressions using
   the mock harness, observe alerting, capture screenshots under
   `readiness/screenshots/`.

## Evidence Capture and Dashboards

- Schema diffs: `scripts/telemetry/run_schema_diff.sh` with outputs stored in
  `docs/source/sdk/android/readiness/schema_diffs/`.
- Override audit evidence: `docs/source/sdk/android/readiness/override_logs/`
  plus validator outputs from `scripts/telemetry/*`.
- Lab artefacts: store logs and screenshots in
  `docs/source/sdk/android/readiness/labs/reports/<stamp>/`.
- Governance updates: post a summary to `status.md` (Android section) linking
  the archive folder and open follow-ups.

## 2026-02 Session Outcomes

- Response export: `docs/source/sdk/android/readiness/forms/responses/2026-02-and7.csv`
  (session date 2026-02-18).
- Quiz summary: `docs/source/sdk/android/readiness/reports/202602_and7_quiz.md`
  with JSON companion under `artifacts/android/telemetry/and7_quiz/`.
- Attendance log: `docs/source/sdk/android/readiness/archive/2026-02/attendance.md`.

## Post-Session Follow-Ups

1. Upload recording to `s3://sora-readiness/android/telemetry/2026-02-18/` and
   add link to the archive README.
2. Email slides, quick reference card, lab guide, and quiz link to attendees
   within 24 hours.
3. Collect quiz responses by 2026-02-25; follow up with anyone scoring <90
   percent; log outcomes in `readiness/forms/responses/2026-02.csv`.
4. Update `status.md` and the roadmap risk notes with readiness completion data.
5. File action items from Q&A in the AND7 board within two business days.

## Open Tasks

- [x] Draft readiness outline (`telemetry_readiness_outline.md`).
- [x] Create slide storyboard stub (`readiness/deck/telemetry_redaction_v1.md`).
- [x] Draft quick reference card (`telemetry_redaction_quick_reference.md`).
- [x] Document lab scenarios (`readiness/labs/telemetry_lab_01.md`).
- [x] Capture knowledge-check questions (`readiness/forms/telemetry_quiz_2026-02.md`).
- [x] Record dry-run and archive notes (`readiness/archive/2026-02/dry_run_notes.md`).
- [x] Collect rehearsal screenshots/logs (`readiness/screenshots/` + `salt_drift.log`).
- [x] Publish support digest update referencing readiness session.
