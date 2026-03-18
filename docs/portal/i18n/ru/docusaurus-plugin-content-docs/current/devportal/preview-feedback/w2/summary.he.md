---
lang: he
direction: rtl
source: docs/portal/i18n/ru/docusaurus-plugin-content-docs/current/devportal/preview-feedback/w2/summary.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 776346a77269ef28ccc5b57286c4d281dcde895d096796631dc4e58b3d7c0745
source_last_modified: "2026-01-03T18:08:02+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: preview-feedback-w2-summary
lang: ru
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w2/summary.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

| Пункт | Детали |
| --- | --- |
| Волна | W2 - Community reviewers |
| Окно приглашений | 2025-06-15 -> 2025-06-29 |
| Тег артефакта | `preview-2025-06-15` |
| Issue трекера | `DOCS-SORA-Preview-W2` |
| Участники | comm-vol-01...comm-vol-08 |

## Основные моменты

1. **Управление и инструменты** - Политика community intake единогласно одобрена 2025-05-20; обновленный шаблон запроса с полями мотивации/часового пояса лежит в `docs/examples/docs_preview_request_template.md`.
2. **Доказательства preflight** - Изменение Try it proxy `OPS-TRYIT-188` выполнено 2025-06-09, сняты дашборды Grafana, а outputs descriptor/checksum/probe для `preview-2025-06-15` архивированы в `artifacts/docs_preview/W2/`.
3. **Волна приглашений** - Восемь community reviewers приглашены 2025-06-15, acknowledgements записаны в таблицу приглашений трекера; все завершили проверку checksum перед просмотром.
4. **Обратная связь** - `docs-preview/w2 #1` (текст tooltip) и `#2` (порядок sidebar локализации) заведены 2025-06-18 и закрыты к 2025-06-21 (Docs-core-04/05); инцидентов во время волны не было.

## Действия

| ID | Описание | Владелец | Статус |
| --- | --- | --- | --- |
| W2-A1 | Закрыть `docs-preview/w2 #1` (текст tooltip). | Docs-core-04 | ✅ Завершено (2025-06-21). |
| W2-A2 | Закрыть `docs-preview/w2 #2` (sidebar локализации). | Docs-core-05 | ✅ Завершено (2025-06-21). |
| W2-A3 | Архивировать доказательства выхода + обновить roadmap/status. | Docs/DevRel lead | ✅ Завершено (2025-06-29). |

## Итоги выхода (2025-06-29)

- Все восемь community reviewers подтвердили завершение, доступ к preview отозван; acknowledgements записаны в журнал приглашений трекера.
- Финальные телеметрические снимки (`docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`) оставались зелеными; логи и transcripts Try it proxy приложены к `DOCS-SORA-Preview-W2`.
- Пакет доказательств (descriptor, checksum log, probe output, link report, Grafana screenshots, invite acknowledgements) архивирован в `artifacts/docs_preview/W2/preview-2025-06-15/`.
- Журнал checkpoint W2 в трекере обновлен до выхода, чтобы roadmap сохранял проверяемую историю перед началом планирования W3.
