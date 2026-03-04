# Data Availability Rent & Incentive Policy (DA-7)

_Status: Drafting — Owners: Economics WG / Treasury / Storage Team_

Roadmap item **DA-7** introduces an explicit XOR-denominated rent for every blob
submitted to `/v1/da/ingest`, plus bonuses that reward PDP/PoTR execution and
egress served to fetch clients. This document defines the initial parameters,
their data-model representation, and the calculation workflow used by Torii,
SDKs, and Treasury dashboards.

## Policy structure

The policy is encoded as [`DaRentPolicyV1`](/crates/iroha_data_model/src/da/types.rs)
within the data model. Torii and governance tooling persist the policy in
Norito payloads so that rent quotes and incentive ledgers can be recomputed
deterministically. The schema exposes five knobs:

| Field | Description | Default |
|-------|-------------|---------|
| `base_rate_per_gib_month` | XOR charged per GiB per month of retention. | `250_000` micro-XOR (0.25 XOR) |
| `protocol_reserve_bps` | Share of the rent routed to the protocol reserve (basis points). | `2_000` (20 %) |
| `pdp_bonus_bps` | Bonus percentage per successful PDP evaluation. | `500` (5 %) |
| `potr_bonus_bps` | Bonus percentage per successful PoTR evaluation. | `250` (2.5 %) |
| `egress_credit_per_gib` | Credit paid when a provider serves 1 GiB of DA data. | `1_500` micro-XOR |

All basis-point values are validated against `BASIS_POINTS_PER_UNIT` (10 000).
Policy updates must travel through governance, and every Torii node exposes the
active policy via the `torii.da_ingest.rent_policy` configuration section
(`iroha_config`). Operators can override the defaults in `config.toml`:

```toml
[torii.da_ingest.rent_policy]
base_rate_per_gib_month_micro = 250000        # 0.25 XOR/GiB-month
protocol_reserve_bps = 2000                   # 20% protocol reserve
pdp_bonus_bps = 500                           # 5% PDP bonus
potr_bonus_bps = 250                          # 2.5% PoTR bonus
egress_credit_per_gib_micro = 1500            # 0.0015 XOR/GiB egress credit
```

CLI tooling (`iroha app da rent-quote`) accepts the same Norito/JSON policy inputs
and emits artefacts that mirror the active `DaRentPolicyV1` without reaching
back into Torii state. Supply the policy snapshot used for an ingest run so the
quote remains reproducible.

### Persisting rent quote artefacts

Run `iroha app da rent-quote --gib <size> --months <months> --quote-out <path>` to
emit both the on-screen summary and a pretty-printed JSON artefact. The file
records `policy_source`, the inlined `DaRentPolicyV1` snapshot, the computed
`DaRentQuote`, and a derived `ledger_projection` (serialized via
[`DaRentLedgerProjection`](/crates/iroha_data_model/src/da/types.rs)) making it suitable for treasury dashboards and ledger ISI
pipelines. When `--quote-out` points at a nested directory the CLI creates any
missing parents, so operators can standardise locations such as
`artifacts/da/rent_quotes/<timestamp>.json` alongside other DA evidence bundles.
Attach the artefact to rent approvals or reconciliation runs so the XOR
breakdown (base rent, reserve, PDP/PoTR bonuses, and egress credits) is
reproducible. Pass `--policy-label "<text>"` to override the automatically
derived `policy_source` description (file paths, embedded default, etc.) with a
human-readable tag such as a governance ticket or manifest hash; the CLI trims
this value and rejects empty/whitespace-only strings so the recorded evidence
remains auditable.

```json
{
  "policy_source": "policy JSON `configs/da/rent_policy.json`",
  "gib": 10,
  "months": 3,
  "policy": { "...": "DaRentPolicyV1 fields elided" },
  "quote": { "...": "DaRentQuote breakdown" },
  "ledger_projection": {
    "rent_due": { "micro": 7500000 },
    "protocol_reserve_due": { "micro": 1500000 },
    "provider_reward_due": { "micro": 6000000 },
    "pdp_bonus_pool": { "micro": 375000 },
    "potr_bonus_pool": { "micro": 187500 },
    "egress_credit_per_gib": { "micro": 1500 }
  }
}
```

The ledger projection section feeds directly into the DA rent ledger ISIs: it
defines the XOR deltas destined for the protocol reserve, provider payouts, and
the per-proof bonus pools without requiring bespoke orchestration code.

### Generating rent ledger plans

Run `iroha app da rent-ledger --quote <path> --payer-account <id> --treasury-account <id> --protocol-reserve-account <id> --provider-account <id> --pdp-bonus-account <id> --potr-bonus-account <id> --asset-definition xor#sora`
to convert a persisted rent quote into executable ledger transfers. The command
parses the embedded `ledger_projection`, emits Norito `Transfer` instructions
that collect the base rent into the treasury, routes the reserve/provider
portions, and pre-funds the PDP/PoTR bonus pools directly from the payer. The
output JSON mirrors the quote metadata so CI and treasury tooling can reason
about the same artefact:

```json
{
  "quote_path": "artifacts/da/rent_quotes/2025-12-07/rent.json",
  "rent_due_micro_xor": 7500000,
  "protocol_reserve_due_micro_xor": 1500000,
  "provider_reward_due_micro_xor": 6000000,
  "pdp_bonus_pool_micro_xor": 375000,
  "potr_bonus_pool_micro_xor": 187500,
  "egress_credit_per_gib_micro_xor": 1500,
  "instructions": [
    { "Transfer": { "...": "payer -> treasury base rent instruction elided" }},
    { "Transfer": { "...": "treasury -> reserve" }},
    { "Transfer": { "...": "treasury -> provider payout" }},
    { "Transfer": { "...": "payer -> PDP bonus escrow" }},
    { "Transfer": { "...": "payer -> PoTR bonus escrow" }}
  ]
}
```

The final `egress_credit_per_gib_micro_xor` field lets dashboards and payout
schedulers align egress reimbursements with the rent policy that produced the
quote without recomputing the policy math in scripting glue.

## Example quote

```rust
use iroha_data_model::da::types::DaRentPolicyV1;

// 10 GiB retained for 3 months.
let policy = DaRentPolicyV1::default();
let quote = policy.quote(10, 3).expect("policy validated");

assert_eq!(quote.base_rent.as_micro(), 7_500_000);      // 7.5 XOR total rent
assert_eq!(quote.protocol_reserve.as_micro(), 1_500_000); // 20% reserve
assert_eq!(quote.provider_reward.as_micro(), 6_000_000);  // Direct provider payout
assert_eq!(quote.pdp_bonus.as_micro(), 375_000);          // PDP success bonus
assert_eq!(quote.potr_bonus.as_micro(), 187_500);         // PoTR success bonus
assert_eq!(quote.egress_credit_per_gib.as_micro(), 1_500);
```

The quote is reproducible across Torii nodes, SDKs, and Treasury reports because
it uses deterministic Norito structures instead of ad-hoc math. Operators can
attach the JSON/CBOR encoded `DaRentPolicyV1` to governance proposals or rent
audits to prove which parameters were in force for any given blob.

## Bonuses and reserves

- **Protocol reserve:** `protocol_reserve_bps` funds the XOR reserve that backs
  emergency re-replication and slashing refunds. Treasury tracks this bucket
  separately to ensure ledger balances match the configured rate.
- **PDP/PoTR bonuses:** Each successful proof evaluation receives an additional
  payout derived from `base_rent × bonus_bps`. When the DA scheduler emits proof
  receipts it includes the basis-point tags so incentives can be replayed.
- **Egress credit:** Providers record GiB served per manifest, multiply by
  `egress_credit_per_gib`, and submit the receipts via `iroha app da prove-availability`.
  The rent policy keeps the per-GiB amount in sync with governance.

## Operational flow

1. **Ingest:** `/v1/da/ingest` loads the active `DaRentPolicyV1`, quotes rent
   based on blob size and retention, and embeds the quote into the Norito
   manifest. The submitter signs a statement that references the rent hash and
   the storage ticket id.
2. **Accounting:** Treasury ingest scripts decode the manifest, call
   `DaRentPolicyV1::quote`, and populate rent ledgers (base rent, reserve,
   bonuses, and expected egress credits). Any discrepancy between recorded rent
   and recomputed quotes fails CI.
3. **Proof rewards:** When PDP/PoTR schedulers mark a success they emit a receipt
   containing the manifest digest, proof kind, and the XOR bonus derived from
   the policy. Governance can audit the payouts by recomputing the same quote.
4. **Egress reimbursement:** Fetch orchestrators submit signed egress summaries.
   Torii multiplies the GiB count by `egress_credit_per_gib` and issues payment
   instructions against the rent escrow.

## Telemetry

Torii nodes expose rent usage via the following Prometheus metrics (labels:
`cluster`, `storage_class`):

- `torii_da_rent_gib_months_total` — GiB-months quoted by `/v1/da/ingest`.
- `torii_da_rent_base_micro_total` — base rent (micro XOR) accrued at ingest.
- `torii_da_protocol_reserve_micro_total` — protocol reserve contributions.
- `torii_da_provider_reward_micro_total` — provider-side rent payouts.
- `torii_da_pdp_bonus_micro_total` and `torii_da_potr_bonus_micro_total` —
  PDP/PoTR bonus pools sourced from the ingest quote.

Economics dashboards rely on these counters to ensure ledger ISIs, reserve taps,
and PDP/PoTR bonus schedules all match the policy parameters in force for each
cluster and storage class. The SoraFS Capacity Health Grafana board
(`dashboards/grafana/sorafs_capacity_health.json`) now renders dedicated panels
for rent distribution, PDP/PoTR bonus accrual, and GiB-month capture, allowing
Treasury to filter by Torii cluster or storage class when reviewing ingest
volume and payouts.

## Next steps

- ✅ `/v1/da/ingest` receipts now embed `rent_quote` and the CLI/SDK surfaces display the quoted
  base rent, reserve share, and PDP/PoTR bonuses so submitters can review the XOR obligations before
  committing payloads.
- Integrate the rent ledger with the forthcoming DA reputation/order-book feeds
  to prove that high-availability providers are receiving the correct payouts.
