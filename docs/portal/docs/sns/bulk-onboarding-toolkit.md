---
title: Atomic Alias Setup Toolkit
sidebar_label: Bulk alias setup
description: Plan and submit a typed alias dependency graph without handling keys or payment proofs.
---

# Atomic Alias Setup Toolkit

`scripts/sns_bulk_onboard.py` validates typed `EnsureAlias` intents and
delegates both planning and apply to the Rust CLI. It never calls direct SNS
mutation routes, creates settlement proofs, accepts token/key command-line
values, or splits the vector.

The JSON document contains version `1` and the groups `dataspaces`, `domains`,
and `accounts`. Every group item has `intent`, `acquisition`, and `quote_guard`.
Groups are flattened in that dependency order into one
`AliasSetupPlanRequestV1`.

```bash
python3 scripts/sns_bulk_onboard.py setup.json \
  --config client.toml \
  --iroha-cli ./target/release/iroha \
  --plan-file setup.plan.json \
  --plan-only
```

Omit `--plan-only` to locally verify, sign, and submit the exact framed vector
in one ordinary transaction. A conflict returns no plan; no partial transaction
is ever submitted. Exact replays are free no-ops and repairs carry no lease
charge.

Intent and plan files are secret-free. Signing material remains in the client
configuration's runtime-only key source. Sponsored-onboarding tokens belong in
headers or token files, never in these documents or command-line values.
