---
lang: zh-hans
direction: ltr
source: docs/source/status/sorafs_da_weekly_digest.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5267f501c8511b7ae9cb80464451149a9403b37c54fac0a8d2b36767efbff77f
source_last_modified: "2025-12-29T18:16:36.212941+00:00"
translation_last_reviewed: 2026-02-07
title: SoraFS Data Availability Weekly Digest Template
summary: Checklist for DA ingestion/replication/PDP-PoTR evidence shared with storage/observability stakeholders.
---

# SoraFS Data Availability Weekly Digest — Week of {{ week_start }}

## Provider & Capacity Snapshot

| Metric | Value | Target | Notes |
|--------|-------|--------|-------|
| Registered providers | {{ providers_total }} | {{ providers_target }} | {{ providers_notes }} |
| `torii_sorafs_capacity_effective_gib` (total) | {{ capacity_effective_gib }} GiB | {{ capacity_commitment_gib }} GiB | {{ capacity_notes }} |
| `torii_sorafs_capacity_utilised_gib` (total) | {{ capacity_used_gib }} GiB | ≤ {{ capacity_threshold_gib }} GiB | {{ utilisation_notes }} |
| `torii_sorafs_capacity_outstanding_gib` (total) | {{ capacity_outstanding_gib }} GiB | ≤ {{ outstanding_cap }} GiB | {{ outstanding_notes }} |

## Ingestion & Replication Health

- **Ingestion latency:** p95 `sorafs_orchestrator_fetch_duration_ms` = {{ ingest_p95_ms }} ms (target ≤ {{ ingest_target_ms }} ms). Spike summary: {{ ingest_notes }}.
- **Chunk latency tail:** 99th percentile `sorafs_orchestrator_chunk_latency_ms` = {{ chunk_p99_ms }} ms. Watchlist providers: {{ chunk_watchlist }}.
- **Replication backlog:** `torii_sorafs_replication_backlog_total` = {{ replication_backlog }} orders; `torii_sorafs_replication_deadline_slack_epochs` median = {{ slack_median_epochs }} epochs. Notes: {{ replication_notes }}.
- **Gateway retries/denials:** `torii_sorafs_gateway_refusals_total` increase (24 h) = {{ gateway_refusals }} / `torii_sorafs_range_fetch_throttle_events_total` = {{ throttle_events }}.

## PDP / PoTR Challenge Outcomes

| Signal | Value (weekly) | Target | Notes |
|--------|----------------|--------|-------|
| `torii_sorafs_proof_stream_events_total{kind="pdp",result="success"}` | {{ pdp_success }} | ≥ {{ pdp_target }} | {{ pdp_notes }} |
| `torii_sorafs_proof_stream_events_total{kind="pdp",result="failure"}` | {{ pdp_fail }} | 0 | {{ pdp_fail_notes }} |
| `torii_sorafs_proof_stream_events_total{kind="potr",result="success"}` | {{ potr_success }} | ≥ {{ potr_target }} | {{ potr_notes }} |
| `torii_sorafs_proof_stream_latency_ms_bucket{kind="pdp",le="60000"}` hit rate | {{ pdp_latency_pct }} % | ≥ {{ pdp_latency_target }} % | {{ pdp_latency_notes }} |

- **Garbage collection:** `torii_sorafs_por_ingest_backlog` max = {{ por_backlog_max }}; `torii_sorafs_por_ingest_failures_total` increase = {{ por_failures }}.

## Retention & SLA Guarantees

| Metric | Value | Target | Notes |
|--------|-------|--------|-------|
| `torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity` (avg) | {{ retention_ratio }} % | {{ retention_target }} % | {{ retention_notes }} |
| `torii_sorafs_replication_sla_total{outcome="met"}` (weekly delta) | {{ sla_met }} | Track trend | {{ sla_notes }} |
| `torii_sorafs_slash_proposals_total` (weekly delta) | {{ slash_delta }} | 0 unless planned | {{ slash_notes }} |

## Alerts & Follow-Ups

- Active alerts from `dashboards/alerts/sorafs_fetch_rules.yml`: {{ fetch_alerts }}
- Active alerts from `dashboards/alerts/sorafs_gateway_rules.yml`: {{ gateway_alerts }}
- Weekly Grafana exports:
  - `dashboards/grafana/sorafs_fetch_observability.json` snapshot: {{ fetch_export_path }}
  - `dashboards/grafana/sorafs_gateway_observability.json` snapshot: {{ gateway_export_path }}

### Action Items

- [ ] Providers breaching latency/SLA thresholds contacted (list + ticket IDs)
- [ ] PDP/PoTR anomaly report filed (link)
- [ ] Capacity reallocation request logged with Storage WG (link)
- [ ] Digest emailed to `sorafs-observability@` with PromQL excerpts and dashboard attachments

## Notes & Decisions

- {{ decision_one }}
- {{ decision_two }}
