---
lang: zh-hant
direction: ltr
source: docs/automation/android/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 676798a4cf7c3e7737a0f80640f3f268a2f625f92afdd359ac528881d2aeb046
source_last_modified: "2025-12-29T18:16:35.060950+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android Documentation Automation Baseline (AND5)

Roadmap item AND5 requires documentation, localization, and publishing
automation to be auditable before AND6 (CI & Compliance) can start. This folder
records the commands, artefacts, and evidence layout that AND5/AND6 reference,
mirroring the plans captured in
`docs/source/sdk/android/developer_experience_plan.md` and
`docs/source/sdk/android/parity_dashboard_plan.md`.

## Pipelines & Commands

| Task | Command(s) | Expected Artefacts | Notes |
|------|------------|--------------------|-------|
| Localization stub sync | `python3 scripts/sync_docs_i18n.py` (optionally pass `--lang <code>` per run) | Log file stored under `docs/automation/android/i18n/<timestamp>-sync.log` plus the translated stub commits | Keeps `docs/i18n/manifest.json` in sync with translated stubs; the log records language codes touched and the git commit captured in the baseline. |
| Norito fixture + parity verification | `ci/check_android_fixtures.sh` (wraps `python3 scripts/check_android_fixtures.py --json-out artifacts/android/parity/<stamp>/summary.json`) | Copy the generated summary JSON into `docs/automation/android/parity/<stamp>-summary.json` | Verifies `java/iroha_android/src/test/resources` payloads, manifest hashes, and signed fixture lengths. Attach the summary alongside the cadence evidence under `artifacts/android/fixture_runs/`. |
| Sample manifest & publishing proof | `scripts/publish_android_sdk.sh --version <semver> [--repo-url …]` (runs tests + SBOM + provenance) | Provenance bundle metadata plus the resulting `sample_manifest.json` from `docs/source/sdk/android/samples/` stored under `docs/automation/android/samples/<version>/` | Ties AND5 sample apps and release automation together—capture the generated manifest, SBOM hash, and provenance log for the beta review. |
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

Following this layout satisfies the “docs/automation baselines available for
audit” prerequisite that AND6 cites and keeps the Android documentation program
in lockstep with the published plans.
