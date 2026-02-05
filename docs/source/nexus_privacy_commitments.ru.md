---
lang: ru
direction: ltr
source: docs/source/nexus_privacy_commitments.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e7ef8ab7f52ec333d3fb9686dd744e23a859058dc0dbb91cfafee1cb7d1452ac
source_last_modified: "2025-11-21T17:27:28.236084+00:00"
translation_last_reviewed: 2026-01-01
---

# Фреймворк приватных коммитментов и доказательств (NX-10)

> **Статус:** 🈴 Completed (NX-10)  
> **Owners:** Cryptography WG · Privacy WG · Nexus Core WG  
> **Связанный код:** [`crates/iroha_crypto/src/privacy.rs`](../../crates/iroha_crypto/src/privacy.rs)

NX-10 вводит общую поверхность коммитментов для приватных lanes. Каждый dataspace публикует
детерминированный дескриптор, который связывает Merkle корни или zk-SNARK схемы с
воспроизводимыми хешами. Глобальное кольцо Nexus может проверять cross-lane переводы и
доказательства конфиденциальности без bespoke парсеров.

## Цели и область

- Канонизировать идентификаторы коммитментов, чтобы governance manifests и SDK согласовывали
  числовой slot, используемый при admission.
- Поставить переиспользуемые helpers проверки (`iroha_crypto::privacy`), чтобы runtimes, Torii и
  off-chain аудиторы оценивали proofs единообразно.
- Привязать zk-SNARK proofs к каноническому digest verifying key и детерминированным кодировкам
  public inputs. Без ad-hoc transcripts.
- Документировать workflow registry/export, чтобы lane bundles и доказательства governance включали
  те же хеши, которые enforced runtime.

Вне области для этой записки: механика DA fan-out, relay messaging и plumbing settlement router.
См. `nexus_cross_lane.md` для этих слоев.

## Модель коммитментов lane

Registry хранит упорядоченный список `LanePrivacyCommitment` entries:

```rust
use iroha_crypto::privacy::{
    CommitmentScheme, LaneCommitmentId, LanePrivacyCommitment, MerkleCommitment, SnarkCircuit,
    SnarkCircuitId,
};

let id = LaneCommitmentId::new(1);
let merkle = LanePrivacyCommitment::merkle(
    id,
    MerkleCommitment::from_root_bytes(root_bytes, 16),
);

let snark = LanePrivacyCommitment::snark(
    LaneCommitmentId::new(2),
    SnarkCircuit::new(
        SnarkCircuitId::new(42),
        verifying_key_digest,
        statement_hash,
        proof_hash,
    ),
);
```

- **`LaneCommitmentId`** — стабильный 16-битный идентификатор, записываемый в lane manifests.
- **`CommitmentScheme`** — `Merkle` или `Snark`. Будущие варианты (например, bulletproofs) расширяют enum.
- **Copy semantics** — все дескрипторы реализуют `Copy`, чтобы configs переиспользовали их без heap churn.

Registry идет вместе с Nexus lane bundle (`scripts/nexus/lane_registry_bundle.py`). Когда governance
одобряет новый коммитмент, bundle обновляет и JSON manifest, и Norito overlay, который потребляет
admission.

### Схема manifest (`privacy_commitments`)

Lane manifests теперь предоставляют массив `privacy_commitments`. Каждый entry назначает ID и scheme и
включает параметры, специфичные для схемы:

```json
{
  "lane": "cbdc",
  "governance": "council",
  "privacy_commitments": [
    {
      "id": 1,
      "scheme": "merkle",
      "merkle": {
        "root": "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        "max_depth": 16
      }
    },
    {
      "id": 2,
      "scheme": "snark",
      "snark": {
        "circuit_id": 5,
        "verifying_key_digest": "0x...",
        "statement_hash": "0x...",
        "proof_hash": "0x..."
      }
    }
  ]
}
```

Registry bundler копирует manifest, записывает tuples `(id, scheme)` в `summary.json`, и CI
(`lane_registry_verify.py`) повторно парсит manifest, чтобы удостовериться, что summary совпадает
с содержимым на диске.

## Merkle коммитменты

`MerkleCommitment` фиксирует канонический `HashOf<MerkleTree<[u8;32]>>` и максимальную глубину
audit path. Операторы экспортируют root напрямую из prover или snapshot ledger; внутри registry
нет повторного хеширования.

Поток валидации:

```rust
use iroha_crypto::privacy::{LanePrivacyCommitment, MerkleWitness, PrivacyWitness};

let witness = MerkleWitness::from_leaf_bytes(leaf_bytes, proof);
LanePrivacyCommitment::merkle(id, commitment)
    .verify(PrivacyWitness::Merkle(witness))?;
```

- Proofs, превышающие `max_depth`, эмитируют `PrivacyError::MerkleProofExceedsDepth`.
- Audit paths используют утилиты `iroha_crypto::MerkleProof`, чтобы shielded pools и Nexus private
  lanes разделяли одну сериализацию.
- Hosts, которые ingest внешние pools, конвертируют shielded leaves через
  `MerkleTree::shielded_leaf_from_commitment` перед построением witness.

### Операционный чеклист

- Публиковать tuple `(id, root, depth)` в summary manifest lane и evidence bundle.
- Прикладывать Merkle inclusion proof для каждого cross-lane перевода к admission log; helper
  воспроизводит root byte-for-byte, так что аудит сравнивает только экспортированный hash.
- Отслеживать depth budgets в телеметрии (`nexus_privacy_commitments.merkle.depth_used`), чтобы
  алерты срабатывали до превышения настроенных max.

## zk-SNARK коммитменты

`SnarkCircuit` entries связывают четыре поля:

| Field | Description |
|-------|-------------|
| `circuit_id` | Идентификатор, контролируемый governance, для пары circuit/version. |
| `verifying_key_digest` | Blake3 хеш канонического verifying key (DER или Norito encoding). |
| `statement_hash` | Blake3 хеш канонического public-input encoding (Norito или Borsh). |
| `proof_hash` | Blake3 хеш `verifying_key_digest || proof_bytes`. |

Runtime helpers:

```rust
use iroha_crypto::privacy::{
    hash_proof, hash_public_inputs, LanePrivacyCommitment, PrivacyWitness, SnarkWitness,
};

let witness = SnarkWitness {
    public_inputs: encoded_inputs,
    proof: proof_bytes,
};

LanePrivacyCommitment::snark(id, circuit)
    .verify(PrivacyWitness::Snark(witness))?;
```

- `hash_public_inputs` и `hash_proof` живут в том же модуле; SDK должны вызывать их при генерации
  manifests или proofs, чтобы избежать format drift.
- Любое несовпадение между зафиксированным statement hash и представленными public inputs дает
  `PrivacyError::SnarkStatementMismatch`.
- Proof bytes должны уже быть в канонической сжатой форме (Groth16, Plonk и т. д.). Helper лишь
  проверяет hash binding; полноценная проверка кривой остается в сервисе prover/verifier.

### Evidence workflow

1. Экспортировать verifying key digest и proof hash в manifest lane bundle.
2. Прикрепить raw proof artefact к governance evidence package, чтобы аудиторы могли
   пересчитать `hash_proof`.
3. Опубликовать канонический public-input encoding (hex или Norito) вместе со statement hash для
   детерминированных replay.

## Pipeline ingestion proofs

1. **Lane manifests** объявляют, какой `LaneCommitmentId` управляет каждым контрактом или bucket
   programmable-money.
2. **Admission** потребляет `ProofAttachment.lane_privacy` entries, проверяет приложенные witnesses
   для маршрутизированной lane через `LanePrivacyRegistry::verify` и передает валидированные
   commitment ids в lane compliance (`privacy_commitments_any_of`), чтобы правила allow/deny для
   programmable-money требовали наличие proofs перед постановкой в очередь.
3. **Telemetry** увеличивает счетчики `nexus_privacy_commitments.{merkle,snark}` с `LaneId`,
   `commitment_id` и outcome (`ok`, `depth_mismatch`, etc.).
4. **Governance evidence** использует тот же helper для генерации acceptance reports
   (`ci/check_nexus_lane_registry_bundle.sh` подключается к bundle verification, как только
   появляется SNARK metadata).

### Видимость для операторов

Endpoint Torii `/v1/sumeragi/status` теперь отображает массив
`lane_governance[].privacy_commitments`, чтобы операторы и SDK могли сравнить live registry с
опубликованными manifests без повторного чтения bundle. Snapshot строится в
`crates/iroha_core/src/sumeragi/status.rs`, экспортируется REST/JSON handlers Torii
(`crates/iroha_torii/src/routing.rs`) и декодируется каждым клиентом
(`javascript/iroha_js/src/toriiClient.js`,
`python/iroha_python/src/iroha_python/client.py`, `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift`),
зеркаля схему manifest для Merkle roots и SNARK digests.

Commitment-only или split-replica lanes теперь проваливают admission, если их manifest не содержит
раздел `privacy_commitments`, гарантируя, что programmable-money flows не начнутся, пока
детерминированные proof anchors не будут доставлены вместе с bundle.

## Registry runtime и admission

- `LanePrivacyRegistry` (`crates/iroha_core/src/interlane/mod.rs`) snapshot'ит структуры
  `LaneManifestStatus` из manifest loader и хранит per-lane maps commitments по ключу
  `LaneCommitmentId`. Transaction queue и `State` устанавливают этот registry при каждой
  перезагрузке manifests (`Queue::install_lane_manifests`, `State::install_lane_manifests`),
  так что admission и consensus validation всегда имеют доступ к каноническим commitments.
- `LaneComplianceContext` теперь несет опциональную ссылку `Arc<LanePrivacyRegistry>`
  (`crates/iroha_core/src/compliance/mod.rs`). Когда `LaneComplianceEngine` оценивает транзакцию,
  programmable-money flows могут проверять те же commitments на lane, что и Torii, до постановки
  payload в очередь.
- Admission и core validation хранят `Arc<LanePrivacyRegistry>` рядом с handle governance manifest
  (`crates/iroha_core/src/queue.rs`, `crates/iroha_core/src/state.rs`), гарантируя, что
  programmable-money модули и будущие interlane hosts читают согласованное представление
  дескрипторов приватности при ротации manifests.

## Runtime enforcement

Lane privacy proofs теперь идут вместе с attachments транзакции через `ProofAttachment.lane_privacy`.
Torii admission и проверка в `iroha_core` валидируют каждый attachment по registry маршрутизированной
lane через `LanePrivacyRegistry::verify`, записывают proven commitment ids и передают их в compliance
engine. Любое правило `privacy_commitments_any_of` теперь возвращает `false`, пока соответствующий
attachment не пройдет проверку, поэтому programmable-money lanes нельзя поставить в очередь или
коммитить без нужного witness. Юнит покрытие находится в `interlane::tests` и тестах lane
compliance, чтобы закрепить путь attachments и policy guardrails.

Для быстрого эксперимента см. юнит-тесты в
[`privacy.rs`](../../crates/iroha_crypto/src/privacy.rs), которые показывают успешные и ошибочные
кейсы для Merkle и zk-SNARK commitments.
