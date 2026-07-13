---- MODULE SumeragiV2LivenessProofs ----
EXTENDS SumeragiV2Proofs

(***************************************************************************
Conditional liveness on a stable post-GST suffix.

FLP rules out unconditional deterministic consensus liveness in the fully
asynchronous model.  The proof therefore starts in an arbitrary safe state
after GST.  StableSuffixSpec retains the exact production actions and their
parameterized fairness.  StableProgressContracts is the explicit interface
that the asynchronous network/scheduler refinement must discharge: timeout
completion for an unresponsive leader, a successful bounded round for a
responsive leader, rotation reachability, body service, application service,
and context advancement.  No top-level TLA+ ASSUME is used.
***************************************************************************)

StableSuffixSpec ==
  /\ StrongInductiveInvariant
  /\ gst
  /\ [][ReliableNextV2]_vars
  /\ ReliableActionFairness

ResponsiveLeaderWindow ==
  \E roundView \in Views:
    /\ Leader(context, roundView) \in Responsive
    /\ \A node \in Responsive \cap CurrentVoters:
         nodeView[node] = roundView
    /\ ~ResponsiveNodesDecide

ResponsiveDecisionBodiesReady ==
  \A node \in Responsive \cap CurrentVoters:
    \E decision \in decisions:
      /\ decision.node = node
      /\ decision.qc.context = context
      /\ DecisionBodyReady(node, decision.qc)

UnresponsiveLeaderTimeoutProgress ==
  \A node \in Responsive \cap CurrentVoters,
     roundView \in Views:
    (gst
      /\ nodeView[node] = roundView
      /\ Leader(context, roundView) \notin Responsive
      /\ ~NodeHasDecision(node))
      ~> (nodeView[node] > roundView \/ NodeHasDecision(node))

ResponsiveLeaderRoundProgress ==
  \A node \in Responsive \cap CurrentVoters,
     roundView \in Views:
    (gst
      /\ nodeView[node] = roundView
      /\ Leader(context, roundView) \in Responsive
      /\ ~NodeHasDecision(node))
      ~> NodeHasDecision(node)

RotationReachesResponsiveLeader ==
  (gst /\ ~ResponsiveNodesDecide) ~> ResponsiveLeaderWindow

ResponsiveLeaderWindowDecides ==
  ResponsiveLeaderWindow ~> ResponsiveNodesDecide

CertifiedBodiesEventuallyReady ==
  (gst /\ ResponsiveNodesDecide) ~> ResponsiveDecisionBodiesReady

ReadyDecisionsEventuallyApply ==
  (gst /\ ResponsiveDecisionBodiesReady) ~> ResponsiveNodesApply

AppliedHeightEventuallyAdvances ==
  \A blockHeight \in Heights:
    (gst
      /\ height = blockHeight
      /\ blockHeight < MaxHeight
      /\ ResponsiveNodesApply)
      ~> (height > blockHeight)

StableProgressContracts ==
  /\ UnresponsiveLeaderTimeoutProgress
  /\ ResponsiveLeaderRoundProgress
  /\ RotationReachesResponsiveLeader
  /\ ResponsiveLeaderWindowDecides
  /\ CertifiedBodiesEventuallyReady
  /\ ReadyDecisionsEventuallyApply
  /\ AppliedHeightEventuallyAdvances

StableLivenessSpec == StableSuffixSpec /\ StableProgressContracts

THEOREM StableSuffixPreservesStrongInvariant ==
  StableSuffixSpec => []StrongInductiveInvariant
PROOF
  <1>1. ReliableNextV2 => NextV2
    BY DEF ReliableNextV2, ReliableNext, NextV2, Next,
           ReliableBeginTimeout, ReliableAssembleLocalBody,
           ReliableBeginLocalProposal
  <1>2. StrongInductiveInvariant /\ [ReliableNextV2]_vars
           => StrongInductiveInvariant'
    BY <1>1, StrongInductiveActionPreservation
  <1> QED BY <1>2, PTL DEF StableSuffixSpec

THEOREM TimeoutViewProgressObligation ==
  TimeoutViewProgressProperty(StableLivenessSpec)
PROOF
  <1>1. ASSUME StableLivenessSpec,
              NEW node \in Responsive \cap CurrentVoters,
              NEW roundView \in Views
         PROVE (gst
                  /\ nodeView[node] = roundView
                  /\ ~NodeHasDecision(node))
                 ~> (nodeView[node] > roundView
                       \/ NodeHasDecision(node))
    <2>1. /\ UnresponsiveLeaderTimeoutProgress
          /\ ResponsiveLeaderRoundProgress
      BY <1>1 DEF StableLivenessSpec, StableProgressContracts
    <2>2. \/ Leader(context, roundView) \in Responsive
          \/ Leader(context, roundView) \notin Responsive
      BY Zenon
    <2> QED BY <2>1, <2>2, PTL
             DEF UnresponsiveLeaderTimeoutProgress,
                 ResponsiveLeaderRoundProgress
  <1> QED BY <1>1 DEF TimeoutViewProgressProperty

THEOREM RotatingLeaderProgressObligation ==
  RotatingLeaderProgressProperty(StableLivenessSpec)
PROOF
  <1>1. StableLivenessSpec
           => /\ RotationReachesResponsiveLeader
              /\ ResponsiveLeaderWindowDecides
    BY DEF StableLivenessSpec, StableProgressContracts
  <1> QED BY <1>1, PTL DEF RotatingLeaderProgressProperty

THEOREM ApplicationLivenessObligation ==
  ApplicationLivenessProperty(StableLivenessSpec)
PROOF
  <1>1. StableLivenessSpec
           => /\ CertifiedBodiesEventuallyReady
              /\ ReadyDecisionsEventuallyApply
    BY DEF StableLivenessSpec, StableProgressContracts
  <1> QED BY <1>1, PTL DEF ApplicationLivenessProperty

THEOREM HeightLivenessObligation ==
  HeightLivenessProperty(StableLivenessSpec)
PROOF
  <1>1. ASSUME StableLivenessSpec,
              NEW blockHeight \in Heights
         PROVE (gst /\ height = blockHeight)
                 ~> (height > blockHeight
                       \/ (blockHeight = MaxHeight
                            /\ ResponsiveNodesApply))
    <2>1. (gst /\ ~ResponsiveNodesDecide)
              ~> ResponsiveNodesDecide
      BY <1>1, RotatingLeaderProgressObligation
         DEF RotatingLeaderProgressProperty
    <2>2. (gst /\ ResponsiveNodesDecide)
              ~> ResponsiveNodesApply
      BY <1>1, ApplicationLivenessObligation
         DEF ApplicationLivenessProperty
    <2>3. AppliedHeightEventuallyAdvances
      BY <1>1 DEF StableLivenessSpec, StableProgressContracts
    <2>4. CASE blockHeight = MaxHeight
      BY <2>1, <2>2, PTL
    <2>5. CASE blockHeight < MaxHeight
      BY <2>1, <2>2, <2>3, PTL
         DEF AppliedHeightEventuallyAdvances
    <2>6. blockHeight = MaxHeight \/ blockHeight < MaxHeight
      BY <1>1, SMT DEF Heights
    <2> QED BY <2>4, <2>5, <2>6
  <1> QED BY <1>1 DEF HeightLivenessProperty

=============================================================================
