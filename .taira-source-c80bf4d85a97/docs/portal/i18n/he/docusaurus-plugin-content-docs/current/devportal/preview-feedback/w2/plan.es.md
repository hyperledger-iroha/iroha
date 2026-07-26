---
lang: he
direction: rtl
source: docs/portal/docs/devportal/preview-feedback/w2/plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: preview-feedback-w2-plan
כותרת: Plan de intake comunitario W2
sidebar_label: תוכנית W2
תיאור: נטילת, אפרובאציונות ורשימת הוכחות לצוות של קהילת תצוגה מקדימה.
---

| פריט | פרטים |
| --- | --- |
| אולה | W2 - מבקרים comunitarios |
| Ventana objetivo | Q3 2025 Semana 1 (tentativa) |
| Tag de artefacto (planeado) | `preview-2025-06-15` |
| גיליון del tracker | `DOCS-SORA-Preview-W2` |

## אובייקטיביות

1. מגדירים קריטריונים לצריכה וטיפול בבדיקה.
2. Obtener aprobacion de gobernanza para el roster propuesto y el addendum de uso aceptable.
3. Refrescar el artefacto de preview verificado por checksum y el bundle de telemetria para la nueva ventana.
4. הכן את ה-proxy נסה את זה עם לוחות המחוונים לפני ההזמנה.

## Desglose de tareas

| תעודת זהות | טארא | אחראי | Fecha limite | Estado | Notas |
| --- | --- | --- | --- | --- | --- |
| W2-P1 | עריכת קריטריונים של כניסת קומוניטאריים (מקסימום משבצות, דרישות CoC) y circular a gobernanza | Docs/DevRel lead | 2025-05-15 | Completado | La politica de intake se fusiono en `DOCS-SORA-Preview-W2` y se respaldo en la reunion del consejo 2025-05-20. |
| W2-P2 | תבנית בקשת אקטואליזציה עם מאפיינים ספציפיים לקהילה (מוטיבציה, ביטול, נחוצות מקומיות) | Docs-core-01 | 2025-05-18 | Completado | `docs/examples/docs_preview_request_template.md` ahora incluye la seccion Community, referenciada en el formulario de intake. |
| W2-P3 | Asegurar aprobacion de gobernanza para el plan de intake (voto en reunion + actas registradas) | קשר ממשל | 22-05-2025 | Completado | Voto aprobado por unanimidad el 2025-05-20; actas y roll call enlazados en `DOCS-SORA-Preview-W2`. |
| W2-P4 | תוכנת בימוי של פרוקסי נסה את זה + קלטת טלמטריה עבור חלון W2 (`preview-2025-06-15`) | Docs/DevRel + Ops | 2025-06-05 | Completado | Ticket de cambio `OPS-TRYIT-188` aprobado y ejecutado 2025-06-09 02:00-04:00 UTC; צילומי מסך של Grafana בארכיון עם כרטיס. |
| W2-P5 | Construir/verificar nuevo tag de artefacto de preview (`preview-2025-06-15`) y descriptor/checksum/probe logs | פורטל TL | 2025-06-07 | Completado | `scripts/preview_wave_preflight.sh --tag preview-2025-06-15 ...` se ejecuto 2025-06-10; יציאות guardados bajo `artifacts/docs_preview/W2/preview-2025-06-15/`. |
| W2-P6 | רשימת ההזמנות של Armar de invitaciones comunitarias (<=25 סוקרים, lotes escalonados) עם מידע יצירת קשר עם גוברננזה | מנהל קהילה | 2025-06-10 | Completado | Primer cohorte של 8 סוקרים comunitarios aprobado; IDs de solicitud `DOCS-SORA-Preview-REQ-C01...C08` registrados en el tracker. |

## רשימת הוכחות- [x] Registro de aprobacion de gobernanza (notas de reunion + link de voto) adjunto a `DOCS-SORA-Preview-W2`.
- [x] Template de solicitud actualizado commiteado bajo `docs/examples/`.
- [x] מתאר `preview-2025-06-15`, יומן בדיקה, פלט בדיקה, דוח קישור ותמלול פרוקסי נסה את זה guardados bajo `artifacts/docs_preview/W2/`.
- [x] צילומי מסך של Grafana (`docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`) צילומי מסך ל-W2.
- [x] טבלה של רוסטר של הזמנות עם תעודות זהות של סוקרים, כרטיסים מבוקשים וחותמות זמן של אפרובאציון קומפלטאדוס אנטים דל סביבות (ברק W2 del tracker).

Mantener este plan actualizado; el tracker lo referencia para que el roadmap DOCS-SORA vea exactamente lo que resta antes de que salgan las invitaciones W2.