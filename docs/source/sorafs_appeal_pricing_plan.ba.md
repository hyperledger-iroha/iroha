---
lang: ba
direction: ltr
source: docs/source/sorafs_appeal_pricing_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2d21426a144d9c1bf189920341c501a55cfe6918a1aa25143830aa5f0b7250eb
source_last_modified: 2026-06-24T07:58:22.808631Z
translation_last_reviewed: 2026-02-07
title: Moderation Appeal Pricing Engine
summary: SFM-4b2 implementation status for appeal quote, settlement, and disbursement helpers plus the remaining escrow and service gates.
---

# Moderation Appeal Pricing Engine

## Current Status

SFM-4b2 has shipped deterministic appeal finance foundations in
`crates/sorafs_orchestrator/src/appeals.rs` and the `sorafs_cli appeal`
operator commands. Torii now exposes read-only appeal finance config/status/
quote REST APIs, an authenticated native asset-lock deposit instruction builder
with status, confirmation, settlement-execution instruction checks, and
post-submission settlement reconciliation against runtime asset-lock records
with deterministic audit digests for peer/operator comparison, plus a
configured-signer settlement submitter that queues the next pending native
`DrawdownAssetLock` or `CancelAssetLock` transaction and publishes a typed
settlement submission receipt to the local Governance DAG when a publisher is
configured,
plus stateless settlement and disbursement plan APIs backed by the same baseline
helpers, and `sorafs_node` can publish typed
`SoraFsAppealFinanceReportV1` records into the local Governance DAG filesystem
sink and optional signed runtime DAG. Deposit-backed local moderation tallies now
derive and publish those finance reports automatically from the confirmed
asset-lock metadata captured at ballot intake. `sorafs_manifest` can also
aggregate validated finance reports into deterministic
`SoraFsAppealFinanceWeeklyRollupV1` records for transparency dashboards and
treasury review, and `sorafs_node` can publish those rollups through the same
local Governance DAG filesystem and optional signed runtime DAG path. Torii
exposes read-only local report, weekly rollup, and settlement receipt dashboard
APIs backed by the Governance DAG publish-index plus canonical-auth report and
weekly rollup publish APIs for the local Governance DAG pipeline. The local
dashboard APIs report full aggregate totals while bounding returned source-entry
arrays through `limit` with a default of 50 and max of 500. Torii also exposes
authenticated deposit-status and deposit-confirmation lookups for native runtime
asset-lock records returned to the lock opener, destination, or release
authority.
The repository also ships a Grafana/Prometheus appeal-finance dashboard and
alert pack over those Governance DAG publication metrics for rollout monitoring.
The repo still does not ship a standalone pricing daemon. Deposit custody uses
the returned native `OpenAssetLock` instruction, which the authenticated payer
must sign and submit through the normal transaction path, then external
moderation intake can call Torii's confirmation endpoint before advancing the
case. Settlement custody mutation can either use the returned
`DrawdownAssetLock` and `CancelAssetLock` instructions for required-authority
client signing or Torii's configured signer submitter at
`POST /v1/sorafs/appeals/finance/deposits/submit-settlement`. The submitter
requires `torii.sorafs.appeal_finance_settlement.submitter_private_keys` and
only queues the next pending step whose required authority has a configured
private key. If `sorafs_node` has a local Governance DAG publisher, each queued
server-side step is also recorded as a typed settlement receipt for audit and
dashboard ingestion. Torii's local moderation ballot announcement API now
performs the deposit confirmation gate before admitting a ballot and stores the
confirmed deposit fingerprint, including evidence hashes, with the local
moderation record. When Torii starts with configured settlement submitter keys,
its moderation settlement worker replays and subscribes to local tallied ballot
events, reconstructs the confirmed deposit from that captured fingerprint, and
queues pending native settlement steps through the same submitter and receipt
path. The worker also periodically rescans tallied local ballots at
`torii.sorafs.appeal_finance_settlement.worker_scan_interval_ms` so changed
runtime reconciliation state can advance to the next pending settlement step.
It persists each worker-submitted step with transaction hash, status, attempts,
and last error under the SoraFS storage data directory, refreshes queued
transaction status from Torii's local pipeline cache, and retries rejected or
expired worker submissions up to
`torii.sorafs.appeal_finance_settlement.worker_max_retry_attempts`.
Treat this page as the production-readiness ledger for the
implemented helpers and the remaining service gates.

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
- `SoraFsAppealFinanceReportV1` records account-aware refund, treasury,
  held-escrow, juror payout, and no-show lines for Governance DAG publication.
- `NodeHandle::publish_appeal_finance_report` and
  `FilesystemGovernancePublisher::publish_appeal_finance_report` validate and
  publish those reports as canonical `.to` payloads, JSON mirrors, BLAKE3
  sidecars, `publish-index.json` entries, local CAR queue segments, and optional
  signed runtime DAG blocks.
- `SoraFsAppealFinanceWeeklyRollupV1::from_reports` validates source reports,
  rejects duplicate report ids, and emits deterministic weekly totals by
  outcome, case count, config version, juror payout count, no-show count,
  refund, treasury, held escrow, rewards paid, and forfeited rewards.
- `NodeHandle::publish_appeal_finance_weekly_rollup` and
  `FilesystemGovernancePublisher::publish_appeal_finance_weekly_rollup` publish
  weekly rollups as canonical `.to` payloads, JSON mirrors, BLAKE3 sidecars,
  `publish-index.json` entries, local CAR queue segments, and optional signed
  runtime DAG blocks.
- `SoraFsAppealFinanceSettlementReceiptV1` records server-submitted native
  settlement steps with the queued transaction hash, required authority,
  reconciliation digest, observed ledger state, and settlement amounts.
- `NodeHandle::publish_appeal_finance_settlement_receipt` and
  `FilesystemGovernancePublisher::publish_appeal_finance_settlement_receipt`
  publish settlement receipts as canonical `.to` payloads, JSON mirrors, BLAKE3
  sidecars, `publish-index.json` entries, local CAR queue segments, and optional
  signed runtime DAG blocks under `appeal_finance_settlement_receipt`.
- `GET /v1/sorafs/appeals/finance/settlement-receipts` summarizes locally
  published settlement receipts from the Governance DAG publish-index for
  operator dashboards, including submitted-step counts, reconciliation-status
  counts, distinct case counts, and settlement totals. Aggregate totals are
  computed over the full local index, and the returned `entries` array is
  bounded by `limit` with a default of 50 and max of 500.
- `GET /v1/sorafs/appeals/finance/weekly-rollups` summarizes locally published
  weekly rollups from the Governance DAG publish-index for operator dashboards.
  Aggregate totals are computed over the full local index, and the returned
  `entries` array is bounded by `limit` with a default of 50 and max of 500.
- `GET /v1/sorafs/appeals/finance/reports` summarizes locally published
  appeal finance reports from the Governance DAG publish-index for operator
  dashboards, including outcome counts, distinct case counts, payout/no-show
  counts, finance totals, and source entries. Aggregate totals are computed over
  the full local index, and the returned `entries` array is bounded by `limit`
  with a default of 50 and max of 500.
- `dashboards/grafana/sorafs_appeal_finance.json` and
  `dashboards/alerts/sorafs_appeal_finance_rules.yml` track appeal-finance
  report/weekly-rollup/settlement-receipt publication freshness, publication
  failures, payload throughput, rollup lag, receipt/report lag, and Governance
  DAG backlog.
- `POST /v1/sorafs/appeals/finance/reports` and
  `POST /v1/sorafs/appeals/finance/weekly-rollups` require canonical
  `X-Iroha-*` request authentication and publish validated report/rollup JSON
  into the configured local Governance DAG publisher.
- `POST /v1/sorafs/appeals/finance/deposits` requires canonical `X-Iroha-*`
  request authentication, verifies that `payer_account` matches the
  authenticated account, derives a stable appeal escrow id from the case,
  payer, destination, optional release authority, amount, asset, optional
  expiry, evidence hashes, and idempotency key, and returns a canonical native
  `OpenAssetLock` instruction for client-side signing and submission.
- `GET /v1/sorafs/appeals/finance/deposits/{escrow_id_hex}` requires canonical
  `X-Iroha-*` request authentication and returns the native runtime lock record
  only when the authenticated account is the lock opener, destination, or
  release authority.
- `POST /v1/sorafs/appeals/finance/deposits/confirm` requires canonical
  `X-Iroha-*` request authentication, re-derives the expected appeal deposit
  escrow id from normalized request parameters, checks the runtime
  `AssetEscrowRecord`, and confirms only locked `OpenAssetLock` custody whose
  payer, destination, release authority, asset, amount, expiry, and evidence
  hashes still match the submitted appeal deposit.
- `POST /v1/sorafs/appeals/finance/deposits/settle` requires canonical
  `X-Iroha-*` request authentication, confirms the same runtime deposit lock,
  computes the baseline settlement breakdown for the requested outcome, and
  returns ordered native `DrawdownAssetLock` and `CancelAssetLock` instructions
  with the required signer account for each client-submitted ledger mutation.
- `POST /v1/sorafs/appeals/finance/deposits/submit-settlement` requires
  canonical `X-Iroha-*` request authentication, recomputes the same settlement
  expectation, selects the next pending native settlement step, signs it with a
  configured submitter key whose canonical `AccountId` matches the step's
  required authority, and queues the transaction through Torii. Without
  configured signer coverage it returns `submitter_not_configured` or
  `missing_required_signer` alongside reconciliation evidence. The internal
  signing, queueing, reconciliation, and settlement-receipt publication path is
  factored out of the HTTP handler and is also used by the always-on
  moderation-derived worker.
- The moderation-derived settlement worker starts with Torii when SoraFS storage
  and `torii.sorafs.appeal_finance_settlement.submitter_private_keys` are
  configured. It replays the local moderation event backlog, subscribes to live
  `BallotTallied` events, reconstructs the deposit confirmation from the
  moderation-captured native lock fingerprint including evidence hashes, checks
  the runtime ledger for static deposit mismatches, maps the tally decision to
  the appeal settlement outcome, and queues the same next pending native
  settlement step plus settlement receipt as the authenticated submit endpoint.
  It also rescans tallied local ballots on `worker_scan_interval_ms` and
  persists already-queued worker submissions under the SoraFS storage data
  directory with transaction hash, pipeline status, attempt count, and last
  error. Follow-up scans and process restarts can submit a new step after ledger
  state advances without resubmitting the unchanged state, and rejected or
  expired worker transactions are retried until `worker_max_retry_attempts` is
  reached.
- `POST /v1/sorafs/appeals/finance/deposits/reconcile` requires canonical
  `X-Iroha-*` request authentication, recomputes the same baseline settlement
  expectation, reads the current native asset-lock ledger record, and reports
  whether settlement is still pending client submission, waiting for the refund
  cancellation step, fully reconciled, or mismatched. Each response includes
  `reconciliation_digest_hex`, a BLAKE3 digest over the config version,
  requested settlement inputs, expected result, observed ledger state, and
  ordered mismatch list.
- `POST /v1/sorafs/moderation/ballots` requires a matching
  `deposit_confirmation` object and rejects the announcement unless Torii can
  confirm runtime ledger custody for the same case, round, and evidence bundle
  before the local ballot is admitted.
- Deposit-backed local moderation ballot tallies now derive a deterministic
  `SoraFsAppealFinanceReportV1` from the final decision, confirmed deposit
  snapshot, evidence-hash fingerprint, panel roster, revealed jurors, and
  no-show jurors, then publish it through the same local Governance DAG report
  pipeline used by operator submissions.
- `SorafsReconciliationReportV1` can embed an appeal-finance reconciliation
  summary derived from local weekly rollup publish-index entries and JSON
  sidecars, including source report count, case count, treasury-bound XOR, and
  forfeited reward XOR.

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

## Torii API

The app API publishes the deterministic baseline through JSON endpoints:

- `GET /v1/sorafs/appeals/pricing/config` returns the active baseline config,
  quote TTL, default panel size, and class parameters.
- `GET /v1/sorafs/appeals/pricing/status` reports that config and quote APIs are
  enabled, the authenticated native asset-lock deposit instruction/status/
  confirmation APIs, settlement-execution instruction builder, and
  post-submission settlement reconciliation API with deterministic audit digests
  plus configured-signer next-step settlement submission with local Governance
  DAG settlement receipt publication, moderation-derived next-step settlement
  retry-aware worker with configured follow-up scan interval and attempt cap, and
  a local receipt dashboard API are enabled, stateless
  settlement/disbursement plan APIs are enabled, local Governance DAG
  report/weekly rollup publication plus the local report and weekly rollup
  dashboard APIs and authenticated report/rollup publish APIs are enabled.
- `POST /v1/sorafs/appeals/pricing/quote` accepts `class`, `backlog`,
  `evidence_size_mb`, optional `urgency`, and optional `panel_size`, then returns
  the deposit and multiplier breakdown.
- `POST /v1/sorafs/appeals/finance/settle` accepts `deposit_xor`, `outcome`, and
  optional `panel_size`, then returns the refund, treasury, held-escrow, and
  panel-reward breakdown for the active baseline settlement config.
- `POST /v1/sorafs/appeals/finance/disburse` accepts the same settlement inputs
  plus canonical refund, treasury, escrow, juror, and optional no-show account
  ids, then returns the deterministic per-account payout plan. This endpoint is
  a calculator/reporting surface only; it does not mutate escrow or ledger
  state.
- `POST /v1/sorafs/appeals/finance/deposits` accepts `case_id`, optional
  `round_id`, canonical `payer_account`, canonical `destination_account`,
  optional `release_authority_account`, canonical `asset_definition_id`,
  `deposit_xor`, optional `expires_at_ms`, an `idempotency_key`, and optional
  `evidence_hashes_hex`, then returns `escrow_id_hex` plus a framed
  `OpenAssetLock` transaction
  instruction. The request must be signed by the payer with canonical app
  authentication; ledger mutation occurs only after the client submits a signed
  transaction containing the returned instruction.
- `GET /v1/sorafs/appeals/finance/deposits/{escrow_id_hex}` checks the runtime
  native asset-lock ledger and returns lifecycle status, opener/destination,
  release authority, asset, amount, remaining custody, evidence hashes, custody
  account, timestamps, and optional resolution. The request must be signed by
  the lock opener, destination, or release authority.
- `POST /v1/sorafs/appeals/finance/deposits/confirm` accepts the same normalized
  deposit parameters as the builder plus `escrow_id_hex`, requires canonical app
  authentication, verifies that the supplied escrow id matches the derived id,
  and confirms that the visible runtime `AssetEscrowRecord` is still a locked
  native asset lock with full remaining custody for the expected appeal deposit.
  A visible but mismatched ledger record returns a conflict response with the
  mismatch list.
- `POST /v1/sorafs/appeals/finance/deposits/settle` accepts a
  `deposit_confirmation`, `outcome`, and optional `panel_size`, confirms the
  visible runtime deposit lock, and returns ordered native settlement
  instructions. Non-refunded custody is emitted as a `DrawdownAssetLock` to the
  lock destination; refundable custody is emitted as a `CancelAssetLock` back to
  the lock opener. Each step includes the required signer account and framed
  instruction payload for client-side transaction submission.
- `POST /v1/sorafs/appeals/finance/deposits/submit-settlement` accepts the same
  `deposit_confirmation`, `outcome`, and optional `panel_size`, then signs and
  queues exactly one next pending settlement step when
  `torii.sorafs.appeal_finance_settlement.submitter_private_keys` contains a key
  for that step's required authority. It returns `queued` with `tx_hash_hex`, or
  an explicit signer/configuration status with the same reconciliation evidence.
  When a local Governance DAG publisher is configured, a queued submission also
  publishes an `appeal_finance_settlement_receipt` artifact and returns its
  publication status, receipt id, transaction hash, and reconciliation digest.
- `GET /v1/sorafs/appeals/finance/settlement-receipts` returns the local
  settlement receipt publication summary from `publish-index.json`, with totals
  by submitted settlement step and reconciliation status plus source entries for
  dashboard drill-downs. The response includes published/returned counts,
  applied `limit`, and a truncation flag for the bounded `entries` array.
- `POST /v1/sorafs/appeals/finance/deposits/reconcile` accepts the same
  `deposit_confirmation`, `outcome`, and optional `panel_size`, then compares
  the current runtime asset-lock ledger record with the expected settlement
  result. It returns `pending_client_submission`, `awaiting_refund_cancel`,
  `settled`, or `mismatch` with expected and observed lifecycle/remaining
  amounts, mismatch details, and `reconciliation_digest_hex` for deterministic
  audit comparison across operators or peers.
- `POST /v1/sorafs/appeals/finance/reports` accepts a
  `SoraFsAppealFinanceReportV1` JSON payload and publishes it to the configured
  local Governance DAG filesystem/runtime publication pipeline. The request must
  be signed with canonical app authentication.
- `GET /v1/sorafs/appeals/finance/reports` returns the local published report
  count, outcome summaries, distinct case count, juror payout and no-show
  counts, finance totals, latest publication timestamp, and matching
  publish-index entries. The response includes published/returned counts,
  applied `limit`, and a truncation flag for the bounded `entries` array.
- `GET /v1/sorafs/appeals/finance/weekly-rollups` returns the local published
  weekly rollup count, cycle summaries, source-report totals, latest publication
  timestamp, and matching publish-index entries. The response includes
  published/returned counts, applied `limit`, and a truncation flag for the
  bounded `entries` array.
- `POST /v1/sorafs/appeals/finance/weekly-rollups` accepts a
  `SoraFsAppealFinanceWeeklyRollupV1` JSON payload and publishes it through the
  same authenticated local Governance DAG pipeline.

Example quote request:

```sh
curl -sS http://127.0.0.1:8080/v1/sorafs/appeals/pricing/quote \
  -H 'content-type: application/json' \
  -d '{"class":"content","backlog":28,"evidence_size_mb":45,"urgency":"normal","panel_size":7}'
```

Example stateless settlement request:

```sh
curl -sS http://127.0.0.1:8080/v1/sorafs/appeals/finance/settle \
  -H 'content-type: application/json' \
  -d '{"deposit_xor":"250","outcome":"overturn","panel_size":7}'
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

- Wire the checked-in receipt-aware appeal-finance dashboard and alert pack to
  hosted live/public observability once the public Governance DAG and ledger
  reconciliation paths exist.
- Add end-to-end tests that cover quote creation, deposit posting, decision
  ingestion, settlement submission, disbursement, and treasury reconciliation
  against a multi-peer runtime ledger.

## Validation

Focused Rust coverage lives with the helper implementations and the
`sorafs_cli` tests. For this page, the minimum refresh check is:

```sh
cargo test -p sorafs_orchestrator appeal
CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/iroha-codex-sorafs-appeal-dashboard-limit cargo test -j 1 -p iroha_torii appeal_finance_dashboards_limit --lib --features app_api -- --nocapture
```

Run broader SoraFS CLI tests when changing command arguments, manifest parsing,
or JSON/table output.
