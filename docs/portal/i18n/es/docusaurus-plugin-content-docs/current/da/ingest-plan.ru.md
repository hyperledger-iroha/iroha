---
lang: es
direction: ltr
source: docs/portal/docs/da/ingest-plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota Канонический источник
Esta página está escrita `docs/source/da/ingest_plan.md`. Держите обе версии
:::

# Plan de ingesta de disponibilidad de datos Sora Nexus

_Черновик: 2026-02-20 — Владельцы: Core Protocol WG / Equipo de almacenamiento / DA WG_

Рабочий поток DA-2 расширяет Torii API ingest para blob, который выпускает
Norito-metadannye and запускает репликацию SoraFS. Descripción del documento anterior
схему, API-область и поток валидации, чтобы реализация шла без блокировки на
ожидающих симуляциях (seguimiento DA-1). Все форматы payload ДОЛЖНЫ использовать
códigos Norito; El respaldo en serde/JSON no está disponible.

## Цели

- Принимать большие blob (сегменты Taikai, carril lateral, артефакты управления)
  El parámetro es Torii.
- Создавать канонические manifiestos Norito, descripción de blob, códec de parámetros,
  borrado de perfil y retención política.
- Сохранять метаданные fragment в hot Storage SoraFS y ставить задачи репликации в
  очередь.
- Publicar intenciones de pin + etiquetas de política en el registro SoraFS y en la base
  управления.
- Выдавать recibos de admisión, чтобы клиенты получали детерминированное
  подтверждение публикации.

## Superficie API (Torii)

```
POST /v1/da/ingest
Content-Type: application/norito+v1
```

Carga útil: este es `DaIngestRequest`, incluido Norito. Ответы используют
`application/norito+v1` y возвращают `DaIngestReceipt`.| Ответ | Значение |
| --- | --- |
| 202 Aceptado | Blob se actualiza mediante fragmentación/replicación; recibo возвращен. |
| 400 Solicitud incorrecta | Нарушение esquema/размера (см. проверки). |
| 401 No autorizado | Отсутствует/некорректен API-token. |
| 409 Conflicto | Duplica `client_blob_id` con un metadano nuevo. |
| 413 Carga útil demasiado grande | Превышен limит длины blob. |
| 429 Demasiadas solicitudes | Límite de tasa anterior. |
| 500 Error interno | Неожиданная ошибка (лог + алерт). |

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

> Principales realizaciones: tipos de carga útil de Rust-preдставления этих
> находятся в `iroha_data_model::da::types`, с envoltorios de solicitud/recibo в
> `iroha_data_model::da::ingest` y manifiesto estructural en
> `iroha_data_model::da::manifest`.

El polo `compression` describe la carga útil de la persona que llama. Torii modelo
`identity`, `gzip`, `deflate` y `zstd`, распаковывая байты перед hash,
Manifiestos de fragmentación y проверкой опциональных.

### Чеклист валидации1. Pruebe, что заголовок Norito соответствует `DaIngestRequest`.
2. Ошибка, если `total_size` отличается от канонической длины payload
   (после распаковки) или превышает настроенный максимум.
3. Pruebe el valor `chunk_size` (степень двойки, = 2.
5. `retention_policy.required_replica_count` должен соблюдать línea base de gobernanza.
6. Проверка подписи по каноническому hash (без поля подписи).
7. Eliminación de duplicados `client_blob_id`, otra carga útil de hash y metadanos
   идентичны.
8. Primero, `norito_manifest` muestra el esquema + hash en la base de datos
   manifiesto, пересчитанным после fragmentación; иначе узел генерирует manifiesto и
   сохраняет его.
9. Principales respuestas políticas actuales: Torii переписывает отправленный
   `RetentionPolicy` desde `torii.da_ingest.replication_policy` (см.
   `replication-policy.md`) y отклоняет заранее созданные manifiestos, если их
   La retención de metadanos no es compatible con el perfil forzado.

### Fragmentos y replicaciones de fotos1. Cambie la carga útil a `chunk_size`, use BLAKE3 para el archivo chunk + Merkle
   raíz.
2. Сформировать Norito `DaManifestV1` (nueva estructura), fragmento de compromiso físico
   (role/group_id), diseño de borrado (числа паритета строк и столбцов плюс
   `ipa_commitment`), retención política y metadanos.
3. Publicar el manifiesto de bytes canónicos en el módulo anterior
   `config.da_ingest.manifest_store_dir` (Torii пишет `manifest.encoded` по
   carril/época/secuencia/ticket/huella digital), чтобы оркестрация SoraFS могла
   поглотить их и связать ticket de almacenamiento с сохраненными данными.
4. Publicar las intenciones del pin con `sorafs_car::PinIntent` con el tema actualizado y
   político.
5. Eliminar el problema Norito `DaIngestPublished` para un dispositivo nuevo
   (clientes ligeros, gobernanza, analítica).
6. Llamar a la persona que llama `DaIngestReceipt` (presionando el botón Torii DA) y activar
   заголовок `Sora-PDP-Commitment`, чтобы SDK сразу получили compromiso. Recibo
   теперь включает `rent_quote` (Norito `DaRentQuote`) y `stripe_layout`, чтобы
   отправители могли показывать базовую аренду, долю резерва, ожидания бонусов
   PDP/PoTR y diseño de borrado 2D рядом со ticket de almacenamiento до фиксации средств.

## Обновления Almacenamiento / Registro- Расширить `sorafs_manifest` новым `DaManifestV1`, обеспечив детерминированный
  parsing.
- Добавить новый registro flujo `da.pin_intent` с версионированным carga útil,
  который ссылается на manifest hash + ticket id.
- Eliminación de la observabilidad, los programas para monitorear la ingesta y el rendimiento de los datos.
  fragmentación, acumulación de pedidos y tareas pendientes.

## Prueba de estrategia

- Pruebas de unidad para validar esquemas, pruebas de detección y duplicados.
- Golden-тесты для проверки Norito codificación `DaIngestRequest`, manifiesto y recibo.
- Интеграционный arnés, поднимающий simulacro SoraFS + registro y проверяющий
  потоки trozo + alfiler.
- Pruebas de propiedad para borrado de perfil de propiedad y retención combinada.
- Fuzzing de carga útil Norito para eliminar metadatos incorrectos.

## Herramientas CLI y SDK (DA-8)- `iroha app da submit` (nuevo punto de entrada CLI) que incluye el generador de ingesta/
  editor, чтобы операторы могли ingest-itь произвольные blobs вне потока
  Paquete Taikai. Команда находится в `crates/iroha_cli/src/commands/da.rs:1` и
  carga útil de configuración, borrado/retención de perfiles y archivos opcionales
  metadatos/manifiesto antes de escribir el código `DaIngestRequest`
  configuraciones CLI. Успешные запуски сохраняют `da_request.{norito,json}` и
  `da_receipt.{norito,json}` en `artifacts/da/submission_<timestamp>/`
  (anular через `--artifact-dir`), чтобы релизные artefactos фиксировали точные
  Norito bytes, disponible para la ingesta.
- Команда по умолчанию использует `client_blob_id = blake3(payload)`, no
  La opción anula los metadatos de los mapas JSON `--client-blob-id`.
  (`--metadata-json`) y manifiestos pregenerados (`--manifest`), y luego
  `--no-submit` para los archivos adjuntos y `--endpoint` para los archivos Torii.
  Recibo JSON grabado en la salida estándar y grabado en el disco, grabado en el DA-8
  "submit_blob" y la solución de paridad del SDK.
- `iroha app da get` добавляет DA-ориентированный alias для multi-source Orchestrator,
  который уже питает `iroha app sorafs fetch`. Los operadores pueden utilizar artefactos
  manifiesto + plan de fragmentos (`--manifest`, `--plan`, `--manifest-id`) **или** передать
  Boleto de almacenamiento Torii через `--storage-ticket`. При использовании ticket CLI
  загружает manifest из `/v1/da/manifests/<ticket>`, сохраняет paquete в`artifacts/da/fetch_<timestamp>/` (anulación de `--manifest-cache-dir`), выводит
  blob hash para `--manifest-id` y descarga el orquestador con el tiempo
  `--gateway-provider` списком. Otros productos de perillas del buscador SoraFS
  сохраняются (sobres de manifiesto, etiquetas de cliente, cachés de guardia, anónimos
  anulaciones de transporte, marcador de deportes, `--output` пути), un punto final manifiesto
  можно переопределить через `--manifest-endpoint` для кастомных Torii хостов,
  Esta es la disponibilidad de un extremo a otro que se encuentra en el espacio de nombres `da` без
  дублирования orquestador логики.
- `iroha app da get-blob` забирает канонические manifiestos напрямую из Torii через
  `GET /v1/da/manifests/{storage_ticket}`. Команда пишет
  `manifest_{ticket}.norito`, `manifest_{ticket}.json` y `chunk_plan_{ticket}.json`
  en `artifacts/da/fetch_<timestamp>/` (или пользовательский `--output-dir`), при
  Este es el comando `iroha app da get` (включая `--manifest-id`), no disponible
  для последующего orquestador buscar. Estos son los operadores de robots
  directorios de carrete de manifiesto y garantía, qué buscador está utilizando
  подписанные artefactos Torii. Cliente JavaScript Torii para esta página
  `ToriiClient.getDaManifest(storageTicketHex)`, decoración del hogar Norito
  bytes, manifiesto JSON y plan de fragmentos, las personas que llaman al SDK pueden mejorar
  orquestador сессии без CLI. El SDK de Swift está disponible para los usuarios
  (`ToriiClient.getDaManifestBundle(...)` y `fetchDaPayloadViaGateway(...)`),Paquete de paquete en contenedor nativo SoraFS Orchestrator, muchos clientes de iOS
  скачивать manifiestos, выполнять búsqueda de múltiples fuentes y собирать доказательства без
  вызова CLI.
  [IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240][IrohaSwift/Sources/IrohaSwift/SorafsOrchestratorClient.swift:12]
- `iroha app da rent-quote` вычисляет детерминированную alquiler y разбор incentivo для
  заданного размера almacenamiento y окна retención. Хелпер потребляет активную
  `DaRentPolicyV1` (JSON o Norito bytes) por defecto, válido
  Política y configuración JSON (`gib`, `months`, metadatos y políticas de políticas
  `DaRentQuote`), los auditores pueden citar los cargos XOR en los protocolos
  управления без скриптов ad hoc. Команда также печатает однострочный
  `rent_quote ...` antes de la carga útil JSON de los logotipos de otros taladros.
  Свяжите `--quote-out artifacts/da/rent_quotes/<stamp>.json` с
  `--policy-label "governance ticket #..."`, чтобы сохранить аккуратные artefactos
  с точной ссылкой на голосование или paquete de configuración; CLI обрезает пользовательскую
  метку и отвергает пустые строки, чтобы `policy_source` оставался пригодным для
  дашбордов. См. `crates/iroha_cli/src/commands/da.rs` para controladores y
  `docs/source/da/rent_policy.md` para estas políticas.
  [crates/iroha_cli/src/commands/da.rs:1][docs/source/da/rent_policy.md:1]
- `iroha app da prove-availability` объединяет все выше: берет ticket de almacenamiento,скачивает канонический paquete de manifiesto, запускает orquestador de múltiples fuentes
  (`iroha app sorafs fetch`) против списка `--gateway-provider`, сохраняет
  Carga útil + marcador en `artifacts/da/prove_availability_<timestamp>/`,
  y un nuevo asistente de PoR (`iroha app da prove`) con soporte técnico
  bytes. Los operadores pueden instalar perillas de orquestador (`--max-peers`,
  `--scoreboard-out`, anulaciones de punto final de manifiesto) y muestra de prueba
  (`--sample-count`, `--leaf-index`, `--sample-seed`), según este comando
  выпускает artefactos, требуемые аудитами DA-5/DA-9: копию payload, доказательство
  marcador y prueba JSON-резюме.