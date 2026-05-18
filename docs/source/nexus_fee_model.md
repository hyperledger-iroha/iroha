# Nexus Fee Model Updates

Nexus gas is denominated in XOR across every dataspace. The unified settlement
path captures deterministic per-lane receipts so operators can reconcile XOR
gas debits against the Nexus fee model without introducing per-dataspace gas
assets.

- For the full router architecture, buffer policy, telemetry matrix, and rollout
  sequencing see `docs/settlement-router.md`. That guide explains how the
  parameters documented here tie into the NX-3 roadmap deliverable and how SREs
  should monitor the router in production.
- Nexus fee configuration accepts only the canonical XOR fee asset
  (`xor#universal` or its canonical asset definition selector). Local gas-token
  conversion metadata (`twap_local_per_xor`, `liquidity_profile`, and
  `volatility_class`) is reserved for explicit settlement products and is not
  the default gas path.
- IVM transactions must include `gas_limit` metadata (`u64`, > 0) to cap fee
  exposure. The `/v1/contracts/call` endpoint requires `gas_limit`
  explicitly, and invalid values are rejected.
- When a transaction sets `fee_sponsor` metadata, that explicit sponsor takes
  precedence. The sponsor must grant `CanUseFeeSponsor { sponsor }` to the
  caller unless it is also configured as the routed dataspace's default
  sponsor.
- Dataspaces may configure `fee_sponsor_account_id` in
  `nexus.dataspace_catalog`. When sponsorship is enabled and a routed
  transaction does not set explicit `fee_sponsor` metadata, Nexus charges the
  dataspace default sponsor automatically. This keeps onboarding from requiring
  per-account sponsorship grants.
- Every transaction that pays gas records the XOR fee payer/sponsor and the
  fee schedule inputs needed to recompute the amount. Lane-relay-burn mode
  embeds versioned Nexus fee receipts in lane commitments; direct mode mutates
  public XOR in the universal fee context.
- Block execution aggregates receipts per lane/dataspace and publishes them
  via `lane_settlement_commitments` in `/v1/sumeragi/status`.  The totals
  expose XOR fee receipt totals for nightly reconciliation exports.
- A new `total_xor_variance_micro` counter tracks how much safety margin was
  consumed (difference between the due XOR and the post-haircut expectation),
  and `swap_metadata` documents the deterministic conversion parameters
  (TWAP, epsilon, liquidity profile, and volatility_class) so auditors can
  verify the quote inputs independent of runtime configuration.

Consumers can watch `lane_settlement_commitments` alongside the existing lane
and dataspace commitment snapshots to verify that fee buffers, haircut tiers,
and swap execution match the configured Nexus fee model.

## Lane Relay XOR Burn Settlement

The default `nexus.fees.settlement_mode = "direct"` path keeps the existing
fee behavior. For DPN lanes that finalize locally and settle fees on Nexus,
operators can enable `nexus.fees.settlement_mode = "lane_relay_burn"`.

In lane relay burn mode, DPN block production remains local. Transaction
execution validates the configured sponsor metadata, computes the Nexus fee
deterministically, and records a versioned Nexus fee receipt in the block
settlement accumulator. The receipt is part of the lane block commitment and
includes the source transaction id, dataspace id, lane id, block height, payer
or sponsor Nexus account id, `xor#universal` fee asset id, computed amount, and
the fee schedule inputs required to recompute the amount. DPN does not burn,
transfer, escrow, or otherwise mutate public XOR in this mode.

`record_lane_relay()` remains non-mutating. Nexus applies XOR burns only when a
merge entry commits a relayed lane block settlement. Settlement validates the
referenced relay, settlement hash, receipt coordinates, fee asset, deterministic
fee amount, duplicate receipt source ids, and sponsor balance before mutating
state. The merge settlement is all-or-nothing for Nexus fee burns: invalid
proof material, invalid receipts, duplicate receipt ids, or insufficient public
XOR reject the settlement without partial fee mutation.

Receipt idempotency is keyed by the settled dataspace, lane, block height,
settlement hash, and receipt source ids so duplicate relay submission, merge
replay, and restart recovery do not double-burn public XOR. Operators should
keep the temporary `taira-DPN-nexus-fee-reconciler.timer` active until a
post-activation DPN block is observed settling through a protocol Nexus burn.
After that verification, disable the timer, preserve the reconciler settlement
records for audit, and mark the reconciler retired rather than part of normal
fee settlement.
