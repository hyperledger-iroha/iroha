<!-- Japanese translation of docs/source/samples/sumeragi_rbc_status.md -->

---
lang: ja
direction: ltr
source: docs/source/samples/sumeragi_rbc_status.md
status: needs-update
translator: manual
---

> NOTE: This translation has not yet been updated for the v1 DA availability (advisory). Refer to `docs/source/samples/sumeragi_rbc_status.md` for current semantics.

# Sumeragi — RBC 状態（Torii）

エンドポイント
- `GET /v1/sumeragi/rbc`

レスポンス（例）
```json
{
  "sessions_active": 0,
  "sessions_pruned_total": 0,
  "ready_broadcasts_total": 0,
  "ready_rebroadcasts_skipped_total": 0,
  "deliver_broadcasts_total": 0,
  "payload_bytes_delivered_total": 0,
  "payload_rebroadcasts_skipped_total": 0
}
```

セッション詳細のサンプル（`GET /v1/sumeragi/rbc/sessions`、抜粋）:

```json
{
  "sessions_active": 1,
  "items": [
    {
      "block_hash": "deadbeefcafebabe1234567890abcdef",
      "height": 4201,
      "view": 7,
      "total_chunks": 168,
      "received_chunks": 168,
      "ready_count": 4,
      "delivered": true,
      "invalid": false,
      "payload_hash": "9ef2e4f3c9c0b1c2d3e4f5a6978899aa",
      "recovered": false,
      "lane_backlog": [
        {
          "lane_id": 0,
          "tx_count": 6,
          "total_chunks": 168,
          "pending_chunks": 0,
          "rbc_bytes_total": 11010048
        }
      ],
      "dataspace_backlog": [
        {
          "lane_id": 0,
          "dataspace_id": 0,
          "tx_count": 6,
          "total_chunks": 168,
          "pending_chunks": 0,
          "rbc_bytes_total": 11010048
        }
      ]
    }
  ]
}
```

備考
- テレメトリを有効にしたビルドでのみ利用可能です。
- RBC（Reliable Broadcast）活動の集計カウンターを提供します。
- `sumeragi.da_enabled = true` の場合、コミットは `availability evidence` の観測を待ってからブロックを確定します（ローカルの RBC `DELIVER` は待ちません）。RBC はペイロード配布と欠落回復のための輸送・補助として使われます。
