---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/chunker-profile-authoring.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: авторство профиля-чанкера
title: Руководство по созданию блока профилей SoraFS
Sidebar_label: Руководство по созданию чанка
описание: Контрольный список для предложения новых профилей SoraFS и светильников.
---

:::note Источник канонический
Эта страница отражена `docs/source/sorafs/chunker_profile_authoring.md`. Gardez les deux копирует синхронизированные копии только для полного восстановления набора наследия Сфинкса.
:::

# Руководство по созданию чанкера профилей SoraFS

Это руководство предлагает поясняющие комментарии и публикует новые профили для SoraFS.
Полный RFC по архитектуре (SF-1) и ссылка на реестр (SF-2a)
с потребностями конкретной редакции, этапами проверки и моделями предложений.
Налейте канонический пример, voir
`docs/source/sorafs/proposals/sorafs_sf1_profile_v1.json`
et le log de Dry-Run Associé dans
`docs/source/sorafs/reports/sf1_determinism.md`.

## ансамбль

Введите профиль, который необходимо пройти при регистрации:

- объявление параметров CDC, определяющих и регулирующих мультихэш-идентификаторы между
  архитектуры;
- четыре обновляемых светильника (JSON Rust/Go/TS + corpora fuzz + témoins PoR), которые
  SDK можно проверить без инструментов;
- включить метадонные запросы для управления (пространство имен, имя, значение) ainsi que
  советы по развертыванию и операционным возможностям; и др.
- pass la suite de diff déterministe avant la revue du conseil.

Используйте контрольный список для подготовки предложения, которое соблюдает эти правила.

## Обзор чарта регистрации

Прежде чем внести предложение, проверьте, соответствует ли приложение к хартии регистрации.
пар `sorafs_manifest::chunker_registry::ensure_charter_compliance()`:

- Les ID de profil sont des entiers positifs qui augmentent de façon monotone santrous.
- Каноническая ручка (`namespace.name@semver`) находится в списке псевдонимов.
- Этот псевдоним не может вступить в столкновение с другим дескриптором канонического устройства плюс d'une fois.
- Les alias doivent être non vides et tripés des espaces.

Помощники CLI:

```bash
# Listing JSON de tous les descripteurs enregistrés (ids, handles, aliases, multihash)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles

# Émettre des métadonnées pour un profil par défaut candidat (handle canonique + aliases)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --promote-profile=sorafs.sf1@1.0.0 --json-out=-
```

Эти команды поддерживают согласованные предложения с хартией регистрации и четырьмя
канонические метадонники, необходимые для дискуссий по управлению.

## Реквизиты Métadonnées| Чемпион | Описание | Пример (`sorafs.sf1@1.0.0`) |
|-------|-------------|------------------------------|
| `namespace` | Логика перегруппировки профилей лжи. | `sorafs` |
| `name` | Libellé lisible. | `sf1` |
| `semver` | Семантическая версия для набора параметров. | `1.0.0` |
| `profile_id` | Идентифицирующая монотонная цифра является атрибутом цельного профиля. Зарезервируйте все, что осталось, но не используйте больше числа существующих людей. | `1` |
| `profile.min_size` | Минимальная длина фрагмента в байтах. | `65536` |
| `profile.target_size` | Длинный кабель в байтах. | `262144` |
| `profile.max_size` | Максимальная длина фрагмента в байтах. | `524288` |
| `profile.break_mask` | Маска адаптируется с использованием скользящего хеша (шестнадцатеричного). | `0x0000ffff` |
| `profile.polynomial` | Шестерня Constante du Polynôme (шестигранная). | `0x3da3358b4dc173` |
| `gear_seed` | Начальное значение, используемое для получения табличного механизма размером 64 КиБ. | `sorafs-v1-gear` |
| `chunk_multihash.code` | Мультихеширование кода для дайджестов по частям. | `0x1f` (BLAKE3-256) |
| `chunk_multihash.digest` | Дайджест канонических светильников. | `13fa...c482` |
| `fixtures_root` | Репертуар связан с регулярными светильниками. | `fixtures/sorafs_chunker/sorafs.sf1@1.0.0/` |
| `por_seed` | Начальное значение для детерминированного PoR (`splitmix64`). | `0xfeedbeefcafebabe` (пример) |

Les métadonnées doivent apparaître à la fois dans le document de proposition et à l'inérieur des
общие настройки для регистрации, инструментов CLI и мощной автоматизации управления
подтверждение стоимости без возмещения вручную. В данном случае выполните CLI-хранилище фрагментов и т. д.
Манифест с `--json-out=-` для трансляции метадоннических расчетов в заметках об ревю.

### Контактные данные CLI и регистрация

- `sorafs_manifest_chunk_store --profile=<handle>` — relancer les métadonnées de chunk,
  Дайджест манифеста и проверки PoR с предлагаемыми параметрами.
- `sorafs_manifest_chunk_store --json-out=-` — стример раппорта версии chunk-store
  stdout для автоматизированных сравнений.
- `sorafs_manifest_stub --chunker-profile=<handle>` — подтверждение того, что манифесты и файлы
  планирует CAR установить дескриптор canonique и псевдонимы.
- `sorafs_manifest_stub --plan=-` — повторно введите предыдущий `chunk_fetch_specs` для
  проверка смещений/дайджестов после модификации.

Отправьте вылазку команд (дайджесты, сборы PoR, хэши манифеста) в предложении в ближайшее время
que les рецензенты puissent les reproduire mot pour mot.

## Контрольный список определения и проверки1. **Обработка светильников**
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors \
     --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
   ```
2. **Exécuter la suite de parité** — `cargo test -p sorafs_chunker` и разница в проводке
   межъязыковый (`crates/sorafs_chunker/tests/vectors.rs`) doivent être verts avec les
   Nouvelles светильники на месте.
3. **Rejouer les corpora fuzz/back-pressure** — выполните `cargo fuzz list` и обвязку
   потоковая передача (`fuzz/sorafs_chunker`) против устаревших активов.
4. **Проверка доказательств возможности восстановления** — exécutez
   `sorafs_manifest_chunk_store --por-sample=<n>` с предложенным профилем и подтверждением его
   Корреспондент racines в манифесте светильников.
5. **Пробный прогон CI** — вызов локали `ci/check_sorafs_fixtures.sh`; сценарий
   doit réussir с новыми светильниками и существующими `manifest_signatures.json`.
6. **Подтверждение в разных средах выполнения** — убедитесь, что привязки Go/TS соответствуют JSON.
   régénéré и émettent des Limites et Digests identiques.

Документируйте команды и дайджесты результатов в предложении, которое может предложить Tooling WG.
les rejouer без домыслов.

### Манифест подтверждения / PoR

После регенерации светильников выполните полный манифест конвейера для гарантии того, что ле
métadonnées CAR и les preuves PoR restent cohérentes:

```bash
# Valider les métadonnées chunk + PoR avec le nouveau profil
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --profile=sorafs.sf2@1.0.0 \
  --json-out=- --por-json-out=- fixtures/sorafs_chunker/input.bin

# Générer manifest + CAR et capturer les chunk fetch specs
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --chunk-fetch-plan-out=chunk_plan.json \
  --manifest-out=sf2.manifest \
  --car-out=sf2.car \
  --json-out=sf2.report.json

# Relancer avec le plan de fetch sauvegardé (évite les offsets obsolètes)
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --plan=chunk_plan.json --json-out=-
```

Замените входную карту в корпусе, используемом для ваших светильников.
(например, определен поток 1 ГиБ) и присоединяйтесь к дайджестам результатов в соответствии с предложением.

## Модель предложения

Предложения в форме записей Norito `ChunkerProfileProposalV1` помещены в записи
`docs/source/sorafs/proposals/`. Шаблон JSON Ci-dessous, иллюстрирующий форму участника
(заменить значения, если они необходимы):


Fournissez un rapport Markdown корреспондент (`determinism_report`), который захватывает вылазку
команды, дайджесты фрагментов и все расхождения происходят при проверке.

## Потоки управления

1. **Союз PR с предложением + оборудование.** Включает общие активы,
   предложение Norito и les Mises à jour de `chunker_registry_data.rs`.
2. **Revue Tooling WG.** Рецензенты радуются контрольному списку проверки и подтверждения.
   что предложение соблюдает правила регистрации (pas de reutilisation d'id,
   удовлетворительный детерминизм).
3. **Конверт для совета.** Для одобрения члены совета подписывают дайджест.
   предложение (`blake3("sorafs-chunker-profile-v1" || canonical_bytes)`) и дополнительные
   Подпишитесь на конверте профиля со всеми светильниками.
4. **Публикация реестра.** Объединение в течение дня с регистрацией, документами и приборами.
   CLI по умолчанию остается в предыдущем профиле только потому, что управление объявляется
   миграционная практика.
5. **Следующая амортизация**. После окончания миграции необходимо провести регистрацию в течение дня.
   де миграции.

## Советы по созданию- Предпочтение отдается мощности двух пар для минимизации необходимости разделения на части в бордюре.
- Выполните смену мультихэш-кода без согласования манифеста и шлюза пользователей;
  включите примечание к операции «Lorsque vous le faites».
- Найдите более уникальные глобальные семена настольного оборудования для упрощения аудита.
- Stockez рекламирует артефакты сравнительного анализа (например, сравнения дебета) су
  `docs/source/sorafs/reports/` для ссылки на будущее.

Чтобы следить за операциями при развертывании, представьте себе книгу миграции
(`docs/source/sorafs/migration_ledger.md`). Для соблюдения правил соответствия среды выполнения, представьте себе
`docs/source/sorafs/chunker_conformance.md`.