---
lang: he
direction: rtl
source: docs/source/soranet/av_sync_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7f3afa23794f593aff7206a8628e53ba611973d894012d574272d157f2cb3340
source_last_modified: "2026-01-03T18:08:01.741826+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: Streaming A/V Sync Enforcement
summary: Specification for NSC-28b covering telemetry, validation, and enforcement of audio/video sync tolerance.
---

# Streaming A/V Sync Enforcement

## Goals & Scope
- Track per-hop audio/video timing in SoraNet streaming, enforce ±10 ms tolerance, and surface anomalies via telemetry (NSC-28b).
- Ensure validators/gateways reject out-of-sync segments without enabling griefing attacks.

## Telemetry
- Extend `StreamingTelemetry` with fields:
  - `audio_pts`, `video_pts` (presentation timestamps).
  - `pts_delta_ms` (difference between audio/video frames).
  - `hop_latency_ms` for each relay/gateway.
- Collect at each streaming hop; emit to Prometheus (`sorafs_streaming_av_delta_ms`, `sorafs_streaming_hop_latency_ms`).

## Validation
- Streaming auditors now execute inside `StreamingHandle::handle_receiver_report`. The gate reads `ReceiverReport.sync_diagnostics`, compares the metrics against the configured policy (`streaming.sync`), and raises `AudioSyncViolation` when drift crosses the thresholds.
  - Hard failures occur whenever `max_av_drift_ms` exceeds `streaming.sync.hard_cap_ms` (default 12 ms).
  - Sustained EWMA drift beyond `streaming.sync.ewma_threshold_ms` (default 10 ms) triggers rejections once the reporting window meets `min_window_ms` (default 5 s).
  - Operators can run the gate in observe-only mode via `streaming.sync.observe_only = true`; setting it to `false` enforces hard failures.
- Integration harness simulates happy path, jitter, and malicious offset. Negative tests assert that `AudioSyncViolation` surfaces in Torii logs/CLI responses (mapped to `AV_SYNC_VIOLATION` for REST consumers).

## Observability
- Alerts when `pts_delta_ms` gauge exceeds thresholds.
- Dashboards showing distribution of sync deltas per stream and per relay.
- Logging: `av_sync_event` with stream ID, delta, action.

## Rollout
1. Implement telemetry updates and segment auditor changes.
2. Run impaired network tests to calibrate hysteresis.
3. Deploy to Testus with `streaming.sync.enabled = true` but `observe_only = true`; monitor telemetry and alerts.
4. Flip `streaming.sync.observe_only = false` (and, if necessary, adjust thresholds) once metrics stabilize; enable in Nexus afterwards.
