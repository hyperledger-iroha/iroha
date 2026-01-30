---
lang: ja
direction: ltr
source: docs/portal/i18n/ar/docusaurus-plugin-content-docs/current/da/ingest-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4297a72d8e00a6db26a62c33ddf6280db0da5f0bcb46a6ee1a88d41d89da52e7
source_last_modified: "2026-01-22T15:38:30+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: ar
direction: rtl
source: docs/portal/docs/da/ingest-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note المصدر القياسي
يعكس `docs/source/da/ingest_plan.md`. ابق النسختين متزامنتين حتى يتم سحب
الوثائق القديمة.
:::

# خطة ingest لتوفر البيانات في Sora Nexus

_مسودة: 2026-02-20 - المالك: Core Protocol WG / Storage Team / DA WG_

يمدد مسار DA-2 منصة Torii بواجهة ingest للـ blobs تصدر بيانات Norito الوصفية
وتبذر تكرار SoraFS. يوثق هذا المستند المخطط المقترح وسطح API وتدفق التحقق حتى
يتقدم التنفيذ دون انتظار محاكاة DA-1 المتبقية. يجب ان تستخدم جميع تنسيقات
الحمولة ترميزات Norito؛ لا يسمح باي fallback الى serde/JSON.

## الاهداف

- قبول blobs كبيرة (قطاعات Taikai، sidecars للحارات، وادوات حوكمة) بشكل حتمي
  عبر Torii.
- انتاج manifests Norito معيارية تصف الـ blob ومعلمات codec وملف erasure
  وسياسة الاحتفاظ.
- حفظ بيانات chunks الوصفية في تخزين SoraFS الساخن ووضع مهام التكرار في
  الطابور.
- نشر نوايا pin + وسوم السياسة في سجل SoraFS ومراقبي الحوكمة.
- اتاحة receipts القبول كي يستعيد العملاء دليلا حتميا على النشر.

## سطح API (Torii)

```
POST /v1/da/ingest
Content-Type: application/norito+v1
```

الحمولة هي `DaIngestRequest` مشفرة بـ Norito. تستخدم الاستجابات
`application/norito+v1` وتعيد `DaIngestReceipt`.

| الاستجابة | المعنى |
| --- | --- |
| 202 Accepted | تم وضع الـ blob في طابور التجزئة/التكرار؛ تم ارجاع receipt. |
| 400 Bad Request | انتهاك schema/الحجم (انظر فحوصات التحقق). |
| 401 Unauthorized | رمز API مفقود/غير صالح. |
| 409 Conflict | تكرار `client_blob_id` مع بيانات وصفية غير مطابقة. |
| 413 Payload Too Large | يتجاوز حد طول الـ blob المكون. |
| 429 Too Many Requests | تم بلوغ rate limit. |
| 500 Internal Error | فشل غير متوقع (log + تنبيه). |

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

> ملاحظة تنفيذية: تم نقل التمثيلات Rust القياسية لهذه الحمولات الى
> `iroha_data_model::da::types`، مع wrappers للطلب/receipt في
> `iroha_data_model::da::ingest` وبنية manifest في `iroha_data_model::da::manifest`.

يعلن حقل `compression` كيف جهز المتصلون الحمولة. يقبل Torii `identity` و`gzip`
و`deflate` و`zstd` مع فك الضغط قبل hashing والتجزئة والتحقق من manifests
الاختيارية.

### قائمة تحقق التحقق

1. تحقق من ان ترويسة Norito للطلب تطابق `DaIngestRequest`.
2. افشل اذا كان `total_size` مختلفا عن طول الحمولة القانوني (بعد فك الضغط) او
   يتجاوز الحد الاقصى المكون.
3. فرض محاذاة `chunk_size` (قوة اثنين، <= 2 MiB).
4. ضمان `data_shards + parity_shards` <= الحد الاقصى العالمي و parity >= 2.
5. يجب ان يحترم `retention_policy.required_replica_count` خط اساس الحوكمة.
6. تحقق التوقيع مقابل الهاش القياسي (مع استبعاد حقل التوقيع).
7. رفض `client_blob_id` المكرر ما لم تكن قيمة هاش الحمولة والبيانات الوصفية
   متطابقة.
8. عند توفير `norito_manifest`، تحقق من مطابقة schema + hash للـ manifest المعاد
   حسابه بعد التجزئة؛ والا يقوم العقدة بتوليد manifest وتخزينه.
9. تطبيق سياسة التكرار المكونة: يعيد Torii كتابة `RetentionPolicy` المرسل عبر
   `torii.da_ingest.replication_policy` (راجع `replication-policy.md`) ويرفض
   manifests المجهزة مسبقا اذا لم تطابق بيانات الاحتفاظ الملف المفروض.

### تدفق التجزئة والتكرار

1. تجزئة الحمولة الى `chunk_size`، وحساب BLAKE3 لكل chunk + جذر Merkle.
2. بناء Norito `DaManifestV1` (struct جديدة) تلتقط التزامات الـ chunk
   (role/group_id)، وتخطيط erasure (اعداد تكافؤ الصفوف والاعمدة مع
   `ipa_commitment`)، وسياسة الاحتفاظ والبيانات الوصفية.
3. وضع bytes الـ manifest القياسي تحت `config.da_ingest.manifest_store_dir`
   (يكتب Torii ملفات `manifest.encoded` حسب lane/epoch/sequence/ticket/fingerprint)
   حتى تتمكن منظومة SoraFS من ابتلاعها وربط storage ticket بالبيانات المحفوظة.
4. نشر نوايا pin عبر `sorafs_car::PinIntent` مع وسم الحوكمة والسياسة.
5. بث حدث Norito `DaIngestPublished` لاخطار المراقبين (عملاء خفيفين، الحوكمة،
   التحليلات).
6. ارجاع `DaIngestReceipt` للمتصل (موقع بمفتاح خدمة Torii DA) وارسال ترويسة
   `Sora-PDP-Commitment` كي تلتقط SDKs الالتزام المشفر فورا. يتضمن الـ receipt
   الان `rent_quote` (Norito `DaRentQuote`) و`stripe_layout` لتمكين
   المرسلين من عرض الاجرة الاساسية، وحصة الاحتياطي، وتوقعات مكافاة PDP/PoTR
   وتخطيط erasure ثنائي الابعاد بجانب storage ticket قبل الالتزام بالاموال.

## تحديثات التخزين/السجل

- توسيع `sorafs_manifest` بـ `DaManifestV1` لتمكين parsing الحتمي.
- اضافة stream جديد للسجل `da.pin_intent` مع حمولة ذات اصدار تشير الى hash
  manifest + ticket id.
- تحديث خطوط الملاحظة لمتابعة زمن ingest، وthroughput التجزئة، وباك لوج
  التكرار، وعدد الاخفاقات.

## استراتيجية الاختبارات

- اختبارات وحدة للتحقق من schema، وفحوصات التوقيع، وكشف التكرار.
- اختبارات golden لتاكيد ترميز Norito لـ `DaIngestRequest` وmanifest وreceipt.
- harness تكامل يشغل SoraFS + registry وهمي، ويتحقق من تدفقات chunk + pin.
- اختبارات خصائص تغطي ملفات erasure وتركيبات الاحتفاظ العشوائية.
- Fuzzing لحمولات Norito للحماية من metadata تالفة.

## ادوات CLI و SDK (DA-8)

- `iroha app da submit` (مدخل CLI جديد) يلف الان builder/publisher ingest المشترك
  حتى يتمكن المشغلون من ادخال blobs عشوائية خارج مسار Taikai bundle. تعيش
  الاوامر في `crates/iroha_cli/src/commands/da.rs:1` وتستهلك حمولة وملف
  erasure/retention وملفات metadata/manifest اختيارية قبل توقيع
  `DaIngestRequest` القياسي بمفتاح تهيئة CLI. تحفظ التشغيلات الناجحة
  `da_request.{norito,json}` و`da_receipt.{norito,json}` تحت
  `artifacts/da/submission_<timestamp>/` (override عبر `--artifact-dir`) لكي
  تسجل artefacts الاصدار bytes Norito الدقيقة المستخدمة اثناء ingest.
- يستخدم الامر افتراضيا `client_blob_id = blake3(payload)` لكنه يقبل overrides
  عبر `--client-blob-id`، ويحترم خرائط metadata JSON (`--metadata-json`) و
  manifests الجاهزة (`--manifest`)، ويدعم `--no-submit` للتحضير offline مع
  `--endpoint` لمضيفي Torii المخصصين. يطبع receipt JSON الى stdout اضافة الى
  كتابته على القرص، مما يغلق متطلب tooling "submit_blob" في DA-8 ويفتح عمل
  تكافؤ SDK.
- `iroha app da get` يضيف alias موجه لـ DA للمشغل متعدد المصادر الذي يشغل بالفعل
  `iroha app sorafs fetch`. يمكن للمشغلين توجيهه الى artefacts manifest + chunk-plan
  (`--manifest`, `--plan`, `--manifest-id`) **او** تمرير storage ticket من Torii
  عبر `--storage-ticket`. عند استخدام مسار ticket، تقوم CLI بجلب manifest من
  `/v1/da/manifests/<ticket>`، وتخزن الحزمة تحت `artifacts/da/fetch_<timestamp>/`
  (override مع `--manifest-cache-dir`)، وتشتق hash الـ blob لـ `--manifest-id`،
  ثم تشغل orchestrator مع قائمة `--gateway-provider` المعطاة. تبقى جميع knobs
  المتقدمة من جالب SoraFS كما هي (manifest envelopes، تسميات العميل، guard
  caches، overrides نقل مجهول، تصدير scoreboard، ومسارات `--output`)، ويمكن
  استبدال manifest endpoint عبر `--manifest-endpoint` لمضيفي Torii المخصصين، لذا
  تعيش فحوصات availability من النهاية للنهاية ضمن مساحة `da` دون تكرار منطق
  orchestrator.
- `iroha app da get-blob` يسحب manifests القياسية مباشرة من Torii عبر
  `GET /v1/da/manifests/{storage_ticket}`. يكتب الامر
  `manifest_{ticket}.norito` و`manifest_{ticket}.json` و`chunk_plan_{ticket}.json`
  تحت `artifacts/da/fetch_<timestamp>/` (او `--output-dir` يحدده المستخدم) مع
  طباعة امر `iroha app da get` الدقيق (بما في ذلك `--manifest-id`) المطلوب لجلب
  orchestrator. هذا يبقي المشغلين بعيدا عن ادلة manifest spool ويضمن ان
  fetcher يستخدم دائما artefacts الموقعة الصادرة عن Torii. يعكس عميل Torii
  في JavaScript هذا التدفق عبر `ToriiClient.getDaManifest(storageTicketHex)`،
  ويعيد bytes Norito المفكوكة وmanifest JSON وchunk plan ليتمكن callers في SDK
  من تهيئة جلسات orchestrator دون استخدام CLI. يعرض SDK Swift الان نفس
  الاسطح (`ToriiClient.getDaManifestBundle(...)` مع
  `fetchDaPayloadViaGateway(...)`)، موجها الحزم الى غلاف orchestrator SoraFS
  الاصلي حتى يتمكن عملاء iOS من تنزيل manifests وتنفيذ fetches متعددة المصادر
  والتقاط الادلة دون استدعاء CLI.
  [IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240][IrohaSwift/Sources/IrohaSwift/SorafsOrchestratorClient.swift:12]
- `iroha app da rent-quote` يحسب rent حتمي وتفصيل الحوافز لحجم تخزين ونافذة احتفاظ
  مقدمة. يستهلك المساعد `DaRentPolicyV1` النشط (JSON او bytes Norito) او الافتراضي
  المدمج، ويتحقق من السياسة ويطبع ملخص JSON (`gib`, `months`، بيانات السياسة،
  وحقول `DaRentQuote`) حتى يتمكن المدققون من الاستشهاد برسوم XOR الدقيقة في
  محاضر الحوكمة دون نصوص مخصصة. كما يصدر الامر ملخصا من سطر واحد
  `rent_quote ...` قبل حمولة JSON للحفاظ على قابلية قراءة سجلات الطرفية اثناء
  تمارين الحوادث. قم بقرن `--quote-out artifacts/da/rent_quotes/<stamp>.json`
  مع `--policy-label "governance ticket #..."` لحفظ artefacts منسقة تشير الى
  تصويت السياسة او حزمة التهيئة؛ تقوم CLI بقص الوسم المخصص وترفض السلاسل الفارغة
  حتى تبقى قيم `policy_source` قابلة للتنفيذ في لوحات خزينة. راجع
  `crates/iroha_cli/src/commands/da.rs` للامر الفرعي و
  `docs/source/da/rent_policy.md` لمخطط السياسة.
  [crates/iroha_cli/src/commands/da.rs:1][docs/source/da/rent_policy.md:1]
- `iroha app da prove-availability` يربط كل ما سبق: ياخذ storage ticket، ينزل حزمة
  manifest القياسية، يشغل orchestrator متعدد المصادر (`iroha app sorafs fetch`) مقابل
  قائمة `--gateway-provider` المعطاة، ويحفظ الحمولة التي تم تنزيلها + scoreboard
  تحت `artifacts/da/prove_availability_<timestamp>/`، ويستدعي مباشرة مساعد PoR
  الحالي (`iroha app da prove`) باستخدام bytes المحملة. يستطيع المشغلون ضبط knobs
  orchestrator (`--max-peers`, `--scoreboard-out`, overrides لعنوان manifest) و
  sampler للاثبات (`--sample-count`, `--leaf-index`, `--sample-seed`) بينما ينتج
  امر واحد artefacts المطلوبة لتدقيقات DA-5/DA-9: نسخة من الحمولة، دليل
  scoreboard، وملخصات اثبات JSON.
