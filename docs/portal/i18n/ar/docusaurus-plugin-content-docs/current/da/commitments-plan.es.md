---
lang: ar
direction: rtl
source: docs/portal/docs/da/commitments-plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::ملاحظة فوينتي كانونيكا
ريفليجا `docs/source/da/commitments_plan.md`. Mantenga ambas الإصدارات en
:::

# خطة اختراق توفر البيانات لـ Sora Nexus (DA-3)

_تم التنقيح: 25-03-2026 -- المسؤولون: مجموعة عمل البروتوكول الأساسي / فريق العقد الذكي / فريق التخزين_

يقوم DA-3 بتوسيع تنسيق الكتلة Nexus بحيث يتم إضافة كل المسار إلى السجلات
المحددات التي تصف النقط المقبولة لـ DA-2. هذه ملاحظة ملتقطة
هياكل البيانات الأساسية، وخطافات خط أنابيب الكتل، واختبارات
العميل الخفيف والأسطح Torii/RPC الذي يجب عليك تنظيفه قبل ذلك
يمكن للمصادقين أن يثقوا في التنازلات DA أثناء القبول أو الشيكات
com.gobernanza. جميع الحمولات مشفرة على Norito؛ إعلان sin SCALE وJSON
مخصص.

##الأهداف

- تسوية التسوية من خلال النقطة (جذر قطعة + تجزئة واضحة + التزام KZG
  اختياري) داخل كل كتلة Nexus حتى يتمكن أقرانهم من إعادة بناء
  حالة التوفر بدون استشارة تخزين دفتر الأستاذ.
- إثبات دقة الذاكرة المحددة حتى يتمكن العملاء من التحقق منها
  قد يتم الانتهاء من عملية تجزئة البيان في كتلة واحدة.
- يستشير العارض Torii (`/v2/da/commitments/*`) ويختبر ما يسمح به
  تراقب المرحلات ومجموعات SDK وأتمتة الإدارة مدى التوفر دون إعادة إنتاجها
  كتلة كادا.
- صيانة المغلف `SignedBlockWire` canonico al enhebrar las nuevas
  إنشاءات من خلال رأس البيانات التعريفية Norito واشتقاق التجزئة
  كتلة.

## بانوراما دي ألكانس

1. **إضافة إلى نموذج البيانات** في `iroha_data_model::da::commitment` mas
   تغيير رأس الكتلة في `iroha_data_model::block`.
2. **خطافات المنفذ** حتى يتمكن `iroha_core` من استيعاب الإيصالات الصادرة من قبل
   Torii (`crates/iroha_core/src/queue.rs` و`crates/iroha_core/src/block.rs`).
3. **الاستمرارية/الفهارس** لكي يستجيب WSV لاستشارات التسوية
   رابيدو (`iroha_core/src/wsv/mod.rs`).
4. **إضافات RPC في Torii** لنقاط نهاية القائمة/الاستشارة/اختبار الخلفية
   `/v2/da/commitments`.
5. **اختبارات التكامل + التركيبات** التحقق من تخطيط السلك وتدفقه
   إثبات en `integration_tests/tests/da/commitments.rs`.

## 1. إضافة نموذج البيانات

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

- `KzgCommitment` يعيد استخدام النقطة 48 بايت المستخدمة في `iroha_crypto::kzg`.
  عندما يكون الأمر كذلك، قم برؤية إثباتات Merkle بهدوء.
- `proof_scheme` مشتق من كتالوج الممرات؛ لاس لاينز ميركل ريشازان
  الحمولات النافعة KZG تتطلب التزامات KZG
  لا سيرو. Torii يقوم حاليًا بإنتاج تسويات منفردة لخطوط Merkle و Rechaza
  التكوينات مع KZG.
- `KzgCommitment` يعيد استخدام النقطة 48 بايت المستخدمة في `iroha_crypto::kzg`.
  عندما تنظر إلى ممرات ميركل مباشرة، فإنها تنظر إلى إثباتات ميركل بهدوء.
- `proof_digest` يتوقع التكامل DA-5 PDP/PoTR لتسجيل نفس الشيء
  تعداد الجدول الزمني لأخذ العينات المستخدمة للحفاظ على النقط الحية.

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

تجزئة الحزمة في نفس الوقت ضمن تجزئة الكتلة مثل البيانات الوصفية
`SignedBlockWire`. عند إنشاء كتلة لا تفتح البيانات في المجال الدائم `None`

ملاحظة التنفيذ: `BlockPayload` والشفاف `BlockBuilder` الآن
أدوات ضبط/حروف الأس `da_commitments` (الإصدار `BlockBuilder::set_da_commitments`
y `SignedBlock::set_da_commitments`)، حيث يمكن للمضيفين إضافة حزمة
تم إنشاؤها مسبقًا قبل بيع كتلة. جميع المنشئين يساعدون ديجان إل
Campo en `None` hasta que Torii يضم الحزم الحقيقية.

### 1.3 ترميز الأسلاك

- `SignedBlockWire::canonical_wire()` إضافة الرأس Norito للفقرة
  `DaCommitmentBundle` فورًا بعد قائمة المعاملات
  موجود. بايت الإصدار هو `0x01`.
- `SignedBlockWire::decode_wire()` حزم rechaza cuyo `version` غير معروفة،
  اتبع السياسة Norito الموصوفة في `norito.md`.
- تحديثات اشتقاق التجزئة تظهر فقط في `block::Hasher`؛ لوس
  العملاء السهلون الذين يقومون بفك تشفير تنسيق السلك الموجود في المجال الجديد
  يتم ذلك تلقائيًا بسبب ظهور الرأس Norito.

## 2. تدفق إنتاج الكتل

1. قم بإدراج DA de Torii وانتهى من `DaIngestReceipt` ونشره في الكولا
   الداخلية (`iroha_core::gossiper::QueueMessage::DaReceipt`).
2. `PendingBlocks` يعيد تجميع جميع الإيصالات `lane_id` يتزامن مع el
   كتلة في الإنشاء، وإزالة البيانات المكررة من خلال `(lane_id،client_blob_id،
   البيان_التجزئة)`.
3. قبل البيع، يقوم المُنشئ بترتيب التنازلات من خلال `(lane_id,
   عصر، تسلسل)` للحفاظ على تحديد التجزئة، وتدوين الحزمة مع
   برنامج الترميز Norito، وقم بتحديث `da_commitments_hash`.
4. يتم تخزين الحزمة الكاملة في WSV ويتم إرسالها جنبًا إلى جنب مع الكتلة الموجودة في
   `SignedBlockWire`.

إذا فشل إنشاء الكتلة، فإن الإيصالات تظل ثابتة في الكولا حتى يتمكن
siguiente نية to los tome؛ سجل المنشئ الأخير `sequence` متضمن
من أجل تجنب هجمات الإعادة.

## 3. Superficie RPC والاستشارة

يعرض Torii ثلاث نقاط نهاية:

| روتا | الطريقة | الحمولة | نوتاس |
|------|--------|---------|-------|
| `/v2/da/commitments` | `POST` | `DaCommitmentQuery` (مرشح حسب نطاق المسار/العصر/التسلسل، الصفحة) | Devuelve `DaCommitmentPage` مع إجمالي وتسويات وتجزئة الكتلة. |
| `/v2/da/commitments/prove` | `POST` | `DaCommitmentProofRequest` (الممر + تجزئة البيان أو tupla `(epoch, sequence)`). | الرد على `DaCommitmentProof` (سجل + ruta Merkle + تجزئة الكتلة). |
| `/v2/da/commitments/verify` | `POST` | `DaCommitmentProof` | مساعد عديم الجنسية لإعادة حساب تجزئة الكتلة والتحقق من صحة التضمين؛ يتم استخدامه من خلال مجموعات SDK التي لا يمكن توجيهها مباشرة إلى `iroha_crypto`. |

ستعيش جميع الحمولات الصافية `iroha_data_model::da::commitment`. أجهزة التوجيه دي
Torii يتم تجميع المعالجات جنبًا إلى جنب مع نقاط نهاية استيعاب DA الموجودة
إعادة استخدام السياسة الرمزية/mTLS.

## 4. تجربة الإدماج والعملاء الخفيفين

- يقوم منتج الكتل ببناء شجرة Merkle الثنائية فوق القائمة
  تم تسلسل `DaCommitmentRecord`. الطعام الرايز `da_commitments_hash`.
- `DaCommitmentProof` يقوم بتعبئة سجل الهدف باعتباره ناقلًا
  `(sibling_hash, position)` حتى يتمكن المدققون من إعادة بناء الأساس. لاس
  تتضمن الاختبارات أيضًا تجزئة الكتلة والرأس الثابت لذلك
  عملاء ligeros التحقق من النهاية.
- مساعدو CLI (`iroha_cli app da prove-commitment`) يحيطون بالحلقة
  التماس/التحقق من الاختبار وإظهار النتائج Norito/hex لـ
  مشغلي.

## 5. فهرسة التخزين

WSV Almacena يتنازل عن عمود عائلي مخصص بمفتاح
`manifest_hash`. الفهارس الثانية المكعبة `(lane_id, epoch)` y
`(lane_id, sequence)` حتى تتمكن الاستشارات من مسح الحزم الكاملة.
كل ما سجله هو ارتفاع الكتلة التي ستبيعها، قم بالسماح لها بالعقد
اللحاق بالركب إعادة بناء المؤشر بسرعة من سجل الكتلة.

## 6. القياس عن بعد وإمكانية المراقبة

- `torii_da_commitments_total` يتم زيادته عندما يكون هناك كتلة صغيرة جدًا
  سجل.
- `torii_da_commitment_queue_depth` إيصالات راستريا من المقرر أن يتم تغليفها
  (بور لين).
- لوحة القيادة Grafana `dashboards/grafana/da_commitments.json` تظهر
  تضمين الكتل وعمق الكولا وإنتاجية الاختبار لذلك
  يمكن لبوابات إصدار DA-3 مراجعة الأداء.

## 7. استراتيجية الاختبار

1. **الاختبارات الوحدوية** للتشفير/فك التشفير `DaCommitmentBundle` y
   تحديثات اشتقاق تجزئة الكتلة.
2. **التركيبات الذهبية** bajo `fixtures/da/commitments/` التي تلتقط البايتات
   Canonicos del Bundle و Merkle.
3. **اختبارات التكامل** تقتضي من المصادقين إدخال النقط
   عرض والتحقق من أن جميع العقد متفق عليها في محتوى الحزمة
   las respuestas de Consulta/prueba.
4. **اختبارات العميل الخفيفة** في `integration_tests/tests/da/commitments.rs`
   (Rust) اتصل بـ `/prove` وتحقق من الاختبار بدون التحدث مع Torii.
5. **Smoke de CLI** مع `scripts/da/check_commitments.sh` لأدوات الصيانة
   مشغلي قابلة للتكرار.

## 8. خطة الطرح

| فاس | الوصف | معيار الخروج |
|------|----------------------------|----|
| P0 - دمج نموذج البيانات | دمج `DaCommitmentRecord`، وتحديث رأس الكتلة وبرامج الترميز Norito. | `cargo test -p iroha_data_model` أخضر مع تركيبات جديدة. |
| P1 - كابلادو كور/WSV | تعلم منطق الكولا + منشئ الكتل والفهارس المستمرة ومعالجات RPC. | `cargo test -p iroha_core`، `integration_tests/tests/da/commitments.rs` يعتمد على تأكيدات إثبات الحزمة. |
| P2 - أدوات التشغيل | يساعد Lanzar في CLI ولوحة المعلومات Grafana وتحديث مستندات التحقق من الإثبات. | `iroha_cli app da prove-commitment` وظيفة مكافحة devnet; لوحة القيادة تعرض البيانات في الجسم الحي. |
| P3 - بوابة غوبيرنانزا | قم بتأهيل مدقق الكتل التي تتطلب اختراق DA في علامات الممرات في `iroha_config::nexus`. | تم إدخال الحالة + تحديث خريطة الطريق لعلامة DA-3 كـ COMPLETADO. |

## أسئلة مفتوحة1. **KZG vs Merkle defaults** - يجب علينا حذف تسويات KZG والنقط الصغيرة
   لتقليل حجم الكتلة؟ العرض: الصيانة `kzg_commitment`
   اختياري وبوابة عبر `iroha_config::da.enable_kzg`.
2. **فجوات التسلسل** - هل تسمح للممرات بالنظام؟ El Plan Rechaza الفعلي
   الثغرات التي تطلقها gobernanza active `allow_sequence_skips` لإعادة التشغيل
   الطوارئ.
3. **Light-client Cache** - مجموعة SDK تعمل على إنشاء ذاكرة تخزين مؤقت SQLite لـ
   البراهين. يتبع بعد DA-8.

المستجيب هذه الأسئلة في شروط التنفيذ Mueve DA-3 de BORRADOR (este
documento) a EN PROGRESO cuando el trabajo de codigo comence.