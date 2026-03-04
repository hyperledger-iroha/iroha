---
lang: ar
direction: rtl
source: docs/portal/docs/da/ingest-plan.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::ملاحظة مستند ماخذ
لنقم بإلغاء مزامنة المزامنة.
:::

# Sora Nexus خطة استيعاب توفر البيانات

_مسودہ: 2026-02-20 -- مالک: Core Protocol WG / Storage Team / DA WG_

يعمل DA-2 على Torii وهو عبارة عن blob ingest API يشتمل على معلومات وNorito
جارى كارت و SoraFS نسخة طبق الأصل من كرت البذور. هذا مخطط مجوز،
سطح واجهة برمجة التطبيقات (API)، وتدفق التحقق من الصحة، بالإضافة إلى التنفيذ الكامل
عمليات المحاكاة (متابعات DA-1) تمام تنسيقات الحمولة کو
تم استخدام برامج الترميز Norito بشكل ضروري؛ لا يُسمح باحتياطي serde/JSON.

## اداف

- النقط (قطاعات Taikai، الممرات الجانبية، المصنوعات اليدوية للحوكمة) Torii
  ذریعے قبول حتمي کرنا۔
- النقطة، ومعلمات برنامج الترميز، وملف تعريف المسح، وسياسة الاحتفاظ بالبيانات
  والے الكنسي Norito يظهر تیار کرنا۔
- البيانات التعريفية للقطعة SoraFS تعمل على تحسين وظائف التخزين والنسخ المتماثل
  أدرج کرنا۔
- نوايا الدبوس + علامات السياسة في التسجيل SoraFS ومراقبي الإدارة
  نشر کرنا۔
- إيصالات القبول فراہم کرنا تکہ العملاء کو دليل قاطع على
  منشور مل سکے۔

## سطح واجهة برمجة التطبيقات (Torii)

```
POST /v1/da/ingest
Content-Type: application/norito+v1
```

الحمولة النافعة Norito المرمزة `DaIngestRequest` ہے. الردود `application/norito+v1`
استخدم البطاقة و`DaIngestReceipt` بطاقة الاتصال.

| الرد | مطلب |
| --- | --- |
| 202 مقبول | يقوم Blob بتجميع/تقطيع قائمة انتظار النسخ المتماثل؛ إيصال واپس۔ |
| 400 طلب سيء | انتهاك المخطط/الحجم (عمليات التحقق من الصحة دیکھیں). |
| 401 غير مصرح به | رمز واجهة برمجة التطبيقات (API) موجود. |
| 409 الصراع | `client_blob_id` الإصدارات والبيانات الوصفية المختلفة. |
| 413 الحمولة كبيرة جدًا | الحد الأقصى لطول النقطة التي تم تكوينها سے تجاوز۔ |
| 429 طلبات كثيرة جدًا | وصل حد المعدل ۔ |
| 500 خطأ داخلي | غير المتوقع فشل (سجل + تنبيه)۔ |

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

> ملاحظة التنفيذ: ان الحمولات کی تمثيلات الصدأ الأساسية اب
> `iroha_data_model::da::types` تحت، أغلفة الطلب/الإيصال
> `iroha_data_model::da::ingest`، وبنية البيان
> `iroha_data_model::da::manifest` .

`compression` حقل بيانات المتصلين ليس لديهم حمولة. Torii
`identity`، و`gzip`، و`deflate`، و`zstd` تقبل كرتا ہے والتجزئة والتقطيع و
تحقق البيانات الاختيارية من عدد البايتات التي يتم فك ضغطها.

### قائمة التحقق من الصحة

1. تحقق من أن الطلب Norito header `DaIngestRequest` يتطابق مع العنوان.
2. إذا كان `total_size` طول الحمولة الأساسية (غير المضغوطة) مختلفًا أو مختلفًا
   تم تكوين الحد الأقصى لعدد كبير جدًا ولن تفشل.
3. فرض محاذاة `chunk_size` (قوة اثنين، <= 2 MiB).
4. `data_shards + parity_shards` <= الحد الأقصى العالمي والتكافؤ >= 2 نقاط.
5. `retention_policy.required_replica_count` احترام خط الأساس للحوكمة.
6. التجزئة الأساسية کے خلاف التحقق من التوقيع (حقل التوقيع کے بیر)۔
7. ترفض التكرارات `client_blob_id` تجزئة الحمولة الصافية + بيانات التعريف المتطابقة.
8. إذا قمت `norito_manifest` بإدراج المخطط + التجزئة والتقطيع بعد ذلك
   مطابقة البيان المعاد حسابه؛ إنشاء بيان العقدة مخزن کر کے.
9. فرض سياسة النسخ المتماثل المكونة: Torii `RetentionPolicy`
   `torii.da_ingest.replication_policy` سے إعادة كتابة کرتا ہے ( `replication-policy.md`
   د. ) والبيانات المعدة مسبقًا والتي ترفض الكلمات أو في حالة الاحتفاظ بالبيانات الوصفية
   الملف الشخصي القسري لا يتطابق.

### تدفق التقطيع والنسخ

1. تتضمن الحمولة النافعة `chunk_size` تقسيمًا للبطاقة وجزءًا كبيرًا من حساب BLAKE3 وحساب جذر Merkle.
2. Norito `DaManifestV1` (هيكل جديد) التزامات جزء جوي (الدور/معرف_المجموعة)،
   تخطيط المحو (عدد تكافؤ الصفوف/الأعمدة + `ipa_commitment`)، سياسة الاستبقاء،
   والبيانات الوصفية تلتقطها.
3. بايتات البيان الأساسية `config.da_ingest.manifest_store_dir` موجودة في قائمة الانتظار
   (Torii `manifest.encoded` حارة الملفات/العصر/التسلسل/التذكرة/بصمة الإصبع کے لحاظ سے لکھتا ہے)
   تعمل مزامنة SoraFS على استيعاب تذكرة تخزين وارتباط بيانات مستمر.
4. `sorafs_car::PinIntent` علامة الحوكمة + سياسة عدم نشر نوايا الدبوس.
5. الحدث Norito `DaIngestPublished` يصدر مراقبين (عملاء خفيفين، والحوكمة، والتحليلات)
   کو اطلاع ملے۔
6. `DaIngestReceipt` المتصل بالواتس اب (Torii DA مفتاح الخدمة موقّع) و
   يُصدر رأس `Sora-PDP-Commitment` أدوات تطوير البرامج (SDK) على الفور لالتقاط الالتزام.
   يشمل إيصال الاستلام `rent_quote` (Norito `DaRentQuote`) و`stripe_layout`،
   جس سے الإيجار الأساسي للمرسلين، الحصة الاحتياطية، توقعات مكافأة PDP/PoTR وثنائية الأبعاد
   تخطيط المحو وتذكرة التخزين والأموال التي تلتزم بها أكثر من مرة.

## تحديثات التخزين / التسجيل

- `sorafs_manifest` و `DaManifestV1` يوسع نطاق التحليل الحتمي.
- يتضمن دفق التسجيل الجديد `da.pin_intent` حمولة الإصدار
  تجزئة البيان + معرف التذكرة ومرجع كرتا ہے۔
- خطوط أنابيب المراقبة کو استيعاب الكمون، تقطيع الإنتاجية، تراكم النسخ المتماثل
  والفشل مهم في كل مرة.

## استراتيجية الاختبار

- التحقق من صحة المخطط، والتحقق من التوقيع، والكشف عن التكرارات، واختبارات الوحدة.
- ترميز Norito کے الاختبارات الذهبية (`DaIngestRequest`، البيان، الاستلام).
- حزام التكامل جو موك SoraFS + سجل التسجيل وقطعة + تدفقات الدبوس التحقق من كرتا .
- تغطي اختبارات الخصائص ملفات تعريف المحو العشوائية ومجموعات الاستبقاء.
- Norito تشويش الحمولة بسبب بيانات التعريف المشوهة.

## أدوات CLI وSDK (DA-8)- `iroha app da submit` (نقطة دخول CLI الجديدة) اب منشئ/ناشر استيعاب مشترك کو التفاف کرتا ہے تکہ
  يقوم مشغلو Taikai بتدفق الحزمة باستخدام النقط التعسفية التي تستوعب الكرات. إنه رائع
  `crates/iroha_cli/src/commands/da.rs:1` سعة وحمولة وملف تعريف المسح/الاحتفاظ واختياري
  بيانات التعريف/ملفات البيان لمفتاح تكوين CLI الذي تم تسجيله بعلامة `DaIngestRequest` الأساسية.
  يعمل الكمبيوتر على تشغيل `da_request.{norito,json}` و`da_receipt.{norito,json}`
  `artifacts/da/submission_<timestamp>/` تحت المحفوظات (التجاوز عبر `--artifact-dir`)
  الافراج عن المصنوعات اليدوية استيعابها واستخدامها بالضبط Norito بايت سجل.
- التحكم الافتراضي في استخدام `client_blob_id = blake3(payload)` وتجاوز `--client-blob-id`،
  خرائط البيانات الوصفية JSON (`--metadata-json`) والبيانات التي تم إنشاؤها مسبقًا (`--manifest`) وتقبل البطاقة مرة أخرى،
  و `--no-submit` (التحضير دون اتصال) و `--endpoint` (مضيفو Torii المخصصون) الرياضة. إيصال JSON
  ستدوت لطباعة أوتا والقرص للطباعة على جاتا، جيس س DA-8 متطلبات "إرسال_بلوب" بورا
  و يعمل تكافؤ SDK على إلغاء الحظر ہوتا ہے۔
- `iroha app da get` الاسم المستعار الذي يركز على DA فراہم کرتا ہے جو منسق متعدد المصادر کو استخدام کرتا ہے جو پہلے ہی
  `iroha app sorafs fetch` چلاتا ہے۔ بيان المشغلين + عناصر مخطط القطع (`--manifest`، `--plan`، `--manifest-id`)
  **أو** تذكرة تخزين Torii عبر `--storage-ticket`. مسار التذكرة پر CLI `/v1/da/manifests/<ticket>` سے
  تنزيل البيان حزمة واحدة `artifacts/da/fetch_<timestamp>/` محفوظات البطاقة (تجاوز عبر
  `--manifest-cache-dir`)، `--manifest-id` اشتقاق تجزئة blob للبطاقة، وإطار قائمة `--gateway-provider`
  لقد قام المنسق بتشغيل الكرت. SoraFS جلب المقابض المتقدمة لجلب البيانات (مظاريف البيان،
  تسميات العميل، وذاكرة التخزين المؤقت للحراسة، وتجاوزات نقل إخفاء الهوية، وتصدير لوحة النتائج، ومسارات `--output`) اور
  `--manifest-endpoint` تجاوز نقطة النهاية الواضحة، وهو أمر ضروري للتحقق من التوفر الشامل
  مكتمل لمساحة الاسم `da`، هناك تكرار لمنطق المنسق.
- `iroha app da get-blob` Torii سے `GET /v1/da/manifests/{storage_ticket}` ذریعے البيانات القانونية الأساسية.
  التحكم `manifest_{ticket}.norito` و`manifest_{ticket}.json` و`chunk_plan_{ticket}.json`
  `artifacts/da/fetch_<timestamp>/` موجود أيضًا (أو `--output-dir` الذي يوفره المستخدم) و`iroha app da get`
  استدعاء (بشمول `--manifest-id`) صدى كرتي وهو منسق متابعة الجلب كل درکار ہے. مشغلي اس سے
  تم استخدام أدلة التخزين المؤقت الواضحة وأداة الجلب Torii وهي المصنوعات اليدوية الموقعة.
  تم تحديث عميل JavaScript Torii وتدفق `ToriiClient.getDaManifest(storageTicketHex)` وتم فك تشفيره
  Norito بايت، بيان JSON وخطة القطعة واپس كرتا وعدد كبير من المتصلين SDK CLI أعلى جلسات منسق الهيدرات
  كر سكي. Swift SDK أيضًا وأسطح تعرض العناصر (`ToriiClient.getDaManifestBundle(...)` و
  `fetchDaPayloadViaGateway(...)`)، الحزم الموجودة في مجمّع الأوركسترا SoraFS الأصلي تعمل على توصيل الأنابيب لعملاء iOS
  تنزيل البيانات، وجلب الملفات متعددة المصادر، والتقاط البراهين، وجلب CLI.
  [IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240][IrohaSwift/Sources/IrohaSwift/SorafsOrchestratorClient.swift:12]
- `iroha app da rent-quote` حجم التخزين ونافذة الاحتفاظ الإيجار الحتمي وانهيار الحوافز
  حساب كرتا ہے۔ أو مساعد نشط `DaRentPolicyV1` (JSON أو Norito بايت) أو استخدام افتراضي مدمج
  التحقق من صحة السياسة كرتا ہے، وملخص JSON (`gib`، `months`، بيانات تعريف السياسة، حقول `DaRentQuote`) طباعة كرتا ہے
  يتضمن محضر حوكمة المدققين رسوم XOR الدقيقة، ويستشهد بالنصوص البرمجية المخصصة. حمولة JSON
  يوجد ملخص على الإنترنت `rent_quote ...` حتى تكون سجلات وحدة التحكم قابلة للقراءة.
  `--quote-out artifacts/da/rent_quotes/<stamp>.json` إلى `--policy-label "governance ticket #..."`
  يمكن استخدام هذه العناصر الجميلة بشكل صحيح من خلال التصويت على السياسة أو حزمة التكوين الاستشهاد بها؛ تسمية مخصصة CLI کو تقليم
  ترفض المفاتيح والسلاسل الفارغة المفاتيح، كما أن `policy_source` يمكن أن تكون لوحة معلومات القيمة قابلة للتنفيذ. رائع
  `crates/iroha_cli/src/commands/da.rs` (أمر فرعي) و`docs/source/da/rent_policy.md` (مخطط السياسة).
  [صناديق/iroha_cli/src/commands/da.rs:1][docs/source/da/rent_policy.md:1]
- `iroha app da prove-availability` سلسلة مفاتيح السلسلة الأولى: تذكرة تخزين ليتا، حزمة البيان الكنسي
  تنزيل قائمة كرتا، منسق متعدد المصادر (`iroha app sorafs fetch`) قائمة الاختلافات `--gateway-provider`
  كلاتا، الحمولة التي تم تنزيلها + لوحة النتائج `artifacts/da/prove_availability_<timestamp>/` محفوظات كرتا، و
  يتم توفير مساعد PoR فورًا (`iroha app da prove`) ويتم جلب البايتات من خلال استدعاء كرتا. مقابض مشغلي الأوركسترا
  (`--max-peers`، `--scoreboard-out`، تجاوزات نقطة النهاية الواضحة) وأداة أخذ العينات (`--sample-count`، `--leaf-index`،
  `--sample-seed`) ضبط عملية تدقيق الأمر DA-5/DA-9 بعد الانتهاء من عملية تدقيق بعض المصنوعات اليدوية المتأخرة:
  نسخة الحمولة وأدلة لوحة النتائج وملخصات إثبات JSON۔