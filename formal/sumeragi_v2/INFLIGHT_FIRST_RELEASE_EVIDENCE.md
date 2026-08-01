# In-flight first-release carrier evidence

`SumeragiV2InFlightFirstRelease.tla` is a three-validator bounded safety
kernel for the in-flight first-release protocol. Its TLC evidence is owned by
`scripts/formal/run_sumeragi_v2_inflight_first_release.sh`; the fixed model is
also part of the pinned positive Apalache matrix.

The production container remains `LaneExecutablePayloadV1`, whose sole
accepted first-release schema is
`LANE_EXECUTABLE_PAYLOAD_VERSION_V2`. The fixed model represents that schema
as exact authenticated `QueuePlanAdmissionBindingV2` preimage custody by the
selected producer. Other committee members may have no established custody;
the model and production projection do not infer all-validator knowledge. It
checks:

- exactly one `ProducerSelected` owner and two `ReplicatedCarrier` owners;
- producer-inclusive, committee-bounded authenticated preimage custody;
- one content-bound selected-batch conjunction over exact individual
  QueuePlan journal V4 `Put` records before reservation journal V5 fsync;
- Kura Active and execution-input durability before volatile READY
  authorization, local READY signature, and durable READY QC;
- the canonical strict two-thirds count threshold (3-of-3 in this deliberately
  fixed three-validator instance);
- loss of volatile body/authorization custody on crash while every durable
  claim, reservation, Kura/input/QC, and release-prefix fact remains
  recoverable;
- lane commit as a consensus decision distinct from post-carrier reservation
  Commit, QueuePlan tombstone, and ForgetCommit, each represented as a durable
  canonical per-key prefix rather than a group-wide transition;
- crash-visible mixed Commit/tombstone/ForgetCommit prefixes, with exact
  one-based next-key order and no skipped or decreasing prefix transition;
- atomic WSV carrier application exactly once, with later receipt, sidecar,
  index, and reservation repairs modeled as stuttering;
- the four-stage release protocol: Kura retirement plus prefix-recoverable
  ReleasePending claims, Queue PrepareRelease, prefix-recoverable Released
  claims, then Queue CompleteRelease/FIFO restoration/ForgetRelease;
- exact binding scope for lane commit and release; and
- the literal 4096 selected-entry ceiling.

The twenty-two `_bug.cfg` controls are required to emit their named TLC invariant
violation. They cover inverted selected-V4/V5 order, Kura-before-reservation,
each READY ordering boundary, durable loss and improper volatile retention on
crash, conflicting/ABA binding preimages, lane-commit and release scope drift,
duplicate WSV application, each post-carrier cleanup boundary, each durable
release boundary, a skipped or decreasing Commit-key prefix, and a 4097-entry
selected conjunction.

## Evidence status and projection boundary

This kernel is **bounded abstract model-checking evidence only**. It is not a
production trace-extraction theorem. Schema 5 of
`multilane_source_bindings.json` records the separate
`composed_state_action_relation_no_trace_extraction` contract. That source
binding fails closed if the model, configs, runners, documentation, accepted
version constants, exact payload/reservation fields, queue durability order,
Kura execution-input persistence/recovery consumers, composed Rust/Verus
projection, or the formal release receipt's distinct layout-only result row
drift. It remains outside the five production-refinement kernels.

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

The fixed-width composed state/action relation is now implemented in Rust and
mirrored in Verus for canonical committees of 1 through 128 validators. The
three-validator TLA+ state space embeds into that relation and remains bounded
abstract evidence. The relation covers selected QueuePlan V4 conjunction,
reservation V5, Kura Active, body fanout/late service, execution input, READY
authorization/signature/QC, crash/recover, lane commit, atomic WSV application,
post-carrier Commit/tombstone/ForgetCommit, and the four-stage release. The
three post-carrier cleanup stages advance one canonical ordered key at a time,
retain partial prefixes across crash/recovery, and expose Commit cleanup as
terminal canonical-WSV ownership only after the full ForgetCommit prefix. The
reverse terminal-owner projection classifies that completed cleanup as canonical-WSV
ownership and ordered/direct release as ordinary-FIFO ownership. V5 snapshot
reconstruction is an exact abstract stutter. The retired lane-wide removal
operation is absent from the current V5 schema; the schema-bound bootstrap and
operation decoder reject its old bytes without compatibility replay.

One narrow production consumer now extracts and consumes the composed
`DirectReleased` projection. The pre-Kura autonomous reservation-batch release
path holds the Queue transition and FIFO locks while it revalidates QueuePlan
V4, reservation V5, FIFO order, group binding, and committee geometry, then
consumes its move-only checked token immediately before the reservation
journal's `release_batch` append. Only after that durable append does it
publish ordinary-FIFO ownership. This local slice is not a complete production
trace-extraction theorem.

The remaining exact blocker is a machine-checked extraction from every other
Rust QueuePlan journal V4, reservation journal V5, Kura, recovery,
filesystem-error, restart, READY authorization/signature/QC, lane-commit,
atomic WSV application, post-carrier cleanup, and remaining Release
transitions into
`InFlightFirstReleaseSpec`, plus a backward ownership projection for every
concrete terminal Commit/Release outcome into the implemented reverse
terminal-owner projection.
Neither TLAPS nor Apalache consumes Rust operational semantics, and the
repository has no verified Rust semantics or trace-extraction theorem. The
fixed-width composed state/action relation is therefore not evidence for the
missing production trace-extraction theorem. Do not upgrade
`proof_coverage.json` or release status until that extraction is implemented
and checked at the production linearization points.
