<!-- Japanese translation of docs/source/samples/sumeragi_pacemaker_status.md -->

---
lang: ja
direction: ltr
source: docs/source/samples/sumeragi_pacemaker_status.md
status: complete
translator: manual
---

# Sumeragi — ペースメーカー状態（Torii）

エンドポイント
- `GET /v1/sumeragi/pacemaker`

レスポンス（例）
```json
{
  "backoff_ms": 0,
  "rtt_floor_ms": 0,
  "backoff_multiplier": 0,
  "rtt_floor_multiplier": 0,
  "max_backoff_ms": 0,
  "jitter_ms": 0,
  "jitter_frac_permille": 0
}
```

備考
- テレメトリ機能を有効にしてビルドした場合のみ利用できます。
- 現在のペースメーカーのバックオフウィンドウとタイミングパラメータを公開します。
- オペレーターの可視化を目的とした値であり、数値は実装に依存するゲージです。
