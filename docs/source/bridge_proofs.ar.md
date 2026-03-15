---
lang: ar
direction: rtl
source: docs/source/bridge_proofs.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f3f6049cbf3aa135e35e4cf06967993c11c1c571ca97dd11c469142dd620be77
source_last_modified: "2025-12-11T23:36:13.998930+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/source/bridge_proofs.md -->

# اثباتات Bridge

تنتقل طلبات اثباتات Bridge عبر مسار التعليمات القياسي (`SubmitBridgeProof`) وتصل الى سجل الاثباتات بحالة verified. السطح الحالي يغطي اثباتات Merkle على نمط ICS و payloads من transparent-ZK مع احتفاظ مثبت وربط بالـ manifest.

## قواعد القبول

- يجب ان تكون النطاقات مرتبة وغير فارغة وتلتزم بـ `zk.bridge_proof_max_range_len` (0 يعطل الحد).
- نوافذ الارتفاع الاختيارية ترفض الاثباتات القديمة/المستقبلية: `zk.bridge_proof_max_past_age_blocks` و `zk.bridge_proof_max_future_drift_blocks` تقاس مقابل ارتفاع الكتلة التي تستقبل الاثبات (0 يعطل الحواجز).
- لا يجوز لاثباتات Bridge ان تتداخل مع اثبات موجود لنفس backend (الاثباتات المثبتة تبقى وتمنع التداخل).
- يجب ان تكون hashes الخاصة بالـ manifest غير صفرية؛ احجام payloads محددة بـ `zk.max_proof_size_bytes`.
- payloads الخاصة بـ ICS تحترم حد عمق Merkle المكون وتتحقق من المسار باستخدام دالة الهاش المعلنة؛ payloads الشفافة يجب ان تعلن عن label غير فارغ للـ backend.
- الاثباتات المثبتة معفاة من التقليم حسب الاحتفاظ؛ الاثباتات غير المثبتة ما زالت تحترم اعدادات `zk.proof_history_cap`/grace/batch العامة.

## سطح واجهة Torii API

- `GET /v1/zk/proofs` و `GET /v1/zk/proofs/count` يقبلان مرشحات واعية بالـ bridge:
  - `bridge_only=true` يعيد اثباتات Bridge فقط.
  - `bridge_pinned_only=true` يضيق الى اثباتات Bridge المثبتة فقط.
  - `bridge_start_from_height` / `bridge_end_until_height` يحدان نافذة نطاق bridge.
- `GET /v1/zk/proof/{backend}/{hash}` يعيد بيانات bridge (النطاق، hash الخاص بالـ manifest، ملخص الـ payload) بجانب معرف/حالة الاثبات وروابط VK.
- سجل Norito الكامل (بما في ذلك bytes الخاصة بالـ payload) يبقى متاحا عبر `GET /v1/proofs/{proof_id}` للمتحققين خارج العقدة.

## احداث ايصالات Bridge

تطلق lanes الخاصة بالـ bridge ايصالات مكتوبة عبر التعليمة `RecordBridgeReceipt`. تنفيذ هذه التعليمة يسجل payload من نوع `BridgeReceipt` ويطلق `DataEvent::Bridge(BridgeEvent::Emitted)` على تيار الاحداث، مستبدلا الـ stub السابق المعتمد على السجلات فقط. مساعد CLI `iroha bridge emit-receipt` يرسل التعليمة المكتوبة كي يتمكن المفهرسون من استهلاك الايصالات بشكل حتمي.

## مخطط التحقق الخارجي (ICS)

```rust
use iroha_data_model::bridge::{BridgeHashFunction, BridgeProofPayload, BridgeProofRecord};
use iroha_crypto::{Hash, HashOf, MerkleTree};

fn verify_ics(record: &BridgeProofRecord) -> bool {
    let BridgeProofPayload::Ics(ics) = &record.proof.payload else {
        return false;
    };
    let leaf = HashOf::<[u8; 32]>::from_untyped_unchecked(Hash::prehashed(ics.leaf_hash));
    let root =
        HashOf::<MerkleTree<[u8; 32]>>::from_untyped_unchecked(Hash::prehashed(ics.state_root));
    match ics.hash_function {
        BridgeHashFunction::Sha256 => ics.proof.clone().verify_sha256(&leaf, &root, ics.proof.audit_path().len()),
        BridgeHashFunction::Blake2b => ics.proof.clone().verify(&leaf, &root, ics.proof.audit_path().len()),
    }
}
```

</div>
