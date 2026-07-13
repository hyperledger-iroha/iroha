---- MODULE SumeragiV2ChainEpochRefinement ----
EXTENDS SumeragiV2AsyncNetwork, TLAPS

(***************************************************************************
Refinement from the production asynchronous corridor to the per-node
chain/epoch model.

The abstract ChainEpoch variables are mapped to the auxiliary observations in
SumeragiV2AsyncNetwork.  The abstract action still requires `UNCHANGED vars`:
recording actions satisfy that requirement, while production protocol actions
stutter the four observable ChainEpoch variables.  Consequently the projected
specification deliberately uses ChainEpochVars, not ChainEpochAllVars; mutable
transport and reducer state is hidden by the refinement.
***************************************************************************)

Chain == INSTANCE SumeragiV2ChainEpoch
  WITH certifiedHeight <- asyncCertifiedHeight,
       decidedAt <- asyncDecidedAt,
       nodeHeight <- asyncNodeHeight,
       nodeContext <- asyncNodeContext

ChainProof == INSTANCE SumeragiV2ChainEpochProofs
  WITH certifiedHeight <- asyncCertifiedHeight,
       decidedAt <- asyncDecidedAt,
       nodeHeight <- asyncNodeHeight,
       nodeContext <- asyncNodeContext

ProjectedChainEpochSpec ==
  Chain!ChainEpochInit
    /\ [][Chain!ChainEpochNext]_Chain!ChainEpochVars

THEOREM AsyncInitRefinesChainEpochInit ==
  AsyncInit => Chain!ChainEpochInit
BY DEF AsyncInit, Chain!ChainEpochInit

THEOREM RecordCertifiedRefinesCertifyNextSubject ==
  \A qc \in QcRecordSet:
    RecordCertifiedNext(qc)
      => Chain!CertifyNextSubject(qc.subject)
BY DEF RecordCertifiedNext, AsyncResponsiveAppliedCertifiedPrefix,
       Chain!CertifyNextSubject, Chain!ResponsiveAppliedCertifiedPrefix

THEOREM RecordAppliedRefinesApplyCertifiedNext ==
  \A node \in Honest, qc \in QcRecordSet:
    RecordAppliedNext(node, qc)
      => Chain!ApplyCertifiedNext(node)
BY DEF RecordAppliedNext, AsyncHistoryThrough,
       Chain!ApplyCertifiedNext, Chain!HistoryThrough

THEOREM AsyncHistoryNextRefinesChainEpochNext ==
  AsyncHistoryNext => Chain!ChainEpochNext
BY RecordCertifiedRefinesCertifyNextSubject,
   RecordAppliedRefinesApplyCertifiedNext
   DEF AsyncHistoryNext, Chain!ChainEpochNext

THEOREM AsyncStepRefinesChainEpochStep ==
  [AsyncNext]_AsyncAllVars
    => [Chain!ChainEpochNext]_Chain!ChainEpochVars
PROOF
  <1>1. ASSUME [AsyncNext]_AsyncAllVars
         PROVE [Chain!ChainEpochNext]_Chain!ChainEpochVars
    <2>1. CASE UNCHANGED AsyncAllVars
      BY <2>1 DEF AsyncAllVars, Chain!ChainEpochVars
    <2>2. CASE AsyncProtocolStep
      BY <2>2 DEF AsyncProtocolStep, AsyncHistoryVars,
                      Chain!ChainEpochVars
    <2>3. CASE AsyncHistoryNext
      BY <2>3, AsyncHistoryNextRefinesChainEpochNext
    <2>4. \/ UNCHANGED AsyncAllVars
          \/ AsyncProtocolStep
          \/ AsyncHistoryNext
      BY <1>1 DEF AsyncNext
    <2> QED BY <2>1, <2>2, <2>3, <2>4
  <1> QED BY <1>1

THEOREM AsyncSpecRefinesChainEpochSpec ==
  AsyncSpec => ProjectedChainEpochSpec
PROOF
  <1>1. AsyncInit => Chain!ChainEpochInit
    BY AsyncInitRefinesChainEpochInit
  <1>2. [AsyncNext]_AsyncAllVars
           => [Chain!ChainEpochNext]_Chain!ChainEpochVars
    BY AsyncStepRefinesChainEpochStep
  <1> QED BY <1>1, <1>2, PTL
           DEF AsyncSpec, ProjectedChainEpochSpec

THEOREM ProjectedChainEpochInductiveStep ==
  Chain!ChainEpochInvariant
    /\ [Chain!ChainEpochNext]_Chain!ChainEpochVars
      => Chain!ChainEpochInvariant'
PROOF
  <1>1. ASSUME Chain!ChainEpochInvariant,
              [Chain!ChainEpochNext]_Chain!ChainEpochVars
         PROVE Chain!ChainEpochInvariant'
    <2>1. CASE UNCHANGED Chain!ChainEpochVars
      BY <1>1, <2>1, Isa
         DEF Chain!ChainEpochInvariant, Chain!ChainEpochTypeInvariant,
             Chain!CertifiedPrefixValid,
             Chain!NodesDoNotOutrunCertificates,
             Chain!ContextsMatchLocalHistories,
             Chain!HistoryPrefixComparable, Chain!PerNodeFrozenEpoch,
             Chain!PerNodeParentFinality, Chain!ForeignLineageRejected,
             Chain!CanApplyCertifiedLineage, Chain!HistoryThrough,
             Chain!ChainEpochVars
    <2>2. CASE Chain!ChainEpochNext
      <3>1. CASE \E subject \in ValidSubjects:
                    Chain!CertifyNextSubject(subject)
        BY <1>1, <3>1,
           ChainProof!CertifyPreservesChainEpochInvariant
      <3>2. CASE \E node \in Honest:
                    Chain!ApplyCertifiedNext(node)
        BY <1>1, <3>2,
           ChainProof!ApplyPreservesChainEpochInvariant
      <3> QED BY <3>1, <3>2 DEF Chain!ChainEpochNext
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM AsyncChainPrefixAndEpochSafety ==
  AsyncSpec => []Chain!ChainEpochInvariant
PROOF
  <1>1. AsyncInit => Chain!ChainEpochInvariant
    BY AsyncInitRefinesChainEpochInit,
       ChainProof!GenesisEstablishesChainEpochInvariant
  <1>2. Chain!ChainEpochInvariant
           /\ [Chain!ChainEpochNext]_Chain!ChainEpochVars
             => Chain!ChainEpochInvariant'
    BY ProjectedChainEpochInductiveStep
  <1>3. AsyncSpec => ProjectedChainEpochSpec
    BY AsyncSpecRefinesChainEpochSpec
  <1> QED BY <1>1, <1>2, <1>3, PTL
           DEF ProjectedChainEpochSpec

THEOREM AsyncHistoriesArePrefixComparable ==
  AsyncSpec => []Chain!HistoryPrefixComparable
PROOF
  <1>1. Chain!ChainEpochInvariant => Chain!HistoryPrefixComparable
    BY DEF Chain!ChainEpochInvariant
  <1> QED BY <1>1, AsyncChainPrefixAndEpochSafety, PTL

THEOREM AsyncEpochRoutingIsFrozen ==
  AsyncSpec
    => [](/\ Chain!PerNodeFrozenEpoch
          /\ Chain!PerNodeParentFinality
          /\ Chain!ForeignLineageRejected)
PROOF
  <1>1. Chain!ChainEpochInvariant
           => /\ Chain!PerNodeFrozenEpoch
              /\ Chain!PerNodeParentFinality
              /\ Chain!ForeignLineageRejected
    BY DEF Chain!ChainEpochInvariant
  <1> QED BY <1>1, AsyncChainPrefixAndEpochSafety, PTL

=============================================================================
