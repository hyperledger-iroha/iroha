---
id: training-collateral
lang: kk
direction: ltr
source: docs/portal/docs/sns/training-collateral.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SNS Training Collateral
description: Curriculum, localization workflow, and annex evidence capture required by SN-8.
---

> Mirrors `docs/source/sns/training_collateral.md`. Use this page when briefing
> registrar, DNS, guardian, and finance teams ahead of each suffix launch.

## 1. Curriculum snapshot

| Track | Objectives | Pre-reads |
|-------|------------|-----------|
| Registrar ops | Submit manifests, monitor KPI dashboards, escalate errors. | `sns/onboarding-kit`, `sns/kpi-dashboard`. |
| DNS & gateway | Apply resolver skeletons, rehearse freezes/rollback. | `sorafs/gateway-dns-runbook`, direct-mode policy samples. |
| Guardians & council | Execute disputes, update governance addenda, log annexes. | `sns/governance-playbook`, steward scorecards. |
| Finance & analytics | Capture ARPU/bulk metrics, publish annex bundles. | `finance/settlement-iso-mapping`, KPI dashboard JSON. |

### Module flow

1. **M1 — KPI orientation (30 min):** Walk suffix filters, exports, and fugitive
   freeze counters. Deliverable: PDF/CSV snapshots with SHA-256 digest.
2. **M2 — Manifest lifecycle (45 min):** Build & validate registrar manifests,
   generate resolver skeletons via `scripts/sns_zonefile_skeleton.py`. Deliverable:
   git diff showing skeleton + GAR evidence.
3. **M3 — Dispute drills (40 min):** Simulate guardian freeze + appeal, capture
   guardian CLI logs beneath `artifacts/sns/training/<suffix>/<cycle>/logs/`.
4. **M4 — Annex capture (25 min):** Export dashboard JSON and run:

   ```bash
   cargo xtask sns-annex \
     --suffix <suffix> \
     --cycle <cycle> \
     --dashboard artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json \
     --dashboard-artifact artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json \
     --output docs/source/sns/reports/<suffix>/<cycle>.md \
     --regulatory-entry docs/source/sns/regulatory/<memo>.md \
     --portal-entry docs/portal/docs/sns/regulatory/<memo-id>.md
   ```

   Deliverable: updated annex Markdown + regulatory + portal memo blocks.

## 2. Localization workflow

- Languages: `ar`, `es`, `fr`, `ja`, `pt`, `ru`, `ur`.
- Each translation lives beside the source file
  (`docs/source/sns/training_collateral.<lang>.md`). Update `status` +
  `translation_last_reviewed` after refreshing.
- Assets per language belong under
  `artifacts/sns/training/<suffix>/<lang>/<cycle>/` (slides/, workbooks/,
  recordings/, logs/).
- Run `python3 scripts/sync_docs_i18n.py --lang <code>` after editing the English
  source so translators see the new hash.

### Delivery checklist

1. Update translation stub (`status: complete`) once localized.
2. Export slides to PDF and upload to the per-language `slides/` directory.
3. Record ≤10 min KPI walkthrough; link from the language stub.
4. File governance ticket tagged `sns-training` containing slide/workbook
   digests, recording links, and annex evidence.

## 3. Training assets

- Slide outline: `docs/examples/sns_training_template.md`.
- Workbook template: `docs/examples/sns_training_workbook.md` (one per attendee).
- Invite + reminders: `docs/examples/sns_training_invite_email.md`.
- Evaluation form: `docs/examples/sns_training_eval_template.md` (responses
  archived under `artifacts/sns/training/<suffix>/<cycle>/feedback/`).

## 4. Scheduling & metrics

| Cycle | Window | Metrics | Notes |
|-------|--------|---------|-------|
| 2026‑03 | Post KPI review | Attendance %, annex digest logged | `.sora` + `.nexus` cohorts |
| 2026‑06 | Pre `.dao` GA | Finance readiness ≥90 % | Include policy refresh |
| 2026‑09 | Expansion | Dispute drill <20 min, annex SLA ≤2 days | Align with SN-7 incentives |

Capture anonymous feedback in `docs/source/sns/reports/sns_training_feedback.md`
so subsequent cohorts can improve localization and labs.
