---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/node-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: ノードプラン
タイトル: Plano deimpleacao do nodo SoraFS
Sidebar_label: ノードの実装計画
説明: ストレージ SF-3 のロードマップをマルコス、タレファス、テストの対象となるアクションに変換します。
---

:::note フォンテ カノニカ
エスタ・ページナ・エスペルハ`docs/source/sorafs/sorafs_node_plan.md`。マンテンハは、スフィンクスの代替品として記録されたものを食べました。
:::

SF-3 は、最初のクレート実行 `sorafs-node` 変換プロセス Iroha/Torii およびストレージ SoraFS を証明しました。 [ストレージの管理](node-storage.md)、[政治的承認の証明](provider-admission-policy.md)、[ストレージ容量のマーケットプレイスのロードマップ](storage-capacity-marketplace.md) および一連のエントリを使用します。

## Escopo alvo (マルコ M1)

1. **チャンク ストアを統合します。** Envolver `sorafs_car::ChunkStore` コム バックエンドの永続的なチャンク バイト、マニフェストは、PoR のディレクトリーの管理を行いません。
2. **ゲートウェイのエンドポイント** エンドポイント HTTP Norito をピンの送信、チャンクのフェッチ、記憶域のテレメトリ管理、プロセス Torii でエクスポートします。
3. **構成の配管** 構成 `SoraFsStorage` (フラグ ハビリタド、容量、ディレトリオス、コンコルレンシアの制限) を `iroha_config`、`iroha_core`、`iroha_torii` 経由で接続します。
4. **クォータ/議題** ディスコ/パラレリズムの制限をインポートし、ペロ オペラドールと必要なバックプレッシャーをインポートします。
5. **Telemetria.** ピンの実行、フェッチの遅延、チャンクの取得、PoR の実行結果などのメトリクス/ログを送信します。

## ケブラ デ トラバーリョ

### A. モジュールによる計算

|タレファ |ドノ |メモ |
|------|------|------|
| Criar `crates/sorafs_node` モジュール: `config`、`store`、`gateway`、`scheduler`、`telemetry`。 |ストレージチーム | Torii に関する情報を再エクスポートします。 |
| `StorageConfig` を `SoraFsStorage` に実装します (ユーザー -> 実際の -> デフォルト)。 |ストレージ チーム / 構成 WG | Norito/`iroha_config` 永久カメラの決定性を保証します。 |
| Fornecer uma ファサード `NodeHandle` que Torii usa パラ サブメーター ピン/フェッチ。 |ストレージチーム |ストレージ内部のカプセル化と配管は非同期です。 |

### B. チャンクストアの永続化

|タレファ |ドノ |メモ |
|------|------|------|
|ディスコのバックエンドを構築し、`sorafs_car::ChunkStore` ディスコのマニフェスト インデックスを作成します (`sled`/`sqlite`)。 |ストレージチーム |レイアウト決定性: `<data_dir>/<manifest_cid>/chunk_{idx}.bin`。 |
| Manter メタダドス PoR (arvores 64 KiB/4 KiB) は `ChunkStore::sample_leaves` を使用します。 |ストレージチーム |サポートリプレイアポス再起動;ファルハ・ラピド・エム・コルプカオ。 |
|起動なしの統合リプレイを実装します (マニフェストの再ハッシュ、不完全なポダーピン)。 |ストレージチーム | Torii を開始し、ターミナルを再生します。 |

### C. エンドポイントとゲートウェイ

|エンドポイント |コンポルタメント |タレファス |
|----------|--------------|----------|
| `POST /sorafs/pin` | Aceita `PinProposalV1`、検証マニフェスト、enfileira ingestao、応答 com o CID マニフェスト。 |チャンカーの有効性、クォータのインポート、チャンク ストア経由のデータのストリーム。 |
| `GET /sorafs/chunks/{cid}` + 範囲のクエリ |チャンク com ヘッダーの Servir バイト `Content-Chunker`;範囲を特定してください。 |ユーザー スケジューラー + ストリームのオルカメント (SF-2d 範囲の容量)。 |
| `POST /sorafs/por/sample` | Rodar は、PoR パラメータのマニフェストとレトルナーバンドルを提供します。 |チャンク ストア、レスポンダー com ペイロード Norito JSON を再利用します。 |
| `GET /sorafs/telemetry` |再開: キャパシダード、スセッソ PoR、エラー デ フェッチのコンタドール。 |ダッシュボード/オペレーターのフォルネサーダドス。 |

O `sorafs_node::por` 経由の相互接続 PoR としてのランタイムパスの配管: o トラッカーレジストラカード `PorChallengeV1`、`PorProofV1` e `AuditVerdictV1` はメトリクスとしてパラメトリック `CapacityMeter` 政府の論理を確認Torii別注。 [crates/sorafs_node/src/scheduler.rs:147]

実装上の注意点:

- スタック Axum de Torii com ペイロード `norito::json` を使用します。
- アディシオン スキーマ Norito パラ レスポスト (`PinResultV1`、`FetchErrorV1`、テレメトリア構造体)。- `/v2/sorafs/por/ingestion/{manifest_digest_hex}` は、`sorafs_node::NodeHandle::por_ingestion_status`、Torii レジストラ OS ゲージを介して、エポカ/デッドラインの未解決の資金を事前に公開し、OS のタイムスタンプを確認し、成功/ファルハの証明を取得します。 `torii_sorafs_por_ingest_backlog`/`torii_sorafs_por_ingest_failures_total` パラ ダッシュボード。 [crates/sorafs_node/src/lib.rs:510] [crates/iroha_torii/src/sorafs/api.rs:1883] [crates/iroha_torii/src/routing.rs:7244] [crates/iroha_telemetry/src/metrics.rs:5390]

### D. スケジューラとクォータの蓄積

|タレファ |デタルヘス |
|-----|----------|
|クォータ・デ・ディスコ |ラストレアはディスコでバイトします。 rejeitar novos ピン ao エクシーダー `max_capacity_bytes`。 Fornecer は、未来の政治に悪影響を及ぼします。 |
|フェッチのコンコルンシア | Semaforo グローバル (`max_parallel_fetches`) は、SF-2d 範囲のキャップを証明するための機能を提供します。 |
|フィラ・デ・ピン |保留中のジョブの制限。エンドポイント Norito のステータスを詳細にエクスポートします。 |
|カデンシア PoR | `por_sample_interval_secs` のバックグラウンド ディリギドのワーカー。 |

### E. テレメトリアのログ記録

メトリカ (Prometheus):

- `sorafs_pin_success_total`、`sorafs_pin_failure_total`
- `sorafs_chunk_fetch_duration_seconds` (ヒストグラム com ラベル `result`)
- `torii_sorafs_storage_bytes_used`、`torii_sorafs_storage_bytes_capacity`
- `torii_sorafs_storage_pin_queue_depth`、`torii_sorafs_storage_fetch_inflight`
- `torii_sorafs_storage_fetch_bytes_per_sec`
- `torii_sorafs_storage_por_inflight`
- `torii_sorafs_storage_por_samples_success_total`、`torii_sorafs_storage_por_samples_failed_total`

ログ/イベント:

- Telemetria Norito 政府の摂取に関する評価 (`StorageTelemetryV1`)。
- 連続稼働率が 90% を超え、PoR がしきい値を超えていることを警告します。

### F. 精巣戦略

1. **テストユニット。** 永続化はチャンクストア、クォータ計算、不変量はスケジューラーを実行します (ver `crates/sorafs_node/src/scheduler.rs`)。
2. **統合テスト** (`crates/sorafs_node/tests`)。ピン -> フェッチラウンドトリップ、回復アポス再起動、割り当て量の再設定、PoR のプロバスの検証。
3. **統合 Torii のテスト。** Rodar Torii com storage habilitado、exercitar エンドポイント HTTP via `assert_cmd`。
4. **計画のロードマップ** ディスコの実行シミュレーション、IO レント、プローベドールの実行を訓練します。

## 依存関係

- 承認政治 SF-2b - 承認前に封筒を検証するノードを保証します。
- 容量性SF-2cのマーケットプレイス - 容量性デ​​クララコエスを宣言するリガーテレメトリアデボルタ。
- SF-2d の広告の範囲 - 範囲の容量を消費し、ストリームの容量を制限します。

## マルコの基準

- `cargo run -p sorafs_node --example pin_fetch` 機能コントラフィクスチャロケール。
- Torii compila com `--features sorafs-storage` は完全なテストです。
- Documentacao ([ストレージのノード](node-storage.md)) デフォルトの設定 + CLI の例。オペレーター対応のランブック。
- Telemetria visivel em ダッシュボードのステージング; PoR の容量に関する警告を設定します。

## 文書作成業務に参加する

- [ストレージノードの参照](node-storage.md) com のデフォルト設定、CLI およびトラブルシューティングの使用を可能にします。
- 管理者は、SF-3 evolui に準拠した [オペラ座のランブック](node-operations.md) を実行します。
- エンドポイントに関する API 参照 `/sorafs/*` を公開し、ポータルの展開を解除し、マニフェストに接続します。 OpenAPI は、Torii のハンドラーを割り当てます。