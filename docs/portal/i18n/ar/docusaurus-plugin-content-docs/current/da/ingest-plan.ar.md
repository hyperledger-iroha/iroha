---
lang: ar
direction: rtl
source: docs/portal/docs/da/ingest-plan.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::ملاحظة المصدر القياسي
تعكس `docs/source/da/ingest_plan.md`. ابق النسختين متزامنتين حتى يتم سحبه
الوثائق القديمة.
:::

#خطة استيعاب لتوفر البيانات في سورا Nexus

_مسودة: 2026-02-20 - المالك: Core Protocol WG / Storage Team / DA WG_

يمدد مسار DA-2 منصة Torii بواجهة ingest للـ blobs ستكشف بيانات Norito الوصفية
وتبذر SoraFS. يوثق هذا المستند المقترح وسطح API وتدفق التحقق حتى
يتقدم التنفيذ دون توقع محاكاة DA-1 المتبقية. يجب ان تستخدم جميع التنسيقات
الحمولة ترميزات Norito؛ لا يسمح باي احتياطي الى serde/JSON.

## الاهداف

- تقبل النقط كبيرة (قطاعات Taikai، Sidecars الحارات، وادوات و) بشكل حتمي
  عبر Torii.
- انتاج بيانات Norito معيارية تصف الـ blob ومعلمات الترميز ومحو الملف
  وسياسة الإصلاح.
- حفظ البيانات الوصفية في التخزين SoraFS تعليمات المهام التكرارية في
  الطابور.
- نشر نوايا pin + وسوم السياسة في السجل SoraFS ومراقبي التصفح.
- اتاحة الفرصة للإيصالات كي يستعيد العملاء دليلا هتميا على النشر.

## سطح API (Torii)

```
POST /v1/da/ingest
Content-Type: application/norito+v1
```

الحمولة هي `DaIngestRequest` مشفرة بـ Norito. تستخدم الاستجابات
`application/norito+v1` وتعيد `DaIngestReceipt`.

| الشكل | المعنى |
| --- | --- |
| 202 مقبول | تم وضع الـ blob في طابور التغذية/التكرار؛ تم ارجاع الاستلام. |
| 400 طلب سيء | لتحسين المخطط/الحجم (انظر فحوصات التحقق). |
| 401 غير مصرح به | رمز API مفقود/غير صالح. |
| 409 الصراع | ملحوظة `client_blob_id` مع بيانات وصفية غير ملائمة. |
| 413 الحمولة كبيرة جدًا | تجاوز حد طول الـ blob المكون. |
| 429 طلبات كثيرة جدًا | تم تقدم الحد الأقصى للمعدل. |
| 500 خطأ داخلي | فشل غير متوقع (سجل + تنبيه). |

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

> ملاحظة تنفيذية قياسية: تم نقل مؤثرات الصدأ لهذه الحمولات الى
> `iroha_data_model::da::types`، مع مغلفات للطلب/إيصال في
> `iroha_data_model::da::ingest` وبيان البنية في `iroha_data_model::da::manifest`.

أعلن الباحث `compression` كيف يجهز المتصلون الدعائيون. يقبل Torii `identity` و`gzip`
و`deflate` و`zstd` مع فك الضغط قبل التجزئة والتجزئة والتحقق من البيانات
الاختيارية.

### قائمة التحقق من صحة

1. تحقق من ان ترويسة Norito ليتم تطابق `DaIngestRequest`.
2. فشل إذا كان `total_size` اخترا طول العداد الساقط (بعد فك الضغط) او
   التجاوز الحد الاقصى المكون.
3. يفترض جعفر `chunk_size` (قوة الرجال، <= 2 MiB).
4. ضمان `data_shards + parity_shards` <= الحد الاقصى العالمي والتكافؤ >= 2.
5. يجب ان يحترم `retention_policy.required_replica_count` خط أساسي في الشبكة.
6. قم باختيار التوقيع مقابل الهاش الكلاسيكي (مع استبعاد التوقيع).
7. الرفض `client_blob_id` المكرر ما لم تكن قيمة هاش التخصيص والبيانات الوصفية
   متطابقة.
8. عند توفير `norito_manifest`، تحقق من مطابقة المخطط + التجزئة للـ مانيفست المعاد
   حسابه بعد التغذية؛ ويقوم بعقدة بتوليد المانيفست وتخزينه.
9. تطبيق أبو التكرار يشير إلى: إعادة كتابة Torii كتابة `RetentionPolicy` المرسل عبر
   `torii.da_ingest.replication_policy` (راجع `replication-policy.md`) ويرفض
   بيانات المجهزة المسبقة اذا لم تتطابق مع بيانات الملف المفروض.

### تدفق التغذية والتكرار

1. تجزئة الحمولة الى `chunk_size`، وحساب BLAKE3 لكل قطعة + جذر ميركل.
2. بناء Norito `DaManifestV1` (بنية جديدة) تلتقط الـ Chunk
   (role/group_id)، وتخطيط المحو (اعداد تكافؤ الصفوف والاعمدة معها
   `ipa_commitment`)، وسياسة الإصلاح والبيانات الوصفية.
3. وضع البايتات الـ البيان الكلاسيكي تحت `config.da_ingest.manifest_store_dir`
   (يكتب Torii ملفات `manifest.encoded` حسب الممر/العصر/التسلسل/التذكرة/البصمة)
   حتى يبدأ عملها SoraFS من ابتلاعها وربط تذكرة التخزين بالبيانات المحفوظة.
4. نشر نوايا دبوس عبر `sorafs_car::PinIntent` مع وصلة ال تور والسياسة.
5. بث مباشر حدث Norito `DaIngestPublished` لاخطار الشبكة (عملاء خفيفين، الـ تورين،
   التحليلات).
6. ارجاع `DaIngestReceipt` للمتصل (موقع بمفتاح خدمة Torii DA) وارسال ترويسة
   `Sora-PDP-Commitment` كي تلتقط SDKs الالتزام بالم قطع نهائيًا. يشملـ الإيصال
   الان `rent_quote` (Norito `DaRentQuote`) و`stripe_layout`
   المرسلين من عرض الاجرة الاساسية، وحصة احتياطية، وتوقعات مكافاة PDP/PoTR
   وتخطيط محو البيانات الثنائية الابعاد بجانب تذكرة التخزين قبل الالتزام بالاموال.

## تحديثات التخزين/السجل

- النهائي `sorafs_manifest` بـ `DaManifestV1` اعراب الحتمي.
- اضافة تيار جديد للتسجيل `da.pin_intent` مع وظيفه اذا تشير الى hash
  المانيفست + معرف التذكرة.
- تحديث خطط العمل لمكافحة الاستيعاب، والإنتاجية الصافية، وباك لوج
  التكرار، وعدد التخفيضات.

## استراتيجية بحث

- العناصر المستخدمة من المخطط، وفحوصات التوقيع، وكشف التكرار.
- السيولة الذهبية لتاكيد ترميز Norito لـ `DaIngestRequest` وmanifest وreceipt.
- تسخير تكامل SoraFS + التسجيل وهمي، ويتحقق من تدفقات قطعة + دبوس.
- خصائص تغطية ملفات المحو والتركيبات العشوائية.
- Fuzzing لحمولات Norito للحماية من البيانات الوصفية.

## ادوات CLI و SDK (DA-8)

- `iroha app da submit` (مدخل CLI ديم جديد) يلف الان builder/publisher ingest
  حتى يبدأوا في إدخال النقط المباشرة خارج مسار حزمة Taikai. عش
  اوامر في `crates/iroha_cli/src/commands/da.rs:1` ومستهلك حمولة وملف
  محو/الاحتفاظ بالبيانات الوصفية الصغيرة/بيان اختيارية قبل التوقيع
  `DaIngestRequest` القياسي بمفتاح تهيئة CLI. تحفظ التشغيلات الناجحة
  `da_request.{norito,json}` و`da_receipt.{norito,json}` تحت
  `artifacts/da/submission_<timestamp>/` (تجاوز عبر `--artifact-dir`) لكي
  اتهمت القطع الأثرية بإصدار بايت Norito الدقيقة المستخدمة أثناء تناولها.
- يستخدم الأمر الافتراضي `client_blob_id = blake3(payload)` لكنه يقبل التجاوزات
  عبر `--client-blob-id`، ويحترم خرائط البيانات الوصفية JSON (`--metadata-json`) و
  البيانات الجميلة (`--manifest`)، ويدعم `--no-submit` للتحضير دون اتصال مع
  `--endpoint` لمضيفي Torii مخصصين. يطبع إيصال JSON الى stdout اضافة الى
  كتابه على القرص، مما يغلق متطلب الأدوات "submit_blob" في DA-8 ويفتح العمل
  تكافؤ SDK.
- `iroha app da get` alias متوجّه لـ DA للمشغل متعدد الاحتياجات الذي يحسن العمل
  `iroha app sorafs fetch`. يمكن للمشغلين توجيهه الى بيان التحف + خطة القطع
  (`--manifest`, `--plan`, `--manifest-id`) **او** تذكرة تخزين من Torii
  عبر `--storage-ticket`. عند استخدام مسار التذكرة، تقوم CLI بجلب البيان من
  `/v1/da/manifests/<ticket>`، وتخزن الصندوق تحت `artifacts/da/fetch_<timestamp>/`
  (تجاوز مع `--manifest-cache-dir`)، وتشتق hash الـ blob لـ `--manifest-id`،
  ثم تشغل الأوركسترا مع قائمة `--gateway-provider` المعطاة. تبقى جميع المقابض
  المتقدمة من جالب SoraFS كما هي (مظاريف البيان، ملصقات العميل، الحارس
  ذاكرة التخزين المؤقت، تتجاوز نقل مجهول، تصدير لوحة النتائج، ومسارات `--output`)، ويمكن
  استبدال نقطة نهاية البيان عبر `--manifest-endpoint` لمضيفي Torii لسببين، لذا
  تعيش فحوصات التوفر من النهاية للنهاية ضمن مساحة `da` دون شمالي
  منسق.
- `iroha app da get-blob` يتم سحب البيانات القياسية مباشرة من Torii عبر
  `GET /v1/da/manifests/{storage_ticket}`. كتب الأمر
  `manifest_{ticket}.norito` و`manifest_{ticket}.json` و`chunk_plan_{ticket}.json`
  تحت `artifacts/da/fetch_<timestamp>/` (او `--output-dir` يحدده المستخدم) مع
  طباعة امرر `iroha app da get` الصباح (بما في ذلك `--manifest-id`) المطلوب لجلب
  منسق. يبقي هذا التشغيل بعيدًا عن بكرة مضاعفة ويضمن ان
  يستخدم الجلب دائما المصنوعات اليدوية المصدر عن Torii. يعكس عميل Torii
  في جافا سكريبت هذا الكريم عبر `ToriiClient.getDaManifest(storageTicketHex)`،
  ويعيد بايت Norito المفكوكة وmanifest JSON وchunk Plan ليتمكن المتصلون في SDK
  من تهيئة جلسات الأوركسترا دون استخدام CLI. المعروض SDK Swift الان نفس
  الا سطح (`ToriiClient.getDaManifestBundle(...)` مع
  `fetchDaPayloadViaGateway(...)`)، موجها الحزم الى غلاف أوركسترا SoraFS
  الأساسي حتى يستهدف عملاء iOS من تنزيل البيانات أن يجلبوا فوائد متعددة
  وقاطع المعادلة دون الاتصال CLI.
  [IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240][IrohaSwift/Sources/IrohaSwift/SorafsOrchestratorClient.swift:12]
- `iroha app da rent-quote` يحسب رينت حتمي وفصيلة الحوافز لحجم تخزين ونافذة احتفاظ
  قبل. يستهلك المساعدة `DaRentPolicyV1` (JSON او bytes Norito) او افتراضيا
  نظام، ويتحقق من السياسة ويطبع ملخص JSON (`gib`، `months`، بيانات السياسة،
  وقول `DaRentQuote`) حتى يتقدم المفاضلون الاستراليون برسوم XOR الدقيقة في
  محاضر ال تور دون نصوص مخصصة. كما يأتي الأمر ملخصا من سطر واحد
  `rent_quote ...` قبل تكتيكي JSON على قراءة أرشيفات الأحداث اللاحقة
  عمليات حيث. قم بقرن `--quote-out artifacts/da/rent_quotes/<stamp>.json`
  مع `--policy-label "governance ticket #..."` حفظ التحف منسقة تشير الى
  تصويت السياسة او حزمة التهيئة؛ تقوم CLI بقص الوسم ورفض قوائم الفارغة
  حتى تبقى قيم `policy_source` قابلة للتنفيذ في لوحات خزينة. إعادة النظر
  `crates/iroha_cli/src/commands/da.rs` للامر العميق و
  `docs/source/da/rent_policy.md` لمخطط السياسة.
  [صناديق/iroha_cli/src/commands/da.rs:1][docs/source/da/rent_policy.md:1]
- `iroha app da prove-availability` يربط كل ما سبق: ياخذ تذكرة تخزين، ينزل الحزمة
  واضح جيد جدًا، مُنسق متعدد المصادر (`iroha app sorafs fetch`) مقابل
  قائمة `--gateway-provider` الطاطة، ويحفظ الحمولة التي تم تنزيلها + لوحة النتائج
  تحت `artifacts/da/prove_availability_<timestamp>/`، ويستدعي مباشرة مساعد PoR
  الحالي (`iroha app da prove`) باستخدام البايتات المحملة. يستطيع تشغيل المقابض
  منسق (`--max-peers`, `--scoreboard-out`, يتجاوز لعنوان البيان) و
  جهاز أخذ العينات للاثبات (`--sample-count`, `--leaf-index`, `--sample-seed`) بينما يخرج
  أمر واحد artefacts مطلوب لدقيقات DA-5/DA-9: نسخة من الحمولة، دليل
  لوحة النتائج، وملخصات اثبات JSON.