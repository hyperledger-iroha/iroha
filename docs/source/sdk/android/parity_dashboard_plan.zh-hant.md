---
lang: zh-hant
direction: ltr
source: docs/source/sdk/android/parity_dashboard_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8afea6b7b7813b13aaf1e79a14473939d3ed42789db533715f680e01ef8ce20a
source_last_modified: "2026-01-05T09:28:12.058617+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android Norito Parity Dashboard Plan (AND3)

This note documents how the Android SDK parity evidence will be captured and
exposed to the observability stack for roadmap **AND3** (Norito parity &
instruction builders). The plan aligns the new `scripts/check_android_fixtures.py
--json-out` summary feed, Buildkite automation, and Grafana dashboards so
release candidates cannot merge when fixture drift or regen SLA breaches occur.

## Scope

- Cover parity between the Rust canonical fixtures and the Java resources under
  `java/iroha_android/src/test/resources`.
- Surface regeneration cadence and owning engineer for every rotation (bi-weekly
  Tue/Fri 09:00 UTC windows).
- Track pipeline health for `ci/run_android_tests.sh` and `make
  android-fixtures-check`.
- Provide JSON artefacts that downstream systems (Grafana/Alertmanager and
  release gates) can consume without scraping console output.
- Export Prometheus textfiles from the same job so dashboards do not depend on
  ad-hoc log scraping or manual uploads.

## Data Sources

1. **Parity summary JSON:** `scripts/check_android_fixtures.py` now accepts
   `--json-out <path>` and writes a structured summary describing the manifest,
   fixtures, exit status, and any validation errors. The file includes the
   cadence state (`artifacts/android_fixture_regen_state.json`) so dashboards
   know who last regenerated fixtures and which cadence label applied.
   `scripts/android_parity_pipeline_metadata.py` emits a Buildkite-friendly
   metadata blob (job duration, per-test durations, Buildkite URL) that gets
   injected back into the summary and recorded under
   `artifacts/android/parity/<timestamp>/pipeline_metadata.json`.
2. **Regen state:** `scripts/android_fixture_regen.sh` continues to track
   `generated_at`, `rotation_owner`, and cadence labels in
   `artifacts/android_fixture_regen_state.json`.
3. **CI signals:** `ci/run_android_tests.sh` plus
   `ci/check_android_fixtures.sh` run on Buildkite. The Buildkite job now
   writes the parity summary to `artifacts/android/parity/<timestamp>/summary.json`,
   injects the pipeline metadata described above, exports a Prometheus
   textfile (`metrics.prom`) for textfile collectors, and copies everything to
   `artifacts/android/parity/latest/` for stable dashboard links.
4. **SoraFS capacity marketplace builders:** the dedicated regression suite
   (`SorafsCapacityMarketplaceInstructionTests`) now covers Norito round-trips
   for `RegisterCapacityDeclaration`, `RegisterCapacityDispute`,
   `SetPricingSchedule`, and `UpsertProviderCredit`. These tests exercise the
   same typed builders (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/model/instructions/*.java`)
   that parity dashboards rely on, so fixture diffs immediately surface schema
   drift for the capacity marketplace flows in AND7’s scope.

## Automation Steps

1. **Local runs**

   ```bash
   # writes artifacts/android/parity/<timestamp>/summary.json + latest copy
   make android-fixtures-check
   ```

   Override the destination with
   `ANDROID_PARITY_SUMMARY=target-codex/parity/summary.json` (or any repository
   path) when needed; the CI wrapper still enforces manifest/resources locations
   inside the repo so audit trails stay deterministic. The summary contains:

   - `generated_at`, `resources_dir`, `fixtures_path`, and `manifest_path`.
   - `state.rotation_owner`, `state.cadence`, and `state.generated_at` (if the
     regen state file is present).
   - `result.status`, `result.error_count`, and `result.errors[]`.

   When pipeline/test metadata is produced dynamically (for example via
   `buildkite-agent meta-data get android_parity_pipeline_metadata`), persist it
   to a JSON file and set `ANDROID_PARITY_PIPELINE_METADATA=<file>` before
   running the wrapper:

   ```bash
   buildkite-agent meta-data get android_parity_pipeline_metadata \
     > artifacts/android/parity/pipeline_metadata.json
   ANDROID_PARITY_PIPELINE_METADATA=artifacts/android/parity/pipeline_metadata.json \
     make android-fixtures-check
   ```

2. **CI/Buildkite**

   - The Buildkite step now records job/test durations via
     `scripts/android_parity_pipeline_metadata.py` and injects them into the
     parity summary before uploading artefacts.
   - `artifacts/android/parity/<stamp>/summary.json` (and the `latest/` copy)
     always carry the pipeline metadata block and get mirrored alongside
     `metrics.prom` so Grafana and Alertmanager can ingest the parity state
     without log scraping.
   - `.buildkite/android-parity.yml` uploads the summary, pipeline metadata, and
     metrics for every run; alerts fire from the Prometheus textfile when
     `result.status != "ok"` or the regen age breaches SLA.

3. **Dashboards**

   - Grafana datasource points at the S3 bucket (or the Buildkite artefact proxy)
     and renders:
     - **Fixture Drift Panel:** Plots `result.error_count`; shows the last five
       errors when non-zero.
     - **Regen SLA Panel:** Displays `state.generated_at` vs current time,
       highlighting breaches over 48 hours.
     - **Owner Heatmap:** Aggregates `state.rotation_owner` to ensure rotations
       are shared across the on-call roster.
   - Alert rules fire when:
     - `result.status == "error"` for more than one run.
     - `state.generated_at` age exceeds 48 hours.
     - No summary JSON has been uploaded within the last 24 hours (CI gap).

## Release Gating

- `make android-fixtures-check` must run with `--json-out
  artifacts/android/parity/latest/summary.json` before tagging a release.
- `ci/check_android_fixtures.sh` now fails when the summary JSON is missing,
  reports `result.status != "ok"`, or references fixtures outside the
  repository, preventing CI from publishing stale or incomplete parity
  evidence.
- Release engineering attaches the latest summary JSON to the candidate ticket so
  auditors can inspect fixture drift without replaying the CI logs.

## Grafana Dashboard

- Dashboard JSON lives in `dashboards/grafana/android_parity_overview.json`.
- Panels:
  1. **Parity Errors (latest run)** — stat panel with alert when
     `android_parity_error_count > 0` and `android_parity_summary_status{status!="ok"}` rises above zero.
  2. **Hours Since Last Regen** — stat panel with alert when regen age exceeds
     48 hours.
  3. **Rotation Owner Heatmap** — bar gauge showing the last rotation owner
     distribution.
  4. **Recent Summary Snapshots** — table listing the latest recorded summary
     timestamps (`android_parity_summary_timestamp_seconds` vector).
- Alerts are configured inline using Grafana's classic alerting schema so
  on-call can receive notifications directly out of the dashboard once the
  Prometheus metrics land.

## Next Steps

1. Mirror the parity summary + Prometheus textfile workflow for Swift so IOS2
   dashboards surface the same `summary_status`, `error_count`, and cadence
   gauges without bespoke exporters.
2. Keep the Android summary S3 uploads and metrics feed attached to release
   automation by wiring `ANDROID_PARITY_S3_PREFIX`/`ANDROID_PARITY_METRICS_PATH`
   into the publishing pipelines so governance can fetch artefacts outside of
   Buildkite.
