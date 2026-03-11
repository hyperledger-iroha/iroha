---
title: Settlement Buffer & Circuit Breaker Runbook (NX-3)
summary: Response plan for the unified settlement router buffer thresholds and circuit-breaker actions described in roadmap item NX-3.
---

## 1. Audience & Scope

- **Applies to:** Sora Nexus lane owners, Treasury/SRE on-call, Economics WG, Release Engineering.
- **Roadmap link:** NX-3 “Unified lane settlement & XOR conversion” (see `docs/settlement-router.md`).
- **Objective:** Keep the per-lane XOR buffer within policy, execute swap-line or treasury top-ups, and handle router circuit breakers without violating determinism.

## 2. Prerequisites & Inputs

| Input | Source | Notes |
|-------|--------|-------|
| Telemetry metrics | Grafana `nexus_lanes` board (`dashboards/grafana/nexus_lanes.json`) + Prometheus (`iroha_settlement_buffer_*`, `iroha_oracle_*`, `iroha_settlement_pnl_xor`, `iroha_settlement_swapline_utilisation`, `iroha_settlement_schedule_lag_ms`). | Panels `Settlement Buffer Headroom`, `Oracle/Price`, `Swap-line Utilisation`, and `TWAMM Schedule Lag` back the SLOs. |
| Alerting | Alertmanager templates `AddressLocal8Resurgence`, `AddressInvalidRatioSlo` (reuse channels) + forthcoming settlement router rules (track `NX-3` ticket). | Page on buffer status ≥2 or `CIRCUIT_BREAKER_ACTIVE`. |
| Torii status snapshots | `iroha_cli sumeragi status --format json` or `/v1/sumeragi/status`. | Provides `lane_governance[].settlement.buffer` and `lane_settlement_commitments`. |
| Router spec | `docs/settlement-router.md`. | Authoritative description of buffer states, error codes, and telemetry. |
| Automation | `iroha address normalize`, `iroha_cli transaction submit` (or equivalent SDK automation), swap-line tooling owned by Treasury. | Required when moving XOR into reserve accounts. |
| Evidence log | `ops/drill-log.md`. | Record every alert, refill, or circuit-breaker action. |

## 3. Monitoring Surfaces

| Metric / Signal | Target | Action when breaching |
|-----------------|--------|-----------------------|
| `iroha_settlement_buffer_status` (0=normal, 1=alert, 2=throttle, 3=XOR-only, 4=halt) | 0 | ≥1 requires remediation per §4. |
| `iroha_settlement_buffer_xor` vs `iroha_settlement_buffer_capacity_xor` | ≥75 % (alert), ≥25 % (throttle), ≥10 % (XOR-only) | Calculates headroom; export before and after top-ups. |
| `iroha_settlement_pnl_xor` | Stable over 24 h | Spikes imply haircut misconfiguration. |
| `iroha_oracle_staleness_seconds`, `iroha_oracle_price_local_per_xor`, `iroha_oracle_haircut_basis_points` | Staleness ≤60 s, TWAP window 60 s, haircut matches tier | Required before clearing halt state. |
| `iroha_settlement_swapline_utilisation` | <80 % per sponsor | Escalate to Treasury when swap lines >80 % before buffer reaches throttle. |
| `iroha_settlement_schedule_lag_ms` | <500 ms | Large lag + circuit breaker implies DEX/AMM degradation. |

## 4. Alert & Response Matrix

| Condition | Error / Alert | Immediate Actions | Evidence |
|-----------|---------------|-------------------|----------|
| Buffer <75 % but ≥25 % | `SETTLEMENT_ROUTER_UNAVAILABLE` (warning) | Notify lane owner & Treasury. Queue sponsor/MM refill, monitor swap-line utilisation. | Grafana screenshot, Prometheus query, `sumeragi status` excerpt. |
| Buffer <25 % but ≥10 % | `SETTLEMENT_ROUTER_UNAVAILABLE` (throttle) | Reduce subsidised inclusion by 50 %, prioritise XOR-paying lanes, trigger sponsor credit transfer, log ticket. | Router log excerpt showing throttles, Alertmanager ID, refill plan. |
| Buffer <10 % but ≥2 % | `BUFFER_DEPLETED_XOR_ONLY` | Force XOR-only inclusion, pause AMX debt legs, execute top-up immediately. | Before/after telemetry, transfer hash, ops log entry. |
| Buffer <2 % **or** oracle stale >90 s | `ORACLE_STALE` + halt | Halt conversions, block non-XOR tx, engage oracle owners, require two consecutive fresh TWAP windows before resuming. | Oracle logs, screenshot of staleness metric, approval from Governance. |
| AMM illiquidity (tier 3 ≥15 min) | `CIRCUIT_BREAKER_ACTIVE` | Slow TWAMM slices to 25 %, open DEX ticket, consider RFQ swap-line. Stay in XOR-only if buffers <25 %. | TWAMM schedule log, DEX owner acknowledgement. |

## 5. Response Procedure

### Step 1 — Confirm the Signal

1. Capture Prometheus samples:
   ```bash
   promtool query instant http://prometheus/api/v1/query \
     'iroha_settlement_buffer_status{lane="lane-A"}'
   ```
2. Export the Grafana panel (PNG/JSON) from `nexus_lanes` for the affected lane.
3. Run `iroha_cli sumeragi status --format json > status.json` and isolate the lane metadata:
   ```bash
   jq '.lane_governance[] | select(.lane_id=="lane-A") | .settlement' status.json
   ```

### Step 2 — Choose Remediation Path

- **Soft alert (<75 %)**: Coordinate sponsor/MM transfer before swap lines exceed 80 %.
- **Throttle (<25 %)**: Switch the affected dataspace to reduced inclusion (e.g., lower batch size, disable subsidised AMX) and schedule refill within 1 h.
- **XOR-only (<10 %)**: Immediately refuse non-XOR fees for the lane and fast-track refill (≤30 min SLA).
- **Halt (<2 % or oracle stale)**: Freeze conversions, run oracle remediation (restart feeds, fail over signer), obtain governance approval before re-enabling.

### Step 3 — Execute Sponsor/MM Top-Up

1. Identify the reserve from `metadata.settlement.buffer_account` / `buffer_asset` (via the `lane_governance` payload).
2. Determine refill amount:  
   `needed = (capacity * target_ratio) - current_headroom`, where `target_ratio` defaults to 0.9 (90 %).
3. Issue a deterministic `TransferAsset` using your approved tooling (for example, submit a signed envelope via `iroha_cli transaction submit --file refill.json` or trigger the sponsor automation pipeline). Always include metadata describing the lane, reason, and ticket ID.
4. Record the transfer hash, ledger height, and sponsoring entity in the incident ticket.
5. Re-check `iroha_settlement_buffer_xor` and confirm the status dropped back to `0`.

### Step 4 — Treasury Swap-Line / TWAMM Intervention

If sponsor balances are insufficient or AMM depth is degraded:

1. Notify Treasury (Slack `#nexus-treasury`) with current utilisation from `iroha_settlement_swapline_utilisation`.
2. Engage the canonical AMM/CLMM operator to verify pool depth; if < tier 2, allow the router to stay in pacing mode and document the expected recovery timeline.
3. For RFQ fallback, update the lane manifest entry (or apply a temporary override) and note the change in `ops/drill-log.md`.

### Step 5 — Circuit Breaker Handling

1. Confirm breaker reason from router logs (`CIRCUIT_BREAKER_ACTIVE`, `AMM_DEPTH_DEGRADED`, etc.).
2. Ensure XOR-only mode is active while breaker is engaged.
3. Work with DEX governance to restore liquidity; capture `lane_settlement_commitments` snapshots before and after the breaker clears.
4. Once two consecutive TWAMM slices execute with canonical-I105 receipts and lag <500 ms, clear the incident and document approvals.

### Step 6 — Logging & Evidence

- Append an entry to `ops/drill-log.md` covering time, lane, action, and artefacts.
- Attach:
  - Grafana panel export.
  - Prometheus query output.
  - `sumeragi status` excerpt (`lane_governance` + `lane_settlement_commitments`).
  - Transfer hash or governance proposal ID.
  - Alertmanager notification ID.

## 6. Maintenance Checklist

- Validate this runbook during every quarterly TRACE drill; log updates in `ops/drill-log.md`.
- When metrics or alert rules change, update both this document and `docs/settlement-router.md`.
- Keep CLI snippets aligned with the current `iroha_cli` interface (verify via `iroha_cli --help` after upgrades).
- Ensure swap-line owner roster stays current; stale contacts invalidate the SLA.

Following this runbook satisfies the NX-3 documentation requirement for “operator runbooks covering buffer management and circuit-breaker responses” and keeps settlement router evidence in sync with the roadmap deliverable.
