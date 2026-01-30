---
lang: ru
direction: ltr
source: docs/portal/docs/da/ingest-plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note Канонический источник
Эта страница отражает `docs/source/da/ingest_plan.md`. Держите обе версии
:::

# План ingest Data Availability Sora Nexus

_Черновик: 2026-02-20 — Владельцы: Core Protocol WG / Storage Team / DA WG_

Рабочий поток DA-2 расширяет Torii API ingest для blob, который выпускает
Norito-метаданные и запускает репликацию SoraFS. Документ описывает предложенную
схему, API-область и поток валидации, чтобы реализация шла без блокировки на
ожидающих симуляциях (follow-up DA-1). Все форматы payload ДОЛЖНЫ использовать
кодеки Norito; fallback на serde/JSON не допускается.

## Цели

- Принимать большие blob (сегменты Taikai, sidecar lane, артефакты управления)
  детерминированно через Torii.
- Создавать канонические Norito manifests, описывающие blob, параметры codec,
  профиль erasure и политику retention.
- Сохранять метаданные chunk в hot storage SoraFS и ставить задачи репликации в
  очередь.
- Публиковать pin intents + policy tags в реестр SoraFS и наблюдателям
  управления.
- Выдавать admission receipts, чтобы клиенты получали детерминированное
  подтверждение публикации.

## API Surface (Torii)

```
POST /v1/da/ingest
Content-Type: application/norito+v1
```

Payload — это `DaIngestRequest`, закодированный Norito. Ответы используют
`application/norito+v1` и возвращают `DaIngestReceipt`.

| Ответ | Значение |
| --- | --- |
| 202 Accepted | Blob поставлен в очередь на chunking/replication; receipt возвращен. |
| 400 Bad Request | Нарушение schema/размера (см. проверки). |
| 401 Unauthorized | Отсутствует/некорректен API-токен. |
| 409 Conflict | Дубликат `client_blob_id` с несовпадающей метаданной. |
| 413 Payload Too Large | Превышен лимит длины blob. |
| 429 Too Many Requests | Превышен rate limit. |
| 500 Internal Error | Неожиданная ошибка (лог + алерт). |

## Предлагаемая Norito схема

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

> Примечание по реализации: канонические Rust-представления этих payload теперь
> находятся в `iroha_data_model::da::types`, с request/receipt wrappers в
> `iroha_data_model::da::ingest` и структурой manifest в
> `iroha_data_model::da::manifest`.

Поле `compression` описывает, как caller подготовил payload. Torii принимает
`identity`, `gzip`, `deflate` и `zstd`, распаковывая байты перед hashing,
chunking и проверкой опциональных manifests.

### Чеклист валидации

1. Проверить, что заголовок Norito соответствует `DaIngestRequest`.
2. Ошибка, если `total_size` отличается от канонической длины payload
   (после распаковки) или превышает настроенный максимум.
3. Принудить выравнивание `chunk_size` (степень двойки, <= 2 MiB).
4. Убедиться, что `data_shards + parity_shards` <= глобального максимума и
   parity >= 2.
5. `retention_policy.required_replica_count` должен соблюдать governance baseline.
6. Проверка подписи по каноническому hash (без поля подписи).
7. Отклонять дубликаты `client_blob_id`, если hash payload и метаданные не
   идентичны.
8. При наличии `norito_manifest` проверить schema + hash на совпадение с
   manifest, пересчитанным после chunking; иначе узел генерирует manifest и
   сохраняет его.
9. Применять настроенную политику репликации: Torii переписывает отправленный
   `RetentionPolicy` через `torii.da_ingest.replication_policy` (см.
   `replication-policy.md`) и отклоняет заранее созданные manifests, если их
   метаданные retention не совпадают с enforced профилем.

### Поток chunking и репликации

1. Разбить payload на `chunk_size`, вычислить BLAKE3 для каждого chunk + Merkle
   root.
2. Сформировать Norito `DaManifestV1` (новая struct), фиксируя commitment chunk
   (role/group_id), erasure layout (числа паритета строк и столбцов плюс
   `ipa_commitment`), политику retention и метаданные.
3. Поставить канонические bytes manifest в очередь под
   `config.da_ingest.manifest_store_dir` (Torii пишет `manifest.encoded` по
   lane/epoch/sequence/ticket/fingerprint), чтобы оркестрация SoraFS могла
   поглотить их и связать storage ticket с сохраненными данными.
4. Публиковать pin intents через `sorafs_car::PinIntent` с тегом управления и
   политикой.
5. Эмитировать событие Norito `DaIngestPublished` для оповещения наблюдателей
   (light clients, governance, analytics).
6. Возвращать `DaIngestReceipt` caller (подписан ключом Torii DA) и отправлять
   заголовок `Sora-PDP-Commitment`, чтобы SDK сразу получили commitment. Receipt
   теперь включает `rent_quote` (Norito `DaRentQuote`) и `stripe_layout`, чтобы
   отправители могли показывать базовую аренду, долю резерва, ожидания бонусов
   PDP/PoTR и 2D erasure layout рядом со storage ticket до фиксации средств.

## Обновления Storage / Registry

- Расширить `sorafs_manifest` новым `DaManifestV1`, обеспечив детерминированный
  парсинг.
- Добавить новый registry stream `da.pin_intent` с версионированным payload,
  который ссылается на manifest hash + ticket id.
- Обновить observability-пайплайны для мониторинга задержки ingest, throughput
  chunking, репликационного backlog и счетчиков ошибок.

## Стратегия тестирования

- Unit-тесты для валидации schema, проверки подписей, детекта дубликатов.
- Golden-тесты для проверки Norito encoding `DaIngestRequest`, manifest и receipt.
- Интеграционный harness, поднимающий mock SoraFS + registry и проверяющий
  потоки chunk + pin.
- Property-тесты для случайных профилей erasure и комбинаций retention.
- Fuzzing Norito payload для защиты от некорректной метадаты.

## CLI и SDK tooling (DA-8)

- `iroha app da submit` (новый CLI entrypoint) оборачивает общий ingest builder/
  publisher, чтобы операторы могли ingest-ить произвольные blobs вне потока
  Taikai bundle. Команда находится в `crates/iroha_cli/src/commands/da.rs:1` и
  принимает payload, профиль erasure/retention и опциональные файлы
  metadata/manifest перед подписью канонического `DaIngestRequest` ключом
  конфигурации CLI. Успешные запуски сохраняют `da_request.{norito,json}` и
  `da_receipt.{norito,json}` под `artifacts/da/submission_<timestamp>/`
  (override через `--artifact-dir`), чтобы релизные artefacts фиксировали точные
  Norito bytes, использованные при ingest.
- Команда по умолчанию использует `client_blob_id = blake3(payload)`, но
  поддерживает overrides через `--client-blob-id`, JSON карты metadata
  (`--metadata-json`) и pre-generated manifests (`--manifest`), а также
  `--no-submit` для офлайн подготовки и `--endpoint` для кастомных Torii хостов.
  Receipt JSON выводится в stdout и пишется на диск, закрывая требование DA-8
  "submit_blob" и разблокируя SDK parity работу.
- `iroha app da get` добавляет DA-ориентированный alias для multi-source orchestrator,
  который уже питает `iroha app sorafs fetch`. Операторы могут указать artefacts
  manifest + chunk-plan (`--manifest`, `--plan`, `--manifest-id`) **или** передать
  Torii storage ticket через `--storage-ticket`. При использовании ticket CLI
  загружает manifest из `/v1/da/manifests/<ticket>`, сохраняет bundle в
  `artifacts/da/fetch_<timestamp>/` (override с `--manifest-cache-dir`), выводит
  blob hash для `--manifest-id` и запускает orchestrator с заданным
  `--gateway-provider` списком. Все продвинутые knobs из SoraFS fetcher
  сохраняются (manifest envelopes, client labels, guard caches, anonymous
  transport overrides, экспорт scoreboard, `--output` пути), а manifest endpoint
  можно переопределить через `--manifest-endpoint` для кастомных Torii хостов,
  так что end-to-end availability проверки живут в namespace `da` без
  дублирования orchestrator логики.
- `iroha app da get-blob` забирает канонические manifests напрямую из Torii через
  `GET /v1/da/manifests/{storage_ticket}`. Команда пишет
  `manifest_{ticket}.norito`, `manifest_{ticket}.json` и `chunk_plan_{ticket}.json`
  в `artifacts/da/fetch_<timestamp>/` (или пользовательский `--output-dir`), при
  этом выводит точную команду `iroha app da get` (включая `--manifest-id`), нужную
  для последующего orchestrator fetch. Это избавляет операторов от работы с
  manifest spool директориями и гарантирует, что fetcher всегда использует
  подписанные artefacts Torii. JavaScript клиент Torii повторяет этот поток через
  `ToriiClient.getDaManifest(storageTicketHex)`, возвращая декодированные Norito
  bytes, manifest JSON и chunk plan, чтобы SDK callers могли поднимать
  orchestrator сессии без CLI. Swift SDK теперь предоставляет те же поверхности
  (`ToriiClient.getDaManifestBundle(...)` и `fetchDaPayloadViaGateway(...)`),
  прокидывая bundle в native wrapper SoraFS orchestrator, чтобы iOS клиенты могли
  скачивать manifests, выполнять multi-source fetch и собирать доказательства без
  вызова CLI.
  [IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240][IrohaSwift/Sources/IrohaSwift/SorafsOrchestratorClient.swift:12]
- `iroha app da rent-quote` вычисляет детерминированную rent и разбор incentive для
  заданного размера storage и окна retention. Хелпер потребляет активную
  `DaRentPolicyV1` (JSON или Norito bytes) либо встроенный default, валидирует
  политику и печатает JSON-сводку (`gib`, `months`, policy metadata и поля
  `DaRentQuote`), чтобы аудиторы могли цитировать точные XOR charges в протоколах
  управления без ad hoc скриптов. Команда также печатает однострочный
  `rent_quote ...` перед JSON payload для читабельности логов во время drills.
  Свяжите `--quote-out artifacts/da/rent_quotes/<stamp>.json` с
  `--policy-label "governance ticket #..."`, чтобы сохранить аккуратные artefacts
  с точной ссылкой на голосование или config bundle; CLI обрезает пользовательскую
  метку и отвергает пустые строки, чтобы `policy_source` оставался пригодным для
  дашбордов. См. `crates/iroha_cli/src/commands/da.rs` для подкоманды и
  `docs/source/da/rent_policy.md` для схемы политики.
  [crates/iroha_cli/src/commands/da.rs:1][docs/source/da/rent_policy.md:1]
- `iroha app da prove-availability` объединяет все выше: берет storage ticket,
  скачивает канонический manifest bundle, запускает multi-source orchestrator
  (`iroha app sorafs fetch`) против списка `--gateway-provider`, сохраняет
  скачанный payload + scoreboard в `artifacts/da/prove_availability_<timestamp>/`,
  и сразу вызывает существующий PoR helper (`iroha app da prove`) с полученными
  bytes. Операторы могут настраивать orchestrator knobs (`--max-peers`,
  `--scoreboard-out`, manifest endpoint overrides) и proof sampler
  (`--sample-count`, `--leaf-index`, `--sample-seed`), при этом одна команда
  выпускает artefacts, требуемые аудитами DA-5/DA-9: копию payload, доказательство
  scoreboard и JSON-резюме proof.
