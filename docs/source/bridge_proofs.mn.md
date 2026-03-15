---
lang: mn
direction: ltr
source: docs/source/bridge_proofs.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 65aff839e8970e96edb07dfb9655cb4e79f56d1d885b7782647f5dc8f328027b
source_last_modified: "2025-12-29T18:16:35.921274+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Гүүрний баталгаа

Гүүр нотлох баримтууд нь стандарт зааврын замаар (`SubmitBridgeProof`) дамжиж, баталгаажуулсан статустай баталгааны бүртгэлд ордог. Одоогийн гадаргуу нь ICS загварын Merkle proofs ба ил тод ZK ачааллыг бэхэлсэн хадгалалт, манифест холболттой хамардаг.

## Хүлээн авах дүрэм

- Хүрээ нь дараалсан/хоосон биш байх ёстой бөгөөд `zk.bridge_proof_max_range_len` (0 нь хязгаарыг идэвхгүй болгодог).
- Нэмэлт өндөртэй цонхнууд нь хуучирсан/ирээдүйн баталгааг үгүйсгэдэг: `zk.bridge_proof_max_past_age_blocks` болон `zk.bridge_proof_max_future_drift_blocks` нь нотолгоог шингээж буй блокийн өндрөөр хэмжигддэг (0 нь хамгаалалтын хашлагыг идэвхгүй болгодог).
- Гүүрний нотлох баримтууд нь ижил арын хэсэгт байгаа нотлох баримттай давхцахгүй байж болно (заасан нотлох баримтууд хадгалагдаж, давхцлыг блоклодог).
- Манифест хэш нь тэгээс өөр байх ёстой; Ачааллын хэмжээ нь `zk.max_proof_size_bytes`-ээр хязгаарлагддаг.
- ICS-ийн ачаалал нь тохируулсан Merkle гүний хязгаарыг дагаж мөрдөж, зарласан хэш функцийг ашиглан замыг баталгаажуулдаг; ил тод ачаалал нь хоосон биш арын шошгыг зарлах ёстой.
- Хадгалагдсан нотлох баримтууд нь тайрахаас чөлөөлөгдөнө; тогтоогдоогүй нотолгоо нь дэлхийн `zk.proof_history_cap`/grace/batch тохиргоог хүндэтгэсээр байна.

## Torii API гадаргуу

- `GET /v2/zk/proofs` болон `GET /v2/zk/proofs/count` нь гүүрийг мэддэг шүүлтүүрийг хүлээн авдаг:
  - `bridge_only=true` нь зөвхөн гүүрний баталгааг буцаана.
  - `bridge_pinned_only=true` нь бэхлэгдсэн гүүрний баталгаа хүртэл нарийсдаг.
  - `bridge_start_from_height` / `bridge_end_until_height` гүүрний хүрээний цонхыг хавчих.
- `GET /v2/zk/proof/{backend}/{hash}` нь нотлох id/status/VK холболтын зэрэгцээ гүүрний мета өгөгдлийг (муж, манифест хэш, ачааллын хураангуй) буцаана.
- Norito баталгаажуулалтын бүрэн бичлэг (ачааллын байтыг оруулаад) нь `GET /v2/proofs/{proof_id}`-ээр дамжуулан зангилаанаас гадуурх баталгаажуулагчдад боломжтой хэвээр байна.

## Баримт хүлээн авах гүүрний үйл явдал

Гүүрний замууд нь `RecordBridgeReceipt` заавраар бичигдсэн баримтыг гаргадаг. Энэ зааврыг биелүүлж байна
`BridgeReceipt` ачааллыг бүртгэж, үйл явдал дээр `DataEvent::Bridge(BridgeEvent::Emitted)` ялгаруулдаг
урсгал, өмнөх зөвхөн бүртгэлийн бүдүүвчийг солих. CLI `iroha bridge emit-receipt` туслах нь
бичсэн заавар нь индексжүүлэгчид төлбөрийн баримтыг тодорхой хэмжээгээр ашиглах боломжтой.

## Гадаад баталгаажуулалтын ноорог (ICS)

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