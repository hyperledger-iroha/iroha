---
lang: fr
direction: ltr
source: docs/source/sdk/swift/telemetry_redaction.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7b3906385b81bab09b04052bbe0b07a110b3596e989469ff1fad02d5a9362c45
source_last_modified: "2026-01-04T10:50:53.650398+00:00"
translation_last_reviewed: 2026-01-30
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

---
title: Swift Telemetry Redaction Plan
summary: Privacy posture, signal inventory, and validation workflow for the IOS7/IOS8 telemetry rollout.
---

# Swift Telemetry Redaction Plan (IOS7/IOS8)

This document captures the privacy posture and enablement plan for the Swift SDK
as required by roadmap items **IOS7** (Connect end-to-end) and **IOS8** (production
readiness & CI). It mirrors the AND7 Android effort so the mobile SDKs share the
same observability guarantees ahead of the February 2026 SRE governance review.

## Scope & Goals

- Inventory every Swift-emitted signal that lands in the shared observability
  stack (OpenTelemetry spans/events, Prometheus metrics, JSON dashboards).
- Document redaction semantics for Swift-specific metadata (Connect aliases,
  bundle identifiers, device classes) and align them with the Rust/Android
  baselines.
- Define the validation + governance artefacts required before flipping the new
  telemetry on in CI, demo apps, and partner pilots.

Swift reuses the Blake2b-256 hashing strategy (`authority_hash`,
`connect_party_hash`) and the quarter-rotated salt stored in
`iroha_config.telemetry.redaction_salt` so support engineers can correlate
signals across SDKs without exposing raw identifiers.

## Current Artefacts

- `dashboards/data/swift_schema.sample.json` captures the initial schema snapshot for the
  signal inventory above. Keep it in sync with the OTEL descriptors emitted by the SDK.
- `dashboards/data/mobile_parity.sample.json` now includes the `telemetry` summary block
  (salt epoch, overrides, profile alignment) so dashboards and `swift_status_export.py`
  visualize redaction health alongside parity/acceleration data.
- `scripts/swift_status_export.py` and `dashboards/mobile_parity.swift` render the new
  telemetry block and expose it through the weekly digest + Slack summaries.
- `scripts/swift_enrich_parity_feed.py` lets CI jobs merge live telemetry JSON (salt rotation
  gauges, override counts) into the parity feed; `ci/swift_status_export.sh` wires it behind
  environment variables so Buildkite can publish telemetry in lockstep with the digest.
- `scripts/swift_collect_redaction_status.py` synthesizes the telemetry block from salt status
  files (`dashboards/data/swift_salt_status.sample.json`), the override ledger, and (optionally)
  a `telemetry-schema-diff` JSON report so `schema_policy_violations` surface on the dashboard; run
  `python3 scripts/swift_status_export.py telemetry-override {list,create,revoke}` to manage overrides
- `docs/source/sdk/swift/telemetry_chaos_checklist.md` documents the override/salt drift rehearsal
  steps so IOS7 readiness evidence stays reproducible.

## Signal Inventory (Draft)

| Signal ID | Channel | Key Fields | Redaction / Retention | Notes |
|-----------|---------|------------|-----------------------|-------|
| `swift.torii.http.request` | Span (`ToriiClient`) | `authority_hash`, `route`, `status_code`, `latency_ms`, `retry_policy` | `authority_hash = blake2b_256(authority || salt)`; traces retained 30 days | Mirrors Rust `torii.http.request`; `route` excludes query/body data. |
| `swift.torii.http.retry` | Event | `authority_hash`, `route`, `attempt`, `error_code`, `backoff_ms` | Same hash as above; retained 30 days | Captures SDK retry logic for `/v1/pipeline` and `/v1/connect`. |
| `swift.pipeline.submit` | Counter metric | `manifest_kind`, `pipeline_region`, `sdk_mode` | No PII; retained 90 days | Feeds the pipeline dashboard used by `dashboards/mobile_parity.swift`. |
| `swift.pipeline.status` | Event + histogram | `hash_prefix`, `state`, `duration_ms`, `authority_hash` | Export only first 12 hex chars of transaction hash + hashed authority; retained 30 days | Provides parity with Torii pipeline telemetry while keeping hashes short. |
| `swift.connect.session_event` | Event (`ConnectClient`) | `session_alias_hash`, `event_kind`, `wallet_role`, `torii_domain_hash` | Hash alias + Torii domain; retained 30 days | Covers open/approve/reject/close flows for IOS7 readiness. |
| `swift.connect.frame_latency` | Histogram | `session_alias_hash`, `frame_kind`, `latency_ms`, `direction` | Alias hash only; retained 30 days | Detects Connect regressions without leaking raw dApp IDs. |
| `swift.offline.queue_depth` | Gauge | `queue_kind`, `authority_hash`, `device_profile_bucket` | Hash authority + bucket device profile; retained 30 days | Required for offline replay parity with Android (queue kinds: `connect`, `pipeline`). |
| `swift.sdk.error` | Event | `category`, `error_code`, `device_profile_bucket`, `bundle_bucket` | `bundle_bucket` maps to `prod`, `beta`, `internal`; device bucket = `simulator`, `iphone_small`, `iphone_large`, `ipad`, `mac_catalyst`; retained 90 days | Normalises crash/error reporting across platforms. |
| `swift.telemetry.export.status` | Counter | `backend`, `status`, `failure_reason?` | No PII; retained 30 days | Signals exporter health for textfile + OTLP backends. |
| `swift.telemetry.redaction.override` | Event | `override_id`, `actor_role_masked`, `reason`, `expires_at` | `actor_role_masked` exposes category (`support`, `sre`, `audit`); retained 365 days in audit log | Matches override governance flow defined for Android AND7. |
| `swift.telemetry.redaction.salt_version` | Gauge | `salt_epoch`, `rotation_id` | No PII; retained 365 days | Parity alert when Swift salt diverges from Rust/Android rotation. |

All hashed identifiers use the shared Blake2b salt that rotates on the first
Monday of each quarter. The salt lives in the encrypted config bundle shipped
with the demo/XCFramework smoke harness and is pulled from operator `iroha_config`
manifests in production.

## Cross-SDK Device Profile Alignment

Swift adopts the canonical `mobile_profile_class` described in
`docs/source/sdk/mobile_device_profile_alignment.md` so IOS7 telemetry lines up
with Android AND7 data and the shared `dashboards/mobile_parity.swift` overlays:

- `lab` — `device_profile_bucket = simulator`, matching Android
  `hardware_tier = emulator`.
- `consumer` — `device_profile_bucket ∈ {iphone_small, iphone_large, ipad}` and
  therefore grouped with Android’s `hardware_tier = consumer`.
- `enterprise` — `device_profile_bucket = mac_catalyst`, paired with Android’s
  `hardware_tier = enterprise`.

Any new Swift bucket must be added to the alignment document and schema samples
(`dashboards/data/swift_schema.sample.json`) before dashboards consume it.

## Policy Deltas vs Rust Baseline

| Category | Rust Baseline | Swift Policy | Mitigation / Validation |
|----------|---------------|--------------|-------------------------|
| Authority metadata | Plain authority string | `authority_hash` (Blake2b-256 + rotated salt) | Hash helper lives in `IrohaSDKTelemetry`; nightly parity test compares Swift vs Rust outputs for a shared fixture set. |
| Connect session identifiers | Not present in Rust nodes | `session_alias_hash`, `wallet_role` without raw bundle IDs | Connect events emit hashed alias/bundle buckets; governance requires override approval to deanonymise. |
| Device profile & bundle metadata | N/A | Bucketed classes (`simulator`/`iphone_small`/`iphone_large`/`ipad`/`mac_catalyst`) + `bundle_bucket` (`prod`/`beta`/`internal`) | Buckets documented here and in `docs/norito_demo_contributor.md`; chaos rehearsals verify mapping. |
| Offline queue telemetry | Rust nodes expose queue depths per host | Swift gauges hash authority + bucket device profile | Dashboard overlay ensures parity while keeping device info coarse. |
| Override handling | Rust nodes rely on ops overrides only | SDK emits signed override events recorded in Norito | Override events reference support ticket IDs and expire automatically; audit log retained 1 year. |

## Implementation Tasks (Pre-SRE Governance)

1. **Schema harvest** — Materialise the Swift telemetry schema (`swift_telemetry.proto`)
   from the SDK build and publish JSON snapshots under
   `dashboards/data/swift_schema.sample.json`. Owners: Swift Observability TL, LLM.
2. **Exporter plumbing** — Extend `scripts/swift_status_export.py` and
   `dashboards/mobile_ci.swift` to emit/visualise the new metrics (esp.
   `swift.telemetry.redaction.salt_version`). Owner: Swift Program PM.
3. **Unit & integration tests** — Add redaction unit tests covering hashed
   authorities, Connect aliases, and device/bundle bucket mapping. Owners: Swift
   SDK maintainers (tests live under `IrohaSwift/Tests/TelemetryTests.swift`).
4. **Runbook updates** — Update `docs/source/swift_parity_triage.md` with the
   telemetry alert flow (redaction override breaches, exporter failures) and
   cross-link this plan from `docs/norito_demo_contributor.md`. Owner: Docs/Support.
5. **Chaos rehearsal script** — Port the Android AND7 chaos checklist to Swift
   (`scripts/swift_telemetry_chaos.sh`) so governance can replay exporter failures
   before GA. Owner: Swift Observability TL with Telemetry team support.

## Governance & Distribution

- **Pre-read package (due 2026-02-05):** this document, schema diff CSV,
  dashboard screenshots (new `acceleration` + `redaction` panels), and the
  updated parity/runbook snippets.
- **Review forum:** SRE governance (same session as AND7); attendees include
  Swift Observability TL, SDK Program Lead, SRE privacy lead, Android telemetry
  representative, and Docs/Support.
- **Decision logging:** outcomes recorded in `docs/source/sdk/swift/telemetry_redaction_minutes_20260212.md`
  (placeholder to be created after the session) and summarized in `status.md`.

## Audit & Compliance Notes

- Signals omit raw bundle identifiers, Connect endpoints, or Torii credentials.
- Override events are Norito-signed, expire after 30 days unless renewed, and
  retain masked actor roles only.
- Salt rotation evidence (gauge samples + vault change log) must be attached to
  the quarterly compliance packet referenced by IOS8.
- Telemetry data inherits the retention windows defined above; exporters must
  drop local buffers once uploaded to Prometheus/OpenTelemetry to avoid
  accidental long-term storage on developer laptops.

## Follow-Up Tracking

1. **Device-profile alignment (due 2026-03-01).** ✅ Completed — Swift now
   references the shared mapping in `docs/source/sdk/mobile_device_profile_alignment.md`
   so dashboards translate `device_profile_bucket` into the canonical
   `mobile_profile_class` before comparing IOS7 vs AND7 telemetry.
2. **Connect crash telemetry:** Decide whether Connect crash reports reuse the
   same exporters or ship via TestFlight diagnostics. Owner: SDK Program Lead —
   due 2026-03-15.
3. **Override tooling:** ✅ Completed — `python3
   scripts/swift_status_export.py telemetry-override {list,create,revoke}`
   wraps the existing ledger helper so support engineers can raise temporary
   `scripts/swift_telemetry_override.py` entry remains available). Owner:
   Docs/Support.
4. **CI adoption:** Update `ci/xcframework-smoke.yml` to assert the new metrics
   appear in the Buildkite textfile collector before IOS7 beta. Owner: Swift QA
   Lead — due 2026-02-28.
