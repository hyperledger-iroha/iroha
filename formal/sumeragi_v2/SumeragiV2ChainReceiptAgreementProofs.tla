---- MODULE SumeragiV2ChainReceiptAgreementProofs ----
EXTENDS SumeragiV2ChainEpochRefinement, TLAPS

(***************************************************************************
Exact receipt-subject agreement across the indexed chain product.

SumeragiV2ChainEpoch records the first certified subject for each slot in the
single write-once `decidedAt` map.  That projection is useful only after the
indexed Core product has established that all durable receipts for a slot
already agree; the map itself must not be the reason a conflicting Core step
is impossible.

The bridge below keeps those arguments separate.  First, an inductive source
ownership invariant records that every current decision receipt was created
by a Core instance which had already joined the product.  Second, canonical
joined-context identity reduces equal receipt slots to the same frozen Core
instance.  Finally that instance's `DecisionAgreement` conjunct -- the state
predicate closed temporally by `AgreementObligation` and ultimately backed by
the no-conflicting-CommitQC proof -- supplies subject equality.  No
`CanonicalCommitForSlot` or `decidedAt[slot]` equality is used to obtain the
result.
***************************************************************************)

DurableCommitReceiptEvidence ==
  durableDecisionEvidence \cup durableApplicationEvidence

CommitReceiptSlot(receipt) == receipt.qc.context.height + 1

(***************************************************************************
Every projected current decision has an instance which was joined before the
receipt appeared.  This is a history fact: StrongInductiveInvariant alone
does not distinguish a dormant pre-created instance from a joined instance.
***************************************************************************)
IndexedDecisionReceiptSourceOwnership ==
  \A decision \in IndexedDecisionEvidence:
    \E sourceContext \in JoinedContexts:
      decision \in IndexedCurrentDecisions(sourceContext)

(***************************************************************************
The exact chain-level target.  Application receipts are included explicitly;
AppliedRequiresDecision later reduces them to the corresponding durable
decision receipts.  The bounded terminal receipt slot MaxHeight + 1 is also
covered even though `decidedAt` has no cell for it.
***************************************************************************)
ExactPerSlotDurableCommitReceiptSubjectAgreement ==
  \A left, right \in DurableCommitReceiptEvidence:
    CommitReceiptSlot(left) = CommitReceiptSlot(right)
      => /\ left.qc.context = right.qc.context
         /\ left.qc.subject = right.qc.subject

THEOREM IndexedInitEstablishesDecisionReceiptSourceOwnership ==
  IndexedChainInit => IndexedDecisionReceiptSourceOwnership
BY IndexedChainInitHasEmptyCurrentReceiptUnion
   DEF IndexedDecisionReceiptSourceOwnership

THEOREM IndexedActionPreservesDecisionReceiptSourceOwnership ==
  IndexedDecisionReceiptSourceOwnership /\ IndexedChainNext
    => IndexedDecisionReceiptSourceOwnership'
BY Isa, JoinedMembershipIsMonotone
   DEF IndexedDecisionReceiptSourceOwnership,
       IndexedDecisionEvidence, IndexedCurrentDecisions,
       IndexedDecisions, IndexedChainNext,
       IndexedProductActionAt, IndexedReceiptClassification,
       IndexedReceiptFreeChainStutter,
       IndexedDecisionReceiptHandoff,
       IndexedApplicationReceiptHandoff,
       NewIndexedDecisionReceipt, NewIndexedApplicationReceipt,
       NoNewIndexedDurableReceipt

THEOREM IndexedStepPreservesDecisionReceiptSourceOwnership ==
  IndexedDecisionReceiptSourceOwnership
    /\ [IndexedChainNext]_IndexedChainVars
    => IndexedDecisionReceiptSourceOwnership'
PROOF
  <1>1. ASSUME IndexedDecisionReceiptSourceOwnership,
              [IndexedChainNext]_IndexedChainVars
         PROVE IndexedDecisionReceiptSourceOwnership'
    <2>1. CASE IndexedChainNext
      BY <1>1, <2>1,
         IndexedActionPreservesDecisionReceiptSourceOwnership
    <2>2. CASE UNCHANGED IndexedChainVars
      BY <1>1, <2>2, Isa
         DEF IndexedDecisionReceiptSourceOwnership,
             IndexedDecisionEvidence, IndexedCurrentDecisions,
             IndexedDecisions, IndexedChainVars
    <2> QED BY <1>1, <2>1, <2>2
  <1> QED BY <1>1

THEOREM IndexedChainSpecEstablishesDecisionReceiptSourceOwnership ==
  IndexedChainSpec => []IndexedDecisionReceiptSourceOwnership
PROOF
  <1>1. IndexedChainInit => IndexedDecisionReceiptSourceOwnership
    BY IndexedInitEstablishesDecisionReceiptSourceOwnership
  <1>2. IndexedDecisionReceiptSourceOwnership
           /\ [IndexedChainNext]_IndexedChainVars
           => IndexedDecisionReceiptSourceOwnership'
    BY IndexedStepPreservesDecisionReceiptSourceOwnership
  <1> QED BY <1>1, <1>2, PTL DEF IndexedChainSpec

(***************************************************************************
This is the state-level form of the one-height AgreementObligation used by the
chain bridge.  It mentions the exact durable decision records rather than the
global chain projection.
***************************************************************************)
THEOREM IndexedOneHeightDecisionReceiptsAgree ==
  IndexedEveryInstanceStrongInvariant
    => \A initialContext \in AdmissibleContextRecords:
         \A left, right \in IndexedCurrentDecisions(initialContext):
           left.qc.context = right.qc.context
             => left.qc.subject = right.qc.subject
BY Isa DEF IndexedEveryInstanceStrongInvariant,
           IndexedCurrentDecisions, IndexedDecisions,
           IndexedAsync!StrongInductiveInvariant,
           IndexedAsync!Safety, IndexedAsync!DecisionAgreement

THEOREM IndexedApplicationEvidenceIsDecisionEvidence ==
  IndexedCompositionInvariant
    => durableApplicationEvidence \subseteq durableDecisionEvidence
BY Isa DEF IndexedCompositionInvariant,
           Chain!ChainEpochInvariant,
           Chain!DurableApplicationEvidenceSound,
           Chain!ApplicationHasRecordedDecision

THEOREM JoinedContextsAtEqualHeightAreIdentical ==
  JoinedContextCertificationInvariant
    => \A leftContext, rightContext \in JoinedContexts:
         leftContext.height = rightContext.height
           => leftContext = rightContext
BY Isa DEF JoinedContextCertificationInvariant

THEOREM CompositionAndSourceOwnershipImplyExactReceiptAgreement ==
  IndexedCompositionInvariant
    /\ IndexedDecisionReceiptSourceOwnership
    => ExactPerSlotDurableCommitReceiptSubjectAgreement
PROOF
  <1>1. ASSUME IndexedCompositionInvariant,
              IndexedDecisionReceiptSourceOwnership,
              NEW left \in DurableCommitReceiptEvidence,
              NEW right \in DurableCommitReceiptEvidence,
              CommitReceiptSlot(left) = CommitReceiptSlot(right)
         PROVE /\ left.qc.context = right.qc.context
               /\ left.qc.subject = right.qc.subject
    <2>1. durableApplicationEvidence
             \subseteq durableDecisionEvidence
      BY <1>1, IndexedApplicationEvidenceIsDecisionEvidence
    <2>2. /\ left \in durableDecisionEvidence
           /\ right \in durableDecisionEvidence
      BY <1>1, <2>1 DEF DurableCommitReceiptEvidence
    <2>3. durableDecisionEvidence = IndexedDecisionEvidence
      BY <1>1
         DEF IndexedCompositionInvariant,
             IndexedTotalReceiptProjection,
             IndexedDecisionReceiptProjection
    <2>4. /\ left \in IndexedDecisionEvidence
           /\ right \in IndexedDecisionEvidence
      BY <2>2, <2>3
    <2>5. PICK leftContext \in JoinedContexts:
             left \in IndexedCurrentDecisions(leftContext)
      BY <1>1, <2>4 DEF IndexedDecisionReceiptSourceOwnership
    <2>6. PICK rightContext \in JoinedContexts:
             right \in IndexedCurrentDecisions(rightContext)
      BY <1>1, <2>4 DEF IndexedDecisionReceiptSourceOwnership
    <2>7. /\ left.qc.context = leftContext
           /\ right.qc.context = rightContext
      BY <2>5, <2>6 DEF IndexedCurrentDecisions
    <2>8. leftContext.height = rightContext.height
      BY <1>1, <2>7, SMT DEF CommitReceiptSlot
    <2>9. JoinedContextCertificationInvariant
      BY <1>1 DEF IndexedCompositionInvariant
    <2>10. leftContext = rightContext
      BY <2>5, <2>6, <2>8, <2>9,
         JoinedContextsAtEqualHeightAreIdentical
    <2>11. left.qc.context = right.qc.context
      BY <2>7, <2>10
    <2>12. IndexedEveryInstanceStrongInvariant
      BY <1>1 DEF IndexedCompositionInvariant
    <2>13. left.qc.subject = right.qc.subject
      BY <2>5, <2>6, <2>10, <2>11, <2>12,
         IndexedOneHeightDecisionReceiptsAgree
    <2> QED BY <2>11, <2>13
  <1> QED BY <1>1

THEOREM IndexedChainSpecEstablishesExactPerSlotReceiptAgreement ==
  IndexedChainSpec
    => []ExactPerSlotDurableCommitReceiptSubjectAgreement
PROOF
  <1>1. IndexedChainSpec => []IndexedCompositionInvariant
    BY IndexedChainSpecEstablishesCompositionInvariant
  <1>2. IndexedChainSpec
           => []IndexedDecisionReceiptSourceOwnership
    BY IndexedChainSpecEstablishesDecisionReceiptSourceOwnership
  <1>3. IndexedCompositionInvariant
           /\ IndexedDecisionReceiptSourceOwnership
           => ExactPerSlotDurableCommitReceiptSubjectAgreement
    BY CompositionAndSourceOwnershipImplyExactReceiptAgreement
  <1> QED BY <1>1, <1>2, <1>3, PTL

=============================================================================
