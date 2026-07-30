---- MODULE SumeragiV2EffectPreemptionPriorityMutation ----
EXTENDS Naturals, Sequences

(***************************************************************************
Finite mutation for deterministic durable-Sign Fetch preemption.

Six queue permutations contain two speculative Fetches with different work
ids, one certified non-lock Fetch, one locked Fetch, and one decided Fetch.
Correct production selection is the minimum (class, work_id) among
non-decided owners, independent of map/queue presentation order.  Four Signs
therefore cancel speculative-old, speculative-new, certified, and locked in
that exact order.  The fifth Sign remains retained behind the decided owner.

Crash/restart reconstruction is outside this compact selection model and is
delegated to SumeragiV2CrashReplayMutation.  Exhaustive TLC over these finite
permutations is mutation evidence, not deductive liveness closure.
***************************************************************************)

CONSTANT SelectionPolicy

CorrectPolicy == "Correct"
WrongClassPolicy == "WrongClass"
WrongWorkIdPolicy == "WrongWorkId"
DecidedVictimPolicy == "DecidedVictim"

ASSUME SelectionPolicy \in
  {CorrectPolicy, WrongClassPolicy, WrongWorkIdPolicy, DecidedVictimPolicy}

SpeculativeOld ==
  [name |-> "SpeculativeOld", tier |-> 0, workId |-> 1, decided |-> FALSE]

SpeculativeNew ==
  [name |-> "SpeculativeNew", tier |-> 0, workId |-> 4, decided |-> FALSE]

CertifiedNonLock ==
  [name |-> "CertifiedNonLock", tier |-> 1, workId |-> 2, decided |-> FALSE]

LockedFetch ==
  [name |-> "LockedFetch", tier |-> 2, workId |-> 3, decided |-> FALSE]

DecidedFetch ==
  [name |-> "DecidedFetch", tier |-> 0, workId |-> 0, decided |-> TRUE]

AllFetches ==
  {SpeculativeOld,
   SpeculativeNew,
   CertifiedNonLock,
   LockedFetch,
   DecidedFetch}

PendingOrders ==
  {<<SpeculativeOld,
     SpeculativeNew,
     CertifiedNonLock,
     LockedFetch,
     DecidedFetch>>,
   <<DecidedFetch,
     LockedFetch,
     CertifiedNonLock,
     SpeculativeNew,
     SpeculativeOld>>,
   <<LockedFetch,
     SpeculativeNew,
     DecidedFetch,
     SpeculativeOld,
     CertifiedNonLock>>,
   <<CertifiedNonLock,
     SpeculativeOld,
     LockedFetch,
     DecidedFetch,
     SpeculativeNew>>,
   <<SpeculativeNew,
     DecidedFetch,
     CertifiedNonLock,
     SpeculativeOld,
     LockedFetch>>,
   <<LockedFetch,
     DecidedFetch,
     SpeculativeOld,
     SpeculativeNew,
     CertifiedNonLock>>}

ExpectedCancellationNames ==
  <<"SpeculativeOld", "SpeculativeNew", "CertifiedNonLock", "LockedFetch">>

VARIABLES originOrder,
          pendingFetches,
          cancelledNames,
          retainedSign

vars == <<originOrder, pendingFetches, cancelledNames, retainedSign>>

PendingSet ==
  {pendingFetches[index]: index \in 1..Len(pendingFetches)}

EligibleSet == {fetch \in PendingSet: ~fetch.decided}

FetchRank(fetch) == <<fetch.tier, fetch.workId>>

LexLess(left, right) ==
  \/ left[1] < right[1]
  \/ /\ left[1] = right[1]
     /\ left[2] < right[2]

CorrectVictim ==
  CHOOSE candidate \in EligibleSet:
    \A other \in EligibleSet: ~LexLess(FetchRank(other), FetchRank(candidate))

WrongClassVictim ==
  CHOOSE candidate \in EligibleSet:
    \A other \in EligibleSet: ~LexLess(FetchRank(candidate), FetchRank(other))

WrongWorkIdVictim ==
  IF SpeculativeNew \in EligibleSet THEN SpeculativeNew ELSE CorrectVictim

DecidedVictim ==
  IF DecidedFetch \in PendingSet THEN DecidedFetch ELSE CorrectVictim

SelectedVictim ==
  CASE SelectionPolicy = CorrectPolicy -> CorrectVictim
    [] SelectionPolicy = WrongClassPolicy -> WrongClassVictim
    [] SelectionPolicy = WrongWorkIdPolicy -> WrongWorkIdVictim
    [] SelectionPolicy = DecidedVictimPolicy -> DecidedVictim

SelectedVictimIndex ==
  CHOOSE index \in 1..Len(pendingFetches):
    pendingFetches[index] = SelectedVictim

RemoveSelectedVictim ==
  SubSeq(pendingFetches, 1, SelectedVictimIndex - 1)
    \o SubSeq(pendingFetches, SelectedVictimIndex + 1, Len(pendingFetches))

TypeInvariant ==
  /\ originOrder \in PendingOrders
  /\ pendingFetches \in Seq(AllFetches)
  /\ Len(pendingFetches) \in 1..5
  /\ cancelledNames \in Seq({fetch.name: fetch \in AllFetches})
  /\ Len(cancelledNames) \in 0..5
  /\ retainedSign \in BOOLEAN

CancellationPrefixMatchesClassAndWorkId ==
  /\ Len(cancelledNames) <= Len(ExpectedCancellationNames)
  /\ \A index \in 1..Len(cancelledNames):
       cancelledNames[index] = ExpectedCancellationNames[index]

DecidedFetchNeverPreempted ==
  "DecidedFetch" \notin
    {cancelledNames[index]: index \in 1..Len(cancelledNames)}

RetainedSignPreservesDecidedFetch ==
  retainedSign =>
    /\ pendingFetches = <<DecidedFetch>>
    /\ cancelledNames = ExpectedCancellationNames

Init ==
  /\ originOrder \in PendingOrders
  /\ pendingFetches = originOrder
  /\ cancelledNames = <<>>
  /\ retainedSign = FALSE

PreemptForDurableSign ==
  /\ ~retainedSign
  /\ EligibleSet # {}
  /\ pendingFetches' = RemoveSelectedVictim
  /\ cancelledNames' = Append(cancelledNames, SelectedVictim.name)
  /\ UNCHANGED <<originOrder, retainedSign>>

RetainSignBehindDecidedFetch ==
  /\ ~retainedSign
  /\ EligibleSet = {}
  /\ pendingFetches = <<DecidedFetch>>
  /\ retainedSign' = TRUE
  /\ UNCHANGED <<originOrder, pendingFetches, cancelledNames>>

Next == PreemptForDurableSign \/ RetainSignBehindDecidedFetch

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(PreemptForDurableSign)
  /\ WF_vars(RetainSignBehindDecidedFetch)

AllPermutationsEventuallyRetainBehindDecision == TRUE ~> retainedSign

=============================================================================
