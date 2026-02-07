---
lang: dz
direction: ltr
source: docs/portal/docs/sdks/android-telemetry-redaction.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1c9ce5a256683152440986a028d1e57ed298bbbb189196af866128962228caa5
source_last_modified: "2026-01-05T09:28:11.847834+00:00"
translation_last_reviewed: 2026-02-07
title: Android Telemetry Redaction Plan
sidebar_label: Android Telemetry
slug: /sdks/android-telemetry
---

:::note Canonical Source
:::

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android Telemetry Redaction Plan (AND7)

## Scope

This document captures the proposed telemetry redaction policy and enablement
artefacts for the Android SDK as required by roadmap item **AND7**. It aligns
mobile instrumentation with the Rust node baseline while accounting for
device-specific privacy guarantees. The output serves as the pre-read for the
February 2026 SRE governance review.

Objectives:

- Catalogue every Android-emitted signal that reaches shared observability
  backends (OpenTelemetry traces, Norito-encoded logs, metrics exports).
- Classify fields that differ from the Rust baseline and document redaction or
  retention controls.
- Outline enablement and testing work so support teams can respond
  deterministically to redaction-related alerts.

## Signal Inventory (Draft)

Planned instrumentation grouped by channel. All field names follow the Android
SDK telemetry schema (`org.hyperledger.iroha.android.telemetry.*`). Optional
fields are marked with `?`.

| Signal ID | Channel | Key Fields | PII/PHI Classification | Redaction / Retention | Notes |
|-----------|---------|------------|------------------------|-----------------------|-------|
| `android.torii.http.request` | Trace span | `authority_hash`, `route`, `status_code`, `latency_ms` | Authority is public; route contains no secrets | Emit hashed authority (`blake2b_256`) before export; retain for 7 days | Mirrors Rust `torii.http.request`; hashing ensures mobile alias privacy. |
| `android.torii.http.retry` | Event | `route`, `retry_count`, `error_code`, `backoff_ms` | None | No redaction; retain 30 days | Used for deterministic retry audits; identical to Rust fields. |
| `android.pending_queue.depth` | Gauge metric | `queue_type`, `depth` | None | No redaction; retain 90 days | Matches Rust `pipeline.pending_queue_depth`. |
| `android.keystore.attestation.result` | Event | `alias_label`, `security_level`, `attestation_digest`, `device_brand_bucket` | Alias (derived), device metadata | Replace alias with deterministic label, redact brand to enum bucket | Required for AND2 attestation readiness; Rust nodes do not emit device metadata. |
| `android.keystore.attestation.failure` | Counter | `alias_label`, `failure_reason` | None after alias redaction | No redaction; retain 90 days | Supports chaos drills; alias_label derived from hashed alias. |
| `android.telemetry.redaction.override` | Event | `override_id`, `actor_role_masked`, `reason`, `expires_at` | Actor role qualifies as operational PII | Field exports masked role category; retain 365 days with audit log | Not present in Rust; operators must file overrides through support. |
| `android.telemetry.export.status` | Counter | `backend`, `status` | None | No redaction; retain 30 days | Parity with Rust exporter status counters. |
| `android.telemetry.redaction.failure` | Counter | `signal_id`, `reason` | None | No redaction; retain 30 days | Required to mirror Rust `streaming_privacy_redaction_fail_total`. |
| `android.telemetry.device_profile` | Gauge | `profile_id`, `sdk_level`, `hardware_tier` | Device metadata | Emit coarse buckets (SDK major, hardware tier); retain 30 days | Enables parity dashboards without exposing OEM specifics. |
| `android.telemetry.network_context` | Event | `network_type`, `roaming` | Carrier may be PII | Drop `carrier_name` entirely; retain other fields 7 days | `ClientConfig.networkContextProvider` supplies the sanitised snapshot so apps can emit network type + roaming without exposing subscriber data; parity dashboards treat the signal as the mobile analogue to Rust `peer_host`. |
| `android.telemetry.config.reload` | Event | `source`, `result`, `duration_ms` | None | No redaction; retain 30 days | Mirrors Rust config reload spans. |
| `android.telemetry.chaos.scenario` | Event | `scenario_id`, `outcome`, `duration_ms`, `device_profile` | Device profile is bucketed | Same as `device_profile`; retain 30 days | Logged during chaos rehearsals required for AND7 readiness. |
| `android.telemetry.redaction.salt_version` | Gauge | `salt_epoch`, `rotation_id` | None | No redaction; retain 365 days | Tracks Blake2b salt rotation; parity alert when Android hash epoch diverges from Rust nodes. |
| `android.crash.report.capture` | Event | `crash_id`, `signal`, `process_state`, `has_native_trace`, `anr_watchdog_bucket` | Crash fingerprint + process metadata | Hash `crash_id` with the shared redaction salt, bucket watchdog state, drop stack frames before export; retain 30 days | Enabled automatically when `ClientConfig.Builder.enableCrashTelemetryHandler()` is called; feeds parity dashboards without exposing device-identifying traces. |
| `android.crash.report.upload` | Counter | `crash_id`, `backend`, `status`, `retry_count` | Crash fingerprint | Reuse hashed `crash_id`, emit status only; retain 30 days | Emit via `ClientConfig.crashTelemetryReporter()` or `CrashTelemetryHandler.recordUpload` so uploads share the same Sigstore/OLTP guarantees as other telemetry. |

### Implementation Hooks

- `ClientConfig` now threads manifest-derived telemetry data via
  `setTelemetryOptions(...)`/`setTelemetrySink(...)`, automatically registering
  `TelemetryObserver` so hashed authorities and salt metrics flow without bespoke observers.
  See `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java`
  and the companion classes under
  `java/iroha_android/src/main/java/org/hyperledger/iroha/android/telemetry/`.
- Applications can call
  `ClientConfig.Builder.enableAndroidNetworkContext(android.content.Context)` to register the
  reflection-based `AndroidNetworkContextProvider`, which queries `ConnectivityManager` at runtime
  and emits the `android.telemetry.network_context` event without introducing compile-time Android
  dependencies.
- Unit tests `TelemetryOptionsTests` and `TelemetryObserverTests`
  (`java/iroha_android/src/test/java/org/hyperledger/iroha/android/telemetry/`) guard the hashing
  helpers plus the ClientConfig integration hook so manifest regressions surface immediately.
- The enablement kit/labs now cite concrete APIs instead of pseudocode, keeping this document and
  the runbook aligned with the shipping SDK.

> **Operations note:** the owner/status worksheet lives at
> `docs/source/sdk/android/readiness/signal_inventory_worksheet.md` and must be
> updated alongside this table during every AND7 checkpoint.

## Parity allowlists & schema-diff workflow

Governance requires a dual allowlist so Android exports never leak identifiers
that Rust services intentionally surface. This section mirrors the runbook entry
(`docs/source/android_runbook.md` §2.3) but keeps the AND7 redaction plan
self-contained.

| Category | Android exporters | Rust services | Validation hook |
|----------|-------------------|---------------|-----------------|
| Authority / route context | Hash `authority`/`alias` via Blake2b-256 and drop raw Torii hostnames before export; emit `android.telemetry.redaction.salt_version` to prove salt rotation. | Emit full Torii hostnames and peer IDs for correlation. | Compare `android.torii.http.request` vs `torii.http.request` entries in the latest schema diff under `docs/source/sdk/android/readiness/schema_diffs/`, then run `scripts/telemetry/check_redaction_status.py` to confirm salt epochs. |
| Device & signer identity | Bucket `hardware_tier`/`device_profile`, hash controller aliases, and never export serial numbers. | Emit validator `peer_id`, controller `public_key`, and queue hashes verbatim. | Align with `docs/source/sdk/mobile_device_profile_alignment.md`, exercise alias hashing tests inside `java/iroha_android/run_tests.sh`, and archive queue-inspector outputs during labs. |
| Network metadata | Export only `network_type` + `roaming`; drop `carrier_name`. | Retain peer hostname/TLS endpoint metadata. | Store each schema diff in `readiness/schema_diffs/` and alert if Grafana’s “Network Context” widget shows carrier strings. |
| Override / chaos evidence | Emit `android.telemetry.redaction.override`/`android.telemetry.chaos.scenario` with masked actor roles. | Emit unmasked override approvals; no chaos-specific spans. | Cross-check `docs/source/sdk/android/readiness/and7_operator_enablement.md` after drills to ensure override tokens + chaos artefacts sit alongside the unmasked Rust events. |

Workflow:

1. After each manifest/exporter change, run
   `scripts/telemetry/run_schema_diff.sh --android-config <android.json> --rust-config <rust.json>` and place the JSON under `docs/source/sdk/android/readiness/schema_diffs/`.
2. Review the diff against the table above. If Android emits a Rust-only field
   (or vice versa), file an AND7 readiness bug and update both this plan and the
   runbook.
3. During weekly ops reviews, execute
   `scripts/telemetry/check_redaction_status.py --status-url https://android-telemetry-stg/api/redaction/status`
   and log the salt epoch plus schema-diff timestamp in the readiness worksheet.
4. Record any deviations in `docs/source/sdk/android/readiness/signal_inventory_worksheet.md`
   so governance packets capture parity decisions.

> **Schema reference:** canonical field identifiers originate from
> `android_telemetry_redaction.proto` (materialised during the Android SDK build
> alongside the Norito descriptors). The schema exposes the `authority_hash`,
> `alias_label`, `attestation_digest`, `device_brand_bucket`, and
> `actor_role_masked` fields now used across the SDK and telemetry exporters.

`authority_hash` is a fixed 32-byte digest of the Torii authority value recorded
in the proto. `attestation_digest` captures the canonical attestation statement
fingerprint, while `device_brand_bucket` maps the raw Android brand string onto
the approved enum (`generic`, `oem`, `enterprise`). `actor_role_masked` carries
the redaction override actor category (`support`, `sre`, `audit`) instead of the
raw user identifier.

### Crash Telemetry Export Alignment

Crash telemetry now shares the same OpenTelemetry exporters and provenance
pipeline as the Torii networking signals, closing the governance follow-up about
duplicate exporters. The crash handler feeds the `android.crash.report.capture`
event with a hashed `crash_id` (Blake2b-256 using the redaction salt already
tracked by `android.telemetry.redaction.salt_version`), process-state buckets,
and sanitized ANR watchdog metadata. Stack traces remain on-device and are only
summarised into the `has_native_trace` and `anr_watchdog_bucket` fields before
export so no PII or OEM strings leave the device.

Uploading a crash creates the `android.crash.report.upload` counter entry,
allowing SRE to audit backend reliability without learning anything about the
user or stack trace. Because both signals reuse the Torii exporter, they inherit
the same Sigstore signing, retention policy, and alerting hooks already defined
for AND7. Support runbooks can therefore correlate a hashed crash identifier
between Android and Rust evidence bundles without a bespoke crash pipeline.

Enable the handler via `ClientConfig.Builder.enableCrashTelemetryHandler()` once
telemetry options and sinks are configured; crash upload bridges can reuse
`ClientConfig.crashTelemetryReporter()` (or `CrashTelemetryHandler.recordUpload`)
to emit backend outcomes in the same signed pipeline.

## Policy Deltas vs Rust Baseline

Differences between Android and Rust telemetry policies with mitigation steps.

| Category | Rust Baseline | Android Policy | Mitigation / Validation |
|----------|---------------|----------------|-------------------------|
| Authority / peer identifiers | Plain authority strings | `authority_hash` (Blake2b-256, rotated salt) | Shared salt published via `iroha_config.telemetry.redaction_salt`; parity test ensures reversible mapping for support staff. |
| Host / network metadata | Node hostnames/IPs exported | Network type + roaming only | Network health dashboards updated to use availability categories instead of hostnames. |
| Device characteristics | N/A (server-side) | Bucketed profile (SDK 21/23/29+, tier `emulator`/`consumer`/`enterprise`) | Chaos rehearsals verify bucket mapping; support runbook documents escalation path when finer detail needed. |
| Redaction overrides | Not supported | Manual override token stored in Norito ledger (`actor_role_masked`, `reason`) | Overrides require signed request; audit log retained 1 year. |
| Attestation traces | Server attestation via SRE only | SDK emits sanitized attestation summary | Cross-check attestation hashes against Rust attestation validator; hashed alias prevents leakage. |

Validation checklist:

- Redaction unit tests for each signal verifying hashed/masked fields before
  exporter submission.
- Schema diff tool (shared with Rust nodes) run nightly to confirm field parity.
- Chaos rehearsal script exercises override workflow and confirms audit logging.

## Implementation Tasks (Pre-SRE Governance)

1. **Inventory Confirmation** — Cross-verify table above with actual Android SDK
   instrumentation hooks and Norito schema definitions. Owners: Android
   Observability TL, LLM.
2. **Telemetry Schema Diff** — Run the shared diff tool against Rust metrics to
   produce parity artefacts for the SRE review. Owner: SRE privacy lead.
3. **Runbook Draft (Completed 2026-02-03)** — `docs/source/android_runbook.md`
   now documents the end-to-end override workflow (Section 3) and the expanded
   escalation matrix plus role responsibilities (Section 3.1), tying the CLI
   helpers, incident evidence, and chaos scripts back to the governance policy.
   Owners: LLM with Docs/Support editing.
4. **Enablement Content** — Prepare briefing slides, lab instructions, and
   knowledge-check questions for the Feb 2026 session. Owners: Docs/Support
   Manager, SRE enablement team.

## Enablement Workflow & Runbook Hooks

### 1. Local + CI smoke coverage

- `scripts/android_sample_env.sh --telemetry --telemetry-duration=5m --telemetry-cluster=<host>` spins up the Torii sandbox, replays the canonical multi-source SoraFS fixture (delegating to `ci/check_sorafs_orchestrator_adoption.sh`), and seeds synthetic Android telemetry.
  - Traffic generation is handled by `scripts/telemetry/generate_android_load.py`, which records a request/response transcript under `artifacts/android/telemetry/load-generator.log` and honours headers, path overrides, or dry-run mode.
  - The helper copies SoraFS scoreboard/summaries into `${WORKDIR}/sorafs/` so AND7 rehearsals can prove multi-source parity before touching mobile clients.
- CI reuses the same tooling: `ci/check_android_dashboard_parity.sh` runs `scripts/telemetry/compare_dashboards.py` against `dashboards/grafana/android_telemetry_overview.json`, the Rust reference dashboard, and the allowance file at `dashboards/data/android_rust_dashboard_allowances.json`, emitting the signed diff snapshot `docs/source/sdk/android/readiness/dashboard_parity/android_vs_rust-latest.json`.
- Chaos rehearsals follow `docs/source/sdk/android/telemetry_chaos_checklist.md`; the sample-env script plus the dashboard parity check form the “ready” evidence bundle that feeds the AND7 burn-in audit.

### 2. Override issuance and audit trail

- `scripts/android_override_tool.py` is the canonical CLI for issuing and revoking redaction overrides. `apply` ingests a signed request, emits the manifest bundle (`telemetry_redaction_override.to` by default), and appends a hashed token row into `docs/source/sdk/android/telemetry_override_log.md`. `revoke` stamps the revocation timestamp against that same row, and `digest` writes the sanitised JSON snapshot required for governance.
- The CLI refuses to modify the audit log unless the Markdown table header is present, matching the compliance requirement captured in `docs/source/android_support_playbook.md`. Unit coverage in `scripts/tests/test_android_override_tool_cli.py` protects the table parser, manifest emitters, and error handling.
- Operators attach the generated manifest, updated log excerpt, **and** the digest JSON under `docs/source/sdk/android/readiness/override_logs/` whenever an override is exercised; the log retains 365 days of history per the governance decision in this plan.

### 3. Evidence capture & retention

- Every rehearsal or incident produces a structured bundle under `artifacts/android/telemetry/` containing:
  - The load-generator transcript and aggregate counters from `generate_android_load.py`.
  - Dashboard parity diff (`android_vs_rust-<stamp>.json`) and allowance hash emitted by `ci/check_android_dashboard_parity.sh`.
  - Override log delta (if an override was granted), the corresponding manifest, and the refreshed digest JSON.
- The SRE burn-in report references those artefacts plus the SoraFS scoreboard copied by `android_sample_env.sh`, giving the AND7 readiness review a deterministic chain from telemetry hashes → dashboards → override status.

## Cross-SDK Device Profile Alignment

Dashboards translate Android’s `hardware_tier` into the canonical
`mobile_profile_class` defined in
`docs/source/sdk/mobile_device_profile_alignment.md` so AND7 and IOS7 telemetry
compare the same cohorts:

- `lab` — emitted as `hardware_tier = emulator`, matching Swift’s
  `device_profile_bucket = simulator`.
- `consumer` — emitted as `hardware_tier = consumer` (with the SDK-major suffix)
  and grouped with Swift’s `iphone_small`/`iphone_large`/`ipad` buckets.
- `enterprise` — emitted as `hardware_tier = enterprise`, aligning with Swift’s
  `mac_catalyst` bucket and future managed/iOS desktop runtimes.

Any new tier must be added to the alignment document and schema diff artefacts
before dashboards consume it.

## Governance & Distribution

- **Pre-read package** — This document plus appendix artefacts (schema diff,
  runbook diff, readiness deck outline) will be distributed to the SRE governance
  mailing list no later than **2026-02-05**.
- **Feedback loop** — Comments collected during governance will feed into the
  `AND7` JIRA epic; blockers are surfaced in `status.md` and the Android weekly
  stand-up notes.
- **Publishing** — Once approved, the policy summary will be linked from
  `docs/source/android_support_playbook.md` and referenced by the shared
  telemetry FAQ in `docs/source/telemetry.md`.

## Audit & Compliance Notes

- Policy honours GDPR/CCPA requirements by removing mobile subscriber data
  before export; the hashed authority salt rotates quarterly and is stored in
  the shared secrets vault.
- Enablement artefacts and runbook updates are logged in the compliance registry.
- Quarterly reviews confirm that overrides remain closed-loop (no stale access).

## Governance Outcome (2026-02-12)

The SRE governance session on **2026-02-12** approved the Android redaction
policy without modifications. Key decisions (see
`docs/source/sdk/android/telemetry_redaction_minutes_20260212.md`):

- **Policy acceptance.** Hashed authority, device-profile bucketting, and the
  omission of carrier names were ratified. Salt rotation tracking via
  `android.telemetry.redaction.salt_version` becomes a quarterly audit item.
- **Validation plan.** Unit/integration coverage, nightly schema diff runs, and
  quarterly chaos rehearsals were endorsed. Action item: publish a dashboard
  parity report after each rehearsal.
- **Override governance.** Norito-recorded override tokens were approved with a
  365-day retention window. Support engineering will own the override log
  digest review during monthly operations syncs.

## Follow-up Status

1. **Device-profile alignment (due 2026-03-01).** ✅ Completed — the shared
   mapping in `docs/source/sdk/mobile_device_profile_alignment.md` defines how
   Android `hardware_tier` values map to the canonical `mobile_profile_class`
   consumed by the parity dashboards and schema diff tooling.

## Upcoming SRE Governance Brief (Q2 2026)

Roadmap item **AND7** requires that the next SRE governance session receives a
concise Android telemetry redaction pre-read. Use this section as the living
brief; keep it updated before every council meeting.

### Prep checklist

1. **Evidence bundle** — export the latest schema diff, dashboard screenshots,
   and override log digest (see matrix below) and place them under a dated
   folder (for example
   `docs/source/sdk/android/readiness/and7_sre_brief/2026-02-07/`) before
   sending the invite.
2. **Drill summary** — attach the most recent chaos rehearsal log plus the
   `android.telemetry.redaction.failure` metric snapshot; ensure Alertmanager
   annotations reference the same timestamp.
3. **Override audit** — confirm all active overrides are recorded in the Norito
   registry and summarised in the meeting deck. Include expiry dates and the
   corresponding incident IDs.
4. **Agenda note** — ping the SRE chair 48 hours ahead of the meeting with the
   brief link, highlighting any decisions required (new signals, retention
   changes, or override policy updates).

### Evidence matrix

| Artefact | Location | Owner | Notes |
|----------|----------|-------|-------|
| Schema diff vs Rust | `docs/source/sdk/android/readiness/schema_diffs/<latest>.json` | Telemetry tooling DRI | Must be generated <72 h before meeting. |
| Dashboard diff screenshots | `docs/source/sdk/android/readiness/dashboards/<date>/` | Observability TL | Include `sorafs.fetch.*`, `android.telemetry.*`, and Alertmanager snapshots. |
| Override digest | `docs/source/sdk/android/readiness/override_logs/<date>.json` | Support engineering | Run `scripts/android_override_tool.sh digest` (see README in that directory) against the latest `telemetry_override_log.md`; tokens remain hashed before sharing. |
| Chaos rehearsal log | `artifacts/android/telemetry/chaos/<date>/log.ndjson` | QA automation | Attach KPI summary (stall count, retry ratio, override usage). |

### Open questions for the council

- Do we need to shorten the override retention window from 365 days now that
  the digest is automated?
- Should `android.telemetry.device_profile` adopt the new shared
  `mobile_profile_class` labels in the next release, or wait for the Swift/JS
  SDKs to ship the same change?
- Is additional guidance required for regional data residency once Torii
  Norito-RPC events land on Android (NRPC-3 follow-up)?

### Telemetry Schema Diff Procedure

Run the schema diff tool at least once per release candidate (and whenever the Android
instrumentation changes) so the SRE council receives fresh parity artefacts alongside the
dashboard diff:

1. Export the Android and Rust telemetry schemas you want to compare. For CI the configs live
   under `configs/android_telemetry.json` and `configs/rust_telemetry.json`.
2. Execute `scripts/telemetry/run_schema_diff.sh --android-config configs/android_telemetry.json --rust-config configs/rust_telemetry.json --out docs/source/sdk/android/readiness/schema_diffs/<date>-android_vs_rust.json`.
   - Alternatively pass commits (`scripts/telemetry/run_schema_diff.sh android-main rust-main`) to
     pull the configs directly from git; the script pins the hashes inside the artefact.
3. Attach the generated JSON to the readiness bundle and link it from `status.md` +
   `docs/source/telemetry.md`. The diff highlights added/removed fields and retention deltas so
   auditors can confirm redaction parity without replaying the tool.
4. When the diff reveals allowable divergence (e.g., Android-only override signals), update the
   allowance file referenced by `ci/check_android_dashboard_parity.sh` and note the rationale in
   the schema-diff directory README.

> **Archive rules:** keep the five most recent diffs under
> `docs/source/sdk/android/readiness/schema_diffs/` and move older snapshots to
> `artifacts/android/telemetry/schema_diffs/` so governance reviewers always see the latest data.
