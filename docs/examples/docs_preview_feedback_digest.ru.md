---
lang: ru
direction: ltr
source: docs/examples/docs_preview_feedback_digest.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4b1191d3475126df594d0f9f81d91f8ebbcd002c74a1f4d4176f2f42a59ca885
source_last_modified: "2025-11-19T07:56:11.635822+00:00"
translation_last_reviewed: 2026-01-01
---

# Дайджест фидбэка по превью портала docs (шаблон)

Используйте этот шаблон при подведении итогов волны превью для governance, release
review или `status.md`. Скопируйте Markdown в тикет отслеживания, замените
плейсхолдеры реальными данными и приложите JSON-резюме, выгруженное через
`npm run --prefix docs/portal preview:log -- --summary --summary-json`. Хелпер
`preview:digest` (`npm run --prefix docs/portal preview:digest -- --wave <label>`) генерирует
секцию метрик ниже, поэтому нужно заполнить только строки highlights/actions/artefacts.

```markdown
## Дайджест фидбэка волны preview-<tag> (YYYY-MM-DD)
- Окно приглашений: <start -> end>
- Приглашенные ревьюеры: <count> (open: <count>)
- Отправки фидбэка: <count>
- Открытые issues: <count>
- Последний timestamp события: <ISO8601 from summary.json>

| Категория | Детали | Owner / Follow-up |
| --- | --- | --- |
| Highlights | <напр., "ISO builder walkthrough landed well"> | <owner + срок> |
| Блокирующие находки | <список issue IDs или ссылки трекера> | <owner> |
| Минорные правки | <сгруппировать косметические правки или copy> | <owner> |
| Аномалии телеметрии | <ссылка на snapshot дашборда / log probe> | <owner> |

## Действия
1. <Действие + ссылка + ETA>
2. <Опциональное второе действие>

## Артефакты
- Feedback log: `artifacts/docs_portal_preview/feedback_log.json` (`sha256:<digest>`)
- Резюме волны: `artifacts/docs_portal_preview/preview-<tag>-summary.json`
- Snapshot дашборда: `<link or path>`

```

Храните каждый дайджест вместе с тикетом приглашений, чтобы ревьюеры и governance могли
восстановить цепочку доказательств без просмотра логов CI.
