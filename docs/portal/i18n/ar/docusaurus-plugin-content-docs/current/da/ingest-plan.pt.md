---
lang: ar
direction: rtl
source: docs/portal/docs/da/ingest-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::ملاحظة فونتي كانونيكا
إسبيلها `docs/source/da/ingest_plan.md`. Mantenha كما أدعية الآيات م
:::

# خطة استيعاب توفر البيانات من Sora Nexus

_Redigido: 2026/02/20 - الردود: مجموعة عمل البروتوكول الأساسي / فريق التخزين / DA WG_

مسار العمل DA-2 موجود Torii مع واجهة برمجة تطبيقات استيعاب النقط التي تصدرها
التعريفات Norito والنسخ المتماثل SoraFS. تم التقاط هذا المستند أو الرسم
اقتراح، سطح واجهة برمجة التطبيقات (API) وتدفق التحقق من أجل التنفيذ
قم بحظر المحاكاة المعلقة مسبقًا (تابع DA-1). جميع نظام التشغيل
تنسيقات الحمولة DEVEM تستخدم برامج الترميز Norito؛ ناو ساو المسموح به احتياطيات
سيردي/JSON.

##الأهداف

- استخدام النقط الكبيرة (مقاطع Taikai، وsidecars de lane، وartefatos de
  Goveranca) دي شكل حتمي عبر Torii.
- يُظهر المنتج Norito Canonicos التي يتم الكشف عنها أو معلمات برنامج الترميز،
  ملف المحو والاحتفاظ السياسي.
- الاستمرار في البيانات الوصفية للأجزاء التي لا تحتوي على تخزين ساخن لـ SoraFS وتسجيل المهام
  com.replicao.
- نشر نوايا الدبوس + العلامات السياسية في سجل SoraFS والمراقبين
  دي الحاكم.
- تصدير إيصالات القبول حتى يتمكن العملاء من استعادة الحتمية المؤكدة
  de publicacao.

## واجهة برمجة تطبيقات Superficie (Torii)

```
POST /v2/da/ingest
Content-Type: application/norito+v1
```

الحمولة الصافية هي `DaIngestRequest` المرمزة في Norito. كما الرد usam
`application/norito+v1` ثم أعيد تسميته `DaIngestReceipt`.

| الرد | هامة |
| --- | --- |
| 202 مقبول | Blob enfileirado للتقطيع/النسخ المتماثل؛ إرجاع الاستلام. |
| 400 طلب سيء | Violacao de esquema/tamanho (veja validacoes). |
| 401 غير مصرح به | واجهة برمجة تطبيقات الرمز المميز/غير صالحة. |
| 409 الصراع | Duplicado `client_blob_id` مع البيانات الوصفية المتباينة. |
| 413 الحمولة كبيرة جدًا | تجاوز الحد الأقصى لتكوين حجم النقطة الكبيرة. |
| 429 طلبات كثيرة جدًا | الحد الأقصى للسعر. |
| 500 خطأ داخلي | Falha inesperada (سجل + تنبيه). |

## Esquema Norito proposto

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

> ملاحظة التنفيذ: كممثلين قانونيين في Rust لهذه الحمولات
> Agora viem em `iroha_data_model::da::types`، مع أغلفة الطلب/الاستلام
> em `iroha_data_model::da::ingest` وإجراءات البيان
> `iroha_data_model::da::manifest`.

يُعلم المجال `compression` كيفية إعداد المتصلين للحمولة. Torii
`identity`، `gzip`، `deflate`، و`zstd`، إلغاء تجزئة البايتات السابقة للتجزئة،
التقطيع والتحقق من البيانات الاختيارية.

### قائمة التحقق من صحة

1. تحقق مما إذا كان الرأس Norito من المتطلبات يتوافق مع `DaIngestRequest`.
2. يختلف Falhar se `total_size` عن حجم الحمولة الكنسي (ديسكبريميدو)
   أو تتجاوز الحد الأقصى للتكوين.
3. قوة الضخ `chunk_size` (قدرة العدد <= 2 MiB).
4. Garantir `data_shards + parity_shards` <= الحد الأقصى للتكافؤ الإلكتروني العالمي >= 2.
5. `retention_policy.required_replica_count` يجب أن يستمر في خط الأساس
   com.goveranca.
6. التحقق من القتل ضد التجزئة الكنسي (باستثناء التوقيع الميداني).
7. قم بتسجيل `client_blob_id` مكررًا، باستثناء تجزئة الحمولة والبيانات التعريفية
   متطابقة الشكل.
8. Quando `norito_manifest` for fornecido، مخطط التحقق + التجزئة المتزامنة مع
   o إعادة الحساب الواضح بعد التقطيع؛ القضية المعاكسة أو العقدة هي أو البيان
   ه يا أرمازينا.
9. قم بإنشاء نسخة طبق الأصل من السياسة: Torii إعادة إنشاء
   `RetentionPolicy` أرسل مع `torii.da_ingest.replication_policy` (فيجا
   `replication-policy.md`) ويعرض العرض البيانات الوصفية التي تم إنشاؤها مسبقًا
   لا يتزامن الاحتفاظ بالملف مع ملف التعريف.

### تدفق القطع والنسخ

1. حمولة الحمولة em `chunk_size`، حساب BLAKE3 بواسطة قطعة + رايز ميركل.
2. قم ببناء Norito `DaManifestV1` (الهيكل الجديد) من خلال التقاط التنازلات
   القطعة (الدور/معرف المجموعة)، تخطيط المحو (عدوى خطوط الخطوط e
   عمود آخر `ipa_commitment`)، سياسة الاحتفاظ بالبيانات الوصفية.
3. يقوم Enfileirar os bytes بإظهار canonico sob
   `config.da_ingest.manifest_store_dir` (Torii قم بفتح الملفات
   `manifest.encoded` حسب المسار/العصر/التسلسل/التذكرة/بصمة الإصبع) لذلك
   orquestracao SoraFS os ingira e vincule o تذكرة تخزين مع دادوس
   استمرار.
4. نشر نوايا الدبوس عبر `sorafs_car::PinIntent` com tag de goveranca e
   سياسة.
5. أرسل الحدث Norito `DaIngestPublished` لإخطار المراقبين
   (عملاء ليفز، جوفرنانكا، أناليتيكا).
6. إعادة الاتصال `DaIngestReceipt` للمتصل (تم تشغيله بواسطة خدمة DA de
   Torii) وإصدار الرأس `Sora-PDP-Commitment` لالتقاط SDKs
   التزام مقنن فوريا. يا الإيصال يشمل `rent_quote`
   (أم Norito `DaRentQuote`) و`stripe_layout`، مما يسمح لنا بالتذكير
   عرض قاعدة بيانات وحجز وتوقعات إضافية لـ PDP/PoTR وتخطيط
   قم بمسح 2D من خلال تخزين تذكرة التخزين قبل المساس بقاعدة البيانات.

## تحديث التخزين/التسجيل

- Estender `sorafs_manifest` com `DaManifestV1`، التحليل المؤهل
  حتمية.
- إضافة دفق التسجيل الجديد `da.pin_intent` com payload versionado
  مرجع تجزئة البيان + معرف التذكرة.
- تحديث خطوط أنابيب المراقبة من أجل زمن الوصول المنخفض للاستهلاك،
  إنتاجية التقطيع وتراكم النسخ ورسائل الخطأ.

## استراتيجية الخصيتين

- اختبارات موحدة للتحقق من صحة المخطط، وفحوصات التثبيت، والكشف عن
  مكررة.
- اختبارات ترميز التحقق الذهبي Norito de `DaIngestRequest`، البيان الإلكتروني
  إيصال.
- Harness de integracao subindo mock SoraFS + التسجيل، validando Fluxos de
  قطعة + دبوس.
- اختبارات الملكية كوبريندو بيرفيس دي محو ومجموعات الاحتفاظ
  aleatorias.
- تشويش الحمولات Norito لحماية البيانات التعريفية غير الصحيحة.

## أدوات CLI وSDK (DA-8)- `iroha app da submit` (نقطة الدخول الجديدة CLI) تتضمن الآن المنشئ/الناشر
  تمت مشاركة المحتوى بحيث يقوم المشغلون بإدخال النقط التعسفية
  منتديات لتدفق حزمة Taikai. يا كوماندوز فيف م
  `crates/iroha_cli/src/commands/da.rs:1` واستهلك الحمولة الصافية
  المسح/الاحتفاظ والمحفوظات الاختيارية للبيانات الوصفية/البيان قبل الإضافة
  `DaIngestRequest` canonico مع مهمة تكوين CLI. ينفذون بنجاح
  استمرار `da_request.{norito,json}` و `da_receipt.{norito,json}` تنهد
  `artifacts/da/submission_<timestamp>/` (التجاوز عبر `--artifact-dir`) لذلك
  يتم تحرير القطع المصطنعة من خلال تسجيل وحدات البايت Norito الزائدة المستخدمة خلال فترة
  استيعاب.
- يا كوماندو الولايات المتحدة الأمريكية بور بادراو `client_blob_id = blake3(payload)` ماس أسيتا
  يتم التجاوزات عبر `--client-blob-id`، مع تحديد بيانات تعريف JSON (`--metadata-json`)
  e يظهر ما قبل الاستغلال (`--manifest`)، ويدعم `--no-submit` للتحضير
  غير متصل بالإنترنت ولكن `--endpoint` للمضيفين Torii المخصصين. يا إيصال JSON ه
  ليس من الضروري أن يكون كاتبًا على الديسكو أو متاحًا أو متطلبًا
  أدوات "submit_blob" في DA-8 وتدمير عمل جدار SDK.
- `iroha app da get` إضافة اسم مستعار إلى DA للأوركسترادور متعدد المصادر
  هذا هو الطعام `iroha app sorafs fetch`. يمكن للمشغلين أن يستجيبوا للصناعات
  البيان + خطة القطع (`--manifest`، `--plan`، `--manifest-id`) **ou**
  تمرير تذكرة التخزين Torii عبر `--storage-ticket`. عندما تفعل ذلك
  التذكرة المستخدمة، CLI baixa أو البيان `/v2/da/manifests/<ticket>`،
  استمرار الحزمة sob `artifacts/da/fetch_<timestamp>/` (تجاوز com
  `--manifest-cache-dir`)، مشتق من تجزئة النقطة لـ `--manifest-id`، وما إلى ذلك
  قم بتنفيذ الملف من القائمة `--gateway-provider`. جميع نظام التشغيل
  المقابض المتقدمة لجلب SoraFS دائمة سليمة (مظاريف البيان،
  تسميات العميل، حماية المخابئ، تجاوزات النقل المجهول، التصدير
  لوحة النتائج والمسارات `--output`)، ونقطة النهاية للبيان يمكن أن تكون محددة
  عبر `--manifest-endpoint` للمضيفين Torii المخصصين، بما في ذلك عمليات التحقق من نظام التشغيل
  التوفر من طرف إلى طرف ينبض بالحياة تمامًا أو مساحة الاسم `da` شبه مكررة
  منطق القيام orquestrador.
- `iroha app da get-blob` baixa يُظهر canonicos direto de Torii عبر
  `GET /v2/da/manifests/{storage_ticket}`. يا كوماندوز اخرج
  `manifest_{ticket}.norito`، `manifest_{ticket}.json` ه
  `chunk_plan_{ticket}.json` تنهدات `artifacts/da/fetch_<timestamp>/` (أو أم
  `--output-dir` fornecido pelo user) أثناء الطباعة أو الأمر الإضافي
  `iroha app da get` (بما في ذلك `--manifest-id`) ضروري للقيام بالجلب
  com.orquestrador. هذا هو مشغلي المجلدات الخاصة بمخزن البيانات e
  نضمن أن عملية الجلب ستستمر في استخدام المنتجات التي تم إطلاقها بواسطة Torii. يا
  العميل Torii جافا سكريبت يتحدث أو يتدفق عبر
  `ToriiClient.getDaManifest(storageTicketHex)`، استعادة وحدات البايت لنظام التشغيل Norito
  تم فك التشفير وبيان JSON وخطة القطعة لإخفاء المتصلين بـ SDK
  جلسات الأوركسترا بدون استخدام CLI. O SDK Swift Agora يعرض كـ mesmas
  السطوح (`ToriiClient.getDaManifestBundle(...)` mais
  `fetchDaPayloadViaGateway(...)`)، حزم القنوات للغلاف الأصلي
  orquestrador SoraFS لتمكين عملاء iOS من تثبيت البيانات وتنفيذها
  جلب مصادر متعددة والتقاطها دون الحاجة إلى استدعاء CLI.
  [IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240][IrohaSwift/Sources/IrohaSwift/SorafsOrchestratorClient.swift:12]
- `iroha app da rent-quote` حساب الإيجارات الحتمية وتفاصيلها
  حوافز لحجم التخزين ومستلزمات الاحتفاظ بالبيانات. يا مساعد
  استخدم `DaRentPolicyV1` ativa (JSON أو بايتات Norito) أو embutido الافتراضي،
  التحقق من السياسة وطباعة ملخص JSON (`gib`، `months`، البيانات الوصفية
  السياسة والمجالات في `DaRentQuote`) لكي يستشهد المدققون برسوم XOR الإضافية
  في حالة إدارة نصوص برمجية مخصصة. يا كوماندو تامبيم قم بإصدار ملخص لهم
  uma linha `rent_quote ...` تقوم بتحميل JSON مسبقًا لحفظ سجلات وحدة التحكم
  قانوني خلال التدريبات الحادثة. إمباريلهي
  `--quote-out artifacts/da/rent_quotes/<stamp>.json` كوم
  `--policy-label "governance ticket #..."` للاستمرار في صنع التحف الجميلة
  استشهد بالتصويت أو حزمة التكوينات الإضافية؛ اختصار CLI أو تسمية مخصصة e
  Recusa strings vazias para que valores `policy_source` permanecam acionavis
  لوحات المعلومات لدينا. فيجا
  `crates/iroha_cli/src/commands/da.rs` للأمر الفرعي e
  `docs/source/da/rent_policy.md` للمخطط السياسي.
  [صناديق/iroha_cli/src/commands/da.rs:1][docs/source/da/rent_policy.md:1]
- `iroha app da prove-availability` encadeia tudo acima: احصل على تذكرة تخزين،
  قم بتنزيل حزمة البيان الكنسي أو تشغيلها أو تشغيلها من خلال برنامج متعدد المصادر
  (`iroha app sorafs fetch`) مقابل القائمة `--gateway-provider` للقتل، المستمر
  o الحمولة النافعة baixado + لوحة النتائج sob `artifacts/da/prove_availability_<timestamp>/`،
  استدعاء فوري لمساعد PoR الموجود (`iroha app da prove`) باستخدام نظام التشغيل
  بايت بايت. يمكن للمشغلين ضبط المقابض على orquestrador
  (`--max-peers`، `--scoreboard-out`، يتجاوز بيان نقطة النهاية) e o
  عينة الإثبات (`--sample-count`, `--leaf-index`, `--sample-seed`) في هذا الوقت
  أمر واحد لإنتاج الأدوات الفنية المنتظرة من خلال المستمعين DA-5/DA-9: نسخ
  الحمولة وأدلة لوحة النتائج واستئنافات إثبات JSON.