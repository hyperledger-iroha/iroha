---
lang: ar
direction: rtl
source: docs/portal/docs/da/commitments-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::ملاحظة فونتي كانونيكا
إسبيلها `docs/source/da/commitments_plan.md`. Mantenha كما أدعية الآيات م
:::

# خطة خلل توفر البيانات في Sora Nexus (DA-3)

_التاريخ: 25/03/2026 -- الردود: مجموعة عمل البروتوكول الأساسي / فريق العقد الذكي / فريق التخزين_

DA-3 هو تنسيق الكتلة Nexus بحيث يتضمن كل مسار السجلات
الحتمية التي تكتشف النقطات ذات الصلة بـ DA-2. هذه هي الصورة الملتقطة
نماذج البيانات الأساسية، وخطافات خط أنابيب الكتل، واختبارها
يريد العملاء السطوح Torii/RPC التي تتطلب المحاولة قبل ذلك
المصادقون possam يثقون في تنازلاتنا DA أثناء القبول أو الشيكات
com.goveranca. جميع الحمولات النافعة مرمزة في Norito؛ إعلان SCALE أو JSON
مخصص.

##الأهداف

- Carregar compromissos por blob (جذر قطعة + تجزئة واضحة + التزام KZG
  اختياري) داخل كل كتلة Nexus حتى يتمكن أقرانك من إعادة بناء حالتهم
  توفر منتديات التخزين الاستشارية شبه لدفتر الأستاذ.
- إثبات حتمية العضوية التي يحتاجها العملاء
  تحقق من أن بيان التجزئة قد تم الانتهاء منه في كتلة.
- تصدير الاستشارات Torii (`/v2/da/commitments/*`) والتأكد من السماح بالمرحلات،
  تقوم أدوات تطوير البرامج (SDKs) والحوكمة التلقائية بمراجعة التوفر دون إعادة إنتاج كل كتلة.
- قم بتغيير المغلف `SignedBlockWire` canonico ao enfiar as nova estruturas
  رأس البيانات الوصفية Norito ومشتق تجزئة الكتلة.

## بانوراما دي إسكوبو

1. **الإضافة إلى نموذج البيانات** في `iroha_data_model::da::commitment` بدلاً من ذلك
   رأس الكتلة في `iroha_data_model::block`.
2. ** الخطافات المنفذة ** لكي `iroha_core` تستوعب الإيصالات الصادرة من قبل
   Torii (`crates/iroha_core/src/queue.rs` و`crates/iroha_core/src/block.rs`).
3. **الاستمرارية/الفهارس** لكي يستجيب WSV لاستشارات التسوية
   بسرعة (`iroha_core/src/wsv/mod.rs`).
4. **نصائح RPC في Torii** لنقاط نهاية القائمة/المشورة/التجريب
   `/v2/da/commitments`.
5. **اختبارات التكامل + التركيبات** التحقق من صحة تخطيط الأسلاك وتدفق الإثبات
   م `integration_tests/tests/da/commitments.rs`.

## 1. نموذج بيانات Adicoes ao

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

- يتم إعادة استخدام `KzgCommitment` أو استخدام 48 بايت في `iroha_crypto::kzg`.
  عندما يكون الأمر كذلك، لمعرفة كيفية اختبار ميركل فقط.
- `proof_scheme` مشتق من كتالوج الممرات؛ ممرات Merkle rejeitam حمولات KZG
  enquanto ممرات `kzg_bls12_381` exigem التزامات KZG nao صفر. Torii في الوقت الحالي
  لذلك يتم إنتاج تسويات Merkle وإعادة إنشاء الممرات التي تم تكوينها بواسطة KZG.
- `KzgCommitment` يتم إعادة استخدامه أو استخدام 48 بايت في `iroha_crypto::kzg`.
  عندما يقترب من ممرات Merkle، لمعرفة كيفية اختبار Merkle فقط.
- `proof_digest` يتوقع دمج DA-5 PDP/PoTR لتسجيل نفس الشيء
  تعداد أو جدول أخذ العينات المستخدم للحفاظ على النقط الحية.

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

قم بعمل حزمة تجزئة لهذه المدة دون تجزئة الكتلة بقدر البيانات الوصفية
`SignedBlockWire`. عندما تكون الكتلة لا تحمل بيانات DA، أو المجال العملي `None` para

مذكرة التنفيذ: `BlockPayload` e o `BlockBuilder` شفافة الآن
أدوات ضبط/حروف العرض `da_commitments` (الإصدار `BlockBuilder::set_da_commitments`
e `SignedBlock::set_da_commitments`)، ويستضيف إمكانية إضافة حزمة
تم الإنشاء مسبقًا قبل إنشاء كتلة. جميع المساعدين موجودون في `None`
ate كيو Torii encadeie حزم ريال.

### 1.3 ترميز الأسلاك

- `SignedBlockWire::canonical_wire()` إضافة رأس Norito للفقرة
  `DaCommitmentBundle` مباشرة بعد قائمة المعاملات الموجودة. يا
  البايت العكسي و`0x01`.
- `SignedBlockWire::decode_wire()` حزم rejeita cujo `version` seja
  تم وصفه من خلال السياسة Norito في `norito.md`.
- تحديث مشتقات التجزئة الموجودة فقط في `block::Hasher`؛ عملاء
  المستويات التي يتم فيها فك تشفير تنسيق السلك الموجود في مكان آخر أو مجال جديد
  يتم تلقائيًا الإعلان عن رأس Norito الخاص به.

## 2. تدفق إنتاج الكتل

1. استيعاب DA Torii نهائيًا لـ `DaIngestReceipt` ونشر الملف
   الداخلية (`iroha_core::gossiper::QueueMessage::DaReceipt`).
2. `PendingBlocks` جميع إيصالات نظام التشغيل cujo `lane_id` تقابل الكتلة
   تم البناء، وإزالة التكرار من خلال `(lane_id, client_blob_id, manifest_hash)`.
3. لا يزال بإمكانك البدء، أو إنشاء تسويات من أجل `(lane_id, epoch,
   تسلسل)` للإضافة إلى حتمية التجزئة أو الكوديك أو الحزمة مع برنامج الترميز
   Norito، وتم التحديث `da_commitments_hash`.
4. حزمة كاملة ومخزنة في WSV ويتم إصدارها جنبًا إلى جنب مع الكتلة الموجودة في
   `SignedBlockWire`.

إذا تم إنشاء كتلة من الانهيار، فإن الإيصالات تظل ثابتة في الملف قريبًا

التقاط نظام التشغيل Tentativa؛ o سجل المنشئ أو `sequence` الأخير متضمن في المسار
لتجنب هجمات الإعادة.

## 3. الاستشارات والمشورة السطحية لـ RPC

Torii يعرض ثلاث نقاط نهاية:

| روتا | الطريقة | الحمولة | نوتاس |
|------|--------|---------|-------|
| `/v2/da/commitments` | `POST` | `DaCommitmentQuery` (مرشح المدى حسب المسار/العصر/التسلسل، الصفحة) | Retorna `DaCommitmentPage` مع إجمالي وتسويات وتجزئة الكتلة. |
| `/v2/da/commitments/prove` | `POST` | `DaCommitmentProofRequest` (الممر + التجزئة الواضحة أو tupla `(epoch, sequence)`). | الرد على com `DaCommitmentProof` (سجل + caminho Merkle + hash de bloco). |
| `/v2/da/commitments/verify` | `POST` | `DaCommitmentProof` | مساعد عديم الجنسية الذي يعيد حساب تجزئة الكتلة والتحقق من صحتها ؛ يتم استخدامه من خلال مجموعات SDK التي لا يمكنها الارتباط مباشرة بـ `iroha_crypto`. |

جميع حمولات نظام التشغيل vivem sob `iroha_data_model::da::commitment`. أجهزة التوجيه OS دي
Torii يقوم بمعالجة نظام التشغيل من خلال نقاط النهاية لاستيعاب DA الموجود
إعادة استخدام السياسة الرمزية/mTLS.

## 4. عروض شاملة للعملاء

- منتج الكتل الذي قام ببناء ملف Merkle Binaria حول القائمة
  تم تسلسل `DaCommitmentRecord`. طعام رايز `da_commitments_hash`.
- `DaCommitmentProof` قم بالتسجيل أو التسجيل أكثر من ذلك
  `(sibling_hash, position)` لكي يقوم المدققون بإعادة بناء رأسهم. كما
  يتضمن الاختبار أيضًا تجزئة الكتلة والرأس الذي تم تثبيته للعملاء
  يفرض صلاحية نهائية.
- مساعدو CLI (`iroha_cli app da prove-commitment`) يشملون الدائرة
  الطلب/التحقق والعرض على Norito/hex للمشغلين.

## 5. تخزين وفهرسة

إن سوق WSV يقدم التنازلات في عائلة عمود مخصصة للأشياء
`manifest_hash`. الفهارس الثانوية cobrem `(lane_id, epoch)` ه
`(lane_id, sequence)` حتى تتمكن من استشارة تجنب الحزم المكتملة. كادا
سجل ارتفاعًا في الكتلة مما يسمح لك باللحاق بالركب
إعادة بناء الفهرس بسرعة من سجل الحظر.

## 6. القياس عن بعد وإمكانية المراقبة

- `torii_da_commitments_total` يتم زيادة عندما يتم إضافة كتلة إلى أقل منها
  سجل.
- `torii_da_commitment_queue_depth` حزمة إيصالات راستريا aguardando (من قبل
  حارة).
- يا لوحة القيادة Grafana `dashboards/grafana/da_commitments.json` تصور أ
  بما في ذلك الكتل وعمق الملف وإنتاجية الاختبار بالنسبة لك
  يمكن لبوابات تحرير DA-3 مراقبة السلوك.

## 7. استراتيجية الخصيتين

1. **الاختبارات الوحدوية** للتشفير/فك التشفير لـ `DaCommitmentBundle` e
   تحديث مشتقات تجزئة الكتلة.
2. **التركيبات الذهبية** sob `fixtures/da/commitments/` capturando bytes canonicos
   قم بعمل الحزمة وبروفاس ميركل.
3. **اختبارات التكامل** مع العديد من المدققين، ودمج النقطتين النموذجيتين
   التحقق من أننا متفقون على عدم محتوى الحزمة والردود
   دي الاستعلام/إثبات.
4. **اختبارات مستوى العميل** في `integration_tests/tests/da/commitments.rs`
   (الصدأ) الذي يشير إلى `/prove` ويتحقق من صحته مع Torii.
5. **Smoke CLI** com `scripts/da/check_commitments.sh` لمزيد من الأدوات
   مشغلي التكاثر.

## 8. خطة الطرح

| فاس | وصف | معيار الصيدة |
|------|---------------------------|--|
| P0 - دمج نموذج البيانات | دمج `DaCommitmentRecord`، وتحديث رأس الكتلة وبرامج الترميز Norito. | `cargo test -p iroha_data_model` تركيبات verde com novas. |
| P1 - الأسلاك الأساسية/WSV | قم بتوسيع منطق الملف + أداة إنشاء الكتل والفهارس المستمرة وتصدير معالجات RPC. | `cargo test -p iroha_core`، `integration_tests/tests/da/commitments.rs` تأكيدات إثبات الحزمة. |
| P2 - أدوات التشغيل | قم بإدخال مساعدي CLI ولوحة المعلومات Grafana وتحديثات مستندات التحقق من الإثبات. | `iroha_cli app da prove-commitment` وظيفة ضد devnet; o لوحة القيادة موسترا دادوس آو فيفو. |
| P3 - بوابة الحكم | تأهيل أو التحقق من الكتل التي تتطلب التنازلات على علامات الممرات في `iroha_config::nexus`. | تم إدخال الحالة + تحديث خريطة الطريق لمارك DA-3 كـ COMPLETADO. |

## Perguntas abertas

1. **KZG vs Merkle defaults** - Devemos semper Pular التزامات KZG em blobs
   pequenos para reduzir o tamanho do bloco؟ الاقتراح: manter `kzg_commitment`
   بوابة اختيارية عبر `iroha_config::da.enable_kzg`.
2. **فجوات التسلسل** - هل تسمح للممرات بالترتيب؟ يا بلانو الثغرات rejeita الفعلية
   يتم إطلاق النار على إدارة تشغيل `allow_sequence_skips` لإعادة تشغيل الطوارئ.
3. ** ذاكرة التخزين المؤقت للعميل الخفيف ** - حان الوقت لتنزيل ذاكرة التخزين المؤقت لـ SDK من SQLite للإثباتات؛
   معلقة على DA-8.

المستجيب هذه الأسئلة في PRs للتنفيذ move DA-3 de RASCUNHO (هذا
documento) para EM ANDAMENTO When do o trabalho de codigo Comecar.