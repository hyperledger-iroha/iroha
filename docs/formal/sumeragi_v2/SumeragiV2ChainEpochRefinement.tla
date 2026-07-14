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
  historicalCatchUpApplications

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

(***************************************************************************
The concrete AsyncChainSpec begins at genesis and can carry each responsive
validator across that instance's application boundary into its exact first
successor context.  This formula records only that genesis handoff; it does
not start the successor AsyncSpecAt instance or claim indexed height progress.

The proof ledger records this narrower theorem as specified_unproved. The
separate HeightLivenessObligation below targets the indexed composition; this
genesis theorem is not used as a substitute for that multi-height induction.
***************************************************************************)
GenesisHeightSuccessorHandoffProperty ==
  \A node \in AsyncCurrentResponsiveVoters:
    gst ~> NeedsSuccessorAsyncInstance(node)

THEOREM GenesisHeightSuccessorHandoffObligation ==
  AsyncChainSpec => GenesisHeightSuccessorHandoffProperty


(***************************************************************************
Authoritative indexed successor-instance product.

Every admissible frozen ContextRecord owns one pre-created, dormant copy of the
complete AsyncAllVars tuple. IndexedAsync is an actual parameterized instance
of the one-height asynchronous proof module; there is no shadow consensus
relation. ContextRecords with an invalid lineage are outside this domain
because AsyncInitAt rejects them as well.
A context becomes live when its first validator joins after an exact durable
application receipt. Validators join independently, and RunNode is gated only
by that validator's current nodeContext. Thus an early validator may execute
without waiting for its peers. Joined membership is monotone, so old instances
remain available to RunHistoricalServer after validators advance.

The nested tuple layout is exactly <<vars, AsyncSchedulerVars>>: 45 Core
components followed by 29 scheduler/transport components. Shape predicates
exclude unmodelled fields and make every instance projection extensional.
***************************************************************************)
IndexedCore(initialContext, component) ==
  indexedAsyncState[initialContext][1][component]

IndexedScheduler(initialContext, component) ==
  indexedAsyncState[initialContext][2][component]

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
    IndexedCore(initialContext, 45),
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
       validatedBodies <- IndexedCore(initialContext, 10),
       invalidBodies <- IndexedCore(initialContext, 11),
       seenProposals <- IndexedCore(initialContext, 12),
       receivedVotes <- IndexedCore(initialContext, 13),
       receivedQCs <- IndexedCore(initialContext, 14),
       receivedTimeoutVotes <- IndexedCore(initialContext, 15),
       receivedTCs <- IndexedCore(initialContext, 16),
       proposalIntents <- IndexedCore(initialContext, 17),
       prepareIntents <- IndexedCore(initialContext, 18),
       commitIntents <- IndexedCore(initialContext, 19),
       timeoutIntents <- IndexedCore(initialContext, 20),
       prepareQCs <- IndexedCore(initialContext, 21),
       commitQCs <- IndexedCore(initialContext, 22),
       formedTCs <- IndexedCore(initialContext, 23),
       installedTCs <- IndexedCore(initialContext, 24),
       lockRank <- IndexedCore(initialContext, 25),
       lockSubject <- IndexedCore(initialContext, 26),
       highestRank <- IndexedCore(initialContext, 27),
       highestSubject <- IndexedCore(initialContext, 28),
       pendingProposal <- IndexedCore(initialContext, 29),
       pendingPrepare <- IndexedCore(initialContext, 30),
       pendingObservePrepare <- IndexedCore(initialContext, 31),
       pendingLockCommit <- IndexedCore(initialContext, 32),
       pendingTimeout <- IndexedCore(initialContext, 33),
       pendingInstallTC <- IndexedCore(initialContext, 34),
       pendingDecision <- IndexedCore(initialContext, 35),
       signProposals <- IndexedCore(initialContext, 36),
       signVotes <- IndexedCore(initialContext, 37),
       signTimeouts <- IndexedCore(initialContext, 38),
       proposalNetwork <- IndexedCore(initialContext, 39),
       voteNetwork <- IndexedCore(initialContext, 40),
       qcNetwork <- IndexedCore(initialContext, 41),
       timeoutNetwork <- IndexedCore(initialContext, 42),
       tcNetwork <- IndexedCore(initialContext, 43),
       decisions <- IndexedCore(initialContext, 44),
       applied <- IndexedCore(initialContext, 45),
       asyncNow <- IndexedScheduler(initialContext, 1),
       asyncCommandQueues <- IndexedScheduler(initialContext, 2),
       asyncFifoOwed <- IndexedScheduler(initialContext, 3),
       asyncTimeoutEmitted <- IndexedScheduler(initialContext, 4),
       asyncRunnerPhase <- IndexedScheduler(initialContext, 5),
       asyncRunnerBudget <- IndexedScheduler(initialContext, 6),
       asyncIoQueues <- IndexedScheduler(initialContext, 7),
       asyncOutstandingWork <- IndexedScheduler(initialContext, 8),
       asyncIoReadyCompletions <- IndexedScheduler(initialContext, 9),
       asyncLocalReadyCompletions <- IndexedScheduler(initialContext, 10),
       asyncNextCompletionSource <- IndexedScheduler(initialContext, 11),
       asyncIoControlAvailable <- IndexedScheduler(initialContext, 12),
       asyncDeferredCompletionQueues <- IndexedScheduler(initialContext, 13),
       asyncDeferredProgressQueues <- IndexedScheduler(initialContext, 14),
       asyncDeferredNormalQueues <- IndexedScheduler(initialContext, 15),
       asyncDeferredDrainOwed <- IndexedScheduler(initialContext, 16),
       asyncCausalQueues <- IndexedScheduler(initialContext, 17),
       asyncOutstandingTags <- IndexedScheduler(initialContext, 18),
       asyncNodeDeadlines <- IndexedScheduler(initialContext, 19),
       asyncRetransmitDeadlines <- IndexedScheduler(initialContext, 20),
       asyncNodeServiceDeadlines <- IndexedScheduler(initialContext, 21),
       asyncIoServiceDeadlines <- IndexedScheduler(initialContext, 22),
       asyncSentItems <- IndexedScheduler(initialContext, 23),
       asyncRetainedControl <- IndexedScheduler(initialContext, 24),
       asyncActiveRequests <- IndexedScheduler(initialContext, 25),
       asyncTransport <- IndexedScheduler(initialContext, 26),
       asyncIngressLanes <- IndexedScheduler(initialContext, 27),
       asyncIngressReady <- IndexedScheduler(initialContext, 28),
       asyncHeldChunks <- IndexedScheduler(initialContext, 29)

AdmissibleContextRecords ==
  {initialContext \in ContextRecords:
     FrozenContextAdmissible(initialContext)}

IndexedAsyncStateShape ==
  /\ DOMAIN indexedAsyncState = AdmissibleContextRecords
  /\ \A initialContext \in AdmissibleContextRecords:
       /\ Len(indexedAsyncState[initialContext]) = 2
       /\ Len(indexedAsyncState[initialContext][1]) = 45
       /\ Len(indexedAsyncState[initialContext][2]) = 29

JoinedByContextShape ==
  joinedByContext \in [AdmissibleContextRecords -> SUBSET ValidatorIds]

GenesisContext == ContextRecord(0, <<>>)

JoinedContexts ==
  {initialContext \in AdmissibleContextRecords:
     joinedByContext[initialContext] # {}}

IndexedNodeCurrentAt(initialContext, node) ==
  /\ node \in joinedByContext[initialContext]
  /\ nodeContext[node] = initialContext

IndexedDecisions(initialContext) == IndexedCore(initialContext, 44)
IndexedApplications(initialContext) == IndexedCore(initialContext, 45)

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

IndexedDecisionReceiptProjection ==
  durableDecisionEvidence = IndexedDecisionEvidence

IndexedApplicationReceiptProjection ==
  durableApplicationEvidence = IndexedApplicationEvidence

IndexedTotalReceiptProjection ==
  /\ IndexedDecisionReceiptProjection
  /\ IndexedApplicationReceiptProjection

IndexedChainVars ==
  <<indexedAsyncState, joinedByContext,
    historicalCatchUpDecisions, historicalCatchUpApplications,
    Chain!ChainEpochVars>>

(***************************************************************************
The joined runner is a restriction of the exact AsyncNext relation, never an
alternate step. Current consensus work requires only the selected node's join.
Historical serving and outstanding IO remain enabled for every node that ever
joined the context, even after its authoritative nodeContext advances.
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
     \/ \E node \in IndexedAsync(initialContext)!AsyncCurrentResponsiveVoters:
          /\ node \in joinedByContext[initialContext]
          /\ IndexedAsync(initialContext)!ServiceIoWorker(node)
     \/ \E node \in IndexedAsync(initialContext)!AsyncCurrentResponsiveVoters:
          /\ node \in joinedByContext[initialContext]
          /\ IndexedAsync(initialContext)!EnqueueIoLocalControl(node)
     \/ IndexedAsync(initialContext)!AsyncNetworkStep
     \/ IndexedAsync(initialContext)!AsyncFaultStep
  /\ UNCHANGED IndexedScheduler(initialContext, 21)

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
  /\ UNCHANGED joinedByContext
  /\ \/ Chain!RecordCertifiedNext(decision)
     \/ Chain!RecordKnownDecision(decision)

SuccessorContextFor(application) ==
  LET nextHeight == nodeHeight[application.node] + 1
      nextLineage == Chain!HistoryThrough(nextHeight)
  IN Chain!ContextRecord(nextHeight, nextLineage)

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
        /\ joinedByContext' =
             [joinedByContext EXCEPT
                ![SuccessorContextFor(application)] =
                  @ \cup {application.node}]
     \/ /\ Chain!RecordKnownApplication(application)
        /\ UNCHANGED joinedByContext

IndexedReceiptFreeChainStutter(initialContext) ==
  /\ NoNewIndexedDurableReceipt(initialContext)
  /\ UNCHANGED <<joinedByContext, Chain!ChainEpochVars>>

IndexedReceiptClassification(initialContext) ==
  \/ IndexedReceiptFreeChainStutter(initialContext)
  \/ \E decision \in Chain!DecisionEvidenceSet:
       IndexedDecisionReceiptHandoff(initialContext, decision)
  \/ \E application \in Chain!DecisionEvidenceSet:
       IndexedApplicationReceiptHandoff(initialContext, application)

(***************************************************************************
Authenticated historical catch-up.

Production `V2BlockSyncDiscovery` signs a request for the lagging node's exact
frozen context. `V2BlockSyncServer` may answer only from canonical Kura history,
and the historical body server must be one of the CommitQC's certified signers.
The response then enters the ordinary reducer: first the CommitQC is durably
recorded, then its exact body is stored, deterministically validated, applied,
and only that node joins the successor context.

The two actions below expose those two durable receipt boundaries. They do not
change any authoritative Async instance. A target must be responsive but absent
from the old context's voting roster, so catch-up cannot duplicate or race that
instance's own decision/application receipt. This includes a validator newly
entering the successor roster and also lets it traverse every intermediate
historical context after a longer roster absence. Exact context, subject, QC,
and signer-backed body identity are copied from already durable canonical
decision and application evidence; no synthetic finality artifact is created.
***************************************************************************)
HistoricalCatchUpRecord(node, source) ==
  [node |-> node, qc |-> source.qc]

(***************************************************************************
These are the exact projection-native expansions of `AsyncVotersAt` and
`NodeHasApplication` from SumeragiV2AsyncNetwork: the IndexedAsync WITH-clause
maps `applied` to IndexedCore(initialContext, 45) (IndexedApplications) and
`context` to IndexedCore(initialContext, 2).  Spelling the definitions here
keeps the parameterized proof INSTANCE out of later ENABLED obligations; no
roster, context, phase, or per-node application condition is weakened.
***************************************************************************)

HistoricalCatchUpTarget(initialContext, node) ==
  /\ initialContext.height < MaxHeight
  /\ node \in Responsive
  /\ node \notin Responsive \cap VotingRoster(initialContext.epoch)
  /\ nodeHeight[node] = initialContext.height
  /\ nodeContext[node] = initialContext
  /\ ~IndexedProjectedNodeHasApplication(initialContext, node)

HistoricalCatchUpSource(initialContext, server, source) ==
  /\ initialContext \in JoinedContexts
  /\ source \in IndexedCurrentDecisions(initialContext)
  /\ source \in IndexedCurrentApplications(initialContext)
  /\ source \in durableDecisionEvidence
  /\ source \in durableApplicationEvidence
  /\ Chain!CanonicalCommitForSlot(
       source.qc, initialContext.height + 1)
  /\ server \in source.qc.signers \cap Honest
  /\ server \in joinedByContext[initialContext]
  /\ BodyHeldBy(IndexedCore(initialContext, 9), server,
                 initialContext, source.qc.subject)

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
     /\ decision \notin IndexedDecisionEvidence
     /\ Chain!RecordKnownDecision(decision)
     /\ historicalCatchUpDecisions' =
          historicalCatchUpDecisions \cup {decision}
     /\ UNCHANGED <<historicalCatchUpApplications,
                     indexedAsyncState, joinedByContext>>
     /\ IndexedAsyncStateShape'
     /\ JoinedByContextShape'
     /\ HistoricalCatchUpShape'

IndexedHistoricalCatchUpApplication(initialContext, node, server, source) ==
  LET application == HistoricalCatchUpRecord(node, source)
  IN /\ HistoricalCatchUpTarget(initialContext, node)
     /\ HistoricalCatchUpSource(initialContext, server, source)
     /\ application \in historicalCatchUpDecisions
     /\ application \notin historicalCatchUpApplications
     /\ Chain!RecordAppliedNext(application)
     /\ historicalCatchUpApplications' =
          historicalCatchUpApplications \cup {application}
     /\ joinedByContext' =
          [joinedByContext EXCEPT
             ![SuccessorContextFor(application)] = @ \cup {node}]
     /\ UNCHANGED <<historicalCatchUpDecisions, indexedAsyncState>>
     /\ IndexedAsyncStateShape'
     /\ JoinedByContextShape'
     /\ HistoricalCatchUpShape'

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
  /\ UNCHANGED <<historicalCatchUpDecisions,
                  historicalCatchUpApplications>>
  /\ IndexedAsyncStateShape'
  /\ JoinedByContextShape'
  /\ HistoricalCatchUpShape'
  /\ IndexedReceiptClassification(initialContext)

IndexedChainNext ==
  /\ IndexedAsyncStateShape
  /\ JoinedByContextShape
  /\ HistoricalCatchUpShape
  /\ \/ \E initialContext \in JoinedContexts:
          IndexedProductActionAt(initialContext)
     \/ \E initialContext \in AdmissibleContextRecords,
           node \in ValidatorIds, server \in ValidatorIds,
           source \in Chain!DecisionEvidenceSet:
          IndexedHistoricalCatchUpDecision(
            initialContext, node, server, source)
     \/ \E initialContext \in AdmissibleContextRecords,
           node \in ValidatorIds, server \in ValidatorIds,
           source \in Chain!DecisionEvidenceSet:
          IndexedHistoricalCatchUpApplication(
            initialContext, node, server, source)

IndexedChainInit ==
  /\ IndexedAsyncStateShape
  /\ JoinedByContextShape
  /\ HistoricalCatchUpShape
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
  /\ IndexedTotalReceiptProjection

(***************************************************************************
Fairness is attached to full indexed-product steps. Dormant contexts make each
action disabled. After the first independent join, the instance scheduler and
transport become fair. Node-attributed consensus work is fair after that node
joins; no action tests whether every peer has joined.
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
These weak-fairness clauses are the explicit historical-service premise.
Once exact canonical source evidence and an honest signer-held body persist,
the signed CommitQC response cannot be postponed forever. Once that decision
receipt is local, the ordinary store/validate/apply reducer path cannot be
postponed forever either. No fairness is assumed for an unauthenticated or
noncanonical response.
***************************************************************************)
IndexedCatchUpServiceStep(initialContext, node) ==
  /\ IndexedAsyncStateShape
  /\ JoinedByContextShape
  /\ HistoricalCatchUpShape
  /\ \E server \in ValidatorIds,
       source \in Chain!DecisionEvidenceSet:
       IndexedHistoricalCatchUpDecision(
         initialContext, node, server, source)

IndexedCatchUpApplicationStep(initialContext, node) ==
  /\ IndexedAsyncStateShape
  /\ JoinedByContextShape
  /\ HistoricalCatchUpShape
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
           IndexedCatchUpServiceStep(initialContext, node))
    /\ \A node \in ValidatorIds:
         WF_IndexedChainVars(
           IndexedCatchUpApplicationStep(initialContext, node))

IndexedChainSpec ==
  /\ IndexedChainInit
  /\ [][IndexedChainNext]_IndexedChainVars
  /\ IndexedFairness

(***************************************************************************
Composition invariant.

Every joined context is a canonical prefix no higher than the globally
certified prefix. A node remains current in a joined instance until its exact
application receipt atomically advances ChainEpoch; thereafter that receipt
persists in the old instance. Conversely, every nonterminal current-context
application is reflected by a strictly greater global node height. The last
fact is the state bridge from one-height completion to indexed completion.
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

JoinedRoutingInvariant ==
  \A initialContext \in AdmissibleContextRecords:
    \A node \in joinedByContext[initialContext]:
      \/ IndexedNodeCurrentAt(initialContext, node)
      \/ /\ nodeHeight[node] > initialContext.height
         /\ \/ IndexedAsync(initialContext)!NodeHasApplication(node)
            \/ HistoricalCatchUpApplicationAt(initialContext, node)

IndexedApplicationsRespectNodeHeight ==
  \A initialContext \in AdmissibleContextRecords:
    \A node \in IndexedAsync(initialContext)!
                  AsyncVotersAt(initialContext):
      IndexedAsync(initialContext)!NodeHasApplication(node)
        => \/ initialContext.height = MaxHeight
           \/ nodeHeight[node] > initialContext.height

HistoricalCatchUpReceiptSound(receipt) ==
  \E initialContext \in AdmissibleContextRecords,
     server \in ValidatorIds,
     source \in Chain!DecisionEvidenceSet:
    /\ initialContext.height < MaxHeight
    /\ receipt.node \in Responsive
    /\ receipt.node \notin IndexedAsync(initialContext)!
                           AsyncVotersAt(initialContext)
    /\ HistoricalCatchUpSource(initialContext, server, source)
    /\ receipt = HistoricalCatchUpRecord(receipt.node, source)

HistoricalCatchUpEvidenceInvariant ==
  /\ \A decision \in historicalCatchUpDecisions:
       HistoricalCatchUpReceiptSound(decision)
  /\ \A initialContext \in AdmissibleContextRecords,
       node \in ValidatorIds:
       HistoricalCatchUpApplicationAt(initialContext, node)
         => nodeHeight[node] > initialContext.height

IndexedCompositionInvariant ==
  /\ IndexedAsyncStateShape
  /\ JoinedByContextShape
  /\ HistoricalCatchUpShape
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
           IndexedCore, IndexedScheduler

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
    <2> QED BY <2>2,
       IndexedAsync(initialContext)!
         InitAtEstablishesStrongInductiveInvariant
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
    <2> QED BY <2>1, <2>4,
       IndexedAsync(initialContext)!
         CoreStrongInductiveActionPreservation
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
             JoinedContexts, IndexedNodeCurrentAt,
             Chain!ChainEpochVars
    <2> QED BY <1>1, <2>1, <2>2
  <1> QED BY <1>1

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
atomic join update stays inside the pre-created function domain.
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
       IndexedHistoricalServerStep, IndexedIoWorkerStep,
       IndexedAdmitPacketStep, IndexedChainNext,
       IndexedProductActionAt, IndexedJoinedAsyncNext,
       IndexedJoinedNonCrashStep, IndexedJoinedRunnerStep,
       IndexedJoinedNonRunnerStep, IndexedNodeCurrentAt,
       IndexedAsync!PostGstRunNode,
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
IndexedHistoricalCatchUpProgressObligation is the remaining fairness-transfer
lemma for nodes absent from an old roster: exact canonical application and
signer-held body evidence enable authenticated block sync, and its two explicit
weak-fair actions must deliver the node's own application and successor join.
IndexedOneHeightCompletion is exactly the one-height completion property from
SumeragiV2LivenessProofs, imported through the parameterized asynchronous proof
instance. The remaining proof must compose those facts over finite Heights; it
must not assume them as a new protocol relation.
***************************************************************************)
IndexedAllResponsiveJoined(initialContext) ==
  IndexedAsync(initialContext)!AsyncVotersAt(initialContext)
    \subseteq joinedByContext[initialContext]

IndexedInstanceActivationObligation(initialContext) ==
  (/\ IndexedChainSpec
   /\ TRUE ~> IndexedAllResponsiveJoined(initialContext))
    => IndexedAsync(initialContext)!AsyncSpecAt(initialContext)

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
The two readiness predicates are exact enabledness witnesses for the two
authenticated catch-up actions.  The composition invariant supplies the
receipt projection and canonical-prefix facts required by RecordKnownDecision
and RecordAppliedNext; readiness contributes the particular honest signer,
body, target, and source.  These lemmas make the fairness transfer explicit
instead of treating the historical service as an abstract progress oracle.
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
     /\ decision \notin IndexedDecisionEvidence
     /\ Chain!DurableCommitDecision(decision)
     /\ decision \notin durableDecisionEvidence
     /\ \/ Chain!DecisionBacksCertifiedSlot(decision)
        \/ Chain!ReceiptOutsideChainHorizon(decision)

THEOREM IndexedCatchUpDecisionReadySuppliesActionGuard ==
  \A initialContext \in AdmissibleContextRecords,
     node \in ValidatorIds:
    IndexedCatchUpFrameInvariant
      /\ IndexedCatchUpDecisionReady(initialContext, node)
      => \E server \in ValidatorIds,
            source \in Chain!DecisionEvidenceSet:
           IndexedCatchUpDecisionActionGuard(
             initialContext, node, server, source)
BY Isa DEF IndexedCatchUpFrameInvariant,
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

THEOREM IndexedCatchUpDecisionReadyEnablesService ==
  \A initialContext \in AdmissibleContextRecords,
     node \in ValidatorIds:
    IndexedCompositionInvariant
      /\ IndexedCatchUpDecisionReady(initialContext, node)
      => ENABLED
           <<IndexedCatchUpServiceStep(initialContext, node)>>_(
             IndexedChainVars)
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
              NEW node \in ValidatorIds,
              IndexedCompositionInvariant,
              IndexedCatchUpDecisionReady(initialContext, node)
         PROVE ENABLED
                 <<IndexedCatchUpServiceStep(initialContext, node)>>_(
                   IndexedChainVars)
    <2>1. IndexedCatchUpFrameInvariant
      BY <1>1, IndexedCompositionSuppliesCatchUpFrame
    <2>2. \E server \in ValidatorIds,
               source \in Chain!DecisionEvidenceSet:
             IndexedCatchUpDecisionActionGuard(
               initialContext, node, server, source)
      BY <1>1, <2>1, IndexedCatchUpDecisionReadySuppliesActionGuard
    <2> QED BY <2>1, <2>2,
                 IndexedCatchUpDecisionFrameGuardEnablesService
  <1> QED BY <1>1

IndexedCatchUpApplicationActionGuard(initialContext, node, server, source) ==
  LET application == HistoricalCatchUpRecord(node, source)
      nextHeight == nodeHeight[node] + 1
  IN /\ HistoricalCatchUpTarget(initialContext, node)
     /\ HistoricalCatchUpSource(initialContext, server, source)
     /\ application \in historicalCatchUpDecisions
     /\ application \notin historicalCatchUpApplications
     /\ application \in Chain!DecisionEvidenceSet
     /\ node \in Honest
     /\ Chain!DurableCommitDecision(application)
     /\ nodeHeight[node] < certifiedHeight
     /\ Chain!CanonicalCommitForSlot(application.qc, nextHeight)
     /\ Chain!ApplicationHasRecordedDecision(application)

THEOREM IndexedCatchUpApplicationReadySuppliesActionGuard ==
  \A initialContext \in AdmissibleContextRecords,
     node \in ValidatorIds:
    IndexedCatchUpFrameInvariant
      /\ IndexedCatchUpApplicationReady(initialContext, node)
      => \E server \in ValidatorIds,
            source \in Chain!DecisionEvidenceSet:
           IndexedCatchUpApplicationActionGuard(
             initialContext, node, server, source)
BY Isa DEF IndexedCatchUpFrameInvariant,
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

THEOREM IndexedCatchUpApplicationReadyEnablesApplication ==
  \A initialContext \in AdmissibleContextRecords,
     node \in ValidatorIds:
    IndexedCompositionInvariant
      /\ IndexedCatchUpApplicationReady(initialContext, node)
      => ENABLED
           <<IndexedCatchUpApplicationStep(initialContext, node)>>_(
             IndexedChainVars)
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
              NEW node \in ValidatorIds,
              IndexedCompositionInvariant,
              IndexedCatchUpApplicationReady(initialContext, node)
         PROVE ENABLED
                 <<IndexedCatchUpApplicationStep(initialContext, node)>>_(
                   IndexedChainVars)
    <2>1. IndexedCatchUpFrameInvariant
      BY <1>1, IndexedCompositionSuppliesCatchUpFrame
    <2>2. \E server \in ValidatorIds,
               source \in Chain!DecisionEvidenceSet:
             IndexedCatchUpApplicationActionGuard(
               initialContext, node, server, source)
      BY <1>1, <2>1, IndexedCatchUpApplicationReadySuppliesActionGuard
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
    IndexedCatchUpApplicationStep(initialContext, node)
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
      /\ IndexedCatchUpDecisionReady(initialContext, node)
      /\ [IndexedChainNext]_IndexedChainVars
      => \/ IndexedCatchUpDecisionReady(initialContext, node)'
         \/ HistoricalCatchUpDecisionAt(initialContext, node)'
BY Isa DEF IndexedCatchUpDecisionReady,
           HistoricalCatchUpDecisionAt,
           IndexedChainNext, IndexedChainVars,
           IndexedProductActionAt, IndexedReceiptClassification,
           IndexedReceiptFreeChainStutter,
           IndexedDecisionReceiptHandoff,
           IndexedApplicationReceiptHandoff,
           IndexedHistoricalCatchUpDecision,
           IndexedHistoricalCatchUpApplication,
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
      /\ IndexedCatchUpApplicationReady(initialContext, node)
      /\ [IndexedChainNext]_IndexedChainVars
      => \/ IndexedCatchUpApplicationReady(initialContext, node)'
         \/ (/\ HistoricalCatchUpApplicationAt(initialContext, node)'
             /\ nodeHeight'[node] > initialContext.height)
BY Isa DEF IndexedCatchUpApplicationReady,
           HistoricalCatchUpApplicationAt,
           IndexedChainNext, IndexedChainVars,
           IndexedProductActionAt, IndexedReceiptClassification,
           IndexedReceiptFreeChainStutter,
           IndexedDecisionReceiptHandoff,
           IndexedApplicationReceiptHandoff,
           IndexedHistoricalCatchUpDecision,
           IndexedHistoricalCatchUpApplication,
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

THEOREM IndexedHistoricalCatchUpProgressObligation ==
  IndexedChainSpec =>
    \A initialContext \in AdmissibleContextRecords,
       node \in ValidatorIds:
      /\ IndexedCatchUpDecisionReady(initialContext, node)
           ~> HistoricalCatchUpDecisionAt(initialContext, node)
      /\ IndexedCatchUpApplicationReady(initialContext, node)
           ~> (/\ HistoricalCatchUpApplicationAt(initialContext, node)
               /\ nodeHeight[node] > initialContext.height)
PROOF
  <1>1. ASSUME IndexedChainSpec,
              NEW initialContext \in AdmissibleContextRecords,
              NEW node \in ValidatorIds
         PROVE
           /\ IndexedCatchUpDecisionReady(initialContext, node)
                ~> HistoricalCatchUpDecisionAt(initialContext, node)
           /\ IndexedCatchUpApplicationReady(initialContext, node)
                ~> (/\ HistoricalCatchUpApplicationAt(initialContext, node)
                    /\ nodeHeight[node] > initialContext.height)
    <2>1. []IndexedCompositionInvariant
      BY <1>1, IndexedChainSpecEstablishesCompositionInvariant, PTL
    <2>2. [][IndexedChainNext]_IndexedChainVars
      BY <1>1, PTL DEF IndexedChainSpec
    <2>3. WF_IndexedChainVars(
             IndexedCatchUpServiceStep(initialContext, node))
      BY <1>1, PTL DEF IndexedChainSpec, IndexedFairness
    <2>4. WF_IndexedChainVars(
             IndexedCatchUpApplicationStep(initialContext, node))
      BY <1>1, PTL DEF IndexedChainSpec, IndexedFairness
    <2>5. IndexedCatchUpDecisionReady(initialContext, node)
             ~> HistoricalCatchUpDecisionAt(initialContext, node)
      BY <2>1, <2>2, <2>3,
         IndexedCatchUpDecisionReadyEnablesService,
         IndexedCatchUpServiceEstablishesDecision,
         IndexedCatchUpDecisionReadyPersistsUntilReceipt,
         PTL
    <2>6. IndexedCatchUpApplicationReady(initialContext, node)
             ~> (/\ HistoricalCatchUpApplicationAt(initialContext, node)
                 /\ nodeHeight[node] > initialContext.height)
      BY <2>1, <2>2, <2>4,
         IndexedCatchUpApplicationReadyEnablesApplication,
         IndexedCatchUpApplicationEstablishesAdvance,
         IndexedCatchUpApplicationReadyPersistsUntilAdvance,
         PTL
    <2> QED BY <2>5, <2>6
  <1> QED BY <1>1

IndexedOneHeightCompletion(initialContext) ==
  IndexedAsync(initialContext)!AsyncSpecAt(initialContext)
    => (IndexedCore(initialContext, 7)
          ~> IndexedAsync(initialContext)!
               AsyncAllResponsiveAppliedAt(initialContext))

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

IndexedHeightLivenessProperty ==
  \A initialContext \in AdmissibleContextRecords:
    (/\ initialContext \in JoinedContexts
     /\ IndexedCore(initialContext, 7))
      ~> IndexedContextCompleted(initialContext)

(***************************************************************************
This is the exact indexed multi-height release theorem. It remains proofless
while the ledger says specified_unproved: the non-temporal product/refinement
kernels above are explicit, and its eventual proof must use
IndexedInstanceActivationObligation,
IndexedHistoricalCatchUpProgressObligation, and IndexedOneHeightCompletion at
each successive canonical context.
***************************************************************************)
THEOREM HeightLivenessObligation ==
  IndexedChainSpec => IndexedHeightLivenessProperty


=============================================================================
