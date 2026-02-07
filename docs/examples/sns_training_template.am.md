---
lang: am
direction: ltr
source: docs/examples/sns_training_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: dd9da5045f5f40dbc31837145ad13bf79b4d751b0803c0b6d69bab49885ed1b4
source_last_modified: "2025-12-29T18:16:35.079313+00:00"
translation_last_reviewed: 2026-02-07
---

# SNS Training Slide Template

This Markdown outline mirrors the slides that facilitators should adapt for
their language cohorts. Copy these sections into Keynote/PowerPoint/Google
Slides and localise the bullet points, screenshots, and diagrams as needed.

## Title slide
- Program: “Sora Name Service onboarding”
- Subtitle: specify suffix + cycle (e.g., `.sora — 2026‑03`)
- Presenters + affiliations

## KPI orientation
- Screenshot or embed of `docs/portal/docs/sns/kpi-dashboard.md`
- Bullet list explaining suffix filters, ARPU table, freeze tracker
- Callouts for exporting PDF/CSV

## Manifest lifecycle
- Diagram: registrar → Torii → governance → DNS/gateway
- Steps referencing `docs/source/sns/registry_schema.md`
- Example manifest excerpt with annotations

## Dispute and freeze drills
- Flow diagram for guardian intervention
- Checklist referencing `docs/source/sns/governance_playbook.md`
- Example freeze ticket timeline

## Annex capture
- Command snippet showing `cargo xtask sns-annex ... --portal-entry ...`
- Reminder to archive Grafana JSON under `artifacts/sns/regulatory/<suffix>/<cycle>/`
- Link to `docs/source/sns/reports/.<suffix>/<cycle>.md`

## Next steps
- Training feedback link (see `docs/examples/sns_training_eval_template.md`)
- Slack/Matrix channel handles
- Upcoming milestone dates
