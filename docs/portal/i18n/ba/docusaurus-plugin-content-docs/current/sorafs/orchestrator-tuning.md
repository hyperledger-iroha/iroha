---
id: orchestrator-tuning
lang: ba
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-tuning.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Orchestrator Rollout & Tuning
sidebar_label: Orchestrator Tuning
description: Practical defaults, tuning guidance, and audit checkpoints for taking the multi-source orchestrator to GA.
---

:::note Canonical Source
:::

# Orchestrator Rollout & Tuning Guide

This guide builds on the [configuration reference](orchestrator-config.md) and the
[multi-source rollout runbook](multi-source-rollout.md). It explains
how to tune the orchestrator for each rollout phase, how to interpret the
scoreboard artefacts, and which telemetry signals must be in place before
expanding traffic. Apply the recommendations consistently across CLI, SDKs, and
automation so every node follows the same deterministic fetch policy.

## 1. Baseline Parameter Sets

Start from a shared configuration template and adjust a small set of knobs as
the rollout progresses. The table below captures the recommended values for the
most common phases; values not listed fall back to the defaults in
`OrchestratorConfig::default()` and `FetchOptions::default()`.

| Phase | `max_providers` | `fetch.per_chunk_retry_limit` | `fetch.provider_failure_threshold` | `scoreboard.latency_cap_ms` | `scoreboard.telemetry_grace_secs` | Notes |
|-------|-----------------|-------------------------------|------------------------------------|-----------------------------|------------------------------------|-------|
| **Lab / CI** | `3` | `2` | `2` | `2500` | `300` | Tight latency cap and grace window surface noisy telemetry quickly. Keep retries low to expose invalid manifests sooner. |
| **Staging** | `4` | `3` | `3` | `4000` | `600` | Mirrors production defaults while leaving headroom for exploratory peers. |
| **Canary** | `6` | `3` | `3` | `5000` | `900` | Matches defaults; set `telemetry_region` so dashboards can pivot on canary traffic. |
| **General Availability** | `None` (use all eligible) | `4` | `4` | `5000` | `900` | Increase retry and failure thresholds to absorb transient faults while audits continue to enforce determinism. |

- `scoreboard.weight_scale` remains at the default `10_000` unless a downstream
  system requires a different integer resolution. Increasing the scale does not
  change provider ordering; it only emits a denser credit distribution.
- When migrating between phases, persist the JSON bundle and use
  `--scoreboard-out` so the audit trail records the exact parameter set.

## 2. Scoreboard Hygiene

The scoreboard combines manifest requirements, provider adverts, and telemetry.
Before rolling forward:

1. **Validate telemetry freshness.** Ensure the snapshots referenced by
   `--telemetry-json` were captured within the configured grace window. Entries
   older than the configured `telemetry_grace_secs` fail with
   `TelemetryStale { last_updated }`. Treat this as a hard stop and refresh the
   telemetry export before continuing.
2. **Inspect eligibility reasons.** Persist artefacts via
   `--scoreboard-out=/var/lib/sorafs/scoreboards/preflight.json`. Each entry
   carries an `eligibility` block with the exact failure cause. Do not override
   capability mismatches or expired adverts; fix the upstream payload.
3. **Review weight deltas.** Compare the `normalised_weight` field against the
   previous release. Weight shifts >10 % should correlate with deliberate advert
   or telemetry changes and must be acknowledged in the rollout log.
4. **Archive artefacts.** Configure `scoreboard.persist_path` so every run emits
   the final scoreboard snapshot. Attach the artefact to the release record
   alongside the manifest and telemetry bundle.
5. **Record provider mix evidence.** `scoreboard.json` metadata _and_ the
   matching `summary.json` must expose `provider_count`,
   `gateway_provider_count`, and the derived `provider_mix` label so reviewers
   can prove whether the run was `direct-only`, `gateway-only`, or `mixed`.
   Gateway captures therefore report `provider_count=0` plus
   `provider_mix="gateway-only"`, while mixed runs require non-zero counts for
   both sources. `cargo xtask sorafs-adoption-check` enforces these fields (and
   fails when the counts/labels disagree), so always run it alongside
   `ci/check_sorafs_orchestrator_adoption.sh` or your bespoke capture script to
   produce the `adoption_report.json` evidence bundle. When Torii gateways are
   involved, keep `gateway_manifest_id`/`gateway_manifest_cid` in the scoreboard
   metadata so the adoption gate can correlate the manifest envelope with the
   captured provider mix.

For detailed field definitions see
`crates/sorafs_car/src/scoreboard.rs` and the CLI summary structure exposed by
`sorafs_cli fetch --json-out`.

## CLI & SDK Flag Reference

`sorafs_cli fetch` (see `crates/sorafs_car/src/bin/sorafs_cli.rs`) and the
`iroha_cli app sorafs fetch` wrapper (`crates/iroha_cli/src/commands/sorafs.rs`)
share the same orchestrator configuration surface. Use the following flags when
capturing rollout evidence or replaying the canonical fixtures:

Shared multi-source flag reference (keep CLI help and docs in sync by editing this file only):

- `--max-peers=<count>` limits how many eligible providers survive the scoreboard filter. Leave unset to stream from every eligible provider, set to `1` only when intentionally exercising the single-source fallback. Mirrors the `maxPeers` knob in SDKs (`SorafsGatewayFetchOptions.maxPeers`, `SorafsGatewayFetchOptions.max_peers`).
- `--retry-budget=<count>` forwards to the per-chunk retry limit enforced by `FetchOptions`. Use the rollout table in the tuning guide for recommended values; CLI runs that collect evidence must match SDK defaults to keep parity.
- `--telemetry-region=<label>` tags `sorafs_orchestrator_*` Prometheus series (and OTLP relays) with a region/env label so dashboards can separate lab, staging, canary, and GA traffic.
- `--telemetry-json=<path>` injects the snapshot referenced by the scoreboard. Persist the JSON next to the scoreboard so auditors can replay the run (and so `cargo xtask sorafs-adoption-check --require-telemetry` can prove which OTLP stream fed the capture).
- `--local-proxy-*` (`--local-proxy-mode`, `--local-proxy-norito-spool`, `--local-proxy-kaigi-spool`, `--local-proxy-kaigi-policy`) enable the bridge observer hooks. When set, the orchestrator streams chunks through the local Norito/Kaigi proxy so browser clients, guard caches, and Kaigi rooms receive the same receipts emitted by Rust.
- `--scoreboard-out=<path>` (optionally paired with `--scoreboard-now=<unix_secs>`) persists the eligibility snapshot for auditors. Always pair the persisted JSON with the telemetry and manifest artefacts referenced in the release ticket.
- `--deny-provider name=ALIAS` / `--boost-provider name=ALIAS:delta` apply deterministic adjustments on top of the advert metadata. Use these flags only for rehearsals; production downgrades must flow through governance artefacts so every node applies the same policy bundle.
- `--provider-metrics-out` / `--chunk-receipts-out` retain the per-provider health metrics and chunk receipts referenced by the rollout checklist; attach both artefacts when filing adoption evidence.

Example (using the published fixture):

```bash
sorafs_cli fetch \
  --plan fixtures/sorafs_orchestrator/multi_peer_parity_v1/plan.json \
  --gateway-provider gw-alpha=... \
  --telemetry-source-label otlp::staging \
  --scoreboard-out artifacts/sorafs_orchestrator/latest/scoreboard.json \
  --json-out artifacts/sorafs_orchestrator/latest/summary.json \
  --provider-metrics-out artifacts/sorafs_orchestrator/latest/provider_metrics.json \
  --chunk-receipts-out artifacts/sorafs_orchestrator/latest/chunk_receipts.json

cargo xtask sorafs-adoption-check \
  --scoreboard artifacts/sorafs_orchestrator/latest/scoreboard.json \
  --summary artifacts/sorafs_orchestrator/latest/summary.json
```

SDKs consume the same configuration via `SorafsGatewayFetchOptions` in the Rust
client (`crates/iroha/src/client.rs`), the JS bindings
(`javascript/iroha_js/src/sorafs.js`), and the Swift SDK
(`IrohaSwift/Sources/IrohaSwift/SorafsOptions.swift`). Keep those helpers in
lock-step with the CLI defaults so operators can copy policies across automation
without bespoke translation layers.

## 3. Fetch Policy Tuning

`FetchOptions` controls retry behaviour, concurrency, and verification. When
tuning:

- **Retries:** Raising `per_chunk_retry_limit` beyond `4` increases recovery
  time but risks masking provider faults. Prefer keeping `4` as the ceiling and
  relying on the provider rotation to surface poor performers.
- **Failure threshold:** The `provider_failure_threshold` governs when a
  provider is disabled for the remainder of the session. Align this value with
  retry policy: a threshold lower than the retry budget forces the orchestrator
  to eject a peer before all retries are exhausted.
- **Concurrency:** Leave `global_parallel_limit` unset (`None`) unless a
  specific environment cannot saturate the advertised ranges. When set, ensure
  the value is ≤ the sum of provider stream budgets to avoid starvation.
- **Verification toggles:** `verify_lengths` and `verify_digests` must remain
  enabled in production. They guarantee determinism when mixed provider fleets
  are in play; only disable them in isolated fuzzing environments.

## 4. Transport & Anonymity Staging

Use the `rollout_phase`, `anonymity_policy`, and `transport_policy` fields to
represent the privacy posture:

- Prefer `rollout_phase="snnet-5"` and allow the default anonymity policy to
  track the SNNet-5 milestones. Override via `anonymity_policy_override` only
  when governance issues a signed directive.
- Keep `transport_policy="soranet-first"` as the baseline while SNNet-4/5/5a/5b/6a/7/8/12/13 are 🈺
  (see `roadmap.md`). Use `transport_policy="direct-only"` only for documented
  downgrades/compliance drills, and wait for the PQ coverage review before
  promoting to `transport_policy="soranet-strict"`—that tier will fail fast if
  only classical relays remain.
- `write_mode="pq-only"` should only be enforced when every write path (SDK,
  orchestrator, governance tooling) can satisfy PQ requirements. During
  rollouts keep `write_mode="allow-downgrade"` so emergency responses can rely
  on direct routes while telemetry flags the downgrade.
- Guard selection and circuit staging rely on the SoraNet directory. Supply the
  signed `relay_directory` snapshot and persist the `guard_set` cache so guard
  churn stays within the agreed retention window. The cache fingerprint logged
  by `sorafs_cli fetch` forms part of the rollout evidence.

## 5. Downgrade & Compliance Hooks

Two orchestrator subsystems help enforce policy without manual intervention:

- **Downgrade remediation** (`downgrade_remediation`): monitors
  `handshake_downgrade_total` events and, after the configured `threshold` is
  exceeded within `window_secs`, forces the local proxy into the `target_mode`
  (metadata-only by default). Keep the defaults (`threshold=3`, `window=300`,
  `cooldown=900`) unless incident reviews show a different pattern. Document any
  override in the rollout log and ensure dashboards track
  `sorafs_proxy_downgrade_state`.
- **Compliance policy** (`compliance`): jurisdiction and manifest carve-outs
  flow through governance-managed opt-out lists. Never inline ad-hoc overrides
  in the configuration bundle; instead, request a signed update to
  `governance/compliance/soranet_opt_outs.json` and redeploy the generated JSON.

For both systems, persist the resulting configuration bundle and include it in
release evidences so auditors can trace how downshifts were triggered.

## 6. Telemetry & Dashboards

Before widening the rollout, confirm that the following signals are live in the
target environment:

- `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` —
  should be zero after the canary completes.
- `sorafs_orchestrator_retries_total` and
  `sorafs_orchestrator_retry_ratio` — should stabilise below 10 % during the
  canary and stay below 5 % after GA.
- `sorafs_orchestrator_policy_events_total` — validates that the expected
  rollout stage is active (`stage` label) and records brownouts via `outcome`.
- `sorafs_orchestrator_pq_candidate_ratio` /
  `sorafs_orchestrator_pq_deficit_ratio` — track PQ relay supply against
  policy expectations.
- `telemetry::sorafs.fetch.*` log targets — must be streamed to the shared log
  aggregator with saved searches for `status=failed`.

Load the canonical Grafana dashboard from
`dashboards/grafana/sorafs_fetch_observability.json` (exported in the portal
under **SoraFS → Fetch Observability**) so the region/manifest selectors,
provider retry heatmap, chunk latency histograms, and stall counters match
what SRE reviews during burn-ins. Wire the Alertmanager rules in
`dashboards/alerts/sorafs_fetch_rules.yml` and validate the Prometheus syntax
with `scripts/telemetry/test_sorafs_fetch_alerts.sh` (the helper automatically
runs `promtool test rules` locally or in Docker). Alert hand-offs require the
same routing block that the script prints so operators can pin the evidence to
the rollout ticket.

### Telemetry burn-in workflow

Roadmap item **SF-6e** requires a 30-day telemetry burn-in before flipping the
multi-source orchestrator to its GA defaults. Use the repository scripts to
capture a reproducible artefact bundle for every day in the window:

1. Run `ci/check_sorafs_orchestrator_adoption.sh` with the burn-in environment
   knobs set. Example:

   ```bash
   SORAFS_BURN_IN_LABEL=canary-week-1 \
   SORAFS_BURN_IN_REGION=us-east-1 \
   SORAFS_BURN_IN_MANIFEST=manifest-v4 \
   SORAFS_BURN_IN_DAY=7 \
   SORAFS_BURN_IN_WINDOW_DAYS=30 \
   ci/check_sorafs_orchestrator_adoption.sh
   ```

   The helper replays `fixtures/sorafs_orchestrator/multi_peer_parity_v1`,
   writes `scoreboard.json`, `summary.json`, `provider_metrics.json`,
   `chunk_receipts.json`, and `adoption_report.json` under
   `artifacts/sorafs_orchestrator/<timestamp>/`, and enforces a minimum number
   of eligible providers via `cargo xtask sorafs-adoption-check`.
2. When the burn-in variables are present the script also emits
   `burn_in_note.json`, capturing the label, day index, manifest id, telemetry
   source, and artefact digests. Attach this JSON to the rollout log so it is
   obvious which capture satisfied each day in the 30-day window.
3. Import the updated Grafana board (`dashboards/grafana/sorafs_fetch_observability.json`)
   into the staging/production workspace, tag it with the burn-in label, and
   confirm that every panel shows samples for the manifest/region under test.
4. Run `scripts/telemetry/test_sorafs_fetch_alerts.sh` (or `promtool test rules …`)
   whenever `dashboards/alerts/sorafs_fetch_rules.yml` changes to document that
   alert routing matches the metrics exported during the burn-in.
5. Archive the resulting dashboard snapshot, alert test output, and log tail
   from the `telemetry::sorafs.fetch.*` searches alongside the orchestrator
   artefacts so governance can replay the evidence without pulling metrics from
   live systems.

## 7. Rollout Checklist

1. Regenerate scoreboards in CI using the candidate configuration and capture
   artefacts under version control.
2. Run the deterministic fixture fetch in each environment (lab, staging,
   canary, production) and attach the `--scoreboard-out` and `--json-out`
   artefacts to the rollout record.
3. Review telemetry dashboards with the on-call engineer, ensuring all metrics
   above have live samples.
4. Record the final configuration path (usually via `iroha_config`) and the
   git commit of the governance registry used for adverts and compliance.
5. Update the rollout tracker and notify SDK teams of new defaults so client
   integrations stay aligned.

Following this guide keeps orchestrator deployments deterministic and auditable
while providing clear feedback loops for tuning retry budgets, provider
capacity, and privacy posture.
