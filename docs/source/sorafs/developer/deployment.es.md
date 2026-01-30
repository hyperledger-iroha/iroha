---
lang: es
direction: ltr
source: docs/source/sorafs/developer/deployment.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 410a5d027a4a6c8f290bdea16f3debf579acfc2dad94797c48963186efb6cc43
source_last_modified: "2026-01-03T18:07:58.352875+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: Deployment Notes
summary: Checklist for promoting the SoraFS pipeline from CI to production.
---

> **Portal:** Mirrored in `docs/portal/docs/sorafs/developer-deployment.md`.
> Update both versions to keep reviewers aligned.

# Deployment Notes

The SoraFS packaging workflow hardens determinism, so moving from CI to
production mainly requires operational guardrails. Use this checklist when
rolling the tooling out to real gateways and storage providers.

## Pre-flight

- **Registry alignment** — confirm chunker profiles and manifests reference the
  same `namespace.name@semver` tuple (`docs/source/sorafs/chunker_registry.md`).
- **Admission policy** — review the signed provider adverts and alias proofs
  needed for `manifest submit` (`docs/source/sorafs/provider_admission_policy.md`).
- **Pin registry runbook** — keep `docs/source/sorafs/runbooks/pin_registry_ops.md`
  handy for recovery scenarios (alias rotation, replication failures).

## Environment configuration

- Gateways must enable the proof streaming endpoint (`POST /v1/sorafs/proof/stream`)
  so the CLI can emit telemetry summaries.
- Configure `sorafs_alias_cache` policy using the defaults in
  `iroha_config` or the CLI helper (`sorafs_cli manifest submit --alias-*`).
- Provide stream tokens (or Torii credentials) via a secure secret manager.
- Enable telemetry exporters (`torii_sorafs_proof_stream_*`,
  `torii_sorafs_chunk_range_*`) and ship them to your Prometheus/OTel stack.

## Rollout strategy

1. **Blue/green manifests**
   - Use `manifest submit --summary-out` to archive responses for each rollout.
   - Keep an eye on `torii_sorafs_gateway_refusals_total` to catch capability
     mismatches early.
2. **Proof validation**
   - Treat failures in `sorafs_cli proof stream` as deployment blockers; latency
     spikes often indicate provider throttling or misconfigured tiers.
   - `proof verify` should be part of the post-pin smoke test to ensure the CAR
     hosted by providers still matches the manifest digest.
3. **Telemetry dashboards**
   - Import `docs/examples/sorafs_proof_streaming_dashboard.json` into Grafana.
   - Layer additional panels for pin registry health
     (`docs/source/sorafs/runbooks/pin_registry_ops.md`) and chunk range stats.
4. **Multi-source enablement**
   - Follow the staged rollout steps in
     `docs/source/sorafs/runbooks/multi_source_rollout.md` when turning on the
     orchestrator, and archive the scoreboard/telemetry artefacts for audits.

## Incident handling

- Follow the escalation paths in `docs/source/sorafs/runbooks/`:
  - `sorafs_gateway_operator_playbook.md` for gateway outages and stream-token
    exhaustion.
  - `dispute_revocation_runbook.md` when replication disputes occur.
  - `sorafs_node_ops.md` for node-level maintenance.
  - `multi_source_rollout.md` for orchestrator overrides, peer blacklisting, and
    staged rollouts.
- Record proof failures and latency anomalies in GovernanceLog via the existing
  PoR tracker APIs so governance can assess provider performance.

## Next steps

- Integrate orchestrator automation (`sorafs_car::multi_fetch`) once the
  multi-source fetch orchestrator (SF-6b) lands.
- Track PDP/PoTR upgrades under SF-13/SF-14; the CLI and docs will evolve to
  surface deadlines and tier selection once those proofs stabilize.

By combining these deployment notes with the quickstart and CI recipes, teams
can move from local experiments to production-grade SoraFS pipelines with a
repeatable, observable process.
