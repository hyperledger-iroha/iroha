# Iroha Android Samples

This workspace hosts the AND5 reference applications:

- **operator-console** — demonstrates governance/operator flows (pending queue, StrongBox posture, telemetry status widgets).
- **retail-wallet** — showcases offline signing envelopes, deterministic exports, and recovery hand-offs for customer devices.

Both apps are now wired to the in-repo Android SDK: the operator console reads
the sample harness exports (`android_sample_env.sh`) and surfaces StrongBox
posture, Torii health, queue depth, and SoraFS evidence digests, while the
retail wallet exercises offline envelope previews, policy/handoff metadata, and
revocation/manifest snapshots bundled with the sample assets.

Telemetry is enabled automatically when `ANDROID_SAMPLE_TELEMETRY_LOG` is set
(exported by `android_sample_env.sh`). The operator console attaches a
redaction-aware `TelemetryObserver` and writes JSON Lines records to the
configured log, hashing authorities with the sample salt/version/rotation ids
embedded in the app manifest so AND7 dashboards can ingest the evidence
directly.

## Prerequisites

- Android Studio Iguana or newer (Gradle 8.2+, AGP 8.2+).
- Android SDK Platform 34 and Build Tools 34.0.0.
- JDK 21+ on your PATH or via `JAVA_HOME`.

## Building

```bash
cd examples/android
./gradlew :operator-console:assembleDebug
./gradlew :retail-wallet:assembleDebug
```

The provided `gradlew` script delegates to the system Gradle installation. If Gradle is not available, install version 8.2 or newer, or run the tasks from Android Studio.

From the repository root you can invoke `scripts/check_android_samples.sh` to build both apps (and the shared SDK module) in one go. Wire this script into CI once Gradle is available on the runners.

## Sample Manifests

Every sample emits a `build/sample-manifest/sample_manifest.json` file describing the SDK version, Torii endpoint, and feature flags used for the build. Generate or inspect the manifest with:

```bash
cd examples/android
./gradlew :operator-console:printSampleManifest
./gradlew :retail-wallet:printSampleManifest
```

Override the recorded metadata via Gradle properties, for example:

```bash
./gradlew :operator-console:printSampleManifest \
  -PoperatorConsoleToriiEndpoint=https://torii.devnet.sora.org \
  -PoperatorConsoleFeatures=pipeline,attestation
```

The CI helper (`scripts/check_android_samples.sh`) runs these tasks and fails if the manifest files are missing so release artifacts always include the provenance snippet referenced by `docs/source/sdk/android/developer_experience_plan.md`.

Manifests now capture the sandbox inputs exported by `android_sample_env.sh`
(`sorafs_scoreboard`/`sorafs_summary` paths + SHA-256 digests, telemetry logs,
harness receipts, and optional hand-off endpoints), along with retail wallet
policy overrides and SHA-256 fingerprints for the bundled POS assets
(`pos_manifest.json`, `security_policy.json`, `offline_revocations.json`,
`pinned_root.pem`). The localization annotator still augments each manifest with
available screenshots for `en`, `ja`, and `he`.

## Local Environment Bootstrap

Use `scripts/android_sample_env.sh` to spin up the supporting sandbox when demoing either sample:

```bash
# Operator console defaults: Torii sandbox + SoraFS fixture + telemetry seed
scripts/android_sample_env.sh

# Wallet preview: adds the mock hand-off receiver and writes retail-specific .env hints
scripts/android_sample_env.sh --wallet --handoff
```

The helper stores artefacts under `artifacts/android/sample_env/` and writes a
`${profile}.env` file (e.g. `retail-wallet.env`) with the exported Torii URL,
account dump, SoraFS scoreboard, telemetry log, and hand-off endpoint. Load this
file in Android Studio or your shell before launching the sample UI to keep the
demo pointing at the freshly provisioned sandbox.

## Telemetry & Evidence

- Override the sample telemetry salt/version/rotation/exporter via
  `ANDROID_SAMPLE_TELEMETRY_SALT_HEX`,
  `ANDROID_SAMPLE_TELEMETRY_SALT_VERSION`,
  `ANDROID_SAMPLE_TELEMETRY_ROTATION_ID`, and
  `ANDROID_SAMPLE_TELEMETRY_EXPORTER` if you need to mirror production manifests.
- The operator console writes JSONL telemetry to `ANDROID_SAMPLE_TELEMETRY_LOG`
  with `event`, `route`, `method`, `latency_ms`, `status_code`, and hashed
  authorities; the sink also emits `android.telemetry.export.status` so the
  adoption harness can fail fast on exporter errors.
- Sample manifests now embed the telemetry redaction parameters to keep
  provenance bundles self-contained.

Track implementation progress in `docs/source/sdk/android/developer_experience_plan.md`
and update the corresponding roadmap entries when milestones advance.
