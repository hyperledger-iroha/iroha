---- MODULE SumeragiV2ChainEpochRefinementShard03 ----
EXTENDS SumeragiV2ChainEpochRefinementShard02

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
journal components, the proof-only fixed-corridor receipt set, and the
per-node post-Serve producer-episode debt function.  The
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

IndexedServeProducerEpisodeDue(initialContext) ==
  indexedAsyncState[initialContext][7]

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
         IndexedFixedCorridorDeadlines(initialContext),
       asyncServeProducerEpisodeDue <-
         IndexedServeProducerEpisodeDue(initialContext)

(***************************************************************************
The context-indexed proof provider uses the identical production state tuple.
It is intentionally consumed only for the three non-temporal Init/action
safety facts below.  Full temporal closure remains owned by the fixed
`VerificationAsyncProof` instance, so no parameterized temporal proof is
introduced into the indexed chain specification.
***************************************************************************)
IndexedAsyncSafetyProof(initialContext) ==
  INSTANCE SumeragiV2AsyncFairServiceProofs
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
         IndexedFixedCorridorDeadlines(initialContext),
       asyncServeProducerEpisodeDue <-
         IndexedServeProducerEpisodeDue(initialContext)

THEOREM IndexedAsyncInitEstablishesStrongTypeInvariant ==
  \A initialContext:
    IndexedAsync(initialContext)!AsyncInitAt(initialContext)
      => IndexedAsync(initialContext)!AsyncStrongTypeInvariant
PROOF
  <1>1. ASSUME NEW initialContext,
              IndexedAsync(initialContext)!AsyncInitAt(initialContext)
         PROVE IndexedAsync(initialContext)!AsyncStrongTypeInvariant
    <2> QED BY <1>1,
         IndexedAsyncSafetyProof(initialContext)!
           AsyncInitEstablishesStrongTypeInvariant
         DEF IndexedAsync!AsyncInitAt,
             IndexedAsync!AsyncStrongTypeInvariant,
             IndexedAsyncSafetyProof!AsyncInitAt,
             IndexedAsyncSafetyProof!AsyncStrongTypeInvariant
  <1> QED BY <1>1

THEOREM IndexedAsyncBracketNextPreservesStrongTypeInvariant ==
  \A initialContext:
    IndexedAsync(initialContext)!AsyncStrongTypeInvariant
      /\ [IndexedAsync(initialContext)!AsyncNext]_(
           IndexedAsync(initialContext)!AsyncAllVars)
      => (IndexedAsync(initialContext)!AsyncStrongTypeInvariant)'
PROOF
  <1>1. ASSUME NEW initialContext,
              IndexedAsync(initialContext)!AsyncStrongTypeInvariant,
              [IndexedAsync(initialContext)!AsyncNext]_(
                IndexedAsync(initialContext)!AsyncAllVars)
         PROVE (IndexedAsync(initialContext)!AsyncStrongTypeInvariant)'
    <2> QED BY <1>1,
         IndexedAsyncSafetyProof(initialContext)!
           AsyncBracketNextPreservesStrongTypeInvariant
         DEF IndexedAsync!AsyncStrongTypeInvariant,
             IndexedAsync!AsyncNext, IndexedAsync!AsyncAllVars,
             IndexedAsyncSafetyProof!AsyncStrongTypeInvariant,
             IndexedAsyncSafetyProof!AsyncNext,
             IndexedAsyncSafetyProof!AsyncAllVars
  <1> QED BY <1>1

THEOREM IndexedGstAsyncStepIsMonotone ==
  \A initialContext:
    IndexedCore(initialContext, 7)
      /\ [IndexedAsync(initialContext)!AsyncNext]_(
           IndexedAsync(initialContext)!AsyncAllVars)
      => (IndexedCore(initialContext, 7))'
PROOF
  <1>1. ASSUME NEW initialContext,
              IndexedCore(initialContext, 7),
              [IndexedAsync(initialContext)!AsyncNext]_(
                IndexedAsync(initialContext)!AsyncAllVars)
         PROVE (IndexedCore(initialContext, 7))'
    <2> QED BY <1>1,
         IndexedAsyncSafetyProof(initialContext)!GstAsyncStepIsMonotone
         DEF IndexedAsync!AsyncNext, IndexedAsync!AsyncAllVars,
             IndexedAsyncSafetyProof!AsyncNext,
             IndexedAsyncSafetyProof!AsyncAllVars
  <1> QED BY <1>1

(***************************************************************************
The indexed INSTANCE adds its context argument to inherited pure operators,
even though it substitutes only state variables.  These definitional bridges
keep later certificate and availability proofs in the base quorum vocabulary
without asking a backend to normalize an entire StrongInductiveInvariant at
once.  They import no theorem through the parameterized production instance.
***************************************************************************)
THEOREM IndexedQuorumOperatorsMatchBase ==
  \A initialContext, epoch, signers, candidates, validator:
    /\ IndexedAsync(initialContext)!Epochs = Epochs
    /\ IndexedAsync(initialContext)!VotingRoster(epoch)
         = VotingRoster(epoch)
    /\ IndexedAsync(initialContext)!CertificateSignerCount(epoch)
         = CertificateSignerCount(epoch)
    /\ IndexedAsync(initialContext)!RosterIndex(epoch, validator)
         = RosterIndex(epoch, validator)
    /\ IndexedAsync(initialContext)!
         CanonicalCertificateSigners(epoch, candidates)
         = CanonicalCertificateSigners(epoch, candidates)
    /\ (IndexedAsync(initialContext)!DualQuorum(epoch, signers)
          <=> DualQuorum(epoch, signers))
    /\ (IndexedAsync(initialContext)!
          ExactCertificateQuorum(epoch, signers)
          <=> ExactCertificateQuorum(epoch, signers))
BY DEF IndexedAsync!Epochs,
       IndexedAsync!VotingRoster, IndexedAsync!RosterSequence,
       IndexedAsync!CertificateSignerCount,
       IndexedAsync!RosterIndex,
       IndexedAsync!CanonicalCertificateSigners,
       IndexedAsync!ExactCertificateQuorum,
       IndexedAsync!DualQuorum, IndexedAsync!CountQuorum,
       IndexedAsync!PowerQuorum, IndexedAsync!PowerOf,
       IndexedAsync!PowerUnits, IndexedAsync!VotingPower,
       IndexedAsync!Cardinality,
       Epochs, VotingRoster, RosterSequence,
       CertificateSignerCount, RosterIndex,
       CanonicalCertificateSigners, ExactCertificateQuorum,
       DualQuorum, CountQuorum, PowerQuorum, PowerOf, PowerUnits,
       VotingPower, Cardinality

=============================================================================
