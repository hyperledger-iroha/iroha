---
lang: pt
direction: ltr
source: docs/source/sdk/android/configuration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9cca3ce17a7c2e3ba699b139fe08097d5890050315e92b1695bd6bac3c3c3009
source_last_modified: "2026-01-04T11:42:43.528518+00:00"
translation_last_reviewed: 2026-01-30
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android Client Configuration Guide (AND5)

Roadmap item **AND5 P1** requires a configuration guide that threads every
runtime knob in the Android SDK back to the canonical `iroha_config`
manifests. This document explains how to generate, distribute, and validate the
`ClientConfig` bundles consumed by apps so configuration stays deterministic and
operator runbooks (see `docs/source/android_runbook.md`) can rely on the same
checksums as Rust services.

## 1. Source of truth & manifest pipeline

1. **Start from `iroha_config`.** Use the CLI helpers that ship with the
   workspace (`iroha_cli config show --actual` or the config snapshot emitted
   during provisioning) to materialise the Torii, telemetry, and SoraFS blocks
   for the target environment. Do not edit values by hand—record overrides in
   the config repository and commit the diffs.
2. **Produce an Android manifest.** Convert the `iroha_config` fragments into a
   JSON manifest stored under `configs/android_client_manifest.json` (or a
   release-specific filename alongside it). Each publish/runbook entry should
   reference the git SHA + checksum of this file. The recommended layout is:
   ```jsonc
   {
     "torii": {
       "base_uri": "https://torii.nexus.sora.org",
       "sorafs_gateway_uri": "https://sorafs-gw.nexus.sora.org",
       "timeout_ms": 10000,
       "default_headers": {
         "User-Agent": "IrohaAndroid/0.9.0",
         "X-Iroha-Override-Policy": "strict"
       }
     },
     "retry": { "max_attempts": 4, "base_delay_ms": 400, "max_delay_ms": 6000 },
     "pending_queue": {
       "kind": "offline_journal",
       "path": "Android/data/org.hyperledger.iroha/app-pending.queue"
     },
     "telemetry": {
       "enabled": true,
       "exporter_name": "android-main",
       "sink": "https://otel.nexus.sora.org/v1/traces",
       "redaction": {
         "salt_b64": "mX5Z…==",
         "salt_version": "2026-03-05T00:00Z",
         "rotation_id": "telemetry-salt-2026q1"
       }
     }
   }
   ```
   The `torii` block maps directly to
   `ClientConfig.Builder.setBaseUri(...)`, `.setSorafsGatewayUri(...)`,
   `.setRequestTimeout(...)`, and `.putDefaultHeader(...)`. The `retry` section
   feeds `RetryPolicy.Builder`—`base_delay_ms` maps to
   `setBaseDelay(Duration.ofMillis(...))` and the optional `max_delay_ms`
   clamps linear backoff via `setMaxDelay(...)`. The `pending_queue` section indicates whether
   `HttpClientTransport.withOfflineJournalQueue(...)` should wrap the config
   with `OfflineJournalPendingTransactionQueue` or stay in memory. Telemetry
   options convert into `TelemetryOptions.builder().setTelemetryRedaction(...)`
   (see Section 3).
3. **Record provenance.** Every manifest rotation should produce a SHA256 hash
   (drop it beside the manifest or in `status.md`) and a short README capturing
   the environment, Torii cluster, and signer. This same digest appears in the
   Android logs when the manifest loads so incidents can compare values quickly.
4. **Ship the manifest with the app.** Release automation bundles the manifest
  inside the artefact (or alongside the app in managed distributions) so the
  `ConfigWatcher` can hot-reload without network requests. Sample scripts such
  as `scripts/release/load_env.sh android-maven-staging` are already wired to
  inject the manifest path via Gradle properties.

## 2. Boot-time ingestion & validation

`ClientConfig` is immutable; every adjustment must happen before sharing the
instance with transports:

- Load the manifest at start-up, feed it into a builder, and log the SHA256
  digest. The operator console sample
  (`examples/android/operator-console/src/main/java/.../SampleClientFactory.kt`)
  demonstrates the plumbing with env-specific defaults, retry policy, pending
  queue wiring, and telemetry hooks.
- Store the manifest hash in structured logs (the samples emit it via a
  `ConfigDigestLogger`). This hash is the value incident responders compare to
  the `configs/android_client_manifest.json` digest when resolving drift.
- If manifest validation fails, emit the `android.telemetry.config.reload`
  signal and fall back to read-only-safe defaults (no pending queue submissions,
  telemetry disabled) per `docs/source/android_runbook.md` §1/§2.

## 3. Telemetry redaction & exporters

`java/iroha_android/src/main/java/org/hyperledger/iroha/android/telemetry/TelemetryOptions.java`
captures the AND7 requirements. When constructing `TelemetryOptions` from the
manifest:

- `enabled` toggles whether the `TelemetryObserver` attaches itself in
  `ClientConfig` (`ClientConfig` wraps sinks via
  `TelemetryExportStatusSink.wrap(...)`).
- `redaction.salt_b64` maps to `TelemetryOptions.Redaction.Builder#setSalt(...)`;
  the value must match the salt stored in the Rust config. The manifest should
  also capture `salt_version` and `rotation_id` strings so the SDK can emit
  `android.telemetry.redaction.salt_version`.
- `network_context`/`device_profile` booleans map to
  `ClientConfig.Builder.setNetworkContextProvider(...)` and
  `.setDeviceProfileProvider(...)`. On Android, use
  `AndroidNetworkContextProvider.fromContext(context)` and
  `AndroidDeviceProfileProvider.create()` so the telemetry stream matches the
  AND7 inventory.
- The exporter URI and headers populate `TelemetrySink` implementations used by
  `TelemetryObserverTests` and `ClientConfigKeystoreTelemetryTests`
  (`java/iroha_android/src/test/java/...`) to guarantee redaction metadata is
  attached when keystore events fire.

Keep all telemetry manifests under version control and diff them using
`scripts/telemetry/run_schema_diff.sh --android-config <manifest> --rust-config configs/rust_telemetry.json`
before deployments so governance sees the same evidence the Rust team reviews.

## 4. Pending queues & offline envelopes

Offline workflows require consistent queue configuration:

- Use `pending_queue.path` to determine whether the SDK should wrap
  `ClientConfig` via `HttpClientTransport.withOfflineJournalQueue(...)`. That
  helper installs `OfflineJournalPendingTransactionQueue` and associates an
  `OfflineJournalKey` (seed/passphrase) so queued entries survive restarts and
  encrypt to disk.
- Document the queue path in the manifest so support engineers can pull artefacts
  during incidents (`docs/source/android_runbook.md` §3). The operator console
  sample names queues via the selected profile; mirror that behaviour for
  partner apps.
- When offline signing envelopes are required, propagate manifest entries into
  `ClientConfig.ExportOptions` so the `HttpClientTransport` instrumentation and
  `FilePendingTransactionQueueTests` continue to assert alias preservation +
  deterministic exports.

## 5. Hot reload & overrides

Deployments that need config tweaks without rebuilding the app should:

1. Drop the new manifest alongside the old file and signal the
   `ConfigWatcher` (shared helper referenced throughout
   `docs/source/android_runbook.md`). The watcher compares digests, reloads via
   `ClientConfig.toBuilder()`, and emits the `android.telemetry.config.reload`
   event with `result=success` or `result=failure`.
2. If telemetry needs a temporary override, follow the override workflow in the
   runbook (`scripts/android_override_tool.sh`) instead of editing manifests.
   The guide keeps `scripts/android_override_tool.py` and
   `docs/source/sdk/android/telemetry_override_log.md` in sync with the active
   config so governance can audit overrides alongside manifest digests.

## 6. Local development profile

For local testing, reuse the sample helpers:

- `SampleEnvironment` (operator console sample) reads environment variables like
  `ANDROID_SAMPLE_TORII_URL` and produces URIs + headers for `ClientConfig`.
- `SampleClientFactory` demonstrates how to build a config with
  `AndroidNetworkContextProvider`, `AndroidDeviceProfileProvider`, and a file
  backed pending queue.
- `scripts/android_sample_env.sh` prepares `.env` files for both samples and
  ensures the resulting config still passes the schema diff checks.
  Developers should run `make android-tests` and the sample Gradle tasks before
  shipping new manifests.

## 7. Validation checklist

Before distributing a manifest or cutting a release:

1. **Schema diff:** Run
   `scripts/telemetry/run_schema_diff.sh --android-config configs/android_telemetry.json --rust-config configs/rust_telemetry.json`
   and check the `policy_violations` array documented in
   `docs/source/sdk/android/telemetry_schema_diff.md`.
2. **Unit coverage:** Execute `ci/run_android_tests.sh` so the
   configuration-centric test suites (`ClientConfigNoritoRpcTests`,
   `ClientConfigKeystoreTelemetryTests`, `HttpClientTransportHarnessTests`)
   confirm the manifest-derived config produces the expected observers,
   telemetry wiring, and fallback behaviour.
3. **Pending queue rehearsal:** When `pending_queue.kind` requires disk-backed
   queues, run the operator console sample’s queue reconciliation flow to ensure
   `FilePendingTransactionQueue` can replay records with the new config.
4. **Log verification:** Capture the manifest digest, telemetry exporter target,
   and `android.telemetry.config.reload` result from logcat, then file them in
   the release evidence bundle (`docs/source/sdk/android/readiness/...`).

Following these steps keeps Android configuration aligned with the roadmap
goals and guarantees that operators, SDK consumers, and governance reviewers can
trace every knob back to `iroha_config` and the evidence stored in this
repository.
