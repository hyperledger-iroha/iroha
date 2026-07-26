---
lang: ja
direction: ltr
source: docs/portal/docs/da/commitments-plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note ソースカノニク
リフレテ`docs/source/da/commitments_plan.md`。 Gardez les deux バージョンと同期
:::

# エンゲージメントの計画 データの可用性 Sora Nexus (DA-3)

_Redige: 2026-03-25 -- 担当: コアプロトコル WG / スマートコントラクトチーム / ストレージチーム_

DA-3 ブロック Nexus のブロック フォーマットを設定し、登録されたチャック レーンを統合します
deterministes derivant les blob は DA-2 に従って受け入れます。 Cette ノートキャプチャファイル
ドネの規範構造、ブロックのパイプラインのフック、プリューブの構造
クライアントのレジェ、および表面 Torii/RPC が到着する前に前もってファイルを送信する
Validateurs puissent s'appuyer sur lesエンゲージメント DA lors des checks
入学と統治。ファイル ペイロードは Norito をエンコードしません。パド
SCALEでJSONアドホック。

## 目的

- BLOB ごとのポーター デ エンゲージメント (チャンク ルート + マニフェスト ハッシュ + コミットメント KZG)
  オプション) ブロック Nexus のペアを再構築する必要があります
  在庫馬台帳のコンサルタントなしで在庫を入手できます。
- Fournir des preuves de メンバーシップは、クライアントのレジェーの範囲内で決定されます
  Verifient qu'un マニフェストのハッシュとブロックの完了を確認します。
- 要求されたエクスポーザー Torii (`/v1/da/commitments/*`) およびプレユーブ浸透剤
  補助リレー、SDK、および可用性の監査の自動化
  サン・レジュエ・チャク・ブロック。
- Conserver l'enveloppe `SignedBlockWire` canonique en transmettant les nouvelles
  ファイル ヘッダーとメタデータ Norito を介した構造、およびハッシュ ブロックの導出。

## スコープのアンサンブルを表示

1. **Ajouts au データ モデル** dans `iroha_data_model::da::commitment` plus
   `iroha_data_model::block` によるブロックのヘッダーの変更。
2. **実行者フック** は、`iroha_core` でレシートを取り込み、DA EMIS パーを取得します
   Torii (`crates/iroha_core/src/queue.rs` および `crates/iroha_core/src/block.rs`)。
3. **永続性/インデックス** WSV の迅速な応答に関する応答の補助
   コミットメント (`iroha_core/src/wsv/mod.rs`)。
4. **Ajouts RPC Torii** エンドポイントのリスト/講義/証明を行う
   `/v1/da/commitments`。
5. **統合 + 治具のテスト** ワイヤ レイアウトおよび磁束耐性の検証
   `integration_tests/tests/da/commitments.rs` です。

## 1. Ajouts au データモデル

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

- `KzgCommitment` ファイルポイント 48 オクテットは `iroha_crypto::kzg` を再利用します。
  マークルの唯一の特徴であるレトンブ・シュール・デ・プリューヴスに関しては、まったく存在しません。
- `proof_scheme` レーンのカタログを取得します。レ・レーン・マークル・レジェッテント・レ
  ペイロード KZG タンディス キュー レ レーン `kzg_bls12_381` 緊急のコミットメント KZG
  null ではない。 Torii マークルとリジェットに関する製品の実行に関する義務
  レーンは KZG で構成されます。
- `KzgCommitment` ファイルポイント 48 オクテットは `iroha_crypto::kzg` を再利用します。
  Quand il est missing sur les lanes Merkle on retombe sur des preuves Merkle
  ユニークさ。
- `proof_digest` ミーム レコードに対する DA-5 PDP/PoTR の統合を予想
  サンプリングのスケジュールを列挙し、メンテナンス中のブロブを利用します。

### 1.2 ヘッダーブロックの拡張

```
pub struct BlockHeader {
    ...
    pub da_commitments_hash: Option<HashOf<DaCommitmentBundle>>,
}

pub struct DaCommitmentBundle {
    pub version: u16,                // start with 1
    pub commitments: Vec<DaCommitmentRecord>,
}
```ブロックのハッシュとメタデータのハッシュ デュ バンドルのエントリ
`SignedBlockWire`。 Quand un bloc ne Transporte pas de donnees DA、le Champ Reste

実装に注意してください: `BlockPayload` et le `BlockBuilder` 透明露出
セッター/ゲッターのメンテナンス `da_commitments` (voir
`BlockBuilder::set_da_commitments` および `SignedBlock::set_da_commitments`)、ドンク
レ ホスト プベント アタッチャーとバンドルの事前構築前にブロックを実行します。トス
les helpers laissent le champ a `None` tant que Torii ne fournit pas de Bundles
リール。

### 1.3 エンコードワイヤー

- `SignedBlockWire::canonical_wire()` アジューテ ル ヘッダー Norito 注ぐ
  `DaCommitmentBundle` トランザクションが存在するリストの直後。
  バージョン推定バイト `0x01`。
- `SignedBlockWire::decode_wire()` バンドルを拒否する `version` est
  続行せず、政治的政治 Norito と `norito.md` を決定します。
- `block::Hasher` によるハッシュ ヴィベントの独自の派生時間のレミゼス。
  クライアント レジェ キ デコデント ル ワイヤー フォーマットの既存のガニャント ル ヌーボー
  チャンプオートマティークメントカールヘッダーNoritoは存在を発表します。

## 2. ブロック単位での生産の流動化

1. DA Torii を取り込み、`DaIngestReceipt` をキューに公開します
   インターネット (`iroha_core::gossiper::QueueMessage::DaReceipt`)。
2. `PendingBlocks` 領収書を収集する `lane_id` はブロックに対応しません
   構築、重複排除パー `(lane_id, client_blob_id,
   マニフェスト_ハッシュ)`。
3. `(lane_id, epoch,
   sequence)` ガーダー ファイルのハッシュ決定、バンドルの平均的なコーデックのエンコード
   Norito は、`da_commitments_hash` に遭遇しました。
4. WSV およびブロック全体の在庫を完了するバンドル
   `SignedBlockWire`。

ブロックの創造をエコーし、レシートを休ませて、キューを注ぎます
プロチェイン暫定的なレプレンヌ。ルビルダー登録ルデルニエ `sequence`
リプレイの攻撃に関するレーンの情報も含まれます。

## 3. Surface RPC とリクエスト

Torii は trois エンドポイントを公開します。

|ルート |メトーデ |ペイロード |メモ |
|----------|----------|----------|----------|
| `/v1/da/commitments` | `POST` | `DaCommitmentQuery` (範囲レーン/エポック/シーケンス、ページネーションのフィルター) | Renvoie `DaCommitmentPage` 平均合計、コミットメントおよびハッシュ デ ブロック。 |
| `/v1/da/commitments/prove` | `POST` | `DaCommitmentProofRequest` (レーン + マニフェスト ハッシュ タプル `(epoch, sequence)`)。 | avec `DaCommitmentProof` (レコード + chemin Merkle + ブロックのハッシュ) を応答します。 |
| `/v1/da/commitments/verify` | `POST` | `DaCommitmentProof` |ヘルパーステートレスはブロックのハッシュと有効性を計算します。 `iroha_crypto` に従って SDK を使用してください。 |

Tous les ペイロードは `iroha_data_model::da::commitment` を生き生きとさせます。レ・ルートゥール
Torii モンテント ファイル ハンドラーは、DA の存在を取り込むエンドポイントのコートを提供します
reutiliser les politiques トークン/mTLS。

## 4. プルーヴ・ダン・インクルージョンとクライアント・レジェ- Le producteur de bloc construit un arbre Merkle binaire sur la liste
  `DaCommitmentRecord` のシリアル番号を参照してください。ラシーンアリメンテ`da_commitments_hash`。
- `DaCommitmentProof` 記録ファイルとベクトルを表示
  `(sibling_hash, position)` は、ラの検証を再構築します
  ラシーン。 Les preuves incluent aussi le hash de block et le header Signe pour que
  レ・クライアント・レジェ・ピュイサント検証者ラ・フィナライト。
- Les helpers CLI (`iroha_cli app da prove-commitment`) エンベロープ ファイル サイクル
  Norito/16 進数のオペレーターを要求/検証し、出撃の暴露を要求します。

## 5. ストレージとインデックス作成

WSV ストックのコミットメントと列の家族のディディー、クレパー
`manifest_hash`。二次インデックス `(lane_id, epoch)` et
`(lane_id, sequence)` スキャナーのバンドルの要求に関する要求
完了します。 Chaque レコード スーツ la hauteur de bloc qui l'a scelle、permettant aux
ブロック ログの一部のインデックスを迅速に再構築することはできません。

## 6. 遠隔測定と観測可能性

- `torii_da_commitments_total` ブロック単位での増加分
  記録する。
- `torii_da_commitment_queue_depth` バンドルに付属の領収書をスーツします
  （パーレーン）。
- Le ダッシュボード Grafana `dashboards/grafana/da_commitments.json` 視覚化
  ブロックを含む、キューの詳細、およびプレビューのスループットを含む
  DA-3 のリリースのゲートは、監査員の要求に応えます。

## 7. テスト戦略

1. **`DaCommitmentBundle` などのエンコード/デコードをテストする**
   ハッシュドブロックの派生を逃します。
2. **フィクスチャ ゴールデン** スー `fixtures/da/commitments/` キャプチャラント ファイル バイト
   canoniques du Bundle et les preuves Merkle。
3. **統合テスト** の検証、ブロブの摂取
   テスト、および検証、バンドルなどの内容の一致を確認します
   質問/証明に対する応答。
4. **`integration_tests/tests/da/commitments.rs` による **ライト クライアントのテスト**
   (Rust) 控訴人 `/prove` およびパーラーなしの検証法 Torii。
5. **Smoke CLI** avec `scripts/da/check_commitments.sh` pour garder l'outillage
   操作者は再現可能です。

## 8. 展開の計画

|フェーズ |説明 |終了基準 |
|------|-------------|------|
| P0 - データ モデルのマージ |インテグレーター `DaCommitmentRecord`、ブロック ヘッダーとコーデック Norito が見つかりません。 | `cargo test -p iroha_data_model` vert avec nouvelles の備品。 |
| P1 - ワイヤリングコア/WSV |キュー ロジック + ブロック ビルダー、永続化ファイル インデックス、エクスポーザー ファイル ハンドラー RPC を構築します。 | `cargo test -p iroha_core`、`integration_tests/tests/da/commitments.rs` バンドル証明の過去のアベック アサーション。 |
| P2 - ツーリングオペレータ | Livrer ヘルパー CLI、ダッシュボード Grafana は、検証と証明のドキュメントを作成します。 | `iroha_cli app da prove-commitment` 開発ネット上の機能。ダッシュボードの添付ファイルはライブです。 |
| P3 - 統治ゲート | `iroha_config::nexus` により、ブロックの検証を要求するコミットメント DA シュールレーンの信号をアクティブにします。 |ステータスとロードマップの主要な更新 DA-3 は TERMINE にアクセスします。 |

## 質問が先に出る1. **KZG 対 Merkle のデフォルト** - KZG が注ぎ込む約束を無視したトゥージュールの実行
   les petits blobs afin de reduire la taille des blocs?命題: ガーダー
   `kzg_commitment` オプションネルゲートは `iroha_config::da.enable_kzg` 経由で。
2. **シーケンスのギャップ** - レーンの自動起動は必要ですか? Le plan actuel rejette
   アクティブな `allow_sequence_skips` を注いでリプレイを行うギャップ sauf si la gouvernance
   緊急です。
3. **ライト クライアント キャッシュ** - SQLite キャッシュ ファイルを要求する SDK を装備する
   証拠;スイヴィ アン アテント スー DA-8。

DA-3 の実装に関する PR に関する質問に応答します。
BROUILLON (ce document) EN COURS des que le travail de code が開始されます。