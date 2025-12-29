---
lang: ru
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/reference/publishing-checklist.md
status: complete
translator: manual
source_hash: 9be80e0138e1e8aa453c703c53069837b24f29f6b463d14c846a01b015918f24
source_last_modified: "2025-11-05T23:45:36.259903+00:00"
translation_last_reviewed: 2025-11-14
---

---
title: Чек‑лист публикации портала (2025‑Q2)
description: Шаги проверки перед обновлением портала документации Iroha (snapshot 2025‑Q2).
---

Используйте этот чек‑лист при обновлении портала разработчика для snapshot’а 2025‑Q2. Он
обеспечивает, что CI‑сборка, деплой GitHub Pages и ручные smoke‑тесты покрывают ключевые
разделы перед релизом или вехой roadmap’а.

## 1. Локальная проверка

- `npm run sync-openapi -- --version=current --latest` (при изменении OpenAPI для
  замороженного snapshot’а добавьте флаги `--mirror=<label>`).
- `npm run build` — убедитесь, что hero‑копирайт “Build on Iroha with confidence” всё
  ещё присутствует в `build/index.html`.
- `./docs/portal/scripts/preview_verify.sh --build-dir build` — подтвердите manifest
  checksum’ов (используйте `--descriptor`/`--archive` при проверке скачанных CI‑bundle’ов).
- `npm run serve` — запустите checksum‑gated preview, который валидирует manifest перед
  вызовом `docusaurus serve`, чтобы никто не просматривал неподписанный snapshot.
- Просмотрите изменённые markdown‑файлы через `npm run start` и dev‑сервер.

## 2. Проверки pull‑request’а

- Убедитесь, что job `docs-portal-build` в `.github/workflows/check-docs.yml` прошёл.
- Проверьте выполнение `ci/check_docs_portal.sh` (в логах CI видна hero‑проверка).
- Убедитесь, что workflow preview загрузил `build/checksums.sha256` и что
  `scripts/preview_verify.sh` завершился без ошибок (логи CI содержат его вывод).
- Добавьте preview‑URL из окружения GitHub Pages в описание PR.

## 3. Подписание по разделам

| Раздел | Владелец | Чек‑лист |
|--------|----------|----------|
| Главная | DevRel | Hero‑копирайт отображается, карточки quickstart ведут на валидные маршруты, CTA‑кнопки работают. |
| Norito | Norito WG | Обзор и гайды по началу работы ссылаются на актуальные флаги CLI и Norito‑schema‑доки. |
| SoraFS | Storage Team | Quickstart завершается, поля отчёта по manifest’у задокументированы, инструкции по симуляции fetch’а проверены. |
| SDK‑гайды | Лиды SDK | Гайды Rust/Python/JS строятся с актуальными примерами и ссылаются на живые репозитории. |
| Reference | Docs/DevRel | Индекс перечисляет актуальные спецификации, справочник Norito‑codec’а соответствует текущему `norito.md`. |
| Preview‑артефакт | Docs/DevRel | `docs-portal-preview` прикреплён к PR, smoke‑чек прошёл, ссылка передана ревьюерам. |
| Security & Try it sandbox | Docs/DevRel · Security | OAuth device‑code login (`DOCS_OAUTH_*`) настроен, чек‑лист `security-hardening.md` выполнен, заголовки CSP/Trusted Types проверены. |

## 4. Release‑notes

- Указывайте `https://docs.iroha.tech/` (или URL окружения из job’а деплоя) в
  release‑notes и статусных обновлениях.
- Явно перечисляйте новые/изменённые разделы, чтобы downstream‑команды знали, какие
  smoke‑тесты им нужно перезапустить.
