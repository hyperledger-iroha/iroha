# Sumeragi — Pacemaker Status (Torii)

Endpoint
- `GET /v1/sumeragi/pacemaker`

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

