---
lang: ar
direction: rtl
source: docs/portal/docs/da/commitments-plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note Канонический источник
يتم إرسال هذا الشريط إلى `docs/source/da/commitments_plan.md`. آخر الإصدارات
:::

# التزامات الخطة توفر البيانات Sora Nexus (DA-3)

_التاريخ: 25-03-2026 — الأعضاء: فريق عمل البروتوكول الأساسي / فريق العقد الذكي / فريق التخزين_

DA-3 كتلة التنسيق الموسع Nexus لذلك يتم إنشاء كل حارة
تحديد البيانات، تسجيل النقط، DA-2. في هذه الوثيقة
هياكل البيانات الرسمية المؤمنة, التي تكون غير رسمية,
توصية العملاء العاديين والأعلى مستوى Torii/RPC، التي تؤثر على النفقات
علاوة على ذلك، كيف يمكن للمدققين الاستفادة من التزامات DA قبل القبول أو
إدارة التحقق. جميع الحمولة Norito-الترميز; باستثناء SCALE وJSON المخصص.

## كيلي

- التزامات بسيطة على النقطة (جذر القطعة + تجزئة البيان + KZG الاختياري)
  الالتزام) داخل كتلة Nexus، بحيث يمكن إعادة بناءها
  توافر الظروف دون الخضوع للتخزين خارج دفتر الأستاذ.
- التعرف على إثباتات العضوية المحددة حتى يتمكن العملاء الخفيفون من التحقق،
  ما هو بيان التجزئة النهائي في الكتلة المحددة.
- تصدير Torii للقضايا (`/v1/da/commitments/*`) والأثباتات الداعمة
  تعمل المرحلات وحزم SDK وحوكمة الأتمتة على التحقق من التوفر دون إعادة نشر أي شيء
  كتل.
- حفظ المغلف الكنسي `SignedBlockWire`، الهياكل الجديدة المنشورة
  من خلال Norito رأس البيانات التعريفية وتجزئة كتلة الاشتقاق.

## منطقة العمل

1. **نموذج إضافة البيانات** في `iroha_data_model::da::commitment` بالإضافة إلى التحسين
   رأس الكتلة в `iroha_data_model::block`.
2. **خطافات المنفذ** من أجل `iroha_core` إيصالات استيعاب أو DA، قابلة للاستبدال
   Torii (`crates/iroha_core/src/queue.rs` و`crates/iroha_core/src/block.rs`).
3. **الثبات/الفهارس** التي تتجاهل WSV استعلامات الالتزام
   (`iroha_core/src/wsv/mod.rs`).
4. **Torii إضافات RPC** للقائمة/الاستعلام/إثبات نقاط النهاية
   `/v1/da/commitments`.
5. ** اختبارات التكامل + التركيبات ** للتحقق من تخطيط الأسلاك وإثبات التدفق
   `integration_tests/tests/da/commitments.rs`.

## 1. نموذج بيانات التكملة

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

- `KzgCommitment` ينقل مسافة 48 بايت من `iroha_crypto::kzg`.
  عند الإجابة على هذا السؤال، يتم استخدام إثباتات Merkle فقط.
- `proof_scheme` من ممرات الكتالوج؛ تقوم ممرات ميركل بإلغاء حجب حمولات KZG،
  تتطلب الممرات `kzg_bls12_381` التزامات KZG الجديدة. Torii جيد
  ينفذ نفس التزامات Merkle ويفتح الممرات بتكوين KZG.
- `KzgCommitment` ينقل مسافة 48 بايت من `iroha_crypto::kzg`.
  عند اكتشاف ممرات Merkle، يتم استخدام إثباتات Merkle فقط.
- `proof_digest` يدعم تكامل DA-5 PDP/PoTR، لبدء التوصيل
  فحص العينات، يستخدم لعرض النقط.

### 1.2 رأس الكتلة المتناثرة

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

تظهر هذه الحزمة في كتلة التجزئة وفي البيانات الوصفية `SignedBlockWire`. وقت
накладныи расchoдов.

ملاحظة التنفيذ: `BlockPayload` و`BlockBuilder` التالي
المستوطنون/الحروف `da_commitments` (см. `BlockBuilder::set_da_commitments` и
`SignedBlock::set_da_commitments`)، بحيث يمكن للمضيفين أن يعترضوا
حزمة مسبقة الصنع للكتلة المختومة. جميع المنشئات المساعدة
لا يؤدي إلغاء القطب `None` عبر Torii إلى إعادة الحزم الحقيقية.

### 1.3 تشفير الأسلاك

- `SignedBlockWire::canonical_wire()` يتم إضافة رأس Norito إلى
  `DaCommitmentBundle` بعد إجراء معاملة السجل. نسخة البايت `0x01`.
- `SignedBlockWire::decode_wire()` إلغاء الحزم باستخدام `version` غير المعروف،
  في اتصال مع Norito سياسي من `norito.md`.
- اشتقاق التجزئة يتم اشتقاقه تمامًا في `block::Hasher`; العملاء الخفيفين
  فك تشفير تنسيق الأسلاك الخاصة، الحصول على القطب الجديد تلقائيا، أسفل
  أن رأس Norito يشير إليه.

## 2. قم بإغلاق الكتل

1.Torii DA استيعاب `DaIngestReceipt` بشكل نهائي ونشره على الإنترنت
   مرة أخرى (`iroha_core::gossiper::QueueMessage::DaReceipt`).
2. `PendingBlocks` قم بالاتصال بجميع الإيصالات مع `lane_id` للحظر
   تم الإنشاء والنسخ بواسطة `(lane_id, client_blob_id, manifest_hash)`.
3. قبل إغلاق المنشئ، قم بفرز الالتزامات على `(lane_id، Epoch،
   تسلسل)` لتحديد التجزئة وحزمة الترميز Norito وبرنامج الترميز
   قم بالتحقق من `da_commitments_hash`.
4. يتم تسجيل الحزمة الكاملة في WSV ويتم إصدارها في كتلة
   `SignedBlockWire`.

إذا تم إنشاء الكتلة، فإن الإيصالات تبقى في مراقبة لما يلي
تبرعات. قام المنشئ بكتابة `sequence` التالي بما في ذلك في كل حارة,
لتقترح هجمات الإعادة.

## 3. RPC وسطح الاستعلام

يوفر Torii ثلاث نقاط نهاية:

| الطريق | الطريقة | الحمولة | ملاحظات |
|-------|--------|---------|-------|
| `/v1/da/commitments` | `POST` | `DaCommitmentQuery` (نطاق النطاق على المسار/العصر/التسلسل، ترقيم الصفحات) | قم بتأكيد `DaCommitmentPage` مع العدد الإجمالي والالتزامات وكتلة التجزئة. |
| `/v1/da/commitments/prove` | `POST` | `DaCommitmentProofRequest` (الممر + تجزئة البيان أو البطاقة `(epoch, sequence)`). | Отвечает `DaCommitmentProof` (سجل + مسار Merkle + كتلة التجزئة). |
| `/v1/da/commitments/verify` | `POST` | `DaCommitmentProof` | مساعد عديم الجنسية، كتلة تجزئة مكررة، وإدراج محقق؛ مفيد لمجموعات SDK دون الوصول المباشر إلى `iroha_crypto`. |

جميع الحمولات تصل إلى `iroha_data_model::da::commitment`. أجهزة التوجيه Torii
تعمل معالجات الرصد بشكل جيد مع أجهزة DA لاستيعاب نقاط النهاية لاستخدامها
الرمز السياسي/mTLS.

## 4. إثباتات الشمول والعملاء الخفيفين

- منتج الكتلة ينشئ برنامجًا تجاريًا من Merkle من خلال قائمة تسلسلية
  `DaCommitmentRecord`. كورنثود `da_commitments_hash`.
- `DaCommitmentProof` يقوم بتعبئة السجل الخلوي والمتجه `(sibling_hash,
  موقف)` لكي يتمكن المدققون من الوصول إلى العملة. يتضمن الدليل التجزئة
  الكتلة والرأس الموضح حتى يتمكن العملاء الخفيفون من التحقق من النهاية.
- مساعدو CLI (`iroha_cli app da prove-commitment`) يجيبون على طلب إثبات السلسلة/
  تحقق وأظهر Norito/hex voyvod للمشغلين.

## 5. التخزين والفهرسة

التزامات WSV في عائلة الأعمدة القديمة بمفتاح `manifest_hash`.
تظهر المؤشرات الأولية `(lane_id, epoch)` و`(lane_id, sequence)`، لذلك
لا تقوم بفحص الحزم الكاملة. كل ما عليك فعله هو تسجيل كتلة الصوت، في
ما الذي كان مخيبا للآمال مما يسمح باللحاق بالركب على الفور
فهرس من سجل الكتلة.

## 6. القياس عن بعد وإمكانية الملاحظة

- يتم تعزيز `torii_da_commitments_total` عندما يتم إغلاق الكتلة على الأقل
  سجل واحد.
- `torii_da_commitment_queue_depth` مراقبة الإيصالات وتجميع الأعمال
  (على المسار).
- لوحة القيادة Grafana `dashboards/grafana/da_commitments.json` visualiziruitet
  التضمين في الكتلة والمراقبة المتلألئة وإنتاجية الإثبات لتدقيق إصدار DA-3
  بوابة.

## 7. اختبار الإستراتيجية

1. **اختبارات الوحدة** للتشفير/فك التشفير `DaCommitmentBundle` والتحديث
   كتلة اشتقاق التجزئة.
2. **التركيبات الذهبية** في `fixtures/da/commitments/` مع حزمة البايتات القياسية
   وبراهين ميركل.
3. **اختبارات التكامل** مع المدققين واستيعاب العينات النقطية والاختبار
   حزمة المساعدة والإجابات الاستعلام/إثبات.
4. **اختبارات العميل الخفيف** в `integration_tests/tests/da/commitments.rs` (الصدأ)،
   تم التحقق من `/prove` والتحقق من الإثبات بدون تشويش مع Torii.
5. **CLI smoke** النص البرمجي `scripts/da/check_commitments.sh` للتنشيط
   أدوات المشغل.

## 8. طرح الخطة

| المرحلة | الوصف | معايير الخروج |
|-------|------------|---------------|
| P0 - دمج نموذج البيانات | قم بطباعة `DaCommitmentRecord` ورأس الكتلة المتجدد وبرامج الترميز Norito. | `cargo test -p iroha_data_model` نظيف مع تركيبات جديدة. |
| P1 - الأسلاك الأساسية/WSV | قم بتغيير قائمة الانتظار + منطق منشئ الكتل، وحافظ على الفهارس واكتشف معالجات RPC. | `cargo test -p iroha_core`، `integration_tests/tests/da/commitments.rs` يتم التحقق من صحتها. |
| P2 - أدوات المشغل | قم بنشر مساعدي CLI ولوحة المعلومات Grafana ومستندات الموافقة من أجل التحقق من الإثبات. | `iroha_cli app da prove-commitment` يعمل على devnet; تعرض لوحة القيادة البيانات المباشرة. |
| ج3 - بوابة الحوكمة | قم بتضمين أداة التحقق من صحة الكتلة، والتزامات DA المطلوبة للممرات، والمحددة في `iroha_config::nexus`. | إدخال الحالة + تحديث خريطة الطريق يساعد على DA-3 كآخر. |

## الأسئلة المفتوحة

1. **KZG vs Merkle defaults** — من الضروري نشر النقط الصغيرة دائمًا
   التزامات KZG، لتصغير حجم الكتلة؟ اقتراح: توقف
   `kzg_commitment` اختياري وبوابة عبر `iroha_config::da.enable_kzg`.
2. **فجوات التسلسل** — هل تبحث عن أحدث النتائج؟ خطة تيكيوسي
   سد الفجوات، إذا لم تتضمن الإدارة `allow_sequence_skips`
   إعادة تشغيل خارجية.
3. ** ذاكرة التخزين المؤقت للعميل الخفيف ** — يتم التحكم في ذاكرة التخزين المؤقت لـ SDK من خلال ذاكرة التخزين المؤقت SQLite للإثباتات؛
   العمل الأخير تحت DA-8.

الإجابات على هذه الأسئلة في تنفيذ العلاقات العامة تحوّل DA-3 إلى حالة "مسودة"
(هذه الوثيقة) في "قيد التقدم" بعد بدء العمل الكودي.