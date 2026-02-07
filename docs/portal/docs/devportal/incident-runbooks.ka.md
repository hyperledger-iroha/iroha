---
lang: ka
direction: ltr
source: docs/portal/docs/devportal/incident-runbooks.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8599dbc1a8e4fe846965eed90af128deb5950f83dc61838fea583b326b92a011
source_last_modified: "2025-12-29T18:16:35.104300+00:00"
translation_last_reviewed: 2026-02-07
id: incident-runbooks
title: Incident Runbooks & Rollback Drills
sidebar_label: Incident Runbooks
description: Response guides for failed portal deployments, SoraFS replication degradation, analytics outages, and the quarterly rehearsal cadence required by DOCS-9.
---

## Purpose

Roadmap item **DOCS-9** calls for actionable playbooks plus a rehearsal plan so
portal operators can recover from shipping failures without guessing. This note
covers three high-signal incidents—failed deployments, replication
degradation, and analytics outages—and documents the quarterly drills that
prove alias rollback and synthetic validation still work end to end.

### Related material

- [`devportal/deploy-guide`](./deploy-guide) — packaging, signing, and alias
  promotion workflow.
- [`devportal/observability`](./observability) — release tags, analytics, and
  probes referenced below.
- `docs/source/sorafs_node_client_protocol.md`
  and [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops)
  — registry telemetry and escalation thresholds.
- `docs/portal/scripts/sorafs-pin-release.sh` and `npm run probe:*` helpers
  referenced throughout the checklists.

### Shared telemetry & tooling

| Signal / Tool | Purpose |
| ------------- | ------- |
| `torii_sorafs_replication_sla_total` (met/missed/pending) | Detects replication stalls and SLA breaches. |
| `torii_sorafs_replication_backlog_total`, `torii_sorafs_replication_completion_latency_epochs` | Quantifies backlog depth and completion latency for triage. |
| `torii_sorafs_gateway_refusals_total`, `torii_sorafs_manifest_submit_total{status="error"}` | Shows gateway-side failures that often follow a bad deploy. |
| `npm run probe:portal` / `npm run probe:tryit-proxy` | Synthetic probes that gate releases and validate runbacks. |
| `npm run check:links` | Broken-link gate; used after every mitigation. |
| `sorafs_cli manifest submit … --alias-*` (wrapped by `scripts/sorafs-pin-release.sh`) | Alias promotion/reversion mechanism. |
| `Docs Portal Publishing` Grafana board (`dashboards/grafana/docs_portal.json`) | Aggregates refusal/alias/TLS/replication telemetry. PagerDuty alerts reference these panels for evidence. |

## Runbook — Failed deployment or bad artefact

### Trigger conditions

- Preview/production probes fail (`npm run probe:portal -- --expect-release=…`).
- Grafana alerts on `torii_sorafs_gateway_refusals_total` or
  `torii_sorafs_manifest_submit_total{status="error"}` after a rollout.
- Manual QA notices broken routes or Try-It proxy failures immediately after
  alias promotion.

### Immediate containment

1. **Freeze deployments:** mark the CI pipeline with `DEPLOY_FREEZE=1` (GitHub
   workflow input) or pause the Jenkins job so no additional artefacts go out.
2. **Capture artefacts:** download the failing build’s `build/checksums.sha256`,
   `portal.manifest*.{json,to,bundle,sig}`, and probe output so the rollback can
   reference exact digests.
3. **Notify stakeholders:** storage SRE, Docs/DevRel lead, and the governance
   duty officer for awareness (especially when `docs.sora` is impacted).

### Rollback procedure

1. Identify the last-known-good (LKG) manifest. The production workflow stores
   them under `artifacts/devportal/<release>/sorafs/portal.manifest.to`.
2. Rebind the alias to that manifest with the shipping helper:

```bash
cd docs/portal
./scripts/sorafs-pin-release.sh \
  --build-dir build \
  --artifact-dir artifacts/revert-$(date +%Y%m%d%H%M) \
  --sorafs-dir artifacts/revert-$(date +%Y%m%d%H%M)/sorafs \
  --pin-min-replicas 5 \
  --alias "docs-prod-revert" \
  --alias-namespace "${PIN_ALIAS_NAMESPACE}" \
  --alias-name "${PIN_ALIAS_NAME}" \
  --alias-proof "${PIN_ALIAS_PROOF_PATH}" \
  --torii-url "${TORII_URL}" \
  --submitted-epoch "$(date +%Y%m%d)" \
  --authority "${AUTHORITY}" \
  --private-key "${PRIVATE_KEY}" \
  --skip-submit

# swap in the LKG artefacts before submission
cp /secure/archive/lkg/portal.manifest.to artifacts/.../sorafs/portal.manifest.to
cp /secure/archive/lkg/portal.manifest.bundle.json artifacts/.../sorafs/

cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  manifest submit \
  --manifest artifacts/.../sorafs/portal.manifest.to \
  --chunk-plan artifacts/.../sorafs/portal.plan.json \
  --torii-url "${TORII_URL}" \
  --authority "${AUTHORITY}" \
  --private-key "${PRIVATE_KEY}" \
  --alias-namespace "${PIN_ALIAS_NAMESPACE}" \
  --alias-name "${PIN_ALIAS_NAME}" \
  --alias-proof "${PIN_ALIAS_PROOF_PATH}" \
  --metadata rollback_from="${FAILED_RELEASE}" \
  --summary-out artifacts/.../sorafs/rollback.submit.json
```

3. Record the rollback summary in the incident ticket together with the LKG and
   failed manifest digests.

### Validation

1. `npm run probe:portal -- --expect-release=${LKG_TAG}`.
2. `npm run check:links`.
3. `sorafs_cli manifest verify-signature …` and `sorafs_cli proof verify …`
   (see the deployment guide) to confirm the re-promoted manifest still matches
   the archived CAR.
4. `npm run probe:tryit-proxy` to ensure the Try-It staging proxy came back.

### Post-incident

1. Re-enable the deployment pipeline only after the root cause is understood.
2. Backfill [`devportal/deploy-guide`](./deploy-guide) “Lessons learned”
   entries with new gotchas, if any.
3. File defects for the failing test suite (probe, link checker, etc.).

## Runbook — Replication degradation

### Trigger conditions

- Alert: `sum(torii_sorafs_replication_sla_total{outcome="met"}) /
  clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) <
  0.95` for 10 minutes.
- `torii_sorafs_replication_backlog_total > 10` for 10 minutes (see
  `pin-registry-ops.md`).
- Governance reports slow alias availability after a release.

### Triage

1. Inspect [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops) dashboards to confirm
   whether the backlog is localized to a storage class or a provider fleet.
2. Cross-check Torii logs for `sorafs_registry::submit_manifest` warnings to
   determine whether submissions themselves are failing.
3. Sample replica health via `sorafs_cli manifest status --manifest …` (lists
   per-provider replication outcomes).

### Mitigation

1. Reissue the manifest with higher replica count (`--pin-min-replicas 7`) using
   `scripts/sorafs-pin-release.sh` so the scheduler spreads load across a larger
   provider set. Record the new manifest digest in the incident log.
2. If backlog is tied to a single provider, temporarily disable it via the
   replication scheduler (documented in `pin-registry-ops.md`) and submit a new
   manifest forcing the other providers to refresh the alias.
3. When alias freshness is more critical than replication parity, rebind the
   alias to a warm manifest already staged (`docs-preview`), then publish a
   follow-up manifest once SRE clears the backlog.

### Recovery & closure

1. Monitor `torii_sorafs_replication_sla_total{outcome="missed"}` to ensure the
   count plateaus.
2. Capture `sorafs_cli manifest status` output as evidence that every replica is
   back in compliance.
3. File or update the replication backlog post-mortem with next steps
   (provider scaling, chunker tuning, etc.).

## Runbook — Analytics or telemetry outage

### Trigger conditions

- `npm run probe:portal` succeeds but dashboards stop ingesting
  `AnalyticsTracker` events for >15 minutes.
- Privacy review flags an unexpected increase in dropped events.
- `npm run probe:tryit-proxy` fails on `/probe/analytics` paths.

### Response

1. Verify build-time inputs: `DOCS_ANALYTICS_ENDPOINT` and
   `DOCS_ANALYTICS_SAMPLE_RATE` in the failing release artifact (`build/release.json`).
2. Re-run `npm run probe:portal` with `DOCS_ANALYTICS_ENDPOINT` pointing at the
   staging collector to confirm the tracker still emits payloads.
3. If collectors are down, set `DOCS_ANALYTICS_ENDPOINT=""` and rebuild so the
   tracker short-circuits; record the outage window in the incident timeline.
4. Validate `scripts/check-links.mjs` still fingerprints `checksums.sha256`
   (analytics outages must *not* block sitemap validation).
5. Once the collector recovers, run `npm run test:widgets` to exercise the
   analytics helper unit tests before republishing.

### Post-incident

1. Update [`devportal/observability`](./observability) with any new collector
   limitations or sampling requirements.
2. File governance notice if any analytics data was dropped or redacted outside
   policy.

## Quarterly resilience drills

Run both drills during the **first Tuesday of each quarter** (Jan/Apr/Jul/Oct)
or immediately after any major infrastructure change. Store artifacts under
`artifacts/devportal/drills/<YYYYMMDD>/`.

| Drill | Steps | Evidence |
| ----- | ----- | -------- |
| Alias rollback rehearsal | 1. Replay the “Failed deployment” rollback using the most recent production manifest.<br/>2. Re-bind to production once probes pass.<br/>3. Record `portal.manifest.submit.summary.json` and probe logs in the drill folder. | `rollback.submit.json`, probe output, and release tag of the rehearsal. |
| Synthetic validation audit | 1. Run `npm run probe:portal` and `npm run probe:tryit-proxy` against production and staging.<br/>2. Run `npm run check:links` and archive `build/link-report.json`.<br/>3. Attach screenshots/exports of Grafana panels confirming probe success. | Probe logs + `link-report.json` referencing the manifest fingerprint. |

Escalate missed drills to the Docs/DevRel manager and SRE governance review,
since the roadmap requires deterministic, quarterly evidence that both alias
rollback and portal probes remain healthy.

## PagerDuty & on-call coordination

- PagerDuty service **Docs Portal Publishing** owns the alerts generated from
  `dashboards/grafana/docs_portal.json`. The rules `DocsPortal/GatewayRefusals`,
  `DocsPortal/AliasCache`, and `DocsPortal/TLSExpiry` page the Docs/DevRel
  primary with Storage SRE as secondary.
- When paged, include the `DOCS_RELEASE_TAG`, attach screenshots of the affected
  Grafana panels, and link probe/link-check output in the incident notes before
  mitigation starts.
- After mitigation (rollback or redeploy), re-run `npm run probe:portal`,
  `npm run check:links`, and capture fresh Grafana snapshots showing the metrics
  back within thresholds. Attach all evidence to the PagerDuty incident prior to
  resolving it.
- If two alerts fire simultaneously (for example TLS expiry plus backlog), triage
  refuses first (stop publishing), execute the rollback procedure, then clear
  TLS/backlog items with Storage SRE on the bridge.
