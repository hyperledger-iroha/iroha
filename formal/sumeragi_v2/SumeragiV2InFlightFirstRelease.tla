---- MODULE SumeragiV2InFlightFirstRelease ----
EXTENDS Naturals, FiniteSets

(***************************************************************************
Finite safety kernel for the first-release, in-flight lane carrier path.

The accepted schema V1 carried by the Rust `LaneExecutablePayloadV1`
container is represented by `payloadBinding`. A validator is mapped to
`BindingA` only where authenticated custody of the selected FIFO-ordered
conjunction of exact `QueuePlanAdmissionBindingV1` preimages is established.
The production projection is the canonical reservation-group hash covering
every complete key in that order. Init establishes that custody for the
selected producer; it deliberately does not assert knowledge by every
validator.

QueuePlan journal V1 has individual Put records, not a batch-Put frame.
`SelectQueuePlanV1Conjunction` therefore observes that every exact claim in
the selected batch is already live in one content-bound replay snapshot.  It
does not stand for a nonexistent journal append.  The forward durable order
is

 selected QueuePlan V1 claim conjunction -> reservation journal V1 fsync ->
 Kura Active -> execution-input durability -> READY authorization -> local
 READY signature -> durable READY QC -> lane commit -> atomic WSV carrier
 application.

Reservation Commit, QueuePlan tombstoning, and ForgetCommit each advance one
canonical ordered key at a time after atomic WSV application. Their durable
key sets may expose mixed prefixes after a crash; the broad queue state changes
only when the corresponding prefix is complete. Post-WSV sidecar/index repairs
remain stuttering. The release
path separately models Kura retirement/ReleasePending claim prefixes, the
Queue PrepareRelease barrier, Released claim prefixes, and Queue
CompleteRelease/FIFO restoration/ForgetRelease. A nonproducer replica instead
uses a move-only exact-group Queue observation to classify either complete
local Queue absence or preservation of a pre-existing exact ordinary FIFO
owner; that observer path never claims the Queue release barrier or FIFO
restoration. Prefix counters are durable and survive Crash/Recover.

`session.bodies` and `session.readyAuthorized` are volatile.  Crash loses the
crashed validator's copies; durable Kura/input/QC and release-prefix facts
remain recoverable.

The model has a producer and two independently crashing replicated carriers.
Its three-validator states embed into the 1..128-validator fixed-width
Rust/Verus `ProductionInFlightFirstReleaseStateProjection` and transition
kernel. TLC and Apalache still establish only this bounded abstract transition
system; a production trace-extraction theorem remains a separate obligation.
***************************************************************************)

CONSTANTS
  \* @type: Str;
  Mode,
  \* @type: Str;
  Producer,
  \* @type: Str;
  ReplicaOne,
  \* @type: Str;
  ReplicaTwo,
  \* @type: Str;
  BindingA,
  \* @type: Str;
  BindingB

Modes ==
  {"Fixed",
   "ReservationBeforeSelectedQueuePlan",
   "KuraBeforeReservation",
   "ReadyAuthorizationBeforeInput",
   "ReadySignatureBeforeAuthorization",
   "ReadyQcBeforeSignatures",
   "CrashDropsDurable",
   "CrashRetainsVolatileBody",
   "PayloadBindingConflict",
   "LaneCommitScopeConflict",
   "ReleaseScopeConflict",
   "DuplicateApply",
   "ReservationCommitBeforeCarrier",
   "PlanTombstoneBeforeReservationCommit",
   "ForgetCommitBeforePlanTombstone",
   "CommitPrefixSkippedKey",
   "CommitPrefixDecrease",
   "ReleasePendingBeforeRetirement",
   "ReleasePrepareBeforePending",
   "ReleasedClaimsBeforePrepare",
   "ReleaseCompleteBeforeReleased",
   "ForgetReleaseBeforeFifo",
   "OversizeSelectedQueuePlan"}

Validators == {Producer, ReplicaOne, ReplicaTwo}
Ownership == {"ProducerSelected", "ReplicatedCarrier"}
OptionalBinding == {"None", BindingA, BindingB}
OptionalValidator == Validators \union {"None"}
QueuePlanStates == {"Absent", "SelectedConjunction", "Tombstoned"}
ReservationStates ==
  {"Absent", "Live", "Committed", "CommitForgotten",
   "ReleasePrepared", "ReleaseCompleted", "ReleaseForgotten",
   "DirectReleased", "ReplicaQueueAbsent", "ReplicaQueueFifoPreserved"}
PreparedReleaseStates ==
  {"ReleasePrepared", "ReleaseCompleted", "ReleaseForgotten"}
ReplicaQueueReleaseStates ==
  {"ReplicaQueueAbsent", "ReplicaQueueFifoPreserved"}
ReleasedClaimGateStates ==
  PreparedReleaseStates \union ReplicaQueueReleaseStates
CompletedReleaseStates == {"ReleaseCompleted", "ReleaseForgotten"}
FifoRestoredReservationStates ==
  CompletedReleaseStates \union {"DirectReleased"}
\* Canonical strict two-thirds count threshold for this fixed cardinality:
\* 3 - ((3 - 1) \div 3) = 3.
ReadyQuorum == 3
SelectedBatchSize ==
  IF Mode = "OversizeSelectedQueuePlan" THEN 4097 ELSE 2

PrefixThrough(prefix) ==
  IF prefix = 0 THEN {} ELSE 1..prefix

CanonicalKeyPrefix(keys, bound) ==
  /\ keys \subseteq PrefixThrough(bound)
  /\ keys = PrefixThrough(Cardinality(keys))

CommitTerminal ==
  /\ queue.reservation = "CommitForgotten"
  /\ Cardinality(history.reservationCommitForgottenKeys) = queue.selectedCount

ReplicaQueueAbsentTerminal ==
  /\ queue.reservation = "ReplicaQueueAbsent"
  /\ release.releasedPrefix = queue.selectedCount

ReplicaQueueFifoPreservedTerminal ==
  /\ queue.reservation = "ReplicaQueueFifoPreserved"
  /\ release.releasedPrefix = queue.selectedCount

OrdinaryFifoTerminal ==
  \/ queue.reservation \in {"ReleaseForgotten", "DirectReleased"}
  \/ ReplicaQueueFifoPreservedTerminal

ReleaseTerminal == OrdinaryFifoTerminal \/ ReplicaQueueAbsentTerminal

Configuration ==
  /\ Mode \in Modes
  /\ Cardinality(Validators) = 3
  /\ Producer # ReplicaOne
  /\ Producer # ReplicaTwo
  /\ ReplicaOne # ReplicaTwo
  /\ BindingA # BindingB

VARIABLES
  \* @type: Str -> Str;
  ownership,
  \* @type: Str -> Str;
  payloadBinding,
  queue,
  carrier,
  session,
  history,
  decision,
  release

vars ==
  <<ownership, payloadBinding, queue, carrier, session, history, decision,
    release>>

Init ==
  /\ Configuration
  /\ ownership = [p \in Validators |->
       IF p = Producer THEN "ProducerSelected" ELSE "ReplicatedCarrier"]
  /\ payloadBinding = [p \in Validators |->
       IF p = Producer THEN BindingA ELSE "None"]
  /\ queue =
       [plan |-> "Absent",
        selectedCount |-> 0,
        reservation |-> "Absent"]
  /\ carrier =
       [kuraActive |-> {},
        inputDurable |-> {},
        readyQcDurable |-> FALSE]
  /\ session =
       [bodies |-> {Producer},
        readyAuthorized |-> {},
        crashed |-> {},
        producerAlive |-> TRUE]
  /\ history =
       [everQueuePlanV1 |-> FALSE,
        everReservationV1 |-> FALSE,
        everInputDurable |-> {},
        everReadyAuthorized |-> {},
        readySigned |-> {},
        everReadyQcDurable |-> FALSE,
        reservationCommittedKeys |-> {},
        queuePlanTombstonedKeys |-> {},
        reservationCommitForgottenKeys |-> {},
        pendingHighWater |-> 0,
        releasedHighWater |-> 0]
  /\ decision =
       [laneCommitScope |-> "None",
        releaseScope |-> "None",
        laneCommitOwner |-> "None",
        releaseOwner |-> "None",
        wsvCommitted |-> FALSE,
        applicationCount |-> 0,
        appliedBy |-> "None"]
  /\ release =
       [kuraRetired |-> FALSE,
        pendingPrefix |-> 0,
        releasedPrefix |-> 0,
        fifoRestored |-> FALSE]

(***************************************************************************
One abstract observation over the selected content-bound V1 replay snapshot.
Every selected transaction already owns an individual durable V1 Put record.
***************************************************************************)
SelectQueuePlanV1Conjunction ==
  /\ queue.plan = "Absent"
  /\ queue' =
       [queue EXCEPT
          !.plan = "SelectedConjunction",
          !.selectedCount = SelectedBatchSize]
  /\ history' = [history EXCEPT !.everQueuePlanV1 = TRUE]
  /\ UNCHANGED <<ownership, payloadBinding, carrier, session, decision, release>>

FsyncReservationV1 ==
  /\ queue.reservation = "Absent"
  /\ (queue.plan = "SelectedConjunction"
      \/ Mode = "ReservationBeforeSelectedQueuePlan")
  /\ queue' = [queue EXCEPT !.reservation = "Live"]
  /\ history' = [history EXCEPT !.everReservationV1 = TRUE]
  /\ UNCHANGED <<ownership, payloadBinding, carrier, session, decision, release>>

ActivateKura(p) ==
  /\ p \in Validators
  /\ p \notin session.crashed
  /\ p \in session.bodies
  /\ (queue.reservation = "Live" \/ Mode = "KuraBeforeReservation")
  /\ carrier' =
       [carrier EXCEPT !.kuraActive = @ \union {p}]
  /\ UNCHANGED <<ownership, payloadBinding, queue, session, history, decision,
                 release>>

(***************************************************************************
Fanout and late-body service change only volatile transport custody.  The
durable body fact is established by ActivateKura, not by these actions.
***************************************************************************)
FanoutFromProducer(p) ==
  /\ p \in Validators \ {Producer}
  /\ session.producerAlive
  /\ Producer \in session.bodies
  /\ history.everReservationV1
  /\ p \notin session.crashed
  /\ session' = [session EXCEPT !.bodies = @ \union {p}]
  /\ UNCHANGED <<ownership, payloadBinding, queue, carrier, history, decision,
                 release>>

ServeLateBody(source, target) ==
  /\ source \in Validators
  /\ target \in Validators
  /\ source # target
  /\ source \in session.bodies
  /\ target \notin session.crashed
  /\ session' = [session EXCEPT !.bodies = @ \union {target}]
  /\ UNCHANGED <<ownership, payloadBinding, queue, carrier, history, decision,
                 release>>

PersistExecutionInput(p) ==
  /\ p \in carrier.kuraActive
  /\ p \in session.bodies
  /\ p \notin session.crashed
  /\ carrier' =
       [carrier EXCEPT !.inputDurable = @ \union {p}]
  /\ history' =
       [history EXCEPT !.everInputDurable = @ \union {p}]
  /\ UNCHANGED <<ownership, payloadBinding, queue, session, decision, release>>

AuthorizeReady(p) ==
  /\ p \in Validators
  /\ p \notin session.crashed
  /\ (p \in carrier.inputDurable
      \/ Mode = "ReadyAuthorizationBeforeInput")
  /\ session' =
       [session EXCEPT !.readyAuthorized = @ \union {p}]
  /\ history' =
       [history EXCEPT !.everReadyAuthorized = @ \union {p}]
  /\ UNCHANGED <<ownership, payloadBinding, queue, carrier, decision, release>>

SignReady(p) ==
  /\ p \in Validators
  /\ p \notin session.crashed
  /\ (p \in session.readyAuthorized
      \/ Mode = "ReadySignatureBeforeAuthorization")
  /\ history' = [history EXCEPT !.readySigned = @ \union {p}]
  /\ UNCHANGED <<ownership, payloadBinding, queue, carrier, session, decision,
                 release>>

PersistReadyQc ==
  /\ ~carrier.readyQcDurable
  /\ (Cardinality(history.readySigned) >= ReadyQuorum
      \/ Mode = "ReadyQcBeforeSignatures")
  /\ carrier' = [carrier EXCEPT !.readyQcDurable = TRUE]
  /\ history' = [history EXCEPT !.everReadyQcDurable = TRUE]
  /\ UNCHANGED <<ownership, payloadBinding, queue, session, decision, release>>

Crash(p) ==
  /\ p \in Validators
  /\ p \notin session.crashed
  /\ session' =
       [session EXCEPT
          !.crashed = @ \union {p},
          !.readyAuthorized = @ \ {p},
          !.bodies =
             IF Mode = "CrashRetainsVolatileBody" THEN @ ELSE @ \ {p},
          !.producerAlive =
             IF p = Producer THEN FALSE ELSE @]
  /\ queue' =
       IF Mode = "CrashDropsDurable"
          /\ p = Producer
          /\ queue.plan = "SelectedConjunction"
       THEN [queue EXCEPT
               !.plan = "Absent",
               !.reservation =
                  IF queue.reservation = "Live"
                  THEN "Absent"
                  ELSE queue.reservation]
       ELSE queue
  /\ UNCHANGED <<ownership, payloadBinding, carrier, history, decision, release>>

Recover(p) ==
  /\ p \in session.crashed
  /\ session' = [session EXCEPT !.crashed = @ \ {p}]
  /\ UNCHANGED <<ownership, payloadBinding, queue, carrier, history, decision,
                 release>>

(***************************************************************************
Reservation snapshot replay reconstructs process-local indexes from V1 bytes
that are already represented by the abstract durable owner. It is therefore
a named stutter, not a second reservation acquisition.
***************************************************************************)
RecoverReservationSnapshot ==
  UNCHANGED vars

(***************************************************************************
Direct abort/orphan release is a real journal action outside the ordered
four-stage lane release. It ends with ordinary FIFO ownership and may not
masquerade as a Commit cleanup or ordered-release transition. The V1 operation
inventory contains no lane-wide removal action.
***************************************************************************)
ReleaseReservationDirect ==
  /\ queue.plan = "SelectedConjunction"
  /\ queue.reservation = "Live"
  /\ decision.laneCommitOwner = "None"
  /\ decision.releaseOwner = "None"
  /\ queue' = [queue EXCEPT !.reservation = "DirectReleased"]
  /\ release' = [release EXCEPT !.fifoRestored = TRUE]
  /\ UNCHANGED <<ownership, payloadBinding, carrier, session, history, decision>>

(***************************************************************************
After `Recover` clears one local crash marker, exact durable Kura payload
ownership may restore only that validator's volatile body custody. The frozen
producer alone regains producer liveness. READY authorization and every
durable/economic fact remain unchanged, and terminal or retired work cannot be
resurrected.
***************************************************************************)
RehydrateLocalKuraCustody(p) ==
  /\ p \in Validators
  /\ p \notin session.crashed
  /\ p \in carrier.kuraActive
  /\ p \notin session.bodies
  /\ ~release.kuraRetired
  /\ ~decision.wsvCommitted
  /\ decision.releaseOwner = "None"
  /\ queue.reservation
       \notin {"CommitForgotten", "ReleaseForgotten", "DirectReleased"}
  /\ session' =
       [session EXCEPT
          !.bodies = @ \union {p},
          !.producerAlive = IF p = Producer THEN TRUE ELSE @]
  /\ UNCHANGED <<ownership, payloadBinding, queue, carrier, history, decision,
                 release>>

(***************************************************************************
LaneCommit is the lane consensus decision.  It is intentionally distinct from
the post-carrier reservation journal Commit action below.
***************************************************************************)
LaneCommit(p) ==
  /\ p \in history.readySigned
  /\ carrier.readyQcDurable
  /\ decision.laneCommitOwner = "None"
  /\ decision.releaseOwner = "None"
  /\ payloadBinding[p] = BindingA
  /\ decision' =
       [decision EXCEPT
          !.laneCommitOwner = p,
          !.laneCommitScope =
             IF Mode = "LaneCommitScopeConflict" THEN BindingB ELSE BindingA]
  /\ UNCHANGED <<ownership, payloadBinding, queue, carrier, session, history,
                 release>>

(***************************************************************************
The abstract ApplyCarrier step is the atomic WSV State commit.  Kura carrier
storage may precede it and receipts/index/cache repair may follow it, but none
of those auxiliary operations increments applicationCount.
***************************************************************************)
ApplyCarrier(p) ==
  /\ p = decision.laneCommitOwner
  /\ p \in Validators
  /\ (decision.applicationCount = 0 \/ Mode = "DuplicateApply")
  /\ decision' =
       [decision EXCEPT
          !.wsvCommitted = TRUE,
          !.applicationCount = @ + 1,
          !.appliedBy = p]
  /\ UNCHANGED <<ownership, payloadBinding, queue, carrier, session, history,
                 release>>

PersistReservationCommitted(key) ==
  /\ decision.laneCommitOwner # "None"
  /\ (decision.wsvCommitted \/ Mode = "ReservationCommitBeforeCarrier")
  /\ IF Mode = "CommitPrefixDecrease" /\ queue.reservation = "Committed"
        THEN
          /\ queue.reservation = "Committed"
          /\ history.queuePlanTombstonedKeys = {}
          /\ history.reservationCommitForgottenKeys = {}
          /\ Cardinality(history.reservationCommittedKeys) = queue.selectedCount
          /\ key = 1
          /\ queue' = [queue EXCEPT !.reservation = "Live"]
          /\ history' =
               [history EXCEPT
                  !.reservationCommittedKeys = @ \ {key}]
        ELSE
          LET nextKey ==
                Cardinality(history.reservationCommittedKeys)
                  + (IF Mode = "CommitPrefixSkippedKey" THEN 2 ELSE 1)
          IN
            /\ queue.reservation = "Live"
            /\ nextKey \in PrefixThrough(queue.selectedCount)
            /\ key = nextKey
            /\ queue' =
                 [queue EXCEPT
                    !.reservation =
                       IF nextKey = queue.selectedCount
                       THEN "Committed"
                       ELSE "Live"]
            /\ history' =
                 [history EXCEPT
                    !.reservationCommittedKeys = @ \union {key}]
  /\ UNCHANGED <<ownership, payloadBinding, carrier, session, decision, release>>

PersistPlanTombstone(key) ==
  /\ queue.plan = "SelectedConjunction"
  /\ (Cardinality(history.reservationCommittedKeys) = queue.selectedCount
      \/ Mode = "PlanTombstoneBeforeReservationCommit")
  /\ (queue.reservation = "Committed"
      \/ Mode = "PlanTombstoneBeforeReservationCommit")
  /\ key = Cardinality(history.queuePlanTombstonedKeys) + 1
  /\ key \in PrefixThrough(queue.selectedCount)
  /\ queue' =
       [queue EXCEPT
          !.plan =
             IF key = queue.selectedCount
             THEN "Tombstoned"
             ELSE "SelectedConjunction"]
  /\ history' =
       [history EXCEPT
          !.queuePlanTombstonedKeys = @ \union {key}]
  /\ UNCHANGED <<ownership, payloadBinding, carrier, session, decision, release>>

ForgetReservationCommit(key) ==
  /\ queue.reservation = "Committed"
  /\ (Cardinality(history.queuePlanTombstonedKeys) = queue.selectedCount
      \/ Mode = "ForgetCommitBeforePlanTombstone")
  /\ (queue.plan = "Tombstoned"
      \/ Mode = "ForgetCommitBeforePlanTombstone")
  /\ key = Cardinality(history.reservationCommitForgottenKeys) + 1
  /\ key \in PrefixThrough(queue.selectedCount)
  /\ queue' =
       [queue EXCEPT
          !.reservation =
             IF key = queue.selectedCount
             THEN "CommitForgotten"
             ELSE "Committed"]
  /\ history' =
       [history EXCEPT
          !.reservationCommitForgottenKeys = @ \union {key}]
  /\ UNCHANGED <<ownership, payloadBinding, carrier, session, decision, release>>

(***************************************************************************
Four-stage ordered release:
  1. durable Kura retirement plus prefix-recoverable ReleasePending claims;
  2. durable reservation PrepareRelease;
  3. prefix-recoverable Kura Released claims;
  4. reservation CompleteRelease, FIFO restore, then ForgetRelease.
***************************************************************************)
PersistKuraRetirement(p) ==
  /\ p \in carrier.kuraActive
  /\ queue.plan = "SelectedConjunction"
  /\ queue.reservation = "Live"
  /\ decision.laneCommitOwner = "None"
  /\ decision.releaseOwner = "None"
  /\ payloadBinding[p] = BindingA
  /\ decision' =
       [decision EXCEPT
          !.releaseOwner = p,
          !.releaseScope =
             IF Mode = "ReleaseScopeConflict" THEN BindingB ELSE BindingA]
  /\ release' = [release EXCEPT !.kuraRetired = TRUE]
  /\ UNCHANGED <<ownership, payloadBinding, queue, carrier, session, history>>

AdvanceReleasePendingPrefix ==
  /\ release.pendingPrefix < queue.selectedCount
  /\ queue.plan = "SelectedConjunction"
  /\ (release.kuraRetired \/ Mode = "ReleasePendingBeforeRetirement")
  /\ release' = [release EXCEPT !.pendingPrefix = @ + 1]
  /\ history' = [history EXCEPT !.pendingHighWater = @ + 1]
  /\ UNCHANGED <<ownership, payloadBinding, queue, carrier, session, decision>>

PrepareReservationRelease ==
  /\ decision.releaseOwner = Producer
  /\ queue.reservation = "Live"
  /\ (release.pendingPrefix = queue.selectedCount
      \/ Mode = "ReleasePrepareBeforePending")
  /\ queue' = [queue EXCEPT !.reservation = "ReleasePrepared"]
  /\ UNCHANGED <<ownership, payloadBinding, carrier, session, history, decision,
                 release>>

(***************************************************************************
A remote replica consumes an external move-only exact-group Queue observation
after every ReleasePending claim is durable. The disposition states distinguish
complete local Queue absence from an exact ordinary FIFO owner that already
existed and remains unchanged. This is not PrepareRelease or FIFO restoration.
***************************************************************************)
ObserveReplicaQueueRelease(disposition) ==
  /\ disposition \in ReplicaQueueReleaseStates
  /\ decision.releaseOwner \in Validators \ {Producer}
  /\ queue.plan = "SelectedConjunction"
  /\ queue.reservation = "Live"
  /\ release.kuraRetired
  /\ release.pendingPrefix = queue.selectedCount
  /\ release.releasedPrefix = 0
  /\ ~release.fifoRestored
  /\ queue' = [queue EXCEPT !.reservation = disposition]
  /\ UNCHANGED <<ownership, payloadBinding, carrier, session, history, decision,
                 release>>

AdvanceReleasedPrefix ==
  /\ decision.releaseOwner # "None"
  /\ release.releasedPrefix < queue.selectedCount
  /\ ((queue.reservation \in
         {"ReleasePrepared", "ReplicaQueueAbsent", "ReplicaQueueFifoPreserved"}
       /\ release.pendingPrefix = queue.selectedCount)
      \/ Mode = "ReleasedClaimsBeforePrepare")
  /\ release' = [release EXCEPT !.releasedPrefix = @ + 1]
  /\ history' = [history EXCEPT !.releasedHighWater = @ + 1]
  /\ UNCHANGED <<ownership, payloadBinding, queue, carrier, session, decision>>

CompleteReservationRelease ==
  /\ decision.releaseOwner # "None"
  /\ queue.reservation \in {"Live", "ReleasePrepared"}
  /\ ((queue.reservation = "ReleasePrepared"
       /\ release.releasedPrefix = queue.selectedCount)
      \/ Mode = "ReleaseCompleteBeforeReleased")
  /\ queue' = [queue EXCEPT !.reservation = "ReleaseCompleted"]
  /\ UNCHANGED <<ownership, payloadBinding, carrier, session, history, decision,
                 release>>

RestoreReleasedFifo ==
  /\ queue.reservation = "ReleaseCompleted"
  /\ ~release.fifoRestored
  /\ release' = [release EXCEPT !.fifoRestored = TRUE]
  /\ UNCHANGED <<ownership, payloadBinding, queue, carrier, session, history,
                 decision>>

ForgetReservationRelease ==
  /\ queue.reservation = "ReleaseCompleted"
  /\ (release.fifoRestored \/ Mode = "ForgetReleaseBeforeFifo")
  /\ queue' = [queue EXCEPT !.reservation = "ReleaseForgotten"]
  /\ UNCHANGED <<ownership, payloadBinding, carrier, session, history, decision,
                 release>>

(***************************************************************************
Compaction, pointer/index reconstruction, finality/receipt publication, and
other exact post-WSV repairs preserve every modeled fact.  This named action
makes their stuttering classification explicit.
***************************************************************************)
RepairPostCarrierEvidence ==
  /\ decision.wsvCommitted
  /\ UNCHANGED vars

ConflictingPayloadBindingMutation ==
  /\ Mode = "PayloadBindingConflict"
  /\ payloadBinding[Producer] = BindingA
  /\ payloadBinding' = [payloadBinding EXCEPT ![Producer] = BindingB]
  /\ UNCHANGED <<ownership, queue, carrier, session, history, decision, release>>

Next ==
  \/ SelectQueuePlanV1Conjunction
  \/ FsyncReservationV1
  \/ \E p \in Validators: ActivateKura(p)
  \/ \E p \in Validators \ {Producer}: FanoutFromProducer(p)
  \/ \E source \in Validators, target \in Validators:
       ServeLateBody(source, target)
  \/ \E p \in Validators: PersistExecutionInput(p)
  \/ \E p \in Validators: AuthorizeReady(p)
  \/ \E p \in Validators: SignReady(p)
  \/ PersistReadyQc
  \/ \E p \in Validators: Crash(p)
  \/ \E p \in Validators: Recover(p)
  \/ RecoverReservationSnapshot
  \/ ReleaseReservationDirect
  \/ \E p \in Validators: RehydrateLocalKuraCustody(p)
  \/ \E p \in Validators: LaneCommit(p)
  \/ \E p \in Validators: ApplyCarrier(p)
  \/ \E key \in PrefixThrough(SelectedBatchSize):
       PersistReservationCommitted(key)
  \/ \E key \in PrefixThrough(SelectedBatchSize): PersistPlanTombstone(key)
  \/ \E key \in PrefixThrough(SelectedBatchSize): ForgetReservationCommit(key)
  \/ \E p \in Validators: PersistKuraRetirement(p)
  \/ AdvanceReleasePendingPrefix
  \/ PrepareReservationRelease
  \/ \E disposition \in ReplicaQueueReleaseStates:
       ObserveReplicaQueueRelease(disposition)
  \/ AdvanceReleasedPrefix
  \/ CompleteReservationRelease
  \/ RestoreReleasedFifo
  \/ ForgetReservationRelease
  \/ RepairPostCarrierEvidence
  \/ ConflictingPayloadBindingMutation

FirstReleaseTypeInvariant ==
  /\ Configuration
  /\ ownership \in [Validators -> Ownership]
  /\ payloadBinding \in [Validators -> OptionalBinding]
  /\ queue \in
       [plan: QueuePlanStates,
        selectedCount: Nat,
        reservation: ReservationStates]
  /\ carrier \in
       [kuraActive: SUBSET Validators,
        inputDurable: SUBSET Validators,
        readyQcDurable: BOOLEAN]
  /\ session \in
       [bodies: SUBSET Validators,
        readyAuthorized: SUBSET Validators,
        crashed: SUBSET Validators,
        producerAlive: BOOLEAN]
  /\ history \in
       [everQueuePlanV1: BOOLEAN,
        everReservationV1: BOOLEAN,
        everInputDurable: SUBSET Validators,
        everReadyAuthorized: SUBSET Validators,
        readySigned: SUBSET Validators,
        everReadyQcDurable: BOOLEAN,
        reservationCommittedKeys: SUBSET PrefixThrough(queue.selectedCount),
        queuePlanTombstonedKeys: SUBSET PrefixThrough(queue.selectedCount),
        reservationCommitForgottenKeys: SUBSET PrefixThrough(queue.selectedCount),
        pendingHighWater: Nat,
        releasedHighWater: Nat]
  /\ decision \in
       [laneCommitScope: OptionalBinding,
        releaseScope: OptionalBinding,
        laneCommitOwner: OptionalValidator,
        releaseOwner: OptionalValidator,
        wsvCommitted: BOOLEAN,
        applicationCount: Nat,
        appliedBy: OptionalValidator]
  /\ release \in
       [kuraRetired: BOOLEAN,
        pendingPrefix: Nat,
        releasedPrefix: Nat,
        fifoRestored: BOOLEAN]

MLPayloadSchemaV2CarriesExactAdmissionPreimage ==
  /\ payloadBinding[Producer] = BindingA
  /\ \A p \in Validators:
       payloadBinding[p] = "None" \/ payloadBinding[p] = BindingA

MLValidatorCarrierOwnership ==
  /\ ownership[Producer] = "ProducerSelected"
  /\ \A p \in Validators \ {Producer}:
       ownership[p] = "ReplicatedCarrier"

MLSelectedQueuePlanV1ConjunctionBeforeReservationV1 ==
  queue.reservation # "Absent" => queue.plan # "Absent"

MLReservationV1BeforeKuraActive ==
  carrier.kuraActive # {} => history.everReservationV1

MLKuraActiveBeforeExecutionInput ==
  carrier.inputDurable \subseteq carrier.kuraActive

MLExecutionInputBeforeReadyAuthorization ==
  session.readyAuthorized \subseteq carrier.inputDurable

MLReadyAuthorizationBeforeLocalSignature ==
  history.readySigned \subseteq history.everReadyAuthorized

MLLocalSignaturesBeforeDurableReadyQc ==
  carrier.readyQcDurable =>
    Cardinality(history.readySigned) >= ReadyQuorum

MLCrashDurableFactsRecoverable ==
  /\ history.everQueuePlanV1 => queue.plan # "Absent"
  /\ history.everReservationV1 => queue.reservation # "Absent"
  /\ history.everInputDurable \subseteq carrier.inputDurable
  /\ history.everReadyQcDurable => carrier.readyQcDurable
  /\ history.pendingHighWater <= release.pendingPrefix
  /\ history.releasedHighWater <= release.releasedPrefix

MLVolatileSessionLostOnCrash ==
  /\ session.bodies \cap session.crashed = {}
  /\ session.readyAuthorized \cap session.crashed = {}

MLCommitAndReleaseRetainExactScope ==
  /\ decision.laneCommitScope # "None" =>
       decision.laneCommitScope = BindingA
  /\ decision.releaseScope # "None" =>
       decision.releaseScope = BindingA
  /\ decision.laneCommitOwner # "None" =>
       decision.releaseOwner = "None"
  /\ decision.releaseOwner # "None" =>
       decision.laneCommitOwner = "None"

MLLaneCommitBeforeAtomicWsvCarrierApplication ==
  /\ (decision.wsvCommitted <=> decision.applicationCount > 0)
  /\ decision.applicationCount > 0 =>
       /\ decision.laneCommitOwner # "None"
       /\ decision.appliedBy = decision.laneCommitOwner
       /\ decision.laneCommitScope = BindingA

MLExactlyOnceCarrierApplication ==
  /\ decision.applicationCount <= 1
  /\ decision.applicationCount > 0 =>
       /\ decision.appliedBy = decision.laneCommitOwner
       /\ decision.appliedBy \in carrier.inputDurable

MLPostCarrierCommitCleanupOrder ==
  /\ CanonicalKeyPrefix(
       history.reservationCommittedKeys,
       queue.selectedCount)
  /\ CanonicalKeyPrefix(
       history.queuePlanTombstonedKeys,
       queue.selectedCount)
  /\ CanonicalKeyPrefix(
       history.reservationCommitForgottenKeys,
       queue.selectedCount)
  /\ history.queuePlanTombstonedKeys
       \subseteq history.reservationCommittedKeys
  /\ history.reservationCommitForgottenKeys
       \subseteq history.queuePlanTombstonedKeys
  /\ history.reservationCommittedKeys # {} =>
       /\ decision.wsvCommitted
       /\ decision.laneCommitOwner # "None"
  /\ queue.reservation = "Committed" =>
       /\ decision.wsvCommitted
       /\ decision.laneCommitOwner # "None"
       /\ Cardinality(history.reservationCommittedKeys) = queue.selectedCount
       /\ Cardinality(history.reservationCommitForgottenKeys)
            < queue.selectedCount
  /\ queue.reservation = "Live" =>
       /\ Cardinality(history.reservationCommittedKeys) < queue.selectedCount
       /\ history.queuePlanTombstonedKeys = {}
       /\ history.reservationCommitForgottenKeys = {}
  /\ queue.reservation
       \notin {"Live", "Committed", "CommitForgotten"} =>
       /\ history.reservationCommittedKeys = {}
       /\ history.queuePlanTombstonedKeys = {}
       /\ history.reservationCommitForgottenKeys = {}
  /\ queue.plan = "Tombstoned" =>
       Cardinality(history.queuePlanTombstonedKeys) = queue.selectedCount
  /\ queue.plan = "SelectedConjunction" =>
       Cardinality(history.queuePlanTombstonedKeys) < queue.selectedCount
  /\ queue.reservation = "CommitForgotten" =>
       /\ queue.plan = "Tombstoned"
       /\ Cardinality(history.reservationCommittedKeys) = queue.selectedCount
       /\ Cardinality(history.queuePlanTombstonedKeys) = queue.selectedCount
       /\ Cardinality(history.reservationCommitForgottenKeys)
            = queue.selectedCount

MLReleasePrefixesRecoverable ==
  /\ release.pendingPrefix <= queue.selectedCount
  /\ release.releasedPrefix <= release.pendingPrefix
  /\ history.pendingHighWater <= release.pendingPrefix
  /\ history.releasedHighWater <= release.releasedPrefix

MLReleaseStageOrder ==
  /\ release.kuraRetired =>
       /\ decision.releaseOwner # "None"
       /\ decision.releaseScope = BindingA
  /\ release.pendingPrefix > 0 =>
       /\ release.kuraRetired
       /\ decision.releaseOwner # "None"
  /\ queue.reservation \in PreparedReleaseStates =>
       /\ release.kuraRetired
       /\ release.pendingPrefix = queue.selectedCount
       /\ decision.releaseOwner = Producer
  /\ queue.reservation \in ReplicaQueueReleaseStates =>
       /\ release.kuraRetired
       /\ release.pendingPrefix = queue.selectedCount
       /\ decision.releaseOwner \in Validators \ {Producer}
       /\ ~release.fifoRestored
  /\ release.releasedPrefix > 0 =>
       /\ queue.reservation \in ReleasedClaimGateStates
       /\ release.pendingPrefix = queue.selectedCount
  /\ queue.reservation \in CompletedReleaseStates =>
       release.releasedPrefix = queue.selectedCount
  /\ release.fifoRestored =>
       queue.reservation \in FifoRestoredReservationStates
  /\ queue.reservation = "ReleaseForgotten" =>
       release.fifoRestored
  /\ queue.reservation = "DirectReleased" =>
       release.fifoRestored

MLTerminalDispositionExclusive ==
  /\ ~(CommitTerminal /\ ReleaseTerminal)
  /\ (ReplicaQueueAbsentTerminal => ~OrdinaryFifoTerminal)
  /\ (ReplicaQueueFifoPreservedTerminal =>
       /\ OrdinaryFifoTerminal
       /\ ~release.fifoRestored)

MLQueuePlanV1SelectedConjunctionBound4096 ==
  /\ queue.selectedCount <= 4096
  /\ queue.plan # "Absent" => queue.selectedCount > 0

InFlightFirstReleaseSafetyInvariant ==
  /\ FirstReleaseTypeInvariant
  /\ MLPayloadSchemaV2CarriesExactAdmissionPreimage
  /\ MLValidatorCarrierOwnership
  /\ MLSelectedQueuePlanV1ConjunctionBeforeReservationV1
  /\ MLReservationV1BeforeKuraActive
  /\ MLKuraActiveBeforeExecutionInput
  /\ MLExecutionInputBeforeReadyAuthorization
  /\ MLReadyAuthorizationBeforeLocalSignature
  /\ MLLocalSignaturesBeforeDurableReadyQc
  /\ MLCrashDurableFactsRecoverable
  /\ MLVolatileSessionLostOnCrash
  /\ MLCommitAndReleaseRetainExactScope
  /\ MLLaneCommitBeforeAtomicWsvCarrierApplication
  /\ MLExactlyOnceCarrierApplication
  /\ MLPostCarrierCommitCleanupOrder
  /\ MLReleasePrefixesRecoverable
  /\ MLReleaseStageOrder
  /\ MLTerminalDispositionExclusive
  /\ MLQueuePlanV1SelectedConjunctionBound4096

InFlightFirstReleaseSpec == Init /\ [][Next]_vars

====
