---
lang: es
direction: ltr
source: docs/source/bridge_proofs.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f3f6049cbf3aa135e35e4cf06967993c11c1c571ca97dd11c469142dd620be77
source_last_modified: "2025-12-11T23:36:13.998930+00:00"
translation_last_reviewed: 2026-01-01
---

# Pruebas de bridge

Las presentaciones de pruebas de bridge pasan por la ruta estandar de instrucciones (`SubmitBridgeProof`) y llegan al registro de pruebas con estado verificado. La superficie actual cubre pruebas Merkle estilo ICS y payloads transparent-ZK con retencion fija y enlace a manifest.

## Reglas de aceptacion

- Los rangos deben estar ordenados/no vacios y respetar `zk.bridge_proof_max_range_len` (0 desactiva el limite).
- Ventanas de altura opcionales rechazan pruebas antiguas/futuras: `zk.bridge_proof_max_past_age_blocks` y `zk.bridge_proof_max_future_drift_blocks` se miden contra la altura del bloque que ingiere la prueba (0 desactiva las guardas).
- Las pruebas de bridge no pueden solaparse con una prueba existente para el mismo backend (las pruebas fijadas se preservan y bloquean solapes).
- Los hashes de manifest no deben ser cero; los payloads estan limitados por `zk.max_proof_size_bytes`.
- Los payloads ICS respetan el limite de profundidad Merkle configurado y verifican la ruta usando la funcion hash declarada; los payloads transparentes deben declarar una etiqueta de backend no vacia.
- Las pruebas fijadas estan exentas de la poda por retencion; las no fijadas siguen respetando `zk.proof_history_cap`/gracia/lotes globales.

## Superficie de API Torii

- `GET /v1/zk/proofs` y `GET /v1/zk/proofs/count` aceptan filtros con conocimiento de bridge:
  - `bridge_only=true` devuelve solo pruebas de bridge.
  - `bridge_pinned_only=true` limita a pruebas de bridge fijadas.
  - `bridge_start_from_height` / `bridge_end_until_height` acotan la ventana de rango del bridge.
- `GET /v1/zk/proof/{backend}/{hash}` devuelve metadatos de bridge (rango, hash de manifest, resumen de payload) junto con el id/estado de la prueba y enlaces de VK.
- El registro completo de pruebas Norito (incluyendo bytes del payload) sigue disponible via `GET /v1/proofs/{proof_id}` para verificadores fuera del nodo.

## Eventos de recibos de bridge

Las lanes de bridge emiten recibos tipados mediante la instruccion `RecordBridgeReceipt`. Al ejecutar esta instruccion se registra un payload `BridgeReceipt` y se emite `DataEvent::Bridge(BridgeEvent::Emitted)` en el stream de eventos, reemplazando el stub previo solo de log. El helper de CLI `iroha bridge emit-receipt` envia la instruccion tipada para que los indexadores puedan consumir recibos de forma determinista.

## Bosquejo de verificacion externa (ICS)

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
