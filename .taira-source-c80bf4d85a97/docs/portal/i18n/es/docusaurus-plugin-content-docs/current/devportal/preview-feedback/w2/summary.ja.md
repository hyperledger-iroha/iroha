---
lang: ja
direction: ltr
source: docs/portal/i18n/es/docusaurus-plugin-content-docs/current/devportal/preview-feedback/w2/summary.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b438434d5c889e920e693ee5601eaf3a9dcf30b483dd77f79d17dd8dc9c3ebbd
source_last_modified: "2026-01-03T18:07:57+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: preview-feedback-w2-summary
lang: es
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w2/summary.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

| Item | Detalles |
| --- | --- |
| Ola | W2 - Reviewers comunitarios |
| Ventana de invitacion | 2025-06-15 -> 2025-06-29 |
| Tag de artefacto | `preview-2025-06-15` |
| Issue del tracker | `DOCS-SORA-Preview-W2` |
| Participantes | comm-vol-01 ... comm-vol-08 |

## Destacados

1. **Gobernanza y tooling** - La politica de intake comunitario fue aprobada por unanimidad el 2025-05-20; el template de solicitud actualizado con campos de motivacion/zona horaria vive en `docs/examples/docs_preview_request_template.md`.
2. **Evidencia de preflight** - El cambio del proxy Try it `OPS-TRYIT-188` se ejecuto el 2025-06-09, dashboards de Grafana capturados, y los outputs de descriptor/checksum/probe de `preview-2025-06-15` archivados bajo `artifacts/docs_preview/W2/`.
3. **Ola de invitaciones** - Ocho reviewers comunitarios invitados el 2025-06-15, con acknowledgements registrados en la tabla de invitaciones del tracker; todos completaron verificacion de checksum antes de navegar.
4. **Feedback** - `docs-preview/w2 #1` (wording de tooltip) y `#2` (orden de sidebar de localizacion) se registraron el 2025-06-18 y se resolvieron para 2025-06-21 (Docs-core-04/05); no hubo incidentes durante la ola.

## Acciones

| ID | Descripcion | Responsable | Estado |
| --- | --- | --- | --- |
| W2-A1 | Atender `docs-preview/w2 #1` (wording de tooltip). | Docs-core-04 | Completado 2025-06-21 |
| W2-A2 | Atender `docs-preview/w2 #2` (sidebar de localizacion). | Docs-core-05 | Completado 2025-06-21 |
| W2-A3 | Archivar evidencia de salida + actualizar roadmap/status. | Docs/DevRel lead | Completado 2025-06-29 |

## Resumen de salida (2025-06-29)

- Los ocho reviewers comunitarios confirmaron finalizacion y se les revoco el acceso al preview; acknowledgements registrados en el log de invitaciones del tracker.
- Los snapshots finales de telemetria (`docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`) se mantuvieron verdes; logs y transcripts del proxy Try it adjuntos a `DOCS-SORA-Preview-W2`.
- Bundle de evidencia (descriptor, checksum log, probe output, link report, screenshots de Grafana, acknowledgements de invitacion) archivado bajo `artifacts/docs_preview/W2/preview-2025-06-15/`.
- El log de checkpoints W2 del tracker se actualizo hasta el cierre, asegurando que el roadmap mantenga un registro auditable antes de iniciar la planificacion de W3.
