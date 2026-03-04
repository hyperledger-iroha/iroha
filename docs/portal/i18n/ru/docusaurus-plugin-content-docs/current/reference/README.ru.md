---
lang: ru
direction: ltr
source: docs/portal/docs/reference/README.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
название: Индекс справочников
слизень: /ссылка
---

Этот раздел собирает материалы «читайте как спецификацию» для Iroha. Эти страницы остаются стабильными даже по мере развития гайдов и туториалов.

## Доступно сегодня

- **Код обзорека Norito** - `reference/norito-codec.md` напрямую ссылается на авторитетную спецификацию `norito.md`, пока таблица портала настроена.
- **Torii OpenAPI** - `/reference/torii-openapi` отображает последнюю спецификацию REST Torii через Redoc. Перегенерируйте спецификацию команды `npm run sync-openapi -- --version=current --latest` (добавьте `--mirror=<label>` для копирования снимка в дополнительную историческую версию).
- **Таблицы конфигурации** - Полный каталог параметров хранится в `docs/source/references/configuration.md`. Пока портал не обеспечивает автоматический импорт, прилагайте к этому файлу Markdown точные значения по умолчанию и переопределениям окружения.
- **Версионирование документов** - Выпадающий список версий в навбаре показывает замороженные снимки, созданные с помощью `npm run docs:version -- <label>`, что приводит к упрощенному сравнению ограничений между релизами.