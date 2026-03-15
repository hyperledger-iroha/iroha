---
lang: ja
direction: ltr
source: docs/portal/docs/da/commitments-plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note Канонический источник
Эта страница отражает `docs/source/da/commitments_plan.md`。 жержите обе версии
:::

# План коммитментов データの可用性 Sora Nexus (DA-3)

_チーム: 2026-03-25 — チーム: コア プロトコル WG / スマート コントラクト チーム / ストレージ チーム_

DA-3 は、Nexus так、чтобы каждая lane встраивала です。
ブロブ、DA-2 を使用してください。 В этом документе
зафиксированы канонические структуры данных, хуки блокового пайплайна,
лайт-клиентские доказательства и поверхности Torii/RPC, которые должны появиться
до того, как валидаторы смогут полагаться на DA-коммитменты при 入場料 или
そうです。ペイロード Norito-кодированы; 「スケール」と「アドホック JSON」。

## Цели

- Нести коммитменты на blob (チャンク ルート + マニフェスト ハッシュ + опциональный KZG)
  コミットメント) внутри каждого блока Nexus, чтобы пиры могли реконструировать
  可用性の確認と台帳外保管。
- 会員証明、ライトクライアントの認証、
  マニフェスト ハッシュ финализирован в конкретном блоке。
- Экспортировать Torii запросы (`/v1/da/commitments/*`) と証拠、позволяющие
  リレー、SDK、自動化ガバナンスの可用性を確認する
  ブロカボ。
- Сохранить канонический `SignedBlockWire` 封筒、пропуская новые структуры
  Norito メタデータ ヘッダーと導出ブロック ハッシュ。

## Обзор области работ

1. `iroha_data_model::da::commitment` の **データ モデル**
   ブロックヘッダー × `iroha_data_model::block`。
2. **エグゼキューターフック** чтобы `iroha_core` 取り込み - DA レシート、эмитированные
   Torii (`crates/iroha_core/src/queue.rs` または `crates/iroha_core/src/block.rs`)。
3. **永続性/インデックス** WSV のコミットメント クエリ
   (`iroha_core/src/wsv/mod.rs`)。
4. **Torii RPC の追加** リスト/クエリ/エンドポイントの証明
   `/v1/da/commitments`。
5. **統合テスト + フィクスチャ** ワイヤー レイアウトとプルーフ フローの組み合わせ
   `integration_tests/tests/da/commitments.rs`。

## 1. データモデル

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

- `KzgCommitment` は 48-байтовую точку из `iroha_crypto::kzg` です。
  マークル証明を使用します。
- `proof_scheme` レーン。 Merkle レーン отклоняют KZG ペイロード、
  `kzg_bls12_381` レーンは KZG コミットメントを満たしています。 Torii 認証済み
  マークルのコミットメントとレーンを конфигурацией KZG で確認してください。
- `KzgCommitment` は 48-байтовую точку из `iroha_crypto::kzg` です。
  マークル レーンを使用して、マークル証明を作成します。
- `proof_digest` закладывает DA-5 PDP/PoTR интеграцию, чтобы запись содержала
  サンプリング、ブロブの作成。

### 1.2 ブロックヘッダー

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

バンドルとハッシュ、メタデータ `SignedBlockWire`。 Когда
накладных расходов。実装メモ: `BlockPayload` и прозрачный `BlockBuilder` теперь имеют
セッター/ゲッター `da_commitments` (最低 `BlockBuilder::set_da_commitments` および
`SignedBlock::set_da_commitments`)、так что ホスト могут прикрепить
いくつかのバンドルが含まれています。 Все ヘルパー - конструкторы
`None` と Torii のバンドルが必要です。

### 1.3 ワイヤーエンコーディング

- `SignedBlockWire::canonical_wire()` ヘッダー Norito ヘッダー
  `DaCommitmentBundle` は最高です。 Версионный バイト `0x01`。
- `SignedBlockWire::decode_wire()` отклоняет バンドル с неизвестной `version`、
  Norito と `norito.md` を確認してください。
- ハッシュ導出 обновляется только в `block::Hasher`;ライトクライアント、クラス
  декодируют существующий ワイヤ フォーマット、автоматически получают новое поле、потому
  что Norito ヘッダー объявляет его наличие。

## 2. Поток выпуска блоков

1. Torii DA 取り込み финализирует `DaIngestReceipt` и публикует его во внутреннюю
   очередь (`iroha_core::gossiper::QueueMessage::DaReceipt`)。
2. `PendingBlocks` は領収書を受け取ります `lane_id` は領収書を受け取ります
   `(lane_id, client_blob_id, manifest_hash)` を確認してください。
3. ビルダーのコミットメントを確認します `(lane_id, epoch,
   シーケンス)` для детерминированного ハッシュ、バンドル Norito コーデック и
   `da_commitments_hash`。
4. WSV および эмитируется のПолный バンドル сохраняется вместе с блоком
   `SignedBlockWire`。

Если создание блока проваливается、領収書 остаются в очереди для следующей
そうです。ビルダー записывает последний включенный `sequence` по каждой レーン、
再生 атаки をご覧ください。

## 3. RPC и クエリサーフェス

Torii エンドポイント:

|ルート |方法 |ペイロード |メモ |
|----------|----------|----------|----------|
| `/v1/da/commitments` | `POST` | `DaCommitmentQuery` (範囲 - レーン/エポック/シーケンス、ページネーション) | `DaCommitmentPage` の合計数、コミットメント、ハッシュ値。 |
| `/v1/da/commitments/prove` | `POST` | `DaCommitmentProofRequest` (レーン + マニフェスト ハッシュ или кортеж `(epoch, sequence)`)。 | `DaCommitmentProof` (レコード + マークル パス + ハッシュ)。 |
| `/v1/da/commitments/verify` | `POST` | `DaCommitmentProof` |ステートレス ヘルパー、ハッシュ値とインクルード値。 SDK は `iroha_crypto` に対応しています。 |

ペイロードは `iroha_data_model::da::commitment` です。 Torii роутеры
ハンドラー рядом с существующими DA 取り込みエンドポイント、чтобы переиспользовать
トークン/mTLS。

## 4. インクルージョンプルーフとライトクライアント

- マークルのメッセージを受け取ると、メッセージが表示されます。
  `DaCommitmentRecord`。 Корень подает `da_commitments_hash`。
- `DaCommitmentProof` упаковывает целевой レコード и вектор `(sibling_hash,
  位置)` чтобы верификаторы смогли восстановить корень.証明ハッシュ
  ヘッダー、ライト クライアント、ファイナリティを備えています。
- CLI ヘルパー (`iroha_cli app da prove-commitment`) оборачивают цикл 証明リクエスト/
  Norito/hex を確認してください。

## 5. ストレージとストレージWSV のコミットメントと列ファミリーの ключом `manifest_hash`。
Вторичные индексы покрывают `(lane_id, epoch)` および `(lane_id, sequence)`、чтобы
バンドルが必要です。 Каждый レコード хранит высоту блока, в
котором он был запечатан, что позволяет キャッチアップ узлам быстро восстанавливать
ブロックログ。

## 6. テレメトリーと可観測性

- `torii_da_commitments_total` увеличивается、когда блок запечатывает минимум
  один 記録。
- `torii_da_commitment_queue_depth` 領収書、バンドル
  (レーン)。
- Grafana ダッシュボード `dashboards/grafana/da_commitments.json` の画面
  DA-3 リリースのテストと証明スループット
  門。

## 7. Стратегия тестирования

1. **単体テスト** エンコード/デコード `DaCommitmentBundle` および обновлений
   ブロックハッシュ導出。
2. **ゴールデン フィクスチャ** в `fixtures/da/commitments/` с каноническими バイト バンドル
   и マークル証明。
3. **統合テスト** テストを実行し、サンプル BLOB を取り込み、テストします。
   バンドルとクエリ/証明を実行します。
4. **ライトクライアントテスト** × `integration_tests/tests/da/commitments.rs` (Rust)、
   `/prove` と Torii の証明。
5. **CLI スモーク** が `scripts/da/check_commitments.sh` で表示されます
   ツール。

## 8.「План」ロールアウト

|フェーズ |説明 |終了基準 |
|------|-------------|------|
| P0 - データ モデルのマージ | `DaCommitmentRecord`、ブロック ヘッダーおよび Norito コーデック。 | `cargo test -p iroha_data_model` のフィクスチャ。 |
| P1 - コア/WSV 配線 |キュー + ブロック ビルダー、RPC ハンドラーの組み合わせ。 | `cargo test -p iroha_core`、`integration_tests/tests/da/commitments.rs` はバンドル プルーフです。 |
| P2 - オペレーターツール | CLI ヘルパー、Grafana ダッシュボード、ドキュメント、証明検証をサポートします。 | `iroha_cli app da prove-commitment` 開発ネット;ダッシュボードはライブ配信を表示します。 |
| P3 - ガバナンス ゲート |ブロックバリデーター、レーン、отмеченных в `iroha_config::nexus` の DA コミットメント。 |ステータス エントリ + ロードマップの更新。DA-3 が表示されます。 |

## Открытые вопросы

1. **KZG 対 Merkle のデフォルト** — Нужно ли для маленьких blob всегда пропускать
   KZG のコミットメント、どのようなことをしますか?翻訳: оставить
   `kzg_commitment` は、`iroha_config::da.enable_kzg` をゲートします。
2. **シーケンス ギャップ** — 何か問題がありますか? Текущий план
   ギャップ、ガバナンスに関する問題 `allow_sequence_skips` 日
   экстренного リプレイ。
3. **ライト クライアント キャッシュ** — SDK と SQLite キャッシュの証拠。
   DA-8 を搭載しています。

DA-3 の実装 PR の「ドラフト」を確認します。
(этот документ) в "進行中" после старта кодовых работ.