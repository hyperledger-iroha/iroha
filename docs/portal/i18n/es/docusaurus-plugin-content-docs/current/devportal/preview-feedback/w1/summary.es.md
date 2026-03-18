---
lang: es
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/summary.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: vista previa-comentarios-w1-resumen
título: Resumen de retroalimentación y cierre W1
sidebar_label: Resumen W1
descripción: Hallazgos, acciones y evidencia de cierre para la ola de vista previa de socios e integradores Torii.
---

| Artículo | Detalles |
| --- | --- |
| ola | W1 - Socios e integradores de Torii |
| Ventana de invitación | 2025-04-12 -> 2025-04-26 |
| Etiqueta de artefacto | `preview-2025-04-12` |
| Problema del rastreador | `DOCS-SORA-Preview-W1` |
| Participantes | sorafs-op-01...03, torii-int-01...02, sdk-partner-01...02, gateway-ops-01 |

## Destacados

1. **Flujo de checksum** - Todos los revisores verificaron el descriptor/archivo vía `scripts/preview_verify.sh`; los logs se guardaron junto a los acuses de invitacion.
2. **Telemetria** - Los tableros `docs.preview.integrity`, `TryItProxyErrors` y `DocsPortal/GatewayRefusals` se mantuvieron en verde durante toda la ola; no hubo incidentes ni paginas de alerta.
3. **Feedback de docs (`docs-preview/w1`)** - Se registraron dos nits menores:
   - `docs-preview/w1 #1`: aclarar el texto de navegación en la sección Pruébalo (resuelto).
   - `docs-preview/w1 #2`: actualización de captura de pantalla de Try it (resuelto).
4. **Paridad de runbooks** - Operadores de SoraFS confirmaron que los nuevos cross-links entre `orchestrator-ops` y `multi-source-rollout` resolvieron sus preocupaciones de W0.

## Acciones| identificación | Descripción | Responsable | Estado |
| --- | --- | --- | --- |
| W1-A1 | Actualizar el texto de navegación de Pruébelo según `docs-preview/w1 #1`. | Documentos-core-02 | Completado (2025-04-18). |
| W1-A2 | Actualizar captura de pantalla de Pruébelo según `docs-preview/w1 #2`. | Documentos-core-03 | Completado (2025-04-19). |
| W1-A3 | Resumir hallazgos de socios y evidencia de telemetría en roadmap/status. | Líder de Docs/DevRel | Completado (ver tracker + status.md). |

## Resumen de cierre (2025-04-26)

- Los ocho revisores confirmaron la finalización durante las finales del horario de oficina, limpiaron artefactos locales y se revoco su acceso.
- La telemetria se mantuvo en verde hasta el cierre; instantáneas finales adjuntos a `DOCS-SORA-Preview-W1`.
- El registro de invitaciones se actualiza con acusaciones de salida; el tracker marco W1 como completado y agrega los puntos de control.
- Paquete de evidencia (descriptor, registro de suma de verificación, salida de sonda, transcripción del proxy Pruébelo, capturas de pantalla de telemetría, resumen de comentarios) archivado bajo `artifacts/docs_preview/W1/`.

## Siguientes pasos

- Preparar el plan de ingesta comunitario W2 (aprobación de gobernanza + ajustes de plantilla de solicitud).
- Refrescar el tag de artefacto de previa para la ola W2 y reejecutar el script de preflight cuando se finalicen fechas.
- Volcar hallazgos aplicables de W1 en roadmap/status para que la ola comunitaria tenga la guía más reciente.