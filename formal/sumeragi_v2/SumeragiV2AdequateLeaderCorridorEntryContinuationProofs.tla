---- MODULE SumeragiV2AdequateLeaderCorridorEntryContinuationProofs ----
EXTENDS SumeragiV2AdequateLeaderSelectedOwnerContinuationProofs

(***************************************************************************
Fresh target/leader corridor entry.

Timeout/view progress proves that one responsive target either decides or
strictly advances its local view.  That result is intentionally insufficient
for this leaf: an authenticated future TC may skip an arbitrary finite set of
views, and a configured adequate timeout does not imply that both the target
and the selected leader still own a fresh service window.  The exact exposure
property below therefore retains the original target and returns one concrete
fresh self-leader corridor.  Choosing the responsive leader itself as the
corridor target collapses the target/leader identity, but not the quorum
timing obligation.  One exact TC is installed at the frozen responsive dual
quorum before the fixed suffix begins.  Because that synchronized leader is
the original target, the frozen-corridor decomposition opens the target's own
productive subject directly; no Decision dissemination is needed for entry.
The exposure provider must discharge the finite future-TC/timeout-origin,
exact synchronization, and fresh-window episodes; it may not consume
aggregate Decision, rotating-leader, application, successor, or height
liveness.

This continuation owns the other half of corridor entry.  Once a frozen
corridor is exposed, the existing state decomposition opens the deterministic
proposal subject or records one exact corridor-exit handoff.  The exit is
composed only with the target-local exit clause of the view-reach property.
No replenishment event is treated as protocol progress.
***************************************************************************)

(***************************************************************************
Authenticated rotating-view exposure.

The public exposure predicate below intentionally keeps its older existential
shape.  The proof uses the strictly stronger target-self endpoint: the
original responsive target itself is the scheduled leader of the fresh view.
That endpoint avoids smuggling aggregate Decision dissemination into view
reach and, because the frozen-corridor decomposition is a state theorem,
opens the original target's productive subject immediately.

There are two different kinds of future certificate debt and they must not be
conflated.

  * `residentTcs` and `residentOrigins` freeze the finite concrete stock at
    the source occurrence.  Installing one of those identities consumes a
    member of the frozen stock before a new self-leader distance is selected.
  * a certificate first formed after the source occurrence is not resident
    debt.  Dual-quorum intersection ties it to one exact responsive honest
    `TimeoutOrigin`.  The separate origin episode below drains that retained
    vote/TC lifecycle through the already-proved physical timeout kernels.

The rank therefore never calls certificate replenishment progress.  Its
outer coordinate is the remaining frozen stock and its inner coordinate is
the finite distance to the target's next adequate self-leader view.  A new
authenticated origin owns a bounded non-descent episode.  Once that episode
closes, either the target has decided, the target owns the fresh self-leader
window, or the outer rank is strictly lower.

No finite maximum-view premise is used.  The timeout floor below follows
directly from the unbounded `Nat` view domain and the configured saturating
linear timeout.  The leader window is one concrete roster period.
***************************************************************************)

AdequateLeaderTimeoutFloor == AsyncWorstCaseServiceBudget

AdequateLeaderResponsiveViewRecords ==
  {[highRank |-> nodeView[node]]:
    node \in AsyncCurrentResponsiveVoters}

AdequateLeaderResponsiveMaximumViewRecord ==
  CHOOSE maximumRecord \in AdequateLeaderResponsiveViewRecords:
    \A otherRecord \in AdequateLeaderResponsiveViewRecords:
      otherRecord.highRank <= maximumRecord.highRank

AdequateLeaderResponsiveMaximumView ==
  AdequateLeaderResponsiveMaximumViewRecord.highRank

AdequateLeaderStrictFutureBase(target) ==
  LET nextView == AdequateLeaderResponsiveMaximumView + 1
  IN IF nextView >= AdequateLeaderTimeoutFloor
     THEN nextView
     ELSE AdequateLeaderTimeoutFloor

AdequateLeaderTargetSelfViewCandidates(target) ==
  LET base == AdequateLeaderStrictFutureBase(target)
      span == Len(context.roster)
  IN {roundView \in base..(base + span - 1):
        Leader(context, roundView) = target}

AdequateLeaderNextTargetSelfView(target) ==
  CHOOSE roundView \in AdequateLeaderTargetSelfViewCandidates(target):
    \A other \in AdequateLeaderTargetSelfViewCandidates(target):
      roundView <= other

AdequateLeaderTargetSelfViewDistance(target) ==
  AdequateLeaderNextTargetSelfView(target) - nodeView[target]

AdequateLeaderResidentFutureTcDebt(target, residentTcs) ==
  {tc \in residentTcs:
    /\ tc \in formedTCs
    /\ tc.context = context
    /\ tc.view >= nodeView[target]
    /\ TCValid(tc)}

AdequateLeaderResidentFutureOriginDebt(target, residentOrigins) ==
  {vote \in residentOrigins:
    /\ vote \in timeoutIntents
    /\ vote.context = context
    /\ vote.height = height
    /\ vote.view >= nodeView[target]
    /\ vote.signer \in AsyncCurrentResponsiveVoters}

AdequateLeaderResidentFutureDebt(target, residentTcs, residentOrigins) ==
  Cardinality(AdequateLeaderResidentFutureTcDebt(target, residentTcs))
    + Cardinality(
        AdequateLeaderResidentFutureOriginDebt(target, residentOrigins))

AdequateLeaderViewExposureRank(target, residentTcs, residentOrigins) ==
  <<AdequateLeaderResidentFutureDebt(
       target, residentTcs, residentOrigins),
    AdequateLeaderTargetSelfViewDistance(target)>>

AdequateLeaderViewExposureRankCarrier == Nat \X Nat

AdequateLeaderViewExposureRankOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat), OpToRel(<, Nat), Nat, Nat)

AdequateLeaderFrozenViewEpisodeSource(
    target, residentTcs, residentOrigins) ==
  /\ AdequateLeaderLocalTargetDecisionSource(target)
  /\ residentTcs = formedTCs
  /\ residentOrigins = timeoutIntents
  /\ IsFiniteSet(residentTcs)
  /\ IsFiniteSet(residentOrigins)

AdequateLeaderTargetFreshSelfCorridorGoal(target) ==
  \/ NodeHasDecision(target)
  \/ \E leaderView \in Views:
       AdequateLeaderFreshSynchronizedTargetCorridor(
         target, context, target, leaderView)

AdequateLeaderViewExposureRankFrontier(
    target, residentTcs, residentOrigins, rank) ==
  /\ AdequateLeaderLocalTargetDecisionSource(target)
  /\ IsFiniteSet(residentTcs)
  /\ IsFiniteSet(residentOrigins)
  /\ ~AdequateLeaderTargetFreshSelfCorridorGoal(target)
  /\ rank = AdequateLeaderViewExposureRank(
       target, residentTcs, residentOrigins)

AdequateLeaderViewExposureStrictDescentGoal(
    target, residentTcs, residentOrigins, sourceRank) ==
  \/ AdequateLeaderTargetFreshSelfCorridorGoal(target)
  \/ \E lowerRank \in SetLessThan(
          sourceRank,
          AdequateLeaderViewExposureRankOrdering,
          AdequateLeaderViewExposureRankCarrier):
       AdequateLeaderViewExposureRankFrontier(
         target, residentTcs, residentOrigins, lowerRank)

AdequateLeaderFirstPostSnapshotTcOccurrence(
    residentTcs, tc) ==
  /\ tc \notin residentTcs
  /\ tc \notin formedTCs
  /\ tc \in formedTCs'

AdequateLeaderPostSnapshotResponsiveOrigin(
    residentOrigins, roundView, vote) ==
  /\ vote \notin residentOrigins
  /\ vote \in timeoutIntents
  /\ TimeoutVoteSemanticIdentity(vote.signer, roundView, vote)
  /\ vote.signer \in AsyncCurrentResponsiveVoters
  /\ TimeoutOrigin(vote.signer, roundView, vote)

AdequateLeaderAuthenticatedTcOriginEpisode(
    target, residentTcs, residentOrigins, roundView, tc) ==
  /\ AdequateLeaderLocalTargetDecisionSource(target)
  /\ tc \in formedTCs \ residentTcs
  /\ TimeoutCertificateSemanticIdentity(tc, roundView)
  /\ nodeView[target] <= tc.view
  /\ \E vote \in tc.votes:
       AdequateLeaderPostSnapshotResponsiveOrigin(
         residentOrigins, roundView, vote)

AdequateLeaderAuthenticatedTcEpisodeIdentity(
    target, episodeContext, roundView, tc) ==
  [target |-> target,
   context |-> episodeContext,
   view |-> roundView,
   certificate |-> tc]

AdequateLeaderAuthenticatedTcEpisodePhysicalOwners(target, tc) ==
  {<<stage, source>>:
    /\ stage \in 1..7
    /\ source \in AsyncCurrentResponsiveVoters
    /\ CASE stage = 1 ->
              \/ TimeoutTcExactPersistCandidateOwner(
                   target, tc, tc.view)
              \/ TimeoutTcInstallWalOwner(target, tc, tc.view)
       [] stage = 2 ->
              \/ TimeoutTcImportedBeginCandidateOwner(
                   target, tc, tc.view)
              \/ TimeoutTcReceivedReducerOwner(target, tc, tc.view)
       [] stage = 3 ->
              TimeoutTcReducerCandidateOwner(
                source, target, tc, tc.view)
       [] stage = 4 ->
              TimeoutTcIngressOwner(source, target, tc, tc.view)
       [] stage = 5 ->
              TimeoutTcPacketOwner(source, target, tc, tc.view)
       [] stage = 6 ->
              TimeoutTcRetainedControlOwner(
                source, target, tc, tc.view)
       [] stage = 7 ->
              TimeoutTcInstallWalOwner(source, tc, tc.view)}

AdequateLeaderAuthenticatedTcEpisodeStageSet(target, tc) ==
  {stage \in 1..7:
    \E source \in AsyncCurrentResponsiveVoters:
      <<stage, source>>
        \in AdequateLeaderAuthenticatedTcEpisodePhysicalOwners(target, tc)}

AdequateLeaderAuthenticatedTcEpisodeStageRank(target, tc) ==
  CHOOSE stage \in AdequateLeaderAuthenticatedTcEpisodeStageSet(target, tc):
    \A other \in AdequateLeaderAuthenticatedTcEpisodeStageSet(target, tc):
      stage <= other

AdequateLeaderAuthenticatedTcEpisodeOwnsStage(target, tc, stage) ==
  stage \in AdequateLeaderAuthenticatedTcEpisodeStageSet(target, tc)

AdequateLeaderAuthenticatedTcEpisodeBoundaryHandoff(
    target, tc, sourceStage) ==
  \/ AdequateLeaderAuthenticatedTcEpisodeRetired(target, tc)'
  \/ \E nextStage \in 1..sourceStage:
       AdequateLeaderAuthenticatedTcEpisodeOwnsStage(
         target, tc, nextStage)'

AdequateLeaderAuthenticatedTcEpisodeAtBudget(
    target, residentTcs, residentOrigins,
    sourceRank, roundView, tc, budget) ==
  /\ AdequateLeaderAuthenticatedTcOriginEpisode(
       target, residentTcs, residentOrigins, roundView, tc)
  /\ AdequateLeaderAuthenticatedTcEpisodeIdentity(
       target, context, roundView, tc)
       = AdequateLeaderAuthenticatedTcEpisodeIdentity(
           target, context, tc.view, tc)
  /\ sourceRank = AdequateLeaderViewExposureRank(
       target, residentTcs, residentOrigins)
  /\ AdequateLeaderAuthenticatedTcEpisodeStageSet(target, tc) # {}
  /\ budget = AdequateLeaderAuthenticatedTcEpisodeStageRank(target, tc)

AdequateLeaderAuthenticatedTcEpisodeSameStageReplacement(
    target, residentTcs, residentOrigins,
    sourceRank, roundView, tc, budget) ==
  /\ AdequateLeaderAuthenticatedTcEpisodeAtBudget(
       target, residentTcs, residentOrigins,
       sourceRank, roundView, tc, budget)
  /\ [AsyncNext]_AsyncAllVars
  /\ ~AdequateLeaderViewExposureStrictDescentGoal(
       target, residentTcs, residentOrigins, sourceRank)'
  /\ (AdequateLeaderAuthenticatedTcEpisodeStageSet(target, tc))' # {}
  /\ Cardinality(
       (AdequateLeaderAuthenticatedTcEpisodePhysicalOwners(target, tc))')
       = Cardinality(
           AdequateLeaderAuthenticatedTcEpisodePhysicalOwners(target, tc))
  /\ (AdequateLeaderAuthenticatedTcEpisodePhysicalOwners(target, tc))'
       # AdequateLeaderAuthenticatedTcEpisodePhysicalOwners(target, tc)
  /\ AdequateLeaderViewExposureRank(
       target, residentTcs, residentOrigins)' = sourceRank
  /\ (AdequateLeaderAuthenticatedTcEpisodeStageRank(target, tc))'
       = budget

\* Retirement is semantic and durable.  A volatile candidate Terminal marker
\* is deliberately insufficient: crash/restart may reopen that same physical
\* work, which is replay of this identity rather than resurrection.  Decision
\* and strict installed-view advance are both persistent Core milestones.
AdequateLeaderDurableExactTcEpisodeRetired(target, tc) ==
  \/ NodeHasDecision(target)
  \/ nodeView[target] > tc.view

AdequateLeaderAuthenticatedTcEpisodeRetired(target, tc) ==
  AdequateLeaderDurableExactTcEpisodeRetired(target, tc)

AdequateLeaderSameRoundUpgradeRemainingBudget(target) ==
  IF highestRank[target] = NoRank
  THEN nodeView[target] + 1
  ELSE nodeView[target] - highestRank[target]

AdequateLeaderSameRoundUpgradeOwnerSet(target) ==
  {tc \in formedTCs:
    /\ StrictSameRoundTcUpgrade(target, tc)
    /\ \E source \in AsyncCurrentResponsiveVoters:
         \/ TimeoutCertificateDelivery(source, target, tc)
         \/ TimeoutCertificateInstallOwner(target, tc)}

AdequateLeaderSameRoundUpgradeEpisodeAtBudget(
    target, leaderView, budget) ==
  /\ gst
  /\ target \in AsyncCurrentResponsiveVoters
  /\ nodeView[target] = leaderView
  /\ Leader(context, leaderView) = target
  /\ AsyncViewTimeout(leaderView) > AsyncWorstCaseServiceBudget
  /\ ~NodeHasDecision(target)
  /\ AdequateLeaderSameRoundUpgradeOwnerSet(target) # {}
  /\ budget = AdequateLeaderSameRoundUpgradeRemainingBudget(target)

AdequateLeaderSameRoundUpgradeEpisodeExitGoal(
    target, leaderView, sourceBudget) ==
  \/ NodeHasDecision(target)
  \/ nodeView[target] > leaderView
  \/ \E lowerBudget \in 0..(sourceBudget - 1):
       AdequateLeaderSameRoundUpgradeEpisodeAtBudget(
         target, leaderView, lowerBudget)

AdequateLeaderSameRoundUpgradeBudgetDescentProperty(specification) ==
  specification
    => \A target \in ValidatorIds,
          leaderView \in Views,
          budget \in Nat:
         AdequateLeaderSameRoundUpgradeEpisodeAtBudget(
           target, leaderView, budget)
           ~> AdequateLeaderSameRoundUpgradeEpisodeExitGoal(
                target, leaderView, budget)

AdequateLeaderSameRoundUpgradeEpisodeClosureProperty(specification) ==
  specification
    => \A target \in ValidatorIds,
          leaderView \in Views,
          budget \in Nat:
         AdequateLeaderSameRoundUpgradeEpisodeAtBudget(
           target, leaderView, budget)
           ~> (NodeHasDecision(target)
                \/ nodeView[target] > leaderView)

(***************************************************************************
Exact-TC responsive-roster synchronization.

The selected adequate self view is strictly above every responsive voter's
view at the source occurrence.  Its first exact TC install starts one timed
episode.  The immutable TC is retained for every voter in the frozen
responsive dual quorum; the physical rank below drains those exact deliveries
without restarting the clock budget.  A newer installed view exits back to
the outer rotating-view rank.  Only the no-overtake case reaches a synchronized
fixed corridor.
***************************************************************************)

AdequateLeaderSynchronizationRemainingNodes(roster, tc) ==
  {node \in roster: nodeView[node] <= tc.view}

AdequateLeaderSynchronizationOvershot(roster, leaderView) ==
  \E node \in roster: nodeView[node] > leaderView

AdequateLeaderSynchronizationSelectedNode(roster, tc) ==
  CHOOSE node \in AdequateLeaderSynchronizationRemainingNodes(roster, tc):
    \A other \in AdequateLeaderSynchronizationRemainingNodes(roster, tc):
      node <= other

AdequateLeaderSynchronizationSelectedStage(roster, tc) ==
  AdequateLeaderAuthenticatedTcEpisodeStageRank(
    AdequateLeaderSynchronizationSelectedNode(roster, tc), tc)

AdequateLeaderSynchronizationPhysicalRank(roster, tc) ==
  LET remaining ==
        AdequateLeaderSynchronizationRemainingNodes(roster, tc)
  IN IF remaining = {}
     THEN 0
     ELSE Cardinality(remaining) * 8
            + AdequateLeaderSynchronizationSelectedStage(roster, tc)

AdequateLeaderSynchronizationClockSlack(startTime) ==
  IF asyncNow - startTime <= AsyncViewSynchronizationBudget
  THEN AsyncViewSynchronizationBudget - (asyncNow - startTime)
  ELSE 0

AdequateLeaderSynchronizationRank(roster, tc, startTime) ==
  <<AdequateLeaderSynchronizationClockSlack(startTime),
    AdequateLeaderSynchronizationPhysicalRank(roster, tc)>>

AdequateLeaderSynchronizationRankCarrier == Nat \X Nat

AdequateLeaderSynchronizationRankOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat), OpToRel(<, Nat), Nat, Nat)

AdequateLeaderInstalledSynchronizationPrefixNode(
    node, episodeContext, tc, leaderView, firstDeadline) ==
  /\ nodeView[node] = leaderView
  /\ [node |-> node, tc |-> tc] \in installedTCs
  /\ asyncNodeDeadlines[node] >= firstDeadline
  /\ ~NodeTimedOut(node, leaderView)
  /\ ~asyncTimeoutEmitted[node]
  /\ "TimeoutElapsed" \notin asyncOutstandingTags[node]
  /\ ~AdequateLeaderOlderOrEqualTimeoutLifecycleOwned(
       node, episodeContext, leaderView)

AdequateLeaderExactTcSynchronizationSource(
    target, episodeContext, roster, tc,
    leaderView, startTime, firstDeadline) ==
  /\ gst
  /\ episodeContext = context
  /\ roster = Responsive \cap VotingRoster(episodeContext.epoch)
  /\ roster # {}
  /\ IsFiniteSet(roster)
  /\ DualQuorum(episodeContext.epoch, roster)
  /\ target \in roster
  /\ tc \in formedTCs
  /\ TimeoutCertificateSemanticIdentity(tc, tc.view)
  /\ tc.context = episodeContext
  /\ leaderView = tc.view + 1
  /\ leaderView \in Views
  /\ Leader(episodeContext, leaderView) = target
  /\ AsyncViewTimeout(leaderView) > AsyncWorstCaseServiceBudget
  /\ nodeView[target] = leaderView
  /\ \A node \in roster \ {target}: nodeView[node] < leaderView
  /\ startTime = asyncNow
  /\ firstDeadline = asyncNodeDeadlines[target]
  /\ startTime + AsyncWorstCaseServiceBudget < firstDeadline
  /\ AdequateLeaderInstalledSynchronizationPrefixNode(
       target, episodeContext, tc, leaderView, firstDeadline)
  /\ \A node \in roster \ {target}:
       TimeoutTcRetainedControlOwner(target, node, tc, tc.view)

AdequateLeaderExactTcSynchronizationAtRank(
    target, episodeContext, roster, tc,
    leaderView, startTime, firstDeadline, rank) ==
  /\ gst
  /\ context = episodeContext
  /\ roster = Responsive \cap VotingRoster(episodeContext.epoch)
  /\ roster # {}
  /\ IsFiniteSet(roster)
  /\ DualQuorum(episodeContext.epoch, roster)
  /\ target \in roster
  /\ tc \in formedTCs
  /\ tc.context = episodeContext
  /\ leaderView = tc.view + 1
  /\ Leader(episodeContext, leaderView) = target
  /\ AsyncViewTimeout(leaderView) > AsyncWorstCaseServiceBudget
  /\ ~NodeHasDecision(target)
  /\ ~AdequateLeaderSynchronizationOvershot(roster, leaderView)
  /\ asyncNow - startTime <= AsyncViewSynchronizationBudget
  /\ asyncNow + AsyncFixedCorridorServiceBudget < firstDeadline
  /\ \A node \in roster:
       nodeView[node] <= leaderView
  /\ \A node \in
       roster \ AdequateLeaderSynchronizationRemainingNodes(roster, tc):
       AdequateLeaderInstalledSynchronizationPrefixNode(
         node, episodeContext, tc, leaderView, firstDeadline)
  /\ \A node \in AdequateLeaderSynchronizationRemainingNodes(roster, tc):
       AdequateLeaderAuthenticatedTcEpisodeStageSet(node, tc) # {}
  /\ rank = AdequateLeaderSynchronizationRank(roster, tc, startTime)

AdequateLeaderExactTcSynchronizationGoal(
    target, episodeContext, roster, tc, leaderView) ==
  \/ NodeHasDecision(target)
  \/ AdequateLeaderSynchronizationOvershot(roster, leaderView)
  \/ /\ AdequateLeaderSynchronizationRemainingNodes(roster, tc) = {}
     /\ AdequateLeaderFreshSynchronizedTargetCorridor(
          target, episodeContext, target, leaderView)

AdequateLeaderExactTcSynchronizationDescentGoal(
    target, episodeContext, roster, tc,
    leaderView, startTime, firstDeadline, sourceRank) ==
  \/ AdequateLeaderExactTcSynchronizationGoal(
       target, episodeContext, roster, tc, leaderView)
  \/ \E lowerRank \in SetLessThan(
          sourceRank,
          AdequateLeaderSynchronizationRankOrdering,
          AdequateLeaderSynchronizationRankCarrier):
       AdequateLeaderExactTcSynchronizationAtRank(
         target, episodeContext, roster, tc,
         leaderView, startTime, firstDeadline, lowerRank)

AdequateLeaderExactTcSynchronizationRankStepProperty(specification) ==
  specification
    => \A target \in ValidatorIds,
          episodeContext \in ContextRecords,
          roster \in SUBSET ValidatorIds,
          tc \in TcRecordSet,
          leaderView \in Views,
          startTime, firstDeadline \in Nat,
          sourceRank \in AdequateLeaderSynchronizationRankCarrier:
         AdequateLeaderExactTcSynchronizationAtRank(
           target, episodeContext, roster, tc,
           leaderView, startTime, firstDeadline, sourceRank)
           ~> AdequateLeaderExactTcSynchronizationDescentGoal(
                target, episodeContext, roster, tc,
                leaderView, startTime, firstDeadline, sourceRank)

AdequateLeaderExactTcSynchronizationClosureProperty(specification) ==
  specification
    => \A target \in ValidatorIds,
          episodeContext \in ContextRecords,
          roster \in SUBSET ValidatorIds,
          tc \in TcRecordSet,
          leaderView \in Views,
          startTime, firstDeadline \in Nat,
          sourceRank \in AdequateLeaderSynchronizationRankCarrier:
         AdequateLeaderExactTcSynchronizationAtRank(
           target, episodeContext, roster, tc,
           leaderView, startTime, firstDeadline, sourceRank)
           ~> AdequateLeaderExactTcSynchronizationGoal(
                target, episodeContext, roster, tc, leaderView)

(***************************************************************************
Fixed-suffix deadline receipt.

The synchronization endpoint has a strict fixed-suffix margin at every
member of its frozen responsive dual quorum.  The synthetic boundary below
is exactly one tick after that configured suffix and is no later than any
member's actual timeout deadline.  The ghost receipt records this boundary;
it does not clamp a later deadline or disable Tick.  The service property is
therefore deliberately an obligation, not a timing assumption: its provider
must charge the exact packet, I/O, deferred, runner, and
producer-continuation episodes to the frozen cumulative budget.  Qualitative
weak fairness and a finite rank alone do not provide the numeric inequality.
***************************************************************************)

AdequateLeaderFixedCorridorDeadlineSource(
    target, leaderContext, leaderView, startTime, deadline) ==
  /\ AdequateLeaderFreshSynchronizedTargetCorridor(
       target, leaderContext, target, leaderView)
  /\ startTime = asyncNow
  /\ deadline = startTime + AsyncFixedCorridorServiceBudget + 1
  /\ \A node \in AdequateLeaderFrozenResponsiveRoster(leaderContext):
       deadline <= asyncNodeDeadlines[node]

AdequateLeaderFixedCorridorDecisionBeforeDeadline(target, deadline) ==
  /\ NodeHasDecision(target)
  /\ asyncNow < deadline

AdequateLeaderFixedCorridorDeadlineServiceProperty(specification) ==
  specification
    => \A target \in ValidatorIds,
          leaderContext \in ContextRecords,
          leaderView \in Views,
          startTime, deadline \in Nat:
         AdequateLeaderFixedCorridorDeadlineSource(
           target, leaderContext, leaderView, startTime, deadline)
           ~> AdequateLeaderFixedCorridorDecisionBeforeDeadline(
                target, deadline)

(***************************************************************************
Ghost-receipt safety and action erasure.

The receipt is deliberately outside `AsyncSchedulerVars`.  Its own induction
below proves the finite typed carrier, constructor provenance, and armed-clock
relation without adding a premise to any production scheduler action.  The
action-erasure theorem in `SumeragiV2AsyncNetwork` existentially removes the
ghost next value and proves that every original step has exactly the
constructor-selected extension.
***************************************************************************)

AsyncFixedCorridorDeadlineReceiptClockInvariant ==
  \A receipt \in AsyncFixedCorridorDeadlineReceipts:
    receipt.armedAt <= asyncNow

AsyncFixedCorridorDeadlineArmableReceiptCoverageInvariant ==
  \A target \in AsyncFixedCorridorDeadlineArmableTargets:
    AsyncFixedCorridorDeadlineReceiptKeyOwned(
      target, context, nodeView[target])

AsyncFixedCorridorDeadlineReceiptProofInvariant ==
  /\ AsyncFixedCorridorDeadlineReceiptInvariant
  /\ AsyncFixedCorridorDeadlineReceiptClockInvariant
  /\ AsyncFixedCorridorDeadlineArmableReceiptCoverageInvariant

THEOREM AsyncFixedCorridorDeadlineInitEstablishesReceiptInvariant ==
  \A initialContext:
    AsyncInitAt(initialContext)
      => AsyncFixedCorridorDeadlineReceiptInvariant
BY FS_EmptySet, Isa
   DEF AsyncInitAt, AsyncBaseInitAt, AsyncTransportInit,
       AsyncFixedCorridorDeadlineReceiptInvariant,
       AsyncFixedCorridorDeadlineReceiptTypeInvariant,
       AsyncFixedCorridorDeadlineReceiptProvenanceInvariant,
       AsyncFixedCorridorDeadlineReceipts,
       AsyncFixedCorridorDeadlineReceiptsFor

THEOREM AsyncFixedCorridorDeadlineInitEstablishesArmableCoverage ==
  \A initialContext:
    AsyncInitAt(initialContext)
      => AsyncFixedCorridorDeadlineArmableReceiptCoverageInvariant
BY Isa
   DEF AsyncInitAt, AsyncBaseInitAt, InitAt,
       AsyncFixedCorridorDeadlineArmableReceiptCoverageInvariant,
       AsyncFixedCorridorDeadlineArmableTargets,
       AsyncFixedCorridorDeadlineArmable

THEOREM AsyncFixedCorridorDeadlineTransitionPublishesThreeOwnershipCases ==
  AsyncFixedCorridorDeadlineTransition
    => /\ \A receipt \in AsyncFixedCorridorDeadlineRetainedAfterStep:
              receipt \in AsyncFixedCorridorDeadlineReceipts'
       /\ \A target \in AsyncFixedCorridorDeadlineNewTargets:
              AsyncFixedCorridorDeadlineReceipt(
                target, context, nodeView[target], asyncNow)
                \in AsyncFixedCorridorDeadlineReceipts'
       /\ \A target \in AsyncFixedCorridorDeadlinePostStepNewTargets:
              AsyncFixedCorridorDeadlineReceipt(
                target, context', nodeView'[target], asyncNow')
                \in AsyncFixedCorridorDeadlineReceipts'
BY Isa
   DEF AsyncFixedCorridorDeadlineTransition,
       AsyncFixedCorridorDeadlinesAfterStep

\* A key-changing target cannot use the pre-state arm and cannot retain an
\* old receipt.  If the new self-leader key is armable, the post-state branch
\* publishes exactly the new key at the post-state clock.
THEOREM AsyncFixedCorridorDeadlineChangedLeaderKeyUsesPostStateArm ==
  \A target \in ValidatorIds:
    /\ target \in AsyncFixedCorridorDeadlineChangedKeyTargets
    /\ target \in AsyncFixedCorridorDeadlineArmableTargets'
    /\ AsyncFixedCorridorDeadlineTransition
    => AsyncFixedCorridorDeadlineReceipt(
         target, context', nodeView'[target], asyncNow')
         \in AsyncFixedCorridorDeadlineReceipts'
BY AsyncFixedCorridorDeadlineTransitionPublishesThreeOwnershipCases,
   Isa
   DEF AsyncFixedCorridorDeadlineRetainedAfterStep,
       AsyncFixedCorridorDeadlineChangedKeyTargets,
       AsyncFixedCorridorDeadlineNewTargets,
       AsyncFixedCorridorDeadlinePostStepReceiptKeyOwned,
       AsyncFixedCorridorDeadlinePostStepNewTargets

THEOREM AsyncFixedCorridorDeadlineTransitionPreservesArmableCoverage ==
  /\ AsyncFixedCorridorDeadlineArmableReceiptCoverageInvariant
  /\ AsyncFixedCorridorDeadlineTransition
  => AsyncFixedCorridorDeadlineArmableReceiptCoverageInvariant'
BY AsyncFixedCorridorDeadlineTransitionPublishesThreeOwnershipCases,
   AsyncFixedCorridorDeadlineChangedLeaderKeyUsesPostStateArm,
   IsaT(1800)
   DEF AsyncFixedCorridorDeadlineArmableReceiptCoverageInvariant,
       AsyncFixedCorridorDeadlineArmableTargets,
       AsyncFixedCorridorDeadlineReceiptKeyOwned,
       AsyncFixedCorridorDeadlineRetainedAfterStep,
       AsyncFixedCorridorDeadlineChangedKeyTargets,
       AsyncFixedCorridorDeadlineNewTargets,
       AsyncFixedCorridorDeadlinePostStepReceiptKeyOwned,
       AsyncFixedCorridorDeadlinePostStepNewTargets,
       AsyncFixedCorridorDeadlineReceipts

THEOREM AsyncFixedCorridorDeadlineTransitionPreservesReceiptInvariant ==
  /\ ModelConfiguration
  /\ AsyncFixedCorridorDeadlineReceiptInvariant
  /\ AsyncFixedCorridorDeadlineTransition
  => AsyncFixedCorridorDeadlineReceiptInvariant'
BY FS_Subset, FS_Image, FS_Union, FS_CardinalityType, IsaT(1200)
   DEF AsyncFixedCorridorDeadlineReceiptInvariant,
       AsyncFixedCorridorDeadlineReceiptTypeInvariant,
       AsyncFixedCorridorDeadlineReceiptProvenanceInvariant,
       AsyncFixedCorridorDeadlineTransition,
       AsyncFixedCorridorDeadlinesAfterStep,
       AsyncFixedCorridorDeadlineRetainedAfterStep,
       AsyncFixedCorridorDeadlineChangedKeyTargets,
       AsyncFixedCorridorDeadlineNewTargets,
       AsyncFixedCorridorDeadlinePostStepReceiptKeyOwned,
       AsyncFixedCorridorDeadlinePostStepNewTargets,
       AsyncFixedCorridorDeadlineUnarmedTargets,
       AsyncFixedCorridorDeadlineArmableTargets,
       AsyncFixedCorridorDeadlineReceiptKeyOwned,
       AsyncFixedCorridorDeadlineReceipt,
       AsyncFixedCorridorDeadlineReceiptSet,
       AsyncFixedCorridorDeadlineReceipts,
       AsyncFixedCorridorDeadlineReceiptsFor

THEOREM AsyncFixedCorridorDeadlineAsyncNextClockIsMonotone ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncNext
  => asyncNow' >= asyncNow
PROOF
  <1>1. ASSUME AsyncStrongTypeInvariant, AsyncNext
         PROVE asyncNow' >= asyncNow
    <2>1. CASE AsyncTick
      BY <1>1, <2>1, ExactDecisionRequestTypedTickAdvancesClock
    <2>2. CASE ~AsyncTick
      <3>1. asyncNow' = asyncNow
        BY <1>1, <2>2,
           ExactDecisionTargetNeutralNonTickAsyncNextLeavesClock
      <3> QED BY <3>1
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM AsyncFixedCorridorDeadlineTransitionPreservesReceiptClock ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncFixedCorridorDeadlineReceiptClockInvariant
  /\ AsyncNext
  => AsyncFixedCorridorDeadlineReceiptClockInvariant'
BY AsyncFixedCorridorDeadlineAsyncNextClockIsMonotone,
   IsaT(900)
   DEF AsyncNext,
       AsyncFixedCorridorDeadlineReceiptClockInvariant,
       AsyncFixedCorridorDeadlineTransition,
       AsyncFixedCorridorDeadlinesAfterStep,
       AsyncFixedCorridorDeadlineRetainedAfterStep,
       AsyncFixedCorridorDeadlineChangedKeyTargets,
       AsyncFixedCorridorDeadlineNewTargets,
       AsyncFixedCorridorDeadlineUnarmedTargets,
       AsyncFixedCorridorDeadlinePostStepReceiptKeyOwned,
       AsyncFixedCorridorDeadlinePostStepNewTargets,
       AsyncFixedCorridorDeadlineReceipt

THEOREM AsyncFixedCorridorDeadlineBracketPreservesProofInvariant ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncFixedCorridorDeadlineReceiptProofInvariant
  /\ [AsyncNext]_AsyncAllVars
  => AsyncFixedCorridorDeadlineReceiptProofInvariant'
PROOF
  <1>1. ASSUME AsyncStrongTypeInvariant,
                AsyncFixedCorridorDeadlineReceiptProofInvariant,
                [AsyncNext]_AsyncAllVars
         PROVE AsyncFixedCorridorDeadlineReceiptProofInvariant'
    <2>1. CASE AsyncNext
      <3>1. AsyncFixedCorridorDeadlineReceiptInvariant'
        BY <1>1, <2>1,
           AsyncFixedCorridorDeadlineTransitionPreservesReceiptInvariant
           DEF AsyncNext,
               AsyncFixedCorridorDeadlineReceiptProofInvariant,
               AsyncStrongTypeInvariant, StrongInductiveInvariant,
               Safety, TypeInvariant
      <3>2. AsyncFixedCorridorDeadlineReceiptClockInvariant'
        BY <1>1, <2>1,
           AsyncFixedCorridorDeadlineTransitionPreservesReceiptClock
           DEF AsyncFixedCorridorDeadlineReceiptProofInvariant
      <3>3. AsyncFixedCorridorDeadlineArmableReceiptCoverageInvariant'
        BY <1>1, <2>1,
           AsyncFixedCorridorDeadlineTransitionPreservesArmableCoverage
           DEF AsyncNext,
               AsyncFixedCorridorDeadlineReceiptProofInvariant
      <3> QED BY <3>1, <3>2, <3>3
           DEF AsyncFixedCorridorDeadlineReceiptProofInvariant
    <2>2. CASE UNCHANGED AsyncAllVars
      BY <1>1, <2>2, Isa
         DEF AsyncAllVars,
             AsyncFixedCorridorDeadlineReceiptProofInvariant,
             AsyncFixedCorridorDeadlineReceiptInvariant,
             AsyncFixedCorridorDeadlineReceiptClockInvariant,
             AsyncFixedCorridorDeadlineArmableReceiptCoverageInvariant
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM AsyncSpecAlwaysFixedCorridorDeadlineReceiptProofInvariant ==
  \A initialContext:
    AsyncSpecAt(initialContext)
      => []AsyncFixedCorridorDeadlineReceiptProofInvariant
PROOF
  <1>1. ASSUME NEW initialContext, AsyncSpecAt(initialContext)
         PROVE []AsyncFixedCorridorDeadlineReceiptProofInvariant
    <2>1. AsyncInitAt(initialContext)
             => AsyncFixedCorridorDeadlineReceiptProofInvariant
      BY AsyncFixedCorridorDeadlineInitEstablishesReceiptInvariant,
         AsyncFixedCorridorDeadlineInitEstablishesArmableCoverage,
         FS_EmptySet, Isa
         DEF AsyncInitAt, AsyncBaseInitAt, AsyncTransportInit,
             AsyncFixedCorridorDeadlineReceiptProofInvariant,
             AsyncFixedCorridorDeadlineReceiptClockInvariant,
             AsyncFixedCorridorDeadlineArmableReceiptCoverageInvariant,
             AsyncFixedCorridorDeadlineReceipts
    <2>2. []AsyncStrongTypeInvariant
      BY <1>1, AsyncSpecAlwaysStrongTypeInvariant
    <2>3. /\ AsyncStrongTypeInvariant
           /\ AsyncFixedCorridorDeadlineReceiptProofInvariant
           /\ [AsyncNext]_AsyncAllVars
          => AsyncFixedCorridorDeadlineReceiptProofInvariant'
      BY AsyncFixedCorridorDeadlineBracketPreservesProofInvariant
    <2> QED BY <1>1, <2>1, <2>2, <2>3, PTL
         DEF AsyncSpecAt
  <1> QED BY <1>1

THEOREM AsyncLiveAlwaysFixedCorridorDeadlineReceiptProofInvariant ==
  \A initialContext:
    AsyncLiveSpecAt(initialContext)
      => []AsyncFixedCorridorDeadlineReceiptProofInvariant
BY AsyncLiveSpecProjectsAsyncSpec,
   AsyncSpecAlwaysFixedCorridorDeadlineReceiptProofInvariant

AsyncFixedCorridorDeadlineArmableReceiptCoverageProperty(specification) ==
  specification
    => []AsyncFixedCorridorDeadlineArmableReceiptCoverageInvariant

THEOREM AsyncLiveProvidesFixedCorridorArmableReceiptCoverage ==
  \A initialContext:
    AsyncFixedCorridorDeadlineArmableReceiptCoverageProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncLiveAlwaysFixedCorridorDeadlineReceiptProofInvariant
   DEF AsyncFixedCorridorDeadlineArmableReceiptCoverageProperty,
       AsyncFixedCorridorDeadlineReceiptProofInvariant

\* Two active receipts must name the same synchronized roster view.  The
\* deterministic Leader function then gives the same target, and the
\* per-target receipt invariant makes the records equal.
THEOREM AsyncActiveFixedCorridorDeadlineReceiptsAreSingular ==
  AsyncFixedCorridorDeadlineReceiptInvariant
    => /\ IsFiniteSet(AsyncActiveFixedCorridorDeadlineReceipts)
       /\ Cardinality(AsyncActiveFixedCorridorDeadlineReceipts) <= 1
BY FS_Subset, FS_CardinalityType, IsaT(600)
   DEF AsyncFixedCorridorDeadlineReceiptInvariant,
       AsyncFixedCorridorDeadlineReceiptTypeInvariant,
       AsyncActiveFixedCorridorDeadlineReceipts,
       AsyncFixedCorridorDeadlineReceipts,
       AsyncFixedCorridorDeadlineReceiptsFor

THEOREM AsyncActiveFixedCorridorDeadlineChooseIsOwned ==
  /\ AsyncFixedCorridorDeadlineReceiptInvariant
  /\ AsyncFixedCorridorDeadlineActive
  => /\ AsyncActiveFixedCorridorDeadlineReceipt
           \in AsyncActiveFixedCorridorDeadlineReceipts
     /\ Cardinality(AsyncActiveFixedCorridorDeadlineReceipts) = 1
BY AsyncActiveFixedCorridorDeadlineReceiptsAreSingular,
   FS_CardinalityType, Isa
   DEF AsyncFixedCorridorDeadlineActive,
       AsyncActiveFixedCorridorDeadlineReceipt

THEOREM AsyncActiveFixedCorridorDeadlineReceiptIsTypedProvenantAndStrict ==
  /\ AsyncFixedCorridorDeadlineReceiptProofInvariant
  /\ AsyncFixedCorridorDeadlineActive
  => LET receipt == AsyncActiveFixedCorridorDeadlineReceipt
     IN /\ receipt \in AsyncFixedCorridorDeadlineReceiptSet
        /\ receipt.deadline =
             receipt.armedAt + AsyncFixedCorridorServiceBudget + 1
        /\ receipt.armedAt <= asyncNow
        /\ asyncNow < receipt.deadline
BY AsyncActiveFixedCorridorDeadlineChooseIsOwned, Isa
   DEF AsyncFixedCorridorDeadlineReceiptProofInvariant,
       AsyncFixedCorridorDeadlineReceiptInvariant,
       AsyncFixedCorridorDeadlineReceiptTypeInvariant,
       AsyncFixedCorridorDeadlineReceiptProvenanceInvariant,
       AsyncFixedCorridorDeadlineReceiptClockInvariant,
       AsyncFixedCorridorDeadlineActive,
       AsyncActiveFixedCorridorDeadlineReceipt,
       AsyncActiveFixedCorridorDeadlineReceipts

THEOREM AsyncFixedCorridorServiceBudgetDominatesRefreshPeriods ==
  ModelConfiguration
    => /\ AsyncDeliveryBound <= AsyncFixedCorridorServiceBudget
       /\ AsyncRetransmitPeriod <= AsyncFixedCorridorServiceBudget
BY SMT
   DEF ModelConfiguration, AsyncConfiguration,
       AsyncFixedCorridorServiceBudget,
       AsyncProposalPipelineBudget,
       AsyncCandidatePhysicalServiceBudget,
       AsyncCandidateProducerActionEpisodeBudget,
       AsyncCandidateProducerEpisodeBudget,
       AsyncCandidateProducerEpisodeCapacity,
       AsyncCausalCandidateLifecycleCapacity,
       AsyncOneWayTransportBudget,
       AsyncRuntimeCycleBudget, AsyncRunnerCycleBudget,
       AsyncIoDrainBudget, AsyncDeferredDrainBudget,
       AsyncRetransmitEmissionBudget,
       AsyncRetainedControlBudget,
       AsyncRetainedProposalChunkBudget,
       AsyncActiveRequestBudget,
       AsyncActiveCertifiedRequestBudget,
       AsyncActiveCommitRequestBudget

\* Arming reads the pre-state, so publication/service refreshes in that same
\* transition do not yet see an active receipt.  They are nevertheless
\* strictly inside the newly recorded boundary by configured arithmetic.
THEOREM AsyncFixedCorridorFirstArmRefreshesAreStrict ==
  \A target \in AsyncFixedCorridorDeadlineNewTargets, item:
    ModelConfiguration
      => LET receipt ==
               AsyncFixedCorridorDeadlineReceipt(
                 target, context, nodeView[target], asyncNow)
         IN /\ PacketForItem(item).deadline < receipt.deadline
            /\ asyncNow + AsyncDeliveryBound < receipt.deadline
            /\ asyncNow + AsyncRetransmitPeriod < receipt.deadline
BY AsyncFixedCorridorServiceBudgetDominatesRefreshPeriods, SMT
   DEF AsyncFixedCorridorDeadlineReceipt,
       PacketForItem, AsyncPacket

THEOREM AdequateLeaderFixedCorridorSourceIsReceiptArmable ==
  \A target \in ValidatorIds,
     leaderContext \in ContextRecords,
     leaderView \in Views,
     startTime, deadline \in Nat:
    /\ AsyncStrongTypeInvariant
    /\ AdequateLeaderFixedCorridorDeadlineSource(
         target, leaderContext, leaderView, startTime, deadline)
    => AsyncFixedCorridorDeadlineArmable(target)
BY IsaT(900)
   DEF AdequateLeaderFixedCorridorDeadlineSource,
       AdequateLeaderFreshSynchronizedTargetCorridor,
       AdequateLeaderFrozenTargetCorridor,
       AdequateLeaderCorridorAuthorityReceipt,
       AdequateLeaderCorridorAuthorityReceiptValid,
       AdequateLeaderResponsiveViewSynchronized,
       AdequateLeaderFreshNodeServiceWindow,
       AdequateLeaderActiveNodeServiceWindow,
       AdequateLeaderFrozenResponsiveRoster,
       AdequateLeaderOlderOrEqualTimeoutLifecycleOwned,
       AsyncFixedCorridorDeadlineArmable,
       AsyncStrongTypeInvariant, StrongInductiveInvariant,
       Safety, TypeInvariant, ModelConfiguration

THEOREM AsyncFixedCorridorChangedTargetKeyRetiresOrRearmsExactly ==
  \A target \in ValidatorIds:
  /\ AsyncFixedCorridorDeadlineReceiptInvariant
  /\ target \in AsyncFixedCorridorDeadlineChangedKeyTargets
  /\ AsyncFixedCorridorDeadlineTransition
  => \A receipt \in AsyncFixedCorridorDeadlineReceipts':
       receipt.target = target
         => /\ receipt.context = context'
            /\ receipt.view = nodeView'[target]
BY Isa
   DEF AsyncFixedCorridorDeadlineReceiptInvariant,
       AsyncFixedCorridorDeadlineReceiptTypeInvariant,
       AsyncFixedCorridorDeadlineTransition,
       AsyncFixedCorridorDeadlinesAfterStep,
       AsyncFixedCorridorDeadlineRetainedAfterStep,
       AsyncFixedCorridorDeadlineChangedKeyTargets,
       AsyncFixedCorridorDeadlineNewTargets,
       AsyncFixedCorridorDeadlinePostStepReceiptKeyOwned,
       AsyncFixedCorridorDeadlinePostStepNewTargets,
       AsyncFixedCorridorDeadlineReceipts,
       AsyncFixedCorridorDeadlineReceipt

THEOREM AdequateLeaderPersistInstallExitsOldCorridor ==
  \A target \in ValidatorIds,
     leaderContext \in ContextRecords,
     leaderView \in Views,
     startTime, deadline \in Nat,
     command:
    /\ AsyncStrongTypeInvariant
    /\ AsyncFixedCorridorDeadlineReceiptInvariant
    /\ AdequateLeaderFixedCorridorDeadlineSource(
         target, leaderContext, leaderView, startTime, deadline)
    /\ command.node
         \in AdequateLeaderFrozenResponsiveRoster(leaderContext)
    /\ AsyncNext
    /\ ExecutePersistInstall(command)
    => ~AdequateLeaderFixedCorridorDeadlineSource(
          target, leaderContext, leaderView,
          startTime, deadline)'
BY ExecutePersistInstallAdvancesCertifiedView,
   IsaT(1800)
   DEF AdequateLeaderFixedCorridorDeadlineSource,
       AdequateLeaderFreshSynchronizedTargetCorridor,
       AdequateLeaderFrozenTargetCorridor,
       AdequateLeaderResponsiveViewSynchronized,
       AdequateLeaderActiveNodeServiceWindow,
       AdequateLeaderFrozenResponsiveRoster,
       ExecutePersistInstall, AsyncNext

AdequateLeaderAuthenticatedTcEpisodeExitGoal(
    target, residentTcs, residentOrigins,
    sourceRank, roundView, tc, budget) ==
  \/ AdequateLeaderViewExposureStrictDescentGoal(
       target, residentTcs, residentOrigins, sourceRank)
  \/ \E lowerBudget \in 1..(budget - 1):
       AdequateLeaderAuthenticatedTcEpisodeAtBudget(
         target, residentTcs, residentOrigins,
         sourceRank, roundView, tc, lowerBudget)

AdequateLeaderAuthenticatedTcEpisodeBudgetDescentProperty(specification) ==
  specification
    => \A target \in ValidatorIds,
          residentTcs \in SUBSET TcRecordSet,
          residentOrigins \in SUBSET TimeoutVoteRecordSet,
          sourceRank \in AdequateLeaderViewExposureRankCarrier,
          roundView \in Views,
          tc \in TcRecordSet,
          budget \in Nat:
         AdequateLeaderAuthenticatedTcEpisodeAtBudget(
           target, residentTcs, residentOrigins,
           sourceRank, roundView, tc, budget)
           ~> AdequateLeaderAuthenticatedTcEpisodeExitGoal(
                target, residentTcs, residentOrigins,
                sourceRank, roundView, tc, budget)

AdequateLeaderAuthenticatedTcEpisodeClosureProperty(specification) ==
  specification
    => \A target \in ValidatorIds,
          residentTcs \in SUBSET TcRecordSet,
          residentOrigins \in SUBSET TimeoutVoteRecordSet,
          sourceRank \in AdequateLeaderViewExposureRankCarrier,
          roundView \in Views,
          tc \in TcRecordSet,
          budget \in Nat:
         AdequateLeaderAuthenticatedTcEpisodeAtBudget(
           target, residentTcs, residentOrigins,
           sourceRank, roundView, tc, budget)
           ~> AdequateLeaderViewExposureStrictDescentGoal(
                target, residentTcs, residentOrigins, sourceRank)

AdequateLeaderAuthenticatedTcOvertakeSource(
    target, residentTcs, residentOrigins,
    sourceRank, selectedView, roundView, tc) ==
  /\ AdequateLeaderViewExposureRankFrontier(
       target, residentTcs, residentOrigins, sourceRank)
  /\ selectedView = AdequateLeaderNextTargetSelfView(target)
  /\ nodeView[target] < selectedView
  /\ roundView + 1 > selectedView
  /\ AdequateLeaderAuthenticatedTcOriginEpisode(
       target, residentTcs, residentOrigins, roundView, tc)

AdequateLeaderAuthenticatedTcNoOvertakeProperty(specification) ==
  specification
    => \A target \in ValidatorIds,
          residentTcs \in SUBSET TcRecordSet,
          residentOrigins \in SUBSET TimeoutVoteRecordSet,
          sourceRank \in AdequateLeaderViewExposureRankCarrier,
          selectedView, roundView \in Views,
          tc \in TcRecordSet:
         AdequateLeaderAuthenticatedTcOvertakeSource(
           target, residentTcs, residentOrigins,
           sourceRank, selectedView, roundView, tc)
           ~> AdequateLeaderViewExposureStrictDescentGoal(
                target, residentTcs, residentOrigins, sourceRank)

AdequateLeaderAuthenticatedTcNoResurrectionProperty(specification) ==
  specification
    => [](\A target \in ValidatorIds,
             residentTcs \in SUBSET TcRecordSet,
             residentOrigins \in SUBSET TimeoutVoteRecordSet,
             roundView \in Views,
             tc \in TcRecordSet:
            AdequateLeaderDurableExactTcEpisodeRetired(target, tc)
              => []~AdequateLeaderAuthenticatedTcOriginEpisode(
                    target, residentTcs, residentOrigins, roundView, tc))

AdequateLeaderViewExposureRankStepProperty(specification) ==
  specification
    => \A target \in ValidatorIds,
          residentTcs \in SUBSET TcRecordSet,
          residentOrigins \in SUBSET TimeoutVoteRecordSet,
          sourceRank \in AdequateLeaderViewExposureRankCarrier:
         AdequateLeaderViewExposureRankFrontier(
           target, residentTcs, residentOrigins, sourceRank)
           ~> AdequateLeaderViewExposureStrictDescentGoal(
                target, residentTcs, residentOrigins, sourceRank)

AdequateLeaderLocalFreshSelfCorridorExposureProperty(specification) ==
  specification
    => \A target \in ValidatorIds:
         AdequateLeaderLocalTargetDecisionSource(target)
           ~> AdequateLeaderTargetFreshSelfCorridorGoal(target)

THEOREM AdequateLeaderTimeoutFloorIsAdequateWithoutMaximumView ==
  /\ ModelConfiguration
  /\ AsyncConfiguration
  /\ ViewDomain = Nat
  => /\ AdequateLeaderTimeoutFloor \in Views
     /\ \A roundView \in Views:
          roundView >= AdequateLeaderTimeoutFloor
            => AsyncViewTimeout(roundView)
                 > AsyncWorstCaseServiceBudget
BY SMTT(30)
   DEF AdequateLeaderTimeoutFloor,
       AsyncConfiguration, AsyncServiceBoundRepresentable,
       AsyncLinearViewTimeout, AsyncViewTimeout, Views

THEOREM AdequateLeaderResponsiveMaximumViewFacts ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncCurrentResponsiveVoters # {}
  => /\ IsFiniteSet(AdequateLeaderResponsiveViewRecords)
     /\ AdequateLeaderResponsiveMaximumViewRecord
          \in AdequateLeaderResponsiveViewRecords
     /\ AdequateLeaderResponsiveMaximumView \in Views
     /\ \A node \in AsyncCurrentResponsiveVoters:
          nodeView[node] <= AdequateLeaderResponsiveMaximumView
BY RuntimeValidatorIdsAreFinite, FS_Subset,
   FiniteIntegerRanksHaveMaximum, IsaT(600)
   DEF AdequateLeaderResponsiveViewRecords,
       AdequateLeaderResponsiveMaximumViewRecord,
       AdequateLeaderResponsiveMaximumView,
       AsyncCurrentResponsiveVoters,
       AsyncStrongTypeInvariant, StrongInductiveInvariant,
       Safety, TypeInvariant

THEOREM AdequateLeaderResponsiveTargetHasSelfViewInOneRosterPeriod ==
  \A target \in AsyncCurrentResponsiveVoters:
    /\ AsyncStrongTypeInvariant
    /\ ViewDomain = Nat
    => /\ AdequateLeaderTargetSelfViewCandidates(target) # {}
       /\ IsFiniteSet(AdequateLeaderTargetSelfViewCandidates(target))
BY AdequateLeaderResponsiveMaximumViewFacts,
   RuntimeValidatorIdsAreFinite, FS_Interval, FS_Subset,
   FS_CardinalityType, IsaT(600)
   DEF AdequateLeaderTargetSelfViewCandidates,
       AdequateLeaderStrictFutureBase,
       AdequateLeaderTimeoutFloor,
       Leader, AsyncCurrentResponsiveVoters,
       CurrentVoters, CurrentEpoch, VotingRoster,
       AsyncStrongTypeInvariant, StrongInductiveInvariant,
       Safety, TypeInvariant, ModelConfiguration,
       QuorumConfiguration, Views

THEOREM AdequateLeaderNextTargetSelfViewFacts ==
  \A target \in AsyncCurrentResponsiveVoters:
    /\ AsyncStrongTypeInvariant
    /\ ViewDomain = Nat
    => LET nextSelf == AdequateLeaderNextTargetSelfView(target)
       IN /\ nextSelf
                \in AdequateLeaderTargetSelfViewCandidates(target)
          /\ nextSelf \in Views
          /\ nextSelf > nodeView[target]
          /\ Leader(context, nextSelf) = target
          /\ nextSelf >= AdequateLeaderTimeoutFloor
          /\ AsyncViewTimeout(nextSelf)
               > AsyncWorstCaseServiceBudget
          /\ AdequateLeaderTargetSelfViewDistance(target)
               \in Nat \ {0}
          /\ \A node \in AsyncCurrentResponsiveVoters:
               nodeView[node] < nextSelf
BY AdequateLeaderResponsiveTargetHasSelfViewInOneRosterPeriod,
   AdequateLeaderResponsiveMaximumViewFacts,
   AdequateLeaderTimeoutFloorIsAdequateWithoutMaximumView,
   IsaT(600)
   DEF AdequateLeaderNextTargetSelfView,
       AdequateLeaderTargetSelfViewCandidates,
       AdequateLeaderStrictFutureBase,
       AdequateLeaderTargetSelfViewDistance,
       AdequateLeaderResponsiveMaximumView

THEOREM AdequateLeaderFrozenResidentDebtIsFinite ==
  \A target \in ValidatorIds,
     residentTcs \in SUBSET TcRecordSet,
     residentOrigins \in SUBSET TimeoutVoteRecordSet:
    /\ IsFiniteSet(residentTcs)
    /\ IsFiniteSet(residentOrigins)
    => /\ IsFiniteSet(
             AdequateLeaderResidentFutureTcDebt(target, residentTcs))
       /\ IsFiniteSet(
             AdequateLeaderResidentFutureOriginDebt(
               target, residentOrigins))
BY FS_Subset, Isa
   DEF AdequateLeaderResidentFutureTcDebt,
       AdequateLeaderResidentFutureOriginDebt

THEOREM AdequateLeaderFrozenResidentRankIsInCarrier ==
  \A target \in AsyncCurrentResponsiveVoters,
     residentTcs \in SUBSET TcRecordSet,
     residentOrigins \in SUBSET TimeoutVoteRecordSet:
    /\ AsyncStrongTypeInvariant
    /\ ViewDomain = Nat
    /\ IsFiniteSet(residentTcs)
    /\ IsFiniteSet(residentOrigins)
    => AdequateLeaderViewExposureRank(
         target, residentTcs, residentOrigins)
           \in AdequateLeaderViewExposureRankCarrier
BY AdequateLeaderNextTargetSelfViewFacts,
   AdequateLeaderFrozenResidentDebtIsFinite,
   FS_CardinalityType, Isa
   DEF AdequateLeaderViewExposureRank,
       AdequateLeaderViewExposureRankCarrier,
       AdequateLeaderResidentFutureDebt,
       AdequateLeaderTargetSelfViewDistance,
       AsyncStrongTypeInvariant, StrongInductiveInvariant,
       Safety, TypeInvariant

THEOREM AdequateLeaderFrozenSourceStartsConcreteViewRank ==
  \A target \in ValidatorIds,
     residentTcs \in SUBSET TcRecordSet,
     residentOrigins \in SUBSET TimeoutVoteRecordSet:
    /\ AsyncStrongTypeInvariant
    /\ ViewDomain = Nat
    /\ AdequateLeaderFrozenViewEpisodeSource(
         target, residentTcs, residentOrigins)
    /\ ~AdequateLeaderTargetFreshSelfCorridorGoal(target)
    => \E rank \in AdequateLeaderViewExposureRankCarrier:
         AdequateLeaderViewExposureRankFrontier(
           target, residentTcs, residentOrigins, rank)
BY AdequateLeaderFrozenResidentRankIsInCarrier, Isa
   DEF AdequateLeaderFrozenViewEpisodeSource,
       AdequateLeaderViewExposureRankFrontier,
       AdequateLeaderLocalTargetDecisionSource,
       AsyncCurrentResponsiveVoters,
       CurrentVoters, CurrentEpoch

THEOREM AdequateLeaderAuthenticatedTcEpisodeOwnerSetIsFinite ==
  \A target \in ValidatorIds, tc \in TcRecordSet:
    IsFiniteSet(
      AdequateLeaderAuthenticatedTcEpisodePhysicalOwners(target, tc))
BY RuntimeValidatorIdsAreFinite, FS_Interval,
   FS_Product, FS_Subset, Isa
   DEF AdequateLeaderAuthenticatedTcEpisodePhysicalOwners,
       AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch

THEOREM AdequateLeaderAuthenticatedTcEpisodeStageRankFacts ==
  \A target \in ValidatorIds,
     roundView \in Views, tc \in TcRecordSet:
    /\ AsyncStrongTypeInvariant
    /\ TimeoutCertificateSemanticIdentity(tc, roundView)
    /\ AdequateLeaderAuthenticatedTcEpisodeStageSet(target, tc) # {}
    => /\ IsFiniteSet(
             AdequateLeaderAuthenticatedTcEpisodeStageSet(target, tc))
       /\ AdequateLeaderAuthenticatedTcEpisodeStageRank(target, tc)
            \in 1..7
BY AdequateLeaderAuthenticatedTcEpisodeOwnerSetIsFinite,
   FS_Interval, FS_Subset, Isa
   DEF AdequateLeaderAuthenticatedTcEpisodeStageSet,
       AdequateLeaderAuthenticatedTcEpisodeStageRank,
       AsyncCurrentResponsiveVoters,
       CurrentVoters, CurrentEpoch,
       AsyncStrongTypeInvariant, StrongInductiveInvariant,
       Safety, TypeInvariant

THEOREM AdequateLeaderTcSourceInstallStagePersistsOrHandsToRetained ==
  \A target \in ValidatorIds, tc \in TcRecordSet:
    /\ AsyncStrongTypeInvariant
    /\ gst
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ TimeoutImportedCertificateReducerTailInvariant
    /\ AdequateLeaderAuthenticatedTcEpisodeOwnsStage(target, tc, 7)
    /\ [AsyncNext]_AsyncAllVars
    => AdequateLeaderAuthenticatedTcEpisodeBoundaryHandoff(target, tc, 7)
BY AsyncBracketPreservesTimeoutImportedCertificateReducerTail,
   ExecuteExactPersistInstallReachesMinimumView,
   ExecutePersistInstallRetainsExactInstalledTcAuthority,
   AsyncCandidateScheduledIdentityDepartureRetiresLifecycleAtGst,
   IsaT(1800)
   DEF AdequateLeaderAuthenticatedTcEpisodeOwnsStage,
       AdequateLeaderAuthenticatedTcEpisodeStageSet,
       AdequateLeaderAuthenticatedTcEpisodePhysicalOwners,
       AdequateLeaderAuthenticatedTcEpisodeBoundaryHandoff,
       AdequateLeaderAuthenticatedTcEpisodeRetired,
       TimeoutTcInstallWalOwner,
       TimeoutTcRetainedControlOwner,
       TimeoutTcExactPersistCandidateOwner,
       TimeoutDirectGoal, TimeoutViewGoal,
       AsyncNext, AsyncAllVars

THEOREM AdequateLeaderTcRetainedStagePersistsOrHandsToPacket ==
  \A target \in ValidatorIds, tc \in TcRecordSet:
    /\ AsyncStrongTypeInvariant
    /\ gst
    /\ AsyncProgressOwnershipInvariant
    /\ AdequateLeaderAuthenticatedTcEpisodeOwnsStage(target, tc, 6)
    /\ [AsyncNext]_AsyncAllVars
    => AdequateLeaderAuthenticatedTcEpisodeBoundaryHandoff(target, tc, 6)
BY TimeoutPhysicalControlRetransmissionCreatesExactPacket,
   GstAsyncStepIsMonotone, IsaT(1200)
   DEF AdequateLeaderAuthenticatedTcEpisodeOwnsStage,
       AdequateLeaderAuthenticatedTcEpisodeStageSet,
       AdequateLeaderAuthenticatedTcEpisodePhysicalOwners,
       AdequateLeaderAuthenticatedTcEpisodeBoundaryHandoff,
       AdequateLeaderAuthenticatedTcEpisodeRetired,
       TimeoutTcRetainedControlOwner,
       TimeoutTcPacketOwner,
       TimeoutPhysicalControlRetainedOwner,
       TimeoutPhysicalControlPacketOwner,
       TimeoutCertificateItem, AsyncNext, AsyncAllVars

THEOREM AdequateLeaderTcPacketStagePersistsOrHandsToIngress ==
  \A target \in ValidatorIds, tc \in TcRecordSet:
    /\ AsyncStrongTypeInvariant
    /\ gst
    /\ AsyncProgressOwnershipInvariant
    /\ AdequateLeaderAuthenticatedTcEpisodeOwnsStage(target, tc, 5)
    /\ [AsyncNext]_AsyncAllVars
    => AdequateLeaderAuthenticatedTcEpisodeBoundaryHandoff(target, tc, 5)
BY TimeoutPhysicalControlPacketAdmissionPreservesExactHandoff,
   GstAsyncStepIsMonotone, IsaT(1500)
   DEF AdequateLeaderAuthenticatedTcEpisodeOwnsStage,
       AdequateLeaderAuthenticatedTcEpisodeStageSet,
       AdequateLeaderAuthenticatedTcEpisodePhysicalOwners,
       AdequateLeaderAuthenticatedTcEpisodeBoundaryHandoff,
       AdequateLeaderAuthenticatedTcEpisodeRetired,
       TimeoutTcPacketOwner, TimeoutTcIngressOwner,
       TimeoutPhysicalControlPacketOwner,
       TimeoutPhysicalControlIngressOwner,
       TimeoutCertificateItem, AsyncNext, AsyncAllVars

THEOREM AdequateLeaderTcIngressStagePersistsOrHandsToCandidate ==
  \A target \in ValidatorIds, tc \in TcRecordSet:
    /\ AsyncStrongTypeInvariant
    /\ gst
    /\ AsyncProgressOwnershipInvariant
    /\ AdequateLeaderAuthenticatedTcEpisodeOwnsStage(target, tc, 4)
    /\ [AsyncNext]_AsyncAllVars
    => AdequateLeaderAuthenticatedTcEpisodeBoundaryHandoff(target, tc, 4)
BY TimeoutPhysicalControlIngressDrainPreservesExactHandoff,
   GstAsyncStepIsMonotone, IsaT(1500)
   DEF AdequateLeaderAuthenticatedTcEpisodeOwnsStage,
       AdequateLeaderAuthenticatedTcEpisodeStageSet,
       AdequateLeaderAuthenticatedTcEpisodePhysicalOwners,
       AdequateLeaderAuthenticatedTcEpisodeBoundaryHandoff,
       AdequateLeaderAuthenticatedTcEpisodeRetired,
       TimeoutTcIngressOwner, TimeoutTcReducerCandidateOwner,
       TimeoutPhysicalControlIngressOwner,
       TimeoutPhysicalControlCandidateOwner,
       TimeoutCertificateItem, AsyncNext, AsyncAllVars

THEOREM AdequateLeaderTcCandidateStagePersistsOrHandsToReceipt ==
  \A target \in ValidatorIds, tc \in TcRecordSet:
    /\ AsyncStrongTypeInvariant
    /\ gst
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ AdequateLeaderAuthenticatedTcEpisodeOwnsStage(target, tc, 3)
    /\ [AsyncNext]_AsyncAllVars
    => AdequateLeaderAuthenticatedTcEpisodeBoundaryHandoff(target, tc, 3)
BY TimeoutTcDeliveryCandidatePersistsOrReachesExactOwner,
   ExecuteExactTimeoutCertificateDeliveryCreatesInstallOrGoal,
   IsaT(1800)
   DEF AdequateLeaderAuthenticatedTcEpisodeOwnsStage,
       AdequateLeaderAuthenticatedTcEpisodeStageSet,
       AdequateLeaderAuthenticatedTcEpisodePhysicalOwners,
       AdequateLeaderAuthenticatedTcEpisodeBoundaryHandoff,
       AdequateLeaderAuthenticatedTcEpisodeRetired,
       TimeoutTcReducerCandidateOwner,
       TimeoutTcReceivedReducerOwner,
       TimeoutTcInstallWalOwner,
       TimeoutDirectGoal, TimeoutViewGoal,
       AsyncNext, AsyncAllVars

THEOREM AdequateLeaderTcReceiptStagePersistsOrHandsToPersist ==
  \A target \in ValidatorIds, tc \in TcRecordSet:
    /\ AsyncStrongTypeInvariant
    /\ gst
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ TimeoutImportedCertificateReducerTailInvariant
    /\ AdequateLeaderAuthenticatedTcEpisodeOwnsStage(target, tc, 2)
    /\ [AsyncNext]_AsyncAllVars
    => AdequateLeaderAuthenticatedTcEpisodeBoundaryHandoff(target, tc, 2)
BY AsyncBracketPreservesTimeoutImportedCertificateReducerTail,
   ExecuteExactBeginInstallCreatesExactWalOwner,
   IsaT(1800)
   DEF AdequateLeaderAuthenticatedTcEpisodeOwnsStage,
       AdequateLeaderAuthenticatedTcEpisodeStageSet,
       AdequateLeaderAuthenticatedTcEpisodePhysicalOwners,
       AdequateLeaderAuthenticatedTcEpisodeBoundaryHandoff,
       AdequateLeaderAuthenticatedTcEpisodeRetired,
       TimeoutTcReceivedReducerOwner,
       TimeoutTcImportedBeginCandidateOwner,
       TimeoutTcInstallWalOwner,
       TimeoutTcExactPersistCandidateOwner,
       TimeoutDirectGoal, TimeoutViewGoal,
       AsyncNext, AsyncAllVars

THEOREM AdequateLeaderTcPersistStagePersistsOrReachesView ==
  \A target \in ValidatorIds, tc \in TcRecordSet:
    /\ AsyncStrongTypeInvariant
    /\ gst
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ TimeoutImportedCertificateReducerTailInvariant
    /\ AdequateLeaderAuthenticatedTcEpisodeOwnsStage(target, tc, 1)
    /\ [AsyncNext]_AsyncAllVars
    => AdequateLeaderAuthenticatedTcEpisodeBoundaryHandoff(target, tc, 1)
BY AsyncBracketPreservesTimeoutImportedCertificateReducerTail,
   ExecuteExactPersistInstallReachesMinimumView,
   AsyncCandidateScheduledIdentityDepartureRetiresLifecycleAtGst,
   IsaT(1800)
   DEF AdequateLeaderAuthenticatedTcEpisodeOwnsStage,
       AdequateLeaderAuthenticatedTcEpisodeStageSet,
       AdequateLeaderAuthenticatedTcEpisodePhysicalOwners,
       AdequateLeaderAuthenticatedTcEpisodeBoundaryHandoff,
       AdequateLeaderAuthenticatedTcEpisodeRetired,
       TimeoutTcInstallWalOwner,
       TimeoutTcExactPersistCandidateOwner,
       TimeoutDirectGoal, TimeoutViewGoal,
       AsyncNext, AsyncAllVars

THEOREM AdequateLeaderExactTcMinimumStageCannotIncreaseWithoutExit ==
  \A target \in ValidatorIds,
     residentTcs \in SUBSET TcRecordSet,
     residentOrigins \in SUBSET TimeoutVoteRecordSet,
     sourceRank \in AdequateLeaderViewExposureRankCarrier,
     roundView \in Views,
     tc \in TcRecordSet,
     budget \in 1..7:
    /\ AsyncStrongTypeInvariant
    /\ AsyncStrongTypeInvariant'
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ TimeoutImportedCertificateReducerTailInvariant
    /\ AdequateLeaderAuthenticatedTcEpisodeAtBudget(
         target, residentTcs, residentOrigins,
         sourceRank, roundView, tc, budget)
    /\ [AsyncNext]_AsyncAllVars
    /\ ~AdequateLeaderViewExposureStrictDescentGoal(
         target, residentTcs, residentOrigins, sourceRank)'
    => /\ AdequateLeaderAuthenticatedTcEpisodeStageSet(target, tc)' # {}
       /\ AdequateLeaderAuthenticatedTcEpisodeStageRank(target, tc)'
            <= budget
BY AdequateLeaderTcSourceInstallStagePersistsOrHandsToRetained,
   AdequateLeaderTcRetainedStagePersistsOrHandsToPacket,
   AdequateLeaderTcPacketStagePersistsOrHandsToIngress,
   AdequateLeaderTcIngressStagePersistsOrHandsToCandidate,
   AdequateLeaderTcCandidateStagePersistsOrHandsToReceipt,
   AdequateLeaderTcReceiptStagePersistsOrHandsToPersist,
   AdequateLeaderTcPersistStagePersistsOrReachesView,
   AdequateLeaderAuthenticatedTcEpisodeStageRankFacts,
   IsaT(1200)
   DEF AdequateLeaderAuthenticatedTcEpisodeAtBudget,
       AdequateLeaderAuthenticatedTcOriginEpisode,
       AdequateLeaderAuthenticatedTcEpisodePhysicalOwners,
       AdequateLeaderAuthenticatedTcEpisodeStageSet,
       AdequateLeaderAuthenticatedTcEpisodeStageRank,
       AdequateLeaderViewExposureStrictDescentGoal,
       TimeoutTcRetainedControlOwner,
       TimeoutTcPacketOwner, TimeoutTcIngressOwner,
       TimeoutTcReducerCandidateOwner,
       TimeoutTcReceivedReducerOwner,
       TimeoutTcImportedBeginCandidateOwner,
       TimeoutTcInstallWalOwner,
       TimeoutTcExactPersistCandidateOwner,
       TimeoutDirectGoal, TimeoutViewGoal,
       ResponsiveProtectedCandidateOwned,
       ProtectedCandidateOwned, ProtectedServiceCandidate,
       CandidateConsumerCurrent, CandidateScheduled,
       AsyncNext, AsyncNonCrashStep, AsyncRunnerStep,
       AsyncNonRunnerStep, AsyncAllVars

THEOREM AdequateLeaderSameStageEqualCountReplacementRetainsEpisode ==
  \A target \in ValidatorIds,
     residentTcs \in SUBSET TcRecordSet,
     residentOrigins \in SUBSET TimeoutVoteRecordSet,
     sourceRank \in AdequateLeaderViewExposureRankCarrier,
     roundView \in Views,
     tc \in TcRecordSet,
     budget \in 1..7:
    /\ AsyncStrongTypeInvariant
    /\ AsyncStrongTypeInvariant'
    /\ AdequateLeaderAuthenticatedTcEpisodeSameStageReplacement(
         target, residentTcs, residentOrigins,
         sourceRank, roundView, tc, budget)
    => /\ AdequateLeaderAuthenticatedTcEpisodeAtBudget(
             target, residentTcs, residentOrigins,
             sourceRank, roundView, tc, budget)'
       /\ ~AdequateLeaderViewExposureStrictDescentGoal(
             target, residentTcs, residentOrigins, sourceRank)'
BY PostGstAsyncBracketAdvancesEveryNodeView,
   AdequateLeaderExactTcMinimumStageCannotIncreaseWithoutExit,
   IsaT(900)
   DEF AdequateLeaderAuthenticatedTcEpisodeSameStageReplacement,
       AdequateLeaderAuthenticatedTcEpisodeAtBudget,
       AdequateLeaderAuthenticatedTcOriginEpisode,
       AdequateLeaderAuthenticatedTcEpisodeIdentity,
       AdequateLeaderPostSnapshotResponsiveOrigin,
       AdequateLeaderViewExposureRankFrontier,
       AdequateLeaderLocalTargetDecisionSource,
       TimeoutCertificateSemanticIdentity,
       TimeoutVoteSemanticIdentity, TimeoutOrigin,
       AsyncAllVars

THEOREM AdequateLeaderRetiredExactTcEpisodeCannotResurrectStep ==
  \A target \in ValidatorIds,
     residentTcs \in SUBSET TcRecordSet,
     residentOrigins \in SUBSET TimeoutVoteRecordSet,
     roundView \in Views,
     tc \in TcRecordSet:
    /\ AsyncStrongTypeInvariant
    /\ gst
    /\ AdequateLeaderDurableExactTcEpisodeRetired(target, tc)
    /\ [AsyncNext]_AsyncAllVars
    => /\ (AdequateLeaderDurableExactTcEpisodeRetired(target, tc))'
       /\ ~(AdequateLeaderAuthenticatedTcOriginEpisode(
              target, residentTcs, residentOrigins, roundView, tc))'
BY AdequateLeaderAsyncBracketStepPreservesTargetDecision,
   PostGstAsyncBracketAdvancesEveryNodeView, Isa
   DEF AdequateLeaderDurableExactTcEpisodeRetired,
       AdequateLeaderAuthenticatedTcEpisodeRetired,
       AdequateLeaderAuthenticatedTcOriginEpisode,
       AdequateLeaderLocalTargetDecisionSource

THEOREM AsyncLiveRetiredExactTcEpisodeCannotResurrect ==
  \A initialContext:
    AdequateLeaderAuthenticatedTcNoResurrectionProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncLiveSpecProjectsAsyncSpec,
   AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecKeepsGstOnceSet,
   AdequateLeaderRetiredExactTcEpisodeCannotResurrectStep,
   PTL
   DEF AdequateLeaderAuthenticatedTcNoResurrectionProperty,
       AdequateLeaderDurableExactTcEpisodeRetired,
       AdequateLeaderAuthenticatedTcEpisodeRetired,
       AdequateLeaderAuthenticatedTcOriginEpisode

THEOREM AdequateLeaderSameRoundUpgradeBudgetIsNatural ==
  \A target \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ AdequateLeaderSameRoundUpgradeOwnerSet(target) # {}
      => AdequateLeaderSameRoundUpgradeRemainingBudget(target) \in Nat
BY Isa
   DEF AdequateLeaderSameRoundUpgradeRemainingBudget,
       AdequateLeaderSameRoundUpgradeOwnerSet,
       TimeoutCertificateDelivery,
       TimeoutCertificateInstallOwner,
       StrictSameRoundTcUpgrade, TcHighRank,
       AsyncStrongTypeInvariant, StrongInductiveInvariant,
       Safety, TypeInvariant, ReducerProvenanceInvariant,
       FormedTimeoutCertificatesSound,
       TCMaximumProtectsReports, Ranks, Views, NoRank

THEOREM AdequateLeaderSameRoundPersistStrictlyLowersUpgradeBudget ==
  \A target \in ValidatorIds,
     tc \in TcRecordSet,
     command:
    /\ AsyncStrongTypeInvariant
    /\ StrictSameRoundTcUpgrade(target, tc)
    /\ ExactPersistInstallTcCommand(target, tc, command)
    /\ ExecutePersistInstall(command)
    => /\ nodeView'[target] = nodeView[target]
       /\ AdequateLeaderSameRoundUpgradeRemainingBudget(target)'
            < AdequateLeaderSameRoundUpgradeRemainingBudget(target)
BY ValidInstallSelectedRankDoesNotExceedTcView, IsaT(600)
   DEF AdequateLeaderSameRoundUpgradeRemainingBudget,
       ExactPersistInstallTcCommand,
       ExecutePersistInstall, PersistInstallTC,
       StrictSameRoundTcUpgrade, TcHighRank,
       AsyncStrongTypeInvariant, StrongInductiveInvariant,
       Safety, TypeInvariant, ReducerProvenanceInvariant,
       PendingCertificateWritesAuthorized,
       Ranks, Views, NoRank

THEOREM AsyncLiveProvidesSameRoundUpgradeBudgetDescent ==
  \A initialContext:
    AdequateLeaderSameRoundUpgradeBudgetDescentProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncLiveSpecProjectsAsyncSpec,
   AsyncSpecAlwaysStrongTypeInvariant,
   TimeoutViewOwnershipKernelInvariantFromAsyncSpec,
   AsyncLiveProvidesDirectTimeoutViewClosureResidual,
   TimeoutTcPhysicalKernelsDischargeFrontier,
   InstallRankInvariantExcludesGenerationExhaustion,
   AdequateLeaderSameRoundUpgradeBudgetIsNatural,
   AdequateLeaderSameRoundPersistStrictlyLowersUpgradeBudget,
   PostGstAsyncBracketAdvancesEveryNodeView,
   AsyncSpecKeepsGstOnceSet,
   PTL, IsaT(2400)
   DEF AdequateLeaderSameRoundUpgradeBudgetDescentProperty,
       AdequateLeaderSameRoundUpgradeEpisodeAtBudget,
       AdequateLeaderSameRoundUpgradeEpisodeExitGoal,
       AdequateLeaderSameRoundUpgradeOwnerSet,
       AdequateLeaderSameRoundUpgradeRemainingBudget,
       TimeoutTcPhysicalKernelProperties,
       TimeoutCertificateDelivery,
       TimeoutCertificateInstallOwner,
       TimeoutViewGoal, TimeoutDirectGoal

THEOREM AdequateLeaderFiniteSameRoundUpgradeEpisodeCloses ==
  \A specification:
    AdequateLeaderSameRoundUpgradeBudgetDescentProperty(specification)
      => AdequateLeaderSameRoundUpgradeEpisodeClosureProperty(
           specification)
BY NatLessThanWellFounded, WellFoundedLeadsTo
   DEF AdequateLeaderSameRoundUpgradeBudgetDescentProperty,
       AdequateLeaderSameRoundUpgradeEpisodeClosureProperty,
       AdequateLeaderSameRoundUpgradeEpisodeExitGoal

(***************************************************************************
Concrete formation and fresh-install boundaries.

The first theorem classifies the action which adds a previously absent TC.
`formedTCs` contains only reducer-formed certificates.  Intersecting that
certificate's dual quorum with the configured responsive dual quorum selects
an honest responsive vote.  Formed-certificate soundness ties that vote to
the immutable timeout intent; timeout ownership then supplies its exact
`TimeoutOrigin`.  If the intent was not in the frozen source set, this is the
required post-snapshot producer episode.

The second theorem is the only place this module depends on the atomic
timeout-lifecycle retirement performed by `PersistInstallTC`.  It uses the
complete `AsyncNext` wrapper, not bare Core persistence: FIFO/deferred/causal,
I/O, tag, and emitted-timeout owners are retired in the same transition.
Only a strict view advance is admitted here; a same-round Prepare-rank upgrade
is not claimed to create a new-view window.
***************************************************************************)

THEOREM AdequateLeaderFirstPostSnapshotTcHasConcreteResponsiveOrigin ==
  \A residentTcs \in SUBSET TcRecordSet,
     residentOrigins \in SUBSET TimeoutVoteRecordSet,
     tc \in TcRecordSet:
    /\ AsyncStrongTypeInvariant
    /\ TimeoutViewOwnershipInvariant
    /\ gst
    /\ [AsyncNext]_AsyncAllVars
    /\ AdequateLeaderFirstPostSnapshotTcOccurrence(residentTcs, tc)
    => \E vote \in tc.votes:
         /\ vote.signer \in AsyncCurrentResponsiveVoters
         /\ TimeoutVoteSemanticIdentity(vote.signer, tc.view, vote)
         /\ \/ vote \in residentOrigins
            \/ AdequateLeaderPostSnapshotResponsiveOrigin(
                 residentOrigins, tc.view, vote)'
BY DualQuorumIntersectionHasHonest,
   ExecuteTimeoutFormingReducerCreatesExactInstallOwner,
   TimeoutViewOwnershipKernelProjectsPublicInvariant,
   ChangedAsyncNextExecutesCommandOrCrashes,
   GstAsyncStepIsMonotone, IsaT(2400)
   DEF AdequateLeaderFirstPostSnapshotTcOccurrence,
       AdequateLeaderPostSnapshotResponsiveOrigin,
       TimeoutCertificateSemanticIdentity,
       TimeoutVoteSemanticIdentity, TimeoutOrigin,
       AsyncStrongTypeInvariant, StrongInductiveInvariant,
       Safety, TypeInvariant, FormedTimeoutCertificatesSound,
       ModelConfiguration, TCValid, TimeoutSignerSet,
       AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch,
       ExecuteCommand, ExecuteSignTimeout, ExecuteCoreDelivery,
       CompleteTimeoutSignature, DeliverTimeout,
       AsyncNext, AsyncNonCrashStep, AsyncRunnerStep,
       AsyncNonRunnerStep, AsyncAllVars

THEOREM AdequateLeaderStrictPersistInstallStartsExactTcSynchronization ==
  \A command:
    /\ AsyncStrongTypeInvariant
    /\ TimeoutViewOwnershipInvariant
    /\ gst
    /\ command.node \in AsyncCurrentResponsiveVoters
    /\ command.view >= nodeView[command.node]
    /\ Leader(context, command.view + 1) = command.node
    /\ AsyncViewTimeout(command.view + 1)
         > AsyncWorstCaseServiceBudget
    /\ \A node \in
          AsyncCurrentResponsiveVoters \ {command.node}:
         nodeView[node] < command.view + 1
    /\ [AsyncNext]_AsyncAllVars
    /\ AsyncNext
    /\ ExecutePersistInstall(command)
    => \E episodeContext \in ContextRecords,
          roster \in SUBSET ValidatorIds,
          tc \in TcRecordSet,
          firstDeadline \in Nat:
         (AdequateLeaderExactTcSynchronizationSource(
            command.node, episodeContext, roster, tc,
            command.view + 1, asyncNow, firstDeadline))'
BY ExecutePersistInstallAdvancesCertifiedView,
   ExecutePersistInstallRetainsExactInstalledTcAuthority,
   PostGstAsyncBracketAdvancesEveryNodeView,
   GstAsyncStepIsMonotone, IsaT(3600)
   DEF AdequateLeaderExactTcSynchronizationSource,
       AdequateLeaderInstalledSynchronizationPrefixNode,
       AdequateLeaderOlderOrEqualTimeoutLifecycleOwned,
       TimeoutTcRetainedControlOwner,
       TimeoutTcKernelSource, TimeoutCertificateItem,
       TimeoutCertificateSemanticIdentity,
       TcOutbox, AsyncNetworkItem,
       AsyncOlderOrEqualTimeoutLifecycleOwned,
       AsyncTimeoutLifecycleCandidateRetiredBy,
       AsyncTimeoutLifecycleCandidateRetiredThroughInstall,
       AsyncTimeoutLifecycleSequenceAfterInstall,
       AsyncTimeoutLifecycleIoSequenceAfterInstall,
       AsyncTimeoutLifecycleSetAfterInstall,
       AsyncIoTimeoutLifecycleRetirementTransition,
       AsyncControlServiceStateAfterTimeoutRetirement,
       ExecutePersistInstall, PersistInstallTC,
       StrictSameRoundTcUpgrade, NodeTimedOut,
       AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch,
       AsyncStrongTypeInvariant, StrongInductiveInvariant,
       Safety, TypeInvariant, ModelConfiguration,
       AsyncWorstCaseServiceBudget,
       AsyncViewSynchronizationBudget,
       AsyncFixedCorridorServiceBudget,
       AsyncNext, AsyncNonCrashStep, AsyncRunnerStep,
       AsyncNonRunnerStep, RunNode, RunNodeWork,
       SerializedRunnerRuntimeStep, SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       RuntimeStep, FifoRuntimeStep, DeferredDrainStep,
       AsyncAllVars, vars

THEOREM AdequateLeaderExactTcSynchronizationSourceStartsRank ==
  \A target \in ValidatorIds,
     episodeContext \in ContextRecords,
     roster \in SUBSET ValidatorIds,
     tc \in TcRecordSet,
     leaderView \in Views,
     startTime, firstDeadline \in Nat:
    /\ AsyncStrongTypeInvariant
    /\ AdequateLeaderExactTcSynchronizationSource(
         target, episodeContext, roster, tc,
         leaderView, startTime, firstDeadline)
    => \/ NodeHasDecision(target)
       \/ \E rank \in AdequateLeaderSynchronizationRankCarrier:
            AdequateLeaderExactTcSynchronizationAtRank(
              target, episodeContext, roster, tc,
              leaderView, startTime, firstDeadline, rank)
BY AdequateLeaderAuthenticatedTcEpisodeStageRankFacts,
   AdequateLeaderAuthenticatedTcEpisodeOwnerSetIsFinite,
   AsyncServiceBudgetSplitReconstructsWholePipeline,
   RuntimeValidatorIdsAreFinite, FS_Subset, FS_CardinalityType,
   IsaT(1200)
   DEF AdequateLeaderExactTcSynchronizationSource,
       AdequateLeaderExactTcSynchronizationAtRank,
       AdequateLeaderInstalledSynchronizationPrefixNode,
       AdequateLeaderSynchronizationRemainingNodes,
       AdequateLeaderSynchronizationSelectedNode,
       AdequateLeaderSynchronizationSelectedStage,
       AdequateLeaderSynchronizationPhysicalRank,
       AdequateLeaderSynchronizationClockSlack,
       AdequateLeaderSynchronizationRank,
       AdequateLeaderSynchronizationRankCarrier,
       AdequateLeaderSynchronizationOvershot,
       AdequateLeaderAuthenticatedTcEpisodeStageSet,
       AdequateLeaderAuthenticatedTcEpisodePhysicalOwners,
       TimeoutTcRetainedControlOwner,
       AsyncStrongTypeInvariant, StrongInductiveInvariant,
       Safety, TypeInvariant

THEOREM AdequateLeaderExactTcSynchronizationRankStep ==
  \A target \in ValidatorIds,
     episodeContext \in ContextRecords,
     roster \in SUBSET ValidatorIds,
     tc \in TcRecordSet,
     leaderView \in Views,
     startTime, firstDeadline \in Nat,
     sourceRank \in AdequateLeaderSynchronizationRankCarrier:
    /\ AsyncStrongTypeInvariant
    /\ AsyncStrongTypeInvariant'
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ TimeoutImportedCertificateReducerTailInvariant
    /\ AdequateLeaderExactTcSynchronizationAtRank(
         target, episodeContext, roster, tc,
         leaderView, startTime, firstDeadline, sourceRank)
    /\ [AsyncNext]_AsyncAllVars
    => \/ (AdequateLeaderExactTcSynchronizationGoal(
             target, episodeContext, roster, tc, leaderView))'
       \/ \E lowerRank \in SetLessThan(
              sourceRank,
              AdequateLeaderSynchronizationRankOrdering,
              AdequateLeaderSynchronizationRankCarrier):
            (AdequateLeaderExactTcSynchronizationAtRank(
               target, episodeContext, roster, tc,
               leaderView, startTime, firstDeadline, lowerRank))'
BY AdequateLeaderTcSourceInstallStagePersistsOrHandsToRetained,
   AdequateLeaderTcRetainedStagePersistsOrHandsToPacket,
   AdequateLeaderTcPacketStagePersistsOrHandsToIngress,
   AdequateLeaderTcIngressStagePersistsOrHandsToCandidate,
   AdequateLeaderTcCandidateStagePersistsOrHandsToReceipt,
   AdequateLeaderTcReceiptStagePersistsOrHandsToPersist,
   AdequateLeaderTcPersistStagePersistsOrReachesView,
   AdequateLeaderExactTcMinimumStageCannotIncreaseWithoutExit,
   AdequateLeaderSameStageEqualCountReplacementRetainsEpisode,
   AsyncTickStrictlyLowersPredeadlineClockRankOrExits,
   OverdueNodeServiceStopsPostGstClock,
   OverdueIoServiceStopsPostGstClock,
   OverdueResponsivePacketStopsPostGstClock,
   ExecutePersistInstallAdvancesCertifiedView,
   AsyncServiceBudgetSplitReconstructsWholePipeline,
   PostGstAsyncBracketAdvancesEveryNodeView,
   GstAsyncStepIsMonotone, IsaT(4800)
   DEF AdequateLeaderExactTcSynchronizationAtRank,
       AdequateLeaderExactTcSynchronizationGoal,
       AdequateLeaderExactTcSynchronizationDescentGoal,
       AdequateLeaderSynchronizationRemainingNodes,
       AdequateLeaderSynchronizationOvershot,
       AdequateLeaderSynchronizationSelectedNode,
       AdequateLeaderSynchronizationSelectedStage,
       AdequateLeaderSynchronizationPhysicalRank,
       AdequateLeaderSynchronizationClockSlack,
       AdequateLeaderSynchronizationRank,
       AdequateLeaderSynchronizationRankCarrier,
       AdequateLeaderSynchronizationRankOrdering,
       AdequateLeaderAuthenticatedTcEpisodeStageSet,
       AdequateLeaderAuthenticatedTcEpisodeStageRank,
       AdequateLeaderFreshSynchronizedTargetCorridor,
       AdequateLeaderFrozenTargetCorridor,
       AdequateLeaderCorridorAuthorityReceipt,
       AdequateLeaderCorridorAuthorityReceiptValid,
       AdequateLeaderResponsiveViewSynchronized,
       AdequateLeaderFrozenResponsiveRoster,
       AdequateLeaderFreshTargetLeaderServiceWindow,
       AdequateLeaderActiveTargetLeaderServiceWindow,
       AdequateLeaderFreshNodeServiceWindow,
       AdequateLeaderActiveNodeServiceWindow,
       AdequateLeaderOlderOrEqualTimeoutLifecycleOwned,
       AsyncNext, AsyncAllVars

THEOREM AsyncLiveProvidesExactTcSynchronizationRankStep ==
  \A initialContext:
    AdequateLeaderExactTcSynchronizationRankStepProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncLiveSpecProjectsAsyncSpec,
   AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysProgressOwnershipInvariant,
   ExactDecisionAsyncSpecAlwaysCandidateTombstones,
   TimeoutViewOwnershipKernelInvariantFromAsyncSpec,
   AsyncLiveProvidesDirectTimeoutViewClosureResidual,
   TimeoutTcPhysicalKernelsDischargeFrontier,
   AdequateLeaderExactTcSynchronizationRankStep,
   AsyncSpecKeepsGstOnceSet,
   PTL, IsaT(3600)
   DEF AdequateLeaderExactTcSynchronizationRankStepProperty,
       AdequateLeaderExactTcSynchronizationAtRank,
       AdequateLeaderExactTcSynchronizationDescentGoal,
       TimeoutCertificateAndDecisionConvergenceProperty,
       TimeoutTcPhysicalKernelProperties

THEOREM AdequateLeaderExactTcSynchronizationRankOrderingWellFounded ==
  IsWellFoundedOn(
    AdequateLeaderSynchronizationRankOrdering,
    AdequateLeaderSynchronizationRankCarrier)
BY NatLessThanWellFounded, WFLexPairOrdering
   DEF AdequateLeaderSynchronizationRankOrdering,
       AdequateLeaderSynchronizationRankCarrier

THEOREM AdequateLeaderFiniteExactTcSynchronizationCloses ==
  \A specification:
    AdequateLeaderExactTcSynchronizationRankStepProperty(specification)
      => AdequateLeaderExactTcSynchronizationClosureProperty(specification)
BY AdequateLeaderExactTcSynchronizationRankOrderingWellFounded,
   WellFoundedLeadsTo
   DEF AdequateLeaderExactTcSynchronizationRankStepProperty,
       AdequateLeaderExactTcSynchronizationClosureProperty,
       AdequateLeaderExactTcSynchronizationDescentGoal

THEOREM AsyncLiveProvidesExactTcResponsiveRosterSynchronization ==
  \A initialContext:
    AdequateLeaderExactTcSynchronizationClosureProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncLiveProvidesExactTcSynchronizationRankStep,
   AdequateLeaderFiniteExactTcSynchronizationCloses

THEOREM AdequateLeaderFreshSynchronizedCorridorStartsFixedDeadline ==
  \A target \in ValidatorIds,
     leaderContext \in ContextRecords,
     leaderView \in Views:
    AdequateLeaderFreshSynchronizedTargetCorridor(
      target, leaderContext, target, leaderView)
      => AdequateLeaderFixedCorridorDeadlineSource(
           target, leaderContext, leaderView, asyncNow,
           asyncNow + AsyncFixedCorridorServiceBudget + 1)
BY SMT
   DEF AdequateLeaderFixedCorridorDeadlineSource,
       AdequateLeaderFreshSynchronizedTargetCorridor,
       AdequateLeaderFreshNodeServiceWindow,
       AdequateLeaderFrozenResponsiveRoster

\* A synchronized dual quorum statically excludes every already-formed TC at
\* or above its view.  Any valid TC quorum intersects the frozen responsive
\* quorum in an honest signer; formed-certificate soundness maps that signer
\* to a durable timeout intent, while current-intent soundness bounds the vote
\* by the synchronized node view.  Equality would make that node timed out,
\* contradicting its fresh window.
THEOREM AdequateLeaderFreshSynchronizedCorridorHasNoFormedTcAtOrAboveView ==
  \A target \in ValidatorIds,
     leaderContext \in ContextRecords,
     leaderView \in Views,
     tc \in TcRecordSet:
    /\ AsyncStrongTypeInvariant
    /\ AdequateLeaderFreshSynchronizedTargetCorridor(
         target, leaderContext, target, leaderView)
    /\ tc \in formedTCs
    /\ tc.context = leaderContext
    /\ tc.view >= leaderView
    => FALSE
BY DualQuorumIntersectionHasHonest, IsaT(2400)
   DEF AdequateLeaderFreshSynchronizedTargetCorridor,
       AdequateLeaderFrozenTargetCorridor,
       AdequateLeaderCorridorAuthorityReceipt,
       AdequateLeaderCorridorAuthorityReceiptValid,
       AdequateLeaderResponsiveViewSynchronized,
       AdequateLeaderActiveNodeServiceWindow,
       AdequateLeaderFreshNodeServiceWindow,
       AdequateLeaderFrozenResponsiveRoster,
       AsyncStrongTypeInvariant, StrongInductiveInvariant,
       Safety, TypeInvariant, ModelConfiguration,
       ReducerProvenanceInvariant, LineageInvariant,
       FormedTimeoutCertificatesSound, CurrentIntentViewsBound,
       TimeoutVotesBindCertificate, TimeoutSignerSet,
       NodeTimedOut

THEOREM AdequateLeaderFreshSelfCorridorImpliesAdequateViewReached ==
  \A target \in ValidatorIds, leaderView \in Views:
    AdequateLeaderFreshSynchronizedTargetCorridor(
      target, context, target, leaderView)
      => /\ AdequateResponsiveHonestLeaderViewReached
         /\ AdequateLeaderTargetDecisionSource(target)
BY Isa
   DEF AdequateLeaderFreshSynchronizedTargetCorridor,
       AdequateLeaderFrozenTargetCorridor,
       AdequateResponsiveHonestLeaderViewReached,
       AdequateLeaderTargetDecisionSource,
       AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch

THEOREM AdequateLeaderResidentTcSkipStrictlyLowersExposureRank ==
  \A target \in AsyncCurrentResponsiveVoters,
     residentTcs \in SUBSET TcRecordSet,
     residentOrigins \in SUBSET TimeoutVoteRecordSet,
     tc \in residentTcs,
     command:
    /\ AsyncStrongTypeInvariant
    /\ AsyncStrongTypeInvariant'
    /\ ViewDomain = Nat
    /\ IsFiniteSet(residentTcs)
    /\ IsFiniteSet(residentOrigins)
    /\ TimeoutCertificateSemanticIdentity(tc, nodeView[target])
    /\ ExactPersistInstallTcCommand(target, tc, command)
    /\ ExecutePersistInstall(command)
    /\ nodeView'[target]
         > AdequateLeaderNextTargetSelfView(target)
    /\ ~AdequateLeaderTargetFreshSelfCorridorGoal(target)'
    => <<AdequateLeaderViewExposureRank(
           target, residentTcs, residentOrigins)',
         AdequateLeaderViewExposureRank(
           target, residentTcs, residentOrigins)>>
         \in AdequateLeaderViewExposureRankOrdering
BY ExecutePersistInstallAdvancesCertifiedView,
   AdequateLeaderNextTargetSelfViewFacts,
   AdequateLeaderFrozenResidentDebtIsFinite,
   FS_CardinalityType, IsaT(1200)
   DEF AdequateLeaderViewExposureRank,
       AdequateLeaderViewExposureRankCarrier,
       AdequateLeaderViewExposureRankOrdering,
       AdequateLeaderResidentFutureDebt,
       AdequateLeaderResidentFutureTcDebt,
       AdequateLeaderResidentFutureOriginDebt,
       AdequateLeaderTargetSelfViewDistance,
       AdequateLeaderNextTargetSelfView,
       ExactPersistInstallTcCommand,
       TimeoutCertificateSemanticIdentity

THEOREM AdequateLeaderResidentOriginSkipStrictlyLowersExposureRank ==
  \A target \in AsyncCurrentResponsiveVoters,
     residentTcs \in SUBSET TcRecordSet,
     residentOrigins \in SUBSET TimeoutVoteRecordSet,
     tc \in TcRecordSet,
     vote \in residentOrigins,
     command:
    /\ AsyncStrongTypeInvariant
    /\ AsyncStrongTypeInvariant'
    /\ ViewDomain = Nat
    /\ IsFiniteSet(residentTcs)
    /\ IsFiniteSet(residentOrigins)
    /\ tc \notin residentTcs
    /\ vote \in tc.votes
    /\ vote.signer \in AsyncCurrentResponsiveVoters
    /\ TimeoutVoteSemanticIdentity(vote.signer, tc.view, vote)
    /\ TimeoutCertificateSemanticIdentity(tc, nodeView[target])
    /\ ExactPersistInstallTcCommand(target, tc, command)
    /\ ExecutePersistInstall(command)
    /\ nodeView'[target]
         > AdequateLeaderNextTargetSelfView(target)
    /\ ~AdequateLeaderTargetFreshSelfCorridorGoal(target)'
    => <<AdequateLeaderViewExposureRank(
           target, residentTcs, residentOrigins)',
         AdequateLeaderViewExposureRank(
           target, residentTcs, residentOrigins)>>
         \in AdequateLeaderViewExposureRankOrdering
BY ExecutePersistInstallAdvancesCertifiedView,
   AdequateLeaderNextTargetSelfViewFacts,
   AdequateLeaderFrozenResidentDebtIsFinite,
   FS_CardinalityType, IsaT(1200)
   DEF AdequateLeaderViewExposureRank,
       AdequateLeaderViewExposureRankCarrier,
       AdequateLeaderViewExposureRankOrdering,
       AdequateLeaderResidentFutureDebt,
       AdequateLeaderResidentFutureTcDebt,
       AdequateLeaderResidentFutureOriginDebt,
       AdequateLeaderTargetSelfViewDistance,
       AdequateLeaderNextTargetSelfView,
       ExactPersistInstallTcCommand,
       TimeoutCertificateSemanticIdentity,
       TimeoutVoteSemanticIdentity

THEOREM AsyncLiveAuthenticatedTcCatchupCannotOvertakeFreshSelfWindow ==
  \A initialContext:
    AdequateLeaderAuthenticatedTcNoOvertakeProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncLiveSpecProjectsAsyncSpec,
   AsyncSpecAlwaysStrongTypeInvariant,
   TimeoutViewOwnershipKernelInvariantFromAsyncSpec,
   AsyncLiveProvidesTimeoutFixedClockLifecyclePhysicalKernels,
   TimeoutFixedClockPhysicalKernelsDischargeLifecycleService,
   TimeoutPredeadlineRankDescentClosesDeadlineClock,
   AsyncLiveProvidesDirectTimeoutViewClosureResidual,
   TimeoutVotePhysicalKernelsDischargeSourceIsolatedDelivery,
   TimeoutTcPhysicalKernelsDischargeFrontier,
   TimeoutTcPhysicalKernelsDischargeFormationFrontier,
   AsyncLiveProvidesSameRoundUpgradeBudgetDescent,
   AdequateLeaderFiniteSameRoundUpgradeEpisodeCloses,
   AdequateLeaderNextTargetSelfViewFacts,
   AdequateLeaderStrictPersistInstallStartsExactTcSynchronization,
   AdequateLeaderExactTcSynchronizationSourceStartsRank,
   AsyncLiveProvidesExactTcResponsiveRosterSynchronization,
   AsyncTickStrictlyLowersPredeadlineClockRankOrExits,
   TimeoutPhysicalControlTickLowersRetainedClockRank,
   PostGstAsyncBracketAdvancesEveryNodeView,
   AsyncSpecKeepsGstOnceSet,
   PTL, IsaT(7200)
   DEF AdequateLeaderAuthenticatedTcNoOvertakeProperty,
       AdequateLeaderAuthenticatedTcOvertakeSource,
       AdequateLeaderAuthenticatedTcOriginEpisode,
       AdequateLeaderViewExposureRankFrontier,
       AdequateLeaderViewExposureStrictDescentGoal,
       AdequateLeaderTargetFreshSelfCorridorGoal,
       AdequateLeaderExactTcSynchronizationSource,
       AdequateLeaderExactTcSynchronizationAtRank,
       AdequateLeaderExactTcSynchronizationGoal,
       AdequateLeaderExactTcSynchronizationClosureProperty,
       AdequateLeaderNextTargetSelfView,
       AdequateLeaderTargetSelfViewDistance,
       AdequateLeaderPostSnapshotResponsiveOrigin,
       AdequateLeaderSameRoundUpgradeBudgetDescentProperty,
       AdequateLeaderSameRoundUpgradeEpisodeClosureProperty,
       TimeoutFixedClockLifecycleOwnerServiceProperty,
       TimeoutFixedClockServicePrerequisites,
       TimeoutDeadlineClockConvergenceProperty,
       TimeoutPhysicalControlTransportKernelProperties,
       TimeoutCertificateAndDecisionConvergenceProperty,
       DirectTimeoutViewClosureResidualProperty,
       TimeoutSourceIsolatedDeliveryConvergenceProperty,
       TimeoutCertificateSemanticIdentity,
       TimeoutViewGoal, TimeoutDirectGoal, TcFrontier,
       AsyncTickEnabled, AsyncTick,
       OverdueResponsivePackets,
       AsyncWorstCaseServiceBudget,
       AsyncViewTimeout

(***************************************************************************
Temporal origin-episode and outer-rank closure.

The stage descent theorem consumes only direct timeout providers: deadline
clock, retained timeout origin, exact vote delivery, receipt/TC formation,
exact TC transport, and the imported Begin/Persist tail.  The episode freezes
the exact `(target, context, round, TC)` identity.  Its state-dependent stage
is the minimum live physical owner: source install, retained, packet, ingress,
delivery candidate, target receipt/Begin, and target Persist.  Retransmission
may add another owner for the same identity but cannot replace a lower stage.
The adequate
timeout inequality makes the target's strict install into its selected
self-view win before a later newly-created TC can overtake that view.  A TC
already capable of overtaking it is therefore charged to the frozen resident
TC/origin stock.  Packet creation or a new TC is never an endpoint.
***************************************************************************)

THEOREM AsyncLiveProvidesAuthenticatedTcEpisodeBudgetDescent ==
  \A initialContext:
    AdequateLeaderAuthenticatedTcEpisodeBudgetDescentProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncLiveSpecProjectsAsyncSpec,
   AsyncSpecAlwaysStrongTypeInvariant,
   TimeoutViewOwnershipKernelInvariantFromAsyncSpec,
   AsyncLiveProvidesDirectTimeoutViewClosureResidual,
   TimeoutVotePhysicalKernelsDischargeSourceIsolatedDelivery,
   TimeoutTcPhysicalKernelsDischargeFrontier,
   TimeoutTcPhysicalKernelsDischargeFormationFrontier,
   DirectTimeoutViewDecompositionClosesTimeoutViewProgress,
   AsyncLiveProvidesSameRoundUpgradeBudgetDescent,
   AdequateLeaderFiniteSameRoundUpgradeEpisodeCloses,
   AdequateLeaderExactTcMinimumStageCannotIncreaseWithoutExit,
   AdequateLeaderSameStageEqualCountReplacementRetainsEpisode,
   AsyncLiveRetiredExactTcEpisodeCannotResurrect,
   AdequateLeaderFirstPostSnapshotTcHasConcreteResponsiveOrigin,
   AdequateLeaderStrictPersistInstallStartsExactTcSynchronization,
   AdequateLeaderExactTcSynchronizationSourceStartsRank,
   AsyncLiveProvidesExactTcResponsiveRosterSynchronization,
   AdequateLeaderResidentTcSkipStrictlyLowersExposureRank,
   AdequateLeaderResidentOriginSkipStrictlyLowersExposureRank,
   AsyncLiveAuthenticatedTcCatchupCannotOvertakeFreshSelfWindow,
   AdequateLeaderNextTargetSelfViewFacts,
   PostGstAsyncBracketAdvancesEveryNodeView,
   AsyncSpecKeepsGstOnceSet,
   PTL, IsaT(7200)
   DEF AdequateLeaderAuthenticatedTcEpisodeBudgetDescentProperty,
       AdequateLeaderAuthenticatedTcEpisodeAtBudget,
       AdequateLeaderAuthenticatedTcOriginEpisode,
       AdequateLeaderAuthenticatedTcEpisodeExitGoal,
       AdequateLeaderAuthenticatedTcEpisodeIdentity,
       AdequateLeaderAuthenticatedTcEpisodePhysicalOwners,
       AdequateLeaderAuthenticatedTcEpisodeStageSet,
       AdequateLeaderAuthenticatedTcEpisodeStageRank,
       AdequateLeaderAuthenticatedTcEpisodeSameStageReplacement,
       AdequateLeaderAuthenticatedTcEpisodeRetired,
       AdequateLeaderSameRoundUpgradeBudgetDescentProperty,
       AdequateLeaderSameRoundUpgradeEpisodeClosureProperty,
       AdequateLeaderSameRoundUpgradeEpisodeAtBudget,
       AdequateLeaderSameRoundUpgradeEpisodeExitGoal,
       AdequateLeaderAuthenticatedTcNoResurrectionProperty,
       AdequateLeaderAuthenticatedTcNoOvertakeProperty,
       AdequateLeaderAuthenticatedTcOvertakeSource,
       AdequateLeaderViewExposureStrictDescentGoal,
       AdequateLeaderViewExposureRankFrontier,
       AdequateLeaderTargetFreshSelfCorridorGoal,
       AdequateLeaderExactTcSynchronizationSource,
       AdequateLeaderExactTcSynchronizationAtRank,
       AdequateLeaderExactTcSynchronizationGoal,
       AdequateLeaderExactTcSynchronizationClosureProperty,
       AdequateLeaderPostSnapshotResponsiveOrigin,
       TimeoutCertificateAndDecisionConvergenceProperty,
       DirectTimeoutViewClosureResidualProperty,
       TimeoutDirectGoal, TimeoutViewGoal, TcFrontier,
       TimeoutCertificateFormationFrontier,
       TimeoutCertificateSemanticIdentity,
       TimeoutSourceIsolatedDeliveryConvergenceProperty,
       TimeoutViewProgressProperty

THEOREM AdequateLeaderFiniteAuthenticatedTcEpisodeCloses ==
  \A specification:
    AdequateLeaderAuthenticatedTcEpisodeBudgetDescentProperty(
      specification)
      => AdequateLeaderAuthenticatedTcEpisodeClosureProperty(
           specification)
BY NatLessThanWellFounded, WellFoundedLeadsTo
   DEF AdequateLeaderAuthenticatedTcEpisodeBudgetDescentProperty,
       AdequateLeaderAuthenticatedTcEpisodeClosureProperty,
       AdequateLeaderAuthenticatedTcEpisodeExitGoal

THEOREM AsyncLiveProvidesAdequateLeaderViewExposureRankStep ==
  \A initialContext:
    AdequateLeaderViewExposureRankStepProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncLiveSpecProjectsAsyncSpec,
   AsyncSpecAlwaysStrongTypeInvariant,
   TimeoutViewOwnershipKernelInvariantFromAsyncSpec,
   AsyncLiveProvidesDirectTimeoutViewClosureResidual,
   DirectTimeoutViewDecompositionClosesTimeoutViewProgress,
   AsyncLiveProvidesAuthenticatedTcEpisodeBudgetDescent,
   AdequateLeaderFiniteAuthenticatedTcEpisodeCloses,
   AdequateLeaderFirstPostSnapshotTcHasConcreteResponsiveOrigin,
   AdequateLeaderStrictPersistInstallStartsExactTcSynchronization,
   AdequateLeaderExactTcSynchronizationSourceStartsRank,
   AsyncLiveProvidesExactTcResponsiveRosterSynchronization,
   AdequateLeaderResidentTcSkipStrictlyLowersExposureRank,
   AdequateLeaderResidentOriginSkipStrictlyLowersExposureRank,
   AsyncLiveAuthenticatedTcCatchupCannotOvertakeFreshSelfWindow,
   AdequateLeaderNextTargetSelfViewFacts,
   AdequateLeaderFrozenResidentDebtIsFinite,
   PostGstAsyncBracketAdvancesEveryNodeView,
   AsyncSpecKeepsGstOnceSet,
   PTL, IsaT(7200)
   DEF AdequateLeaderViewExposureRankStepProperty,
       AdequateLeaderViewExposureRankFrontier,
       AdequateLeaderViewExposureStrictDescentGoal,
       AdequateLeaderAuthenticatedTcEpisodeClosureProperty,
       AdequateLeaderAuthenticatedTcEpisodeAtBudget,
       AdequateLeaderAuthenticatedTcOriginEpisode,
       AdequateLeaderAuthenticatedTcNoOvertakeProperty,
       AdequateLeaderAuthenticatedTcOvertakeSource,
       AdequateLeaderFrozenViewEpisodeSource,
       AdequateLeaderResidentFutureTcDebt,
       AdequateLeaderResidentFutureOriginDebt,
       AdequateLeaderResidentFutureDebt,
       AdequateLeaderViewExposureRank,
       AdequateLeaderTargetSelfViewDistance,
       AdequateLeaderNextTargetSelfView,
       AdequateLeaderTargetFreshSelfCorridorGoal,
       AdequateLeaderExactTcSynchronizationSource,
       AdequateLeaderExactTcSynchronizationAtRank,
       AdequateLeaderExactTcSynchronizationGoal,
       AdequateLeaderExactTcSynchronizationClosureProperty,
       TimeoutViewProgressProperty,
       AdequateLeaderLocalTargetDecisionSource

THEOREM AdequateLeaderViewExposureRankOrderingIsWellFounded ==
  IsWellFoundedOn(
    AdequateLeaderViewExposureRankOrdering,
    AdequateLeaderViewExposureRankCarrier)
BY NatLessThanWellFounded, WFLexPairOrdering
   DEF AdequateLeaderViewExposureRankOrdering,
       AdequateLeaderViewExposureRankCarrier

THEOREM AsyncLiveProvidesLocalFreshSelfCorridorExposure ==
  \A initialContext:
    AdequateLeaderLocalFreshSelfCorridorExposureProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncLiveSpecProjectsAsyncSpec,
   AsyncSpecAlwaysStrongTypeInvariant,
   AsyncLiveProvidesAdequateLeaderViewExposureRankStep,
   AdequateLeaderFrozenSourceStartsConcreteViewRank,
   AdequateLeaderViewExposureRankOrderingIsWellFounded,
   WellFoundedLeadsTo, PTL, IsaT(1200)
   DEF AdequateLeaderLocalFreshSelfCorridorExposureProperty,
       AdequateLeaderFrozenViewEpisodeSource,
       AdequateLeaderViewExposureRankStepProperty,
       AdequateLeaderViewExposureRankFrontier,
       AdequateLeaderViewExposureStrictDescentGoal,
       AdequateLeaderTargetFreshSelfCorridorGoal,
       AdequateLeaderFreshSynchronizedTargetCorridor,
       AdequateLeaderLocalTargetDecisionSource,
       AsyncStrongTypeInvariant, StrongInductiveInvariant,
       Safety, TypeInvariant

AdequateLeaderTargetFreshCorridorExposureProperty(specification) ==
  specification
    => \A target \in ValidatorIds:
         AdequateLeaderTargetDecisionSource(target)
           ~> (NodeHasDecision(target)
                \/ \E leaderContext \in ContextRecords,
                      leader \in AsyncCurrentResponsiveVoters,
                      leaderView \in Views:
                     AdequateLeaderFreshSynchronizedTargetCorridor(
                       leader, leaderContext, leader, leaderView))

AdequateLeaderLocalViewReachSourceProperty(specification) ==
  specification
    => \A target \in ValidatorIds:
         AdequateLeaderLocalTargetDecisionSource(target)
           ~> (NodeHasDecision(target)
                \/ AdequateLeaderTargetDecisionSource(target))

THEOREM AdequateLeaderLocalSelfExposureSuppliesTargetExposure ==
  \A specification:
    AdequateLeaderLocalFreshSelfCorridorExposureProperty(specification)
      => AdequateLeaderTargetFreshCorridorExposureProperty(specification)
BY PTL, Isa
   DEF AdequateLeaderLocalFreshSelfCorridorExposureProperty,
       AdequateLeaderTargetFreshCorridorExposureProperty,
       AdequateLeaderTargetFreshSelfCorridorGoal,
       AdequateLeaderFreshSynchronizedTargetCorridor,
       AdequateLeaderLocalTargetDecisionSource,
       AdequateLeaderTargetDecisionSource,
       AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch

THEOREM AsyncLiveProvidesTargetFreshCorridorExposure ==
  \A initialContext:
    AdequateLeaderTargetFreshCorridorExposureProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncLiveProvidesLocalFreshSelfCorridorExposure,
   AdequateLeaderLocalSelfExposureSuppliesTargetExposure

THEOREM AdequateLeaderLocalSelfExposureSuppliesViewReachSource ==
  \A specification:
    AdequateLeaderLocalFreshSelfCorridorExposureProperty(specification)
      => AdequateLeaderLocalViewReachSourceProperty(specification)
BY AdequateLeaderFreshSelfCorridorImpliesAdequateViewReached,
   PTL
   DEF AdequateLeaderLocalFreshSelfCorridorExposureProperty,
       AdequateLeaderLocalViewReachSourceProperty,
       AdequateLeaderTargetFreshSelfCorridorGoal,
       AdequateLeaderFreshSynchronizedTargetCorridor

THEOREM AsyncLiveProvidesAdequateLeaderLocalViewReachSource ==
  \A initialContext:
    AdequateLeaderLocalViewReachSourceProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncLiveProvidesLocalFreshSelfCorridorExposure,
   AdequateLeaderLocalSelfExposureSuppliesViewReachSource

(***************************************************************************
Exact remaining ViewReach boundary.

The theorem above is only the first conjunct of
`AdequateLeaderViewReachCompositionProperty`.  The exit-to-Decision conjunct
cannot be inferred by looping corridor exit back through local exposure: that
admits an infinite view-reset lasso.

The higher-TC lasso is now repaired at the source boundary.  The first exact
TC install freezes one responsive dual quorum, the synchronization rank
delivers that immutable TC to every member, and the total service budget is
split into synchronization and fixed-corridor suffixes.  The static quorum
lemma above then excludes every formed TC at or above the synchronized view.
The exit handoff also stores the original corridor authority receipt, so a
nonresponsive candidate cannot masquerade as the exited target.

TODO: provide `AdequateLeaderFixedCorridorDeadlineServiceProperty` directly
from the existing service deadlines and frozen owner/stage ranks.  The proof
must count each exact packet, I/O, deferred, runner, leader-wire, and
producer-continuation episode and show that their cumulative clock charge is
at most `AsyncFixedCorridorServiceBudget`.  The receipt is ghost-only and
`AsyncTickEnabled` is unchanged.  The current qualitative finite-continuation
provider has no theorem subsuming its rank under the configured numeric
budget, so it cannot yet be used as this inequality.  Until that arithmetic
bridge is proved, the conditional provider below is not a release promotion
of the second ViewReach conjunct.  The numeric provider must additionally
discharge
`AdequateLeaderAuthorityBoundActiveReceiptDecisionCarryProperty`: receipt
acquisition carries the exact frozen authority from the fresh self-leader
arming boundary, and receipt service includes exact-target CommitQC
dissemination.  The fresh-source deadline property alone has no past-time
carrier for an arbitrary later target corridor and therefore cannot be used
as that stronger interface.
***************************************************************************)

AdequateLeaderTargetFrozenCorridorOpenGoal(
    target, leaderContext, leader, leaderView) ==
  \/ NodeHasDecision(target)
  \/ \E subject \in Subjects:
       AdequateLeaderTargetProductiveSubjectOpenFrontier(
         target, leaderContext, leader, leaderView, subject)

AdequateLeaderTargetCorridorExitDecisionProperty(specification) ==
  specification
    => \A target \in ValidatorIds,
          leaderContext \in ContextRecords,
          leader \in ValidatorIds,
          leaderView \in Views:
         AdequateLeaderTargetAnyCorridorExitHandoff(
           target, leaderContext, leader, leaderView)
           ~> NodeHasDecision(target)

AdequateLeaderFrozenCorridorOpeningProperty(specification) ==
  specification
    => \A target \in ValidatorIds,
          leaderContext \in ContextRecords,
          leader \in ValidatorIds,
          leaderView \in Views:
         AdequateLeaderFrozenTargetCorridor(
           target, leaderContext, leader, leaderView)
           ~> AdequateLeaderTargetFrozenCorridorOpenGoal(
                target, leaderContext, leader, leaderView)

AdequateLeaderSelfLeaderFrozenCorridorDecisionProperty(specification) ==
  specification
    => \A leaderContext \in ContextRecords,
          leader \in AsyncCurrentResponsiveVoters,
          leaderView \in Views:
         AdequateLeaderFrozenTargetCorridor(
           leader, leaderContext, leader, leaderView)
           ~> NodeHasDecision(leader)

AdequateLeaderFreshSelfLeaderDecisionProperty(specification) ==
  specification
    => \A leaderContext \in ContextRecords,
          leader \in AsyncCurrentResponsiveVoters,
          leaderView \in Views:
         AdequateLeaderFreshSynchronizedTargetCorridor(
           leader, leaderContext, leader, leaderView)
           ~> NodeHasDecision(leader)

\* This is target-local certificate dissemination, not aggregate Decision.
\* `source` and `target` remain fixed through one exact CommitQC identity;
\* another validator's receipt cannot discharge the endpoint.
AdequateLeaderResponsiveDecisionDisseminationProperty(specification) ==
  specification
    => \A source, target \in AsyncCurrentResponsiveVoters:
         NodeHasDecision(source) ~> NodeHasDecision(target)

THEOREM AdequateLeaderCorridorExitReceiptRetainsLocalTargetSource ==
  \A target \in ValidatorIds,
     leaderContext \in ContextRecords,
     leader \in ValidatorIds,
     leaderView \in Views:
    AdequateLeaderTargetAnyCorridorExitHandoff(
      target, leaderContext, leader, leaderView)
      => \/ NodeHasDecision(target)
         \/ AdequateLeaderLocalTargetDecisionSource(target)
BY Isa
   DEF AdequateLeaderTargetAnyCorridorExitHandoff,
       AdequateLeaderTargetOccurrenceCorridorExitHandoff,
       AdequateLeaderCorridorAuthorityReceipt,
       AdequateLeaderCorridorAuthorityReceiptValid,
       AdequateLeaderFrozenResponsiveRoster,
       AdequateLeaderLocalTargetDecisionSource,
       AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch

THEOREM AdequateLeaderFixedDeadlineServiceClosesFreshSelfCorridor ==
  \A specification:
    AdequateLeaderFixedCorridorDeadlineServiceProperty(specification)
      => AdequateLeaderFreshSelfLeaderDecisionProperty(specification)
BY AdequateLeaderFreshSynchronizedCorridorStartsFixedDeadline,
   PTL, Isa
   DEF AdequateLeaderFixedCorridorDeadlineServiceProperty,
       AdequateLeaderFixedCorridorDeadlineSource,
       AdequateLeaderFixedCorridorDecisionBeforeDeadline,
       AdequateLeaderFreshSelfLeaderDecisionProperty

THEOREM AdequateLeaderLocalExposureAndFreshServiceCloseCorridorExit ==
  \A specification:
    /\ AdequateLeaderLocalFreshSelfCorridorExposureProperty(specification)
    /\ AdequateLeaderFreshSelfLeaderDecisionProperty(specification)
    => AdequateLeaderTargetCorridorExitDecisionProperty(specification)
BY AdequateLeaderCorridorExitReceiptRetainsLocalTargetSource,
   PTL, Isa
   DEF AdequateLeaderLocalFreshSelfCorridorExposureProperty,
       AdequateLeaderTargetFreshSelfCorridorGoal,
       AdequateLeaderFreshSelfLeaderDecisionProperty,
       AdequateLeaderTargetCorridorExitDecisionProperty

THEOREM AdequateLeaderLocalExposureAndDeadlineServiceSupplyViewReach ==
  \A specification:
    /\ AdequateLeaderLocalFreshSelfCorridorExposureProperty(specification)
    /\ AdequateLeaderFixedCorridorDeadlineServiceProperty(specification)
    => AdequateLeaderViewReachCompositionProperty(specification)
BY AdequateLeaderLocalSelfExposureSuppliesViewReachSource,
   AdequateLeaderFixedDeadlineServiceClosesFreshSelfCorridor,
   AdequateLeaderLocalExposureAndFreshServiceCloseCorridorExit,
   PTL
   DEF AdequateLeaderViewReachCompositionProperty,
       AdequateLeaderLocalViewReachSourceProperty,
       AdequateLeaderTargetCorridorExitDecisionProperty

THEOREM AsyncLiveFixedDeadlineServiceSuppliesAdequateLeaderViewReach ==
  \A initialContext:
    AdequateLeaderFixedCorridorDeadlineServiceProperty(
      AsyncLiveSpecAt(initialContext))
      => AdequateLeaderViewReachCompositionProperty(
           AsyncLiveSpecAt(initialContext))
BY AsyncLiveProvidesLocalFreshSelfCorridorExposure,
   AdequateLeaderLocalExposureAndDeadlineServiceSupplyViewReach

THEOREM AdequateLeaderViewReachCompositionSuppliesCorridorExitDecision ==
  \A specification:
    AdequateLeaderViewReachCompositionProperty(specification)
      => AdequateLeaderTargetCorridorExitDecisionProperty(specification)
BY PTL
   DEF AdequateLeaderViewReachCompositionProperty,
       AdequateLeaderTargetCorridorExitDecisionProperty

THEOREM AdequateLeaderFrozenCorridorExposesProductiveFrontierOrExit ==
  \A target \in ValidatorIds,
     leaderContext \in ContextRecords,
     leader \in ValidatorIds,
     leaderView \in Views:
    /\ AsyncStrongTypeInvariant
    /\ AdequateLeaderFrozenTargetCorridor(
         target, leaderContext, leader, leaderView)
    => \/ NodeHasDecision(target)
       \/ AdequateLeaderTargetAnyCorridorExitHandoff(
            target, leaderContext, leader, leaderView)
       \/ \E subject \in Subjects:
            AdequateLeaderTargetProductiveSubjectOpenFrontier(
              target, leaderContext, leader, leaderView, subject)
BY AdequateLeaderFrozenCorridorHasProductiveSubjectReentry,
   AsyncHeartbeatSubjectIsValid, IsaT(240)
   DEF AdequateLeaderTargetProductiveSubjectReentryGoal,
       AsyncStrongTypeInvariant, StrongInductiveInvariant,
       Safety, TypeInvariant

THEOREM AdequateLeaderFreshSelfCorridorOpensOriginalTarget ==
  \A target \in ValidatorIds, leaderView \in Views:
    /\ AsyncStrongTypeInvariant
    /\ AdequateLeaderFrozenTargetCorridor(
         target, context, target, leaderView)
    => \/ NodeHasDecision(target)
       \/ \E subject \in Subjects:
            AdequateLeaderTargetProductiveSubjectOpenFrontier(
              target, context, target, leaderView, subject)
BY AdequateLeaderFrozenCorridorExposesProductiveFrontierOrExit,
   Isa
   DEF AdequateLeaderTargetAnyCorridorExitHandoff,
       AdequateLeaderTargetOccurrenceCorridorExitHandoff

THEOREM AdequateLeaderLocalSelfExposureSuppliesTargetCorridorEntry ==
  \A specification:
    /\ (specification => []AsyncStrongTypeInvariant)
    /\ AdequateLeaderLocalFreshSelfCorridorExposureProperty(specification)
    => AdequateLeaderTargetCorridorEntryProperty(specification)
BY AdequateLeaderFreshSelfCorridorOpensOriginalTarget,
   PTL, Isa
   DEF AdequateLeaderLocalFreshSelfCorridorExposureProperty,
       AdequateLeaderTargetFreshSelfCorridorGoal,
       AdequateLeaderFreshSynchronizedTargetCorridor,
       AdequateLeaderTargetCorridorEntryProperty,
       AdequateLeaderLocalTargetDecisionSource,
       AdequateLeaderTargetDecisionSource

THEOREM AsyncLiveProvidesAdequateLeaderTargetCorridorEntry ==
  \A initialContext:
    AdequateLeaderTargetCorridorEntryProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncLiveSpecProjectsAsyncSpec,
   AsyncSpecAlwaysStrongTypeInvariant,
   AsyncLiveProvidesLocalFreshSelfCorridorExposure,
   AdequateLeaderLocalSelfExposureSuppliesTargetCorridorEntry

THEOREM AdequateLeaderStrongTypeAndExitClosureOpenFrozenCorridor ==
  \A specification:
    /\ (specification => []AsyncStrongTypeInvariant)
    /\ AdequateLeaderTargetCorridorExitDecisionProperty(specification)
    => AdequateLeaderFrozenCorridorOpeningProperty(specification)
BY AdequateLeaderFrozenCorridorExposesProductiveFrontierOrExit, PTL
   DEF AdequateLeaderTargetCorridorExitDecisionProperty,
       AdequateLeaderFrozenCorridorOpeningProperty,
       AdequateLeaderTargetFrozenCorridorOpenGoal

THEOREM AsyncLiveViewReachOpensEveryFrozenCorridor ==
  \A initialContext:
    AdequateLeaderViewReachCompositionProperty(
      AsyncLiveSpecAt(initialContext))
      => AdequateLeaderFrozenCorridorOpeningProperty(
           AsyncLiveSpecAt(initialContext))
BY AsyncLiveSpecProjectsAsyncSpec,
   AsyncSpecAlwaysStrongTypeInvariant,
   AdequateLeaderViewReachCompositionSuppliesCorridorExitDecision,
   AdequateLeaderStrongTypeAndExitClosureOpenFrozenCorridor

THEOREM AdequateLeaderOpeningConvergenceAndExitCloseSelfLeaderCorridor ==
  \A specification:
    /\ AdequateLeaderFrozenCorridorOpeningProperty(specification)
    /\ AdequateLeaderFixedTargetCorridorConvergenceProperty(specification)
    /\ AdequateLeaderTargetCorridorExitDecisionProperty(specification)
    => AdequateLeaderSelfLeaderFrozenCorridorDecisionProperty(specification)
BY PTL
   DEF AdequateLeaderFrozenCorridorOpeningProperty,
       AdequateLeaderTargetFrozenCorridorOpenGoal,
       AdequateLeaderFixedTargetCorridorConvergenceProperty,
       AdequateLeaderTargetFrozenCorridorTerminalGoal,
       AdequateLeaderTargetCorridorExitDecisionProperty,
       AdequateLeaderSelfLeaderFrozenCorridorDecisionProperty

THEOREM AdequateLeaderDecisionSourceIsItsNodeDecision ==
  \A source, qc:
    DecisionSourceAt(source, qc) => NodeHasDecision(source)
BY Isa DEF DecisionSourceAt, NodeHasDecision

THEOREM AsyncLiveDecisionAuthorityRetainsExactTargetDelivery ==
  \A initialContext:
    AsyncLiveSpecAt(initialContext)
      => [](\A source, target \in AsyncCurrentResponsiveVoters,
               qc \in QcRecordSet:
             DecisionSourceAt(source, qc)
               => \/ NodeHasDecision(target)
                  \/ /\ TimeoutDecisionKernelSource(source, target, qc)
                     /\ CommitCertificateDelivery(source, target, qc))
BY AsyncLiveSpecProjectsAsyncSpec,
   AsyncSpecAlwaysStrongTypeInvariant,
   TimeoutViewOwnershipKernelInvariantFromAsyncSpec,
   TimeoutDecisionSourceRetainsExactDirectDelivery,
   AdequateLeaderDecisionSourceIsItsNodeDecision,
   PTL, IsaT(300)
   DEF TimeoutDecisionKernelSource

THEOREM AsyncLiveProvidesResponsiveDecisionDissemination ==
  \A initialContext:
    AdequateLeaderResponsiveDecisionDisseminationProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncLiveProvidesDirectTimeoutViewClosureResidual,
   TimeoutDecisionDirectPhysicalKernelsDischargeDelivery,
   AsyncLiveDecisionAuthorityRetainsExactTargetDelivery,
   AdequateLeaderDecisionSourceIsItsNodeDecision,
   PTL, IsaT(600)
   DEF DirectTimeoutViewClosureResidualProperty,
       TimeoutCertificateDecisionPhysicalKernelProperties,
       AdequateLeaderResponsiveDecisionDisseminationProperty,
       NodeHasDecision

THEOREM AdequateLeaderFreshSelfLeaderAndDisseminationSupplyCorridorEntry ==
  \A specification:
    /\ AdequateLeaderTargetFreshCorridorExposureProperty(specification)
    /\ AdequateLeaderSelfLeaderFrozenCorridorDecisionProperty(specification)
    /\ AdequateLeaderResponsiveDecisionDisseminationProperty(specification)
    => AdequateLeaderTargetCorridorEntryProperty(specification)
BY PTL, Isa
   DEF AdequateLeaderTargetFreshCorridorExposureProperty,
       AdequateLeaderSelfLeaderFrozenCorridorDecisionProperty,
       AdequateLeaderResponsiveDecisionDisseminationProperty,
       AdequateLeaderTargetCorridorEntryProperty,
       AdequateLeaderFreshSynchronizedTargetCorridor,
       AdequateLeaderFrozenTargetCorridor,
       AsyncCurrentResponsiveVoters,
       CurrentVoters, CurrentEpoch

THEOREM AsyncLiveFreshCorridorExposureSuppliesCorridorEntry ==
  \A initialContext:
    /\ AdequateLeaderViewReachCompositionProperty(
         AsyncLiveSpecAt(initialContext))
    /\ AdequateLeaderTargetFreshCorridorExposureProperty(
         AsyncLiveSpecAt(initialContext))
    /\ AdequateLeaderFixedTargetCorridorConvergenceProperty(
         AsyncLiveSpecAt(initialContext))
    => AdequateLeaderTargetCorridorEntryProperty(
         AsyncLiveSpecAt(initialContext))
BY AsyncLiveViewReachOpensEveryFrozenCorridor,
   AdequateLeaderViewReachCompositionSuppliesCorridorExitDecision,
   AdequateLeaderOpeningConvergenceAndExitCloseSelfLeaderCorridor,
   AsyncLiveProvidesResponsiveDecisionDissemination,
   AdequateLeaderFreshSelfLeaderAndDisseminationSupplyCorridorEntry

(***************************************************************************
Protected qualitative corridor composition.

The numeric fixed-deadline receipt above remains a diagnostic arithmetic
boundary.  Release liveness does not consume it.  Once an exact leader wire
is due, receiver-local reservation freezes a scheduler ordinal before the
current-view timeout lifecycle; after ingress drain the Delivery candidate
inherits that same ordinal.  The active service window consequently survives
wall-clock expiry only while one of those concrete owners remains.  The
finite producer/occurrence episode then either consumes the owner, reaches a
strictly lower occurrence rank, or installs Decision.  No tick, retry,
equal-count replacement, or replenishment is called progress.

The first theorem is the generic temporal compositor used by the indexed
product.  Its fresh-corridor premise must be proved from the exact
reserve/admit/runner/producer fair actions; it is intentionally not replaced
by the proof-only numeric deadline predicate.
***************************************************************************)

THEOREM AdequateLeaderLocalExposureAndProtectedServiceSupplyViewReach ==
  \A specification:
    /\ AdequateLeaderLocalFreshSelfCorridorExposureProperty(specification)
    /\ AdequateLeaderFreshSelfLeaderDecisionProperty(specification)
    => AdequateLeaderViewReachCompositionProperty(specification)
BY AdequateLeaderLocalSelfExposureSuppliesViewReachSource,
   AdequateLeaderLocalExposureAndFreshServiceCloseCorridorExit,
   PTL
   DEF AdequateLeaderViewReachCompositionProperty,
       AdequateLeaderLocalViewReachSourceProperty,
       AdequateLeaderTargetCorridorExitDecisionProperty

AdequateLeaderCompletedLocalProviderKernelProperty(specification) ==
  /\ (specification => []AsyncStrongTypeInvariant)
  /\ AdequateLeaderLocalFreshSelfCorridorExposureProperty(specification)
  /\ AdequateLeaderFreshSelfLeaderDecisionProperty(specification)
  /\ AdequateLeaderTargetProofInvariantsProperty(specification)
  /\ AdequateLeaderTargetProducerTransportClosureProperty(specification)
  /\ AdequateLeaderTargetProductiveEpisodeRankStepProperty(specification)

THEOREM AdequateLeaderCompletedLocalProviderKernelSuppliesSemanticKernel ==
  \A specification:
    AdequateLeaderCompletedLocalProviderKernelProperty(specification)
      => AdequateLeaderLocalSemanticKernelProperty(specification)
BY AdequateLeaderLocalExposureAndProtectedServiceSupplyViewReach,
   AdequateLeaderLocalSelfExposureSuppliesTargetCorridorEntry,
   AdequateLeaderLocalFixedCorridorKernelSuppliesConvergence,
   PTL
   DEF AdequateLeaderCompletedLocalProviderKernelProperty,
       AdequateLeaderLocalFixedCorridorKernelProperty,
       AdequateLeaderLocalSemanticKernelProperty

=============================================================================
