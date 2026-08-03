---- MODULE SumeragiV2AsyncRankAndInitProofs ----
EXTENDS SumeragiV2ServiceRankLemmas,
        SumeragiV2AsyncFairnessRefinementProofs,
        SumeragiV2DurableDecisionRecoveryProofs,
        SumeragiV2EffectiveLockAcquisitionProofs,
        SequenceTheorems,
        FunctionTheorems

(***************************************************************************
The eleven external production propositions below are the explicit
implementation-refinement seam.  They are deliberately not assigned values
here.  The four effective-lock claims and seven progress-witness claims retain
their independent seams.  The EnterView claim now carries the full locked
PrepareQC identity (reference, phase, signer set, quorum totals, and canonical
evidence class) through every persisted-TC position, so no separate quotient
proposition remains.  The internal recovery witness may transfer ownership
between exact QcRecords with the same full CertificateRef; that progress
quotient neither authenticates a signer set nor chooses the exact QC bytes
persisted in the WAL.
Keeping every proposition separate prevents the abstract asynchronous proof
from silently claiming that it has inspected production state which this
module does not model.
***************************************************************************)
CONSTANTS ProductionEnterViewUsesPostInstallEffectiveLock,
          ProductionBodyOwnershipPreservesEffectiveLock,
          ProductionBodyCapacityRetirementPreservesEffectiveLock,
          ProductionBodyServiceRefinesAsyncFairness,
          ProductionDurableIntentTraceRefinesProgressWitness,
          ProductionDecisionTraceRefinesRecoveryWitness,
          ProductionSchedulerTraceRefinesProtectedOwnership,
          ProductionIngressIdentityAndClassTraceRefinesProtectedOwnership,
          ProductionTwoStageRelayRetryTraceRefinesSourceFairness,
          ProductionReliableFlushTraceRefinesOutboundOwnership,
          ProductionApplicationTraceRefinesDecisionCompletion

ProductionEffectiveLockBodyAcquisitionRefinement ==
  /\ ProductionEnterViewUsesPostInstallEffectiveLock = TRUE
  /\ ProductionBodyOwnershipPreservesEffectiveLock = TRUE
  /\ ProductionBodyCapacityRetirementPreservesEffectiveLock = TRUE
  /\ ProductionBodyServiceRefinesAsyncFairness = TRUE

ProductionProgressWitnessTraceRefinement ==
  /\ ProductionDurableIntentTraceRefinesProgressWitness = TRUE
  /\ ProductionDecisionTraceRefinesRecoveryWitness = TRUE
  /\ ProductionSchedulerTraceRefinesProtectedOwnership = TRUE
  /\ ProductionIngressIdentityAndClassTraceRefinesProtectedOwnership = TRUE
  /\ ProductionTwoStageRelayRetryTraceRefinesSourceFairness = TRUE
  /\ ProductionReliableFlushTraceRefinesOutboundOwnership = TRUE
  /\ ProductionApplicationTraceRefinesDecisionCompletion = TRUE

ProductionHistoricalLockedBodyRecoveryRefinement ==
  /\ ProductionEffectiveLockBodyAcquisitionRefinement
  /\ ProductionDurableIntentTraceRefinesProgressWitness = TRUE

(***************************************************************************
Rank and fairness proof for the production-coupled asynchronous layer.

The lemmas start at the concrete service boundaries.  RuntimeReachRank counts
the remaining serialized run-loop invocations before the reducer phase;
ingress source rank follows the exact scan-and-rotate ready queue; IO rank is
the position in the single worker FIFO; causal rank combines its doubled FIFO
position with the debt-aware local-source cursor.  Recurring Control jobs are
appended and re-armed only after service, so they cannot increase the rank of an
already-admitted Serve or Consensus job.
***************************************************************************)

THEOREM LocalAdmissionStrictlyDecreasesRuntimeReach ==
  \A node \in ValidatorIds:
    AsyncTypeInvariant /\ LocalAdmissionStep(node)
      => RuntimeReachRank(node)' < RuntimeReachRank(node)
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncTypeInvariant,
                LocalAdmissionStep(node)
         PROVE RuntimeReachRank(node)' < RuntimeReachRank(node)
    <2>1. asyncRunnerPhase[node] = "Local"
      BY <1>1 DEF LocalAdmissionStep
    <2>2. /\ asyncRunnerPhase \in
                  [ValidatorIds -> {"Local", "Ingress", "Runtime"}]
           /\ asyncRunnerBudget \in
                  [ValidatorIds ->
                    0..(AsyncQueueCapacity + AsyncIngressCapacity)]
      BY <1>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant
    <2>3. AsyncConfiguration
      BY <1>1
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant
    <2>4. /\ asyncRunnerBudget[node] \in Nat
           /\ AsyncIngressCapacity \in Nat
      BY <1>1, <2>2, <2>3, SMT DEF AsyncConfiguration
    <2>5. CASE LocalAdmissionCanAdvance(node)
      <3>1. /\ asyncRunnerPhase' = asyncRunnerPhase
             /\ asyncRunnerBudget' =
                  [asyncRunnerBudget EXCEPT ![node] = @ - 1]
        BY <1>1, <2>5 DEF LocalAdmissionStep
      <3>2. RuntimeReachRank(node) =
               asyncRunnerBudget[node] + AsyncIngressCapacity + 2
        BY <2>1 DEF RuntimeReachRank
      <3>3. RuntimeReachRank(node)' =
               asyncRunnerBudget[node] - 1
                 + AsyncIngressCapacity + 2
        BY <2>1, <2>2, <3>1, Isa DEF RuntimeReachRank
      <3>4. asyncRunnerBudget[node] - 1
                 + AsyncIngressCapacity + 2
               < asyncRunnerBudget[node] + AsyncIngressCapacity + 2
        BY <2>4, SMT
      <3> QED BY <3>2, <3>3, <3>4
    <2>6. CASE ~LocalAdmissionCanAdvance(node)
      <3>1. /\ asyncRunnerPhase' =
                  [asyncRunnerPhase EXCEPT ![node] = "Ingress"]
             /\ asyncRunnerBudget' =
                  [asyncRunnerBudget EXCEPT
                     ![node] = AsyncIngressCapacity]
        BY <1>1, <2>6 DEF LocalAdmissionStep
      <3>2. RuntimeReachRank(node) =
               asyncRunnerBudget[node] + AsyncIngressCapacity + 2
        BY <2>1 DEF RuntimeReachRank
      <3>3. /\ asyncRunnerPhase'[node] = "Ingress"
             /\ asyncRunnerBudget'[node] = AsyncIngressCapacity
        BY <2>2, <3>1, Isa
      <3>4. RuntimeReachRank(node)' = AsyncIngressCapacity + 1
        BY <3>3, SMT DEF RuntimeReachRank
      <3>5. AsyncIngressCapacity + 1
               < asyncRunnerBudget[node] + AsyncIngressCapacity + 2
        BY <2>4, SMT
      <3> QED BY <3>2, <3>4, <3>5
    <2> QED BY <2>5, <2>6
  <1> QED BY <1>1

THEOREM SerializedLocalPredecessorStrictlyDecreasesRuntimeReach ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ SerializedLocalPrecedesServeIngressStep(node)
    => RuntimeReachRank(node)' < RuntimeReachRank(node)
BY SMT
   DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
       AsyncConfiguration,
       SerializedLocalPrecedesServeIngressStep,
       SelectedLocalAdmissionAdvance,
       AsyncOlderLocalLifecyclePrecedesServeIngress,
       RuntimeReachRank

THEOREM LocalTargetOnlyTurnStrictlyDecreasesRuntimeReach ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ asyncRunnerPhase[node] = "Local"
    /\ AsyncServeIngressTargetOnlyTurn(node)
    => RuntimeReachRank(node)' < RuntimeReachRank(node)
BY SMT
   DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
       AsyncConfiguration, AsyncServeIngressTargetOnlyTurn,
       RuntimeReachRank

THEOREM ProducerAdmissionRecordsCausalDebt ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ LocalAdmissionStep(node)
    /\ LocalAdmissionCanAdvance(node)
    /\ SelectedLocalSource(node) = "Producer"
    => /\ asyncNextLocalSource'[node] = "Causal"
       /\ asyncCausalAdmissionOwed'[node] =
            ((asyncCausalAdmissionOwed[node] = TRUE)
              \/ CausalQueueNonempty(node))
BY SMT
   DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
       AsyncLocalSources, LocalAdmissionStep,
       UpdateLocalAdmissionMetadata, OtherLocalSource

THEOREM SelectedLocalSourceCanAdmit ==
  \A node \in ValidatorIds:
    LocalAdmissionCanAdvance(node)
      => LocalSourceCanAdmit(node, SelectedLocalSource(node))
BY SMT
   DEF LocalAdmissionCanAdvance, SelectedLocalSource,
       PreferredLocalSource, LocalSourceCanAdmit, OtherLocalSource

THEOREM SelectedProducerCanAdvance ==
  \A node \in ValidatorIds:
    /\ LocalAdmissionCanAdvance(node)
    /\ SelectedLocalSource(node) = "Producer"
    => ProducerCompletionCanAdvance(node)
BY SelectedLocalSourceCanAdmit
   DEF LocalSourceCanAdmit

THEOREM SelectedProducerCanAdmit ==
  \A node \in ValidatorIds:
    /\ LocalAdmissionCanAdvance(node)
    /\ SelectedLocalSource(node) = "Producer"
    => ProducerCompletionCanAdmit(node)
BY SelectedProducerCanAdvance
   DEF ProducerCompletionCanAdvance

THEOREM LocalAdmissionMetadataUpdatePreservesType ==
  \A node \in ValidatorIds, source \in AsyncLocalSources:
    /\ asyncCausalAdmissionOwed \in [ValidatorIds -> BOOLEAN]
    /\ asyncNextLocalSource \in [ValidatorIds -> AsyncLocalSources]
    /\ UpdateLocalAdmissionMetadata(node, source)
    => /\ asyncCausalAdmissionOwed' \in [ValidatorIds -> BOOLEAN]
       /\ asyncNextLocalSource' \in [ValidatorIds -> AsyncLocalSources]
BY FunctionalUpdatePreservesType, SMTT(30)
   DEF UpdateLocalAdmissionMetadata, OtherLocalSource,
       AsyncLocalSources

THEOREM SelectedCausalCanAdvance ==
  \A node \in ValidatorIds:
    /\ LocalAdmissionCanAdvance(node)
    /\ SelectedLocalSource(node) = "Causal"
    => CausalHeadCanAdvance(node)
BY SelectedLocalSourceCanAdmit
   DEF LocalSourceCanAdmit

THEOREM CausalDebtSurvivesProducerFallback ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ asyncCausalAdmissionOwed[node] = TRUE
    /\ LocalAdmissionStep(node)
    /\ LocalAdmissionCanAdvance(node)
    /\ SelectedLocalSource(node) = "Producer"
    => asyncCausalAdmissionOwed'[node] = TRUE
BY SMT
   DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
       LocalAdmissionStep, UpdateLocalAdmissionMetadata

THEOREM OwedAdmissibleCausalCannotBeOvertaken ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ asyncCausalAdmissionOwed[node] = TRUE
    /\ asyncRunnerBudget[node] > 0
    /\ CausalHeadCanAdvance(node)
    /\ LocalAdmissionStep(node)
    => /\ SelectedLocalSource(node) = "Causal"
       /\ AdmitCausalHead(node)
       /\ asyncCausalAdmissionOwed'[node] = FALSE
       /\ asyncNextLocalSource'[node] = "Producer"
BY SMT
   DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
       AsyncLocalSources, LocalAdmissionStep, LocalAdmissionCanAdvance,
       SelectedLocalSource, PreferredLocalSource, LocalSourceCanAdmit,
       UpdateLocalAdmissionMetadata, OtherLocalSource

THEOREM IngressDrainStrictlyDecreasesRuntimeReach ==
  \A node \in ValidatorIds:
    AsyncTypeInvariant /\ IngressDrainStep(node)
      => RuntimeReachRank(node)' < RuntimeReachRank(node)
BY SMT DEF AsyncTypeInvariant, IngressDrainStep, RuntimeReachRank

THEOREM RuntimeReachRankWithinRunnerCycle ==
  AsyncTypeInvariant
    => \A node \in ValidatorIds:
         /\ RuntimeReachRank(node) \in Nat
         /\ RuntimeReachRank(node) < AsyncRunnerCycleBudget
PROOF
  <1>1. ASSUME AsyncTypeInvariant
         PROVE \A node \in ValidatorIds:
                 /\ RuntimeReachRank(node) \in Nat
                 /\ RuntimeReachRank(node) < AsyncRunnerCycleBudget
    <2>1. ASSUME NEW node \in ValidatorIds
           PROVE /\ RuntimeReachRank(node) \in Nat
                 /\ RuntimeReachRank(node) < AsyncRunnerCycleBudget
      <3>1. /\ asyncRunnerPhase[node]
                    \in {"Local", "Ingress", "Runtime"}
             /\ asyncRunnerBudget[node]
                    \in 0..(AsyncQueueCapacity + AsyncIngressCapacity)
             /\ AsyncQueueCapacity \in Nat \ {0}
             /\ AsyncIngressCapacity \in Nat \ {0}
        BY <1>1, <2>1
           DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
               AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
               AsyncConfiguration
      <3>2. CASE asyncRunnerPhase[node] = "Local"
        BY <3>1, <3>2, SMT
           DEF RuntimeReachRank, AsyncRunnerCycleBudget
      <3>3. CASE asyncRunnerPhase[node] = "Ingress"
        BY <3>1, <3>3, SMT
           DEF RuntimeReachRank, AsyncRunnerCycleBudget
      <3>4. CASE asyncRunnerPhase[node] = "Runtime"
        BY <3>1, <3>4, SMT
           DEF RuntimeReachRank, AsyncRunnerCycleBudget
      <3> QED BY <3>1, <3>2, <3>3, <3>4
    <2> QED BY <2>1
  <1> QED BY <1>1

(***************************************************************************
Target-aware reach to a drainable ingress turn.

`RuntimeReachRank` intentionally maps Runtime to zero and is therefore not a
rank across the Runtime-to-Local reset.  For an ingress owner which is known
to be drainable, the scheduler has a simpler acyclic path to its next service
turn:

  exhausted Ingress -> Runtime -> Local -> positive-budget Ingress.

The positive-budget Ingress state is rank zero because the next fair node
turn must consume the drainable owner.  The two reset states sit above every
typed Local budget.  This rank is deliberately not used for a
capacity-blocked certified-response claim; such a claim needs the additional
FIFO/deferred/tag debts in the temporal-rank module.
***************************************************************************)

DrainableIngressTurnReachRank(node) ==
  CASE /\ asyncRunnerPhase[node] = "Ingress"
          /\ asyncRunnerBudget[node] > 0 ->
         0
    [] asyncRunnerPhase[node] = "Ingress" ->
         AsyncQueueCapacity + AsyncIngressCapacity + 3
    [] asyncRunnerPhase[node] = "Runtime" ->
         AsyncQueueCapacity + AsyncIngressCapacity + 2
    [] OTHER ->
         asyncRunnerBudget[node] + 1

THEOREM DrainableIngressTurnReachRankIsNatural ==
  AsyncTypeInvariant
    => \A node \in ValidatorIds:
         /\ DrainableIngressTurnReachRank(node) \in Nat
         /\ DrainableIngressTurnReachRank(node)
              < 2 * (AsyncQueueCapacity + AsyncIngressCapacity) + 4
BY SMT
   DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
       AsyncConfiguration, DrainableIngressTurnReachRank

THEOREM LocalStepDecreasesDrainableIngressTurnReach ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ LocalAdmissionStep(node)
    => DrainableIngressTurnReachRank(node)'
         < DrainableIngressTurnReachRank(node)
BY SMT
   DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
       AsyncConfiguration, LocalAdmissionStep,
       DrainableIngressTurnReachRank

THEOREM SerializedLocalPredecessorDecreasesDrainableIngressTurnReach ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ SerializedLocalPrecedesServeIngressStep(node)
    => DrainableIngressTurnReachRank(node)'
         < DrainableIngressTurnReachRank(node)
BY SMT
   DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
       AsyncConfiguration,
       SerializedLocalPrecedesServeIngressStep,
       SelectedLocalAdmissionAdvance,
       AsyncOlderLocalLifecyclePrecedesServeIngress,
       DrainableIngressTurnReachRank

THEOREM ExhaustedIngressStepDecreasesDrainableIngressTurnReach ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ asyncRunnerPhase[node] = "Ingress"
    /\ asyncRunnerBudget[node] = 0
    /\ IngressDrainStep(node)
    => DrainableIngressTurnReachRank(node)'
         < DrainableIngressTurnReachRank(node)
BY SMT
   DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
       AsyncConfiguration, IngressDrainStep,
       DrainableIngressTurnReachRank

THEOREM RuntimeStepDecreasesDrainableIngressTurnReach ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ SerializedRuntimeStep(node)
    => DrainableIngressTurnReachRank(node)'
         < DrainableIngressTurnReachRank(node)
BY SMT
   DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
       AsyncConfiguration, SerializedRuntimeStep,
       DrainableIngressTurnReachRank

THEOREM OlderRuntimeInterleaveDecreasesDrainableIngressTurnReach ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ SerializedRuntimePrecedesServeIngressStep(node)
    => DrainableIngressTurnReachRank(node)'
         < DrainableIngressTurnReachRank(node)
BY SMT
   DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
       AsyncConfiguration,
       SerializedRuntimePrecedesServeIngressStep,
       DrainableIngressTurnReachRank

THEOREM ExactTicketTurnDecreasesDrainableIngressTurnReach ==
  \A node \in ValidatorIds:
    /\ AsyncTypeInvariant
    /\ AsyncServeIngressTargetOnlyTurn(node)
    => DrainableIngressTurnReachRank(node)'
         < DrainableIngressTurnReachRank(node)
BY SMT
   DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
       AsyncConfiguration, AsyncServeIngressTargetOnlyTurn,
       DrainableIngressTurnReachRank

THEOREM SchedulerRankArithmeticBound ==
  \A capacity \in Nat \ {0}:
    \A ordinal \in 1..capacity, distance \in 0..2:
      3 * ordinal + distance \in 3..(3 * capacity + 2)
BY SMT

THEOREM SchedulerClassPrefixRankBound ==
  \A node \in ValidatorIds:
    \A candidate:
      /\ AsyncTypeInvariant
      /\ candidate \in SequenceSet(asyncCommandQueues[node])
      => /\ Cardinality(SchedulerClassPrefixIndices(node, candidate))
                \in 1..AsyncQueueCapacity
         /\ CommandClassDistance(
              asyncNextCommandClass[node], candidate.class) \in 0..2
         /\ SchedulerServiceRank(node, candidate)
                \in 3..(3 * AsyncQueueCapacity + 2)
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds, NEW candidate,
                AsyncTypeInvariant,
                candidate \in SequenceSet(asyncCommandQueues[node])
         PROVE /\ Cardinality(
                      SchedulerClassPrefixIndices(node, candidate))
                        \in 1..AsyncQueueCapacity
               /\ CommandClassDistance(
                    asyncNextCommandClass[node], candidate.class) \in 0..2
               /\ SchedulerServiceRank(node, candidate)
                    \in 3..(3 * AsyncQueueCapacity + 2)
    <2>1. /\ AsyncQueueTyped(asyncCommandQueues[node])
           /\ AsyncQueueDepth(node) <= AsyncQueueCapacity
           /\ AsyncQueueCapacity \in Nat \ {0}
           /\ asyncNextCommandClass[node] \in AsyncCommandClasses
           /\ AsyncCandidateTyped(candidate)
      BY <1>1, SMT
         DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant, AsyncRuntimeScalarTypeInvariant,
             AsyncIoTypeInvariant, AsyncIoCapacityTypeInvariant,
             AsyncQueueTyped, SequenceSet
    <2>2. PICK matching \in 1..Len(asyncCommandQueues[node]):
             asyncCommandQueues[node][matching] = candidate
      BY <1>1 DEF SequenceSet
    <2>3. matching \in SchedulerClassPrefixIndices(node, candidate)
      BY <2>2 DEF SchedulerClassPrefixIndices,
                      SchedulerCandidateIndices
    <2>4. SchedulerClassPrefixIndices(node, candidate)
               \subseteq 1..Len(asyncCommandQueues[node])
      BY DEF SchedulerClassPrefixIndices
    <2>5. /\ Len(asyncCommandQueues[node]) \in Nat
           /\ IsFiniteSet(1..Len(asyncCommandQueues[node]))
           /\ Cardinality(1..Len(asyncCommandQueues[node]))
                = Len(asyncCommandQueues[node])
      BY <2>1, LenProperties, FS_Interval, SMT DEF AsyncQueueTyped
    <2>6. /\ IsFiniteSet(
                  SchedulerClassPrefixIndices(node, candidate))
           /\ Cardinality(SchedulerClassPrefixIndices(node, candidate))
                <= Len(asyncCommandQueues[node])
      BY <2>4, <2>5, FS_Subset
    <2>7. Cardinality(SchedulerClassPrefixIndices(node, candidate))
               \in Nat \ {0}
      BY <2>3, <2>6, FS_CardinalityType, FS_EmptySet, SMT
    <2>8. Cardinality(SchedulerClassPrefixIndices(node, candidate))
               \in 1..AsyncQueueCapacity
      BY <2>1, <2>6, <2>7, SMT DEF AsyncQueueDepth
    <2>9. CommandClassDistance(
             asyncNextCommandClass[node], candidate.class) \in 0..2
      BY <2>1, SMTT(30)
         DEF AsyncCandidateTyped, AsyncCommandClasses,
             CommandClassDistance, NextCommandClass
    <2> QED BY <1>1, <2>8, <2>9, SchedulerRankArithmeticBound
         DEF SchedulerServiceRank, AsyncTypeInvariant,
             AsyncSchedulerTypeInvariant, AsyncRuntimeTypeInvariant,
             AsyncRuntimeScalarTypeInvariant, AsyncConfiguration
  <1> QED BY <1>1

(***************************************************************************
Removing a serialized parent and appending its fresh causal children is an
ownership transfer, so the pre-state coalescing predicate is sound only when
the parent can never be one of its own children.  Keep the proof split over
the closed 28-kind parent inventory: adding a successor-producing reducer arm
must add both a source-fidelity CASE and a disjointness case here.
***************************************************************************)

THEOREM CandidateKindMismatchIsDistinct ==
  \A left, right:
    left.kind # right.kind => left # right
BY Isa

THEOREM CommandSuccessorParentDisjoint ==
  \A command:
    /\ AsyncCandidateTyped(command)
    /\ command.kind \in CausalSuccessorParentKinds
    => command \notin SequenceSet(CommandSuccessors(command))
PROOF
  <1>1. ASSUME NEW command,
                AsyncCandidateTyped(command),
                command.kind \in CausalSuccessorParentKinds
         PROVE command \notin SequenceSet(CommandSuccessors(command))
    <2>1. CASE command.kind = "AssembleBody"
      <3>1. CommandSuccessors(command) =
               IF ExactDecidedLocalBody(
                    command.node, command.view, command.subject)
               THEN <<CausalCandidate("Completion", "Apply", command)>>
               ELSE <<CausalCandidate(
                        "Completion", "BeginProposal", command)>>
        BY <2>1 DEF CommandSuccessors
      <3>2. CASE ExactDecidedLocalBody(
                    command.node, command.view, command.subject)
        <4>1. CommandSuccessors(command) =
                 <<CausalCandidate("Completion", "Apply", command)>>
          BY <3>1, <3>2, Isa
        <4>2. command.kind #
                 CausalCandidate("Completion", "Apply", command).kind
          BY <2>1, Isa
             DEF CausalCandidate, NoItemCandidate, AsyncCandidate
        <4>3. command #
                 CausalCandidate("Completion", "Apply", command)
          BY <4>2, CandidateKindMismatchIsDistinct
        <4>4. SequenceSet(CommandSuccessors(command)) =
                 {CausalCandidate("Completion", "Apply", command)}
          BY <4>1, Isa DEF SequenceSet
        <4> QED BY <4>3, <4>4
      <3>3. CASE ~ExactDecidedLocalBody(
                    command.node, command.view, command.subject)
        <4>1. CommandSuccessors(command) =
                 <<CausalCandidate(
                     "Completion", "BeginProposal", command)>>
          BY <3>1, <3>3, Isa
        <4>2. command.kind #
                 CausalCandidate(
                   "Completion", "BeginProposal", command).kind
          BY <2>1, Isa
             DEF CausalCandidate, NoItemCandidate, AsyncCandidate
        <4>3. command #
                 CausalCandidate(
                   "Completion", "BeginProposal", command)
          BY <4>2, CandidateKindMismatchIsDistinct
        <4>4. SequenceSet(CommandSuccessors(command)) =
                 {CausalCandidate(
                    "Completion", "BeginProposal", command)}
          BY <4>1, Isa DEF SequenceSet
        <4> QED BY <4>3, <4>4
      <3> QED BY <3>2, <3>3
    <2>2. CASE command.kind = "BeginProposal"
      BY <1>1, <2>2, Isa
         DEF CommandSuccessors, CausalCandidate,
             NoItemCandidate, SequenceSet
    <2>3. CASE command.kind = "PersistProposal"
      BY <1>1, <2>3, Isa
         DEF CommandSuccessors, CausalCandidate,
             NoItemCandidate, SequenceSet
    <2>4. CASE command.kind = "DeliverProposal"
      BY <1>1, <2>4, Isa
         DEF CommandSuccessors, RetainedBodyRebindCandidate,
             CausalCandidate, NoItemCandidate, SequenceSet
    <2>5. CASE command.kind = "DeliverChunk"
      BY <1>1, <2>5, Isa
         DEF CommandSuccessors, CausalCandidate,
             NoItemCandidate, SequenceSet
    <2>6. CASE command.kind = "FetchBody"
      BY <1>1, <2>6, Isa
         DEF CommandSuccessors, CausalCandidate,
             NoItemCandidate, SequenceSet
    <2>7. CASE command.kind = "RebindRetainedBody"
      BY <1>1, <2>7, Isa
         DEF CommandSuccessors, CausalCandidate,
             NoItemCandidate, SequenceSet
    <2>8. CASE command.kind = "FetchCertifiedBody"
      BY <1>1, <2>8, Isa
         DEF CommandSuccessors, CausalCandidate,
             NoItemCandidate, SequenceSet
    <2>9. CASE command.kind = "StoreBody"
      BY <1>1, <2>9, Isa
         DEF CommandSuccessors, CausalCandidate,
             NoItemCandidate, SequenceSet
    <2>10. CASE command.kind = "ValidateBody"
      BY <1>1, <2>10, Isa
         DEF CommandSuccessors, CausalCandidate,
             NoItemCandidate, SequenceSet
    <2>11. CASE command.kind = "BeginPrepare"
      BY <1>1, <2>11, Isa
         DEF CommandSuccessors, CausalCandidate,
             NoItemCandidate, SequenceSet
    <2>12. CASE command.kind = "PersistPrepare"
      BY <1>1, <2>12, Isa
         DEF CommandSuccessors, CausalCandidate,
             NoItemCandidate, SequenceSet
    <2>13. CASE command.kind = "DeliverVote"
      BY <1>1, <2>13, Isa
         DEF CommandSuccessors, CausalCandidate,
             NoItemCandidate, SequenceSet
    <2>14. CASE command.kind = "DeliverQC"
      BY <1>1, <2>14, Isa
         DEF CommandSuccessors, CausalCandidate,
             NoItemCandidate, SequenceSet
    <2>15. CASE command.kind = "BeginObservePrepare"
      BY <1>1, <2>15, Isa
         DEF CommandSuccessors, CausalCandidate,
             NoItemCandidate, SequenceSet
    <2>16. CASE command.kind = "PersistObservePrepare"
      BY <1>1, <2>16, Isa
         DEF CommandSuccessors, CausalCandidate,
             NoItemCandidate, SequenceSet
    <2>17. CASE command.kind = "BeginLockCommit"
      BY <1>1, <2>17, Isa
         DEF CommandSuccessors, CausalCandidate,
             NoItemCandidate, SequenceSet
    <2>18. CASE command.kind = "PersistLockCommit"
      BY <1>1, <2>18, Isa
         DEF CommandSuccessors, CausalCandidate,
             NoItemCandidate, SequenceSet
    <2>19. CASE command.kind = "FormCommitQC"
      BY <1>1, <2>19, Isa
         DEF CommandSuccessors, CausalCandidate,
             NoItemCandidate, SequenceSet
    <2>20. CASE command.kind = "BeginDecision"
      BY <1>1, <2>20, Isa
         DEF CommandSuccessors, CausalCandidate,
             NoItemCandidate, SequenceSet
    <2>21. CASE command.kind = "PersistDecision"
      BY <1>1, <2>21, Isa
         DEF CommandSuccessors, PersistDecisionRecoverySuccessor,
             PersistDecisionRecoveryKind, PersistDecisionBody,
             PersistDecisionValidationHeld, PersistDecisionRequest,
             AsyncCandidateCausalSuccessorWithIdentityAndOrigin,
             AsyncCandidateSuccessorProposalRound,
             AsyncCandidateWithIdentityAndOrigin,
             NoItemCandidate, SequenceSet
    <2>22. CASE command.kind = "BeginTimeout"
      BY <1>1, <2>22, Isa
         DEF CommandSuccessors, CausalCandidate,
             NoItemCandidate, SequenceSet
    <2>23. CASE command.kind = "PersistTimeout"
      BY <1>1, <2>23, Isa
         DEF CommandSuccessors, CausalCandidate,
             NoItemCandidate, SequenceSet
    <2>24. CASE command.kind = "DeliverTimeout"
      BY <1>1, <2>24, Isa
         DEF CommandSuccessors, CausalCandidate,
             NoItemCandidate, SequenceSet
    <2>25. CASE command.kind = "SignTimeout"
      BY <1>1, <2>25, Isa
         DEF CommandSuccessors, SignTimeoutFormsTC,
             CausalCandidateWithEvidence,
             AsyncCandidateCausalSuccessorWithIdentityAndOrigin,
             AsyncCandidateWithIdentityAndOrigin,
             NoItemCandidate, SequenceSet
    <2>26. CASE command.kind = "DeliverTC"
      BY <1>1, <2>26, Isa
         DEF CommandSuccessors, CausalCandidate,
             NoItemCandidate, SequenceSet
    <2>27. CASE command.kind = "BeginInstallTC"
      BY <1>1, <2>27, Isa
         DEF CommandSuccessors, CausalCandidate,
             NoItemCandidate, SequenceSet
    <2>28. CASE command.kind = "PersistInstallTC"
      BY <1>1, <2>28, Isa
         DEF CommandSuccessors, InstallCommandSuccessors,
             InstallLockedFetchSuccessors,
             InstallCommitSignSuccessors,
             InstallLockedFetchSuccessor,
             InstallCommitSignSuccessor, InstallProposalSuccessor,
             AsyncCandidateCausalSuccessorWithIdentityAndOrigin,
             AsyncCandidateSuccessorProposalRound,
             AsyncCandidateWithIdentityAndOrigin,
             NoItemCandidate, SequenceSet
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>5, <2>6,
         <2>7, <2>8, <2>9, <2>10, <2>11, <2>12, <2>13, <2>14,
         <2>15, <2>16, <2>17, <2>18, <2>19, <2>20, <2>21,
         <2>22, <2>23, <2>24, <2>25, <2>26, <2>27, <2>28
         DEF CausalSuccessorParentKinds
  <1> QED BY <1>1

THEOREM ControlWorkerRearmsExactlyAfterService ==
  \A node \in AsyncCurrentResponsiveVoters:
    (ServiceIoWorker(node)
      /\ Head(asyncIoQueues[node]).class = "Control")
      => asyncIoControlAvailable'[node]
BY SMT DEF ServiceIoWorker

THEOREM NonControlWorkerServiceDoesNotSpuriouslyRearm ==
  \A node \in AsyncArchiveIoServiceNodes:
    (ServiceIoWorker(node)
      /\ Head(asyncIoQueues[node]).class # "Control")
      => asyncIoControlAvailable' = asyncIoControlAvailable
BY SMT DEF ServiceIoWorker

THEOREM RecurringControlAppendsBehindAdmittedWork ==
  \A node \in AsyncCurrentResponsiveVoters:
    EnqueueIoLocalControl(node)
      => SubSeq(asyncIoQueues'[node], 1, AsyncIoQueueDepth(node))
           = asyncIoQueues[node]
BY Isa DEF EnqueueIoLocalControl, AsyncIoQueueDepth

THEOREM IoWorkerRemovesOnlyTheFifoHead ==
  \A node \in AsyncArchiveIoServiceNodes:
    ServiceIoWorker(node)
      => asyncIoQueues'[node] = Tail(asyncIoQueues[node])
BY SMT DEF ServiceIoWorker

THEOREM FirstDrainableSourceNeverFollowsAnotherDrainableSource ==
  \A node \in ValidatorIds:
    \A source \in SequenceSet(asyncIngressReady[node]):
      /\ IngressSourceCanDrain(node, source)
      /\ DrainableClaimedResponseReadyIndices(node) = {}
      /\ DrainableRequestFencedCompletionReadyIndices(node) = {}
        => FirstDrainableIngressIndex(node)
             <= IngressSourceServiceRank(node, source)
BY Isa DEF FirstDrainableIngressIndex, DrainableIngressIndices,
           IngressSourceServiceRank

THEOREM OverdueNodeServiceStopsPostGstClock ==
  \A node \in AsyncTimedServiceNodes:
    gst /\ asyncNodeServiceDeadlines[node] <= asyncNow
      => ~AsyncTickEnabled
BY SMT DEF AsyncTickEnabled

THEOREM OverdueIoServiceStopsPostGstClock ==
  \A node \in AsyncTimedServiceNodes:
    (gst /\ AsyncIoQueueDepth(node) > 0
      /\ asyncIoServiceDeadlines[node] <= asyncNow)
      => ~AsyncTickEnabled
BY SMT DEF AsyncTickEnabled

THEOREM OverdueResponsivePacketStopsPostGstClock ==
  \A packet \in asyncTransport:
    (gst
      /\ packet \in OverdueResponsivePackets)
      => ~AsyncTickEnabled
BY SMT DEF AsyncTickEnabled, OverdueResponsivePackets,
           AsyncPacketOwnsClockDeadline

THEOREM CertifiedRecoveryNeverRequestsSelf ==
  \A node \in ValidatorIds:
    \A qc \in QcRecordSet:
      \A item \in CertifiedRequestOutbox(node, qc):
        item.envelope.recipient # node
BY SMT DEF CertifiedRequestOutbox, AsyncNetworkItem, AsyncBodyEnvelope

THEOREM RetainedProposalRetryContainsEveryChunk ==
  \A node \in ValidatorIds, item \in asyncRetainedControl:
    (item.source = node /\ item.kind = "Proposal")
      => BroadcastChunkOutbox(node, item.envelope.proposal.view,
                              item.envelope.proposal.subject)
           \subseteq RetainedProposalChunks(node)
BY Isa DEF RetainedProposalChunks

(***************************************************************************
Deductive type closure for the scheduler product.  The Core component is
supplied by the parameterized Core Init/Next induction; this layer proves that
the concrete queues, reservations, deadlines, retained requests, transport,
and ingress topology are initialized and preserved as typed values.
***************************************************************************)

THEOREM AsyncInitEstablishesRuntimeScalarType ==
  \A initialContext:
    (AsyncInitAt(initialContext) /\ TypeInvariant)
      => AsyncRuntimeScalarTypeInvariant
BY SMTT(30)
   DEF AsyncInitAt, AsyncBaseInitAt, AsyncRuntimeInit,
       AsyncRuntimeScalarTypeInvariant, AsyncConfiguration,
       AsyncCommandQueueOwnership, AsyncQueueTyped, AsyncLocalSources,
       SequenceSet

AsyncCausalCoreTypingFacts ==
  /\ context \in ContextRecords
  /\ context.height \in Heights
  /\ nodeView \in [ValidatorIds -> Views]
  /\ generation \in [ValidatorIds -> Generations]
  /\ highestRank \in [ValidatorIds -> Ranks]
  /\ highestSubject \in [ValidatorIds -> SubjectOrNone]
  /\ AsyncHeartbeatSubject \in SubjectOrNone

THEOREM CoreTypeImpliesCausalTypingFacts ==
  TypeInvariant => AsyncCausalCoreTypingFacts
BY SMTT(30)
   DEF TypeInvariant, AsyncCausalCoreTypingFacts, ModelConfiguration,
       AsyncHeartbeatSubject, SubjectOrNone

InitialCausalCandidate(node) ==
  NoItemCandidate("Normal", "AssembleBody", node,
                  nodeView[node], AsyncProposalSubject(node))

(***************************************************************************
Exact candidate identity.  These are structural obligations, not hash
collision assumptions: every stored candidate field appears in the frozen
identity.  Therefore scheduler-wide coalescing can reject only the same
consumer epoch, payload/evidence, immutable causal origin, work, body,
manifest, and execution commitment.
***************************************************************************)

THEOREM ExactIdentityProjectsConsumerTag ==
  \A left, right:
    ExactAsyncCandidateIdentity(left) = ExactAsyncCandidateIdentity(right)
      => AsyncConsumerEventTag(left) = AsyncConsumerEventTag(right)
BY SMT DEF ExactAsyncCandidateIdentity

THEOREM ExactIdentityProjectsWorkIdentity ==
  \A left, right:
    ExactAsyncCandidateIdentity(left) = ExactAsyncCandidateIdentity(right)
      => AsyncWorkIdentity(left) = AsyncWorkIdentity(right)
BY SMT DEF ExactAsyncCandidateIdentity

THEOREM ExactIdentityProjectsDirectFields ==
  \A left, right:
    ExactAsyncCandidateIdentity(left) = ExactAsyncCandidateIdentity(right)
      => /\ left.item = right.item
         /\ left.evidence = right.evidence
         /\ left.causalOrigin = right.causalOrigin
         /\ left.bodyIdentity = right.bodyIdentity
         /\ left.manifestIdentity = right.manifestIdentity
         /\ left.commitmentIdentity = right.commitmentIdentity
BY SMT DEF ExactAsyncCandidateIdentity

THEOREM ConsumerTagEqualityProjectsContext ==
  \A left, right:
    AsyncConsumerEventTag(left) = AsyncConsumerEventTag(right)
      => left.consumerContext = right.consumerContext
BY SMT DEF AsyncConsumerEventTag

THEOREM ConsumerTagEqualityProjectsView ==
  \A left, right:
    AsyncConsumerEventTag(left) = AsyncConsumerEventTag(right)
      => left.consumerView = right.consumerView
BY SMT DEF AsyncConsumerEventTag

THEOREM ConsumerTagEqualityProjectsGeneration ==
  \A left, right:
    AsyncConsumerEventTag(left) = AsyncConsumerEventTag(right)
      => left.consumerGeneration = right.consumerGeneration
BY SMT DEF AsyncConsumerEventTag

THEOREM WorkIdentityEqualityProjectsClass ==
  \A left, right:
    AsyncWorkIdentity(left) = AsyncWorkIdentity(right)
      => left.class = right.class
BY SMT DEF AsyncWorkIdentity

THEOREM WorkIdentityEqualityProjectsKind ==
  \A left, right:
    AsyncWorkIdentity(left) = AsyncWorkIdentity(right)
      => left.kind = right.kind
BY SMT DEF AsyncWorkIdentity

THEOREM WorkIdentityEqualityProjectsNode ==
  \A left, right:
    AsyncWorkIdentity(left) = AsyncWorkIdentity(right)
      => left.node = right.node
BY SMT DEF AsyncWorkIdentity

THEOREM WorkIdentityEqualityProjectsHeight ==
  \A left, right:
    AsyncWorkIdentity(left) = AsyncWorkIdentity(right)
      => left.height = right.height
BY SMT DEF AsyncWorkIdentity

THEOREM WorkIdentityEqualityProjectsView ==
  \A left, right:
    AsyncWorkIdentity(left) = AsyncWorkIdentity(right)
      => left.view = right.view
BY SMT DEF AsyncWorkIdentity

THEOREM WorkIdentityEqualityProjectsSubject ==
  \A left, right:
    AsyncWorkIdentity(left) = AsyncWorkIdentity(right)
      => left.subject = right.subject
BY SMT DEF AsyncWorkIdentity

THEOREM CandidateFieldsDetermineCandidate ==
  \A left, right:
    /\ left \in AsyncCandidateSet
    /\ right \in AsyncCandidateSet
    /\ left.class = right.class
    /\ left.kind = right.kind
    /\ left.node = right.node
    /\ left.height = right.height
    /\ left.view = right.view
    /\ left.subject = right.subject
    /\ left.item = right.item
    /\ left.consumerContext = right.consumerContext
    /\ left.consumerView = right.consumerView
    /\ left.consumerGeneration = right.consumerGeneration
    /\ left.evidence = right.evidence
    /\ left.causalOrigin = right.causalOrigin
    /\ left.bodyIdentity = right.bodyIdentity
    /\ left.manifestIdentity = right.manifestIdentity
    /\ left.commitmentIdentity = right.commitmentIdentity
    => left = right
PROOF
  <1>1. ASSUME NEW left, NEW right,
                /\ left \in AsyncCandidateSet
                /\ right \in AsyncCandidateSet
                /\ left.class = right.class
                /\ left.kind = right.kind
                /\ left.node = right.node
                /\ left.height = right.height
                /\ left.view = right.view
                /\ left.subject = right.subject
                /\ left.item = right.item
                /\ left.consumerContext = right.consumerContext
                /\ left.consumerView = right.consumerView
                /\ left.consumerGeneration = right.consumerGeneration
                /\ left.evidence = right.evidence
                /\ left.causalOrigin = right.causalOrigin
                /\ left.bodyIdentity = right.bodyIdentity
                /\ left.manifestIdentity = right.manifestIdentity
                /\ left.commitmentIdentity = right.commitmentIdentity
         PROVE left = right
    <2>1. \A key \in AsyncCandidateDomain: left[key] = right[key]
      <3>1. ASSUME NEW key \in AsyncCandidateDomain
             PROVE left[key] = right[key]
        <4>1. CASE key = "class" BY <1>1, <4>1
        <4>2. CASE key = "kind" BY <1>1, <4>2
        <4>3. CASE key = "node" BY <1>1, <4>3
        <4>4. CASE key = "height" BY <1>1, <4>4
        <4>5. CASE key = "view" BY <1>1, <4>5
        <4>6. CASE key = "subject" BY <1>1, <4>6
        <4>7. CASE key = "item" BY <1>1, <4>7
        <4>8. CASE key = "consumerContext" BY <1>1, <4>8
        <4>9. CASE key = "consumerView" BY <1>1, <4>9
        <4>10. CASE key = "consumerGeneration" BY <1>1, <4>10
        <4>11. CASE key = "evidence" BY <1>1, <4>11
        <4>12. CASE key = "bodyIdentity" BY <1>1, <4>12
        <4>13. CASE key = "manifestIdentity" BY <1>1, <4>13
        <4>14. CASE key = "commitmentIdentity" BY <1>1, <4>14
        <4>15. CASE key = "causalOrigin" BY <1>1, <4>15
        <4> QED BY <3>1, <4>1, <4>2, <4>3, <4>4, <4>5, <4>6,
                     <4>7, <4>8, <4>9, <4>10, <4>11, <4>12, <4>13,
                     <4>14, <4>15 DEF AsyncCandidateDomain
      <3> QED BY <3>1
    <2>2. /\ DOMAIN left = AsyncCandidateDomain
           /\ DOMAIN right = AsyncCandidateDomain
      BY <1>1 DEF AsyncCandidateSet, AsyncCandidateDomain
    <2> QED BY <1>1, <2>1, <2>2, SetExtensionality, SMTT(30)
         DEF AsyncCandidateSet, AsyncCandidateDomain
  <1> QED BY <1>1

THEOREM ExactCandidateIdentityIffCandidateEquality ==
  \A left, right:
    /\ left \in AsyncCandidateSet
    /\ right \in AsyncCandidateSet
    => (ExactAsyncCandidateIdentity(left)
          = ExactAsyncCandidateIdentity(right)
        <=> left = right)
PROOF
  <1>1. ASSUME NEW left, NEW right,
                left \in AsyncCandidateSet,
                right \in AsyncCandidateSet
         PROVE ExactAsyncCandidateIdentity(left)
                 = ExactAsyncCandidateIdentity(right)
               <=> left = right
    <2>1. ASSUME ExactAsyncCandidateIdentity(left)
                    = ExactAsyncCandidateIdentity(right)
           PROVE left = right
      <3>1. AsyncConsumerEventTag(left) = AsyncConsumerEventTag(right)
        BY <2>1, ExactIdentityProjectsConsumerTag
      <3>2. AsyncWorkIdentity(left) = AsyncWorkIdentity(right)
        BY <2>1, ExactIdentityProjectsWorkIdentity
      <3>3. /\ left.item = right.item
             /\ left.evidence = right.evidence
             /\ left.causalOrigin = right.causalOrigin
             /\ left.bodyIdentity = right.bodyIdentity
             /\ left.manifestIdentity = right.manifestIdentity
             /\ left.commitmentIdentity = right.commitmentIdentity
        BY <2>1, ExactIdentityProjectsDirectFields
      <3>4. /\ left.consumerContext = right.consumerContext
             /\ left.consumerView = right.consumerView
             /\ left.consumerGeneration = right.consumerGeneration
        BY <3>1, ConsumerTagEqualityProjectsContext,
           ConsumerTagEqualityProjectsView,
           ConsumerTagEqualityProjectsGeneration
      <3>5. /\ left.class = right.class
             /\ left.kind = right.kind
             /\ left.node = right.node
             /\ left.height = right.height
             /\ left.view = right.view
             /\ left.subject = right.subject
        BY <3>2, WorkIdentityEqualityProjectsClass,
           WorkIdentityEqualityProjectsKind,
           WorkIdentityEqualityProjectsNode,
           WorkIdentityEqualityProjectsHeight,
           WorkIdentityEqualityProjectsView,
           WorkIdentityEqualityProjectsSubject
      <3>6. /\ DOMAIN left = AsyncCandidateDomain
             /\ DOMAIN right = AsyncCandidateDomain
        BY <1>1 DEF AsyncCandidateSet, AsyncCandidateDomain
      <3> QED BY <1>1, <3>3, <3>4, <3>5, <3>6,
                   CandidateFieldsDetermineCandidate
    <2>2. ASSUME left = right
           PROVE ExactAsyncCandidateIdentity(left)
                   = ExactAsyncCandidateIdentity(right)
      BY <2>2
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM DifferentCandidateGenerationHasDifferentIdentity ==
  \A left, right:
    left.consumerGeneration # right.consumerGeneration
      => ExactAsyncCandidateIdentity(left)
           # ExactAsyncCandidateIdentity(right)
BY SMT DEF ExactAsyncCandidateIdentity, AsyncConsumerEventTag

THEOREM DifferentCandidateEvidenceHasDifferentIdentity ==
  \A left, right:
    left.evidence # right.evidence
      => ExactAsyncCandidateIdentity(left)
           # ExactAsyncCandidateIdentity(right)
BY SMT DEF ExactAsyncCandidateIdentity

THEOREM DifferentCandidateCausalOriginHasDifferentIdentity ==
  \A left, right:
    left.causalOrigin # right.causalOrigin
      => ExactAsyncCandidateIdentity(left)
           # ExactAsyncCandidateIdentity(right)
BY SMT DEF ExactAsyncCandidateIdentity

THEOREM DifferentCandidateWorkHasDifferentIdentity ==
  \A left, right:
    AsyncWorkIdentity(left) # AsyncWorkIdentity(right)
      => ExactAsyncCandidateIdentity(left)
           # ExactAsyncCandidateIdentity(right)
BY SMT DEF ExactAsyncCandidateIdentity

THEOREM DifferentCandidateBodyHasDifferentIdentity ==
  \A left, right:
    left.bodyIdentity # right.bodyIdentity
      => ExactAsyncCandidateIdentity(left)
           # ExactAsyncCandidateIdentity(right)
BY SMT DEF ExactAsyncCandidateIdentity

THEOREM DifferentCandidateManifestHasDifferentIdentity ==
  \A left, right:
    left.manifestIdentity # right.manifestIdentity
      => ExactAsyncCandidateIdentity(left)
           # ExactAsyncCandidateIdentity(right)
BY SMT DEF ExactAsyncCandidateIdentity

THEOREM DifferentCandidateCommitmentHasDifferentIdentity ==
  \A left, right:
    left.commitmentIdentity # right.commitmentIdentity
      => ExactAsyncCandidateIdentity(left)
           # ExactAsyncCandidateIdentity(right)
BY SMT DEF ExactAsyncCandidateIdentity

THEOREM ItemDeliveryDedupIsConsumerScoped ==
  \A item:
    ItemInScheduledDelivery(item)
      => \E candidate \in QueuedCandidates \cup DeferredCandidates
                            \cup CausalCandidates
                            \cup TrackedWorkCandidates:
           /\ candidate.item = item
           /\ CandidateConsumerCurrent(candidate)
BY DEF ItemInScheduledDelivery

(***************************************************************************
Installing a TC changes the consumer generation before the old candidate value
can be dispatched.  An ordinary install also advances the view; a strict
same-round high-QC upgrade deliberately leaves the view unchanged.  In both
cases the immutable class remains whatever admission recorded, but the old
value immediately loses dedup authority; a retransmitted exact locked Commit
can therefore be reconstructed under the new consumer epoch and its new
progress classification.
***************************************************************************)
THEOREM PersistInstallStalesCurrentNodeCandidate ==
  \A command, candidate:
    /\ TypeInvariant
    /\ CandidateConsumerCurrent(candidate)
    /\ candidate.node = command.node
    /\ ExecutePersistInstall(command)
    => ~(/\ candidate.consumerContext = context'
          /\ candidate.consumerView = nodeView'[candidate.node]
          /\ candidate.consumerGeneration = generation'[candidate.node])
PROOF
  <1>1. ASSUME NEW command, NEW candidate,
                TypeInvariant,
                CandidateConsumerCurrent(candidate),
                candidate.node = command.node,
                ExecutePersistInstall(command)
         PROVE ~(/\ candidate.consumerContext = context'
                   /\ candidate.consumerView = nodeView'[candidate.node]
                   /\ candidate.consumerGeneration =
                        generation'[candidate.node])
    <2>1. PICK request \in pendingInstallTC:
             /\ command.node = request.node
             /\ command.view = request.tc.view
             /\ PersistInstallTC(request)
      BY <1>1 DEF ExecutePersistInstall
    <2>2. /\ nodeView' =
                [nodeView EXCEPT ![request.node] =
                   IF StrictSameRoundTcUpgrade(request.node, request.tc)
                   THEN @ ELSE request.tc.view + 1]
           /\ generation' =
                [generation EXCEPT ![request.node] =
                   IF StrictSameRoundTcUpgrade(request.node, request.tc)
                   THEN @ + 1 ELSE 0]
      BY <2>1 DEF PersistInstallTC
    <2>3. /\ request \in InstallTcWalSet
           /\ request.node \in ValidatorIds
           /\ DOMAIN nodeView = ValidatorIds
           /\ request.tc.view \in Views
           /\ nodeView[request.node] \in Views
           /\ Views \subseteq Nat
      BY <1>1, <2>1, SMT
         DEF TypeInvariant, InstallTcWalSet,
             TcRecordSet, ModelConfiguration, Views
    <2>4. /\ candidate.node = request.node
           /\ candidate.consumerView = nodeView[request.node]
      BY <1>1, <2>1 DEF CandidateConsumerCurrent
    <2>5. CASE StrictSameRoundTcUpgrade(request.node, request.tc)
      <3>1. /\ GenerationCanIncrement(generation[request.node])
             /\ candidate.consumerGeneration = generation[request.node]
        BY <1>1, <2>1, <2>4, <2>5
           DEF CandidateConsumerCurrent
      <3>2. generation'[request.node] = generation[request.node] + 1
        BY <2>2, <2>3, <3>1, Isa
      <3>3. candidate.consumerGeneration # generation'[candidate.node]
        BY <2>4, <3>1, <3>2, SMT
      <3> QED BY <3>3
    <2>6. CASE ~StrictSameRoundTcUpgrade(request.node, request.tc)
      <3>1. request.tc.view >= nodeView[request.node]
        BY <2>1, <2>6 DEF PersistInstallTC
      <3>2. nodeView'[request.node] = request.tc.view + 1
        BY <2>2, <2>3, <2>6, Isa
      <3>3. candidate.consumerView <= request.tc.view
        BY <2>4, <3>1
      <3>4. request.tc.view < request.tc.view + 1
        BY <2>3, SMT
      <3>5. nodeView'[candidate.node] = request.tc.view + 1
        BY <2>4, <3>2
      <3>6. candidate.consumerView # nodeView'[candidate.node]
        BY <3>3, <3>4, <3>5, SMT
      <3> QED BY <3>6
    <2> QED BY <2>5, <2>6
  <1> QED BY <1>1

THEOREM RestartCandidateIsTyped ==
  \A commandClass, kind, node, roundView, subject, evidence:
    /\ TypeInvariant
    /\ commandClass \in AsyncCommandClasses
    /\ kind \in AsyncWorkKinds
    /\ node \in ValidatorIds
    /\ roundView \in Views
    /\ subject \in SubjectOrNone
    /\ evidence \in AsyncEvidenceSet
    => AsyncCandidateTyped(
         RestartCandidate(commandClass, kind, node,
                          roundView, subject, evidence))
BY SMTT(30)
   DEF RestartCandidate, AsyncCandidateAtConsumer,
       AsyncCandidateWithIdentity, AsyncCandidateTyped,
       AsyncCandidateDomain, TypeInvariant, Generations

THEOREM StrongInvariantProjectsType ==
  StrongInductiveInvariant => TypeInvariant
BY DEF StrongInductiveInvariant, Safety

THEOREM RestartDecisionChoiceIsAvailable ==
  \A node:
    RestartDecisions(node) # {}
      => RestartDecision(node) \in RestartDecisions(node)
BY FS_EmptySet, Zenon DEF RestartDecision

THEOREM RestartLockedCommitChoiceIsAvailable ==
  \A node:
    RestartLockedCommitIntents(node) # {}
      => RestartLockedCommitIntent(node) \in RestartLockedCommitIntents(node)
BY FS_EmptySet, Zenon DEF RestartLockedCommitIntent

THEOREM RestartTimeoutChoiceIsAvailable ==
  \A node:
    RestartTimeoutIntents(node) # {}
      => RestartTimeoutIntent(node) \in RestartTimeoutIntents(node)
BY FS_EmptySet, Zenon DEF RestartTimeoutIntent

THEOREM RestartPrepareChoiceIsAvailable ==
  \A node:
    RestartPrepareIntents(node) # {}
      => RestartPrepareIntent(node) \in RestartPrepareIntents(node)
BY FS_EmptySet, Zenon DEF RestartPrepareIntent

THEOREM RestartProposalChoiceIsAvailable ==
  \A node:
    RestartProposalIntents(node) # {}
      => RestartProposalIntent(node) \in RestartProposalIntents(node)
BY FS_EmptySet, Zenon DEF RestartProposalIntent

THEOREM VoteRecordCarriesRestartEvidence ==
  \A vote \in VoteRecordSet:
    /\ vote.view \in Views
    /\ vote.subject \in SubjectOrNone
    /\ vote \in AsyncEvidenceSet
BY SMT DEF VoteRecordSet, SubjectOrNone, AsyncEvidenceSet

THEOREM TimeoutRecordCarriesRestartEvidence ==
  \A vote \in TimeoutVoteRecordSet:
    /\ vote.view \in Views
    /\ vote.highSubject \in SubjectOrNone
    /\ vote \in AsyncEvidenceSet
BY SMT DEF TimeoutVoteRecordSet, AsyncEvidenceSet

THEOREM ProposalRecordCarriesRestartEvidence ==
  \A proposal \in ProposalRecordSet:
    /\ proposal.view \in Views
    /\ proposal.subject \in SubjectOrNone
    /\ proposal \in AsyncEvidenceSet
BY SMT DEF ProposalRecordSet, SubjectOrNone, AsyncEvidenceSet

THEOREM BodyRecordCarriesRestartEvidence ==
  \A body \in BodyRecordSet:
    /\ body.view \in Views
    /\ body.subject \in SubjectOrNone
    /\ body \in AsyncEvidenceSet
BY SMT DEF BodyRecordSet, SubjectOrNone, AsyncEvidenceSet

THEOREM QcRecordCarriesRestartEvidence ==
  \A qc \in QcRecordSet:
    /\ qc.view \in Views
    /\ qc.subject \in SubjectOrNone
    /\ qc \in AsyncEvidenceSet
BY SMT DEF QcRecordSet, SubjectOrNone, AsyncEvidenceSet

THEOREM TypedOwnedSingletonIsReplay ==
  \A node, candidate:
    /\ AsyncCandidateTyped(candidate)
    /\ candidate.node = node
    => /\ AsyncQueueTyped(<<candidate>>)
       /\ AsyncCausalQueueOwnership(node, <<candidate>>)
       /\ SequenceHasUniqueValues(<<candidate>>)
PROOF
  <1>1. ASSUME NEW node, NEW candidate,
                AsyncCandidateTyped(candidate),
                candidate.node = node
         PROVE /\ AsyncQueueTyped(<<candidate>>)
               /\ AsyncCausalQueueOwnership(node, <<candidate>>)
               /\ SequenceHasUniqueValues(<<candidate>>)
    <2>1. [index \in 1..1 |-> candidate] \in Seq({candidate})
      BY IsASeq, SMT
    <2>2. <<candidate>> = [index \in 1..1 |-> candidate]
      BY Isa
    <2>3. /\ <<candidate>> \in Seq({candidate})
           /\ Len(<<candidate>>) = 1
           /\ DOMAIN <<candidate>> = 1..1
      BY <2>1, <2>2, Isa
    <2>4. Range(<<candidate>>) =
             {<<candidate>>[index]: index \in 1..Len(<<candidate>>)}
      BY <2>3, RangeEquality
    <2>5. Range(<<candidate>>) = {candidate}
      BY <2>3, <2>4, Isa
    <2>6. SequenceSet(<<candidate>>) = {candidate}
      BY <2>3, Isa DEF SequenceSet
    <2>7. AsyncQueueTyped(<<candidate>>)
      BY <1>1, <2>3, <2>5, Isa DEF AsyncQueueTyped
    <2>8. AsyncCausalQueueOwnership(node, <<candidate>>)
      BY <1>1, <2>6 DEF AsyncCausalQueueOwnership
    <2>9. SequenceHasUniqueValues(<<candidate>>)
      BY <2>3, <2>6, FS_Singleton, SMT DEF SequenceHasUniqueValues
    <2> QED BY <2>7, <2>8, <2>9
  <1> QED BY <1>1

THEOREM TypedRestartEvidenceProducesSingletonReplay ==
  \A node, kind, roundView, subject, evidence:
    /\ TypeInvariant
    /\ node \in ValidatorIds
    /\ kind \in AsyncWorkKinds
    /\ roundView \in Views
    /\ subject \in SubjectOrNone
    /\ evidence \in AsyncEvidenceSet
    => /\ AsyncQueueTyped(
              <<RestartCandidate("Completion", kind, node,
                                  roundView, subject, evidence)>>)
       /\ AsyncCausalQueueOwnership(
              node,
              <<RestartCandidate("Completion", kind, node,
                                  roundView, subject, evidence)>>)
       /\ SequenceHasUniqueValues(
              <<RestartCandidate("Completion", kind, node,
                                  roundView, subject, evidence)>>)
PROOF
  <1>1. ASSUME NEW node, NEW kind, NEW roundView,
                NEW subject, NEW evidence,
                TypeInvariant,
                node \in ValidatorIds,
                kind \in AsyncWorkKinds,
                roundView \in Views,
                subject \in SubjectOrNone,
                evidence \in AsyncEvidenceSet
         PROVE /\ AsyncQueueTyped(
                       <<RestartCandidate("Completion", kind, node,
                                           roundView, subject, evidence)>>)
               /\ AsyncCausalQueueOwnership(
                       node,
                       <<RestartCandidate("Completion", kind, node,
                                           roundView, subject, evidence)>>)
               /\ SequenceHasUniqueValues(
                       <<RestartCandidate("Completion", kind, node,
                                           roundView, subject, evidence)>>)
    <2> DEFINE Candidate ==
           RestartCandidate("Completion", kind, node,
                            roundView, subject, evidence)
    <2>1. AsyncCandidateTyped(Candidate)
      BY <1>1, RestartCandidateIsTyped
         DEF Candidate, AsyncCommandClasses
    <2>2. Candidate.node = node
      BY DEF Candidate, RestartCandidate, AsyncCandidateAtConsumer,
             AsyncCandidateWithIdentity
    <2> QED BY <2>1, <2>2, TypedOwnedSingletonIsReplay DEF Candidate
  <1> QED BY <1>1


THEOREM RestartDecisionReplayProperties ==
  \A node:
    /\ StrongInductiveInvariant
    /\ node \in ValidatorIds
    /\ RestartDecisions(node) # {}
    => /\ AsyncQueueTyped(RestartDecisionReplay(node))
       /\ AsyncCausalQueueOwnership(node, RestartDecisionReplay(node))
       /\ SequenceHasUniqueValues(RestartDecisionReplay(node))
PROOF
  <1>1. ASSUME NEW node,
                StrongInductiveInvariant,
                node \in ValidatorIds,
                RestartDecisions(node) # {}
         PROVE /\ AsyncQueueTyped(RestartDecisionReplay(node))
               /\ AsyncCausalQueueOwnership(
                    node, RestartDecisionReplay(node))
               /\ SequenceHasUniqueValues(RestartDecisionReplay(node))
    <2>1. TypeInvariant
      BY <1>1, StrongInvariantProjectsType
    <2>2. RestartDecision(node) \in RestartDecisions(node)
      BY <1>1, RestartDecisionChoiceIsAvailable
    <2>3. RestartDecision(node).qc \in QcRecordSet
      BY <1>1, <2>1, <2>2, SMTT(20)
         DEF RestartDecisions, StrongInductiveInvariant, Safety,
             DecisionAgreement, TypeInvariant
    <2>4. /\ RestartDecision(node).qc.view \in Views
           /\ RestartDecision(node).qc.subject \in SubjectOrNone
           /\ RestartDecision(node).qc \in AsyncEvidenceSet
      BY <2>3, QcRecordCarriesRestartEvidence
    <2>5. /\ AsyncQueueTyped(RestartDecisionReplay(node))
           /\ AsyncCausalQueueOwnership(
                node, RestartDecisionReplay(node))
           /\ SequenceHasUniqueValues(RestartDecisionReplay(node))
      BY <1>1, <2>1, <2>4,
         TypedRestartEvidenceProducesSingletonReplay
         DEF RestartDecisionReplay, AsyncWorkKinds, AsyncReducerKinds
    <2> QED BY <2>5
  <1> QED BY <1>1

THEOREM RestartDecisionOwnsOneFetchFrontier ==
  \A node:
    RestartDecisions(node) # {} =>
      /\ Len(RestartDecisionReplay(node)) = 1
      /\ RestartDecisionReplay(node)[1].kind = "FetchBody"
BY DEF RestartDecisionReplay

THEOREM RestartLockedCommitReplayProperties ==
  \A node:
    /\ TypeInvariant
    /\ node \in ValidatorIds
    /\ RestartLockedCommitIntents(node) # {}
    => /\ AsyncQueueTyped(RestartLockedCommitReplay(node))
       /\ AsyncCausalQueueOwnership(node, RestartLockedCommitReplay(node))
       /\ SequenceHasUniqueValues(RestartLockedCommitReplay(node))
PROOF
  <1>1. ASSUME NEW node, TypeInvariant, node \in ValidatorIds,
                RestartLockedCommitIntents(node) # {}
         PROVE /\ AsyncQueueTyped(RestartLockedCommitReplay(node))
               /\ AsyncCausalQueueOwnership(
                    node, RestartLockedCommitReplay(node))
               /\ SequenceHasUniqueValues(RestartLockedCommitReplay(node))
    <2>1. RestartLockedCommitIntent(node)
             \in RestartLockedCommitIntents(node)
      BY <1>1, RestartLockedCommitChoiceIsAvailable
    <2>2. /\ RestartLockedCommitIntent(node).view \in Views
           /\ RestartLockedCommitIntent(node).subject \in SubjectOrNone
           /\ RestartLockedCommitIntent(node) \in AsyncEvidenceSet
      BY <1>1, <2>1, SMT
         DEF RestartLockedCommitIntents, TypeInvariant,
             VoteRecordSet, AsyncEvidenceSet
    <2>3. /\ AsyncQueueTyped(
                    <<RestartCandidate(
                        "Completion", "SignVote", node,
                        RestartLockedCommitIntent(node).view,
                        RestartLockedCommitIntent(node).subject,
                        RestartLockedCommitIntent(node))>>)
           /\ AsyncCausalQueueOwnership(
                    node,
                    <<RestartCandidate(
                        "Completion", "SignVote", node,
                        RestartLockedCommitIntent(node).view,
                        RestartLockedCommitIntent(node).subject,
                        RestartLockedCommitIntent(node))>>)
           /\ SequenceHasUniqueValues(
                    <<RestartCandidate(
                        "Completion", "SignVote", node,
                        RestartLockedCommitIntent(node).view,
                        RestartLockedCommitIntent(node).subject,
                        RestartLockedCommitIntent(node))>>)
      BY <1>1, <2>2, TypedRestartEvidenceProducesSingletonReplay
         DEF AsyncWorkKinds, AsyncReducerKinds
    <2>4. RestartLockedCommitReplay(node) =
             <<RestartCandidate(
                 "Completion", "SignVote", node,
                 RestartLockedCommitIntent(node).view,
                 RestartLockedCommitIntent(node).subject,
                 RestartLockedCommitIntent(node))>>
      BY DEF RestartLockedCommitReplay
    <2> QED BY <2>3, <2>4
  <1> QED BY <1>1

THEOREM RestartTimeoutReplayProperties ==
  \A node:
    /\ TypeInvariant
    /\ node \in ValidatorIds
    /\ RestartTimeoutIntents(node) # {}
    => /\ AsyncQueueTyped(RestartTimeoutReplay(node))
       /\ AsyncCausalQueueOwnership(node, RestartTimeoutReplay(node))
       /\ SequenceHasUniqueValues(RestartTimeoutReplay(node))
PROOF
  <1>1. ASSUME NEW node, TypeInvariant, node \in ValidatorIds,
                RestartTimeoutIntents(node) # {}
         PROVE /\ AsyncQueueTyped(RestartTimeoutReplay(node))
               /\ AsyncCausalQueueOwnership(node, RestartTimeoutReplay(node))
               /\ SequenceHasUniqueValues(RestartTimeoutReplay(node))
    <2>1. RestartTimeoutIntent(node) \in RestartTimeoutIntents(node)
      BY <1>1, RestartTimeoutChoiceIsAvailable
    <2>2. /\ RestartTimeoutIntent(node).view \in Views
           /\ RestartTimeoutIntent(node).highSubject \in SubjectOrNone
           /\ RestartTimeoutIntent(node) \in AsyncEvidenceSet
      BY <1>1, <2>1, SMT
         DEF RestartTimeoutIntents, TypeInvariant,
             TimeoutVoteRecordSet, AsyncEvidenceSet
    <2>3. /\ AsyncQueueTyped(
                    <<RestartCandidate(
                        "Completion", "SignTimeout", node,
                        RestartTimeoutIntent(node).view,
                        RestartTimeoutIntent(node).highSubject,
                        RestartTimeoutIntent(node))>>)
           /\ AsyncCausalQueueOwnership(
                    node,
                    <<RestartCandidate(
                        "Completion", "SignTimeout", node,
                        RestartTimeoutIntent(node).view,
                        RestartTimeoutIntent(node).highSubject,
                        RestartTimeoutIntent(node))>>)
           /\ SequenceHasUniqueValues(
                    <<RestartCandidate(
                        "Completion", "SignTimeout", node,
                        RestartTimeoutIntent(node).view,
                        RestartTimeoutIntent(node).highSubject,
                        RestartTimeoutIntent(node))>>)
      BY <1>1, <2>2, TypedRestartEvidenceProducesSingletonReplay
         DEF AsyncWorkKinds, AsyncReducerKinds
    <2>4. RestartTimeoutReplay(node) =
             <<RestartCandidate(
                 "Completion", "SignTimeout", node,
                 RestartTimeoutIntent(node).view,
                 RestartTimeoutIntent(node).highSubject,
                 RestartTimeoutIntent(node))>>
      BY DEF RestartTimeoutReplay
    <2> QED BY <2>3, <2>4
  <1> QED BY <1>1

THEOREM RestartPrepareReplayProperties ==
  \A node:
    /\ TypeInvariant
    /\ node \in ValidatorIds
    /\ RestartPrepareIntents(node) # {}
    => /\ AsyncQueueTyped(RestartPrepareReplay(node))
       /\ AsyncCausalQueueOwnership(node, RestartPrepareReplay(node))
       /\ SequenceHasUniqueValues(RestartPrepareReplay(node))
PROOF
  <1>1. ASSUME NEW node, TypeInvariant, node \in ValidatorIds,
                RestartPrepareIntents(node) # {}
         PROVE /\ AsyncQueueTyped(RestartPrepareReplay(node))
               /\ AsyncCausalQueueOwnership(node, RestartPrepareReplay(node))
               /\ SequenceHasUniqueValues(RestartPrepareReplay(node))
    <2>1. RestartPrepareIntent(node) \in RestartPrepareIntents(node)
      BY <1>1, RestartPrepareChoiceIsAvailable
    <2>2a. RestartPrepareIntent(node) \in prepareIntents
      BY <2>1 DEF RestartPrepareIntents
    <2>2b. prepareIntents \subseteq VoteRecordSet
      BY <1>1 DEF TypeInvariant
    <2>2c. RestartPrepareIntent(node) \in VoteRecordSet
      BY <2>2a, <2>2b
    <2>2. /\ RestartPrepareIntent(node).view \in Views
           /\ RestartPrepareIntent(node).subject \in SubjectOrNone
           /\ RestartPrepareIntent(node) \in AsyncEvidenceSet
      BY <2>2c, VoteRecordCarriesRestartEvidence
    <2>3. /\ AsyncQueueTyped(
                    <<RestartCandidate(
                        "Completion", "SignVote", node,
                        RestartPrepareIntent(node).view,
                        RestartPrepareIntent(node).subject,
                        RestartPrepareIntent(node))>>)
           /\ AsyncCausalQueueOwnership(
                    node,
                    <<RestartCandidate(
                        "Completion", "SignVote", node,
                        RestartPrepareIntent(node).view,
                        RestartPrepareIntent(node).subject,
                        RestartPrepareIntent(node))>>)
           /\ SequenceHasUniqueValues(
                    <<RestartCandidate(
                        "Completion", "SignVote", node,
                        RestartPrepareIntent(node).view,
                        RestartPrepareIntent(node).subject,
                        RestartPrepareIntent(node))>>)
      BY <1>1, <2>2, TypedRestartEvidenceProducesSingletonReplay
         DEF AsyncWorkKinds, AsyncReducerKinds
    <2>4. RestartPrepareReplay(node) =
             <<RestartCandidate(
                 "Completion", "SignVote", node,
                 RestartPrepareIntent(node).view,
                 RestartPrepareIntent(node).subject,
                 RestartPrepareIntent(node))>>
      BY DEF RestartPrepareReplay
    <2> QED BY <2>3, <2>4
  <1> QED BY <1>1

THEOREM RestartProposalReplayProperties ==
  \A node:
    /\ TypeInvariant
    /\ node \in ValidatorIds
    /\ RestartProposalIntents(node) # {}
    => /\ AsyncQueueTyped(RestartProposalReplay(node))
       /\ AsyncCausalQueueOwnership(node, RestartProposalReplay(node))
       /\ SequenceHasUniqueValues(RestartProposalReplay(node))
PROOF
  <1>1. ASSUME NEW node, TypeInvariant, node \in ValidatorIds,
                RestartProposalIntents(node) # {}
         PROVE /\ AsyncQueueTyped(RestartProposalReplay(node))
               /\ AsyncCausalQueueOwnership(node, RestartProposalReplay(node))
               /\ SequenceHasUniqueValues(RestartProposalReplay(node))
    <2>1. RestartProposalIntent(node) \in RestartProposalIntents(node)
      BY <1>1, RestartProposalChoiceIsAvailable
    <2>2a. RestartProposalIntent(node) \in proposalIntents
      BY <2>1 DEF RestartProposalIntents
    <2>2b. proposalIntents \subseteq ProposalRecordSet
      BY <1>1 DEF TypeInvariant
    <2>2c. RestartProposalIntent(node) \in ProposalRecordSet
      BY <2>2a, <2>2b
    <2>2. /\ RestartProposalIntent(node).view \in Views
           /\ RestartProposalIntent(node).subject \in SubjectOrNone
           /\ RestartProposalIntent(node) \in AsyncEvidenceSet
      BY <2>2c, ProposalRecordCarriesRestartEvidence
    <2>3. /\ AsyncQueueTyped(
                    <<RestartCandidate(
                        "Completion", "SignProposal", node,
                        RestartProposalIntent(node).view,
                        RestartProposalIntent(node).subject,
                        RestartProposalIntent(node))>>)
           /\ AsyncCausalQueueOwnership(
                    node,
                    <<RestartCandidate(
                        "Completion", "SignProposal", node,
                        RestartProposalIntent(node).view,
                        RestartProposalIntent(node).subject,
                        RestartProposalIntent(node))>>)
           /\ SequenceHasUniqueValues(
                    <<RestartCandidate(
                        "Completion", "SignProposal", node,
                        RestartProposalIntent(node).view,
                        RestartProposalIntent(node).subject,
                        RestartProposalIntent(node))>>)
      BY <1>1, <2>2, TypedRestartEvidenceProducesSingletonReplay
         DEF AsyncWorkKinds, AsyncReducerKinds
    <2>4. RestartProposalReplay(node) =
             <<RestartCandidate(
                 "Completion", "SignProposal", node,
                 RestartProposalIntent(node).view,
                 RestartProposalIntent(node).subject,
                 RestartProposalIntent(node))>>
      BY DEF RestartProposalReplay
    <2> QED BY <2>3, <2>4
  <1> QED BY <1>1


THEOREM RestartSignatureReplayExactOrder ==
  \A node:
    RestartSignatureReplay(node) =
      IF NodeHasApplication(node) \/ RestartDecisions(node) # {}
      THEN <<>>
      ELSE RestartTimeoutOrProposalReplay(node)
             \o RestartPrepareReplayIfActive(node)
             \o RestartLockedCommitReplayIfActive(node)
BY DEF RestartSignatureReplay

THEOREM EmptyReplayProperties ==
  \A node:
    /\ AsyncQueueTyped(<<>>)
    /\ AsyncCausalQueueOwnership(node, <<>>)
    /\ SequenceHasUniqueValues(<<>>)
    /\ Len(<<>>) = 0
BY EmptySeq, RangeEquality, FS_EmptySet, Isa
   DEF AsyncQueueTyped, AsyncCausalQueueOwnership,
       SequenceHasUniqueValues, SequenceSet

THEOREM RestartLockedPrepareChoiceIsAvailable ==
  \A node:
    RestartLockedPrepareQCs(node) # {}
      => RestartLockedPrepareQC(node)
           \in RestartLockedPrepareQCs(node)
BY FS_EmptySet, Zenon DEF RestartLockedPrepareQC

THEOREM RestartLockedBodyReplayProperties ==
  \A node \in ValidatorIds:
    TypeInvariant =>
      /\ AsyncQueueTyped(RestartLockedBodyReplay(node))
      /\ AsyncCausalQueueOwnership(node, RestartLockedBodyReplay(node))
      /\ SequenceHasUniqueValues(RestartLockedBodyReplay(node))
      /\ Len(RestartLockedBodyReplay(node)) <= 1
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds, TypeInvariant
         PROVE /\ AsyncQueueTyped(RestartLockedBodyReplay(node))
               /\ AsyncCausalQueueOwnership(
                    node, RestartLockedBodyReplay(node))
               /\ SequenceHasUniqueValues(
                    RestartLockedBodyReplay(node))
               /\ Len(RestartLockedBodyReplay(node)) <= 1
    <2>1. CASE RestartLockedPrepareQCs(node) = {}
      <3>1. RestartLockedBodyReplay(node) = <<>>
        BY <2>1 DEF RestartLockedBodyReplay
      <3> QED BY <3>1, EmptyReplayProperties
    <2>2. CASE RestartLockedPrepareQCs(node) # {}
      <3> DEFINE Qc == RestartLockedPrepareQC(node)
      <3>1. Qc \in RestartLockedPrepareQCs(node)
        BY <2>2, RestartLockedPrepareChoiceIsAvailable DEF Qc
      <3>2. Qc \in QcRecordSet
        BY <1>1, <3>1, SMT
           DEF RestartLockedPrepareQCs, TypeInvariant
      <3>3. /\ Qc.view \in Views
             /\ Qc.subject \in SubjectOrNone
             /\ Qc \in AsyncEvidenceSet
        BY <3>2, QcRecordCarriesRestartEvidence
      <3>4. /\ AsyncQueueTyped(
                    <<RestartCandidate(
                        "Completion", "FetchBody", node,
                        Qc.view, Qc.subject, Qc)>>)
             /\ AsyncCausalQueueOwnership(
                    node,
                    <<RestartCandidate(
                        "Completion", "FetchBody", node,
                        Qc.view, Qc.subject, Qc)>>)
             /\ SequenceHasUniqueValues(
                    <<RestartCandidate(
                        "Completion", "FetchBody", node,
                        Qc.view, Qc.subject, Qc)>>)
        BY <1>1, <3>3, TypedRestartEvidenceProducesSingletonReplay
           DEF AsyncWorkKinds, AsyncReducerKinds
      <3>5. /\ RestartLockedBodyReplay(node) =
                    <<RestartCandidate(
                        "Completion", "FetchBody", node,
                        Qc.view, Qc.subject, Qc)>>
             /\ Len(RestartLockedBodyReplay(node)) = 1
        BY <2>2 DEF RestartLockedBodyReplay, Qc
      <3> QED BY <3>4, <3>5
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM ReplayUniqueSequenceIsInjective ==
  \A sequence:
    /\ sequence \in Seq(Range(sequence))
    /\ SequenceHasUniqueValues(sequence)
    => IsInjective(sequence)
PROOF
  <1>1. ASSUME NEW sequence,
                sequence \in Seq(Range(sequence)),
                SequenceHasUniqueValues(sequence)
         PROVE IsInjective(sequence)
    <2>1. /\ Len(sequence) \in Nat
           /\ sequence \in [1..Len(sequence) -> Range(sequence)]
      BY <1>1, LenProperties
    <2>2. /\ IsFiniteSet(1..Len(sequence))
           /\ Cardinality(1..Len(sequence)) = Len(sequence)
      BY <2>1, FS_Interval, SMT
    <2>3. SequenceSet(sequence) = Range(sequence)
      BY <1>1, RangeEquality DEF SequenceSet
    <2>4. sequence \in Surjection(1..Len(sequence), Range(sequence))
      BY <2>1, Fun_RangeProperties
    <2>5. Cardinality(1..Len(sequence)) =
             Cardinality(Range(sequence))
      BY <1>1, <2>2, <2>3 DEF SequenceHasUniqueValues
    <2>6. sequence \in Injection(1..Len(sequence), Range(sequence))
      BY <2>2, <2>4, <2>5, FS_SurjSameCardinalityImpliesInj
    <2> QED BY <2>6 DEF Injection
  <1> QED BY <1>1

THEOREM ReplayInjectiveSequenceIsUnique ==
  \A sequence:
    /\ sequence \in Seq(Range(sequence))
    /\ IsInjective(sequence)
    => SequenceHasUniqueValues(sequence)
PROOF
  <1>1. ASSUME NEW sequence,
                sequence \in Seq(Range(sequence)),
                IsInjective(sequence)
         PROVE SequenceHasUniqueValues(sequence)
    <2>1. /\ Len(sequence) \in Nat
           /\ sequence \in [1..Len(sequence) -> Range(sequence)]
      BY <1>1, LenProperties
    <2>2. /\ IsFiniteSet(1..Len(sequence))
           /\ Cardinality(1..Len(sequence)) = Len(sequence)
      BY <2>1, FS_Interval, SMT
    <2>3. sequence \in Surjection(1..Len(sequence), Range(sequence))
      BY <2>1, Fun_RangeProperties
    <2>4. sequence \in Injection(1..Len(sequence), Range(sequence))
      BY <1>1, <2>1 DEF Injection
    <2>5. Cardinality(Range(sequence)) =
             Cardinality(1..Len(sequence))
      BY <2>2, <2>3, <2>4, FS_Surjection
    <2>6. SequenceSet(sequence) = Range(sequence)
      BY <1>1, RangeEquality DEF SequenceSet
    <2> QED BY <2>2, <2>5, <2>6 DEF SequenceHasUniqueValues
  <1> QED BY <1>1

THEOREM ConcatTypedOwnedDisjointReplay ==
  \A node, left, right:
    /\ AsyncQueueTyped(left)
    /\ AsyncCausalQueueOwnership(node, left)
    /\ SequenceHasUniqueValues(left)
    /\ AsyncQueueTyped(right)
    /\ AsyncCausalQueueOwnership(node, right)
    /\ SequenceHasUniqueValues(right)
    /\ SequenceSet(left) \cap SequenceSet(right) = {}
    => /\ AsyncQueueTyped(left \o right)
       /\ AsyncCausalQueueOwnership(node, left \o right)
       /\ SequenceHasUniqueValues(left \o right)
       /\ Len(left \o right) = Len(left) + Len(right)
PROOF
  <1>1. ASSUME NEW node, NEW left, NEW right,
                AsyncQueueTyped(left),
                AsyncCausalQueueOwnership(node, left),
                SequenceHasUniqueValues(left),
                AsyncQueueTyped(right),
                AsyncCausalQueueOwnership(node, right),
                SequenceHasUniqueValues(right),
                SequenceSet(left) \cap SequenceSet(right) = {}
         PROVE /\ AsyncQueueTyped(left \o right)
               /\ AsyncCausalQueueOwnership(node, left \o right)
               /\ SequenceHasUniqueValues(left \o right)
               /\ Len(left \o right) = Len(left) + Len(right)
    <2>1. /\ left \in Seq(Range(left))
           /\ right \in Seq(Range(right))
           /\ SequenceSet(left) = Range(left)
           /\ SequenceSet(right) = Range(right)
      BY <1>1, RangeEquality DEF AsyncQueueTyped, SequenceSet
    <2>2. /\ left \o right
                  \in Seq(Range(left) \cup Range(right))
           /\ Len(left \o right) = Len(left) + Len(right)
           /\ Range(left \o right) =
                Range(left) \cup Range(right)
      BY <2>1, ConcatProperties, RangeConcatenation
    <2>3. AsyncQueueTyped(left \o right)
      BY <1>1, <2>1, <2>2, Isa DEF AsyncQueueTyped
    <2>4. AsyncCausalQueueOwnership(node, left \o right)
      BY <1>1, <2>1, <2>2, Isa
         DEF AsyncCausalQueueOwnership, SequenceSet
    <2>5. /\ IsInjective(left)
           /\ IsInjective(right)
           /\ Range(left) \cap Range(right) = {}
      BY <1>1, <2>1, ReplayUniqueSequenceIsInjective
    <2>6. IsInjective(left \o right)
      BY <2>1, <2>5, ConcatInjectiveSeq
    <2>7. SequenceHasUniqueValues(left \o right)
      BY <2>2, <2>6, ReplayInjectiveSequenceIsUnique
    <2> QED BY <2>2, <2>3, <2>4, <2>7
  <1> QED BY <1>1

THEOREM RestartTimeoutOrProposalReplayProperties ==
  \A node \in ValidatorIds:
    TypeInvariant =>
      /\ AsyncQueueTyped(RestartTimeoutOrProposalReplay(node))
      /\ AsyncCausalQueueOwnership(
           node, RestartTimeoutOrProposalReplay(node))
      /\ SequenceHasUniqueValues(RestartTimeoutOrProposalReplay(node))
      /\ Len(RestartTimeoutOrProposalReplay(node)) <= 1
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds, TypeInvariant
         PROVE /\ AsyncQueueTyped(RestartTimeoutOrProposalReplay(node))
               /\ AsyncCausalQueueOwnership(
                    node, RestartTimeoutOrProposalReplay(node))
               /\ SequenceHasUniqueValues(
                    RestartTimeoutOrProposalReplay(node))
               /\ Len(RestartTimeoutOrProposalReplay(node)) <= 1
    <2>1. CASE RestartTimeoutIntents(node) # {}
      <3>1. /\ AsyncQueueTyped(RestartTimeoutReplay(node))
             /\ AsyncCausalQueueOwnership(
                  node, RestartTimeoutReplay(node))
             /\ SequenceHasUniqueValues(RestartTimeoutReplay(node))
        BY <1>1, <2>1, RestartTimeoutReplayProperties
      <3>2. /\ RestartTimeoutOrProposalReplay(node) =
                    RestartTimeoutReplay(node)
             /\ Len(RestartTimeoutReplay(node)) = 1
        BY <2>1 DEF RestartTimeoutOrProposalReplay,
                      RestartTimeoutReplay
      <3> QED BY <3>1, <3>2
    <2>2. CASE /\ RestartTimeoutIntents(node) = {}
                 /\ RestartProposalIntents(node) # {}
      <3>1. /\ AsyncQueueTyped(RestartProposalReplay(node))
             /\ AsyncCausalQueueOwnership(
                  node, RestartProposalReplay(node))
             /\ SequenceHasUniqueValues(RestartProposalReplay(node))
        BY <1>1, <2>2, RestartProposalReplayProperties
      <3>2. /\ RestartTimeoutOrProposalReplay(node) =
                    RestartProposalReplay(node)
             /\ Len(RestartProposalReplay(node)) = 1
        BY <2>2 DEF RestartTimeoutOrProposalReplay,
                      RestartProposalReplay
      <3> QED BY <3>1, <3>2
    <2>3. CASE /\ RestartTimeoutIntents(node) = {}
                 /\ RestartProposalIntents(node) = {}
      <3>1. RestartTimeoutOrProposalReplay(node) = <<>>
        BY <2>3 DEF RestartTimeoutOrProposalReplay
      <3> QED BY <3>1, EmptyReplayProperties
    <2> QED BY <2>1, <2>2, <2>3, SMT
  <1> QED BY <1>1

THEOREM RestartPrepareReplayIfActiveProperties ==
  \A node \in ValidatorIds:
    TypeInvariant =>
      /\ AsyncQueueTyped(RestartPrepareReplayIfActive(node))
      /\ AsyncCausalQueueOwnership(
           node, RestartPrepareReplayIfActive(node))
      /\ SequenceHasUniqueValues(RestartPrepareReplayIfActive(node))
      /\ Len(RestartPrepareReplayIfActive(node)) <= 1
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds, TypeInvariant
         PROVE /\ AsyncQueueTyped(RestartPrepareReplayIfActive(node))
               /\ AsyncCausalQueueOwnership(
                    node, RestartPrepareReplayIfActive(node))
               /\ SequenceHasUniqueValues(
                    RestartPrepareReplayIfActive(node))
               /\ Len(RestartPrepareReplayIfActive(node)) <= 1
    <2>1. CASE RestartPrepareIntents(node) # {}
      <3>1. /\ AsyncQueueTyped(RestartPrepareReplay(node))
             /\ AsyncCausalQueueOwnership(node, RestartPrepareReplay(node))
             /\ SequenceHasUniqueValues(RestartPrepareReplay(node))
        BY <1>1, <2>1, RestartPrepareReplayProperties
      <3>2. /\ RestartPrepareReplayIfActive(node) =
                    RestartPrepareReplay(node)
             /\ Len(RestartPrepareReplay(node)) = 1
        BY <2>1 DEF RestartPrepareReplayIfActive, RestartPrepareReplay
      <3> QED BY <3>1, <3>2
    <2>2. CASE RestartPrepareIntents(node) = {}
      <3>1. RestartPrepareReplayIfActive(node) = <<>>
        BY <2>2 DEF RestartPrepareReplayIfActive
      <3> QED BY <3>1, EmptyReplayProperties
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM RestartLockedCommitReplayIfActiveProperties ==
  \A node \in ValidatorIds:
    TypeInvariant =>
      /\ AsyncQueueTyped(RestartLockedCommitReplayIfActive(node))
      /\ AsyncCausalQueueOwnership(
           node, RestartLockedCommitReplayIfActive(node))
      /\ SequenceHasUniqueValues(
           RestartLockedCommitReplayIfActive(node))
      /\ Len(RestartLockedCommitReplayIfActive(node)) <= 1
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds, TypeInvariant
         PROVE /\ AsyncQueueTyped(RestartLockedCommitReplayIfActive(node))
               /\ AsyncCausalQueueOwnership(
                    node, RestartLockedCommitReplayIfActive(node))
               /\ SequenceHasUniqueValues(
                    RestartLockedCommitReplayIfActive(node))
               /\ Len(RestartLockedCommitReplayIfActive(node)) <= 1
    <2>1. CASE RestartLockedCommitIntents(node) # {}
      <3>1. /\ AsyncQueueTyped(RestartLockedCommitReplay(node))
             /\ AsyncCausalQueueOwnership(
                  node, RestartLockedCommitReplay(node))
             /\ SequenceHasUniqueValues(RestartLockedCommitReplay(node))
        BY <1>1, <2>1, RestartLockedCommitReplayProperties
      <3>2. /\ RestartLockedCommitReplayIfActive(node) =
                    RestartLockedCommitReplay(node)
             /\ Len(RestartLockedCommitReplay(node)) = 1
        BY <2>1 DEF RestartLockedCommitReplayIfActive,
                      RestartLockedCommitReplay
      <3> QED BY <3>1, <3>2
    <2>2. CASE RestartLockedCommitIntents(node) = {}
      <3>1. RestartLockedCommitReplayIfActive(node) = <<>>
        BY <2>2 DEF RestartLockedCommitReplayIfActive
      <3> QED BY <3>1, EmptyReplayProperties
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM RestartSignatureReplayComponentsAreDisjoint ==
  \A node:
    /\ SequenceSet(RestartTimeoutOrProposalReplay(node)) \cap
         SequenceSet(RestartPrepareReplayIfActive(node)) = {}
    /\ SequenceSet(RestartTimeoutOrProposalReplay(node)) \cap
         SequenceSet(RestartLockedCommitReplayIfActive(node)) = {}
    /\ SequenceSet(RestartPrepareReplayIfActive(node)) \cap
         SequenceSet(RestartLockedCommitReplayIfActive(node)) = {}
BY Isa
   DEF RestartTimeoutOrProposalReplay,
       RestartPrepareReplayIfActive, RestartLockedCommitReplayIfActive,
       RestartTimeoutReplay, RestartProposalReplay,
       RestartPrepareReplay, RestartLockedCommitReplay,
       RestartTimeoutIntents, RestartProposalIntents,
       RestartPrepareIntents, RestartLockedCommitIntents,
       RestartCandidate, AsyncCandidateAtConsumer,
       AsyncCandidateWithIdentity, SequenceSet

THEOREM RestartSignatureReplayProperties ==
  \A node \in ValidatorIds:
    TypeInvariant =>
      /\ AsyncQueueTyped(RestartSignatureReplay(node))
      /\ AsyncCausalQueueOwnership(node, RestartSignatureReplay(node))
      /\ SequenceHasUniqueValues(RestartSignatureReplay(node))
      /\ Len(RestartSignatureReplay(node)) <= 3
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds, TypeInvariant
         PROVE /\ AsyncQueueTyped(RestartSignatureReplay(node))
               /\ AsyncCausalQueueOwnership(
                    node, RestartSignatureReplay(node))
               /\ SequenceHasUniqueValues(RestartSignatureReplay(node))
               /\ Len(RestartSignatureReplay(node)) <= 3
    <2>1. CASE NodeHasApplication(node) \/ RestartDecisions(node) # {}
      <3>1. RestartSignatureReplay(node) = <<>>
        BY <2>1 DEF RestartSignatureReplay
      <3> QED BY <3>1, EmptyReplayProperties
    <2>2. CASE /\ ~NodeHasApplication(node)
                 /\ RestartDecisions(node) = {}
      <3> DEFINE First == RestartTimeoutOrProposalReplay(node)
      <3> DEFINE Second == RestartPrepareReplayIfActive(node)
      <3> DEFINE Third == RestartLockedCommitReplayIfActive(node)
      <3>1. /\ AsyncQueueTyped(First)
             /\ AsyncCausalQueueOwnership(node, First)
             /\ SequenceHasUniqueValues(First)
             /\ Len(First) <= 1
        BY <1>1, RestartTimeoutOrProposalReplayProperties DEF First
      <3>2. /\ AsyncQueueTyped(Second)
             /\ AsyncCausalQueueOwnership(node, Second)
             /\ SequenceHasUniqueValues(Second)
             /\ Len(Second) <= 1
        BY <1>1, RestartPrepareReplayIfActiveProperties DEF Second
      <3>3. /\ AsyncQueueTyped(Third)
             /\ AsyncCausalQueueOwnership(node, Third)
             /\ SequenceHasUniqueValues(Third)
             /\ Len(Third) <= 1
        BY <1>1, RestartLockedCommitReplayIfActiveProperties DEF Third
      <3>4. /\ SequenceSet(First) \cap SequenceSet(Second) = {}
             /\ SequenceSet(First) \cap SequenceSet(Third) = {}
             /\ SequenceSet(Second) \cap SequenceSet(Third) = {}
        BY RestartSignatureReplayComponentsAreDisjoint
           DEF First, Second, Third
      <3>5. /\ AsyncQueueTyped(First \o Second)
             /\ AsyncCausalQueueOwnership(node, First \o Second)
             /\ SequenceHasUniqueValues(First \o Second)
             /\ Len(First \o Second) = Len(First) + Len(Second)
        BY <3>1, <3>2, <3>4, ConcatTypedOwnedDisjointReplay
      <3>6. SequenceSet(First \o Second) \cap SequenceSet(Third) = {}
        BY <3>1, <3>2, <3>4, RangeConcatenation, RangeEquality, Isa
           DEF AsyncQueueTyped, SequenceSet
      <3>7. /\ AsyncQueueTyped((First \o Second) \o Third)
             /\ AsyncCausalQueueOwnership(node, (First \o Second) \o Third)
             /\ SequenceHasUniqueValues((First \o Second) \o Third)
             /\ Len((First \o Second) \o Third) =
                  Len(First \o Second) + Len(Third)
        BY <3>3, <3>5, <3>6, ConcatTypedOwnedDisjointReplay
      <3>8. RestartSignatureReplay(node) =
               (First \o Second) \o Third
        BY <2>2 DEF RestartSignatureReplay, First, Second, Third
      <3>9. Len((First \o Second) \o Third) <= 3
        BY <3>1, <3>2, <3>3, <3>5, <3>7, SMT
      <3> QED BY <3>7, <3>8, <3>9
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM RestartLockedBodyReplayCommandsAreFetch ==
  \A node:
    \A candidate \in SequenceSet(RestartLockedBodyReplay(node)):
      candidate.kind = "FetchBody"
BY Isa
   DEF RestartLockedBodyReplay, RestartCandidate,
       AsyncCandidateAtConsumer, AsyncCandidateWithIdentity, SequenceSet

THEOREM RestartLockedBodyReplayCandidateShape ==
  \A node:
    \A candidate \in SequenceSet(RestartLockedBodyReplay(node)):
      RestartLockedBodyPipelineCandidate(node, candidate)
BY Isa
   DEF RestartLockedBodyReplay, RestartLockedBodyPipelineCandidate,
       RestartLockedPrepareQCs, RestartCandidate,
       AsyncCandidateAtConsumer, AsyncCandidateWithIdentity,
       CandidateConsumerCurrent, SequenceSet

THEOREM RestartSignatureReplayCommandsAreSignatures ==
  \A node:
    \A candidate \in SequenceSet(RestartSignatureReplay(node)):
      candidate.kind \in {"SignProposal", "SignVote", "SignTimeout"}
BY RangeConcatenation, Isa
   DEF RestartSignatureReplay, RestartTimeoutOrProposalReplay,
       RestartPrepareReplayIfActive, RestartLockedCommitReplayIfActive,
       RestartTimeoutReplay, RestartProposalReplay,
       RestartPrepareReplay, RestartLockedCommitReplay,
       RestartCandidate, AsyncCandidateAtConsumer,
       AsyncCandidateWithIdentity, SequenceSet

THEOREM RestartLockedBodyAndSignatureReplayAreDisjoint ==
  \A node:
    SequenceSet(RestartLockedBodyReplay(node)) \cap
      SequenceSet(RestartSignatureReplay(node)) = {}
BY RestartLockedBodyReplayCommandsAreFetch,
   RestartSignatureReplayCommandsAreSignatures, SMT

THEOREM RestartReplayReplayingCandidateShape ==
  \A node:
    /\ ~NodeHasApplication(node)
    /\ RestartDecisions(node) = {}
    /\ Len(RestartSignatureReplay(node)) > 0
    => \A candidate \in SequenceSet(RestartReplay(node)):
         \/ candidate
              \in SequenceSet(RestartSignatureReplay(node))
         \/ RestartLockedBodyPipelineCandidate(node, candidate)
BY RestartLockedBodyReplayCandidateShape, RangeConcatenation,
   RangeEquality, Isa
   DEF RestartReplay, SequenceSet

ReplayLockedCommitCandidate(node, vote) ==
  RestartCandidate("Completion", "SignVote", node,
                   vote.view, vote.subject, vote)

THEOREM UndecidedActiveLockedCommitIsInSignatureReplay ==
  \A node \in Responsive:
    \A vote \in RestartLockedCommitIntents(node):
      /\ StrongInductiveInvariant
      /\ ~NodeHasApplication(node)
      /\ ~NodeHasDecision(node)
      => ReplayLockedCommitCandidate(node, vote)
           \in SequenceSet(RestartSignatureReplay(node))
BY RestartLockedCommitChoiceIsAvailable, SMTT(60), Isa
   DEF StrongInductiveInvariant, Safety, TypeInvariant,
       ModelConfiguration, HonestCommitUniqueness,
       RestartLockedCommitIntents, RestartLockedCommitIntent,
       RestartLockedCommitReplayIfActive,
       RestartLockedCommitReplay, RestartSignatureReplay,
       RestartTimeoutOrProposalReplay, RestartPrepareReplayIfActive,
       RestartDecisions, ReplayLockedCommitCandidate, RestartCandidate,
       AsyncCandidateAtConsumer, AsyncCandidateWithIdentity,
       NodeHasDecision, NodeHasApplication, SequenceSet

THEOREM UnreadyActiveLockedCommitIsInSignatureReplay ==
  \A node \in Responsive:
    \A vote \in RestartLockedCommitIntents(node):
      /\ StrongInductiveInvariant
      /\ ~NodeHasApplication(node)
      /\ ~ReplayCommitIntentReady(node, vote)
      => ReplayLockedCommitCandidate(node, vote)
           \in SequenceSet(RestartSignatureReplay(node))
BY UndecidedActiveLockedCommitIsInSignatureReplay, Isa
   DEF ReplayCommitIntentReady

THEOREM ReplayTailRetainsNonHeadValue ==
  \A values:
    \A sequence \in Seq(values):
      \A value:
        /\ Len(sequence) > 0
        /\ value \in SequenceSet(sequence)
        /\ value # Head(sequence)
        => value \in SequenceSet(Tail(sequence))
PROOF
  <1>1. ASSUME NEW values, NEW sequence \in Seq(values), NEW value,
                Len(sequence) > 0,
                value \in SequenceSet(sequence),
                value # Head(sequence)
         PROVE value \in SequenceSet(Tail(sequence))
    <2>1. PICK original \in 1..Len(sequence):
             value = sequence[original]
      BY <1>1 DEF SequenceSet
    <2>2. /\ sequence # <<>>
           /\ Head(sequence) = sequence[1]
           /\ Tail(sequence) \in Seq(values)
           /\ Len(Tail(sequence)) = Len(sequence) - 1
           /\ \A index \in 1..Len(Tail(sequence)):
                Tail(sequence)[index] = sequence[index + 1]
      BY <1>1, EmptySeq, HeadTailProperties, SMT
    <2>3. original - 1 \in 1..Len(Tail(sequence))
      BY <1>1, <2>1, <2>2, SMT
    <2>4. Tail(sequence)[original - 1] = value
      BY <2>1, <2>2, <2>3, SMT
    <2> QED BY <2>3, <2>4 DEF SequenceSet
  <1> QED BY <1>1

THEOREM RestartRunnerAssemblyProperties ==
  \A node \in ValidatorIds:
    TypeInvariant =>
      /\ AsyncQueueTyped(RestartRunnerAssembly(node))
      /\ AsyncCausalQueueOwnership(node, RestartRunnerAssembly(node))
      /\ SequenceHasUniqueValues(RestartRunnerAssembly(node))
      /\ Len(RestartRunnerAssembly(node)) <= 1
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds, TypeInvariant
         PROVE /\ AsyncQueueTyped(RestartRunnerAssembly(node))
               /\ AsyncCausalQueueOwnership(
                    node, RestartRunnerAssembly(node))
               /\ SequenceHasUniqueValues(RestartRunnerAssembly(node))
               /\ Len(RestartRunnerAssembly(node)) <= 1
    <2>1. CASE ~RestartRunnerAssemblyEnabled(node)
      <3>1. RestartRunnerAssembly(node) = <<>>
        BY <2>1 DEF RestartRunnerAssembly
      <3> QED BY <3>1, EmptyReplayProperties
    <2>2. CASE RestartRunnerAssemblyEnabled(node)
      <3> DEFINE Candidate ==
             RestartCandidate(
               "Normal", "AssembleBody", node, nodeView[node],
               AsyncProposalSubject(node), NoAsyncItem)
      <3>1. AsyncCausalCoreTypingFacts
        BY <1>1, CoreTypeImpliesCausalTypingFacts
      <3>2. /\ nodeView[node] \in Views
             /\ AsyncProposalSubject(node) \in SubjectOrNone
             /\ NoAsyncItem \in AsyncEvidenceSet
        BY <1>1, <3>1, SMT
           DEF AsyncCausalCoreTypingFacts, AsyncProposalSubject,
               AsyncEvidenceSet
      <3>3. AsyncCandidateTyped(Candidate)
        BY <1>1, <3>2, RestartCandidateIsTyped
           DEF Candidate, AsyncCommandClasses, AsyncWorkKinds,
               AsyncReducerKinds
      <3>4. Candidate.node = node
        BY DEF Candidate, RestartCandidate, AsyncCandidateAtConsumer,
               AsyncCandidateWithIdentity
      <3>5. /\ AsyncQueueTyped(<<Candidate>>)
             /\ AsyncCausalQueueOwnership(node, <<Candidate>>)
             /\ SequenceHasUniqueValues(<<Candidate>>)
        BY <3>3, <3>4, TypedOwnedSingletonIsReplay
      <3>6. /\ RestartRunnerAssembly(node) = <<Candidate>>
             /\ Len(<<Candidate>>) = 1
        BY <2>2 DEF RestartRunnerAssembly, Candidate
      <3> QED BY <3>5, <3>6
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM AppliedRecoveryCannotScheduleSameHeightAssembly ==
  \A node:
    NodeHasApplication(node) => RestartRunnerAssembly(node) = <<>>
BY DEF RestartRunnerAssembly, RestartRunnerAssemblyEnabled

THEOREM AppliedRecoverySchedulesNoSameHeightWork ==
  \A node:
    NodeHasApplication(node) => RestartReplay(node) = <<>>
BY DEF RestartReplay

THEOREM NonemptyTypedOwnedReplayHeadProperties ==
  \A node, queue:
    /\ AsyncQueueTyped(queue)
    /\ AsyncCausalQueueOwnership(node, queue)
    /\ Len(queue) > 0
    => /\ AsyncQueueTyped(<<Head(queue)>>)
       /\ AsyncCausalQueueOwnership(node, <<Head(queue)>>)
       /\ SequenceHasUniqueValues(<<Head(queue)>>)
       /\ SequenceSet(<<Head(queue)>>) \subseteq SequenceSet(queue)
       /\ Len(<<Head(queue)>>) = 1
BY TypedOwnedSingletonIsReplay, HeadTailProperties, RangeEquality,
   FS_Singleton, SMTT(30), Isa
   DEF AsyncQueueTyped, AsyncCausalQueueOwnership,
       SequenceHasUniqueValues, SequenceSet

THEOREM RestartReplayIsTypedOwnedAndUnique ==
  \A node \in ValidatorIds:
    StrongInductiveInvariant
    => /\ AsyncQueueTyped(RestartReplay(node))
       /\ AsyncCausalQueueOwnership(node, RestartReplay(node))
       /\ SequenceHasUniqueValues(RestartReplay(node))
       /\ Len(RestartReplay(node)) <= 2
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                StrongInductiveInvariant
         PROVE /\ AsyncQueueTyped(RestartReplay(node))
               /\ AsyncCausalQueueOwnership(node, RestartReplay(node))
               /\ SequenceHasUniqueValues(RestartReplay(node))
               /\ Len(RestartReplay(node)) <= 2
    <2>1. TypeInvariant
      BY <1>1, StrongInvariantProjectsType
    <2>2. CASE NodeHasApplication(node)
      <3>1. RestartReplay(node) = <<>>
        BY <2>2 DEF RestartReplay
      <3> QED BY <3>1, EmptyReplayProperties
    <2>3. CASE /\ ~NodeHasApplication(node)
                 /\ RestartDecisions(node) # {}
      <3>1. /\ AsyncQueueTyped(RestartDecisionReplay(node))
             /\ AsyncCausalQueueOwnership(
                  node, RestartDecisionReplay(node))
             /\ SequenceHasUniqueValues(RestartDecisionReplay(node))
        BY <1>1, <2>3, RestartDecisionReplayProperties
      <3>2. /\ RestartReplay(node) = RestartDecisionReplay(node)
             /\ Len(RestartDecisionReplay(node)) = 1
        BY <2>3 DEF RestartReplay, RestartDecisionReplay
      <3> QED BY <3>1, <3>2
    <2>4. CASE /\ ~NodeHasApplication(node)
                 /\ RestartDecisions(node) = {}
      <3> DEFINE Locked == RestartLockedBodyReplay(node)
      <3> DEFINE Signatures == RestartSignatureReplay(node)
      <3>1. /\ AsyncQueueTyped(Locked)
             /\ AsyncCausalQueueOwnership(node, Locked)
             /\ SequenceHasUniqueValues(Locked)
             /\ Len(Locked) <= 1
        BY <1>1, <2>1, RestartLockedBodyReplayProperties DEF Locked
      <3>2. /\ AsyncQueueTyped(Signatures)
             /\ AsyncCausalQueueOwnership(node, Signatures)
             /\ SequenceHasUniqueValues(Signatures)
             /\ Len(Signatures) <= 3
        BY <1>1, <2>1, RestartSignatureReplayProperties DEF Signatures
      <3>3. SequenceSet(Locked) \cap SequenceSet(Signatures) = {}
        BY RestartLockedBodyAndSignatureReplayAreDisjoint
           DEF Locked, Signatures
      <3>4. CASE Len(Signatures) > 0
        <4>1. /\ AsyncQueueTyped(<<Head(Signatures)>>)
               /\ AsyncCausalQueueOwnership(
                    node, <<Head(Signatures)>>)
               /\ SequenceHasUniqueValues(<<Head(Signatures)>>)
               /\ SequenceSet(<<Head(Signatures)>>)
                    \subseteq SequenceSet(Signatures)
               /\ Len(<<Head(Signatures)>>) = 1
          BY <3>2, <3>4, NonemptyTypedOwnedReplayHeadProperties
        <4>2. SequenceSet(Locked) \cap
                 SequenceSet(<<Head(Signatures)>>) = {}
          BY <3>3, <4>1, SMT
        <4>3. /\ AsyncQueueTyped(Locked \o <<Head(Signatures)>>)
               /\ AsyncCausalQueueOwnership(
                    node, Locked \o <<Head(Signatures)>>)
               /\ SequenceHasUniqueValues(
                    Locked \o <<Head(Signatures)>>)
               /\ Len(Locked \o <<Head(Signatures)>>) =
                    Len(Locked) + Len(<<Head(Signatures)>>)
          BY <3>1, <4>1, <4>2, ConcatTypedOwnedDisjointReplay
        <4>4. /\ RestartReplay(node) =
                      Locked \o <<Head(Signatures)>>
               /\ Len(Locked \o <<Head(Signatures)>>) <= 2
          BY <2>4, <3>1, <4>1, <4>3, <3>4, SMT
             DEF RestartReplay, Locked, Signatures
        <4> QED BY <4>3, <4>4
      <3>5. CASE /\ Len(Signatures) = 0
                   /\ Len(Locked) > 0
        <4>1. RestartReplay(node) = Locked
          BY <2>4, <3>5 DEF RestartReplay, Locked, Signatures
        <4> QED BY <3>1, <4>1
      <3>6. CASE /\ Len(Signatures) = 0
                   /\ Len(Locked) = 0
        <4>1. /\ AsyncQueueTyped(RestartRunnerAssembly(node))
               /\ AsyncCausalQueueOwnership(
                    node, RestartRunnerAssembly(node))
               /\ SequenceHasUniqueValues(RestartRunnerAssembly(node))
               /\ Len(RestartRunnerAssembly(node)) <= 1
          BY <2>1, RestartRunnerAssemblyProperties
        <4>2. RestartReplay(node) = RestartRunnerAssembly(node)
          BY <2>4, <3>6 DEF RestartReplay, Locked, Signatures
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>1, <3>2, <3>4, <3>5, <3>6, SMT
    <2> QED BY <2>2, <2>3, <2>4, SMT
  <1> QED BY <1>1

THEOREM InitialCausalCandidateShape ==
  \A node:
    /\ DOMAIN InitialCausalCandidate(node) = AsyncCandidateDomain
    /\ InitialCausalCandidate(node).class \in AsyncCommandClasses
    /\ InitialCausalCandidate(node).kind \in AsyncWorkKinds
    /\ InitialCausalCandidate(node).node = node
    /\ InitialCausalCandidate(node).height = context.height
    /\ InitialCausalCandidate(node).view = nodeView[node]
    /\ InitialCausalCandidate(node).subject = AsyncProposalSubject(node)
    /\ InitialCausalCandidate(node).item = NoAsyncItem
PROOF
  <1>1. ASSUME NEW node
         PROVE /\ DOMAIN InitialCausalCandidate(node) =
                    AsyncCandidateDomain
               /\ InitialCausalCandidate(node).class
                    \in AsyncCommandClasses
               /\ InitialCausalCandidate(node).kind \in AsyncWorkKinds
               /\ InitialCausalCandidate(node).node = node
               /\ InitialCausalCandidate(node).height = context.height
               /\ InitialCausalCandidate(node).view = nodeView[node]
               /\ InitialCausalCandidate(node).subject =
                    AsyncProposalSubject(node)
               /\ InitialCausalCandidate(node).item = NoAsyncItem
    <2>1. DOMAIN InitialCausalCandidate(node) = AsyncCandidateDomain
      BY DEF InitialCausalCandidate, NoItemCandidate, AsyncCandidate,
             AsyncCandidateWithIdentity, AsyncCandidateDomain
    <2>2. InitialCausalCandidate(node).class \in AsyncCommandClasses
      BY DEF InitialCausalCandidate, NoItemCandidate, AsyncCandidate,
             AsyncCandidateWithIdentity, AsyncCommandClasses
    <2>3. InitialCausalCandidate(node).kind \in AsyncWorkKinds
      BY DEF InitialCausalCandidate, NoItemCandidate, AsyncCandidate,
             AsyncCandidateWithIdentity, AsyncWorkKinds, AsyncReducerKinds
    <2>4. /\ InitialCausalCandidate(node).node = node
           /\ InitialCausalCandidate(node).height = context.height
           /\ InitialCausalCandidate(node).view = nodeView[node]
           /\ InitialCausalCandidate(node).subject =
                AsyncProposalSubject(node)
           /\ InitialCausalCandidate(node).item = NoAsyncItem
      BY DEF InitialCausalCandidate, NoItemCandidate, AsyncCandidate,
             AsyncCandidateWithIdentity
    <2> QED BY <2>1, <2>2, <2>3, <2>4
  <1> QED BY <1>1

THEOREM InitialCausalCandidateIsTyped ==
  TypeInvariant
    => \A node \in ValidatorIds:
         AsyncCandidateTyped(
           InitialCausalCandidate(node))
PROOF
  <1>1. ASSUME TypeInvariant,
                NEW node \in ValidatorIds
         PROVE AsyncCandidateTyped(
           InitialCausalCandidate(node))
    <2>1. AsyncCausalCoreTypingFacts
      BY <1>1, CoreTypeImpliesCausalTypingFacts
    <2>2. context.height \in Heights
      BY <2>1 DEF AsyncCausalCoreTypingFacts
    <2>3. nodeView[node] \in Views
      BY <1>1, <2>1, SMT DEF AsyncCausalCoreTypingFacts
    <2>4. AsyncProposalSubject(node) \in SubjectOrNone
      <3>1. CASE highestRank[node] = NoRank
        BY <2>1 DEF AsyncCausalCoreTypingFacts, AsyncProposalSubject
      <3>2. CASE highestRank[node] # NoRank
        BY <1>1, <2>1, SMT
           DEF AsyncCausalCoreTypingFacts, AsyncProposalSubject
      <3> QED BY <3>1, <3>2
    <2>5. /\ context \in ContextRecords
           /\ generation[node] \in Generations
           /\ NoAsyncItem \in AsyncEvidenceSet
      BY <1>1, <2>1, SMT
         DEF AsyncCausalCoreTypingFacts, AsyncEvidenceSet
    <2>6. InitialCausalCandidate(node).node \in ValidatorIds
      BY <1>1, InitialCausalCandidateShape
    <2> QED BY <2>2, <2>3, <2>4, <2>5, <2>6,
      InitialCausalCandidateShape
      DEF AsyncCandidateTyped
  <1> QED BY <1>1

THEOREM SingletonSequenceFacts ==
  \A value:
    /\ <<value>> \in Seq({value})
    /\ Range(<<value>>) = {value}
PROOF
  <1>1. ASSUME NEW value
         PROVE /\ <<value>> \in Seq({value})
               /\ Range(<<value>>) = {value}
    <2>1. [index \in 1..1 |-> value] \in Seq({value})
      BY IsASeq, SMT
    <2>2. <<value>> = [index \in 1..1 |-> value]
      BY Isa
    <2>3. <<value>> \in Seq({value})
      BY <2>1, <2>2
    <2>4. Range(<<value>>) =
             {<<value>>[index]: index \in 1..Len(<<value>>)}
      BY <2>3, RangeEquality
    <2> QED BY <2>3, <2>4, Isa
  <1> QED BY <1>1

THEOREM TypedCandidateFormsTypedSingleton ==
  \A candidate:
    AsyncCandidateTyped(candidate) => AsyncQueueTyped(<<candidate>>)
BY SingletonSequenceFacts, Isa DEF AsyncQueueTyped

THEOREM AsyncInitEstablishesCausalType ==
  \A initialContext:
    (AsyncInitAt(initialContext) /\ TypeInvariant)
      => AsyncCausalTypeInvariant
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncInitAt(initialContext) /\ TypeInvariant
         PROVE AsyncCausalTypeInvariant
    <2>1. asyncCausalQueues =
             [node \in ValidatorIds |->
                <<InitialCausalCandidate(node)>>]
      BY <1>1
         DEF AsyncInitAt, AsyncBaseInitAt, AsyncRuntimeInit,
             InitialCausalCandidate
    <2>2. \A node \in ValidatorIds:
             AsyncCandidateTyped(InitialCausalCandidate(node))
      BY <1>1, InitialCausalCandidateIsTyped
    <2>3. DOMAIN asyncCausalQueues = ValidatorIds
      BY <2>1, SMT
    <2>4. \A node \in ValidatorIds:
             AsyncQueueTyped(asyncCausalQueues[node])
      BY <2>1, <2>2, TypedCandidateFormsTypedSingleton, SMT
    <2>5. \A node \in ValidatorIds:
             AsyncCausalQueueOwnership(node, asyncCausalQueues[node])
      BY <2>1, InitialCausalCandidateShape, SingletonSequenceFacts, SMT
         DEF AsyncCausalQueueOwnership, SequenceSet
    <2> QED BY <2>3, <2>4, <2>5 DEF AsyncCausalTypeInvariant
  <1> QED BY <1>1

THEOREM AsyncInitEstablishesRuntimeType ==
  \A initialContext:
    (AsyncInitAt(initialContext) /\ TypeInvariant)
      => AsyncRuntimeTypeInvariant
BY AsyncInitEstablishesRuntimeScalarType,
   AsyncInitEstablishesCausalType
   DEF AsyncRuntimeTypeInvariant

THEOREM EmptySequenceFacts ==
  /\ <<>> \in Seq({})
  /\ Range(<<>>) = {}
BY EmptySeq, RangeEquality, Isa

THEOREM EmptyAsyncQueueIsTyped ==
  AsyncQueueTyped(<<>>)
BY EmptySequenceFacts, Isa DEF AsyncQueueTyped

THEOREM EmptyAsyncIoSequenceIsTyped ==
  AsyncIoSequenceTyped(<<>>)
BY EmptySequenceFacts, Isa DEF AsyncIoSequenceTyped

THEOREM EmptyAsyncIoServeNonceOwnership ==
  AsyncIoServeNonceOwnership(<<>>)
BY EmptySequenceFacts, Isa
   DEF AsyncIoServeNonceOwnership, AsyncIoServeIndices

THEOREM EmptyAsyncCompletionSequenceIsTyped ==
  AsyncCompletionSequenceTyped(<<>>)
BY EmptySequenceFacts, Isa DEF AsyncCompletionSequenceTyped

THEOREM EmptyAsyncSequenceSet ==
  SequenceSet(<<>>) = {}
BY Isa DEF SequenceSet

THEOREM EmptyAsyncSequenceLengthMatchesCardinality ==
  Len(<<>>) = Cardinality(SequenceSet(<<>>))
BY EmptyAsyncSequenceSet, FS_EmptySet, SMT

THEOREM EmptyQueuedCompletionIndexSet ==
  {index \in 1..Len(<<>>): <<>>[index].class = "Completion"} = {}
BY Isa

THEOREM EmptyAsyncIoConsensusCandidateOwnership ==
  \A node:
    asyncIoQueues[node] = <<>>
      => AsyncIoConsensusCandidateOwnership(
           node, asyncIoQueues, asyncIoReadyCompletions,
           asyncLocalReadyCompletions)
BY Isa
   DEF AsyncIoConsensusCandidateOwnership,
       AsyncIoConsensusQueueOwnership, AsyncIoConsensusIndices,
       SequenceSet

THEOREM AsyncInitEstablishesIoTopologyType ==
  \A initialContext:
    AsyncInitAt(initialContext) => AsyncIoTopologyTypeInvariant
BY SMT
   DEF AsyncInitAt, AsyncBaseInitAt, AsyncIoInit,
       AsyncIoTopologyTypeInvariant

THEOREM AsyncInitEstablishesIoContentType ==
  \A initialContext:
    AsyncInitAt(initialContext) => AsyncIoContentTypeInvariant
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncInitAt(initialContext)
         PROVE AsyncIoContentTypeInvariant
      <2>1. ASSUME NEW node \in ValidatorIds
           PROVE /\ AsyncIoSequenceTyped(asyncIoQueues[node])
                 /\ AsyncIoServeNonceOwnership(asyncIoQueues[node])
                 /\ IsFiniteSet(asyncOutstandingWork[node])
                 /\ \A candidate \in asyncOutstandingWork[node]:
                      /\ AsyncCandidateTyped(candidate)
                      /\ candidate.class = "Completion"
                      /\ candidate.node = node
                 /\ AsyncCompletionSequenceTyped(
                      asyncIoReadyCompletions[node])
                 /\ AsyncCompletionSequenceTyped(
                      asyncLocalReadyCompletions[node])
                 /\ Len(asyncIoReadyCompletions[node]) =
                      Cardinality(SequenceSet(
                        asyncIoReadyCompletions[node]))
                 /\ Len(asyncLocalReadyCompletions[node]) =
                      Cardinality(SequenceSet(
                        asyncLocalReadyCompletions[node]))
                 /\ SequenceSet(asyncIoReadyCompletions[node])
                      \subseteq asyncOutstandingWork[node]
                 /\ SequenceSet(asyncLocalReadyCompletions[node])
                      \subseteq asyncOutstandingWork[node]
                 /\ SequenceSet(asyncIoReadyCompletions[node]) \cap
                      SequenceSet(asyncLocalReadyCompletions[node]) = {}
                 /\ \A job \in SequenceSet(asyncIoQueues[node]):
                      job.class = "Consensus" =>
                        job.candidate \in asyncOutstandingWork[node]
                 /\ AsyncIoConsensusCandidateOwnership(
                      node, asyncIoQueues, asyncIoReadyCompletions,
                      asyncLocalReadyCompletions)
                 /\ SequenceSet(asyncCommandQueues[node]) \cap
                      asyncOutstandingWork[node] = {}
      <3>1. /\ asyncIoQueues[node] = <<>>
             /\ asyncOutstandingWork[node] = {}
             /\ asyncIoReadyCompletions[node] = <<>>
             /\ asyncLocalReadyCompletions[node] = <<>>
             /\ asyncCommandQueues[node] = <<>>
        BY <1>1, <2>1, SMT
           DEF AsyncInitAt, AsyncBaseInitAt, AsyncRuntimeInit, AsyncIoInit
      <3>2. /\ AsyncIoSequenceTyped(asyncIoQueues[node])
             /\ AsyncIoServeNonceOwnership(asyncIoQueues[node])
             /\ AsyncCompletionSequenceTyped(
                  asyncIoReadyCompletions[node])
             /\ AsyncCompletionSequenceTyped(
                  asyncLocalReadyCompletions[node])
        BY <3>1, EmptyAsyncIoSequenceIsTyped,
           EmptyAsyncIoServeNonceOwnership,
           EmptyAsyncCompletionSequenceIsTyped
      <3>3. /\ Len(asyncIoReadyCompletions[node]) =
                  Cardinality(SequenceSet(
                    asyncIoReadyCompletions[node]))
             /\ Len(asyncLocalReadyCompletions[node]) =
                  Cardinality(SequenceSet(
                    asyncLocalReadyCompletions[node]))
        BY <3>1, EmptyAsyncSequenceLengthMatchesCardinality
      <3>4. /\ IsFiniteSet(asyncOutstandingWork[node])
             /\ (\A candidate \in asyncOutstandingWork[node]:
                   /\ AsyncCandidateTyped(candidate)
                   /\ candidate.class = "Completion"
                   /\ candidate.node = node)
             /\ SequenceSet(asyncIoReadyCompletions[node])
                  \subseteq asyncOutstandingWork[node]
             /\ SequenceSet(asyncLocalReadyCompletions[node])
                  \subseteq asyncOutstandingWork[node]
             /\ SequenceSet(asyncIoReadyCompletions[node]) \cap
                  SequenceSet(asyncLocalReadyCompletions[node]) = {}
             /\ (\A job \in SequenceSet(asyncIoQueues[node]):
                   job.class = "Consensus" =>
                     job.candidate \in asyncOutstandingWork[node])
             /\ AsyncIoConsensusCandidateOwnership(
                  node, asyncIoQueues, asyncIoReadyCompletions,
                  asyncLocalReadyCompletions)
             /\ SequenceSet(asyncCommandQueues[node]) \cap
                  asyncOutstandingWork[node] = {}
        BY <3>1, EmptyAsyncSequenceSet,
           EmptyAsyncIoConsensusCandidateOwnership, FS_EmptySet, SMT
      <3> QED BY <3>2, <3>3, <3>4
    <2> QED BY <2>1
         DEF AsyncIoContentTypeInvariant,
             AsyncIoQueueContentTypeInvariant,
             AsyncIoWorkContentTypeInvariant
  <1> QED BY <1>1

THEOREM AsyncInitEstablishesIoCapacityType ==
  \A initialContext:
    AsyncInitAt(initialContext) => AsyncIoCapacityTypeInvariant
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncInitAt(initialContext)
         PROVE AsyncIoCapacityTypeInvariant
    <2>1. ASSUME NEW node \in ValidatorIds
           PROVE /\ AsyncQueueDepth(node) <= AsyncQueueCapacity
                 /\ AsyncIoQueueDepth(node) <= AsyncIoCapacity
                 /\ AsyncOutstandingWorkCount(node) <= AsyncIoWorkCapacity
      <3>1. /\ asyncCommandQueues[node] = <<>>
             /\ asyncIoQueues[node] = <<>>
             /\ asyncOutstandingWork[node] = {}
             /\ asyncDeferredCompletionQueues[node] = <<>>
        BY <1>1, <2>1, SMT
           DEF AsyncInitAt, AsyncBaseInitAt, AsyncRuntimeInit, AsyncIoInit,
               AsyncDeferredInit
      <3>2. /\ AsyncQueueDepth(node) = 0
             /\ AsyncIoQueueDepth(node) = 0
             /\ AsyncOutstandingWorkCount(node) = 0
             /\ DeferredCompletionCount(node) = 0
        BY <3>1, Isa
           DEF AsyncQueueDepth, AsyncIoQueueDepth,
               AsyncOutstandingWorkCount, DeferredCompletionCount
      <3>3. QueuedCompletionIndices(node) = {}
        BY <3>1, EmptyQueuedCompletionIndexSet, SMT
           DEF QueuedCompletionIndices
      <3>4. AsyncCompletionLoad(node) = 0
        BY <3>1, <3>2, <3>3, FS_EmptySet, SMT
           DEF AsyncCompletionLoad, AsyncOutstandingWorkCount,
               QueuedCompletionCount
      <3>5. /\ AsyncQueueCapacity \in Nat
             /\ AsyncCompletionReserve \in Nat
             /\ AsyncIoCapacity \in Nat
             /\ AsyncIoWorkCapacity \in Nat
        BY <1>1, SMT
           DEF AsyncInitAt, AsyncBaseInitAt, AsyncConfiguration,
               AsyncIoCapacity
      <3> QED BY <3>2, <3>5, SMT
    <2> QED BY <2>1 DEF AsyncIoCapacityTypeInvariant
  <1> QED BY <1>1

THEOREM AsyncInitEstablishesIoType ==
  \A initialContext:
    (AsyncInitAt(initialContext) /\ TypeInvariant)
      => AsyncIoTypeInvariant
BY AsyncInitEstablishesIoTopologyType,
   AsyncInitEstablishesIoContentType,
   AsyncInitEstablishesIoCapacityType
   DEF AsyncIoTypeInvariant

THEOREM AsyncInitEstablishesDeferredTopologyType ==
  \A initialContext:
    AsyncInitAt(initialContext) => AsyncDeferredTopologyTypeInvariant
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncInitAt(initialContext)
         PROVE AsyncDeferredTopologyTypeInvariant
    <2>1. /\ asyncDeferredCompletionQueues =
                [node \in ValidatorIds |-> <<>>]
           /\ asyncDeferredProgressQueues =
                [node \in ValidatorIds |-> <<>>]
           /\ asyncDeferredNormalQueues =
                [node \in ValidatorIds |-> <<>>]
           /\ asyncDeferredHandoffs =
                [node \in ValidatorIds |-> NoAsyncDeferredHandoff]
           /\ asyncNextDeferredClass =
                [node \in ValidatorIds |-> "Completion"]
           /\ asyncDeferredDrainOwed =
                [node \in ValidatorIds |-> FALSE]
      BY <1>1
         DEF AsyncInitAt, AsyncBaseInitAt, AsyncDeferredInit
    <2>2. /\ DOMAIN asyncDeferredCompletionQueues = ValidatorIds
           /\ DOMAIN asyncDeferredProgressQueues = ValidatorIds
           /\ DOMAIN asyncDeferredNormalQueues = ValidatorIds
      BY <2>1, Isa
    <2>3. asyncNextDeferredClass
             \in [ValidatorIds -> AsyncCommandClasses]
      BY <2>1, Isa DEF AsyncCommandClasses
    <2>4. /\ asyncDeferredHandoffs
                  \in [ValidatorIds -> AsyncDeferredHandoffSet]
           /\ asyncDeferredDrainOwed
                  \in [ValidatorIds -> BOOLEAN]
      BY <2>1, Isa DEF AsyncDeferredHandoffSet
    <2> QED BY <2>2, <2>3, <2>4
         DEF AsyncDeferredTopologyTypeInvariant
  <1> QED BY <1>1

THEOREM AsyncInitEstablishesDeferredContentType ==
  \A initialContext:
    AsyncInitAt(initialContext) => AsyncDeferredContentTypeInvariant
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncInitAt(initialContext)
         PROVE AsyncDeferredContentTypeInvariant
    <2>1. ASSUME NEW node \in ValidatorIds
           PROVE /\ AsyncCompletionSequenceTyped(
                        asyncDeferredCompletionQueues[node])
                 /\ AsyncQueueTyped(asyncDeferredProgressQueues[node])
                 /\ AsyncQueueTyped(asyncDeferredNormalQueues[node])
                 /\ AsyncCommandQueueOwnership(
                      node, asyncDeferredCompletionQueues[node])
                 /\ AsyncCommandQueueOwnership(
                      node, asyncDeferredProgressQueues[node])
                 /\ AsyncCommandQueueOwnership(
                      node, asyncDeferredNormalQueues[node])
                 /\ \A candidate \in
                        SequenceSet(asyncDeferredProgressQueues[node]):
                      candidate.class = "Progress"
                 /\ \A candidate \in
                        SequenceSet(asyncDeferredNormalQueues[node]):
                      candidate.class = "Normal"
                 /\ Len(asyncDeferredProgressQueues[node]) <=
                      AsyncDeferredProgressCapacity
                 /\ Len(asyncDeferredNormalQueues[node]) <=
                      AsyncDeferredNormalCapacity
      <3>1. /\ asyncDeferredCompletionQueues[node] = <<>>
             /\ asyncDeferredProgressQueues[node] = <<>>
             /\ asyncDeferredNormalQueues[node] = <<>>
        BY <1>1, <2>1, SMT
           DEF AsyncInitAt, AsyncBaseInitAt, AsyncDeferredInit
      <3>2. /\ AsyncCompletionSequenceTyped(
                  asyncDeferredCompletionQueues[node])
             /\ AsyncQueueTyped(asyncDeferredProgressQueues[node])
             /\ AsyncQueueTyped(asyncDeferredNormalQueues[node])
        BY <3>1, EmptyAsyncQueueIsTyped,
           EmptyAsyncCompletionSequenceIsTyped
      <3>3. /\ (\A candidate \in
                       SequenceSet(asyncDeferredProgressQueues[node]):
                     candidate.class = "Progress")
             /\ (\A candidate \in
                       SequenceSet(asyncDeferredNormalQueues[node]):
                     candidate.class = "Normal")
        BY <3>1, EmptyAsyncSequenceSet, SMT
      <3>4. /\ AsyncCommandQueueOwnership(
                    node, asyncDeferredCompletionQueues[node])
             /\ AsyncCommandQueueOwnership(
                    node, asyncDeferredProgressQueues[node])
             /\ AsyncCommandQueueOwnership(
                    node, asyncDeferredNormalQueues[node])
        BY <3>1, EmptyAsyncSequenceSet
           DEF AsyncCommandQueueOwnership
      <3>5. /\ Len(asyncDeferredProgressQueues[node]) <=
                  AsyncDeferredProgressCapacity
             /\ Len(asyncDeferredNormalQueues[node]) <=
                  AsyncDeferredNormalCapacity
        BY <1>1, <3>1, SMT
           DEF AsyncInitAt, AsyncBaseInitAt, AsyncConfiguration
      <3> QED BY <3>2, <3>3, <3>4, <3>5
    <2> QED BY <2>1 DEF AsyncDeferredContentTypeInvariant
  <1> QED BY <1>1

THEOREM AsyncInitEstablishesDeferredHandoffOwnership ==
  \A initialContext:
    AsyncInitAt(initialContext) =>
      AsyncDeferredHandoffOwnershipInvariant
BY Isa
   DEF AsyncInitAt, AsyncBaseInitAt, AsyncDeferredInit,
       AsyncDeferredHandoffOwnershipInvariant, DeferredHandoffActive,
       NoAsyncDeferredHandoff

THEOREM AsyncInitEstablishesDeferredType ==
  \A initialContext:
    (AsyncInitAt(initialContext) /\ TypeInvariant)
      => AsyncDeferredTypeInvariant
BY AsyncInitEstablishesDeferredTopologyType,
   AsyncInitEstablishesDeferredContentType
   DEF AsyncDeferredTypeInvariant

THEOREM SaturatingLinearTimeoutIsPositiveNatural ==
  \A base, maximum, roundView \in Nat:
    /\ base > 0
    /\ maximum > 0
    => (IF base * (roundView + 1) <= maximum
        THEN base * (roundView + 1)
        ELSE maximum) \in Nat \ {0}
BY SMT

THEOREM AsyncViewTimeoutIsPositiveNatural ==
  \A roundView \in Nat:
    AsyncConfiguration => AsyncViewTimeout(roundView) \in Nat \ {0}
BY SaturatingLinearTimeoutIsPositiveNatural, SMT
   DEF AsyncConfiguration, AsyncViewTimeout, AsyncLinearViewTimeout

THEOREM SaturatingLinearTimeoutExceedsRepresentableBound ==
  \A base, maximum, bound \in Nat:
    /\ base > 0
    /\ bound < maximum
    => (IF base * (bound + 1) <= maximum
        THEN base * (bound + 1)
        ELSE maximum) > bound
BY SMT

THEOREM AsyncWorstCaseServiceBudgetIsNatural ==
  /\ ModelConfiguration
  /\ AsyncConfiguration
  => AsyncWorstCaseServiceBudget \in Nat
PROOF
  <1>1. ASSUME ModelConfiguration, AsyncConfiguration
         PROVE AsyncWorstCaseServiceBudget \in Nat
    <2>1. N \in Nat
      BY <1>1, SMT DEF ModelConfiguration, QuorumConfiguration
    <2>2. /\ AsyncQueueCapacity \in Nat
           /\ AsyncProgressReserve \in Nat
           /\ AsyncCompletionReserve \in Nat
           /\ AsyncIngressCapacity \in Nat
           /\ AsyncIoAuxCapacity \in Nat
           /\ AsyncIoWorkCapacity \in Nat
           /\ AsyncDeferredNormalCapacity \in Nat
           /\ AsyncDeferredProgressCapacity \in Nat
           /\ AsyncDeliveryBound \in Nat
           /\ AsyncRetransmitPeriod \in Nat
           /\ AsyncChunkCount \in Nat
      BY <1>1, SMT DEF AsyncConfiguration
    <2>3. /\ AsyncRunnerCycleBudget \in Nat
           /\ AsyncRuntimeCycleBudget \in Nat
           /\ AsyncIoDrainBudget \in Nat
           /\ AsyncDeferredDrainBudget \in Nat
           /\ AsyncCausalCandidateLifecycleCapacity \in Nat
           /\ AsyncCandidateProducerEpisodeCapacity \in Nat
           /\ AsyncCandidateProducerEpisodeBudget \in Nat
           /\ AsyncCandidateProducerActionEpisodeBudget \in Nat
           /\ AsyncCandidatePhysicalServiceBudget \in Nat
           /\ AsyncRetainedControlBudget \in Nat
           /\ AsyncRetainedProposalChunkBudget \in Nat
           /\ AsyncActiveCertifiedRequestBudget \in Nat
           /\ AsyncActiveCommitRequestBudget \in Nat
      BY <2>1, <2>2, SMT
         DEF AsyncRunnerCycleBudget, AsyncRuntimeCycleBudget,
             AsyncIoDrainBudget,
             AsyncDeferredDrainBudget,
             AsyncCausalCandidateLifecycleCapacity,
             AsyncCandidateProducerEpisodeCapacity,
             AsyncCandidateProducerEpisodeBudget,
             AsyncCandidateProducerActionEpisodeBudget,
             AsyncCandidatePhysicalServiceBudget,
             AsyncRetainedControlBudget,
             AsyncRetainedProposalChunkBudget,
             AsyncActiveCertifiedRequestBudget,
             AsyncActiveCommitRequestBudget
    <2>4. /\ AsyncActiveRequestBudget \in Nat
           /\ AsyncRetransmitEmissionBudget \in Nat
      BY <2>3, SMT
         DEF AsyncActiveRequestBudget, AsyncRetransmitEmissionBudget
    <2>5. /\ AsyncOneWayTransportBudget \in Nat
           /\ AsyncProposalPipelineBudget \in Nat
      BY <2>1, <2>2, <2>3, <2>4, SMT
         DEF AsyncOneWayTransportBudget, AsyncProposalPipelineBudget
    <2>6. AsyncCertifiedRecoveryBudget \in Nat
      BY <2>2, <2>3, <2>5, SMT
         DEF AsyncCertifiedRecoveryBudget
    <2>7. /\ AsyncViewSynchronizationBudget \in Nat
           /\ AsyncFixedCorridorServiceBudget \in Nat
      BY <2>2, <2>3, <2>5, SMT
         DEF AsyncViewSynchronizationBudget,
             AsyncFixedCorridorServiceBudget
    <2> QED BY <2>7, SMT
         DEF AsyncWorstCaseServiceBudget
  <1> QED BY <1>1

THEOREM AsyncServiceBudgetSplitReconstructsWholePipeline ==
  AsyncWorstCaseServiceBudget
    = AsyncProposalPipelineBudget * AsyncDeliveryBound
        + AsyncCertifiedRecoveryBudget
        + 4 * AsyncRetransmitPeriod
        + AsyncProgressReserve + AsyncCompletionReserve
BY SMT
   DEF AsyncWorstCaseServiceBudget,
       AsyncViewSynchronizationBudget,
       AsyncFixedCorridorServiceBudget,
       AsyncCertifiedRecoveryBudget

THEOREM AdequateViewTimeoutExists ==
  /\ ModelConfiguration
  /\ AsyncConfiguration
  /\ ViewDomain = Nat
  => \E roundView \in Views:
       /\ roundView <= AsyncMaximumView
       /\ AsyncViewTimeout(roundView) > AsyncWorstCaseServiceBudget
PROOF
  <1>1. ASSUME ModelConfiguration,
                AsyncConfiguration,
                ViewDomain = Nat
         PROVE \E roundView \in Views:
                 /\ roundView <= AsyncMaximumView
                 /\ AsyncViewTimeout(roundView) >
                      AsyncWorstCaseServiceBudget
    <2>1. AsyncWorstCaseServiceBudget \in Nat
      BY <1>1, AsyncWorstCaseServiceBudgetIsNatural
    <2>2. AsyncViewTimeout(AsyncWorstCaseServiceBudget) >
             AsyncWorstCaseServiceBudget
      BY <1>1, <2>1,
         SaturatingLinearTimeoutExceedsRepresentableBound
         DEF AsyncConfiguration, AsyncServiceBoundRepresentable,
             AsyncViewTimeout, AsyncLinearViewTimeout
    <2>3. AsyncWorstCaseServiceBudget \in Views
      BY <1>1, <2>1 DEF Views
    <2>4. AsyncWorstCaseServiceBudget <= AsyncMaximumView
      BY <1>1 DEF AsyncConfiguration, AsyncServiceBoundRepresentable
    <2> QED BY <2>2, <2>3, <2>4
  <1> QED BY <1>1

THEOREM AsyncInitEstablishesTransportClockType ==
  \A initialContext:
    AsyncInitAt(initialContext) => AsyncTransportClockTypeInvariant
PROOF
  <1>1. ASSUME NEW initialContext, AsyncInitAt(initialContext)
         PROVE AsyncTransportClockTypeInvariant
    <2>1. /\ AsyncConfiguration
           /\ nodeView = [node \in ValidatorIds |-> 0]
           /\ asyncOutstandingTags = [node \in ValidatorIds |-> {}]
           /\ asyncNodeDeadlines =
                [node \in ValidatorIds |-> AsyncViewTimeout(nodeView[node])]
           /\ asyncRetransmitDeadlines =
                [node \in ValidatorIds |-> AsyncRetransmitPeriod]
           /\ asyncNodeServiceDeadlines =
                [node \in ValidatorIds |-> AsyncDeliveryBound]
           /\ asyncIoServiceDeadlines =
                [node \in ValidatorIds |-> AsyncDeliveryBound]
      BY <1>1
         DEF AsyncInitAt, AsyncBaseInitAt, InitAt, AsyncTransportInit
    <2>2. /\ AsyncViewTimeout(0) \in Nat
           /\ AsyncRetransmitPeriod \in Nat
           /\ AsyncDeliveryBound \in Nat
      BY <2>1, AsyncViewTimeoutIsPositiveNatural, SMT
         DEF AsyncConfiguration
    <2>3. /\ asyncOutstandingTags \in [ValidatorIds -> SUBSET AsyncCompletionTags]
           /\ asyncNodeDeadlines \in [ValidatorIds -> Nat]
           /\ asyncRetransmitDeadlines \in [ValidatorIds -> Nat]
           /\ asyncNodeServiceDeadlines \in [ValidatorIds -> Nat]
           /\ asyncIoServiceDeadlines \in [ValidatorIds -> Nat]
      BY <2>1, <2>2, Isa
    <2> QED BY <2>3 DEF AsyncTransportClockTypeInvariant
  <1> QED BY <1>1

THEOREM EmptyRetainedClassItems ==
  \A source, controlClass:
    RetainedClassItems({}, source, controlClass) = {}
BY Isa DEF RetainedClassItems

THEOREM AsyncInitEstablishesTransportContentType ==
  \A initialContext:
    AsyncInitAt(initialContext) => AsyncTransportContentTypeInvariant
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncInitAt(initialContext)
         PROVE AsyncTransportContentTypeInvariant
    <2>1. /\ asyncSentItems = {}
           /\ asyncRetainedControl = {}
           /\ asyncActiveRequests = {}
           /\ asyncCertifiedResponseClaim = {}
           /\ asyncTransport = {}
           /\ asyncHeldChunks = {}
      BY <1>1 DEF AsyncInitAt, AsyncBaseInitAt, AsyncTransportInit
    <2>2. /\ IsFiniteSet(asyncSentItems)
           /\ IsFiniteSet(asyncRetainedControl)
           /\ IsFiniteSet(asyncActiveRequests)
           /\ IsFiniteSet(asyncTransport)
      BY <2>1, FS_EmptySet, SMT
    <2>3. /\ (\A item \in asyncSentItems: AsyncItemTyped(item))
           /\ (\A item \in asyncRetainedControl:
                 /\ AsyncItemTyped(item)
                 /\ item.kind \in AsyncControlKinds)
           /\ asyncActiveRequests \subseteq asyncSentItems
           /\ (\A item \in asyncActiveRequests:
                 /\ AsyncItemTyped(item)
                 /\ item.kind \in {"CertifiedRequest",
                                     "CommitCertificateRequest"})
           /\ (\A packet \in asyncTransport: AsyncPacketTyped(packet))
           /\ asyncHeldChunks \subseteq AsyncChunkReceiptSet
      BY <2>1, SMT
    <2>4. AsyncActiveRequestLogicalIndexConsistencyInvariant
      BY <2>1, Isa
         DEF AsyncActiveRequestLogicalIndexConsistencyInvariant,
             AsyncCertifiedRequestLogicalIndexConsistent,
             AsyncCertifiedRequestsIn
    <2>5. AsyncCertifiedResponseClaimInvariant
      BY <2>1, FS_EmptySet, SMT
         DEF AsyncCertifiedResponseClaimInvariant,
             ActiveCertifiedRequestHashes,
             ActiveCertifiedRequestHashesIn,
             AsyncCertifiedRequestsIn,
             CertifiedResponseAuthorityReady,
             CertifiedResponseAuthorityClaimed
    <2>6. \A source \in ValidatorIds,
                controlClass \in AsyncControlKinds:
             LET retained ==
                   RetainedClassItems(
                     asyncRetainedControl, source, controlClass)
             IN \/ retained = {}
                \/ /\ Cardinality(retained) <=
                         Cardinality(CurrentVoters)
                   /\ {item.envelope.recipient: item \in retained}
                        = CurrentVoters
                   /\ \A left, right \in retained:
                        ControlView(left) = ControlView(right)
      BY <2>1, EmptyRetainedClassItems
    <2> QED BY <2>2, <2>3, <2>4, <2>5, <2>6
      DEF AsyncTransportContentTypeInvariant,
          AsyncTransportHistoryTypeInvariant,
          AsyncPacketContentTypeInvariant,
          AsyncHeldChunksTypeInvariant
  <1> QED BY <1>1

THEOREM AsyncInitEstablishesTransportType ==
  \A initialContext:
    (AsyncInitAt(initialContext) /\ TypeInvariant)
      => AsyncTransportTypeInvariant
BY AsyncInitEstablishesTransportClockType,
   AsyncInitEstablishesTransportContentType
   DEF AsyncTransportTypeInvariant

THEOREM EmptyIngressReadySourceSet ==
  \A sources:
    {source \in sources:
       Len([entry \in sources |-> <<>>][source]) > 0} = {}
BY Isa

THEOREM EmptyIngressZeroLaneSourceSet ==
  \A sources:
    {source \in sources:
       Len([entry \in sources |-> <<>>][source]) = 0} = sources
BY Isa

THEOREM EmptyIngressIndexedPairSet ==
  \A sources:
    \A capacity \in Nat:
      {pair \in sources \X (1..capacity): pair[2] <= Len(<<>>)} = {}
PROOF
  <1>1. ASSUME NEW sources, NEW capacity \in Nat
         PROVE {pair \in sources \X (1..capacity):
                  pair[2] <= Len(<<>>)} = {}
    <2>1. {pair \in sources \X (1..capacity):
             pair[2] <= Len(<<>>)} \subseteq {}
      <3>1. ASSUME NEW pair \in
                       {entry \in sources \X (1..capacity):
                          entry[2] <= Len(<<>>)}
             PROVE pair \in {}
        <4>1. /\ pair[2] \in 1..capacity
               /\ pair[2] <= Len(<<>>)
          BY <3>1, Isa
        <4>2. /\ pair[2] >= 1
               /\ Len(<<>>) = 0
          BY <4>1, Isa
        <4> QED BY <4>1, <4>2, SMT
      <3> QED BY <3>1
    <2>2. {} \subseteq
             {pair \in sources \X (1..capacity):
                pair[2] <= Len(<<>>)}
      BY Isa
    <2> QED BY <2>1, <2>2, Isa
  <1> QED BY <1>1

THEOREM AsyncIngressSourcesAreFinite ==
  (AsyncConfiguration /\ ModelConfiguration)
    => IsFiniteSet(AsyncIngressSources)
PROOF
  <1>1. ASSUME AsyncConfiguration /\ ModelConfiguration
         PROVE IsFiniteSet(AsyncIngressSources)
    <2>1. /\ 0 \in Int
           /\ N - 1 \in Int
      BY <1>1, SMT
         DEF AsyncConfiguration, ModelConfiguration, QuorumConfiguration
    <2>2. IsFiniteSet(ValidatorIds)
      BY <2>1, FS_Interval DEF ValidatorIds
    <2>3. IsFiniteSet(ValidatorIds \cup {AsyncUntrustedSource})
      BY <2>2, FS_AddElement
    <2> QED BY <2>3 DEF AsyncIngressSources
  <1> QED BY <1>1

THEOREM AsyncIngressSourceCardinalityIsNatural ==
  (AsyncConfiguration /\ ModelConfiguration)
    => Cardinality(AsyncIngressSources) \in Nat
BY AsyncIngressSourcesAreFinite, FS_CardinalityType

THEOREM AsyncValidatorCardinalityIsNatural ==
  (AsyncConfiguration /\ ModelConfiguration)
    => Cardinality(ValidatorIds) \in Nat
PROOF
  <1>1. ASSUME AsyncConfiguration /\ ModelConfiguration
         PROVE Cardinality(ValidatorIds) \in Nat
    <2>1. IsFiniteSet(AsyncIngressSources)
      BY <1>1, AsyncIngressSourcesAreFinite
    <2>2. ValidatorIds \subseteq AsyncIngressSources
      BY Isa DEF AsyncIngressSources
    <2>3. IsFiniteSet(ValidatorIds)
      BY <2>1, <2>2, FS_Subset
    <2> QED BY <2>3, FS_CardinalityType
  <1> QED BY <1>1

THEOREM AsyncIngressCapacityGeometry ==
  ModelConfiguration
    => /\ Cardinality(ValidatorIds) = N
       /\ Cardinality(AsyncIngressSources) = N + 1
PROOF
  <1>1. ASSUME ModelConfiguration
         PROVE /\ Cardinality(ValidatorIds) = N
               /\ Cardinality(AsyncIngressSources) = N + 1
    <2>1. /\ N \in Nat \ {0}
           /\ IsFiniteSet(ValidatorIds)
           /\ Cardinality(ValidatorIds) = N
      BY <1>1, FS_Interval, SMT
         DEF ValidatorIds, ModelConfiguration, QuorumConfiguration
    <2>2. AsyncUntrustedSource \notin ValidatorIds
      BY SMT DEF AsyncUntrustedSource, ValidatorIds
    <2>3. Cardinality(ValidatorIds \cup {AsyncUntrustedSource}) = N + 1
      BY <2>1, <2>2, FS_AddElement, SMT
    <2> QED BY <2>1, <2>3 DEF AsyncIngressSources
  <1> QED BY <1>1

THEOREM ValidValidatorTimeoutVotePassesByteGate ==
  \A item:
    /\ item.kind = "TimeoutVote"
    /\ item.source \in ValidatorIds
    /\ ~IngressLaneHasTimeoutVoteIn(
          asyncIngressLanes, item.envelope.recipient, item.source)
    => AsyncTimeoutVoteByteGateAllows(item)
BY SMT
   DEF AsyncTimeoutVoteByteGateAllows,
       IngressLaneHasTimeoutVoteIn,
       AsyncValidTimeoutVoteWireByteBound,
       AsyncTimeoutVoteByteReserve

THEOREM AsyncInitEstablishesIngressTopologyType ==
  \A initialContext:
    AsyncInitAt(initialContext) => AsyncIngressTopologyTypeInvariant
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncInitAt(initialContext)
         PROVE AsyncIngressTopologyTypeInvariant
    <2>1. /\ DOMAIN asyncIngressLanes = ValidatorIds
           /\ DOMAIN asyncIngressReady = ValidatorIds
      BY <1>1, SMT
         DEF AsyncInitAt, AsyncBaseInitAt, AsyncIngressInit
    <2>2. ASSUME NEW recipient \in ValidatorIds
           PROVE /\ DOMAIN asyncIngressLanes[recipient] =
                        AsyncIngressSources
                 /\ DOMAIN asyncIngressReady[recipient] =
                      1..Len(asyncIngressReady[recipient])
                 /\ SequenceSet(asyncIngressReady[recipient])
                      \subseteq AsyncIngressSources
                 /\ Len(asyncIngressReady[recipient]) =
                      Cardinality(
                        SequenceSet(asyncIngressReady[recipient]))
                 /\ SequenceSet(asyncIngressReady[recipient]) =
                      {source \in AsyncIngressSources:
                         IngressLaneDepth(recipient, source) > 0}
      <3>1. /\ asyncIngressLanes[recipient] =
                    [source \in AsyncIngressSources |-> <<>>]
             /\ asyncIngressReady[recipient] = <<>>
        BY <1>1, <2>2, SMT
           DEF AsyncInitAt, AsyncBaseInitAt, AsyncIngressInit
      <3>2. /\ DOMAIN asyncIngressLanes[recipient] =
                    AsyncIngressSources
             /\ DOMAIN asyncIngressReady[recipient] =
                  1..Len(asyncIngressReady[recipient])
             /\ asyncIngressReady[recipient]
                  \in Seq(Range(asyncIngressReady[recipient]))
        BY <3>1, EmptyAsyncQueueIsTyped, SMT DEF AsyncQueueTyped
      <3>3. /\ SequenceSet(asyncIngressReady[recipient])
                    \subseteq AsyncIngressSources
             /\ Len(asyncIngressReady[recipient]) =
                  Cardinality(
                    SequenceSet(asyncIngressReady[recipient]))
        BY <3>1, EmptyAsyncSequenceSet,
           EmptyAsyncSequenceLengthMatchesCardinality, SMT
      <3>4. SequenceSet(asyncIngressReady[recipient]) =
               {source \in AsyncIngressSources:
                  IngressLaneDepth(recipient, source) > 0}
        BY <3>1, EmptyAsyncSequenceSet,
           EmptyIngressReadySourceSet, SMT
           DEF IngressLaneDepth, IngressLane
      <3> QED BY <3>2, <3>3, <3>4
    <2> QED BY <2>1, <2>2 DEF AsyncIngressTopologyTypeInvariant
  <1> QED BY <1>1

THEOREM AsyncInitEstablishesIngressCapacityType ==
  \A initialContext:
    AsyncInitAt(initialContext) => AsyncIngressCapacityTypeInvariant
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncInitAt(initialContext)
         PROVE AsyncIngressCapacityTypeInvariant
    <2>1. ASSUME NEW recipient \in ValidatorIds
           PROVE /\ \A source \in AsyncIngressSources:
                         IngressLaneDepth(recipient, source) <=
                           AsyncIngressCapacity
                 /\ IngressDepth(recipient) <= AsyncIngressCapacity
                 /\ IngressDepth(recipient)
                      + IngressProtectedSlotCountFor(
                          asyncIngressLanes, recipient)
                      <= AsyncIngressCapacity
      <3>1. /\ asyncIngressLanes[recipient] =
                    [source \in AsyncIngressSources |-> <<>>]
             /\ AsyncIngressCapacity \in Nat
             /\ AsyncIngressCapacity >= 5 * N + 2
        BY <1>1, <2>1, SMT
           DEF AsyncInitAt, AsyncBaseInitAt, AsyncIngressInit,
               AsyncConfiguration
      <3>2. {pair \in AsyncIngressSources \X
                      (1..AsyncIngressCapacity):
                 pair[2] <= IngressLaneDepth(recipient, pair[1])} = {}
        BY <3>1, EmptyIngressIndexedPairSet, SMT
           DEF IngressLaneDepth, IngressLane
      <3>3. IngressDepth(recipient) = 0
        BY <3>2, FS_EmptySet, SMT DEF IngressDepth
      <3>4. IngressProtectedSourcesFor(
                 asyncIngressLanes, recipient) = AsyncIngressSources
        BY <3>1, EmptyIngressZeroLaneSourceSet, SMT
           DEF IngressProtectedSourcesFor,
               IngressLaneHasNonTimeoutProgressIn, IngressAdmissionClass,
               IngressProgressKinds, SequenceSet
      <3>5. AsyncConfiguration /\ ModelConfiguration
        BY <1>1 DEF AsyncInitAt, AsyncBaseInitAt, InitAt
      <3>6. Cardinality(AsyncIngressSources) \in Nat
        BY <3>5, AsyncIngressSourceCardinalityIsNatural
      <3>7. IngressContinuationProtectedSourcesFor(
                 asyncIngressLanes, recipient) = ValidatorIds
        <4>1. ValidatorIds \subseteq AsyncIngressSources
          BY Isa DEF AsyncIngressSources
        <4>2. \A source \in ValidatorIds:
                 Len(asyncIngressLanes[recipient][source]) = 0
          BY <3>1, <4>1, Isa
        <4> QED BY <4>2, Isa
             DEF IngressContinuationProtectedSourcesFor,
                 IngressProtectedClassesPresentIn,
                 IngressLaneHasNonTimeoutProgressIn,
                 IngressLaneHasCertifiedFenceEscapeIn,
                 IngressLaneHasTimeoutVoteIn,
                 IngressLaneHasTransportCompletionIn, SequenceSet
      <3>8. IngressTimeoutVoteProtectedSourcesFor(
                 asyncIngressLanes, recipient) = ValidatorIds
        <4>1. ValidatorIds \subseteq AsyncIngressSources
          BY Isa DEF AsyncIngressSources
        <4>2. \A source \in ValidatorIds:
                 Len(asyncIngressLanes[recipient][source]) = 0
          BY <3>1, <4>1, Isa
        <4> QED BY <4>2, Isa
             DEF IngressTimeoutVoteProtectedSourcesFor,
                 IngressLaneHasTimeoutVoteIn, SequenceSet
      <3>8b. IngressCertifiedFenceEscapeProtectedSourcesFor(
                  asyncIngressLanes, recipient) = ValidatorIds
        <4>1. ValidatorIds \subseteq AsyncIngressSources
          BY Isa DEF AsyncIngressSources
        <4>2. \A source \in ValidatorIds:
                 Len(asyncIngressLanes[recipient][source]) = 0
          BY <3>1, <4>1, Isa
        <4> QED BY <4>2, Isa
             DEF IngressCertifiedFenceEscapeProtectedSourcesFor,
                 IngressLaneHasCertifiedFenceEscapeIn, SequenceSet
      <3>8c. IngressTransportCompletionProtectedSourcesFor(
                  asyncIngressLanes, recipient) = AsyncIngressSources
        <4>1. \A source \in AsyncIngressSources:
                 Len(asyncIngressLanes[recipient][source]) = 0
          BY <3>1, Isa
        <4> QED BY <4>1, Isa
             DEF IngressTransportCompletionProtectedSourcesFor,
                 IngressLaneHasTransportCompletionIn, SequenceSet
      <3>9. Cardinality(ValidatorIds) \in Nat
        BY <3>5, AsyncValidatorCardinalityIsNatural
      <3>10. /\ Cardinality(ValidatorIds) = N
               /\ Cardinality(AsyncIngressSources) = N + 1
        BY <3>5, AsyncIngressCapacityGeometry
      <3>11. \A source \in AsyncIngressSources:
               IngressLaneDepth(recipient, source) <=
                 AsyncIngressCapacity
        BY <3>1, SMT DEF IngressLaneDepth, IngressLane
      <3>12. IngressDepth(recipient) <= AsyncIngressCapacity
        BY <3>1, <3>3, SMT
      <3>13. IngressDepth(recipient)
                + IngressProtectedSlotCountFor(
                    asyncIngressLanes, recipient)
              = 5 * N + 2
        BY <3>3, <3>4, <3>6, <3>7, <3>8, <3>8b, <3>8c,
           <3>9, <3>10, SMT
           DEF IngressProtectedSlotCountFor
      <3>14. IngressDepth(recipient)
                + IngressProtectedSlotCountFor(
                    asyncIngressLanes, recipient)
              <= AsyncIngressCapacity
        BY <3>1, <3>13, SMT
      <3> QED BY <3>11, <3>12, <3>14
    <2> QED BY <2>1 DEF AsyncIngressCapacityTypeInvariant
  <1> QED BY <1>1

THEOREM AsyncInitEstablishesIngressContentType ==
  \A initialContext:
    AsyncInitAt(initialContext) => AsyncIngressContentTypeInvariant
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncInitAt(initialContext)
         PROVE AsyncIngressContentTypeInvariant
    <2>1. ASSUME NEW recipient \in ValidatorIds,
                  NEW source \in AsyncIngressSources
           PROVE /\ IngressLane(recipient, source)
                        \in Seq(Range(IngressLane(recipient, source)))
                 /\ DOMAIN IngressLane(recipient, source) =
                        1..IngressLaneDepth(recipient, source)
                 /\ \A index \in
                        1..IngressLaneDepth(recipient, source):
                      /\ AsyncItemTyped(
                           IngressLane(recipient, source)[index])
                      /\ IngressLane(recipient, source)[index]
                           .envelope.recipient = recipient
                      /\ IngressResourceSource(
                           IngressLane(recipient, source)[index]) = source
      <3>1. IngressLane(recipient, source) = <<>>
        BY <1>1, <2>1, SMT
           DEF AsyncInitAt, AsyncBaseInitAt, AsyncIngressInit, IngressLane
      <3>2. DOMAIN IngressLane(recipient, source) =
               1..IngressLaneDepth(recipient, source)
             /\ IngressLane(recipient, source)
                  \in Seq(Range(IngressLane(recipient, source)))
        BY <3>1, EmptyAsyncQueueIsTyped
           DEF AsyncQueueTyped, IngressLaneDepth
      <3>3. \A index \in
                    1..IngressLaneDepth(recipient, source):
                 /\ AsyncItemTyped(IngressLane(recipient, source)[index])
                 /\ IngressLane(recipient, source)[index]
                      .envelope.recipient = recipient
                 /\ IngressResourceSource(
                      IngressLane(recipient, source)[index]) = source
        BY <3>1, Isa DEF IngressLaneDepth
      <3> QED BY <3>2, <3>3
    <2> QED BY <2>1 DEF AsyncIngressContentTypeInvariant
  <1> QED BY <1>1

THEOREM AsyncInitEstablishesIngressType ==
  \A initialContext:
    (AsyncInitAt(initialContext) /\ TypeInvariant)
      => AsyncIngressTypeInvariant
BY AsyncInitEstablishesIngressTopologyType,
   AsyncInitEstablishesIngressCapacityType,
   AsyncInitEstablishesIngressContentType
   DEF AsyncIngressTypeInvariant

THEOREM ModelResponsiveValidators ==
  ModelConfiguration => Responsive \subseteq ValidatorIds
BY SMT DEF ModelConfiguration, QuorumConfiguration

=============================================================================
