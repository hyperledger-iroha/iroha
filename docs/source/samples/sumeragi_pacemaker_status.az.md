---
lang: az
direction: ltr
source: docs/source/samples/sumeragi_pacemaker_status.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c91eb98f17f4c9f6120828b74e0ec7276abbf129472e32721f3550ed16aff637
source_last_modified: "2025-12-29T18:16:36.034419+00:00"
translation_last_reviewed: 2026-02-07
---

# Sumeragi — Pacemaker Status (Torii)

Endpoint
- `GET /v2/sumeragi/pacemaker`

Response (example)
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

Notes
- Available only when built with telemetry enabled.
- Exposes current pacemaker backoff window and timing parameters.
- Intended for operator visibility; values are implementation-specific gauges.

