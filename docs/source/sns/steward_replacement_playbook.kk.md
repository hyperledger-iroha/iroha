---
lang: kk
direction: ltr
source: docs/source/sns/steward_replacement_playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 82eeaaa3a56aecfeadd4db06604407a40e8b3b58cbadddd2bf7469aefbfdf615
source_last_modified: "2025-12-29T18:16:36.101060+00:00"
translation_last_reviewed: 2026-02-07
title: SNS Steward Replacement Playbook
summary: Scorecard-driven workflow for warning, escalating, and rotating suffix stewards (SN-9).
---

# SNS Steward Replacement Playbook (SN-9)

> **Scope:** Applies to every SNS suffix steward once the quarterly KPI scorecard is published. This playbook links SN-9 deliverables (automation, reporting, rotation runbooks) to the specific artefacts we must archive for governance and regulatory reviews.

## 1. Scorecard Generation & Distribution

1. Prepare the metrics bundle (registrar exports, dispute tracker, guardian incident log) as a Norito JSON file matching `fixtures/sns/steward_metrics_<quarter>.json`.
2. Run the generator:
   ```sh
   cargo xtask sns-scorecard \
     --input fixtures/sns/steward_metrics_2026q1.json \
     --output-json docs/examples/sns/steward_scorecard_2026q1.json \
     --output-markdown docs/source/sns/reports/steward_scorecard_2026q1.md \
     --handoff-json docs/examples/sns/steward_handoff_2026q1.json \
     --handoff-markdown docs/source/sns/reports/steward_handoff_2026q1.md \
     --handoff-dir docs/examples/sns/handoffs/2026q1
   ```
3. Attach the JSON + Markdown artefacts to the governance docket (`docs/examples/sns/` and `docs/source/sns/reports/`) and mirror the digest in `status.md`. The hand-off packet now also writes per-suffix council/DAO hand-offs under `docs/examples/sns/handoffs/<quarter>/` (index + per-suffix JSON/Markdown) so tickets and DAO briefs can embed the exact deadlines/action owners.
4. Post the Markdown excerpt to the SNS council channel and submit it with the steward health review ticket. When the DAO needs visibility, share either the aggregate hand-off Markdown or the per-suffix Markdown from `docs/examples/sns/handoffs/<quarter>/` so observers see the same deadlines and motion language the council uses.

## 2. Rotation Timeline

| Stage | Trigger | Actions | Artefacts |
|-------|---------|---------|-----------|
| **Advisory (Day 0)** | Any KPI warning (`status=warning`) or support/dispute note flagged in the scorecard. | Steward lead sends advisory notice, requests remediation plan within 5 business days. | Advisory notice template filled + steward acknowledgement stored in `artifacts/sns/stewards/<suffix>/notices/`.
| **Escalation (Day 5)** | Two consecutive warnings, any KPI breach, or unresolved guardian freeze. | Council chair kicks off escalation call, assigns remediation owner, opens rotation ticket with timeline. | Escalation notice, rotation ticket link, remediation checklist, updated scorecard diff.
| **Replacement (≤Day 14)** | Breach persists or remediation rejected; guardian freeze unresolved; DAO vote demands change. | Execute rotation: guardians freeze registrar queue, council votes interim steward, publish incident write-up + customer comms. | Replacement notice, signed council motion, updated steward policy manifest, customer FAQ template, transparency log entry.

The 14-day window is hard-gated by the charter: once escalation starts, council must either accept a remediation plan or initiate replacement before the next scorecard is approved.

## 3. Notification Templates

### 3.1 Advisory Notice (send to steward + council observers)
```
Subject: SNS Steward Advisory — <suffix> KPI warning
Body:
Hello <steward contact>,

The <quarter> scorecard recorded a <metric / value>. Please acknowledge receipt within 2 business days and share a remediation outline (root cause, owners, timeline) within 5 business days.

Evidence: docs/examples/sns/steward_scorecard_<quarter>.json
Ticket: sns/<ticket-id>
```

### 3.2 Escalation Notice
```
Subject: SNS Steward Escalation — <suffix> KPI breach
Body:
Council has opened an escalation window because the <metric> breach persists / guardian freeze remains open. Provide a signed corrective plan within 48 hours. Failure to do so will trigger the rotation workflow described in docs/source/sns/steward_replacement_playbook.md.

Attachments: updated scorecard diff, remediation owner matrix, freeze log.
```

### 3.3 Replacement Notice
```
Subject: SNS Steward Replacement — <suffix>
Body:
Effective <date>, guardians have frozen the registrar queue and the council has appointed <interim steward>. All customer messaging must reference the attached FAQ. Your operations keys lose access within 24 hours; data export instructions are attached.

Artefacts: council decision hash, guardian freeze ticket, FAQ, runbook link.
```

## 4. Evidence & Archival Requirements

- File every notice + steward response beside the corresponding incident or governance ticket.
- For replacement events, include the signed Norito manifest that records the handover in `docs/source/sns/governance_addenda/<year>/<id>.md`.
- Update `docs/source/sns/governance_playbook.md` Appendix with the ticket ids so auditors can trace the full remediation chain.

## 5. Hand-Off Checklist (Replacement Day)

1. **Freeze + snapshot:** Guardians freeze registrar queue; ops exports registrar state, dispute ledger, revenue balances.
2. **Credential rotation:** Disable steward API keys, rotate monitoring hooks, update PagerDuty schedules.
3. **Customer messaging:** Publish FAQ + status update every 4 hours until replacement steward confirms readiness.
4. **Regulatory updates:** File memo in `docs/source/sns/regulatory/<jurisdiction>/` when the steward operates in regulated regions.
5. **Post-mortem:** Within 7 days capture the remediation timeline and add TODOs to `roadmap.md` / `status.md` if new automation is required.

Keep this playbook synchronized with SN-9 notes in `roadmap.md` so new stewards inherit the latest templates and expectations.
