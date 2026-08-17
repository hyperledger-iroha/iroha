---
title: Sumeragi Revision-4 Parameter Surface
status: implemented
last_updated: 2026-08-15
---

# Sumeragi revision-4 parameter surface

The first-release revision-4 cutover completed the parameter streamlining as a
hard break from V1. Consensus-critical choices are authenticated chain context;
local configuration only supplies a participation role, finite resource bounds,
and signing-key policy.

## Signed consensus context

Signed genesis/current-height context selects the consensus mode, block cadence,
committee and leader ordering, exact `3f + 1`/`2f + 1` quorum geometry, and the
mandatory Reed-Solomon-16 DA layout. NPoS election and epoch parameters are
governed on-chain. Validators do not merge these values with local timing,
collector, RBC, DA, or pacemaker overrides.

The view-zero deadline is ten signed cadence intervals, retransmission is one
fifth of that deadline, and later certified views use deterministic linear
backoff capped at ten base deadlines. These values are derived, not configured.
The cap keeps an idle chain's monotone view number from imposing an unbounded
delay when work later arrives; it does not authorize empty blocks.

## Node-local configuration

The accepted `[sumeragi]` surface contains only:

- `role`: validator or observer participation;
- `block`: finite candidate transaction/body/queue-scan bounds;
- `queues`: bounded reducer, body, chunk, and ready-body ingress resources;
- `limits`: finite lane, merge, recovery, and Native AMX service resources;
- `keys`: consensus key rotation, algorithm, and HSM policy.

The canonical shared runtime projection fingerprints signed mode/cadence with
the finite limits and key policy. Validators must align that fingerprint before
activation. Unknown V1 and adaptive tables, global timing fields, and local
protocol-version selectors are rejected rather than silently ignored.

## Operational rule

Change consensus-critical values through a signed genesis/current-height
rollout. Change finite node-local resources consistently across validators and
validate the resulting shared fingerprint. Tests that need Byzantine protocol
behavior inject authenticated revision-4 messages; availability and liveness
tests may instead stop or partition real validators. A local debug table is not
a consensus test control.
