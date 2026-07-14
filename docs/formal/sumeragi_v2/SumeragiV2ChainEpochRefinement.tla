---- MODULE SumeragiV2ChainEpochRefinement ----
EXTENDS SumeragiV2AsyncNetwork, TLAPS

(***************************************************************************
Total synchronous refinement from the asynchronous production corridor to
the per-validator chain/epoch abstraction.

The two abstract evidence variables are substituted by the complete concrete
`decisions` and `applied` sets.  PersistDecision and ApplyDecision are fused
with their history transition in SumeragiV2AsyncNetwork, so there is no later
optional recorder that could omit a conflicting durable receipt.  A first
canonical decision certifies the next slot, later decisions for that exact
slot take RecordKnownDecision, one validator's first contiguous application
takes RecordAppliedNext, and every duplicate/Byzantine/terminal receipt takes
RecordKnownApplication.  All other concrete actions stutter this projection.
***************************************************************************)

Chain == INSTANCE SumeragiV2ChainEpoch
  WITH certifiedHeight <- asyncCertifiedHeight,
       decidedAt <- asyncDecidedAt,
       nodeHeight <- asyncNodeHeight,
       nodeContext <- asyncNodeContext,
       durableDecisionEvidence <- asyncDurableDecisionEvidence,
       durableApplicationEvidence <- asyncDurableApplicationEvidence

ChainProof == INSTANCE SumeragiV2ChainEpochProofs
  WITH certifiedHeight <- asyncCertifiedHeight,
       decidedAt <- asyncDecidedAt,
       nodeHeight <- asyncNodeHeight,
       nodeContext <- asyncNodeContext,
       durableDecisionEvidence <- asyncDurableDecisionEvidence,
       durableApplicationEvidence <- asyncDurableApplicationEvidence

CoreProof == INSTANCE SumeragiV2Proofs

ConcreteDecision(request) ==
  [node |-> request.node, qc |-> request.qc]

ConcreteApplication(node, qc) ==
  [node |-> node, qc |-> qc]

TotalConcreteReceiptProjection ==
  /\ asyncDurableDecisionEvidence = decisions
  /\ asyncDurableApplicationEvidence = applied

ProjectedChainEpochSpec ==
  Chain!ChainEpochInit
    /\ [][Chain!ChainEpochNext]_Chain!ChainEpochVars

THEOREM AsyncInitRefinesChainEpochInit ==
  AsyncInit => Chain!ChainEpochInit
BY DEF AsyncInit, Chain!ChainEpochInit,
       asyncDurableDecisionEvidence, asyncDurableApplicationEvidence

(***************************************************************************
Concrete CommitQC receipts satisfy the abstract historical certificate
contract.  These are the only semantic bridge lemmas; all remaining mapping
proofs are exact record/set updates.
***************************************************************************)
THEOREM PendingDecisionIsDurableCommitEvidence ==
  \A request:
    StrongInductiveInvariant /\ request \in pendingDecision
      => Chain!DurableCommitDecision(ConcreteDecision(request))
BY IsaMT("blast", 180)
   DEF StrongInductiveInvariant, Safety, TypeInvariant,
       ReducerProvenanceInvariant, PendingCertificateWritesAuthorized,
       CertificatesBackedByIntents, HistoricalQcValid,
       Chain!DurableCommitDecision, Chain!HistoricalCommitCertificate,
       Chain!DecisionEvidenceSet, ConcreteDecision, DecisionWalSet,
       QcRecordSet

THEOREM RecordedApplicationIsDurableCommitEvidence ==
  \A node \in ValidatorIds, qc \in DecisionQcValues:
    StrongInductiveInvariant /\ ConcreteApplication(node, qc) \in decisions
      => Chain!DurableCommitDecision(ConcreteApplication(node, qc))
BY IsaMT("blast", 180)
   DEF StrongInductiveInvariant, Safety, TypeInvariant, DecisionAgreement,
       ReducerProvenanceInvariant, CertificatesBackedByIntents,
       HistoricalQcValid, Chain!DurableCommitDecision,
       Chain!HistoricalCommitCertificate, Chain!DecisionEvidenceSet,
       ConcreteApplication, QcRecordSet

(***************************************************************************
Each synchronous concrete persistence wrapper maps to exactly one of the two
abstract decision-receipt actions.
***************************************************************************)
THEOREM CertifiedDecisionReceiptRefinesChain ==
  \A request:
    /\ StrongInductiveInvariant
    /\ AsyncPersistDecisionStep(request)
    /\ RecordCertifiedNext(request.qc)
      => Chain!RecordCertifiedNext(ConcreteDecision(request))
BY PendingDecisionIsDurableCommitEvidence, IsaMT("blast", 180)
   DEF AsyncPersistDecisionStep, RecordCertifiedNext,
       CanonicalAsyncContext, AsyncHistoryThrough,
       PersistDecision, ConcreteDecision,
       Chain!RecordCertifiedNext, Chain!HistoryThrough,
       asyncDurableDecisionEvidence, asyncDurableApplicationEvidence

THEOREM KnownDecisionReceiptRefinesChain ==
  \A request:
    /\ StrongInductiveInvariant
    /\ AsyncPersistDecisionStep(request)
    /\ RecordKnownDecision(request.qc)
      => Chain!RecordKnownDecision(ConcreteDecision(request))
BY PendingDecisionIsDurableCommitEvidence, IsaMT("blast", 180)
   DEF AsyncPersistDecisionStep, RecordKnownDecision,
       CanonicalAsyncContext, AsyncHistoryThrough,
       PersistDecision, ConcreteDecision,
       Chain!RecordKnownDecision, Chain!DecisionBacksCertifiedSlot,
       Chain!ReceiptOutsideChainHorizon, Chain!CanonicalCommitForSlot,
       Chain!HistoryThrough, Chain!DecisionSlots,
       asyncDurableDecisionEvidence, asyncDurableApplicationEvidence

THEOREM PersistDecisionReceiptRefinesChain ==
  \A request:
    StrongInductiveInvariant /\ AsyncPersistDecisionStep(request)
      => Chain!ChainEpochNext
BY CertifiedDecisionReceiptRefinesChain,
   KnownDecisionReceiptRefinesChain
   DEF AsyncPersistDecisionStep, Chain!ChainEpochNext,
       Chain!DecisionEvidenceSet, ConcreteDecision, DecisionWalSet

(***************************************************************************
Every concrete ApplyDecision receipt is also mapped synchronously.  Only the
first exact contiguous honest receipt advances that validator's local prefix;
known, Byzantine, and bounded-horizon receipts remain visible in the total
application evidence set without moving another validator.
***************************************************************************)
THEOREM AppliedDecisionReceiptRefinesChain ==
  \A node \in ValidatorIds, qc \in DecisionQcValues:
    /\ StrongInductiveInvariant
    /\ AsyncApplyDecisionStep(node, qc)
    /\ RecordAppliedNext(node, qc)
      => Chain!RecordAppliedNext(ConcreteApplication(node, qc))
BY RecordedApplicationIsDurableCommitEvidence, IsaMT("blast", 180)
   DEF AsyncApplyDecisionStep, RecordAppliedNext,
       CanonicalAsyncContext, AsyncHistoryThrough,
       ApplyDecision, ConcreteApplication,
       Chain!RecordAppliedNext, Chain!CanonicalCommitForSlot,
       Chain!ApplicationHasRecordedDecision, Chain!HistoryThrough,
       Chain!DecisionSlots,
       asyncDurableDecisionEvidence, asyncDurableApplicationEvidence

THEOREM KnownApplicationReceiptRefinesChain ==
  \A node \in ValidatorIds, qc \in DecisionQcValues:
    /\ StrongInductiveInvariant
    /\ AsyncApplyDecisionStep(node, qc)
    /\ RecordKnownApplication(node, qc)
      => Chain!RecordKnownApplication(ConcreteApplication(node, qc))
BY RecordedApplicationIsDurableCommitEvidence, IsaMT("blast", 180)
   DEF AsyncApplyDecisionStep, RecordKnownApplication,
       CanonicalAsyncContext, AsyncHistoryThrough,
       ApplyDecision, ConcreteApplication,
       Chain!RecordKnownApplication,
       Chain!DecisionBacksCertifiedSlot,
       Chain!ReceiptOutsideChainHorizon, Chain!CanonicalCommitForSlot,
       Chain!ApplicationHasRecordedDecision, Chain!HistoryThrough,
       Chain!DecisionSlots,
       asyncDurableDecisionEvidence, asyncDurableApplicationEvidence

THEOREM ApplyDecisionReceiptRefinesChain ==
  \A node \in ValidatorIds, qc \in DecisionQcValues:
    StrongInductiveInvariant /\ AsyncApplyDecisionStep(node, qc)
      => Chain!ChainEpochNext
BY AppliedDecisionReceiptRefinesChain,
   KnownApplicationReceiptRefinesChain
   DEF AsyncApplyDecisionStep, Chain!ChainEpochNext,
       Chain!DecisionEvidenceSet, ConcreteApplication

THEOREM AsyncHistoryNextRefinesChainEpochNext ==
  StrongInductiveInvariant /\ AsyncHistoryNext
    => Chain!ChainEpochNext
BY PersistDecisionReceiptRefinesChain, ApplyDecisionReceiptRefinesChain
   DEF AsyncHistoryNext

(***************************************************************************
The asynchronous corridor projects to the original reducer specification, so
the already-discharged strong invariant is available at every concrete step.
***************************************************************************)
THEOREM AsyncHistoryStepIsCoreStep ==
  AsyncHistoryNext => NextV2
BY DEF AsyncHistoryNext, AsyncPersistDecisionStep,
       AsyncApplyDecisionStep, NextV2, Next

THEOREM AsyncEnvironmentLeavesCoreAndHistory ==
  AsyncEnvironmentNext => UNCHANGED <<vars, AsyncHistoryVars>>
BY IsaM("blast")
   DEF AsyncEnvironmentNext, SetAsyncGST, PreGstDrop,
       PreGstDuplicate, PreGstCrash, PostGstByzantineFlood

THEOREM AsyncNextRefinesCoreStep ==
  [AsyncNext]_AsyncAllVars => [NextV2]_vars
PROOF
  <1>1. ASSUME [AsyncNext]_AsyncAllVars
         PROVE [NextV2]_vars
    <2>1. CASE UNCHANGED AsyncAllVars
      BY <2>1 DEF AsyncAllVars
    <2>2. CASE AsyncProtocolStep
      BY <2>2 DEF AsyncProtocolStep
    <2>3. CASE AsyncHistoryNext
      BY <2>3, AsyncHistoryStepIsCoreStep
    <2>4. CASE AsyncEnvironmentNext
      BY <2>4, AsyncEnvironmentLeavesCoreAndHistory
    <2>5. CASE /\ AsyncRunLoopStep
                 /\ UNCHANGED <<vars, AsyncHistoryVars>>
      BY <2>5
    <2>6. \/ UNCHANGED AsyncAllVars
          \/ AsyncProtocolStep
          \/ AsyncHistoryNext
          \/ AsyncEnvironmentNext
          \/ /\ AsyncRunLoopStep
               /\ UNCHANGED <<vars, AsyncHistoryVars>>
      BY <1>1 DEF AsyncNext
    <2> QED BY <2>1, <2>2, <2>3, <2>4, <2>5, <2>6
  <1> QED BY <1>1

THEOREM AsyncSpecRefinesCoreSpec ==
  AsyncSpec => Spec
PROOF
  <1>1. AsyncInit => Init
    BY DEF AsyncInit
  <1>2. [AsyncNext]_AsyncAllVars => [NextV2]_vars
    BY AsyncNextRefinesCoreStep
  <1> QED BY <1>1, <1>2, PTL DEF AsyncSpec, Spec

THEOREM AsyncSpecHasStrongInductiveInvariant ==
  AsyncSpec => []StrongInductiveInvariant
BY AsyncSpecRefinesCoreSpec,
   CoreProof!SpecImpliesAlwaysStrongInductiveInvariant, PTL

(***************************************************************************
Non-receipt actions stutter the complete abstract projection, including the
two total concrete evidence sets.
***************************************************************************)
THEOREM AsyncEnvironmentLeavesChainProjection ==
  AsyncEnvironmentNext => UNCHANGED Chain!ChainEpochVars
BY AsyncEnvironmentLeavesCoreAndHistory
   DEF Chain!ChainEpochVars, asyncDurableDecisionEvidence,
       asyncDurableApplicationEvidence, AsyncHistoryVars, vars

THEOREM AsyncStepRefinesChainEpochStep ==
  StrongInductiveInvariant /\ [AsyncNext]_AsyncAllVars
    => [Chain!ChainEpochNext]_Chain!ChainEpochVars
PROOF
  <1>1. ASSUME StrongInductiveInvariant, [AsyncNext]_AsyncAllVars
         PROVE [Chain!ChainEpochNext]_Chain!ChainEpochVars
    <2>1. CASE UNCHANGED AsyncAllVars
      BY <2>1 DEF AsyncAllVars, Chain!ChainEpochVars,
                    asyncDurableDecisionEvidence,
                    asyncDurableApplicationEvidence
    <2>2. CASE AsyncProtocolStep
      BY <2>2 DEF AsyncProtocolStep, AsyncHistoryVars,
                    Chain!ChainEpochVars,
                    asyncDurableDecisionEvidence,
                    asyncDurableApplicationEvidence
    <2>3. CASE AsyncHistoryNext
      BY <1>1, <2>3, AsyncHistoryNextRefinesChainEpochNext
    <2>4. CASE AsyncEnvironmentNext
      BY <2>4, AsyncEnvironmentLeavesChainProjection
    <2>5. CASE /\ AsyncRunLoopStep
                 /\ UNCHANGED <<vars, AsyncHistoryVars>>
      BY <2>5 DEF Chain!ChainEpochVars,
                    asyncDurableDecisionEvidence,
                    asyncDurableApplicationEvidence,
                    AsyncHistoryVars, vars
    <2>6. \/ UNCHANGED AsyncAllVars
          \/ AsyncProtocolStep
          \/ AsyncHistoryNext
          \/ AsyncEnvironmentNext
          \/ /\ AsyncRunLoopStep
               /\ UNCHANGED <<vars, AsyncHistoryVars>>
      BY <1>1 DEF AsyncNext
    <2> QED BY <2>1, <2>2, <2>3, <2>4, <2>5, <2>6
  <1> QED BY <1>1

THEOREM AsyncSpecRefinesChainEpochSpec ==
  AsyncSpec => ProjectedChainEpochSpec
PROOF
  <1>1. AsyncInit => Chain!ChainEpochInit
    BY AsyncInitRefinesChainEpochInit
  <1>2. AsyncSpec => []StrongInductiveInvariant
    BY AsyncSpecHasStrongInductiveInvariant
  <1>3. StrongInductiveInvariant /\ [AsyncNext]_AsyncAllVars
           => [Chain!ChainEpochNext]_Chain!ChainEpochVars
    BY AsyncStepRefinesChainEpochStep
  <1> QED BY <1>1, <1>2, <1>3, PTL
           DEF AsyncSpec, ProjectedChainEpochSpec

(***************************************************************************
Stable release-level theorem: the projected receipt logs are exactly, not a
subset of, the concrete durable state, and the whole async specification
refines the receipt-driven chain specification.
***************************************************************************)
THEOREM TotalConcreteDurableReceiptRefinement ==
  AsyncSpec
    => /\ ProjectedChainEpochSpec
       /\ []TotalConcreteReceiptProjection
BY AsyncSpecRefinesChainEpochSpec, PTL
   DEF TotalConcreteReceiptProjection,
       asyncDurableDecisionEvidence, asyncDurableApplicationEvidence

THEOREM AsyncChainPrefixAndEpochSafety ==
  AsyncSpec => []Chain!ChainEpochSafety
PROOF
  <1>1. AsyncSpec => ProjectedChainEpochSpec
    BY AsyncSpecRefinesChainEpochSpec
  <1>2. ProjectedChainEpochSpec => []Chain!ChainEpochSafety
    BY ChainProof!ChainPrefixAndEpochSafety
  <1> QED BY <1>1, <1>2, PTL

THEOREM AsyncHistoriesArePrefixComparable ==
  AsyncSpec => []Chain!HistoryPrefixComparable
PROOF
  <1>1. Chain!ChainEpochSafety => Chain!HistoryPrefixComparable
    BY DEF Chain!ChainEpochSafety
  <1> QED BY <1>1, AsyncChainPrefixAndEpochSafety, PTL

THEOREM AsyncEpochRoutingIsFrozen ==
  AsyncSpec
    => [](/\ Chain!PerNodeFrozenEpoch
          /\ Chain!PerNodeParentFinality
          /\ Chain!ForeignLineageRejected)
PROOF
  <1>1. Chain!ChainEpochSafety
           => /\ Chain!PerNodeFrozenEpoch
              /\ Chain!PerNodeParentFinality
              /\ Chain!ForeignLineageRejected
    BY DEF Chain!ChainEpochSafety
  <1> QED BY <1>1, AsyncChainPrefixAndEpochSafety, PTL

THEOREM ConcreteCertificationHasNoGlobalApplicationBarrier ==
  \A request:
    AsyncPersistDecisionStep(request) /\ RecordCertifiedNext(request.qc)
      => /\ asyncCertifiedHeight' = asyncCertifiedHeight + 1
         /\ UNCHANGED <<asyncNodeHeight, asyncNodeContext>>
BY DEF AsyncPersistDecisionStep, RecordCertifiedNext

(***************************************************************************
Stable release-level theorem: certification never waits for or rewrites a
global applied frontier, while every honest validator's independently applied
history remains prefix-comparable and carries the frozen epoch/parent identity.
***************************************************************************)
THEOREM NoGlobalBarrierPerNodePrefixAndEpochSafety ==
  /\ AsyncSpec
       => [](/\ Chain!HistoryPrefixComparable
             /\ Chain!PerNodeFrozenEpoch
             /\ Chain!PerNodeParentFinality
             /\ Chain!ForeignLineageRejected)
  /\ \A request:
       AsyncPersistDecisionStep(request) /\ RecordCertifiedNext(request.qc)
         => /\ asyncCertifiedHeight' = asyncCertifiedHeight + 1
            /\ UNCHANGED <<asyncNodeHeight, asyncNodeContext>>
PROOF
  <1>1. AsyncSpec
           => [](/\ Chain!HistoryPrefixComparable
                 /\ Chain!PerNodeFrozenEpoch
                 /\ Chain!PerNodeParentFinality
                 /\ Chain!ForeignLineageRejected)
    BY AsyncHistoriesArePrefixComparable, AsyncEpochRoutingIsFrozen, PTL
  <1>2. \A request:
           AsyncPersistDecisionStep(request) /\ RecordCertifiedNext(request.qc)
             => /\ asyncCertifiedHeight' = asyncCertifiedHeight + 1
                /\ UNCHANGED <<asyncNodeHeight, asyncNodeContext>>
    BY ConcreteCertificationHasNoGlobalApplicationBarrier
  <1> QED BY <1>1, <1>2

=============================================================================
