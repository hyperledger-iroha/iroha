# Swift Dashboard Assets

This directory contains the tooling and sample data used by the Swift SDK
parity/CI dashboards that feed the roadmap and `status.md`.

## Layout

- `mobile_parity.swift` — CLI renderer that consumes a Norito parity JSON feed, including
  optional acceleration parity/perf data (`acceleration` object).
- `mobile_ci.swift` — CLI renderer for Buildkite/CI telemetry.
- `mobile_pipeline_metadata.swift` — CLI renderer for the shared pipeline metadata feed
  that parity exporters publish for other SDKs (job/test duration summary).
- `data/mobile_parity.sample.json` — Sample parity feed showcasing the expected
  schema when exporters are wired.
- `data/mobile_ci.sample.json` — Sample CI feed with Buildkite lane metrics and
  device coverage plus optional acceleration benchmarks (`acceleration_bench`).
- `data/mobile_pipeline_metadata.sample.json` — Sample shared pipeline metadata blob
  referenced by both Android and Swift parity exporters.
- `grafana/fastpq_acceleration.json` — Grafana board that visualises FASTPQ Metal
  adoption using `fastpq_execution_mode_total` plus the latest benchmark bundle
  reference.
- `grafana/settlement_router_overview.json` — Grafana board for the Nexus settlement
  router metrics (buffer health, swap-line utilisation, oracle feeds, DA quorum).
- `alerts/fastpq_acceleration_rules.yml` + `alerts/tests/fastpq_acceleration_rules.test.yml`
  — Alerting pack and promtool coverage for Metal downgrades/fallback bursts.

The JSON structure for these dashboards is documented in
`docs/source/references/ios_metrics.md`. Exporters should populate real feeds in
`dashboards/data/` (see `.gitignore` for the ignore rules) and re-use the scripts
to emit summaries for weekly reports or ad-hoc checks.

## Rendering Both Dashboards

Use the helper script at `scripts/render_swift_dashboards.sh` to render the parity,
CI, and pipeline metadata summaries in one command. Without arguments it falls
back to the sample data:

```bash
scripts/render_swift_dashboards.sh
```

Pass explicit JSON payloads to inspect production feeds (third argument optional when the
pipeline feed lives alongside the parity data):

```bash
# Render all three feeds:
scripts/render_swift_dashboards.sh /path/to/parity.json /path/to/ci.json /path/to/pipeline_metadata.json

# Environment variables also work (the third value defaults to dashboards/data/mobile_pipeline_metadata.sample.json):
#   export SWIFT_PARITY_FEED=/tmp/parity.json
#   export SWIFT_CI_FEED=/tmp/ci.json
#   export SWIFT_PIPELINE_METADATA_FEED=/tmp/pipeline_metadata.json
#   scripts/render_swift_dashboards.sh

# Capture summaries to files while still printing to stdout:
#   export SWIFT_DASHBOARD_OUTPUT_DIR=/tmp/swift-dashboards
#   scripts/render_swift_dashboards.sh
#   # files are written to /tmp/swift-dashboards/mobile_parity.txt, mobile_ci.txt, and mobile_pipeline_metadata.txt
```

Validate feed structure before wiring exporters:

```bash
scripts/check_swift_dashboard_data.py dashboards/data/mobile_parity.sample.json dashboards/data/mobile_ci.sample.json
# or run both validation and render steps:
make swift-dashboards
# CI pipelines can call `ci/check_swift_dashboards.sh` to reuse the same steps.

# Validate against the JSON schema explicitly (optional):
python3 -m jsonschema --output pretty docs/source/references/ios_metrics.schema.json dashboards/data/mobile_parity.sample.json
python3 -m jsonschema --output pretty docs/source/references/ios_metrics.schema.json dashboards/data/mobile_ci.sample.json
```

### Shared Pipeline Metadata Feed

Swift and Android parity exporters share a compact pipeline metadata feed (`dashboards/data/mobile_pipeline_metadata.sample.json`)
capturing the latest Buildkite job timings. Validate the structure via:

```bash
python3 scripts/check_swift_pipeline_metadata.py dashboards/data/mobile_pipeline_metadata.sample.json
```

Render human-readable summaries with:

```bash
swift dashboards/mobile_pipeline_metadata.swift dashboards/data/mobile_pipeline_metadata.sample.json
```

### SLA Enforcement

`make swift-dashboards` now fails if Swift parity/CI metrics drift past the roadmap
targets:

- Norito diffs must be younger than 14 days (`SWIFT_PARITY_MAX_OLDEST_HOURS=336`).
- Fixture regeneration streak cannot exceed 48h (`SWIFT_PARITY_MAX_REGEN_HOURS=48`) and
  any `regen_sla.breach` flag triggers a failure.
- `ci/xcode-swift-parity` must stay above a 95% success rate over the last 14 runs
  (`SWIFT_CI_PARITY_MIN_SUCCESS=0.95`).
- CI consecutive failures are capped at one before the script fails
  (`SWIFT_CI_MAX_CONSEC_FAIL=1`).

Override these defaults by exporting the associated environment variables before
invoking the Make target or `ci/check_swift_dashboards.sh`.

## Next Steps

- Wire telemetry exporters so the sample JSON files are replaced with live
  feeds produced by CI.
- Add automated publishing (e.g., upload to a dashboard service or commit
  renderer output) once the exporters are in place.
