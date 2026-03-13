---
lang: he
direction: rtl
source: docs/portal/i18n/es/docusaurus-plugin-content-docs/current/devportal/preview-feedback/w1/log.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 49ef294e19d8cba62004786575313064f7f03fec084e62e6547d51e2fc721539
source_last_modified: "2026-01-03T18:07:57+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: preview-feedback-w1-log
lang: es
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/log.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

Este log mantiene el roster de invitaciones, checkpoints de telemetria y feedback de reviewers para el
**preview de partners W1** que acompana las tareas de aceptacion en
[`preview-feedback/w1/plan.md`](./plan.md) y la entrada del tracker de la ola en
[`../../preview-invite-tracker.md`](../../preview-invite-tracker.md). Actualizalo cuando se envie una invitacion,
se registre un snapshot de telemetria o se triagee un item de feedback para que los reviewers de gobernanza puedan reproducir
la evidencia sin perseguir tickets externos.

## Roster de cohorte

| Partner ID | Ticket de solicitud | NDA recibida | Invitacion enviada (UTC) | Ack/primer login (UTC) | Estado | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| partner-w1-01 | `DOCS-SORA-Preview-REQ-P01` | OK 2025-04-03 | 2025-04-12 15:00 | 2025-04-12 15:11 | Completado 2025-04-26 | sorafs-op-01; enfocado en evidencia de paridad de docs del orchestrator. |
| partner-w1-02 | `DOCS-SORA-Preview-REQ-P02` | OK 2025-04-03 | 2025-04-12 15:03 | 2025-04-12 15:15 | Completado 2025-04-26 | sorafs-op-02; valido cross-links de Norito/telemetria. |
| partner-w1-03 | `DOCS-SORA-Preview-REQ-P03` | OK 2025-04-04 | 2025-04-12 15:06 | 2025-04-12 15:18 | Completado 2025-04-26 | sorafs-op-03; ejecuto drills de failover multi-source. |
| partner-w1-04 | `DOCS-SORA-Preview-REQ-P04` | OK 2025-04-04 | 2025-04-12 15:09 | 2025-04-12 15:21 | Completado 2025-04-26 | torii-int-01; revision del cookbook de Torii `/v2/pipeline` + Try it. |
| partner-w1-05 | `DOCS-SORA-Preview-REQ-P05` | OK 2025-04-05 | 2025-04-12 15:12 | 2025-04-12 15:23 | Completado 2025-04-26 | torii-int-02; acompanio la actualizacion de screenshot de Try it (docs-preview/w1 #2). |
| partner-w1-06 | `DOCS-SORA-Preview-REQ-P06` | OK 2025-04-05 | 2025-04-12 15:15 | 2025-04-12 15:26 | Completado 2025-04-26 | sdk-partner-01; feedback de cookbooks JS/Swift + sanity checks del puente ISO. |
| partner-w1-07 | `DOCS-SORA-Preview-REQ-P07` | OK 2025-04-11 | 2025-04-12 15:18 | 2025-04-12 15:29 | Completado 2025-04-26 | sdk-partner-02; compliance aprobado 2025-04-11, enfocado en notas de Connect/telemetria. |
| partner-w1-08 | `DOCS-SORA-Preview-REQ-P08` | OK 2025-04-11 | 2025-04-12 15:21 | 2025-04-12 15:33 | Completado 2025-04-26 | gateway-ops-01; audito la guia de ops del gateway + flujo anonimo del proxy Try it. |

Completa los timestamps de **Invitacion enviada** y **Ack** apenas se emita el email saliente.
Ancla los tiempos al calendario UTC definido en el plan W1.

## Checkpoints de telemetria

| Timestamp (UTC) | Dashboards / probes | Responsable | Resultado | Artefacto |
| --- | --- | --- | --- | --- |
| 2025-04-06 18:05 | `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` | Docs/DevRel + Ops | Todo en verde | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250406` |
| 2025-04-06 18:20 | Transcript de `npm run manage:tryit-proxy -- --stage preview-w1` | Ops | Preparado | `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log` |
| 2025-04-12 14:45 | Dashboards de arriba + `probe:portal` | Docs/DevRel + Ops | Snapshot pre-invite, sin regresiones | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250412` |
| 2025-04-19 17:55 | Dashboards de arriba + diff de latencia del proxy Try it | Docs/DevRel lead | Chequeo de mitad de ola ok (0 alertas; latencia Try it p95=410 ms) | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250419` |
| 2025-04-26 16:25 | Dashboards de arriba + probe de salida | Docs/DevRel + Governance liaison | Snapshot de salida, cero alertas pendientes | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250426` |

Las muestras diarias de office hours (2025-04-13 -> 2025-04-25) se agrupan como exportes NDJSON + PNG bajo
`artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/` con nombres de archivo
`docs-preview-integrity-<date>.json` y los screenshots correspondientes.

## Log de feedback y issues

Usa esta tabla para resumir hallazgos enviados por reviewers. Enlaza cada entrada al ticket de GitHub/discuss
mas el formulario estructurado capturado via
[`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md).

| Referencia | Severidad | Responsable | Estado | Notas |
| --- | --- | --- | --- | --- |
| `docs-preview/w1 #1` | Low | Docs-core-02 | Resuelto 2025-04-18 | Se aclaro el wording de nav de Try it + ancla de sidebar (`docs/source/sorafs/tryit.md` actualizado con nuevo label). |
| `docs-preview/w1 #2` | Low | Docs-core-03 | Resuelto 2025-04-19 | Se actualizo screenshot de Try it + caption segun pedido del reviewer; artefacto `artifacts/docs_preview/W1/preview-2025-04-12/feedback/partner-w1-05/screenshot-diff.png`. |
| - | Info | Docs/DevRel lead | Cerrado | Los comentarios restantes fueron solo Q&A; capturados en el formulario de feedback de cada partner bajo `artifacts/docs_preview/W1/preview-2025-04-12/feedback/`. |

## Seguimiento de knowledge check y surveys

1. Registra los puntajes del quiz (objetivo >=90%) para cada reviewer; adjunta el CSV exportado junto a los artefactos de invitacion.
2. Recolecta las respuestas cualitativas del survey capturadas con el template de feedback y reflejalas bajo
   `artifacts/docs_preview/W1/preview-2025-04-12/surveys/`.
3. Agenda llamadas de remediacion para cualquiera por debajo del umbral y registralas en este archivo.

Los ocho reviewers marcaron >=94% en el knowledge check (CSV:
`artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`). No se requirieron llamadas de remediacion;
los exports de survey para cada partner viven bajo
`artifacts/docs_preview/W1/preview-2025-04-12/surveys/<partner-id>/summary.json`.

## Inventario de artefactos

- Bundle de descriptor/checksum de preview: `artifacts/docs_preview/W1/preview-2025-04-12/descriptor.json`
- Resumen de probe + link-check: `artifacts/docs_preview/W1/preview-2025-04-12/preflight-summary.json`
- Log de cambio del proxy Try it: `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log`
- Exportes de telemetria: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/<date>/`
- Bundle diario de telemetria de office hours: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/`
- Exportes de feedback + surveys: colocar carpetas por reviewer bajo
  `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`
- CSV y resumen del knowledge check: `artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`

Mantener el inventario sincronizado con el issue del tracker. Adjunta hashes al copiar artefactos al ticket de gobernanza
para que los auditores verifiquen los archivos sin acceso de shell.
