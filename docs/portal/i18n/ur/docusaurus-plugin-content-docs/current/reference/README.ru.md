---
lang: ur
direction: rtl
source: docs/portal/docs/reference/README.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
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
