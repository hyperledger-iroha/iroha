<!-- Japanese translation of docs/source/sumeragi_pacemaker.md -->

---
lang: ja
direction: ltr

Note: Defaults now seed a 1s slot with phase timeouts propose/prevote/precommit/commit/DA = 350/450/550/750/650 ms. See the English `sumeragi_pacemaker.md` for the latest pacing details.
source: docs/source/sumeragi_pacemaker.md
status: complete
translator: manual
---

# Sumeragi ペースメーカー — タイマー、バックオフ、ジッター

本ノートは Sumeragi のペースメーカー（タイマー）方針を概説し、オペレーター向けのガイダンスと具体例を提供します。ペースメーカーはリーダーが提案を行うタイミングと、非アクティブ時にバリデータが新しいビューを提案／移行するタイミングを制御します。

実装状況: EMA による基準ウィンドウ、バックオフ、RTT フロア、および設定可能なジッタ帯（`sumeragi.advanced.pacemaker.jitter_frac_permille`）が導入済み。ジッタはノードと `(height, view)` ごとに決定的に適用されます。

## 概念
- **基準ウィンドウ**: 観測したコンセンサスフェーズ（`propose`, `collect_da`, `collect_prevote`, `collect_precommit`, `commit`）の指数移動平均。EMA は `sumeragi.advanced.npos.timeouts.*_ms` からシードされ、十分なサンプルが集まるまでは設定値とほぼ一致します。`collect_aggregator` の EMA は可観測性のため公開されていますが、ペースメーカーウィンドウには含まれません。平滑値は `sumeragi_phase_latency_ema_ms{phase=…}` で確認できます。
- **バックオフ乗数**: `sumeragi.advanced.pacemaker.backoff_multiplier`（既定 1）。タイムアウトが発生するたびに `window += base * multiplier`。
- **RTT フロア**: `avg_rtt_ms * sumeragi.advanced.pacemaker.rtt_floor_multiplier`（既定 2）。高レイテンシリンクでタイムアウトが攻撃的になりすぎるのを防ぎます。
- **上限**: `sumeragi.advanced.pacemaker.max_backoff_ms`（既定 60,000 ms）。ウィンドウのハード上限。

タイムアウト時の有効ウィンドウ更新:
```
window = min(cap, max(window + base * backoff_mul, avg_rtt * rtt_floor_mul))
```
RTT サンプルが無い場合、RTT フロアは 0 とみなします。

公開テレメトリ（詳細は telemetry.md）:
- ランタイム: `sumeragi_pacemaker_backoff_ms`, `sumeragi_pacemaker_rtt_floor_ms`, `sumeragi_phase_latency_ema_ms{phase=…}`
- REST スナップショット: `/v1/sumeragi/phases` は各フェーズの最新レイテンシとともに `ema_ms` を含み、Prometheus を直接スクレイプしなくてもトレンドを可視化可能。
- 設定: `sumeragi.advanced.pacemaker.backoff_multiplier`, `sumeragi.advanced.pacemaker.rtt_floor_multiplier`, `sumeragi.advanced.pacemaker.max_backoff_ms`

## ジッタ方針
ハードウェアの群集行動（同時タイムアウト）を避けるため、ペースメーカーはウィンドウ周辺に小さなノード固有ジッタを導入できます。

ノードごとの決定的ジッタ:
- ソース: `blake2(chain_id || peer_id || height || view)` → 64 ビット値を [−J, +J] にスケーリング。
- 推奨ジッタ帯: 計算された `window` の ±10%。
- 同じ `(height, view)` の間は変動させず、一度のみ適用。

擬似コード:
```text
base = ema_total_ms(view, height)
window = min(cap, max(prev + base * backoff_mul, avg_rtt * rtt_floor_mul))
seed = blake2(chain_id || peer_id || height || view)
u = (seed % 10_000) / 10_000.0
jfrac = 0.10
jitter = (u * 2 - 1) * jfrac * window
window_jittered_ms = clamp(window + jitter, 0, cap)
```

備考:
- ジッタはノードと `(height, view)` ごとに決定的で外部乱数を必要としません。
- 応答性を損なわないため、帯域は 10% 以下に抑えてください。
- ジッタはライブネスに影響しますが安全性には影響しません。

テレメトリ:
- `sumeragi_pacemaker_jitter_ms` — 適用されたジッタの絶対値（ms）
- `sumeragi_pacemaker_jitter_frac_permille` — 設定中のジッタ帯（パーミル単位）

## オペレーター向けガイダンス

低レイテンシ LAN / PoA
- `backoff_multiplier`: 1–2
- `rtt_floor_multiplier`: 2
- `max_backoff_ms`: 5,000–10,000
- 理由: 提案頻度を高く保ち、スタール時の回復を短く。

地理的に分散した WAN
- `backoff_multiplier`: 2–3
- `rtt_floor_multiplier`: 3–5
- `max_backoff_ms`: 30,000–60,000
- 理由: 高 RTT リンクでの急激な再試行を避け、バーストに耐える。

高変動／モバイルネットワーク
- `backoff_multiplier`: 3–4
- `rtt_floor_multiplier`: 4–6
- `max_backoff_ms`: 60,000
- 理由: 同期ビュー変更を最小化。コミット遅延を許容できる場合はジッタの導入も検討。

## 例

1) LAN（平均 RTT ≈ 2 ms、基準 2,000 ms、上限 10,000 ms）
   - `backoff_mul = 1`, `rtt_floor_mul = 2`
   - 1 回目タイムアウト: `max(0 + 2000, 2*2) = 2000 ms`
   - 2 回目タイムアウト: `min(10 000, 2000 + 2000) = 4000 ms`

2) WAN（平均 RTT ≈ 80 ms、基準 4,000 ms、上限 60,000 ms）
   - `backoff_mul = 2`, `rtt_floor_mul = 3`
   - 1 回目タイムアウト: `max(0 + 8000, 80*3) = 8000 ms`
   - 2 回目タイムアウト: `min(60 000, 8000 + 8000) = 16 000 ms`

3) ジッタ帯 10%（例示）
   - `window = 16 000 ms` のときジッタ範囲は [−1 600, +1 600] ms → ノードごとに `window' ∈ [14 400, 17 600] ms`

## モニタリング
- バックオフトレンド: `avg_over_time(sumeragi_pacemaker_backoff_ms[5m])`
- RTT フロア確認: `avg_over_time(sumeragi_pacemaker_rtt_floor_ms[5m])`
- EMA と分布比較: `sumeragi_phase_latency_ema_ms{phase=…}` と `sumeragi_phase_latency_ms{phase=…}` のパーセンタイルを並べて観測
- 設定の検証: `max(sumeragi.advanced.pacemaker.backoff_multiplier)`, `max(sumeragi.advanced.pacemaker.rtt_floor_multiplier)`, `max(sumeragi.advanced.pacemaker.max_backoff_ms)`

## 決定性と安全性
- タイマー／バックオフ／ジッタは提案やビュー変更のトリガ時刻にのみ影響し、署名検証や commit certificate ルールには影響しません。
- 乱数を用いる際はノードおよび `(height, view)` ごとに決定的な方式を保ち、日付時刻や OS RNG をコンセンサスクリティカル経路で利用しないでください。
