---
lang: ur
direction: rtl
source: docs/source/sumeragi_pacemaker.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b0e7a0ca09fb294fed62bae221a6fd82ce07d9cf0802d90d960afaa5bcec40e9
source_last_modified: "2025-12-09T14:07:26.178015+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- اردو ترجمہ برائے docs/source/sumeragi_pacemaker.md -->

# Sumeragi Pacemaker - ٹائمرز، backoff اور jitter

یہ نوٹ Sumeragi کے pacemaker (timer) پالیسی کی وضاحت کرتا ہے اور آپریٹر رہنمائی اور مثالیں دیتا ہے۔ ٹائمرز یہ طے کرتے ہیں کہ لیڈرز کب propose کریں اور validators کب inactivity کے بعد نئی view تجویز/داخل ہوں۔

Status: EMA سے اخذ کردہ base window، RTT floor، اور configurable jitter/backoff caps نافذ ہیں۔ pacemaker proposal interval اب block-time target اور RTT-floor والے propose timeout کے درمیان clamp کیا جاتا ہے، `sumeragi.advanced.pacemaker.max_backoff_ms` سے اوپر نہیں جاتا۔ jitter ہر node اور ہر (height, view) پر deterministically apply ہوتا ہے۔

## تصورات
- Base window: observed consensus phases کی exponential moving average
  (propose, collect_da, collect_prevote, collect_precommit, commit). EMA کو
  `sumeragi.advanced.npos.timeouts.*_ms` سے seed کیا جاتا ہے؛ جب تک کافی samples نہ آئیں یہ
  عملی طور پر configured defaults سے match کرتی ہے۔ `collect_aggregator` EMA
  observability کیلئے export ہوتی ہے مگر pacemaker window میں شامل نہیں۔
  Smoothed values `sumeragi_phase_latency_ema_ms{phase=...}` کے ذریعے نظر آتی ہیں۔
- Backoff multiplier: `sumeragi.advanced.pacemaker.backoff_multiplier` (default 1). ہر timeout موجودہ window میں `base * multiplier` کا اضافہ کرتا ہے۔
- RTT floor: `avg_rtt_ms * sumeragi.advanced.pacemaker.rtt_floor_multiplier` (default 2). زیادہ latency والے links پر overly aggressive timeouts روکتا ہے۔
- Cap: `sumeragi.advanced.pacemaker.max_backoff_ms` (default 60_000 ms). window کیلئے سخت ceiling.
- Proposal interval seed: `max(effective_block_time_ms, propose_timeout_ms * rtt_floor_multiplier)` اور کبھی `sumeragi.advanced.pacemaker.max_backoff_ms` سے اوپر نہیں۔ (`effective_block_time_ms` = `block_time_ms` scaled by `pacing_factor_bps`). یہ steady-state interval ہے حتی کہ backoff کے بغیر بھی۔

timeout پر effective window اپ ڈیٹ:
- `window = min(cap, max(window + base * backoff_mul, avg_rtt * rtt_floor_mul))`
- جب RTT samples نہ ہوں تو RTT floor = 0.

Exposed telemetria (telemetry.md دیکھیں):
- Runtime: `sumeragi_pacemaker_backoff_ms`, `sumeragi_pacemaker_rtt_floor_ms`, `sumeragi_phase_latency_ema_ms{phase=...}`
- REST snapshot: `/v1/sumeragi/phases` اب تازہ ترین per-phase latencies کے ساتھ `ema_ms` شامل کرتا ہے تاکہ dashboards براہ راست Prometheus scrape کیے بغیر EMA trend دکھا سکیں۔
- Config: `sumeragi.advanced.pacemaker.backoff_multiplier`, `sumeragi.advanced.pacemaker.rtt_floor_multiplier`, `sumeragi.advanced.pacemaker.max_backoff_ms`

## Jitter پالیسی
herd effects (synchronized timeouts) سے بچنے کیلئے pacemaker effective window کے ارد گرد ہر node کیلئے چھوٹا jitter سپورٹ کرتا ہے۔

Deterministic jitter per node:
- Source: `blake2(chain_id || peer_id || height || view)` -> 64-bit، [-J, +J] تک scale کیا جاتا ہے۔
- Recommended jitter band: computed `window` کا +/-10%.
- ہر (height, view) window میں ایک بار apply کریں؛ ایک ہی view میں vary نہ کریں۔

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

نوٹس:
- Jitter ہر node اور (height, view) کیلئے deterministic ہے اور external randomness کی ضرورت نہیں۔
- band کو چھوٹا رکھیں (<= 10%) تاکہ responsiveness برقرار رہے اور stampede کم ہوں۔
- Jitter liveness پر اثر ڈالتا ہے مگر safety پر نہیں۔

Telemetria:
- `sumeragi_pacemaker_jitter_ms` — applied absolute jitter magnitude (ms).
- `sumeragi_pacemaker_jitter_frac_permille` — configured jitter band (permille).

## آپریٹر رہنمائی

Low-latency LAN/PoA
- backoff_multiplier: 1-2
- rtt_floor_multiplier: 2
- max_backoff_ms: 5_000-10_000
- Rationale: proposals کو frequent رکھیں؛ stalls میں recovery مختصر رہے۔

Geo-distributed WAN
- backoff_multiplier: 2-3
- rtt_floor_multiplier: 3-5
- max_backoff_ms: 30_000-60_000
- Rationale: high-RTT links پر aggressive churn سے بچیں؛ bursts tolerate کریں۔

High-variance یا mobile networks
- backoff_multiplier: 3-4
- rtt_floor_multiplier: 4-6
- max_backoff_ms: 60_000
- Rationale: synchronized view changes کم کریں؛ اگر deployment سست commits برداشت کرے تو jitter enable کرنے پر غور کریں۔

## مثالیں

1) LAN میں avg RTT ~= 2 ms, base = 2000 ms, cap = 10_000 ms
- backoff_mul = 1, rtt_floor_mul = 2
- پہلی timeout: window = max(0 + 2000, 2*2) = 2000 ms
- دوسری timeout: window = min(10_000, 2000 + 2000) = 4000 ms

2) WAN میں avg RTT ~= 80 ms, base = 4000 ms, cap = 60_000 ms
- backoff_mul = 2, rtt_floor_mul = 3
- پہلی timeout: window = max(0 + 8000, 80*3) = 8000 ms
- دوسری timeout: window = min(60_000, 8000 + 8000) = 16_000 ms

3) jitter band 10% کے ساتھ (illustrative، implemented نہیں)
- window = 16_000 ms, jitter in [-1_600, +1_600] ms -> `window' in [14_400, 17_600]` ms per node

## مانیٹرنگ
- Trend backoff: `avg_over_time(sumeragi_pacemaker_backoff_ms[5m])`
- Inspect RTT floor: `avg_over_time(sumeragi_pacemaker_rtt_floor_ms[5m])`
- EMA vs histograms: `sumeragi_phase_latency_ema_ms{phase=...}` ساتھ میں `sumeragi_phase_latency_ms{phase=...}` کے matching percentiles
- Verify config: `max(sumeragi.advanced.pacemaker.backoff_multiplier)`, `max(sumeragi.advanced.pacemaker.rtt_floor_multiplier)`, `max(sumeragi.advanced.pacemaker.max_backoff_ms)`

## حتمیت اور سیفٹی
- Timers/backoff/jitter صرف اس بات پر اثر ڈالتے ہیں کہ nodes کب proposals/view-changes trigger کریں؛ signatures کی validity یا commit certificate rules پر اثر نہیں پڑتا۔
- کوئی بھی randomness ہر node اور (height, view) کیلئے deterministic رکھیں۔ time-of-day یا OS RNG کو consensus-critical paths میں استعمال نہ کریں۔

</div>
