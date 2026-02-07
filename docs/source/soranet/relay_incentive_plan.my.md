---
lang: my
direction: ltr
source: docs/source/soranet/relay_incentive_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bd84a0831ba689db769998167003989338540598a7dab519f440df943a93cb79
source_last_modified: "2026-01-22T16:26:46.593230+00:00"
translation_last_reviewed: 2026-02-07
title: Relay Incentive & Credits Plan (SNNet-7)
summary: SNNet-7 bonding, measurement, and XOR payout architecture for the SoraNet relay programme.
---

# Relay Incentive & Credits (SNNet-7)

SNNet-7 introduces the first production-ready incentive pipeline for SoraNet relays. The workstream
covers bonding requirements for exits, blinded bandwidth attestations, epoch-level scoring, XOR
treasury payouts, and the observability story needed for Sora Parliament oversight. This document
tracks the artefacts implemented in this repository and the follow-up items required to complete
the roadmap milestone.

## Goals & Scope

- Enforce a deterministic minimum bond for exit-capable relays so Sybil costs remain aligned with
  the anonymity budget.
- Collect blinded bandwidth proofs and uptime counters per relay/epoch and expose them through
  Norito payloads that the treasury pipeline can verify offline.
- Convert relay metrics into XOR payout instructions that integrate with Sora Parliament budget
  approvals and share treasury reporting with the broader storage incentives stack.
- Surface clear dashboards: per-relay earnings, QoS trends, and compliance status, all derived from
  the data model types shipped with SNNet-7.

## Bonding & Policy Surface

- `RelayBondPolicyV1` and `RelayBondLedgerEntryV1` live in
  `crates/iroha_data_model/src/soranet/incentives.rs` and encode the minimum exit bond, grace window,
  and ledger entries tracked per relay. The helper `RelayBondLedgerEntryV1::meets_exit_minimum`
  ensures exit relays adhere to the bond requirement.
- `RelayRewardEngine` (in `crates/sorafs_orchestrator/src/incentives.rs`) consumes these records and
  zeroes payouts when the minimum bond is not maintained.
- Directory publishers can now invoke `RelayDirectory::enforce_exit_bond_policy` (see
  `crates/sorafs_orchestrator/src/soranet.rs`) to revoke exit roles for relays whose bond ledger
  entries are missing, use the wrong asset, or fall short of `RelayBondPolicyV1::minimum_exit_bond`.
  The helper returns structured demotion reasons so governance tooling can surface audit trails.

## Bandwidth Measurement & Proof Flow

- `RelayBandwidthProofV1`, `BandwidthConfidenceV1`, and the supporting measurement identifiers live
  in the SoraNet data model. Blinded measurement clients sign these payloads before shipping them to
  relays/treasury.
- `tools/soranet-relay/src/incentives.rs` introduces `RelayPerformanceAccumulator`, which deduplicates
  measurement flows, tracks the minimum confidence per epoch, and aggregates verified byte counters.
- `RelayEpochMetricsV1` captures uptime, bandwidth, and compliance inputs for an epoch. The
  accumulator finalises these metrics and emits Norito payloads ready for treasury ingestion.
- The relay runtime now feeds blinded bandwidth proofs arriving over QUIC into
  `RelayPerformanceAccumulator` in real time, deduplicating measurements and exposing the live state
  via Prometheus for downstream treasury tooling.

## Epoch Scoring & Reward Calculation

- `RelayRewardEngine` normalises the metrics using configurable per-mille weights for uptime and
  bandwidth, applies optional compliance penalties, and emits `RelayRewardInstructionV1`
  (Norito-friendly XOR payout instructions).
- The engine clamps bandwidth ratios against a governance-set target and exposes the budget approval
  hash so Parliament can trace payouts back to signed approvals.
- The orchestrator can persist `RelayEpochMetricsV1` snapshots via the optional
  `RewardConfig::metrics_log_path`, allowing auditors to replay the scoring pipeline deterministically.【crates/sorafs_orchestrator/src/incentives.rs:192】

## Treasury & XOR Integration

- Payout instructions reuse the XOR asset tracked by the bonding policy; no additional asset wiring
  is required. The orchestrator emits sealed instructions referencing `RelayRewardInstructionV1`
  so the treasury daemon can batch them into XOR transactions.
- `RelayPayoutService` (`crates/sorafs_orchestrator/src/treasury.rs`) bridges the reward engine with
  `iroha_core::soranet_incentives::RelayPayoutLedger`, records per-epoch payouts, materialises XOR
  transfers, manages `RelayRewardDisputeV1` lifecycles (credit/debit/no-change), and exposes
  dashboard-ready aggregates for the treasury daemon.【crates/sorafs_orchestrator/src/treasury.rs:1】【crates/iroha_core/src/soranet_incentives.rs:351】
- Operators can now drive the full payout workflow via `iroha app sorafs incentives service <subcommand>`:
  `init` seeds a Norito state file, `process` records epoch rewards, `record` ingests replayed
  instructions, `dispute` manages credits/debits, `dashboard` renders live earnings summaries,
  `daemon` polls a metrics spool to evaluate pending epochs, and `reconcile` compares the recorded
  payouts against XOR ledger exports. The CLI calls straight into `RelayPayoutService`, ensuring
  offline tooling stays aligned with the treasury daemon’s ledger rules.【crates/iroha_cli/src/commands/sorafs.rs:1116】
- The `daemon` subcommand consumes Norito snapshots from `--metrics-dir`, looks up relay metadata
  (beneficiary account and bond ledger entry) from a JSON config, emits reward/transfer instructions
  into caller-provided directories, and optionally archives processed snapshots. A minimal config
  looks like:

  ```json
  {
    "relays": [
      {
        "relay_id": "c4c2…ff2e",
        "beneficiary": "ih58...",
        "bond_path": "/var/lib/incentives/bonds/relay-c4c2ff2e.to"
      }
    ]
  }
  ```

  The daemon skips already-recorded epochs deterministically, so operators can re-run it after
  provisioning new metrics without clearing the spool.【crates/iroha_cli/src/commands/sorafs.rs:1625】

## Observability & Dashboards

- The relay metrics endpoint now exports per-epoch incentive counters keyed by relay fingerprint:
  - `soranet_relay_uptime_seconds_total{mode,relay,epoch}`
  - `soranet_relay_scheduled_seconds_total{mode,relay,epoch}`
  - `soranet_relay_bandwidth_verified_bytes_total{mode,relay,epoch}`
  - `soranet_relay_measurements_total{mode,relay,epoch}`
  - `soranet_relay_confidence_floor_per_mille{mode,relay,epoch}`
  - `soranet_reward_skips_total{relay,reason}` (cumulative skip counters keyed by skip reason such as `insufficient_bond`)
  - `soranet_reward_base_payout_nanos` (gauge capturing the configured XOR base payout per epoch)
- Snapshots of the running accumulator are persisted in Norito-encoded
  `RelayEpochMetricsV1` files under the configured incentive spool directory
  (default `artifacts/incentives/`), allowing auditors to replay the scoring
  pipeline deterministically with the raw inputs the relay observed.
- Operators can run `./check_pending_incentive_snapshots.sh --details` to audit
  the spool and highlight epochs missing either the uptime or measurement
  snapshots before shipping logs to treasury tooling.
- CLI tooling under `iroha_cli app sorafs incentives` now covers reward evaluation (`compute`),
  dispute authoring (`open-dispute`), and offline dashboard summaries so operators can reconcile
  payouts before the treasury daemon lands.【crates/iroha_cli/src/commands/sorafs.rs:929】
- Operators can pin the production Grafana dashboard shipped in
  `dashboards/grafana/soranet_incentives.json` to visualise uptime, bandwidth, measurement proofs,
  skip reasons, and payout health directly from Prometheus.
  The dashboard expects a Prometheus datasource and surfaces `mode` / `relay` template variables for
  multi-tenant environments. Treasury tooling can ingest the same metrics via
  `treasury::RelayPayoutService::earnings_dashboard()` when assembling operator reports.【crates/sorafs_orchestrator/src/treasury.rs:226】
- Dispute/credit telemetry is wired end-to-end: payout/dispute flows increment
  `soranet_reward_disputes_total` and `soranet_reward_adjustment_nanos_total`, and the dashboard
  row titled “Dispute Lifecycle / Credits & Debits” surfaces the rate of filed/rejected/resolved
  disputes plus the XOR volume of credit/debit adjustments so Treasury can spot claw-backs or
  dispute surges during rollout.【dashboards/grafana/soranet_incentives.json:1】【crates/sorafs_orchestrator/src/treasury.rs:439】
- Prometheus alerting rules live in `dashboards/alerts/soranet_incentives_rules.yml`, covering exit
  bond compliance and payout anomaly detection. Apply them alongside the existing SoraNet/SoraFS
  alert suites before enabling automatic payouts.

## Governance Approval Checklist

Before Sora Parliament authorises automatic payouts, the operations lead must confirm:

- [x] `RelayBondPolicyV1` and `RelayBondLedgerEntryV1` state is current for every exit relay (run
      `iroha app sorafs incentives service audit --scope bond`).
- [x] `increase(soranet_reward_skips_total{reason="insufficient_bond"}[24h]) == 0` on staging and the
      `SoranetRelayBondDeficit` alert stays green for at least 24 h.
- [x] Treasury reconciliation via `iroha app sorafs incentives service reconcile --window 3` reports
      zero missing transfers and dispute adjustments remain below the 5 % budget guardrail.
- [x] The Grafana dashboard `dashboards/grafana/soranet_incentives.json` is published to the shared
      Observatory instance with datasource bindings reviewed by SRE.
- [x] Prometheus loads `dashboards/alerts/soranet_incentives_rules.yml` and a test page shows both
      alerts in the `Inactive`/`Pending` state during staging drills.
- [x] A signed Parliament packet (bond policy, payout cap, rollback plan) is archived in Nexus
      governance alongside the Norito manifests referenced by the budget approval hash.
- [x] Daemon runs report `missing_budget_approval=0` for processed payouts and each summary row
      includes the hex `budget_approval_id`, keeping Treasury/Parliament audit trails aligned with
      the budget hash embedded in reward instructions. The CLI now rejects configs without the
      Parliament hash and fails daemon runs that encounter missing approvals, while the summaries
      print the hash for every payout row.【crates/iroha_cli/src/commands/sorafs.rs:3314】【crates/iroha_cli/src/commands/sorafs.rs:4470】

Running with `--allow-missing-budget-approval` is reserved for lab/staging replays; production
daemons/shadow-runs should keep the default enforcement so the `expected_budget_approval` hash stays
present and the `missing_budget_approval` / `mismatched_budget_approval` counters remain at zero.

Use `iroha app sorafs incentives service audit --scope all --config <daemon.json> --state <state.json>`
to gate both bond and budget readiness. The budget scope fails when a payout is missing or carrying
a mismatched `budget_approval_id` and echoes the configured Parliament hash in the JSON summary so
operators can lift the digest straight into governance packets.【crates/iroha_cli/src/commands/sorafs.rs:3122】【crates/iroha_cli/src/commands/sorafs.rs:12758】

## Risk Limits & Alerting

- **Exit bond tolerance:** the exit programme tolerates zero underfunded bonds. Any positive value of
  `increase(soranet_reward_skips_total{reason="insufficient_bond"}[10m])` triggers the
  `SoranetRelayBondDeficit` alert (critical) and automatic payouts must stay disabled until the bond
  ledger is restored.
- **Payout floor:** rewarded relays must accumulate at least 0.1 XOR of payouts across any rolling
  15 minute window where rewards are issued. Falling below this floor raises the
  `SoranetRewardPayoutAnomaly` alert (warning) and blocks the release checklist until treasury
  reconciliation explains the variance.
- **Dispute guardrail:** dispute adjustments must remain under 5 % of payouts per epoch. Track
  `increase(soranet_reward_adjustment_nanos_total[1h]) / increase(soranet_reward_payout_nanos_total[1h])`
  via the dashboard; crossing the guardrail escalates to Treasury before payouts resume.

## Rollout Steps

1. Wire the treasury daemon to `treasury::RelayPayoutService` so it ingests `RelayRewardInstructionV1`,
   verifies Norito payloads, and mints XOR payouts deterministically. _Status:_ Implemented via batched
   payout processing (`RelayPayoutService::process_batch`) and the `iroha app sorafs incentives service process`
   CLI batch mode that feeds bonded beneficiaries and reconciles multi-epoch exports.【crates/sorafs_orchestrator/src/treasury.rs#L202】【crates/iroha_cli/src/commands/sorafs.rs#L1261】【crates/iroha_cli/tests/cli_smoke.rs:1358】
2. Publish dashboards + SLOs for the incentives pipeline and run dry-runs on the staging network.
   _Status:_ Completed — dashboards/alerts are live (`dashboards/grafana/soranet_incentives.json`,
   `dashboards/alerts/soranet_incentives_rules.yml`), and the 60-day shadow replay report is published
   with signed artefacts for Parliament review.【docs/source/soranet/reports/incentive_shadow_run.md:1】
3. Secure Sora Parliament approval for the bonding/payout parameters and migrate the configuration
   into production manifests. _Status:_ Completed — the approval packet carries the budget hash and
   `RewardConfig` now refuses to start without `budget_approval_id`, so payout daemons fail closed
   until Parliament issues a new digest; rollback keeps using the null hash to halt automation.【crates/sorafs_orchestrator/src/incentives.rs:51】【docs/examples/soranet_incentive_parliament_packet/rollback_plan.md:1】【docs/source/soranet/reports/incentive_parliament_packet.md:1】

## Implementation Checklist

- [x] Model bonding, bandwidth proof, epoch metrics, and reward instruction payloads in the SoraNet
  data model.
- [x] Implement relay-side aggregation (`tools/soranet-relay/src/incentives.rs`), runtime reward
  calculator (`crates/iroha_core/src/soranet_incentives.rs`), and orchestrator reward engine
  (`crates/sorafs_orchestrator/src/incentives.rs`) with unit coverage.
- [x] Integrate the accumulator and reward engine into the treasury pipeline via
  `treasury::RelayPayoutService`, providing XOR transfer generation, dispute tracking, and earnings dashboards.【crates/sorafs_orchestrator/src/treasury.rs:36】
- [x] Publish observability assets (metrics, dashboards, alerts) and treasury runbooks (telemetry
  counters landed in `iroha_telemetry`; dashboards now live under
  `dashboards/grafana/soranet_incentives.json`; alert rules ship in
  `dashboards/alerts/soranet_incentives_rules.yml`; runbook updates below).
