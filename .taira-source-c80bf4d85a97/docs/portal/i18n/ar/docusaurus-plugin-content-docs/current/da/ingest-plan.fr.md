---
lang: ar
direction: rtl
source: docs/portal/docs/da/ingest-plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::ملاحظة المصدر الكنسي
ريفليت `docs/source/da/ingest_plan.md`. قم بمزامنة الإصدارات الثنائية بشكل متزامن
:::

# خطة استيعاب توفر البيانات Sora Nexus

_Redige: 2026/02/20 - المسؤولون: مجموعة عمل البروتوكول الأساسي / فريق التخزين / DA WG_

مسار العمل DA-2 يصل إلى Torii مع واجهة برمجة تطبيقات استيعاب النقط التي يتم إنشاؤها
metadonnees Norito ويحب النسخ المتماثل SoraFS. التقاط وثيقة Ce جنيه
يقترح المخطط واجهة برمجة التطبيقات السطحية وتدفق التحقق من الصحة
التنفيذ المسبق بدون حجب عمليات المحاكاة المتبقية (suivi
دا-1). جميع تنسيقات الحمولة DOIVENT تستخدم برامج الترميز Norito؛ com.aucun
serde/JSON ليس مسموحًا به.

## الأهداف

- قبول النقط الحجمية (شرائح Taikai، وsidecars de lane، وartifacts de
  الحوكمة) الطريقة المحددة عبر Torii.
- إنتاج البيانات Norito الأساسية المشتقة من النقطة، المعلمات
  برنامج الترميز وملف تعريف المحو وسياسة الاحتفاظ.
- الاحتفاظ بالبيانات الوصفية للأجزاء في المخزون الساخن من SoraFS والتعامل معها
  ملف وظائف النسخ المتماثل.
- نشر نوايا الدبوس + العلامات السياسية في السجل SoraFS وآخرون
  مراقبو الحكم.
- الكشف عن إيصالات القبول حتى يتمكن العملاء من إعادة النظر في الأمر
  تحديد النشر.

## واجهة برمجة تطبيقات السطح (Torii)

```
POST /v1/da/ingest
Content-Type: application/norito+v1
```

تم تشفير الحمولة باستخدام `DaIngestRequest` ضمن Norito. يتم استخدام الردود
`application/norito+v1` و renvoient `DaIngestReceipt`.

| الرد | الدلالة |
| --- | --- |
| 202 مقبول | Blob en file صب التقطيع/النسخ المتماثل؛ إيصال الاستلام. |
| 400 طلب سيء | انتهاك المخطط/التفاصيل (التحقق من التحقق من الصحة). |
| 401 غير مصرح به | واجهة برمجة تطبيقات الرمز المميز غير صالحة/غير صالحة. |
| 409 الصراع | Doublon `client_blob_id` مع البيانات الوصفية غير محددة. |
| 413 الحمولة كبيرة جدًا | تجاوز الحد الذي تم تكوينه لطول النقطة. |
| 429 طلبات كثيرة جدًا | الوصول إلى الحد الأقصى للمعدل. |
| 500 خطأ داخلي | Echec inattendu (سجل + تنبيه). |

## اقتراح المخطط Norito

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

> ملاحظة التنفيذ: تمثيلات Rust canoniques لهذه الحمولات
> استمرار التشغيل المستمر `iroha_data_model::da::types`، مع الأغلفة
> الطلب/الاستلام في `iroha_data_model::da::ingest` وهيكله
> البيان في `iroha_data_model::da::manifest`.

يعلن البطل `compression` عن تعليق المتصلين على إعداد الحمولة. Torii
قبول `identity`، و`gzip`، و`deflate`، و`zstd`، وإلغاء ضغط البايتات الأمامية
التجزئة والتقطيع والتحقق من خيارات البيانات.

### قائمة التحقق من الصحة

1. تحقق من أن الرأس Norito للطلب يتوافق مع `DaIngestRequest`.
2. Echouer si `total_size` يختلف عن الطول الكنسي للحمولة النافعة
   (فك الضغط) أو تجاوز الحد الأقصى للتكوين.
3. فرض محاذاة `chunk_size` (قوة مزدوجة، <= 2 MiB).
4. الضامن `data_shards + parity_shards` <= الحد الأقصى للتكافؤ العالمي والتكافؤ >= 2.
5.`retention_policy.required_replica_count` احترم خط الأساس
   الحكم.
6. التحقق من التوقيع ضد التجزئة التقليدية (باستثناء البطل).
   التوقيع).
7. قم بإلغاء تحديد `client_blob_id` مرتين إذا قمت بتجزئة الحمولة والبيانات الوصفية
   معرفات الابن.
8. عند `norito_manifest`، يتم التحقق من المخطط + مراسل التجزئة
   إعادة حساب البيان بعد التقطيع؛ Sinon Le Noeud Genere Le Manifest et Le
   ستوك.
9. قم بإضافة سياسة النسخ المتماثل التي تم تكوينها: Torii قم بإعادة كتابة
   `RetentionPolicy` مع `torii.da_ingest.replication_policy` (العرض
   `replication-policy.md`) وأعد مسح البيانات المسبقة
   لا تتوافق البيانات الوصفية التي يتم الاحتفاظ بها مع الملف الشخصي.

### تدفق القطع والنسخ المتماثل

1. فك الحمولة في `chunk_size`، آلة حاسبة BLAKE3 par قطعة + راسين ميركل.
2. إنشاء Norito `DaManifestV1` (البنية الجديدة) لالتقاط الارتباطات
   dechunk (role/group_id)، le Layout d'erasure (comptes de parite de lignes et
   Colonnes plus `ipa_commitment`)، سياسة الاحتفاظ والبيانات الوصفية.
3. قم بإنشاء ملف بايتات البيان القانوني الفرعي
   `config.da_ingest.manifest_store_dir` (Torii كتابة الملفات
   `manifest.encoded` الفهارس الاسمية للحارة/العصر/التسلسل/التذكرة/بصمة الإصبع) afin
   تقوم عملية التنسيق SoraFS بإدخال واعتماد تذكرة التخزين على البيانات
   المثابرة.
4. نشر أهداف الدبوس عبر `sorafs_car::PinIntent` مع علامة الإدارة
   وآخرون سياسة.
5. أرسل الحدث Norito `DaIngestPublished` لإعلام المراقبين
   (قانون العملاء، الحوكمة، التحليل).
6. Renvoyer `DaIngestReceipt` au المتصل (توقيع حسب مفتاح الخدمة DA de Torii)
   وقم بإخراج الرأس `Sora-PDP-Commitment` حتى تلتقط SDKs
   التزام ترميز فوري. يتضمن الإيصال الصيانة `rent_quote`
   (un Norito `DaRentQuote`) و`stripe_layout`، مُرسلو المساعدة المسموح بها
   عرض الإيجار الأساسي والاحتياطي وانتظارات PDP/PoTR الإضافية وما إلى ذلك
   تخطيط محو 2D لتذكرة التخزين قبل مشاركة الملفات.

## يفتقد مخزون/تسجيل يوم

-العنوان `sorafs_manifest` مع `DaManifestV1`، يسمح بالتحليل
  الحتمية.
- إضافة دفق جديد من التسجيل `da.pin_intent` مع إصدار حمولة واحد
  المرجع تجزئة البيان + معرف التذكرة.
- قم بضبط خطوط الأنابيب التي يمكن ملاحظتها يوميًا لمتابعة زمن تأخر الإدخال،
  إنتاجية التقطيع وتراكم النسخ المتماثل والحسابات
  d'echecs.

## استراتيجية الاختبارات

- اختبارات موحدة للتحقق من صحة المخطط، والتحقق من التوقيع، والكشف عن
  مضاعفة.
- اختبارات التحقق الذهبي من الترميز Norito de `DaIngestRequest`، البيان وآخرون
  إيصال.
- أداة التكامل التي تحدد SoraFS + محاكاة التسجيل، والوثائق الصحيحة
  تدفق قطعة + دبوس.
- اختبارات الملكية تغطي ملفات التعريف والمجموعات
  أليات الاحتفاظ.
- تشويش الحمولات Norito لحماية بيانات التعريف الخاطئة.

## الأدوات CLI وSDK (DA-8)- `iroha app da submit` (نقطة الدخول الجديدة CLI) المغلف الرئيسي للباني
  المشاركة تمكن المشغلين من دمج النقط
  المحكمون خارج حزمة تدفق Taikai. لا أمر فيت دان
  `crates/iroha_cli/src/commands/da.rs:1` واستهلك حمولة وملفًا شخصيًا
  المسح/الاحتفاظ والملفات الاختيارية للبيانات الوصفية/البيان المسبق
  قم بالتوقيع على `DaIngestRequest` canonique مع مفتاح config CLI. ليه يركض
  reussis المستمر `da_request.{norito,json}` et `da_receipt.{norito,json}` sous
  `artifacts/da/submission_<timestamp>/` (التجاوز عبر `--artifact-dir`) للتوصل إلى ذلك
  يتم تحرير القطع الأثرية وتسجيل وحدات البايت Norito التي تستخدم قلادة بالضبط
  l'ingest.
- يتم استخدام الأمر افتراضيًا `client_blob_id = blake3(payload)` بشكل مقبول
  التجاوزات عبر `--client-blob-id`، تكريم خرائط JSON للبيانات الوصفية
  (`--metadata-json`) والبيانات السابقة (`--manifest`)، ودعم
  `--no-submit` للتحضير في وضع عدم الاتصال بالإضافة إلى `--endpoint` للمضيفين
  يتم تخصيص Torii. يتم طباعة الإيصال JSON على الوضع القياسي بالإضافة إلى ذلك
  أكتب على القرص، وأؤكد على الحاجة إلى الأدوات "submit_blob" من DA-8 وآخرون
  إلغاء حظر العمل على قدم المساواة SDK.
- `iroha app da get` أضف اسمًا مستعارًا DA لمنسق المصادر المتعددة المغذية
  ديجا `iroha app sorafs fetch`. يمكن للمشغلين رؤية المؤشر على القطع الأثرية
  البيان + خطة القطعة (`--manifest`، `--plan`، `--manifest-id`) **ou**
  تذكرة تخزين الأمم المتحدة Torii عبر `--storage-ticket`. Quand le chemin تذكرة مؤسسة
  استخدم CLI لاستعادة البيان من `/v1/da/manifests/<ticket>`،
  استمرار الحزمة الفرعية `artifacts/da/fetch_<timestamp>/` (تجاوز avec
  `--manifest-cache-dir`)، اشتقاق تجزئة النقطة من أجل `--manifest-id`، ثم
  قم بتنفيذ المُنسق باستخدام القائمة `--gateway-provider`. كل شيء
  المقابض avances du fetcher SoraFS المتبقية سليمة (مظاريف البيان والملصقات
  العميل، حراسة المخابئ، تجاوزات النقل المجهول، لوحة النتائج التصديرية وما إلى ذلك
  المسارات `--output`)، وقد تظهر نقطة النهاية بتكلفة إضافية عبر
  `--manifest-endpoint` من أجل المضيفين Torii مخصص، قم بإجراء عمليات التحقق
  التوفر الشامل يعيش بشكل كامل داخل مساحة الاسم `da` sans
  تكرار منطق الأوركسترا.
- `iroha app da get-blob` يستعيد البيانات الأساسية مباشرة من Torii
  عبر `GET /v1/da/manifests/{storage_ticket}`. لا أمر مكتوب
  `manifest_{ticket}.norito`، `manifest_{ticket}.json` وآخرون
  `chunk_plan_{ticket}.json` sous `artifacts/da/fetch_<timestamp>/` (أو
  `--output-dir` يقدمه المستخدم) لعرض الأمر الدقيق
  يتطلب `iroha app da get` (بما في ذلك `--manifest-id`) جلب المنسق.
  يحافظ على المشغلين خارج نطاق الذخيرة ويضمن لهم
  يستخدم جهاز الجلب جميع العناصر المنبعثة من العلامات وفقًا لـ Torii. لو العميل
  Torii يقوم JavaScript بإعادة إنتاج تدفق ce عبر
  `ToriiClient.getDaManifest(storageTicketHex)`، يستعيد البايتات Norito
  يتم فك التشفير وبيان JSON وخطة القطعة حتى يتمكن المتصلون من SDK من الترطيب
  جلسات التنسيق بدون المرور عبر CLI. كشف لو SDK سويفت
  صيانة أسطح الميمات (`ToriiClient.getDaManifestBundle(...)` plus
  `fetchDaPayloadViaGateway(...)`)، الحزم المتفرعة على الغلاف الأصلي
  منسق SoraFS لتمكين عملاء iOS من تنزيله
  البيانات، منفذ الجلب متعدد المصادر، والتقاط الصور بدون أخطاء
  استدعاء لا CLI.
  [IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240][IrohaSwift/Sources/IrohaSwift/SorafsOrchestratorClient.swift:12]
- `iroha app da rent-quote` حساب الإيجارات المحددة والتهوية
  حوافز لحجم التخزين ونافذة الاحتفاظ الأربعة.
  المساعدة في استخدام `DaRentPolicyV1` نشط (JSON أو بايت Norito) أو
  التكامل الافتراضي، التحقق من السياسة وطباعة السيرة الذاتية JSON (`gib`،
  `months`، البيانات الوصفية السياسية، الأبطال `DaRentQuote`) لمراجعي الحسابات
  اذكر الرسوم XOR الدقيقة في محاضر الإدارة بدون نصوص إعلانية
  مخصص. الأمر يصدر أيضًا استئنافًا عبر خط `rent_quote ...` قبل ذلك
  حمولة JSON لحفظ سجلات وحدة التحكم غير المرئية المعلقة على التدريبات
  حادثة. أسوسيز `--quote-out artifacts/da/rent_quotes/<stamp>.json` مع
  `--policy-label "governance ticket #..."` لاستمرار الأعمال الفنية
  citant le التصويت أو حزمة التكوين الدقيق؛ La CLI tronque le label personnalise
  ورفض السلاسل المرئية حتى تبقى القيم `policy_source`
  actionnables في لوحات المعلومات في الخزانة. إستفتاء
  `crates/iroha_cli/src/commands/da.rs` للتحكم في الأمر وآخرين
  `docs/source/da/rent_policy.md` للمخطط السياسي.
  [صناديق/iroha_cli/src/commands/da.rs:1][docs/source/da/rent_policy.md:1]
- `iroha app da prove-availability` السلسلة التي تسبق: il prend un تخزين
  تذكرة، تنزيل الحزمة الكنسي للبيان، تنفيذ الأوركسترا
  متعدد المصادر (`iroha app sorafs fetch`) مقابل القائمة `--gateway-provider`
  فورني، استمر في تحميل الحمولة + لوحة النتائج
  `artifacts/da/prove_availability_<timestamp>/`، واستدعاءه على الفور
  المساعد PoR موجود (`iroha app da prove`) مع البايتات المستردة. المشغلون
  يمكن ضبط مقابض التحكم (`--max-peers`, `--scoreboard-out`,
  يتجاوز بيان نقطة النهاية) وعينة الإثبات (`--sample-count`،
  `--leaf-index`, `--sample-seed`) ومن ثم يأمر المنتج وحده
  المصنوعات اليدوية حاضرة حسب عمليات التدقيق DA-5/DA-9: نسخة من الحمولة، دليل على ذلك
  لوحة النتائج واستئناف JSON.