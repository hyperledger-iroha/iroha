---- MODULE SumeragiV2RotatingLeaderProgressProofs ----
EXTENDS SumeragiV2LockedBodyReproposalProgressProofs,
        SumeragiV2TimeoutViewProgressProofs

(***************************************************************************
Rotating-leader temporal kernel isolation.

The two release-facing rotating-leader clauses are logically equivalent,
under the live asynchronous specification, to one aggregate convergence
claim:

  (gst /\ ~ResponsiveNodesDecide) ~> ResponsiveNodesDecide.

The reverse direction was already proved by the locked-body leaf: GST is
stable, so reaching an honest-leader view and then discharging that view
composes to aggregate Decision.  This module proves the forward direction:
aggregate Decision is itself an allowed target of the first clause and
directly discharges the second clause.  The equivalence removes duplicated
temporal debt without importing the open release aggregate.

The remaining production-specific work is exposed below as an adequate
leader-service kernel.  The timeout arithmetic already proves that a view
whose timeout exceeds `AsyncWorstCaseServiceBudget` is representable.  What
the current rank leaves do not yet prove is the semantic temporal bridge
which (1) reaches such a responsive self-leader window or Decision and
(2) completes the proposal/vote/QC/Decision pipeline in that window.
Generic candidate starvation is insufficient for this bridge: an immutable
candidate may leave protected ownership after a consumer-view change without
having produced the next consensus milestone.

No scheduler action, fairness clause, voter domain, or archive-service domain
is redefined here.
***************************************************************************)

AdequateResponsiveHonestLeaderViewReached ==
  \E leader \in (AsyncCurrentResponsiveVoters \cap Honest):
    /\ ~NodeHasDecision(leader)
    /\ Leader(context, nodeView[leader]) = leader
    /\ AsyncViewTimeout(nodeView[leader])
         > AsyncWorstCaseServiceBudget

THEOREM AdequateLeaderViewIsResponsiveHonestLeaderView ==
  AdequateResponsiveHonestLeaderViewReached
    => ResponsiveHonestLeaderViewReached
BY DEF AdequateResponsiveHonestLeaderViewReached,
       ResponsiveHonestLeaderViewReached

THEOREM AsyncLiveSpecHasRepresentableAdequateTimeout ==
  \A initialContext:
    AsyncLiveSpecAt(initialContext)
      => \E roundView \in Views:
           /\ roundView <= AsyncMaximumView
           /\ AsyncViewTimeout(roundView)
                > AsyncWorstCaseServiceBudget
BY AdequateViewTimeoutExists
   DEF AsyncLiveSpecAt, AsyncSpecAt, AsyncInitAt, AsyncBaseInitAt,
       InitAt

(***************************************************************************
The imported rank closure already drains every protected Normal
proposal/Prepare owner.  Recording that fact at the live-spec level pins the
boundary precisely: the missing kernel is semantic successor preservation and
adequate-window convergence, not another FIFO/cursor starvation theorem.
***************************************************************************)

THEOREM AsyncLiveNormalProposalPrepareRanksProgress ==
  \A initialContext:
    NormalProposalPrepareRankProgressProperty(
      AsyncLiveSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE NormalProposalPrepareRankProgressProperty(
                 AsyncLiveSpecAt(initialContext))
    <2>1. ProtectedServiceRanksProgressProperty(
             AsyncLiveSpecAt(initialContext))
      BY ProtectedServiceRankProgressObligation
    <2>2. ProtectedServiceRankProgressProperty(
             AsyncLiveSpecAt(initialContext))
      BY <2>1 DEF ProtectedServiceRanksProgressProperty
    <2>3. ASSUME AsyncLiveSpecAt(initialContext)
           PROVE \A candidate \in AsyncCandidateSet,
                     stage \in 2..6, position \in Nat:
              (gst
                /\ ResponsiveProtectedCandidateOwned(candidate)
                /\ NormalProposalPrepareCandidate(candidate)
                /\ CandidateServiceRank(candidate) = <<stage, position>>)
                ~> (~ResponsiveProtectedCandidateOwned(candidate)
                     \/ ServiceRankLess(CandidateServiceRank(candidate),
                          <<stage, position>>))
      <3>1. ASSUME NEW candidate \in AsyncCandidateSet,
                    NEW stage \in 2..6,
                    NEW position \in Nat
             PROVE (gst
                      /\ ResponsiveProtectedCandidateOwned(candidate)
                      /\ NormalProposalPrepareCandidate(candidate)
                      /\ CandidateServiceRank(candidate) =
                           <<stage, position>>)
                     ~> (~ResponsiveProtectedCandidateOwned(candidate)
                          \/ ServiceRankLess(
                               CandidateServiceRank(candidate),
                               <<stage, position>>))
        <4>1. (gst
                 /\ ResponsiveProtectedCandidateOwned(candidate)
                 /\ CandidateServiceRank(candidate) = <<stage, position>>)
                ~> (~ResponsiveProtectedCandidateOwned(candidate)
                     \/ ServiceRankLess(CandidateServiceRank(candidate),
                          <<stage, position>>))
          BY <2>2, <2>3 DEF ProtectedServiceRankProgressProperty
        <4> QED BY <4>1, PTL
      <3> QED BY <3>1
    <2> QED BY <2>3
         DEF NormalProposalPrepareRankProgressProperty
  <1> QED BY <1>1

(***************************************************************************
Exact fair-action/rank kernel still required from the concrete asynchronous
pipeline.

The first conjunct is the timeout/TC/backoff and leader-selection case.  The
second is the proposal, Prepare, Commit, and Decision service case under a
timeout window larger than the declared whole-pipeline budget.  Packaging
them together makes the remaining implementation obligation precise while
keeping every theorem in this module proved.
***************************************************************************)

AdequateLeaderServiceKernelProperty(specification) ==
  specification
    => /\ (gst /\ ~ResponsiveNodesDecide)
             ~> (AdequateResponsiveHonestLeaderViewReached
                   \/ ResponsiveNodesDecide)
       /\ (gst /\ AdequateResponsiveHonestLeaderViewReached
                 /\ ~ResponsiveNodesDecide)
             ~> ResponsiveNodesDecide

THEOREM AdequateLeaderServiceKernelSuppliesDecisionConvergence ==
  \A initialContext:
    AdequateLeaderServiceKernelProperty(
      AsyncLiveSpecAt(initialContext))
      => ResponsiveDecisionConvergenceProperty(
           AsyncLiveSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext,
                AdequateLeaderServiceKernelProperty(
                  AsyncLiveSpecAt(initialContext))
         PROVE ResponsiveDecisionConvergenceProperty(
                 AsyncLiveSpecAt(initialContext))
    <2>1. ASSUME AsyncLiveSpecAt(initialContext)
           PROVE (gst /\ ~ResponsiveNodesDecide)
                   ~> ResponsiveNodesDecide
      <3>1. AsyncSpecAt(initialContext)
        BY <2>1, AsyncLiveSpecProjectsAsyncSpec
      <3>2. AsyncSpecAt(initialContext) => [](gst => []gst)
        BY <3>1, AsyncSpecKeepsGstOnceSet
      <3>3. (gst /\ ~ResponsiveNodesDecide)
               ~> (AdequateResponsiveHonestLeaderViewReached
                     \/ ResponsiveNodesDecide)
        BY <1>1, <2>1 DEF AdequateLeaderServiceKernelProperty
      <3>4. (gst /\ AdequateResponsiveHonestLeaderViewReached
                    /\ ~ResponsiveNodesDecide)
               ~> ResponsiveNodesDecide
        BY <1>1, <2>1 DEF AdequateLeaderServiceKernelProperty
      <3> QED BY <3>2, <3>3, <3>4, PTL
    <2> QED BY <2>1
         DEF ResponsiveDecisionConvergenceProperty
  <1> QED BY <1>1

(***************************************************************************
Aggregate convergence is sufficient for both rotating-leader clauses.
The first clause weakens its target by adding the leader-view disjunct; the
second strengthens the convergence antecedent by adding the leader-view
predicate.
***************************************************************************)

THEOREM ResponsiveDecisionConvergenceSuppliesRotatingLeaderProgress ==
  \A initialContext:
    ResponsiveDecisionConvergenceProperty(
      AsyncLiveSpecAt(initialContext))
      => RotatingLeaderProgressProperty(
           AsyncLiveSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext,
                ResponsiveDecisionConvergenceProperty(
                  AsyncLiveSpecAt(initialContext))
         PROVE RotatingLeaderProgressProperty(
                 AsyncLiveSpecAt(initialContext))
    <2>1. ASSUME AsyncLiveSpecAt(initialContext)
           PROVE /\ (gst /\ ~ResponsiveNodesDecide)
                       ~> (ResponsiveHonestLeaderViewReached
                             \/ ResponsiveNodesDecide)
                 /\ (gst /\ ResponsiveHonestLeaderViewReached
                           /\ ~ResponsiveNodesDecide)
                       ~> ResponsiveNodesDecide
      <3>1. (gst /\ ~ResponsiveNodesDecide)
               ~> ResponsiveNodesDecide
        BY <1>1, <2>1
           DEF ResponsiveDecisionConvergenceProperty
      <3>2. (gst /\ ~ResponsiveNodesDecide)
               ~> (ResponsiveHonestLeaderViewReached
                     \/ ResponsiveNodesDecide)
        BY <3>1, PTL
      <3>3. (gst /\ ResponsiveHonestLeaderViewReached
                    /\ ~ResponsiveNodesDecide)
               ~> ResponsiveNodesDecide
        BY <3>1, PTL
      <3> QED BY <3>2, <3>3
    <2> QED BY <2>1
         DEF RotatingLeaderProgressProperty
  <1> QED BY <1>1

THEOREM RotatingLeaderProgressIsDecisionConvergence ==
  \A initialContext:
    RotatingLeaderProgressProperty(
      AsyncLiveSpecAt(initialContext))
      <=> ResponsiveDecisionConvergenceProperty(
            AsyncLiveSpecAt(initialContext))
BY RotatingLeaderProgressSuppliesResponsiveDecisionConvergence,
   ResponsiveDecisionConvergenceSuppliesRotatingLeaderProgress

THEOREM AdequateLeaderServiceKernelSuppliesRotatingLeaderProgress ==
  \A initialContext:
    AdequateLeaderServiceKernelProperty(
      AsyncLiveSpecAt(initialContext))
      => RotatingLeaderProgressProperty(
           AsyncLiveSpecAt(initialContext))
BY AdequateLeaderServiceKernelSuppliesDecisionConvergence,
   ResponsiveDecisionConvergenceSuppliesRotatingLeaderProgress

(***************************************************************************
Dependent-debt collapse.

Once the single adequate leader-service kernel is proved from the concrete
fair actions and semantic successor ranks, it closes the rotating-leader
property and, through the two acyclic leaves imported above, the exact
timeout/view and locked-body reproposal properties as well.
***************************************************************************)

THEOREM AdequateLeaderServiceKernelClosesDependentProgress ==
  \A initialContext:
    AdequateLeaderServiceKernelProperty(
      AsyncLiveSpecAt(initialContext))
      => /\ RotatingLeaderProgressProperty(
               AsyncLiveSpecAt(initialContext))
         /\ TimeoutViewProgressProperty(
              AsyncLiveSpecAt(initialContext))
         /\ LockedBodyReproposalProgressProperty(
              AsyncLiveSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext,
                AdequateLeaderServiceKernelProperty(
                  AsyncLiveSpecAt(initialContext))
         PROVE /\ RotatingLeaderProgressProperty(
                      AsyncLiveSpecAt(initialContext))
                /\ TimeoutViewProgressProperty(
                     AsyncLiveSpecAt(initialContext))
                /\ LockedBodyReproposalProgressProperty(
                     AsyncLiveSpecAt(initialContext))
    <2>1. RotatingLeaderProgressProperty(
             AsyncLiveSpecAt(initialContext))
      BY <1>1,
         AdequateLeaderServiceKernelSuppliesRotatingLeaderProgress
    <2>2. TimeoutViewProgressProperty(
             AsyncLiveSpecAt(initialContext))
      BY <2>1,
         RotatingLeaderProgressPropertyImpliesTimeoutViewProgressProperty
    <2>3. LockedBodyReproposalProgressProperty(
             AsyncLiveSpecAt(initialContext))
      BY <2>1, RotatingLeaderProgressClosesLockedBodyReproposal
    <2> QED BY <2>1, <2>2, <2>3
  <1> QED BY <1>1

=============================================================================
