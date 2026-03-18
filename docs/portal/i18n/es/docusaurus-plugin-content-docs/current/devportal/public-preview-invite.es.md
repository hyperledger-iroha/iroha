---
lang: es
direction: ltr
source: docs/portal/docs/devportal/public-preview-invite.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Playbook de invitaciones del avance público

## Objetivos del programa

Este playbook explica como anunciar y ejecutar el avance público una vez que el
flujo de trabajo de incorporación de revisores este activo. Mantiene honesto el roadmap DOCS-SORA al
asegurar que cada invitación se envie con artefactos verificables, guía de seguridad y un
camino claro de retroalimentación.

- **Audiencia:** lista curada de miembros de la comunidad, socios y mantenedores que
  firmaron la política de uso aceptable del previo.
- **Limites:** tamano de ola por defecto <= 25 revisores, ventana de acceso de 14 días, respuesta
  a incidentes en 24h.

## Lista de verificación de puerta de lanzamiento

Completa estas tareas antes de enviar cualquier invitación:

1. Últimos artefactos de vista previa cargados en CI (`docs-portal-preview`,
   manifiesto de suma de comprobación, descriptor, paquete SoraFS).
2. `npm run --prefix docs/portal serve` (gateado por checksum) probado en el mismo tag.
3. Tickets de onboarding de revisores aprobados y enlazados a la ola de invitaciones.
4. Documentos de seguridad, observabilidad e incidentes validados
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md)).
5. Formulario de retroalimentación o plantilla de emisión preparada (incluye campos de severidad,
   pasos de reproducción, capturas de pantalla e información de entorno).
6. Texto del anuncio revisado por Docs/DevRel + Governance.## Paquete de invitación

Cada invitación debe incluir:

1. **Artefactos verificados** - Proporciona enlaces al manifiesto/plan de SoraFS o a los
   Los artefactos de GitHub son el manifiesto de suma de comprobación y el descriptor. Referencia del comando
   de verificación explícitamente para que los revisores puedan ejecutarlo antes de levantar
   el sitio.
2. **Instrucciones de servicio** - Incluye el comando de vista previa gateado por checksum:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

3. **Recordatorios de seguridad** - Indica que los tokens caducan automáticamente, los enlaces
   no deben compartirse y los incidentes deben reportarse de inmediato.
4. **Canal de feedback** - Enlaza la plantilla/formulario y aclara expectativas de tiempos de respuesta.
5. **Fechas del programa** - Proporciona fechas de inicio/fin, office hours o syncs, y la proxima
   ventana de actualización.

El email de muestra en
[`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
cubre estos requisitos. Actualiza los marcadores de posición (fechas, URL, contactos)
antes de enviar.

## Exponer el host de vista previa

Solo promociona el host de previa una vez que el onboarding este completo y el ticket de cambio
este aprobado. Consulta la [guia de exposición del host de vista previa](./preview-host-exposure.md)
para los pasos de principio a fin de construir/publicar/verificar usados en esta sección.

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
   ```El script de pin escribe `portal.car`, `portal.manifest.*`, `portal.pin.proposal.json`,
   y `portal.dns-cutover.json` bajo `artifacts/sorafs/`. Adjunta esos archivos a la ola de
   invitaciones para que cada revisor pueda verificar los mismos bits.

2. **Publicar el alias de vista previa:** Repite el comando sin `--skip-submit`
   (proporciona `TORII_URL`, `AUTHORITY`, `PRIVATE_KEY[_FILE]`, y la prueba de alias emitida
   por gobernanza). El script enlazara el manifest a `docs-preview.sora` y emitira
   `portal.manifest.submit.summary.json` mas `portal.pin.report.json` para el paquete de evidencia.

3. **Probar el despliegue:** Confirma que el alias resuelve y que la suma de comprobación coincide con la etiqueta
   antes de enviar invitaciones.

   ```bash
   npm run probe:portal -- \
     --base-url=https://docs-preview.sora.link \
     --expect-release="$DOCS_RELEASE_TAG"
   ```

   Mantener `npm run serve` (`scripts/serve-verified-preview.mjs`) a mano como respaldo para
   que los revisores puedan levantar una copia local si el borde de vista previa falla.

## Cronología de comunicaciones| Día | Acción | Propietario |
| --- | --- | --- |
| D-3 | Finalizar copia de invitación, refrescar artefactos, simulacro de verificación | Documentos/DevRel |
| D-2 | Aprobación de gobernanza + ticket de cambio | Documentos/DevRel + Gobernanza |
| D-1 | Enviar invitaciones usando la plantilla, actualizar tracker con lista de destinatarios | Documentos/DevRel |
| D | Llamada inicial / horario de oficina, monitorear paneles de telemetria | Documentos/DevRel + De guardia |
| D+7 | Digest de feedback de mitad de ola, triage de issues bloqueantes | Documentos/DevRel |
| D+14 | Cerrar ola, revocar acceso temporal, publicar resumen en `status.md` | Documentos/DevRel |

## Seguimiento de acceso y telemetria

1. Registra cada destinatario, marca de tiempo de invitación y fecha de revocación con el
   registrador de comentarios de vista previa (ver
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
   `artifacts/docs_portal_preview/feedback_log.json` por defecto; adjuntelo al ticket de
   la ola de invitaciones junto con los formularios de consentimiento. Usa el ayudante de
   resumen para producir un resumen auditable antes de la nota de cierre:

   ```bash
   npm run --prefix docs/portal preview:summary -- --summary-json \
     > artifacts/docs_portal_preview/preview-20250303-summary.json
   ```El resumen JSON enumera invitaciones por ola, destinatarios abiertos, conteos de
   feedback y la marca de tiempo del evento más reciente. El ayudante esta respaldado por
   [`scripts/preview-feedback-log.mjs`](../../scripts/preview-feedback-log.mjs),
   Así que el mismo flujo de trabajo puede ejecutarse localmente o en CI. Usa la plantilla de digest en
   [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md)
   al publicar el resumen de la ola.
2. Etiqueta los paneles de telemetria con el `DOCS_RELEASE_TAG` usado para la ola para que
   los picos se puedan correlacionar con las cohortes de invitación.
3. Ejecuta `npm run probe:portal -- --expect-release=<tag>` después del despliegue para confirmar
   que el entorno de vista previa anuncia los metadatos correctos de liberación.
4. Registra cualquier incidente en la plantilla del runbook y enlazalo a la cohorte.

## Comentarios y cierre1. Agregue comentarios en un documento compartido o tablero de problemas. Etiquetar artículos con
   `docs-preview/<wave>` para que los propietarios del roadmap los consulten facilmente.
2. Usa la salida resumen del logger de vista previa para poblar el reporte de la ola, luego resume
   la cohorte en `status.md` (participantes, encuentra principales, fixes planeados) y
   Actualiza `roadmap.md` si el hito DOCS-SORA cambia.
3. Sigue los pasos de offboarding de
   [`reviewer-onboarding`](./reviewer-onboarding.md): revoca acceso, solicitudes de archivo y
   agradece a los participantes.
4. Prepara la siguiente ola refrescando artefactos, re-ejecutando los gates de checksum y
   actualizando la plantilla de invitación con nuevas fechas.

Aplicar este playbook de forma consistente mantiene el programa de vista previa auditable y
le da a Docs/DevRel una forma repetible de escalar invitaciones a medida que el portal se
cerca de GA.