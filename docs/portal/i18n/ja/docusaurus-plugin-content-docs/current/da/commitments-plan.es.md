---
lang: ja
direction: ltr
source: docs/portal/docs/da/commitments-plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::ノート フエンテ カノニカ
リフレジャ`docs/source/da/commitments_plan.md`。 Mantenga ambas バージョン en
:::

# Sora Nexus (DA-3) でのデータ可用性の侵害計画

_編集者: 2026-03-25 -- 担当者: コア プロトコル WG / スマート コントラクト チーム / ストレージ チーム_

DA-3 は、Nexus のブロック レーンの登録情報を拡張します。
DA-2 のブロブの認識を決定します。エスタ ノタ キャプチャー ラス
正規の構造、パイプラインのフック、プルエバスの損失
cliente ligero y las superficies Torii/RPC que deben aterrizar antes de que los
バリダドレス プエダン コンフィア エン コンプロミソス DA デュランテ アドミッション オブ チェケオス
ゴベルナンザ。 Todos los ペイロード estan codificados en Norito; sin retired codec に JSON 広告
ほら。

## オブジェクト

- BLOB による被害 (チャンク ルート + マニフェスト ハッシュ + コミットメント KZG)
  オプション) dentro de cada bloque Nexus para que los Peers puedan reconstruir el
  アルマセナミエントの燃料帳簿の可用性を調べます。
- クライアントの検証結果を確認するための決定事項を証明する
  マニフェスト ハッシュ ファイルの最終処理はブロック ダドで行われます。
- 公開者は、Torii (`/v1/da/commitments/*`) y の許可を取得します。
  リレー、SDK、および自動監査による再現性の可用性
  カダブロック。
- 封筒 `SignedBlockWire` canonico al enhebrar las nuevas
  ヘッダーとメタデータの構造体 Norito とハッシュの導出
  ブロック。

## パノラマ デ アルカンス

1. **日付のモデル** en `iroha_data_model::da::commitment` mas
   `iroha_data_model::block` のヘッダーブロックのカンビオ。
2. **実行者によるフック** パラケ `iroha_core` でレシートを取り込み、DA が送信する
   Torii (`crates/iroha_core/src/queue.rs` y `crates/iroha_core/src/block.rs`)。
3. **永続化/インデックス** による WSV の侵害対応の相談
   ラピド (`iroha_core/src/wsv/mod.rs`)。
4. **Adiciones RPC en Torii** エンドポイントのリスト/コンサルタ/プルエババジョ
   `/v1/da/commitments`。
5. **統合テストとフィクスチャ** ワイヤ レイアウトとフルホの検証
   証明 en `integration_tests/tests/da/commitments.rs`。

## 1. アディシオネス・アル・モデルロ・デ・ダトス

### 1.1 `DaCommitmentRecord`

```rust
/// Canonical record stored on-chain and inside SignedBlockWire.
pub struct DaCommitmentRecord {
    pub lane_id: LaneId,
    pub epoch: u64,
    pub sequence: u64,
    pub client_blob_id: BlobDigest,
    pub manifest_hash: ManifestDigest,        // BLAKE3 over DaManifestV1 bytes
    pub proof_scheme: DaProofScheme,          // V1 lane policy (merkle_sha256 only)
    pub chunk_root: Hash,                     // Merkle root of chunk digests
    pub proof_digest: Option<Hash>,           // hash of PDP/PoTR schedule
    pub retention_class: RetentionClass,      // mirrors DA-2 retention policy
    pub storage_ticket: StorageTicketId,
    pub acknowledgement_sig: Signature,       // Torii DA service key
}
```

- **V1 protocol invariant:** V1 contains only the `merkle_sha256` proof scheme. It has no KZG variant, commitment field, setup, generation, or verification path; unsupported configuration is rejected before node startup.
- A future KZG design requires a separately reviewed protocol version and an explicit wire-layout change. Hash expansion is not an elliptic-curve commitment.
- The verification endpoint checks commitment consistency against a caller-authenticated canonical block header and committed policy sidecar; it does not independently authenticate block signatures or finality.

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
```ハッシュ デル バンドルとメタデータのハッシュ デル ブロックの接続
`SignedBlockWire`。クアンド アン ブロック ノ レッヴァ ダトス DA エル カンポ パーマネス `None`

実装上の注意: `BlockPayload` は透明です `BlockBuilder` ああ
指数セッター/ゲッター `da_commitments` (バージョン `BlockBuilder::set_da_commitments`)
y `SignedBlock::set_da_commitments`)、asi que los hosts pueden adjuntar unbundle
ブロックの前に事前に準備しておきます。 Todos los コンストラクター ヘルパー デジャン エル
Campo en `None` hasta que Torii enhebre は現実をバンドルします。

### 1.3 ワイヤのエンコーディング

- `SignedBlockWire::canonical_wire()` アグリガ エル ヘッダー Norito パラ
  `DaCommitmentBundle` 取引リストの管理
  存在します。バージョン `0x01` のバイト。
- `SignedBlockWire::decode_wire()` は、`version` をデスコノシドとしてバンドルします。
  `norito.md` の政治的情報 Norito の説明。
- `block::Hasher` でハッシュ ビブン ソロの実際の派生を実現します。ロス
  クライアント リジェロス クエリ解読電子ワイヤー フォーマット存在ガナン エル ヌエボ カンポ
  自動ヘッダー Norito アヌンシア スー プレセンシア。

## 2. ブロック生産のフルホ

1. La ingesta DA de Torii Finaliza un `DaIngestReceipt` y lo publica en la cola
   インテル (`iroha_core::gossiper::QueueMessage::DaReceipt`)。
2. `PendingBlocks` は受信確認を再コピーします。 `lane_id` は一致します
   構築中のブロック、`(lane_id, client_blob_id,
   マニフェスト_ハッシュ)`。
3. 売り出す前に、ビルダーが妥協するかどうか `(lane_id,
   エポック、シーケンス)` パラマンテナー、ハッシュ決定、文書化、バンドルコン
   コーデックは Norito、実際は `da_commitments_hash`。
4. バンドルを完全にアルマセナと WSV に送信して、ブロック全体を表示する
   `SignedBlockWire`。

火事で大騒ぎになり、コーラのレシートが永久に失われる
シギエンテ・インテント・ロス・トメ。エル ビルダー レジストラ エル ウルティモ `sequence` を含む
レーン・パラ・エヴィタール・アタケス・デ・リプレイ。

## 3. RPC の詳細については、相談してください。

Torii トレス エンドポイントを説明します:

|ルタ |メトド |ペイロード |メモ |
|------|--------|-----------|----------|
| `/v1/da/commitments` | `POST` | `DaCommitmentQuery` (レーン/エポック/シーケンス、ページのランゴ フィルター) | Devuelve `DaCommitmentPage` 合計、ブロックのハッシュが侵害されています。 |
| `/v1/da/commitments/prove` | `POST` | `DaCommitmentProofRequest` (レーン + マニフェスト ハッシュ値 `(epoch, sequence)`)。 | `DaCommitmentProof` (レコード + ルタ マークル + ハッシュ デ ブロック) に応答します。 |
| `/v1/da/commitments/verify` | `POST` | `DaCommitmentProof` |ヘルパーステートレスクエリのブロックとブロックのハッシュと検証の計算を実行します。 SDK を使用して、`iroha_crypto` を直接参照することはできません。 |

Todos のペイロードは、バホ `iroha_data_model::da::commitment` に表示されます。ルーターの紛失
Torii モンタン ロス ハンドラー ジュント ロス エンドポイント デ インジェスタ DA 存在パラ
トークン/mTLS の政治を再利用します。

## 4. 包括的な顧客のプルーバ- El productor de bloques construye un arbol Merkle binario sobre la lista
  `DaCommitmentRecord` のシリアル番号。ラ・ライズ・アリメンタ`da_commitments_hash`。
- `DaCommitmentProof` エンパケタ エル レコード オブジェクト ベクトル デ
  `(sibling_hash, position)` パラ・ケ・ロス・ベリフィカドレスは、ラ・ライズを再構築します。ラス
  プルエバス タンビエンには、ハッシュ デ ブロックとヘッダー パラケが含まれます
  クライアントはファイナリティを検証します。
- CLI ヘルパー (`iroha_cli app da prove-commitment`) 環境設定
  solicitud/verificacion de pruebas y exponen salidas Norito/hex para
  オペラドール。

## 5. ストレージとインデックスの作成

El WSV アルマセナ コンプロミソスとウナ コラム ファミリー デディカダ コン クラーベ
`manifest_hash`。 Los インデックス secundarios cubren `(lane_id, epoch)` y
`(lane_id, sequence)` は、エビテン エスカニア バンドルを完全にコンサルトします。
ブロックの記録を記録し、ノードを許可する
ブロック ログを再構築し、インデックスを迅速に作成します。

## 6. テレメトリアと観察可能性

- `torii_da_commitments_total` インクリメンタル キュアンド アン ブロック セラー アル メノス アン
  記録する。
- `torii_da_commitment_queue_depth` rastrea 領収書 esperando ser empaquetados
  （ポーレーン）。
- El ダッシュボード Grafana `dashboards/grafana/da_commitments.json` 視覚化
  ブロック全体の包含、豊富なコーラとプルエバスのスループット
  DA-3 のゲートがリリースされ、監査用のコンポルタミエントが失われます。

## 7. プルエバスの戦略

1. **Future KZG protocol** — KZG is not part of V1. A separately reviewed later protocol version must specify polynomial encoding, setup provenance, commitment/opening algorithms, consensus verification, deterministic test vectors, and a versioned wire-layout change. V1 has no enable toggle or reserved accepted value.
2. **シーケンス ギャップ** - レーンの制限は許可されますか?エルプラン実レチャザ
   ギャップサルボ ケ ゴベルナンザ アクティブ `allow_sequence_skips` パラ リプレイ デ
   緊急事態。
3. **ライト クライアント キャッシュ** - SQLite ライブラリ パラメタで SDK の pidio をキャッシュするための機能
   証拠;セギミエント ペンディエンテ バホ DA-8。

レスポンダーは BORRADOR の DA-3 を実装する前に PR を実行します (エステ
documento) EN PROGRESO cuando el trabajo de codigo comience。