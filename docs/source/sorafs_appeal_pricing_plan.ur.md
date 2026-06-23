---
lang: ur
direction: rtl
source: docs/source/sorafs_appeal_pricing_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9f73be34bae18420b53954733c1b5e732bac698962e3413911c6573a13297e78
source_last_modified: "2026-01-03T18:07:58.704525+00:00"
translation_last_reviewed: 2026-01-30
---

# Moderation Appeal Pricing Engine

## Current Status

SFM-4b2 has shipped deterministic appeal finance foundations in
`crates/sorafs_orchestrator/src/appeals.rs` and the `sorafs_cli appeal`
operator commands. Torii now exposes read-only appeal finance config, status,
and quote REST APIs backed by the same baseline pricing helper. The repo does
not yet ship a standalone pricing daemon, mutating deposit/report APIs, or an
on-chain escrow contract. Treat this page as the production-readiness ledger for
the implemented helpers and the remaining service gates.

## Shipped Foundations

- `AppealPricingConfig::baseline_v1()` implements the congestion-aware deposit
  formula for `content`, `access`, `fraud`, and `other` appeal classes.
- `AppealPricingConfig::from_manifest_value` loads governance-managed JSON
  manifests such as
  `docs/examples/ministry/appeal_pricing_config_baseline.json`.
- `AppealSettlementConfig::baseline_v1()` and
  `AppealSettlementConfig::from_manifest_value` calculate refund, treasury,
  escrow holdback, panel reward, and no-show forfeiture amounts from
  `docs/examples/ministry/appeal_settlement_config_baseline.json`.
- `AppealSettlementConfig::disburse` emits account-aware payout plans with
  refund, treasury, escrow, and per-juror reward lines.
- `sorafs_cli appeal quote`, `sorafs_cli appeal settle`, and
  `sorafs_cli appeal disburse` expose the deterministic calculator for
  moderation, treasury, QA, and governance evidence workflows.

## Pricing Model

The shipped quote helper applies the roadmap formula:

```text
base = class_base_rate[class]
backlog_factor = min(backlog / backlog_target[class], backlog_cap[class])
size_multiplier = 1 + min(evidence_size_mb / size_divisor[class], size_cap[class])
urgency_multiplier = { normal: 1.0, high: 1.2 }
panel_multiplier = panel_size / default_panel_size
deposit = base * (1 + backlog_factor) * size_multiplier * urgency_multiplier * panel_multiplier * surge_multiplier
deposit = clamp(deposit, min_deposit[class], max_deposit[class])
```

Default baseline parameters are content 150 XOR, access 200 XOR, fraud 500 XOR,
and other 120 XOR, with class-specific backlog targets, size divisors, and
deposit caps encoded in `AppealPricingConfig::baseline_v1()`. Governance
manifests may override those parameters without changing the CLI or library.

## Torii Read-Only API

The app API publishes the deterministic baseline through JSON endpoints:

- `GET /v1/sorafs/appeals/pricing/config` returns the active baseline config,
  quote TTL, default panel size, and class parameters.
- `GET /v1/sorafs/appeals/pricing/status` reports that config and quote APIs are
  enabled, while deposit, report, and settlement processing remain pending
  runtime escrow and ledger integration.
- `POST /v1/sorafs/appeals/pricing/quote` accepts `class`, `backlog`,
  `evidence_size_mb`, optional `urgency`, and optional `panel_size`, then returns
  the deposit and multiplier breakdown.

Example quote request:

```sh
curl -sS http://127.0.0.1:8080/v1/sorafs/appeals/pricing/quote \
  -H 'content-type: application/json' \
  -d '{"class":"content","backlog":28,"evidence_size_mb":45,"urgency":"normal","panel_size":7}'
```

## Operator Commands

Quote a deposit from the baseline config:

```sh
cargo run --locked -p sorafs_orchestrator --bin sorafs_cli -- \
  appeal quote \
  --class=content \
  --backlog=28 \
  --evidence-mb=45 \
  --urgency=normal \
  --panel-size=7 \
  --format=json
```

Quote using a governance manifest:

```sh
cargo run --locked -p sorafs_orchestrator --bin sorafs_cli -- \
  appeal quote \
  --class=fraud \
  --backlog=9 \
  --evidence-mb=12 \
  --panel-size=7 \
  --format=json \
  --config=docs/examples/ministry/appeal_pricing_config_baseline.json
```

Settle a resolved deposit:

```sh
cargo run --locked -p sorafs_orchestrator --bin sorafs_cli -- \
  appeal settle \
  --deposit=250 \
  --outcome=overturn \
  --panel-size=7 \
  --format=json \
  --config=docs/examples/ministry/appeal_settlement_config_baseline.json
```

Generate a full payout plan:

```sh
cargo run --locked -p sorafs_orchestrator --bin sorafs_cli -- \
  appeal disburse \
  --deposit=250 \
  --outcome=overturn \
  --refund-account=appellant \
  --treasury-account=treasury \
  --escrow-account=appeal_escrow \
  --juror=juror_a \
  --juror=juror_b \
  --juror=juror_c \
  --juror=juror_d \
  --juror=juror_e \
  --juror=juror_f \
  --juror=juror_g \
  --panel-size=7 \
  --format=json \
  --config=docs/examples/ministry/appeal_settlement_config_baseline.json
```

Use `--config=-` when automation streams the governance manifest over stdin.
Archive the JSON output next to the appeal evidence bundle so later audits can
replay the exact parameters used for the quote or payout.

## Remaining Production Gates

- Ship the mutating deposit and report API surface, or a daemon, once the
  runtime escrow and ledger workflow exists.
- Implement the escrow contract or ledger workflow that actually locks, refunds,
  slashes, and releases XOR according to the calculated plan.
- Wire moderation decision events into a settlement processor that produces
  signed treasury entries instead of local CLI-only output.
- Publish weekly appeal finance reports to the Governance DAG and transparency
  dashboards.
- Add end-to-end tests that cover quote creation, deposit posting, decision
  ingestion, disbursement, and treasury reconciliation against the runtime ledger.

## Validation

Focused Rust coverage lives with the helper implementations and the
`sorafs_cli` tests. For this page, the minimum refresh check is:

```sh
cargo test -p sorafs_orchestrator appeal
```

Run broader SoraFS CLI tests when changing command arguments, manifest parsing,
or JSON/table output.
