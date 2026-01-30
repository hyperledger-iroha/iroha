---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/chunker-registry-charter.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: chunker-registry-charter
title: Хартия реестра chunker SoraFS
sidebar_label: Хартия реестра chunker
description: Хартия управления для подачи и утверждения профилей chunker.
---

:::note Канонический источник
Эта страница отражает `docs/source/sorafs/chunker_registry_charter.md`. Держите обе копии синхронизированными, пока старый набор Sphinx не будет выведен из эксплуатации.
:::

# Хартия управления реестром chunker SoraFS

> **Ратифицировано:** 2025-10-29 Sora Parliament Infrastructure Panel (см.
> `docs/source/sorafs/council_minutes_2025-10-29.md`). Любые поправки требуют
> формального голосования по governance; команды внедрения должны считать этот документ
> нормативным, пока не будет утверждена новая хартия.

Эта хартия определяет процесс и роли для эволюции реестра chunker SoraFS.
Она дополняет [Руководство по авторингу профилей chunker](./chunker-profile-authoring.md), описывая, как новые
профили предлагаются, рассматриваются, ратифицируются и в итоге выводятся из обращения.

## Область

Хартия применяется к каждой записи в `sorafs_manifest::chunker_registry` и
к любому tooling, который потребляет реестр (manifest CLI, provider-advert CLI,
SDKs). Она фиксирует инварианты alias и handle, проверяемые
`chunker_registry::ensure_charter_compliance()`:

- ID профилей — положительные целые числа, монотонно возрастающие.
- Канонический handle `namespace.name@semver` **должен** быть первой записью
- Строки alias обрезаны, уникальны и не конфликтуют с каноническими handle других записей.

## Роли

- **Автор(ы)** – готовят предложение, регенерируют fixtures и собирают
  доказательства детерминизма.
- **Tooling Working Group (TWG)** – валидирует предложение по опубликованным
  чеклистам и убеждается, что инварианты реестра соблюдены.
- **Governance Council (GC)** – рассматривает отчет TWG, подписывает конверт предложения
  и утверждает сроки публикации/депрекации.
- **Storage Team** – поддерживает реализацию реестра и публикует
  обновления документации.

## Жизненный цикл

1. **Подача предложения**
   - Автор запускает чеклист валидации из руководства по авторингу и создает
     JSON `ChunkerProfileProposalV1` в
     `docs/source/sorafs/proposals/`.
   - Включает вывод CLI из:
     ```bash
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
       --promote-profile=<handle> --json-out=-
     cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
       --chunker-profile=<handle> --json-out=-
     ```
   - Отправляет PR, содержащий fixtures, предложение, отчет о детерминизме и
     обновления реестра.

2. **Ревью tooling (TWG)**
   - Повторяет чеклист валидации (fixtures, fuzz, pipeline manifest/PoR).
   - Запускает `cargo test -p sorafs_car --chunker-registry` и убеждается, что
     `ensure_charter_compliance()` проходит с новой записью.
   - Проверяет, что поведение CLI (`--list-profiles`, `--promote-profile`, streaming
     `--json-out=-`) отражает обновленные alias и handle.
   - Готовит краткий отчет с выводами и статусом pass/fail.

3. **Одобрение совета (GC)**
   - Рассматривает отчет TWG и метаданные предложения.
   - Подписывает digest предложения (`blake3("sorafs-chunker-profile-v1" || bytes)`)
     и добавляет подписи в конверт совета, который хранится рядом с fixtures.
   - Фиксирует результат голосования в governance протоколе.

4. **Публикация**
   - Мержит PR, обновляя:
     - `sorafs_manifest::chunker_registry_data`.
     - Документацию (`chunker_registry.md`, руководства по авторингу/соответствию).
     - Fixtures и отчеты о детерминизме.
   - Уведомляет операторов и команды SDK о новом профиле и плане rollout.

5. **Депрекация / Закат**
   - Предложения, заменяющие существующий профиль, должны включать окно двойной публикации
     (грейс-периоды) и план upgrade.
     в реестре и обновить migration ledger.

6. **Экстренные изменения**
   - Удаление или hotfix требуют голосования совета с большинством.
   - TWG должен документировать шаги снижения риска и обновить журнал инцидентов.

## Ожидания от tooling

- `sorafs_manifest_chunk_store` и `sorafs_manifest_stub` предоставляют:
  - `--list-profiles` для инспекции реестра.
  - `--promote-profile=<handle>` для генерации канонического блока метаданных,
    используемого при продвижении профиля.
  - `--json-out=-` для стриминга отчетов в stdout, обеспечивая воспроизводимые
    логи ревью.
- `ensure_charter_compliance()` вызывается при запуске релевантных бинарников
  (`manifest_chunk_store`, `provider_advert_stub`). CI тесты должны падать, если
  новые записи нарушают хартию.

## Документирование

- Храните все отчеты о детерминизме в `docs/source/sorafs/reports/`.
- Протоколы совета с решениями по chunker находятся в
  `docs/source/sorafs/migration_ledger.md`.
- Обновляйте `roadmap.md` и `status.md` после каждого крупного изменения реестра.

## Ссылки

- Руководство по авторингу: [Руководство по авторингу профилей chunker](./chunker-profile-authoring.md)
- Чеклист соответствия: `docs/source/sorafs/chunker_conformance.md`
- Справочник реестра: [Реестр профилей chunker](./chunker-registry.md)
