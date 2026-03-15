---
lang: ru
direction: ltr
source: docs/portal/docs/da/ingest-plan.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::примечание
Используйте синхронизацию для синхронизации
:::

# Sora Nexus План приема данных о доступности

Дата: 20 февраля 2026 г. -- Информация: Рабочая группа по базовому протоколу / Группа хранения / DA WG_

DA-2 используется для Torii для приема больших двоичных объектов API для Norito для Norito
جاری کرتی ہے اور SoraFS ریپلیکیشن کو семя کرتی ہے۔ یہ دستاویز مجوزہ схема,
Поверхность API, поток проверки и особенности реализации.
симуляции (последующие наблюдения DA-1) Форматы полезной нагрузки
Кодеки Norito Резервный вариант serde/JSON

## اہداف

- بڑے blobs (сегменты Taikai, коляски, артефакты управления) کو Torii کے
  ذریعے детерминированно قبول کرنا۔
- blob, параметры кодека, профиль стирания, политика хранения и т. д.
  Канонический Norito манифестирует تیار کرنا۔
- Метаданные фрагмента — SoraFS — горячее хранилище для выполнения заданий репликации
  поставить в очередь
- закрепить намерения + теги политики, реестр SoraFS и наблюдатели управления.
  опубликовать
- квитанции о приеме для клиентов и детерминированные доказательства
  публикация مل سکے۔

## Поверхность API (Torii)

```
POST /v2/da/ingest
Content-Type: application/norito+v1
```

Полезная нагрузка Norito в кодировке `DaIngestRequest` ہے۔ Отзывы `application/norito+v1`
Дополнительная информация `DaIngestReceipt`

| Ответ | مطلب |
| --- | --- |
| 202 Принято | Большие двоичные объекты, фрагментирование/репликация, очередь и т. д. квитанция |
| 400 неверный запрос | Нарушение схемы/размера (проверка проверки دیکھیں)۔ |
| 401 Несанкционированный | Токен API موجود نہیں/غلط۔ |
| 409 Конфликт | `client_blob_id` ڈپلیکیٹ ہے اور метаданных مختلف ہے۔ |
| 413 Полезная нагрузка слишком велика | Настроен предел длины большого двоичного объекта. |
| 429 Слишком много запросов | Достигнут предел ставки۔ |
| 500 Внутренняя ошибка | Ошибка сбоя (журнал + оповещение)۔ |

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

> Примечание по реализации: полезные нагрузки и канонические представления Rust.
> `iroha_data_model::da::types` کے تحت ہیں، обертки запросов/получений
> `iroha_data_model::da::ingest` — структура манифеста
> `iroha_data_model::da::manifest` میں ہے۔

Поле `compression` Отображение вызывающих абонентов и полезной нагрузки Обмен данными Torii
`identity`, `gzip`, `deflate`, `zstd` для хеширования, фрагментации.
дополнительные манифесты проверяют количество байтов и распаковывают их

### Контрольный список проверки1. Проверьте соответствие запроса Norito заголовку `DaIngestRequest` и совпадению с ним.
2. Длина канонической (распакованной) полезной нагрузки `total_size` سے مختلف ہو یا
   настроен макс. سے زیادہ ہو تو неудачно
3. Выравнивание `chunk_size` обеспечивает کریں (степень двойки, = 2 یقینی بنائیں۔
5. `retention_policy.required_replica_count` Уважение базового уровня управления
6. Канонический хеш для проверки подписи (поле подписи کے بغیر)۔
7. `client_blob_id` дубликаты отклоняют, если хеш полезной нагрузки + метаданные идентичны.
8. `norito_manifest` позволяет использовать схему + хеш для разбиения на фрагменты.
   пересчитанный манифест سے соответствует کریں؛ Манифест узла генерирует или сохраняет файл
9. Настроенная политика репликации обеспечивает принудительное выполнение: Torii `RetentionPolicy`.
   `torii.da_ingest.replication_policy` سے переписать текст ( `replication-policy.md`
   Также можно использовать предварительно созданные манифесты или отклонять метаданные хранения.
   принудительный профиль سے match نہ کرے۔

### Блокирование и процесс репликации

1. Полезная нагрузка `chunk_size` в виде фрагмента BLAKE3 или корня Merkle
2. Norito `DaManifestV1` (структура) для выполнения обязательств по чану (role/group_id).
   макет стирания (счет четности строк/столбцов + `ipa_commitment`), политика хранения,
   Метаданные для захвата данных
3. Канонические байты манифеста — `config.da_ingest.manifest_store_dir` — в очереди.
   (Torii `manifest.encoded` файлы дорожка/эпоха/последовательность/тикет/отпечаток пальца могут быть удалены в течение нескольких минут)
   SoraFS оркестровка, прием данных, билет хранилища, постоянные данные и ссылка.
4. `sorafs_car::PinIntent` — тег управления + политика — вы можете закрепить намерения опубликовать —
5. Событие Norito `DaIngestPublished` генерирует множество наблюдателей (легкие клиенты, управление, аналитика).
   کو اطلاع ملے۔
6. `DaIngestReceipt` вызывает абонента (Torii служебный ключ DA подписан).
   Заголовок `Sora-PDP-Commitment` испускает SDK и захват фиксации обязательств
   Квитанция `rent_quote` (Norito `DaRentQuote`) или `stripe_layout` شامل ہیں،
   Базовая арендная плата отправителей, резервная доля, бонусные ожидания PDP/PoTR и 2D
   макет стирания کو билет на хранение کے ساتھ средства фиксации کرنے سے پہلے دکھا سکتے ہیں۔

## Обновления хранилища/реестра

- `sorafs_manifest` или `DaManifestV1`, чтобы расширить возможности детерминированного синтаксического анализа.
- Поток реестра `da.pin_intent` может содержать версионную полезную нагрузку.
  Хэш манифеста + идентификатор билета или ссылка
- Конвейеры наблюдения: задержка приема, разделение пропускной способности, отставание в репликации.
  Неудача засчитывается.

## Стратегия тестирования- Проверка схемы, проверка подписей, обнаружение дубликатов и модульные тесты.
- Norito кодирует золотые тесты (`DaIngestRequest`, манифест, квитанция)۔
- Интеграционный жгут с макетом SoraFS + реестр, блокировка блоков + потоки выводов, проверка правильности.
- Тесты свойств, профили случайного стирания и комбинации хранения охватывают все возможности.
- Norito фаззинг полезной нагрузки и некорректные метаданные.

## Инструменты CLI и SDK (DA-8)- `iroha app da submit` (точка входа CLI) для общего сборщика/издателя вставки, а также для создания обертки.
  Операторы Пакетный поток Taikai کے باہر Прием произвольных больших двоичных объектов کر سکیں۔ یہ کمانڈ
  `crates/iroha_cli/src/commands/da.rs:1` — дополнительная полезная нагрузка, профиль стирания/сохранения (необязательно).
  Файлы метаданных/манифеста Ключ конфигурации CLI или канонический знак `DaIngestRequest` کرتی ہے۔
  کامیاب запускает `da_request.{norito,json}` или `da_receipt.{norito,json}` کو
  `artifacts/da/submission_<timestamp>/` может быть заблокирован (переопределить через `--artifact-dir`)
  выпуск артефактов прием میں استعمال ہونے والے точная запись Norito байтов کریں۔
- По умолчанию используется `client_blob_id = blake3(payload)`, а также переопределения `--client-blob-id`.
  Карты метаданных JSON (`--metadata-json`) и предварительно созданные манифесты (`--manifest`)
  `--no-submit` (подготовка в автономном режиме) или `--endpoint` (пользовательские хосты Torii) Квитанция JSON
  stdout и print ہوتا ہے На диске может быть установлен запрос на DA-8 для требования "submit_blob" پورا ہوتا ہے
  Разблокировка SDK по четности ہوتا ہے۔
- `iroha app da get` псевдоним, ориентированный на DA.
  `iroha app sorafs fetch` کو چلاتا ہے۔ Манифест операторов + артефакты плана фрагментов (`--manifest`, `--plan`, `--manifest-id`)
  **یا** Torii билет хранилища через `--storage-ticket` دے سکتے ہیں۔ Путь к билету в CLI `/v2/da/manifests/<ticket>` سے
  загрузка манифеста کرتی ہے، Bundle کو `artifacts/da/fetch_<timestamp>/` میں محفوظ کرتی ہے (переопределить через
  `--manifest-cache-dir`), `--manifest-id` کیلئے получение хэша больших двоичных объектов для получения списка `--gateway-provider`
  Запуск оркестратора کرتی ہے۔ SoraFS сборщик расширенных ручек برقرار رہتے ہیں (конверты манифеста,
  метки клиентов, защитные кэши, переопределение анонимного транспорта, экспорт табло, пути `--output`).
  `--manifest-endpoint` Переопределение конечной точки манифеста для сквозной проверки доступности
  Пространство имен `da` или дубликат логики оркестратора.
- `iroha app da get-blob` Torii سے `GET /v2/da/manifests/{storage_ticket}` کے ذریعے канонические манифесты کھینچتا ہے۔
  Например, `manifest_{ticket}.norito`, `manifest_{ticket}.json`, `chunk_plan_{ticket}.json`.
  `artifacts/da/fetch_<timestamp>/` — дополнительная информация (предоставляется пользователем `--output-dir`) `iroha app da get`
  вызов (بشمول `--manifest-id`) echo کرتی ہے جو последующий оркестратор fetch کیلئے درکار ہے۔ اس سے операторы
  Каталоги спулинга манифеста سے دور رہتے ور fetcher ہمیشہ Torii کے подписанные артефакты
  JavaScript Torii клиентский поток `ToriiClient.getDaManifest(storageTicketHex)` декодирован
  Norito байт, манифест JSON и план фрагментов, а также вызовы SDK CLI и гидрат сеансов оркестратора
  کر سکیں۔ Swift SDK обеспечивает доступ к поверхностям с открытым исходным кодом (`ToriiClient.getDaManifestBundle(...)` اور
  `fetchDaPayloadViaGateway(...)`), пакеты, встроенная оболочка оркестратора SoraFS, канал и клиенты iOS.
  Загрузка манифестов Возможность выборки из нескольких источников Возможность захвата доказательств Возможность использования CLI[IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240][IrohaSwift/Sources/IrohaSwift/SorafsOrchestratorClient.swift:12]
- `iroha app da rent-quote` Размер хранилища и окно удержания, детерминированная арендная плата и разбивка стимулов
  вычислить کرتا ہے۔ یہ активный помощник `DaRentPolicyV1` (JSON یا Norito байт) یا встроенный стандарт по умолчанию.
  проверка политики کرتا ہے، اور JSON-сводка (`gib`, `months`, метаданные политики, поля `DaRentQuote`) print کرتا ہے
  Протоколы управления аудиторов, точные обвинения XOR, цитаты, специальные сценарии, специальные сценарии. Использование полезной нагрузки JSON
  سے پہلے ایک لائن `rent_quote ...` summary بھی دیتی ہے تاکہ консольные журналы, доступные для чтения رہیں۔
  `--quote-out artifacts/da/rent_quotes/<stamp>.json` или `--policy-label "governance ticket #..."`, как это сделать
  جوڑیں تاکہ pretified artefacts محفوظ ہوں جو درست policy voice یا config Bundle cite کریں؛ Пользовательская метка CLI и обрезка
  Если вы хотите отклонить строки, вы можете использовать `policy_source`. Панель управления может быть действенной. دیکھیں
  `crates/iroha_cli/src/commands/da.rs` (подкоманда) или `docs/source/da/rent_policy.md` (схема политики)۔
  [crates/iroha_cli/src/commands/da.rs:1][docs/source/da/rent_policy.md:1]
- `iroha app da prove-availability` Для создания цепи цепочки: билет хранилища для канонического пакета манифеста.
  загрузить оркестратор с несколькими источниками (`iroha app sorafs fetch`) и список `--gateway-provider`.
  Загрузите загруженную полезную нагрузку + табло `artifacts/da/prove_availability_<timestamp>/`.
  Воспользуйтесь помощником PoR (`iroha app da prove`) для извлечения байтов и вызова функции کرتا ہے۔ Ручки оркестратора операторов
  (`--max-peers`, `--scoreboard-out`, переопределения конечной точки манифеста)
  `--sample-seed`) отрегулируйте параметры проверки команд DA-5/DA-9 и проверьте наличие артефактов:
  копия полезной нагрузки, данные табло, сводные данные доказательств в формате JSON.