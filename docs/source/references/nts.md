## Network Time Service (NTS)

The Network Time Service provides a robust, byzantine‑resilient estimate of “network time” for node timers and diagnostics. It does not change consensus rules or OS clocks; block header timestamps remain authoritative for acceptance.

### Overview

- Sampling: Periodically probes online peers with NTP‑style ping/pong and computes per‑sample offset/RTT.
- Aggregation: Filters high‑RTT outliers and computes a trimmed median offset with MAD confidence.
- Smoothing (optional): Exponential moving average (EMA) with a slew cap to avoid abrupt jumps.
- Determinism: NTS is advisory; consensus validity and block acceptance do not consult NTS.

### Configuration

TOML section `[nts]` (see `docs/source/references/peer.template.toml` for a template):

- `sample_interval_ms` (u64): Probe period. Default ≈ 5000.
- `sample_cap_per_round` (usize): Max peers per round. Default 8.
- `max_rtt_ms` (u64): Discard samples above this RTT. Default 500.
- `trim_percent` (u8): Symmetric trim percentage (each side) for median. Default 10.
- `per_peer_buffer` (usize): Per‑peer ring buffer depth. Default 16.
- `smoothing_enabled` (bool): Enable EMA smoothing. Default false.
- `smoothing_alpha` (f64): EMA alpha in [0,1]; higher = more responsive. Default 0.2.
- `max_adjust_ms_per_min` (u64): Max allowed offset change per minute, in ms. Default 50.

Example:

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

Operator guidance:

- Configure via `iroha_config`; environment variable overrides exist for developer and CI harnesses but should not drive production behavior. Defaults are conservative and suitable for most deployments.

### Torii Endpoints

- `GET /v1/time/now` → `{ "now": <ms_epoch>, "offset_ms": <i64>, "confidence_ms": <u64> }`
- `GET /v1/time/status` → diagnostics and RTT histogram buckets:
  - `{ "peers": <u64>, "samples": [{"peer","last_offset_ms","last_rtt_ms","count"}, ...], "rtt": {"buckets": [{"le","count"},...], "sum_ms", "count"}, "note": "NTS running" }`

### Telemetry Metrics

Exported under Prometheus metrics:

- `nts_offset_ms` (gauge, signed) — smoothed or raw offset vs local clock.
- `nts_confidence_ms` (gauge) — MAD confidence bound.
- `nts_peers_sampled` (gauge) — peers contributing recent samples.
- `nts_rtt_ms_bucket{le="…"}` (gauge) — RTT histogram buckets (ms).
- `nts_rtt_ms_sum` / `nts_rtt_ms_count` — histogram sum/count.

### Behavior & Guarantees

- Determinism: No consensus decisions depend on wall‑clock or NTS; final state remains identical across hardware.
- Safety: NTS never adjusts the OS/system clock; it maintains an offset against the local clock.
- Performance: Sampling and aggregation are lightweight; per‑peer ring buffers cap memory use.

### Notes

- Observers and light clients may rely on `time/now` as advisory time; validators use NTS only for timers/timeouts.
- Tune `smoothing_alpha` and `max_adjust_ms_per_min` for your network’s latency characteristics if needed; defaults are conservative.
