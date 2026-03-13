---
lang: az
direction: ltr
source: docs/source/bridge_proofs.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 65aff839e8970e96edb07dfb9655cb4e79f56d1d885b7782647f5dc8f328027b
source_last_modified: "2025-12-29T18:16:35.921274+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Körpü sübutları

Körpü sübut təqdimatları standart təlimat yolundan (`SubmitBridgeProof`) keçir və təsdiqlənmiş statusla sübut reyestrinə düşür. Cari səth ICS tipli Merkle sübutlarını və bərkidilmiş saxlama və manifest bağlama ilə şəffaf ZK yüklərini əhatə edir.

## Qəbul qaydaları

- Aralıqlar sıralanmalı/boş olmamalıdır və `zk.bridge_proof_max_range_len`-ə hörmət edilməlidir (0 qapağı söndürür).
- İsteğe bağlı hündürlüklü pəncərələr köhnəlmiş/gələcək sübutları rədd edir: `zk.bridge_proof_max_past_age_blocks` və `zk.bridge_proof_max_future_drift_blocks` sübutu qəbul edən blokun hündürlüyü ilə ölçülür (0 qoruyucu barmaqlıqları söndürür).
- Körpü sübutları eyni arxa tərəf üçün mövcud sübutla üst-üstə düşə bilməz (bağlı sübutlar qorunur və üst-üstə düşür).
- Manifest xeşləri sıfırdan fərqli olmalıdır; faydalı yüklərin ölçüsü `zk.max_proof_size_bytes` ilə məhdudlaşdırılır.
- ICS yükləri konfiqurasiya edilmiş Merkle dərinlik qapağına uyğun gəlir və elan edilmiş hash funksiyasından istifadə edərək yolu yoxlayır; şəffaf faydalı yüklər boş olmayan backend etiketi elan etməlidir.
- bərkidilmiş sübutlar saxlama budamasından azaddır; bağlanmamış sübutlar hələ də qlobal `zk.proof_history_cap`/grace/batch parametrlərinə hörmət edir.

## Torii API səthi

- `GET /v2/zk/proofs` və `GET /v2/zk/proofs/count` körpüdən xəbərdar filtrləri qəbul edir:
  - `bridge_only=true` yalnız körpü sübutlarını qaytarır.
  - `bridge_pinned_only=true` bərkidilmiş körpü sübutlarına qədər daralır.
  - `bridge_start_from_height` / `bridge_end_until_height` körpü diapazonu pəncərəsini sıxın.
- `GET /v2/zk/proof/{backend}/{hash}` sübut id/status/VK bağlamaları ilə yanaşı körpü metadatasını (aralıq, manifest hash, faydalı yükün xülasəsi) qaytarır.
- Tam Norito sübut qeydi (faydalı yük baytları daxil olmaqla) node yoxlayıcılar üçün `GET /v2/proofs/{proof_id}` vasitəsilə mövcuddur.

## Körpü qəbz hadisələri

Körpü zolaqları `RecordBridgeReceipt` təlimatı vasitəsilə yazılmış qəbzlər yayır. Bu göstərişin icrası
`BridgeReceipt` faydalı yükünü qeyd edir və hadisə zamanı `DataEvent::Bridge(BridgeEvent::Emitted)` yayır
axın, əvvəlki yalnız giriş stubunu əvəz edir. CLI `iroha bridge emit-receipt` köməkçisi təqdim edir
indeksləşdiricilərin qəbzləri deterministik şəkildə istehlak edə bilməsi üçün yazılmış təlimat.

## Xarici yoxlama eskizi (ICS)

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