---
lang: ru
direction: ltr
source: docs/source/sdk/swift/readiness/labs/swift_telemetry_lab_01.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 00798397042c61defb8483aed4818b9ad4f393e7e7d7935a00e43400fc42a576
source_last_modified: "2026-01-03T18:08:01.598478+00:00"
translation_last_reviewed: 2026-01-30
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Swift Telemetry Lab 01 — Chaos & Readiness Scenarios

This lab guide complements `docs/source/sdk/swift/telemetry_chaos_checklist.md`
and enumerates the exact Swift tooling, staging assets, and evidence capture
steps required before the IOS7/IOS8 readiness review. Each scenario should take
≤10 minutes and must be recorded in the readiness archive.

## Scenario Matrix

| Scenario | Objective | Tooling & Commands | Evidence |
|----------|-----------|--------------------|----------|
| A — Salt Drift Injection | Prove salt alerts fire + recover cleanly. | `python3 scripts/swift_collect_redaction_status.py --inject-drift --preview` then `--clear-drift`. | Alert screenshot, `swift.telemetry.redaction.salt_version` gauge sample, entry in `archive/2026-03/salt_drift.log`. |
| B — Override Ledger Workflow | Exercise Norito override creation/removal. | `python3 scripts/swift_status_export.py telemetry-override create …`, confirm exporter counters, then `revoke`. | CLI output, ledger diff, support ticket reference, screenshot of `swift.telemetry.redaction.override`. |
| C — Exporter Outage Drill | Validate buffered telemetry + alerting. | Stop local OTEL collector (e.g., `pkill -STOP otelcol-swift`), run `swift_status_export.py telemetry --once`, restart collector. | Exporter alert screenshot, buffered queue log, `mobile_parity.swift` capture before/after. |
| D — Offline Queue Replay | Confirm `swift.offline.queue_depth` parity + replay runbook. | Run NoritoDemoXcode offline flow or `swift test --filter OfflineQueueSmokeTests`, toggle airplane mode, replay once connectivity restored. | Pending queue snapshots, `docs/source/sdk/swift/readiness/screenshots/<date>/queue-replay.png`. |
| E — Connect Session Fault Injection | Ensure Connect telemetry surfaces latency regressions. | Use mock dApp harness (`examples/ios/NoritoDemoXcode/Tools/ConnectHarness.swift`) to inject artificial delay; watch `swift.connect.frame_latency`. | Histogram capture, log snippet, note in `archive/2026-03/connect_fault.md`. |

## Execution Checklist

1. Reserve Torii staging hosts + Connect mock service (`torii-staging-ios7`).
2. Export `IROHA_CONFIG_PATH` pointing to the redaction-ready manifest.
3. Run the scenario commands above; leave terminals open for screenshot capture.
4. Store logs under `docs/source/sdk/swift/readiness/archive/YYYY-MM/`.
5. Upload screenshots to `docs/source/sdk/swift/readiness/screenshots/<date>/`
   and note filenames inside the archive README.
6. Update `status.md` telemetry notes if any scenario uncovers new action items.

## Reporting Template

```
Scenario: <A-E>
Date: 2026-02-28
Participants: Mei Nakamura, LLM
Commands executed:
  - python3 scripts/swift_collect_redaction_status.py --inject-drift --preview
Findings:
  - Alert fired within 45s, auto-cleared after salt reset.
Evidence:
  - screenshots/2026-02-28/salt-drift.png
  - archive/2026-03/salt_drift.log
Follow-ups:
  - None
```

Store completed templates in `docs/source/sdk/swift/readiness/archive/2026-03/`.
