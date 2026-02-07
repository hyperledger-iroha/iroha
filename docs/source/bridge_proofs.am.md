---
lang: am
direction: ltr
source: docs/source/bridge_proofs.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 65aff839e8970e96edb07dfb9655cb4e79f56d1d885b7782647f5dc8f328027b
source_last_modified: "2025-12-29T18:16:35.921274+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# የድልድይ ማስረጃዎች

የድልድይ ማረጋገጫ ማቅረቢያዎች በመደበኛው የማስተማሪያ መንገድ (`SubmitBridgeProof`) እና በማረጋገጫ መዝገብ ውስጥ ከተረጋገጠ ሁኔታ ጋር ይጓዛሉ። አሁን ያለው ወለል የአይ.ሲ.ኤስ አይነት የመርክል ማረጋገጫዎችን እና ግልጽ-ZK ሸክሞችን በተሰካ ማቆየት እና ግልጽ ማሰርን ይሸፍናል።

## የመቀበል ህጎች

- ክልሎች ማዘዝ/ባዶ ያልሆኑ እና `zk.bridge_proof_max_range_len` ማክበር አለባቸው (0 ቆብ ያሰናክላል)።
- አማራጭ ቁመት መስኮቶች ያረጁ/ወደፊት ማረጋገጫዎች ውድቅ: `zk.bridge_proof_max_past_age_blocks` እና `zk.bridge_proof_max_future_drift_blocks` ያለውን የማገጃ ቁመት ላይ የሚለካው ማስረጃውን ወደ ውስጥ (0 ጠባቂዎቹም ያሰናክላል).
- የድልድይ ማረጋገጫዎች ለተመሳሳይ የኋላ መጋረጃ ነባር ማረጋገጫ ላይደራረቡ አይችሉም (የተሰካው ማረጋገጫዎች ተጠብቀው መደራረብን አግደዋል)።
- አንጸባራቂ hashes ዜሮ ያልሆኑ መሆን አለባቸው; የሚጫኑ ጭነቶች በ `zk.max_proof_size_bytes` በመጠን ተያይዘዋል።
- የአይ.ሲ.ኤስ የደመወዝ ጭነቶች የተዋቀረውን የመርክል ጥልቀት ካፕ ያከብራሉ እና የታወጀውን የሃሽ ተግባር በመጠቀም መንገዱን ያረጋግጡ። ግልጽ ጭነት ባዶ ያልሆነ የኋላ መለያ መለያ ማወጅ አለበት።
- የተጣበቁ ማረጋገጫዎች ከማቆየት መግረዝ ነፃ ናቸው; ያልተሰካ ማረጋገጫዎች አሁንም ዓለም አቀፋዊውን የ`zk.proof_history_cap`/የጸጋ/ባች ቅንጅቶችን ያከብራሉ።

## Torii ኤፒአይ ወለል

- `GET /v1/zk/proofs` እና `GET /v1/zk/proofs/count` ድልድይ የሚያውቁ ማጣሪያዎችን ይቀበላሉ፡
  - `bridge_only=true` የድልድይ ማረጋገጫዎችን ብቻ ይመልሳል።
  - `bridge_pinned_only=true` በተሰካው ድልድይ ማረጋገጫዎች ላይ ጠባብ።
  - `bridge_start_from_height` / `bridge_end_until_height` የድልድይ ክልል መስኮቱን አጣብቅ።
- `GET /v1/zk/proof/{backend}/{hash}` ከማስረጃ መታወቂያ/ሁኔታ/VK ማሰሪያዎች ጎን ለጎን ድልድይ ሜታዳታ (ክልል፣ አንጸባራቂ ሃሽ፣ የክፍያ ማጠቃለያ) ይመልሳል።
- ሙሉው የNorito የማስረጃ መዝገብ (የክፍያ ባይት ጨምሮ) በ`GET /v1/proofs/{proof_id}` ከአንጓ ውጪ አረጋጋጮች ይገኛል።

## የድልድይ ደረሰኝ ዝግጅቶች

የድልድይ መስመሮች የተተየቡ ደረሰኞችን በ`RecordBridgeReceipt` መመሪያ ይለቃሉ። ይህንን መመሪያ በመተግበር ላይ
የ `BridgeReceipt` ክፍያ መዝግቦ በዝግጅቱ ላይ `DataEvent::Bridge(BridgeEvent::Emitted)` ያወጣል።
ዥረት፣ የቀደመውን የምዝግብ ማስታወሻ-ብቻ ግንድ በመተካት። የ CLI `iroha bridge emit-receipt` ረዳት ያቀርባል
መረጃ ጠቋሚዎች ደረሰኞችን በቆራጥነት እንዲበሉ የተተየበው መመሪያ።

## የውጭ ማረጋገጫ ንድፍ (ICS)

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