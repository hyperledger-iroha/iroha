---
lang: ar
direction: rtl
source: docs/portal/docs/da/commitments-plan.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::ملاحظة المصدر القياسي
تعكس `docs/source/da/commitments_plan.md`. ابق النسختين متزامنتين حتى يتم سحبه
الوثائق القديمة.
:::

#خطة تعهدات توفر البيانات في Sora Nexus (DA-3)

_مسودة: 25-03-2026 -- المالكون: Core Protocol WG / Smart Contract Team / Storage Team_

يمدد DA-3 تنسيق كتلة Nexus بحيث تم دمج كل سجلات الحارة حتمية تصف الـ blobs السهلة
من DA-2. لأن هذه المذكرة هياكل البيانات القياسية، وبما أنها خط انابيب الكتل،
وبرهان عملاء التجزئة، واسطح Torii/RPC التي يجب ان تكتمل قبل ان يعتمد
المتدربون على تعهدات DA خلال الاختبارات التي تتحسن او التحسن. جميع الحمولات مشفرة
بـ Norito؛ لا SCALE ولا JSON مخصص.

## الاهداف

- حمل تعهدات لكل blob (chunk root + Manifest hash + تعهدات KZG اختياري) داخل كل
  Nexus كتلة لكي يتم النظر في إعادة بناء حالة توافرها دون الرجوع
  الى تخزين الدفتر.
- توفير براهين عضوية حتمية لكي يصنع العملاء الجدد من تجزئة البيان
  تثبيته في كتلة محددة.
- كشف استعلامات Torii (`/v2/da/commitments/*`) وبراهين تسمح للـ Relays وSDKs
  وادوات تشغيل ال تور بتدقيق التوفر دون إعادة كل الكتلة.
- تحقق على ظرف `SignedBlockWire` القياسي عبر TB6 البنى الجديدة من خلال
  ترويسة بيانات Norito الوصفية واشتقاق hash الجاذبية.

## نظرة عامة على النطاق

1. **اضافات نموذج البيانات** في `iroha_data_model::da::commitment` مع التغيرات
   ترويسة الكتلة في `iroha_data_model::block`.
2. **خطافات للمنفذ** حتى يقوم `iroha_core` بابتلاع الإيصالات الخاصة بـ DA
   توجيه من Torii (`crates/iroha_core/src/queue.rs` و
   `crates/iroha_core/src/block.rs`).
3. **الفهارس المستمرة** حتى بدأت WSV من اجابة استعلامات التعهدات بسرعة
   (`iroha_core/src/wsv/mod.rs`).
4. **اضافات RPC في Torii** لقائمة النقاط/الاستعلام/الإثبات تحت `/v2/da/commitments`.
5. **اختبارات تكامل + تركيبات** متعددة من تخطيط الأسلاك وإثبات التدفق
   `integration_tests/tests/da/commitments.rs`.

##1.إضافات نموذج البيانات

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

- `KzgCommitment` إعادة استخدام النقطة ذات 48 بايت الموجود في
  `iroha_crypto::kzg`. عند غيابها نعود الى براهين ميركل فقط.
-`proof_scheme` ممرات مفتوحة من كتالوج؛ حارات من نوع Merkle ترفض حمولات KZG،
  بينما الممرات `kzg_bls12_381` تتطلب تعهدات KZG غير صفرية. Torii مساء النتيجة
  تعهدات ميركل فقط ويرفض الممرات الهيئة على KZG.
- `KzgCommitment` إعادة استخدام النقطة ذات 48 بايت الموجود في
  `iroha_crypto::kzg`. عند غيابها في ممرات ميركل نعود الى براهين ميركل فقط.
- `proof_digest` يمهد لتكامل DA-5 PDP/PoTR ليسجل السجل بنفسه جدول اخذ
  عدة المستخدم على النقط.

### 1.2 الأوقات ترويسة كوز

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

يدخل التجزئة الخاصة بالحزمة في التجزئة وفي بيانات `SignedBlockWire`. عندما لا

ملاحظة تنفيذية: `BlockPayload` و`BlockBuilder` الشاشة المعروضة الان
setters/getters لـ `da_commitments` (راجع `BlockBuilder::set_da_commitments`
و`SignedBlock::set_da_commitments`) من أجل المضيفين المبتدئين من اضافات الحزمة الإضافية
الانشاء بواسطة ختم كندا. جميع الدوال المساعدة غادر الحقل `None` حتى يمر Torii
حزم حقيقية.

###سلك ترميز 1.3

- `SignedBlockWire::canonical_wire()` ترويسة Norito لـ
  `DaCommitmentBundle` مباشرة بعد إدخال المعاملات الحالية. بايت الاصدار هو
  `0x01`.
- `SignedBlockWire::decode_wire()` يرفض الحزم ذات `version` غير معروف، بما في ذلك
  مرحبا بكم مع Norito الموضحة في `norito.md`.
- تحديثات اشتقاق hash موجودة فقط في `block::Hasher`; العملاء الذين
  فيككون سلك الشكل الحالي النامية على الحقل الجديد لان ترويسة Norito
  نعلن عن وجودها.

##2. تدفق إنتاج الكتل

1. تنهي عملية ابتلاع الخاصة بـ Torii DA ايصال `DaIngestReceipt` ليشيكه
   على الطابور الداخلي (`iroha_core::gossiper::QueueMessage::DaReceipt`).
2. يجمع `PendingBlocks` كل الإيصالات التي تطابق `lane_id` للكتلة قيد البناء،
   مع إزالة التكرار حسب `(lane_id, client_blob_id, manifest_hash)`.
3. قبل الختم مباشرة، يقوم المنشئ بفرز التعهدات حسب `(lane_id, epoch, sequence)`
   القتال على التجزئة حتمي، ويقوم بترميز الحزمة باستخدام Norito، ويحدث
   `da_commitments_hash`.
4. يتم تخزينه في الثلاجة بشكل كامل في WSV ويتصدر الكتلة داخل `SignedBlockWire`.

اذا فشل إنشاء الكتلة وجود إيصالات في الطابور الكامل لالتقاطها في المحاولة
التالية؛ ويسجل builder اخر `sequence` تم تضمينه لكل حارة يمنع منع الإعادة.

## 3. سطح RPC للاستعلام

يوفر Torii ثلاثة نقاط نهاية:

| المسار | الطريقة | الحمولة | تعليقات |
|--------|---------|--------|---------|
| `/v2/da/commitments` | `POST` | `DaCommitmentQuery` (فلترة بنطاق النطاق/العصر/التسلسل، مع ترقيم الصفحات) | أعاد `DaCommitmentPage` المجدي الاجمالي والتعهدات و التجزئة. |
| `/v2/da/commitments/prove` | `POST` | `DaCommitmentProofRequest` (الممر + تجزئة البيان أو tuple `(epoch, sequence)`). | إعادة `DaCommitmentProof` (سجل + مسار ميركل + هاش كيدج). |
| `/v2/da/commitments/verify` | `POST` | `DaCommitmentProof` | مساعد عديم الجنسية يعيد حساب التجزئة الكتلة ويتحقق من الاشتمال؛ يستخدمه SDKs التي لا يمكن الربط مباشرة مع `iroha_crypto`. |

كل الحمولات تعيش تحت `iroha_data_model::da::commitment`. يقوم بتركيب Torii
تقوم المعالجات بجانب نقاط النهاية باستقبال LDA الحالي لاعادة استخدام سياسات token/mTLS.

## 4. براهين الاشتمال المحاكمة الخفيفة

- يبني منتج الكتل شجرة Merkle ثنائية فوق قائمة `DaCommitmentRecord`
  مسلسل. تريجر يغذي `da_commitments_hash`.
- يتجه إلى `DaCommitmentProof` سجل الاتجاه نحو `(sibling_hash, position)`
  ليحدد المستغرقون بناء على رغبتك. تتضمن البراهين أيضًا تجزئة الكتلة والترويسة
  الموقع حتى العملاء الجدد من التحقق من النهائية.
- تساعد اوامر CLI (`iroha_cli app da prove-commitment`) على تنفيذ طلب/تحقق
  البراهين وتعرض مخارج Norito/hex للمشغلين.

## 5. التخزين والفهرسة

يخزن WSV التعهدات في عائلة العمود مخصص بمفتاح `manifest_hash`. يغطي
الفهارس الثانوية `(lane_id, epoch)` و`(lane_id, sequence)` كي تتجنب تجنبات
حزمة الحذف الكاملة. يتتبع كل سجل الأكاديمية التي ختمته، مما يسمح بالعقد في
مرحلة اللحاق باعادة بناء الفهرس بسرعة من سجل الكتل.

## 6. القياس والرصد

- تحتوي على `torii_da_commitments_total` يزيد عند ختم الكتلة على سجل واحد على المبلغ.
- `torii_da_commitment_queue_depth` تتبع الإيصالات المنتظرة للجميع (لكل حارة).
- لوحة Grafana `dashboards/grafana/da_commitments.json` تم تصوير ادراج الكتل وعمق
  الطابور وتدفق البراهين حتى بوابات اصدار DA-3 من تدقيق السلوك.

## 7. استراتيجية التحدي

1. **اختبارات وحدات ليتر**/فك ترميز `DaCommitmentBundle` وتحديثات اشتقاق hash
   اكيا.
2. **التركيبات الذهبية** تحت `fixtures/da/commitments/` لتقط بايت بايت
   المستوى القياسي وبراهين ميركل.
3. **الاختبارات المتكاملة** تحسب مترين، وتبتلع النقط السعرية المزدوجة، وتتحقق من ان
   تعقد كلاتين تتفقان على البث الفضائي وعربات النقل/البرهان.
4. **اختبارات العملاء الخفيفين** في `integration_tests/tests/da/commitments.rs`
   (الصدأ) يستدعي `/prove` وتتحقق من البرهان دون جراثيم الى Torii.
5. **Smoke CLI** عبر `scripts/da/check_commitments.sh` لابقاء ادوات التشغيل
   لا يمكن إنتاج السعادة.

## 8. خطة الاطلاق

| المرحلة | الوصف | اعتماد الخروج |
|---------|-------|--------------|
| P0 - دمج نموذج البيانات | دمج `DaCommitmentRecord` وتحديثات ترويسة كويكودات Norito. | `cargo test -p iroha_data_model` ناجح بالتركيبات الجديدة. |
| P1 - توصيل كور/WSV | تسارع منطق الطابور + block builder، حفظ الفهارس، وكشف معالجات RPC. | `cargo test -p iroha_core` و `integration_tests/tests/da/commitments.rs` ناجحان مع إثبات حزمة الثبات. |
| P2 - ادوات التشغيل | شحن المساعدين للـ CLI ولوحة Grafana وتحديثات تحقق البرهان. | `iroha_cli app da prove-commitment` يعمل على devnet؛ اللوحة تعرضت لبيانات حية. |
| ج3 - بوابة ال تور | تفعيل معمل الكتل الذي يفرض تعهدات DA على الممرات المحددة في `iroha_config::nexus`. | تحديث الحالة وخارطة الطريق يشيران الى القانوني DA-3. |

##مسابقات

1. **KZG vs Merkle defaults** - هل يجب تخطي تعهدات KZG للـ blobs الرئيسية الرئيسية
   حجم اكيدا؟ الاقتراحات: بلس `kzg_commitment` اختياريا وتفعيله عبر
   `iroha_config::da.enable_kzg`.
2. **فجوات التسلسل** - هل نسمح بفجوات الترتيب؟ البناء الحالي يرفض الفجوات الا
   اذا فعلت الـ `allow_sequence_skips` لا تسير الأمور بسرعة.
3. ** ذاكرة التخزين المؤقت للعميل الخفيف ** - طلب فريق SDK تخزين SQLite خفيف للبراهين؛ متابعة
   لاحقًا تحت DA-8.

الإجابة على هذه الاسئلة في PRs تواصل DA-3 من مسودة (هذه القيادة) الى
قيد العمل بمجرد بدء التنفيذ البرمجي.