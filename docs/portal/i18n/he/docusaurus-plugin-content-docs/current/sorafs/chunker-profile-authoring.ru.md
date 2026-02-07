---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/chunker-profile-authoring.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: chunker-profile-authoring
כותרת: Руководство по авторингу профилей chunker SoraFS
sidebar_label: Авторинг chunker
תיאור: Чеклист для предложения новых профилей chunker SoraFS и גופי.
---

:::note Канонический источник
Эта страница отражает `docs/source/sorafs/chunker_profile_authoring.md`. Держите обе копии синхронизированными, пока старый набор Sphinx не будет выведен из эксплуатации.
:::

# Руководство по авторингу профилей chunker SoraFS

Это руководство объясняет, как предлагать и публиковать новые chunker ל-SoraFS.
Оно дополняет архитектурный RFC (SF-1) и справочник реестра (SF-2a)
конкретными требованиями к авторингу, шагами валидации ו шаблонами предложений.
В качестве канонического примера см.
`docs/source/sorafs/proposals/sorafs_sf1_profile_v1.json`
и соответствующий ריצה יבשה лог в
`docs/source/sorafs/reports/sf1_determinism.md`.

## תיאור

Каждый профиль, попадающий в реестр, должен:

- объявлять детерминированные параметры CDC и настройки multihash, одинаковые на всех
  архитектурах;
- поставлять воспроизводимые גופי (JSON Rust/Go/TS + גופי fuzz + PoR witness), которые
  SDKs במורד הזרם могут проверить без специализированного כלי עבודה;
- включать метаданные, готовые לממשל (מרחב שמות, שם, semver), а также рекомендации
  по миграции и окна совместимости; и
- הצע סוויטה שונה ליחידה.

Следуйте чеклисту ниже, чтобы подготовить предложение, удовлетворяющее этим правилам.

## Снимок чартеров реестра

Перед подготовкой предложения убедитесь, что оно соответствует чартеру реестра,
который обеспечивает `sorafs_manifest::chunker_registry::ensure_charter_compliance()`:

- ID профилей — положительные целые числа, монотонно возрастающие без пропусков.
- ידית Канонический (`namespace.name@semver`) должен присутствовать в списке alias и
- Ни один כינוי не должен конфликтовать с другим каноническим handle и повторяться.
- כינוי должны быть непустыми и без пробелов по краям.

פרופילי CLI:

```bash
# JSON список всех зарегистрированных дескрипторов (ids, handles, aliases, multihash)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles

# Эмитить метаданные для кандидата на профиль по умолчанию (канонический handle + aliases)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --promote-profile=sorafs.sf1@1.0.0 --json-out=-
```

Эти команды держат предложения согласованными с чартером реестра и дают канонические
метаданные для обсуждений ממשל.

## Требуемые метаданные| Поле | Описание | Пример (`sorafs.sf1@1.0.0`) |
|------|--------|--------------------------------|
| `namespace` | Логическая группировка связанных профилей. | `sorafs` |
| `name` | Читаемая человеком метка. | `sf1` |
| `semver` | Строка семантической версии для набора параметров. | `1.0.0` |
| `profile_id` | Монотонный числовой идентификатор, назначаемый после попадания профиля. Резервируйте следующий id, но не переиспользуйте существующие номера. | `1` |
| `profile.min_size` | Минимальная длина чанка в בתים. | `65536` |
| `profile.target_size` | Целевая длина чанка в בתים. | `262144` |
| `profile.max_size` | Максимальная длина чанка в בתים. | `524288` |
| `profile.break_mask` | Адаптивная маска לגלגול חשיש (hex). | `0x0000ffff` |
| `profile.polynomial` | Константа gear полинома (hex). | `0x3da3358b4dc173` |
| `gear_seed` | זרע עבור вычисления 64 KiB ציוד таблицы. | `sorafs-v1-gear` |
| `chunk_multihash.code` | Multihash код для digest по чанкам. | `0x1f` (BLAKE3-256) |
| `chunk_multihash.digest` | אביזרי חבילות Digest канонического. | `13fa...c482` |
| `fixtures_root` | Относительный каталог с регенерированными גופי. | `fixtures/sorafs_chunker/sorafs.sf1@1.0.0/` |
| `por_seed` | Seed для детерминированной PoR выборки (`splitmix64`). | `0xfeedbeefcafebabe` (פרימייר) |

Метаданные должны присутствовать как в документе предложения, так и внутри сгенерированных
מתקנים, ציוד עזר, כלי CLI וממשל אוטומאטי.
ручных сверок. Если есть сомнения, запускайте chunk-store ו- Manifest CLIs с `--json-out=-`,
чтобы стримить вычисленные метаданные в заметки ревью.

### Точки взаимодействия CLI и реестра

- `sorafs_manifest_chunk_store --profile=<handle>` — повторно запускает метаданные чанка,
  manifest digest и PoR проверки с предлагаемыми параметрами.
- `sorafs_manifest_chunk_store --json-out=-` — פריט отчет chunk-store в stdout для
  автоматизированных сравнений.
- `sorafs_manifest_stub --chunker-profile=<handle>` — подтверждает, что manifests и CAR
  планы встраивают канонический ידית וכינויים.
- `sorafs_manifest_stub --plan=-` — подает предыдущие `chunk_fetch_specs` для проверки
  קיזוז/עיכובים после изменения.

Запишите вывод команд (עיכובים, שורשי PoR, גיבוב מניפסט) в предложении, чтобы ревьюеры могли
воспроизвести их буквально.

## Чеклист детерминизма и валидации1. **משחקי Регенерировать**
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors \
     --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
   ```
2. **Запустить suite паритета** — `cargo test -p sorafs_chunker` ורתמת הבדל בין שפות
   (`crates/sorafs_chunker/tests/vectors.rs`) должны быть зелеными с новыми גופי.
3. **הצג את גופי ה-fuzz/back-pressure corpora** — выполните `cargo fuzz list` ורתמת סטרימינג
   (`fuzz/sorafs_chunker`) על נכסי регенерированных.
4. **צריך עדי הוכחה לשליפה** — запустите
   `sorafs_manifest_chunk_store --por-sample=<n>` с предлагаемым профилем и подтвердите,
   что roots совпадают с מתקן מניפסט.
5. **ריצת CI יבשה** — выполните `ci/check_sorafs_fixtures.sh` локально; скрипт должен
   пройти с новыми fixtures и существующим `manifest_signatures.json`.
6. **Cross-runtime подтверждение** — убедитесь, что Go/TS bindings потребляют регенерированный
   JSON и выдают идентичные границы чанков и digests.

Документируйте команды и полученные digests в предложении, чтобы Tooling WG мог повторить их без догадок.

### Подтверждение מניפסט / PoR

После регенерации גופי запустите полный מניפסט צינור, чтобы убедиться, что
CAR метаданные и PoR הוכחות остаются согласованными:

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

Замените входной файл любым представительным корпусом, используемым ваших
(например, детерминированным потоком 1 GiB), и приложите полученные digests к предложению.

## Шаблон предложения

Предложения подаются как Norito ספייס `ChunkerProfileProposalV1` ופיקוח в
`docs/source/sorafs/proposals/`. JSON шаблон ниже показывает ожидаемую FORMу
(подставьте свои значения по мере необходимости):


Предоставьте соответствующий Markdown отчет (`determinism_report`), фиксирующий вывод
команд, digests чанков и любые отклонения, обнаруженные при валидации.

## זרימת עבודה של ממשל

1. **Отправить PR с предложением + fixtures.** Включите сгенерированные assets, Norito
   предложение и обновления `chunker_registry_data.rs`.
2. **Ревью Tooling WG.** Ревьюеры повторно запускают чеклист валидации и подтверждают,
   что предложение соответствует правилам реестра (ללא זיהוי повторного использования,
   детерминизм достигнут).
3. **Конверт совета.** После одобрения члены совета подписывают digest предложения
   (`blake3("sorafs-chunker-profile-v1" || canonical_bytes)`) ו- добавляют подписи
   в конверт профиля, хранящийся вместе с גופי.
4. **Публикация реестра.** מיזוג обновляет реестр, מסמכים וגופים. По умолчанию CLI
   остается на предыдущем профиле, пока ממשל не объявит миграцию готовой.
5. **Отслеживание депрекации.** После окна миграции обновите реестр, отметив замененные

## Советы по авторингу

- Предпочитайте четные границы степеней двойки, чтобы минимизировать крайние случаи chunking.
- אין להשתמש ב-multihash код без координации с потребителями מניפסט и שער; добавляйте
  заметку о совместимости.
- Держите ציוד זרעים таблицы читаемыми, но глобально уникальными для упрощения аудита.
- Сохраняйте артефакты бенчмаркинга (נקרא, תפוקה сравнения) в
  `docs/source/sorafs/reports/` עבור будущих ссылок.

Операционные ожидания во время השקה см. в ספר הגירה
(`docs/source/sorafs/migration_ledger.md`). Правила זמן ריצה соответствия см. в
`docs/source/sorafs/chunker_conformance.md`.