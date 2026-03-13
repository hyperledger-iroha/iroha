---
lang: ar
direction: rtl
source: docs/portal/docs/da/commitments-plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::ملاحظة المصدر الكنسي
ريفليت `docs/source/da/commitments_plan.md`. قم بمزامنة الإصدارات الثنائية
:::

# خطة الارتباطات توفر البيانات Sora Nexus (DA-3)

_Redige: 25-03-2026 -- المسؤولون: مجموعة عمل البروتوكول الأساسي / فريق العقد الذكي / فريق التخزين_

DA-3 قم بإعادة تنسيق الكتلة Nexus لكل مسار متكامل للتسجيلات
تشتق الحتمية النقط المقبولة على أساس DA-2. هذه ملاحظة التقاط ليه
هياكل البيانات الأساسية، وخطافات خط أنابيب الكتل، والتجهيزات المسبقة
مزيد من المعلومات للعميل والأسطح Torii/RPC التي يجب أن تصل قبلها
يمكن للمدققين الضغط على الارتباطات DA عند الشيكات
القبول أو الحكم. يتم تشفير جميع الحمولات بـ Norito؛ لا دي
SCALE ni de JSON مخصص.

## الأهداف

- Porter des engagements par blob (جذر قطعة + تجزئة واضحة + التزام KZG
  optionnel) في كل كتلة Nexus من أجل إعادة بناء الأزواج القوية
  حالة التوفر بدون استشارة المخزون خارج دفتر الأستاذ.
- تقديم طلبات العضوية المحددة حتى يصبح العملاء قانونيين
  التحقق من تجزئة البيان إلى وضع اللمسات الأخيرة في كتلة دون.
- عرض الطلبات Torii (`/v2/da/commitments/*`) والطلبات المسموح بها
  المرحلات ومجموعات SDK وأتمتة إدارة تدقيق التوافر
  بلا كتلة rejouer chaque.
- الحفاظ على المغلف `SignedBlockWire` الكنسي ونقل الملفات الجديدة
  الهياكل عبر رأس البيانات الوصفية Norito واشتقاق تجزئة الكتلة.

## عرض نطاق النطاق

1. **Ajouts au نموذج البيانات** في `iroha_data_model::da::commitment` plus
   تعديلات رأس الكتلة في `iroha_data_model::block`.
2. **خطافات التنفيذ** التي تمكن `iroha_core` من استيعاب الإيصالات الصادرة بالتساوي
   Torii (`crates/iroha_core/src/queue.rs` و`crates/iroha_core/src/block.rs`).
3. **الثبات/الفهارس** لتمكين WSV من الاستجابة السريعة لطلبات
   الالتزامات (`iroha_core/src/wsv/mod.rs`).
4. **Ajouts RPC Torii** لنقاط نهاية القائمة/المحاضرة/الإثبات
   `/v2/da/commitments`.
5. **اختبارات التكامل + التركيبات** صالحة لتخطيط السلك وإثبات التدفق
   في `integration_tests/tests/da/commitments.rs`.

## 1. نموذج بيانات Ajouts au

### 1.1 `DaCommitmentRecord`

```rust
/// Canonical record stored on-chain and inside SignedBlockWire.
pub struct DaCommitmentRecord {
    pub lane_id: LaneId,
    pub epoch: u64,
    pub sequence: u64,
    pub client_blob_id: BlobDigest,
    pub manifest_hash: ManifestDigest,        // BLAKE3 over DaManifestV1 bytes
    pub proof_scheme: DaProofScheme,          // lane policy (merkle_sha256 or kzg_bls12_381)
    pub chunk_root: Hash,                     // Merkle root of chunk digests
    pub kzg_commitment: Option<KzgCommitment>,
    pub proof_digest: Option<Hash>,           // hash of PDP/PoTR schedule
    pub retention_class: RetentionClass,      // mirrors DA-2 retention policy
    pub storage_ticket: StorageTicketId,
    pub acknowledgement_sig: Signature,       // Torii DA service key
}
```

- `KzgCommitment` يعيد استخدام النقطة 48 ثمانيًا في `iroha_crypto::kzg`.
  عندما يكون غائبًا، في Retombe sur des Preuves Merkle الفريدة.
- `proof_scheme` اشتقاق كتالوج الممرات؛ les Lanes Merkle rejettent les
  الحمولات النافعة KZG tandis que les الممرات `kzg_bls12_381` تتطلب التزامات KZG
  غير فارغة. Torii لا يقوم بإنتاج التزامات Merkle وRejette الحالية
  الممرات التي تم تكوينها في KZG.
- `KzgCommitment` يعيد استخدام النقطة 48 ثمانيًا في `iroha_crypto::kzg`.
  عندما يكون غائبًا عن ممرات Merkle عند العودة إلى Merkle
  فريدة من نوعها.
- `proof_digest` توقع التكامل DA-5 PDP/PoTR لتسجيل الميم
  قم بتعداد جدول أخذ العينات المستخدم للحفاظ على استمرارية النقط.

### 1.2 امتداد رأس الكتلة

```
pub struct BlockHeader {
    ...
    pub da_commitments_hash: Option<HashOf<DaCommitmentBundle>>,
}

pub struct DaCommitmentBundle {
    pub version: u16,                // start with 1
    pub commitments: Vec<DaCommitmentRecord>,
}
```

تجزئة الحزمة بين الوقت في تجزئة الكتلة وفي البيانات الوصفية
`SignedBlockWire`. عندما لا تنقل الكتلة بياناتها DA، فإن البطل يبقى

ملاحظة التنفيذ: `BlockPayload` و`BlockBuilder` كاشف شفاف
صيانة أدوات الضبط/الحروف `da_commitments` (الصوت
`BlockBuilder::set_da_commitments` و`SignedBlock::set_da_commitments`)، دونك
يمكن للمضيفين إرفاق حزمة تم إنشاؤها مسبقًا قبل إنشاء كتلة. طوس
تساعد المساعدة في `None` حتى لا يوفر Torii الحزم
بكرات.

### 1.3 سلك التشفير

- `SignedBlockWire::canonical_wire()` أضف الرأس Norito صب
  `DaCommitmentBundle` فورًا بعد قائمة المعاملات الموجودة.
  بايت الإصدار هو `0x01`.
- `SignedBlockWire::decode_wire()` أعد تجميع الحزم `version` est
  inconnue، en ligne avec la politique Norito decrite dans `norito.md`.
- تعيش أيام اشتقاق التجزئة الفريدة في `block::Hasher`؛
  العملاء الذين قاموا بفك تشفير تنسيق السلك الموجود أصبحوا جددًا
  البطل أتمتة السيارة لو رأس Norito الإعلان عن وجود سا.

## 2. تدفق إنتاج الكتل

1. أدخل DA Torii أنهي `DaIngestReceipt` وانشره في قائمة الانتظار
   داخلي (`iroha_core::gossiper::QueueMessage::DaReceipt`).
2. `PendingBlocks` يجمع جميع الإيصالات ولا يتوافق `lane_id` مع الكتلة
   في الإنشاء، وإلغاء التكرار على قدم المساواة `(lane_id،client_blob_id،
   البيان_التجزئة)`.
3. فقط قبل التغيير، يقوم المنشئ بتجريب الالتزامات حسب `(lane_id, Epoch,
   تسلسل)` لحماية التجزئة المحددة، قم بتشفير الحزمة مع برنامج الترميز
   Norito، والتقيت باليوم `da_commitments_hash`.
4. الحزمة الكاملة مخزنة في WSV وتصدر مع الكتلة في
   `SignedBlockWire`.

إذا تم إنشاء كتلة صدى، فستظل الإيصالات في قائمة الانتظار لذلك
prochaine مؤقت ليه reprenne؛ قام المنشئ بتسجيل الجزء الأخير من `sequence`
بما في ذلك المسار لتجنب هجمات الإعادة.

## 3.Surface RPC والمتطلبات

يعرض Torii نقاط النهاية الثلاثية:

| الطريق | ميثود | الحمولة | ملاحظات |
|-------|--------|---------|-------|
| `/v2/da/commitments` | `POST` | `DaCommitmentQuery` (مرشح النطاق/العصر/التسلسل، ترقيم الصفحات) | Renvoie `DaCommitmentPage` مع الإجمالي والالتزامات وتجزئة الكتلة. |
| `/v2/da/commitments/prove` | `POST` | `DaCommitmentProofRequest` (الممر + التجزئة الواضحة أو الصف `(epoch, sequence)`). | الرد مع `DaCommitmentProof` (سجل + طريق ميركل + تجزئة الكتلة). |
| `/v2/da/commitments/verify` | `POST` | `DaCommitmentProof` | مساعد عديمي الجنسية الذي يستمتع بحساب تجزئة الكتلة ويصلح التضمين؛ استخدم نفس حزم SDK التي لا يمكن أن تكون مباشرة `iroha_crypto`. |

جميع الحمولات حية جدًا `iroha_data_model::da::commitment`. أجهزة التوجيه
يقوم Torii بتركيب المعالجات على جميع نقاط النهاية لاستيعاب DA الموجود
إعادة استخدام الرموز المميزة للسياسة/mTLS.

## 4. إجراءات الإدماج والعملاء القانونيين

- قام منتج الكتلة بإنشاء شجرة Merkle ثنائية على القائمة
  تسلسل `DaCommitmentRecord`. العنصر الغذائي `da_commitments_hash`.
- `DaCommitmentProof` قم بتخزين السجل السلكي بالإضافة إلى ناقل
  `(sibling_hash, position)` حتى يتمكن المحققون من إعادة بناء
  راسين. تتضمن الإجراءات أيضًا تجزئة الكتلة وعلامة الرأس لذلك
  يمكن للعملاء التحقق من النهاية.
- المساعدين CLI (`iroha_cli app da prove-commitment`) يغلفون الدورة
  اطلب/تحقق واكشف عن الطلعات Norito/hex للمشغلين.

## 5. التخزين والفهرسة

يقوم WSV بتخزين الالتزامات في عمود عائلي، واضح تمامًا
`manifest_hash`. الفهارس الثانوية couvrent `(lane_id, epoch)` et
`(lane_id, sequence)` يوضح أن الطلبات تمنع الماسح الضوئي للحزم
يكمل. Chaque Record Suit la hauteur de Bloc qui l'a scelle, permettant aux
قم بالنقر على إعادة إنشاء الفهرس بسرعة من سجل الكتلة.

## 6. القياس عن بعد والمراقبة

- `torii_da_commitments_total` زيادة حجم الكتلة على الأقل
  سجل.
- `torii_da_commitment_queue_depth` يناسب الإيصالات في حالة انتظار الحزمة
  (حارة الاسمية).
- تصور لوحة القيادة Grafana `dashboards/grafana/da_commitments.json`
  تضمين الكتل وعمق قائمة الانتظار وإنتاجية المعالجة
  يمكن لبوابات إطلاق DA-3 مراقبة السلوك.

## 7. استراتيجية الاختبارات

1. **الاختبارات الوحدوية** للتشفير/فك التشفير لـ `DaCommitmentBundle` وآخرون
   لا يوجد يوم من اشتقاق تجزئة الكتلة.
2. **تركيبات ذهبية** sous `fixtures/da/commitments/` تلتقط البايتات
   Canoniques du Bundle et les Preuves Merkle.
3. **اختبارات التكامل** تحدد مدققين اثنين، وتستوعب النقطتين
   الاختبار والتحقق من أن العقدين الجديدين متطابقان مع محتوى الحزمة
   ردود الاستعلام/الإثبات.
4. **اختبارات العميل الخفيف** في `integration_tests/tests/da/commitments.rs`
   (Rust) الذي استأنف `/prove` وتحقق من صحة الأمر بدون التحدث إلى Torii.
5. **Smoke CLI** مع `scripts/da/check_commitments.sh` لحماية الإمداد
   عامل قابل للتكرار.

## 8. خطة الطرح

| المرحلة | الوصف | معايير الخروج |
|-------|------------|---------------|
| P0 - دمج نموذج بيانات دو | Integrer `DaCommitmentRecord`، يعرض يوم رأس الكتلة وبرامج الترميز Norito. | `cargo test -p iroha_data_model` vert avec nouvelles تركيبات. |
| P1 - الأسلاك الأساسية/WSV | قم بربط منطق قائمة الانتظار + أداة إنشاء الكتل ومواصلة الفهارس وكشف معالجات RPC. | `cargo test -p iroha_core`، `integration_tests/tests/da/commitments.rs` تم تمريره مع تأكيدات إثبات الحزمة. |
| P2 - مشغل الأدوات | Livrer helper CLI، لوحة المعلومات Grafana، ويعرض يوم من المستندات للتحقق من الإثبات. | `iroha_cli app da prove-commitment` يعمل على جهاز التطوير؛ تعرض لوحة القيادة البيانات مباشرة. |
| P3 - بوابة الحكم | يتطلب تنشيط مدقق الكتل الالتزامات DA على الممرات المشار إليها في `iroha_config::nexus`. | دخول الحالة + تحديث خريطة الطريق الخاصة بـ DA-3 باسم TERMINE. |

## الأسئلة المفتوحة1. **KZG vs Merkle defaults** - قم بذلك طوال الوقت متجاهلاً التزامات KZG من أجل
   هل النقط الصغيرة قادرة على تقليل حجم الكتل؟ الاقتراح: جاردر
   `kzg_commitment` optionnel والبوابة عبر `iroha_config::da.enable_kzg`.
2. **فجوات التسلسل** - التشغيل التلقائي للممرات خارج الترتيب؟ الخطة الفعلية متجددة
   الفجوات سوف تكون si la gouvernance active `allow_sequence_skips` من أجل إعادة التشغيل
   حالة الطوارئ.
3. **ذاكرة التخزين المؤقت للعميل الخفيف** - قم بتجهيز SDK لطلب ذاكرة تخزين مؤقت SQLite أسهل من أجلها
   البراهين. suivi en attente sous DA-8.

الرد على هذه الأسئلة في علاقات التنفيذ بعد مرور DA-3
BROUILLON (هذا المستند) في دورة تدريبية حول بدء عمل التعليمات البرمجية.