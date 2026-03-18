---
lang: ru
direction: ltr
source: docs/portal/docs/reference/README.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
Название: Индекс ссылок
слизень: /ссылка
---

Это секционный материал или материал «как особый» для Iroha. Essas paginas permanecem estaveis mesmo quando guias e tutoriais evoluem.

## Disponivel hoje

- **Общее изображение кодека Norito** - `reference/norito-codec.md` отвечает непосредственно за конкретный авторизованный кодек `norito.md`, помещенный в таблицу на портале, отправленную по запросу.
- **Torii OpenAPI** - `/reference/torii-openapi` визуализирует конкретный REST, который был последним из Torii, с использованием Redoc. Перегенерируйте спецификацию `npm run sync-openapi -- --version=current --latest` (укажите `--mirror=<label>` для копирования или моментального снимка для дополнительных исторических версий).
- **Таблицы конфигурации** - Полный каталог параметров файла `docs/source/references/configuration.md`. На портале, предлагающем автоматический импорт, проконсультируйтесь с архивом Markdown для значений по умолчанию и переопределения окружающей среды.
- **Версии документов** - В раскрывающемся списке версий на панели навигации отображаются снимки замороженных криад с `npm run docs:version -- <label>`, что позволяет легко сравнивать ориентацию между выпусками.