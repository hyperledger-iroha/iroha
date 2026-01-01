---
lang: ru
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
title: Сводка отзывов и статус W3 beta
sidebar_label: Сводка W3
description: Живой дайджест для beta preview-волны 2026 (finance, observability, SDK, ecosystem).
---

| Пункт | Детали |
| --- | --- |
| Волна | W3 - Beta cohorts (finance + ops + SDK partner + ecosystem advocate) |
| Окно приглашений | 2026-02-18 -> 2026-02-28 |
| Тег артефакта | `preview-20260218` |
| Issue трекера | `DOCS-SORA-Preview-W3` |
| Участники | finance-beta-01, observability-ops-02, partner-sdk-03, ecosystem-advocate-04 |

## Основные моменты

1. **End-to-end pipeline доказательств.** `npm run preview:wave -- --wave preview-20260218 --invite-start 2026-02-18 --invite-end 2026-02-28 --report-date 2026-03-01 --notes "Finance/observability beta wave"` генерирует per-wave summary (`artifacts/docs_portal_preview/preview-20260218-summary.json`), digest (`preview-20260218-digest.md`) и обновляет `docs/portal/src/data/previewFeedbackSummary.json`, чтобы reviewers по governance могли опираться на одну команду.
2. **Покрытие телеметрии + governance.** Все четыре reviewers подтвердили доступ с checksum, отправили feedback и были отозваны вовремя; digest ссылается на issues (`docs-preview/20260218` set + `DOCS-SORA-Preview-20260218`) вместе с запусками Grafana, собранными во время волны.
3. **Отображение в портале.** Обновленная таблица портала теперь показывает закрытую волну W3 с метриками latency и response-rate, а новая страница лога ниже отражает таймлайн для аудиторов, которые не вытягивают сырой JSON лог.

## Действия

| ID | Описание | Владелец | Статус |
| --- | --- | --- | --- |
| W3-A1 | Захватить preview digest и приложить к трекеру. | Docs/DevRel lead | ✅ Завершено (2026-02-28). |
| W3-A2 | Отразить evidence приглашения/digest в портале + roadmap/status. | Docs/DevRel lead | ✅ Завершено (2026-02-28). |

## Итоги выхода (2026-02-28)

- Приглашения отправлены 2026-02-18, acknowledgements зафиксированы через несколько минут; доступ preview отозван 2026-02-28 после финальной проверки телеметрии.
- Digest + summary сохранены в `artifacts/docs_portal_preview/`, а сырой лог закреплен `artifacts/docs_portal_preview/feedback_log.json` для воспроизводимости.
- Follow-ups по issues заведены в `docs-preview/20260218` с governance tracker `DOCS-SORA-Preview-20260218`; заметки CSP/Try it направлены владельцам observability/finance и связаны из digest.
- Строка трекера обновлена до 🈴 Completed, а таблица feedback портала отражает закрытие волны, завершая оставшуюся beta-ready задачу DOCS-SORA.
