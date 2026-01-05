---
title: Data Availability Quorum Runbook (NX-18)
summary: Operator procedure for responding to DA quorum degradation, missing-availability warnings, and attester outages referenced in roadmap item NX-18.
---

## 1. Audience & Scope

- **Applies to:** Sora Nexus SRE, DA program owners, attester operators, and Core Consensus on-call.
- **Roadmap link:** NX-18 “1 s finality instrumentation & acceptance harness” (see `roadmap.md` and the `nexus_lanes` Grafana board).
- **Objective:** Detect and remediate DA quorum drops or excessive missing-availability warnings before finality exceeds the 1 s SLA.

## 2. Prerequisites & Inputs

| Input | Source | Notes |
|-------|--------|-------|
| Telemetry metrics | Grafana board `nexus_lanes` (`dashboards/grafana/nexus_lanes.json`) — panels “DA Quorum Ratio”, “DA Availability Warnings (5 m)”, “Finality Lag (slots)”. | Driven by `iroha_da_quorum_ratio`, `sumeragi_da_gate_block_total{reason="missing_local_data"}`, and `nexus_lane_finality_lag_slots`. |
| Prometheus queries | `iroha_da_quorum_ratio{lane=...,dataspace=...}`, `increase(sumeragi_da_gate_block_total{reason="missing_local_data"}[5m])`, `nexus_lane_attester_health`. | Use `promtool query instant` or Grafana Explore to capture snapshots. |
| Torii/Sumeragi status | `iroha_cli sumeragi status --format json` or `/v1/sumeragi/status`. | Provides `lane_governance[].da_profile`, attester roster, and `lane_settlement_commitments`. |
| DA attester manifests | Space Directory manifest bundle or `scripts/nexus_lane_registry_bundle.sh`. | Confirms which attesters should be active. |
| Evidence log | `ops/drill-log.md`. | Record every alert, mitigation, and verification artefact. |

## 3. Monitoring Surfaces & Thresholds

| Signal | Target | Action |
|--------|--------|--------|
| `iroha_da_quorum_ratio` | ≥0.99 rolling | <0.98 = investigate, <0.95 = engage DA incident protocol. |
| `increase(sumeragi_da_gate_block_total{reason="missing_local_data"}[5m])` | ≈0 | >0.05 warnings/slot indicates attester lag; >0.1 triggers paging. |
| `nexus_lane_finality_lag_slots` | 0–1 | Clamp by halting AMX/settlement for affected lanes when lag ≥3 slots. |
| `nexus_lane_attester_health{status="degraded"}` | 0 | Non-zero means at least one attester is failing heartbeat checks. |
| `iroha_slot_duration_ms_latest` | ≤1000 ms | If slot duration climbs while DA quorum drops, follow both this runbook and the settlement router playbook. |

## 4. Alert Matrix

| Condition | Example Alert | Immediate Actions | Escalation |
|-----------|---------------|-------------------|------------|
| Ratio 0.98–0.95 for >5 min | `DA_QUORUM_WARN` | Verify failing attesters, restart gossip peers, capture telemetry snapshot. | DA program owner within 30 min. |
| Ratio <0.95 or >0.05 warnings/slot | `DA_QUORUM_PAGE` | Freeze risky lanes (set `allow_new=false`), notify attester leads, consider resharding or increasing redundancy. | Page DA TL, inform Core Consensus. |
| Ratio <0.90 or lag ≥3 slots | `DA_FINALITY_AT_RISK` | Halt affected dataspace, trigger attester failover, engage governance for temporary policy change. | Bridge call with DA TL + Core Consensus within 15 min. |

## 5. Procedure

### Step 1 — Confirm the Signal

1. Export the Grafana panels (PNG + JSON) showing the ratio and missing-availability traces.
2. Capture Prometheus samples:
   ```bash
   promtool query instant "$PROM" \
     'iroha_da_quorum_ratio{lane="lane-A",dataspace="profile.defi.v1"}'
   promtool query range "$PROM" --start "$START" --end "$END" --step 30s \
     'increase(sumeragi_da_gate_block_total{reason="missing_local_data",lane="lane-A"}[5m])'
   ```
3. Dump the relevant section of `sumeragi status`:
   ```bash
   iroha_cli sumeragi status --format json |
     jq '.lane_governance[] | select(.lane_id=="lane-A")'
   ```

### Step 2 — Identify the Failure Mode

- Compare `lane_governance[].da_profile` with the manifest bundle to ensure the expected attester set is active.
- Query `nexus_lane_attester_health` for `status="degraded"` to isolate specific attester IDs; cross-reference with gossip/attester logs.
- If reschedules correlate with a specific cluster, inspect network telemetry (`torii_p2p_latency_ms`, `nexus_lane_link_latency_ms`).

### Step 3 — Mitigate

1. **Restart or replace unhealthy attesters.** Coordinate with the attester operator to restart the process or move the slot assignment. Record the attester ID and action.
2. **Adjust redundancy.** Temporarily raise the attester count for the affected profile (e.g., profile B from 9→11) via the governance manifest workflow if quorum keeps failing.
3. **Throttle workloads.** For lanes exceeding lag ≥3 slots, set `allow_new=false` on AMX admissions via `iroha_cli governance lane update --lane lane-A --allow-new false` until quorum stabilises.
4. **Rebuild gossip peers.** Flush `/var/lib/iroha/dag` mirror, restart peer, and ensure `nexus_lane_block_height` catches up.

### Step 4 — Verify Recovery

1. Wait for at least two consecutive `iroha_da_quorum_ratio` samples ≥0.99.
2. Confirm `increase(sumeragi_rbc_da_reschedule_total[5m])` returns to ≈0.
3. Check `nexus_lane_finality_lag_slots` ≤1 and `iroha_slot_duration_ms_latest` ≤1000 ms.
4. Capture before/after dashboards and attach to the incident ticket.

### Step 5 — Document

- Append an entry to `ops/drill-log.md` summarising:
  - Timeframe and affected lane/dataspace.
  - Metrics/graphs collected.
  - Attester actions (restart, failover, manifest change).
  - Governance approvals or CLI commands issued.
- Link the evidence bundle (Grafana exports, Prometheus JSON, `sumeragi status`, attester logs).

## 6. Maintenance

- Rehearse this runbook quarterly alongside the NX-18 chaos harness; note improvements in `ops/drill-log.md`.
- Keep attester contact lists and failover scripts current; outdated metadata invalidates the SLA.
- When telemetry names change (e.g., new histogram buckets), update the queries above and the Grafana dashboard to stay aligned with `dashboards/grafana/nexus_lanes.json`.
- Ensure this document matches the acceptance criteria referenced in `docs/settlement-router.md` and the roadmap.

Executing this runbook closes the NX-18 deliverable for “DA quorum runbook with deterministic evidence capture” and keeps operations aligned with the roadmap mandates.
