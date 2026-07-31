---- MODULE SumeragiV2EffectCapacityRetirementMutation ----
EXTENDS Naturals, Sequences

(***************************************************************************
Finite Stage-6 mutation for the non-preemptible capacity branch.

Two terminating owners fill the concrete work capacity two.  BlockingOwner
is checked once as a decided-body Fetch and once as non-Fetch local work.  A
genuinely new Fetch is then attempted while full: production leaves the
reducer in Missing with a reconstructible request and does not retain that
Fetch in the adapter FIFO.  The subsequently persisted TimeoutVote Sign is
therefore the FIFO head.  Weak fairness for terminating local work retires
one protected owner, after which FIFO priority admits the retained Sign.

Crash/restart authority is intentionally delegated to the existing
SumeragiV2CrashReplayMutation model.  This bounded TLC mutation checks only
live-process effect ownership and scheduling.  It is evidence for these
finite transitions, not a deductive proof of protocol liveness.
***************************************************************************)

CONSTANTS BlockingOwner, RetainFullCapacityFetch, RetirementEnabled

DecidedBodyFetch == "DecidedBodyFetch"
NonFetchLocalWork == "NonFetchLocalWork"
StableProtectedWork == "StableProtectedWork"
NewMissingFetch == "NewMissingFetch"
TimeoutVoteSign == "TimeoutVoteSign"
Empty == "Empty"

ASSUME BlockingOwner \in {DecidedBodyFetch, NonFetchLocalWork}
ASSUME RetainFullCapacityFetch \in BOOLEAN
ASSUME RetirementEnabled \in BOOLEAN

VARIABLES phase,
          slotOne,
          slotTwo,
          retainedEffects,
          timeoutPersisted,
          newFetchAttempted,
          newFetchMissing,
          newFetchReconstructible,
          retiredOwner

vars ==
  <<phase,
    slotOne,
    slotTwo,
    retainedEffects,
    timeoutPersisted,
    newFetchAttempted,
    newFetchMissing,
    newFetchReconstructible,
    retiredOwner>>

EffectCapacity == 2
ReducerMaxEffectsPerStep == 8

WorkOwners ==
  {Empty,
   DecidedBodyFetch,
   NonFetchLocalWork,
   StableProtectedWork,
   TimeoutVoteSign}

RetainableEffects == {NewMissingFetch, TimeoutVoteSign}

Occupied(owner) == IF owner = Empty THEN 0 ELSE 1

PendingWorkCount == Occupied(slotOne) + Occupied(slotTwo)

TimeoutSignPending ==
  (slotOne = TimeoutVoteSign) \/ (slotTwo = TimeoutVoteSign)

TimeoutSignRetained == retainedEffects = <<TimeoutVoteSign>>

RetainedEffectSet ==
  {retainedEffects[index]: index \in 1..Len(retainedEffects)}

OwnershipDebt ==
  IF TimeoutSignPending
    THEN 0
    ELSE IF TimeoutSignRetained THEN 1 ELSE 2

CapacityDebt == IF TimeoutSignPending THEN 0 ELSE PendingWorkCount

CompletionDebt == <<OwnershipDebt, CapacityDebt>>

CompletionDebtDomain ==
  {<<ownership, capacity>>:
     ownership \in 0..2, capacity \in 0..EffectCapacity}

LexLess(left, right) ==
  \/ left[1] < right[1]
  \/ /\ left[1] = right[1]
     /\ left[2] < right[2]

TypeInvariant ==
  /\ phase \in 0..4
  /\ slotOne \in WorkOwners
  /\ slotTwo \in WorkOwners
  /\ retainedEffects \in Seq(RetainableEffects)
  /\ Len(retainedEffects) \in 0..1
  /\ timeoutPersisted \in BOOLEAN
  /\ newFetchAttempted \in BOOLEAN
  /\ newFetchMissing \in BOOLEAN
  /\ newFetchReconstructible \in BOOLEAN
  /\ retiredOwner \in {Empty, DecidedBodyFetch, NonFetchLocalWork}

PendingWorkWithinCapacity == PendingWorkCount <= EffectCapacity

RetainedEffectFifoIsBounded ==
  Len(retainedEffects) <= ReducerMaxEffectsPerStep

FullCapacityFetchRemainsMissingReconstructibleAndUnqueued ==
  newFetchAttempted =>
    /\ newFetchMissing
    /\ newFetchReconstructible
    /\ NewMissingFetch \notin RetainedEffectSet

PersistedSignIsSoleFifoHead ==
  (timeoutPersisted /\ ~TimeoutSignPending) => TimeoutSignRetained

RetirementIsExactTerminatingOwner ==
  (phase \in 3..4) => retiredOwner = BlockingOwner

CompletionDebtIsFinite == CompletionDebt \in CompletionDebtDomain

Init ==
  /\ phase = 0
  /\ slotOne = BlockingOwner
  /\ slotTwo = StableProtectedWork
  /\ retainedEffects = <<>>
  /\ timeoutPersisted = FALSE
  /\ newFetchAttempted = FALSE
  /\ newFetchMissing = FALSE
  /\ newFetchReconstructible = FALSE
  /\ retiredOwner = Empty

AttemptGenuinelyNewFetchAtFullCapacity ==
  /\ phase = 0
  /\ PendingWorkCount = EffectCapacity
  /\ newFetchAttempted' = TRUE
  /\ newFetchMissing' = TRUE
  /\ newFetchReconstructible' = TRUE
  /\ retainedEffects' =
       IF RetainFullCapacityFetch THEN <<NewMissingFetch>> ELSE <<>>
  /\ phase' = 1
  /\ UNCHANGED
       <<slotOne,
         slotTwo,
         timeoutPersisted,
         retiredOwner>>

PersistTimeoutVoteSign ==
  /\ phase = 1
  /\ retainedEffects = <<>>
  /\ PendingWorkCount = EffectCapacity
  /\ timeoutPersisted' = TRUE
  /\ retainedEffects' = <<TimeoutVoteSign>>
  /\ phase' = 2
  /\ UNCHANGED
       <<slotOne,
         slotTwo,
         newFetchAttempted,
         newFetchMissing,
         newFetchReconstructible,
         retiredOwner>>

FairlyRetireTerminatingOwner ==
  /\ phase = 2
  /\ RetirementEnabled
  /\ TimeoutSignRetained
  /\ slotOne = BlockingOwner
  /\ slotOne' = Empty
  /\ retiredOwner' = BlockingOwner
  /\ phase' = 3
  /\ UNCHANGED
       <<slotTwo,
         retainedEffects,
         timeoutPersisted,
         newFetchAttempted,
         newFetchMissing,
         newFetchReconstructible>>

AdmitRetainedTimeoutVoteSign ==
  /\ phase = 3
  /\ TimeoutSignRetained
  /\ PendingWorkCount = 1
  /\ slotOne = Empty
  /\ slotOne' = TimeoutVoteSign
  /\ retainedEffects' = Tail(retainedEffects)
  /\ phase' = 4
  /\ UNCHANGED
       <<slotTwo,
         timeoutPersisted,
         newFetchAttempted,
         newFetchMissing,
         newFetchReconstructible,
         retiredOwner>>

Next ==
  \/ AttemptGenuinelyNewFetchAtFullCapacity
  \/ PersistTimeoutVoteSign
  \/ FairlyRetireTerminatingOwner
  \/ AdmitRetainedTimeoutVoteSign

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(AttemptGenuinelyNewFetchAtFullCapacity)
  /\ WF_vars(PersistTimeoutVoteSign)
  /\ WF_vars(FairlyRetireTerminatingOwner)
  /\ WF_vars(AdmitRetainedTimeoutVoteSign)

TimeoutVoteSignEventuallyPending ==
  timeoutPersisted ~> TimeoutSignPending

CompletionDebtEventuallyDrops ==
  \A debt \in CompletionDebtDomain:
    (timeoutPersisted /\ ~TimeoutSignPending /\ CompletionDebt = debt)
      ~> (TimeoutSignPending \/ LexLess(CompletionDebt, debt))

ScenarioEventuallyCompletes == (phase = 0) ~> (phase = 4)

RepairedRetirementProgress ==
  /\ TimeoutVoteSignEventuallyPending
  /\ CompletionDebtEventuallyDrops
  /\ ScenarioEventuallyCompletes

=============================================================================
