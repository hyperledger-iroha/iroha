---
lang: es
direction: ltr
source: docs/portal/docs/devportal/preview-invite-flow.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Flujo de invitaciones de preview

## Proposito

El item del roadmap **DOCS-SORA** identifica el onboarding de revisores y el programa de invitaciones de preview publico como los bloqueadores finales antes de que el portal salga de beta. Esta pagina describe como abrir cada ola de invitaciones, que artefactos deben enviarse antes de mandar invites y como demostrar que el flujo es auditable. Usala junto con:

- [`devportal/reviewer-onboarding`](./reviewer-onboarding.md) para el manejo por revisor.
- [`devportal/preview-integrity-plan`](./preview-integrity-plan.md) para garantias de checksum.
- [`devportal/observability`](./observability.md) para exports de telemetria y hooks de alertas.

## Plan de olas

| Ola | Audiencia | Criterios de entrada | Criterios de salida | Notas |
| --- | --- | --- | --- | --- |
| **W0 - Maintainers core** | Maintainers de Docs/SDK validando contenido del dia uno. | Equipo GitHub `docs-portal-preview` poblado, gate de checksum en `npm run serve` en verde, Alertmanager silencioso por 7 dias. | Todos los docs P0 revisados, backlog etiquetado, sin incidentes bloqueantes. | Se usa para validar el flujo; no hay email de invitacion, solo se comparten los artefactos de preview. |
| **W1 - Partners** | Operadores SoraFS, integradores Torii, revisores de gobernanza bajo NDA. | W0 cerrado, terminos legales aprobados, proxy Try-it en staging. | Sign-off de partners (issue o formulario firmado) recogido, telemetria muestra <=10 revisores concurrentes, sin regresiones de seguridad por 14 dias. | Aplicar plantilla de invitacion + tickets de solicitud. |
| **W2 - Comunidad** | Contribuidores seleccionados de la lista de espera de la comunidad. | W1 cerrado, drills de incidentes ensayados, FAQ publico actualizado. | Feedback digerido, >=2 releases de documentacion enviados via pipeline de preview sin rollback. | Limitar invitaciones concurrentes (<=25) y agrupar semanalmente. |

Documenta que ola esta activa en `status.md` y en el tracker de solicitudes de preview para que la gobernanza vea el estado de un vistazo.

## Checklist de preflight

Completa estas acciones **antes** de programar invitaciones para una ola:

1. **Artefactos de CI disponibles**
   - El ultimo `docs-portal-preview` + descriptor cargado por `.github/workflows/docs-portal-preview.yml`.
   - Pin de SoraFS anotado en `docs/portal/docs/devportal/deploy-guide.md` (descriptor de cutover presente).
2. **Enforcement de checksum**
   - `docs/portal/scripts/serve-verified-preview.mjs` invocado via `npm run serve`.
   - Instrucciones de `scripts/preview_verify.sh` probadas en macOS + Linux.
3. **Baseline de telemetria**
   - `dashboards/grafana/docs_portal.json` muestra trafico Try it saludable y la alerta `docs.preview.integrity` esta en verde.
   - Ultimo apendice de `docs/portal/docs/devportal/observability.md` actualizado con enlaces de Grafana.
4. **Artefactos de gobernanza**
   - Issue del invite tracker listo (una issue por ola).
   - Plantilla de registro de revisores copiada (ver [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)).
   - Aprobaciones legales y de SRE requeridas adjuntas a la issue.

Registra la finalizacion del preflight en el invite tracker antes de enviar cualquier correo.

## Pasos del flujo

1. **Seleccionar candidatos**
   - Extraer de la hoja de espera o cola de partners.
   - Asegurar que cada candidato tenga la plantilla de solicitud completa.
2. **Aprobar acceso**
   - Asignar un aprobador a la issue del invite tracker.
   - Verificar prerequisitos (CLA/contrato, uso aceptable, brief de seguridad).
3. **Enviar invitaciones**
   - Completar los placeholders de [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) (`<preview_tag>`, `<request_ticket>`, contactos).
   - Adjuntar el descriptor + hash del archive, URL de staging de Try it, y canales de soporte.
   - Guardar el email final (o transcript de Matrix/Slack) en la issue.
4. **Rastrear onboarding**
   - Actualizar el invite tracker con `invite_sent_at`, `expected_exit_at`, y estado (`pending`, `active`, `complete`, `revoked`).
   - Enlazar la solicitud de ingreso del revisor para auditabilidad.
5. **Monitorear telemetria**
   - Vigilar `docs.preview.session_active` y alertas `TryItProxyErrors`.
   - Abrir un incidente si la telemetria se desvia del baseline y registrar el resultado junto a la entrada de invitacion.
6. **Recolectar feedback y cerrar**
   - Cerrar invitaciones cuando el feedback llegue o `expected_exit_at` se cumpla.
   - Actualizar la issue de la ola con un resumen corto (hallazgos, incidentes, siguientes acciones) antes de pasar al siguiente cohorte.

## Evidencia y reportes

| Artefacto | Donde guardar | Cadencia de actualizacion |
| --- | --- | --- |
| Issue del invite tracker | Proyecto GitHub `docs-portal-preview` | Actualizar despues de cada invite. |
| Export del roster de revisores | Registro enlazado en `docs/portal/docs/devportal/reviewer-onboarding.md` | Semanal. |
| Snapshots de telemetria | `docs/source/sdk/android/readiness/dashboards/<date>/` (reusar bundle de telemetria) | Por ola + despues de incidentes. |
| Digest de feedback | `docs/portal/docs/devportal/preview-feedback/<wave>/summary.md` (crear carpeta por ola) | Dentro de 5 dias tras salir de la ola. |
| Nota de reunion de gobernanza | `docs/portal/docs/devportal/preview-invite-notes/<date>.md` | Completar antes de cada sync de gobernanza DOCS-SORA. |

Ejecuta `cargo xtask docs-preview summary --wave <wave_label> --json artifacts/docs_portal_preview/<wave_label>_summary.json`
despues de cada lote para producir un digest legible por maquinas. Adjunta el JSON renderizado a la issue de la ola para que los revisores de gobernanza confirmen los conteos de invitaciones sin reproducir todo el log.

Adjunta la lista de evidencia a `status.md` cada vez que una ola termine para que la entrada del roadmap pueda actualizarse rapido.

## Criterios de rollback y pausa

Pausa el flujo de invitaciones (y notifica a gobernanza) cuando ocurra cualquiera de estos casos:

- Un incidente de proxy Try it que requirio rollback (`npm run manage:tryit-proxy`).
- Fatiga de alertas: >3 alert pages para endpoints solo de preview dentro de 7 dias.
- Brecha de cumplimiento: invitacion enviada sin terminos firmados o sin registrar la plantilla de solicitud.
- Riesgo de integridad: mismatch de checksum detectado por `scripts/preview_verify.sh`.

Reanuda solo despues de documentar la remediacion en el invite tracker y confirmar que el dashboard de telemetria este estable por al menos 48 horas.
