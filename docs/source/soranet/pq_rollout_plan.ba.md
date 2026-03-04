---
lang: ba
direction: ltr
source: docs/source/soranet/pq_rollout_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 008611a98157ab0ea7ec44c52b12339e16424fc5155d378dedaff5314d78f778
source_last_modified: "2026-01-22T14:45:02.226420+00:00"
translation_last_reviewed: 2026-02-07
title: SNNet-16G Post-Quantum Rollout Playbook
summary: Operational guide for promoting the SoraNet hybrid X25519+ML-KEM handshake from canary to default across relays, clients, and SDKs.
---

SNNet-16G finishes the post-quantum rollout for the SoraNet transport. The new
`rollout_phase` knobs let operators coordinate a deterministic promotion from
the existing Stage A guard requirement to Stage B majority coverage and the
Stage C strict PQ posture without editing raw JSON/TOML for every surface.

This playbook covers:

- Phase definitions and the new configuration knobs (`sorafs.gateway.rollout_phase`,
  `sorafs.rollout_phase`) wired in the codebase (`crates/iroha_config/src/parameters/actual.rs:2230`,
  `crates/iroha/src/config/user.rs:251`).
- SDK/CLI flag mapping so every client can track the rollout.
- Relay/client canary scheduling expectations and the governance dashboard entry
  points that gate promotion (`dashboards/grafana/soranet_pq_ratchet.json`).
- Rollback hooks and references to the fire-drill runbook
  (`docs/source/soranet/pq_ratchet_runbook.md`).

## Phase Map

| `rollout_phase` | Effective anonymity stage | Default effect | Typical usage |
|-----------------|---------------------------|----------------|----------------|
| `canary`        | `anon-guard-pq` (Stage A) | Require at least one PQ guard per circuit while the fleet warms up. | Baseline and early canary weeks. |
| `ramp`          | `anon-majority-pq` (Stage B) | Bias selection toward PQ relays for ≥⅔ coverage; classical relays remain as fallbacks. | Region-by-region relay canaries; SDK preview toggles. |
| `default`       | `anon-strict-pq` (Stage C) | Enforce PQ-only circuits and tighten downgrade alarms. | Final promotion once telemetry and governance sign-off complete. |

If a surface also sets an explicit `anonymity_policy`, it overrides the phase
for that component. Omitting the explicit stage now defers to the `rollout_phase`
value so operators can flip the phase once per environment and let clients
inherit it.

## Configuration Reference

### Orchestrator (`sorafs_gateway`)

```toml
[sorafs.gateway]
# Promote to Stage B (majority-PQ) canary
rollout_phase = "ramp"
# Optional: force a specific stage independent of the phase
# anonymity_policy = "anon-majority-pq"
```

The orchestrator loader resolves the fallback stage at runtime
(`crates/sorafs_orchestrator/src/lib.rs:2229`) and surfaces it in telemetry via
`sorafs_orchestrator_policy_events_total` and `sorafs_orchestrator_pq_ratio_*`.
See `docs/examples/sorafs_rollout_stage_b.toml` and
`docs/examples/sorafs_rollout_stage_c.toml` for ready-to-apply snippets.

### Rust client / `iroha_cli`

```toml
[sorafs]
# Keep clients aligned with orchestrator promotion cadence
rollout_phase = "default"
# anonymity_policy = "anon-strict-pq"  # optional explicit override
```

`iroha::Client` now records the parsed phase (`crates/iroha/src/client.rs:2315`)
so helper commands (e.g., `iroha_cli app sorafs fetch`) can report the current phase
alongside the default anonymity policy.

## Automation

Two `cargo xtask` helpers automate the schedule generation and artefact
capture:

1. **Generate the regional schedule**

   ```bash
   cargo xtask soranet-rollout-plan \
     --regions us-east,eu-west,apac \
     --start 2026-04-01T00:00:00Z \
     --window 6h \
     --spacing 24h \
     --client-offset 8h \
     --phase ramp \
     --environment production
   ```

   Durations accept `s`, `m`, `h`, or `d` suffixes. The command emits
   `artifacts/soranet_pq_rollout_plan.json` and a Markdown summary
   (`artifacts/soranet_pq_rollout_plan.md`) that can be shipped with the change
   request.

2. **Capture drill artefacts with signatures**

   ```bash
   cargo xtask soranet-rollout-capture \
     --log logs/pq_fire_drill.log \
     --artifact kind=scoreboard,path=artifacts/canary.scoreboard.json \
     --artifact kind=fetch-summary,path=artifacts/canary.fetch.json \
     --key secrets/pq_rollout_signing_ed25519.hex \
     --phase ramp \
     --label "beta-canary" \
     --note "Relay canary – APAC first"
   ```

   The command copies the supplied files into
   `artifacts/soranet_pq_rollout/<timestamp>_<label>/`, computes BLAKE3 digests
   for each artefact, and writes `rollout_capture.json` containing the metadata
   plus an Ed25519 signature over the payload. Use the same private key that
   signs the fire-drill minutes so governance can validate the capture quickly.

## SDK & CLI Flag Matrix

| Surface | Canary (Stage A) | Ramp (Stage B) | Default (Stage C) |
|---------|------------------|----------------|-------------------|
| `sorafs_cli` fetch | `--anonymity-policy stage-a` or rely on phase | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Orchestrator config JSON (`sorafs.gateway.rollout_phase`) | `canary` | `ramp` | `default` |
| Rust client config (`iroha.toml`) | `rollout_phase = "canary"` (default) | `rollout_phase = "ramp"` | `rollout_phase = "default"` |
| `iroha_cli` signed commands | `--anonymity-policy stage-a` | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Java/Android `GatewayFetchOptions` | `setRolloutPhase("canary")`, optionally `setAnonymityPolicy(AnonymityPolicy.ANON_GUARD_PQ)` | `setRolloutPhase("ramp")`, optionally `.ANON_MAJORIY_PQ` | `setRolloutPhase("default")`, optionally `.ANON_STRICT_PQ` |
| JavaScript orchestrator helpers | `rolloutPhase: "canary"` or `anonymityPolicy: "anon-guard-pq"` | `"ramp"` / `"anon-majority-pq"` | `"default"` / `"anon-strict-pq"` |
| Python `fetch_manifest` | `rollout_phase="canary"` | `"ramp"` | `"default"` |
| Swift `SorafsGatewayFetchOptions` | `anonymityPolicy: "anon-guard-pq"` | `"anon-majority-pq"` | `"anon-strict-pq"` |

All SDK toggles map to the same stage parser used by the orchestrator
(`crates/sorafs_orchestrator/src/lib.rs:365`), so mixed-language deployments stay
in lock-step with the configured phase.

## Canary Scheduling Checklist

1. **Preflight (T‑2 weeks)**  
   - Confirm Stage A brownout rate <1 % over the previous fortnight and PQ
     coverage ≥70 % per region (`sorafs_orchestrator_pq_candidate_ratio`).
   - Schedule the governance review slot that approves the canary window.
   - Update `sorafs.gateway.rollout_phase = "ramp"` in staging (edit the orchestrator JSON and redeploy) and dry-run the promotion pipeline.

2. **Relay canary (T‑0)**  
   - Promote one region at a time by setting `rollout_phase = "ramp"` on the
     orchestrator and the participating relay manifests.
   - Monitor `Policy Events per Outcome` and `Brownout Rate` in the PQ Ratchet
     dashboard (now featuring the rollout panel) for 2× the guard cache TTL.
   - Cut `sorafs_cli guard-directory fetch` snapshots before and after the run
     for audit storage.

3. **Client/SDK canary (T+1 week)**  
   - Flip `rollout_phase = "ramp"` in client configs or pass `stage-b` overrides
     for the designated SDK cohorts.
   - Capture telemetry diffs (`sorafs_orchestrator_policy_events_total` grouped by
     `client_id` and `region`) and attach them to the rollout incident log.

4. **Default promotion (T+3 weeks)**  
   - Once governance signs off, switch both orchestrator and client configs to
     `rollout_phase = "default"` and rotate the signed readiness checklist into
     the release artefacts.

### Regional promotion template

| Wave | Relay action | Client action | Evidence & uploads | Notes |
|------|--------------|---------------|--------------------|-------|
| Wave 1 — APAC (T0–T+48 h) | Flip the APAC relay manifests plus the gateway orchestrator to `rollout_phase = "ramp"` and pin the `stage-b` anonymity override for the designated PoPs. Run `cargo xtask soranet-rollout-plan --regions apac --phase ramp --start <UTC> --window 6h` so the artefact bundle records the launch window. | Keep clients inheriting the `canary` phase while the relay fleet burns in; only the APAC SDK preview cohort receives `stage-b` overrides. | `artifacts/soranet_pq_rollout/<stamp>_apac/rollout_plan.{json,md}`, paired `sorafs_cli guard-directory fetch` snapshots, and a `cargo xtask soranet-rollout-capture --label apac-relay` bundle. | Use this wave to validate Argon2 ticket cost and guard cache churn before onboarding end users. |
| Wave 2 — EU/MEA (T+48 h→T+96 h) | Promote EU/MEA relays to `ramp` once the APAC telemetry stays green. Re-run `soranet-rollout-plan` with `--regions apac,emea` so the Markdown summary documents the completed wave and pending regions. | Enable the EU/MEA SDK cohorts by setting `rollout_phase = "ramp"` in their configs or by passing `stage-b` overrides to the Android/Swift/JS SDK helpers. | Capture per-region policy metrics (`sorafs_orchestrator_policy_events_total{region="emea"}`) and drop a signed `soranet-rollout-capture --label emea-client` bundle alongside governance minutes. | This wave unlocks brownout-free Stage B coverage in two continents, proving the majority-PQ policy before final promotion. |
| Wave 3 — Americas (T+96 h→T+144 h) | Switch the remaining relays to `ramp` (or directly to `default` if governance approves) and regenerate the rollout plan so all three regions appear in the schedule artefact. | Promote the remaining client cohorts to the next phase and stage the `rollout_phase = "default"` configs that will ship once the burn-in completes. | Signed `soranet-rollout-capture --label americas` bundle (telemetry screenshots + promtool output), plus the guard-directory diffs collected immediately before the Stage C promotion vote. | Treat this wave as the dress rehearsal for the fleet-wide flip; verify that Alertmanager remains silent while the downgrade panels stay flat. |

- Keep every schedule artefact under `artifacts/soranet_pq_rollout_plan.{json,md}`
  (one per promotion) so governance can replay the cadence.
- Each wave should reuse the same `cargo xtask soranet-rollout-capture` key to
  sign evidence bundles, keeping the audit trail consistent with the fire-drill
  runbook referenced in the governance checklist below.

## Governance & Evidence Checklist

| Phase change | Promotion gate | Evidence bundle | Dashboards & alerts |
|--------------|----------------|-----------------|---------------------|
| Canary → Ramp *(Stage B preview)* | Stage-A brownout rate <1 % over the trailing 14 days, `sorafs_orchestrator_pq_candidate_ratio` ≥0.7 per promoted region, Argon2 ticket verify p95 <50 ms, and the governance slot for the promotion booked. | `cargo xtask soranet-rollout-plan` JSON/Markdown pair, paired `sorafs_cli guard-directory fetch` snapshots (before/after), signed `cargo xtask soranet-rollout-capture --label canary` bundle, and canary minutes referencing `docs/source/soranet/pq_ratchet_runbook.md`. | `dashboards/grafana/soranet_pq_ratchet.json` (Policy Events + Brownout Rate), `dashboards/grafana/soranet_privacy_metrics.json` (SN16 downgrade ratio), telemetry references in `docs/source/soranet/snnet16_telemetry_plan.md`. |
| Ramp → Default *(Stage C enforcement)* | 30-day SN16 telemetry burn-in met, `sn16_handshake_downgrade_total` flat at baseline, `sorafs_orchestrator_brownouts_total` zero during client canary, and the proxy toggle rehearsal logged. | `sorafs_cli proxy set-mode --mode gateway|direct` transcript, `promtool test rules dashboards/alerts/soranet_handshake_rules.yml` output, `sorafs_cli guard-directory verify` log, and a signed `cargo xtask soranet-rollout-capture --label default` bundle. | Same PQ Ratchet board plus the SN16 downgrade panels documented in `docs/source/sorafs_orchestrator_rollout.md` and `dashboards/grafana/soranet_privacy_metrics.json`. |
| Emergency demotion / rollback readiness | Triggered when downgrade counters spike, guard-directory verification fails, or the `/policy/proxy-toggle` buffer records sustained downgrade events. | Checklist from `docs/source/ops/soranet_transport_rollback.md`, `sorafs_cli guard-directory import` / `guard-cache prune` logs, `cargo xtask soranet-rollout-capture --label rollback`, incident tickets, and notification templates. | `dashboards/grafana/soranet_pq_ratchet.json`, `dashboards/grafana/soranet_privacy_metrics.json`, and both alert packs (`dashboards/alerts/soranet_handshake_rules.yml`, `dashboards/alerts/soranet_privacy_rules.yml`). |

- Store every artefact under `artifacts/soranet_pq_rollout/<timestamp>_<label>/` with
  the generated `rollout_capture.json` so governance packets contain the
  scoreboard, promtool traces, and digests.
- Attach SHA256 digests of uploaded evidence (minutes PDF, capture bundle, guard
  snapshots) to the promotion minutes so Parliament approvals can be replayed
  without access to the staging cluster.
- Reference the telemetry plan in the promotion ticket to prove
  `docs/source/soranet/snnet16_telemetry_plan.md` remains the canonical source
  for downgrade vocabularies and alert thresholds.

## Dashboard & Telemetry Updates

`dashboards/grafana/soranet_pq_ratchet.json` now ships with a “Rollout Plan”
annotation panel that links back to this playbook and surfaces the current
phase so governance reviews can confirm which stage is active. Keep the panel
description in sync with future changes to the config knobs.

For alerting, ensure existing rules use the `stage` label so the canary and
default phases trigger separate policy thresholds (`dashboards/alerts/soranet_handshake_rules.yml`).

### Governance dashboard checklist

| Asset | Panels / rules | Data source | Owner | Evidence requirement |
|-------|----------------|-------------|-------|----------------------|
| `dashboards/grafana/soranet_pq_ratchet.json` | “Rollout Plan” annotation, “Policy Events per Outcome”, “Brownout Rate” | `sorafs_orchestrator_policy_events_total`, `sorafs_orchestrator_brownouts_total`, `sorafs_orchestrator_pq_ratio_*` | Observability TL | Attach screenshots of the three panels plus the active annotation ID to each `soranet-rollout-capture` bundle; include the Grafana share link in the promotion minutes. |
| `dashboards/grafana/soranet_privacy_metrics.json` | “SN16 Downgrade Ratio (5m)”, “Proxy Toggle Health”, “Padding Budget” | `sn16_handshake_*`, `soranet_padding_*`, `/policy/proxy-toggle` synth sources documented in `docs/source/soranet/snnet16_telemetry_plan.md` | Privacy WG + SRE | Export the panel JSON via Grafana’s snapshot tool and drop it in `artifacts/soranet_pq_rollout/<stamp>_*/dashboard_snapshot.json` so downgrade evidence accompanies every promotion. |
| `dashboards/alerts/soranet_handshake_rules.yml`, `dashboards/alerts/soranet_privacy_rules.yml` | AlertManager rules for downgrade spikes, brownouts, and padding saturation | Same metrics as above plus Argon2 summaries (`sn16_puzzle_verify_seconds_*`) | SRE On-call | Capture `promtool test rules` output and include it in the signed rollout capture (for canary/ramp/default transitions) to prove alert coverage before and after each phase change. |

- When exporting dashboards use the same timestamp as the region’s capture
  label so the JSON snapshot hash matches the bundle manifest.
- Ticket descriptions should cite the exact panel IDs/UIDs, making it trivial
  for governance reviewers to cross-reference the evidence with the live board.
- Keep the vocabulary in sync with `docs/source/soranet/snnet16_telemetry_plan.md`
  so future telemetry updates do not invalidate the promotion gates.

## Rollback Hooks

### Default → Ramp (Stage C → Stage B)

1. Demote the orchestrator with
   `sorafs_cli config set --config orchestrator.json sorafs.gateway.rollout_phase ramp`
   (and mirror the same phase across SDK configs) so Stage B resumes fleet-wide.
2. Force clients into the safe transport profile via
   `sorafs_cli proxy set-mode --mode direct --note "sn16 rollback"`, capturing
   the transcript so the `/policy/proxy-toggle` remediation workflow stays
   auditable.
3. Run `cargo xtask soranet-rollout-capture --label rollback-default` to archive
   guard-directory diffs, promtool output, and dashboard screenshots under
   `artifacts/soranet_pq_rollout/`.

### Ramp → Canary (Stage B → Stage A)

1. Import the guard-directory snapshot captured before promotion with
   `sorafs_cli guard-directory import --guard-directory guards.json` and rerun
   `sorafs_cli guard-directory verify` so the demotion packet includes hashes.
2. Set `rollout_phase = "canary"` (or override with `anonymity_policy stage-a`)
   on orchestrator and client configs, then replay the PQ ratchet drill from
   `docs/source/soranet/pq_ratchet_runbook.md` to prove the downgrade pipeline.
3. Attach the updated PQ Ratchet and SN16 telemetry screenshots plus the alert
   outcomes to the incident log before notifying governance.

### Guardrail reminders

- Reference `docs/source/ops/soranet_transport_rollback.md` whenever a demotion
  occurs and log any temporary mitigation as a `TODO:` item in the rollout
  tracker for follow-up work.
- Keep `dashboards/alerts/soranet_handshake_rules.yml` and
  `dashboards/alerts/soranet_privacy_rules.yml` under `promtool test rules`
  coverage before and after a rollback so alert drift is documented alongside
  the capture bundle.
