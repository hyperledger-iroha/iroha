---
lang: zh-hant
direction: ltr
source: docs/examples/soranet_incentive_parliament_packet/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 29808ff4511c668963b3c8c4326cca49e033bea91b1b9aa56968ef494648f18e
source_last_modified: "2026-01-22T14:35:37.885694+00:00"
translation_last_reviewed: 2026-02-07
---

# SoraNet Relay Incentive Parliament Packet

This bundle captures the artefacts required by Sora Parliament to approve
automatic relay payouts (SNNet-7):

- `reward_config.json` - Norito-serialisable reward engine configuration, ready
  to be ingested by `iroha app sorafs incentives service init`. The
  `budget_approval_id` matches the hash listed in the governance minutes.
- `shadow_daemon.json` - beneficiary and bond mapping consumed by the replay
  harness (`shadow-run`) and the production daemon.
- `economic_analysis.md` - fairness summary for the 2025-10 -> 2025-11
  shadow simulation.
- `rollback_plan.md` - operational playbook for disabling automatic payouts.
- Supporting artefacts: `docs/examples/soranet_incentive_shadow_run.{json,pub,sig}`,
  `dashboards/grafana/soranet_incentives.json`,
  `dashboards/alerts/soranet_incentives_rules.yml`.

## Integrity Checks

```bash
shasum -a 256 docs/examples/soranet_incentive_parliament_packet/* \
  docs/examples/soranet_incentive_shadow_run.json \
  docs/examples/soranet_incentive_shadow_run.sig
```

Compare the digests with the values recorded in the Parliament minutes. Verify
the shadow-run signature as described in
`docs/source/soranet/reports/incentive_shadow_run.md`.

## Updating the Packet

1. Refresh `reward_config.json` whenever the reward weights, base payout, or
   approval hash change.
2. Re-run the 60-day shadow simulation, update `economic_analysis.md` with the
   new findings, and commit the JSON + detached signature pair.
3. Present the updated bundle to Parliament together with Observatory dashboard
   exports when seeking renewed approval.
