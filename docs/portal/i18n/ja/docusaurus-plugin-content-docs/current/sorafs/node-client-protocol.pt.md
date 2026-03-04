---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/node-client-protocol.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SoraFS の <-> クライアントのプロトコル

エステギアはプロトコルの定義を再開します
[`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)。
特定のアップストリーム パラ レイアウト Norito バイト変更ログを使用します。
ポータル マンテム os デスタケス オペレーション レポートのコピー、Dos Runbook
SoraFS。

## 証明された広告と検証

Provedores SoraFS 配布ペイロード `ProviderAdvertV1` (veja
`crates/sorafs_manifest::provider_advert`) アッシナドス ペロ オペラドール ガバナド。オス
広告 fixam os メタダドス デ デスコベルタ e os ガードレール ケ オ オルケストラドール
マルチソースの aplica em ランタイム。

- **ビゲンシア** - `issued_at < expires_at <= issued_at + 86,400 s`。プロヴェドール
  devem renovar a cada 12 horas。
- **TLV 容量** - 輸送時の TLV 通知リスト (Torii,
  QUIC+Noise、リレー SoraNet、拡張機能)。コディゴス・デスコンヘシドス
  podem ser ignorados quando `allow_unknown_capabilities = true`、セギンド
  オリエンタカオグリース。
- **QoS のヒント** - `availability` 層 (ホット/ウォーム/コールド)、最大遅延
  回復、コンコルレンシアの制限、およびストリームの予算の制限。 QoSの開発
  アリンハルはテレメトリアを観察し、監査を承認します。
- **エンドポイントとランデブー トピック** - メタデータに関するサービスの URL
  TLS/ALPN は、OS トピックを説明し、OS クライアントを開発し、セキュリティを強化します。
  青コンストライアガードセット。
- **カミーニョの多様な政治** - `min_guard_weight`、ファンアウトのキャップ
  AS/プール `provider_failure_threshold` トルナムは確定的なフェッチを取得します
  マルチピア。
- **Identificadores de perfil** - provedores devem expor o handle canonico (ex.
  `sorafs.sf1@1.0.0`); `profile_aliases` 移民のクライアントであるアンティゴスを選択します。

ステークゼロを確認し、機能/エンドポイント/トピックのリストを確認し、
必要に応じて QoS をターゲットにします。入学封筒の比較
os corpos do advert e da proposta (`compare_core_fields`) セミナー前
アトゥアリザコ。

### 範囲フェッチの拡張

Provedores com の範囲には、メタデータが含まれています:

|カンポ |プロポジト |
|------|-----------|
| `CapabilityType::ChunkRangeFetch` | `max_chunk_span`、`min_granularity` のフラグをアリンハメント/プロバに宣言します。 |
| `StreamBudgetV1` |コンコルレンシア/スループットのオプションのエンベロープ (`max_in_flight`、`max_bytes_per_sec`、`burst` オプション)。範囲内の容量を要求してください。 |
| `TransportHintV1` |交通機関の優先順位 (例: `torii_http_range`、`quic_stream`、`soranet_relay`)。 SAO `0-15` と重複する優先順位。 |

ツールのサポート:

- プロバイダーの広告開発のパイプライン、範囲、ストリーム予算などの容量を検証
  トランスポートヒントは、ペイロードを送信する前に、聴覚的に決定します。
- `cargo xtask sorafs-admission-fixtures` agrupa がマルチソースの canonicos を広告します
  junto com ダウングレード フィクスチャ em `fixtures/sorafs_manifest/provider_admission/`。
- 広告の範囲を省略 `stream_budget` ou `transport_hints` sao rejeitados
  ペロスローダー CLI/SDK は、アジェンダメントを実行し、マルチソースを活用するために管理します
  alinhado com は Torii を期待しています。

## ゲートウェイの範囲内のエンドポイント

ゲートウェイは、メタデータの HTTP 決定を要求します
広告。

### `GET /v1/sorafs/storage/car/{manifest_id}`

|レクシト |デタルヘス |
|----------|----------|
| **ヘッダー** | `Range` (チャンクのジャネラ ユニカ アリンハダ AOS オフセット)、`dag-scope: block`、`X-SoraFS-Chunker`、`X-SoraFS-Nonce` オプション、`X-SoraFS-Stream-Token` Base64 義務。 |
| **レスポスタ** | `206` com `Content-Type: application/vnd.ipld.car`、`Content-Range` は、ジャネラ サービス、メタデータ `X-Sora-Chunk-Range` チャンカー/トークン エコードのヘッダーを記述します。 |
| **ファルハス** | `416` はサリンハドスの範囲、`401` はトークンの有効/無効、`429` はストリーム/バイトの超過予算です。 |

### `GET /v1/sorafs/storage/chunk/{manifest_id}/{digest}`

チャンクを取得して、ユニコ コムのメスモス ヘッダーを取得し、チャンクをダイジェストで決定します。
再試行して、必要な CAR のスライスをダウンロードするまで使用します。

## orquestrador マルチソースを実行するワークフロー

マルチソース SF-6 を取得できるようになりました (`sorafs_fetch` 経由の CLI Rust、
`sorafs_orchestrator` 経由の SDK):1. **Coletar entradas** - チャンクの解読、プクサー OS
   最近の広告、オプション、テレメトリ スナップショットの通過
   (`--telemetry-json` または `TelemetrySnapshot`)。
2. **スコアボードの作成** - `Orchestrator::build_scoreboard` アヴァリア
   エレギビリダーデとレジェイソン・ラゾエス登録。 `sorafs_fetch --scoreboard-out`
   JSON を永続化します。
3. **アジェンダ チャンク** - `fetch_with_scoreboard` (ou `--plan`) 復元リスト
   範囲、ストリームの予算、再試行/ピアの上限 (`--retry-budget`、`--max-peers`)
   必要なマニフェストを使用してストリーム トークンを発行します。
4. **Verificar recibos** - `chunk_receipts` および `provider_reports` を含むと述べています。
   CLI 永続化 `provider_reports`、`chunk_receipts` e を実行します。
   `ineligible_providers` パラ証拠バンドル。

オペラドール/SDK との共通のエラー:

|エラー |説明 |
|------|-----------|
| `no providers were supplied` |ネンフマ・エントラダ・エレギベル・アポス・オ・フィルトロ。 |
| `no compatible providers available for chunk {index}` |予算の範囲とチャンクの特定が不一致です。 |
| `retry budget exhausted after {attempts}` | `--retry-budget` を削除してピア com falha を削除してください。 |
| `no healthy providers remaining` | Todos os provedores foram desabilitados apos falhas repetitidas。 |
| `streaming observer failed` |おおライターCAR下流中止。 |
| `orchestrator invariant violated` |マニフェスト、スコアボード、テレメトリ スナップショット、CLI JSON のトリアージをキャプチャします。 |

## テレメトリアと証拠

- メトリカス・エミティダス・ペロ・オルケストラドール:  
  `sorafs_orchestrator_active_fetches`、`sorafs_orchestrator_fetch_duration_ms`、
  `sorafs_orchestrator_retries_total`、`sorafs_orchestrator_provider_failures_total`
  (マニフェスト/リージョン/プロバイダーのタグ)。 Defina `telemetry_region` 構成
  CLI のフラグを使用して、特定のダッシュボードを表示します。
- スコアボード JSON 永続化、チャンク受信を含む CLI/SDK を取得しないマリオ
  プロバイダーは、SF-6/SF-7 のゲートに関する開発バンドルの IR NOS ロールアウト バンドルを報告します。
- ゲートウェイ ハンドラーは `telemetry::sorafs.fetch.lifecycle|retry|provider_failure|error` を表示します
  ダッシュボードの SRE 相関性を決定し、Orquestrador com o comportamento を実行します
  奉仕をします。

## CLI および REST のヘルパー

- `iroha app sorafs pin list|show`、`alias list`、`replication list` は OS に関与します
  エンドポイント REST do pin-registry e imprimem Norito JSON ブルート コム ブロック
  聴覚的証拠の証明。
- `iroha app sorafs storage pin` e `torii /v1/sorafs/pin/register` アセチタムマニフェスト
  Norito ou JSON com エイリアス証明と後継オプション。マルフォルマドの証拠
  geram `400`、証明が古い retornam `503` com `Warning: 110`、e 証明 expirados
  レトルナム`412`。
- エンドポイント REST (`/v1/sorafs/pin`、`/v1/sorafs/aliases`、`/v1/sorafs/replication`)
  顧客の検証に必要な証明書を含める
  究極のヘッダーデブロコアンテスデアジール。

## 参考資料

- 仕様カノニカ:
  [`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)
- ティポス Norito: `crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`
- CLI ヘルパー: `crates/iroha_cli/src/commands/sorafs.rs`、
  `crates/sorafs_car/src/bin/sorafs_fetch.rs`
- オルケストラドールの箱: `crates/sorafs_orchestrator`
- ダッシュボードのパック: `dashboards/grafana/sorafs_fetch_observability.json`