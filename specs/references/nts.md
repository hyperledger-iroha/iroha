## Network Time Service (NTS)

The Network Time Service provides a robust, byzantine‑resilient estimate of “network time” for node timers and diagnostics. It does not change consensus rules or OS clocks; block header timestamps remain authoritative for acceptance.

### Overview

- Sampling: Periodically probes the accepted, key-ACL-filtered configured logical peers with NTP‑style ping/pong and computes per‑sample offset/RTT. Spoke nodes reach those semantic targets through their hub; authenticated observers outside the configured topology are never quorum inputs. A rotating start position prevents a best-effort queue from permanently favoring the canonical prefix.
- Aggregation: Filters high‑RTT outliers and computes a trimmed median offset with MAD confidence.
- Smoothing (optional): Sample-driven exponential moving average (EMA) with a slew cap, including the first healthy adjustment, to avoid abrupt jumps.
- Determinism: NTS is advisory; consensus validity and block acceptance do not consult NTS.

### Configuration

TOML section `[nts]` (see `specs/references/peer.template.toml` for a template):

- `sample_interval_ms` (u64): Probe period. Default ≈ 5000.
- `sample_cap_per_round` (positive usize): Max peers per round. Default 8.
- `max_rtt_ms` (positive u64): Discard samples above this RTT. Default 500.
- `trim_percent` (u8, 0–45): Symmetric trim percentage (each side) for median. Default 10.
- `per_peer_buffer` (positive usize): Per‑peer ring buffer depth. Default 16.
- `smoothing_enabled` (bool): Enable EMA smoothing. Default false.
- `smoothing_alpha` (finite f64 in [0,1]): EMA alpha; higher = more responsive. Default 0.2.
- `max_adjust_ms_per_min` (u64): Max allowed offset change per minute, in ms. Default 50.
- `min_samples` (positive usize): Minimum peer samples required for healthy status. Default 3; zero is invalid and never disables the quorum.
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

- `GET /v1/time/now` → `{ "now": <ms_epoch>, "offset_ms": <i64>, "confidence_ms": <u64>, "sample_count": <u64>, "peer_count": <u64>, "fallback": <bool>, "health": { "healthy": <bool>, "min_samples_ok": <bool>, "offset_ok": <bool>, "confidence_ok": <bool> } }`
  Time status and `enforcement_mode` come from one service-generation snapshot.
- `GET /v1/time/status` → operator-only diagnostics and RTT histogram buckets.
  The node-local read requires a fresh `OperatorSignature` bound to the exact
  genesis `NetworkId`, `GET`, path, query, and empty body:
  - `{ "peers": <u64>, "samples_used": <u64>, "offset_ms": <i64>, "confidence_ms": <u64>, "fallback": <bool>, "health": {...}, "samples": [{"peer","last_offset_ms","last_rtt_ms","count"}, ...], "rtt": {"buckets": [{"le","count"},...], "sum_ms", "count"}, "note": "NTS running|stopped" }`
  - Status, samples, active policy, and RTT counters are captured under one service lock, so a response never mixes pre- and post-restart state.

### Telemetry Metrics

Exported under Prometheus metrics:

- `nts_offset_ms` (gauge, signed) — smoothed or raw offset vs local clock.
- `nts_confidence_ms` (gauge) — MAD confidence bound.
- `nts_peers_sampled` (gauge) — peers contributing recent samples.
- `nts_samples_used` (gauge) — samples used after RTT filtering.
- `nts_fallback` (gauge) — 1 when falling back to local time.
- `nts_healthy` (gauge) — 1 when health thresholds pass and no fallback.
- `nts_min_samples_ok` / `nts_offset_ok` / `nts_confidence_ok` (gauges) — per-check health flags.
- `nts_rtt_ms_bucket{le="…"}` (gauge) — cumulative RTT histogram buckets (ms), including `+Inf`.
- `nts_rtt_ms_sum` / `nts_rtt_ms_count` — histogram sum/count.

### Behavior & Guarantees

- Determinism: No consensus decisions depend on wall‑clock or NTS; final state remains identical across hardware.
- Safety: NTS never adjusts the OS/system clock; it applies healthy offsets to a bracket-paired monotonic local clock anchor and falls back to that anchor when the sample quorum or health bounds fail. Process-local public network time never moves backwards when offsets or fallback state change. If the monotonic output floor retains a later value, NTS recomputes the reported effective offset, fallback decision, and health flags against that value; reject-mode admission therefore fails closed when the retained offset exceeds policy.
- Performance: Sampling and aggregation are lightweight; per‑peer ring buffers cap retained history and at most one unanswered probe is retained per configured peer. Freshness includes the rounds needed for one complete rotation through the configured topology.
- Admission: Production transaction envelope/TTL checks use the NTS snapshot's monotonic-based `now`; explicit test time sources remain authoritative when supplied. Time-sensitive instructions are gated by the same snapshot's health and policy when `enforcement_mode = "reject"`; `warn` mode logs and allows. Accepted transactions carry that exact validation instant into ordinary, requeue, and globally bound QueuePlan journal records. Durable queue replay revalidates an acknowledged entrypoint at the persisted validation instant without reapplying live NTS health, so a different raw queue clock or a cold sampler cannot invalidate durable state. If the sampler is not running yet, new ingress still applies the configured enforcement mode with `fallback=true` health.
- Time-sensitive scope: Includes Offline receipt-ack submissions, attestation flows (twitter binding records/rewards), governance window actions, both proposal-level and low-level runtime-upgrade operations, repo lifecycle actions, staking exit/unbond/finalize, settlement DvP/PvP, ExecuteTrigger calls, trigger registrations whose actions execute time-sensitive instructions, CustomInstruction payloads (treated as time-sensitive by default), and all IVM bytecode transactions.
- Destructive retention: Irreversible evidence erasure fails closed whenever NTS is stopped, below quorum, outside its health bounds, or otherwise in fallback.
- Lifecycle: Sampler shutdown immediately invalidates retained offsets and RTT state; a later start begins from an empty service rather than reusing stale samples. Any effective configured-peer identity change creates a new membership generation and atomically invalidates prior samples and in-flight probes before that generation contributes. Sampler ownership and admission policy are claimed together, so a failed second startup cannot replace the active policy. Dropping an unpolled or running daemon-supervision future signals shutdown and aborts its Tokio children, releasing the NTS singleton instead of detaching it.
- Slewing: New measurements and contributing-sample expiries advance smoothing at their actual monotonic event times. API read frequency cannot change the applied offset.
- Torii rejects NTS-unhealthy admission with `x-iroha-reject-code: PRTRY:NTS_UNHEALTHY`.

### Notes

- Observers and light clients may rely on `time/now` as advisory time; validators use NTS only for timers/timeouts.
- Tune `smoothing_alpha` and `max_adjust_ms_per_min` for your network’s latency characteristics if needed; defaults are conservative.
