---
lang: es
direction: ltr
source: docs/portal/docs/devportal/reviewer-onboarding.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Incorporación de revisores de vista previa

## Resumen

DOCS-SORA sigue un lanzamiento escalado del portal de desarrolladores. Los builds con gate de checksum
(`npm run serve`) y los flujos Try it reforzados desbloquean el siguiente hito:
onboarding de revisores validados antes de que el avance público se abra de forma amplia. Esta guia
describir como recopilar solicitudes, verificar elegibilidad, aprovisionar acceso y dar de baja
participantes de forma segura. Consulta el
[vista previa del flujo de invitación](./preview-invite-flow.md) para la planificacion de cohortes, la
cadencia de invitaciones y las exportaciones de telemetria; los pasos de abajo se enfocan en las acciones
a tomar una vez que un revisor ha sido seleccionado.

- **Alcance:** revisores que necesitan acceso al avance de documentos (`docs-preview.sora`,
  compilaciones de páginas de GitHub o paquetes de SoraFS) antes de GA.
- **Fuera de alcance:** operadores de Torii o SoraFS (cubiertos por sus propios kits de onboarding)
  y despliegues del portal de producción (ver
  [`devportal/deploy-guide`](./deploy-guide.md)).

## Roles y requisitos previos| papel | Objetivos típicos | Artefactos requeridos | Notas |
| --- | --- | --- | --- |
| Mantenedor principal | Verificar nuevas guias, ejecutar pruebas de humo. | Identificador de GitHub, contacto Matrix, CLA firmada en archivo. | Generalmente ya está en el equipo GitHub `docs-preview`; aun asi registra una solicitud para que el acceso sea auditable. |
| Revisor socio | Validar fragmentos de SDK o contenido de gobernanza antes del lanzamiento público. | Correo electrónico corporativo, POC legal, términos de vista previa firmados. | Debe reconocer los requerimientos de telemetría + manejo de datos. |
| Voluntario comunitario | Aportar comentarios de usabilidad sobre guías. | Mango de GitHub, contacto preferido, horario de casa, aceptación del CoC. | Mantener cohortes pequeñas; priorizar revisores que han firmado el acuerdo de contribución. |

Todos los tipos de revisores deben:

1. Reconocer la política de uso aceptable para artefactos de vista previa.
2. Leer los apéndices de seguridad/observabilidad
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md)).
3. Aceptar ejecutar `docs/portal/scripts/preview_verify.sh` antes de servir cualquier
   instantánea localmente.

## Flujo de ingesta1. Pedir al solicitante que complete el
   [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)
   formulario (o copiar/pegar en una edición). Capturar al menos: identidad, método de contacto,
   Manija de GitHub, fechas de revisión previstas y confirmación de que los documentos de seguridad fueron leídos.
2. Registrar la solicitud en el tracker `docs-preview` (emisión de GitHub o ticket de gobernanza)
   y asignar un aprobador.
3. Validar requisitos previos:
   - CLA / acuerdo de contribución en archivo (o referencia de contrato socio).
   - Reconocimiento de uso aceptable almacenado en la solicitud.
   - Evaluación de riesgo completa (por ejemplo, revisores socios aprobados por Legal).
4. El aprobador firma en la solicitud y enlaza la emisión de seguimiento con cualquier
   entrada de gestión de cambios (ejemplo: `DOCS-SORA-Preview-####`).

## Aprovisionamiento y herramientas

1. **Compartir artefactos** - Proporcionar el descriptor + archivo de vista previa más reciente desde
   el flujo de trabajo de CI o el pin de SoraFS (artefacto `docs-portal-preview`). Grabar a los revisores
   ejecutar:

   ```bash
   ./docs/portal/scripts/preview_verify.sh \
     --build-dir build \
     --descriptor artifacts/preview-descriptor.json \
     --archive artifacts/preview-site.tar.gz
   ```

2. **Servir con enforzamiento de checksum** - Indicar a los revisores el comando con gate de checksum:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

   Esto reutiliza `scripts/serve-verified-preview.mjs` para que no se lance un build sin verificar
   por accidente.3. **Dar acceso a GitHub (opcional)** - Si los revisores necesitan ramas no publicadas, agréguelos
   al equipo GitHub `docs-preview` durante la revisión y registrar el cambio de membresía en la solicitud.

4. **Comunicar canales de soporte** - Compartir el contacto de guardia (Matrix/Slack) y el procedimiento
   de incidentes de [`incident-runbooks`](./incident-runbooks.md).

5. **Telemetria + feedback** - Recordar a los revisores que se recopila analitica anonimizada
   (ver [`observability`](./observability.md)). Proporcionar el formulario de comentarios o la plantilla
   de tema referenciada en la invitación y registrar el evento con el ayudante
   [`preview-feedback-log`](./preview-feedback-log) para que el resumen de ola se mantenga al día.

## Lista de verificación del revisor

Antes de acceder al avance, los revisores deben completar lo siguiente:

1. Verificar los artefactos descargados (`preview_verify.sh`).
2. Lanzar el portal vía `npm run serve` (o `serve:verified`) para asegurar que el guardia de checksum está activo.
3. Leer las notas de seguridad y observabilidad enlazadas arriba.
4. Probar la consola OAuth/Try it usando login de dispositivo-code (si aplica) y evitar reutilizar tokens de producción.
5. Registrar encuentra en el tracker acordado (issue, doc compartido o formulario) y etiquetarlos
   con la etiqueta de lanzamiento de vista previa.

## Responsabilidades de mantenedores y offboarding| Fase | Acciones |
| --- | --- |
| Inicio | Confirmar que el checklist de ingesta está adjunto a la solicitud, compartir artefactos + instrucciones, agregar una entrada `invite-sent` via [`preview-feedback-log`](./preview-feedback-log), y agendar un sync de mitad de período si la revisión dura más de una semana. |
| Monitoreo | Monitorear telemetría de vista previa (buscar tráfico Try it inusual, fallas de sonda) y seguir el runbook de incidentes si ocurre algo sospechoso. Registrar eventos `feedback-submitted`/`issue-opened` conforme llegan hallazgos para que las métricas de la ola se mantengan precisas. |
| Baja de embarque | Revocar acceso temporal de GitHub o SoraFS, registrar `access-revoked`, archivar la solicitud (incluir resumen de feedback + acciones pendientes), y actualizar el registro de revisores. Solicite al revisor que elimine compilaciones locales y adjunte el resumen generado desde [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md). |

Usa el mismo proceso al rotar revisores entre olas. Mantener el rastro en el repo (edición + plantillas) ayuda
a que DOCS-SORA siga siendo auditable y permite a gobernanza confirmar que el acceso de vista previa sigue
los controles documentados.

## Plantillas de invitación y seguimiento- Inicia todo alcance con el
  [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
  archivo. Captura el lenguaje legal mínimo, las instrucciones de checksum de previa y la expectativa
  de que los revisores reconozcan la política de uso aceptable.
- Al editar la plantilla, reemplaza los marcadores de posición para `<preview_tag>`, `<request_ticket>` y canales.
  de contacto. Guarde una copia del mensaje final en el ticket de admisión para que revisores, aprobadores
  y auditores puedan referenciar el texto exacto que se envió.
- Después de enviar la invitación, actualiza la hoja de seguimiento o emisión con la marca de tiempo `invite_sent_at`
  y la fecha esperada de cierre para que el reporte de
  [vista previa del flujo de invitación](./preview-invite-flow.md) pueda detectar la cohorte automáticamente.