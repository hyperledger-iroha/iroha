---
lang: he
direction: rtl
source: docs/source/sdk/android/samples/operator_console.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b5ab729fa1ef8af962095e39cb11fb60545ad70f540a21b79e48a9f25732bb1a
source_last_modified: "2026-01-03T18:08:01.317812+00:00"
translation_last_reviewed: 2026-01-30
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Operator Console Sample — Design Notes & Walkthrough

**Roadmap Link:** AND5 developer experience & docs  
**Owners:** Android DX TL (sample implementation), Docs/DevRel (guide), Release Engineering (publishing artefacts)  
**Status:** Completed 2026-05-02 — sample wired to SDK + sandbox harness

## 1. Purpose & Audience

The operator console sample targets validator operators who need a deterministic way to prepare governance transactions, inspect Torii pipeline queues, and prove that StrongBox or other secure elements signed each envelope. The demo must:

- showcase how the Android SDK stitches together key management (AND2), Norito RPC networking (AND4), and telemetry hooks (AND7);
- prove provenance by attaching attestation bundles, SBOM hashes, and `sample_manifest.json` metadata to every build;
- expose troubleshooting guidance for operations teams so the sample doubles as a living runbook.

## 2. Functional Scope

| Flow | Details | Acceptance Criteria |
|------|---------|---------------------|
| Governance transaction builder | Guided composer for role/permission, staking, and treasury instructions with manifest preview | Emits Norito JSON + `.to` artefacts, validates against `iroha_cli schema check`, stores drafts locally with encrypted backups |
| StrongBox signing | Enforce StrongBox-backed keys with fallback warnings, surface attestation bundle upload to Torii | Shows alias lifecycle, supports re-key, and blocks submission if attestation verification fails |
| Pipeline inspector | List `/v1/pipeline` pending transactions, retries with exponential backoff, and surfaces `iroha_config.client.pipeline.*` overrides | Metrics feed into `android.telemetry.pipeline.*` counters; UI flags stalled queues |
| Telemetry & dashboards | Push counters to OTEL collector, link to Grafana dashboards referenced in `docs/source/sdk/android/readiness/dashboard_parity/` | Sample ships default `otel-config.yaml` plus instructions to override endpoints |

## 3. Architecture & Modules

- Lives under `examples/android/operator-console` with shared Gradle configuration (`examples/android/build.gradle.kts`).
- Depends on `:sdk:core`, `:sdk:pipeline`, and `:sdk:telemetry` modules.
- Incorporates `scripts/android_sample_env.sh` to spin up Torii mock, SoraFS fixtures, and OTEL exporter so developers can reproduce the walkthrough.
- Stores configuration in `operator_console.env` (checked into `examples/android/config/`) with documented mapping to `iroha_config::client`.

## 4. Build, Test, and CI

| Stage | Command | Notes |
|-------|---------|-------|
| Lint/tests | `./gradlew :examples:operator-console:lintRelease :examples:operator-console:testRelease` | Run locally and in CI; lint uses the shared Android lint config |
| Managed Device smoke | `./gradlew :examples:operator-console:managedDeviceDebugAndroidTest` | Required gate; baseline devices: Pixel 8 (Android 15), Pixel 6 (Android 14) |
| Manifest snapshot | `./gradlew :examples:operator-console:printSampleManifest` | Custom Gradle task writes `sample_manifest.json` referencing SDK commit, config knobs, and attestation settings |
| SBOM/provenance | `./gradlew :examples:operator-console:cyclonedxBom` + `scripts/publish_android_sdk.sh --sample operator-console` | SBOM stored under `artifacts/android/samples/operator-console/`; provenance manifest signed via Sigstore |

CI (`ci/check_android_samples.sh`) must call each stage, verify deterministic artefacts, and upload `sample_manifest.json` for diffing.

## 5. Telemetry & Observability

- OTEL exporter pushes metrics such as `android.operator_console.pipeline_retry_total`, `android.operator_console.strongbox_attestation_fail_total`, and `android.operator_console.governance_submit_latency_ms`.
- Dashboard references: `dashboards/grafana/android_operator_console.json` (to be added during AND7).
- Logs include Norito payload hashes only—no plaintext instructions—to stay within telemetry redaction policy (`docs/source/sdk/android/telemetry_redaction.md`).

## 6. Documentation Checklist

Publish `docs/source/sdk/android/samples/operator_console.md` (this file) plus a walkthrough page that covers:

- prerequisite tooling (Torii mock, SoraFS cache, OTEL collector);
- step-by-step governance transaction creation, StrongBox enrolment, attestation upload, and queue inspection;
- troubleshooting matrix (attestation mismatch, queue stuck, proxy failure) with references back to `android_support_playbook.md`.

Localization requirements: Japanese by beta, Hebrew by GA. Track translations in `docs/source/sdk/android/i18n_plan.md`.

## 7. Decisions & KPIs

The outstanding TODOs from the outline are now closed:

1. **Governance instruction subset.** Beta targets three flows: `GrantRole`, `SetParameter`, and staking (`Stake`/`Unstake`). These cover the council/UHV actions requested by operators without forcing bespoke UI per instruction. Additional opcodes can land post-beta behind a feature flag.
2. **Managed Device tests.** All managed-device runs talk to the shared Torii mock spun up by `scripts/android_sample_env.sh`. The instrumentation target reads the mock base URL from `ANDROID_OPERATOR_CONSOLE_API_BASE`, so CI and local developers can point at the same fixture without patching the APK.
3. **StrongBox reminders.** When StrongBox is unavailable, the console shows a persistent inline banner above the action buttons with remediation links. Attempting to send a transaction still triggers a modal explaining why the action is blocked, but the banner keeps the status visible during normal navigation.

### 7.1 Build/Test KPIs

Baseline timings captured from `ci/check_android_samples.sh` (M3 Max, Xcode 15.4 toolchain):

| Stage | Command | Duration | Notes |
|-------|---------|----------|-------|
| Release build + lint | `./gradlew :examples:operator-console:assembleRelease` | 6m 05s | Includes codegen + R8; cached runs stay under 3m. |
| Unit + lint suite | `./gradlew :examples:operator-console:lintRelease :examples:operator-console:testRelease` | 2m 12s | Runs Kotlin + Robolectric tests. |
| Managed device smoke | `./gradlew :examples:operator-console:managedDeviceDebugAndroidTest` | 3m 48s | Pixel 8/Android 15 target; reuses Torii mock env variables. |

These values now live in `status.md` so regressions are easy to track during AND5/AND7 work.

Document updates here whenever scope changes or new dependencies emerge.

## 8. Environment Preparation & Configuration

This section turns the design brief into a reproducible demo script. It assumes
you can run the shared sandbox bundled with the repository and that you have at
least one StrongBox-capable Pixel (see the
[device matrix](../readiness/android_strongbox_device_matrix.md)).

### 8.1 Prerequisites

- macOS or Linux workstation with Android Studio Iguana+, Android SDK Platform 34,
  and JDK 21 or newer.
- `cargo`, `python3`, and the helper scripts under `scripts/`.
- Access to the `iroha_test_network` dependencies so the sandbox can build and
  run Torii and the SoraFS fixtures.
- A StrongBox alias enrolled per the [key management guide](../key_management.md)
  so the console can exercise hardware-backed flows.

### 8.2 Bootstrap the sandbox

From the repository root run:

```bash
scripts/android_sample_env.sh \
  --profile operator \
  --telemetry-duration 8m \
  --telemetry-rps 6 \
  --torii-telemetry-profile android-operator-console
```

Key outputs land under `artifacts/android/sample_env/`:

| Artefact | Purpose |
|----------|---------|
| `operator.env` | Environment variables consumed by Gradle/Android Studio. |
| `torii.log` / `torii.metrics` | Capture pipeline responses for audits. |
| `sorafs_scoreboard.json` / `sorafs_summary.json` | Freeze the orchestrator scoreboard used during the session. |
| `telemetry/load-generator.log` | Proof that AND7 telemetry seeds ran; attach to readiness evidence. |

Load the exported variables before launching Android Studio:

```bash
set -a
source artifacts/android/sample_env/operator.env
set +a
```

or, for direnv users:

```bash
direnv dotenv artifacts/android/sample_env/operator.env
```

Gradle properties resolve the same values via `ANDROID_SAMPLE_*`, so
`./gradlew :operator-console:printSampleManifest` always reports the Torii URL,
scoreboard, and telemetry log captured during the walkthrough. The helper now
also writes `ANDROID_SAMPLE_SORAFS_SCOREBOARD_SHA256` and
`ANDROID_SAMPLE_SORAFS_SUMMARY_SHA256` so adoption reports and readiness
packets can cite the exact digest emitted by the SF-6c gate.

### 8.3 Wire the sample to the sandbox

1. Copy `artifacts/android/sample_env/operator.env` into
   `examples/android/operator-console/local.properties` (or configure Android
   Studio’s *Run/Debug* template to export the variables).
2. Inspect the generated manifest:

   ```bash
   cd examples/android
   ./gradlew :operator-console:printSampleManifest \
     -PoperatorConsoleFeatures=pipeline,attestation \
     -PoperatorConsoleScoreboard="$ANDROID_SAMPLE_SORAFS_SCOREBOARD"
   ```

3. Confirm the manifest lists the Torii endpoint captured by the sandbox and
   attach the JSON to the release or readiness ticket as required by
   `docs/source/android_release_checklist.md`.

## 9. Step-by-Step Walkthrough

Follow these phases whenever you demo the console for operators or auditors.

### Step 1 — Onboard StrongBox keys & attestation

1. Launch the sample, open **Settings → Keys & Attestation**, and pick the alias
   that maps to your validator identity.
2. When prompted, the app calls `IrohaKeyManager.generateOrLoad` with
   `KeySecurityPreference.STRONGBOX_REQUIRED`. If the UI displays a downgrade
   banner, stop the walkthrough and remediate the device per
   [key_management.md](../key_management.md#3-alias-lifecycle).
3. Tap **Capture Attestation** to mint a challenge-bound bundle. Export the ZIP
   the sample produces and verify it locally:

   ```bash
   scripts/android_keystore_attestation.sh \
     --bundle-dir artifacts/android/attestation/pixel8-strongbox-a/20260324 \
     --trust-root configs/android/trust_roots/google_root.pem \
     --require-strongbox \
     --output artifacts/android/attestation/pixel8-strongbox-a/20260324/result.json
   ```

4. Archive the attestation digest under
   `docs/source/sdk/android/readiness/android_strongbox_attestation_bundle.md`
   as required by AND2/AND6.

### Step 2 — Compose & review a governance transaction

1. From the dashboard choose **New Instruction → Governance**.
2. Select one of the beta-supported flows (`GrantRole`, `SetParameter`,
   `Stake`). The form auto-fills account metadata using
   `ANDROID_SAMPLE_TORII_ACCOUNTS`.
3. Inspect the generated Norito payload preview. The **Download** button emits
   both the JSON schema and `.to` bytes under
   `Android/data/org.hyperledger.iroha.samples.operator_console/files/drafts/`.
4. Record the draft hash so you can cross-reference it with Torii receipts
   later.

### Step 3 — Submit, monitor pipeline, and reconcile telemetry

1. Tap **Sign & Submit**; the console routes the request through the `/v1/pipeline`
   client backed by the AND4 networking stack.
2. The **Pending Queue** panel refreshes automatically and mirrors the same data
   exposed by `curl "$ANDROID_SAMPLE_TORII_URL/v1/pipeline?limit=20"`.
3. Monitor the sandbox logs while waiting:

   ```bash
   tail -f "$ANDROID_SAMPLE_TORII_LOG" &
   jq '.hash,.status' "$ANDROID_SAMPLE_TORII_ACCOUNTS" | head -n 20
   ```

4. Once the transaction finalises, capture the receipt from the UI and attach it
   to your ticket alongside the draft hash recorded earlier.

### Step 4 — Capture telemetry and evidence

1. Run the telemetry parity guard immediately after the walkthrough:

   ```bash
   ./ci/check_android_dashboard_parity.sh
   ```

   The script refreshes
   `docs/source/sdk/android/readiness/dashboard_parity/android_vs_rust-latest.json`
   and fails if AND7 dashboards drift from the Rust baseline.
2. Move the artefacts listed below into the run’s evidence bundle:
   - `sample_manifest.json` (from the Gradle task above).
   - `ANDROID_SAMPLE_SORAFS_SCOREBOARD`, `ANDROID_SAMPLE_SORAFS_SUMMARY`,
     `ANDROID_SAMPLE_SORAFS_RECEIPTS`.
   - The digest variables `ANDROID_SAMPLE_SORAFS_SCOREBOARD_SHA256` and
     `ANDROID_SAMPLE_SORAFS_SUMMARY_SHA256` (record them in the ticket or attach
     the `.env` export) so telemetry/adoption reviewers can match the evidence
     against the captured scoreboard/summary.
   - `ANDROID_SAMPLE_TELEMETRY_LOG` plus the parity diff.
   - StrongBox attestation bundle + validation JSON.
   - Torii log/metrics exported by the sandbox helper.
3. Update the release or readiness ticket with links to the evidence bundle and
   note the commit hash of the sample plus the `iroha_config` snapshot you used.

## 10. Troubleshooting & Operational Notes

| Symptom | Resolution |
|---------|------------|
| **Attestation capture fails** | Confirm the device appears as 🈴 in the [StrongBox matrix](../readiness/android_strongbox_device_matrix.md), re-run `scripts/android_keystore_attestation.sh --require-strongbox`, and document the remediation in `android_strongbox_attestation_run_log.md`. |
| **Pipeline panel never clears** | Tail the Torii log referenced by `ANDROID_SAMPLE_TORII_LOG` and cross-check the pending queue with `iroha_cli pipeline inspect`. If retries climb above the thresholds in Section 6, roll back to the last known-good sample config and file an AND4 bug. |
| **Dashboard parity diff fails** | Review the JSON written by `ci/check_android_dashboard_parity.sh`, compare it against the allowance file, and follow the override escalation in [android_support_playbook.md](../../../android_support_playbook.md#8-telemetry-redaction--observability-and7). |
| **Telemetry export refuses to run** | Ensure `scripts/android_sample_env.sh` was invoked without `--no-telemetry` and that the dry-run file `ANDROID_SAMPLE_TELEMETRY_LOG` exists. For live tests, pass `--telemetry-live --telemetry-cluster <cluster>` and capture the HTTP 202 receipts in the evidence bundle. |
| **StrongBox downgrade banner** | Device lacks StrongBox or its keystore service failed. Reboot into the latest Android 14 QPR2 build, clear the alias, and regenerate keys following the [alias lifecycle guidance](../key_management.md#3-alias-lifecycle). Do not continue until the banner clears. |

Escalate recurring issues through the AND5 tracker and keep `status.md` in sync,
as the roadmap requires weekly doc/readiness updates for the Android DX
program.
