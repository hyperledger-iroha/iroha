---
lang: pt
direction: ltr
source: docs/source/sdk/android/telemetry_redaction_pre_read.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5b7fb4b860d833af60b02ccdbf82521c96380a9e0636e0f9b5b3234a793b60c4
source_last_modified: "2026-01-03T18:08:01.031867+00:00"
translation_last_reviewed: 2026-01-30
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# SRE Governance Pre-read — Android Telemetry Redaction (February 2026)

## Session Overview

- **Meeting:** Monthly SRE governance sync (February 2026)
- **Presenter:** LLM (acting AND7 owner) with Android Observability TL
- **Objective:** Secure consensus on the Android telemetry redaction policy,
  confirm rollout sequencing, and approve the enablement plan ahead of the AND7
  observability beta.
- **Duration:** 25 minutes (15-minute briefing, 10-minute discussion)
- **Date Confirmed:** 2026-02-12 14:00 UTC (SRE Governance Zoom bridge, invite
  sent 2026-01-24)
- **Pre-read Distribution:** 2026-02-05 17:00 UTC via governance mailing list

## Materials

| Artefact | Description | Owner | Status |
|----------|-------------|-------|--------|
| `telemetry_redaction.md` | Draft signal inventory, policy deltas, validation checklist | LLM, Android Observability TL | ✅ Draft complete |
| `android_support_playbook.md` diff | Redaction/override workflow updates for support staff | LLM, Docs/Support Manager | ✅ Diff merged 2026-02-03 (sections 8.1–8.4) |
| `android_runbook.md` | Operational procedures (config, overrides, incident response) | LLM | ✅ Updated 2026-02-15 (telemetry pipeline + override matrix) |
| `telemetry_redaction_pre_read.md` | Agenda + decision log (this document) | LLM | ✅ Updated 2026-01-24 |
| `telemetry_schema_diff.md` | Rust vs Android telemetry schema comparison summary | SRE privacy lead | ✅ Diff regenerated 2026-03-05 (March inventory refresh); awaiting governance sign-off |
| `telemetry_readiness_outline.md` | Session agenda plus enablement artefacts | Docs/Support Manager, SRE Enablement | ✅ Draft committed 2026-01-24 |
| `readiness/deck/telemetry_redaction_v1.md` | Slide content for enablement session | Docs/Support Manager | ✅ Draft committed 2026-01-24 |
| `readiness/cards/telemetry_redaction_qrc.md` | Quick-reference card content | Docs/Support design | ✅ Draft committed 2026-01-24 |
| `readiness/labs/telemetry_lab_01.md` | Chaos lab walkthrough and validation checklist | Android Observability TL | ✅ Draft committed 2026-01-24 |
| `readiness/forms/telemetry_quiz_2026-02.md` | Knowledge check questionnaire | LLM | ✅ Finalised 2026-02-18 (answers + storage workflow) |
| `telemetry_chaos_checklist.md` | Scenario checklist for staging validation | Android Observability TL | ✅ Draft committed 2026-01-24 |

## Agenda Proposal

1. **Current state recap (5 min)** — Why AND7 requires redaction alignment, scope
   of mobile telemetry, summary of Rust baseline.
2. **Policy walkthrough (7 min)** — Highlight hashed authority flow, device-profile
   bucketing, override process, and validation coverage.
3. **Enablement & rollout (3 min)** — Runbook updates, enablement cadence, chaos
   rehearsal milestones, and parity dashboards.
4. **Decisions & approvals (5 min)** — Seek sign-off on policy, confirm schema
   diff acceptance criteria, agree on override audit workflow.
5. **Discussion & feedback (5 min)** — Capture open questions, assign follow-ups.

## Decisions Requested

| Decision | Proposed Outcome | Notes |
|----------|------------------|-------|
| Approve Android redaction policy | ✅ Accept hashed authority, bucketed device metadata, and omission of carrier names | Requires SRE privacy lead confirmation on hashing salt handling. |
| Accept validation plan | ✅ Approve unit/integration tests + chaos rehearsal coverage | Pending schema diff report (see `telemetry_schema_diff.md`). |
| Confirm enablement cadence | ✅ Endorse quarterly refresh + 2026-02-18 recording (15:00 UTC) with Docs/Support + SRE enablement | Logistics confirmed with Docs/Support Manager (Aiko Tanaka) and SRE enablement (Rafael Bishop). |
| Confirm override governance | ✅ Adopt Norito-recorded override tokens with annual audit | Needs audit team sign-off; audit rep invited to session. |

## Discussion Prompts

- Are additional telemetry consumers (e.g., incident response tooling) affected
  by the hashed authority salt rotation schedule?
- Do support teams require a shorter retention window for override audit logs?
- Should we introduce automated alerts when Android export parity drifts from
  Rust dashboards?
- How do we coordinate schema updates with Swift/JS SDKs to keep device-profile
  bucket naming aligned?

## Timeline & Checklist

| Date | Deliverable | Status |
|------|-------------|--------|
| 2025-11-15 | Schema diff revalidated with latest configs | ✅ Output `readiness/schema_diffs/android_vs_rust-20251115.json` (tool `telemetry-schema-diff 2.0.0-rc.2.0`); Android-only signals confirmed + allowlist updated |
| 2026-01-24 | Governance invite sent; agenda slot confirmed | ✅ |
| 2026-01-29 | Draft policy inventory (`telemetry_redaction.md`) shared with SRE privacy lead | ✅ |
| 2026-02-03 | Runbook diff (`android_support_playbook.md` + `android_runbook.md`) + chaos checklist ready for review | ✅ Drafts committed; awaiting SRE comments |
| 2026-02-02 | Schema diff regenerated with release commits | ✅ Output `readiness/schema_diffs/android_vs_rust-20260202.json`; sign-off pending |
| 2026-02-05 | Pre-read packet (this doc + appendices) circulated to governance list | ✅ Sent 2026-02-05 17:02 UTC (archive: `docs/source/sdk/android/readiness/archive/2026-02/pre-read-email.eml`) |
| 2026-02-10 | Final agenda reminder & questions collected | ✅ Reminder sent 2026-02-10 09:30 UTC; questions logged below |
| 2026-02-15 | Runbook refresh shared with governance list | ✅ Updated sections 2.1/2.2/3.1/5.* circulated via pre-read packet |
| 2026-02-12 | Governance session + decision capture | ✅ Decisions recorded in `telemetry_redaction_minutes_20260212.md` |
| 2026-02-18 | Operator enablement recording (Docs/Support + SRE enablement) | ⏳ Booked 15:00 UTC |
| 2026-02-24 | Schema diff rerun with config snapshots + policy marked ready for AND7 beta | ✅ Output `readiness/schema_diffs/android_vs_rust-20260224.json`; owner tracker + summary refreshed |
| 2026-02-27 | Schema diff rerun + recommendation closure | ✅ Output `readiness/schema_diffs/android_vs_rust-20260227.json`; attestation/device-profile buckets revalidated and policy docs updated |
| 2026-03-05 | Schema diff rerun + inventory sweep after governance follow-ups | ✅ Output `readiness/schema_diffs/android_vs_rust-20260305.json`; worksheet, roadmap, and schema summary refreshed ahead of the April digest |

## Risks & Mitigations

- **Schema diff delay:** Tool output not ready in time. Mitigation: request interim
  manual review from SRE privacy lead and include comparison excerpts in the deck.
- **Enablement resource conflict:** Docs/Support bandwidth limited. Mitigation:
  availability confirmed for 2026-02-18; record session and distribute slides.
- **Override audit ambiguity:** Governance needs more detail. Mitigation:
  prepare example override ticket + Norito record snippet for meeting appendix.
- **Meeting overrun:** Additional Q&A may extend beyond 10 minutes. Mitigation:
  provide written FAQ in pre-read and park follow-ups to async doc if needed.

## Agenda Logistics

- Calendar invite: “SRE Governance — Android Telemetry Redaction Review”,
  Thursday, 2026-02-12 14:00–14:25 UTC, Zoom bridge
  `https://meet.sora.dev/sre-governance`.
- Attendees: SRE leads, Android Observability TL, Docs/Support Manager,
  SRE privacy lead, Audit liaison, Swift/JS observability reps (optional).
- Pre-read email draft stored in `docs/source/sdk/android/telemetry_pre_read_email.txt`
  (to be sent 2026-02-05 17:00 UTC).

## Action Items Before Circulation

All circulation prerequisites have been completed:

1. Latest `telemetry_redaction.md` attached to the pre-read packet with inline
   review markers (section 3.2).
2. Cross-SDK alignment questions captured in the discussion prompts above and
   shared with Swift/JS observability leads on 2026-02-05.
3. Schema diff summary (`telemetry_schema_diff.md`) updated from the
   2026-03-05 run; key findings highlighted on slide 6 of the readiness deck.
4. Artefacts archived in `docs/source/sdk/android/readiness/archive/2026-02/`
   and linked from the pre-read email template.
5. Refreshed `android_runbook.md` (sections 2.1/2.2/3.1/5.1/5.2) attached to the
   packet on 2026-02-15 with callouts in the email to ensure reviewers see the
   telemetry workflow updates.

## Post-Meeting Outcome

- Governance approved the redaction policy, validation plan, and override
  workflow without changes (see
  `docs/source/sdk/android/telemetry_redaction_minutes_20260212.md`).
- Action items assigned during the meeting are tracked in the minutes and
  mirrored in the AND7 JIRA epic.
- Enablement materials will incorporate the governance summary for the
  2026-02-18 recording.

## Governance Feedback Tracker

| Meeting Date | Decision Summary | Follow-ups | Owner |
|--------------|------------------|------------|-------|
| 2026-02-12 | Policy, validation plan, and override workflow approved. | Crash telemetry exporter review (2026-03-15); device bucket alignment with Swift/JS (2026-03-01). | LLM |

Status fields and artefact references were refreshed on 2026-02-12 following the
governance session.
