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

The TLC corpus in this file is **bounded abstract model-checking evidence**; it
does not by itself establish a production refinement theorem. Schema 5 of
`multilane_source_bindings.json` records the separate
`composed_state_action_relation_with_source_bound_trace_extraction` contract.
The production trace-extraction certificate independently binds every model
action to its concrete checked-transition producer and move-only consumer, the
Rust refinement kernel, and the matching Verus theorem. That source binding
fails closed if the model, configs, runners, documentation, accepted version
constants, exact payload/reservation fields, Queue durability order, Kura
execution-input/recovery consumers, or certificate inputs drift. Release still
requires fresh positive and mutation-engine results; the source binding alone
does not claim those runs passed.

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

Current production bindings cover several bounded slices. For selection, the
canonical autonomous slot plan creates a move-only authorization containing
the exact reservation scope, frozen height-context identity, committee width,
and one-hot producer. After
exact QueuePlan V4/global registry/FIFO selection, Queue derives the complete
ordered reservation-group identity, checks `SelectQueuePlanV4Conjunction`, and
carries the checked `FsyncReservationV5` projection directly to the exact
journal `put_batch` append. A 4,097-entry request fails before culling, FIFO
mutation, or journal I/O.

For the local producer's Kura boundary, Queue revalidates that complete group
against the live V4 claims, V5 records, immutable FIFO ordinals, and exact
queued transactions. It returns a move-only authorization which retains the
per-transaction Queue transition fence. Kura validates the signed executable
payload, recomputes its canonical reservation-owner and proposal hashes from
the frozen height context and exact proposal descriptor, matches the ordered
group and producer committee bit, checks `ActivateKura`, and consumes that
authorization while the durable payload write runs. A substituted predecessor,
committee, QC domain, group member, or FIFO order therefore fails before the
producer persistence sink; concurrent Commit or release cannot invalidate the
checked Queue facts during that write.

Before the first autonomous execution-input sidecar append, Kura performs a
repair-disabled read of the exact producer-authenticated payload, reconstructs
the complete input, reservation group, committee geometry, and writer witness,
and mints a move-only authorization. The indexed writer matches the exact
input again, checks `PersistExecutionInput`, and consumes the authorization
before data/index publication; an exact replay only reissues durability
barriers and is an explicit storage stutter.

Before the first READY-QC view-state write, Kura validates the exact certificate,
canonical ordered reservation group, committee bitmap, producer, payload hash,
proposal hash, chain, and epoch into a move-only authorization. The writer
consumes it, checks `PersistReadyQc`, and only then calls the durable Kura sink;
an exact replay remains an explicit stutter. Before the first autonomous
certified-session publication, Kura reconstructs the exact repair-disabled
merge source, binds its payload, execution input, READY and Commit signer
sets, canonical source bytes, and reservation group into another move-only
authorization, then checks and consumes `LaneCommit` before the latest-frontier
sink. An exact certified-session replay remains a storage stutter. The durable
merge-source reader independently revalidates the exact same group while
checking `PersistExecutionInput`, `PersistReadyQc`, and `LaneCommit`. Kura's move-only
READY authorization checks `AuthorizeReady` from repair-disabled durable input
and carries the exact input hash, proposal, availability body, reservation
group, producer, signer, and height context. Its one-shot signer rederives the
committee bits and shared group identity, checks `SignReady`, and consumes the
checked projection immediately before `Signature::try_new`.

Before the first autonomous slot-retirement tombstone, Kura revalidates the
exact payload, incarnation-bound reservation group, committee geometry,
producer/writer custody, retirement identity, and target view-state path into a
move-only authorization. The writer consumes it, rechecks
`PersistKuraRetirement`, and reaches the validated atomic view-state sink only
afterward; an exact durable retirement retry is a storage stutter. Claim release
then reads and validates the complete ordered group before promoting a crash
temporary, deleting a redundant temporary, or replacing any claim. Only
`ReleasePending* / Active*` and `Released* / ReleasePending*` crash prefixes are
accepted. Each missing prefix element gets its own path-and-replacement-bound
authorization and checked `AdvanceReleasePendingPrefix` or
`AdvanceReleasedPrefix` projection immediately before the synced atomic replace.
The source contract binds the write/flush/fsync/rename/fsync-directory order,
and adversarial tests require invalid mixed-stage groups to remain byte-identical
while canonical restart prefixes resume idempotently.

Canonical application retains every checked `ApplyCarrier` projection in a
move-only `CheckedCarrierApplications` batch. V2 binds its exact finalized
block, merge entry, and lane cardinality, then boxes it as a
`StateBlockCommitAuthorization`. `StateBlock::commit_inner` consumes that
authorization while holding `state_commit_lock` and, for lifecycle changes,
the lane-lifecycle fence, after the final scale-in Queue veto and before lane
geometry or `transactions.commit()` can publish. V2 acquires the Queue
retirement observer only when the staged block reports an exact pending
scale-in. Queue then checks each ordered `PersistReservationCommitted`,
`PersistPlanTombstone`, and `ForgetReservationCommit` prefix against the same
group. Separately, the pre-Kura reservation-batch release path uses the same
complete-group revalidation predicate while holding the Queue transition and
FIFO locks, checks committee geometry, then consumes its move-only
checked `DirectReleased` token immediately before the journal `release_batch`
append. These local slices are not a complete production trace-extraction
theorem.

Post-carrier evidence repair now starts from the same authenticated autonomous
source-bundle projection as `ApplyCarrier`. A move-only
`PostCarrierEvidenceRepairAuthorization` binds the accepted post-WSV state to
the exact merge-entry hash, canonical carrier height/hash, and complete ordered
reservation group. The live apply path mints it only after
`State::ensure_globally_committed_merge_entry_applied`; State journal replay
does so only after `merge_execution_already_applied`; and unified startup repair
does so only after its all-item State/Kura preflight. Kura revalidates the exact
committed log/carrier or finalized reverse-carrier plan, consumes every checked
stutter token before the first receipt or reverse-index publication, and has no
ungated production receipt-repair entrypoint. The source-certificate contract
binds all three seams to `RepairPostCarrierEvidence`.

The source-certificate contract authenticates that direct-release seam plus
`FanoutFromProducer` and `ServeLateBody`. Lane-committee producer publication
consumes one Queue-fenced exact-slot token at each fresh effect insertion after
exact Kura readback; periodic retransmission must reconstruct and match the
same slot before using that shared gate. Operational cache copies to global
validators outside the lane committee have no actor bit in this model and are
not claimed as `FanoutFromProducer` actions. Autonomous late-body response
publication consumes exact certified Kura evidence at its fresh insertion
boundary. Exact queued duplicates are classified before either token is minted
and remain stutters. The four formerly open model actions, `Crash`, `Recover`,
`RecoverReservationSnapshot`, and `RehydrateLocalKuraCustody`, now have named
source-extraction seams through signed lifecycle bootstrap, generation
takeover, Queue snapshot recovery, local Kura rehydration, drain-queue
installation, and one-shot activation revalidation. The registered open-action
tuple is empty.

For `RecoverReservationSnapshot`, the source contract binds the primitive V5
snapshot transition, exact owner/record/release normalization, ordered coverage
root, post-open file-content revalidation, move-only Queue install and planning
receipts, and the final publication gate. Its parametric Verus lemma proves
noninterference for an independently valid composed state, while the production
bridge supplies the checked transition and exact authorization source. That
source partition does not itself prove that every concrete replay execution
refines the abstract transition.

`RehydrateLocalKuraCustody` is now a separate first-release action instead of
being inferred from `Recover` or late-body service. Its shared Rust/Verus/TLA+
kernel requires one non-crashed local validator with exact durable Kura payload
ownership and missing volatile body custody. It adds only that actor's body,
revives producer liveness only for the frozen producer, preserves READY
authorization and every durable/economic fact, and rejects retired,
post-application, or terminal work. The source contract includes positive,
guard-omission, READY-tampering, and terminal-resurrection controls. The
startup consumer now derives and consumes the checked custody transition at
the production rehydration boundary.

The remaining blocker is formal operational correspondence and fresh evidence,
not a missing name in the structural action partition. Neither TLAPS nor
Apalache consumes Rust operational semantics, and the repository has no
verified Rust operational semantics or production trace-extraction theorem.
The fixed-width composed state/action relation and source-bound transition
checks therefore do not prove that every concrete execution refines
`InFlightFirstReleaseSpec`, nor do they replace immutable-candidate TLC,
Apalache, TLAPS, Verus, mutation, trace-replay, and cross-tool receipts. Do not
treat the checked-in promoted `proof_coverage.json` status vector as release
evidence. Release status and receipts remain blocked until those obligations
are executed and archived against one immutable candidate.
