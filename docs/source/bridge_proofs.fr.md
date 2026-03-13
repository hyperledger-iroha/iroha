---
lang: fr
direction: ltr
source: docs/source/bridge_proofs.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f3f6049cbf3aa135e35e4cf06967993c11c1c571ca97dd11c469142dd620be77
source_last_modified: "2025-12-11T23:36:13.998930+00:00"
translation_last_reviewed: 2026-01-01
---

# Preuves de bridge

Les soumissions de preuves de bridge passent par le chemin d instruction standard (`SubmitBridgeProof`) et arrivent dans le registre des preuves avec un statut verifie. La surface actuelle couvre les preuves Merkle de type ICS et les payloads transparent-ZK avec retention figee et liaison au manifest.

## Regles d acceptation

- Les plages doivent etre ordonnees/non vides et respecter `zk.bridge_proof_max_range_len` (0 desactive le plafond).
- Les fenetres de hauteur optionnelles rejettent les preuves obsoletes/futures: `zk.bridge_proof_max_past_age_blocks` et `zk.bridge_proof_max_future_drift_blocks` sont mesurees par rapport a la hauteur de bloc qui ingere la preuve (0 desactive les garde-fous).
- Les preuves de bridge ne peuvent pas chevaucher une preuve existante pour le meme backend (les preuves epinglees sont preservees et bloquent les chevauchements).
- Les hashes de manifest ne doivent pas etre nuls; les payloads sont limites en taille par `zk.max_proof_size_bytes`.
- Les payloads ICS respectent le plafond de profondeur Merkle configure et verifient le chemin avec la fonction de hash declaree; les payloads transparents doivent declarer un label backend non vide.
- Les preuves epinglees sont exemptes de la purge par retention; les preuves non epinglees respectent toujours `zk.proof_history_cap`/grace/batch globaux.

## Surface API Torii

- `GET /v2/zk/proofs` et `GET /v2/zk/proofs/count` acceptent des filtres aware des bridges:
  - `bridge_only=true` renvoie uniquement les preuves de bridge.
  - `bridge_pinned_only=true` limite aux preuves de bridge epinglees.
  - `bridge_start_from_height` / `bridge_end_until_height` bornent la fenetre de plage bridge.
- `GET /v2/zk/proof/{backend}/{hash}` renvoie les metadonnees bridge (plage, hash de manifest, resume du payload) ainsi que l id/statut de preuve et les liaisons VK.
- L enregistrement Norito complet (y compris les octets du payload) reste disponible via `GET /v2/proofs/{proof_id}` pour les verificateurs hors noeud.

## Evenements de recu de bridge

Les lanes bridge emettent des recus types via l instruction `RecordBridgeReceipt`. L execution de cette instruction enregistre un payload `BridgeReceipt` et emet `DataEvent::Bridge(BridgeEvent::Emitted)` sur le flux d evenements, remplacant l ancien stub log-only. Le helper CLI `iroha bridge emit-receipt` soumet l instruction typee afin que les indexeurs puissent consommer les recus de maniere deterministe.

## Esquisse de verification externe (ICS)

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
