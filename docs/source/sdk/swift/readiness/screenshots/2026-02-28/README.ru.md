---
lang: ru
direction: ltr
source: docs/source/sdk/swift/readiness/screenshots/2026-02-28/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 05ebe051878041eec44cb0e55292caa8088013a9bc707185bbf4b72ef35cd3ab
source_last_modified: "2026-01-03T18:08:01.707521+00:00"
translation_last_reviewed: 2026-01-30
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Screenshot Index — Swift Telemetry Dry-Run (2026-02-28)

Add PNG/JPEG captures from the rehearsal here. Files are stored in
`s3://sora-readiness/swift/telemetry/20260228/` to avoid bloating the repo; the
table below lists canonical object names referenced by the archive notes.

| File | Scenario | Description |
|------|----------|-------------|
| `s3://sora-readiness/swift/telemetry/20260228/20260228-salt-drift-alert.png` | A | `mobile_parity.swift` alert panel showing `swift.telemetry.redaction.salt_drift` firing and clearing. |
| `s3://sora-readiness/swift/telemetry/20260228/20260228-override-ledger.png` | B | Override ledger entry created via `scripts/swift_status_export.py telemetry-override create …`. |
| `s3://sora-readiness/swift/telemetry/20260228/20260228-exporter-outage.png` | C | Exporter outage dashboard with paused OTLP collector, highlighting suppression + recovery timeline. |
| `s3://sora-readiness/swift/telemetry/20260228/20260228-offline-queue.png` | D | Offline queue replay metrics comparing pre/post airplane-mode run. |
| `s3://sora-readiness/swift/telemetry/20260228/20260228-connect-latency.png` | E | Connect latency histogram showing induced 450 ms spike and alert annotation. |
