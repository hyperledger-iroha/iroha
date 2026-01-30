---
lang: ru
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/reference/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e1f2fc637a1fc283e3079ffc22ddff70c6eb1e568a21b951b61491529052c234
source_last_modified: "2025-11-04T12:24:28.218431+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: Индекс справочников
slug: /reference
---

Этот раздел собирает материалы «читайте как спецификацию» для Iroha. Эти страницы остаются стабильными даже по мере развития гайдов и туториалов.

## Доступно сегодня

- **Обзор кодека Norito** - `reference/norito-codec.md` напрямую ссылается на авторитетную спецификацию `norito.md`, пока таблица портала заполняется.
- **Torii OpenAPI** - `/reference/torii-openapi` отображает последнюю спецификацию REST Torii через Redoc. Перегенерируйте spec командой `npm run sync-openapi -- --version=current --latest` (добавьте `--mirror=<label>` для копирования snapshot в дополнительные исторические версии).
- **Таблицы конфигурации** - Полный каталог параметров хранится в `docs/source/references/configuration.md`. Пока портал не предоставляет auto-import, обращайтесь к этому Markdown файлу за точными значениями по умолчанию и переопределениями окружения.
- **Версионирование docs** - Выпадающий список версий в навбаре показывает замороженные snapshots, созданные с помощью `npm run docs:version -- <label>`, что упрощает сравнение рекомендаций между релизами.
