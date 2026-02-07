---
lang: az
direction: ltr
source: docs/source/soranet/pq_ratchet_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0a63ec9d7172237bc45c6386daae7999ad8cf51d0d76a20214e88e96d73d4093
source_last_modified: "2025-12-29T18:16:36.191874+00:00"
translation_last_reviewed: 2026-02-07
title: SoraNet PQ Ratchet Fire Drill Runbook
summary: On-call rehearsal steps for promoting and demoting the staged PQ anonymity policy with deterministic telemetry validation.
---

## Purpose

This runbook guides the fire-drill sequence for SoraNet’s staged post-quantum
anonymity policy. Operators rehearse both promotion (Stage A → Stage B → Stage C)
and controlled demotion back to Stage B/A when PQ supply drops. The drill
validates telemetry hooks (`sorafs_orchestrator_policy_events_total`,
`sorafs_orchestrator_brownouts_total`, `sorafs_orchestrator_pq_ratio_*`) and
collects artefacts for the incident rehearsal log.

## Prerequisites

- Latest `sorafs_orchestrator` binary with capability-weighting (commit ≥ the
  drill reference shown in `docs/source/soranet/reports/pq_ratchet_validation.md`).
- Access to the Prometheus/Grafana stack shipping
  `dashboards/grafana/soranet_pq_ratchet.json`.
- Nominal guard directory snapshot. Export a copy prior to the drill:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./artefacts/guard_directory_pre_drill.norito \
  --expected-directory-hash <directory-hash-hex>
```
  The command verifies every SRCv2 bundle before persisting the snapshot so the
  drill starts from a trusted baseline. If the directory only publishes JSON,
  re-encode it to the Norito binary format with `soranet-directory build`
  before running the rotation helpers.

- Capture the metadata and pre-stage issuer rotation artefacts with the CLI:

```bash
soranet-directory inspect \
  --snapshot ./artefacts/guard_directory_pre_drill.norito
soranet-directory rotate \
  --snapshot ./artefacts/guard_directory_pre_drill.norito \
  --out ./artefacts/guard_directory_post_drill.norito \
  --keys-out ./artefacts/guard_issuer_rotation --overwrite
```

- Change window approved by networking & observability on-call.

## Promotion Steps

1. **Stage Audit**  
   Record the starting stage:
   ```bash
   sorafs_cli config get --config orchestrator.json sorafs.anonymity_policy
   ```
   Expect `anon-guard-pq` before promotion.

2. **Promote to Stage B (Majority PQ)**  
   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```
   - Wait ≥5 minutes for manifests to refresh.
   - On Grafana: dashboard `SoraNet PQ Ratchet Drill` → confirm the
     “Policy Events” panel shows `outcome=met` for `stage=anon-majority-pq`.
   - Capture screenshot or panel JSON and attach to incident log.

3. **Promote to Stage C (Strict PQ)**  
   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-strict-pq
   ```
   - Verify `sorafs_orchestrator_pq_ratio_*` histograms trend to 1.0.
   - Confirm brownout counter remains flat; otherwise follow demotion steps.

## Demotion / Brownout Drill

1. **Induce Synthetic PQ Shortage**  
   Disable PQ relays in the playground environment by trimming the guard
   directory to classical entries only, then reload orchestrator cache:
   ```bash
   sorafs_cli guard-cache prune --config orchestrator.json --keep-classical-only
   ```

2. **Observe Brownout Telemetry**  
   - Dashboard: panel “Brownout Rate” spikes >0.
   - `promql`:  
     `sum(rate(sorafs_orchestrator_brownouts_total{region="$region"}[5m]))`
   - Fetch^1 should report `anonymity_outcome="brownout"` with
     `anonymity_reason="missing_majority_pq"`.

3. **Demote to Stage B / Stage A**  
   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```
   If PQ supply is still insufficient, demote to `anon-guard-pq`. The drill
   completes once brownout counters settle and promotions can be reapplied.

4. **Restore Guard Directory**  
   ```bash
   sorafs_cli guard-directory import \
     --config orchestrator.json \
     --input ./artefacts/guard_directory_pre_drill.json
   ```

## Telemetry & Artefacts

- **Dashboard**: `dashboards/grafana/soranet_pq_ratchet.json`
- **Prometheus Alerts**: Ensure `sorafs_orchestrator_policy_events_total` brownout alert stays below configured SLO (<5% / 10 min).
- **Incident Log**: Append the captured telemetry snippets and operator notes to `docs/examples/soranet_pq_ratchet_fire_drill.log`.
- **Signed Capture**: Use `cargo xtask soranet-rollout-capture` to copy the drill log and scoreboard into
  `artifacts/soranet_pq_rollout/<timestamp>/`, compute BLAKE3 digests, and produce a signed
  `rollout_capture.json`. Example:
  ```
  cargo xtask soranet-rollout-capture \
    --log logs/pq_fire_drill.log \
    --artifact kind=scoreboard,path=artifacts/canary.scoreboard.json \
    --artifact kind=fetch-summary,path=artifacts/canary.fetch.json \
    --key secrets/pq_rollout_signing_ed25519.hex \
    --phase ramp \
    --label "drill-2026-02-21"
  ```
  Attach the generated metadata and signature to the governance packet.

## Rollback

If the drill uncovers real PQ shortages, remain on Stage A, notify the
Networking TL, and attach the collected metrics plus guard directory diffs to
the incident tracker. Use the guard directory export captured earlier to
restore normal service.

---
1. `cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics` provides the synthetic validation backing this drill.
