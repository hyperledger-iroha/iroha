# In-flight first-release carrier evidence

`SumeragiV2InFlightFirstRelease.tla` is a three-validator bounded safety
kernel for the in-flight first-release protocol. Its TLC evidence is owned by
`scripts/formal/run_sumeragi_v2_inflight_first_release.sh`; the fixed model is
also part of the pinned positive Apalache matrix.

The production container remains `LaneExecutablePayloadV1`, whose sole
accepted first-release schema is
`LANE_EXECUTABLE_PAYLOAD_VERSION_V2`. The fixed model represents that schema
as an exact `QueuePlanAdmissionBindingV2` preimage at every validator. It
checks:

- exactly one `ProducerSelected` owner and two `ReplicatedCarrier` owners;
- QueuePlan journal V4 `PutBatch` before reservation journal V5 fsync, before
  Kura Active, execution-input durability, and `READY`;
- crash-prefix preservation of every durable fact, including recovery after a
  producer death and replica-to-replica late-body service;
- exact binding scope for both Commit and Release;
- exactly-once carrier application; and
- the literal 4096 `PutBatch` entry bound.

The nine `_bug.cfg` controls are required to emit the named TLC invariant
violation. They cover inverted V4/V5 order, Kura-before-reservation,
READY-before-input, durable loss on crash, conflicting/ABA binding preimages,
Commit and Release scope drift, duplicate application, and a 4097-entry
batch.

## Evidence status and projection boundary

This kernel is **bounded abstract model-checking evidence only**. It is not a
production-refinement theorem. Schema 3 of
`multilane_source_bindings.json` records a separate
`layout_only_no_transition_refinement` contract. That layout-only source
binding fails closed if the model, configs, runners, documentation, accepted
version constants, exact payload/reservation fields, queue durability order,
Kura execution-input persistence/recovery consumers, or the formal release
receipt's distinct fifth layout-only result row drift. It remains outside the
four production-refinement kernels.

The exact blocker is a machine-checked forward simulation from all Rust
QueuePlan journal V4, reservation journal V5, Kura, recovery, filesystem-error,
restart, Commit, and Release transitions to `InFlightFirstReleaseSpec`, plus a
backward ownership projection for every terminal Commit/Release outcome.
Neither TLAPS nor Apalache consumes Rust operational semantics, and the
repository has no verified Rust semantics or trace-extraction theorem. The
layout-only source binding is therefore not evidence for this missing theorem.
Do not upgrade `proof_coverage.json` or release status until the total projection theorem
is implemented and checked.
