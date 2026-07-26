---
lang: he
direction: rtl
source: docs/portal/docs/devportal/preview-feedback/w1/plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: preview-feedback-w1-plan
כותרת: Plan de preflight de partners W1
sidebar_label: תוכנית W1
תיאור: Tareas, Responsables y checklist de evidencia para la cohorte de preview de partners.
---

| פריט | פרטים |
| --- | --- |
| אולה | W1 - Partners y integradores de Torii |
| Ventana objetivo | Q2 2025 Semana 3 |
| Tag de artefacto (planeado) | `preview-2025-04-12` |
| גיליון del tracker | `DOCS-SORA-Preview-W1` |

## אובייקטיביות

1. Asegurar aprobaciones legales y de gobernanza para los terminos de preview de partners.
2. הכן את ה-proxy נסה את זה ותמונות בזק של טלמטריה בארה"ב ב-al paquete de invitacion.
3. Refrescar el artefacto de preview verificado por checksum y los resultados de probes.
4. סיים את רשימת השותפים והבקשות לפני הקנאה.

## Desglose de tareas

| תעודת זהות | טארא | אחראי | Fecha limite | Estado | Notas |
| --- | --- | --- | --- | --- | --- |
| W1-P1 | Obtener aprobacion legal para el anexo de terminos de preview | Docs/DevRel lead -> משפטי | 2025-04-05 | Completado | כרטיס חוקי `DOCS-SORA-Preview-W1-Legal` aprobado el 2025-04-05; PDF תוספת למעקב. |
| W1-P2 | Capturar Ventana de staging del proxy נסה את זה (2025-04-10) y validar salud del proxy | Docs/DevRel + Ops | 2025-04-06 | Completado | Se ejecuto `npm run manage:tryit-proxy -- --stage preview-w1 --expires-in=21d --target https://tryit-preprod.sora` el 2025-04-06; תמלול של CLI y `.env.tryit-proxy.bak` ארכיון. |
| W1-P3 | Construir artefacto de preview (`preview-2025-04-12`), correr `scripts/preview_verify.sh` + `npm run probe:portal`, מתאר/סיכומי בדיקה של ארכיון | פורטל TL | 2025-04-08 | Completado | Artefacto y logs de verificacion guardados en `artifacts/docs_preview/W1/preview-2025-04-12/`; salida de probe adjunta al tracker. |
| W1-P4 | עדכון נוסחאות כניסת שותפים (`DOCS-SORA-Preview-REQ-P01...P08`), אישור אנשי קשר ו-NDAs | קשר ממשל | 2025-04-07 | Completado | Las ocho solicitudes aprobadas (las ultimas dos el 2025-04-11); aprobaciones enlazadas en el tracker. |
| W1-P5 | Redactar copy de invitacion (basado en `docs/examples/docs_preview_invite_template.md`), fijar `<preview_tag>` y `<request_ticket>` para cada partner | Docs/DevRel lead | 2025-04-08 | Completado | Borrador de invitacion enviado el 2025-04-12 15:00 UTC junto con enlaces de artefacto. |

## רשימת בדיקה מוקדמת

> התקנה: ejecuta `scripts/preview_wave_preflight.sh --tag preview-2025-04-12 --base-url https://preview.staging.sora --descriptor artifacts/preview-2025-04-12/descriptor.json --archive artifacts/preview-2025-04-12/docs-portal-preview.tar.zst --tryit-target https://tryit-proxy.staging.sora --output-json artifacts/preview-2025-04-12/preflight-summary.json` עבור ejecutar los pasos 1-5 אוטומטית (בנה, אימות בדיקת סכום, בדיקה של פורטל, בודק קישורים ואקטואליזציה של proxy נסה זאת). רישום הסקריפט של JSON יומן את הנושא של המעקב.

1. `npm run build` (con `DOCS_RELEASE_TAG=preview-2025-04-12`) עבור מחדש `build/checksums.sha256` y `build/release.json`.
2. `docs/portal/scripts/preview_verify.sh --build-dir docs/portal/build --descriptor artifacts/<tag>/descriptor.json --archive artifacts/<tag>/docs-portal-preview.tar.zst`.
3. `PORTAL_BASE_URL=https://preview.staging.sora DOCS_RELEASE_TAG=preview-2025-04-12 npm run probe:portal -- --expect-release=preview-2025-04-12`.
4. `DOCS_RELEASE_TAG=preview-2025-04-12 npm run check:links` y archivar `build/link-report.json` junto al descriptor.
5. `npm run manage:tryit-proxy -- update --target https://tryit-proxy.staging.sora` (o pasar el target adecuado via `--tryit-target`); commitea el `.env.tryit-proxy` בפועל ושמירה על `.bak` עבור החזרה לאחור.
6. אקטואליזציה של בעיה W1 עם רישומי יומנים (בדיקת סכום תיאור, בדיקה, שרת פרוקסי נסה את זה ותמונות Snapshot Grafana).

## רשימת הוכחות- [x] Aprobacion legal firmada (PDF o enlace al ticket) adjunta a `DOCS-SORA-Preview-W1`.
- [x] צילומי מסך של Grafana עבור `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`.
- [x] Descriptor y log de checksum de `preview-2025-04-12` guardados bajo `artifacts/docs_preview/W1/`.
- [x] לוח ההזמנות עם חותמות זמן `invite_sent_at` השלמות (ולוג W1 del tracker).
- [x] Artefactos de feedback reflejados en [`preview-feedback/w1/log.md`](./log.md) עם שותף לשותף (אקטואליזדו 2025-04-26 עם נתונים בסגל/טלמטריה/בעיות).

Actualiza este plan a medida que avancen las tareas; el tracker lo referencia para mantener מפת הדרכים ניתנת לביקורת.

## Flujo de feedback

1. מבקר Para cada, duplica la plantilla en
   [`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md),
   completa los metadatos y guarda la copia terminada bajo
   `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`.
2. הזמנות קורות חיים, מחסומים של טלמטריה ובעיות אבiertos dentro del log vivo en
   [`preview-feedback/w1/log.md`](./log.md) para que los reviewers de gobernanza puedan revisar toda la ola
   sin salir del repositorio.
3. Cuando lleguen exports de know-check o encuestas, adjuntalos en la ruta de artefactos indicada en el log
   y enlaza el issue del tracker.