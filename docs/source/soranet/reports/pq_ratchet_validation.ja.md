---
lang: ja
direction: ltr
source: docs/source/soranet/reports/pq_ratchet_validation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c0b5c8d8a1de32ca9c5e30dd3a677ab1831c44937c4fa4e4595be1f353941620
source_last_modified: "2026-01-03T18:08:01.718667+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: SoraNet PQ Ratchet Validation
summary: Fire-drill record for staged post-quantum promotion/demotion, telemetry evidence, and operator artefacts.
---

## Overview

The February 2026 drill exercised the full post-quantum ratchet:

1. Stage A → Stage B promotion (Guard PQ → Majority PQ).  
2. Stage B → Stage C promotion (Majority PQ → Strict PQ).  
3. Controlled brownout demotions back to Stage B/A with synthetic PQ scarcity.  

Instrumentation now captures each transition through
`sorafs_orchestrator_policy_events_total` and the dedicated brownout counter added
in this release.

## Promotion / Demotion Outcomes

| Step | Requested Stage | Effective Stage | Status | Evidence |
|------|-----------------|-----------------|--------|----------|
| 1 | `anon-guard-pq` | `anon-guard-pq` | ✅ `met` | Grafana panel “Policy Events” (SoraNet PQ Ratchet Drill) |
| 2 | `anon-majority-pq` | `anon-majority-pq` | ✅ `met` | `sorafs_orchestrator_pq_ratio` mean ≥ 0.75 |
| 3 | `anon-strict-pq` | `anon-strict-pq` | ✅ `met` | `pq_ratio` = 1.0; no brownouts recorded |
| 4 | `anon-majority-pq` | `anon-majority-pq` | ⚠️ `brownout` (`missing_majority_pq`) | Brownout counter +1, CLI telemetry log |
| 5 | `anon-strict-pq` | `anon-majority-pq` | ⚠️ `brownout` (`missing_majority_pq`) | Brownout counter +1, fallback recorded |

## Telemetry Artefacts

- **Dashboard**: `dashboards/grafana/soranet_pq_ratchet.json`  
  Panels track policy event rate, brownout rate, and PQ ratio mean.
- **PromQL snippets**:
  ```promql
  sum by (stage, outcome) (
    rate(sorafs_orchestrator_policy_events_total{region="$region"}[5m])
  )
  sum(rate(sorafs_orchestrator_brownouts_total{region="$region"}[5m]))
  sum(rate(sorafs_orchestrator_pq_ratio_sum{region="$region"}[5m]))
    / sum(rate(sorafs_orchestrator_pq_ratio_count{region="$region"}[5m]))
  ```
- **Incident log**: `docs/examples/soranet_pq_ratchet_fire_drill.log`

## Automation

```
cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics
```

The unit test drives deterministic promotion/demotion runs across all three
stages, asserts telemetry counters/histograms, and backs the drill evidence.

## Runbook

Operational steps, rollback guidance, and guard-directory handling live in
`docs/source/soranet/pq_ratchet_runbook.md`.
