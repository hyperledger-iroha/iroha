---- MODULE SumeragiV2CertifiedRequestCapacityMutation ----
EXTENDS Naturals, Sequences, FiniteSets

(***************************************************************************
Finite mutation model for the independent certified-request capacity seam.

The concrete prefix keeps pending-work capacity P, certified-request capacity
Q, and the bounded retained AdapterEffect FIFO T distinct:

  1. Fetch A owns one P slot.  In the P-pressure scenario it is ordinary and
     fills P while Q remains free; in both Q-pressure scenarios it also owns
     the sole Q slot;
  2. Fetch B is either genuinely new or, in the upgrade scenario, already
     owns one ordinary, uncertified P slot;
  3. higher certified authority for B is admitted once as one exact retained
     owner.  Its task, authority, lifecycle ordinal, and FIFO position are
     frozen.  B gains no new P owner and no Q owner before capacity releases;
  4. the authenticated response for A remains admissible through independent
     outer count and canonical-envelope byte reserves, even while unrelated
     retained debt is present;
  5. consuming response A atomically retires A's exact P and Q owners; and
  6. DrainRetainedFetchB removes the retained head and atomically installs
     B's exact P/Q ownership.  For an existing ordinary B, the P task identity
     is upgraded in place; for a new B, P and Q appear together.

RetentionPolicy supplies four independent negative controls:

  DropRetentionPolicy        drops B after accepting its producer;
  SubstituteRetentionPolicy  replaces B's exact task and authority;
  DuplicateRetentionPolicy   creates two owners for one admission; and
  OvertakeRetentionPolicy    lets a later control cross B's FIFO position.

RequestOnlyDrainPolicy independently exposes a non-atomic Q-only drain.
FailOnCapacityBlockedFetch, the two outer reserve switches, and
AllowTransportResponseUnderDebt preserve the independent pressure mutations
that were already covered by this model.

This compact finite mutation/refinement obligation covers the live-process
executor and outer-ingress seam.  It does not model Byzantine response
validation, crash/restart reconstruction, or deductive protocol liveness.
***************************************************************************)

CONSTANTS RetentionPolicy,
          DrainPolicy,
          FailOnCapacityBlockedFetch,
          ReserveTransportResponseCount,
          ReserveTransportResponseBytes,
          AllowTransportResponseUnderDebt

FixedRetentionPolicy == "FixedRetention"
DropRetentionPolicy == "DropRetention"
SubstituteRetentionPolicy == "SubstituteRetention"
DuplicateRetentionPolicy == "DuplicateRetention"
OvertakeRetentionPolicy == "OvertakeRetention"

AtomicDrainPolicy == "AtomicDrain"
RequestOnlyDrainPolicy == "RequestOnlyDrain"

ASSUME RetentionPolicy \in
  {FixedRetentionPolicy,
   DropRetentionPolicy,
   SubstituteRetentionPolicy,
   DuplicateRetentionPolicy,
   OvertakeRetentionPolicy}
ASSUME DrainPolicy \in {AtomicDrainPolicy, RequestOnlyDrainPolicy}
ASSUME FailOnCapacityBlockedFetch \in BOOLEAN
ASSUME ReserveTransportResponseCount \in BOOLEAN
ASSUME ReserveTransportResponseBytes \in BOOLEAN
ASSUME AllowTransportResponseUnderDebt \in BOOLEAN

CertifiedRequestCapacity == 1
ReducerMaxEffectsPerStep == 8

FullWorkPressureScenario == "FullWorkPressure"
NewRequestPressureScenario == "NewRequestPressure"
UpgradeRequestPressureScenario == "UpgradeRequestPressure"
PressureScenarios ==
  {FullWorkPressureScenario,
   NewRequestPressureScenario,
   UpgradeRequestPressureScenario}

NoTask == [round |-> 0, subject |-> "None"]
TaskB == [round |-> 2, subject |-> "SubjectB"]
SubstituteTask == [round |-> 2, subject |-> "SubjectC"]

NoAuthority ==
  [phase |-> "None", certificate |-> "None", sources |-> "None"]
AuthorityA ==
  [phase |-> "Prepare", certificate |-> "PrepareQCA", sources |-> "RosterA"]
AuthorityB ==
  [phase |-> "Prepare", certificate |-> "PrepareQCB", sources |-> "RosterB"]
SubstituteAuthority ==
  [phase |-> "Prepare", certificate |-> "PrepareQCC", sources |-> "RosterC"]

NoRetainedOwner ==
  [effect |-> "None",
   task |-> NoTask,
   authority |-> NoAuthority,
   lifecycleOrdinal |-> 0]
ExactFetchBOwner ==
  [effect |-> "FetchBody",
   task |-> TaskB,
   authority |-> AuthorityB,
   lifecycleOrdinal |-> 2]
SubstituteFetchOwner ==
  [effect |-> "FetchBody",
   task |-> SubstituteTask,
   authority |-> SubstituteAuthority,
   lifecycleOrdinal |-> 2]
OvertakingControlOwner ==
  [effect |-> "TimeoutVoteSign",
   task |-> SubstituteTask,
   authority |-> NoAuthority,
   lifecycleOrdinal |-> 3]

RetainedOwners ==
  {ExactFetchBOwner, SubstituteFetchOwner, OvertakingControlOwner}

NoRequest == "None"
FetchA == "FetchA"
FetchB == "FetchB"
RequestOwners == {NoRequest, FetchA, FetchB}

VARIABLES pressureScenario,
          phase,
          preexistingOrdinaryWorkB,
          workA,
          workB,
          workBTask,
          certifiedWorkB,
          requestOwner,
          requestAuthority,
          retainedEffects,
          dispatchedFetchOwner,
          unrelatedRetainedT,
          higherAuthorityEmitted,
          outerGenericCountOwned,
          outerGenericBytesOwned,
          responseAAdmitted,
          responseAQueued,
          responseAConsumed,
          fatal,
          independentCapacityWitness

vars ==
  <<pressureScenario,
    phase,
    preexistingOrdinaryWorkB,
    workA,
    workB,
    workBTask,
    certifiedWorkB,
    requestOwner,
    requestAuthority,
    retainedEffects,
    dispatchedFetchOwner,
    unrelatedRetainedT,
    higherAuthorityEmitted,
    outerGenericCountOwned,
    outerGenericBytesOwned,
    responseAAdmitted,
    responseAQueued,
    responseAConsumed,
    fatal,
    independentCapacityWitness>>

BoolCount(value) == IF value THEN 1 ELSE 0

PendingWorkCount == BoolCount(workA) + BoolCount(workB)

CertifiedRequestCount == IF requestOwner = NoRequest THEN 0 ELSE 1

GeneralWorkCapacity ==
  IF pressureScenario = FullWorkPressureScenario THEN 1 ELSE 2

ExactFetchBOwnerIndexes ==
  {index \in DOMAIN retainedEffects:
     retainedEffects[index] = ExactFetchBOwner}

ExactFetchBOwnerCount == Cardinality(ExactFetchBOwnerIndexes)

TypeInvariant ==
  /\ pressureScenario \in PressureScenarios
  /\ phase \in 0..4
  /\ preexistingOrdinaryWorkB \in BOOLEAN
  /\ workA \in BOOLEAN
  /\ workB \in BOOLEAN
  /\ workBTask \in {NoTask, TaskB}
  /\ certifiedWorkB \in BOOLEAN
  /\ requestOwner \in RequestOwners
  /\ requestAuthority \in {NoAuthority, AuthorityA, AuthorityB}
  /\ retainedEffects \in Seq(RetainedOwners)
  /\ Len(retainedEffects) <= ReducerMaxEffectsPerStep
  /\ dispatchedFetchOwner \in RetainedOwners \cup {NoRetainedOwner}
  /\ unrelatedRetainedT \in BOOLEAN
  /\ higherAuthorityEmitted \in BOOLEAN
  /\ outerGenericCountOwned \in BOOLEAN
  /\ outerGenericBytesOwned \in BOOLEAN
  /\ responseAAdmitted \in BOOLEAN
  /\ responseAQueued \in BOOLEAN
  /\ responseAConsumed \in BOOLEAN
  /\ fatal \in BOOLEAN
  /\ independentCapacityWitness \in BOOLEAN

PendingWorkWithinCapacity == PendingWorkCount <= GeneralWorkCapacity

CertifiedRequestsWithinCapacity ==
  CertifiedRequestCount <= CertifiedRequestCapacity

PressureScenarioMatchesPOwnership ==
  preexistingOrdinaryWorkB =
    (pressureScenario = UpgradeRequestPressureScenario)

WorkBTaskMatchesExactPOwnership ==
  /\ workB => workBTask = TaskB
  /\ ~workB => workBTask = NoTask

ExactRequestHasWorkOwner ==
  /\ (requestOwner = FetchA) =>
       /\ workA
       /\ requestAuthority = AuthorityA
  /\ (requestOwner = FetchB) =>
       /\ workB
       /\ workBTask = TaskB
       /\ certifiedWorkB
       /\ requestAuthority = AuthorityB
  /\ (requestOwner = NoRequest) => requestAuthority = NoAuthority

(***************************************************************************
The witness distinguishes the capacities: Q is free in the full-P scenario; a
new Q-blocked Fetch B still has a free P slot; and an ordinary Q-blocked B
already owns its stable P slot and needs only an authority upgrade in Q.
***************************************************************************)
IndependentRequestCapacityWitness ==
  higherAuthorityEmitted => independentCapacityWitness

(***************************************************************************
The capacity-blocked effect remains one exact FIFO owner.  These predicates
are deliberately separate so drop, substitution, duplication, and overtaking
mutations fail at their own semantic boundary.
***************************************************************************)
RetainedFetchBIsNotDropped ==
  (phase \in 1..3) => Len(retainedEffects) >= 1

RetainedFetchBHasExactAuthorityAndTask ==
  (phase \in 1..3) => ExactFetchBOwnerCount >= 1

RetainedFetchBHasOneOwner ==
  (phase \in 1..3) => ExactFetchBOwnerCount = 1

RetainedFetchBRemainsFifoHead ==
  (phase \in 1..3) =>
    /\ Len(retainedEffects) >= 1
    /\ retainedEffects[1] = ExactFetchBOwner

RetainedEffectFifoIsBounded ==
  Len(retainedEffects) <= ReducerMaxEffectsPerStep

(***************************************************************************
Before the retained head drains, B has exactly its prior ordinary P state and
no certified Q state.  No work ID, pipeline owner, certified bit, request
owner, or request authority is partially installed.
***************************************************************************)
CapacityBlockedFetchBHasNoPartialPQT ==
  (phase \in 1..3) =>
    /\ workB = preexistingOrdinaryWorkB
    /\ workBTask = IF preexistingOrdinaryWorkB THEN TaskB ELSE NoTask
    /\ ~certifiedWorkB
    /\ requestOwner # FetchB
    /\ requestAuthority # AuthorityB

CertifiedRequestPressureIsNonfatal ==
  higherAuthorityEmitted => ~fatal

UnrelatedRetainedDebtIsPreserved == unrelatedRetainedT

ResponseARetiresExactWorkAndRequest ==
  responseAConsumed =>
    /\ responseAAdmitted
    /\ ~responseAQueued
    /\ ~workA
    /\ requestOwner # FetchA
    /\ requestAuthority # AuthorityA

OuterGenericSaturationIsPreserved ==
  /\ outerGenericCountOwned
  /\ outerGenericBytesOwned

OuterResponseAdmissionUsesBothReserves ==
  responseAAdmitted =>
    /\ ReserveTransportResponseCount
    /\ ReserveTransportResponseBytes

QueuedResponseIsExactAdmittedOwner ==
  responseAQueued =>
    /\ responseAAdmitted
    /\ ~responseAConsumed

(***************************************************************************
After A releases capacity, one DrainRetainedFetchB transition carries the
same retained owner into exact P/Q state and removes it from the FIFO.  The
existing-B branch preserves TaskB; the new-B branch allocates TaskB once.
***************************************************************************)
DrainRetainedFetchBIsAtomic ==
  (phase = 4) =>
    /\ (pressureScenario = FullWorkPressureScenario \/ responseAConsumed)
    /\ ~workA
    /\ workB
    /\ workBTask = TaskB
    /\ certifiedWorkB
    /\ requestOwner = FetchB
    /\ requestAuthority = AuthorityB
    /\ dispatchedFetchOwner = ExactFetchBOwner
    /\ retainedEffects = <<>>
    /\ PendingWorkCount = 1
    /\ CertifiedRequestCount = CertifiedRequestCapacity

Init ==
  /\ pressureScenario \in PressureScenarios
  /\ phase = 0
  /\ preexistingOrdinaryWorkB =
       (pressureScenario = UpgradeRequestPressureScenario)
  /\ workA = TRUE
  /\ workB = preexistingOrdinaryWorkB
  /\ workBTask = IF preexistingOrdinaryWorkB THEN TaskB ELSE NoTask
  /\ certifiedWorkB = FALSE
  /\ requestOwner =
       IF pressureScenario = FullWorkPressureScenario
         THEN NoRequest
         ELSE FetchA
  /\ requestAuthority =
       IF pressureScenario = FullWorkPressureScenario
         THEN NoAuthority
         ELSE AuthorityA
  /\ retainedEffects = <<>>
  /\ dispatchedFetchOwner = NoRetainedOwner
  /\ unrelatedRetainedT = TRUE
  /\ higherAuthorityEmitted = FALSE
  /\ outerGenericCountOwned = TRUE
  /\ outerGenericBytesOwned = TRUE
  /\ responseAAdmitted = FALSE
  /\ responseAQueued = FALSE
  /\ responseAConsumed = FALSE
  /\ fatal = FALSE
  /\ independentCapacityWitness = FALSE

(***************************************************************************
The serialized executor accepts the producer once.  P and Q remain unchanged;
only the exact owned effect enters T.  Each retention policy changes this one
boundary and is otherwise unable to mint P/Q ownership.
***************************************************************************)
RetainCapacityBlockedFetchB ==
  /\ phase = 0
  /\ workA
  /\ workB = preexistingOrdinaryWorkB
  /\ workBTask = IF preexistingOrdinaryWorkB THEN TaskB ELSE NoTask
  /\ ~certifiedWorkB
  /\ IF pressureScenario = FullWorkPressureScenario
       THEN /\ ~preexistingOrdinaryWorkB
            /\ PendingWorkCount = GeneralWorkCapacity
            /\ requestOwner = NoRequest
            /\ requestAuthority = NoAuthority
            /\ CertifiedRequestCount < CertifiedRequestCapacity
       ELSE /\ \/ /\ ~preexistingOrdinaryWorkB
                   /\ PendingWorkCount < GeneralWorkCapacity
                \/ /\ preexistingOrdinaryWorkB
                   /\ PendingWorkCount = GeneralWorkCapacity
            /\ requestOwner = FetchA
            /\ requestAuthority = AuthorityA
            /\ CertifiedRequestCount = CertifiedRequestCapacity
  /\ phase' = 1
  /\ retainedEffects' =
       CASE RetentionPolicy = FixedRetentionPolicy ->
              <<ExactFetchBOwner>>
         [] RetentionPolicy = DropRetentionPolicy ->
              <<>>
         [] RetentionPolicy = SubstituteRetentionPolicy ->
              <<SubstituteFetchOwner>>
         [] RetentionPolicy = DuplicateRetentionPolicy ->
              <<ExactFetchBOwner, ExactFetchBOwner>>
         [] RetentionPolicy = OvertakeRetentionPolicy ->
              <<OvertakingControlOwner, ExactFetchBOwner>>
  /\ higherAuthorityEmitted' = TRUE
  /\ fatal' = FailOnCapacityBlockedFetch
  /\ independentCapacityWitness' = TRUE
  /\ UNCHANGED
       <<pressureScenario,
         preexistingOrdinaryWorkB,
         workA,
         workB,
         workBTask,
         certifiedWorkB,
         requestOwner,
         requestAuthority,
         dispatchedFetchOwner,
         unrelatedRetainedT,
         outerGenericCountOwned,
         outerGenericBytesOwned,
         responseAAdmitted,
         responseAQueued,
         responseAConsumed>>

(***************************************************************************
Reducer-producing count and byte owners saturate their ordinary outer ingress
partitions.  The authenticated response crosses that boundary only through a
dedicated response count slot and full canonical-envelope byte reserve.
Unrelated retained T and retained Fetch B cannot block the response.
***************************************************************************)
AdmitOuterTransportResponseA ==
  /\ phase = 1
  /\ pressureScenario # FullWorkPressureScenario
  /\ ~fatal
  /\ Len(retainedEffects) >= 1
  /\ retainedEffects[1] = ExactFetchBOwner
  /\ unrelatedRetainedT
  /\ workA
  /\ requestOwner = FetchA
  /\ requestAuthority = AuthorityA
  /\ outerGenericCountOwned
  /\ outerGenericBytesOwned
  /\ ReserveTransportResponseCount
  /\ ReserveTransportResponseBytes
  /\ ~responseAAdmitted
  /\ ~responseAQueued
  /\ phase' = 2
  /\ responseAAdmitted' = TRUE
  /\ responseAQueued' = TRUE
  /\ UNCHANGED
       <<pressureScenario,
         preexistingOrdinaryWorkB,
         workA,
         workB,
         workBTask,
         certifiedWorkB,
         requestOwner,
         requestAuthority,
         retainedEffects,
         dispatchedFetchOwner,
         unrelatedRetainedT,
         higherAuthorityEmitted,
         outerGenericCountOwned,
         outerGenericBytesOwned,
         responseAConsumed,
         fatal,
         independentCapacityWitness>>

(***************************************************************************
The admitted response is transport-only.  It crosses unrelated retained T and
atomically releases the exact P and Q resources owned by A.
***************************************************************************)
ConsumeTransportOnlyResponseA ==
  /\ phase = 2
  /\ pressureScenario # FullWorkPressureScenario
  /\ ~fatal
  /\ Len(retainedEffects) >= 1
  /\ retainedEffects[1] = ExactFetchBOwner
  /\ unrelatedRetainedT
  /\ AllowTransportResponseUnderDebt
  /\ responseAAdmitted
  /\ responseAQueued
  /\ workA
  /\ requestOwner = FetchA
  /\ requestAuthority = AuthorityA
  /\ phase' = 3
  /\ workA' = FALSE
  /\ requestOwner' = NoRequest
  /\ requestAuthority' = NoAuthority
  /\ responseAQueued' = FALSE
  /\ responseAConsumed' = TRUE
  /\ UNCHANGED
       <<pressureScenario,
         preexistingOrdinaryWorkB,
         workB,
         workBTask,
         certifiedWorkB,
         retainedEffects,
         dispatchedFetchOwner,
         unrelatedRetainedT,
         higherAuthorityEmitted,
         outerGenericCountOwned,
         outerGenericBytesOwned,
         responseAAdmitted,
         fatal,
         independentCapacityWitness>>

(***************************************************************************
P-pressure has no certified A response.  Completion of A's ordinary service
owner releases the sole P slot directly, leaving retained Fetch B untouched
and Q still empty.
***************************************************************************)
ReleaseOrdinaryWorkCapacityA ==
  /\ pressureScenario = FullWorkPressureScenario
  /\ phase = 1
  /\ ~fatal
  /\ Len(retainedEffects) >= 1
  /\ retainedEffects[1] = ExactFetchBOwner
  /\ workA
  /\ PendingWorkCount = GeneralWorkCapacity
  /\ requestOwner = NoRequest
  /\ requestAuthority = NoAuthority
  /\ phase' = 3
  /\ workA' = FALSE
  /\ UNCHANGED
       <<pressureScenario,
         preexistingOrdinaryWorkB,
         workB,
         workBTask,
         certifiedWorkB,
         requestOwner,
         requestAuthority,
         retainedEffects,
         dispatchedFetchOwner,
         unrelatedRetainedT,
         higherAuthorityEmitted,
         outerGenericCountOwned,
         outerGenericBytesOwned,
         responseAAdmitted,
         responseAQueued,
         responseAConsumed,
         fatal,
         independentCapacityWitness>>

(***************************************************************************
Capacity release drains the already-admitted owner directly; there is no
periodic reconstruction step.  AtomicDrain installs or upgrades exact P/Q
state in one transition.  RequestOnlyDrain is the partial-ownership mutant.
***************************************************************************)
DrainRetainedFetchB ==
  /\ phase = 3
  /\ ~fatal
  /\ (pressureScenario = FullWorkPressureScenario \/ responseAConsumed)
  /\ ~workA
  /\ requestOwner = NoRequest
  /\ requestAuthority = NoAuthority
  /\ CertifiedRequestCount < CertifiedRequestCapacity
  /\ Len(retainedEffects) >= 1
  /\ retainedEffects[1] = ExactFetchBOwner
  /\ workB = preexistingOrdinaryWorkB
  /\ workBTask = IF preexistingOrdinaryWorkB THEN TaskB ELSE NoTask
  /\ ~certifiedWorkB
  /\ phase' = 4
  /\ retainedEffects' = Tail(retainedEffects)
  /\ dispatchedFetchOwner' = ExactFetchBOwner
  /\ IF DrainPolicy = AtomicDrainPolicy
       THEN /\ workB' = TRUE
            /\ workBTask' = TaskB
            /\ certifiedWorkB' = TRUE
            /\ requestOwner' = FetchB
            /\ requestAuthority' = AuthorityB
       ELSE /\ workB' = workB
            /\ workBTask' = workBTask
            /\ certifiedWorkB' = FALSE
            /\ requestOwner' = FetchB
            /\ requestAuthority' = AuthorityB
  /\ UNCHANGED
       <<pressureScenario,
         preexistingOrdinaryWorkB,
         workA,
         unrelatedRetainedT,
         higherAuthorityEmitted,
         outerGenericCountOwned,
         outerGenericBytesOwned,
         responseAAdmitted,
         responseAQueued,
         responseAConsumed,
         fatal,
         independentCapacityWitness>>

Next ==
  \/ RetainCapacityBlockedFetchB
  \/ AdmitOuterTransportResponseA
  \/ ConsumeTransportOnlyResponseA
  \/ ReleaseOrdinaryWorkCapacityA
  \/ DrainRetainedFetchB

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(RetainCapacityBlockedFetchB)
  /\ WF_vars(AdmitOuterTransportResponseA)
  /\ WF_vars(ConsumeTransportOnlyResponseA)
  /\ WF_vars(ReleaseOrdinaryWorkCapacityA)
  /\ WF_vars(DrainRetainedFetchB)

OuterResponseEventuallyAdmitted ==
  (phase = 1
     /\ pressureScenario # FullWorkPressureScenario
     /\ retainedEffects = <<ExactFetchBOwner>>
     /\ ~fatal)
    ~> responseAAdmitted

TransportResponseEventuallyReleasesA ==
  (phase = 1
     /\ pressureScenario # FullWorkPressureScenario
     /\ retainedEffects = <<ExactFetchBOwner>>
     /\ ~fatal)
    ~> responseAConsumed

RetainedFetchBEventuallyOwnsExactRequest ==
  (phase = 1 /\ retainedEffects = <<ExactFetchBOwner>> /\ ~fatal)
    ~> (phase = 4
          /\ requestOwner = FetchB
          /\ requestAuthority = AuthorityB
          /\ certifiedWorkB
          /\ dispatchedFetchOwner = ExactFetchBOwner)

RepairedCertifiedRequestProgress ==
  /\ OuterResponseEventuallyAdmitted
  /\ TransportResponseEventuallyReleasesA
  /\ RetainedFetchBEventuallyOwnsExactRequest

=============================================================================
