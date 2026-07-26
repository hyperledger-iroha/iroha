---
lang: he
direction: rtl
source: docs/portal/docs/devportal/reviewer-onboarding.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Onboarding de revisores de preview

## קורות חיים

DOCS-SORA הצג את הפורטל של סופרים. Los בונה con gate de checksum
(`npm run serve`) y los flujos נסה את זה reforzados desbloquean el suuiente hito:
onboarding de revisores validados antes de que el תצוגה מקדימה publico se abra de forma amplia. Esta guia
תאר como recopilar solicitudes, verificar elegibilidad, aprovisionar acceso y dar de baja
משתתפי הפורמה סגורה. Consulta el
[זרימת הזמנת תצוגה מקדימה](./preview-invite-flow.md) para la planificacion de cohortes, la
cadencia de invitaciones y los exports de telemetria; los pasos de abajo se enfocan en las acciones
a tomar una vez que un revisor ha sido seleccionado.

- **Alcance:** revisores que necesitan acceso al preview de docs (`docs-preview.sora`,
  בונה דפי GitHub או חבילות של SoraFS) לפני GA.
- **Fuera de alcance:** Operatores de Torii או SoraFS (קוביות פור sus propios kits de onboarding)
  y despliegues del portal de produccion (ver
  [`devportal/deploy-guide`](./deploy-guide.md)).

## תפקידים ודרישות מוקדמות

| רול | Objetivos tipicos | Artefactos requeridos | Notas |
| --- | --- | --- | --- |
| מתחזק ליבה | אימות חדש, בדיקות עשן. | ידית GitHub, יצירת קשר עם מטריקס, חברת CLA בארכיון. | Uualmente ya esta en el equipo GitHub `docs-preview`; אאונ אסי registra una solicitud para que el acceso sea auditable. |
| סוקר שותף | קטעי קוד תקפים של SDK או תוכן דיגיטלי לפני הפרסום. | דוא"ל corporativo, POC legal, terminos de preview firmados. | Debe reconocer los requerimientos de telemetria + manejo de data. |
| מתנדב קהילתי | הצגת משוב על שימושי. | ידית GitHub, contacto preferido, so horario, acceptation del CoC. | Mantener cohortes pequenas; priorizar revisores que han firmado el acuerdo de contribucion. |

כל הטיפים לחשבון החשבון:

1. Reconocer la politica de uso acceptable para artefactos de preview.
2. Leer los apendices de seguridad/observabilidad
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md)).
3. Aceptar Ejecutar `docs/portal/scripts/preview_verify.sh` antes de servir cualquier
   תמונת מצב מקומית.

## צריכת נוזלים1. Pedir al solicitante que complete el
   [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)
   נוסחאות (o copiar/pegar en una issue). לכידת התפריטים: זיהוי, שיטה ליצירת קשר,
   ידית ה-GitHub, אישור הגרסה הקודמת ואישור המסמכים של הסגירה.
2. הרשם la solicitud en el tracker `docs-preview` (הנפקה של GitHub או כרטיס טיסה)
   y asignar un aprobador.
3. דרישות קדם תקפות:
   - CLA / acuerdo de contribucion en archivo (o referencia de contrato partner).
   - Reconocimiento de uso aceptable almacenado en la solicitud.
   - Evaluacion de riesgo completa (לפי דוגמה, שותף עורך דין מאושר על ידי משפטי).
4. El aprobador firma en la solicitud y enlaza la issue de tracking con cualquier
   אנטרדה של ניהול שינויים (דוגמה: `DOCS-SORA-Preview-####`).

## Aprovisionamiento y herramientas

1. **Compartir artefactos** - Proporcionar el descriptor + archivo de preview mas reciente desde
   זרימת עבודה של CI או פין SoraFS (ארטפקטו `docs-portal-preview`). תקליטו או בודקים
   ejecutar:

   ```bash
   ./docs/portal/scripts/preview_verify.sh \
     --build-dir build \
     --descriptor artifacts/preview-descriptor.json \
     --archive artifacts/preview-site.tar.gz
   ```

2. **Servir con enforcement de checksum** - Indicar a los revisores el comando con gate de checksum:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

   Esto reutiliza `scripts/serve-verified-preview.mjs` para que no se lance un build sin verificar
   במקרה.

3. **השתמש ב-GitHub (אופציונלי)** - Si los revisores necesitan ramas no publicadas, agregarlos
   אל equipo GitHub `docs-preview` נמשך עדכון ורשם el cambio de membresia en la solicitud.

4. **Comunicar canales de soporte** - Compartir el contacto on call (Matrix/Slack) y el procedimiento
   de incidentes de [`incident-runbooks`](./incident-runbooks.md).

5. **טלמטריה + משוב** - Recordar a los revisores que se recopila analitica anonimizada
   (ver [`observability`](./observability.md)). Proporcionar el formulario de feedback o la plantilla
   de issue referenciada en la invitacion y registrar el evento con el helper
   [`preview-feedback-log`](./preview-feedback-log) para que el resumen de ola se mantenga al dia.

## רשימת ביקורת של המבקר

Antes de acceder al preview, los revisores deben completar lo suuiente:

1. Verificar los artefactos descargados (`preview_verify.sh`).
2. פורטל Lanzar el via `npm run serve` (o `serve:verified`) para asegurar que el guard de checksum esta activo.
3. Leer las notas de seguridad y observabilidad enlazadas arriba.
4. נסה את הקונסולה OAuth/נסה את זה באמצעות כניסת קוד המכשיר (ביישום) ושימוש מחדש באסימוני ייצור.
5. הרשם hallazgos en el tracker acordado (הנפקה, מסמך מפורט או נוסחאות) y etiquetarlos
   con el tag de release de preview.

## אחריות תחזוקה ויציאה מהמטוס| פאזה | Acciones |
| --- | --- |
| בעיטה | Confirmar que el checklist de intake esta adjunto a la solicitud, compartir artefactos + instrucciones, agregar una entrada `invite-sent` via [`preview-feedback-log`](./preview-feedback-log), y de mit agenda du sync. |
| Monitoreo | Monitorear טלמטריה מקדימה (בקרת תנועה נסה את זה לא רגיל, בדיקה) וסגור את ספר תקריות ההפעלה והאירועים האלה. רשם אירועי `feedback-submitted`/`issue-opened` תואם את הלגאן הולאזגוס פאר que las metricas de la ola se mantengan precisas. |
| יציאה למטוס | Revocer acceso temporal de GitHub o SoraFS, רשם `access-revoked`, archivar la solicitud (כולל קורות חיים של משוב + המלצות), וממשיכים את רישום הבוחנים. Pedir al revisor que elimine builds locales y adjuntar el digest generado desde [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md). |

Usa el mismo processo al rotar revisores entre olas. Mantener el Rastro en el repo (גיליון + plantillas) ayuda
a que DOCS-SORA siga siendo auditable y permite a gobernanza confirmar que el acceso de preview siguio
los controls documentados.

## Plantillas de invitacion y tracking

- Inicia todo outreach con el
  [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
  ארכיון. Captura el lenguaje legal minimo, las instrucciones de checksum de preview y la expectativa
  de que los revisores reconozcan la politica de uso מקובל.
- Al editar la plantilla, reemplaza los placeholders para `<preview_tag>`, `<request_ticket>` y canales
  de contacto. Guarda una copia del mensaje final en el ticket de intake para que revisores, aprobadores
  y auditores puedan referenciar el texto exacto que se envio.
- Despues de enviar la invitacion, actualiza la hoja de tracking o issue con el timestamp `invite_sent_at`
  y la fecha esperada de cierre para que el reporte de
  [תצוגה מקדימה של הזמנת זרימת](./preview-invite-flow.md) pueda detectar la cohorte automaticamente.