---
lang: es
direction: ltr
source: docs/portal/docs/da/ingest-plan.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota المصدر القياسي
Aquí `docs/source/da/ingest_plan.md`. ابق النسختين متزامنتين حتى يتم سحب
الوثائق القديمة.
:::

# خطة ingest لتوفر البيانات في Sora Nexus

_مسودة: 2026-02-20 - المالك: Core Protocol WG / Equipo de almacenamiento / DA WG_

مسار DA-2 منصة Torii بواجهة بواجهة للـ blobs تصدر بيانات Norito الوصفية
Aquí está el mensaje SoraFS. يوثق هذا المستند المخطط المقترح وسطح API وتدفق التحقق حتى
يتقدم التنفيذ دون انتظار محاكاة DA-1 المتبقية. يجب ان تستخدم جميع تنسيقات
Adaptadores de corriente Norito؛ Aquí hay un respaldo en serde/JSON.

## الاهداف

- قبول blobs كبيرة (قطاعات Taikai, sidecars للحارات، وادوات حوكمة) بشكل حتمي
  Ver Torii.
- Manifiestos Norito Cambio de blob, códec y borrado
  وسياسة الاحتفاظ.
- حفظ بيانات trozos الوصفية في تخزين SoraFS الساخن ووضع مهام التكرار في
  الطابور.
- نشر نوايا pin + وسوم السياسة في سجل SoraFS ومراقبي الحوكمة.
- اتاحة recibos القبول كي يستعيد العملاء دليلا حتميا على النشر.

## API API (Torii)

```
POST /v1/da/ingest
Content-Type: application/norito+v1
```

La conexión entre `DaIngestRequest` y Norito. تستخدم الاستجابات
`application/norito+v1` y `DaIngestReceipt`.| الاستجابة | المعنى |
| --- | --- |
| 202 Aceptado | تم وضع الـ blob في طابور التجزئة/التكرار؛ تم ارجاع recibo. |
| 400 Solicitud incorrecta | انتهاك esquema/الحجم (انظر فحوصات التحقق). |
| 401 No autorizado | رمز API مفقود/غير صالح. |
| 409 Conflicto | تكرار `client_blob_id` مع بيانات وصفية غير مطابقة. |
| 413 Carga útil demasiado grande | يتجاوز حد طول الـ blob المكون. |
| 429 Demasiadas solicitudes | تم بلوغ límite de tasa. |
| 500 Error interno | فشل غير متوقع (log + تنبيه). |

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

> Aplicaciones de Rust: Aplicaciones de Rust.
> `iroha_data_model::da::types`, envoltorios de recibos/recibos
> `iroha_data_model::da::ingest` y manifiesto de `iroha_data_model::da::manifest`.

Este es el `compression` que está conectado al sistema. Incluye Torii `identity` y `gzip`
و`deflate` و`zstd` para crear hash, hashing y manifiestos
الاختيارية.

### قائمة تحقق التحقق1. Conecte el dispositivo Norito al dispositivo `DaIngestRequest`.
2. افشل اذا كان `total_size` مختلفا عن طول الحمولة القانوني (بعد فك الضغط) او
   يتجاوز الحد الاقصى المكون.
3. Inserte `chunk_size` (índice de almacenamiento = 2.
5. يجب ان يحترم `retention_policy.required_replica_count` خط اساس الحوكمة.
6. تحقق التوقيع مقابل الهاش القياسي (مع استبعاد حقل التوقيع).
7. رفض `client_blob_id` المكرر ما لم تكن قيمة هاش الحمولة والبيانات الوصفية
   متطابقة.
8. Utilice `norito_manifest`, utilice el esquema + hash del manifiesto
   حسابه بعد التجزئة؛ والا يقوم العقدة بتوليد manifiesto وتخزينه.
9. تطبيق سياسة التكرار المكونة: يعيد Torii كتابة `RetentionPolicy` المرسل عبر
   `torii.da_ingest.replication_policy` (راجع `replication-policy.md`) y
   manifiesta المجهزة مسبقا اذا لم تطابق بيانات الاحتفاظ الملف المفروض.

### تدفق التجزئة والتكرار1. تجزئة الحمولة الى `chunk_size`, وحساب BLAKE3 لكل chunk + جذر Merkle.
2. بناء Norito `DaManifestV1` (estructura جديدة) تلتقط التزامات الـ trozo
   (rol/group_id) ، وتخطيط borrado (اعداد تكافؤ الصفوف والاعمدة مع
   `ipa_commitment`) , وسياسة الاحتفاظ والبيانات الوصفية.
3. وضع bytes del manifiesto القياسي تحت `config.da_ingest.manifest_store_dir`
   (يكتب Torii ملفات `manifest.encoded` حسب carril/época/secuencia/ticket/huella digital)
   حتى تمكن منظومة SoraFS من ابتلاعها y ticket de almacenamiento بالبيانات المحفوظة.
4. El pin de nuevo es `sorafs_car::PinIntent` مع وسم الحوكمة والسياسة.
5. بث حدث Norito `DaIngestPublished` لاخطار المراقبين (عملاء خفيفين، الحوكمة،
   التحليلات).
6. ارجاع `DaIngestReceipt` للمتصل (موقع بمفتاح خدمة Torii DA) وارسال ترويسة
   `Sora-PDP-Commitment` Aquí está el SDK de la aplicación. يتضمن الـ recibo
   Por ejemplo, `rent_quote` (Norito `DaRentQuote`) y `stripe_layout`.
   المرسلين من عرض الاجرة الاساسية، وحصة الاحتياطي، وتوقعات مكافاة PDP/PoTR
   وتخطيط borrado ثنائي الابعاد بجانب boleto de almacenamiento قبل الالتزام بالاموال.

## تحديثات التخزين/السجل

- توسيع `sorafs_manifest` بـ `DaManifestV1` para analizar el archivo.
- اضافة stream جديد للسجل `da.pin_intent` مع حمولة ذات اصدار تشير الى hash
  manifiesto + identificación del ticket.
- تحديث خطوط الملاحظة لمتابعة زمن ingesta, وthroughput التجزئة, وباك لوج
  التكرار، وعدد الاخفاقات.

## استراتيجية الاختبارات- اختبارات وحدة للتحقق من esquema, وفحوصات التوقيع، وكشف التكرار.
- اختبارات golden لتاكيد ترميز Norito لـ `DaIngestRequest` وmanifiesto وrecibo.
- arnés تكامل يشغل SoraFS + registro وهمي، ويتحقق من تدفقات trozo + pin.
- اختبارات خصائص تغطي ملفات borrado y تركيبات الاحتفاظ العشوائية.
- Fuzzing لحمولات Norito للحماية من metadatos تالفة.

## CLI y SDK (DA-8)- `iroha app da submit` (mediante CLI) para la ingesta del constructor/editor
  حتى يتمكن المشغلون من ادخال blobs عشوائية خارج مسار Paquete Taikai. تعيش
  Fuente de alimentación `crates/iroha_cli/src/commands/da.rs:1`
  borrado/retención y metadatos/manifiesto اختيارية قبل توقيع
  `DaIngestRequest` Haga clic en la CLI. تحفظ التشغيلات الناجحة
  Funciones `da_request.{norito,json}` y `da_receipt.{norito,json}`
  `artifacts/da/submission_<timestamp>/` (anulación de `--artifact-dir`) aquí
  تسجل artefactos الاصدار bytes Norito الدقيقة المستخدمة اثناء ingerir.
- يستخدم الامر افتراضيا `client_blob_id = blake3(payload)` لكنه يقبل anulaciones
  Utilice `--client-blob-id` y seleccione metadatos JSON (`--metadata-json`) y
  manifiesta الجاهزة (`--manifest`) , y `--no-submit` للتحضير fuera de línea مع
  `--endpoint` لمضيفي Torii مخصصين. يطبع recibo JSON y salida estándar اضافة الى
  Utilice la herramienta "submit_blob" para DA-8 y utilice la herramienta "submit_blob"
  Utilice el SDK.
- `iroha app da get` يضيف alias موجه لـ DA للمشغل متعدد المصادر الذي يشغل بالفعل
  `iroha app sorafs fetch`. يمكن للمشغلين توجيهه الى manifiesto de artefactos + plan de fragmentos
  (`--manifest`, `--plan`, `--manifest-id`) **او** تمرير ticket de almacenamiento de Torii
  Ver `--storage-ticket`. عند استخدام مسار ticket, تقوم CLI بجلب manifiesto من
  `/v1/da/manifests/<ticket>`, y las funciones de `artifacts/da/fetch_<timestamp>/`
  (anulación de `--manifest-cache-dir`), y un hash de blob en `--manifest-id`,
  ثم تشغل Orchestrator مع قائمة `--gateway-provider` المعطاة. تبقى جميع perillasالمتقدمة من جالب SoraFS كما هي (sobres manifiestos, تسميات العميل، guardia
  cachés, anula el marcador del marcador (y el `--output`), y
  Punto final del manifiesto desde `--manifest-endpoint` hasta Torii
  تعيش فحوصات disponibilidad من النهاية للنهاية ضمن مساحة `da` دون تكرار منطق
  orquestador.
- `iroha app da get-blob` يسحب manifiesta القياسية مباشرة من Torii عبر
  `GET /v1/da/manifests/{storage_ticket}`. يكتب الامر
  `manifest_{ticket}.norito` y `manifest_{ticket}.json` y `chunk_plan_{ticket}.json`
  تحت `artifacts/da/fetch_<timestamp>/` (او `--output-dir` يحدده المستخدم) مع
  طباعة امر `iroha app da get` الدقيق (بما في ذلك `--manifest-id`) المطلوب لجلب
  orquestador. هذا يبقي المشغلين بعيدا عن ادلة manifiesto spool ويضمن ان
  Fetcher يستخدم دائما artefactos الموقعة الصادرة عن Torii. يعكس عميل Torii
  Aquí JavaScript está escrito en `ToriiClient.getDaManifest(storageTicketHex)`.
  Bytes Norito Archivo, manifiesto JSON y plan fragmentado para personas que llaman desde SDK
  من تهيئة جلسات Orchestrator دون استخدام CLI. يعرض SDK Swift en español
  الاسطح (`ToriiClient.getDaManifestBundle(...)` مع
  `fetchDaPayloadViaGateway(...)`) ، موجها الحزم الى غلاف orquestador SoraFS
  La aplicación iOS muestra manifiestos y recuperaciones de archivos
  والتقاط الادلة دون استدعاء CLI.
  [IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240][IrohaSwift/Sources/IrohaSwift/SorafsOrchestratorClient.swift:12]- `iroha app da rent-quote` يحسب alquiler حتمي وتفصيل الحوافز لحجم تخزين ونافذة احتفاظ
  مقدمة. Formato `DaRentPolicyV1` (JSON y bytes Norito) y contenido
  Configuración y configuración de archivos JSON (`gib`, `months`, archivos adjuntos)
  وحقول `DaRentQuote`) حتى يتمكن المدققون من الاستشهاد برسوم XOR الدقيقة في
  محاضر الحوكمة دون نصوص مخصصة. كما يصدر الامر ملخصا من سطر واحد
  `rent_quote ...` Formato JSON para configuración de archivos de datos
  تمارين الحوادث. Por otra parte `--quote-out artifacts/da/rent_quotes/<stamp>.json`
  مع `--policy-label "governance ticket #..."` لحفظ artefactos منسقة تشير الى
  تصويت السياسة او حزمة التهيئة؛ تقوم CLI بقص الوسم المخصص وترفض السلاسل الفارغة
  Conecte el `policy_source` para que funcione correctamente. راجع
  `crates/iroha_cli/src/commands/da.rs` للامر الفرعي و
  `docs/source/da/rent_policy.md` لمخطط السياسة.
  [crates/iroha_cli/src/commands/da.rs:1][docs/source/da/rent_policy.md:1]
- `iroha app da prove-availability` يربط كل ما سبق: ياخذ ticket de almacenamiento, ينزل حزمة
  manifiesto القياسية، يشغل orquestador متعدد المصادر (`iroha app sorafs fetch`) مقابل
  قائمة `--gateway-provider` المعطاة، ويحفظ الحمولة التي تم تنزيلها + marcador
  تحت `artifacts/da/prove_availability_<timestamp>/`, ويستدعي مباشرة مساعد PoR
  الحالي (`iroha app da prove`) باستخدام bytes المحملة. يستطيع المشغلون ضبط perillas
  orquestador (`--max-peers`, `--scoreboard-out`, anula el manifiesto de لعنوان) y
  sampler للاثبات (`--sample-count`, `--leaf-index`, `--sample-seed`) بينما ينتجامر واحد artefactos المطلوبة لتدقيقات DA-5/DA-9: نسخة من الحمولة، دليل
  marcador, وملخصات اثبات JSON.