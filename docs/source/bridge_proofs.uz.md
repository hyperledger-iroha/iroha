---
lang: uz
direction: ltr
source: docs/source/bridge_proofs.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 65aff839e8970e96edb07dfb9655cb4e79f56d1d885b7782647f5dc8f328027b
source_last_modified: "2025-12-29T18:16:35.921274+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Ko'prik dalillari

Ko'prikni tasdiqlovchi taqdimotlar standart ko'rsatmalar yo'lidan (`SubmitBridgeProof`) o'tadi va tasdiqlangan maqomga ega bo'lgan dalillar reestriga tushadi. Joriy sirt ICS uslubidagi Merkle isbotlari va mahkamlangan ushlab turish va manifest bog'lash bilan shaffof ZK foydali yuklarni qamrab oladi.

## Qabul qilish qoidalari

- Diapazonlar tartibga solinishi/bo'sh bo'lmasligi va `zk.bridge_proof_max_range_len` ga hurmat ko'rsatishi kerak (0 qopqoqni o'chiradi).
- Ixtiyoriy balandlik oynalari eskirgan/kelajakdagi isbotlarni rad etadi: `zk.bridge_proof_max_past_age_blocks` va `zk.bridge_proof_max_future_drift_blocks` isbotni qabul qiladigan blok balandligi bilan o'lchanadi (0 qo'riqchi panjaralarni o'chiradi).
- Ko'prik isbotlari bir xil orqa qism uchun mavjud isbot bilan bir-biriga mos kelmasligi mumkin (qadamlangan dalillar saqlanib qoladi va bir-biriga yopishib qolmaydi).
- Manifest xeshlari nolga teng bo'lmasligi kerak; foydali yuklarning o'lchamlari `zk.max_proof_size_bytes` bilan cheklangan.
- ICS foydali yuklari sozlangan Merkle chuqurligi qopqog'ini hurmat qiladi va e'lon qilingan xesh funktsiyasidan foydalangan holda yo'lni tasdiqlaydi; shaffof foydali yuklar bo'sh bo'lmagan backend yorlig'ini e'lon qilishi kerak.
- mahkamlangan isbotlar ushlab turuvchi budamalardan ozod qilinadi; Ochilmagan dalillar hali ham global `zk.proof_history_cap`/grace/paket sozlamalarini hurmat qiladi.

## Torii API yuzasi

- `GET /v2/zk/proofs` va `GET /v2/zk/proofs/count` ko'prikdan xabardor filtrlarni qabul qiladi:
  - `bridge_only=true` faqat ko'prik dalillarini qaytaradi.
  - `bridge_pinned_only=true` mahkamlangan ko'prik isbotlariga torayadi.
  - `bridge_start_from_height` / `bridge_end_until_height` ko'prik diapazoni oynasini mahkamlang.
- `GET /v2/zk/proof/{backend}/{hash}` ko'prik metama'lumotlarini (diapazon, manifest xesh, foydali yuk xulosasi) isbot identifikatori/status/VK ulanishlari bilan birga qaytaradi.
- To'liq Norito isbot yozuvi (shu jumladan foydali yuk baytlari) tugundan tashqari tekshiruvchilar uchun `GET /v2/proofs/{proof_id}` orqali mavjud bo'lib qoladi.

## Ko'prik olish hodisalari

Ko'prik yo'llari `RecordBridgeReceipt` ko'rsatmasi orqali yozilgan kvitansiyalarni chiqaradi. Ushbu ko'rsatmani bajarish
`BridgeReceipt` foydali yukini qayd qiladi va voqeada `DataEvent::Bridge(BridgeEvent::Emitted)` chiqaradi
oqim, oldingi faqat jurnal stubini almashtirish. CLI `iroha bridge emit-receipt` yordamchisi
terilgan ko'rsatma, shuning uchun indekserlar kvitansiyalarni aniq iste'mol qilishi mumkin.

## Tashqi tekshirish eskizi (ICS)

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