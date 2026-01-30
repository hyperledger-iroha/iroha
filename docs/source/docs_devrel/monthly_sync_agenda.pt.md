---
lang: pt
direction: ltr
source: docs/source/docs_devrel/monthly_sync_agenda.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a2f89131efc0c79ddf63d71a25c04029014ba58393fb6336e676181322bc5066
source_last_modified: "2026-01-03T18:08:00.500077+00:00"
translation_last_reviewed: 2026-01-30
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Docs/DevRel Monthly Sync Agenda

This agenda formalizes the monthly Docs/DevRel sync that is referenced across
`roadmap.md` (see “Add localization staffing review to monthly Docs/DevRel
sync”) and the Android AND5 i18n plan. Use it as the canonical checklist, and
update it whenever roadmap deliverables add or retire agenda items.

## Cadence & Logistics

- **Frequency:** monthly (typically the second Thursday, 16:00 UTC)
- **Duration:** 45 minutes + optional 15 minute hang-back for deep dives
- **Location:** Zoom (`https://meet.sora.dev/docs-devrel-sync`) with shared
  notes in HackMD or `docs/source/docs_devrel/minutes/<yyyy-mm>.md`
- **Audience:** Docs/DevRel manager (chair), Docs engineers, localization
  program manager, SDK DX TLs (Android, Swift, JS), Product Docs, Release
  Engineering delegate, Support/QA observers
- **Facilitator:** Docs/DevRel manager; appoint a rotating scribe who will
  commit the minutes into the repo within 24 hours

## Pre-Work Checklist

| Owner | Task | Artefact |
|-------|------|----------|
| Scribe | Create the month’s notes file (`docs/source/docs_devrel/minutes/<yyyy-mm>.md`) using the template below. | Notes file |
| Localization PM | Refresh `docs/source/sdk/android/i18n_plan.md#translation-status` and the staffing log; pre-fill proposed decisions. | i18n plan |
| DX TLs | Run `ci/check_android_docs_i18n.sh` or `scripts/sync_docs_i18n.py --dry-run` and attach digests for discussion. | CI artefacts |
| Docs tooling | Export `docs/i18n/manifest.json` digests + outstanding ticket list from `docs/source/sdk/android/i18n_requests/`. | Manifest & ticket summary |
| Support/Release | Gather any escalations that require Docs/DevRel action (e.g., pending preview invites, blocking reviewer feedback). | Status.md or escalation doc |

## Agenda Blocks

1. **Roll call & objectives (5 min)**
   - Confirm quorum, scribe, and logistics.
   - Highlight any urgent incidents (docs preview outage, localization block).
2. **Localization staffing review (15 min)**
   - Review the staffing decision log in
     `docs/source/sdk/android/i18n_plan.md#staffing-decision-log`.
   - Confirm status of open POs (`DOCS-L10N-*`) and interim coverage.
   - Compare CI freshness output vs. the translation status table; call out any
     doc whose locale SLA (>5 business days) will be breached before the next
     sync.
   - Decide whether escalation is required (Product Ops, Finance, contractor
     management). Record the decision in both the staffing log and the monthly
     minutes, including owner + due date.
   - If staffing is healthy, document the confirmation so the roadmap action can
     move back to 🈺/🈴 with evidence.
3. **Docs/roadmap updates (10 min)**
   - Status of DOCS-SORA portal work, Try-It proxy, and SoraFS publication
     readiness.
   - Highlight doc debt or reviewers needed for current release trains.
4. **SDK highlights (10 min)**
   - Android AND5/AND7 doc readiness, Swift IOS5 parity, JS GA progress.
   - Capture shared fixtures or schema diffs that will affect docs.
5. **Action review & parking lot (5 min)**
   - Revisit open items from the previous sync; confirm closures.
   - Record new actions in the notes file with explicit owners and deadlines.

## Localization Staffing Review Template

Include the following table in each month’s minutes:

| Locale | Capacity (FTE) | Commitments & POs | Risks / Escalations | Decision & Owner |
|--------|----------------|-------------------|---------------------|------------------|
| JP | e.g., 0.5 contractor + 0.1 Docs backup | PO `DOCS-L10N-4901` (awaiting signature) | “Contract not signed by 2026-03-04” | “Escalate to Product Ops — @docs-devrel, due 2026-03-02” |
| HE | e.g., 0.1 Docs engineer | Rotation enters PTO 2026-03-18 | “Need backup reviewer” | “@docs-lead to identify backup by 2026-03-05” |

Also log a short narrative covering:

- **SLA outlook:** Any doc expected to miss the five-business-day SLA and the
  mitigation (swap priority, enlist backup vendor, etc.).
- **Ticket & asset health:** Outstanding entries in
  `docs/source/sdk/android/i18n_requests/` and whether screenshots/assets are
  ready for translators.

### Localization Staffing Review Logging

- **Minutes:** Copy the staffing table + narrative into
  `docs/source/docs_devrel/minutes/<yyyy-mm>.md` (all locales mirror the
  English minutes via localized files under the same directory). Link the entry
  back to the agenda (`docs/source/docs_devrel/monthly_sync_agenda.md`) so
  governance can trace evidence.
- **i18n plan:** Update the staffing decision log and translation status table
  in `docs/source/sdk/android/i18n_plan.md` immediately after the meeting.
- **Status:** When staffing decisions affect roadmap gates, add a short entry in
  `status.md` (Docs/DevRel section) referencing the minute file and i18n plan
  update.

## Minutes Template

Copy this skeleton into `docs/source/docs_devrel/minutes/<yyyy-mm>.md`:

```markdown
<!-- SPDX-License-Identifier: Apache-2.0 -->

# Docs/DevRel Monthly Sync — 2026-03-12

## Attendees
- Chair: …
- Scribe: …
- Participants: …

## Agenda Notes
1. Roll call & objectives — …
2. Localization staffing review — include table + narrative.
3. Docs/roadmap updates — …
4. SDK highlights — …
5. Action review & parking lot — …

## Decisions & Actions
| Item | Owner | Due | Notes |
|------|-------|-----|-------|
| JP contractor PO follow-up | @docs-devrel-manager | 2026-03-02 | Example entry |
```

Publish the notes via PR soon after the meeting and link them from `status.md`
when referencing risk or staffing decisions.

## Follow-Up Expectations

1. **Minutes committed:** within 24 hours (`docs/source/docs_devrel/minutes/`).
2. **i18n plan updated:** adjust the staffing log and translation table to
   reflect new commitments or escalations.
3. **Status.md entry:** summarize any high-risk decisions to keep the roadmap
   in sync.
4. **Escalations filed:** when the review calls for escalation, create/refresh
   the relevant ticket (e.g., Product Ops, Finance approval, vendor onboarding)
   and reference it in both the minutes and the i18n plan.

By following this agenda, the roadmap requirement to include localization
staffing reviews in the Docs/DevRel monthly sync stays auditable, and downstream
teams always know where to find the evidence.
