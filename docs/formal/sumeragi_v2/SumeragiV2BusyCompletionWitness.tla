---- MODULE SumeragiV2BusyCompletionWitness ----
EXTENDS SumeragiV2LivenessProofs

(***************************************************************************
Authoritative regression for executor-work/runtime-completion capacity
separation.  The schedule below restricts only the order of exact
SumeragiV2AsyncNetwork actions; it introduces no surrogate state transition.

Validator zero's initial AssembleBody successor, BeginProposal, is admitted
to Consensus I/O and deliberately held while the pre-GST clock reaches the
absolute timeout.  DirectTimeoutStep then creates pendingTimeout and its
causal PersistTimeout successor.  Once the held BeginProposal reaches the
runtime queue it is no longer dispatchable because BeginLocalProposal requires
NodeIdle, so the Busy branch moves it into the deferred Completion lane.

The retired model counted that deferred command against AsyncIoWorkCapacity;
PersistTimeout could never enter I/O and the schedule ended in a fair Busy
lasso.  Production releases executor work ownership when a completion enters
the serialized runtime.  The repaired model therefore admits PersistTimeout
while BeginProposal remains runtime/deferred-owned and cannot reach the old
trap.
***************************************************************************)

VARIABLE pc

WitnessVars == <<AsyncAllVars, pc>>

WitnessInit == AsyncFiniteInit /\ pc = 0

WitnessNext ==
  \/ /\ pc = 0 /\ RunNode(0) /\ pc' = 1
  \/ /\ pc = 1 /\ RunNode(0) /\ pc' = 2
  \/ /\ pc = 2 /\ RunNode(0) /\ pc' = 3
  \/ /\ pc = 3 /\ RunNode(0) /\ pc' = 4
  \/ /\ pc \in 4..8 /\ AsyncTick /\ pc' = pc + 1
  \/ /\ pc = 9 /\ RunNode(0) /\ pc' = 10
  \/ /\ pc = 10 /\ RunNode(0) /\ pc' = 11
  \/ /\ pc = 11 /\ RunNode(0) /\ pc' = 12
  \/ /\ pc = 12 /\ RunNode(0) /\ pc' = 13
  \/ /\ pc = 13 /\ RunNode(0) /\ pc' = 14
  \/ /\ pc = 14 /\ RunNode(0) /\ pc' = 15
  \/ /\ pc = 15 /\ ServiceIoWorker(0) /\ pc' = 16
  \/ /\ pc = 16 /\ RunNode(0) /\ pc' = 17
  \/ /\ pc = 17 /\ RunNode(0) /\ pc' = 18
  \/ /\ pc = 18 /\ RunNode(0) /\ pc' = 19
  \/ /\ pc = 19 /\ RunNode(0) /\ pc' = 20
  \/ /\ pc = 20 /\ RunNode(0) /\ pc' = 21
  \/ /\ pc = 21 /\ RunNode(0) /\ pc' = 22
  \/ /\ pc = 22 /\ RunNode(0) /\ pc' = 23
  \/ /\ pc = 23 /\ RunNode(0) /\ pc' = 24
  \/ /\ pc = 24 /\ RunNode(0) /\ pc' = 25
  \/ /\ pc = 25 /\ RunNode(0) /\ pc' = 26
  \/ /\ pc = 26 /\ RunNode(0) /\ pc' = 27

WitnessSpec == WitnessInit /\ [][WitnessNext]_WitnessVars

BlockedPersistTimeout ==
  /\ CausalQueueNonempty(0)
  /\ HeadCausalCandidate(0).class = "Completion"
  /\ HeadCausalCandidate(0).kind = "PersistTimeout"
  /\ ~CausalHeadCanAdvance(0)

DeferredBeginProposal ==
  /\ Len(asyncDeferredCompletionQueues[0]) > 0
  /\ Head(asyncDeferredCompletionQueues[0]).class = "Completion"
  /\ Head(asyncDeferredCompletionQueues[0]).kind = "BeginProposal"
  /\ ~CommandDispatchable(Head(asyncDeferredCompletionQueues[0]))

BusyCompletionCapacityTrapState ==
  /\ ~NodeIdle(0)
  /\ AsyncCompletionLoad(0) = AsyncIoWorkCapacity
  /\ DeferredBeginProposal
  /\ BlockedPersistTimeout
  /\ ~asyncDeferredDrainOwed[0]
  /\ ~NodeQueueNonempty(0)
  /\ AsyncIoQueueDepth(0) = 0
  /\ asyncIoReadyCompletions[0] = <<>>
  /\ asyncLocalReadyCompletions[0] = <<>>

BusyCompletionCapacityTrap ==
  /\ pc = 27
  /\ BusyCompletionCapacityTrapState

NoBusyCompletionCapacityTrap == ~BusyCompletionCapacityTrap

WitnessPersistTimeout ==
  NoItemCandidate("Completion", "PersistTimeout", 0, 0, NoSubject)

(***************************************************************************
Pin the positive end state as well as excluding the old trap.  One executor
work owner and one separately queued runtime completion coexist; the former is
the exact PersistTimeout I/O job, so post-GST I/O fairness can advance it.
***************************************************************************)
WitnessEndSeparatesWorkFromRuntimeOwnership ==
  pc # 27
    \/ /\ AsyncOutstandingWorkCount(0) = 1
       /\ AsyncCompletionLoad(0) = 2
       /\ CandidateInIoQueue(WitnessPersistTimeout)
       /\ AsyncOutstandingWorkCount(0) <= AsyncIoWorkCapacity

=============================================================================
