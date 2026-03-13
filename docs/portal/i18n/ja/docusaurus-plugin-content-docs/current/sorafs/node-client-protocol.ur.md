---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/node-client-protocol.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SoraFS پوڈ ↔ کلائنٹ پروٹوکول

یہ گائیڈ پروٹوکول کی 正規 تعریف کو
[`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)
要約する کرتی ہے۔バイトレベルの Norito レイアウト、変更ログ、アップストリーム
仕様ポータル コピー SoraFS ランブックの操作上のハイライト
قریب رکھتی ہے۔

## プロバイダーの広告と検証

SoraFS プロバイダー `ProviderAdvertV1` ペイロード (دیکھیں)
`crates/sorafs_manifest::provider_advert`) ゴシップ ہیں 管理されたオペレーター
نے サイン کیے ہوتے ہیں۔広告発見メタデータ ガードレール ピン ピン
マルチソース オーケストレーター ランタイムの強化

- **生涯** — `issued_at < expires_at ≤ issued_at + 86,400 s`。プロバイダー
  ہر 12 گھنٹے میں リフレッシュ کرنا چاہیے۔
- **機能 TLV** — TLV リスト トランスポート機能がアドバタイズします (Torii、
  QUIC+Noise、SoraNet リレー、ベンダー拡張機能）。未知のコード
  `allow_unknown_capabilities = true` پر スキップ کیا جا سکتا ہے، GREASE ガイダンス کے مطابق۔
- **QoS ヒント** — `availability` 層 (ホット/ウォーム/コールド)、最大取得遅延、
  同時実行制限とオプションのストリーム予算。 QoS 監視テレメトリ
  入学手続き 監査検査
- **エンドポイントとランデブー トピック** — TLS/ALPN メタデータの具体的な内容
  サービス URL 検出トピック クライアント ガード セット 購読
  うわー
- **パス ダイバーシティ ポリシー** — `min_guard_weight`、AS/プール ファンアウト キャップ
  `provider_failure_threshold` 決定論的マルチピアフェッチの実行
- **プロファイル識別子** — プロバイダーは正規ハンドルを公開します
  (`sorafs.sf1@1.0.0`);オプションの `profile_aliases` クライアント
  移行 میں مدد دیتے ہیں۔

検証ルールのゼロステーク、空の機能/エンドポイント/トピックリストの順序が間違っています
ライフタイム数、QoS ターゲットが欠落しています、拒否、拒否されます入学封筒の広告
提案機関 (`compare_core_fields`) 比較、ゴシップ更新
ありがとうございます

### 範囲フェッチ拡張機能

範囲対応プロバイダーのメタデータの説明:

|フィールド |目的 |
|------|-----------|
| `CapabilityType::ChunkRangeFetch` | `max_chunk_span`、`min_granularity` アライメント/プルーフ フラグが宣言されます。 |
| `StreamBudgetV1` |オプションの同時実行/スループット エンベロープ (`max_in_flight`、`max_bytes_per_sec`、オプションの `burst`)。射程能力が必要です|
| `TransportHintV1` |順序付きトランスポート設定 (`torii_http_range`、`quic_stream`、`soranet_relay`)。優先順位 `0–15` ہیں اور 重複拒否 ہوتے ہیں۔ |

ツールのサポート:

- プロバイダーの広告パイプライン、範囲機能、ストリーム予算、トランスポート ヒント
  検証する 監査する 決定論的なペイロードが放出する
- `cargo xtask sorafs-admission-fixtures` 正規のマルチソース広告
  フィクスチャのダウングレード `fixtures/sorafs_manifest/provider_admission/` バンドル ہے۔
- 範囲対応広告 `stream_budget` `transport_hints` CLI/SDK を省略
  ローダー スケジュール管理 拒否 承認 マルチソース ハーネス
  Torii 入場期待値が揃っています

## ゲートウェイ範囲エンドポイント

ゲートウェイの決定論的な HTTP リクエストは、広告メタデータとミラーリングを受け入れます。
ありがとうございます

### `GET /v2/sorafs/storage/car/{manifest_id}`

|要件 |詳細 |
|-----------|-----------|
| **ヘッダー** | `Range` (チャンク オフセットに合わせて配置された単一ウィンドウ)、`dag-scope: block`、`X-SoraFS-Chunker`、オプションの `X-SoraFS-Nonce`、base64 `X-SoraFS-Stream-Token`。 |
| **回答** | `206` と `Content-Type: application/vnd.ipld.car`、`Content-Range` は、サーブド ウィンドウを表示します。 `X-Sora-Chunk-Range` メタデータは、チャンカー/トークン ヘッダーとエコーを表示します。 |
| **障害モード** |範囲が正しくありません `416`、欠落/無効なトークン `401`、ストリーム/バイト バジェットが超過 `429`۔ |

### `GET /v2/sorafs/storage/chunk/{manifest_id}/{digest}`

単一チャンクのフェッチ、ヘッダー、および決定論的なチャンク ダイジェスト再試行
フォレンジックのダウンロード、分析、車のスライス、分析、分析、分析、分析、分析、分析、分析、分析、分析、分析、分析、分析、分析

## マルチソース オーケストレーター ワークフロー

SF-6 マルチソースフェッチが有効化 (`sorafs_fetch` 経由の Rust CLI、`sorafs_fetch` 経由の SDK
`sorafs_orchestrator`):1. **入力を収集** — マニフェスト チャンク プランをデコードし、最新の広告をプルします
   オプションのテレメトリ スナップショット (`--telemetry-json` または `TelemetrySnapshot`)
2. **スコアボードの作成** — `Orchestrator::build_scoreboard` の適格性の評価
   拒否理由の記録`sorafs_fetch --scoreboard-out`
   JSON 永続化
3. **スケジュール チャンク** — `fetch_with_scoreboard` (`--plan`) 範囲制約
   ストリーム バジェット、再試行/ピア キャップ (`--retry-budget`、`--max-peers`) を適用します。
   リクエスト リクエスト マニフェスト スコープ ストリーム トークンの発行 リクエスト
4. **領収書の確認** — 出力 `chunk_receipts` `provider_reports` メッセージ
   فوتے ہیں؛ CLI 概要 `provider_reports`、`chunk_receipts`、
   `ineligible_providers` 証拠バンドルを保持します。

オペレーター/SDK のエラー:

|エラー |説明 |
|------|-----------|
| `no providers were supplied` |フィルタリング بعد کوئی 対象となるエントリ نہیں۔ |
| `no compatible providers available for chunk {index}` |チャンクの範囲、予算の不一致、 |
| `retry budget exhausted after {attempts}` | `--retry-budget` 失敗したピアを排除します|
| `no healthy providers remaining` |繰り返される失敗 プロバイダーが無効になる ہو گئے۔ |
| `streaming observer failed` |ダウンストリーム CAR ライターが中止されます|
| `orchestrator invariant violated` |トリアージ マニフェスト スコアボード テレメトリ スナップショット CLI JSON キャプチャ|

## テレメトリーの証拠

- オーケストレーターのメトリクス:  
  `sorafs_orchestrator_active_fetches`、`sorafs_orchestrator_fetch_duration_ms`、
  `sorafs_orchestrator_retries_total`、`sorafs_orchestrator_provider_failures_total`
  (マニフェスト/リージョン/プロバイダーのタグ)。ダッシュボードと艦隊の情報
  パーティション 構成 構成 CLI フラグ `telemetry_region` 構成
- CLI/SDK フェッチ概要、永続化されたスコアボード JSON、チャンク レシート
  プロバイダーレポート شامل ہوتے ہیں جو SF-6/SF-7 Gates کے ロールアウトバンドル میں شامل ہونے چاہییں۔
- ゲートウェイ ハンドラー `telemetry::sorafs.fetch.lifecycle|retry|provider_failure|error`
  SRE ダッシュボード、オーケストレーターの決定、サーバーの動作を公開する
  相関する

## CLI および REST ヘルパー

- `iroha app sorafs pin list|show`、`alias list`、`replication list` ピン レジストリ
  REST エンドポイントは、監査証拠をラップし、構成証明ブロックをラップします。
  raw Norito JSON 印刷
- `iroha app sorafs storage pin` 国際 `torii /v2/sorafs/pin/register` Norito 国際 JSON
  マニフェスト オプションのエイリアス証明 後継者は受け入れます奇形な
  プルーフ `400`、古いプルーフ `503`、`Warning: 110`、完全に期限切れのプルーフ
  پر `412`۔
- REST エンドポイント (`/v2/sorafs/pin`、`/v2/sorafs/aliases`、`/v2/sorafs/replication`)
  認証構造 クライアントの最新のブロック ヘッダー
  データ検証

## 参考文献

- 正規仕様:
  [`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)
- Norito タイプ: `crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`
- CLI ヘルパー: `crates/iroha_cli/src/commands/sorafs.rs`、
  `crates/sorafs_car/src/bin/sorafs_fetch.rs`
- オーケストレーター クレート: `crates/sorafs_orchestrator`
- ダッシュボード パック: `dashboards/grafana/sorafs_fetch_observability.json`