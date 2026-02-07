---
lang: he
direction: rtl
source: docs/portal/docs/devportal/preview-feedback/w1/summary.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: preview-feedback-w1-summary
כותרת: Resumen de feedback y cierre W1
sidebar_label: קורות חיים W1
תיאור: Hallazgos, acciones y evidencia de cierre para la ola de preview de partners e integradores Torii.
---

| פריט | פרטים |
| --- | --- |
| אולה | W1 - Partners e integradores de Torii |
| Ventana de invitacion | 2025-04-12 -> 2025-04-26 |
| Tag de artefacto | `preview-2025-04-12` |
| גיליון del tracker | `DOCS-SORA-Preview-W1` |
| משתתפים | sorafs-op-01...03, torii-int-01...02, sdk-partner-01...02, gateway-ops-01 |

## Destacados

1. **Flujo de checksum** - Todos los reviewers verificaron el descriptor/archive via `scripts/preview_verify.sh`; los logs se guardaron junto a los acuses de invitacion.
2. **Telemetria** - לוחות מחוונים לוס `docs.preview.integrity`, `TryItProxyErrors` y `DocsPortal/GatewayRefusals` se mantuvieron en verde durante toda la ola; אין תקריות של התראה.
3. **משוב על מסמכים (`docs-preview/w1`)** - ראה את הרשמי של דוס nits menores:
   - `docs-preview/w1 #1`: ניסוח ברור de navegacion en la seccion נסה זאת (resuelto).
   - `docs-preview/w1 #2`: צילום מסך בפועל של נסה את זה (resuelto).
4. **Paridad de runbooks** - Operadores de SoraFS confirmaron que los nuevos crosslinks entre `orchestrator-ops` y `multi-source-rollout` resolvieron sus preocupaciones de W0.

## פעולות

| תעודת זהות | תיאור | אחראי | Estado |
| --- | --- | --- | --- |
| W1-A1 | ניסוח בפועל של נסה זאת `docs-preview/w1 #1`. | Docs-core-02 | קומפלטאדו (2025-04-18). |
| W1-A2 | צילום מסך בפועל של נסה את זה עם `docs-preview/w1 #2`. | Docs-core-03 | קומפלטאדו (2025-04-19). |
| W1-A3 | רזומה כללי של שותפים וראיות טלמטריה במפת דרכים/סטטוס. | Docs/DevRel lead | Completado (ver tracker + status.md). |

## קורות חיים (2025-04-26)

- סוקרי Los ocho confirmaron finalizacion durante las שעות המשרד הסיום, limpiaron artefactos locales y se revoco su acceso.
- La telemetria se mantuvo en verde hasta el cierre; תמונת מצב סיום משלים ל-`DOCS-SORA-Preview-W1`.
- El log de invitaciones se actualizo con acuses de salida; el tracker marco W1 como completado y agrego los checkpoints.
- Paquete de evidencia (תיאור, יומן בדיקה, פלט בדיקה, תמליל פרוקסי נסה את זה, צילומי מסך של טלמטריה, תקציר משוב) archivado bajo `artifacts/docs_preview/W1/`.

## Siguiente pasos

- הכנת תוכנית כניסת קומוניטאריו W2 (aprobacion de gobernanza + ajustes de template de solicitud).
- Refrescar el tag de artefacto de preview para la ola W2 y reejecutar el script de preflight cuando se finalicen fechas.
- Volcar hallazgos aplicables de W1 en מפת דרכים/סטטוס para que la ola comunitaria tenga la guia mas reciente.