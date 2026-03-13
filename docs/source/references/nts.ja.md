<!-- Japanese translation of docs/source/references/nts.md -->

---
lang: ja
direction: ltr
source: docs/source/references/nts.md
status: complete
translator: manual
---

## ネットワークタイムサービス（NTS）

Network Time Service は、ノードのタイマーや診断のためにビザンチン耐性を備えた「ネットワーク時間」の推定値を提供します。コンセンサス規則や OS クロックは変更せず、ブロックヘッダーのタイムスタンプが受理判断の基準であり続けます。

### 概要

- サンプリング: 稼働中のピアへ NTP スタイルの ping/pong を定期的に行い、オフセットと RTT を取得。
- 集約: RTT の大きい外れ値を除外し、トリム付き中央値と MAD 信頼区間を計算。
- 平滑化（任意）: 突発的な変動を抑えるための指数移動平均（EMA）とスルー制限。
- 決定性: NTS は助言的な情報であり、コンセンサス検証やブロック受理は NTS を参照しません。

### 設定

TOML の `[nts]` セクション（雛形は `docs/source/references/peer.template.toml` を参照）:

- `sample_interval_ms` (u64): ピアをポーリングする周期。既定 ≈ 5000。
- `sample_cap_per_round` (usize): 1 ラウンドあたりのピア数上限。既定 8。
- `max_rtt_ms` (u64): この RTT を超えるサンプルは除外。既定 500。
- `trim_percent` (u8): 中央値を計算する際の左右対称トリム率。既定 10。
- `per_peer_buffer` (usize): ピアごとのリングバッファ深度。既定 16。
- `smoothing_enabled` (bool): EMA 平滑化の有効化。既定 false。
- `smoothing_alpha` (f64): EMA の α（0〜1）。値が大きいほど応答が速い。既定 0.2。
- `max_adjust_ms_per_min` (u64): 1 分あたりに許可するオフセット変化量（ms）。既定 50。

例:

```toml
[nts]
sample_interval_ms = 5_000
sample_cap_per_round = 8
max_rtt_ms = 500
trim_percent = 10
per_peer_buffer = 16
smoothing_enabled = true
smoothing_alpha = 0.2
max_adjust_ms_per_min = 50
```

オペレーター向けガイダンス:

- 設定は `iroha_config` 経由で行います。開発・CI での環境変数上書きは存在しますが、本番運用の制御には利用しないでください。既定値は保守的で、多くのデプロイでそのまま使用できます。

### Torii エンドポイント

- `GET /v2/time/now` → `{ "now": <ms_epoch>, "offset_ms": <i64>, "confidence_ms": <u64> }`
- `GET /v2/time/status` → 診断情報と RTT ヒストグラム:
  - `{ "peers": <u64>, "samples": [{"peer","last_offset_ms","last_rtt_ms","count"}, ...], "rtt": {"buckets": [{"le","count"},...], "sum_ms", "count"}, "note": "NTS running" }`

### テレメトリメトリクス

Prometheus で公開される指標:

- `nts_offset_ms`（ゲージ・符号付き）— ローカルクロックとのオフセット（平滑化後または生値）。
- `nts_confidence_ms`（ゲージ）— MAD に基づく信頼幅。
- `nts_peers_sampled`（ゲージ）— 最近のサンプルに寄与したピア数。
- `nts_rtt_ms_bucket{le="…"}`
  （ゲージ）— RTT ヒストグラムの各バケット（ミリ秒）。
- `nts_rtt_ms_sum` / `nts_rtt_ms_count` — ヒストグラムの合計と件数。

### 挙動と保証

- 決定性: ウォールクロックや NTS に基づくコンセンサス決定は存在せず、ハードウェアの違いにかかわらず最終状態は同一です。
- 安全性: NTS が OS／システムクロックを直接調整することはありません。ローカルクロックに対するオフセットを保持するのみです。
- 性能: サンプリングと集計は軽量で、ピアごとのリングバッファによりメモリ使用量は制限されます。

### 備考

- オブザーバーやライトクライアントは `time/now` を助言的な時間として参照できます。バリデータは NTS をタイマー／タイムアウトにのみ使用します。
- ネットワーク遅延特性に合わせて `smoothing_alpha` や `max_adjust_ms_per_min` を調整できますが、既定値は保守的に設計されています。
