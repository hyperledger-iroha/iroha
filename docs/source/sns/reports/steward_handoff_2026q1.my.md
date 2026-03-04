---
lang: my
direction: ltr
source: docs/source/sns/reports/steward_handoff_2026q1.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a2f74f1fb77665f5736bb36bef14919de9b8d0bde6371a50d41a5638f0df70c6
source_last_modified: "2025-12-29T18:16:36.100211+00:00"
translation_last_reviewed: 2026-02-07
title: SNS Steward Hand-off Packet
summary: Motions and hand-offs derived from the steward KPI scorecard (SN-9).
---

# SNS Steward Hand-off Packet — 2026-Q1

_Generated at 2026-04-05T18:45:00Z (version 1)_

## Inputs

- Scorecard JSON: `docs/examples/sns/steward_scorecard_2026q1.json`
- Scorecard summary: `docs/source/sns/reports/steward_scorecard_2026q1.md`
- Steward playbook: `docs/source/sns/steward_replacement_playbook.md`
- Governance playbook: `docs/source/sns/governance_playbook.md`

| Suffix | Steward | Rotation | Deadline (days) | Due (UTC) |
|--------|---------|---------|-----------------|-----------|
| .dao | DAO Launchpad Guild | replace | 14 | 2026-04-19T18:45:00Z |

## .dao (DAO Launchpad Guild) — replace

**Reasons:**
- Single KPI breach recorded
- 1 guardian freeze(s) opened

**Deadline:** 14 days (2026-04-19T18:45:00Z)

**Council motion**
- Council replacement motion for .dao
  - Freeze registrar queue, appoint an interim steward for DAO Launchpad Guild, and attach the KPI evidence bundle.
  - Guardians freeze registrar queue and export registrar/dispute state.
  - File interim steward motion in the governance tracker with vote window + quorum.
  - Publish customer FAQ and transparency log entry once voting starts.
  - Action owners:
    - [guardian board] Freeze registrar queue and export registrar/dispute/state snapshots. (due `2026-04-07T18:45:00Z`)
    - [council chair] File interim steward motion with vote window + quorum requirements. (due `2026-04-12T18:45:00Z`)
    - [steward ops] Publish customer FAQ + transparency log once rotation vote is opened. (due `2026-04-12T18:45:00Z`)
    - [council secretary] Record vote outcome and schedule hand-off with replacement steward. (due `2026-04-19T18:45:00Z`)

**DAO motion**
- DAO ratification for .dao
  - Ratify the steward replacement, archive the vote record, and mirror the evidence in the DAO register.
  - Post ratification draft referencing the scorecard hash and council motion id.
  - Record vote outcome, attach FAQ + freeze log, and file in governance addenda.
  - Action owners:
    - [dao rapporteur] Publish ratification draft referencing scorecard hash + council motion id. (due `2026-04-08T18:45:00Z`)
    - [dao secretary] Attach FAQ/freeze log to DAO register and archive vote outcome. (due `2026-04-19T18:45:00Z`)

**Attachments:**
- `docs/examples/sns/steward_scorecard_2026q1.json`
- `docs/source/sns/governance_playbook.md`
- `docs/source/sns/reports/steward_scorecard_2026q1.md`
- `docs/source/sns/steward_replacement_playbook.md`

