---
lang: ar
direction: rtl
source: docs/portal/docs/da/ingest-plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note Канонический источник
هذا الجزء يعرض `docs/source/da/ingest_plan.md`. آخر الإصدارات
:::

# خطة استيعاب توفر البيانات Sora Nexus

_التاريخ: 2026/02/20 — المشرفون: فريق عمل البروتوكول الأساسي / فريق التخزين / DA WG_

يتم توصيل نقطة العمل DA-2 Torii API للنقطة التي يتم إرسالها
Norito هو تحويل ونسخ متماثل SoraFS. الوثيقة التي تصف المقترحات
النظام ومنطقة واجهة برمجة التطبيقات (API) والتحقق السريع من صحة التنفيذ بدون حجب
المحاكاة الفيزيائية (متابعة DA-1). يتم استخدام جميع تنسيقات الحمولة
الكود Norito; لا يتم توفير البديل الاحتياطي/JSON.

## كيلي

- إنشاء أكبر نقطة (أجزاء Taikai، ممر جانبي، إدارة القطع الأثرية)
  تم تحديده من خلال Torii.
- إنشاء قوائم Norito الأساسية، وتسجيل البيانات الثنائية الكبيرة، ومعلمات الترميز،
  محو الملف الشخصي والاحتفاظ بالسياسة.
- حفظ القطعة المتغيرة في وحدة التخزين الساخنة SoraFS وتثبيت المزيد من النسخ المتماثلة في
  مرة أخرى.
- نشر نوايا الدبوس + علامات السياسة في المسجل SoraFS وراجعها
  الإدارة.
- التحقق من إيصالات القبول حتى يتمكن العملاء من اتخاذ القرار
  متابعة النشر.

## سطح واجهة برمجة التطبيقات (Torii)

```
POST /v1/da/ingest
Content-Type: application/norito+v1
```

الحمولة — هذا `DaIngestRequest`, закодированный Norito. تستخدم الإجابات
`application/norito+v1` ثم قم بالتعبير عن `DaIngestReceipt`.

| رد | تهنئة |
| --- | --- |
| 202 مقبول | يتم إرسال النقطة إلى الجزء الخلفي من التقطيع/النسخ المتماثل؛ تم استلام الإيصال. |
| 400 طلب سيء | المخطط/الحجم المصغر (sm.proverки). |
| 401 غير مصرح به | إجابة/رمز واجهة برمجة التطبيقات غير الصحيح. |
| 409 الصراع | قم بنسخ `client_blob_id` باستخدام غير قابل للتحويل. |
| 413 الحمولة كبيرة جدًا | blob الحد السابق. |
| 429 طلبات كثيرة جدًا | الحد الأقصى للسعر السابق. |
| 500 خطأ داخلي | الاشتراك غير المشروط (سجل + تنبيه). |

## اقتراح مخطط Norito

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

> المساعدة في التحقيق: الحمولة الأساسية التي توفرها هذه الحمولة
> الوصول إلى `iroha_data_model::da::types`، مع أغلفة الطلب/الإيصال в
> `iroha_data_model::da::ingest` والبيان الهيكلي в
> `iroha_data_model::da::manifest`.

يوضح `compression` كيف يرسل المتصل الحمولة. تم تنفيذ Torii
`identity`، `gzip`، `deflate` و`zstd`، تجميع البيانات قبل التجزئة،
التقطيع والبيانات الاختيارية.

### التحقق من صحة قائمة الاختيار

1. تحقق من أن القفل Norito يطابق `DaIngestRequest`.
2. حسنًا، إذا تم حذف `total_size` من حمولة الخطوط القياسية
   (بعد المراسلات) أو الحد الأقصى السابق.
3. قم ببدء تشغيل `chunk_size` (خطوة بخطوة، <= 2 MiB).
4. Убедиться, что `data_shards + parity_shards` <= глобального maxимума и
   التكافؤ >= 2.
5.`retention_policy.required_replica_count` يجب الالتزام بخط الأساس للحوكمة.
6. التحقق من التجزئة القانونية (بدون مشاركة).
7. إلغاء نقرات `client_blob_id`، في حالة عدم استخدام حمولة التجزئة والتحويل
   متطابقة.
8. عند الانتهاء من `norito_manifest`، تحقق من المخطط + التجزئة الملحقة به
   واضح، متقطع بعد التقطيع؛ هناك نوع آخر من البيان و
   ساهم في نفسك.
9. تنفيذ النسخ المتماثلة للسياسة الأساسية: Torii إعادة النسخ
   `RetentionPolicy` من خلال `torii.da_ingest.replication_policy` (سم.
   `replication-policy.md`) وإلغاء استنساخ بيانات المنشأة، إذا كانت كذلك
   لا يتم دمج الاحتفاظ التحويلي مع الملف الشخصي القسري.

### التقطيع والتكرار

1. تحديد الحمولة على `chunk_size`، قم بتعيين BLAKE3 لكل قطعة + Merkle
   root.
2. تنسيق Norito `DaManifestV1` (بنية جديدة)، قطعة الالتزام الثابتة
   (الدور/معرف_المجموعة)، تخطيط المحو (خطوط الجدار والأعمدة الإضافية)
   `ipa_commitment`)، الاحتفاظ بالسياسة والتحويل.
3. قم بنشر البايتات الأساسية في الأعلى
   `config.da_ingest.manifest_store_dir` (Torii أرسل `manifest.encoded` إلى
   الممر/العصر/التسلسل/التذكرة/بصمة الإصبع)، من أجل تنظيم SoraFS
   قم بتصفحها وإرسال تذكرة التخزين باستخدام البيانات المخزنة.
4. نشر نوايا الدبوس من خلال `sorafs_car::PinIntent` باستخدام نفس الإدارة و
   سياسي.
5. قم بإلغاء تحديد Norito `DaIngestPublished` للمراقبة
   (العملاء الخفيفون، الحوكمة، التحليلات).
6. قم بالاتصال بـ `DaIngestReceipt` المتصل (يدعم المفتاح Torii DA) وقم بالإجابة
   заголовок `Sora-PDP-Commitment`، لحصول SDK على الالتزام بسرعة. استلام
   يتضمن الخط `rent_quote` (Norito `DaRentQuote`) و`stripe_layout`، لذلك
   يمكن للمسؤولين أن يعلنوا عن أساس الربح، وحجز الأموال، ومكافآت المكافأة
   يعد تخطيط PDP/PoTR ومسح ثنائي الأبعاد بمثابة تذكرة تخزين لملفات التثبيت.

## تجديد التخزين / التسجيل

- rasshirить `sorafs_manifest` `DaManifestV1` الجديد، محدد التحديد
  تحليل.
- إضافة دفق التسجيل الجديد `da.pin_intent` مع حمولة الإصدار،
  يتم الوصول إلى تجزئة البيان + معرف التذكرة.
- إنشاء شبكات مراقبة للمراقبة من خلال الاستيعاب والإنتاجية
  التقطيع، وتراكم النسخ المتماثلة، وأجهزة الكمبيوتر.

## اختبار الإستراتيجية

- اختبارات الوحدة للتحقق من صحة المخطط، والتحقق من صحة الوصف، والكشف عن النسخ.
- الاختبارات الذهبية للتحقق من ترميز Norito `DaIngestRequest` والبيان والإيصال.
- تسخير التكامل، محاكاة وهمية SoraFS + التسجيل والتحقق
  قطعة قطعة + دبوس.
- اختبارات الملكية لمحو الملفات الشخصية والاحتفاظ بها.
- الحمولة النافعة Norito لحماية الطرق غير الصحيحة.

## أدوات CLI وSDK (DA-8)- `iroha app da submit` (نقطة دخول CLI الجديدة) التي تم إنشاؤها بواسطة Ingest builder/
  الناشر، لكي يتمكن المشغلون من استيعاب النقط الإنتاجية في الخارج
  حزمة تايكاي. يتم توجيه الأمر إلى `crates/iroha_cli/src/commands/da.rs:1` و
  قم بتجريب الحمولة ومحو الملف الشخصي/الاحتفاظ به والملفات الاختيارية
  البيانات الوصفية/البيان قبل إضافة المفتاح القانوني `DaIngestRequest`
  تكوينات CLI. خزنة الإيداع الخاصة `da_request.{norito,json}` و
  `da_receipt.{norito,json}` تحت `artifacts/da/submission_<timestamp>/`
  (التجاوز عبر `--artifact-dir`)، لإصلاح العناصر المصطنعة بشكل جيد
  Norito بايت، يتم استخدامه عند الاستيعاب.
- يستخدم أمر التحكم `client_blob_id = blake3(payload)`، لا
  قم بمتابعة التجاوزات من خلال `--client-blob-id`، بيانات تعريف بطاقة JSON
  (`--metadata-json`) والبيانات التي تم إنشاؤها مسبقًا (`--manifest`)، وأيضًا
  `--no-submit` للمضيفين غير المتصلين و`--endpoint` للمضيفين Torii.
  يتم إرسال إيصال JSON إلى stdout ويتم إدخاله على القرص، مما يؤدي إلى إغلاق DA-8
  "submit_blob" وعمل تكافؤ SDK.
- `iroha app da get` يضيف الاسم المستعار DA-orientirovanny للمنسق متعدد المصادر،
  quetorый питает `iroha app sorafs fetch`. يمكن للمشغلين اكتشاف المصنوعات اليدوية
  البيان + خطة القطع (`--manifest`، `--plan`، `--manifest-id`) **أو** إعادة
  تذكرة التخزين Torii من خلال `--storage-ticket`. При использовании ticket CLI
  قم بتعبئة البيان من `/v1/da/manifests/<ticket>`، وقم بإدراج الحزمة في
  `artifacts/da/fetch_<timestamp>/` (تجاوز باستخدام `--manifest-cache-dir`)، قم بالتغيير
  تجزئة النقطة لـ `--manifest-id` وإغلاق المنسق مع الأسف
  `--gateway-provider` سبيسكوم. جميع المقابض الرائعة من جلبة SoraFS
  сограняются (المظاريف الواضحة، تسميات العميل، مخابئ الحراسة، المجهول
  تجاوزات النقل، لوحة نتائج التصدير، `--output`، نقطة نهاية واضحة
  يمكن إجراء إعادة صياغة عبر `--manifest-endpoint` لمضيف Torii المخصص،
  كما أن التوفر الشامل يتحقق من الحياة في مساحة الاسم `da` بدون
  دوبليروفانييا أوركسترا المنطق.
- `iroha app da get-blob` تختار البيانات الكنسي من Torii من خلال
  `GET /v1/da/manifests/{storage_ticket}`. أمر القيادة
  `manifest_{ticket}.norito`، `manifest_{ticket}.json` و`chunk_plan_{ticket}.json`
  في `artifacts/da/fetch_<timestamp>/` (أو `--output-dir`)
  هذا هو الأمر الصحيح `iroha app da get` (بما في ذلك `--manifest-id`)، مطلوب
  لجلب المنسق التالي. إنه يستخرج مشغلين من العمل
  مجلدات التخزين المؤقت الواضحة والضمانات التي ستستخدمها أداة الجلب دائمًا
  المصنوعات اليدوية المنشورة Torii. عميل جافا سكريبت Torii يستعيد هذا الطريق عبر الإنترنت
  `ToriiClient.getDaManifest(storageTicketHex)`، وحدة فك التشفير Norito
  البايتات وبيان JSON وخطة القطعة التي يمكن لمتصلي SDK مناقشتها
  منسق الجلسة بدون CLI. يوفر Swift SDK ثباتًا أفضل
  (`ToriiClient.getDaManifestBundle(...)` и `fetchDaPayloadViaGateway(...)`),
  تعزيز الحزمة في المجمع الأصلي SoraFS منسق، بحيث يمكن لعملاء iOS
  تنزيل البيانات واستخدام الجلب متعدد المصادر والاشتراك في التنزيلات بدون
  вызова CLI.
  [IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240][IrohaSwift/Sources/IrohaSwift/SorafsOrchestratorClient.swift:12]
- `iroha app da rent-quote` يتم تحديد حوافز الإيجار والحوافز اللازمة
  مساحة تخزين جيدة واحتفاظ جيد. النشاط المساعد الذي تريده
  `DaRentPolicyV1` (JSON أو Norito بايت) أي افتراضي أساسي، تم التحقق من صحته
  السياسة وتلقي رسائل JSON (`gib` و`months` وبيانات تعريف السياسة والمسح)
  `DaRentQuote`)، لكي يتمكن المدققون من التحقق من رسوم XOR الدقيقة في البروتوكول
  التحكم بدون البرامج النصية المخصصة. الأمر أيضًا هو الحذف الفردي
  `rent_quote ...` يسبق حمولة JSON لشعار التصفح خلال التدريبات.
  تواصل مع `--quote-out artifacts/da/rent_quotes/<stamp>.json` مع
  `--policy-label "governance ticket #..."`، لحفظ القطع الأثرية الدقيقة
  مع سهولة الوصول أو حزمة التكوين؛ CLI обезает пользоваtelьскую
  من خلال التغلب على السكتات الدماغية المستمرة التي تجعل `policy_source` مرتاحًا
  لوحة القيادة. سم. `crates/iroha_cli/src/commands/da.rs` للأوامر و
  `docs/source/da/rent_policy.md` للمخططات السياسية.
  [صناديق/iroha_cli/src/commands/da.rs:1][docs/source/da/rent_policy.md:1]
- `iroha app da prove-availability` أتبع كل شيء: بطاقة تخزين تذكرة،
  قم بتنزيل حزمة البيانات الأساسية، ثم أغلق منسقًا متعدد المصادر
  (`iroha app sorafs fetch`) ضد القائمة `--gateway-provider`، المشاركة
  تحميل الحمولة + لوحة النتائج в `artifacts/da/prove_availability_<timestamp>/`,
  وسرعان ما تم إنشاء مساعد PoR (`iroha app da prove`) مع المستفيدين
  بايت. يمكن للمشغلين إنشاء مقابض الأوركسترا (`--max-peers`،
  `--scoreboard-out`، تجاوزات نقطة النهاية الواضحة) وأخذ عينات الإثبات
  (`--sample-count`، `--leaf-index`، `--sample-seed`)، قبل هذا الأمر
  شراء المصنوعات اليدوية، عمليات التدقيق المطلوبة DA-5/DA-9: حمولة النسخ، المطابقة
  لوحة النتائج وإثبات السيرة الذاتية JSON.