---
lang: kk
direction: ltr
source: docs/source/bridge_proofs.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 65aff839e8970e96edb07dfb9655cb4e79f56d1d885b7782647f5dc8f328027b
source_last_modified: "2025-12-29T18:16:35.921274+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Көпір дәлелдері

Көпірді растайтын хабарлар стандартты нұсқау жолы (`SubmitBridgeProof`) арқылы өтеді және расталған күйі бар дәлелдеу тізіліміне түседі. Ағымдағы бет ICS стиліндегі Merkle дәлелдерін және бекітілген ұстауы және манифестпен байланыстыруы бар мөлдір-ZK пайдалы жүктемелерін қамтиды.

## Қабылдау ережелері

- Ауқым реттелген/бос емес және `zk.bridge_proof_max_range_len` (0 қақпақты өшіреді) құрметтелуі керек.
- Қосымша биіктік терезелері ескірген/болашақ дәлелдемелерді қабылдамайды: `zk.bridge_proof_max_past_age_blocks` және `zk.bridge_proof_max_future_drift_blocks` дәлелді қабылдайтын блок биіктігімен өлшенеді (0 қоршауларды өшіреді).
- Көпір дәлелдері бір сервер үшін бар дәлелдемені қабаттастырмауы мүмкін (қақталған дәлелдер сақталады және қабаттасуларды блоктайды).
- Манифест хэштері нөлден өзгеше болуы керек; пайдалы жүктемелер өлшемі `zk.max_proof_size_bytes` арқылы шектелген.
- ICS пайдалы жүктемелері конфигурацияланған Merkle тереңдігінің қақпағын құрметтейді және жарияланған хэш функциясы арқылы жолды тексереді; мөлдір пайдалы жүктемелер бос емес сервер белгісін жариялауы керек.
- Бекітілген дәлелдер ұстап тұратын кесуден босатылады; бекітілмеген дәлелдер әлі де жаһандық `zk.proof_history_cap`/grace/пакет параметрлерін құрметтейді.

## Torii API беті

- `GET /v2/zk/proofs` және `GET /v2/zk/proofs/count` көпірден хабардар сүзгілерді қабылдайды:
  - `bridge_only=true` тек көпір дәлелдерін қайтарады.
  - `bridge_pinned_only=true` бекітілген көпір дәлелдеріне дейін тарылтады.
  - `bridge_start_from_height` / `bridge_end_until_height` көпір ауқымының терезесін қысыңыз.
- `GET /v2/zk/proof/{backend}/{hash}` дәлелдеу идентификаторы/күй/VK байланыстарымен қатар көпір метадеректерін (диапазон, манифест хэші, пайдалы жүктеме туралы қорытынды) қайтарады.
- Толық Norito дәлелдеме жазбасы (пайдалы жүктеме байттарын қоса) түйіннен тыс тексерушілер үшін `GET /v2/proofs/{proof_id}` арқылы қолжетімді болып қалады.

## Көпір алу оқиғалары

Көпір жолдары `RecordBridgeReceipt` нұсқауы арқылы терілген түбіртектерді шығарады. Осы нұсқауды орындау
`BridgeReceipt` пайдалы жүктемесін жазады және оқиғада `DataEvent::Bridge(BridgeEvent::Emitted)` шығарады
ағыны, алдыңғы тек журналға арналған түйінді ауыстыру. CLI `iroha bridge emit-receipt` көмекшісі жібереді
индекстеушілер түбіртектерді детерминистикалық түрде тұтынуы үшін терілген нұсқаулық.

## Сыртқы тексеру эскизі (ICS)

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