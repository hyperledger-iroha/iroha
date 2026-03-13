---
lang: zh-hant
direction: ltr
source: docs/source/references/nts.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ba35f5721d4835a313d788e55e34b05cb700ba7b6ac042df46e915e8782542c1
source_last_modified: "2025-12-31T15:58:47.542087+00:00"
translation_last_reviewed: 2026-02-07
---

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
- `min_samples` (usize): Minimum peer samples required for healthy status. Default 3.
- `max_offset_ms` (u64): Max absolute offset (ms) before unhealthy; 0 disables. Default 1000.
- `max_confidence_ms` (u64): Max MAD confidence (ms) before unhealthy; 0 disables. Default 500.
- `enforcement_mode` ("warn" | "reject"): Admission behavior when unhealthy. Default `warn`.

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
min_samples = 3
max_offset_ms = 1_000
max_confidence_ms = 500
enforcement_mode = "warn"
```

Operator guidance:

- Configure via `iroha_config`; environment variable overrides exist for developer and CI harnesses but should not drive production behavior. Defaults are conservative and suitable for most deployments.

### Torii Endpoints

- `GET /v2/time/now` → `{ "now": <ms_epoch>, "offset_ms": <i64>, "confidence_ms": <u64>, "sample_count": <u64>, "peer_count": <u64>, "fallback": <bool>, "health": { "healthy": <bool>, "min_samples_ok": <bool>, "offset_ok": <bool>, "confidence_ok": <bool> } }`
- `GET /v2/time/status` → diagnostics and RTT histogram buckets:
  - `{ "peers": <u64>, "samples_used": <u64>, "offset_ms": <i64>, "confidence_ms": <u64>, "fallback": <bool>, "health": {...}, "samples": [{"peer","last_offset_ms","last_rtt_ms","count"}, ...], "rtt": {"buckets": [{"le","count"},...], "sum_ms", "count"}, "note": "NTS running" }`

### Telemetry Metrics

Exported under Prometheus metrics:

- `nts_offset_ms` (gauge, signed) — smoothed or raw offset vs local clock.
- `nts_confidence_ms` (gauge) — MAD confidence bound.
- `nts_peers_sampled` (gauge) — peers contributing recent samples.
- `nts_samples_used` (gauge) — samples used after RTT filtering.
- `nts_fallback` (gauge) — 1 when falling back to local time.
- `nts_healthy` (gauge) — 1 when health thresholds pass and no fallback.
- `nts_min_samples_ok` / `nts_offset_ok` / `nts_confidence_ok` (gauges) — per-check health flags.
- `nts_rtt_ms_bucket{le="…"}` (gauge) — RTT histogram buckets (ms).
- `nts_rtt_ms_sum` / `nts_rtt_ms_count` — histogram sum/count.

### Behavior & Guarantees

- Determinism: No consensus decisions depend on wall‑clock or NTS; final state remains identical across hardware.
- Safety: NTS never adjusts the OS/system clock; it maintains an offset against the local clock.
- Performance: Sampling and aggregation are lightweight; per‑peer ring buffers cap memory use.
- Admission: Time-sensitive instructions are gated by NTS health when `enforcement_mode = "reject"`; `warn` mode logs and allows. If the sampler is not running yet, admission still applies the configured enforcement mode with `fallback=true` health.
- Time-sensitive scope: Includes offline receipt submissions, attestation flows (twitter binding records/rewards), governance window actions, repo lifecycle actions, staking exit/unbond/finalize, settlement DvP/PvP, ExecuteTrigger calls, trigger registrations whose actions execute time-sensitive instructions, CustomInstruction payloads (treated as time-sensitive by default), and all IVM bytecode transactions.
- Torii rejects NTS-unhealthy admission with `x-iroha-reject-code: PRTRY:NTS_UNHEALTHY`.

### Notes

- Observers and light clients may rely on `time/now` as advisory time; validators use NTS only for timers/timeouts.
- Tune `smoothing_alpha` and `max_adjust_ms_per_min` for your network’s latency characteristics if needed; defaults are conservative.
