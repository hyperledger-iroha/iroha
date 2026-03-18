---
lang: dz
direction: ltr
source: docs/source/sdk/android/telemetry_redaction.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 92647c0a4dc38c28c9cec9a74c7885f8e2e819dc9892951a83ecd1b0a47301bf
source_last_modified: "2026-01-05T09:28:12.059217+00:00"
translation_last_reviewed: 2026-02-07
---

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

For incident bridges and the enablement workshop, pair this policy with the
[`telemetry_redaction_quick_reference.md`](telemetry_redaction_quick_reference.md)
card, which summarizes the mandatory checks/commands on a single page, and the
[`telemetry_redaction_faq.md`](telemetry_redaction_faq.md) backlog that captures
answers surfaced during trainings and incident reviews.

Objectives:

- Catalogue every Android-emitted signal that reaches shared observability
  backends (OpenTelemetry traces, Norito-encoded logs, metrics exports).
- Classify fields that differ from the Rust baseline and document redaction or
  retention controls.
- Outline enablement and testing work so support teams can respond
  deterministically to redaction-related alerts.

## Readiness Outline & Evidence (Roadmap AND7)

Roadmap item **AND7** ties the redaction policy to a repeatable readiness
program so SRE and operators can prove parity with the Rust baseline. Every
train must cover the four tracks below (recording artefacts alongside the chaos
labs described in the operations runbook):

| Track | Scope | Owners | Evidence pointers |
|-------|-------|--------|-------------------|
| Schema diff + inventory | Regenerate the Android vs Rust diff via `scripts/telemetry/run_schema_diff.sh`, refresh the owner worksheet, and attach the JSON produced under `docs/source/sdk/android/readiness/schema_diffs/YYYYMMDD.json`. | Android Observability TL · SRE privacy lead | `docs/source/sdk/android/readiness/schema_diffs/android_vs_rust-20251116.json`, `docs/source/sdk/android/readiness/schema_diffs/android_vs_rust_policy-20251116.json`, `docs/source/sdk/android/readiness/signal_inventory_worksheet.md` |
| Runbook + chaos rehearsal | Walk through Sections 2–9 of the [operations runbook](../../android_runbook.md), execute one scenario from `readiness/labs/telemetry_lab_01.md`, and archive screenshots/logs under `docs/source/sdk/android/readiness/labs/reports/<stamp>/`. | Docs/Support manager · Android Observability TL | `docs/source/sdk/android/readiness/labs/telemetry_lab_01.md`, `docs/source/sdk/android/readiness/labs/reports/` |
| Enablement session & quick reference | Deliver the 60‑minute briefing (policy, runbook drills, override workflow) paired with the quick-reference card and FAQ, then record attendance + quiz artefacts. | LLM (DRI) · Support engineering | `docs/source/sdk/android/readiness/and7_operator_enablement.md`, `docs/source/sdk/android/telemetry_redaction_quick_reference.md`, `docs/source/sdk/android/telemetry_redaction_faq.md`, `docs/source/sdk/android/readiness/forms/attendance/README.md`, `docs/source/sdk/android/readiness/forms/responses/` |
| Override & audit log hygiene | Exercise `scripts/android_override_tool.sh` (issue → revoke → digest), rotate the `android.telemetry.redaction.salt_version` evidence bundle, and capture the sanitised event log for governance. | SRE on-call · Docs/Support | `docs/source/sdk/android/telemetry_override_log.md`, `docs/source/sdk/android/readiness/override_logs/`, validator output from `scripts/telemetry/validate_override_events.py` or `scripts/telemetry/run_override_event_validation.sh` |

Each readiness cycle must post a short summary to `status.md` (Android section)
linking the archive folder plus any open follow-ups. The table keeps the SRE
governance gate deterministic and mirrors the action plan called out in
`roadmap.md` (§Priority 7).

## Privacy Allowlists (Rust vs. Android)

Android exporters intentionally emit a smaller field set than Rust nodes to
avoid leaking device metadata, Torii hostnames, or per-app routing hints. The
`tools/telemetry-schema-diff` utility now generates both a JSON diff and a
Markdown checklist (`docs/source/sdk/android/readiness/schema_diffs/*.md`) so
reviewers can confirm that every mobile-only removal is covered by policy.

| Channel / Signal | Rust Baseline | Android Emission | Notes |
|------------------|---------------|------------------|-------|
| `torii.http.request` span | Includes `authority`, `route`, `client_ip`, `tls_cipher`, `node_id` | Drops `authority`, `client_ip`, `node_id`; emits `authority_hash` instead | Hashing uses Blake2b-256 keyed with `android.telemetry.redaction.salt_version`; CI asserts hash rotation + absence of the raw hostnames. |
| `torii.connect.session` event | Persists Connect app id, session SID, WebSocket URL | Emits SID + Connect app slug only; replaces URL with `connect_endpoint_hash` | Prevents leaking staging endpoints yet keeps operators able to correlate Connect queues via the slug. |
| `keystore.attestation.result` gauge | Records device model, boot patch level, hardware IDs | Emits `hardware_class` enum + attestation digest; strips model + patch strings | Aligns with compliance guidance while still exposing revocation evidence through digest comparison. |
| `telemetry.override.audit` log | Stores operator user id, Torii host, CLI arguments | Emits anonymised operator alias + override id; CLI args truncated to `--flag` list | Supports auditability without revealing exact hostnames or operator email addresses. |

Per-roadmap AND7 requirements, the allowlists above are mirrored in
`docs/source/sdk/android/telemetry_redaction_quick_reference.md` and the runbook
(`docs/source/android_runbook.md` §4.3). The schema diff CI job fails whenever a
field reappears on Android without an explicit allowlist entry, and the
enablement pre-read links directly to the diff artefacts so reviewers can verify
policy coverage before each governance review.

## Signal Inventory (Draft)

Planned instrumentation grouped by channel. All field names follow the Android
SDK telemetry schema (`org.hyperledger.iroha.android.telemetry.*`). Optional
fields are marked with `?`.

| Signal ID | Channel | Key Fields | PII/PHI Classification | Redaction / Retention | Notes |
|-----------|---------|------------|------------------------|-----------------------|-------|
| `android.torii.http.request` | Trace span | `authority_hash`, `route`, `status_code`, `latency_ms` | Authority is public; route contains no secrets | Emit hashed authority (`blake2b_256`) before export; retain for 7 days | Mirrors Rust `torii.http.request`; hashing ensures mobile alias privacy. |
| `android.torii.http.retry` | Event | `route`, `retry_count`, `error_code`, `backoff_ms` | None | No redaction; retain 30 days | Used for deterministic retry audits; identical to Rust fields. |
| `android.pending_queue.depth` | Gauge metric | `queue_type`, `depth` | None | No redaction; retain 90 days | Matches Rust `pipeline.pending_queue_depth`. |
| `android.keystore.attestation.result` | ✅ Implemented | `alias_label`, `security_level`, `attestation_digest`, `device_brand_bucket` | Alias (derived), device metadata | Alias labels reuse the telemetry redaction salt, attestation digests hash the leaf cert (`SHA-256`), and brand buckets piggy-back on the device-profile provider. `KeystoreTelemetryEmitter` emits the event whenever `IrohaKeyManager.verifyAttestation(...)` succeeds; tests guard the hashing + digest wiring (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/telemetry/KeystoreTelemetryEmitter.java`,`java/iroha_android/src/main/java/org/hyperledger/iroha/android/IrohaKeyManager.java`,`java/iroha_android/src/test/java/org/hyperledger/iroha/android/crypto/keystore/attestation/IrohaKeyManagerTelemetryTests.java`). |
| `android.keystore.attestation.failure` | ✅ Implemented | `alias_label`, `failure_reason` | None after alias redaction | No redaction; retain 90 days | The same emitter records failures (`failure_reason` derives from the thrown `AttestationVerificationException`) so chaos drills and auditor spot checks can diff alias coverage deterministically. |
| `android.telemetry.redaction.override` | Event | `override_id`, `actor_role_masked`, `reason`, `expires_at` | Actor role qualifies as operational PII | Field exports masked role category; retain 365 days with audit log | Not present in Rust; operators must file overrides through support. |
| `android.telemetry.export.status` | Counter | `backend`, `status` | None | No redaction; retain 30 days | Parity with Rust exporter status counters. |
| `android.telemetry.redaction.failure` | Counter | `signal_id`, `reason` | None | No redaction; retain 30 days | Required to mirror Rust `streaming_privacy_redaction_fail_total`. |
| `android.telemetry.device_profile` | Gauge | `profile_id`, `sdk_level`, `hardware_tier` | Device metadata | Emit coarse buckets (SDK major, hardware tier); retain 30 days | Enables parity dashboards without exposing OEM specifics. |
| `android.telemetry.network_context` | Event | `network_type`, `roaming` | Carrier may be PII | Drop `carrier_name` entirely; retain other fields 7 days | `ClientConfig.networkContextProvider` supplies the sanitised snapshot so apps can emit network type + roaming without exposing subscriber data; parity dashboards treat the signal as the mobile analogue to Rust `peer_host`. |
| `android.telemetry.config.reload` | Event | `source`, `result`, `duration_ms` | None | No redaction; retain 30 days | Mirrors Rust config reload spans. |
| `android.telemetry.chaos.scenario` | ✅ Implemented | `scenario_id`, `outcome`, `duration_ms`, `device_profile` | Device profile is bucketed | `ChaosScenarioLogger` wraps the telemetry sink so chaos rehearsals (`scripts/telemetry` and lab tooling) can emit the scenario id/outcome/duration straight from Java/Kotlin helpers; device-profile buckets reuse `DeviceProfileProvider`. Tests under `java/iroha_android/src/test/java/org/hyperledger/iroha/android/telemetry/ChaosScenarioLoggerTests.java` keep the output map stable. |
| `android.telemetry.redaction.salt_version` | Gauge | `salt_epoch`, `rotation_id` | None | No redaction; retain 365 days | Tracks Blake2b salt rotation; parity alert when Android hash epoch diverges from Rust nodes. |
| `android.crash.report.capture` | Event | `crash_id`, `signal`, `process_state`, `has_native_trace`, `anr_watchdog_bucket` | Crash fingerprint + process metadata | Hash `crash_id` with the shared redaction salt, bucket watchdog state, drop stack frames before export; retain 30 days | Feeds parity dashboards without exposing device-identifying traces; crashes stay correlated via hashed id. |
| `android.crash.report.upload` | Counter | `crash_id`, `backend`, `status`, `retry_count` | Crash fingerprint | Reuse hashed `crash_id`, emit status only; retain 30 days | Shares the Torii exporter pipeline so the crash bridge inherits Sigstore/OLTP guarantees required by **NRPC/AND7**. |

### Implementation Hooks

- `ClientConfig` now threads manifest-derived telemetry data via
  `setTelemetryOptions(...)`/`setTelemetrySink(...)`, automatically registering
  `TelemetryObserver` so hashed authorities and salt metrics flow without bespoke observers.
  See `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java`
  and the companion classes under
  `java/iroha_android/src/main/java/org/hyperledger/iroha/android/telemetry/`.
- `KeystoreTelemetryEmitter` lets apps/projects attach a telemetry sink to
  `IrohaKeyManager` so attestation successes/failures emit the
  `android.keystore.attestation.*` signals automatically. The emitter handles alias hashing,
  digest computation, and brand bucketing (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/telemetry/KeystoreTelemetryEmitter.java`).
- `ChaosScenarioLogger` ships a helper for the chaos harness / lab tooling so each rehearsal logs
  the mandated scenario/outcome/duration tuple alongside the current device profile bucket
  (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/telemetry/ChaosScenarioLogger.java`).
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

### Rust Correlation Map

| Android signal | Rust equivalent | Alignment goal | Evidence hook |
|----------------|-----------------|----------------|---------------|
| `android.torii.http.request` | `torii.http.request` span/metric emitted by Rust services | Keep routing diagnostics identical while hashing the mobile authority; proves retries/backoff alerts trigger off the same payloads on every platform. | `java/iroha_android/src/main/java/org/hyperledger/iroha/android/telemetry/TelemetryObserver.java` (hashing) · Rust schema reference in `docs/source/telemetry.md` and the diff artefacts under `docs/source/sdk/android/readiness/schema_diffs/`. |
| `android.pending_queue.depth` | `pipeline.pending_queue_depth` metric | Ensure mobile queue exports share the same label set so backlog dashboards and chaos drills render a unified view. | Android metric wiring in `java/iroha_android/src/main/java/org/hyperledger/iroha/android/telemetry/PendingQueueGauge.java`; Rust counterpart covered by `telemetry_schema_diff.md` §3.2 and the node tests linked from `docs/source/telemetry.md`. |
| `android.telemetry.network_context` | Rust peer/connection metadata (`torii.peer.status`, `iroha_network_context` tags) | Replace carrier names with the hashed/bucketed network tuple while still feeding the mixed Torii/SRE dashboards used for AND4/NRPC rollout. | Android provider lives in `android/client/network/AndroidNetworkContextProvider.java`; Rust baseline described in `docs/source/telemetry.md#network-context`. Gaps are flagged by `scripts/telemetry/run_schema_diff.sh`. |
| `android.telemetry.redaction.override` | No direct Rust signal (Rust governance logs overrides via manifests/on-ledger events) | Capture every mobile break-glass action and keep the actor mask in sync with the override digest + Torii audit events. | CLI + NDJSON logging in `scripts/android_override_tool.sh` and the logbook `docs/source/sdk/android/telemetry_override_log.md`; governance compares the NDJSON stream with Rust manifests during AND7 reviews. |
| `android.telemetry.chaos.scenario` | None (Rust chaos suites log results in CI artefacts only) | Encode rehearsal identifiers/outcomes so chaos labs, Grafana annotations, and AND7 readiness packets reference the same ids. | `java/iroha_android/src/main/java/org/hyperledger/iroha/android/telemetry/ChaosScenarioLogger.java` emits the event; lab outputs stored under `docs/source/sdk/android/readiness/labs/`. |
| `android.crash.report.capture` | None; Rust nodes emit OS crash notes, not structured spans | Hash crash identifiers and bucket watchdog metadata so Android-only exporters can feed parity dashboards without leaking stack traces. | Exporter in `java/iroha_android/src/main/java/org/hyperledger/iroha/android/telemetry/CrashTelemetryEmitter.java`; parity verified via the crash section of `scripts/telemetry/run_schema_diff.sh` and the Grafana “Crash Telemetry” board cited in the quick reference. |

## Parity allowlists & schema-diff workflow

Governance requires a dual allowlist so Android exports never leak identifiers
that Rust services intentionally surface. This section mirrors the runbook entry
(`docs/source/android_runbook.md` §2.3) but keeps the AND7 redaction plan
self-contained.

| Category | Android exporters | Rust services | Validation hook |
|----------|-------------------|---------------|-----------------|
| Authority / route context | Hash `authority`/`alias` via Blake2b-256 and drop raw Torii hostnames before export; emit `android.telemetry.redaction.salt_version` to prove salt rotation. | Emit full Torii hostnames and peer IDs for correlation. | Compare `android.torii.http.request` vs `torii.http.request` entries in the latest schema diff under `docs/source/sdk/android/readiness/schema_diffs/`, then run `scripts/telemetry/check_redaction_status.py` to confirm salt epochs. |
| Device & signer identity | Bucket `hardware_tier`/`device_profile`, hash controller aliases, and never export serial numbers. | Emit validator `peer_id`, controller `public_key`, and queue hashes verbatim. | Align with `docs/source/sdk/mobile_device_profile_alignment.md`, exercise alias hashing tests inside `ci/run_android_tests.sh`, and archive queue-inspector outputs during labs. |
| Network metadata | Export only `network_type` + `roaming`; drop `carrier_name`. | Retain peer hostname/TLS endpoint metadata. | Store each schema diff in `readiness/schema_diffs/` and alert if Grafana’s “Network Context” widget shows carrier strings. |
| Override / chaos evidence | Emit `android.telemetry.redaction.override`/`android.telemetry.chaos.scenario` with masked actor roles. | Emit unmasked override approvals; no chaos-specific spans. | Cross-check `docs/source/sdk/android/readiness/and7_operator_enablement.md` after drills to ensure override tokens + chaos artefacts sit alongside the unmasked Rust events. |

Workflow:

1. After each manifest/exporter change, run
   `scripts/telemetry/run_schema_diff.sh --android-config <android.json> --rust-config <rust.json>` and place the JSON under `docs/source/sdk/android/readiness/schema_diffs/`.
2. Review the diff against the table above. If Android emits a Rust-only field
   (or vice versa), file an AND7 readiness bug and update both this plan and the
   runbook.
3. During weekly ops reviews, execute
   ```bash
   scripts/telemetry/check_redaction_status.py \
     --status-url https://android-telemetry-stg/api/redaction/status \
     --expected-salt-epoch "${ANDROID_TELEMETRY_EXPECTED_SALT_EPOCH}" \
     --expected-salt-rotation "${ANDROID_TELEMETRY_EXPECTED_SALT_ROTATION}"
   ```
   (or set the `ANDROID_TELEMETRY_EXPECTED_SALT_EPOCH/ROTATION` env vars). The helper
   now exits with code 2 when the observed salt epoch/rotation diverges, so the
   readiness worksheet always records whether Android and Rust hash epochs match.
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

Enable the handler by calling
`ClientConfig.Builder.enableCrashTelemetryHandler()` after telemetry is
configured; upload bridges can reuse `ClientConfig.crashTelemetryReporter()` (or
`CrashTelemetryHandler.recordUpload`) so every backend response registers in the
shared evidence timeline.

## Policy Deltas vs Rust Baseline

Differences between Android and Rust telemetry policies with mitigation steps.

| Category | Rust Baseline | Android Policy | Mitigation / Validation |
|----------|---------------|----------------|-------------------------|
| Authority / peer identifiers | Plain authority strings | `authority_hash` (Blake2b-256, rotated salt) | Shared salt published via `iroha_config.telemetry.redaction_salt`; parity test ensures reversible mapping for support staff. |
| Host / network metadata | Node hostnames/IPs exported | Network type + roaming only | Network health dashboards updated to use availability categories instead of hostnames. |
| Device characteristics | N/A (server-side) | Bucketed profile (SDK 21/23/29+, tier `emulator`/`consumer`/`enterprise`) | Chaos rehearsals verify bucket mapping; support runbook documents escalation path when finer detail needed. |
| Redaction overrides | Not supported | Manual override token stored in Norito ledger (`actor_role_masked`, `reason`) | Overrides require signed request; audit log retained 1 year. |
| Attestation traces | Server attestation via SRE only | SDK emits sanitized attestation summary | Cross-check attestation hashes against Rust attestation validator; hashed alias prevents leakage. |
| Crash telemetry exports | Rust crash capture remains on server with raw stack trace | Android emits summarised crash spans + counters with salted hashes | Shared exporter path enforces same signing/retention; hashed crash ids are replay-tested against Rust evidence bundles before publishing. |
| Chaos rehearsal evidence | Rust nodes tag scenario id + hostname | Android emits `android.telemetry.chaos.scenario` with bucketed profile only | Chaos lab scripts replay Android and Rust traces together; schema diff detects attempts to add host/device identifiers. |

Validation checklist:

- Redaction unit tests for each signal verifying hashed/masked fields before
  exporter submission.
- Schema diff tool (shared with Rust nodes) run nightly to confirm field parity.
- Chaos rehearsal script exercises override workflow and confirms audit logging.

### Allowlist Alignment Workflow

Android and Rust exporters now share the `tools/telemetry-schema-diff` utility,
which consumes the `configs/android_telemetry.json` manifest and the Rust
schema registry. CI invokes `cargo run -p telemetry-schema-diff -- \
    --android-config configs/android_telemetry.json \
    --rust-config configs/rust_telemetry.json` for direct runs, or the wrapper
`scripts/telemetry/run_schema_diff.sh --android-config configs/android_telemetry.json \
--rust-config configs/rust_telemetry.json --out artifacts/android_vs_rust.json \
--policy-out artifacts/android_rust_policy_diff.json --textfile-dir /var/lib/node_exporter/textfile_collector`
when governance needs the policy summary alongside the full diff and the
node_exporter metrics to prove the gate keeps running. The helper writes
`artifacts/android/telemetry/schema_diff.prom` automatically; overriding
`--metrics-out` remains supported for bespoke workflows. The resulting JSON
artefact exposes `policy_violations` entries which gate merges when Android
diverges from the Rust allowlist. Operator runbooks
(`docs/source/android_runbook.md`) now link to the latest diff output so SRE can
confirm which fields differ before responding to an incident.

## Implementation Tasks (Pre-SRE Governance)

1. **Inventory Confirmation (Completed 2025-11-11)** — Cross-verified the signal
   inventory in this plan against the current Android SDK sources. Findings are
   captured in the verification snapshot below so owners can close the gaps that
   remain before the AND7 governance review. Owners: Android Observability TL,
   LLM.
2. **Telemetry Schema Diff (Completed 2025-11-16)** — Regenerated the Android vs
   Rust schema diff with the config-mode helper and captured both the full JSON
   artefact and policy summary (`android_vs_rust-20251116.json`,
   `android_vs_rust_policy-20251116.json`) for the AND7 evidence bundle. Owner:
   SRE privacy lead.
3. **Runbook Draft (Completed 2026-02-03)** — `docs/source/android_runbook.md`
   now documents the end-to-end override workflow (Section 3) and the expanded
   escalation matrix plus role responsibilities (Section 3.1), tying the CLI
   helpers, incident evidence, and chaos scripts back to the governance policy.
   Owners: LLM with Docs/Support editing.
4. **Enablement Content** — Prepare briefing slides, lab instructions, and
   knowledge-check questions for the Feb 2026 session. Owners: Docs/Support
   Manager, SRE enablement team. The module structure and assessment plan live
   in the Readiness Outline section below; facilitators must use
   `docs/source/sdk/android/readiness/forms/telemetry_quiz_2026-02.md`
   (localized variants under the same directory), collect responses in
   `docs/source/sdk/android/readiness/forms/responses/<stamp>.csv`, and attach
   both the raw CSV and the scored summary to the governance packet alongside
   the attendance log.

## Rust ↔ Android Policy Delta Matrix

Roadmap item **AND7** requires the redaction policy to call out every place
where the Android SDK intentionally diverges from the Rust node baseline. The
table below is the canonical mapping that presenters use during the governance
brief (`telemetry_readiness_outline.md`) and when running the schema diff helper
(`scripts/telemetry/run_schema_diff.sh`). Refresh it whenever new signals or
redaction knobs ship, and archive the resulting diff artefacts under
`docs/source/sdk/android/readiness/schema_diffs/`.

| Dimension | Rust baseline | Android policy | Validation / Evidence |
|-----------|---------------|----------------|-----------------------|
| Authority & Torii routes | Nodes emit plain Torii authority strings in spans/metrics. | `TelemetryObserver` hashes every authority (Blake2b-256 + quarterly salt) before exporting spans such as `android.torii.http.request`, while keeping the rest of the payload identical to Rust. | Hashing/allowlist enforcement sits in `java/iroha_android/src/main/java/org/hyperledger/iroha/android/telemetry/TelemetryObserver.java` and is compared against the node schema via `tools/telemetry-schema-diff/src/main.rs`; diff snapshots live in `docs/source/sdk/android/readiness/schema_diffs/README.md`. |
| Device metadata | Rust services tag telemetry with exact model identifiers and OS build IDs. | Mobile exporters collapse devices into coarse buckets (`simulator`, `consumer`, `enterprise`, `mac_catalyst`) plus SDK major version so operators never collect OEM-identifying data. | Bucket definitions and the parity mapping with Rust live in `docs/source/sdk/mobile_device_profile_alignment.md`; automation checks the mapping during chaos labs (`docs/source/sdk/android/readiness/labs/swift_telemetry_lab_01.md`). |
| Network context | Nodes export peer hostnames and anonymised carrier hashes. | Android removes carrier names entirely and only exports `network_type` + `roaming` flags supplied by `ClientConfig.networkContextProvider`, mirroring `android.telemetry.network_context`. | Implementation hooks live in `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java`, and the enablement kit (`docs/source/sdk/android/readiness/and7_operator_enablement.md`) calls out the trimmed fields so operators rely on parity dashboards instead of carrier logs. |
| Override governance | Rust nodes do not implement per-tenant redaction overrides. | Android records overrides as Norito payloads (`android.telemetry.redaction.override`) with masked actor roles, signed tokens, and a 365-day retention policy. | Override creation + logging is automated by `scripts/android_override_tool.sh`; digest outputs are archived in `docs/source/sdk/android/readiness/override_logs/README.md` and referenced by Section 3 of `docs/source/android_runbook.md`. The override-event validator enforces hashed ids/roles/timestamps and can auto-source inputs from AND7 bundle manifests when assets carry the `override-events` and `override-ledger` labels. |
| Exporter health & telemetry counters | Rust exporters expose `telemetry.export.status` and `streaming_privacy_redaction_fail_total`. | Android emits the same counters plus `android.telemetry.device_profile`, `android.telemetry.redaction.salt_version`, and chaos-only probes so ops can monitor mobile-specific risk. | Emission hooks live in `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/HttpClientTransport.java` and `java/iroha_android/src/main/java/org/hyperledger/iroha/android/telemetry/TelemetryObserver.java`; `scripts/telemetry/check_redaction_status.py` verifies the counters and the dashboards consuming them are documented in `docs/source/sdk/android/readiness/dashboard_parity/README.md`. |

Whenever schema diffs highlight new deltas, update this table and rerun
`scripts/telemetry/run_schema_diff.sh --android-config configs/android_telemetry.json --rust-config configs/rust_telemetry.json`
so the evidence bundle reflects the change. The governance brief directory
(`docs/source/sdk/android/readiness/and7_sre_brief/2026-02-07/` for the initial
Feb 2026 session, with future briefs following the same pattern)
expects these deltas to be linked alongside the latest `schema_diffs/*.json`
artefacts.

## Readiness & Enablement Outline

The AND7 governance review requires a documented enablement path so operators,
support, and auditors can consume the new telemetry policy without bespoke
briefings. The programme mirrors the roadmap’s enablement charter and is now
anchored in this document for cross-team visibility.

### Session Modules

| Module | Duration | Focus | Outputs |
|--------|----------|-------|---------|
| 0 — Policy Overview | 15 min | Review legal/compliance guardrails, compare Android vs Rust telemetry, highlight allowlist diffs from `tools/telemetry-schema-diff`. | Slides + narration stored under `docs/source/sdk/android/readiness/` and linked from `telemetry_readiness_outline.md`. |
| 1 — Operational Runbook | 20 min | Walk through alert triage, dashboards, override workflow, and `android_runbook.md` escalation matrix. | Screen-capture demo + checklist appended to the quick-reference card. |
| 2 — Support Workflow | 15 min | Cover messaging templates, audit logging expectations, and Norito evidence capture for overrides. | Template pack recorded in `telemetry_redaction_faq.md` and the readiness folder. |
| 3 — Chaos Lab | 10 min hands-on | Execute staging chaos scenario, verify `android.telemetry.redaction.*` metrics, and inspect hashed payloads. | Lab steps reuse `telemetry_chaos_checklist.md` with additional annotations captured alongside the recording. |

### Artefacts & Success Metrics

- **Quick-reference card** — `docs/source/sdk/android/telemetry_redaction_quick_reference.md`
  condenses CLI commands, metric thresholds, and evidence checklists; versioned
  alongside this plan.
- **FAQ backlog** — `docs/source/sdk/android/telemetry_redaction_faq.md` records
  questions from rehearsals, linking directly to runbook sections for fast
  follow-up.
- **Knowledge check** — 10-question assessment hosted in the enablement portal
  (`readiness/forms/telemetry_quiz_2026-02*.md`); facilitators export the graded
  CSV into `readiness/forms/responses/` and log aggregate scores in `status.md`.
  Completion ≥90% is required for sign-off; remediation tasks ride the same
  tracker as schema-diff and lab follow-ups.
- **Attendance + recording** — CI uploads session recordings and attendance
  manifests under `artifacts/android/telemetry_enablement/`, satisfying the
  roadmap’s evidence requirement and keeping parity with the Rust exporter
  enablement workflow.

### Timeline & Ownership

- **Pre-read circulation (T-7 days)** — Owners: LLM, Android Observability TL.
- **Live workshop + lab (target Feb 2026 SRE governance week)** — Owners:
  Docs/Support Manager and SRE enablement team.
- **Post-session remediation (T+5 days)** — Outstanding questions folded into
  the FAQ, and telemetry schema diffs are re-run to confirm no regressions after
  lab exercises.

The readiness checklist above allows the action plan referenced in the roadmap
to close once the session artefacts, recordings, and schema diffs are attached
to the governance evidence bundle.

## Inventory Verification Snapshot (2026-02-18)

We compared the canonical signal list in `configs/android_telemetry.json` with
the Android SDK sources under `java/iroha_android/src/main/java`. AND7
instrumentation now covers every networking, telemetry, keystore, and chaos
counter, so the table below records concrete evidence for each signal.

| Signal | Status | Evidence / Notes |
|--------|--------|------------------|
| `android.torii.http.request` | ✅ Implemented | `ClientConfig` wires `TelemetryObserver` whenever telemetry options + sink are provided, and the observer now tracks per-request spans—hashing authorities, recording route/method/salt, and attaching latency + HTTP status/error metadata before invoking the sink (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java:33-52`,`java/iroha_android/src/main/java/org/hyperledger/iroha/android/telemetry/TelemetryObserver.java:34-92`). `TelemetryObserverTests` exercise both the success and failure paths so latency/status/error fields stay covered in CI (`java/iroha_android/src/test/java/org/hyperledger/iroha/android/telemetry/TelemetryObserverTests.java:40-138`). |
| `android.torii.http.retry` | ✅ Implemented | `HttpClientTransport.scheduleRetry` now calls `emitRetryTelemetry` so every retry publishes hashed authority, route, retry count, failure code, and backoff delay, and the new test ensures the signal matches the schema (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/HttpClientTransport.java:333`,`java/iroha_android/src/test/java/org/hyperledger/iroha/android/client/HttpClientTransportTests.java:334`). |
| `android.pending_queue.depth` | ✅ Implemented | `HttpClientTransport` now samples the configured queue after every drain/enqueue, emits the `android.pending_queue.depth` gauge via the telemetry sink, and tags events with the queue identifier returned by `PendingTransactionQueue.telemetryQueueName()` (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/HttpClientTransport.java:178`,`java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/queue/PendingTransactionQueue.java:48`). |
| `android.keystore.attestation.result` | ✅ Implemented | `ClientConfig` now wraps export-option key managers with `KeystoreTelemetryEmitter` so `IrohaKeyManager.verifyAttestation` emits `android.keystore.attestation.result`, and `ClientConfigKeystoreTelemetryTests` exercises the flow end-to-end.【java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java:1】【java/iroha_android/src/test/java/org/hyperledger/iroha/android/client/ClientConfigKeystoreTelemetryTests.java:1】 |
| `android.keystore.attestation.failure` | ✅ Implemented | The same telemetry wiring records `android.keystore.attestation.failure` when the verifier raises `AttestationVerificationException`, with regression coverage in the keystore telemetry tests.【java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java:1】【java/iroha_android/src/test/java/org/hyperledger/iroha/android/client/ClientConfigKeystoreTelemetryTests.java:1】 |
| `android.telemetry.redaction.override` | ✅ Implemented (CLI) | `scripts/android_override_tool.py --event-log docs/source/sdk/android/readiness/override_logs/override_events.ndjson --actor-role <bucket>` now appends NDJSON records whenever overrides are issued or revoked, capturing `override_id` (hashed token), `ticket_id`, `reason`, `actor_role_masked`, and timestamps. Tests under `scripts/tests/test_android_override_tool_cli.py` cover apply/revoke event logging so the telemetry pipeline can ingest the feed alongside the Markdown audit log. |
| `android.telemetry.export.status` | ✅ Implemented | `ClientConfig#setTelemetrySink` now wraps every sink with `TelemetryExportStatusSink`, which emits `status="ok"`/`status="error"` events for each delegate invocation. Use `ClientConfig.Builder.setTelemetryExporterName("otel"|"textfile"|…)` to label the backend; tests in `java/iroha_android/src/test/java/org/hyperledger/iroha/android/telemetry/TelemetryExportStatusSinkTests.java` exercise both success and failure paths so dashboards can rely on the counter.【java/iroha_android/src/main/java/org/hyperledger/iroha/android/telemetry/TelemetryExportStatusSink.java:1】【java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java:33】 |
| `android.telemetry.redaction.failure` | ✅ Implemented | `TelemetryObserver` now emits `android.telemetry.redaction.failure` whenever hashing skips a payload, tagging the originating signal id + reason, and `TelemetryObserverTests.emitsRedactionFailureEvent` exercises the blank-authority path so Alertmanager plumbing has evidence.【java/iroha_android/src/main/java/org/hyperledger/iroha/android/telemetry/TelemetryObserver.java:1】【java/iroha_android/src/test/java/org/hyperledger/iroha/android/telemetry/TelemetryObserverTests.java:1】 |
| `android.telemetry.device_profile` | ✅ Implemented | `DeviceProfileProvider` snapshots feed the new `HttpClientTransport.emitDeviceProfileTelemetry` helper, which emits the bucket once per process via the configured `TelemetrySink`. Builders can supply platform-specific providers (and `ClientConfig.Builder.enableAndroidDeviceProfileProvider()` reflects into `android.os.Build` via `AndroidDeviceProfileProvider`), while tests (`HttpClientTransportTests.submitEmitsDeviceProfileTelemetry`) guard the emission path.【java/iroha_android/src/main/java/org/hyperledger/iroha/android/telemetry/AndroidDeviceProfileProvider.java:1】【java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/HttpClientTransport.java:404】 |
| `android.telemetry.network_context` | ✅ Implemented | `ClientConfig.Builder.setNetworkContextProvider(...)` registers a sanitised provider, `HttpClientTransport.notifyRequest` emits the signal via `TelemetrySink.emitSignal`, and `HttpClientTransportTests.submitEmitsNetworkContextTelemetry` exercises the path under a fake provider.【java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java:1】【java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/HttpClientTransport.java:1】【java/iroha_android/src/test/java/org/hyperledger/iroha/android/client/HttpClientTransportTests.java:1】 |
| `android.telemetry.config.reload` | ✅ Implemented | `ConfigWatcher` rebuilds `ClientConfig` instances when manifests change and emits `android.telemetry.config.reload` via the active telemetry sink, tagging every event with the manifest `source`, SHA-256 `digest`, elapsed `duration_ms`, and optional `error` metadata so incident responders can trace reload attempts. `ConfigWatcherTests` now assert the success/retry/failure payloads, covering the digest/source/duration fields alongside the failure error-class evidence that SRE governance requested.【java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ConfigWatcher.java:1】【java/iroha_android/src/test/java/org/hyperledger/iroha/android/client/ConfigWatcherTests.java:1】 |
| `android.telemetry.chaos.scenario` | ✅ Implemented | `ChaosScenarioLogger.from(telemetryOptions, sink, deviceProfileProvider)` lets chaos harnesses emit scenario id/outcome/duration directly through the configured `TelemetrySink`, and it reuses the `DeviceProfileProvider` buckets so dashboards can compare Android vs Rust chaos drills. Tests under `java/iroha_android/src/test/java/org/hyperledger/iroha/android/telemetry/ChaosScenarioLoggerTests.java` keep the payload map stable, and lab tooling captures the emitted NDJSON alongside `scripts/telemetry` artefacts. |
| `android.telemetry.redaction.salt_version` | ✅ Implemented | `TelemetryObserver` emits the salt-version gauge the first time telemetry fires, exporting `salt_epoch`/`rotation_id` so dashboards and `scripts/telemetry/check_redaction_status.py` can track rotations; `TelemetryObserverTests.emitsHashedAuthorityRecords` verifies the gauge payload.【java/iroha_android/src/main/java/org/hyperledger/iroha/android/telemetry/TelemetryObserver.java:1】【java/iroha_android/src/test/java/org/hyperledger/iroha/android/telemetry/TelemetryObserverTests.java:1】 |

Rotation identifiers now come directly from the manifest’s
`telemetry.redaction.rotation_id` knob via `TelemetryOptions.Redaction::setRotationId`, so
`rotation_id` can diverge from the human-readable salt epoch without code changes.【java/iroha_android/src/main/java/org/hyperledger/iroha/android/telemetry/TelemetryOptions.java:90】
| `android.crash.report.capture` | ✅ Implemented | `ClientConfig.Builder.enableCrashTelemetryHandler()` installs `CrashTelemetryHandler`, which hooks `Thread.setDefaultUncaughtExceptionHandler` and emits hashed crash IDs, watchdog buckets, and native-trace flags through `CrashTelemetryReporter` using the same redaction policy as HTTP telemetry. Tests cover handler metadata, delegation, and hashing semantics.【java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java:1】【java/iroha_android/src/main/java/org/hyperledger/iroha/android/telemetry/CrashTelemetryHandler.java:1】【java/iroha_android/src/test/java/org/hyperledger/iroha/android/telemetry/CrashTelemetryHandlerTests.java:1】 |
| `android.crash.report.upload` | ✅ Implemented | `ClientConfig.crashTelemetryReporter()` exposes a preconfigured reporter so crash upload bridges can log backend/status/retry outcomes with hashed crash IDs. The new handler also exposes `recordUpload` for integrators that keep a reference to the installation handle.【java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java:218】【java/iroha_android/src/main/java/org/hyperledger/iroha/android/telemetry/CrashTelemetryHandler.java:61】【java/iroha_android/src/test/java/org/hyperledger/iroha/android/telemetry/CrashTelemetryHandlerTests.java:1】 |

With the reload signal wired, the schema diff now shows zero Android-only gaps
beyond the intentional override counters, so the remaining AND7 work in this
document focuses on enablement artefacts (chaos rehearsals, override workflow,
dashboard parity) rather than instrumentation gaps.

## Enablement Workflow & Runbook Hooks

### 1. Local + CI smoke coverage

- `scripts/android_sample_env.sh --telemetry --telemetry-duration=5m --telemetry-cluster=<host>` spins up the Torii sandbox, replays the canonical multi-source SoraFS fixture (delegating to `ci/check_sorafs_orchestrator_adoption.sh`), and seeds synthetic Android telemetry.
  - Traffic generation is handled by `scripts/telemetry/generate_android_load.py`, which records a request/response transcript under `artifacts/android/telemetry/load-generator.log` and honours headers, path overrides, or dry-run mode.
  - The helper copies SoraFS scoreboard/summaries into `${WORKDIR}/sorafs/` so AND7 rehearsals can prove multi-source parity before touching mobile clients.
- CI reuses the same tooling: `ci/check_android_dashboard_parity.sh` runs `scripts/telemetry/compare_dashboards.py` against `dashboards/grafana/android_telemetry_overview.json`, the Rust reference dashboard, and the default allowance file at `dashboards/data/android_rust_dashboard_allowances.json`. The script now accepts `--android`, `--rust`, `--allowance`, and `--artifact` overrides so evidence bundles can rely on ad-hoc dashboards/allowlists (for example, `docs/source/sdk/android/readiness/dashboard_parity/allowance.json`). The signed diff snapshot continues to live under `docs/source/sdk/android/readiness/dashboard_parity/android_vs_rust-latest.json`.
- Chaos rehearsals follow `docs/source/sdk/android/telemetry_chaos_checklist.md`; the sample-env script plus the dashboard parity check form the “ready” evidence bundle that feeds the AND7 burn-in audit.

### 2. Override issuance and audit trail

- `scripts/android_override_tool.py` is the canonical CLI for issuing and revoking redaction overrides. `apply` ingests a signed request, emits the manifest bundle (`telemetry_redaction_override.to` by default), and appends a hashed token row into `docs/source/sdk/android/telemetry_override_log.md`. `revoke` stamps the revocation timestamp against that same row.
- Pass `--event-log docs/source/sdk/android/readiness/override_logs/override_events.ndjson` (or another NDJSON path) plus `--actor-role <support|sre|docs|compliance|program|other>` so every issuance/revocation also emits the `android.telemetry.redaction.override` event with masked role metadata; the resulting feed is what dashboards consume. See `scripts/tests/test_android_override_tool_cli.py` for coverage.
- The CLI refuses to modify the audit log unless the Markdown table header is present, matching the compliance requirement captured in `docs/source/android_support_playbook.md`. Unit coverage in `scripts/tests/test_android_override_tool_cli.py` protects the table parser, manifest emitters, and error handling.
- `scripts/android_override_tool.sh digest` exports the sanitised JSON snapshot required for the AND7 evidence bundle (`docs/source/sdk/android/readiness/override_logs/`; see the README in that directory for publishing guidance).
- Operators attach the generated manifest plus the updated log excerpt whenever an override is exercised; the log retains 365 days of history per the governance decision in this plan.

### 3. Evidence capture & retention

- Every rehearsal or incident produces a structured bundle under `artifacts/android/telemetry/` containing:
  - The load-generator transcript and aggregate counters from `generate_android_load.py`.
  - Dashboard parity diff (`android_vs_rust-<stamp>.json`) and allowance hash emitted by `ci/check_android_dashboard_parity.sh`.
  - Override log delta (if an override was granted) and the corresponding manifest.
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

## Training & Curriculum Deliverables

The AND7 operator curriculum is driven directly from this policy. Use the artefacts below to
keep the enablement session, runbooks, and evidence bundles in lockstep:

| Module | How this policy is applied | Where it is presented | Evidence |
|--------|---------------------------|-----------------------|----------|
| Policy deep dive | Highlight the signal table, hashing rules, and override governance described here. | “Policy deep dive” slot in the readiness agenda (`docs/source/sdk/android/telemetry_readiness_outline.md`). | Schema diff JSON under `docs/source/sdk/android/readiness/schema_diffs/`, snippet from §2 of `android_runbook.md`. |
| Runbook + override workflow | Demonstrate how redaction allowlists drive override issuance, audit logging, and rollback expectations. | “Runbook walkthrough” segment and Section 3 of the enablement kit (`docs/source/sdk/android/readiness/and7_operator_enablement.md`). | Latest `telemetry_override_log.md`, manifest emitted by `scripts/android_override_tool.sh digest`. |
| Dashboards, labs, and knowledge check | Show the dashboards that visualise the redaction policy (device profile buckets, override counters) and replay the chaos lab scenarios that prove the guardrails. | “Dashboards & alerts”, “Chaos rehearsal lab”, and “Knowledge check” segments listed in both the kit and outline. | Dashboard exports under `readiness/screenshots/<stamp>/`, lab reports `readiness/labs/reports/<YYYY-MM>/`, quiz CSV in `readiness/forms/responses/`. |

Whenever this document changes, update the kit (§3.1) and outline agenda so presenters are
briefing operators on the same canonical deltas. Archive the refreshed materials in
`docs/source/sdk/android/readiness/archive/<stamp>/` and note the completion inside
`status.md` (AND7 entry) to keep the roadmap acceptance criteria satisfied.

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
   folder (e.g., `docs/source/sdk/android/readiness/and7_sre_brief/2026-02-07/`)
   before circulating the invite.
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
| Override digest | `docs/source/sdk/android/readiness/override_logs/<date>.json` | Support engineering | Run `scripts/android_override_tool.sh digest` (see README in the same directory) to summarise the ledger; tokens stay hashed before sharing. |
| Knowledge-check summary | `docs/source/sdk/android/readiness/reports/<date>_and7_quiz.md` + `artifacts/android/telemetry/and7_quiz/<date>-summary.json` | Docs/Support Manager | Produced via `python3 scripts/telemetry/generate_and7_quiz_summary.py --responses-dir docs/source/sdk/android/readiness/forms/responses --pass-threshold 0.9 --score-scale 100`. Columns prefixed with `q` (or flagged via `--question-column`) render the per-question accuracy table that governance reviews to spot weak topics. Attach both outputs to the packet. |
| Chaos rehearsal log | `artifacts/android/telemetry/chaos/<date>/log.ndjson` | QA automation | Attach KPI summary (stall count, retry ratio, override usage). |

## Runbook Integration & On-Call Alignment

- **Support playbooks.** `docs/source/android_support_playbook.md` (plus the localized copies under `docs/source/android_support_playbook.*.md`) now points at this policy for override scope, hashing rules, and retention. Any change to the signal table or override workflow must land in those playbooks at the same time so frontline responders and governance reviewers share a single source of truth.
- **Enablement kit linkage.** `docs/source/sdk/android/readiness/and7_operator_enablement.md` and `docs/source/sdk/android/telemetry_readiness_outline.md` embed the override CLI drills, schema-diff lab, and dashboard review material from §1–3. Update both references whenever this plan changes so presenters never rely on stale screenshots or CLI output.
- **Override ledger threading.** Every live override appears in `docs/source/sdk/android/telemetry_override_log.md` and in the hashed digests under `docs/source/sdk/android/readiness/override_logs/`. Support engineers must cite both artefacts inside the incident ticket (hash + Markdown row) before SRE will approve the change request recorded in §2.
- **On-call checklist.** The “Telemetry Redaction” section inside `docs/source/android_support_playbook.md` now mirrors the evidence matrix above: paging SRE requires a schema diff link, dashboard diff, chaos log, and override digest. Missing artefacts keep the incident in triage until the matrix is complete.

## Decision Gates & Timeline

1. **Pre-read circulation (no later than 2026‑02‑05).** Publish this document, the latest schema diff, and the runbook delta inside `docs/source/sdk/android/readiness/deck/<date>/` and ping the SRE governance list. The council slot is automatically deferred if the evidence bundle is missing.
2. **Governance review (Feb 2026 meeting).** Capture decisions plus follow-ups in `docs/source/sdk/android/readiness/cards/<date>.md`. Any requested adjustments (new signals, shorter retention) must be reflected in §1 and mirrored into `status.md` the same week.
3. **30-day burn-in.** After the council accepts the policy, run the exporter burn-in for 30 consecutive days using `ci/run_android_telemetry_chaos_prep.sh` and `ci/check_android_dashboard_parity.sh`. Store the OTLP log, parity diff, and Alertmanager snapshots under `docs/source/sdk/android/readiness/labs/reports/<YYYY-MM>/` and mention the results in the weekly `status.md` AND7 update.
4. **Documentation rollout.** Once burn-in closes green, update `docs/source/android_support_playbook.md`, `docs/source/sdk/android/telemetry_readiness_outline.md`, `docs/source/telemetry.md`, and the customer-facing FAQ in a single change so every audience reads the same retention/override guidance.

## Drill Cadence & Evidence Flow

- **Weekly chaos replay.** Execute `ci/run_android_telemetry_chaos_prep.sh` against staging at least once per week. Drop the NDJSON log, replay summary, and Alertmanager annotation bundle into `artifacts/android/telemetry/chaos/<date>/` and mirror the screenshots under `docs/source/sdk/android/readiness/screenshots/<date>/`.
- **Dashboard & allowance refresh.** After each drill, run `ci/check_android_dashboard_parity.sh --allowance docs/source/sdk/android/readiness/dashboard_parity/allowance.json --artifact docs/source/sdk/android/readiness/dashboard_parity/android_vs_rust-$(date -u +%Y%m%d).json` (or whichever dated artefact you are refreshing) and copy the diff JSON into `docs/source/sdk/android/readiness/dashboard_parity/<date>/`. Passing `--allowance` pins the manual evidence bundle to the curated allowlist while CI continues to rely on the default dashboard-scoped allowances, keeping the parity artefact current without replaying older runs.
- **Override dry runs.** Before every governance meeting, run `scripts/android_override_tool.py apply --reason=chaos-drill` with a synthetic ticket, persist the digest next to the drill outputs, and immediately revoke the token. The dry run proves that issuance, hashing, and audit logging remain functional.
- **Distribution trail.** Whenever the drill bundle changes, append the new paths to `docs/source/sdk/android/readiness/and7_operator_enablement.md` so trainers, SRE, and support can trace screenshots/logs back to the exact rehearsal referenced during enablement.

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
   allowance file referenced by `ci/check_android_dashboard_parity.sh` (via `--allowance`) and note
   the rationale in the schema-diff directory README.

> **Archive rules:** keep the five most recent diffs under
> `docs/source/sdk/android/readiness/schema_diffs/` and move older snapshots to
> `artifacts/android/telemetry/schema_diffs/` so governance reviewers always see the latest data.
