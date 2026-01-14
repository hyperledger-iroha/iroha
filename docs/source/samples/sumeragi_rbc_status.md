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
- When `sumeragi.da_enabled = true`, availability evidence (an `availability evidence` or an RBC `READY` quorum) is tracked, but commit does not wait on it or on local RBC `DELIVER`. RBC remains transport/recovery and its delivery latency is still useful for debugging.
