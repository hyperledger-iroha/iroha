---- MODULE SumeragiV2ChainEpochRefinement ----
EXTENDS SumeragiV2AsyncLivenessProofs, TLAPS

(***************************************************************************
`VerificationContext` is an arbitrary module constant.  Proving the final
height property for this constant is therefore the ordinary TLA+ universal
closure over every admissible assignment, while keeping the asynchronous
proof INSTANCE nonparameterized for TLAPS.
***************************************************************************)
CONSTANT VerificationContext

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
  historicalCatchUpDecisions,
  historicalCatchUpApplications,
  historicalCatchUpStage,
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
  /\ AsyncChainSpec
  /\ OneHeightCompletionLiveness(ContextRecord(0, <<>>))
  => GenesisHeightSuccessorHandoffProperty
PROOF
  <1>1. ASSUME AsyncChainSpec,
              OneHeightCompletionLiveness(ContextRecord(0, <<>>))
         PROVE GenesisHeightSuccessorHandoffProperty
    <2>1. AsyncSpec
      BY <1>1, AsyncChainSpecProjectsAsyncSpec
    <2>2. AsyncSpecAt(ContextRecord(0, <<>>))
      BY <2>1
         DEF AsyncSpec, AsyncSpecAt, AsyncInit, AsyncFairness
    <2>3. gst ~> AsyncAllResponsiveAppliedAt(
                  ContextRecord(0, <<>>))
      BY <1>1, <2>2 DEF OneHeightCompletionLiveness
    <2>4. []GenesisApplicationHandoffInvariant
      BY <1>1, AsyncChainAlwaysGenesisApplicationHandoff
    <2>5. CurrentEpoch = ContextRecord(0, <<>>).epoch
      BY <1>1
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
debts.  This release-facing wrapper deliberately names the remaining premise:
OneHeightCompletionObligation currently depends on the proofless rotating-
leader and application-liveness declarations in AsyncLivenessProofs.
***************************************************************************)
THEOREM GenesisHeightSuccessorHandoffObligation ==
  AsyncChainSpec => GenesisHeightSuccessorHandoffProperty
PROOF
  <1>1. OneHeightCompletionLiveness(ContextRecord(0, <<>>))
    BY OneHeightCompletionObligation
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

The nested tuple layout is exactly
<<vars, AsyncSchedulerVars, AsyncRecoveryVars>>: 46 Core components followed
by 33 scheduler/transport components and four responsive-node recovery
components. Shape predicates exclude unmodelled fields and make every
instance projection extensional.
***************************************************************************)
IndexedCore(initialContext, component) ==
  indexedAsyncState[initialContext][1][component]

IndexedScheduler(initialContext, component) ==
  indexedAsyncState[initialContext][2][component]

IndexedRecovery(initialContext, component) ==
  indexedAsyncState[initialContext][3][component]

IndexedAsyncStateAt(initialContext) ==
  indexedAsyncState[initialContext]

CatchUpNodeHasApplicationProjection(applicationEvidence,
                                    applicationContext, node) ==
  \E application \in applicationEvidence:
    /\ application.node = node
    /\ application.qc.context = applicationContext
    /\ application.qc.phase = "Commit"

THEOREM CatchUpVotersProjectionMatchesAsyncVocabulary ==
  \A initialContext:
    AsyncVotersAt(initialContext)
      = Responsive \cap VotingRoster(initialContext.epoch)
BY DEF AsyncVotersAt

THEOREM CatchUpApplicationProjectionMatchesAsyncVocabulary ==
  \A node:
    NodeHasApplication(node)
      <=> CatchUpNodeHasApplicationProjection(applied, context, node)
BY DEF NodeHasApplication, CatchUpNodeHasApplicationProjection

IndexedProjectedNodeHasApplication(initialContext, node) ==
  CatchUpNodeHasApplicationProjection(
    IndexedCore(initialContext, 46),
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
       lockRank <- IndexedCore(initialContext, 26),
       lockSubject <- IndexedCore(initialContext, 27),
       highestRank <- IndexedCore(initialContext, 28),
       highestSubject <- IndexedCore(initialContext, 29),
       pendingProposal <- IndexedCore(initialContext, 30),
       pendingPrepare <- IndexedCore(initialContext, 31),
       pendingObservePrepare <- IndexedCore(initialContext, 32),
       pendingLockCommit <- IndexedCore(initialContext, 33),
       pendingTimeout <- IndexedCore(initialContext, 34),
       pendingInstallTC <- IndexedCore(initialContext, 35),
       pendingDecision <- IndexedCore(initialContext, 36),
       signProposals <- IndexedCore(initialContext, 37),
       signVotes <- IndexedCore(initialContext, 38),
       signTimeouts <- IndexedCore(initialContext, 39),
       proposalNetwork <- IndexedCore(initialContext, 40),
       voteNetwork <- IndexedCore(initialContext, 41),
       qcNetwork <- IndexedCore(initialContext, 42),
       timeoutNetwork <- IndexedCore(initialContext, 43),
       tcNetwork <- IndexedCore(initialContext, 44),
       decisions <- IndexedCore(initialContext, 45),
       applied <- IndexedCore(initialContext, 46),
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
       asyncOutstandingWork <- IndexedScheduler(initialContext, 11),
       asyncIoReadyCompletions <- IndexedScheduler(initialContext, 12),
       asyncLocalReadyCompletions <- IndexedScheduler(initialContext, 13),
       asyncNextCompletionSource <- IndexedScheduler(initialContext, 14),
       asyncIoControlAvailable <- IndexedScheduler(initialContext, 15),
       asyncDeferredCompletionQueues <- IndexedScheduler(initialContext, 16),
       asyncDeferredProgressQueues <- IndexedScheduler(initialContext, 17),
       asyncDeferredNormalQueues <- IndexedScheduler(initialContext, 18),
       asyncNextDeferredClass <- IndexedScheduler(initialContext, 19),
       asyncDeferredDrainOwed <- IndexedScheduler(initialContext, 20),
       asyncCausalQueues <- IndexedScheduler(initialContext, 21),
       asyncOutstandingTags <- IndexedScheduler(initialContext, 22),
       asyncNodeDeadlines <- IndexedScheduler(initialContext, 23),
       asyncRetransmitDeadlines <- IndexedScheduler(initialContext, 24),
       asyncNodeServiceDeadlines <- IndexedScheduler(initialContext, 25),
       asyncIoServiceDeadlines <- IndexedScheduler(initialContext, 26),
       asyncSentItems <- IndexedScheduler(initialContext, 27),
       asyncRetainedControl <- IndexedScheduler(initialContext, 28),
       asyncActiveRequests <- IndexedScheduler(initialContext, 29),
       asyncTransport <- IndexedScheduler(initialContext, 30),
       asyncIngressLanes <- IndexedScheduler(initialContext, 31),
       asyncIngressReady <- IndexedScheduler(initialContext, 32),
       asyncHeldChunks <- IndexedScheduler(initialContext, 33),
       asyncRecoveryPhase <- IndexedRecovery(initialContext, 1),
       asyncRecoveryNode <- IndexedRecovery(initialContext, 2),
       asyncRecoveryGeneration <- IndexedRecovery(initialContext, 3),
       asyncRecoveryReplayQueue <- IndexedRecovery(initialContext, 4)

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

VerificationAsyncProof ==
  INSTANCE SumeragiV2AsyncLivenessProofs
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
       lockRank <- VerificationCore(26),
       lockSubject <- VerificationCore(27),
       highestRank <- VerificationCore(28),
       highestSubject <- VerificationCore(29),
       pendingProposal <- VerificationCore(30),
       pendingPrepare <- VerificationCore(31),
       pendingObservePrepare <- VerificationCore(32),
       pendingLockCommit <- VerificationCore(33),
       pendingTimeout <- VerificationCore(34),
       pendingInstallTC <- VerificationCore(35),
       pendingDecision <- VerificationCore(36),
       signProposals <- VerificationCore(37),
       signVotes <- VerificationCore(38),
       signTimeouts <- VerificationCore(39),
       proposalNetwork <- VerificationCore(40),
       voteNetwork <- VerificationCore(41),
       qcNetwork <- VerificationCore(42),
       timeoutNetwork <- VerificationCore(43),
       tcNetwork <- VerificationCore(44),
       decisions <- VerificationCore(45),
       applied <- VerificationCore(46),
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
       asyncOutstandingWork <- VerificationScheduler(11),
       asyncIoReadyCompletions <- VerificationScheduler(12),
       asyncLocalReadyCompletions <- VerificationScheduler(13),
       asyncNextCompletionSource <- VerificationScheduler(14),
       asyncIoControlAvailable <- VerificationScheduler(15),
       asyncDeferredCompletionQueues <- VerificationScheduler(16),
       asyncDeferredProgressQueues <- VerificationScheduler(17),
       asyncDeferredNormalQueues <- VerificationScheduler(18),
       asyncNextDeferredClass <- VerificationScheduler(19),
       asyncDeferredDrainOwed <- VerificationScheduler(20),
       asyncCausalQueues <- VerificationScheduler(21),
       asyncOutstandingTags <- VerificationScheduler(22),
       asyncNodeDeadlines <- VerificationScheduler(23),
       asyncRetransmitDeadlines <- VerificationScheduler(24),
       asyncNodeServiceDeadlines <- VerificationScheduler(25),
       asyncIoServiceDeadlines <- VerificationScheduler(26),
       asyncSentItems <- VerificationScheduler(27),
       asyncRetainedControl <- VerificationScheduler(28),
       asyncActiveRequests <- VerificationScheduler(29),
       asyncTransport <- VerificationScheduler(30),
       asyncIngressLanes <- VerificationScheduler(31),
       asyncIngressReady <- VerificationScheduler(32),
       asyncHeldChunks <- VerificationScheduler(33),
       asyncRecoveryPhase <- VerificationRecovery(1),
       asyncRecoveryNode <- VerificationRecovery(2),
       asyncRecoveryGeneration <- VerificationRecovery(3),
       asyncRecoveryReplayQueue <- VerificationRecovery(4)

AdmissibleContextRecords ==
  {initialContext \in ContextRecords:
     FrozenContextAdmissible(initialContext)}

IndexedAsyncStateShape ==
  /\ DOMAIN indexedAsyncState = AdmissibleContextRecords
  /\ \A initialContext \in AdmissibleContextRecords:
       /\ Len(indexedAsyncState[initialContext]) = 3
       /\ DOMAIN indexedAsyncState[initialContext] = 1..3
       /\ Len(indexedAsyncState[initialContext][1]) = 46
       /\ DOMAIN indexedAsyncState[initialContext][1] = 1..46
       /\ Len(indexedAsyncState[initialContext][2]) = 33
       /\ DOMAIN indexedAsyncState[initialContext][2] = 1..33
       /\ Len(indexedAsyncState[initialContext][3]) = 4
       /\ DOMAIN indexedAsyncState[initialContext][3] = 1..4

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

IndexedDecisions(initialContext) == IndexedCore(initialContext, 45)
IndexedApplications(initialContext) == IndexedCore(initialContext, 46)

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
  (UNION {IndexedCurrentDecisions(initialContext):
            initialContext \in AdmissibleContextRecords})
    \cup historicalCatchUpDecisions

IndexedApplicationEvidence ==
  (UNION {IndexedCurrentApplications(initialContext):
            initialContext \in AdmissibleContextRecords})
    \cup historicalCatchUpApplications

HistoricalCatchUpShape ==
  /\ historicalCatchUpDecisions \subseteq Chain!DecisionEvidenceSet
  /\ historicalCatchUpApplications \subseteq historicalCatchUpDecisions
  /\ historicalCatchUpStage
       \in [AdmissibleContextRecords ->
             [ValidatorIds ->
               {"Idle", "DecisionRecovered", "BodyRecovered",
                "Stored", "Validated", "Applied"}]]

HistoricalCatchUpVars ==
  <<historicalCatchUpDecisions, historicalCatchUpApplications,
    historicalCatchUpStage>>

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

SuccessorRecoveryAuthoritySet ==
  [parentContext: AdmissibleContextRecords,
   node: ValidatorIds,
   successorContext: AdmissibleContextRecords,
   application: Chain!DecisionEvidenceSet]

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
  [parentContext |-> parentContext,
   node |-> node,
   successorContext |-> successorContext,
   application |-> application]

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

IndexedChainVars ==
  <<indexedAsyncState, joinedByContext,
    HistoricalCatchUpVars, SuccessorActivationVars,
    Chain!ChainEpochVars>>

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
  \/ \E node \in IndexedAsync(initialContext)!AsyncCurrentResponsiveVoters:
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
     \/ \E node \in IndexedAsync(initialContext)!AsyncCurrentResponsiveVoters:
          /\ node \in joinedByContext[initialContext]
          /\ IndexedAsync(initialContext)!ServiceIoWorker(node)
     \/ \E node \in IndexedAsync(initialContext)!AsyncCurrentResponsiveVoters:
          /\ node \in joinedByContext[initialContext]
          /\ IndexedAsync(initialContext)!EnqueueIoLocalControl(node)
     \/ IndexedAsync(initialContext)!AsyncNetworkStep
     \/ IndexedAsync(initialContext)!AsyncFaultStep
  /\ UNCHANGED IndexedScheduler(initialContext, 25)

IndexedJoinedNonCrashStep(initialContext) ==
  /\ (IndexedJoinedRunnerStep(initialContext)
        \/ IndexedJoinedNonRunnerStep(initialContext))
  /\ UNCHANGED IndexedCore(initialContext, 6)

IndexedJoinedAsyncNext(initialContext) ==
  /\ (IndexedJoinedNonCrashStep(initialContext)
        \/ \E node \in ValidatorIds:
             IndexedAsync(initialContext)!PreGstCrash(node))
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
  /\ \/ /\ IndexedNodeCurrentAt(initialContext, application.node)
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
application evidence.  Applied startup may publish that predecessor entry;
recovery after a fail-closed reset must authenticate the exact durable tip
while the predecessor entry is absent.  Both paths share the ordered startup
pipeline, but only the Applied publication writes physical `Complete`.
***************************************************************************)
SuccessorActivationEnvironmentStutter ==
  /\ UNCHANGED indexedAsyncState
  /\ UNCHANGED HistoricalCatchUpVars
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
  /\ successorContext =
       CanonicalIndexedContext(parentContext.height + 1)
  /\ successorActivationStatus[parentContext][node] = "Queued"
  /\ successorPredecessorStatusOwnership[parentContext][node] = "Published"
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

FailClosedSuccessorStartup(parentContext, node) ==
  LET owner == SuccessorActivationOwner(parentContext, node)
  IN /\ successorActivationStatus[parentContext][node]
           \in {"Queued", "Running"}
     /\ successorPredecessorStatusOwnership[parentContext][node]
          = "Published"
     /\ owner \notin successorActivationFailureHistory
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
     /\ UNCHANGED <<publishedSuccessorActivationMarkers,
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
     /\ owner \in successorActivationFailures
     /\ owner \in successorActivationFailureHistory
     /\ ExactDurableParentApplication(parentContext, node, application)
     /\ token \notin successorActivationTokens
     /\ authority \notin successorRecoveryAuthorities
     /\ successorActivationStatus' =
          [successorActivationStatus EXCEPT
             ![parentContext][node] = "Running"]
     /\ successorActivationTokens' =
          successorActivationTokens \cup {token}
     /\ successorRecoveryAuthorities' =
          successorRecoveryAuthorities \cup {authority}
     /\ successorActivationFailures' =
          successorActivationFailures \ {owner}
     /\ UNCHANGED <<successorPredecessorStatusOwnership,
                     successorActivationPrerequisites,
                     preparedSuccessorActivationMarkers,
                     publishedSuccessorActivationMarkers,
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
     /\ SuccessorActivationEnvironmentStutter

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
     /\ SuccessorActivationEnvironmentStutter

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
       \/ FailClosedSuccessorStartup(parentContext, node)
       \/ \E application \in Chain!DecisionEvidenceSet:
            AuthenticateRecoveredSuccessorActivation(
              parentContext, node, successorContext, application)
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

TerminalSuccessorActivationExcluded ==
  \A terminalContext \in AdmissibleContextRecords,
     node \in ValidatorIds:
    terminalContext.height = MaxHeight
      => /\ successorActivationStatus[terminalContext][node] = "Idle"
         /\ successorPredecessorStatusOwnership[terminalContext][node]
              = "Absent"

(***************************************************************************
After the single permitted fail-closed transition has been authenticated and
the full startup pipeline has reached its publication boundary, failure can
no longer pre-empt publication.  A later-height catch-up may legitimately
supersede this old activation, so the unconditional suffix records that case;
the stable-height corollary below yields exact publication.
***************************************************************************)
PostFailureSuccessorPublicationReady(parentContext, node) ==
  LET successorContext ==
        CanonicalIndexedContext(parentContext.height + 1)
      marker == SuccessorActivationMarker(
                  parentContext, node, successorContext)
  IN /\ parentContext \in AdmissibleContextRecords
     /\ parentContext.height < MaxHeight
     /\ successorContext \in AdmissibleContextRecords
     /\ SuccessorActivationOwner(parentContext, node)
          \in successorActivationFailureHistory
     /\ SuccessorActivationCredentialReady(
          parentContext, node, successorContext)
     /\ successorActivationPrerequisites[parentContext][node]
          = SuccessorActivationRequiredPrerequisites
     /\ marker \in preparedSuccessorActivationMarkers

SuccessorPublicationOrSuperseded(parentContext, node) ==
  \/ SuccessorHeightActivated(parentContext, node)
  \/ nodeHeight[node] > parentContext.height + 1

(***************************************************************************
Authenticated historical catch-up.

Production `V2BlockSyncDiscovery` signs a request for the lagging node's exact
frozen context. `V2BlockSyncServer` may answer only from canonical Kura history,
and the historical body server must be one of the CommitQC's certified signers.
The response then enters the ordinary reducer through the exact staged path
Idle -> DecisionRecovered -> BodyRecovered -> Stored -> Validated -> Applied.
For a nonterminal parent, application queues the ordinary successor activation
pipeline but does not join it. At the finite terminal horizon, a responsive
observer or validator records known application without advancing nodeHeight,
creating a successor activation, or joining a fictitious context.

Requester authentication is identity/context based; only the historical body
responder must be a certified old-roster signer. Exact context, round, subject,
QC, and signer-backed body identity are copied from already durable canonical
decision and application evidence; no synthetic finality artifact is created.
***************************************************************************)
HistoricalCatchUpRecord(node, source) ==
  [node |-> node, qc |-> source.qc]

(***************************************************************************
These are the exact projection-native expansions of `AsyncVotersAt` and
`NodeHasApplication` from SumeragiV2AsyncNetwork: the IndexedAsync WITH-clause
maps `applied` to IndexedCore(initialContext, 46) (IndexedApplications) and
`context` to IndexedCore(initialContext, 2).  Spelling the definitions here
keeps the parameterized proof INSTANCE out of later ENABLED obligations; no
roster, context, phase, or per-node application condition is weakened.
***************************************************************************)

HistoricalCatchUpTarget(initialContext, node) ==
  /\ node \in Responsive
  /\ nodeHeight[node] = initialContext.height
  /\ nodeContext[node] = initialContext
  /\ ~IndexedProjectedNodeHasApplication(initialContext, node)

HistoricalCatchUpSource(initialContext, server, source) ==
  /\ initialContext \in JoinedContexts
  /\ source \in IndexedCurrentDecisions(initialContext)
  /\ source \in IndexedCurrentApplications(initialContext)
  /\ source \in durableDecisionEvidence
  /\ source \in durableApplicationEvidence
  /\ \/ /\ initialContext.height < MaxHeight
        /\ Chain!CanonicalCommitForSlot(
             source.qc, initialContext.height + 1)
     \/ /\ initialContext.height = MaxHeight
        /\ Chain!ReceiptOutsideChainHorizon(source)
  /\ server \in source.qc.signers \cap Honest
  /\ server \in joinedByContext[initialContext]
  /\ BodyHeldBy(IndexedCore(initialContext, 9), server,
                 initialContext, source.qc.view, source.qc.subject)

HistoricalCatchUpDecisionAt(initialContext, node) ==
  \E decision \in historicalCatchUpDecisions:
    /\ decision.node = node
    /\ decision.qc.context = initialContext
    /\ decision.qc.height = initialContext.height

HistoricalCatchUpApplicationAt(initialContext, node) ==
  \E application \in historicalCatchUpApplications:
    /\ application.node = node
    /\ application.qc.context = initialContext
    /\ application.qc.height = initialContext.height

IndexedHistoricalCatchUpDecision(initialContext, node, server, source) ==
  LET decision == HistoricalCatchUpRecord(node, source)
  IN /\ HistoricalCatchUpTarget(initialContext, node)
     /\ HistoricalCatchUpSource(initialContext, server, source)
     /\ historicalCatchUpStage[initialContext][node] = "Idle"
     /\ decision \notin IndexedDecisionEvidence
     /\ Chain!RecordKnownDecision(decision)
     /\ historicalCatchUpDecisions' =
          historicalCatchUpDecisions \cup {decision}
     /\ historicalCatchUpStage' =
          [historicalCatchUpStage EXCEPT
             ![initialContext][node] = "DecisionRecovered"]
     /\ UNCHANGED <<historicalCatchUpApplications,
                     indexedAsyncState, joinedByContext,
                     SuccessorActivationVars>>
     /\ IndexedAsyncStateShape'
     /\ JoinedByContextShape'
     /\ HistoricalCatchUpShape'
     /\ SuccessorActivationShape'

IndexedHistoricalCatchUpBodyRecovery(initialContext, node, server, source) ==
  LET decision == HistoricalCatchUpRecord(node, source)
  IN /\ HistoricalCatchUpTarget(initialContext, node)
     /\ HistoricalCatchUpSource(initialContext, server, source)
     /\ historicalCatchUpStage[initialContext][node]
          = "DecisionRecovered"
     /\ decision \in historicalCatchUpDecisions
     /\ historicalCatchUpStage' =
          [historicalCatchUpStage EXCEPT
             ![initialContext][node] = "BodyRecovered"]
     /\ UNCHANGED <<historicalCatchUpDecisions,
                     historicalCatchUpApplications,
                     indexedAsyncState, joinedByContext,
                     SuccessorActivationVars, Chain!ChainEpochVars>>
     /\ IndexedAsyncStateShape'
     /\ JoinedByContextShape'
     /\ HistoricalCatchUpShape'
     /\ SuccessorActivationShape'

IndexedHistoricalCatchUpBodyStore(initialContext, node, server, source) ==
  LET decision == HistoricalCatchUpRecord(node, source)
  IN /\ HistoricalCatchUpTarget(initialContext, node)
     /\ HistoricalCatchUpSource(initialContext, server, source)
     /\ historicalCatchUpStage[initialContext][node] = "BodyRecovered"
     /\ decision \in historicalCatchUpDecisions
     /\ historicalCatchUpStage' =
          [historicalCatchUpStage EXCEPT
             ![initialContext][node] = "Stored"]
     /\ UNCHANGED <<historicalCatchUpDecisions,
                     historicalCatchUpApplications,
                     indexedAsyncState, joinedByContext,
                     SuccessorActivationVars, Chain!ChainEpochVars>>
     /\ IndexedAsyncStateShape'
     /\ JoinedByContextShape'
     /\ HistoricalCatchUpShape'
     /\ SuccessorActivationShape'

IndexedHistoricalCatchUpValidation(initialContext, node, server, source) ==
  LET decision == HistoricalCatchUpRecord(node, source)
  IN /\ HistoricalCatchUpTarget(initialContext, node)
     /\ HistoricalCatchUpSource(initialContext, server, source)
     /\ historicalCatchUpStage[initialContext][node] = "Stored"
     /\ decision \in historicalCatchUpDecisions
     /\ historicalCatchUpStage' =
          [historicalCatchUpStage EXCEPT
             ![initialContext][node] = "Validated"]
     /\ UNCHANGED <<historicalCatchUpDecisions,
                     historicalCatchUpApplications,
                     indexedAsyncState, joinedByContext,
                     SuccessorActivationVars, Chain!ChainEpochVars>>
     /\ IndexedAsyncStateShape'
     /\ JoinedByContextShape'
     /\ HistoricalCatchUpShape'
     /\ SuccessorActivationShape'

IndexedHistoricalCatchUpNonterminalApplication(
    initialContext, node, server, source) ==
  LET application == HistoricalCatchUpRecord(node, source)
  IN /\ HistoricalCatchUpTarget(initialContext, node)
     /\ HistoricalCatchUpSource(initialContext, server, source)
     /\ initialContext.height < MaxHeight
     /\ historicalCatchUpStage[initialContext][node] = "Validated"
     /\ application \in historicalCatchUpDecisions
     /\ application \notin historicalCatchUpApplications
     /\ Chain!RecordAppliedNext(application)
     /\ historicalCatchUpApplications' =
          historicalCatchUpApplications \cup {application}
     /\ historicalCatchUpStage' =
          [historicalCatchUpStage EXCEPT
             ![initialContext][node] = "Applied"]
     /\ QueueSuccessorActivation(initialContext, node)
     /\ UNCHANGED <<historicalCatchUpDecisions,
                     indexedAsyncState, joinedByContext>>
     /\ IndexedAsyncStateShape'
     /\ JoinedByContextShape'
     /\ HistoricalCatchUpShape'
     /\ SuccessorActivationShape'

IndexedHistoricalCatchUpTerminalApplication(
    initialContext, node, server, source) ==
  LET application == HistoricalCatchUpRecord(node, source)
  IN /\ HistoricalCatchUpTarget(initialContext, node)
     /\ HistoricalCatchUpSource(initialContext, server, source)
     /\ initialContext.height = MaxHeight
     /\ Chain!ReceiptOutsideChainHorizon(application)
     /\ historicalCatchUpStage[initialContext][node] = "Validated"
     /\ application \in historicalCatchUpDecisions
     /\ application \notin historicalCatchUpApplications
     /\ Chain!RecordKnownApplication(application)
     /\ historicalCatchUpApplications' =
          historicalCatchUpApplications \cup {application}
     /\ historicalCatchUpStage' =
          [historicalCatchUpStage EXCEPT
             ![initialContext][node] = "Applied"]
     /\ UNCHANGED <<historicalCatchUpDecisions,
                     indexedAsyncState, joinedByContext,
                     SuccessorActivationVars>>
     /\ IndexedAsyncStateShape'
     /\ JoinedByContextShape'
     /\ HistoricalCatchUpShape'
     /\ SuccessorActivationShape'

IndexedHistoricalCatchUpApplication(initialContext, node, server, source) ==
  \/ IndexedHistoricalCatchUpNonterminalApplication(
       initialContext, node, server, source)
  \/ IndexedHistoricalCatchUpTerminalApplication(
       initialContext, node, server, source)

IndexedHistoricalCatchUpPipelineAction(
    initialContext, node, server, source) ==
  \/ IndexedHistoricalCatchUpDecision(
       initialContext, node, server, source)
  \/ IndexedHistoricalCatchUpBodyRecovery(
       initialContext, node, server, source)
  \/ IndexedHistoricalCatchUpBodyStore(
       initialContext, node, server, source)
  \/ IndexedHistoricalCatchUpValidation(
       initialContext, node, server, source)
  \/ IndexedHistoricalCatchUpApplication(
       initialContext, node, server, source)

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
  /\ UNCHANGED HistoricalCatchUpVars
  /\ IndexedAsyncStateShape'
  /\ JoinedByContextShape'
  /\ HistoricalCatchUpShape'
  /\ SuccessorActivationShape'
  /\ IndexedReceiptClassification(initialContext)

IndexedChainNext ==
  /\ IndexedAsyncStateShape
  /\ JoinedByContextShape
  /\ HistoricalCatchUpShape
  /\ SuccessorActivationShape
  /\ \/ \E initialContext \in JoinedContexts:
          IndexedProductActionAt(initialContext)
     \/ \E initialContext \in AdmissibleContextRecords,
           node \in ValidatorIds, server \in ValidatorIds,
           source \in Chain!DecisionEvidenceSet:
          IndexedHistoricalCatchUpPipelineAction(
            initialContext, node, server, source)
     \/ \E parentContext \in AdmissibleContextRecords,
           node \in ValidatorIds:
          IndexedSuccessorActivationProgressStep(parentContext, node)

IndexedChainInit ==
  /\ IndexedAsyncStateShape
  /\ JoinedByContextShape
  /\ HistoricalCatchUpShape
  /\ SuccessorActivationShape
  /\ \A initialContext \in AdmissibleContextRecords:
       IndexedAsync(initialContext)!AsyncInitAt(initialContext)
  /\ Chain!ChainEpochInit
  /\ joinedByContext =
       [initialContext \in AdmissibleContextRecords |->
          IF initialContext = GenesisContext
          THEN ValidatorIds
          ELSE {}]
  /\ historicalCatchUpDecisions = {}
  /\ historicalCatchUpApplications = {}
  /\ historicalCatchUpStage =
       [initialContext \in AdmissibleContextRecords |->
          [node \in ValidatorIds |-> "Idle"]]
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
context; no action tests whether every peer has joined.
***************************************************************************)
IndexedSetGstStep(initialContext) ==
  /\ IndexedChainNext
  /\ IndexedAsync(initialContext)!AsyncSetGST

IndexedTickStep(initialContext) ==
  /\ IndexedChainNext
  /\ IndexedAsync(initialContext)!AsyncTick

IndexedRunNodeStep(initialContext, node) ==
  /\ IndexedChainNext
  /\ IndexedNodeCurrentAt(initialContext, node)
  /\ IndexedAsync(initialContext)!PostGstRunNode(node)

IndexedCommitCertificateDiscoveryStep(initialContext, node) ==
  /\ IndexedChainNext
  /\ IndexedNodeCurrentAt(initialContext, node)
  /\ IndexedAsync(initialContext)!
       PostGstCommitCertificateDiscovery(node)

IndexedHistoricalServerStep(initialContext, node) ==
  /\ IndexedChainNext
  /\ node \in joinedByContext[initialContext]
  /\ IndexedAsync(initialContext)!PostGstRunHistoricalServer(node)

IndexedIoWorkerStep(initialContext, node) ==
  /\ IndexedChainNext
  /\ node \in joinedByContext[initialContext]
  /\ IndexedAsync(initialContext)!PostGstServiceIoWorker(node)

IndexedAdmitPacketStep(initialContext, recipient, source) ==
  /\ IndexedChainNext
  /\ IndexedAsync(initialContext)!
       PostGstAdmitHiddenPacket(recipient, source)

(***************************************************************************
These weak-fairness clauses are the explicit local-progress premise. Once
exact canonical source evidence and an honest signer-held body persist, the
entire decision/body/store/validation/application pipeline cannot be postponed
forever. Likewise, an enqueued successor activation must either fail closed
once and recover, or decrease through its exact ordered startup phases to one
of the two publication actions. No fairness is assumed for unauthenticated or
noncanonical evidence.
***************************************************************************)
IndexedCatchUpPipelineStep(initialContext, node) ==
  /\ IndexedAsyncStateShape
  /\ JoinedByContextShape
  /\ HistoricalCatchUpShape
  /\ SuccessorActivationShape
  /\ \E server \in ValidatorIds,
       source \in Chain!DecisionEvidenceSet:
       IndexedHistoricalCatchUpPipelineAction(
         initialContext, node, server, source)

(***************************************************************************
Proof-facing subactions retained as projections of the fair pipeline. The
specification attaches fairness to `IndexedCatchUpPipelineStep`; these names
only let the existing decomposition lemmas state the decision and application
boundary cases while their staged replacements are discharged.
***************************************************************************)
IndexedCatchUpServiceStep(initialContext, node) ==
  /\ IndexedAsyncStateShape
  /\ JoinedByContextShape
  /\ HistoricalCatchUpShape
  /\ SuccessorActivationShape
  /\ \E server \in ValidatorIds,
       source \in Chain!DecisionEvidenceSet:
       IndexedHistoricalCatchUpDecision(
         initialContext, node, server, source)

IndexedCatchUpApplicationStep(initialContext, node) ==
  /\ IndexedAsyncStateShape
  /\ JoinedByContextShape
  /\ HistoricalCatchUpShape
  /\ SuccessorActivationShape
  /\ \E server \in ValidatorIds,
       source \in Chain!DecisionEvidenceSet:
       IndexedHistoricalCatchUpApplication(
         initialContext, node, server, source)

IndexedFairness ==
  \A initialContext \in AdmissibleContextRecords:
    /\ WF_IndexedChainVars(IndexedSetGstStep(initialContext))
    /\ WF_IndexedChainVars(IndexedTickStep(initialContext))
    /\ \A node \in IndexedAsync(initialContext)!
                   AsyncVotersAt(initialContext):
         WF_IndexedChainVars(
           IndexedRunNodeStep(initialContext, node))
    /\ \A node \in IndexedAsync(initialContext)!
                   AsyncVotersAt(initialContext):
         WF_IndexedChainVars(
           IndexedCommitCertificateDiscoveryStep(
             initialContext, node))
    /\ \A node \in IndexedAsync(initialContext)!
                   AsyncVotersAt(initialContext):
         WF_IndexedChainVars(
           IndexedHistoricalServerStep(initialContext, node))
    /\ \A node \in IndexedAsync(initialContext)!
                   AsyncVotersAt(initialContext):
         WF_IndexedChainVars(
           IndexedIoWorkerStep(initialContext, node))
    /\ \A recipient \in IndexedAsync(initialContext)!
                        AsyncVotersAt(initialContext),
          source \in IndexedAsync(initialContext)!
                     AsyncVotersAt(initialContext):
         WF_IndexedChainVars(
           IndexedAdmitPacketStep(initialContext, recipient, source))
    /\ \A node \in ValidatorIds:
         WF_IndexedChainVars(
           IndexedCatchUpPipelineStep(initialContext, node))
    /\ \A node \in ValidatorIds:
         WF_IndexedChainVars(
           IndexedSuccessorActivationProgressStep(
             initialContext, node))

IndexedChainSpec ==
  /\ IndexedChainInit
  /\ [][IndexedChainNext]_IndexedChainVars
  /\ IndexedFairness

(***************************************************************************
Terminal exclusion is an internal model invariant, independent of the still
external Rust trace-refinement seam.  The terminal application action records
known evidence while preserving node height/context and the entire activation
tuple; no terminal context can satisfy the queue guard.
***************************************************************************)
THEOREM TerminalContextCannotQueueSuccessorActivation ==
  \A terminalContext \in AdmissibleContextRecords,
     node \in ValidatorIds:
    terminalContext.height = MaxHeight
      => ~QueueSuccessorActivation(terminalContext, node)
BY DEF QueueSuccessorActivation

THEOREM TerminalHistoricalCatchUpPreservesHorizon ==
  \A terminalContext \in AdmissibleContextRecords,
     node \in ValidatorIds,
     server \in ValidatorIds,
     source \in Chain!DecisionEvidenceSet:
    IndexedHistoricalCatchUpTerminalApplication(
      terminalContext, node, server, source)
      => /\ historicalCatchUpStage'[terminalContext][node] = "Applied"
         /\ nodeHeight'[node] = terminalContext.height
         /\ nodeContext'[node] = terminalContext
         /\ UNCHANGED SuccessorActivationVars
BY Isa DEF IndexedHistoricalCatchUpTerminalApplication,
           HistoricalCatchUpTarget, HistoricalCatchUpRecord,
           Chain!RecordKnownApplication, SuccessorActivationVars

THEOREM IndexedInitEstablishesTerminalActivationExclusion ==
  IndexedChainInit => TerminalSuccessorActivationExcluded
BY Isa DEF IndexedChainInit, TerminalSuccessorActivationExcluded

THEOREM IndexedActionPreservesTerminalActivationExclusion ==
  TerminalSuccessorActivationExcluded
    /\ IndexedChainNext
    => TerminalSuccessorActivationExcluded'
BY Isa DEF TerminalSuccessorActivationExcluded,
           IndexedChainNext, IndexedProductActionAt,
           IndexedReceiptClassification,
           IndexedReceiptFreeChainStutter,
           IndexedDecisionReceiptHandoff,
           IndexedApplicationReceiptHandoff,
           IndexedHistoricalCatchUpPipelineAction,
           IndexedHistoricalCatchUpDecision,
           IndexedHistoricalCatchUpBodyRecovery,
           IndexedHistoricalCatchUpBodyStore,
           IndexedHistoricalCatchUpValidation,
           IndexedHistoricalCatchUpApplication,
           QueueSuccessorActivation,
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
           ActivateRecoveredSuccessorHeight

THEOREM IndexedStepPreservesTerminalActivationExclusion ==
  TerminalSuccessorActivationExcluded
    /\ [IndexedChainNext]_IndexedChainVars
    => TerminalSuccessorActivationExcluded'
BY Isa, IndexedActionPreservesTerminalActivationExclusion
   DEF IndexedChainVars, TerminalSuccessorActivationExcluded

THEOREM IndexedChainAlwaysExcludesTerminalActivation ==
  IndexedChainSpec => []TerminalSuccessorActivationExcluded
PROOF
  <1>1. IndexedChainInit => TerminalSuccessorActivationExcluded
    BY IndexedInitEstablishesTerminalActivationExclusion
  <1>2. TerminalSuccessorActivationExcluded
           /\ [IndexedChainNext]_IndexedChainVars
           => TerminalSuccessorActivationExcluded'
    BY IndexedStepPreservesTerminalActivationExclusion
  <1> QED BY <1>1, <1>2, PTL DEF IndexedChainSpec

THEOREM PostFailurePublicationReadyEnablesProgress ==
  \A parentContext \in AdmissibleContextRecords,
     node \in ValidatorIds:
    SuccessorActivationShape
      /\ PostFailureSuccessorPublicationReady(parentContext, node)
      => ENABLED
           <<IndexedSuccessorActivationProgressStep(
               parentContext, node)>>_(IndexedChainVars)
BY ExpandENABLED, Isa
   DEF PostFailureSuccessorPublicationReady,
       IndexedSuccessorActivationProgressStep,
       SuccessorActivationCredentialReady,
       ExactSuccessorActivationToken,
       ExactCompleteTipRecoveryAuthority,
       ActivateAppliedSuccessorHeight,
       ActivateRecoveredSuccessorHeight,
       SuccessorActivationEnvironmentStutter,
       IndexedChainVars, SuccessorActivationVars

THEOREM PostFailurePublicationProgressPublishes ==
  \A parentContext \in AdmissibleContextRecords,
     node \in ValidatorIds:
    PostFailureSuccessorPublicationReady(parentContext, node)
      /\ IndexedSuccessorActivationProgressStep(parentContext, node)
      => SuccessorHeightActivated(parentContext, node)'
BY Isa DEF PostFailureSuccessorPublicationReady,
           IndexedSuccessorActivationProgressStep,
           SuccessorActivationCredentialReady,
           ExactSuccessorActivationToken,
           ExactCompleteTipRecoveryAuthority,
           SuccessorHeightActivated,
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
           ActivateRecoveredSuccessorHeight

(***************************************************************************
Composition invariant.

Every joined context is a canonical prefix no higher than the globally
certified prefix. Application advances the durable per-node chain first, then
queues a context-exact activation. A node joins the successor only after the
Applied or Recovered publication action records the exact activation marker.
Terminal historical catch-up is retained as application evidence without a
fictitious successor or node-height advance.
***************************************************************************)
IndexedEveryInstanceStrongInvariant ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedAsync(initialContext)!StrongInductiveInvariant

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
            \/ /\ blockHeight = MaxHeight
               /\ blockHeight = nodeHeight[node]
               /\ HistoricalCatchUpApplicationAt(
                    CanonicalIndexedContext(blockHeight), node)

JoinedRoutingInvariant ==
  \A initialContext \in AdmissibleContextRecords:
    \A node \in joinedByContext[initialContext]:
      \/ IndexedNodeCurrentAt(initialContext, node)
      \/ /\ nodeHeight[node] > initialContext.height
         /\ \/ IndexedAsync(initialContext)!NodeHasApplication(node)
            \/ HistoricalCatchUpApplicationAt(initialContext, node)

IndexedApplicationsRespectNodeHeight ==
  \A initialContext \in AdmissibleContextRecords:
    \A node \in Responsive:
      IndexedAsync(initialContext)!NodeHasApplication(node)
        => \/ initialContext.height = MaxHeight
           \/ nodeHeight[node] > initialContext.height

HistoricalCatchUpReceiptSound(receipt) ==
  \E initialContext \in AdmissibleContextRecords,
     server \in ValidatorIds,
     source \in Chain!DecisionEvidenceSet:
    /\ receipt.node \in Responsive
    /\ HistoricalCatchUpSource(initialContext, server, source)
    /\ receipt = HistoricalCatchUpRecord(receipt.node, source)

HistoricalCatchUpEvidenceInvariant ==
  /\ \A decision \in historicalCatchUpDecisions:
       HistoricalCatchUpReceiptSound(decision)
  /\ \A initialContext \in AdmissibleContextRecords,
       node \in ValidatorIds:
       HistoricalCatchUpApplicationAt(initialContext, node)
         => \/ /\ initialContext.height < MaxHeight
                  /\ nodeHeight[node] > initialContext.height
             \/ /\ initialContext.height = MaxHeight
                  /\ nodeHeight[node] = initialContext.height
                  /\ nodeContext[node] = initialContext
                  /\ historicalCatchUpStage[initialContext][node]
                       = "Applied"

IndexedCompositionInvariant ==
  /\ IndexedAsyncStateShape
  /\ JoinedByContextShape
  /\ HistoricalCatchUpShape
  /\ SuccessorActivationShape
  /\ Chain!ChainEpochInvariant
  /\ IndexedTotalReceiptProjection
  /\ IndexedEveryInstanceStrongInvariant
  /\ JoinedContextCertificationInvariant
  /\ JoinedRoutingInvariant
  /\ IndexedApplicationsRespectNodeHeight
  /\ HistoricalCatchUpEvidenceInvariant

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
           IndexedCore, IndexedScheduler, IndexedRecovery

(***************************************************************************
The recovery projection is extensional, not merely length-compatible.  These
facts pin the four production fields at the chain-composition boundary and
prevent a future WITH-clause edit from silently dropping, duplicating, or
reordering recovery phase, owner, generation, or replay-queue state.
***************************************************************************)
THEOREM IndexedFourFieldRecoveryProjectionIsExact ==
  IndexedAsyncStateShape
    => \A initialContext \in AdmissibleContextRecords:
         /\ IndexedAsync(initialContext)!AsyncRecoveryVars =
              indexedAsyncState[initialContext][3]
         /\ indexedAsyncState[initialContext][3] =
              <<IndexedRecovery(initialContext, 1),
                IndexedRecovery(initialContext, 2),
                IndexedRecovery(initialContext, 3),
                IndexedRecovery(initialContext, 4)>>
BY Isa DEF IndexedAsyncStateShape,
           IndexedAsync!AsyncRecoveryVars, IndexedRecovery

THEOREM VerificationFourFieldRecoveryProjectionIsExact ==
  IndexedAsyncStateShape
    /\ VerificationContext \in AdmissibleContextRecords
    => /\ VerificationAsyncProof!AsyncRecoveryVars =
             indexedAsyncState[VerificationContext][3]
       /\ indexedAsyncState[VerificationContext][3] =
            <<VerificationRecovery(1), VerificationRecovery(2),
              VerificationRecovery(3), VerificationRecovery(4)>>
BY Isa DEF IndexedAsyncStateShape,
           VerificationAsyncProof!AsyncRecoveryVars,
           VerificationRecovery, IndexedRecovery

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
    /\ historicalCatchUpDecisions = {}
    /\ historicalCatchUpApplications = {}
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
           IndexedAsync!AsyncRunnerStep

THEOREM JoinedNonRunnerIsExactAsyncWork ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedJoinedNonRunnerStep(initialContext)
      => IndexedAsync(initialContext)!AsyncNonRunnerStep
BY Isa DEF IndexedJoinedNonRunnerStep,
           IndexedAsync!AsyncNonRunnerStep

THEOREM JoinedAsyncStepRefinesExactAsyncStep ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedJoinedAsyncNext(initialContext)
      => IndexedAsync(initialContext)!AsyncNext
BY Isa DEF IndexedJoinedAsyncNext, IndexedJoinedNonCrashStep,
           IndexedAsync!AsyncNext, IndexedAsync!AsyncNonCrashStep

THEOREM JoinedNodeNeverWaitsForAllPeers ==
  \A initialContext \in AdmissibleContextRecords:
    \A node \in IndexedAsync(initialContext)!
                  AsyncCurrentResponsiveVoters:
      (/\ IndexedNodeCurrentAt(initialContext, node)
       /\ IndexedAsync(initialContext)!RunNode(node))
        => IndexedJoinedRunnerStep(initialContext)
BY DEF IndexedJoinedRunnerStep

THEOREM HistoricalServiceSurvivesLocalAdvance ==
  \A initialContext \in AdmissibleContextRecords:
    \A node \in IndexedAsync(initialContext)!
                  AsyncCurrentResponsiveVoters:
      (/\ node \in joinedByContext[initialContext]
       /\ IndexedAsync(initialContext)!RunHistoricalServer(node))
        => IndexedJoinedRunnerStep(initialContext)
BY DEF IndexedJoinedRunnerStep

THEOREM HistoricalCatchUpCopiesCanonicalIdentity ==
  \A initialContext \in AdmissibleContextRecords,
     node \in ValidatorIds, server \in ValidatorIds,
     source \in Chain!DecisionEvidenceSet:
    (IndexedHistoricalCatchUpDecision(
       initialContext, node, server, source)
      \/ IndexedHistoricalCatchUpApplication(
           initialContext, node, server, source))
      => /\ HistoricalCatchUpRecord(node, source).qc.context
                = initialContext
         /\ HistoricalCatchUpRecord(node, source).qc = source.qc
         /\ HistoricalCatchUpRecord(node, source).qc.subject
                = source.qc.subject
         /\ Chain!CanonicalCommitForSlot(
              HistoricalCatchUpRecord(node, source).qc,
              initialContext.height + 1)
BY DEF IndexedHistoricalCatchUpDecision,
       IndexedHistoricalCatchUpApplication,
       HistoricalCatchUpSource, HistoricalCatchUpRecord,
       IndexedCurrentDecisions

THEOREM SuccessorRosterEntrantIsCatchUpEligible ==
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
      => HistoricalCatchUpTarget(initialContext, node)
BY DEF HistoricalCatchUpTarget

(***************************************************************************
Regression witness for the production restart path: old-roster membership is
not a requester exclusion.  A responsive validator that restarts at its exact
old context and lacks that context's application is eligible for the same
authenticated CommitQC/body recovery as an observer or successor entrant.
***************************************************************************)
THEOREM RestartedCurrentRosterValidatorIsCatchUpEligible ==
  \A initialContext \in AdmissibleContextRecords,
     node \in ValidatorIds:
    ( /\ initialContext.height < MaxHeight
      /\ node \in Responsive
      /\ node \in IndexedAsync(initialContext)!
                   AsyncVotersAt(initialContext)
      /\ nodeHeight[node] = initialContext.height
      /\ nodeContext[node] = initialContext
      /\ ~IndexedAsync(initialContext)!NodeHasApplication(node))
      => HistoricalCatchUpTarget(initialContext, node)
BY DEF HistoricalCatchUpTarget,
       IndexedProjectedNodeHasApplication,
       CatchUpNodeHasApplicationProjection,
       IndexedAsync!NodeHasApplication

THEOREM JoinedMembershipIsMonotone ==
  IndexedChainNext
    => \A initialContext \in AdmissibleContextRecords:
         joinedByContext[initialContext]
           \subseteq joinedByContext'[initialContext]
BY Isa DEF IndexedChainNext, IndexedReceiptClassification,
           IndexedReceiptFreeChainStutter,
           IndexedDecisionReceiptHandoff,
           IndexedApplicationReceiptHandoff,
           IndexedHistoricalCatchUpDecision,
           IndexedHistoricalCatchUpApplication

THEOREM IndexedNodeHeightsAreMonotone ==
  IndexedChainNext
    => \A node \in ValidatorIds: nodeHeight[node] <= nodeHeight'[node]
BY Isa DEF IndexedChainNext, IndexedReceiptClassification,
           IndexedReceiptFreeChainStutter,
           IndexedDecisionReceiptHandoff,
           IndexedApplicationReceiptHandoff,
           IndexedHistoricalCatchUpDecision,
           IndexedHistoricalCatchUpApplication,
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
           IndexedHistoricalCatchUpDecision,
           IndexedHistoricalCatchUpApplication,
           Chain!ChainEpochNext

THEOREM IndexedStepProjectsEveryAsyncStep ==
  \A observedContext \in AdmissibleContextRecords:
    IndexedChainNext
      => [IndexedAsync(observedContext)!AsyncNext]_(
           IndexedAsyncStateAt(observedContext))
BY Isa DEF IndexedChainNext, JoinedAsyncStepRefinesExactAsyncStep,
           IndexedInstanceVariablesAreExact,
           IndexedHistoricalCatchUpDecision,
           IndexedHistoricalCatchUpApplication

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
           IndexedHistoricalCatchUpDecision,
           IndexedHistoricalCatchUpApplication,
           HistoricalCatchUpRecord,
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

THEOREM IndexedInitEstablishesCompositionInvariant ==
  IndexedChainInit => IndexedCompositionInvariant
BY Isa, Chain!GenesisEstablishesChainEpochInvariant,
   IndexedChainInitHasEmptyCurrentReceiptUnion,
   IndexedInitEstablishesEveryInstanceStrongInvariant
   DEF IndexedChainInit, IndexedCompositionInvariant,
       IndexedTotalReceiptProjection,
       IndexedDecisionReceiptProjection,
       IndexedApplicationReceiptProjection,
       HistoricalCatchUpShape,
       JoinedContextCertificationInvariant, JoinedRoutingInvariant,
       IndexedApplicationsRespectNodeHeight,
       HistoricalCatchUpEvidenceInvariant,
       HistoricalCatchUpApplicationAt,
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

THEOREM IndexedActionPreservesCompositionInvariant ==
  IndexedCompositionInvariant /\ IndexedChainNext
    => IndexedCompositionInvariant'
BY Isa, AppliedSuccessorIsAdmissible,
   IndexedStepProjectsChainEpochStep,
   Chain!ChainEpochInductiveStep,
   IndexedStepPreservesReceiptProjection,
   IndexedActionPreservesEveryInstanceStrongInvariant
   DEF IndexedCompositionInvariant, IndexedChainNext,
       IndexedProductActionAt, IndexedReceiptClassification,
       IndexedReceiptFreeChainStutter,
       IndexedDecisionReceiptHandoff,
       IndexedApplicationReceiptHandoff,
       IndexedHistoricalCatchUpDecision,
       IndexedHistoricalCatchUpApplication,
       HistoricalCatchUpTarget, HistoricalCatchUpSource,
       HistoricalCatchUpRecord,
       NewIndexedDecisionReceipt, NewIndexedApplicationReceipt,
       NoNewIndexedDurableReceipt,
       IndexedEveryInstanceStrongInvariant,
       JoinedContextCertificationInvariant, JoinedRoutingInvariant,
       IndexedApplicationsRespectNodeHeight,
       HistoricalCatchUpEvidenceInvariant,
       HistoricalCatchUpReceiptSound,
       HistoricalCatchUpDecisionAt,
       HistoricalCatchUpApplicationAt,
       IndexedNodeCurrentAt, JoinedContexts, SuccessorContextFor,
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
             JoinedContextCertificationInvariant, JoinedRoutingInvariant,
             IndexedApplicationsRespectNodeHeight,
             HistoricalCatchUpShape,
             HistoricalCatchUpEvidenceInvariant,
             HistoricalCatchUpReceiptSound,
             HistoricalCatchUpApplicationAt,
             HistoricalCatchUpSource, HistoricalCatchUpRecord,
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

THEOREM PostFailurePublicationReadyPersistsUntilProgress ==
  \A parentContext \in AdmissibleContextRecords,
     node \in ValidatorIds:
    IndexedCompositionInvariant
      /\ PostFailureSuccessorPublicationReady(parentContext, node)
      /\ [IndexedChainNext]_IndexedChainVars
      => \/ PostFailureSuccessorPublicationReady(parentContext, node)'
         \/ SuccessorPublicationOrSuperseded(parentContext, node)'
BY Isa DEF PostFailureSuccessorPublicationReady,
           SuccessorPublicationOrSuperseded,
           SuccessorHeightActivated,
           IndexedChainNext, IndexedChainVars,
           IndexedProductActionAt, IndexedReceiptClassification,
           IndexedReceiptFreeChainStutter,
           IndexedDecisionReceiptHandoff,
           IndexedApplicationReceiptHandoff,
           IndexedHistoricalCatchUpPipelineAction,
           IndexedHistoricalCatchUpDecision,
           IndexedHistoricalCatchUpBodyRecovery,
           IndexedHistoricalCatchUpBodyStore,
           IndexedHistoricalCatchUpValidation,
           IndexedHistoricalCatchUpApplication,
           IndexedSuccessorActivationProgressStep,
           QueueSuccessorActivation,
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
           IndexedCompositionInvariant,
           Chain!ChainEpochInvariant,
           Chain!RecordCertifiedNext, Chain!RecordKnownDecision,
           Chain!RecordAppliedNext, Chain!RecordKnownApplication

THEOREM PostFailureSuccessorPublicationFairSuffix ==
  IndexedChainSpec =>
    \A parentContext \in AdmissibleContextRecords,
       node \in ValidatorIds:
      PostFailureSuccessorPublicationReady(parentContext, node)
        ~> SuccessorPublicationOrSuperseded(parentContext, node)
PROOF
  <1>1. ASSUME IndexedChainSpec,
              NEW parentContext \in AdmissibleContextRecords,
              NEW node \in ValidatorIds
         PROVE PostFailureSuccessorPublicationReady(parentContext, node)
                 ~> SuccessorPublicationOrSuperseded(
                      parentContext, node)
    <2>1. []IndexedCompositionInvariant
      BY <1>1, IndexedChainSpecEstablishesCompositionInvariant, PTL
    <2>2. [][IndexedChainNext]_IndexedChainVars
      BY <1>1, PTL DEF IndexedChainSpec
    <2>3. WF_IndexedChainVars(
             IndexedSuccessorActivationProgressStep(
               parentContext, node))
      BY <1>1, PTL DEF IndexedChainSpec, IndexedFairness
    <2>4. IndexedCompositionInvariant
             /\ PostFailureSuccessorPublicationReady(parentContext, node)
             => ENABLED
                  <<IndexedSuccessorActivationProgressStep(
                      parentContext, node)>>_(IndexedChainVars)
      BY <1>1, PostFailurePublicationReadyEnablesProgress
         DEF IndexedCompositionInvariant
    <2>5. PostFailureSuccessorPublicationReady(parentContext, node)
             /\ IndexedSuccessorActivationProgressStep(
                  parentContext, node)
             => SuccessorPublicationOrSuperseded(parentContext, node)'
      BY <1>1, PostFailurePublicationProgressPublishes
         DEF SuccessorPublicationOrSuperseded
    <2>6. IndexedCompositionInvariant
             /\ PostFailureSuccessorPublicationReady(parentContext, node)
             /\ [IndexedChainNext]_IndexedChainVars
             => \/ PostFailureSuccessorPublicationReady(
                       parentContext, node)'
                \/ SuccessorPublicationOrSuperseded(parentContext, node)'
      BY <1>1, PostFailurePublicationReadyPersistsUntilProgress
    <2> QED BY <2>1, <2>2, <2>3, <2>4, <2>5, <2>6, PTL
  <1> QED BY <1>1

THEOREM StableHeightPostFailureSuffixPublishes ==
  IndexedChainSpec =>
    \A parentContext \in AdmissibleContextRecords,
       node \in ValidatorIds:
      []~(nodeHeight[node] > parentContext.height + 1)
        => PostFailureSuccessorPublicationReady(parentContext, node)
             ~> SuccessorHeightActivated(parentContext, node)
PROOF
  <1>1. ASSUME IndexedChainSpec,
              NEW parentContext \in AdmissibleContextRecords,
              NEW node \in ValidatorIds,
              []~(nodeHeight[node] > parentContext.height + 1)
         PROVE PostFailureSuccessorPublicationReady(parentContext, node)
                 ~> SuccessorHeightActivated(parentContext, node)
    <2>1. PostFailureSuccessorPublicationReady(parentContext, node)
             ~> SuccessorPublicationOrSuperseded(parentContext, node)
      BY <1>1, PostFailureSuccessorPublicationFairSuffix
    <2> QED BY <1>1, <2>1, PTL
         DEF SuccessorPublicationOrSuperseded
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
           IndexedHistoricalCatchUpDecision,
           IndexedHistoricalCatchUpApplication,
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
           /\ \/ IndexedAsync(initialContext)!NodeHasApplication(node)
              \/ HistoricalCatchUpApplicationAt(initialContext, node)
BY DEF IndexedCompositionInvariant, JoinedRoutingInvariant

THEOREM JoinedNonCurrentDisablesExactRunNode ==
  \A initialContext \in AdmissibleContextRecords:
    \A node \in joinedByContext[initialContext]:
      (IndexedCompositionInvariant
        /\ ~IndexedNodeCurrentAt(initialContext, node))
        => ~IndexedAsync(initialContext)!RunNode(node)
BY Isa, JoinedNonCurrentHasApplicationEvidence
   DEF IndexedCompositionInvariant,
       HistoricalCatchUpEvidenceInvariant,
       HistoricalCatchUpReceiptSound,
       HistoricalCatchUpApplicationAt,
       IndexedAsync!RunNode, IndexedAsync!AsyncVotersAt

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
           JoinedByContextShape, HistoricalCatchUpShape,
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
       HistoricalCatchUpShape,
       HistoricalCatchUpEvidenceInvariant,
       HistoricalCatchUpReceiptSound,
       IndexedTotalReceiptProjection,
       IndexedDecisionReceiptProjection,
       IndexedApplicationReceiptProjection,
       IndexedDecisionEvidence, IndexedApplicationEvidence,
       IndexedCurrentDecisions, IndexedCurrentApplications,
       IndexedNodeCurrentAt, JoinedContexts, SuccessorContextFor,
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
                                  PostGstRunHistoricalServer(node)
                          => ENABLED IndexedHistoricalServerStep(
                               initialContext, node))
                   /\ (ENABLED IndexedAsync(initialContext)!
                                  PostGstServiceIoWorker(node)
                          => ENABLED
                               IndexedIoWorkerStep(initialContext, node))
         /\ \A recipient \in IndexedAsync(initialContext)!
                            AsyncVotersAt(initialContext),
               source \in IndexedAsync(initialContext)!
                         AsyncVotersAt(initialContext):
              ENABLED IndexedAsync(initialContext)!
                        PostGstAdmitHiddenPacket(recipient, source)
                => ENABLED IndexedAdmitPacketStep(
                     initialContext, recipient, source)
BY Isa, IndexedJoinedActionHasProductExtension,
   JoinedNonCurrentDisablesExactRunNode
   DEF IndexedSetGstStep, IndexedTickStep, IndexedRunNodeStep,
       IndexedCommitCertificateDiscoveryStep,
       IndexedHistoricalServerStep, IndexedIoWorkerStep,
       IndexedAdmitPacketStep, IndexedChainNext,
       IndexedProductActionAt, IndexedJoinedAsyncNext,
       IndexedJoinedNonCrashStep, IndexedJoinedRunnerStep,
       IndexedJoinedNonRunnerStep, IndexedNodeCurrentAt,
       IndexedAsync!PostGstRunNode,
       IndexedAsync!PostGstCommitCertificateDiscovery,
       IndexedAsync!PostGstRunHistoricalServer,
       IndexedAsync!PostGstServiceIoWorker,
       IndexedAsync!PostGstAdmitHiddenPacket,
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
voter, the already-running restricted behavior satisfies the exact
AsyncSpecAt fairness obligations. Early joined work is part of that same
behavior and is never blocked. IndexedFairActionsRemainEnabledInProduct proves
that the receipt wrapper does not hide enabled exact actions. Once a joined
node is no longer current, JoinedNonCurrentDisablesExactRunNode makes its exact
RunNode fairness obligation vacuous while historical service stays fair.
The exact Idle and Validated catch-up suffixes consume fairness of the complete
pipeline.  Progress across the intervening body-recovery, store, and validation
stages remains the explicit IndexedHistoricalCatchUpRankProgress premise of
the conditional height kernel; no subaction fairness is inferred from the
union action. Terminal application has no join.
VerificationOneHeightCompletion is the exact fixed-context expansion of the
one-height completion property over the parameterized production-network
instance. Its wrapper is supplied by the nonparameterized asynchronous
liveness theorem, which still depends on the proofless rotating-leader and
application-liveness declarations.  The conditional final proof composes
explicit premises over finite Heights; it does not hide them as a new protocol
relation.
***************************************************************************)
IndexedAllResponsiveJoined(initialContext) ==
  IndexedAsync(initialContext)!AsyncVotersAt(initialContext)
    \subseteq joinedByContext[initialContext]

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
   DEF IndexedAllResponsiveJoined, JoinedContexts

THEOREM IndexedAllResponsiveJoinedIsStable ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedAllResponsiveJoined(initialContext)
      /\ [IndexedChainNext]_IndexedChainVars
      => IndexedAllResponsiveJoined(initialContext)'
BY Isa, JoinedMembershipIsMonotone
   DEF IndexedAllResponsiveJoined, IndexedChainVars

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
         /\ (IndexedHistoricalServerStep(initialContext, node)
               => <<IndexedAsync(initialContext)!
                       PostGstRunHistoricalServer(node)>>_(
                    IndexedAsyncStateAt(initialContext)))
         /\ (IndexedIoWorkerStep(initialContext, node)
               => <<IndexedAsync(initialContext)!
                       PostGstServiceIoWorker(node)>>_(
                    IndexedAsyncStateAt(initialContext)))
    /\ \A recipient \in IndexedAsync(initialContext)!
                         AsyncVotersAt(initialContext),
          source \in IndexedAsync(initialContext)!
                    AsyncVotersAt(initialContext):
         IndexedAdmitPacketStep(initialContext, recipient, source)
           => <<IndexedAsync(initialContext)!
                   PostGstAdmitHiddenPacket(recipient, source)>>_(
                IndexedAsyncStateAt(initialContext))
BY Isa DEF IndexedSetGstStep, IndexedTickStep,
           IndexedRunNodeStep, IndexedHistoricalServerStep,
           IndexedCommitCertificateDiscoveryStep,
           IndexedIoWorkerStep, IndexedAdmitPacketStep,
           IndexedChainNext, IndexedProductActionAt,
           IndexedReceiptClassification, IndexedReceiptFreeChainStutter,
           IndexedDecisionReceiptHandoff,
           IndexedApplicationReceiptHandoff,
           IndexedHistoricalCatchUpDecision,
           IndexedHistoricalCatchUpApplication,
           IndexedChainVars, IndexedAsyncStateAt,
           IndexedAsync!AsyncSetGST, IndexedAsync!SetGST,
           IndexedAsync!AsyncTick,
           IndexedAsync!PostGstRunNode, IndexedAsync!RunNode,
           IndexedAsync!PostGstCommitCertificateDiscovery,
           IndexedAsync!DirectCommitCertificateDiscoveryStep,
           IndexedAsync!PostGstRunHistoricalServer,
           IndexedAsync!RunHistoricalServer,
           IndexedAsync!PostGstServiceIoWorker,
           IndexedAsync!ServiceIoWorker,
           IndexedAsync!PostGstAdmitHiddenPacket,
           IndexedAsync!AdmitHiddenPacket

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
         /\ \A recipient \in IndexedAsync(initialContext)!
                            AsyncVotersAt(initialContext),
               source \in IndexedAsync(initialContext)!
                        AsyncVotersAt(initialContext):
              ENABLED
                <<IndexedAsync(initialContext)!
                    PostGstAdmitHiddenPacket(recipient, source)>>_(
                  IndexedAsyncStateAt(initialContext))
                => ENABLED
                     <<IndexedAdmitPacketStep(
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
           /\ WF_(IndexedAsyncStateAt(initialContext))(
                  IndexedAsync(initialContext)!
                    PostGstRunHistoricalServer(node))
           /\ WF_(IndexedAsyncStateAt(initialContext))(
                  IndexedAsync(initialContext)!PostGstServiceIoWorker(node))
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
                /\ WF_(IndexedAsyncStateAt(initialContext))(
                       IndexedAsync(initialContext)!
                         PostGstRunHistoricalServer(node))
                /\ WF_(IndexedAsyncStateAt(initialContext))(
                       IndexedAsync(initialContext)!
                         PostGstServiceIoWorker(node))
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
              /\ (IndexedHistoricalServerStep(initialContext, node)
                   => <<IndexedAsync(initialContext)!
                           PostGstRunHistoricalServer(node)>>_(
                        IndexedAsyncStateAt(initialContext)))
              /\ (IndexedIoWorkerStep(initialContext, node)
                   => <<IndexedAsync(initialContext)!
                           PostGstServiceIoWorker(node)>>_(
                        IndexedAsyncStateAt(initialContext)))
      BY <1>1, IndexedFairProductStepsProjectExactOccurrences
    <2>4. IndexedChainSpec
             => /\ WF_IndexedChainVars(
                       IndexedRunNodeStep(initialContext, node))
                /\ WF_IndexedChainVars(
                       IndexedCommitCertificateDiscoveryStep(
                         initialContext, node))
                /\ WF_IndexedChainVars(
                       IndexedHistoricalServerStep(initialContext, node))
                /\ WF_IndexedChainVars(
                       IndexedIoWorkerStep(initialContext, node))
      BY <1>1 DEF IndexedChainSpec, IndexedFairness
    <2> QED BY <2>1, <2>2, <2>3, <2>4, PTL
  <1> QED BY <1>1

THEOREM IndexedPacketFairnessTransfers ==
  \A initialContext \in AdmissibleContextRecords:
    \A recipient \in IndexedAsync(initialContext)!
                        AsyncVotersAt(initialContext),
       source \in IndexedAsync(initialContext)!
                  AsyncVotersAt(initialContext):
      (/\ IndexedChainSpec
       /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
        => WF_(IndexedAsyncStateAt(initialContext))(
             IndexedAsync(initialContext)!
               PostGstAdmitHiddenPacket(recipient, source))
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
              NEW recipient \in IndexedAsync(initialContext)!
                                  AsyncVotersAt(initialContext),
              NEW source \in IndexedAsync(initialContext)!
                               AsyncVotersAt(initialContext)
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
    <2>4. (/\ IndexedChainSpec
            /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
             => WF_(IndexedAsyncStateAt(initialContext))(
                  IndexedAsync(initialContext)!AsyncSetGST)
      BY <1>1, IndexedSetGstFairnessTransfers
    <2>5. (/\ IndexedChainSpec
            /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
             => WF_(IndexedAsyncStateAt(initialContext))(
                  IndexedAsync(initialContext)!AsyncTick)
      BY <1>1, IndexedTickFairnessTransfers
    <2>6. (/\ IndexedChainSpec
            /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
             => \A node \in IndexedAsync(initialContext)!
                               AsyncVotersAt(initialContext):
                  /\ WF_(IndexedAsyncStateAt(initialContext))(
                       IndexedAsync(initialContext)!PostGstRunNode(node))
                  /\ WF_(IndexedAsyncStateAt(initialContext))(
                       IndexedAsync(initialContext)!
                         PostGstCommitCertificateDiscovery(node))
                  /\ WF_(IndexedAsyncStateAt(initialContext))(
                       IndexedAsync(initialContext)!
                         PostGstRunHistoricalServer(node))
                  /\ WF_(IndexedAsyncStateAt(initialContext))(
                       IndexedAsync(initialContext)!
                         PostGstServiceIoWorker(node))
      BY <1>1, IndexedNodeFairnessTransfers
    <2>7. (/\ IndexedChainSpec
            /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
             => \A recipient \in IndexedAsync(initialContext)!
                                  AsyncVotersAt(initialContext),
                   source \in IndexedAsync(initialContext)!
                             AsyncVotersAt(initialContext):
                  WF_(IndexedAsyncStateAt(initialContext))(
                    IndexedAsync(initialContext)!
                      PostGstAdmitHiddenPacket(recipient, source))
      BY <1>1, IndexedPacketFairnessTransfers
    <2> QED BY <2>1, <2>2, <2>3, <2>4, <2>5, <2>6, <2>7, PTL
         DEF IndexedAsync!AsyncSpecAt, IndexedAsync!AsyncFairnessAt
  <1> QED BY <1>1

IndexedCatchUpDecisionReady(initialContext, node) ==
  \E server \in ValidatorIds,
     source \in Chain!DecisionEvidenceSet:
    /\ HistoricalCatchUpTarget(initialContext, node)
    /\ HistoricalCatchUpSource(initialContext, server, source)
    /\ HistoricalCatchUpRecord(node, source)
         \notin IndexedDecisionEvidence

IndexedCatchUpApplicationReady(initialContext, node) ==
  \E server \in ValidatorIds,
     source \in Chain!DecisionEvidenceSet:
    /\ HistoricalCatchUpTarget(initialContext, node)
    /\ HistoricalCatchUpSource(initialContext, server, source)
    /\ HistoricalCatchUpRecord(node, source)
         \in historicalCatchUpDecisions
    /\ HistoricalCatchUpRecord(node, source)
         \notin historicalCatchUpApplications

(***************************************************************************
The broad predicates above are discovery predicates used by the finite-height
argument.  Enabledness is stage-exact: a decision can enter only at Idle and
an application can enter only after validation.  Keeping those notions
separate prevents a durable decision receipt from being mistaken for an
enabled application action while body recovery, storage, or validation is
still outstanding.
***************************************************************************)
IndexedCatchUpIdleDecisionReady(initialContext, node) ==
  /\ initialContext.height < MaxHeight
  /\ historicalCatchUpStage[initialContext][node] = "Idle"
  /\ IndexedCatchUpDecisionReady(initialContext, node)

IndexedCatchUpValidatedApplicationReady(initialContext, node) ==
  /\ initialContext.height < MaxHeight
  /\ historicalCatchUpStage[initialContext][node] = "Validated"
  /\ IndexedCatchUpApplicationReady(initialContext, node)

HistoricalCatchUpStageRank(stage) ==
  CASE stage = "Idle"              -> 5
    [] stage = "DecisionRecovered" -> 4
    [] stage = "BodyRecovered"     -> 3
    [] stage = "Stored"            -> 2
    [] stage = "Validated"         -> 1
    [] OTHER                        -> 0

IndexedCatchUpStageRankAt(initialContext, node) ==
  HistoricalCatchUpStageRank(
    historicalCatchUpStage[initialContext][node])

THEOREM IndexedCatchUpStageRankIsFinite ==
  HistoricalCatchUpShape
    => \A initialContext \in AdmissibleContextRecords,
          node \in ValidatorIds:
         IndexedCatchUpStageRankAt(initialContext, node) \in 0..5
BY Isa DEF HistoricalCatchUpShape,
           IndexedCatchUpStageRankAt, HistoricalCatchUpStageRank

THEOREM IndexedCatchUpPipelineStrictlyDecreasesStageRank ==
  \A initialContext \in AdmissibleContextRecords,
     node \in ValidatorIds:
    IndexedCatchUpPipelineStep(initialContext, node)
      => IndexedCatchUpStageRankAt(initialContext, node)'
           < IndexedCatchUpStageRankAt(initialContext, node)
BY Isa DEF IndexedCatchUpStageRankAt, HistoricalCatchUpStageRank,
           IndexedCatchUpPipelineStep,
           IndexedHistoricalCatchUpPipelineAction,
           IndexedHistoricalCatchUpDecision,
           IndexedHistoricalCatchUpBodyRecovery,
           IndexedHistoricalCatchUpBodyStore,
           IndexedHistoricalCatchUpValidation,
           IndexedHistoricalCatchUpApplication

IndexedHistoricalCatchUpRankProgress ==
  \A initialContext \in AdmissibleContextRecords,
     node \in ValidatorIds:
    /\ (initialContext.height < MaxHeight
          /\ IndexedCatchUpDecisionReady(initialContext, node))
         ~> nodeHeight[node] > initialContext.height
    /\ (initialContext.height < MaxHeight
          /\ IndexedCatchUpApplicationReady(initialContext, node))
         ~> nodeHeight[node] > initialContext.height

(***************************************************************************
The Idle/Validated refinements are exact enabledness witnesses for the two
boundary actions.  The composition invariant supplies the receipt projection
and canonical-prefix facts required by RecordKnownDecision and
RecordAppliedNext; stage-exact readiness contributes the particular honest
signer, body, target, and source.  These lemmas make the supported fairness
suffixes explicit without treating historical service as a progress oracle.
***************************************************************************)
IndexedCatchUpFrameInvariant ==
  /\ IndexedAsyncStateShape
  /\ JoinedByContextShape
  /\ HistoricalCatchUpShape
  /\ Chain!ChainEpochInvariant
  /\ IndexedTotalReceiptProjection

THEOREM IndexedCompositionSuppliesCatchUpFrame ==
  IndexedCompositionInvariant => IndexedCatchUpFrameInvariant
BY DEF IndexedCompositionInvariant, IndexedCatchUpFrameInvariant

IndexedCatchUpDecisionActionGuard(initialContext, node, server, source) ==
  LET decision == HistoricalCatchUpRecord(node, source)
  IN /\ HistoricalCatchUpTarget(initialContext, node)
     /\ HistoricalCatchUpSource(initialContext, server, source)
     /\ initialContext.height < MaxHeight
     /\ historicalCatchUpStage[initialContext][node] = "Idle"
     /\ decision \notin IndexedDecisionEvidence
     /\ Chain!DurableCommitDecision(decision)
     /\ decision \notin durableDecisionEvidence
     /\ \/ Chain!DecisionBacksCertifiedSlot(decision)
        \/ Chain!ReceiptOutsideChainHorizon(decision)

THEOREM IndexedCatchUpIdleDecisionReadySuppliesActionGuard ==
  \A initialContext \in AdmissibleContextRecords,
     node \in ValidatorIds:
    IndexedCatchUpFrameInvariant
      /\ IndexedCatchUpIdleDecisionReady(initialContext, node)
      => \E server \in ValidatorIds,
            source \in Chain!DecisionEvidenceSet:
           IndexedCatchUpDecisionActionGuard(
             initialContext, node, server, source)
BY Isa DEF IndexedCatchUpFrameInvariant,
           IndexedCatchUpIdleDecisionReady,
           IndexedCatchUpDecisionReady,
           IndexedCatchUpDecisionActionGuard,
           HistoricalCatchUpSource, HistoricalCatchUpRecord,
           IndexedTotalReceiptProjection,
           IndexedDecisionReceiptProjection,
           Chain!ChainEpochInvariant,
           Chain!DurableDecisionEvidenceSound,
           Chain!DurableCommitDecision,
           Chain!HistoricalCommitCertificate,
           Chain!DecisionBacksCertifiedSlot,
           Chain!ReceiptOutsideChainHorizon

THEOREM IndexedCatchUpDecisionFrameGuardEnablesService ==
  \A initialContext \in AdmissibleContextRecords,
     node \in ValidatorIds,
     server \in ValidatorIds,
     source \in Chain!DecisionEvidenceSet:
    IndexedCatchUpFrameInvariant
      /\ IndexedCatchUpDecisionActionGuard(
           initialContext, node, server, source)
      => ENABLED
           <<IndexedCatchUpServiceStep(initialContext, node)>>_(
             IndexedChainVars)
BY ExpandENABLED, Isa
   DEF IndexedCatchUpFrameInvariant,
       IndexedCatchUpDecisionActionGuard,
       IndexedCatchUpServiceStep,
       IndexedHistoricalCatchUpDecision,
       HistoricalCatchUpRecord,
       IndexedChainVars, IndexedAsyncStateShape,
       JoinedByContextShape, HistoricalCatchUpShape,
       Chain!RecordKnownDecision, Chain!ChainEpochVars

THEOREM IndexedCatchUpIdleDecisionReadyEnablesService ==
  \A initialContext \in AdmissibleContextRecords,
     node \in ValidatorIds:
    IndexedCompositionInvariant
      /\ IndexedCatchUpIdleDecisionReady(initialContext, node)
      => ENABLED
           <<IndexedCatchUpServiceStep(initialContext, node)>>_(
             IndexedChainVars)
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
              NEW node \in ValidatorIds,
              IndexedCompositionInvariant,
              IndexedCatchUpIdleDecisionReady(initialContext, node)
         PROVE ENABLED
                 <<IndexedCatchUpServiceStep(initialContext, node)>>_(
                   IndexedChainVars)
    <2>1. IndexedCatchUpFrameInvariant
      BY <1>1, IndexedCompositionSuppliesCatchUpFrame
    <2>2. \E server \in ValidatorIds,
               source \in Chain!DecisionEvidenceSet:
             IndexedCatchUpDecisionActionGuard(
               initialContext, node, server, source)
      BY <1>1, <2>1,
         IndexedCatchUpIdleDecisionReadySuppliesActionGuard
    <2> QED BY <2>1, <2>2,
                 IndexedCatchUpDecisionFrameGuardEnablesService
  <1> QED BY <1>1

IndexedCatchUpApplicationActionGuard(initialContext, node, server, source) ==
  LET application == HistoricalCatchUpRecord(node, source)
      nextHeight == nodeHeight[node] + 1
  IN /\ HistoricalCatchUpTarget(initialContext, node)
     /\ HistoricalCatchUpSource(initialContext, server, source)
     /\ initialContext.height < MaxHeight
     /\ historicalCatchUpStage[initialContext][node] = "Validated"
     /\ application \in historicalCatchUpDecisions
     /\ application \notin historicalCatchUpApplications
     /\ application \in Chain!DecisionEvidenceSet
     /\ node \in Honest
     /\ Chain!DurableCommitDecision(application)
     /\ nodeHeight[node] < certifiedHeight
     /\ Chain!CanonicalCommitForSlot(application.qc, nextHeight)
     /\ Chain!ApplicationHasRecordedDecision(application)

THEOREM IndexedCatchUpValidatedReadySuppliesActionGuard ==
  \A initialContext \in AdmissibleContextRecords,
     node \in ValidatorIds:
    IndexedCatchUpFrameInvariant
      /\ IndexedCatchUpValidatedApplicationReady(initialContext, node)
      => \E server \in ValidatorIds,
            source \in Chain!DecisionEvidenceSet:
           IndexedCatchUpApplicationActionGuard(
             initialContext, node, server, source)
BY Isa DEF IndexedCatchUpFrameInvariant,
           IndexedCatchUpValidatedApplicationReady,
           IndexedCatchUpApplicationReady,
           IndexedCatchUpApplicationActionGuard,
           HistoricalCatchUpTarget, HistoricalCatchUpSource,
           HistoricalCatchUpRecord,
           IndexedTotalReceiptProjection,
           IndexedDecisionReceiptProjection,
           IndexedApplicationReceiptProjection,
           IndexedDecisionEvidence, IndexedApplicationEvidence,
           Chain!ChainEpochInvariant, Chain!ChainEpochTypeInvariant,
           Chain!DurableDecisionEvidenceSound,
           Chain!DurableApplicationEvidenceSound,
           Chain!NodesDoNotOutrunCertificates,
           Chain!ApplicationHasRecordedDecision,
           Chain!DecisionBacksCertifiedSlot,
           Chain!ReceiptOutsideChainHorizon,
           Chain!DurableCommitDecision,
           Chain!CanonicalCommitForSlot,
           Chain!HistoricalCommitCertificate

THEOREM IndexedCatchUpApplicationFrameGuardEnablesApplication ==
  \A initialContext \in AdmissibleContextRecords,
     node \in ValidatorIds,
     server \in ValidatorIds,
     source \in Chain!DecisionEvidenceSet:
    IndexedCatchUpFrameInvariant
      /\ IndexedCatchUpApplicationActionGuard(
           initialContext, node, server, source)
      => ENABLED
           <<IndexedCatchUpApplicationStep(initialContext, node)>>_(
             IndexedChainVars)
BY AppliedSuccessorIsAdmissible, ExpandENABLED, Isa
   DEF IndexedCatchUpFrameInvariant,
       IndexedCatchUpApplicationActionGuard,
       IndexedCatchUpApplicationStep,
       IndexedHistoricalCatchUpApplication,
       HistoricalCatchUpRecord, SuccessorContextFor,
       IndexedChainVars, IndexedAsyncStateShape,
       JoinedByContextShape, HistoricalCatchUpShape,
       Chain!RecordAppliedNext, Chain!ChainEpochVars

THEOREM IndexedCatchUpValidatedReadyEnablesApplication ==
  \A initialContext \in AdmissibleContextRecords,
     node \in ValidatorIds:
    IndexedCompositionInvariant
      /\ IndexedCatchUpValidatedApplicationReady(initialContext, node)
      => ENABLED
           <<IndexedCatchUpApplicationStep(initialContext, node)>>_(
             IndexedChainVars)
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
              NEW node \in ValidatorIds,
              IndexedCompositionInvariant,
              IndexedCatchUpValidatedApplicationReady(initialContext, node)
         PROVE ENABLED
                 <<IndexedCatchUpApplicationStep(initialContext, node)>>_(
                   IndexedChainVars)
    <2>1. IndexedCatchUpFrameInvariant
      BY <1>1, IndexedCompositionSuppliesCatchUpFrame
    <2>2. \E server \in ValidatorIds,
               source \in Chain!DecisionEvidenceSet:
             IndexedCatchUpApplicationActionGuard(
               initialContext, node, server, source)
      BY <1>1, <2>1,
         IndexedCatchUpValidatedReadySuppliesActionGuard
    <2> QED BY <2>1, <2>2,
                 IndexedCatchUpApplicationFrameGuardEnablesApplication
  <1> QED BY <1>1

THEOREM IndexedCatchUpServiceEstablishesDecision ==
  \A initialContext \in AdmissibleContextRecords,
     node \in ValidatorIds:
    IndexedCatchUpServiceStep(initialContext, node)
      => HistoricalCatchUpDecisionAt(initialContext, node)'
BY Isa DEF IndexedCatchUpServiceStep,
           IndexedHistoricalCatchUpDecision,
           HistoricalCatchUpDecisionAt,
           HistoricalCatchUpRecord

THEOREM IndexedCatchUpApplicationEstablishesAdvance ==
  \A initialContext \in AdmissibleContextRecords,
     node \in ValidatorIds:
    initialContext.height < MaxHeight
      /\ IndexedCatchUpApplicationStep(initialContext, node)
      => /\ HistoricalCatchUpApplicationAt(initialContext, node)'
         /\ nodeHeight'[node] > initialContext.height
BY Isa DEF IndexedCatchUpApplicationStep,
           IndexedHistoricalCatchUpApplication,
           HistoricalCatchUpApplicationAt,
           HistoricalCatchUpRecord,
           Chain!RecordAppliedNext

THEOREM IndexedCatchUpDecisionReadyPersistsUntilReceipt ==
  \A initialContext \in AdmissibleContextRecords,
     node \in ValidatorIds:
    IndexedCompositionInvariant
      /\ IndexedCatchUpIdleDecisionReady(initialContext, node)
      /\ [IndexedChainNext]_IndexedChainVars
      => \/ IndexedCatchUpIdleDecisionReady(initialContext, node)'
         \/ HistoricalCatchUpDecisionAt(initialContext, node)'
         \/ nodeHeight'[node] > initialContext.height
BY Isa DEF IndexedCatchUpIdleDecisionReady,
           IndexedCatchUpDecisionReady,
           HistoricalCatchUpDecisionAt,
           IndexedChainNext, IndexedChainVars,
           IndexedProductActionAt, IndexedReceiptClassification,
           IndexedReceiptFreeChainStutter,
           IndexedDecisionReceiptHandoff,
           IndexedApplicationReceiptHandoff,
           IndexedHistoricalCatchUpDecision,
           IndexedHistoricalCatchUpBodyRecovery,
           IndexedHistoricalCatchUpBodyStore,
           IndexedHistoricalCatchUpValidation,
           IndexedHistoricalCatchUpApplication,
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
           HistoricalCatchUpTarget, HistoricalCatchUpSource,
           HistoricalCatchUpRecord,
           IndexedCompositionInvariant,
           IndexedTotalReceiptProjection,
           IndexedDecisionReceiptProjection,
           IndexedApplicationReceiptProjection,
           IndexedDecisionEvidence, IndexedApplicationEvidence,
           IndexedCurrentDecisions, IndexedCurrentApplications,
           IndexedEveryInstanceStrongInvariant,
           HistoricalCatchUpEvidenceInvariant,
           HistoricalCatchUpReceiptSound,
           HistoricalCatchUpApplicationAt,
           JoinedContexts, IndexedNodeCurrentAt,
           Chain!ChainEpochInvariant, Chain!ChainEpochTypeInvariant,
           Chain!RecordCertifiedNext, Chain!RecordKnownDecision,
           Chain!RecordAppliedNext, Chain!RecordKnownApplication,
           Chain!ChainEpochVars,
           IndexedAsync!StrongInductiveInvariant,
           IndexedAsync!Safety, IndexedAsync!TypeInvariant,
           IndexedAsync!DecisionAgreement,
           IndexedAsync!AppliedRequiresDecision,
           IndexedAsync!NodeHasApplication,
           IndexedAsync!AsyncVotersAt

THEOREM IndexedCatchUpApplicationReadyPersistsUntilAdvance ==
  \A initialContext \in AdmissibleContextRecords,
     node \in ValidatorIds:
    IndexedCompositionInvariant
      /\ IndexedCatchUpValidatedApplicationReady(initialContext, node)
      /\ [IndexedChainNext]_IndexedChainVars
      => \/ IndexedCatchUpValidatedApplicationReady(initialContext, node)'
         \/ (/\ HistoricalCatchUpApplicationAt(initialContext, node)'
             /\ nodeHeight'[node] > initialContext.height)
         \/ nodeHeight'[node] > initialContext.height
BY Isa DEF IndexedCatchUpValidatedApplicationReady,
           IndexedCatchUpApplicationReady,
           HistoricalCatchUpApplicationAt,
           IndexedChainNext, IndexedChainVars,
           IndexedProductActionAt, IndexedReceiptClassification,
           IndexedReceiptFreeChainStutter,
           IndexedDecisionReceiptHandoff,
           IndexedApplicationReceiptHandoff,
           IndexedHistoricalCatchUpDecision,
           IndexedHistoricalCatchUpBodyRecovery,
           IndexedHistoricalCatchUpBodyStore,
           IndexedHistoricalCatchUpValidation,
           IndexedHistoricalCatchUpApplication,
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
           HistoricalCatchUpTarget, HistoricalCatchUpSource,
           HistoricalCatchUpRecord,
           IndexedCompositionInvariant,
           IndexedTotalReceiptProjection,
           IndexedDecisionReceiptProjection,
           IndexedApplicationReceiptProjection,
           IndexedDecisionEvidence, IndexedApplicationEvidence,
           IndexedCurrentDecisions, IndexedCurrentApplications,
           IndexedEveryInstanceStrongInvariant,
           HistoricalCatchUpEvidenceInvariant,
           HistoricalCatchUpReceiptSound,
           HistoricalCatchUpDecisionAt,
           JoinedContexts, SuccessorContextFor,
           Chain!ChainEpochInvariant, Chain!ChainEpochTypeInvariant,
           Chain!RecordCertifiedNext, Chain!RecordKnownDecision,
           Chain!RecordAppliedNext, Chain!RecordKnownApplication,
           Chain!ChainEpochVars,
           IndexedAsync!StrongInductiveInvariant,
           IndexedAsync!Safety, IndexedAsync!TypeInvariant,
           IndexedAsync!DecisionAgreement,
           IndexedAsync!AppliedRequiresDecision,
           IndexedAsync!NodeHasApplication,
           IndexedAsync!AsyncVotersAt

THEOREM IndexedCatchUpServiceStepRefinesPipelineStep ==
  \A initialContext \in AdmissibleContextRecords,
     node \in ValidatorIds:
    IndexedCatchUpServiceStep(initialContext, node)
      => IndexedCatchUpPipelineStep(initialContext, node)
BY DEF IndexedCatchUpServiceStep, IndexedCatchUpPipelineStep,
       IndexedHistoricalCatchUpPipelineAction

THEOREM IndexedCatchUpApplicationStepRefinesPipelineStep ==
  \A initialContext \in AdmissibleContextRecords,
     node \in ValidatorIds:
    IndexedCatchUpApplicationStep(initialContext, node)
      => IndexedCatchUpPipelineStep(initialContext, node)
BY DEF IndexedCatchUpApplicationStep, IndexedCatchUpPipelineStep,
       IndexedHistoricalCatchUpPipelineAction

THEOREM IndexedCatchUpIdleDecisionReadyEnablesPipeline ==
  \A initialContext \in AdmissibleContextRecords,
     node \in ValidatorIds:
    IndexedCompositionInvariant
      /\ IndexedCatchUpIdleDecisionReady(initialContext, node)
      => ENABLED
           <<IndexedCatchUpPipelineStep(initialContext, node)>>_(
             IndexedChainVars)
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
              NEW node \in ValidatorIds,
              IndexedCompositionInvariant,
              IndexedCatchUpIdleDecisionReady(initialContext, node)
         PROVE ENABLED
                 <<IndexedCatchUpPipelineStep(initialContext, node)>>_(
                   IndexedChainVars)
    <2>1. ENABLED
             <<IndexedCatchUpServiceStep(initialContext, node)>>_(
               IndexedChainVars)
      BY <1>1, IndexedCatchUpIdleDecisionReadyEnablesService
    <2>2. IndexedCatchUpServiceStep(initialContext, node)
             => IndexedCatchUpPipelineStep(initialContext, node)
      BY <1>1, IndexedCatchUpServiceStepRefinesPipelineStep
    <2> QED BY <2>1, <2>2, Isa
  <1> QED BY <1>1

THEOREM IndexedCatchUpValidatedReadyEnablesPipeline ==
  \A initialContext \in AdmissibleContextRecords,
     node \in ValidatorIds:
    IndexedCompositionInvariant
      /\ IndexedCatchUpValidatedApplicationReady(initialContext, node)
      => ENABLED
           <<IndexedCatchUpPipelineStep(initialContext, node)>>_(
             IndexedChainVars)
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
              NEW node \in ValidatorIds,
              IndexedCompositionInvariant,
              IndexedCatchUpValidatedApplicationReady(
                initialContext, node)
         PROVE ENABLED
                 <<IndexedCatchUpPipelineStep(initialContext, node)>>_(
                   IndexedChainVars)
    <2>1. ENABLED
             <<IndexedCatchUpApplicationStep(initialContext, node)>>_(
               IndexedChainVars)
      BY <1>1, IndexedCatchUpValidatedReadyEnablesApplication
    <2>2. IndexedCatchUpApplicationStep(initialContext, node)
             => IndexedCatchUpPipelineStep(initialContext, node)
      BY <1>1, IndexedCatchUpApplicationStepRefinesPipelineStep
    <2> QED BY <2>1, <2>2, Isa
  <1> QED BY <1>1

THEOREM IndexedCatchUpIdlePipelineEstablishesReceiptOrAdvance ==
  \A initialContext \in AdmissibleContextRecords,
     node \in ValidatorIds:
    IndexedCompositionInvariant
      /\ IndexedCatchUpIdleDecisionReady(initialContext, node)
      /\ IndexedCatchUpPipelineStep(initialContext, node)
      => \/ HistoricalCatchUpDecisionAt(initialContext, node)'
         \/ nodeHeight'[node] > initialContext.height
BY Isa DEF IndexedCatchUpIdleDecisionReady,
           IndexedCatchUpDecisionReady,
           IndexedCatchUpPipelineStep,
           IndexedHistoricalCatchUpPipelineAction,
           IndexedHistoricalCatchUpDecision,
           IndexedHistoricalCatchUpBodyRecovery,
           IndexedHistoricalCatchUpBodyStore,
           IndexedHistoricalCatchUpValidation,
           IndexedHistoricalCatchUpApplication,
           HistoricalCatchUpDecisionAt, HistoricalCatchUpRecord

THEOREM IndexedCatchUpValidatedPipelineEstablishesAdvance ==
  \A initialContext \in AdmissibleContextRecords,
     node \in ValidatorIds:
    IndexedCompositionInvariant
      /\ IndexedCatchUpValidatedApplicationReady(initialContext, node)
      /\ IndexedCatchUpPipelineStep(initialContext, node)
      => nodeHeight'[node] > initialContext.height
BY IndexedCatchUpApplicationEstablishesAdvance, Isa
   DEF IndexedCatchUpValidatedApplicationReady,
       IndexedCatchUpApplicationReady,
       IndexedCatchUpPipelineStep,
       IndexedHistoricalCatchUpPipelineAction,
       IndexedHistoricalCatchUpDecision,
       IndexedHistoricalCatchUpBodyRecovery,
       IndexedHistoricalCatchUpBodyStore,
       IndexedHistoricalCatchUpValidation,
       IndexedHistoricalCatchUpApplication

(***************************************************************************
These are the two genuinely supported fair suffixes.  They consume the weak
fairness that the specification actually grants to the complete pipeline,
not fictitious fairness for its proof-facing decision/application projections.
The intermediate DecisionRecovered -> BodyRecovered -> Stored -> Validated
rank induction remains explicit below.
***************************************************************************)
THEOREM IndexedCatchUpIdleDecisionFairSuffix ==
  IndexedChainSpec =>
    \A initialContext \in AdmissibleContextRecords,
       node \in ValidatorIds:
      IndexedCatchUpIdleDecisionReady(initialContext, node)
        ~> (\/ HistoricalCatchUpDecisionAt(initialContext, node)
            \/ nodeHeight[node] > initialContext.height)
PROOF
  <1>1. ASSUME IndexedChainSpec,
              NEW initialContext \in AdmissibleContextRecords,
              NEW node \in ValidatorIds
         PROVE
           IndexedCatchUpIdleDecisionReady(initialContext, node)
             ~> (\/ HistoricalCatchUpDecisionAt(initialContext, node)
                 \/ nodeHeight[node] > initialContext.height)
    <2>1. []IndexedCompositionInvariant
      BY <1>1, IndexedChainSpecEstablishesCompositionInvariant, PTL
    <2>2. [][IndexedChainNext]_IndexedChainVars
      BY <1>1, PTL DEF IndexedChainSpec
    <2>3. WF_IndexedChainVars(
             IndexedCatchUpPipelineStep(initialContext, node))
      BY <1>1, PTL DEF IndexedChainSpec, IndexedFairness
    <2>4. IndexedCompositionInvariant
             /\ IndexedCatchUpIdleDecisionReady(initialContext, node)
             => ENABLED
                  <<IndexedCatchUpPipelineStep(
                      initialContext, node)>>_(IndexedChainVars)
      BY <1>1, IndexedCatchUpIdleDecisionReadyEnablesPipeline
    <2>5. IndexedCompositionInvariant
             /\ IndexedCatchUpIdleDecisionReady(initialContext, node)
             /\ IndexedCatchUpPipelineStep(initialContext, node)
             => \/ HistoricalCatchUpDecisionAt(initialContext, node)'
                \/ nodeHeight'[node] > initialContext.height
      BY <1>1, IndexedCatchUpIdlePipelineEstablishesReceiptOrAdvance
    <2>6. IndexedCompositionInvariant
             /\ IndexedCatchUpIdleDecisionReady(initialContext, node)
             /\ [IndexedChainNext]_IndexedChainVars
             => \/ IndexedCatchUpIdleDecisionReady(
                       initialContext, node)'
                \/ HistoricalCatchUpDecisionAt(initialContext, node)'
                \/ nodeHeight'[node] > initialContext.height
      BY IndexedCatchUpDecisionReadyPersistsUntilReceipt
    <2> QED BY <2>1, <2>2, <2>3, <2>4, <2>5, <2>6, PTL
  <1> QED BY <1>1

THEOREM IndexedCatchUpValidatedApplicationFairSuffix ==
  IndexedChainSpec =>
    \A initialContext \in AdmissibleContextRecords,
       node \in ValidatorIds:
      IndexedCatchUpValidatedApplicationReady(initialContext, node)
        ~> nodeHeight[node] > initialContext.height
PROOF
  <1>1. ASSUME IndexedChainSpec,
              NEW initialContext \in AdmissibleContextRecords,
              NEW node \in ValidatorIds
         PROVE IndexedCatchUpValidatedApplicationReady(
                 initialContext, node)
                   ~> nodeHeight[node] > initialContext.height
    <2>1. []IndexedCompositionInvariant
      BY <1>1, IndexedChainSpecEstablishesCompositionInvariant, PTL
    <2>2. [][IndexedChainNext]_IndexedChainVars
      BY <1>1, PTL DEF IndexedChainSpec
    <2>3. WF_IndexedChainVars(
             IndexedCatchUpPipelineStep(initialContext, node))
      BY <1>1, PTL DEF IndexedChainSpec, IndexedFairness
    <2>4. IndexedCompositionInvariant
             /\ IndexedCatchUpValidatedApplicationReady(
                  initialContext, node)
             => ENABLED
                  <<IndexedCatchUpPipelineStep(
                      initialContext, node)>>_(IndexedChainVars)
      BY <1>1, IndexedCatchUpValidatedReadyEnablesPipeline
    <2>5. IndexedCompositionInvariant
             /\ IndexedCatchUpValidatedApplicationReady(
                  initialContext, node)
             /\ IndexedCatchUpPipelineStep(initialContext, node)
             => nodeHeight'[node] > initialContext.height
      BY <1>1, IndexedCatchUpValidatedPipelineEstablishesAdvance
    <2>6. IndexedCompositionInvariant
             /\ IndexedCatchUpValidatedApplicationReady(
                  initialContext, node)
             /\ [IndexedChainNext]_IndexedChainVars
             => \/ IndexedCatchUpValidatedApplicationReady(
                       initialContext, node)'
                \/ nodeHeight'[node] > initialContext.height
      BY <1>1, IndexedCatchUpApplicationReadyPersistsUntilAdvance
    <2> QED BY <2>1, <2>2, <2>3, <2>4, <2>5, <2>6, PTL
  <1> QED BY <1>1

(***************************************************************************
Irreducible local rank debt.  Weak fairness of the union pipeline does not
entail weak fairness of each subaction.  A complete proof must show that every
nonterminal broad readiness state either advances independently or keeps an
exact source and the union action continuously enabled until its finite rank
decreases.  Finiteness and strict rank decrease on actual pipeline steps are
proved above; the stable-enabledness argument and temporal induction through
DecisionRecovered, BodyRecovered, Stored, and Validated remain.  The former
proof incorrectly cited subaction fairness from IndexedFairness and is
intentionally not retained.  IndexedHistoricalCatchUpRankProgress is a premise
of the non-circular height kernel, not a separately declared release theorem;
the already-ledgered HeightLivenessObligation owns this remaining debt.
***************************************************************************)

VerificationOneHeightCompletion ==
  IndexedAsync(VerificationContext)!AsyncSpecAt(VerificationContext)
    => (IndexedCore(VerificationContext, 7)
          ~> IndexedAsync(VerificationContext)!
               AsyncAllResponsiveAppliedAt(VerificationContext))

THEOREM VerificationOneHeightCompletionObligation ==
  VerificationOneHeightCompletion
PROOF
  <1>1. VerificationAsyncProof!OneHeightCompletionObligation
    BY VerificationAsyncProof!OneHeightCompletionObligation
  <1> QED BY <1>1
       DEF VerificationOneHeightCompletion,
           VerificationAsyncProof!OneHeightCompletionLiveness,
           VerificationAsyncProof!AsyncSpecAt,
           VerificationAsyncProof!AsyncAllResponsiveAppliedAt,
           IndexedAsync!AsyncSpecAt,
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
           IndexedHistoricalCatchUpDecision,
           IndexedHistoricalCatchUpApplication,
           NewIndexedDecisionReceipt, NewIndexedApplicationReceipt,
           NoNewIndexedDurableReceipt, IndexedApplications,
           IndexedAsync!AsyncAllResponsiveAppliedAt,
           IndexedAsync!AsyncVotersAt,
           IndexedAsync!NodeHasApplication

THEOREM VerificationFrontierActivatedInstanceEventuallyApplies ==
  /\ IndexedChainSpec
    /\ VerificationOneHeightCompletion
    /\ VerificationContext \in AdmissibleContextRecords
    /\ []~JoinedCanonicalDescendant(VerificationContext)
    => IndexedAllResponsiveJoined(VerificationContext)
         ~> IndexedAsync(VerificationContext)!
               AsyncAllResponsiveAppliedAt(VerificationContext)
PROOF
  <1>1. ASSUME IndexedChainSpec,
              VerificationOneHeightCompletion,
              VerificationContext \in AdmissibleContextRecords,
              []~JoinedCanonicalDescendant(VerificationContext)
         PROVE IndexedAllResponsiveJoined(VerificationContext)
                 ~> IndexedAsync(VerificationContext)!
                       AsyncAllResponsiveAppliedAt(VerificationContext)
    <2>1. IndexedAllResponsiveJoined(VerificationContext)
             /\ [IndexedChainNext]_IndexedChainVars
             => IndexedAllResponsiveJoined(VerificationContext)'
      BY <1>1, IndexedAllResponsiveJoinedIsStable
    <2>2. <>IndexedAllResponsiveJoined(VerificationContext)
             => (TRUE ~> IndexedAllResponsiveJoined(VerificationContext))
      BY <2>1, PTL DEF IndexedChainSpec
    <2>3. (/\ IndexedChainSpec
            /\ TRUE ~> IndexedAllResponsiveJoined(VerificationContext)
            /\ []~JoinedCanonicalDescendant(VerificationContext))
             => IndexedAsync(VerificationContext)!
                  AsyncSpecAt(VerificationContext)
      BY <1>1, IndexedInstanceActivationObligation
    <2>4. IndexedAsync(VerificationContext)!
             AsyncSpecAt(VerificationContext)
             => <>IndexedCore(VerificationContext, 7)
      BY VerificationAsyncProof!AsyncGstEventually
         DEF VerificationAsyncProof!AsyncSpecAt,
             IndexedAsync!AsyncSpecAt, IndexedAsyncStateAt,
             IndexedCore, IndexedRecovery,
             VerificationCore, VerificationScheduler,
             VerificationRecovery
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
  /\ IndexedChainSpec
    /\ VerificationOneHeightCompletion
    /\ VerificationContext \in AdmissibleContextRecords
    => IndexedAllResponsiveJoined(VerificationContext)
         ~> VerificationFrontierEscape
PROOF
  <1>1. ASSUME IndexedChainSpec,
              VerificationOneHeightCompletion,
              VerificationContext \in AdmissibleContextRecords
         PROVE IndexedAllResponsiveJoined(VerificationContext)
                 ~> VerificationFrontierEscape
    <2>1. []IndexedCompositionInvariant
      BY <1>1, IndexedChainSpecEstablishesCompositionInvariant, PTL
    <2>2. IndexedCompositionInvariant
             /\ VerificationFrontierEscape
             /\ [IndexedChainNext]_IndexedChainVars
             => VerificationFrontierEscape'
      BY <1>1, VerificationFrontierEscapeIsStable
    <2>3. <>JoinedCanonicalDescendant(VerificationContext)
             => (IndexedAllResponsiveJoined(VerificationContext)
                   ~> VerificationFrontierEscape)
      BY <1>1, <2>1, <2>2, PTL
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
      => \/ decision \in historicalCatchUpDecisions
         \/ \E sourceContext \in AdmissibleContextRecords:
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

THEOREM IndexedAppliedContextProvidesCatchUpSource ==
  \A initialContext \in AdmissibleContextRecords:
    (/\ IndexedCompositionInvariant
     /\ initialContext.height < MaxHeight
     /\ IndexedAllResponsiveJoined(initialContext)
     /\ IndexedAsync(initialContext)!
          AsyncAllResponsiveAppliedAt(initialContext))
      => \E server \in ValidatorIds,
            source \in Chain!DecisionEvidenceSet:
           HistoricalCatchUpSource(initialContext, server, source)
BY Isa, QuorumIntersectionObligation,
   IndexedResponsiveVoterSetIsNonempty
   DEF IndexedCompositionInvariant,
       IndexedEveryInstanceStrongInvariant,
       IndexedTotalReceiptProjection,
       IndexedDecisionReceiptProjection,
       IndexedApplicationReceiptProjection,
       IndexedDecisionEvidence, IndexedApplicationEvidence,
       IndexedCurrentDecisions, IndexedCurrentApplications,
       IndexedAllResponsiveJoined, HistoricalCatchUpSource,
       IndexedAsync!AsyncAllResponsiveAppliedAt,
       IndexedAsync!AsyncVotersAt,
       IndexedAsync!NodeHasApplication,
       IndexedAsync!StrongInductiveInvariant,
       IndexedAsync!Safety, IndexedAsync!TypeInvariant,
       IndexedAsync!DecisionAgreement,
       IndexedAsync!AppliedRequiresDecision,
       IndexedAsync!ReducerProvenanceInvariant,
       IndexedAsync!CertificatesBackedByIntents,
       IndexedAsync!HistoricalQcValid,
       IndexedAsync!HonestDurableIntentsSound,
       IndexedAsync!HonestIntentSound,
       IndexedAsync!CertificateBackedBy,
       IndexedAsync!VoteBacksCertificate,
       Chain!ChainEpochInvariant, Chain!ChainEpochTypeInvariant,
       Chain!DurableDecisionEvidenceSound,
       Chain!DurableApplicationEvidenceSound,
       Chain!DecisionBacksCertifiedSlot,
       Chain!ReceiptOutsideChainHorizon,
       Chain!DurableCommitDecision,
       Chain!HistoricalCommitCertificate,
       Chain!CanonicalCommitForSlot,
       Chain!DecisionEvidenceSet,
       ModelConfiguration, QuorumConfiguration,
       DualQuorumIntersectionHasHonest, DualQuorum, CountQuorum,
       ContextRecords, Heights

IndexedResponsiveLagAt(initialContext, node) ==
  /\ initialContext.height < MaxHeight
  /\ node \in Responsive
  /\ nodeHeight[node] = initialContext.height

THEOREM IndexedHistoricalDecisionMakesApplicationReadyOrAdvance ==
  \A initialContext \in AdmissibleContextRecords,
     node \in ValidatorIds:
    (/\ IndexedCompositionInvariant
     /\ IndexedResponsiveLagAt(initialContext, node)
     /\ HistoricalCatchUpDecisionAt(initialContext, node))
      => \/ IndexedCatchUpApplicationReady(initialContext, node)
         \/ nodeHeight[node] > initialContext.height
BY Isa DEF IndexedResponsiveLagAt,
           IndexedCatchUpApplicationReady,
           HistoricalCatchUpDecisionAt,
           HistoricalCatchUpApplicationAt,
           HistoricalCatchUpTarget, HistoricalCatchUpSource,
           HistoricalCatchUpRecord,
           IndexedCompositionInvariant,
           HistoricalCatchUpEvidenceInvariant,
           HistoricalCatchUpReceiptSound,
           IndexedTotalReceiptProjection,
           IndexedDecisionReceiptProjection,
           IndexedApplicationReceiptProjection,
           IndexedDecisionEvidence, IndexedApplicationEvidence,
           IndexedCurrentDecisions, IndexedCurrentApplications,
           IndexedProjectedNodeHasApplication,
           CatchUpNodeHasApplicationProjection,
           Chain!ChainEpochInvariant, Chain!ChainEpochTypeInvariant,
           Chain!ContextsMatchLocalHistories,
           IndexedAsync!AsyncVotersAt

THEOREM IndexedCatchUpReadinessAdvancesResponsiveNode ==
  /\ IndexedChainSpec
  /\ IndexedHistoricalCatchUpRankProgress
  =>
    \A initialContext \in AdmissibleContextRecords,
       node \in Responsive:
      initialContext.height < MaxHeight
        => (\/ IndexedCatchUpDecisionReady(initialContext, node)
            \/ IndexedCatchUpApplicationReady(initialContext, node))
             ~> nodeHeight[node] > initialContext.height
PROOF
  <1>1. ASSUME IndexedChainSpec,
              IndexedHistoricalCatchUpRankProgress,
              NEW initialContext \in AdmissibleContextRecords,
              NEW node \in Responsive,
              initialContext.height < MaxHeight
         PROVE (\/ IndexedCatchUpDecisionReady(initialContext, node)
                \/ IndexedCatchUpApplicationReady(initialContext, node))
                 ~> nodeHeight[node] > initialContext.height
    <2>1. (initialContext.height < MaxHeight
              /\ IndexedCatchUpDecisionReady(initialContext, node))
             ~> nodeHeight[node] > initialContext.height
      BY <1>1 DEF IndexedHistoricalCatchUpRankProgress
    <2>2. (initialContext.height < MaxHeight
              /\ IndexedCatchUpApplicationReady(initialContext, node))
             ~> nodeHeight[node] > initialContext.height
      BY <1>1 DEF IndexedHistoricalCatchUpRankProgress
    <2>3. [](initialContext.height < MaxHeight)
      BY <1>1, PTL
    <2> QED BY <2>1, <2>2, <2>3, PTL
  <1> QED BY <1>1

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

THEOREM IndexedReachedAncestorHasEveryResponsiveVoterJoined ==
  \A targetContext \in AdmissibleContextRecords:
    \A blockHeight \in 0..targetContext.height:
      IndexedCompositionInvariant
        /\ IndexedJoinedThroughLocalHeight
        /\ IndexedTargetJoined(targetContext)
        /\ IndexedResponsiveHeightReached(blockHeight)
        => IndexedAllResponsiveJoined(
             IndexedAncestorContext(targetContext, blockHeight))
BY Isa, IndexedJoinedTargetIdentifiesEveryCanonicalAncestor
   DEF IndexedJoinedThroughLocalHeight,
       IndexedResponsiveHeightReached,
       IndexedAllResponsiveJoined,
       IndexedAsync!AsyncVotersAt,
       ModelConfiguration, ValidatorIds, Heights

THEOREM IndexedJoinedReachedTargetProvidesAncestorCatchUpSource ==
  \A targetContext \in AdmissibleContextRecords:
    \A blockHeight \in 0..targetContext.height:
      IndexedCompositionInvariant
        /\ IndexedJoinedThroughLocalHeight
        /\ IndexedTargetJoined(targetContext)
        /\ IndexedResponsiveHeightReached(blockHeight)
        /\ blockHeight < targetContext.height
        => \E server \in ValidatorIds,
              source \in Chain!DecisionEvidenceSet:
             HistoricalCatchUpSource(
               IndexedAncestorContext(targetContext, blockHeight),
               server, source)
BY Isa, QuorumIntersectionObligation,
   IndexedJoinedTargetIdentifiesEveryCanonicalAncestor,
   IndexedReachedAncestorHasEveryResponsiveVoterJoined
   DEF IndexedCompositionInvariant,
       IndexedEveryInstanceStrongInvariant,
       IndexedTotalReceiptProjection,
       IndexedDecisionReceiptProjection,
       IndexedApplicationReceiptProjection,
       IndexedDecisionEvidence, IndexedApplicationEvidence,
       IndexedCurrentDecisions, IndexedCurrentApplications,
       IndexedJoinedThroughLocalHeight,
       JoinedContextCertificationInvariant, JoinedRoutingInvariant,
       IndexedTargetJoined, JoinedContexts,
       IndexedAncestorContext, CanonicalIndexedContext,
       IndexedResponsiveHeightReached,
       IndexedAllResponsiveJoined,
       HistoricalCatchUpSource, HistoricalCatchUpApplicationAt,
       HistoricalCatchUpEvidenceInvariant,
       HistoricalCatchUpReceiptSound, HistoricalCatchUpRecord,
       IndexedAsync!StrongInductiveInvariant,
       IndexedAsync!Safety, IndexedAsync!TypeInvariant,
       IndexedAsync!DecisionAgreement,
       IndexedAsync!AppliedRequiresDecision,
       IndexedAsync!ReducerProvenanceInvariant,
       IndexedAsync!CertificatesBackedByIntents,
       IndexedAsync!HistoricalQcValid,
       IndexedAsync!HonestDurableIntentsSound,
       IndexedAsync!HonestIntentSound,
       IndexedAsync!CertificateBackedBy,
       IndexedAsync!VoteBacksCertificate,
       IndexedAsync!AsyncVotersAt,
       IndexedAsync!NodeHasApplication,
       Chain!ChainEpochInvariant, Chain!ChainEpochTypeInvariant,
       Chain!ContextsMatchLocalHistories,
       Chain!DurableDecisionEvidenceSound,
       Chain!DurableApplicationEvidenceSound,
       Chain!DecisionBacksCertifiedSlot,
       Chain!ReceiptOutsideChainHorizon,
       Chain!DurableCommitDecision,
       Chain!HistoricalCommitCertificate,
       Chain!CanonicalCommitForSlot,
       Chain!DecisionEvidenceSet,
       ModelConfiguration, QuorumConfiguration,
       DualQuorumIntersectionHasHonest, DualQuorum, CountQuorum,
       AdmissibleContextRecords, FrozenContextAdmissible,
       ContextRecords, LineagesAt, Heights

IndexedTargetHeightStepPremise(targetContext, blockHeight) ==
  /\ IndexedTargetJoined(targetContext)
  /\ IndexedResponsiveHeightReached(blockHeight)

THEOREM IndexedTargetStepEitherPassedOrCatchUpReady ==
  \A targetContext \in AdmissibleContextRecords:
    \A blockHeight \in 0..targetContext.height:
      \A node \in Responsive:
        IndexedCompositionInvariant
          /\ IndexedJoinedThroughLocalHeight
          /\ IndexedTargetHeightStepPremise(targetContext, blockHeight)
          /\ blockHeight < targetContext.height
          => \/ IndexedNodePastContext(
                  IndexedAncestorContext(targetContext, blockHeight), node)
             \/ IndexedCatchUpDecisionReady(
                  IndexedAncestorContext(targetContext, blockHeight), node)
             \/ IndexedCatchUpApplicationReady(
                  IndexedAncestorContext(targetContext, blockHeight), node)
BY Isa, IndexedJoinedTargetIdentifiesEveryCanonicalAncestor,
   IndexedJoinedReachedTargetProvidesAncestorCatchUpSource
   DEF IndexedTargetHeightStepPremise,
       IndexedResponsiveHeightReached, IndexedNodePastContext,
       IndexedResponsiveLagAt,
       IndexedCatchUpDecisionReady, IndexedCatchUpApplicationReady,
       HistoricalCatchUpTarget, HistoricalCatchUpSource,
       HistoricalCatchUpDecisionAt, HistoricalCatchUpApplicationAt,
       HistoricalCatchUpRecord,
       IndexedCompositionInvariant,
       IndexedApplicationsRespectNodeHeight,
       HistoricalCatchUpEvidenceInvariant,
       HistoricalCatchUpReceiptSound,
       IndexedTotalReceiptProjection,
       IndexedDecisionReceiptProjection,
       IndexedApplicationReceiptProjection,
       IndexedDecisionEvidence, IndexedApplicationEvidence,
       IndexedCurrentDecisions, IndexedCurrentApplications,
       IndexedProjectedNodeHasApplication,
       CatchUpNodeHasApplicationProjection,
       IndexedAncestorContext, CanonicalIndexedContext,
       Chain!ChainEpochInvariant, Chain!ChainEpochTypeInvariant,
       Chain!ContextsMatchLocalHistories,
       IndexedAsync!NodeHasApplication

THEOREM IndexedAdvanceReadyEitherPassedOrNeedsCatchUp ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    IndexedCompositionInvariant
      /\ IndexedJoinedThroughLocalHeight
      /\ IndexedContextAdvanceReady(initialContext)
      => \/ IndexedNodePastContext(initialContext, node)
         \/ IndexedCatchUpDecisionReady(initialContext, node)
         \/ IndexedCatchUpApplicationReady(initialContext, node)
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
              NEW node \in Responsive,
              IndexedCompositionInvariant,
              IndexedJoinedThroughLocalHeight,
              IndexedContextAdvanceReady(initialContext)
         PROVE \/ IndexedNodePastContext(initialContext, node)
               \/ IndexedCatchUpDecisionReady(initialContext, node)
               \/ IndexedCatchUpApplicationReady(initialContext, node)
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
             \/ IndexedCatchUpDecisionReady(
                  IndexedAncestorContext(
                    descendantContext, initialContext.height), node)
             \/ IndexedCatchUpApplicationReady(
                  IndexedAncestorContext(
                    descendantContext, initialContext.height), node)
      BY <1>1, <2>1, <2>2, <2>3, <2>7,
         IndexedTargetStepEitherPassedOrCatchUpReady
    <2> QED BY <2>6, <2>8
  <1> QED BY <1>1

THEOREM IndexedAdvanceReadyEventuallyPassesEachResponsiveNode ==
  /\ IndexedChainSpec
  /\ IndexedHistoricalCatchUpRankProgress
  =>
    \A initialContext \in AdmissibleContextRecords,
       node \in Responsive:
      IndexedContextAdvanceReady(initialContext)
        ~> IndexedNodePastContext(initialContext, node)
PROOF
  <1>1. ASSUME IndexedChainSpec,
              IndexedHistoricalCatchUpRankProgress,
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
                \/ IndexedCatchUpDecisionReady(initialContext, node)
                \/ IndexedCatchUpApplicationReady(initialContext, node)
      BY <1>1, IndexedAdvanceReadyEitherPassedOrNeedsCatchUp
    <2>4. initialContext.height < MaxHeight
      BY <1>1, JoinedCanonicalDescendantStaysWithinHorizon
         DEF IndexedContextAdvanceReady
    <2>5. (\/ IndexedCatchUpDecisionReady(initialContext, node)
            \/ IndexedCatchUpApplicationReady(initialContext, node))
             ~> IndexedNodePastContext(initialContext, node)
      BY <1>1, <2>4,
         IndexedCatchUpReadinessAdvancesResponsiveNode
         DEF IndexedNodePastContext
    <2> QED BY <2>1, <2>2, <2>3, <2>5, PTL
  <1> QED BY <1>1

THEOREM IndexedAdvanceReadyPassesEveryFiniteResponsivePrefix ==
  /\ IndexedChainSpec
  /\ IndexedHistoricalCatchUpRankProgress
  =>
    \A initialContext \in AdmissibleContextRecords,
       limit \in Nat:
      IndexedContextAdvanceReady(initialContext)
        ~> IndexedResponsivePrefixPast(initialContext, limit)
PROOF
  <1>1. ASSUME IndexedChainSpec,
              IndexedHistoricalCatchUpRankProgress,
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
  /\ IndexedHistoricalCatchUpRankProgress
  =>
    \A initialContext \in AdmissibleContextRecords:
      IndexedContextAdvanceReady(initialContext)
        ~> IndexedResponsiveHeightReached(initialContext.height + 1)
PROOF
  <1>1. ASSUME IndexedChainSpec,
              IndexedHistoricalCatchUpRankProgress,
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
  /\ IndexedHistoricalCatchUpRankProgress
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
              IndexedHistoricalCatchUpRankProgress,
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
                \/ IndexedCatchUpDecisionReady(
                     IndexedAncestorContext(targetContext, blockHeight),
                     node)
                \/ IndexedCatchUpApplicationReady(
                     IndexedAncestorContext(targetContext, blockHeight),
                     node)
      BY <1>1, IndexedTargetStepEitherPassedOrCatchUpReady
    <2>4. IndexedAncestorContext(targetContext, blockHeight).height
             < MaxHeight
      BY <1>1, IndexedAdmissibleTargetHasAdmissibleAncestors,
         Isa DEF IndexedAncestorContext,
                 AdmissibleContextRecords, FrozenContextAdmissible,
                 ContextRecords, Heights
    <2>5. (\/ IndexedCatchUpDecisionReady(
                   IndexedAncestorContext(targetContext, blockHeight), node)
            \/ IndexedCatchUpApplicationReady(
                   IndexedAncestorContext(targetContext, blockHeight), node))
             ~> IndexedNodePastContext(
                  IndexedAncestorContext(targetContext, blockHeight), node)
      BY <1>1, <2>4,
         IndexedAdmissibleTargetHasAdmissibleAncestors,
         IndexedCatchUpReadinessAdvancesResponsiveNode
         DEF IndexedNodePastContext
    <2> QED BY <2>1, <2>2, <2>3, <2>5, PTL
  <1> QED BY <1>1

THEOREM IndexedTargetStepPassesEveryFiniteResponsivePrefix ==
  /\ IndexedChainSpec
  /\ IndexedHistoricalCatchUpRankProgress
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
              IndexedHistoricalCatchUpRankProgress,
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
  /\ IndexedHistoricalCatchUpRankProgress
  =>
    \A targetContext \in AdmissibleContextRecords:
      \A blockHeight \in 0..targetContext.height:
        blockHeight < targetContext.height
          => (IndexedTargetJoined(targetContext)
                /\ IndexedResponsiveHeightReached(blockHeight))
               ~> IndexedResponsiveHeightReached(blockHeight + 1)
PROOF
  <1>1. ASSUME IndexedChainSpec,
              IndexedHistoricalCatchUpRankProgress,
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
  /\ IndexedHistoricalCatchUpRankProgress
  =>
    \A targetContext \in AdmissibleContextRecords:
      \A blockHeight \in 0..targetContext.height:
        IndexedTargetJoined(targetContext)
          ~> IndexedResponsiveHeightReached(blockHeight)
PROOF
  <1>1. ASSUME IndexedChainSpec,
              IndexedHistoricalCatchUpRankProgress,
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

IndexedContextCompleted(initialContext) ==
  IF initialContext.height = MaxHeight
  THEN IndexedAsync(initialContext)!
         AsyncAllResponsiveAppliedAt(initialContext)
  ELSE \A node \in IndexedAsync(initialContext)!
                    AsyncVotersAt(initialContext):
         nodeHeight[node] > initialContext.height

THEOREM IndexedAllAppliedImpliesContextCompleted ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedCompositionInvariant
      /\ IndexedAsync(initialContext)!
           AsyncAllResponsiveAppliedAt(initialContext)
      => IndexedContextCompleted(initialContext)
BY Isa DEF IndexedCompositionInvariant,
           IndexedApplicationsRespectNodeHeight,
           IndexedContextCompleted,
           IndexedAsync!AsyncAllResponsiveAppliedAt

THEOREM IndexedContextCompletedIsStable ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedContextCompleted(initialContext)
      /\ [IndexedChainNext]_IndexedChainVars
      => IndexedContextCompleted(initialContext)'
BY Isa, IndexedAllResponsiveAppliedIsStable,
   IndexedBracketStepKeepsNodeHeightsMonotone
   DEF IndexedContextCompleted, IndexedAsync!AsyncVotersAt,
       ModelConfiguration, ValidatorIds, Heights,
       AdmissibleContextRecords, FrozenContextAdmissible,
       ContextRecords

THEOREM VerificationSuccessorHeightImpliesContextCompleted ==
  VerificationContext \in AdmissibleContextRecords
    /\ VerificationContext.height < MaxHeight
    /\ IndexedResponsiveHeightReached(VerificationContext.height + 1)
    => IndexedContextCompleted(VerificationContext)
BY Isa DEF IndexedResponsiveHeightReached,
           IndexedContextCompleted, IndexedAsync!AsyncVotersAt,
           ModelConfiguration, ValidatorIds,
           AdmissibleContextRecords, FrozenContextAdmissible,
           ContextRecords, Heights

THEOREM VerificationReachedEscapeEitherCompletesOrAdvances ==
  IndexedCompositionInvariant
    /\ VerificationContext \in AdmissibleContextRecords
    /\ IndexedTargetJoined(VerificationContext)
    /\ IndexedResponsiveHeightReached(VerificationContext.height)
    /\ VerificationFrontierEscape
    => \/ IndexedContextCompleted(VerificationContext)
       \/ IndexedContextAdvanceReady(VerificationContext)
BY Isa, IndexedAllAppliedImpliesContextCompleted,
   JoinedCanonicalDescendantBoundsImmediateSuccessor
   DEF VerificationFrontierEscape, IndexedContextAdvanceReady,
       IndexedTargetJoined

THEOREM VerificationAdvanceReadyEventuallyCompletes ==
  /\ IndexedChainSpec
    /\ IndexedHistoricalCatchUpRankProgress
    /\ VerificationContext \in AdmissibleContextRecords
    => IndexedContextAdvanceReady(VerificationContext)
         ~> IndexedContextCompleted(VerificationContext)
PROOF
  <1>1. ASSUME IndexedChainSpec,
              IndexedHistoricalCatchUpRankProgress,
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
    /\ IndexedHistoricalCatchUpRankProgress
    /\ VerificationContext \in AdmissibleContextRecords
    => (/\ IndexedTargetJoined(VerificationContext)
        /\ IndexedResponsiveHeightReached(VerificationContext.height)
        /\ VerificationFrontierEscape)
         ~> IndexedContextCompleted(VerificationContext)
PROOF
  <1>1. ASSUME IndexedChainSpec,
              IndexedHistoricalCatchUpRankProgress,
              VerificationContext \in AdmissibleContextRecords
         PROVE (/\ IndexedTargetJoined(VerificationContext)
                /\ IndexedResponsiveHeightReached(
                     VerificationContext.height)
                /\ VerificationFrontierEscape)
                 ~> IndexedContextCompleted(VerificationContext)
    <2>1. []IndexedCompositionInvariant
      BY <1>1, IndexedChainSpecEstablishesCompositionInvariant, PTL
    <2>2. IndexedCompositionInvariant
             /\ IndexedTargetJoined(VerificationContext)
             /\ IndexedResponsiveHeightReached(
                  VerificationContext.height)
             /\ VerificationFrontierEscape
             => \/ IndexedContextCompleted(VerificationContext)
                \/ IndexedContextAdvanceReady(VerificationContext)
      BY <1>1, VerificationReachedEscapeEitherCompletesOrAdvances
    <2>3. IndexedContextAdvanceReady(VerificationContext)
             ~> IndexedContextCompleted(VerificationContext)
      BY <1>1, VerificationAdvanceReadyEventuallyCompletes
    <2> QED BY <2>1, <2>2, <2>3, PTL
  <1> QED BY <1>1

THEOREM VerificationJoinedTargetEventuallyReachesAndEscapes ==
  /\ IndexedChainSpec
    /\ IndexedHistoricalCatchUpRankProgress
    /\ VerificationOneHeightCompletion
    /\ VerificationContext \in AdmissibleContextRecords
    => IndexedTargetJoined(VerificationContext)
         ~> (/\ IndexedTargetJoined(VerificationContext)
             /\ IndexedResponsiveHeightReached(
                  VerificationContext.height)
             /\ VerificationFrontierEscape)
PROOF
  <1>1. ASSUME IndexedChainSpec,
              IndexedHistoricalCatchUpRankProgress,
              VerificationOneHeightCompletion,
              VerificationContext \in AdmissibleContextRecords
         PROVE IndexedTargetJoined(VerificationContext)
                 ~> (/\ IndexedTargetJoined(VerificationContext)
                     /\ IndexedResponsiveHeightReached(
                          VerificationContext.height)
                     /\ VerificationFrontierEscape)
    <2>1. []IndexedCompositionInvariant
      BY <1>1, IndexedChainSpecEstablishesCompositionInvariant, PTL
    <2>2. []IndexedJoinedThroughLocalHeight
      BY <1>1, IndexedChainSpecJoinsEveryNodeThroughLocalHeight, PTL
    <2>3. IndexedTargetJoined(VerificationContext)
             ~> IndexedResponsiveHeightReached(
                  VerificationContext.height)
      BY <1>1, IndexedJoinedTargetEventuallyReachesEveryAncestorHeight,
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
    <2>6. IndexedCompositionInvariant
             /\ IndexedJoinedThroughLocalHeight
             /\ IndexedTargetJoined(VerificationContext)
             /\ IndexedResponsiveHeightReached(
                  VerificationContext.height)
             => IndexedAllResponsiveJoined(VerificationContext)
      BY <1>1, IndexedReachedAncestorHasEveryResponsiveVoterJoined,
         Isa DEF IndexedAncestorContext
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

TerminalHistoricalCatchUpBoundaryInvariant ==
  \A terminalContext \in AdmissibleContextRecords,
     node \in ValidatorIds:
    terminalContext.height = MaxHeight
      /\ HistoricalCatchUpApplicationAt(terminalContext, node)
      => /\ nodeHeight[node] = terminalContext.height
         /\ nodeContext[node] = terminalContext
         /\ historicalCatchUpStage[terminalContext][node] = "Applied"
         /\ successorActivationStatus[terminalContext][node] = "Idle"
         /\ successorPredecessorStatusOwnership[terminalContext][node]
              = "Absent"

THEOREM IndexedChainAlwaysPreservesTerminalCatchUpBoundary ==
  IndexedChainSpec => []TerminalHistoricalCatchUpBoundaryInvariant
PROOF
  <1>1. IndexedChainSpec => []IndexedCompositionInvariant
    BY IndexedChainSpecEstablishesCompositionInvariant
  <1>2. IndexedChainSpec => []TerminalSuccessorActivationExcluded
    BY IndexedChainAlwaysExcludesTerminalActivation
  <1>3. IndexedCompositionInvariant
           /\ TerminalSuccessorActivationExcluded
           => TerminalHistoricalCatchUpBoundaryInvariant
    BY Isa DEF IndexedCompositionInvariant,
               HistoricalCatchUpEvidenceInvariant,
               TerminalSuccessorActivationExcluded,
               TerminalHistoricalCatchUpBoundaryInvariant
  <1> QED BY <1>1, <1>2, <1>3, PTL

SuccessorActivationAndHistoricalCatchUpProductionRefinementInvariant ==
  /\ SuccessorActivationShape
  /\ HistoricalCatchUpShape
  /\ \A parentContext \in AdmissibleContextRecords,
       node \in ValidatorIds:
       SuccessorHeightActivated(parentContext, node)
         => /\ parentContext.height < MaxHeight
            /\ node \in joinedByContext[
                         CanonicalIndexedContext(
                           parentContext.height + 1)]
            /\ successorPredecessorStatusOwnership[parentContext][node]
                 = "Absent"
  /\ \A terminalContext \in AdmissibleContextRecords,
       node \in ValidatorIds:
       terminalContext.height = MaxHeight
         /\ historicalCatchUpStage[terminalContext][node] = "Applied"
         => /\ nodeHeight[node] = terminalContext.height
            /\ nodeContext[node] = terminalContext
            /\ successorActivationStatus[terminalContext][node] = "Idle"
            /\ successorPredecessorStatusOwnership[terminalContext][node]
                 = "Absent"

(***************************************************************************
This is the deliberately explicit Rust-to-TLA refinement seam. Its discharge
must connect the production open_deferred_status adapter, serialized runtime,
effect executor, service startup, startup/recovery effect consumption, clock
arming, exact marker preparation, authenticated ingress opening, and final
Applied/Recovered publication to the ordered actions above. It also must map
block-sync recovery through every historical stage and preserve terminal
observer application without inventing a successor. The ledger keeps this
obligation unproved until that trace mapping is machine checked.  The model-
internal activated-state projection and terminal exclusion are proved above;
they do not by themselves establish that a Rust execution refines these TLA+
actions.  In particular, this declaration remains the external trace sentinel
rather than being discharged from the state-side invariants alone.
***************************************************************************)
THEOREM SuccessorActivationAndHistoricalCatchUpProductionRefinementObligation ==
  IndexedChainSpec
    => []SuccessorActivationAndHistoricalCatchUpProductionRefinementInvariant

(***************************************************************************
Exact indexed multi-height release theorem for the arbitrary free
VerificationContext. Natural induction catches every responsive node through
its canonical ancestors using only authenticated historical decisions. At the
target frontier, either the fixed one-height instance applies or a higher
canonical context becomes joined; in the latter case the same fair catch-up
actions move every lagging target voter past the target.
***************************************************************************)
THEOREM HeightLivenessFromOneHeightAndCatchUpProgress ==
  /\ IndexedChainSpec
  /\ IndexedHistoricalCatchUpRankProgress
  /\ VerificationOneHeightCompletion
  => IndexedHeightLivenessProperty
PROOF
  <1>1. ASSUME IndexedChainSpec,
              IndexedHistoricalCatchUpRankProgress,
              VerificationOneHeightCompletion
         PROVE IndexedHeightLivenessProperty
    <2>1. CASE VerificationContext \in AdmissibleContextRecords
      <3>1. IndexedTargetJoined(VerificationContext)
               ~> (/\ IndexedTargetJoined(VerificationContext)
                   /\ IndexedResponsiveHeightReached(
                        VerificationContext.height)
                   /\ VerificationFrontierEscape)
        BY <1>1, <2>1,
           VerificationJoinedTargetEventuallyReachesAndEscapes
      <3>2. (/\ IndexedTargetJoined(VerificationContext)
              /\ IndexedResponsiveHeightReached(
                   VerificationContext.height)
              /\ VerificationFrontierEscape)
               ~> IndexedContextCompleted(VerificationContext)
        BY <1>1, <2>1,
           VerificationReachedEscapeEventuallyCompletes
      <3> QED BY <3>1, <3>2, PTL
           DEF IndexedHeightLivenessProperty, IndexedTargetJoined
    <2>2. CASE VerificationContext \notin AdmissibleContextRecords
      BY <2>2 DEF IndexedHeightLivenessProperty
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

(***************************************************************************
Release-facing declaration.  The finite validator and finite-height
inductions above are contained in the conditional kernel and do not import an
unproved theorem.  This already-ledgered declaration owns its two remaining
temporal debts: exact one-height completion and the middle-stage historical
catch-up rank proof.  It remains proofless rather than manufacturing a second
unledgered theorem for the rank premise.
***************************************************************************)
THEOREM HeightLivenessObligation ==
  IndexedChainSpec => IndexedHeightLivenessProperty


=============================================================================
