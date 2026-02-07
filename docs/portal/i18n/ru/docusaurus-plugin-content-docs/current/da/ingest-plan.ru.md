---
lang: ru
direction: ltr
source: docs/portal/docs/da/ingest-plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note Канонический источник
На этой странице отражено `docs/source/da/ingest_plan.md`. Держите обе версии
:::

# План приема данных о доступности Sora Nexus

_Черновик: 20 февраля 2026 г. — Владельцы: Core Protocol WG / Storage Team / DA WG_

Рабочий поток DA-2 расширяет Torii прием API для больших двоичных объектов, который выпускает
Norito-метаданные и запускают репликацию SoraFS. В документе описывается предлагаемая
схема, API-область и поток валидации, чтобы реализация шла без блокировки
ожидающих симуляций (последующие DA-1). Все форматы полезной нагрузки ДОЛЖНЫиспользовать
кодеки Norito; резервный вариант на сервере/JSON не указывает.

## Цели

- Принимать большие капли (сегменты Taikai, коляска, артефакты управления)
  детерминированно через Torii.
- Создавать канонические манифесты Norito, описывающие blob, параметры кодека,
  Удаление профиля и сохранение политики.
- Сохранять метаданные чанки в горячем хранилище SoraFS и поставить задачи репликации в
  очередь.
- Публиковать намерения закрепления + теги политики в реестре SoraFS и наблюдателях.
  управления.
- Выдавать квитанции о приеме, чтобы клиенты обратились к определенному
  подтверждение публикации.

## Поверхность API (Torii)

```
POST /v1/da/ingest
Content-Type: application/norito+v1
```

Полезная нагрузка — это `DaIngestRequest`, закодированный Norito. Ответы использовать
`application/norito+v1` и возвращают `DaIngestReceipt`.

| Ответ | Значение |
| --- | --- |
| 202 Принято | Blob поставлен в очередь на фрагментирование/репликацию; чек возвращен. |
| 400 неверный запрос | Нарушение схемы/размера (см. проверки). |
| 401 Несанкционированный | Отсутствует/некорректен API-токен. |
| 409 Конфликт | Дубликат `client_blob_id` с несовпадающей метаданной. |
| 413 Полезная нагрузка слишком велика | Превышен предел длины большого двоичного объекта. |
| 429 Слишком много запросов | Превышен лимит скорости. |
| 500 Внутренняя ошибка | Неожиданная ошибка (журнал + оповещение). |

## Предлагаемая схема Norito

```rust
/// Top-level ingest request.
pub struct DaIngestRequest {
    pub client_blob_id: BlobDigest,      // submitter-chosen identifier
    pub lane_id: LaneId,                 // target Nexus lane
    pub epoch: u64,                      // epoch blob belongs to
    pub sequence: u64,                   // monotonic sequence per (lane, epoch)
    pub blob_class: BlobClass,           // TaikaiSegment, GovernanceArtifact, etc.
    pub codec: BlobCodec,                // e.g. "cmaf", "pdf", "norito-batch"
    pub erasure_profile: ErasureProfile, // parity configuration
    pub retention_policy: RetentionPolicy,
    pub chunk_size: u32,                 // bytes (must align with profile)
    pub total_size: u64,
    pub compression: Compression,        // Identity, gzip, deflate, or zstd
    pub norito_manifest: Option<Vec<u8>>, // optional pre-built manifest
    pub payload: Vec<u8>,                 // raw blob data (<= configured limit)
    pub metadata: ExtraMetadata,          // optional key/value metadata map
    pub submitter: PublicKey,             // signing key of caller
    pub signature: Signature,             // canonical signature over request
}

pub enum BlobClass {
    TaikaiSegment,
    NexusLaneSidecar,
    GovernanceArtifact,
    Custom(u16),
}

pub struct ErasureProfile {
    pub data_shards: u16,
    pub parity_shards: u16,
    pub chunk_alignment: u16, // chunks per availability slice
    pub fec_scheme: FecScheme,
}

pub struct RetentionPolicy {
    pub hot_retention_secs: u64,
    pub cold_retention_secs: u64,
    pub required_replicas: u16,
    pub storage_class: StorageClass,
    pub governance_tag: GovernanceTag,
}

pub struct ExtraMetadata {
    pub items: Vec<MetadataEntry>,
}

pub struct MetadataEntry {
    pub key: String,
    pub value: Vec<u8>,
    pub visibility: MetadataVisibility, // public vs governance-only
}

pub enum MetadataVisibility {
    Public,
    GovernanceOnly,
}

pub struct DaIngestReceipt {
    pub client_blob_id: BlobDigest,
    pub lane_id: LaneId,
    pub epoch: u64,
    pub blob_hash: BlobDigest,          // BLAKE3 of raw payload
    pub chunk_root: BlobDigest,         // Merkle root after chunking
    pub manifest_hash: BlobDigest,      // Norito manifest hash
    pub storage_ticket: StorageTicketId,
    pub pdp_commitment: Option<Vec<u8>>,     // Norito-encoded PDP bytes
    #[norito(default)]
    pub stripe_layout: DaStripeLayout,   // total_stripes, shards_per_stripe, row_parity_stripes
    pub queued_at_unix: u64,
    #[norito(default)]
    pub rent_quote: DaRentQuote,        // XOR rent + incentives derived from policy
    pub operator_signature: Signature,
}
```

> Примечание по реализации: стандартные Rust-представления этой полезной нагрузки теперь
> обнаружить в `iroha_data_model::da::types`, с обертками запроса/получения в
> `iroha_data_model::da::ingest` и структурной манифест в
> `iroha_data_model::da::manifest`.

Поле `compression` описывает, как представляет полезную нагрузку вызывающего абонента. Torii принимает
`identity`, `gzip`, `deflate` и `zstd`, распаковывая байты перед хешированием,
разделение и проверка опциональных манифестов.

### Чеклист валидации1. Убедитесь, что заголовок Norito соответствует `DaIngestRequest`.
2. Ошибка, если `total_size` отличается от канонической длины полезной нагрузки.
   (после распаковки) или увеличить максимум настроения.
3. Принудить спортивные `chunk_size` (степень двойки, = 2.
5. `retention_policy.required_replica_count` должен соблюдать базовые показатели управления.
6. Проверка подключения по условному хэшу (без поля подключения).
7. Отклонять дубликаты `client_blob_id`, если хэш-полезная нагрузка и метаданные не
   что.
8. В наличии `norito_manifest` проверка схемы + хеш на совпадение с
   манифест, пересчитанным после разделения на фрагменты; иначе узел внешнего манифеста и
   сохранить его.
9. Применять настроенную политику репликации: Torii переписывает отправленный
   `RetentionPolicy` через `torii.da_ingest.replication_policy` (см.
   `replication-policy.md`) и отклоняет заранее созданные манифесты, если их
   метаданные сохранения не соответствуют принудительному профилю.

### Поток фрагментации и репликации

1. Разбить полезную нагрузку на `chunk_size`, вычислить BLAKE3 для каждого чанка + Merkle
   корень.
2. Сформировать Norito `DaManifestV1` (новая структура), фиксируя фрагмент обязательства
   (роль/group_id), макет стирания (число строк паритета и столбцов плюс
   `ipa_commitment`), политика сохранения и метаданные.
3. Поставить канонические байты манифеста в свою очередь под
   `config.da_ingest.manifest_store_dir` (Torii пишет `manifest.encoded` по
   дорожка/эпоха/последовательность/билет/отпечаток пальца), чтобы оркестрация SoraFS могла
   поглотить их и связать билет хранилища с сохраненными данными.
4. Закрепить намерения через `sorafs_car::PinIntent` с тегом управления и Публикации.
   политика.
5. Эмитировать событие Norito `DaIngestPublished` для оповещений наблюдателей.
   (легкие клиенты, управление, аналитика).
6. Возвращает вызывающую сторону `DaIngestReceipt` (подписан ключом Torii DA) и отправляет
   заголовок `Sora-PDP-Commitment`, чтобы SDK сразу получил обязательства. Квитанция
   теперь включает `rent_quote` (Norito `DaRentQuote`) и `stripe_layout`, чтобы
   отправители могли показать базовую аренду, долю резерва, ожидания бонусов
   PDP/PoTR и средства 2D-стирания рядом с билетом хранения до фиксации.

## Обновления Хранилище/Реестр

- Расширить `sorafs_manifest` новым `DaManifestV1`, обеспечив определённый
  парсинг.
- Добавить новый поток реестра `da.pin_intent` с версионированными полезными данными,
  который ссылается на хеш манифеста + идентификатор билета.
- Обновить наблюдаемость-пайплайны для задержки приема данных Диптихов, пропускной способности.
  фрагментирование, репликационное отставание и счетчик ошибок.

## Стратегия тестирования- Юнит-тесты для валидации схемы, проверки подписей, обнаружения дубликатов.
- Golden-тесты для проверки Norito кодировки `DaIngestRequest`, манифеста и квитанции.
- Интеграционный жгут, поднимающий макет SoraFS + реестр и проверяющий
  потоки чанк + пин.
- Property-тесты для стирания случайных профилей и сохранения комбинаций.
- Фаззинг полезной нагрузки Norito для защиты от некорректной методики.

## Инструментарий CLI и SDK (DA-8)- `iroha app da submit` (новая точка входа CLI) оборачивает общий сборщик ingest/
  издатель, чтобы операторы могли проглотить произвольные капли вне потока
  Пакет Тайкай. Команда находится в `crates/iroha_cli/src/commands/da.rs:1` и
  принимает полезную нагрузку, стирание/сохранение профиля и опциональные файлы
  метаданные/манифест перед подписью канонического `DaIngestRequest` ключом
  конфигурация CLI. Успешные запуски сохраняют `da_request.{norito,json}` и
  `da_receipt.{norito,json}` под `artifacts/da/submission_<timestamp>/`
  (переопределить через `--artifact-dir`), чтобы релизные артефакты фиксировались точные
  Norito байт, использованные при приеме.
- Команда по умолчанию использует `client_blob_id = blake3(payload)`, но
  поддержка переопределяет через `--client-blob-id`, метаданные карты JSON
  (`--metadata-json`) и предварительно созданные манифесты (`--manifest`), а также
  `--no-submit` для оффлайн подготовки и `--endpoint` для кастомных хостов Torii.
  Квитанция JSON выводится в стандартный вывод и записывается на диск, закрывая требование DA-8.
  "submit_blob" и разблокируя работу SDK по четности.
- `iroha app da get` добавлен DA-ориентированный псевдоним для оркестратора с несколькими источниками,
  который уже питает `iroha app sorafs fetch`. Операторы могут указывать на артефакты
  манифест + план фрагмента (`--manifest`, `--plan`, `--manifest-id`) **или** передать
  Билет хранения Torii через `--storage-ticket`. При использовании CLI билета
  загружает манифест из `/v1/da/manifests/<ticket>`, сохраняет пакет в
  `artifacts/da/fetch_<timestamp>/` (переопределить с `--manifest-cache-dir`), выводит
  хеш больших двоичных объектов для `--manifest-id` и запускает оркестратор с заданным значением
  `--gateway-provider` списком. Все продвинутые ручки из сборщика SoraFS
  информации (конверты манифеста, метки клиентов, защитные кэши, анонимные
  переопределения транспорта, табло экспорта, путь `--output`), конечная точка манифеста
  можно переопределить через `--manifest-endpoint` для кастомных Torii хостов,
  так что сквозные проверки доступности живут в пространстве имен `da` без
  дублирование логики оркестратора.
- `iroha app da get-blob` забирает канонические манифесты напрямую из Torii через
  `GET /v1/da/manifests/{storage_ticket}`. Команда пишет
  `manifest_{ticket}.norito`, `manifest_{ticket}.json` и `chunk_plan_{ticket}.json`
  в `artifacts/da/fetch_<timestamp>/` (или пользовательский `--output-dir`), при
  эта выводит точную команду `iroha app da get` (`--manifest-id`), включая необходимые варианты.
  для последующей выборки оркестратора. Это освобождает операторов от работы с
  каталоги спула манифеста и гарантия того, что сборщик всегда использует
  подписанные артефакты Torii. Клиент JavaScript Torii повторяет этот поток через поток
  `ToriiClient.getDaManifest(storageTicketHex)`, возвращающиеся декодированные Norito
  байты, манифест JSON и план фрагментов, чтобы вызывающие SDK могли поднять
  сессия оркестратора без CLI. Swift SDK теперь обеспечивает ту же самую поверхность
  (`ToriiClient.getDaManifestBundle(...)` и `fetchDaPayloadViaGateway(...)`),
  прокидывание бандла в родную обертку SoraFS оркестратора, чтобы клиенты iOS могли
  скачивать манифесты, выполнять выборку из нескольких источников и собирать доказательства без
  вызов CLI.[IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240][IrohaSwift/Sources/IrohaSwift/SorafsOrchestratorClient.swift:12]
- `iroha app da rent-quote` высчитывает детерминированную арендную плату и льготу по разбору
  Хранение заданного размера и сохранение окон. Хелпер потребляет активную
  `DaRentPolicyV1` (JSON или Norito байт) либо встроенный по умолчанию, валидирует
  политика и печатает JSON-сводку (`gib`, `months`, метаданные политики и поля
  `DaRentQuote`), чтобы аудиторы могли процитировать точные расходы XOR в протоколах
  управления без специальных скриптов. Команда также печатает однострочный
  `rent_quote ...` перед полезной нагрузкой JSON для читабельности журналов во время тренировок.
  Свяжите `--quote-out artifacts/da/rent_quotes/<stamp>.json` с
  `--policy-label "governance ticket #..."`, чтобы сохранить аккуратные артефакты
  с точной ссылкой на голосование или комплект конфигурации; CLI обрезает пользовательскую
  метку и отвергает пустые строки, чтобы `policy_source` были стабильными и пригодными для
  дашбордов. См. `crates/iroha_cli/src/commands/da.rs` для подкоманд и
  `docs/source/da/rent_policy.md` для схемы политики.
  [crates/iroha_cli/src/commands/da.rs:1][docs/source/da/rent_policy.md:1]
- `iroha app da prove-availability` объединяет все выше: берет билет на хранение,
  загружает стандартный пакет манифеста, запускает оркестратор с несколькими источниками
  (`iroha app sorafs fetch`) против списка `--gateway-provider`, сохранить
  скачанный пэйлоад + табло в `artifacts/da/prove_availability_<timestamp>/`,
  и сразу возникает существующий помощник PoR (`iroha app da prove`) полученными
  байты. Операторы могут настраивать ручки оркестратора (`--max-peers`,
  `--scoreboard-out`, переопределения конечной точки манифеста) и образец доказательства
  (`--sample-count`, `--leaf-index`, `--sample-seed`), при этом одна команда
  выпускает артефакты, требуемые аудиты DA-5/DA-9: полезная нагрузка, доказательство
  табло и доказательство JSON-резюме.