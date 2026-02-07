---
lang: ru
direction: ltr
source: docs/portal/docs/da/ingest-plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::обратите внимание на Фуэнте каноника
Рефлея `docs/source/da/ingest_plan.md`. Mantenga ambas versiones ru
:::

# План приема данных о доступности данных Nexus

_Отредактировано: 20 февраля 2026 г. — Ответственный: Рабочая группа по базовому протоколу / Группа хранения данных / Рабочая группа по DA_

Рабочий поток DA-2 расширен Torii с помощью API приема эмитируемых больших двоичных объектов
метаданные Norito и симбра репликации SoraFS. Этот документ захвачен
это необходимо, поверхность API и поток проверки для того, чтобы
реализация заранее без блокировки для симуляций (seguimientos
ДА-1). Все форматы полезной нагрузки DEBEN с использованием кодеков Norito; нет разрешения
резервные файлы serde/JSON.

## Объективос

- Aceptar blobs grandes (сегменты Тайкай, коляски де Лейн, артефакты де
  gobernanza) детерминированной формы через Torii.
- Создатель манифеста Norito canonicos, который описывает большой двоичный объект, параметры
  кодек, защита от стирания и политика сохранения.
- Сохранять метаданные блоков в горячем хранилище SoraFS и записывать задания
  репликация.
- Публикация намерений + теги политического реестра SoraFS и наблюдателей
  де Гобернанса.
- Выставочные отзывы о приеме для того, чтобы клиенты могли восстановиться, определившись с выбором.
  де публикация.

## API-интерфейс Superficie (Torii)

```
POST /v1/da/ingest
Content-Type: application/norito+v1
```

Полезная нагрузка соответствует `DaIngestRequest`, закодированному в Norito. Лас-ответас в США
`application/norito+v1` и усовершенствованный `DaIngestReceipt`.

| Ответ | Значение |
| --- | --- |
| 202 Принято | Blob en cola для фрагментации/репликации; se deuelve el recibo. |
| 400 неверный запрос | Нарушение esquema/tamano (проверка достоверности). |
| 401 Несанкционированный | API токенов отключен/недействителен. |
| 409 Конфликт | Дубликат `client_blob_id` с несовпадающими метаданными. |
| 413 Полезная нагрузка слишком велика | Превысьте ограничение по длине объекта. |
| 429 Слишком много запросов | Установите ограничение скорости. |
| 500 Внутренняя ошибка | Fallo inesperado (журнал + оповещение). |

## Esquema Norito пропуесто

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

> Примечание о реализации: канонические представления в Rust для этого пункта
> полезные нагрузки сейчас вивен bajo `iroha_data_model::da::types`, с обертками
> запрос/получение в `iroha_data_model::da::ingest` и структуре манифеста
> en `iroha_data_model::da::manifest`.

Кампо `compression` объявляет, как вызывающие абоненты готовят полезную нагрузку. Torii
принять `identity`, `gzip`, `deflate`, y `zstd`, извлечь байты до этого
hashear, фрагментация и проверка дополнительных манифестов.

### Контрольный список проверки1. Убедитесь, что заголовок Norito запроса совпадает с `DaIngestRequest`.
2. Если `total_size` отличается от большого канонического значения полезной нагрузки (распространено)
   o превосходить максимальную конфигурацию.
3. Создайте подключение `chunk_size` (мощность, = 2.
5. `retention_policy.required_replica_count` должен соответствовать базовой линии
   гобернанса.
6. Проверка фирмы против канонического хэша (исключая подпись el Campo).
7. Повторное копирование `client_blob_id`, хеш-код полезной нагрузки и ла
   метаданные Шона Идентикоса.
8. Когда вы подтвердите `norito_manifest`, проверьте проверку + хеш-код совпадает с эл.
   манифест перерасчета через фрагментирование; de lo contrario el nodogenera el
   манифест и ло альмасена.
9. Для настройки политики репликации: Torii перепишите адрес
   `RetentionPolicy` отправлен с `torii.da_ingest.replication_policy` (версия
   `replication-policy.md`) и rechaza отображает предварительно созданные метаданные
   Сохранение не совпадает с невыполненным заданием.

### Flujo де фрагментации и репликации

1. Найдите полезную нагрузку в `chunk_size`, вычислите BLAKE3 для фрагмента + Raiz Merkle.
2. Создайте Norito `DaManifestV1` (struct nueva), захватив компромиссные решения.
   чанк (роль/group_id), макет стирания (conteos de paridad de filas y
   Columnas mas `ipa_commitment`), политика сохранения и метаданных.
3. Укажите байты манифеста canonico bajo.
   `config.da_ingest.manifest_store_dir` (Torii описать архивы
   `manifest.encoded` по полосе/эпохе/последовательности/билету/отпечатку пальца) для того, что я
   orquestacion de SoraFS лос-инжера и билет на хранение в хранилище с данными
   персисидос.
4. Опубликуйте намерения закрепления через `sorafs_car::PinIntent` с тегом gobernanza y.
   политика.
5. Отправьте событие Norito `DaIngestPublished` для уведомления наблюдателей (клиентов).
   ligeros, gobernanza, analitica).
6. Devolver `DaIngestReceipt` для звонящего (фирма для обслуживания DA де
   Torii) и заголовок `Sora-PDP-Commitment` для захвата SDK
   обязательство, кодифицированное непосредственно. Эль ресибо сейчас включает `rent_quote`
   (un Norito `DaRentQuote`) и `stripe_layout`, разрешите денежные средства
   Мострар базовая аренда, резерв, бонусные ожидания PDP/PoTR и другие
   Макет стирания 2D вместе со всеми билетами для хранения до компрометации фондов.

## Актуализация хранилища/реестра

- Удлинитель `sorafs_manifest` с `DaManifestV1`, опытный анализатор
  детерминистский.
- Добавлен новый поток реестра `da.pin_intent` с версией полезной нагрузки, которую
  хэш манифеста ссылки + идентификатор билета.
- Актуализация трубопроводов наблюдения для обеспечения задержки приема пищи,
  пропускная способность фрагментации, отставание от репликации и потоки данных.

## Эстратегия де Прюбас- Унитарные тесты для проверки результатов, проверки фирм и обнаружения
  дубликаты.
- Тестирует золотую проверку кодировки Norito от `DaIngestRequest`, манифест y
  квитанция.
- Левантандский жгут интеграции SoraFS + имитация реестра, проверка
  flujos de chunk + булавка.
- Тесты свойств куба на возможность стирания и комбинаций
  алеатории удержания.
- Фаззинг полезных данных Norito для защиты от некорректных метаданных.

## Инструментарий CLI и SDK (DA-8)- `iroha app da submit` (новая точка входа в CLI) сейчас связывает разработчика/издателя
  отсек для приема, который позволяет использовать произвольные капли
  комплект fuera del flujo Taikai. El comando vive en
  `crates/iroha_cli/src/commands/da.rs:1` потребляет полезную нагрузку, выполняет фильтрацию
  стирание/сохранение и дополнительное архивирование метаданных/манифест перед фирмой
  el `DaIngestRequest` canonico с ключом конфигурации CLI. Выбросы
  exitosas persisten `da_request.{norito,json}` и `da_receipt.{norito,json}` бахо
  `artifacts/da/submission_<timestamp>/` (переопределить через `--artifact-dir`) для того, что
  артефакты выпуска зарегистрированы в байтах Norito точно используются в течение всего времени
  проглатывание.
- Командир США из-за дефекта `client_blob_id = blake3(payload)` полностью принят
  переопределения через `--client-blob-id`, отображение метаданных в формате JSON
  (`--metadata-json`) y манифестирует предварительно созданные (`--manifest`), y soporta
  `--no-submit` для подготовки к работе в автономном режиме `--endpoint` для хостов Torii
  персонализированные. Квитанция JSON отображается в стандартном формате для написания
  дискотека, возьмите необходимые инструменты "submit_blob" DA-8 и разблокируйте их.
  эль-работа де-де-SDK.
- `iroha app da get` объединяет псевдонимы в DA для нескольких источников
  какая у тебя сила `iroha app sorafs fetch`. Лос-операторы могут apuntarlo a
  артефакты манифеста + план фрагмента (`--manifest`, `--plan`, `--manifest-id`)
  **o** передать билет хранения Torii через `--storage-ticket`. Где бы вы ни находились, США
  путь к билету, CLI Баха-эль-манифест от `/v1/da/manifests/<ticket>`,
  сохраняться в пакете bajo `artifacts/da/fetch_<timestamp>/` (переопределить
  `--manifest-cache-dir`), извлеките хэш большого двоичного объекта для `--manifest-id`, и добавьте
  извлеките оркеста из списка `--gateway-provider` suministrada. Тодос
  ручки для извлечения SoraFS постоянные неповрежденные (манифест
  конверты, клиентские этикетки, охранные тайники, блокировка анонимной транспортировки,
  экспортировать табло и пути `--output`), и конечную точку манифеста можно
  запишите через `--manifest-endpoint` для персонализированных хостов Torii, asi
  какие проверки сквозной доступности выполняются в каждом пространстве имен
  `da` не может дублировать логику орвестора.
- `iroha app da get-blob` baja манифестирует canonicos Directo от Torii через
  `GET /v1/da/manifests/{storage_ticket}`. Эль-командо описать
  `manifest_{ticket}.norito`, `manifest_{ticket}.json` у
  `chunk_plan_{ticket}.json` bajo `artifacts/da/fetch_<timestamp>/` (o un
  `--output-dir` provisto por el usuario) mientras imprime el comando точно де
  `iroha app da get` (включая `--manifest-id`) требуется для извлечения данных
  Оркестер. Это означает, что операторы работают с катушкой директорий.
  заявляет и гарантирует, что сборщик всегда будет использовать твердые артефакты
  излучатели по Torii. Клиент Torii JavaScript отображает сообщение об ошибке через
  `ToriiClient.getDaManifest(storageTicketHex)`, передача байтов Norito
  декодированные, манифест JSON и план фрагментов для скрытия вызывающих абонентов SDK
  сеансы организатора без использования CLI. El SDK de Swift сейчас покажет
  ошибочные поверхности (`ToriiClient.getDaManifestBundle(...)` mas`fetchDaPayloadViaGateway(...)`), пакетная обработка исходной оболочки
  orquestador SoraFS для клиентов iOS, которые могут загрузить манифесты и извлечь их
  извлекает данные из нескольких источников и захватывает их без вызова CLI.
  [IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240][IrohaSwift/Sources/IrohaSwift/SorafsOrchestratorClient.swift:12]
- `iroha app da rent-quote` расчет арендной платы по определенным критериям и определение стимулов
  для хранения и вентиляции сумок. Эль помощник
  использовать активный `DaRentPolicyV1` (JSON или байты Norito) или интегрированный по умолчанию,
  проверить политику и зафиксировать возобновление JSON (`gib`, `months`, метаданные
  политика и кампании `DaRentQuote`) для того, чтобы аудиторы цитировали грузы XOR точно
  en actas de gobernanza греховные сценарии ad hoc. Эль-командо также возобновил работу
  в строке `rent_quote ...` перед полезной нагрузкой JSON, чтобы сохранить разборчивость лос
  журналы консоли во время тренировок по происшествиям. Эмпаредже
  `--quote-out artifacts/da/rent_quotes/<stamp>.json` кон
  `--policy-label "governance ticket #..."`, чтобы сохранить хорошие артефакты
  проголосуйте за комплект точной конфигурации; CLI записывает персонализированную метку
  и rechaza strings vacios para que los valores `policy_source` se mantengan
  аксессуары и информационные панели tesoreria. Вер
  `crates/iroha_cli/src/commands/da.rs` для подкоманды y
  `docs/source/da/rent_policy.md` для политической школы.
  [crates/iroha_cli/src/commands/da.rs:1][docs/source/da/rent_policy.md:1]
- `iroha app da prove-availability` encadena todo lo anterior: тома ип хранения
  билет, выдача канонического манифеста, выдача оркестадора
  несколько источников (`iroha app sorafs fetch`) против списка `--gateway-provider`
  Суминистрада, сохранение полезной нагрузки выгруженной + нижняя часть табло
  `artifacts/da/prove_availability_<timestamp>/`, и немедленный вызов помощника
  PoR существует (`iroha app da prove`) с использованием потерянных байтов. Лос-операторы
  можно настроить ручки управления (`--max-peers`, `--scoreboard-out`,
  переопределяет конечную точку манифеста) и образец доказательства (`--sample-count`,
  `--leaf-index`, `--sample-seed`)
  Исследованные артефакты для залов DA-5/DA-9: копия полезной нагрузки, доказательства
  табло и резюме в формате JSON.