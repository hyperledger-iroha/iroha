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
- one content-bound selected-batch conjunction over exact individual
  QueuePlan journal V4 `Put` records before reservation journal V5 fsync;
- Kura Active and execution-input durability before volatile READY
  authorization, local READY signature, and durable READY QC;
- loss of volatile body/authorization custody on crash while every durable
  claim, reservation, Kura/input/QC, and release-prefix fact remains
  recoverable;
- lane commit as a consensus decision distinct from post-carrier reservation
  Commit, QueuePlan tombstone, and ForgetCommit;
- atomic WSV carrier application exactly once, with later receipt, sidecar,
  index, and reservation repairs modeled as stuttering;
- the four-stage release protocol: Kura retirement plus prefix-recoverable
  ReleasePending claims, Queue PrepareRelease, prefix-recoverable Released
  claims, then Queue CompleteRelease/FIFO restoration/ForgetRelease;
- exact binding scope for lane commit and release; and
- the literal 4096 selected-entry ceiling.

The twenty `_bug.cfg` controls are required to emit their named TLC invariant
violation. They cover inverted selected-V4/V5 order, Kura-before-reservation,
each READY ordering boundary, durable loss and improper volatile retention on
crash, conflicting/ABA binding preimages, lane-commit and release scope drift,
duplicate WSV application, each post-carrier cleanup boundary, each durable
release boundary, and a 4097-entry selected conjunction.

## Evidence status and projection boundary

This kernel is **bounded abstract model-checking evidence only**. It is not a
production-refinement theorem. Schema 4 of
`multilane_source_bindings.json` records a separate
`layout_only_no_transition_refinement` contract. That layout-only source
binding fails closed if the model, configs, runners, documentation, accepted
version constants, exact payload/reservation fields, queue durability order,
Kura execution-input persistence/recovery consumers, or the formal release
receipt's distinct fifth layout-only result row drift. It remains outside the
four production-refinement kernels.

Within that deliberately limited contract, the reservation journal's local
primitive transition seam is also source-bound. The binding covers the
move-only `PreparedReservationJournalTransition` capability, its exact frame,
ownership bound, state-instance domain, structural pre-state shape, generation,
resulting-state history, and ordered owner-token coverage, the primitive
refinement check, runtime rejection of an Absent-to-Committed transition,
post-I/O semantic/owner revalidation, bounded direct application without a
full-state clone, durable append, and snapshot compaction. Adversarial
capability tests and post-sync restart tests are whole-file tokens of this
contract. Disk-ahead publication failures poison the live owner and require
reconstruction from the canonical journal on restart.

This local source binding is not a cross-store transition theorem. Refinement
across QueuePlan, Kura, the canonical carrier, WSV, FIFO restoration, and lane
lifecycle remains open, as do the total forward simulation and reverse
terminal-owner projection.

The exact blocker is a machine-checked forward simulation from all Rust
QueuePlan journal V4, reservation journal V5, Kura, recovery, filesystem-error,
restart, READY authorization/signature/QC, lane-commit, atomic WSV application,
post-carrier cleanup, and four-stage Release transitions to
`InFlightFirstReleaseSpec`, plus a backward ownership projection for every
terminal Commit/Release outcome.
Neither TLAPS nor Apalache consumes Rust operational semantics, and the
repository has no verified Rust semantics or trace-extraction theorem. The
layout-only source binding is therefore not evidence for this missing theorem.
Do not upgrade `proof_coverage.json` or release status until the total projection theorem
is implemented and checked.
