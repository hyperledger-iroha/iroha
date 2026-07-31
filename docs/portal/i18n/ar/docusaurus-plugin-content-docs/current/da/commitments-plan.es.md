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
com.gobernanza. جميع الحمولات مشفرة على Norito؛ إعلان sin retired codec وJSON
مخصص.

##الأهداف

- تسوية التسوية من خلال النقطة (جذر قطعة + تجزئة واضحة + التزام KZG
  اختياري) داخل كل كتلة Nexus حتى يتمكن أقرانهم من إعادة بناء
  حالة التوفر بدون استشارة تخزين دفتر الأستاذ.
- إثبات دقة الذاكرة المحددة حتى يتمكن العملاء من التحقق منها
  قد يتم الانتهاء من عملية تجزئة البيان في كتلة واحدة.
- يستشير العارض Torii (`/v1/da/commitments/*`) ويختبر ما يسمح به
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
   `/v1/da/commitments`.
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
    pub proof_scheme: DaProofScheme,          // V1 lane policy (merkle_sha256 only)
    pub chunk_root: Hash,                     // Merkle root of chunk digests
    pub proof_digest: Option<Hash>,           // hash of PDP/PoTR schedule
    pub retention_class: RetentionClass,      // mirrors DA-2 retention policy
    pub storage_ticket: StorageTicketId,
    pub acknowledgement_sig: Signature,       // Torii DA service key
}
```

- **V1 protocol invariant:** V1 contains only the `merkle_sha256` proof scheme. It has no KZG variant, commitment field, setup, generation, or verification path; unsupported configuration is rejected before node startup.
- A future KZG design requires a separately reviewed protocol version and an explicit wire-layout change. Hash expansion is not an elliptic-curve commitment.
- The verification endpoint checks commitment consistency against a caller-authenticated canonical block header and committed policy sidecar; it does not independently authenticate block signatures or finality.

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
| `/v1/da/commitments` | `POST` | `DaCommitmentQuery` (مرشح حسب نطاق المسار/العصر/التسلسل، الصفحة) | Devuelve `DaCommitmentPage` مع إجمالي وتسويات وتجزئة الكتلة. |
| `/v1/da/commitments/prove` | `POST` | `DaCommitmentProofRequest` (الممر + تجزئة البيان أو tupla `(epoch, sequence)`). | الرد على `DaCommitmentProof` (سجل + ruta Merkle + تجزئة الكتلة). |
| `/v1/da/commitments/verify` | `POST` | `DaCommitmentProof` | مساعد عديم الجنسية لإعادة حساب تجزئة الكتلة والتحقق من صحة التضمين؛ يتم استخدامه من خلال مجموعات SDK التي لا يمكن توجيهها مباشرة إلى `iroha_crypto`. |

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

1. **Future KZG protocol** — KZG is not part of V1. A separately reviewed later protocol version must specify polynomial encoding, setup provenance, commitment/opening algorithms, consensus verification, deterministic test vectors, and a versioned wire-layout change. V1 has no enable toggle or reserved accepted value.
2. **فجوات التسلسل** - هل تسمح للممرات بالنظام؟ El Plan Rechaza الفعلي
   الثغرات التي تطلقها gobernanza active `allow_sequence_skips` لإعادة التشغيل
   الطوارئ.
3. **Light-client Cache** - مجموعة SDK تعمل على إنشاء ذاكرة تخزين مؤقت SQLite لـ
   البراهين. يتبع بعد DA-8.

المستجيب هذه الأسئلة في شروط التنفيذ Mueve DA-3 de BORRADOR (este
documento) a EN PROGRESO cuando el trabajo de codigo comence.