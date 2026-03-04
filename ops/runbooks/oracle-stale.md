---
title: Oracle Staleness & TWAP Drift Runbook (NX-3/NX-18)
summary: Response plan for `iroha_oracle_*` telemetry alerts so the NX-3 settlement router and NX-18 finality SLO stay within policy.
---

## 1. Audience & Scope

- **Applies to:** Nexus Core operators, SRE/Telemetry on-call, Treasury, Governance reviewers.
- **Roadmap links:** NX-3 “Unified lane settlement & XOR conversion” and NX-18 “1 s finality instrumentation.”
- **Objective:** Detect and remediate stale or misconfigured oracle feeds so the settlement router never debits buffers with unverified data and the `/v1/sumeragi/status` commitments remain deterministic.
- **Scenarios covered:** `iroha_oracle_staleness_seconds` or `iroha_oracle_twap_window_seconds` drifting, haircut tiers deviating from liquidity profiles, and router halts triggered by `ORACLE_STALE`.

Use this runbook with `docs/settlement-router.md`, `docs/source/runbooks/nexus_lane_finality.md`, and the Grafana boards under `dashboards/grafana/nexus_lanes.json`.

## 2. Prerequisites & Inputs

| Input | Source | Notes |
|-------|--------|-------|
| Oracle/TWAP dashboard | `dashboards/grafana/nexus_lanes.json` (Oracle panels) | Panel titles “Oracle Price”, “Oracle Staleness”, “Oracle TWAP Window”, “Oracle Haircut”. |
| Alert rules | `dashboards/alerts/nexus_lane_rules.yml` | Pages on `ORACLE_STALE`, `TWAP_WINDOW_DRIFT`, or router halt events tied to oracle health. |
| CLI/SDK status | `iroha_cli sumeragi status --format json`, `/v1/sumeragi/status` | Confirms lane settlement metadata and buffer state before/after remediation. |
| Smoke helper | `scripts/nexus_lane_smoke.py` | `--max-oracle-staleness`, `--expected-oracle-twap`, and `--max-oracle-haircut-bps` enforce the same thresholds locally or in CI. |
| Telemetry reference | `docs/source/telemetry.md` | Defines `iroha_oracle_*` semantics and alert tolerances. |
| Evidence log | `ops/drill-log.md` | Record every alert, intervention, rehearsal, and governance approval. |

## 3. Monitoring Surfaces

| Metric / Signal | SLO / Alert Threshold | Action |
|-----------------|-----------------------|--------|
| `iroha_oracle_staleness_seconds` | Target ≤ 60 s, warn ≥ 75 s, page ≥ 90 s. | ≥ 75 s means the feed stopped pushing samples; ≥ 90 s halts the router (`ORACLE_STALE`). |
| `iroha_oracle_twap_window_seconds` | 60 s ± 5 s tolerance. | Divergence implies config drift; redeploy oracle job with correct window. |
| `iroha_oracle_price_local_per_xor` | Tracks reference TWAP. | Compare against vendor feeds (XOR pair) to detect stale side channels before the router halts. |
| `iroha_oracle_haircut_basis_points` | Matches liquidity tier (0/25/75 bp). | Unexpected value means the oracle downgraded its liquidity profile or the router forced a fallback. |
| `iroha_settlement_buffer_status` | Should remain 0 when the oracle is healthy. | Status 4 indicates the router halted because the oracle SLA failed. |

## 4. Alert & Response Matrix

| Condition | Alert / Error | Immediate Actions | Evidence to Capture |
|-----------|---------------|-------------------|---------------------|
| Staleness warning (≥ 75 s & < 90 s) | `ORACLE_STALE_WARNING` or dashboard warning | Notify feed owner, inspect vendor API health, pre-stage failover. | Grafana panel, PromQL sample, ticket link. |
| Staleness breach (≥ 90 s) | `ORACLE_STALE` + `SETTLEMENT_ROUTER_UNAVAILABLE` | Halt non-XOR debits, fail over or restart the oracle job, leave router in XOR-only until two fresh samples arrive. | Alert ID, PromQL output, Torii status excerpt, restart command. |
| TWAP window drift (>± 5 s) | `TWAP_WINDOW_DRIFT` | Compare running config vs `iroha_config`, redeploy oracle container/service with the approved window, re-run smoke helper. | Config diff, redeploy logs, `scripts/nexus_lane_smoke.py` output. |
| Haircut spike / mismatch | `HAIRCUT_TIER_DRIFT` | Validate liquidity tier + swap-line utilisation, coordinate with Treasury before accepting new haircuts. | Telemetry snapshot + Treasury approval note. |
| Price jump without market move | Manual alert or `ANOMALOUS_ORACLE_PRICE` | Cross-check vendor feeds, enable router circuit breaker if variance > governance epsilon, log incident. | Vendor feed export, router log snippet, governance acknowledgement. |

## 5. Response Procedure

### Step 1 — Confirm the Signal

1. Capture the live metric via PromQL:
   ```bash
   promtool query instant "$PROM" \
     'iroha_oracle_staleness_seconds{lane="payments"}'
   ```
2. Export the Grafana panel for Oracle Staleness/TWAP.
3. Run the smoke helper to double-check thresholds against the same metrics snapshot:
   ```bash
   scripts/nexus_lane_smoke.py \
     --status-url https://torii.example.com/v1/sumeragi/status \
     --metrics-url https://torii.example.com/metrics \
     --lane-alias payments \
     --max-oracle-staleness 75 \
     --expected-oracle-twap 60 \
     --oracle-twap-tolerance 5 \
     --max-oracle-haircut-bps 75
   ```

### Step 2 — Identify the Failing Feed

- Use `iroha_cli sumeragi status --format json` to locate the affected lane/dataspace and confirm whether `lane_settlement_commitments` are paused.
- Inspect the oracle service logs (e.g., `kubectl logs deployment/nexus-oracle-payments` or `journalctl -u iroha-oracle@payments`) for vendor API failures, signature errors, or stalled timers.
- Verify vendor-side health by querying the governance-approved price feed or checking the most recent TWAP snapshot referenced in `docs/source/nexus_settlement_faq.md`.

### Step 3 — Recover or Fail Over

1. **Restart / roll out** the oracle job in the affected environment:
   ```bash
   kubectl -n nexus-oracles rollout restart deployment/nexus-oracle-payments
   # or systemd
   sudo systemctl restart iroha-oracle@payments
   ```
   Update commands as needed for your environment; capture the restart hash.
2. If the primary vendor is down, switch to the standby feed listed in the lane manifest or apply the approved override in `nexus.lane_catalog[].oracle`.
3. Keep the settlement router in XOR-only mode (buffer status 3) until:
   - two consecutive samples show `iroha_oracle_staleness_seconds < 60`,
   - `iroha_oracle_twap_window_seconds` returns to 60 s ± 5 s,
   - `iroha_oracle_haircut_basis_points` matches the lane’s liquidity tier.

### Step 4 — Validate & Clear the Incident

1. Re-run `scripts/nexus_lane_smoke.py` against the live endpoints (or captured metrics) and archive the JSON/Prometheus outputs.
2. Confirm the router resumed normal mode by checking `iroha_settlement_buffer_status` and the `LaneSettlementReceipt` entries under `/v1/sumeragi/status`.
3. Announce recovery in the on-call channel and attach artefacts (dashboards, PromQL output, restart evidence).

### Step 5 — Record Evidence

- Append an entry to `ops/drill-log.md` covering the detection time, root cause, remediation, and links to artefacts.
- Include:
  - Grafana panel export,
  - Prometheus sample,
  - Smoke helper output,
  - Restart or configuration change reference,
  - Governance/Treasury approvals if haircuts changed.

## 6. Maintenance Checklist

- Exercise this runbook during quarterly NX-18 chaos drills (oracle stall scenario) and log the rehearsal in `ops/drill-log.md`.
- Keep `dashboards/grafana/nexus_lanes.json` and `dashboards/alerts/nexus_lane_rules.yml` in sync with telemetry schema updates; document any threshold change referencing the roadmap ID.
- Verify `scripts/nexus_lane_smoke.py` defaults (`--max-oracle-staleness`, `--expected-oracle-twap`, `--max-oracle-haircut-bps`) match the operator SLO each time oracle config changes.
- Review vendor feed contracts and failover procedures alongside Treasury before rotating liquidity tiers or epsilon values referenced in `docs/settlement-router.md`.

Following this runbook delivers the operator documentation called out in NX-3/NX-18 and provides a repeatable process for proving oracle health whenever the settlement router halts on `ORACLE_STALE`.
