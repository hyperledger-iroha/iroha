---
lang: my
direction: ltr
source: docs/source/telemetry/swift_status_feeds.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5048036a667a225e784178dfa077205e78132bb781a98305f492bf3b8c154845
source_last_modified: "2026-01-05T09:28:12.102523+00:00"
translation_last_reviewed: 2026-02-07
title: Swift Status Feed Provisioning
summary: How parity/CI telemetry feeds are produced and consumed by the Swift weekly digest exporter.
---

# Swift Status Feed Provisioning

This note documents how the Swift SDK weekly digest exporter retrieves the
parity and CI dashboard feeds once telemetry automation is live. It serves as
the operational hand-off between the telemetry exporter job and the status
reporting workflow.

## Feed Contracts

- **Parity feed** — JSON document matching `docs/source/references/ios_metrics.md`
  (`mobile_parity` schema). Published every 15 minutes.
- **CI feed** — JSON document matching the same schema (`mobile_ci` object).
  Published after each Buildkite run and hourly if no new runs occur.
- Both feeds must be accessible via HTTPS endpoints (or pre-signed S3 URLs)
  for the CI job to download.

## Exporter Integration

`scripts/swift_status_export.py` can consume local files or remote URLs via
`--parity-url` / `--ci-url`. Remote downloads automatically retry with
exponential backoff; defaults are 3 attempts with an initial 2 s delay. The CI
wrapper (`ci/swift_status_export.sh`) resolves secrets provided as plain
environment variables, `<VAR>_FILE` pointers, or Buildkite meta-data keys
(`<VAR>_META_KEY`) so pipelines can rely on the standard secrets plug-ins.

Example CI invocation:

```bash
python3 scripts/swift_status_export.py \
  --parity-url "$SWIFT_PARITY_FEED_URL" \
  --ci-url "$SWIFT_CI_FEED_URL" \
  --format markdown \
  --slack-webhook "$SWIFT_STATUS_SLACK_WEBHOOK" \
  --retry-attempts 5 \
  --retry-backoff 1
```

## Secret Management

- Store feed URLs (or tokens) in Buildkite secrets (`swift_status_export` group).
- Export as environment variables `SWIFT_PARITY_FEED_URL` and `SWIFT_CI_FEED_URL`
  — either directly, via `<VAR>_FILE`, or by publishing Buildkite meta-data keys
  `SWIFT_PARITY_FEED_URL_META_KEY` / `SWIFT_CI_FEED_URL_META_KEY` ahead of the job.
- Operators can seed those keys with
  `scripts/buildkite/swift_status_seed_meta.sh --parity-url <url> --ci-url <url> --slack-webhook <url>`
  (add `--dry-run` locally to preview commands).
- Provide a Slack webhook using `SWIFT_STATUS_SLACK_WEBHOOK` (`_FILE`/`_META_KEY`
  variants supported). The exporter posts a digest headline + preview once the
  artefacts upload; override the preview line count with
  `SWIFT_STATUS_SLACK_PREVIEW_LINES`. For local dry-runs, point the webhook at a
  `file://` target to capture the payload for inspection.
- When feeds require headers (e.g., bearer tokens), extend the CI script to add
  `Authorization` headers via `urllib.request.Request`.
- To keep downstream lint jobs in sync with the exact data the exporter used,
  set `SWIFT_PARITY_FEED_EXPORT_PATH` / `SWIFT_CI_FEED_EXPORT_PATH`. The CI wrapper
  copies the resolved feeds to those paths (e.g., `artifacts/swift_status/mobile_parity.json`)
  and uploads them as Buildkite artifacts. Optional
  `SWIFT_PARITY_FEED_PATH_META_KEY` / `SWIFT_CI_FEED_PATH_META_KEY` entries record the
  final filesystem paths via `buildkite-agent meta-data set`, allowing dependent
  steps to discover the files without hardcoding locations. The `swift-dashboards`
  Buildkite step downloads these artifacts, exports `SWIFT_PARITY_FEED` /
  `SWIFT_CI_FEED`, and runs `ci/check_swift_dashboards.sh`, so `make swift-dashboards`
  always validates the live telemetry feeds instead of the static samples.

## Prometheus textfile output

Set the following variables to collect Prometheus counters/gauges alongside the
digest artefacts. The Buildkite lane `.buildkite/swift-status-export.yml` sets
them to the `artifacts/swift_status/` directory and uploads the results at the
end of every run.

| Variable | Purpose |
|----------|---------|
| `SWIFT_STATUS_METRICS_PATH` | Path to the Prometheus textfile emitted by `swift_status_export.py` (e.g., `artifacts/swift_status/swift_status.prom`). |
| `SWIFT_STATUS_METRICS_STATE` | JSON state file that stores the cumulative counters so the next run can pick up where the previous one left off (e.g., `artifacts/swift_status/swift_status_state.json`). |

If the state file does not exist yet, the exporter seeds the counters at zero.
Prime it by copying a previous artifact into place (e.g., download the prior
`swift_status_state.json` before invoking `ci/swift_status_export.sh`) when you
need cumulative counters across jobs. The uploaded `.prom` file is scraped by
the textfile collector and also serves as evidence for parity cadence in release
bundles.

### Swift sample smoke metrics

The IOS5 sample smoke lane now emits its own Prometheus textfile via
`scripts/swift_samples_metrics.py`, which converts `scripts/check_swift_samples.sh`
summaries into the `swift_samples_*` metric family:

- `swift_samples_report_info{cluster, destination, status}`
- `swift_samples_sample_status{cluster, sample, status}`
- `swift_samples_sample_duration_seconds{cluster, sample}`
- `swift_samples_sample_reason_info{cluster, sample, status, reason}` (only when a
  reason is provided)

Wire the export by setting `SWIFT_SAMPLES_METRICS_OUT` before running
`ci/check_swift_samples.sh` (the `.buildkite/swift-samples.yml` job defaults this
to `artifacts/swift_samples/swift_samples.prom`). The helper also honours
`SWIFT_SAMPLES_METRICS_CLUSTER` to tag metrics per environment; it defaults to
`buildkite`.

To surface the data in Grafana/Alertmanager, copy the resulting `.prom` into the
node exporter textfile collector after each run:

```bash
buildkite-agent artifact download artifacts/swift_samples/swift_samples.prom .
install -Dm644 artifacts/swift_samples/swift_samples.prom \
  /var/lib/node_exporter/textfile_collector/swift_samples.prom
```

Operators scraping the Buildkite agent host can instead run the smoke helper
with `SWIFT_SAMPLES_METRICS_OUT=/var/lib/node_exporter/textfile_collector/swift_samples.prom`
so the textfile collector sees it immediately. Downstream dashboards and the
Swift parity live site now plot `swift_samples_sample_duration_seconds` and
alert when durations exceed the target envelopes without parsing Buildkite
annotations.

### Readiness document attachments

The exporter also emits an "IOS8 Readiness Docs" section. By default it references
`docs/source/sdk/swift/reproducibility_checklist.md` and
`docs/source/sdk/swift/support_playbook.md`, hashing each file, recording the
last git commit, and copying the summary text so readiness reviews share the
same evidence bundle as the parity digest. Pass additional documents to
`scripts/swift_status_export.py` with `--readiness-doc <path>` (repeat as
needed) or disable the defaults via `--skip-default-readiness-docs`.

`ci/swift_status_export.sh` wires the same configuration through environment
variables:

| Variable | Purpose |
|----------|---------|
| `SWIFT_STATUS_READINESS_DOCS` | Newline-separated list of extra paths passed to `--readiness-doc`. |
| `SWIFT_STATUS_SKIP_DEFAULT_READINESS_DOCS` | When set (any non-empty value), disables the built-in IOS8 docs so only the custom list is exported. |

Use these knobs when a release needs to cite additional runbooks (e.g., Connect
readiness notes) alongside the standard IOS8 documents.

### Telemetry metadata inputs

`ci/swift_status_export.sh` enriches the parity feed with the redaction telemetry
block. Provide one of the following input sets:

1. **Automatic collection (preferred):**
   - `SWIFT_TELEMETRY_SALT_STATUS` — path to a JSON file with `salt_epoch` and
     `last_rotation` (e.g., exporter artifact or `dashboards/data/swift_salt_status.sample.json`).
   - `SWIFT_TELEMETRY_OVERRIDES_STORE` — path to the override ledger produced by
   - Optional `SWIFT_TELEMETRY_PROFILE_ALIGNMENT`, `SWIFT_TELEMETRY_SCHEMA_VERSION`,
     `SWIFT_TELEMETRY_NOTES_FILE`, and `SWIFT_TELEMETRY_NOTES` (newline-separated string)
     to add alignment status and operator notes.
   - Optional `SWIFT_TELEMETRY_SCHEMA_DIFF_REPORT` pointing at a `telemetry-schema-diff`
     JSON report; policy violations are copied into the telemetry block so the parity dashboard
     flags unreviewed schema diffs.
   The collector (`scripts/swift_collect_redaction_status.py`) runs automatically and
   feeds its output to `scripts/swift_enrich_parity_feed.py`.

2. **Manual overrides (fallback):**
   Provide `SWIFT_TELEMETRY_SALT_EPOCH`, `SWIFT_TELEMETRY_SALT_ROTATION_HOURS`,
   `SWIFT_TELEMETRY_OVERRIDES_OPEN`, and `SWIFT_TELEMETRY_PROFILE_ALIGNMENT`
   directly; the script will inject them without calling the collector.

Use `python3 scripts/swift_status_export.py telemetry-override …` during chaos drills to add/revoke
overrides so the collector reflects real counts. For local dry-runs, point
`SWIFT_TELEMETRY_SALT_STATUS` at the sample file under `dashboards/data/`.

### Pipeline metadata inputs

  `SWIFT_PIPELINE_METADATA` / `MOBILE_PARITY_PIPELINE_METADATA` variables). When
  pulling from a secret store, set `SWIFT_PIPELINE_METADATA_FEED_URL`,
  `SWIFT_PIPELINE_METADATA_FEED_URL_FILE`, or
  `SWIFT_PIPELINE_METADATA_FEED_URL_META_KEY`; the CI helper will download the
  JSON to a scratch directory before enriching the parity feed.
- To share the resolved file with downstream steps, set
  `SWIFT_PIPELINE_METADATA_EXPORT_PATH` (and optionally
  `SWIFT_PIPELINE_METADATA_PATH_META_KEY` for Buildkite meta-data publishing).
- The JSON schema mirrors what the Android parity dashboard expects:
  `{ "job_name": "...", "duration_seconds": 42.5, "tests": [{ "name": "...", "duration_seconds": 12.3 }] }`.
  Additional fields are preserved verbatim inside `pipeline.metadata`.
  Validate locally via `python3 scripts/check_swift_pipeline_metadata.py <file>`.
- The repository ships `dashboards/data/mobile_pipeline_metadata.sample.json`
  as a template; copy it when bootstrapping exporters locally. Publishing jobs
  should keep the canonical copy under something like
  `artifacts/mobile/parity/pipeline_metadata.json` so both SDK dashboards pick
  up the same timing evidence.

## Monitoring & Alerts

- Telemetry exporter should surface scrape metrics (`swift_status_feed_success_total`,
  `swift_status_feed_age_seconds`) so we can alert on stale data.
- Status exporter job will fail hard when feeds are unreachable; Buildkite will
  page the Swift program on-call and include the retry log in artifacts.

## Future Work

- Add signed checksum files for both feeds to detect partial uploads.
- Provide a lightweight `/healthz` endpoint that reports feed freshness for
  observability dashboards.
