---
lang: my
direction: ltr
source: docs/source/bridge_proofs.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 65aff839e8970e96edb07dfb9655cb4e79f56d1d885b7782647f5dc8f328027b
source_last_modified: "2025-12-29T18:16:35.921274+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# တံတားအထောက်အထားများ

တံတားအထောက်အထား တင်ပြချက်များသည် စံညွန်ကြားချက်လမ်းကြောင်း (`SubmitBridgeProof`) မှတဆင့် ဖြတ်သန်းပြီး အထောက်အထား မှတ်ပုံတင်ခြင်းတွင် ဆင်းသက်သည်။ လက်ရှိမျက်နှာပြင်သည် ICS ပုံစံ Merkle အထောက်အထားများနှင့် ဖောက်ထွင်းမြင်ရသော-ZK ပါ၀င်ပစ္စည်းများကို ပင်ထိုးထိန်းသိမ်းထားပြီး ထင်ရှားသောစည်းနှောင်မှုဖြင့် ဖုံးအုပ်ထားသည်။

##လက်ခံမှုစည်းမျဉ်း

- အပိုင်းအခြားများကို အမိန့်ပေးရမည်/ဗလာမဟုတ်သော နှင့် `zk.bridge_proof_max_range_len` ကိုလေးစားရပါမည် (0 ထုပ်ကိုပိတ်သည်)။
- ရွေးချယ်နိုင်သော အမြင့်ပြတင်းပေါက်များသည် ဟောင်းနွမ်းခြင်း/အနာဂတ် အထောက်အထားများကို ငြင်းပယ်ခြင်း- `zk.bridge_proof_max_past_age_blocks` နှင့် `zk.bridge_proof_max_future_drift_blocks` ကို အထောက်အထားထည့်သွင်းသည့် ဘလောက်အမြင့်နှင့် တိုင်းတာသည် (0 guardrails များကို ပိတ်သည်)။
- တံတားအထောက်အထားများသည် တူညီသောနောက်ခံအတွက် ရှိပြီးသားအထောက်အထားများကို ထပ်နေမည်မဟုတ်ပါ (ပင်ထိုးထားသောအထောက်အထားများကို ထိန်းသိမ်းထားပြီး ထပ်နေမှုများကို ပိတ်ဆို့ထားသည်)။
- Manifest hash များသည် သုညမဟုတ်ရပါမည်။ payload များသည် `zk.max_proof_size_bytes` ဖြင့် အရွယ်အစားကို ကန့်သတ်ထားသည်။
- ICS payloads များသည် သတ်မှတ်ထားသော Merkle အတိမ်အနက်ကို ဂုဏ်ပြုပြီး ကြေညာထားသော hash လုပ်ဆောင်ချက်ကို အသုံးပြု၍ လမ်းကြောင်းကို စစ်ဆေးပါ။ ပွင့်လင်းမြင်သာသော payload များသည် ဗလာမဟုတ်သော နောက်ခံအညွှန်းကို ကြေညာရပါမည်။
- Pinned proof များသည် retention pruning မှ ကင်းလွတ်ပါသည်။ ပင်မထိုးထားသော အထောက်အထားများသည် ကမ္ဘာလုံးဆိုင်ရာ `zk.proof_history_cap`/grace/batch ဆက်တင်များကို လေးစားဆဲဖြစ်သည်။

## Torii API မျက်နှာပြင်

- `GET /v1/zk/proofs` နှင့် `GET /v1/zk/proofs/count` တံတားသတိပြုမိသော စစ်ထုတ်မှုများကို လက်ခံသည်-
  - `bridge_only=true` သည် တံတားအထောက်အထားများကိုသာ ပြန်ပေးသည်။
  - `bridge_pinned_only=true` သည် ချိတ်တွဲထားသော တံတားအထောက်အထားများကို ကျဉ်းစေသည်။
  - `bridge_start_from_height` / `bridge_end_until_height` တံတားဘောင်ပြတင်းပေါက်ကို ကုပ်ပါ။
- `GET /v1/zk/proof/{backend}/{hash}` သည် အထောက်အထား id/status/VK bindings များနှင့်အတူ တံတား metadata (အပိုင်းအခြား၊ manifest hash၊ payload summary) ကို ပြန်ပေးသည်။
- Norito အထောက်အထားမှတ်တမ်း အပြည့်အစုံ (payload bytes အပါအဝင်) ကို off-node verifiers အတွက် `GET /v1/proofs/{proof_id}` မှတစ်ဆင့် ရရှိနိုင်ပါသည်။

## တံတားဖြတ်ပိုင်းပွဲများ

တံတားလမ်းများသည် `RecordBridgeReceipt` ညွှန်ကြားချက်မှတစ်ဆင့် ရိုက်ထားသည့် ပြေစာများကို ထုတ်လွှတ်ပါသည်။ ဤညွှန်ကြားချက်ကို အကောင်အထည်ဖော်ခြင်း။
`BridgeReceipt` payload ကို မှတ်တမ်းတင်ပြီး `DataEvent::Bridge(BridgeEvent::Emitted)` ကို ပွဲပေါ်တွင် ထုတ်လွှတ်သည်
ကြိုတင် မှတ်တမ်းသီးသန့် ဆောင်းပါးတိုကို အစားထိုး၍ စီးကြောင်း။ CLI `iroha bridge emit-receipt` အကူအညီပေးသူက တင်ပြသည်။
စာရိုက်ထားသည့် ညွှန်ကြားချက်ကြောင့် ညွှန်းကိန်းများသည် ပြေစာများကို အဆုံးအဖြတ်ပေးနိုင်သည်။

## ပြင်ပအတည်ပြုပုံကြမ်း (ICS)

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