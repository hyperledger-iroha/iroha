---
lang: hy
direction: ltr
source: docs/source/sumeragi_pacemaker.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e8fc392a8d40af460b58efb3e238953eea43dc7b45c6e58d842dcf9f3b94c26b
source_last_modified: "2026-02-01T05:44:29.201509+00:00"
translation_last_reviewed: 2026-02-07
---

# Sumeragi Pacemaker — Timers, Backoff, and Jitter

This note outlines the pacemaker (timer) policy for Sumeragi and provides operator guidance and examples. Timers govern when leaders propose and when validators suggest/enter a new view after inactivity.

Status: implemented EMA-derived base window, RTT floor, and configurable jitter/backoff caps. The pacemaker proposal interval is now clamped between the block-time target and the RTT‑floored propose timeout, capped by `sumeragi.advanced.pacemaker.max_backoff_ms`. Jitter is applied deterministically per node and per (height, view).

## Concepts
- Base window: exponential moving average of the observed consensus phases
  (propose, collect_da, collect_prevote, collect_precommit, commit). The EMA is
  seeded from `sumeragi.advanced.npos.timeouts.*_ms`; until sufficient samples arrive it
  effectively matches the configured defaults. Derived NPoS timeouts are normalized so
  `propose + collect_da + collect_prevote + collect_precommit + commit` tracks the
  target block time, keeping the pipeline budget consistent at 1 Hz. `collect_aggregator`
  EMA is exported for observability but is not included in the pacemaker window. The
  smoothed values surface via `sumeragi_phase_latency_ema_ms{phase=…}`.
- Backoff multiplier: `sumeragi.advanced.pacemaker.backoff_multiplier` (default 1). Each timeout adds `base * multiplier` to the current window.
- RTT floor: `avg_rtt_ms * sumeragi.advanced.pacemaker.rtt_floor_multiplier` (default 2). Prevents too-aggressive timeouts on higher-latency links.
- Cap: `sumeragi.advanced.pacemaker.max_backoff_ms` (default 60_000 ms). Hard ceiling on the window.
- Proposal interval seed: `max(effective_block_time_ms, propose_timeout_ms * rtt_floor_multiplier)` and never above `sumeragi.advanced.pacemaker.max_backoff_ms`. Here `effective_block_time_ms` is the on-chain `block_time_ms` scaled by `pacing_factor_bps` (basis points). This is the steady-state interval even without backoff.
- Pending-block gating: proposals defer while a tip-extending pending block is unresolved. If no votes/QCs arrive by `min(effective_block_time, effective_commit_time)`, proposal backpressure lifts even before the full quorum-timeout window to avoid long stalls. The fast path is disabled once any votes/QCs are observed for the pending block. In DA mode, fast reschedule is disabled by default; enable `sumeragi.advanced.pacemaker.da_fast_reschedule` to allow fast-timeout reschedules when the payload is locally available and no votes/QCs are observed.

Effective window update on timeout:
- `window = min(cap, max(window + base * backoff_mul, avg_rtt * rtt_floor_mul))`
- When there are no RTT samples, the RTT floor is 0.

Exposed telemetry (see telemetry.md):
- Runtime: `sumeragi_pacemaker_backoff_ms`, `sumeragi_pacemaker_rtt_floor_ms`, `sumeragi_phase_latency_ema_ms{phase=…}`
- Backpressure: `sumeragi_pacemaker_backpressure_deferrals_by_reason_total{reason=…}`, `sumeragi_pacemaker_backpressure_deferral_age_ms{reason=…}`
- Tick timings: `sumeragi_pacemaker_eval_ms`, `sumeragi_pacemaker_propose_ms`
- REST snapshot: `/v2/sumeragi/phases` now includes `ema_ms` alongside the latest
  per-phase latencies so dashboards can plot the EMA trend without scraping
  Prometheus directly.
- Config: `sumeragi.advanced.pacemaker.backoff_multiplier`, `sumeragi.advanced.pacemaker.rtt_floor_multiplier`, `sumeragi.advanced.pacemaker.max_backoff_ms`

## Jitter Policy
To avoid herd effects (synchronized timeouts), the pacemaker supports a small per-node jitter around the effective window.

Deterministic jitter per node:
- Source: `blake2(chain_id || peer_id || height || view)` → 64-bit, scaled to [−J, +J].
- Recommended jitter band: ±10% of the computed `window`.
- Apply once per (height, view) window; do not vary within the same view.

Pseudocode:
```
base = ema_total_ms(view, height)  // seeded by sumeragi.advanced.npos.timeouts.*_ms
window = min(cap, max(prev + base * backoff_mul, avg_rtt * rtt_floor_mul))
seed = blake2(chain_id || peer_id || height || view)
u = (seed % 10_000) as f64 / 10_000.0  // [0, 1)
jfrac = 0.10 // 10% jitter band
jitter = (u * 2.0 - 1.0) * jfrac * window.as_millis() as f64
window_jittered_ms = (window.as_millis() as f64 + jitter).clamp(0.0, cap_ms as f64)
```

Notes:
- Jitter is deterministic per node and (height, view) and does not require external randomness.
- Keep the band small (≤ 10%) to preserve responsiveness while reducing stampedes.
- Jitter affects liveness but not safety.

Telemetry:
- `sumeragi_pacemaker_jitter_ms` — absolute jitter magnitude (ms) applied.
- `sumeragi_pacemaker_jitter_frac_permille` — configured jitter band (permille).

## Operator Guidance

Low-latency LAN/PoA
- backoff_multiplier: 1–2
- rtt_floor_multiplier: 2
- max_backoff_ms: 5_000–10_000
- Rationale: keep proposals frequent; short recovery under stalls.

Geo-distributed WAN
- backoff_multiplier: 2–3
- rtt_floor_multiplier: 3–5
- max_backoff_ms: 30_000–60_000
- Rationale: avoid aggressive churn across high-RTT links; tolerate bursts.

High-variance or mobile networks
- backoff_multiplier: 3–4
- rtt_floor_multiplier: 4–6
- max_backoff_ms: 60_000
- Rationale: minimize synchronized view changes; consider enabling jitter when the deployment tolerates slower commits.

## Examples

1) LAN with avg RTT ≈ 2 ms, base = 2000 ms, cap = 10_000 ms
- backoff_mul = 1, rtt_floor_mul = 2
- First timeout: window = max(0 + 2000, 2*2) = 2000 ms
- Second timeout: window = min(10_000, 2000 + 2000) = 4000 ms

2) WAN with avg RTT ≈ 80 ms, base = 4000 ms, cap = 60_000 ms
- backoff_mul = 2, rtt_floor_mul = 3
- First timeout: window = max(0 + 8000, 80*3) = 8000 ms
- Second timeout: window = min(60_000, 8000 + 8000) = 16_000 ms

3) With jitter band 10% (illustrative, not implemented)
- window = 16_000 ms, jitter ∈ [−1_600, +1_600] ms → `window' ∈ [14_400, 17_600]` ms per node

## Monitoring
- Trend backoff: `avg_over_time(sumeragi_pacemaker_backoff_ms[5m])`
- Inspect RTT floor: `avg_over_time(sumeragi_pacemaker_rtt_floor_ms[5m])`
- Compare EMA vs histograms: `sumeragi_phase_latency_ema_ms{phase=…}` alongside the matching `sumeragi_phase_latency_ms{phase=…}` percentiles
- Verify config: `max(sumeragi.advanced.pacemaker.backoff_multiplier)`, `max(sumeragi.advanced.pacemaker.rtt_floor_multiplier)`, `max(sumeragi.advanced.pacemaker.max_backoff_ms)`

## Determinism & Safety
- Timers/backoff/jitter influence only when nodes trigger proposals/view-changes; they do not affect signature validity or commit-certificate rules.
- Keep any randomness deterministic per node and per (height, view). Avoid time-of-day or OS RNG in consensus-critical paths.
