---
lang: hy
direction: ltr
source: docs/source/project_tracker/swift_status_export_automation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: db97c3c7094d6546ee4218b5dcb32790def157b93433eba42ffe6a17e6241c01
source_last_modified: "2025-12-29T18:16:36.015498+00:00"
translation_last_reviewed: 2026-02-07
title: Swift Status Export Automation Plan
summary: CI integration plan for automating the Swift weekly status exporter.
---

# Swift Status Export Automation Plan

## Goal

Run `scripts/swift_status_export.py` automatically once telemetry feeds are
published, archive the Markdown summary as a build artifact, and push the output
to the weekly digest location so program leads no longer hand-run the exporter.

## Prerequisites

- Live parity/CI JSON feeds delivered to an internal bucket or artifact store.
- Buildkite secret handling for feed URLs/API tokens.
- `scripts/swift_status_export.py` supports HTTP sources (extend as needed).

## Target CI Flow

1. **Fetch feeds**
   - Download parity and CI JSON from the telemetry exporter (S3 or Torii endpoint).
   - Feeds supplied to the CI script via `SWIFT_PARITY_FEED_URL` / `SWIFT_CI_FEED_URL` (HTTPS or signed S3 URLs). Secrets can be injected as plain environment variables, via `<VAR>_FILE` (path to file containing the secret), or `<VAR>_META_KEY` (Buildkite meta-data key resolved through `buildkite-agent meta-data get`).
   - Verify schema via `python3 scripts/check_swift_dashboard_data.py`.
2. **Render status**
   - Run `python3 scripts/swift_status_export.py --parity <downloaded-parity.json> --ci <downloaded-ci.json> --format markdown` (implemented in `ci/swift_status_export.sh`).
   - Save output to `artifacts/swift_status/weekly_digest.md` and emit a JSON summary to `artifacts/swift_status/summary.json`.
3. **Archive & notify**
   - Upload Markdown and JSON artefacts via `buildkite-agent artifact upload`.
   - If Buildkite metadata is used downstream, expose archive paths with `SWIFT_STATUS_ARCHIVE_META_KEY` (digest) and `SWIFT_STATUS_ARCHIVE_SUMMARY_META_KEY` (JSON summary) so other jobs or annotations can link to the artefacts.
   - Invoke the exporter with `--slack-webhook "$SWIFT_STATUS_SLACK_WEBHOOK"` (supports `_FILE`/`_META_KEY` variants) so it posts the digest headline + preview to `#sdk-parity`. Adjust preview length via `SWIFT_STATUS_SLACK_PREVIEW_LINES` when needed.
4. **Optional: open PR**
   - If configured, open an automated PR updating `docs/source/status/swift_weekly_digest_<date>.md`.

## Timeline

| Milestone | Target | Owner | Notes |
|-----------|--------|-------|-------|
| Prototype CI script (`ci/swift_status_export.sh`) | Feb 2026 Sprint 1 | Swift Program PM + CI Eng | Reuse existing dashboard schema checks (script scaffolded in-tree). |
| Secure feed access & secrets | Feb 2026 Sprint 2 | Telemetry WG | Ensure feeds are accessible to CI job (export URLs via Buildkite secrets). |
| Pilot automation on sample feeds | Feb 2026 Sprint 3 | Swift QA Lead | Validate job output matches manual exporter. |
| Launch automation on live feeds | Mar 2026 Sprint 1 | Swift Program PM | Switch weekly digest generation to CI. |

## Dependencies

- Telemetry exporter job to populate parity/CI feeds.
- Slack webhook for automated notifications.
- Buildkite pipeline addition (`.buildkite/swift-status-export.yml`).
- Meta seeding helper (`scripts/buildkite/swift_status_seed_meta.sh`) for operators who
  need to populate feed/webhook secrets before the job runs.

## Follow-ups

- Extend `scripts/swift_status_export.py` to accept HTTP URLs (`--parity-url`, `--ci-url`). ✔
- Add retry/backoff logic for feed downloads. ✔
- Consider JSON output publishing to tie into status dashboards. *(Completed: JSON summary emitted via `SWIFT_STATUS_SUMMARY_OUT` and published as a Buildkite artifact.)*
- Wire Slack/webhook notifications once the CI job lands. ✔

## Initial Run Checklist

1. Seed Buildkite meta-data with feed URLs and Slack webhook using
   `scripts/buildkite/swift_status_seed_meta.sh --parity-url <url> --ci-url <url> --slack-webhook <url>`
   (optional `--dry-run` for verification).
2. Trigger the pipeline: `buildkite-agent pipeline upload .buildkite/swift-status-export.yml`.
3. Confirm the build uploads `weekly_digest.md`/`summary.json` artefacts and posts the Slack
   digest preview. Rotate secrets or meta-data as telemetry endpoints change.
