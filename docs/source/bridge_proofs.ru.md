---
lang: ru
direction: ltr
source: docs/source/bridge_proofs.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f3f6049cbf3aa135e35e4cf06967993c11c1c571ca97dd11c469142dd620be77
source_last_modified: "2025-12-11T23:36:13.998930+00:00"
translation_last_reviewed: 2026-01-01
---

# Bridge proofs

Отправка bridge proof проходит по стандартному пути инструкций (`SubmitBridgeProof`) и попадает в реестр доказательств со статусом verified. Текущая поверхность покрывает Merkle proofs в стиле ICS и transparent-ZK payloads с фиксированным удержанием и привязкой к manifest.

## Правила принятия

- Диапазоны должны быть упорядочены/не пусты и соблюдать `zk.bridge_proof_max_range_len` (0 отключает лимит).
- Опциональные окна высоты отклоняют устаревшие/будущие доказательства: `zk.bridge_proof_max_past_age_blocks` и `zk.bridge_proof_max_future_drift_blocks` измеряются относительно высоты блока, который принимает proof (0 отключает защиту).
- Bridge proofs не могут перекрываться с существующим proof для того же backend (pinned proofs сохраняются и блокируют пересечения).
- Manifest hashes не должны быть нулевыми; размер payload ограничен `zk.max_proof_size_bytes`.
- ICS payloads соблюдают настроенный лимит глубины Merkle и проверяют путь с использованием заявленной хеш-функции; transparent payloads должны объявлять непустую метку backend.
- Pinned proofs освобождены от удержания по сроку; unpinned proofs все еще соблюдают глобальные настройки `zk.proof_history_cap`/grace/batch.

## Поверхность Torii API

- `GET /v1/zk/proofs` и `GET /v1/zk/proofs/count` принимают фильтры, учитывающие bridge:
  - `bridge_only=true` возвращает только bridge proofs.
  - `bridge_pinned_only=true` ограничивает pinned bridge proofs.
  - `bridge_start_from_height` / `bridge_end_until_height` ограничивают окно диапазона bridge.
- `GET /v1/zk/proof/{backend}/{hash}` возвращает метаданные bridge (диапазон, manifest hash, summary payload) вместе с id/статусом proof и VK bindings.
- Полная запись Norito proof (включая bytes payload) остается доступной через `GET /v1/proofs/{proof_id}` для проверяющих вне узла.

## События bridge receipt

Bridge lanes выпускают типизированные receipts через инструкцию `RecordBridgeReceipt`. Выполнение этой инструкции записывает payload `BridgeReceipt` и публикует `DataEvent::Bridge(BridgeEvent::Emitted)` в потоке событий, заменяя прежний log-only stub. CLI helper `iroha bridge emit-receipt` отправляет типизированную инструкцию, чтобы индексаторы могли детерминированно потреблять receipts.

## Эскиз внешней проверки (ICS)

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
