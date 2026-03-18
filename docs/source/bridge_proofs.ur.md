---
lang: ur
direction: rtl
source: docs/source/bridge_proofs.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f3f6049cbf3aa135e35e4cf06967993c11c1c571ca97dd11c469142dd620be77
source_last_modified: "2025-12-11T23:36:13.998930+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- اردو ترجمہ برائے docs/source/bridge_proofs.md -->

# Bridge proofs

Bridge proof submissions معیاری instruction path (`SubmitBridgeProof`) سے گزرتی ہیں اور verified status کے ساتھ proof registry میں پہنچتی ہیں۔ موجودہ سطح ICS طرز کی Merkle proofs اور transparent-ZK payloads کو کور کرتی ہے جن میں pinned retention اور manifest binding شامل ہے۔

## قبولیت کے قواعد

- رینجز مرتب/غیر خالی ہوں اور `zk.bridge_proof_max_range_len` کی پابندی کریں (0 حد کو غیر فعال کرتا ہے)۔
- اختیاری height windows پرانی/مستقبل کی proofs کو مسترد کرتی ہیں: `zk.bridge_proof_max_past_age_blocks` اور `zk.bridge_proof_max_future_drift_blocks` اس بلاک کی height کے مقابل ماپی جاتی ہیں جو proof کو ingest کرتا ہے (0 guardrails کو غیر فعال کرتا ہے)۔
- Bridge proofs اسی backend کیلئے موجودہ proof کے ساتھ overlap نہیں کر سکتیں (pinned proofs محفوظ رہتی ہیں اور overlaps روکتی ہیں)۔
- Manifest hashes غیر صفر ہونے چاہئیں؛ payloads کا سائز `zk.max_proof_size_bytes` کے تحت محدود ہے۔
- ICS payloads configured Merkle depth cap کی پابندی کرتے ہیں اور اعلان شدہ hash function کے ساتھ path verify کرتے ہیں؛ transparent payloads کو non-empty backend label اعلان کرنا لازم ہے۔
- Pinned proofs retention pruning سے مستثنی ہیں؛ unpinned proofs پھر بھی global `zk.proof_history_cap`/grace/batch سیٹنگز کی پابندی کرتی ہیں۔

## Torii API سطح

- `GET /v1/zk/proofs` اور `GET /v1/zk/proofs/count` bridge-aware filters قبول کرتے ہیں:
  - `bridge_only=true` صرف bridge proofs واپس کرتا ہے۔
  - `bridge_pinned_only=true` pinned bridge proofs تک محدود کرتا ہے۔
  - `bridge_start_from_height` / `bridge_end_until_height` bridge range window کو محدود کرتے ہیں۔
- `GET /v1/zk/proof/{backend}/{hash}` bridge metadata (range, manifest hash, payload summary) کے ساتھ proof id/status/VK bindings واپس کرتا ہے۔
- مکمل Norito proof record (payload bytes سمیت) `GET /v1/proofs/{proof_id}` کے ذریعے آف نوڈ verifiers کیلئے دستیاب رہتا ہے۔

## Bridge receipt events

Bridge lanes `RecordBridgeReceipt` instruction کے ذریعے typed receipts جاری کرتی ہیں۔ اس instruction کو execute کرنے سے `BridgeReceipt` payload ریکارڈ ہوتا ہے اور event stream میں `DataEvent::Bridge(BridgeEvent::Emitted)` خارج ہوتا ہے، جو پہلے کے log-only stub کو بدل دیتا ہے۔ CLI `iroha bridge emit-receipt` helper typed instruction بھیجتا ہے تاکہ indexers receipts کو deterministically consume کر سکیں۔

## External verification sketch (ICS)

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
