---
lang: hy
direction: ltr
source: docs/source/bridge_proofs.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 65aff839e8970e96edb07dfb9655cb4e79f56d1d885b7782647f5dc8f328027b
source_last_modified: "2025-12-29T18:16:35.921274+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Կամուրջի ապացույցներ

Կամուրջի ապացույցների ներկայացումները անցնում են ստանդարտ հրահանգների ուղով (`SubmitBridgeProof`) և հաստատված կարգավիճակով տեղավորվում ապացույցների գրանցամատյանում: Ընթացիկ մակերեսը ծածկում է ICS-ի ոճի Merkle proof-ները և թափանցիկ-ZK օգտակար բեռները՝ ամրացված պահմամբ և բացահայտ կապով:

## Ընդունման կանոններ

- Շրջանակները պետք է պատվիրված/ոչ դատարկ լինեն և հարգեն `zk.bridge_proof_max_range_len` (0-ն անջատում է գլխարկը):
- Ընտրովի բարձրության պատուհանները մերժում են հնացած/ապագա ապացույցները. `zk.bridge_proof_max_past_age_blocks` և `zk.bridge_proof_max_future_drift_blocks` չափվում են այն բլոկի բարձրության համեմատ, որը կլանում է ապացույցը (0-ն անջատում է պաշտպանիչ բազրիքները):
- Կամուրջի ապացույցները չեն կարող համընկնել գոյություն ունեցող ապացույցների հետ նույն հետևի համար (կապված ապացույցները պահպանվում են և արգելափակում են համընկնումները):
- Մանիֆեստի հեշերը պետք է լինեն ոչ զրոյական; ծանրաբեռնվածության չափը սահմանվում է `zk.max_proof_size_bytes`-ով:
- ICS օգտակար բեռները հարգում են կազմաձևված Merkle խորության գլխարկը և ստուգում են ուղին՝ օգտագործելով հայտարարված հեշ ֆունկցիան; թափանցիկ օգտակար բեռները պետք է հայտարարեն ոչ դատարկ հետնամասի պիտակ:
- Ամրացված ապացույցները ազատված են պահպանման էտումից. չամրացված ապացույցները դեռևս հարգում են `zk.proof_history_cap`/grace/խմբաքանակի գլոբալ կարգավորումները:

## Torii API մակերես

- `GET /v2/zk/proofs` և `GET /v2/zk/proofs/count` ընդունում են կամուրջների մասին տեղեկացված զտիչներ.
  - `bridge_only=true`-ը վերադարձնում է միայն կամրջի ապացույցները:
  - `bridge_pinned_only=true`-ը նեղանում է մինչև ամրացված կամուրջները:
  - `bridge_start_from_height` / `bridge_end_until_height` սեղմել կամրջի միջակայքի պատուհանը:
- `GET /v2/zk/proof/{backend}/{hash}`-ը վերադարձնում է կամուրջի մետատվյալները (միջակայք, մանիֆեստի հեշ, օգտակար բեռնվածքի ամփոփում) ապացուցման id/status/VK կապերի հետ մեկտեղ:
- Norito ամբողջական ապացույցի գրառումը (ներառյալ օգտակար բայթերը) մնում է հասանելի `GET /v2/proofs/{proof_id}`-ի միջոցով՝ անջատված հաստատիչների համար:

## Կամուրջի ստացման իրադարձություններ

Կամուրջի ուղիները տպագրված անդորրագրեր են հաղորդում `RecordBridgeReceipt` հրահանգի միջոցով: Այս հրահանգի կատարումը
գրանցում է `BridgeReceipt` օգտակար բեռ և թողարկում `DataEvent::Bridge(BridgeEvent::Emitted)` միջոցառմանը
հոսք՝ փոխարինելով միայն գրանցամատյանում պարունակվող նախկին կոճակը: CLI `iroha bridge emit-receipt` օգնականը ներկայացնում է
մուտքագրված հրահանգ, որպեսզի ինդեքսավորողները կարողանան դետերմինիստական կերպով սպառել անդորրագրերը:

## Արտաքին հաստատման ուրվագիծ (ICS)

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