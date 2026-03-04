---
lang: ru
direction: ltr
source: docs/portal/docs/da/ingest-plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note Источник канонический
Reflete `docs/source/da/ingest_plan.md`. Gardez les Deux версии синхронизированы
:::

# План приема данных о доступности Sora Nexus

_Redige: 20 февраля 2026 г. — Ответственные: рабочая группа по базовому протоколу / группа хранения данных / рабочая группа DA_

Рабочий поток DA-2 включает Torii с API для приема больших двоичных объектов, которые они могут использовать.
метадоны Norito и любовь к репликации SoraFS. Ce захват документа
предложить схему, поверхностный API и поток проверки в конечном итоге
l'реализация без блокировки при оставшихся симуляциях (suivi
ДА-1). Все форматы полезной нагрузки DOIVENT используются кодеками Norito; Аукун
Резервный файл/JSON не разрешен.

## Цели

- Приемник объемных капель (сегменты Тайкай, коляски, артефакты).
  управление) детерминированным способом через Torii.
- Создание канонических манифестов Norito, декривантных le blob, les parametres de
  кодек, профиль стирания и политика сохранения.
- Сохранять метаданные фрагментов в горячих хранилищах SoraFS и в других местах.
  файлы файлов заданий репликации.
- Опубликовать намерения де-пина + политические теги в реестре SoraFS и др.
  наблюдатели за управлением.
- Разоблачение квитанций о приеме для того, чтобы клиенты могли вернуться в прежнее состояние.
  детерминирование публикации.

## API поверхности (Torii)

```
POST /v1/da/ingest
Content-Type: application/norito+v1
```

Полезная нагрузка имеет кодировку `DaIngestRequest` и Norito. Использование ответов
`application/norito+v1` и отправьте `DaIngestReceipt`.

| Ответ | Значение |
| --- | --- |
| 202 Принято | Blob в файле для фрагментации/репликации; квитанция отправка. |
| 400 неверный запрос | Нарушение схемы/хвоста (воир проверки валидации). |
| 401 Несанкционированный | Токен API недействителен/недействителен. |
| 409 Конфликт | Doublon `client_blob_id` с неидентичными метаданными. |
| 413 Полезная нагрузка слишком велика | Отмените ограничение на настройку длины BLOB-объекта. |
| 429 Слишком много запросов | Внимание: ограничение скорости. |
| 500 Внутренняя ошибка | Проверьте невнимательность (журнал + оповещение). |

## Схема Norito предлагает

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

> Примечание по реализации: канонические представления Rust для этих полезных нагрузок
> vivent maintenant sous `iroha_data_model::da::types`, с обертками
> запрос/получение в `iroha_data_model::da::ingest` и в структуре
> манифест в `iroha_data_model::da::manifest`.

Чемпион `compression` объявляет комментарий вызывающим абонентам о подготовке полезной нагрузки. Torii
примите `identity`, `gzip`, `deflate` и `zstd` и распакуйте предшествующие байты
хеширование, фрагментирование и проверка опций манифестов.

### Контрольный список проверки1. Проверка того, что заголовок запроса Norito соответствует `DaIngestRequest`.
2. Эхо `total_size` отличается от канонической длины полезной нагрузки
   (распаковать) или отменить максимальную настройку.
3. Принудительное выравнивание `chunk_size` (мощность двух, = 2.
5. `retention_policy.required_replica_count` уважайте базовую линию
   управление.
6. Проверка подписи против канонического хэша (en excluant le champ
   подпись).
7. Отмените дублирование `client_blob_id`, содержащее хэш полезной нагрузки и метаданные.
   Сон тождества.
8. Quand `norito_manifest` est Fourni, схема проверки + корреспондент хеша
   или манифест пересчета после разделения на фрагменты; Sinon le Noeud Genere le Manifest et le
   шток.
9. Примените политику репликации: Torii повторно запишите файл.
   `RetentionPolicy` soumis avec `torii.da_ingest.replication_policy` (voir
   `replication-policy.md`) и отвергайте предварительные конструкции манифестов, не
   Метаданные хранения не соответствуют требованиям профиля.

### Поток фрагментации и репликации

1. Распаковка полезной нагрузки в `chunk_size`, калькулятор BLAKE3 по чанк + раскин Меркла.
2. Создайте Norito `DaManifestV1` (новую структуру), захватывающую взаимодействия.
   фрагмент (role/group_id), макет стирания (comptes de parite de lignes et
   столбцы плюс `ipa_commitment`), политика хранения и метаданные.
3. Меттр в файле байтов канонического манифеста
   `config.da_ingest.manifest_store_dir` (Torii сертификат документа
   Индексы `manifest.encoded` по полосе/эпохе/последовательности/билету/отпечатку пальца) afin
   que l'orchestration SoraFS les ingere et relie le Storage Ticket aux donnees
   упорствует.
4. Опубликуйте намерения с помощью `sorafs_car::PinIntent` с тегом управления.
   и политика.
5. Emettre l'evenement Norito `DaIngestPublished` для уведомления наблюдателей
   (клиенты, управление, аналитика).
6. Отправка звонящего `DaIngestReceipt` (подпись службы DA de Torii)
   и укажите заголовок `Sora-PDP-Commitment` для захвата файла SDK.
   обязательство закодировать немедленно. Квитанция включает в себя техническое обслуживание `rent_quote`
   (un Norito `DaRentQuote`) и `stripe_layout`, постоянные дополнительные отправители
   d'afficher la rente de base, la резерв, бонусные бонусы PDP/PoTR и т. д.
   Макет стирания 2D для хранения билетов перед использованием фондов.

## Неправильный запас/реестр

- Etendre `sorafs_manifest` с `DaManifestV1`, позволяющий анализировать
  детерминированный.
- Добавить новый поток реестра `da.pin_intent` с версией полезной нагрузки.
  референтный хэш манифеста + идентификатор билета.
- Mettre a Jour les Pipelines d'Observabilite для отслеживания задержки приема,
  пропускная способность фрагментации, резерв репликации и компьютеры
  d'echecs.

## Стратегия испытаний- Единые тесты для проверки схемы, проверки подписи, обнаружения
  дублоны.
- Тестирует золотой сертификат кодировки Norito от `DaIngestRequest`, манифест и т. д.
  квитанция.
- Демаркация интеграции SoraFS + модели реестра, действительные файлы
  флюс де кусок + штырь.
- Проверка свойств стирания профилей и комбинаций
  складские помещения.
- Фаззинг полезных нагрузок Norito для защиты от неправильной формы метаданных.

## Инструментарий CLI и SDK (DA-8)- `iroha app da submit` (новая точка входа CLI), конверт, поддерживающий сборщик
  d'ingest partage afin que les operurs puissent ingerer des blobs
  Арбитры hors du flux Пакет Тайкай. La Commande vit Dans
  `crates/iroha_cli/src/commands/da.rs:1` и используйте полезную нагрузку, профиль
  стирание/сохранение и варианты хранения метаданных/предварительного манифеста
  подписавший `DaIngestRequest` canonique с ключом конфигурации CLI. Лес бежит
  reussis упорный `da_request.{norito,json}` и `da_receipt.{norito,json}` су
  `artifacts/da/submission_<timestamp>/` (переопределение через `--artifact-dir`) еще
  les artefacts de Release enregistrent les bytes Norito точно использует кулон
  Я глотаю.
- Команда использует стандартный `client_blob_id = blake3(payload)`, который можно принять.
  для переопределений через `--client-blob-id`, обратите внимание на карты JSON метаданных
  (`--metadata-json`) и предродовые манифесты (`--manifest`) и поддерживаются
  `--no-submit` для подготовки в автономном режиме плюс `--endpoint` для хостов
  Torii персонализируется. Квитанция JSON est imprime sur stdout en plus d'etre
  Запись на диске, новый инструментарий "submit_blob" для DA-8 и др.
  разблокировать тяжелую работу по парите SDK.
- `iroha app da get` добавляет псевдоним DA для оркестратора с несколькими источниками питания
  дежа `iroha app sorafs fetch`. Возможности операторов: указатель на артефакты
  манифест + план фрагмента (`--manifest`, `--plan`, `--manifest-id`) **ou** Fournir
  билет хранения Torii через `--storage-ticket`. Билет на Quand le chemin est
  используйте CLI, чтобы восстановить манифест от `/v1/da/manifests/<ticket>`,
  persiste le Bundle sous `artifacts/da/fetch_<timestamp>/` (переопределить avec
  `--manifest-cache-dir`), получить хэш двоичного объекта для `--manifest-id`, puis
  выполнить оркестратор со списком `--gateway-provider` Fournie. Все ле
  ручки avances du fetcher SoraFS остаются неповрежденными (конверты манифеста, этикетки
  клиент, защита кешей, отмена анонимного транспорта, табло экспорта и т. д.
  пути `--output`), и за манифест конечной точки может взиматься дополнительная плата через
  `--manifest-endpoint` для хостов Torii персонализирует, выполняет проверки
  сквозная доступность vivent entierement sous le namespace `da` без
  дубликат логики оркестратора.
- `iroha app da get-blob` восстановить канонические манифесты в указанном направлении Torii
  через `GET /v1/da/manifests/{storage_ticket}`. La Commande ecrit
  `manifest_{ticket}.norito`, `manifest_{ticket}.json` и др.
  `chunk_plan_{ticket}.json` sous `artifacts/da/fetch_<timestamp>/` (или
  `--output-dir` Fourni Par l'Utilisateur) tout en affichant la Commande Точное управление
  `iroha app da get` (включая `--manifest-id`) требуется для оркестратора выборки.
  Будьте осторожны с операторами за пределами репертуара, катушками деклараций и гарантиями.
  что сборщик использует все артефакты, обозначенные параметром Torii. Клиент
  Torii JavaScript воспроизводит поток через
  `ToriiClient.getDaManifest(storageTicketHex)`, отозвать байты Norito
  декодирует, манифест JSON и план фрагментов, которые будут содержать SDK вызывающих абонентов
  сеансы оркестратора без прохода через CLI. Le SDK Swift раскрывает
  обслуживание поверхностей мемов (`ToriiClient.getDaManifestBundle(...)` plus`fetchDaPayloadViaGateway(...)`), разветвленные пакеты Sur le Wrapper Natif
  оркестратор SoraFS для мощного телезарядного устройства для клиентов iOS
  манифесты, исполнитель выборки из нескольких источников и захватчик без ограничений
  вызов CLI.
  [IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240][IrohaSwift/Sources/IrohaSwift/SorafsOrchestratorClient.swift:12]
- `iroha app da rent-quote` расчет арендной платы и вентиляции
  стимулы для хранения и хранения четырех предметов.
  Помощник по работе с активным `DaRentPolicyV1` (JSON или байтами Norito) или файлом
  Интеграция по умолчанию, проверка политики и подтверждение резюме в формате JSON (`gib`,
  `months`, политические метаданные, поля `DaRentQuote`) для аудиторов
  Гражданин взимает плату XOR в течение нескольких минут управления без использования скриптов
  хок. Команда emet aussi un резюме на прямой линии `rent_quote ...` перед началом работы
  полезная нагрузка JSON pour garder les logs консоль lisibles подвеска les сверла
  инцидент. Ассоциация `--quote-out artifacts/da/rent_quotes/<stamp>.json` с
  `--policy-label "governance ticket #..."` для сохранения старых артефактов
  citant le голосование или пакет конфигурации точный; CLI активирует персональную метку
  и откажитесь от цепочек видений, которые являются ценностями `policy_source` restent
  Actionnables в информационных панелях Tresorerie. Вуар
  `crates/iroha_cli/src/commands/da.rs` для помощника и др.
  `docs/source/da/rent_policy.md` для политической схемы.
  [crates/iroha_cli/src/commands/da.rs:1][docs/source/da/rent_policy.md:1]
- `iroha app da prove-availability` цепочка, которая предшествует: il prend un Storage
  билет, телезарядка канонического манифеста, казнь оркестратора
  множественный источник (`iroha app sorafs fetch`) против списка `--gateway-provider`
  Фурни, сохраняй телезаряд полезной нагрузки + су-табло
  `artifacts/da/prove_availability_<timestamp>/` и немедленно вызовите файл
  Существующий помощник PoR (`iroha app da prove`) с восстанавливаемыми байтами. Лес операторы
  регулировка ручек оркестра (`--max-peers`, `--scoreboard-out`,
  переопределяет манифест конечной точки) и образец доказательства (`--sample-count`,
  `--leaf-index`, `--sample-seed`) и какие команды производятся
  артефакты, сопровождающие аудиты DA-5/DA-9: копия полезной нагрузки, доказательства
  табло и резюме в формате JSON.