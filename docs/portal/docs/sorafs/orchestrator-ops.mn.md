---
lang: mn
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-ops.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9174f3d9559c8656bbe8062e64fc22d93ad654347409798f29231d09ba1628e6
source_last_modified: "2026-01-05T09:28:11.903551+00:00"
translation_last_reviewed: 2026-02-07
id: orchestrator-ops
title: SoraFS Orchestrator Operations Runbook
sidebar_label: Orchestrator Ops Runbook
description: Step-by-step operational guide for rolling out, monitoring, and rolling back the multi-source orchestrator.
---

:::note Canonical Source
:::

This runbook guides SREs through preparing, rolling out, and operating the multi-source fetch orchestrator. It complements the developer guide with procedures tuned for production rollouts, including staged enablement and peer blacklisting.

> **See also:** The [Multi-Source Rollout Runbook](./multi-source-rollout.md) focuses on fleet-wide rollout waves and emergency provider denial. Reference it for governance / staging coordination while using this document for day-to-day orchestrator operations.

## 1. Pre-flight Checklist

1. **Collect provider inputs**
   - Latest provider adverts (`ProviderAdvertV1`) and telemetry snapshot for the target fleet.
   - Payload plan (`plan.json`) derived from the manifest under test.
2. **Render a deterministic scoreboard**

   ```bash
   sorafs_fetch \
     --plan fixtures/plan.json \
     --telemetry-json fixtures/telemetry.json \
     --provider alpha=fixtures/provider-alpha.bin \
     --provider beta=fixtures/provider-beta.bin \
     --provider gamma=fixtures/provider-gamma.bin \
     --scoreboard-out artifacts/scoreboard.json \
     --json-out artifacts/session.summary.json
   ```

   - Validate that `artifacts/scoreboard.json` lists every production provider as `eligible`.
   - Archive the summary JSON alongside the scoreboard; auditors rely on the chunk retry counters when certifying the change request.
3. **Dry-run with fixtures** — Exercise the same command against the public fixtures in `docs/examples/sorafs_ci_sample/` to ensure the orchestrator binary matches the expected version before touching production payloads.

## 2. Staged Rollout Procedure

1. **Canary stage (≤2 providers)**
   - Rebuild the scoreboard and run with `--max-peers=2` to clamp the orchestrator to a small subset.
   - Monitor:
     - `sorafs_orchestrator_active_fetches`
     - `sorafs_orchestrator_fetch_failures_total{reason!="retry"}`
     - `sorafs_orchestrator_retries_total`
   - Proceed once retry rates remain below 1% for a complete manifest fetch and no provider accumulates failures.
2. **Ramp stage (50% providers)**
   - Increase `--max-peers` and rerun with a fresh telemetry snapshot.
   - Persist every run with `--provider-metrics-out` and `--chunk-receipts-out`. Retain the artefacts for ≥7 days.
3. **Full rollout**
   - Remove `--max-peers` (or set it to the full eligible count).
   - Enable orchestrator mode in client deployments: distribute the persisted scoreboard and configuration JSON via your configuration management system.
   - Update dashboards to display `sorafs_orchestrator_fetch_duration_ms` p95/p99 and retry histograms per region.

## 3. Peer Blacklisting & Boosting

Use the CLI’s scoring policy overrides to triage unhealthy providers without waiting for governance updates.

```bash
sorafs_fetch \
  --plan fixtures/plan.json \
  --telemetry-json fixtures/telemetry.json \
  --provider alpha=fixtures/provider-alpha.bin \
  --provider beta=fixtures/provider-beta.bin \
  --provider gamma=fixtures/provider-gamma.bin \
  --deny-provider=beta \
  --boost-provider=gamma=5 \
  --json-out artifacts/override.summary.json
```

- `--deny-provider` removes the listed alias from consideration for the current session.
- `--boost-provider=<alias>=<weight>` raises the provider’s scheduler weight. Values are additive to the normalised scoreboard weight and apply only to the local run.
- Record overrides in the incident ticket and attach the JSON outputs so the owning team can reconcile state once the underlying issue is fixed.

For permanent changes, amend the source telemetry (mark the offender penalised) or refresh the advert with updated stream budgets before clearing the CLI overrides.

## 4. Failure Triage

When a fetch fails:

1. Capture the following artefacts before rerunning:
   - `scoreboard.json`
   - `session.summary.json`
   - `chunk_receipts.json`
   - `provider_metrics.json`
2. Inspect `session.summary.json` for the human-readable error string:
   - `no providers were supplied` → verify provider paths and adverts.
   - `retry budget exhausted ...` → increase `--retry-budget` or remove unstable peers.
   - `no compatible providers available ...` → audit the offending provider’s range capability metadata.
3. Correlate the provider name with `sorafs_orchestrator_provider_failures_total` and create a follow-up ticket if the metric spikes.
4. Replay the fetch offline with `--scoreboard-json` and the captured telemetry to reproduce the failure deterministically.

## 5. Rollback

To revert an orchestrator rollout:

2. Remove any `--boost-provider` overrides so the scoreboard reverts to neutral weighting.
3. Continue scraping the orchestrator metrics for at least one day to confirm no residual fetches are in-flight.

Maintaining disciplined artefact capture and staged rollouts ensures the multi-source orchestrator can be operated safely across heterogeneous provider fleets while keeping observability and audit requirements intact.
