---
lang: he
direction: rtl
source: docs/portal/docs/devportal/public-preview-invite.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Playbook de invitaciones del publico תצוגה מקדימה

## אובייקטיבוס של תוכנה

Este Playbook explica como anunciar y ejecutar el תצוגה מקדימה publico una vez que el
זרימת עבודה של ההטמעה של ה-Revisores este activo. Mantiene honesto el מפת הדרכים DOCS-SORA al
asegurar que cada invitacion se envie con artefactos verificables, guia de seguridad y un
camino claro de feedback.

- **Audiencia:** רשימה של מימברוס דה לה קומונידאד, שותפים ומנהלים
  firmaron la politica de uso קבילה תצוגה מקדימה.
- **מגבלות:** tamano de ola por defecto <= 25 revisores, ventana de acceso de 14 dias, respuesta
  תקריות 24 שעות ביממה.

## רשימת רשימת השערים

הזמנה מלאה לפני הקנאה:

1. Ultimos artefactos de preview cargados en CI (`docs-portal-preview`,
   Manifest de Checksum, Descriptor, Bundle SoraFS).
2. `npm run --prefix docs/portal serve` (gateado por checksum) probado en el mismo tag.
3. כרטיסים ל- onboarding de revisores aprobados y enlazados a la ola de invitaciones.
4. Docs de seguridad, observabilidad e incidentes validados
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md)).
5. נוסחת משוב או הכנה לסוגיה (incluye campos de severidad,
   pasos de reproduction, צילומי מסך e info de entorno).
6. Texto del anuncio revisado por Docs/DevRel + Governance.

## Paquete de invitacion

Cada invitacion debe כולל:

1. **Artefactos verificados** - Proporciona enlaces al manifiesto/plan de SoraFS o a los
   artefactos de GitHub mas el manifest de checksum y el descriptor. Referencia el comando
   de verificacion explicitamente para que los revisores puedan ejecutarlo antes de levantar
   el sitio.
2. **Instrucciones de serve** - כולל תצוגה מקדימה gateado por checksum:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

3. **Recordatorios de seguridad** - Indica que los tokens expiran automaticamente, los links
   no deben compartirse y los incidentes deben reportarse de inmediato.
4. **Canal de feedback** - Enlaza la plantilla/formulario y aclara expectativas de tiempos de respuesta.
5. **Fechas del programa** - Proporciona fechas de inicio/fin, שעות המשרד o syncs, y la proxima
   ventana de refresh.

El email de muestra en
[`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
cubre estos requisitos. מצייני מקום בפועל (נקודות, כתובות אתרים, אנשי קשר)
antes de enviar.

## Exponer el host de preview

סולו promociona el host de preview una vez que el onboarding este completo y el ticket de cambio
este aprobado. Consulta la [guia de exposicion del host de preview](./preview-host-exposure.md)
para los pasos מקצה לקצה de build/publish/verify usados en esta seccion.

1. **בנה ותגבור:** תגית שחרור תג ייצור חפצים דטרמיניסטים.

   ```bash
   cd docs/portal
   export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
   npm ci
   npm run build
   ./scripts/sorafs-pin-release.sh \
     --alias docs-preview.sora \
     --alias-namespace docs \
     --alias-name preview \
     --pin-label docs-preview \
     --skip-submit
   node scripts/generate-preview-descriptor.mjs \
     --manifest artifacts/checksums.sha256 \
     --archive artifacts/sorafs/portal.tar.gz \
     --out artifacts/sorafs/preview-descriptor.json
   ```ה-script de pin escribe `portal.car`, `portal.manifest.*`, `portal.pin.proposal.json`,
   y `portal.dns-cutover.json` bajo `artifacts/sorafs/`. Adjunta esos archivos a la ola de
   invitaciones para que cada revisor pueda verificar los mismos bits.

2. **Publicar el alias de preview:** Repite el comando sin `--skip-submit`
   (פרופורציונה `TORII_URL`, `AUTHORITY`, `PRIVATE_KEY[_FILE]`, y la prueba de alias emitida
   por gobernanza). El script enlazara el manifest a `docs-preview.sora` y emitira
   `portal.manifest.submit.summary.json` mas `portal.pin.report.json` para el bundle de evidencia.

3. **Probar el despliegue:** אישור que el alias resuelve y que el checksum coincide con el tag
   antes de enviar invitaciones.

   ```bash
   npm run probe:portal -- \
     --base-url=https://docs-preview.sora.link \
     --expect-release="$DOCS_RELEASE_TAG"
   ```

   Mantener `npm run serve` (`scripts/serve-verified-preview.mjs`) a mano como fallback para
   que los revisores puedan levantar una copia local si el edge de preview falla.

## ציר זמן של תקשורת

| דיא | אקציון | בעלים |
| --- | --- | --- |
| D-3 | גמר עותק הזמנה, חפצי אמנות מחודשים, אימות יבשה | Docs/DevRel |
| D-2 | Sign-off de gobernanza + ticket de cambio | Docs/DevRel + ממשל |
| D-1 | קבלו הזמנה לצמחייה, מעקב אחר מציאות עם רשימה של יעדים | Docs/DevRel |
| ד | שיחת בעיטה / שעות עבודה, לוחות מחוונים של monitorear de telemetria | Docs/DevRel + כוננות |
| D+7 | Digest de feedback de mitad de ola, triage de issues bloqueantes | Docs/DevRel |
| D+14 | Cerrar ola, revocar acceso temporal, resume public en `status.md` | Docs/DevRel |

## Seguimiento de acceso y telemetria

1. Registra cada destinatario, timestamp de invitacion y fecha de revocacion con el
   לוגר משוב תצוגה מקדימה (ver
   [`preview-feedback-log`](./preview-feedback-log)) para que cada ola comparta el mismo
   ראסטרו דה evidencia:

   ```bash
   # Agrega un nuevo evento de invitacion a artifacts/docs_portal_preview/feedback_log.json
   npm run --prefix docs/portal preview:log -- \
     --wave preview-20250303 \
     --recipient alice@example.com \
     --event invite-sent \
     --notes "wave-01 seed"
   ```

   Los eventos soportados son `invite-sent`, `acknowledged`,
   `feedback-submitted`, `issue-opened`, y `access-revoked`. El log vive en
   `artifacts/docs_portal_preview/feedback_log.json` על ידי פגום; adjuntalo al ticket de
   la ola de invitaciones junto con los formularios de consentimiento. Usa el helper de
   תקציר להפקת קורות חיים הניתנים לביקורת אנטות דה לה נוטה דה סייר:

   ```bash
   npm run --prefix docs/portal preview:summary -- --summary-json \
     > artifacts/docs_portal_preview/preview-20250303-summary.json
   ```

   תקציר ההזמנות של JSON עבור אולה, יעדי אבירטוס,
   משוב y el timestamp del evento mas reciente. El helper esta respaldado por
   [`scripts/preview-feedback-log.mjs`](../../scripts/preview-feedback-log.mjs),
   asi que el mismo workflow puede correr localmente o en CI. Usa la plantilla de digest en
   [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md)
   al publicar el recap de la ola.
2. לוחות המחוונים של תקנון טלמטריה קון אל `DOCS_RELEASE_TAG` usado para la ola para que
   los picos se puedan correlacionar con las cohortes de invitacion.
3. Ejecuta `npm run probe:portal -- --expect-release=<tag>` despues del deploy para confirmar
   que el entorno de preview anuncia la metadata correcta de release.
4. Registra cualquier incidente en la plantilla del runbook y enlazalo a la cohorte.

## משוב y cierre1. משוב אגרה ו-un doc compartido o tablero de issues. פריטי נימוס קו
   `docs-preview/<wave>` עבור הבעלים של מפת הדרכים לסיוע בייעוץ.
2. תקציר ארה"ב ל-Salida del Preview Logger para poblar el reporte de la ola, לוגו קורות חיים
   la cohorte en `status.md` (משתתפים, hallazgos principales, fixes planeados) y
   actualiza `roadmap.md` si el hito DOCS-SORA cambio.
3. Sigue los pasos de offboarding de
   [`reviewer-onboarding`](./reviewer-onboarding.md): revoca acceso, archiva solicitudes y
   agradece a los participantes.
4. Prepara la suuiente ola refrescando artefactos, re-ejecutando los gates de checksum y
   actualizando la plantilla de invitacion con nuevas fechas.

אפליקציית Playbook de forma consistente mantiene el programa de previewable auditable y
le da a Docs/DevRel una forma repetible de escalar invitaciones a medida que el portal se
Acerca a GA.