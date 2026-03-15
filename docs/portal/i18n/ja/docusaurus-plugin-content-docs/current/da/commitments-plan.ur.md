---
lang: ja
direction: ltr
source: docs/portal/docs/da/commitments-plan.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note メモ
ہونے تک دونوں ورژنز کو 同期 رکھیں۔
:::

# Sora Nexus データ可用性コミットメント プラン (DA-3)

_重要: 2026-03-25 -- 重要: コア プロトコル WG / スマート コントラクト チーム / ストレージ チーム_

DA-3 Nexus 決定論的
レコード شامل ہوں جو DA-2 کے ذریعے قبول شدہ blob کو بیان کریں۔ ٌہ ٹ
正規のデータ構造、ブロック パイプライン フック、ライト クライアントの証明、
Torii/RPC サーフェス、検証、承認、ガバナンス
DA コミットメントをチェックします پر بھروسہ کرنے سے پہلے لازمی ہیں۔最大のペイロード
Norito でエンコードされたスケール アドホック JSON スケール

## 大事

- BLOB ごとのコミットメント (チャンク ルート + マニフェスト ハッシュ + オプション)
  KZG コミットメント) ピアオフレジャーストレージの管理
  可用性状態 دوبارہ بنا سکیں۔
- 決定的なメンバーシップ証明、ライトクライアントは確認します。
  マニフェスト ハッシュ مخصوص بلاک میں 確定 ہوا تھا۔
- Torii クエリ (`/v2/da/commitments/*`) 証明、リレー
  SDK のガバナンス自動化とリプレイの可用性の向上
  監査する
- `SignedBlockWire` エンベロープ 正規の構造 Norito
  メタデータ ヘッダーとブロック ハッシュの導出とスレッドの生成

## 範囲の概要

1. **データ モデルの追加** `iroha_data_model::da::commitment` ブロック
   ヘッダーの変更 `iroha_data_model::block` میں۔
2. **Executor フック** `iroha_core` Torii 発行、DA レシートの取り込み
   (`crates/iroha_core/src/queue.rs` اور `crates/iroha_core/src/block.rs`)。
3. **永続性/インデックス** WSV コミットメント クエリの処理
   (`iroha_core/src/wsv/mod.rs`)。
4. **Torii RPC の追加** エンドポイントのリスト/クエリ/証明
   `/v2/da/commitments` いいえ
5. **統合テスト + フィクスチャ** 配線レイアウト、プルーフ フロー、検証
   `integration_tests/tests/da/commitments.rs` すごい

## 1. データモデルの追加

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

- `KzgCommitment` موجودہ 48 バイト پوائنٹ کو 再利用 کرتا ہے جو `iroha_crypto::kzg`
  すごい欠席 ہو تو صرف マークル証明 ستعمال ہوتے ہیں۔
- `proof_scheme` レーン カタログ سے 派生 ہوتا ہے؛マークルレーン KZG ペイロード
  拒否 `kzg_bls12_381` レーン ゼロ以外の KZG コミットメントが必要
  ありがとうございますTorii マークル コミットメント、KZG 構成
  レーンを拒否する
- `KzgCommitment` موجودہ 48 バイト پوائنٹ کو 再利用 کرتا ہے جو `iroha_crypto::kzg`
  すごいマークル レーンがありません ہو تو صرف マークル証明がありません ہوتے ہیں۔
- `proof_digest` DA-5 PDP/PoTR 統合
  サンプリング スケジュールを記録する ブロブを記録する
  ہوتا ہے۔

### 1.2 ブロックヘッダー拡張

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

バンドル ハッシュ ハッシュ `SignedBlockWire` メタデータ دونوں میں شامل ہوتا ہے۔ああ
और देखें実装メモ: `BlockPayload` 透過 `BlockBuilder` 透過
`da_commitments` セッター/ゲッターは کرتے ہیں (دیکھیں) を公開します
`BlockBuilder::set_da_commitments` は `SignedBlock::set_da_commitments`) です
ホストのシール سے پہلے 構築済みのバンドルの添付助っ人
コンストラクター フィールド کو `None` رکھتے ہیں جب تک Torii حقیقی バンドル スレッド نہ کرے۔

### 1.3 ワイヤーエンコーディング

- `SignedBlockWire::canonical_wire()` トランザクション リスト
  `DaCommitmentBundle` ヘッダー追加 Norito ヘッダー追加バージョン バイト `0x01` ہے۔
- `SignedBlockWire::decode_wire()` 不明 `version` バンドルは拒否されます
  ہے، جیسا کہ `norito.md` میں Norito ポリシー بیان ہے۔
- ハッシュ導出の更新 صرف `block::Hasher` میں ہیں؛既存のワイヤ形式
  ライトクライアントをデコードする フィールドをデコードする
  Norito ヘッダー اس کی موجودگی بتاتا ہے۔

## 2. ブロック生産の流れ

1. Torii DA 取り込み `DaIngestReceipt` 内部キューのファイナライズ
   پر ہے (`iroha_core::gossiper::QueueMessage::DaReceipt`) を公開します。
2. `PendingBlocks` 領収書 جمع کرتا ہے جن کا `lane_id` زیر تعمیر بلاک سے
   重複排除の一致 `(lane_id, client_blob_id, manifest_hash)` پر ہے، اور
   ありがとうございます
3. ビルダーのコミットメントをシールします `(lane_id, epoch, sequence)`
   ソート ハッシュ決定論的バンドル Norito コーデック エンコード
   `da_commitments_hash` 更新日
4. バンドル WSV ストア ہوتا ہے اور `SignedBlockWire` کے ساتھ بلاک میں
   ہوتا ہے۔を発する

失敗する ہو تو レシートキュー میں رہتے ہیں تاکہ اگلی کوشش انہیں لے
ああビルダー ہر レーン کیلئے آخری شامل شدہ `sequence` ریکارڈ کرتا ہے تاکہ リプレイ
攻撃 攻撃 攻撃

## 3. RPC とクエリ サーフェス

Torii のエンドポイントの数:

|ルート |方法 |ペイロード |メモ |
|----------|----------|----------|----------|
| `/v2/da/commitments` | `POST` | `DaCommitmentQuery` (レーン/エポック/シーケンス範囲フィルター、ページネーション) | `DaCommitmentPage` 合計数、コミットメント数、ブロック ハッシュ数、数字、数字。 |
| `/v2/da/commitments/prove` | `POST` | `DaCommitmentProofRequest` (レーン + マニフェスト ハッシュ `(epoch, sequence)` タプル)۔ | `DaCommitmentProof` واپس کرتا ہے (レコード + マークル パス + ブロック ハッシュ)۔ |
| `/v2/da/commitments/verify` | `POST` | `DaCommitmentProof` |ステートレス ヘルパー ブロック ハッシュ計算 ブロック 包含検証 ブロック ハッシュ計算SDK のアクセス `iroha_crypto` アクセス リンクの確認|

ペイロード `iroha_data_model::da::commitment` の数Toriiルーター
ハンドラー DA 取り込みエンドポイント マウント トークン/mTLS
ポリシーの再利用

## 4. 包含証明とライトクライアント

- プロデューサーのシリアル化 `DaCommitmentRecord` リスト バイナリ マークル ツリー
  ❁❁❁❁ルート `da_commitments_hash` フィード ہے۔
- `DaCommitmentProof` ターゲット レコード `(sibling_hash, position)` ベクトル
  پیک کرتا ہے تاکہ Verifiers root دوبارہ بنا سکیں۔証明ブロックハッシュ
  署名付きヘッダー بھی شامل ہیں تاکہ ライト クライアントのファイナリティ検証 کر سکیں۔
- CLI ヘルパー (`iroha_cli app da prove-commitment`) 証明リクエスト/検証サイクル
  ラップ کرتے ہیں اور 演算子 کیلئے Norito/hex 出力 دیتے ہیں۔## 5. ストレージとインデックス作成

WSV コミットメント専用の列ファミリー `manifest_hash` キー
ストアセカンダリ インデックス `(lane_id, epoch)` `(lane_id, sequence)`
カバー پورے クエリ バンドル スキャン 完了レコードとブロック
高さ トラック ブロック ログ シール キャッチアップ ノード ブロック ログ
インデックス 再構築 سکتے ہیں۔

## 6. テレメトリーと可観測性

- `torii_da_commitments_total` 増分 ہوتا ہے جب کوئی بلاک کم از کم ایک
  レコードシール
- `torii_da_commitment_queue_depth` バンドルの領収書
  トラック (レーンごと)۔
- Grafana ダッシュボード `dashboards/grafana/da_commitments.json` ブロックの包含
  キューの深さ 証明スループット DA-3 リリース ゲート
  行動監査

## 7. テスト戦略

1. `DaCommitmentBundle` エンコード/デコード、ブロック ハッシュ導出の更新
   **単体テスト**۔
2. `fixtures/da/commitments/` **ゴールデン フィクスチャ** 正規バンドル バイト
   マークル証明のキャプチャー
3. **統合テスト** 検証ツールの起動、サンプル BLOB の取り込み
   バンドルの内容 クエリ/証明応答 検証 確認
   やあ
4. **ライトクライアントテスト** `integration_tests/tests/da/commitments.rs` (Rust)
   میں جو `/prove` call کر کے Torii سے بات کئے بغیر 証明検証 کرتے ہیں۔
5. **CLI スモーク** スクリプト `scripts/da/check_commitments.sh` オペレーター ツール
   再現性のあるもの

## 8.展開計画

|フェーズ |説明 |終了基準 |
|------|-------------|------|
| P0 — データ モデルのマージ | `DaCommitmentRecord`、ブロック ヘッダーの更新 Norito コーデック ランド| `cargo test -p iroha_data_model` 固定具 緑の|
| P1 — コア/WSV 配線 |キュー + ブロック ビルダー ロジック スレッド インデックスは永続化 RPC ハンドラーは公開されます| `cargo test -p iroha_core`、`integration_tests/tests/da/commitments.rs` バンドル証明アサーションをパスします|
| P2 — オペレーターツール | CLI ヘルパー、Grafana ダッシュボード、証拠検証ドキュメントの出荷| `iroha_cli app da prove-commitment` 開発ネットダッシュボードのライブデータ|
| P3 — ガバナンスゲート | `iroha_config::nexus` フラグ付きレーンの DA コミットメントにはブロック バリデータの有効化が必要です|ステータス入力 + ロードマップ更新 DA-3 完了マーク کریں۔ |

## 未解決の質問

1. **KZG とマークルのデフォルト** — ブロブと KZG のコミットメント
   ブロック サイズ ブロック サイズバージョン: `kzg_commitment` オプション
   اور `iroha_config::da.enable_kzg` کے ذریعے ゲート کریں۔
2. **シーケンス ギャップ** — レーンの順序が乱れています。ギャップがすごい
   拒否する ガバナンス `allow_sequence_skips` 緊急リプレイを拒否する
   有効にする
3. **ライト クライアント キャッシュ** — SDK の証明と SQLite キャッシュ
   やあDA-8 フォローアップ باقی ہے۔

実装 PRs میں دینے سے DA-3 اس مسودے سے نکل کر
「進行中」 کی حالت میں جائے گا جب コード作業 شروع ہوگا۔