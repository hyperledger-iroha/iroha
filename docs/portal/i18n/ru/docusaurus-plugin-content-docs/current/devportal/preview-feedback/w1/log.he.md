---
lang: he
direction: rtl
source: docs/portal/i18n/ru/docusaurus-plugin-content-docs/current/devportal/preview-feedback/w1/log.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d80e16d13aa80fb0d3a97a5ac001f5ed4c164e003db08f9a9172be2c37350ee8
source_last_modified: "2026-01-03T18:08:02+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: preview-feedback-w1-log
lang: ru
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/log.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

Этот лог хранит roster приглашений, телеметрические checkpoints и отзывы reviewers для
**preview партнеров W1**, сопровождающей задачи приема в
[`preview-feedback/w1/plan.md`](./plan.md) и запись трекера волны в
[`../../preview-invite-tracker.md`](../../preview-invite-tracker.md). Обновляйте его, когда отправлено приглашение,
записан телеметрический snapshot или triage выполнен по пункту отзывов, чтобы governance reviewers могли
воспроизвести доказательства без поиска внешних тикетов.

## Рoster когорты

| Partner ID | Тикет запроса | NDA получено | Приглашение отправлено (UTC) | Ack/первый логин (UTC) | Статус | Примечания |
| --- | --- | --- | --- | --- | --- | --- |
| partner-w1-01 | `DOCS-SORA-Preview-REQ-P01` | ✅ 2025-04-03 | 2025-04-12 15:00 | 2025-04-12 15:11 | ✅ Завершено 2025-04-26 | sorafs-op-01; сфокусирован на доказательствах parity для orchestrator docs. |
| partner-w1-02 | `DOCS-SORA-Preview-REQ-P02` | ✅ 2025-04-03 | 2025-04-12 15:03 | 2025-04-12 15:15 | ✅ Завершено 2025-04-26 | sorafs-op-02; проверил cross-links Norito/telemetry. |
| partner-w1-03 | `DOCS-SORA-Preview-REQ-P03` | ✅ 2025-04-04 | 2025-04-12 15:06 | 2025-04-12 15:18 | ✅ Завершено 2025-04-26 | sorafs-op-03; провел multi-source failover drills. |
| partner-w1-04 | `DOCS-SORA-Preview-REQ-P04` | ✅ 2025-04-04 | 2025-04-12 15:09 | 2025-04-12 15:21 | ✅ Завершено 2025-04-26 | torii-int-01; ревью cookbook Torii `/v1/pipeline` + Try it. |
| partner-w1-05 | `DOCS-SORA-Preview-REQ-P05` | ✅ 2025-04-05 | 2025-04-12 15:12 | 2025-04-12 15:23 | ✅ Завершено 2025-04-26 | torii-int-02; участвовал в обновлении скриншота Try it (docs-preview/w1 #2). |
| partner-w1-06 | `DOCS-SORA-Preview-REQ-P06` | ✅ 2025-04-05 | 2025-04-12 15:15 | 2025-04-12 15:26 | ✅ Завершено 2025-04-26 | sdk-partner-01; feedback по cookbook JS/Swift + sanity checks для ISO bridge. |
| partner-w1-07 | `DOCS-SORA-Preview-REQ-P07` | ✅ 2025-04-11 | 2025-04-12 15:18 | 2025-04-12 15:29 | ✅ Завершено 2025-04-26 | sdk-partner-02; compliance закрыт 2025-04-11, фокус на заметках Connect/telemetry. |
| partner-w1-08 | `DOCS-SORA-Preview-REQ-P08` | ✅ 2025-04-11 | 2025-04-12 15:21 | 2025-04-12 15:33 | ✅ Завершено 2025-04-26 | gateway-ops-01; аудит ops гайда gateway + анонимизированный поток Try it proxy. |

Заполните **Приглашение отправлено** и **Ack** сразу после отправки письма.
Привяжите время к UTC расписанию, заданному в плане W1.

## Телеметрические checkpoints

| Время (UTC) | Dashboards / probes | Владелец | Результат | Артефакт |
| --- | --- | --- | --- | --- |
| 2025-04-06 18:05 | `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` | Docs/DevRel + Ops | ✅ Все зеленое | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250406` |
| 2025-04-06 18:20 | Транскрипт `npm run manage:tryit-proxy -- --stage preview-w1` | Ops | ✅ Подготовлено | `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log` |
| 2025-04-12 14:45 | Дашборды выше + `probe:portal` | Docs/DevRel + Ops | ✅ Pre-invite snapshot, без регрессий | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250412` |
| 2025-04-19 17:55 | Дашборды выше + diff по латентности Try it proxy | Docs/DevRel lead | ✅ Midpoint check прошел (0 алертов; латентность Try it p95=410 ms) | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250419` |
| 2025-04-26 16:25 | Дашборды выше + exit probe | Docs/DevRel + Governance liaison | ✅ Exit snapshot, нет активных алертов | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250426` |

Ежедневные выборки office hours (2025-04-13 -> 2025-04-25) упакованы как NDJSON + PNG экспорты под
`artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/` с именами файлов
`docs-preview-integrity-<date>.json` и соответствующими скриншотами.

## Лог отзывов и issues

Используйте эту таблицу для суммирования результатов reviewers. Ссылайтесь на каждый элемент на GitHub/discuss
тикет и на структурированную форму, заполненную через
[`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md).

| Reference | Severity | Owner | Status | Notes |
| --- | --- | --- | --- | --- |
| `docs-preview/w1 #1` | Low | Docs-core-02 | ✅ Resolved 2025-04-18 | Уточнены формулировка навигации Try it + якорь sidebar (`docs/source/sorafs/tryit.md` обновлен новым label). |
| `docs-preview/w1 #2` | Low | Docs-core-03 | ✅ Resolved 2025-04-19 | Обновлены скриншот Try it и подпись; artefact `artifacts/docs_preview/W1/preview-2025-04-12/feedback/partner-w1-05/screenshot-diff.png`. |
| - | Info | Docs/DevRel lead | 🟢 Closed | Остальные комментарии были только Q&A; зафиксированы в форме каждого партнера под `artifacts/docs_preview/W1/preview-2025-04-12/feedback/`. |

## Knowledge check и surveys

1. Запишите результаты quiz (цель >=90%) для каждого reviewer; прикрепите экспорт CSV рядом с артефактами приглашений.
2. Соберите качественные ответы survey, записанные в форме feedback, и сохраните их под
   `artifacts/docs_preview/W1/preview-2025-04-12/surveys/`.
3. Запланируйте remediation звонки для тех, кто ниже порога, и отметьте их в этом файле.

Все восемь reviewers набрали >=94% в knowledge check (CSV:
`artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`). remediation звонки не потребовались;
exports survey для каждого партнера находятся под
`artifacts/docs_preview/W1/preview-2025-04-12/surveys/<partner-id>/summary.json`.

## Инвентаризация артефактов

- Bundle preview descriptor/checksum: `artifacts/docs_preview/W1/preview-2025-04-12/descriptor.json`
- Summary probe + link-check: `artifacts/docs_preview/W1/preview-2025-04-12/preflight-summary.json`
- Лог изменений Try it proxy: `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log`
- Telemetry exports: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/<date>/`
- Daily office-hour telemetry bundle: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/`
- Feedback + survey exports: размещать папки per reviewer под
  `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`
- Knowledge check CSV и summary: `artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`

Держите инвентарь синхронизированным с issue трекера. При копировании артефактов в тикет governance
прикладывайте хэши, чтобы аудиторы могли проверить файлы без shell-доступа.
