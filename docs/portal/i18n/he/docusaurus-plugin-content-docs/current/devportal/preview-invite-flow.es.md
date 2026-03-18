---
lang: he
direction: rtl
source: docs/portal/docs/devportal/preview-invite-flow.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Flujo de invitaciones de preview

## הצעה

פריט מפת הדרכים **DOCS-SORA** מזהה את ההטמעה של הבודקים ותוכנית ההזמנות של תצוגה מקדימה לציבור בדומה ל-Bloqueadores הסיום של הפורטל לפני תחילת הבטא. דף זה מתאר את כתפיו של ההזמנה, que artefactos deben enviarse antes de mandar invites y como demostrar que el flujo es auditable. Usala Junto Con:

- [`devportal/reviewer-onboarding`](./reviewer-onboarding.md) para el manejo por revisor.
- [`devportal/preview-integrity-plan`](./preview-integrity-plan.md) para garantias de checksum.
- [`devportal/observability`](./observability.md) עבור ייצוא של טלמטריה ו-hooks de alertas.

## Plan de olas

| אולה | Audiencia | קריטריונים דה אנטרדה | קריטריונים דה סלידה | Notas |
| --- | --- | --- | --- | --- |
| **W0 - ליבת תחזוקה** | Maintainers de Docs/SDK validando contenido del dia uno. | Equipo GitHub `docs-portal-preview` poblado, gate de checksum en `npm run serve` en verde, Alertmanager silencioso por 7 dias. | Todos los docs P0 revisados, פיגור התנהגות, תקריות חטאות. | Se usa para validar el flujo; no hay email de invitacion, solo se comparten los artefactos de preview. |
| **W1 - שותפים** | Operadores SoraFS, אינטגרדורים Torii, revisores de gobernanza bajo NDA. | W0 cerrado, terminos legales aprobados, proxy Try-it en staging. | הרשמה לשותפים (בעיה או בנוסחא של החברה) מוכרת, טלמטריה <=10 חשבונות בו-זמנית, חטאת החזרה ל-14 ימים. | אפליקציית plantilla de invitacion + כרטיסים לבקשת הלקוח. |
| **W2 - קומונידד** | Contribuidores seleccionados de la Liste de espera de la comunidad. | W1 cerrado, תרגילי תקריות, שאלות נפוצות ציבוריות בפועל. | משוב עדכני, >=2 מהדורות מסמכים דרך צינור תצוגה מקדימה לאחר החזרה לאחור. | Limitar invitaciones concurrentes (<=25) y agrupar semanalmente. |

Documenta que ola esta active en `status.md` y en el tracker de solicitudes de preview para que la gobernanza vea el estado de un vistazo.

## רשימת בדיקה מוקדמת

הזמנות מלאות **אנטות** להזמנות תוכנה עבור אונה:

1. **Artefactos de CI disponibles**
   - El ultimo `docs-portal-preview` + cargado descriptor por `.github/workflows/docs-portal-preview.yml`.
   - Pin de SoraFS anotado en `docs/portal/docs/devportal/deploy-guide.md` (תיאור de cutover presente).
2. **אכיפה של סכום ביקורת**
   - `docs/portal/scripts/serve-verified-preview.mjs` אינבוקדו דרך `npm run serve`.
   - הוראות `scripts/preview_verify.sh` ב-macOS + Linux.
3. **בסיס טלמטריה**
   - `dashboards/grafana/docs_portal.json` muestra trafico נסה את זה בר ברכה y la alerta `docs.preview.integrity` esta en verde.
   - Ultimo apendice de `docs/portal/docs/devportal/observability.md` אקטואליזד עם חיבורים של Grafana.
4. **Artefactos de gobernanza**
   - בעיה של רשימה של מעקב אחר הזמנת (una issue por ola).
   - Plantilla de registro de revisores copiada (ver [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)).
   - Aprobaciones legales y de SRE requeridas adjuntas a la issue.

הרשמה לסיום הטיסה המוקדמת והזמן הגשש לפני הקנאה.## Pasos del flujo

1. **מועמדים נבחרים**
   - Extraer de la hoja de espera o cola de partners.
   - Asegurar que cada candidato tenga la plantilla de solicitud completa.
2. **Aprobar acceso**
   - Asignar un aprobador a la issue del invite tracker.
   - תנאים מוקדמים לאמת (CLA/contrato, uso acceptable, brief de seguridad).
3. **קבל הזמנות**
   - Completar los placeholders de [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) (`<preview_tag>`, `<request_ticket>`, contactos).
   - Adjuntar el descriptor + hash del archive, URL de staging de נסה את זה, y canales de soporte.
   - שומר אימייל סופית (תמליל של Matrix/Slack) בעניין.
4. **הצטרפות ל-Rastrear**
   - Actualizar el invite tracker con `invite_sent_at`, `expected_exit_at`, y estado (`pending`, `active`, `complete`, Torii).
   - Enlazar la solicitud de ingreso del revisor para auditabilidad.
5. **טלמטריה מוניטורית**
   - Vigilar `docs.preview.session_active` y alertas `TryItProxyErrors`.
   - Abrir un incidente si la telemetria se desvia del baseline y רשם el resultado junto a la entrada de invitacion.
6. **משוב חוזר ונראה**
   - קבל הזמנה לקבלת משוב על `expected_exit_at`.
   - Actualizar la issue de la ola con un resumen corto (hallazgos, incidentes, siguientes acciones) antes de pasar al suuiente cohorte.

## מדווחות Evidencia y

| Artefacto | Donde guardar | Cadencia de actualizacion |
| --- | --- | --- |
| גיליון מעקב אחר הזמנות | Proyecto GitHub `docs-portal-preview` | Actualizar dispues de cada הזמנה. |
| ייצוא רשימת רוסטרים | Registro enlazado en `docs/portal/docs/devportal/reviewer-onboarding.md` | סמנאל. |
| תמונות Snapshots de telemetria | `docs/source/sdk/android/readiness/dashboards/<date>/` (צרור חוזר של טלמטריה) | Por ola + despues de incidentes. |
| תקציר משוב | `docs/portal/docs/devportal/preview-feedback/<wave>/summary.md` (crear carpeta por ola) | Dentro de 5 Dias Tras Salir de la Ola. |
| Nota de reunion de gobernanza | `docs/portal/docs/devportal/preview-invite-notes/<date>.md` | השלם את הסנכרון לפני המלחמה DOCS-SORA. |

Ejecuta `cargo xtask docs-preview summary --wave <wave_label> --json artifacts/docs_portal_preview/<wave_label>_summary.json`
despues de cada lote para producir un digest קריא por maquinas. Adjunta el JSON renderizado a la issue de la ola para que los revisores de gobernanza confirmen los conteos de invitaciones sin reproducir todo el log.

הוספה לרשימת הוכחות ל-`status.md`.

## קריטריונים להחזרה לאחור

Pausa el flujo de invitaciones (y notifica a gobernanza) cuando ocurra cualquiera de estos casos:

- Un incidente de proxy נסה את זה que requirio rollback (`npm run manage:tryit-proxy`).
- גילוי התראות: >3 דפי התראה לנקודות קצה בודדות של תצוגה מקדימה של 7 ימים.
- Brecha de cumplimiento: invitacion enviada sin terminos firmados o sin registrar la plantilla de solicitud.
- Riesgo de integridad: חוסר התאמה של checksum detectado por `scripts/preview_verify.sh`.

Reanuda solo despues de documentar la remediacion en el invite tracker y confirmar que el dashboard de telemetria este estable por al menos 48 horas.