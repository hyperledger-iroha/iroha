---- MODULE SumeragiV2RetainedEffectBatchMutation ----
EXTENDS Naturals, Sequences

(***************************************************************************
Finite mutation matrix for the bounded retained AdapterEffect batch.

The four scenarios are intentionally separate executions:

  PartialFifo    dispatches a harmless prefix, retains Sign/Report behind
                 full work capacity, then drains that exact FIFO tail;
  SecondBatch    rejects and fail-closes an overtaking reducer batch without
                 changing the already-retained causal suffix;
  DecisionFilter installs durable Decision and removes stale Sign/proposal
                 effects while retaining exact CommitQC broadcast/diagnostic;
  SourceBound    accepts and drains exactly eight effects, then rejects a
                 nine-effect transition against the source bound 8.

Separating the fail-closed scenarios avoids pretending that an executor may
continue after a fatal contract rejection.  Crash/restart reconstruction is
delegated to SumeragiV2CrashReplayMutation.  These finite TLC checks provide
mutation evidence for dispatch mechanics, not deductive liveness closure.
***************************************************************************)

CONSTANTS Scenario, BatchPolicy

PartialFifoScenario == "PartialFifo"
SecondBatchScenario == "SecondBatch"
DecisionFilterScenario == "DecisionFilter"
SourceBoundScenario == "SourceBound"

FixedPolicy == "Fixed"
ReverseDrainPolicy == "ReverseDrain"
AcceptSecondBatchPolicy == "AcceptSecondBatch"
NoDecisionFilterPolicy == "NoDecisionFilter"
AcceptOversizePolicy == "AcceptOversize"

ASSUME Scenario \in
  {PartialFifoScenario,
   SecondBatchScenario,
   DecisionFilterScenario,
   SourceBoundScenario}

ASSUME BatchPolicy \in
  {FixedPolicy,
   ReverseDrainPolicy,
   AcceptSecondBatchPolicy,
   NoDecisionFilterPolicy,
   AcceptOversizePolicy}

ProposalBroadcast == "ProposalBroadcast"
TimeoutVoteSign == "TimeoutVoteSign"
EquivocationReport == "EquivocationReport"
OvertakingBroadcast == "OvertakingBroadcast"
StaleProposalBroadcast == "StaleProposalBroadcast"
ExactCommitQCBroadcast == "ExactCommitQCBroadcast"

Effects ==
  {ProposalBroadcast,
   TimeoutVoteSign,
   EquivocationReport,
   OvertakingBroadcast,
   StaleProposalBroadcast,
   ExactCommitQCBroadcast}

FirstBatch ==
  <<ProposalBroadcast, TimeoutVoteSign, EquivocationReport>>

FirstBlockedTail == <<TimeoutVoteSign, EquivocationReport>>

OvertakingBatch == <<OvertakingBroadcast>>

DecisionInput ==
  <<TimeoutVoteSign,
    StaleProposalBroadcast,
    ExactCommitQCBroadcast,
    EquivocationReport>>

DecisionSurvivors == <<ExactCommitQCBroadcast, EquivocationReport>>

EightEffectBatch ==
  <<ProposalBroadcast,
    ProposalBroadcast,
    ProposalBroadcast,
    ProposalBroadcast,
    ProposalBroadcast,
    ProposalBroadcast,
    ProposalBroadcast,
    ProposalBroadcast>>

NineEffectBatch == Append(EightEffectBatch, ProposalBroadcast)

ReducerMaxEffectsPerStep == 8
WorkCapacity == 1

VARIABLES phase,
          retainedEffects,
          dispatchedEffects,
          pendingWork,
          secondBatchAttempted,
          secondBatchRejected,
          decisionInstalled,
          oversizeAttempted,
          oversizeRejected,
          fatal

vars ==
  <<phase,
    retainedEffects,
    dispatchedEffects,
    pendingWork,
    secondBatchAttempted,
    secondBatchRejected,
    decisionInstalled,
    oversizeAttempted,
    oversizeRejected,
    fatal>>

TerminalPhase ==
  CASE Scenario = PartialFifoScenario -> 4
    [] Scenario = SecondBatchScenario -> 2
    [] Scenario = DecisionFilterScenario -> 3
    [] Scenario = SourceBoundScenario -> 3

TypeInvariant ==
  /\ phase \in 0..4
  /\ retainedEffects \in Seq(Effects)
  /\ dispatchedEffects \in Seq(Effects)
  /\ pendingWork \in 0..WorkCapacity
  /\ secondBatchAttempted \in BOOLEAN
  /\ secondBatchRejected \in BOOLEAN
  /\ decisionInstalled \in BOOLEAN
  /\ oversizeAttempted \in BOOLEAN
  /\ oversizeRejected \in BOOLEAN
  /\ fatal \in BOOLEAN

RetainedBatchWithinSourceBound ==
  Len(retainedEffects) <= ReducerMaxEffectsPerStep

PartialDrainIsExactFifoPrefix ==
  (Scenario = PartialFifoScenario /\ phase = 2) =>
    /\ dispatchedEffects = <<ProposalBroadcast>>
    /\ retainedEffects = FirstBlockedTail
    /\ pendingWork = WorkCapacity

PartialCompletionPreservesWholeFifo ==
  (Scenario = PartialFifoScenario /\ phase = 4) =>
    /\ dispatchedEffects = FirstBatch
    /\ retainedEffects = <<>>

SecondBatchRejectedBeforeTailMutation ==
  (Scenario = SecondBatchScenario /\ phase = 2) =>
    /\ secondBatchAttempted
    /\ secondBatchRejected
    /\ fatal
    /\ retainedEffects = FirstBlockedTail
    /\ dispatchedEffects = <<>>

DecisionTailContainsExactlySurvivors ==
  (Scenario = DecisionFilterScenario /\ phase = 2) =>
    /\ decisionInstalled
    /\ retainedEffects = DecisionSurvivors

DecisionSurvivorsDrainInFifoOrder ==
  (Scenario = DecisionFilterScenario /\ phase = 3) =>
    /\ dispatchedEffects = DecisionSurvivors
    /\ retainedEffects = <<>>

MaximumSizedBatchIsAccepted ==
  (Scenario = SourceBoundScenario /\ phase = 1) =>
    /\ retainedEffects = EightEffectBatch
    /\ Len(retainedEffects) = ReducerMaxEffectsPerStep
    /\ ~fatal

MaximumSizedBatchDrainsExactly ==
  (Scenario = SourceBoundScenario /\ phase = 2) =>
    /\ retainedEffects = <<>>
    /\ dispatchedEffects = EightEffectBatch

OversizeBatchRejectedBeforeInstall ==
  (Scenario = SourceBoundScenario /\ phase = 3) =>
    /\ oversizeAttempted
    /\ oversizeRejected
    /\ fatal
    /\ retainedEffects = <<>>

Init ==
  /\ phase = 0
  /\ retainedEffects = <<>>
  /\ dispatchedEffects = <<>>
  /\ pendingWork = WorkCapacity
  /\ secondBatchAttempted = FALSE
  /\ secondBatchRejected = FALSE
  /\ decisionInstalled = FALSE
  /\ oversizeAttempted = FALSE
  /\ oversizeRejected = FALSE
  /\ fatal = FALSE

InstallPartialFifoBatch ==
  /\ Scenario = PartialFifoScenario
  /\ phase = 0
  /\ retainedEffects' = FirstBatch
  /\ phase' = 1
  /\ UNCHANGED
       <<dispatchedEffects,
         pendingWork,
         secondBatchAttempted,
         secondBatchRejected,
         decisionInstalled,
         oversizeAttempted,
         oversizeRejected,
         fatal>>

DrainAvailablePrefixUntilCapacity ==
  /\ Scenario = PartialFifoScenario
  /\ phase = 1
  /\ retainedEffects = FirstBatch
  /\ pendingWork = WorkCapacity
  /\ IF BatchPolicy = ReverseDrainPolicy
       THEN /\ dispatchedEffects' = <<EquivocationReport>>
            /\ retainedEffects' = <<ProposalBroadcast, TimeoutVoteSign>>
       ELSE /\ dispatchedEffects' = <<ProposalBroadcast>>
            /\ retainedEffects' = FirstBlockedTail
  /\ phase' = 2
  /\ UNCHANGED
       <<pendingWork,
         secondBatchAttempted,
         secondBatchRejected,
         decisionInstalled,
         oversizeAttempted,
         oversizeRejected,
         fatal>>

FairlyRetireInitialWork ==
  /\ Scenario = PartialFifoScenario
  /\ phase = 2
  /\ pendingWork = WorkCapacity
  /\ pendingWork' = 0
  /\ phase' = 3
  /\ UNCHANGED
       <<retainedEffects,
         dispatchedEffects,
         secondBatchAttempted,
         secondBatchRejected,
         decisionInstalled,
         oversizeAttempted,
         oversizeRejected,
         fatal>>

DrainRetainedFifoTail ==
  /\ Scenario = PartialFifoScenario
  /\ phase = 3
  /\ BatchPolicy # ReverseDrainPolicy
  /\ retainedEffects = FirstBlockedTail
  /\ pendingWork = 0
  /\ retainedEffects' = <<>>
  /\ dispatchedEffects' = dispatchedEffects \o FirstBlockedTail
  /\ pendingWork' = WorkCapacity
  /\ phase' = 4
  /\ UNCHANGED
       <<secondBatchAttempted,
         secondBatchRejected,
         decisionInstalled,
         oversizeAttempted,
         oversizeRejected,
         fatal>>

InstallBlockedTailForSecondBatch ==
  /\ Scenario = SecondBatchScenario
  /\ phase = 0
  /\ retainedEffects' = FirstBlockedTail
  /\ phase' = 1
  /\ UNCHANGED
       <<dispatchedEffects,
         pendingWork,
         secondBatchAttempted,
         secondBatchRejected,
         decisionInstalled,
         oversizeAttempted,
         oversizeRejected,
         fatal>>

AttemptOvertakingSecondBatch ==
  /\ Scenario = SecondBatchScenario
  /\ phase = 1
  /\ retainedEffects = FirstBlockedTail
  /\ secondBatchAttempted' = TRUE
  /\ IF BatchPolicy = AcceptSecondBatchPolicy
       THEN /\ retainedEffects' = OvertakingBatch
            /\ secondBatchRejected' = FALSE
            /\ fatal' = FALSE
       ELSE /\ retainedEffects' = retainedEffects
            /\ secondBatchRejected' = TRUE
            /\ fatal' = TRUE
  /\ phase' = 2
  /\ UNCHANGED
       <<dispatchedEffects,
         pendingWork,
         decisionInstalled,
         oversizeAttempted,
         oversizeRejected>>

InstallDecisionBlockedBatch ==
  /\ Scenario = DecisionFilterScenario
  /\ phase = 0
  /\ retainedEffects' = DecisionInput
  /\ phase' = 1
  /\ UNCHANGED
       <<dispatchedEffects,
         pendingWork,
         secondBatchAttempted,
         secondBatchRejected,
         decisionInstalled,
         oversizeAttempted,
         oversizeRejected,
         fatal>>

InstallDecisionAndFilterRetainedTail ==
  /\ Scenario = DecisionFilterScenario
  /\ phase = 1
  /\ retainedEffects = DecisionInput
  /\ decisionInstalled' = TRUE
  /\ retainedEffects' =
       IF BatchPolicy = NoDecisionFilterPolicy
         THEN retainedEffects
         ELSE DecisionSurvivors
  /\ phase' = 2
  /\ UNCHANGED
       <<dispatchedEffects,
         pendingWork,
         secondBatchAttempted,
         secondBatchRejected,
         oversizeAttempted,
         oversizeRejected,
         fatal>>

DrainDecisionSurvivors ==
  /\ Scenario = DecisionFilterScenario
  /\ phase = 2
  /\ BatchPolicy # NoDecisionFilterPolicy
  /\ retainedEffects = DecisionSurvivors
  /\ retainedEffects' = <<>>
  /\ dispatchedEffects' = DecisionSurvivors
  /\ phase' = 3
  /\ UNCHANGED
       <<pendingWork,
         secondBatchAttempted,
         secondBatchRejected,
         decisionInstalled,
         oversizeAttempted,
         oversizeRejected,
         fatal>>

InstallMaximumSizedBatch ==
  /\ Scenario = SourceBoundScenario
  /\ phase = 0
  /\ retainedEffects' = EightEffectBatch
  /\ phase' = 1
  /\ UNCHANGED
       <<dispatchedEffects,
         pendingWork,
         secondBatchAttempted,
         secondBatchRejected,
         decisionInstalled,
         oversizeAttempted,
         oversizeRejected,
         fatal>>

DrainMaximumSizedBatch ==
  /\ Scenario = SourceBoundScenario
  /\ phase = 1
  /\ retainedEffects = EightEffectBatch
  /\ retainedEffects' = <<>>
  /\ dispatchedEffects' = EightEffectBatch
  /\ phase' = 2
  /\ UNCHANGED
       <<pendingWork,
         secondBatchAttempted,
         secondBatchRejected,
         decisionInstalled,
         oversizeAttempted,
         oversizeRejected,
         fatal>>

AttemptOversizeBatch ==
  /\ Scenario = SourceBoundScenario
  /\ phase = 2
  /\ oversizeAttempted' = TRUE
  /\ IF BatchPolicy = AcceptOversizePolicy
       THEN /\ retainedEffects' = NineEffectBatch
            /\ oversizeRejected' = FALSE
            /\ fatal' = FALSE
       ELSE /\ retainedEffects' = <<>>
            /\ oversizeRejected' = TRUE
            /\ fatal' = TRUE
  /\ phase' = 3
  /\ UNCHANGED
       <<dispatchedEffects,
         pendingWork,
         secondBatchAttempted,
         secondBatchRejected,
         decisionInstalled>>

Next ==
  \/ InstallPartialFifoBatch
  \/ DrainAvailablePrefixUntilCapacity
  \/ FairlyRetireInitialWork
  \/ DrainRetainedFifoTail
  \/ InstallBlockedTailForSecondBatch
  \/ AttemptOvertakingSecondBatch
  \/ InstallDecisionBlockedBatch
  \/ InstallDecisionAndFilterRetainedTail
  \/ DrainDecisionSurvivors
  \/ InstallMaximumSizedBatch
  \/ DrainMaximumSizedBatch
  \/ AttemptOversizeBatch

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(Next)

ScenarioEventuallyReachesTerminal == (phase = 0) ~> (phase = TerminalPhase)

=============================================================================
