---
lang: he
direction: rtl
source: docs/portal/docs/devportal/preview-feedback/w1/log.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: preview-feedback-w1-log
כותרת: Log de feedback y telemetria W1
sidebar_label: יומן W1
תיאור: רוסטר אגראגאדו, מחסומים טלמטריים ותעודות בודקים לראשון או תצוגה מקדימה של שותפים.
---

Este log mantiene el roster de invitaciones, checkpoints de telemetria y feedback de reviewers para el
**תצוגה מקדימה של שותפים W1** que acompana las tareas de aceptacion en
[`preview-feedback/w1/plan.md`](./plan.md) y la entrada del tracker de la ola en
[`../../preview-invite-tracker.md`](../../preview-invite-tracker.md). אקטואליזאלו קנאו בהזמנה,
se registre un snapshot de telemetria o se triagee un item de feedback para que los reviewers de gobernanza puedan reproducir
la evidencia sin perseguir tickets externos.

## סגל הקבוצה

| מזהה שותף | Ticket de solicitud | NDA recibida | Invitacion enviada (UTC) | Ack/primer login (UTC) | Estado | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| partner-w1-01 | `DOCS-SORA-Preview-REQ-P01` | בסדר 2025-04-03 | 2025-04-12 15:00 | 2025-04-12 15:11 | Completado 2025-04-26 | soraps-op-01; enfocado en evidencia de paridad de docs del orchestrator. |
| partner-w1-02 | `DOCS-SORA-Preview-REQ-P02` | בסדר 2025-04-03 | 12-04-2025 15:03 | 12-04-2025 15:15 | Completado 2025-04-26 | soraps-op-02; valido crosslinks de Norito/telemetria. |
| partner-w1-03 | `DOCS-SORA-Preview-REQ-P03` | בסדר 2025-04-04 | 2025-04-12 15:06 | 2025-04-12 15:18 | Completado 2025-04-26 | soraps-op-03; ejecuto drills de failover multi-source. |
| partner-w1-04 | `DOCS-SORA-Preview-REQ-P04` | בסדר 2025-04-04 | 12-04-2025 15:09 | 2025-04-12 15:21 | Completado 2025-04-26 | torii-int-01; revision del cookbook de Torii `/v2/pipeline` + נסה את זה. |
| partner-w1-05 | `DOCS-SORA-Preview-REQ-P05` | בסדר 2025-04-05 | 12-04-2025 15:12 | 2025-04-12 15:23 | Completado 2025-04-26 | torii-int-02; acompanio la actualizacion de screenshot de נסה את זה (docs-preview/w1 #2). |
| partner-w1-06 | `DOCS-SORA-Preview-REQ-P06` | בסדר 2025-04-05 | 12-04-2025 15:15 | 2025-04-12 15:26 | Completado 2025-04-26 | sdk-partner-01; משוב על ספרי הבישול JS/Swift + בדיקות שפיות del puente ISO. |
| partner-w1-07 | `DOCS-SORA-Preview-REQ-P07` | בסדר 2025-04-11 | 2025-04-12 15:18 | 2025-04-12 15:29 | Completado 2025-04-26 | sdk-partner-02; תאימות aprobado 2025-04-11, enfocado en notas de Connect/telemetria. |
| partner-w1-08 | `DOCS-SORA-Preview-REQ-P08` | בסדר 2025-04-11 | 2025-04-12 15:21 | 2025-04-12 15:33 | Completado 2025-04-26 | gateway-ops-01; audito la guia de ops del gateway + flujo anonimo del proxy נסה זאת. |

חותמות זמן מלאות של **Invitacion enviada** y **Ack** apenas se emita el email saliente.
Ancla los tiempos al calendario UTC definido en el plan W1.

## מחסומי טלמטריה| חותמת זמן (UTC) | לוחות מחוונים / בדיקות | אחראי | תוצאות | Artefacto |
| --- | --- | --- | --- | --- |
| 2025-04-06 18:05 | `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` | Docs/DevRel + Ops | Todo en verde | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250406` |
| 06-04-2025 18:20 | תמלול דה `npm run manage:tryit-proxy -- --stage preview-w1` | אופס | פרפרדו | `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log` |
| 2025-04-12 14:45 | לוחות מחוונים דה arriba + `probe:portal` | Docs/DevRel + Ops | הזמנה מראש של תמונת מצב, רגרסיות חטא | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250412` |
| 19/04/2025 17:55 | לוחות מחוונים של arriba + diff de latencia del proxy נסה זאת | Docs/DevRel lead | Chequeo de mitad de ola ok (0 התראות; עכבות נסה את זה p95=410 ms) | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250419` |
| 26/04/2025 16:25 | לוחות מחוונים דה arriba + probe de salida | Docs/DevRel + קשר ממשל | תמונת מצב של סלידה, cro alertas pendientes | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250426` |

Las muestras diarias de office hours (2025-04-13 -> 2025-04-25) se agrupan como מייצא NDJSON + PNG bajo
`artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/` עם שמות ארכיון
`docs-preview-integrity-<date>.json` y los צילומי מסך מתכתבים.

## רישום משוב ובעיות

Usa esta tabla para resumir hallazgos enviados por מבקרים. Enlaza cada entrada al ticket de GitHub/לדון
mas el formulario estructurado capturado via
[`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md).

| Referencia | Severidad | אחראי | Estado | Notas |
| --- | --- | --- | --- | --- |
| `docs-preview/w1 #1` | נמוך | Docs-core-02 | Resuelto 2025-04-18 | נסה את זה + סרגל צד (`docs/source/sorafs/tryit.md` בפועל עם תווית חדשה). |
| `docs-preview/w1 #2` | נמוך | Docs-core-03 | Resuelto 2025-04-19 | ראה צילום מסך ממשי של נסה את זה + הכיתוב סגן פדידו של המבקר; artefacto `artifacts/docs_preview/W1/preview-2025-04-12/feedback/partner-w1-05/screenshot-diff.png`. |
| - | מידע | Docs/DevRel lead | סראדו | שאלות ותשובות בלבד; capturados en el formulario de feedback de cada partner bajo `artifacts/docs_preview/W1/preview-2025-04-12/feedback/`. |

## בדיקת ידע בסקרים

1. Registra los puntajes del quiz (objetivo >=90%) para cada reviewer; adjunta el CSV exportado junto a los artefactos de invitacion.
2. Recolecta las respuestas cualitativas del survey capturadas con el template de feedback y reflejalas bajo
   `artifacts/docs_preview/W1/preview-2025-04-12/surveys/`.
3. סדר היום llamadas de remediacion para cualquiera por debajo del umbral y registralas en este archivo.

מבקרי Los ocho marcaron >=94% בבדיקת ידע (CSV:
`artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`). No se requirieron llamadas de remediacion;
los exports de survey para cada partner viven bajo
`artifacts/docs_preview/W1/preview-2025-04-12/surveys/<partner-id>/summary.json`.

## Inventario de artefactos

- חבילת תיאור/בדיקה מקדימה: `artifacts/docs_preview/W1/preview-2025-04-12/descriptor.json`
- קורות חיים + בדיקת קישור: `artifacts/docs_preview/W1/preview-2025-04-12/preflight-summary.json`
- Log de cambio del proxy נסה את זה: `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log`
- ייצוא טלמטריה: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/<date>/`
- חבילת יומן טלמטריה לשעות המשרד: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/`
- ייצוא משוב + סקרים: colocar carpetas por reviewer bajo
  `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`
- CSV ובדיקת קורות חיים: `artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`Mantener el inventario sincronizado con el issue del tracker. Adjunta hashes al copiar artefactos al ticket de gobernanza
para que los auditores verifiquen los archivos sin acceso de shell.