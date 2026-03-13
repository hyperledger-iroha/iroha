---
lang: ka
direction: ltr
source: docs/source/bridge_proofs.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 65aff839e8970e96edb07dfb9655cb4e79f56d1d885b7782647f5dc8f328027b
source_last_modified: "2025-12-29T18:16:35.921274+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# ხიდის მტკიცებულებები

ხიდის მტკიცებულების წარდგენები გადის სტანდარტული ინსტრუქციის გზაზე (`SubmitBridgeProof`) და დაფიქსირდა მტკიცებულების რეესტრში დამოწმებული სტატუსით. მიმდინარე ზედაპირი მოიცავს ICS-ის სტილის Merkle-ის მტკიცებულებებს და გამჭვირვალე-ZK დატვირთვას დამაგრებული შეკავებითა და მანიფესტური შეკვრით.

## მიღების წესები

- დიაპაზონები უნდა იყოს შეკვეთილი/არა ცარიელი და პატივი სცეს `zk.bridge_proof_max_range_len` (0 გამორთავს თავსახურს).
- სურვილისამებრ სიმაღლის ფანჯრები უარყოფენ მოძველებულ/მომავალ მტკიცებულებებს: `zk.bridge_proof_max_past_age_blocks` და `zk.bridge_proof_max_future_drift_blocks` იზომება ბლოკის სიმაღლის მიხედვით, რომელიც შთანთქავს მტკიცებულებას (0 გამორთავს დაცვას).
- ხიდის მტკიცებულებები შეიძლება არ ემთხვეოდეს არსებულ მტკიცებულებას იმავე უკანა ნაწილისთვის (დამაგრებული მტკიცებულებები შენარჩუნებულია და ბლოკავს გადახურვებს).
- მანიფესტის ჰეშები არ უნდა იყოს ნულოვანი; ტვირთამწეობა იფარება `zk.max_proof_size_bytes`-ით.
- ICS payloads პატივს სცემს კონფიგურირებულ Merkle-ს სიღრმის თავს და ამოწმებს გზას დეკლარირებული ჰეშის ფუნქციის გამოყენებით; გამჭვირვალე ტვირთამწეობამ უნდა გამოაცხადოს არაცარიელი საფონდო ლეიბლი.
- დამაგრებული მტკიცებულებები თავისუფლდება შეკავების მორთვისაგან; დაუმაგრებელი მტკიცებულებები კვლავ პატივს სცემს გლობალურ `zk.proof_history_cap`/grace/batch პარამეტრებს.

## Torii API ზედაპირი

- `GET /v2/zk/proofs` და `GET /v2/zk/proofs/count` იღებენ ხიდის შემეცნებით ფილტრებს:
  - `bridge_only=true` აბრუნებს მხოლოდ ხიდის მტკიცებულებებს.
  - `bridge_pinned_only=true` ვიწროვდება დამაგრებული ხიდის მტკიცებულებამდე.
  - `bridge_start_from_height` / `bridge_end_until_height` დაამაგრეთ ხიდის დიაპაზონის ფანჯარა.
- `GET /v2/zk/proof/{backend}/{hash}` აბრუნებს ხიდის მეტამონაცემებს (დიაპაზონი, მანიფესტის ჰეში, დატვირთვის შეჯამება) მტკიცებულების id/სტატუსის/VK აკინძებთან ერთად.
- სრული Norito მტკიცებულება ჩანაწერი (მათ შორის, დატვირთვის ბაიტი) ხელმისაწვდომი რჩება `GET /v2/proofs/{proof_id}`-ის მეშვეობით კვანძების გარეშე შემმოწმებლებისთვის.

## ხიდის მიღების ღონისძიებები

ხიდის ზოლები გამოსცემს აკრეფილ ქვითრებს `RecordBridgeReceipt` ინსტრუქციის მეშვეობით. ამ ინსტრუქციის შესრულება
ჩაწერს `BridgeReceipt` დატვირთვას და გამოსცემს `DataEvent::Bridge(BridgeEvent::Emitted)` ღონისძიებაზე
ნაკადი, ჩაანაცვლა მხოლოდ ჟურნალის წინა ნამუშევარი. CLI `iroha bridge emit-receipt` დამხმარე წარადგენს
აკრეფილი ინსტრუქცია, რათა ინდექსერებმა შეძლონ ქვითრების დეტერმინისტულად გამოყენება.

## გარე დადასტურების ესკიზი (ICS)

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