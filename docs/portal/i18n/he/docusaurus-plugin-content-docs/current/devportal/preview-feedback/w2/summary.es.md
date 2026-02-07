---
lang: he
direction: rtl
source: docs/portal/docs/devportal/preview-feedback/w2/summary.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: preview-feedback-w2-summary
כותרת: Resumen de feedback y estado W2
sidebar_label: קורות חיים W2
תיאור: Resumen en vivo para la ola de preview comunitaria (W2).
---

| פריט | פרטים |
| --- | --- |
| אולה | W2 - מבקרים comunitarios |
| Ventana de invitacion | 2025-06-15 -> 2025-06-29 |
| Tag de artefacto | `preview-2025-06-15` |
| גיליון del tracker | `DOCS-SORA-Preview-W2` |
| משתתפים | comm-vol-01 ... comm-vol-08 |

## Destacados

1. **Gobernanza y tooling** - La politica de intake comunitario fue aprobada por unanimidad el 2025-05-20; el template de solicitud actualizado con campos de motivacion/zona horaria vive en `docs/examples/docs_preview_request_template.md`.
2. **Evidencia de preflight** - El cambio del proxy Try it `OPS-TRYIT-188` se ejecuto el 2025-06-09, dashboards de Grafana capturados, y los outputs descriptor/checksum/probe de I1800iva `artifacts/docs_preview/W2/`.
3. **Ola de invitaciones** - Ocho reviewers comunitarios invitados el 2025-06-15, con acnowledgements registrados en la tabla de invitaciones del tracker; todos completaron verificacion de checksum antes de navegar.
4. **משוב** - `docs-preview/w2 #1` (ניסוח de tooltip) y `#2` (orden de sidebar de localizacion) se registraron el 2025-06-18 y se resolvieron para 2025-06-21 (4/05-21); no hubo incidentes durante la ola.

## פעולות

| תעודת זהות | תיאור | אחראי | Estado |
| --- | --- | --- | --- |
| W2-A1 | Atender `docs-preview/w2 #1` (ניסוח de tooltip). | Docs-core-04 | Completado 2025-06-21 |
| W2-A2 | Atender `docs-preview/w2 #2` (סרגל צד של לוקליזציה). | Docs-core-05 | Completado 2025-06-21 |
| W2-A3 | ארכיון ראיות דה סלידה + מפת דרכים/סטטוס ממשי. | Docs/DevRel lead | Completado 2025-06-29 |

## קורות חיים דה סלידה (2025-06-29)

- Los ocho reviewers comunitarios confirmaron finalizacion y se les revoco el acceso al preview; אישורים registrados en el log de invitaciones del tracker.
- Los צילומי המצב הסופי של טלמטריה (`docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`) se mantuvieron verdes; יומנים ותמלילים של פרוקסי נסה זאת עם `DOCS-SORA-Preview-W2`.
- Bundle de Evidencia (תיאור, יומן בדיקה, פלט בדיקה, דוח קישור, צילומי מסך של Grafana, אישורי הזמנה) archivado bajo `artifacts/docs_preview/W2/preview-2025-06-15/`.
- יומן המחסומים W2 של הגשש נמצא במציאות, כאשר מפת הדרכים ניתנת לרישום לפני תחילת התכנון של W3.