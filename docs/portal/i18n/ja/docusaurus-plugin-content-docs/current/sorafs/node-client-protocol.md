<!-- Auto-generated stub for Japanese (ja) translation. Replace this content with the full translation. -->

---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/node-client-protocol.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# SoraFS ノード ↔ クライアントプロトコル

このガイドは、プロトコルの正規定義を
[`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)
で要約したものです。バイトレベルの Norito レイアウトと変更履歴は upstream
仕様を参照し、ポータル版は SoraFS の他の runbook と並べて運用の要点を
まとめています。

## プロバイダー広告と検証

SoraFS のプロバイダーは、ガバナンス対象のオペレーターが署名した
`ProviderAdvertV1` ペイロード（`crates/sorafs_manifest::provider_advert` を参照）を
ゴシップします。広告はディスカバリのメタデータと、マルチソース
オーケストレーターが実行時に強制するガードレールを固定します。

- **有効期限** — `issued_at < expires_at ≤ issued_at + 86,400 s`。プロバイダーは
  12 時間ごとに更新する必要があります。
- **Capability TLV** — TLV リストがトランスポート機能（Torii、QUIC+Noise、
  SoraNet リレー、ベンダー拡張）を広告します。`allow_unknown_capabilities = true`
  の場合、GREASE ガイダンスに従って未知コードをスキップできます。
- **QoS ヒント** — `availability` tier（Hot/Warm/Cold）、最大取得レイテンシ、
  同時実行上限、任意のストリーム予算。QoS は観測されたテレメトリと整合し、
  アドミッションで監査されます。
- **Endpoints と rendezvous topics** — TLS/ALPN メタデータ付きの具体的なサービス
  URL と、ガードセット構築時にクライアントが購読すべきディスカバリ
  トピック。
- **パス多様性ポリシー** — `min_guard_weight`、AS/プールの fan-out 上限、
  `provider_failure_threshold` が決定的なマルチピア fetch を可能にします。
- **プロファイル識別子** — プロバイダーは正規ハンドル（例: `sorafs.sf1@1.0.0`）を
  公開する必要があります。`profile_aliases` は旧クライアントの移行を支援します。

検証ルールは、ゼロステーク、空の capability/endpoint/topic リスト、順序違いの
有効期限、欠落した QoS ターゲットを拒否します。アドミッションのエンベロープは、
更新をゴシップする前に広告と提案の本文（`compare_core_fields`）を比較します。

### レンジ取得の拡張

レンジ対応プロバイダーは次のメタデータを含みます。

| フィールド | 目的 |
|-----------|------|
| `CapabilityType::ChunkRangeFetch` | `max_chunk_span`、`min_granularity`、整列/証明フラグを宣言。 |
| `StreamBudgetV1` | 任意の同時実行/スループット枠（`max_in_flight`, `max_bytes_per_sec`, 省略可 `burst`）。レンジ機能が必要。 |
| `TransportHintV1` | 順序付きのトランスポート優先度（例: `torii_http_range`, `quic_stream`, `soranet_relay`）。優先度は `0–15`、重複は拒否。 |

ツール面のサポート:

- プロバイダー広告のパイプラインは、監査用の決定的ペイロードを生成する前に、
  レンジ機能、ストリーム予算、トランスポートヒントを検証する必要があります。
- `cargo xtask sorafs-admission-fixtures` は、正規のマルチソース広告とダウングレード
  フィクスチャを `fixtures/sorafs_manifest/provider_admission/` にバンドルします。
- レンジ対応広告が `stream_budget` または `transport_hints` を省略すると、CLI/SDK
  ローダーがスケジューリング前に拒否し、Torii のアドミッション期待と
  マルチソース・ハーネスの整合を保ちます。

## Gateway のレンジエンドポイント

Gateways は広告メタデータを反映した決定的な HTTP リクエストを受け付けます。

### `GET /v1/sorafs/storage/car/{manifest_id}`

| 要件 | 詳細 |
|------|------|
| **Headers** | `Range`（chunk オフセットに整列した単一ウィンドウ）、`dag-scope: block`、`X-SoraFS-Chunker`、任意の `X-SoraFS-Nonce`、必須の base64 `X-SoraFS-Stream-Token`。 |
| **Responses** | `206` と `Content-Type: application/vnd.ipld.car`、提供ウィンドウを示す `Content-Range`、`X-Sora-Chunk-Range` メタデータ、chunker/token ヘッダーのエコー。 |
| **Failure modes** | レンジ不整合は `416`、トークン欠落/無効は `401`、ストリーム/バイト予算超過は `429`。 |

### `GET /v1/sorafs/storage/chunk/{manifest_id}/{digest}`

同じヘッダーに決定的な chunk digest を加えた単一 chunk 取得。CAR スライスが
不要なときの再試行やフォレンジックダウンロードに有効です。

## マルチソース・オーケストレーターのワークフロー

SF-6 マルチソース fetch が有効な場合（Rust CLI の `sorafs_fetch`、SDK の
`sorafs_orchestrator`）:

1. **入力収集** — manifest の chunk プランをデコードし、最新の広告を取得し、
   必要ならテレメトリスナップショット（`--telemetry-json` または
   `TelemetrySnapshot`）を渡します。
2. **スコアボード作成** — `Orchestrator::build_scoreboard` が適格性を評価し、
   拒否理由を記録。`sorafs_fetch --scoreboard-out` が JSON を永続化します。
3. **chunk スケジュール** — `fetch_with_scoreboard`（または `--plan`）がレンジ制約、
   ストリーム予算、リトライ/ピア上限（`--retry-budget`, `--max-peers`）を
   強制し、各リクエストに manifest スコープの stream token を発行します。
4. **レシート検証** — 出力には `chunk_receipts` と `provider_reports` が含まれ、
   CLI サマリーが `provider_reports`、`chunk_receipts`、`ineligible_providers` を
   証拠バンドル用に保存します。

オペレーター/SDK に返る共通エラー:

| エラー | 説明 |
|-------|------|
| `no providers were supplied` | フィルタ後に適格なエントリがない。 |
| `no compatible providers available for chunk {index}` | 特定の chunk に対するレンジ/予算の不一致。 |
| `retry budget exhausted after {attempts}` | `--retry-budget` を増やすか失敗ピアを除外。 |
| `no healthy providers remaining` | 反復失敗により全プロバイダーが無効化。 |
| `streaming observer failed` | 下流の CAR writer が中断。 |
| `orchestrator invariant violated` | manifest、scoreboard、テレメトリスナップショット、CLI JSON を取得してトリアージ。 |

## テレメトリと証拠

- オーケストレーターが発行するメトリクス:  
  `sorafs_orchestrator_active_fetches`, `sorafs_orchestrator_fetch_duration_ms`,
  `sorafs_orchestrator_retries_total`, `sorafs_orchestrator_provider_failures_total`
  （manifest/region/provider でタグ付け）。`telemetry_region` を config または
  CLI フラグで設定し、ダッシュボードをフリート別に分割します。
- CLI/SDK の fetch サマリーには、永続化された scoreboard JSON、chunk レシート、
  provider レポートが含まれ、SF-6/SF-7 ゲート用の rollout バンドルに同梱する必要があります。
- Gateway ハンドラーは `telemetry::sorafs.fetch.lifecycle|retry|provider_failure|error`
  を公開し、SRE ダッシュボードがオーケストレーターの判断とサーバ挙動を
  相関できるようにします。

## CLI と REST のヘルパー

- `iroha app sorafs pin list|show`、`alias list`、`replication list` は pin-registry の
  REST エンドポイントをラップし、監査証拠のためにアテステーションブロック付きの
  生 Norito JSON を出力します。
- `iroha app sorafs storage pin` と `torii /v1/sorafs/pin/register` は Norito または JSON
  の manifest と、任意の alias proof/ successor を受け付けます。不正な proof は
  `400`、古い proof は `Warning: 110` 付きで `503`、期限切れ proof は `412` を返します。
- REST エンドポイント（`/v1/sorafs/pin`, `/v1/sorafs/aliases`,
  `/v1/sorafs/replication`）にはアテステーション構造が含まれ、クライアントは
  最新ブロックヘッダーに対してデータを検証してから操作できます。

## 参照

- 正規仕様:
  [`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)
- Norito 型: `crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`
- CLI ヘルパー: `crates/iroha_cli/src/commands/sorafs.rs`,
  `crates/sorafs_car/src/bin/sorafs_fetch.rs`
- オーケストレーター crate: `crates/sorafs_orchestrator`
- ダッシュボードパック: `dashboards/grafana/sorafs_fetch_observability.json`
