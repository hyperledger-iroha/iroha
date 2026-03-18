---
lang: ja
direction: ltr
source: docs/source/project_tracker/nsc28b_av_sync_telemetry.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4852427ddfc61cb8e76c9b40e656ed4d898d472eb6e9ac5d209a06c6acf5eade
source_last_modified: "2026-01-03T18:08:01.672660+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Outline for NSC-28b A/V sync enforcement telemetry spec. -->

# NSC-28b — A/V Sync Telemetry & Validation Plan

## Objectives
- Define deterministic jitter/drift metrics emitted by validators and viewers.
- Establish ±10 ms enforcement thresholds, roll-forward tolerances, and fallback behaviour.
- Extend `SegmentAuditor` plus integration harnesses to block segments that violate sync policy.

## Signal Specification (Draft)
- **Metrics**
  - `streaming_audio_jitter_ms` (histogram): per-hop jitter measured between encoded audio timestamps and arrival time at validator.
  - `streaming_av_drift_ms` (gauge): cumulative drift between audio frame presentation and paired video frame.
  - `streaming_av_sync_violation_total` (counter): increments when drift exceeds enforcement threshold.
  - `streaming_av_sync_window_ms` (gauge): configured enforcement window (default 10 ms).
- **Norito Payload Updates**
  - Extend `TelemetryEncodeStats` with `max_audio_jitter_ms`, `avg_audio_jitter_ms`.
  - Extend `TelemetryDecodeStats` with `max_av_drift_ms`, `avg_av_drift_ms`.
  - Add optional `SyncDiagnostics` sub-struct to `ReceiverReport`.
- **Collection Points**
  - Publisher ingress: capture encoder → manifest emission timestamps.
  - Viewer decode: measure arrival → decode completion.
  - Validator audit: compare reported frame timestamps with canonical schedule.
- **Aggregation**
  - EWMA window: 5 seconds by default, configurable via telemetry config.
  - Hard cap: reject single frame drift > 12 ms; sustained EWMA > 10 ms triggers throttle.

## Enforcement Plan
1. Instrument telemetry counters in `StreamingTelemetry` and expose via Prometheus/OTLP.
2. Update the streaming runtime to:
   - Read EWMA/drift samples from `ReceiverReport.sync_diagnostics`.
   - Reject segments when thresholds breeched and return the `AudioSyncViolation` error surfaced via Torii/CLI.
3. Extend integration tests:
   - `norito_streaming_negative.rs`: simulate clock skew, verify rejection.
   - New fixture injecting delayed audio frames.
   - Add Torii API tests to assert `AudioSyncViolation` error surface + CLI messaging.
4. Update Torii/SDK handling:
   - Map validator error to Torii status code & CLI output.
   - Provide SDK hooks for displaying sync warnings.
5. Rollout steps:
   - Phase 1 (observe-only): enable metrics, log warnings.
   - Phase 2 (soft enforcement): reject but allow override via config.
   - Phase 3 (hard enforcement): remove override once telemetry stable.

## Draft Checklist
- [x] Finalise metric names and Prometheus buckets.
- [x] Update Norito telemetry structs + regenerate schema hashes.
- [x] Implement EWMA evaluation + config knob (`streaming.sync.*`).
- [x] Add `AudioSyncViolation` error mapping in Torii/CLI + SDK adapters.
- [ ] Document operator rollout (status.md, norito_streaming.md).
- [ ] Coordinate with SDK teams for UI/alert updates when sync violations occur.

## Dependencies & Risks
- Ensure clocks (PTP/NTP) are synchronised across validators to avoid false positives.
- Need telemetry storage adjustments for additional histograms.
- Coordinate with SDK teams to surface sync warnings in clients.

## Meeting Prep
- **Target review window:** week of **2026-03-02**.
- **Participants:** Streaming Runtime TL, Telemetry Ops WG, Validator Ops, SDK leads.
- **Pre-read:** this draft, current telemetry inventory, integration test plan.
- **Expected outcome:** sign-off on metric definitions and enforcement phases.
