# In-flight first-release carrier evidence

`SumeragiV2InFlightFirstRelease.tla` is a three-validator bounded safety
kernel for the in-flight first-release protocol.  Its executable evidence is
owned by `scripts/formal/run_sumeragi_v2_inflight_first_release.sh`.

The fixed model represents `LaneExecutablePayloadV3` as an exact
`QueuePlanAdmissionBindingV2` preimage at every validator. It checks:

- exactly one `ProducerSelected` owner and two `ReplicatedCarrier` owners;
- QueuePlan V5 `PutBatch` before V9 reservation fsync, before Kura Active,
  execution-input durability, and `READY`;
- crash-prefix preservation of every durable fact, including recovery after a
  producer death and replica-to-replica late-body service;
- exact binding scope for both Commit and Release;
- exactly-once carrier application; and
- the literal 4096 `PutBatch` entry bound.

The nine `_bug.cfg` controls are required to emit the named TLC invariant
violation. They cover inverted V5/V9 order, Kura-before-reservation,
READY-before-input, durable loss on crash, conflicting/ABA binding preimages,
Commit and Release scope drift, duplicate application, and a 4097-entry
batch.

## Evidence status and projection boundary

This kernel is **bounded abstract model-checking evidence only**. It is not a
production-refinement theorem and is intentionally not listed as a
source-bound production kernel in `multilane_source_bindings.json`. That
ledger's token checks can establish that named Rust items still exist; they
cannot establish the total pre/post-state projection demanded by this model.

The exact blocker is a machine-checked forward simulation from all Rust
QueuePlan V5/Kura V9/recovery transitions (including every filesystem error
and restart branch) to `InFlightFirstReleaseSpec`, plus a backward ownership
projection for every Commit/Release outcome. Neither TLAPS nor Apalache can
consume Rust operational semantics, and the repository has no verified Rust
semantics or trace-extraction theorem. A source-token match is therefore not
evidence for this missing theorem.

When the V3 payload and execution-input symbols are stable, add a separately
reviewed source-binding entry that records their exact fields and recovery
consumers. Do not upgrade this file, `proof_coverage.json`, or any release
status until the total projection theorem is implemented and checked.
