---
lang: ar
direction: rtl
source: docs/portal/docs/da/ingest-plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::ملاحظة فوينتي كانونيكا
ريفليجا `docs/source/da/ingest_plan.md`. Mantenga ambas الإصدارات en
:::

# خطة استيعاب توفر بيانات Sora Nexus

_Redactado: 2026-02-20 - المسؤول: فريق عمل البروتوكول الأساسي / فريق التخزين / DA WG_

يمتد مسار العمل DA-2 إلى Torii مع واجهة برمجة تطبيقات استيعاب النقط التي تصدرها
البيانات الوصفية Norito وستظل نسخة SoraFS. تم التقاط هذا المستند
هذا هو السبب وسطح واجهة برمجة التطبيقات وتدفق التحقق من الصحة
التنفيذ المسبق بدون حظر للمحاكاة النموذجية (التسلسلات
دا-1). جميع تنسيقات الحمولة DEBEN تستخدم برامج الترميز Norito؛ لا يسمح بذلك
النسخ الاحتياطية serde/JSON.

##الأهداف

- قبول النقط الكبيرة (مقاطع Taikai، الممرات الجانبية، المصنوعات اليدوية
  gobernanza) لشكل التحديد عبر Torii.
- يُظهر منتج Norito Canonicos الذي يصف النقطة، ومعلمات
  برنامج الترميز وملف المحو وسياسة الاحتفاظ.
- الاحتفاظ بالبيانات الوصفية للقطع في التخزين السريع لـ SoraFS وتضمين المهام
  النسخ المتماثل.
- نشر نوايا الدبوس + العلامات السياسية في السجل SoraFS والمراقبين
  دي غوبرنانزا.
- توضيح استلامات القبول حتى يتمكن العملاء من التعافي بشكل محدد
  دي النشر.

## واجهة برمجة تطبيقات Superficie (Torii)

```
POST /v2/da/ingest
Content-Type: application/norito+v1
```

الحمولة هي `DaIngestRequest` مرمزة في Norito. الردود المستخدمة
`application/norito+v1` وتطوير `DaIngestReceipt`.

| الرد | هامة |
| --- | --- |
| 202 مقبول | Blob en cola para تقطيع/نسخ متماثل؛ إذا كنت ستحقق الاستلام. |
| 400 طلب سيء | Violacion de esquema/tamano (ver validaciones). |
| 401 غير مصرح به | واجهة برمجة تطبيقات الرمز المميز/غير صالحة. |
| 409 الصراع | نسخة مكررة من `client_blob_id` مع بيانات وصفية ليست من قبيل الصدفة. |
| 413 الحمولة كبيرة جدًا | تجاوز الحد الأقصى لتكوين طول النقطة. |
| 429 طلبات كثيرة جدًا | هذا هو الحد الأقصى للمعدل. |
| 500 خطأ داخلي | Fallo inesperado (سجل + تنبيه). |

## Esquema Norito propuesto

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

> مذكرة التنفيذ: التمثيلات القانونية في الصدأ لهذه
> الحمولات الآن قيد التشغيل `iroha_data_model::da::types`، مع أغلفة
> الطلب/الاستلام في `iroha_data_model::da::ingest` وبنية البيان
> ar `iroha_data_model::da::manifest`.

يُعلن المجال `compression` عن كيفية إعداد المتصلين للحمولة. Torii
قبول `identity`، `gzip`، `deflate`، و`zstd`، إلغاء التنازل عن وحدات البايت السابقة
تظهر عمليات التجزئة والتقطيع والتحقق اختيارية.

### قائمة التحقق من الصحة

1. تحقق من أن رأس الطلب Norito يتطابق مع `DaIngestRequest`.
2. قم بالتغيير إلى `total_size` بعد طول الحمولة الكنسي (غير متضمن)
   o تجاوز الحد الأقصى للتكوين.
3. تطابق `chunk_size` (قوة العمل، <= 2 MiB).
4. آمن `data_shards + parity_shards` <= الحد الأقصى لتكافؤ y العالمي >= 2.
5. يجب على `retention_policy.required_replica_count` إعادة تشغيل قاعدة الخط
   com.gobernanza.
6. التحقق من الشركة ضد التجزئة الكنسي (باستثناء توقيع المجال).
7. قم باستعادة نسخة `client_blob_id` المكررة من تجزئة الحمولة والحمولة
   البيانات الوصفية شون identicos.
8. عند إثبات `norito_manifest`، تحقق من الاختبار + تزامن التجزئة مع
   إعادة حساب واضحة من خلال التقطيع؛ دي لو على النقيض من العقدة العامة
   واضح و لو ألماسينا.
9. استعادة سياسة النسخ المتماثل التي تم تكوينها: Torii إعادة كتابة
   `RetentionPolicy` مرسل مع `torii.da_ingest.replication_policy` (الإصدار
   `replication-policy.md`) والرسالة تظهر البيانات الوصفية التي تم إنشاؤها مسبقًا
   لا يتزامن الاحتفاظ مع الأداء المدفوع.

### تدفق القطع والنسخ المتماثل

1. قم بتمرير الحمولة في `chunk_size`، وحساب BLAKE3 بواسطة قطعة + رأس ميركل.
2. قم ببناء Norito `DaManifestV1` (الهيكل الجديد) لالتقاط التنازلات
   قطعة (دور/معرف_المجموعة)، تخطيط المحو (حسابات سلسلة الملفات y
   أعمدة mas `ipa_commitment`)، سياسة الاحتفاظ والبيانات الوصفية.
3. قم بتضمين بايتات البيان الكنسي المتبقي
   `config.da_ingest.manifest_store_dir` (Torii أكتب أرشيفات
   `manifest.encoded` حسب المسار/العصر/التسلسل/التذكرة/بصمة الإصبع) لذلك
   طلب SoraFS من الذاكرة الداخلية وبطاقة التخزين مع البيانات
   استمرار.
4. نشر نوايا الدبوس عبر `sorafs_car::PinIntent` مع علامة الإدارة
   سياسة.
5. أرسل الحدث Norito `DaIngestPublished` لإخطار المراقبين (العملاء)
   ليجيروس، جوبيرنانزا، أناليتيكا).
6. تحويل `DaIngestReceipt` إلى المتصل (تم التثبيت بواسطة مفتاح الخدمة DA de
   Torii) وأصدر الرأس `Sora-PDP-Commitment` لتتمكن مجموعات SDK من التقاطها
   الالتزام المقنن الوسيط. يتضمن الاستلام الآن `rent_quote`
   (un Norito `DaRentQuote`) و`stripe_layout`، يسمح بالتحويلات المالية
   إظهار قاعدة الإيجار والحجز والتوقعات الإضافية لـ PDP/PoTR وما إلى ذلك
   تخطيط المسح ثنائي الأبعاد جنبًا إلى جنب مع تذكرة التخزين قبل المساس بالخلفية.

## Actualizaciones de almacenamiento/registry

- الموسع `sorafs_manifest` مع `DaManifestV1`، بارسيو مؤهل
  حتمية.
- قم بإضافة دفق التسجيل الجديد `da.pin_intent` مع إصدار الحمولة الذي تريده
  مرجع تجزئة البيان + معرف التذكرة.
- تحديث خطوط أنابيب المراقبة لمتابعة زمن الوصول للاستهلاك،
  إنتاجية التقطيع وتراكم النسخ وبيانات السقوط.

## استراتيجية الاختبار

- اختبارات موحدة للتحقق من صحة esquema، والتحقق من الشركة والكشف عن
  مكررة.
- اختبارات التحقق الذهبي من الترميز Norito de `DaIngestRequest`، البيان y
  إيصال.
- تسخير التكامل Levantando SoraFS + محاكاة التسجيل، صالحة
  فلوجو دي قطعة + دبوس.
- اختبارات الملكية من خلال مسح الملفات الشخصية ودمجها
  ألياتورياس الاحتفاظ.
- تشويش الحمولات Norito لحماية البيانات التعريفية غير الصحيحة.

## أدوات CLI وSDK (DA-8)- `iroha app da submit` (نقطة الدخول الجديدة لـ CLI) الآن يحيط بالمنشئ/الناشر
  مشاركة الاستخدام حتى يتمكن المشغلون من إدراج النقط العشوائية
  حزمة Fuera del Flujo Taikai. الكوماندو فيف أون
  `crates/iroha_cli/src/commands/da.rs:1` وتستهلك حمولة كاملة
  المحو/الاحتفاظ والأرشفة الاختيارية للبيانات الوصفية/البيان قبل التثبيت
  `DaIngestRequest` Canon مع مفتاح تكوين CLI. القذف
  الخروج المستمر `da_request.{norito,json}` و `da_receipt.{norito,json}` آخر
  `artifacts/da/submission_<timestamp>/` (التجاوز عبر `--artifact-dir`) لذلك
  يتم تحرير العناصر المصطنعة من وحدات البايت Norito تمامًا أثناء الاستخدام
  إنغستا.
- El comando usa por deffecto `client_blob_id = blake3(payload)` pero acepta
  التجاوزات عبر `--client-blob-id`، إعادة تعيين بيانات تعريف JSON
  (`--metadata-json`) ويظهر ما قبل الإنشاء (`--manifest`)، ويدعم
  `--no-submit` للتحضير دون الاتصال بالإنترنت و`--endpoint` للمضيفين Torii
  تخصيص. سيتم طباعة الإيصال JSON بشكل قياسي بعد كتابته
  القرص، والانتهاء من متطلبات الأدوات "submit_blob" لـ DA-8 وإلغاء القفل
  وظيفة موازنة SDK.
- `iroha app da get` جمع اسم مستعار تم إنشاؤه في DA للأوركيستادور متعدد المصادر
  ما هي إمكانات `iroha app sorafs fetch`. يمكن للمشغلين أن يصححوا أ
  المصنوعات اليدوية + مخطط القطع (`--manifest`، `--plan`، `--manifest-id`)
  **o** قم بشراء تذكرة تخزين Torii عبر `--storage-ticket`. عندما تكون في الولايات المتحدة
  مسار التذكرة، CLI باجا البيان من `/v2/da/manifests/<ticket>`،
  استمرار الحزمة الباجو `artifacts/da/fetch_<timestamp>/` (تجاوز con
  `--manifest-cache-dir`)، مشتق من تجزئة النقطة لـ `--manifest-id`، ومثلها
  قم بتشغيل المشغل باستخدام القائمة `--gateway-provider`. كل شيء
  المقابض المتقدمة لجلب SoraFS تظل سليمة (البيان
  المغلفات، ملصقات العملاء، حراسة المخابئ، تجاوزات النقل المجهول،
  تصدير لوحة النتائج والمسارات `--output`)، ويمكن بيان نقطة النهاية
  يتم الاشتراك عبر `--manifest-endpoint` للمضيفين Torii المخصصين أيضًا
  ستظل عمليات التحقق من التوفر من البداية إلى النهاية متوقفة تمامًا عن مساحة الاسم
  `da` ليس له منطق مكرر في orquestador.
- `iroha app da get-blob` يُظهر baja canonicos directo desde Torii عبر
  `GET /v2/da/manifests/{storage_ticket}`. الكوماندو يكتب
  `manifest_{ticket}.norito`، `manifest_{ticket}.json` ص
  `chunk_plan_{ticket}.json` باجو `artifacts/da/fetch_<timestamp>/` (أو الأمم المتحدة
  `--output-dir` يوفر للمستخدم) أثناء تشغيل الأمر الدقيق
  `iroha app da get` (بما في ذلك `--manifest-id`) مطلوب للجلب
  orquestador. هذا يحافظ على المشغلين من خلال بكرة المجلد
  البيانات وتضمن أن الجلب سيستمر في استخدام المصنوعات اليدوية الثابتة
  صادرات بواسطة Torii. يعكس العميل Torii من JavaScript نفس التدفق عبر
  `ToriiClient.getDaManifest(storageTicketHex)`، نقل البايتات Norito
  تم فك التشفير وبيان JSON وخطة القطعة لإخفاء المتصلين بـ SDK
  جلسات orquestador بدون استخدام CLI. يعرض SDK الخاص بـ Swift الآن
  سطوح ميسماس (`ToriiClient.getDaManifestBundle(...)` mas
  `fetchDaPayloadViaGateway(...)`)، حزم البث المجمعة الأصلية
  orquestador SoraFS حتى يتمكن عملاء iOS من تنزيل البيانات وإخراجها
  جلب مصادر متعددة والتقاطها دون الحاجة إلى استدعاء CLI.
  [IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240][IrohaSwift/Sources/IrohaSwift/SorafsOrchestratorClient.swift:12]
- `iroha app da rent-quote` حساب الإيجارات وتحديد الحوافز
  لحجم التخزين ونافذة الاحتفاظ. المساعد
  استهلاك `DaRentPolicyV1` النشط (JSON أو البايتات Norito) أو التكامل الافتراضي،
  التحقق من السياسة وطباعة استئناف JSON (`gib`، `months`، البيانات الوصفية
  السياسة والمجالات `DaRentQuote`) لكي يستشهد المدققون بشحنات XOR الدقيقة
  في أعمال الإدارة بدون نصوص مخصصة. الأمر أيضًا يصدر استئنافًا
  في سطر `rent_quote ...` قبل حمولة JSON للحفاظ على المقروءات
  سجلات وحدة التحكم أثناء التدريبات على الأحداث. إمباريجي
  `--quote-out artifacts/da/rent_quotes/<stamp>.json` يخدع
  `--policy-label "governance ticket #..."` لاستمرار القطع الأثرية الجميلة
  ذكر الصوت أو حزمة التكوين الدقيق؛ يقوم CLI باستعادة الملصق المخصص
  y rechaza strings vacios para que los valores `policy_source` يتم الاحتفاظ بها
  متاحة في لوحات المعلومات. الاصدار
  `crates/iroha_cli/src/commands/da.rs` للأمر الفرعي y
  `docs/source/da/rent_policy.md` للإسلام السياسي.
  [صناديق/iroha_cli/src/commands/da.rs:1][docs/source/da/rent_policy.md:1]
- `iroha app da prove-availability` encadena todo lo anterior: toma un Storage
  تذكرة، تنزيل الحزمة Canonico de Manifest، تشغيل Orquestador
  متعدد المصادر (`iroha app sorafs fetch`) مقابل القائمة `--gateway-provider`
  Suministrada، استمر في تنزيل الحمولة + لوحة النتائج
  `artifacts/da/prove_availability_<timestamp>/`، واستدعاء الوسيط المساعد
  PoR موجود (`iroha app da prove`) يستخدم البايتات المتعددة. المشغلون
  يمكن ضبط مقابض orquestador (`--max-peers`، `--scoreboard-out`،
  يتجاوز بيان نقطة النهاية) وعينة الإثبات (`--sample-count`،
  `--leaf-index`, `--sample-seed`) أثناء تنفيذ أمر منفرد
  المصنوعات اليدوية المفحوصة من خلال المستمعين DA-5/DA-9: نسخة من الحمولة النافعة والأدلة
  لوحة النتائج والسيرة الذاتية للاختبار JSON.