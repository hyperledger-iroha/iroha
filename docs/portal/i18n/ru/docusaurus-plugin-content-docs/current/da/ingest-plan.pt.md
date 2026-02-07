---
lang: ru
direction: ltr
source: docs/portal/docs/da/ingest-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::примечание Fonte canonica
Эспелья `docs/source/da/ingest_plan.md`. Мантенья как дуа-версоэс
:::

# План приема данных о доступности от Sora Nexus

_Регистрация: 20 февраля 2026 г. - Ответы: Рабочая группа по базовому протоколу / Группа хранения / DA WG_

Рабочий поток DA-2 находится в Torii с API-интерфейсом приема больших двоичных объектов, которые излучаются.
метаданные Norito и семья с копией SoraFS. Этот документ захватывает или показывает
кстати, интерфейс API и поток валидации для реализации
заранее заблокируйте симулированные изображения (сегменты DA-1). Все ОС
форматы полезной нагрузки DEVEM, использующие кодеки Norito; запасные варианты разрешения nao sao
сердце/JSON.

## Объективос

- Aceitar blobs grandes (сегменты Тайкай, коляски де Лейн, Артефатос де
  govanca) детерминированной формы через Torii.
- Производитель манифестирует Norito canonicos, который определяет blob, параметры кодека,
  ошибка стирания и политика сохранения.
- Сохранять метаданные фрагментов, не хранящихся в горячем виде SoraFS, и загружать задания
  реплика.
- Публикация намерений + теги политики без регистрации SoraFS и наблюдателей
  де Губернанка.
- Экспортируйте квитанции о приеме, чтобы клиенты могли восстановиться, проверив детерминированность.
  де publicacao.

## API-интерфейс Superficie (Torii)

```
POST /v1/da/ingest
Content-Type: application/norito+v1
```

Полезная нагрузка имеет номер `DaIngestRequest`, кодированный в Norito. Как ответит нам
`application/norito+v1` и повторное имя `DaIngestReceipt`.

| Ответ | Значение |
| --- | --- |
| 202 Принято | Загрузка больших двоичных объектов для фрагментации/репликации; квитанция реторнадо. |
| 400 неверный запрос | Violacao de esquema/tamanho (veja validacoes). |
| 401 Несанкционированный | API токенов отключен/недействителен. |
| 409 Конфликт | Дубликат `client_blob_id` с разными метаданными. |
| 413 Полезная нагрузка слишком велика | Превышайте ограничения по конфигурации отдельных больших двоичных объектов. |
| 429 Слишком много запросов | Ограничение скорости установлено. |
| 500 Внутренняя ошибка | Falha inesperada (журнал + оповещение). |

## Esquema Norito пропосто

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

> Примечание о реализации: как канонические изображения в Rust для полезных нагрузок
> Agora vivem em `iroha_data_model::da::types`, com обертки запроса/получения
> em `iroha_data_model::da::ingest` и описание манифеста em
> `iroha_data_model::da::manifest`.

В поле `compression` информация о подготовке вызывающих абонентов или полезной нагрузки. Torii ацеита
`identity`, `gzip`, `deflate`, и `zstd`, расшифровав байты до хэша,
разделение на части и проверка возможных манифестов.

### Контрольный список проверки подлинности1. Убедитесь, что заголовок Norito соответствует требуемому `DaIngestRequest`.
2. Falhar se `total_size` отличается от стандартного canonico do payload (descomprimido)
   или превосходите максимальную конфигурацию.
3. Подключите `chunk_size` (мощность, = 2.
5. `retention_policy.required_replica_count` должен сохранить базовый уровень
   губернатора.
6. Проверка подлинности против канонического хеша (за исключением подписи или подписи).
7. Повторите дубликат `client_blob_id`, а также хешируйте полезную нагрузку и метаданные.
   формальные идентификаторы.
8. Quando `norito_manifest` для fornecido, схема проверки + хеш совпадают, com
   o манифестировать перерасчет после разделения на фрагменты; случай наоборот или узел Гера или манифест
   э о армазена.
9. Для настройки политики репликации: Torii повторно создайте
   `RetentionPolicy` отправлен с `torii.da_ingest.replication_policy` (veja
   `replication-policy.md`) e rejeita манифестирует заранее подготовленные метаданные
   retencao nao совпало с perfil imposto.

### Поток фрагментации и репликации

1. Выполните полезную нагрузку в `chunk_size`, вычислите BLAKE3 для фрагмента + Raiz Merkle.
2. Создайте Norito `DaManifestV1` (struct nova) захватив компромиссные решения.
   фрагмент (роль/group_id), макет стирания (контагены де-паридаде-де-линхас и
   больше столбцов `ipa_commitment`), политика сохранения и метаданных.
3. Зачисление байтов действительно является каноническим рыданием
   `config.da_ingest.manifest_store_dir` (Torii хранить архивы
   `manifest.encoded` по полосе/эпохе/последовательности/билету/отпечатку пальца) для того, чтобы
   orquestracao SoraFS, чтобы получить билет и билет на хранение в дадо
   персисидос.
4. Опубликуйте намерения закрепления через `sorafs_car::PinIntent` с помощью тега управления и электронной почты.
   политика.
5. Отправьте событие Norito `DaIngestPublished` для уведомления наблюдателей.
   (клиенты левес, губернатора, аналитика).
6. Возврат `DaIngestReceipt` к вызывающему абоненту (уведомление об обслуживании DA de
   Torii) и излучатель или заголовок `Sora-PDP-Commitment` для захвата ОС SDK
   обязательство, кодифицированное немедленно. Квитанция агоры включает `rent_quote`
   (um Norito `DaRentQuote`) и `stripe_layout`, разрешите, что можно удалить
   exibam — база аренды, резерв, ожидаемые бонусы PDP/PoTR и макет
   стирание 2D или использование билета на хранение перед компрометром.

## Настройка хранилища/регистрации

- Estender `sorafs_manifest` com `DaManifestV1`, опытный разбор
  детерминированный.
- Новый поток реестра `da.pin_intent` с версией полезной нагрузки.
  хеш-код манифеста + идентификатор билета.
- Настроить трубопроводы наблюдения для задержки задержки приема,
  пропускная способность фрагментации, отставание от репликации и количество загрязнений.

## Стратегия яичек- Унитарные тесты для проверки схемы, проверки подлинности, обнаружения
  дубликаты.
- Тестирует золотую верификацию кодировки Norito от `DaIngestRequest`, манифест e.
  квитанция.
- Интегрированный подпрограммный макет SoraFS + реестр, действительные потоки
  кусок + булавка.
- Тесты свойств, сочетающие возможность стирания и комбинирования удержания
  алеатории.
- Фаззинг полезных данных Norito для защиты от некорректных метаданных.

## Инструментарий CLI и SDK (DA-8)- `iroha app da submit` (новая точка входа CLI) уже включает в себя сборщика/издателя
  Совместное использование для того, чтобы операторы могли использовать произвольные капли
  пучок fora do fluxo de Taikai. О comando vive em
  `crates/iroha_cli/src/commands/da.rs:1` и потребление полезной нагрузки, выполнение
  стирание/сохранение и сохранение опций метаданных/манифест перед удалением
  `DaIngestRequest` canonico с функцией настройки CLI. Execucoes bem-sucedidas
  persistem `da_request.{norito,json}` и `da_receipt.{norito,json}` рыдание
  `artifacts/da/submission_<timestamp>/` (переопределить через `--artifact-dir`) для того, что
  Артефатос де-регистрации освобождения байтов Norito exatos usados durante a
  проглатывание.
- Команда США по-падрао `client_blob_id = blake3(payload)` mas aceita
  переопределяет через `--client-blob-id`, отображает JSON метаданных (`--metadata-json`)
  Манифесты предварительной подготовки (`--manifest`), поддержка `--no-submit` для подготовки
  в автономном режиме больше `--endpoint` для персонализированных хостов Torii. О квитанция JSON e
  impresso no stdout alem de ser escrito no disco, fechando or requisito de
  Инструментарий «submit_blob» для DA-8 и удаления или работы с SDK.
- `iroha app da get` добавление псевдонима в DA для размещения нескольких источников
  что я ем `iroha app sorafs fetch`. Operadores podem apontar para artefatos
  манифест + план фрагмента (`--manifest`, `--plan`, `--manifest-id`) **ou**
  passar um билет хранения Torii через `--storage-ticket`. Когда ты делаешь это
  билет и использовать CLI или манифест `/v1/da/manifests/<ticket>`,
  сохраняться в пакете рыданий `artifacts/da/fetch_<timestamp>/` (переопределить com
  `--manifest-cache-dir`), производное или хеш-объект для `--manifest-id`, и в целом
  выполнить или заказать в списке `--gateway-provider` fornecida. Все ОС
  ручки avancados do fetcher SoraFS permanecem неповрежденные (конверты манифеста,
  метки клиентов, защита кэшей, переопределение анонимной транспортировки, экспорт
  табло и пути `--output`), и конечная точка манифеста, который будет записан
  через `--manifest-endpoint` для хостов Torii персонализированные настройки, включая проверки ОС
  Сквозная доступность в полной мере в пространстве имен `da` с дубликатом
  logica do orquestrador.
- `iroha app da get-blob` baixa манифестирует канонические ссылки Torii через
  `GET /v1/da/manifests/{storage_ticket}`. О comando escreve
  `manifest_{ticket}.norito`, `manifest_{ticket}.json` е
  `chunk_plan_{ticket}.json` рыдание `artifacts/da/fetch_<timestamp>/` (оу эм
  `--output-dir` fornecido pelo usuario) enquanto imprime o comando exato de
  `iroha app da get` (включая `--manifest-id`), необходимый для извлечения данных
  оркестратор. Это работа для катушек манифестов и
  Гарантия, что сборщик всегда будет использовать артефатос, испускаемый по Torii. О
  cliente Torii JavaScript используется или меняется через Fluxo
  `ToriiClient.getDaManifest(storageTicketHex)`, возврат байтов Norito
  декодированные, манифест JSON и план фрагментов для гибких вызывающих абонентов SDK
  Сеансы оркестратора выполняются через CLI. O SDK Swift агора разоблачает как месмы
  поверхности (`ToriiClient.getDaManifestBundle(...)` больше
  `fetchDaPayloadViaGateway(...)`), подключаемые пакеты для собственной оболочкиorquestrador SoraFS для клиентов iOS, использующих манифесты, executar
  извлекает данные из нескольких источников и захватывает их с помощью CLI.
  [IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240][IrohaSwift/Sources/IrohaSwift/SorafsOrchestratorClient.swift:12]
- `iroha app da rent-quote` расчет арендной платы детерминированный и детальный расчет
  стимулы для хранения и хранения денег. О помощник
  используйте активный формат `DaRentPolicyV1` (JSON или байты Norito) или встроенный по умолчанию,
  Проверка политики и подтверждение резюме в формате JSON (`gib`, `months`, метаданные
  политика и кампании `DaRentQuote`) для того, чтобы аудиторы предъявили обвинения XOR exatas
  em atas degovanca sem сценарии ad hoc. О comando tambem emite um resumo em
  uma linha `rent_quote ...` перед выполнением JSON полезной нагрузки для журналов консоли
  legiveis во время учений по инциденту. Эмпарелье
  `--quote-out artifacts/da/rent_quotes/<stamp>.json` ком
  `--policy-label "governance ticket #..."`, чтобы сохранить артефакты, которые
  ситем или голосование или пакет конфигурации exatos; Корта CLI или персонализированная метка
  строки recusa vazias para que valores `policy_source` permanecam acionaveis
  нет информационных панелей de tesouraria. Вежа
  `crates/iroha_cli/src/commands/da.rs` для подкоманды e
  `docs/source/da/rent_policy.md` для политической схемы.
  [crates/iroha_cli/src/commands/da.rs:1][docs/source/da/rent_policy.md:1]
- `iroha app da prove-availability` encadeia tudo acima: получить билет на хранение,
  байша или пакет канонического манифеста, исполняемый или оркестратор с несколькими источниками
  (`iroha app sorafs fetch`) против списка `--gateway-provider` fornecida, сохраняться
  o полезная нагрузка baixado + табло sob `artifacts/da/prove_availability_<timestamp>/`,
  Немедленный вызов существующего помощника PoR (`iroha app da prove`) с использованием ОС
  байты байшадос. Операции с возможностью регулировки ручек управления
  (`--max-peers`, `--scoreboard-out`, переопределяет конечную точку манифеста) e o
  образец доказательства (`--sample-count`, `--leaf-index`, `--sample-seed`) enquanto um
  единая команда, производящая артефатос, успешно использованная в аудиториях DA-5/DA-9: копия сделать
  полезная нагрузка, доказательства табло и резюме проверки в формате JSON.