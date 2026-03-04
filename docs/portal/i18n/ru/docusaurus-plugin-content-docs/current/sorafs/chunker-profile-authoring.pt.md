---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/chunker-profile-authoring.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: авторство профиля-чанкера
title: Авторское руководство по работе с чанкером da SoraFS
Sidebar_label: Руководство по авторизации фрагмента
описание: Контрольный список для обеспечения новых возможностей и приспособлений для блоков SoraFS.
---

:::примечание Fonte canonica
Эта страница написана `docs/source/sorafs/chunker_profile_authoring.md`. Мантенья представился как копиас синхронизадас.
:::

# Инструкция по авторизации фрагмента данных SoraFS

Это пояснение, как и опубликование новых результатов фрагментации для SoraFS.
Дополняющий RFC по архитектуре (SF-1) и ссылка на реестр (SF-2a)
com requisitos concretos de autoria, этапы валидации и модели предложений.
Для канонического примера, очень важно
`docs/source/sorafs/proposals/sorafs_sf1_profile_v1.json`
e o журнал сухого прогона, связанного с ними
`docs/source/sorafs/reports/sf1_determinism.md`.

## Визао гераль

Если вы хотите войти в систему регистрации:

- объявляет детерминированные параметры CDC и конфигурирует мультихеш-идентификаторы между ними.
  архитектуры;
- воспроизводятся дополнительные приспособления (JSON Rust/Go/TS + corpora fuzz + testemunhas PoR), которые
  os SDK нижестоящие possam verificar semtooling sob medida;
- включить метададос быстро для управления (пространство имен, имя, имя) junto com orientacao
  развертывание и операционная деятельность; е
- выберите набор различий, определенных до пересмотра до консультации.

Составьте контрольный список, который поможет подготовить предложение, которое поможет вам подготовиться к этому.

## Резюме регистрации

Перед повторным предложением подтвердите, что она присоединилась к хартии регистрации приложения для
`sorafs_manifest::chunker_registry::ensure_charter_compliance()`:

- Идентификаторы результатов в виде внутренних положительных результатов, которые увеличивают монотонную форму с пробелами.
- Ручка canonico (`namespace.name@semver`) появится в списке псевдонимов и
  **разработайте** первый раз. Альтернативные псевдонимы (например, `sorafs.sf1@1.0.0`) в наличии.
- Nenhum псевдоним может быть использован в качестве внешнего дескриптора canonico или aparecer mais de uma vez.
- Псевдонимы должны быть разнообразными и спортивными устройствами на открытом воздухе.

Помощники CLI:

```bash
# Listagem JSON de todos os descritores registrados (ids, handles, aliases, multihash)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles

# Emitir metadados para um perfil default candidato (handle canonico + aliases)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --promote-profile=sorafs.sf1@1.0.0 --json-out=-
```

Esses comandos mantem as propostas alinhadas com a carta do registro e fornecem os
метададо канонические необходимые для обсуждения вопросов управления.

## Требуемые метададо| Кампо | Описание | Пример (`sorafs.sf1@1.0.0`) |
|-------|-----------|------------------------------|
| `namespace` | Логическое соединение для идеальной связи. | `sorafs` |
| `name` | Rotulo legivel для людей. | `sf1` |
| `semver` | Семантическая версия текста для соединения параметров. | `1.0.0` |
| `profile_id` | Монотонный цифровой идентификатор, указанный при входе или входе. Зарезервируйте или проксимально идите, чтобы повторно использовать множество существующих. | `1` |
| `profile_aliases` | Управляет дополнительными опциями (альтернативными, сокращенными именами), объясняющими клиентам во время переговоров. Включите всегда или обрабатывайте канонику как первый вход. | `["sorafs.sf1@1.0.0"]` |
| `profile.min_size` | Comprimento minimo do chunk em bytes. | `65536` |
| `profile.target_size` | Можно также выполнить фрагментацию байтов. | `262144` |
| `profile.max_size` | Comprimento maximo do chunk em bytes. | `524288` |
| `profile.break_mask` | Тушь для ресниц Adaptativa usada pelo Rolling Hash (шестигранник). | `0x0000ffff` |
| `profile.polynomial` | Константа до полиномиальной шестерни (шестигранной). | `0x3da3358b4dc173` |
| `gear_seed` | Начальное значение используется для получения таблицы передач размером 64 КиБ. | `sorafs-v1-gear` |
| `chunk_multihash.code` | Codigo multihash para дайджест порции. | `0x1f` (BLAKE3-256) |
| `chunk_multihash.digest` | Дайджест комплекта светильников Canonico. | `13fa...c482` |
| `fixtures_root` | Относительное соперничество с восстановленными светильниками. | `fixtures/sorafs_chunker/sorafs.sf1@1.0.0/` |
| `por_seed` | Посевной материал для амострагема PoR детерминистический (`splitmix64`). | `0xfeedbeefcafebabe` (пример) |

Os Metadados Devem Aparecer Tanto no Documento de Proposta Quanto Dentro Dos Fixtures Gerados
для регистрации, инструментов CLI и автоматического подтверждения управления своими значениями
крузаментос мануаис. В случае необходимости выполните CLI фрагмента-хранилища и манифеста com.
`--json-out=-` для передачи расчетных метаданных для примечаний к пересмотру.

### Контактные данные CLI и регистрация

- `sorafs_manifest_chunk_store --profile=<handle>` - повторное выполнение метаданных фрагмента,
  дайджест делает манифест и проверяет PoR, как и предлагаемые параметры.
- `sorafs_manifest_chunk_store --json-out=-` - передача или связь с хранилищем фрагментов для
  стандартный вывод для автоматического сравнения.
- `sorafs_manifest_stub --chunker-profile=<handle>` - подтверждение деклараций и планов CAR
  Вставьте или обработайте большинство псевдонимов Canonico.
- `sorafs_manifest_stub --plan=-` - повторите или `chunk_fetch_specs` передний пункт
  verificar компенсирует/дайджест в зависимости от ситуации.

Зарегистрируйте команду (обрабатывает, вызывает PoR, хеширует манифест) и предлагает ее
os revisores possam воспроизводится буквально.

## Контрольный список детерминизма и достоверности1. **Регенерировать приспособления**
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors \
     --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
   ```
2. **Выполнить набор парададов** — `cargo test -p sorafs_chunker` и дифференциал жгута проводов.
   межъязыковый (`crates/sorafs_chunker/tests/vectors.rs`) devem ficar verdes com os
   Новос светильники нет Лугар.
3. **Reexecutar corpora fuzz/back-pressure** — выполните `cargo fuzz list` и жгут де
   потоковая передача (`fuzz/sorafs_chunker`) против восстановленных ресурсов ОС.
4. **Verificar testemunhas Proof-of-Retrivability** — выполнить
   `sorafs_manifest_chunk_store --por-sample=<n>` использовать или выполнить предложение и подтвердить
   что вызывает корреспонденцию в манифесте светильников.
5. **Пробный запуск CI** — локально выполнить `ci/check_sorafs_fixtures.sh`; о сценарий
   добились успеха в существующих светильниках и `manifest_signatures.json`.
6. **Подтверждение перекрестной среды выполнения** — проверьте, используются ли привязки Go/TS или JSON.
   регенерадо и эмитам ограничивает и переваривает идентификаторы.

Документируйте команды и дайджесты результатов, которые могут быть предоставлены Tooling WG.
reexecuta-los sem adivinhacoes.

### Подтверждение манифеста / PoR

Депо регенерирующих приспособлений, выполнение полного манифеста конвейера для гарантии того, что
Metadados CAR и Provas PoR продолжают соответствовать:

```bash
# Validar metadados de chunk + PoR com o novo perfil
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --profile=sorafs.sf2@1.0.0 \
  --json-out=- --por-json-out=- fixtures/sorafs_chunker/input.bin

# Gerar manifest + CAR e capturar chunk fetch specs
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --chunk-fetch-plan-out=chunk_plan.json \
  --manifest-out=sf2.manifest \
  --car-out=sf2.car \
  --json-out=sf2.report.json

# Reexecutar usando o plano de fetch salvo (evita offsets obsoletos)
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --plan=chunk_plan.json --json-out=-
```

Замена или входной билет для любого представительского корпуса, использующего наши светильники
(например, детерминированный поток размером 1 ГиБ) и приложение, в котором обрабатываются полученные результаты.

## Модель предложения

Как предлагается к субметидам в качестве регистров Norito `ChunkerProfileProposalV1`, зарегистрированных в них
`docs/source/sorafs/proposals/`. Шаблон JSON с иллюстрацией или форматированным текстом
(substitua seus valores conforme necessario):


Forneca um relatorio Markdown корреспондент (`determinism_report`), который захватывает
Сказал дос-командос, переваривает фрагменты и делает их контрадосами во время проверки.

## Fluxo degovanca

1. **Подметр PR с предложением + светильниками.** Включая все имеющиеся активы, предложение
   Norito и настроен на `chunker_registry_data.rs`.
2. **Переработка Tooling WG.** Внесение изменений в контрольный список проверки и подтверждения.
   что предлагается перейти к процедуре регистрации (sem reutilizacao de id, determinismo satisfeito).
3. **Конверт для консульства.** Если вы согласны, члены совета сообщат или дайджест да
   предложение (`blake3("sorafs-chunker-profile-v1" || canonical_bytes)`) и экзамен suas
   assinaturas ao конверт сделать perfil Armazenado junto aos светильники.
4. **Публикация реестра.** Объедините актуализацию реестра, документов и приспособлений. О CLI
   Постоянство по умолчанию не имеет переднего значения, поскольку правительство объявляет о скорой миграции.
5. **Уведомление об утере.** После миграционного периода выполните регистрацию для

## Дикас де автория

- Prefira ограничивает часть мощности, чтобы свести к минимуму удобство разделения на кусочки.
- Эвитируйте мудар или мультихэш-код, который является координатором потребления манифеста и шлюза; включая ум
  Nota Operational Quando Fizer isso.
- Mantenha как семена да таблицы передач, законные для людей, mas globalmente unicas для упрощения аудиторий.
- Армазенские артефакты сравнительного анализа (например, сравнения пропускной способности)
  `docs/source/sorafs/reports/` для будущей ссылки.Для ожидаемой работы во время развертывания проконсультируйтесь с миграционным реестром
(`docs/source/sorafs/migration_ledger.md`). Для изменения соответствия во время выполнения, важно
`docs/source/sorafs/chunker_conformance.md`.