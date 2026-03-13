---
lang: dz
direction: ltr
source: docs/source/bridge_proofs.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 65aff839e8970e96edb07dfb9655cb4e79f56d1d885b7782647f5dc8f328027b
source_last_modified: "2025-12-29T18:16:35.921274+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# ཟམ་པའི་བདེན་དཔང་།

ཟམ་གྱི་བདེན་དཔང་ཕུལ་མི་ཚུ་གིས་ གནས་ཚད་ཅན་གྱི་སློབ་སྟོན་ལམ་ལུགས་ (`SubmitBridgeProof`) དང་ བདེན་ཁུངས་ཐོ་བཀོད་ནང་ བདེན་དཔྱད་འབད་ཡོད་པའི་གནས་རིམ་ཐོག་ལས་ འགྱོཝ་ཨིན། ད་ལྟོའི་ཁ་ཐོག་འདི་གིས་ ICS-style Merkle གི་བདེན་ཁུངས་དང་ དྭངས་གསལ་-ZK གི་དངུལ་ཕོགས་ཚུ་ བཀག་བཞག་ནི་དང་ མངོན་གསལ་གྱི་བསྡམ་བཞག་ཚུ་ ཁྱབ་ཚུགསཔ་ཨིན།

## ངོས་ལེན་ལམ་ལུགས་ཀྱི་ལམ་ལུགས།

- ཁྱབ་ཚད་ཚུ་ བཀོད་སྒྲིག་འབད་/སྟོངམ་དང་ སྟོངམ་དང་ བརྩི་མཐོང་ `zk.bridge_proof_max_range_len` (༠ ལྕོགས་མིན་བཟོཝ་ཨིན།)
- གདམ་ཁའི་མཐོ་ཚད་སྒོ་སྒྲིག་ཚུ་གིས་ ཕྲང་ཏང་ཏ་/མ་འོངས་པའི་བདེན་ཁུངས་ཚུ་ ངོས་ལེན་མ་འབད་བར་: `zk.bridge_proof_max_past_age_blocks` དང་ `zk.bridge_proof_max_future_drift_blocks` ཚུ་ བདེན་དཔང་འབད་མི་ སྡེབ་ཚན་གྱི་མཐོ་ཚད་ལུ་ འཇལ་ཚད་ (༠ སྲུང་སྐྱོབ་ཚུ་ ལྕོགས་མིན་བཟོཝ་ཨིན།)
- ཟམ་གྱི་བདེན་ཁུངས་ཚུ་གིས་ རྒྱབ་ཐག་གཅིག་པའི་དོན་ལུ་ ད་ལྟོ་ཡོད་པའི་བདེན་ཁུངས་ཅིག་ གཅིག་ལུ་གཅིག་བརྩེགས་མི་འབད་ ༼པིན་འབད་ཡོད་པའི་བདེན་ཁུངས་ཚུ་ ཉམས་སྲུང་འབད་དེ་ བཀག་ཆ་ཚུ་ གཅིག་ཁར་བསྡོམས་ཏེ་ཡོདཔ་ཨིན།༽
- མ་ཕེསཊ་ཧ་ཤེ་སི་ ཀླད་ཀོར་མེན་དགོ། པེ་ལོཌ་ཚུ་ `zk.max_proof_size_bytes` གིས་ ཚད་བཟུང་ཡོདཔ་ཨིན།
- ICS གིས་ རིམ་སྒྲིག་འབད་ཡོད་པའི་ Merkle གཏིང་ཚད་ཀྱི་ མགུ་ཏོག་ལུ་གུས་ཞབས་འབད་ཞིནམ་ལས་ གསལ་བསྒྲགས་འབད་ཡོད་པའི་ཧེ་ཤི་ལས་འགན་ལག་ལེན་འཐབ་ཐོག་ལས་ འགྲུལ་ལམ་བདེན་དཔྱད་འབདཝ་ཨིན། དྭངས་གསལ་ཅན་གྱི་གླ་ཆ་ཚུ་གིས་ སྟོངམ་མེན་པའི་རྒྱབ་ཐག་ཁ་ཡིག་ཅིག་ གསལ་བསྒྲགས་འབད་དགོ།
- པིན་ནའིན་གྱི་བདེན་ཁུངས་ཚུ་ བཀག་འཛིན་གྱི་ གཤག་བཅོས་ལས་ དགོངས་ཞུ་འབད་ཡོདཔ་ཨིན། unpinned བདེན་དཔང་ཚུ་གིས་ ད་ལྟོ་ཡང་ འཛམ་གླིང་ཡོངས་ཁྱབ་ཀྱི་ `zk.proof_history_cap`/grace/batch སྒྲིག་སྟངས་ཚུ་ལུ་ གུས་ཞབས་འབདཝ་ཨིན།

## Torii API ངོས་འཛིན།

- `GET /v2/zk/proofs` དང་ `GET /v2/zk/proofs/count` དང་ལེན་གྱི་ངོས་ལེན་ཅན་གྱི་ཟམ་པའི་ཚགས་མ་ཚུ།
  - `bridge_only=true` གིས་ ཟམ་གྱི་བདེན་ཁུངས་ཚུ་རྐྱངམ་ཅིག་སླར་ལོག་འབདཝ་ཨིན།
  - `bridge_pinned_only=true` གིས་ ཟམ་གྱི་བདེན་དཔྱད་ཚུ་ལུ་ རྒྱ་ཆུང་ཀུ་བཟོཝ་ཨིན།
  - `bridge_start_from_height` / `bridge_end_until_height` ཟམ་གྱི་ཁྱབ་ཚད་སྒོ་སྒྲིག་འདི་བཀག་བཞགཔ་ཨིན།
- `GET /v2/zk/proof/{backend}/{hash}` གིས་ ཟམ་གྱི་མེ་ཊ་ཌེ་ཊ་ (ཁྱབ་ཚད་, གསལ་སྟོན་ཧེཤ་, པེ་ལོཌི་བཅུད་བསྡུས་) སླར་ལོག་འབདཝ་ཨིན།
- Norito ཆ་ཚང་བདེན་པའི་དྲན་ཐོ་ (པེ་ལོཌ་བཱའིཊིསི་ཚུ་རྩིས་ཏེ་) ཆ་ཚང་འདི་ `GET /v2/proofs/{proof_id}` བརྒྱུད་དེ་ མཐུད་ལམ་མེད་པའི་བདེན་བཤད་ཚུ་གི་དོན་ལུ་ འཐོབ་ཚུགས།

## ཟམ་པའི་དངུལ་འབབ་ཀྱི་བྱུང་བ།

ཟམ་གྱི་ལམ་ཚུ་ བཏོན་གཏང་ནི་ `RecordBridgeReceipt` བཀོད་རྒྱ་བརྒྱུད་དེ་ ཡིག་དཔར་རྐྱབས་ཡོདཔ་ཨིན། བཀོད་རྒྱ་འདི་ལག་ལེན་འཐབ་དོ།
`BridgeReceipt` གིས་ བྱུང་རིམ་གུ་ `DataEvent::Bridge(BridgeEvent::Emitted)` ཅིག་བཟོཝ་ཨིན།
རྒྱུན་སྤེལ་, སྔ་གོང་ནང་བསྐྱོད་རྐྱངམ་ཅིག་གི་ སྲབ་ཐག་འདི་ བསྐྱར་ལོག་འབད་དོ། CLI `iroha bridge emit-receipt` གྲོགས་རམ་འབད་མི་གིས་ ཕུལ་ཡོདཔ་ཨིན།
དེ་འབདཝ་ལས་ ཟུར་ཐོར་ཚུ་གིས་ འོང་འབབ་ཚུ་ བཟའ་སྤྱོད་འབད་བཏུབ་པའི་ བཀོད་རྒྱ་ཡིག་དཔར་རྐྱབས།

## ཕྱི་ཕྱོགས་བདེན་དཔྱད་ཀྱི་རི་མོ་ (ICS)

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