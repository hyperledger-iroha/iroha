---
lang: mn
direction: ltr
source: docs/source/sdk/swift/issues/IOS3-CONNECT-004.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 11e56bc4a3ed44d2efbfd52be3a5b8932cdb2e2c7cbf1ae5057e1985cb5cf0f7
source_last_modified: "2025-12-29T18:16:36.076176+00:00"
translation_last_reviewed: 2026-02-07
title: IOS3-CONNECT-004 – Connect WebSocket fixture pack
summary: Capture and publish deterministic WebSocket traces (heartbeat loss, salt rotation, multi-observer) for IOS3 tests.
---

# IOS3-CONNECT-004 – Connect WebSocket fixture pack

Tests for the new Combine/async facades require deterministic replay logs that
cover the roadmap risks (heartbeat loss, salt rotation, queue stress). This
issue captures, documents, and stores the fixtures referenced by IOS3.

## Scope

- Capture the following traces from Torii staging (with sanitized payloads):
  1. **Heartbeat loss + reconnect** – drop two heartbeat frames mid-session to
     force resume with OTLP evidence.
  2. **Salt rotation mid-stream** – rotate salt while frames are in flight to
     validate resume tokens/reset counters.
  3. **Concurrent observers/backpressure** – attach two observers, drive queue
     depth to thresholds, then drain.
- Store fixtures under
  `docs/source/sdk/swift/readiness/archive/2026-05/connect/` with hashes + meta.
- Provide helper loader in `IrohaSwift/Tests/IrohaSwiftTests/ConnectFixtureLoader.swift`.
- Document fixture usage + replay recipe in `docs/source/sdk/swift/connect_replay_fixtures.md`.
- Update `connect_workshop.md` coverage table once fixtures exist.

## Deliverables

1. Sanitized `.json`/`.bin` fixture files with metadata (timestamp, Torii build, hash).
2. Loader utilities + tests verifying fixtures decode into `ConnectFrame`s.
3. Docs describing capture procedure, anonymization steps, and usage in tests.
4. CI wiring to copy fixtures into `artifacts/swift/connect_workshop/<date>/fixtures`.

## Dependencies

- Access to Torii staging + mock harness.
- Coordination with Security for sanitizing data.
- Requires device pool window to record concurrent observer scenarios.

## Timeline

| Date | Action |
|------|--------|
| 2026-05-16 | Begin capture window (device pool reserved). |
| 2026-05-18 | Publish fixtures + metadata. |
| 2026-05-20 | Present fixtures during workshop. |

## Owners

- **Primary:** SDK Program PM.
- **Support:** Swift Connect maintainers (capture tooling), Telemetry TL (OTLP
  payload), Security reviewer (sanitization).

## Status

- ✅ Fixture bundle captured under
  `docs/source/sdk/swift/readiness/archive/2026-05/connect/` with scenario JSON
  (`fixtures/heartbeat_loss.json`, `salt_rotation.json`, `multi_observer.json`),
  `metrics.ndjson`, and the evidence manifest/session descriptors.
- ✅ `ConnectFixtureLoaderTests` parse the manifest/metrics/scenarios in CI so
  schema/shape regressions fail early when fixtures refresh.
- ➜ If Torii replay traces change, regenerate the bundle with the same
  filenames/layout and rerun the loader tests to refresh hashes in status exports.
