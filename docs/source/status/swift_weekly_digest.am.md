---
lang: am
direction: ltr
source: docs/source/status/swift_weekly_digest.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c7f91bb4d3599665a28316ab8d30938f57af7d833a6ba66efa2bf5b41171ba8a
source_last_modified: "2025-12-29T18:16:36.214505+00:00"
translation_last_reviewed: 2026-02-07
title: Swift SDK Weekly Digest - Week of Apr 25 2026
summary: Fixtures remain green post-Apr 24 regen, CI success stays above 95 percent, Connect risks CR-1/CR-2 tracked.
---

# Swift SDK Weekly Digest - Week of Apr 25 2026

## Metrics Snapshot

Captured via:

```bash
python3 scripts/swift_status_export.py \
  --parity dashboards/data/swift/mobile_parity.2026-04-25.json \
  --ci dashboards/data/swift/mobile_ci.2026-04-25.json \
  --ci-signal-lane ci/xcode-swift-parity \
  --ci-signal-lane ci/xcframework-smoke:strongbox
```

```text
## Swift Norito Parity Snapshot (2026-04-25T09:00:00Z)

- Outstanding diffs: 0 (oldest 0.0h)
- Regen SLA: last success 2026-04-24T01:35:00Z; 31.4h since success; breach=no
- Pipeline: status=green; last_run=2026-04-25T08:44:00Z; failed_tests=none
- Acceleration: metal: enabled=True, parity=pass, perf_delta=-13.2%; neon: enabled=True, parity=pass; strongbox: enabled=False, parity=pending
- Telemetry: salt_epoch=2026Q2; rotation_age=36.0h; overrides=1; profile_alignment=ok; schema=ios_metrics/v2
  Notes:
    - Rotation queued for Apr 29 cutover window.
    - Override tracks Connect queue drop monitors.
- Alerts:
  - [INFO] Connect queue instrumentation still pending (CR-2); keep queue-depth alerts manual until telemetry lands.

### CI Signals

- swift_parity_success_total: metrics state unavailable
- ci/xcode-swift-parity (parity): success=97% | last_failure=2026-04-24T12:18:00Z | flakes=1 | mttr=6.2h
- ci/xcframework-smoke:strongbox (strongbox): success=95% | last_failure=2026-04-21T02:10:00Z | flakes=1 | mttr=7.5h

### Swift CI Snapshot (2026-04-25T09:05:00Z)

- Queue depth: 1; avg runtime: 33.0 min
- Consecutive failures: 0; incidents: none
- Acceleration bench: metal_vs_cpu_merkle_ms: cpu=8.3 metal=3.5 (delta=-57.8%); neon_crc64_throughput_mb_s=924.7
- Device results: emulators 16/0 | strongbox 4/0
- Lane summary (last 14 runs):
- ci/xcode-swift-parity (parity): success=97% | last_failure=2026-04-24T12:18:00Z | flakes=1 | mttr=6.2h
- ci/xcframework-smoke:iphone-sim (iphone-sim): success=96% | last_failure=2026-04-23T19:02:00Z | flakes=1 | mttr=5.0h
- ci/xcframework-smoke:ipad-sim (ipad-sim): success=100% | last_failure=n/a | flakes=0 | mttr=0.0h
- ci/xcframework-smoke:strongbox (strongbox): success=95% | last_failure=2026-04-21T02:10:00Z | flakes=1 | mttr=7.5h
- ci/xcframework-smoke:macos-fallback (mac-fallback): success=100% | last_failure=n/a | flakes=0 | mttr=0.0h
```

## Highlights

- Fixture regen cadence remains on schedule: no diffs since the Apr 24 rotation, so the next `scripts/swift_fixture_regen.sh` fallback window (Apr 27) stays optional unless telemetry overrides expand beyond the single Connect queue probe.
- StrongBox smoke runs recovered to 95 % success after the Metal warm-up patch; monitoring continues on `ci/xcframework-smoke:strongbox` to ensure the MTTR trend (7.5 h) keeps dropping ahead of the AND8 pilot freeze.
- IOS3 Connect workshop closed (2026-05-20); decisions/fixtures captured under `docs/source/sdk/swift/readiness/archive/2026-05/connect/` with notes in `workshop-notes.md` and CI coverage via `ConnectFixtureLoaderTests`. Issues IOS3-CONNECT-001–004 now carry the outcomes + evidence links.
- Connect AsyncStream/Combine facades remain shipped: `ConnectSession.eventStream`/`eventsPublisher` and `balanceStream`/`balancePublisher` feed `ConnectEvent`/`ConnectBalanceSnapshot` values into SwiftUI + telemetry, with queue diagnostics and NDJSON exports exercised by the new fixture bundle.

## Risks & Mitigations

| Risk | Owner | Status | Notes |
|------|-------|--------|-------|
| CR-1 - Connect codec now fails closed on missing bridge | Swift SDK Lead | **Medium** | `ConnectCodec` throws `ConnectCodecError.bridgeUnavailable` and tests cover the bridge-backed fixtures; remaining work is keeping the xcframework bundled across SPM/Carthage as outlined in `docs/source/sdk/swift/connect_risk_tracker.md`. |
| CR-2 - No telemetry on queue depth / flow control | SDK Program Lead | **High** | Queue/back-pressure structs inside `ConnectClient` remain dark; the workshop closed with the NDJSON/manifest export path (`ConnectSessionDiagnostics`) and fixtures, but live OTLP wiring is still pending. |
| CR-3 - Missing key custody & attestation plumbing | Swift SDK Lead | **Medium** | Secure Enclave-backed `ConnectKeyStore` work has not started and wallets still emit raw bridge keys, so attested approvals can't satisfy IOS3 acceptance yet. |

## Governance Watchers

| Item | Owner | Status | Notes |
|------|-------|--------|-------|
| `/v2/pipeline` adoption | Swift SDK Lead | Tracking | `/v2/pipeline` staging runs remain green and the digest now flags the CR-2 dependency so governance can see why Connect queue telemetry still blocks GA. |
| Telemetry readiness (`connect.queue_*`) | Observability TL | Attention needed | Queue-depth OTLP exporters still pending; `scripts/swift_status_export.py` reports `swift_telemetry_overrides_open=1` and CR-2 remains high until the Connect queue probes emit real metrics. |
| Torii spec review (IOS2) | Torii Platform TL | Scheduled | Session locked for 2026-11-21 (agenda/invite in `docs/source/sdk/swift/torii_spec_review.md`) to freeze `/v2/pipeline` + Norito RPC behaviour for IOS2. |
| Governance vote / readiness review | SDK Program PM | Scheduled | Council review pencilled in for 2026-05-02 to confirm Connect instrumentation + `/v2/pipeline` evidence; see `status.md` for the briefing packet. |
| Risk owner spotlight | Observability TL | Attention needed | SRE requested weekly confirmation that queue-depth telemetry is still dark; the risks/notes above point to the OTLP work that must land before the vote proceeds. |

## Upcoming Actions

1. Update the xcframework/SPM/Carthage bundling checklist to guarantee the Norito bridge ships alongside the new fail-closed codec (CR-1 follow-up).
2. Prototype the bounded `ConnectQueue` + OTLP exporters so queue-depth telemetry can flip on ahead of the Connect strawman review (CR-2 mitigation); reuse the new fixtures to validate exporters.
3. Mirror the 2026-05-20 workshop bundle hash/path into the next status export and rerun `ConnectFixtureLoaderTests` whenever fixtures regenerate.

## Links & Artefacts

- Parity feed: `dashboards/data/swift/mobile_parity.2026-04-25.json`
- CI feed: `dashboards/data/swift/mobile_ci.2026-04-25.json`
- Dashboards/scripts: `dashboards/mobile_parity.swift`, `dashboards/mobile_ci.swift`, `scripts/swift_status_export.py`
- Risk tracker: `docs/source/sdk/swift/connect_risk_tracker.md`

## CI Automation

Run `ci/swift_status_export.sh` from Buildkite (or any CI host) to keep this
digest current. When the telemetry feeds are live, set the archive knobs so
weekly drops are stored automatically:

```bash
export SWIFT_STATUS_ARCHIVE_DIR=docs/source/status
export SWIFT_STATUS_ARCHIVE_TAG="$(date -u +%Y-%m-%d)"
# Optional override if you prefer a different prefix than swift_weekly_digest
export SWIFT_STATUS_ARCHIVE_BASENAME=swift_weekly_digest
# Optional: surface the archived digest path to Buildkite annotations
export SWIFT_STATUS_ARCHIVE_META_KEY=swift_status_export.archive_path
# Optional: also export the JSON summary path for downstream steps
export SWIFT_STATUS_ARCHIVE_SUMMARY_META_KEY=swift_status_export.summary_path

bash ci/swift_status_export.sh
```

By default the CI wrapper also writes Prometheus metrics to
`artifacts/swift/swift_status_metrics.prom` and persists the counter state in
`artifacts/swift/swift_status_metrics_state.json` so the digest no longer shows
“metrics state unavailable.” Override those paths with
`SWIFT_STATUS_METRICS_PATH` / `SWIFT_STATUS_METRICS_STATE` or disable the
automatic export entirely with `SWIFT_STATUS_DISABLE_METRICS=1` when a build
needs to skip telemetry output.

The CI wrapper copies the generated Markdown digest and JSON summary to
`${SWIFT_STATUS_ARCHIVE_DIR}/${SWIFT_STATUS_ARCHIVE_BASENAME}_${SWIFT_STATUS_ARCHIVE_TAG}.md`
and `.json`, so ops can publish the artefacts straight to Docs/Status without
manual renaming.
