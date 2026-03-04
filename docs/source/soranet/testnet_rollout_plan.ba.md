---
lang: ba
direction: ltr
source: docs/source/soranet/testnet_rollout_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1f8562a8d6cf82507a262dd99150e112f9afb6403bde1c62513867ed7a55fcde
source_last_modified: "2025-12-29T18:16:36.202693+00:00"
translation_last_reviewed: 2026-02-07
title: SoraNet Testnet Rollout Plan (SNNet-10)
summary: Execution plan for bringing the SoraNet anonymity overlay from limited testnet to global default.
---

# SoraNet Testnet Rollout Plan (SNNet-10)

SNNet-10 coordinates the staged activation of the SoraNet anonymity overlay
across the network. This document translates the roadmap bullet into concrete
deliverables, runbooks, and telemetry gates so that every operator understands
the expectations before the default-on switch flips.

## Launch Phases

| Phase | Timeline (target) | Scope | Required artefacts |
|-------|-------------------|-------|--------------------|
| **T0 — Closed Testnet** | Q4 2026 | 20–50 relays across ≥3 ASNs operated by core contributors. | Testnet onboarding kit (see below), guard pinning smoke suite, baseline latency + PoW metrics, brownout drill log. |
| **T1 — Public Beta** | Q1 2027 | ≥100 relays, guard rotation enabled, exit bonding enforced, SDK betas default to SoraNet with `anon-guard-pq`. | Updated onboarding kit, operator verification checklist, directory publishing SOP, telemetry dashboard pack, incident rehearsal reports. |
| **T2 — Mainnet Default** | Q2 2027 (gated on SNNet-6/7/9 completion) | Production network defaults to SoraNet; obfs/MASQUE transports and PQ ratchet enforcement enabled. | Governance approval minutes, direct-only rollback procedure, downgrade alarms, signed success metrics report. |

There is **no skip path**: each phase requires the telemetry and governance
artefacts from the preceding stage before promotion.

## Testnet Onboarding Kit

Every relay operator receives a deterministic package with the following files:

| Artefact | Description |
|----------|-------------|
| `01-readme.md` | Overview, contact points, and timeline. |
| `02-checklist.md` | Pre-flight checklist (hardware, network reachability, guard policy verification). |
| `03-config-example.toml` | Minimal SoraNet relay + orchestrator configuration aligned with SNNet-9 compliance blocks, including a `guard_directory` block that pins the latest guard snapshot hash. |
| `04-telemetry.md` | Instructions for wiring the SoraNet privacy metrics dashboards and alert thresholds. |
| `05-incident-playbook.md` | Brownout/downgrade response procedure with escalation matrix. |
| `06-verification-report.md` | Template operators complete and return once smoke tests pass. |

A rendered copy lives in `docs/examples/soranet_testnet_operator_kit/` (see the
companion example). Each promotion refreshes the kit; version numbers track the
phase (e.g., `testnet-kit-vT0.1`).

For public beta (T1) operators, the concise onboarding brief in
`docs/source/soranet/snnet10_beta_onboarding.md` summarises prerequisites,
telemetry deliverables, and the submission workflow while pointing back to the
deterministic kit and validator helpers.

`cargo xtask soranet-testnet-feed` generates the JSON feed that aggregates the
promotion window, relay roster, metrics report, drill evidence, and attachment
hashes referenced by the stage-gate template. Sign drill logs plus attachments
with `cargo xtask soranet-testnet-drill-bundle` first; the feed records
`drill_log.signed = true` when the bundle includes a signature.

## Success Metrics

Promotion between phases is gated on the following telemetry, collected for a
minimum of two weeks:

- `soranet_privacy_circuit_events_total`: 95% of circuits complete without
  brownout or downgrade events; remaining 5% capped by PQ supply.
- `sorafs_orchestrator_policy_events_total{outcome="brownout"}`: <1% of fetch
  sessions per day trigger brownout outside scheduled drills.
- `soranet_privacy_gar_reports_total`: variance within ±10% of expected GAR
  category mix; spikes must be explained by approved policy updates.
- PoW ticket success rate: ≥99% within 3s target window; reported via
  `soranet_privacy_throttles_total{scope="congestion"}`.
- Latency (95th percentile) per region: <200 ms once circuits are fully built,
  captured via `soranet_privacy_rtt_millis{percentile="p95"}`.

Dashboard and alert templates live in `dashboard_templates/` and
`alert_templates/`; mirror them into your telemetry repository and add them to
CI lint checks. Use `cargo xtask soranet-testnet-metrics` to generate the
governance-facing report before requesting promotion.

Stage-gate submissions must follow
`docs/source/soranet/snnet10_stage_gate_template.md`, which also links to the
ready-to-copy Markdown form stored under
`docs/examples/soranet_testnet_stage_gate/stage_gate_report_template.md`.

## Verification Checklist

Operators must sign off on the following before entering each phase:

- ✅ Relay advert signed with current admission envelope.
- ✅ Guard rotation smoke test (`tools/soranet-relay --check-rotation`) passes.
- ✅ `guard_directory` points at the latest `GuardDirectorySnapshotV2` artefact and `expected_directory_hash_hex` matches the committee digest (relay startup logs the validated hash).
- ✅ PQ ratchet metrics (`sorafs_orchestrator_pq_ratio`) stay above target
  thresholds for the requested stage.
- ✅ GAR compliance config matches latest tag (see SNNet-9 catalogue).
- ✅ Downgrade alarm simulation (disable collectors, expect alert within 5 min).
- ✅ PoW/DoS drill executed with documented mitigation steps.

A pre-filled template is included in the onboarding kit. Operators submit the
completed report to the governance helpdesk before receiving production
credentials.

## Governance & Reporting

- **Change control:** promotions require Governance Council approval recorded in
  the council minutes and attached to the status page.
- **Status digest:** publish weekly updates summarising relay count, PQ ratio,
  brownout incidents, and outstanding action items (stored in
  `docs/source/status/soranet_testnet_digest.md` once the cadence starts).
- **Rollbacks:** maintain a signed rollback plan that returns the network to the
  previous phase within 30 minutes, including DNS/guard cache invalidation and
  client communication templates.

## Supporting Assets

- `cargo xtask soranet-testnet-kit [--out <dir>]` materialises the onboarding kit
  from `xtask/templates/soranet_testnet/` into the target directory (defaults to
  `docs/examples/soranet_testnet_operator_kit/`).
- `cargo xtask soranet-testnet-metrics --input <metrics.json> [--out <path|->]`
  evaluates the SNNet-10 success metrics and emits a structured pass/fail report
  suitable for governance reviews. A sample snapshot lives in
  `docs/examples/soranet_testnet_metrics_sample.json`.
- Grafana and Alertmanager templates live under
  `dashboard_templates/soranet_testnet_overview.json` and
  `alert_templates/soranet_testnet_rules.yml`; copy them into your telemetry
  repository or wire them into CI lint checks.
- The downgrade communication template for SDK/portal messaging resides in
  `docs/source/soranet/templates/downgrade_communication_template.md`.
- Weekly status digests should use
  `docs/source/status/soranet_testnet_weekly_digest.md` as the canonical form.

Pull requests should update this document alongside any artefact or telemetry
changes so the rollout plan stays canonical.
