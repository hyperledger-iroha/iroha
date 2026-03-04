---
lang: ru
direction: ltr
source: docs/portal/docs/reference/README.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
заголовок: Указатель ссылок
слизень: /ссылка
---

В этом разделе перегруппируются материалы, соответствующие спецификации Iroha. Cespages restent конюшни мем lorsque lesguides ettutorielsevoluent.

## Disponible aujourd'hui

- **Открытие кодека Norito** - `reference/norito-codec.md` отсылает в соответствии со спецификацией авторитетного `norito.md` подвесного устройства, которое представляет собой переносной стол в процессе ремплиссажа.
- **Torii OpenAPI** - `/reference/torii-openapi` отображает последнюю спецификацию REST de Torii с Redoc. Обновите спецификацию через `npm run sync-openapi -- --version=current --latest` (подключите `--mirror=<label>` для копирования моментальных снимков в дополнительных исторических версиях).
- **Таблицы конфигурации** - Полный каталог параметров, найденных в `docs/source/references/configuration.md`. Если порт не предлагает автоматический импорт, ссылайтесь на документ Markdown для значений по умолчанию и дополнительных сборов за окружающую среду.
- **Версия документов** - Меню версий в панели навигации отображает снимки изображений с `npm run docs:version -- <label>`, что облегчает сравнение рекомендаций между выпусками.