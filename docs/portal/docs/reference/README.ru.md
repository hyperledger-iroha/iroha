---
lang: ru
direction: ltr
source: docs/portal/docs/reference/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b3b2becfdbab1446f8f230ace905de306e1e89147f5a5e578d784be97445d74d
source_last_modified: "2025-11-08T06:08:33.073497+00:00"
translation_last_reviewed: 2025-12-30
---

---
title: Индекс справочников
slug: /reference
---

Этот раздел собирает материалы «читайте как спецификацию» для Iroha. Эти страницы остаются стабильными даже по мере развития гайдов и туториалов.

## Доступно сегодня

- **Обзор кодека Norito** - `reference/norito-codec.md` напрямую ссылается на авторитетную спецификацию `norito.md`, пока таблица портала заполняется.
- **Torii OpenAPI** - `/reference/torii-openapi` отображает последнюю спецификацию REST Torii через Redoc. Перегенерируйте spec командой `npm run sync-openapi -- --version=current --latest` (добавьте `--mirror=<label>` для копирования snapshot в дополнительные исторические версии).
- **Torii MCP API** - `/reference/torii-mcp` documents MCP JSON-RPC usage (`initialize`, `tools/list`, `tools/call`) and async job polling for `/v1/mcp`.
- **Таблицы конфигурации** - Полный каталог параметров хранится в `docs/source/references/configuration.md`. Пока портал не предоставляет auto-import, обращайтесь к этому Markdown файлу за точными значениями по умолчанию и переопределениями окружения.
- **Версионирование docs** - Выпадающий список версий в навбаре показывает замороженные snapshots, созданные с помощью `npm run docs:version -- <label>`, что упрощает сравнение рекомендаций между релизами.
