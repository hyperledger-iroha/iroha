---- MODULE SumeragiV2TimeoutViewProgressProofs ----
EXTENDS SumeragiV2ProgressWitnessFinalClosureProofs

(***************************************************************************
Timeout/view temporal closure by subsumption.

The release-facing timeout property deliberately permits a Decision as its
terminal result:

  (gst /\ nodeView[node] = roundView /\ ~NodeHasDecision(node))
    ~> (nodeView[node] > roundView \/ NodeHasDecision(node)).

Consequently it is not an independent temporal assumption once rotating
leader progress is available.  The proof below first composes the two
rotating-leader clauses into aggregate responsive Decision progress, then
uses the frozen one-height voter frame to project that aggregate result to
the particular responsive node quantified by the timeout property.

No scheduler action, fairness clause, or voter/service domain is redefined.
In particular, `AsyncCurrentResponsiveVoters` remains the voter-only domain;
archive service nodes do not enter the timeout theorem.
***************************************************************************)

ResponsiveDecisionProgressProperty(specification) ==
  specification
    => (gst /\ ~ResponsiveNodesDecide) ~> ResponsiveNodesDecide

THEOREM RotatingLeaderProgressSuppliesResponsiveDecisionProgress ==
  \A initialContext:
    /\ AsyncLiveSpecAt(initialContext)
    /\ RotatingLeaderProgressProperty(
         AsyncLiveSpecAt(initialContext))
    => ResponsiveDecisionProgressProperty(
         AsyncLiveSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncLiveSpecAt(initialContext),
                RotatingLeaderProgressProperty(
                  AsyncLiveSpecAt(initialContext))
         PROVE ResponsiveDecisionProgressProperty(
                 AsyncLiveSpecAt(initialContext))
    <2>1. AsyncSpecAt(initialContext)
      BY <1>1, AsyncLiveSpecProjectsAsyncSpec
    <2>2. AsyncSpecAt(initialContext) => [](gst => []gst)
      BY <2>1, AsyncSpecKeepsGstOnceSet
    <2>3. (gst /\ ~ResponsiveNodesDecide)
             ~> ResponsiveNodesDecide
      BY <1>1, <2>2, PTL DEF RotatingLeaderProgressProperty
    <2> QED BY <1>1, <2>3
         DEF ResponsiveDecisionProgressProperty
  <1> QED BY <1>1

THEOREM FrozenResponsiveDecisionProgressSuppliesNodeDecision ==
  \A initialContext, node:
    /\ AsyncLiveSpecAt(initialContext)
    /\ node \in AsyncVotersAt(initialContext)
    /\ ResponsiveDecisionProgressProperty(
         AsyncLiveSpecAt(initialContext))
    => (gst /\ ~NodeHasDecision(node)) ~> NodeHasDecision(node)
PROOF
  <1>1. ASSUME NEW initialContext,
                NEW node,
                AsyncLiveSpecAt(initialContext),
                node \in AsyncVotersAt(initialContext),
                ResponsiveDecisionProgressProperty(
                  AsyncLiveSpecAt(initialContext))
         PROVE (gst /\ ~NodeHasDecision(node))
                 ~> NodeHasDecision(node)
    <2>0. AsyncSpecAt(initialContext)
      BY <1>1, AsyncLiveSpecProjectsAsyncSpec
    <2>1. []OneHeightFrameAt(initialContext)
      BY <2>0, AsyncSpecAlwaysKeepsOneHeightFrame
    <2>2. [](node \in AsyncCurrentResponsiveVoters)
      BY <1>1, <2>1, PTL DEF OneHeightFrameAt
    <2>3. [](/\ gst
             /\ ~NodeHasDecision(node)
             => gst /\ ~ResponsiveNodesDecide)
      BY <2>2, PTL DEF ResponsiveNodesDecide
    <2>4. [](/\ ResponsiveNodesDecide
             => NodeHasDecision(node))
      BY <2>2, PTL DEF ResponsiveNodesDecide
    <2>5. (gst /\ ~ResponsiveNodesDecide)
             ~> ResponsiveNodesDecide
      BY <1>1 DEF ResponsiveDecisionProgressProperty
    <2> QED BY <2>3, <2>4, <2>5, PTL
  <1> QED BY <1>1

THEOREM ResponsiveDecisionProgressSuppliesTimeoutViewProgress ==
  \A initialContext:
    /\ AsyncLiveSpecAt(initialContext)
    /\ ResponsiveDecisionProgressProperty(
         AsyncLiveSpecAt(initialContext))
    => TimeoutViewProgressProperty(
         AsyncLiveSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncLiveSpecAt(initialContext),
                ResponsiveDecisionProgressProperty(
                  AsyncLiveSpecAt(initialContext))
         PROVE TimeoutViewProgressProperty(
                 AsyncLiveSpecAt(initialContext))
    <2>1. AsyncSpecAt(initialContext)
      BY <1>1, AsyncLiveSpecProjectsAsyncSpec
    <2>2. OneHeightFrameAt(initialContext)
      BY <2>1, AsyncSpecAlwaysKeepsOneHeightFrame, PTL
    <2>3. ASSUME NEW node \in AsyncCurrentResponsiveVoters,
                    NEW roundView \in Views
           PROVE (gst
                    /\ nodeView[node] = roundView
                    /\ ~NodeHasDecision(node))
                   ~> TimeoutViewGoal(node, roundView)
      <3>1. node \in AsyncVotersAt(initialContext)
        BY <2>2, <2>3 DEF OneHeightFrameAt
      <3>2. (gst /\ ~NodeHasDecision(node))
                 ~> NodeHasDecision(node)
        BY <1>1, <2>1, <3>1,
           FrozenResponsiveDecisionProgressSuppliesNodeDecision
      <3>3. (gst
              /\ nodeView[node] = roundView
              /\ ~NodeHasDecision(node))
                 ~> NodeHasDecision(node)
        BY <3>2, PTL
      <3>4. NodeHasDecision(node)
               => TimeoutViewGoal(node, roundView)
        BY DEF TimeoutViewGoal
      <3> QED BY <3>3, <3>4, PTL
    <2> QED BY <1>1, <2>3
         DEF TimeoutViewProgressProperty, TimeoutViewGoal
  <1> QED BY <1>1

THEOREM RotatingLeaderProgressSuppliesTimeoutViewProgress ==
  \A initialContext:
    /\ AsyncLiveSpecAt(initialContext)
    /\ RotatingLeaderProgressProperty(
         AsyncLiveSpecAt(initialContext))
    => TimeoutViewProgressProperty(
         AsyncLiveSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncLiveSpecAt(initialContext),
                RotatingLeaderProgressProperty(
                  AsyncLiveSpecAt(initialContext))
         PROVE TimeoutViewProgressProperty(
                 AsyncLiveSpecAt(initialContext))
    <2>1. ResponsiveDecisionProgressProperty(
             AsyncLiveSpecAt(initialContext))
      BY <1>1,
         RotatingLeaderProgressSuppliesResponsiveDecisionProgress
    <2> QED BY <1>1, <2>1,
         ResponsiveDecisionProgressSuppliesTimeoutViewProgress
  <1> QED BY <1>1

(***************************************************************************
Release-facing reduction.

Both progress properties are implications from the same live specification,
so the explicit `AsyncLiveSpecAt` premise above can be discharged by
implication introduction.  This theorem is the non-circular leaf a higher
temporal closure can consume directly: proving rotating-leader progress once
also proves the exact timeout/view property, without a second fairness
assumption or an independent timeout-production axiom.
***************************************************************************)
THEOREM RotatingLeaderProgressPropertyImpliesTimeoutViewProgressProperty ==
  \A initialContext:
    RotatingLeaderProgressProperty(
      AsyncLiveSpecAt(initialContext))
      => TimeoutViewProgressProperty(
           AsyncLiveSpecAt(initialContext))
BY RotatingLeaderProgressSuppliesTimeoutViewProgress, PTL
   DEF TimeoutViewProgressProperty

=============================================================================
