---
lang: ja
direction: ltr
source: docs/portal/i18n/ru/docusaurus-plugin-content-docs/current/reference/publishing-checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0b16d3564739b7026f08961efa9c736fd1d3b169fbdf95944109380136487806
source_last_modified: "2026-01-03T18:08:01+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: ru
direction: ltr
source: docs/portal/docs/reference/publishing-checklist.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
title: Чек‑лист публикации портала
description: Шаги проверки перед обновлением портала документации Iroha.
---

Используйте этот чек‑лист при каждом обновлении портала разработчика. Он гарантирует, что
CI‑сборка, деплой GitHub Pages и ручные smoke‑тесты покрывают все разделы перед релизом
или вехой roadmap’а.

## 1. Локальная проверка

- `npm run sync-openapi -- --version=current --latest` (добавьте один или несколько
  флагов `--mirror=<label>`, если Torii OpenAPI меняется для замороженного snapshot’а).
- `npm run build` — убедитесь, что слоган “Build on Iroha with confidence” всё ещё
  отображается в `build/index.html`.
- `./docs/portal/scripts/preview_verify.sh --build-dir build` — проверьте manifest
  checksum’ов (добавьте `--descriptor`/`--archive` при тестировании скачанных CI‑артефактов).
- `npm run serve` — запускает helper‑режим с проверкой checksum’ов, который валидирует
  manifest перед вызовом `docusaurus serve`, чтобы ревьюеры никогда не смотрели на
  неподписанный snapshot (alias `serve:verified` остаётся для явных вызовов).
- Просмотрите изменённые markdown‑файлы через `npm run start` и dev‑сервер с live‑reload.

## 2. Проверки pull‑request’а

- Убедитесь, что job `docs-portal-build` прошёл успешно в
  `.github/workflows/check-docs.yml`.
- Проверьте, что был запущен `ci/check_docs_portal.sh` (в логах CI отображается hero‑проверка).
- Убедитесь, что workflow preview загрузил manifest (`build/checksums.sha256`) и что
  скрипт проверки preview выполнился успешно (логи CI содержат вывод
  `scripts/preview_verify.sh`).
- Добавьте опубликованный preview‑URL из окружения GitHub Pages в описание PR.

## 3. Подписание по разделам

| Раздел | Владелец | Чек‑лист |
|--------|----------|----------|
| Главная | DevRel | Hero‑копирайт отображается; карточки quickstart ведут на валидные маршруты; CTA‑кнопки работают. |
| Norito | Norito WG | Обзор и гайды по началу работы ссылаются на актуальные CLI‑флаги и Norito‑schema‑доки. |
| SoraFS | Storage Team | Quickstart выполняется до конца, поля отчёта по manifest’у задокументированы, инструкции по симуляции fetch’а проверены. |
| SDK‑гайды | Лиды SDK | Rust/Python/JS‑гайды собирают актуальные примеры и ссылаются на живые репозитории. |
| Reference | Docs/DevRel | Индекс содержит свежие спецификации, справочник Norito‑codec’а соответствует `norito.md`. |
| Preview‑артефакт | Docs/DevRel | Артефакт `docs-portal-preview` прикреплён к PR, smoke‑проверки пройдены, ссылка опубликована для ревьюеров. |
| Security & Try it sandbox | Docs/DevRel · Security | Настроен OAuth device-code login (`DOCS_OAUTH_*`), чек‑лист `security-hardening.md` выполнен, заголовки CSP/Trusted Types проверены через `npm run build` или `npm run probe:portal`. |

Отметьте каждую строку в ходе review PR’а или зафиксируйте follow‑up‑задачи, чтобы статус
оставался точным.

## 4. Release‑notes

- Включайте `https://docs.iroha.tech/` (или URL окружения из job’а деплоя) в release‑notes
  и статусные апдейты.
- Явно перечисляйте новые или изменённые разделы, чтобы downstream‑команды понимали,
  какие smoke‑тесты им требуется перезапустить.
