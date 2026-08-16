---- MODULE SumeragiV2ChainEpochRefinementShard16 ----
EXTENDS SumeragiV2ChainEpochRefinementShard15

THEOREM ActivatedSuccessorHasExactStateProjection ==
  \A parentContext \in AdmissibleContextRecords,
     node \in ValidatorIds:
    SuccessorHeightActivated(parentContext, node)
      => /\ parentContext.height < MaxHeight
         /\ node \in joinedByContext[
                      CanonicalIndexedContext(parentContext.height + 1)]
         /\ successorPredecessorStatusOwnership[parentContext][node]
              = "Absent"
BY DEF SuccessorHeightActivated

FiniteHorizonExactHistoricalRecoveryProjectionInvariant ==
  \A terminalContext \in AdmissibleContextRecords,
     node \in Responsive:
    terminalContext.height = MaxHeight
      /\ IndexedAsync(terminalContext)!NodeHasApplication(node)
      => /\ nodeHeight[node] = terminalContext.height
         /\ nodeContext[node] = terminalContext
         /\ successorActivationStatus[terminalContext][node] = "Idle"
         /\ successorPredecessorStatusOwnership[terminalContext][node]
              = "Absent"

THEOREM IndexedChainPreservesFiniteHorizonExactRecoveryProjection ==
  IndexedChainSpec => []FiniteHorizonExactHistoricalRecoveryProjectionInvariant
PROOF
  <1>1. IndexedChainSpec => []IndexedCompositionInvariant
    BY IndexedChainSpecEstablishesCompositionInvariant
  <1>2. IndexedChainSpec => []FiniteHorizonSuccessorProjectionDormant
    BY IndexedChainAlwaysExcludesTerminalActivation
  <1>3. IndexedCompositionInvariant
           /\ FiniteHorizonSuccessorProjectionDormant
           => FiniteHorizonExactHistoricalRecoveryProjectionInvariant
    BY Isa DEF IndexedCompositionInvariant,
               IndexedTerminalExactApplicationBoundaryInvariant,
               FiniteHorizonSuccessorProjectionDormant,
               FiniteHorizonExactHistoricalRecoveryProjectionInvariant,
               ExactNodeLocationAt
  <1> QED BY <1>1, <1>2, <1>3, PTL

SuccessorActivationAndExactHistoricalRecoveryProductionRefinementInvariant ==
  /\ SuccessorActivationShape
  /\ \A parentContext \in AdmissibleContextRecords,
       node \in ValidatorIds:
       SuccessorHeightActivated(parentContext, node)
         => /\ parentContext.height < MaxHeight
            /\ node \in joinedByContext[
                         CanonicalIndexedContext(
                           parentContext.height + 1)]
            /\ successorPredecessorStatusOwnership[parentContext][node]
                 = "Absent"

THEOREM IndexedChainSpecEstablishesSuccessorActivationAndExactHistoricalRecoveryInvariant ==
  IndexedChainSpec
    => []SuccessorActivationAndExactHistoricalRecoveryProductionRefinementInvariant
PROOF
  <1>1. ASSUME IndexedChainSpec
         PROVE
           []SuccessorActivationAndExactHistoricalRecoveryProductionRefinementInvariant
    <2>1. []IndexedCompositionInvariant
      BY <1>1, IndexedChainSpecEstablishesCompositionInvariant, PTL
    <2> QED BY <2>1, PTL, Isa
         DEF SuccessorActivationAndExactHistoricalRecoveryProductionRefinementInvariant,
             IndexedCompositionInvariant,
             SuccessorHeightActivated
  <1> QED BY <1>1

(***************************************************************************
External production-trace evidence is deliberately represented separately
from the model-side invariant above. These six booleans are not assigned by
this module: source-order checks, adversarial tests, and source-manifest
binding can constrain the trace claims, but none of those artifacts alone
proves them.  The conditional theorem below composes the separately checked
trace claims with the deductive model invariant; it does not manufacture any
of the six premises. `MaxHeight` is absent: it is a finite-horizon projection
parameter and has no production trace counterpart.

Keeping the source seam in the theorem statement prevents the already-proved
abstract invariant from being reused as a vacuous Rust-to-TLA refinement.
***************************************************************************)
ProductionSuccessorAndExactRecoveryTraceRefinement ==
  /\ ProductionAppliedSuccessorTraceRefinesIndexedActivation = TRUE
  /\ ProductionRecoveredSuccessorTraceRefinesIndexedActivation = TRUE
  /\ ProductionStartupFailureAndRestartRefinesIndexedLifecycle = TRUE
  /\ ProductionHistoricalCertificateTraceRefinesIndexedAsync = TRUE
  /\ ProductionHistoricalBodyPipelineTraceRefinesIndexedAsync = TRUE
  /\ ProductionTerminalApplicationWithoutSuccessorActivationTraceRefinesIndexedTerminal = TRUE

(***************************************************************************
This is the deliberately explicit Rust-to-TLA refinement seam. Its discharge
must connect the production open_deferred_status adapter, serialized runtime,
effect executor, service startup, startup/recovery effect consumption, clock
arming, exact marker preparation, authenticated ingress opening, and final
Applied/Recovered publication to the ordered actions above. It also must map
block-sync recovery to `OpenHistoricalRecovery` and the exact Async
decision/body/store/validate/apply deltas. Finite-horizon stuttering is proved
only as an internal projection and is not a production claim. The ledger records
this obligation as `cross_tool_proved`; release acceptance still requires a
fresh machine-checked trace mapping bound to the exact source. The model-internal
activated-state and finite-horizon projections are proved above;
they do not by themselves establish that a Rust execution refines these TLA+
actions.  In particular, this declaration remains the external trace sentinel
rather than being discharged from the state-side invariants alone.
***************************************************************************)
SuccessorActivationAndExactHistoricalRecoveryProductionRefinementObligation ==
  /\ ProductionSuccessorAndExactRecoveryTraceRefinement
  /\ (IndexedChainSpec
        => []SuccessorActivationAndExactHistoricalRecoveryProductionRefinementInvariant)

THEOREM SuccessorActivationAndExactHistoricalRecoveryCrossToolRefinement ==
  ProductionSuccessorAndExactRecoveryTraceRefinement
    => SuccessorActivationAndExactHistoricalRecoveryProductionRefinementObligation
PROOF
  BY IndexedChainSpecEstablishesSuccessorActivationAndExactHistoricalRecoveryInvariant
     DEF SuccessorActivationAndExactHistoricalRecoveryProductionRefinementObligation

(***************************************************************************
Exact indexed multi-height release theorem for the arbitrary free
VerificationContext. Natural induction recovers every responsive node through
its canonical ancestors using only authenticated exact historical targets. At the
target frontier, either the fixed one-height instance applies or a higher
canonical context becomes joined; in the latter case the exact recovery
obligation moves every lagging responsive node past the target.
***************************************************************************)
THEOREM HeightLivenessFromOneHeightAndExactRecoveryProgress ==
  /\ IndexedLiveChainSpec
  /\ IndexedGstEventuallyCondition
  /\ IndexedExactHistoricalRecoveryProgress
  /\ IndexedSuccessorActivationProgress
  /\ VerificationOneHeightCompletion
  => IndexedHeightLivenessProperty
PROOF
  <1>1. ASSUME IndexedLiveChainSpec,
              IndexedGstEventuallyCondition,
              IndexedExactHistoricalRecoveryProgress,
              IndexedSuccessorActivationProgress,
              VerificationOneHeightCompletion
         PROVE IndexedHeightLivenessProperty
    <2>0. IndexedChainSpec
      BY <1>1, IndexedLiveChainSpecProjectsIndexedChainSpec
    <2>1. CASE VerificationContext \in AdmissibleContextRecords
      <3>1. IndexedTargetJoined(VerificationContext)
               ~> (/\ IndexedTargetJoined(VerificationContext)
                   /\ IndexedResponsiveHeightReached(
                        VerificationContext.height)
                   /\ VerificationFrontierEscape)
        BY <1>1, <2>1,
           VerificationJoinedTargetEventuallyReachesAndEscapes
           DEF IndexedGstEventuallyCondition
      <3>2. (/\ IndexedTargetJoined(VerificationContext)
              /\ IndexedResponsiveHeightReached(
                   VerificationContext.height)
              /\ VerificationFrontierEscape)
               ~> IndexedContextCompleted(VerificationContext)
        BY <1>1, <2>0, <2>1,
           VerificationReachedEscapeEventuallyCompletes
      <3> QED BY <3>1, <3>2, PTL
           DEF IndexedHeightLivenessProperty, IndexedTargetJoined
    <2>2. CASE VerificationContext \notin AdmissibleContextRecords
      BY <2>2 DEF IndexedHeightLivenessProperty
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

(***************************************************************************
The release-facing theorem lives in SumeragiV2ChainLivenessProofs, a child of
the successor-activation proof module. Keeping only the target proposition in
this base module avoids the former impossible parent-to-child dependency while
leaving the conditional finite-height kernel above reusable.
***************************************************************************)
IndexedHeightLivenessReleaseTarget ==
  /\ IndexedLiveChainSpec
  /\ IndexedGstEventuallyCondition
  => IndexedHeightLivenessProperty


=============================================================================
