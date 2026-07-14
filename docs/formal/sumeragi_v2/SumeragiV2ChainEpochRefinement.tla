---- MODULE SumeragiV2ChainEpochRefinement ----
EXTENDS SumeragiV2AsyncNetwork, TLAPS

(***************************************************************************
Selected-height synchronous product.

SumeragiV2AsyncNetwork is deliberately one frozen Core instance.  This module
does not manufacture a second chain-progress relation beside that instance.
Instead it adds the canonical ChainEpoch variables and pairs every new Core
decision/application receipt with exactly one ChainEpoch receipt transition.
Every Async step with no new durable receipt stutters the complete ChainEpoch
state.  Thus certification and local application are causally backed by the
exact production receipt delta, while historical service remains available.

This product is a safety refinement for the selected frozen height.  Once an
honest node applies that height, its successor nodeContext names another Core
instance that is not present in SumeragiV2AsyncNetwork.  Consequently this
module intentionally makes no successor-height liveness claim; an indexed
family of AsyncSpecAt instances (or a discharged universal induction over
them) is the explicit remaining composition seam.
***************************************************************************)

VARIABLES
  certifiedHeight,
  decidedAt,
  nodeHeight,
  nodeContext,
  durableDecisionEvidence,
  durableApplicationEvidence

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

SuccessorInstanceSeam ==
  \E node \in Honest: NeedsSuccessorAsyncInstance(node)

=============================================================================
