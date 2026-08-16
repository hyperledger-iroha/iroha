---- MODULE SumeragiV2ChainEpochRefinementShard02 ----
EXTENDS SumeragiV2ChainEpochRefinementShard01

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

=============================================================================
