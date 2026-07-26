---
lang: ar
direction: rtl
source: docs/portal/docs/da/commitments-plan.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::ملاحظة مستند ماخذ
لنقم بإلغاء مزامنة المزامنة.
:::

# Sora Nexus خطة التزامات توفر البيانات (DA-3)

_مسودہ: 25-03-2026 -- مالکان: Core Protocol WG / Smart Contract Team / Storage Team_

DA-3 Nexus شريط أسود واسع النطاق ومسار حتمي
السجلات تتضمن DA-2 التي تم قبولها من قبل النقط. لا جديد
هياكل البيانات الأساسية، خطافات خطوط الأنابيب، إثباتات العميل الخفيف، أو
أسطح Torii/RPC عبارة عن أدوات التحقق من الصحة والقبول والحوكمة
يتم إجراء الشيكات على التزامات DA حسب الضرورة القصوى. تمام الحمولات
ترميز Norito؛ لا يوجد مقياس أو JSON المخصص.

##مقاصد

- Nexus الالتزامات لكل فقاعة (جذر قطعة + تجزئة البيان + اختياري)
  التزام KZG) يتضمن التخزين خارج دفتر الأستاذ لجميع الأقران
  حالة التوفر مرة أخرى.
- أدلة العضوية الحتمية التي يمكن للعملاء الخفيفين التحقق منها
  تم وضع اللمسات الأخيرة على تجزئة البيان الخاصة بمخصوص البلاك.
- استعلامات Torii (`/v1/da/commitments/*`) ومرحلات البراهين،
  أدوات تطوير البرامج (SDK) وأتمتة الحوكمة توفر إمكانية إعادة التشغيل بلا حدود
  مراجعة الحسابات.
- `SignedBlockWire` المغلف وهو الرنك الأساسي، والهياكل الجديدة التي هي Norito
  رأس البيانات الوصفية واشتقاق تجزئة الكتلة سے موضوع کرتے ہوئے.

## نظرة عامة على النطاق

1. **إضافات نماذج البيانات** `iroha_data_model::da::commitment` وكتلة
   تغييرات الرأس `iroha_data_model::block`.
2. ** خطافات المنفذ ** `iroha_core` Torii تصدر إيصالات DA
   كے (`crates/iroha_core/src/queue.rs` و`crates/iroha_core/src/block.rs`).
3. **الثبات/الفهارس** يمكنك التعامل مع استعلامات التزامات WSV
   (`iroha_core/src/wsv/mod.rs`).
4. **Torii إضافات RPC** قائمة/استعلام/إثبات نقاط النهاية
   `/v1/da/commitments` تحت.
5. ** اختبارات التكامل + التركيبات ** تخطيط سلك الهواء والتحقق من صحة التدفق
   البطاقة `integration_tests/tests/da/commitments.rs`.

## 1. إضافات نماذج البيانات

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

- `KzgCommitment` متاح وبطاقة 48 بايت قابلة لإعادة الاستخدام و`iroha_crypto::kzg`
  . إذا كنت غائبًا، يمكنك استخدام أدلة Merkle.
- كتالوج الممرات `proof_scheme` سے اشتق ہوتا ہے؛ ممرات Merkle بحمولات KZG
  رفض البطاقة التي تتطلب التزامات KZG للممرات غير الصفرية `kzg_bls12_381`
  شكرا جزيلا. Torii في الواقع يصرف التزامات Merkle بناتا ہے وتكوين KZG
  الممرات کو ترفض کرتا ہے۔
- `KzgCommitment` متاح وبطاقة 48 بايت قابلة لإعادة الاستخدام و`iroha_crypto::kzg`
  . إذا كانت ممرات Merkle غائبة، يمكنك استخدام أدلة Merkle بشكل فعال.
- `proof_digest` DA-5 تكامل PDP/PoTR نقرة واحدة فقط
  سجل جدول أخذ العينات ودرج النقط التي تستخدمها
  ہوتا ہے۔

### 1.2 ملحق رأس الكتلة

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

تتضمن حزمة التجزئة بلا هاش والبيانات الوصفية `SignedBlockWire` دونوں كل ما تحتاجه. ج
بچيں۔

ملاحظة التنفيذ: `BlockPayload` وشفاف `BlockBuilder` اب
`da_commitments` يعرض المستوطنون/الحاصلون على البطاقة (ممتاز)
`BlockBuilder::set_da_commitments` و`SignedBlock::set_da_commitments`)، إلخ
المضيفون بلا ختم يتم إرفاق حزمة مسبقة الصنع بإرفاقها. تمام مساعد
مجال المنشئين الذي `None` مقيد بـ Torii لا يوجد خيط حزم حقيقي.

### 1.3 تشفير الأسلاك

- `SignedBlockWire::canonical_wire()` قائمة المعاملات موجودة فوراً
  `DaCommitmentBundle` إلحاق Norito رأس كرتا ہے. بايت الإصدار `0x01` ہے۔
- `SignedBlockWire::decode_wire()` `version` غير معروف الحزم التي ترفض كرتا
  لقد تم استخدام `norito.md` في سياسة Norito.
- تحديثات اشتقاق التجزئة تم إنفاقها `block::Hasher`؛ تنسيق الأسلاك الموجودة
  يعد فك رموز العملاء الخفيفين مجالًا خاصًا بهم
  رأس Norito به بيانات موجودة.

## 2. تدفق إنتاج الكتل

1. Torii DA استيعاب `DaIngestReceipt` إنهاء القائمة وقائمة الانتظار الداخلية
   پر نشر کرتا ہے (`iroha_core::gossiper::QueueMessage::DaReceipt`).
2. `PendingBlocks` جميع الإيصالات المجمعة منذ `lane_id` تم الانتهاء منها باللون الأسود
   مطابقة الكرتا ہے، و `(lane_id, client_blob_id, manifest_hash)` لإلغاء التكرار
   كرتا ہے۔
3. ختم التزامات المنشئ `(lane_id, epoch, sequence)`
   فرز كرتا ہے تاكہ تجزئة حتمية رہے، حزمة ترميز Norito سے ترميز
   كرتا، وتحديث `da_commitments_hash` كرتا.
4. حزمة كاملة من متجر WSV متجددة و`SignedBlockWire` ومستقرة باللون الأسود
   تنبعث منها ہوتا ے۔

إذا فشلت جهودنا البيضاء، فستساعدك قائمة انتظار الإيصالات في الحصول على دفعة إضافية
سکے؛ منشئ الممرات الأخيرة يتضمن `sequence` تسجيل وإعادة التشغيل
الهجمات سے بچا جا سکے۔

## 3. RPC وسطح الاستعلام

Torii نقاط النهاية هذه هي:

| الطريق | الطريقة | الحمولة | ملاحظات |
|-------|--------|---------|-------|
| `/v1/da/commitments` | `POST` | `DaCommitmentQuery` (مرشح نطاق المسار/العصر/التسلسل، ترقيم الصفحات) | `DaCommitmentPage` عبارة عن رقم إجمالي وعدد الالتزامات وتجزئة الكتلة تتضمن ہے. |
| `/v1/da/commitments/prove` | `POST` | `DaCommitmentProofRequest` (الممر + تجزئة البيان أو `(epoch, sequence)` tuple)۔ | `DaCommitmentProof` واپس کرتا ہے (سجل + مسار Merkle + تجزئة الكتلة)۔ |
| `/v1/da/commitments/verify` | `POST` | `DaCommitmentProof` | مساعد عديم الجنسية جو كتلة حساب التجزئة تكرار والتحقق من صحة التضمين؛ لا يوجد رابط لمجموعات تطوير البرمجيات (SDK) `iroha_crypto`. |

تمام الحمولات `iroha_data_model::da::commitment` تحت ہیں. أجهزة التوجيه Torii
توفر المعالجات لاستيعاب نقاط النهاية وتركيب البطاقة بشكل ثابت والرمز المميز/mTLS
إعادة استخدام السياسات

## 4. إثباتات الشمول والعملاء الخفيفين

- بلاک منتج تسلسلي قائمة `DaCommitmentRecord` پر شجرة ميركل الثنائية بناتا
  ہے۔ إنه جذر `da_commitments_hash` وهو يغذي الكرت.
- `DaCommitmentProof` سجل الهدف کے ساتھ `(sibling_hash, position)` کا ناقل
  قم بإعادة تحديد الجذر باستخدام أدوات التحقق من الجذر. البراهين میں كتلة التجزئة و
  يتضمن الرأس الموقع أيضًا ما يضمن التحقق من نهائية العملاء الخفيفين.
- مساعدو CLI (`iroha_cli app da prove-commitment`) إثبات الطلب/دورة التحقق کو
  قم بلف البطاقة والمشغلين رقم Norito/hex مخرجات.

## 5. التخزين والفهرسة

التزامات WSV عبارة عن مجموعة أعمدة مخصصة تحتوي على مفتاح `manifest_hash`
مخزن كرتا ہے۔ الفهارس الثانوية `(lane_id, epoch)` و `(lane_id, sequence)`
نحن لا نغطي الاستعلامات المتعلقة بفحص الحزم بتغطية البطاقة. ہر سجل كتلة
ارتفاع کو المسار کرتا ہے جس نے اسي ختم کیا، جس سے عقد اللحاق بلوك سجل سے
يتم إعادة بناء الفهرس بسهولة.

## 6. القياس عن بعد وإمكانية الملاحظة

- `torii_da_commitments_total` زيادة الـTip ہوتا و جب كويت بلاك كم من كم إيك
  ختم السجل کرے۔
- حزمة `torii_da_commitment_queue_depth` تنتظر الإيصالات
  المسار کرتا ہے (لكل حارة)۔
- تضمين كتلة لوحة المعلومات Grafana `dashboards/grafana/da_commitments.json`،
  عمق قائمة الانتظار وإنتاجية الإثبات وبوابات تحرير DA-3
  تدقيق السلوك .

## 7. استراتيجية الاختبار

1. `DaCommitmentBundle` ترميز/فك التشفير وتحديثات اشتقاق تجزئة الكتلة
   **اختبارات الوحدة**۔
2. `fixtures/da/commitments/` **التركيبات الذهبية** هي بايتات الحزمة الأساسية
   وتلتقط براهين ميركل الكرتے ہيں.
3. **اختبارات التكامل** يتم تشغيل أداة التحقق من الصحة، ويتم استيعاب عينات النقط
   التحقق من البطاقة ومحتويات الحزمة واستجابات الاستعلام/الإثبات
   ہیں۔
4. **اختبارات العميل الخفيف** `integration_tests/tests/da/commitments.rs` (الصدأ)
   لدي `/prove` اتصل بـ Torii وهو دليل على التحقق من البطاقة.
5. **CLI smoke** البرنامج النصي `scripts/da/check_commitments.sh` أدوات المشغل
   رہے۔

## 8. خطة الطرح

| المرحلة | الوصف | معايير الخروج |
|-------|------------|---------------|
| P0 — دمج نموذج البيانات | `DaCommitmentRecord`، تحديثات رأس الكتلة وبرامج الترميز Norito. | `cargo test -p iroha_data_model` تركيبات جديدة باللون الأخضر. |
| P1 — الأسلاك الأساسية/WSV | قائمة الانتظار + مؤشر ترابط منطق منشئ الكتلة، وتستمر الفهارس في القراءة، وتكشف معالجات RPC عن البيانات. | `cargo test -p iroha_core`، `integration_tests/tests/da/commitments.rs` تؤكد تأكيدات إثبات الحزمة. |
| P2 — أدوات المشغل | مساعدو CLI، لوحة معلومات Grafana، ومستندات التحقق من الإثبات يتم شحنها. | `iroha_cli app da prove-commitment` devnet پر چلتا ہو؛ البيانات الحية للوحة البيانات دکھائے۔ |
| ج3 — بوابة الحوكمة | `iroha_config::nexus` الممرات التي تم وضع علامة عليها تتطلب التزامات DA تمكين أداة التحقق من صحة الحظر والحظر. | إدخال الحالة + تحديث خريطة الطريق DA-3 لوضع العلامة الكاملة کریں۔ |

## أسئلة مفتوحة

1. **KZG vs Merkle defaults** — ما هي النقط الصغيرة التي تلتزم بها التزامات KZG
   ما هو حجم الكتلة التي تريدها؟ البحث: `kzg_commitment` خيار اختياري
   و `iroha_config::da.enable_kzg` بوابة كريت.
2. **فجوات التسلسل** — ما هي الممرات الخارجة عن الترتيب؟ موجود مخطط الفجوات
   رفض الكرتا والحكم `allow_sequence_skips` وإعادة تشغيل الطوارئ
   تمكين نہ کرے۔
3. **Light-client Cache** — SDK لا تحتوي على أي إثباتات لذاكرة التخزين المؤقت SQLite
   ہے؛ DA-8 متابعة مستمرة.

إن سوالات جوابات تنفيذ العلاقات العامة معززة بـ DA-3 نكل كر
"قيد التقدم" ستبدأ الحالة في بدء عمل الكود في وقت لاحق.