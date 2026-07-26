---
lang: es
direction: ltr
source: docs/examples/docs_preview_feedback_digest.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4b1191d3475126df594d0f9f81d91f8ebbcd002c74a1f4d4176f2f42a59ca885
source_last_modified: "2025-11-19T07:56:11.635822+00:00"
translation_last_reviewed: 2026-01-01
---

# Resumen de feedback de vista previa del portal de docs (Plantilla)

Usa esta plantilla cuando resumas una ola de preview para governance, revisiones de
release o `status.md`. Copia el Markdown en el ticket de tracking, reemplaza los
placeholders con datos reales y adjunta el resumen JSON exportado via
`npm run --prefix docs/portal preview:log -- --summary --summary-json`. El helper
`preview:digest` (`npm run --prefix docs/portal preview:digest -- --wave <label>`) genera
la seccion de metricas mostrada abajo para que solo completes las filas de
highlights/actions/artefacts.

```markdown
## Digest de feedback de la ola preview-<tag> (YYYY-MM-DD)
- Ventana de invitacion: <start -> end>
- Revisores invitados: <count> (abiertos: <count>)
- Envios de feedback: <count>
- Issues abiertos: <count>
- Ultimo timestamp de evento: <ISO8601 from summary.json>

| Categoria | Detalles | Responsable / Seguimiento |
| --- | --- | --- |
| Highlights | <p. ej., "ISO builder walkthrough landed well"> | <responsable + fecha limite> |
| Hallazgos bloqueantes | <lista de issue IDs o links del tracker> | <responsable> |
| Items menores de pulido | <agrupar cambios cosmeticos o de copy> | <responsable> |
| Anomalias de telemetria | <link a snapshot de dashboard / log de probe> | <responsable> |

## Acciones
1. <Accion + link + ETA>
2. <Segunda accion opcional>

## Artefactos
- Feedback log: `artifacts/docs_portal_preview/feedback_log.json` (`sha256:<digest>`)
- Resumen de ola: `artifacts/docs_portal_preview/preview-<tag>-summary.json`
- Snapshot de dashboard: `<link o ruta>`

```

Mantiene cada digest junto al ticket de tracking de invitaciones para que revisores y
governance puedan reconstruir el rastro de evidencia sin revisar logs de CI.
