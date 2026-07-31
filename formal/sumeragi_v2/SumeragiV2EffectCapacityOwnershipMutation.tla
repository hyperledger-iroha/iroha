---- MODULE SumeragiV2EffectCapacityOwnershipMutation ----
EXTENDS Naturals, Sequences

(***************************************************************************
Concrete Stage-6 effect-capacity mutation.

The serialized production prefix is represented explicitly:

  1. a Proposal for subject A emits FetchBody and consumes one work slot;
  2. a distinct PrepareQC for subject B emits FetchBody and consumes the
     second slot; and
  3. a TimeoutVote is durably persisted before its causal Sign effect reaches
     the executor.

EffectCapacity is the concrete bound two.  The old fail-stop behavior has no
owner for the already-persisted Sign when ensure_pending_slot rejects it.
The repaired abstraction retains the unconsumed causal suffix in the same
bounded FIFO shape used by production.  It drains that FIFO before asking the
runtime for more effects.  A durable Sign at full work capacity therefore
preempts the deterministic highest-priority non-decided Fetch: speculative,
then certified non-lock, then locked; a decided-body Fetch is not preempted.
Only when every owner is protected does the Sign wait for fair terminating
local work to retire.  The refill mutation keeps retiring real work fairly
but atomically replaces it with normal Fetch work before the retained FIFO is
drained.

Crash/restart authority is delegated to the existing
SumeragiV2CrashReplayMutation model.  This is a finite mutation/refinement
obligation for the live-process executor seam.  It neither supplies deductive
liveness closure nor asserts that the Rust repair exists without corresponding
source-linked implementation tests.
***************************************************************************)

CONSTANTS RetainPersistedSign, ProtectPersistedSignDebt

ASSUME RetainPersistedSign \in BOOLEAN
ASSUME ProtectPersistedSignDebt \in BOOLEAN

VARIABLES phase,
          slotOne,
          slotTwo,
          timeoutPersisted,
          retainedEffects,
          reconstructibleFetches,
          fatal,
          distinctFullCapacityWitness,
          refillParity

vars ==
  <<phase,
    slotOne,
    slotTwo,
    timeoutPersisted,
    retainedEffects,
    reconstructibleFetches,
    fatal,
    distinctFullCapacityWitness,
    refillParity>>

EffectCapacity == 2

ReducerMaxEffectsPerStep == 8

Empty == "Empty"
ProposalAFetch == "SpeculativeProposalAFetch"
PrepareQCBFetch == "CertifiedNonLockPrepareQCBFetch"
LockedPrepareQCFetch == "LockedPrepareQCFetch"
DecidedBodyFetch == "DecidedBodyFetch"
RefillFetchEven == "RefillFetchEven"
RefillFetchOdd == "RefillFetchOdd"
TimeoutVoteSign == "TimeoutVoteSign"

EffectOwners ==
  {Empty,
   ProposalAFetch,
   PrepareQCBFetch,
   LockedPrepareQCFetch,
   DecidedBodyFetch,
   RefillFetchEven,
   RefillFetchOdd,
   TimeoutVoteSign}

FetchOwners ==
  {ProposalAFetch,
   PrepareQCBFetch,
   LockedPrepareQCFetch,
   DecidedBodyFetch,
   RefillFetchEven,
   RefillFetchOdd}

IsFetch(owner) == owner \in FetchOwners

Occupied(owner) == IF owner = Empty THEN 0 ELSE 1

FetchOwned(owner) == IF IsFetch(owner) THEN 1 ELSE 0

PendingWorkCount == Occupied(slotOne) + Occupied(slotTwo)

PendingFetchCount == FetchOwned(slotOne) + FetchOwned(slotTwo)

TimeoutSignPending ==
  (slotOne = TimeoutVoteSign) \/ (slotTwo = TimeoutVoteSign)

TimeoutSignRetained ==
  retainedEffects = <<TimeoutVoteSign>>

TimeoutSignOwnerCount ==
  (IF TimeoutSignRetained THEN 1 ELSE 0)
    + (IF slotOne = TimeoutVoteSign THEN 1 ELSE 0)
    + (IF slotTwo = TimeoutVoteSign THEN 1 ELSE 0)

(***************************************************************************
The first component is durable-target ownership debt:

  2: the persisted target has no retained or pending owner;
  1: the bounded retained owner awaits executor admission;
  0: the Sign is pending in an executor work slot.

The second component is the number of Fetch owners obstructing admission and
is zero after the target owns a slot.  Thus preemption gives the strict finite
lexicographic descent <<1, 2>> -> <<0, 0>>.  If every Fetch is protected,
fair retirement instead exposes <<1, 2>> -> <<1, 1>> -> <<0, 0>>.
***************************************************************************)

OwnershipDebt ==
  IF TimeoutSignPending
    THEN 0
    ELSE IF TimeoutSignRetained THEN 1 ELSE 2

CapacityDebt == IF TimeoutSignPending THEN 0 ELSE PendingFetchCount

CompletionDebt == <<OwnershipDebt, CapacityDebt>>

CompletionDebtDomain ==
  {<<ownership, capacity>>:
     ownership \in 0..2, capacity \in 0..EffectCapacity}

LexLess(left, right) ==
  \/ left[1] < right[1]
  \/ /\ left[1] = right[1]
     /\ left[2] < right[2]

TypeInvariant ==
  /\ phase \in 0..3
  /\ slotOne \in EffectOwners
  /\ slotTwo \in EffectOwners
  /\ timeoutPersisted \in BOOLEAN
  /\ retainedEffects \in Seq({TimeoutVoteSign})
  /\ Len(retainedEffects) \in 0..1
  /\ reconstructibleFetches \in SUBSET FetchOwners
  /\ fatal \in BOOLEAN
  /\ distinctFullCapacityWitness \in BOOLEAN
  /\ refillParity \in BOOLEAN

PendingWorkWithinCapacity == PendingWorkCount <= EffectCapacity

CompletionDebtIsFinite == CompletionDebt \in CompletionDebtDomain

RetainedEffectFifoIsBounded ==
  Len(retainedEffects) <= ReducerMaxEffectsPerStep

UniqueTimeoutSignOwnership == TimeoutSignOwnerCount \in 0..1

PersistedTimeoutSignHasOwner ==
  timeoutPersisted => TimeoutSignOwnerCount = 1

PersistenceFollowedDistinctFullCapacity ==
  timeoutPersisted => distinctFullCapacityWitness

ConcretePreemptionKeepsProposalAReconstructible ==
  (timeoutPersisted /\ TimeoutSignPending) =>
    /\ ProposalAFetch \in reconstructibleFetches
    /\ slotTwo = PrepareQCBFetch

FatalIsLostPersistedSign ==
  fatal =>
    /\ timeoutPersisted
    /\ TimeoutSignOwnerCount = 0

Init ==
  /\ phase = 0
  /\ slotOne = Empty
  /\ slotTwo = Empty
  /\ timeoutPersisted = FALSE
  /\ retainedEffects = <<>>
  /\ reconstructibleFetches = {}
  /\ fatal = FALSE
  /\ distinctFullCapacityWitness = FALSE
  /\ refillParity = FALSE

AdmitProposalAFetch ==
  /\ phase = 0
  /\ slotOne = Empty
  /\ slotTwo = Empty
  /\ slotOne' = ProposalAFetch
  /\ phase' = 1
  /\ UNCHANGED
       <<slotTwo,
         timeoutPersisted,
         retainedEffects,
         reconstructibleFetches,
         fatal,
         distinctFullCapacityWitness,
         refillParity>>

AdmitDistinctPrepareQCBFetch ==
  /\ phase = 1
  /\ slotOne = ProposalAFetch
  /\ slotTwo = Empty
  /\ slotTwo' = PrepareQCBFetch
  /\ phase' = 2
  /\ distinctFullCapacityWitness' = TRUE
  /\ UNCHANGED
       <<slotOne,
         timeoutPersisted,
         retainedEffects,
         reconstructibleFetches,
         fatal,
         refillParity>>

PersistTimeoutVote ==
  /\ phase = 2
  /\ slotOne = ProposalAFetch
  /\ slotTwo = PrepareQCBFetch
  /\ PendingWorkCount = EffectCapacity
  /\ phase' = 3
  /\ timeoutPersisted' = TRUE
  /\ retainedEffects' =
       IF RetainPersistedSign THEN <<TimeoutVoteSign>> ELSE <<>>
  /\ fatal' = ~RetainPersistedSign
  /\ UNCHANGED
       <<slotOne,
         slotTwo,
         reconstructibleFetches,
         distinctFullCapacityWitness,
         refillParity>>

(***************************************************************************
Production preemption is deterministic and never chooses the decided body.
The numeric priority is used only to identify the victim; CompletionDebt is
the separate well-founded liveness rank.
***************************************************************************)
PreemptionPriority(owner) ==
  IF owner \in {ProposalAFetch, RefillFetchEven, RefillFetchOdd}
    THEN 3
    ELSE IF owner = PrepareQCBFetch
      THEN 2
      ELSE IF owner = LockedPrepareQCFetch THEN 1 ELSE 0

PreferredPreemptionPriority ==
  IF PreemptionPriority(slotOne) >= PreemptionPriority(slotTwo)
    THEN PreemptionPriority(slotOne)
    ELSE PreemptionPriority(slotTwo)

PreferredPreemptionSlot ==
  IF PreemptionPriority(slotOne) >= PreemptionPriority(slotTwo) THEN 1 ELSE 2

(***************************************************************************
The retained causal head drains before any new runtime effect.  At full
capacity its durable Sign replaces the highest-priority non-decided Fetch in
one serialized step.  In the concrete prefix this is Proposal A, ahead of the
certified non-lock PrepareQC B Fetch, and the rank drops directly from
<<1, 2>> to <<0, 0>>.
***************************************************************************)
DrainRetainedSignByPreemption ==
  /\ phase = 3
  /\ ~fatal
  /\ TimeoutSignRetained
  /\ ~TimeoutSignPending
  /\ ProtectPersistedSignDebt
  /\ PendingWorkCount = EffectCapacity
  /\ PreferredPreemptionPriority > 0
  /\ slotOne' =
       IF PreferredPreemptionSlot = 1 THEN TimeoutVoteSign ELSE slotOne
  /\ slotTwo' =
       IF PreferredPreemptionSlot = 2 THEN TimeoutVoteSign ELSE slotTwo
  /\ retainedEffects' = Tail(retainedEffects)
  /\ reconstructibleFetches' =
       reconstructibleFetches
         \cup {IF PreferredPreemptionSlot = 1 THEN slotOne ELSE slotTwo}
  /\ UNCHANGED
       <<phase,
         timeoutPersisted,
         fatal,
         distinctFullCapacityWitness,
         refillParity>>

(***************************************************************************
If every full-capacity owner is protected (in production, most importantly a
decided-body Fetch), no preemption is allowed.  Fair terminating local work
eventually retires one such owner; the retained FIFO still blocks runtime
refill and its head then takes the free slot.  This fallback is deliberately
unreachable in the A/B prefix, whose Proposal A owner is preemptible, but it
states the other branch of the source policy.
***************************************************************************)
FairlyRetireProtectedFetch ==
  /\ phase = 3
  /\ ~fatal
  /\ TimeoutSignRetained
  /\ ~TimeoutSignPending
  /\ ProtectPersistedSignDebt
  /\ PendingWorkCount = EffectCapacity
  /\ PreferredPreemptionPriority = 0
  /\ IsFetch(slotOne)
  /\ slotOne' = Empty
  /\ UNCHANGED
       <<phase,
         slotTwo,
         timeoutPersisted,
         retainedEffects,
         reconstructibleFetches,
         fatal,
         distinctFullCapacityWitness,
         refillParity>>

(***************************************************************************
This mutation still performs fair worker retirement.  It models a scheduler
which lets ordinary Fetch work refill the freed slot atomically before the
retained causal Sign is admitted.  refillParity makes the starvation cycle
visible in the finite graph while occupancy and CompletionDebt stay fixed.
***************************************************************************)
RetireAndRefillAheadOfPersistedSign ==
  /\ phase = 3
  /\ ~fatal
  /\ TimeoutSignRetained
  /\ ~TimeoutSignPending
  /\ ~ProtectPersistedSignDebt
  /\ PendingWorkCount = EffectCapacity
  /\ IsFetch(slotOne)
  /\ slotOne' =
       IF refillParity THEN RefillFetchEven ELSE RefillFetchOdd
  /\ refillParity' = ~refillParity
  /\ UNCHANGED
       <<phase,
         slotTwo,
         timeoutPersisted,
         retainedEffects,
         reconstructibleFetches,
         fatal,
         distinctFullCapacityWitness>>

AdmitRetainedTimeoutSign ==
  /\ phase = 3
  /\ ~fatal
  /\ TimeoutSignRetained
  /\ ~TimeoutSignPending
  /\ PendingWorkCount < EffectCapacity
  /\ ((slotOne = Empty) \/ (slotTwo = Empty))
  /\ slotOne' =
       IF slotOne = Empty THEN TimeoutVoteSign ELSE slotOne
  /\ slotTwo' =
       IF slotOne = Empty THEN slotTwo ELSE TimeoutVoteSign
  /\ retainedEffects' = Tail(retainedEffects)
  /\ UNCHANGED
       <<phase,
         timeoutPersisted,
         reconstructibleFetches,
         fatal,
         distinctFullCapacityWitness,
         refillParity>>

Next ==
  \/ AdmitProposalAFetch
  \/ AdmitDistinctPrepareQCBFetch
  \/ PersistTimeoutVote
  \/ DrainRetainedSignByPreemption
  \/ FairlyRetireProtectedFetch
  \/ RetireAndRefillAheadOfPersistedSign
  \/ AdmitRetainedTimeoutSign

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(AdmitProposalAFetch)
  /\ WF_vars(AdmitDistinctPrepareQCBFetch)
  /\ WF_vars(PersistTimeoutVote)
  /\ WF_vars(DrainRetainedSignByPreemption)
  /\ WF_vars(FairlyRetireProtectedFetch)
  /\ WF_vars(RetireAndRefillAheadOfPersistedSign)
  /\ WF_vars(AdmitRetainedTimeoutSign)

TimeoutVoteSignEventuallyPending ==
  timeoutPersisted ~> TimeoutSignPending

CompletionDebtEventuallyDrops ==
  \A debt \in CompletionDebtDomain:
    (timeoutPersisted /\ ~TimeoutSignPending /\ CompletionDebt = debt)
      ~> (TimeoutSignPending \/ LexLess(CompletionDebt, debt))

RepairedCompletionProgress ==
  /\ TimeoutVoteSignEventuallyPending
  /\ CompletionDebtEventuallyDrops

=============================================================================
