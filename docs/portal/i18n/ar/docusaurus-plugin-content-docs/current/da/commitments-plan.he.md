---
lang: he
direction: rtl
source: docs/portal/i18n/ar/docusaurus-plugin-content-docs/current/da/commitments-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: dbea3310d389cef4e3633c2e898d35375550d7667cc7a086d6b33209770377cf
source_last_modified: "2026-01-22T15:38:30+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: ar
direction: rtl
source: docs/portal/docs/da/commitments-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note المصدر القياسي
يعكس `docs/source/da/commitments_plan.md`. ابق النسختين متزامنتين حتى يتم سحب
الوثائق القديمة.
:::

# خطة تعهدات توفر البيانات في Sora Nexus (DA-3)

_مسودة: 2026-03-25 -- المالكون: Core Protocol WG / Smart Contract Team / Storage Team_

يمدد DA-3 تنسيق كتلة Nexus بحيث تدمج كل lane سجلات حتمية تصف الـ blobs المقبولة
من DA-2. توضح هذه المذكرة هياكل البيانات القياسية، ووصلات خط انابيب الكتل،
وبرهان العملاء الخفيفين، واسطح Torii/RPC التي يجب ان تكتمل قبل ان يعتمد
المدققون على تعهدات DA اثناء فحوصات القبول او الحوكمة. جميع الحمولات مشفرة
بـ Norito؛ لا SCALE ولا JSON مخصص.

## الاهداف

- حمل تعهدات لكل blob (chunk root + manifest hash + تعهد KZG اختياري) داخل كل
  كتلة Nexus لكي يتمكن النظراء من اعادة بناء حالة availability بدون الرجوع
  الى تخزين خارج الدفتر.
- توفير براهين عضوية حتمية لكي يتحقق العملاء الخفيفون من ان manifest hash تم
  تثبيته في كتلة محددة.
- كشف استعلامات Torii (`/v1/da/commitments/*`) وبراهين تسمح للـ relays وSDKs
  وادوات الحوكمة بتدقيق availability دون اعادة تشغيل كل كتلة.
- الحفاظ على ظرف `SignedBlockWire` القياسي عبر تمرير البنى الجديدة من خلال
  ترويسة بيانات Norito الوصفية واشتقاق hash الكتلة.

## نظرة عامة على النطاق

1. **اضافات نموذج البيانات** في `iroha_data_model::da::commitment` مع تغييرات
   ترويسة الكتلة في `iroha_data_model::block`.
2. **Hooks للمنفذ** حتى يقوم `iroha_core` بابتلاع receipts الخاصة بـ DA
   الصادرة من Torii (`crates/iroha_core/src/queue.rs` و
   `crates/iroha_core/src/block.rs`).
3. **Persisting/indexes** حتى يتمكن WSV من اجابة استعلامات التعهدات بسرعة
   (`iroha_core/src/wsv/mod.rs`).
4. **اضافات RPC في Torii** لنقاط list/query/prove تحت `/v1/da/commitments`.
5. **اختبارات تكامل + fixtures** للتحقق من wire layout وتدفق proof في
   `integration_tests/tests/da/commitments.rs`.

## 1. اضافات نموذج البيانات

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

- `KzgCommitment` يعيد استخدام النقطة ذات 48 بايت الموجودة في
  `iroha_crypto::kzg`. عند غيابها نعود الى براهين Merkle فقط.
- `proof_scheme` مشتق من كتالوج lanes؛ lanes من نوع Merkle ترفض حمولات KZG،
  بينما lanes `kzg_bls12_381` تتطلب تعهدات KZG غير صفرية. Torii حاليا ينتج
  تعهدات Merkle فقط ويرفض lanes المهيئة على KZG.
- `KzgCommitment` يعيد استخدام النقطة ذات 48 بايت الموجودة في
  `iroha_crypto::kzg`. عند غيابها في lanes Merkle نعود الى براهين Merkle فقط.
- `proof_digest` يمهد لتكامل DA-5 PDP/PoTR لكي يسرد السجل نفسه جدول اخذ
  العينات المستخدم للحفاظ على blobs.

### 1.2 توسيع ترويسة الكتلة

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

يدخل hash الخاص بالحزمة في hash الكتلة وفي بيانات `SignedBlockWire`. عندما لا

ملاحظة تنفيذية: `BlockPayload` و`BlockBuilder` الشفاف يعرضان الان
setters/getters لـ `da_commitments` (راجع `BlockBuilder::set_da_commitments`
و`SignedBlock::set_da_commitments`) لكي تتمكن hosts من ارفاق حزمة مسبقة
الانشاء قبل ختم الكتلة. جميع الدوال المساعدة تترك الحقل `None` حتى يمر Torii
حزم حقيقية.

### 1.3 ترميز wire

- `SignedBlockWire::canonical_wire()` يضيف ترويسة Norito لـ
  `DaCommitmentBundle` مباشرة بعد قائمة المعاملات الحالية. بايت الاصدار هو
  `0x01`.
- `SignedBlockWire::decode_wire()` يرفض الحزم ذات `version` غير معروفة، بما
  يتوافق مع سياسة Norito الموضحة في `norito.md`.
- تحديثات اشتقاق hash موجودة فقط في `block::Hasher`; العملاء الخفيفون الذين
  يفككون wire format الحالي يحصلون تلقائيا على الحقل الجديد لان ترويسة Norito
  تعلن وجوده.

## 2. تدفق انتاج الكتل

1. تنهي عملية ingest الخاصة بـ Torii DA ايصال `DaIngestReceipt` وتقوم بنشره
   على الطابور الداخلي (`iroha_core::gossiper::QueueMessage::DaReceipt`).
2. يجمع `PendingBlocks` كل receipts التي تطابق `lane_id` للكتلة قيد البناء،
   مع ازالة التكرار حسب `(lane_id, client_blob_id, manifest_hash)`.
3. قبل الختم مباشرة، يقوم builder بفرز التعهدات حسب `(lane_id, epoch, sequence)`
   للحفاظ على hash حتمي، ويقوم بترميز الحزمة باستخدام Norito، ويحدث
   `da_commitments_hash`.
4. يتم تخزين الحزمة كاملة في WSV وتصدر مع الكتلة داخل `SignedBlockWire`.

اذا فشل انشاء الكتلة تبقى receipts في الطابور ليتم التقاطها في المحاولة
التالية؛ ويسجل builder اخر `sequence` تم تضمينه لكل lane لتجنب هجمات replay.

## 3. سطح RPC والاستعلام

Torii يوفر ثلاثة endpoints:

| المسار | الطريقة | الحمولة | ملاحظات |
|--------|---------|---------|---------|
| `/v1/da/commitments` | `POST` | `DaCommitmentQuery` (فلترة بنطاق lane/epoch/sequence، مع pagination) | يعيد `DaCommitmentPage` بعدد الاجمالي والتعهدات و hash الكتلة. |
| `/v1/da/commitments/prove` | `POST` | `DaCommitmentProofRequest` (lane + manifest hash او tuple `(epoch, sequence)`). | يعيد `DaCommitmentProof` (record + مسار Merkle + hash الكتلة). |
| `/v1/da/commitments/verify` | `POST` | `DaCommitmentProof` | مساعد stateless يعيد حساب hash الكتلة ويتحقق من الاشتمال؛ يستخدمه SDKs التي لا يمكنها الربط مباشرة مع `iroha_crypto`. |

كل الحمولات تعيش تحت `iroha_data_model::da::commitment`. يقوم Torii بتركيب
handlers بجانب endpoints ingest الحالية لDA لاعادة استخدام سياسات token/mTLS.

## 4. براهين الاشتمال والعملاء الخفيفون

- يبني منتج الكتل شجرة Merkle ثنائية فوق قائمة `DaCommitmentRecord`
  المسلسلة. الجذر يغذي `da_commitments_hash`.
- يحزم `DaCommitmentProof` السجل المستهدف مع متجه من `(sibling_hash, position)`
  لكي يعيد المدققون بناء الجذر. تتضمن البراهين ايضا hash الكتلة والترويسة
  الموقعة حتى يتمكن العملاء الخفيفون من التحقق من finality.
- تساعد اوامر CLI (`iroha_cli app da prove-commitment`) على تنفيذ دورة طلب/تحقق
  البراهين وتعرض مخارج Norito/hex للمشغلين.

## 5. التخزين والفهرسة

يخزن WSV التعهدات في column family مخصصة بمفتاح `manifest_hash`. تغطي
الفهارس الثانوية `(lane_id, epoch)` و`(lane_id, sequence)` كي تتجنب الاستعلامات
مسح الحزم كاملة. يتتبع كل سجل ارتفاع الكتلة التي ختمته، مما يسمح للعقد في
مرحلة catch-up باعادة بناء الفهرس بسرعة من سجل الكتل.

## 6. القياس والرصد

- `torii_da_commitments_total` يزيد عند ختم كتلة تحتوي على سجل واحد على الاقل.
- `torii_da_commitment_queue_depth` يتتبع receipts المنتظرة للتجميع (لكل lane).
- لوحة Grafana `dashboards/grafana/da_commitments.json` تعرض ادراج الكتل وعمق
  الطابور وthroughput البراهين حتى تتمكن بوابات اصدار DA-3 من تدقيق السلوك.

## 7. استراتيجية الاختبارات

1. **اختبارات وحدات** لترميز/فك ترميز `DaCommitmentBundle` وتحديثات اشتقاق hash
   الكتلة.
2. **Fixtures golden** تحت `fixtures/da/commitments/` تلتقط bytes الحزمة
   القياسية وبراهين Merkle.
3. **اختبارات تكامل** تشغل مدققين اثنين، وتبتلع blobs تجريبية، وتتحقق من ان
   كلا العقدتين تتفقان على محتوى الحزمة واستجابات الاستعلام/البرهان.
4. **اختبارات عملاء خفيفين** في `integration_tests/tests/da/commitments.rs`
   (Rust) تستدعي `/prove` وتتحقق من البرهان دون التحدث الى Torii.
5. **Smoke CLI** عبر `scripts/da/check_commitments.sh` لابقاء ادوات المشغلين
   قابلة لاعادة الانتاج.

## 8. خطة الاطلاق

| المرحلة | الوصف | معيار الخروج |
|---------|-------|--------------|
| P0 - دمج نموذج البيانات | دمج `DaCommitmentRecord` وتحديثات ترويسة الكتلة وكودكات Norito. | `cargo test -p iroha_data_model` ينجح مع fixtures جديدة. |
| P1 - توصيل Core/WSV | تمرير منطق الطابور + block builder، وحفظ الفهارس، وكشف handlers RPC. | `cargo test -p iroha_core` و `integration_tests/tests/da/commitments.rs` ينجحان مع اثباتات bundle proof. |
| P2 - ادوات المشغلين | شحن helpers للـ CLI ولوحة Grafana وتحديثات توثيق تحقق proof. | `iroha_cli app da prove-commitment` يعمل على devnet؛ اللوحة تعرض بيانات حية. |
| P3 - بوابة الحوكمة | تفعيل مدقق الكتل الذي يفرض تعهدات DA على lanes المحددة في `iroha_config::nexus`. | تحديث status وroadmap يشيران الى اكتمال DA-3. |

## اسئلة مفتوحة

1. **KZG vs Merkle defaults** - هل يجب تخطي تعهدات KZG للـ blobs الصغيرة لتقليل
   حجم الكتلة؟ الاقتراح: جعل `kzg_commitment` اختياريا وتفعيله عبر
   `iroha_config::da.enable_kzg`.
2. **Sequence gaps** - هل نسمح بفجوات الترتيب؟ الخطة الحالية ترفض الفجوات الا
   اذا فعلت الحوكمة `allow_sequence_skips` لاعادة تشغيل طارئة.
3. **Light-client cache** - طلب فريق SDK تخزين SQLite خفيف للبراهين؛ متابعة
   لاحقة تحت DA-8.

الاجابة على هذه الاسئلة في PRs التنفيذ تنقل DA-3 من مسودة (هذه الوثيقة) الى
قيد العمل بمجرد بدء التنفيذ البرمجي.
