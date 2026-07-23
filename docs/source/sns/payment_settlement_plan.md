---
title: Native Alias Charges and Reconciliation
summary: Consensus-native lease charging and audit evidence without client payment proofs or mutation services.
---

# Native Alias Charges and Reconciliation

Alias acquisition and renewal are charged inside the ordinary signed
transaction that performs the lifecycle operation. The former SN-5 design for
a separate settlement service is retired: there is no
`/v1/sns/settlements` mutation API, client-created `PaymentBundleV1`,
uploaded payment proof, or `iroha app sns settlement` command.

## Deterministic charging

The read-only alias planner classifies live state before quoting:

- exact existing state is a zero-charge `NoOp`;
- missing derived state is a zero-charge `Repair`;
- an absent lease-bearing resource is `Create` and receives an exact quote;
- drift is `Conflict` and produces no executable plan.

For acquisition, `AliasQuoteGuardV1` commits the expected policy version,
payment asset, maximum amount, and deadline. Consensus recomputes the quote at
execution and debits the transaction authority, which is the lease payer, by
the exact calculated amount rather than the cap. Renewal uses the same guard
plus expected-current-expiry compare-and-set semantics.

No client-supplied receipt or off-chain settlement assertion can authorize a
lease. Native execution and the committed world state are authoritative.

## Reconciliation evidence

Treasury and governance reporting should derive evidence from canonical
ledger data:

1. canonical alias plan hash, chain id, block anchor, and expiry;
2. transaction hash, authority, ordered framed instructions, and final status;
3. resource disposition, policy version, payment asset, and exact native
   charge;
4. resulting lease, owner, binding, capability, and auto-renew revision.

Read-only reporting may aggregate these records into statements and revenue
reports. Such reports are audit projections; replaying or uploading one never
moves funds or changes alias state.

## Corrections and refunds

Alias provisioning exposes no special refund or settlement mutation route.
Any financial correction must be an explicitly authorized ordinary ledger
transaction under the applicable governance policy. Revenue splitting, if
enabled by policy, must be deterministic native execution using canonical
integer/Norito representations; an external service must not replace or
reinterpret the consensus charge.

Keep plan and report files secret-free. API credentials belong in headers or
permission-restricted token files, and signing keys remain in runtime-only
client configuration.
