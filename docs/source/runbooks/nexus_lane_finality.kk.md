---
lang: kk
direction: ltr
source: docs/source/runbooks/nexus_lane_finality.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7d0abf8ecac76db37c539f3214b287840e3db27177cf48699be9f6b9d3b2983a
source_last_modified: "2026-01-07T18:57:11.648998+00:00"
translation_last_reviewed: 2026-02-07
---

# Nexus Lane Finality & Oracle Runbook

**Status:** Active — satisfies the NX-18 dashboard/runbook deliverable.  
**Audience:** Core Consensus WG, SRE/Telemetry, Release Engineering, on-call leads.  
**Scope:** Covers the slot-duration, DA quorum, oracle, and settlement-buffer SLOs that gate the 1 s finality promise. Use this alongside `dashboards/grafana/nexus_lanes.json` and the telemetry helpers under `scripts/telemetry/`.

## Dashboards

- **Grafana (`dashboards/grafana/nexus_lanes.json`)** — publishes the “Nexus Lane Finality & Oracles” board. Panels track:
  - `histogram_quantile()` on `iroha_slot_duration_ms` (p50/p95/p99) plus the latest sample gauge.
  - `iroha_da_quorum_ratio` and `sumeragi_da_gate_block_total{reason="missing_local_data"}` to highlight DA payload recovery lag.
  - Oracle surfaces: `iroha_oracle_price_local_per_xor`, `iroha_oracle_staleness_seconds`, `iroha_oracle_twap_window_seconds`, and `iroha_oracle_haircut_basis_points`.
  - Settlement buffer panel (`iroha_settlement_buffer_xor`) showing live per-lane debits sourced from `LaneBlockCommitment` receipts.
- **Alert rules** — reuse the Slot/DA SLO clauses from `ans3.md`. Page when:
  - slot-duration p95 > 1000 ms for two consecutive 5 m windows,
  - DA quorum ratio < 0.95 or spikes in `sumeragi_da_gate_block_total{reason="missing_local_data"}`,
  - oracle staleness > 90 s or TWAP window ≠ configured 60 s,
  - settlement buffer < 25 % (soft) / 10 % (hard) once the metric is live.

## Metric Cheat Sheet

| Metric | Target / Alert | Notes |
|--------|----------------|-------|
| `histogram_quantile(0.95, iroha_slot_duration_ms)` | ≤ 1000 ms (hard), 950 ms warning | Use the dashboard panel or run `scripts/telemetry/check_slot_duration.py` (`--json-out artifacts/nx18/slot_summary.json`) against the Prometheus export gathered during chaos runs. |
| `iroha_slot_duration_ms_latest` | Mirrors most recent slot; investigate if > 1100 ms even when quantiles look OK. | Export value when filing incidents. |
| `iroha_da_quorum_ratio` | ≥ 0.95 over rolling 30 m window. | Derived from DA availability telemetry during block commits. |
| `sumeragi_da_gate_block_total{reason="missing_local_data"}` | Should remain flat outside chaos rehearsals. | Treat sustained increases as missing local payloads. |
| `lane_relay_emergency_override_total{outcome="applied"}` | Should stay at 0 outside emergency drills. | Non-zero indicates admin override of lane relay validators. Check `outcome="disabled"` if overrides are not configured. |
| `iroha_oracle_staleness_seconds` | ≤ 60 s. Alert at 75 s. | Indicates stale 60 s TWAP feeds. |
| `iroha_oracle_twap_window_seconds` | Exactly 60 s ± 5 s tolerance. | Divergence means the oracle is misconfigured. |
| `iroha_oracle_haircut_basis_points` | Matches lane liquidity tier (0/25/75 bps). | Escalate if haircuts spike unexpectedly. |
| `iroha_settlement_buffer_xor` | Soft 25 %, hard 10 %. Force XOR-only mode below 10 %. | Panel exposes live micro-XOR debits per lane/dataspace; export before adjusting router policy. |

## Response Playbook

### Slot-duration breach
1. Confirm via dashboard + `promql` (p95/p99).  
2. Capture `scripts/telemetry/check_slot_duration.py --json-out <path>` output (and the metrics snapshot) so CXO reviewers can verify the 1 s gate.  
3. Inspect RCA inputs: mempool queue depth, missing-payload counters, IVM traces.  
4. File incident, attach Grafana screenshot, and schedule chaos drill if regression persists.

### DA quorum degradation
1. Check `iroha_da_quorum_ratio` and missing-payload counters; correlate with consensus logs.  
2. If ratio <0.95, pin failing attesters, widen sampling parameters, or move profile to XOR-only mode.  
3. Run `scripts/telemetry/check_nexus_audit_outcome.py` during routed-trace rehearsals to prove `nexus.audit.outcome` events still pass after remediation.  
4. Archive DA receipt bundles with the incident ticket.

### Lane relay quorum recovery
1. Confirm `lane_relay_invalid_total{error="InvalidValidatorSet"}` and
   `lane_relay_emergency_override_total{outcome}` spikes for the affected lane/dataspace.
2. Ensure `nexus.lane_relay_emergency.enabled = true` and the authority is a multisig account
   meeting the configured minimums (default 3-of-5).
3. Prepare an admin multisig transaction using `SetLaneRelayEmergencyValidators`:
   - `dataspace_id` for the impacted dataspace.
   - `validators` list with enough extra accounts to restore `3f+1`.
   - Optional `expires_at_height` (compared against the lane relay envelope `block_height`).
   - Optional `metadata` for audit notes.
4. Submit the multisig as an authority with `CanManagePeers`.
5. Once the lane validator pool recovers, clear the override by re-submitting the instruction
   with an empty `validators` list.

### Oracle staleness / haircut drift
1. Use panels 5–8 to verify price, staleness, TWAP window, and haircut.  
2. For staleness >90 s: restart or fail over the oracle feed, then re-run the chaos harness.  
3. For haircut mismatch: inspect liquidity profile config and recent governance changes; notify treasury if swap lines need intervention.

### Settlement buffer alerts
1. Use `iroha_settlement_buffer_xor` (plus nightly receipts) to confirm headroom before adjusting router policy.  
2. When the metric breaches a threshold, trigger procedures:
   - **Soft breach (<25 %)**: engage treasury, consider drawing swap lines, and log the alert.  
   - **Hard breach (<10 %)**: force XOR-only inclusion, refuse subsidised lanes, and document in `ops/drill-log.md`.  
3. Reference `docs/source/settlement_router.md` for repo/reverse-repo levers.

## Evidence & Automation

- **CI** — wire `scripts/telemetry/check_slot_duration.py --json-out artifacts/nx18/slot_summary.json` and `scripts/telemetry/nx18_acceptance.py --json-out artifacts/nx18/nx18_acceptance.json <metrics.prom>` into the RC acceptance workflow so every release candidate ships the slot-duration summary plus the DA/oracle/buffer gate results alongside the metrics snapshot referenced above. The helper is already invoked from `ci/check_nexus_lane_smoke.sh`.  
- **Dashboard parity** — run `scripts/telemetry/compare_dashboards.py dashboards/grafana/nexus_lanes.json <prod-export.json>` to ensure the published board matches staging/prod exports.  
- **Trace artefacts** — during TRACE rehearsals or NX-18 chaos drills, invoke `scripts/telemetry/check_nexus_audit_outcome.py` to archive the latest `nexus.audit.outcome` payload (`docs/examples/nexus_audit_outcomes/`). Attach both the archive and Grafana screenshots to the drill log.
- **Slot evidence bundling** — after generating the summary JSON, run `scripts/telemetry/bundle_slot_artifacts.py --metrics <prometheus.tgz-extract>/metrics.prom --summary artifacts/nx18/slot_summary.json --out-dir artifacts/nx18` so the resulting `slot_bundle_manifest.json` captures SHA-256 digests for both artefacts. Upload the directory as-is with the RC evidence bundle. The release pipeline executes this automatically (skippable via `--skip-nexus-lane-smoke`) and copies `artifacts/nx18/` into the release output.

## Maintenance Checklist

- Keep `dashboards/grafana/nexus_lanes.json` in sync with Grafana exports after every schema change; document edits in commit messages referencing NX-18.  
- Update this runbook when new metrics (e.g., settlement buffer gauges) or alert thresholds land.  
- Record every chaos rehearsal (slot latency, DA jitter, oracle stall, buffer depletion) with `scripts/telemetry/log_sorafs_drill.sh --log ops/drill-log.md --program NX-18 --status <status>`.

Following this runbook provides the “operator dashboards/runbooks” evidence called out by NX-18 and ensures the finality SLO remains enforceable before Nexus GA.
