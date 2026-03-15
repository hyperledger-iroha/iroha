---
lang: he
direction: rtl
source: docs/portal/docs/devportal/preview-feedback/w1/log.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: preview-feedback-w1-log
כותרת: Log de feedback e telemetria W1
sidebar_label: יומן W1
תיאור: רוסטר אגראגאדו, מחסומים טלמטריים e notas de reviewers para a primeira onda de preview de parceiros.
---

Este log mantem o roster de convites, checkpoints de telemetria e משוב של סוקרים עבור o
**תצוגה מקדימה de parceiros W1** que acompanha as tarefas de aceitacao em
[`preview-feedback/w1/plan.md`](./plan.md) e a entrada do tracker da onda em
[`../../preview-invite-tracker.md`](../../preview-invite-tracker.md). להטמיע את quando um convite עבור enviado,
אום תמונת מצב דה טלמטריה לרישום או אום פריט דה משוב לטריאדו עבור סוקרים דה גוברננקה פוסאם
reproduzir as evidencias sem perseguir כרטיסים externos.

## רוסטר דה קורטה

| מזהה שותף | Ticket de solicitacao | קבלת NDA | Convite enviado (UTC) | Ack/primeiro התחברות (UTC) | סטטוס | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| partner-w1-01 | `DOCS-SORA-Preview-REQ-P01` | בסדר 2025-04-03 | 2025-04-12 15:00 | 2025-04-12 15:11 | Concluido 2025-04-26 | soraps-op-01; focado em evidencia de paridade dos docs do orchestrator. |
| partner-w1-02 | `DOCS-SORA-Preview-REQ-P02` | בסדר 2025-04-03 | 12-04-2025 15:03 | 12-04-2025 15:15 | Concluido 2025-04-26 | soraps-op-02; validou crosslinks Norito/telemetria. |
| partner-w1-03 | `DOCS-SORA-Preview-REQ-P03` | בסדר 2025-04-04 | 2025-04-12 15:06 | 2025-04-12 15:18 | Concluido 2025-04-26 | soraps-op-03; executou drills de failover multi-source. |
| partner-w1-04 | `DOCS-SORA-Preview-REQ-P04` | בסדר 2025-04-04 | 12-04-2025 15:09 | 2025-04-12 15:21 | Concluido 2025-04-26 | torii-int-01; revisao do cookbook Torii `/v1/pipeline` + נסה את זה. |
| partner-w1-05 | `DOCS-SORA-Preview-REQ-P05` | בסדר 2025-04-05 | 12-04-2025 15:12 | 2025-04-12 15:23 | Concluido 2025-04-26 | torii-int-02; acompanhou a atualizacao do צילום מסך נסה את זה (docs-preview/w1 #2). |
| partner-w1-06 | `DOCS-SORA-Preview-REQ-P06` | בסדר 2025-04-05 | 12-04-2025 15:15 | 2025-04-12 15:26 | Concluido 2025-04-26 | sdk-partner-01; משוב על ספרי הבישול JS/Swift + בדיקות שפיות עושות גשר ISO. |
| partner-w1-07 | `DOCS-SORA-Preview-REQ-P07` | בסדר 2025-04-11 | 2025-04-12 15:18 | 2025-04-12 15:29 | Concluido 2025-04-26 | sdk-partner-02; תאימות aprovado 2025-04-11, focado em notas de Connect/telemetria. |
| partner-w1-08 | `DOCS-SORA-Preview-REQ-P08` | בסדר 2025-04-11 | 2025-04-12 15:21 | 2025-04-12 15:33 | Concluido 2025-04-26 | gateway-ops-01; auditou o guia ops do gateway + fluxo anonimo do proxy נסה זאת. |

Preencha os timestamps de **Convite enviado** e **Ack** assim que o email de saida for emitido.
Ancore os horarios ללא cronograma UTC definido ללא plano W1.

## מחסומי טלמטריה| חותמת זמן (UTC) | לוחות מחוונים / בדיקות | שו"ת | תוצאות | ארטפטו |
| --- | --- | --- | --- | --- |
| 2025-04-06 18:05 | `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` | Docs/DevRel + Ops | Tudo verde | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250406` |
| 06-04-2025 18:20 | תמלול דה `npm run manage:tryit-proxy -- --stage preview-w1` | אופס | מבוים | `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log` |
| 2025-04-12 14:45 | לוחות מחוונים acima + `probe:portal` | Docs/DevRel + Ops | הזמנה מראש של תמונת מצב, sem regressao | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250412` |
| 19/04/2025 17:55 | לוחות מחוונים acima + diff de latencia do proxy נסה את זה | Docs/DevRel lead | Checkpoint de meio aprovado (0 התראות; עכבות נסה את זה p95=410 ms) | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250419` |
| 26/04/2025 16:25 | לוחות מחוונים acima + probe de saida | Docs/DevRel + קשר ממשל | תמונת מצב, אפס התראות תלויות | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250426` |

כפי שידורי שעות העבודה (2025-04-13 -> 2025-04-25) estao agrupadas como מייצאת NDJSON + PNG em
`artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/` com nomes de arquivo
`docs-preview-integrity-<date>.json` הכתבים צילומי מסך.

## התחבר לבעיות משוב

השתמש ב-esta tabela para resumir achados enviados por סוקרים. Vincule cada entrada ao כרטיס GitHub/לדון
mais o formulario estruturado capturado via
[`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md).

| Referencia | Severidade | שו"ת | סטטוס | Notas |
| --- | --- | --- | --- | --- |
| `docs-preview/w1 #1` | נמוך | Docs-core-02 | Resolvido 2025-04-18 | Eclareceu o formule de nav do Try it + ancora de sidebar (`docs/source/sorafs/tryit.md` atualizado com novo label). |
| `docs-preview/w1 #2` | נמוך | Docs-core-03 | Resolvido 2025-04-19 | צילום מסך לעשות נסה את זה + legenda atualizados conforme pedido; artefato `artifacts/docs_preview/W1/preview-2025-04-12/feedback/partner-w1-05/screenshot-diff.png`. |
| - | מידע | Docs/DevRel lead | Fechado | הערות restantes foram apenas שאלות ותשובות; capturados אין נוסחאות משוב de cada parceiro sob `artifacts/docs_preview/W1/preview-2025-04-12/feedback/`. |

## הדרכה לבדיקת ידע וסקרים

1. הירשם כ-notas do quiz (meta >=90%) למען המבקר; anexe o CSV exportado ao lado dos artefatos de convite.
2. Colete as respostas qualitativas do survey capturadas no template de feedback e espelhe em
   `artifacts/docs_preview/W1/preview-2025-04-12/surveys/`.
3. Agende chamadas de remediation para quem estiver abaixo do limite e registre aqui.

Todos os oito מבקרים marcaram >=94% ללא בדיקת ידע (CSV:
`artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`). Nenhuma chamada de remediation
foi necessaria; יצוא de survey para cada parceiro vivem em
`artifacts/docs_preview/W1/preview-2025-04-12/surveys/<partner-id>/summary.json`.

## Inventario de artefatos

- מתאר/סיכום בדיקה מקדימה של חבילה: `artifacts/docs_preview/W1/preview-2025-04-12/descriptor.json`
- רזומה של בדיקה + בדיקת קישור: `artifacts/docs_preview/W1/preview-2025-04-12/preflight-summary.json`
- Log de mudanca do proxy נסה את זה: `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log`
- יצוא טלמטריה: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/<date>/`
- חבילת יומן טלמטריה לשעות המשרד: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/`
- יצוא משוב + סקר: colocar pastas por reviewer em
  `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`
- CSV e resumo לעשות בדיקת ידע: `artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`

Mantenha o inventario sincronizado com o issue do tracker. גיבושי אנקס כמו עותק ארטפטוס לכרטיס דה גוברננקה
para que auditores verifiquem os arquivos Sem acesso de shell.