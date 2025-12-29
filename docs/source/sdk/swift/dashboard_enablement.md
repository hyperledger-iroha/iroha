<!--
  SPDX-License-Identifier: Apache-2.0
-->

---
title: Swift Dashboard Enablement Runbook
summary: How to feed, render, and publish the Swift parity/CI dashboards with live telemetry.
---

# Swift Dashboard Enablement Runbook

This runbook closes the "Stand up iOS parity/CI dashboards" coordination action
by describing how to feed live telemetry into the Swift dashboards, render the
summaries, and publish evidence bundles for roadmap/status updates.

## Inputs

- **Parity feed:** JSON matching `mobile_parity` (see
  `dashboards/data/mobile_parity.sample.json` for shape).
- **CI feed:** JSON matching `mobile_ci` (see
  `dashboards/data/mobile_ci.sample.json`).
- **Pipeline metadata:** optional job/test timing block consumed by both
  dashboards (`dashboards/data/mobile_pipeline_metadata.sample.json`).
- **Telemetry enrichment:** optional salt/override/profile data injected via
  `scripts/swift_enrich_parity_feed.py` or the status exporter.

## Rendering

Run locally or in CI:

```bash
SWIFT_PARITY_FEED=/path/to/parity.json \
SWIFT_CI_FEED=/path/to/ci.json \
SWIFT_PIPELINE_METADATA_FEED=/path/to/pipeline_metadata.json \
make swift-dashboards
```

This validates schemas, enforces thresholds, and renders summaries via
`scripts/render_swift_dashboards.sh`. CI uses `ci/check_swift_dashboards.sh`,
which wraps the same target with repository defaults.

## Publishing

1. Drop the rendered summaries and the raw feeds into
   `artifacts/swift_dashboards/<date>/`.
2. If Buildkite is available, set `SWIFT_PARITY_FEED_URL` /
   `SWIFT_CI_FEED_URL` to artifact URLs and invoke
   `scripts/swift_status_export.py --parity-url ... --ci-url ... --format markdown`
   to generate the weekly digest snippet for `status.md`.
3. Attach the digest plus raw feeds to the roadmap/status bundle so the gating
   evidence is replayable.

## Telemetry Wiring Notes

- The parity feed should already include regen SLO state and optional salt
  redaction metadata; use `scripts/swift_collect_redaction_status.py` to build
  the telemetry block when exporting from OTLP collectors.
- Buildkite lanes must emit `device_tag` metadata; the CI feed carries the tags
  so dashboards can flag lane-specific regressions.
- Acceleration gating is enabled by default (`SWIFT_PARITY_ACCEL_REQUIRE` and
  `SWIFT_PARITY_ACCEL_REQUIRE_ENABLED`), ensuring Metal/NEON paths stay enabled
  and pass parity before promotion.

## Evidence Checklist

- Latest parity and CI JSON feeds (or URLs) plus rendered summaries.
- Schema validation logs (`make swift-dashboards` output).
- Status exporter digest (Markdown/JSON) attached to `status.md` update.
- Escalations (threshold breach or missing telemetry) logged with owners/dates.
