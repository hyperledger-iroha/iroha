---- MODULE SumeragiV2ChainEpochRefinement ----
EXTENDS SumeragiV2AsyncTemporalClosureProofs, TLAPS

(***************************************************************************
`VerificationContext` is an arbitrary module constant.  Proving the final
height property for this constant is therefore the ordinary TLA+ universal
closure over every admissible assignment, while keeping the asynchronous
proof INSTANCE nonparameterized for TLAPS.
***************************************************************************)
CONSTANT VerificationContext

CONSTANTS ProductionAppliedSuccessorTraceRefinesIndexedActivation,
          ProductionRecoveredSuccessorTraceRefinesIndexedActivation,
          ProductionStartupFailureAndRestartRefinesIndexedLifecycle,
          ProductionHistoricalCertificateTraceRefinesIndexedAsync,
          ProductionHistoricalBodyPipelineTraceRefinesIndexedAsync,
          ProductionTerminalApplicationWithoutSuccessorActivationTraceRefinesIndexedTerminal

(***************************************************************************
Selected-height synchronous product.

SumeragiV2AsyncNetwork is deliberately one frozen Core instance.  This module
does not manufacture a second chain-progress relation beside that instance.
Instead it adds the canonical ChainEpoch variables and pairs every new Core
decision/application receipt with exactly one ChainEpoch receipt transition.
Every Async step with no new durable receipt stutters the complete ChainEpoch
state.  Thus certification and local application are causally backed by the
exact production receipt delta, while historical service remains available.

The first product below is the selected-height safety refinement.  The indexed
product later in this module pre-creates one dormant authoritative AsyncSpecAt
state per frozen context, joins validators independently after their exact
application receipts, and retains joined old contexts for historical service.
It therefore models successor-height composition without a global apply
barrier, a favourable-network relation, or a second consensus transition.
***************************************************************************)

VARIABLES
  certifiedHeight,
  decidedAt,
  nodeHeight,
  nodeContext,
  durableDecisionEvidence,
  durableApplicationEvidence,
  indexedAsyncState,
  joinedByContext,
  successorActivationStatus,
  successorPredecessorStatusOwnership,
  successorActivationPrerequisites,
  successorActivationTokens,
  successorRecoveryAuthorities,
  preparedSuccessorActivationMarkers,
  publishedSuccessorActivationMarkers,
  successorActivationFailures,
  successorActivationFailureHistory,
  successorActivationCompletions

Chain == INSTANCE SumeragiV2ChainEpochProofs
  WITH certifiedHeight <- certifiedHeight,
       decidedAt <- decidedAt,
       nodeHeight <- nodeHeight,
       nodeContext <- nodeContext,
       durableDecisionEvidence <- durableDecisionEvidence,
       durableApplicationEvidence <- durableApplicationEvidence

AsyncChainVars == <<AsyncAllVars, Chain!ChainEpochVars>>

DecisionReceiptProjection == durableDecisionEvidence = decisions

ApplicationReceiptProjection == durableApplicationEvidence = applied

TotalReceiptProjection ==
  /\ DecisionReceiptProjection
  /\ ApplicationReceiptProjection

(***************************************************************************
An Async transition has one of three exact durable-receipt deltas.  Set-union
equality rules out deletion and extra receipts; the negative membership guard
rules out treating a duplicate persistence attempt as a fresh receipt.
***************************************************************************)
NewDecisionReceipt(decision) ==
  /\ decision \notin decisions
  /\ decisions' = decisions \cup {decision}
  /\ applied' = applied

NewApplicationReceipt(application) ==
  /\ application \notin applied
  /\ applied' = applied \cup {application}
  /\ decisions' = decisions

NoNewDurableReceipt ==
  /\ decisions' = decisions
  /\ applied' = applied

DecisionReceiptHandoff(decision) ==
  /\ NewDecisionReceipt(decision)
  /\ \/ Chain!RecordCertifiedNext(decision)
     \/ Chain!RecordKnownDecision(decision)

ApplicationReceiptHandoff(application) ==
  /\ NewApplicationReceipt(application)
  /\ \/ Chain!RecordAppliedNext(application)
     \/ Chain!RecordKnownApplication(application)

ReceiptFreeChainStutter ==
  /\ NoNewDurableReceipt
  /\ UNCHANGED Chain!ChainEpochVars

(***************************************************************************
The authoritative product step.  AsyncNext supplies the concrete scheduler,
transport, WAL, application, and historical-server behavior.  The other
conjunct classifies its exact durable receipt delta and permits no independent
ChainEpoch progress.
***************************************************************************)
AsyncChainNext ==
  /\ AsyncNext
  /\ \/ ReceiptFreeChainStutter
     \/ \E decision \in Chain!DecisionEvidenceSet:
          DecisionReceiptHandoff(decision)
     \/ \E application \in Chain!DecisionEvidenceSet:
          ApplicationReceiptHandoff(application)

AsyncChainInit ==
  /\ AsyncInit
  /\ Chain!ChainEpochInit

AsyncChainSpec ==
  /\ AsyncChainInit
  /\ [][AsyncChainNext]_AsyncChainVars
  /\ AsyncFairness

(***************************************************************************
The live chain surface adds no finite-counter premise to the authoritative
product.  It does retain the same explicit representative-peer boundary as
`AsyncLiveSpecAt`: safety remains `AsyncChainSpec`, while liveness requires at
least four peers.  The live model uses `Nat` generations; bounded TLC
exhaustion is diagnostic only, while production physical nonexhaustion is
derived from strict same-view Prepare-rank ascent.  The indexed GST condition
introduced below is the environmental premise in this dimension.
***************************************************************************)
AsyncLiveChainSpec ==
  /\ AsyncRepresentativeLiveConfiguration
  /\ AsyncChainSpec

(***************************************************************************
The product projects to both of its components.  The Chain projection is the
formal safety bridge; the Async projection preserves the production scheduler
and all of its historical-service fairness obligations.
***************************************************************************)
THEOREM AsyncChainInitProjectsAsyncInit ==
  AsyncChainInit => AsyncInit
BY DEF AsyncChainInit

THEOREM AsyncChainStepProjectsAsyncStep ==
  [AsyncChainNext]_AsyncChainVars => [AsyncNext]_AsyncAllVars
PROOF
  <1>1. ASSUME [AsyncChainNext]_AsyncChainVars
         PROVE [AsyncNext]_AsyncAllVars
    <2>1. CASE AsyncChainNext
      BY <2>1 DEF AsyncChainNext
    <2>2. CASE UNCHANGED AsyncChainVars
      BY <2>2 DEF AsyncChainVars
    <2> QED BY <1>1, <2>1, <2>2
  <1> QED BY <1>1

THEOREM AsyncChainSpecProjectsAsyncSpec ==
  AsyncChainSpec => AsyncSpec
PROOF
  <1>1. AsyncChainInit => AsyncInit
    BY AsyncChainInitProjectsAsyncInit
  <1>2. [AsyncChainNext]_AsyncChainVars => [AsyncNext]_AsyncAllVars
    BY AsyncChainStepProjectsAsyncStep
  <1> QED BY <1>1, <1>2, PTL DEF AsyncChainSpec, AsyncSpec

THEOREM AsyncLiveChainSpecProjectsGenesisAsyncLiveSpec ==
  AsyncLiveChainSpec
    => AsyncLiveSpecAt(ContextRecord(0, <<>>))
PROOF
  <1>1. AsyncLiveChainSpec => AsyncChainSpec
    BY DEF AsyncLiveChainSpec
  <1>2. AsyncChainSpec => AsyncSpec
    BY AsyncChainSpecProjectsAsyncSpec
  <1>3. AsyncSpec => AsyncSpecAt(ContextRecord(0, <<>>))
    BY DEF AsyncSpec, AsyncSpecAt, AsyncInit, AsyncFairness
  <1>4. AsyncLiveChainSpec => AsyncRepresentativeLiveConfiguration
    BY DEF AsyncLiveChainSpec
  <1> QED BY <1>1, <1>2, <1>3, <1>4
     DEF AsyncLiveSpecAt

THEOREM AsyncChainInitProjectsChainEpochInit ==
  AsyncChainInit => Chain!ChainEpochInit
BY DEF AsyncChainInit

THEOREM AsyncChainActionProjectsChainEpochAction ==
  AsyncChainNext => [Chain!ChainEpochNext]_Chain!ChainEpochVars
PROOF
  <1>1. ASSUME AsyncChainNext
         PROVE [Chain!ChainEpochNext]_Chain!ChainEpochVars
    <2>1. CASE ReceiptFreeChainStutter
      BY <2>1 DEF ReceiptFreeChainStutter
    <2>2. CASE \E decision \in Chain!DecisionEvidenceSet:
                  DecisionReceiptHandoff(decision)
      BY <2>2 DEF DecisionReceiptHandoff, Chain!ChainEpochNext
    <2>3. CASE \E application \in Chain!DecisionEvidenceSet:
                  ApplicationReceiptHandoff(application)
      BY <2>3 DEF ApplicationReceiptHandoff, Chain!ChainEpochNext
    <2> QED BY <1>1, <2>1, <2>2, <2>3 DEF AsyncChainNext
  <1> QED BY <1>1

THEOREM AsyncChainStepProjectsChainEpochStep ==
  [AsyncChainNext]_AsyncChainVars
    => [Chain!ChainEpochNext]_Chain!ChainEpochVars
PROOF
  <1>1. ASSUME [AsyncChainNext]_AsyncChainVars
         PROVE [Chain!ChainEpochNext]_Chain!ChainEpochVars
    <2>1. CASE AsyncChainNext
      BY <2>1, AsyncChainActionProjectsChainEpochAction
    <2>2. CASE UNCHANGED AsyncChainVars
      BY <2>2 DEF AsyncChainVars
    <2> QED BY <1>1, <2>1, <2>2
  <1> QED BY <1>1

THEOREM AsyncChainSpecRefinesChainEpochSpec ==
  AsyncChainSpec => Chain!ChainEpochSpec
PROOF
  <1>1. AsyncChainInit => Chain!ChainEpochInit
    BY AsyncChainInitProjectsChainEpochInit
  <1>2. [AsyncChainNext]_AsyncChainVars
           => [Chain!ChainEpochNext]_Chain!ChainEpochVars
    BY AsyncChainStepProjectsChainEpochStep
  <1> QED BY <1>1, <1>2, PTL
           DEF AsyncChainSpec, Chain!ChainEpochSpec

(***************************************************************************
Exact total-receipt coupling is inductive.  This is stronger than a subset or
existential projection: every durable Core receipt is the corresponding
canonical ChainEpoch evidence entry in the same transition.
***************************************************************************)
THEOREM AsyncChainInitEstablishesReceiptProjection ==
  AsyncChainInit => TotalReceiptProjection
PROOF
  <1>1. ContextRecord(0, <<>>).height = 0
    BY DEF ContextRecord
  <1>2. AsyncInit => /\ decisions = {}
                         /\ applied = {}
    BY <1>1
       DEF AsyncInit, AsyncInitAt, AsyncBaseInitAt, InitAt
  <1>3. Chain!ChainEpochInit
           => /\ durableDecisionEvidence = {}
              /\ durableApplicationEvidence = {}
    BY DEF Chain!ChainEpochInit
  <1> QED BY <1>2, <1>3
       DEF AsyncChainInit, TotalReceiptProjection,
           DecisionReceiptProjection, ApplicationReceiptProjection

THEOREM ReceiptFreeStepPreservesReceiptProjection ==
  TotalReceiptProjection /\ ReceiptFreeChainStutter
    => TotalReceiptProjection'
BY Isa DEF TotalReceiptProjection, DecisionReceiptProjection,
           ApplicationReceiptProjection, ReceiptFreeChainStutter,
           NoNewDurableReceipt, Chain!ChainEpochVars

THEOREM DecisionHandoffPreservesReceiptProjection ==
  \A decision \in Chain!DecisionEvidenceSet:
    TotalReceiptProjection /\ DecisionReceiptHandoff(decision)
      => TotalReceiptProjection'
BY Isa DEF TotalReceiptProjection, DecisionReceiptProjection,
           ApplicationReceiptProjection, DecisionReceiptHandoff,
           NewDecisionReceipt, Chain!RecordCertifiedNext,
           Chain!RecordKnownDecision

THEOREM ApplicationHandoffPreservesReceiptProjection ==
  \A application \in Chain!DecisionEvidenceSet:
    TotalReceiptProjection /\ ApplicationReceiptHandoff(application)
      => TotalReceiptProjection'
BY Isa DEF TotalReceiptProjection, DecisionReceiptProjection,
           ApplicationReceiptProjection, ApplicationReceiptHandoff,
           NewApplicationReceipt, Chain!RecordAppliedNext,
           Chain!RecordKnownApplication

THEOREM AsyncChainActionPreservesReceiptProjection ==
  TotalReceiptProjection /\ AsyncChainNext
    => TotalReceiptProjection'
PROOF
  <1>1. ASSUME TotalReceiptProjection, AsyncChainNext
         PROVE TotalReceiptProjection'
    <2>1. CASE ReceiptFreeChainStutter
      BY <1>1, <2>1, ReceiptFreeStepPreservesReceiptProjection
    <2>2. CASE \E decision \in Chain!DecisionEvidenceSet:
                  DecisionReceiptHandoff(decision)
      BY <1>1, <2>2, DecisionHandoffPreservesReceiptProjection
    <2>3. CASE \E application \in Chain!DecisionEvidenceSet:
                  ApplicationReceiptHandoff(application)
      BY <1>1, <2>3, ApplicationHandoffPreservesReceiptProjection
    <2> QED BY <1>1, <2>1, <2>2, <2>3 DEF AsyncChainNext
  <1> QED BY <1>1

THEOREM AsyncChainStepPreservesReceiptProjection ==
  TotalReceiptProjection /\ [AsyncChainNext]_AsyncChainVars
    => TotalReceiptProjection'
PROOF
  <1>1. ASSUME TotalReceiptProjection,
              [AsyncChainNext]_AsyncChainVars
         PROVE TotalReceiptProjection'
    <2>1. CASE AsyncChainNext
      BY <1>1, <2>1, AsyncChainActionPreservesReceiptProjection
    <2>2. CASE UNCHANGED AsyncChainVars
      BY <1>1, <2>2, Isa
         DEF AsyncChainVars, AsyncAllVars, vars, Chain!ChainEpochVars,
             TotalReceiptProjection, DecisionReceiptProjection,
             ApplicationReceiptProjection
    <2> QED BY <1>1, <2>1, <2>2
  <1> QED BY <1>1

THEOREM TotalConcreteDurableReceiptRefinement ==
  AsyncChainSpec => []TotalReceiptProjection
PROOF
  <1>1. AsyncChainInit => TotalReceiptProjection
    BY AsyncChainInitEstablishesReceiptProjection
  <1>2. TotalReceiptProjection /\ [AsyncChainNext]_AsyncChainVars
           => TotalReceiptProjection'
    BY AsyncChainStepPreservesReceiptProjection
  <1> QED BY <1>1, <1>2, PTL DEF AsyncChainSpec

(***************************************************************************
Release-level selected-height safety consequences.
***************************************************************************)
THEOREM AsyncChainPrefixAndEpochSafety ==
  AsyncChainSpec => []Chain!ChainEpochSafety
PROOF
  <1>1. AsyncChainSpec => Chain!ChainEpochSpec
    BY AsyncChainSpecRefinesChainEpochSpec
  <1>2. Chain!ChainEpochSpec => []Chain!ChainEpochSafety
    BY Chain!ChainPrefixAndEpochSafety
  <1> QED BY <1>1, <1>2, PTL

THEOREM AsyncHistoriesArePrefixComparable ==
  AsyncChainSpec
    => [](/\ Chain!HistoryPrefixComparable
          /\ Chain!NodeAppliedPrefixBacked)
PROOF
  <1>1. Chain!ChainEpochSafety
           => /\ Chain!HistoryPrefixComparable
              /\ Chain!NodeAppliedPrefixBacked
    BY DEF Chain!ChainEpochSafety
  <1> QED BY <1>1, AsyncChainPrefixAndEpochSafety, PTL

THEOREM AsyncEpochRoutingIsFrozen ==
  AsyncChainSpec
    => [](/\ Chain!PerNodeFrozenEpoch
          /\ Chain!PerNodeParentFinality
          /\ Chain!ForeignLineageRejected
          /\ Chain!ForeignContextCertificateRejected)
PROOF
  <1>1. Chain!ChainEpochSafety
           => /\ Chain!PerNodeFrozenEpoch
              /\ Chain!PerNodeParentFinality
              /\ Chain!ForeignLineageRejected
              /\ Chain!ForeignContextCertificateRejected
    BY DEF Chain!ChainEpochSafety
  <1> QED BY <1>1, AsyncChainPrefixAndEpochSafety, PTL

(***************************************************************************
The missing multi-height seam is intentionally observable.  A node crosses
it exactly when its authoritative local context has advanced beyond the one
frozen Core context served by this product.  Historical service continues in
the old instance, but progress now requires a successor AsyncSpecAt instance.
***************************************************************************)
NeedsSuccessorAsyncInstance(node) ==
  /\ node \in ValidatorIds
  /\ nodeHeight[node] > context.height
  /\ nodeContext[node]
       = Chain!ContextRecord(nodeHeight[node],
                             Chain!HistoryThrough(nodeHeight[node]))

GenesisApplicationAdvanceInvariant ==
  ContextRecord(0, <<>>).height < MaxHeight
    => \A node \in AsyncCurrentResponsiveVoters:
         NodeHasApplication(node)
           => /\ node \in ValidatorIds
              /\ nodeHeight[node] > context.height

GenesisApplicationHeightInvariant ==
  /\ context = ContextRecord(0, <<>>)
  /\ GenesisApplicationAdvanceInvariant

GenesisApplicationHandoffInvariant ==
  /\ context = ContextRecord(0, <<>>)
  /\ (ContextRecord(0, <<>>).height < MaxHeight
        => \A node \in AsyncCurrentResponsiveVoters:
             NodeHasApplication(node) => NeedsSuccessorAsyncInstance(node))

SuccessorInstanceSeam ==
  \E node \in Honest: NeedsSuccessorAsyncInstance(node)

(***************************************************************************
The concrete AsyncChainSpec begins at genesis and, when a successor height
exists, can carry each responsive validator across that instance's application
boundary into its exact first successor context.  This formula records only
that genesis handoff; it does not start the successor AsyncSpecAt instance or
claim indexed height progress.  At the finite terminal horizon there is no
successor-instance obligation.

The theorem below discharges this narrower genesis seam.  The separate
HeightLivenessObligation targets the indexed composition; this genesis theorem
is not used as a substitute for that multi-height induction.
***************************************************************************)
GenesisHeightSuccessorHandoffProperty ==
  ContextRecord(0, <<>>).height < MaxHeight
    => \A node \in AsyncCurrentResponsiveVoters:
         gst ~> NeedsSuccessorAsyncInstance(node)

THEOREM GenesisTerminalHorizonHasNoSuccessorObligation ==
  ContextRecord(0, <<>>).height = MaxHeight
    => GenesisHeightSuccessorHandoffProperty
BY SMT DEF GenesisHeightSuccessorHandoffProperty

THEOREM AsyncChainInitEstablishesGenesisApplicationHeight ==
  AsyncChainInit => GenesisApplicationHeightInvariant
PROOF
  <1>1. AsyncChainInit => context = ContextRecord(0, <<>>)
    BY DEF AsyncChainInit, AsyncInit, AsyncInitAt,
           AsyncBaseInitAt, InitAt
  <1>2. AsyncChainInit => applied = {}
    BY DEF AsyncChainInit, AsyncInit, AsyncInitAt,
           AsyncBaseInitAt, InitAt, ContextRecord
  <1> QED BY <1>1, <1>2, SMT
       DEF GenesisApplicationHeightInvariant,
           GenesisApplicationAdvanceInvariant, NodeHasApplication

THEOREM AsyncChainStepKeepsFrozenContext ==
  context = ContextRecord(0, <<>>)
    /\ [AsyncChainNext]_AsyncChainVars
    => context' = ContextRecord(0, <<>>)
BY Isa DEF AsyncChainNext, AsyncNext, AsyncChainVars,
           AsyncAllVars, vars

THEOREM UnchangedApplicationEvidenceProjectsUnchangedApplications ==
  TotalReceiptProjection
    /\ TotalReceiptProjection'
    /\ durableApplicationEvidence' = durableApplicationEvidence
    => applied' = applied
BY SMT DEF TotalReceiptProjection, ApplicationReceiptProjection

THEOREM AppendedApplicationEvidenceProjectsAppendedApplication ==
  \A application:
    TotalReceiptProjection
      /\ TotalReceiptProjection'
      /\ durableApplicationEvidence' =
           durableApplicationEvidence \cup {application}
      => applied' = applied \cup {application}
BY SMT DEF TotalReceiptProjection, ApplicationReceiptProjection

THEOREM AppendedApplicationCanOnlyAddItsNode ==
  \A application, node:
    (/\ context' = context
     /\ applied' = applied \cup {application}
     /\ NodeHasApplication(node)')
      => \/ NodeHasApplication(node)
         \/ application.node = node
BY Isa DEF NodeHasApplication

THEOREM NewlyAppendedNodeApplicationCarriesExactContext ==
  \A application, node:
    (/\ context' = context
     /\ applied' = applied \cup {application}
     /\ NodeHasApplication(node)'
     /\ ~NodeHasApplication(node))
      => /\ application.node = node
         /\ application.qc.context = context
         /\ application.qc.phase = "Commit"
BY Isa DEF NodeHasApplication

(***************************************************************************
The Chain instance carries the model and state typing needed by the genesis
handoff proof.  Keeping these projections explicit prevents INSTANCE-local
operators from being expanded differently in each application branch.
***************************************************************************)
THEOREM ChainInvariantProvidesGenesisModelTyping ==
  Chain!ChainEpochInvariant
    => /\ Responsive \subseteq Honest
       /\ Honest \subseteq ValidatorIds
       /\ nodeHeight \in [ValidatorIds -> Heights]
       /\ MaxHeight \in Nat
BY SMT DEF Chain!ChainEpochInvariant,
           Chain!ChainEpochTypeInvariant,
           Chain!ModelConfiguration,
           Chain!QuorumConfiguration,
           Chain!ValidatorIds, ValidatorIds,
           Chain!Heights, Heights

THEOREM ChainInvariantTypesGenesisResponsiveVoters ==
  Chain!ChainEpochInvariant
    => AsyncCurrentResponsiveVoters \subseteq ValidatorIds
BY Isa, ChainInvariantProvidesGenesisModelTyping
   DEF AsyncCurrentResponsiveVoters

THEOREM ChainInvariantTypesGenesisResponsiveVotersAsHonest ==
  Chain!ChainEpochInvariant
    => AsyncCurrentResponsiveVoters \subseteq Honest
BY Isa, ChainInvariantProvidesGenesisModelTyping
   DEF AsyncCurrentResponsiveVoters

THEOREM ChainInvariantProvidesNodeHeightDomain ==
  Chain!ChainEpochInvariant
    => DOMAIN nodeHeight = ValidatorIds
BY Isa, ChainInvariantProvidesGenesisModelTyping

THEOREM ChainInvariantTypesNodeHeightsAsNaturals ==
  Chain!ChainEpochInvariant
    => \A node \in ValidatorIds: nodeHeight[node] \in Nat
BY SMT, ChainInvariantProvidesGenesisModelTyping DEF Heights

THEOREM ChainEpochStepPreservesGenesisApplicationAdvance ==
  GenesisApplicationAdvanceInvariant
    /\ context = ContextRecord(0, <<>>)
    /\ context' = context
    /\ Chain!ChainEpochInvariant
    /\ TotalReceiptProjection
    /\ TotalReceiptProjection'
    /\ [Chain!ChainEpochNext]_Chain!ChainEpochVars
    => GenesisApplicationAdvanceInvariant'
PROOF
  <1>1. ASSUME GenesisApplicationAdvanceInvariant,
              context = ContextRecord(0, <<>>),
              context' = context,
              Chain!ChainEpochInvariant,
              TotalReceiptProjection,
              TotalReceiptProjection',
              [Chain!ChainEpochNext]_Chain!ChainEpochVars
         PROVE GenesisApplicationAdvanceInvariant'
    <2>1. CASE UNCHANGED Chain!ChainEpochVars
      <3>1. /\ nodeHeight' = nodeHeight
             /\ nodeContext' = nodeContext
             /\ durableApplicationEvidence' =
                  durableApplicationEvidence
        BY <2>1, Isa DEF Chain!ChainEpochVars
      <3>2. applied' = applied
        BY <1>1, <3>1,
           UnchangedApplicationEvidenceProjectsUnchangedApplications
      <3> QED BY <1>1, <3>1, <3>2, Isa
         DEF GenesisApplicationAdvanceInvariant,
             NodeHasApplication, AsyncCurrentResponsiveVoters,
             CurrentVoters, CurrentEpoch
    <2>2. CASE \E decision \in Chain!DecisionEvidenceSet:
                  Chain!RecordCertifiedNext(decision)
      <3>1. /\ nodeHeight' = nodeHeight
             /\ nodeContext' = nodeContext
             /\ durableApplicationEvidence' =
                  durableApplicationEvidence
        BY <2>2, Isa DEF Chain!RecordCertifiedNext
      <3>2. applied' = applied
        BY <1>1, <3>1,
           UnchangedApplicationEvidenceProjectsUnchangedApplications
      <3> QED BY <1>1, <3>1, <3>2, Isa
         DEF GenesisApplicationAdvanceInvariant,
             NodeHasApplication, AsyncCurrentResponsiveVoters,
             CurrentVoters, CurrentEpoch
    <2>3. CASE \E decision \in Chain!DecisionEvidenceSet:
                  Chain!RecordKnownDecision(decision)
      <3>1. /\ nodeHeight' = nodeHeight
             /\ nodeContext' = nodeContext
             /\ durableApplicationEvidence' =
                  durableApplicationEvidence
        BY <2>3, Isa DEF Chain!RecordKnownDecision
      <3>2. applied' = applied
        BY <1>1, <3>1,
           UnchangedApplicationEvidenceProjectsUnchangedApplications
      <3> QED BY <1>1, <3>1, <3>2, Isa
         DEF GenesisApplicationAdvanceInvariant,
             NodeHasApplication, AsyncCurrentResponsiveVoters,
             CurrentVoters, CurrentEpoch
    <2>4. CASE \E application \in Chain!DecisionEvidenceSet:
                  Chain!RecordAppliedNext(application)
      <3>1. PICK application \in Chain!DecisionEvidenceSet:
               Chain!RecordAppliedNext(application)
        BY <2>4
      <3>2. durableApplicationEvidence' =
               durableApplicationEvidence \cup {application}
        BY <3>1 DEF Chain!RecordAppliedNext
      <3>3. applied' = applied \cup {application}
        BY <1>1, <3>2,
           AppendedApplicationEvidenceProjectsAppendedApplication
      <3>4. CASE ~(ContextRecord(0, <<>>).height < MaxHeight)
        BY <3>4 DEF GenesisApplicationAdvanceInvariant
      <3>5. CASE ContextRecord(0, <<>>).height < MaxHeight
        <4>1. AsyncCurrentResponsiveVoters' =
                 AsyncCurrentResponsiveVoters
          BY <1>1, Isa
             DEF AsyncCurrentResponsiveVoters,
                 CurrentVoters, CurrentEpoch
        <4>2. ASSUME NEW node \in AsyncCurrentResponsiveVoters',
                    NodeHasApplication(node)'
               PROVE /\ node \in ValidatorIds
                     /\ nodeHeight'[node] > context'.height
          <5>1. node \in AsyncCurrentResponsiveVoters
            BY <4>1, <4>2
          <5>2. node \in ValidatorIds
            BY <1>1, <5>1,
               ChainInvariantTypesGenesisResponsiveVoters
          <5>3. node \in AsyncCurrentResponsiveVoters
            BY <4>1, <4>2
          <5>4. CASE node = application.node
            <6>1. nodeHeight[node] \in Nat
              BY <1>1, <5>2,
                 ChainInvariantTypesNodeHeightsAsNaturals
            <6>2. node \in DOMAIN nodeHeight
              BY <1>1, <5>2, Isa,
                 ChainInvariantProvidesNodeHeightDomain
            <6>3. nodeHeight' =
                     [nodeHeight EXCEPT
                        ![application.node] =
                          nodeHeight[application.node] + 1]
              BY <3>1 DEF Chain!RecordAppliedNext
            <6>4. nodeHeight'[application.node] =
                     nodeHeight[application.node] + 1
              BY <5>4, <6>2, <6>3,
                 Chain!FunctionalUpdateAtKey
            <6>5. nodeHeight'[node] = nodeHeight[node] + 1
              BY <5>4, <6>4
            <6>6. context'.height = 0
              BY <1>1 DEF ContextRecord
            <6> QED BY <5>2, <6>1, <6>5, <6>6, SMT
          <5>5. CASE node # application.node
            <6>1. NodeHasApplication(node)
              BY <1>1, <3>3, <4>2, <5>5,
                 AppendedApplicationCanOnlyAddItsNode
            <6>2. /\ node \in ValidatorIds
                   /\ nodeHeight[node] > context.height
              BY <1>1, <3>5, <5>3, <6>1
                 DEF GenesisApplicationAdvanceInvariant
            <6>3. node \in DOMAIN nodeHeight
              BY <1>1, <6>2, Isa,
                 ChainInvariantProvidesNodeHeightDomain
            <6>4. nodeHeight' =
                     [nodeHeight EXCEPT
                        ![application.node] =
                          nodeHeight[application.node] + 1]
              BY <3>1 DEF Chain!RecordAppliedNext
            <6>5. nodeHeight'[node] = nodeHeight[node]
              BY <5>5, <6>3, <6>4,
                 Chain!FunctionalUpdateAwayFromKey
            <6> QED BY <1>1, <5>2, <6>2, <6>5, SMT
          <5> QED BY <5>4, <5>5
        <4> QED BY <4>2 DEF GenesisApplicationAdvanceInvariant
      <3> QED BY <3>4, <3>5
    <2>5. CASE \E application \in Chain!DecisionEvidenceSet:
                  Chain!RecordKnownApplication(application)
      <3>1. PICK application \in Chain!DecisionEvidenceSet:
               Chain!RecordKnownApplication(application)
        BY <2>5
      <3>2. durableApplicationEvidence' =
               durableApplicationEvidence \cup {application}
        BY <3>1 DEF Chain!RecordKnownApplication
      <3>3. applied' = applied \cup {application}
        BY <1>1, <3>2,
           AppendedApplicationEvidenceProjectsAppendedApplication
      <3>4. CASE ~(ContextRecord(0, <<>>).height < MaxHeight)
        BY <3>4 DEF GenesisApplicationAdvanceInvariant
      <3>5. CASE ContextRecord(0, <<>>).height < MaxHeight
        <4>1. AsyncCurrentResponsiveVoters' =
                 AsyncCurrentResponsiveVoters
          BY <1>1, Isa
             DEF AsyncCurrentResponsiveVoters,
                 CurrentVoters, CurrentEpoch
        <4>2. ASSUME NEW node \in AsyncCurrentResponsiveVoters',
                    NodeHasApplication(node)'
               PROVE /\ node \in ValidatorIds
                     /\ nodeHeight'[node] > context'.height
          <5>1. node \in AsyncCurrentResponsiveVoters
            BY <4>1, <4>2
          <5>2. /\ node \in ValidatorIds
                 /\ node \in Honest
            BY <1>1, <5>1,
               ChainInvariantTypesGenesisResponsiveVoters,
               ChainInvariantTypesGenesisResponsiveVotersAsHonest
          <5>3. node \in AsyncCurrentResponsiveVoters
            BY <4>1, <4>2
          <5>4. nodeHeight'[node] = nodeHeight[node]
            BY <3>1 DEF Chain!RecordKnownApplication
          <5>5. CASE NodeHasApplication(node)
            <6>1. /\ node \in ValidatorIds
                   /\ nodeHeight[node] > context.height
              BY <1>1, <3>5, <5>3, <5>5
                 DEF GenesisApplicationAdvanceInvariant
            <6> QED BY <1>1, <5>2, <5>4, <6>1, SMT
          <5>6. CASE ~NodeHasApplication(node)
            <6>1. /\ application.node = node
                   /\ application.qc.context = context
                   /\ application.qc.phase = "Commit"
              BY <1>1, <3>3, <4>2, <5>6,
                 NewlyAppendedNodeApplicationCarriesExactContext
            <6>2. \/ Chain!ReceiptOutsideChainHorizon(application)
                   \/ application.node \notin Honest
                   \/ application.qc.context.height + 1
                        <= nodeHeight[application.node]
              BY <3>1 DEF Chain!RecordKnownApplication
            <6>3. MaxHeight \in Nat
              BY <1>1, ChainInvariantProvidesGenesisModelTyping
            <6>4. ContextRecord(0, <<>>).height = 0
              BY DEF ContextRecord
            <6>5. application.qc.context.height = 0
              <7>1. application.qc.context = ContextRecord(0, <<>>)
                BY <1>1, <6>1
              <7> QED BY <7>1 DEF ContextRecord
            <6>6. ~Chain!ReceiptOutsideChainHorizon(application)
              BY <3>5, <6>3, <6>4, <6>5, SMT
                 DEF Chain!ReceiptOutsideChainHorizon
            <6>7. application.node \in Honest
              BY <5>2, <6>1
            <6>8. application.qc.context.height + 1
                     <= nodeHeight[application.node]
              BY <6>2, <6>6, <6>7
            <6>9. application.qc.context.height = context.height
              BY <6>1
            <6>10. nodeHeight[application.node] = nodeHeight[node]
              BY <6>1
            <6>11. nodeHeight[node] > context.height
              <7>1. context.height + 1 <= nodeHeight[node]
                BY <6>8, <6>9, <6>10
              <7>2. context.height = 0
                BY <1>1 DEF ContextRecord
              <7>3. nodeHeight[node] \in Nat
                BY <1>1, <5>2,
                   ChainInvariantTypesNodeHeightsAsNaturals
              <7> QED BY <7>1, <7>2, <7>3, SMT
            <6> QED BY <1>1, <5>2, <5>4, <6>11, SMT
          <5> QED BY <5>5, <5>6
        <4> QED BY <4>2 DEF GenesisApplicationAdvanceInvariant
      <3> QED BY <3>4, <3>5
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>5
         DEF Chain!ChainEpochNext
  <1> QED BY <1>1

THEOREM AsyncChainStepPreservesGenesisApplicationHeight ==
  GenesisApplicationHeightInvariant
    /\ Chain!ChainEpochInvariant
    /\ TotalReceiptProjection
    /\ [AsyncChainNext]_AsyncChainVars
    => GenesisApplicationHeightInvariant'
PROOF
  <1>1. ASSUME GenesisApplicationHeightInvariant,
              Chain!ChainEpochInvariant,
              TotalReceiptProjection,
              [AsyncChainNext]_AsyncChainVars
         PROVE GenesisApplicationHeightInvariant'
    <2>1. context' = ContextRecord(0, <<>>)
      BY <1>1, AsyncChainStepKeepsFrozenContext
         DEF GenesisApplicationHeightInvariant
    <2>2. TotalReceiptProjection'
      BY <1>1, AsyncChainStepPreservesReceiptProjection
         DEF GenesisApplicationHeightInvariant
    <2>3. [Chain!ChainEpochNext]_Chain!ChainEpochVars
      BY <1>1, AsyncChainStepProjectsChainEpochStep
         DEF GenesisApplicationHeightInvariant
    <2>4. GenesisApplicationAdvanceInvariant'
      BY <1>1, <2>1, <2>2, <2>3,
         ChainEpochStepPreservesGenesisApplicationAdvance
         DEF GenesisApplicationHeightInvariant
    <2> QED BY <2>1, <2>4
         DEF GenesisApplicationHeightInvariant
  <1> QED BY <1>1

THEOREM GenesisApplicationHeightAndChainContextImplyHandoff ==
  (/\ GenesisApplicationHeightInvariant
   /\ Chain!ChainEpochInvariant)
    => GenesisApplicationHandoffInvariant
PROOF
  <1>1. ASSUME GenesisApplicationHeightInvariant,
              Chain!ChainEpochInvariant
         PROVE GenesisApplicationHandoffInvariant
    <2>1. /\ context = ContextRecord(0, <<>>)
           /\ GenesisApplicationAdvanceInvariant
      BY <1>1 DEF GenesisApplicationHeightInvariant
    <2>2. Chain!ContextsMatchLocalHistories
      BY <1>1 DEF Chain!ChainEpochInvariant
    <2>3. ASSUME ContextRecord(0, <<>>).height < MaxHeight
           PROVE \A node \in AsyncCurrentResponsiveVoters:
                   NodeHasApplication(node)
                     => NeedsSuccessorAsyncInstance(node)
      <3>1. ASSUME NEW node \in AsyncCurrentResponsiveVoters,
                    NodeHasApplication(node)
             PROVE NeedsSuccessorAsyncInstance(node)
        <4>1. /\ node \in ValidatorIds
               /\ nodeHeight[node] > context.height
          BY <2>1, <2>3, <3>1
             DEF GenesisApplicationAdvanceInvariant
        <4>2. node \in Chain!ValidatorIds
          BY <4>1 DEF Chain!ValidatorIds, ValidatorIds
        <4>3. nodeContext[node]
                 = Chain!ContextRecord(
                     nodeHeight[node],
                     Chain!HistoryThrough(nodeHeight[node]))
          BY <2>2, <4>2, Isa DEF Chain!ContextsMatchLocalHistories
        <4> QED BY <4>1, <4>3 DEF NeedsSuccessorAsyncInstance
      <3> QED BY <3>1
    <2> QED BY <2>1, <2>3 DEF GenesisApplicationHandoffInvariant
  <1> QED BY <1>1

THEOREM AsyncChainAlwaysGenesisApplicationHeight ==
  AsyncChainSpec => []GenesisApplicationHeightInvariant
PROOF
  <1>1. AsyncChainInit => GenesisApplicationHeightInvariant
    BY AsyncChainInitEstablishesGenesisApplicationHeight
  <1>2. AsyncChainSpec => []Chain!ChainEpochInvariant
    BY AsyncChainPrefixAndEpochSafety, PTL DEF Chain!ChainEpochSafety
  <1>3. AsyncChainSpec => []TotalReceiptProjection
    BY TotalConcreteDurableReceiptRefinement
  <1>4. GenesisApplicationHeightInvariant
           /\ Chain!ChainEpochInvariant
           /\ TotalReceiptProjection
           /\ [AsyncChainNext]_AsyncChainVars
           => GenesisApplicationHeightInvariant'
    BY AsyncChainStepPreservesGenesisApplicationHeight
  <1> QED BY <1>1, <1>2, <1>3, <1>4, PTL DEF AsyncChainSpec

THEOREM AsyncChainAlwaysGenesisApplicationHandoff ==
  AsyncChainSpec => []GenesisApplicationHandoffInvariant
PROOF
  <1>1. AsyncChainSpec => []GenesisApplicationHeightInvariant
    BY AsyncChainAlwaysGenesisApplicationHeight
  <1>2. AsyncChainSpec => []Chain!ChainEpochInvariant
    BY AsyncChainPrefixAndEpochSafety, PTL DEF Chain!ChainEpochSafety
  <1>3. GenesisApplicationHeightInvariant
           /\ Chain!ChainEpochInvariant
           => GenesisApplicationHandoffInvariant
    BY GenesisApplicationHeightAndChainContextImplyHandoff
  <1> QED BY <1>1, <1>2, <1>3, PTL

THEOREM AlwaysAsyncAllResponsiveAppliedIncludesVoter ==
  \A initialContext: \A node \in AsyncVotersAt(initialContext):
    [](AsyncAllResponsiveAppliedAt(initialContext) => NodeHasApplication(node))
PROOF
  <1>1. ASSUME NEW initialContext, NEW node \in AsyncVotersAt(initialContext)
         PROVE [](AsyncAllResponsiveAppliedAt(initialContext) => NodeHasApplication(node))
    <2>1. AsyncAllResponsiveAppliedAt(initialContext) => NodeHasApplication(node)
      BY <1>1 DEF AsyncAllResponsiveAppliedAt
    <2> QED BY <2>1, PTL
  <1> QED BY <1>1
THEOREM AlwaysGenesisAllResponsiveAppliedIncludesVoter ==
  \A node \in AsyncVotersAt(ContextRecord(0, <<>>)):
    [](AsyncAllResponsiveAppliedAt(ContextRecord(0, <<>>)) => NodeHasApplication(node))
PROOF
  <1>1. ASSUME NEW node \in AsyncVotersAt(ContextRecord(0, <<>>))
         PROVE [](AsyncAllResponsiveAppliedAt(ContextRecord(0, <<>>)) => NodeHasApplication(node))
    <2>1. AsyncAllResponsiveAppliedAt(ContextRecord(0, <<>>)) => NodeHasApplication(node)
      BY <1>1 DEF AsyncAllResponsiveAppliedAt
    <2> QED BY <2>1, PTL
  <1> QED BY <1>1
THEOREM AlwaysGenesisContextPreservesResponsiveVoter ==
  \A node \in Responsive \cap VotingRoster(ContextRecord(0, <<>>).epoch):
    [](context = ContextRecord(0, <<>>) => node \in AsyncCurrentResponsiveVoters)
PROOF
  <1>1. ASSUME NEW node \in Responsive \cap VotingRoster(ContextRecord(0, <<>>).epoch)
         PROVE [](context = ContextRecord(0, <<>>) => node \in AsyncCurrentResponsiveVoters)
    <2>1. context = ContextRecord(0, <<>>) => node \in AsyncCurrentResponsiveVoters
      BY <1>1, SMT DEF AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch
    <2> QED BY <2>1, PTL
  <1> QED BY <1>1
THEOREM AlwaysGenesisHandoffIncludesCurrentVoter ==
  \A node:
    []((/\ GenesisApplicationHandoffInvariant
         /\ ContextRecord(0, <<>>).height < MaxHeight
         /\ node \in AsyncCurrentResponsiveVoters)
          => (NodeHasApplication(node) => NeedsSuccessorAsyncInstance(node)))
PROOF
  <1>1. ASSUME NEW node
         PROVE []((/\ GenesisApplicationHandoffInvariant
                    /\ ContextRecord(0, <<>>).height < MaxHeight
                    /\ node \in AsyncCurrentResponsiveVoters)
                     => (NodeHasApplication(node) => NeedsSuccessorAsyncInstance(node)))
    <2>1. (/\ GenesisApplicationHandoffInvariant
           /\ ContextRecord(0, <<>>).height < MaxHeight
           /\ node \in AsyncCurrentResponsiveVoters)
            => (NodeHasApplication(node) => NeedsSuccessorAsyncInstance(node))
      BY SMT DEF GenesisApplicationHandoffInvariant
    <2> QED BY <2>1, PTL
  <1> QED BY <1>1
THEOREM AlwaysGenesisNonterminal ==
  ContextRecord(0, <<>>).height < MaxHeight => [](ContextRecord(0, <<>>).height < MaxHeight)
PROOF
  <1>1. ASSUME ContextRecord(0, <<>>).height < MaxHeight
         PROVE [](ContextRecord(0, <<>>).height < MaxHeight)
    <2> QED BY <1>1, PTL
  <1> QED BY <1>1
THEOREM GenesisHeightSuccessorHandoffFromOneHeightCompletion ==
  /\ AsyncLiveChainSpec
  /\ OneHeightCompletionLiveness(ContextRecord(0, <<>>))
  => GenesisHeightSuccessorHandoffProperty
PROOF
  <1>1. ASSUME AsyncLiveChainSpec,
              OneHeightCompletionLiveness(ContextRecord(0, <<>>))
         PROVE GenesisHeightSuccessorHandoffProperty
    <2>1. AsyncChainSpec
      BY <1>1 DEF AsyncLiveChainSpec
    <2>2. AsyncLiveSpecAt(ContextRecord(0, <<>>))
      BY <1>1, AsyncLiveChainSpecProjectsGenesisAsyncLiveSpec
    <2>3. gst ~> AsyncAllResponsiveAppliedAt(
                  ContextRecord(0, <<>>))
      BY <1>1, <2>2 DEF OneHeightCompletionLiveness
    <2>4. []GenesisApplicationHandoffInvariant
      BY <2>1, AsyncChainAlwaysGenesisApplicationHandoff
    <2>5. CurrentEpoch = ContextRecord(0, <<>>).epoch
      BY <2>1
         DEF AsyncChainSpec, AsyncChainInit, AsyncInit,
             AsyncInitAt, AsyncBaseInitAt, InitAt,
             ContextRecord, CurrentEpoch
    <2>6. ASSUME ContextRecord(0, <<>>).height < MaxHeight,
                  NEW node \in AsyncCurrentResponsiveVoters
           PROVE gst ~> NeedsSuccessorAsyncInstance(node)
      <3>1. node \in AsyncVotersAt(ContextRecord(0, <<>>))
        BY <2>5, <2>6
           DEF AsyncCurrentResponsiveVoters, AsyncVotersAt,
               CurrentVoters
      <3>2. gst ~> NodeHasApplication(node)
        <4>1. [](AsyncAllResponsiveAppliedAt(
                 ContextRecord(0, <<>>))
                   => NodeHasApplication(node))
          BY <3>1, AlwaysGenesisAllResponsiveAppliedIncludesVoter
        <4> QED BY <2>3, <4>1, PTL
      <3>3. node \in Responsive
                    \cap VotingRoster(
                         ContextRecord(0, <<>>).epoch)
        BY <2>5, <2>6
           DEF AsyncCurrentResponsiveVoters,
               CurrentVoters, CurrentEpoch
      <3>4. [](context = ContextRecord(0, <<>>))
        BY <2>4, PTL DEF GenesisApplicationHandoffInvariant
      <3>5. [](context = ContextRecord(0, <<>>)
                  => node \in AsyncCurrentResponsiveVoters)
        BY <3>3, AlwaysGenesisContextPreservesResponsiveVoter
      <3>6. [](node \in AsyncCurrentResponsiveVoters)
        BY <3>4, <3>5, PTL
      <3>7. []((/\ GenesisApplicationHandoffInvariant
                 /\ ContextRecord(0, <<>>).height < MaxHeight
                 /\ node \in AsyncCurrentResponsiveVoters)
                  => (NodeHasApplication(node)
                        => NeedsSuccessorAsyncInstance(node)))
        BY AlwaysGenesisHandoffIncludesCurrentVoter
      <3>8. [](ContextRecord(0, <<>>).height < MaxHeight)
        BY <2>6, AlwaysGenesisNonterminal
      <3>9. [](NodeHasApplication(node)
                  => NeedsSuccessorAsyncInstance(node))
        BY <2>4, <3>6, <3>7, <3>8, PTL
      <3> QED BY <3>2, <3>9, PTL
    <2> QED BY <2>6 DEF GenesisHeightSuccessorHandoffProperty
  <1> QED BY <1>1

(***************************************************************************
The composition kernel above is independent of the asynchronous liveness
debts.  This release-facing wrapper consumes only the exact temporal closure:
rotating-leader convergence and exact Decision-stage application service are
proved before one-height completion is projected into the genesis product.
***************************************************************************)
THEOREM GenesisHeightSuccessorHandoffObligation ==
  AsyncLiveChainSpec => GenesisHeightSuccessorHandoffProperty
PROOF
  <1>1. OneHeightCompletionLiveness(ContextRecord(0, <<>>))
    BY AsyncTemporalClosureOneHeightCompletionObligation
  <1> QED BY <1>1, GenesisHeightSuccessorHandoffFromOneHeightCompletion


(***************************************************************************
Authoritative indexed successor-instance product.

Every admissible frozen ContextRecord owns one pre-created, dormant copy of the
complete AsyncAllVars tuple. IndexedAsync is an actual parameterized instance
of the production SumeragiV2AsyncNetwork vocabulary; there is no shadow
consensus relation. One-height proof facts are stated separately over that
exact instance rather than cited as hidden parameterized instance theorems.
ContextRecords with an invalid lineage are outside this domain because
AsyncInitAt rejects them as well.
A context becomes live when its first validator joins after an exact durable
application receipt. Validators join independently, and RunNode is gated only
by that validator's current nodeContext. Thus an early validator may execute
without waiting for its peers. Joined membership is monotone, so old instances
remain available to RunHistoricalServer after validators advance.

The nested tuple layout is exactly the production `AsyncAllVars` projection:
the duplicated GST scalar, 49 Core components, 46 scheduler/transport
components, five responsive-node recovery components, three monotone producer-
journal components, and the proof-only fixed-corridor receipt set.  The
duplicated scalar is pinned to Core component
7; retaining that established semantic shape makes source drift explicit
rather than silently normalizing the authoritative action tuple.  Immediately
after the I/O queues, the scheduler tuple carries
the two immutable Serve ordinal high-watermarks and the ingress-admission,
admission, reservation, tombstone, and retained-attempt stores.  The
certified-response claim precedes the
transport state; scheduler component 42 owns the receiver-local leader-wire
lifecycle table, component 45 owns the fixed roster/class-bounded control-
service slot table, and component 46 owns the internal irreversible service-
activation record.  The final recovery component owns the exact historical-
lock restart-authority projection. Shape predicates exclude unmodelled fields
and make every instance projection extensional.
***************************************************************************)
IndexedDuplicatedGst(initialContext) ==
  indexedAsyncState[initialContext][1]

IndexedCore(initialContext, component) ==
  indexedAsyncState[initialContext][2][component]

IndexedScheduler(initialContext, component) ==
  indexedAsyncState[initialContext][3][component]

IndexedRecovery(initialContext, component) ==
  indexedAsyncState[initialContext][4][component]

IndexedProducer(initialContext, component) ==
  indexedAsyncState[initialContext][5][component]

IndexedFixedCorridorDeadlines(initialContext) ==
  indexedAsyncState[initialContext][6]

IndexedAsyncStateAt(initialContext) ==
  indexedAsyncState[initialContext]

HistoricalRecoveryNodeHasApplicationProjection(applicationEvidence,
                                               applicationContext, node) ==
  \E application \in applicationEvidence:
    /\ application.node = node
    /\ application.qc.context = applicationContext
    /\ application.qc.phase = "Commit"

THEOREM HistoricalRecoveryVotersProjectionMatchesAsyncVocabulary ==
  \A initialContext:
    AsyncVotersAt(initialContext)
      = Responsive \cap VotingRoster(initialContext.epoch)
BY DEF AsyncVotersAt

THEOREM HistoricalRecoveryApplicationProjectionMatchesAsyncVocabulary ==
  \A node:
    NodeHasApplication(node)
      <=> HistoricalRecoveryNodeHasApplicationProjection(
            applied, context, node)
BY DEF NodeHasApplication,
       HistoricalRecoveryNodeHasApplicationProjection

IndexedProjectedNodeHasApplication(initialContext, node) ==
  HistoricalRecoveryNodeHasApplicationProjection(
    IndexedCore(initialContext, 49),
    IndexedCore(initialContext, 2), node)

IndexedAsync(initialContext) ==
  INSTANCE SumeragiV2AsyncNetwork
    WITH
       height <- IndexedCore(initialContext, 1),
       context <- IndexedCore(initialContext, 2),
       contextHistory <- IndexedCore(initialContext, 3),
       nodeView <- IndexedCore(initialContext, 4),
       generation <- IndexedCore(initialContext, 5),
       up <- IndexedCore(initialContext, 6),
       gst <- IndexedCore(initialContext, 7),
       availableBodies <- IndexedCore(initialContext, 8),
       durableBodies <- IndexedCore(initialContext, 9),
       retainedLockedBodies <- IndexedCore(initialContext, 10),
       validatedBodies <- IndexedCore(initialContext, 11),
       invalidBodies <- IndexedCore(initialContext, 12),
       seenProposals <- IndexedCore(initialContext, 13),
       receivedVotes <- IndexedCore(initialContext, 14),
       receivedQCs <- IndexedCore(initialContext, 15),
       receivedTimeoutVotes <- IndexedCore(initialContext, 16),
       receivedTCs <- IndexedCore(initialContext, 17),
       proposalIntents <- IndexedCore(initialContext, 18),
       prepareIntents <- IndexedCore(initialContext, 19),
       commitIntents <- IndexedCore(initialContext, 20),
       timeoutIntents <- IndexedCore(initialContext, 21),
       prepareQCs <- IndexedCore(initialContext, 22),
       commitQCs <- IndexedCore(initialContext, 23),
       formedTCs <- IndexedCore(initialContext, 24),
       installedTCs <- IndexedCore(initialContext, 25),
       lastInstalledTc <- IndexedCore(initialContext, 26),
       lockPrepareQc <- IndexedCore(initialContext, 27),
       highestPrepareQc <- IndexedCore(initialContext, 28),
       lockRank <- IndexedCore(initialContext, 29),
       lockSubject <- IndexedCore(initialContext, 30),
       highestRank <- IndexedCore(initialContext, 31),
       highestSubject <- IndexedCore(initialContext, 32),
       pendingProposal <- IndexedCore(initialContext, 33),
       pendingPrepare <- IndexedCore(initialContext, 34),
       pendingObservePrepare <- IndexedCore(initialContext, 35),
       pendingLockCommit <- IndexedCore(initialContext, 36),
       pendingTimeout <- IndexedCore(initialContext, 37),
       pendingInstallTC <- IndexedCore(initialContext, 38),
       pendingDecision <- IndexedCore(initialContext, 39),
       signProposals <- IndexedCore(initialContext, 40),
       signVotes <- IndexedCore(initialContext, 41),
       signTimeouts <- IndexedCore(initialContext, 42),
       proposalNetwork <- IndexedCore(initialContext, 43),
       voteNetwork <- IndexedCore(initialContext, 44),
       qcNetwork <- IndexedCore(initialContext, 45),
       timeoutNetwork <- IndexedCore(initialContext, 46),
       tcNetwork <- IndexedCore(initialContext, 47),
       decisions <- IndexedCore(initialContext, 48),
       applied <- IndexedCore(initialContext, 49),
       asyncNow <- IndexedScheduler(initialContext, 1),
       asyncCommandQueues <- IndexedScheduler(initialContext, 2),
       asyncNextCommandClass <- IndexedScheduler(initialContext, 3),
       asyncFifoOwed <- IndexedScheduler(initialContext, 4),
       asyncTimeoutEmitted <- IndexedScheduler(initialContext, 5),
       asyncRunnerPhase <- IndexedScheduler(initialContext, 6),
       asyncRunnerBudget <- IndexedScheduler(initialContext, 7),
       asyncCausalAdmissionOwed <- IndexedScheduler(initialContext, 8),
       asyncNextLocalSource <- IndexedScheduler(initialContext, 9),
       asyncIoQueues <- IndexedScheduler(initialContext, 10),
       asyncNextServeAdmissionOrdinal <- IndexedScheduler(initialContext, 11),
       asyncNextServeIngressOrdinal <- IndexedScheduler(initialContext, 12),
       asyncServeIngressAdmissions <- IndexedScheduler(initialContext, 13),
       asyncServeAdmissions <- IndexedScheduler(initialContext, 14),
       asyncServeReservations <- IndexedScheduler(initialContext, 15),
       asyncServeTombstones <- IndexedScheduler(initialContext, 16),
       asyncServeAttempts <- IndexedScheduler(initialContext, 17),
       asyncOutstandingWork <- IndexedScheduler(initialContext, 18),
       asyncIoReadyCompletions <- IndexedScheduler(initialContext, 19),
       asyncLocalReadyCompletions <- IndexedScheduler(initialContext, 20),
       asyncNextCompletionSource <- IndexedScheduler(initialContext, 21),
       asyncIoControlAvailable <- IndexedScheduler(initialContext, 22),
       asyncDeferredCompletionQueues <- IndexedScheduler(initialContext, 23),
       asyncDeferredProgressQueues <- IndexedScheduler(initialContext, 24),
       asyncDeferredNormalQueues <- IndexedScheduler(initialContext, 25),
       asyncDeferredHandoffs <- IndexedScheduler(initialContext, 26),
       asyncNextDeferredClass <- IndexedScheduler(initialContext, 27),
       asyncDeferredDrainOwed <- IndexedScheduler(initialContext, 28),
       asyncCausalQueues <- IndexedScheduler(initialContext, 29),
       asyncOutstandingTags <- IndexedScheduler(initialContext, 30),
       asyncNodeDeadlines <- IndexedScheduler(initialContext, 31),
       asyncRetransmitDeadlines <- IndexedScheduler(initialContext, 32),
       asyncNodeServiceDeadlines <- IndexedScheduler(initialContext, 33),
       asyncIoServiceDeadlines <- IndexedScheduler(initialContext, 34),
       asyncSentItems <- IndexedScheduler(initialContext, 35),
       asyncRetainedControl <- IndexedScheduler(initialContext, 36),
       asyncActiveRequests <- IndexedScheduler(initialContext, 37),
       asyncCertifiedResponseClaim <- IndexedScheduler(initialContext, 38),
       asyncTransport <- IndexedScheduler(initialContext, 39),
       asyncIngressLanes <- IndexedScheduler(initialContext, 40),
       asyncIngressReady <- IndexedScheduler(initialContext, 41),
       asyncLeaderWireLifecycles <- IndexedScheduler(initialContext, 42),
       asyncHeldChunks <- IndexedScheduler(initialContext, 43),
       asyncHistoricalRecoveryTargets <- IndexedScheduler(initialContext, 44),
       asyncControlServiceState <- IndexedScheduler(initialContext, 45),
       asyncServiceActivationState <- IndexedScheduler(initialContext, 46),
       asyncRecoveryPhase <- IndexedRecovery(initialContext, 1),
       asyncRecoveryNode <- IndexedRecovery(initialContext, 2),
       asyncRecoveryGeneration <- IndexedRecovery(initialContext, 3),
       asyncRecoveryReplayQueue <- IndexedRecovery(initialContext, 4),
       asyncHistoricalLockRestartAuthorities <-
         IndexedRecovery(initialContext, 5),
       asyncProducerKnownObligations <- IndexedProducer(initialContext, 1),
       asyncProducerConsumedEpisodes <- IndexedProducer(initialContext, 2),
       asyncProducerOriginHistory <- IndexedProducer(initialContext, 3),
       asyncFixedCorridorDeadlines <-
         IndexedFixedCorridorDeadlines(initialContext)

(***************************************************************************
The indexed INSTANCE adds its context argument to inherited pure operators,
even though it substitutes only state variables.  These definitional bridges
keep later certificate and availability proofs in the base quorum vocabulary
without asking a backend to normalize an entire StrongInductiveInvariant at
once.  They import no theorem through the parameterized production instance.
***************************************************************************)
THEOREM IndexedQuorumOperatorsMatchBase ==
  \A initialContext, epoch, signers:
    /\ IndexedAsync(initialContext)!Epochs = Epochs
    /\ IndexedAsync(initialContext)!VotingRoster(epoch)
         = VotingRoster(epoch)
    /\ (IndexedAsync(initialContext)!DualQuorum(epoch, signers)
          <=> DualQuorum(epoch, signers))
BY DEF IndexedAsync!Epochs,
       IndexedAsync!VotingRoster, IndexedAsync!RosterSequence,
       IndexedAsync!DualQuorum, IndexedAsync!CountQuorum,
       IndexedAsync!PowerQuorum, IndexedAsync!PowerOf,
       IndexedAsync!PowerUnits, IndexedAsync!VotingPower,
       IndexedAsync!Cardinality,
       Epochs, VotingRoster, RosterSequence,
       DualQuorum, CountQuorum, PowerQuorum, PowerOf, PowerUnits,
       VotingPower, Cardinality

THEOREM IndexedQuorumConfigurationMatchesBase ==
  \A initialContext:
    IndexedAsync(initialContext)!QuorumConfiguration
      <=> QuorumConfiguration
BY DEF IndexedAsync!QuorumConfiguration,
       IndexedAsync!Epochs, IndexedAsync!RosterSequence,
       IndexedAsync!VotingRoster, IndexedAsync!ValidatorIds,
       IndexedAsync!VotingPower, IndexedAsync!PowerUnits,
       IndexedAsync!PowerOf, IndexedAsync!Byzantine,
       IndexedAsync!Cardinality, IndexedAsync!IsFiniteSet,
       QuorumConfiguration, Epochs, RosterSequence, VotingRoster,
       ValidatorIds, VotingPower, PowerUnits, PowerOf, Byzantine,
       Cardinality, IsFiniteSet

THEOREM IndexedBodyHeldByMatchesBase ==
  \A initialContext, durable, node, bodyContext, roundView, subject:
    IndexedAsync(initialContext)!BodyHeldBy(
      durable, node, bodyContext, roundView, subject)
      <=> BodyHeldBy(durable, node, bodyContext, roundView, subject)
BY DEF IndexedAsync!BodyHeldBy, IndexedAsync!BodyRecord,
       BodyHeldBy, BodyRecord

(***************************************************************************
Proof facts are instantiated over the identical concrete tuple at one arbitrary
free module constant.  `IndexedAsync` above remains the authoritative
production-network relation; this fixed proof-only instance contributes
theorems but no alternate state or step.  A theorem conditional on
VerificationContext membership is semantically valid for every interpretation
of that constant without using an unsupported parameterized proof INSTANCE.
***************************************************************************)
VerificationCore(component) ==
  IndexedCore(VerificationContext, component)

VerificationScheduler(component) ==
  IndexedScheduler(VerificationContext, component)

VerificationRecovery(component) ==
  IndexedRecovery(VerificationContext, component)

VerificationProducer(component) ==
  IndexedProducer(VerificationContext, component)

VerificationFixedCorridorDeadlines ==
  IndexedFixedCorridorDeadlines(VerificationContext)

VerificationAsyncProof ==
  INSTANCE SumeragiV2AsyncTemporalClosureProofs
    WITH
       height <- VerificationCore(1),
       context <- VerificationCore(2),
       contextHistory <- VerificationCore(3),
       nodeView <- VerificationCore(4),
       generation <- VerificationCore(5),
       up <- VerificationCore(6),
       gst <- VerificationCore(7),
       availableBodies <- VerificationCore(8),
       durableBodies <- VerificationCore(9),
       retainedLockedBodies <- VerificationCore(10),
       validatedBodies <- VerificationCore(11),
       invalidBodies <- VerificationCore(12),
       seenProposals <- VerificationCore(13),
       receivedVotes <- VerificationCore(14),
       receivedQCs <- VerificationCore(15),
       receivedTimeoutVotes <- VerificationCore(16),
       receivedTCs <- VerificationCore(17),
       proposalIntents <- VerificationCore(18),
       prepareIntents <- VerificationCore(19),
       commitIntents <- VerificationCore(20),
       timeoutIntents <- VerificationCore(21),
       prepareQCs <- VerificationCore(22),
       commitQCs <- VerificationCore(23),
       formedTCs <- VerificationCore(24),
       installedTCs <- VerificationCore(25),
       lastInstalledTc <- VerificationCore(26),
       lockPrepareQc <- VerificationCore(27),
       highestPrepareQc <- VerificationCore(28),
       lockRank <- VerificationCore(29),
       lockSubject <- VerificationCore(30),
       highestRank <- VerificationCore(31),
       highestSubject <- VerificationCore(32),
       pendingProposal <- VerificationCore(33),
       pendingPrepare <- VerificationCore(34),
       pendingObservePrepare <- VerificationCore(35),
       pendingLockCommit <- VerificationCore(36),
       pendingTimeout <- VerificationCore(37),
       pendingInstallTC <- VerificationCore(38),
       pendingDecision <- VerificationCore(39),
       signProposals <- VerificationCore(40),
       signVotes <- VerificationCore(41),
       signTimeouts <- VerificationCore(42),
       proposalNetwork <- VerificationCore(43),
       voteNetwork <- VerificationCore(44),
       qcNetwork <- VerificationCore(45),
       timeoutNetwork <- VerificationCore(46),
       tcNetwork <- VerificationCore(47),
       decisions <- VerificationCore(48),
       applied <- VerificationCore(49),
       asyncNow <- VerificationScheduler(1),
       asyncCommandQueues <- VerificationScheduler(2),
       asyncNextCommandClass <- VerificationScheduler(3),
       asyncFifoOwed <- VerificationScheduler(4),
       asyncTimeoutEmitted <- VerificationScheduler(5),
       asyncRunnerPhase <- VerificationScheduler(6),
       asyncRunnerBudget <- VerificationScheduler(7),
       asyncCausalAdmissionOwed <- VerificationScheduler(8),
       asyncNextLocalSource <- VerificationScheduler(9),
       asyncIoQueues <- VerificationScheduler(10),
       asyncNextServeAdmissionOrdinal <- VerificationScheduler(11),
       asyncNextServeIngressOrdinal <- VerificationScheduler(12),
       asyncServeIngressAdmissions <- VerificationScheduler(13),
       asyncServeAdmissions <- VerificationScheduler(14),
       asyncServeReservations <- VerificationScheduler(15),
       asyncServeTombstones <- VerificationScheduler(16),
       asyncServeAttempts <- VerificationScheduler(17),
       asyncOutstandingWork <- VerificationScheduler(18),
       asyncIoReadyCompletions <- VerificationScheduler(19),
       asyncLocalReadyCompletions <- VerificationScheduler(20),
       asyncNextCompletionSource <- VerificationScheduler(21),
       asyncIoControlAvailable <- VerificationScheduler(22),
       asyncDeferredCompletionQueues <- VerificationScheduler(23),
       asyncDeferredProgressQueues <- VerificationScheduler(24),
       asyncDeferredNormalQueues <- VerificationScheduler(25),
       asyncDeferredHandoffs <- VerificationScheduler(26),
       asyncNextDeferredClass <- VerificationScheduler(27),
       asyncDeferredDrainOwed <- VerificationScheduler(28),
       asyncCausalQueues <- VerificationScheduler(29),
       asyncOutstandingTags <- VerificationScheduler(30),
       asyncNodeDeadlines <- VerificationScheduler(31),
       asyncRetransmitDeadlines <- VerificationScheduler(32),
       asyncNodeServiceDeadlines <- VerificationScheduler(33),
       asyncIoServiceDeadlines <- VerificationScheduler(34),
       asyncSentItems <- VerificationScheduler(35),
       asyncRetainedControl <- VerificationScheduler(36),
       asyncActiveRequests <- VerificationScheduler(37),
       asyncCertifiedResponseClaim <- VerificationScheduler(38),
       asyncTransport <- VerificationScheduler(39),
       asyncIngressLanes <- VerificationScheduler(40),
       asyncIngressReady <- VerificationScheduler(41),
       asyncLeaderWireLifecycles <- VerificationScheduler(42),
       asyncHeldChunks <- VerificationScheduler(43),
       asyncHistoricalRecoveryTargets <- VerificationScheduler(44),
       asyncControlServiceState <- VerificationScheduler(45),
       asyncServiceActivationState <- VerificationScheduler(46),
       asyncRecoveryPhase <- VerificationRecovery(1),
       asyncRecoveryNode <- VerificationRecovery(2),
       asyncRecoveryGeneration <- VerificationRecovery(3),
       asyncRecoveryReplayQueue <- VerificationRecovery(4),
       asyncHistoricalLockRestartAuthorities <- VerificationRecovery(5),
       asyncProducerKnownObligations <- VerificationProducer(1),
       asyncProducerConsumedEpisodes <- VerificationProducer(2),
       asyncProducerOriginHistory <- VerificationProducer(3),
       asyncFixedCorridorDeadlines <- VerificationFixedCorridorDeadlines

AdmissibleContextRecords ==
  {initialContext \in ContextRecords:
     FrozenContextAdmissible(initialContext)}

IndexedAsyncStateShape ==
  /\ DOMAIN indexedAsyncState = AdmissibleContextRecords
  /\ \A initialContext \in AdmissibleContextRecords:
       /\ Len(indexedAsyncState[initialContext]) = 6
       /\ DOMAIN indexedAsyncState[initialContext] = 1..6
       /\ indexedAsyncState[initialContext][1]
            = indexedAsyncState[initialContext][2][7]
       /\ Len(indexedAsyncState[initialContext][2]) = 49
       /\ DOMAIN indexedAsyncState[initialContext][2] = 1..49
       /\ Len(indexedAsyncState[initialContext][3]) = 46
       /\ DOMAIN indexedAsyncState[initialContext][3] = 1..46
       /\ Len(indexedAsyncState[initialContext][4]) = 5
       /\ DOMAIN indexedAsyncState[initialContext][4] = 1..5
       /\ Len(indexedAsyncState[initialContext][5]) = 3
       /\ DOMAIN indexedAsyncState[initialContext][5] = 1..3

JoinedByContextShape ==
  joinedByContext \in [AdmissibleContextRecords -> SUBSET ValidatorIds]

GenesisContext == ContextRecord(0, <<>>)

CanonicalIndexedContext(blockHeight) ==
  Chain!ContextRecord(blockHeight, Chain!HistoryThrough(blockHeight))

JoinedContexts ==
  {initialContext \in AdmissibleContextRecords:
     joinedByContext[initialContext] # {}}

JoinedCanonicalDescendant(initialContext) ==
  \E descendantContext \in JoinedContexts:
    /\ descendantContext.height > initialContext.height
    /\ descendantContext =
         Chain!ContextRecord(descendantContext.height,
                             Chain!HistoryThrough(descendantContext.height))

IndexedNodeCurrentAt(initialContext, node) ==
  /\ node \in joinedByContext[initialContext]
  /\ nodeContext[node] = initialContext

ExactNodeLocationAt(initialContext, node) ==
  /\ nodeHeight[node] = initialContext.height
  /\ nodeContext[node] = initialContext

IndexedDecisions(initialContext) == IndexedCore(initialContext, 48)
IndexedApplications(initialContext) == IndexedCore(initialContext, 49)

(***************************************************************************
InitAt for a non-genesis context contains one synthetic parent receipt. It is
private bootstrap evidence for that exact one-height proof, not a receipt
created by the context itself. The ChainEpoch projection therefore selects
only receipts whose frozen context and height are exactly this instance. This
keeps every bootstrap receipt available internally while making the indexed
genesis receipt union genuinely empty.
***************************************************************************)
IndexedCurrentDecisions(initialContext) ==
  {decision \in IndexedDecisions(initialContext):
     /\ decision.qc.context = initialContext
     /\ decision.qc.height = initialContext.height}

IndexedCurrentApplications(initialContext) ==
  {application \in IndexedApplications(initialContext):
     /\ application.qc.context = initialContext
     /\ application.qc.height = initialContext.height}

IndexedDecisionEvidence ==
  UNION {IndexedCurrentDecisions(initialContext):
           initialContext \in AdmissibleContextRecords}

IndexedApplicationEvidence ==
  UNION {IndexedCurrentApplications(initialContext):
           initialContext \in AdmissibleContextRecords}

SuccessorActivationStatusValues ==
  {"Idle", "Queued", "Running", "Complete"}

SuccessorPredecessorOwnershipValues == {"Published", "Absent"}

SuccessorActivationRequiredPrerequisites ==
  {"DeferredStatus", "AdapterReady", "RuntimeReady", "ServicesReady",
   "StartupApplied", "ClocksArmed", "IngressOpen"}

SuccessorActivationAdapterPrerequisites ==
  {"DeferredStatus", "AdapterReady"}

SuccessorActivationRuntimePrerequisites ==
  SuccessorActivationAdapterPrerequisites \cup {"RuntimeReady"}

SuccessorActivationServicePrerequisites ==
  SuccessorActivationRuntimePrerequisites \cup {"ServicesReady"}

SuccessorActivationStartupPrerequisites ==
  SuccessorActivationServicePrerequisites \cup {"StartupApplied"}

SuccessorActivationClockPrerequisites ==
  SuccessorActivationStartupPrerequisites \cup {"ClocksArmed"}

SuccessorActivationOwnerSet ==
  [parentContext: AdmissibleContextRecords, node: ValidatorIds]

SuccessorActivationTokenSet ==
  [kind: {"Applied", "Recovered"},
   parentContext: AdmissibleContextRecords,
   node: ValidatorIds,
   successorContext: AdmissibleContextRecords]

CompleteTipRecoveryAuthoritySet ==
  [kind: {"CompleteTip"},
   parentContext: AdmissibleContextRecords,
   node: ValidatorIds,
   successorContext: AdmissibleContextRecords,
   application: Chain!DecisionEvidenceSet]

SnapshotBootstrapRecoveryAuthoritySet ==
  [kind: {"SnapshotBootstrap"},
   parentContext: AdmissibleContextRecords,
   node: ValidatorIds,
   successorContext: AdmissibleContextRecords]

SuccessorRecoveryAuthoritySet ==
  CompleteTipRecoveryAuthoritySet \cup
    SnapshotBootstrapRecoveryAuthoritySet

SuccessorActivationMarkerSet ==
  [parentContext: AdmissibleContextRecords,
   node: ValidatorIds,
   successorContext: AdmissibleContextRecords,
   successorHeight: Heights,
   generation: Generations,
   view: Views,
   transition: {"SuccessorHeightActivated"}]

SuccessorActivationOwner(parentContext, node) ==
  [parentContext |-> parentContext, node |-> node]

SuccessorActivationToken(kind, parentContext, node, successorContext) ==
  [kind |-> kind,
   parentContext |-> parentContext,
   node |-> node,
   successorContext |-> successorContext]

CompleteTipRecoveryAuthorityRecord(parentContext, node,
                                   successorContext, application) ==
  [kind |-> "CompleteTip",
   parentContext |-> parentContext,
   node |-> node,
   successorContext |-> successorContext,
   application |-> application]

SnapshotBootstrapRecoveryAuthorityRecord(parentContext, node,
                                         successorContext) ==
  [kind |-> "SnapshotBootstrap",
   parentContext |-> parentContext,
   node |-> node,
   successorContext |-> successorContext]

SuccessorActivationMarker(parentContext, node, successorContext) ==
  [parentContext |-> parentContext,
   node |-> node,
   successorContext |-> successorContext,
   successorHeight |-> successorContext.height,
   generation |-> 0,
   view |-> 0,
   transition |-> "SuccessorHeightActivated"]

SuccessorActivationVars ==
  <<successorActivationStatus,
    successorPredecessorStatusOwnership,
    successorActivationPrerequisites,
    successorActivationTokens,
    successorRecoveryAuthorities,
    preparedSuccessorActivationMarkers,
    publishedSuccessorActivationMarkers,
    successorActivationFailures,
    successorActivationFailureHistory,
    successorActivationCompletions>>

SuccessorActivationShape ==
  /\ successorActivationStatus
       \in [AdmissibleContextRecords ->
             [ValidatorIds -> SuccessorActivationStatusValues]]
  /\ successorPredecessorStatusOwnership
       \in [AdmissibleContextRecords ->
             [ValidatorIds -> SuccessorPredecessorOwnershipValues]]
  /\ successorActivationPrerequisites
       \in [AdmissibleContextRecords ->
             [ValidatorIds -> SUBSET SuccessorActivationRequiredPrerequisites]]
  /\ successorActivationTokens \subseteq SuccessorActivationTokenSet
  /\ successorRecoveryAuthorities \subseteq SuccessorRecoveryAuthoritySet
  /\ preparedSuccessorActivationMarkers
       \subseteq SuccessorActivationMarkerSet
  /\ publishedSuccessorActivationMarkers
       \subseteq SuccessorActivationMarkerSet
  /\ successorActivationFailures \subseteq SuccessorActivationOwnerSet
  /\ successorActivationFailureHistory \subseteq SuccessorActivationOwnerSet
  /\ successorActivationCompletions \subseteq SuccessorActivationTokenSet

IndexedDecisionReceiptProjection ==
  durableDecisionEvidence = IndexedDecisionEvidence

IndexedApplicationReceiptProjection ==
  durableApplicationEvidence = IndexedApplicationEvidence

IndexedTotalReceiptProjection ==
  /\ IndexedDecisionReceiptProjection
  /\ IndexedApplicationReceiptProjection

(***************************************************************************
Authenticated historical recovery is owned by the exact Async instance.

The chain wrapper may open recovery only for a responsive node located at the
frozen context and only when a joined responsive applied archive still holds
the body certified by the exact CommitQC.  The serving archive authenticates
its own response and must be one of that QC's frozen signers; full-roster
request fanout is broader than this response authority.  The exact body hash
and signer identity bind the historical source. `OpenHistoricalRecovery`
records that exact target in
scheduler component 44. From then on the ordinary Async reducer persists the
decision, recovers and stores the body, validates it, and appends the
application to the same per-context `decisions` and `applied` sets used by
ordinary consensus. There is no shadow receipt set, stage variable, or
independent recovery step.
***************************************************************************)
HistoricalRecoveryRecord(node, source) ==
  [node |-> node, qc |-> source.qc]

IndexedHistoricalRecoveryTargetReady(initialContext, node) ==
  /\ node \in Responsive
  /\ node \in IndexedCore(initialContext, 6)
  /\ node \in joinedByContext[initialContext]
  /\ ExactNodeLocationAt(initialContext, node)
  /\ ~IndexedAsync(initialContext)!NodeHasDecision(node)
  /\ ~IndexedProjectedNodeHasApplication(initialContext, node)
  /\ ~IndexedAsync(initialContext)!HistoricalRecoveryTarget(node)

IndexedHistoricalRecoverySourceReady(initialContext, server, source) ==
  /\ initialContext \in JoinedContexts
  /\ source \in IndexedCurrentDecisions(initialContext)
  /\ source \in IndexedCurrentApplications(initialContext)
  /\ source \in durableDecisionEvidence
  /\ source \in durableApplicationEvidence
  /\ source.node = server
  /\ \/ /\ initialContext.height < MaxHeight
        /\ Chain!CanonicalCommitForSlot(
             source.qc, initialContext.height + 1)
     \/ /\ initialContext.height = MaxHeight
        /\ Chain!ReceiptOutsideChainHorizon(source)
  /\ server \in IndexedAsync(initialContext)!
                 AsyncCurrentResponsiveVoters
  /\ server \in IndexedCore(initialContext, 6)
  /\ server \in joinedByContext[initialContext]
  /\ server \in source.qc.signers
  /\ BodyHeldBy(IndexedCore(initialContext, 9), server,
                 initialContext, source.qc.view, source.qc.subject)

IndexedHistoricalRecoveryReady(initialContext, node) ==
  /\ node \in Responsive
  /\ node \in IndexedCore(initialContext, 6)
  /\ node \in joinedByContext[initialContext]
  /\ ExactNodeLocationAt(initialContext, node)
  /\ ~IndexedAsync(initialContext)!NodeHasDecision(node)
  /\ ~IndexedProjectedNodeHasApplication(initialContext, node)
  /\ \E server \in ValidatorIds,
       source \in Chain!DecisionEvidenceSet:
       IndexedHistoricalRecoverySourceReady(
         initialContext, server, source)

IndexedOpenHistoricalRecovery(initialContext, node, server, source) ==
  /\ IndexedHistoricalRecoveryTargetReady(initialContext, node)
  /\ IndexedHistoricalRecoverySourceReady(
       initialContext, server, source)
  /\ IndexedAsync(initialContext)!OpenHistoricalRecovery(node)

IndexedChainVars ==
  <<indexedAsyncState, joinedByContext,
    SuccessorActivationVars, Chain!ChainEpochVars>>

(***************************************************************************
The joined runner is a restriction of the exact AsyncNext relation, never an
alternate step. Current consensus work requires only the selected node's join;
both RunNode and direct Commit-certificate discovery also require that this is
the node's authoritative current context. Historical serving and outstanding
IO remain enabled for every node that ever joined the context, even after its
authoritative nodeContext advances.
***************************************************************************)
IndexedJoinedRunnerStep(initialContext) ==
  \/ \E node \in IndexedAsync(initialContext)!AsyncCurrentResponsiveVoters:
       /\ IndexedNodeCurrentAt(initialContext, node)
       /\ IndexedAsync(initialContext)!RunNode(node)
  \/ \E node \in Responsive:
       IndexedAsync(initialContext)!RunHistoricalRecoveryNode(node)
  \/ \E node \in Responsive:
       /\ node \in joinedByContext[initialContext]
       /\ IndexedAsync(initialContext)!RunHistoricalServer(node)

IndexedJoinedNonRunnerStep(initialContext) ==
  /\ \/ IndexedAsync(initialContext)!AsyncSetGST
     \/ IndexedAsync(initialContext)!AsyncTick
     \/ \E node \in IndexedAsync(initialContext)!
                    AsyncCurrentResponsiveVoters:
          /\ IndexedNodeCurrentAt(initialContext, node)
          /\ IndexedAsync(initialContext)!
               DirectCommitCertificateDiscoveryStep(node)
     \/ \E node \in Responsive:
          IndexedAsync(initialContext)!
            DirectHistoricalCommitCertificateDiscoveryStep(node)
     \/ \E node \in Responsive:
          IndexedAsync(initialContext)!
            ServiceHistoricalRecoveryIoWorker(node)
     \/ \E node \in Responsive:
          IndexedAsync(initialContext)!
            EnqueueHistoricalRecoveryIoLocalControl(node)
     \/ \E node \in ValidatorIds,
           server \in ValidatorIds,
           source \in Chain!DecisionEvidenceSet:
          IndexedOpenHistoricalRecovery(
            initialContext, node, server, source)
     \/ \E node \in Responsive:
          /\ node \in joinedByContext[initialContext]
          /\ IndexedAsync(initialContext)!ServiceIoWorker(node)
     \/ \E node \in IndexedAsync(initialContext)!AsyncCurrentResponsiveVoters:
          /\ node \in joinedByContext[initialContext]
          /\ IndexedAsync(initialContext)!EnqueueIoLocalControl(node)
     \/ \E node \in IndexedAsync(initialContext)!AsyncCurrentResponsiveVoters:
          /\ IndexedNodeCurrentAt(initialContext, node)
          /\ IndexedAsync(initialContext)!
               ResolveCandidateProducerContinuation(node)
     \/ IndexedAsync(initialContext)!AsyncNetworkStep
     \/ IndexedAsync(initialContext)!AsyncFaultStep
  /\ UNCHANGED IndexedScheduler(initialContext, 33)

IndexedJoinedNonCrashStep(initialContext) ==
  /\ (IndexedJoinedRunnerStep(initialContext)
        \/ IndexedJoinedNonRunnerStep(initialContext))
  /\ UNCHANGED <<IndexedCore(initialContext, 6),
                 IndexedAsync(initialContext)!AsyncRecoveryControlVars>>

IndexedJoinedAsyncNext(initialContext) ==
  /\ (IndexedJoinedNonCrashStep(initialContext)
        \/ \E node \in ValidatorIds:
             IndexedAsync(initialContext)!PreGstCrash(node))
  /\ IndexedAsync(initialContext)!
       AsyncHistoricalLockRestartAuthorityTransition
  /\ IndexedAsync(initialContext)!AsyncProducerProjectionStep
  \* Service activation belongs only to the atomic successor-publication
  \* actions below.  An ordinary joined-context step must retain component 46;
  \* the unchanged arm of AsyncServiceActivationTransition then refines the
  \* exact standalone AsyncNext relation without activating an unjoined peer.
  /\ UNCHANGED IndexedScheduler(initialContext, 46)
  /\ UNCHANGED <<IndexedCore(initialContext, 1),
                 IndexedCore(initialContext, 2)>>
  /\ [IndexedAsync(initialContext)!Next]_(
       IndexedAsync(initialContext)!vars)

NewIndexedDecisionReceipt(initialContext, decision) ==
  /\ decision \notin IndexedDecisions(initialContext)
  /\ IndexedDecisions(initialContext)' =
       IndexedDecisions(initialContext) \cup {decision}
  /\ IndexedApplications(initialContext)' =
       IndexedApplications(initialContext)

NewIndexedApplicationReceipt(initialContext, application) ==
  /\ application \notin IndexedApplications(initialContext)
  /\ IndexedApplications(initialContext)' =
       IndexedApplications(initialContext) \cup {application}
  /\ IndexedDecisions(initialContext)' =
       IndexedDecisions(initialContext)

NoNewIndexedDurableReceipt(initialContext) ==
  /\ IndexedDecisions(initialContext)' =
       IndexedDecisions(initialContext)
  /\ IndexedApplications(initialContext)' =
       IndexedApplications(initialContext)

IndexedDecisionReceiptHandoff(initialContext, decision) ==
  /\ NewIndexedDecisionReceipt(initialContext, decision)
  /\ UNCHANGED <<joinedByContext, SuccessorActivationVars>>
  /\ \/ Chain!RecordCertifiedNext(decision)
     \/ Chain!RecordKnownDecision(decision)

SuccessorContextFor(application) ==
  CanonicalIndexedContext(application.qc.context.height + 1)

ExactDurableParentApplication(parentContext, node, application) ==
  /\ parentContext.height < MaxHeight
  /\ application \in durableDecisionEvidence
  /\ application \in durableApplicationEvidence
  /\ application.node = node
  /\ application.qc.context = parentContext
  /\ application.qc.height = parentContext.height
  /\ Chain!CanonicalCommitForSlot(
       application.qc, parentContext.height + 1)
  /\ nodeHeight[node] = parentContext.height + 1
  /\ nodeContext[node] =
       CanonicalIndexedContext(parentContext.height + 1)

ExactSuccessorActivationToken(kind, parentContext, node,
                              successorContext) ==
  /\ successorContext =
       CanonicalIndexedContext(parentContext.height + 1)
  /\ SuccessorActivationToken(
       kind, parentContext, node, successorContext)
       \in successorActivationTokens

ExactCompleteTipRecoveryAuthority(parentContext, node,
                                  successorContext, application) ==
  /\ ExactDurableParentApplication(parentContext, node, application)
  /\ successorContext =
       CanonicalIndexedContext(parentContext.height + 1)
  /\ CompleteTipRecoveryAuthorityRecord(
       parentContext, node, successorContext, application)
       \in successorRecoveryAuthorities

ExactSnapshotBootstrapRecoveryAuthority(parentContext, node,
                                        successorContext) ==
  /\ successorContext =
       CanonicalIndexedContext(parentContext.height + 1)
  /\ SnapshotBootstrapRecoveryAuthorityRecord(
       parentContext, node, successorContext)
       \in successorRecoveryAuthorities

(***************************************************************************
Snapshot bootstrap is an initialization authority, not evidence that the
local database contains the exact CommitQC-backed complete tip.  Its tagged
record deliberately has no application field, and the complete-tip
credential predicate above accepts only the independently tagged authority
whose application is present in both durable evidence sets.  A later genesis
handoff proof may refine snapshot startup; it cannot discharge exact-tip
recovery by projecting the imported height into an `Option<Height>`.
***************************************************************************)
THEOREM SnapshotBootstrapAuthorityIsDistinctFromCompleteTipAuthority ==
  \A parentContext \in AdmissibleContextRecords,
     node \in ValidatorIds,
     successorContext \in AdmissibleContextRecords,
     application \in Chain!DecisionEvidenceSet:
    SnapshotBootstrapRecoveryAuthorityRecord(
      parentContext, node, successorContext)
      # CompleteTipRecoveryAuthorityRecord(
          parentContext, node, successorContext, application)
BY Isa DEF SnapshotBootstrapRecoveryAuthorityRecord,
           CompleteTipRecoveryAuthorityRecord

QueueSuccessorActivation(parentContext, node) ==
  /\ parentContext.height < MaxHeight
  /\ successorActivationStatus[parentContext][node] = "Idle"
  /\ successorPredecessorStatusOwnership[parentContext][node] = "Absent"
  /\ SuccessorActivationOwner(parentContext, node)
       \notin successorActivationFailures
  /\ successorActivationStatus' =
       [successorActivationStatus EXCEPT
          ![parentContext][node] = "Queued"]
  /\ successorPredecessorStatusOwnership' =
       [successorPredecessorStatusOwnership EXCEPT
          ![parentContext][node] = "Published"]
  /\ successorActivationPrerequisites' =
       [successorActivationPrerequisites EXCEPT
          ![parentContext][node] = {}]
  /\ UNCHANGED <<successorActivationTokens,
                  successorRecoveryAuthorities,
                  preparedSuccessorActivationMarkers,
                  publishedSuccessorActivationMarkers,
                  successorActivationFailures,
                  successorActivationFailureHistory,
                  successorActivationCompletions>>

(***************************************************************************
RecordAppliedNext is defined only below the finite chain horizon. The certified
prefix supplies a valid subject at every position in its exact successor
lineage, so the context inserted into joinedByContext is always an existing
member of the pre-created admissible domain.
***************************************************************************)
THEOREM AppliedSuccessorIsAdmissible ==
  \A application \in Chain!DecisionEvidenceSet:
    Chain!ChainEpochInvariant /\ Chain!RecordAppliedNext(application)
      => /\ SuccessorContextFor(application)
               \in AdmissibleContextRecords
         /\ SuccessorContextFor(application).height
               = nodeHeight[application.node] + 1
BY Isa DEF SuccessorContextFor, AdmissibleContextRecords,
           FrozenContextAdmissible, ContextRecords, LineagesAt,
           Chain!ChainEpochInvariant, Chain!ChainEpochTypeInvariant,
           Chain!CertifiedPrefixBacked, Chain!RecordAppliedNext,
           Chain!CanonicalCommitForSlot, Chain!HistoryThrough

IndexedApplicationReceiptHandoff(initialContext, application) ==
  /\ NewIndexedApplicationReceipt(initialContext, application)
  /\ \/ /\ ExactNodeLocationAt(initialContext, application.node)
        /\ Chain!RecordAppliedNext(application)
        /\ QueueSuccessorActivation(initialContext, application.node)
        /\ UNCHANGED joinedByContext
     \/ /\ Chain!RecordKnownApplication(application)
        /\ UNCHANGED <<joinedByContext, SuccessorActivationVars>>

IndexedReceiptFreeChainStutter(initialContext) ==
  /\ NoNewIndexedDurableReceipt(initialContext)
  /\ UNCHANGED <<joinedByContext, SuccessorActivationVars,
                  Chain!ChainEpochVars>>

IndexedReceiptClassification(initialContext) ==
  \/ IndexedReceiptFreeChainStutter(initialContext)
  \/ \E decision \in Chain!DecisionEvidenceSet:
       IndexedDecisionReceiptHandoff(initialContext, decision)
  \/ \E application \in Chain!DecisionEvidenceSet:
       IndexedApplicationReceiptHandoff(initialContext, application)

(***************************************************************************
Successor activation is a durable, ordered protocol rather than an atomic
side effect of application.  `successorPredecessorStatusOwnership` models the
process-visible predecessor registry entry independently from durable parent
application evidence. Applied startup may publish that predecessor entry. A
clean process restart and a restart after a latched failure both rehydrate the
exact durable complete tip while the predecessor entry is absent. Failure and
restart are separate lifecycle actions: an Applied failure leaves its visible
status Running until restart, and a Recovered attempt may fail again. Both
paths share the ordered startup pipeline, but only Applied publication writes
physical `Complete`. Snapshot bootstrap remains a separate initialization
authority and is never accepted by the complete-tip credential below.
***************************************************************************)
SuccessorActivationEnvironmentStutter ==
  /\ UNCHANGED indexedAsyncState
  /\ UNCHANGED Chain!ChainEpochVars

(***************************************************************************
The final publication step also activates the exact successor-height service
owner.  Every pre-created Async instance starts from the canonical standalone
initializer.  Its first independent join irreversibly restricts service to
that node; every later join monotonically adds one node and rearms both local
service clocks from the successor instance's current proof time.  No other
context changes, and the activation record is internal scheduler metadata.
***************************************************************************)
SuccessorActivationEnvironmentActivatesNode(successorContext, node) ==
  /\ IF joinedByContext[successorContext] = {}
     THEN IndexedAsync(successorContext)!
            AsyncEnterIndexedServiceActivation(node)
     ELSE IndexedAsync(successorContext)!AsyncActivateServiceNode(node)
  /\ \A otherContext \in
          AdmissibleContextRecords \ {successorContext}:
       UNCHANGED IndexedAsyncStateAt(otherContext)
  /\ UNCHANGED Chain!ChainEpochVars

SuccessorActivationCredentialReady(parentContext, node,
                                   successorContext) ==
  /\ successorActivationStatus[parentContext][node] = "Running"
  /\ SuccessorActivationOwner(parentContext, node)
       \notin successorActivationFailures
  /\ \/ /\ successorPredecessorStatusOwnership[parentContext][node]
              = "Published"
        /\ ExactSuccessorActivationToken(
             "Applied", parentContext, node, successorContext)
     \/ /\ successorPredecessorStatusOwnership[parentContext][node]
              = "Absent"
        /\ ExactSuccessorActivationToken(
             "Recovered", parentContext, node, successorContext)
        /\ \E application \in Chain!DecisionEvidenceSet:
             ExactCompleteTipRecoveryAuthority(
               parentContext, node, successorContext, application)

BeginSuccessorActivation(parentContext, node, successorContext) ==
  LET token == SuccessorActivationToken(
                 "Applied", parentContext, node, successorContext)
  IN /\ successorContext =
          CanonicalIndexedContext(parentContext.height + 1)
     /\ successorActivationStatus[parentContext][node] = "Queued"
     /\ successorPredecessorStatusOwnership[parentContext][node] = "Published"
     /\ successorActivationPrerequisites[parentContext][node] = {}
     /\ token \notin successorActivationTokens
     /\ \E application \in Chain!DecisionEvidenceSet:
          ExactDurableParentApplication(parentContext, node, application)
     /\ successorActivationStatus' =
          [successorActivationStatus EXCEPT
             ![parentContext][node] = "Running"]
     /\ UNCHANGED <<successorPredecessorStatusOwnership,
                     successorActivationPrerequisites,
                     successorActivationTokens,
                     successorRecoveryAuthorities,
                     preparedSuccessorActivationMarkers,
                     publishedSuccessorActivationMarkers,
                     successorActivationFailures,
                     successorActivationFailureHistory,
                     successorActivationCompletions,
                     joinedByContext>>
     /\ SuccessorActivationEnvironmentStutter

BindAppliedSuccessorActivationToken(parentContext, node,
                                    successorContext) ==
  LET token == SuccessorActivationToken(
                  "Applied", parentContext, node, successorContext)
  IN /\ successorContext =
           CanonicalIndexedContext(parentContext.height + 1)
     /\ successorActivationStatus[parentContext][node] = "Running"
     /\ successorPredecessorStatusOwnership[parentContext][node]
          = "Published"
     /\ successorActivationPrerequisites[parentContext][node] = {}
     /\ SuccessorActivationOwner(parentContext, node)
          \notin successorActivationFailures
     /\ \E application \in Chain!DecisionEvidenceSet:
          ExactDurableParentApplication(parentContext, node, application)
     /\ token \notin successorActivationTokens
     /\ successorActivationTokens' =
          successorActivationTokens \cup {token}
     /\ UNCHANGED <<successorActivationStatus,
                     successorPredecessorStatusOwnership,
                     successorActivationPrerequisites,
                     successorRecoveryAuthorities,
                     preparedSuccessorActivationMarkers,
                     publishedSuccessorActivationMarkers,
                     successorActivationFailures,
                     successorActivationFailureHistory,
                     successorActivationCompletions,
                     joinedByContext>>
     /\ SuccessorActivationEnvironmentStutter

LatchAppliedSuccessorStartupFailure(parentContext, node) ==
  LET owner == SuccessorActivationOwner(parentContext, node)
  IN /\ successorActivationStatus[parentContext][node] = "Running"
     /\ successorPredecessorStatusOwnership[parentContext][node]
          = "Published"
     /\ owner \notin successorActivationFailures
     /\ \E application \in Chain!DecisionEvidenceSet:
          ExactDurableParentApplication(parentContext, node, application)
     /\ successorActivationPrerequisites' =
          [successorActivationPrerequisites EXCEPT
             ![parentContext][node] = {}]
     /\ successorActivationTokens' =
          {token \in successorActivationTokens:
             \/ token.parentContext # parentContext
             \/ token.node # node}
     /\ successorRecoveryAuthorities' =
          {authority \in successorRecoveryAuthorities:
             \/ authority.parentContext # parentContext
             \/ authority.node # node}
     /\ preparedSuccessorActivationMarkers' =
          {marker \in preparedSuccessorActivationMarkers:
             \/ marker.parentContext # parentContext
             \/ marker.node # node}
     /\ successorActivationFailures' =
          successorActivationFailures \cup {owner}
     /\ successorActivationFailureHistory' =
          successorActivationFailureHistory \cup {owner}
     /\ UNCHANGED <<successorActivationStatus,
                     successorPredecessorStatusOwnership,
                     publishedSuccessorActivationMarkers,
                     successorActivationCompletions,
                     joinedByContext>>
     /\ SuccessorActivationEnvironmentStutter

LatchRecoveredSuccessorStartupFailure(parentContext, node,
                                      successorContext, application) ==
  LET owner == SuccessorActivationOwner(parentContext, node)
      token == SuccessorActivationToken(
                 "Recovered", parentContext, node, successorContext)
  IN /\ successorContext =
           CanonicalIndexedContext(parentContext.height + 1)
     /\ successorActivationStatus[parentContext][node] = "Running"
     /\ successorPredecessorStatusOwnership[parentContext][node] = "Absent"
     /\ owner \notin successorActivationFailures
     /\ token \in successorActivationTokens
     /\ ExactCompleteTipRecoveryAuthority(
          parentContext, node, successorContext, application)
     /\ successorActivationPrerequisites' =
          [successorActivationPrerequisites EXCEPT
             ![parentContext][node] = {}]
     /\ successorActivationTokens' =
          {candidate \in successorActivationTokens:
             \/ candidate.parentContext # parentContext
             \/ candidate.node # node}
     /\ successorRecoveryAuthorities' =
          {authority \in successorRecoveryAuthorities:
             \/ authority.parentContext # parentContext
             \/ authority.node # node}
     /\ preparedSuccessorActivationMarkers' =
          {marker \in preparedSuccessorActivationMarkers:
             \/ marker.parentContext # parentContext
             \/ marker.node # node}
     /\ successorActivationFailures' =
          successorActivationFailures \cup {owner}
     /\ successorActivationFailureHistory' =
          successorActivationFailureHistory \cup {owner}
     /\ UNCHANGED <<successorActivationStatus,
                     successorPredecessorStatusOwnership,
                     publishedSuccessorActivationMarkers,
                     successorActivationCompletions,
                     joinedByContext>>
     /\ SuccessorActivationEnvironmentStutter

RehydrateCleanCompleteTipSuccessorStartup(parentContext, node,
                                          successorContext, application) ==
  LET owner == SuccessorActivationOwner(parentContext, node)
      authority == CompleteTipRecoveryAuthorityRecord(
                     parentContext, node, successorContext, application)
  IN /\ successorContext =
           CanonicalIndexedContext(parentContext.height + 1)
     /\ successorActivationStatus[parentContext][node]
          \in {"Queued", "Running"}
     /\ successorPredecessorStatusOwnership[parentContext][node]
          = "Published"
     /\ owner \notin successorActivationFailures
     /\ ExactDurableParentApplication(parentContext, node, application)
     /\ successorActivationStatus' =
          [successorActivationStatus EXCEPT
             ![parentContext][node] = "Queued"]
     /\ successorPredecessorStatusOwnership' =
          [successorPredecessorStatusOwnership EXCEPT
             ![parentContext][node] = "Absent"]
     /\ successorActivationPrerequisites' =
          [successorActivationPrerequisites EXCEPT
             ![parentContext][node] = {}]
     /\ successorActivationTokens' =
          {token \in successorActivationTokens:
             \/ token.parentContext # parentContext
             \/ token.node # node}
     /\ successorRecoveryAuthorities' =
          {candidate \in successorRecoveryAuthorities:
             \/ candidate.parentContext # parentContext
             \/ candidate.node # node}
            \cup {authority}
     /\ preparedSuccessorActivationMarkers' =
          {marker \in preparedSuccessorActivationMarkers:
             \/ marker.parentContext # parentContext
             \/ marker.node # node}
     /\ UNCHANGED <<publishedSuccessorActivationMarkers,
                     successorActivationFailures,
                     successorActivationFailureHistory,
                     successorActivationCompletions,
                     joinedByContext>>
     /\ SuccessorActivationEnvironmentStutter

RehydrateFailedSuccessorStartup(parentContext, node,
                                successorContext, application) ==
  LET owner == SuccessorActivationOwner(parentContext, node)
      authority == CompleteTipRecoveryAuthorityRecord(
                     parentContext, node, successorContext, application)
  IN /\ successorContext =
           CanonicalIndexedContext(parentContext.height + 1)
     /\ successorActivationStatus[parentContext][node] = "Running"
     /\ successorPredecessorStatusOwnership[parentContext][node]
          \in {"Published", "Absent"}
     /\ owner \in successorActivationFailures
     /\ ExactDurableParentApplication(parentContext, node, application)
     /\ successorActivationStatus' =
          [successorActivationStatus EXCEPT
             ![parentContext][node] = "Queued"]
     /\ successorPredecessorStatusOwnership' =
          [successorPredecessorStatusOwnership EXCEPT
             ![parentContext][node] = "Absent"]
     /\ successorActivationPrerequisites' =
          [successorActivationPrerequisites EXCEPT
             ![parentContext][node] = {}]
     /\ successorActivationTokens' =
          {token \in successorActivationTokens:
             \/ token.parentContext # parentContext
             \/ token.node # node}
     /\ successorRecoveryAuthorities' =
          {candidate \in successorRecoveryAuthorities:
             \/ candidate.parentContext # parentContext
             \/ candidate.node # node}
            \cup {authority}
     /\ preparedSuccessorActivationMarkers' =
          {marker \in preparedSuccessorActivationMarkers:
             \/ marker.parentContext # parentContext
             \/ marker.node # node}
     /\ successorActivationFailures' =
          successorActivationFailures \ {owner}
     /\ UNCHANGED <<publishedSuccessorActivationMarkers,
                     successorActivationFailureHistory,
                     successorActivationCompletions,
                     joinedByContext>>
     /\ SuccessorActivationEnvironmentStutter

AuthenticateRecoveredSuccessorActivation(parentContext, node,
                                          successorContext, application) ==
  LET owner == SuccessorActivationOwner(parentContext, node)
      token == SuccessorActivationToken(
                 "Recovered", parentContext, node, successorContext)
      authority == CompleteTipRecoveryAuthorityRecord(
                     parentContext, node, successorContext, application)
  IN /\ successorContext =
           CanonicalIndexedContext(parentContext.height + 1)
     /\ successorActivationStatus[parentContext][node] = "Queued"
     /\ successorPredecessorStatusOwnership[parentContext][node] = "Absent"
     /\ successorActivationPrerequisites[parentContext][node] = {}
     /\ owner \notin successorActivationFailures
     /\ ExactDurableParentApplication(parentContext, node, application)
     /\ authority \in successorRecoveryAuthorities
     /\ token \notin successorActivationTokens
     /\ successorActivationStatus' =
          [successorActivationStatus EXCEPT
             ![parentContext][node] = "Running"]
     /\ successorActivationTokens' =
          successorActivationTokens \cup {token}
     /\ UNCHANGED <<successorPredecessorStatusOwnership,
                     successorActivationPrerequisites,
                     successorRecoveryAuthorities,
                     preparedSuccessorActivationMarkers,
                     publishedSuccessorActivationMarkers,
                     successorActivationFailures,
                     successorActivationFailureHistory,
                     successorActivationCompletions,
                     joinedByContext>>
     /\ SuccessorActivationEnvironmentStutter

OpenDeferredSuccessorAdapter(parentContext, node, successorContext) ==
  /\ SuccessorActivationCredentialReady(
       parentContext, node, successorContext)
  /\ successorActivationPrerequisites[parentContext][node] = {}
  /\ successorActivationPrerequisites' =
       [successorActivationPrerequisites EXCEPT
          ![parentContext][node] = SuccessorActivationAdapterPrerequisites]
  /\ UNCHANGED <<successorActivationStatus,
                  successorPredecessorStatusOwnership,
                  successorActivationTokens,
                  successorRecoveryAuthorities,
                  preparedSuccessorActivationMarkers,
                  publishedSuccessorActivationMarkers,
                  successorActivationFailures,
                  successorActivationFailureHistory,
                  successorActivationCompletions,
                  joinedByContext>>
  /\ SuccessorActivationEnvironmentStutter

ConstructSuccessorRuntime(parentContext, node, successorContext) ==
  /\ SuccessorActivationCredentialReady(
       parentContext, node, successorContext)
  /\ successorActivationPrerequisites[parentContext][node]
       = SuccessorActivationAdapterPrerequisites
  /\ successorActivationPrerequisites' =
       [successorActivationPrerequisites EXCEPT
          ![parentContext][node] = SuccessorActivationRuntimePrerequisites]
  /\ UNCHANGED <<successorActivationStatus,
                  successorPredecessorStatusOwnership,
                  successorActivationTokens,
                  successorRecoveryAuthorities,
                  preparedSuccessorActivationMarkers,
                  publishedSuccessorActivationMarkers,
                  successorActivationFailures,
                  successorActivationFailureHistory,
                  successorActivationCompletions,
                  joinedByContext>>
  /\ SuccessorActivationEnvironmentStutter

StartSuccessorServices(parentContext, node, successorContext) ==
  /\ SuccessorActivationCredentialReady(
       parentContext, node, successorContext)
  /\ successorActivationPrerequisites[parentContext][node]
       = SuccessorActivationRuntimePrerequisites
  /\ successorActivationPrerequisites' =
       [successorActivationPrerequisites EXCEPT
          ![parentContext][node] = SuccessorActivationServicePrerequisites]
  /\ UNCHANGED <<successorActivationStatus,
                  successorPredecessorStatusOwnership,
                  successorActivationTokens,
                  successorRecoveryAuthorities,
                  preparedSuccessorActivationMarkers,
                  publishedSuccessorActivationMarkers,
                  successorActivationFailures,
                  successorActivationFailureHistory,
                  successorActivationCompletions,
                  joinedByContext>>
  /\ SuccessorActivationEnvironmentStutter

ApplySuccessorStartupEffects(parentContext, node, successorContext) ==
  /\ SuccessorActivationCredentialReady(
       parentContext, node, successorContext)
  /\ successorActivationPrerequisites[parentContext][node]
       = SuccessorActivationServicePrerequisites
  /\ successorActivationPrerequisites' =
       [successorActivationPrerequisites EXCEPT
          ![parentContext][node] = SuccessorActivationStartupPrerequisites]
  /\ UNCHANGED <<successorActivationStatus,
                  successorPredecessorStatusOwnership,
                  successorActivationTokens,
                  successorRecoveryAuthorities,
                  preparedSuccessorActivationMarkers,
                  publishedSuccessorActivationMarkers,
                  successorActivationFailures,
                  successorActivationFailureHistory,
                  successorActivationCompletions,
                  joinedByContext>>
  /\ SuccessorActivationEnvironmentStutter

ArmSuccessorClocks(parentContext, node, successorContext) ==
  /\ SuccessorActivationCredentialReady(
       parentContext, node, successorContext)
  /\ successorActivationPrerequisites[parentContext][node]
       = SuccessorActivationStartupPrerequisites
  /\ successorActivationPrerequisites' =
       [successorActivationPrerequisites EXCEPT
          ![parentContext][node] = SuccessorActivationClockPrerequisites]
  /\ UNCHANGED <<successorActivationStatus,
                  successorPredecessorStatusOwnership,
                  successorActivationTokens,
                  successorRecoveryAuthorities,
                  preparedSuccessorActivationMarkers,
                  publishedSuccessorActivationMarkers,
                  successorActivationFailures,
                  successorActivationFailureHistory,
                  successorActivationCompletions,
                  joinedByContext>>
  /\ SuccessorActivationEnvironmentStutter

PrepareSuccessorActivationMarker(parentContext, node, successorContext) ==
  LET marker == SuccessorActivationMarker(
                  parentContext, node, successorContext)
  IN /\ SuccessorActivationCredentialReady(
           parentContext, node, successorContext)
     /\ successorActivationPrerequisites[parentContext][node]
          = SuccessorActivationClockPrerequisites
     /\ marker \notin preparedSuccessorActivationMarkers
     /\ marker \notin publishedSuccessorActivationMarkers
     /\ preparedSuccessorActivationMarkers' =
          preparedSuccessorActivationMarkers \cup {marker}
     /\ UNCHANGED <<successorActivationStatus,
                     successorPredecessorStatusOwnership,
                     successorActivationPrerequisites,
                     successorActivationTokens,
                     successorRecoveryAuthorities,
                     publishedSuccessorActivationMarkers,
                     successorActivationFailures,
                     successorActivationFailureHistory,
                     successorActivationCompletions,
                     joinedByContext>>
     /\ SuccessorActivationEnvironmentStutter

OpenSuccessorIngress(parentContext, node, successorContext) ==
  LET marker == SuccessorActivationMarker(
                  parentContext, node, successorContext)
  IN /\ SuccessorActivationCredentialReady(
           parentContext, node, successorContext)
     /\ successorActivationPrerequisites[parentContext][node]
          = SuccessorActivationClockPrerequisites
     /\ marker \in preparedSuccessorActivationMarkers
     /\ successorActivationPrerequisites' =
          [successorActivationPrerequisites EXCEPT
             ![parentContext][node] =
               SuccessorActivationRequiredPrerequisites]
     /\ UNCHANGED <<successorActivationStatus,
                     successorPredecessorStatusOwnership,
                     successorActivationTokens,
                     successorRecoveryAuthorities,
                     preparedSuccessorActivationMarkers,
                     publishedSuccessorActivationMarkers,
                     successorActivationFailures,
                     successorActivationFailureHistory,
                     successorActivationCompletions,
                     joinedByContext>>
     /\ SuccessorActivationEnvironmentStutter

ActivateAppliedSuccessorHeight(parentContext, node, successorContext) ==
  LET token == SuccessorActivationToken(
                  "Applied", parentContext, node, successorContext)
      marker == SuccessorActivationMarker(
                   parentContext, node, successorContext)
  IN /\ SuccessorActivationCredentialReady(
           parentContext, node, successorContext)
     /\ ExactSuccessorActivationToken(
          "Applied", parentContext, node, successorContext)
     /\ successorPredecessorStatusOwnership[parentContext][node]
          = "Published"
     /\ successorActivationStatus[parentContext][node] = "Running"
     /\ successorActivationPrerequisites[parentContext][node]
          = SuccessorActivationRequiredPrerequisites
     /\ marker \in preparedSuccessorActivationMarkers
     /\ SuccessorActivationOwner(parentContext, node)
          \notin successorActivationFailures
     /\ successorActivationStatus' =
          [successorActivationStatus EXCEPT
             ![parentContext][node] = "Complete"]
     /\ successorPredecessorStatusOwnership' =
          [successorPredecessorStatusOwnership EXCEPT
             ![parentContext][node] = "Absent"]
     /\ successorActivationTokens' = successorActivationTokens \ {token}
     /\ preparedSuccessorActivationMarkers' =
          preparedSuccessorActivationMarkers \ {marker}
     /\ publishedSuccessorActivationMarkers' =
          publishedSuccessorActivationMarkers \cup {marker}
     /\ successorActivationCompletions' =
          successorActivationCompletions \cup {token}
     /\ joinedByContext' =
          [joinedByContext EXCEPT ![successorContext] = @ \cup {node}]
     /\ UNCHANGED <<successorActivationPrerequisites,
                     successorRecoveryAuthorities,
                     successorActivationFailures,
                     successorActivationFailureHistory>>
     /\ SuccessorActivationEnvironmentActivatesNode(
          successorContext, node)

ActivateRecoveredSuccessorHeight(parentContext, node, successorContext) ==
  LET token == SuccessorActivationToken(
                  "Recovered", parentContext, node, successorContext)
      marker == SuccessorActivationMarker(
                   parentContext, node, successorContext)
  IN /\ SuccessorActivationCredentialReady(
           parentContext, node, successorContext)
     /\ ExactSuccessorActivationToken(
          "Recovered", parentContext, node, successorContext)
     /\ successorPredecessorStatusOwnership[parentContext][node] = "Absent"
     /\ successorActivationStatus[parentContext][node] = "Running"
     /\ successorActivationPrerequisites[parentContext][node]
          = SuccessorActivationRequiredPrerequisites
     /\ marker \in preparedSuccessorActivationMarkers
     /\ SuccessorActivationOwner(parentContext, node)
          \notin successorActivationFailures
     /\ \E application \in Chain!DecisionEvidenceSet:
          ExactCompleteTipRecoveryAuthority(
            parentContext, node, successorContext, application)
     /\ UNCHANGED successorActivationStatus
     /\ successorActivationTokens' = successorActivationTokens \ {token}
     /\ successorRecoveryAuthorities' =
          {authority \in successorRecoveryAuthorities:
             \/ authority.parentContext # parentContext
             \/ authority.node # node}
     /\ preparedSuccessorActivationMarkers' =
          preparedSuccessorActivationMarkers \ {marker}
     /\ publishedSuccessorActivationMarkers' =
          publishedSuccessorActivationMarkers \cup {marker}
     /\ successorActivationCompletions' =
          successorActivationCompletions \cup {token}
     /\ joinedByContext' =
          [joinedByContext EXCEPT ![successorContext] = @ \cup {node}]
     /\ UNCHANGED <<successorPredecessorStatusOwnership,
                     successorActivationPrerequisites,
                     successorActivationFailures,
                     successorActivationFailureHistory>>
     /\ SuccessorActivationEnvironmentActivatesNode(
          successorContext, node)

SuccessorHeightActivated(parentContext, node) ==
  LET successorContext ==
        CanonicalIndexedContext(parentContext.height + 1)
      marker == SuccessorActivationMarker(
                  parentContext, node, successorContext)
  IN /\ parentContext.height < MaxHeight
     /\ successorPredecessorStatusOwnership[parentContext][node] = "Absent"
     /\ marker \in publishedSuccessorActivationMarkers
     /\ node \in joinedByContext[successorContext]
     /\ \/ /\ SuccessorActivationToken(
                  "Applied", parentContext, node, successorContext)
                  \in successorActivationCompletions
            /\ successorActivationStatus[parentContext][node] = "Complete"
        \/ /\ SuccessorActivationToken(
                  "Recovered", parentContext, node, successorContext)
                  \in successorActivationCompletions
            /\ successorActivationStatus[parentContext][node] = "Running"

IndexedSuccessorActivationProgressStep(parentContext, node) ==
  /\ SuccessorActivationShape
  /\ \E successorContext \in AdmissibleContextRecords:
       \/ BeginSuccessorActivation(parentContext, node, successorContext)
       \/ BindAppliedSuccessorActivationToken(
            parentContext, node, successorContext)
       \/ \E application \in Chain!DecisionEvidenceSet:
            \/ LatchRecoveredSuccessorStartupFailure(
                 parentContext, node, successorContext, application)
            \/ RehydrateCleanCompleteTipSuccessorStartup(
                 parentContext, node, successorContext, application)
            \/ RehydrateFailedSuccessorStartup(
                 parentContext, node, successorContext, application)
            \/ AuthenticateRecoveredSuccessorActivation(
                 parentContext, node, successorContext, application)
       \/ LatchAppliedSuccessorStartupFailure(parentContext, node)
       \/ OpenDeferredSuccessorAdapter(
            parentContext, node, successorContext)
       \/ ConstructSuccessorRuntime(parentContext, node, successorContext)
       \/ StartSuccessorServices(parentContext, node, successorContext)
       \/ ApplySuccessorStartupEffects(
            parentContext, node, successorContext)
       \/ ArmSuccessorClocks(parentContext, node, successorContext)
       \/ PrepareSuccessorActivationMarker(
            parentContext, node, successorContext)
       \/ OpenSuccessorIngress(parentContext, node, successorContext)
       \/ ActivateAppliedSuccessorHeight(
            parentContext, node, successorContext)
       \/ ActivateRecoveredSuccessorHeight(
            parentContext, node, successorContext)
  /\ SuccessorActivationShape'

SuccessorStartupFailureStep(parentContext, node) ==
  \/ LatchAppliedSuccessorStartupFailure(parentContext, node)
  \/ \E successorContext \in AdmissibleContextRecords,
       application \in Chain!DecisionEvidenceSet:
       LatchRecoveredSuccessorStartupFailure(
         parentContext, node, successorContext, application)

SuccessorStartupRestartStep(parentContext, node) ==
  \E successorContext \in AdmissibleContextRecords,
     application \in Chain!DecisionEvidenceSet:
    \/ RehydrateCleanCompleteTipSuccessorStartup(
         parentContext, node, successorContext, application)
    \/ RehydrateFailedSuccessorStartup(
         parentContext, node, successorContext, application)

SuccessorActivationAdvancingStep(parentContext, node) ==
  /\ IndexedSuccessorActivationProgressStep(parentContext, node)
  /\ ~SuccessorStartupFailureStep(parentContext, node)
  /\ ~SuccessorStartupRestartStep(parentContext, node)

(***************************************************************************
This is the runtime premise excluded by FLP-style reasoning: for every
responsive local owner there is a suffix with no further startup failure
transition. It does not bound or count failures before that suffix. Combined
with weak fairness of the terminating local activation worker, a rehydrated
attempt can traverse the finite startup pipeline.
***************************************************************************)
EventualFailureFreeSuccessorStartupSuffix ==
  \A parentContext \in AdmissibleContextRecords,
     node \in Responsive:
    <>[](SuccessorActivationOwner(parentContext, node)
           \notin successorActivationFailures)

FiniteHorizonSuccessorProjectionDormant ==
  \A terminalContext \in AdmissibleContextRecords,
     node \in ValidatorIds:
    terminalContext.height = MaxHeight
      => /\ successorActivationStatus[terminalContext][node] = "Idle"
         /\ successorPredecessorStatusOwnership[terminalContext][node]
              = "Absent"

(***************************************************************************
The indexed product deliberately excludes responsive-process crash/restart
transitions.  Every pre-created Async instance therefore remains in its
initialized recovery phase.  This state invariant is the exact reason that
the six responsive restart/replay weak-fairness clauses of AsyncSpecAt are
vacuous in the product projection; absence from the product action inventory
alone would not be sufficient without the recovery-control frame above.
***************************************************************************)
IndexedResponsiveRecoveryDormant ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedRecovery(initialContext, 1) = "Eligible"

SuccessorPublicationOrSuperseded(parentContext, node) ==
  \/ SuccessorHeightActivated(parentContext, node)
  \/ nodeHeight[node] > parentContext.height + 1

IndexedSuccessorActivationPending(parentContext, node) ==
  /\ parentContext \in AdmissibleContextRecords
  /\ node \in ValidatorIds
  /\ parentContext.height < MaxHeight
  /\ successorActivationStatus[parentContext][node]
       \in {"Queued", "Running"}
  /\ ~SuccessorPublicationOrSuperseded(parentContext, node)

IndexedSuccessorActivationProgress ==
  \A parentContext \in AdmissibleContextRecords,
     node \in Responsive:
    IndexedSuccessorActivationPending(parentContext, node)
      ~> SuccessorPublicationOrSuperseded(parentContext, node)

IndexedReceiptFreeAsyncAction(initialContext) ==
  /\ IndexedJoinedAsyncNext(initialContext)
  /\ NoNewIndexedDurableReceipt(initialContext)

IndexedFreshReceiptAsyncAction(initialContext) ==
  /\ IndexedJoinedAsyncNext(initialContext)
  /\ \/ \E decision \in Chain!DecisionEvidenceSet:
            NewIndexedDecisionReceipt(initialContext, decision)
     \/ \E application \in Chain!DecisionEvidenceSet:
            NewIndexedApplicationReceipt(initialContext, application)

IndexedProductActionAt(initialContext) ==
  /\ IndexedJoinedAsyncNext(initialContext)
  /\ \A otherContext \in AdmissibleContextRecords \ {initialContext}:
       UNCHANGED IndexedAsyncStateAt(otherContext)
  /\ IndexedAsyncStateShape'
  /\ JoinedByContextShape'
  /\ SuccessorActivationShape'
  /\ IndexedReceiptClassification(initialContext)

IndexedChainNext ==
  /\ IndexedAsyncStateShape
  /\ JoinedByContextShape
  /\ SuccessorActivationShape
  /\ \/ \E initialContext \in JoinedContexts:
          IndexedProductActionAt(initialContext)
     \/ \E parentContext \in AdmissibleContextRecords,
           node \in ValidatorIds:
          IndexedSuccessorActivationProgressStep(parentContext, node)

IndexedChainInit ==
  /\ IndexedAsyncStateShape
  /\ JoinedByContextShape
  /\ SuccessorActivationShape
  /\ \A initialContext \in AdmissibleContextRecords:
       IndexedAsync(initialContext)!AsyncInitAt(initialContext)
  /\ Chain!ChainEpochInit
  /\ joinedByContext =
       [initialContext \in AdmissibleContextRecords |->
          IF initialContext = GenesisContext
          THEN ValidatorIds
          ELSE {}]
  /\ successorActivationStatus =
       [parentContext \in AdmissibleContextRecords |->
          [node \in ValidatorIds |-> "Idle"]]
  /\ successorPredecessorStatusOwnership =
       [parentContext \in AdmissibleContextRecords |->
          [node \in ValidatorIds |-> "Absent"]]
  /\ successorActivationPrerequisites =
       [parentContext \in AdmissibleContextRecords |->
          [node \in ValidatorIds |-> {}]]
  /\ successorActivationTokens = {}
  /\ successorRecoveryAuthorities = {}
  /\ preparedSuccessorActivationMarkers = {}
  /\ publishedSuccessorActivationMarkers = {}
  /\ successorActivationFailures = {}
  /\ successorActivationFailureHistory = {}
  /\ successorActivationCompletions = {}
  /\ IndexedTotalReceiptProjection

(***************************************************************************
Fairness is attached to full indexed-product steps. Dormant contexts make each
action disabled. After the first independent join, the instance scheduler and
transport become fair. Node-attributed consensus work is fair after that node
joins, while direct Commit-certificate discovery is fair only for its current
context. Successor activation is weakly fair only for Responsive validators;
an honest validator outside Responsive may retain queued local work forever
without strengthening the conditional production liveness target.
***************************************************************************)
IndexedSetGstStep(initialContext) ==
  /\ IndexedChainNext
  /\ IndexedAsync(initialContext)!AsyncSetGST

IndexedTickStep(initialContext) ==
  /\ IndexedChainNext
  /\ IndexedAsync(initialContext)!AsyncTick

\* Bare AsyncTick is enabled before GST, including in a dormant pre-created
\* instance which the product intentionally cannot schedule.  Fixed-clock
\* historical recovery only consumes Tick after GST, so this is the exact
\* local action whose fairness can soundly refine the indexed product.
IndexedPostGstTick(initialContext) ==
  /\ IndexedAsync(initialContext)!gst
  /\ IndexedAsync(initialContext)!AsyncTick

IndexedRunNodeStep(initialContext, node) ==
  /\ IndexedChainNext
  /\ IndexedNodeCurrentAt(initialContext, node)
  /\ IndexedAsync(initialContext)!PostGstRunNode(node)

IndexedOpenHistoricalRecoveryStep(initialContext, node) ==
  /\ IndexedChainNext
  /\ \E server \in ValidatorIds,
       source \in Chain!DecisionEvidenceSet:
       IndexedOpenHistoricalRecovery(
         initialContext, node, server, source)

IndexedRunHistoricalRecoveryStep(initialContext, node) ==
  /\ IndexedChainNext
  /\ IndexedAsync(initialContext)!
       PostGstRunHistoricalRecoveryNode(node)

IndexedCommitCertificateDiscoveryStep(initialContext, node) ==
  /\ IndexedChainNext
  /\ IndexedNodeCurrentAt(initialContext, node)
  /\ IndexedAsync(initialContext)!
       PostGstCommitCertificateDiscovery(node)

IndexedHistoricalCommitCertificateDiscoveryStep(initialContext, node) ==
  /\ IndexedChainNext
  /\ IndexedAsync(initialContext)!
       PostGstHistoricalCommitCertificateDiscovery(node)

IndexedHistoricalServerStep(initialContext, node) ==
  /\ IndexedChainNext
  /\ node \in joinedByContext[initialContext]
  /\ IndexedAsync(initialContext)!PostGstRunHistoricalServer(node)

IndexedIoWorkerStep(initialContext, node) ==
  /\ IndexedChainNext
  /\ node \in joinedByContext[initialContext]
  /\ IndexedAsync(initialContext)!PostGstServiceIoWorker(node)

IndexedHistoricalRecoveryIoWorkerStep(initialContext, node) ==
  /\ IndexedChainNext
  /\ IndexedAsync(initialContext)!
       PostGstServiceHistoricalRecoveryIoWorker(node)

IndexedResolveLocalProducerContinuationStep(initialContext, node) ==
  /\ IndexedChainNext
  /\ IndexedNodeCurrentAt(initialContext, node)
  /\ IndexedAsync(initialContext)!
       PostGstResolveLocalCandidateProducerContinuation(node)

IndexedServiceConditionalProducerContinuationStep(initialContext, node) ==
  /\ IndexedChainNext
  /\ IndexedNodeCurrentAt(initialContext, node)
  /\ IndexedAsync(initialContext)!
       PostGstServiceConditionalTransportProducerContinuation(node)

IndexedServiceVolatileProducerContinuationStep(initialContext, node) ==
  /\ IndexedChainNext
  /\ IndexedNodeCurrentAt(initialContext, node)
  /\ IndexedAsync(initialContext)!
       PostGstServiceVolatileBodyProducerContinuation(node)

IndexedRetireLeaderWireLifecycleStep(initialContext, slot) ==
  /\ IndexedChainNext
  /\ IndexedAsync(initialContext)!
       PostGstRetireLeaderWireLifecycleSlot(slot)

IndexedAdmitPacketStep(initialContext, recipient, source) ==
  /\ IndexedChainNext
  /\ IndexedAsync(initialContext)!
       PostGstAdmitHiddenPacket(recipient, source)

IndexedAdmitHistoricalRecoveryPacketStep(
    initialContext, recipient, source) ==
  /\ IndexedChainNext
  /\ IndexedAsync(initialContext)!
       PostGstAdmitHistoricalRecoveryPacket(recipient, source)

IndexedFairness ==
  \A initialContext \in AdmissibleContextRecords:
    /\ WF_IndexedChainVars(IndexedSetGstStep(initialContext))
    /\ WF_IndexedChainVars(IndexedTickStep(initialContext))
    /\ \A node \in IndexedAsync(initialContext)!
                   AsyncVotersAt(initialContext):
         WF_IndexedChainVars(
           IndexedRunNodeStep(initialContext, node))
    /\ \A node \in Responsive:
         WF_IndexedChainVars(
           IndexedOpenHistoricalRecoveryStep(initialContext, node))
    /\ \A node \in Responsive:
         WF_IndexedChainVars(
           IndexedRunHistoricalRecoveryStep(initialContext, node))
    /\ \A node \in IndexedAsync(initialContext)!
                   AsyncVotersAt(initialContext):
         WF_IndexedChainVars(
           IndexedCommitCertificateDiscoveryStep(
             initialContext, node))
    /\ \A node \in Responsive:
         WF_IndexedChainVars(
           IndexedHistoricalCommitCertificateDiscoveryStep(
             initialContext, node))
    /\ \A node \in Responsive:
         WF_IndexedChainVars(
           IndexedHistoricalServerStep(initialContext, node))
    /\ \A node \in Responsive:
         WF_IndexedChainVars(
           IndexedIoWorkerStep(initialContext, node))
    /\ \A node \in Responsive:
         WF_IndexedChainVars(
           IndexedHistoricalRecoveryIoWorkerStep(
             initialContext, node))
    /\ \A node \in IndexedAsync(initialContext)!
                   AsyncVotersAt(initialContext):
         /\ WF_IndexedChainVars(
              IndexedResolveLocalProducerContinuationStep(
                initialContext, node))
         /\ WF_IndexedChainVars(
              IndexedServiceConditionalProducerContinuationStep(
                initialContext, node))
         /\ WF_IndexedChainVars(
              IndexedServiceVolatileProducerContinuationStep(
                initialContext, node))
    /\ \A slot \in IndexedAsync(initialContext)!
                   AsyncLeaderWireLifecycleSlotSet:
         WF_IndexedChainVars(
           IndexedRetireLeaderWireLifecycleStep(initialContext, slot))
    /\ \A recipient \in Responsive,
          source \in IndexedAsync(initialContext)!
                     AsyncIngressSources:
         WF_IndexedChainVars(
           IndexedAdmitPacketStep(initialContext, recipient, source))
    /\ \A recipient \in ValidatorIds,
          source \in IndexedAsync(initialContext)!AsyncIngressSources:
         WF_IndexedChainVars(
           IndexedAdmitHistoricalRecoveryPacketStep(
             initialContext, recipient, source))
    /\ \A node \in Responsive:
         WF_IndexedChainVars(
           IndexedSuccessorActivationProgressStep(
             initialContext, node))

IndexedChainSpec ==
  /\ IndexedChainInit
  /\ [][IndexedChainNext]_IndexedChainVars
  /\ IndexedFairness
  /\ EventualFailureFreeSuccessorStartupSuffix

(***************************************************************************
GST is an environmental liveness condition, never a consequence of a finite
process-generation budget.  The live transition surface stays identical to
the safety spec, while the live-only wrapper records the stated representative
deployment boundary.  Safety and bounded TLC configurations remain valid below
four peers because IndexedChainSpec itself is unchanged.  Aggregate height
induction receives the condition below as an explicit release premise;
per-context theorems expose its exact instance.
***************************************************************************)
IndexedLiveChainSpec ==
  /\ AsyncRepresentativeLiveConfiguration
  /\ IndexedChainSpec

IndexedGstEventuallyCondition ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedAsync(initialContext)!AsyncLiveSpecAt(initialContext)
      => <>IndexedCore(initialContext, 7)

THEOREM IndexedLiveChainSpecProjectsIndexedChainSpec ==
  IndexedLiveChainSpec => IndexedChainSpec
BY DEF IndexedLiveChainSpec

(***************************************************************************
`MaxHeight` is only the finite verification projection. A fresh exact Async
application receipt at that boundary is handed to `RecordKnownApplication`,
so the bounded successor projection stutters while preserving node
height/context and the activation tuple. Production has no terminal height,
kernel, or trace claim corresponding to this model-checking horizon.
***************************************************************************)
THEOREM TerminalContextCannotQueueSuccessorActivation ==
  \A terminalContext \in AdmissibleContextRecords,
     node \in ValidatorIds:
    terminalContext.height = MaxHeight
      => ~QueueSuccessorActivation(terminalContext, node)
BY DEF QueueSuccessorActivation

THEOREM TerminalExactApplicationPreservesHorizon ==
  \A terminalContext \in AdmissibleContextRecords,
     application \in Chain!DecisionEvidenceSet:
    terminalContext.height = MaxHeight
      /\ IndexedApplicationReceiptHandoff(terminalContext, application)
      => /\ nodeHeight'[application.node] = terminalContext.height
         /\ nodeContext'[application.node] = terminalContext
         /\ UNCHANGED SuccessorActivationVars
BY Isa DEF IndexedApplicationReceiptHandoff,
           ExactNodeLocationAt, QueueSuccessorActivation,
           Chain!RecordAppliedNext, Chain!RecordKnownApplication,
           SuccessorActivationVars

THEOREM IndexedInitEstablishesTerminalActivationExclusion ==
  IndexedChainInit => FiniteHorizonSuccessorProjectionDormant
BY Isa DEF IndexedChainInit, FiniteHorizonSuccessorProjectionDormant

THEOREM IndexedActionPreservesTerminalActivationExclusion ==
  FiniteHorizonSuccessorProjectionDormant
    /\ IndexedChainNext
    => FiniteHorizonSuccessorProjectionDormant'
BY Isa DEF FiniteHorizonSuccessorProjectionDormant,
           IndexedChainNext, IndexedProductActionAt,
           IndexedReceiptClassification,
           IndexedReceiptFreeChainStutter,
           IndexedDecisionReceiptHandoff,
           IndexedApplicationReceiptHandoff,
           QueueSuccessorActivation,
           IndexedSuccessorActivationProgressStep,
           BeginSuccessorActivation,
           BindAppliedSuccessorActivationToken,
           LatchAppliedSuccessorStartupFailure,
           LatchRecoveredSuccessorStartupFailure,
           RehydrateCleanCompleteTipSuccessorStartup,
           RehydrateFailedSuccessorStartup,
           AuthenticateRecoveredSuccessorActivation,
           OpenDeferredSuccessorAdapter,
           ConstructSuccessorRuntime,
           StartSuccessorServices,
           ApplySuccessorStartupEffects,
           ArmSuccessorClocks,
           PrepareSuccessorActivationMarker,
           OpenSuccessorIngress,
           ActivateAppliedSuccessorHeight,
           ActivateRecoveredSuccessorHeight

THEOREM IndexedStepPreservesTerminalActivationExclusion ==
  FiniteHorizonSuccessorProjectionDormant
    /\ [IndexedChainNext]_IndexedChainVars
    => FiniteHorizonSuccessorProjectionDormant'
BY Isa, IndexedActionPreservesTerminalActivationExclusion
   DEF IndexedChainVars, FiniteHorizonSuccessorProjectionDormant

THEOREM IndexedChainAlwaysExcludesTerminalActivation ==
  IndexedChainSpec => []FiniteHorizonSuccessorProjectionDormant
PROOF
  <1>1. IndexedChainInit => FiniteHorizonSuccessorProjectionDormant
    BY IndexedInitEstablishesTerminalActivationExclusion
  <1>2. FiniteHorizonSuccessorProjectionDormant
           /\ [IndexedChainNext]_IndexedChainVars
           => FiniteHorizonSuccessorProjectionDormant'
    BY IndexedStepPreservesTerminalActivationExclusion
  <1> QED BY <1>1, <1>2, PTL DEF IndexedChainSpec

(***************************************************************************
Composition invariant.

Every joined context is a canonical prefix no higher than the globally
certified prefix. Application advances the durable per-node chain first, then
queues a context-exact activation. A node joins the successor only after the
Applied or Recovered publication action records the exact activation marker.
Terminal historical recovery writes the exact instance application evidence
without a fictitious successor or node-height advance.
***************************************************************************)
IndexedEveryInstanceStrongInvariant ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedAsync(initialContext)!StrongInductiveInvariant

\* The scheduler-side service-activation pair is part of AsyncStrongType,
\* not the Core-only StrongInductiveInvariant above.  Retain the complete
\* typed Async boundary so component 46 and its paired deadlines can be
\* preserved through both ordinary product steps and final join actions.
IndexedEveryInstanceAsyncStrongTypeInvariant ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedAsync(initialContext)!AsyncStrongTypeInvariant

JoinedContextCertificationInvariant ==
  \A initialContext \in JoinedContexts:
    /\ initialContext =
         Chain!ContextRecord(initialContext.height,
                             Chain!HistoryThrough(initialContext.height))
    /\ initialContext.height <= certifiedHeight

IndexedJoinedThroughLocalHeight ==
  \A node \in ValidatorIds, blockHeight \in Heights:
    blockHeight <= nodeHeight[node]
      => /\ CanonicalIndexedContext(blockHeight)
               \in AdmissibleContextRecords
         /\ \/ node \in joinedByContext[
                        CanonicalIndexedContext(blockHeight)]
            \/ /\ blockHeight = nodeHeight[node]
               /\ blockHeight > 0
               /\ LET parentContext ==
                         CanonicalIndexedContext(blockHeight - 1)
                  IN /\ successorActivationStatus[parentContext][node]
                           \in {"Queued", "Running"}
                     /\ \E application \in Chain!DecisionEvidenceSet:
                          ExactDurableParentApplication(
                            parentContext, node, application)

JoinedRoutingInvariant ==
  \A initialContext \in AdmissibleContextRecords:
    \A node \in joinedByContext[initialContext]:
      \/ IndexedNodeCurrentAt(initialContext, node)
      \/ /\ nodeHeight[node] > initialContext.height
         /\ IndexedAsync(initialContext)!NodeHasApplication(node)

IndexedApplicationsRespectNodeHeight ==
  \A initialContext \in AdmissibleContextRecords:
    \A node \in Responsive:
      IndexedAsync(initialContext)!NodeHasApplication(node)
        => \/ initialContext.height = MaxHeight
           \/ nodeHeight[node] > initialContext.height

IndexedHistoricalRecoveryTargetCoherence ==
  \A initialContext \in AdmissibleContextRecords,
     node \in ValidatorIds:
    IndexedAsync(initialContext)!HistoricalRecoveryTarget(node)
      => /\ node \in Responsive
         /\ node \in joinedByContext[initialContext]
         /\ ExactNodeLocationAt(initialContext, node)
         /\ ~IndexedAsync(initialContext)!NodeHasApplication(node)

(***************************************************************************
Service activation is the exact scheduler-side mirror of independent joined
membership.  Dormant pre-created instances and genesis retain the canonical
unrestricted initializer.  The first non-genesis join burns the irreversible
restriction tombstone; thereafter the active set equals joined membership and
can grow only in the same atomic publication action.  The paired deadline
invariant prevents a zeroed inactive owner from entering any timed blocker.
***************************************************************************)
IndexedServiceActivationMembershipCoherenceAt(initialContext) ==
  IF IndexedAsync(initialContext)!AsyncServiceActivationRestricted
  THEN /\ joinedByContext[initialContext] # {}
       /\ IndexedAsync(initialContext)!AsyncActiveServiceNodes
            = joinedByContext[initialContext]
  ELSE /\ IndexedAsync(initialContext)!AsyncActiveServiceNodes
               = ValidatorIds
       /\ IF initialContext = GenesisContext
          THEN joinedByContext[initialContext] = ValidatorIds
          ELSE /\ joinedByContext[initialContext] = {}
               /\ IndexedAsync(initialContext)!
                    AsyncServiceActivationClockPristine

IndexedServiceActivationCoherence ==
  \A initialContext \in AdmissibleContextRecords:
    /\ IndexedAsync(initialContext)!AsyncServiceActivationPairInvariant
    /\ IndexedServiceActivationMembershipCoherenceAt(initialContext)

(***************************************************************************
GST cannot become true in a dormant pre-created instance.  AsyncSetGST is an
ordinary product action and IndexedChainNext selects such actions only from
JoinedContexts; successor activation may join a context but frames GST.  This
one-way coherence is the product-local enabledness bridge used by historical
post-GST fairness and does not require all responsive peers to have joined.
***************************************************************************)
IndexedPostGstContextJoinedCoherence ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedAsync(initialContext)!gst
      => initialContext \in JoinedContexts

\* `AsyncSetGST` requires the complete Responsive service roster to be active.
\* The restriction tombstone can only be installed while `~gst`, and every
\* later activation grows the active set.  Retain that exact executable guard
\* as reachable-state evidence instead of treating GST as enabled by one
\* joined owner.
IndexedPostGstResponsiveActiveRosterCoherence ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedAsync(initialContext)!gst
      => Responsive \subseteq
           IndexedAsync(initialContext)!AsyncActiveServiceNodes

IndexedTerminalExactApplicationBoundaryInvariant ==
  \A terminalContext \in AdmissibleContextRecords,
     node \in Responsive:
    terminalContext.height = MaxHeight
      /\ IndexedAsync(terminalContext)!NodeHasApplication(node)
      => ExactNodeLocationAt(terminalContext, node)

IndexedCompositionInvariant ==
  /\ IndexedAsyncStateShape
  /\ JoinedByContextShape
  /\ SuccessorActivationShape
  /\ Chain!ChainEpochInvariant
  /\ IndexedTotalReceiptProjection
  /\ IndexedEveryInstanceStrongInvariant
  /\ IndexedEveryInstanceAsyncStrongTypeInvariant
  /\ JoinedContextCertificationInvariant
  /\ JoinedRoutingInvariant
  /\ IndexedApplicationsRespectNodeHeight
  /\ IndexedHistoricalRecoveryTargetCoherence
  /\ IndexedServiceActivationCoherence
  /\ IndexedPostGstContextJoinedCoherence
  /\ IndexedPostGstResponsiveActiveRosterCoherence
  /\ IndexedTerminalExactApplicationBoundaryInvariant

(***************************************************************************
Non-temporal composition and refinement kernels.
***************************************************************************)
THEOREM AdmissibleContextDomainIsFinite ==
  ModelConfiguration => IsFiniteSet(AdmissibleContextRecords)
BY Isa DEF AdmissibleContextRecords, FrozenContextAdmissible,
           ContextRecords, LineagesAt, Heights, Subjects

THEOREM IndexedInstanceVariablesAreExact ==
  IndexedAsyncStateShape
    => \A initialContext \in AdmissibleContextRecords:
         IndexedAsync(initialContext)!AsyncAllVars =
           IndexedAsyncStateAt(initialContext)
BY Isa DEF IndexedAsyncStateShape, IndexedAsyncStateAt,
           IndexedAsync!AsyncAllVars,
           IndexedAsync!AsyncSchedulerVars,
           IndexedAsync!AsyncRecoveryVars,
           IndexedAsync!AsyncProducerVars,
           IndexedAsync!vars,
           IndexedDuplicatedGst, IndexedCore, IndexedScheduler,
           IndexedRecovery, IndexedProducer,
           IndexedFixedCorridorDeadlines

(***************************************************************************
Exact indexed field-order pins.

Arity alone is insufficient at this boundary: an insertion in Core or the
scheduler can leave every tuple well typed while shifting a later durable or
fairness owner onto the wrong state component.  These extensional equalities
pin the duplicated GST scalar, all 49 Core fields, all 46 scheduler fields,
the five recovery fields, all three producer-journal fields, and the proof-only
fixed-corridor receipt.
***************************************************************************)
THEOREM IndexedDuplicatedGstProjectionIsExact ==
  IndexedAsyncStateShape
    => \A initialContext \in AdmissibleContextRecords:
         /\ IndexedDuplicatedGst(initialContext)
              = IndexedCore(initialContext, 7)
         /\ IndexedAsync(initialContext)!gst
              = IndexedDuplicatedGst(initialContext)
BY DEF IndexedAsyncStateShape, IndexedDuplicatedGst, IndexedCore

THEOREM IndexedFortyNineFieldCoreProjectionIsExact ==
  IndexedAsyncStateShape
    => \A initialContext \in AdmissibleContextRecords:
         IndexedAsync(initialContext)!vars =
           <<IndexedCore(initialContext, 1),
             IndexedCore(initialContext, 2),
             IndexedCore(initialContext, 3),
             IndexedCore(initialContext, 4),
             IndexedCore(initialContext, 5),
             IndexedCore(initialContext, 6),
             IndexedCore(initialContext, 7),
             IndexedCore(initialContext, 8),
             IndexedCore(initialContext, 9),
             IndexedCore(initialContext, 10),
             IndexedCore(initialContext, 11),
             IndexedCore(initialContext, 12),
             IndexedCore(initialContext, 13),
             IndexedCore(initialContext, 14),
             IndexedCore(initialContext, 15),
             IndexedCore(initialContext, 16),
             IndexedCore(initialContext, 17),
             IndexedCore(initialContext, 18),
             IndexedCore(initialContext, 19),
             IndexedCore(initialContext, 20),
             IndexedCore(initialContext, 21),
             IndexedCore(initialContext, 22),
             IndexedCore(initialContext, 23),
             IndexedCore(initialContext, 24),
             IndexedCore(initialContext, 25),
             IndexedCore(initialContext, 26),
             IndexedCore(initialContext, 27),
             IndexedCore(initialContext, 28),
             IndexedCore(initialContext, 29),
             IndexedCore(initialContext, 30),
             IndexedCore(initialContext, 31),
             IndexedCore(initialContext, 32),
             IndexedCore(initialContext, 33),
             IndexedCore(initialContext, 34),
             IndexedCore(initialContext, 35),
             IndexedCore(initialContext, 36),
             IndexedCore(initialContext, 37),
             IndexedCore(initialContext, 38),
             IndexedCore(initialContext, 39),
             IndexedCore(initialContext, 40),
             IndexedCore(initialContext, 41),
             IndexedCore(initialContext, 42),
             IndexedCore(initialContext, 43),
             IndexedCore(initialContext, 44),
             IndexedCore(initialContext, 45),
             IndexedCore(initialContext, 46),
             IndexedCore(initialContext, 47),
             IndexedCore(initialContext, 48),
             IndexedCore(initialContext, 49)>>
BY DEF IndexedAsync!vars, IndexedCore

THEOREM IndexedFortySixFieldSchedulerProjectionIsExact ==
  IndexedAsyncStateShape
    => \A initialContext \in AdmissibleContextRecords:
         IndexedAsync(initialContext)!AsyncSchedulerVars =
           <<IndexedScheduler(initialContext, 1),
             IndexedScheduler(initialContext, 2),
             IndexedScheduler(initialContext, 3),
             IndexedScheduler(initialContext, 4),
             IndexedScheduler(initialContext, 5),
             IndexedScheduler(initialContext, 6),
             IndexedScheduler(initialContext, 7),
             IndexedScheduler(initialContext, 8),
             IndexedScheduler(initialContext, 9),
             IndexedScheduler(initialContext, 10),
             IndexedScheduler(initialContext, 11),
             IndexedScheduler(initialContext, 12),
             IndexedScheduler(initialContext, 13),
             IndexedScheduler(initialContext, 14),
             IndexedScheduler(initialContext, 15),
             IndexedScheduler(initialContext, 16),
             IndexedScheduler(initialContext, 17),
             IndexedScheduler(initialContext, 18),
             IndexedScheduler(initialContext, 19),
             IndexedScheduler(initialContext, 20),
             IndexedScheduler(initialContext, 21),
             IndexedScheduler(initialContext, 22),
             IndexedScheduler(initialContext, 23),
             IndexedScheduler(initialContext, 24),
             IndexedScheduler(initialContext, 25),
             IndexedScheduler(initialContext, 26),
             IndexedScheduler(initialContext, 27),
             IndexedScheduler(initialContext, 28),
             IndexedScheduler(initialContext, 29),
             IndexedScheduler(initialContext, 30),
             IndexedScheduler(initialContext, 31),
             IndexedScheduler(initialContext, 32),
             IndexedScheduler(initialContext, 33),
             IndexedScheduler(initialContext, 34),
             IndexedScheduler(initialContext, 35),
             IndexedScheduler(initialContext, 36),
             IndexedScheduler(initialContext, 37),
             IndexedScheduler(initialContext, 38),
             IndexedScheduler(initialContext, 39),
             IndexedScheduler(initialContext, 40),
             IndexedScheduler(initialContext, 41),
             IndexedScheduler(initialContext, 42),
             IndexedScheduler(initialContext, 43),
             IndexedScheduler(initialContext, 44),
             IndexedScheduler(initialContext, 45),
             IndexedScheduler(initialContext, 46)>>
BY DEF IndexedAsync!AsyncSchedulerVars, IndexedScheduler

THEOREM IndexedFixedCorridorDeadlineProjectionIsExact ==
  IndexedAsyncStateShape
    => \A initialContext \in AdmissibleContextRecords:
         IndexedAsync(initialContext)!asyncFixedCorridorDeadlines
           = IndexedFixedCorridorDeadlines(initialContext)
BY DEF IndexedFixedCorridorDeadlines

(***************************************************************************
The producer journal is part of the authoritative transition state.  These
equalities prevent indexed contexts from aliasing a hidden global journal and
pin the known-obligation, consumed-episode, and origin-history order used by
the finite producer ranks.
***************************************************************************)
THEOREM IndexedThreeFieldProducerProjectionIsExact ==
  IndexedAsyncStateShape
    => \A initialContext \in AdmissibleContextRecords:
         /\ IndexedAsync(initialContext)!AsyncProducerVars =
              indexedAsyncState[initialContext][5]
         /\ indexedAsyncState[initialContext][5] =
              <<IndexedProducer(initialContext, 1),
                IndexedProducer(initialContext, 2),
                IndexedProducer(initialContext, 3)>>
BY Isa DEF IndexedAsyncStateShape,
           IndexedAsync!AsyncProducerVars, IndexedProducer

THEOREM VerificationThreeFieldProducerProjectionIsExact ==
  IndexedAsyncStateShape
    /\ VerificationContext \in AdmissibleContextRecords
    => /\ VerificationAsyncProof!AsyncProducerVars =
             indexedAsyncState[VerificationContext][5]
       /\ indexedAsyncState[VerificationContext][5] =
            <<VerificationProducer(1), VerificationProducer(2),
              VerificationProducer(3)>>
BY Isa DEF IndexedAsyncStateShape,
           VerificationAsyncProof!AsyncProducerVars,
           VerificationProducer, IndexedProducer

THEOREM VerificationInstanceVariablesAreExact ==
  /\ IndexedAsyncStateShape
  /\ VerificationContext \in AdmissibleContextRecords
  => VerificationAsyncProof!AsyncAllVars =
       IndexedAsyncStateAt(VerificationContext)
BY Isa
   DEF IndexedAsyncStateShape, IndexedAsyncStateAt,
       VerificationAsyncProof!AsyncAllVars,
       VerificationAsyncProof!AsyncSchedulerVars,
       VerificationAsyncProof!AsyncRecoveryVars,
       VerificationAsyncProof!AsyncProducerVars,
       VerificationAsyncProof!vars,
       VerificationCore, VerificationScheduler, VerificationRecovery,
       VerificationProducer,
       VerificationFixedCorridorDeadlines,
       IndexedDuplicatedGst, IndexedCore, IndexedScheduler,
       IndexedRecovery, IndexedProducer,
       IndexedFixedCorridorDeadlines

(***************************************************************************
The seven Serve lifecycle fields are pinned separately from the aggregate
scheduler tuple.  This prevents an arity-correct WITH clause from silently
dropping the retained-attempt field at index 17, shifting every later owner,
and thereby erasing immutable admission, tombstone, or retry-coalescing state
from the indexed liveness product.
***************************************************************************)
THEOREM IndexedSevenFieldServeLifecycleProjectionIsExact ==
  IndexedAsyncStateShape
    => \A initialContext \in AdmissibleContextRecords:
         /\ IndexedAsync(initialContext)!AsyncServeLifecycleVars =
              <<IndexedScheduler(initialContext, 11),
                IndexedScheduler(initialContext, 14),
                IndexedScheduler(initialContext, 15),
                IndexedScheduler(initialContext, 16),
                IndexedScheduler(initialContext, 17)>>
         /\ IndexedAsync(initialContext)!AsyncServeIngressAdmissionVars =
              <<IndexedScheduler(initialContext, 12),
                IndexedScheduler(initialContext, 13)>>
BY Isa DEF IndexedAsyncStateShape,
           IndexedAsync!AsyncServeLifecycleVars,
           IndexedAsync!AsyncServeIngressAdmissionVars,
           IndexedScheduler

THEOREM VerificationSevenFieldServeLifecycleProjectionIsExact ==
  /\ IndexedAsyncStateShape
  /\ VerificationContext \in AdmissibleContextRecords
  => /\ VerificationAsyncProof!AsyncServeLifecycleVars =
           <<VerificationScheduler(11), VerificationScheduler(14),
             VerificationScheduler(15), VerificationScheduler(16),
             VerificationScheduler(17)>>
     /\ VerificationAsyncProof!AsyncServeIngressAdmissionVars =
           <<VerificationScheduler(12), VerificationScheduler(13)>>
BY Isa DEF IndexedAsyncStateShape,
           VerificationAsyncProof!AsyncServeLifecycleVars,
           VerificationAsyncProof!AsyncServeIngressAdmissionVars,
           VerificationScheduler, IndexedScheduler

(***************************************************************************
The appended service-activation record is pinned independently.  This keeps
all reviewed scheduler indices 1..45 stable while preventing an arity-correct
instance from dropping or aliasing the irreversible restriction tombstone.
***************************************************************************)
THEOREM IndexedServiceActivationProjectionIsExact ==
  IndexedAsyncStateShape
    => \A initialContext \in AdmissibleContextRecords:
       IndexedAsync(initialContext)!AsyncSchedulerVars[46]
           = IndexedScheduler(initialContext, 46)
BY DEF IndexedAsync!AsyncSchedulerVars, IndexedScheduler

THEOREM VerificationServiceActivationProjectionIsExact ==
  /\ IndexedAsyncStateShape
  /\ VerificationContext \in AdmissibleContextRecords
  => VerificationAsyncProof!AsyncSchedulerVars[46]
       = VerificationScheduler(46)
BY DEF VerificationAsyncProof!AsyncSchedulerVars,
       VerificationScheduler, IndexedScheduler

THEOREM IndexedLeaderWireLifecycleProjectionIsExact ==
  IndexedAsyncStateShape
    => \A initialContext \in AdmissibleContextRecords:
         IndexedAsync(initialContext)!asyncLeaderWireLifecycles
           = IndexedScheduler(initialContext, 42)
BY DEF IndexedScheduler

THEOREM VerificationLeaderWireLifecycleProjectionIsExact ==
  /\ IndexedAsyncStateShape
  /\ VerificationContext \in AdmissibleContextRecords
  => VerificationAsyncProof!asyncLeaderWireLifecycles
       = VerificationScheduler(42)
BY DEF VerificationScheduler, IndexedScheduler

(***************************************************************************
The recovery projection is extensional, not merely length-compatible.  These
facts pin the five production fields at the chain-composition boundary and
prevent a future WITH-clause edit from silently dropping, duplicating, or
reordering recovery phase, owner, generation, replay-queue state, or the
historical-lock restart authority.
***************************************************************************)
THEOREM IndexedFiveFieldRecoveryProjectionIsExact ==
  IndexedAsyncStateShape
    => \A initialContext \in AdmissibleContextRecords:
         /\ IndexedAsync(initialContext)!AsyncRecoveryVars =
              indexedAsyncState[initialContext][4]
         /\ indexedAsyncState[initialContext][4] =
              <<IndexedRecovery(initialContext, 1),
                IndexedRecovery(initialContext, 2),
                IndexedRecovery(initialContext, 3),
                IndexedRecovery(initialContext, 4),
                IndexedRecovery(initialContext, 5)>>
BY Isa DEF IndexedAsyncStateShape,
           IndexedAsync!AsyncRecoveryVars, IndexedRecovery

THEOREM VerificationFiveFieldRecoveryProjectionIsExact ==
  IndexedAsyncStateShape
    /\ VerificationContext \in AdmissibleContextRecords
    => /\ VerificationAsyncProof!AsyncRecoveryVars =
             indexedAsyncState[VerificationContext][4]
       /\ indexedAsyncState[VerificationContext][4] =
            <<VerificationRecovery(1), VerificationRecovery(2),
              VerificationRecovery(3), VerificationRecovery(4),
              VerificationRecovery(5)>>
BY Isa DEF IndexedAsyncStateShape,
           VerificationAsyncProof!AsyncRecoveryVars,
           VerificationRecovery, IndexedRecovery

THEOREM IndexedHistoricalRecoveryTargetProjectionIsExact ==
  IndexedAsyncStateShape
    => \A initialContext \in AdmissibleContextRecords:
         \A node \in ValidatorIds:
           IndexedAsync(initialContext)!HistoricalRecoveryTarget(node)
             <=> node \in IndexedScheduler(initialContext, 44)
BY DEF IndexedAsync!HistoricalRecoveryTarget

THEOREM VerificationHistoricalRecoveryTargetProjectionIsExact ==
  IndexedAsyncStateShape
    /\ VerificationContext \in AdmissibleContextRecords
    => \A node \in ValidatorIds:
         VerificationAsyncProof!HistoricalRecoveryTarget(node)
           <=> node \in VerificationScheduler(44)
BY DEF VerificationAsyncProof!HistoricalRecoveryTarget

THEOREM IndexedInitProjectsEveryAsyncInit ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedChainInit =>
      IndexedAsync(initialContext)!AsyncInitAt(initialContext)
BY DEF IndexedChainInit

(***************************************************************************
This fact is derived from the exact InitAt payload, rather than from the
ChainEpoch equality in IndexedChainInit. Non-genesis instances retain their
synthetic parent receipt internally, but its context/height is strictly below
the frozen instance and hence absent from both projected current-receipt sets.
***************************************************************************)
THEOREM IndexedAsyncInitHasNoCurrentReceipts ==
  (IndexedAsyncStateShape
    /\ \A initialContext \in AdmissibleContextRecords:
         IndexedAsync(initialContext)!AsyncInitAt(initialContext))
    => /\ IndexedDecisionEvidence = {}
       /\ IndexedApplicationEvidence = {}
BY Isa DEF IndexedDecisionEvidence, IndexedApplicationEvidence,
           IndexedCurrentDecisions, IndexedCurrentApplications,
           IndexedDecisions, IndexedApplications,
           IndexedAsync!AsyncInitAt, IndexedAsync!AsyncBaseInitAt,
           IndexedAsync!InitAt, IndexedAsync!BootstrapParentDecision

THEOREM IndexedChainInitHasEmptyCurrentReceiptUnion ==
  IndexedChainInit
    => /\ IndexedDecisionEvidence = {}
       /\ IndexedApplicationEvidence = {}
BY IndexedAsyncInitHasNoCurrentReceipts DEF IndexedChainInit

THEOREM JoinedRunnerIsExactAsyncWork ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedJoinedRunnerStep(initialContext)
      => IndexedAsync(initialContext)!AsyncRunnerStep
BY Isa DEF IndexedJoinedRunnerStep,
           IndexedAsync!AsyncRunnerStep,
           IndexedAsync!RunHistoricalRecoveryNode,
           IndexedAsync!HistoricalRecoveryTarget,
           IndexedAsync!RunHistoricalServer

THEOREM JoinedNonRunnerIsExactAsyncWork ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedJoinedNonRunnerStep(initialContext)
      => IndexedAsync(initialContext)!AsyncNonRunnerStep
BY Isa DEF IndexedJoinedNonRunnerStep,
           IndexedAsync!AsyncNonRunnerStep,
           IndexedOpenHistoricalRecovery,
           IndexedAsync!DirectHistoricalCommitCertificateDiscoveryStep,
           IndexedAsync!HistoricalCommitCertificateDiscoveryDue,
           IndexedAsync!ServiceIoWorker,
           IndexedAsync!ServiceHistoricalRecoveryIoWorker,
           IndexedAsync!EnqueueHistoricalRecoveryIoLocalControl,
           IndexedAsync!HistoricalRecoveryTarget

THEOREM JoinedAsyncStepRefinesExactAsyncStep ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedJoinedAsyncNext(initialContext)
      => IndexedAsync(initialContext)!AsyncNext
BY Isa, JoinedRunnerIsExactAsyncWork,
   JoinedNonRunnerIsExactAsyncWork
   DEF IndexedJoinedAsyncNext, IndexedJoinedNonCrashStep,
       IndexedAsync!AsyncNext, IndexedAsync!AsyncNonCrashStep,
       IndexedAsync!AsyncProducerProjectionStep,
       IndexedAsync!AsyncServiceActivationTransition,
       IndexedScheduler

(***************************************************************************
Responsive restart/replay is intentionally outside the indexed chain product.
The normal joined branch now carries the complete production non-crash
recovery-control frame, and the remaining non-responsive crash branch already
frames AsyncRecoveryVars.  Final successor publication changes only the exact
successor instance's internal service-activation state and paired deadlines;
every recovery component still stutters.  These facts make the initialized
Eligible phase inductive without silently adding a favourable restart relation.
***************************************************************************)
THEOREM IndexedInitEstablishesResponsiveRecoveryDormancy ==
  IndexedChainInit => IndexedResponsiveRecoveryDormant
BY Isa DEF IndexedChainInit, IndexedResponsiveRecoveryDormant,
           IndexedAsync!AsyncInitAt, IndexedAsync!AsyncBaseInitAt,
           IndexedAsync!AsyncRecoveryInit, IndexedRecovery

THEOREM IndexedJoinedAsyncStepPreservesResponsiveRecoveryEligibility ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedRecovery(initialContext, 1) = "Eligible"
      /\ IndexedJoinedAsyncNext(initialContext)
      => IndexedRecovery(initialContext, 1)' = "Eligible"
BY Isa DEF IndexedJoinedAsyncNext, IndexedJoinedNonCrashStep,
           IndexedAsync!PreGstCrash,
           IndexedAsync!AsyncRecoveryVars,
           IndexedAsync!AsyncRecoveryControlVars,
           IndexedRecovery

THEOREM IndexedProductActionPreservesResponsiveRecoveryDormancy ==
  \A selectedContext \in AdmissibleContextRecords:
    IndexedResponsiveRecoveryDormant
      /\ IndexedProductActionAt(selectedContext)
      => IndexedResponsiveRecoveryDormant'
BY Isa, IndexedJoinedAsyncStepPreservesResponsiveRecoveryEligibility
   DEF IndexedResponsiveRecoveryDormant, IndexedProductActionAt,
       IndexedAsyncStateAt, IndexedRecovery

THEOREM IndexedSuccessorActivationStepPreservesRecoveryState ==
  \A parentContext \in AdmissibleContextRecords,
     node \in ValidatorIds:
    IndexedSuccessorActivationProgressStep(parentContext, node)
      => \A initialContext \in AdmissibleContextRecords:
           UNCHANGED indexedAsyncState[initialContext][4]
BY Isa DEF IndexedSuccessorActivationProgressStep,
           BeginSuccessorActivation,
           BindAppliedSuccessorActivationToken,
           LatchAppliedSuccessorStartupFailure,
           LatchRecoveredSuccessorStartupFailure,
           RehydrateCleanCompleteTipSuccessorStartup,
           RehydrateFailedSuccessorStartup,
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
           SuccessorActivationEnvironmentActivatesNode,
           IndexedAsync!AsyncEnterIndexedServiceActivation,
           IndexedAsync!AsyncActivateServiceNode,
           IndexedAsync!AsyncServiceActivationFrameVars,
           IndexedAsync!AsyncRecoveryVars,
           IndexedAsyncStateAt, IndexedRecovery

THEOREM IndexedActionPreservesResponsiveRecoveryDormancy ==
  IndexedResponsiveRecoveryDormant /\ IndexedChainNext
    => IndexedResponsiveRecoveryDormant'
BY Isa, IndexedProductActionPreservesResponsiveRecoveryDormancy,
   IndexedSuccessorActivationStepPreservesRecoveryState
   DEF IndexedChainNext, JoinedContexts,
       IndexedResponsiveRecoveryDormant,
       IndexedRecovery

THEOREM IndexedStepPreservesResponsiveRecoveryDormancy ==
  IndexedResponsiveRecoveryDormant
    /\ [IndexedChainNext]_IndexedChainVars
    => IndexedResponsiveRecoveryDormant'
BY Isa, IndexedActionPreservesResponsiveRecoveryDormancy
   DEF IndexedChainVars, IndexedResponsiveRecoveryDormant,
       IndexedRecovery

THEOREM IndexedChainSpecKeepsResponsiveRecoveryDormant ==
  IndexedChainSpec => []IndexedResponsiveRecoveryDormant
PROOF
  <1>1. IndexedChainInit => IndexedResponsiveRecoveryDormant
    BY IndexedInitEstablishesResponsiveRecoveryDormancy
  <1>2. IndexedResponsiveRecoveryDormant
           /\ [IndexedChainNext]_IndexedChainVars
           => IndexedResponsiveRecoveryDormant'
    BY IndexedStepPreservesResponsiveRecoveryDormancy
  <1> QED BY <1>1, <1>2, PTL DEF IndexedChainSpec

(***************************************************************************
Local historical-service activation bridge.

Post-GST work needs only its own joined context and owner; it does not wait
for every Responsive peer.  A timed service owner is active by construction.
Component-44 coherence therefore maps that exact owner to the monotonically
joined product membership.  Historical recovery targets already have the
stronger routing witness in the composition invariant.
***************************************************************************)
THEOREM IndexedPostGstContextHasJoinedProductInstance ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedCompositionInvariant
      /\ IndexedAsync(initialContext)!gst
      => initialContext \in JoinedContexts
BY DEF IndexedCompositionInvariant,
       IndexedPostGstContextJoinedCoherence

THEOREM IndexedPostGstActiveServiceOwnerHasJoinedProductInstance ==
  \A initialContext \in AdmissibleContextRecords,
     node \in ValidatorIds:
    /\ IndexedCompositionInvariant
    /\ IndexedAsync(initialContext)!gst
    /\ node \in IndexedAsync(initialContext)!AsyncActiveServiceNodes
    => /\ initialContext \in JoinedContexts
       /\ node \in joinedByContext[initialContext]
BY Isa, IndexedPostGstContextHasJoinedProductInstance
   DEF IndexedCompositionInvariant,
       IndexedServiceActivationCoherence,
       IndexedServiceActivationMembershipCoherenceAt,
       IndexedAsync!AsyncServiceActivationRestricted,
       IndexedAsync!AsyncActiveServiceNodes

THEOREM IndexedHistoricalRecoveryTargetHasJoinedActiveOwner ==
  \A initialContext \in AdmissibleContextRecords,
     node \in ValidatorIds:
    /\ IndexedCompositionInvariant
    /\ IndexedAsync(initialContext)!gst
    /\ IndexedAsync(initialContext)!HistoricalRecoveryTarget(node)
    => /\ initialContext \in JoinedContexts
       /\ node \in Responsive
       /\ node \in joinedByContext[initialContext]
       /\ node \in IndexedAsync(initialContext)!AsyncActiveServiceNodes
BY Isa, IndexedPostGstContextHasJoinedProductInstance
   DEF IndexedCompositionInvariant,
       IndexedHistoricalRecoveryTargetCoherence,
       IndexedServiceActivationCoherence,
       IndexedServiceActivationMembershipCoherenceAt,
       IndexedAsync!AsyncServiceActivationRestricted,
       IndexedAsync!AsyncActiveServiceNodes

THEOREM JoinedNodeNeverWaitsForAllPeers ==
  \A initialContext \in AdmissibleContextRecords:
    \A node \in IndexedAsync(initialContext)!
                  AsyncCurrentResponsiveVoters:
      (/\ IndexedServiceActivationCoherence
       /\ IndexedNodeCurrentAt(initialContext, node)
       /\ IndexedAsync(initialContext)!RunNode(node))
        => /\ node \in IndexedAsync(initialContext)!
                       AsyncActiveServiceNodes
           /\ IndexedJoinedRunnerStep(initialContext)
BY Isa
   DEF IndexedServiceActivationCoherence,
       IndexedServiceActivationMembershipCoherenceAt,
       IndexedNodeCurrentAt, IndexedJoinedRunnerStep,
       IndexedAsync!AsyncServiceActivationRestricted,
       IndexedAsync!AsyncActiveServiceNodes

THEOREM HistoricalServiceSurvivesLocalAdvance ==
  \A initialContext \in AdmissibleContextRecords:
    \A node \in IndexedAsync(initialContext)!
                  AsyncCurrentResponsiveVoters:
      (/\ node \in joinedByContext[initialContext]
       /\ IndexedAsync(initialContext)!RunHistoricalServer(node))
        => IndexedJoinedRunnerStep(initialContext)
BY DEF IndexedJoinedRunnerStep

(***************************************************************************
Historical recovery always copies the source QC exactly.  Slot identity is
split at the finite horizon: nonterminal contexts name the canonical successor
slot, while a terminal context can only carry outside-horizon receipt identity.
There is no DecisionSlots member at MaxHeight + 1.
***************************************************************************)

THEOREM HistoricalRecoveryOpenCopiesExactIdentity ==
  \A initialContext \in AdmissibleContextRecords,
     node \in ValidatorIds, server \in ValidatorIds,
     source \in Chain!DecisionEvidenceSet:
    IndexedOpenHistoricalRecovery(initialContext, node, server, source)
      => /\ HistoricalRecoveryRecord(node, source).qc.context
                = initialContext
         /\ HistoricalRecoveryRecord(node, source).qc = source.qc
         /\ HistoricalRecoveryRecord(node, source).qc.subject
                = source.qc.subject
BY DEF IndexedOpenHistoricalRecovery,
       IndexedHistoricalRecoverySourceReady, HistoricalRecoveryRecord,
       IndexedCurrentDecisions

THEOREM NonterminalHistoricalRecoveryCopiesCanonicalSlotIdentity ==
  \A initialContext \in AdmissibleContextRecords,
     node \in ValidatorIds, server \in ValidatorIds,
     source \in Chain!DecisionEvidenceSet:
    initialContext.height < MaxHeight
      /\ IndexedOpenHistoricalRecovery(
           initialContext, node, server, source)
      => /\ HistoricalRecoveryRecord(node, source).qc.context
                = initialContext
         /\ HistoricalRecoveryRecord(node, source).qc = source.qc
         /\ Chain!CanonicalCommitForSlot(
              HistoricalRecoveryRecord(node, source).qc,
              initialContext.height + 1)
BY Isa DEF IndexedOpenHistoricalRecovery,
           IndexedHistoricalRecoverySourceReady, HistoricalRecoveryRecord,
           IndexedCurrentDecisions

THEOREM TerminalHistoricalRecoveryCopiesOutsideHorizonIdentity ==
  \A initialContext \in AdmissibleContextRecords,
     node \in ValidatorIds, server \in ValidatorIds,
     source \in Chain!DecisionEvidenceSet:
    initialContext.height = MaxHeight
      /\ IndexedOpenHistoricalRecovery(
           initialContext, node, server, source)
      => /\ HistoricalRecoveryRecord(node, source).qc.context
                = initialContext
         /\ HistoricalRecoveryRecord(node, source).qc = source.qc
         /\ Chain!ReceiptOutsideChainHorizon(
              HistoricalRecoveryRecord(node, source))
BY Isa DEF IndexedOpenHistoricalRecovery,
           IndexedHistoricalRecoverySourceReady, HistoricalRecoveryRecord,
           IndexedCurrentDecisions,
           Chain!ReceiptOutsideChainHorizon

THEOREM SuccessorRosterEntrantIsHistoricalRecoveryEligible ==
  \A initialContext \in AdmissibleContextRecords,
     node \in ValidatorIds:
    ( /\ initialContext.height < MaxHeight
      /\ node \in Responsive
      /\ node \in VotingRoster(ExpectedEpoch(initialContext.height + 1))
      /\ node \notin IndexedAsync(initialContext)!
                       AsyncVotersAt(initialContext)
      /\ nodeHeight[node] = initialContext.height
      /\ nodeContext[node] = initialContext
      /\ ~IndexedAsync(initialContext)!NodeHasApplication(node))
      => /\ node \in Responsive
         /\ ExactNodeLocationAt(initialContext, node)
         /\ ~IndexedProjectedNodeHasApplication(initialContext, node)
BY DEF ExactNodeLocationAt, IndexedProjectedNodeHasApplication,
       HistoricalRecoveryNodeHasApplicationProjection,
       IndexedAsync!NodeHasApplication

(***************************************************************************
Regression witness for the production restart path: old-roster membership is
not a requester exclusion.  A responsive validator that restarts at its exact
old context and lacks that context's application is eligible for the same
authenticated CommitQC/body recovery as an observer or successor entrant.
***************************************************************************)
THEOREM RestartedCurrentRosterValidatorIsHistoricalRecoveryEligible ==
  \A initialContext \in AdmissibleContextRecords,
     node \in ValidatorIds:
    ( /\ initialContext.height < MaxHeight
      /\ node \in Responsive
      /\ node \in IndexedAsync(initialContext)!
                   AsyncVotersAt(initialContext)
      /\ nodeHeight[node] = initialContext.height
      /\ nodeContext[node] = initialContext
      /\ ~IndexedAsync(initialContext)!NodeHasApplication(node))
      => /\ node \in Responsive
         /\ ExactNodeLocationAt(initialContext, node)
         /\ ~IndexedProjectedNodeHasApplication(initialContext, node)
BY DEF ExactNodeLocationAt, IndexedProjectedNodeHasApplication,
       HistoricalRecoveryNodeHasApplicationProjection,
       IndexedAsync!NodeHasApplication

THEOREM JoinedMembershipIsMonotone ==
  IndexedChainNext
    => \A initialContext \in AdmissibleContextRecords:
         joinedByContext[initialContext]
           \subseteq joinedByContext'[initialContext]
BY Isa DEF IndexedChainNext, IndexedReceiptClassification,
           IndexedReceiptFreeChainStutter,
           IndexedDecisionReceiptHandoff,
           IndexedApplicationReceiptHandoff

THEOREM IndexedNodeHeightsAreMonotone ==
  IndexedChainNext
    => \A node \in ValidatorIds: nodeHeight[node] <= nodeHeight'[node]
BY Isa DEF IndexedChainNext, IndexedReceiptClassification,
           IndexedReceiptFreeChainStutter,
           IndexedDecisionReceiptHandoff,
           IndexedApplicationReceiptHandoff,
           Chain!RecordCertifiedNext, Chain!RecordKnownDecision,
           Chain!RecordAppliedNext, Chain!RecordKnownApplication

THEOREM IndexedBracketStepKeepsNodeHeightsMonotone ==
  [IndexedChainNext]_IndexedChainVars
    => \A node \in ValidatorIds: nodeHeight[node] <= nodeHeight'[node]
BY Isa, IndexedNodeHeightsAreMonotone
   DEF IndexedChainVars, Chain!ChainEpochVars

THEOREM IndexedStepProjectsChainEpochStep ==
  IndexedChainNext => [Chain!ChainEpochNext]_Chain!ChainEpochVars
BY Isa DEF IndexedChainNext, IndexedReceiptClassification,
           IndexedReceiptFreeChainStutter,
           IndexedDecisionReceiptHandoff,
           IndexedApplicationReceiptHandoff,
           Chain!ChainEpochNext

(***************************************************************************
Final successor publication is an exact Async service-activation action.
For the selected successor instance, use the production theorem which embeds
both restriction and monotone rearm into AsyncNext.  Every other pre-created
instance stutters extensionally.  This is the component-46 projection seam;
ordinary joined work is handled separately by JoinedAsyncStepRefinesExact.
***************************************************************************)
THEOREM SuccessorActivationEnvironmentProjectsEveryAsyncStep ==
  \A successorContext \in AdmissibleContextRecords,
     node \in ValidatorIds,
     observedContext \in AdmissibleContextRecords:
    /\ IndexedAsyncStateShape
    /\ SuccessorActivationEnvironmentActivatesNode(
         successorContext, node)
    => [IndexedAsync(observedContext)!AsyncNext]_(
         IndexedAsyncStateAt(observedContext))
BY IndexedAsync(successorContext)!
     AsyncServiceActivationActionsRefineAsyncNext,
   IndexedInstanceVariablesAreExact, Isa
   DEF SuccessorActivationEnvironmentActivatesNode,
       IndexedAsyncStateAt

THEOREM IndexedSuccessorActivationStepProjectsEveryAsyncStep ==
  \A parentContext \in AdmissibleContextRecords,
     node \in ValidatorIds,
     observedContext \in AdmissibleContextRecords:
    /\ IndexedAsyncStateShape
    /\ IndexedSuccessorActivationProgressStep(parentContext, node)
      => [IndexedAsync(observedContext)!AsyncNext]_(
           IndexedAsyncStateAt(observedContext))
BY SuccessorActivationEnvironmentProjectsEveryAsyncStep, Isa
   DEF IndexedSuccessorActivationProgressStep,
       BeginSuccessorActivation,
       BindAppliedSuccessorActivationToken,
       LatchAppliedSuccessorStartupFailure,
       LatchRecoveredSuccessorStartupFailure,
       RehydrateCleanCompleteTipSuccessorStartup,
       RehydrateFailedSuccessorStartup,
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
       IndexedAsyncStateAt

THEOREM IndexedStepProjectsEveryAsyncStep ==
  \A observedContext \in AdmissibleContextRecords:
    IndexedChainNext
      => [IndexedAsync(observedContext)!AsyncNext]_(
           IndexedAsyncStateAt(observedContext))
BY IndexedSuccessorActivationStepProjectsEveryAsyncStep,
   JoinedAsyncStepRefinesExactAsyncStep,
   IndexedInstanceVariablesAreExact, Isa
   DEF IndexedChainNext, IndexedProductActionAt

THEOREM IndexedInitEstablishesReceiptProjection ==
  IndexedChainInit => IndexedTotalReceiptProjection
BY DEF IndexedChainInit

THEOREM IndexedStepPreservesReceiptProjection ==
  IndexedCompositionInvariant /\ [IndexedChainNext]_IndexedChainVars
    => IndexedTotalReceiptProjection'
BY Isa DEF IndexedChainNext, IndexedChainVars,
           IndexedCompositionInvariant,
           IndexedReceiptClassification,
           IndexedReceiptFreeChainStutter,
           IndexedDecisionReceiptHandoff,
           IndexedApplicationReceiptHandoff,
           IndexedTotalReceiptProjection,
           IndexedDecisionReceiptProjection,
           IndexedApplicationReceiptProjection,
           IndexedDecisionEvidence, IndexedApplicationEvidence,
           IndexedCurrentDecisions, IndexedCurrentApplications,
           NewIndexedDecisionReceipt, NewIndexedApplicationReceipt,
           NoNewIndexedDurableReceipt

(***************************************************************************
The initialized and step-preserved product invariant closes the two state
seams used by the temporal composition: routing after a local advance, and
canonical/certified activation of a successor instance.
***************************************************************************)
THEOREM IndexedInitEstablishesEveryInstanceStrongInvariant ==
  IndexedChainInit => IndexedEveryInstanceStrongInvariant
PROOF
  <1>1. ASSUME IndexedChainInit,
               NEW initialContext \in AdmissibleContextRecords
         PROVE IndexedAsync(initialContext)!StrongInductiveInvariant
    <2>1. IndexedAsync(initialContext)!AsyncInitAt(initialContext)
      BY <1>1 DEF IndexedChainInit
    <2>2. IndexedAsync(initialContext)!InitAt(initialContext)
      BY <2>1 DEF IndexedAsync!AsyncInitAt,
                    IndexedAsync!AsyncBaseInitAt
    <2> QED BY <2>2, SMT
  <1> QED BY <1>1 DEF IndexedEveryInstanceStrongInvariant

THEOREM IndexedInitEstablishesEveryInstanceAsyncStrongTypeInvariant ==
  IndexedChainInit => IndexedEveryInstanceAsyncStrongTypeInvariant
PROOF
  <1>1. ASSUME IndexedChainInit,
               NEW initialContext \in AdmissibleContextRecords
         PROVE IndexedAsync(initialContext)!AsyncStrongTypeInvariant
    <2>1. IndexedAsync(initialContext)!AsyncInitAt(initialContext)
      BY <1>1, IndexedInitProjectsEveryAsyncInit
    <2> QED BY <2>1,
         IndexedAsync(initialContext)!
           AsyncInitEstablishesStrongTypeInvariant
  <1> QED BY <1>1
       DEF IndexedEveryInstanceAsyncStrongTypeInvariant

THEOREM IndexedInitEstablishesServiceActivationCoherence ==
  IndexedChainInit => IndexedServiceActivationCoherence
PROOF
  <1>1. ASSUME IndexedChainInit,
               NEW initialContext \in AdmissibleContextRecords
         PROVE
           /\ IndexedAsync(initialContext)!
                AsyncServiceActivationPairInvariant
           /\ IF IndexedAsync(initialContext)!
                   AsyncServiceActivationRestricted
              THEN /\ joinedByContext[initialContext] # {}
                   /\ IndexedAsync(initialContext)!
                        AsyncActiveServiceNodes
                        = joinedByContext[initialContext]
              ELSE /\ IndexedAsync(initialContext)!
                        AsyncActiveServiceNodes = ValidatorIds
                   /\ \/ initialContext = GenesisContext
                      \/ /\ joinedByContext[initialContext] = {}
                         /\ IndexedAsync(initialContext)!
                              AsyncServiceActivationClockPristine
    <2>1. IndexedAsync(initialContext)!AsyncStrongTypeInvariant
      BY <1>1,
         IndexedInitEstablishesEveryInstanceAsyncStrongTypeInvariant
         DEF IndexedEveryInstanceAsyncStrongTypeInvariant
    <2>2. IndexedAsync(initialContext)!
             AsyncServiceActivationPairInvariant
      BY <2>1
         DEF IndexedAsync!AsyncStrongTypeInvariant
    <2>3. IndexedAsync(initialContext)!AsyncInitAt(initialContext)
      BY <1>1, IndexedInitProjectsEveryAsyncInit
    <2> QED BY <1>1, <2>2, <2>3, Isa
         DEF IndexedChainInit, GenesisContext,
             IndexedAsync!AsyncInitAt,
             IndexedAsync!AsyncBaseInitAt,
             IndexedAsync!AsyncRuntimeInit,
             IndexedAsync!AsyncTransportInit,
             IndexedAsync!AsyncServiceActivationRestricted,
             IndexedAsync!AsyncActiveServiceNodes,
             IndexedAsync!AsyncServiceActivationClockPristine
  <1> QED BY <1>1
       DEF IndexedServiceActivationCoherence,
           IndexedServiceActivationMembershipCoherenceAt

THEOREM IndexedInitEstablishesPostGstContextJoinedCoherence ==
  IndexedChainInit => IndexedPostGstContextJoinedCoherence
BY Isa
   DEF IndexedChainInit,
       IndexedPostGstContextJoinedCoherence,
       IndexedAsync!AsyncInitAt,
       IndexedAsync!AsyncBaseInitAt,
       IndexedAsync!InitAt,
       JoinedContexts

THEOREM IndexedInitEstablishesPostGstResponsiveActiveRosterCoherence ==
  IndexedChainInit => IndexedPostGstResponsiveActiveRosterCoherence
BY Isa
   DEF IndexedChainInit,
       IndexedPostGstResponsiveActiveRosterCoherence,
       IndexedAsync!AsyncInitAt,
       IndexedAsync!AsyncBaseInitAt,
       IndexedAsync!InitAt,
       IndexedAsync!AsyncActiveServiceNodes

THEOREM IndexedInitEstablishesCompositionInvariant ==
  IndexedChainInit => IndexedCompositionInvariant
BY Isa, Chain!GenesisEstablishesChainEpochInvariant,
   IndexedChainInitHasEmptyCurrentReceiptUnion,
   IndexedInitEstablishesEveryInstanceStrongInvariant,
   IndexedInitEstablishesEveryInstanceAsyncStrongTypeInvariant,
   IndexedInitEstablishesServiceActivationCoherence,
   IndexedInitEstablishesPostGstContextJoinedCoherence,
   IndexedInitEstablishesPostGstResponsiveActiveRosterCoherence
   DEF IndexedChainInit, IndexedCompositionInvariant,
       IndexedTotalReceiptProjection,
       IndexedDecisionReceiptProjection,
       IndexedApplicationReceiptProjection,
       JoinedContextCertificationInvariant, JoinedRoutingInvariant,
       IndexedApplicationsRespectNodeHeight,
       IndexedEveryInstanceAsyncStrongTypeInvariant,
       IndexedServiceActivationCoherence,
       IndexedPostGstContextJoinedCoherence,
       IndexedPostGstResponsiveActiveRosterCoherence,
       JoinedContexts,
       IndexedNodeCurrentAt, GenesisContext,
       IndexedAsync!NodeHasApplication,
       IndexedAsync!AsyncVotersAt, IndexedAsync!InitAt,
       IndexedAsync!BootstrapParentDecision

THEOREM IndexedActionPreservesEveryInstanceStrongInvariant ==
  IndexedCompositionInvariant /\ IndexedChainNext
    => IndexedEveryInstanceStrongInvariant'
PROOF
  <1>1. ASSUME IndexedCompositionInvariant,
              IndexedChainNext,
              NEW initialContext \in AdmissibleContextRecords
         PROVE (IndexedAsync(initialContext)!
                  StrongInductiveInvariant)'
    <2>1. IndexedAsync(initialContext)!StrongInductiveInvariant
      BY <1>1 DEF IndexedCompositionInvariant,
                    IndexedEveryInstanceStrongInvariant
    <2>2. IndexedAsync(initialContext)!AsyncAllVars
               = IndexedAsyncStateAt(initialContext)
      BY <1>1, IndexedInstanceVariablesAreExact
         DEF IndexedCompositionInvariant
    <2>3. [IndexedAsync(initialContext)!AsyncNext]_(
             IndexedAsyncStateAt(initialContext))
      BY <1>1, IndexedStepProjectsEveryAsyncStep
    <2>4. [IndexedAsync(initialContext)!Next]_(
             IndexedAsync(initialContext)!vars)
      BY <2>2, <2>3, Isa
         DEF IndexedAsync!AsyncNext, IndexedAsync!AsyncAllVars
    <2> QED BY <2>1, <2>4, SMT
  <1> QED BY <1>1 DEF IndexedEveryInstanceStrongInvariant

THEOREM IndexedActionPreservesEveryInstanceAsyncStrongTypeInvariant ==
  IndexedCompositionInvariant /\ IndexedChainNext
    => IndexedEveryInstanceAsyncStrongTypeInvariant'
PROOF
  <1>1. ASSUME IndexedCompositionInvariant,
              IndexedChainNext,
              NEW initialContext \in AdmissibleContextRecords
         PROVE (IndexedAsync(initialContext)!
                  AsyncStrongTypeInvariant)'
    <2>1. IndexedAsync(initialContext)!AsyncStrongTypeInvariant
      BY <1>1
         DEF IndexedCompositionInvariant,
             IndexedEveryInstanceAsyncStrongTypeInvariant
    <2>2. IndexedAsync(initialContext)!AsyncAllVars
               = IndexedAsyncStateAt(initialContext)
      BY <1>1, IndexedInstanceVariablesAreExact
         DEF IndexedCompositionInvariant
    <2>3. [IndexedAsync(initialContext)!AsyncNext]_(
             IndexedAsyncStateAt(initialContext))
      BY <1>1, IndexedStepProjectsEveryAsyncStep
    <2>4. [IndexedAsync(initialContext)!AsyncNext]_(
             IndexedAsync(initialContext)!AsyncAllVars)
      BY <2>2, <2>3, Isa
    <2> QED BY <2>1, <2>4,
         IndexedAsync(initialContext)!
           AsyncBracketNextPreservesStrongTypeInvariant
  <1> QED BY <1>1
       DEF IndexedEveryInstanceAsyncStrongTypeInvariant

(***************************************************************************
Atomic join/activation guard audit.

The branch selector reads unprimed joined membership.  The same final action
then primes both joinedByContext and scheduler component 46.  Consequently a
first join can only burn the restriction tombstone and install the singleton
active owner, while a later join can only monotonically add and rearm that
exact node.  Neither publication path can expose joined membership one step
before its service clocks become active.
***************************************************************************)
SuccessorFinalPublicationAction(parentContext, node, successorContext) ==
  \/ ActivateAppliedSuccessorHeight(
       parentContext, node, successorContext)
  \/ ActivateRecoveredSuccessorHeight(
       parentContext, node, successorContext)

THEOREM FirstSuccessorJoinAtomicallyRestrictsServiceActivation ==
  \A parentContext, successorContext \in AdmissibleContextRecords,
     node \in ValidatorIds:
    /\ joinedByContext[successorContext] = {}
    /\ SuccessorFinalPublicationAction(
         parentContext, node, successorContext)
    => /\ joinedByContext'[successorContext] = {node}
       /\ (IndexedAsync(successorContext)!
             AsyncServiceActivationRestricted)'
       /\ (IndexedAsync(successorContext)!AsyncActiveServiceNodes)'
            = {node}
       /\ indexedAsyncState'[successorContext][3][33][node]
            = AsyncDeliveryBound
       /\ indexedAsyncState'[successorContext][3][34][node]
            = AsyncDeliveryBound
BY Isa
   DEF SuccessorFinalPublicationAction,
       ActivateAppliedSuccessorHeight,
       ActivateRecoveredSuccessorHeight,
       SuccessorActivationEnvironmentActivatesNode,
       IndexedAsync!AsyncEnterIndexedServiceActivation,
       IndexedAsync!AsyncServiceActivationRestricted,
       IndexedAsync!AsyncActiveServiceNodes,
       IndexedScheduler

THEOREM LaterSuccessorJoinAtomicallyRearmsServiceActivation ==
  \A parentContext, successorContext \in AdmissibleContextRecords,
     node \in ValidatorIds:
    /\ joinedByContext[successorContext] # {}
    /\ SuccessorFinalPublicationAction(
         parentContext, node, successorContext)
    => /\ IndexedAsync(successorContext)!
             AsyncServiceActivationRestricted
       /\ (IndexedAsync(successorContext)!
             AsyncServiceActivationRestricted)'
       /\ joinedByContext'[successorContext]
            = joinedByContext[successorContext] \cup {node}
       /\ (IndexedAsync(successorContext)!AsyncActiveServiceNodes)'
            = IndexedAsync(successorContext)!AsyncActiveServiceNodes
                \cup {node}
       /\ node \in
            (IndexedAsync(successorContext)!AsyncActiveServiceNodes)'
       /\ indexedAsyncState'[successorContext][3][33][node]
            = IndexedScheduler(successorContext, 1)
                + AsyncDeliveryBound
       /\ indexedAsyncState'[successorContext][3][34][node]
            = IndexedScheduler(successorContext, 1)
                + AsyncDeliveryBound
BY Isa
   DEF SuccessorFinalPublicationAction,
       ActivateAppliedSuccessorHeight,
       ActivateRecoveredSuccessorHeight,
       SuccessorActivationEnvironmentActivatesNode,
       IndexedAsync!AsyncActivateServiceNode,
       IndexedAsync!AsyncServiceActivationRestricted,
       IndexedAsync!AsyncActiveServiceNodes,
       IndexedScheduler

THEOREM IndexedProductActionPreservesServiceActivationMembership ==
  \A selectedContext \in AdmissibleContextRecords:
    /\ IndexedCompositionInvariant
    /\ IndexedProductActionAt(selectedContext)
    => \A initialContext \in AdmissibleContextRecords:
         (IndexedServiceActivationMembershipCoherenceAt(
            initialContext))'
BY Isa
   DEF IndexedCompositionInvariant,
       IndexedServiceActivationCoherence,
       IndexedServiceActivationMembershipCoherenceAt,
       IndexedProductActionAt,
       IndexedJoinedAsyncNext,
       IndexedReceiptClassification,
       IndexedReceiptFreeChainStutter,
       IndexedDecisionReceiptHandoff,
       IndexedApplicationReceiptHandoff,
       IndexedAsync!AsyncServiceActivationRestricted,
       IndexedAsync!AsyncActiveServiceNodes,
       IndexedAsync!AsyncServiceActivationClockPristine,
       IndexedScheduler

THEOREM IndexedSuccessorActivationActionPreservesServiceActivationMembership ==
  \A parentContext \in AdmissibleContextRecords,
     node \in ValidatorIds:
    /\ IndexedCompositionInvariant
    /\ IndexedSuccessorActivationProgressStep(parentContext, node)
    => \A initialContext \in AdmissibleContextRecords:
         (IndexedServiceActivationMembershipCoherenceAt(
            initialContext))'
BY FirstSuccessorJoinAtomicallyRestrictsServiceActivation,
   LaterSuccessorJoinAtomicallyRearmsServiceActivation, Isa
   DEF IndexedCompositionInvariant,
       IndexedServiceActivationCoherence,
       IndexedServiceActivationMembershipCoherenceAt,
       IndexedSuccessorActivationProgressStep,
       BeginSuccessorActivation,
       BindAppliedSuccessorActivationToken,
       LatchAppliedSuccessorStartupFailure,
       LatchRecoveredSuccessorStartupFailure,
       RehydrateCleanCompleteTipSuccessorStartup,
       RehydrateFailedSuccessorStartup,
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
       SuccessorActivationEnvironmentActivatesNode,
       IndexedAsync!AsyncEnterIndexedServiceActivation,
       IndexedAsync!AsyncActivateServiceNode,
       IndexedAsync!AsyncServiceActivationRestricted,
       IndexedAsync!AsyncActiveServiceNodes,
       IndexedAsync!AsyncServiceActivationClockPristine,
       IndexedAsyncStateAt, IndexedScheduler

THEOREM IndexedActionPreservesServiceActivationCoherence ==
  IndexedCompositionInvariant /\ IndexedChainNext
    => IndexedServiceActivationCoherence'
PROOF
  <1>1. ASSUME IndexedCompositionInvariant,
              IndexedChainNext
         PROVE IndexedServiceActivationCoherence'
    <2>1. IndexedEveryInstanceAsyncStrongTypeInvariant'
      BY <1>1,
         IndexedActionPreservesEveryInstanceAsyncStrongTypeInvariant
    <2>2. \A initialContext \in AdmissibleContextRecords:
             (IndexedAsync(initialContext)!
                AsyncServiceActivationPairInvariant)'
      BY <2>1
         DEF IndexedEveryInstanceAsyncStrongTypeInvariant,
             IndexedAsync!AsyncStrongTypeInvariant
    <2>3. \A initialContext \in AdmissibleContextRecords:
             (IndexedServiceActivationMembershipCoherenceAt(
                initialContext))'
      BY <1>1,
         IndexedProductActionPreservesServiceActivationMembership,
         IndexedSuccessorActivationActionPreservesServiceActivationMembership,
         Isa DEF IndexedChainNext
    <2> QED BY <2>2, <2>3
         DEF IndexedServiceActivationCoherence
  <1> QED BY <1>1

THEOREM IndexedActionKeepsServiceActivationRestrictionIrreversible ==
  IndexedCompositionInvariant /\ IndexedChainNext
    => \A initialContext \in AdmissibleContextRecords:
         IndexedAsync(initialContext)!AsyncServiceActivationRestricted
           => (IndexedAsync(initialContext)!
                 AsyncServiceActivationRestricted)'
BY FirstSuccessorJoinAtomicallyRestrictsServiceActivation,
   LaterSuccessorJoinAtomicallyRearmsServiceActivation, Isa
   DEF IndexedCompositionInvariant,
       IndexedServiceActivationCoherence,
       IndexedChainNext, IndexedProductActionAt,
       IndexedJoinedAsyncNext,
       IndexedSuccessorActivationProgressStep,
       SuccessorFinalPublicationAction,
       BeginSuccessorActivation,
       BindAppliedSuccessorActivationToken,
       LatchAppliedSuccessorStartupFailure,
       LatchRecoveredSuccessorStartupFailure,
       RehydrateCleanCompleteTipSuccessorStartup,
       RehydrateFailedSuccessorStartup,
       AuthenticateRecoveredSuccessorActivation,
       OpenDeferredSuccessorAdapter,
       ConstructSuccessorRuntime,
       StartSuccessorServices,
       ApplySuccessorStartupEffects,
       ArmSuccessorClocks,
       PrepareSuccessorActivationMarker,
       OpenSuccessorIngress,
       SuccessorActivationEnvironmentStutter,
       SuccessorActivationEnvironmentActivatesNode,
       ActivateAppliedSuccessorHeight,
       ActivateRecoveredSuccessorHeight,
       IndexedAsync!AsyncEnterIndexedServiceActivation,
       IndexedAsync!AsyncActivateServiceNode,
       IndexedAsync!AsyncServiceActivationRestricted,
       IndexedScheduler

THEOREM IndexedStepKeepsServiceActivationRestrictionIrreversible ==
  IndexedCompositionInvariant
    /\ [IndexedChainNext]_IndexedChainVars
    => \A initialContext \in AdmissibleContextRecords:
         IndexedAsync(initialContext)!AsyncServiceActivationRestricted
           => (IndexedAsync(initialContext)!
                 AsyncServiceActivationRestricted)'
BY IndexedActionKeepsServiceActivationRestrictionIrreversible, Isa
   DEF IndexedChainVars, IndexedAsyncStateAt,
       IndexedCore, IndexedScheduler, IndexedRecovery

THEOREM IndexedStutterPreservesServiceActivationCoherence ==
  IndexedServiceActivationCoherence
    /\ UNCHANGED IndexedChainVars
    => IndexedServiceActivationCoherence'
BY Isa
   DEF IndexedServiceActivationCoherence,
       IndexedServiceActivationMembershipCoherenceAt,
       IndexedChainVars, IndexedAsyncStateAt,
       IndexedCore, IndexedScheduler, IndexedRecovery

THEOREM IndexedStepPreservesServiceActivationCoherence ==
  IndexedCompositionInvariant
    /\ [IndexedChainNext]_IndexedChainVars
    => IndexedServiceActivationCoherence'
BY IndexedActionPreservesServiceActivationCoherence,
   IndexedStutterPreservesServiceActivationCoherence, Isa

THEOREM IndexedNewGstRequiresJoinedContext ==
  \A initialContext \in AdmissibleContextRecords:
    /\ IndexedChainNext
    /\ ~IndexedAsync(initialContext)!gst
    /\ (IndexedAsync(initialContext)!gst)'
    => /\ initialContext \in JoinedContexts
       /\ initialContext \in JoinedContexts'
BY JoinedMembershipIsMonotone, Isa
   DEF IndexedChainNext, IndexedProductActionAt,
       IndexedJoinedAsyncNext,
       IndexedSuccessorActivationProgressStep,
       SuccessorActivationEnvironmentStutter,
       SuccessorActivationEnvironmentActivatesNode,
       IndexedAsync!AsyncEnterIndexedServiceActivation,
       IndexedAsync!AsyncActivateServiceNode,
       IndexedAsync!AsyncServiceActivationFrameVars,
       IndexedAsyncStateAt, IndexedCore,
       JoinedContexts

THEOREM IndexedActionPreservesPostGstContextJoinedCoherence ==
  IndexedCompositionInvariant /\ IndexedChainNext
    => IndexedPostGstContextJoinedCoherence'
BY IndexedNewGstRequiresJoinedContext,
   JoinedMembershipIsMonotone,
   IndexedAsync!GstAsyncStepIsMonotone,
   IndexedStepProjectsEveryAsyncStep,
   IndexedInstanceVariablesAreExact, Isa
   DEF IndexedCompositionInvariant,
       IndexedPostGstContextJoinedCoherence,
       IndexedChainNext, IndexedChainVars

THEOREM IndexedStutterPreservesPostGstContextJoinedCoherence ==
  IndexedPostGstContextJoinedCoherence
    /\ UNCHANGED IndexedChainVars
    => IndexedPostGstContextJoinedCoherence'
BY Isa
   DEF IndexedPostGstContextJoinedCoherence,
       IndexedChainVars, JoinedContexts,
       IndexedAsyncStateAt, IndexedCore,
       IndexedScheduler, IndexedRecovery

THEOREM IndexedStepPreservesPostGstContextJoinedCoherence ==
  IndexedCompositionInvariant
    /\ [IndexedChainNext]_IndexedChainVars
    => IndexedPostGstContextJoinedCoherence'
BY IndexedActionPreservesPostGstContextJoinedCoherence,
   IndexedStutterPreservesPostGstContextJoinedCoherence, Isa

THEOREM IndexedNewGstRequiresResponsiveActiveRoster ==
  \A initialContext \in AdmissibleContextRecords:
    /\ IndexedCompositionInvariant
    /\ IndexedChainNext
    /\ ~IndexedAsync(initialContext)!gst
    /\ (IndexedAsync(initialContext)!gst)'
    => /\ Responsive \subseteq
             IndexedAsync(initialContext)!AsyncActiveServiceNodes
       /\ Responsive \subseteq
             (IndexedAsync(initialContext)!AsyncActiveServiceNodes)'
BY IndexedStepProjectsEveryAsyncStep,
   IndexedInstanceVariablesAreExact, Isa
   DEF IndexedChainNext, IndexedProductActionAt,
       IndexedJoinedAsyncNext,
       IndexedSuccessorActivationProgressStep,
       SuccessorActivationEnvironmentStutter,
       SuccessorActivationEnvironmentActivatesNode,
       IndexedAsync!AsyncNext,
       IndexedAsync!AsyncNonCrashStep,
       IndexedAsync!AsyncSetGST,
       IndexedAsync!AsyncServiceActivationTransition,
       IndexedAsync!AsyncEnterIndexedServiceActivation,
       IndexedAsync!AsyncActivateServiceNode,
       IndexedAsync!AsyncServiceActivationFrameVars,
       IndexedAsync!AsyncActiveServiceNodes,
       IndexedAsyncStateAt, IndexedCore, IndexedScheduler

THEOREM IndexedPostGstResponsiveActiveRosterSurvivesAction ==
  \A initialContext \in AdmissibleContextRecords:
    /\ IndexedCompositionInvariant
    /\ IndexedChainNext
    /\ IndexedAsync(initialContext)!gst
    /\ Responsive \subseteq
         IndexedAsync(initialContext)!AsyncActiveServiceNodes
    => Responsive \subseteq
         (IndexedAsync(initialContext)!AsyncActiveServiceNodes)'
BY IndexedStepProjectsEveryAsyncStep,
   IndexedInstanceVariablesAreExact, Isa
   DEF IndexedChainNext, IndexedProductActionAt,
       IndexedJoinedAsyncNext,
       IndexedSuccessorActivationProgressStep,
       SuccessorActivationEnvironmentStutter,
       SuccessorActivationEnvironmentActivatesNode,
       IndexedAsync!AsyncNext,
       IndexedAsync!AsyncNonCrashStep,
       IndexedAsync!AsyncServiceActivationTransition,
       IndexedAsync!AsyncEnterIndexedServiceActivation,
       IndexedAsync!AsyncActivateServiceNode,
       IndexedAsync!AsyncServiceActivationClockPristine,
       IndexedAsync!AsyncServiceActivationFrameVars,
       IndexedAsync!AsyncActiveServiceNodes,
       IndexedAsyncStateAt, IndexedCore, IndexedScheduler

THEOREM IndexedActionPreservesPostGstResponsiveActiveRosterCoherence ==
  IndexedCompositionInvariant /\ IndexedChainNext
    => IndexedPostGstResponsiveActiveRosterCoherence'
BY IndexedNewGstRequiresResponsiveActiveRoster,
   IndexedPostGstResponsiveActiveRosterSurvivesAction,
   IndexedAsync!GstAsyncStepIsMonotone,
   IndexedStepProjectsEveryAsyncStep,
   IndexedInstanceVariablesAreExact, Isa
   DEF IndexedCompositionInvariant,
       IndexedPostGstResponsiveActiveRosterCoherence,
       IndexedChainNext, IndexedChainVars

THEOREM IndexedStutterPreservesPostGstResponsiveActiveRosterCoherence ==
  IndexedPostGstResponsiveActiveRosterCoherence
    /\ UNCHANGED IndexedChainVars
    => IndexedPostGstResponsiveActiveRosterCoherence'
BY Isa
   DEF IndexedPostGstResponsiveActiveRosterCoherence,
       IndexedChainVars, IndexedAsyncStateAt,
       IndexedCore, IndexedScheduler, IndexedRecovery

THEOREM IndexedStepPreservesPostGstResponsiveActiveRosterCoherence ==
  IndexedCompositionInvariant
    /\ [IndexedChainNext]_IndexedChainVars
    => IndexedPostGstResponsiveActiveRosterCoherence'
BY IndexedActionPreservesPostGstResponsiveActiveRosterCoherence,
   IndexedStutterPreservesPostGstResponsiveActiveRosterCoherence, Isa

THEOREM IndexedActionPreservesCompositionInvariant ==
  IndexedCompositionInvariant /\ IndexedChainNext
    => IndexedCompositionInvariant'
BY Isa, AppliedSuccessorIsAdmissible,
   IndexedStepProjectsChainEpochStep,
   Chain!ChainEpochInductiveStep,
   IndexedStepPreservesReceiptProjection,
   IndexedActionPreservesEveryInstanceStrongInvariant,
   IndexedActionPreservesEveryInstanceAsyncStrongTypeInvariant,
   IndexedActionPreservesServiceActivationCoherence,
   IndexedActionPreservesPostGstContextJoinedCoherence,
   IndexedActionPreservesPostGstResponsiveActiveRosterCoherence
   DEF IndexedCompositionInvariant, IndexedChainNext,
       IndexedProductActionAt, IndexedReceiptClassification,
       IndexedReceiptFreeChainStutter,
       IndexedDecisionReceiptHandoff,
       IndexedApplicationReceiptHandoff,
       NewIndexedDecisionReceipt, NewIndexedApplicationReceipt,
       NoNewIndexedDurableReceipt,
       IndexedEveryInstanceStrongInvariant,
       IndexedEveryInstanceAsyncStrongTypeInvariant,
       IndexedServiceActivationCoherence,
       IndexedServiceActivationMembershipCoherenceAt,
       IndexedPostGstContextJoinedCoherence,
       IndexedPostGstResponsiveActiveRosterCoherence,
       JoinedContextCertificationInvariant, JoinedRoutingInvariant,
       IndexedApplicationsRespectNodeHeight,
       IndexedNodeCurrentAt, ExactNodeLocationAt,
       JoinedContexts, SuccessorContextFor,
       IndexedCurrentDecisions, IndexedCurrentApplications,
       Chain!ChainEpochInvariant, Chain!ChainEpochTypeInvariant,
       Chain!ContextsMatchLocalHistories,
       Chain!RecordCertifiedNext, Chain!RecordKnownDecision,
       Chain!RecordAppliedNext, Chain!RecordKnownApplication,
       IndexedAsync!StrongInductiveInvariant,
       IndexedAsync!Safety, IndexedAsync!TypeInvariant,
       IndexedAsync!DecisionAgreement,
       IndexedAsync!AppliedRequiresDecision,
       IndexedAsync!NodeHasApplication

THEOREM IndexedStepPreservesCompositionInvariant ==
  IndexedCompositionInvariant /\ [IndexedChainNext]_IndexedChainVars
    => IndexedCompositionInvariant'
PROOF
  <1>1. ASSUME IndexedCompositionInvariant,
              [IndexedChainNext]_IndexedChainVars
         PROVE IndexedCompositionInvariant'
    <2>1. CASE IndexedChainNext
      BY <1>1, <2>1, IndexedActionPreservesCompositionInvariant
    <2>2. CASE UNCHANGED IndexedChainVars
      BY <1>1, <2>2, Isa
         DEF IndexedChainVars, IndexedCompositionInvariant,
             IndexedEveryInstanceStrongInvariant,
             IndexedEveryInstanceAsyncStrongTypeInvariant,
             JoinedContextCertificationInvariant, JoinedRoutingInvariant,
             IndexedApplicationsRespectNodeHeight,
             IndexedServiceActivationCoherence,
             IndexedServiceActivationMembershipCoherenceAt,
             IndexedPostGstContextJoinedCoherence,
             IndexedPostGstResponsiveActiveRosterCoherence,
             IndexedTotalReceiptProjection,
             IndexedDecisionReceiptProjection,
             IndexedApplicationReceiptProjection,
             IndexedDecisionEvidence, IndexedApplicationEvidence,
             IndexedCurrentDecisions, IndexedCurrentApplications,
             IndexedAsyncStateAt, IndexedCore, IndexedScheduler,
             IndexedRecovery,
             JoinedContexts, IndexedNodeCurrentAt,
             Chain!ChainEpochVars
    <2> QED BY <1>1, <2>1, <2>2
  <1> QED BY <1>1

THEOREM JoinedCanonicalDescendantIsStable ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedCompositionInvariant
      /\ JoinedCanonicalDescendant(initialContext)
      /\ [IndexedChainNext]_IndexedChainVars
      => JoinedCanonicalDescendant(initialContext)'
BY Isa, JoinedMembershipIsMonotone,
   IndexedStepPreservesCompositionInvariant
   DEF JoinedCanonicalDescendant, JoinedContexts,
       IndexedChainVars, IndexedCompositionInvariant,
       JoinedContextCertificationInvariant,
       Chain!ChainEpochInvariant, Chain!ChainEpochTypeInvariant,
       Chain!ContextRecord, Chain!HistoryThrough

THEOREM IndexedChainSpecEstablishesCompositionInvariant ==
  IndexedChainSpec => []IndexedCompositionInvariant
PROOF
  <1>1. IndexedChainInit => IndexedCompositionInvariant
    BY IndexedInitEstablishesCompositionInvariant
  <1>2. IndexedCompositionInvariant
           /\ [IndexedChainNext]_IndexedChainVars
           => IndexedCompositionInvariant'
    BY IndexedStepPreservesCompositionInvariant
  <1> QED BY <1>1, <1>2, PTL DEF IndexedChainSpec

THEOREM IndexedChainSpecAlwaysKeepsPostGstContextsJoined ==
  IndexedChainSpec
    => []IndexedPostGstContextJoinedCoherence
BY IndexedChainSpecEstablishesCompositionInvariant, PTL
   DEF IndexedCompositionInvariant

THEOREM IndexedChainSpecAlwaysKeepsPostGstResponsiveRosterActive ==
  IndexedChainSpec
    => []IndexedPostGstResponsiveActiveRosterCoherence
BY IndexedChainSpecEstablishesCompositionInvariant, PTL
   DEF IndexedCompositionInvariant

THEOREM IndexedPostGstResponsiveRosterIsActive ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedChainSpec
      => [](IndexedAsync(initialContext)!gst
             => Responsive \subseteq
                  IndexedAsync(initialContext)!AsyncActiveServiceNodes)
BY IndexedChainSpecAlwaysKeepsPostGstResponsiveRosterActive, PTL
   DEF IndexedPostGstResponsiveActiveRosterCoherence

THEOREM IndexedChainSpecAlwaysJoinsEachPostGstContext ==
  IndexedChainSpec
    => \A initialContext \in AdmissibleContextRecords:
         [](IndexedAsync(initialContext)!gst
              => initialContext \in JoinedContexts)
BY IndexedChainSpecAlwaysKeepsPostGstContextsJoined, PTL
   DEF IndexedPostGstContextJoinedCoherence

THEOREM IndexedChainSpecKeepsServiceActivationRestrictionIrreversible ==
  IndexedChainSpec
    => \A initialContext \in AdmissibleContextRecords:
         [](IndexedAsync(initialContext)!AsyncServiceActivationRestricted
              => []IndexedAsync(initialContext)!
                    AsyncServiceActivationRestricted)
PROOF
  <1>1. ASSUME IndexedChainSpec,
               NEW initialContext \in AdmissibleContextRecords
         PROVE
           [](IndexedAsync(initialContext)!AsyncServiceActivationRestricted
                => []IndexedAsync(initialContext)!
                      AsyncServiceActivationRestricted)
    <2>1. []IndexedCompositionInvariant
      BY <1>1, IndexedChainSpecEstablishesCompositionInvariant
    <2>2. [](IndexedAsync(initialContext)!
                AsyncServiceActivationRestricted
              /\ [IndexedChainNext]_IndexedChainVars
              => (IndexedAsync(initialContext)!
                    AsyncServiceActivationRestricted)')
      BY <2>1,
         IndexedStepKeepsServiceActivationRestrictionIrreversible,
         PTL
    <2> QED BY <1>1, <2>2, PTL
         DEF IndexedChainSpec
  <1> QED BY <1>1

THEOREM IndexedInitJoinsEveryNodeThroughGenesis ==
  IndexedChainInit => IndexedJoinedThroughLocalHeight
BY Isa DEF IndexedChainInit, IndexedJoinedThroughLocalHeight,
           CanonicalIndexedContext, JoinedByContextShape,
           AdmissibleContextRecords, FrozenContextAdmissible,
           ContextRecords, LineagesAt, Heights,
           Chain!ChainEpochInit, Chain!HistoryThrough,
           Chain!ContextRecord

THEOREM IndexedActionPreservesJoinedThroughLocalHeight ==
  IndexedCompositionInvariant
    /\ IndexedJoinedThroughLocalHeight
    /\ IndexedChainNext
    => IndexedJoinedThroughLocalHeight'
BY Isa DEF IndexedJoinedThroughLocalHeight,
           CanonicalIndexedContext, IndexedChainNext,
           IndexedProductActionAt, IndexedReceiptClassification,
           IndexedReceiptFreeChainStutter,
           IndexedDecisionReceiptHandoff,
           IndexedApplicationReceiptHandoff,
           SuccessorContextFor,
           IndexedCompositionInvariant,
           JoinedContextCertificationInvariant,
           JoinedRoutingInvariant,
           JoinedContexts, IndexedNodeCurrentAt,
           Chain!ChainEpochInvariant, Chain!ChainEpochTypeInvariant,
           Chain!CertifiedPrefixBacked,
           Chain!NodesDoNotOutrunCertificates,
           Chain!ContextsMatchLocalHistories,
           Chain!RecordCertifiedNext, Chain!RecordKnownDecision,
           Chain!RecordAppliedNext, Chain!RecordKnownApplication,
           Chain!CanonicalCommitForSlot, Chain!HistoryThrough,
           Chain!ContextRecord, AdmissibleContextRecords,
           FrozenContextAdmissible, ContextRecords, LineagesAt,
           Heights

THEOREM IndexedStepPreservesJoinedThroughLocalHeight ==
  IndexedCompositionInvariant
    /\ IndexedJoinedThroughLocalHeight
    /\ [IndexedChainNext]_IndexedChainVars
    => IndexedJoinedThroughLocalHeight'
PROOF
  <1>1. ASSUME IndexedCompositionInvariant,
              IndexedJoinedThroughLocalHeight,
              [IndexedChainNext]_IndexedChainVars
         PROVE IndexedJoinedThroughLocalHeight'
    <2>1. CASE IndexedChainNext
      BY <1>1, <2>1, IndexedActionPreservesJoinedThroughLocalHeight
    <2>2. CASE UNCHANGED IndexedChainVars
      BY <1>1, <2>2, Isa
         DEF IndexedJoinedThroughLocalHeight,
             CanonicalIndexedContext, IndexedChainVars,
             Chain!ChainEpochVars
    <2> QED BY <1>1, <2>1, <2>2
  <1> QED BY <1>1

THEOREM IndexedChainSpecJoinsEveryNodeThroughLocalHeight ==
  IndexedChainSpec => []IndexedJoinedThroughLocalHeight
PROOF
  <1>1. IndexedChainInit => IndexedJoinedThroughLocalHeight
    BY IndexedInitJoinsEveryNodeThroughGenesis
  <1>2. IndexedChainSpec => []IndexedCompositionInvariant
    BY IndexedChainSpecEstablishesCompositionInvariant
  <1>3. IndexedCompositionInvariant
           /\ IndexedJoinedThroughLocalHeight
           /\ [IndexedChainNext]_IndexedChainVars
           => IndexedJoinedThroughLocalHeight'
    BY IndexedStepPreservesJoinedThroughLocalHeight
  <1> QED BY <1>1, <1>2, <1>3, PTL DEF IndexedChainSpec

THEOREM IndexedSpecPreservesJoinedRouting ==
  IndexedChainSpec => []JoinedRoutingInvariant
BY IndexedChainSpecEstablishesCompositionInvariant, PTL
   DEF IndexedCompositionInvariant

THEOREM IndexedSpecPreservesJoinedCertification ==
  IndexedChainSpec => []JoinedContextCertificationInvariant
BY IndexedChainSpecEstablishesCompositionInvariant, PTL
   DEF IndexedCompositionInvariant

THEOREM JoinedNonCurrentHasApplicationEvidence ==
  \A initialContext \in AdmissibleContextRecords:
    \A node \in joinedByContext[initialContext]:
      (IndexedCompositionInvariant
        /\ ~IndexedNodeCurrentAt(initialContext, node))
        => /\ nodeHeight[node] > initialContext.height
           /\ IndexedAsync(initialContext)!NodeHasApplication(node)
BY DEF IndexedCompositionInvariant, JoinedRoutingInvariant

THEOREM IndexedCurrentNodeHasExactLocation ==
  \A initialContext \in AdmissibleContextRecords,
     node \in ValidatorIds:
    /\ IndexedCompositionInvariant
    /\ IndexedNodeCurrentAt(initialContext, node)
    => ExactNodeLocationAt(initialContext, node)
BY Isa
   DEF IndexedCompositionInvariant, IndexedNodeCurrentAt,
       ExactNodeLocationAt,
       Chain!ChainEpochInvariant, Chain!ChainEpochTypeInvariant,
       Chain!ContextsMatchLocalHistories

THEOREM JoinedNonCurrentDisablesExactRunNode ==
  \A initialContext \in AdmissibleContextRecords:
    \A node \in joinedByContext[initialContext]:
      (IndexedCompositionInvariant
        /\ ~IndexedNodeCurrentAt(initialContext, node))
        => ~IndexedAsync(initialContext)!RunNode(node)
BY Isa, JoinedNonCurrentHasApplicationEvidence
   DEF IndexedCompositionInvariant,
       IndexedAsync!RunNode, IndexedAsync!AsyncVotersAt

THEOREM ExactHistoricalRecoveryTargetOwnsCurrentLocation ==
  \A initialContext \in AdmissibleContextRecords,
     node \in ValidatorIds:
    IndexedCompositionInvariant
      /\ IndexedAsync(initialContext)!HistoricalRecoveryTarget(node)
      => /\ node \in Responsive
         /\ ExactNodeLocationAt(initialContext, node)
         /\ ~IndexedAsync(initialContext)!NodeHasApplication(node)
BY DEF IndexedCompositionInvariant,
       IndexedHistoricalRecoveryTargetCoherence

(***************************************************************************
Product enabledness is proved, not assumed through hiding. The strong exact
instance invariant types a fresh receipt and supplies per-context agreement;
the receipt projection identifies already certified decisions. Joined-context
certification selects RecordCertifiedNext versus RecordKnownDecision, while
routing and the certified height select RecordAppliedNext versus
RecordKnownApplication. AppliedSuccessorIsAdmissible guarantees that the
queued successor context stays inside the pre-created function domain; the
kind-specific activation publication performs the later join.
***************************************************************************)
THEOREM IndexedReceiptFreeActionHasProductExtension ==
  \A initialContext \in AdmissibleContextRecords:
    (IndexedCompositionInvariant
      /\ initialContext \in JoinedContexts
      /\ ENABLED IndexedReceiptFreeAsyncAction(initialContext))
      => ENABLED
           (/\ IndexedProductActionAt(initialContext)
            /\ IndexedReceiptFreeAsyncAction(initialContext))
BY Isa DEF IndexedProductActionAt, IndexedReceiptFreeAsyncAction,
           IndexedReceiptClassification,
           IndexedReceiptFreeChainStutter,
           IndexedCompositionInvariant, IndexedAsyncStateShape,
           JoinedByContextShape,
           IndexedChainVars

THEOREM IndexedFreshReceiptActionHasProductExtension ==
  \A initialContext \in AdmissibleContextRecords:
    (IndexedCompositionInvariant
      /\ initialContext \in JoinedContexts
      /\ ENABLED IndexedFreshReceiptAsyncAction(initialContext))
      => ENABLED
           (/\ IndexedProductActionAt(initialContext)
            /\ IndexedFreshReceiptAsyncAction(initialContext))
BY Isa, AppliedSuccessorIsAdmissible
   DEF IndexedFreshReceiptAsyncAction, IndexedProductActionAt,
       IndexedReceiptClassification,
       IndexedDecisionReceiptHandoff,
       IndexedApplicationReceiptHandoff,
       NewIndexedDecisionReceipt, NewIndexedApplicationReceipt,
       IndexedCompositionInvariant,
       IndexedEveryInstanceStrongInvariant,
       JoinedContextCertificationInvariant, JoinedRoutingInvariant,
       IndexedApplicationsRespectNodeHeight,
       IndexedTotalReceiptProjection,
       IndexedDecisionReceiptProjection,
       IndexedApplicationReceiptProjection,
       IndexedDecisionEvidence, IndexedApplicationEvidence,
       IndexedCurrentDecisions, IndexedCurrentApplications,
       IndexedNodeCurrentAt, ExactNodeLocationAt,
       JoinedContexts, SuccessorContextFor,
       Chain!ChainEpochInvariant, Chain!ChainEpochTypeInvariant,
       Chain!DecisionBacksCertifiedSlot,
       Chain!ReceiptOutsideChainHorizon,
       Chain!RecordCertifiedNext, Chain!RecordKnownDecision,
       Chain!RecordAppliedNext, Chain!RecordKnownApplication,
       IndexedAsync!StrongInductiveInvariant,
       IndexedAsync!Safety, IndexedAsync!TypeInvariant,
       IndexedAsync!DecisionAgreement,
       IndexedAsync!AppliedRequiresDecision,
       IndexedAsync!NodeHasApplication

THEOREM IndexedJoinedActionHasProductExtension ==
  \A initialContext \in AdmissibleContextRecords:
    (IndexedCompositionInvariant
      /\ initialContext \in JoinedContexts
      /\ ENABLED IndexedJoinedAsyncNext(initialContext))
      => ENABLED IndexedProductActionAt(initialContext)
BY Isa, IndexedReceiptFreeActionHasProductExtension,
   IndexedFreshReceiptActionHasProductExtension
   DEF IndexedReceiptFreeAsyncAction,
       IndexedFreshReceiptAsyncAction,
       IndexedCompositionInvariant,
       IndexedEveryInstanceStrongInvariant,
       IndexedAsync!StrongInductiveInvariant,
       IndexedAsync!Safety, IndexedAsync!TypeInvariant,
       IndexedAsync!Next, IndexedAsync!PersistDecision,
       IndexedAsync!ApplyDecision,
       NoNewIndexedDurableReceipt,
       NewIndexedDecisionReceipt, NewIndexedApplicationReceipt

THEOREM IndexedFairActionsRemainEnabledInProduct ==
  \A initialContext \in AdmissibleContextRecords:
    (IndexedCompositionInvariant
      /\ initialContext \in JoinedContexts)
      => /\ (ENABLED IndexedAsync(initialContext)!AsyncSetGST
                => ENABLED IndexedSetGstStep(initialContext))
         /\ (ENABLED IndexedAsync(initialContext)!AsyncTick
                => ENABLED IndexedTickStep(initialContext))
         /\ \A node \in IndexedAsync(initialContext)!
                       AsyncVotersAt(initialContext):
              node \in joinedByContext[initialContext]
                => /\ (ENABLED IndexedAsync(initialContext)!
                                  PostGstRunNode(node)
                          => ENABLED
                               IndexedRunNodeStep(initialContext, node))
                   /\ (ENABLED IndexedAsync(initialContext)!
                                  PostGstCommitCertificateDiscovery(node)
                          => ENABLED
                               IndexedCommitCertificateDiscoveryStep(
                                 initialContext, node))
                   /\ (ENABLED IndexedAsync(initialContext)!
                                  PostGstResolveLocalCandidateProducerContinuation(
                                    node)
                          => ENABLED
                               IndexedResolveLocalProducerContinuationStep(
                                 initialContext, node))
                   /\ (ENABLED IndexedAsync(initialContext)!
                                  PostGstServiceConditionalTransportProducerContinuation(
                                    node)
                          => ENABLED
                               IndexedServiceConditionalProducerContinuationStep(
                                 initialContext, node))
                   /\ (ENABLED IndexedAsync(initialContext)!
                                  PostGstServiceVolatileBodyProducerContinuation(
                                    node)
                          => ENABLED
                               IndexedServiceVolatileProducerContinuationStep(
                                 initialContext, node))
         /\ \A node \in Responsive:
              node \in joinedByContext[initialContext]
                => /\ (ENABLED IndexedAsync(initialContext)!
                                  PostGstRunHistoricalServer(node)
                          => ENABLED IndexedHistoricalServerStep(
                               initialContext, node))
                   /\ (ENABLED IndexedAsync(initialContext)!
                                  PostGstServiceIoWorker(node)
                          => ENABLED
                               IndexedIoWorkerStep(initialContext, node))
                   /\ (ENABLED IndexedAsync(initialContext)!
                                  PostGstOpenHistoricalRecovery(node)
                          => ENABLED IndexedOpenHistoricalRecoveryStep(
                               initialContext, node))
                   /\ (ENABLED IndexedAsync(initialContext)!
                                  PostGstRunHistoricalRecoveryNode(node)
                          => ENABLED IndexedRunHistoricalRecoveryStep(
                               initialContext, node))
                   /\ (ENABLED IndexedAsync(initialContext)!
                                  PostGstHistoricalCommitCertificateDiscovery(
                                    node)
                          => ENABLED
                               IndexedHistoricalCommitCertificateDiscoveryStep(
                                 initialContext, node))
                   /\ (ENABLED IndexedAsync(initialContext)!
                                  PostGstServiceHistoricalRecoveryIoWorker(node)
                          => ENABLED
                               IndexedHistoricalRecoveryIoWorkerStep(
                                 initialContext, node))
         /\ \A slot \in IndexedAsync(initialContext)!
                       AsyncLeaderWireLifecycleSlotSet:
              ENABLED IndexedAsync(initialContext)!
                        PostGstRetireLeaderWireLifecycleSlot(slot)
                => ENABLED IndexedRetireLeaderWireLifecycleStep(
                     initialContext, slot)
         /\ \A recipient \in Responsive,
               source \in IndexedAsync(initialContext)!
                         AsyncIngressSources:
              ENABLED IndexedAsync(initialContext)!
                        PostGstAdmitHiddenPacket(recipient, source)
                => ENABLED IndexedAdmitPacketStep(
                     initialContext, recipient, source)
         /\ \A recipient \in ValidatorIds,
               source \in IndexedAsync(initialContext)!
                          AsyncIngressSources:
              ENABLED IndexedAsync(initialContext)!
                        PostGstAdmitHistoricalRecoveryPacket(
                          recipient, source)
                => ENABLED IndexedAdmitHistoricalRecoveryPacketStep(
                     initialContext, recipient, source)
BY Isa, IndexedJoinedActionHasProductExtension,
   JoinedNonCurrentDisablesExactRunNode,
   ExactHistoricalRecoveryTargetOwnsCurrentLocation
   DEF IndexedSetGstStep, IndexedTickStep, IndexedRunNodeStep,
       IndexedOpenHistoricalRecoveryStep,
       IndexedRunHistoricalRecoveryStep,
       IndexedCommitCertificateDiscoveryStep,
       IndexedHistoricalCommitCertificateDiscoveryStep,
       IndexedHistoricalServerStep, IndexedIoWorkerStep,
       IndexedHistoricalRecoveryIoWorkerStep,
       IndexedResolveLocalProducerContinuationStep,
       IndexedServiceConditionalProducerContinuationStep,
       IndexedServiceVolatileProducerContinuationStep,
       IndexedRetireLeaderWireLifecycleStep,
       IndexedAdmitPacketStep, IndexedChainNext,
       IndexedAdmitHistoricalRecoveryPacketStep,
       IndexedOpenHistoricalRecovery,
       IndexedHistoricalRecoveryTargetReady,
       IndexedHistoricalRecoverySourceReady,
       IndexedProductActionAt, IndexedJoinedAsyncNext,
       IndexedJoinedNonCrashStep, IndexedJoinedRunnerStep,
       IndexedJoinedNonRunnerStep, IndexedNodeCurrentAt,
       IndexedAsync!PostGstRunNode,
       IndexedAsync!PostGstOpenHistoricalRecovery,
       IndexedAsync!PostGstRunHistoricalRecoveryNode,
       IndexedAsync!PostGstCommitCertificateDiscovery,
       IndexedAsync!PostGstResolveLocalCandidateProducerContinuation,
       IndexedAsync!PostGstServiceConditionalTransportProducerContinuation,
       IndexedAsync!PostGstServiceVolatileBodyProducerContinuation,
       IndexedAsync!PostGstRetireLeaderWireLifecycleSlot,
       IndexedAsync!PostGstHistoricalCommitCertificateDiscovery,
       IndexedAsync!PostGstRunHistoricalServer,
       IndexedAsync!PostGstServiceIoWorker,
       IndexedAsync!PostGstServiceHistoricalRecoveryIoWorker,
       IndexedAsync!PostGstAdmitHiddenPacket,
       IndexedAsync!PostGstAdmitHistoricalRecoveryPacket,
       IndexedAsync!AsyncNonCrashStep,
       IndexedAsync!AsyncRunnerStep,
       IndexedAsync!AsyncNonRunnerStep,
       IndexedAsync!AsyncNext

THEOREM IndexedChainSpecRefinesChainEpochSpec ==
  IndexedChainSpec => Chain!ChainEpochSpec
PROOF
  <1>1. IndexedChainInit => Chain!ChainEpochInit
    BY DEF IndexedChainInit
  <1>2. IndexedChainNext
           => [Chain!ChainEpochNext]_Chain!ChainEpochVars
    BY IndexedStepProjectsChainEpochStep
  <1> QED BY <1>1, <1>2, PTL
     DEF IndexedChainSpec, Chain!ChainEpochSpec

THEOREM IndexedChainSafety ==
  IndexedChainSpec => []Chain!ChainEpochSafety
PROOF
  <1>1. IndexedChainSpec => Chain!ChainEpochSpec
    BY IndexedChainSpecRefinesChainEpochSpec
  <1>2. Chain!ChainEpochSpec => []Chain!ChainEpochSafety
    BY Chain!ChainPrefixAndEpochSafety
  <1> QED BY <1>1, <1>2, PTL

(***************************************************************************
Temporal induction interface.

IndexedInstanceActivationObligation is the suffix argument: once the finite
prior-height application induction has eventually joined every responsive
validator, the already-running restricted behavior satisfies the exact
AsyncSpecAt fairness obligations. Early joined work is part of that same
behavior and is never blocked. IndexedFairActionsRemainEnabledInProduct proves
that the receipt wrapper does not hide enabled exact actions. Once a joined
node is no longer current, JoinedNonCurrentDisablesExactRunNode makes its exact
RunNode fairness obligation vacuous while historical service stays fair.
Historical recovery is owned by the exact Async target and its ordinary
decision/body/store/validate/apply corridor; its remaining temporal debt is
the explicit IndexedExactHistoricalRecoveryProgress premise of the conditional
height kernel. Terminal application has no successor join.
VerificationOneHeightCompletion is the exact fixed-context expansion of the
one-height completion property over the parameterized production-network
instance. Its wrapper is supplied by the exact asynchronous temporal closure,
after rotating-leader convergence and exact Decision-stage application service
have closed.  The conditional final proof composes explicit premises over
finite Heights; it does not hide them as a new protocol relation.
***************************************************************************)
IndexedAllResponsiveJoined(initialContext) ==
  Responsive \subseteq joinedByContext[initialContext]

THEOREM IndexedResponsiveVoterSetIsNonempty ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedAsync(initialContext)!AsyncVotersAt(initialContext) # {}
BY Isa DEF AdmissibleContextRecords, FrozenContextAdmissible,
           IndexedAsync!AsyncVotersAt, ModelConfiguration,
           DualQuorum, CountQuorum, QuorumConfiguration,
           ContextRecords, LineagesAt, Heights

THEOREM IndexedAllResponsiveJoinedMakesContextJoined ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedAllResponsiveJoined(initialContext)
      => initialContext \in JoinedContexts
BY Isa, IndexedResponsiveVoterSetIsNonempty
   DEF IndexedAllResponsiveJoined, JoinedContexts,
       IndexedAsync!AsyncVotersAt

THEOREM IndexedAllResponsiveJoinedIsStable ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedAllResponsiveJoined(initialContext)
      /\ [IndexedChainNext]_IndexedChainVars
      => IndexedAllResponsiveJoined(initialContext)'
BY Isa, JoinedMembershipIsMonotone
   DEF IndexedAllResponsiveJoined, IndexedChainVars

THEOREM IndexedAllResponsiveJoinedHasActiveRoster ==
  \A initialContext \in AdmissibleContextRecords:
    /\ IndexedCompositionInvariant
    /\ IndexedAllResponsiveJoined(initialContext)
    => Responsive \subseteq
         IndexedAsync(initialContext)!AsyncActiveServiceNodes
BY Isa
   DEF IndexedCompositionInvariant,
       IndexedAllResponsiveJoined,
       IndexedServiceActivationCoherence,
       IndexedServiceActivationMembershipCoherenceAt,
       IndexedAsync!AsyncServiceActivationRestricted,
       IndexedAsync!AsyncActiveServiceNodes

THEOREM IndexedChainSpecKeepsGenesisResponsiveRosterActive ==
  IndexedChainSpec
    => [](Responsive \subseteq
          IndexedAsync(GenesisContext)!AsyncActiveServiceNodes)
BY IndexedChainSpecEstablishesCompositionInvariant,
   IndexedAllResponsiveJoinedHasActiveRoster, PTL, Isa
   DEF IndexedChainInit, IndexedChainSpec,
       IndexedAllResponsiveJoined, GenesisContext,
       AdmissibleContextRecords, FrozenContextAdmissible,
       ContextRecords, LineagesAt, Heights,
       ModelConfiguration, ValidatorIds

IndexedActivationStable(initialContext) ==
  /\ IndexedCompositionInvariant
  /\ IndexedAllResponsiveJoined(initialContext)
  /\ initialContext \in JoinedContexts

THEOREM IndexedActivationEventuallyStabilizes ==
  \A initialContext \in AdmissibleContextRecords:
    (/\ IndexedChainSpec
     /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
      => <>[]IndexedActivationStable(initialContext)
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords
         PROVE
           (/\ IndexedChainSpec
            /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
             => <>[]IndexedActivationStable(initialContext)
    <2>1. IndexedChainSpec => []IndexedCompositionInvariant
      BY IndexedChainSpecEstablishesCompositionInvariant
    <2>2. IndexedAllResponsiveJoined(initialContext)
             /\ [IndexedChainNext]_IndexedChainVars
             => IndexedAllResponsiveJoined(initialContext)'
      BY <1>1, IndexedAllResponsiveJoinedIsStable
    <2>3. IndexedAllResponsiveJoined(initialContext)
             => initialContext \in JoinedContexts
      BY <1>1, IndexedAllResponsiveJoinedMakesContextJoined
    <2> QED BY <2>1, <2>2, <2>3, PTL
         DEF IndexedChainSpec, IndexedActivationStable
  <1> QED BY <1>1

(***************************************************************************
The product never leaves the Eligible recovery phase, so the six pre-GST
restart/replay actions required by AsyncFairnessAt are permanently disabled.
Their weak-fairness clauses are therefore satisfied semantically, rather than
being dropped from the exact AsyncSpecAt projection.
***************************************************************************)
IndexedResponsiveRecoveryActionsDisabled ==
  \A initialContext \in AdmissibleContextRecords:
    /\ ~ENABLED
          <<IndexedAsync(initialContext)!PreGstResponsiveRestart>>_(
            IndexedAsyncStateAt(initialContext))
    /\ ~ENABLED
          <<IndexedAsync(initialContext)!PreGstResponsiveReplay>>_(
            IndexedAsyncStateAt(initialContext))
    /\ ~ENABLED
          <<IndexedAsync(initialContext)!ResponsiveReplayRunNode>>_(
            IndexedAsyncStateAt(initialContext))
    /\ ~ENABLED
          <<IndexedAsync(initialContext)!
              ResponsiveReplayServiceIoWorker>>_(
            IndexedAsyncStateAt(initialContext))
    /\ ~ENABLED
          <<IndexedAsync(initialContext)!DriveResponsiveReplayHead>>_(
            IndexedAsyncStateAt(initialContext))
    /\ ~ENABLED
          <<IndexedAsync(initialContext)!FinishResponsiveReplay>>_(
            IndexedAsyncStateAt(initialContext))

THEOREM IndexedResponsiveRecoveryDormancyDisablesFairActions ==
  IndexedResponsiveRecoveryDormant
    => IndexedResponsiveRecoveryActionsDisabled
BY ExpandENABLED, Isa
   DEF IndexedResponsiveRecoveryDormant,
       IndexedResponsiveRecoveryActionsDisabled,
       IndexedAsyncStateAt, IndexedRecovery,
       IndexedAsync!PreGstResponsiveRestart,
       IndexedAsync!PreGstResponsiveReplay,
       IndexedAsync!ResponsiveReplayRunNode,
       IndexedAsync!ResponsiveReplayServiceIoWorker,
       IndexedAsync!DriveResponsiveReplayHead,
       IndexedAsync!FinishResponsiveReplay,
       IndexedAsync!ResponsiveReplayDraining

THEOREM IndexedChainSpecAlwaysDisablesResponsiveRecoveryActions ==
  IndexedChainSpec => []IndexedResponsiveRecoveryActionsDisabled
BY IndexedChainSpecKeepsResponsiveRecoveryDormant,
   IndexedResponsiveRecoveryDormancyDisablesFairActions, PTL

THEOREM IndexedResponsiveRecoveryFairnessIsVacuous ==
  IndexedChainSpec
    => \A initialContext \in AdmissibleContextRecords:
         /\ WF_(IndexedAsyncStateAt(initialContext))(
              IndexedAsync(initialContext)!PreGstResponsiveRestart)
         /\ WF_(IndexedAsyncStateAt(initialContext))(
              IndexedAsync(initialContext)!PreGstResponsiveReplay)
         /\ WF_(IndexedAsyncStateAt(initialContext))(
              IndexedAsync(initialContext)!ResponsiveReplayRunNode)
         /\ WF_(IndexedAsyncStateAt(initialContext))(
              IndexedAsync(initialContext)!
                ResponsiveReplayServiceIoWorker)
         /\ WF_(IndexedAsyncStateAt(initialContext))(
              IndexedAsync(initialContext)!DriveResponsiveReplayHead)
         /\ WF_(IndexedAsyncStateAt(initialContext))(
              IndexedAsync(initialContext)!FinishResponsiveReplay)
PROOF
  <1>1. ASSUME IndexedChainSpec,
               NEW initialContext \in AdmissibleContextRecords
         PROVE
           /\ WF_(IndexedAsyncStateAt(initialContext))(
                IndexedAsync(initialContext)!PreGstResponsiveRestart)
           /\ WF_(IndexedAsyncStateAt(initialContext))(
                IndexedAsync(initialContext)!PreGstResponsiveReplay)
           /\ WF_(IndexedAsyncStateAt(initialContext))(
                IndexedAsync(initialContext)!ResponsiveReplayRunNode)
           /\ WF_(IndexedAsyncStateAt(initialContext))(
                IndexedAsync(initialContext)!
                  ResponsiveReplayServiceIoWorker)
           /\ WF_(IndexedAsyncStateAt(initialContext))(
                IndexedAsync(initialContext)!DriveResponsiveReplayHead)
           /\ WF_(IndexedAsyncStateAt(initialContext))(
                IndexedAsync(initialContext)!FinishResponsiveReplay)
    <2>1. /\ []~ENABLED
                  <<IndexedAsync(initialContext)!
                      PreGstResponsiveRestart>>_(
                    IndexedAsyncStateAt(initialContext))
            /\ []~ENABLED
                  <<IndexedAsync(initialContext)!
                      PreGstResponsiveReplay>>_(
                    IndexedAsyncStateAt(initialContext))
            /\ []~ENABLED
                  <<IndexedAsync(initialContext)!
                      ResponsiveReplayRunNode>>_(
                    IndexedAsyncStateAt(initialContext))
            /\ []~ENABLED
                  <<IndexedAsync(initialContext)!
                      ResponsiveReplayServiceIoWorker>>_(
                    IndexedAsyncStateAt(initialContext))
            /\ []~ENABLED
                  <<IndexedAsync(initialContext)!
                      DriveResponsiveReplayHead>>_(
                    IndexedAsyncStateAt(initialContext))
            /\ []~ENABLED
                  <<IndexedAsync(initialContext)!
                      FinishResponsiveReplay>>_(
                    IndexedAsyncStateAt(initialContext))
      BY <1>1, IndexedChainSpecAlwaysDisablesResponsiveRecoveryActions, PTL
         DEF IndexedResponsiveRecoveryActionsDisabled
    <2> QED BY <2>1, PTL
  <1> QED BY <1>1

(***************************************************************************
The standalone Async specification weakly fairly rearms every Responsive
service owner.  In the indexed product, rearm is fused into the corresponding
successor publication.  Once the activation premise has joined every
Responsive node, component-46 coherence makes every standalone rearm action
permanently disabled, so those exact weak-fairness clauses hold vacuously.
***************************************************************************)
IndexedResponsiveServiceActivationActionsDisabledAt(initialContext) ==
  \A node \in Responsive:
    ~ENABLED
       <<IndexedAsync(initialContext)!AsyncActivateServiceNode(node)>>_(
         IndexedAsyncStateAt(initialContext))

THEOREM IndexedActivationStableDisablesResponsiveServiceActivation ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedActivationStable(initialContext)
      => IndexedResponsiveServiceActivationActionsDisabledAt(
           initialContext)
BY ExpandENABLED, Isa
   DEF IndexedActivationStable,
       IndexedAllResponsiveJoined,
       IndexedCompositionInvariant,
       IndexedServiceActivationCoherence,
       IndexedServiceActivationMembershipCoherenceAt,
       IndexedResponsiveServiceActivationActionsDisabledAt,
       IndexedAsync!AsyncActivateServiceNode,
       IndexedAsync!AsyncServiceActivationRestricted,
       IndexedAsync!AsyncActiveServiceNodes,
       IndexedAsyncStateAt

THEOREM IndexedResponsiveServiceActivationFairnessIsVacuous ==
  \A initialContext \in AdmissibleContextRecords:
    (/\ IndexedChainSpec
     /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
      => \A node \in Responsive:
           WF_(IndexedAsyncStateAt(initialContext))(
             IndexedAsync(initialContext)!AsyncActivateServiceNode(node))
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
               /\ IndexedChainSpec
                  /\ TRUE ~> IndexedAllResponsiveJoined(initialContext)
         PROVE \A node \in Responsive:
                 WF_(IndexedAsyncStateAt(initialContext))(
                   IndexedAsync(initialContext)!
                     AsyncActivateServiceNode(node))
    <2>1. <>[]IndexedActivationStable(initialContext)
      BY <1>1, IndexedActivationEventuallyStabilizes
    <2>2. [](IndexedActivationStable(initialContext)
               => IndexedResponsiveServiceActivationActionsDisabledAt(
                    initialContext))
      BY IndexedActivationStableDisablesResponsiveServiceActivation, PTL
    <2>3. \A node \in Responsive:
             <>[]~ENABLED
               <<IndexedAsync(initialContext)!
                    AsyncActivateServiceNode(node)>>_(
                 IndexedAsyncStateAt(initialContext))
      BY <2>1, <2>2, PTL
         DEF IndexedResponsiveServiceActivationActionsDisabledAt
    <2> QED BY <2>3, PTL
  <1> QED BY <1>1

(***************************************************************************
Every weakly fair product action is an exact nonstuttering action of the
selected Async instance.  Conversely, after activation the product
enabledness theorem extends every exact nonstuttering witness to the paired
receipt/ChainEpoch transition.  These two directions are the concrete
fairness-refinement argument; no fairness clause is added to the projection.
***************************************************************************)
THEOREM IndexedFairProductStepsProjectExactOccurrences ==
  \A initialContext \in AdmissibleContextRecords:
    /\ (IndexedSetGstStep(initialContext)
          => <<IndexedAsync(initialContext)!AsyncSetGST>>_(
               IndexedAsyncStateAt(initialContext)))
    /\ (IndexedTickStep(initialContext)
          => <<IndexedAsync(initialContext)!AsyncTick>>_(
               IndexedAsyncStateAt(initialContext)))
    /\ \A node \in IndexedAsync(initialContext)!
                    AsyncVotersAt(initialContext):
         /\ (IndexedRunNodeStep(initialContext, node)
               => <<IndexedAsync(initialContext)!
                       PostGstRunNode(node)>>_(
                    IndexedAsyncStateAt(initialContext)))
         /\ (IndexedCommitCertificateDiscoveryStep(
                  initialContext, node)
               => <<IndexedAsync(initialContext)!
                       PostGstCommitCertificateDiscovery(node)>>_(
                    IndexedAsyncStateAt(initialContext)))
         /\ (IndexedResolveLocalProducerContinuationStep(
                  initialContext, node)
               => <<IndexedAsync(initialContext)!
                       PostGstResolveLocalCandidateProducerContinuation(
                         node)>>_(IndexedAsyncStateAt(initialContext)))
         /\ (IndexedServiceConditionalProducerContinuationStep(
                  initialContext, node)
               => <<IndexedAsync(initialContext)!
                       PostGstServiceConditionalTransportProducerContinuation(
                         node)>>_(IndexedAsyncStateAt(initialContext)))
         /\ (IndexedServiceVolatileProducerContinuationStep(
                  initialContext, node)
               => <<IndexedAsync(initialContext)!
                       PostGstServiceVolatileBodyProducerContinuation(
                         node)>>_(IndexedAsyncStateAt(initialContext)))
    /\ \A node \in Responsive:
         /\ (IndexedHistoricalServerStep(initialContext, node)
               => <<IndexedAsync(initialContext)!
                       PostGstRunHistoricalServer(node)>>_(
                    IndexedAsyncStateAt(initialContext)))
         /\ (IndexedIoWorkerStep(initialContext, node)
               => <<IndexedAsync(initialContext)!
                       PostGstServiceIoWorker(node)>>_(
                    IndexedAsyncStateAt(initialContext)))
         /\ (IndexedOpenHistoricalRecoveryStep(initialContext, node)
               => <<IndexedAsync(initialContext)!
                       PostGstOpenHistoricalRecovery(node)>>_(
                    IndexedAsyncStateAt(initialContext)))
         /\ (IndexedRunHistoricalRecoveryStep(initialContext, node)
               => <<IndexedAsync(initialContext)!
                       PostGstRunHistoricalRecoveryNode(node)>>_(
                    IndexedAsyncStateAt(initialContext)))
         /\ (IndexedHistoricalCommitCertificateDiscoveryStep(
                  initialContext, node)
               => <<IndexedAsync(initialContext)!
                       PostGstHistoricalCommitCertificateDiscovery(node)>>_(
                    IndexedAsyncStateAt(initialContext)))
         /\ (IndexedHistoricalRecoveryIoWorkerStep(
                  initialContext, node)
               => <<IndexedAsync(initialContext)!
                       PostGstServiceHistoricalRecoveryIoWorker(node)>>_(
                    IndexedAsyncStateAt(initialContext)))
    /\ \A slot \in IndexedAsync(initialContext)!
                  AsyncLeaderWireLifecycleSlotSet:
         IndexedRetireLeaderWireLifecycleStep(initialContext, slot)
           => <<IndexedAsync(initialContext)!
                   PostGstRetireLeaderWireLifecycleSlot(slot)>>_(
                IndexedAsyncStateAt(initialContext))
    /\ \A recipient \in Responsive,
          source \in IndexedAsync(initialContext)!
                    AsyncIngressSources:
         IndexedAdmitPacketStep(initialContext, recipient, source)
           => <<IndexedAsync(initialContext)!
                   PostGstAdmitHiddenPacket(recipient, source)>>_(
                IndexedAsyncStateAt(initialContext))
    /\ \A recipient \in ValidatorIds,
          source \in IndexedAsync(initialContext)!AsyncIngressSources:
         IndexedAdmitHistoricalRecoveryPacketStep(
           initialContext, recipient, source)
           => <<IndexedAsync(initialContext)!
                   PostGstAdmitHistoricalRecoveryPacket(
                     recipient, source)>>_(
                IndexedAsyncStateAt(initialContext))
BY Isa DEF IndexedSetGstStep, IndexedTickStep,
           IndexedRunNodeStep, IndexedHistoricalServerStep,
           IndexedOpenHistoricalRecoveryStep,
           IndexedRunHistoricalRecoveryStep,
           IndexedCommitCertificateDiscoveryStep,
           IndexedHistoricalCommitCertificateDiscoveryStep,
           IndexedIoWorkerStep, IndexedHistoricalRecoveryIoWorkerStep,
           IndexedResolveLocalProducerContinuationStep,
           IndexedServiceConditionalProducerContinuationStep,
           IndexedServiceVolatileProducerContinuationStep,
           IndexedRetireLeaderWireLifecycleStep,
           IndexedAdmitPacketStep,
           IndexedAdmitHistoricalRecoveryPacketStep,
           IndexedChainNext, IndexedProductActionAt,
           IndexedReceiptClassification, IndexedReceiptFreeChainStutter,
           IndexedDecisionReceiptHandoff,
           IndexedApplicationReceiptHandoff,
           IndexedChainVars, IndexedAsyncStateAt,
           IndexedAsync!AsyncSetGST, IndexedAsync!SetGST,
           IndexedAsync!AsyncTick,
           IndexedAsync!PostGstRunNode, IndexedAsync!RunNode,
           IndexedAsync!PostGstOpenHistoricalRecovery,
           IndexedAsync!OpenHistoricalRecovery,
           IndexedAsync!PostGstRunHistoricalRecoveryNode,
           IndexedAsync!RunHistoricalRecoveryNode,
           IndexedAsync!PostGstCommitCertificateDiscovery,
           IndexedAsync!DirectCommitCertificateDiscoveryStep,
           IndexedAsync!PostGstHistoricalCommitCertificateDiscovery,
           IndexedAsync!DirectHistoricalCommitCertificateDiscoveryStep,
           IndexedAsync!PostGstRunHistoricalServer,
           IndexedAsync!RunHistoricalServer,
           IndexedAsync!PostGstServiceIoWorker,
           IndexedAsync!ServiceIoWorker,
           IndexedAsync!PostGstServiceHistoricalRecoveryIoWorker,
           IndexedAsync!ServiceHistoricalRecoveryIoWorker,
           IndexedAsync!PostGstResolveLocalCandidateProducerContinuation,
           IndexedAsync!PostGstServiceConditionalTransportProducerContinuation,
           IndexedAsync!PostGstServiceVolatileBodyProducerContinuation,
           IndexedAsync!PostGstRetireLeaderWireLifecycleSlot,
           IndexedAsync!PostGstAdmitHiddenPacket,
           IndexedAsync!PostGstAdmitHistoricalRecoveryPacket,
           IndexedAsync!AdmitHiddenPacket

(***************************************************************************
Activation-local historical non-packet fairness.

Unlike the aggregate AsyncSpecAt projection below, these bridges do not wait
for all Responsive peers.  Each post-GST exact action exposes its own active
owner in its guard.  The composition invariant maps that owner to joined
membership, which is exactly the enabledness premise of the corresponding
weakly fair product action.  Tick is guarded explicitly by GST because bare
AsyncTick is intentionally enabled in dormant pre-GST instances.
***************************************************************************)
THEOREM IndexedPostGstHistoricalFairOccurrencesEnableProduct ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedCompositionInvariant
      => /\ (ENABLED
                <<IndexedPostGstTick(initialContext)>>_(
                  IndexedAsyncStateAt(initialContext))
                => ENABLED
                     <<IndexedTickStep(initialContext)>>_(
                       IndexedChainVars))
         /\ \A node \in IndexedAsync(initialContext)!
                       AsyncVotersAt(initialContext):
              /\ (ENABLED
                    <<IndexedAsync(initialContext)!
                        PostGstRunNode(node)>>_(
                      IndexedAsyncStateAt(initialContext))
                    => ENABLED
                         <<IndexedRunNodeStep(initialContext, node)>>_(
                           IndexedChainVars))
              /\ (ENABLED
                    <<IndexedAsync(initialContext)!
                        PostGstResolveLocalCandidateProducerContinuation(
                          node)>>_(IndexedAsyncStateAt(initialContext))
                    => ENABLED
                         <<IndexedResolveLocalProducerContinuationStep(
                             initialContext, node)>>_(IndexedChainVars))
              /\ (ENABLED
                    <<IndexedAsync(initialContext)!
                        PostGstServiceConditionalTransportProducerContinuation(
                          node)>>_(IndexedAsyncStateAt(initialContext))
                    => ENABLED
                         <<IndexedServiceConditionalProducerContinuationStep(
                             initialContext, node)>>_(IndexedChainVars))
              /\ (ENABLED
                    <<IndexedAsync(initialContext)!
                        PostGstServiceVolatileBodyProducerContinuation(
                          node)>>_(IndexedAsyncStateAt(initialContext))
                    => ENABLED
                         <<IndexedServiceVolatileProducerContinuationStep(
                             initialContext, node)>>_(IndexedChainVars))
         /\ \A slot \in IndexedAsync(initialContext)!
                       AsyncLeaderWireLifecycleSlotSet:
              ENABLED
                <<IndexedAsync(initialContext)!
                    PostGstRetireLeaderWireLifecycleSlot(slot)>>_(
                  IndexedAsyncStateAt(initialContext))
                => ENABLED
                     <<IndexedRetireLeaderWireLifecycleStep(
                         initialContext, slot)>>_(IndexedChainVars)
         /\ \A recipient \in Responsive,
               source \in IndexedAsync(initialContext)!
                         AsyncIngressSources:
              ENABLED
                <<IndexedAsync(initialContext)!
                    PostGstAdmitHiddenPacket(recipient, source)>>_(
                  IndexedAsyncStateAt(initialContext))
                => ENABLED
                     <<IndexedAdmitPacketStep(
                         initialContext, recipient, source)>>_(
                       IndexedChainVars)
         /\ \A node \in Responsive:
              /\ (ENABLED
                    <<IndexedAsync(initialContext)!
                        PostGstRunHistoricalServer(node)>>_(
                      IndexedAsyncStateAt(initialContext))
                    => ENABLED
                         <<IndexedHistoricalServerStep(
                             initialContext, node)>>_(IndexedChainVars))
              /\ (ENABLED
                    <<IndexedAsync(initialContext)!
                        PostGstServiceIoWorker(node)>>_(
                      IndexedAsyncStateAt(initialContext))
                    => ENABLED
                         <<IndexedIoWorkerStep(
                             initialContext, node)>>_(IndexedChainVars))
              /\ (ENABLED
                    <<IndexedAsync(initialContext)!
                        PostGstRunHistoricalRecoveryNode(node)>>_(
                      IndexedAsyncStateAt(initialContext))
                    => ENABLED
                         <<IndexedRunHistoricalRecoveryStep(
                             initialContext, node)>>_(IndexedChainVars))
              /\ (ENABLED
                    <<IndexedAsync(initialContext)!
                        PostGstServiceHistoricalRecoveryIoWorker(node)>>_(
                      IndexedAsyncStateAt(initialContext))
                    => ENABLED
                         <<IndexedHistoricalRecoveryIoWorkerStep(
                             initialContext, node)>>_(IndexedChainVars))
BY IndexedPostGstContextHasJoinedProductInstance,
   IndexedPostGstActiveServiceOwnerHasJoinedProductInstance,
   IndexedFairActionsRemainEnabledInProduct,
   IndexedFairProductStepsProjectExactOccurrences,
   ExpandENABLED, Isa
   DEF IndexedPostGstTick,
       IndexedResolveLocalProducerContinuationStep,
       IndexedServiceConditionalProducerContinuationStep,
       IndexedServiceVolatileProducerContinuationStep,
       IndexedRetireLeaderWireLifecycleStep,
       IndexedAdmitPacketStep,
       IndexedAsync!PostGstRunNode,
       IndexedAsync!PostGstResolveLocalCandidateProducerContinuation,
       IndexedAsync!PostGstServiceConditionalTransportProducerContinuation,
       IndexedAsync!PostGstServiceVolatileBodyProducerContinuation,
       IndexedAsync!PostGstRetireLeaderWireLifecycleSlot,
       IndexedAsync!PostGstAdmitHiddenPacket,
       IndexedAsync!PostGstRunHistoricalServer,
       IndexedAsync!PostGstServiceIoWorker,
       IndexedAsync!PostGstRunHistoricalRecoveryNode,
       IndexedAsync!PostGstServiceHistoricalRecoveryIoWorker,
       IndexedAsync!RunNode, IndexedAsync!RunNodeWork,
       IndexedAsync!LocalAdmissionStep,
       IndexedAsync!IngressDrainStep,
       IndexedAsync!SerializedRunnerRuntimeStep,
       IndexedAsync!SerializedRuntimeStep,
       IndexedAsync!SerializedRuntimePrecedesServeIngressStep,
       IndexedAsync!SerializedLocalPrecedesServeIngressStep,
       IndexedAsync!AsyncServeIngressTargetOnlyTurn,
       IndexedAsync!SelectedLocalAdmissionAdvance,
       IndexedAsync!RunHistoricalServer,
       IndexedAsync!RunHistoricalRecoveryNode,
       IndexedAsync!ServiceIoWorker,
       IndexedAsync!ServiceHistoricalRecoveryIoWorker,
       IndexedAsync!ServiceIoWorkerWork,
       IndexedAsync!AsyncArchiveIoServiceNodes,
       IndexedAsync!AsyncTimedServiceNodes

THEOREM IndexedPostGstTickProductStepProjectsExactOccurrence ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedAsync(initialContext)!gst
      /\ IndexedTickStep(initialContext)
      => <<IndexedPostGstTick(initialContext)>>_(
           IndexedAsyncStateAt(initialContext))
BY IndexedFairProductStepsProjectExactOccurrences
   DEF IndexedPostGstTick

THEOREM IndexedPostGstTickFairnessTransfersLocally ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedChainSpec
      => WF_(IndexedAsyncStateAt(initialContext))(
           IndexedPostGstTick(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
               IndexedChainSpec
         PROVE WF_(IndexedAsyncStateAt(initialContext))(
                 IndexedPostGstTick(initialContext))
    <2>1. []IndexedCompositionInvariant
      BY <1>1, IndexedChainSpecEstablishesCompositionInvariant
    <2>2. [](IndexedCompositionInvariant
               => (ENABLED
                     <<IndexedPostGstTick(initialContext)>>_(
                       IndexedAsyncStateAt(initialContext))
                     => ENABLED
                          <<IndexedTickStep(initialContext)>>_(
                            IndexedChainVars)))
      BY <1>1,
         IndexedPostGstHistoricalFairOccurrencesEnableProduct, PTL
    <2>3. [](ENABLED
               <<IndexedPostGstTick(initialContext)>>_(
                 IndexedAsyncStateAt(initialContext))
               => IndexedAsync(initialContext)!gst)
      BY ExpandENABLED, PTL DEF IndexedPostGstTick
    <2>4. [](IndexedAsync(initialContext)!gst
               /\ IndexedTickStep(initialContext)
               => <<IndexedPostGstTick(initialContext)>>_(
                    IndexedAsyncStateAt(initialContext)))
      BY <1>1,
         IndexedPostGstTickProductStepProjectsExactOccurrence, PTL
    <2>5. WF_IndexedChainVars(IndexedTickStep(initialContext))
      BY <1>1 DEF IndexedChainSpec, IndexedFairness
    <2> QED BY <2>1, <2>2, <2>3, <2>4, <2>5, PTL
  <1> QED BY <1>1

THEOREM IndexedPostGstRunNodeFairnessTransfersLocally ==
  \A initialContext \in AdmissibleContextRecords:
    \A node \in IndexedAsync(initialContext)!
                  AsyncVotersAt(initialContext):
      IndexedChainSpec
        => WF_(IndexedAsyncStateAt(initialContext))(
             IndexedAsync(initialContext)!PostGstRunNode(node))
BY IndexedChainSpecEstablishesCompositionInvariant,
   IndexedPostGstHistoricalFairOccurrencesEnableProduct,
   IndexedFairProductStepsProjectExactOccurrences, PTL
   DEF IndexedChainSpec, IndexedFairness

THEOREM IndexedAdequateLeaderNonRunnerFairnessTransfersLocally ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedChainSpec
      => /\ \A node \in IndexedAsync(initialContext)!
                         AsyncVotersAt(initialContext):
              /\ WF_(IndexedAsyncStateAt(initialContext))(
                   IndexedAsync(initialContext)!
                     PostGstResolveLocalCandidateProducerContinuation(node))
              /\ WF_(IndexedAsyncStateAt(initialContext))(
                   IndexedAsync(initialContext)!
                     PostGstServiceConditionalTransportProducerContinuation(
                       node))
              /\ WF_(IndexedAsyncStateAt(initialContext))(
                   IndexedAsync(initialContext)!
                     PostGstServiceVolatileBodyProducerContinuation(node))
         /\ \A slot \in IndexedAsync(initialContext)!
                       AsyncLeaderWireLifecycleSlotSet:
              WF_(IndexedAsyncStateAt(initialContext))(
                IndexedAsync(initialContext)!
                  PostGstRetireLeaderWireLifecycleSlot(slot))
         /\ \A recipient \in Responsive,
               source \in IndexedAsync(initialContext)!AsyncIngressSources:
              WF_(IndexedAsyncStateAt(initialContext))(
                IndexedAsync(initialContext)!
                  PostGstAdmitHiddenPacket(recipient, source))
BY IndexedChainSpecEstablishesCompositionInvariant,
   IndexedPostGstHistoricalFairOccurrencesEnableProduct,
   IndexedFairProductStepsProjectExactOccurrences, PTL
   DEF IndexedChainSpec, IndexedFairness

THEOREM IndexedHistoricalNonPacketOwnerFairnessTransfersLocally ==
  \A initialContext \in AdmissibleContextRecords:
    \A node \in Responsive:
      IndexedChainSpec
        => /\ WF_(IndexedAsyncStateAt(initialContext))(
                  IndexedAsync(initialContext)!
                    PostGstRunHistoricalServer(node))
           /\ WF_(IndexedAsyncStateAt(initialContext))(
                  IndexedAsync(initialContext)!
                    PostGstServiceIoWorker(node))
           /\ WF_(IndexedAsyncStateAt(initialContext))(
                  IndexedAsync(initialContext)!
                    PostGstRunHistoricalRecoveryNode(node))
           /\ WF_(IndexedAsyncStateAt(initialContext))(
                  IndexedAsync(initialContext)!
                    PostGstServiceHistoricalRecoveryIoWorker(node))
BY IndexedChainSpecEstablishesCompositionInvariant,
   IndexedPostGstHistoricalFairOccurrencesEnableProduct,
   IndexedFairProductStepsProjectExactOccurrences, PTL
   DEF IndexedChainSpec, IndexedFairness

THEOREM IndexedFairExactOccurrencesEnableProductOccurrences ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedActivationStable(initialContext)
      => /\ (ENABLED
                <<IndexedAsync(initialContext)!AsyncSetGST>>_(
                  IndexedAsyncStateAt(initialContext))
                => ENABLED
                     <<IndexedSetGstStep(initialContext)>>_(
                       IndexedChainVars))
         /\ (ENABLED
               <<IndexedAsync(initialContext)!AsyncTick>>_(
                 IndexedAsyncStateAt(initialContext))
               => ENABLED
                    <<IndexedTickStep(initialContext)>>_(
                      IndexedChainVars))
         /\ \A node \in IndexedAsync(initialContext)!
                       AsyncVotersAt(initialContext):
              /\ (ENABLED
                    <<IndexedAsync(initialContext)!
                        PostGstRunNode(node)>>_(
                      IndexedAsyncStateAt(initialContext))
                    => ENABLED
                         <<IndexedRunNodeStep(initialContext, node)>>_(
                           IndexedChainVars))
              /\ (ENABLED
                    <<IndexedAsync(initialContext)!
                        PostGstCommitCertificateDiscovery(node)>>_(
                      IndexedAsyncStateAt(initialContext))
                    => ENABLED
                         <<IndexedCommitCertificateDiscoveryStep(
                             initialContext, node)>>_(IndexedChainVars))
              /\ (ENABLED
                    <<IndexedAsync(initialContext)!
                        PostGstResolveLocalCandidateProducerContinuation(
                          node)>>_(IndexedAsyncStateAt(initialContext))
                    => ENABLED
                         <<IndexedResolveLocalProducerContinuationStep(
                             initialContext, node)>>_(IndexedChainVars))
              /\ (ENABLED
                    <<IndexedAsync(initialContext)!
                        PostGstServiceConditionalTransportProducerContinuation(
                          node)>>_(IndexedAsyncStateAt(initialContext))
                    => ENABLED
                         <<IndexedServiceConditionalProducerContinuationStep(
                             initialContext, node)>>_(IndexedChainVars))
              /\ (ENABLED
                    <<IndexedAsync(initialContext)!
                        PostGstServiceVolatileBodyProducerContinuation(
                          node)>>_(IndexedAsyncStateAt(initialContext))
                    => ENABLED
                         <<IndexedServiceVolatileProducerContinuationStep(
                             initialContext, node)>>_(IndexedChainVars))
         /\ \A node \in Responsive:
              /\ (ENABLED
                    <<IndexedAsync(initialContext)!
                        PostGstRunHistoricalServer(node)>>_(
                      IndexedAsyncStateAt(initialContext))
                    => ENABLED
                         <<IndexedHistoricalServerStep(
                             initialContext, node)>>_(IndexedChainVars))
              /\ (ENABLED
                    <<IndexedAsync(initialContext)!
                        PostGstServiceIoWorker(node)>>_(
                      IndexedAsyncStateAt(initialContext))
                    => ENABLED
                         <<IndexedIoWorkerStep(initialContext, node)>>_(
                           IndexedChainVars))
              /\ (ENABLED
                    <<IndexedAsync(initialContext)!
                        PostGstOpenHistoricalRecovery(node)>>_(
                      IndexedAsyncStateAt(initialContext))
                    => ENABLED
                         <<IndexedOpenHistoricalRecoveryStep(
                             initialContext, node)>>_(IndexedChainVars))
              /\ (ENABLED
                    <<IndexedAsync(initialContext)!
                        PostGstRunHistoricalRecoveryNode(node)>>_(
                      IndexedAsyncStateAt(initialContext))
                    => ENABLED
                         <<IndexedRunHistoricalRecoveryStep(
                             initialContext, node)>>_(IndexedChainVars))
              /\ (ENABLED
                    <<IndexedAsync(initialContext)!
                        PostGstHistoricalCommitCertificateDiscovery(node)>>_(
                      IndexedAsyncStateAt(initialContext))
                    => ENABLED
                         <<IndexedHistoricalCommitCertificateDiscoveryStep(
                             initialContext, node)>>_(IndexedChainVars))
              /\ (ENABLED
                    <<IndexedAsync(initialContext)!
                        PostGstServiceHistoricalRecoveryIoWorker(node)>>_(
                      IndexedAsyncStateAt(initialContext))
                    => ENABLED
                         <<IndexedHistoricalRecoveryIoWorkerStep(
                             initialContext, node)>>_(IndexedChainVars))
         /\ \A slot \in IndexedAsync(initialContext)!
                       AsyncLeaderWireLifecycleSlotSet:
              ENABLED
                <<IndexedAsync(initialContext)!
                    PostGstRetireLeaderWireLifecycleSlot(slot)>>_(
                  IndexedAsyncStateAt(initialContext))
                => ENABLED
                     <<IndexedRetireLeaderWireLifecycleStep(
                         initialContext, slot)>>_(IndexedChainVars)
         /\ \A recipient \in Responsive,
               source \in IndexedAsync(initialContext)!
                        AsyncIngressSources:
              ENABLED
                <<IndexedAsync(initialContext)!
                    PostGstAdmitHiddenPacket(recipient, source)>>_(
                  IndexedAsyncStateAt(initialContext))
                => ENABLED
                     <<IndexedAdmitPacketStep(
                         initialContext, recipient, source)>>_(
                       IndexedChainVars)
         /\ \A recipient \in ValidatorIds,
               source \in IndexedAsync(initialContext)!
                          AsyncIngressSources:
              ENABLED
                <<IndexedAsync(initialContext)!
                    PostGstAdmitHistoricalRecoveryPacket(
                      recipient, source)>>_(
                  IndexedAsyncStateAt(initialContext))
                => ENABLED
                     <<IndexedAdmitHistoricalRecoveryPacketStep(
                         initialContext, recipient, source)>>_(
                       IndexedChainVars)
BY IndexedFairActionsRemainEnabledInProduct,
   IndexedFairProductStepsProjectExactOccurrences,
   ExpandENABLED, Isa
   DEF IndexedActivationStable

THEOREM IndexedSetGstFairnessTransfers ==
  \A initialContext \in AdmissibleContextRecords:
    (/\ IndexedChainSpec
     /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
      => WF_(IndexedAsyncStateAt(initialContext))(
           IndexedAsync(initialContext)!AsyncSetGST)
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords
         PROVE
           (/\ IndexedChainSpec
            /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
             => WF_(IndexedAsyncStateAt(initialContext))(
                  IndexedAsync(initialContext)!AsyncSetGST)
    <2>1. (/\ IndexedChainSpec
            /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
             => <>[]IndexedActivationStable(initialContext)
      BY <1>1, IndexedActivationEventuallyStabilizes
    <2>2. IndexedActivationStable(initialContext)
             => (ENABLED
                   <<IndexedAsync(initialContext)!AsyncSetGST>>_(
                     IndexedAsyncStateAt(initialContext))
                   => ENABLED
                        <<IndexedSetGstStep(initialContext)>>_(
                          IndexedChainVars))
      BY <1>1, IndexedFairExactOccurrencesEnableProductOccurrences
    <2>3. IndexedSetGstStep(initialContext)
             => <<IndexedAsync(initialContext)!AsyncSetGST>>_(
                  IndexedAsyncStateAt(initialContext))
      BY <1>1, IndexedFairProductStepsProjectExactOccurrences
    <2>4. IndexedChainSpec
             => WF_IndexedChainVars(IndexedSetGstStep(initialContext))
      BY <1>1 DEF IndexedChainSpec, IndexedFairness
    <2> QED BY <2>1, <2>2, <2>3, <2>4, PTL
  <1> QED BY <1>1

THEOREM IndexedTickFairnessTransfers ==
  \A initialContext \in AdmissibleContextRecords:
    (/\ IndexedChainSpec
     /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
      => WF_(IndexedAsyncStateAt(initialContext))(
           IndexedAsync(initialContext)!AsyncTick)
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords
         PROVE
           (/\ IndexedChainSpec
            /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
             => WF_(IndexedAsyncStateAt(initialContext))(
                  IndexedAsync(initialContext)!AsyncTick)
    <2>1. (/\ IndexedChainSpec
            /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
             => <>[]IndexedActivationStable(initialContext)
      BY <1>1, IndexedActivationEventuallyStabilizes
    <2>2. IndexedActivationStable(initialContext)
             => (ENABLED
                   <<IndexedAsync(initialContext)!AsyncTick>>_(
                     IndexedAsyncStateAt(initialContext))
                   => ENABLED
                        <<IndexedTickStep(initialContext)>>_(
                          IndexedChainVars))
      BY <1>1, IndexedFairExactOccurrencesEnableProductOccurrences
    <2>3. IndexedTickStep(initialContext)
             => <<IndexedAsync(initialContext)!AsyncTick>>_(
                  IndexedAsyncStateAt(initialContext))
      BY <1>1, IndexedFairProductStepsProjectExactOccurrences
    <2>4. IndexedChainSpec
             => WF_IndexedChainVars(IndexedTickStep(initialContext))
      BY <1>1 DEF IndexedChainSpec, IndexedFairness
    <2> QED BY <2>1, <2>2, <2>3, <2>4, PTL
  <1> QED BY <1>1

THEOREM IndexedNodeFairnessTransfers ==
  \A initialContext \in AdmissibleContextRecords:
    \A node \in IndexedAsync(initialContext)!AsyncVotersAt(initialContext):
      (/\ IndexedChainSpec
       /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
        => /\ WF_(IndexedAsyncStateAt(initialContext))(
                  IndexedAsync(initialContext)!PostGstRunNode(node))
           /\ WF_(IndexedAsyncStateAt(initialContext))(
                  IndexedAsync(initialContext)!
                    PostGstCommitCertificateDiscovery(node))
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
              NEW node \in IndexedAsync(initialContext)!
                            AsyncVotersAt(initialContext)
         PROVE
           (/\ IndexedChainSpec
            /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
             => /\ WF_(IndexedAsyncStateAt(initialContext))(
                       IndexedAsync(initialContext)!PostGstRunNode(node))
                /\ WF_(IndexedAsyncStateAt(initialContext))(
                       IndexedAsync(initialContext)!
                         PostGstCommitCertificateDiscovery(node))
    <2>1. (/\ IndexedChainSpec
            /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
             => <>[]IndexedActivationStable(initialContext)
      BY <1>1, IndexedActivationEventuallyStabilizes
    <2>2. IndexedActivationStable(initialContext)
             => /\ (ENABLED
                       <<IndexedAsync(initialContext)!
                           PostGstRunNode(node)>>_(
                         IndexedAsyncStateAt(initialContext))
                       => ENABLED
                            <<IndexedRunNodeStep(
                                initialContext, node)>>_(IndexedChainVars))
                /\ (ENABLED
                       <<IndexedAsync(initialContext)!
                           PostGstCommitCertificateDiscovery(node)>>_(
                         IndexedAsyncStateAt(initialContext))
                       => ENABLED
                            <<IndexedCommitCertificateDiscoveryStep(
                                initialContext, node)>>_(IndexedChainVars))
      BY <1>1, IndexedFairExactOccurrencesEnableProductOccurrences
    <2>3. /\ (IndexedRunNodeStep(initialContext, node)
                   => <<IndexedAsync(initialContext)!
                           PostGstRunNode(node)>>_(
                        IndexedAsyncStateAt(initialContext)))
              /\ (IndexedCommitCertificateDiscoveryStep(
                       initialContext, node)
                   => <<IndexedAsync(initialContext)!
                           PostGstCommitCertificateDiscovery(node)>>_(
                        IndexedAsyncStateAt(initialContext)))
      BY <1>1, IndexedFairProductStepsProjectExactOccurrences
    <2>4. IndexedChainSpec
             => /\ WF_IndexedChainVars(
                       IndexedRunNodeStep(initialContext, node))
                /\ WF_IndexedChainVars(
                       IndexedCommitCertificateDiscoveryStep(
                         initialContext, node))
      BY <1>1 DEF IndexedChainSpec, IndexedFairness
    <2> QED BY <2>1, <2>2, <2>3, <2>4, PTL
  <1> QED BY <1>1

THEOREM IndexedResponsiveServiceFairnessTransfers ==
  \A initialContext \in AdmissibleContextRecords:
    \A node \in Responsive:
      (/\ IndexedChainSpec
       /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
        => /\ WF_(IndexedAsyncStateAt(initialContext))(
                  IndexedAsync(initialContext)!
                    PostGstRunHistoricalServer(node))
           /\ WF_(IndexedAsyncStateAt(initialContext))(
                  IndexedAsync(initialContext)!
                    PostGstServiceIoWorker(node))
BY IndexedActivationEventuallyStabilizes,
   IndexedFairExactOccurrencesEnableProductOccurrences,
   IndexedFairProductStepsProjectExactOccurrences, PTL
   DEF IndexedChainSpec, IndexedFairness

THEOREM IndexedHistoricalRecoveryFairnessTransfers ==
  \A initialContext \in AdmissibleContextRecords:
    \A node \in Responsive:
      (/\ IndexedChainSpec
       /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
        => /\ WF_(IndexedAsyncStateAt(initialContext))(
                  IndexedAsync(initialContext)!
                    PostGstOpenHistoricalRecovery(node))
           /\ WF_(IndexedAsyncStateAt(initialContext))(
                  IndexedAsync(initialContext)!
                    PostGstRunHistoricalRecoveryNode(node))
           /\ WF_(IndexedAsyncStateAt(initialContext))(
                  IndexedAsync(initialContext)!
                    PostGstHistoricalCommitCertificateDiscovery(node))
           /\ WF_(IndexedAsyncStateAt(initialContext))(
                  IndexedAsync(initialContext)!
                    PostGstServiceHistoricalRecoveryIoWorker(node))
BY IndexedActivationEventuallyStabilizes,
   IndexedFairExactOccurrencesEnableProductOccurrences,
   IndexedFairProductStepsProjectExactOccurrences, PTL
   DEF IndexedChainSpec, IndexedFairness

THEOREM IndexedPacketFairnessTransfers ==
  \A initialContext \in AdmissibleContextRecords:
    \A recipient \in Responsive,
       source \in IndexedAsync(initialContext)!
                  AsyncIngressSources:
      (/\ IndexedChainSpec
       /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
        => WF_(IndexedAsyncStateAt(initialContext))(
             IndexedAsync(initialContext)!
               PostGstAdmitHiddenPacket(recipient, source))
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
              NEW recipient \in Responsive,
              NEW source \in IndexedAsync(initialContext)!
                               AsyncIngressSources
         PROVE
           (/\ IndexedChainSpec
            /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
             => WF_(IndexedAsyncStateAt(initialContext))(
                  IndexedAsync(initialContext)!
                    PostGstAdmitHiddenPacket(recipient, source))
    <2>1. (/\ IndexedChainSpec
            /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
             => <>[]IndexedActivationStable(initialContext)
      BY <1>1, IndexedActivationEventuallyStabilizes
    <2>2. IndexedActivationStable(initialContext)
             => (ENABLED
                   <<IndexedAsync(initialContext)!
                       PostGstAdmitHiddenPacket(recipient, source)>>_(
                     IndexedAsyncStateAt(initialContext))
                   => ENABLED
                        <<IndexedAdmitPacketStep(
                            initialContext, recipient, source)>>_(
                          IndexedChainVars))
      BY <1>1, IndexedFairExactOccurrencesEnableProductOccurrences
    <2>3. IndexedAdmitPacketStep(initialContext, recipient, source)
             => <<IndexedAsync(initialContext)!
                     PostGstAdmitHiddenPacket(recipient, source)>>_(
                  IndexedAsyncStateAt(initialContext))
      BY <1>1, IndexedFairProductStepsProjectExactOccurrences
    <2>4. IndexedChainSpec
             => WF_IndexedChainVars(
                  IndexedAdmitPacketStep(initialContext, recipient, source))
      BY <1>1 DEF IndexedChainSpec, IndexedFairness
    <2> QED BY <2>1, <2>2, <2>3, <2>4, PTL
  <1> QED BY <1>1

THEOREM IndexedHistoricalRecoveryPacketFairnessTransfers ==
  \A initialContext \in AdmissibleContextRecords:
    \A recipient \in ValidatorIds,
       source \in IndexedAsync(initialContext)!AsyncIngressSources:
      (/\ IndexedChainSpec
       /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
        => WF_(IndexedAsyncStateAt(initialContext))(
             IndexedAsync(initialContext)!
               PostGstAdmitHistoricalRecoveryPacket(recipient, source))
BY IndexedActivationEventuallyStabilizes,
   IndexedFairExactOccurrencesEnableProductOccurrences,
   IndexedFairProductStepsProjectExactOccurrences, PTL
   DEF IndexedChainSpec, IndexedFairness

THEOREM IndexedInstanceActivationObligation ==
  \A initialContext \in AdmissibleContextRecords:
    (/\ IndexedChainSpec
     /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
      => IndexedAsync(initialContext)!AsyncSpecAt(initialContext)
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords
         PROVE
           (/\ IndexedChainSpec
            /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
             => IndexedAsync(initialContext)!AsyncSpecAt(initialContext)
    <2>1. IndexedChainInit
             => IndexedAsync(initialContext)!AsyncInitAt(initialContext)
      BY <1>1, IndexedInitProjectsEveryAsyncInit
    <2>2. IndexedChainSpec
             => [][IndexedAsync(initialContext)!AsyncNext]_(
                  IndexedAsyncStateAt(initialContext))
      BY <1>1, IndexedStepProjectsEveryAsyncStep, PTL
         DEF IndexedChainSpec, IndexedChainVars
    <2>3. IndexedChainSpec
             => [](IndexedAsync(initialContext)!AsyncAllVars
                    = IndexedAsyncStateAt(initialContext))
      BY <1>1, IndexedChainSpecEstablishesCompositionInvariant,
         IndexedInstanceVariablesAreExact, PTL
         DEF IndexedCompositionInvariant
    <2>4. IndexedChainSpec
             => /\ WF_(IndexedAsyncStateAt(initialContext))(
                       IndexedAsync(initialContext)!
                         PreGstResponsiveRestart)
                /\ WF_(IndexedAsyncStateAt(initialContext))(
                       IndexedAsync(initialContext)!
                         PreGstResponsiveReplay)
                /\ WF_(IndexedAsyncStateAt(initialContext))(
                       IndexedAsync(initialContext)!
                         ResponsiveReplayRunNode)
                /\ WF_(IndexedAsyncStateAt(initialContext))(
                       IndexedAsync(initialContext)!
                         ResponsiveReplayServiceIoWorker)
                /\ WF_(IndexedAsyncStateAt(initialContext))(
                       IndexedAsync(initialContext)!
                         DriveResponsiveReplayHead)
                /\ WF_(IndexedAsyncStateAt(initialContext))(
                       IndexedAsync(initialContext)!
                         FinishResponsiveReplay)
      BY <1>1, IndexedResponsiveRecoveryFairnessIsVacuous
    <2>4a. (/\ IndexedChainSpec
             /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
              => \A node \in Responsive:
                   WF_(IndexedAsyncStateAt(initialContext))(
                     IndexedAsync(initialContext)!
                       AsyncActivateServiceNode(node))
      BY <1>1,
         IndexedResponsiveServiceActivationFairnessIsVacuous
    <2>5. (/\ IndexedChainSpec
            /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
             => WF_(IndexedAsyncStateAt(initialContext))(
                  IndexedAsync(initialContext)!AsyncSetGST)
      BY <1>1, IndexedSetGstFairnessTransfers
    <2>6. (/\ IndexedChainSpec
            /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
             => WF_(IndexedAsyncStateAt(initialContext))(
                  IndexedAsync(initialContext)!AsyncTick)
      BY <1>1, IndexedTickFairnessTransfers
    <2>7. (/\ IndexedChainSpec
            /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
             => \A node \in IndexedAsync(initialContext)!
                               AsyncVotersAt(initialContext):
                  /\ WF_(IndexedAsyncStateAt(initialContext))(
                       IndexedAsync(initialContext)!PostGstRunNode(node))
                  /\ WF_(IndexedAsyncStateAt(initialContext))(
                       IndexedAsync(initialContext)!
                         PostGstCommitCertificateDiscovery(node))
      BY <1>1, IndexedNodeFairnessTransfers
    <2>7a. IndexedChainSpec
              => /\ \A node \in IndexedAsync(initialContext)!
                                 AsyncVotersAt(initialContext):
                       /\ WF_(IndexedAsyncStateAt(initialContext))(
                            IndexedAsync(initialContext)!
                              PostGstResolveLocalCandidateProducerContinuation(
                                node))
                       /\ WF_(IndexedAsyncStateAt(initialContext))(
                            IndexedAsync(initialContext)!
                              PostGstServiceConditionalTransportProducerContinuation(
                                node))
                       /\ WF_(IndexedAsyncStateAt(initialContext))(
                            IndexedAsync(initialContext)!
                              PostGstServiceVolatileBodyProducerContinuation(
                                node))
                  /\ \A slot \in IndexedAsync(initialContext)!
                                AsyncLeaderWireLifecycleSlotSet:
                       WF_(IndexedAsyncStateAt(initialContext))(
                         IndexedAsync(initialContext)!
                           PostGstRetireLeaderWireLifecycleSlot(slot))
      BY <1>1, IndexedAdequateLeaderNonRunnerFairnessTransfersLocally
    <2>8. (/\ IndexedChainSpec
            /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
             => \A node \in Responsive:
                  /\ WF_(IndexedAsyncStateAt(initialContext))(
                       IndexedAsync(initialContext)!
                         PostGstRunHistoricalServer(node))
                  /\ WF_(IndexedAsyncStateAt(initialContext))(
                       IndexedAsync(initialContext)!
                         PostGstServiceIoWorker(node))
      BY <1>1, IndexedResponsiveServiceFairnessTransfers
    <2>9. (/\ IndexedChainSpec
            /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
             => \A node \in Responsive:
                  /\ WF_(IndexedAsyncStateAt(initialContext))(
                       IndexedAsync(initialContext)!
                         PostGstOpenHistoricalRecovery(node))
                  /\ WF_(IndexedAsyncStateAt(initialContext))(
                       IndexedAsync(initialContext)!
                         PostGstRunHistoricalRecoveryNode(node))
                  /\ WF_(IndexedAsyncStateAt(initialContext))(
                       IndexedAsync(initialContext)!
                         PostGstHistoricalCommitCertificateDiscovery(node))
                  /\ WF_(IndexedAsyncStateAt(initialContext))(
                       IndexedAsync(initialContext)!
                         PostGstServiceHistoricalRecoveryIoWorker(node))
      BY <1>1, IndexedHistoricalRecoveryFairnessTransfers
    <2>10. (/\ IndexedChainSpec
             /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
              => \A recipient \in Responsive,
                    source \in IndexedAsync(initialContext)!
                              AsyncIngressSources:
                   WF_(IndexedAsyncStateAt(initialContext))(
                     IndexedAsync(initialContext)!
                       PostGstAdmitHiddenPacket(recipient, source))
      BY <1>1, IndexedPacketFairnessTransfers
    <2>11. (/\ IndexedChainSpec
             /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
              => \A recipient \in ValidatorIds,
                    source \in IndexedAsync(initialContext)!
                              AsyncIngressSources:
                   WF_(IndexedAsyncStateAt(initialContext))(
                     IndexedAsync(initialContext)!
                       PostGstAdmitHistoricalRecoveryPacket(
                         recipient, source))
      BY <1>1, IndexedHistoricalRecoveryPacketFairnessTransfers
    <2> QED BY <2>1, <2>2, <2>3, <2>4, <2>4a, <2>5, <2>6,
                 <2>7, <2>7a, <2>8, <2>9, <2>10, <2>11, PTL
         DEF IndexedAsync!AsyncSpecAt, IndexedAsync!AsyncFairnessAt
  <1> QED BY <1>1

THEOREM IndexedLiveInstanceActivationObligation ==
  \A initialContext \in AdmissibleContextRecords:
    (/\ IndexedLiveChainSpec
     /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
      => IndexedAsync(initialContext)!AsyncLiveSpecAt(initialContext)
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords
         PROVE
           (/\ IndexedLiveChainSpec
            /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
             => IndexedAsync(initialContext)!
                  AsyncLiveSpecAt(initialContext)
    <2>1. (/\ IndexedLiveChainSpec
            /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
             => (/\ IndexedChainSpec
                 /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
      BY DEF IndexedLiveChainSpec
    <2>2. (/\ IndexedLiveChainSpec
            /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
             => IndexedAsync(initialContext)!AsyncSpecAt(initialContext)
      BY <1>1, <2>1, IndexedInstanceActivationObligation
    <2>3. (/\ IndexedLiveChainSpec
            /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
             => AsyncRepresentativeLiveConfiguration
      BY DEF IndexedLiveChainSpec
    <2> QED BY <2>2, <2>3
         DEF IndexedAsync!AsyncLiveSpecAt
  <1> QED BY <1>1

(***************************************************************************
Exact historical-recovery progress boundary.

Opening and every subsequent recovery transition belong to the exact Async
instance. This product therefore carries no second stage rank. The remaining
temporal debt is stated directly over exact target ownership: once a responsive
node at its frozen context eventually becomes recovery-eligible and acquires
that context's exact application evidence. Eligibility is intentionally
separate: the node must have an authenticated source ready to open, already own
the exact target, or already have the exact durable Decision. A merely joined
node with no source is still waiting for current-height consensus and is not
silently reclassified as an enabled historical-recovery action. Nonterminal
receipt handoff then advances nodeHeight; the terminal horizon intentionally
records application without inventing a successor. The child chain-liveness
module names the eligibility leadsto separately, proves fair target opening,
and exposes the two exact Async temporal prerequisites: target-to-Decision and
responsive Decision-to-application.
***************************************************************************)
HistoricalRecoveryOutstanding(initialContext, node) ==
  /\ node \in Responsive
  /\ node \in joinedByContext[initialContext]
  /\ ExactNodeLocationAt(initialContext, node)
  /\ ~IndexedAsync(initialContext)!NodeHasApplication(node)

HistoricalRecoveryProgressEligible(initialContext, node) ==
  /\ HistoricalRecoveryOutstanding(initialContext, node)
  /\ \/ IndexedHistoricalRecoveryReady(initialContext, node)
     \/ IndexedAsync(initialContext)!HistoricalRecoveryTarget(node)
     \/ IndexedAsync(initialContext)!NodeHasDecision(node)

HistoricalRecoveryComplete(initialContext, node) ==
  IF initialContext.height = MaxHeight
  THEN IndexedAsync(initialContext)!NodeHasApplication(node)
  ELSE nodeHeight[node] > initialContext.height

IndexedExactHistoricalRecoveryProgress ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    HistoricalRecoveryOutstanding(initialContext, node)
      ~> HistoricalRecoveryComplete(initialContext, node)

VerificationOneHeightCompletion ==
  IndexedAsync(VerificationContext)!AsyncLiveSpecAt(VerificationContext)
    => (IndexedCore(VerificationContext, 7)
          ~> IndexedAsync(VerificationContext)!
               AsyncAllResponsiveAppliedAt(VerificationContext))

THEOREM VerificationOneHeightCompletionObligation ==
  VerificationOneHeightCompletion
PROOF
  <1>1. VerificationAsyncProof!AsyncTemporalClosureOneHeightCompletionObligation
    BY VerificationAsyncProof!AsyncTemporalClosureOneHeightCompletionObligation
  <1> QED BY <1>1
       DEF VerificationOneHeightCompletion,
           VerificationAsyncProof!OneHeightCompletionLiveness,
           VerificationAsyncProof!AsyncLiveSpecAt,
           VerificationAsyncProof!AsyncAllResponsiveAppliedAt,
           IndexedAsync!AsyncLiveSpecAt,
           IndexedAsync!AsyncAllResponsiveAppliedAt,
           IndexedAsyncStateAt, IndexedCore, IndexedRecovery,
           VerificationCore, VerificationScheduler, VerificationRecovery

THEOREM IndexedAllResponsiveAppliedIsStable ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedAsync(initialContext)!
      AsyncAllResponsiveAppliedAt(initialContext)
      /\ [IndexedChainNext]_IndexedChainVars
      => (IndexedAsync(initialContext)!
            AsyncAllResponsiveAppliedAt(initialContext))'
BY Isa DEF IndexedChainNext, IndexedChainVars,
           IndexedProductActionAt, IndexedReceiptClassification,
           IndexedReceiptFreeChainStutter,
           IndexedDecisionReceiptHandoff,
           IndexedApplicationReceiptHandoff,
           NewIndexedDecisionReceipt, NewIndexedApplicationReceipt,
           NoNewIndexedDurableReceipt, IndexedApplications,
           IndexedAsync!AsyncAllResponsiveAppliedAt,
           IndexedAsync!AsyncVotersAt,
           IndexedAsync!NodeHasApplication

THEOREM VerificationFrontierActivatedInstanceEventuallyApplies ==
  /\ IndexedLiveChainSpec
    /\ VerificationOneHeightCompletion
    /\ VerificationContext \in AdmissibleContextRecords
    /\ (IndexedAsync(VerificationContext)!
          AsyncLiveSpecAt(VerificationContext)
          => <>IndexedCore(VerificationContext, 7))
    /\ []~JoinedCanonicalDescendant(VerificationContext)
    => IndexedAllResponsiveJoined(VerificationContext)
         ~> IndexedAsync(VerificationContext)!
               AsyncAllResponsiveAppliedAt(VerificationContext)
PROOF
  <1>1. ASSUME IndexedLiveChainSpec,
              VerificationOneHeightCompletion,
              VerificationContext \in AdmissibleContextRecords,
              (IndexedAsync(VerificationContext)!
                 AsyncLiveSpecAt(VerificationContext)
                 => <>IndexedCore(VerificationContext, 7)),
              []~JoinedCanonicalDescendant(VerificationContext)
         PROVE IndexedAllResponsiveJoined(VerificationContext)
                 ~> IndexedAsync(VerificationContext)!
                       AsyncAllResponsiveAppliedAt(VerificationContext)
    <2>0. IndexedChainSpec
      BY <1>1, IndexedLiveChainSpecProjectsIndexedChainSpec
    <2>1. IndexedAllResponsiveJoined(VerificationContext)
             /\ [IndexedChainNext]_IndexedChainVars
             => IndexedAllResponsiveJoined(VerificationContext)'
      BY <1>1, IndexedAllResponsiveJoinedIsStable
    <2>2. <>IndexedAllResponsiveJoined(VerificationContext)
             => (TRUE ~> IndexedAllResponsiveJoined(VerificationContext))
      BY <2>0, <2>1, PTL DEF IndexedChainSpec
    <2>3. (/\ IndexedLiveChainSpec
            /\ TRUE ~> IndexedAllResponsiveJoined(VerificationContext)
            /\ []~JoinedCanonicalDescendant(VerificationContext))
             => IndexedAsync(VerificationContext)!
                  AsyncLiveSpecAt(VerificationContext)
      BY <1>1, IndexedLiveInstanceActivationObligation
    <2>4. IndexedAsync(VerificationContext)!
             AsyncLiveSpecAt(VerificationContext)
             => <>IndexedCore(VerificationContext, 7)
      BY <1>1
    <2>5. VerificationOneHeightCompletion
      BY <1>1
    <2> QED BY <2>1, <2>2, <2>3, <2>4, <2>5, PTL
  <1> QED BY <1>1

VerificationFrontierEscape ==
  \/ JoinedCanonicalDescendant(VerificationContext)
  \/ IndexedAsync(VerificationContext)!
       AsyncAllResponsiveAppliedAt(VerificationContext)

THEOREM VerificationFrontierEscapeIsStable ==
  IndexedCompositionInvariant
    /\ VerificationContext \in AdmissibleContextRecords
    /\ VerificationFrontierEscape
    /\ [IndexedChainNext]_IndexedChainVars
    => VerificationFrontierEscape'
BY Isa, JoinedCanonicalDescendantIsStable,
   IndexedAllResponsiveAppliedIsStable
   DEF VerificationFrontierEscape

THEOREM VerificationActivatedFrontierEventuallyEscapes ==
  /\ IndexedLiveChainSpec
    /\ VerificationOneHeightCompletion
    /\ VerificationContext \in AdmissibleContextRecords
    /\ (IndexedAsync(VerificationContext)!
          AsyncLiveSpecAt(VerificationContext)
          => <>IndexedCore(VerificationContext, 7))
    => IndexedAllResponsiveJoined(VerificationContext)
         ~> VerificationFrontierEscape
PROOF
  <1>1. ASSUME IndexedLiveChainSpec,
              VerificationOneHeightCompletion,
              VerificationContext \in AdmissibleContextRecords,
              (IndexedAsync(VerificationContext)!
                 AsyncLiveSpecAt(VerificationContext)
                 => <>IndexedCore(VerificationContext, 7))
         PROVE IndexedAllResponsiveJoined(VerificationContext)
                 ~> VerificationFrontierEscape
    <2>0. IndexedChainSpec
      BY <1>1, IndexedLiveChainSpecProjectsIndexedChainSpec
    <2>1. []IndexedCompositionInvariant
      BY <2>0, IndexedChainSpecEstablishesCompositionInvariant, PTL
    <2>2. IndexedCompositionInvariant
             /\ VerificationFrontierEscape
             /\ [IndexedChainNext]_IndexedChainVars
             => VerificationFrontierEscape'
      BY <1>1, VerificationFrontierEscapeIsStable
    <2>3. <>JoinedCanonicalDescendant(VerificationContext)
             => (IndexedAllResponsiveJoined(VerificationContext)
                   ~> VerificationFrontierEscape)
      BY <2>0, <2>1, <2>2, PTL
         DEF VerificationFrontierEscape, IndexedChainSpec
    <2>4. []~JoinedCanonicalDescendant(VerificationContext)
             => (IndexedAllResponsiveJoined(VerificationContext)
                   ~> VerificationFrontierEscape)
      BY <1>1,
         VerificationFrontierActivatedInstanceEventuallyApplies,
         PTL DEF VerificationFrontierEscape
    <2> QED BY <2>3, <2>4, PTL
  <1> QED BY <1>1

THEOREM IndexedDecisionEvidenceMemberClassification ==
  \A decision:
    decision \in IndexedDecisionEvidence
      => \E sourceContext \in AdmissibleContextRecords:
           decision \in IndexedCurrentDecisions(sourceContext)
BY Isa DEF IndexedDecisionEvidence

THEOREM IndexedCurrentCanonicalDecisionIdentifiesContext ==
  \A initialContext \in AdmissibleContextRecords,
     sourceContext \in AdmissibleContextRecords,
     decision \in Chain!DecisionEvidenceSet:
    (/\ JoinedContextCertificationInvariant
     /\ initialContext \in JoinedContexts
     /\ decision \in IndexedCurrentDecisions(sourceContext)
     /\ Chain!CanonicalCommitForSlot(
          decision.qc, initialContext.height + 1))
      => sourceContext = initialContext
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
              NEW sourceContext \in AdmissibleContextRecords,
              NEW decision \in Chain!DecisionEvidenceSet,
              JoinedContextCertificationInvariant,
              initialContext \in JoinedContexts,
              decision \in IndexedCurrentDecisions(sourceContext),
              Chain!CanonicalCommitForSlot(
                decision.qc, initialContext.height + 1)
         PROVE sourceContext = initialContext
    <2>1. initialContext \in ContextRecords
      BY <1>1 DEF AdmissibleContextRecords
    <2>2. initialContext.height \in Heights
      BY <2>1, ContextRecordHeightTyped
    <2>3. (initialContext.height + 1) - 1 = initialContext.height
      BY <2>2, Isa DEF Heights
    <2>4. initialContext =
             Chain!ContextRecord(
               initialContext.height,
               Chain!HistoryThrough(initialContext.height))
      BY <1>1 DEF JoinedContextCertificationInvariant
    <2>5. decision.qc.context =
             Chain!ContextRecord(
               (initialContext.height + 1) - 1,
               Chain!HistoryThrough((initialContext.height + 1) - 1))
      BY <1>1 DEF Chain!CanonicalCommitForSlot
    <2>6. Chain!ContextRecord(
             (initialContext.height + 1) - 1,
             Chain!HistoryThrough((initialContext.height + 1) - 1))
           = Chain!ContextRecord(
               initialContext.height,
               Chain!HistoryThrough(initialContext.height))
      BY <2>3
    <2>7. decision.qc.context = initialContext
      BY <2>4, <2>5, <2>6
    <2> QED BY <1>1, <2>7, Isa DEF IndexedCurrentDecisions
  <1> QED BY <1>1

THEOREM JoinedCanonicalDescendantStaysWithinHorizon ==
  \A initialContext \in AdmissibleContextRecords:
    JoinedCanonicalDescendant(initialContext)
      => initialContext.height < MaxHeight
BY Isa DEF JoinedCanonicalDescendant, JoinedContexts,
           AdmissibleContextRecords, FrozenContextAdmissible,
           ContextRecords, Heights

THEOREM JoinedCanonicalDescendantBoundsImmediateSuccessor ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedCompositionInvariant
      /\ JoinedCanonicalDescendant(initialContext)
      => /\ initialContext.height < MaxHeight
         /\ initialContext.height < certifiedHeight
         /\ initialContext.height + 1 \in 1..certifiedHeight
BY Isa DEF JoinedCanonicalDescendant,
           IndexedCompositionInvariant,
           JoinedContextCertificationInvariant,
           JoinedContexts, CanonicalIndexedContext,
           Chain!ChainEpochInvariant, Chain!ChainEpochTypeInvariant,
           Chain!ContextRecord, Chain!HistoryThrough,
           AdmissibleContextRecords, FrozenContextAdmissible,
           ContextRecords, Heights

IndexedResponsiveLagAt(initialContext, node) ==
  /\ initialContext.height < MaxHeight
  /\ node \in Responsive
  /\ nodeHeight[node] = initialContext.height

THEOREM IndexedHistoricalRecoveryAdvancesResponsiveNode ==
  IndexedExactHistoricalRecoveryProgress
  =>
    \A initialContext \in AdmissibleContextRecords,
       node \in Responsive:
      initialContext.height < MaxHeight
        => HistoricalRecoveryOutstanding(initialContext, node)
             ~> nodeHeight[node] > initialContext.height
BY PTL DEF IndexedExactHistoricalRecoveryProgress,
           HistoricalRecoveryComplete

IndexedResponsiveHeightReached(blockHeight) ==
  \A node \in Responsive: nodeHeight[node] >= blockHeight

IndexedNodePastContext(initialContext, node) ==
  nodeHeight[node] > initialContext.height

IndexedContextAdvanceReady(initialContext) ==
  /\ initialContext \in AdmissibleContextRecords
  /\ initialContext \in JoinedContexts
  /\ JoinedCanonicalDescendant(initialContext)
  /\ IndexedResponsiveHeightReached(initialContext.height)

IndexedResponsivePrefixPast(initialContext, limit) ==
  \A node \in Responsive \cap (0..limit):
    IndexedNodePastContext(initialContext, node)

THEOREM IndexedNodePastContextIsStable ==
  \A initialContext \in AdmissibleContextRecords,
     node \in ValidatorIds:
    IndexedNodePastContext(initialContext, node)
      /\ [IndexedChainNext]_IndexedChainVars
      => IndexedNodePastContext(initialContext, node)'
BY IndexedBracketStepKeepsNodeHeightsMonotone, SMT
   DEF IndexedNodePastContext, Heights, AdmissibleContextRecords,
       FrozenContextAdmissible, ContextRecords

THEOREM IndexedResponsivePrefixPastIsStable ==
  \A initialContext \in AdmissibleContextRecords,
     limit \in Nat:
    IndexedResponsivePrefixPast(initialContext, limit)
      /\ [IndexedChainNext]_IndexedChainVars
      => IndexedResponsivePrefixPast(initialContext, limit)'
BY Isa, IndexedNodePastContextIsStable
   DEF IndexedResponsivePrefixPast

IndexedAncestorContext(targetContext, blockHeight) ==
  ContextRecord(
    blockHeight,
    [index \in 1..blockHeight |-> targetContext.lineage[index]])

IndexedTargetJoined(targetContext) ==
  targetContext \in JoinedContexts

THEOREM IndexedAdmissibleTargetHasAdmissibleAncestors ==
  \A targetContext \in AdmissibleContextRecords:
    \A blockHeight \in 0..targetContext.height:
      IndexedAncestorContext(targetContext, blockHeight)
        \in AdmissibleContextRecords
BY Isa DEF IndexedAncestorContext, AdmissibleContextRecords,
           FrozenContextAdmissible, ContextRecords, LineagesAt,
           Heights, ContextRecord

THEOREM IndexedTargetJoinedIsStable ==
  \A targetContext \in AdmissibleContextRecords:
    IndexedTargetJoined(targetContext)
      /\ [IndexedChainNext]_IndexedChainVars
      => IndexedTargetJoined(targetContext)'
BY Isa, JoinedMembershipIsMonotone
   DEF IndexedTargetJoined, JoinedContexts, IndexedChainVars

THEOREM IndexedResponsiveHeightReachedIsStable ==
  \A blockHeight \in Heights:
    IndexedResponsiveHeightReached(blockHeight)
      /\ [IndexedChainNext]_IndexedChainVars
      => IndexedResponsiveHeightReached(blockHeight)'
BY Isa, IndexedBracketStepKeepsNodeHeightsMonotone
   DEF IndexedResponsiveHeightReached, ModelConfiguration,
       ValidatorIds

THEOREM IndexedJoinedTargetIdentifiesEveryCanonicalAncestor ==
  \A targetContext \in AdmissibleContextRecords:
    \A blockHeight \in 0..targetContext.height:
      IndexedCompositionInvariant
        /\ IndexedTargetJoined(targetContext)
        => /\ IndexedAncestorContext(targetContext, blockHeight)
                 \in AdmissibleContextRecords
           /\ IndexedAncestorContext(targetContext, blockHeight)
                 = CanonicalIndexedContext(blockHeight)
BY Isa DEF IndexedCompositionInvariant,
           JoinedContextCertificationInvariant,
           IndexedTargetJoined, JoinedContexts,
           IndexedAncestorContext, CanonicalIndexedContext,
           AdmissibleContextRecords, FrozenContextAdmissible,
           ContextRecords, LineagesAt, Heights,
           Chain!HistoryThrough, Chain!ContextRecord

IndexedActivationPendingIntoContext(initialContext, node) ==
  IF initialContext.height = 0
  THEN FALSE
  ELSE /\ initialContext =
            CanonicalIndexedContext(initialContext.height)
       /\ IndexedSuccessorActivationPending(
            CanonicalIndexedContext(initialContext.height - 1), node)

THEOREM IndexedReachedAncestorClassifiesEveryResponsiveNode ==
  \A targetContext \in AdmissibleContextRecords:
    \A blockHeight \in 0..targetContext.height:
      IndexedCompositionInvariant
        /\ IndexedJoinedThroughLocalHeight
        /\ IndexedTargetJoined(targetContext)
        /\ IndexedResponsiveHeightReached(blockHeight)
        => \A node \in Responsive:
             \/ node \in joinedByContext[
                  IndexedAncestorContext(targetContext, blockHeight)]
             \/ IndexedActivationPendingIntoContext(
                  IndexedAncestorContext(targetContext, blockHeight), node)
BY Isa, IndexedJoinedTargetIdentifiesEveryCanonicalAncestor
   DEF IndexedJoinedThroughLocalHeight,
       IndexedResponsiveHeightReached,
       IndexedActivationPendingIntoContext,
       IndexedSuccessorActivationPending,
       SuccessorPublicationOrSuperseded,
       SuccessorHeightActivated,
       ModelConfiguration, ValidatorIds, Heights

THEOREM IndexedActivationOutcomeJoinsExactContext ==
  \A initialContext \in AdmissibleContextRecords,
     node \in ValidatorIds:
    IndexedCompositionInvariant
      /\ IndexedJoinedThroughLocalHeight
      /\ initialContext.height > 0
      /\ initialContext =
           CanonicalIndexedContext(initialContext.height)
      /\ SuccessorPublicationOrSuperseded(
           CanonicalIndexedContext(initialContext.height - 1), node)
      => node \in joinedByContext[initialContext]
BY Isa DEF IndexedCompositionInvariant,
           IndexedJoinedThroughLocalHeight,
           SuccessorPublicationOrSuperseded,
           SuccessorHeightActivated,
           SuccessorActivationMarker,
           CanonicalIndexedContext,
           AdmissibleContextRecords, FrozenContextAdmissible,
           ContextRecords, Heights,
           Chain!ContextRecord, Chain!HistoryThrough

THEOREM IndexedNodeJoinIsStable ==
  \A initialContext \in AdmissibleContextRecords,
     node \in ValidatorIds:
    node \in joinedByContext[initialContext]
      /\ [IndexedChainNext]_IndexedChainVars
      => node \in joinedByContext[initialContext]'
BY Isa, JoinedMembershipIsMonotone DEF IndexedChainVars

THEOREM IndexedActivationPendingIntoContextEventuallyJoins ==
  /\ IndexedChainSpec
  /\ IndexedSuccessorActivationProgress
  => \A initialContext \in AdmissibleContextRecords,
       node \in Responsive:
       IndexedActivationPendingIntoContext(initialContext, node)
         ~> node \in joinedByContext[initialContext]
PROOF
  <1>1. ASSUME IndexedChainSpec,
              IndexedSuccessorActivationProgress,
              NEW initialContext \in AdmissibleContextRecords,
              NEW node \in Responsive
         PROVE IndexedActivationPendingIntoContext(initialContext, node)
                 ~> node \in joinedByContext[initialContext]
    <2>1. IndexedChainSpec => []IndexedCompositionInvariant
      BY IndexedChainSpecEstablishesCompositionInvariant
    <2>2. IndexedChainSpec => []IndexedJoinedThroughLocalHeight
      BY IndexedChainSpecJoinsEveryNodeThroughLocalHeight
    <2>3. IndexedActivationPendingIntoContext(initialContext, node)
             ~> SuccessorPublicationOrSuperseded(
                  CanonicalIndexedContext(initialContext.height - 1), node)
      BY <1>1, PTL DEF IndexedSuccessorActivationProgress,
                         IndexedActivationPendingIntoContext
    <2>4. node \in joinedByContext[initialContext]
             /\ [IndexedChainNext]_IndexedChainVars
             => node \in joinedByContext[initialContext]'
      BY <1>1, IndexedNodeJoinIsStable
    <2> QED BY <2>1, <2>2, <2>3, <2>4,
                 IndexedActivationOutcomeJoinsExactContext, PTL
         DEF IndexedActivationPendingIntoContext
  <1> QED BY <1>1

THEOREM IndexedActivationOutcomeLeavesPastOrRecoveryOutstanding ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    IndexedCompositionInvariant
      /\ IndexedJoinedThroughLocalHeight
      /\ initialContext.height > 0
      /\ initialContext.height < MaxHeight
      /\ initialContext =
           CanonicalIndexedContext(initialContext.height)
      /\ SuccessorPublicationOrSuperseded(
           CanonicalIndexedContext(initialContext.height - 1), node)
      => \/ IndexedNodePastContext(initialContext, node)
         \/ HistoricalRecoveryOutstanding(initialContext, node)
BY Isa, IndexedActivationOutcomeJoinsExactContext
   DEF IndexedCompositionInvariant,
       JoinedRoutingInvariant, IndexedApplicationsRespectNodeHeight,
       IndexedNodeCurrentAt, ExactNodeLocationAt,
       IndexedNodePastContext, HistoricalRecoveryOutstanding,
       IndexedAsync!NodeHasApplication,
       Chain!ChainEpochInvariant, Chain!ChainEpochTypeInvariant,
       Chain!ContextsMatchLocalHistories

THEOREM IndexedActivationPendingEventuallyLeavesPastOrRecoveryOutstanding ==
  /\ IndexedChainSpec
  /\ IndexedSuccessorActivationProgress
  => \A initialContext \in AdmissibleContextRecords,
       node \in Responsive:
       initialContext.height < MaxHeight
         => IndexedActivationPendingIntoContext(initialContext, node)
              ~> (IndexedNodePastContext(initialContext, node)
                   \/ HistoricalRecoveryOutstanding(initialContext, node))
PROOF
  <1>1. ASSUME IndexedChainSpec,
              IndexedSuccessorActivationProgress,
              NEW initialContext \in AdmissibleContextRecords,
              NEW node \in Responsive,
              initialContext.height < MaxHeight
         PROVE IndexedActivationPendingIntoContext(initialContext, node)
                 ~> (IndexedNodePastContext(initialContext, node)
                      \/ HistoricalRecoveryOutstanding(
                           initialContext, node))
    <2>1. IndexedChainSpec => []IndexedCompositionInvariant
      BY IndexedChainSpecEstablishesCompositionInvariant
    <2>2. IndexedChainSpec => []IndexedJoinedThroughLocalHeight
      BY IndexedChainSpecJoinsEveryNodeThroughLocalHeight
    <2>3. IndexedActivationPendingIntoContext(initialContext, node)
             ~> SuccessorPublicationOrSuperseded(
                  CanonicalIndexedContext(initialContext.height - 1), node)
      BY <1>1, PTL DEF IndexedSuccessorActivationProgress,
                         IndexedActivationPendingIntoContext
    <2>4. IndexedActivationPendingIntoContext(initialContext, node)
             => initialContext.height > 0
      BY DEF IndexedActivationPendingIntoContext
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4,
                 IndexedActivationOutcomeLeavesPastOrRecoveryOutstanding,
                 PTL DEF IndexedActivationPendingIntoContext
  <1> QED BY <1>1

IndexedTargetHeightStepPremise(targetContext, blockHeight) ==
  /\ IndexedTargetJoined(targetContext)
  /\ IndexedResponsiveHeightReached(blockHeight)

THEOREM IndexedTargetStepEitherPassedOrRecoveryOutstanding ==
  \A targetContext \in AdmissibleContextRecords:
    \A blockHeight \in 0..targetContext.height:
      \A node \in Responsive:
        IndexedCompositionInvariant
          /\ IndexedJoinedThroughLocalHeight
          /\ IndexedTargetHeightStepPremise(targetContext, blockHeight)
          /\ blockHeight < targetContext.height
          => \/ IndexedNodePastContext(
                  IndexedAncestorContext(targetContext, blockHeight), node)
             \/ IndexedActivationPendingIntoContext(
                  IndexedAncestorContext(targetContext, blockHeight), node)
             \/ HistoricalRecoveryOutstanding(
                  IndexedAncestorContext(targetContext, blockHeight), node)
BY Isa, IndexedJoinedTargetIdentifiesEveryCanonicalAncestor,
   IndexedReachedAncestorClassifiesEveryResponsiveNode
   DEF IndexedTargetHeightStepPremise,
       IndexedResponsiveHeightReached, IndexedNodePastContext,
       IndexedResponsiveLagAt,
       HistoricalRecoveryOutstanding,
       IndexedActivationPendingIntoContext,
       IndexedCompositionInvariant,
       IndexedApplicationsRespectNodeHeight,
       IndexedTotalReceiptProjection,
       IndexedDecisionReceiptProjection,
       IndexedApplicationReceiptProjection,
       IndexedDecisionEvidence, IndexedApplicationEvidence,
       IndexedCurrentDecisions, IndexedCurrentApplications,
       IndexedProjectedNodeHasApplication,
       HistoricalRecoveryNodeHasApplicationProjection,
       IndexedAncestorContext, CanonicalIndexedContext,
       Chain!ChainEpochInvariant, Chain!ChainEpochTypeInvariant,
       Chain!ContextsMatchLocalHistories,
       IndexedAsync!NodeHasApplication

THEOREM IndexedAdvanceReadyEitherPassedOrNeedsRecovery ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    IndexedCompositionInvariant
      /\ IndexedJoinedThroughLocalHeight
      /\ IndexedContextAdvanceReady(initialContext)
      => \/ IndexedNodePastContext(initialContext, node)
         \/ IndexedActivationPendingIntoContext(initialContext, node)
         \/ HistoricalRecoveryOutstanding(initialContext, node)
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
              NEW node \in Responsive,
              IndexedCompositionInvariant,
              IndexedJoinedThroughLocalHeight,
              IndexedContextAdvanceReady(initialContext)
         PROVE \/ IndexedNodePastContext(initialContext, node)
               \/ IndexedActivationPendingIntoContext(initialContext, node)
               \/ HistoricalRecoveryOutstanding(initialContext, node)
    <2>1. PICK descendantContext \in JoinedContexts:
             /\ descendantContext.height > initialContext.height
             /\ descendantContext =
                  Chain!ContextRecord(
                    descendantContext.height,
                    Chain!HistoryThrough(descendantContext.height))
      BY <1>1 DEF IndexedContextAdvanceReady,
                    JoinedCanonicalDescendant
    <2>2. descendantContext \in AdmissibleContextRecords
      BY <2>1 DEF JoinedContexts
    <2>3. initialContext.height \in 0..descendantContext.height
      BY <1>1, <2>1, Isa
         DEF AdmissibleContextRecords, FrozenContextAdmissible,
             ContextRecords, Heights
    <2>4. IndexedAncestorContext(
             descendantContext, initialContext.height)
           = CanonicalIndexedContext(initialContext.height)
      BY <1>1, <2>1, <2>2, <2>3,
         IndexedJoinedTargetIdentifiesEveryCanonicalAncestor
         DEF IndexedTargetJoined
    <2>5. initialContext =
             CanonicalIndexedContext(initialContext.height)
      BY <1>1 DEF IndexedContextAdvanceReady,
                    IndexedCompositionInvariant,
                    JoinedContextCertificationInvariant,
                    CanonicalIndexedContext
    <2>6. IndexedAncestorContext(
             descendantContext, initialContext.height)
           = initialContext
      BY <2>4, <2>5
    <2>7. IndexedTargetHeightStepPremise(
             descendantContext, initialContext.height)
      BY <1>1, <2>1
         DEF IndexedContextAdvanceReady,
             IndexedTargetHeightStepPremise, IndexedTargetJoined
    <2>8. \/ IndexedNodePastContext(
                  IndexedAncestorContext(
                    descendantContext, initialContext.height), node)
             \/ IndexedActivationPendingIntoContext(
                  IndexedAncestorContext(
                    descendantContext, initialContext.height), node)
             \/ HistoricalRecoveryOutstanding(
                  IndexedAncestorContext(
                    descendantContext, initialContext.height), node)
      BY <1>1, <2>1, <2>2, <2>3, <2>7,
         IndexedTargetStepEitherPassedOrRecoveryOutstanding
    <2> QED BY <2>6, <2>8
  <1> QED BY <1>1

THEOREM IndexedAdvanceReadyEventuallyPassesEachResponsiveNode ==
  /\ IndexedChainSpec
  /\ IndexedExactHistoricalRecoveryProgress
  /\ IndexedSuccessorActivationProgress
  =>
    \A initialContext \in AdmissibleContextRecords,
       node \in Responsive:
      IndexedContextAdvanceReady(initialContext)
        ~> IndexedNodePastContext(initialContext, node)
PROOF
  <1>1. ASSUME IndexedChainSpec,
              IndexedExactHistoricalRecoveryProgress,
              IndexedSuccessorActivationProgress,
              NEW initialContext \in AdmissibleContextRecords,
              NEW node \in Responsive
         PROVE IndexedContextAdvanceReady(initialContext)
                 ~> IndexedNodePastContext(initialContext, node)
    <2>1. []IndexedCompositionInvariant
      BY <1>1, IndexedChainSpecEstablishesCompositionInvariant, PTL
    <2>2. []IndexedJoinedThroughLocalHeight
      BY <1>1, IndexedChainSpecJoinsEveryNodeThroughLocalHeight, PTL
    <2>3. IndexedCompositionInvariant
             /\ IndexedJoinedThroughLocalHeight
             /\ IndexedContextAdvanceReady(initialContext)
             => \/ IndexedNodePastContext(initialContext, node)
                \/ IndexedActivationPendingIntoContext(
                     initialContext, node)
                \/ HistoricalRecoveryOutstanding(
                     initialContext, node)
      BY <1>1, IndexedAdvanceReadyEitherPassedOrNeedsRecovery
    <2>4. initialContext.height < MaxHeight
      BY <1>1, JoinedCanonicalDescendantStaysWithinHorizon
         DEF IndexedContextAdvanceReady
    <2>5. IndexedActivationPendingIntoContext(initialContext, node)
             ~> (IndexedNodePastContext(initialContext, node)
                  \/ HistoricalRecoveryOutstanding(initialContext, node))
      BY <1>1, <2>4,
         IndexedActivationPendingEventuallyLeavesPastOrRecoveryOutstanding
    <2>6. HistoricalRecoveryOutstanding(initialContext, node)
             ~> IndexedNodePastContext(initialContext, node)
      BY <1>1, <2>4,
         IndexedHistoricalRecoveryAdvancesResponsiveNode
         DEF IndexedNodePastContext
    <2> QED BY <2>1, <2>2, <2>3, <2>5, <2>6, PTL
  <1> QED BY <1>1

THEOREM IndexedAdvanceReadyPassesEveryFiniteResponsivePrefix ==
  /\ IndexedChainSpec
  /\ IndexedExactHistoricalRecoveryProgress
  /\ IndexedSuccessorActivationProgress
  =>
    \A initialContext \in AdmissibleContextRecords,
       limit \in Nat:
      IndexedContextAdvanceReady(initialContext)
        ~> IndexedResponsivePrefixPast(initialContext, limit)
PROOF
  <1>1. ASSUME IndexedChainSpec,
              IndexedExactHistoricalRecoveryProgress,
              IndexedSuccessorActivationProgress,
              NEW initialContext \in AdmissibleContextRecords
         PROVE \A limit \in Nat:
                 IndexedContextAdvanceReady(initialContext)
                   ~> IndexedResponsivePrefixPast(initialContext, limit)
    <2> DEFINE P(limit) ==
           IndexedContextAdvanceReady(initialContext)
             ~> IndexedResponsivePrefixPast(initialContext, limit)
    <2>1. P(0)
      <3>1. CASE 0 \in Responsive
        <4>1. IndexedContextAdvanceReady(initialContext)
                 ~> IndexedNodePastContext(initialContext, 0)
          BY <1>1, <3>1,
             IndexedAdvanceReadyEventuallyPassesEachResponsiveNode
        <4> QED BY <4>1, PTL
             DEF P, IndexedResponsivePrefixPast
      <3>2. CASE 0 \notin Responsive
        BY <3>2, PTL DEF P, IndexedResponsivePrefixPast
      <3> QED BY <3>1, <3>2
    <2>2. ASSUME NEW limit \in Nat,
                  P(limit)
           PROVE P(limit + 1)
      <3>1. CASE limit + 1 \in Responsive
        <4>1. IndexedContextAdvanceReady(initialContext)
                 ~> IndexedNodePastContext(
                      initialContext, limit + 1)
          BY <1>1, <3>1,
             IndexedAdvanceReadyEventuallyPassesEachResponsiveNode
        <4>2. IndexedResponsivePrefixPast(initialContext, limit)
                 /\ [IndexedChainNext]_IndexedChainVars
                 => IndexedResponsivePrefixPast(
                      initialContext, limit)'
          BY <1>1, <2>2, IndexedResponsivePrefixPastIsStable
        <4>3. IndexedNodePastContext(initialContext, limit + 1)
                 /\ [IndexedChainNext]_IndexedChainVars
                 => IndexedNodePastContext(
                      initialContext, limit + 1)'
          BY <1>1, <3>1, IndexedNodePastContextIsStable,
             Isa DEF ModelConfiguration, ValidatorIds
        <4>4. IndexedResponsivePrefixPast(initialContext, limit + 1)
                 <=> /\ IndexedResponsivePrefixPast(
                           initialContext, limit)
                     /\ IndexedNodePastContext(
                           initialContext, limit + 1)
          BY <2>2, <3>1, Isa
             DEF IndexedResponsivePrefixPast
        <4> QED BY <2>2, <4>1, <4>2, <4>3, <4>4, PTL DEF P
      <3>2. CASE limit + 1 \notin Responsive
        <4>1. IndexedResponsivePrefixPast(initialContext, limit)
                 => IndexedResponsivePrefixPast(
                      initialContext, limit + 1)
          BY <2>2, <3>2, Isa DEF IndexedResponsivePrefixPast
        <4> QED BY <2>2, <4>1, PTL DEF P
      <3> QED BY <3>1, <3>2
    <2>3. \A limit \in Nat: P(limit)
      BY <2>1, <2>2, NatInduction
    <2> QED BY <2>3 DEF P
  <1> QED BY <1>1

THEOREM IndexedAdvanceReadyReachesSuccessorHeight ==
  /\ IndexedChainSpec
  /\ IndexedExactHistoricalRecoveryProgress
  /\ IndexedSuccessorActivationProgress
  =>
    \A initialContext \in AdmissibleContextRecords:
      IndexedContextAdvanceReady(initialContext)
        ~> IndexedResponsiveHeightReached(initialContext.height + 1)
PROOF
  <1>1. ASSUME IndexedChainSpec,
              IndexedExactHistoricalRecoveryProgress,
              IndexedSuccessorActivationProgress,
              NEW initialContext \in AdmissibleContextRecords
         PROVE IndexedContextAdvanceReady(initialContext)
                 ~> IndexedResponsiveHeightReached(
                      initialContext.height + 1)
    <2>1. IndexedContextAdvanceReady(initialContext)
             ~> IndexedResponsivePrefixPast(initialContext, N - 1)
      BY <1>1, IndexedAdvanceReadyPassesEveryFiniteResponsivePrefix,
         SMT DEF ModelConfiguration
    <2>2. IndexedResponsivePrefixPast(initialContext, N - 1)
             => IndexedResponsiveHeightReached(
                  initialContext.height + 1)
      BY SMT DEF IndexedResponsivePrefixPast,
                 IndexedResponsiveHeightReached,
                 IndexedNodePastContext, ModelConfiguration,
                 ValidatorIds, AdmissibleContextRecords,
                 FrozenContextAdmissible, ContextRecords, Heights
    <2> QED BY <2>1, <2>2, PTL
  <1> QED BY <1>1

THEOREM IndexedTargetStepEventuallyPassesEachResponsiveNode ==
  /\ IndexedChainSpec
  /\ IndexedExactHistoricalRecoveryProgress
  /\ IndexedSuccessorActivationProgress
  =>
    \A targetContext \in AdmissibleContextRecords:
      \A blockHeight \in 0..targetContext.height:
        \A node \in Responsive:
          blockHeight < targetContext.height
            => IndexedTargetHeightStepPremise(targetContext, blockHeight)
                 ~> IndexedNodePastContext(
                      IndexedAncestorContext(targetContext, blockHeight),
                      node)
PROOF
  <1>1. ASSUME IndexedChainSpec,
              IndexedExactHistoricalRecoveryProgress,
              IndexedSuccessorActivationProgress,
              NEW targetContext \in AdmissibleContextRecords,
              NEW blockHeight \in 0..targetContext.height,
              NEW node \in Responsive,
              blockHeight < targetContext.height
         PROVE IndexedTargetHeightStepPremise(
                   targetContext, blockHeight)
                 ~> IndexedNodePastContext(
                      IndexedAncestorContext(targetContext, blockHeight),
                      node)
    <2>1. []IndexedCompositionInvariant
      BY <1>1, IndexedChainSpecEstablishesCompositionInvariant, PTL
    <2>2. []IndexedJoinedThroughLocalHeight
      BY <1>1, IndexedChainSpecJoinsEveryNodeThroughLocalHeight, PTL
    <2>3. IndexedCompositionInvariant
             /\ IndexedJoinedThroughLocalHeight
             /\ IndexedTargetHeightStepPremise(
                  targetContext, blockHeight)
             => \/ IndexedNodePastContext(
                     IndexedAncestorContext(targetContext, blockHeight),
                     node)
                \/ IndexedActivationPendingIntoContext(
                     IndexedAncestorContext(targetContext, blockHeight),
                     node)
                \/ HistoricalRecoveryOutstanding(
                     IndexedAncestorContext(targetContext, blockHeight),
                     node)
      BY <1>1,
         IndexedTargetStepEitherPassedOrRecoveryOutstanding
    <2>4. IndexedAncestorContext(targetContext, blockHeight).height
             < MaxHeight
      BY <1>1, IndexedAdmissibleTargetHasAdmissibleAncestors,
         Isa DEF IndexedAncestorContext,
                 AdmissibleContextRecords, FrozenContextAdmissible,
                 ContextRecords, Heights
    <2>5. IndexedActivationPendingIntoContext(
               IndexedAncestorContext(targetContext, blockHeight), node)
             ~> (IndexedNodePastContext(
                    IndexedAncestorContext(targetContext, blockHeight), node)
                  \/ HistoricalRecoveryOutstanding(
                       IndexedAncestorContext(targetContext, blockHeight),
                       node))
      BY <1>1, <2>4,
         IndexedAdmissibleTargetHasAdmissibleAncestors,
         IndexedActivationPendingEventuallyLeavesPastOrRecoveryOutstanding
    <2>6. HistoricalRecoveryOutstanding(
               IndexedAncestorContext(targetContext, blockHeight), node)
             ~> IndexedNodePastContext(
                  IndexedAncestorContext(targetContext, blockHeight), node)
      BY <1>1, <2>4,
         IndexedAdmissibleTargetHasAdmissibleAncestors,
         IndexedHistoricalRecoveryAdvancesResponsiveNode
         DEF IndexedNodePastContext
    <2> QED BY <2>1, <2>2, <2>3, <2>5, <2>6, PTL
  <1> QED BY <1>1

THEOREM IndexedTargetStepPassesEveryFiniteResponsivePrefix ==
  /\ IndexedChainSpec
  /\ IndexedExactHistoricalRecoveryProgress
  /\ IndexedSuccessorActivationProgress
  =>
    \A targetContext \in AdmissibleContextRecords:
      \A blockHeight \in 0..targetContext.height:
        \A limit \in Nat:
          blockHeight < targetContext.height
            => IndexedTargetHeightStepPremise(targetContext, blockHeight)
                 ~> IndexedResponsivePrefixPast(
                      IndexedAncestorContext(targetContext, blockHeight),
                      limit)
PROOF
  <1>1. ASSUME IndexedChainSpec,
              IndexedExactHistoricalRecoveryProgress,
              IndexedSuccessorActivationProgress,
              NEW targetContext \in AdmissibleContextRecords,
              NEW blockHeight \in 0..targetContext.height,
              blockHeight < targetContext.height
         PROVE \A limit \in Nat:
                 IndexedTargetHeightStepPremise(targetContext, blockHeight)
                   ~> IndexedResponsivePrefixPast(
                        IndexedAncestorContext(targetContext, blockHeight),
                        limit)
    <2> DEFINE P(limit) ==
           IndexedTargetHeightStepPremise(targetContext, blockHeight)
             ~> IndexedResponsivePrefixPast(
                  IndexedAncestorContext(targetContext, blockHeight),
                  limit)
    <2>1. P(0)
      <3>1. CASE 0 \in Responsive
        <4>1. IndexedTargetHeightStepPremise(targetContext, blockHeight)
                 ~> IndexedNodePastContext(
                      IndexedAncestorContext(targetContext, blockHeight), 0)
          BY <1>1, <3>1,
             IndexedTargetStepEventuallyPassesEachResponsiveNode
        <4> QED BY <4>1, PTL DEF P, IndexedResponsivePrefixPast
      <3>2. CASE 0 \notin Responsive
        BY <3>2, PTL DEF P, IndexedResponsivePrefixPast
      <3> QED BY <3>1, <3>2
    <2>2. ASSUME NEW limit \in Nat, P(limit)
           PROVE P(limit + 1)
      <3>1. CASE limit + 1 \in Responsive
        <4>1. IndexedTargetHeightStepPremise(targetContext, blockHeight)
                 ~> IndexedNodePastContext(
                      IndexedAncestorContext(targetContext, blockHeight),
                      limit + 1)
          BY <1>1, <3>1,
             IndexedTargetStepEventuallyPassesEachResponsiveNode
        <4>2. IndexedResponsivePrefixPast(
                 IndexedAncestorContext(targetContext, blockHeight), limit)
                 /\ [IndexedChainNext]_IndexedChainVars
                 => IndexedResponsivePrefixPast(
                      IndexedAncestorContext(targetContext, blockHeight),
                      limit)'
          BY <1>1, <2>2, IndexedAdmissibleTargetHasAdmissibleAncestors,
             IndexedResponsivePrefixPastIsStable
        <4>3. IndexedNodePastContext(
                 IndexedAncestorContext(targetContext, blockHeight),
                 limit + 1)
                 /\ [IndexedChainNext]_IndexedChainVars
                 => IndexedNodePastContext(
                      IndexedAncestorContext(targetContext, blockHeight),
                      limit + 1)'
          BY <1>1, <3>1,
             IndexedAdmissibleTargetHasAdmissibleAncestors,
             IndexedNodePastContextIsStable,
             Isa DEF ModelConfiguration, ValidatorIds
        <4>4. IndexedResponsivePrefixPast(
                 IndexedAncestorContext(targetContext, blockHeight),
                 limit + 1)
                 <=> /\ IndexedResponsivePrefixPast(
                           IndexedAncestorContext(targetContext, blockHeight),
                           limit)
                     /\ IndexedNodePastContext(
                           IndexedAncestorContext(targetContext, blockHeight),
                           limit + 1)
          BY <2>2, <3>1, Isa DEF IndexedResponsivePrefixPast
        <4> QED BY <2>2, <4>1, <4>2, <4>3, <4>4, PTL DEF P
      <3>2. CASE limit + 1 \notin Responsive
        <4>1. IndexedResponsivePrefixPast(
                 IndexedAncestorContext(targetContext, blockHeight), limit)
                 => IndexedResponsivePrefixPast(
                      IndexedAncestorContext(targetContext, blockHeight),
                      limit + 1)
          BY <2>2, <3>2, Isa DEF IndexedResponsivePrefixPast
        <4> QED BY <2>2, <4>1, PTL DEF P
      <3> QED BY <3>1, <3>2
    <2>3. \A limit \in Nat: P(limit)
      BY <2>1, <2>2, NatInduction
    <2> QED BY <2>3 DEF P
  <1> QED BY <1>1

THEOREM IndexedJoinedTargetAdvancesOneAncestorHeight ==
  /\ IndexedChainSpec
  /\ IndexedExactHistoricalRecoveryProgress
  /\ IndexedSuccessorActivationProgress
  =>
    \A targetContext \in AdmissibleContextRecords:
      \A blockHeight \in 0..targetContext.height:
        blockHeight < targetContext.height
          => (IndexedTargetJoined(targetContext)
                /\ IndexedResponsiveHeightReached(blockHeight))
               ~> IndexedResponsiveHeightReached(blockHeight + 1)
PROOF
  <1>1. ASSUME IndexedChainSpec,
              IndexedExactHistoricalRecoveryProgress,
              IndexedSuccessorActivationProgress,
              NEW targetContext \in AdmissibleContextRecords,
              NEW blockHeight \in 0..targetContext.height,
              blockHeight < targetContext.height
         PROVE (IndexedTargetJoined(targetContext)
                   /\ IndexedResponsiveHeightReached(blockHeight))
                  ~> IndexedResponsiveHeightReached(blockHeight + 1)
    <2>1. []IndexedCompositionInvariant
      BY <1>1, IndexedChainSpecEstablishesCompositionInvariant, PTL
    <2>2. []IndexedJoinedThroughLocalHeight
      BY <1>1, IndexedChainSpecJoinsEveryNodeThroughLocalHeight, PTL
    <2>3. IndexedTargetJoined(targetContext)
             /\ [IndexedChainNext]_IndexedChainVars
             => IndexedTargetJoined(targetContext)'
      BY <1>1, IndexedTargetJoinedIsStable
    <2>4. IndexedResponsiveHeightReached(blockHeight)
             /\ [IndexedChainNext]_IndexedChainVars
             => IndexedResponsiveHeightReached(blockHeight)'
      BY <1>1, IndexedResponsiveHeightReachedIsStable,
         Isa DEF Heights, AdmissibleContextRecords,
                 FrozenContextAdmissible, ContextRecords
    <2>5. IndexedTargetHeightStepPremise(targetContext, blockHeight)
             ~> IndexedResponsivePrefixPast(
                  IndexedAncestorContext(targetContext, blockHeight),
                  N - 1)
      BY <1>1, IndexedTargetStepPassesEveryFiniteResponsivePrefix,
         SMT DEF ModelConfiguration
    <2>6. IndexedResponsivePrefixPast(
             IndexedAncestorContext(targetContext, blockHeight), N - 1)
             => IndexedResponsiveHeightReached(blockHeight + 1)
      BY <1>1, SMT
         DEF IndexedResponsivePrefixPast,
             IndexedResponsiveHeightReached,
             IndexedNodePastContext, IndexedAncestorContext,
             ModelConfiguration, ValidatorIds,
             AdmissibleContextRecords, FrozenContextAdmissible,
             ContextRecords, Heights
    <2> QED BY <2>3, <2>4, <2>5, <2>6, PTL
         DEF IndexedTargetHeightStepPremise
  <1> QED BY <1>1

THEOREM IndexedJoinedTargetEventuallyReachesEveryAncestorHeight ==
  /\ IndexedChainSpec
  /\ IndexedExactHistoricalRecoveryProgress
  /\ IndexedSuccessorActivationProgress
  =>
    \A targetContext \in AdmissibleContextRecords:
      \A blockHeight \in 0..targetContext.height:
        IndexedTargetJoined(targetContext)
          ~> IndexedResponsiveHeightReached(blockHeight)
PROOF
  <1>1. ASSUME IndexedChainSpec,
              IndexedExactHistoricalRecoveryProgress,
              IndexedSuccessorActivationProgress,
              NEW targetContext \in AdmissibleContextRecords
         PROVE \A blockHeight \in 0..targetContext.height:
                 IndexedTargetJoined(targetContext)
                   ~> IndexedResponsiveHeightReached(blockHeight)
    <2> DEFINE P(blockHeight) ==
           blockHeight <= targetContext.height
             => (IndexedTargetJoined(targetContext)
                   ~> IndexedResponsiveHeightReached(blockHeight))
    <2>1. P(0)
      BY SMT DEF P, IndexedResponsiveHeightReached,
                 AdmissibleContextRecords, FrozenContextAdmissible,
                 ContextRecords, Heights, ModelConfiguration,
                 ValidatorIds
    <2>2. ASSUME NEW blockHeight \in Nat,
                  P(blockHeight)
           PROVE P(blockHeight + 1)
      <3>1. CASE blockHeight < targetContext.height
        <4>1. blockHeight \in 0..targetContext.height
          BY <1>1, <2>2, <3>1, SMT
             DEF AdmissibleContextRecords, FrozenContextAdmissible,
                 ContextRecords, Heights
        <4>2. (IndexedTargetJoined(targetContext)
                  /\ IndexedResponsiveHeightReached(blockHeight))
                 ~> IndexedResponsiveHeightReached(blockHeight + 1)
          BY <1>1, <3>1, <4>1,
             IndexedJoinedTargetAdvancesOneAncestorHeight
        <4>3. IndexedTargetJoined(targetContext)
                 /\ [IndexedChainNext]_IndexedChainVars
                 => IndexedTargetJoined(targetContext)'
          BY <1>1, IndexedTargetJoinedIsStable
        <4> QED BY <2>2, <3>1, <4>2, <4>3, PTL DEF P
      <3>2. CASE blockHeight >= targetContext.height
        BY <3>2, SMT DEF P
      <3> QED BY <3>1, <3>2
    <2>3. \A blockHeight \in Nat: P(blockHeight)
      BY <2>1, <2>2, NatInduction
    <2> QED BY <2>3 DEF P
  <1> QED BY <1>1

IndexedResponsiveJoinPrefixAt(initialContext, limit) ==
  \A node \in Responsive \cap (0..limit):
    node \in joinedByContext[initialContext]

THEOREM IndexedResponsiveJoinPrefixAtIsStable ==
  \A initialContext \in AdmissibleContextRecords,
     limit \in Nat:
    IndexedResponsiveJoinPrefixAt(initialContext, limit)
      /\ [IndexedChainNext]_IndexedChainVars
      => IndexedResponsiveJoinPrefixAt(initialContext, limit)'
BY Isa, JoinedMembershipIsMonotone
   DEF IndexedResponsiveJoinPrefixAt, IndexedChainVars

THEOREM IndexedReachedAncestorEventuallyJoinsResponsiveNode ==
  /\ IndexedChainSpec
  /\ IndexedSuccessorActivationProgress
  => \A targetContext \in AdmissibleContextRecords:
       \A blockHeight \in 0..targetContext.height:
         \A node \in Responsive:
           (IndexedTargetJoined(targetContext)
             /\ IndexedResponsiveHeightReached(blockHeight))
             ~> node \in joinedByContext[
                  IndexedAncestorContext(targetContext, blockHeight)]
PROOF
  <1>1. ASSUME IndexedChainSpec,
              IndexedSuccessorActivationProgress,
              NEW targetContext \in AdmissibleContextRecords,
              NEW blockHeight \in 0..targetContext.height,
              NEW node \in Responsive
         PROVE (IndexedTargetJoined(targetContext)
                  /\ IndexedResponsiveHeightReached(blockHeight))
                 ~> node \in joinedByContext[
                      IndexedAncestorContext(targetContext, blockHeight)]
    <2>1. IndexedChainSpec => []IndexedCompositionInvariant
      BY IndexedChainSpecEstablishesCompositionInvariant
    <2>2. IndexedChainSpec => []IndexedJoinedThroughLocalHeight
      BY IndexedChainSpecJoinsEveryNodeThroughLocalHeight
    <2>3. IndexedCompositionInvariant
             /\ IndexedJoinedThroughLocalHeight
             /\ IndexedTargetJoined(targetContext)
             /\ IndexedResponsiveHeightReached(blockHeight)
             => \/ node \in joinedByContext[
                       IndexedAncestorContext(targetContext, blockHeight)]
                \/ IndexedActivationPendingIntoContext(
                     IndexedAncestorContext(targetContext, blockHeight), node)
      BY <1>1, IndexedReachedAncestorClassifiesEveryResponsiveNode
    <2>4. IndexedActivationPendingIntoContext(
             IndexedAncestorContext(targetContext, blockHeight), node)
             ~> node \in joinedByContext[
                  IndexedAncestorContext(targetContext, blockHeight)]
      BY <1>1, IndexedAdmissibleTargetHasAdmissibleAncestors,
         IndexedActivationPendingIntoContextEventuallyJoins
    <2>5. node \in joinedByContext[
             IndexedAncestorContext(targetContext, blockHeight)]
             /\ [IndexedChainNext]_IndexedChainVars
             => node \in joinedByContext[
                  IndexedAncestorContext(targetContext, blockHeight)]'
      BY <1>1, IndexedAdmissibleTargetHasAdmissibleAncestors,
         IndexedNodeJoinIsStable
    <2> QED BY <2>1, <2>2, <2>3, <2>4, <2>5, PTL
  <1> QED BY <1>1

THEOREM IndexedReachedAncestorEventuallyJoinsEveryResponsivePrefix ==
  /\ IndexedChainSpec
  /\ IndexedSuccessorActivationProgress
  => \A targetContext \in AdmissibleContextRecords:
       \A blockHeight \in 0..targetContext.height:
         \A limit \in Nat:
           (IndexedTargetJoined(targetContext)
             /\ IndexedResponsiveHeightReached(blockHeight))
             ~> IndexedResponsiveJoinPrefixAt(
                  IndexedAncestorContext(targetContext, blockHeight), limit)
PROOF
  <1>1. ASSUME IndexedChainSpec,
              IndexedSuccessorActivationProgress,
              NEW targetContext \in AdmissibleContextRecords,
              NEW blockHeight \in 0..targetContext.height
         PROVE \A limit \in Nat:
                 (IndexedTargetJoined(targetContext)
                   /\ IndexedResponsiveHeightReached(blockHeight))
                   ~> IndexedResponsiveJoinPrefixAt(
                        IndexedAncestorContext(targetContext, blockHeight),
                        limit)
    <2> DEFINE P(limit) ==
           (IndexedTargetJoined(targetContext)
             /\ IndexedResponsiveHeightReached(blockHeight))
             ~> IndexedResponsiveJoinPrefixAt(
                  IndexedAncestorContext(targetContext, blockHeight), limit)
    <2>1. P(0)
      <3>1. CASE 0 \in Responsive
        BY <1>1, <3>1,
           IndexedReachedAncestorEventuallyJoinsResponsiveNode, PTL
           DEF P, IndexedResponsiveJoinPrefixAt
      <3>2. CASE 0 \notin Responsive
        BY <3>2, PTL DEF P, IndexedResponsiveJoinPrefixAt
      <3> QED BY <3>1, <3>2
    <2>2. ASSUME NEW limit \in Nat, P(limit)
           PROVE P(limit + 1)
      <3>1. CASE limit + 1 \in Responsive
        <4>1. (IndexedTargetJoined(targetContext)
                 /\ IndexedResponsiveHeightReached(blockHeight))
                 ~> limit + 1 \in joinedByContext[
                      IndexedAncestorContext(targetContext, blockHeight)]
          BY <1>1, <3>1,
             IndexedReachedAncestorEventuallyJoinsResponsiveNode
        <4>2. IndexedResponsiveJoinPrefixAt(
                 IndexedAncestorContext(targetContext, blockHeight), limit)
                 /\ [IndexedChainNext]_IndexedChainVars
                 => IndexedResponsiveJoinPrefixAt(
                      IndexedAncestorContext(targetContext, blockHeight),
                      limit)'
          BY <1>1, <2>2, IndexedAdmissibleTargetHasAdmissibleAncestors,
             IndexedResponsiveJoinPrefixAtIsStable
        <4>3. limit + 1 \in joinedByContext[
                 IndexedAncestorContext(targetContext, blockHeight)]
                 /\ [IndexedChainNext]_IndexedChainVars
                 => limit + 1 \in joinedByContext[
                      IndexedAncestorContext(targetContext, blockHeight)]'
          BY <1>1, <3>1, IndexedAdmissibleTargetHasAdmissibleAncestors,
             IndexedNodeJoinIsStable
        <4>4. IndexedResponsiveJoinPrefixAt(
                 IndexedAncestorContext(targetContext, blockHeight),
                 limit + 1)
                 <=> /\ IndexedResponsiveJoinPrefixAt(
                           IndexedAncestorContext(targetContext, blockHeight),
                           limit)
                     /\ limit + 1 \in joinedByContext[
                           IndexedAncestorContext(targetContext, blockHeight)]
          BY <2>2, <3>1, Isa DEF IndexedResponsiveJoinPrefixAt
        <4> QED BY <2>2, <4>1, <4>2, <4>3, <4>4, PTL DEF P
      <3>2. CASE limit + 1 \notin Responsive
        <4>1. IndexedResponsiveJoinPrefixAt(
                 IndexedAncestorContext(targetContext, blockHeight), limit)
                 => IndexedResponsiveJoinPrefixAt(
                      IndexedAncestorContext(targetContext, blockHeight),
                      limit + 1)
          BY <2>2, <3>2, Isa DEF IndexedResponsiveJoinPrefixAt
        <4> QED BY <2>2, <4>1, PTL DEF P
      <3> QED BY <3>1, <3>2
    <2>3. \A limit \in Nat: P(limit)
      BY <2>1, <2>2, NatInduction
    <2> QED BY <2>3 DEF P
  <1> QED BY <1>1

THEOREM IndexedReachedAncestorEventuallyJoinsEveryResponsiveNode ==
  /\ IndexedChainSpec
  /\ IndexedSuccessorActivationProgress
  => \A targetContext \in AdmissibleContextRecords:
       \A blockHeight \in 0..targetContext.height:
         (IndexedTargetJoined(targetContext)
           /\ IndexedResponsiveHeightReached(blockHeight))
           ~> IndexedAllResponsiveJoined(
                IndexedAncestorContext(targetContext, blockHeight))
BY IndexedReachedAncestorEventuallyJoinsEveryResponsivePrefix, SMT
   DEF IndexedResponsiveJoinPrefixAt,
       IndexedAllResponsiveJoined,
       ModelConfiguration, ValidatorIds

(***************************************************************************
Strict-ancestor catchup kernel.

The older finite-height induction above consumes the global
`IndexedExactHistoricalRecoveryProgress` property.  That interface is too
broad for proving historical source acquisition itself: recovery at the
frozen target height would then be one of its own premises.

`IndexedStrictAncestorRecoveryAdvance` removes that cycle.  For one frozen
joined target it assumes only that an outstanding responsive node eventually
passes each strict ancestor.  Existing indexed safety classifies every node
as already past, pending the typed successor lifecycle, or outstanding at
that exact ancestor.  The finite responsive-prefix and height inductions
below then reach the target height and join every responsive node to the
target.  No current-height recovery, one-height completion, or indexed
height-liveness property is consumed.
***************************************************************************)

IndexedStrictAncestorRecoveryAdvance(targetContext) ==
  \A blockHeight \in 0..targetContext.height:
    blockHeight < targetContext.height
      => \A node \in Responsive:
           HistoricalRecoveryOutstanding(
             IndexedAncestorContext(targetContext, blockHeight), node)
             ~> IndexedNodePastContext(
                  IndexedAncestorContext(targetContext, blockHeight), node)

THEOREM IndexedTargetStepPassesEachResponsiveNodeFromStrictAncestorRecovery ==
  /\ IndexedChainSpec
  /\ IndexedSuccessorActivationProgress
  => \A targetContext \in AdmissibleContextRecords:
       IndexedStrictAncestorRecoveryAdvance(targetContext)
         => \A blockHeight \in 0..targetContext.height:
              \A node \in Responsive:
                blockHeight < targetContext.height
                  => IndexedTargetHeightStepPremise(
                       targetContext, blockHeight)
                       ~> IndexedNodePastContext(
                            IndexedAncestorContext(
                              targetContext, blockHeight),
                            node)
PROOF
  <1>1. ASSUME IndexedChainSpec,
              IndexedSuccessorActivationProgress,
              targetContext \in AdmissibleContextRecords,
              IndexedStrictAncestorRecoveryAdvance(targetContext),
              NEW blockHeight \in 0..targetContext.height,
              NEW node \in Responsive,
              blockHeight < targetContext.height
         PROVE IndexedTargetHeightStepPremise(
                   targetContext, blockHeight)
                 ~> IndexedNodePastContext(
                      IndexedAncestorContext(targetContext, blockHeight),
                      node)
    <2>1. []IndexedCompositionInvariant
      BY <1>1, IndexedChainSpecEstablishesCompositionInvariant, PTL
    <2>2. []IndexedJoinedThroughLocalHeight
      BY <1>1, IndexedChainSpecJoinsEveryNodeThroughLocalHeight, PTL
    <2>3. IndexedCompositionInvariant
             /\ IndexedJoinedThroughLocalHeight
             /\ IndexedTargetHeightStepPremise(
                  targetContext, blockHeight)
             => \/ IndexedNodePastContext(
                     IndexedAncestorContext(targetContext, blockHeight),
                     node)
                \/ IndexedActivationPendingIntoContext(
                     IndexedAncestorContext(targetContext, blockHeight),
                     node)
                \/ HistoricalRecoveryOutstanding(
                     IndexedAncestorContext(targetContext, blockHeight),
                     node)
      BY <1>1, IndexedTargetStepEitherPassedOrRecoveryOutstanding
    <2>4. IndexedAncestorContext(targetContext, blockHeight).height
             < MaxHeight
      BY <1>1, IndexedAdmissibleTargetHasAdmissibleAncestors,
         Isa DEF IndexedAncestorContext,
                 AdmissibleContextRecords, FrozenContextAdmissible,
                 ContextRecords, Heights
    <2>5. IndexedActivationPendingIntoContext(
               IndexedAncestorContext(targetContext, blockHeight), node)
             ~> (IndexedNodePastContext(
                    IndexedAncestorContext(targetContext, blockHeight), node)
                  \/ HistoricalRecoveryOutstanding(
                       IndexedAncestorContext(targetContext, blockHeight),
                       node))
      BY <1>1, <2>4,
         IndexedAdmissibleTargetHasAdmissibleAncestors,
         IndexedActivationPendingEventuallyLeavesPastOrRecoveryOutstanding
    <2>6. HistoricalRecoveryOutstanding(
               IndexedAncestorContext(targetContext, blockHeight), node)
             ~> IndexedNodePastContext(
                  IndexedAncestorContext(targetContext, blockHeight), node)
      BY <1>1 DEF IndexedStrictAncestorRecoveryAdvance
    <2> QED BY <2>1, <2>2, <2>3, <2>5, <2>6, PTL
  <1> QED BY <1>1

THEOREM IndexedTargetStepPassesEveryResponsivePrefixFromStrictAncestorRecovery ==
  /\ IndexedChainSpec
  /\ IndexedSuccessorActivationProgress
  => \A targetContext \in AdmissibleContextRecords:
       IndexedStrictAncestorRecoveryAdvance(targetContext)
         => \A blockHeight \in 0..targetContext.height:
              \A limit \in Nat:
                blockHeight < targetContext.height
                  => IndexedTargetHeightStepPremise(
                       targetContext, blockHeight)
                       ~> IndexedResponsivePrefixPast(
                            IndexedAncestorContext(
                              targetContext, blockHeight),
                            limit)
PROOF
  <1>1. ASSUME IndexedChainSpec,
              IndexedSuccessorActivationProgress,
              targetContext \in AdmissibleContextRecords,
              IndexedStrictAncestorRecoveryAdvance(targetContext),
              NEW blockHeight \in 0..targetContext.height,
              blockHeight < targetContext.height
         PROVE \A limit \in Nat:
                 IndexedTargetHeightStepPremise(targetContext, blockHeight)
                   ~> IndexedResponsivePrefixPast(
                        IndexedAncestorContext(targetContext, blockHeight),
                        limit)
    <2> DEFINE P(limit) ==
           IndexedTargetHeightStepPremise(targetContext, blockHeight)
             ~> IndexedResponsivePrefixPast(
                  IndexedAncestorContext(targetContext, blockHeight),
                  limit)
    <2>1. P(0)
      <3>1. CASE 0 \in Responsive
        <4>1. IndexedTargetHeightStepPremise(targetContext, blockHeight)
                 ~> IndexedNodePastContext(
                      IndexedAncestorContext(targetContext, blockHeight), 0)
          BY <1>1, <3>1,
             IndexedTargetStepPassesEachResponsiveNodeFromStrictAncestorRecovery
        <4> QED BY <4>1, PTL DEF P, IndexedResponsivePrefixPast
      <3>2. CASE 0 \notin Responsive
        BY <3>2, PTL DEF P, IndexedResponsivePrefixPast
      <3> QED BY <3>1, <3>2
    <2>2. ASSUME NEW limit \in Nat, P(limit)
           PROVE P(limit + 1)
      <3>1. CASE limit + 1 \in Responsive
        <4>1. IndexedTargetHeightStepPremise(targetContext, blockHeight)
                 ~> IndexedNodePastContext(
                      IndexedAncestorContext(targetContext, blockHeight),
                      limit + 1)
          BY <1>1, <3>1,
             IndexedTargetStepPassesEachResponsiveNodeFromStrictAncestorRecovery
        <4>2. IndexedResponsivePrefixPast(
                 IndexedAncestorContext(targetContext, blockHeight), limit)
                 /\ [IndexedChainNext]_IndexedChainVars
                 => IndexedResponsivePrefixPast(
                      IndexedAncestorContext(targetContext, blockHeight),
                      limit)'
          BY <1>1, <2>2, IndexedAdmissibleTargetHasAdmissibleAncestors,
             IndexedResponsivePrefixPastIsStable
        <4>3. IndexedNodePastContext(
                 IndexedAncestorContext(targetContext, blockHeight),
                 limit + 1)
                 /\ [IndexedChainNext]_IndexedChainVars
                 => IndexedNodePastContext(
                      IndexedAncestorContext(targetContext, blockHeight),
                      limit + 1)'
          BY <1>1, <3>1,
             IndexedAdmissibleTargetHasAdmissibleAncestors,
             IndexedNodePastContextIsStable,
             Isa DEF ModelConfiguration, ValidatorIds
        <4>4. IndexedResponsivePrefixPast(
                 IndexedAncestorContext(targetContext, blockHeight),
                 limit + 1)
                 <=> /\ IndexedResponsivePrefixPast(
                           IndexedAncestorContext(targetContext, blockHeight),
                           limit)
                     /\ IndexedNodePastContext(
                           IndexedAncestorContext(targetContext, blockHeight),
                           limit + 1)
          BY <2>2, <3>1, Isa DEF IndexedResponsivePrefixPast
        <4> QED BY <2>2, <4>1, <4>2, <4>3, <4>4, PTL DEF P
      <3>2. CASE limit + 1 \notin Responsive
        <4>1. IndexedResponsivePrefixPast(
                 IndexedAncestorContext(targetContext, blockHeight), limit)
                 => IndexedResponsivePrefixPast(
                      IndexedAncestorContext(targetContext, blockHeight),
                      limit + 1)
          BY <2>2, <3>2, Isa DEF IndexedResponsivePrefixPast
        <4> QED BY <2>2, <4>1, PTL DEF P
      <3> QED BY <3>1, <3>2
    <2>3. \A limit \in Nat: P(limit)
      BY <2>1, <2>2, NatInduction
    <2> QED BY <2>3 DEF P
  <1> QED BY <1>1

THEOREM IndexedJoinedTargetAdvancesAncestorFromStrictRecovery ==
  /\ IndexedChainSpec
  /\ IndexedSuccessorActivationProgress
  => \A targetContext \in AdmissibleContextRecords:
       IndexedStrictAncestorRecoveryAdvance(targetContext)
         => \A blockHeight \in 0..targetContext.height:
              blockHeight < targetContext.height
                => (IndexedTargetJoined(targetContext)
                      /\ IndexedResponsiveHeightReached(blockHeight))
                     ~> IndexedResponsiveHeightReached(blockHeight + 1)
PROOF
  <1>1. ASSUME IndexedChainSpec,
              IndexedSuccessorActivationProgress,
              targetContext \in AdmissibleContextRecords,
              IndexedStrictAncestorRecoveryAdvance(targetContext),
              NEW blockHeight \in 0..targetContext.height,
              blockHeight < targetContext.height
         PROVE (IndexedTargetJoined(targetContext)
                   /\ IndexedResponsiveHeightReached(blockHeight))
                  ~> IndexedResponsiveHeightReached(blockHeight + 1)
    <2>1. IndexedTargetJoined(targetContext)
             /\ [IndexedChainNext]_IndexedChainVars
             => IndexedTargetJoined(targetContext)'
      BY <1>1, IndexedTargetJoinedIsStable
    <2>2. IndexedResponsiveHeightReached(blockHeight)
             /\ [IndexedChainNext]_IndexedChainVars
             => IndexedResponsiveHeightReached(blockHeight)'
      BY <1>1, IndexedResponsiveHeightReachedIsStable,
         Isa DEF Heights, AdmissibleContextRecords,
                 FrozenContextAdmissible, ContextRecords
    <2>3. IndexedTargetHeightStepPremise(targetContext, blockHeight)
             ~> IndexedResponsivePrefixPast(
                  IndexedAncestorContext(targetContext, blockHeight),
                  N - 1)
      BY <1>1,
         IndexedTargetStepPassesEveryResponsivePrefixFromStrictAncestorRecovery,
         SMT DEF ModelConfiguration
    <2>4. IndexedResponsivePrefixPast(
             IndexedAncestorContext(targetContext, blockHeight), N - 1)
             => IndexedResponsiveHeightReached(blockHeight + 1)
      BY <1>1, SMT
         DEF IndexedResponsivePrefixPast,
             IndexedResponsiveHeightReached,
             IndexedNodePastContext, IndexedAncestorContext,
             ModelConfiguration, ValidatorIds,
             AdmissibleContextRecords, FrozenContextAdmissible,
             ContextRecords, Heights
    <2> QED BY <2>1, <2>2, <2>3, <2>4, PTL
         DEF IndexedTargetHeightStepPremise
  <1> QED BY <1>1

THEOREM IndexedJoinedTargetReachesEveryAncestorFromStrictRecovery ==
  /\ IndexedChainSpec
  /\ IndexedSuccessorActivationProgress
  => \A targetContext \in AdmissibleContextRecords:
       IndexedStrictAncestorRecoveryAdvance(targetContext)
         => \A blockHeight \in 0..targetContext.height:
              IndexedTargetJoined(targetContext)
                ~> IndexedResponsiveHeightReached(blockHeight)
PROOF
  <1>1. ASSUME IndexedChainSpec,
              IndexedSuccessorActivationProgress,
              targetContext \in AdmissibleContextRecords,
              IndexedStrictAncestorRecoveryAdvance(targetContext)
         PROVE \A blockHeight \in 0..targetContext.height:
                 IndexedTargetJoined(targetContext)
                   ~> IndexedResponsiveHeightReached(blockHeight)
    <2> DEFINE P(blockHeight) ==
           blockHeight <= targetContext.height
             => (IndexedTargetJoined(targetContext)
                   ~> IndexedResponsiveHeightReached(blockHeight))
    <2>1. P(0)
      BY <1>1, SMT
         DEF P, IndexedResponsiveHeightReached,
             AdmissibleContextRecords, FrozenContextAdmissible,
             ContextRecords, Heights, ModelConfiguration,
             ValidatorIds
    <2>2. ASSUME NEW blockHeight \in Nat, P(blockHeight)
           PROVE P(blockHeight + 1)
      <3>1. CASE blockHeight < targetContext.height
        <4>1. blockHeight \in 0..targetContext.height
          BY <1>1, <2>2, <3>1, SMT
             DEF AdmissibleContextRecords, FrozenContextAdmissible,
                 ContextRecords, Heights
        <4>2. (IndexedTargetJoined(targetContext)
                  /\ IndexedResponsiveHeightReached(blockHeight))
                 ~> IndexedResponsiveHeightReached(blockHeight + 1)
          BY <1>1, <3>1, <4>1,
             IndexedJoinedTargetAdvancesAncestorFromStrictRecovery
        <4>3. IndexedTargetJoined(targetContext)
                 /\ [IndexedChainNext]_IndexedChainVars
                 => IndexedTargetJoined(targetContext)'
          BY <1>1, IndexedTargetJoinedIsStable
        <4> QED BY <2>2, <3>1, <4>2, <4>3, PTL DEF P
      <3>2. CASE blockHeight >= targetContext.height
        BY <3>2, SMT DEF P
      <3> QED BY <3>1, <3>2
    <2>3. \A blockHeight \in Nat: P(blockHeight)
      BY <2>1, <2>2, NatInduction
    <2> QED BY <2>3 DEF P
  <1> QED BY <1>1

THEOREM IndexedStrictAncestorRecoveryEventuallyJoinsTarget ==
  /\ IndexedChainSpec
  /\ IndexedSuccessorActivationProgress
  => \A targetContext \in AdmissibleContextRecords:
       IndexedStrictAncestorRecoveryAdvance(targetContext)
         => IndexedTargetJoined(targetContext)
              ~> IndexedAllResponsiveJoined(targetContext)
PROOF
  <1>1. ASSUME IndexedChainSpec,
              IndexedSuccessorActivationProgress,
              targetContext \in AdmissibleContextRecords,
              IndexedStrictAncestorRecoveryAdvance(targetContext)
         PROVE IndexedTargetJoined(targetContext)
                 ~> IndexedAllResponsiveJoined(targetContext)
    <2>1. targetContext.height \in 0..targetContext.height
      BY <1>1, Isa
         DEF AdmissibleContextRecords, FrozenContextAdmissible,
             ContextRecords, Heights
    <2>2. IndexedTargetJoined(targetContext)
             ~> IndexedResponsiveHeightReached(targetContext.height)
      BY <1>1, <2>1,
         IndexedJoinedTargetReachesEveryAncestorFromStrictRecovery
    <2>3. IndexedTargetJoined(targetContext)
             /\ [IndexedChainNext]_IndexedChainVars
             => IndexedTargetJoined(targetContext)'
      BY <1>1, IndexedTargetJoinedIsStable
    <2>4. (IndexedTargetJoined(targetContext)
              /\ IndexedResponsiveHeightReached(targetContext.height))
             ~> IndexedAllResponsiveJoined(
                  IndexedAncestorContext(
                    targetContext, targetContext.height))
      BY <1>1, <2>1,
         IndexedReachedAncestorEventuallyJoinsEveryResponsiveNode
    <2>5. IndexedAncestorContext(targetContext, targetContext.height)
             = targetContext
      BY <1>1, Isa
         DEF IndexedAncestorContext, AdmissibleContextRecords,
             FrozenContextAdmissible, ContextRecords, LineagesAt,
             Heights, ContextRecord
    <2> QED BY <2>2, <2>3, <2>4, <2>5, PTL
  <1> QED BY <1>1

THEOREM IndexedStrictAncestorRecoveryEventuallyActivatesResponsiveRoster ==
  /\ IndexedChainSpec
  /\ IndexedSuccessorActivationProgress
  => \A targetContext \in AdmissibleContextRecords:
       IndexedStrictAncestorRecoveryAdvance(targetContext)
         => IndexedTargetJoined(targetContext)
              ~> (Responsive \subseteq
                    IndexedAsync(targetContext)!AsyncActiveServiceNodes)
BY IndexedStrictAncestorRecoveryEventuallyJoinsTarget,
   IndexedChainSpecEstablishesCompositionInvariant,
   IndexedAllResponsiveJoinedHasActiveRoster, PTL

IndexedAllResponsiveExactApplicationsAt(initialContext) ==
  \A node \in Responsive:
    IndexedAsync(initialContext)!NodeHasApplication(node)

IndexedContextCompleted(initialContext) ==
  IF initialContext.height = MaxHeight
  THEN IndexedAllResponsiveExactApplicationsAt(initialContext)
  ELSE \A node \in Responsive:
         nodeHeight[node] > initialContext.height

THEOREM IndexedAllResponsiveExactApplicationsImpliesContextCompleted ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedCompositionInvariant
      /\ IndexedAllResponsiveExactApplicationsAt(initialContext)
      => IndexedContextCompleted(initialContext)
BY Isa DEF IndexedCompositionInvariant,
           IndexedApplicationsRespectNodeHeight,
           IndexedAllResponsiveExactApplicationsAt,
           IndexedContextCompleted,
           IndexedAsync!NodeHasApplication

THEOREM IndexedAllResponsiveExactApplicationsIsStable ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedAllResponsiveExactApplicationsAt(initialContext)
      /\ [IndexedChainNext]_IndexedChainVars
      => IndexedAllResponsiveExactApplicationsAt(initialContext)'
BY Isa DEF IndexedAllResponsiveExactApplicationsAt,
           IndexedChainNext, IndexedChainVars,
           IndexedProductActionAt, IndexedReceiptClassification,
           IndexedReceiptFreeChainStutter,
           IndexedDecisionReceiptHandoff,
           IndexedApplicationReceiptHandoff,
           NewIndexedApplicationReceipt,
           NoNewIndexedDurableReceipt,
           IndexedApplications, IndexedAsync!NodeHasApplication

THEOREM IndexedContextCompletedIsStable ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedContextCompleted(initialContext)
      /\ [IndexedChainNext]_IndexedChainVars
      => IndexedContextCompleted(initialContext)'
BY Isa, IndexedAllResponsiveExactApplicationsIsStable,
   IndexedBracketStepKeepsNodeHeightsMonotone
   DEF IndexedContextCompleted,
       IndexedAllResponsiveExactApplicationsAt,
       ModelConfiguration, ValidatorIds, Heights,
       AdmissibleContextRecords, FrozenContextAdmissible,
       ContextRecords

THEOREM VerificationSuccessorHeightImpliesContextCompleted ==
  VerificationContext \in AdmissibleContextRecords
    /\ VerificationContext.height < MaxHeight
    /\ IndexedResponsiveHeightReached(VerificationContext.height + 1)
    => IndexedContextCompleted(VerificationContext)
BY Isa DEF IndexedResponsiveHeightReached,
           IndexedContextCompleted,
           IndexedAllResponsiveExactApplicationsAt,
           ModelConfiguration, ValidatorIds,
           AdmissibleContextRecords, FrozenContextAdmissible,
           ContextRecords, Heights

(***************************************************************************
Once the voting roster has applied the exact frontier receipt, every remaining
responsive observer is either already past the context or has an exact
historical-recovery source/target. Finiteness of Responsive plus
IndexedExactHistoricalRecoveryProgress closes those observers one at a time.
At MaxHeight the outcome is exact per-context application evidence; below the
horizon the same receipt handoff advances nodeHeight.
***************************************************************************)
THEOREM VerificationAppliedFrontierEventuallyCompletes ==
  /\ IndexedChainSpec
  /\ IndexedExactHistoricalRecoveryProgress
  /\ IndexedSuccessorActivationProgress
  /\ VerificationContext \in AdmissibleContextRecords
  => (/\ IndexedTargetJoined(VerificationContext)
      /\ IndexedResponsiveHeightReached(VerificationContext.height)
      /\ IndexedAsync(VerificationContext)!
           AsyncAllResponsiveAppliedAt(VerificationContext))
       ~> IndexedContextCompleted(VerificationContext)
BY IndexedReachedAncestorEventuallyJoinsEveryResponsiveNode,
   IndexedJoinedTargetIdentifiesEveryCanonicalAncestor,
   IndexedHistoricalRecoveryAdvancesResponsiveNode,
   IndexedContextCompletedIsStable, PTL
   DEF IndexedTargetJoined, IndexedResponsiveHeightReached,
       IndexedContextCompleted,
       IndexedAllResponsiveExactApplicationsAt,
       HistoricalRecoveryOutstanding,
       IndexedHistoricalRecoveryReady,
       HistoricalRecoveryComplete, IndexedAsync!AsyncVotersAt,
       IndexedAsync!AsyncAllResponsiveAppliedAt,
       IndexedAsync!NodeHasApplication,
       ModelConfiguration, ValidatorIds

THEOREM VerificationAdvanceReadyEventuallyCompletes ==
  /\ IndexedChainSpec
    /\ IndexedExactHistoricalRecoveryProgress
    /\ IndexedSuccessorActivationProgress
    /\ VerificationContext \in AdmissibleContextRecords
    => IndexedContextAdvanceReady(VerificationContext)
         ~> IndexedContextCompleted(VerificationContext)
PROOF
  <1>1. ASSUME IndexedChainSpec,
              IndexedExactHistoricalRecoveryProgress,
              IndexedSuccessorActivationProgress,
              VerificationContext \in AdmissibleContextRecords
         PROVE IndexedContextAdvanceReady(VerificationContext)
                 ~> IndexedContextCompleted(VerificationContext)
    <2>1. IndexedContextAdvanceReady(VerificationContext)
             ~> IndexedResponsiveHeightReached(
                  VerificationContext.height + 1)
      BY <1>1, IndexedAdvanceReadyReachesSuccessorHeight
    <2>2. IndexedContextAdvanceReady(VerificationContext)
             => VerificationContext.height < MaxHeight
      BY <1>1, JoinedCanonicalDescendantStaysWithinHorizon
         DEF IndexedContextAdvanceReady
    <2>3. VerificationContext.height < MaxHeight
             /\ IndexedResponsiveHeightReached(
                  VerificationContext.height + 1)
             => IndexedContextCompleted(VerificationContext)
      BY <1>1, VerificationSuccessorHeightImpliesContextCompleted
    <2> QED BY <2>1, <2>2, <2>3, PTL
  <1> QED BY <1>1

THEOREM VerificationReachedEscapeEventuallyCompletes ==
  /\ IndexedChainSpec
    /\ IndexedExactHistoricalRecoveryProgress
    /\ IndexedSuccessorActivationProgress
    /\ VerificationContext \in AdmissibleContextRecords
    => (/\ IndexedTargetJoined(VerificationContext)
        /\ IndexedResponsiveHeightReached(VerificationContext.height)
        /\ VerificationFrontierEscape)
         ~> IndexedContextCompleted(VerificationContext)
PROOF
  <1>1. ASSUME IndexedChainSpec,
              IndexedExactHistoricalRecoveryProgress,
              IndexedSuccessorActivationProgress,
              VerificationContext \in AdmissibleContextRecords
         PROVE (/\ IndexedTargetJoined(VerificationContext)
                /\ IndexedResponsiveHeightReached(
                     VerificationContext.height)
                /\ VerificationFrontierEscape)
                 ~> IndexedContextCompleted(VerificationContext)
    <2>1. (/\ IndexedTargetJoined(VerificationContext)
            /\ IndexedResponsiveHeightReached(
                 VerificationContext.height)
            /\ JoinedCanonicalDescendant(VerificationContext))
             => IndexedContextAdvanceReady(VerificationContext)
      BY <1>1 DEF IndexedContextAdvanceReady, IndexedTargetJoined
    <2>2. IndexedContextAdvanceReady(VerificationContext)
             ~> IndexedContextCompleted(VerificationContext)
      BY <1>1, VerificationAdvanceReadyEventuallyCompletes
    <2>3. (/\ IndexedTargetJoined(VerificationContext)
            /\ IndexedResponsiveHeightReached(
                 VerificationContext.height)
            /\ IndexedAsync(VerificationContext)!
                 AsyncAllResponsiveAppliedAt(VerificationContext))
             ~> IndexedContextCompleted(VerificationContext)
      BY <1>1, VerificationAppliedFrontierEventuallyCompletes
    <2> QED BY <2>1, <2>2, <2>3, PTL
         DEF VerificationFrontierEscape
  <1> QED BY <1>1

THEOREM VerificationJoinedTargetEventuallyReachesAndEscapes ==
  /\ IndexedLiveChainSpec
    /\ IndexedExactHistoricalRecoveryProgress
    /\ IndexedSuccessorActivationProgress
    /\ VerificationOneHeightCompletion
    /\ VerificationContext \in AdmissibleContextRecords
    /\ (IndexedAsync(VerificationContext)!
          AsyncLiveSpecAt(VerificationContext)
          => <>IndexedCore(VerificationContext, 7))
    => IndexedTargetJoined(VerificationContext)
         ~> (/\ IndexedTargetJoined(VerificationContext)
             /\ IndexedResponsiveHeightReached(
                  VerificationContext.height)
             /\ VerificationFrontierEscape)
PROOF
  <1>1. ASSUME IndexedLiveChainSpec,
              IndexedExactHistoricalRecoveryProgress,
              IndexedSuccessorActivationProgress,
              VerificationOneHeightCompletion,
              VerificationContext \in AdmissibleContextRecords,
              (IndexedAsync(VerificationContext)!
                 AsyncLiveSpecAt(VerificationContext)
                 => <>IndexedCore(VerificationContext, 7))
         PROVE IndexedTargetJoined(VerificationContext)
                 ~> (/\ IndexedTargetJoined(VerificationContext)
                     /\ IndexedResponsiveHeightReached(
                          VerificationContext.height)
                     /\ VerificationFrontierEscape)
    <2>0. IndexedChainSpec
      BY <1>1, IndexedLiveChainSpecProjectsIndexedChainSpec
    <2>1. []IndexedCompositionInvariant
      BY <2>0, IndexedChainSpecEstablishesCompositionInvariant, PTL
    <2>2. []IndexedJoinedThroughLocalHeight
      BY <2>0, IndexedChainSpecJoinsEveryNodeThroughLocalHeight, PTL
    <2>3. IndexedTargetJoined(VerificationContext)
             ~> IndexedResponsiveHeightReached(
                  VerificationContext.height)
      BY <1>1, <2>0,
         IndexedJoinedTargetEventuallyReachesEveryAncestorHeight,
         Isa DEF AdmissibleContextRecords, FrozenContextAdmissible,
                 ContextRecords, Heights
    <2>4. IndexedTargetJoined(VerificationContext)
             /\ [IndexedChainNext]_IndexedChainVars
             => IndexedTargetJoined(VerificationContext)'
      BY <1>1, IndexedTargetJoinedIsStable
    <2>5. IndexedResponsiveHeightReached(VerificationContext.height)
             /\ [IndexedChainNext]_IndexedChainVars
             => IndexedResponsiveHeightReached(
                  VerificationContext.height)'
      BY <1>1, IndexedResponsiveHeightReachedIsStable,
         Isa DEF AdmissibleContextRecords, FrozenContextAdmissible,
                 ContextRecords, Heights
    <2>6. (IndexedTargetJoined(VerificationContext)
             /\ IndexedResponsiveHeightReached(
                  VerificationContext.height))
             ~> IndexedAllResponsiveJoined(VerificationContext)
      BY <1>1,
         <2>0,
         IndexedReachedAncestorEventuallyJoinsEveryResponsiveNode,
         IndexedJoinedTargetIdentifiesEveryCanonicalAncestor,
         PTL DEF IndexedAncestorContext
    <2>7. IndexedAllResponsiveJoined(VerificationContext)
             ~> VerificationFrontierEscape
      BY <1>1, VerificationActivatedFrontierEventuallyEscapes
    <2>8. IndexedCompositionInvariant
             /\ VerificationFrontierEscape
             /\ [IndexedChainNext]_IndexedChainVars
             => VerificationFrontierEscape'
      BY <1>1, VerificationFrontierEscapeIsStable
    <2> QED BY <2>1, <2>2, <2>3, <2>4,
                 <2>5, <2>6, <2>7, <2>8, PTL
  <1> QED BY <1>1

IndexedHeightLivenessProperty ==
  (/\ VerificationContext \in AdmissibleContextRecords
   /\ VerificationContext \in JoinedContexts
   /\ IndexedCore(VerificationContext, 7))
    ~> IndexedContextCompleted(VerificationContext)

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
only as an internal projection and is not a production claim. The ledger keeps this
obligation unproved until that trace mapping is machine checked. The model-
internal activated-state and finite-horizon projections are proved above;
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
