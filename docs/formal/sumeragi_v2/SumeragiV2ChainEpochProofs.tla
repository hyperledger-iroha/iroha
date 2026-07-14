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

THEOREM FunctionalUpdateAwayFromKey ==
  \A mapping, key, value, other:
    other \in DOMAIN mapping /\ other # key
      => [mapping EXCEPT ![key] = value][other] = mapping[other]
BY Isa

THEOREM FunctionalUpdateAtKey ==
  \A mapping, key, value:
    key \in DOMAIN mapping
      => [mapping EXCEPT ![key] = value][key] = value
BY Isa

THEOREM IndexBelowSuccessorIsDistinct ==
  \A upper \in Nat:
    \A index \in 1..upper:
      index # upper + 1
BY SMT

THEOREM PrefixBelowSuccessorSubset ==
  \A upper \in Nat, bound \in Nat:
    upper + 1 \in 1..bound
      => 1..upper \subseteq 1..bound
BY SMT

THEOREM PrefixSuccessorWithinBound ==
  \A prefixHeight, bound \in Nat:
    \A slot \in 1..bound:
      prefixHeight < slot => prefixHeight + 1 \in 1..bound
BY SMT

THEOREM NatBoundBelowSuccessor ==
  \A lower \in Nat, upper \in Nat:
    lower <= upper => lower <= upper + 1
BY SMT

THEOREM NatBoundStrictlyBelowSuccessor ==
  \A lower \in Nat, upper \in Nat:
    lower <= upper => lower < upper + 1
BY SMT

THEOREM BoundedPrefixMember ==
  \A lower \in Nat, upper \in Nat:
    lower <= upper => \A index \in 1..lower: index \in 1..upper
BY SMT

THEOREM StrictHeightSuccessorIntervals ==
  \A bound \in Nat:
    \A lower, upper \in 0..bound:
      lower < upper
        => /\ lower + 1 \in Nat
           /\ lower + 1 \in 0..bound
           /\ lower + 1 \in 0..upper
           /\ lower + 1 \in 1..upper
BY SMT

THEOREM PositivePrefixMemberHasPredecessor ==
  \A upper \in Nat:
    \A index \in 1..upper:
      /\ index - 1 \in 0..upper
      /\ index \in 0..upper
BY SMT

(***************************************************************************
Updating a write-once decision slot above a local prefix cannot change that
prefix.  This is the key fact allowing certification and per-node application
to proceed independently.
***************************************************************************)
THEOREM UpdateAbovePrefixPreservesPrefix ==
  \A oldMap, slot, value, prefixHeight:
    (/\ oldMap \in DecisionMapSet
     /\ slot \in DecisionSlots
     /\ value \in SubjectOrNone
     /\ MaxHeight \in Nat
     /\ prefixHeight \in Nat
     /\ prefixHeight < slot)
      => [index \in 1..prefixHeight
            |-> [oldMap EXCEPT ![slot] = value][index]]
           = [index \in 1..prefixHeight |-> oldMap[index]]
PROOF
  <1>1. ASSUME NEW oldMap, NEW slot, NEW value, NEW prefixHeight,
              oldMap \in DecisionMapSet,
              slot \in DecisionSlots,
              value \in SubjectOrNone,
              MaxHeight \in Nat,
              prefixHeight \in Nat,
              prefixHeight < slot
         PROVE [index \in 1..prefixHeight
                  |-> [oldMap EXCEPT ![slot] = value][index]]
                 = [index \in 1..prefixHeight |-> oldMap[index]]
    <2>1. \A index \in 1..prefixHeight: index # slot
      BY <1>1, SMT
    <2>2. prefixHeight + 1 \in DecisionSlots
      BY <1>1, PrefixSuccessorWithinBound DEF DecisionSlots
    <2>3. 1..prefixHeight \subseteq DecisionSlots
      BY <1>1, <2>2, PrefixBelowSuccessorSubset DEF DecisionSlots
    <2>4. DOMAIN oldMap = DecisionSlots
      BY <1>1, Isa DEF DecisionMapSet
    <2>5. \A index \in 1..prefixHeight: index \in DOMAIN oldMap
      BY <2>3, <2>4
    <2>6. \A index \in 1..prefixHeight:
             [oldMap EXCEPT ![slot] = value][index] = oldMap[index]
      BY <2>1, <2>5, FunctionalUpdateAwayFromKey
    <2> QED BY <2>6, Isa
  <1> QED BY <1>1

THEOREM ChainInvariantImpliesHistoryPrefixComparable ==
  ChainEpochInvariant => HistoryPrefixComparable
BY Isa DEF ChainEpochInvariant, ChainEpochTypeInvariant,
           HistoryPrefixComparable, HistoryThrough

THEOREM ChainInvariantImpliesNodeAppliedPrefixBacked ==
  ChainEpochInvariant => NodeAppliedPrefixBacked
BY DEF ChainEpochInvariant

THEOREM GenesisHistoryIsEmpty ==
  HistoryThrough(0) = <<>>
BY Isa DEF HistoryThrough

THEOREM ChainInvariantImpliesPerNodeFrozenEpoch ==
  ChainEpochInvariant => PerNodeFrozenEpoch
PROOF
  <1>1. ASSUME ChainEpochInvariant, NEW node \in ValidatorIds
         PROVE /\ nodeContext[node].height = nodeHeight[node]
               /\ nodeContext[node].epoch
                    = ExpectedEpoch(nodeHeight[node])
               /\ nodeContext[node].roster
                    = RosterSequence(ExpectedEpoch(nodeHeight[node]))
               /\ nodeContext[node].powers
                    = EpochPowers[ExpectedEpoch(nodeHeight[node]) + 1]
    <2>1. nodeContext[node]
             = ContextRecord(nodeHeight[node],
                             HistoryThrough(nodeHeight[node]))
      BY <1>1 DEF ChainEpochInvariant, ContextsMatchLocalHistories
    <2> QED BY <2>1 DEF ContextRecord
  <1> QED BY <1>1 DEF PerNodeFrozenEpoch

THEOREM ChainInvariantImpliesPerNodeParentFinality ==
  ChainEpochInvariant => PerNodeParentFinality
PROOF
  <1>1. ASSUME ChainEpochInvariant,
              NEW node \in ValidatorIds,
              nodeHeight[node] > 0
         PROVE /\ nodeContext[node].parent
                    = decidedAt[nodeHeight[node]]
               /\ nodeContext[node].parentContextKey
                    = ParentContextKey(nodeHeight[node],
                        HistoryThrough(nodeHeight[node]))
               /\ nodeContext[node].parentFinality
                    = ParentFinalityIdentity(nodeHeight[node],
                        HistoryThrough(nodeHeight[node]))
    <2>1. nodeHeight[node] \in Heights
      BY <1>1
         DEF ChainEpochInvariant, ChainEpochTypeInvariant
    <2>2. nodeHeight[node] \in 1..nodeHeight[node]
      BY <1>1, <2>1, SMT DEF Heights
    <2>3. HistoryThrough(nodeHeight[node])[nodeHeight[node]]
             = decidedAt[nodeHeight[node]]
      BY <2>2 DEF HistoryThrough
    <2>4. nodeContext[node]
             = ContextRecord(nodeHeight[node],
                             HistoryThrough(nodeHeight[node]))
      BY <1>1 DEF ChainEpochInvariant, ContextsMatchLocalHistories
    <2> QED BY <1>1, <2>3, <2>4 DEF ContextRecord
  <1> QED BY <1>1 DEF PerNodeParentFinality

THEOREM ForeignLineagesCannotBeApplied ==
  ForeignLineageRejected
BY DEF ForeignLineageRejected, CanApplyCertifiedLineage

THEOREM ForeignContextCertificatesCannotBeAdmitted ==
  ForeignContextCertificateRejected
BY DEF ForeignContextCertificateRejected, CanAdmitNodeCertificate

THEOREM ChainInvariantImpliesChainEpochSafety ==
  ChainEpochInvariant => ChainEpochSafety
BY ChainInvariantImpliesHistoryPrefixComparable,
   ChainInvariantImpliesNodeAppliedPrefixBacked,
   ChainInvariantImpliesPerNodeFrozenEpoch,
   ChainInvariantImpliesPerNodeParentFinality,
   ForeignLineagesCannotBeApplied,
   ForeignContextCertificatesCannotBeAdmitted
   DEF ChainEpochSafety

THEOREM GenesisEstablishesChainEpochInvariant ==
  ChainEpochInit => ChainEpochInvariant
PROOF
  <1>1. ASSUME ChainEpochInit
         PROVE ChainEpochInvariant
    <2>1. ContextRecord(0, <<>>) \in ContextRecords
      BY <1>1, GenesisContextIsAContextRecord DEF ChainEpochInit
    <2>2. ChainEpochTypeInvariant
      <3>1. certifiedHeight \in Heights
        BY <1>1, SMT DEF ChainEpochInit, ModelConfiguration, Heights
      <3>2. decidedAt \in DecisionMapSet
        BY <1>1, Isa
           DEF ChainEpochInit, DecisionMapSet, DecisionSlots,
               SubjectOrNone
      <3>3. nodeHeight \in [ValidatorIds -> Heights]
        BY <1>1, <3>1, Isa DEF ChainEpochInit
      <3>4. /\ durableDecisionEvidence \subseteq DecisionEvidenceSet
             /\ durableApplicationEvidence \subseteq DecisionEvidenceSet
        BY <1>1 DEF ChainEpochInit
      <3>5. nodeContext \in [ValidatorIds -> ContextRecords]
        BY <1>1, <2>1, Isa DEF ChainEpochInit
      <3> QED BY <3>1, <3>2, <3>3, <3>4, <3>5
         DEF ChainEpochTypeInvariant
    <2>3. /\ DurableDecisionEvidenceSound
          /\ DurableApplicationEvidenceSound
          /\ CertifiedPrefixBacked
          /\ NodeAppliedPrefixBacked
          /\ NodesDoNotOutrunCertificates
          /\ ContextsMatchLocalHistories
      <3>1. /\ DurableDecisionEvidenceSound
             /\ DurableApplicationEvidenceSound
        BY <1>1
           DEF ChainEpochInit, DurableDecisionEvidenceSound,
               DurableApplicationEvidenceSound
      <3>2. /\ CertifiedPrefixBacked
             /\ NodeAppliedPrefixBacked
             /\ NodesDoNotOutrunCertificates
        BY <1>1, SMT
           DEF ChainEpochInit, CertifiedPrefixBacked,
               NodeAppliedPrefixBacked, NodesDoNotOutrunCertificates
      <3>3. ContextsMatchLocalHistories
        BY <1>1, GenesisHistoryIsEmpty, Isa
           DEF ChainEpochInit, ContextsMatchLocalHistories
      <3> QED BY <3>1, <3>2, <3>3
    <2> QED BY <1>1, <2>2, <2>3 DEF ChainEpochInvariant,
                                             ChainEpochInit
  <1> QED BY <1>1

(***************************************************************************
Certification writes only the next decision-map slot.  These lemmas expose
the exact old-prefix and new-slot facts used by every receipt invariant,
instead of asking a backend to rediscover functional-update semantics inside
several nested quantifiers.
***************************************************************************)
THEOREM CertificationPreservesHistoryThroughOldHeight ==
  \A decision \in DecisionEvidenceSet:
    ChainEpochInvariant /\ RecordCertifiedNext(decision)
      => \A prefixHeight \in 0..certifiedHeight:
           (HistoryThrough(prefixHeight))' = HistoryThrough(prefixHeight)
PROOF
  <1>1. ASSUME NEW decision \in DecisionEvidenceSet,
              ChainEpochInvariant,
              RecordCertifiedNext(decision)
         PROVE \A prefixHeight \in 0..certifiedHeight:
                 (HistoryThrough(prefixHeight))'
                   = HistoryThrough(prefixHeight)
    <2>1. decidedAt \in DecisionMapSet
      BY <1>1 DEF ChainEpochInvariant, ChainEpochTypeInvariant
    <2>2. certifiedHeight + 1 \in DecisionSlots
      BY <1>1, SMT
         DEF ChainEpochInvariant, ChainEpochTypeInvariant,
             ModelConfiguration, RecordCertifiedNext,
             DecisionSlots, Heights
    <2>3. decision.qc.subject \in ValidSubjects
      BY <1>1
         DEF RecordCertifiedNext, DurableCommitDecision,
             HistoricalCommitCertificate
    <2>4. ValidSubjects \subseteq Subjects
      BY <1>1 DEF ChainEpochInvariant, ModelConfiguration
    <2>5. decision.qc.subject \in SubjectOrNone
      BY <2>3, <2>4 DEF SubjectOrNone
    <2>6. /\ certifiedHeight \in Nat
           /\ MaxHeight \in Nat
      BY <1>1, SMT
         DEF ChainEpochInvariant, ChainEpochTypeInvariant,
             ModelConfiguration, Heights
    <2>7. \A prefixHeight \in 0..certifiedHeight:
             /\ prefixHeight \in Nat
             /\ prefixHeight < certifiedHeight + 1
      BY <2>6, SMT
    <2>8. \A prefixHeight \in 0..certifiedHeight:
             [index \in 1..prefixHeight |-> decidedAt'[index]]
               = [index \in 1..prefixHeight |-> decidedAt[index]]
      BY <1>1, <2>1, <2>2, <2>5, <2>6, <2>7,
         UpdateAbovePrefixPreservesPrefix, SMT
         DEF RecordCertifiedNext
    <2> QED BY <2>8 DEF HistoryThrough
  <1> QED BY <1>1

THEOREM CertificationPreservesCanonicalCommitForOldSlot ==
  \A decision \in DecisionEvidenceSet:
    ChainEpochInvariant /\ RecordCertifiedNext(decision)
      => \A qc, index:
           index \in 1..certifiedHeight
             /\ CanonicalCommitForSlot(qc, index)
               => (CanonicalCommitForSlot(qc, index))'
PROOF
  <1>1. ASSUME NEW decision \in DecisionEvidenceSet,
              ChainEpochInvariant,
              RecordCertifiedNext(decision),
              NEW qc,
              NEW index,
              index \in 1..certifiedHeight,
              CanonicalCommitForSlot(qc, index)
         PROVE (CanonicalCommitForSlot(qc, index))'
    <2>1. certifiedHeight \in Nat
      BY <1>1, SMT
         DEF ChainEpochInvariant, ChainEpochTypeInvariant, Heights
    <2>2. /\ index - 1 \in 0..certifiedHeight
           /\ index \in 0..certifiedHeight
      BY <1>1, <2>1, PositivePrefixMemberHasPredecessor
    <2>3. (HistoryThrough(index - 1))' = HistoryThrough(index - 1)
      BY <1>1, <2>2, CertificationPreservesHistoryThroughOldHeight
    <2>4. index \in DecisionSlots
      BY <1>1 DEF CanonicalCommitForSlot
    <2>5. index \in DOMAIN decidedAt
      BY <1>1, <2>4, Isa
         DEF ChainEpochInvariant, ChainEpochTypeInvariant,
             DecisionMapSet
    <2>6. index # certifiedHeight + 1
      BY <1>1, <2>1, IndexBelowSuccessorIsDistinct
    <2>7. decidedAt'[index] = decidedAt[index]
      BY <1>1, <2>5, <2>6, FunctionalUpdateAwayFromKey
         DEF RecordCertifiedNext
    <2> QED BY <1>1, <2>3, <2>7 DEF CanonicalCommitForSlot
  <1> QED BY <1>1

THEOREM CertificationAddsCanonicalCommitForNextSlot ==
  \A decision \in DecisionEvidenceSet:
    ChainEpochInvariant /\ RecordCertifiedNext(decision)
      => (CanonicalCommitForSlot(decision.qc, certifiedHeight))'
PROOF
  <1>1. ASSUME NEW decision \in DecisionEvidenceSet,
              ChainEpochInvariant,
              RecordCertifiedNext(decision)
         PROVE (CanonicalCommitForSlot(decision.qc, certifiedHeight))'
    <2>1. decidedAt \in DecisionMapSet
      BY <1>1 DEF ChainEpochInvariant, ChainEpochTypeInvariant
    <2>2. certifiedHeight + 1 \in DecisionSlots
      BY <1>1, SMT
         DEF ChainEpochInvariant, ChainEpochTypeInvariant,
             ModelConfiguration, RecordCertifiedNext,
             DecisionSlots, Heights
    <2>3. certifiedHeight \in Nat
      BY <1>1, SMT
         DEF ChainEpochInvariant, ChainEpochTypeInvariant, Heights
    <2>4. MaxHeight \in Nat
      BY <1>1, SMT DEF ChainEpochInvariant, ModelConfiguration
    <2>5. 1..certifiedHeight \subseteq DecisionSlots
      BY <2>2, <2>3, <2>4, PrefixBelowSuccessorSubset
         DEF DecisionSlots
    <2>6. \A index \in 1..certifiedHeight: index \in DOMAIN decidedAt
      BY <2>1, <2>5, Isa DEF DecisionMapSet
    <2>7. \A index \in 1..certifiedHeight:
             index # certifiedHeight + 1
      BY <2>3, IndexBelowSuccessorIsDistinct
    <2>8. \A index \in 1..certifiedHeight:
             decidedAt'[index] = decidedAt[index]
      BY <1>1, <2>6, <2>7, FunctionalUpdateAwayFromKey
         DEF RecordCertifiedNext
    <2>9. [index \in 1..certifiedHeight |-> decidedAt'[index]]
             = HistoryThrough(certifiedHeight)
      BY <2>8, Isa DEF HistoryThrough
    <2>10. certifiedHeight + 1 \in DOMAIN decidedAt
      BY <1>1, <2>2, Isa
         DEF ChainEpochInvariant, ChainEpochTypeInvariant,
             DecisionMapSet
    <2>11. decidedAt'[certifiedHeight + 1] = decision.qc.subject
      BY <1>1, <2>10, FunctionalUpdateAtKey
         DEF RecordCertifiedNext
    <2>12. decision.qc.height = certifiedHeight
      BY <1>1
         DEF RecordCertifiedNext, DurableCommitDecision,
             HistoricalCommitCertificate, ContextRecord
    <2>13. certifiedHeight' = certifiedHeight + 1
      BY <1>1 DEF RecordCertifiedNext
    <2>14. certifiedHeight' \in DecisionSlots
      BY <2>2, <2>13
    <2>15. [index \in 1..(certifiedHeight' - 1) |-> decidedAt'[index]]
               = HistoryThrough(certifiedHeight)
      BY <2>3, <2>9, <2>13, SMT
    <2>16. decision.qc.context
               = ContextRecord(certifiedHeight' - 1,
                               [index \in 1..(certifiedHeight' - 1)
                                  |-> decidedAt'[index]])
      BY <1>1, <2>3, <2>13, <2>15, SMT DEF RecordCertifiedNext
    <2>17. decision.qc.height = certifiedHeight' - 1
      BY <2>3, <2>12, <2>13, SMT
    <2>18. decision.qc.phase = "Commit"
      BY <1>1
         DEF RecordCertifiedNext, DurableCommitDecision,
             HistoricalCommitCertificate
    <2>19. decision.qc.subject = decidedAt'[certifiedHeight']
      BY <2>11, <2>13
    <2> QED BY <2>14, <2>16, <2>17, <2>18, <2>19
       DEF CanonicalCommitForSlot, HistoryThrough
  <1> QED BY <1>1

THEOREM CertificationPreservesRecordedDecisionBacking ==
  \A decision \in DecisionEvidenceSet:
    ChainEpochInvariant /\ RecordCertifiedNext(decision)
      => \A recorded:
           DecisionBacksCertifiedSlot(recorded)
             => (DecisionBacksCertifiedSlot(recorded))'
PROOF
  <1>1. ASSUME NEW decision \in DecisionEvidenceSet,
              ChainEpochInvariant,
              RecordCertifiedNext(decision),
              NEW recorded,
              DecisionBacksCertifiedSlot(recorded)
         PROVE (DecisionBacksCertifiedSlot(recorded))'
    <2>1. \E index \in 1..certifiedHeight:
             CanonicalCommitForSlot(recorded.qc, index)
      BY <1>1 DEF DecisionBacksCertifiedSlot
    <2>2. \A index \in 1..certifiedHeight:
             CanonicalCommitForSlot(recorded.qc, index)
               => (CanonicalCommitForSlot(recorded.qc, index))'
      BY <1>1, CertificationPreservesCanonicalCommitForOldSlot
    <2>3. certifiedHeight \in Nat
      BY <1>1, SMT
         DEF ChainEpochInvariant, ChainEpochTypeInvariant, Heights
    <2>4. certifiedHeight' = certifiedHeight + 1
      BY <1>1 DEF RecordCertifiedNext
    <2>5. 1..certifiedHeight \subseteq 1..certifiedHeight'
      BY <2>3, <2>4, SMT
    <2> QED BY <2>1, <2>2, <2>5, Isa DEF DecisionBacksCertifiedSlot
  <1> QED BY <1>1

THEOREM CertificationRecordsNewDecisionBacking ==
  \A decision \in DecisionEvidenceSet:
    ChainEpochInvariant /\ RecordCertifiedNext(decision)
      => (DecisionBacksCertifiedSlot(decision))'
PROOF
  <1>1. ASSUME NEW decision \in DecisionEvidenceSet,
              ChainEpochInvariant,
              RecordCertifiedNext(decision)
         PROVE (DecisionBacksCertifiedSlot(decision))'
    <2>1. certifiedHeight \in Nat
      BY <1>1, SMT
         DEF ChainEpochInvariant, ChainEpochTypeInvariant, Heights
    <2>2. certifiedHeight' = certifiedHeight + 1
      BY <1>1 DEF RecordCertifiedNext
    <2>3. certifiedHeight' \in 1..certifiedHeight'
      BY <2>1, <2>2, SMT
    <2>4. (CanonicalCommitForSlot(decision.qc, certifiedHeight))'
      BY <1>1, CertificationAddsCanonicalCommitForNextSlot
    <2> QED BY <2>3, <2>4, Isa DEF DecisionBacksCertifiedSlot
  <1> QED BY <1>1

THEOREM CertificationPreservesChainEpochInvariant ==
  \A decision \in DecisionEvidenceSet:
    ChainEpochInvariant /\ RecordCertifiedNext(decision)
      => ChainEpochInvariant'
PROOF
  <1>1. ASSUME NEW decision \in DecisionEvidenceSet,
              ChainEpochInvariant,
              RecordCertifiedNext(decision)
         PROVE ChainEpochInvariant'
    <2>1. ChainEpochTypeInvariant'
      <3>1. ChainEpochTypeInvariant
        BY <1>1 DEF ChainEpochInvariant
      <3>2. certifiedHeight' \in Heights
        BY <1>1, <3>1, SMT
           DEF ChainEpochInvariant, ChainEpochTypeInvariant,
               ModelConfiguration, RecordCertifiedNext, Heights
      <3>3. decision.qc.subject \in SubjectOrNone
        BY <1>1, <3>1
           DEF ChainEpochInvariant, ModelConfiguration,
               RecordCertifiedNext, DurableCommitDecision,
               HistoricalCommitCertificate, SubjectOrNone
      <3>4. decidedAt' \in DecisionMapSet
        BY <1>1, <3>1, <3>3, Isa
           DEF ChainEpochTypeInvariant, RecordCertifiedNext,
               DecisionMapSet, DecisionSlots
      <3>5. /\ nodeHeight' \in [ValidatorIds -> Heights]
             /\ nodeContext' \in [ValidatorIds -> ContextRecords]
             /\ durableDecisionEvidence' \subseteq DecisionEvidenceSet
             /\ durableApplicationEvidence' \subseteq DecisionEvidenceSet
        BY <1>1, <3>1, Isa
           DEF ChainEpochTypeInvariant, RecordCertifiedNext
      <3> QED BY <3>2, <3>4, <3>5 DEF ChainEpochTypeInvariant
    <2>2. DurableDecisionEvidenceSound'
      <3>1. durableDecisionEvidence'
               = durableDecisionEvidence \cup {decision}
        BY <1>1 DEF RecordCertifiedNext
      <3>2. ASSUME NEW recorded \in durableDecisionEvidence'
             PROVE /\ (DurableCommitDecision(recorded))'
                   /\ \/ (DecisionBacksCertifiedSlot(recorded))'
                      \/ (ReceiptOutsideChainHorizon(recorded))'
        <4>1. CASE recorded = decision
          <5>1. DurableCommitDecision(decision)
            BY <1>1 DEF RecordCertifiedNext
          <5>2. (DurableCommitDecision(recorded))'
            BY <4>1, <5>1, Isa
               DEF DurableCommitDecision, HistoricalCommitCertificate
          <5>3. (DecisionBacksCertifiedSlot(recorded))'
            BY <1>1, <4>1, CertificationRecordsNewDecisionBacking
          <5> QED BY <5>2, <5>3
        <4>2. CASE recorded \in durableDecisionEvidence
          <5>1. /\ DurableCommitDecision(recorded)
                 /\ \/ DecisionBacksCertifiedSlot(recorded)
                    \/ ReceiptOutsideChainHorizon(recorded)
            BY <1>1, <4>2
               DEF ChainEpochInvariant, DurableDecisionEvidenceSound
          <5>2. (DurableCommitDecision(recorded))'
            BY <5>1, Isa
               DEF DurableCommitDecision, HistoricalCommitCertificate
          <5>3. CASE DecisionBacksCertifiedSlot(recorded)
            BY <1>1, <5>2, <5>3,
               CertificationPreservesRecordedDecisionBacking
          <5>4. CASE ReceiptOutsideChainHorizon(recorded)
            BY <5>2, <5>4, Isa DEF ReceiptOutsideChainHorizon
          <5> QED BY <5>1, <5>2, <5>3, <5>4
        <4>3. recorded = decision
                 \/ recorded \in durableDecisionEvidence
          BY <3>1, <3>2, Isa
        <4> QED BY <4>1, <4>2, <4>3
      <3> QED BY <3>2 DEF DurableDecisionEvidenceSound
    <2>3. DurableApplicationEvidenceSound'
      <3>1. /\ durableApplicationEvidence'
                    = durableApplicationEvidence
             /\ durableDecisionEvidence'
                    = durableDecisionEvidence \cup {decision}
        BY <1>1 DEF RecordCertifiedNext
      <3>2. ASSUME NEW application \in durableApplicationEvidence'
             PROVE /\ (DurableCommitDecision(application))'
                   /\ (ApplicationHasRecordedDecision(application))'
                   /\ \/ (DecisionBacksCertifiedSlot(application))'
                      \/ (ReceiptOutsideChainHorizon(application))'
        <4>1. application \in durableApplicationEvidence
          BY <3>1, <3>2
        <4>2. /\ DurableCommitDecision(application)
               /\ ApplicationHasRecordedDecision(application)
               /\ \/ DecisionBacksCertifiedSlot(application)
                  \/ ReceiptOutsideChainHorizon(application)
          BY <1>1, <4>1
             DEF ChainEpochInvariant, DurableApplicationEvidenceSound
        <4>3. (DurableCommitDecision(application))'
          BY <4>2, Isa
             DEF DurableCommitDecision, HistoricalCommitCertificate
        <4>4. (ApplicationHasRecordedDecision(application))'
          BY <3>1, <4>2, Isa DEF ApplicationHasRecordedDecision
        <4>5. CASE DecisionBacksCertifiedSlot(application)
          BY <1>1, <4>3, <4>4, <4>5,
             CertificationPreservesRecordedDecisionBacking
        <4>6. CASE ReceiptOutsideChainHorizon(application)
          BY <4>3, <4>4, <4>6, Isa DEF ReceiptOutsideChainHorizon
        <4> QED BY <4>2, <4>3, <4>4, <4>5, <4>6
      <3> QED BY <3>2 DEF DurableApplicationEvidenceSound
    <2>4. CertifiedPrefixBacked'
      <3>1. certifiedHeight \in Nat
        BY <1>1, SMT
           DEF ChainEpochInvariant, ChainEpochTypeInvariant, Heights
      <3>2. certifiedHeight' = certifiedHeight + 1
        BY <1>1 DEF RecordCertifiedNext
      <3>3. ASSUME NEW index \in 1..certifiedHeight'
             PROVE /\ decidedAt'[index] \in ValidSubjects
                   /\ \E recorded \in durableDecisionEvidence':
                        (CanonicalCommitForSlot(recorded.qc, index))'
        <4>1. CASE index \in 1..certifiedHeight
          <5>1. /\ decidedAt[index] \in ValidSubjects
                 /\ \E recorded \in durableDecisionEvidence:
                      CanonicalCommitForSlot(recorded.qc, index)
            BY <1>1, <4>1
               DEF ChainEpochInvariant, CertifiedPrefixBacked
          <5>2. PICK recorded \in durableDecisionEvidence:
                   CanonicalCommitForSlot(recorded.qc, index)
            BY <5>1
          <5>3. (CanonicalCommitForSlot(recorded.qc, index))'
            BY <1>1, <4>1, <5>2,
               CertificationPreservesCanonicalCommitForOldSlot
          <5>4. recorded \in durableDecisionEvidence'
            BY <1>1, <5>2, Isa DEF RecordCertifiedNext
          <5>5. decidedAt'[index] = decidedAt[index]
            BY <5>2, <5>3, Isa DEF CanonicalCommitForSlot
          <5> QED BY <5>1, <5>3, <5>4, <5>5
        <4>2. CASE index = certifiedHeight'
          <5>1. decision.qc.subject \in ValidSubjects
            BY <1>1
               DEF RecordCertifiedNext, DurableCommitDecision,
                   HistoricalCommitCertificate
          <5>2. (CanonicalCommitForSlot(decision.qc,
                                                   certifiedHeight))'
            BY <1>1, CertificationAddsCanonicalCommitForNextSlot
          <5>3. decidedAt'[index] = decision.qc.subject
            BY <4>2, <5>2, Isa DEF CanonicalCommitForSlot
          <5>4. decision \in durableDecisionEvidence'
            BY <1>1, Isa DEF RecordCertifiedNext
          <5> QED BY <5>1, <5>2, <5>3, <5>4, <4>2
        <4>3. index \in 1..certifiedHeight
                 \/ index = certifiedHeight'
          BY <3>1, <3>2, <3>3, SMT
        <4> QED BY <4>1, <4>2, <4>3
      <3> QED BY <3>3 DEF CertifiedPrefixBacked
    <2>5. NodesDoNotOutrunCertificates'
      <3>1. /\ certifiedHeight' = certifiedHeight + 1
             /\ nodeHeight' = nodeHeight
        BY <1>1 DEF RecordCertifiedNext
      <3>2. /\ certifiedHeight \in Nat
             /\ \A node \in ValidatorIds:
                  /\ nodeHeight[node] \in Nat
                  /\ nodeHeight[node] <= certifiedHeight
        BY <1>1, SMT
           DEF ChainEpochInvariant, ChainEpochTypeInvariant,
               NodesDoNotOutrunCertificates, Heights
      <3>3. ASSUME NEW node \in ValidatorIds
             PROVE nodeHeight'[node] <= certifiedHeight'
        <4>1. nodeHeight'[node] = nodeHeight[node]
          BY <3>1
        <4>2. /\ nodeHeight[node] \in Nat
               /\ nodeHeight[node] <= certifiedHeight
          BY <3>2, <3>3
        <4>3. nodeHeight[node] <= certifiedHeight + 1
          BY <3>2, <4>2, NatBoundBelowSuccessor
        <4> QED BY <3>1, <4>1, <4>3
      <3> QED BY <3>3 DEF NodesDoNotOutrunCertificates
    <2>6. NodeAppliedPrefixBacked'
      <3>1. /\ nodeHeight' = nodeHeight
             /\ durableApplicationEvidence'
                  = durableApplicationEvidence
        BY <1>1 DEF RecordCertifiedNext
      <3>2. /\ certifiedHeight \in Nat
             /\ \A node \in ValidatorIds:
                  /\ nodeHeight[node] \in Nat
                  /\ nodeHeight[node] <= certifiedHeight
        BY <1>1, SMT
           DEF ChainEpochInvariant, ChainEpochTypeInvariant,
               NodesDoNotOutrunCertificates, Heights
      <3>3. ASSUME NEW node \in ValidatorIds,
                    NEW index \in 1..nodeHeight'[node]
             PROVE \E application \in durableApplicationEvidence':
                     /\ application.node = node
                     /\ (CanonicalCommitForSlot(application.qc, index))'
        <4>1. nodeHeight'[node] = nodeHeight[node]
          BY <3>1
        <4>2. index \in 1..nodeHeight[node]
          BY <3>3, <4>1
        <4>3. index \in 1..certifiedHeight
          BY <3>2, <3>3, <4>2, BoundedPrefixMember
        <4>4. PICK application \in durableApplicationEvidence:
                 /\ application.node = node
                 /\ CanonicalCommitForSlot(application.qc, index)
          BY <1>1, <3>3, <4>2
             DEF ChainEpochInvariant, NodeAppliedPrefixBacked
        <4>5. (CanonicalCommitForSlot(application.qc, index))'
          BY <1>1, <4>3, <4>4,
             CertificationPreservesCanonicalCommitForOldSlot
        <4>6. application \in durableApplicationEvidence'
          BY <3>1, <4>4
        <4> QED BY <4>4, <4>5, <4>6
      <3> QED BY <3>3 DEF NodeAppliedPrefixBacked
    <2>7. ContextsMatchLocalHistories'
      <3>1. /\ nodeHeight' = nodeHeight
             /\ nodeContext' = nodeContext
        BY <1>1 DEF RecordCertifiedNext
      <3>2. /\ decidedAt \in DecisionMapSet
             /\ certifiedHeight \in Nat
             /\ MaxHeight \in Nat
             /\ certifiedHeight + 1 \in DecisionSlots
             /\ decision.qc.subject \in SubjectOrNone
        BY <1>1, SMT
           DEF ChainEpochInvariant, ChainEpochTypeInvariant,
               ModelConfiguration, RecordCertifiedNext,
               DurableCommitDecision, HistoricalCommitCertificate,
               SubjectOrNone, Heights, DecisionSlots
      <3>3. ASSUME NEW node \in ValidatorIds
             PROVE nodeContext'[node]
                     = ContextRecord(nodeHeight'[node],
                         [index \in 1..nodeHeight'[node]
                            |-> decidedAt'[index]])
        <4>1. /\ nodeHeight[node] \in Nat
               /\ nodeHeight[node] <= certifiedHeight
          BY <1>1, <3>3, SMT
             DEF ChainEpochInvariant, ChainEpochTypeInvariant,
                 NodesDoNotOutrunCertificates, Heights
        <4>2. nodeHeight[node] < certifiedHeight + 1
          BY <3>2, <4>1, NatBoundStrictlyBelowSuccessor
        <4>3. [index \in 1..nodeHeight[node] |-> decidedAt'[index]]
                   = HistoryThrough(nodeHeight[node])
          BY <1>1, <3>2, <4>1, <4>2,
             UpdateAbovePrefixPreservesPrefix
             DEF RecordCertifiedNext, HistoryThrough
        <4>4. nodeContext[node]
                   = ContextRecord(nodeHeight[node],
                                   HistoryThrough(nodeHeight[node]))
          BY <1>1, <3>3
             DEF ChainEpochInvariant, ContextsMatchLocalHistories
        <4> QED BY <3>1, <4>3, <4>4
      <3> QED BY <3>3 DEF ContextsMatchLocalHistories, HistoryThrough
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>5, <2>6, <2>7
       DEF ChainEpochInvariant, RecordCertifiedNext
  <1> QED BY <1>1

(***************************************************************************
Application leaves the canonical decision map fixed.  The following small
lemmas isolate the resulting lineage and CommitQC stability before the local
node-height update is considered.
***************************************************************************)
THEOREM CertifiedHistoryPrefixIsLineage ==
  ChainEpochInvariant
    => \A prefixHeight \in 0..certifiedHeight:
         HistoryThrough(prefixHeight) \in LineagesAt(prefixHeight)
PROOF
  <1>1. ASSUME ChainEpochInvariant,
              NEW prefixHeight \in 0..certifiedHeight
         PROVE HistoryThrough(prefixHeight) \in LineagesAt(prefixHeight)
    <2>1. /\ prefixHeight \in Nat
           /\ certifiedHeight \in Nat
           /\ prefixHeight <= certifiedHeight
      BY <1>1, SMT
         DEF ChainEpochInvariant, ChainEpochTypeInvariant, Heights
    <2>2. \A index \in 1..prefixHeight:
             index \in 1..certifiedHeight
      BY <2>1, BoundedPrefixMember
    <2>3. \A index \in 1..prefixHeight:
             decidedAt[index] \in ValidSubjects
      BY <1>1, <2>2 DEF ChainEpochInvariant, CertifiedPrefixBacked
    <2>4. ValidSubjects \subseteq Subjects
      BY <1>1 DEF ChainEpochInvariant, ModelConfiguration
    <2>5. HistoryThrough(prefixHeight)
             \in [1..prefixHeight -> Subjects]
      BY <2>3, <2>4, Isa DEF HistoryThrough
    <2> QED BY <2>5 DEF LineagesAt
  <1> QED BY <1>1

THEOREM ApplicationPreservesCanonicalCommitForSlot ==
  \A application \in DecisionEvidenceSet:
    RecordAppliedNext(application)
      => \A qc, index:
           CanonicalCommitForSlot(qc, index)
             => (CanonicalCommitForSlot(qc, index))'
PROOF
  <1>1. ASSUME NEW application \in DecisionEvidenceSet,
              RecordAppliedNext(application),
              NEW qc,
              NEW index,
              CanonicalCommitForSlot(qc, index)
         PROVE (CanonicalCommitForSlot(qc, index))'
    <2>1. decidedAt' = decidedAt
      BY <1>1 DEF RecordAppliedNext
    <2> QED BY <1>1, <2>1, Isa
       DEF CanonicalCommitForSlot, HistoryThrough
  <1> QED BY <1>1

THEOREM UnchangedDecisionMapPreservesCanonicalCommit ==
  decidedAt' = decidedAt
    => \A qc, index:
         CanonicalCommitForSlot(qc, index)
           => (CanonicalCommitForSlot(qc, index))'
PROOF
  <1>1. ASSUME decidedAt' = decidedAt,
              NEW qc,
              NEW index,
              CanonicalCommitForSlot(qc, index)
         PROVE (CanonicalCommitForSlot(qc, index))'
    <2> QED BY <1>1, Isa
       DEF CanonicalCommitForSlot, HistoryThrough
  <1> QED BY <1>1

THEOREM UnchangedChainStatePreservesDecisionBacking ==
  certifiedHeight' = certifiedHeight /\ decidedAt' = decidedAt
    => \A recorded:
         DecisionBacksCertifiedSlot(recorded)
           => (DecisionBacksCertifiedSlot(recorded))'
PROOF
  <1>1. ASSUME certifiedHeight' = certifiedHeight,
              decidedAt' = decidedAt,
              NEW recorded,
              DecisionBacksCertifiedSlot(recorded)
         PROVE (DecisionBacksCertifiedSlot(recorded))'
    <2>1. \A index \in 1..certifiedHeight:
             CanonicalCommitForSlot(recorded.qc, index)
               => (CanonicalCommitForSlot(recorded.qc, index))'
      BY <1>1, UnchangedDecisionMapPreservesCanonicalCommit
    <2> QED BY <1>1, <2>1, Isa DEF DecisionBacksCertifiedSlot
  <1> QED BY <1>1

THEOREM UnchangedChainStatePreservesReceiptSound ==
  \A receipt:
    (/\ certifiedHeight' = certifiedHeight
     /\ decidedAt' = decidedAt
     /\ DurableCommitDecision(receipt)
     /\ \/ DecisionBacksCertifiedSlot(receipt)
        \/ ReceiptOutsideChainHorizon(receipt))
      => /\ (DurableCommitDecision(receipt))'
         /\ \/ (DecisionBacksCertifiedSlot(receipt))'
            \/ (ReceiptOutsideChainHorizon(receipt))'
PROOF
  <1>1. ASSUME NEW receipt,
              certifiedHeight' = certifiedHeight,
              decidedAt' = decidedAt,
              DurableCommitDecision(receipt),
              DecisionBacksCertifiedSlot(receipt)
                \/ ReceiptOutsideChainHorizon(receipt)
         PROVE /\ (DurableCommitDecision(receipt))'
               /\ \/ (DecisionBacksCertifiedSlot(receipt))'
                  \/ (ReceiptOutsideChainHorizon(receipt))'
    <2>1. (DurableCommitDecision(receipt))'
      BY <1>1, Isa
         DEF DurableCommitDecision, HistoricalCommitCertificate
    <2>2. CASE DecisionBacksCertifiedSlot(receipt)
      BY <1>1, <2>1, <2>2,
         UnchangedChainStatePreservesDecisionBacking
    <2>3. CASE ReceiptOutsideChainHorizon(receipt)
      BY <2>1, <2>3, Isa DEF ReceiptOutsideChainHorizon
    <2> QED BY <1>1, <2>2, <2>3
  <1> QED BY <1>1

THEOREM MonotoneDecisionEvidencePreservesCertifiedPrefix ==
  (/\ certifiedHeight' = certifiedHeight
   /\ decidedAt' = decidedAt
   /\ durableDecisionEvidence \subseteq durableDecisionEvidence'
   /\ CertifiedPrefixBacked)
    => CertifiedPrefixBacked'
PROOF
  <1>1. ASSUME certifiedHeight' = certifiedHeight,
              decidedAt' = decidedAt,
              durableDecisionEvidence \subseteq durableDecisionEvidence',
              CertifiedPrefixBacked
         PROVE CertifiedPrefixBacked'
    <2>1. ASSUME NEW index \in 1..certifiedHeight'
           PROVE /\ decidedAt'[index] \in ValidSubjects
                 /\ \E recorded \in durableDecisionEvidence':
                      (CanonicalCommitForSlot(recorded.qc, index))'
      <3>1. index \in 1..certifiedHeight
        BY <1>1, <2>1
      <3>2. /\ decidedAt[index] \in ValidSubjects
             /\ \E recorded \in durableDecisionEvidence:
                  CanonicalCommitForSlot(recorded.qc, index)
        BY <1>1, <3>1 DEF CertifiedPrefixBacked
      <3>3. PICK recorded \in durableDecisionEvidence:
               CanonicalCommitForSlot(recorded.qc, index)
        BY <3>2
      <3>4. (CanonicalCommitForSlot(recorded.qc, index))'
        BY <1>1, <3>3, UnchangedDecisionMapPreservesCanonicalCommit
      <3>5. recorded \in durableDecisionEvidence'
        BY <1>1, <3>3
      <3> QED BY <1>1, <3>2, <3>4, <3>5
    <2> QED BY <2>1 DEF CertifiedPrefixBacked
  <1> QED BY <1>1

THEOREM MonotoneApplicationEvidencePreservesNodePrefix ==
  (/\ nodeHeight' = nodeHeight
   /\ decidedAt' = decidedAt
   /\ durableApplicationEvidence \subseteq durableApplicationEvidence'
   /\ NodeAppliedPrefixBacked)
    => NodeAppliedPrefixBacked'
PROOF
  <1>1. ASSUME nodeHeight' = nodeHeight,
              decidedAt' = decidedAt,
              durableApplicationEvidence
                \subseteq durableApplicationEvidence',
              NodeAppliedPrefixBacked
         PROVE NodeAppliedPrefixBacked'
    <2>1. ASSUME NEW node \in ValidatorIds,
                  NEW index \in 1..nodeHeight'[node]
           PROVE \E application \in durableApplicationEvidence':
                   /\ application.node = node
                   /\ (CanonicalCommitForSlot(application.qc, index))'
      <3>1. index \in 1..nodeHeight[node]
        BY <1>1, <2>1
      <3>2. PICK application \in durableApplicationEvidence:
               /\ application.node = node
               /\ CanonicalCommitForSlot(application.qc, index)
        BY <1>1, <2>1, <3>1 DEF NodeAppliedPrefixBacked
      <3>3. (CanonicalCommitForSlot(application.qc, index))'
        BY <1>1, <3>2, UnchangedDecisionMapPreservesCanonicalCommit
      <3>4. application \in durableApplicationEvidence'
        BY <1>1, <3>2
      <3> QED BY <3>2, <3>3, <3>4
    <2> QED BY <2>1 DEF NodeAppliedPrefixBacked
  <1> QED BY <1>1

THEOREM UnchangedHeightsPreserveCertificateBound ==
  certifiedHeight' = certifiedHeight
    /\ nodeHeight' = nodeHeight
    /\ NodesDoNotOutrunCertificates
      => NodesDoNotOutrunCertificates'
BY Isa DEF NodesDoNotOutrunCertificates

THEOREM UnchangedLocalStatePreservesContexts ==
  decidedAt' = decidedAt
    /\ nodeHeight' = nodeHeight
    /\ nodeContext' = nodeContext
    /\ ContextsMatchLocalHistories
      => ContextsMatchLocalHistories'
BY Isa DEF ContextsMatchLocalHistories, HistoryThrough

THEOREM ApplicationPreservesDecisionBacking ==
  \A application \in DecisionEvidenceSet:
    RecordAppliedNext(application)
      => \A recorded:
           DecisionBacksCertifiedSlot(recorded)
             => (DecisionBacksCertifiedSlot(recorded))'
PROOF
  <1>1. ASSUME NEW application \in DecisionEvidenceSet,
              RecordAppliedNext(application),
              NEW recorded,
              DecisionBacksCertifiedSlot(recorded)
         PROVE (DecisionBacksCertifiedSlot(recorded))'
    <2>1. certifiedHeight' = certifiedHeight
      BY <1>1 DEF RecordAppliedNext
    <2>2. \A index \in 1..certifiedHeight:
             CanonicalCommitForSlot(recorded.qc, index)
               => (CanonicalCommitForSlot(recorded.qc, index))'
      BY <1>1, ApplicationPreservesCanonicalCommitForSlot
    <2> QED BY <1>1, <2>1, <2>2, Isa
       DEF DecisionBacksCertifiedSlot
  <1> QED BY <1>1

THEOREM RecordAppliedTransitionFacts ==
  \A application \in DecisionEvidenceSet:
    ChainEpochInvariant /\ RecordAppliedNext(application)
      => /\ application.node \in ValidatorIds
         /\ nodeHeight \in [ValidatorIds -> Heights]
         /\ nodeContext \in [ValidatorIds -> ContextRecords]
         /\ nodeHeight[application.node] \in Heights
         /\ certifiedHeight \in Heights
         /\ nodeHeight[application.node] < certifiedHeight
         /\ nodeHeight[application.node] + 1 \in Nat
         /\ nodeHeight[application.node] + 1 \in Heights
         /\ nodeHeight[application.node] + 1
              \in 0..certifiedHeight
         /\ nodeHeight[application.node] + 1
              \in 1..certifiedHeight
         /\ certifiedHeight' = certifiedHeight
         /\ decidedAt' = decidedAt
         /\ durableDecisionEvidence' = durableDecisionEvidence
         /\ durableApplicationEvidence'
              = durableApplicationEvidence \cup {application}
         /\ nodeHeight' = [nodeHeight EXCEPT
              ![application.node] = nodeHeight[application.node] + 1]
         /\ nodeContext' = [nodeContext EXCEPT
              ![application.node]
                = ContextRecord(nodeHeight[application.node] + 1,
                    HistoryThrough(nodeHeight[application.node] + 1))]
PROOF
  <1>1. ASSUME NEW application \in DecisionEvidenceSet,
              ChainEpochInvariant,
              RecordAppliedNext(application)
         PROVE /\ application.node \in ValidatorIds
               /\ nodeHeight \in [ValidatorIds -> Heights]
               /\ nodeContext \in [ValidatorIds -> ContextRecords]
               /\ nodeHeight[application.node] \in Heights
               /\ certifiedHeight \in Heights
               /\ nodeHeight[application.node] < certifiedHeight
               /\ nodeHeight[application.node] + 1 \in Nat
               /\ nodeHeight[application.node] + 1 \in Heights
               /\ nodeHeight[application.node] + 1
                    \in 0..certifiedHeight
               /\ nodeHeight[application.node] + 1
                    \in 1..certifiedHeight
               /\ certifiedHeight' = certifiedHeight
               /\ decidedAt' = decidedAt
               /\ durableDecisionEvidence' = durableDecisionEvidence
               /\ durableApplicationEvidence'
                    = durableApplicationEvidence \cup {application}
               /\ nodeHeight' = [nodeHeight EXCEPT
                    ![application.node]
                      = nodeHeight[application.node] + 1]
               /\ nodeContext' = [nodeContext EXCEPT
                    ![application.node]
                      = ContextRecord(nodeHeight[application.node] + 1,
                          HistoryThrough(nodeHeight[application.node] + 1))]
    <2>1. ChainEpochTypeInvariant
      BY <1>1 DEF ChainEpochInvariant
    <2>2. /\ ModelConfiguration
           /\ Honest \subseteq ValidatorIds
      BY <1>1
         DEF ChainEpochInvariant, ModelConfiguration,
             QuorumConfiguration
    <2>3. application.node \in Honest
      BY <1>1 DEF RecordAppliedNext
    <2>4. application.node \in ValidatorIds
      BY <2>2, <2>3
    <2>5. /\ nodeHeight \in [ValidatorIds -> Heights]
           /\ nodeContext \in [ValidatorIds -> ContextRecords]
           /\ certifiedHeight \in Heights
      BY <2>1 DEF ChainEpochTypeInvariant
    <2>6. nodeHeight[application.node] \in Heights
      BY <2>4, <2>5, Isa
    <2>7. nodeHeight[application.node] < certifiedHeight
      BY <1>1 DEF RecordAppliedNext
    <2>8. /\ MaxHeight \in Nat
           /\ nodeHeight[application.node] + 1 \in Nat
           /\ nodeHeight[application.node] + 1 \in Heights
           /\ nodeHeight[application.node] + 1
                \in 0..certifiedHeight
           /\ nodeHeight[application.node] + 1
                \in 1..certifiedHeight
      BY <2>2, <2>5, <2>6, <2>7,
         StrictHeightSuccessorIntervals DEF ModelConfiguration, Heights
    <2>9. /\ certifiedHeight' = certifiedHeight
           /\ decidedAt' = decidedAt
           /\ durableDecisionEvidence' = durableDecisionEvidence
           /\ durableApplicationEvidence'
                = durableApplicationEvidence \cup {application}
           /\ nodeHeight' = [nodeHeight EXCEPT
                ![application.node] = nodeHeight[application.node] + 1]
           /\ nodeContext' = [nodeContext EXCEPT
                ![application.node]
                  = ContextRecord(nodeHeight[application.node] + 1,
                      HistoryThrough(nodeHeight[application.node] + 1))]
      BY <1>1 DEF RecordAppliedNext
    <2> QED BY <2>4, <2>5, <2>6, <2>7, <2>8, <2>9
  <1> QED BY <1>1

THEOREM ApplicationPreservesChainEpochInvariant ==
  \A application \in DecisionEvidenceSet:
    ChainEpochInvariant /\ RecordAppliedNext(application)
      => ChainEpochInvariant'
PROOF
  <1>1. ASSUME NEW application \in DecisionEvidenceSet,
              ChainEpochInvariant,
              RecordAppliedNext(application)
         PROVE ChainEpochInvariant'
    <2>1. ChainEpochTypeInvariant'
      <3>1. ChainEpochTypeInvariant
        BY <1>1 DEF ChainEpochInvariant
      <3>2. /\ application.node \in ValidatorIds
             /\ nodeHeight[application.node] \in Heights
             /\ certifiedHeight \in Heights
             /\ nodeHeight[application.node] < certifiedHeight
        BY <1>1, RecordAppliedTransitionFacts
      <3>3. /\ nodeHeight[application.node] + 1 \in Heights
             /\ nodeHeight[application.node] + 1
                  \in 0..certifiedHeight
        BY <1>1, RecordAppliedTransitionFacts
      <3>4. HistoryThrough(nodeHeight[application.node] + 1)
               \in LineagesAt(nodeHeight[application.node] + 1)
        BY <1>1, <3>3, CertifiedHistoryPrefixIsLineage
      <3>5. ContextRecord(nodeHeight[application.node] + 1,
                         HistoryThrough(nodeHeight[application.node] + 1))
               \in ContextRecords
        BY <3>3, <3>4, Isa DEF ContextRecords
      <3>6. nodeHeight' \in [ValidatorIds -> Heights]
        BY <1>1, <3>1, <3>2, <3>3, Isa
           DEF ChainEpochTypeInvariant, RecordAppliedNext
      <3>7. nodeContext' \in [ValidatorIds -> ContextRecords]
        BY <1>1, <3>1, <3>2, <3>5, Isa
           DEF ChainEpochTypeInvariant, RecordAppliedNext
      <3>8. durableApplicationEvidence'
               \subseteq DecisionEvidenceSet
        BY <1>1, <3>1, Isa
           DEF ChainEpochTypeInvariant, RecordAppliedNext
      <3>9. /\ certifiedHeight' \in Heights
             /\ decidedAt' \in DecisionMapSet
             /\ durableDecisionEvidence'
                  \subseteq DecisionEvidenceSet
        BY <1>1, <3>1
           DEF ChainEpochTypeInvariant, RecordAppliedNext
      <3> QED BY <3>6, <3>7, <3>8, <3>9
         DEF ChainEpochTypeInvariant
    <2>2. /\ DurableDecisionEvidenceSound'
          /\ CertifiedPrefixBacked'
      <3>1. /\ certifiedHeight' = certifiedHeight
             /\ decidedAt' = decidedAt
             /\ durableDecisionEvidence'
                  = durableDecisionEvidence
        BY <1>1 DEF RecordAppliedNext
      <3>2. DurableDecisionEvidenceSound'
        <4>1. ASSUME NEW recorded \in durableDecisionEvidence'
               PROVE /\ (DurableCommitDecision(recorded))'
                     /\ \/ (DecisionBacksCertifiedSlot(recorded))'
                        \/ (ReceiptOutsideChainHorizon(recorded))'
          <5>1. /\ DurableCommitDecision(recorded)
                 /\ \/ DecisionBacksCertifiedSlot(recorded)
                    \/ ReceiptOutsideChainHorizon(recorded)
            BY <1>1, <3>1, <4>1
               DEF ChainEpochInvariant, DurableDecisionEvidenceSound
          <5>2. (DurableCommitDecision(recorded))'
            BY <5>1, Isa
               DEF DurableCommitDecision, HistoricalCommitCertificate
          <5>3. CASE DecisionBacksCertifiedSlot(recorded)
            BY <1>1, <5>2, <5>3,
               ApplicationPreservesDecisionBacking
          <5>4. CASE ReceiptOutsideChainHorizon(recorded)
            BY <5>2, <5>4, Isa DEF ReceiptOutsideChainHorizon
          <5> QED BY <5>1, <5>3, <5>4
        <4> QED BY <4>1 DEF DurableDecisionEvidenceSound
      <3>3. CertifiedPrefixBacked'
        <4>1. ASSUME NEW index \in 1..certifiedHeight'
               PROVE /\ decidedAt'[index] \in ValidSubjects
                     /\ \E recorded \in durableDecisionEvidence':
                          (CanonicalCommitForSlot(recorded.qc, index))'
          <5>1. index \in 1..certifiedHeight
            BY <3>1, <4>1
          <5>2. /\ decidedAt[index] \in ValidSubjects
                 /\ \E recorded \in durableDecisionEvidence:
                      CanonicalCommitForSlot(recorded.qc, index)
            BY <1>1, <5>1
               DEF ChainEpochInvariant, CertifiedPrefixBacked
          <5>3. PICK recorded \in durableDecisionEvidence:
                   CanonicalCommitForSlot(recorded.qc, index)
            BY <5>2
          <5>4. (CanonicalCommitForSlot(recorded.qc, index))'
            BY <1>1, <5>3,
               ApplicationPreservesCanonicalCommitForSlot
          <5> QED BY <3>1, <5>2, <5>3, <5>4
        <4> QED BY <4>1 DEF CertifiedPrefixBacked
      <3> QED BY <3>2, <3>3
    <2>3. DurableApplicationEvidenceSound'
      <3>1. /\ durableApplicationEvidence'
                    = durableApplicationEvidence \cup {application}
             /\ durableDecisionEvidence'
                    = durableDecisionEvidence
             /\ certifiedHeight' = certifiedHeight
        BY <1>1 DEF RecordAppliedNext
      <3>2. /\ nodeHeight[application.node] \in Nat
             /\ certifiedHeight \in Nat
             /\ nodeHeight[application.node] < certifiedHeight
             /\ nodeHeight[application.node] + 1
                  \in 1..certifiedHeight
        BY <1>1, RecordAppliedTransitionFacts, SMT DEF Heights
      <3>3. DecisionBacksCertifiedSlot(application)
        BY <1>1, <3>2
           DEF DecisionBacksCertifiedSlot, RecordAppliedNext
      <3>4. (DecisionBacksCertifiedSlot(application))'
        BY <1>1, <3>3, ApplicationPreservesDecisionBacking
      <3>5. ASSUME NEW recorded \in durableApplicationEvidence'
             PROVE /\ (DurableCommitDecision(recorded))'
                   /\ (ApplicationHasRecordedDecision(recorded))'
                   /\ \/ (DecisionBacksCertifiedSlot(recorded))'
                      \/ (ReceiptOutsideChainHorizon(recorded))'
        <4>1. CASE recorded = application
          <5>1. (DurableCommitDecision(recorded))'
            BY <1>1, <4>1, Isa
               DEF RecordAppliedNext, DurableCommitDecision,
                   HistoricalCommitCertificate
          <5>2. (ApplicationHasRecordedDecision(recorded))'
            BY <1>1, <4>1, Isa
               DEF RecordAppliedNext, ApplicationHasRecordedDecision
          <5> QED BY <4>1, <3>4, <5>1, <5>2
        <4>2. CASE recorded \in durableApplicationEvidence
          <5>1. /\ DurableCommitDecision(recorded)
                 /\ ApplicationHasRecordedDecision(recorded)
                 /\ \/ DecisionBacksCertifiedSlot(recorded)
                    \/ ReceiptOutsideChainHorizon(recorded)
            BY <1>1, <4>2
               DEF ChainEpochInvariant, DurableApplicationEvidenceSound
          <5>2. (DurableCommitDecision(recorded))'
            BY <5>1, Isa
               DEF DurableCommitDecision, HistoricalCommitCertificate
          <5>3. (ApplicationHasRecordedDecision(recorded))'
            BY <3>1, <5>1, Isa DEF ApplicationHasRecordedDecision
          <5>4. CASE DecisionBacksCertifiedSlot(recorded)
            BY <1>1, <5>2, <5>3, <5>4,
               ApplicationPreservesDecisionBacking
          <5>5. CASE ReceiptOutsideChainHorizon(recorded)
            BY <5>2, <5>3, <5>5, Isa DEF ReceiptOutsideChainHorizon
          <5> QED BY <5>1, <5>4, <5>5
        <4>3. recorded = application
                 \/ recorded \in durableApplicationEvidence
          BY <3>1, <3>5, Isa
        <4> QED BY <4>1, <4>2, <4>3
      <3> QED BY <3>5 DEF DurableApplicationEvidenceSound
    <2>4. NodesDoNotOutrunCertificates'
      <3>1. /\ certifiedHeight' = certifiedHeight
             /\ nodeHeight \in [ValidatorIds -> Heights]
             /\ application.node \in ValidatorIds
             /\ nodeHeight' = [nodeHeight EXCEPT
                  ![application.node] = nodeHeight[application.node] + 1]
        BY <1>1, RecordAppliedTransitionFacts
      <3>2. /\ nodeHeight[application.node] \in Nat
             /\ certifiedHeight \in Nat
             /\ nodeHeight[application.node] < certifiedHeight
        BY <1>1, RecordAppliedTransitionFacts, SMT DEF Heights
      <3>3. \A node \in ValidatorIds:
               nodeHeight[node] <= certifiedHeight
        BY <1>1 DEF ChainEpochInvariant, NodesDoNotOutrunCertificates
      <3>4. ASSUME NEW node \in ValidatorIds
             PROVE nodeHeight'[node] <= certifiedHeight'
        <4>1. CASE node = application.node
          <5>1. nodeHeight'[node]
                   = nodeHeight[application.node] + 1
            BY <3>1, <4>1, FunctionalUpdateAtKey
          <5> QED BY <3>1, <3>2, <5>1, SMT
        <4>2. CASE node # application.node
          <5>1. nodeHeight'[node] = nodeHeight[node]
            BY <3>1, <3>4, <4>2, FunctionalUpdateAwayFromKey
          <5>2. nodeHeight[node] <= certifiedHeight
            BY <3>3, <3>4
          <5> QED BY <3>1, <5>1, <5>2
        <4>3. node = application.node \/ node # application.node
          BY SMT
        <4> QED BY <4>1, <4>2, <4>3
      <3> QED BY <3>4 DEF NodesDoNotOutrunCertificates
    <2>5. NodeAppliedPrefixBacked'
      <3>1. /\ nodeHeight \in [ValidatorIds -> Heights]
             /\ application.node \in ValidatorIds
             /\ nodeHeight' = [nodeHeight EXCEPT
                  ![application.node] = nodeHeight[application.node] + 1]
             /\ durableApplicationEvidence'
                  = durableApplicationEvidence \cup {application}
        BY <1>1, RecordAppliedTransitionFacts
      <3>2. /\ nodeHeight[application.node] \in Nat
             /\ nodeHeight[application.node] + 1 \in Nat
        BY <1>1, RecordAppliedTransitionFacts, SMT DEF Heights
      <3>3. ASSUME NEW node \in ValidatorIds,
                    NEW index \in 1..nodeHeight'[node]
             PROVE \E recorded \in durableApplicationEvidence':
                     /\ recorded.node = node
                     /\ (CanonicalCommitForSlot(recorded.qc, index))'
        <4>1. CASE node = application.node
          <5>1. nodeHeight'[node]
                   = nodeHeight[application.node] + 1
            BY <3>1, <4>1, FunctionalUpdateAtKey
          <5>2. index \in 1..nodeHeight[application.node]
                   \/ index = nodeHeight[application.node] + 1
            BY <3>2, <3>3, <5>1, SMT
          <5>3. CASE index \in 1..nodeHeight[application.node]
            <6>1. PICK recorded \in durableApplicationEvidence:
                     /\ recorded.node = node
                     /\ CanonicalCommitForSlot(recorded.qc, index)
              BY <1>1, <4>1, <5>3
                 DEF ChainEpochInvariant, NodeAppliedPrefixBacked
            <6>2. (CanonicalCommitForSlot(recorded.qc, index))'
              BY <1>1, <6>1,
                 ApplicationPreservesCanonicalCommitForSlot
            <6>3. recorded \in durableApplicationEvidence'
              BY <3>1, <6>1, Isa
            <6> QED BY <6>1, <6>2, <6>3
          <5>4. CASE index = nodeHeight[application.node] + 1
            <6>1. application \in durableApplicationEvidence'
              BY <3>1, Isa
            <6>2. (CanonicalCommitForSlot(application.qc, index))'
              BY <1>1, <5>4,
                 ApplicationPreservesCanonicalCommitForSlot
                 DEF RecordAppliedNext
            <6> QED BY <4>1, <6>1, <6>2
          <5> QED BY <5>2, <5>3, <5>4
        <4>2. CASE node # application.node
          <5>1. nodeHeight'[node] = nodeHeight[node]
            BY <3>1, <3>3, <4>2, FunctionalUpdateAwayFromKey
          <5>2. index \in 1..nodeHeight[node]
            BY <3>3, <5>1
          <5>3. PICK recorded \in durableApplicationEvidence:
                   /\ recorded.node = node
                   /\ CanonicalCommitForSlot(recorded.qc, index)
            BY <1>1, <3>3, <5>2
               DEF ChainEpochInvariant, NodeAppliedPrefixBacked
          <5>4. (CanonicalCommitForSlot(recorded.qc, index))'
            BY <1>1, <5>3,
               ApplicationPreservesCanonicalCommitForSlot
          <5>5. recorded \in durableApplicationEvidence'
            BY <3>1, <5>3, Isa
          <5> QED BY <5>3, <5>4, <5>5
        <4>3. node = application.node \/ node # application.node
          BY SMT
        <4> QED BY <4>1, <4>2, <4>3
      <3> QED BY <3>3 DEF NodeAppliedPrefixBacked
    <2>6. ContextsMatchLocalHistories'
      <3>1. /\ nodeHeight \in [ValidatorIds -> Heights]
             /\ nodeContext \in [ValidatorIds -> ContextRecords]
             /\ application.node \in ValidatorIds
             /\ decidedAt' = decidedAt
             /\ nodeHeight' = [nodeHeight EXCEPT
                  ![application.node] = nodeHeight[application.node] + 1]
             /\ nodeContext' = [nodeContext EXCEPT
                  ![application.node]
                    = ContextRecord(nodeHeight[application.node] + 1,
                        HistoryThrough(nodeHeight[application.node] + 1))]
        BY <1>1, RecordAppliedTransitionFacts
      <3>2. ASSUME NEW node \in ValidatorIds
             PROVE nodeContext'[node]
                     = ContextRecord(nodeHeight'[node],
                         [index \in 1..nodeHeight'[node]
                            |-> decidedAt'[index]])
        <4>1. CASE node = application.node
          <5>1. nodeHeight'[node]
                   = nodeHeight[application.node] + 1
            BY <3>1, <4>1, FunctionalUpdateAtKey
          <5>2. nodeContext'[node]
                   = ContextRecord(nodeHeight[application.node] + 1,
                       HistoryThrough(nodeHeight[application.node] + 1))
            BY <3>1, <4>1, FunctionalUpdateAtKey
          <5> QED BY <3>1, <5>1, <5>2 DEF HistoryThrough
        <4>2. CASE node # application.node
          <5>1. nodeHeight'[node] = nodeHeight[node]
            BY <3>1, <3>2, <4>2, FunctionalUpdateAwayFromKey
          <5>2. nodeContext'[node] = nodeContext[node]
            BY <3>1, <3>2, <4>2, FunctionalUpdateAwayFromKey
          <5>3. nodeContext[node]
                   = ContextRecord(nodeHeight[node],
                                   HistoryThrough(nodeHeight[node]))
            BY <1>1, <3>2
               DEF ChainEpochInvariant, ContextsMatchLocalHistories
          <5> QED BY <3>1, <5>1, <5>2, <5>3 DEF HistoryThrough
        <4>3. node = application.node \/ node # application.node
          BY SMT
        <4> QED BY <4>1, <4>2, <4>3
      <3> QED BY <3>2 DEF ContextsMatchLocalHistories, HistoryThrough
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>5, <2>6
       DEF ChainEpochInvariant, RecordAppliedNext
  <1> QED BY <1>1

THEOREM KnownDecisionPreservesChainEpochInvariant ==
  \A decision \in DecisionEvidenceSet:
    ChainEpochInvariant /\ RecordKnownDecision(decision)
      => ChainEpochInvariant'
PROOF
  <1>1. ASSUME NEW decision \in DecisionEvidenceSet,
              ChainEpochInvariant,
              RecordKnownDecision(decision)
         PROVE ChainEpochInvariant'
    <2>1. ChainEpochTypeInvariant'
      BY <1>1, Isa
         DEF ChainEpochInvariant, ChainEpochTypeInvariant,
             RecordKnownDecision
    <2>2. DurableDecisionEvidenceSound'
      <3>1. /\ certifiedHeight' = certifiedHeight
             /\ decidedAt' = decidedAt
             /\ durableDecisionEvidence'
                  = durableDecisionEvidence \cup {decision}
        BY <1>1 DEF RecordKnownDecision
      <3>2. ASSUME NEW recorded \in durableDecisionEvidence'
             PROVE /\ (DurableCommitDecision(recorded))'
                   /\ \/ (DecisionBacksCertifiedSlot(recorded))'
                      \/ (ReceiptOutsideChainHorizon(recorded))'
        <4>1. recorded = decision
                 \/ recorded \in durableDecisionEvidence
          BY <3>1, <3>2, Isa
        <4>2. CASE recorded = decision
          <5>1. /\ DurableCommitDecision(recorded)
                 /\ \/ DecisionBacksCertifiedSlot(recorded)
                    \/ ReceiptOutsideChainHorizon(recorded)
            BY <1>1, <4>2 DEF RecordKnownDecision
          <5> QED BY <3>1, <5>1,
             UnchangedChainStatePreservesReceiptSound
        <4>3. CASE recorded \in durableDecisionEvidence
          <5>1. /\ DurableCommitDecision(recorded)
                 /\ \/ DecisionBacksCertifiedSlot(recorded)
                    \/ ReceiptOutsideChainHorizon(recorded)
            BY <1>1, <4>3
               DEF ChainEpochInvariant, DurableDecisionEvidenceSound
          <5> QED BY <3>1, <5>1,
             UnchangedChainStatePreservesReceiptSound
        <4> QED BY <4>1, <4>2, <4>3
      <3> QED BY <3>2 DEF DurableDecisionEvidenceSound
    <2>3. DurableApplicationEvidenceSound'
      <3>1. /\ certifiedHeight' = certifiedHeight
             /\ decidedAt' = decidedAt
             /\ durableDecisionEvidence'
                  = durableDecisionEvidence \cup {decision}
             /\ durableApplicationEvidence'
                  = durableApplicationEvidence
        BY <1>1 DEF RecordKnownDecision
      <3>2. ASSUME NEW application \in durableApplicationEvidence'
             PROVE /\ (DurableCommitDecision(application))'
                   /\ (ApplicationHasRecordedDecision(application))'
                   /\ \/ (DecisionBacksCertifiedSlot(application))'
                      \/ (ReceiptOutsideChainHorizon(application))'
        <4>1. /\ DurableCommitDecision(application)
               /\ ApplicationHasRecordedDecision(application)
               /\ \/ DecisionBacksCertifiedSlot(application)
                  \/ ReceiptOutsideChainHorizon(application)
          BY <1>1, <3>1, <3>2
             DEF ChainEpochInvariant, DurableApplicationEvidenceSound
        <4>2. /\ (DurableCommitDecision(application))'
               /\ \/ (DecisionBacksCertifiedSlot(application))'
                  \/ (ReceiptOutsideChainHorizon(application))'
          BY <3>1, <4>1, UnchangedChainStatePreservesReceiptSound
        <4>3. (ApplicationHasRecordedDecision(application))'
          BY <3>1, <4>1, Isa DEF ApplicationHasRecordedDecision
        <4> QED BY <4>2, <4>3
      <3> QED BY <3>2 DEF DurableApplicationEvidenceSound
    <2>4. /\ CertifiedPrefixBacked'
          /\ NodeAppliedPrefixBacked'
          /\ NodesDoNotOutrunCertificates'
          /\ ContextsMatchLocalHistories'
      <3>1. /\ certifiedHeight' = certifiedHeight
             /\ decidedAt' = decidedAt
             /\ nodeHeight' = nodeHeight
             /\ nodeContext' = nodeContext
             /\ durableDecisionEvidence'
                  = durableDecisionEvidence \cup {decision}
             /\ durableApplicationEvidence'
                  = durableApplicationEvidence
        BY <1>1 DEF RecordKnownDecision
      <3>2. /\ durableDecisionEvidence
                    \subseteq durableDecisionEvidence'
             /\ durableApplicationEvidence
                    \subseteq durableApplicationEvidence'
        BY <3>1, Isa
      <3>3. CertifiedPrefixBacked'
        BY <1>1, <3>1, <3>2,
           MonotoneDecisionEvidencePreservesCertifiedPrefix
           DEF ChainEpochInvariant
      <3>4. NodeAppliedPrefixBacked'
        BY <1>1, <3>1, <3>2,
           MonotoneApplicationEvidencePreservesNodePrefix
           DEF ChainEpochInvariant
      <3>5. NodesDoNotOutrunCertificates'
        BY <1>1, <3>1, UnchangedHeightsPreserveCertificateBound
           DEF ChainEpochInvariant
      <3>6. ContextsMatchLocalHistories'
        BY <1>1, <3>1, UnchangedLocalStatePreservesContexts
           DEF ChainEpochInvariant
      <3> QED BY <3>3, <3>4, <3>5, <3>6
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4
       DEF ChainEpochInvariant, RecordKnownDecision
  <1> QED BY <1>1

THEOREM KnownApplicationPreservesChainEpochInvariant ==
  \A application \in DecisionEvidenceSet:
    ChainEpochInvariant /\ RecordKnownApplication(application)
      => ChainEpochInvariant'
PROOF
  <1>1. ASSUME NEW application \in DecisionEvidenceSet,
              ChainEpochInvariant,
              RecordKnownApplication(application)
         PROVE ChainEpochInvariant'
    <2>1. ChainEpochTypeInvariant'
      BY <1>1, Isa
         DEF ChainEpochInvariant, ChainEpochTypeInvariant,
             RecordKnownApplication
    <2>2. DurableApplicationEvidenceSound'
      <3>1. /\ certifiedHeight' = certifiedHeight
             /\ decidedAt' = decidedAt
             /\ durableDecisionEvidence'
                  = durableDecisionEvidence
             /\ durableApplicationEvidence'
                  = durableApplicationEvidence \cup {application}
        BY <1>1 DEF RecordKnownApplication
      <3>2. ASSUME NEW recorded \in durableApplicationEvidence'
             PROVE /\ (DurableCommitDecision(recorded))'
                   /\ (ApplicationHasRecordedDecision(recorded))'
                   /\ \/ (DecisionBacksCertifiedSlot(recorded))'
                      \/ (ReceiptOutsideChainHorizon(recorded))'
        <4>1. recorded = application
                 \/ recorded \in durableApplicationEvidence
          BY <3>1, <3>2, Isa
        <4>2. CASE recorded = application
          <5>1. /\ DurableCommitDecision(recorded)
                 /\ ApplicationHasRecordedDecision(recorded)
                 /\ \/ DecisionBacksCertifiedSlot(recorded)
                    \/ ReceiptOutsideChainHorizon(recorded)
            BY <1>1, <4>2 DEF RecordKnownApplication
          <5>2. /\ (DurableCommitDecision(recorded))'
                 /\ \/ (DecisionBacksCertifiedSlot(recorded))'
                    \/ (ReceiptOutsideChainHorizon(recorded))'
            BY <3>1, <5>1, UnchangedChainStatePreservesReceiptSound
          <5>3. (ApplicationHasRecordedDecision(recorded))'
            BY <3>1, <5>1, Isa DEF ApplicationHasRecordedDecision
          <5> QED BY <5>2, <5>3
        <4>3. CASE recorded \in durableApplicationEvidence
          <5>1. /\ DurableCommitDecision(recorded)
                 /\ ApplicationHasRecordedDecision(recorded)
                 /\ \/ DecisionBacksCertifiedSlot(recorded)
                    \/ ReceiptOutsideChainHorizon(recorded)
            BY <1>1, <4>3
               DEF ChainEpochInvariant, DurableApplicationEvidenceSound
          <5>2. /\ (DurableCommitDecision(recorded))'
                 /\ \/ (DecisionBacksCertifiedSlot(recorded))'
                    \/ (ReceiptOutsideChainHorizon(recorded))'
            BY <3>1, <5>1, UnchangedChainStatePreservesReceiptSound
          <5>3. (ApplicationHasRecordedDecision(recorded))'
            BY <3>1, <5>1, Isa DEF ApplicationHasRecordedDecision
          <5> QED BY <5>2, <5>3
        <4> QED BY <4>1, <4>2, <4>3
      <3> QED BY <3>2 DEF DurableApplicationEvidenceSound
    <2>3. /\ DurableDecisionEvidenceSound'
          /\ CertifiedPrefixBacked'
          /\ NodeAppliedPrefixBacked'
          /\ NodesDoNotOutrunCertificates'
          /\ ContextsMatchLocalHistories'
      <3>1. /\ certifiedHeight' = certifiedHeight
             /\ decidedAt' = decidedAt
             /\ nodeHeight' = nodeHeight
             /\ nodeContext' = nodeContext
             /\ durableDecisionEvidence'
                  = durableDecisionEvidence
             /\ durableApplicationEvidence'
                  = durableApplicationEvidence \cup {application}
        BY <1>1 DEF RecordKnownApplication
      <3>2. /\ durableDecisionEvidence
                    \subseteq durableDecisionEvidence'
             /\ durableApplicationEvidence
                    \subseteq durableApplicationEvidence'
        BY <3>1, Isa
      <3>3. DurableDecisionEvidenceSound'
        <4>1. ASSUME NEW recorded \in durableDecisionEvidence'
               PROVE /\ (DurableCommitDecision(recorded))'
                     /\ \/ (DecisionBacksCertifiedSlot(recorded))'
                        \/ (ReceiptOutsideChainHorizon(recorded))'
          <5>1. /\ DurableCommitDecision(recorded)
                 /\ \/ DecisionBacksCertifiedSlot(recorded)
                    \/ ReceiptOutsideChainHorizon(recorded)
            BY <1>1, <3>1, <4>1
               DEF ChainEpochInvariant, DurableDecisionEvidenceSound
          <5> QED BY <3>1, <5>1,
             UnchangedChainStatePreservesReceiptSound
        <4> QED BY <4>1 DEF DurableDecisionEvidenceSound
      <3>4. CertifiedPrefixBacked'
        BY <1>1, <3>1, <3>2,
           MonotoneDecisionEvidencePreservesCertifiedPrefix
           DEF ChainEpochInvariant
      <3>5. NodeAppliedPrefixBacked'
        BY <1>1, <3>1, <3>2,
           MonotoneApplicationEvidencePreservesNodePrefix
           DEF ChainEpochInvariant
      <3>6. NodesDoNotOutrunCertificates'
        BY <1>1, <3>1, UnchangedHeightsPreserveCertificateBound
           DEF ChainEpochInvariant
      <3>7. ContextsMatchLocalHistories'
        BY <1>1, <3>1, UnchangedLocalStatePreservesContexts
           DEF ChainEpochInvariant
      <3> QED BY <3>3, <3>4, <3>5, <3>6, <3>7
    <2> QED BY <1>1, <2>1, <2>2, <2>3
       DEF ChainEpochInvariant, RecordKnownApplication
  <1> QED BY <1>1

THEOREM LaggingNodesRemainUnchanged ==
  \A application \in DecisionEvidenceSet:
    ChainEpochTypeInvariant /\ RecordAppliedNext(application)
      => \A other \in ValidatorIds \ {application.node}:
           /\ nodeHeight'[other] = nodeHeight[other]
           /\ nodeContext'[other] = nodeContext[other]
BY FunctionalUpdateAwayFromKey, Isa
   DEF ChainEpochTypeInvariant, RecordAppliedNext

THEOREM CertificationHasNoApplicationBarrier ==
  \A decision \in DecisionEvidenceSet:
    RecordCertifiedNext(decision)
      => /\ decision \in durableDecisionEvidence'
         /\ certifiedHeight' = certifiedHeight + 1
         /\ UNCHANGED <<nodeHeight, nodeContext>>
BY DEF RecordCertifiedNext

THEOREM StutterPreservesChainEpochInvariant ==
  ChainEpochInvariant /\ UNCHANGED ChainEpochVars
    => ChainEpochInvariant'
PROOF
  <1>1. ASSUME ChainEpochInvariant,
              UNCHANGED ChainEpochVars
         PROVE ChainEpochInvariant'
    <2>1. /\ certifiedHeight' = certifiedHeight
           /\ decidedAt' = decidedAt
           /\ nodeHeight' = nodeHeight
           /\ nodeContext' = nodeContext
           /\ durableDecisionEvidence' = durableDecisionEvidence
           /\ durableApplicationEvidence' = durableApplicationEvidence
      BY <1>1 DEF ChainEpochVars
    <2>2. ChainEpochTypeInvariant'
      BY <1>1, <2>1, Isa
         DEF ChainEpochInvariant, ChainEpochTypeInvariant
    <2>3. DurableDecisionEvidenceSound'
      <3>1. ASSUME NEW recorded \in durableDecisionEvidence'
             PROVE /\ (DurableCommitDecision(recorded))'
                   /\ \/ (DecisionBacksCertifiedSlot(recorded))'
                      \/ (ReceiptOutsideChainHorizon(recorded))'
        <4>1. /\ DurableCommitDecision(recorded)
               /\ \/ DecisionBacksCertifiedSlot(recorded)
                  \/ ReceiptOutsideChainHorizon(recorded)
          BY <1>1, <2>1, <3>1
             DEF ChainEpochInvariant, DurableDecisionEvidenceSound
        <4> QED BY <2>1, <4>1,
           UnchangedChainStatePreservesReceiptSound
      <3> QED BY <3>1 DEF DurableDecisionEvidenceSound
    <2>4. DurableApplicationEvidenceSound'
      <3>1. ASSUME NEW application \in durableApplicationEvidence'
             PROVE /\ (DurableCommitDecision(application))'
                   /\ (ApplicationHasRecordedDecision(application))'
                   /\ \/ (DecisionBacksCertifiedSlot(application))'
                      \/ (ReceiptOutsideChainHorizon(application))'
        <4>1. /\ DurableCommitDecision(application)
               /\ ApplicationHasRecordedDecision(application)
               /\ \/ DecisionBacksCertifiedSlot(application)
                  \/ ReceiptOutsideChainHorizon(application)
          BY <1>1, <2>1, <3>1
             DEF ChainEpochInvariant, DurableApplicationEvidenceSound
        <4>2. /\ (DurableCommitDecision(application))'
               /\ \/ (DecisionBacksCertifiedSlot(application))'
                  \/ (ReceiptOutsideChainHorizon(application))'
          BY <2>1, <4>1, UnchangedChainStatePreservesReceiptSound
        <4>3. (ApplicationHasRecordedDecision(application))'
          BY <2>1, <4>1, Isa DEF ApplicationHasRecordedDecision
        <4> QED BY <4>2, <4>3
      <3> QED BY <3>1 DEF DurableApplicationEvidenceSound
    <2>5. CertifiedPrefixBacked'
      <3>1. durableDecisionEvidence \subseteq durableDecisionEvidence'
        BY <2>1
      <3> QED BY <1>1, <2>1, <3>1,
         MonotoneDecisionEvidencePreservesCertifiedPrefix
         DEF ChainEpochInvariant
    <2>6. NodeAppliedPrefixBacked'
      <3>1. durableApplicationEvidence
               \subseteq durableApplicationEvidence'
        BY <2>1
      <3> QED BY <1>1, <2>1, <3>1,
         MonotoneApplicationEvidencePreservesNodePrefix
         DEF ChainEpochInvariant
    <2>7. NodesDoNotOutrunCertificates'
      BY <1>1, <2>1, UnchangedHeightsPreserveCertificateBound
         DEF ChainEpochInvariant
    <2>8. ContextsMatchLocalHistories'
      BY <1>1, <2>1, UnchangedLocalStatePreservesContexts
         DEF ChainEpochInvariant
    <2> QED BY <1>1, <2>2, <2>3, <2>4, <2>5, <2>6, <2>7, <2>8
       DEF ChainEpochInvariant
  <1> QED BY <1>1

THEOREM ChainEpochInductiveStep ==
  ChainEpochInvariant /\ [ChainEpochNext]_ChainEpochVars
    => ChainEpochInvariant'
PROOF
  <1>1. ASSUME ChainEpochInvariant,
              [ChainEpochNext]_ChainEpochVars
         PROVE ChainEpochInvariant'
    <2>1. CASE UNCHANGED ChainEpochVars
      BY <1>1, <2>1, StutterPreservesChainEpochInvariant
    <2>2. CASE ChainEpochNext
      <3>1. CASE \E decision \in DecisionEvidenceSet:
                    RecordCertifiedNext(decision)
        BY <1>1, <3>1, CertificationPreservesChainEpochInvariant
      <3>2. CASE \E decision \in DecisionEvidenceSet:
                    RecordKnownDecision(decision)
        BY <1>1, <3>2, KnownDecisionPreservesChainEpochInvariant
      <3>3. CASE \E application \in DecisionEvidenceSet:
                    RecordAppliedNext(application)
        BY <1>1, <3>3, ApplicationPreservesChainEpochInvariant
      <3>4. CASE \E application \in DecisionEvidenceSet:
                    RecordKnownApplication(application)
        BY <1>1, <3>4, KnownApplicationPreservesChainEpochInvariant
      <3> QED BY <2>2, <3>1, <3>2, <3>3, <3>4
         DEF ChainEpochNext
    <2> QED BY <1>1, <2>1, <2>2
  <1> QED BY <1>1

THEOREM ChainPrefixAndEpochSafety ==
  ChainEpochSpec => []ChainEpochSafety
PROOF
  <1>1. ChainEpochInit => ChainEpochInvariant
    BY GenesisEstablishesChainEpochInvariant
  <1>2. ChainEpochInvariant /\ [ChainEpochNext]_ChainEpochVars
           => ChainEpochInvariant'
    BY ChainEpochInductiveStep
  <1>3. ChainEpochSpec => []ChainEpochInvariant
    BY <1>1, <1>2, PTL DEF ChainEpochSpec
  <1>4. ChainEpochInvariant => ChainEpochSafety
    BY ChainInvariantImpliesChainEpochSafety
  <1> QED BY <1>3, <1>4, PTL

(***************************************************************************
Stable public properties.  The proof ledger pins these bodies exactly so the
release obligations cannot be redirected to the one-height Core spec or to a
global application barrier.
***************************************************************************)
ChainPrefixProperty(specification) ==
  specification => [](/\ HistoryPrefixComparable
                       /\ NodeAppliedPrefixBacked)

EpochBoundaryProperty(specification) ==
  specification => [](/\ PerNodeFrozenEpoch
                       /\ PerNodeParentFinality
                       /\ ForeignLineageRejected
                       /\ ForeignContextCertificateRejected)

THEOREM ChainPrefixObligation ==
  ChainPrefixProperty(ChainEpochSpec)
BY ChainPrefixAndEpochSafety, PTL
   DEF ChainPrefixProperty, ChainEpochSafety

THEOREM EpochBoundaryObligation ==
  EpochBoundaryProperty(ChainEpochSpec)
BY ChainPrefixAndEpochSafety, PTL
   DEF EpochBoundaryProperty, ChainEpochSafety

=============================================================================
