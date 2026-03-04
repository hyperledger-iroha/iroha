---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/chunker-profile-authoring.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: авторство профиля-чанкера
title: Руководство по авторизации файлов фрагментов SoraFS
Sidebar_label: Руководство по авторизации фрагмента
описание: Контрольный список для предложения новых файлов и приспособлений для блоков SoraFS.
---

:::примечание Фуэнте каноника
Эта страница отражает `docs/source/sorafs/chunker_profile_authoring.md`. Нам пришлось скопировать синхронизированные копии, чтобы удалить комплект документации Sphinx.
:::

# Руководство по авторизации файлов фрагментов SoraFS

Это объяснение, которое предлагает и публикует новые файлы фрагментов для SoraFS.
Дополнение к RFC по архитектуре (SF-1) и ссылке на реестр (SF-2a)
с требованиями к конкретным авторам, требованиям к проверке и плантациям собственности.
Для канонического примера, проконсультируйтесь
`docs/source/sorafs/proposals/sorafs_sf1_profile_v1.json`
и в реестре участников сухого пробега в
`docs/source/sorafs/reports/sf1_determinism.md`.

## Резюме

Если вы хотите войти в реестр, выполните следующие действия:

- объявление параметров CDC, определяющих и настраивающих мультихэш-идентификаторы между всеми
  архитектуры;
- воспроизводимые фикстуры (JSON Rust/Go/TS + corpora fuzz + testigos PoR)
  последующие SDK можно проверить без использования инструментов medida;
- включить списки метаданных для управления (пространство имен, имя, имя) вместе с руководством по развертыванию.
  у вентанас оперативас; й
- пройдите набор определений различий до пересмотра совета.

Используйте контрольный список, чтобы подготовиться к выполнению этих правил.

## Резюме карты регистрации

Перед редактированием заявки подтвердите, что вы завершили приложение к карте регистрации.
пор `sorafs_manifest::chunker_registry::ensure_charter_compliance()`:

- Los IDs de perfil son enteros positivos que aumentan de forma monotona sin huecos.
- Ручка canónico (`namespace.name@semver`) может отображаться в списке псевдонимов.
  и **debe** ser la primera entrada. Siguen los alias heredados (стр. ej., `sorafs.sf1@1.0.0`).
- Псевдоним Ningún может столкнуться с другим ручным управлением, которое больше не будет отображаться.
- Лос-псевдоним должен быть пустым и записанным на белом фоне.

Доступные функции CLI:

```bash
# Listado JSON de todos los descriptores registrados (ids, handles, aliases, multihash)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles

# Emitir metadatos para un perfil por defecto candidato (handle canónico + aliases)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --promote-profile=sorafs.sf1@1.0.0 --json-out=-
```

Estos comandos mantienen las propuestas alineadas con la carta del registero y proporcionan los
метаданные канонические необходимые в дискуссиях правительства.

## Требуемые метаданные| Кампо | Описание | Пример (`sorafs.sf1@1.0.0`) |
|-------|-------------|------------------------------|
| `namespace` | Логическое объединение связанных файлов. | `sorafs` |
| `name` | Этикет разборчивый для людей. | `sf1` |
| `semver` | Семантическая информация о версии для соединения параметров. | `1.0.0` |
| `profile_id` | Цифровой идентификатор назначается только для того, чтобы войти в систему. Reserva el siguiente id pero no reuses numeros existentes. | `1` |
| `profile_aliases` | Указывает дополнительные дополнительные параметры (номера наследников, сокращения), которые используются клиентами во время переговоров. Включите каноническую ручку в качестве первого входа. | `["sorafs.sf1@1.0.0"]` |
| `profile.min_size` | Минимальная длина фрагмента в байтах. | `65536` |
| `profile.target_size` | Объектная длина фрагмента в байтах. | `262144` |
| `profile.max_size` | Максимальная длина фрагмента в байтах. | `524288` |
| `profile.break_mask` | Адаптивная тушь для использования с скользящей решеткой (шестнадцатеричной). | `0x0000ffff` |
| `profile.polynomial` | Шестерня Constante del polinomio (шестигранная). | `0x3da3358b4dc173` |
| `gear_seed` | Начальное значение используется для получения таблицы передач размером 64 КиБ. | `sorafs-v1-gear` |
| `chunk_multihash.code` | Código multihash для дайджеста порций. | `0x1f` (BLAKE3-256) |
| `chunk_multihash.digest` | Сборник канонических светильников. | `13fa...c482` |
| `fixtures_root` | Относительное направление, в котором содержатся восстановленные светильники. | `fixtures/sorafs_chunker/sorafs.sf1@1.0.0/` |
| `por_seed` | Начальное значение для определенного PoR (`splitmix64`). | `0xfeedbeefcafebabe` (пример) |

Метаданные должны быть доступны в документации, как в устройствах
регенерируется для реестра, инструментов CLI и автоматизации управления
подтвердите лос-ценности без крестов вручную. Если вы хотите, извлеките CLI из chunk-store и
Манифест с `--json-out=-` для передачи рассчитанных метаданных в заметки о пересмотре.

### Пункты контакта CLI и регистрации

- `sorafs_manifest_chunk_store --profile=<handle>` — позволяет извлечь фрагмент метаданных,
  дайджест манифеста и проверка PoR с нужными параметрами.
- `sorafs_manifest_chunk_store --json-out=-` — передать отчет о сохранении фрагментов
  стандартный вывод для автоматического сравнения.
- `sorafs_manifest_stub --chunker-profile=<handle>` — подтверждение того, что заявлены самолеты CAR
  вставьте дескриптор canonico под большим псевдонимом.
- `sorafs_manifest_stub --plan=-` — можно использовать в пищу `chunk_fetch_specs` предыдущий пункт
  проверка смещений/дайджестов после камбио.

Зарегистрируйте данные команд (дайджесты, результаты PoR, хэши манифеста) в папке, которая будет нужна
Лос-ревизоры могут быть воспроизведены в тексте.

## Контрольный список детерминированности и проверки1. **Регенерировать приспособления**
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors \
     --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
   ```
2. **Ejecutar la suite de paridad** — `cargo test -p sorafs_chunker` и другие различия
   На разных языках (`crates/sorafs_chunker/tests/vectors.rs`) нужно быть уверенным в зелени с лос
   новые светильники в своем доме.
3. **Воспроизведение пуха/противодавления тел** — выброс `cargo fuzz list` и el arnés de
   потоковая передача (`fuzz/sorafs_chunker`) против восстановленных действий.
4. **Verificar testigos Доказательство возможности восстановления** — ejecuta
   `sorafs_manifest_chunk_store --por-sample=<n>` использовать пропуск и подтвердить его
   некоторые совпадения с манифестом светильников.
5. **Пробный запуск CI** — локальный вызов `ci/check_sorafs_fixtures.sh`; эль сценарий
   debe tener éxito с новыми светильниками и существующим `manifest_signatures.json`.
6. **Подтверждение перекрестной среды выполнения** – подтверждение того, что привязки Go/TS используются после регенерации JSON.
   вы излучаете ограничения и перевариваете идентичные вещи.

Документация команд и дайджестов результатов в папке, которую может предоставить Tooling WG.
повторил грех замысла.

### Подтверждение манифеста / PoR

После восстановления приспособлений полностью извлеките трубопровод, чтобы обеспечить его безопасность.
метаданные CAR и las pruebas PoR sigan siendo соответствует:

```bash
# Validar metadata de chunk + PoR con el nuevo perfil
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --profile=sorafs.sf2@1.0.0 \
  --json-out=- --por-json-out=- fixtures/sorafs_chunker/input.bin

# Generar manifest + CAR y capturar chunk fetch specs
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --chunk-fetch-plan-out=chunk_plan.json \
  --manifest-out=sf2.manifest \
  --car-out=sf2.car \
  --json-out=sf2.report.json

# Reejecutar usando el plan de fetch guardado (evita offsets obsoletos)
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --plan=chunk_plan.json --json-out=-
```

Замените входной архив с любым представительским корпусом для ваших светильников
(стр. например, определенный поток размером 1 ГиБ) и дополнительные дайджесты, полученные в соответствии с собственными требованиями.

## Плантилья де пропуэста

Свойства отправляются как зарегистрированные Norito `ChunkerProfileProposalV1` зарегистрированные ru
`docs/source/sorafs/proposals/`. Растение JSON де Абахо иллюстрирует форму ожидания
(sustituye tus valores según sea necesario):


Соответствующий отчет Markdown (`determinism_report`) для захвата
команды, дайджесты фрагментов и различные варианты отправки во время проверки.

## Флюхо де гобернанса

1. **Предложите PR с собственностью + светильниками.** Включите созданные активы, собственность
   Norito и актуализируются `chunker_registry_data.rs`.
2. **Revisión del Tooling WG.** Внесенные изменения повторно вносятся в контрольный список проверки и
   Подтвердите, что собственность указана в соответствии с правилами регистрации (без повторного использования идентификатора,
   детерминизм удовлетворительный).
3. **Сообщение о совете.** Очень хорошо, лос-мимброс-дель-совет твердят в дайджесте.
   la propuesta (`blake3("sorafs-chunker-profile-v1" || canonical_bytes)`) и anexan sus
   фирмы, которые охраняют перила от оборудования.
4. **Публикация реестра.** Объединяет актуализированный реестр, документы и данные. Эль
   CLI для постоянного дефекта в предварительном файле, прежде чем правительство объявит миграцию
   список.
5. **Следует устаревать.** После выхода из миграции актуализировать реестр
   миграционную книгу.

## Авторский совет- Предпочтительные ограничения мощности пар, чтобы свести к минимуму удобство измельчения на куски в разных случаях.
- Эвита заменяет мультихеш-код без координат потребителей манифеста и шлюза; включая одну
  nota operativa cuando lo hagas.
- Возьмите семена табличного механизма, разборчивые для людей, для глобальных уникальных людей для упрощения.
  аудитории.
- Проверка артефактов сравнительного анализа (например, сравнение пропускной способности) ниже
  `docs/source/sorafs/reports/` для будущей ссылки.

Для ожидаемой работы во время развертывания, проконсультируйтесь с миграционным реестром
(`docs/source/sorafs/migration_ledger.md`). Для соблюдения требований во время выполнения
`docs/source/sorafs/chunker_conformance.md`.