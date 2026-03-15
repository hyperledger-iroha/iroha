---
lang: kk
direction: ltr
source: docs/source/sdk/android/i18n_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f092d7710f53f4767f519aacfdef5dc6d24a30efca4182861ca238684c1668e8
source_last_modified: "2025-12-29T18:16:36.040422+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android SDK Localization & I18N Plan (AND5)

**Status:** Approved 2026-04-08 (staffing decision locked for AND5)  
**Scope:** AND5 developer experience deliverables (samples + docs)  
**Owners:** Docs/DevRel Manager (staffing), Android DX TL (source updates), Localization vendor/contractors (execution), QA Guild (linguistic review)

## 1. Objectives

1. Provide Japanese translations for all beta-facing Android SDK docs (quickstart, key management, troubleshooting, sample walkthroughs) within five business days of English updates.
2. Add Hebrew translations for GA, mirroring the Japanese set plus any additional compliance notes requested by regulators.
3. Keep localization commits deterministic: locale directories must be updated alongside English sources with tracked hashes and provenance metadata.
4. Integrate doc-locale freshness checks into CI so merges fail when translations drift beyond the SLA.

## 2. Scope of Work

| Artifact | English Path | JP Target | HE Target | Notes |
|----------|--------------|-----------|-----------|-------|
| Quickstart | `docs/source/sdk/android/index.md` | `index.ja.md` | `index.he.md` | Already localized; requires AND5 sample references + screenshot updates. |
| Key management | `docs/source/sdk/android/key_management.md` | `key_management.ja.md` | `key_management.he.md` | Update once AND2 alias lifecycle stabilizes; ensure glossary terms align with StrongBox doc. |
| Offline signing | `docs/source/sdk/android/offline_signing.md` | `offline_signing.ja.md` | `offline_signing.he.md` | Include envelope/handoff terminology. |
| Networking guide | `docs/source/sdk/android/networking.md` | `networking.ja.md` | `networking.he.md` | Must cover `/v2/pipeline`, Norito RPC, telemetry toggles. |
| Sample walkthrough — Operator console | `docs/source/sdk/android/samples/operator_console.md` | `samples/operator_console.ja.md` | `samples/operator_console.he.md` | New for AND5; share screenshots + CLI output. |
| Sample walkthrough — Retail wallet | `docs/source/sdk/android/samples/retail_wallet.md` | `samples/retail_wallet.ja.md` | `samples/retail_wallet.he.md` | Emphasize offline signing guidance. |
| Troubleshooting matrix | `docs/source/sdk/android/troubleshooting.md` | `troubleshooting.ja.md` | `troubleshooting.he.md` | To be written post-sample CI; include localization-ready strings. |
| Release/provenance notes | `docs/source/release/provenance/android/*.md` | JP/HE appendices | Optional if regulators require; track via governance tickets. |

## 3. Workflow & SLA

1. **Source of truth:** English docs live on `main`. Every change touching scoped files must update `docs/source/sdk/android/i18n_plan.md` status tables.
2. **Localization request:** open ticket in Docs tracker with commit hash, diff summary, and required locale(s). Attach diff-friendly `.po` files if tooling is available; otherwise include rendered HTML snippets for reviewers.
3. **Execution:** translators update locale files, run `scripts/sync_docs_i18n.py --locale <lang>`, and attach review notes. QA Guild performs linguistic QA for technical accuracy.
   - Every translation must keep the front-matter metadata (`source_hash`, `source_last_modified`, `translation_last_reviewed`) in sync with the English source. `ci/check_android_docs_i18n.sh` enforces the hashes and dates whenever a locale is marked ✅/⚠️ in the status table.
4. **Publishing:** once translations land, update status tables below and add an entry to `status.md` if the change impacts beta/GA readiness.
5. **SLA enforcement:** CI (`ci/check_android_docs_i18n.sh`) reads `docs/i18n/manifest.json` plus this file; merges fail if any required locale lags more than five business days.

## 4. Staffing & Schedule

| Milestone | Deadline | Staffing Need | Status |
|-----------|----------|---------------|--------|
| Approve Japanese translator contract | 2026-04-05 | 0.5 FTE contractor | 🈴 Completed — PO `DOCS-L10N-4901` countersigned 2026-04-04; Kotodama Language Partners (0.5 FTE) onboarding 2026-04-08 with NDA + billing profile captured in `docs/source/docs_devrel/minutes/2026-03.md`. |
| Allocate Hebrew reviewer | 2026-05-15 | Internal Docs engineer (0.1 FTE) | 🈴 Completed — Shira Kaplan owns the primary rota; Eitan Levi is the documented backup for PTO/incident spikes with escalation path noted in the minutes. |
| Establish localization review rota | 2026-04-15 | Android DX + Docs alternating weekly | 🈴 Completed — Tuesday/Friday diff digests now rotate between Android DX (Tue) and Docs/DevRel (Fri); rota stored in Section 4.3 and mirrored in `status.md`. |
| Wire CI freshness check | 2026-05-01 | Docs tooling engineer | 🈴 Completed — `ci/check_android_docs_i18n.sh` now enforces plan/translation parity |
| Vendor onboarding & QA shadowing | 2026-04-12 | Android DX TL + Docs vendor lead | 🈴 Completed — Kickoff held 2026-04-08, tooling demo + screenshot SOP recorded, and the QA checklist lives in `docs/source/sdk/android/i18n_vendor_onboarding_checklist.md` to guide ongoing drops. |

Escalate resource blockers during the monthly Docs sync (see `docs/source/sdk/android/telemetry_redaction_minutes_20260212.md` for template).

### 4.1 Staffing Decision Log

| Date | Forum | Decision | Owners / Follow-ups |
|------|-------|----------|---------------------|
| 2026-02-12 | Feb Docs/DevRel sync | Confirmed interim staffing while contractor paperwork closes: Docs/DevRel will commit 0.5 FTE JP vendor capacity (PO `DOCS-L10N-4901`, due 2026-02-28) and assign Shira Kaplan (Docs engineer) at 0.1 FTE as the temporary HE reviewer. Android DX will continue providing Tuesday/Friday diff digests until the vendor onboard date. | Docs/DevRel Manager to finalize PO by 2026-02-28; Product Ops to escalate if the vendor start date slips past 2026-03-08; Android DX TL to publish weekly diff summary in `status.md` until both roles are staffed. |
| 2026-02-12 | Action item from same sync | If the JP vendor contract is still unsigned by 2026-03-04, escalate to Product/Docs leadership for emergency approval so AND5 localization does not block beta docs. | Docs/DevRel Manager to post status update in `status.md` and raise the escalation in the March doc sync if the PO remains pending. |
| 2026-03-12 | Mar Docs/DevRel sync (`docs/source/docs_devrel/minutes/2026-03.md`) | PO `DOCS-L10N-4901` still pending; decision to escalate via ticket `PO-20260312` with SLA evidence and to increase Android diff digest cadence (Tue/Thu/Sat) until the contractor is live. HE reviewer PTO (2026-03-25–28) requires onboarding Eitan Levi as backup and updating the reviewer rota. | Docs/DevRel Manager to file escalation + update `status.md` (due 2026-03-14); Android DX TL to run expanded diff digests immediately; Android DX TL + Docs/DevRel Manager to confirm HE backup by 2026-03-18 and log rota update by 2026-03-20. |
| 2026-04-04 | Procurement stand-up (`PO-20260312` follow-up) | Contract executed; Kotodama Language Partners cleared finance checks and accepted the Tue/Thu/Sat digest cadence. Procurement approved kickoff for 2026-04-08 with the first deliverable targeting the Android quickstart request. | Docs/DevRel Manager uploaded countersignature + NDA to the ticket archive, Product Ops closed the escalation, and Android DX TL scheduled the kickoff invite plus glossary review. |
| 2026-04-08 | Vendor kickoff (Docs/DevRel + Android DX) | Locked the reviewer rota (Tue Android DX, Thu vendor QA shadow, Fri Docs/DevRel), confirmed HE backup coverage, and filed localization request `docs/source/sdk/android/i18n_requests/quickstart_20260408.md` for the beta quickstart so translators have a live artifact. | Vendor to deliver JP translation by 2026-04-12; Docs/DevRel to run HE review 2026-04-15–19; Android DX to attach diff digest + glossary updates to `status.md` on 2026-04-09. |

### 4.2 Monthly Docs/DevRel Sync Hook

Roadmap item “Add localization staffing review to monthly Docs/DevRel sync” now
has a formal agenda: see `docs/source/docs_devrel/monthly_sync_agenda.md`.
Follow that agenda for every monthly sync and ensure:

- The month’s minutes live under `docs/source/docs_devrel/minutes/` and include
  the localization staffing table from the agenda.
- Any staffing decision or escalation captured during the sync is mirrored in
  the log above and summarized in `status.md`.
- CI freshness output (`ci/check_android_docs_i18n.sh`) and ticket summaries
  from `docs/source/sdk/android/i18n_requests/` are reviewed live so SLA risks
  are caught early.

If the meeting uncovers a blocker, document the owner/due date in both the
minutes and the Staffing Decision Log to keep the roadmap deliverable auditable.

### 4.3 Reviewer & Diff Digest Rota (Effective 2026-04-08)

| Slot | Owner(s) | Responsibilities |
|------|----------|------------------|
| Tuesday diff digest | Android DX TL rotation (Sachiko → Kenji → Emi) | Publish Tue digest summarising English doc deltas, attach hash list to `status.md`, and prep vendor questions before the Thu QA shadow. |
| Thursday QA shadow | Kotodama Language Partners PM + Android DX shadow | Walk through glossary changes, validate screenshots, and stage JP draft pull requests. Android DX shadow signs off that deterministic commands/screenshots were preserved. |
| Friday reviewer block | Docs/DevRel (Shira Kaplan) with backup Eitan Levi | Finalize HE translation reviews, update `docs/i18n/manifest.json`, and log completion in `status.md`. Eitan covers PTO or Sev 1 incidents. |
| Saturday contingency | Android DX on-call | Only triggered when CI freshness alerts fire mid-week; ensures backlog is cleared before the Monday governance prep. |

The rota mirrors the schedule captured in the April Docs/DevRel sync agenda and
gives SRE a deterministic contact for escalations. Any deviations must be logged
both here and in the weekly `status.md` digest.

For the detailed onboarding/QA artefacts promised in the staffing table, refer
to `docs/source/sdk/android/i18n_vendor_onboarding_checklist.md`. Keep that
checklist updated alongside this plan whenever contacts, tooling, or evidence
requirements change.

## 5. Tracking Tables

### 5.1 Translation Status

| Doc | English Updated | JP Status | HE Status | Notes |
|-----|-----------------|-----------|-----------|-------|
| Quickstart | 2026-04-08 | 🈺 In Progress — request `i18n_requests/quickstart_20260408.md` in vendor queue (JP due 2026-04-12) | 🈺 In Progress — HE reviewer rota slot 2026-04-19 with Eitan Levi as backup | Kickoff evidence logged in `status.md` (2026-04-08); translators only need the refreshed screenshot bundle (due 2026-04-09). |
| Key management | 2026-02-15 | 🈳 Not Started (waiting for AND2 alias lifecycle freeze on 2026-04-15) | 🈳 Not Started (mirrors JP delivery) | Hold request until AND2 spec freeze; schedule immediately afterward. |
| Offline signing | 2026-04-10 | 🈺 In Progress — request `i18n_requests/offline_signing_20260410.md` assigned to Kotodama (JP drop due 2026-04-16) | 🈯 Reviewer slot reserved for 2026-04-23 | Shares glossary with quickstart; QA shadow happening in the Thu session before delivery. |
| Networking | 2026-04-10 | 🈺 In Progress — request `i18n_requests/networking_20260410.md` queued after the 2026-04-11 NRPC diff digest | 🈳 Waiting on Torii mock stability (HE blocked until `/v2/pipeline` parity is green) | Add telemetry toggle appendix during QA shadow so HE reviewer can translate once Torii confirms `/v2/pipeline` parity. |
| Operator console sample | 2026-03-24 | 🈳 Not Started | 🈳 Not Started | Awaiting localized screenshots + sample harness freeze (target 2026-04-18). |
| Retail wallet sample | 2026-03-24 | 🈳 Not Started | 🈳 Not Started | Same dependency as operator console; combine tickets once harness is stable. |
| Troubleshooting matrix | 2026-03-24 | 🈺 In Progress — request `i18n_requests/troubleshooting_matrix_20260324.md` tracking JP drop (due 2026-04-12) | 🈯 Scheduled — HE review window 2026-04-26 | Keep evidence bundle under `i18n_requests/troubleshooting_matrix_20260324.*`; escalate if JP drop misses the SLA. |
| Nexus overview (`docs/source/nexus_overview.md`) | 2026-03-24 | 🈯 Ticket `DOCS-L10N-4821` (vendor pending ETA) | 🈯 Ticket `DOCS-L10N-4822` (Docs engineer rota 2026-04-15) | Placeholder files checked in (`nexus_overview.ja.md`/`.he.md`); translators need settlement context. |
| Nexus operations runbook (`docs/source/nexus_operations.md`) | 2026-03-24 | 🈯 Ticket `DOCS-L10N-4823` | 🈯 Ticket `DOCS-L10N-4824` | Mirrors operator runbook severity matrix; ensure terminology matches support playbook. |
| Nexus settlement FAQ (`docs/source/nexus_settlement_faq.md`) | 2026-03-24 | 🈳 Not Started | 🈳 Not Started | Assign after worked examples land; note dependency in Section 7. |
| Nexus SDK quickstarts (`docs/source/nexus_sdk_quickstarts.md`) | 2026-03-24 | 🈳 Not Started | 🈳 Not Started | Blocked until SDK snippet API surfaces stabilize; track under NX-14. |

Legend: ✅ current · 🈺 in progress · 🈳 not started · ⚠️ needs attention

### 5.2 Risks & Mitigations

- **Translator bandwidth:** maintain backup vendor; document onboarding steps in `docs/i18n/manifest.json`.
- **Doc churn during beta:** batch localization requests weekly to reduce thrash; note schedule in `status.md`.
- **Screenshot updates:** keep annotated source files in `docs/assets/android/samples/`; include Figma links for translators.

## 6. Evidence & Reporting

Roadmap reviews expect repeatable artefacts that prove the localization SLA is
met. Capture the following for every weekly/urgent change:

| Evidence | Command / Source | Notes |
|----------|------------------|-------|
| Diff digest + request | `scripts/sync_docs_i18n.py --locale ja --locale he --status-out docs/source/sdk/android/i18n_requests/<doc>_YYYYMMDD.md` | Attach git hash + rendered HTML snippets; link the request file in the Docs tracker ticket. |
| CI freshness snapshot | `ci/check_android_docs_i18n.sh --json-out artifacts/android/i18n/status-$(date -u +%Y%m%dT%H%M%SZ).json` | Upload artefact to the review bundle; CI already blocks merges when `docs/i18n/manifest.json` disagrees with the status table. The JSON payload captures `plan_path`, `generated_at`, and one entry per doc with the English hash/revision plus each locale’s `exists`, `required`, and metadata/error state so reviewers can diff snapshots without scraping CI logs. |
| Governance update | Add a `status.md` bullet summarising the cadence slot, locales delivered, and outstanding gaps. | Required for AND5 meetings; include ticket IDs. |
| Monthly staffing minutes | `docs/source/sdk/android/i18n_requests/minutes/YYYY-MM.md` | Mirrors the Docs sync template; list owners, blockers, and escalation notes. |

During enablement reviews, bundle:

1. Latest status snapshot (`artifacts/android/i18n/status-*.json`).
2. Copies of the request files for translations completed that month.
3. Links to the CI job that enforced the freshness gate.

Store the bundle under `docs/source/sdk/android/i18n_requests/archive/YYYY-MM/`
so auditors can replay the evidence trail without digging through CI logs.

## 7. Next Actions

1. ✅ Confirm Japanese contractor availability before 2026-04-05; PO `DOCS-L10N-4901` countersigned 2026-04-04 and kickoff scheduled for 2026-04-08.
2. ✅ Publish the HE reviewer rota (Shira Kaplan primary, Eitan Levi backup) during the April prep window; rota now captured in Section 4.3.
3. Track quickstart request `i18n_requests/quickstart_20260408.md` through JP delivery (due 2026-04-12) and HE review (due 2026-04-19); attach evidence to the AND5 status bundle.
4. Track networking request `i18n_requests/networking_20260410.md` (JP due 2026-04-16, HE queued once `/v2/pipeline` parity is green) and log NRPC diff digest evidence with each drop.
5. Track offline-signing request `i18n_requests/offline_signing_20260410.md` (JP due 2026-04-16, HE review window 2026-04-23) alongside OA checklist references.
6. Track troubleshooting request `i18n_requests/troubleshooting_matrix_20260324.md` through the JP 2026-04-12 deadline and HE 2026-04-26 review.
7. ✅ Deliver the vendor onboarding + QA checklist (2026-04-08 artefacts) and reference it from the staffing table/status.md.

Update this plan whenever staffing, scope, or SLA assumptions change.
