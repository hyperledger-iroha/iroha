---
lang: he
direction: rtl
source: docs/portal/docs/da/ingest-plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note Канонический источник
Эта страница отражает `docs/source/da/ingest_plan.md`. Держите обе версии
:::

# План ingest זמינות נתונים Sora Nexus

_Черновик: 2026-02-20 — Владельцы: Core Protocol WG / Storage Team / DA WG_

Рабочий поток DA-2 расширяет Torii API ingest ל-blob, который выпускает
Norito-מטאדאננייע וספיקות רזרביות SoraFS. Документ описывает предложенную
схему, API-область и поток валидации, чтобы реализация шла без блокировки на
ожидающих симуляциях (מעקב DA-1). Все форматы מטען ДОЛЖНЫ использовать
кодеки Norito; fallback на serde/JSON не допускается.

## Цели

- Принимать большие כתם (сегменты Taikai, נתיב קרונות צד, артефакты управления)
  детерминированно через Torii.
- Создавать канонические Norito מניפסט, описывающие כתם, параметры codec,
  מחיקת профиль и политику שימור.
- Сохранять метаданные chunk באחסון חם SoraFS и ставить задачи репликации в
  очередь.
- Публиковать כוונות סיכה + תגי מדיניות в реестр SoraFS и наблюдателям
  управления.
- Выдавать קבלות כניסה, чтобы клиенты получали детерминированное
  подтверждение публикации.

## משטח API (Torii)

```
POST /v2/da/ingest
Content-Type: application/norito+v1
```

מטען - это `DaIngestRequest`, закодированный Norito. Ответы используют
`application/norito+v1` ו-`DaIngestReceipt`.

| Ответ | Значение |
| --- | --- |
| 202 מקובל | בלוב поставлен в очередь на chunking/שכפול; קבלה возвращен. |
| 400 בקשה רעה | Нарушение schema/размера (см. проверки). |
| 401 לא מורשה | Отсутствует/некорректен API-токен. |
| 409 קונפליקט | Дубликат `client_blob_id` с несовпадающей метаданной. |
| 413 מטען גדול מדי | כתם Превышен лимит длины. |
| 429 יותר מדי בקשות | מגבלת שיעור Превышен. |
| 500 שגיאה פנימית | Неожиданная ошибка (лог + алерт). |

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

> מידע על מטען: канонические Rust-представления этих מטען מטען
> находятся в `iroha_data_model::da::types`, с עטיפות בקשה/קבלה в
> `iroha_data_model::da::ingest` и структурой מניפסט в
> `iroha_data_model::da::manifest`.

Поле `compression` описывает, как מטען המתקשר подготовил. Torii принимает
`identity`, `gzip`, `deflate` ו-`zstd`, распаковывая байты перед hashing,
chunking и проверкой опциональных מניפסטים.

### Чеклист валидации1. Проверить, что заголовок Norito соответствует `DaIngestRequest`.
2. Ошибка, если `total_size` отличается от канонической длины מטען
   (после распаковки) или превышает настроенный максимум.
3. Принудить выравнивание `chunk_size` (степень двойки, <= 2 MiB).
4. Убедиться, что `data_shards + parity_shards` <= глобального максимума и
   זוגיות >= 2.
5. `retention_policy.required_replica_count` должен соблюдать בסיס ממשל.
6. Проверка подписи по каноническому hash (ללא поля подписи).
7. התקן את התקן `client_blob_id`, מטען גיבוב נוסף ואמצעי מטען לא
   идентичны.
8. При наличии `norito_manifest` проверить schema + hash на совпадение с
   מניפסט, пересчитанным после chunking; иначе узел генерирует מניפסט и
   сохраняет его.
9. הצג פרופיל מדיניות: Torii
   `RetentionPolicy` через `torii.da_ingest.replication_policy` (см.
   `replication-policy.md`) и отклоняет заранее созданные מניפסטים, если их
   שמירה על שיטות לא ניתנת לאכיפה.

### פוטוק chunking и репликации

1. Разбить מטען на `chunk_size`, вычислить BLAKE3 для каждого chunk + Merkle
   שורש.
2. Сформировать Norito `DaManifestV1` (מבנה חדש), נתח התחייבות
   (role/group_id), פריסת מחיקה (числа паритета строк и столбцов плюс
   `ipa_commitment`), שמירה על חומרה.
3. Поставить канонические מניפסט בתים в очередь под
   `config.da_ingest.manifest_store_dir` (Torii пишет `manifest.encoded` по
   מסלול/תקופה/רצף/כרטיס/טביעת אצבע), чтобы оркестрация SoraFS могла
   поглотить их и связать כרטיס אחסון с сохраненными данными.
4. Публиковать pin intents через `sorafs_car::PinIntent` с тегом управления и
   политикой.
5. Эмитировать событие Norito `DaIngestPublished` для оповещения наблюдателей
   (לקוחות קלים, ממשל, אנליטיקה).
6. התקן את המתקשר `DaIngestReceipt` (תצלום Torii DA) או התקן
   заголовок `Sora-PDP-Commitment`, чтобы SDK сразу получили התחייבות. קבלה
   теперь включает `rent_quote` (Norito `DaRentQuote`) ו-`stripe_layout`, чтобы
   отправители могли показывать базовую аренду, долю резерва, ожидания бонусов
   פריסת PDP/PoTR и 2D מחיקה рядом со כרטיס אחסון до фиксации средств.

## ביטול אחסון / רישום

- Расширить `sorafs_manifest` новым `DaManifestV1`, обеспечив детерминированный
  парсинг.
- Добавить новый זרם הרישום `da.pin_intent` עם מטען версионированным,
  который ссылается ב-hash מניפסט + מזהה כרטיס.
- Обновить observability-пайплайны для мониторинга задержки לצרוך, תפוקה
  chunking, репликационного backlog и счетчиков ошибок.

## Стратегия тестирования- Unit-тесты для валидации סכימה, проверки подписей, детекта дубликатов.
- Golden-тесты для проверки Norito קידוד `DaIngestRequest`, מניפסט и קבלה.
- רתמה Интеграционный, поднимающий דוגמנית SoraFS + registry и проверяющий
  потоки גוש + סיכה.
- Property-тесты для случайных профилей מחיקה и комбинаций שימור.
- מטען Norito Fuzzing לרכיבי חומרי גלם.

## כלי עבודה של CLI ו-SDK (DA-8)- `iroha app da submit` (נקודת כניסת CLI חדשה) оборачивает общий ingest builder/
  מוציא לאור, чтобы операторы могли ingest-ить произвольные blobs вне потока
  צרור טאיקאי. Команда находится в `crates/iroha_cli/src/commands/da.rs:1` и
  принимает מטען, מחיקה/שמירה של רכיבים ו- опциональные файлы
  metadata/manifest перед подписью канонического `DaIngestRequest` ключом
  конфигурации CLI. Успешные запуски сохраняют `da_request.{norito,json}` и
  `da_receipt.{norito,json}` פוד `artifacts/da/submission_<timestamp>/`
  (עקוף את через `--artifact-dir`), чтобы релизные artefacts фиксировали точные
  Norito בתים, использованные при לצרוך.
- Команда по умолчанию использует `client_blob_id = blake3(payload)`, но
  поддерживает עוקף через `--client-blob-id`, JSON карты metadata
  (`--metadata-json`) ומניפסטים שנוצרו מראש (`--manifest`), а также
  `--no-submit` ל- офлайн подготовки ו- `--endpoint` ל- кастомных Torii хостов.
  קבלה JSON выводится в stdout и пишется на диск, закрывая требование DA-8
  "submit_blob" ו разблокируя SDK זוגיות работу.
- `iroha app da get` добавляет DA-ориентированный כינוי למזמר מרובה מקורות,
  который уже питает `iroha app sorafs fetch`. Операторы могут указать חפצי אמנות
  מניפסט + תוכנית נתח (`--manifest`, `--plan`, `--manifest-id`) **אולי** передать
  Torii כרטיס אחסון через `--storage-ticket`. При использовании כרטיס CLI
  загружает מניפסט из `/v2/da/manifests/<ticket>`, сохраняет חבילה в
  `artifacts/da/fetch_<timestamp>/` (עקוף באמצעות `--manifest-cache-dir`), выводит
  כתם חשיש ל-`--manifest-id` и запускает orchestrator с заданным
  `--gateway-provider` списком. Все продвинутые כפתורים из SoraFS מאחזר
  сохраняются (מעטפות מניפסט, תוויות לקוחות, מטמוני שמירה, אנונימי
  עקיפות תחבורה, לוח תוצאות של эксPORT, `--output` пути), נקודת קצה מניפסט
  можно переопределить через `--manifest-endpoint` для кастомных Torii хостов,
  כדי להגיע לזמינות מקצה לקצה.
  дублирования מתזמר логики.
- `iroha app da get-blob` забирает канонические מניפסטים напрямую из Torii через
  `GET /v2/da/manifests/{storage_ticket}`. Команда пишет
  `manifest_{ticket}.norito`, `manifest_{ticket}.json` ו-`chunk_plan_{ticket}.json`
  в `artifacts/da/fetch_<timestamp>/` (или пользовательский `--output-dir`), при
  этом выводит точную команду `iroha app da get` (включая `--manifest-id`), нужную
  для последующего אחזור מתזמר. Это избавляет операторов от работы с
  מניפסט סליל директориями и гарантирует, что fetcher всегда использует
  подписанные חפצי אמנות Torii. JavaScript клиент Torii повторяет этот поток через
  `ToriiClient.getDaManifest(storageTicketHex)`, возвращая декодированные Norito
  בתים, מניפסט JSON ותוכנית נתחים, чтобы מתקשרי SDK могли поднимать
  מתזמר сессии без CLI. Swift SDK теперь предоставляет те же поверхности
  (`ToriiClient.getDaManifestBundle(...)` ו-`fetchDaPayloadViaGateway(...)`),
  חבילה прокидывая в Native wrapper SoraFS מתזמר, чтобы iOS клиенты могли
  скачивать מניפסטים, выполнять ריבוי מקורות אחזור ו собирать доказательства без
  вызова CLI.[IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240][IrohaSwift/Sources/IrohaSwift/SorafsOrchestratorClient.swift:12]
- `iroha app da rent-quote` вычисляет детерминированную השכרה ותמריץ разбор для
  заданного размера אחסון и окна שימור. Хелпер потребляет активную
  `DaRentPolicyV1` (JSON או Norito בתים) либо встроенный default, валидирует
  מדיניות ותקשורת JSON-сводку (`gib`, `months`, מטא נתונים של מדיניות ועוד
  `DaRentQuote`), чтобы аудиторы могли цитировать точные חיובים XOR в протоколах
  управления без אד הוק скриптов. Команда также печатает однострочный
  `rent_quote ...` перед JSON מטען עבור читабельности логов во время מקדחות.
  Свяжите `--quote-out artifacts/da/rent_quotes/<stamp>.json` с
  `--policy-label "governance ticket #..."`, чтобы сохранить аккуратные חפצי אמנות
  с точной ссылкой на голосование или חבילת תצורה; CLI обрезает пользовательскую
  מטקות ושיטות עבודה, ציוד `policy_source` оставался пригодным для
  дашбордов. См. `crates/iroha_cli/src/commands/da.rs` עבור подкоманды и
  `docs/source/da/rent_policy.md` עבור схемы политики.
  [crates/iroha_cli/src/commands/da.rs:1][docs/source/da/rent_policy.md:1]
- `iroha app da prove-availability` אובדן כרטיס אחסון,
  חבילת מניפסט скачивает канонический, запускает מתזמר רב מקורות
  (`iroha app sorafs fetch`) против списка `--gateway-provider`, сохраняет
  скачанный מטען + לוח תוצאות в `artifacts/da/prove_availability_<timestamp>/`,
  и сразу вызывает существующий PoR helper (`iroha app da prove`) с полученными
  בתים. Операторы могут настраивать כפתורי התזמורת (`--max-peers`,
  `--scoreboard-out`, עקיפות נקודת קצה מניפסט) и דוגם הוכחה
  (`--sample-count`, `--leaf-index`, `--sample-seed`), при этом одна команда
  выпускает חפצי אמנות, требуемые аудитами DA-5/DA-9: копию מטען, доказательство
  לוח תוצאות והוכחה ל-JSON-резюме.