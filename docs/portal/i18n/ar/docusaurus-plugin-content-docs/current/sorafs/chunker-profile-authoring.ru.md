---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/chunker-profile-authoring.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: chunker-profile-authoring
title: Руководство по авторингу профилей chunker SoraFS
sidebar_label: Авторинг chunker
description: Чеклист для предложения новых профилей chunker SoraFS и fixtures.
---

:::note Канонический источник
Эта страница отражает `docs/source/sorafs/chunker_profile_authoring.md`. Держите обе копии синхронизированными, пока старый набор Sphinx не будет выведен из эксплуатации.
:::

# Руководство по авторингу профилей chunker SoraFS

Это руководство объясняет, как предлагать и публиковать новые профили chunker для SoraFS.
Оно дополняет архитектурный RFC (SF-1) и справочник реестра (SF-2a)
конкретными требованиями к авторингу, шагами валидации и шаблонами предложений.
В качестве канонического примера см.
`docs/source/sorafs/proposals/sorafs_sf1_profile_v1.json`
и соответствующий dry-run лог в
`docs/source/sorafs/reports/sf1_determinism.md`.

## Обзор

Каждый профиль, попадающий в реестр, должен:

- объявлять детерминированные параметры CDC и настройки multihash, одинаковые на всех
  архитектурах;
- поставлять воспроизводимые fixtures (JSON Rust/Go/TS + fuzz corpora + PoR witness), которые
  downstream SDKs могут проверить без специализированного tooling;
- включать метаданные, готовые для governance (namespace, name, semver), а также рекомендации
  по миграции и окна совместимости; и
- проходить детерминированную diff-suite до ревью совета.

Следуйте чеклисту ниже, чтобы подготовить предложение, удовлетворяющее этим правилам.

## Снимок чартеров реестра

Перед подготовкой предложения убедитесь, что оно соответствует чартеру реестра,
который обеспечивает `sorafs_manifest::chunker_registry::ensure_charter_compliance()`:

- ID профилей — положительные целые числа, монотонно возрастающие без пропусков.
- Канонический handle (`namespace.name@semver`) должен присутствовать в списке alias и
- Ни один alias не должен конфликтовать с другим каноническим handle и повторяться.
- Alias должны быть непустыми и без пробелов по краям.

Полезные CLI помощники:

```bash
# JSON список всех зарегистрированных дескрипторов (ids, handles, aliases, multihash)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles

# Эмитить метаданные для кандидата на профиль по умолчанию (канонический handle + aliases)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --promote-profile=sorafs.sf1@1.0.0 --json-out=-
```

Эти команды держат предложения согласованными с чартером реестра и дают канонические
метаданные для обсуждений governance.

## Требуемые метаданные

| Поле | Описание | Пример (`sorafs.sf1@1.0.0`) |
|------|----------|------------------------------|
| `namespace` | Логическая группировка связанных профилей. | `sorafs` |
| `name` | Читаемая человеком метка. | `sf1` |
| `semver` | Строка семантической версии для набора параметров. | `1.0.0` |
| `profile_id` | Монотонный числовой идентификатор, назначаемый после попадания профиля. Резервируйте следующий id, но не переиспользуйте существующие номера. | `1` |
| `profile.min_size` | Минимальная длина чанка в bytes. | `65536` |
| `profile.target_size` | Целевая длина чанка в bytes. | `262144` |
| `profile.max_size` | Максимальная длина чанка в bytes. | `524288` |
| `profile.break_mask` | Адаптивная маска для rolling hash (hex). | `0x0000ffff` |
| `profile.polynomial` | Константа gear полинома (hex). | `0x3da3358b4dc173` |
| `gear_seed` | Seed для вычисления 64 KiB gear таблицы. | `sorafs-v1-gear` |
| `chunk_multihash.code` | Multihash код для digest по чанкам. | `0x1f` (BLAKE3-256) |
| `chunk_multihash.digest` | Digest канонического bundles fixtures. | `13fa...c482` |
| `fixtures_root` | Относительный каталог с регенерированными fixtures. | `fixtures/sorafs_chunker/sorafs.sf1@1.0.0/` |
| `por_seed` | Seed для детерминированной PoR выборки (`splitmix64`). | `0xfeedbeefcafebabe` (пример) |

Метаданные должны присутствовать как в документе предложения, так и внутри сгенерированных
fixtures, чтобы реестр, CLI tooling и автоматизация governance могли подтвердить значения без
ручных сверок. Если есть сомнения, запускайте chunk-store и manifest CLIs с `--json-out=-`,
чтобы стримить вычисленные метаданные в заметки ревью.

### Точки взаимодействия CLI и реестра

- `sorafs_manifest_chunk_store --profile=<handle>` — повторно запускает метаданные чанка,
  manifest digest и PoR проверки с предлагаемыми параметрами.
- `sorafs_manifest_chunk_store --json-out=-` — стримит отчет chunk-store в stdout для
  автоматизированных сравнений.
- `sorafs_manifest_stub --chunker-profile=<handle>` — подтверждает, что manifests и CAR
  планы встраивают канонический handle и aliases.
- `sorafs_manifest_stub --plan=-` — подает предыдущие `chunk_fetch_specs` для проверки
  offsets/digests после изменения.

Запишите вывод команд (digests, PoR roots, manifest hashes) в предложении, чтобы ревьюеры могли
воспроизвести их буквально.

## Чеклист детерминизма и валидации

1. **Регенерировать fixtures**
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors \
     --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
   ```
2. **Запустить suite паритета** — `cargo test -p sorafs_chunker` и cross-language diff harness
   (`crates/sorafs_chunker/tests/vectors.rs`) должны быть зелеными с новыми fixtures.
3. **Переиграть fuzz/back-pressure corpora** — выполните `cargo fuzz list` и streaming harness
   (`fuzz/sorafs_chunker`) на регенерированных assets.
4. **Проверить Proof-of-Retrievability witnesses** — запустите
   `sorafs_manifest_chunk_store --por-sample=<n>` с предлагаемым профилем и подтвердите,
   что roots совпадают с fixture manifest.
5. **CI dry run** — выполните `ci/check_sorafs_fixtures.sh` локально; скрипт должен
   пройти с новыми fixtures и существующим `manifest_signatures.json`.
6. **Cross-runtime подтверждение** — убедитесь, что Go/TS bindings потребляют регенерированный
   JSON и выдают идентичные границы чанков и digests.

Документируйте команды и полученные digests в предложении, чтобы Tooling WG мог повторить их без догадок.

### Подтверждение Manifest / PoR

После регенерации fixtures запустите полный manifest pipeline, чтобы убедиться, что
CAR метаданные и PoR proofs остаются согласованными:

```bash
# Проверить метаданные чанка + PoR с новым профилем
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --profile=sorafs.sf2@1.0.0 \
  --json-out=- --por-json-out=- fixtures/sorafs_chunker/input.bin

# Сгенерировать manifest + CAR и сохранить chunk fetch specs
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --chunk-fetch-plan-out=chunk_plan.json \
  --manifest-out=sf2.manifest \
  --car-out=sf2.car \
  --json-out=sf2.report.json

# Повторно запустить с сохраненным планом fetch (защищает от устаревших offsets)
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --plan=chunk_plan.json --json-out=-
```

Замените входной файл любым представительным корпусом, используемым в ваших fixtures
(например, детерминированным потоком 1 GiB), и приложите полученные digests к предложению.

## Шаблон предложения

Предложения подаются как Norito записи `ChunkerProfileProposalV1` и фиксируются в
`docs/source/sorafs/proposals/`. JSON шаблон ниже показывает ожидаемую форму
(подставьте свои значения по мере необходимости):


Предоставьте соответствующий Markdown отчет (`determinism_report`), фиксирующий вывод
команд, digests чанков и любые отклонения, обнаруженные при валидации.

## Governance workflow

1. **Отправить PR с предложением + fixtures.** Включите сгенерированные assets, Norito
   предложение и обновления `chunker_registry_data.rs`.
2. **Ревью Tooling WG.** Ревьюеры повторно запускают чеклист валидации и подтверждают,
   что предложение соответствует правилам реестра (без повторного использования id,
   детерминизм достигнут).
3. **Конверт совета.** После одобрения члены совета подписывают digest предложения
   (`blake3("sorafs-chunker-profile-v1" || canonical_bytes)`) и добавляют подписи
   в конверт профиля, хранящийся вместе с fixtures.
4. **Публикация реестра.** Merge обновляет реестр, docs и fixtures. По умолчанию CLI
   остается на предыдущем профиле, пока governance не объявит миграцию готовой.
5. **Отслеживание депрекации.** После окна миграции обновите реестр, отметив замененные

## Советы по авторингу

- Предпочитайте четные границы степеней двойки, чтобы минимизировать крайние случаи chunking.
- Не меняйте multihash код без координации с потребителями manifest и gateway; добавляйте
  заметку о совместимости.
- Держите seeds gear таблицы читаемыми, но глобально уникальными для упрощения аудита.
- Сохраняйте артефакты бенчмаркинга (например, сравнения throughput) в
  `docs/source/sorafs/reports/` для будущих ссылок.

Операционные ожидания во время rollout см. в migration ledger
(`docs/source/sorafs/migration_ledger.md`). Правила runtime соответствия см. в
`docs/source/sorafs/chunker_conformance.md`.
