---
lang: he
direction: rtl
source: docs/portal/i18n/ru/docusaurus-plugin-content-docs/current/reference/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b12fcba9806f4466e3d7abbdd0a7ea53c3a69a2f6a8a96396756b2cf7f23e6cb
source_last_modified: "2025-11-14T04:43:20.931883+00:00"
translation_last_reviewed: 2026-01-30
---

Этот раздел собирает материалы «читайте как спецификацию» для Iroha. Эти страницы остаются стабильными даже по мере развития гайдов и туториалов.

## Доступно сегодня

- **Обзор кодека Norito** - `reference/norito-codec.md` напрямую ссылается на авторитетную спецификацию `norito.md`, пока таблица портала заполняется.
- **Torii OpenAPI** - `/reference/torii-openapi` отображает последнюю спецификацию REST Torii через Redoc. Перегенерируйте spec командой `npm run sync-openapi -- --version=current --latest` (добавьте `--mirror=<label>` для копирования snapshot в дополнительные исторические версии).
- **Таблицы конфигурации** - Полный каталог параметров хранится в `docs/source/references/configuration.md`. Пока портал не предоставляет auto-import, обращайтесь к этому Markdown файлу за точными значениями по умолчанию и переопределениями окружения.
- **Версионирование docs** - Выпадающий список версий в навбаре показывает замороженные snapshots, созданные с помощью `npm run docs:version -- <label>`, что упрощает сравнение рекомендаций между релизами.
