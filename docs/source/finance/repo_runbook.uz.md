---
lang: uz
direction: ltr
source: docs/source/finance/repo_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d5dd8e1b666be34bb9101898d355fe5e3c6efc32500c238c72a6ef9228c157f0
source_last_modified: "2026-01-22T16:26:46.568155+00:00"
translation_last_reviewed: 2026-02-07
---

# Repo Settlement Runbook

This guide documents the deterministic flow for repo and reverse-repo agreements in Iroha.
It covers CLI orchestration, SDK helpers, and the expected governance knobs so operators can
initiate, margin, and unwind agreements without writing raw Norito payloads. For governance
checklists, evidence capture, and fraud/rollback procedures see
[`repo_ops.md`](./repo_ops.md), which satisfies roadmap item F1.

## CLI commands

The `iroha app repo` command groups repo-specific helpers:

```bash
# Stage an initiation instruction without submitting
iroha --config client.toml --output \
  repo initiate \
  --agreement-id daily_repo \
  --initiator ih58... \
  --counterparty ih58... \
  --custodian ih58... \
  --cash-asset usd#wonderland \
  --cash-quantity 1000 \
  --collateral-asset bond#wonderland \
  --collateral-quantity 1050 \
  --rate-bps 250 \
  --maturity-timestamp-ms 1704000000000 \
  --haircut-bps 1500 \
  --margin-frequency-secs 86400

# Generate the unwind leg
iroha --config client.toml --output \
  repo unwind \
  --agreement-id daily_repo \
  --initiator ih58... \
  --counterparty ih58... \
  --cash-asset usd#wonderland \
  --cash-quantity 1005 \
  --collateral-asset bond#wonderland \
  --collateral-quantity 1055 \
  --settlement-timestamp-ms 1704086400000

# Inspect the next margin checkpoint for an active agreement
iroha --config client.toml repo margin --agreement-id daily_repo

# Trigger a margin call when cadence elapses
iroha --config client.toml repo margin-call --agreement-id daily_repo
```

* `repo initiate` and `repo unwind` respect `--input/--output` so the generated `InstructionBox`
  payloads can be piped into other CLI flows or submitted immediately.
* Pass `--custodian <account>` to route collateral to a tri-party custodian. When omitted, the
  counterparty receives the pledge directly (bilateral repo).
* `repo margin` queries the ledger via `FindRepoAgreements` and reports the next expected margin
  timestamp (in milliseconds) alongside whether a margin callback is currently due.
* `repo margin-call` appends a `RepoMarginCallIsi` instruction, recording the margin checkpoint and
  emitting events for all participants. Calls are rejected if the cadence has not elapsed or if the
  instruction is submitted by a non-participant.

## Python SDK helpers

```python
from iroha_python import (
    create_torii_client,
    RepoAgreementRecord,
    RepoCashLeg,
    RepoCollateralLeg,
    RepoGovernance,
    TransactionConfig,
    TransactionDraft,
)

client = create_torii_client("client.toml")

cash = RepoCashLeg(asset_definition_id="usd#wonderland", quantity="1000")
collateral = RepoCollateralLeg(
    asset_definition_id="bond#wonderland",
    quantity="1050",
    metadata={"isin": "ABC123"},
)
governance = RepoGovernance(haircut_bps=1500, margin_frequency_secs=86_400)

draft = TransactionDraft(TransactionConfig(chain_id="dev-chain", authority="ih58..."))
draft.repo_initiate(
    agreement_id="daily_repo",
    initiator="ih58...",
    counterparty="ih58...",
    cash_leg=cash,
    collateral_leg=collateral,
    rate_bps=250,
    maturity_timestamp_ms=1_704_000_000_000,
    governance=governance,
)
# ... additional instructions ...
envelope = draft.sign_with_keypair(my_keypair)
client.submit_transaction_envelope(envelope)

# Margin schedule
agreements = client.list_repo_agreements()
record = RepoAgreementRecord.from_payload(agreements[0])
next_margin = record.next_margin_check_after(at_timestamp_ms=now_ms)
```

* Both helpers normalise numeric quantities and metadata fields before invoking the PyO3 bindings.
* `RepoAgreementRecord` mirrors the runtime schedule calculation so off-ledger automation can
  determine when callbacks are due without recomputing the cadence manually.

## DvP / PvP settlements

The `iroha app settlement` command stages delivery-versus-payment and payment-versus-payment instructions:

```bash
# Delivery leg first, then payment
iroha --config client.toml --output \
  settlement dvp \
  --settlement-id trade_dvp \
  --delivery-asset bond#wonderland \
  --delivery-quantity 10 \
  --delivery-from ih58... \
  --delivery-to ih58... \
  --delivery-instrument-id US0378331005 \
  --payment-asset usd#wonderland \
  --payment-quantity 1000 \
  --payment-from ih58... \
  --payment-to ih58... \
  --order payment-then-delivery \
  --atomicity all-or-nothing \
  --iso-reference-crosswalk /opt/iso/isin_crosswalk.json \
  --iso-xml-out trade_dvp.xml

# Cross-currency swap (payment-versus-payment)
iroha --config client.toml --output \
  settlement pvp \
  --settlement-id trade_pvp \
  --primary-asset usd#wonderland \
  --primary-quantity 500 \
  --primary-from ih58... \
  --primary-to ih58... \
  --counter-asset eur#wonderland \
  --counter-quantity 460 \
  --counter-from ih58... \
  --counter-to ih58... \
  --iso-xml-out trade_pvp.xml
```

* Leg quantities accept integral or decimal values and are validated against the asset precision.
* `--atomicity` accepts `all-or-nothing`, `commit-first-leg`, or `commit-second-leg`. Use these modes
  with `--order` to express which leg remains committed if subsequent processing fails (`commit-first-leg`
  keeps the first leg applied; `commit-second-leg` retains the second).
* CLI invocations emit empty instruction metadata today; use the Python helpers when settlement-level
  metadata needs to be attached.
* See [`settlement_iso_mapping.md`](./settlement_iso_mapping.md) for the ISO 20022 field mapping that
  backs these instructions (`sese.023`, `sese.025`, `colr.007`, `pacs.009`, `camt.054`).
* Pass `--iso-xml-out <path>` to have the CLI emit a canonical XML preview alongside the Norito
  instruction; the file follows the mapping above (`sese.023` for DvP, `sese.025` for PvP`). Pair the
  flag with `--iso-reference-crosswalk <path>` so the CLI verifies `--delivery-instrument-id` against the
  same snapshot Torii uses during runtime admission.

Python helpers mirror the CLI surface:

```python
from iroha_python import (
    SettlementLeg,
    SettlementPlan,
    SettlementExecutionOrder,
    TransactionConfig,
    TransactionDraft,
)

draft = TransactionDraft(TransactionConfig(chain_id="dev-chain", authority="ih58..."))
delivery = SettlementLeg(
    asset_definition_id="bond#wonderland",
    quantity="10",
    from_account="ih58...",
    to_account="ih58...",
    metadata={"isin": "ABC123"},
)
payment = SettlementLeg(
    asset_definition_id="usd#wonderland",
    quantity="1000",
    from_account="ih58...",
    to_account="ih58...",
)
plan = SettlementPlan(order=SettlementExecutionOrder.PAYMENT_THEN_DELIVERY)

draft.settlement_dvp("trade_dvp", delivery, payment, plan=plan, metadata={"desk": "rates"})
draft.settlement_pvp(
    "trade_pvp",
    SettlementLeg(
        asset_definition_id="usd#wonderland",
        quantity="500",
        from_account="ih58...",
        to_account="ih58...",
    ),
    SettlementLeg(
        asset_definition_id="eur#wonderland",
        quantity="460",
        from_account="ih58...",
        to_account="ih58...",
    ),
)
```

## Determinism & Governance Expectations

Repo instructions rely exclusively on Norito-encoded numeric types and the shared
`RepoGovernance::with_defaults` logic. Keep the following invariants in mind:

* Quantities are serialised with deterministic `NumericSpec` values: cash legs use
  `fractional(2)` (two decimal places), collateral legs use `integer()`. Do not submit
  values with greater precision—runtime guards will reject them and peers would diverge.
* Tri-party repos persist the custodian account id in `RepoAgreement`. Lifecycle and margin events
  emit a `RepoAccountRole::Custodian` payload so custodians can subscribe and reconcile inventory.
* Haircuts are clamped to 10 000 bps (100 %) and margin frequencies are whole seconds. Provide
  governance parameters in those canonical units to stay aligned with runtime expectations.
* Timestamps are always unix milliseconds. All helpers forward them unchanged to the Norito
  payload so peers derive identical schedules.
* Initiation and unwind instructions reuse the same agreement identifier. The runtime rejects
  duplicate IDs and unwinds for unknown agreements; CLI/SDK helpers surface those errors early.
* `repo margin`/`RepoAgreementRecord::next_margin_check_after` return the canonical cadence. Always
  consult this snapshot before triggering callbacks to avoid replaying stale schedules.
