# Atomic multisig policy changes

Ordinary `ReplaceAccountController` rekeys a native multisig account while
preserving its proposal lifecycle. Applications that require a signer or
threshold policy change to cancel prior work must opt in explicitly by placing
these instructions, in this order, in one native multisig proposal:

1. `MultisigInstructionBox::InvalidateOutstanding` for the currently active
   multisig `AccountId`;
2. `ReplaceAccountController` for that same account and the exact replacement
   controller.

`InvalidateOutstanding` is owner-authorized: it can execute only as the target
multisig account, so a signatory cannot call it directly. A top-level multisig
proposal is terminalized and pruned before its approved instructions execute.
The policy-change proposal therefore does not cancel itself; it finalizes, then
the invalidation instruction writes `CANCELED` or already-`EXPIRED` terminal
records for every other outstanding proposal before the controller is rekeyed.
All of these writes share one state transaction and roll back together on any
error.

For exact audit evidence, freeze the collecting proposal ids before proposing
the policy change. After the policy-change proposal is `FINALIZED`, resolve
each frozen id through `/v1/multisig/proposals/resolve` and require:

- the exact proposal id and instruction hash;
- status `CANCELED` or `EXPIRED`, never `FINALIZED`;
- a positive `terminal_at_ms`; and
- no remaining `COLLECTING_SIGNATURES` proposal from the prior controller.

The policy-change proposal's own terminal evidence must match the exact ordered
instruction batch, including `InvalidateOutstanding` first. Private keys are
never accepted by these routes; preparation and submission continue to use the
existing detached Ed25519 signing flow.
