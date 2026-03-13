---
lang: es
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/log.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: vista previa-comentarios-w1-log
título: Registro de comentarios y telemetría W1
sidebar_label: Registro W1
Descripción: Lista agregada, puntos de control de telemetría y notas de revisores para la primera onda de vista previa de paquetes.
---

Este registro mantiene la lista de invitados, puntos de control de telemetría y comentarios de los revisores para o
**vista previa de parceiros W1** que acompañan como tarefas de aceitacao em
[`preview-feedback/w1/plan.md`](./plan.md) y la entrada del rastreador de onda en
[`../../preview-invite-tracker.md`](../../preview-invite-tracker.md). Actualizar cuando un convite para enviado,
una instantánea de telemetría para el registro o un elemento de retroalimentación para el triado para que los revisores de gobierno puedan
reproducir como evidencias sin buscar entradas externas.

## Lista de la corte| ID de socio | Boleto de solicitud | NDA recibido | Convite enviado (UTC) | Confirmar/primero inicio de sesión (UTC) | Estado | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| socio-w1-01 | `DOCS-SORA-Preview-REQ-P01` | OK 2025-04-03 | 2025-04-12 15:00 | 2025-04-12 15:11 | Concluido 2025-04-26 | sorafs-op-01; Focado em evidencia de paridade dos docs do Orchestrator. |
| socio-w1-02 | `DOCS-SORA-Preview-REQ-P02` | OK 2025-04-03 | 2025-04-12 15:03 | 2025-04-12 15:15 | Concluido 2025-04-26 | sorafs-op-02; enlaces cruzados válidos Norito/telemetria. |
| socio-w1-03 | `DOCS-SORA-Preview-REQ-P03` | OK 2025-04-04 | 2025-04-12 15:06 | 2025-04-12 15:18 | Concluido 2025-04-26 | sorafs-op-03; Ejecute simulacros de conmutación por error de múltiples fuentes. |
| socio-w1-04 | `DOCS-SORA-Preview-REQ-P04` | OK 2025-04-04 | 2025-04-12 15:09 | 2025-04-12 15:21 | Concluido 2025-04-26 | torii-int-01; revisao do cookbook Torii `/v2/pipeline` + Pruébalo. |
| socio-w1-05 | `DOCS-SORA-Preview-REQ-P05` | OK 2025-04-05 | 2025-04-12 15:12 | 2025-04-12 15:23 | Concluido 2025-04-26 | torii-int-02; Acompaña una actualización de la captura de pantalla Pruébalo (docs-preview/w1 #2). |
| socio-w1-06 | `DOCS-SORA-Preview-REQ-P06` | OK 2025-04-05 | 2025-04-12 15:15 | 2025-04-12 15:26 | Concluido 2025-04-26 | socio-sdk-01; Comentarios de libros de cocina JS/Swift + controles de cordura hacen puente ISO. || socio-w1-07 | `DOCS-SORA-Preview-REQ-P07` | OK 2025-04-11 | 2025-04-12 15:18 | 2025-04-12 15:29 | Concluido 2025-04-26 | socio-sdk-02; cumplimiento aprobado 2025-04-11, focado em notas de Connect/telemetria. |
| socio-w1-08 | `DOCS-SORA-Preview-REQ-P08` | OK 2025-04-11 | 2025-04-12 15:21 | 2025-04-12 15:33 | Concluido 2025-04-26 | puerta de enlace-ops-01; Audita la guía de operaciones de la puerta de enlace + flujo anónimo del proxy Pruébalo. |

Preencha las marcas de tiempo de **Convite enviado** y **Ack** así como el correo electrónico de dicha emisión.
Ancore os horarios no cronograma UTC definido no plano W1.

## Puntos de control de telemetria

| Marca de tiempo (UTC) | Cuadros de mandos / sondas | Responsavel | Resultado | Artefacto |
| --- | --- | --- | --- | --- |
| 2025-04-06 18:05 | `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` | Documentos/DevRel + Operaciones | Tudo verde | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250406` |
| 2025-04-06 18:20 | Transcripción de `npm run manage:tryit-proxy -- --stage preview-w1` | Operaciones | En escena | `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log` |
| 2025-04-12 14:45 | Cuadros acima + `probe:portal` | Documentos/DevRel + Operaciones | Preinvitación instantánea, sin regreso | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250412` |
| 2025-04-19 17:55 | Paneles acima + diferencia de latencia del proxy Pruébalo | Líder de Docs/DevRel | Checkpoint de meio aprobado (0 alertas; latencia Pruébalo p95=410 ms) | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250419` |
| 2025-04-26 16:25 | Cuadros acima + sonda de saya | Docs/DevRel + enlace de gobernanza | Instantánea de Saida, cero alertas pendientes | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250426` |Como amostras diarios de horario de oficina (2025-04-13 -> 2025-04-25) estao agrupadas como exportaciones NDJSON + PNG em
`artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/` con nombres de archivo
`docs-preview-integrity-<date>.json` y capturas de pantalla correspondientes.

## Registro de comentarios y problemas

Utilice esta tabla para resumir los artículos enviados por los revisores. Vincule cada entrada al ticket GitHub/discuss
mais o formulario estructurado capturado via
[`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md).

| Referencia | Severidad | Responsavel | Estado | Notas |
| --- | --- | --- | --- | --- |
| `docs-preview/w1 #1` | Bajo | Documentos-core-02 | Resuelto 2025-04-18 | Esclareceu o wording de nav do Try it + ancora de sidebar (`docs/source/sorafs/tryit.md` actualizado con nueva etiqueta). |
| `docs-preview/w1 #2` | Bajo | Documentos-core-03 | Resuelto 2025-04-19 | Captura de pantalla Pruébalo + leyenda atualizados conforme pedido; artefacto `artifacts/docs_preview/W1/preview-2025-04-12/feedback/partner-w1-05/screenshot-diff.png`. |
| - | Información | Líder de Docs/DevRel | Fechado | Comentarios restantes para preguntas y respuestas; capturados no formulario de feedback de cada parceiro sob `artifacts/docs_preview/W1/preview-2025-04-12/feedback/`. |

## Acompañamiento de verificación de conocimientos y encuestas.

1. Regístrese como notas del cuestionario (meta >=90%) para cada revisor; anexe o CSV exportado al lado de dos artefatos de convite.
2. Colete as respuestas cualitativas de la encuesta capturadas sin plantilla de retroalimentación y espelhe em
   `artifacts/docs_preview/W1/preview-2025-04-12/surveys/`.
3. Agend chamadas de remediation para quem estiver abaixo do limite e registre aqui.Todos los revisores de oito marcaram >=94% sin verificación de conocimientos (CSV:
`artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`). Nenhuma chamada de remediación
foi necesaria; exports de Survey para cada parceiro vivem em
`artifacts/docs_preview/W1/preview-2025-04-12/surveys/<partner-id>/summary.json`.

## Inventario de artefatos

- Descriptor de vista previa del paquete/suma de comprobación: `artifacts/docs_preview/W1/preview-2025-04-12/descriptor.json`
- Resumen de sonda + verificación de enlace: `artifacts/docs_preview/W1/preview-2025-04-12/preflight-summary.json`
- Registro de mudanca del proxy Pruébalo: `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log`
- Exportaciones de telemetria: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/<date>/`
- Paquete diario de telemetria de horario de oficina: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/`
- Exportaciones de comentarios + encuesta: colocar pastas por revisor en
  `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`
- CSV y resumen de la verificación de conocimientos: `artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`

Mantenha o inventario sincronizado con o issues do tracker. Anexe hashes para copiar artefatos para el ticket de gobierno
para que los auditores verifiquen los archivos sin acceso a shell.