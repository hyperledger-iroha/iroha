<!-- Hebrew translation of docs/source/samples/sumeragi_rbc_status.md -->

---
lang: he
direction: rtl
source: docs/source/samples/sumeragi_rbc_status.md
status: needs-update
translator: manual
---

<div dir="rtl">

> NOTE: This translation has not yet been updated for the v1 DA availability (advisory). Refer to `docs/source/samples/sumeragi_rbc_status.md` for current semantics.

# ‏Sumeragi — מצב RBC (Torii)

נקודת קצה
- `GET /v1/sumeragi/rbc`

תגובה (דוגמה)
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

דוגמת פירוט סשן (`GET /v1/sumeragi/rbc/sessions`, מקוצר):

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

הערות
- נקודת הקצה זמינה רק כאשר הטלמטריה מופעלת בזמן קומפילציה.
- מספקת מוני צבירה לפעילות RBC (שידור אמין).
- כאשר `sumeragi.da.enabled = true`, תהליך ה־commit ממתין ל־`availability evidence` (ולא לאירוע `DELIVER` מקומי של RBC); ה־RBC משמש כמנגנון הפצה/שחזור של ה־payload.

</div>
