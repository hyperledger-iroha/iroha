---
lang: zh-hans
direction: ltr
source: docs/source/nexus_fee_model.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b65586c3295ef3005d3d306efeedeb8433e516cd742bdb623486e547aaa5a5ab
source_last_modified: "2026-01-09T14:27:28.945285+00:00"
translation_last_reviewed: 2026-02-07
---

# Nexus Fee Model Updates

The unified settlement router now captures deterministic per-lane receipts so
operators can reconcile gas debits against the Nexus fee model.

- For the full router architecture, buffer policy, telemetry matrix, and rollout
  sequencing see `docs/settlement-router.md`. That guide explains how the
  parameters documented here tie into the NX-3 roadmap deliverable and how SREs
  should monitor the router in production.
- Gas asset configuration (`pipeline.gas.units_per_gas`) includes a
  `twap_local_per_xor` decimal, a `liquidity_profile` (`tier1`, `tier2`,
  or `tier3`), and a `volatility_class` (`stable`, `elevated`, `dislocated`).
  These flags feed the settlement router so the resulting XOR
  quote matches the canonical TWAP and haircut tier for the lane.
- IVM transactions must include `gas_limit` metadata (`u64`, > 0) to cap fee
  exposure. The `/v2/contracts/call` endpoint requires `gas_limit`
  explicitly, and invalid values are rejected.
- When a transaction sets `fee_sponsor` metadata, the sponsor must grant
  `CanUseFeeSponsor { sponsor }` to the caller. Unauthorized sponsorship
  attempts are rejected and recorded.
- Every transaction that pays gas records a `LaneSettlementReceipt`.  Each
  receipt stores the caller-provided source identifier, the local micro-amount,
  the XOR due immediately, the XOR expected after the haircut, the realised
  safety margin (`xor_variance_micro`), and the block timestamp in milliseconds.
- Block execution aggregates receipts per lane/dataspace and publishes them
  via `lane_settlement_commitments` in `/v2/sumeragi/status`.  The totals
  expose `total_local_micro`, `total_xor_due_micro`, and
  `total_xor_after_haircut_micro` summed over the block for nightly
  reconciliation exports.
- A new `total_xor_variance_micro` counter tracks how much safety margin was
  consumed (difference between the due XOR and the post-haircut expectation),
  and `swap_metadata` documents the deterministic conversion parameters
  (TWAP, epsilon, liquidity profile, and volatility_class) so auditors can
  verify the quote inputs independent of runtime configuration.

Consumers can watch `lane_settlement_commitments` alongside the existing lane
and dataspace commitment snapshots to verify that fee buffers, haircut tiers,
and swap execution match the configured Nexus fee model.
