---
lang: uz
direction: ltr
source: docs/portal/docs/soranet/pq-ratchet-runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 370733f000ffa7022cab18931c2697031a225d2ce3ae83382896c0d61f9fe6e2
source_last_modified: "2026-01-05T09:28:11.912569+00:00"
translation_last_reviewed: 2026-02-07
id: pq-ratchet-runbook
title: SoraNet PQ Ratchet Fire Drill
sidebar_label: PQ Ratchet Runbook
description: On-call rehearsal steps for promoting or demoting the staged PQ anonymity policy with deterministic telemetry validation.
---

:::note Canonical Source
:::

## Purpose

This runbook guides the fire-drill sequence for SoraNet's staged post-quantum (PQ) anonymity policy. Operators rehearse both promotion (Stage A -> Stage B -> Stage C) and controlled demotion back to Stage B/A when PQ supply drops. The drill validates telemetry hooks (`sorafs_orchestrator_policy_events_total`, `sorafs_orchestrator_brownouts_total`, `sorafs_orchestrator_pq_ratio_*`) and collects artefacts for the incident rehearsal log.

## Prerequisites

- Latest `sorafs_orchestrator` binary with capability-weighting (commit at or after the drill reference shown in `docs/source/soranet/reports/pq_ratchet_validation.md`).
- Access to the Prometheus/Grafana stack serving `dashboards/grafana/soranet_pq_ratchet.json`.
- Nominal guard directory snapshot. Fetch and verify a copy prior to the drill:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./artefacts/guard_directory_pre_drill.norito \
  --expected-directory-hash <directory-hash-hex>
```

If the source directory only publishes JSON, re-encode it to Norito binary with `soranet-directory build` before running the rotation helpers.

- Capture metadata and pre-stage issuer rotation artefacts with the CLI:

```bash
soranet-directory inspect \
  --snapshot ./artefacts/guard_directory_pre_drill.norito
soranet-directory rotate \
  --snapshot ./artefacts/guard_directory_pre_drill.norito \
  --out ./artefacts/guard_directory_post_drill.norito \
  --keys-out ./artefacts/guard_issuer_rotation --overwrite
```

- Change window approved by networking and observability on-call teams.

## Promotion steps

1. **Stage audit**

   Record the starting stage:

   ```bash
   sorafs_cli config get --config orchestrator.json sorafs.anonymity_policy
   ```

   Expect `anon-guard-pq` before promotion.

2. **Promote to Stage B (Majority PQ)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   - Wait >=5 minutes for manifests to refresh.
   - In Grafana (`SoraNet PQ Ratchet Drill` dashboard) confirm the "Policy Events" panel shows `outcome=met` for `stage=anon-majority-pq`.
   - Capture a screenshot or panel JSON and attach it to the incident log.

3. **Promote to Stage C (Strict PQ)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-strict-pq
   ```

   - Verify `sorafs_orchestrator_pq_ratio_*` histograms trend to 1.0.
   - Confirm the brownout counter remains flat; otherwise follow the demotion steps.

## Demotion / brownout drill

1. **Induce a synthetic PQ shortage**

   Disable PQ relays in the playground environment by trimming the guard directory to classical entries only, then reload the orchestrator cache:

   ```bash
   sorafs_cli guard-cache prune --config orchestrator.json --keep-classical-only
   ```

2. **Observe brownout telemetry**

   - Dashboard: panel "Brownout Rate" spikes above 0.
   - PromQL: `sum(rate(sorafs_orchestrator_brownouts_total{region="$region"}[5m]))`
   - `sorafs_fetch` should report `anonymity_outcome="brownout"` with `anonymity_reason="missing_majority_pq"`.

3. **Demote to Stage B / Stage A**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   If PQ supply is still insufficient, demote to `anon-guard-pq`. The drill completes once brownout counters settle and promotions can be reapplied.

4. **Restore guard directory**

   ```bash
   sorafs_cli guard-directory import \
     --config orchestrator.json \
     --input ./artefacts/guard_directory_pre_drill.json
   ```

## Telemetry & artefacts

- **Dashboard:** `dashboards/grafana/soranet_pq_ratchet.json`
- **Prometheus alerts:** ensure `sorafs_orchestrator_policy_events_total` brownout alert stays below the configured SLO (&lt;5% across any 10 minute window).
- **Incident log:** append the captured telemetry snippets and operator notes to `docs/examples/soranet_pq_ratchet_fire_drill.log`.
- **Signed capture:** use `cargo xtask soranet-rollout-capture` to copy the drill log and scoreboard into `artifacts/soranet_pq_rollout/<timestamp>/`, compute BLAKE3 digests, and produce a signed `rollout_capture.json`.

Example:

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

If the drill uncovers real PQ shortages, remain on Stage A, notify the Networking TL, and attach the collected metrics plus guard directory diffs to the incident tracker. Use the guard directory export captured earlier to restore normal service.

:::tip Regression Coverage
`cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics` provides the synthetic validation backing this drill.
:::
