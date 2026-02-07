---
lang: am
direction: ltr
source: docs/source/ops/soranet_transport_rollback.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6b81a066ce40132baa04826165730cffa79f329bd32c7406a2acde1e2a0f98b3
source_last_modified: "2026-01-04T08:19:26.518691+00:00"
translation_last_reviewed: 2026-02-07
title: SoraNet Transport Default Rollback Plan
summary: Playbook for reversing the SNNet-5/SF-6 multi-source default and returning clients to the regulated direct-fetch profile.
---

# SoraNet Transport Default Rollback Plan

This playbook fulfils the roadmap requirement that every SoraNet transport
rollout ships with a documented rollback path (`roadmap.md`, “Sora Nexus Program
Integration Map”). It assumes the multi-source orchestrator described in
`docs/source/sorafs_orchestrator_rollout.md` is live and that clients default to
`TransportPolicy::SoranetPreferred`.

## 1. Metadata

- **Feature / rollout name:** SNNet-5 — SoraNet transport default (SF-6c multi-source orchestrator GA).
- **Owner / DRI:** Networking TL (SoraNet) + Developer Experience TL (SF-6) with SRE duty officer covering Alertmanager.
- **Change window:** Align with the SNNet-10 stage-gate window for “Public Beta → Mainnet Default” (`docs/source/soranet/snnet10_stage_gate_template.md`).
- **Rollback window:** First 60 minutes after the default-on flip or a governance-mandated downgrade; extend only with Networking TL approval.
- **Critical dependencies:** `sorafs_orchestrator` release ≥ the commit referenced in `status.md`, Torii build that embeds `torii.sorafs.orchestrator` config, guard directory snapshot signed via `soranet-directory`, puzzle service health (SNNet-6a), and governance approval minutes.
- **Related tickets / RFCs:** SF-6c adoption checklist, SNNet-5 rollouts, SNNet-5a direct-mode fallback pack (`docs/source/sorafs/direct_mode_pack.md`), PQ ratchet drill log (`docs/source/soranet/pq_ratchet_runbook.md`), CI helper `ci/check_sorafs_orchestrator_adoption.sh`.

## 2. Trigger Conditions

Rollback is authorised when any of the following hold and the duty officer and Networking TL agree the blast-radius exceeds the burn-in SLOs:

- **Adoption gate failure:** `cargo xtask sorafs-adoption-check` (or the CI wrapper) reports `provider_mix != soranet-first` or `allow_single_source_required=true` for GA artefacts. Detection: CI job `ci/check_sorafs_orchestrator_adoption.sh`; escalate to Storage + Networking.
- **No healthy SoraNet relays:** `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` > 10 over 5 minutes in two regions. Detection: Grafana `dashboards/grafana/sorafs_fetch_observability.json` panel “Failure Reasons”. Escalation: Networking TL + relay on-call.
- **PQ deficit brownout:** `sorafs_orchestrator_pq_deficit_ratio` or `sorafs_orchestrator_brownouts_total` trips the SNNet-5b alert (`dashboards/grafana/soranet_pq_ratchet.json`). Detection: Alertmanager route `snnet-pq-brownout`. Mitigation requires falling back to `direct-only` until PQ supply recovers.
- **Telemetry redaction/compliance request:** Governance or compliance demands direct mode (for example, regulator prohibits anonymous relays for a jurisdiction). Detection: `docs/source/sorafs/direct_mode_pack.md` checklist, ticket prefix `SNNET-DIR-***`.
- **Guard directory mismatch:** `sorafs_cli guard-directory verify` reports `expected-directory-hash` drift or `soranet-directory verify` fails signature validation, indicating the relay directory cannot be trusted.

## 3. Prerequisites & Safeguards

- **Snapshot evidence:** Before rolling back, export the guard directory and copy the guard cache used for the GA run. Set `GUARD_CACHE` to the file referenced by `--guard-cache` (for example, `/var/lib/sorafs/guards.norito`):

  ```bash
  sorafs_cli guard-directory fetch \
    --url "$SNAPSHOT_URL" \
    --output artifacts/rollback/guard_directory_pre_rollback.norito \
    --expected-directory-hash "$DIRECTORY_HASH"

  cp "$GUARD_CACHE" artifacts/rollback/guard_cache_pre_rollback.norito
  ```

- **Telemetry baselines:** Bookmark `dashboards/grafana/sorafs_fetch_observability.json` and `dashboards/grafana/soranet_pq_ratchet.json`; download JSON exports so Alertmanager silences reference the same panels.
- **Adoption artefacts:** Run `ci/check_sorafs_orchestrator_adoption.sh` (or `cargo xtask sorafs-adoption-check --scoreboard path --summary path --allow-single-source --require-direct-only --require-telemetry`) and persist `adoption_report.json` plus the `scoreboard.json`/`summary.json` used for GA.
- **Communication drafts:** Pre-fill the operator + governance templates stored in `docs/source/sorafs/direct_mode_pack.md` (`Announce Direct Mode` section) and keep the status-page copy staged.
- **Override shakedown:** Verify the downgrade knobs by running `sorafs_cli fetch --transport-policy-override direct-only` against the canonical manifest (`fixtures/sorafs_manifest/ci_sample/payload.plan.json`) so the on-call knows the override still works.

## 4. Step-by-Step Rollback

| Step | Command / Action | Owner | Notes |
|------|------------------|-------|-------|
| 1 | Freeze orchestrator rollouts (`kubectl scale deploy/sorafs-orchestrator --replicas=0` in canary) and notify Networking in `#soranet-ops`. | SRE duty | Prevents new pods from starting with stale configs mid-rollback. |
| 2 | Force direct-mode transport by editing `torii.sorafs.orchestrator.transport_policy` to `"direct-only"` and pushing the config to all Torii nodes (`helm upgrade ... --set torii.sorafs.orchestrator.transport_policy=direct-only`). | Networking TL | Matches the override semantics in `crates/sorafs_orchestrator/src/policy.rs`. |
| 3 | Flush guard caches / disable circuit manager to avoid stale SoraNet circuits: `sorafs_cli guard-cache prune --config torii.sorafs.orchestrator --keep-classical-only` followed by `sorafs_cli config set --config torii.sorafs.orchestrator circuit_manager.enabled false`. | Storage / SDK owner | Ensures future fetches will not attempt PQ relays until re-enabled. |
| 4 | Redeploy SDK/CLI defaults (JS/Swift/Python) with `transportPolicy: "direct-only"` or `policyOverride.transportPolicy = "direct-only"` while attaching the incident ticket to release notes. | SDK owners | Refer to `docs/source/sorafs/direct_mode_pack.md` guidance for each SDK. |
| 5 | Capture evidence: run `sorafs_cli fetch` against the canonical plan with `--transport-policy direct-only --scoreboard-out artifacts/rollback/direct_only.scoreboard.json --json-out artifacts/rollback/direct_only.summary.json`, then `cargo xtask sorafs-adoption-check --scoreboard artifacts/rollback/direct_only.scoreboard.json --summary artifacts/rollback/direct_only.summary.json --allow-single-source --require-direct-only --require-telemetry`. | Release Engineering | The adoption report must show `provider_mix="direct-only"` and record the ticket ID via `--telemetry-source-label`. |
| 6 | Resume traffic by scaling the orchestrator deployments back up once the adoption report passes and Alertmanager is green. | SRE duty | Keep Alert silences scoped to the rollback window only. |

## 5. Verification Checklist

- `sorafs_orchestrator_transport_events_total{mode="direct-only"}` equals 100 % of samples for the affected regions within 10 minutes.
- `sorafs_orchestrator_brownouts_total` stops increasing; `sorafs_orchestrator_pq_ratio` panels will drop but Alertmanager should switch to the direct-mode dashboard.
- `scoreboard.json` and `summary.json` attached to the incident contain `transport_policy="direct-only"`, `provider_mix="direct-only"`, `gateway_manifest_provided=true` (when applicable), and `provider_count=0` for gateway-only runs.
- `ci/check_sorafs_orchestrator_adoption.sh` rerun succeeds with explicit `--allow-single-source --require-direct-only` flags and stores the regenerated `adoption_report.json`.
- Governance docket updated with the Alertmanager silence link and scoreboard hash.

## 6. Communications

| Audience | Channel | Owner | Status Message |
|----------|---------|-------|----------------|
| Operators / partners | `#soranet-ops`, status page, email distro | Communications lead | “SoraNet transport temporarily rolled back to the regulated direct-profile while we investigate SNNet-5 incident `<ticket>`. Direct fetch limits unchanged; expect higher latency but no data loss.” |
| Governance council | Stage-gate docket | Networking TL | “Rollback executed at `<timestamp>` UTC; adoption report + scoreboard hash attached; re-enable blocked pending guard directory remediation.” |
| Incident log | `docs/source/ops/archive/<incident-id>/` | SRE duty | Store `scoreboard.json`, `summary.json`, adoption report, guard directory hash, Alertmanager exports, and link to `docs/source/sorafs/direct_mode_pack.md` justification. |

## 7. Post-Rollback Actions

- Assign RCA owner (Networking TL by default) and open issues for the blocking SNNet item (directory corruption, PQ deficit, etc.).
- File follow-up tickets for any SDKs that required manual overrides so they can revert once SoraNet is healthy.
- Define re-enable criteria: guard directory hash matches trusted snapshot, `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` stays at zero for 30 minutes, PQ deficit alert silenced legitimately (not force muted), and adoption gate passes without `--allow-single-source`.
- Schedule lessons-learned at the next governance sync; attach this playbook to the meeting notes and update `status.md`.

## 8. Attachments

- Guard directory snapshot + verification log (`artifacts/rollback/guard_directory_pre_rollback.norito` + `soranet-directory verify` stdout).
- Guard cache dump (`artifacts/rollback/guard_cache_pre_rollback.norito` + signature tag when `--guard-cache-key` is used).
- `scoreboard.json`, `summary.json`, and `adoption_report.json` emitted during the rollback.
- Grafana JSON exports from `dashboards/grafana/sorafs_fetch_observability.json` and `dashboards/grafana/soranet_pq_ratchet.json`.
- Incident communications (status page entry, governance memo, Alertmanager silence export).
