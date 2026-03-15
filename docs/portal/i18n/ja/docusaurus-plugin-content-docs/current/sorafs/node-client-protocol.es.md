---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/node-client-protocol.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# ノードのプロトコル ↔ クライアント SoraFS

プロトコルの定義に関する履歴書
[`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)。
米国の特別な上流パラロス レイアウト Norito の小さなバイト数
変更ログ;ラ コピア デル ポータル マンティエン ロス プントス オペラティボス セルカ デル レスト
SoraFS の Runbook です。

## 証明と検証の発表

SoraFS のペイロードの損失 `ProviderAdvertV1` (ver
`crates/sorafs_manifest::provider_advert`) オペラ ゴベルナドの会社です。
フィジャンのメタデータと保護者の安全に関する情報の喪失
または、射出の際に多大な影響を及ぼします。

- **ビゲンシア** — `issued_at < expires_at ≤ issued_at + 86,400 s`。ロス・プローベドアス
  デベン・リフレスカル・カダ12ホラ。
- **容量に関する TLV** — 輸送機能に関する TLV 通知のリスト (Torii,
  QUIC+Noise、SoraNet 関連、証明機能の拡張)。ロス コディゴス デスコノシドス
  プエデン オミティルセ クアンド `allow_unknown_capabilities = true`、シギエンド ラ ギア
  グリース。
- **QoS のステータス** — `availability` (ホット/ウォーム/コールド) の最大遅延時間
  回復、同意およびストリームの事前準備の制限は任意です。ラQoS
  遠隔測定による観察を行って、監査と管理を行うことができます。
- **エンドポイントとランデブーのトピック** — サービスの具体的な URL
  TLS/ALPN のクライアントに関するトピックのメタデータ
  警備員を配置することにより、安全を確保します。
- **多様性のある政治** — `min_guard_weight`、ファンアウトの話題
  AS/プール y `provider_failure_threshold` 可能性のあるフェッチが確定しない
  マルチピア。
- **Identificadores de perfil** — ロス・プロフェドーレス・デベン・エクスポナー・エル・ハンドル
  カノニコ (p. ej.、`sorafs.sf1@1.0.0`); `profile_aliases` オプチナレス アユダン a
  移民クライアントアンティグオス。

レチャザン ステーク セロの有効性の確認、機能のリスト、
エンドポイント、トピック、QoS 問題の解決策やオブジェクト。ロス
安全な情報と安全な情報を比較する
(`compare_core_fields`) 実際に問題が発生する前に。

### ランゴスを取得するための拡張機能

ランゴの証明には次のメタデータが含まれます:

|カンポ |プロポシト |
|------|----------|
| `CapabilityType::ChunkRangeFetch` | `max_chunk_span`、`min_granularity` および alineación/prueba フラグを宣言します。 |
| `StreamBudgetV1` |コンカレンシア/スループットのオプションのエンベロープ (`max_in_flight`、`max_bytes_per_sec`、`burst` オプション)。ランゴの能力を要求してください。 |
| `TransportHintV1` |交通機関の優先事項 (p. ej.、`torii_http_range`、`quic_stream`、`soranet_relay`)。 `0–15` は複製を優先します。 |

ツールのサポート:

- プロバイダー広告のパイプラインの損失、ランゴの容量、ストリームの有効性
  予算と輸送のヒントは、聴覚的にペイロードを決定する前に送信されます。
- `cargo xtask sorafs-admission-fixtures` エンパケタ アヌンシオス マルチフエンテ
  ダウングレード用のフィクスチャを簡単に実行できます
  `fixtures/sorafs_manifest/provider_admission/`。
- `stream_budget` または `transport_hints` 息子を報告します。
  ローダーの再起動 CLI/SDK のプログラマー、管理ツール
  Torii での期待に応えられるよう、多面的な取り組みを行っています。

## ゲートウェイのエンドポイント

ゲートウェイの損失は、メタデータの HTTP 決定情報を受け入れます
ロスアナウンス。

### `GET /v2/sorafs/storage/car/{manifest_id}`

|レクシト |詳細 |
|----------|----------|
| **ヘッダー** | `Range` (チャンクのオフセットを調整する)、`dag-scope: block`、`X-SoraFS-Chunker`、`X-SoraFS-Nonce` オプション、`X-SoraFS-Stream-Token` Base64 義務。 |
| **レスペスタ** | `206` は、`Content-Type: application/vnd.ipld.car`、`Content-Range` のベンタナ サービス、メタデータ `X-Sora-Chunk-Range` y ヘッダーのチャンカー/トークン エコードを説明します。 |
| **ファロモード** | `416` は生理食塩水のランゴス、`401` は無効な無効なトークン、`429` はストリーム/バイトの事前準備を超えています。 |

### `GET /v2/sorafs/storage/chunk/{manifest_id}/{digest}`

ミスモスのヘッダーを含むソロチャンクを取得して、ダイジェストを決定します
チャンク。必要なスライスをすべて削除するまで
車。

## フルホ デ トラバホ デル オルケスタドール マルチフエンテ

SF-6 を使用して複数のファイルを取得できます (`sorafs_fetch` 経由の CLI Rust、
`sorafs_orchestrator` 経由の SDK):1. **エントラダの再コピー** — マニフェスト、チャンクのプランの解読、トレーア
   遠隔測定のスナップショットの記録、オプション、パサールの記録
   (`--telemetry-json` または `TelemetrySnapshot`)。
2. **スコアボードの作成** — `Orchestrator::build_scoreboard` 評価
   elegibilidad y registra razones de rechazo; `sorafs_fetch --scoreboard-out`
   JSON を永続化します。
3. **プログラマ チャンク** — `fetch_with_scoreboard` (o `--plan`) impone
   ランゴの制限、ストリームのプレステス、ラインテント/ピアの制限
   (`--retry-budget`, `--max-peers`) y はストリームトークン管理を発行します
   マニフェスト・ポル・カダ・ソリトゥド。
4. **Verificar recibos** — las salidas incluyen `chunk_receipts` y
   `provider_reports`; CLI 持続 `provider_reports` の再開、
   `chunk_receipts` および `ineligible_providers` は証拠のバンドルです。

オペラドール/SDK に関するエラー:

|エラー |説明 |
|------|-----------|
| `no providers were supplied` |干し草は含まれていません。 |
| `no compatible providers available for chunk {index}` |チャンク特有のランゴを事前に決定します。 |
| `retry budget exhausted after {attempts}` |インクリメント `--retry-budget` o ピアのフォールドを追放します。 |
| `no healthy providers remaining` | Todos los proveedores quedaron deshabilitados tras fallos repetidos。 |
| `streaming observer failed` |エスクリトール CAR ダウンストリームが中止されます。 |
| `orchestrator invariant violated` |キャプチャ マニフェスト、スコアボード、テレメトリのスナップショット、JSON デル CLI パラトリアージ。 |

## 遠隔測定と証拠

- オルケスタドールの排出量:  
  `sorafs_orchestrator_active_fetches`、`sorafs_orchestrator_fetch_duration_ms`、
  `sorafs_orchestrator_retries_total`、`sorafs_orchestrator_provider_failures_total`
  (マニフェスト/地域/証明者のエチケット)。 `telemetry_region` ja の構成
  フロタごとにダッシュボードごとに CLI パラメタのフラグを介して設定します。
- スコアボード JSON 永続化、レシボスを含む CLI/SDK でのフェッチの再開
  チャンクとロールアウトのバンドルを介して証明された情報を提供します
  パラ ラス プエルタス SF-6/SF-7。
- ゲートウェイ指数 `telemetry::sorafs.fetch.lifecycle|retry|provider_failure|error` のハンドラーが失われています
  ダッシュボードに関連した SRE の相関性に関する意思決定を決定します。
  comportamiento del servidor。

## CLI と REST のアユダ

- `iroha app sorafs pin list|show`、`alias list` y `replication list` 環境ロス
  エンドポイント REST デルのピン レジストリとインプリメント Norito JSON のブロック ブロック
  監査パラ証拠。
- `iroha app sorafs storage pin` y `torii /v2/sorafs/pin/register` アセプタンマニフェスト
  Norito o JSON のエイリアスオプションと後継の証明。マルフォルマドの証拠
  エレバン `400`、証明は古い指数 `503` コン `Warning: 110`、y 証明
  エクスピラドス デブエルベン `412`。
- エンドポイント REST を失った (`/v2/sorafs/pin`、`/v2/sorafs/aliases`、
  `/v2/sorafs/replication`) 証明書の構造を含む
  クライアントは、実際のヘッダーの内容を確認してから、実際のブロックを確認します。

## 参考資料

- 規格の詳細:
  [`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)
- ティポス Norito: `crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`
- CLI のアユダ: `crates/iroha_cli/src/commands/sorafs.rs`、
  `crates/sorafs_car/src/bin/sorafs_fetch.rs`
- オルケスタドールの箱: `crates/sorafs_orchestrator`
- ダッシュボードのパッケージ: `dashboards/grafana/sorafs_fetch_observability.json`