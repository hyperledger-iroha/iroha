---
lang: ru
direction: ltr
source: docs/portal/docs/intro.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9f775ae297c910da91c6ce97e97ee36fb87f60218fcfb97639ace6eba39f2252
source_last_modified: "2025-11-21T17:05:19.972106+00:00"
translation_last_reviewed: 2025-12-30
---

# Добро пожаловать на портал разработчиков SORA Nexus

Портал разработчиков SORA Nexus объединяет интерактивную документацию, учебные материалы SDK и справочники API для операторов Nexus и контрибьюторов Hyperledger Iroha. Он дополняет основной сайт документации, выводя на первый план практические гайды и спецификации, генерируемые напрямую из этого репозитория. Лендинг теперь включает тематические точки входа Norito/SoraFS, подписанные снимки OpenAPI и отдельный справочник Norito Streaming, чтобы контрибьюторы могли найти контракт контрольной плоскости стриминга, не пролистывая корневую спецификацию.

## Что можно сделать здесь

- **Изучить Norito** - начните с обзора и quickstart, чтобы понять модель сериализации и инструменты bytecode.
- **Запустить SDK** - следуйте quickstart для JavaScript и Rust уже сегодня; руководства для Python, Swift и Android появятся по мере миграции рецептов.
- **Просмотреть справочники API** - страница Torii OpenAPI отображает актуальную спецификацию REST, а таблицы конфигурации ссылаются на канонические Markdown источники.
- **Подготовить развертывания** - эксплуатационные runbook-и (telemetry, settlement, Nexus overlays) переносятся из `docs/source/` и появятся здесь по мере миграции.

## Текущий статус

- ✅ Тематический лендинг Docusaurus v3 с обновленной типографикой, hero/cards на градиентах и тайлами ресурсов, включая сводку Norito Streaming.
- ✅ Плагин Torii OpenAPI подключен к `npm run sync-openapi`, с проверками подписанных снимков и CSP-guard защитами, применяемыми `buildSecurityHeaders`.
- ✅ Preview и probe coverage выполняются в CI (`docs-portal-preview.yml` + `scripts/portal-probe.mjs`), теперь они гейтят doc по streaming, quickstarts SoraFS и reference checklists перед публикацией артефактов.
- ✅ Quickstarts Norito, SoraFS и SDK плюс секции справочников уже в боковой панели; новые импорты из `docs/source/` (streaming, orchestration, runbooks) появляются здесь по мере их написания.

## Как принять участие

- См. `docs/portal/README.md` для локальных команд разработки (`npm install`, `npm run start`, `npm run build`).
- Задачи миграции контента отслеживаются вместе с пунктами roadmap `DOCS-*`. Вклад приветствуется - переносите секции из `docs/source/` и добавляйте страницу в боковую панель.
- Если вы добавляете генерируемый артефакт (specs, config tables), задокументируйте команду сборки, чтобы будущие участники могли легко его обновить.
