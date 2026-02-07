---
lang: dz
direction: ltr
source: docs/portal/docs/sorafs/taikai-monitoring-dashboards.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f5f8db3cc2a4255f29c4a196c14f7c14dcdf9019a0dc6e6a55ba4a9037815e58
source_last_modified: "2025-12-29T18:16:35.205443+00:00"
translation_last_reviewed: 2026-02-07
title: Taikai Monitoring Dashboards
description: Portal summary of the viewer/cache Grafana boards that back SN13-C evidence
---

Taikai routing-manifest (TRM) readiness hinges on two Grafana boards and their
companion alerts. This page mirrors the highlights from
`dashboards/grafana/taikai_viewer.json`, `dashboards/grafana/taikai_cache.json`,
and `dashboards/alerts/taikai_viewer_rules.yml` so reviewers can follow along
without cloning the repository.

## Viewer dashboard (`taikai_viewer.json`)

- **Live edge & latency:** Panels visualise the p95/p99 latency histograms
  (`taikai_ingest_segment_latency_ms`, `taikai_ingest_live_edge_drift_ms`) per
  cluster/stream. Watch for p99 > 900 ms or drift > 1.5 s (triggers the
  `TaikaiLiveEdgeDrift` alert).
- **Segment errors:** Breaks out `taikai_ingest_segment_errors_total{reason}` to
  expose decode failures, lineage replay attempts, or manifest mismatches.
  Attach screenshots to SN13-C incidents whenever this panel rises above the
  “warning” band.
- **Viewer & CEK health:** Panels sourced from `taikai_viewer_*` metrics track
  CEK rotation age, PQ guard mix, rebuffer counts, and alert roll-ups. The CEK
  panel enforces the rotation SLA that governance reviews before approving new
  aliases.
- **Alias telemetry snapshot:** The `/status → telemetry.taikai_alias_rotations`
  table sits directly on the board so operators can confirm manifest digests
  before attaching governance evidence.

## Cache dashboard (`taikai_cache.json`)

- **Tier pressure:** Panels chart `sorafs_taikai_cache_{hot,warm,cold}_occupancy`
  and `sorafs_taikai_cache_promotions_total`. Use these to see whether a TRM
  rotation is overloading specific tiers.
- **QoS denials:** `sorafs_taikai_qos_denied_total` surfaces when cache pressure
  forces throttling; annotate the drill log whenever the rate departs from zero.
- **Egress utilisation:** Helps confirm that SoraFS exits keep up with Taikai
  viewers when CMAF windows rotate.

## Alerts & evidence capture

- Paging rules live in `dashboards/alerts/taikai_viewer_rules.yml` and map one
  to one with the panels above (`TaikaiLiveEdgeDrift`, `TaikaiIngestFailure`,
  `TaikaiCekRotationLag`, proof-health warnings). Ensure every production
  cluster wires these into Alertmanager.
- Snapshots/screenshots captured during drills must be stored in
  `artifacts/sorafs_taikai/drills/<YYYYMMDD>/` alongside the spool files and
  `/status` JSON. Use `scripts/telemetry/log_sorafs_drill.sh --scenario taikai-anchor`
  to append the execution to the shared drill log.
- When dashboards change, include the SHA-256 digest of the JSON file inside the
  portal PR description so auditors can match the managed Grafana folder to the
  repo version.

## Evidence bundle checklist

SN13-C reviews expect every drill or incident to ship the same artefacts listed
in the Taikai anchor runbook. Capture them in the order below so the bundle is
ready for governance review:

1. Copy the most recent `taikai-anchor-request-*.json`,
   `taikai-trm-state-*.json`, and `taikai-lineage-*.json` files from
   `config.da_ingest.manifest_store_dir/taikai/`. These spool artefacts prove
   which routing manifest (TRM) and lineage window were active. The helper
   `cargo xtask taikai-anchor-bundle --spool <dir> --copy-dir <out> --out <out>/anchor_bundle.json [--signing-key <ed25519>]`
   will copy the spool files, emit hashes, and optionally sign the summary.
2. Record `/v1/status` output filtered to
   `.telemetry.taikai_alias_rotations[]` and store it next to the spool files.
   Reviewers compare the reported `manifest_digest_hex` and window bounds with
   the copied spool state.
3. Export Prometheus snapshots for the metrics listed above and take screenshots
   of the viewer/cache dashboards with the relevant cluster/stream filters in
   view. Drop the raw JSON/CSV and the screenshots into the artefact folder.
4. Include Alertmanager incident IDs (if any) that reference the rules from
   `dashboards/alerts/taikai_viewer_rules.yml` and note whether they auto-closed
   once the condition cleared.

Store everything under `artifacts/sorafs_taikai/drills/<YYYYMMDD>/` so drill
audits and SN13-C governance reviews can fetch a single archive.

## Drill cadence & logging

- Run the Taikai anchor drill on the first Tuesday of every month at 15:00 UTC.
  The schedule keeps evidence fresh ahead of the SN13 governance sync.
- After capturing the artefacts above, append the execution to the shared ledger
  with `scripts/telemetry/log_sorafs_drill.sh --scenario taikai-anchor`. The
  helper emits the JSON entry required by `docs/source/sorafs/runbooks-index.md`.
- Link the archived artefacts in the runbook index entry and escalate any failed
  alerts or dashboard regressions within 48 hours via the Media Platform WG/SRE
  channel.
- Keep the drill summary screenshot set (latency, drift, errors, CEK rotation,
  cache pressure) alongside the spool bundle so operators can show exactly how
  the dashboards behaved during the rehearsal.

Refer back to the [Taikai Anchor Runbook](./taikai-anchor-runbook.md) for the
full Sev 1 procedure and evidence checklist. This page only captures the
dashboard-specific guidance that SN13-C requires before leaving 🈺.
