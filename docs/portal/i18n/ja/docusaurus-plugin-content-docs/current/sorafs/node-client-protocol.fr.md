---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/node-client-protocol.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# プロトコル番号 ↔ クライアント SoraFS

プロトコルの定義に関するガイドの履歴書
[`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)。
仕様上流のレイアウト Norito ニボー オクテットなどを利用
変更ログ;ポルテール ギャルドのポイント操作のコピー
Runbook SoraFS の残り。

## 広告と検証

Les fournisseurs SoraFS diffusent des payloads `ProviderAdvertV1` (voir)
`crates/sorafs_manifest::provider_advert`) 政府の運用に関する署名。レ
オーケストラの音楽と音楽のメタドンを固定する広告
マルチソースのアップリケを実行します。

- **有効期間** — `issued_at < expires_at ≤ issued_at + 86 400 s`。レ
  fournisseurs doivent rafraîchir は les 12 heures を宣伝しています。
- **TLV de capacités** — TLV の機能に関する通知のリスト
  (Torii、QUIC+Noise、リレー SoraNet、拡張機能ベンダー)。レ コード インコンナス
  peuvent être ignorés lorsque `allow_unknown_capabilities = true`、en suivant
  推奨グリース。
- **インデックス QoS** — `availability` 層 (ホット/ウォーム/コールド)、最大遅延
  回復、同意の制限、およびストリームのオプションの予算。 QoS を実行する
  入学審査の監視と監査を行います。
- **エンドポイントとランデブーのトピック** — 平均的なサービス具体的な URL
  TLS/ALPN のメタドン、およびクライアントに関するトピックの詳細
  doivent s'abonner lors de la construction des ガードセット。
- **化学の多様な政治** — `min_guard_weight`、ファンアウトのキャップ
  AS/プールと `provider_failure_threshold` レンデントの可能なファイルのフェッチ
  マルチピアを決定します。
- **プロファイルの識別子** — 暴露者ファイル ハンドルの情報
  canonique (例: `sorafs.sf1@1.0.0`) ; `profile_aliases` オプション アシスタント
  移民の顧客。

ステークゼロを拒否する検証の規則、機能のリスト動画、
エンドポイント、トピック、耐久性、目的、QoS マンクォント。レ
広告と提案の封筒を比較します
(`compare_core_fields`) 前衛的なディフューザー デ ミゼア ジュール。

### パープページを取得するための拡張機能

供給されるメタドンを含む平均容量範囲:

|チャンピオン |目的 |
|------|----------|
| `CapabilityType::ChunkRangeFetch` | `max_chunk_span`、`min_granularity` およびアラインメント/プレユーブのフラグを宣言します。 |
| `StreamBudgetV1` |同時実行/スループットのエンベロープ オプション (`max_in_flight`、`max_bytes_per_sec`、`burst` オプション)。容量範囲が必要です。 |
| `TransportHintV1` |輸送優先順位 (例: `torii_http_range`、`quic_stream`、`soranet_relay`)。 `0–15` と二重の問題を優先してください。 |

サポートツール:

- パイプラインの広告容量範囲、ストリームの検証
  予算と輸送に関するヒント ペイロードを事前に決定するためのヒント
  監査。
- `cargo xtask sorafs-admission-fixtures` マルチソース広告の再グループ化
  ダウングレード時の試合結果の標準化
  `fixtures/sorafs_manifest/provider_admission/`。
- 広告範囲は `stream_budget` または `transport_hints` です
  ローダー CLI/SDK の事前計画、調整されたハーネスの拒否
  マルチソース シュール アテント アドミッション Torii。

## ゲートウェイのエンドポイント範囲

要求を受け入れるゲートウェイ HTTP 決定要求を反映するゲートウェイ
広告のメタドンネ。

### `GET /v1/sorafs/storage/car/{manifest_id}`

|エクシジェンス |詳細 |
|----------|----------|
| **ヘッダー** | `Range` (チャンクのオフセットに関する固有の調整)、`dag-scope: block`、`X-SoraFS-Chunker`、`X-SoraFS-Nonce` オプション、および `X-SoraFS-Stream-Token` Base64 義務。 |
| **返答** | `206` avec `Content-Type: application/vnd.ipld.car`、`Content-Range` フェネット サービスの定義、メタドン、`X-Sora-Chunk-Range`、およびヘッダー チャンカー/トークンの監視。 |
| **モード デシェック** | `416` プラージュ・マル・アライン、`401` トークンのマンカント/無効、`429` lorsque les Budgets ストリーム/オクテット・ソント・デパス。 |

### `GET /v1/sorafs/storage/chunk/{manifest_id}/{digest}`

ヘッダーのチャンクを取得し、さらにダイジェストを取得します
チャンク。再検査や犯罪捜査の利用を可能にする
CAR のスライスは無駄になります。

## オーケストラのワークフロー マルチソース

マルチソース SF-6 のフェッチを有効にします (`sorafs_fetch` 経由の CLI Rust、
SDK (`sorafs_orchestrator` 経由):1. **エントリの収集** — マニフェストの計画とチャンクのデコーダー、レキュペラー
   広告、オプション、スナップショットの通過者など
   (`--telemetry-json` または `TelemetrySnapshot`)。
2. **スコアボードの作成** — `Orchestrator::build_scoreboard` の値
   登録の許可と登録 ; `sorafs_fetch --scoreboard-out`
   JSON ファイルを永続化します。
3. **Planifier ファイル チャンク** — `fetch_with_scoreboard` (ou `--plan`) ファイルをインポーズします
   制約の範囲、バジェット ストリーム、再試行の上限/ピア (`--retry-budget`、
   `--max-peers`) マニフェストによるストリーム トークン スコープの設定
   リクエストする。
4. **検証結果** — `chunk_receipts` などを含む出撃
   `provider_reports`;履歴書 CLI 永続 `provider_reports`、
   `chunk_receipts` と `ineligible_providers` プルーヴの束。

操作/SDK のエラー :

|エラー |説明 |
|----------|---------------|
| `no providers were supplied` | Aucune entrée éligible after filtrage. |
| `no compatible providers available for chunk {index}` |特定のチャンクに予算を注ぎ込むためのプラージュのエラー。 |
| `retry budget exhausted after {attempts}` | Augmentez `--retry-budget` ou évincez les Peers en échec. |
| `no healthy providers remaining` | Tous les fournisseurs は、après des échecs repétés を無効にします。 |
| `streaming observer failed` | Le Writer CAR は下流で avorté を実行します。 |
| `orchestrator invariant violated` | Capturez マニフェスト、スコアボード、テレメトリーおよび JSON CLI のトリアージ用スナップショット。 |

## テレメトリーとプルーヴ

- オーケストラの指揮官のメトリック:  
  `sorafs_orchestrator_active_fetches`、`sorafs_orchestrator_fetch_duration_ms`、
  `sorafs_orchestrator_retries_total`、`sorafs_orchestrator_provider_failures_total`
  (マニフェスト/地域/Fournisseur のタグ)。 `telemetry_region` ja の定義
  フラグ CLI を介して、フロットのパーティション作成ダッシュボードを設定します。
- スコアボードの JSON 永続化を含む CLI/SDK フェッチの履歴、履歴
  バンドルとボイジャーのチャンクと関係を 4 つ共有する
  ロールアウト プール レ ゲート SF-6/SF-7。
- ハンドラー ゲートウェイ公開 `telemetry::sorafs.fetch.lifecycle|retry|provider_failure|error`
  ダッシュボードに SRE の相関関係を示し、決定をオーケストレーターとして実行する
  コンポートメントサーバー。

## CLI と REST の補助

- `iroha app sorafs pin list|show`、`alias list` および `replication list` 保護ファイル
  エンドポイント REST のピン レジストリと Norito JSON ブルート アベック ブロックの実装
  d'attestation pour l'audit。
- `iroha app sorafs storage pin` および `torii /v1/sorafs/pin/register` は受け入れられます
  マニフェスト Norito ou JSON、および証明、エイリアス オプション、および後継者。
  不正な形式の証明 `400`、廃止された証明 `503` avec
  `Warning: 110`、および証明の有効期限は `412` です。
- エンドポイント REST (`/v1/sorafs/pin`、`/v1/sorafs/aliases`、
  `/v1/sorafs/replication`) 構造証明に含まれる情報
  クライアントは、ブロックの前のヘッダーを確認します。

## 参照

- 標準仕様 :
  [`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)
- タイプ Norito : `crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`
- 補佐官 CLI : `crates/iroha_cli/src/commands/sorafs.rs`、
  `crates/sorafs_car/src/bin/sorafs_fetch.rs`
- クレートオーケストレーター : `crates/sorafs_orchestrator`
- パックダッシュボード: `dashboards/grafana/sorafs_fetch_observability.json`