---
lang: mn
direction: ltr
source: docs/portal/docs/nexus/nexus-bootstrap-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: aa25c267f36e3245866776d5149039e1b9833407a84126d66a21cf5296e51414
source_last_modified: "2025-12-29T18:16:35.135788+00:00"
translation_last_reviewed: 2026-02-07
id: nexus-bootstrap-plan
title: Sora Nexus bootstrap & observability
description: Operational plan for bringing the core Nexus validator cluster online before layering SoraFS and SoraNet services.
---

:::note Canonical Source
This page mirrors `docs/source/soranexus_bootstrap_plan.md`. Keep both copies aligned until localized versions land in the portal.
:::

# Sora Nexus Bootstrap & Observability Plan

## Objectives
- Stand up the base Sora Nexus validator/observer network with governance keys, Torii APIs, and consensus monitoring.
- Validate core services (Torii, consensus, persistence) before enabling SoraFS/SoraNet piggyback deployments.
- Establish CI/CD workflows and observability dashboards/alerts to ensure network health.

## Prerequisites
- Governance key material (council multisig, committee keys) available in HSM or Vault.
- Baseline infrastructure (Kubernetes clusters or bare-metal nodes) in primary/secondary regions.
- Updated bootstrap configuration (`configs/nexus/bootstrap/*.toml`) reflecting latest consensus parameters.

## Network Environments
- Operate two Nexus environments with distinct network prefixes:
- **Sora Nexus (mainnet)** – production network prefix `nexus`, hosting canonical governance and SoraFS/SoraNet piggyback services (chain ID `0x02F1` / UUID `00000000-0000-0000-0000-000000000753`).
- **Sora Testus (testnet)** – staging network prefix `testus`, mirroring mainnet configuration for integration testing and pre-release validation (chain UUID `809574f5-fee7-5e69-bfcf-52451e42d50f`).
- Maintain separate genesis files, governance keys, and infrastructure footprints for each environment. Testus acts as the proving ground for all SoraFS/SoraNet rollouts before promotion to Nexus.
- CI/CD pipelines should deploy to Testus first, execute automated smoke tests, and require manual promotion to Nexus once checks pass.
- Reference configuration bundles live under `configs/soranexus/nexus/` (mainnet) and `configs/soranexus/testus/` (testnet), each containing sample `config.toml`, `genesis.json`, and Torii admission directories.

## Step 1 – Configuration Review
1. Audit existing documentation:
   - `docs/source/nexus/architecture.md` (consensus, Torii layout).
   - `docs/source/nexus/deployment_checklist.md` (infra requirements).
   - `docs/source/nexus/governance_keys.md` (key custody procedures).
2. Validate genesis files (`configs/nexus/genesis/*.json`) align with current validator roster and staking weights.
3. Confirm network parameters:
   - Consensus committee size & quorum.
   - Block interval / finality thresholds.
   - Torii service ports and TLS certificates.

## Step 2 – Bootstrap Cluster Deployment
1. Provision validator nodes:
   - Deploy `irohad` instances (validators) with persistent volumes.
   - Ensure network firewall rules allow consensus & Torii traffic between nodes.
2. Start Torii services (REST/WebSocket) on each validator with TLS.
3. Deploy observer nodes (read-only) for extra resilience.
4. Run bootstrap scripts (`scripts/nexus_bootstrap.sh`) to distribute genesis, start consensus, and register nodes.
5. Execute smoke tests:
   - Submit test transactions via Torii (`iroha_cli tx submit`).
   - Verify block production/finality through telemetry.
   - Check ledger replication across validators/observers.

## Step 3 – Governance & Key Management
1. Load council multisig configuration; confirm governance proposals can be submitted and ratified.
2. Securely store consensus/committee keys; configure automatic backups with access logging.
3. Set up emergency key rotation procedures (`docs/source/nexus/key_rotation.md`) and verify runbook.

## Step 4 – CI/CD Integration
1. Configure pipelines:
   - Build & publish validator/Torii images (GitHub Actions or GitLab CI).
   - Automated configuration validation (lint genesis, verify signatures).
   - Deployment pipelines (Helm/Kustomize) for staging & production clusters.
2. Implement smoke tests in CI (spin up ephemeral cluster, run canonical transaction suite).
3. Add rollback scripts for failed deployments and document runbooks.

## Step 5 – Observability & Alerts
1. Deploy monitoring stack (Prometheus + Grafana + Alertmanager) per region.
2. Collect core metrics:
  - `nexus_consensus_height`, `nexus_finality_lag`, `torii_request_duration_seconds`, `validator_peer_count`.
   - Logs via Loki/ELK for Torii & consensus services.
3. Dashboards:
   - Consensus health (block height, finality, peer status).
   - Torii API latency/error rates.
   - Governance transactions & proposal statuses.
4. Alerts:
   - Block production stall (>2 block intervals).
   - Peer count drop below quorum.
   - Torii error rate spikes.
   - Governance proposal queue backlog.

## Step 6 – Validation & Handoff
1. Run end-to-end validation:
   - Submit governance proposal (e.g., parameter change).
   - Process it through council approval to ensure governance pipeline works.
   - Run ledger state diff to ensure consistency.
2. Document runbook for on-call (incident response, failover, scaling).
3. Communicate readiness to SoraFS/SoraNet teams; confirm piggyback deployments can point to Nexus nodes.

## Implementation Checklist
- [ ] Genesis/configuration audit completed.
- [ ] Validator & observer nodes deployed with healthy consensus.
- [ ] Governance keys loaded, proposal tested.
- [ ] CI/CD pipelines running (build + deploy + smoke tests).
- [ ] Observability dashboards live with alerting.
- [ ] Handoff documentation delivered to downstream teams.
