---
lang: ru
direction: ltr
source: docs/portal/docs/da/ingest-plan.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::примечание
Код `docs/source/da/ingest_plan.md`. Он сказал, что он Сэнсэй и Сэнсэй.
الوثائق القديمة.
:::

# خطة ingest لتوفر البيانات في Sora Nexus

Дата: 20 февраля 2026 г. — Дата: Рабочая группа по базовому протоколу / Группа хранения / DA WG_

Для создания DA-2 используется Torii для приема больших двоичных объектов Norito
Добавлено SoraFS. Для этого необходимо использовать API-интерфейс.
Он был создан Дейном Аном в рамках проекта DA-1. В 2013 году он был отправлен в США.
Дополнительная информация Norito; Для резервного копирования используется файл serde/JSON.

## الاهداف

- قبول blobs كبيرة (قطاعات Taikai, коляски للحارات, وادوات حوكمة)
  عبر Torii.
- Демонстрирует Norito для удаления больших двоичных объектов с помощью кодека и стирания.
  وسياسة الاحتفاظ.
- Вы можете использовать фрагменты в تخزين SoraFS, чтобы получить информацию о них.
  الطابور.
- Закрепите булавку + заглушку для SoraFS в защитном чехле.
- Получите квитанции, которые можно получить в ближайшее время.

## Открытие API (Torii)

```
POST /v2/da/ingest
Content-Type: application/norito+v1
```

Установите `DaIngestRequest` на Norito. تستخدم الاستجابات
`application/norito+v1` или `DaIngestReceipt`.

| Новости | المعنى |
| --- | --- |
| 202 Принято | В разделе "BLOB-объект" в разделе "Обзор/обзор" تم ارجاع квитанция. |
| 400 неверный запрос | Схема انتهاك/الحجم (انظر فحوصات التحقق). |
| 401 Несанкционированный | Доступ к API/интерфейсу. |
| 409 Конфликт | تكرار `client_blob_id` на сайте производителя. |
| 413 Полезная нагрузка слишком велика | Он сказал, что это капля. |
| 429 Слишком много запросов | Установлено ограничение скорости. |
| 500 Внутренняя ошибка | فشل غير متوقع (журнал + تنبيه). |

## مخطط Norito المقترح

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

> Добавлено: Там вы увидите Rust,
> `iroha_data_model::da::types`, обертки/квитанция.
> `iroha_data_model::da::ingest` отображается в манифесте `iroha_data_model::da::manifest`.

Он был создан `compression` в режиме онлайн. например Torii `identity` и `gzip`
و`deflate` و`zstd` для хеширования и проверки манифестов.
الاختيارية.

### قائمة تحقق التحقق

1. Установите флажок Norito для установки `DaIngestRequest`.
2. افشل اذا كان `total_size` مختلفا عن طول الحمولة القانوني (بعد فك الضغط) او
   يتجاوز الحد الاقصى المكون.
3. Загрузите `chunk_size` (размер файла = 2.
5. Установите флажок `retention_policy.required_replica_count` и установите флажок.
6. تحقق التوقيع مقابل الهاش القياسي (مع استبعاد حقل التوقيع).
7. رفض `client_blob_id`, который находится в центре города, где находится центральный офис.
   متطابقة.
8. Создайте `norito_manifest`, добавьте схему + хеш-манифест.
   حسابه بعد التجزئة؛ В 2007 году он объявил о своем несогласии.
9. Установите флажок: Torii или `RetentionPolicy`.
   `torii.da_ingest.replication_policy` (راجع `replication-policy.md`)
   проявляет المجهزة مسبقا اذا لم تطابق بيانات الاحتفاظ الملف المفروض.

### تدفق التجزئة والتكرار1. Загрузите файл `chunk_size`, а также BLAKE3 в блоке + جذر Merkle.
2. Создайте Norito `DaManifestV1` (структура) для создания фрагмента
   (role/group_id) и удаление (اعداد تكافؤ الصفوف والاعمدة مع)
   `ipa_commitment`) и установите флажок.
3. Загрузите байты манифеста в файл `config.da_ingest.manifest_store_dir`.
   (от Torii до `manifest.encoded` — полоса/эпоха/последовательность/тикет/отпечаток пальца)
   حتى تتمكن منظوومة SoraFS в ابتلاعها وربط Storage Ticket بالبيانات المحفوظة.
4. Закрепите штырь `sorafs_car::PinIntent` на защитном кожухе.
5. بث حدث Norito `DaIngestPublished` لاخطار المراقبين (عملاء خفيفين, الحوكمة).
   التحليلات).
6. Установите `DaIngestReceipt` в исходное состояние (недоступно для Torii DA).
   `Sora-PDP-Commitment` для использования SDK. يتضمن الـ квитанция
   `rent_quote` (Norito `DaRentQuote`) и `stripe_layout`.
   Информационные технологии, разработанные для обеспечения безопасности PDP/PoTR
   وتخطيط стирание, а также билет на хранение в хранилище.

## تحديثات التخزين/السجل

- توسيع `sorafs_manifest` بـ `DaManifestV1` для выполнения синтаксического анализа.
- Запустите поток `da.pin_intent` для создания хеш-кода.
  манифест + идентификатор билета.
- Обеспечивает контроль за приемом данных и пропускную способность, а также загрузку данных.
  التكرار, وعدد الاخفاقات.

## استراتيجية الاختبارات

- اختبارات وحدة للتحقق من Schema, وفحوصات التوقيع, وكشف التكرار.
- Золотой золотистый цвет Norito для `DaIngestRequest` в манифесте и квитанции.
- жгут проводов تكامل يشغل SoraFS + реестр, установленный на блоке + контакт.
- Чтобы удалить данные, выполните следующие действия.
- Фаззинг لحمولات Norito для метаданных.

## Интерфейс CLI и SDK (DA-8)- `iroha app da submit` (с интерфейсом командной строки) для сбора данных разработчиком/издателем.
  Он был создан в 2017 году в 2017 году в наборе Taikai Bundle. تعيش
  Ссылка на `crates/iroha_cli/src/commands/da.rs:1`, установленная на сайте.
  стирание/сохранение метаданных/манифеста
  `DaIngestRequest` Запустите CLI. تحفظ التشغيلات الناجحة
  `da_request.{norito,json}` и `da_receipt.{norito,json}` تحت
  `artifacts/da/submission_<timestamp>/` (переопределить `--artifact-dir`)
  Загрузить артефакты можно в байтах Norito для импорта.
- Переопределение переопределений в приложении `client_blob_id = blake3(payload)`.
  Загрузите `--client-blob-id`, загрузите метаданные JSON (`--metadata-json`) и
  манифестирует الجاهزة (`--manifest`) и `--no-submit` в автономном режиме.
  `--endpoint` для Torii. Получение квитанции JSON в формате stdout اضافة الى
  Он создал инструмент "submit_blob" для DA-8, созданный в 2007 году.
  Скачать SDK.
- `iroha app da get` псевдоним موجه لـ DA للمشغل متعدد المصادر الذي يشغل بالفعل.
  `iroha app sorafs fetch`. يمكن للمشغلين توجيهه الى манифест артефактов + план фрагментов
  (`--manifest`, `--plan`, `--manifest-id`) **او** Зарегистрируйте билет хранения для Torii
  عبر `--storage-ticket`. Создайте билет, откройте CLI и манифест.
  `/v2/da/manifests/<ticket>`, а также `artifacts/da/fetch_<timestamp>/`
  (переопределить `--manifest-cache-dir`), а также хеш-объект blob в `--manifest-id`,
  Для оркестратора используется `--gateway-provider`. Ручки تبقى جميع
  المتقدمة من جالب SoraFS كما هي (конверты манифеста, تسميات العميل, Guard
  кэши, переопределяет نقل مجهول, табло تصدير, ومسارات `--output`), ويمكن
  Конечная точка манифеста: `--manifest-endpoint`, Torii.
  Доступность будет доступна в ближайшее время, когда появится сообщение `da`.
  оркестратор.
- `iroha app da get-blob` проявляется в виде ошибки, связанной с Torii عبر.
  `GET /v2/da/manifests/{storage_ticket}`. يكتب الامر
  `manifest_{ticket}.norito` و`manifest_{ticket}.json` و`chunk_plan_{ticket}.json`
  تحت `artifacts/da/fetch_<timestamp>/` (او `--output-dir` يحدده المستخدم)
  طباعة امر `iroha app da get` الدقيق (Pما في ذلك `--manifest-id`)
  оркестратор. Для этого необходимо просмотреть катушку манифеста.
  Сборщик артефактов الموقعة الصادرة عن Torii. Код Torii
  В JavaScript в коде `ToriiClient.getDaManifest(storageTicketHex)`,
  Загрузка байтов Norito для манифеста JSON и плана фрагментов вызовов в SDK
  В программе оркестратора используется интерфейс CLI. Использование SDK Swift
  الاسطح (`ToriiClient.getDaManifestBundle(...)` مع
  `fetchDaPayloadViaGateway(...)`), используйте оркестратор SoraFS
  Приложение работает в iOS, когда приложение манифестирует и извлекает данные.
  Откройте интерфейс CLI.
  [IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240][IrohaSwift/Sources/IrohaSwift/SorafsOrchestratorClient.swift:12]
- `iroha app da rent-quote` арендован в аренду в Нижнем Новгороде.
  مقدمة. Создайте файл `DaRentPolicyV1` (JSON и байты Norito).
  Создание файла в формате JSON (`gib`, `months`, DB). سياسة،وحقول `DaRentQuote`) Он был использован в программе XOR الدقيقة في.
  Это было сделано в честь Дня Рождения. Он сказал, что в действительности
  `rent_quote ...` содержит JSON-файл, созданный в формате JSON.
  تمارين الحوادث. قم بقرن `--quote-out artifacts/da/rent_quotes/<stamp>.json`
  `--policy-label "governance ticket #..."` содержит артефакты.
  تصويت السياسة او حزمة التهيئة؛ Доступ к интерфейсу командной строки для изменения настроек
  Он был установлен на `policy_source` и был установлен в Лос-Анджелесе. راجع
  `crates/iroha_cli/src/commands/da.rs` للامر الفرعي و
  `docs/source/da/rent_policy.md` отключен.
  [crates/iroha_cli/src/commands/da.rs:1][docs/source/da/rent_policy.md:1]
- `iroha app da prove-availability` يربط كل سبق: ياخذ Storage Ticket, ينزل حزمة
  манифест манифеста и оркестратора متعدد المصادر (`iroha app sorafs fetch`)
  قائمة `--gateway-provider` المعطاة, ويحفظ الحمولة التي تم تنزيلها + табло
  تحت `artifacts/da/prove_availability_<timestamp>/`, добавление PoR
  Загружено (`iroha app da prove`) байтов. Защитные ручки
  оркестратор (`--max-peers`, `--scoreboard-out`, переопределяет манифест لعنوان) и
  Пробоотборник (`--sample-count`, `--leaf-index`, `--sample-seed`)
  Артефакты для создания артефактов DA-5/DA-9:
  табло, формат JSON.