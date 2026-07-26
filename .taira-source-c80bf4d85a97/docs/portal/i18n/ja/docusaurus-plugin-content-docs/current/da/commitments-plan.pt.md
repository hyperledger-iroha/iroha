---
lang: ja
direction: ltr
source: docs/portal/docs/da/commitments-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note フォンテ カノニカ
エスペラ `docs/source/da/commitments_plan.md`。デュアス・ヴェルソエス・エムとしてのマンテンハ
:::

# Sora Nexus (DA-3) のデータ可用性の妥協計画

_レディ: 2026-03-25 -- 対応: コア プロトコル WG / スマート コントラクト チーム / ストレージ チーム_

DA-3 ブロック ダ Nexus パラケ カダ レーンが登録されている形式を保持しています
DA-2 のブロブを決定的に決定します。エスタノタキャプチャー
正規のデータの作成、OS フックはブロックとしてパイプラインを実行します。
クライアントは地上権 Torii/RPC の優先権を保持します
validadores possam confiar nos compromisos DA durante admissao ou checks de
ガバナンカ。 Todos os ペイロード sao codificados em Norito; JSON 広告をスケールする
ほら。

## オブジェクト

- BLOB による Carregar の妥協 (チャンク ルート + マニフェスト ハッシュ + コミットメント KZG)
  オプション) デントロ デ カダ ブロコ Nexus パラ ケ ピア ポッサム レコンストライル オ スタド
  可用性を検討したり、台帳のストレージを調べたりします。
- 顧客レベルでのメンバーシップの決定を決定する
  マニフェスト ハッシュを確認し、最終的なブロックを確認します。
- リレーを許可する Torii (`/v1/da/commitments/*`) をエクスポートします。
  SDK と自動管理監査の可用性は、完全に再現されます。
- マンター・オ・エンベロープ `SignedBlockWire` canonico ao enfiar as novas estruturas
  メタデータ Norito のヘッダーは、ハッシュ デ ブロックの派生です。

## パノラマ デ エスコポ

1. **Adicoes ao データ モデル** em `iroha_data_model::da::commitment` mais alteracoes
   ヘッダー デ ブロック em `iroha_data_model::block`。
2. **フックはエグゼキュータ** パラメータ `iroha_core` でレシートを取り込み、DA の出力を実行します
   Torii (`crates/iroha_core/src/queue.rs` e `crates/iroha_core/src/block.rs`)。
3. **永続化/インデックス** パラケオ WSV は、妥協の相談に応じます
   急速 (`iroha_core/src/wsv/mod.rs`)。
4. **Adicoes RPC em Torii** エンドポイントのリスト/コンサルタ/プロバソブ
   `/v1/da/commitments`。
5. **統合テストと治具** 検証、ワイヤレイアウト、耐フラックス防止
   em `integration_tests/tests/da/commitments.rs`。

## 1. Adicoes ao データモデル

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

- `KzgCommitment` は 48 バイトを使用して `iroha_crypto::kzg` を再利用します。
  Quando ausente、cai para provas Merkle apenas。
- `proof_scheme` レーンのカタログを作成します。レーン マークル レジェイタム ペイロード KZG
  enquanto レーン `kzg_bls12_381` exigem コミットメント KZG nao ゼロ。 Torii 実物
  したがって、マークルとレーンの設定を KZG で行う必要があります。
- `KzgCommitment` は 48 バイトを使用して `iroha_crypto::kzg` を再利用します。
  マークルをすべて使用し、マークルを最大限に活用してください。
- `proof_digest` 事前統合 DA-5 PDP/PoTR パラメータ メッセージ レコード
  生体内での米国のパラマンターブロブのサンプリングスケジュールを列挙します。

### 1.2 ヘッダーを拡張する

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

O ハッシュ ド バンドル エントラ タント ハッシュ ド ブロック カント ナ メタデータ
`SignedBlockWire`。 Quando um bloco nao carrega mados DA、o Campo fica `None` para実装ノート: `BlockPayload` または `BlockBuilder` 透過的なアゴラ
expoem セッター/ゲッター `da_commitments` (ver `BlockBuilder::set_da_commitments`)
e `SignedBlock::set_da_commitments`)、entao ホスト ポデム anexar um バンドル
ブロックの前に準備を整えます。 Todos os ヘルパー デイザム オ カンポ エム `None`
Ate que Torii encadeie は、レアをバンドルします。

### 1.3 ワイヤのエンコーディング

- `SignedBlockWire::canonical_wire()` ヘッダー Norito パラメタ
  `DaCommitmentBundle` 存在する取引リストをすぐに確認してください。 ○
  バイト・デ・バーサオ・e `0x01`。
- `SignedBlockWire::decode_wire()` rejeita バンドル cujo `version` seja
  政治 Norito の説明、`norito.md`。
- `block::Hasher` でハッシュが存在するかどうかを確認します。顧客
  ガンハムやノボ カンポの存在するワイヤー フォーマットを復号化することもできます
  自動ヘッダー Norito が表示されます。

## 2. ブロコスの生産

1. Torii 最終的な `DaIngestReceipt` を公開ファイルに取り込みます
   インテル (`iroha_core::gossiper::QueueMessage::DaReceipt`)。
2. `PendingBlocks` coleta todos os の領収書 cujo `lane_id` 対応する ao bloco
   em construcao、deduplicando por `(lane_id, client_blob_id, manifest_hash)`。
3. 安全性を確保し、ビルダーが妥協するかどうかを決定します (lane_id, epoch,
   sequence)` パラメータ、ハッシュ決定性、コードフィカ、バンドル コム、コーデック
   Norito、`da_commitments_hash` と一致します。
4. バンドルを完全に装備し、WSV を送信して、ブロック デントロ デを送信する
   `SignedBlockWire`。

ファルハールを確認し、領収書を永久に発行し、近くで確認してください

テンタティバOS攻略; o ビルダー レジストラ o ultimo `sequence` はレーンを含む
パラエビタールのリプレイ。

## 3. 上位 RPC とコンサルテーション

Torii トレス エンドポイントを公開します:

|ロタ |メトド |ペイロード |メモ |
|------|--------|-----------|----------|
| `/v1/da/commitments` | `POST` | `DaCommitmentQuery` (レーン/エポック/シーケンスの範囲フィルター、ページ) | Retorna `DaCommitmentPage` 合計、ハッシュ デ ブロコの妥協。 |
| `/v1/da/commitments/prove` | `POST` | `DaCommitmentProofRequest` (レーン + マニフェスト ハッシュ トゥプラ `(epoch, sequence)`)。 | com `DaCommitmentProof` (レコード + カミーニョ マークル + ハッシュ デ ブロコ) に返信してください。 |
| `/v1/da/commitments/verify` | `POST` | `DaCommitmentProof` |ヘルパーはステートレスなクエリを実行し、ハッシュ デ ブロコと検証を含む計算を実行します。 SDK は `iroha_crypto` のポデム リンカー ディレクトリにあります。 |

Todos OS ペイロードがすすり泣き `iroha_data_model::da::commitment` で表示されます。 OSルーター
Torii モンタム OS ハンドラー、ラド、ドス、エンドポイントの取り込み DA 存在パラ
トークン/mTLS の政治を再利用します。

## 4. 顧客の権利を含むプロバス- O produtor de blocos constroi uma arvore Merkle binaria sobre a lista
  `DaCommitmentRecord` のシリアル番号。ライズアリメンタ`da_commitments_hash`。
- `DaCommitmentProof` エンパコタまたは記録をすべて記録します
  `(sibling_hash, position)` para que verificadores がライズを再解釈します。として
  provas tambem incluem o hash do bloco e o header assinado para que clientes
  ファイナリティを有効にします。
- CLI ヘルパー (`iroha_cli app da prove-commitment`) は、CICL に関与します。
  Norito/hex パラオペラドールの説明を要求/検証します。

## 5. ストレージとインデックスの作成

O WSV armazena compromisos em uma コラム ファミリー dedicada com Chave
`manifest_hash`。インデックス secundarios cobrem `(lane_id, epoch)` e
`(lane_id, sequence)` は、完全なEV項目バリアント バンドルを参照します。カダ
ラストレイア・アルトゥラ・ド・ブロコ・ケ・オ・セロウを記録し、追いつくことを許可してください
ブロックログのインデックスを迅速に再構築します。

## 6. テレメトリアと観察

- `torii_da_commitments_total` インクリメント Quando um bloco sela ao menos um
  記録する。
- `torii_da_commitment_queue_depth` rastreia 領収書アガードバンドル (por
  レーン）。
- O ダッシュボード Grafana `dashboards/grafana/da_commitments.json` 視覚化
  ブロック、フィラデルフィアの広範囲にわたるスループットを含む
  DA-3 のゲートをリリースすると、オーディオコンポルタメントが完成します。

## 7. 精巣戦略

1. **`DaCommitmentBundle` e のエンコード/デコードに関する **テスト ユニタリオ**
   アチュアリザコエス デ デリバカオ ドゥ ハッシュ デ ブロコ。
2. **フィクスチャ ゴールデン** 泣き叫ぶ `fixtures/da/commitments/` キャプチャランド バイト カノニコス
   Provas Merkle をバンドルしてください。
3. **検証テスト** com dois validadores, ingerindo blob de exemplo e
   Verificando que ambos os nos concordam no conteudo dobundle e nas respostas
   クエリ/証明。
4. **顧客レベルのテスト** em `integration_tests/tests/da/commitments.rs`
   (Rust) que Chamam `/prove` e verificam a prova sem falar com Torii。
5. **Smoke CLI** com `scripts/da/check_commitments.sh` パラメータ ツール
   オペラドールの再現。

## 8. 計画的なロールアウト

|ファセ |説明 |基準 |
|------|-----------|--------|
| P0 - データ モデルをマージ |統合型 `DaCommitmentRecord`、ヘッダーとブロックのコーデック Norito の設定。 | `cargo test -p iroha_data_model` verde com novas 備品。 |
| P1 - ワイヤリングコア/WSV | Encadear ロジック + ブロック ビルダー、永続化インデックス、エクスポート ハンドラー RPC。 | `cargo test -p iroha_core`、`integration_tests/tests/da/commitments.rs` バンドル証明のパスサム コム アサーション。 |
| P2 - オペレーターのツール | Entregar ヘルパー CLI、ダッシュボード Grafana は、ドキュメントの検証を更新します。 | `iroha_cli app da prove-commitment` 開発ネットに反する機能。ダッシュボードのモストラ・ダドス・アオ・ヴィヴォ。 |
| P3 - ガバナンカの門 | `iroha_config::nexus` のマルカダで、妥協を要求するブロックの有効性を確認してください。 | DA-3 と COMPLETADO のステータスとロードマップの更新を記録します。 |

## ペルグンタス・アベルタス1. **KZG とマークルのデフォルト** - Devemos のコミットメント KZG em blob
   ペケノス・パラ・レドゥジル・オ・タマンホ・ド・ブロコ?プロポスタ: マンター `kzg_commitment`
   `iroha_config::da.enable_kzg` 経由のオプションの電子ゲート。
2. **シーケンス ギャップ** - 順序に従ってレーンを許可しますか?オ・プラノ・アチュアル・レジェイタ・ギャップ
   緊急時の政府 `allow_sequence_skips` の一斉射撃。
3. **ライトクライアント キャッシュ** - SDK の継続的なキャッシュ SQLite レベルの検証に時間がかかりません。
   DA-8のペンデンテ。

レスポンダーは、RASCUNHO で DA-3 を移動して PR を実行します (エステ
documento) para EM ANDAMENTO quando o trabalho de codigo Comecar。