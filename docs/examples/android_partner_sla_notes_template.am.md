---
lang: am
direction: ltr
source: docs/examples/android_partner_sla_notes_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5ca51ec624ebbb4b3760d5f2265d31047cd3b6492e21bdb10a3aa61655ccca69
source_last_modified: "2025-12-29T18:16:35.069181+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android Partner SLA Discovery Notes — Template

Use this template for every AND8 SLA discovery session. Store the filled copy
under `docs/source/sdk/android/partner_sla_sessions/<partner>/<date>/minutes.md`
and attach supporting artefacts (questionnaire responses, acknowledgements,
attachments) in the same directory.

```
Partner: <Name>                      Date: <YYYY-MM-DD>  Time: <UTC>
Primary contact(s): <names, roles, email>
Android attendees: <Program Lead / Partner Eng / Support Eng / Compliance>
Meeting link / ticket: <URL or ID>
```

## 1. Agenda & Context

- Purpose of the session (pilot scope, release window, telemetry expectations).
- Reference docs shared ahead of the call (support playbook, release calendar,
  telemetry dashboards).

## 2. Workload Overview

| Topic | Notes |
|-------|-------|
| Target workloads / chains | |
| Expected transaction volume | |
| Critical business windows / blackout periods | |
| Regulatory regimes (GDPR, MAS, FISC, etc.) | |
| Required languages / localisation | |

## 3. SLA Discussion

| SLA Class | Partner expectation | Delta from baseline? | Action required |
|-----------|--------------------|----------------------|-----------------|
| Critical fix (48 h) | | Yes/No | |
| High-severity (5 business days) | | Yes/No | |
| Maintenance (30 days) | | Yes/No | |
| Cutover notice (60 days) | | Yes/No | |
| Incident communications cadence | | Yes/No | |

Document any additional SLA clauses requested by the partner (e.g. dedicated
phone bridge, extra telemetry exports).

## 4. Telemetry & Access Requirements

- Grafana / Prometheus access needs:
- Log/trace export requirements:
- Offline evidence or dossier expectations:

## 5. Compliance & Legal Notes

- Jurisdictional notification requirements (statute + timing).
- Required legal contacts for incident updates.
- Data residency constraints / storage requirements.

## 6. Decisions & Action Items

| Item | Owner | Due | Notes |
|------|-------|-----|-------|
| | | | |

## 7. Acknowledgement

- Partner acknowledged baseline SLA? (Y/N)
- Follow-up acknowledgement method (email / ticket / signature):
- Attach confirming email or meeting minutes to this directory before closing.
