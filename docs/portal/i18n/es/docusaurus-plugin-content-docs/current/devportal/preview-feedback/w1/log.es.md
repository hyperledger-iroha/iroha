---
lang: es
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/log.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: vista previa-comentarios-w1-log
título: Registro de comentarios y telemetría W1
sidebar_label: Registro W1
descripción: Lista agregada, puntos de control de telemetría y notas de revisores para la primera ola de vista previa de socios.
---

Este registro mantiene la lista de invitaciones, puntos de control de telemetría y comentarios de revisores para el
**vista previa de partners W1** que acompaña las tareas de aceptación en
[`preview-feedback/w1/plan.md`](./plan.md) y la entrada del rastreador de la ola en
[`../../preview-invite-tracker.md`](../../preview-invite-tracker.md). Actualizalo cuando se envie una invitación,
se registre una instantánea de telemetría o se clasifique un elemento de retroalimentación para que los revisores de gobernanza puedan reproducir
la evidencia sin perseguir entradas externas.

## Lista de cohorte| ID de socio | Boleto de solicitud | NDA recibida | Invitación enviada (UTC) | Acuse de recibo/primer inicio de sesión (UTC) | Estado | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| socio-w1-01 | `DOCS-SORA-Preview-REQ-P01` | OK 2025-04-03 | 2025-04-12 15:00 | 2025-04-12 15:11 | Completado 2025-04-26 | sorafs-op-01; enfocado en evidencia de paridad de documentos del orquestador. |
| socio-w1-02 | `DOCS-SORA-Preview-REQ-P02` | OK 2025-04-03 | 2025-04-12 15:03 | 2025-04-12 15:15 | Completado 2025-04-26 | sorafs-op-02; valido cross-links de Norito/telemetria. |
| socio-w1-03 | `DOCS-SORA-Preview-REQ-P03` | OK 2025-04-04 | 2025-04-12 15:06 | 2025-04-12 15:18 | Completado 2025-04-26 | sorafs-op-03; ejecuto simulacros de conmutación por error de múltiples fuentes. |
| socio-w1-04 | `DOCS-SORA-Preview-REQ-P04` | OK 2025-04-04 | 2025-04-12 15:09 | 2025-04-12 15:21 | Completado 2025-04-26 | torii-int-01; revisión del libro de cocina de Torii `/v1/pipeline` + Pruébalo. |
| socio-w1-05 | `DOCS-SORA-Preview-REQ-P05` | OK 2025-04-05 | 2025-04-12 15:12 | 2025-04-12 15:23 | Completado 2025-04-26 | torii-int-02; Acompañamos la actualización de captura de pantalla de Pruébelo (docs-preview/w1 #2). |
| socio-w1-06 | `DOCS-SORA-Preview-REQ-P06` | OK 2025-04-05 | 2025-04-12 15:15 | 2025-04-12 15:26 | Completado 2025-04-26 | socio-sdk-01; feedback de libros de cocina JS/Swift + controles de cordura del puente ISO. || socio-w1-07 | `DOCS-SORA-Preview-REQ-P07` | OK 2025-04-11 | 2025-04-12 15:18 | 2025-04-12 15:29 | Completado 2025-04-26 | socio-sdk-02; cumplimiento aprobado 2025-04-11, enfocado en notas de Connect/telemetria. |
| socio-w1-08 | `DOCS-SORA-Preview-REQ-P08` | OK 2025-04-11 | 2025-04-12 15:21 | 2025-04-12 15:33 | Completado 2025-04-26 | puerta de enlace-ops-01; audite la guía de operaciones del gateway + flujo anónimo del proxy Pruébelo. |

Completa las marcas de tiempo de **Invitación enviada** y **Ack** apenas se emite el correo electrónico saliente.
Ancla los tiempos al calendario UTC definidos en el plan W1.

## Puntos de control de telemetria

| Marca de tiempo (UTC) | Cuadros de mandos / sondas | Responsable | Resultado | Artefacto |
| --- | --- | --- | --- | --- |
| 2025-04-06 18:05 | `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` | Documentos/DevRel + Operaciones | Todo en verde | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250406` |
| 2025-04-06 18:20 | Transcripción de `npm run manage:tryit-proxy -- --stage preview-w1` | Operaciones | Preparado | `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log` |
| 2025-04-12 14:45 | Paneles de arriba + `probe:portal` | Documentos/DevRel + Operaciones | Pre-invitación instantánea, sin regresiones | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250412` |
| 2025-04-19 17:55 | Paneles de arriba + diferencia de latencia del proxy Pruébalo | Líder de Docs/DevRel | Chequeo de mitad de ola ok (0 alertas; latencia Pruébalo p95=410 ms) | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250419` |
| 2025-04-26 16:25 | Cuadros de arriba + sonda de salida | Docs/DevRel + enlace de gobernanza | Snapshot de salida, cero alertas pendientes | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250426` |Las muestras diarias de office hours (2025-04-13 -> 2025-04-25) se agrupan como exportes NDJSON + PNG bajo
`artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/` con nombres de archivo
`docs-preview-integrity-<date>.json` y las capturas de pantalla correspondientes.

## Registro de comentarios y problemas

Usa esta tabla para resumir los hallazgos enviados por los revisores. Enlaza cada entrada al ticket de GitHub/discuss
mas el formulario estructurado capturado vía
[`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md).

| Referencia | Severidad | Responsable | Estado | Notas |
| --- | --- | --- | --- | --- |
| `docs-preview/w1 #1` | Bajo | Documentos-core-02 | Resultado 2025-04-18 | Se aclaro el texto de navegación de Try it + ancla de sidebar (`docs/source/sorafs/tryit.md` actualizado con nueva etiqueta). |
| `docs-preview/w1 #2` | Bajo | Documentos-core-03 | Resultado 2025-04-19 | Se actualiza captura de pantalla de Pruébalo + título según pedido del revisor; artefacto `artifacts/docs_preview/W1/preview-2025-04-12/feedback/partner-w1-05/screenshot-diff.png`. |
| - | Información | Líder de Docs/DevRel | Cerrado | Los comentarios restantes fueron solo Q&A; capturados en el formulario de feedback de cada socio bajo `artifacts/docs_preview/W1/preview-2025-04-12/feedback/`. |

## Seguimiento de verificación de conocimientos y encuestas1. Registra los puntajes del cuestionario (objetivo >=90%) para cada revisor; adjunte el CSV exportado junto a los artefactos de invitación.
2. Recolecta las respuestas cualitativas de la encuesta capturadas con el template de feedback y reflejalas bajo
   `artifacts/docs_preview/W1/preview-2025-04-12/surveys/`.
3. Agenda llamadas de remediacion para cualquiera por debajo del umbral y registralas en este archivo.

Los ocho revisores marcaron >=94% en el control de conocimientos (CSV:
`artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`). No se requirieron llamadas de remediación;
los exports de Survey para cada socio viven bajo
`artifacts/docs_preview/W1/preview-2025-04-12/surveys/<partner-id>/summary.json`.

## Inventario de artefactos

- Paquete de descriptor/suma de verificación de vista previa: `artifacts/docs_preview/W1/preview-2025-04-12/descriptor.json`
- Resumen de sonda + link-check: `artifacts/docs_preview/W1/preview-2025-04-12/preflight-summary.json`
- Registro de cambio del proxy Pruébalo: `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log`
- Exportaciones de telemetría: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/<date>/`
- Paquete diario de telemetria de horario de oficina: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/`
- Exportaciones de comentarios + encuestas: colocar carpetas por revisor bajo
  `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`
- CSV y resumen del control de conocimientos: `artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`

Mantener el inventario sincronizado con la emisión del rastreador. Adjunta hashes al copiar artefactos al ticket de gobernanza
para que los auditores verifiquen los archivos sin acceso de shell.