---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w3/log.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3863fc4d8f6b1358d34e413af6a6be984bcd6b9b9df8e2d8acf4fac32abae52e
source_last_modified: "2025-11-20T12:45:58.005633+00:00"
translation_last_reviewed: 2026-01-01
---

---
id: preview-feedback-w3-log
title: Log de convites beta W3
sidebar_label: Log W3
description: Linha do tempo para a onda de convites de preview 2026-02-18.
---

Os eventos registrados abaixo sao espelhados em `artifacts/docs_portal_preview/feedback_log.json`
e resumidos em `preview-20260218-summary.json` / `preview-20260218-digest.md`.

| Timestamp (UTC) | Evento | Destinatario | Notas |
| --- | --- | --- | --- |
| 2026-02-18 14:00 | invite-sent | finance-beta-01 | Cohorte piloto de financas |
| 2026-02-18 14:08 | acknowledged | finance-beta-01 |  |
| 2026-02-21 10:22 | feedback-submitted | finance-beta-01 | docs-preview/20260218#1 |
| 2026-02-28 17:00 | access-revoked | finance-beta-01 |  |
| 2026-02-18 14:05 | invite-sent | observability-ops-02 | Preparacao de observability |
| 2026-02-18 14:20 | acknowledged | observability-ops-02 |  |
| 2026-02-23 09:45 | feedback-submitted | observability-ops-02 | docs-preview/20260218#2 |
| 2026-02-23 11:15 | issue-opened | observability-ops-02 | DOCS-SORA-Preview-20260218 |
| 2026-02-28 17:05 | access-revoked | observability-ops-02 |  |
| 2026-02-18 14:10 | invite-sent | partner-sdk-03 | Onda de partner SDK |
| 2026-02-19 08:30 | acknowledged | partner-sdk-03 |  |
| 2026-02-24 16:10 | feedback-submitted | partner-sdk-03 | docs-preview/20260218#3 |
| 2026-02-28 17:10 | access-revoked | partner-sdk-03 |  |
| 2026-02-18 14:15 | invite-sent | ecosystem-advocate-04 | Advocate de ecossistema |
| 2026-02-18 14:50 | acknowledged | ecosystem-advocate-04 |  |
| 2026-02-26 12:35 | feedback-submitted | ecosystem-advocate-04 | docs-preview/20260218#4 |
| 2026-02-28 17:15 | access-revoked | ecosystem-advocate-04 |  |

Use `npm run --prefix docs/portal preview:wave -- --wave preview-20260218 --invite-start 2026-02-18 --invite-end 2026-02-28 --report-date 2026-03-01`
para regenerar o digest e os dados do portal ao atualizar este log.
