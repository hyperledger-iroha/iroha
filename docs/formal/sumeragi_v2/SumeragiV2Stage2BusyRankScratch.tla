---- MODULE SumeragiV2Stage2BusyRankScratch ----
EXTENDS SumeragiV2AsyncLivenessProofs

VARIABLE stage2DeferredHandoffs

(***************************************************************************
Scratch decomposition for stage-2 deferred service.

This module is deliberately outside the release proof.  None of the
obligations below is release evidence until the exact source span has passed
the pinned strict TLAPS corridor with a fresh cache.  More importantly, the
current asynchronous state has no durable deferred-handoff token, so the
final stage-2 theorem must not be migrated merely by proving the finite Busy
phase kernel.

The decomposition keeps two independent ranks:

  1. `BusyPhaseRank` measures the one serialized Core owner at a validator.
     WAL work which must still sign has rank 2; WAL work which terminates on
     persistence and signature work have rank 1; an idle reducer has rank 0.

  2. `DeferredCandidatePosition` measures the three-class deferred cursor.
     A Busy retry may legitimately raise that position by two when the
     selected class advances.  Consequently there is intentionally no
     global UNLESS theorem claiming that the stage-2 service rank never
     rises.  `SumeragiV2DeferredCursorEffectMutation` pins that boundary.

The missing bridge is an exact-identity handoff: once Busy rejects a selected
deferred candidate, that candidate's owner/token must survive until the node
is idle and must be retried before a foreign deferred command can create a
fresh Busy owner.  The token relation at the bottom is stated separately so
production source-fidelity can check it rather than hiding the requirement in
a fairness slogan.
***************************************************************************)

Stage2TwoStepBusyOwners ==
  pendingProposal \cup pendingPrepare \cup pendingLockCommit
    \cup pendingTimeout \cup pendingInstallTC

Stage2OneStepBusyOwners ==
  pendingObservePrepare \cup pendingDecision
    \cup signProposals \cup signVotes \cup signTimeouts

Stage2TwoStepBusyNodes == RequestNodeSet(Stage2TwoStepBusyOwners)

Stage2OneStepBusyNodes == RequestNodeSet(Stage2OneStepBusyOwners)

BusyPhaseCarrier == 0..2

BusyPhaseRank(node) ==
  IF node \in Stage2TwoStepBusyNodes
  THEN 2
  ELSE IF node \in Stage2OneStepBusyNodes THEN 1 ELSE 0

Stage2TwoStepCompletionKinds ==
  {"PersistProposal", "PersistPrepare", "PersistLockCommit",
   "PersistTimeout", "PersistInstallTC"}

Stage2OneStepCompletionKinds ==
  {"PersistObservePrepare", "PersistDecision",
   "SignProposal", "SignVote", "SignTimeout"}

(***************************************************************************
Exact serialized-owner partition.

`SerializedBusyOwnershipInvariant` is essential here.  Without it, a
rank-two owner and an unrelated rank-one owner could coexist at one node and
executing either completion would not necessarily lower the node rank.
***************************************************************************)

THEOREM BusyPhaseOwnerPartitionObligation ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    => /\ SerializedBusyOwners =
             Stage2TwoStepBusyOwners \cup Stage2OneStepBusyOwners
       /\ BusyPhaseRank(node) \in BusyPhaseCarrier
       /\ (BusyPhaseRank(node) = 0 <=> NodeIdle(node))
       /\ (BusyPhaseRank(node) = 2
             <=> node \in Stage2TwoStepBusyNodes)
       /\ (BusyPhaseRank(node) = 1
             <=> node \in Stage2OneStepBusyNodes)

THEOREM BusyCompletionKindMatchesPhaseObligation ==
  \A node \in ValidatorIds, witness:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ witness \in BusyCompletionCandidates(node)
    => /\ (BusyPhaseRank(node) = 2
             => witness.kind \in Stage2TwoStepCompletionKinds)
       /\ (BusyPhaseRank(node) = 1
             => witness.kind \in Stage2OneStepCompletionKinds)

(***************************************************************************
Concrete Core transition kernel.

The rank-two cases are exactly:

  PersistProposal       pendingProposal   -> signProposals
  PersistPrepare        pendingPrepare    -> signVotes
  PersistLockCommit     pendingLockCommit -> signVotes
  PersistTimeout        pendingTimeout    -> signTimeouts
  PersistInstallTC      pendingInstallTC  -> signVotes or idle

The rank-one cases remove pendingObservePrepare, pendingDecision, or the
matching signature request and install no new Busy owner.  Thus execution of
the authenticated matching completion strictly changes 2 -> {1, 0} or
1 -> 0.  This is the local-work termination fact required by stage 2; merely
removing the scheduler occurrence is not a substitute for it.
***************************************************************************)

BusyCompletionExecution(node, witness) ==
  /\ node \in ValidatorIds
  /\ BusyPhaseRank(node) \in 1..2
  /\ witness \in BusyCompletionCandidates(node)
  /\ ExecuteCommand(witness)

THEOREM BusyCompletionExecutionDropsPhaseObligation ==
  \A node, witness:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ BusyCompletionExecution(node, witness)
    => /\ BusyPhaseRank(node)' \in 0..1
       /\ BusyPhaseRank(node)' < BusyPhaseRank(node)
       /\ (BusyPhaseRank(node) = 1 => BusyPhaseRank(node)' = 0)
       /\ (BusyPhaseRank(node) = 2
             => BusyPhaseRank(node)' \in 0..1)

(***************************************************************************
A matching Busy completion is executable while its node is Busy because the
Completion class is the sole `CommandDispatchable` exception to NodeIdle.
This leaf must use the exact pending/signature request guards; proving only
that some Completion command is enabled would not connect the scheduler
owner to the Core rank above.
***************************************************************************)

THEOREM BusyCompletionCandidateDispatchableObligation ==
  \A node \in ValidatorIds, witness:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ BusyPhaseRank(node) \in 1..2
    /\ witness \in BusyCompletionCandidates(node)
    => CommandDispatchable(witness)

(***************************************************************************
Restricted post-deferred convergence.

Stage 2 cannot assume the full protected-rank theorem which it is needed to
prove.  Busy completion witnesses live only in the active carrier, so their
temporal service composes exactly stages 3 through 6.  Stage-4 and Stage-5
are already proved; Stage-3 and Stage-6 must be supplied by their independent
strict slices before this restricted property is available.
***************************************************************************)

PostDeferredServiceRankCarrier == (3..6) \X Nat

PostDeferredServiceRankOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat), OpToRel(<, Nat), 3..6, Nat)

ProtectedPostDeferredRankProgressProperty(specification) ==
  specification
    => \A candidate \in AsyncCandidateSet,
          stage \in 3..6, position \in Nat:
         (gst
           /\ ResponsiveProtectedCandidateOwned(candidate)
           /\ CandidateServiceRank(candidate) = <<stage, position>>)
           ~> (~ResponsiveProtectedCandidateOwned(candidate)
                \/ ServiceRankLess(CandidateServiceRank(candidate),
                     <<stage, position>>))

ProtectedStage2RankProgressProperty(specification) ==
  specification
    => \A candidate \in AsyncCandidateSet, position \in Nat:
         (gst
           /\ ResponsiveProtectedCandidateOwned(candidate)
           /\ CandidateServiceRank(candidate) = <<2, position>>)
           ~> (~ResponsiveProtectedCandidateOwned(candidate)
                \/ ServiceRankLess(CandidateServiceRank(candidate),
                     <<2, position>>))

ProtectedStage3RankProgressProperty(specification) ==
  specification
    => \A candidate \in AsyncCandidateSet, position \in Nat:
         (gst
           /\ ResponsiveProtectedCandidateOwned(candidate)
           /\ CandidateServiceRank(candidate) = <<3, position>>)
           ~> (~ResponsiveProtectedCandidateOwned(candidate)
                \/ ServiceRankLess(CandidateServiceRank(candidate),
                     <<3, position>>))

ProtectedStage6RankProgressProperty(specification) ==
  specification
    => \A candidate \in AsyncCandidateSet, position \in Nat:
         (gst
           /\ ResponsiveProtectedCandidateOwned(candidate)
           /\ CandidateServiceRank(candidate) = <<6, position>>)
           ~> (~ResponsiveProtectedCandidateOwned(candidate)
                \/ ServiceRankLess(CandidateServiceRank(candidate),
                     <<6, position>>))

THEOREM ProtectedPostDeferredRanksComposeFromLeavesObligation ==
  \A initialContext:
    /\ ProtectedStage3RankProgressProperty(
         AsyncSpecAt(initialContext))
    /\ ProtectedStage4RankProgressProperty(
         AsyncSpecAt(initialContext))
    /\ ProtectedStage5RankProgressProperty(
         AsyncSpecAt(initialContext))
    /\ ProtectedStage6RankProgressProperty(
         AsyncSpecAt(initialContext))
    => ProtectedPostDeferredRankProgressProperty(
         AsyncSpecAt(initialContext))

ProtectedPostDeferredExit(candidate) ==
  \/ ~ResponsiveProtectedCandidateOwned(candidate)
  \/ CandidateServiceRank(candidate)[1] \notin 3..6

THEOREM PostDeferredServiceRankOrderingWellFoundedObligation ==
  IsWellFoundedOn(
    PostDeferredServiceRankOrdering, PostDeferredServiceRankCarrier)

THEOREM PostDeferredRankProgressConvergesObligation ==
  \A initialContext, candidate:
    /\ AsyncSpecAt(initialContext)
    /\ ProtectedPostDeferredRankProgressProperty(
         AsyncSpecAt(initialContext))
    => (gst
          /\ ResponsiveProtectedCandidateOwned(candidate)
          /\ CandidateServiceRank(candidate)[1] \in 3..6)
         ~> ProtectedPostDeferredExit(candidate)

(***************************************************************************
Fixed-witness bridge.

While the target is still protected and the Core phase has not dropped, the
same exact Busy completion may move 6 -> 5 -> 4 -> 3 but may neither enter the
Busy-deferred stage nor disappear.  An execution/removal changes the matching
pending/signature owner and therefore lowers `BusyPhaseRank`; applying the
height ends protection for both witness and target.  This is the non-vacuous
connection from post-deferred scheduler progress to terminating local work.
***************************************************************************)

ProtectedStage2Owned(candidate) ==
  /\ gst
  /\ ResponsiveProtectedCandidateOwned(candidate)
  /\ candidate \in DeferredCandidates

ProtectedStage2Pending(candidate, position) ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncProgressOwnershipInvariant
  /\ ProtectedOwnedAtServiceRank(candidate, <<2, position>>)

ProtectedBusyCompletionWitness(target, witness) ==
  /\ ProtectedStage2Owned(target)
  /\ BusyPhaseRank(target.node) \in 1..2
  /\ witness \in BusyCompletionCandidates(target.node)

THEOREM ProtectedBusyWitnessHasPostDeferredRankObligation ==
  \A target, witness:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ ProtectedBusyCompletionWitness(target, witness)
    => /\ ResponsiveProtectedCandidateOwned(witness)
       /\ CandidateServiceRank(witness)
            \in PostDeferredServiceRankCarrier

THEOREM BusyWitnessPersistsUntilTargetExitOrPhaseDropObligation ==
  \A target, witness:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ ProtectedBusyCompletionWitness(target, witness)
    /\ [AsyncNext]_AsyncAllVars
    /\ ~ProtectedServiceOwnershipExit(target)'
    /\ BusyPhaseRank(target.node)'
         >= BusyPhaseRank(target.node)
    => /\ ResponsiveProtectedCandidateOwned(witness)'
       /\ CandidateServiceRank(witness)'[1] \in 3..6

THEOREM ProtectedStage2BusyPhaseDescentObligation ==
  \A initialContext, target, phase \in 1..2:
    /\ AsyncSpecAt(initialContext)
    /\ ProtectedPostDeferredRankProgressProperty(
         AsyncSpecAt(initialContext))
    => (ProtectedStage2Owned(target)
          /\ BusyPhaseRank(target.node) = phase)
         ~> (ProtectedServiceOwnershipExit(target)
              \/ BusyPhaseRank(target.node) < phase)

THEOREM ProtectedStage2BusyTerminatesLocallyObligation ==
  \A initialContext, target:
    /\ AsyncSpecAt(initialContext)
    /\ ProtectedPostDeferredRankProgressProperty(
         AsyncSpecAt(initialContext))
    => (ProtectedStage2Owned(target) /\ ~NodeIdle(target.node))
         ~> (ProtectedServiceOwnershipExit(target)
              \/ NodeIdle(target.node))

(***************************************************************************
Concrete equal-rank rebusy boundary.

The following three ranks are the exact cursor arithmetic for the smallest
problematic cycle with a Normal target and a Progress BeginObservePrepare
blocker:

  idle before blocker:       3 * prefix + 2, Busy 0
  blocker starts Busy:       3 * prefix + 0, Busy 1
  target is retried Busy:    3 * prefix + 2, Busy 1
  PersistObserve completes:  3 * prefix + 2, Busy 0

The final state has the initial target rank and cursor.  A later authenticated
higher-view Observe command can occupy the Progress head and repeat the same
cycle.  Weak fairness of RunNode and termination of the current Busy owner do
not by themselves exclude it.
***************************************************************************)

Stage2EqualRankRebusyRanks(prefix) ==
  <<3 * prefix + 2, 3 * prefix, 3 * prefix + 2, 3 * prefix + 2>>

Stage2ObserveRebusyBoundary(target, blocker) ==
  /\ ProtectedStage2Owned(target)
  /\ NodeIdle(target.node)
  /\ asyncDeferredDrainOwed[target.node]
  /\ target.class = "Normal"
  /\ blocker = NextDeferredCommand(target.node)
  /\ blocker.class = "Progress"
  /\ blocker.kind = "BeginObservePrepare"
  /\ CommandClassDistance(
       asyncNextDeferredClass[target.node], target.class) = 2

(***************************************************************************
Minimum exact-identity handoff premise.

The token contains the authenticated reducer owner plus the full immutable
candidate identity.  It is not a connection generation, cursor position, or
fresh replacement candidate.  Production can check this relation at the
Busy Hold/Release seam: Busy records this token; Release retains it across
the 2 -> 1 -> 0 completion chain; the next idle deferred dispatch consumes
the exact token before another command may install a Busy owner.

The current TLA state has no variable which stores this token.  The property
below is therefore an explicit missing refinement premise, not a theorem of
`AsyncSpecAt`.  A model repair may add a per-node token/held-item owner or an
equivalent selector rule, but it must preserve the exact identity and must
not reset `DeferredCandidatePosition` on duplicate/reconnect events.
***************************************************************************)

Stage2DeferredHandoffToken(candidate) ==
  [owner |-> candidate.node,
   identity |-> ExactAsyncCandidateIdentity(candidate)]

Stage2NoDeferredHandoff == [active |-> FALSE]

Stage2ActiveDeferredHandoff(candidate) ==
  [active |-> TRUE,
   owner |-> candidate.node,
   identity |-> ExactAsyncCandidateIdentity(candidate)]

Stage2DeferredHandoffValues ==
  {Stage2NoDeferredHandoff}
    \cup {Stage2ActiveDeferredHandoff(candidate):
            candidate \in AsyncCandidateSet}

Stage2DeferredHandoffTypeInvariant ==
  stage2DeferredHandoffs
    \in [ValidatorIds -> Stage2DeferredHandoffValues]

Stage2DeferredHandoffInit ==
  stage2DeferredHandoffs =
    [node \in ValidatorIds |-> Stage2NoDeferredHandoff]

Stage2DeferredHandoffOwned(candidate) ==
  /\ candidate.node \in ValidatorIds
  /\ stage2DeferredHandoffs[candidate.node]
       = Stage2ActiveDeferredHandoff(candidate)
  /\ candidate \in DeferredCandidates

THEOREM Stage2DeferredHandoffTokenIsInjectiveObligation ==
  \A left, right \in AsyncCandidateSet:
    Stage2DeferredHandoffToken(left)
      = Stage2DeferredHandoffToken(right)
      => left = right

Stage2BusyRejectedSelected(candidate) ==
  /\ ProtectedStage2Owned(candidate)
  /\ ~NodeIdle(candidate.node)
  /\ asyncDeferredDrainOwed[candidate.node]
  /\ DeferredQueueNonempty(candidate.node)
  /\ NextDeferredCommand(candidate.node) = candidate
  /\ ~CommandDispatchable(candidate)

Stage2BusyRetryClaimsHandoff(candidate) ==
  /\ Stage2BusyRejectedSelected(candidate)
  /\ DeferredDrainStep(candidate.node)
  /\ stage2DeferredHandoffs'[candidate.node]
       = Stage2ActiveDeferredHandoff(candidate)
  /\ \A other \in ValidatorIds \ {candidate.node}:
       stage2DeferredHandoffs'[other] = stage2DeferredHandoffs[other]

Stage2ExactIdleRetryReady(candidate) ==
  /\ Stage2DeferredHandoffOwned(candidate)
  /\ ProtectedStage2Owned(candidate)
  /\ NodeIdle(candidate.node)
  /\ asyncDeferredDrainOwed[candidate.node]
  /\ DeferredQueueNonempty(candidate.node)
  /\ Stage2DeferredHandoffToken(NextDeferredCommand(candidate.node))
       = Stage2DeferredHandoffToken(candidate)

Stage2ExactHandoffConsumed(candidate) ==
  /\ Stage2ExactIdleRetryReady(candidate)
  /\ DeferredDrainStep(candidate.node)
  /\ ~ResponsiveProtectedCandidateOwned(candidate)'

Stage2HandoffRetentionAction(candidate) ==
  /\ Stage2DeferredHandoffOwned(candidate)
  /\ [AsyncNext]_AsyncAllVars
  /\ ResponsiveProtectedCandidateOwned(candidate)'
  => stage2DeferredHandoffs'[candidate.node]
       = stage2DeferredHandoffs[candidate.node]

Stage2HandoffClearOnlyOnExitAction(candidate) ==
  /\ Stage2DeferredHandoffOwned(candidate)
  /\ [AsyncNext]_AsyncAllVars
  /\ stage2DeferredHandoffs'[candidate.node]
       # stage2DeferredHandoffs[candidate.node]
  => ~ResponsiveProtectedCandidateOwned(candidate)'

Stage2HandoffExitClearsAction(candidate) ==
  /\ Stage2DeferredHandoffOwned(candidate)
  /\ [AsyncNext]_AsyncAllVars
  /\ ~ResponsiveProtectedCandidateOwned(candidate)'
  => stage2DeferredHandoffs'[candidate.node]
       = Stage2NoDeferredHandoff

Stage2HandoffCreationOnlyOnBusyRetryAction(node) ==
  /\ node \in ValidatorIds
  /\ [AsyncNext]_AsyncAllVars
  /\ stage2DeferredHandoffs[node] = Stage2NoDeferredHandoff
  /\ stage2DeferredHandoffs'[node] # Stage2NoDeferredHandoff
  => \E candidate \in AsyncCandidateSet:
       /\ candidate.node = node
       /\ Stage2BusyRejectedSelected(candidate)
       /\ DeferredDrainStep(node)
       /\ stage2DeferredHandoffs'[node]
            = Stage2ActiveDeferredHandoff(candidate)

(***************************************************************************
Safety half: while the exact handoff is outstanding, an idle -> Busy edge is
legal only if that same candidate is removed by the edge.  This rejects the
foreign Progress/Normal blocker which closes the equal-rank cycle above.
***************************************************************************)

Stage2NoForeignRebusyAction(candidate) ==
  /\ Stage2DeferredHandoffOwned(candidate)
  /\ NodeIdle(candidate.node)
  /\ [AsyncNext]_AsyncAllVars
  /\ ~NodeIdle(candidate.node)'
  => ~ResponsiveProtectedCandidateOwned(candidate)'

(***************************************************************************
Token-realization contract.

Acquisition is allowed only on the concrete Busy deferred-drain edge;
retention is exact while the candidate remains protected; exit clears the
owner; and an idle owner forces the exact token to be the next drain item.
Together with the independently proved Busy-phase termination and fair
RunNode cycle, these safety facts supply the eventual exact retry without
postulating it as an uncheckable fairness slogan.  Equality is over
`ExactAsyncCandidateIdentity`, so a same-class/same-kind successor, a
reconnect, or a foreign source cannot satisfy the handoff accidentally.
***************************************************************************)

Stage2DeferredHandoffIdleReadyInvariant ==
  \A candidate \in AsyncCandidateSet:
    /\ Stage2DeferredHandoffOwned(candidate)
    /\ ProtectedStage2Owned(candidate)
    /\ NodeIdle(candidate.node)
    => Stage2ExactIdleRetryReady(candidate)

Stage2ExactDeferredHandoffProperty(specification) ==
  specification
    => /\ Stage2DeferredHandoffInit
       /\ []Stage2DeferredHandoffTypeInvariant
       /\ []Stage2DeferredHandoffIdleReadyInvariant
       /\ [](\A candidate \in AsyncCandidateSet:
              /\ Stage2BusyRejectedSelected(candidate)
              /\ DeferredDrainStep(candidate.node)
              => Stage2BusyRetryClaimsHandoff(candidate))
       /\ [](\A candidate \in AsyncCandidateSet:
              Stage2HandoffRetentionAction(candidate))
       /\ [](\A candidate \in AsyncCandidateSet:
              Stage2HandoffClearOnlyOnExitAction(candidate))
       /\ [](\A candidate \in AsyncCandidateSet:
              Stage2HandoffExitClearsAction(candidate))
       /\ [](\A node \in ValidatorIds:
              Stage2HandoffCreationOnlyOnBusyRetryAction(node))
       /\ [](\A candidate \in AsyncCandidateSet:
              Stage2NoForeignRebusyAction(candidate))

THEOREM ProtectedStage2RankProgressWithExactHandoffObligation ==
  \A initialContext:
    /\ AsyncSpecAt(initialContext)
    /\ ProtectedPostDeferredRankProgressProperty(
         AsyncSpecAt(initialContext))
    /\ Stage2ExactDeferredHandoffProperty(
         AsyncSpecAt(initialContext))
    => ProtectedStage2RankProgressProperty(
         AsyncSpecAt(initialContext))

(***************************************************************************
Entry-38 composition boundary.

This is the only admissible route to the aggregate protected-rank theorem:
the exact stage-2 Busy/handoff leaf, the independently checked stage-3 and
stage-6 leaves, the existing stage-4 and stage-5 leaves, and the separate
fresh-nonce Serve FIFO theorem.  Omitting Serve would prove only the
candidate half of `ProtectedServiceRanksProgressProperty`.
***************************************************************************)

THEOREM ProtectedServiceRanksProgressLeafCompositionObligation ==
  \A initialContext:
    /\ ProtectedStage2RankProgressProperty(
         AsyncSpecAt(initialContext))
    /\ ProtectedStage3RankProgressProperty(
         AsyncSpecAt(initialContext))
    /\ ProtectedStage4RankProgressProperty(
         AsyncSpecAt(initialContext))
    /\ ProtectedStage5RankProgressProperty(
         AsyncSpecAt(initialContext))
    /\ ProtectedStage6RankProgressProperty(
         AsyncSpecAt(initialContext))
    /\ ProtectedServeRankProgressProperty(
         AsyncSpecAt(initialContext))
    => ProtectedServiceRanksProgressProperty(
         AsyncSpecAt(initialContext))

=============================================================================
