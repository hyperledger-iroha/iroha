---
lang: ba
direction: ltr
source: docs/source/bridge_proofs.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 65aff839e8970e96edb07dfb9655cb4e79f56d1d885b7782647f5dc8f328027b
source_last_modified: "2025-12-29T18:16:35.921274+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Күпер .

Күпер иҫбатлаусы тапшырыуҙар стандарт инструкция юлы аша үтә (`SubmitBridgeProof`) һәм раҫланған статуслы иҫбатлау реестрында ер. Ток өҫтө ҡаплай ICS-стиль Merkle иҫбатлау һәм үтә күренмәле-ZK файҙалы йөктәр менән пинированный һаҡлау һәм асыҡ бәйләү.

## Ҡабул итеү ҡағиҙәләре

- Диапазондарҙы заказ бирергә/бушҡа һәм хөрмәт итергә кәрәк `zk.bridge_proof_max_range_len` (0 ҡапҡасты өҙөп).
- Опциональ бейеклектәге тәҙрәләр иҫке/киләсәктә дәлилдәрҙе кире ҡаға: `zk.bridge_proof_max_past_age_blocks` һәм `zk.bridge_proof_max_future_drift_blocks` блок бейеклегенә ҡаршы үлсәнә, улар иҫбатлауҙы ингестировать (0 ҡоршауҙарҙы өҙөп).
- Күпер дәлилдәре шул уҡ бэкэнд өсөн булған дәлилде ҡапламауы ла ихтимал (пеннированный дәлилдәр һаҡлана һәм ҡапланыуҙы блоклай).
- Манифест хештары нуль булмаған булырға тейеш; файҙалы йөктәр ҙурлыҡтағы `zk.max_proof_size_bytes` менән ҡапланған.
- ICS файҙалы йөкләмәләр настроить конфигурацияланған Merkle тәрәнлеге ҡапҡасы һәм юлды раҫлау ҡулланып иғлан ителгән хеш функцияһы; үтә күренмәле файҙалы йөктәр иғлан итергә тейеш, буш булмаған бэкэнд ярлыҡ.
- Пиннированный дәлилдәр һаҡлауҙан азат ителә; unpinned дәлилдәр һаман да донъя `zk.proof_history_cap`/рәхмәт/партия параметрҙарын хөрмәт итә.

## Torii API өҫтө

- `GET /v1/zk/proofs` һәм `GET /v1/zk/proofs/count` күпер-аңлы фильтрҙар ҡабул итә:
  - `bridge_only=true` күперле дәлилдәрҙе генә ҡайтара.
  - `bridge_pinned_only=true` тарайтыу өсөн күперле дәлилдәрҙе ҡыҫтырылған.
  - `bridge_start_from_height` / `bridge_end_until_height` күпер диапазоны тәҙрәһен ҡыҫҡыс.
- `GET /v1/zk/proof/{backend}/{hash}` ҡайтарыу күпер метамағлүмәттәр (диапазон, асыҡ хеш, файҙалы йөк резюме) менән бер рәттән иҫбатлау id/статус/ВК бәйләүҙәр.
- Тулы Norito дәлил яҙмаһы (шул иҫәптән файҙалы йөк байттары) `GET /v1/proofs/{proof_id}` аша төйөндән тыш тикшерелеүселәр өсөн мөмкин.

## Күпер квитанция саралары

Күпер һыҙаттары `RecordBridgeReceipt` инструкцияһы аша тип квитанциялар сығара. Был күрһәтмә башҡарыу
18NI000000018X файҙалы йөкләмәһен теркәй һәм сарала `DataEvent::Bridge(BridgeEvent::Emitted)` сығарыла
ағым, алмаштырыу өсөн алдан лог-тик стаб. CLI `iroha bridge emit-receipt` ярҙамсыһы тапшыра
тип инструкция шулай индексерҙар ҡуллана ала квитанциялар детерминистик.

## Тышҡы тикшерелгән эскиз (ИКС)

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