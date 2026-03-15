---
lang: kk
direction: ltr
source: docs/source/sorafs/reports/sf2c_capacity_soak.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 09567fc0280e726bdd1f2f1289dc98547ac70db9b19324ef5e413c2cff34de80
source_last_modified: "2025-12-29T18:16:36.125877+00:00"
translation_last_reviewed: 2026-02-07
---

# SF-2c Capacity Accrual Soak Report

Date: 2026-03-21

## Scope

This report records the deterministic SoraFS capacity accrual and payout soak
tests requested under the SF-2c roadmap track.

- **30-day multi-provider soak:** Exercised by
  `capacity_fee_ledger_30_day_soak_deterministic` in
  `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`.
  The harness instantiates five providers, spans 30 settlement windows, and
  validates that ledger totals match an independently computed reference
  projection. The test emits a Blake3 digest (`capacity_soak_digest=...`) so
  CI can capture and diff the canonical snapshot.
- **Under-delivery penalties:** Enforced by
  `record_capacity_telemetry_penalises_persistent_under_delivery`
  (same file). The test confirms strike thresholds, cooldowns, collateral slashes,
  and ledger counters remain deterministic.

## Execution

Run the soak validations locally with:

```bash
cargo test -p iroha_core -- record_capacity_telemetry_penalises_persistent_under_delivery
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
```

The tests complete in under one second on a standard laptop and require no
external fixtures.

## Observability

Torii now exposes provider credit snapshots alongside fee ledgers so dashboards
can gate on low balances and penalty strikes:

- REST: `GET /v1/sorafs/capacity/state` returns `credit_ledger[*]` entries that
  mirror the ledger fields verified in the soak test. See
  `crates/iroha_torii/src/sorafs/registry.rs`.
- Grafana import: `dashboards/grafana/sorafs_capacity_penalties.json` plots the
  exported strike counters, penalty totals, and bonded collateral so on-call
  staff can compare soak baselines with live environments.

## Follow-up

- Schedule weekly gate runs in CI to replay the soak test (smoke-tier).
- Extend the Grafana board with Torii scrape targets once production telemetry
  exports go live.
