---
lang: pt
direction: ltr
source: docs/source/bridge_proofs.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f3f6049cbf3aa135e35e4cf06967993c11c1c571ca97dd11c469142dd620be77
source_last_modified: "2025-12-11T23:36:13.998930+00:00"
translation_last_reviewed: 2026-01-01
---

# Provas de bridge

As submissoes de provas de bridge seguem o caminho padrao de instrucao (`SubmitBridgeProof`) e chegam ao registro de provas com status verificado. A superficie atual cobre provas Merkle estilo ICS e payloads transparent-ZK com retencao fixa e vinculo a manifest.

## Regras de aceitacao

- Os intervalos devem estar ordenados/nao vazios e respeitar `zk.bridge_proof_max_range_len` (0 desativa o limite).
- Janelas de altura opcionais rejeitam provas antigas/futuras: `zk.bridge_proof_max_past_age_blocks` e `zk.bridge_proof_max_future_drift_blocks` sao medidos contra a altura do bloco que ingere a prova (0 desativa as guardas).
- Provas de bridge nao podem sobrepor uma prova existente para o mesmo backend (provas fixadas sao preservadas e bloqueiam sobreposicoes).
- Hashes de manifest nao podem ser zero; payloads sao limitados por `zk.max_proof_size_bytes`.
- Payloads ICS respeitam o limite de profundidade Merkle configurado e verificam o caminho usando a funcao hash declarada; payloads transparentes devem declarar uma etiqueta de backend nao vazia.
- Provas fixadas estao isentas da poda por retencao; provas nao fixadas ainda respeitam `zk.proof_history_cap`/grace/batch globais.

## Superficie de API Torii

- `GET /v2/zk/proofs` e `GET /v2/zk/proofs/count` aceitam filtros conscientes de bridge:
  - `bridge_only=true` retorna apenas provas de bridge.
  - `bridge_pinned_only=true` restringe a provas de bridge fixadas.
  - `bridge_start_from_height` / `bridge_end_until_height` limitam a janela de intervalo do bridge.
- `GET /v2/zk/proof/{backend}/{hash}` retorna metadados de bridge (intervalo, hash do manifest, resumo do payload) junto com o id/status da prova e os vinculos de VK.
- O registro completo de provas Norito (incluindo bytes do payload) permanece disponivel via `GET /v2/proofs/{proof_id}` para verificadores fora do nodo.

## Eventos de recibo de bridge

As lanes de bridge emitem recibos tipados via a instrucao `RecordBridgeReceipt`. Ao executar essa instrucao, um payload `BridgeReceipt` e registrado e `DataEvent::Bridge(BridgeEvent::Emitted)` e emitido no stream de eventos, substituindo o stub anterior apenas de logs. O helper de CLI `iroha bridge emit-receipt` envia a instrucao tipada para que indexadores possam consumir recibos de forma determinista.

## Esboco de verificacao externa (ICS)

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
