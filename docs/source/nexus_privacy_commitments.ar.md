---
lang: ar
direction: rtl
source: docs/source/nexus_privacy_commitments.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e7ef8ab7f52ec333d3fb9686dd744e23a859058dc0dbb91cfafee1cb7d1452ac
source_last_modified: "2025-11-21T17:27:28.236084+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/source/nexus_privacy_commitments.md -->

# اطار التزامات الخصوصية واثباتاتها (NX-10)

> **الحالة:** 🈴 مكتمل (NX-10)  
> **المالكون:** Cryptography WG / Privacy WG / Nexus Core WG  
> **الكود المرتبط:** [`crates/iroha_crypto/src/privacy.rs`](../../crates/iroha_crypto/src/privacy.rs)

تقدم NX-10 سطح التزام موحد للمسارات الخاصة. ينشر كل dataspace واصفا حتميا يربط جذور Merkle او
دوائر zk-SNARK بهاشات قابلة لاعادة الانتاج. بالتالي يمكن للحلقة العالمية في Nexus التحقق من
تحويلات cross-lane واثباتات السرية دون parsers مخصصة.

## الاهداف والنطاق

- توحيد معرفات الالتزام حتى تتفق manifests الحوكمة وSDKs على الفتحة العددية المستخدمة اثناء
  admission.
- توفير helpers تحقق قابلة لاعادة الاستخدام (`iroha_crypto::privacy`) حتى تتحقق runtimes وTorii
  والمدققون off-chain من الاثباتات بشكل متسق.
- ربط اثباتات zk-SNARK بالـ digest القياسي لمفتاح التحقق وبترميزات حتمية للـ public inputs. بدون
  transcripts عشوائية.
- توثيق سير registry/export حتى تتضمن حزم lanes وادلة الحوكمة نفس الهاشات التي يفرضها runtime.

خارج نطاق هذه المذكرة: ميكانيكيات DA fan-out ورسائل relay واعمال plumbing الخاصة بـ settlement
router. راجع `nexus_cross_lane.md` لهذه الطبقات.

## نموذج التزامات lane

يحتفظ registry بقائمة مرتبة من entries لـ `LanePrivacyCommitment`:

```rust
use iroha_crypto::privacy::{
    CommitmentScheme, LaneCommitmentId, LanePrivacyCommitment, MerkleCommitment, SnarkCircuit,
    SnarkCircuitId,
};

let id = LaneCommitmentId::new(1);
let merkle = LanePrivacyCommitment::merkle(
    id,
    MerkleCommitment::from_root_bytes(root_bytes, 16),
);

let snark = LanePrivacyCommitment::snark(
    LaneCommitmentId::new(2),
    SnarkCircuit::new(
        SnarkCircuitId::new(42),
        verifying_key_digest,
        statement_hash,
        proof_hash,
    ),
);
```

- **`LaneCommitmentId`** — معرف ثابت بطول 16-بت يسجل في manifests الخاصة بالlane.
- **`CommitmentScheme`** — `Merkle` او `Snark`. المتغيرات المستقبلية (مثل bulletproofs) توسع enum.
- **Semantics النسخ** — كل الواصفات تطبق `Copy` حتى تعيد configs استخدامها دون churn في heap.

يسير registry مع bundle الخاص بـ Nexus lane (`scripts/nexus/lane_registry_bundle.py`). عندما تعتمد
الحوكمة التزاما جديدا، يقوم bundle بتحديث manifest JSON وoverlay Norito الذي تستهلكه admission.

### مخطط manifest (`privacy_commitments`)

تكشف manifests الخاصة بالlane الان مصفوفة `privacy_commitments`. كل entry يعين ID وscheme ويشمل
المعلمات الخاصة بالscheme:

```json
{
  "lane": "cbdc",
  "governance": "council",
  "privacy_commitments": [
    {
      "id": 1,
      "scheme": "merkle",
      "merkle": {
        "root": "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        "max_depth": 16
      }
    },
    {
      "id": 2,
      "scheme": "snark",
      "snark": {
        "circuit_id": 5,
        "verifying_key_digest": "0x...",
        "statement_hash": "0x...",
        "proof_hash": "0x..."
      }
    }
  ]
}
```

ينسخ bundler الخاص بالregistry الـ manifest، ويسجل ازواج `(id, scheme)` في `summary.json`، وتعيد
CI (`lane_registry_verify.py`) تحليل manifest للتأكد من ان summary يطابق المحتوى على القرص.

## التزامات Merkle

تلتقط `MerkleCommitment` قيمة `HashOf<MerkleTree<[u8;32]>>` قياسية وعمق مسار تدقيق اقصى. يقوم
المشغلون بتصدير الجذر مباشرة من prover او snapshot للدفتر؛ لا يوجد اعادة hashing داخل registry.

تدفق التحقق:

```rust
use iroha_crypto::privacy::{LanePrivacyCommitment, MerkleWitness, PrivacyWitness};

let witness = MerkleWitness::from_leaf_bytes(leaf_bytes, proof);
LanePrivacyCommitment::merkle(id, commitment)
    .verify(PrivacyWitness::Merkle(witness))?;
```

- الاثباتات التي تتجاوز `max_depth` تصدر `PrivacyError::MerkleProofExceedsDepth`.
- تستخدم audit paths ادوات `iroha_crypto::MerkleProof` حتى تشترك shielded pools وNexus private lanes
  في نفس التسلسل.
- المضيفون الذين يبتلعون pools خارجية يحولون shielded leaves عبر
  `MerkleTree::shielded_leaf_from_commitment` قبل بناء الـ witness.

### قائمة تشغيل

- نشر tuple `(id, root, depth)` في summary الخاص بـ manifest والـ evidence bundle.
- ارفاق Merkle inclusion proof لكل تحويل cross-lane في سجل admission؛ يعيد helper انتاج الجذر
  byte-for-byte لذا تقارن عمليات التدقيق الهاش المصدر فقط.
- تتبع ميزانيات العمق في التلِمتري (`nexus_privacy_commitments.merkle.depth_used`) حتى تطلق
  التنبيهات قبل تجاوز حدود rollout المضبوطة.

## التزامات zk-SNARK

تربط entries `SnarkCircuit` اربعة حقول:

| الحقل | الوصف |
|-------|-------------|
| `circuit_id` | معرف تتحكم به الحوكمة لزوج الدائرة/الاصدار. |
| `verifying_key_digest` | هاش Blake3 لمفتاح التحقق القياسي (DER او ترميز Norito). |
| `statement_hash` | هاش Blake3 لترميز public inputs القياسي (Norito او Borsh). |
| `proof_hash` | هاش Blake3 لـ `verifying_key_digest || proof_bytes`. |

مساعدات runtime:

```rust
use iroha_crypto::privacy::{
    hash_proof, hash_public_inputs, LanePrivacyCommitment, PrivacyWitness, SnarkWitness,
};

let witness = SnarkWitness {
    public_inputs: encoded_inputs,
    proof: proof_bytes,
};

LanePrivacyCommitment::snark(id, circuit)
    .verify(PrivacyWitness::Snark(witness))?;
```

- `hash_public_inputs` و`hash_proof` يعيشان في نفس الوحدة؛ يجب على SDKs استدعاؤهما عند توليد
  manifests او الاثباتات لتجنب format drift.
- اي عدم تطابق بين statement hash المسجل وpublic inputs المقدمة ينتج
  `PrivacyError::SnarkStatementMismatch`.
- يجب ان تكون proof bytes بالفعل في صيغة مضغوطة قياسية (Groth16, Plonk, الخ). يقوم helper فقط
  بالتحقق من binding الهاش؛ يبقى التحقق الكامل للمنحنيات في خدمة prover/verifier.

### سير ادلة الاثبات

1. تصدير verifying key digest وproof hash الى manifest الخاص بـ lane bundle.
2. ارفاق artefact الاثبات الخام بحزمة ادلة الحوكمة حتى يتمكن المدققون من اعادة حساب `hash_proof`.
3. نشر ترميز public inputs القياسي (hex او Norito) بجانب statement hash لاعادة التشغيل الحتمي.

## مسار ابتلاع الاثباتات

1. **Manifests الخاصة بالlane** تعلن اي `LaneCommitmentId` يحكم كل عقد او bucket
   programmable-money.
2. **Admission** تستهلك entries من `ProofAttachment.lane_privacy`، وتتحقق من witnesses المرفقة
   للمسار الموجه عبر `LanePrivacyRegistry::verify`، وتغذي commitment ids المؤكدة في lane
   compliance (`privacy_commitments_any_of`) حتى تفرض قواعد allow/deny الخاصة بـ programmable-money
   وجود الاثبات قبل ادخال الصف.
3. **التلِمتري** يزيد عدادات `nexus_privacy_commitments.{merkle,snark}` مع `LaneId` و`commitment_id`
   والنتيجة (`ok`, `depth_mismatch`, الخ).
4. **ادلة الحوكمة** تستخدم نفس helper لاعادة توليد تقارير acceptance
   (`ci/check_nexus_lane_registry_bundle.sh` يرتبط بالتحقق من bundle عند وصول بيانات SNARK).

### رؤية المشغلين

يكشف endpoint `/v2/sumeragi/status` في Torii الان مصفوفة `lane_governance[].privacy_commitments`
حتى يتمكن المشغلون وSDKs من مقارنة registry الحي مع manifests المنشورة دون اعادة قراءة bundle. يتم
بناء snapshot داخل `crates/iroha_core/src/sumeragi/status.rs`، ويتم تصديره عبر REST/JSON handlers
في Torii (`crates/iroha_torii/src/routing.rs`)، ويفك كل عميل ترميزه
(`javascript/iroha_js/src/toriiClient.js`,
`python/iroha_python/src/iroha_python/client.py`, `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift`)،
مع محاكاة مخطط manifest لجذور Merkle وdigests الخاصة بـ SNARK.

تفشل lanes من نوع commitment-only او split-replica في admission اذا اهمل manifest قسم
`privacy_commitments`، مما يضمن عدم بدء تدفقات programmable-money حتى تصل anchors الاثبات الحتمية
مع bundle.

## Registry وقت التشغيل و admission

- `LanePrivacyRegistry` (`crates/iroha_core/src/interlane/mod.rs`) تلتقط snapshot لهياكل
  `LaneManifestStatus` من manifest loader وتخزن خرائط commitments لكل lane بمفاتيح
  `LaneCommitmentId`. تقوم transaction queue و`State` بتثبيت هذا registry مع كل اعادة تحميل للـ
  manifest (`Queue::install_lane_manifests`, `State::install_lane_manifests`)، بحيث يمتلك مسار
  admission ومسار consensus validation دائما الالتزامات القياسية.
- `LaneComplianceContext` يحمل الان مرجعا اختياريا `Arc<LanePrivacyRegistry>`
  (`crates/iroha_core/src/compliance/mod.rs`). عندما يقوم `LaneComplianceEngine` بتقييم معاملة،
  يمكن لتدفقات programmable-money فحص نفس الالتزامات لكل lane المعروضة عبر Torii قبل ادخال
  الحمولة في الصف.
- Admission وcore validation تحتفظان بـ `Arc<LanePrivacyRegistry>` بجانب مقبض manifest الحوكمي
  (`crates/iroha_core/src/queue.rs`, `crates/iroha_core/src/state.rs`) لضمان ان وحدات
  programmable-money والمضيفين interlane المستقبليين يقرؤون رؤية متسقة لوصفات الخصوصية حتى مع
  تدوير manifests.

## فرض وقت التشغيل

تنتقل اثباتات الخصوصية للـ lane الان مع attachments المعاملة عبر `ProofAttachment.lane_privacy`.
يتحقق admission عبر Torii وvalidation في `iroha_core` من كل attachment مقابل registry الخاص بالlane
الموجه عبر `LanePrivacyRegistry::verify`، ويسجل commitment ids المثبتة، ويغذيها في compliance
engine. اي قاعدة تحدد `privacy_commitments_any_of` تقيم الى `false` ما لم ينجح attachment مطابق،
لذا لا يمكن وضع programmable-money lanes في الصف او commitها دون witness المطلوب. تغطية
الاختبارات الوحدوية موجودة في `interlane::tests` واختبارات lane compliance للحفاظ على مسار
attachments وحواجز السياسة.

للتجربة الفورية، راجع الاختبارات الوحدوية في
[`privacy.rs`](../../crates/iroha_crypto/src/privacy.rs) التي توضح حالات النجاح والفشل لالتزامات
Merkle وzk-SNARK.

</div>
