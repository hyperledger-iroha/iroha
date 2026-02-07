---
lang: uz
direction: ltr
source: docs/source/android_release_checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5ee3613b544a847953f5ec152092cb2fe1da35279c5482486513d6b8d6dddf02
source_last_modified: "2026-01-05T09:28:11.999717+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android Release Checklist (AND6)

This checklist captures the **AND6 — CI & Compliance Hardening** gates from
`roadmap.md` (§Priority 5). It aligns Android SDK releases with the Rust
release RFC expectations by spelling out the CI jobs, compliance artefacts,
device-lab evidence, and provenance bundles that must be attached before a GA,
LTS, or hotfix train moves forward.

Use this document together with:

- `docs/source/android_support_playbook.md` — release calendar, SLAs, and
  escalation tree.
- `docs/source/android_runbook.md` — day‑to‑day operational runbooks.
- `docs/source/compliance/android/and6_compliance_checklist.md` — regulator
  artefact inventory.
- `docs/source/release_dual_track_runbook.md` — dual-track release governance.

## 1. Stage Gates at a Glance

| Stage | Required Gates | Evidence |
|-------|----------------|----------|
| **T−7 days (pre-freeze)** | Nightly `ci/run_android_tests.sh` green for 14 days; `ci/check_android_fixtures.sh`, `ci/check_android_samples.sh`, and `ci/check_android_docs_i18n.sh` passing; lint/dependency scans queued. | Buildkite dashboards, fixture diff report, sample screenshot snapshots. |
| **T−3 days (RC promotion)** | Device-lab reservation confirmed; StrongBox attestation CI run (`scripts/android_strongbox_attestation_ci.sh`); Robolectric/instrumented suites exercised on scheduled hardware; `./gradlew lintRelease ktlintCheck detekt dependencyGuard` clean. | Device matrix CSV, attestation bundle manifest, Gradle reports archived under `artifacts/android/lint/<version>/`. |
| **T−1 day (go/no-go)** | Telemetry redaction status bundle refreshed (`scripts/telemetry/check_redaction_status.py --write-cache`); compliance artefacts updated per `and6_compliance_checklist.md`; provenance rehearsal completed (`scripts/android_sbom_provenance.sh --dry-run`). | `docs/source/compliance/android/evidence_log.csv`, telemetry status JSON, provenance dry-run log. |
| **T0 (GA/LTS cutover)** | `scripts/publish_android_sdk.sh --dry-run` completed; provenance + SBOM signed; release checklist exported and attached to go/no-go minutes; `ci/sdk_sorafs_orchestrator.sh` smoke job green. | Release RFC attachments, Sigstore bundle, adoption artefacts under `artifacts/android/`. |
| **T+1 day (post-cutover)** | Hotfix readiness verified (`scripts/publish_android_sdk.sh --validate-bundle`); dashboard diffs reviewed (`ci/check_android_dashboard_parity.sh`); evidence packet uploaded to `status.md`. | Dashboard diff export, link to `status.md` entry, archived release packet. |

## 2. CI & Quality Gate Matrix

| Gate | Command(s) / Script | Notes |
|------|--------------------|-------|
| Unit + integration tests | `ci/run_android_tests.sh` (wraps `ci/run_android_tests.sh`) | Emits `artifacts/android/tests/test-summary.json` + test log. Includes Norito codec, queue, StrongBox fallback, and Torii client harness tests. Required nightly and before tagging. |
| Fixture parity | `ci/check_android_fixtures.sh` (wraps `scripts/check_android_fixtures.py`) | Ensures regenerated Norito fixtures match the Rust canonical set; attach the JSON diff when the gate fails. |
| Sample apps | `ci/check_android_samples.sh` | Builds `examples/android/{operator-console,retail-wallet}` and validates localized screenshots via `scripts/android_sample_localization.py`. |
| Docs/I18N | `ci/check_android_docs_i18n.sh` | Guards README + localized quickstarts. Run again after doc edits land in the release branch. |
| Dashboard parity | `ci/check_android_dashboard_parity.sh` | Confirms CI/exported metrics align with the Rust counterparts; required during T+1 verification. |
| SDK adoption smoke | `ci/sdk_sorafs_orchestrator.sh` | Exercises the multi-source Sorafs orchestrator bindings with the current SDK. Required before uploading staged artefacts. |
| Attestation verification | `scripts/android_strongbox_attestation_ci.sh --summary-out artifacts/android/attestation/ci-summary.json` | Aggregates the StrongBox/TEE attestation bundles under `artifacts/android/attestation/**`; attach the summary to GA packets. |
| Device-lab slot validation | `scripts/check_android_device_lab_slot.py --root artifacts/android/device_lab/<slot> --json-out artifacts/android/device_lab/summary.json` | Validates instrumentation bundles before attaching evidence to release packets; CI runs against the sample slot in `fixtures/android/device_lab/slot-sample` (telemetry/attestation/queue/logs + `sha256sum.txt`). |

> **Tip:** add these jobs to the `android-release` Buildkite pipeline so that
> freeze weeks automatically re-run every gate with the release branch tip.

The consolidated `.github/workflows/android-and6.yml` job runs the lint,
test-suite, attestation-summary, and device-lab slot checks on every PR/push
touching Android sources, uploading evidence under `artifacts/android/{lint,tests,attestation,device_lab}/`.

## 3. Lint & Dependency Scans

Run `scripts/android_lint_checks.sh --version <semver>` from the repo root. The
script executes:

```
lintRelease ktlintCheck detekt dependencyGuardBaseline \
:operator-console:lintRelease :retail-wallet:lintRelease
```

- Reports and dependency-guard outputs are archived under
  `artifacts/android/lint/<label>/` and a `latest/` symlink for release
  pipelines.
- Failing lint findings require either remediation or an entry in the release
  RFC documenting the accepted risk (approved by Release Engineering + Program
  Lead).
- `dependencyGuardBaseline` regenerates the dependency lock; attach the diff
  to the go/no-go packet.

## 4. Device Lab & StrongBox Coverage

1. Reserve Pixel + Galaxy devices using the capacity tracker referenced in
   `docs/source/compliance/android/device_lab_contingency.md`. Blocks releases
   if <70 % availability.
2. Execute `scripts/android_strongbox_attestation_ci.sh --report \
   artifacts/android/attestation/<version>` to refresh the attestation report.
3. Run the instrumentation matrix (document the suite/ABI list in the device
   tracker). Capture failures in the incident log even if retries succeed.
4. File a ticket if fallback to Firebase Test Lab is required; link the ticket
   in the checklist below.

## 5. Compliance & Telemetry Artefacts

- Follow `docs/source/compliance/android/and6_compliance_checklist.md` for EU
  and JP submissions. Update `docs/source/compliance/android/evidence_log.csv`
  with hashes + Buildkite job URLs.
- Refresh telemetry redaction evidence via
  `scripts/telemetry/check_redaction_status.py --write-cache \
   --status-url https://android-observability.example/status.json`.
  Store the resulting JSON under
  `artifacts/android/telemetry/<version>/status.json`.
- Record the schema diff output from
  `scripts/telemetry/run_schema_diff.sh --android-config ... --rust-config ...`
  to prove parity with Rust exporters.

## 6. Provenance, SBOM, and Publishing

1. Dry-run the publish pipeline:

   ```bash
   scripts/publish_android_sdk.sh \
     --version <semver> \
     --repo-dir artifacts/android/maven/<semver> \
     --dry-run
   ```

2. Generate SBOM + Sigstore provenance:

   ```bash
   scripts/android_sbom_provenance.sh \
     --version <semver> \
     --out artifacts/android/provenance/<semver>
   ```

3. Attach `artifacts/android/provenance/<semver>/manifest.json` and signed
   `checksums.sha256` to the release RFC.
4. When promoting to the real Maven repository, rerun
   `scripts/publish_android_sdk.sh` without `--dry-run`, capture the console
   log, and upload the resulting artefacts to `artifacts/android/maven/<semver>`.

## 7. Submission Packet Template

Every GA/LTS/hotfix release should include:

1. **Completed checklist** — copy this file’s table, tick each item, and link
   to supporting artefacts (Buildkite run, logs, doc diffs).
2. **Device lab evidence** — attestation report summary, reservation log, and
   any contingency activations.
3. **Telemetry packet** — redaction status JSON, schema diff, link to
   `docs/source/sdk/android/telemetry_redaction.md` updates (if any).
4. **Compliance artefacts** — entries added/updated in the compliance folder
   plus the refreshed evidence log CSV.
5. **Provenance bundle** — SBOM, Sigstore signature, and `checksums.sha256`.
6. **Release summary** — one‑page overview attached to `status.md` summarising
   the above (date, version, highlight of any waived gates).

Store the packet under `artifacts/android/releases/<version>/` and reference it
in `status.md` and the release RFC.

- `scripts/run_release_pipeline.py --publish-android-sdk ...` automatically
  copies the latest lint archive (`artifacts/android/lint/latest`) and the
  compliance evidence log into `artifacts/android/releases/<version>/` so the
  submission packet always has a canonical location.

---

**Reminder:** update this checklist whenever new CI jobs, compliance artefacts,
or telemetry requirements are added. Roadmap item AND6 stays open until the
checklist and associated automation prove stable for two consecutive release
trains.
