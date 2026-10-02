<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android SDK Automation Baseline

This folder records the current Android SDK commands, artefacts, and evidence
layout referenced by
`specs/sdk/android/developer_experience_plan.md` and
`specs/sdk/android/parity_dashboard_plan.md`.

## Pipelines & Commands

| Task | Command(s) | Expected Artefacts | Notes |
|------|------------|--------------------|-------|
| Norito fixture + parity verification | `ci/check_android_fixtures.sh` (wraps `python3 scripts/check_android_fixtures.py --json-out artifacts/android/parity/<stamp>/summary.json`) | Copy the generated summary JSON into `docs/automation/android/parity/<stamp>-summary.json` | Verifies `java/iroha_android/src/test/resources` payloads, manifest hashes, and signed fixture lengths. Attach the summary alongside the cadence evidence under `artifacts/android/fixture_runs/`. |
| Canonical SDK publication | `scripts/publish_android_sdk.sh --version <version> --repo-dir <new-external-maven> [--repo-url <https>]` | Three same-version Kotlin publications, signed runtime SBOMs, and external publication hashes/receipt | Requires clean pinned source, JDK21, authenticated native build inputs and cosign. Source/provenance checks and signed SBOMs remain mandatory; tests and lint are optional development diagnostics outside publication. Java consumers retain Kotlin JDK8 API checks. Remote credentials use runtime environment inputs and never enter project arguments or receipts. Component publication is not native/device/release qualification. |
| Parity dashboard feed | `python3 scripts/check_android_fixtures.py … --json-out artifacts/android/parity/<stamp>/summary.json` followed by `python3 scripts/android_parity_metrics.py --summary <summary> --output artifacts/android/parity/<stamp>/metrics.prom` | Copy the `metrics.prom` snapshot or the Grafana export JSON into `docs/automation/android/parity/<stamp>-metrics.prom` | Feeds the dashboard plan so AND5/AND7 governance can verify invalid submission counters and telemetry adoption. |

## Evidence Capture

1. **Timestamp everything.** Name files using UTC timestamps
   (`YYYYMMDDTHHMMSSZ`) so parity dashboards, governance minutes, and published
   docs can reference the same run deterministically.
2. **Reference commits.** Each log should include the git commit hash of the run
   plus any relevant configuration (e.g., `ANDROID_PARITY_PIPELINE_METADATA`).
   When privacy requires redaction, include a note and link to the secure vault.
3. **Archive minimal context.** We only check in structured summaries (JSON,
   `.prom`, `.log`). Heavy artefacts (APK bundles, screenshots) should remain in
   `artifacts/` or object storage with a signed hash recorded in the log.
4. **Update status entries.** When AND5 milestones advance in `status.md`, cite
   the corresponding file (e.g., `docs/automation/android/parity/20260324T010203Z-summary.json`)
   so auditors can trace the baseline without scraping CI logs.

Public and translated Android documentation is maintained in the sibling
`iroha-docs` repository and published at <https://docs.iroha.tech/>.
