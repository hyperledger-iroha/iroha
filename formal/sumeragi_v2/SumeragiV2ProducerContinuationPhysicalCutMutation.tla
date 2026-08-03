---- MODULE SumeragiV2ProducerContinuationPhysicalCutMutation ----
EXTENDS Naturals, TLC

(***************************************************************************
Finite mutation kernel for ordinary-ingress and producer-continuation
physical ownership.

The first pair starts with an exact ordinary aggregate carrier already
accepted at physical ordinal one.  The repaired shared ingress turn drains
that exact carrier; the mutation repeatedly selects a later differently
encoded carrier, modeling causal/Control/Completion/priority replenishment
ahead of the accepted target.

The second pair starts after the exact carrier has drained into a producer
continuation.  The target freezes cut two and either physical source one or
the zero sentinel used by the model for Rust `None`.  The later replay root
starts at physical source two.  Its Envelope-to-Completion successor retains
that source in the repair; the mutation sheds the physical provenance while
retaining the smaller logical ordinal, then repeatedly replaces the replay
owner and never reaches the target.

These bounded pairs are mutation evidence for the source-level selector, not
a deductive liveness proof of the asynchronous production specification.
***************************************************************************)

Phases == {"Ingress", "Continuation", "TimeoutSelection", "Done"}
ReplayStages == {"Envelope", "Completion"}
Owners == {"Target", "Replay"}
Selections == {"None"} \cup Owners

VARIABLES
  phase,
  targetDone,
  replayActive,
  replayEpoch,
  replayStage,
  replaySourcePhysicalOrdinal,
  targetHasPhysicalSource,
  lastSelected

mutationVars ==
  <<phase, targetDone, replayActive, replayEpoch,
    replayStage, replaySourcePhysicalOrdinal,
    targetHasPhysicalSource, lastSelected>>

LogicalOrdinal(owner) ==
  IF owner = "Target" THEN 2 ELSE 1

SourcePhysicalOrdinal(owner) ==
  IF owner = "Target"
  THEN IF targetHasPhysicalSource THEN 1 ELSE 0
  ELSE replaySourcePhysicalOrdinal

PhysicalCut(owner) ==
  IF owner = "Target" THEN 2 ELSE 3

PairwisePhysicalPrecedes(left, right) ==
  /\ left # right
  /\ IF SourcePhysicalOrdinal(left) >= PhysicalCut(right)
     THEN FALSE
     ELSE IF SourcePhysicalOrdinal(right) >= PhysicalCut(left)
          THEN TRUE
          ELSE LogicalOrdinal(left) < LogicalOrdinal(right)

LogicalOnlyPrecedes(left, right) ==
  /\ left # right
  /\ LogicalOrdinal(left) < LogicalOrdinal(right)

CurrentIngressInit ==
  /\ phase = "Ingress"
  /\ ~targetDone
  /\ replayActive
  /\ replayEpoch = FALSE
  /\ replayStage = "Envelope"
  /\ replaySourcePhysicalOrdinal = 2
  /\ targetHasPhysicalSource \in BOOLEAN
  /\ lastSelected = "None"

ContinuationInit ==
  /\ phase = "Continuation"
  /\ ~targetDone
  /\ replayActive
  /\ replayEpoch = FALSE
  /\ replayStage = "Envelope"
  /\ replaySourcePhysicalOrdinal = 2
  /\ targetHasPhysicalSource \in BOOLEAN
  /\ lastSelected = "None"

DrainExactOrdinaryIngressCarrier ==
  /\ phase = "Ingress"
  /\ phase' = "Continuation"
  /\ lastSelected' = "Target"
  /\ UNCHANGED
       <<targetDone, replayActive, replayEpoch,
         replayStage, replaySourcePhysicalOrdinal,
         targetHasPhysicalSource>>

LaterAggregateChurnAheadOfIngress ==
  /\ phase = "Ingress"
  /\ replayActive
  /\ replayEpoch' = ~replayEpoch
  /\ lastSelected' = "Replay"
  /\ UNCHANGED
       <<phase, targetDone, replayActive,
         replayStage, replaySourcePhysicalOrdinal,
         targetHasPhysicalSource>>

ShedReplayEnvelopeRetainingPhysicalRoot ==
  /\ phase = "Continuation"
  /\ replayActive
  /\ replayStage = "Envelope"
  /\ replayStage' = "Completion"
  /\ replaySourcePhysicalOrdinal' = replaySourcePhysicalOrdinal
  /\ UNCHANGED
       <<phase, targetDone, replayActive, replayEpoch,
         targetHasPhysicalSource, lastSelected>>

ShedReplayEnvelopeDroppingPhysicalRoot ==
  /\ phase = "Continuation"
  /\ replayActive
  /\ replayStage = "Envelope"
  /\ replayStage' = "Completion"
  /\ replaySourcePhysicalOrdinal' = 0
  /\ lastSelected' = "Replay"
  /\ UNCHANGED
       <<phase, targetDone, replayActive, replayEpoch,
         targetHasPhysicalSource>>

ServePairwiseMinimumTarget ==
  /\ phase = "Continuation"
  /\ replayActive
  /\ replayStage = "Completion"
  /\ PairwisePhysicalPrecedes("Target", "Replay")
  /\ ~PairwisePhysicalPrecedes("Replay", "Target")
  /\ phase' = "Done"
  /\ targetDone'
  /\ lastSelected' = "Target"
  /\ UNCHANGED
       <<replayActive, replayEpoch, replayStage,
         replaySourcePhysicalOrdinal, targetHasPhysicalSource>>

ReplaceReplayBySameLogicalOwner ==
  /\ phase = "Continuation"
  /\ replayActive
  /\ replayStage = "Completion"
  /\ LogicalOnlyPrecedes("Replay", "Target")
  /\ replayEpoch' = ~replayEpoch
  /\ replayStage' = "Envelope"
  /\ replaySourcePhysicalOrdinal' = 2
  /\ lastSelected' = "Replay"
  /\ UNCHANGED
       <<phase, targetDone, replayActive,
         targetHasPhysicalSource>>

(***************************************************************************
The timeout-selector pair isolates the repaired filter-before-minimum order.
Target is a true pre-cut continuation with logical ordinal two.  Replay is a
later physical occurrence at the frozen timeout cut, but it retains logical
ordinal one.  The repair removes Replay before logical selection and services
Target.  The mutation selects the obsolete ordinal and can replace it forever.
***************************************************************************)
TimeoutCutSelectionInit ==
  /\ phase = "TimeoutSelection"
  /\ ~targetDone
  /\ replayActive
  /\ replayEpoch = FALSE
  /\ replayStage = "Completion"
  /\ replaySourcePhysicalOrdinal = 2
  /\ targetHasPhysicalSource
  /\ lastSelected = "None"

TimeoutCutFilteredSelectsPreCutTarget ==
  /\ phase = "TimeoutSelection"
  /\ replayActive
  /\ SourcePhysicalOrdinal("Target") < 2
  /\ SourcePhysicalOrdinal("Replay") >= 2
  /\ LogicalOrdinal("Target") < 3
  /\ phase' = "Done"
  /\ targetDone'
  /\ lastSelected' = "Target"
  /\ UNCHANGED
       <<replayActive, replayEpoch, replayStage,
         replaySourcePhysicalOrdinal, targetHasPhysicalSource>>

TimeoutCutLogicalMinimumSelectsPostCutReplay ==
  /\ phase = "TimeoutSelection"
  /\ replayActive
  /\ SourcePhysicalOrdinal("Replay") >= 2
  /\ LogicalOnlyPrecedes("Replay", "Target")
  /\ replayEpoch' = ~replayEpoch
  /\ lastSelected' = "Replay"
  /\ UNCHANGED
       <<phase, targetDone, replayActive, replayStage,
         replaySourcePhysicalOrdinal, targetHasPhysicalSource>>

CurrentIngressFixedRunner ==
  \/ DrainExactOrdinaryIngressCarrier
  \/ ShedReplayEnvelopeRetainingPhysicalRoot
  \/ ServePairwiseMinimumTarget

CurrentIngressChurnBugRunner ==
  LaterAggregateChurnAheadOfIngress

ContinuationPhysicalCutFixedRunner ==
  \/ ShedReplayEnvelopeRetainingPhysicalRoot
  \/ ServePairwiseMinimumTarget

ContinuationLogicalOnlyBugRunner ==
  \/ ShedReplayEnvelopeDroppingPhysicalRoot
  \/ ReplaceReplayBySameLogicalOwner

TimeoutCutFilteredFixedRunner ==
  TimeoutCutFilteredSelectsPreCutTarget

TimeoutCutLogicalMinimumBugRunner ==
  TimeoutCutLogicalMinimumSelectsPostCutReplay

CurrentIngressFixedSpec ==
  /\ CurrentIngressInit
  /\ [][CurrentIngressFixedRunner]_mutationVars
  /\ WF_mutationVars(CurrentIngressFixedRunner)

CurrentIngressChurnBugSpec ==
  /\ CurrentIngressInit
  /\ [][CurrentIngressChurnBugRunner]_mutationVars
  /\ WF_mutationVars(CurrentIngressChurnBugRunner)

ContinuationPhysicalCutFixedSpec ==
  /\ ContinuationInit
  /\ [][ContinuationPhysicalCutFixedRunner]_mutationVars
  /\ WF_mutationVars(ContinuationPhysicalCutFixedRunner)

ContinuationLogicalOnlyBugSpec ==
  /\ ContinuationInit
  /\ [][ContinuationLogicalOnlyBugRunner]_mutationVars
  /\ WF_mutationVars(ContinuationLogicalOnlyBugRunner)

TimeoutCutFilteredFixedSpec ==
  /\ TimeoutCutSelectionInit
  /\ [][TimeoutCutFilteredFixedRunner]_mutationVars
  /\ WF_mutationVars(TimeoutCutFilteredFixedRunner)

TimeoutCutLogicalMinimumBugSpec ==
  /\ TimeoutCutSelectionInit
  /\ [][TimeoutCutLogicalMinimumBugRunner]_mutationVars
  /\ WF_mutationVars(TimeoutCutLogicalMinimumBugRunner)

MutationTypeInvariant ==
  /\ phase \in Phases
  /\ targetDone \in BOOLEAN
  /\ replayActive \in BOOLEAN
  /\ replayEpoch \in BOOLEAN
  /\ replayStage \in ReplayStages
  /\ replaySourcePhysicalOrdinal \in Nat
  /\ targetHasPhysicalSource \in BOOLEAN
  /\ lastSelected \in Selections

ZeroSourceSentinelIsPreCut ==
  ~targetHasPhysicalSource
    => SourcePhysicalOrdinal("Target") = 0

PostCutReplayCannotPrecedeTarget ==
  ~PairwisePhysicalPrecedes("Replay", "Target")

FrozenTargetPrecedesPostCutReplay ==
  PairwisePhysicalPrecedes("Target", "Replay")

CausalSuccessorRetainsPostCutPhysicalRoot ==
  replayStage # "Completion" \/ replaySourcePhysicalOrdinal = 2

CurrentIngressTurnSelectsExactCarrier ==
  phase # "Ingress" \/ lastSelected # "Replay"

TimeoutCutFilterNeverSelectsPostCutReplay ==
  phase # "TimeoutSelection" \/ lastSelected # "Replay"

EventuallyExactTargetCompletes == <>targetDone

=============================================================================
