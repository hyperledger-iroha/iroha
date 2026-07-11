---- MODULE SumeragiV2ChainEpochProofs ----
EXTENDS SumeragiV2ChainEpoch, TLAPS

THEOREM GenesisContextIsAContextRecord ==
  ModelConfiguration => ContextRecord(0, <<>>) \in ContextRecords
PROOF
  <1>1. ASSUME ModelConfiguration
         PROVE ContextRecord(0, <<>>) \in ContextRecords
    <2>1. 0 \in Heights
      BY <1>1, SMT DEF ModelConfiguration, Heights
    <2>2. <<>> \in LineagesAt(0)
      BY Isa DEF LineagesAt
    <2> QED BY <2>1, <2>2 DEF ContextRecords
  <1> QED BY <1>1

THEOREM GenesisEstablishesChainEpochInvariant ==
  ChainEpochInit => ChainEpochInvariant
PROOF
  <1>1. ASSUME ChainEpochInit
         PROVE ChainEpochInvariant
    <2>1. ModelConfiguration
      BY <1>1 DEF ChainEpochInit, Init
    <2>2. ContextRecord(0, <<>>) \in ContextRecords
      BY <2>1, GenesisContextIsAContextRecord
    <2>3. ChainEpochTypeInvariant
      BY <1>1, <2>1, <2>2, IsaT(120)
         DEF ChainEpochInit, ChainEpochTypeInvariant, DecisionMapSet,
             DecisionSlots, Heights
    <2>4. /\ CertifiedPrefixValid
          /\ NodesDoNotOutrunCertificates
          /\ ContextsMatchLocalHistories
          /\ HistoryPrefixComparable
          /\ PerNodeFrozenEpoch
          /\ PerNodeParentFinality
          /\ ForeignLineageRejected
      BY <1>1, <2>1, IsaMT("blast", 120)
         DEF ChainEpochInit, CertifiedPrefixValid,
             NodesDoNotOutrunCertificates, ContextsMatchLocalHistories,
             HistoryPrefixComparable, PerNodeFrozenEpoch,
             PerNodeParentFinality, ForeignLineageRejected,
             CanApplyCertifiedLineage, HistoryThrough, ContextRecord,
             ParentFinalityIdentity, ExpectedEpoch
    <2> QED BY <2>3, <2>4 DEF ChainEpochInvariant
  <1> QED BY <1>1

THEOREM CertifyPreservesChainEpochInvariant ==
  \A subject \in ValidSubjects:
    ChainEpochInvariant /\ CertifyNextSubject(subject)
      => ChainEpochInvariant'
PROOF
  <1>1. ASSUME NEW subject \in ValidSubjects,
              ChainEpochInvariant,
              CertifyNextSubject(subject)
         PROVE ChainEpochInvariant'
    <2>1. ChainEpochTypeInvariant'
      BY <1>1, IsaT(120)
         DEF ChainEpochInvariant, ChainEpochTypeInvariant,
             CertifyNextSubject, DecisionMapSet, DecisionSlots, Heights
    <2>2. CertifiedPrefixValid'
      BY <1>1, SMTT(60)
         DEF ChainEpochInvariant, CertifiedPrefixValid,
             CertifyNextSubject
    <2>3. /\ NodesDoNotOutrunCertificates'
          /\ ContextsMatchLocalHistories'
          /\ HistoryPrefixComparable'
          /\ PerNodeFrozenEpoch'
          /\ PerNodeParentFinality'
          /\ ForeignLineageRejected'
      BY <1>1, IsaMT("blast", 120)
         DEF ChainEpochInvariant, CertifyNextSubject,
             NodesDoNotOutrunCertificates, ContextsMatchLocalHistories,
             HistoryPrefixComparable, PerNodeFrozenEpoch,
             PerNodeParentFinality, ForeignLineageRejected,
             CanApplyCertifiedLineage, HistoryThrough
    <2> QED BY <2>1, <2>2, <2>3 DEF ChainEpochInvariant
  <1> QED BY <1>1

THEOREM ApplyPreservesChainEpochInvariant ==
  \A node \in Honest:
    ChainEpochInvariant /\ ApplyCertifiedNext(node)
      => ChainEpochInvariant'
PROOF
  <1>1. ASSUME NEW node \in Honest,
              ChainEpochInvariant,
              ApplyCertifiedNext(node)
         PROVE ChainEpochInvariant'
    <2>1. ChainEpochTypeInvariant'
      BY <1>1, IsaT(120)
         DEF ChainEpochInvariant, ChainEpochTypeInvariant,
             ApplyCertifiedNext, Heights
    <2>2. /\ CertifiedPrefixValid'
          /\ NodesDoNotOutrunCertificates'
      BY <1>1, SMTT(60)
         DEF ChainEpochInvariant, CertifiedPrefixValid,
             NodesDoNotOutrunCertificates, ApplyCertifiedNext
    <2>3. ContextsMatchLocalHistories'
      BY <1>1, IsaT(120)
         DEF ChainEpochInvariant, ContextsMatchLocalHistories,
             ApplyCertifiedNext, HistoryThrough
    <2>4. HistoryPrefixComparable'
      BY <1>1, SMTT(60)
         DEF ChainEpochInvariant, HistoryPrefixComparable,
             ApplyCertifiedNext, HistoryThrough
    <2>5. /\ PerNodeFrozenEpoch'
          /\ PerNodeParentFinality'
      BY <1>1, IsaMT("blast", 120)
         DEF ChainEpochInvariant, PerNodeFrozenEpoch,
             PerNodeParentFinality, ApplyCertifiedNext, HistoryThrough,
             ContextRecord, ParentFinalityIdentity, ParentContextKey,
             ExpectedEpoch
    <2>6. ForeignLineageRejected'
      BY SMT DEF ForeignLineageRejected, CanApplyCertifiedLineage
    <2> QED BY <2>1, <2>2, <2>3, <2>4, <2>5, <2>6
       DEF ChainEpochInvariant
  <1> QED BY <1>1

THEOREM LaggingNodesRemainUnchanged ==
  \A node \in Honest:
    ApplyCertifiedNext(node)
      => \A other \in ValidatorIds \ {node}:
           /\ nodeHeight'[other] = nodeHeight[other]
           /\ nodeContext'[other] = nodeContext[other]
BY SMTT(60) DEF ApplyCertifiedNext

THEOREM ChainEpochInductiveStep ==
  ChainEpochInvariant /\ [ChainEpochNext]_ChainEpochAllVars
    => ChainEpochInvariant'
PROOF
  <1>1. ASSUME ChainEpochInvariant,
              [ChainEpochNext]_ChainEpochAllVars
         PROVE ChainEpochInvariant'
    <2>1. CASE UNCHANGED ChainEpochAllVars
      BY <1>1, IsaT(120)
         DEF ChainEpochInvariant, ChainEpochAllVars, ChainEpochVars
    <2>2. CASE ChainEpochNext
      <3>1. CASE \E subject \in ValidSubjects:
                    CertifyNextSubject(subject)
        BY <1>1, <3>1, CertifyPreservesChainEpochInvariant
      <3>2. CASE \E node \in Honest: ApplyCertifiedNext(node)
        BY <1>1, <3>2, ApplyPreservesChainEpochInvariant
      <3> QED BY <1>1, <3>1, <3>2 DEF ChainEpochNext
    <2> QED BY <1>1, <2>1, <2>2
  <1> QED BY <1>1

THEOREM ChainPrefixAndEpochSafety ==
  ChainEpochSpec => []ChainEpochInvariant
PROOF
  <1>1. ChainEpochInit => ChainEpochInvariant
    BY GenesisEstablishesChainEpochInvariant
  <1>2. ChainEpochInvariant /\ [ChainEpochNext]_ChainEpochAllVars
           => ChainEpochInvariant'
    BY ChainEpochInductiveStep
  <1> QED BY <1>1, <1>2, PTL DEF ChainEpochSpec

=============================================================================
