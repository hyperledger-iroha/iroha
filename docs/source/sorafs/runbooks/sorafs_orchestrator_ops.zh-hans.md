---
lang: zh-hans
direction: ltr
source: docs/source/sorafs/runbooks/sorafs_orchestrator_ops.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ad5b314217e5c9cec94561315112a27b573b70de6d12274a07972259ebb249ce
source_last_modified: "2026-01-05T09:28:12.079530+00:00"
translation_last_reviewed: 2026-02-07
---

# SoraFS Orchestrator Operations Runbook

This runbook guides SREs through preparing, rolling out, and operating the multi-source fetch orchestrator. It complements the developer guide with procedures tuned for production rollouts, including staged enablement and peer blacklisting.

## 1. Pre-flight checklist

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

## 2. Staged rollout procedure

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

## 3. Peer blacklisting & boosting

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

## 4. Failure triage

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

## 6. Telemetry burn-in checklist

1. **Capture the raw log stream.** Mirror the `telemetry::sorafs.fetch.*` span for every fetch during the burn-in window (NDJSON or the plain-text format shown in `docs/examples/soranet_pq_ratchet_fire_drill.log`). Store the rotated files under `artifacts/sorafs_orchestrator/<stamp>/telemetry/` so reviewers can replay them later.
2. **Run the validator.** Either point
   `ci/check_sorafs_orchestrator_adoption.sh` at the log bundle with
   `SORAFS_BURN_IN_LOGS="<log0> <log1> ..."` (the helper will invoke
   `cargo xtask sorafs-burn-in-check` automatically) or execute the validator manually:
   ```bash
   cargo xtask sorafs-burn-in-check \
     --log artifacts/sorafs_orchestrator/20260201/telemetry/fetch.log \
     --log artifacts/sorafs_orchestrator/20260208/telemetry/fetch.log \
     --window-days 30 \
     --min-pq-ratio 0.95 \
     --max-brownout-ratio 0.01 \
     --max-no-provider-errors 0 \
     --min-fetches 150 \
     --out artifacts/sorafs_orchestrator/20260215/burn_in_summary.json
   ```
   The helper enforces the 30-day window, PQ ratio floor, brownout ceiling, the `no_healthy_providers` ban, and a minimum fetch sample size so the SF-6e SLO is checked automatically (default `--min-fetches` is 150 when invoked via the helper; only lower it with governance approval).
3. **Attach the summary.** Upload the resulting JSON (or capture stdout when `--out` is omitted) to the deployment ticket along with Grafana screenshots. The summary records coverage days, PQ ratios, stall counts, and error tallies so auditors can validate the burn-in without parsing raw logs.
4. **Escalate on failures.** If the command exits with an error, treat it as an active incident: stop the rollout, follow the rollback procedure above, and investigate the offending telemetry before re-running the validator.
5. **Document retention.** Update the on-call checklist with the paths to the latest telemetry logs and summary JSON so the evidence stays discoverable while the burn-in is in progress.

> Automation note: `ci/check_sorafs_orchestrator_adoption.sh` now refuses to write
> `burn_in_note.json`/`burn_in_summary.json` unless `SORAFS_BURN_IN_LABEL`,
> `SORAFS_BURN_IN_REGION`, `SORAFS_BURN_IN_MANIFEST`, `SORAFS_BURN_IN_DAY`, and
> `SORAFS_BURN_IN_LOGS` are populated and the day index falls within
> `SORAFS_BURN_IN_WINDOW_DAYS` (default 30). The helper invokes
> `cargo xtask sorafs-burn-in-check` with the supplied logs, failing fast when the
> PQ ratio, brownout ceiling, `no_healthy_providers` cap, or minimum fetch count
> are violated. Optional overrides (`SORAFS_BURN_IN_MIN_PQ_RATIO`,
> `SORAFS_BURN_IN_MAX_BROWNOUT_RATIO`, `SORAFS_BURN_IN_MAX_NO_PROVIDER_ERRORS`,
> `SORAFS_BURN_IN_MIN_FETCHES`) mirror the CLI flags; only relax them with
> governance approval and record the justification in the rollout ticket.

## 7. Alert response & on-call checklist

SF-6e adds the orchestrator to the primary on-call rotation. When Alertmanager
pages on the SF-6 dashboards, follow the steps below so the telemetry burn-in
remains auditable.

### 7.1 Alert entry criteria

- `dashboards/alerts/sorafs_fetch_rules.yml` defines the `SoraFSFetchFailureBurst`
  / `SoraFSProviderRetrySpike` / `SoraFSChunkLatency*` rules. Keep the
  associated test fixtures in `dashboards/alerts/tests/sorafs_fetch_rules.test.yml`
  green via `promtool test rules ...` whenever the thresholds change.
- `dashboards/alerts/soranet_policy_rules.yml` contains the
  `SoranetAnonymityBrownout` guard used by SNNet-5/6. Run
  `promtool test rules dashboards/alerts/tests/soranet_policy_rules.test.yml`
  after any edit so the PQ transport gate continues to enforce the same budget
  used in `docs/source/sorafs_orchestrator_rollout.md`.
- Alerts must always include the manifest id and region labels so responders can
  tie them back to the `burn_in_summary.json` artefacts collected in §6.

### 7.2 Investigation workflow

1. Acknowledge the alert and capture screenshots/JSON exports from
   `dashboards/grafana/sorafs_fetch_observability.json` (success ratio, PQ
   ratio, provider mix, latency panels). Drop the files in
   `artifacts/sorafs_orchestrator/<stamp>/telemetry/` alongside the burn-in log
   for the day.
2. Pull the latest `scoreboard.json` + `summary.json` from
   `artifacts/sorafs_orchestrator/latest/` (or the SDK bundle that generated the
   fetch) and run
   ```bash
   cargo xtask sorafs-adoption-check \
     --scoreboard artifacts/sorafs_orchestrator/latest/scoreboard.json \
     --summary artifacts/sorafs_orchestrator/latest/summary.json \
     --report artifacts/sorafs_orchestrator/latest/adoption_report.json
   ```
   The report mirrors `provider_count`, `gateway_provider_count`,
   `provider_mix`, and the transport override state so governance reviewers can
   confirm whether the failure stemmed from a direct-mode downgrade.
3. When a burn-in is active, rerun
   `cargo xtask sorafs-burn-in-check --log <latest NDJSON> --window-days 30`.
   A passing summary proves the PQ ratio, brownout ceiling, and error budgets
   stayed within the SF-6e SLOs; attach the JSON to the incident.
4. Collect the Alertmanager payload (or PagerDuty link) and include it with the
   evidence bundle so the notification trail remains auditable.

### 7.3 Rollback & drill reminder

- If the adoption check reports `provider_mix="direct-only"` without the
  explicit override, pause the rollout immediately and run
  `scripts/sorafs_direct_mode_smoke.sh --skip-adoption-check` to validate the
  fallback path before switching production configs.
- Drills should exercise the script weekly even when no alerts fire; log each
  run under `docs/source/sorafs/runbooks/sorafs_orchestrator_ops.md` §6.4 so the
  rollback evidence stays fresh for the SF-6 governance review.
- After every incident or drill, update the on-call changelog with links to the
  captured `scoreboard.json`, `summary.json`, adoption report, Grafana exports,
  and burn-in summaries to keep the SF-6c/SF-6e acceptance trail intact.
