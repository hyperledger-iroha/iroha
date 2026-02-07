---
lang: es
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/log.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: vista previa-comentarios-w1-log
título: Лог отзывов и телеметрии W1
sidebar_label: Registro W1
descripción: Lista completa, puntos de control telemétricos y revisores seleccionados para todos los socios de vista previa.
---

Este registro de listas de registros, puntos de control telemétricos y revisores externos
**vista previa de socios W1**, сопровождающей задачи приема в
[`preview-feedback/w1/plan.md`](./plan.md) y запись трекера волны в
[`../../preview-invite-tracker.md`](../../preview-invite-tracker.md). Обновляйте его, когда отправлено приглашение,
instantánea telemétrica completa y clasificación de puntos de referencia, muchos revisores de gobernanza
воспроизвести доказательства без поиска внешних тикетов.

## Рoster когорты| ID de socio | Тикет запроса | NDA получено | Приглашение отправлено (UTC) | Confirmar/iniciar sesión (UTC) | Estado | Примечания |
| --- | --- | --- | --- | --- | --- | --- |
| socio-w1-01 | `DOCS-SORA-Preview-REQ-P01` | ✅ 2025-04-03 | 2025-04-12 15:00 | 2025-04-12 15:11 | ✅ Завершено 2025-04-26 | sorafs-op-01; сфокусирован на доказательствах parity для Orchestrator docs. |
| socio-w1-02 | `DOCS-SORA-Preview-REQ-P02` | ✅ 2025-04-03 | 2025-04-12 15:03 | 2025-04-12 15:15 | ✅ Завершено 2025-04-26 | sorafs-op-02; проверил enlaces cruzados Norito/telemetría. |
| socio-w1-03 | `DOCS-SORA-Preview-REQ-P03` | ✅ 2025-04-04 | 2025-04-12 15:06 | 2025-04-12 15:18 | ✅ Завершено 2025-04-26 | sorafs-op-03; провел simulacros de conmutación por error de múltiples fuentes. |
| socio-w1-04 | `DOCS-SORA-Preview-REQ-P04` | ✅ 2025-04-04 | 2025-04-12 15:09 | 2025-04-12 15:21 | ✅ Завершено 2025-04-26 | torii-int-01; ревью libro de cocina Torii `/v1/pipeline` + Pruébalo. |
| socio-w1-05 | `DOCS-SORA-Preview-REQ-P05` | ✅ 2025-04-05 | 2025-04-12 15:12 | 2025-04-12 15:23 | ✅ Завершено 2025-04-26 | torii-int-02; участвовал в обновлении скриншота Pruébelo (docs-preview/w1 #2). |
| socio-w1-06 | `DOCS-SORA-Preview-REQ-P06` | ✅ 2025-04-05 | 2025-04-12 15:15 | 2025-04-12 15:26 | ✅ Завершено 2025-04-26 | socio-sdk-01; Comentarios sobre el libro de cocina JS/Swift + controles de cordura para el puente ISO. || socio-w1-07 | `DOCS-SORA-Preview-REQ-P07` | ✅ 2025-04-11 | 2025-04-12 15:18 | 2025-04-12 15:29 | ✅ Завершено 2025-04-26 | socio-sdk-02; cumplimiento закрыт 2025-04-11, фокус на заметках Connect/telemetry. |
| socio-w1-08 | `DOCS-SORA-Preview-REQ-P08` | ✅ 2025-04-11 | 2025-04-12 15:21 | 2025-04-12 15:33 | ✅ Завершено 2025-04-26 | puerta de enlace-ops-01; аудит ops гайда gateway + анонимизированный поток Pruébalo proxy. |

Utilice **Ack** y **Ack** para después de la operación.
Siga la conexión UTC rápidamente en el plano W1.

## Puntos de control telemétricos

| Время (UTC) | Cuadros de mandos / sondas | Владелец | Respuesta | Artefacto |
| --- | --- | --- | --- | --- |
| 2025-04-06 18:05 | `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` | Documentos/DevRel + Operaciones | ✅ Все зеленое | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250406` |
| 2025-04-06 18:20 | Transcripción `npm run manage:tryit-proxy -- --stage preview-w1` | Operaciones | ✅ Подготовлено | `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log` |
| 2025-04-12 14:45 | Дашборды выше + `probe:portal` | Documentos/DevRel + Operaciones | ✅ Instantánea previa a la invitación, без регрессий | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250412` |
| 2025-04-19 17:55 | Дашборды выше + diff по латентности Pruébalo proxy | Líder de Docs/DevRel | ✅ Verificación del punto medio прошел (0 алертов; латентность Pruébelo p95=410 ms) | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250419` |
| 2025-04-26 16:25 | Дашборды выше + sonda de salida | Docs/DevRel + enlace de gobernanza | ✅ Salir de la instantánea, нет активных алертов | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250426` |Ежедневные выборки horario de oficina (2025-04-13 -> 2025-04-25) упакованы как NDJSON + PNG эксportы под
`artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/` con archivos de iconos
`docs-preview-integrity-<date>.json` y соответствующими скриншотами.

## Лог отзывов и problemas

Utilice esta tabla para resumir los resultados de los revisores. Usar un elemento de archivo en GitHub/discuss
тикет и на структурированную форму, заполненную через
[`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md).

| Referencia | Gravedad | Propietario | Estado | Notas |
| --- | --- | --- | --- | --- |
| `docs-preview/w1 #1` | Bajo | Documentos-core-02 | ✅ Resuelto 2025-04-18 | Уточнены формулировка навигации Pruébelo + якорь barra lateral (`docs/source/sorafs/tryit.md` обновлен новым etiqueta). |
| `docs-preview/w1 #2` | Bajo | Documentos-core-03 | ✅ Resuelto 2025-04-19 | Обновлены скриншот Pruébelo y подпись; artefacto `artifacts/docs_preview/W1/preview-2025-04-12/feedback/partner-w1-05/screenshot-diff.png`. |
| - | Información | Líder de Docs/DevRel | 🟢 Cerrado | Остальные комментарии были только Q&A; зафиксированы в форме каждого партнера под `artifacts/docs_preview/W1/preview-2025-04-12/feedback/`. |

## Verificación de conocimientos y encuestas.

1. Запишите результаты quiz (цель >=90%) для каждого revisor; Registre un archivo CSV con archivos adjuntos.
2. Соберите качественные ответы encuesta, записанные в форме comentarios y сохраните их под
   `artifacts/docs_preview/W1/preview-2025-04-12/surveys/`.
3. Planifique las acciones de remediación para estos, no por el momento y elimine este archivo.Muchos revisores calificaron >=94% en verificación de conocimientos (CSV:
`artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`). remediación звонки не потребовались;
encuesta de exportaciones для каждого партнера находятся под
`artifacts/docs_preview/W1/preview-2025-04-12/surveys/<partner-id>/summary.json`.

## Инвентаризация артефактов

- Descriptor de vista previa del paquete/suma de comprobación: `artifacts/docs_preview/W1/preview-2025-04-12/descriptor.json`
- Sonda resumen + verificación de enlace: `artifacts/docs_preview/W1/preview-2025-04-12/preflight-summary.json`
- Лог изменений Pruébalo proxy: `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log`
- Exportaciones de telemetría: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/<date>/`
- Paquete de telemetría diaria en horario de oficina: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/`
- Comentarios + exportaciones de encuestas: размещать папки por revisor под
  `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`
- Verificación de conocimientos CSV y resumen: `artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`

Es necesario sincronizar el tema con el tema. При копировании артефактов в тикет Governance
прикладывайте хэши, чтобы аудиторы могли проверить файлы без доступа shell.