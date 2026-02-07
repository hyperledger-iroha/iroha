---
lang: am
direction: ltr
source: docs/portal/docs/sorafs/multi-source-rollout.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ee784b5b073019d219d0dfb76d44f6d02a8dbb46c847bfb226f1f72b41ffb1d7
source_last_modified: "2026-01-05T09:28:11.891621+00:00"
translation_last_reviewed: 2026-02-07
id: multi-source-rollout
title: Multi-Source Client Rollout & Blacklisting Runbook
sidebar_label: Multi-Source Rollout Runbook
description: Operational checklist for staged multi-source rollouts and emergency provider blacklisting.
---

:::note Canonical Source
:::

## Purpose

This runbook guides SRE and on-call engineers through two critical workflows:

1. Rolling out the multi-source orchestrator in controlled waves.
2. Blacklisting or de-prioritising misbehaving providers without destabilising existing sessions.

It assumes the orchestration stack delivered under SF-6 is already deployed (`sorafs_orchestrator`, gateway chunk-range API, telemetry exporters).

> **See also:** The [Orchestrator Operations Runbook](./orchestrator-ops.md) dives into per-run procedures (scoreboard capture, staged rollout toggles, rollback). Use both references together during live changes.

## 1. Pre-flight Validation

1. **Confirm governance inputs.**
   - All candidate providers must publish `ProviderAdvertV1` envelopes with range capability payloads and stream budgets. Validate via `/v1/sorafs/providers` and compare against the expected capability fields.
   - Telemetry snapshots supplying latency/failure rates should be < 15 minutes old before each canary run.
2. **Stage configuration.**
   - Persist the orchestrator JSON config in the layered `iroha_config` tree:

     ```toml
     [torii.sorafs.orchestrator]
     config_path = "/etc/iroha/sorafs/orchestrator.json"
     ```

     Update the JSON with rollout-specific limits (`max_providers`, retry budgets). Feed the same file to staging/production so diffs stay small.
3. **Exercise canonical fixtures.**
   - Populate the manifest/token environment variables and run the deterministic fetch:

     ```bash
     sorafs_cli fetch \
       --plan fixtures/sorafs_manifest/ci_sample/payload.plan.json \
       --manifest-id "$CANARY_MANIFEST_ID" \
       --provider name=alpha,provider-id="$PROVIDER_ALPHA_ID",base-url=https://gw-alpha.example,stream-token="$PROVIDER_ALPHA_TOKEN" \
       --provider name=beta,provider-id="$PROVIDER_BETA_ID",base-url=https://gw-beta.example,stream-token="$PROVIDER_BETA_TOKEN" \
       --provider name=gamma,provider-id="$PROVIDER_GAMMA_ID",base-url=https://gw-gamma.example,stream-token="$PROVIDER_GAMMA_TOKEN" \
       --max-peers=3 \
       --retry-budget=4 \
       --scoreboard-out artifacts/canary.scoreboard.json \
       --json-out artifacts/canary.fetch.json
     ```

     Environment variables should contain the manifest payload digest (hex) and base64-encoded stream tokens for each provider participating in the canary.
   - Diff `artifacts/canary.scoreboard.json` against the previous release. Any new ineligible provider or weight shift >10 % requires review.
4. **Verify telemetry is wired.**
   - Open the Grafana export in `docs/examples/sorafs_fetch_dashboard.json`. Ensure the `sorafs_orchestrator_*` metrics populate in staging before progressing.

## 2. Emergency Provider Blacklisting

Follow this procedure when a provider serves corrupt chunks, times out persistently, or fails compliance checks.

1. **Capture evidence.**
   - Export the latest fetch summary (`--json-out` output). Record failing chunk indices, provider aliases, and digest mismatches.
   - Save relevant log excerpts from `telemetry::sorafs.fetch.*` targets.
2. **Apply an immediate override.**
   - Mark the provider penalised in the telemetry snapshot distributed to the orchestrator (set `penalty=true` or clamp `token_health` to `0`). The next scoreboard build will exclude the provider automatically.
   - For ad-hoc smoke tests, pass `--deny-provider gw-alpha` to `sorafs_cli fetch` so the failure path is exercised without waiting for telemetry propagation.
   - Redeploy the updated telemetry/config bundle to the affected environment (staging → canary → production). Document the change in the incident log.
3. **Validate the override.**
   - Re-run the canonical fixture fetch. Confirm the scoreboard marks the provider as ineligible with reason `policy_denied`.
   - Inspect `sorafs_orchestrator_provider_failures_total` to ensure the counter stops incrementing for the denied provider.
4. **Escalate long-lived bans.**
   - If the provider will remain blocked for >24 h, raise a governance ticket to rotate or suspend its advert. Until the vote passes, keep the deny list in place and refresh telemetry snapshots so the provider does not re-enter the scoreboard.
5. **Rollback protocol.**
   - To reinstate the provider, remove it from the deny list, redeploy, and capture a fresh scoreboard snapshot. Attach the change to the incident postmortem.

## 3. Staged Rollout Plan

| Phase | Scope | Required Signals | Go/No-Go Criteria |
|-------|-------|------------------|-------------------|
| **Lab** | Dedicated integration cluster | Manual CLI fetch against fixture payloads | All chunks succeed, provider failure counters stay at 0, retry rate < 5 %. |
| **Staging** | Full control-plane staging | Grafana dashboard connected; alert rules in warning-only mode | `sorafs_orchestrator_active_fetches` returns to zero after each test run; no `warn/critical` alert firings. |
| **Canary** | ≤10 % of production traffic | Pager muted but telemetry monitored in real time | Retry ratio < 10 %, provider failures isolated to known noisy peers, latency histogram matches staging baseline ±20 %. |
| **General Availability** | 100 % roll-out | Pager rules active | Zero `NoHealthyProviders` errors for 24 h, retry ratio stable, dashboard SLA panels green. |

For each phase:

1. Update the orchestrator JSON with the intended `max_providers` and retry budgets.
2. Run `sorafs_cli fetch` or the SDK integration test suite against the canonical fixture and a representative manifest from the environment.
3. Capture scoreboard + summary artefacts and attach them to the release record.
4. Review telemetry dashboards with the on-call engineer before promoting to the next phase.

## 4. Observability & Incident Hooks

- **Metrics:** Ensure Alertmanager monitors `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` and `sorafs_orchestrator_retries_total`. A sudden spike typically means a provider is degrading under load.
- **Logs:** Route the `telemetry::sorafs.fetch.*` targets to the shared log aggregator. Build saved searches for `event=complete status=failed` to speed up triage.
- **Scoreboards:** Persist every scoreboard artefact to long-term storage. The JSON doubles as the evidence trail for compliance reviews and staged rollbacks.
- **Dashboards:** Clone the canonical Grafana board (`docs/examples/sorafs_fetch_dashboard.json`) into the production folder with alert rules from `docs/examples/sorafs_fetch_alerts.yaml`.

## 5. Communication & Documentation

- Log every deny/boost change in the operations changelog with timestamp, operator, reason, and associated incident.
- Notify SDK teams when provider weights or retry budgets change to align client-side expectations.
- After GA completes, update `status.md` with the rollout summary and archive this runbook reference in the release notes.
