---
lang: zh-hans
direction: ltr
source: docs/source/samples/sumeragi_rbc_status.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ad00669e1f686b17ace3f055cee3618e2d5e3d111a5b1120c31b96cd63cc96ac
source_last_modified: "2026-01-23T23:46:10.139424+00:00"
translation_last_reviewed: 2026-02-07
---

# Sumeragi — RBC Status (Torii)

Endpoint
- `GET /v1/sumeragi/rbc`

Response (example)
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

Sample session detail (`GET /v1/sumeragi/rbc/sessions`, truncated):

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

Notes
- Available only when telemetry is enabled.
- Provides aggregate counters for RBC (Reliable Broadcast) activity.
- When `sumeragi.da.enabled = true`, availability evidence (an `availability evidence` or an RBC `READY` quorum) is tracked (advisory; does not gate commit); missing local payloads are fetched via RBC `DELIVER` or block sync. RBC remains transport/recovery and its delivery latency is still useful for debugging.
