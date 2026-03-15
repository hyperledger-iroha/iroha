---
lang: he
direction: rtl
source: docs/source/bridge_proofs.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f3f6049cbf3aa135e35e4cf06967993c11c1c571ca97dd11c469142dd620be77
source_last_modified: "2025-12-11T23:36:13.998930+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- התרגום העברי ל- docs/source/bridge_proofs.md -->

# הוכחות Bridge

הגשת הוכחות Bridge עוברת בנתיב ההוראות הסטנדרטי (`SubmitBridgeProof`) ומגיעה לרישום ההוכחות עם סטטוס מאומת. המשטח הנוכחי מכסה הוכחות Merkle בסגנון ICS ו-payloads של transparent-ZK עם שמירה מוצמדת וקישור ל-manifest.

## כללי קבלה

- טווחים חייבים להיות מסודרים/לא ריקים ולעמוד ב-`zk.bridge_proof_max_range_len` (0 מבטל את המגבלה).
- חלונות גובה אופציונליים דוחים הוכחות ישנות/עתידיות: `zk.bridge_proof_max_past_age_blocks` ו-`zk.bridge_proof_max_future_drift_blocks` נמדדים מול גובה הבלוק שקולט את ההוכחה (0 מבטל את הסייגים).
- הוכחות Bridge אינן יכולות לחפוף הוכחה קיימת עבור אותו backend (הוכחות pinned נשמרות וחוסמות חפיפות).
- Hashes של manifest חייבים להיות לא-אפס; ה-payloads מוגבלים בגודל לפי `zk.max_proof_size_bytes`.
- payloads של ICS מכבדים את מגבלת עומק ה-Merkle המוגדרת ומאמתים את הנתיב באמצעות פונקציית ה-hash המוצהרת; payloads שקופים חייבים להצהיר על תווית backend לא ריקה.
- הוכחות pinned פטורות מניקוי שמירה; הוכחות לא pinned עדיין מכבדות את `zk.proof_history_cap`/grace/batch הגלובליים.

## משטח ה-API של Torii

- `GET /v1/zk/proofs` ו-`GET /v1/zk/proofs/count` מקבלים מסננים מודעי-bridge:
  - `bridge_only=true` מחזיר רק הוכחות Bridge.
  - `bridge_pinned_only=true` מצמצם להוכחות Bridge pinned.
  - `bridge_start_from_height` / `bridge_end_until_height` מגבילים את חלון הטווח של bridge.
- `GET /v1/zk/proof/{backend}/{hash}` מחזיר מטא-דאטה של bridge (טווח, hash של manifest, תקציר payload) לצד מזהה/סטטוס הוכחה וקשרי VK.
- רשומת ההוכחה המלאה של Norito (כולל bytes של payload) זמינה עדיין דרך `GET /v1/proofs/{proof_id}` עבור מאמתים מחוץ לצומת.

## אירועי קבלה של Bridge

נתיבי Bridge פולטים קבלות טיפוסיות דרך ההוראה `RecordBridgeReceipt`. ביצוע ההוראה רושם payload מסוג `BridgeReceipt` ופולט `DataEvent::Bridge(BridgeEvent::Emitted)` בזרם האירועים, במקום ה-stub הישן שהיה רק לוג. העזר CLI `iroha bridge emit-receipt` שולח את ההוראה הטיפוסית כדי שמאנדקסים יצרכו קבלות באופן דטרמיניסטי.

## סקיצת אימות חיצונית (ICS)

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
