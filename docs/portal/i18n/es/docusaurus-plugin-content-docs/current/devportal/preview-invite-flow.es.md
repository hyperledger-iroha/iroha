---
lang: es
direction: ltr
source: docs/portal/docs/devportal/preview-invite-flow.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Flujo de invitaciones de vista previa

## propuesta

El item del roadmap **DOCS-SORA** identifica el onboarding de revisores y el programa de invitaciones de vista previa pública como los bloqueadores finales antes de que el portal salga de beta. Esta página describe cómo abrir cada ola de invitaciones, que artefactos deben enviarse antes de mandar invitaciones y como demostrar que el flujo es auditable. Usala junto con:

- [`devportal/reviewer-onboarding`](./reviewer-onboarding.md) para el manejo por revisor.
- [`devportal/preview-integrity-plan`](./preview-integrity-plan.md) para garantías de suma de comprobación.
- [`devportal/observability`](./observability.md) para exportaciones de telemetría y ganchos de alertas.

## Plan de olas| ola | Audiencia | Criterios de entrada | Criterios de salida | Notas |
| --- | --- | --- | --- | --- |
| **W0 - Núcleo de mantenedores** | Mantenedores de Docs/SDK validando contenido del día uno. | Equipo GitHub `docs-portal-preview` poblado, gate de checksum en `npm run serve` en verde, Alertmanager silencioso por 7 días. | Todos los docs P0 revisados, backlog etiquetado, sin incidentes bloqueantes. | Se usa para validar el flujo; no hay correo electrónico de invitación, solo se comparten los artefactos de vista previa. |
| **W1 - Socios** | Operadores SoraFS, integradores Torii, revisores de gobernanza bajo NDA. | W0 cerrado, términos legales aprobados, proxy Try-it y staging. | Sign-off de partners (emisión o formulario firmado) recogido, telemetria muestra =2 publicaciones de documentación enviadas a través de canal de vista previa sin reversión. | Limitar invitaciones concurrentes (<=25) y agrupar semanalmente. |

Documenta que ola esta activa en `status.md` y en el rastreador de solicitudes de vista previa para que la gobernanza vea el estado de un vistazo.

## Lista de verificación de verificación previaCompleta estas acciones **antes** de programar invitaciones para una ola:

1. **Artefactos de CI disponibles**
   - El último `docs-portal-preview` + descriptor cargado por `.github/workflows/docs-portal-preview.yml`.
   - Pin de SoraFS anotado en `docs/portal/docs/devportal/deploy-guide.md` (descriptor de corte presente).
2. **Cumplimiento de la suma de verificación**
   - `docs/portal/scripts/serve-verified-preview.mjs` invocado a través de `npm run serve`.
   - Instrucciones de `scripts/preview_verify.sh` probadas en macOS + Linux.
3. **Línea base de telemetría**
   - `dashboards/grafana/docs_portal.json` muestra trafico Try it saludable y la alerta `docs.preview.integrity` esta en verde.
   - Último apéndice de `docs/portal/docs/devportal/observability.md` actualizado con enlaces de Grafana.
4. **Artefactos de gobernanza**
   - Problema del rastreador de invitaciones listo (un problema por ola).
   - Plantilla de registro de revisores copiada (ver [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)).
   - Aprobaciones legales y de SRE requeridas adjuntas a la emisión.

Registre la finalización del preflight en el invite tracker antes de enviar cualquier correo.

## Pasos del flujo1. **Seleccionar candidatos**
   - Extraer de la hoja de espera o cola de socios.
   - Asegurar que cada candidato tenga la plantilla de solicitud completa.
2. **Aprobar acceso**
   - Asignar un aprobador a la emisión del rastreador de invitaciones.
   - Verificar prerequisitos (CLA/contrato, uso aceptable, brief de seguridad).
3. **Enviar invitaciones**
   - Completar los marcadores de posición de [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) (`<preview_tag>`, `<request_ticket>`, contactos).
   - Adjuntar el descriptor + hash del archivo, URL de staging de Try it y canales de soporte.
   - Guarde el correo electrónico final (o transcripción de Matrix/Slack) en la edición.
4. **Incorporación de Rastrear**
   - Actualizar el rastreador de invitaciones con `invite_sent_at`, `expected_exit_at`, y estado (`pending`, `active`, `complete`, `revoked`).
   - Enlazar la solicitud de ingreso del revisor para auditabilidad.
5. **Monitorear telemetría**
   - Vigilar `docs.preview.session_active` y alertas `TryItProxyErrors`.
   - Abrir un incidente si la telemetria se desvia del baseline y registrar el resultado junto a la entrada de invitación.
6. **Recolectar comentarios y cerrar**
   - Cerrar invitaciones cuando el feedback llegue o `expected_exit_at` se cumpla.
   - Actualizar la cuestión de la ola con un resumen corto (hallazgos, incidentes, siguientes acciones) antes de pasar al siguiente cohorte.

## Evidencia y reportes| Artefacto | donde guardar | Cadencia de actualización |
| --- | --- | --- |
| Rastreador de invitaciones emitidas | Proyecto GitHub `docs-portal-preview` | Actualizar después de cada invitación. |
| Exportación del roster de revisores | Registro enlazado en `docs/portal/docs/devportal/reviewer-onboarding.md` | Semanal. |
| Instantáneas de telemetría | `docs/source/sdk/android/readiness/dashboards/<date>/` (reutilizar paquete de telemetría) | Por ola + despues de incidentes. |
| Resumen de comentarios | `docs/portal/docs/devportal/preview-feedback/<wave>/summary.md` (crear carpeta por ola) | Dentro de 5 días tras salir de la ola. |
| Nota de reunión de gobernanza | `docs/portal/docs/devportal/preview-invite-notes/<date>.md` | Completar antes de cada sincronización de gobernanza DOCS-SORA. |

Ejecuta `cargo xtask docs-preview summary --wave <wave_label> --json artifacts/docs_portal_preview/<wave_label>_summary.json`
despues de cada lote para producir un resumen legible por maquinas. Adjunta el JSON renderizado a la edición de la ola para que los revisores de gobernanza confirmen los conteos de invitaciones sin reproducir todo el log.

Adjunta la lista de evidencia a `status.md` cada vez que una ola termine para que la entrada del roadmap pueda actualizarse rápidamente.

## Criterios de reversión y pausa

Pausa el flujo de invitaciones (y notifica a gobernanza) cuando ocurre cualquiera de estos casos:- Un incidente de proxy Pruébelo que requiere reversión (`npm run manage:tryit-proxy`).
- Fatiga de alertas: >3 páginas de alerta para puntos finales solo de vista previa dentro de 7 días.
- Brecha de cumplimiento: invitación enviada sin términos firmados o sin registrar la plantilla de solicitud.
- Riesgo de integridad: falta de coincidencia de suma de comprobación detectada por `scripts/preview_verify.sh`.

Reanuda solo despues de documentar la remediacion en el invite tracker y confirmar que el tablero de telemetria este estable por al menos 48 horas.