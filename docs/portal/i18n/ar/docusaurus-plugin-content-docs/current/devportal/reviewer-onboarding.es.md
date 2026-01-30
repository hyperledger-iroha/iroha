---
lang: ar
direction: rtl
source: docs/portal/docs/devportal/reviewer-onboarding.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Onboarding de revisores de preview

## Resumen

DOCS-SORA sigue un lanzamiento escalonado del portal de desarrolladores. Los builds con gate de checksum
(`npm run serve`) y los flujos Try it reforzados desbloquean el siguiente hito:
onboarding de revisores validados antes de que el preview publico se abra de forma amplia. Esta guia
describe como recopilar solicitudes, verificar elegibilidad, aprovisionar acceso y dar de baja
participantes de forma segura. Consulta el
[preview invite flow](./preview-invite-flow.md) para la planificacion de cohortes, la
cadencia de invitaciones y los exports de telemetria; los pasos de abajo se enfocan en las acciones
a tomar una vez que un revisor ha sido seleccionado.

- **Alcance:** revisores que necesitan acceso al preview de docs (`docs-preview.sora`,
  builds de GitHub Pages o bundles de SoraFS) antes de GA.
- **Fuera de alcance:** operadores de Torii o SoraFS (cubiertos por sus propios kits de onboarding)
  y despliegues del portal de produccion (ver
  [`devportal/deploy-guide`](./deploy-guide.md)).

## Roles y prerequisitos

| Rol | Objetivos tipicos | Artefactos requeridos | Notas |
| --- | --- | --- | --- |
| Core maintainer | Verificar nuevas guias, ejecutar smoke tests. | GitHub handle, contacto Matrix, CLA firmada en archivo. | Usualmente ya esta en el equipo GitHub `docs-preview`; aun asi registra una solicitud para que el acceso sea auditable. |
| Partner reviewer | Validar snippets de SDK o contenido de gobernanza antes del release publico. | Email corporativo, POC legal, terminos de preview firmados. | Debe reconocer los requerimientos de telemetria + manejo de datos. |
| Community volunteer | Aportar feedback de usabilidad sobre guias. | GitHub handle, contacto preferido, huso horario, aceptacion del CoC. | Mantener cohortes pequenas; priorizar revisores que han firmado el acuerdo de contribucion. |

Todos los tipos de revisores deben:

1. Reconocer la politica de uso aceptable para artefactos de preview.
2. Leer los apendices de seguridad/observabilidad
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md)).
3. Aceptar ejecutar `docs/portal/scripts/preview_verify.sh` antes de servir cualquier
   snapshot localmente.

## Flujo de intake

1. Pedir al solicitante que complete el
   [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)
   formulario (o copiar/pegar en una issue). Capturar al menos: identidad, metodo de contacto,
   GitHub handle, fechas de revision previstas y confirmacion de que los docs de seguridad fueron leidos.
2. Registrar la solicitud en el tracker `docs-preview` (issue de GitHub o ticket de gobernanza)
   y asignar un aprobador.
3. Validar prerequisitos:
   - CLA / acuerdo de contribucion en archivo (o referencia de contrato partner).
   - Reconocimiento de uso aceptable almacenado en la solicitud.
   - Evaluacion de riesgo completa (por ejemplo, revisores partner aprobados por Legal).
4. El aprobador firma en la solicitud y enlaza la issue de tracking con cualquier
   entrada de change-management (ejemplo: `DOCS-SORA-Preview-####`).

## Aprovisionamiento y herramientas

1. **Compartir artefactos** - Proporcionar el descriptor + archivo de preview mas reciente desde
   el workflow de CI o el pin de SoraFS (artefacto `docs-portal-preview`). Recordar a los revisores
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
   por accidente.

3. **Dar acceso a GitHub (opcional)** - Si los revisores necesitan ramas no publicadas, agregarlos
   al equipo GitHub `docs-preview` durante la revision y registrar el cambio de membresia en la solicitud.

4. **Comunicar canales de soporte** - Compartir el contacto on-call (Matrix/Slack) y el procedimiento
   de incidentes de [`incident-runbooks`](./incident-runbooks.md).

5. **Telemetria + feedback** - Recordar a los revisores que se recopila analitica anonimizada
   (ver [`observability`](./observability.md)). Proporcionar el formulario de feedback o la plantilla
   de issue referenciada en la invitacion y registrar el evento con el helper
   [`preview-feedback-log`](./preview-feedback-log) para que el resumen de ola se mantenga al dia.

## Checklist del revisor

Antes de acceder al preview, los revisores deben completar lo siguiente:

1. Verificar los artefactos descargados (`preview_verify.sh`).
2. Lanzar el portal via `npm run serve` (o `serve:verified`) para asegurar que el guard de checksum esta activo.
3. Leer las notas de seguridad y observabilidad enlazadas arriba.
4. Probar la consola OAuth/Try it usando login de device-code (si aplica) y evitar reutilizar tokens de produccion.
5. Registrar hallazgos en el tracker acordado (issue, doc compartido o formulario) y etiquetarlos
   con el tag de release de preview.

## Responsabilidades de maintainers y offboarding

| Fase | Acciones |
| --- | --- |
| Kickoff | Confirmar que el checklist de intake esta adjunto a la solicitud, compartir artefactos + instrucciones, agregar una entrada `invite-sent` via [`preview-feedback-log`](./preview-feedback-log), y agendar un sync de mitad de periodo si la revision dura mas de una semana. |
| Monitoreo | Monitorear telemetria de preview (buscar trafico Try it inusual, fallas de probe) y seguir el runbook de incidentes si ocurre algo sospechoso. Registrar eventos `feedback-submitted`/`issue-opened` conforme llegan hallazgos para que las metricas de la ola se mantengan precisas. |
| Offboarding | Revocar acceso temporal de GitHub o SoraFS, registrar `access-revoked`, archivar la solicitud (incluir resumen de feedback + acciones pendientes), y actualizar el registro de revisores. Pedir al revisor que elimine builds locales y adjuntar el digest generado desde [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md). |

Usa el mismo proceso al rotar revisores entre olas. Mantener el rastro en el repo (issue + plantillas) ayuda
a que DOCS-SORA siga siendo auditable y permite a gobernanza confirmar que el acceso de preview siguio
los controles documentados.

## Plantillas de invitacion y tracking

- Inicia todo outreach con el
  [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
  archivo. Captura el lenguaje legal minimo, las instrucciones de checksum de preview y la expectativa
  de que los revisores reconozcan la politica de uso aceptable.
- Al editar la plantilla, reemplaza los placeholders para `<preview_tag>`, `<request_ticket>` y canales
  de contacto. Guarda una copia del mensaje final en el ticket de intake para que revisores, aprobadores
  y auditores puedan referenciar el texto exacto que se envio.
- Despues de enviar la invitacion, actualiza la hoja de tracking o issue con el timestamp `invite_sent_at`
  y la fecha esperada de cierre para que el reporte de
  [preview invite flow](./preview-invite-flow.md) pueda detectar la cohorte automaticamente.
