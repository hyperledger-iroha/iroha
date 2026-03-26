---
lang: uz
direction: ltr
source: docs/source/soranet/reports/incentive_parliament_packet.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 260ded7edf9cd6721d4258846b0b2303920ed335c9e8fa1d12181c649f7fe7fe
source_last_modified: "2026-01-22T16:26:46.593445+00:00"
translation_last_reviewed: 2026-02-07
title: SoraNet Relay Incentive Parliament Packet
summary: Artefact bundle and verification flow required for Sora Parliament to approve automatic relay payouts.
---

# Overview

Automatic relay payouts (SNNet-7) must be explicitly authorised by Sora
Parliament. This document inventories the artefacts attached to the approval
packet, explains how to verify their integrity, and records the rollback steps
agreed with Treasury and Operations. The packet ships alongside the 60-day
shadow simulation so Parliament can reason about fairness, latency impact, and
abuse resistance before enabling automation.

## Artefact Inventory

- `docs/examples/soranet_incentive_parliament_packet/reward_config.json` -
  canonical Norito configuration for the reward engine. The current approval
  hash is `4f1a7b86d6c16245d9b5c0e9bd4732a6d01356f3172bbfa5ef5d9cde8790f221`
  (populate `RewardConfig::budget_approval_id` with this digest).
- `docs/examples/soranet_incentive_parliament_packet/shadow_daemon.json` -
  beneficiary + bond mapping consumed by both the replay harness and the
  production payout daemon.
- `docs/examples/soranet_incentive_parliament_packet/economic_analysis.md` -
  fair-distribution analysis for the 2025-10 -> 2025-11 shadow simulation.
- `docs/examples/soranet_incentive_parliament_packet/rollback_plan.md` -
  operational rollback playbook for suspending automatic payouts.
- `docs/examples/soranet_incentive_parliament_packet/README.md` -
  quick-start guide for auditors, including checksum instructions.
- Signed shadow-run report:
  `docs/examples/soranet_incentive_shadow_run.{json,pub,sig}` (detailed in
  `docs/source/soranet/reports/incentive_shadow_run.md`).
- Observability assets:
  `dashboards/grafana/soranet_incentives.json` and
  `dashboards/alerts/soranet_incentives_rules.yml`.

## Verification Workflow

1. **Integrity hashes** - run `shasum -a 256` on every artefact listed above and
   compare with the checks recorded in the Parliament minutes.
2. **Signature validation** - verify the detached signature of the shadow-run
   JSON as described in the shadow-run report. Confirm the embedded
   `report_metadata.git_revision` matches the commit under review.
3. **Config sanity** - run
   `iroha app sorafs incentives service init --state /tmp/incentives-test.json --config docs/examples/soranet_incentive_parliament_packet/reward_config.json --treasury-account soraカタカナ... --force`
   (then delete the temporary file) to ensure the reward configuration parses
   and passes calculator invariants.
4. **Replay confirmation** - execute
   `iroha app sorafs incentives service shadow-run --state <state.json> --config shadow_daemon.json --metrics-dir <telemetry> --report-out /tmp/run.json`
   and confirm the generated summary equals the signed artefact. Any divergence
   invalidates the packet.

## Submission Checklist

- Treasury reconciliation shows no missing or duplicate payouts for the shadow
  window.
- Alert suites (`soranet_incentives_rules.yml`) are deployed to Observatory and
  screenshots are attached to the governance minutes.
- Parliament minutes include:
  - the approval hash listed above,
  - SHA-256 sums of the artefact bundle,
  - confirmation that the rollback playbook was reviewed.
- The rollout owner has acknowledged the rollback plan and the criteria for
  re-enabling automation.

## Change Management

- Update this packet whenever the reward configuration, approval hash, or fleet
  composition changes materially.
- Ship a fresh 60-day replay and update the economic analysis before requesting
  renewed approval.
- Archive superseded packets under `docs/examples/soranet_incentive_parliament_packet/`
  with commit hashes to maintain a full governance trail.
