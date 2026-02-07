---
id: nexus-overview
lang: ka
direction: ltr
source: docs/portal/docs/nexus/overview.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Sora Nexus overview
description: High-level summary of the Iroha 3 (Sora Nexus) architecture with pointers to the canonical mono-repo docs.
---

Nexus (Iroha 3) extends Iroha 2 with multi-lane execution, governance-scoped
data spaces, and shared tooling across every SDK. This page mirrors the new
`docs/source/nexus_overview.md` brief in the mono-repo so portal readers can
quickly understand how the architecture pieces fit together.

## Release lines

- **Iroha 2** – self-hosted deployments for consortium or private networks.
- **Iroha 3 / Sora Nexus** – the multi-lane public network where operators
  register data spaces (DS) and inherit shared governance, settlement, and
  observability tooling.
- Both lines compile from the same workspace (IVM + Kotodama toolchain), so SDK
  fixes, ABI updates, and Norito fixtures remain portable. Operators download
  the `iroha3-<version>-<os>.tar.zst` bundle to join Nexus; refer to
  `docs/source/sora_nexus_operator_onboarding.md` for the fullscreen checklist.

## Building blocks

| Component | Summary | Portal hooks |
|-----------|---------|--------------|
| Data Space (DS) | Governance-defined execution/storages domain that owns one or more lanes, declares validator sets, privacy class, fee + DA policy. | See [Nexus spec](./nexus-spec) for the manifest schema. |
| Lane | Deterministic shard of execution; emits commitments that the global NPoS ring orders. Lane classes include `default_public`, `public_custom`, `private_permissioned`, and `hybrid_confidential`. | [Lane model](./nexus-lane-model) captures geometry, storage prefixes, and retention. |
| Transition plan | Placeholder identifiers, routing phases, and dual-profile packaging track how single-lane deployments evolve into Nexus. | [Transition notes](./nexus-transition-notes) document each migration phase. |
| Space Directory | Registry contract that stores DS manifests + versions. Operators reconcile catalog entries against this directory before joining. | Manifest diff tracker lives under `docs/source/project_tracker/nexus_config_deltas/`. |
| Lane catalog | `[nexus]` config section that maps lane IDs to aliases, routing policies, and DA thresholds. `irohad --sora --config … --trace-config` prints the resolved catalog for audits. | Use `docs/source/sora_nexus_operator_onboarding.md` for the CLI walk-through. |
| Settlement router | XOR transfer orchestrator that connects private CBDC lanes with public liquidity lanes. | `docs/source/cbdc_lane_playbook.md` spells out policy knobs and telemetry gates. |
| Telemetry/SLOs | Dashboards + alerts under `dashboards/grafana/nexus_*.json` capture lane height, DA backlog, settlement latency, and governance queue depth. | [Telemetry remediation plan](./nexus-telemetry-remediation) spells out the dashboards, alerts, and audit evidence. |

## Rollout snapshot

| Phase | Focus | Exit criteria |
|-------|-------|---------------|
| N0 – Closed beta | Council-managed registrar (`.sora`), manual operator onboarding, static lane catalog. | Signed DS manifests + rehearsed governance hand-offs. |
| N1 – Public launch | Adds `.nexus` suffixes, auctions, self-service registrar, XOR settlement wiring. | Resolver/gateway sync tests, billing reconciliation dashboards, dispute tabletop drills. |
| N2 – Expansion | Introduces `.dao`, reseller APIs, analytics, dispute portal, steward scorecards. | Compliance artefacts versioned, policy-jury toolkit online, treasury transparency reports. |
| NX-12/13/14 gate | Compliance engine, telemetry dashboards, and documentation must ship together before partner pilots. | [Nexus overview](./nexus-overview) + [Nexus operations](./nexus-operations) published, dashboards wired, policy engine merged. |

## Operator responsibilities

1. **Config hygiene** – keep `config/config.toml` synced with the published lane &
   dataspace catalog; archive `--trace-config` output with every release ticket.
2. **Manifest tracking** – reconcile catalog entries with the latest Space
   Directory bundle before joining or upgrading nodes.
3. **Telemetry coverage** – expose the `nexus_lanes.json`, `nexus_settlement.json`,
   and related SDK dashboards; wire alerts to PagerDuty and run quarterly reviews per the telemetry remediation plan.
4. **Incident reporting** – follow the severity matrix in
   [Nexus operations](./nexus-operations) and file RCAs within five business days.
5. **Governance readiness** – attend Nexus council votes impacting your lanes and
   rehearse rollback instructions quarterly (tracked via
   `docs/source/project_tracker/nexus_config_deltas/`).

## See also

- Canonical overview: `docs/source/nexus_overview.md`
- Detailed spec: [./nexus-spec](./nexus-spec)
- Lane geometry: [./nexus-lane-model](./nexus-lane-model)
- Transition plan: [./nexus-transition-notes](./nexus-transition-notes)
- Telemetry remediation plan: [./nexus-telemetry-remediation](./nexus-telemetry-remediation)
- Operations runbook: [./nexus-operations](./nexus-operations)
- Operator onboarding guide: `docs/source/sora_nexus_operator_onboarding.md`
