---- MODULE SumeragiV2SuccessorActivationRefinementProofs ----
EXTENDS SumeragiV2ChainEpochRefinement, TLAPS

(***************************************************************************
The finite verification horizon has no successor context.  A terminal
historical application must therefore remain an observer/application receipt:
none of the production-shaped activation actions may create a predecessor
owner, token, marker, prerequisite, or joined successor for that context.
***************************************************************************)
TerminalSuccessorDormancyInvariant ==
  \A terminalContext \in AdmissibleContextRecords,
     node \in ValidatorIds:
    terminalContext.height = MaxHeight
      => /\ successorActivationStatus[terminalContext][node] = "Idle"
         /\ successorPredecessorStatusOwnership[terminalContext][node]
              = "Absent"

THEOREM IndexedInitEstablishesTerminalSuccessorDormancy ==
  IndexedChainInit => TerminalSuccessorDormancyInvariant
BY Isa DEF IndexedChainInit, TerminalSuccessorDormancyInvariant

THEOREM IndexedActionPreservesTerminalSuccessorDormancy ==
  TerminalSuccessorDormancyInvariant /\ IndexedChainNext
    => TerminalSuccessorDormancyInvariant'
BY Isa DEF TerminalSuccessorDormancyInvariant,
           IndexedChainNext, IndexedProductActionAt,
           IndexedReceiptClassification,
           IndexedReceiptFreeChainStutter,
           IndexedDecisionReceiptHandoff,
           IndexedApplicationReceiptHandoff,
           QueueSuccessorActivation,
           IndexedHistoricalCatchUpPipelineAction,
           IndexedHistoricalCatchUpDecision,
           IndexedHistoricalCatchUpBodyRecovery,
           IndexedHistoricalCatchUpBodyStore,
           IndexedHistoricalCatchUpValidation,
           IndexedHistoricalCatchUpApplication,
           IndexedHistoricalCatchUpNonterminalApplication,
           IndexedHistoricalCatchUpTerminalApplication,
           IndexedSuccessorActivationProgressStep,
           BeginSuccessorActivation,
           BindAppliedSuccessorActivationToken,
           FailClosedSuccessorStartup,
           AuthenticateRecoveredSuccessorActivation,
           OpenDeferredSuccessorAdapter,
           ConstructSuccessorRuntime,
           StartSuccessorServices,
           ApplySuccessorStartupEffects,
           ArmSuccessorClocks,
           PrepareSuccessorActivationMarker,
           OpenSuccessorIngress,
           ActivateAppliedSuccessorHeight,
           ActivateRecoveredSuccessorHeight,
           SuccessorActivationEnvironmentStutter,
           CanonicalIndexedContext,
           AdmissibleContextRecords, FrozenContextAdmissible,
           ContextRecords, Heights

THEOREM IndexedStepPreservesTerminalSuccessorDormancy ==
  TerminalSuccessorDormancyInvariant
    /\ [IndexedChainNext]_IndexedChainVars
    => TerminalSuccessorDormancyInvariant'
PROOF
  <1>1. CASE IndexedChainNext
    BY <1>1, IndexedActionPreservesTerminalSuccessorDormancy
  <1>2. CASE UNCHANGED IndexedChainVars
    BY <1>2, Isa
       DEF IndexedChainVars, TerminalSuccessorDormancyInvariant
  <1> QED BY <1>1, <1>2

THEOREM IndexedChainSpecEstablishesTerminalSuccessorDormancy ==
  IndexedChainSpec => []TerminalSuccessorDormancyInvariant
PROOF
  <1>1. IndexedChainInit => TerminalSuccessorDormancyInvariant
    BY IndexedInitEstablishesTerminalSuccessorDormancy
  <1>2. TerminalSuccessorDormancyInvariant
           /\ [IndexedChainNext]_IndexedChainVars
           => TerminalSuccessorDormancyInvariant'
    BY IndexedStepPreservesTerminalSuccessorDormancy
  <1> QED BY <1>1, <1>2, PTL DEF IndexedChainSpec

(***************************************************************************
Strict discharge of the abstract production-shaped invariant.  The separate
source-refinement gate must additionally bind these action names and fields to
the executable Rust transition corridor before the proof ledger may promote
the Rust-to-TLA production refinement obligation.
***************************************************************************)
THEOREM AbstractSuccessorActivationAndHistoricalCatchUpInvariant ==
  IndexedChainSpec
    => []SuccessorActivationAndHistoricalCatchUpProductionRefinementInvariant
PROOF
  <1>1. ASSUME IndexedChainSpec
         PROVE []SuccessorActivationAndHistoricalCatchUpProductionRefinementInvariant
    <2>1. []IndexedCompositionInvariant
      BY <1>1, IndexedChainSpecEstablishesCompositionInvariant, PTL
    <2>2. []TerminalSuccessorDormancyInvariant
      BY <1>1, IndexedChainSpecEstablishesTerminalSuccessorDormancy, PTL
    <2> QED BY <2>1, <2>2, PTL, Isa
         DEF SuccessorActivationAndHistoricalCatchUpProductionRefinementInvariant,
             IndexedCompositionInvariant,
             HistoricalCatchUpEvidenceInvariant,
             HistoricalCatchUpApplicationAt,
             SuccessorHeightActivated
  <1> QED BY <1>1

=============================================================================
