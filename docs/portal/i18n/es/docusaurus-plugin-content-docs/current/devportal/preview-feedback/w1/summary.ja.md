---
lang: ja
direction: ltr
source: docs/portal/i18n/es/docusaurus-plugin-content-docs/current/devportal/preview-feedback/w1/summary.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3fed0f01255fa3f913502447ab0e230b1b671a16deaaa9083a99d8c1ba2a4da4
source_last_modified: "2026-01-03T18:07:57+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: preview-feedback-w1-summary
lang: es
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/summary.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

| Item | Detalles |
| --- | --- |
| Ola | W1 - Partners e integradores de Torii |
| Ventana de invitacion | 2025-04-12 -> 2025-04-26 |
| Tag de artefacto | `preview-2025-04-12` |
| Issue del tracker | `DOCS-SORA-Preview-W1` |
| Participantes | sorafs-op-01...03, torii-int-01...02, sdk-partner-01...02, gateway-ops-01 |

## Destacados

1. **Flujo de checksum** - Todos los reviewers verificaron el descriptor/archive via `scripts/preview_verify.sh`; los logs se guardaron junto a los acuses de invitacion.
2. **Telemetria** - Los dashboards `docs.preview.integrity`, `TryItProxyErrors` y `DocsPortal/GatewayRefusals` se mantuvieron en verde durante toda la ola; no hubo incidentes ni paginas de alerta.
3. **Feedback de docs (`docs-preview/w1`)** - Se registraron dos nits menores:
   - `docs-preview/w1 #1`: aclarar wording de navegacion en la seccion Try it (resuelto).
   - `docs-preview/w1 #2`: actualizar screenshot de Try it (resuelto).
4. **Paridad de runbooks** - Operadores de SoraFS confirmaron que los nuevos cross-links entre `orchestrator-ops` y `multi-source-rollout` resolvieron sus preocupaciones de W0.

## Acciones

| ID | Descripcion | Responsable | Estado |
| --- | --- | --- | --- |
| W1-A1 | Actualizar wording de navegacion de Try it segun `docs-preview/w1 #1`. | Docs-core-02 | Completado (2025-04-18). |
| W1-A2 | Actualizar screenshot de Try it segun `docs-preview/w1 #2`. | Docs-core-03 | Completado (2025-04-19). |
| W1-A3 | Resumir hallazgos de partners y evidencia de telemetria en roadmap/status. | Docs/DevRel lead | Completado (ver tracker + status.md). |

## Resumen de cierre (2025-04-26)

- Los ocho reviewers confirmaron finalizacion durante las office hours finales, limpiaron artefactos locales y se revoco su acceso.
- La telemetria se mantuvo en verde hasta el cierre; snapshots finales adjuntos a `DOCS-SORA-Preview-W1`.
- El log de invitaciones se actualizo con acuses de salida; el tracker marco W1 como completado y agrego los checkpoints.
- Paquete de evidencia (descriptor, checksum log, probe output, transcript del proxy Try it, screenshots de telemetria, feedback digest) archivado bajo `artifacts/docs_preview/W1/`.

## Siguientes pasos

- Preparar el plan de intake comunitario W2 (aprobacion de gobernanza + ajustes de template de solicitud).
- Refrescar el tag de artefacto de preview para la ola W2 y reejecutar el script de preflight cuando se finalicen fechas.
- Volcar hallazgos aplicables de W1 en roadmap/status para que la ola comunitaria tenga la guia mas reciente.
