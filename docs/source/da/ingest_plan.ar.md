---
lang: ar
direction: rtl
source: docs/source/da/ingest_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1bf79d000e0536da04eafac6c0d896b1bf8f0c454e1bf4c4b97ba22c7c7f5db1
source_last_modified: "2026-01-22T15:38:30.661072+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Sora Nexus خطة استيعاب توفر البيانات

_ تمت الصياغة: 20-02-2026 - المالك: مجموعة عمل البروتوكول الأساسي / فريق التخزين / DA WG_

يقوم مسار عمل DA-2 بتوسيع Torii باستخدام واجهة برمجة تطبيقات استيعاب البيانات الثنائية الكبيرة التي تبعث Norito
البيانات الوصفية والبذور SoraFS النسخ المتماثل. هذه الوثيقة تلتقط المقترح
المخطط وسطح واجهة برمجة التطبيقات (API) وتدفق التحقق من الصحة حتى يمكن متابعة التنفيذ بدون
حظر عمليات المحاكاة المتميزة (متابعات DA-1). يجب أن تكون جميع تنسيقات الحمولة
استخدم برامج الترميز Norito؛ لا يُسمح بأي عمليات احتياطية لـ serde/JSON.

## الأهداف

- قبول النقط الكبيرة (قطاعات Taikai، وعربات الحارة الجانبية، ومصنوعات الحكم)
  بشكل حتمي على Torii.
- إنتاج بيانات Norito الأساسية التي تصف النقطة ومعلمات برنامج الترميز،
  ملف تعريف المحو، وسياسة الاحتفاظ.
- استمرار البيانات التعريفية للقطعة في SoraFS للتخزين السريع ومهام النسخ المتماثل.
- نشر نوايا الدبوس + علامات السياسة إلى السجل والإدارة SoraFS
  المراقبين.
- كشف إيصالات القبول حتى يستعيد العملاء إثبات النشر الحتمي.

## واجهة برمجة التطبيقات (Torii)

```
POST /v2/da/ingest
Content-Type: application/norito+v1
```

الحمولة هي Norito بترميز `DaIngestRequest`. استخدام الردود
`application/norito+v1` والعودة `DaIngestReceipt`.

| الرد | معنى |
| --- | --- |
| 202 مقبول | النقطة الموجودة في قائمة الانتظار للتقطيع/النسخ المتماثل؛ تم إرجاع الإيصال. |
| 400 طلب سيء | انتهاك المخطط/الحجم (راجع عمليات التحقق من الصحة). |
| 401 غير مصرح به | رمز API مفقود/غير صالح. |
| 409 الصراع | `client_blob_id` مكرر ببيانات تعريف غير متطابقة. |
| 413 الحمولة كبيرة جدًا | يتجاوز الحد الأقصى لطول الكائن الثنائي الذي تم تكوينه. |
| 429 طلبات كثيرة جدًا | ضرب الحد الأقصى للسعر. |
| 500 خطأ داخلي | فشل غير متوقع (تسجيل + تنبيه). |

```
GET /v2/da/proof_policies
Accept: application/json | application/x-norito
```

إرجاع `DaProofPolicyBundle` المشتق من كتالوج المسار الحالي.
تعلن الحزمة عن `version` (حاليًا `1`)، و`policy_hash` (تجزئة
قائمة السياسات المطلوبة)، وإدخالات `policies` التي تحمل `lane_id`، `dataspace_id`،
`alias`، و`proof_scheme` المفروضة (`merkle_sha256` اليوم؛ ممرات KZG هي
تم رفضه عن طريق الاستيعاب حتى تتوفر التزامات KZG). رأس الكتلة الآن
يلتزم بالحزمة عبر `da_proof_policies_hash`، حتى يتمكن العملاء من تثبيت
تم تعيين السياسة النشطة عند التحقق من التزامات أو إثباتات DA. جلب نقطة النهاية هذه
قبل بناء البراهين للتأكد من مطابقتها لسياسة المسار والحالية
تجزئة الحزمة. تحمل قائمة الالتزام/نقاط النهاية المثبتة نفس الحزمة مثل حزم SDK
لا تحتاج إلى رحلة ذهاب وإياب إضافية لربط الدليل بمجموعة السياسات النشطة.

```
GET /v2/da/proof_policy_snapshot
Accept: application/json | application/x-norito
```

تقوم بإرجاع `DaProofPolicyBundle` الذي يحمل قائمة السياسات المطلوبة بالإضافة إلى أ
`policy_hash` حتى تتمكن مجموعات SDK من تثبيت الإصدار المستخدم عند إنتاج الكتلة. ال
يتم حساب التجزئة عبر صفيف السياسة المشفر Norito ويتغير كلما
تم تحديث `proof_scheme` الخاص بالحارة، مما يسمح للعملاء باكتشاف الانجراف بين
البراهين المخزنة مؤقتا وتكوين السلسلة.

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
```> ملاحظة التنفيذ: تمثيلات Rust الأساسية لهذه الحمولات موجودة الآن
> `iroha_data_model::da::types`، مع أغلفة الطلب/الإيصال في `iroha_data_model::da::ingest`
> والبنية الواضحة في `iroha_data_model::da::manifest`.

يعلن الحقل `compression` عن كيفية قيام المتصلين بإعداد الحمولة. يقبل Torii
`identity`، و`gzip`، و`deflate`، و`zstd`، مما يؤدي إلى فك ضغط البايتات السابقة بشفافية
التجزئة والتقطيع والتحقق من البيانات الاختيارية.

### قائمة التحقق من الصحة

1. تحقق من تطابق رأس الطلب Norito مع `DaIngestRequest`.
2. يفشل إذا كان `total_size` يختلف عن طول الحمولة الأساسية (غير المضغوطة) أو يتجاوز الحد الأقصى الذي تم تكوينه.
3. فرض محاذاة `chunk_size` (قوة اثنين، = 2.
5. يجب أن يحترم `retention_policy.required_replica_count` خط الأساس للحوكمة.
6. التحقق من التوقيع مقابل التجزئة الأساسية (باستثناء حقل التوقيع).
7. ارفض `client_blob_id` المكرر ما لم تكن تجزئة الحمولة + البيانات الوصفية متطابقة.
8. عند تقديم `norito_manifest`، تحقق من إعادة حساب تطابقات المخطط + التجزئة
   واضح بعد التقطيع. وإلا فإن العقدة تنشئ البيان وتخزنه.
9. فرض سياسة النسخ المتماثل التي تم تكوينها: يقوم Torii بإعادة كتابة ما تم إرساله
   `RetentionPolicy` مع `torii.da_ingest.replication_policy` (راجع
   `replication_policy.md`) ويرفض البيانات المعدة مسبقًا والتي يتم الاحتفاظ بها
   البيانات الوصفية لا تتطابق مع الملف الشخصي المفروض.

### تدفق التقطيع والنسخ1. قم بتجميع الحمولة النافعة في `chunk_size`، وقم بحساب BLAKE3 لكل قطعة + جذر Merkle.
2. بناء Norito `DaManifestV1` (بنية جديدة) لالتقاط التزامات القطعة (الدور/معرف_المجموعة)،
   تخطيط المحو (عدد تكافؤ الصفوف والأعمدة بالإضافة إلى `ipa_commitment`)، وسياسة الاستبقاء،
   والبيانات الوصفية.
3. قم بوضع بايتات البيان الأساسية في قائمة الانتظار ضمن `config.da_ingest.manifest_store_dir`
   (Torii يكتب ملفات `manifest.encoded` التي تم مفتاحها بواسطة المسار/العصر/التسلسل/التذكرة/بصمة الإصبع) لذلك SoraFS
   يمكن للتنسيق استيعابها وربط تذكرة التخزين بالبيانات الدائمة.
4. نشر نوايا الدبوس عبر `sorafs_car::PinIntent` باستخدام علامة الإدارة + السياسة.
5. قم بإصدار الحدث Norito `DaIngestPublished` لإعلام المراقبين (العملاء الخفيفون،
   الحوكمة والتحليلات).
6. قم بإرجاع `DaIngestReceipt` (موقع بواسطة مفتاح الخدمة Torii DA) وأضف
   رأس الاستجابة `Sora-PDP-Commitment` الذي يحتوي على تشفير base64 Norito
   من الالتزام المشتق حتى تتمكن حزم تطوير البرامج (SDK) من تخزين بذور العينات على الفور.
   يتضمن الإيصال الآن `rent_quote` (a `DaRentQuote`) و`stripe_layout`
   حتى يتمكن مقدمو الطلبات من عرض التزامات XOR، والحصة الاحتياطية، وتوقعات مكافأة PDP/PoTR،
   وأبعاد مصفوفة المحو ثنائية الأبعاد جنبًا إلى جنب مع البيانات التعريفية لتذكرة التخزين قبل الالتزام بالأموال.
7. بيانات تعريف التسجيل الاختيارية:
   - `da.registry.alias` - سلسلة الاسم المستعار UTF-8 العامة وغير المشفرة لإدخال إدخال تسجيل الدبوس.
   - `da.registry.owner` - سلسلة `AccountId` عامة وغير مشفرة لتسجيل ملكية التسجيل.
   يقوم Torii بنسخ هذه العناصر إلى `DaPinIntent` الذي تم إنشاؤه بحيث يمكن لمعالجة الدبوس النهائي ربط الأسماء المستعارة
   والمالكين دون إعادة تحليل خريطة البيانات الوصفية الأولية؛ يتم رفض القيم المشوهة أو الفارغة أثناء
   استيعاب التحقق من الصحة.

## تحديثات التخزين / التسجيل

- قم بتوسيع `sorafs_manifest` باستخدام `DaManifestV1`، مما يتيح التحليل الحتمي.
- إضافة دفق التسجيل الجديد `da.pin_intent` مع مرجع الحمولة النافعة ذات الإصدار
  تجزئة البيان + معرف التذكرة.
- تحديث خطوط أنابيب المراقبة لتتبع زمن الاستجابة، وتقطيع الإنتاجية،
  تراكم النسخ المتماثل، وتهم الفشل.
- تتضمن استجابات Torii `/status` الآن مصفوفة `taikai_ingest` التي تعرض أحدث
  زمن استجابة التشفير حتى الاستيعاب، والانجراف المباشر، وعدادات الأخطاء لكل (مجموعة، تيار)، تمكين DA-9
  لوحات المعلومات لاستيعاب اللقطات الصحية مباشرة من العقد دون حذف Prometheus.

## استراتيجية الاختبار- اختبارات الوحدة للتحقق من صحة المخطط، والتحقق من التوقيع، والكشف عن التكرارات.
- الاختبارات الذهبية للتحقق من ترميز Norito لـ `DaIngestRequest` والبيان والاستلام.
- أداة التكامل التي تدور حول سجل SoraFS + الوهمي، لتأكيد تدفقات القطعة + الدبوس.
- اختبارات الخصائص التي تغطي ملفات تعريف المحو العشوائية ومجموعات الاحتفاظ.
- تشويش حمولات Norito للحماية من بيانات التعريف المشوهة.
- التركيبات الذهبية لكل فئة blob تعيش تحتها
  `fixtures/da/ingest/manifests/<blob_class>/manifest.{norito.hex,json}` مع قطعة مصاحبة
  القائمة في `fixtures/da/ingest/sample_chunk_records.txt`. الاختبار الذي تم تجاهله
  يقوم `regenerate_da_ingest_fixtures` بتحديث التركيبات، بينما
  يفشل `manifest_fixtures_cover_all_blob_classes` بمجرد إضافة متغير `BlobClass` جديد
  دون تحديث حزمة Norito/JSON. يؤدي هذا إلى إبقاء Torii وSDKs والمستندات صادقة عندما يكون DA-2
  يقبل سطحًا ثنائيًا ضخمًا جديدًا.[fixtures/da/ingest/README.md:1] 【crates/iroha_torii/src/da/tests.rs:2902】

## أدوات CLI وSDK (DA-8)- `iroha app da submit` (نقطة دخول CLI الجديدة) تلتف الآن على منشئ/ناشر الإدخال المشترك حتى يتمكن المشغلون من
  يمكنه استيعاب النقط العشوائية خارج تدفق حزمة Taikai. الأمر يعيش في
  `crates/iroha_cli/src/commands/da.rs:1` ويستهلك الحمولة وملف تعريف المسح/الاحتفاظ و
  البيانات التعريفية/ملفات البيان الاختيارية قبل التوقيع على `DaIngestRequest` الأساسي مع واجهة سطر الأوامر (CLI)
  مفتاح التكوين. تستمر عمليات التشغيل الناجحة `da_request.{norito,json}` و`da_receipt.{norito,json}` ضمن
  `artifacts/da/submission_<timestamp>/` (التجاوز عبر `--artifact-dir`) بحيث يمكن تحرير المصنوعات اليدوية
  قم بتسجيل وحدات البايت Norito الدقيقة المستخدمة أثناء العرض.
- القيمة الافتراضية للأمر هي `client_blob_id = blake3(payload)` ولكنه يقبل التجاوزات عبر
  `--client-blob-id`، يكرم خرائط JSON للبيانات الوصفية (`--metadata-json`) والبيانات التي تم إنشاؤها مسبقًا
  (`--manifest`)، ويدعم `--no-submit` للتحضير دون اتصال بالإضافة إلى `--endpoint` للتخصيص
  المضيفين Torii. تتم طباعة إيصال JSON على stdout بالإضافة إلى كتابته على القرص، مما يؤدي إلى إغلاق الملف
  متطلبات الأدوات DA-8 "submit_blob" وإلغاء حظر عمل تكافؤ SDK.
- يضيف `iroha app da get` اسمًا مستعارًا يركز على DA للمنسق متعدد المصادر الذي يعمل بالفعل
  `iroha app sorafs fetch`. يمكن للمشغلين توجيهه إلى عناصر البيان + المخطط التفصيلي (`--manifest`،
  `--plan`, `--manifest-id`) **أو** ما عليك سوى تمرير تذكرة تخزين Torii عبر `--storage-ticket`. عندما
  يتم استخدام مسار التذكرة، حيث تسحب واجهة سطر الأوامر البيان من `/v2/da/manifests/<ticket>`، وتستمر في الحزمة
  تحت `artifacts/da/fetch_<timestamp>/` (التجاوز بـ `--manifest-cache-dir`)، يشتق البيان **
  hash** لـ `--manifest-id`، ثم يقوم بتشغيل المنسق باستخدام `--gateway-provider` المصاحب
  قائمة. لا يزال التحقق من الحمولة يعتمد على ملخص CAR/`blob_hash` المضمن بينما يكون معرف البوابة
  الآن أصبح تجزئة البيان بحيث يتشارك العملاء والمدققون في معرف blob واحد. جميع المقابض المتقدمة من
  سطح الجلب SoraFS سليم (مظاريف البيان، تسميات العميل، ذاكرات التخزين المؤقت للحماية، نقل عدم الكشف عن هويته
  التجاوزات، وتصدير لوحة النتائج، ومسارات `--output`)، ويمكن تجاوز نقطة النهاية الواضحة عبر
  `--manifest-endpoint` لمضيفي Torii المخصصين، لذلك يتم إجراء عمليات التحقق من التوفر الشامل بالكامل ضمن
  مساحة الاسم `da` دون تكرار منطق المنسق.
- يقوم `iroha app da get-blob` بسحب البيانات الأساسية مباشرة من Torii عبر `GET /v2/da/manifests/{storage_ticket}`.
  يقوم الأمر الآن بتسمية المصنوعات اليدوية باستخدام علامة التجزئة الواضحة (معرف النقطة) والكتابة
  `manifest_{manifest_hash}.norito`، و`manifest_{manifest_hash}.json`، و`chunk_plan_{manifest_hash}.json`
  ضمن `artifacts/da/fetch_<timestamp>/` (أو `--output-dir` الذي يوفره المستخدم) أثناء تكرار صدى الصوت الدقيق
  استدعاء `iroha app da get` (بما في ذلك `--manifest-id`) مطلوب لجلب المنسق للمتابعة.
  يؤدي هذا إلى إبعاد المشغلين عن أدلة التخزين المؤقت للبيان ويضمن أن أداة الجلب تستخدم دائمًا ملف
  المصنوعات اليدوية الموقعة المنبعثة من Torii. يعكس عميل JavaScript Torii هذا التدفق عبر
  `ToriiClient.getDaManifest(storageTicketHex)` أثناء عرض Swift SDK الآن
  `ToriiClient.getDaManifestBundle(...)`. يقوم كلاهما بإرجاع بايتات Norito التي تم فك تشفيرها، وJSON الواضح، وتجزئة البيان،وخطة القطع حتى يتمكن المتصلون بـ SDK من ترطيب جلسات المنسق دون إرسالها إلى CLI وSwift
  يمكن للعملاء أيضًا الاتصال بـ `fetchDaPayloadViaGateway(...)` لتوجيه هذه الحزم عبر الشبكة الأصلية
  غلاف الأوركسترا SoraFS.[IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240]
- تظهر استجابات `/v2/da/manifests` الآن على `manifest_hash`، وكلا مساعدي CLI + SDK (`iroha app da get`،
  `ToriiClient.fetchDaPayloadViaGateway`، ومغلفات بوابة Swift/JS) يتعاملون مع هذا الملخص باعتباره
  معرف البيان الأساسي مع الاستمرار في التحقق من الحمولات مقابل تجزئة CAR/blob المضمنة.
- `iroha app da rent-quote` يحسب الإيجار الحتمي وتفاصيل الحوافز لحجم التخزين المتوفر
  ونافذة الاحتفاظ. يستهلك المساعد إما `DaRentPolicyV1` (بايت JSON أو Norito) أو
  الافتراضي المضمن، والتحقق من صحة السياسة، وطباعة ملخص JSON (`gib`، `months`، بيانات تعريف السياسة،
  والحقول `DaRentQuote`) حتى يتمكن المدققون من الاستشهاد برسوم XOR الدقيقة داخل محاضر الإدارة دون
  كتابة البرامج النصية المخصصة. يقوم الأمر الآن أيضًا بإصدار ملخص `rent_quote ...` من سطر واحد قبل JSON
  الحمولة لتسهيل فحص سجلات وحدة التحكم ودفاتر التشغيل عند إنشاء علامات الاقتباس أثناء الحوادث.
  تمرير `--quote-out artifacts/da/rent_quotes/<stamp>.json` (أو أي مسار آخر)
  للاستمرار في الملخص المطبوع بشكل جميل واستخدام `--policy-label "governance ticket #..."` عندما يكون
  يحتاج المنتج إلى الاستشهاد بحزمة تصويت/تكوين محددة؛ يقوم CLI بقص التسميات المخصصة ويرفضها فارغة
  سلاسل للحفاظ على قيم `policy_source` ذات معنى في حزم الأدلة. انظر
  `crates/iroha_cli/src/commands/da.rs` للأمر الفرعي و`docs/source/da/rent_policy.md`
  لمخطط السياسة.[crates/iroha_cli/src/commands/da.rs:1] 【docs/source/da/rent_policy.md:1】
- يمتد تكافؤ التسجيل Pin الآن إلى SDKs: `ToriiClient.registerSorafsPinManifest(...)` في
  تقوم JavaScript SDK بإنشاء الحمولة الدقيقة المستخدمة بواسطة `iroha app sorafs pin register`، مما يفرض القواعد الأساسية
  البيانات التعريفية للمقطع، وسياسات الدبوس، وإثباتات الأسماء المستعارة، والملخصات اللاحقة قبل النشر إلى
  `/v2/sorafs/pin/register`. يؤدي هذا إلى منع روبوتات CI والأتمتة من الوصول إلى واجهة سطر الأوامر (CLI) عندما
  تسجيل تسجيلات البيان، ويأتي المساعد مزودًا بتغطية TypeScript/README لذا فإن DA-8
  يتم تحقيق تكافؤ الأدوات "إرسال/حصول/إثبات" بشكل كامل على JS جنبًا إلى جنب مع Rust/Swift.
- يقوم `iroha app da prove-availability` بربط كل ما سبق: فهو يأخذ تذكرة تخزين، ويقوم بتنزيل
  تقوم حزمة البيان الأساسية بتشغيل منسق متعدد المصادر (`iroha app sorafs fetch`) ضد
  قائمة `--gateway-provider` المتوفرة، تستمر في الحمولة التي تم تنزيلها + لوحة النتائج أسفل
  `artifacts/da/prove_availability_<timestamp>/`، واستدعاء مساعد PoR الموجود على الفور
  (`iroha app da prove`) باستخدام البايتات التي تم جلبها. يمكن للمشغلين تعديل مقابض الأوركسترا
  (`--max-peers`، `--scoreboard-out`، تجاوزات نقطة النهاية الواضحة) وأداة أخذ عينات الإثبات
  (`--sample-count`، `--leaf-index`، `--sample-seed`) بينما يقوم أمر واحد بإنتاج المصنوعات اليدوية
  المتوقعة من خلال عمليات تدقيق DA-5/DA-9: نسخة الحمولة النافعة، وأدلة لوحة النتائج، وملخصات إثبات JSON.- يقرأ `da_reconstruct` (الجديد في DA-6) البيان الأساسي بالإضافة إلى دليل القطعة المنبعث من القطعة
  تخزين (تخطيط `chunk_{index:05}.bin`) ويعيد تجميع الحمولة بشكل حاسم أثناء التحقق
  كل التزام Blake3. يوجد CLI تحت `crates/sorafs_car/src/bin/da_reconstruct.rs` ويتم شحنه كـ
  جزء من حزمة الأدوات SoraFS. التدفق النموذجي:
  1. `iroha app da get-blob --storage-ticket <ticket>` لتنزيل `manifest_<manifest_hash>.norito` وخطة القطعة.
  2.`iroha app sorafs fetch --manifest manifest_<manifest_hash>.json --plan chunk_plan_<manifest_hash>.json --output payload.car`
     (أو `iroha app da prove-availability`، الذي يكتب جلب العناصر تحت
     `artifacts/da/prove_availability_<ts>/` وتستمر الملفات لكل قطعة داخل الدليل `chunks/`).
  3.`cargo run -p sorafs_car --features cli --bin da_reconstruct --manifest manifest_<manifest_hash>.norito --chunks-dir ./artifacts/da/prove_availability_<ts>/chunks --output reconstructed.bin --json-out summary.json`.

  توجد أداة الانحدار ضمن `fixtures/da/reconstruct/rs_parity_v1/` وتلتقط البيان الكامل
  ومصفوفة القطعة (البيانات + التكافؤ) المستخدمة بواسطة `tests::reconstructs_fixture_with_parity_chunks`. تجديدها مع

  ```sh
  cargo test -p sorafs_car --features da_harness regenerate_da_reconstruct_fixture_assets -- --ignored --nocapture
  ```

  تنبعث الوحدة:

  - `manifest.{norito.hex,json}` — ترميزات `DaManifestV1` الأساسية.
  - `chunk_matrix.json` - صفوف الفهرس/الإزاحة/الطول/الملخص/التكافؤ المطلوبة لمراجع المستند/الاختبار.
  - `chunks/` — شرائح الحمولة `chunk_{index:05}.bin` لكل من البيانات وأجزاء التكافؤ.
  - `payload.bin` — الحمولة الحتمية المستخدمة في اختبار تسخير التكافؤ.
  - `commitment_bundle.{json,norito.hex}` - عينة `DaCommitmentBundle` مع التزام KZG الحتمي للمستندات/الاختبارات.

  يرفض الحزام القطع المفقودة أو المقطوعة، ويتحقق من تجزئة الحمولة النهائية لـ Blake3 مقابل `blob_hash`،
  ويصدر ملخص JSON blob (بايت الحمولة، عدد القطع، تذكرة التخزين) حتى يتمكن CI من تأكيد إعادة الإعمار
  الأدلة. يؤدي هذا إلى إغلاق متطلبات DA-6 الخاصة بأداة إعادة البناء الحتمية التي يستخدمها المشغلون وضمان الجودة
  يمكن استدعاء الوظائف دون توصيل البرامج النصية المخصصة.

## ملخص قرار المهام

تم تنفيذ جميع مهام المهام التي تم حظرها مسبقًا والتحقق منها:- **تلميحات الضغط** — يقبل Torii التسميات المقدمة من المتصل (`identity`، `gzip`، `deflate`،
  `zstd`) وتطبيع الحمولات قبل التحقق من الصحة بحيث يتطابق تجزئة البيان الأساسي مع
  البايتات التي تم فك ضغطها.[crates/iroha_torii/src/da/ingest.rs:220] 【crates/iroha_data_model/src/da/types.rs:161】
- **تشفير البيانات التعريفية للحوكمة فقط** — يقوم Torii الآن بتشفير البيانات التعريفية للحوكمة باستخدام
  تم تكوين مفتاح ChaCha20-Poly1305، ويرفض التسميات غير المتطابقة، ويظهر اثنين واضحين
  مقابض التكوين (`torii.da_ingest.governance_metadata_key_hex`،
  `torii.da_ingest.governance_metadata_key_label`) للحفاظ على حتمية التدوير.
- **تدفق حمولة كبيرة** — يتم البث المباشر لأجزاء متعددة. عملاء تيار حتمية
  الأظرف `DaIngestChunk` التي تم إدخال مفتاح عليها بواسطة `client_blob_id`، Torii تتحقق من صحة كل شريحة، وتنظمها
  تحت `manifest_store_dir`، ويعيد إنشاء البيان ذريًا بمجرد ظهور علامة `is_last`،
  القضاء على ارتفاعات ذاكرة الوصول العشوائي (RAM) التي تظهر مع التحميلات بمكالمة واحدة.
- **إصدار البيان** — يحمل `DaManifestV1` حقل `version` صريحًا ويرفض Torii
  إصدارات غير معروفة، مما يضمن ترقيات حتمية عند شحن تخطيطات البيان الجديدة.
- **خطافات PDP/PoTR** — التزامات PDP مستمدة مباشرة من مخزن القطع وتستمر
  بجانب البيانات حتى يتمكن منظمو جدولة DA-5 من إطلاق تحديات أخذ العينات من البيانات الأساسية؛ ال
  يتم الآن شحن رأس `Sora-PDP-Commitment` مع كل من `/v2/da/ingest` و`/v2/da/manifests/{ticket}`
  الردود حتى تتعلم أدوات تطوير البرامج (SDK) على الفور الالتزام الموقع الذي ستشير إليه التحقيقات المستقبلية.
- **مجلة مؤشر Shard** — قد تحدد بيانات تعريف المسار `da_shard_id` (القيمة الافتراضية هي `lane_id`)، و
  Sumeragi يحتفظ الآن بأعلى `(epoch, sequence)` لكل `(shard_id, lane_id)` في
  `da-shard-cursors.norito` جنبًا إلى جنب مع التخزين المؤقت DA لذا يتم إعادة التشغيل وإسقاط الممرات المعاد تقسيمها/غير المعروفة والاحتفاظ بها
  إعادة الحتمية. يفشل الآن مؤشر مؤشر الجزء الموجود في الذاكرة بسرعة في الالتزامات الخاصة بـ
  الممرات غير المعينة بدلاً من تعيين معرف المسار بشكل افتراضي، مما يؤدي إلى حدوث أخطاء في تقدم المؤشر وإعادة التشغيل
  صريح، ويرفض التحقق من صحة الكتلة انحدارات مؤشر القطعة باستخدام رمز مخصص
  `DaShardCursorViolation` السبب + تسميات القياس عن بعد للمشغلين. بدء التشغيل/اللحاق بالركب يوقف الآن DA
  مؤشر الترطيب إذا كان كورا يحتوي على مسار غير معروف أو مؤشر متراجع ويسجل المخالفة
  ارتفاع الكتلة حتى يتمكن المشغلون من المعالجة قبل تقديم DA الحالة.[الصناديق/iroha_config/src/parameters/actual.rs][الصناديق/iroha_core/src/da/shard_cursor.rs] 【الصناديق/iroha_core/src/ sumeragi/main_loop.rs 】 【crates/iroha_core/src/state.rs 】 【crates/iroha_core/src/block.rs 】 【docs/source/nexus_lanes.md:47】
- ** القياس عن بعد لتأخر مؤشر Shard ** - يوضح مقياس `da_shard_cursor_lag_blocks{lane,shard}` مدىبعيدًا عن الجزء الذي يتتبع الارتفاع الذي يتم التحقق من صحته. الممرات المفقودة/التي لا معنى لها/غير المعروفة تضبط التأخر على
  الارتفاع المطلوب (أو الدلتا)، والتقدمات الناجحة تعيد تعيينه إلى الصفر بحيث تظل الحالة المستقرة ثابتة.
  يجب على المشغلين الإنذار عند حدوث تأخيرات غير صفرية، وفحص بكرة/مجلة DA بحثًا عن المسار المخالف،
  وتحقق من كتالوج المسار بحثًا عن إعادة تقسيم غير مقصودة قبل إعادة تشغيل الكتلة لمسح
  فجوة.
- **الممرات الحسابية السرية** — الممرات المميزة بـ
  يتم التعامل مع `metadata.confidential_compute=true` و`confidential_key_version` على أنهما
  SMPC/مسارات DA المشفرة: يفرض Sumeragi حمولة غير صفرية/ملخصات البيان وتذاكر التخزين،
  يرفض ملفات تعريف التخزين المتماثلة الكاملة، ويقوم بفهرسة تذكرة SoraFS + إصدار السياسة بدون
  تعريض بايتات الحمولة. يتم إيصال الهيدرات من كورا أثناء إعادة التشغيل حتى يستعيد المدققون نفس الشيء
  البيانات التعريفية السرية بعد إعادة التشغيل.

## ملاحظات التنفيذ- تعمل نقطة النهاية `/v2/da/ingest` الخاصة بـ Torii الآن على تطبيع ضغط الحمولة النافعة، وفرض ذاكرة التخزين المؤقت لإعادة التشغيل،
  قطع وحدات البايت الأساسية بشكل حتمي، وإعادة بناء `DaManifestV1`، وإسقاط الحمولة النافعة المشفرة
  في `config.da_ingest.manifest_store_dir` لتنسيق SoraFS قبل إصدار الإيصال؛ ال
  يقوم المعالج أيضًا بإرفاق رأس `Sora-PDP-Commitment` حتى يتمكن العملاء من التقاط الالتزام المشفر
  على الفور.[صناديق/iroha_torii/src/da/ingest.rs:220]
- بعد استمرار `DaCommitmentRecord` الأساسي، يصدر Torii الآن إشارة
  ملف `da-commitment-schedule-<lane>-<epoch>-<sequence>-<ticket>.norito` بجانب التخزين المؤقت للبيان.
  يقوم كل إدخال بتجميع السجل باستخدام وحدات البايت Norito `PdpCommitment` الأولية، لذا فإن منشئي حزم DA-3 و
  تستوعب برامج جدولة DA-5 مدخلات متطابقة دون إعادة قراءة البيانات أو مخازن القطع.
- يكشف مساعدو SDK عن بايتات رأس PDP دون إجبار كل عميل على إعادة تنفيذ تحليل Norito:
  `iroha::da::{decode_pdp_commitment_header, receipt_pdp_commitment}` غطاء الصدأ، بايثون `ToriiClient`
  يقوم الآن بتصدير `decode_pdp_commitment_header`، كما يقوم `IrohaSwift` بشحن المساعدين المطابقين المتنقلين للغاية
  يمكن للعملاء تخزين جدول أخذ العينات المشفر على الفور. 【crates/iroha/src/da.rs:1】【python/iroha_torii_client/client.py:1】【IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:1】
- يعرض Torii أيضًا `GET /v2/da/manifests/{storage_ticket}` حتى تتمكن مجموعات SDK والمشغلون من جلب البيانات
  وخطط القطع دون لمس دليل التخزين المؤقت للعقدة. تقوم الاستجابة بإرجاع البايتات Norito
  (base64)، تم تقديم بيان JSON، `chunk_plan` JSON blob جاهز لـ `sorafs fetch`، بالإضافة إلى العناصر ذات الصلة
  الملخصات السداسية (`storage_ticket`، `client_blob_id`، `blob_hash`، `chunk_root`) بحيث يمكن للأدوات النهائية
  تغذية المنسق دون إعادة حساب الملخصات، وإصدار نفس رأس `Sora-PDP-Commitment` إلى
  مرآة استيعاب الاستجابات. يؤدي تمرير `block_hash=<hex>` كمعلمة استعلام إلى إرجاع قيمة حتمية
  `sampling_plan` متجذر في `block_hash || client_blob_id` (مشترك عبر أدوات التحقق من الصحة) يحتوي على
  `assignment_hash`، و`sample_window` المطلوب، وعينات `(index, role, group)` من الصفوف الممتدة
  تخطيط الشريط ثنائي الأبعاد بالكامل حتى تتمكن أجهزة أخذ عينات PoR وأجهزة التحقق من الصحة من إعادة تشغيل نفس المؤشرات. أخذ العينات
  يدمج `client_blob_id`، و`chunk_root`، و`ipa_commitment` في تجزئة المهمة؛ `iroha التطبيق دا الحصول عليه
  --block-hash ` now writes `sampling_plan_.json` بجوار خطة البيان + القطعة مع
  تم الحفاظ على التجزئة، ويكشف عملاء JS/Swift Torii عن نفس `assignment_hash_hex` لذلك يقوم المدققون
  ويشترك الأمثال في مجموعة مسبار حتمية واحدة. عندما يقوم Torii بإرجاع خطة أخذ العينات، `iroha app da
  إثبات التوفر` now reuses that deterministic probe set (seed derived from `sample_seed`) بدلاً من ذلك
  من أخذ العينات المخصصة بحيث يصطف شهود إثبات صحة المعلومات مع تعيينات المدقق حتى إذا أغفل المشغل
  `--block-hash` override.[crates/iroha_torii_shared/src/da/sampling.rs:1]】[crates/iroha_cli/src/commands/da.rs:523] 【javascript/iroha_js/src/toriiClient.js:15903】[IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:170]

### تدفق تدفق الحمولة الكبيرةالعملاء الذين يحتاجون إلى استيعاب أصول أكبر من حد الطلب الفردي الذي تم تكوينه، يبدأون أ
جلسة البث عن طريق الاتصال بـ `POST /v2/da/ingest/chunk/start`. يستجيب Torii بـ
`ChunkSessionId` (BLAKE3-مشتق من بيانات تعريف blob المطلوبة) وحجم القطعة التي تم التفاوض عليها.
يحمل كل طلب `DaIngestChunk` لاحقاً ما يلي:

- `client_blob_id` — مطابق للرقم `DaIngestRequest` النهائي.
- `chunk_session_id` — يربط الشرائح بجلسة التشغيل.
- `chunk_index` و`offset` - فرض الترتيب الحتمي.
- `payload` — حتى حجم القطعة التي تم التفاوض عليها.
- `payload_hash` - تجزئة BLAKE3 للشريحة حتى يتمكن Torii من التحقق من صحتها دون تخزين النقطة بأكملها مؤقتًا.
- `is_last` - يشير إلى الشريحة الطرفية.

Torii يستمر في التحقق من صحة الشرائح ضمن `config.da_ingest.manifest_store_dir/chunks/<session>/` و
يسجل التقدم داخل ذاكرة التخزين المؤقت لإعادة التشغيل لتكريم العجز. عندما تصل الشريحة الأخيرة، Torii
يعيد تجميع الحمولة على القرص (التدفق عبر دليل القطعة لتجنب ارتفاع الذاكرة)،
يحسب البيان/الإيصال الأساسي تمامًا كما هو الحال مع التحميلات الفردية، ويستجيب أخيرًا
`POST /v2/da/ingest` عن طريق استهلاك القطعة الأثرية المرحلية. يمكن إحباط الجلسات الفاشلة بشكل صريح أو
يتم تجميع البيانات المهملة بعد `config.da_ingest.replay_cache_ttl`. يحافظ هذا التصميم على تنسيق الشبكة
متوافق مع Norito، ويتجنب البروتوكولات القابلة للاستئناف الخاصة بالعميل، ويعيد استخدام مسار البيان الحالي
دون تغيير.

**حالة التنفيذ.** أنواع Norito الأساسية موجودة الآن
`crates/iroha_data_model/src/da/`:

- يحدد `ingest.rs` `DaIngestRequest`/`DaIngestReceipt`، مع
  حاوية `ExtraMetadata` المستخدمة بواسطة Torii.[crates/iroha_data_model/src/da/ingest.rs:1]
- يستضيف `manifest.rs` `DaManifestV1` و`ChunkCommitment`، والذي ينبعث منه Torii بعد ذلك
  اكتمل التقطيع. 【صناديق/iroha_data_model/src/da/manifest.rs:1】
- يوفر `types.rs` أسماء مستعارة مشتركة (`BlobDigest`، `RetentionPolicy`،
  `ErasureProfile`، وما إلى ذلك) وترميز قيم السياسة الافتراضية الموثقة أدناه. 【crates/iroha_data_model/src/da/types.rs:240】
- ملفات التخزين المؤقت للبيان تصل إلى `config.da_ingest.manifest_store_dir`، جاهزة لتنسيق SoraFS
  مراقب لسحب إلى قبول التخزين.
- يفرض Sumeragi توفر البيان عند إغلاق حزم DA أو التحقق من صحتها:
  تفشل الكتل في التحقق من الصحة إذا كان التخزين المؤقت يفتقد البيان أو كان التجزئة مختلفًا
  من الالتزام.[صناديق/iroha_core/src/sumeragi/main_loop.rs:5335][صناديق/iroha_core/src/sumeragi/main_loop.rs:14506]

يتم تعقب تغطية رحلة الذهاب والإياب للطلب والبيان وحمولات الاستلام
`crates/iroha_data_model/tests/da_ingest_roundtrip.rs`، مع ضمان برنامج الترميز Norito
يظل مستقرًا عبر التحديثات.

**الافتراضيات الخاصة بالاحتفاظ.** صدقت الإدارة على سياسة الاحتفاظ الأولية أثناء
SF-6؛ الإعدادات الافتراضية التي يفرضها `RetentionPolicy::default()` هي:- الطبقة الساخنة: 7 أيام (`604_800` ثانية)
- الطبقة الباردة: 90 يومًا (`7_776_000` ثانية)
- النسخ المتماثلة المطلوبة: `3`
- فئة التخزين: `StorageClass::Hot`
- علامة الإدارة: `"da.default"`

يجب على مشغلي المصب تجاوز هذه القيم بشكل صريح عند اعتماد المسار
متطلبات أكثر صرامة.

## قطع أثرية مقاومة للصدأ

لم تعد حزم SDK التي تتضمن عميل Rust بحاجة إلى إرسالها إلى واجهة سطر الأوامر (CLI).
إنتاج حزمة PoR JSON الأساسية. يعرض `Client` اثنين من المساعدين:

- يقوم `build_da_proof_artifact` بإرجاع البنية الدقيقة التي تم إنشاؤها بواسطة
  `iroha app da prove --json-out`، بما في ذلك التعليقات التوضيحية للبيان/الحمولة النافعة المتوفرة
  عبر [`DaProofArtifactMetadata`].[crates/iroha/src/client.rs:3638]
- `write_da_proof_artifact` يغلف المنشئ ويحتفظ بالمنتج على القرص
  (JSON جميل + سطر جديد زائد بشكل افتراضي) حتى تتمكن الأتمتة من إرفاق الملف
  للإصدارات أو حزم أدلة الحوكمة.[crates/iroha/src/client.rs:3653]

### مثال

```rust
use iroha::{
    da::{DaProofArtifactMetadata, DaProofConfig},
    Client,
};

let client = Client::new(config);
let manifest = client.get_da_manifest_bundle(storage_ticket)?;
let payload = std::fs::read("artifacts/da/payload.car")?;
let metadata = DaProofArtifactMetadata::new(
    "artifacts/da/manifest.norito",
    "artifacts/da/payload.car",
);

// Build the JSON artefact in-memory.
let artifact = client.build_da_proof_artifact(
    &manifest,
    &payload,
    &DaProofConfig::default(),
    &metadata,
)?;

// Persist it next to other DA artefacts.
client.write_da_proof_artifact(
    &manifest,
    &payload,
    &DaProofConfig::default(),
    &metadata,
    "artifacts/da/proof_summary.json",
    true,
)?;
```

تطابق حمولة JSON التي تترك المساعد واجهة سطر الأوامر (CLI) حتى أسماء الحقول
(`manifest_path`، `payload_path`، `proofs[*].chunk_digest`، وما إلى ذلك)، موجود جدًا
يمكن للأتمتة أن تقوم بفرق/باركيه/تحميل الملف بدون فروع خاصة بالتنسيق.

## معيار التحقق من الإثبات

استخدم أداة اختبار إثبات DA للتحقق من صحة ميزانيات المدقق على الحمولات النافعة التمثيلية من قبل
تشديد الحدود القصوى على مستوى الكتلة:

- يقوم `cargo xtask da-proof-bench` بإعادة بناء مخزن القطع من زوج البيان/الحمولة النافعة، وعينات PoR
  الإجازات، وأوقات التحقق مقابل الميزانية التي تم تكوينها. تتم تعبئة البيانات التعريفية لـ Taikai تلقائيًا، ويتم إضافة ملف
  يعود الحزام إلى البيان الاصطناعي إذا كان زوج التثبيت غير متناسق. عندما `--payload-bytes`
  تم تعيينه بدون `--payload` صريحًا، وستتم كتابة النقطة التي تم إنشاؤها إليها
  `artifacts/da/proof_bench/payload.bin` لذلك تظل التركيبات دون تغيير.
- التقارير الافتراضية هي `artifacts/da/proof_bench/benchmark.{json,md}` وتتضمن البراهين/التشغيل والإجمالي و
  توقيتات إثبات، ومعدل نجاح الميزانية، والميزانية الموصى بها (110% من التكرار الأبطأ)
  اصطف مع `zk.halo2.verifier_budget_ms`.[artifacts/da/proof_bench/benchmark.md:1]
- أحدث تشغيل (حمولة اصطناعية 1 ميجابايت، 64 كيلو بايت، 32 إثباتًا/تشغيل، 10 تكرارات، ميزانية 250 مللي ثانية)
  أوصى بميزانية تحقق تبلغ 3 مللي ثانية مع 100% من التكرارات داخل الحد الأقصى.[artifacts/da/proof_bench/benchmark.md:1]
- مثال (يولد حمولة حتمية ويكتب كلا التقريرين):

```shell
cargo xtask da-proof-bench \
  --payload-bytes 1048576 \
  --sample-count 32 \
  --iterations 10 \
  --budget-ms 250 \
  --json-out artifacts/da/proof_bench/benchmark.json \
  --markdown-out artifacts/da/proof_bench/benchmark.md
```

يفرض تجميع الكتلة نفس الميزانيات: `sumeragi.da_max_commitments_per_block` و
`sumeragi.da_max_proof_openings_per_block` بوابة حزمة DA قبل أن يتم تضمينها في كتلة، و
يجب أن يحمل كل التزام رقمًا غير صفري `proof_digest`. يعامل الحارس طول الحزمة على أنه
عدد فتح الإثبات حتى يتم تمرير ملخصات الإثبات الصريحة من خلال الإجماع، مع الحفاظ على
≥128 هدف فتح قابل للتنفيذ عند حدود الكتلة.

## التعامل مع فشل PoR وخفضهيعرض عمال التخزين الآن خطوط فشل PoR وتوصيات القطع المائلة بجانب كل منها
الحكم. تؤدي حالات الفشل المتتالية التي تتجاوز حد المخالفة المكوّن إلى إصدار توصية بذلك
يتضمن زوج الموفر/البيان، وطول الخط الذي أدى إلى الشرطة المائلة، والمقترح
العقوبة المحسوبة من سند الموفر و`penalty_bond_bps`؛ نوافذ التهدئة (ثواني) تبقى
خطوط مائلة مكررة من إطلاق النار في نفس الحادثة. 【crates/sorafs_node/src/lib.rs:486】【crates/sorafs_node/src/config.rs:89】 【crates/sorafs_node/src/bin/sorafs-node.rs:343】

- تكوين العتبات/فترة التهدئة عبر منشئ عامل التخزين (تعكس الإعدادات الافتراضية الحوكمة
  سياسة العقوبات).
- يتم تسجيل توصيات القطع في ملخص الحكم بتنسيق JSON حتى يتمكن الحكم/المدققون من إرفاقها
  لهم حزم الأدلة.
- يتم الآن ربط تخطيط الشريط + الأدوار لكل قطعة من خلال نقطة نهاية دبوس تخزين Torii
  (حقول `stripe_layout` + `chunk_roles`) واستمرت في عامل التخزين لذلك
  يمكن للمدققين/أدوات الإصلاح التخطيط لإصلاحات الصفوف/الأعمدة دون إعادة اشتقاق التخطيط من المنبع

### حزام التنسيب + الإصلاح

`cargo run -p sorafs_car --bin da_reconstruct -- --manifest <path> --chunks-dir <dir>` الآن
يحسب تجزئة الموضع عبر `(index, role, stripe/column, offsets)` وينفذ الصف الأول بعد ذلك
إصلاح العمود RS(16) قبل إعادة بناء الحمولة:

- القيمة الافتراضية للموضع هي `total_stripes`/`shards_per_stripe` عندما تكون موجودة وتعود إلى المجموعة
- تتم إعادة بناء القطع المفقودة/التالفة بتكافؤ الصف أولاً؛ يتم إصلاح الفجوات المتبقية مع
  تكافؤ الشريط (العمود). تتم إعادة كتابة القطع التي تم إصلاحها إلى دليل القطعة وملف JSON
  يلتقط الملخص تجزئة الموضع بالإضافة إلى عدادات إصلاح الصفوف/الأعمدة.
- إذا لم يتمكن تكافؤ الصف + العمود من تلبية المجموعة المفقودة، فإن الحزام يفشل بسرعة مع عدم إمكانية استرداده
  مؤشرات حتى يتمكن المدققون من وضع علامة على البيانات التي لا يمكن إصلاحها.