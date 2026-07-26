---
id: preview-feedback-w3-log
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w3/log.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

Les evenements enregistres ci-dessous sont copies dans `artifacts/docs_portal_preview/feedback_log.json`
et resumes dans `preview-20260218-summary.json` / `preview-20260218-digest.md`.

| Horodatage (UTC) | Evenement | Destinataire | Notes |
| --- | --- | --- | --- |
| 2026-02-18 14:00 | invite-sent | finance-beta-01 | Cohorte pilote finance |
| 2026-02-18 14:08 | acknowledged | finance-beta-01 |  |
| 2026-02-21 10:22 | feedback-submitted | finance-beta-01 | docs-preview/20260218#1 |
| 2026-02-28 17:00 | access-revoked | finance-beta-01 |  |
| 2026-02-18 14:05 | invite-sent | observability-ops-02 | Preparation observability |
| 2026-02-18 14:20 | acknowledged | observability-ops-02 |  |
| 2026-02-23 09:45 | feedback-submitted | observability-ops-02 | docs-preview/20260218#2 |
| 2026-02-23 11:15 | issue-opened | observability-ops-02 | DOCS-SORA-Preview-20260218 |
| 2026-02-28 17:05 | access-revoked | observability-ops-02 |  |
| 2026-02-18 14:10 | invite-sent | partner-sdk-03 | Vague partenaire SDK |
| 2026-02-19 08:30 | acknowledged | partner-sdk-03 |  |
| 2026-02-24 16:10 | feedback-submitted | partner-sdk-03 | docs-preview/20260218#3 |
| 2026-02-28 17:10 | access-revoked | partner-sdk-03 |  |
| 2026-02-18 14:15 | invite-sent | ecosystem-advocate-04 | Ecosystem advocate |
| 2026-02-18 14:50 | acknowledged | ecosystem-advocate-04 |  |
| 2026-02-26 12:35 | feedback-submitted | ecosystem-advocate-04 | docs-preview/20260218#4 |
| 2026-02-28 17:15 | access-revoked | ecosystem-advocate-04 |  |

Utilise `npm run --prefix docs/portal preview:wave -- --wave preview-20260218 --invite-start 2026-02-18 --invite-end 2026-02-28 --report-date 2026-03-01`
pour regenerer le digest et les donnees du portail lors de la mise a jour de ce log.
