---
lang: es
direction: ltr
source: docs/portal/docs/devportal/public-preview-invite.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Playbook de invitaciones del preview publico

## Objetivos del programa

Este playbook explica como anunciar y ejecutar el preview publico una vez que el
workflow de onboarding de revisores este activo. Mantiene honesto el roadmap DOCS-SORA al
asegurar que cada invitacion se envie con artefactos verificables, guia de seguridad y un
camino claro de feedback.

- **Audiencia:** lista curada de miembros de la comunidad, partners y maintainers que
  firmaron la politica de uso aceptable del preview.
- **Limites:** tamano de ola por defecto <= 25 revisores, ventana de acceso de 14 dias, respuesta
  a incidentes en 24h.

## Checklist de gate de lanzamiento

Completa estas tareas antes de enviar cualquier invitacion:

1. Ultimos artefactos de preview cargados en CI (`docs-portal-preview`,
   manifest de checksum, descriptor, bundle SoraFS).
2. `npm run --prefix docs/portal serve` (gateado por checksum) probado en el mismo tag.
3. Tickets de onboarding de revisores aprobados y enlazados a la ola de invitaciones.
4. Docs de seguridad, observabilidad e incidentes validados
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md)).
5. Formulario de feedback o plantilla de issue preparada (incluye campos de severidad,
   pasos de reproduccion, screenshots e info de entorno).
6. Texto del anuncio revisado por Docs/DevRel + Governance.

## Paquete de invitacion

Cada invitacion debe incluir:

1. **Artefactos verificados** - Proporciona enlaces al manifiesto/plan de SoraFS o a los
   artefactos de GitHub mas el manifest de checksum y el descriptor. Referencia el comando
   de verificacion explicitamente para que los revisores puedan ejecutarlo antes de levantar
   el sitio.
2. **Instrucciones de serve** - Incluye el comando de preview gateado por checksum:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

3. **Recordatorios de seguridad** - Indica que los tokens expiran automaticamente, los links
   no deben compartirse y los incidentes deben reportarse de inmediato.
4. **Canal de feedback** - Enlaza la plantilla/formulario y aclara expectativas de tiempos de respuesta.
5. **Fechas del programa** - Proporciona fechas de inicio/fin, office hours o syncs, y la proxima
   ventana de refresh.

El email de muestra en
[`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
cubre estos requisitos. Actualiza los placeholders (fechas, URLs, contactos)
antes de enviar.

## Exponer el host de preview

Solo promociona el host de preview una vez que el onboarding este completo y el ticket de cambio
este aprobado. Consulta la [guia de exposicion del host de preview](./preview-host-exposure.md)
para los pasos end-to-end de build/publish/verify usados en esta seccion.

1. **Build y empaquetado:** Marca el release tag y produce artefactos deterministas.

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
   ```

   El script de pin escribe `portal.car`, `portal.manifest.*`, `portal.pin.proposal.json`,
   y `portal.dns-cutover.json` bajo `artifacts/sorafs/`. Adjunta esos archivos a la ola de
   invitaciones para que cada revisor pueda verificar los mismos bits.

2. **Publicar el alias de preview:** Repite el comando sin `--skip-submit`
   (proporciona `TORII_URL`, `AUTHORITY`, `PRIVATE_KEY[_FILE]`, y la prueba de alias emitida
   por gobernanza). El script enlazara el manifest a `docs-preview.sora` y emitira
   `portal.manifest.submit.summary.json` mas `portal.pin.report.json` para el bundle de evidencia.

3. **Probar el despliegue:** Confirma que el alias resuelve y que el checksum coincide con el tag
   antes de enviar invitaciones.

   ```bash
   npm run probe:portal -- \
     --base-url=https://docs-preview.sora.link \
     --expect-release="$DOCS_RELEASE_TAG"
   ```

   Mantener `npm run serve` (`scripts/serve-verified-preview.mjs`) a mano como fallback para
   que los revisores puedan levantar una copia local si el edge de preview falla.

## Timeline de comunicaciones

| Dia | Accion | Owner |
| --- | --- | --- |
| D-3 | Finalizar copy de invitacion, refrescar artefactos, dry-run de verificacion | Docs/DevRel |
| D-2 | Sign-off de gobernanza + ticket de cambio | Docs/DevRel + Governance |
| D-1 | Enviar invitaciones usando la plantilla, actualizar tracker con lista de destinatarios | Docs/DevRel |
| D | Kickoff call / office hours, monitorear dashboards de telemetria | Docs/DevRel + On-call |
| D+7 | Digest de feedback de mitad de ola, triage de issues bloqueantes | Docs/DevRel |
| D+14 | Cerrar ola, revocar acceso temporal, publicar resumen en `status.md` | Docs/DevRel |

## Seguimiento de acceso y telemetria

1. Registra cada destinatario, timestamp de invitacion y fecha de revocacion con el
   preview feedback logger (ver
   [`preview-feedback-log`](./preview-feedback-log)) para que cada ola comparta el mismo
   rastro de evidencia:

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
   `artifacts/docs_portal_preview/feedback_log.json` por defecto; adjuntalo al ticket de
   la ola de invitaciones junto con los formularios de consentimiento. Usa el helper de
   summary para producir un resumen auditable antes de la nota de cierre:

   ```bash
   npm run --prefix docs/portal preview:summary -- --summary-json \
     > artifacts/docs_portal_preview/preview-20250303-summary.json
   ```

   El summary JSON enumera invitaciones por ola, destinatarios abiertos, conteos de
   feedback y el timestamp del evento mas reciente. El helper esta respaldado por
   [`scripts/preview-feedback-log.mjs`](../../scripts/preview-feedback-log.mjs),
   asi que el mismo workflow puede correr localmente o en CI. Usa la plantilla de digest en
   [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md)
   al publicar el recap de la ola.
2. Etiqueta los dashboards de telemetria con el `DOCS_RELEASE_TAG` usado para la ola para que
   los picos se puedan correlacionar con las cohortes de invitacion.
3. Ejecuta `npm run probe:portal -- --expect-release=<tag>` despues del deploy para confirmar
   que el entorno de preview anuncia la metadata correcta de release.
4. Registra cualquier incidente en la plantilla del runbook y enlazalo a la cohorte.

## Feedback y cierre

1. Agrega feedback en un doc compartido o tablero de issues. Etiqueta items con
   `docs-preview/<wave>` para que los owners del roadmap los consulten facilmente.
2. Usa la salida summary del preview logger para poblar el reporte de la ola, luego resume
   la cohorte en `status.md` (participantes, hallazgos principales, fixes planeados) y
   actualiza `roadmap.md` si el hito DOCS-SORA cambio.
3. Sigue los pasos de offboarding de
   [`reviewer-onboarding`](./reviewer-onboarding.md): revoca acceso, archiva solicitudes y
   agradece a los participantes.
4. Prepara la siguiente ola refrescando artefactos, re-ejecutando los gates de checksum y
   actualizando la plantilla de invitacion con nuevas fechas.

Aplicar este playbook de forma consistente mantiene el programa de preview auditable y
le da a Docs/DevRel una forma repetible de escalar invitaciones a medida que el portal se
acerca a GA.
