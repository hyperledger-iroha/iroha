---- MODULE SumeragiV2LockedBodyReproposalProgressProofs ----
EXTENDS SumeragiV2ProgressWitnessFinalClosureProofs

(***************************************************************************
Locked-body reproposal temporal composition.

The release outcome is deliberately disjunctive.  A retained lock may leave
the decision-free frontier through its exact old-round CommitQC, through an
unchanged later-round proposal, through a higher certified Prepare lock, or
through the node's durable Decision.  The first three exits can happen before
global decision convergence; none is required after a durable Decision exists.

Consequently this leaf does not invent a second retained-body scheduler or
assume that a routing recipient is the archive signer.  The imported
historical witness proofs continue to preserve the exact PrepareQC and
route-neutral CertifiedResponse lineages.  The temporal step below uses only
the fourth, already-declared terminal exit: rotating-leader progress entails
responsive decision convergence, and the frozen one-height voter projection
keeps the originally responsive locked node in that convergence domain.

This dependency is acyclic.  No theorem from
SumeragiV2AsyncTemporalClosureProofs is imported, and the result below is an
implication from RotatingLeaderProgressProperty, not an unconditional use of
its still-separate release obligation.
***************************************************************************)

ResponsiveDecisionConvergenceProperty(specification) ==
  specification
    => (gst /\ ~ResponsiveNodesDecide) ~> ResponsiveNodesDecide

(***************************************************************************
Static entry and exit facts for one fixed responsive-voter projection.

Outside every declared locked-body outcome, the stable source's node has no
Decision.  Because that node is responsive, aggregate responsive decision
cannot yet hold.  Conversely, once the same fixed responsive set has decided,
that node's Decision is exactly the terminal outcome already admitted by the
release predicate.
***************************************************************************)

THEOREM StableLockedBodyOutsideOutcomeCreatesDecisionDebt ==
  \A initialContext, node, lockedRound, subject:
    /\ AsyncCurrentResponsiveVoters = AsyncVotersAt(initialContext)
    /\ StableAvailableRetainedLock(node, lockedRound, subject)
    /\ ~LockedBodyReproposalOutcome(node, lockedRound, subject)
    => /\ node \in AsyncVotersAt(initialContext)
       /\ gst
       /\ ~ResponsiveNodesDecide
BY Isa
   DEF StableAvailableRetainedLock,
       LockedBodyReproposalOutcome,
       LockedBodyLegitimatelyDecidedOrSuperseded,
       ResponsiveNodesDecide

THEOREM FixedResponsiveDecisionIsLockedBodyOutcome ==
  \A initialContext, node, lockedRound, subject:
    /\ AsyncCurrentResponsiveVoters = AsyncVotersAt(initialContext)
    /\ node \in AsyncVotersAt(initialContext)
    /\ ResponsiveNodesDecide
    => LockedBodyReproposalOutcome(node, lockedRound, subject)
BY Isa
   DEF ResponsiveNodesDecide,
       LockedBodyReproposalOutcome,
       LockedBodyLegitimatelyDecidedOrSuperseded

(***************************************************************************
The two rotating-leader clauses compose to aggregate responsive decision.
GST monotonicity is essential: if the first clause reaches an honest leader
view rather than Decision directly, GST remains true while the second clause
discharges that leader frontier.
***************************************************************************)

THEOREM RotatingLeaderProgressSuppliesResponsiveDecisionConvergence ==
  \A initialContext:
    RotatingLeaderProgressProperty(AsyncLiveSpecAt(initialContext))
      => ResponsiveDecisionConvergenceProperty(
           AsyncLiveSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext,
                RotatingLeaderProgressProperty(
                  AsyncLiveSpecAt(initialContext))
         PROVE ResponsiveDecisionConvergenceProperty(
                 AsyncLiveSpecAt(initialContext))
    <2>1. ASSUME AsyncLiveSpecAt(initialContext)
           PROVE (gst /\ ~ResponsiveNodesDecide)
                   ~> ResponsiveNodesDecide
      <3>1. AsyncSpecAt(initialContext)
        BY <2>1, AsyncLiveSpecProjectsAsyncSpec
      <3>2. AsyncSpecAt(initialContext) => [](gst => []gst)
        BY AsyncSpecKeepsGstOnceSet
      <3>3. (gst /\ ~ResponsiveNodesDecide)
               ~> (ResponsiveHonestLeaderViewReached
                     \/ ResponsiveNodesDecide)
        BY <1>1, <2>1 DEF RotatingLeaderProgressProperty
      <3>4. (gst /\ ResponsiveHonestLeaderViewReached
                    /\ ~ResponsiveNodesDecide)
               ~> ResponsiveNodesDecide
        BY <1>1, <2>1 DEF RotatingLeaderProgressProperty
      <3> QED BY <3>1, <3>2, <3>3, <3>4, PTL
    <2> QED BY <2>1
         DEF ResponsiveDecisionConvergenceProperty
  <1> QED BY <1>1

(***************************************************************************
Decision convergence closes every stable locked-body source.

The initial stable-source state fixes the node in AsyncVotersAt.  The
one-height context invariant keeps AsyncCurrentResponsiveVoters equal to that
set until convergence.  A source which has not already reached one of the
three stronger exits therefore reaches the fourth exit, NodeHasDecision.
***************************************************************************)

THEOREM ResponsiveDecisionConvergenceClosesLockedBodyReproposal ==
  \A initialContext:
    ResponsiveDecisionConvergenceProperty(
      AsyncLiveSpecAt(initialContext))
      => LockedBodyReproposalProgressProperty(
           AsyncLiveSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext,
                ResponsiveDecisionConvergenceProperty(
                  AsyncLiveSpecAt(initialContext))
         PROVE LockedBodyReproposalProgressProperty(
                 AsyncLiveSpecAt(initialContext))
    <2>1. ASSUME AsyncLiveSpecAt(initialContext)
           PROVE \A node \in ValidatorIds,
                     lockedRound \in Views,
                     subject \in Subjects:
              StableAvailableRetainedLock(node, lockedRound, subject)
                ~> LockedBodyReproposalOutcome(
                     node, lockedRound, subject)
      <3>1. AsyncSpecAt(initialContext)
        BY <2>1, AsyncLiveSpecProjectsAsyncSpec
      <3>2. [](AsyncCurrentResponsiveVoters
                 = AsyncVotersAt(initialContext))
        BY <3>1, AsyncSpecAlwaysUsesFixedResponsiveVoters
      <3>3. (gst /\ ~ResponsiveNodesDecide)
               ~> ResponsiveNodesDecide
        BY <1>1, <2>1
           DEF ResponsiveDecisionConvergenceProperty
      <3>4. ASSUME NEW node \in ValidatorIds,
                    NEW lockedRound \in Views,
                    NEW subject \in Subjects
             PROVE StableAvailableRetainedLock(
                       node, lockedRound, subject)
                     ~> LockedBodyReproposalOutcome(
                          node, lockedRound, subject)
        <4>1. [](StableAvailableRetainedLock(
                    node, lockedRound, subject)
                   /\ ~LockedBodyReproposalOutcome(
                        node, lockedRound, subject)
                  => /\ node \in AsyncVotersAt(initialContext)
                     /\ gst
                     /\ ~ResponsiveNodesDecide)
          BY <3>2,
             StableLockedBodyOutsideOutcomeCreatesDecisionDebt, PTL
        <4>2. [](AsyncCurrentResponsiveVoters
                    = AsyncVotersAt(initialContext)
                   /\ node \in AsyncVotersAt(initialContext)
                   /\ ResponsiveNodesDecide
                  => LockedBodyReproposalOutcome(
                       node, lockedRound, subject))
          BY FixedResponsiveDecisionIsLockedBodyOutcome, PTL
        <4> QED BY <3>2, <3>3, <4>1, <4>2, PTL
      <3> QED BY <3>4
    <2> QED BY <2>1
         DEF LockedBodyReproposalProgressProperty
  <1> QED BY <1>1

(***************************************************************************
Release-facing dependency theorem.

Once the independent rotating-leader leaf is supplied, this theorem closes
the exact locked-body reproposal property without an additional temporal
assumption or an unproved declaration in this module.
***************************************************************************)

THEOREM RotatingLeaderProgressClosesLockedBodyReproposal ==
  \A initialContext:
    RotatingLeaderProgressProperty(AsyncLiveSpecAt(initialContext))
      => LockedBodyReproposalProgressProperty(
           AsyncLiveSpecAt(initialContext))
BY RotatingLeaderProgressSuppliesResponsiveDecisionConvergence,
   ResponsiveDecisionConvergenceClosesLockedBodyReproposal

=============================================================================
