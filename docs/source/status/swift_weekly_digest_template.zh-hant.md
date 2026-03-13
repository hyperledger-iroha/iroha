---
lang: zh-hant
direction: ltr
source: docs/source/status/swift_weekly_digest_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cd6642ba877e3f6732a8488141c6252a65e2b08f31a243c7c70c48fbb7717739
source_last_modified: "2025-12-29T18:16:36.216040+00:00"
translation_last_reviewed: 2026-02-07
title: Swift SDK Weekly Digest Template
summary: Template for the weekly Swift parity/CI status export.
---

# Swift SDK Weekly Digest — Week of {{ week_start }}

Run `python3 scripts/swift_status_export.py --parity <parity-feed.json> --ci <ci-feed.json>`
(or use `--parity-url`/`--ci-url` for remote feeds) to generate the Markdown snippet for
**Metrics Snapshot** below. The command defaults to the sample feeds checked into
`dashboards/data/`.

## Metrics Snapshot

{{ swift_metrics_snippet }}

> `scripts/swift_status_export.py` now emits a **Fixture Drift Summary** table
> plus the CI Signals block automatically; rerun the exporter whenever parity
> feeds update so downstream readers get the refreshed snapshot.

## Highlights

- {{ highlight_one }}
- {{ highlight_two }}

## Risks & Mitigations

| Risk | Owner | Status | Notes |
|------|-------|--------|-------|
| {{ risk_one }} | {{ owner_one }} | {{ status_one }} | {{ notes_one }} |
| {{ risk_two }} | {{ owner_two }} | {{ status_two }} | {{ notes_two }} |

## Governance Watchers

 Summarise the `/v2/pipeline` rollout status, telemetry readiness, and
 governance checkpoints that the council tracks each week. Keep entries short
 and link to `status.md` sections or RFCs for deeper context.

| Item | Owner | Status | Notes |
|------|-------|--------|-------|
| `/v2/pipeline` adoption | {{ pipeline_owner }} | {{ pipeline_status }} | {{ pipeline_notes }} |
| Telemetry readiness (`connect.queue_*`) | {{ telemetry_owner }} | {{ telemetry_status }} | {{ telemetry_notes }} |
| Governance vote / readiness review | {{ vote_owner }} | {{ vote_status }} | {{ vote_notes }} |
| Risk owner spotlight | {{ risk_owner }} | {{ risk_status }} | {{ risk_notes }} |

## Upcoming Actions

1. {{ action_one }}
2. {{ action_two }}

## Links & Artefacts

- Parity feed: {{ parity_feed_url }}
- CI feed: {{ ci_feed_url }}
- Dashboards: `dashboards/mobile_parity.swift`, `dashboards/mobile_ci.swift`
