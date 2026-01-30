---
lang: ja
direction: ltr
source: docs/portal/docs/da/commitments-plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note Канонический источник
Эта страница отражает `docs/source/da/commitments_plan.md`. Держите обе версии
:::

# План коммитментов Data Availability Sora Nexus (DA-3)

_Черновик: 2026-03-25 — Владельцы: Core Protocol WG / Smart Contract Team / Storage Team_

DA-3 расширяет формат блока Nexus так, чтобы каждая lane встраивала
детерминированные записи, описывающие blobs, принятые DA-2. В этом документе
зафиксированы канонические структуры данных, хуки блокового пайплайна,
лайт-клиентские доказательства и поверхности Torii/RPC, которые должны появиться
до того, как валидаторы смогут полагаться на DA-коммитменты при admission или
проверках управления. Все payloadы Norito-кодированы; без SCALE и ad-hoc JSON.

## Цели

- Нести коммитменты на blob (chunk root + manifest hash + опциональный KZG
  commitment) внутри каждого блока Nexus, чтобы пиры могли реконструировать
  состояние availability без обращения к off-ledger storage.
- Дать детерминированные membership proofs, чтобы light clients могли проверить,
  что manifest hash финализирован в конкретном блоке.
- Экспортировать Torii запросы (`/v1/da/commitments/*`) и proofs, позволяющие
  relays, SDKs и automation governance проверять availability без реплея всех
  блоков.
- Сохранить канонический `SignedBlockWire` envelope, пропуская новые структуры
  через Norito metadata header и derivation block hash.

## Обзор области работ

1. **Дополнения data model** в `iroha_data_model::da::commitment` плюс изменения
   block header в `iroha_data_model::block`.
2. **Executor hooks** чтобы `iroha_core` ingest-ил DA receipts, эмитированные
   Torii (`crates/iroha_core/src/queue.rs` и `crates/iroha_core/src/block.rs`).
3. **Persistence/indexes** чтобы WSV быстро отвечал на commitment queries
   (`iroha_core/src/wsv/mod.rs`).
4. **Torii RPC additions** для list/query/prove endpoints под
   `/v1/da/commitments`.
5. **Integration tests + fixtures** для проверки wire layout и proof flow в
   `integration_tests/tests/da/commitments.rs`.

## 1. Дополнения data model

### 1.1 `DaCommitmentRecord`

```rust
/// Canonical record stored on-chain and inside SignedBlockWire.
pub struct DaCommitmentRecord {
    pub lane_id: LaneId,
    pub epoch: u64,
    pub sequence: u64,
    pub client_blob_id: BlobDigest,
    pub manifest_hash: ManifestDigest,        // BLAKE3 over DaManifestV1 bytes
    pub proof_scheme: DaProofScheme,          // lane policy (merkle_sha256 or kzg_bls12_381)
    pub chunk_root: Hash,                     // Merkle root of chunk digests
    pub kzg_commitment: Option<KzgCommitment>,
    pub proof_digest: Option<Hash>,           // hash of PDP/PoTR schedule
    pub retention_class: RetentionClass,      // mirrors DA-2 retention policy
    pub storage_ticket: StorageTicketId,
    pub acknowledgement_sig: Signature,       // Torii DA service key
}
```

- `KzgCommitment` переиспользует 48-байтовую точку из `iroha_crypto::kzg`.
  При отсутствии используем только Merkle proofs.
- `proof_scheme` берется из каталога lanes; Merkle lanes отклоняют KZG payloads,
  а `kzg_bls12_381` lanes требуют ненулевых KZG commitments. Torii сейчас
  производит только Merkle commitments и отклоняет lanes с конфигурацией KZG.
- `KzgCommitment` переиспользует 48-байтовую точку из `iroha_crypto::kzg`.
  При отсутствии на Merkle lanes используем только Merkle proofs.
- `proof_digest` закладывает DA-5 PDP/PoTR интеграцию, чтобы запись содержала
  расписание sampling, использованное для поддержания blobs.

### 1.2 Расширение block header

```
pub struct BlockHeader {
    ...
    pub da_commitments_hash: Option<HashOf<DaCommitmentBundle>>,
}

pub struct DaCommitmentBundle {
    pub version: u16,                // start with 1
    pub commitments: Vec<DaCommitmentRecord>,
}
```

Хэш bundle входит как в hash блока, так и в metadata `SignedBlockWire`. Когда
накладных расходов.

Implementation note: `BlockPayload` и прозрачный `BlockBuilder` теперь имеют
setters/getters `da_commitments` (см. `BlockBuilder::set_da_commitments` и
`SignedBlock::set_da_commitments`), так что hosts могут прикрепить
предварительно собранный bundle до запечатывания блока. Все helper-конструкторы
оставляют поле `None` пока Torii не начнет передавать реальные bundles.

### 1.3 Wire encoding

- `SignedBlockWire::canonical_wire()` добавляет Norito header для
  `DaCommitmentBundle` сразу после списка транзакций. Версионный byte `0x01`.
- `SignedBlockWire::decode_wire()` отклоняет bundles с неизвестной `version`,
  в соответствии с Norito политикой из `norito.md`.
- Hash derivation обновляется только в `block::Hasher`; light clients, которые
  декодируют существующий wire format, автоматически получают новое поле, потому
  что Norito header объявляет его наличие.

## 2. Поток выпуска блоков

1. Torii DA ingest финализирует `DaIngestReceipt` и публикует его во внутреннюю
   очередь (`iroha_core::gossiper::QueueMessage::DaReceipt`).
2. `PendingBlocks` собирает все receipts с совпадающим `lane_id` для блока в
   построении, дедуплицируя по `(lane_id, client_blob_id, manifest_hash)`.
3. Перед запечатыванием builder сортирует commitments по `(lane_id, epoch,
   sequence)` для детерминированного hash, кодирует bundle Norito codec и
   обновляет `da_commitments_hash`.
4. Полный bundle сохраняется в WSV и эмитируется вместе с блоком в
   `SignedBlockWire`.

Если создание блока проваливается, receipts остаются в очереди для следующей
попытки; builder записывает последний включенный `sequence` по каждой lane,
чтобы предотвратить replay атаки.

## 3. RPC и Query surface

Torii предоставляет три endpoint:

| Route | Method | Payload | Notes |
|-------|--------|---------|-------|
| `/v1/da/commitments` | `POST` | `DaCommitmentQuery` (range-фильтр по lane/epoch/sequence, pagination) | Возвращает `DaCommitmentPage` с total count, commitments и hash блока. |
| `/v1/da/commitments/prove` | `POST` | `DaCommitmentProofRequest` (lane + manifest hash или кортеж `(epoch, sequence)`). | Отвечает `DaCommitmentProof` (record + Merkle path + hash блока). |
| `/v1/da/commitments/verify` | `POST` | `DaCommitmentProof` | Stateless helper, пересчитывающий hash блока и проверяющий inclusion; полезен для SDKs без прямого доступа к `iroha_crypto`. |

Все payloads находятся в `iroha_data_model::da::commitment`. Torii роутеры
монтируют handlers рядом с существующими DA ingest endpoints, чтобы переиспользовать
политики token/mTLS.

## 4. Inclusion proofs и light clients

- Производитель блока строит бинарное Merkle дерево по сериализованному списку
  `DaCommitmentRecord`. Корень подает `da_commitments_hash`.
- `DaCommitmentProof` упаковывает целевой record и вектор `(sibling_hash,
  position)` чтобы верификаторы смогли восстановить корень. Proof включает hash
  блока и подписанный header, чтобы light clients могли проверить finality.
- CLI helpers (`iroha_cli app da prove-commitment`) оборачивают цикл proof request/
  verify и показывают Norito/hex вывод для операторов.

## 5. Storage и индексирование

WSV хранит commitments в отдельной column family с ключом `manifest_hash`.
Вторичные индексы покрывают `(lane_id, epoch)` и `(lane_id, sequence)`, чтобы
запросы не сканировали полные bundles. Каждый record хранит высоту блока, в
котором он был запечатан, что позволяет catch-up узлам быстро восстанавливать
индекс из block log.

## 6. Telemetry и Observability

- `torii_da_commitments_total` увеличивается, когда блок запечатывает минимум
  один record.
- `torii_da_commitment_queue_depth` отслеживает receipts, ожидающие bundling
  (по lane).
- Grafana dashboard `dashboards/grafana/da_commitments.json` визуализирует
  включение в блок, глубину очереди и proof throughput для аудита DA-3 release
  gate.

## 7. Стратегия тестирования

1. **Unit tests** для encoding/decoding `DaCommitmentBundle` и обновлений
   block hash derivation.
2. **Golden fixtures** в `fixtures/da/commitments/` с каноническими bytes bundle
   и Merkle proofs.
3. **Integration tests** с двумя валидаторами, ingest sample blobs и проверка
   согласованности bundle и query/proof ответов.
4. **Light-client tests** в `integration_tests/tests/da/commitments.rs` (Rust),
   вызывающие `/prove` и проверяющие proof без обращения к Torii.
5. **CLI smoke** скрипт `scripts/da/check_commitments.sh` для воспроизводимости
   операторского tooling.

## 8. План rollout

| Phase | Description | Exit Criteria |
|-------|-------------|---------------|
| P0 - Data model merge | Принять `DaCommitmentRecord`, обновления block header и Norito codecs. | `cargo test -p iroha_data_model` зеленый с новыми fixtures. |
| P1 - Core/WSV wiring | Протянуть queue + block builder логику, сохранить индексы и открыть RPC handlers. | `cargo test -p iroha_core`, `integration_tests/tests/da/commitments.rs` проходят с проверками bundle proof. |
| P2 - Operator tooling | Поставить CLI helpers, Grafana dashboard и обновления docs по proof verification. | `iroha_cli app da prove-commitment` работает на devnet; dashboard показывает live данные. |
| P3 - Governance gate | Включить block validator, требующий DA commitments на lanes, отмеченных в `iroha_config::nexus`. | Status entry + roadmap update помечают DA-3 как завершенный. |

## Открытые вопросы

1. **KZG vs Merkle defaults** — Нужно ли для маленьких blobs всегда пропускать
   KZG commitments, чтобы уменьшить размер блока? Предложение: оставить
   `kzg_commitment` опциональным и gating через `iroha_config::da.enable_kzg`.
2. **Sequence gaps** — Разрешать ли разрывы последовательности? Текущий план
   отклоняет gaps, если governance не включит `allow_sequence_skips` для
   экстренного replay.
3. **Light-client cache** — Команда SDK запросила легкий SQLite cache для proofs;
   дальнейшая работа под DA-8.

Ответы на эти вопросы в implementation PRs переведут DA-3 из статуса "draft"
(этот документ) в "in progress" после старта кодовых работ.
