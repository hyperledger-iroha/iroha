---
lang: es
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w3/summary.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 76a4303fa2657476a3f983f1aa5597c9ddb478f670d233b0a7cf4e3791419a72
source_last_modified: "2025-11-20T12:45:46.606949+00:00"
translation_last_reviewed: 2026-01-01
---

---
id: preview-feedback-w3-summary
title: Resumen de feedback y estado W3 beta
sidebar_label: Resumen W3
description: Resumen en vivo para la ola beta 2026 (cohortes de finanzas, observabilidad, SDK y ecosistema).
---

| Item | Detalles |
| --- | --- |
| Ola | W3 - Cohortes beta (finanzas + ops + partner SDK + advocate de ecosistema) |
| Ventana de invitacion | 2026-02-18 -> 2026-02-28 |
| Tag de artefacto | `preview-20260218` |
| Issue del tracker | `DOCS-SORA-Preview-W3` |
| Participantes | finance-beta-01, observability-ops-02, partner-sdk-03, ecosystem-advocate-04 |

## Destacados

1. **Pipeline de evidencia end-to-end.** `npm run preview:wave -- --wave preview-20260218 --invite-start 2026-02-18 --invite-end 2026-02-28 --report-date 2026-03-01 --notes "Finance/observability beta wave"` genera el resumen por ola (`artifacts/docs_portal_preview/preview-20260218-summary.json`), el digest (`preview-20260218-digest.md`) y refresca `docs/portal/src/data/previewFeedbackSummary.json` para que los reviewers de gobernanza puedan depender de un solo comando.
2. **Cobertura de telemetria y gobernanza.** Los cuatro reviewers reconocieron acceso con checksum, enviaron feedback y se les revoco a tiempo; el digest referencia los issues de feedback (`docs-preview/20260218` set + `DOCS-SORA-Preview-20260218`) junto con los runs de Grafana capturados durante la ola.
3. **Visibilidad en el portal.** La tabla del portal actualizada ahora muestra la ola W3 cerrada con metricas de latencia y tasa de respuesta, y la nueva pagina de log abajo refleja la linea de tiempo para auditores que no descargan el log JSON crudo.

## Acciones

| ID | Descripcion | Responsable | Estado |
| --- | --- | --- | --- |
| W3-A1 | Capturar el digest de preview y adjuntar al tracker. | Docs/DevRel lead | Completado 2026-02-28 |
| W3-A2 | Reflejar evidencia de invitacion/digest en portal + roadmap/status. | Docs/DevRel lead | Completado 2026-02-28 |

## Resumen de salida (2026-02-28)

- Invitaciones enviadas 2026-02-18 con acknowledgements registrados minutos despues; acceso de preview revocado 2026-02-28 tras pasar la ultima verificacion de telemetria.
- Digest y resumen guardados bajo `artifacts/docs_portal_preview/`, con el log crudo anclado por `artifacts/docs_portal_preview/feedback_log.json` para reproducibilidad.
- Follow-ups de issues archivados bajo `docs-preview/20260218` con el tracker de gobernanza `DOCS-SORA-Preview-20260218`; notas de CSP/Try it enviadas a los owners de observabilidad/finanzas y enlazadas desde el digest.
- La fila del tracker se marco como Completed y la tabla de feedback del portal refleja la ola cerrada, completando la tarea beta restante de DOCS-SORA.
