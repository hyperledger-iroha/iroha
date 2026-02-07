---
lang: es
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w0/summary.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: vista previa-comentarios-w0-resumen
título: Resumen de retroalimentación de mitad de W0
sidebar_label: Comentarios W0 (mitad)
descripción: Puntos de control, hallazgos y acciones de mitad de ola para la ola de vista previa de mantenedores core.
---

| Artículo | Detalles |
| --- | --- |
| ola | W0 - Núcleo de mantenedores |
| Fecha del resumen | 2025-03-27 |
| Ventana de revisión | 2025-03-25 -> 2025-04-08 |
| Participantes | docs-core-01, sdk-rust-01, sdk-js-01, sorafs-ops-01, observabilidad-01 |
| Etiqueta de artefacto | `preview-2025-03-24` |

## Destacados

1. **Flujo de checksum** - Todos los revisores confirmaron que `scripts/preview_verify.sh`
   tuvo éxito contra el par descriptor/archivo compartido. No se requirieron
   anula los manuales.
2. **Feedback de navegación** - Se registraron dos problemas menores de orden del sidebar
   (`docs-preview/w0 #1-#2`). Ambos se asignan a Docs/DevRel y no bloquean la
   ola.
3. **Paridad de runbooks de SoraFS** - sorafs-ops-01 pidio enlaces cruzados más claros
   entre `sorafs/orchestrator-ops` y `sorafs/multi-source-rollout`. Se abrió un
   emisión de seguimiento; se atenderá antes de W1.
4. **Revisión de telemetria** - observability-01 confirmo que `docs.preview.integrity`,
   `TryItProxyErrors` y los logs del proxy Try-it se mantuvieron en verde; no se
   dispararon alertas.

## Acciones| identificación | Descripción | Responsable | Estado |
| --- | --- | --- | --- |
| W0-A1 | Reordenar entradas de la barra lateral del portal de desarrollo para destacar documentos enfocados en revisores (`preview-invite-*` agrupados). | Documentos-core-01 | Completado - la barra lateral ahora lista los documentos de revisores de forma continua (`docs/portal/sidebars.js`). |
| W0-A2 | Agregar enlace cruzado explícito entre `sorafs/orchestrator-ops` y `sorafs/multi-source-rollout`. | Sorafs-ops-01 | Completado - cada runbook ahora enlaza al otro para que los operadores vean ambas guías durante los lanzamientos. |
| W0-A3 | Compartir instantáneas de telemetría + paquete de consultas con el tracker de gobernanza. | Observabilidad-01 | Completado - paquete adjunto a `DOCS-SORA-Preview-W0`. |

## Resumen de cierre (2025-04-08)

- Los cinco revisores confirmaron la finalización, limpiaron builds locales y salieron de la
  ventana de vista previa; las revocaciones de acceso quedaron registradas en `DOCS-SORA-Preview-W0`.
- No hubo incidentes ni alertas durante la ola; los paneles de telemetria se mantuvieron
  en verde todo el periodo.
- Las acciones de navegación + enlaces cruzados (W0-A1/A2) están implementadas y reflejadas en
  los documentos de arriba; la evidencia de telemetría (W0-A3) está adjunta al rastreador.
- Paquete de evidencia archivado: capturas de pantalla de telemetria, acuses de invitacion y este
  resumen están enlazados desde el número del tracker.

## Siguientes pasos- Implementar los elementos de acción de W0 antes de abrir W1.
- Obtener aprobación legal y un slot de staging para el proxy, luego seguir los pasos de
  verificación previa de la ola de partners detallados en el [flujo de invitación preliminar](../../preview-invite-flow.md).

_Este resumen esta enlazado desde el [preview invite tracker](../../preview-invite-tracker.md) para
mantener el roadmap DOCS-SORA trazable._