---- MODULE SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs ----
EXTENDS SumeragiV2ChainEpochRefinement

(***************************************************************************
Indexed exact historical-recovery archive route.

The one-height transport leaf deliberately does not identify its arbitrary
applied archive with a Commit-certificate fanout recipient.  The indexed
wrapper has the missing identity: `IndexedHistoricalRecoverySourceReady`
chooses one exact current responsive voter whose exact current application is
also durable.  This module projects the one-height transport vocabulary onto
that same indexed Async state and retains the chosen route as a safety
witness.

The module discharges both route availability and the fixed-clock non-packet
service family from the exact per-action product fairness clauses.  It adds no
aggregate fairness, full AsyncSpecAt, or all-responsive-joined premise.  The
separate Candidate/Serve packet corridor remains the explicit temporal
residual.
***************************************************************************)

IndexedHistoricalTransport(initialContext) ==
  INSTANCE SumeragiV2AsyncHistoricalRecoveryTemporalSupportProofs
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
       asyncNextServeAdmissionOrdinal <-
         IndexedScheduler(initialContext, 11),
       asyncNextServeIngressOrdinal <- IndexedScheduler(initialContext, 12),
       asyncServeIngressAdmissions <- IndexedScheduler(initialContext, 13),
       asyncServeAdmissions <- IndexedScheduler(initialContext, 14),
       asyncServeReservations <- IndexedScheduler(initialContext, 15),
       asyncServeTombstones <- IndexedScheduler(initialContext, 16),
       asyncOutstandingWork <- IndexedScheduler(initialContext, 17),
       asyncIoReadyCompletions <- IndexedScheduler(initialContext, 18),
       asyncLocalReadyCompletions <- IndexedScheduler(initialContext, 19),
       asyncNextCompletionSource <- IndexedScheduler(initialContext, 20),
       asyncIoControlAvailable <- IndexedScheduler(initialContext, 21),
       asyncDeferredCompletionQueues <- IndexedScheduler(initialContext, 22),
       asyncDeferredProgressQueues <- IndexedScheduler(initialContext, 23),
       asyncDeferredNormalQueues <- IndexedScheduler(initialContext, 24),
       asyncDeferredHandoffs <- IndexedScheduler(initialContext, 25),
       asyncNextDeferredClass <- IndexedScheduler(initialContext, 26),
       asyncDeferredDrainOwed <- IndexedScheduler(initialContext, 27),
       asyncCausalQueues <- IndexedScheduler(initialContext, 28),
       asyncOutstandingTags <- IndexedScheduler(initialContext, 29),
       asyncNodeDeadlines <- IndexedScheduler(initialContext, 30),
       asyncRetransmitDeadlines <- IndexedScheduler(initialContext, 31),
       asyncNodeServiceDeadlines <- IndexedScheduler(initialContext, 32),
       asyncIoServiceDeadlines <- IndexedScheduler(initialContext, 33),
       asyncSentItems <- IndexedScheduler(initialContext, 34),
       asyncRetainedControl <- IndexedScheduler(initialContext, 35),
       asyncActiveRequests <- IndexedScheduler(initialContext, 36),
       asyncCertifiedResponseClaim <- IndexedScheduler(initialContext, 37),
       asyncTransport <- IndexedScheduler(initialContext, 38),
       asyncIngressLanes <- IndexedScheduler(initialContext, 39),
       asyncIngressReady <- IndexedScheduler(initialContext, 40),
       asyncHeldChunks <- IndexedScheduler(initialContext, 41),
       asyncHistoricalRecoveryTargets <- IndexedScheduler(initialContext, 42),
       asyncControlServiceState <- IndexedScheduler(initialContext, 43),
       asyncServiceActivationState <- IndexedScheduler(initialContext, 44),
       asyncRecoveryPhase <- IndexedRecovery(initialContext, 1),
       asyncRecoveryNode <- IndexedRecovery(initialContext, 2),
       asyncRecoveryGeneration <- IndexedRecovery(initialContext, 3),
       asyncRecoveryReplayQueue <- IndexedRecovery(initialContext, 4),
       asyncHistoricalLockRestartAuthorities <-
         IndexedRecovery(initialContext, 5)

(***************************************************************************
Exact projection.

These are the same 46 Core, 44 scheduler, and five recovery substitutions as
`IndexedAsync` and `IndexedDecisionWitness`.  The extensional equality is what
permits the product bracket to feed the transport instance without defining a
second state machine.
***************************************************************************)

THEOREM IndexedHistoricalTransportVariablesAreExact ==
  IndexedAsyncStateShape
    => \A initialContext \in AdmissibleContextRecords:
         IndexedHistoricalTransport(initialContext)!AsyncAllVars =
           IndexedAsyncStateAt(initialContext)
BY Isa
   DEF IndexedAsyncStateShape, IndexedAsyncStateAt,
       IndexedHistoricalTransport!AsyncAllVars,
       IndexedHistoricalTransport!AsyncSchedulerVars,
       IndexedHistoricalTransport!AsyncRecoveryVars,
       IndexedHistoricalTransport!vars,
       IndexedCore, IndexedScheduler, IndexedRecovery

THEOREM IndexedInitProjectsEveryHistoricalTransportInit ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedChainInit
      => IndexedHistoricalTransport(initialContext)!
           AsyncInitAt(initialContext)
BY IndexedInitProjectsEveryAsyncInit
   DEF IndexedHistoricalTransport!AsyncInitAt,
       IndexedAsync!AsyncInitAt

THEOREM IndexedStepProjectsEveryHistoricalTransportStep ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedChainNext
      => [IndexedHistoricalTransport(initialContext)!AsyncNext]_(
           IndexedHistoricalTransport(initialContext)!AsyncAllVars)
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                IndexedChainNext
         PROVE [IndexedHistoricalTransport(initialContext)!AsyncNext]_(
                 IndexedHistoricalTransport(initialContext)!AsyncAllVars)
    <2>1. IndexedAsyncStateShape
      BY <1>1 DEF IndexedChainNext
    <2>2. IndexedHistoricalTransport(initialContext)!AsyncAllVars =
             IndexedAsyncStateAt(initialContext)
      BY <1>1, <2>1, IndexedHistoricalTransportVariablesAreExact
    <2>3. [IndexedAsync(initialContext)!AsyncNext]_(
             IndexedAsyncStateAt(initialContext))
      BY <1>1, IndexedStepProjectsEveryAsyncStep
    <2> QED BY <2>2, <2>3, Isa
         DEF IndexedHistoricalTransport!AsyncNext,
             IndexedAsync!AsyncNext
  <1> QED BY <1>1

THEOREM IndexedBracketStepProjectsEveryHistoricalTransportStep ==
  \A initialContext \in AdmissibleContextRecords:
    [IndexedChainNext]_IndexedChainVars
      => [IndexedHistoricalTransport(initialContext)!AsyncNext]_(
           IndexedHistoricalTransport(initialContext)!AsyncAllVars)
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                [IndexedChainNext]_IndexedChainVars
         PROVE [IndexedHistoricalTransport(initialContext)!AsyncNext]_(
                 IndexedHistoricalTransport(initialContext)!AsyncAllVars)
    <2>1. CASE IndexedChainNext
      BY <1>1, <2>1,
         IndexedStepProjectsEveryHistoricalTransportStep
    <2>2. CASE UNCHANGED IndexedChainVars
      <3>1. UNCHANGED indexedAsyncState
        BY <2>2 DEF IndexedChainVars
      <3>2. UNCHANGED
               (IndexedHistoricalTransport(initialContext)!AsyncAllVars)
        BY <3>1, Isa
           DEF IndexedHistoricalTransport!AsyncAllVars,
               IndexedHistoricalTransport!AsyncSchedulerVars,
               IndexedHistoricalTransport!AsyncRecoveryVars,
               IndexedHistoricalTransport!vars,
               IndexedCore, IndexedScheduler, IndexedRecovery
      <3> QED BY <3>2
    <2> QED BY <1>1, <2>1, <2>2
  <1> QED BY <1>1

(***************************************************************************
Exact non-packet action vocabulary.

The historical proof instance and the authoritative IndexedAsync instance
substitute the same state tuple.  These equalities let the local fairness
bridges in ChainEpoch feed the fixed-clock proof without constructing a full
AsyncSpecAt behavior.
***************************************************************************)
IndexedHistoricalPostGstTick(initialContext) ==
  /\ IndexedHistoricalTransport(initialContext)!gst
  /\ IndexedHistoricalTransport(initialContext)!AsyncTick

THEOREM IndexedHistoricalNonPacketActionsMatchIndexedAsync ==
  \A initialContext \in AdmissibleContextRecords, node:
    /\ (IndexedHistoricalPostGstTick(initialContext)
          <=> IndexedPostGstTick(initialContext))
    /\ (IndexedHistoricalTransport(initialContext)!
          PostGstRunNode(node)
          <=> IndexedAsync(initialContext)!PostGstRunNode(node))
    /\ (IndexedHistoricalTransport(initialContext)!
          PostGstRunHistoricalServer(node)
          <=> IndexedAsync(initialContext)!
                PostGstRunHistoricalServer(node))
    /\ (IndexedHistoricalTransport(initialContext)!
          PostGstServiceIoWorker(node)
          <=> IndexedAsync(initialContext)!PostGstServiceIoWorker(node))
    /\ (IndexedHistoricalTransport(initialContext)!
          PostGstRunHistoricalRecoveryNode(node)
          <=> IndexedAsync(initialContext)!
                PostGstRunHistoricalRecoveryNode(node))
    /\ (IndexedHistoricalTransport(initialContext)!
          PostGstServiceHistoricalRecoveryIoWorker(node)
          <=> IndexedAsync(initialContext)!
                PostGstServiceHistoricalRecoveryIoWorker(node))
BY Isa
   DEF IndexedHistoricalPostGstTick, IndexedPostGstTick,
       IndexedHistoricalTransport!PostGstRunNode,
       IndexedAsync!PostGstRunNode,
       IndexedHistoricalTransport!PostGstRunHistoricalServer,
       IndexedAsync!PostGstRunHistoricalServer,
       IndexedHistoricalTransport!PostGstServiceIoWorker,
       IndexedAsync!PostGstServiceIoWorker,
       IndexedHistoricalTransport!PostGstRunHistoricalRecoveryNode,
       IndexedAsync!PostGstRunHistoricalRecoveryNode,
       IndexedHistoricalTransport!
         PostGstServiceHistoricalRecoveryIoWorker,
       IndexedAsync!PostGstServiceHistoricalRecoveryIoWorker,
       IndexedHistoricalTransport!AsyncTick,
       IndexedAsync!AsyncTick,
       IndexedHistoricalTransport!RunNode,
       IndexedAsync!RunNode,
       IndexedHistoricalTransport!RunHistoricalServer,
       IndexedAsync!RunHistoricalServer,
       IndexedHistoricalTransport!ServiceIoWorker,
       IndexedAsync!ServiceIoWorker,
       IndexedHistoricalTransport!RunHistoricalRecoveryNode,
       IndexedAsync!RunHistoricalRecoveryNode,
       IndexedHistoricalTransport!ServiceHistoricalRecoveryIoWorker,
       IndexedAsync!ServiceHistoricalRecoveryIoWorker

THEOREM IndexedChainSpecAlwaysHasExactHistoricalTransportState ==
  IndexedChainSpec
    => [](\A initialContext \in AdmissibleContextRecords:
          IndexedHistoricalTransport(initialContext)!AsyncAllVars
            = IndexedAsyncStateAt(initialContext))
BY IndexedChainSpecEstablishesCompositionInvariant,
   IndexedHistoricalTransportVariablesAreExact, PTL
   DEF IndexedCompositionInvariant

THEOREM IndexedChainSpecProvidesHistoricalPostGstTickFairness ==
  IndexedChainSpec
    => \A initialContext \in AdmissibleContextRecords:
         WF_(IndexedHistoricalTransport(initialContext)!AsyncAllVars)(
           IndexedHistoricalPostGstTick(initialContext))
BY IndexedPostGstTickFairnessTransfersLocally,
   IndexedChainSpecAlwaysHasExactHistoricalTransportState,
   IndexedHistoricalNonPacketActionsMatchIndexedAsync, PTL

THEOREM IndexedChainSpecProvidesHistoricalRunNodeFairness ==
  IndexedChainSpec
    => \A initialContext \in AdmissibleContextRecords:
         \A node \in IndexedHistoricalTransport(initialContext)!
                      AsyncVotersAt(initialContext):
           WF_(IndexedHistoricalTransport(initialContext)!AsyncAllVars)(
             IndexedHistoricalTransport(initialContext)!
               PostGstRunNode(node))
BY IndexedPostGstRunNodeFairnessTransfersLocally,
   IndexedChainSpecAlwaysHasExactHistoricalTransportState,
   IndexedHistoricalNonPacketActionsMatchIndexedAsync, Isa, PTL
   DEF IndexedHistoricalTransport!AsyncVotersAt,
       IndexedAsync!AsyncVotersAt

THEOREM IndexedChainSpecProvidesHistoricalOwnerServiceFairness ==
  IndexedChainSpec
    => \A initialContext \in AdmissibleContextRecords:
         \A node \in Responsive:
           /\ WF_(IndexedHistoricalTransport(initialContext)!AsyncAllVars)(
                IndexedHistoricalTransport(initialContext)!
                  PostGstRunHistoricalServer(node))
           /\ WF_(IndexedHistoricalTransport(initialContext)!AsyncAllVars)(
                IndexedHistoricalTransport(initialContext)!
                  PostGstServiceIoWorker(node))
           /\ WF_(IndexedHistoricalTransport(initialContext)!AsyncAllVars)(
                IndexedHistoricalTransport(initialContext)!
                  PostGstRunHistoricalRecoveryNode(node))
           /\ WF_(IndexedHistoricalTransport(initialContext)!AsyncAllVars)(
                IndexedHistoricalTransport(initialContext)!
                  PostGstServiceHistoricalRecoveryIoWorker(node))
BY IndexedHistoricalNonPacketOwnerFairnessTransfersLocally,
   IndexedChainSpecAlwaysHasExactHistoricalTransportState,
   IndexedHistoricalNonPacketActionsMatchIndexedAsync, PTL

(***************************************************************************
Historical scheduler support invariant.

The full exact-instance liveness projection waits for every responsive node
to join a context.  A historical target already carries the narrower joined
owner needed by its dedicated runner and I/O worker, so the product proof
uses only the inductive scheduler and request-completeness invariants consumed
by the historical rank kernels.  This is a safety projection, not an added
fairness premise.
The Candidate tombstone and Serve reservation/tombstone high-watermark are
included explicitly because the fixed-clock producer episode consumes those
finite namespaces before occurrence-rank descent; that episode itself is not
called progress.
***************************************************************************)

IndexedHistoricalTemporalSupportAt(initialContext) ==
  /\ IndexedHistoricalTransport(initialContext)!
       AsyncFrozenContextAt(initialContext)
  /\ IndexedHistoricalTransport(initialContext)!
       AsyncStrongTypeInvariant
  /\ IndexedHistoricalTransport(initialContext)!
       AsyncProgressOwnershipInvariant
  /\ IndexedHistoricalTransport(initialContext)!
       DecisionTimeoutFrontierInvariant
  /\ IndexedHistoricalTransport(initialContext)!
       DecisionFrontierUniquenessInvariant
  /\ IndexedHistoricalTransport(initialContext)!
       PostGstReplayQuarantineExcluded
  /\ IndexedHistoricalTransport(initialContext)!
       ExactDecisionFanoutRetentionInvariant
  /\ IndexedHistoricalTransport(initialContext)!
       Stage2BusyKernelInvariant
  /\ IndexedHistoricalTransport(initialContext)!
       AsyncDeferredHandoffOwnershipInvariant
  /\ IndexedHistoricalTransport(initialContext)!
       HistoricalTemporalIdentityLifecycleInvariant
  /\ IndexedHistoricalTransport(initialContext)!
       HistoricalCommitCertificateRequestCompletenessInvariant

THEOREM IndexedInitEstablishesHistoricalTemporalSupport ==
  IndexedChainInit
    => \A initialContext \in AdmissibleContextRecords:
         IndexedHistoricalTemporalSupportAt(initialContext)
PROOF
  <1>1. ASSUME IndexedChainInit,
                NEW initialContext \in AdmissibleContextRecords
         PROVE IndexedHistoricalTemporalSupportAt(initialContext)
    <2>1. IndexedHistoricalTransport(initialContext)!
             AsyncInitAt(initialContext)
      BY <1>1, IndexedInitProjectsEveryHistoricalTransportInit
    <2>2. IndexedHistoricalTransport(initialContext)!
             AsyncStrongTypeInvariant
      BY <2>1,
         IndexedHistoricalTransport(initialContext)!
           AsyncInitEstablishesStrongTypeInvariant
    <2>3. IndexedHistoricalTransport(initialContext)!
             AsyncProgressOwnershipInvariant
      BY <2>1,
         IndexedHistoricalTransport(initialContext)!
           AsyncInitEstablishesProgressOwnership
    <2>4. IndexedHistoricalTransport(initialContext)!
             DecisionTimeoutFrontierInvariant
      BY <2>1,
         IndexedHistoricalTransport(initialContext)!
           AsyncInitEstablishesDecisionTimeoutFrontier
    <2>5. IndexedHistoricalTransport(initialContext)!
             DecisionFrontierUniquenessInvariant
      BY <2>1,
         IndexedHistoricalTransport(initialContext)!
           AsyncInitEstablishesDecisionFrontierUniqueness
    <2>6. IndexedHistoricalTransport(initialContext)!
             PostGstReplayQuarantineExcluded
      BY <2>1,
         IndexedHistoricalTransport(initialContext)!
           AsyncInitExcludesPostGstReplayQuarantine
    <2>7. IndexedHistoricalTransport(initialContext)!
             ExactDecisionFanoutRetentionInvariant
      BY <2>1,
         IndexedHistoricalTransport(initialContext)!
           AsyncInitEstablishesExactDecisionFanoutRetention
    <2>8. IndexedHistoricalTransport(initialContext)!
             Stage2BusyKernelInvariant
      BY <2>1,
         IndexedHistoricalTransport(initialContext)!
           Stage2BusyKernelInitObligation
    <2>9. IndexedHistoricalTransport(initialContext)!
             AsyncDeferredHandoffOwnershipInvariant
      BY <2>1,
         IndexedHistoricalTransport(initialContext)!
           AsyncInitEstablishesDeferredHandoffOwnership
    <2>10. IndexedHistoricalTransport(initialContext)!
             HistoricalTemporalIdentityLifecycleInvariant
      BY <2>1,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalInitEstablishesIdentityLifecycle
    <2>11. IndexedHistoricalTransport(initialContext)!
             HistoricalCommitCertificateRequestCompletenessInvariant
      BY <2>1,
         IndexedHistoricalTransport(initialContext)!
           AsyncInitEstablishesHistoricalCommitRequestCompleteness
    <2>12. IndexedHistoricalTransport(initialContext)!
              AsyncFrozenContextAt(initialContext)
      BY <2>1,
         IndexedHistoricalTransport(initialContext)!
           AsyncInitEstablishesFrozenContext
    <2> QED BY <2>2, <2>3, <2>4, <2>5, <2>6, <2>7,
                 <2>8, <2>9, <2>10, <2>11, <2>12
         DEF IndexedHistoricalTemporalSupportAt
  <1> QED BY <1>1

THEOREM IndexedBracketStepPreservesHistoricalTemporalSupport ==
  /\ \A initialContext \in AdmissibleContextRecords:
       IndexedHistoricalTemporalSupportAt(initialContext)
  /\ [IndexedChainNext]_IndexedChainVars
  => \A initialContext \in AdmissibleContextRecords:
       IndexedHistoricalTemporalSupportAt(initialContext)'
PROOF
  <1>1. ASSUME
          \A initialContext \in AdmissibleContextRecords:
            IndexedHistoricalTemporalSupportAt(initialContext),
          [IndexedChainNext]_IndexedChainVars
         PROVE
          \A initialContext \in AdmissibleContextRecords:
            IndexedHistoricalTemporalSupportAt(initialContext)'
    <2>1. ASSUME NEW initialContext \in AdmissibleContextRecords
           PROVE IndexedHistoricalTemporalSupportAt(initialContext)'
      <3>1. IndexedHistoricalTemporalSupportAt(initialContext)
        BY <1>1, <2>1
      <3>2. [IndexedHistoricalTransport(initialContext)!AsyncNext]_(
               IndexedHistoricalTransport(initialContext)!AsyncAllVars)
        BY <1>1, <2>1,
           IndexedBracketStepProjectsEveryHistoricalTransportStep
      <3>3. IndexedHistoricalTransport(initialContext)!
               AsyncStrongTypeInvariant'
        BY <3>1, <3>2,
           IndexedHistoricalTransport(initialContext)!
             AsyncBracketNextPreservesStrongTypeInvariant
           DEF IndexedHistoricalTemporalSupportAt
      <3>4. IndexedHistoricalTransport(initialContext)!
               AsyncProgressOwnershipInvariant'
        BY <3>1, <3>2,
           IndexedHistoricalTransport(initialContext)!
             AsyncBracketNextPreservesProgressOwnership
      <3>5. IndexedHistoricalTransport(initialContext)!
               DecisionTimeoutFrontierInvariant'
        BY <3>1, <3>2,
           IndexedHistoricalTransport(initialContext)!
             AsyncBracketPreservesDecisionTimeoutFrontier
           DEF IndexedHistoricalTemporalSupportAt
      <3>6. IndexedHistoricalTransport(initialContext)!
               DecisionFrontierUniquenessInvariant'
        BY <3>1, <3>2,
           IndexedHistoricalTransport(initialContext)!
             AsyncBracketPreservesStrongDecisionFrontier
           DEF IndexedHistoricalTemporalSupportAt,
               IndexedHistoricalTransport!AsyncStrongTypeInvariant
      <3>7. IndexedHistoricalTransport(initialContext)!
               PostGstReplayQuarantineExcluded'
        BY <3>1, <3>2,
           IndexedHistoricalTransport(initialContext)!
             AsyncNextPreservesPostGstReplayQuarantineExclusion
           DEF IndexedHistoricalTemporalSupportAt
      <3>8. IndexedHistoricalTransport(initialContext)!
               ExactDecisionFanoutRetentionInvariant'
        BY <3>1, <3>2,
           IndexedHistoricalTransport(initialContext)!
             AsyncNextPreservesExactDecisionFanoutRetention
           DEF IndexedHistoricalTemporalSupportAt
      <3>9. IndexedHistoricalTransport(initialContext)!
               Stage2BusyKernelInvariant'
        BY <3>1, <3>2,
           IndexedHistoricalTransport(initialContext)!
             Stage2BusyKernelNextObligation
           DEF IndexedHistoricalTemporalSupportAt
      <3>10. IndexedHistoricalTransport(initialContext)!
               AsyncDeferredHandoffOwnershipInvariant'
        BY <3>1, <3>2,
           IndexedHistoricalTransport(initialContext)!
             Stage2AsyncNextPreservesDeferredHandoffOwnership
           DEF IndexedHistoricalTemporalSupportAt
      <3>11. (IndexedHistoricalTransport(initialContext)!
               HistoricalTemporalIdentityLifecycleInvariant)'
        BY <3>1, <3>2,
           IndexedHistoricalTransport(initialContext)!
             HistoricalTemporalBracketPreservesIdentityLifecycle
           DEF IndexedHistoricalTemporalSupportAt
      <3>12. (IndexedHistoricalTransport(initialContext)!
               HistoricalCommitCertificateRequestCompletenessInvariant)'
        BY <3>1, <3>2,
           IndexedHistoricalTransport(initialContext)!
             AsyncBracketPreservesHistoricalCommitRequestCompleteness
           DEF IndexedHistoricalTemporalSupportAt
      <3>13. (IndexedHistoricalTransport(initialContext)!
                AsyncFrozenContextAt(initialContext))'
        BY <3>1, <3>2,
           IndexedHistoricalTransport(initialContext)!
             AsyncNextPreservesFrozenContext
           DEF IndexedHistoricalTemporalSupportAt
      <3> QED BY <3>3, <3>4, <3>5, <3>6, <3>7, <3>8,
                   <3>9, <3>10, <3>11, <3>12, <3>13
           DEF IndexedHistoricalTemporalSupportAt
    <2> QED BY <2>1
  <1> QED BY <1>1

THEOREM IndexedChainSpecAlwaysHistoricalTemporalSupport ==
  IndexedChainSpec
    => [](\A initialContext \in AdmissibleContextRecords:
           IndexedHistoricalTemporalSupportAt(initialContext))
PROOF
  <1>1. IndexedChainInit
           => \A initialContext \in AdmissibleContextRecords:
                IndexedHistoricalTemporalSupportAt(initialContext)
    BY IndexedInitEstablishesHistoricalTemporalSupport
  <1>2. /\ \A initialContext \in AdmissibleContextRecords:
              IndexedHistoricalTemporalSupportAt(initialContext)
         /\ [IndexedChainNext]_IndexedChainVars
         => \A initialContext \in AdmissibleContextRecords:
              IndexedHistoricalTemporalSupportAt(initialContext)'
    BY IndexedBracketStepPreservesHistoricalTemporalSupport
  <1> QED BY <1>1, <1>2, PTL DEF IndexedChainSpec

IndexedHistoricalCommitRequestCompletenessProperty ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedHistoricalTransport(initialContext)!
      HistoricalCommitRequestCompletenessProperty(IndexedChainSpec)

THEOREM IndexedChainSpecProvidesHistoricalCommitRequestCompleteness ==
  IndexedChainSpec
    => IndexedHistoricalCommitRequestCompletenessProperty
PROOF
  <1>1. ASSUME IndexedChainSpec
         PROVE IndexedHistoricalCommitRequestCompletenessProperty
    <2>1. [](\A initialContext \in AdmissibleContextRecords:
               IndexedHistoricalTemporalSupportAt(initialContext))
      BY <1>1, IndexedChainSpecAlwaysHistoricalTemporalSupport
    <2>2. ASSUME NEW initialContext \in AdmissibleContextRecords
           PROVE IndexedHistoricalTransport(initialContext)!
                   HistoricalCommitRequestCompletenessProperty(
                     IndexedChainSpec)
      <3>1. []IndexedHistoricalTemporalSupportAt(initialContext)
        BY <2>1, <2>2, PTL
      <3> QED BY <1>1, <3>1
           DEF IndexedHistoricalTemporalSupportAt,
               IndexedHistoricalTransport!
                 HistoricalCommitRequestCompletenessProperty
    <2> QED BY <2>2
         DEF IndexedHistoricalCommitRequestCompletenessProperty
  <1> QED BY <1>1

IndexedHistoricalDecisionRequestCompletenessProperty ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedHistoricalTransport(initialContext)!
      HistoricalDecisionCertifiedRequestCompletenessProperty(
        IndexedChainSpec)

THEOREM IndexedChainSpecProvidesHistoricalDecisionRequestCompleteness ==
  IndexedChainSpec
    => IndexedHistoricalDecisionRequestCompletenessProperty
PROOF
  <1>1. ASSUME IndexedChainSpec
         PROVE IndexedHistoricalDecisionRequestCompletenessProperty
    <2>1. [](\A initialContext \in AdmissibleContextRecords:
               IndexedHistoricalTemporalSupportAt(initialContext))
      BY <1>1, IndexedChainSpecAlwaysHistoricalTemporalSupport
    <2>2. ASSUME NEW initialContext \in AdmissibleContextRecords
           PROVE IndexedHistoricalTransport(initialContext)!
                   HistoricalDecisionCertifiedRequestCompletenessProperty(
                     IndexedChainSpec)
      <3>1. []IndexedHistoricalTransport(initialContext)!
               ExactDecisionFanoutRetentionInvariant
        BY <2>1, <2>2, PTL
           DEF IndexedHistoricalTemporalSupportAt
      <3>2. []IndexedHistoricalTransport(initialContext)!
               HistoricalDecisionCertifiedRequestCompletenessInvariant
        BY <3>1,
           IndexedHistoricalTransport(initialContext)!
             ExactFanoutRetentionImpliesHistoricalDecisionCompleteness,
           PTL
      <3> QED BY <1>1, <3>2
           DEF IndexedHistoricalTransport!
                 HistoricalDecisionCertifiedRequestCompletenessProperty
    <2> QED BY <2>2
         DEF IndexedHistoricalDecisionRequestCompletenessProperty
  <1> QED BY <1>1

(***************************************************************************
Clock support without full one-height activation.

The fixed-clock reduction needs only strong typing and monotone GST.  Both
hold in every indexed instance before all responsive peers have joined: the
product bracket projects to the exact historical transport step, and GST is
monotone under that bracket.  Keeping this surface separate prevents the
certificate corridor from importing `AsyncSpecAt`, indexed height liveness,
or aggregate current-voter progress.
***************************************************************************)

THEOREM IndexedChainSpecKeepsHistoricalTransportGstOnceSet ==
  IndexedChainSpec
    => \A initialContext \in AdmissibleContextRecords:
         [](IndexedHistoricalTransport(initialContext)!gst
              => []IndexedHistoricalTransport(initialContext)!gst)
PROOF
  <1>1. ASSUME IndexedChainSpec,
                NEW initialContext \in AdmissibleContextRecords
         PROVE [](IndexedHistoricalTransport(initialContext)!gst
                    => []IndexedHistoricalTransport(initialContext)!gst)
    <2>1. [][IndexedChainNext]_IndexedChainVars
      BY <1>1 DEF IndexedChainSpec
    <2>2. IndexedHistoricalTransport(initialContext)!gst
             /\ [IndexedChainNext]_IndexedChainVars
           => (IndexedHistoricalTransport(initialContext)!gst)'
      BY <1>1,
         IndexedBracketStepProjectsEveryHistoricalTransportStep,
         IndexedHistoricalTransport(initialContext)!
           GstAsyncStepIsMonotone
    <2> QED BY <2>1, <2>2, PTL
  <1> QED BY <1>1

IndexedHistoricalClockTemporalSupportProperty ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedHistoricalTransport(initialContext)!
      HistoricalDiscoveryClockTemporalSupportProperty(IndexedChainSpec)

THEOREM IndexedChainSpecProvidesHistoricalClockTemporalSupport ==
  IndexedChainSpec
    => IndexedHistoricalClockTemporalSupportProperty
PROOF
  <1>1. ASSUME IndexedChainSpec
         PROVE IndexedHistoricalClockTemporalSupportProperty
    <2>1. [](\A initialContext \in AdmissibleContextRecords:
               IndexedHistoricalTemporalSupportAt(initialContext))
      BY <1>1, IndexedChainSpecAlwaysHistoricalTemporalSupport
    <2>2. \A initialContext \in AdmissibleContextRecords:
             [](IndexedHistoricalTransport(initialContext)!gst
                  => []IndexedHistoricalTransport(initialContext)!gst)
      BY <1>1, IndexedChainSpecKeepsHistoricalTransportGstOnceSet
    <2> QED BY <1>1, <2>1, <2>2
         DEF IndexedHistoricalClockTemporalSupportProperty,
             IndexedHistoricalTemporalSupportAt,
             IndexedHistoricalTransport!
               HistoricalDiscoveryClockTemporalSupportProperty
  <1> QED BY <1>1

(***************************************************************************
Indexed fixed-clock identity prerequisite.

The product safety projection now carries both logical namespaces consumed by
the fixed-clock producer episode.  This theorem does not close that temporal
episode: it proves only finite reservation/tombstone accounting, immutable
retry identity, monotone terminal coverage, and no resurrection.  The
separate leaf premise below must still prove that every finite non-descent
episode is exhausted before occurrence-rank descent is composed.
***************************************************************************)

IndexedHistoricalFixedClockIdentityBridgeProperty ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedHistoricalTransport(initialContext)!
      HistoricalTemporalCandidateServeIdentityBudgetBridgeProperty(
        IndexedChainSpec)

THEOREM IndexedChainSpecProvidesHistoricalFixedClockIdentityBridge ==
  IndexedChainSpec
    => IndexedHistoricalFixedClockIdentityBridgeProperty
PROOF
  <1>1. ASSUME IndexedChainSpec
         PROVE IndexedHistoricalFixedClockIdentityBridgeProperty
    <2>1. [](\A initialContext \in AdmissibleContextRecords:
               IndexedHistoricalTemporalSupportAt(initialContext))
      BY <1>1, IndexedChainSpecAlwaysHistoricalTemporalSupport
    <2>2. ASSUME NEW initialContext \in AdmissibleContextRecords
           PROVE IndexedHistoricalTransport(initialContext)!
                   HistoricalTemporalCandidateServeIdentityBudgetBridgeProperty(
                     IndexedChainSpec)
      <3>1. []IndexedHistoricalTemporalSupportAt(initialContext)
        BY <2>1, <2>2
      <3> QED
        BY <1>1, <3>1,
           IndexedHistoricalTransport(initialContext)!
             AsyncCandidateTombstoneSubsetIsBoundedByFrozenOwnerCarrier,
           IndexedHistoricalTransport(initialContext)!
             AsyncCandidateServicedIdentityCannotReactivate,
           IndexedHistoricalTransport(initialContext)!
             AsyncCandidateAdmissionIdentityObsolescenceIsMonotoneAtGst,
           IndexedHistoricalTransport(initialContext)!
             AsyncCandidateObsoleteAdmissionIdentityCannotReappearAtGst,
           IndexedHistoricalTransport(initialContext)!
             AsyncCandidateTerminalIdentityCannotReactivateAtGst,
           IndexedHistoricalTransport(initialContext)!
             AsyncCandidateScheduledIdentityDepartureRetiresLifecycleAtGst,
           IndexedHistoricalTransport(initialContext)!
             AsyncCandidateServiceRouteNeutralResponseRetryIsStable,
           IndexedHistoricalTransport(initialContext)!
             AsyncServeQueuedIdentityDepartureInstallsTombstone,
           IndexedHistoricalTransport(initialContext)!
             AsyncServeRetiredIdentityCannotRequeueAtGst,
           IndexedHistoricalTransport(initialContext)!
             HistoricalTemporalServeExactRetryKeepsAdmissionHighWatermark,
           FS_Union, FS_Subset, Isa, PTL
           DEF IndexedHistoricalTemporalSupportAt,
               IndexedHistoricalTransport!
                 HistoricalTemporalCandidateServeIdentityBudgetBridgeProperty,
               IndexedHistoricalTransport!
                 HistoricalTemporalCandidateIdentityBudgetBridgeProperty,
               IndexedHistoricalTransport!
                 HistoricalTemporalServeIdentityBudgetBridgeProperty,
               IndexedHistoricalTransport!
                 HistoricalTemporalCandidateServiceTombstonesInIdentityCarrier,
               IndexedHistoricalTransport!
                 HistoricalTemporalServeReservationsInIdentityCarrier,
               IndexedHistoricalTransport!
                 HistoricalTemporalServeTombstonesInIdentityCarrier,
               IndexedHistoricalTransport!
                 HistoricalTemporalServeRollbackTombstonesInIdentityCarrier,
               IndexedHistoricalTransport!
                 HistoricalTemporalServeRetiredRecordsInIdentityCarrier,
               IndexedHistoricalTransport!
                 HistoricalTemporalServeExactRetryCoalescingAction,
               IndexedHistoricalTransport!
                 HistoricalTemporalIdentityLifecycleInvariant,
               IndexedHistoricalTransport!AsyncStrongTypeInvariant,
               IndexedHistoricalTransport!AsyncSchedulerTypeInvariant,
               IndexedHistoricalTransport!AsyncIoTypeInvariant,
               IndexedHistoricalTransport!AsyncProgressOwnershipInvariant,
               IndexedHistoricalTransport!
                 AsyncCandidateServiceTombstoneLifecycleInvariant,
               IndexedHistoricalTransport!AsyncServeLifecycleTypeInvariant,
               IndexedHistoricalTransport!
                 AsyncServeLifecyclePartitionInvariant,
               IndexedHistoricalTransport!
                 AsyncServeFamilyHighWatermarkInvariant,
               IndexedHistoricalTransport!
                 AsyncServeReservationOwnershipInvariant,
               IndexedHistoricalTransport!AsyncServeOrdinalInvariant,
               IndexedHistoricalTransport!AsyncAllVars
    <2> QED BY <2>2
         DEF IndexedHistoricalFixedClockIdentityBridgeProperty
  <1> QED BY <1>1

(***************************************************************************
Exact indexed fixed-clock packet residual.

The product supplies the action-selection clause directly from the frozen
context and replay-quarantine invariants.  The selected `actionKind` is an
ordinary temporal parameter; no successor state recomputes a `CHOOSE` over
the enabled action set.  Three service families remain below: persistence and
service across packet-action handoff, Candidate service at the frozen exact
identity/rank, and Serve FIFO service at its frozen occurrence rank.  The
Candidate family is itself split into exact runner service and the temporal
lift of `AsyncCommandSuccessorBatchStrictlyConsumesRemainingWork` through
lifecycle-stage tombstone/ordinal persistence.  Equal-count replacement and
count-increasing replenishment enter that radix-four DAG budget; neither is
presented as progress.  Serve freezes `workerKind` and uses the finite
historical-target-to-archive mode descent for a legitimate worker-family
handoff; it never relies on fairness of a state-dependent worker choice.
***************************************************************************)

IndexedHistoricalPacketConcreteProductAction(
    initialContext, packet, actionKind) ==
  LET recipient == packet.item.envelope.recipient
      source == packet.item.source
  IN CASE actionKind = "Admit" ->
            IndexedAdmitPacketStep(
              initialContext, recipient, source)
       [] actionKind = "AdmitHistorical" ->
            IndexedAdmitHistoricalRecoveryPacketStep(
              initialContext, recipient, source)
       [] actionKind = "RunNode" ->
            IndexedRunNodeStep(initialContext, recipient)
       [] actionKind = "RunHistoricalRecovery" ->
            IndexedRunHistoricalRecoveryStep(
              initialContext, recipient)
       [] actionKind = "RunHistoricalServer" ->
            IndexedHistoricalServerStep(initialContext, recipient)
       [] actionKind = "ServiceIo" ->
            IndexedIoWorkerStep(initialContext, recipient)
       [] actionKind = "ServiceHistoricalIo" ->
            IndexedHistoricalRecoveryIoWorkerStep(
              initialContext, recipient)
       [] OTHER -> FALSE

IndexedHistoricalPacketConcreteActionFairDomain(
    initialContext, packet, actionKind) ==
  LET recipient == packet.item.envelope.recipient
      source == packet.item.source
      ingressSources ==
        IndexedHistoricalTransport(initialContext)!AsyncIngressSources
      voters ==
        IndexedHistoricalTransport(initialContext)!
          AsyncVotersAt(initialContext)
  IN CASE actionKind = "Admit" ->
            /\ recipient \in Responsive
            /\ source \in ingressSources
       [] actionKind = "AdmitHistorical" ->
            /\ recipient \in ValidatorIds
            /\ source \in ingressSources
       [] actionKind = "RunNode" -> recipient \in voters
       [] actionKind \in
            {"RunHistoricalRecovery", "RunHistoricalServer",
             "ServiceIo", "ServiceHistoricalIo"} ->
            recipient \in Responsive
       [] OTHER -> FALSE

THEOREM IndexedHistoricalPacketProductActionProjectsFrozenLocalAction ==
  \A initialContext \in AdmissibleContextRecords,
     packet,
     actionKind \in
       IndexedHistoricalTransport(initialContext)!
         HistoricalDiscoveryPacketConcreteActionKindCarrier:
    IndexedHistoricalPacketConcreteProductAction(
      initialContext, packet, actionKind)
      => <<IndexedHistoricalTransport(initialContext)!
              HistoricalDiscoveryPacketConcreteAction(
                packet, actionKind)>>_(
            IndexedHistoricalTransport(initialContext)!AsyncAllVars)
BY IndexedFairProductStepsProjectExactOccurrences,
   IndexedHistoricalTransportVariablesAreExact,
   IndexedHistoricalNonPacketActionsMatchIndexedAsync, Isa
   DEF IndexedHistoricalPacketConcreteProductAction,
       IndexedHistoricalTransport!
         HistoricalDiscoveryPacketConcreteAction,
       IndexedHistoricalTransport!
         HistoricalDiscoveryPacketConcreteActionKindCarrier,
       IndexedAdmitPacketStep,
       IndexedAdmitHistoricalRecoveryPacketStep,
       IndexedRunNodeStep, IndexedRunHistoricalRecoveryStep,
       IndexedHistoricalServerStep, IndexedIoWorkerStep,
       IndexedHistoricalRecoveryIoWorkerStep,
       IndexedHistoricalTransport!PostGstAdmitHiddenPacket,
       IndexedHistoricalTransport!
         PostGstAdmitHistoricalRecoveryPacket,
       IndexedAsync!PostGstAdmitHiddenPacket,
       IndexedAsync!PostGstAdmitHistoricalRecoveryPacket,
       IndexedHistoricalTransport!AsyncAllVars,
       IndexedAsyncStateAt

THEOREM IndexedChainSpecProvidesEachHistoricalPacketProductActionFairness ==
  \A initialContext \in AdmissibleContextRecords,
     packet,
     actionKind \in
       IndexedHistoricalTransport(initialContext)!
         HistoricalDiscoveryPacketConcreteActionKindCarrier:
    /\ IndexedChainSpec
    /\ IndexedHistoricalPacketConcreteActionFairDomain(
         initialContext, packet, actionKind)
    => WF_IndexedChainVars(
         IndexedHistoricalPacketConcreteProductAction(
           initialContext, packet, actionKind))
BY Isa
   DEF IndexedChainSpec, IndexedFairness,
       IndexedHistoricalPacketConcreteProductAction,
       IndexedHistoricalPacketConcreteActionFairDomain,
       IndexedHistoricalTransport!
         HistoricalDiscoveryPacketConcreteActionKindCarrier,
       IndexedHistoricalTransport!AsyncIngressSources,
       IndexedAsync!AsyncIngressSources,
       IndexedHistoricalTransport!AsyncVotersAt,
       IndexedAsync!AsyncVotersAt

THEOREM IndexedChainSpecProvidesHistoricalCandidateExactOwnerFairness ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive,
     clockValue \in Nat,
     sourceRank \in
       IndexedHistoricalTransport(initialContext)!
         HistoricalDiscoveryFixedClockBlockerCarrier,
     packet, known, budget,
     identity, candidate, occurrenceRank, physicalKnown:
    /\ IndexedChainSpec
    /\ IndexedHistoricalTransport(initialContext)!
         HistoricalDiscoveryCandidateExactActionOwnerAtRank(
           node, clockValue, sourceRank, packet, known, budget,
           identity, candidate, occurrenceRank, physicalKnown)
    => WF_(IndexedHistoricalTransport(initialContext)!AsyncAllVars)(
         IndexedHistoricalTransport(initialContext)!
           PostGstRunHistoricalRecoveryNode(
             packet.item.envelope.recipient))
BY IndexedChainSpecProvidesHistoricalOwnerServiceFairness,
   IndexedHistoricalTransport(initialContext)!
     HistoricalDiscoveryLiveCandidateDebtHasExactFairOwner,
   Isa
   DEF IndexedHistoricalTransport!
         HistoricalDiscoveryCandidateExactActionOwnerAtRank,
       IndexedHistoricalTransport!
         HistoricalDiscoveryCandidateServeLifecycleEpisodeAtBudget,
       IndexedHistoricalTransport!HistoricalDiscoveryFixedClockPending

THEOREM IndexedChainSpecProvidesHistoricalServeExactOwnerFairness ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive,
     clockValue \in Nat,
     sourceRank \in
       IndexedHistoricalTransport(initialContext)!
         HistoricalDiscoveryFixedClockBlockerCarrier,
     packet, known, budget,
     identity, job, occurrenceRank,
     workerKind \in
       IndexedHistoricalTransport(initialContext)!
         HistoricalDiscoveryServeExactWorkerActionKindCarrier,
     workerMode \in
       IndexedHistoricalTransport(initialContext)!
         HistoricalDiscoveryServeExactWorkerModeCarrier:
    /\ IndexedChainSpec
    /\ IndexedHistoricalTransport(initialContext)!
         HistoricalDiscoveryServeExactActionOwnerAtRank(
           node, clockValue, sourceRank, packet, known, budget,
           identity, job, occurrenceRank, workerKind, workerMode)
    => WF_(IndexedHistoricalTransport(initialContext)!AsyncAllVars)(
         IndexedHistoricalTransport(initialContext)!
           HistoricalDiscoveryServeExactWorkerAction(
             packet, workerKind))
BY IndexedChainSpecProvidesHistoricalOwnerServiceFairness,
   IndexedHistoricalTransport(initialContext)!
     HistoricalDiscoveryLiveServeDebtHasExactFairOwner,
   Isa
   DEF IndexedHistoricalTransport!
         HistoricalDiscoveryServeExactActionOwnerAtRank,
       IndexedHistoricalTransport!
         HistoricalDiscoveryServeExactWorkerAction,
       IndexedHistoricalTransport!
         HistoricalDiscoveryServeExactWorkerActionKindCarrier,
       IndexedHistoricalTransport!
         HistoricalDiscoveryServeExactWorkerModeCarrier,
       IndexedHistoricalTransport!
         HistoricalDiscoveryServeExactWorkerKindForMode,
       IndexedHistoricalTransport!HistoricalDiscoveryTimedOwnerMode,
       IndexedHistoricalTransport!
         HistoricalDiscoveryCandidateServeLifecycleEpisodeAtBudget,
       IndexedHistoricalTransport!HistoricalDiscoveryFixedClockPending

IndexedHistoricalFixedClockPacketActionSelectionProperties ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedHistoricalTransport(initialContext)!
      HistoricalDiscoveryPacketConcreteActionSelectionProperty(
        IndexedChainSpec)

THEOREM IndexedChainSpecProvidesHistoricalPacketConcreteActionSelection ==
  IndexedChainSpec
    => IndexedHistoricalFixedClockPacketActionSelectionProperties
PROOF
  <1>1. ASSUME IndexedChainSpec,
                NEW initialContext \in AdmissibleContextRecords
         PROVE IndexedHistoricalTransport(initialContext)!
                 HistoricalDiscoveryPacketConcreteActionSelectionProperty(
                   IndexedChainSpec)
    <2>1. []IndexedHistoricalTemporalSupportAt(initialContext)
      BY <1>1, IndexedChainSpecAlwaysHistoricalTemporalSupport, PTL
    <2>2. ASSUME NEW node \in Responsive,
                    NEW clockValue \in Nat,
                    NEW sourceRank \in
                      IndexedHistoricalTransport(initialContext)!
                        HistoricalDiscoveryFixedClockBlockerCarrier,
                    NEW packet, NEW known, NEW budget \in Nat
             PROVE (/\
                       IndexedHistoricalTransport(initialContext)!
                         HistoricalDiscoveryCandidateServeLifecycleEpisodeAtBudget(
                           node, clockValue, sourceRank,
                           packet, known, budget)
                       /\ IndexedHistoricalTransport(initialContext)!
                            HistoricalDiscoveryPacketProducerIdentitySet(
                              packet) = {})
                     ~> (IndexedHistoricalTransport(initialContext)!
                           HistoricalDiscoveryCandidateServeLifecycleGoal(
                             node, clockValue, sourceRank,
                             packet, known, budget)
                          \/ \E dependencyRank \in
                               IndexedHistoricalTransport(initialContext)!
                                 HistoricalDiscoveryPacketDependencyCarrier:
                               \E actionKind \in
                                    IndexedHistoricalTransport(initialContext)!
                                      HistoricalDiscoveryPacketConcreteActionKindCarrier:
                                 IndexedHistoricalTransport(initialContext)!
                                   HistoricalDiscoveryPacketConcreteActionPending(
                                     node, clockValue, sourceRank,
                                     packet, known, budget,
                                     dependencyRank, actionKind))
      <3>1. [](IndexedHistoricalTransport(initialContext)!
                  HistoricalDiscoveryCandidateServeLifecycleEpisodeAtBudget(
                    node, clockValue, sourceRank,
                    packet, known, budget)
                /\ IndexedHistoricalTransport(initialContext)!
                     HistoricalDiscoveryPacketProducerIdentitySet(packet)
                     = {}
               => \E dependencyRank \in
                    IndexedHistoricalTransport(initialContext)!
                      HistoricalDiscoveryPacketDependencyCarrier:
                    \E actionKind \in
                         IndexedHistoricalTransport(initialContext)!
                           HistoricalDiscoveryPacketConcreteActionKindCarrier:
                      IndexedHistoricalTransport(initialContext)!
                        HistoricalDiscoveryPacketConcreteActionPending(
                          node, clockValue, sourceRank,
                          packet, known, budget,
                          dependencyRank, actionKind))
        BY <2>1,
           IndexedHistoricalTransport(initialContext)!
             HistoricalDiscoveryPacketTailHasFrozenConcreteAction,
           PTL
           DEF IndexedHistoricalTemporalSupportAt
      <3> QED BY <3>1, PTL
    <2> QED BY <1>1, <2>2
         DEF IndexedHistoricalTransport!
               HistoricalDiscoveryPacketConcreteActionSelectionProperty
  <1> QED BY <1>1
       DEF IndexedHistoricalFixedClockPacketActionSelectionProperties

IndexedHistoricalCandidateCausalDagTemporalResidual ==
  \A initialContext \in AdmissibleContextRecords:
    /\ IndexedHistoricalTransport(initialContext)!
         HistoricalDiscoveryCandidateExactRunnerStepProperty(
           IndexedChainSpec)
    /\ IndexedHistoricalTransport(initialContext)!
         HistoricalDiscoveryCandidateCausalDagBudgetDescentProperty(
           IndexedChainSpec)

IndexedHistoricalFixedClockPacketCorridorTemporalResidual ==
  /\ \A initialContext \in AdmissibleContextRecords:
       IndexedHistoricalTransport(initialContext)!
         HistoricalDiscoveryPacketConcreteActionServiceProperty(
           IndexedChainSpec)
  /\ IndexedHistoricalCandidateCausalDagTemporalResidual
  /\ \A initialContext \in AdmissibleContextRecords:
       IndexedHistoricalTransport(initialContext)!
         HistoricalDiscoveryServeExactWorkerStepProperty(
           IndexedChainSpec)

THEOREM IndexedChainSpecAndPacketServiceResidualProvidePhysicalKernels ==
  /\ IndexedChainSpec
  /\ IndexedHistoricalFixedClockPacketCorridorTemporalResidual
  => \A initialContext \in AdmissibleContextRecords:
       IndexedHistoricalTransport(initialContext)!
         HistoricalDiscoveryCandidateServeLifecyclePhysicalKernelProperties(
           IndexedChainSpec)
BY IndexedChainSpecProvidesHistoricalPacketConcreteActionSelection, Isa
   DEF IndexedHistoricalFixedClockPacketActionSelectionProperties,
       IndexedHistoricalFixedClockPacketCorridorTemporalResidual,
       IndexedHistoricalCandidateCausalDagTemporalResidual,
       IndexedHistoricalTransport!
         HistoricalDiscoveryCandidateServeLifecyclePhysicalKernelProperties,
       IndexedHistoricalTransport!
         HistoricalDiscoveryCandidateExactRunnerServiceProperty,
       IndexedHistoricalTransport!
         HistoricalDiscoveryServeExactWorkerServiceProperty

IndexedHistoricalFixedClockPacketLeafProperties ==
  \A initialContext \in AdmissibleContextRecords:
    /\ IndexedHistoricalTransport(initialContext)!
         HistoricalDiscoveryFixedClockPacketServiceProperty(
           IndexedChainSpec)
    /\ IndexedHistoricalTransport(initialContext)!
         HistoricalDiscoveryCandidateServeIdentityBudgetProperty(
           IndexedChainSpec)

THEOREM IndexedHistoricalFixedClockPacketResidualClosesPacketLeaves ==
  /\ IndexedChainSpec
  /\ IndexedHistoricalFixedClockPacketCorridorTemporalResidual
    => IndexedHistoricalFixedClockPacketLeafProperties
PROOF
  <1>1. ASSUME IndexedChainSpec,
                IndexedHistoricalFixedClockPacketCorridorTemporalResidual,
                NEW initialContext \in AdmissibleContextRecords
         PROVE
           /\ IndexedHistoricalTransport(initialContext)!
                HistoricalDiscoveryFixedClockPacketServiceProperty(
                  IndexedChainSpec)
           /\ IndexedHistoricalTransport(initialContext)!
                HistoricalDiscoveryCandidateServeIdentityBudgetProperty(
                  IndexedChainSpec)
    <2>1. IndexedHistoricalTransport(initialContext)!
             HistoricalDiscoveryCandidateServeLifecyclePhysicalKernelProperties(
               IndexedChainSpec)
      BY <1>1,
         IndexedChainSpecAndPacketServiceResidualProvidePhysicalKernels
    <2> QED BY <2>1,
         IndexedHistoricalTransport(initialContext)!
           HistoricalDiscoveryPhysicalKernelsDischargeLifecycleService,
         IndexedHistoricalTransport(initialContext)!
           HistoricalDiscoveryLifecycleServiceDischargesPacketService,
         IndexedHistoricalTransport(initialContext)!
           HistoricalDiscoveryCandidateServeOwnerServiceClosesIdentityBudget
  <1> QED BY <1>1
       DEF IndexedHistoricalFixedClockPacketLeafProperties

(***************************************************************************
Activation-local fixed-clock non-packet closure.

The one-height structural lemmas already identify one concrete owner mode,
prove that its exact occurrence descends, and preserve that mode or the rank
goal under every Async step.  The indexed lift below changes only the temporal
source of weak fairness: active post-GST owners use the local ChainEpoch
bridges above, and Tick uses the explicitly GST-guarded action.  No aggregate
AsyncSpecAt projection or all-responsive-joined premise is used.
***************************************************************************)

IndexedHistoricalDueNodeOwnerAtMode(
    initialContext, node, clockValue, sourceRank, owner, mode) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalDiscoveryDueNodeOwnerAtMode(
      node, clockValue, sourceRank, owner, mode)

IndexedHistoricalDueIoOwnerAtMode(
    initialContext, node, clockValue, sourceRank, owner, mode) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalDiscoveryDueIoOwnerAtMode(
      node, clockValue, sourceRank, owner, mode)

IndexedHistoricalDueNodeModeProgressGoal(
    initialContext, node, clockValue, sourceRank, owner, mode) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalDiscoveryDueNodeModeProgressGoal(
      node, clockValue, sourceRank, owner, mode)

IndexedHistoricalDueIoModeProgressGoal(
    initialContext, node, clockValue, sourceRank, owner, mode) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalDiscoveryDueIoModeProgressGoal(
      node, clockValue, sourceRank, owner, mode)

IndexedHistoricalDueNodeModeFairAction(initialContext, owner, mode) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalDiscoveryDueNodeModeFairAction(owner, mode)

IndexedHistoricalDueIoModeFairAction(initialContext, owner, mode) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalDiscoveryDueIoModeFairAction(owner, mode)

IndexedHistoricalTickBlockedAtRank(
    initialContext, node, clockValue, sourceRank) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalDiscoveryTickBlockedAtRank(
      node, clockValue, sourceRank)

THEOREM IndexedHistoricalDueNodeModeHasFairDomain ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive,
     clockValue \in Nat,
     sourceRank \in IndexedHistoricalTransport(initialContext)!
                       HistoricalDiscoveryFixedClockBlockerCarrier,
     owner,
     mode \in IndexedHistoricalTransport(initialContext)!
               HistoricalDiscoveryTimedOwnerModeCarrier:
    /\ IndexedHistoricalTemporalSupportAt(initialContext)
    /\ IndexedHistoricalDueNodeOwnerAtMode(
         initialContext, node, clockValue, sourceRank, owner, mode)
    => /\ owner \in Responsive
       /\ (mode = 2
             => owner \in IndexedHistoricalTransport(initialContext)!
                            AsyncVotersAt(initialContext))
BY IndexedHistoricalTransport(initialContext)!
     HistoricalDiscoveryOwnersIncludeNonVoterService,
   IndexedHistoricalTransport(initialContext)!
     FrozenContextFixesResponsiveVoters,
   IndexedHistoricalTransport(initialContext)!
     AsyncStrongTypeProjectsAsyncType,
   IsaT(300)
   DEF IndexedHistoricalTemporalSupportAt,
       IndexedHistoricalDueNodeOwnerAtMode,
       IndexedHistoricalTransport!HistoricalDiscoveryDueNodeOwnerAtMode,
       IndexedHistoricalTransport!HistoricalDiscoveryTimedOwnerMode,
       IndexedHistoricalTransport!HistoricalDiscoveryNodeBlockersAt,
       IndexedHistoricalTransport!AsyncTimedServiceNodes,
       IndexedHistoricalTransport!AsyncArchiveIoServiceNodes,
       IndexedHistoricalTransport!AsyncResponsiveAppliedArchiveServers,
       IndexedHistoricalTransport!AsyncResponsiveOnlineArchiveServers,
       IndexedHistoricalTransport!AsyncResponsiveArchiveServers,
       IndexedHistoricalTransport!HistoricalRecoveryTarget,
       IndexedHistoricalTransport!AsyncCurrentResponsiveVoters,
       IndexedHistoricalTransport!AsyncVotersAt,
       IndexedHistoricalTransport!AsyncFrozenContextAt,
       IndexedHistoricalTransport!AsyncTypeInvariant,
       IndexedHistoricalTransport!AsyncSchedulerTypeInvariant,
       IndexedHistoricalTransport!AsyncHistoricalRecoveryTypeInvariant

THEOREM IndexedHistoricalDueIoModeHasFairDomain ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive,
     clockValue \in Nat,
     sourceRank \in IndexedHistoricalTransport(initialContext)!
                       HistoricalDiscoveryFixedClockBlockerCarrier,
     owner,
     mode \in IndexedHistoricalTransport(initialContext)!
               HistoricalDiscoveryTimedOwnerModeCarrier:
    /\ IndexedHistoricalTemporalSupportAt(initialContext)
    /\ IndexedHistoricalDueIoOwnerAtMode(
         initialContext, node, clockValue, sourceRank, owner, mode)
    => owner \in Responsive
BY IndexedHistoricalTransport(initialContext)!
     HistoricalDiscoveryOwnersIncludeNonVoterService,
   IndexedHistoricalTransport(initialContext)!
     AsyncStrongTypeProjectsAsyncType,
   IsaT(300)
   DEF IndexedHistoricalTemporalSupportAt,
       IndexedHistoricalDueIoOwnerAtMode,
       IndexedHistoricalTransport!HistoricalDiscoveryDueIoOwnerAtMode,
       IndexedHistoricalTransport!HistoricalDiscoveryTimedOwnerMode,
       IndexedHistoricalTransport!
         HistoricalDiscoveryActiveIoBlockersAt,
       IndexedHistoricalTransport!AsyncTimedServiceNodes,
       IndexedHistoricalTransport!AsyncArchiveIoServiceNodes,
       IndexedHistoricalTransport!AsyncResponsiveAppliedArchiveServers,
       IndexedHistoricalTransport!AsyncResponsiveOnlineArchiveServers,
       IndexedHistoricalTransport!AsyncResponsiveArchiveServers,
       IndexedHistoricalTransport!HistoricalRecoveryTarget,
       IndexedHistoricalTransport!AsyncCurrentResponsiveVoters,
       IndexedHistoricalTransport!AsyncTypeInvariant,
       IndexedHistoricalTransport!AsyncSchedulerTypeInvariant,
       IndexedHistoricalTransport!AsyncHistoricalRecoveryTypeInvariant

THEOREM IndexedChainSpecProvidesHistoricalDueNodeModeFairness ==
  \A initialContext \in AdmissibleContextRecords,
     owner,
     mode \in IndexedHistoricalTransport(initialContext)!
               HistoricalDiscoveryTimedOwnerModeCarrier:
    /\ IndexedChainSpec
    /\ owner \in Responsive
    /\ (mode = 2
          => owner \in IndexedHistoricalTransport(initialContext)!
                         AsyncVotersAt(initialContext))
    => WF_(IndexedHistoricalTransport(initialContext)!AsyncAllVars)(
         IndexedHistoricalDueNodeModeFairAction(
           initialContext, owner, mode))
BY IndexedChainSpecProvidesHistoricalRunNodeFairness,
   IndexedChainSpecProvidesHistoricalOwnerServiceFairness, Isa
   DEF IndexedHistoricalDueNodeModeFairAction,
       IndexedHistoricalTransport!
         HistoricalDiscoveryDueNodeModeFairAction,
       IndexedHistoricalTransport!
         HistoricalDiscoveryTimedOwnerModeCarrier

THEOREM IndexedChainSpecProvidesHistoricalDueIoModeFairness ==
  \A initialContext \in AdmissibleContextRecords,
     owner \in Responsive,
     mode \in IndexedHistoricalTransport(initialContext)!
               HistoricalDiscoveryTimedOwnerModeCarrier:
    IndexedChainSpec
      => WF_(IndexedHistoricalTransport(initialContext)!AsyncAllVars)(
           IndexedHistoricalDueIoModeFairAction(
             initialContext, owner, mode))
BY IndexedChainSpecProvidesHistoricalOwnerServiceFairness, Isa
   DEF IndexedHistoricalDueIoModeFairAction,
       IndexedHistoricalTransport!
         HistoricalDiscoveryDueIoModeFairAction,
       IndexedHistoricalTransport!
         HistoricalDiscoveryTimedOwnerModeCarrier

THEOREM IndexedChainSpecHistoricalDueNodeModeMakesProgress ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive,
     clockValue \in Nat,
     sourceRank \in IndexedHistoricalTransport(initialContext)!
                       HistoricalDiscoveryFixedClockBlockerCarrier,
     owner,
     mode \in IndexedHistoricalTransport(initialContext)!
               HistoricalDiscoveryTimedOwnerModeCarrier:
    IndexedChainSpec
      => (IndexedHistoricalDueNodeOwnerAtMode(
            initialContext, node, clockValue, sourceRank, owner, mode)
           ~> IndexedHistoricalDueNodeModeProgressGoal(
                initialContext, node, clockValue, sourceRank,
                owner, mode))
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW node \in Responsive,
                NEW clockValue \in Nat,
                NEW sourceRank \in
                  IndexedHistoricalTransport(initialContext)!
                    HistoricalDiscoveryFixedClockBlockerCarrier,
                NEW owner,
                NEW mode \in
                  IndexedHistoricalTransport(initialContext)!
                    HistoricalDiscoveryTimedOwnerModeCarrier,
                IndexedChainSpec
         PROVE IndexedHistoricalDueNodeOwnerAtMode(
                 initialContext, node, clockValue, sourceRank,
                 owner, mode)
                 ~>
               IndexedHistoricalDueNodeModeProgressGoal(
                 initialContext, node, clockValue, sourceRank,
                 owner, mode)
    <2>1. []IndexedHistoricalTemporalSupportAt(initialContext)
      BY <1>1, IndexedChainSpecAlwaysHistoricalTemporalSupport, PTL
    <2>2. [][IndexedHistoricalTransport(initialContext)!AsyncNext]_(
             IndexedHistoricalTransport(initialContext)!AsyncAllVars)
      BY <1>1, IndexedBracketStepProjectsEveryHistoricalTransportStep,
         PTL DEF IndexedChainSpec
    <2>3. CASE /\ owner \in Responsive
                 /\ (mode = 2
                       => owner \in
                            IndexedHistoricalTransport(initialContext)!
                              AsyncVotersAt(initialContext))
      <3>1. [](IndexedHistoricalDueNodeOwnerAtMode(
                  initialContext, node, clockValue, sourceRank,
                  owner, mode)
                /\ ~IndexedHistoricalDueNodeModeProgressGoal(
                     initialContext, node, clockValue, sourceRank,
                     owner, mode)
               => ENABLED
                    <<IndexedHistoricalDueNodeModeFairAction(
                        initialContext, owner, mode)>>_(
                      IndexedHistoricalTransport(initialContext)!
                        AsyncAllVars))
        BY <2>1,
           IndexedHistoricalTransport(initialContext)!
             HistoricalDiscoveryDueNodeModeHasEnabledExactFairAction,
           PTL
           DEF IndexedHistoricalDueNodeOwnerAtMode,
               IndexedHistoricalDueNodeModeProgressGoal,
               IndexedHistoricalDueNodeModeFairAction,
               IndexedHistoricalTemporalSupportAt
      <3>2. [](IndexedHistoricalDueNodeOwnerAtMode(
                  initialContext, node, clockValue, sourceRank,
                  owner, mode)
                /\ ~IndexedHistoricalDueNodeModeProgressGoal(
                     initialContext, node, clockValue, sourceRank,
                     owner, mode)
                /\ <<IndexedHistoricalDueNodeModeFairAction(
                       initialContext, owner, mode)>>_(
                     IndexedHistoricalTransport(initialContext)!
                       AsyncAllVars)
               => IndexedHistoricalDueNodeModeProgressGoal(
                    initialContext, node, clockValue, sourceRank,
                    owner, mode)')
        BY <2>2,
           IndexedHistoricalTransport(initialContext)!
             HistoricalDiscoveryDueNodeModeFairOccurrenceReachesRankGoal,
           PTL
           DEF IndexedHistoricalDueNodeOwnerAtMode,
               IndexedHistoricalDueNodeModeProgressGoal,
               IndexedHistoricalDueNodeModeFairAction
      <3>3. [](IndexedHistoricalDueNodeOwnerAtMode(
                  initialContext, node, clockValue, sourceRank,
                  owner, mode)
                /\ ~IndexedHistoricalDueNodeModeProgressGoal(
                     initialContext, node, clockValue, sourceRank,
                     owner, mode)
                /\ [IndexedHistoricalTransport(initialContext)!
                       AsyncNext]_(
                     IndexedHistoricalTransport(initialContext)!
                       AsyncAllVars)
               => \/ IndexedHistoricalDueNodeModeProgressGoal(
                       initialContext, node, clockValue, sourceRank,
                       owner, mode)'
                  \/ IndexedHistoricalDueNodeOwnerAtMode(
                       initialContext, node, clockValue, sourceRank,
                       owner, mode)')
        BY IndexedHistoricalTransport(initialContext)!
             HistoricalDiscoveryDueNodeModeStepPreservesOrProgresses,
           PTL
           DEF IndexedHistoricalDueNodeOwnerAtMode,
               IndexedHistoricalDueNodeModeProgressGoal
      <3>4. WF_(
                IndexedHistoricalTransport(initialContext)!AsyncAllVars)(
                IndexedHistoricalDueNodeModeFairAction(
                  initialContext, owner, mode))
        BY <1>1, <2>3,
           IndexedChainSpecProvidesHistoricalDueNodeModeFairness
      <3> QED BY <2>2, <3>1, <3>2, <3>3, <3>4, PTL
    <2>4. CASE \/ owner \notin Responsive
                 \/ /\ mode = 2
                    /\ owner \notin
                         IndexedHistoricalTransport(initialContext)!
                           AsyncVotersAt(initialContext)
      <3>1. []~IndexedHistoricalDueNodeOwnerAtMode(
                  initialContext, node, clockValue, sourceRank,
                  owner, mode)
        BY <2>1, <2>4,
           IndexedHistoricalDueNodeModeHasFairDomain, PTL
      <3> QED BY <3>1, PTL
    <2> QED BY <2>3, <2>4
  <1> QED BY <1>1

THEOREM IndexedChainSpecHistoricalDueIoModeMakesProgress ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive,
     clockValue \in Nat,
     sourceRank \in IndexedHistoricalTransport(initialContext)!
                       HistoricalDiscoveryFixedClockBlockerCarrier,
     owner,
     mode \in IndexedHistoricalTransport(initialContext)!
               HistoricalDiscoveryTimedOwnerModeCarrier:
    IndexedChainSpec
      => (IndexedHistoricalDueIoOwnerAtMode(
            initialContext, node, clockValue, sourceRank, owner, mode)
           ~> IndexedHistoricalDueIoModeProgressGoal(
                initialContext, node, clockValue, sourceRank,
                owner, mode))
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW node \in Responsive,
                NEW clockValue \in Nat,
                NEW sourceRank \in
                  IndexedHistoricalTransport(initialContext)!
                    HistoricalDiscoveryFixedClockBlockerCarrier,
                NEW owner,
                NEW mode \in
                  IndexedHistoricalTransport(initialContext)!
                    HistoricalDiscoveryTimedOwnerModeCarrier,
                IndexedChainSpec
         PROVE IndexedHistoricalDueIoOwnerAtMode(
                 initialContext, node, clockValue, sourceRank,
                 owner, mode)
                 ~>
               IndexedHistoricalDueIoModeProgressGoal(
                 initialContext, node, clockValue, sourceRank,
                 owner, mode)
    <2>1. []IndexedHistoricalTemporalSupportAt(initialContext)
      BY <1>1, IndexedChainSpecAlwaysHistoricalTemporalSupport, PTL
    <2>2. [][IndexedHistoricalTransport(initialContext)!AsyncNext]_(
             IndexedHistoricalTransport(initialContext)!AsyncAllVars)
      BY <1>1, IndexedBracketStepProjectsEveryHistoricalTransportStep,
         PTL DEF IndexedChainSpec
    <2>3. CASE owner \in Responsive
      <3>1. [](IndexedHistoricalDueIoOwnerAtMode(
                  initialContext, node, clockValue, sourceRank,
                  owner, mode)
                /\ ~IndexedHistoricalDueIoModeProgressGoal(
                     initialContext, node, clockValue, sourceRank,
                     owner, mode)
               => ENABLED
                    <<IndexedHistoricalDueIoModeFairAction(
                        initialContext, owner, mode)>>_(
                      IndexedHistoricalTransport(initialContext)!
                        AsyncAllVars))
        BY <2>1,
           IndexedHistoricalTransport(initialContext)!
             HistoricalDiscoveryDueIoModeHasEnabledExactFairAction,
           PTL
           DEF IndexedHistoricalDueIoOwnerAtMode,
               IndexedHistoricalDueIoModeProgressGoal,
               IndexedHistoricalDueIoModeFairAction,
               IndexedHistoricalTemporalSupportAt
      <3>2. [](IndexedHistoricalDueIoOwnerAtMode(
                  initialContext, node, clockValue, sourceRank,
                  owner, mode)
                /\ ~IndexedHistoricalDueIoModeProgressGoal(
                     initialContext, node, clockValue, sourceRank,
                     owner, mode)
                /\ <<IndexedHistoricalDueIoModeFairAction(
                       initialContext, owner, mode)>>_(
                     IndexedHistoricalTransport(initialContext)!
                       AsyncAllVars)
               => IndexedHistoricalDueIoModeProgressGoal(
                    initialContext, node, clockValue, sourceRank,
                    owner, mode)')
        BY <2>2,
           IndexedHistoricalTransport(initialContext)!
             HistoricalDiscoveryDueIoModeFairOccurrenceReachesRankGoal,
           PTL
           DEF IndexedHistoricalDueIoOwnerAtMode,
               IndexedHistoricalDueIoModeProgressGoal,
               IndexedHistoricalDueIoModeFairAction
      <3>3. [](IndexedHistoricalDueIoOwnerAtMode(
                  initialContext, node, clockValue, sourceRank,
                  owner, mode)
                /\ ~IndexedHistoricalDueIoModeProgressGoal(
                     initialContext, node, clockValue, sourceRank,
                     owner, mode)
                /\ [IndexedHistoricalTransport(initialContext)!
                       AsyncNext]_(
                     IndexedHistoricalTransport(initialContext)!
                       AsyncAllVars)
               => \/ IndexedHistoricalDueIoModeProgressGoal(
                       initialContext, node, clockValue, sourceRank,
                       owner, mode)'
                  \/ IndexedHistoricalDueIoOwnerAtMode(
                       initialContext, node, clockValue, sourceRank,
                       owner, mode)')
        BY IndexedHistoricalTransport(initialContext)!
             HistoricalDiscoveryDueIoModeStepPreservesOrProgresses,
           PTL
           DEF IndexedHistoricalDueIoOwnerAtMode,
               IndexedHistoricalDueIoModeProgressGoal
      <3>4. WF_(
                IndexedHistoricalTransport(initialContext)!AsyncAllVars)(
                IndexedHistoricalDueIoModeFairAction(
                  initialContext, owner, mode))
        BY <1>1, <2>3,
           IndexedChainSpecProvidesHistoricalDueIoModeFairness
      <3> QED BY <2>2, <3>1, <3>2, <3>3, <3>4, PTL
    <2>4. CASE owner \notin Responsive
      <3>1. []~IndexedHistoricalDueIoOwnerAtMode(
                  initialContext, node, clockValue, sourceRank,
                  owner, mode)
        BY <2>1, <2>4,
           IndexedHistoricalDueIoModeHasFairDomain, PTL
      <3> QED BY <3>1, PTL
    <2> QED BY <2>3, <2>4
  <1> QED BY <1>1

THEOREM IndexedChainSpecHistoricalDueNodeOwnerReachesRankGoal ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive,
     clockValue \in Nat,
     sourceRank \in IndexedHistoricalTransport(initialContext)!
                       HistoricalDiscoveryFixedClockBlockerCarrier,
     owner:
    IndexedChainSpec
      => ((/\ IndexedHistoricalTransport(initialContext)!
                   HistoricalDiscoveryFixedClockBlockedAtRank(
                     node, clockValue, sourceRank)
            /\ IndexedHistoricalTransport(initialContext)!
                 OverdueResponsivePackets = {}
            /\ owner \in IndexedHistoricalTransport(initialContext)!
                           HistoricalDiscoveryNodeBlockersAt(clockValue))
           ~> IndexedHistoricalTransport(initialContext)!
                 HistoricalDiscoveryFixedClockStrictRankGoal(
                   node, clockValue, sourceRank))
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW node \in Responsive,
                NEW clockValue \in Nat,
                NEW sourceRank \in
                  IndexedHistoricalTransport(initialContext)!
                    HistoricalDiscoveryFixedClockBlockerCarrier,
                NEW owner,
                IndexedChainSpec
         PROVE (/\ IndexedHistoricalTransport(initialContext)!
                      HistoricalDiscoveryFixedClockBlockedAtRank(
                        node, clockValue, sourceRank)
                 /\ IndexedHistoricalTransport(initialContext)!
                      OverdueResponsivePackets = {}
                 /\ owner \in
                      IndexedHistoricalTransport(initialContext)!
                        HistoricalDiscoveryNodeBlockersAt(clockValue))
                ~>
               IndexedHistoricalTransport(initialContext)!
                 HistoricalDiscoveryFixedClockStrictRankGoal(
                   node, clockValue, sourceRank)
    <2>1. \A mode \in
                 IndexedHistoricalTransport(initialContext)!
                   HistoricalDiscoveryTimedOwnerModeCarrier:
             IndexedHistoricalDueNodeOwnerAtMode(
               initialContext, node, clockValue, sourceRank,
               owner, mode)
               ~> IndexedHistoricalDueNodeModeProgressGoal(
                    initialContext, node, clockValue, sourceRank,
                    owner, mode)
      BY <1>1, IndexedChainSpecHistoricalDueNodeModeMakesProgress
    <2>2. \A mode \in
                 IndexedHistoricalTransport(initialContext)!
                   HistoricalDiscoveryTimedOwnerModeCarrier:
             IndexedHistoricalDueNodeOwnerAtMode(
               initialContext, node, clockValue, sourceRank,
               owner, mode)
               ~> IndexedHistoricalTransport(initialContext)!
                     HistoricalDiscoveryFixedClockStrictRankGoal(
                       node, clockValue, sourceRank)
      BY <2>1,
         IndexedHistoricalTransport(initialContext)!
           HistoricalDiscoveryTimedOwnerModeOrderingIsWellFounded,
         WellFoundedLeadsTo
         DEF IndexedHistoricalDueNodeModeProgressGoal,
             IndexedHistoricalDueNodeOwnerAtMode
    <2>3. []IndexedHistoricalTransport(initialContext)!
             AsyncStrongTypeInvariant
      BY <1>1, IndexedChainSpecAlwaysHistoricalTemporalSupport, PTL
         DEF IndexedHistoricalTemporalSupportAt
    <2>4. (/\ IndexedHistoricalTransport(initialContext)!
                   HistoricalDiscoveryFixedClockBlockedAtRank(
                     node, clockValue, sourceRank)
             /\ IndexedHistoricalTransport(initialContext)!
                  OverdueResponsivePackets = {}
             /\ owner \in IndexedHistoricalTransport(initialContext)!
                            HistoricalDiscoveryNodeBlockersAt(clockValue))
            ~> \E mode \in
                 IndexedHistoricalTransport(initialContext)!
                   HistoricalDiscoveryTimedOwnerModeCarrier:
                 IndexedHistoricalDueNodeOwnerAtMode(
                   initialContext, node, clockValue, sourceRank,
                   owner, mode)
      BY <2>3,
         IndexedHistoricalTransport(initialContext)!
           HistoricalDiscoveryTimedOwnerHasFiniteMode, PTL
         DEF IndexedHistoricalDueNodeOwnerAtMode,
             IndexedHistoricalTransport!
               HistoricalDiscoveryDueNodeOwnerAtMode,
             IndexedHistoricalTransport!
               HistoricalDiscoveryNodeBlockersAt
    <2> QED BY <2>2, <2>4, PTL
  <1> QED BY <1>1

THEOREM IndexedChainSpecHistoricalDueIoOwnerReachesRankGoal ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive,
     clockValue \in Nat,
     sourceRank \in IndexedHistoricalTransport(initialContext)!
                       HistoricalDiscoveryFixedClockBlockerCarrier,
     owner:
    IndexedChainSpec
      => ((/\ IndexedHistoricalTransport(initialContext)!
                   HistoricalDiscoveryFixedClockBlockedAtRank(
                     node, clockValue, sourceRank)
            /\ IndexedHistoricalTransport(initialContext)!
                 OverdueResponsivePackets = {}
            /\ IndexedHistoricalTransport(initialContext)!
                 HistoricalDiscoveryNodeBlockersAt(clockValue) = {}
            /\ owner \in IndexedHistoricalTransport(initialContext)!
                           HistoricalDiscoveryActiveIoBlockersAt(
                             clockValue))
           ~> IndexedHistoricalTransport(initialContext)!
                 HistoricalDiscoveryFixedClockStrictRankGoal(
                   node, clockValue, sourceRank))
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW node \in Responsive,
                NEW clockValue \in Nat,
                NEW sourceRank \in
                  IndexedHistoricalTransport(initialContext)!
                    HistoricalDiscoveryFixedClockBlockerCarrier,
                NEW owner,
                IndexedChainSpec
         PROVE (/\ IndexedHistoricalTransport(initialContext)!
                      HistoricalDiscoveryFixedClockBlockedAtRank(
                        node, clockValue, sourceRank)
                 /\ IndexedHistoricalTransport(initialContext)!
                      OverdueResponsivePackets = {}
                 /\ IndexedHistoricalTransport(initialContext)!
                      HistoricalDiscoveryNodeBlockersAt(clockValue) = {}
                 /\ owner \in
                      IndexedHistoricalTransport(initialContext)!
                        HistoricalDiscoveryActiveIoBlockersAt(clockValue))
                ~>
               IndexedHistoricalTransport(initialContext)!
                 HistoricalDiscoveryFixedClockStrictRankGoal(
                   node, clockValue, sourceRank)
    <2>1. \A mode \in
                 IndexedHistoricalTransport(initialContext)!
                   HistoricalDiscoveryTimedOwnerModeCarrier:
             IndexedHistoricalDueIoOwnerAtMode(
               initialContext, node, clockValue, sourceRank,
               owner, mode)
               ~> IndexedHistoricalDueIoModeProgressGoal(
                    initialContext, node, clockValue, sourceRank,
                    owner, mode)
      BY <1>1, IndexedChainSpecHistoricalDueIoModeMakesProgress
    <2>2. \A mode \in
                 IndexedHistoricalTransport(initialContext)!
                   HistoricalDiscoveryTimedOwnerModeCarrier:
             IndexedHistoricalDueIoOwnerAtMode(
               initialContext, node, clockValue, sourceRank,
               owner, mode)
               ~> IndexedHistoricalTransport(initialContext)!
                     HistoricalDiscoveryFixedClockStrictRankGoal(
                       node, clockValue, sourceRank)
      BY <2>1,
         IndexedHistoricalTransport(initialContext)!
           HistoricalDiscoveryTimedOwnerModeOrderingIsWellFounded,
         WellFoundedLeadsTo
         DEF IndexedHistoricalDueIoModeProgressGoal,
             IndexedHistoricalDueIoOwnerAtMode
    <2>3. []IndexedHistoricalTransport(initialContext)!
             AsyncStrongTypeInvariant
      BY <1>1, IndexedChainSpecAlwaysHistoricalTemporalSupport, PTL
         DEF IndexedHistoricalTemporalSupportAt
    <2>4. (/\ IndexedHistoricalTransport(initialContext)!
                   HistoricalDiscoveryFixedClockBlockedAtRank(
                     node, clockValue, sourceRank)
             /\ IndexedHistoricalTransport(initialContext)!
                  OverdueResponsivePackets = {}
             /\ IndexedHistoricalTransport(initialContext)!
                  HistoricalDiscoveryNodeBlockersAt(clockValue) = {}
             /\ owner \in IndexedHistoricalTransport(initialContext)!
                            HistoricalDiscoveryActiveIoBlockersAt(
                              clockValue))
            ~> \E mode \in
                 IndexedHistoricalTransport(initialContext)!
                   HistoricalDiscoveryTimedOwnerModeCarrier:
                 IndexedHistoricalDueIoOwnerAtMode(
                   initialContext, node, clockValue, sourceRank,
                   owner, mode)
      BY <2>3,
         IndexedHistoricalTransport(initialContext)!
           HistoricalDiscoveryTimedOwnerHasFiniteMode, PTL
         DEF IndexedHistoricalDueIoOwnerAtMode,
             IndexedHistoricalTransport!
               HistoricalDiscoveryDueIoOwnerAtMode,
             IndexedHistoricalTransport!
               HistoricalDiscoveryActiveIoBlockersAt
    <2> QED BY <2>2, <2>4, PTL
  <1> QED BY <1>1

THEOREM IndexedHistoricalTickBlockedHasEnabledPostGstTick ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive,
     clockValue \in Nat,
     sourceRank \in IndexedHistoricalTransport(initialContext)!
                       HistoricalDiscoveryFixedClockBlockerCarrier:
    IndexedHistoricalTickBlockedAtRank(
      initialContext, node, clockValue, sourceRank)
      => ENABLED
           <<IndexedHistoricalPostGstTick(initialContext)>>_(
             IndexedHistoricalTransport(initialContext)!AsyncAllVars)
BY IndexedHistoricalTransport(initialContext)!
     HistoricalDiscoveryTickBlockedHasEnabledExactTick,
   ExpandENABLED, Isa
   DEF IndexedHistoricalTickBlockedAtRank,
       IndexedHistoricalPostGstTick,
       IndexedHistoricalTransport!HistoricalDiscoveryTickBlockedAtRank,
       IndexedHistoricalTransport!
         HistoricalDiscoveryFixedClockBlockedAtRank,
       IndexedHistoricalTransport!HistoricalDiscoveryFixedClockPending

THEOREM IndexedChainSpecHistoricalTickReachesRankGoal ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive,
     clockValue \in Nat,
     sourceRank \in IndexedHistoricalTransport(initialContext)!
                       HistoricalDiscoveryFixedClockBlockerCarrier:
    IndexedChainSpec
      => (IndexedHistoricalTickBlockedAtRank(
            initialContext, node, clockValue, sourceRank)
           ~> IndexedHistoricalTransport(initialContext)!
                 HistoricalDiscoveryFixedClockStrictRankGoal(
                   node, clockValue, sourceRank))
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW node \in Responsive,
                NEW clockValue \in Nat,
                NEW sourceRank \in
                  IndexedHistoricalTransport(initialContext)!
                    HistoricalDiscoveryFixedClockBlockerCarrier,
                IndexedChainSpec
         PROVE IndexedHistoricalTickBlockedAtRank(
                 initialContext, node, clockValue, sourceRank)
                 ~>
               IndexedHistoricalTransport(initialContext)!
                 HistoricalDiscoveryFixedClockStrictRankGoal(
                   node, clockValue, sourceRank)
    <2>1. [](IndexedHistoricalTickBlockedAtRank(
                initialContext, node, clockValue, sourceRank)
              /\ ~IndexedHistoricalTransport(initialContext)!
                    HistoricalDiscoveryFixedClockStrictRankGoal(
                      node, clockValue, sourceRank)
             => ENABLED
                  <<IndexedHistoricalPostGstTick(initialContext)>>_(
                    IndexedHistoricalTransport(initialContext)!
                      AsyncAllVars))
      BY IndexedHistoricalTickBlockedHasEnabledPostGstTick, PTL
    <2>2. [](IndexedHistoricalTickBlockedAtRank(
                initialContext, node, clockValue, sourceRank)
              /\ ~IndexedHistoricalTransport(initialContext)!
                    HistoricalDiscoveryFixedClockStrictRankGoal(
                      node, clockValue, sourceRank)
              /\ <<IndexedHistoricalPostGstTick(initialContext)>>_(
                    IndexedHistoricalTransport(initialContext)!
                      AsyncAllVars)
             => IndexedHistoricalTransport(initialContext)!
                  HistoricalDiscoveryFixedClockStrictRankGoal(
                    node, clockValue, sourceRank)')
      BY IndexedHistoricalTransport(initialContext)!
           HistoricalDiscoveryExactTickReachesStrictRankGoal, PTL
         DEF IndexedHistoricalTickBlockedAtRank,
             IndexedHistoricalPostGstTick
    <2>3. [](IndexedHistoricalTickBlockedAtRank(
                initialContext, node, clockValue, sourceRank)
              /\ ~IndexedHistoricalTransport(initialContext)!
                    HistoricalDiscoveryFixedClockStrictRankGoal(
                      node, clockValue, sourceRank)
              /\ [IndexedHistoricalTransport(initialContext)!
                     AsyncNext]_(
                   IndexedHistoricalTransport(initialContext)!
                     AsyncAllVars)
             => \/ IndexedHistoricalTransport(initialContext)!
                     HistoricalDiscoveryFixedClockStrictRankGoal(
                       node, clockValue, sourceRank)'
                \/ IndexedHistoricalTickBlockedAtRank(
                     initialContext, node, clockValue, sourceRank)')
      BY IndexedHistoricalTransport(initialContext)!
           HistoricalDiscoveryTickStepPreservesOrProgresses, PTL
         DEF IndexedHistoricalTickBlockedAtRank
    <2>4. WF_(IndexedHistoricalTransport(initialContext)!AsyncAllVars)(
             IndexedHistoricalPostGstTick(initialContext))
      BY <1>1,
         IndexedChainSpecProvidesHistoricalPostGstTickFairness
    <2>5. [][IndexedHistoricalTransport(initialContext)!AsyncNext]_(
             IndexedHistoricalTransport(initialContext)!AsyncAllVars)
      BY <1>1, IndexedBracketStepProjectsEveryHistoricalTransportStep,
         PTL DEF IndexedChainSpec
    <2> QED BY <2>1, <2>2, <2>3, <2>4, <2>5, PTL
  <1> QED BY <1>1

IndexedHistoricalFixedClockNonPacketServiceProperty ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedHistoricalTransport(initialContext)!
      HistoricalDiscoveryFixedClockNonPacketServiceProperty(
        IndexedChainSpec)

THEOREM IndexedChainSpecClosesHistoricalFixedClockNonPacketService ==
  IndexedChainSpec
    => IndexedHistoricalFixedClockNonPacketServiceProperty
PROOF
  <1>1. ASSUME IndexedChainSpec,
                NEW initialContext \in AdmissibleContextRecords,
                NEW node \in Responsive,
                NEW clockValue \in Nat,
                NEW sourceRank \in
                  IndexedHistoricalTransport(initialContext)!
                    HistoricalDiscoveryFixedClockBlockerCarrier
         PROVE (/\ IndexedHistoricalTransport(initialContext)!
                      HistoricalDiscoveryFixedClockBlockedAtRank(
                        node, clockValue, sourceRank)
                 /\ IndexedHistoricalTransport(initialContext)!
                      OverdueResponsivePackets = {})
                ~>
               IndexedHistoricalTransport(initialContext)!
                 HistoricalDiscoveryFixedClockStrictRankGoal(
                   node, clockValue, sourceRank)
    <2>1. \A owner:
             (/\ IndexedHistoricalTransport(initialContext)!
                    HistoricalDiscoveryFixedClockBlockedAtRank(
                      node, clockValue, sourceRank)
               /\ IndexedHistoricalTransport(initialContext)!
                    OverdueResponsivePackets = {}
               /\ owner \in IndexedHistoricalTransport(initialContext)!
                              HistoricalDiscoveryNodeBlockersAt(
                                clockValue))
             ~> IndexedHistoricalTransport(initialContext)!
                   HistoricalDiscoveryFixedClockStrictRankGoal(
                     node, clockValue, sourceRank)
      BY <1>1,
         IndexedChainSpecHistoricalDueNodeOwnerReachesRankGoal
    <2>2. \A owner:
             (/\ IndexedHistoricalTransport(initialContext)!
                    HistoricalDiscoveryFixedClockBlockedAtRank(
                      node, clockValue, sourceRank)
               /\ IndexedHistoricalTransport(initialContext)!
                    OverdueResponsivePackets = {}
               /\ IndexedHistoricalTransport(initialContext)!
                    HistoricalDiscoveryNodeBlockersAt(clockValue) = {}
               /\ owner \in IndexedHistoricalTransport(initialContext)!
                              HistoricalDiscoveryActiveIoBlockersAt(
                                clockValue))
             ~> IndexedHistoricalTransport(initialContext)!
                   HistoricalDiscoveryFixedClockStrictRankGoal(
                     node, clockValue, sourceRank)
      BY <1>1,
         IndexedChainSpecHistoricalDueIoOwnerReachesRankGoal
    <2>3. IndexedHistoricalTickBlockedAtRank(
             initialContext, node, clockValue, sourceRank)
             ~>
           IndexedHistoricalTransport(initialContext)!
             HistoricalDiscoveryFixedClockStrictRankGoal(
               node, clockValue, sourceRank)
      BY <1>1, IndexedChainSpecHistoricalTickReachesRankGoal
    <2>4. (/\ IndexedHistoricalTransport(initialContext)!
                  HistoricalDiscoveryFixedClockBlockedAtRank(
                    node, clockValue, sourceRank)
             /\ IndexedHistoricalTransport(initialContext)!
                  OverdueResponsivePackets = {})
            ~>
          (\/ IndexedHistoricalTransport(initialContext)!
                HistoricalDiscoveryFixedClockStrictRankGoal(
                  node, clockValue, sourceRank)
           \/ \E owner:
                /\ IndexedHistoricalTransport(initialContext)!
                     HistoricalDiscoveryFixedClockBlockedAtRank(
                       node, clockValue, sourceRank)
                /\ IndexedHistoricalTransport(initialContext)!
                     OverdueResponsivePackets = {}
                /\ owner \in IndexedHistoricalTransport(initialContext)!
                               HistoricalDiscoveryNodeBlockersAt(
                                 clockValue)
           \/ \E owner:
                /\ IndexedHistoricalTransport(initialContext)!
                     HistoricalDiscoveryFixedClockBlockedAtRank(
                       node, clockValue, sourceRank)
                /\ IndexedHistoricalTransport(initialContext)!
                     OverdueResponsivePackets = {}
                /\ IndexedHistoricalTransport(initialContext)!
                     HistoricalDiscoveryNodeBlockersAt(clockValue) = {}
                /\ owner \in IndexedHistoricalTransport(initialContext)!
                               HistoricalDiscoveryActiveIoBlockersAt(
                                 clockValue)
           \/ IndexedHistoricalTickBlockedAtRank(
                initialContext, node, clockValue, sourceRank))
      BY Isa, PTL DEF IndexedHistoricalTickBlockedAtRank
    <2> QED BY <2>1, <2>2, <2>3, <2>4, PTL
  <1> QED BY <1>1
       DEF IndexedHistoricalFixedClockNonPacketServiceProperty,
           IndexedHistoricalTransport!
             HistoricalDiscoveryFixedClockNonPacketServiceProperty

\* Compatibility surface for callers which still package all three leaves.
\* The non-packet conjunct is now a theorem of IndexedChainSpec; only the
\* exact packet corridor/identity budget remains an explicit residual.
IndexedHistoricalFixedClockTemporalLeafProperties ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedHistoricalTransport(initialContext)!
      HistoricalTemporalFixedClockLeaves(IndexedChainSpec)

IndexedHistoricalFixedClockPrerequisiteSurface ==
  /\ IndexedHistoricalFixedClockIdentityBridgeProperty
  /\ \A initialContext \in AdmissibleContextRecords:
       IndexedHistoricalTransport(initialContext)!
         HistoricalDiscoveryFixedClockTemporalPrerequisites(
           IndexedChainSpec)

THEOREM IndexedHistoricalFixedClockLeavesEstablishPrerequisiteSurface ==
  /\ IndexedChainSpec
  /\ IndexedHistoricalFixedClockTemporalLeafProperties
  => IndexedHistoricalFixedClockPrerequisiteSurface
PROOF
  <1>1. ASSUME IndexedChainSpec,
                IndexedHistoricalFixedClockTemporalLeafProperties
         PROVE IndexedHistoricalFixedClockPrerequisiteSurface
    <2>1. IndexedHistoricalFixedClockIdentityBridgeProperty
      BY <1>1,
         IndexedChainSpecProvidesHistoricalFixedClockIdentityBridge
    <2>2. ASSUME NEW initialContext \in AdmissibleContextRecords
           PROVE IndexedHistoricalTransport(initialContext)!
                   HistoricalDiscoveryFixedClockTemporalPrerequisites(
                     IndexedChainSpec)
      <3>1. IndexedHistoricalTransport(initialContext)!
               HistoricalTemporalFixedClockLeaves(IndexedChainSpec)
        BY <1>1, <2>2
           DEF IndexedHistoricalFixedClockTemporalLeafProperties
      <3> QED BY <3>1,
           IndexedHistoricalTransport(initialContext)!
             HistoricalTemporalFixedClockLeavesAreExact
    <2> QED BY <2>1, <2>2
         DEF IndexedHistoricalFixedClockPrerequisiteSurface
  <1> QED BY <1>1

THEOREM IndexedHistoricalFixedClockExactResidualsEstablishPrerequisiteSurface ==
  /\ IndexedChainSpec
  /\ IndexedHistoricalFixedClockPacketCorridorTemporalResidual
  => IndexedHistoricalFixedClockPrerequisiteSurface
PROOF
  <1>1. ASSUME IndexedChainSpec,
               IndexedHistoricalFixedClockPacketCorridorTemporalResidual
         PROVE IndexedHistoricalFixedClockPrerequisiteSurface
    <2>1. IndexedHistoricalFixedClockIdentityBridgeProperty
      BY <1>1,
         IndexedChainSpecProvidesHistoricalFixedClockIdentityBridge
    <2>2. IndexedHistoricalFixedClockPacketLeafProperties
      BY <1>1,
         IndexedHistoricalFixedClockPacketResidualClosesPacketLeaves
    <2>3. ASSUME NEW initialContext \in AdmissibleContextRecords
           PROVE IndexedHistoricalTransport(initialContext)!
                   HistoricalDiscoveryFixedClockTemporalPrerequisites(
                     IndexedChainSpec)
      <3>1. IndexedHistoricalTransport(initialContext)!
               HistoricalDiscoveryFixedClockNonPacketServiceProperty(
                 IndexedChainSpec)
        BY <1>1,
           IndexedChainSpecClosesHistoricalFixedClockNonPacketService
           DEF IndexedHistoricalFixedClockNonPacketServiceProperty
      <3>2. IndexedHistoricalTransport(initialContext)!
               HistoricalDiscoveryFixedClockPacketServiceProperty(
                 IndexedChainSpec)
        BY <2>2
           DEF IndexedHistoricalFixedClockPacketLeafProperties
      <3>3. IndexedHistoricalTransport(initialContext)!
               HistoricalDiscoveryCandidateServeIdentityBudgetProperty(
                 IndexedChainSpec)
        BY <2>2
           DEF IndexedHistoricalFixedClockPacketLeafProperties
      <3> QED BY <3>1, <3>2, <3>3
           DEF IndexedHistoricalTransport!
                 HistoricalDiscoveryFixedClockTemporalPrerequisites,
               IndexedHistoricalTransport!
                 HistoricalDiscoveryFixedClockConcreteServiceProperties
    <2> QED BY <2>1, <2>3
         DEF IndexedHistoricalFixedClockPrerequisiteSurface
  <1> QED BY <1>1

IndexedHistoricalDiscoveryClockProgressProperty ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedHistoricalTransport(initialContext)!
      HistoricalCommitCertificateDiscoveryClockProgressProperty(
        IndexedChainSpec)

THEOREM IndexedHistoricalFixedClockPrerequisitesCloseDiscoveryClockProgress ==
  /\ IndexedChainSpec
  /\ IndexedHistoricalFixedClockPrerequisiteSurface
  => IndexedHistoricalDiscoveryClockProgressProperty
PROOF
  <1>1. ASSUME IndexedChainSpec,
                IndexedHistoricalFixedClockPrerequisiteSurface
         PROVE IndexedHistoricalDiscoveryClockProgressProperty
    <2>1. IndexedHistoricalClockTemporalSupportProperty
      BY <1>1, IndexedChainSpecProvidesHistoricalClockTemporalSupport
    <2>2. ASSUME NEW initialContext \in AdmissibleContextRecords
           PROVE IndexedHistoricalTransport(initialContext)!
                   HistoricalCommitCertificateDiscoveryClockProgressProperty(
                     IndexedChainSpec)
      <3>1. IndexedHistoricalTransport(initialContext)!
               HistoricalDiscoveryClockTemporalSupportProperty(
                 IndexedChainSpec)
        BY <2>1 DEF IndexedHistoricalClockTemporalSupportProperty
      <3>2. IndexedHistoricalTransport(initialContext)!
               HistoricalDiscoveryFixedClockTemporalPrerequisites(
                 IndexedChainSpec)
        BY <1>1
           DEF IndexedHistoricalFixedClockPrerequisiteSurface
      <3> QED BY <3>1, <3>2,
           IndexedHistoricalTransport(initialContext)!
             HistoricalDiscoveryTemporalPrerequisitesCloseClockProgressFromSupport
    <2> QED BY <2>2
         DEF IndexedHistoricalDiscoveryClockProgressProperty
  <1> QED BY <1>1

(***************************************************************************
Product-local historical discovery fairness.

The clock theorem above stops at the exact discovery due state.  The direct
historical-discovery action already has its own product weak-fairness clause,
and an exact historical target is already joined to this context.  The
following bridge therefore schedules only that target's concrete product
action.  It neither projects the full one-height `AsyncSpecAt` nor waits for
unrelated responsive peers to join.
***************************************************************************)

IndexedHistoricalDiscoveryPending(initialContext, node) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalCommitCertificateDiscoveryPending(node)

IndexedHistoricalDiscoveryOutcome(initialContext, node) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalCommitCertificateDiscoveryOutcome(node)

THEOREM IndexedChainSpecAlwaysHistoricalRecoveryTargetRemoteServer ==
  IndexedChainSpec
    => \A initialContext \in AdmissibleContextRecords:
         []IndexedHistoricalTransport(initialContext)!
              HistoricalRecoveryTargetRemoteServerInvariant
PROOF
  <1>1. ASSUME IndexedChainSpec,
                NEW initialContext \in AdmissibleContextRecords
         PROVE []IndexedHistoricalTransport(initialContext)!
                    HistoricalRecoveryTargetRemoteServerInvariant
    <2>1. IndexedChainInit
            => IndexedHistoricalTransport(initialContext)!
                 HistoricalRecoveryTargetRemoteServerInvariant
      BY IndexedInitProjectsEveryHistoricalTransportInit,
         IndexedHistoricalTransport(initialContext)!
           AsyncInitEstablishesHistoricalRecoveryTargetRemoteServer
    <2>2. []IndexedHistoricalTemporalSupportAt(initialContext)
      BY <1>1, IndexedChainSpecAlwaysHistoricalTemporalSupport, PTL
    <2>3. /\ IndexedHistoricalTemporalSupportAt(initialContext)
           /\ IndexedHistoricalTransport(initialContext)!
                HistoricalRecoveryTargetRemoteServerInvariant
           /\ [IndexedChainNext]_IndexedChainVars
          => IndexedHistoricalTransport(initialContext)!
               HistoricalRecoveryTargetRemoteServerInvariant'
      BY IndexedBracketStepProjectsEveryHistoricalTransportStep,
         IndexedHistoricalTransport(initialContext)!
           AsyncBracketPreservesHistoricalRecoveryTargetRemoteServer
         DEF IndexedHistoricalTemporalSupportAt
    <2> QED BY <1>1, <2>1, <2>2, <2>3, PTL
         DEF IndexedChainSpec
  <1> QED BY <1>1

THEOREM IndexedHistoricalDiscoveryPendingHasJoinedOwner ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    /\ IndexedCompositionInvariant
    /\ IndexedHistoricalDiscoveryPending(initialContext, node)
    => /\ node \in joinedByContext[initialContext]
       /\ initialContext \in JoinedContexts
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW node \in Responsive,
                IndexedCompositionInvariant,
                IndexedHistoricalDiscoveryPending(initialContext, node)
         PROVE /\ node \in joinedByContext[initialContext]
               /\ initialContext \in JoinedContexts
    <2>1. IndexedHistoricalTransport(initialContext)!
             HistoricalRecoveryTarget(node)
      BY <1>1
         DEF IndexedHistoricalDiscoveryPending,
             IndexedHistoricalTransport!
               HistoricalCommitCertificateDiscoveryPending,
             IndexedHistoricalTransport!
               HistoricalCommitCertificateDiscoveryDue
    <2>2. IndexedAsync(initialContext)!
             HistoricalRecoveryTarget(node)
      BY <2>1, Isa
         DEF IndexedHistoricalTransport!HistoricalRecoveryTarget,
             IndexedAsync!HistoricalRecoveryTarget,
             IndexedScheduler
    <2>3. node \in joinedByContext[initialContext]
      BY <1>1, <2>2
         DEF IndexedCompositionInvariant,
             IndexedHistoricalRecoveryTargetCoherence
    <2>4. initialContext \in JoinedContexts
      BY <2>3 DEF JoinedContexts
    <2> QED BY <2>3, <2>4
  <1> QED BY <1>1

THEOREM IndexedHistoricalDiscoveryPendingEnablesExactProductAction ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    /\ IndexedCompositionInvariant
    /\ IndexedHistoricalDiscoveryPending(initialContext, node)
    => ENABLED
         IndexedHistoricalCommitCertificateDiscoveryStep(
           initialContext, node)
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW node \in Responsive,
                IndexedCompositionInvariant,
                IndexedHistoricalDiscoveryPending(initialContext, node)
         PROVE ENABLED
                 IndexedHistoricalCommitCertificateDiscoveryStep(
                   initialContext, node)
    <2>1. /\ node \in joinedByContext[initialContext]
           /\ initialContext \in JoinedContexts
      BY <1>1, IndexedHistoricalDiscoveryPendingHasJoinedOwner
    <2>2. ENABLED IndexedHistoricalTransport(initialContext)!
             PostGstHistoricalCommitCertificateDiscovery(node)
      BY <1>1,
         IndexedHistoricalTransport(initialContext)!
           HistoricalCommitCertificateDiscoveryPendingEnablesFairPrefix,
         ENABLEDaxioms
         DEF IndexedHistoricalDiscoveryPending
    <2>3. ENABLED IndexedAsync(initialContext)!
             PostGstHistoricalCommitCertificateDiscovery(node)
      BY <2>2, Isa
         DEF IndexedHistoricalTransport!
               PostGstHistoricalCommitCertificateDiscovery,
             IndexedAsync!PostGstHistoricalCommitCertificateDiscovery,
             IndexedHistoricalTransport!
               DirectHistoricalCommitCertificateDiscoveryStep,
             IndexedAsync!DirectHistoricalCommitCertificateDiscoveryStep
    <2>4. ENABLED
             IndexedHistoricalCommitCertificateDiscoveryStep(
               initialContext, node)
      BY <1>1, <2>1, <2>3,
         IndexedFairActionsRemainEnabledInProduct
    <2> QED BY <2>4
  <1> QED BY <1>1

THEOREM IndexedHistoricalDiscoveryProductActionIsNonstuttering ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    /\ IndexedHistoricalDiscoveryPending(initialContext, node)
    /\ IndexedHistoricalCommitCertificateDiscoveryStep(
         initialContext, node)
    => <<IndexedHistoricalCommitCertificateDiscoveryStep(
           initialContext, node)>>_IndexedChainVars
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW node \in Responsive,
                IndexedHistoricalDiscoveryPending(initialContext, node),
                IndexedHistoricalCommitCertificateDiscoveryStep(
                  initialContext, node)
         PROVE <<IndexedHistoricalCommitCertificateDiscoveryStep(
                   initialContext, node)>>_IndexedChainVars
    <2>1. IndexedHistoricalTransport(initialContext)!
             PostGstHistoricalCommitCertificateDiscovery(node)
      BY <1>1, Isa
         DEF IndexedHistoricalCommitCertificateDiscoveryStep,
             IndexedAsync!PostGstHistoricalCommitCertificateDiscovery,
             IndexedHistoricalTransport!
               PostGstHistoricalCommitCertificateDiscovery,
             IndexedAsync!DirectHistoricalCommitCertificateDiscoveryStep,
             IndexedHistoricalTransport!
               DirectHistoricalCommitCertificateDiscoveryStep
    <2>2. IndexedHistoricalTransport(initialContext)!
             ActiveCommitCertificateRequests(node) = {}
      BY <1>1
         DEF IndexedHistoricalDiscoveryPending,
             IndexedHistoricalTransport!
               HistoricalCommitCertificateDiscoveryPending,
             IndexedHistoricalTransport!
               HistoricalCommitCertificateDiscoveryDue,
             IndexedHistoricalTransport!CommitCertificateDiscoveryReady
    <2>3. IndexedHistoricalTransport(initialContext)!
             ActiveCommitCertificateRequests(node)' # {}
      BY <2>1,
         IndexedHistoricalTransport(initialContext)!
           DirectHistoricalCommitCertificateDiscoveryPublishes
         DEF IndexedHistoricalTransport!
               PostGstHistoricalCommitCertificateDiscovery
    <2>4. IndexedChainVars' # IndexedChainVars
      BY <2>2, <2>3, Isa
         DEF IndexedHistoricalTransport!
               ActiveCommitCertificateRequests,
             IndexedHistoricalTransport!AsyncAllVars,
             IndexedHistoricalTransport!AsyncSchedulerVars,
             IndexedHistoricalTransport!vars,
             IndexedChainVars, IndexedScheduler
    <2> QED BY <1>1, <2>4
  <1> QED BY <1>1

THEOREM IndexedHistoricalDiscoveryPendingEnablesFairOccurrence ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    /\ IndexedCompositionInvariant
    /\ IndexedHistoricalDiscoveryPending(initialContext, node)
    => ENABLED
         <<IndexedHistoricalCommitCertificateDiscoveryStep(
             initialContext, node)>>_IndexedChainVars
BY IndexedHistoricalDiscoveryPendingEnablesExactProductAction,
   IndexedHistoricalDiscoveryProductActionIsNonstuttering,
   ENABLEDaxioms

THEOREM IndexedHistoricalDiscoveryFairOccurrencePublishes ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    /\ IndexedCompositionInvariant
    /\ IndexedHistoricalDiscoveryPending(initialContext, node)
    /\ <<IndexedHistoricalCommitCertificateDiscoveryStep(
           initialContext, node)>>_IndexedChainVars
    => IndexedHistoricalDiscoveryOutcome(initialContext, node)'
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW node \in Responsive,
                IndexedCompositionInvariant,
                IndexedHistoricalDiscoveryPending(initialContext, node),
                <<IndexedHistoricalCommitCertificateDiscoveryStep(
                    initialContext, node)>>_IndexedChainVars
         PROVE IndexedHistoricalDiscoveryOutcome(initialContext, node)'
    <2>1. <<IndexedHistoricalTransport(initialContext)!
                 PostGstHistoricalCommitCertificateDiscovery(node)>>_(
               IndexedHistoricalTransport(initialContext)!AsyncAllVars)
      BY <1>1, IndexedFairProductStepsProjectExactOccurrences,
         IndexedHistoricalTransportVariablesAreExact, Isa
         DEF IndexedHistoricalCommitCertificateDiscoveryStep,
             IndexedHistoricalTransport!
               PostGstHistoricalCommitCertificateDiscovery,
             IndexedAsync!PostGstHistoricalCommitCertificateDiscovery,
             IndexedHistoricalTransport!
               DirectHistoricalCommitCertificateDiscoveryStep,
             IndexedAsync!DirectHistoricalCommitCertificateDiscoveryStep,
             IndexedHistoricalTransport!AsyncAllVars,
             IndexedAsyncStateAt
    <2> QED BY <1>1, <2>1,
         IndexedHistoricalTransport(initialContext)!
           HistoricalCommitCertificateDiscoveryFairStepPublishes
         DEF IndexedHistoricalDiscoveryPending,
             IndexedHistoricalDiscoveryOutcome
  <1> QED BY <1>1

THEOREM IndexedHistoricalDiscoveryPendingUnlessOutcome ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    /\ IndexedHistoricalTemporalSupportAt(initialContext)
    /\ IndexedHistoricalTransport(initialContext)!
         HistoricalRecoveryTargetRemoteServerInvariant
    /\ IndexedHistoricalDiscoveryPending(initialContext, node)
    /\ [IndexedChainNext]_IndexedChainVars
    => \/ IndexedHistoricalDiscoveryPending(initialContext, node)'
       \/ IndexedHistoricalDiscoveryOutcome(initialContext, node)'
BY IndexedBracketStepProjectsEveryHistoricalTransportStep,
   IndexedHistoricalTransport!
     HistoricalCommitCertificateDiscoveryPendingUnlessOutcome
   DEF IndexedHistoricalTemporalSupportAt,
       IndexedHistoricalDiscoveryPending,
       IndexedHistoricalDiscoveryOutcome

THEOREM IndexedChainSpecSchedulesHistoricalDiscovery ==
  IndexedChainSpec
    => \A initialContext \in AdmissibleContextRecords,
          node \in Responsive:
         IndexedHistoricalDiscoveryPending(initialContext, node)
           ~> IndexedHistoricalDiscoveryOutcome(initialContext, node)
PROOF
  <1>1. ASSUME IndexedChainSpec,
                NEW initialContext \in AdmissibleContextRecords,
                NEW node \in Responsive
         PROVE IndexedHistoricalDiscoveryPending(initialContext, node)
                 ~>
               IndexedHistoricalDiscoveryOutcome(initialContext, node)
    <2>1. []IndexedCompositionInvariant
      BY <1>1, IndexedChainSpecEstablishesCompositionInvariant
    <2>2. []IndexedHistoricalTemporalSupportAt(initialContext)
      BY <1>1, IndexedChainSpecAlwaysHistoricalTemporalSupport, PTL
    <2>3. []IndexedHistoricalTransport(initialContext)!
             HistoricalRecoveryTargetRemoteServerInvariant
      BY <1>1,
         IndexedChainSpecAlwaysHistoricalRecoveryTargetRemoteServer
    <2>4. [][IndexedChainNext]_IndexedChainVars
      BY <1>1 DEF IndexedChainSpec
    <2>5. IndexedHistoricalDiscoveryPending(initialContext, node)
               /\ [IndexedChainNext]_IndexedChainVars
              => \/ IndexedHistoricalDiscoveryPending(
                      initialContext, node)'
                 \/ IndexedHistoricalDiscoveryOutcome(
                      initialContext, node)'
      BY <2>2, <2>3,
         IndexedHistoricalDiscoveryPendingUnlessOutcome
    <2>6. IndexedCompositionInvariant
               /\ IndexedHistoricalDiscoveryPending(
                    initialContext, node)
              => ENABLED
                   <<IndexedHistoricalCommitCertificateDiscoveryStep(
                       initialContext, node)>>_IndexedChainVars
      BY IndexedHistoricalDiscoveryPendingEnablesFairOccurrence
    <2>7. IndexedCompositionInvariant
               /\ IndexedHistoricalDiscoveryPending(
                    initialContext, node)
               /\ <<IndexedHistoricalCommitCertificateDiscoveryStep(
                      initialContext, node)>>_IndexedChainVars
              => IndexedHistoricalDiscoveryOutcome(
                   initialContext, node)'
      BY IndexedHistoricalDiscoveryFairOccurrencePublishes
    <2>8. WF_IndexedChainVars(
             IndexedHistoricalCommitCertificateDiscoveryStep(
               initialContext, node))
      BY <1>1 DEF IndexedChainSpec, IndexedFairness
    <2> QED BY <2>1, <2>4, <2>5, <2>6, <2>7, <2>8, PTL
  <1> QED BY <1>1

THEOREM IndexedHistoricalDiscoveryClockReachesPendingOrOutcome ==
  /\ IndexedChainSpec
  /\ IndexedHistoricalDiscoveryClockProgressProperty
  => \A initialContext \in AdmissibleContextRecords,
        node \in Responsive:
       (IndexedHistoricalTransport(initialContext)!gst
         /\ IndexedHistoricalTransport(initialContext)!
              HistoricalRecoveryTarget(node))
         ~> (IndexedHistoricalDiscoveryPending(initialContext, node)
              \/ IndexedHistoricalDiscoveryOutcome(
                   initialContext, node))
PROOF
  <1>1. ASSUME IndexedChainSpec,
                IndexedHistoricalDiscoveryClockProgressProperty,
                NEW initialContext \in AdmissibleContextRecords,
                NEW node \in Responsive
         PROVE (IndexedHistoricalTransport(initialContext)!gst
                  /\ IndexedHistoricalTransport(initialContext)!
                       HistoricalRecoveryTarget(node))
                  ~>
               (IndexedHistoricalDiscoveryPending(initialContext, node)
                 \/ IndexedHistoricalDiscoveryOutcome(
                      initialContext, node))
    <2>1. []IndexedHistoricalTemporalSupportAt(initialContext)
      BY <1>1, IndexedChainSpecAlwaysHistoricalTemporalSupport, PTL
    <2>2. []IndexedHistoricalTransport(initialContext)!
             HistoricalRecoveryTargetRemoteServerInvariant
      BY <1>1,
         IndexedChainSpecAlwaysHistoricalRecoveryTargetRemoteServer
    <2>3. (IndexedHistoricalTransport(initialContext)!gst
              /\ IndexedHistoricalTransport(initialContext)!
                   HistoricalRecoveryTarget(node))
             ~>
           (IndexedHistoricalTransport(initialContext)!NodeHasDecision(node)
             \/ /\ IndexedHistoricalTransport(initialContext)!
                    HistoricalRecoveryTarget(node)
                /\ \/ IndexedHistoricalTransport(initialContext)!
                        ActiveCommitCertificateRequests(node) # {}
                   \/ IndexedHistoricalTransport(initialContext)!asyncNow
                        >= IndexedHistoricalTransport(initialContext)!
                             AsyncRoundTimeout)
      BY <1>1
         DEF IndexedHistoricalDiscoveryClockProgressProperty,
             IndexedHistoricalTransport!
               HistoricalCommitCertificateDiscoveryClockProgressProperty
    <2>4. /\ IndexedHistoricalTemporalSupportAt(initialContext)
           /\ IndexedHistoricalTransport(initialContext)!
                HistoricalRecoveryTargetRemoteServerInvariant
           /\ IndexedHistoricalTransport(initialContext)!gst
           /\ IndexedHistoricalTransport(initialContext)!
                HistoricalRecoveryTarget(node)
           /\ IndexedHistoricalTransport(initialContext)!asyncNow
                >= IndexedHistoricalTransport(initialContext)!
                     AsyncRoundTimeout
           /\ ~IndexedHistoricalTransport(initialContext)!
                  NodeHasDecision(node)
           /\ IndexedHistoricalTransport(initialContext)!
                ActiveCommitCertificateRequests(node) = {}
          => IndexedHistoricalDiscoveryPending(initialContext, node)
      BY Isa
         DEF IndexedHistoricalTemporalSupportAt,
             IndexedHistoricalDiscoveryPending,
             IndexedHistoricalTransport!
               HistoricalCommitCertificateDiscoveryPending,
             IndexedHistoricalTransport!
               HistoricalCommitCertificateDiscoveryDue,
             IndexedHistoricalTransport!CommitCertificateDiscoveryReady,
             IndexedHistoricalTransport!
               HistoricalRecoveryTargetRemoteServerInvariant
    <2> QED BY <2>1, <2>2, <2>3, <2>4, PTL
         DEF IndexedHistoricalDiscoveryOutcome
  <1> QED BY <1>1

THEOREM IndexedChainSpecClosesHistoricalDiscoveryCorridor ==
  /\ IndexedChainSpec
  /\ IndexedHistoricalDiscoveryClockProgressProperty
  => \A initialContext \in AdmissibleContextRecords,
        node \in Responsive:
       (IndexedHistoricalTransport(initialContext)!gst
         /\ IndexedHistoricalTransport(initialContext)!
              HistoricalRecoveryTarget(node))
         ~> IndexedHistoricalDiscoveryOutcome(initialContext, node)
BY IndexedHistoricalDiscoveryClockReachesPendingOrOutcome,
   IndexedChainSpecSchedulesHistoricalDiscovery, PTL

IndexedHistoricalDiscoveryOwnedOutcome(initialContext, node) ==
  \/ IndexedHistoricalTransport(initialContext)!NodeHasApplication(node)
  \/ /\ IndexedHistoricalTransport(initialContext)!
          HistoricalRecoveryTarget(node)
     /\ IndexedHistoricalDiscoveryOutcome(initialContext, node)

THEOREM IndexedHistoricalTargetPersistsUntilApplication ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    /\ IndexedHistoricalTemporalSupportAt(initialContext)
    /\ IndexedHistoricalTransport(initialContext)!
         HistoricalRecoveryTarget(node)
    /\ [IndexedChainNext]_IndexedChainVars
    => \/ IndexedHistoricalTransport(initialContext)!
            HistoricalRecoveryTarget(node)'
       \/ IndexedHistoricalTransport(initialContext)!
            NodeHasApplication(node)'
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW node \in Responsive,
                IndexedHistoricalTemporalSupportAt(initialContext),
                IndexedHistoricalTransport(initialContext)!
                  HistoricalRecoveryTarget(node),
                [IndexedChainNext]_IndexedChainVars
         PROVE \/ IndexedHistoricalTransport(initialContext)!
                    HistoricalRecoveryTarget(node)'
               \/ IndexedHistoricalTransport(initialContext)!
                    NodeHasApplication(node)'
    <2>1. [IndexedHistoricalTransport(initialContext)!AsyncNext]_(
             IndexedHistoricalTransport(initialContext)!AsyncAllVars)
      BY <1>1,
         IndexedBracketStepProjectsEveryHistoricalTransportStep
    <2>2. CASE IndexedHistoricalTransport(initialContext)!
                  NodeHasApplication(node)'
      BY <2>2
    <2>3. CASE ~IndexedHistoricalTransport(initialContext)!
                   NodeHasApplication(node)'
      BY <1>1, <2>1, <2>3,
         IndexedHistoricalTransport(initialContext)!
           HistoricalRecoveryTargetPersistsUnlessApplication
         DEF IndexedHistoricalTemporalSupportAt
    <2> QED BY <2>2, <2>3
  <1> QED BY <1>1

THEOREM IndexedChainSpecClosesOwnedHistoricalDiscoveryCorridor ==
  /\ IndexedChainSpec
  /\ IndexedHistoricalDiscoveryClockProgressProperty
  => \A initialContext \in AdmissibleContextRecords,
        node \in Responsive:
       (IndexedHistoricalTransport(initialContext)!gst
         /\ IndexedHistoricalTransport(initialContext)!
              HistoricalRecoveryTarget(node))
         ~> IndexedHistoricalDiscoveryOwnedOutcome(
              initialContext, node)
PROOF
  <1>1. ASSUME IndexedChainSpec,
                IndexedHistoricalDiscoveryClockProgressProperty,
                NEW initialContext \in AdmissibleContextRecords,
                NEW node \in Responsive
         PROVE (IndexedHistoricalTransport(initialContext)!gst
                  /\ IndexedHistoricalTransport(initialContext)!
                       HistoricalRecoveryTarget(node))
                  ~>
               IndexedHistoricalDiscoveryOwnedOutcome(
                 initialContext, node)
    <2>1. []IndexedHistoricalTemporalSupportAt(initialContext)
      BY <1>1, IndexedChainSpecAlwaysHistoricalTemporalSupport, PTL
    <2>2. [][IndexedChainNext]_IndexedChainVars
      BY <1>1 DEF IndexedChainSpec
    <2>3. IndexedHistoricalTransport(initialContext)!
             HistoricalRecoveryTarget(node)
               /\ [IndexedChainNext]_IndexedChainVars
              => \/ IndexedHistoricalTransport(initialContext)!
                      HistoricalRecoveryTarget(node)'
                 \/ IndexedHistoricalTransport(initialContext)!
                      NodeHasApplication(node)'
      BY <2>1, IndexedHistoricalTargetPersistsUntilApplication
    <2>4. (IndexedHistoricalTransport(initialContext)!gst
              /\ IndexedHistoricalTransport(initialContext)!
                   HistoricalRecoveryTarget(node))
             ~>
           IndexedHistoricalDiscoveryOutcome(initialContext, node)
      BY <1>1, IndexedChainSpecClosesHistoricalDiscoveryCorridor
    <2> QED BY <2>2, <2>3, <2>4, PTL
         DEF IndexedHistoricalDiscoveryOwnedOutcome
  <1> QED BY <1>1

(***************************************************************************
Reusable indexed historical-runner bridge.

Every historical protected candidate identifies its exact historical target.
Composition coherence places that target in the context's joined set, so the
one-height historical runner has a product extension.  The bridge consumes
the existing product weak-fairness occurrence; it does not activate the full
`AsyncSpecAt` instance or require unrelated responsive peers to join.
***************************************************************************)

IndexedHistoricalTemporalCandidateRunnerPending(
    initialContext, candidate) ==
  /\ IndexedHistoricalTemporalSupportAt(initialContext)
  /\ IndexedHistoricalTransport(initialContext)!gst
  /\ IndexedHistoricalTransport(initialContext)!
       HistoricalProtectedCandidateOwned(candidate)

THEOREM IndexedHistoricalCandidateRunnerHasJoinedFairOwner ==
  \A initialContext \in AdmissibleContextRecords, candidate:
    /\ IndexedCompositionInvariant
    /\ IndexedHistoricalTemporalCandidateRunnerPending(
         initialContext, candidate)
    => /\ candidate.node \in Responsive
       /\ candidate.node \in joinedByContext[initialContext]
       /\ initialContext \in JoinedContexts
       /\ IndexedHistoricalTransport(initialContext)!
            HistoricalRecoveryTarget(candidate.node)
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW candidate,
                IndexedCompositionInvariant,
                IndexedHistoricalTemporalCandidateRunnerPending(
                  initialContext, candidate)
         PROVE /\ candidate.node \in Responsive
               /\ candidate.node \in joinedByContext[initialContext]
               /\ initialContext \in JoinedContexts
               /\ IndexedHistoricalTransport(initialContext)!
                    HistoricalRecoveryTarget(candidate.node)
    <2>1. /\ candidate.node \in Responsive
           /\ IndexedHistoricalTransport(initialContext)!
                HistoricalRecoveryTarget(candidate.node)
      BY <1>1
         DEF IndexedHistoricalTemporalCandidateRunnerPending,
             IndexedHistoricalTransport!
               HistoricalProtectedCandidateOwned
    <2>2. IndexedAsync(initialContext)!
             HistoricalRecoveryTarget(candidate.node)
      BY <2>1, Isa
         DEF IndexedHistoricalTransport!HistoricalRecoveryTarget,
             IndexedAsync!HistoricalRecoveryTarget,
             IndexedScheduler
    <2>3. candidate.node \in joinedByContext[initialContext]
      BY <1>1, <2>2
         DEF IndexedCompositionInvariant,
             IndexedHistoricalRecoveryTargetCoherence
    <2>4. initialContext \in JoinedContexts
      BY <2>3 DEF JoinedContexts
    <2> QED BY <2>1, <2>3, <2>4
  <1> QED BY <1>1

THEOREM IndexedHistoricalCandidateRunnerEnablesExactAction ==
  \A initialContext \in AdmissibleContextRecords, candidate:
    /\ IndexedCompositionInvariant
    /\ IndexedHistoricalTemporalCandidateRunnerPending(
         initialContext, candidate)
    => ENABLED
         IndexedRunHistoricalRecoveryStep(
           initialContext, candidate.node)
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW candidate,
                IndexedCompositionInvariant,
                IndexedHistoricalTemporalCandidateRunnerPending(
                  initialContext, candidate)
         PROVE ENABLED
                 IndexedRunHistoricalRecoveryStep(
                   initialContext, candidate.node)
    <2>1. /\ candidate.node \in Responsive
           /\ candidate.node \in joinedByContext[initialContext]
           /\ initialContext \in JoinedContexts
      BY <1>1, IndexedHistoricalCandidateRunnerHasJoinedFairOwner
    <2>2. ENABLED
             <<IndexedHistoricalTransport(initialContext)!
                 PostGstRunHistoricalRecoveryNode(
                   candidate.node)>>_(
               IndexedHistoricalTransport(initialContext)!AsyncAllVars)
      BY <1>1,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalProtectedOwnerEnablesFairRunner,
         ENABLEDaxioms
         DEF IndexedHistoricalTemporalCandidateRunnerPending,
             IndexedHistoricalTemporalSupportAt
    <2>3. ENABLED IndexedAsync(initialContext)!
             PostGstRunHistoricalRecoveryNode(candidate.node)
      BY <2>2, Isa
         DEF IndexedHistoricalTransport!
               PostGstRunHistoricalRecoveryNode,
             IndexedAsync!PostGstRunHistoricalRecoveryNode,
             IndexedHistoricalTransport!
               RunHistoricalRecoveryNode,
             IndexedAsync!RunHistoricalRecoveryNode,
             IndexedHistoricalTransport!RunNodeWork,
             IndexedHistoricalTransport!LocalAdmissionStep,
             IndexedHistoricalTransport!IngressDrainStep,
             IndexedHistoricalTransport!SerializedRunnerRuntimeStep,
             IndexedHistoricalTransport!SerializedRuntimeStep,
             IndexedHistoricalTransport!
               SerializedRuntimePrecedesServeIngressStep,
             IndexedHistoricalTransport!
               SerializedLocalPrecedesServeIngressStep,
             IndexedHistoricalTransport!AsyncServeIngressTargetOnlyTurn,
             IndexedHistoricalTransport!SelectedLocalAdmissionAdvance,
             IndexedAsync!RunNodeWork,
             IndexedAsync!LocalAdmissionStep,
             IndexedAsync!IngressDrainStep,
             IndexedAsync!SerializedRunnerRuntimeStep,
             IndexedAsync!SerializedRuntimeStep,
             IndexedAsync!SerializedRuntimePrecedesServeIngressStep,
             IndexedAsync!SerializedLocalPrecedesServeIngressStep,
             IndexedAsync!AsyncServeIngressTargetOnlyTurn,
             IndexedAsync!SelectedLocalAdmissionAdvance
    <2>4. ENABLED
             IndexedRunHistoricalRecoveryStep(
               initialContext, candidate.node)
      BY <1>1, <2>1, <2>3,
         IndexedFairActionsRemainEnabledInProduct
    <2> QED BY <2>4
  <1> QED BY <1>1

THEOREM IndexedHistoricalCandidateRunnerIsNonstuttering ==
  \A initialContext \in AdmissibleContextRecords, candidate:
    /\ IndexedHistoricalTemporalCandidateRunnerPending(
         initialContext, candidate)
    /\ IndexedRunHistoricalRecoveryStep(
         initialContext, candidate.node)
    => <<IndexedRunHistoricalRecoveryStep(
           initialContext, candidate.node)>>_IndexedChainVars
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW candidate,
                IndexedHistoricalTemporalCandidateRunnerPending(
                  initialContext, candidate),
                IndexedRunHistoricalRecoveryStep(
                  initialContext, candidate.node)
         PROVE <<IndexedRunHistoricalRecoveryStep(
                   initialContext, candidate.node)>>_IndexedChainVars
    <2>1. <<IndexedHistoricalTransport(initialContext)!
               PostGstRunHistoricalRecoveryNode(candidate.node)>>_(
             IndexedHistoricalTransport(initialContext)!AsyncAllVars)
      BY <1>1,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalRunNodeIsNonstuttering, Isa
         DEF IndexedHistoricalTemporalCandidateRunnerPending,
             IndexedHistoricalTemporalSupportAt,
             IndexedHistoricalTransport!
               HistoricalProtectedCandidateOwned,
             IndexedHistoricalTransport!HistoricalRecoveryTarget,
             IndexedRunHistoricalRecoveryStep,
             IndexedHistoricalTransport!
               PostGstRunHistoricalRecoveryNode,
             IndexedAsync!PostGstRunHistoricalRecoveryNode
    <2>2. IndexedAsyncStateShape
      BY <1>1
         DEF IndexedRunHistoricalRecoveryStep, IndexedChainNext
    <2>3. IndexedHistoricalTransport(initialContext)!AsyncAllVars =
             IndexedAsyncStateAt(initialContext)
      BY <1>1, <2>2, IndexedHistoricalTransportVariablesAreExact
    <2>4. IndexedChainVars' # IndexedChainVars
      BY <2>1, <2>3, Isa
         DEF IndexedChainVars, IndexedAsyncStateAt
    <2> QED BY <1>1, <2>4
  <1> QED BY <1>1

THEOREM IndexedHistoricalCandidateRunnerEnablesFairOccurrence ==
  \A initialContext \in AdmissibleContextRecords, candidate:
    /\ IndexedCompositionInvariant
    /\ IndexedHistoricalTemporalCandidateRunnerPending(
         initialContext, candidate)
    => ENABLED
         <<IndexedRunHistoricalRecoveryStep(
             initialContext, candidate.node)>>_IndexedChainVars
BY IndexedHistoricalCandidateRunnerEnablesExactAction,
   IndexedHistoricalCandidateRunnerIsNonstuttering,
   ENABLEDaxioms

IndexedHistoricalTemporalRankProgressExit(
    initialContext, candidate, rank) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalTemporalRankProgressExit(candidate, rank)

(***************************************************************************
Indexed historical Stage-3 lift.

The product owns the same exact Runtime-prefix auxiliary rank as the
one-height proof.  The only fair consumer is the joined historical runner
bridged above.
***************************************************************************)

IndexedHistoricalTemporalStage3Pending(
    initialContext, candidate, position) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalTemporalStage3Pending(candidate, position)

IndexedHistoricalTemporalStage3BlockedAtAux(
    initialContext, candidate, position, rank) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalTemporalStage3AuxBlocked(
      candidate, position, rank)

IndexedHistoricalTemporalStage3AuxProgress(
    initialContext, candidate, position, rank) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalTemporalStage3AuxProgress(
      candidate, position, rank)

IndexedHistoricalTemporalStage3ServeEpisodeResidual(
    initialContext, candidate, position, rank) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalTemporalStage3ServeEpisodeResidual(
      candidate, position, rank)

IndexedHistoricalTemporalStage3FiniteServeEpisodeResidualProperty ==
  \A initialContext \in AdmissibleContextRecords,
     candidate, position:
    \A rank \in IndexedHistoricalTransport(initialContext)!
                   ReadyRunAuxCarrier:
      IndexedHistoricalTemporalStage3ServeEpisodeResidual(
        initialContext, candidate, position, rank)
        ~> IndexedHistoricalTemporalStage3AuxProgress(
             initialContext, candidate, position, rank)

THEOREM IndexedHistoricalStage3BlockedHasRunnerPending ==
  \A initialContext \in AdmissibleContextRecords,
     candidate, position, rank:
    IndexedHistoricalTemporalStage3BlockedAtAux(
      initialContext, candidate, position, rank)
      => IndexedHistoricalTemporalCandidateRunnerPending(
           initialContext, candidate)
BY DEF IndexedHistoricalTemporalStage3BlockedAtAux,
       IndexedHistoricalTemporalCandidateRunnerPending,
       IndexedHistoricalTemporalSupportAt,
       IndexedHistoricalTransport!
         HistoricalTemporalStage3AuxBlocked,
       IndexedHistoricalTransport!
         HistoricalTemporalStage3Pending,
       IndexedHistoricalTransport!
         HistoricalProtectedOwnedAtServiceRank

THEOREM IndexedHistoricalStage3RunnerProducesOutcome ==
  \A initialContext \in AdmissibleContextRecords,
     candidate, position:
    \A rank \in IndexedHistoricalTransport(initialContext)!
                   ReadyRunAuxCarrier:
      /\ IndexedHistoricalTemporalStage3BlockedAtAux(
           initialContext, candidate, position, rank)
      /\ <<IndexedRunHistoricalRecoveryStep(
             initialContext, candidate.node)>>_IndexedChainVars
      => \/ IndexedHistoricalTemporalStage3AuxProgress(
              initialContext, candidate, position, rank)'
         \/ IndexedHistoricalTemporalStage3ServeEpisodeResidual(
              initialContext, candidate, position, rank)'
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW candidate, NEW position,
                NEW rank \in
                  IndexedHistoricalTransport(initialContext)!
                    ReadyRunAuxCarrier,
                IndexedHistoricalTemporalStage3BlockedAtAux(
                  initialContext, candidate, position, rank),
                <<IndexedRunHistoricalRecoveryStep(
                    initialContext, candidate.node)>>_IndexedChainVars
         PROVE \/ IndexedHistoricalTemporalStage3AuxProgress(
                    initialContext, candidate, position, rank)'
               \/ IndexedHistoricalTemporalStage3ServeEpisodeResidual(
                    initialContext, candidate, position, rank)'
    <2>1. IndexedHistoricalTransport(initialContext)!
             PostGstRunHistoricalRecoveryNode(candidate.node)
      BY <1>1, Isa
         DEF IndexedRunHistoricalRecoveryStep,
             IndexedAsync!PostGstRunHistoricalRecoveryNode,
             IndexedHistoricalTransport!
               PostGstRunHistoricalRecoveryNode
    <2> QED BY <1>1, <2>1,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage3SameRunnerAuxOutcome
         DEF IndexedHistoricalTemporalStage3BlockedAtAux,
             IndexedHistoricalTemporalStage3AuxProgress,
             IndexedHistoricalTemporalStage3ServeEpisodeResidual
  <1> QED BY <1>1

THEOREM IndexedHistoricalStage3UnlessAuxProgress ==
  \A initialContext \in AdmissibleContextRecords,
     candidate, position:
    \A rank \in IndexedHistoricalTransport(initialContext)!
                   ReadyRunAuxCarrier:
      /\ IndexedHistoricalTemporalStage3BlockedAtAux(
           initialContext, candidate, position, rank)
      /\ [IndexedChainNext]_IndexedChainVars
      => \/ IndexedHistoricalTemporalStage3BlockedAtAux(
              initialContext, candidate, position, rank)'
         \/ IndexedHistoricalTemporalStage3AuxProgress(
              initialContext, candidate, position, rank)'
         \/ IndexedHistoricalTemporalStage3ServeEpisodeResidual(
              initialContext, candidate, position, rank)'
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW candidate, NEW position,
                NEW rank \in
                  IndexedHistoricalTransport(initialContext)!
                    ReadyRunAuxCarrier,
                IndexedHistoricalTemporalStage3BlockedAtAux(
                  initialContext, candidate, position, rank),
                [IndexedChainNext]_IndexedChainVars
         PROVE
           \/ IndexedHistoricalTemporalStage3BlockedAtAux(
                initialContext, candidate, position, rank)'
           \/ IndexedHistoricalTemporalStage3AuxProgress(
                initialContext, candidate, position, rank)'
           \/ IndexedHistoricalTemporalStage3ServeEpisodeResidual(
                initialContext, candidate, position, rank)'
    <2>1. [IndexedHistoricalTransport(initialContext)!AsyncNext]_(
             IndexedHistoricalTransport(initialContext)!AsyncAllVars)
      BY <1>1,
         IndexedBracketStepProjectsEveryHistoricalTransportStep
    <2>2. CASE IndexedHistoricalTransport(initialContext)!
                  PostGstRunHistoricalRecoveryNode(candidate.node)
      BY <1>1, <2>2,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage3SameRunnerAuxOutcome, Isa
         DEF IndexedHistoricalTemporalStage3BlockedAtAux,
             IndexedHistoricalTemporalStage3AuxProgress,
             IndexedHistoricalTemporalStage3ServeEpisodeResidual
    <2>3. CASE ~IndexedHistoricalTransport(initialContext)!
                   PostGstRunHistoricalRecoveryNode(candidate.node)
      BY <1>1, <2>1, <2>3,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage3OtherStepUnlessAuxDescent
         DEF IndexedHistoricalTemporalStage3BlockedAtAux,
             IndexedHistoricalTemporalStage3AuxProgress,
             IndexedHistoricalTemporalStage3ServeEpisodeResidual
    <2> QED BY <2>2, <2>3
  <1> QED BY <1>1

THEOREM IndexedChainSpecClosesHistoricalTemporalStage3Rank ==
  /\ IndexedChainSpec
  /\ IndexedHistoricalTemporalStage3FiniteServeEpisodeResidualProperty
    => \A initialContext \in AdmissibleContextRecords,
          candidate, position \in Nat:
         IndexedHistoricalTemporalStage3Pending(
           initialContext, candidate, position)
           ~> IndexedHistoricalTemporalRankProgressExit(
                initialContext, candidate, <<3, position>>)
PROOF
  <1>1. ASSUME IndexedChainSpec,
                IndexedHistoricalTemporalStage3FiniteServeEpisodeResidualProperty,
                NEW initialContext \in AdmissibleContextRecords,
                NEW candidate, NEW position \in Nat
         PROVE IndexedHistoricalTemporalStage3Pending(
                 initialContext, candidate, position)
                 ~>
               IndexedHistoricalTemporalRankProgressExit(
                 initialContext, candidate, <<3, position>>)
    <2>1. []IndexedCompositionInvariant
      BY <1>1, IndexedChainSpecEstablishesCompositionInvariant
    <2>2. [][IndexedChainNext]_IndexedChainVars
      BY <1>1 DEF IndexedChainSpec
    <2>3. []IndexedHistoricalTemporalSupportAt(initialContext)
      BY <1>1, IndexedChainSpecAlwaysHistoricalTemporalSupport, PTL
    <2>4. IndexedHistoricalTemporalStage3Pending(
             initialContext, candidate, position)
             ~> \E rank \in
                  IndexedHistoricalTransport(initialContext)!
                    ReadyRunAuxCarrier:
                  IndexedHistoricalTemporalStage3BlockedAtAux(
                    initialContext, candidate, position, rank)
      BY <2>3,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage3AuxRankInCarrier, PTL
         DEF IndexedHistoricalTemporalStage3Pending,
             IndexedHistoricalTemporalStage3BlockedAtAux,
             IndexedHistoricalTransport!
               HistoricalTemporalStage3AuxBlocked,
             IndexedHistoricalTransport!
               HistoricalTemporalStage3Pending,
             IndexedHistoricalTransport!
               HistoricalTemporalStage3RankExit,
             IndexedHistoricalTransport!
               HistoricalTemporalRankProgressExit,
             IndexedHistoricalTransport!
               HistoricalProtectedOwnedAtServiceRank,
             IndexedHistoricalTransport!
               HistoricalProtectedServiceOwnershipExit,
             IndexedHistoricalTransport!ServiceRankLess
    <2>5. ASSUME NEW rank \in
                  IndexedHistoricalTransport(initialContext)!
                    ReadyRunAuxCarrier
           PROVE IndexedHistoricalTemporalStage3BlockedAtAux(
                   initialContext, candidate, position, rank)
                   ~> IndexedHistoricalTemporalStage3AuxProgress(
                        initialContext, candidate, position, rank)
      <3>1. IndexedHistoricalTemporalStage3BlockedAtAux(
               initialContext, candidate, position, rank)
               /\ [IndexedChainNext]_IndexedChainVars
              => \/ IndexedHistoricalTemporalStage3BlockedAtAux(
                      initialContext, candidate, position, rank)'
                 \/ IndexedHistoricalTemporalStage3AuxProgress(
                      initialContext, candidate, position, rank)'
                 \/ IndexedHistoricalTemporalStage3ServeEpisodeResidual(
                      initialContext, candidate, position, rank)'
        BY <1>1, IndexedHistoricalStage3UnlessAuxProgress
      <3>2. IndexedCompositionInvariant
               /\ IndexedHistoricalTemporalStage3BlockedAtAux(
                    initialContext, candidate, position, rank)
              => ENABLED
                   <<IndexedRunHistoricalRecoveryStep(
                       initialContext, candidate.node)>>_IndexedChainVars
        BY <1>1, IndexedHistoricalStage3BlockedHasRunnerPending,
           IndexedHistoricalCandidateRunnerEnablesFairOccurrence
      <3>3. /\ IndexedHistoricalTemporalStage3BlockedAtAux(
                   initialContext, candidate, position, rank)
               /\ <<IndexedRunHistoricalRecoveryStep(
                      initialContext, candidate.node)>>_IndexedChainVars
              => \/ IndexedHistoricalTemporalStage3AuxProgress(
                      initialContext, candidate, position, rank)'
                 \/ IndexedHistoricalTemporalStage3ServeEpisodeResidual(
                      initialContext, candidate, position, rank)'
        BY <1>1, IndexedHistoricalStage3RunnerProducesOutcome
      <3>4. CASE candidate.node \in Responsive
        <4>1. WF_IndexedChainVars(
                 IndexedRunHistoricalRecoveryStep(
                   initialContext, candidate.node))
          BY <1>1, <3>4 DEF IndexedChainSpec, IndexedFairness
        <4>2. IndexedHistoricalTemporalStage3BlockedAtAux(
                 initialContext, candidate, position, rank)
                 ~> (IndexedHistoricalTemporalStage3AuxProgress(
                       initialContext, candidate, position, rank)
                      \/ IndexedHistoricalTemporalStage3ServeEpisodeResidual(
                           initialContext, candidate, position, rank))
          BY <2>1, <2>2, <3>1, <3>2, <3>3, <4>1, PTL
        <4>3. IndexedHistoricalTemporalStage3ServeEpisodeResidual(
                 initialContext, candidate, position, rank)
                 ~> IndexedHistoricalTemporalStage3AuxProgress(
                      initialContext, candidate, position, rank)
          BY <1>1
             DEF IndexedHistoricalTemporalStage3FiniteServeEpisodeResidualProperty
        <4> QED BY <4>2, <4>3, PTL
      <3>5. CASE candidate.node \notin Responsive
        <4>1. []~IndexedHistoricalTemporalStage3BlockedAtAux(
                      initialContext, candidate, position, rank)
          BY <3>5, PTL
             DEF IndexedHistoricalTemporalStage3BlockedAtAux,
                 IndexedHistoricalTransport!
                   HistoricalTemporalStage3AuxBlocked,
                 IndexedHistoricalTransport!
                   HistoricalTemporalStage3Pending,
                 IndexedHistoricalTransport!
                   HistoricalProtectedOwnedAtServiceRank,
                 IndexedHistoricalTransport!
                   HistoricalProtectedCandidateOwned
        <4> QED BY <4>1, PTL
      <3> QED BY <3>4, <3>5
    <2>6. \A rank \in
               IndexedHistoricalTransport(initialContext)!
                 ReadyRunAuxCarrier:
             IndexedHistoricalTemporalStage3BlockedAtAux(
               initialContext, candidate, position, rank)
               ~> IndexedHistoricalTemporalRankProgressExit(
                    initialContext, candidate, <<3, position>>)
      BY <2>5,
         IndexedHistoricalTransport(initialContext)!
           ReadyRunAuxOrderingIsWellFounded,
         WellFoundedLeadsTo
         DEF IndexedHistoricalTemporalStage3AuxProgress,
             IndexedHistoricalTemporalRankProgressExit,
             IndexedHistoricalTransport!
               HistoricalTemporalStage3AuxProgress,
             IndexedHistoricalTransport!
               HistoricalTemporalStage3RankExit
    <2> QED BY <2>4, <2>6, PTL
  <1> QED BY <1>1

IndexedHistoricalTemporalStage3Source(
    initialContext, candidate, position) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalProtectedOwnedAtServiceRank(
      candidate, <<3, position>>)

IndexedHistoricalTemporalStage3Goal(
    initialContext, candidate, position) ==
  \/ IndexedHistoricalTransport(initialContext)!
       HistoricalProtectedServiceOwnershipExit(candidate)
  \/ \E lower \in SetLessThan(
       <<3, position>>,
       IndexedHistoricalTransport(initialContext)!
         OwnedServiceRankOrdering,
       IndexedHistoricalTransport(initialContext)!
         OwnedServiceRankCarrier):
       IndexedHistoricalTransport(initialContext)!
         HistoricalProtectedOwnedAtServiceRank(candidate, lower)

IndexedHistoricalTemporalStage3LeafProperty ==
  \A initialContext \in AdmissibleContextRecords,
     candidate \in IndexedHistoricalTransport(initialContext)!
                    AsyncCandidateSet,
     position \in Nat:
    IndexedHistoricalTemporalStage3Source(
      initialContext, candidate, position)
      ~> IndexedHistoricalTemporalStage3Goal(
           initialContext, candidate, position)

THEOREM IndexedChainSpecClosesHistoricalTemporalStage3Leaf ==
  /\ IndexedChainSpec
  /\ IndexedHistoricalTemporalStage3FiniteServeEpisodeResidualProperty
    => IndexedHistoricalTemporalStage3LeafProperty
PROOF
  <1>1. ASSUME IndexedChainSpec,
                IndexedHistoricalTemporalStage3FiniteServeEpisodeResidualProperty,
                NEW initialContext \in AdmissibleContextRecords,
                NEW candidate \in
                  IndexedHistoricalTransport(initialContext)!
                    AsyncCandidateSet,
                NEW position \in Nat
         PROVE IndexedHistoricalTemporalStage3Source(
                 initialContext, candidate, position)
                 ~>
               IndexedHistoricalTemporalStage3Goal(
                 initialContext, candidate, position)
    <2>1. []IndexedHistoricalTemporalSupportAt(initialContext)
      BY <1>1, IndexedChainSpecAlwaysHistoricalTemporalSupport, PTL
    <2>2. IndexedHistoricalTemporalStage3Source(
             initialContext, candidate, position)
             ~> IndexedHistoricalTemporalStage3Pending(
                  initialContext, candidate, position)
      BY <2>1, PTL
         DEF IndexedHistoricalTemporalStage3Source,
             IndexedHistoricalTemporalStage3Pending,
             IndexedHistoricalTemporalSupportAt,
             IndexedHistoricalTransport!
               HistoricalTemporalStage3Pending
    <2>3. IndexedHistoricalTemporalStage3Pending(
             initialContext, candidate, position)
             ~> IndexedHistoricalTemporalRankProgressExit(
                  initialContext, candidate, <<3, position>>)
      BY <1>1,
         IndexedChainSpecClosesHistoricalTemporalStage3Rank
    <2>4. IndexedHistoricalTemporalSupportAt(initialContext)
             /\ IndexedHistoricalTemporalRankProgressExit(
                  initialContext, candidate, <<3, position>>)
             => IndexedHistoricalTemporalStage3Goal(
                  initialContext, candidate, position)
      BY <1>1,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalRankExitHasWellFoundedSuccessor, Isa
         DEF IndexedHistoricalTemporalSupportAt,
             IndexedHistoricalTemporalRankProgressExit,
             IndexedHistoricalTemporalStage3Goal,
             IndexedHistoricalTransport!
               HistoricalTemporalRankProgressExit,
             IndexedHistoricalTransport!
               HistoricalProtectedServiceOwnershipExit,
             IndexedHistoricalTransport!OwnedServiceRankCarrier
    <2>5. IndexedHistoricalTemporalRankProgressExit(
             initialContext, candidate, <<3, position>>)
             ~> IndexedHistoricalTemporalStage3Goal(
                  initialContext, candidate, position)
      BY <2>1, <2>4, PTL
    <2> QED BY <2>2, <2>3, <2>5, PTL
  <1> QED BY <1>1
       DEF IndexedHistoricalTemporalStage3LeafProperty

(***************************************************************************
Indexed historical Stage-4 lift.

The one-height source has already combined actionable, causal-capacity, and
runner-prefix work into one well-founded episode rank.  The product lift
therefore needs only the same exact historical runner occurrence.
***************************************************************************)

IndexedHistoricalTemporalStage4Pending(
    initialContext, candidate, position) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalTemporalStage4Pending(candidate, position)

IndexedHistoricalTemporalStage4BlockedAtRank(
    initialContext, candidate, position, rank) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalTemporalStage4BlockedAtRank(
      candidate, position, rank)

IndexedHistoricalTemporalStage4Progress(
    initialContext, candidate, position, rank) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalTemporalStage4Progress(
      candidate, position, rank)

IndexedHistoricalTemporalStage4ServeEpisodeResidual(
    initialContext, candidate, position, rank) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalTemporalStage4ServeEpisodeResidual(
      candidate, position, rank)

IndexedHistoricalTemporalStage4FiniteServeEpisodeResidualProperty ==
  \A initialContext \in AdmissibleContextRecords,
     candidate, position:
    \A rank \in IndexedHistoricalTransport(initialContext)!
                   HistoricalTemporalStage4EpisodeCarrier:
      IndexedHistoricalTemporalStage4ServeEpisodeResidual(
        initialContext, candidate, position, rank)
        ~> IndexedHistoricalTemporalStage4Progress(
             initialContext, candidate, position, rank)

THEOREM IndexedHistoricalStage4BlockedHasRunnerPending ==
  \A initialContext \in AdmissibleContextRecords,
     candidate, position, rank:
    IndexedHistoricalTemporalStage4BlockedAtRank(
      initialContext, candidate, position, rank)
      => IndexedHistoricalTemporalCandidateRunnerPending(
           initialContext, candidate)
BY DEF IndexedHistoricalTemporalStage4BlockedAtRank,
       IndexedHistoricalTemporalCandidateRunnerPending,
       IndexedHistoricalTemporalSupportAt,
       IndexedHistoricalTransport!
         HistoricalTemporalStage4BlockedAtRank,
       IndexedHistoricalTransport!
         HistoricalTemporalStage4Pending,
       IndexedHistoricalTransport!
         HistoricalProtectedOwnedAtServiceRank

THEOREM IndexedHistoricalStage4RunnerProducesOutcome ==
  \A initialContext \in AdmissibleContextRecords,
     candidate, position:
    \A rank \in IndexedHistoricalTransport(initialContext)!
                   HistoricalTemporalStage4EpisodeCarrier:
      /\ IndexedHistoricalTemporalStage4BlockedAtRank(
           initialContext, candidate, position, rank)
      /\ <<IndexedRunHistoricalRecoveryStep(
             initialContext, candidate.node)>>_IndexedChainVars
      => \/ IndexedHistoricalTemporalStage4Progress(
              initialContext, candidate, position, rank)'
         \/ IndexedHistoricalTemporalStage4ServeEpisodeResidual(
              initialContext, candidate, position, rank)'
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW candidate, NEW position,
                NEW rank \in
                  IndexedHistoricalTransport(initialContext)!
                    HistoricalTemporalStage4EpisodeCarrier,
                IndexedHistoricalTemporalStage4BlockedAtRank(
                  initialContext, candidate, position, rank),
                <<IndexedRunHistoricalRecoveryStep(
                    initialContext, candidate.node)>>_IndexedChainVars
         PROVE \/ IndexedHistoricalTemporalStage4Progress(
                    initialContext, candidate, position, rank)'
               \/ IndexedHistoricalTemporalStage4ServeEpisodeResidual(
                    initialContext, candidate, position, rank)'
    <2>1. IndexedHistoricalTransport(initialContext)!
             PostGstRunHistoricalRecoveryNode(candidate.node)
      BY <1>1, Isa
         DEF IndexedRunHistoricalRecoveryStep,
             IndexedAsync!PostGstRunHistoricalRecoveryNode,
             IndexedHistoricalTransport!
               PostGstRunHistoricalRecoveryNode
    <2> QED BY <1>1, <2>1,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage4SameRunnerProducesOutcome
         DEF IndexedHistoricalTemporalStage4BlockedAtRank,
             IndexedHistoricalTemporalStage4Progress,
             IndexedHistoricalTemporalStage4ServeEpisodeResidual
  <1> QED BY <1>1

THEOREM IndexedHistoricalStage4UnlessProgress ==
  \A initialContext \in AdmissibleContextRecords,
     candidate, position:
    \A rank \in IndexedHistoricalTransport(initialContext)!
                   HistoricalTemporalStage4EpisodeCarrier:
      /\ IndexedHistoricalTemporalStage4BlockedAtRank(
           initialContext, candidate, position, rank)
      /\ [IndexedChainNext]_IndexedChainVars
      => \/ IndexedHistoricalTemporalStage4BlockedAtRank(
              initialContext, candidate, position, rank)'
         \/ IndexedHistoricalTemporalStage4Progress(
              initialContext, candidate, position, rank)'
         \/ IndexedHistoricalTemporalStage4ServeEpisodeResidual(
              initialContext, candidate, position, rank)'
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW candidate, NEW position,
                NEW rank \in
                  IndexedHistoricalTransport(initialContext)!
                    HistoricalTemporalStage4EpisodeCarrier,
                IndexedHistoricalTemporalStage4BlockedAtRank(
                  initialContext, candidate, position, rank),
                [IndexedChainNext]_IndexedChainVars
         PROVE
           \/ IndexedHistoricalTemporalStage4BlockedAtRank(
                initialContext, candidate, position, rank)'
           \/ IndexedHistoricalTemporalStage4Progress(
                initialContext, candidate, position, rank)'
           \/ IndexedHistoricalTemporalStage4ServeEpisodeResidual(
                initialContext, candidate, position, rank)'
    <2>1. [IndexedHistoricalTransport(initialContext)!AsyncNext]_(
             IndexedHistoricalTransport(initialContext)!AsyncAllVars)
      BY <1>1,
         IndexedBracketStepProjectsEveryHistoricalTransportStep
    <2>2. CASE IndexedHistoricalTransport(initialContext)!
                  PostGstRunHistoricalRecoveryNode(candidate.node)
      BY <1>1, <2>2,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage4SameRunnerProducesOutcome, Isa
         DEF IndexedHistoricalTemporalStage4BlockedAtRank,
             IndexedHistoricalTemporalStage4Progress,
             IndexedHistoricalTemporalStage4ServeEpisodeResidual
    <2>3. CASE ~IndexedHistoricalTransport(initialContext)!
                   PostGstRunHistoricalRecoveryNode(candidate.node)
      BY <1>1, <2>1, <2>3,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage4OtherStepUnlessProgress
         DEF IndexedHistoricalTemporalStage4BlockedAtRank,
             IndexedHistoricalTemporalStage4Progress,
             IndexedHistoricalTemporalStage4ServeEpisodeResidual
    <2> QED BY <2>2, <2>3
  <1> QED BY <1>1

THEOREM IndexedChainSpecClosesHistoricalTemporalStage4Rank ==
  /\ IndexedChainSpec
  /\ IndexedHistoricalTemporalStage4FiniteServeEpisodeResidualProperty
    => \A initialContext \in AdmissibleContextRecords,
          candidate, position \in Nat:
         IndexedHistoricalTemporalStage4Pending(
           initialContext, candidate, position)
           ~> IndexedHistoricalTemporalRankProgressExit(
                initialContext, candidate, <<4, position>>)
PROOF
  <1>1. ASSUME IndexedChainSpec,
                IndexedHistoricalTemporalStage4FiniteServeEpisodeResidualProperty,
                NEW initialContext \in AdmissibleContextRecords,
                NEW candidate, NEW position \in Nat
         PROVE IndexedHistoricalTemporalStage4Pending(
                 initialContext, candidate, position)
                 ~>
               IndexedHistoricalTemporalRankProgressExit(
                 initialContext, candidate, <<4, position>>)
    <2>1. []IndexedCompositionInvariant
      BY <1>1, IndexedChainSpecEstablishesCompositionInvariant
    <2>2. [][IndexedChainNext]_IndexedChainVars
      BY <1>1 DEF IndexedChainSpec
    <2>3. []IndexedHistoricalTemporalSupportAt(initialContext)
      BY <1>1, IndexedChainSpecAlwaysHistoricalTemporalSupport, PTL
    <2>4. IndexedHistoricalTemporalStage4Pending(
             initialContext, candidate, position)
             ~> \E rank \in
                  IndexedHistoricalTransport(initialContext)!
                    HistoricalTemporalStage4EpisodeCarrier:
                  IndexedHistoricalTemporalStage4BlockedAtRank(
                    initialContext, candidate, position, rank)
      BY <2>3,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage4CarrierFacts, PTL
         DEF IndexedHistoricalTemporalStage4Pending,
             IndexedHistoricalTemporalStage4BlockedAtRank,
             IndexedHistoricalTransport!
               HistoricalTemporalStage4BlockedAtRank,
             IndexedHistoricalTransport!
               HistoricalTemporalStage4Pending,
             IndexedHistoricalTransport!
               HistoricalTemporalStage4EpisodeRank,
             IndexedHistoricalTransport!
               HistoricalTemporalRankProgressExit,
             IndexedHistoricalTransport!
               HistoricalProtectedOwnedAtServiceRank,
             IndexedHistoricalTransport!
               HistoricalProtectedServiceOwnershipExit,
             IndexedHistoricalTransport!ServiceRankLess
    <2>5. ASSUME NEW rank \in
                  IndexedHistoricalTransport(initialContext)!
                    HistoricalTemporalStage4EpisodeCarrier
           PROVE IndexedHistoricalTemporalStage4BlockedAtRank(
                   initialContext, candidate, position, rank)
                   ~> IndexedHistoricalTemporalStage4Progress(
                        initialContext, candidate, position, rank)
      <3>1. IndexedHistoricalTemporalStage4BlockedAtRank(
               initialContext, candidate, position, rank)
               /\ [IndexedChainNext]_IndexedChainVars
              => \/ IndexedHistoricalTemporalStage4BlockedAtRank(
                      initialContext, candidate, position, rank)'
                 \/ IndexedHistoricalTemporalStage4Progress(
                      initialContext, candidate, position, rank)'
                 \/ IndexedHistoricalTemporalStage4ServeEpisodeResidual(
                      initialContext, candidate, position, rank)'
        BY <1>1, IndexedHistoricalStage4UnlessProgress
      <3>2. IndexedCompositionInvariant
               /\ IndexedHistoricalTemporalStage4BlockedAtRank(
                    initialContext, candidate, position, rank)
              => ENABLED
                   <<IndexedRunHistoricalRecoveryStep(
                       initialContext, candidate.node)>>_IndexedChainVars
        BY <1>1, IndexedHistoricalStage4BlockedHasRunnerPending,
           IndexedHistoricalCandidateRunnerEnablesFairOccurrence
      <3>3. /\ IndexedHistoricalTemporalStage4BlockedAtRank(
                   initialContext, candidate, position, rank)
               /\ <<IndexedRunHistoricalRecoveryStep(
                      initialContext, candidate.node)>>_IndexedChainVars
              => \/ IndexedHistoricalTemporalStage4Progress(
                      initialContext, candidate, position, rank)'
                 \/ IndexedHistoricalTemporalStage4ServeEpisodeResidual(
                      initialContext, candidate, position, rank)'
        BY <1>1, IndexedHistoricalStage4RunnerProducesOutcome
      <3>4. CASE candidate.node \in Responsive
        <4>1. WF_IndexedChainVars(
                 IndexedRunHistoricalRecoveryStep(
                   initialContext, candidate.node))
          BY <1>1, <3>4 DEF IndexedChainSpec, IndexedFairness
        <4>2. IndexedHistoricalTemporalStage4BlockedAtRank(
                 initialContext, candidate, position, rank)
                 ~> (IndexedHistoricalTemporalStage4Progress(
                       initialContext, candidate, position, rank)
                      \/ IndexedHistoricalTemporalStage4ServeEpisodeResidual(
                           initialContext, candidate, position, rank))
          BY <2>1, <2>2, <3>1, <3>2, <3>3, <4>1, PTL
        <4>3. IndexedHistoricalTemporalStage4ServeEpisodeResidual(
                 initialContext, candidate, position, rank)
                 ~> IndexedHistoricalTemporalStage4Progress(
                      initialContext, candidate, position, rank)
          BY <1>1
             DEF IndexedHistoricalTemporalStage4FiniteServeEpisodeResidualProperty
        <4> QED BY <4>2, <4>3, PTL
      <3>5. CASE candidate.node \notin Responsive
        <4>1. []~IndexedHistoricalTemporalStage4BlockedAtRank(
                      initialContext, candidate, position, rank)
          BY <3>5, PTL
             DEF IndexedHistoricalTemporalStage4BlockedAtRank,
                 IndexedHistoricalTransport!
                   HistoricalTemporalStage4BlockedAtRank,
                 IndexedHistoricalTransport!
                   HistoricalTemporalStage4Pending,
                 IndexedHistoricalTransport!
                   HistoricalProtectedOwnedAtServiceRank,
                 IndexedHistoricalTransport!
                   HistoricalProtectedCandidateOwned
        <4> QED BY <4>1, PTL
      <3> QED BY <3>4, <3>5
    <2>6. \A rank \in
               IndexedHistoricalTransport(initialContext)!
                 HistoricalTemporalStage4EpisodeCarrier:
             IndexedHistoricalTemporalStage4BlockedAtRank(
               initialContext, candidate, position, rank)
               ~> IndexedHistoricalTemporalRankProgressExit(
                    initialContext, candidate, <<4, position>>)
      BY <2>5,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage4EpisodeOrderingIsWellFounded,
         WellFoundedLeadsTo
         DEF IndexedHistoricalTemporalStage4Progress,
             IndexedHistoricalTemporalRankProgressExit,
             IndexedHistoricalTransport!
               HistoricalTemporalStage4Progress
    <2> QED BY <2>4, <2>6, PTL
  <1> QED BY <1>1

IndexedHistoricalTemporalStage4Source(
    initialContext, candidate, position) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalProtectedOwnedAtServiceRank(
      candidate, <<4, position>>)

IndexedHistoricalTemporalStage4Goal(
    initialContext, candidate, position) ==
  \/ IndexedHistoricalTransport(initialContext)!
       HistoricalProtectedServiceOwnershipExit(candidate)
  \/ \E lower \in SetLessThan(
       <<4, position>>,
       IndexedHistoricalTransport(initialContext)!
         OwnedServiceRankOrdering,
       IndexedHistoricalTransport(initialContext)!
         OwnedServiceRankCarrier):
       IndexedHistoricalTransport(initialContext)!
         HistoricalProtectedOwnedAtServiceRank(candidate, lower)

IndexedHistoricalTemporalStage4LeafProperty ==
  \A initialContext \in AdmissibleContextRecords,
     candidate \in IndexedHistoricalTransport(initialContext)!
                    AsyncCandidateSet,
     position \in Nat:
    IndexedHistoricalTemporalStage4Source(
      initialContext, candidate, position)
      ~> IndexedHistoricalTemporalStage4Goal(
           initialContext, candidate, position)

THEOREM IndexedChainSpecClosesHistoricalTemporalStage4Leaf ==
  /\ IndexedChainSpec
  /\ IndexedHistoricalTemporalStage4FiniteServeEpisodeResidualProperty
    => IndexedHistoricalTemporalStage4LeafProperty
PROOF
  <1>1. ASSUME IndexedChainSpec,
                IndexedHistoricalTemporalStage4FiniteServeEpisodeResidualProperty,
                NEW initialContext \in AdmissibleContextRecords,
                NEW candidate \in
                  IndexedHistoricalTransport(initialContext)!
                    AsyncCandidateSet,
                NEW position \in Nat
         PROVE IndexedHistoricalTemporalStage4Source(
                 initialContext, candidate, position)
                 ~>
               IndexedHistoricalTemporalStage4Goal(
                 initialContext, candidate, position)
    <2>1. []IndexedHistoricalTemporalSupportAt(initialContext)
      BY <1>1, IndexedChainSpecAlwaysHistoricalTemporalSupport, PTL
    <2>2. IndexedHistoricalTemporalStage4Source(
             initialContext, candidate, position)
             ~> IndexedHistoricalTemporalStage4Pending(
                  initialContext, candidate, position)
      BY <2>1, PTL
         DEF IndexedHistoricalTemporalStage4Source,
             IndexedHistoricalTemporalStage4Pending,
             IndexedHistoricalTemporalSupportAt,
             IndexedHistoricalTransport!
               HistoricalTemporalStage4Pending
    <2>3. IndexedHistoricalTemporalStage4Pending(
             initialContext, candidate, position)
             ~> IndexedHistoricalTemporalRankProgressExit(
                  initialContext, candidate, <<4, position>>)
      BY <1>1,
         IndexedChainSpecClosesHistoricalTemporalStage4Rank
    <2>4. IndexedHistoricalTemporalSupportAt(initialContext)
             /\ IndexedHistoricalTemporalRankProgressExit(
                  initialContext, candidate, <<4, position>>)
             => IndexedHistoricalTemporalStage4Goal(
                  initialContext, candidate, position)
      BY <1>1,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalRankExitHasWellFoundedSuccessor, Isa
         DEF IndexedHistoricalTemporalSupportAt,
             IndexedHistoricalTemporalRankProgressExit,
             IndexedHistoricalTemporalStage4Goal,
             IndexedHistoricalTransport!
               HistoricalTemporalRankProgressExit,
             IndexedHistoricalTransport!
               HistoricalProtectedServiceOwnershipExit,
             IndexedHistoricalTransport!OwnedServiceRankCarrier
    <2>5. IndexedHistoricalTemporalRankProgressExit(
             initialContext, candidate, <<4, position>>)
             ~> IndexedHistoricalTemporalStage4Goal(
                  initialContext, candidate, position)
      BY <2>1, <2>4, PTL
    <2> QED BY <2>2, <2>3, <2>5, PTL
  <1> QED BY <1>1
       DEF IndexedHistoricalTemporalStage4LeafProperty

(***************************************************************************
Indexed historical Stage-5 lift.

This lift intentionally avoids `AsyncSpecAt`: a lagging historical target can
own a joined exact context before every other responsive peer joins it.  The
product already supplies weak fairness for the target's exact historical I/O
step.  The action-local theorem above supplies enabledness, nonstuttering
queue removal, and strict FIFO descent for that one owner.
***************************************************************************)

IndexedHistoricalTemporalStage5Pending(
    initialContext, candidate, position) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalTemporalStage5Pending(candidate, position)

THEOREM IndexedHistoricalStage5PendingHasJoinedFairOwner ==
  \A initialContext \in AdmissibleContextRecords,
     candidate, position:
    /\ IndexedCompositionInvariant
    /\ IndexedHistoricalTemporalStage5Pending(
         initialContext, candidate, position)
    => /\ candidate.node \in Responsive
       /\ candidate.node \in joinedByContext[initialContext]
       /\ initialContext \in JoinedContexts
       /\ IndexedHistoricalTransport(initialContext)!
            HistoricalRecoveryTarget(candidate.node)
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW candidate, NEW position,
                IndexedCompositionInvariant,
                IndexedHistoricalTemporalStage5Pending(
                  initialContext, candidate, position)
         PROVE /\ candidate.node \in Responsive
               /\ candidate.node \in joinedByContext[initialContext]
               /\ initialContext \in JoinedContexts
               /\ IndexedHistoricalTransport(initialContext)!
                    HistoricalRecoveryTarget(candidate.node)
    <2>1. /\ candidate.node \in Responsive
           /\ IndexedHistoricalTransport(initialContext)!
                HistoricalRecoveryTarget(candidate.node)
      BY <1>1,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage5CarrierFacts
         DEF IndexedHistoricalTemporalStage5Pending
    <2>2. IndexedAsync(initialContext)!
             HistoricalRecoveryTarget(candidate.node)
      BY <2>1, Isa
         DEF IndexedHistoricalTransport!HistoricalRecoveryTarget,
             IndexedAsync!HistoricalRecoveryTarget,
             IndexedScheduler
    <2>3. candidate.node \in joinedByContext[initialContext]
      BY <1>1, <2>2
         DEF IndexedCompositionInvariant,
             IndexedHistoricalRecoveryTargetCoherence
    <2>4. initialContext \in JoinedContexts
      BY <2>3 DEF JoinedContexts
    <2> QED BY <2>1, <2>3, <2>4
  <1> QED BY <1>1

THEOREM IndexedHistoricalStage5PendingEnablesExactWorker ==
  \A initialContext \in AdmissibleContextRecords,
     candidate, position:
    /\ IndexedCompositionInvariant
    /\ IndexedHistoricalTemporalStage5Pending(
         initialContext, candidate, position)
    => ENABLED
         IndexedHistoricalRecoveryIoWorkerStep(
           initialContext, candidate.node)
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW candidate, NEW position,
                IndexedCompositionInvariant,
                IndexedHistoricalTemporalStage5Pending(
                  initialContext, candidate, position)
         PROVE ENABLED
                 IndexedHistoricalRecoveryIoWorkerStep(
                   initialContext, candidate.node)
    <2>1. /\ candidate.node \in Responsive
           /\ candidate.node \in joinedByContext[initialContext]
           /\ initialContext \in JoinedContexts
      BY <1>1, IndexedHistoricalStage5PendingHasJoinedFairOwner
    <2>2. ENABLED
             IndexedHistoricalTransport(initialContext)!
               PostGstServiceHistoricalRecoveryIoWorker(candidate.node)
      BY <1>1,
         IndexedHistoricalTransport(initialContext)!
           HistoricalRecoveryIoWorkerEnabledAfterGst
         DEF IndexedHistoricalTemporalStage5Pending,
             IndexedHistoricalTransport!
               HistoricalTemporalStage5Pending,
             IndexedHistoricalTransport!
               HistoricalProtectedOwnedAtServiceRank,
             IndexedHistoricalTransport!
               HistoricalProtectedCandidateOwned,
             IndexedHistoricalTransport!HistoricalRecoveryTarget
    <2>3. ENABLED IndexedAsync(initialContext)!
             PostGstServiceHistoricalRecoveryIoWorker(candidate.node)
      BY <2>2, Isa
         DEF IndexedHistoricalTransport!
               PostGstServiceHistoricalRecoveryIoWorker,
             IndexedAsync!
               PostGstServiceHistoricalRecoveryIoWorker,
             IndexedHistoricalTransport!
               ServiceHistoricalRecoveryIoWorker,
             IndexedAsync!ServiceHistoricalRecoveryIoWorker,
             IndexedHistoricalTransport!ServiceIoWorkerWork,
             IndexedAsync!ServiceIoWorkerWork
    <2>4. ENABLED
             IndexedHistoricalRecoveryIoWorkerStep(
               initialContext, candidate.node)
      BY <1>1, <2>1, <2>3,
         IndexedFairActionsRemainEnabledInProduct
    <2> QED BY <2>4
  <1> QED BY <1>1

THEOREM IndexedHistoricalStage5WorkerIsNonstuttering ==
  \A initialContext \in AdmissibleContextRecords,
     candidate, position:
    /\ IndexedHistoricalTemporalStage5Pending(
         initialContext, candidate, position)
    /\ IndexedHistoricalRecoveryIoWorkerStep(
         initialContext, candidate.node)
    => <<IndexedHistoricalRecoveryIoWorkerStep(
           initialContext, candidate.node)>>_IndexedChainVars
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW candidate, NEW position,
                IndexedHistoricalTemporalStage5Pending(
                  initialContext, candidate, position),
                IndexedHistoricalRecoveryIoWorkerStep(
                  initialContext, candidate.node)
         PROVE <<IndexedHistoricalRecoveryIoWorkerStep(
                   initialContext, candidate.node)>>_IndexedChainVars
    <2>1. <<IndexedHistoricalTransport(initialContext)!
               PostGstServiceHistoricalRecoveryIoWorker(
                 candidate.node)>>_(
             IndexedHistoricalTransport(initialContext)!AsyncAllVars)
      BY <1>1,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalQueuedIoServiceIsNonstuttering, Isa
         DEF IndexedHistoricalTemporalStage5Pending,
             IndexedHistoricalRecoveryIoWorkerStep,
             IndexedHistoricalTransport!
               HistoricalTemporalStage5Pending,
             IndexedHistoricalTransport!
               HistoricalProtectedOwnedAtServiceRank,
             IndexedHistoricalTransport!
               HistoricalProtectedCandidateOwned,
             IndexedHistoricalTransport!HistoricalRecoveryTarget,
             IndexedHistoricalTransport!
               PostGstServiceHistoricalRecoveryIoWorker,
             IndexedAsync!
               PostGstServiceHistoricalRecoveryIoWorker
    <2>2. IndexedAsyncStateShape
      BY <1>1
         DEF IndexedHistoricalRecoveryIoWorkerStep, IndexedChainNext
    <2>3. IndexedHistoricalTransport(initialContext)!AsyncAllVars =
             IndexedAsyncStateAt(initialContext)
      BY <1>1, <2>2, IndexedHistoricalTransportVariablesAreExact
    <2>4. IndexedChainVars' # IndexedChainVars
      BY <2>1, <2>3, Isa
         DEF IndexedChainVars, IndexedAsyncStateAt
    <2> QED BY <1>1, <2>4
  <1> QED BY <1>1

THEOREM IndexedHistoricalStage5PendingEnablesFairOccurrence ==
  \A initialContext \in AdmissibleContextRecords,
     candidate, position:
    /\ IndexedCompositionInvariant
    /\ IndexedHistoricalTemporalStage5Pending(
         initialContext, candidate, position)
    => ENABLED
         <<IndexedHistoricalRecoveryIoWorkerStep(
             initialContext, candidate.node)>>_IndexedChainVars
BY IndexedHistoricalStage5PendingEnablesExactWorker,
   IndexedHistoricalStage5WorkerIsNonstuttering,
   ENABLEDaxioms

THEOREM IndexedHistoricalStage5FairOccurrenceStrictlyProgresses ==
  \A initialContext \in AdmissibleContextRecords,
     candidate, position:
    /\ IndexedHistoricalTemporalStage5Pending(
         initialContext, candidate, position)
    /\ <<IndexedHistoricalRecoveryIoWorkerStep(
           initialContext, candidate.node)>>_IndexedChainVars
    => IndexedHistoricalTemporalRankProgressExit(
         initialContext, candidate, <<5, position>>)'
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW candidate, NEW position,
                IndexedHistoricalTemporalStage5Pending(
                  initialContext, candidate, position),
                <<IndexedHistoricalRecoveryIoWorkerStep(
                    initialContext, candidate.node)>>_IndexedChainVars
         PROVE IndexedHistoricalTemporalRankProgressExit(
                 initialContext, candidate, <<5, position>>)'
    <2>1. IndexedHistoricalTransport(initialContext)!
             PostGstServiceHistoricalRecoveryIoWorker(candidate.node)
      BY <1>1, Isa
         DEF IndexedHistoricalRecoveryIoWorkerStep,
             IndexedAsync!
               PostGstServiceHistoricalRecoveryIoWorker,
             IndexedHistoricalTransport!
               PostGstServiceHistoricalRecoveryIoWorker
    <2> QED BY <1>1, <2>1,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage5WorkerStrictlyProgresses
         DEF IndexedHistoricalTemporalStage5Pending,
             IndexedHistoricalTemporalRankProgressExit
  <1> QED BY <1>1

THEOREM IndexedHistoricalStage5UnlessProgress ==
  \A initialContext \in AdmissibleContextRecords,
     candidate, position:
    /\ IndexedHistoricalTemporalStage5Pending(
         initialContext, candidate, position)
    /\ [IndexedChainNext]_IndexedChainVars
    => \/ IndexedHistoricalTemporalStage5Pending(
            initialContext, candidate, position)'
       \/ IndexedHistoricalTemporalRankProgressExit(
            initialContext, candidate, <<5, position>>)'
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW candidate, NEW position,
                IndexedHistoricalTemporalStage5Pending(
                  initialContext, candidate, position),
                [IndexedChainNext]_IndexedChainVars
         PROVE
           \/ IndexedHistoricalTemporalStage5Pending(
                initialContext, candidate, position)'
           \/ IndexedHistoricalTemporalRankProgressExit(
                initialContext, candidate, <<5, position>>)'
    <2>1. [IndexedHistoricalTransport(initialContext)!AsyncNext]_(
             IndexedHistoricalTransport(initialContext)!AsyncAllVars)
      BY <1>1,
         IndexedBracketStepProjectsEveryHistoricalTransportStep
    <2> QED BY <1>1, <2>1,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage5UnlessProgress
         DEF IndexedHistoricalTemporalStage5Pending,
             IndexedHistoricalTemporalRankProgressExit
  <1> QED BY <1>1

THEOREM IndexedChainSpecClosesHistoricalTemporalStage5Rank ==
  IndexedChainSpec
    => \A initialContext \in AdmissibleContextRecords,
          candidate, position \in Nat:
         IndexedHistoricalTemporalStage5Pending(
           initialContext, candidate, position)
           ~> IndexedHistoricalTemporalRankProgressExit(
                initialContext, candidate, <<5, position>>)
PROOF
  <1>1. ASSUME IndexedChainSpec,
                NEW initialContext \in AdmissibleContextRecords,
                NEW candidate, NEW position \in Nat
         PROVE IndexedHistoricalTemporalStage5Pending(
                 initialContext, candidate, position)
                 ~>
               IndexedHistoricalTemporalRankProgressExit(
                 initialContext, candidate, <<5, position>>)
    <2>1. []IndexedCompositionInvariant
      BY <1>1, IndexedChainSpecEstablishesCompositionInvariant
    <2>2. [][IndexedChainNext]_IndexedChainVars
      BY <1>1 DEF IndexedChainSpec
    <2>3. IndexedHistoricalTemporalStage5Pending(
             initialContext, candidate, position)
             /\ [IndexedChainNext]_IndexedChainVars
             => \/ IndexedHistoricalTemporalStage5Pending(
                    initialContext, candidate, position)'
                \/ IndexedHistoricalTemporalRankProgressExit(
                     initialContext, candidate, <<5, position>>)'
      BY <1>1, IndexedHistoricalStage5UnlessProgress
    <2>4. IndexedCompositionInvariant
             /\ IndexedHistoricalTemporalStage5Pending(
                  initialContext, candidate, position)
             => ENABLED
                  <<IndexedHistoricalRecoveryIoWorkerStep(
                      initialContext, candidate.node)>>_IndexedChainVars
      BY <1>1, IndexedHistoricalStage5PendingEnablesFairOccurrence
    <2>5. /\ IndexedHistoricalTemporalStage5Pending(
               initialContext, candidate, position)
             /\ <<IndexedHistoricalRecoveryIoWorkerStep(
                    initialContext, candidate.node)>>_IndexedChainVars
            => IndexedHistoricalTemporalRankProgressExit(
                 initialContext, candidate, <<5, position>>)'
      BY <1>1,
         IndexedHistoricalStage5FairOccurrenceStrictlyProgresses
    <2>6. CASE candidate.node \in Responsive
      <3>1. WF_IndexedChainVars(
               IndexedHistoricalRecoveryIoWorkerStep(
                 initialContext, candidate.node))
        BY <1>1, <2>6 DEF IndexedChainSpec, IndexedFairness
      <3> QED BY <2>1, <2>2, <2>3, <2>4, <2>5, <3>1, PTL
    <2>7. CASE candidate.node \notin Responsive
      <3>1. []~IndexedHistoricalTemporalStage5Pending(
                    initialContext, candidate, position)
        BY <2>7, PTL
           DEF IndexedHistoricalTemporalStage5Pending,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage5Pending,
               IndexedHistoricalTransport!
                 HistoricalProtectedOwnedAtServiceRank,
               IndexedHistoricalTransport!
                 HistoricalProtectedCandidateOwned
      <3> QED BY <3>1, PTL
    <2> QED BY <2>6, <2>7
  <1> QED BY <1>1

IndexedHistoricalTemporalStage5Source(
    initialContext, candidate, position) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalProtectedOwnedAtServiceRank(
      candidate, <<5, position>>)

IndexedHistoricalTemporalStage5Goal(
    initialContext, candidate, position) ==
  \/ IndexedHistoricalTransport(initialContext)!
       HistoricalProtectedServiceOwnershipExit(candidate)
  \/ \E lower \in SetLessThan(
       <<5, position>>,
       IndexedHistoricalTransport(initialContext)!
         OwnedServiceRankOrdering,
       IndexedHistoricalTransport(initialContext)!
         OwnedServiceRankCarrier):
       IndexedHistoricalTransport(initialContext)!
         HistoricalProtectedOwnedAtServiceRank(candidate, lower)

IndexedHistoricalTemporalStage5LeafProperty ==
  \A initialContext \in AdmissibleContextRecords,
     candidate \in IndexedHistoricalTransport(initialContext)!
                    AsyncCandidateSet,
     position \in Nat:
    IndexedHistoricalTemporalStage5Source(
      initialContext, candidate, position)
      ~> IndexedHistoricalTemporalStage5Goal(
           initialContext, candidate, position)

THEOREM IndexedChainSpecClosesHistoricalTemporalStage5Leaf ==
  IndexedChainSpec
    => IndexedHistoricalTemporalStage5LeafProperty
PROOF
  <1>1. ASSUME IndexedChainSpec,
                NEW initialContext \in AdmissibleContextRecords,
                NEW candidate \in
                  IndexedHistoricalTransport(initialContext)!
                    AsyncCandidateSet,
                NEW position \in Nat
         PROVE IndexedHistoricalTemporalStage5Source(
                 initialContext, candidate, position)
                 ~>
               IndexedHistoricalTemporalStage5Goal(
                 initialContext, candidate, position)
    <2>1. []IndexedHistoricalTemporalSupportAt(initialContext)
      BY <1>1, IndexedChainSpecAlwaysHistoricalTemporalSupport, PTL
    <2>2. IndexedHistoricalTemporalStage5Source(
             initialContext, candidate, position)
             ~>
           IndexedHistoricalTemporalStage5Pending(
             initialContext, candidate, position)
      BY <2>1, PTL
         DEF IndexedHistoricalTemporalStage5Source,
             IndexedHistoricalTemporalStage5Pending,
             IndexedHistoricalTemporalSupportAt,
             IndexedHistoricalTransport!
               HistoricalTemporalStage5Pending
    <2>3. IndexedHistoricalTemporalStage5Pending(
             initialContext, candidate, position)
             ~>
           IndexedHistoricalTemporalRankProgressExit(
             initialContext, candidate, <<5, position>>)
      BY <1>1,
         IndexedChainSpecClosesHistoricalTemporalStage5Rank
    <2>4. IndexedHistoricalTemporalSupportAt(initialContext)
             /\ IndexedHistoricalTemporalRankProgressExit(
                  initialContext, candidate, <<5, position>>)
             => IndexedHistoricalTemporalStage5Goal(
                  initialContext, candidate, position)
      BY <1>1,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalRankExitHasWellFoundedSuccessor, Isa
         DEF IndexedHistoricalTemporalSupportAt,
             IndexedHistoricalTemporalRankProgressExit,
             IndexedHistoricalTemporalStage5Goal,
             IndexedHistoricalTransport!
               HistoricalTemporalRankProgressExit,
             IndexedHistoricalTransport!
               HistoricalProtectedServiceOwnershipExit,
             IndexedHistoricalTransport!OwnedServiceRankCarrier
    <2>5. IndexedHistoricalTemporalRankProgressExit(
             initialContext, candidate, <<5, position>>)
             ~>
           IndexedHistoricalTemporalStage5Goal(
             initialContext, candidate, position)
      BY <2>1, <2>4, PTL
    <2> QED BY <2>2, <2>3, <2>5, PTL
  <1> QED BY <1>1
       DEF IndexedHistoricalTemporalStage5LeafProperty

(***************************************************************************
Indexed historical Stage-6 runner subkernels.

Pre-admission, the owed causal head, and non-Completion capacity all consume
the same joined historical runner.  Their rank carriers differ, so the mode
tag below selects the existing one-height carrier and ordering without
flattening or inventing a new fairness action.
***************************************************************************)

IndexedHistoricalStage6RunnerModes ==
  {"PreAdmission", "Owed", "NonCompletion"}

IndexedHistoricalStage6RunnerCarrier(initialContext, mode) ==
  IF mode = "NonCompletion"
  THEN IndexedHistoricalTransport(initialContext)!
         Stage4CapacityCarrier
  ELSE IndexedHistoricalTransport(initialContext)!
         ReadyRunAuxCarrier

IndexedHistoricalStage6RunnerOrdering(initialContext, mode) ==
  IF mode = "NonCompletion"
  THEN IndexedHistoricalTransport(initialContext)!
         Stage4CapacityOrdering
  ELSE IndexedHistoricalTransport(initialContext)!
         ReadyRunAuxOrdering

IndexedHistoricalStage6RunnerBlocked(
    initialContext, mode, candidate, position, rank) ==
  CASE mode = "PreAdmission" ->
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage6PreAdmissionBlockedAtAux(
             candidate, position, rank)
    [] mode = "Owed" ->
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage6OwedBlockedAtAux(
             candidate, position, rank)
    [] mode = "NonCompletion" ->
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage6NonCompletionBlockedAtRank(
             candidate, position, rank)
    [] OTHER -> FALSE

IndexedHistoricalStage6RunnerProgress(
    initialContext, mode, candidate, position, rank) ==
  CASE mode = "PreAdmission" ->
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage6PreAdmissionAuxProgress(
             candidate, position, rank)
    [] mode = "Owed" ->
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage6OwedAuxProgress(
             candidate, position, rank)
    [] mode = "NonCompletion" ->
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage6NonCompletionProgress(
             candidate, position, rank)
    [] OTHER -> FALSE

IndexedHistoricalStage6RunnerEpisodeResidual(
    initialContext, mode, candidate, position, rank) ==
  CASE mode = "PreAdmission" ->
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage6PreAdmissionRunnerEpisodeResidual(
             candidate, position, rank)
    [] mode = "Owed" ->
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage6OwedRunnerEpisodeResidual(
             candidate, position, rank)
    [] mode = "NonCompletion" ->
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage6NonCompletionServeEpisodeResidual(
             candidate, position, rank)
    [] OTHER -> FALSE

IndexedHistoricalStage6FiniteRunnerEpisodeResidualProperty ==
  \A initialContext \in AdmissibleContextRecords,
     mode \in IndexedHistoricalStage6RunnerModes,
     candidate, position:
    \A rank \in
         IndexedHistoricalStage6RunnerCarrier(initialContext, mode):
      IndexedHistoricalStage6RunnerEpisodeResidual(
        initialContext, mode, candidate, position, rank)
        ~> IndexedHistoricalStage6RunnerProgress(
             initialContext, mode, candidate, position, rank)

IndexedHistoricalFiniteRunnerEpisodeClosureProperty ==
  /\ IndexedHistoricalTemporalStage3FiniteServeEpisodeResidualProperty
  /\ IndexedHistoricalTemporalStage4FiniteServeEpisodeResidualProperty
  /\ IndexedHistoricalStage6FiniteRunnerEpisodeResidualProperty

IndexedHistoricalStage6RunnerGoal(
    initialContext, mode, candidate, position) ==
  CASE mode = "PreAdmission" ->
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage6PreAdmissionGoal(
             candidate, position)
    [] mode = "Owed" ->
         IndexedHistoricalTemporalRankProgressExit(
           initialContext, candidate, <<6, position>>)
    [] mode = "NonCompletion" ->
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage6NonCompletionGoal(
             candidate, position)
    [] OTHER -> FALSE

THEOREM IndexedHistoricalStage6RunnerOrderingIsWellFounded ==
  \A initialContext \in AdmissibleContextRecords,
     mode \in IndexedHistoricalStage6RunnerModes:
    IsWellFoundedOn(
      IndexedHistoricalStage6RunnerOrdering(initialContext, mode),
      IndexedHistoricalStage6RunnerCarrier(initialContext, mode))
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW mode \in IndexedHistoricalStage6RunnerModes
         PROVE IsWellFoundedOn(
                 IndexedHistoricalStage6RunnerOrdering(
                   initialContext, mode),
                 IndexedHistoricalStage6RunnerCarrier(
                   initialContext, mode))
    <2>1. CASE mode = "NonCompletion"
      BY <2>1,
         IndexedHistoricalTransport(initialContext)!
           Stage4CapacityOrderingIsWellFounded
         DEF IndexedHistoricalStage6RunnerOrdering,
             IndexedHistoricalStage6RunnerCarrier
    <2>2. CASE mode # "NonCompletion"
      BY <2>2,
         IndexedHistoricalTransport(initialContext)!
           ReadyRunAuxOrderingIsWellFounded
         DEF IndexedHistoricalStage6RunnerOrdering,
             IndexedHistoricalStage6RunnerCarrier
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM IndexedHistoricalStage6RunnerBlockedHasPendingOwner ==
  \A initialContext \in AdmissibleContextRecords,
     mode \in IndexedHistoricalStage6RunnerModes,
     candidate, position, rank:
    IndexedHistoricalStage6RunnerBlocked(
      initialContext, mode, candidate, position, rank)
      => IndexedHistoricalTemporalCandidateRunnerPending(
           initialContext, candidate)
BY Isa
   DEF IndexedHistoricalStage6RunnerModes,
       IndexedHistoricalStage6RunnerBlocked,
       IndexedHistoricalTemporalCandidateRunnerPending,
       IndexedHistoricalTemporalSupportAt,
       IndexedHistoricalTransport!
         HistoricalTemporalStage6PreAdmissionBlockedAtAux,
       IndexedHistoricalTransport!
         HistoricalTemporalStage6OwedBlockedAtAux,
       IndexedHistoricalTransport!
         HistoricalTemporalStage6NonCompletionBlockedAtRank,
       IndexedHistoricalTransport!
         HistoricalTemporalStage6Pending,
       IndexedHistoricalTransport!
         HistoricalProtectedOwnedAtServiceRank

THEOREM IndexedHistoricalStage6RunnerProducesOutcome ==
  \A initialContext \in AdmissibleContextRecords,
     mode \in IndexedHistoricalStage6RunnerModes,
     candidate, position:
    \A rank \in
         IndexedHistoricalStage6RunnerCarrier(initialContext, mode):
      /\ IndexedHistoricalStage6RunnerBlocked(
           initialContext, mode, candidate, position, rank)
      /\ <<IndexedRunHistoricalRecoveryStep(
             initialContext, candidate.node)>>_IndexedChainVars
      => \/ IndexedHistoricalStage6RunnerProgress(
              initialContext, mode, candidate, position, rank)'
         \/ IndexedHistoricalStage6RunnerEpisodeResidual(
              initialContext, mode, candidate, position, rank)'
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW mode \in IndexedHistoricalStage6RunnerModes,
                NEW candidate, NEW position,
                NEW rank \in
                  IndexedHistoricalStage6RunnerCarrier(
                    initialContext, mode),
                IndexedHistoricalStage6RunnerBlocked(
                  initialContext, mode, candidate, position, rank),
                <<IndexedRunHistoricalRecoveryStep(
                    initialContext, candidate.node)>>_IndexedChainVars
         PROVE \/ IndexedHistoricalStage6RunnerProgress(
                    initialContext, mode, candidate, position, rank)'
               \/ IndexedHistoricalStage6RunnerEpisodeResidual(
                    initialContext, mode, candidate, position, rank)'
    <2>1. IndexedHistoricalTransport(initialContext)!
             PostGstRunHistoricalRecoveryNode(candidate.node)
      BY <1>1, Isa
         DEF IndexedRunHistoricalRecoveryStep,
             IndexedAsync!PostGstRunHistoricalRecoveryNode,
             IndexedHistoricalTransport!
               PostGstRunHistoricalRecoveryNode
    <2>2. CASE mode = "PreAdmission"
      BY <1>1, <2>1, <2>2,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage6PreAdmissionSameRunnerOutcome
         DEF IndexedHistoricalStage6RunnerBlocked,
             IndexedHistoricalStage6RunnerProgress,
             IndexedHistoricalStage6RunnerEpisodeResidual,
             IndexedHistoricalStage6RunnerCarrier
    <2>3. CASE mode = "Owed"
      BY <1>1, <2>1, <2>3,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage6OwedSameRunnerOutcome
         DEF IndexedHistoricalStage6RunnerBlocked,
             IndexedHistoricalStage6RunnerProgress,
             IndexedHistoricalStage6RunnerEpisodeResidual,
             IndexedHistoricalStage6RunnerCarrier
    <2>4. CASE mode = "NonCompletion"
      BY <1>1, <2>1, <2>4,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage6NonCompletionSameRunnerOutcome
         DEF IndexedHistoricalStage6RunnerBlocked,
             IndexedHistoricalStage6RunnerProgress,
             IndexedHistoricalStage6RunnerEpisodeResidual,
             IndexedHistoricalStage6RunnerCarrier
    <2> QED BY <1>1, <2>2, <2>3, <2>4
         DEF IndexedHistoricalStage6RunnerModes
  <1> QED BY <1>1

THEOREM IndexedHistoricalStage6RunnerUnlessProgress ==
  \A initialContext \in AdmissibleContextRecords,
     mode \in IndexedHistoricalStage6RunnerModes,
     candidate, position:
    \A rank \in
         IndexedHistoricalStage6RunnerCarrier(initialContext, mode):
      /\ IndexedHistoricalStage6RunnerBlocked(
           initialContext, mode, candidate, position, rank)
      /\ [IndexedChainNext]_IndexedChainVars
      => \/ IndexedHistoricalStage6RunnerBlocked(
              initialContext, mode, candidate, position, rank)'
         \/ IndexedHistoricalStage6RunnerProgress(
              initialContext, mode, candidate, position, rank)'
         \/ IndexedHistoricalStage6RunnerEpisodeResidual(
              initialContext, mode, candidate, position, rank)'
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW mode \in IndexedHistoricalStage6RunnerModes,
                NEW candidate, NEW position,
                NEW rank \in
                  IndexedHistoricalStage6RunnerCarrier(
                    initialContext, mode),
                IndexedHistoricalStage6RunnerBlocked(
                  initialContext, mode, candidate, position, rank),
                [IndexedChainNext]_IndexedChainVars
         PROVE
           \/ IndexedHistoricalStage6RunnerBlocked(
                initialContext, mode, candidate, position, rank)'
           \/ IndexedHistoricalStage6RunnerProgress(
                initialContext, mode, candidate, position, rank)'
           \/ IndexedHistoricalStage6RunnerEpisodeResidual(
                initialContext, mode, candidate, position, rank)'
    <2>1. [IndexedHistoricalTransport(initialContext)!AsyncNext]_(
             IndexedHistoricalTransport(initialContext)!AsyncAllVars)
      BY <1>1,
         IndexedBracketStepProjectsEveryHistoricalTransportStep
    <2>2. CASE IndexedHistoricalTransport(initialContext)!
                  PostGstRunHistoricalRecoveryNode(candidate.node)
      BY <1>1, <2>2,
         IndexedHistoricalStage6RunnerProducesOutcome, Isa
         DEF IndexedRunHistoricalRecoveryStep,
             IndexedHistoricalStage6RunnerEpisodeResidual
    <2>3. CASE ~IndexedHistoricalTransport(initialContext)!
                   PostGstRunHistoricalRecoveryNode(candidate.node)
      <3>1. CASE mode = "PreAdmission"
        BY <1>1, <2>1, <2>3, <3>1,
           IndexedHistoricalTransport(initialContext)!
             HistoricalTemporalStage6PreAdmissionOtherStep
           DEF IndexedHistoricalStage6RunnerBlocked,
               IndexedHistoricalStage6RunnerProgress,
               IndexedHistoricalStage6RunnerEpisodeResidual,
               IndexedHistoricalStage6RunnerCarrier
      <3>2. CASE mode = "Owed"
        BY <1>1, <2>1, <2>3, <3>2,
           IndexedHistoricalTransport(initialContext)!
             HistoricalTemporalStage6OwedOtherStep
           DEF IndexedHistoricalStage6RunnerBlocked,
               IndexedHistoricalStage6RunnerProgress,
               IndexedHistoricalStage6RunnerEpisodeResidual,
               IndexedHistoricalStage6RunnerCarrier
      <3>3. CASE mode = "NonCompletion"
        BY <1>1, <2>1, <2>3, <3>3,
           IndexedHistoricalTransport(initialContext)!
             HistoricalTemporalStage6NonCompletionOtherStep
           DEF IndexedHistoricalStage6RunnerBlocked,
               IndexedHistoricalStage6RunnerProgress,
               IndexedHistoricalStage6RunnerEpisodeResidual,
               IndexedHistoricalStage6RunnerCarrier
      <3> QED BY <1>1, <3>1, <3>2, <3>3
           DEF IndexedHistoricalStage6RunnerModes
    <2> QED BY <2>2, <2>3
  <1> QED BY <1>1

THEOREM IndexedHistoricalStage6FairRunnerOneStep ==
  /\ IndexedChainSpec
  /\ IndexedHistoricalStage6FiniteRunnerEpisodeResidualProperty
    => \A initialContext \in AdmissibleContextRecords,
          mode \in IndexedHistoricalStage6RunnerModes,
          candidate, position:
         \A rank \in
              IndexedHistoricalStage6RunnerCarrier(
                initialContext, mode):
           IndexedHistoricalStage6RunnerBlocked(
             initialContext, mode, candidate, position, rank)
             ~> IndexedHistoricalStage6RunnerProgress(
                  initialContext, mode, candidate, position, rank)
PROOF
  <1>1. ASSUME IndexedChainSpec,
                IndexedHistoricalStage6FiniteRunnerEpisodeResidualProperty,
                NEW initialContext \in AdmissibleContextRecords,
                NEW mode \in IndexedHistoricalStage6RunnerModes,
                NEW candidate, NEW position,
                NEW rank \in
                  IndexedHistoricalStage6RunnerCarrier(
                    initialContext, mode)
         PROVE IndexedHistoricalStage6RunnerBlocked(
                 initialContext, mode, candidate, position, rank)
                 ~> IndexedHistoricalStage6RunnerProgress(
                      initialContext, mode, candidate, position, rank)
    <2>1. []IndexedCompositionInvariant
      BY <1>1, IndexedChainSpecEstablishesCompositionInvariant
    <2>2. [][IndexedChainNext]_IndexedChainVars
      BY <1>1 DEF IndexedChainSpec
    <2>3. IndexedHistoricalStage6RunnerBlocked(
             initialContext, mode, candidate, position, rank)
             /\ [IndexedChainNext]_IndexedChainVars
            => \/ IndexedHistoricalStage6RunnerBlocked(
                    initialContext, mode, candidate, position, rank)'
               \/ IndexedHistoricalStage6RunnerProgress(
                    initialContext, mode, candidate, position, rank)'
               \/ IndexedHistoricalStage6RunnerEpisodeResidual(
                    initialContext, mode, candidate, position, rank)'
      BY <1>1, IndexedHistoricalStage6RunnerUnlessProgress
    <2>4. IndexedCompositionInvariant
             /\ IndexedHistoricalStage6RunnerBlocked(
                  initialContext, mode, candidate, position, rank)
            => ENABLED
                 <<IndexedRunHistoricalRecoveryStep(
                     initialContext, candidate.node)>>_IndexedChainVars
      BY <1>1,
         IndexedHistoricalStage6RunnerBlockedHasPendingOwner,
         IndexedHistoricalCandidateRunnerEnablesFairOccurrence
    <2>5. /\ IndexedHistoricalStage6RunnerBlocked(
                 initialContext, mode, candidate, position, rank)
             /\ <<IndexedRunHistoricalRecoveryStep(
                    initialContext, candidate.node)>>_IndexedChainVars
            => \/ IndexedHistoricalStage6RunnerProgress(
                    initialContext, mode, candidate, position, rank)'
               \/ IndexedHistoricalStage6RunnerEpisodeResidual(
                    initialContext, mode, candidate, position, rank)'
      BY <1>1,
         IndexedHistoricalStage6RunnerProducesOutcome
    <2>6. CASE candidate.node \in Responsive
      <3>1. WF_IndexedChainVars(
               IndexedRunHistoricalRecoveryStep(
                 initialContext, candidate.node))
        BY <1>1, <2>6 DEF IndexedChainSpec, IndexedFairness
      <3>2. IndexedHistoricalStage6RunnerBlocked(
               initialContext, mode, candidate, position, rank)
               ~> (IndexedHistoricalStage6RunnerProgress(
                     initialContext, mode, candidate, position, rank)
                    \/ IndexedHistoricalStage6RunnerEpisodeResidual(
                         initialContext, mode, candidate, position, rank))
        BY <2>1, <2>2, <2>3, <2>4, <2>5, <3>1, PTL
      <3>3. IndexedHistoricalStage6RunnerEpisodeResidual(
               initialContext, mode, candidate, position, rank)
               ~> IndexedHistoricalStage6RunnerProgress(
                    initialContext, mode, candidate, position, rank)
        BY <1>1
           DEF IndexedHistoricalStage6FiniteRunnerEpisodeResidualProperty
      <3> QED BY <3>2, <3>3, PTL
    <2>7. CASE candidate.node \notin Responsive
      <3>1. []~IndexedHistoricalStage6RunnerBlocked(
                    initialContext, mode, candidate, position, rank)
        BY <2>7, PTL
           DEF IndexedHistoricalStage6RunnerBlocked,
               IndexedHistoricalStage6RunnerModes,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage6PreAdmissionBlockedAtAux,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage6OwedBlockedAtAux,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage6NonCompletionBlockedAtRank,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage6Pending,
               IndexedHistoricalTransport!
                 HistoricalProtectedOwnedAtServiceRank,
               IndexedHistoricalTransport!
                 HistoricalProtectedCandidateOwned
      <3> QED BY <3>1, PTL
    <2> QED BY <2>6, <2>7
  <1> QED BY <1>1

THEOREM IndexedHistoricalStage6RunnerModeDescent ==
  /\ IndexedChainSpec
  /\ IndexedHistoricalStage6FiniteRunnerEpisodeResidualProperty
    => \A initialContext \in AdmissibleContextRecords,
          mode \in IndexedHistoricalStage6RunnerModes,
          candidate, position:
         \A rank \in
              IndexedHistoricalStage6RunnerCarrier(
                initialContext, mode):
           IndexedHistoricalStage6RunnerBlocked(
             initialContext, mode, candidate, position, rank)
             ~> IndexedHistoricalStage6RunnerGoal(
                  initialContext, mode, candidate, position)
PROOF
  <1>1. ASSUME IndexedChainSpec,
                IndexedHistoricalStage6FiniteRunnerEpisodeResidualProperty,
                NEW initialContext \in AdmissibleContextRecords,
                NEW mode \in IndexedHistoricalStage6RunnerModes,
                NEW candidate, NEW position
         PROVE \A rank \in
                    IndexedHistoricalStage6RunnerCarrier(
                      initialContext, mode):
                  IndexedHistoricalStage6RunnerBlocked(
                    initialContext, mode, candidate, position, rank)
                    ~> IndexedHistoricalStage6RunnerGoal(
                         initialContext, mode, candidate, position)
    <2>1. \A rank \in
               IndexedHistoricalStage6RunnerCarrier(
                 initialContext, mode):
             IndexedHistoricalStage6RunnerBlocked(
               initialContext, mode, candidate, position, rank)
               ~> IndexedHistoricalStage6RunnerProgress(
                    initialContext, mode, candidate, position, rank)
      BY <1>1, IndexedHistoricalStage6FairRunnerOneStep
    <2> QED BY <2>1,
         IndexedHistoricalStage6RunnerOrderingIsWellFounded,
         WellFoundedLeadsTo, Isa
         DEF IndexedHistoricalStage6RunnerProgress,
             IndexedHistoricalStage6RunnerGoal,
             IndexedHistoricalStage6RunnerModes,
             IndexedHistoricalStage6RunnerCarrier,
             IndexedHistoricalStage6RunnerOrdering,
             IndexedHistoricalTransport!
               HistoricalTemporalStage6PreAdmissionAuxProgress,
             IndexedHistoricalTransport!
               HistoricalTemporalStage6OwedAuxProgress,
             IndexedHistoricalTransport!
               HistoricalTemporalStage6NonCompletionProgress,
             IndexedHistoricalTransport!
               HistoricalTemporalStage6PreAdmissionGoal,
             IndexedHistoricalTransport!
               HistoricalTemporalStage6NonCompletionGoal,
             IndexedHistoricalTemporalRankProgressExit
  <1> QED BY <1>1

(***************************************************************************
Each runner mode starts at the exact current one-height rank.  These three
entry lemmas keep that type argument explicit and then reuse the common
product descent above.
***************************************************************************)

THEOREM IndexedChainSpecClosesHistoricalStage6PreAdmission ==
  /\ IndexedChainSpec
  /\ IndexedHistoricalStage6FiniteRunnerEpisodeResidualProperty
    => \A initialContext \in AdmissibleContextRecords,
          candidate, position:
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage6Pending(candidate, position)
           ~> IndexedHistoricalTransport(initialContext)!
                 HistoricalTemporalStage6PreAdmissionGoal(
                   candidate, position)
PROOF
  <1>1. ASSUME IndexedChainSpec,
                IndexedHistoricalStage6FiniteRunnerEpisodeResidualProperty,
                NEW initialContext \in AdmissibleContextRecords,
                NEW candidate, NEW position
         PROVE IndexedHistoricalTransport(initialContext)!
                 HistoricalTemporalStage6Pending(candidate, position)
                 ~>
               IndexedHistoricalTransport(initialContext)!
                 HistoricalTemporalStage6PreAdmissionGoal(
                   candidate, position)
    <2>1. []IndexedHistoricalTemporalSupportAt(initialContext)
      BY <1>1, IndexedChainSpecAlwaysHistoricalTemporalSupport, PTL
    <2>2. IndexedHistoricalTransport(initialContext)!
             HistoricalTemporalStage6Pending(candidate, position)
             ~> (IndexedHistoricalTransport(initialContext)!
                   HistoricalTemporalStage6PreAdmissionGoal(
                     candidate, position)
                  \/ IndexedHistoricalStage6RunnerBlocked(
                       initialContext, "PreAdmission",
                       candidate, position,
                       IndexedHistoricalTransport(initialContext)!
                         ReadyRunAuxRank(candidate.node)))
      BY <2>1,
         IndexedHistoricalTransport(initialContext)!
           AsyncStrongTypeProjectsAsyncType,
         IndexedHistoricalTransport(initialContext)!
           ReadyRunAuxRankInCarrier, PTL
         DEF IndexedHistoricalTemporalSupportAt,
             IndexedHistoricalStage6RunnerBlocked,
             IndexedHistoricalTransport!
               HistoricalTemporalStage6PreAdmissionBlockedAtAux
    <2>3. \A rank \in
               IndexedHistoricalStage6RunnerCarrier(
                 initialContext, "PreAdmission"):
             IndexedHistoricalStage6RunnerBlocked(
               initialContext, "PreAdmission",
               candidate, position, rank)
               ~> IndexedHistoricalTransport(initialContext)!
                     HistoricalTemporalStage6PreAdmissionGoal(
                       candidate, position)
      BY <1>1, IndexedHistoricalStage6RunnerModeDescent, Isa
         DEF IndexedHistoricalStage6RunnerModes,
             IndexedHistoricalStage6RunnerGoal
    <2>4. IndexedHistoricalStage6RunnerBlocked(
             initialContext, "PreAdmission", candidate, position,
             IndexedHistoricalTransport(initialContext)!
               ReadyRunAuxRank(candidate.node))
             ~> IndexedHistoricalTransport(initialContext)!
                   HistoricalTemporalStage6PreAdmissionGoal(
                     candidate, position)
      BY <2>1, <2>3,
         IndexedHistoricalTransport(initialContext)!
           AsyncStrongTypeProjectsAsyncType,
         IndexedHistoricalTransport(initialContext)!
           ReadyRunAuxRankInCarrier
         DEF IndexedHistoricalTemporalSupportAt,
             IndexedHistoricalStage6RunnerCarrier
    <2> QED BY <2>2, <2>4, PTL
  <1> QED BY <1>1

THEOREM IndexedChainSpecClosesHistoricalStage6Owed ==
  /\ IndexedChainSpec
  /\ IndexedHistoricalStage6FiniteRunnerEpisodeResidualProperty
    => \A initialContext \in AdmissibleContextRecords,
          candidate, position:
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage6OwedCausalReady(candidate, position)
           ~> IndexedHistoricalTemporalRankProgressExit(
                 initialContext, candidate, <<6, position>>)
PROOF
  <1>1. ASSUME IndexedChainSpec,
                IndexedHistoricalStage6FiniteRunnerEpisodeResidualProperty,
                NEW initialContext \in AdmissibleContextRecords,
                NEW candidate, NEW position
         PROVE IndexedHistoricalTransport(initialContext)!
                 HistoricalTemporalStage6OwedCausalReady(
                   candidate, position)
                 ~>
               IndexedHistoricalTemporalRankProgressExit(
                 initialContext, candidate, <<6, position>>)
    <2>1. []IndexedHistoricalTemporalSupportAt(initialContext)
      BY <1>1, IndexedChainSpecAlwaysHistoricalTemporalSupport, PTL
    <2>2. IndexedHistoricalTransport(initialContext)!
             HistoricalTemporalStage6OwedCausalReady(
               candidate, position)
             ~> IndexedHistoricalStage6RunnerBlocked(
                   initialContext, "Owed", candidate, position,
                   IndexedHistoricalTransport(initialContext)!
                     ReadyRunAuxRank(candidate.node))
      BY <2>1,
         IndexedHistoricalTransport(initialContext)!
           AsyncStrongTypeProjectsAsyncType,
         IndexedHistoricalTransport(initialContext)!
           ReadyRunAuxRankInCarrier, PTL
         DEF IndexedHistoricalTemporalSupportAt,
             IndexedHistoricalStage6RunnerBlocked,
             IndexedHistoricalTransport!
               HistoricalTemporalStage6OwedBlockedAtAux
    <2>3. \A rank \in
               IndexedHistoricalStage6RunnerCarrier(
                 initialContext, "Owed"):
             IndexedHistoricalStage6RunnerBlocked(
               initialContext, "Owed", candidate, position, rank)
               ~> IndexedHistoricalTemporalRankProgressExit(
                     initialContext, candidate, <<6, position>>)
      BY <1>1, IndexedHistoricalStage6RunnerModeDescent, Isa
         DEF IndexedHistoricalStage6RunnerModes,
             IndexedHistoricalStage6RunnerGoal
    <2>4. IndexedHistoricalStage6RunnerBlocked(
             initialContext, "Owed", candidate, position,
             IndexedHistoricalTransport(initialContext)!
               ReadyRunAuxRank(candidate.node))
             ~> IndexedHistoricalTemporalRankProgressExit(
                   initialContext, candidate, <<6, position>>)
      BY <2>1, <2>3,
         IndexedHistoricalTransport(initialContext)!
           AsyncStrongTypeProjectsAsyncType,
         IndexedHistoricalTransport(initialContext)!
           ReadyRunAuxRankInCarrier
         DEF IndexedHistoricalTemporalSupportAt,
             IndexedHistoricalStage6RunnerCarrier
    <2> QED BY <2>2, <2>4, PTL
  <1> QED BY <1>1

THEOREM IndexedChainSpecClosesHistoricalStage6NonCompletion ==
  /\ IndexedChainSpec
  /\ IndexedHistoricalStage6FiniteRunnerEpisodeResidualProperty
    => \A initialContext \in AdmissibleContextRecords,
          candidate, position:
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage6NonCompletionCapacityBlocked(
             candidate, position)
           ~> IndexedHistoricalTransport(initialContext)!
                 HistoricalTemporalStage6NonCompletionGoal(
                   candidate, position)
PROOF
  <1>1. ASSUME IndexedChainSpec,
                IndexedHistoricalStage6FiniteRunnerEpisodeResidualProperty,
                NEW initialContext \in AdmissibleContextRecords,
                NEW candidate, NEW position
         PROVE IndexedHistoricalTransport(initialContext)!
                 HistoricalTemporalStage6NonCompletionCapacityBlocked(
                   candidate, position)
                 ~>
               IndexedHistoricalTransport(initialContext)!
                 HistoricalTemporalStage6NonCompletionGoal(
                   candidate, position)
    <2>1. []IndexedHistoricalTemporalSupportAt(initialContext)
      BY <1>1, IndexedChainSpecAlwaysHistoricalTemporalSupport, PTL
    <2>2. IndexedHistoricalTransport(initialContext)!
             HistoricalTemporalStage6NonCompletionCapacityBlocked(
               candidate, position)
             ~> IndexedHistoricalStage6RunnerBlocked(
                   initialContext, "NonCompletion",
                   candidate, position,
                   IndexedHistoricalTransport(initialContext)!
                     Stage4CapacityRank(candidate.node))
      BY <2>1,
         IndexedHistoricalTransport(initialContext)!
           AsyncStrongTypeProjectsAsyncType,
         IndexedHistoricalTransport(initialContext)!
           Stage4CapacityRankInCarrier, PTL
         DEF IndexedHistoricalTemporalSupportAt,
             IndexedHistoricalStage6RunnerBlocked,
             IndexedHistoricalTransport!
               HistoricalTemporalStage6NonCompletionBlockedAtRank
    <2>3. \A rank \in
               IndexedHistoricalStage6RunnerCarrier(
                 initialContext, "NonCompletion"):
             IndexedHistoricalStage6RunnerBlocked(
               initialContext, "NonCompletion",
               candidate, position, rank)
               ~> IndexedHistoricalTransport(initialContext)!
                     HistoricalTemporalStage6NonCompletionGoal(
                       candidate, position)
      BY <1>1, IndexedHistoricalStage6RunnerModeDescent, Isa
         DEF IndexedHistoricalStage6RunnerModes,
             IndexedHistoricalStage6RunnerGoal
    <2>4. IndexedHistoricalStage6RunnerBlocked(
             initialContext, "NonCompletion", candidate, position,
             IndexedHistoricalTransport(initialContext)!
               Stage4CapacityRank(candidate.node))
             ~> IndexedHistoricalTransport(initialContext)!
                   HistoricalTemporalStage6NonCompletionGoal(
                     candidate, position)
      BY <2>1, <2>3,
         IndexedHistoricalTransport(initialContext)!
           AsyncStrongTypeProjectsAsyncType,
         IndexedHistoricalTransport(initialContext)!
           Stage4CapacityRankInCarrier
         DEF IndexedHistoricalTemporalSupportAt,
             IndexedHistoricalStage6RunnerCarrier
    <2> QED BY <2>2, <2>4, PTL
  <1> QED BY <1>1

(***************************************************************************
Indexed Stage-6 Completion physical-I/O drain.

The exact historical I/O action is already weakly fair in the product.  A
positive queue depth makes that action nonstuttering, and the one-height
action lemma decreases the natural depth or opens the Completion goal.
***************************************************************************)

IndexedHistoricalStage6CompletionIoBlockedAtDepth(
    initialContext, candidate, position, depth) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalTemporalStage6CompletionIoBlockedAtDepth(
      candidate, position, depth)

IndexedHistoricalStage6CompletionIoDrainGoal(
    initialContext, candidate, position) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalTemporalStage6CompletionIoDrainGoal(
      candidate, position)

IndexedHistoricalStage6CompletionIoProgress(
    initialContext, candidate, position, depth) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalTemporalStage6CompletionIoProgress(
      candidate, position, depth)

THEOREM IndexedHistoricalStage6CompletionIoBlockedHasFairOwner ==
  \A initialContext \in AdmissibleContextRecords,
     candidate, position, depth:
    /\ IndexedCompositionInvariant
    /\ IndexedHistoricalStage6CompletionIoBlockedAtDepth(
         initialContext, candidate, position, depth)
    => /\ candidate.node \in Responsive
       /\ candidate.node \in joinedByContext[initialContext]
       /\ initialContext \in JoinedContexts
       /\ IndexedHistoricalTransport(initialContext)!
            HistoricalRecoveryTarget(candidate.node)
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW candidate, NEW position, NEW depth,
                IndexedCompositionInvariant,
                IndexedHistoricalStage6CompletionIoBlockedAtDepth(
                  initialContext, candidate, position, depth)
         PROVE /\ candidate.node \in Responsive
               /\ candidate.node \in joinedByContext[initialContext]
               /\ initialContext \in JoinedContexts
               /\ IndexedHistoricalTransport(initialContext)!
                    HistoricalRecoveryTarget(candidate.node)
    <2>1. IndexedHistoricalTemporalCandidateRunnerPending(
             initialContext, candidate)
      BY <1>1
         DEF IndexedHistoricalStage6CompletionIoBlockedAtDepth,
             IndexedHistoricalTemporalCandidateRunnerPending,
             IndexedHistoricalTemporalSupportAt,
             IndexedHistoricalTransport!
               HistoricalTemporalStage6CompletionIoBlockedAtDepth,
             IndexedHistoricalTransport!
               HistoricalTemporalStage6CompletionCapacityBlocked,
             IndexedHistoricalTransport!
               HistoricalTemporalStage6Pending,
             IndexedHistoricalTransport!
               HistoricalProtectedOwnedAtServiceRank
    <2> QED BY <1>1, <2>1,
         IndexedHistoricalCandidateRunnerHasJoinedFairOwner
  <1> QED BY <1>1

THEOREM IndexedHistoricalStage6CompletionIoBlockedEnablesExactWorker ==
  \A initialContext \in AdmissibleContextRecords,
     candidate, position, depth:
    /\ IndexedCompositionInvariant
    /\ IndexedHistoricalStage6CompletionIoBlockedAtDepth(
         initialContext, candidate, position, depth)
    => ENABLED
         IndexedHistoricalRecoveryIoWorkerStep(
           initialContext, candidate.node)
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW candidate, NEW position, NEW depth,
                IndexedCompositionInvariant,
                IndexedHistoricalStage6CompletionIoBlockedAtDepth(
                  initialContext, candidate, position, depth)
         PROVE ENABLED
                 IndexedHistoricalRecoveryIoWorkerStep(
                   initialContext, candidate.node)
    <2>1. /\ candidate.node \in Responsive
           /\ candidate.node \in joinedByContext[initialContext]
           /\ initialContext \in JoinedContexts
      BY <1>1,
         IndexedHistoricalStage6CompletionIoBlockedHasFairOwner
    <2>2. /\ IndexedHistoricalTransport(initialContext)!
                HistoricalRecoveryTarget(candidate.node)
           /\ IndexedHistoricalTransport(initialContext)!
                AsyncStrongTypeInvariant
           /\ IndexedHistoricalTransport(initialContext)!gst
           /\ IndexedHistoricalTransport(initialContext)!
                AsyncIoQueueDepth(candidate.node) > 0
      BY <1>1,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage6CompletionIoBlockedCoreFacts
         DEF IndexedHistoricalStage6CompletionIoBlockedAtDepth
    <2>3. ENABLED
             IndexedHistoricalTransport(initialContext)!
               PostGstServiceHistoricalRecoveryIoWorker(candidate.node)
      BY <2>2,
         IndexedHistoricalTransport(initialContext)!
           HistoricalRecoveryIoWorkerEnabledAfterGst
    <2>4. ENABLED IndexedAsync(initialContext)!
             PostGstServiceHistoricalRecoveryIoWorker(candidate.node)
      BY <2>3, Isa
         DEF IndexedHistoricalTransport!
               PostGstServiceHistoricalRecoveryIoWorker,
             IndexedAsync!
               PostGstServiceHistoricalRecoveryIoWorker,
             IndexedHistoricalTransport!
               ServiceHistoricalRecoveryIoWorker,
             IndexedAsync!ServiceHistoricalRecoveryIoWorker,
             IndexedHistoricalTransport!ServiceIoWorkerWork,
             IndexedAsync!ServiceIoWorkerWork
    <2> QED BY <1>1, <2>1, <2>4,
         IndexedFairActionsRemainEnabledInProduct
  <1> QED BY <1>1

THEOREM IndexedHistoricalStage6CompletionIoWorkerIsNonstuttering ==
  \A initialContext \in AdmissibleContextRecords,
     candidate, position, depth:
    /\ IndexedHistoricalStage6CompletionIoBlockedAtDepth(
         initialContext, candidate, position, depth)
    /\ IndexedHistoricalRecoveryIoWorkerStep(
         initialContext, candidate.node)
    => <<IndexedHistoricalRecoveryIoWorkerStep(
           initialContext, candidate.node)>>_IndexedChainVars
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW candidate, NEW position, NEW depth,
                IndexedHistoricalStage6CompletionIoBlockedAtDepth(
                  initialContext, candidate, position, depth),
                IndexedHistoricalRecoveryIoWorkerStep(
                  initialContext, candidate.node)
         PROVE <<IndexedHistoricalRecoveryIoWorkerStep(
                   initialContext, candidate.node)>>_IndexedChainVars
    <2>1. /\ IndexedHistoricalTransport(initialContext)!
                HistoricalRecoveryTarget(candidate.node)
           /\ IndexedHistoricalTransport(initialContext)!
                AsyncTypeInvariant
           /\ IndexedHistoricalTransport(initialContext)!
                AsyncIoQueueDepth(candidate.node) > 0
      BY <1>1,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage6CompletionIoBlockedCoreFacts
         DEF IndexedHistoricalStage6CompletionIoBlockedAtDepth
    <2>2. <<IndexedHistoricalTransport(initialContext)!
               PostGstServiceHistoricalRecoveryIoWorker(
                 candidate.node)>>_(
             IndexedHistoricalTransport(initialContext)!AsyncAllVars)
      BY <1>1, <2>1,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalQueuedIoServiceIsNonstuttering, Isa
         DEF IndexedHistoricalRecoveryIoWorkerStep,
             IndexedHistoricalTransport!
               PostGstServiceHistoricalRecoveryIoWorker,
             IndexedAsync!
               PostGstServiceHistoricalRecoveryIoWorker
    <2>3. IndexedAsyncStateShape
      BY <1>1
         DEF IndexedHistoricalRecoveryIoWorkerStep, IndexedChainNext
    <2>4. IndexedHistoricalTransport(initialContext)!AsyncAllVars =
             IndexedAsyncStateAt(initialContext)
      BY <1>1, <2>3,
         IndexedHistoricalTransportVariablesAreExact
    <2>5. IndexedChainVars' # IndexedChainVars
      BY <2>2, <2>4, Isa
         DEF IndexedChainVars, IndexedAsyncStateAt
    <2> QED BY <1>1, <2>5
  <1> QED BY <1>1

THEOREM IndexedHistoricalStage6CompletionIoBlockedEnablesFairOccurrence ==
  \A initialContext \in AdmissibleContextRecords,
     candidate, position, depth:
    /\ IndexedCompositionInvariant
    /\ IndexedHistoricalStage6CompletionIoBlockedAtDepth(
         initialContext, candidate, position, depth)
    => ENABLED
         <<IndexedHistoricalRecoveryIoWorkerStep(
             initialContext, candidate.node)>>_IndexedChainVars
BY IndexedHistoricalStage6CompletionIoBlockedEnablesExactWorker,
   IndexedHistoricalStage6CompletionIoWorkerIsNonstuttering,
   ENABLEDaxioms

THEOREM IndexedHistoricalStage6CompletionIoFairOccurrenceProgresses ==
  \A initialContext \in AdmissibleContextRecords,
     candidate, position:
    \A depth \in Nat:
      /\ IndexedHistoricalStage6CompletionIoBlockedAtDepth(
           initialContext, candidate, position, depth)
      /\ <<IndexedHistoricalRecoveryIoWorkerStep(
             initialContext, candidate.node)>>_IndexedChainVars
      => IndexedHistoricalStage6CompletionIoProgress(
           initialContext, candidate, position, depth)'
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW candidate, NEW position, NEW depth \in Nat,
                IndexedHistoricalStage6CompletionIoBlockedAtDepth(
                  initialContext, candidate, position, depth),
                <<IndexedHistoricalRecoveryIoWorkerStep(
                    initialContext, candidate.node)>>_IndexedChainVars
         PROVE IndexedHistoricalStage6CompletionIoProgress(
                 initialContext, candidate, position, depth)'
    <2>1. [IndexedHistoricalTransport(initialContext)!AsyncNext]_(
             IndexedHistoricalTransport(initialContext)!AsyncAllVars)
      BY <1>1,
         IndexedBracketStepProjectsEveryHistoricalTransportStep
    <2>2. IndexedHistoricalTransport(initialContext)!
             PostGstServiceHistoricalRecoveryIoWorker(candidate.node)
      BY <1>1, Isa
         DEF IndexedHistoricalRecoveryIoWorkerStep,
             IndexedAsync!
               PostGstServiceHistoricalRecoveryIoWorker,
             IndexedHistoricalTransport!
               PostGstServiceHistoricalRecoveryIoWorker
    <2> QED BY <1>1, <2>1, <2>2,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage6CompletionIoWorkerStrictlyProgresses
         DEF IndexedHistoricalStage6CompletionIoBlockedAtDepth,
             IndexedHistoricalStage6CompletionIoProgress
  <1> QED BY <1>1

THEOREM IndexedHistoricalStage6CompletionIoUnlessProgress ==
  \A initialContext \in AdmissibleContextRecords,
     candidate, position:
    \A depth \in Nat:
      /\ IndexedHistoricalStage6CompletionIoBlockedAtDepth(
           initialContext, candidate, position, depth)
      /\ [IndexedChainNext]_IndexedChainVars
      => \/ IndexedHistoricalStage6CompletionIoBlockedAtDepth(
              initialContext, candidate, position, depth)'
         \/ IndexedHistoricalStage6CompletionIoProgress(
              initialContext, candidate, position, depth)'
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW candidate, NEW position, NEW depth \in Nat,
                IndexedHistoricalStage6CompletionIoBlockedAtDepth(
                  initialContext, candidate, position, depth),
                [IndexedChainNext]_IndexedChainVars
         PROVE
           \/ IndexedHistoricalStage6CompletionIoBlockedAtDepth(
                initialContext, candidate, position, depth)'
           \/ IndexedHistoricalStage6CompletionIoProgress(
                initialContext, candidate, position, depth)'
    <2>1. [IndexedHistoricalTransport(initialContext)!AsyncNext]_(
             IndexedHistoricalTransport(initialContext)!AsyncAllVars)
      BY <1>1,
         IndexedBracketStepProjectsEveryHistoricalTransportStep
    <2>2. CASE IndexedHistoricalTransport(initialContext)!
                  PostGstServiceHistoricalRecoveryIoWorker(candidate.node)
      BY <1>1, <2>1, <2>2,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage6CompletionIoWorkerStrictlyProgresses
         DEF IndexedHistoricalStage6CompletionIoBlockedAtDepth,
             IndexedHistoricalStage6CompletionIoProgress
    <2>3. CASE ~IndexedHistoricalTransport(initialContext)!
                   PostGstServiceHistoricalRecoveryIoWorker(
                     candidate.node)
      BY <1>1, <2>1, <2>3,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage6CompletionIoOtherStep
         DEF IndexedHistoricalStage6CompletionIoBlockedAtDepth,
             IndexedHistoricalStage6CompletionIoProgress
    <2> QED BY <2>2, <2>3
  <1> QED BY <1>1

THEOREM IndexedChainSpecHistoricalStage6CompletionIoOneStep ==
  IndexedChainSpec
    => \A initialContext \in AdmissibleContextRecords,
          candidate, position:
         \A depth \in Nat:
           IndexedHistoricalStage6CompletionIoBlockedAtDepth(
             initialContext, candidate, position, depth)
             ~> IndexedHistoricalStage6CompletionIoProgress(
                  initialContext, candidate, position, depth)
PROOF
  <1>1. ASSUME IndexedChainSpec,
                NEW initialContext \in AdmissibleContextRecords,
                NEW candidate, NEW position, NEW depth \in Nat
         PROVE IndexedHistoricalStage6CompletionIoBlockedAtDepth(
                 initialContext, candidate, position, depth)
                 ~>
               IndexedHistoricalStage6CompletionIoProgress(
                 initialContext, candidate, position, depth)
    <2>1. []IndexedCompositionInvariant
      BY <1>1, IndexedChainSpecEstablishesCompositionInvariant
    <2>2. [][IndexedChainNext]_IndexedChainVars
      BY <1>1 DEF IndexedChainSpec
    <2>3. IndexedHistoricalStage6CompletionIoBlockedAtDepth(
             initialContext, candidate, position, depth)
             /\ [IndexedChainNext]_IndexedChainVars
            => \/ IndexedHistoricalStage6CompletionIoBlockedAtDepth(
                    initialContext, candidate, position, depth)'
               \/ IndexedHistoricalStage6CompletionIoProgress(
                    initialContext, candidate, position, depth)'
      BY <1>1,
         IndexedHistoricalStage6CompletionIoUnlessProgress
    <2>4. IndexedCompositionInvariant
             /\ IndexedHistoricalStage6CompletionIoBlockedAtDepth(
                  initialContext, candidate, position, depth)
            => ENABLED
                 <<IndexedHistoricalRecoveryIoWorkerStep(
                     initialContext, candidate.node)>>_IndexedChainVars
      BY <1>1,
         IndexedHistoricalStage6CompletionIoBlockedEnablesFairOccurrence
    <2>5. /\ IndexedHistoricalStage6CompletionIoBlockedAtDepth(
                 initialContext, candidate, position, depth)
             /\ <<IndexedHistoricalRecoveryIoWorkerStep(
                    initialContext, candidate.node)>>_IndexedChainVars
            => IndexedHistoricalStage6CompletionIoProgress(
                 initialContext, candidate, position, depth)'
      BY <1>1,
         IndexedHistoricalStage6CompletionIoFairOccurrenceProgresses
    <2>6. CASE candidate.node \in Responsive
      <3>1. WF_IndexedChainVars(
               IndexedHistoricalRecoveryIoWorkerStep(
                 initialContext, candidate.node))
        BY <1>1, <2>6 DEF IndexedChainSpec, IndexedFairness
      <3> QED BY <2>1, <2>2, <2>3, <2>4, <2>5, <3>1, PTL
    <2>7. CASE candidate.node \notin Responsive
      <3>1. []~IndexedHistoricalStage6CompletionIoBlockedAtDepth(
                    initialContext, candidate, position, depth)
        BY <2>7, PTL
           DEF IndexedHistoricalStage6CompletionIoBlockedAtDepth,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage6CompletionIoBlockedAtDepth,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage6CompletionCapacityBlocked,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage6Pending,
               IndexedHistoricalTransport!
                 HistoricalProtectedOwnedAtServiceRank,
               IndexedHistoricalTransport!
                 HistoricalProtectedCandidateOwned
      <3> QED BY <3>1, PTL
    <2> QED BY <2>6, <2>7
  <1> QED BY <1>1

THEOREM IndexedChainSpecDrainsHistoricalStage6CompletionIo ==
  IndexedChainSpec
    => \A initialContext \in AdmissibleContextRecords,
          candidate, position:
         \A depth \in Nat:
           IndexedHistoricalStage6CompletionIoBlockedAtDepth(
             initialContext, candidate, position, depth)
             ~> IndexedHistoricalStage6CompletionIoDrainGoal(
                  initialContext, candidate, position)
PROOF
  <1>1. ASSUME IndexedChainSpec,
                NEW initialContext \in AdmissibleContextRecords,
                NEW candidate, NEW position
         PROVE \A depth \in Nat:
                  IndexedHistoricalStage6CompletionIoBlockedAtDepth(
                    initialContext, candidate, position, depth)
                    ~> IndexedHistoricalStage6CompletionIoDrainGoal(
                         initialContext, candidate, position)
    <2>1. \A depth \in Nat:
             IndexedHistoricalStage6CompletionIoBlockedAtDepth(
               initialContext, candidate, position, depth)
               ~> IndexedHistoricalStage6CompletionIoProgress(
                    initialContext, candidate, position, depth)
      BY <1>1,
         IndexedChainSpecHistoricalStage6CompletionIoOneStep
    <2> QED BY <2>1, NatLessThanWellFounded,
         WellFoundedLeadsTo
         DEF IndexedHistoricalStage6CompletionIoProgress,
             IndexedHistoricalStage6CompletionIoDrainGoal,
             IndexedHistoricalTransport!
               HistoricalTemporalStage6CompletionIoProgress
  <1> QED BY <1>1

(***************************************************************************
Indexed Stage-6 Completion ready-owner handoff.

At zero physical depth, the selected Completion is a protected Stage-4
owner.  The already-closed indexed Stage-4 leaf advances that exact owner;
the one-height safety bridge then opens the blocked causal head.
***************************************************************************)

IndexedHistoricalStage6CompletionGoal(
    initialContext, candidate, position) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalTemporalStage6CompletionGoal(candidate, position)

IndexedHistoricalStage6CompletionReadyBlocked(
    initialContext, candidate, position) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalTemporalStage6CompletionReadyBlocked(
      candidate, position)

IndexedHistoricalStage6CompletionReadyWitnessBlocked(
    initialContext, candidate, position,
    readyCandidate, readyPosition) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalTemporalStage6CompletionReadyWitnessBlocked(
      candidate, position, readyCandidate, readyPosition)

THEOREM IndexedHistoricalStage4GoalImpliesRankExit ==
  \A initialContext \in AdmissibleContextRecords,
     candidate, position:
    /\ position \in Nat
    /\ IndexedHistoricalTemporalStage4Goal(
         initialContext, candidate, position)
    => IndexedHistoricalTemporalRankProgressExit(
         initialContext, candidate, <<4, position>>)
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW candidate, NEW position,
                position \in Nat,
                IndexedHistoricalTemporalStage4Goal(
                  initialContext, candidate, position)
         PROVE IndexedHistoricalTemporalRankProgressExit(
                 initialContext, candidate, <<4, position>>)
    <2> QED BY <1>1,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage4LeafGoalImpliesRankExit, Isa
         DEF IndexedHistoricalTemporalStage4Goal,
             IndexedHistoricalTemporalRankProgressExit
  <1> QED BY <1>1

THEOREM IndexedChainSpecClosesHistoricalStage4ToRankExit ==
  /\ IndexedChainSpec
  /\ IndexedHistoricalFiniteRunnerEpisodeClosureProperty
    => \A initialContext \in AdmissibleContextRecords,
          candidate \in
            IndexedHistoricalTransport(initialContext)!AsyncCandidateSet,
          position \in Nat:
         IndexedHistoricalTemporalStage4Source(
           initialContext, candidate, position)
           ~> IndexedHistoricalTemporalRankProgressExit(
                initialContext, candidate, <<4, position>>)
PROOF
  <1>1. ASSUME IndexedChainSpec,
                IndexedHistoricalFiniteRunnerEpisodeClosureProperty,
                NEW initialContext \in AdmissibleContextRecords,
                NEW candidate \in
                  IndexedHistoricalTransport(initialContext)!
                    AsyncCandidateSet,
                NEW position \in Nat
         PROVE IndexedHistoricalTemporalStage4Source(
                 initialContext, candidate, position)
                 ~>
               IndexedHistoricalTemporalRankProgressExit(
                 initialContext, candidate, <<4, position>>)
    <2>1. IndexedHistoricalTemporalStage4Source(
             initialContext, candidate, position)
             ~> IndexedHistoricalTemporalStage4Goal(
                  initialContext, candidate, position)
      BY <1>1,
         IndexedChainSpecClosesHistoricalTemporalStage4Leaf
         DEF IndexedHistoricalFiniteRunnerEpisodeClosureProperty,
             IndexedHistoricalTemporalStage4LeafProperty
    <2>2. IndexedHistoricalTemporalStage4Goal(
             initialContext, candidate, position)
             => IndexedHistoricalTemporalRankProgressExit(
                  initialContext, candidate, <<4, position>>)
      BY <1>1, IndexedHistoricalStage4GoalImpliesRankExit
    <2> QED BY <2>1, <2>2, PTL
  <1> QED BY <1>1

THEOREM IndexedHistoricalStage6CompletionReadyWitnessExists ==
  \A initialContext \in AdmissibleContextRecords,
     candidate, position:
    IndexedHistoricalStage6CompletionReadyBlocked(
      initialContext, candidate, position)
      => \E readyCandidate \in
              IndexedHistoricalTransport(initialContext)!
                AsyncCandidateSet,
            readyPosition \in Nat:
           IndexedHistoricalStage6CompletionReadyWitnessBlocked(
             initialContext, candidate, position,
             readyCandidate, readyPosition)
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW candidate, NEW position,
                IndexedHistoricalStage6CompletionReadyBlocked(
                  initialContext, candidate, position)
         PROVE \E readyCandidate \in
                       IndexedHistoricalTransport(initialContext)!
                         AsyncCandidateSet,
                     readyPosition \in Nat:
                   IndexedHistoricalStage6CompletionReadyWitnessBlocked(
                     initialContext, candidate, position,
                     readyCandidate, readyPosition)
    <2> QED BY <1>1,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage6CompletionReadyWitnessExists
         DEF IndexedHistoricalStage6CompletionReadyBlocked,
             IndexedHistoricalStage6CompletionReadyWitnessBlocked
  <1> QED BY <1>1

THEOREM IndexedHistoricalStage6CompletionReadyWitnessUnless ==
  \A initialContext \in AdmissibleContextRecords,
     candidate, position, readyCandidate, readyPosition:
    /\ IndexedHistoricalStage6CompletionReadyWitnessBlocked(
         initialContext, candidate, position,
         readyCandidate, readyPosition)
    /\ [IndexedChainNext]_IndexedChainVars
    => \/ IndexedHistoricalStage6CompletionReadyWitnessBlocked(
             initialContext, candidate, position,
             readyCandidate, readyPosition)'
       \/ IndexedHistoricalStage6CompletionGoal(
            initialContext, candidate, position)'
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW candidate, NEW position,
                NEW readyCandidate, NEW readyPosition,
                IndexedHistoricalStage6CompletionReadyWitnessBlocked(
                  initialContext, candidate, position,
                  readyCandidate, readyPosition),
                [IndexedChainNext]_IndexedChainVars
         PROVE
           \/ IndexedHistoricalStage6CompletionReadyWitnessBlocked(
                initialContext, candidate, position,
                readyCandidate, readyPosition)'
           \/ IndexedHistoricalStage6CompletionGoal(
                initialContext, candidate, position)'
    <2>1. [IndexedHistoricalTransport(initialContext)!AsyncNext]_(
             IndexedHistoricalTransport(initialContext)!AsyncAllVars)
      BY <1>1,
         IndexedBracketStepProjectsEveryHistoricalTransportStep
    <2> QED BY <1>1, <2>1,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage6CompletionReadyWitnessUnless
         DEF IndexedHistoricalStage6CompletionReadyWitnessBlocked,
             IndexedHistoricalStage6CompletionGoal
  <1> QED BY <1>1

THEOREM IndexedChainSpecOpensHistoricalStage6CompletionReadyWitness ==
  /\ IndexedChainSpec
  /\ IndexedHistoricalFiniteRunnerEpisodeClosureProperty
    => \A initialContext \in AdmissibleContextRecords,
          candidate, position:
         \A readyCandidate \in
              IndexedHistoricalTransport(initialContext)!
                AsyncCandidateSet,
            readyPosition \in Nat:
           IndexedHistoricalStage6CompletionReadyWitnessBlocked(
             initialContext, candidate, position,
             readyCandidate, readyPosition)
             ~> IndexedHistoricalStage6CompletionGoal(
                  initialContext, candidate, position)
PROOF
  <1>1. ASSUME IndexedChainSpec,
                IndexedHistoricalFiniteRunnerEpisodeClosureProperty,
                NEW initialContext \in AdmissibleContextRecords,
                NEW candidate, NEW position,
                NEW readyCandidate \in
                  IndexedHistoricalTransport(initialContext)!
                    AsyncCandidateSet,
                NEW readyPosition \in Nat
         PROVE IndexedHistoricalStage6CompletionReadyWitnessBlocked(
                 initialContext, candidate, position,
                 readyCandidate, readyPosition)
                 ~>
               IndexedHistoricalStage6CompletionGoal(
                 initialContext, candidate, position)
    <2>1. IndexedHistoricalTemporalStage4Source(
             initialContext, readyCandidate, readyPosition)
             ~> IndexedHistoricalTemporalRankProgressExit(
                  initialContext, readyCandidate,
                  <<4, readyPosition>>)
      BY <1>1,
         IndexedChainSpecClosesHistoricalStage4ToRankExit
    <2>2. [][IndexedChainNext]_IndexedChainVars
      BY <1>1 DEF IndexedChainSpec
    <2>3. IndexedHistoricalStage6CompletionReadyWitnessBlocked(
             initialContext, candidate, position,
             readyCandidate, readyPosition)
             /\ [IndexedChainNext]_IndexedChainVars
            => \/ IndexedHistoricalStage6CompletionReadyWitnessBlocked(
                    initialContext, candidate, position,
                    readyCandidate, readyPosition)'
               \/ IndexedHistoricalStage6CompletionGoal(
                    initialContext, candidate, position)'
      BY <1>1,
         IndexedHistoricalStage6CompletionReadyWitnessUnless
    <2>4. IndexedHistoricalStage6CompletionReadyWitnessBlocked(
             initialContext, candidate, position,
             readyCandidate, readyPosition)
            => /\ IndexedHistoricalTemporalStage4Source(
                     initialContext, readyCandidate, readyPosition)
               /\ ~IndexedHistoricalTemporalRankProgressExit(
                    initialContext, readyCandidate,
                    <<4, readyPosition>>)
      BY Isa
         DEF IndexedHistoricalStage6CompletionReadyWitnessBlocked,
             IndexedHistoricalTemporalStage4Source,
             IndexedHistoricalTemporalRankProgressExit,
             IndexedHistoricalTransport!
               HistoricalTemporalStage6CompletionReadyWitnessBlocked,
             IndexedHistoricalTransport!
               HistoricalTemporalRankProgressExit,
             IndexedHistoricalTransport!
               HistoricalProtectedOwnedAtServiceRank,
             IndexedHistoricalTransport!
               HistoricalProtectedServiceOwnershipExit,
             IndexedHistoricalTransport!ServiceRankLess
    <2> QED BY <2>1, <2>2, <2>3, <2>4, PTL
  <1> QED BY <1>1

THEOREM IndexedChainSpecOpensHistoricalStage6CompletionReady ==
  /\ IndexedChainSpec
  /\ IndexedHistoricalFiniteRunnerEpisodeClosureProperty
    => \A initialContext \in AdmissibleContextRecords,
          candidate, position:
         IndexedHistoricalStage6CompletionReadyBlocked(
           initialContext, candidate, position)
           ~> IndexedHistoricalStage6CompletionGoal(
                initialContext, candidate, position)
PROOF
  <1>1. ASSUME IndexedChainSpec,
                IndexedHistoricalFiniteRunnerEpisodeClosureProperty,
                NEW initialContext \in AdmissibleContextRecords,
                NEW candidate, NEW position
         PROVE IndexedHistoricalStage6CompletionReadyBlocked(
                 initialContext, candidate, position)
                 ~>
               IndexedHistoricalStage6CompletionGoal(
                 initialContext, candidate, position)
    <2>1. IndexedHistoricalStage6CompletionReadyBlocked(
             initialContext, candidate, position)
             => \E readyCandidate \in
                    IndexedHistoricalTransport(initialContext)!
                      AsyncCandidateSet,
                  readyPosition \in Nat:
                  IndexedHistoricalStage6CompletionReadyWitnessBlocked(
                    initialContext, candidate, position,
                    readyCandidate, readyPosition)
      BY IndexedHistoricalStage6CompletionReadyWitnessExists
    <2>2. \A readyCandidate \in
               IndexedHistoricalTransport(initialContext)!
                 AsyncCandidateSet,
             readyPosition \in Nat:
             IndexedHistoricalStage6CompletionReadyWitnessBlocked(
               initialContext, candidate, position,
               readyCandidate, readyPosition)
               ~> IndexedHistoricalStage6CompletionGoal(
                    initialContext, candidate, position)
      BY <1>1,
         IndexedChainSpecOpensHistoricalStage6CompletionReadyWitness
    <2> QED BY <2>1, <2>2, PTL
  <1> QED BY <1>1

THEOREM IndexedChainSpecOpensHistoricalStage6CompletionCapacity ==
  /\ IndexedChainSpec
  /\ IndexedHistoricalFiniteRunnerEpisodeClosureProperty
    => \A initialContext \in AdmissibleContextRecords,
          candidate, position:
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStage6CompletionCapacityBlocked(
             candidate, position)
           ~> IndexedHistoricalStage6CompletionGoal(
                initialContext, candidate, position)
PROOF
  <1>1. ASSUME IndexedChainSpec,
                IndexedHistoricalFiniteRunnerEpisodeClosureProperty,
                NEW initialContext \in AdmissibleContextRecords,
                NEW candidate, NEW position
         PROVE IndexedHistoricalTransport(initialContext)!
                 HistoricalTemporalStage6CompletionCapacityBlocked(
                   candidate, position)
                 ~>
               IndexedHistoricalStage6CompletionGoal(
                 initialContext, candidate, position)
    <2>1. []IndexedHistoricalTemporalSupportAt(initialContext)
      BY <1>1, IndexedChainSpecAlwaysHistoricalTemporalSupport, PTL
    <2>2. []IndexedHistoricalTransport(initialContext)!
             AsyncTypeInvariant
      BY <2>1,
         IndexedHistoricalTransport(initialContext)!
           AsyncStrongTypeProjectsAsyncType, PTL
         DEF IndexedHistoricalTemporalSupportAt
    <2>3. IndexedHistoricalTransport(initialContext)!
             HistoricalTemporalStage6CompletionCapacityBlocked(
               candidate, position)
             ~> (IndexedHistoricalStage6CompletionGoal(
                   initialContext, candidate, position)
                  \/ IndexedHistoricalStage6CompletionReadyBlocked(
                       initialContext, candidate, position))
      <3>1. (IndexedHistoricalTransport(initialContext)!
                HistoricalTemporalStage6CompletionCapacityBlocked(
                  candidate, position)
               /\ IndexedHistoricalTransport(initialContext)!
                    AsyncIoQueueDepth(candidate.node) > 0)
               ~> IndexedHistoricalStage6CompletionIoDrainGoal(
                    initialContext, candidate, position)
        <4>1. IndexedHistoricalTransport(initialContext)!
                 HistoricalTemporalStage6CompletionCapacityBlocked(
                   candidate, position)
                 /\ IndexedHistoricalTransport(initialContext)!
                      AsyncIoQueueDepth(candidate.node) > 0
                ~> IndexedHistoricalStage6CompletionIoBlockedAtDepth(
                     initialContext, candidate, position,
                     IndexedHistoricalTransport(initialContext)!
                       AsyncIoQueueDepth(candidate.node))
          BY <2>2, PTL
             DEF IndexedHistoricalStage6CompletionIoBlockedAtDepth,
                 IndexedHistoricalTransport!
                   HistoricalTemporalStage6CompletionIoBlockedAtDepth
        <4>2. \A depth \in Nat:
                 IndexedHistoricalStage6CompletionIoBlockedAtDepth(
                   initialContext, candidate, position, depth)
                   ~> IndexedHistoricalStage6CompletionIoDrainGoal(
                        initialContext, candidate, position)
          BY <1>1,
             IndexedChainSpecDrainsHistoricalStage6CompletionIo
        <4> QED BY <4>1, <4>2, PTL
      <3>2. IndexedHistoricalStage6CompletionIoDrainGoal(
               initialContext, candidate, position)
              => \/ IndexedHistoricalStage6CompletionGoal(
                       initialContext, candidate, position)
                 \/ IndexedHistoricalStage6CompletionReadyBlocked(
                      initialContext, candidate, position)
        BY Isa
           DEF IndexedHistoricalStage6CompletionIoDrainGoal,
               IndexedHistoricalStage6CompletionGoal,
               IndexedHistoricalStage6CompletionReadyBlocked,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage6CompletionIoDrainGoal,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage6CompletionReadyBlocked
      <3>3. /\ IndexedHistoricalTransport(initialContext)!
                    HistoricalTemporalStage6CompletionCapacityBlocked(
                      candidate, position)
              /\ IndexedHistoricalTransport(initialContext)!
                   AsyncIoQueueDepth(candidate.node) = 0
             => IndexedHistoricalStage6CompletionReadyBlocked(
                  initialContext, candidate, position)
        BY DEF IndexedHistoricalStage6CompletionReadyBlocked,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage6CompletionReadyBlocked
      <3> QED BY <3>1, <3>2, <3>3, PTL
    <2>4. IndexedHistoricalStage6CompletionReadyBlocked(
             initialContext, candidate, position)
             ~> IndexedHistoricalStage6CompletionGoal(
                  initialContext, candidate, position)
      BY <1>1,
         IndexedChainSpecOpensHistoricalStage6CompletionReady
    <2> QED BY <2>3, <2>4, PTL
  <1> QED BY <1>1

(***************************************************************************
Indexed historical Stage-6 leaf.
***************************************************************************)

IndexedHistoricalTemporalStage6Source(
    initialContext, candidate, position) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalProtectedOwnedAtServiceRank(
      candidate, <<6, position>>)

IndexedHistoricalTemporalStage6Goal(
    initialContext, candidate, position) ==
  \/ IndexedHistoricalTransport(initialContext)!
       HistoricalProtectedServiceOwnershipExit(candidate)
  \/ \E lower \in SetLessThan(
       <<6, position>>,
       IndexedHistoricalTransport(initialContext)!
         OwnedServiceRankOrdering,
       IndexedHistoricalTransport(initialContext)!
         OwnedServiceRankCarrier):
       IndexedHistoricalTransport(initialContext)!
         HistoricalProtectedOwnedAtServiceRank(candidate, lower)

IndexedHistoricalTemporalStage6LeafProperty ==
  \A initialContext \in AdmissibleContextRecords,
     candidate \in IndexedHistoricalTransport(initialContext)!
                    AsyncCandidateSet,
     position \in Nat:
    IndexedHistoricalTemporalStage6Source(
      initialContext, candidate, position)
      ~> IndexedHistoricalTemporalStage6Goal(
           initialContext, candidate, position)

THEOREM IndexedChainSpecClosesHistoricalTemporalStage6Leaf ==
  /\ IndexedChainSpec
  /\ IndexedHistoricalFiniteRunnerEpisodeClosureProperty
    => IndexedHistoricalTemporalStage6LeafProperty
PROOF
  <1>1. ASSUME IndexedChainSpec,
                IndexedHistoricalFiniteRunnerEpisodeClosureProperty,
                NEW initialContext \in AdmissibleContextRecords,
                NEW candidate \in
                  IndexedHistoricalTransport(initialContext)!
                    AsyncCandidateSet,
                NEW position \in Nat
         PROVE IndexedHistoricalTemporalStage6Source(
                 initialContext, candidate, position)
                 ~>
               IndexedHistoricalTemporalStage6Goal(
                 initialContext, candidate, position)
    <2>1. []IndexedHistoricalTemporalSupportAt(initialContext)
      BY <1>1, IndexedChainSpecAlwaysHistoricalTemporalSupport, PTL
    <2>2. IndexedHistoricalTemporalStage6Source(
             initialContext, candidate, position)
             ~> IndexedHistoricalTransport(initialContext)!
                   HistoricalTemporalStage6Pending(
                     candidate, position)
      BY <2>1, PTL
         DEF IndexedHistoricalTemporalStage6Source,
             IndexedHistoricalTemporalSupportAt,
             IndexedHistoricalTransport!
               HistoricalTemporalStage6Pending
    <2>3. IndexedHistoricalTransport(initialContext)!
             HistoricalTemporalStage6Pending(candidate, position)
             ~> IndexedHistoricalTransport(initialContext)!
                   HistoricalTemporalStage6PreAdmissionGoal(
                     candidate, position)
      BY <1>1,
         IndexedChainSpecClosesHistoricalStage6PreAdmission
         DEF IndexedHistoricalFiniteRunnerEpisodeClosureProperty
    <2>4. IndexedHistoricalTransport(initialContext)!
             HistoricalTemporalStage6OwedCausalReady(
               candidate, position)
             ~> IndexedHistoricalTemporalRankProgressExit(
                   initialContext, candidate, <<6, position>>)
      BY <1>1, IndexedChainSpecClosesHistoricalStage6Owed
         DEF IndexedHistoricalFiniteRunnerEpisodeClosureProperty
    <2>5. IndexedHistoricalTransport(initialContext)!
             HistoricalTemporalStage6NonCompletionCapacityBlocked(
               candidate, position)
             ~> IndexedHistoricalTemporalRankProgressExit(
                   initialContext, candidate, <<6, position>>)
      <3>1. IndexedHistoricalTransport(initialContext)!
               HistoricalTemporalStage6NonCompletionCapacityBlocked(
                 candidate, position)
               ~> IndexedHistoricalTransport(initialContext)!
                     HistoricalTemporalStage6NonCompletionGoal(
                       candidate, position)
        BY <1>1,
           IndexedChainSpecClosesHistoricalStage6NonCompletion
           DEF IndexedHistoricalFiniteRunnerEpisodeClosureProperty
      <3> QED BY <3>1, <2>4, PTL
           DEF IndexedHistoricalTransport!
                 HistoricalTemporalStage6NonCompletionGoal
    <2>6. IndexedHistoricalTransport(initialContext)!
             HistoricalTemporalStage6CompletionCapacityBlocked(
               candidate, position)
             ~> IndexedHistoricalTemporalRankProgressExit(
                   initialContext, candidate, <<6, position>>)
      <3>1. IndexedHistoricalTransport(initialContext)!
               HistoricalTemporalStage6CompletionCapacityBlocked(
                 candidate, position)
               ~> IndexedHistoricalStage6CompletionGoal(
                     initialContext, candidate, position)
        BY <1>1,
           IndexedChainSpecOpensHistoricalStage6CompletionCapacity
      <3> QED BY <3>1, <2>4, PTL
           DEF IndexedHistoricalStage6CompletionGoal,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage6CompletionGoal
    <2>7. IndexedHistoricalTransport(initialContext)!
             HistoricalTemporalStage6PreAdmissionGoal(
               candidate, position)
             ~> IndexedHistoricalTemporalRankProgressExit(
                   initialContext, candidate, <<6, position>>)
      BY <2>4, <2>5, <2>6, PTL
         DEF IndexedHistoricalTransport!
               HistoricalTemporalStage6PreAdmissionGoal
    <2>8. IndexedHistoricalTemporalSupportAt(initialContext)
             /\ IndexedHistoricalTemporalRankProgressExit(
                  initialContext, candidate, <<6, position>>)
             => IndexedHistoricalTemporalStage6Goal(
                  initialContext, candidate, position)
      BY <1>1,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalRankExitHasWellFoundedSuccessor, Isa
         DEF IndexedHistoricalTemporalSupportAt,
             IndexedHistoricalTemporalRankProgressExit,
             IndexedHistoricalTemporalStage6Goal,
             IndexedHistoricalTransport!
               HistoricalTemporalRankProgressExit,
             IndexedHistoricalTransport!
               HistoricalProtectedServiceOwnershipExit,
             IndexedHistoricalTransport!OwnedServiceRankCarrier
    <2>9. IndexedHistoricalTemporalRankProgressExit(
             initialContext, candidate, <<6, position>>)
             ~> IndexedHistoricalTemporalStage6Goal(
                  initialContext, candidate, position)
      BY <2>1, <2>8, PTL
    <2> QED BY <2>2, <2>3, <2>7, <2>9, PTL
  <1> QED BY <1>1
       DEF IndexedHistoricalTemporalStage6LeafProperty

(***************************************************************************
Indexed Stage-2 post-deferred support.

A Busy Stage-2 owner is witnessed by a Completion already in stages 3..6.
The four indexed leaves above therefore close the exact restricted product
rank before the deferred handoff itself is considered.
***************************************************************************)

IndexedHistoricalTemporalPostDeferredExit(initialContext, candidate) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalTemporalPostDeferredExit(candidate)

IndexedHistoricalTemporalPostDeferredAtRank(
    initialContext, candidate, rank) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalTemporalPostDeferredAtRank(candidate, rank)

THEOREM IndexedHistoricalStageLeafGoalImpliesStrictRank ==
  \A initialContext \in AdmissibleContextRecords,
     candidate, stage, position:
    /\ stage \in 3..6
    /\ position \in Nat
    /\ (IndexedHistoricalTransport(initialContext)!
          HistoricalProtectedServiceOwnershipExit(candidate)
         \/ \E lower \in SetLessThan(
              <<stage, position>>,
              IndexedHistoricalTransport(initialContext)!
                OwnedServiceRankOrdering,
              IndexedHistoricalTransport(initialContext)!
                OwnedServiceRankCarrier):
              IndexedHistoricalTransport(initialContext)!
                HistoricalProtectedOwnedAtServiceRank(
                  candidate, lower))
    => \/ IndexedHistoricalTransport(initialContext)!
            HistoricalProtectedServiceOwnershipExit(candidate)
       \/ IndexedHistoricalTransport(initialContext)!
            ServiceRankLess(
              IndexedHistoricalTransport(initialContext)!
                CandidateServiceRank(candidate),
              <<stage, position>>)
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW candidate, NEW stage, NEW position,
                stage \in 3..6,
                position \in Nat,
                (IndexedHistoricalTransport(initialContext)!
                   HistoricalProtectedServiceOwnershipExit(candidate)
                  \/ \E lower \in SetLessThan(
                       <<stage, position>>,
                       IndexedHistoricalTransport(initialContext)!
                         OwnedServiceRankOrdering,
                       IndexedHistoricalTransport(initialContext)!
                         OwnedServiceRankCarrier):
                       IndexedHistoricalTransport(initialContext)!
                         HistoricalProtectedOwnedAtServiceRank(
                           candidate, lower))
         PROVE
           \/ IndexedHistoricalTransport(initialContext)!
                HistoricalProtectedServiceOwnershipExit(candidate)
           \/ IndexedHistoricalTransport(initialContext)!
                ServiceRankLess(
                  IndexedHistoricalTransport(initialContext)!
                    CandidateServiceRank(candidate),
                  <<stage, position>>)
    <2> QED BY <1>1,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalStageLeafGoalImpliesStrictRank
  <1> QED BY <1>1

THEOREM IndexedChainSpecClosesHistoricalPostDeferredRankStep ==
  /\ IndexedChainSpec
  /\ IndexedHistoricalFiniteRunnerEpisodeClosureProperty
    => \A initialContext \in AdmissibleContextRecords,
          candidate \in
            IndexedHistoricalTransport(initialContext)!AsyncCandidateSet,
          stage \in 3..6, position \in Nat:
         IndexedHistoricalTransport(initialContext)!
           HistoricalProtectedOwnedAtServiceRank(
             candidate, <<stage, position>>)
           ~> (IndexedHistoricalTransport(initialContext)!
                 HistoricalProtectedServiceOwnershipExit(candidate)
                \/ IndexedHistoricalTransport(initialContext)!
                     ServiceRankLess(
                       IndexedHistoricalTransport(initialContext)!
                         CandidateServiceRank(candidate),
                       <<stage, position>>))
PROOF
  <1>1. ASSUME IndexedChainSpec,
                IndexedHistoricalFiniteRunnerEpisodeClosureProperty,
                NEW initialContext \in AdmissibleContextRecords,
                NEW candidate \in
                  IndexedHistoricalTransport(initialContext)!
                    AsyncCandidateSet,
                NEW stage \in 3..6, NEW position \in Nat
         PROVE IndexedHistoricalTransport(initialContext)!
                 HistoricalProtectedOwnedAtServiceRank(
                   candidate, <<stage, position>>)
                 ~>
               (IndexedHistoricalTransport(initialContext)!
                  HistoricalProtectedServiceOwnershipExit(candidate)
                 \/ IndexedHistoricalTransport(initialContext)!
                      ServiceRankLess(
                        IndexedHistoricalTransport(initialContext)!
                          CandidateServiceRank(candidate),
                        <<stage, position>>))
    <2> DEFINE Goal ==
           IndexedHistoricalTransport(initialContext)!
             HistoricalProtectedServiceOwnershipExit(candidate)
             \/ \E lower \in SetLessThan(
                  <<stage, position>>,
                  IndexedHistoricalTransport(initialContext)!
                    OwnedServiceRankOrdering,
                  IndexedHistoricalTransport(initialContext)!
                    OwnedServiceRankCarrier):
                  IndexedHistoricalTransport(initialContext)!
                    HistoricalProtectedOwnedAtServiceRank(
                      candidate, lower)
    <2>1. CASE stage = 3
      BY <1>1, <2>1,
         IndexedChainSpecClosesHistoricalTemporalStage3Leaf
         DEF IndexedHistoricalFiniteRunnerEpisodeClosureProperty,
             Goal, IndexedHistoricalTemporalStage3LeafProperty,
             IndexedHistoricalTemporalStage3Source,
             IndexedHistoricalTemporalStage3Goal
    <2>2. CASE stage = 4
      BY <1>1, <2>2,
         IndexedChainSpecClosesHistoricalTemporalStage4Leaf
         DEF IndexedHistoricalFiniteRunnerEpisodeClosureProperty,
             Goal, IndexedHistoricalTemporalStage4LeafProperty,
             IndexedHistoricalTemporalStage4Source,
             IndexedHistoricalTemporalStage4Goal
    <2>3. CASE stage = 5
      BY <1>1, <2>3,
         IndexedChainSpecClosesHistoricalTemporalStage5Leaf
         DEF Goal, IndexedHistoricalTemporalStage5LeafProperty,
             IndexedHistoricalTemporalStage5Source,
             IndexedHistoricalTemporalStage5Goal
    <2>4. CASE stage = 6
      BY <1>1, <2>4,
         IndexedChainSpecClosesHistoricalTemporalStage6Leaf
         DEF IndexedHistoricalFiniteRunnerEpisodeClosureProperty,
             Goal, IndexedHistoricalTemporalStage6LeafProperty,
             IndexedHistoricalTemporalStage6Source,
             IndexedHistoricalTemporalStage6Goal
    <2>5. IndexedHistoricalTransport(initialContext)!
             HistoricalProtectedOwnedAtServiceRank(
               candidate, <<stage, position>>)
             ~> Goal
      BY <1>1, <2>1, <2>2, <2>3, <2>4, Isa
    <2>6. Goal
             => (IndexedHistoricalTransport(initialContext)!
                   HistoricalProtectedServiceOwnershipExit(candidate)
                  \/ IndexedHistoricalTransport(initialContext)!
                       ServiceRankLess(
                         IndexedHistoricalTransport(initialContext)!
                           CandidateServiceRank(candidate),
                         <<stage, position>>))
      BY <1>1, IndexedHistoricalStageLeafGoalImpliesStrictRank
         DEF Goal
    <2> QED BY <2>5, <2>6, PTL
  <1> QED BY <1>1

THEOREM IndexedChainSpecConvergesHistoricalPostDeferredRank ==
  /\ IndexedChainSpec
  /\ IndexedHistoricalFiniteRunnerEpisodeClosureProperty
    => \A initialContext \in AdmissibleContextRecords, candidate:
         (IndexedHistoricalTransport(initialContext)!gst
           /\ IndexedHistoricalTransport(initialContext)!
                HistoricalProtectedCandidateOwned(candidate)
           /\ IndexedHistoricalTransport(initialContext)!
                CandidateServiceRank(candidate)[1] \in 3..6)
           ~> IndexedHistoricalTemporalPostDeferredExit(
                initialContext, candidate)
PROOF
  <1>1. ASSUME IndexedChainSpec,
                IndexedHistoricalFiniteRunnerEpisodeClosureProperty,
                NEW initialContext \in AdmissibleContextRecords,
                NEW candidate
         PROVE (IndexedHistoricalTransport(initialContext)!gst
                 /\ IndexedHistoricalTransport(initialContext)!
                      HistoricalProtectedCandidateOwned(candidate)
                 /\ IndexedHistoricalTransport(initialContext)!
                      CandidateServiceRank(candidate)[1] \in 3..6)
                 ~>
               IndexedHistoricalTemporalPostDeferredExit(
                 initialContext, candidate)
    <2>1. []IndexedHistoricalTemporalSupportAt(initialContext)
      BY <1>1, IndexedChainSpecAlwaysHistoricalTemporalSupport, PTL
    <2>2. ASSUME NEW rank \in
                  IndexedHistoricalTransport(initialContext)!
                    PostDeferredServiceRankCarrier
           PROVE IndexedHistoricalTemporalPostDeferredAtRank(
                   initialContext, candidate, rank)
                   ~> (IndexedHistoricalTemporalPostDeferredExit(
                         initialContext, candidate)
                        \/ \E lower \in SetLessThan(
                             rank,
                             IndexedHistoricalTransport(initialContext)!
                               PostDeferredServiceRankOrdering,
                             IndexedHistoricalTransport(initialContext)!
                               PostDeferredServiceRankCarrier):
                             IndexedHistoricalTemporalPostDeferredAtRank(
                               initialContext, candidate, lower))
      <3>1. PICK stage \in 3..6, position \in Nat:
               rank = <<stage, position>>
        BY <2>2
           DEF IndexedHistoricalTransport!
                 PostDeferredServiceRankCarrier
      <3>2. IndexedHistoricalTemporalPostDeferredAtRank(
               initialContext, candidate, rank)
               ~> (IndexedHistoricalTransport(initialContext)!
                     HistoricalProtectedServiceOwnershipExit(candidate)
                    \/ IndexedHistoricalTransport(initialContext)!
                         ServiceRankLess(
                           IndexedHistoricalTransport(initialContext)!
                             CandidateServiceRank(candidate),
                           rank))
        BY <1>1, <3>1,
           IndexedChainSpecClosesHistoricalPostDeferredRankStep
           DEF IndexedHistoricalTemporalPostDeferredAtRank,
               IndexedHistoricalTransport!
                 HistoricalTemporalPostDeferredAtRank
      <3>3. IndexedHistoricalTemporalSupportAt(initialContext)
               /\ IndexedHistoricalTemporalPostDeferredAtRank(
                    initialContext, candidate, rank)
               /\ IndexedHistoricalTransport(initialContext)!
                    ServiceRankLess(
                      IndexedHistoricalTransport(initialContext)!
                        CandidateServiceRank(candidate),
                      rank)
              => \/ IndexedHistoricalTemporalPostDeferredExit(
                      initialContext, candidate)
                 \/ \E lower \in SetLessThan(
                      rank,
                      IndexedHistoricalTransport(initialContext)!
                        PostDeferredServiceRankOrdering,
                      IndexedHistoricalTransport(initialContext)!
                        PostDeferredServiceRankCarrier):
                      IndexedHistoricalTemporalPostDeferredAtRank(
                        initialContext, candidate, lower)
        BY <2>2,
           IndexedHistoricalTransport(initialContext)!
             AsyncStrongTypeProjectsAsyncType,
           IndexedHistoricalTransport(initialContext)!
             ScheduledCandidateServiceRankInCarrier,
           IndexedHistoricalTransport(initialContext)!
             OwnedServiceRankOrderingMatchesLess, Isa
           DEF IndexedHistoricalTemporalSupportAt,
               IndexedHistoricalTemporalPostDeferredExit,
               IndexedHistoricalTemporalPostDeferredAtRank,
               IndexedHistoricalTransport!
                 HistoricalTemporalPostDeferredExit,
               IndexedHistoricalTransport!
                 HistoricalTemporalPostDeferredAtRank,
               IndexedHistoricalTransport!
                 HistoricalProtectedCandidateOwned,
               IndexedHistoricalTransport!ProtectedCandidateOwned,
               IndexedHistoricalTransport!CandidateScheduled,
               IndexedHistoricalTransport!CandidateServiceRank,
               IndexedHistoricalTransport!ServiceRankLess,
               IndexedHistoricalTransport!
                 PostDeferredServiceRankOrdering,
               IndexedHistoricalTransport!
                 PostDeferredServiceRankCarrier,
               SetLessThan
      <3> QED BY <2>1, <3>2, <3>3, PTL
           DEF IndexedHistoricalTemporalPostDeferredExit
    <2>3. \A rank \in
               IndexedHistoricalTransport(initialContext)!
                 PostDeferredServiceRankCarrier:
             IndexedHistoricalTemporalPostDeferredAtRank(
               initialContext, candidate, rank)
               ~> IndexedHistoricalTemporalPostDeferredExit(
                    initialContext, candidate)
      BY <2>2,
         IndexedHistoricalTransport(initialContext)!
           PostDeferredServiceRankOrderingWellFoundedObligation,
         WellFoundedLeadsTo
    <2>4. (IndexedHistoricalTransport(initialContext)!gst
             /\ IndexedHistoricalTransport(initialContext)!
                  HistoricalProtectedCandidateOwned(candidate)
             /\ IndexedHistoricalTransport(initialContext)!
                  CandidateServiceRank(candidate)[1] \in 3..6)
             ~> \E rank \in
                   IndexedHistoricalTransport(initialContext)!
                     PostDeferredServiceRankCarrier:
                  IndexedHistoricalTemporalPostDeferredAtRank(
                    initialContext, candidate, rank)
      BY Isa, PTL
         DEF IndexedHistoricalTemporalPostDeferredAtRank,
             IndexedHistoricalTransport!
               HistoricalTemporalPostDeferredAtRank,
             IndexedHistoricalTransport!
               PostDeferredServiceRankCarrier
    <2> QED BY <2>3, <2>4, PTL
  <1> QED BY <1>1

(***************************************************************************
Indexed Stage-2 Busy-phase descent.
***************************************************************************)

IndexedHistoricalTemporalStage2Owned(initialContext, candidate) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalTemporalStage2Owned(candidate)

IndexedHistoricalTemporalBusyCompletionWitness(
    initialContext, target, witness) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalTemporalBusyCompletionWitness(target, witness)

IndexedHistoricalTemporalStage2BusyPhaseGoal(
    initialContext, target, phase) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalTemporalStage2BusyPhaseGoal(target, phase)

IndexedHistoricalTemporalStage2BusyWitnessBlocked(
    initialContext, target, witness, phase) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalTemporalStage2BusyWitnessBlocked(
      target, witness, phase)

THEOREM IndexedHistoricalBusyWitnessHasPostDeferredRank ==
  \A initialContext \in AdmissibleContextRecords,
     target, witness:
    /\ IndexedHistoricalTemporalSupportAt(initialContext)
    /\ IndexedHistoricalTemporalBusyCompletionWitness(
         initialContext, target, witness)
    => /\ IndexedHistoricalTransport(initialContext)!
            HistoricalProtectedCandidateOwned(witness)
       /\ IndexedHistoricalTransport(initialContext)!
            CandidateServiceRank(witness)
            \in IndexedHistoricalTransport(initialContext)!
                 PostDeferredServiceRankCarrier
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW target, NEW witness,
                IndexedHistoricalTemporalSupportAt(initialContext),
                IndexedHistoricalTemporalBusyCompletionWitness(
                  initialContext, target, witness)
         PROVE
           /\ IndexedHistoricalTransport(initialContext)!
                HistoricalProtectedCandidateOwned(witness)
           /\ IndexedHistoricalTransport(initialContext)!
                CandidateServiceRank(witness)
                \in IndexedHistoricalTransport(initialContext)!
                     PostDeferredServiceRankCarrier
    <2> QED BY <1>1,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalBusyWitnessHasPostDeferredRank
         DEF IndexedHistoricalTemporalSupportAt,
             IndexedHistoricalTemporalBusyCompletionWitness
  <1> QED BY <1>1

THEOREM IndexedHistoricalBusyWitnessPersistsOrPhaseDrops ==
  \A initialContext \in AdmissibleContextRecords,
     target, witness:
    /\ IndexedHistoricalTemporalSupportAt(initialContext)
    /\ IndexedHistoricalTemporalBusyCompletionWitness(
         initialContext, target, witness)
    /\ [IndexedChainNext]_IndexedChainVars
    /\ ~IndexedHistoricalTransport(initialContext)!
          HistoricalProtectedServiceOwnershipExit(target)'
    /\ IndexedHistoricalTransport(initialContext)!
         BusyPhaseRank(target.node)'
         >= IndexedHistoricalTransport(initialContext)!
              BusyPhaseRank(target.node)
    => IndexedHistoricalTemporalBusyCompletionWitness(
         initialContext, target, witness)'
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW target, NEW witness,
                IndexedHistoricalTemporalSupportAt(initialContext),
                IndexedHistoricalTemporalBusyCompletionWitness(
                  initialContext, target, witness),
                [IndexedChainNext]_IndexedChainVars,
                ~IndexedHistoricalTransport(initialContext)!
                   HistoricalProtectedServiceOwnershipExit(target)',
                IndexedHistoricalTransport(initialContext)!
                  BusyPhaseRank(target.node)'
                  >= IndexedHistoricalTransport(initialContext)!
                       BusyPhaseRank(target.node)
         PROVE IndexedHistoricalTemporalBusyCompletionWitness(
                 initialContext, target, witness)'
    <2>1. [IndexedHistoricalTransport(initialContext)!AsyncNext]_(
             IndexedHistoricalTransport(initialContext)!AsyncAllVars)
      BY <1>1,
         IndexedBracketStepProjectsEveryHistoricalTransportStep
    <2> QED BY <1>1, <2>1,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalBusyWitnessOwnershipPersists
         DEF IndexedHistoricalTemporalSupportAt,
             IndexedHistoricalTemporalBusyCompletionWitness
  <1> QED BY <1>1

THEOREM IndexedHistoricalBusyPhaseCannotIncrease ==
  \A initialContext \in AdmissibleContextRecords, target:
    /\ IndexedHistoricalTemporalSupportAt(initialContext)
    /\ IndexedHistoricalTemporalStage2Owned(initialContext, target)
    /\ IndexedHistoricalTransport(initialContext)!
         BusyPhaseRank(target.node) \in 1..2
    /\ [IndexedChainNext]_IndexedChainVars
    /\ ~IndexedHistoricalTransport(initialContext)!
          HistoricalProtectedServiceOwnershipExit(target)'
    => IndexedHistoricalTransport(initialContext)!
         BusyPhaseRank(target.node)'
         <= IndexedHistoricalTransport(initialContext)!
              BusyPhaseRank(target.node)
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW target,
                IndexedHistoricalTemporalSupportAt(initialContext),
                IndexedHistoricalTemporalStage2Owned(
                  initialContext, target),
                IndexedHistoricalTransport(initialContext)!
                  BusyPhaseRank(target.node) \in 1..2,
                [IndexedChainNext]_IndexedChainVars,
                ~IndexedHistoricalTransport(initialContext)!
                   HistoricalProtectedServiceOwnershipExit(target)'
         PROVE IndexedHistoricalTransport(initialContext)!
                 BusyPhaseRank(target.node)'
                 <= IndexedHistoricalTransport(initialContext)!
                      BusyPhaseRank(target.node)
    <2>1. [IndexedHistoricalTransport(initialContext)!AsyncNext]_(
             IndexedHistoricalTransport(initialContext)!AsyncAllVars)
      BY <1>1,
         IndexedBracketStepProjectsEveryHistoricalTransportStep
    <2> QED BY <1>1, <2>1,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalBusyPhaseCannotIncrease
         DEF IndexedHistoricalTemporalSupportAt,
             IndexedHistoricalTemporalStage2Owned
  <1> QED BY <1>1

THEOREM IndexedChainSpecDescendsHistoricalStage2BusyPhase ==
  /\ IndexedChainSpec
  /\ IndexedHistoricalFiniteRunnerEpisodeClosureProperty
    => \A initialContext \in AdmissibleContextRecords, target:
         \A phase \in 1..2:
           (IndexedHistoricalTemporalStage2Owned(
              initialContext, target)
             /\ IndexedHistoricalTransport(initialContext)!
                  BusyPhaseRank(target.node) = phase)
             ~> IndexedHistoricalTemporalStage2BusyPhaseGoal(
                  initialContext, target, phase)
PROOF
  <1>1. ASSUME IndexedChainSpec,
                IndexedHistoricalFiniteRunnerEpisodeClosureProperty,
                NEW initialContext \in AdmissibleContextRecords,
                NEW target, NEW phase \in 1..2
         PROVE (IndexedHistoricalTemporalStage2Owned(
                  initialContext, target)
                 /\ IndexedHistoricalTransport(initialContext)!
                      BusyPhaseRank(target.node) = phase)
                 ~>
               IndexedHistoricalTemporalStage2BusyPhaseGoal(
                 initialContext, target, phase)
    <2>1. []IndexedHistoricalTemporalSupportAt(initialContext)
      BY <1>1, IndexedChainSpecAlwaysHistoricalTemporalSupport, PTL
    <2>2. IndexedHistoricalTemporalSupportAt(initialContext)
               /\ IndexedHistoricalTemporalStage2Owned(
                    initialContext, target)
               /\ IndexedHistoricalTransport(initialContext)!
                    BusyPhaseRank(target.node) = phase
              => \E witness \in
                    IndexedHistoricalTransport(initialContext)!
                      AsyncCandidateSet:
                   IndexedHistoricalTemporalStage2BusyWitnessBlocked(
                     initialContext, target, witness, phase)
      BY <1>1,
         IndexedHistoricalTransport(initialContext)!
           BusyPhaseOwnerPartitionObligation, Isa
         DEF IndexedHistoricalTemporalSupportAt,
             IndexedHistoricalTemporalStage2Owned,
             IndexedHistoricalTemporalStage2BusyWitnessBlocked,
             IndexedHistoricalTransport!
               HistoricalTemporalStage2BusyWitnessBlocked,
             IndexedHistoricalTransport!
               HistoricalTemporalBusyCompletionWitness,
             IndexedHistoricalTransport!Stage2BusyKernelInvariant,
             IndexedHistoricalTransport!
               AsyncProgressOwnershipInvariant,
             IndexedHistoricalTransport!
               BusyCompletionWitnessInvariant
    <2>3. ASSUME NEW witness \in
                  IndexedHistoricalTransport(initialContext)!
                    AsyncCandidateSet
           PROVE IndexedHistoricalTemporalStage2BusyWitnessBlocked(
                   initialContext, target, witness, phase)
                   ~> IndexedHistoricalTemporalStage2BusyPhaseGoal(
                        initialContext, target, phase)
      <3>1. IndexedHistoricalTemporalSupportAt(initialContext)
               /\ IndexedHistoricalTemporalStage2BusyWitnessBlocked(
                    initialContext, target, witness, phase)
              => /\ IndexedHistoricalTransport(initialContext)!
                      HistoricalProtectedCandidateOwned(witness)
                 /\ IndexedHistoricalTransport(initialContext)!
                      CandidateServiceRank(witness)[1] \in 3..6
        BY <2>1,
           IndexedHistoricalBusyWitnessHasPostDeferredRank
           DEF IndexedHistoricalTemporalStage2BusyWitnessBlocked,
               IndexedHistoricalTemporalBusyCompletionWitness,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage2BusyWitnessBlocked,
               IndexedHistoricalTransport!
                 PostDeferredServiceRankCarrier
      <3>2. (IndexedHistoricalTransport(initialContext)!gst
               /\ IndexedHistoricalTransport(initialContext)!
                    HistoricalProtectedCandidateOwned(witness)
               /\ IndexedHistoricalTransport(initialContext)!
                    CandidateServiceRank(witness)[1] \in 3..6)
              ~> IndexedHistoricalTemporalPostDeferredExit(
                   initialContext, witness)
        BY <1>1,
           IndexedChainSpecConvergesHistoricalPostDeferredRank
      <3>3. IndexedHistoricalTemporalStage2BusyWitnessBlocked(
               initialContext, target, witness, phase)
               ~> IndexedHistoricalTemporalPostDeferredExit(
                    initialContext, witness)
        BY <2>1, <3>1, <3>2, PTL
           DEF IndexedHistoricalTemporalStage2BusyWitnessBlocked,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage2BusyWitnessBlocked,
               IndexedHistoricalTransport!
                 HistoricalTemporalBusyCompletionWitness,
               IndexedHistoricalTemporalStage2Owned,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage2Owned
      <3>4. IndexedHistoricalTemporalSupportAt(initialContext)
               /\ IndexedHistoricalTemporalStage2BusyWitnessBlocked(
                    initialContext, target, witness, phase)
               /\ [IndexedChainNext]_IndexedChainVars
              => \/ IndexedHistoricalTemporalStage2BusyPhaseGoal(
                      initialContext, target, phase)'
                 \/ IndexedHistoricalTemporalStage2BusyWitnessBlocked(
                      initialContext, target, witness, phase)'
        <4>1. ASSUME
                 IndexedHistoricalTemporalSupportAt(initialContext),
                 IndexedHistoricalTemporalStage2BusyWitnessBlocked(
                   initialContext, target, witness, phase),
                 [IndexedChainNext]_IndexedChainVars
               PROVE
                 \/ IndexedHistoricalTemporalStage2BusyPhaseGoal(
                      initialContext, target, phase)'
                 \/ IndexedHistoricalTemporalStage2BusyWitnessBlocked(
                      initialContext, target, witness, phase)'
          <5>1. CASE IndexedHistoricalTemporalStage2BusyPhaseGoal(
                        initialContext, target, phase)'
            BY <5>1
          <5>2. CASE ~IndexedHistoricalTemporalStage2BusyPhaseGoal(
                        initialContext, target, phase)'
            <6>1. /\ ~IndexedHistoricalTransport(initialContext)!
                         HistoricalProtectedServiceOwnershipExit(target)'
                   /\ IndexedHistoricalTransport(initialContext)!
                        BusyPhaseRank(target.node)' >= phase
              BY <5>2
                 DEF IndexedHistoricalTemporalStage2BusyPhaseGoal,
                     IndexedHistoricalTransport!
                       HistoricalTemporalStage2BusyPhaseGoal
            <6>2. IndexedHistoricalTransport(initialContext)!
                     BusyPhaseRank(target.node)'
                     <= IndexedHistoricalTransport(initialContext)!
                          BusyPhaseRank(target.node)
              BY <4>1, <6>1,
                 IndexedHistoricalBusyPhaseCannotIncrease
                 DEF IndexedHistoricalTemporalStage2BusyWitnessBlocked,
                     IndexedHistoricalTransport!
                       HistoricalTemporalStage2BusyWitnessBlocked,
                     IndexedHistoricalTransport!
                       HistoricalTemporalBusyCompletionWitness,
                     IndexedHistoricalTemporalStage2Owned,
                     IndexedHistoricalTransport!
                       HistoricalTemporalStage2Owned
            <6>3. IndexedHistoricalTransport(initialContext)!
                     BusyPhaseRank(target.node)' = phase
              BY <4>1, <6>1, <6>2
                 DEF IndexedHistoricalTemporalStage2BusyWitnessBlocked,
                     IndexedHistoricalTransport!
                       HistoricalTemporalStage2BusyWitnessBlocked
            <6>4. IndexedHistoricalTemporalBusyCompletionWitness(
                     initialContext, target, witness)'
              BY <4>1, <6>1,
                 IndexedHistoricalBusyWitnessPersistsOrPhaseDrops
                 DEF IndexedHistoricalTemporalStage2BusyWitnessBlocked,
                     IndexedHistoricalTransport!
                       HistoricalTemporalStage2BusyWitnessBlocked
            <6> QED BY <6>3, <6>4
                 DEF IndexedHistoricalTemporalStage2BusyWitnessBlocked,
                     IndexedHistoricalTransport!
                       HistoricalTemporalStage2BusyWitnessBlocked
          <5> QED BY <5>1, <5>2
        <4> QED BY <4>1
      <3>5. [](IndexedHistoricalTemporalStage2BusyWitnessBlocked(
                   initialContext, target, witness, phase)
                 /\ IndexedHistoricalTemporalPostDeferredExit(
                      initialContext, witness)
                => FALSE)
        BY <2>1,
           IndexedHistoricalBusyWitnessHasPostDeferredRank, PTL
           DEF IndexedHistoricalTemporalStage2BusyWitnessBlocked,
               IndexedHistoricalTemporalBusyCompletionWitness,
               IndexedHistoricalTemporalPostDeferredExit,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage2BusyWitnessBlocked,
               IndexedHistoricalTransport!
                 HistoricalTemporalPostDeferredExit,
               IndexedHistoricalTransport!
                 PostDeferredServiceRankCarrier
      <3> QED BY <2>1, <3>3, <3>4, <3>5, PTL
    <2>4. (IndexedHistoricalTemporalStage2Owned(
             initialContext, target)
             /\ IndexedHistoricalTransport(initialContext)!
                  BusyPhaseRank(target.node) = phase)
            ~> \E witness \in
                  IndexedHistoricalTransport(initialContext)!
                    AsyncCandidateSet:
                 IndexedHistoricalTemporalStage2BusyWitnessBlocked(
                   initialContext, target, witness, phase)
      BY <2>1, <2>2, PTL
    <2> QED BY <2>3, <2>4, PTL
         DEF IndexedHistoricalTemporalStage2BusyPhaseGoal
  <1> QED BY <1>1

IndexedHistoricalTemporalStage2BusyAtPhase(
    initialContext, target, phase) ==
  /\ IndexedHistoricalTemporalStage2Owned(initialContext, target)
  /\ IndexedHistoricalTransport(initialContext)!
       BusyPhaseRank(target.node) = phase

IndexedHistoricalTemporalStage2BusyTerminationGoal(
    initialContext, target) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalTemporalStage2BusyTerminationGoal(target)

THEOREM IndexedChainSpecTerminatesHistoricalStage2Busy ==
  /\ IndexedChainSpec
  /\ IndexedHistoricalFiniteRunnerEpisodeClosureProperty
    => \A initialContext \in AdmissibleContextRecords, target:
         (IndexedHistoricalTemporalStage2Owned(initialContext, target)
           /\ ~IndexedHistoricalTransport(initialContext)!
                 NodeIdle(target.node))
           ~> IndexedHistoricalTemporalStage2BusyTerminationGoal(
                initialContext, target)
PROOF
  <1>1. ASSUME IndexedChainSpec,
                IndexedHistoricalFiniteRunnerEpisodeClosureProperty,
                NEW initialContext \in AdmissibleContextRecords,
                NEW target
         PROVE (IndexedHistoricalTemporalStage2Owned(
                  initialContext, target)
                 /\ ~IndexedHistoricalTransport(initialContext)!
                       NodeIdle(target.node))
                 ~>
               IndexedHistoricalTemporalStage2BusyTerminationGoal(
                 initialContext, target)
    <2>1. []IndexedHistoricalTemporalSupportAt(initialContext)
      BY <1>1, IndexedChainSpecAlwaysHistoricalTemporalSupport, PTL
    <2>2. IndexedHistoricalTemporalStage2BusyAtPhase(
             initialContext, target, 1)
             ~> IndexedHistoricalTemporalStage2BusyTerminationGoal(
                  initialContext, target)
      <3>1. IndexedHistoricalTemporalStage2BusyAtPhase(
               initialContext, target, 1)
               ~> IndexedHistoricalTemporalStage2BusyPhaseGoal(
                    initialContext, target, 1)
        BY <1>1,
           IndexedChainSpecDescendsHistoricalStage2BusyPhase
           DEF IndexedHistoricalTemporalStage2BusyAtPhase
      <3>2. IndexedHistoricalTemporalSupportAt(initialContext)
               /\ IndexedHistoricalTemporalStage2BusyPhaseGoal(
                    initialContext, target, 1)
              => IndexedHistoricalTemporalStage2BusyTerminationGoal(
                   initialContext, target)
        BY <1>1,
           IndexedHistoricalTransport(initialContext)!
             BusyPhaseOwnerPartitionObligation, Isa
           DEF IndexedHistoricalTemporalSupportAt,
               IndexedHistoricalTemporalStage2BusyPhaseGoal,
               IndexedHistoricalTemporalStage2BusyTerminationGoal,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage2BusyPhaseGoal,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage2BusyTerminationGoal,
               IndexedHistoricalTransport!Stage2BusyKernelInvariant,
               IndexedHistoricalTransport!BusyPhaseCarrier
      <3> QED BY <2>1, <3>1, <3>2, PTL
    <2>3. IndexedHistoricalTemporalStage2BusyAtPhase(
             initialContext, target, 2)
             ~> IndexedHistoricalTemporalStage2BusyTerminationGoal(
                  initialContext, target)
      <3>1. IndexedHistoricalTemporalStage2BusyAtPhase(
               initialContext, target, 2)
               ~> IndexedHistoricalTemporalStage2BusyPhaseGoal(
                    initialContext, target, 2)
        BY <1>1,
           IndexedChainSpecDescendsHistoricalStage2BusyPhase
           DEF IndexedHistoricalTemporalStage2BusyAtPhase
      <3>2. IndexedHistoricalTemporalSupportAt(initialContext)
               /\ IndexedHistoricalTemporalStage2BusyPhaseGoal(
                    initialContext, target, 2)
              => \/ IndexedHistoricalTemporalStage2BusyTerminationGoal(
                      initialContext, target)
                 \/ IndexedHistoricalTemporalStage2BusyAtPhase(
                      initialContext, target, 1)
        BY <1>1,
           IndexedHistoricalTransport(initialContext)!
             BusyPhaseOwnerPartitionObligation, Isa
           DEF IndexedHistoricalTemporalSupportAt,
               IndexedHistoricalTemporalStage2BusyPhaseGoal,
               IndexedHistoricalTemporalStage2BusyAtPhase,
               IndexedHistoricalTemporalStage2BusyTerminationGoal,
               IndexedHistoricalTemporalStage2Owned,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage2BusyPhaseGoal,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage2BusyTerminationGoal,
               IndexedHistoricalTransport!Stage2BusyKernelInvariant,
               IndexedHistoricalTransport!BusyPhaseCarrier,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage2Owned,
               IndexedHistoricalTransport!
                 HistoricalProtectedServiceOwnershipExit
      <3> QED BY <2>2, <3>1, <3>2, PTL
    <2>4. IndexedHistoricalTemporalSupportAt(initialContext)
               /\ IndexedHistoricalTemporalStage2Owned(
                    initialContext, target)
               /\ ~IndexedHistoricalTransport(initialContext)!
                     NodeIdle(target.node)
              => \/ IndexedHistoricalTemporalStage2BusyAtPhase(
                      initialContext, target, 1)
                 \/ IndexedHistoricalTemporalStage2BusyAtPhase(
                      initialContext, target, 2)
      BY <1>1,
         IndexedHistoricalTransport(initialContext)!
           BusyPhaseOwnerPartitionObligation, Isa
         DEF IndexedHistoricalTemporalSupportAt,
             IndexedHistoricalTemporalStage2BusyAtPhase,
             IndexedHistoricalTemporalStage2Owned,
             IndexedHistoricalTransport!Stage2BusyKernelInvariant,
             IndexedHistoricalTransport!
               HistoricalTemporalStage2Owned
    <2> QED BY <2>1, <2>2, <2>3, <2>4, PTL
         DEF IndexedHistoricalTemporalStage2BusyTerminationGoal
  <1> QED BY <1>1

(***************************************************************************
Indexed Stage-2 exact deferred handoff.

The handoff token, three-class cursor, and retry lifecycle are source safety
facts.  The only temporal consumer below is the exact joined historical
runner already bridged to its product weak-fairness clause.
***************************************************************************)

IndexedHistoricalTemporalStage2ExactIdleRetryPending(
    initialContext, candidate) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalTemporalStage2ExactIdleRetryPending(candidate)

IndexedHistoricalTemporalStage2ExactIdleRetrySelected(
    initialContext, candidate) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalTemporalStage2ExactIdleRetrySelected(candidate)

IndexedHistoricalTemporalStage2HandoffProgressExit(
    initialContext, candidate) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalTemporalStage2HandoffProgressExit(candidate)

THEOREM IndexedChainSpecExitsHistoricalStage2IdleHandoff ==
  /\ IndexedChainSpec
  /\ IndexedHistoricalFiniteRunnerEpisodeClosureProperty
    => \A initialContext \in AdmissibleContextRecords, candidate:
         IndexedHistoricalTemporalStage2ExactIdleRetryPending(
           initialContext, candidate)
           ~> IndexedHistoricalTemporalStage2HandoffProgressExit(
                initialContext, candidate)
PROOF
  <1>1. ASSUME IndexedChainSpec,
                IndexedHistoricalFiniteRunnerEpisodeClosureProperty,
                NEW initialContext \in AdmissibleContextRecords,
                NEW candidate
         PROVE IndexedHistoricalTemporalStage2ExactIdleRetryPending(
                 initialContext, candidate)
                 ~>
               IndexedHistoricalTemporalStage2HandoffProgressExit(
                 initialContext, candidate)
    <2>1. []IndexedHistoricalTemporalSupportAt(initialContext)
      BY <1>1, IndexedChainSpecAlwaysHistoricalTemporalSupport, PTL
    <2>2. []IndexedCompositionInvariant
      BY <1>1, IndexedChainSpecEstablishesCompositionInvariant
    <2>3. [][IndexedChainNext]_IndexedChainVars
      BY <1>1 DEF IndexedChainSpec
    <2>4. IndexedHistoricalTemporalStage2ExactIdleRetryPending(
             initialContext, candidate)
             => IndexedHistoricalTemporalCandidateRunnerPending(
                  initialContext, candidate)
      BY <2>1
         DEF IndexedHistoricalTemporalStage2ExactIdleRetryPending,
             IndexedHistoricalTemporalCandidateRunnerPending,
             IndexedHistoricalTemporalSupportAt,
             IndexedHistoricalTransport!
               HistoricalTemporalStage2ExactIdleRetryPending,
             IndexedHistoricalTransport!
               HistoricalTemporalStage2Owned,
             IndexedHistoricalTransport!
               HistoricalProtectedCandidateOwned
    <2>5. IndexedCompositionInvariant
             /\ IndexedHistoricalTemporalCandidateRunnerPending(
                  initialContext, candidate)
            => ENABLED
                 <<IndexedRunHistoricalRecoveryStep(
                     initialContext, candidate.node)>>_IndexedChainVars
      BY IndexedHistoricalCandidateRunnerEnablesFairOccurrence
    <2>6. CASE candidate.node \in Responsive
      <3>1. WF_IndexedChainVars(
               IndexedRunHistoricalRecoveryStep(
                 initialContext, candidate.node))
        BY <1>1, <2>6
           DEF IndexedChainSpec, IndexedFairness
      <3>2. (IndexedHistoricalTemporalStage2Owned(
               initialContext, candidate)
               /\ ~IndexedHistoricalTransport(initialContext)!
                     NodeIdle(candidate.node))
               ~> IndexedHistoricalTemporalStage2BusyTerminationGoal(
                    initialContext, candidate)
        BY <1>1,
           IndexedChainSpecTerminatesHistoricalStage2Busy
      <3>3. IndexedHistoricalTemporalStage2ExactIdleRetryPending(
               initialContext, candidate)
               ~> IndexedHistoricalTemporalStage2HandoffProgressExit(
                    initialContext, candidate)
        BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>5,
           <2>6, <3>1, <3>2,
           IndexedBracketStepProjectsEveryHistoricalTransportStep,
           IndexedHistoricalTransport(initialContext)!
             HistoricalTemporalStage2BusyRetryClaimsHandoffAction,
           IndexedHistoricalTransport(initialContext)!
             HistoricalTemporalStage2ForeignIdleSkipDropsDistance,
           IndexedHistoricalTransport(initialContext)!
             HistoricalTemporalStage2HandoffDistanceInCarrier,
           IndexedHistoricalTransport(initialContext)!
             HistoricalTemporalStage2ExactIdleRetryDrainConsumes,
           IndexedHistoricalTransport(initialContext)!
             Stage2DeferredHandoffTokenIsInjectiveObligation,
           IndexedHistoricalTransport(initialContext)!
             Stage2SelectedDifferentDeferredClassDropsDistance,
           IndexedHistoricalTransport(initialContext)!
             ReadyRunAuxOrderingIsWellFounded,
             IndexedHistoricalTransport(initialContext)!
               LocalAdmissionStrictlyDecreasesRuntimeReach,
             IndexedHistoricalTransport(initialContext)!
               SerializedLocalPredecessorStrictlyDecreasesRuntimeReach,
             IndexedHistoricalTransport(initialContext)!
               IngressDrainStrictlyDecreasesRuntimeReach,
           HeadTailProperties, NatLessThanWellFounded,
           IsWellFoundedOnSubset, IsaT(1200), PTL
           DEF IndexedHistoricalTemporalStage2ExactIdleRetryPending,
               IndexedHistoricalTemporalStage2ExactIdleRetrySelected,
               IndexedHistoricalTemporalStage2HandoffProgressExit,
               IndexedHistoricalTemporalStage2Owned,
               IndexedHistoricalTemporalStage2BusyTerminationGoal,
               IndexedHistoricalTemporalCandidateRunnerPending,
               IndexedRunHistoricalRecoveryStep,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage2ExactIdleRetryPending,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage2ExactIdleRetrySelected,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage2HandoffProgressExit,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage2IdleHandoffAwaitingRearm,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage2IdleHandoffAtDistance,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage2IdleHandoffCursorProgress,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage2ForeignIdleSkip,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage2BusyTerminationGoal,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage2Owned,
               IndexedHistoricalTransport!
                 HistoricalProtectedServiceOwnershipExit,
               IndexedHistoricalTransport!
                 HistoricalProtectedCandidateOwned,
               IndexedHistoricalTransport!
                 Stage2DeferredHandoffOwned,
               IndexedHistoricalTransport!
                 Stage2ActiveDeferredHandoff,
               IndexedHistoricalTransport!
                 Stage2DeferredHandoffToken,
               IndexedHistoricalTransport!Stage2HandoffCursorDistance,
               IndexedHistoricalTransport!DeferredHandoffActive,
               IndexedHistoricalTransport!DeferredHandoffQueueHead,
               IndexedHistoricalTransport!DeferredHandoffMatches,
               IndexedHistoricalTransport!
                 DeferredHandoffAllowsExecution,
               IndexedHistoricalTransport!
                 DeferredHandoffBlocksExecution,
               IndexedHistoricalTransport!DeferredDrainStep,
               IndexedHistoricalTransport!NextDeferredCommand,
               IndexedHistoricalTransport!ReadyRunAuxRank,
               IndexedHistoricalTransport!ReadyRunAuxOrdering,
               IndexedHistoricalTransport!ReadyRunAuxCarrier,
               IndexedHistoricalTransport!
                 PostGstRunHistoricalRecoveryNode,
               IndexedHistoricalTransport!RunHistoricalRecoveryNode,
               IndexedHistoricalTransport!RunNodeWork,
               IndexedHistoricalTransport!LocalAdmissionStep,
               IndexedHistoricalTransport!IngressDrainStep,
               IndexedHistoricalTransport!SerializedRunnerRuntimeStep,
               IndexedHistoricalTransport!SerializedRuntimeStep,
               IndexedHistoricalTransport!
                 SerializedRuntimePrecedesServeIngressStep,
               IndexedHistoricalTransport!
                 SerializedLocalPrecedesServeIngressStep,
               IndexedHistoricalTransport!AsyncServeIngressTargetOnlyTurn,
               IndexedHistoricalTransport!SelectedLocalAdmissionAdvance,
               IndexedHistoricalTransport!AsyncNext,
               IndexedHistoricalTransport!AsyncAllVars
      <3> QED BY <3>3
    <2>7. CASE candidate.node \notin Responsive
      <3>1. []~IndexedHistoricalTemporalStage2ExactIdleRetryPending(
                    initialContext, candidate)
        BY <2>7, PTL
           DEF IndexedHistoricalTemporalStage2ExactIdleRetryPending,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage2ExactIdleRetryPending,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage2Owned,
               IndexedHistoricalTransport!
                 HistoricalProtectedCandidateOwned
      <3> QED BY <3>1, PTL
    <2> QED BY <2>6, <2>7
  <1> QED BY <1>1

IndexedHistoricalTemporalStage2RankProgressExit(
    initialContext, candidate, position) ==
  IndexedHistoricalTemporalRankProgressExit(
    initialContext, candidate, <<2, position>>)

IndexedHistoricalTemporalStage2HandoffRankBlocked(
    initialContext, candidate, position) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalTemporalStage2HandoffRankBlocked(candidate, position)

IndexedHistoricalTemporalStage2RankOrHandoffProgress(
    initialContext, candidate, position) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalTemporalStage2RankOrHandoffProgress(
      candidate, position)

THEOREM IndexedChainSpecReachesHistoricalStage2ExitOrHandoff ==
  /\ IndexedChainSpec
  /\ IndexedHistoricalFiniteRunnerEpisodeClosureProperty
    => \A initialContext \in AdmissibleContextRecords,
          candidate, position:
         IndexedHistoricalTransport(initialContext)!
           HistoricalProtectedOwnedAtServiceRank(
             candidate, <<2, position>>)
           ~> IndexedHistoricalTemporalStage2RankOrHandoffProgress(
                initialContext, candidate, position)
PROOF
  <1>1. ASSUME IndexedChainSpec,
                IndexedHistoricalFiniteRunnerEpisodeClosureProperty,
                NEW initialContext \in AdmissibleContextRecords,
                NEW candidate, NEW position
         PROVE IndexedHistoricalTransport(initialContext)!
                 HistoricalProtectedOwnedAtServiceRank(
                   candidate, <<2, position>>)
                 ~>
               IndexedHistoricalTemporalStage2RankOrHandoffProgress(
                 initialContext, candidate, position)
    <2>1. []IndexedHistoricalTemporalSupportAt(initialContext)
      BY <1>1, IndexedChainSpecAlwaysHistoricalTemporalSupport, PTL
    <2>2. []IndexedCompositionInvariant
      BY <1>1, IndexedChainSpecEstablishesCompositionInvariant
    <2>3. [][IndexedChainNext]_IndexedChainVars
      BY <1>1 DEF IndexedChainSpec
    <2>4. (IndexedHistoricalTemporalStage2Owned(
             initialContext, candidate)
             /\ ~IndexedHistoricalTransport(initialContext)!
                   NodeIdle(candidate.node))
             ~> IndexedHistoricalTemporalStage2BusyTerminationGoal(
                  initialContext, candidate)
      BY <1>1,
         IndexedChainSpecTerminatesHistoricalStage2Busy
    <2>5. IndexedHistoricalTemporalStage2ExactIdleRetryPending(
             initialContext, candidate)
             ~> IndexedHistoricalTemporalStage2HandoffProgressExit(
                  initialContext, candidate)
      BY <1>1,
         IndexedChainSpecExitsHistoricalStage2IdleHandoff
    <2>6. CASE candidate.node \in Responsive
      <3>1. WF_IndexedChainVars(
               IndexedRunHistoricalRecoveryStep(
                 initialContext, candidate.node))
        BY <1>1, <2>6
           DEF IndexedChainSpec, IndexedFairness
      <3>2. IndexedHistoricalTransport(initialContext)!
               HistoricalProtectedOwnedAtServiceRank(
                 candidate, <<2, position>>)
               ~> IndexedHistoricalTemporalStage2RankOrHandoffProgress(
                    initialContext, candidate, position)
        BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>5,
           <2>6, <3>1,
           IndexedHistoricalCandidateRunnerEnablesFairOccurrence,
           IndexedBracketStepProjectsEveryHistoricalTransportStep,
           IndexedHistoricalTransport(initialContext)!
             HistoricalTemporalStage2BusyRetryClaimsHandoffAction,
           IndexedHistoricalTransport(initialContext)!
             HistoricalTemporalStage2ForeignIdleSkipDropsDistance,
           IndexedHistoricalTransport(initialContext)!
             Stage2DeferredHandoffTokenIsInjectiveObligation,
           IndexedHistoricalTransport(initialContext)!
             Stage2SelectedDifferentDeferredClassDropsDistance,
           IndexedHistoricalTransport(initialContext)!
             ReadyRunAuxOrderingIsWellFounded,
           IndexedHistoricalTransport(initialContext)!
             ReadyRunAuxRankInCarrier,
             IndexedHistoricalTransport(initialContext)!
               LocalAdmissionStrictlyDecreasesRuntimeReach,
             IndexedHistoricalTransport(initialContext)!
               SerializedLocalPredecessorStrictlyDecreasesRuntimeReach,
             IndexedHistoricalTransport(initialContext)!
               IngressDrainStrictlyDecreasesRuntimeReach,
           HeadTailProperties, FS_CardinalityType,
           IsaT(1200), PTL
           DEF IndexedHistoricalTemporalStage2RankOrHandoffProgress,
               IndexedHistoricalTemporalStage2RankProgressExit,
               IndexedHistoricalTemporalStage2HandoffRankBlocked,
               IndexedHistoricalTemporalStage2ExactIdleRetryPending,
               IndexedHistoricalTemporalStage2HandoffProgressExit,
               IndexedHistoricalTemporalStage2BusyTerminationGoal,
               IndexedHistoricalTemporalStage2Owned,
               IndexedHistoricalTemporalCandidateRunnerPending,
               IndexedRunHistoricalRecoveryStep,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage2RankOrHandoffProgress,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage2RankProgressExit,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage2HandoffRankBlocked,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage2BusyRejectedSelected,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage2BusyRetryClaimsHandoff,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage2BusyTerminationGoal,
               IndexedHistoricalTransport!
                 HistoricalTemporalStage2Owned,
               IndexedHistoricalTransport!
                 HistoricalProtectedOwnedAtServiceRank,
               IndexedHistoricalTransport!
                 HistoricalProtectedServiceOwnershipExit,
               IndexedHistoricalTransport!
                 HistoricalProtectedCandidateOwned,
               IndexedHistoricalTransport!CandidateServiceRank,
               IndexedHistoricalTransport!ServiceRankLess,
               IndexedHistoricalTransport!Stage2DeferredHandoffOwned,
               IndexedHistoricalTransport!Stage2ActiveDeferredHandoff,
               IndexedHistoricalTransport!Stage2DeferredHandoffToken,
               IndexedHistoricalTransport!DeferredHandoffActive,
               IndexedHistoricalTransport!DeferredHandoffMatches,
               IndexedHistoricalTransport!DeferredHandoffQueueHead,
               IndexedHistoricalTransport!DeferredHandoffCandidate,
               IndexedHistoricalTransport!
                 DeferredHandoffAllowsExecution,
               IndexedHistoricalTransport!
                 DeferredHandoffBlocksExecution,
               IndexedHistoricalTransport!DeferredDrainStep,
               IndexedHistoricalTransport!
                 PostGstRunHistoricalRecoveryNode,
               IndexedHistoricalTransport!RunHistoricalRecoveryNode,
               IndexedHistoricalTransport!RunNodeWork,
               IndexedHistoricalTransport!LocalAdmissionStep,
               IndexedHistoricalTransport!IngressDrainStep,
               IndexedHistoricalTransport!SerializedRunnerRuntimeStep,
               IndexedHistoricalTransport!SerializedRuntimeStep,
               IndexedHistoricalTransport!
                 SerializedRuntimePrecedesServeIngressStep,
               IndexedHistoricalTransport!
                 SerializedLocalPrecedesServeIngressStep,
               IndexedHistoricalTransport!AsyncServeIngressTargetOnlyTurn,
               IndexedHistoricalTransport!SelectedLocalAdmissionAdvance,
               IndexedHistoricalTransport!AsyncNext,
               IndexedHistoricalTransport!AsyncAllVars
      <3> QED BY <3>2
    <2>7. CASE candidate.node \notin Responsive
      <3>1. []~IndexedHistoricalTransport(initialContext)!
                    HistoricalProtectedOwnedAtServiceRank(
                      candidate, <<2, position>>)
        BY <2>7, PTL
           DEF IndexedHistoricalTransport!
                 HistoricalProtectedOwnedAtServiceRank,
               IndexedHistoricalTransport!
                 HistoricalProtectedCandidateOwned
      <3> QED BY <3>1, PTL
    <2> QED BY <2>6, <2>7
  <1> QED BY <1>1

THEOREM IndexedChainSpecExitsHistoricalStage2HandoffRank ==
  /\ IndexedChainSpec
  /\ IndexedHistoricalFiniteRunnerEpisodeClosureProperty
    => \A initialContext \in AdmissibleContextRecords,
          candidate, position:
         IndexedHistoricalTemporalStage2HandoffRankBlocked(
           initialContext, candidate, position)
           ~> IndexedHistoricalTemporalStage2RankProgressExit(
                initialContext, candidate, position)
PROOF
  <1>1. ASSUME IndexedChainSpec,
                IndexedHistoricalFiniteRunnerEpisodeClosureProperty,
                NEW initialContext \in AdmissibleContextRecords,
                NEW candidate, NEW position
         PROVE IndexedHistoricalTemporalStage2HandoffRankBlocked(
                 initialContext, candidate, position)
                 ~>
               IndexedHistoricalTemporalStage2RankProgressExit(
                 initialContext, candidate, position)
    <2>1. []IndexedHistoricalTemporalSupportAt(initialContext)
      BY <1>1, IndexedChainSpecAlwaysHistoricalTemporalSupport, PTL
    <2>2. [][IndexedChainNext]_IndexedChainVars
      BY <1>1 DEF IndexedChainSpec
    <2>3. (IndexedHistoricalTemporalStage2Owned(
             initialContext, candidate)
             /\ ~IndexedHistoricalTransport(initialContext)!
                   NodeIdle(candidate.node))
             ~> IndexedHistoricalTemporalStage2BusyTerminationGoal(
                  initialContext, candidate)
      BY <1>1,
         IndexedChainSpecTerminatesHistoricalStage2Busy
    <2>4. IndexedHistoricalTemporalStage2ExactIdleRetryPending(
             initialContext, candidate)
             ~> IndexedHistoricalTemporalStage2HandoffProgressExit(
                  initialContext, candidate)
      BY <1>1,
         IndexedChainSpecExitsHistoricalStage2IdleHandoff
    <2>5. IndexedHistoricalTemporalStage2HandoffRankBlocked(
             initialContext, candidate, position)
             ~> IndexedHistoricalTemporalStage2RankProgressExit(
                  initialContext, candidate, position)
      BY <1>1, <2>1, <2>2, <2>3, <2>4,
         IndexedBracketStepProjectsEveryHistoricalTransportStep,
         IndexedHistoricalTransport(initialContext)!
           Stage2DeferredHandoffTokenIsInjectiveObligation,
         HeadTailProperties, IsaT(1200), PTL
         DEF IndexedHistoricalTemporalStage2HandoffRankBlocked,
             IndexedHistoricalTemporalStage2RankProgressExit,
             IndexedHistoricalTemporalStage2HandoffProgressExit,
             IndexedHistoricalTemporalStage2ExactIdleRetryPending,
             IndexedHistoricalTemporalStage2Owned,
             IndexedHistoricalTemporalStage2BusyTerminationGoal,
             IndexedHistoricalTransport!
               HistoricalTemporalStage2HandoffRankBlocked,
             IndexedHistoricalTransport!
               HistoricalTemporalStage2RankProgressExit,
             IndexedHistoricalTransport!
               HistoricalTemporalRankProgressExit,
             IndexedHistoricalTransport!
               HistoricalTemporalStage2HandoffProgressExit,
             IndexedHistoricalTransport!
               HistoricalTemporalStage2ExactIdleRetryPending,
             IndexedHistoricalTransport!Stage2DeferredHandoffOwned,
             IndexedHistoricalTransport!Stage2ActiveDeferredHandoff,
             IndexedHistoricalTransport!Stage2DeferredHandoffToken,
             IndexedHistoricalTransport!
               HistoricalTemporalStage2Owned,
             IndexedHistoricalTransport!
               HistoricalTemporalStage2BusyTerminationGoal,
             IndexedHistoricalTransport!
               HistoricalProtectedServiceOwnershipExit,
             IndexedHistoricalTransport!
               HistoricalProtectedCandidateOwned,
             IndexedHistoricalTransport!CandidateServiceRank,
             IndexedHistoricalTransport!ServiceRankLess,
             IndexedHistoricalTransport!DeferredHandoffActive,
             IndexedHistoricalTransport!DeferredHandoffCandidate,
             IndexedHistoricalTransport!DeferredHandoffQueueHead,
             IndexedHistoricalTransport!DeferredHandoffMatches,
             IndexedHistoricalTransport!DeferredHandoffAllowsExecution,
             IndexedHistoricalTransport!DeferredHandoffBlocksExecution,
             IndexedHistoricalTransport!RemoveNextDeferredCommand,
             IndexedHistoricalTransport!ClearDeferredHandoff,
             IndexedHistoricalTransport!RetainDeferredHandoffs,
             IndexedHistoricalTransport!DeferredDrainStep,
             IndexedHistoricalTransport!RunHistoricalRecoveryNode,
             IndexedHistoricalTransport!RunNodeWork,
             IndexedHistoricalTransport!LocalAdmissionStep,
             IndexedHistoricalTransport!IngressDrainStep,
             IndexedHistoricalTransport!SerializedRunnerRuntimeStep,
             IndexedHistoricalTransport!SerializedRuntimeStep,
             IndexedHistoricalTransport!
               SerializedRuntimePrecedesServeIngressStep,
             IndexedHistoricalTransport!
               SerializedLocalPrecedesServeIngressStep,
             IndexedHistoricalTransport!AsyncServeIngressTargetOnlyTurn,
             IndexedHistoricalTransport!SelectedLocalAdmissionAdvance,
             IndexedHistoricalTransport!AsyncNext,
             IndexedHistoricalTransport!AsyncAllVars
    <2> QED BY <2>5
  <1> QED BY <1>1

IndexedHistoricalTemporalStage2Source(
    initialContext, candidate, position) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalProtectedOwnedAtServiceRank(
      candidate, <<2, position>>)

IndexedHistoricalTemporalStage2Goal(
    initialContext, candidate, position) ==
  \/ IndexedHistoricalTransport(initialContext)!
       HistoricalProtectedServiceOwnershipExit(candidate)
  \/ \E lower \in SetLessThan(
       <<2, position>>,
       IndexedHistoricalTransport(initialContext)!
         OwnedServiceRankOrdering,
       IndexedHistoricalTransport(initialContext)!
         OwnedServiceRankCarrier):
       IndexedHistoricalTransport(initialContext)!
         HistoricalProtectedOwnedAtServiceRank(candidate, lower)

IndexedHistoricalTemporalStage2LeafProperty ==
  \A initialContext \in AdmissibleContextRecords,
     candidate \in IndexedHistoricalTransport(initialContext)!
                    AsyncCandidateSet,
     position \in Nat:
    IndexedHistoricalTemporalStage2Source(
      initialContext, candidate, position)
      ~> IndexedHistoricalTemporalStage2Goal(
           initialContext, candidate, position)

THEOREM IndexedChainSpecClosesHistoricalTemporalStage2Leaf ==
  /\ IndexedChainSpec
  /\ IndexedHistoricalFiniteRunnerEpisodeClosureProperty
    => IndexedHistoricalTemporalStage2LeafProperty
PROOF
  <1>1. ASSUME IndexedChainSpec,
                IndexedHistoricalFiniteRunnerEpisodeClosureProperty,
                NEW initialContext \in AdmissibleContextRecords,
                NEW candidate \in
                  IndexedHistoricalTransport(initialContext)!
                    AsyncCandidateSet,
                NEW position \in Nat
         PROVE IndexedHistoricalTemporalStage2Source(
                 initialContext, candidate, position)
                 ~>
               IndexedHistoricalTemporalStage2Goal(
                 initialContext, candidate, position)
    <2>1. IndexedHistoricalTemporalStage2Source(
             initialContext, candidate, position)
             ~> IndexedHistoricalTemporalStage2RankOrHandoffProgress(
                  initialContext, candidate, position)
      BY <1>1,
         IndexedChainSpecReachesHistoricalStage2ExitOrHandoff
         DEF IndexedHistoricalTemporalStage2Source
    <2>2. IndexedHistoricalTemporalStage2HandoffRankBlocked(
             initialContext, candidate, position)
             ~> IndexedHistoricalTemporalStage2RankProgressExit(
                  initialContext, candidate, position)
      BY <1>1,
         IndexedChainSpecExitsHistoricalStage2HandoffRank
    <2>3. IndexedHistoricalTemporalStage2Source(
             initialContext, candidate, position)
             ~> IndexedHistoricalTemporalStage2RankProgressExit(
                  initialContext, candidate, position)
      BY <2>1, <2>2, PTL
         DEF IndexedHistoricalTemporalStage2RankOrHandoffProgress,
             IndexedHistoricalTransport!
               HistoricalTemporalStage2RankOrHandoffProgress
    <2>4. []IndexedHistoricalTemporalSupportAt(initialContext)
      BY <1>1, IndexedChainSpecAlwaysHistoricalTemporalSupport, PTL
    <2>5. IndexedHistoricalTemporalSupportAt(initialContext)
             /\ IndexedHistoricalTemporalStage2RankProgressExit(
                  initialContext, candidate, position)
             => IndexedHistoricalTemporalStage2Goal(
                  initialContext, candidate, position)
      BY <1>1,
         IndexedHistoricalTransport(initialContext)!
           HistoricalTemporalRankExitHasWellFoundedSuccessor, Isa
         DEF IndexedHistoricalTemporalSupportAt,
             IndexedHistoricalTemporalStage2RankProgressExit,
             IndexedHistoricalTemporalRankProgressExit,
             IndexedHistoricalTemporalStage2Goal,
             IndexedHistoricalTransport!
               HistoricalTemporalRankProgressExit,
             IndexedHistoricalTransport!
               HistoricalProtectedServiceOwnershipExit,
             IndexedHistoricalTransport!OwnedServiceRankCarrier
    <2>6. IndexedHistoricalTemporalStage2RankProgressExit(
             initialContext, candidate, position)
             ~> IndexedHistoricalTemporalStage2Goal(
                  initialContext, candidate, position)
      BY <2>4, <2>5, PTL
    <2> QED BY <2>3, <2>6, PTL
  <1> QED BY <1>1
       DEF IndexedHistoricalTemporalStage2LeafProperty

IndexedHistoricalTemporalCandidateStageLeafProperties ==
  /\ IndexedHistoricalTemporalStage2LeafProperty
  /\ IndexedHistoricalTemporalStage3LeafProperty
  /\ IndexedHistoricalTemporalStage4LeafProperty
  /\ IndexedHistoricalTemporalStage5LeafProperty
  /\ IndexedHistoricalTemporalStage6LeafProperty

THEOREM IndexedChainSpecClosesAllHistoricalTemporalCandidateStageLeaves ==
  /\ IndexedChainSpec
  /\ IndexedHistoricalFiniteRunnerEpisodeClosureProperty
    => IndexedHistoricalTemporalCandidateStageLeafProperties
BY IndexedChainSpecClosesHistoricalTemporalStage2Leaf,
   IndexedChainSpecClosesHistoricalTemporalStage3Leaf,
   IndexedChainSpecClosesHistoricalTemporalStage4Leaf,
   IndexedChainSpecClosesHistoricalTemporalStage5Leaf,
   IndexedChainSpecClosesHistoricalTemporalStage6Leaf
   DEF IndexedHistoricalFiniteRunnerEpisodeClosureProperty,
       IndexedHistoricalTemporalCandidateStageLeafProperties

IndexedHistoricalProtectedServiceRankLeafProperties ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedHistoricalTransport(initialContext)!
      HistoricalProtectedServiceRankLeafProperties(IndexedChainSpec)

THEOREM IndexedChainSpecClosesHistoricalProtectedServiceRankLeaves ==
  /\ IndexedChainSpec
  /\ IndexedHistoricalFiniteRunnerEpisodeClosureProperty
    => IndexedHistoricalProtectedServiceRankLeafProperties
PROOF
  <1>1. ASSUME IndexedChainSpec,
                IndexedHistoricalFiniteRunnerEpisodeClosureProperty,
                NEW initialContext \in AdmissibleContextRecords
         PROVE IndexedHistoricalTransport(initialContext)!
                 HistoricalProtectedServiceRankLeafProperties(
                   IndexedChainSpec)
    <2>1. IndexedHistoricalTemporalCandidateStageLeafProperties
      BY <1>1,
         IndexedChainSpecClosesAllHistoricalTemporalCandidateStageLeaves
         DEF IndexedHistoricalFiniteRunnerEpisodeClosureProperty
    <2> QED BY <1>1, <2>1
         DEF IndexedHistoricalProtectedServiceRankLeafProperties,
             IndexedHistoricalTemporalCandidateStageLeafProperties,
             IndexedHistoricalTemporalStage2LeafProperty,
             IndexedHistoricalTemporalStage3LeafProperty,
             IndexedHistoricalTemporalStage4LeafProperty,
             IndexedHistoricalTemporalStage5LeafProperty,
             IndexedHistoricalTemporalStage6LeafProperty,
             IndexedHistoricalTemporalStage2Source,
             IndexedHistoricalTemporalStage2Goal,
             IndexedHistoricalTemporalStage3Source,
             IndexedHistoricalTemporalStage3Goal,
             IndexedHistoricalTemporalStage4Source,
             IndexedHistoricalTemporalStage4Goal,
             IndexedHistoricalTemporalStage5Source,
             IndexedHistoricalTemporalStage5Goal,
             IndexedHistoricalTemporalStage6Source,
             IndexedHistoricalTemporalStage6Goal,
             IndexedHistoricalTransport!
               HistoricalProtectedServiceRankLeafProperties,
             IndexedHistoricalTransport!
               HistoricalProtectedStage2RankProgressProperty,
             IndexedHistoricalTransport!
               HistoricalProtectedStage3RankProgressProperty,
             IndexedHistoricalTransport!
               HistoricalProtectedStage4RankProgressProperty,
             IndexedHistoricalTransport!
               HistoricalProtectedStage5RankProgressProperty,
             IndexedHistoricalTransport!
               HistoricalProtectedStage6RankProgressProperty,
             IndexedHistoricalTransport!
               HistoricalProtectedStageRankProgressProperty,
             IndexedHistoricalTransport!
               HistoricalProtectedOwnedAtServiceRank
  <1> QED BY <1>1
       DEF IndexedHistoricalProtectedServiceRankLeafProperties

IndexedHistoricalProtectedCandidateStarvationProperties ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedHistoricalTransport(initialContext)!
      HistoricalProtectedCandidateStarvationProperty(IndexedChainSpec)

THEOREM IndexedChainSpecClosesHistoricalProtectedCandidateStarvation ==
  /\ IndexedChainSpec
  /\ IndexedHistoricalFiniteRunnerEpisodeClosureProperty
    => IndexedHistoricalProtectedCandidateStarvationProperties
PROOF
  <1>1. ASSUME IndexedChainSpec,
                IndexedHistoricalFiniteRunnerEpisodeClosureProperty,
                NEW initialContext \in AdmissibleContextRecords
         PROVE IndexedHistoricalTransport(initialContext)!
                 HistoricalProtectedCandidateStarvationProperty(
                   IndexedChainSpec)
    <2>1. IndexedHistoricalTransport(initialContext)!
             HistoricalProtectedServiceRankLeafProperties(
               IndexedChainSpec)
      BY <1>1,
         IndexedChainSpecClosesHistoricalProtectedServiceRankLeaves
         DEF IndexedHistoricalFiniteRunnerEpisodeClosureProperty,
             IndexedHistoricalProtectedServiceRankLeafProperties
    <2>2. IndexedHistoricalTransport(initialContext)!
             HistoricalProtectedServiceRankProgressProperty(
               IndexedChainSpec)
      BY <2>1,
         IndexedHistoricalTransport(initialContext)!
           HistoricalProtectedServiceRankProgressFromStageLeaves
    <2> QED BY <2>2,
         IndexedHistoricalTransport(initialContext)!
           HistoricalProtectedServiceRankProgressImpliesStarvation
  <1> QED BY <1>1
       DEF IndexedHistoricalProtectedCandidateStarvationProperties

IndexedHistoricalDecisionCandidateProgressLeaves ==
  \A initialContext \in AdmissibleContextRecords:
    /\ IndexedHistoricalTransport(initialContext)!
         HistoricalCommitDeliveryProgressLeaf(IndexedChainSpec)
    /\ IndexedHistoricalTransport(initialContext)!
         HistoricalBeginDecisionProgressLeaf(IndexedChainSpec)
    /\ IndexedHistoricalTransport(initialContext)!
         HistoricalPersistDecisionProgressLeaf(IndexedChainSpec)

THEOREM IndexedChainSpecClosesHistoricalDecisionCandidateProgressLeaves ==
  IndexedChainSpec
    => IndexedHistoricalDecisionCandidateProgressLeaves
PROOF
  <1>1. ASSUME IndexedChainSpec,
                NEW initialContext \in AdmissibleContextRecords
         PROVE
           /\ IndexedHistoricalTransport(initialContext)!
                HistoricalCommitDeliveryProgressLeaf(IndexedChainSpec)
           /\ IndexedHistoricalTransport(initialContext)!
                HistoricalBeginDecisionProgressLeaf(IndexedChainSpec)
           /\ IndexedHistoricalTransport(initialContext)!
                HistoricalPersistDecisionProgressLeaf(IndexedChainSpec)
    <2>1. []IndexedHistoricalTemporalSupportAt(initialContext)
      BY <1>1, IndexedChainSpecAlwaysHistoricalTemporalSupport, PTL
    <2>2. [](IndexedHistoricalTransport(initialContext)!gst
                  => []IndexedHistoricalTransport(initialContext)!gst)
      BY <1>1, IndexedChainSpecKeepsHistoricalTransportGstOnceSet
    <2>3. [][IndexedChainNext]_IndexedChainVars
      BY <1>1 DEF IndexedChainSpec
    <2>4. [][IndexedHistoricalTransport(initialContext)!AsyncNext]_(
                   IndexedHistoricalTransport(initialContext)!AsyncAllVars)
      BY <2>3,
         IndexedBracketStepProjectsEveryHistoricalTransportStep,
         PTL
    <2>5. IndexedHistoricalTransport(initialContext)!
             HistoricalCommitDecisionTailTemporalSupportProperty(
               IndexedChainSpec)
      BY <1>1, <2>1, <2>2, <2>4
         DEF IndexedHistoricalTemporalSupportAt,
             IndexedHistoricalTransport!
               HistoricalCommitDecisionTailTemporalSupportProperty,
             IndexedHistoricalTransport!
               HistoricalTemporalIdentityLifecycleInvariant
    <2> QED BY <2>5,
         IndexedHistoricalTransport(initialContext)!
           HistoricalCommitDecisionExactCarrierTailClosesProgressLeaves
  <1> QED BY <1>1
       DEF IndexedHistoricalDecisionCandidateProgressLeaves

IndexedHistoricalDecisionBodyCandidateProgressLeaves ==
  \A initialContext \in AdmissibleContextRecords:
    /\ IndexedHistoricalTransport(initialContext)!
         HistoricalDecisionFetchProgressLeaf(IndexedChainSpec)
    /\ IndexedHistoricalTransport(initialContext)!
         HistoricalDecisionRequestBodyProgressLeaf(IndexedChainSpec)
    /\ IndexedHistoricalTransport(initialContext)!
         HistoricalDecisionFetchCertifiedProgressLeaf(IndexedChainSpec)
    /\ IndexedHistoricalTransport(initialContext)!
         HistoricalDecisionStoreProgressLeaf(IndexedChainSpec)
    /\ IndexedHistoricalTransport(initialContext)!
         HistoricalDecisionValidateProgressLeaf(IndexedChainSpec)
    /\ IndexedHistoricalTransport(initialContext)!
         HistoricalDecisionApplyProgressLeaf(IndexedChainSpec)

THEOREM IndexedChainSpecClosesHistoricalDecisionBodyCandidateLeaves ==
  IndexedChainSpec
    => IndexedHistoricalDecisionBodyCandidateProgressLeaves
PROOF
  <1>1. ASSUME IndexedChainSpec,
                NEW initialContext \in AdmissibleContextRecords
         PROVE
           /\ IndexedHistoricalTransport(initialContext)!
                HistoricalDecisionFetchProgressLeaf(IndexedChainSpec)
           /\ IndexedHistoricalTransport(initialContext)!
                HistoricalDecisionRequestBodyProgressLeaf(IndexedChainSpec)
           /\ IndexedHistoricalTransport(initialContext)!
                HistoricalDecisionFetchCertifiedProgressLeaf(
                  IndexedChainSpec)
           /\ IndexedHistoricalTransport(initialContext)!
                HistoricalDecisionStoreProgressLeaf(IndexedChainSpec)
           /\ IndexedHistoricalTransport(initialContext)!
                HistoricalDecisionValidateProgressLeaf(IndexedChainSpec)
           /\ IndexedHistoricalTransport(initialContext)!
                HistoricalDecisionApplyProgressLeaf(IndexedChainSpec)
    <2>1. []IndexedHistoricalTemporalSupportAt(initialContext)
      BY <1>1, IndexedChainSpecAlwaysHistoricalTemporalSupport, PTL
    <2>2. [](IndexedHistoricalTransport(initialContext)!gst
                  => []IndexedHistoricalTransport(initialContext)!gst)
      BY <1>1, IndexedChainSpecKeepsHistoricalTransportGstOnceSet
    <2>3. [][IndexedChainNext]_IndexedChainVars
      BY <1>1 DEF IndexedChainSpec
    <2>4. [][IndexedHistoricalTransport(initialContext)!AsyncNext]_(
                   IndexedHistoricalTransport(initialContext)!AsyncAllVars)
      BY <2>3,
         IndexedBracketStepProjectsEveryHistoricalTransportStep,
         PTL
    <2>5. IndexedHistoricalTransport(initialContext)!
             HistoricalDecisionPipelineTemporalSupportProperty(
               IndexedChainSpec)
      BY <1>1, <2>1, <2>2, <2>4
         DEF IndexedHistoricalTemporalSupportAt,
             IndexedHistoricalTransport!
               HistoricalDecisionPipelineTemporalSupportProperty,
             IndexedHistoricalTransport!
               HistoricalTemporalIdentityLifecycleInvariant
    <2> QED BY <2>5,
         IndexedHistoricalTransport(initialContext)!
           HistoricalDecisionPipelineExactCarrierClosesBodyLeaves
  <1> QED BY <1>1
       DEF IndexedHistoricalDecisionBodyCandidateProgressLeaves

(***************************************************************************
Indexed route witness.

The helper invariant is intentionally stronger than the requested property:
an exact route is retained for every live historical target which has neither
a Decision nor an Application, whether or not that target has published its
Commit-certificate request yet.  Publication can therefore only expose an
already-retained route; it cannot create the missing archive identity.
***************************************************************************)

IndexedHistoricalCommitArchiveRouteAvailable(
    initialContext, target, server) ==
  IndexedHistoricalTransport(initialContext)!
    HistoricalCommitArchiveRouteAvailable(target, server)

IndexedHistoricalCommitArchiveRouteExit(initialContext, target) ==
  \/ IndexedHistoricalTransport(initialContext)!NodeHasDecision(target)
  \/ IndexedHistoricalTransport(initialContext)!NodeHasApplication(target)
  \/ ~IndexedHistoricalTransport(initialContext)!
        HistoricalRecoveryTarget(target)

IndexedHistoricalCommitArchiveRouteWitnessAt(initialContext) ==
  /\ IndexedHistoricalTransport(initialContext)!
       AsyncFrozenContextAt(initialContext)
  /\ IndexedHistoricalTransport(initialContext)!AsyncStrongTypeInvariant
  /\ \A target \in Responsive:
       /\ IndexedHistoricalTransport(initialContext)!
            HistoricalRecoveryTarget(target)
       /\ ~IndexedHistoricalTransport(initialContext)!
              NodeHasDecision(target)
       /\ ~IndexedHistoricalTransport(initialContext)!
              NodeHasApplication(target)
       => \E server \in ValidatorIds:
            IndexedHistoricalCommitArchiveRouteAvailable(
              initialContext, target, server)

IndexedHistoricalCommitArchiveRouteWitnessInvariant ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedHistoricalCommitArchiveRouteWitnessAt(initialContext)

IndexedHistoricalCommitArchiveRouteAvailabilityInvariant ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedHistoricalTransport(initialContext)!
      HistoricalCommitArchiveRouteAvailabilityInvariant

IndexedHistoricalCommitArchiveRouteAvailabilityProperty ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedHistoricalTransport(initialContext)!
      HistoricalCommitArchiveRouteAvailabilityProperty(IndexedChainSpec)

(***************************************************************************
The exact indexed source selects the intersection member.

The source is a current application owned by `server`, and the frozen
transport context is `initialContext`.  The target has no such application,
so `server # target`.  The remaining source guards put `server` in the current
responsive voter set and in `up`.  The server need not occur in the historical
QC signer set: that certificate binds the served subject, while this exact
applied archive owns the independently authenticated response route.
***************************************************************************)

THEOREM IndexedHistoricalSourceSelectsExactAppliedCurrentVoter ==
  \A initialContext \in AdmissibleContextRecords,
     target \in ValidatorIds,
     server \in ValidatorIds,
     source \in Chain!DecisionEvidenceSet:
    /\ IndexedHistoricalTransport(initialContext)!
         AsyncFrozenContextAt(initialContext)
    /\ IndexedHistoricalRecoveryTargetReady(initialContext, target)
    /\ IndexedHistoricalRecoverySourceReady(
         initialContext, server, source)
    => /\ server \in
              IndexedHistoricalTransport(initialContext)!CurrentVoters
                \ {target}
       /\ server \in
              IndexedHistoricalTransport(initialContext)!
                AsyncResponsiveAppliedArchiveServers
BY Isa
   DEF IndexedHistoricalRecoveryTargetReady,
       IndexedHistoricalRecoverySourceReady,
       IndexedCurrentApplications,
       IndexedProjectedNodeHasApplication,
       HistoricalRecoveryNodeHasApplicationProjection,
       IndexedHistoricalTransport!AsyncFrozenContextAt,
       IndexedHistoricalTransport!CurrentVoters,
       IndexedHistoricalTransport!CurrentEpoch,
       IndexedHistoricalTransport!AsyncResponsiveAppliedArchiveServers,
       IndexedHistoricalTransport!AsyncResponsiveOnlineArchiveServers,
       IndexedHistoricalTransport!AsyncResponsiveArchiveServers,
       IndexedHistoricalTransport!AsyncArchiveServerIds,
       IndexedHistoricalTransport!NodeHasApplication,
       IndexedAsync!AsyncCurrentResponsiveVoters,
       IndexedAsync!CurrentVoters, IndexedAsync!CurrentEpoch

THEOREM IndexedOpenHistoricalRecoveryEstablishesPostStateArchiveRoute ==
  \A initialContext \in AdmissibleContextRecords,
     target \in ValidatorIds,
     server \in ValidatorIds,
     source \in Chain!DecisionEvidenceSet:
    /\ IndexedHistoricalTransport(initialContext)!
         AsyncFrozenContextAt(initialContext)
    /\ IndexedOpenHistoricalRecovery(
         initialContext, target, server, source)
    => IndexedHistoricalCommitArchiveRouteAvailable(
         initialContext, target, server)'
BY IndexedHistoricalSourceSelectsExactAppliedCurrentVoter, Isa
   DEF IndexedOpenHistoricalRecovery,
       IndexedHistoricalCommitArchiveRouteAvailable,
       IndexedHistoricalTransport!
         HistoricalCommitArchiveRouteAvailable,
       IndexedHistoricalTransport!HistoricalRecoveryTarget,
       IndexedHistoricalTransport!AsyncResponsiveAppliedArchiveServers,
       IndexedHistoricalTransport!AsyncResponsiveOnlineArchiveServers,
       IndexedHistoricalTransport!AsyncResponsiveArchiveServers,
       IndexedHistoricalTransport!NodeHasApplication,
       IndexedHistoricalTransport!CurrentVoters,
       IndexedHistoricalTransport!CurrentEpoch,
       IndexedAsync!OpenHistoricalRecovery,
       IndexedAsync!HistoricalRecoveryTarget,
       IndexedAsync!AsyncSchedulerExceptHistoricalRecoveryTargets,
       IndexedAsync!AsyncAllVars,
       IndexedAsync!AsyncSchedulerVars,
       IndexedAsync!AsyncRecoveryVars,
       IndexedAsync!vars

(***************************************************************************
Route retention for an existing unresolved target.

Applications and Decisions are durable, GST is monotone, the one-height
context is frozen, and the strong type invariant makes every responsive node
up after GST.  Hence the chosen server remains both an applied archive and a
current voter.  The only excluded semantic case is the target acquiring its
Decision, after which the requested availability predicate is already
vacuous.
***************************************************************************)

THEOREM IndexedHistoricalArchiveRoutePersistsUntilTargetDecision ==
  \A initialContext \in AdmissibleContextRecords,
     target \in ValidatorIds,
     server \in ValidatorIds:
    /\ IndexedHistoricalTransport(initialContext)!
         AsyncFrozenContextAt(initialContext)
    /\ IndexedHistoricalTransport(initialContext)!AsyncStrongTypeInvariant
    /\ IndexedHistoricalCommitArchiveRouteAvailable(
         initialContext, target, server)
    /\ [IndexedChainNext]_IndexedChainVars
    /\ ~IndexedHistoricalTransport(initialContext)!
           NodeHasDecision(target)'
    => IndexedHistoricalCommitArchiveRouteAvailable(
         initialContext, target, server)'
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW target \in ValidatorIds,
                NEW server \in ValidatorIds,
                IndexedHistoricalTransport(initialContext)!
                  AsyncFrozenContextAt(initialContext),
                IndexedHistoricalTransport(initialContext)!
                  AsyncStrongTypeInvariant,
                IndexedHistoricalCommitArchiveRouteAvailable(
                  initialContext, target, server),
                [IndexedChainNext]_IndexedChainVars,
                ~IndexedHistoricalTransport(initialContext)!
                   NodeHasDecision(target)'
         PROVE IndexedHistoricalCommitArchiveRouteAvailable(
                 initialContext, target, server)'
    <2>1. [IndexedHistoricalTransport(initialContext)!AsyncNext]_(
             IndexedHistoricalTransport(initialContext)!AsyncAllVars)
      BY <1>1,
         IndexedBracketStepProjectsEveryHistoricalTransportStep
    <2>2. (IndexedHistoricalTransport(initialContext)!
             AsyncFrozenContextAt(initialContext))'
      BY <1>1, <2>1,
         IndexedHistoricalTransport(initialContext)!
           AsyncNextPreservesFrozenContext
    <2>3. (IndexedHistoricalTransport(initialContext)!
             AsyncStrongTypeInvariant)'
      BY <1>1, <2>1,
         IndexedHistoricalTransport(initialContext)!
           AsyncBracketNextPreservesStrongTypeInvariant
    <2>4. IndexedCore(initialContext, 7)'
      BY <1>1, <2>1,
         IndexedHistoricalTransport(initialContext)!
           GstAsyncStepIsMonotone
         DEF IndexedHistoricalCommitArchiveRouteAvailable,
             IndexedHistoricalTransport!
               HistoricalCommitArchiveRouteAvailable
    <2>5. (IndexedHistoricalTransport(initialContext)!
             HistoricalRecoveryTarget(target))'
      BY <1>1, <2>1,
         IndexedHistoricalTransport(initialContext)!
           HistoricalRecoveryTargetPersistsUnlessDecision
         DEF IndexedHistoricalCommitArchiveRouteAvailable,
             IndexedHistoricalTransport!
               HistoricalCommitArchiveRouteAvailable
    <2>6. (IndexedHistoricalTransport(initialContext)!
             NodeHasApplication(server))'
      BY <1>1, <2>1,
         IndexedHistoricalTransport(initialContext)!
           AsyncBracketStepPreservesNodeApplication
         DEF IndexedHistoricalCommitArchiveRouteAvailable,
             IndexedHistoricalTransport!
               HistoricalCommitArchiveRouteAvailable,
             IndexedHistoricalTransport!
               AsyncResponsiveAppliedArchiveServers,
             IndexedHistoricalTransport!
               AsyncResponsiveOnlineArchiveServers,
             IndexedHistoricalTransport!AsyncResponsiveArchiveServers
    <2>7. (Responsive \subseteq IndexedCore(initialContext, 6))'
      BY <2>3, <2>4,
         IndexedHistoricalTransport(initialContext)!
           GstResponsiveNodesAreUp
         DEF IndexedHistoricalTransport!AsyncStrongTypeInvariant
    <2> QED BY <1>1, <2>2, <2>4, <2>5, <2>6, <2>7, Isa
         DEF IndexedHistoricalCommitArchiveRouteAvailable,
             IndexedHistoricalTransport!
               HistoricalCommitArchiveRouteAvailable,
             IndexedHistoricalTransport!
               AsyncResponsiveAppliedArchiveServers,
             IndexedHistoricalTransport!
               AsyncResponsiveOnlineArchiveServers,
             IndexedHistoricalTransport!AsyncResponsiveArchiveServers,
             IndexedHistoricalTransport!AsyncArchiveServerIds,
             IndexedHistoricalTransport!AsyncFrozenContextAt,
             IndexedHistoricalTransport!CurrentVoters,
             IndexedHistoricalTransport!CurrentEpoch
  <1> QED BY <1>1

THEOREM IndexedHistoricalArchiveRoutePersistsUntilSemanticExit ==
  \A initialContext \in AdmissibleContextRecords,
     target \in ValidatorIds,
     server \in ValidatorIds:
    /\ IndexedHistoricalTransport(initialContext)!
         AsyncFrozenContextAt(initialContext)
    /\ IndexedHistoricalTransport(initialContext)!AsyncStrongTypeInvariant
    /\ IndexedHistoricalCommitArchiveRouteAvailable(
         initialContext, target, server)
    /\ [IndexedChainNext]_IndexedChainVars
    => \/ IndexedHistoricalCommitArchiveRouteAvailable(
             initialContext, target, server)'
       \/ IndexedHistoricalCommitArchiveRouteExit(
            initialContext, target)'
BY IndexedHistoricalArchiveRoutePersistsUntilTargetDecision, Isa
   DEF IndexedHistoricalCommitArchiveRouteExit

(***************************************************************************
Wrapper-only target creation.

All ordinary product actions preserve the historical-target set or remove a
target at Apply.  The sole addition is the indexed open disjunct, whose guard
retains the exact `server` and `source`.  This is the only action
classification which is specific to the chain wrapper.
***************************************************************************)

THEOREM IndexedNonOpenProductStepCannotCreateHistoricalTarget ==
  \A initialContext \in AdmissibleContextRecords,
     target \in ValidatorIds:
    /\ IndexedChainNext
    /\ ~IndexedHistoricalTransport(initialContext)!
           HistoricalRecoveryTarget(target)
    /\ ~(\E server \in ValidatorIds,
             source \in Chain!DecisionEvidenceSet:
          IndexedOpenHistoricalRecovery(
            initialContext, target, server, source))
    => ~IndexedHistoricalTransport(initialContext)!
          HistoricalRecoveryTarget(target)'
BY IndexedSuccessorActivationStepStuttersAsyncState, IsaT(600)
   DEF IndexedChainNext, IndexedProductActionAt,
       IndexedJoinedAsyncNext, IndexedJoinedNonCrashStep,
       IndexedJoinedRunnerStep, IndexedJoinedNonRunnerStep,
       IndexedOpenHistoricalRecovery,
       IndexedHistoricalTransport!HistoricalRecoveryTarget,
       IndexedAsync!HistoricalRecoveryTarget,
       IndexedAsync!AsyncNext, IndexedAsync!AsyncNonCrashStep,
       IndexedAsync!AsyncRunnerStep, IndexedAsync!AsyncNonRunnerStep,
       IndexedAsync!OpenHistoricalRecovery,
       IndexedAsync!HistoricalRecoverySourceReady,
       IndexedAsync!AsyncSetGST, IndexedAsync!AsyncTick,
       IndexedAsync!DirectCommitCertificateDiscoveryStep,
       IndexedAsync!DirectHistoricalCommitCertificateDiscoveryStep,
       IndexedAsync!ServiceIoWorker,
       IndexedAsync!ServiceHistoricalRecoveryIoWorker,
       IndexedAsync!EnqueueIoLocalControl,
       IndexedAsync!EnqueueHistoricalRecoveryIoLocalControl,
       IndexedAsync!AsyncNetworkStep, IndexedAsync!AsyncFaultStep,
       IndexedAsync!RunNode,
       IndexedAsync!RunHistoricalRecoveryNode,
       IndexedAsync!RunNodeWork,
       IndexedAsync!SerializedRunnerRuntimeStep,
       IndexedAsync!SerializedRuntimePrecedesServeIngressStep,
       IndexedAsync!SerializedLocalPrecedesServeIngressStep,
       IndexedAsync!AsyncServeIngressTargetOnlyTurn,
       IndexedAsync!SelectedLocalAdmissionAdvance,
       IndexedAsync!RunHistoricalServer,
       IndexedAsync!ServiceIoWorkerWork,
       IndexedAsync!EnqueueIoLocalControlWork,
       IndexedAsync!CommitCertificateDiscoveryStepWork,
       IndexedAsync!LocalAdmissionStep,
       IndexedAsync!IngressDrainStep,
       IndexedAsync!SerializedRuntimeStep,
       IndexedAsync!RuntimeStep,
       IndexedAsync!FifoRuntimeStep,
       IndexedAsync!ExecuteCommand,
       IndexedAsync!ExecuteApply,
       IndexedAsync!PreGstCrash,
       IndexedAsync!PreGstResponsiveCrash,
       IndexedAsync!PreGstResponsiveRestart,
       IndexedAsync!PreGstResponsiveReplay,
       IndexedAsync!ResetNodeSchedulerForRestart,
       IndexedAsync!AsyncHistoricalLockRestartAuthorityTransition,
       IndexedAsync!AsyncAllVars, IndexedAsync!AsyncSchedulerVars,
       IndexedAsync!AsyncRecoveryVars, IndexedAsync!vars,
       IndexedReceiptClassification,
       IndexedReceiptFreeChainStutter,
       IndexedDecisionReceiptHandoff,
       IndexedApplicationReceiptHandoff,
       IndexedSuccessorActivationProgressStep,
       SuccessorActivationEnvironmentStutter

THEOREM IndexedNewHistoricalTargetHasExactOpenSource ==
  \A initialContext \in AdmissibleContextRecords,
     target \in ValidatorIds:
    /\ [IndexedChainNext]_IndexedChainVars
    /\ ~IndexedHistoricalTransport(initialContext)!
           HistoricalRecoveryTarget(target)
    /\ IndexedHistoricalTransport(initialContext)!
         HistoricalRecoveryTarget(target)'
    => \E server \in ValidatorIds,
          source \in Chain!DecisionEvidenceSet:
         IndexedOpenHistoricalRecovery(
           initialContext, target, server, source)
BY IndexedNonOpenProductStepCannotCreateHistoricalTarget, Isa
   DEF IndexedChainVars,
       IndexedHistoricalTransport!HistoricalRecoveryTarget,
       IndexedHistoricalTransport!AsyncAllVars,
       IndexedHistoricalTransport!AsyncSchedulerVars,
       IndexedHistoricalTransport!AsyncRecoveryVars,
       IndexedHistoricalTransport!vars,
       IndexedCore, IndexedScheduler, IndexedRecovery

(***************************************************************************
Initialization and product preservation of the exact witness.
***************************************************************************)

THEOREM IndexedChainInitEstablishesHistoricalArchiveRouteWitness ==
  IndexedChainInit
    => IndexedHistoricalCommitArchiveRouteWitnessInvariant
PROOF
  <1>1. ASSUME IndexedChainInit,
               NEW initialContext \in AdmissibleContextRecords
         PROVE IndexedHistoricalCommitArchiveRouteWitnessAt(initialContext)
    <2>1. IndexedHistoricalTransport(initialContext)!
             AsyncInitAt(initialContext)
      BY <1>1, IndexedInitProjectsEveryHistoricalTransportInit
    <2>2. IndexedHistoricalTransport(initialContext)!
             AsyncFrozenContextAt(initialContext)
      BY <2>1,
         IndexedHistoricalTransport(initialContext)!
           AsyncInitEstablishesFrozenContext
    <2>3. IndexedHistoricalTransport(initialContext)!
             AsyncStrongTypeInvariant
      BY <2>1,
         IndexedHistoricalTransport(initialContext)!
           AsyncInitEstablishesStrongTypeInvariant
    <2>4. \A target \in Responsive:
             ~IndexedHistoricalTransport(initialContext)!
                HistoricalRecoveryTarget(target)
      BY <2>1, Isa
         DEF IndexedHistoricalTransport!AsyncInitAt,
             IndexedHistoricalTransport!AsyncBaseInitAt,
             IndexedHistoricalTransport!AsyncTransportInit,
             IndexedHistoricalTransport!HistoricalRecoveryTarget
    <2> QED BY <2>2, <2>3, <2>4
         DEF IndexedHistoricalCommitArchiveRouteWitnessAt
  <1> QED BY <1>1
       DEF IndexedHistoricalCommitArchiveRouteWitnessInvariant

THEOREM IndexedBracketStepPreservesHistoricalArchiveRouteWitness ==
  /\ IndexedHistoricalCommitArchiveRouteWitnessInvariant
  /\ [IndexedChainNext]_IndexedChainVars
  => IndexedHistoricalCommitArchiveRouteWitnessInvariant'
PROOF
  <1>1. ASSUME IndexedHistoricalCommitArchiveRouteWitnessInvariant,
              [IndexedChainNext]_IndexedChainVars,
              NEW initialContext \in AdmissibleContextRecords
         PROVE
           IndexedHistoricalCommitArchiveRouteWitnessAt(initialContext)'
    <2>1. IndexedHistoricalCommitArchiveRouteWitnessAt(initialContext)
      BY <1>1
         DEF IndexedHistoricalCommitArchiveRouteWitnessInvariant
    <2>2. [IndexedHistoricalTransport(initialContext)!AsyncNext]_(
             IndexedHistoricalTransport(initialContext)!AsyncAllVars)
      BY <1>1,
         IndexedBracketStepProjectsEveryHistoricalTransportStep
    <2>3. (IndexedHistoricalTransport(initialContext)!
             AsyncFrozenContextAt(initialContext))'
      BY <2>1, <2>2,
         IndexedHistoricalTransport(initialContext)!
           AsyncNextPreservesFrozenContext
         DEF IndexedHistoricalCommitArchiveRouteWitnessAt
    <2>4. (IndexedHistoricalTransport(initialContext)!
             AsyncStrongTypeInvariant)'
      BY <2>1, <2>2,
         IndexedHistoricalTransport(initialContext)!
           AsyncBracketNextPreservesStrongTypeInvariant
         DEF IndexedHistoricalCommitArchiveRouteWitnessAt
    <2>5. ASSUME NEW target \in Responsive,
                   IndexedHistoricalTransport(initialContext)!
                     HistoricalRecoveryTarget(target)',
                   ~IndexedHistoricalTransport(initialContext)!
                      NodeHasDecision(target)',
                   ~IndexedHistoricalTransport(initialContext)!
                      NodeHasApplication(target)'
           PROVE \E server \in ValidatorIds:
                   IndexedHistoricalCommitArchiveRouteAvailable(
                     initialContext, target, server)'
      <3>0. target \in ValidatorIds
        BY <2>1, <2>5, Isa
           DEF IndexedHistoricalCommitArchiveRouteWitnessAt,
               IndexedHistoricalTransport!AsyncStrongTypeInvariant,
               IndexedHistoricalTransport!StrongInductiveInvariant,
               IndexedHistoricalTransport!Safety,
               IndexedHistoricalTransport!TypeInvariant,
               IndexedHistoricalTransport!ModelConfiguration,
               IndexedHistoricalTransport!QuorumConfiguration
      <3>1. ~IndexedHistoricalTransport(initialContext)!
                 NodeHasDecision(target)
        BY <2>2, <2>5,
           IndexedHistoricalTransport(initialContext)!
             AsyncBracketStepPreservesNodeDecision
      <3>2. ~IndexedHistoricalTransport(initialContext)!
                 NodeHasApplication(target)
        BY <2>2, <2>5,
           IndexedHistoricalTransport(initialContext)!
             AsyncBracketStepPreservesNodeApplication
      <3>3. CASE IndexedHistoricalTransport(initialContext)!
                    HistoricalRecoveryTarget(target)
        <4>1. PICK server \in ValidatorIds:
                 IndexedHistoricalCommitArchiveRouteAvailable(
                   initialContext, target, server)
          BY <2>1, <3>1, <3>2, <3>3
             DEF IndexedHistoricalCommitArchiveRouteWitnessAt
        <4>2. IndexedHistoricalCommitArchiveRouteAvailable(
                 initialContext, target, server)'
          BY <1>1, <2>1, <2>5, <3>0, <4>1,
             IndexedHistoricalArchiveRoutePersistsUntilTargetDecision
             DEF IndexedHistoricalCommitArchiveRouteWitnessAt
        <4> QED BY <4>1, <4>2
      <3>4. CASE ~IndexedHistoricalTransport(initialContext)!
                     HistoricalRecoveryTarget(target)
        <4>1. \E server \in ValidatorIds,
                  source \in Chain!DecisionEvidenceSet:
                 IndexedOpenHistoricalRecovery(
                   initialContext, target, server, source)
          BY <1>1, <2>5, <3>0, <3>4,
             IndexedNewHistoricalTargetHasExactOpenSource
        <4>2. PICK server \in ValidatorIds,
                     source \in Chain!DecisionEvidenceSet:
                 IndexedOpenHistoricalRecovery(
                   initialContext, target, server, source)
          BY <4>1
        <4>3. IndexedHistoricalCommitArchiveRouteAvailable(
                 initialContext, target, server)'
          BY <2>1, <3>0, <4>2,
             IndexedOpenHistoricalRecoveryEstablishesPostStateArchiveRoute
             DEF IndexedHistoricalCommitArchiveRouteWitnessAt
        <4> QED BY <4>2, <4>3
      <3> QED BY <3>3, <3>4
    <2> QED BY <2>3, <2>4, <2>5
         DEF IndexedHistoricalCommitArchiveRouteWitnessAt
  <1> QED BY <1>1
       DEF IndexedHistoricalCommitArchiveRouteWitnessInvariant

THEOREM IndexedChainSpecAlwaysHasHistoricalArchiveRouteWitness ==
  IndexedChainSpec
    => []IndexedHistoricalCommitArchiveRouteWitnessInvariant
PROOF
  <1>1. IndexedChainInit
          => IndexedHistoricalCommitArchiveRouteWitnessInvariant
    BY IndexedChainInitEstablishesHistoricalArchiveRouteWitness
  <1>2. /\ IndexedHistoricalCommitArchiveRouteWitnessInvariant
         /\ [IndexedChainNext]_IndexedChainVars
        => IndexedHistoricalCommitArchiveRouteWitnessInvariant'
    BY IndexedBracketStepPreservesHistoricalArchiveRouteWitness
  <1> QED BY <1>1, <1>2, PTL DEF IndexedChainSpec

(***************************************************************************
Requested indexed property.

The auxiliary witness drops directly into the one-height implication.  The
extra active-request, no-Decision, and no-Application guards are consumed
unchanged; no transport liveness or additional responsive-join assumption is
used.
***************************************************************************)

THEOREM IndexedHistoricalArchiveRouteWitnessImpliesAvailability ==
  IndexedHistoricalCommitArchiveRouteWitnessInvariant
    => IndexedHistoricalCommitArchiveRouteAvailabilityInvariant
BY Isa
   DEF IndexedHistoricalCommitArchiveRouteWitnessInvariant,
       IndexedHistoricalCommitArchiveRouteWitnessAt,
       IndexedHistoricalCommitArchiveRouteAvailabilityInvariant,
       IndexedHistoricalCommitArchiveRouteAvailable,
       IndexedHistoricalTransport!
         HistoricalCommitArchiveRouteAvailabilityInvariant

THEOREM IndexedChainSpecAlwaysHasHistoricalCommitArchiveRoute ==
  IndexedChainSpec
    => []IndexedHistoricalCommitArchiveRouteAvailabilityInvariant
BY IndexedChainSpecAlwaysHasHistoricalArchiveRouteWitness,
   IndexedHistoricalArchiveRouteWitnessImpliesAvailability, PTL

THEOREM IndexedChainSpecDischargesHistoricalCommitArchiveRouteAvailability ==
  IndexedHistoricalCommitArchiveRouteAvailabilityProperty
PROOF
  <1>1. IndexedChainSpec
          => []IndexedHistoricalCommitArchiveRouteAvailabilityInvariant
    BY IndexedChainSpecAlwaysHasHistoricalCommitArchiveRoute
  <1>2. ASSUME NEW initialContext \in AdmissibleContextRecords
         PROVE
           IndexedHistoricalTransport(initialContext)!
             HistoricalCommitArchiveRouteAvailabilityProperty(
               IndexedChainSpec)
    <2>1. IndexedChainSpec
            => []IndexedHistoricalTransport(initialContext)!
                  HistoricalCommitArchiveRouteAvailabilityInvariant
      BY <1>1, PTL
         DEF IndexedHistoricalCommitArchiveRouteAvailabilityInvariant
    <2> QED BY <2>1
         DEF IndexedHistoricalTransport!
               HistoricalCommitArchiveRouteAvailabilityProperty
  <1> QED BY <1>2
       DEF IndexedHistoricalCommitArchiveRouteAvailabilityProperty

(***************************************************************************
Joined ownership of ordinary archive-I/O Serve occurrences.

The product intentionally does not require every responsive validator to have
joined every pre-created context.  It does, however, require a join at every
action which can materialize an ordinary Serve occurrence: the normal runner,
the applied historical server, or the historical target runner.  Packet
admission only creates the immutable lifecycle reservation; it cannot append
the I/O occurrence.  Since joins are monotone, every live protected Serve job
therefore has precisely the joined product worker whose weak-fairness clause
will be used below.
***************************************************************************)

IndexedHistoricalServeOwnerJoinedInvariant ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive,
     job \in IndexedHistoricalTransport(initialContext)!AsyncServeJobSet:
    IndexedHistoricalTransport(initialContext)!
      ResponsiveProtectedServeJobOwned(node, job)
      => node \in joinedByContext[initialContext]

THEOREM IndexedChainInitEstablishesHistoricalServeOwnerJoined ==
  IndexedChainInit => IndexedHistoricalServeOwnerJoinedInvariant
BY Isa
   DEF IndexedHistoricalServeOwnerJoinedInvariant,
       IndexedChainInit,
       IndexedHistoricalTransport!ResponsiveProtectedServeJobOwned,
       IndexedHistoricalTransport!AsyncServeJobSet,
       IndexedHistoricalTransport!AsyncIoJob,
       IndexedHistoricalTransport!AsyncInitAt,
       IndexedHistoricalTransport!AsyncBaseInitAt,
       IndexedHistoricalTransport!AsyncSchedulerInit,
       IndexedHistoricalTransport!AsyncIoInit,
       IndexedHistoricalTransport!SequenceSet,
       IndexedCore, IndexedScheduler, IndexedRecovery

THEOREM IndexedNewHistoricalServeOwnerHasJoinedProducer ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive,
     job \in IndexedHistoricalTransport(initialContext)!AsyncServeJobSet:
    /\ IndexedCompositionInvariant
    /\ IndexedChainNext
    /\ ~IndexedHistoricalTransport(initialContext)!
          ResponsiveProtectedServeJobOwned(node, job)
    /\ IndexedHistoricalTransport(initialContext)!
         ResponsiveProtectedServeJobOwned(node, job)'
    => node \in joinedByContext'[initialContext]
BY IsaT(600)
   DEF IndexedCompositionInvariant,
       IndexedHistoricalRecoveryTargetCoherence,
       IndexedChainNext, IndexedProductActionAt,
       IndexedJoinedAsyncNext, IndexedJoinedNonCrashStep,
       IndexedJoinedRunnerStep, IndexedJoinedNonRunnerStep,
       IndexedReceiptClassification,
       IndexedReceiptFreeChainStutter,
       IndexedDecisionReceiptHandoff,
       IndexedApplicationReceiptHandoff,
       IndexedHistoricalTransport!ResponsiveProtectedServeJobOwned,
       IndexedHistoricalTransport!AsyncServeJobSet,
       IndexedHistoricalTransport!AsyncIoJob,
       IndexedHistoricalTransport!AsyncNext,
       IndexedHistoricalTransport!AsyncNonCrashStep,
       IndexedHistoricalTransport!AsyncRunnerStep,
       IndexedHistoricalTransport!AsyncNonRunnerStep,
       IndexedHistoricalTransport!RunNode,
       IndexedHistoricalTransport!RunHistoricalRecoveryNode,
       IndexedHistoricalTransport!RunNodeWork,
       IndexedHistoricalTransport!LocalAdmissionStep,
       IndexedHistoricalTransport!IngressDrainStep,
       IndexedHistoricalTransport!SerializedRunnerRuntimeStep,
       IndexedHistoricalTransport!SerializedRuntimeStep,
       IndexedHistoricalTransport!
         SerializedRuntimePrecedesServeIngressStep,
       IndexedHistoricalTransport!SerializedLocalPrecedesServeIngressStep,
       IndexedHistoricalTransport!AsyncServeIngressTargetOnlyTurn,
       IndexedHistoricalTransport!SelectedLocalAdmissionAdvance,
       IndexedHistoricalTransport!RunHistoricalServer,
       IndexedHistoricalTransport!DrainFairIngressSelected,
       IndexedHistoricalTransport!DrainHistoricalIngressSelected,
       IndexedHistoricalTransport!AsyncIoCertifiedServeJob,
       IndexedHistoricalTransport!ServiceIoWorker,
       IndexedHistoricalTransport!ServiceHistoricalRecoveryIoWorker,
       IndexedHistoricalTransport!EnqueueIoLocalControl,
       IndexedHistoricalTransport!EnqueueHistoricalRecoveryIoLocalControl,
       IndexedHistoricalTransport!AsyncNetworkStep,
       IndexedHistoricalTransport!AsyncFaultStep,
       IndexedHistoricalTransport!PreGstCrash,
       IndexedHistoricalTransport!PreGstResponsiveCrash,
       IndexedHistoricalTransport!PreGstResponsiveRestart,
       IndexedHistoricalTransport!PreGstResponsiveReplay,
       IndexedHistoricalTransport!ResetNodeSchedulerForRestart,
       IndexedHistoricalTransport!SequenceSet,
       IndexedHistoricalTransport!AsyncAllVars,
       IndexedHistoricalTransport!AsyncSchedulerVars,
       IndexedHistoricalTransport!vars,
       IndexedCore, IndexedScheduler, IndexedRecovery

THEOREM IndexedBracketStepPreservesHistoricalServeOwnerJoined ==
  /\ IndexedCompositionInvariant
  /\ IndexedHistoricalServeOwnerJoinedInvariant
  /\ [IndexedChainNext]_IndexedChainVars
  => IndexedHistoricalServeOwnerJoinedInvariant'
PROOF
  <1>1. ASSUME IndexedCompositionInvariant,
               IndexedHistoricalServeOwnerJoinedInvariant,
               [IndexedChainNext]_IndexedChainVars,
               NEW initialContext \in AdmissibleContextRecords,
               NEW node \in Responsive,
               NEW job \in
                 IndexedHistoricalTransport(initialContext)!AsyncServeJobSet'
         PROVE IndexedHistoricalTransport(initialContext)!
                 ResponsiveProtectedServeJobOwned(node, job)'
                 => node \in joinedByContext'[initialContext]
    <2>1. CASE IndexedChainNext
      <3>1. CASE IndexedHistoricalTransport(initialContext)!
                    ResponsiveProtectedServeJobOwned(node, job)
        <4>1. node \in joinedByContext[initialContext]
          BY <1>1, <3>1
             DEF IndexedHistoricalServeOwnerJoinedInvariant
        <4>2. joinedByContext[initialContext]
                 \subseteq joinedByContext'[initialContext]
          BY <2>1, JoinedMembershipIsMonotone
        <4> QED BY <4>1, <4>2
      <3>2. CASE ~IndexedHistoricalTransport(initialContext)!
                     ResponsiveProtectedServeJobOwned(node, job)
        <4> QED BY <1>1, <2>1, <3>2,
             IndexedNewHistoricalServeOwnerHasJoinedProducer
      <3> QED BY <3>1, <3>2
    <2>2. CASE UNCHANGED IndexedChainVars
      BY <1>1, <2>2, Isa
         DEF IndexedChainVars,
             IndexedHistoricalTransport!
               ResponsiveProtectedServeJobOwned,
             IndexedHistoricalTransport!AsyncServeJobSet,
             IndexedHistoricalTransport!AsyncIoJob,
             IndexedHistoricalTransport!AsyncAllVars,
             IndexedHistoricalTransport!AsyncSchedulerVars,
             IndexedHistoricalTransport!vars,
             IndexedCore, IndexedScheduler, IndexedRecovery
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1
       DEF IndexedHistoricalServeOwnerJoinedInvariant

THEOREM IndexedChainSpecAlwaysHasJoinedHistoricalServeOwners ==
  IndexedChainSpec => []IndexedHistoricalServeOwnerJoinedInvariant
PROOF
  <1>1. IndexedChainInit => IndexedHistoricalServeOwnerJoinedInvariant
    BY IndexedChainInitEstablishesHistoricalServeOwnerJoined
  <1>2. /\ IndexedCompositionInvariant
         /\ IndexedHistoricalServeOwnerJoinedInvariant
         /\ [IndexedChainNext]_IndexedChainVars
        => IndexedHistoricalServeOwnerJoinedInvariant'
    BY IndexedBracketStepPreservesHistoricalServeOwnerJoined
  <1>3. IndexedChainSpec => []IndexedCompositionInvariant
    BY IndexedChainSpecEstablishesCompositionInvariant
  <1> QED BY <1>1, <1>2, <1>3, PTL DEF IndexedChainSpec

(***************************************************************************
Product-local ordinary Serve FIFO closure.

This is the natural-position proof used by the one-height scheduler, lifted
to the exact joined archive worker.  No aggregate Async fairness is projected:
each rank cell consumes only `IndexedIoWorkerStep(initialContext, node)` and
its matching individual weak-fairness clause.
***************************************************************************)

IndexedHistoricalServeStage5Pending(
    initialContext, node, job, position) ==
  IndexedHistoricalTransport(initialContext)!
    ProtectedServeStage5Pending(node, job, position)

IndexedHistoricalServeRankProgressExit(
    initialContext, node, job, position) ==
  IndexedHistoricalTransport(initialContext)!
    ProtectedServeRankProgressExit(node, job, <<5, position>>)

THEOREM IndexedHistoricalServePendingHasJoinedFairWorker ==
  \A initialContext \in AdmissibleContextRecords,
     node, job, position:
    /\ IndexedHistoricalServeOwnerJoinedInvariant
    /\ IndexedHistoricalServeStage5Pending(
         initialContext, node, job, position)
    => /\ node \in Responsive
       /\ node \in joinedByContext[initialContext]
       /\ initialContext \in JoinedContexts
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW node, NEW job, NEW position,
                IndexedHistoricalServeOwnerJoinedInvariant,
                IndexedHistoricalServeStage5Pending(
                  initialContext, node, job, position)
         PROVE /\ node \in Responsive
               /\ node \in joinedByContext[initialContext]
               /\ initialContext \in JoinedContexts
    <2>1. /\ node \in Responsive
           /\ IndexedHistoricalTransport(initialContext)!
                ResponsiveProtectedServeJobOwned(node, job)
      BY <1>1,
         IndexedHistoricalTransport(initialContext)!
           ProtectedServeStage5CarrierFacts
         DEF IndexedHistoricalServeStage5Pending
    <2>2. node \in joinedByContext[initialContext]
      BY <1>1, <2>1
         DEF IndexedHistoricalServeOwnerJoinedInvariant
    <2>3. initialContext \in JoinedContexts
      BY <2>2 DEF JoinedContexts
    <2> QED BY <2>1, <2>2, <2>3
  <1> QED BY <1>1

THEOREM IndexedHistoricalServePendingEnablesExactProductWorker ==
  \A initialContext \in AdmissibleContextRecords,
     node, job, position:
    /\ IndexedCompositionInvariant
    /\ IndexedHistoricalServeOwnerJoinedInvariant
    /\ IndexedHistoricalServeStage5Pending(
         initialContext, node, job, position)
    => ENABLED IndexedIoWorkerStep(initialContext, node)
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW node, NEW job, NEW position,
                IndexedCompositionInvariant,
                IndexedHistoricalServeOwnerJoinedInvariant,
                IndexedHistoricalServeStage5Pending(
                  initialContext, node, job, position)
         PROVE ENABLED IndexedIoWorkerStep(initialContext, node)
    <2>1. /\ node \in Responsive
           /\ node \in joinedByContext[initialContext]
           /\ initialContext \in JoinedContexts
      BY <1>1, IndexedHistoricalServePendingHasJoinedFairWorker
    <2>2. ENABLED
             <<IndexedHistoricalTransport(initialContext)!
                 PostGstServiceIoWorker(node)>>_(
               IndexedHistoricalTransport(initialContext)!AsyncAllVars)
      BY <1>1,
         IndexedHistoricalTransport(initialContext)!
           ProtectedServeStage5EnablesFairWorker
         DEF IndexedHistoricalServeStage5Pending
    <2>3. ENABLED
             IndexedAsync(initialContext)!PostGstServiceIoWorker(node)
      BY <2>2, Isa
         DEF IndexedHistoricalTransport!PostGstServiceIoWorker,
             IndexedAsync!PostGstServiceIoWorker,
             IndexedHistoricalTransport!ServiceIoWorker,
             IndexedAsync!ServiceIoWorker
    <2> QED BY <1>1, <2>1, <2>3,
         IndexedFairActionsRemainEnabledInProduct
  <1> QED BY <1>1

THEOREM IndexedHistoricalServeProductWorkerIsNonstuttering ==
  \A initialContext \in AdmissibleContextRecords,
     node, job, position:
    /\ IndexedHistoricalServeStage5Pending(
         initialContext, node, job, position)
    /\ IndexedIoWorkerStep(initialContext, node)
    => <<IndexedIoWorkerStep(
           initialContext, node)>>_IndexedChainVars
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW node, NEW job, NEW position,
                IndexedHistoricalServeStage5Pending(
                  initialContext, node, job, position),
                IndexedIoWorkerStep(initialContext, node)
         PROVE <<IndexedIoWorkerStep(
                   initialContext, node)>>_IndexedChainVars
    <2>1. IndexedHistoricalTransport(initialContext)!
             AsyncIoQueueDepth(node) > 0
      BY <1>1,
         IndexedHistoricalTransport(initialContext)!
           ProtectedServeStage5CarrierFacts
         DEF IndexedHistoricalServeStage5Pending
    <2>2. IndexedHistoricalTransport(initialContext)!
             PostGstServiceIoWorker(node)
      BY <1>1, Isa
         DEF IndexedIoWorkerStep,
             IndexedAsync!PostGstServiceIoWorker,
             IndexedHistoricalTransport!PostGstServiceIoWorker,
             IndexedAsync!ServiceIoWorker,
             IndexedHistoricalTransport!ServiceIoWorker
    <2>3. <<IndexedHistoricalTransport(initialContext)!
               PostGstServiceIoWorker(node)>>_(
             IndexedHistoricalTransport(initialContext)!AsyncAllVars)
      BY <2>1, <2>2,
         IndexedHistoricalTransport(initialContext)!
           QueuedIoServiceIsNonstuttering
    <2>4. IndexedAsyncStateShape
      BY <1>1 DEF IndexedIoWorkerStep, IndexedChainNext
    <2>5. IndexedHistoricalTransport(initialContext)!AsyncAllVars =
             IndexedAsyncStateAt(initialContext)
      BY <2>4, IndexedHistoricalTransportVariablesAreExact
    <2>6. IndexedChainVars' # IndexedChainVars
      BY <2>3, <2>5, Isa
         DEF IndexedChainVars, IndexedAsyncStateAt
    <2> QED BY <1>1, <2>6
  <1> QED BY <1>1

THEOREM IndexedHistoricalServePendingEnablesFairOccurrence ==
  \A initialContext \in AdmissibleContextRecords,
     node, job, position:
    /\ IndexedCompositionInvariant
    /\ IndexedHistoricalServeOwnerJoinedInvariant
    /\ IndexedHistoricalServeStage5Pending(
         initialContext, node, job, position)
    => ENABLED
         <<IndexedIoWorkerStep(
             initialContext, node)>>_IndexedChainVars
BY IndexedHistoricalServePendingEnablesExactProductWorker,
   IndexedHistoricalServeProductWorkerIsNonstuttering,
   ENABLEDaxioms

THEOREM IndexedHistoricalServeFairOccurrenceStrictlyProgresses ==
  \A initialContext \in AdmissibleContextRecords,
     node, job, position:
    /\ IndexedHistoricalServeStage5Pending(
         initialContext, node, job, position)
    /\ <<IndexedIoWorkerStep(
           initialContext, node)>>_IndexedChainVars
    => IndexedHistoricalServeRankProgressExit(
         initialContext, node, job, position)'
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW node, NEW job, NEW position,
                IndexedHistoricalServeStage5Pending(
                  initialContext, node, job, position),
                <<IndexedIoWorkerStep(
                    initialContext, node)>>_IndexedChainVars
         PROVE IndexedHistoricalServeRankProgressExit(
                 initialContext, node, job, position)'
    <2>1. IndexedHistoricalTransport(initialContext)!
             PostGstServiceIoWorker(node)
      BY <1>1, Isa
         DEF IndexedIoWorkerStep,
             IndexedAsync!PostGstServiceIoWorker,
             IndexedHistoricalTransport!PostGstServiceIoWorker,
             IndexedAsync!ServiceIoWorker,
             IndexedHistoricalTransport!ServiceIoWorker
    <2> QED BY <1>1, <2>1,
         IndexedHistoricalTransport(initialContext)!
           ProtectedServeStage5WorkerStrictlyProgresses
         DEF IndexedHistoricalServeStage5Pending,
             IndexedHistoricalServeRankProgressExit
  <1> QED BY <1>1

THEOREM IndexedHistoricalServePendingUnlessProgress ==
  \A initialContext \in AdmissibleContextRecords,
     node, job, position:
    /\ IndexedHistoricalServeStage5Pending(
         initialContext, node, job, position)
    /\ [IndexedChainNext]_IndexedChainVars
    => \/ IndexedHistoricalServeStage5Pending(
            initialContext, node, job, position)'
       \/ IndexedHistoricalServeRankProgressExit(
            initialContext, node, job, position)'
BY IndexedBracketStepProjectsEveryHistoricalTransportStep,
   IndexedHistoricalTransport!
     ProtectedServeStage5UnlessProgress
   DEF IndexedHistoricalServeStage5Pending,
       IndexedHistoricalServeRankProgressExit

THEOREM IndexedChainSpecClosesHistoricalServeRankCell ==
  IndexedChainSpec
    => \A initialContext \in AdmissibleContextRecords,
          node, job:
         \A position \in Nat:
           IndexedHistoricalServeStage5Pending(
             initialContext, node, job, position)
             ~> IndexedHistoricalServeRankProgressExit(
                  initialContext, node, job, position)
PROOF
  <1>1. ASSUME IndexedChainSpec,
                NEW initialContext \in AdmissibleContextRecords,
                NEW node, NEW job, NEW position \in Nat
         PROVE IndexedHistoricalServeStage5Pending(
                 initialContext, node, job, position)
                 ~>
               IndexedHistoricalServeRankProgressExit(
                 initialContext, node, job, position)
    <2>1. []IndexedCompositionInvariant
      BY <1>1, IndexedChainSpecEstablishesCompositionInvariant
    <2>2. []IndexedHistoricalServeOwnerJoinedInvariant
      BY <1>1,
         IndexedChainSpecAlwaysHasJoinedHistoricalServeOwners
    <2>3. [][IndexedChainNext]_IndexedChainVars
      BY <1>1 DEF IndexedChainSpec
    <2>4. IndexedHistoricalServeStage5Pending(
             initialContext, node, job, position)
             /\ [IndexedChainNext]_IndexedChainVars
            => \/ IndexedHistoricalServeStage5Pending(
                    initialContext, node, job, position)'
               \/ IndexedHistoricalServeRankProgressExit(
                    initialContext, node, job, position)'
      BY IndexedHistoricalServePendingUnlessProgress
    <2>5. /\ IndexedCompositionInvariant
           /\ IndexedHistoricalServeOwnerJoinedInvariant
           /\ IndexedHistoricalServeStage5Pending(
                initialContext, node, job, position)
          => ENABLED
               <<IndexedIoWorkerStep(
                   initialContext, node)>>_IndexedChainVars
      BY IndexedHistoricalServePendingEnablesFairOccurrence
    <2>6. /\ IndexedHistoricalServeStage5Pending(
                initialContext, node, job, position)
           /\ <<IndexedIoWorkerStep(
                  initialContext, node)>>_IndexedChainVars
          => IndexedHistoricalServeRankProgressExit(
               initialContext, node, job, position)'
      BY IndexedHistoricalServeFairOccurrenceStrictlyProgresses
    <2>7. IndexedHistoricalServeStage5Pending(
             initialContext, node, job, position)
             => node \in Responsive
      BY IndexedHistoricalTransport(initialContext)!
           ProtectedServeStage5CarrierFacts
         DEF IndexedHistoricalServeStage5Pending
    <2>8. WF_IndexedChainVars(
             IndexedIoWorkerStep(initialContext, node))
      BY <1>1, <2>7 DEF IndexedChainSpec, IndexedFairness
    <2> QED BY <2>1, <2>2, <2>3, <2>4, <2>5,
                 <2>6, <2>8, PTL
  <1> QED BY <1>1

IndexedHistoricalProtectedServeStarvationProperty ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive,
     job \in IndexedHistoricalTransport(initialContext)!AsyncServeJobSet:
    (IndexedHistoricalTransport(initialContext)!gst
      /\ IndexedHistoricalTransport(initialContext)!
           ResponsiveProtectedServeJobOwned(node, job))
      ~> ~IndexedHistoricalTransport(initialContext)!
             ResponsiveProtectedServeJobOwned(node, job)

THEOREM IndexedChainSpecClosesHistoricalProtectedServeStarvation ==
  IndexedChainSpec
    => IndexedHistoricalProtectedServeStarvationProperty
PROOF
  <1>1. ASSUME IndexedChainSpec,
                NEW initialContext \in AdmissibleContextRecords,
                NEW node \in Responsive,
                NEW job \in
                  IndexedHistoricalTransport(initialContext)!AsyncServeJobSet
         PROVE (IndexedHistoricalTransport(initialContext)!gst
                  /\ IndexedHistoricalTransport(initialContext)!
                       ResponsiveProtectedServeJobOwned(node, job))
                 ~> ~IndexedHistoricalTransport(initialContext)!
                        ResponsiveProtectedServeJobOwned(node, job)
    <2>1. []IndexedHistoricalTransport(initialContext)!
             AsyncStrongTypeInvariant
      BY <1>1, IndexedChainSpecAlwaysHistoricalTemporalSupport, PTL
         DEF IndexedHistoricalTemporalSupportAt
    <2>2. \A position \in Nat:
             IndexedHistoricalServeStage5Pending(
               initialContext, node, job, position)
               ~> IndexedHistoricalServeRankProgressExit(
                    initialContext, node, job, position)
      BY <1>1, IndexedChainSpecClosesHistoricalServeRankCell
    <2>3. \A position \in Nat:
             IndexedHistoricalTransport(initialContext)!
               ProtectedServeOwnedAtServiceRank(
                 node, job, <<5, position>>)
               ~>
             (IndexedHistoricalTransport(initialContext)!
                ProtectedServeOwnershipExit(node, job)
               \/ \E lower \in SetLessThan(
                    position, OpToRel(<, Nat), Nat):
                    IndexedHistoricalTransport(initialContext)!
                      ProtectedServeOwnedAtServiceRank(
                        node, job, <<5, lower>>))
      BY <2>1, <2>2,
         IndexedHistoricalTransport(initialContext)!
           ProtectedServeRankExitHasWellFoundedSuccessor,
         PTL
         DEF IndexedHistoricalServeStage5Pending,
             IndexedHistoricalServeRankProgressExit,
             IndexedHistoricalTransport!
               ProtectedServeRankProgressExit
    <2>4. \A position \in Nat:
             IndexedHistoricalTransport(initialContext)!
               ProtectedServeOwnedAtServiceRank(
                 node, job, <<5, position>>)
               ~>
             IndexedHistoricalTransport(initialContext)!
               ProtectedServeOwnershipExit(node, job)
      BY ONLY <2>3,
         IndexedHistoricalTransport(initialContext)!
           ProtectedServeWellFoundedRankConvergence,
         SMT
    <2>5. (\E position \in Nat:
                IndexedHistoricalTransport(initialContext)!
                  ProtectedServeOwnedAtServiceRank(
                    node, job, <<5, position>>))
             ~>
           IndexedHistoricalTransport(initialContext)!
             ProtectedServeOwnershipExit(node, job)
      BY ONLY <2>4,
         IndexedHistoricalTransport(initialContext)!
           ProtectedServeRankExistentialLift,
         SMT
    <2>6. [](IndexedHistoricalTransport(initialContext)!gst
               /\ IndexedHistoricalTransport(initialContext)!
                    ResponsiveProtectedServeJobOwned(node, job)
              => \E position \in Nat:
                   IndexedHistoricalTransport(initialContext)!
                     ProtectedServeOwnedAtServiceRank(
                       node, job, <<5, position>>))
      BY <2>1,
         IndexedHistoricalTransport(initialContext)!
           ResponsiveProtectedServeJobHasRankPosition,
         PTL
    <2> QED BY <2>5, <2>6, PTL
         DEF IndexedHistoricalTransport!ProtectedServeOwnershipExit
  <1> QED BY <1>1
       DEF IndexedHistoricalProtectedServeStarvationProperty

(***************************************************************************
The two exact ordinary-I/O response kernels.

The generic FIFO conclusion is only occurrence departure.  The action-local
lineage theorems in the transport instance retain the exact alias and logical
Serve identity until that departure is the response-producing head service.
Composing those safety classifications with the product-local starvation
theorem closes the Commit and historical-Decision Serve kernels without
assuming either broad transport leaf.
***************************************************************************)

THEOREM IndexedHistoricalCommitServeResidualPersistsOrResponds ==
  \A initialContext \in AdmissibleContextRecords,
     target, server, request, job:
    /\ IndexedHistoricalTransport(initialContext)!
         AsyncStrongTypeInvariant
    /\ IndexedHistoricalTransport(initialContext)!
         HistoricalCommitServeJobOwned(
           target, server, request, job)
    /\ ~IndexedHistoricalTransport(initialContext)!
          HistoricalCommitResponsePacketGoal(target, server, request)
    /\ [IndexedChainNext]_IndexedChainVars
    => \/ IndexedHistoricalTransport(initialContext)!
            HistoricalCommitServeJobOwned(
              target, server, request, job)'
       \/ IndexedHistoricalTransport(initialContext)!
            HistoricalCommitResponsePacketGoal(
              target, server, request)'
BY IndexedBracketStepProjectsEveryHistoricalTransportStep,
   IndexedHistoricalTransport!
     HistoricalCommitServeOwnerPersistsOrHandsOff,
   Isa
   DEF IndexedHistoricalTransport!
         HistoricalCommitResponsePacketGoal

THEOREM IndexedHistoricalDecisionServeResidualPersistsOrResponds ==
  \A initialContext \in AdmissibleContextRecords,
     node, qc, archive, request, job:
    /\ IndexedHistoricalTransport(initialContext)!
         AsyncStrongTypeInvariant
    /\ IndexedHistoricalTransport(initialContext)!
         HistoricalDecisionServeJobOwned(
           node, qc, archive, request, job)
    /\ ~IndexedHistoricalTransport(initialContext)!
          HistoricalDecisionResponsePacketGoal(
            node, qc, archive, request)
    /\ [IndexedChainNext]_IndexedChainVars
    => \/ IndexedHistoricalTransport(initialContext)!
            HistoricalDecisionServeJobOwned(
              node, qc, archive, request, job)'
       \/ IndexedHistoricalTransport(initialContext)!
            HistoricalDecisionResponsePacketGoal(
              node, qc, archive, request)'
BY IndexedBracketStepProjectsEveryHistoricalTransportStep,
   IndexedHistoricalTransport!
     HistoricalDecisionServePersistsOrResponds,
   Isa
   DEF IndexedHistoricalTransport!
         HistoricalDecisionResponsePacketGoal

THEOREM IndexedChainSpecClosesHistoricalCommitServeResponseKernel ==
  IndexedChainSpec
    => \A initialContext \in AdmissibleContextRecords,
          target, server, request, job:
         IndexedHistoricalTransport(initialContext)!
           HistoricalCommitServeJobOwned(
             target, server, request, job)
           ~>
         IndexedHistoricalTransport(initialContext)!
           HistoricalCommitResponsePacketGoal(
             target, server, request)
PROOF
  <1>1. ASSUME IndexedChainSpec,
                NEW initialContext \in AdmissibleContextRecords,
                NEW target, NEW server, NEW request, NEW job
         PROVE IndexedHistoricalTransport(initialContext)!
                 HistoricalCommitServeJobOwned(
                   target, server, request, job)
                 ~>
               IndexedHistoricalTransport(initialContext)!
                 HistoricalCommitResponsePacketGoal(
                   target, server, request)
    <2>1. []IndexedHistoricalTransport(initialContext)!
             AsyncStrongTypeInvariant
      BY <1>1, IndexedChainSpecAlwaysHistoricalTemporalSupport, PTL
         DEF IndexedHistoricalTemporalSupportAt
    <2>2. [](IndexedHistoricalTransport(initialContext)!
               HistoricalCommitServeJobOwned(
                 target, server, request, job)
              => /\ IndexedHistoricalTransport(initialContext)!gst
                 /\ server \in Responsive
                 /\ job \in IndexedHistoricalTransport(initialContext)!
                               AsyncServeJobSet
                 /\ IndexedHistoricalTransport(initialContext)!
                      ResponsiveProtectedServeJobOwned(server, job))
      BY <2>1,
         IndexedHistoricalTransport(initialContext)!
           HistoricalCommitServeJobUsesOrdinaryArchiveIoOwner,
         PTL
         DEF IndexedHistoricalTransport!
               HistoricalCommitServeJobOwned,
             IndexedHistoricalTransport!
               HistoricalCommitArchiveRouteAvailable
    <2>3. (IndexedHistoricalTransport(initialContext)!gst
              /\ IndexedHistoricalTransport(initialContext)!
                   ResponsiveProtectedServeJobOwned(server, job))
             ~>
           ~IndexedHistoricalTransport(initialContext)!
              ResponsiveProtectedServeJobOwned(server, job)
      BY <1>1, <2>2,
         IndexedChainSpecClosesHistoricalProtectedServeStarvation
         DEF IndexedHistoricalProtectedServeStarvationProperty
    <2>4. IndexedHistoricalTransport(initialContext)!
             HistoricalCommitServeJobOwned(
               target, server, request, job)
             ~>
           ~IndexedHistoricalTransport(initialContext)!
              ResponsiveProtectedServeJobOwned(server, job)
      BY <2>2, <2>3, PTL
    <2>5. [][IndexedChainNext]_IndexedChainVars
      BY <1>1 DEF IndexedChainSpec
    <2>6. /\ IndexedHistoricalTransport(initialContext)!
                  AsyncStrongTypeInvariant
           /\ IndexedHistoricalTransport(initialContext)!
                HistoricalCommitServeJobOwned(
                  target, server, request, job)
           /\ ~IndexedHistoricalTransport(initialContext)!
                 HistoricalCommitResponsePacketGoal(
                   target, server, request)
           /\ [IndexedChainNext]_IndexedChainVars
          => \/ IndexedHistoricalTransport(initialContext)!
                  HistoricalCommitServeJobOwned(
                    target, server, request, job)'
             \/ IndexedHistoricalTransport(initialContext)!
                  HistoricalCommitResponsePacketGoal(
                    target, server, request)'
      BY IndexedHistoricalCommitServeResidualPersistsOrResponds
    <2> QED BY <2>1, <2>2, <2>4, <2>5, <2>6, PTL
  <1> QED BY <1>1

THEOREM IndexedChainSpecClosesHistoricalDecisionServeResponseKernel ==
  IndexedChainSpec
    => \A initialContext \in AdmissibleContextRecords,
          node, qc, archive, request, job:
         IndexedHistoricalTransport(initialContext)!
           HistoricalDecisionServeJobOwned(
             node, qc, archive, request, job)
           ~>
         IndexedHistoricalTransport(initialContext)!
           HistoricalDecisionResponsePacketGoal(
             node, qc, archive, request)
PROOF
  <1>1. ASSUME IndexedChainSpec,
                NEW initialContext \in AdmissibleContextRecords,
                NEW node, NEW qc, NEW archive, NEW request, NEW job
         PROVE IndexedHistoricalTransport(initialContext)!
                 HistoricalDecisionServeJobOwned(
                   node, qc, archive, request, job)
                 ~>
               IndexedHistoricalTransport(initialContext)!
                 HistoricalDecisionResponsePacketGoal(
                   node, qc, archive, request)
    <2>1. []IndexedHistoricalTransport(initialContext)!
             AsyncStrongTypeInvariant
      BY <1>1, IndexedChainSpecAlwaysHistoricalTemporalSupport, PTL
         DEF IndexedHistoricalTemporalSupportAt
    <2>2. [](IndexedHistoricalTransport(initialContext)!
               HistoricalDecisionServeJobOwned(
                 node, qc, archive, request, job)
              => /\ IndexedHistoricalTransport(initialContext)!gst
                 /\ archive \in Responsive
                 /\ job \in IndexedHistoricalTransport(initialContext)!
                               AsyncServeJobSet
                 /\ IndexedHistoricalTransport(initialContext)!
                      ResponsiveProtectedServeJobOwned(archive, job))
      BY <2>1,
         IndexedHistoricalTransport(initialContext)!
           HistoricalDecisionServeJobUsesOrdinaryArchiveIoOwner,
         PTL
         DEF IndexedHistoricalTransport!
               HistoricalDecisionServeJobOwned,
             IndexedHistoricalTransport!
               HistoricalDecisionBodyHoldingAlias,
             IndexedHistoricalTransport!
               HistoricalExactDecisionActiveRequestOwner,
             IndexedHistoricalTransport!
               HistoricalExactDecisionServiceSource
    <2>3. (IndexedHistoricalTransport(initialContext)!gst
              /\ IndexedHistoricalTransport(initialContext)!
                   ResponsiveProtectedServeJobOwned(archive, job))
             ~>
           ~IndexedHistoricalTransport(initialContext)!
              ResponsiveProtectedServeJobOwned(archive, job)
      BY <1>1, <2>2,
         IndexedChainSpecClosesHistoricalProtectedServeStarvation
         DEF IndexedHistoricalProtectedServeStarvationProperty
    <2>4. IndexedHistoricalTransport(initialContext)!
             HistoricalDecisionServeJobOwned(
               node, qc, archive, request, job)
             ~>
           ~IndexedHistoricalTransport(initialContext)!
              ResponsiveProtectedServeJobOwned(archive, job)
      BY <2>2, <2>3, PTL
    <2>5. [][IndexedChainNext]_IndexedChainVars
      BY <1>1 DEF IndexedChainSpec
    <2>6. /\ IndexedHistoricalTransport(initialContext)!
                  AsyncStrongTypeInvariant
           /\ IndexedHistoricalTransport(initialContext)!
                HistoricalDecisionServeJobOwned(
                  node, qc, archive, request, job)
           /\ ~IndexedHistoricalTransport(initialContext)!
                 HistoricalDecisionResponsePacketGoal(
                   node, qc, archive, request)
           /\ [IndexedChainNext]_IndexedChainVars
          => \/ IndexedHistoricalTransport(initialContext)!
                  HistoricalDecisionServeJobOwned(
                    node, qc, archive, request, job)'
             \/ IndexedHistoricalTransport(initialContext)!
                  HistoricalDecisionResponsePacketGoal(
                    node, qc, archive, request)'
      BY IndexedHistoricalDecisionServeResidualPersistsOrResponds
    <2> QED BY <2>1, <2>2, <2>4, <2>5, <2>6, PTL
  <1> QED BY <1>1

(***************************************************************************
Exact indexed transport residual decomposition.

The six remaining transport kernels are deliberately split at physical
owners rather than hidden behind packet fairness.  In particular:

  * retransmission uses the historical target's complete runner action.  The
    intersection of that action with the sending Runtime branch is not a fair
    action and is used below only in an action-local handoff theorem;
  * request packet admission is separated from the immutable Serve ingress
    lifecycle.  The lifecycle rank is already typed and well founded, but its
    exact archive activation/membership and indexed action-origin/rank-step
    properties remain explicit;
  * Commit response admission separates the target packet from the exact
    historical-runner ingress occurrence; and
  * historical Decision response admission retains the route-neutral claim.
    Its packet-head property explicitly includes distinct-claim contention
    and normalized physical-completion debt; neither is discharged by the
    ordinary Serve response theorem.

Every name ending in `ResidualProperty` or `ResidualProperties` below is an
operator, not a theorem or assumption.  The `ResidualsCloseKernel` theorems
are conditional reductions only; they do not prove those operators.  The
action-local theorems prove only the concrete handoff after the production
action has actually been selected.  This keeps the missing per-action
fairness/rank premise visible for the indexed scheduler proof and avoids weak
fairness of an action/progress intersection.
***************************************************************************)

IndexedHistoricalSendingRetransmitLocalStep(initialContext, node) ==
  /\ IndexedHistoricalTransport(initialContext)!
       PostGstRunHistoricalRecoveryNode(node)
  /\ \/ /\ IndexedHistoricalTransport(initialContext)!
               DirectRetransmitStep(node)
         /\ IndexedHistoricalTransport(initialContext)!NodeIdle(node)
     \/ IndexedHistoricalTransport(initialContext)!
          DeferredRetransmitStep(node)

(***************************************************************************
The Runtime prefix must use the gated runner split, not the unrestricted
`SerializedRuntimeStepIsEnabled` claim.  With no Serve ticket, Local/Ingress
may finish and ordinary serialized Runtime is the only unrestricted arm.
A strictly older Runtime or Local lifecycle may take exactly its predecessor
interleave while retaining the ticket.  Every other ticket-bearing
Runtime/Local state takes the target-only turn, and Ingress consumes the
immutable ticket rank.  These four cases are only a state partition here;
their temporal closure is kept in each emission Runtime-prefix residual below.
***************************************************************************)

IndexedHistoricalRetransmitNoServeTicketRunnerPrefix(
    initialContext, node) ==
  /\ IndexedHistoricalTransport(initialContext)!
       AsyncServeIngressLifecycleOwnerIdentities(node) = {}
  /\ IndexedHistoricalTransport(initialContext)!
       asyncRunnerPhase[node] \in {"Local", "Ingress", "Runtime"}

IndexedHistoricalRetransmitOlderRuntimePredecessorPrefix(
    initialContext, node) ==
  /\ IndexedHistoricalTransport(initialContext)!
       AsyncServeIngressLifecycleOwnerIdentities(node) # {}
  /\ IndexedHistoricalTransport(initialContext)!
       asyncRunnerPhase[node] = "Runtime"
  /\ IndexedHistoricalTransport(initialContext)!
       AsyncOlderRuntimeLifecyclePrecedesServeIngress(node)

IndexedHistoricalRetransmitOlderLocalPredecessorPrefix(
    initialContext, node) ==
  /\ IndexedHistoricalTransport(initialContext)!
       AsyncServeIngressLifecycleOwnerIdentities(node) # {}
  /\ IndexedHistoricalTransport(initialContext)!
       asyncRunnerPhase[node] = "Local"
  /\ IndexedHistoricalTransport(initialContext)!
       AsyncOlderLocalLifecyclePrecedesServeIngress(node)

IndexedHistoricalRetransmitServeTargetCorridorPrefix(
    initialContext, node) ==
  /\ IndexedHistoricalTransport(initialContext)!
       AsyncServeIngressLifecycleOwnerIdentities(node) # {}
  /\ \/ IndexedHistoricalTransport(initialContext)!
          asyncRunnerPhase[node] = "Ingress"
     \/ /\ IndexedHistoricalTransport(initialContext)!
             asyncRunnerPhase[node] \in {"Runtime", "Local"}
        /\ ~( \/ /\ IndexedHistoricalTransport(initialContext)!
                      asyncRunnerPhase[node] = "Runtime"
                  /\ IndexedHistoricalTransport(initialContext)!
                       AsyncOlderRuntimeLifecyclePrecedesServeIngress(node)
               \/ /\ IndexedHistoricalTransport(initialContext)!
                      asyncRunnerPhase[node] = "Local"
                  /\ IndexedHistoricalTransport(initialContext)!
                       AsyncOlderLocalLifecyclePrecedesServeIngress(node))

IndexedHistoricalRetransmitRunnerSplit(initialContext, node) ==
  \/ IndexedHistoricalRetransmitNoServeTicketRunnerPrefix(
       initialContext, node)
  \/ IndexedHistoricalRetransmitOlderRuntimePredecessorPrefix(
       initialContext, node)
  \/ IndexedHistoricalRetransmitOlderLocalPredecessorPrefix(
       initialContext, node)
  \/ IndexedHistoricalRetransmitServeTargetCorridorPrefix(
       initialContext, node)

IndexedHistoricalCommitRequestEmissionResidual(
    initialContext, target, server, request) ==
  /\ IndexedHistoricalTransport(initialContext)!
       HistoricalCommitRequestRegistered(target, server, request)
  /\ IndexedHistoricalTransport(initialContext)!
       HistoricalCommitArchiveRouteAvailable(target, server)
  /\ ~IndexedHistoricalTransport(initialContext)!
        HistoricalCommitRequestPacketGoal(target, server, request)

IndexedHistoricalCommitRequestRetransmitArmedResidual(
    initialContext, target, server, request) ==
  /\ IndexedHistoricalCommitRequestEmissionResidual(
       initialContext, target, server, request)
  /\ \/ IndexedHistoricalTransport(initialContext)!
          RetransmitDue(target)
     \/ "RetransmitElapsed"
          \in IndexedHistoricalTransport(initialContext)!
                asyncOutstandingTags[target]

IndexedHistoricalCommitRequestSendingReadyResidual(
    initialContext, target, server, request) ==
  /\ IndexedHistoricalCommitRequestRetransmitArmedResidual(
       initialContext, target, server, request)
  /\ ENABLED
       <<IndexedHistoricalSendingRetransmitLocalStep(
           initialContext, target)>>_(
         IndexedHistoricalTransport(initialContext)!AsyncAllVars)

THEOREM IndexedHistoricalCommitEmissionOwnerHasJoinedFairRunnerDomain ==
  \A initialContext \in AdmissibleContextRecords,
     target, server, request:
    /\ IndexedChainSpec
    /\ IndexedCompositionInvariant
    /\ IndexedHistoricalCommitRequestEmissionResidual(
         initialContext, target, server, request)
    => /\ target \in Responsive
       /\ initialContext \in JoinedContexts
       /\ target \in joinedByContext[initialContext]
       /\ WF_IndexedChainVars(
            IndexedRunHistoricalRecoveryStep(
              initialContext, target))
BY IndexedHistoricalRecoveryTargetHasJoinedActiveOwner, Isa
   DEF IndexedChainSpec, IndexedFairness,
       IndexedHistoricalCommitRequestEmissionResidual,
       IndexedHistoricalTransport!HistoricalCommitRequestRegistered

THEOREM IndexedHistoricalCommitSendingStepPublishesExactPacket ==
  \A initialContext \in AdmissibleContextRecords,
     target, server, request:
    /\ IndexedHistoricalTransport(initialContext)!
         AsyncStrongTypeInvariant
    /\ IndexedHistoricalCommitRequestEmissionResidual(
         initialContext, target, server, request)
    /\ IndexedHistoricalSendingRetransmitLocalStep(
         initialContext, target)
    => IndexedHistoricalTransport(initialContext)!
         HistoricalCommitRequestPacketGoal(
           target, server, request)'
BY IndexedHistoricalTransport(initialContext)!
     HistoricalCommitRetransmissionCreatesExactPacket,
   IsaT(240)
   DEF IndexedHistoricalSendingRetransmitLocalStep,
       IndexedHistoricalCommitRequestEmissionResidual,
       IndexedHistoricalTransport!HistoricalCommitRequestPacketGoal,
       IndexedHistoricalTransport!DirectRetransmitStep,
       IndexedHistoricalTransport!DeferredRetransmitStep,
       IndexedHistoricalTransport!RetryableItems,
       IndexedHistoricalTransport!ActiveRequestItems,
       IndexedHistoricalTransport!PostGstRunHistoricalRecoveryNode,
       IndexedHistoricalTransport!RunHistoricalRecoveryNode,
       IndexedHistoricalTransport!RunNodeWork,
       IndexedHistoricalTransport!LocalAdmissionStep,
       IndexedHistoricalTransport!IngressDrainStep,
       IndexedHistoricalTransport!SerializedRunnerRuntimeStep,
       IndexedHistoricalTransport!SerializedRuntimeStep,
       IndexedHistoricalTransport!
         SerializedRuntimePrecedesServeIngressStep,
       IndexedHistoricalTransport!SerializedLocalPrecedesServeIngressStep,
       IndexedHistoricalTransport!AsyncServeIngressTargetOnlyTurn,
       IndexedHistoricalTransport!SelectedLocalAdmissionAdvance

IndexedHistoricalCommitEmissionClockResidualProperty ==
  \A initialContext \in AdmissibleContextRecords,
     target, server, request:
    IndexedChainSpec
      => (IndexedHistoricalCommitRequestEmissionResidual(
            initialContext, target, server, request)
           ~> (IndexedHistoricalTransport(initialContext)!
                 HistoricalCommitRequestPacketGoal(
                   target, server, request)
                \/ IndexedHistoricalCommitRequestRetransmitArmedResidual(
                     initialContext, target, server, request)))

IndexedHistoricalCommitEmissionRunnerSplitResidualProperty ==
  \A initialContext \in AdmissibleContextRecords,
     target, server, request:
    IndexedChainSpec
      => (IndexedHistoricalCommitRequestRetransmitArmedResidual(
            initialContext, target, server, request)
           ~> (IndexedHistoricalTransport(initialContext)!
                 HistoricalCommitRequestPacketGoal(
                   target, server, request)
                \/ /\ IndexedHistoricalCommitRequestRetransmitArmedResidual(
                        initialContext, target, server, request)
                   /\ IndexedHistoricalRetransmitRunnerSplit(
                        initialContext, target)))

IndexedHistoricalCommitEmissionRuntimePrefixResidualProperty ==
  \A initialContext \in AdmissibleContextRecords,
     target, server, request:
    IndexedChainSpec
      => ((/\ IndexedHistoricalCommitRequestRetransmitArmedResidual(
                 initialContext, target, server, request)
            /\ IndexedHistoricalRetransmitRunnerSplit(
                 initialContext, target))
           ~> (IndexedHistoricalTransport(initialContext)!
                 HistoricalCommitRequestPacketGoal(
                   target, server, request)
                \/ IndexedHistoricalCommitRequestSendingReadyResidual(
                     initialContext, target, server, request)))

IndexedHistoricalCommitEmissionSendingHandoffResidualProperty ==
  \A initialContext \in AdmissibleContextRecords,
     target, server, request:
    IndexedChainSpec
      => (IndexedHistoricalCommitRequestSendingReadyResidual(
            initialContext, target, server, request)
           ~> IndexedHistoricalTransport(initialContext)!
                 HistoricalCommitRequestPacketGoal(
                   target, server, request))

IndexedHistoricalCommitEmissionResidualProperties ==
  /\ IndexedHistoricalCommitEmissionClockResidualProperty
  /\ IndexedHistoricalCommitEmissionRunnerSplitResidualProperty
  /\ IndexedHistoricalCommitEmissionRuntimePrefixResidualProperty
  /\ IndexedHistoricalCommitEmissionSendingHandoffResidualProperty

THEOREM IndexedHistoricalCommitEmissionResidualsCloseKernel ==
  IndexedHistoricalCommitEmissionResidualProperties
    => \A initialContext \in AdmissibleContextRecords:
         IndexedHistoricalTransport(initialContext)!
           HistoricalCommitRequestPacketEmissionKernelProperty(
             IndexedChainSpec)
BY PTL
   DEF IndexedHistoricalCommitEmissionResidualProperties,
       IndexedHistoricalCommitEmissionClockResidualProperty,
       IndexedHistoricalCommitEmissionRunnerSplitResidualProperty,
       IndexedHistoricalCommitEmissionRuntimePrefixResidualProperty,
       IndexedHistoricalCommitEmissionSendingHandoffResidualProperty,
       IndexedHistoricalCommitRequestEmissionResidual,
       IndexedHistoricalCommitRequestRetransmitArmedResidual,
       IndexedHistoricalCommitRequestSendingReadyResidual,
       IndexedHistoricalTransport!
         HistoricalCommitRequestPacketEmissionKernelProperty

IndexedHistoricalDecisionRequestEmissionResidual(
    initialContext, node, qc, archive, request) ==
  /\ IndexedHistoricalTransport(initialContext)!
       HistoricalDecisionBodyHoldingAlias(
         node, qc, archive, request)
  /\ ~IndexedHistoricalTransport(initialContext)!
        HistoricalDecisionRequestPacketGoal(
          node, qc, archive, request)

IndexedHistoricalDecisionRequestRetransmitArmedResidual(
    initialContext, node, qc, archive, request) ==
  /\ IndexedHistoricalDecisionRequestEmissionResidual(
       initialContext, node, qc, archive, request)
  /\ \/ IndexedHistoricalTransport(initialContext)!RetransmitDue(node)
     \/ "RetransmitElapsed"
          \in IndexedHistoricalTransport(initialContext)!
                asyncOutstandingTags[node]

IndexedHistoricalDecisionRequestSendingReadyResidual(
    initialContext, node, qc, archive, request) ==
  /\ IndexedHistoricalDecisionRequestRetransmitArmedResidual(
       initialContext, node, qc, archive, request)
  /\ ENABLED
       <<IndexedHistoricalSendingRetransmitLocalStep(
           initialContext, node)>>_(
         IndexedHistoricalTransport(initialContext)!AsyncAllVars)

THEOREM IndexedHistoricalDecisionEmissionOwnerHasJoinedFairRunnerDomain ==
  \A initialContext \in AdmissibleContextRecords,
     node, qc, archive, request:
    /\ IndexedChainSpec
    /\ IndexedCompositionInvariant
    /\ IndexedHistoricalDecisionRequestEmissionResidual(
         initialContext, node, qc, archive, request)
    => /\ node \in Responsive
       /\ initialContext \in JoinedContexts
       /\ node \in joinedByContext[initialContext]
       /\ WF_IndexedChainVars(
            IndexedRunHistoricalRecoveryStep(initialContext, node))
BY IndexedHistoricalRecoveryTargetHasJoinedActiveOwner, Isa
   DEF IndexedChainSpec, IndexedFairness,
       IndexedHistoricalDecisionRequestEmissionResidual,
       IndexedHistoricalTransport!HistoricalDecisionBodyHoldingAlias,
       IndexedHistoricalTransport!
         HistoricalExactDecisionActiveRequestOwner,
       IndexedHistoricalTransport!HistoricalExactDecisionServiceSource

THEOREM IndexedHistoricalDecisionSendingStepPublishesExactPacket ==
  \A initialContext \in AdmissibleContextRecords,
     node, qc, archive, request:
    /\ IndexedHistoricalTransport(initialContext)!
         AsyncStrongTypeInvariant
    /\ IndexedHistoricalDecisionRequestEmissionResidual(
         initialContext, node, qc, archive, request)
    /\ IndexedHistoricalSendingRetransmitLocalStep(
         initialContext, node)
    => IndexedHistoricalTransport(initialContext)!
         HistoricalDecisionRequestPacketGoal(
           node, qc, archive, request)'
BY IndexedHistoricalTransport(initialContext)!
     HistoricalDecisionRetransmissionCreatesExactRequestPacket,
   IsaT(240)
   DEF IndexedHistoricalSendingRetransmitLocalStep,
       IndexedHistoricalDecisionRequestEmissionResidual,
       IndexedHistoricalTransport!HistoricalDecisionRequestPacketGoal,
       IndexedHistoricalTransport!DirectRetransmitStep,
       IndexedHistoricalTransport!DeferredRetransmitStep,
       IndexedHistoricalTransport!RetryableItems,
       IndexedHistoricalTransport!ActiveRequestItems,
       IndexedHistoricalTransport!PostGstRunHistoricalRecoveryNode,
       IndexedHistoricalTransport!RunHistoricalRecoveryNode,
       IndexedHistoricalTransport!RunNodeWork,
       IndexedHistoricalTransport!LocalAdmissionStep,
       IndexedHistoricalTransport!IngressDrainStep,
       IndexedHistoricalTransport!SerializedRunnerRuntimeStep,
       IndexedHistoricalTransport!SerializedRuntimeStep,
       IndexedHistoricalTransport!
         SerializedRuntimePrecedesServeIngressStep,
       IndexedHistoricalTransport!SerializedLocalPrecedesServeIngressStep,
       IndexedHistoricalTransport!AsyncServeIngressTargetOnlyTurn,
       IndexedHistoricalTransport!SelectedLocalAdmissionAdvance

IndexedHistoricalDecisionEmissionClockResidualProperty ==
  \A initialContext \in AdmissibleContextRecords,
     node, qc, archive, request:
    IndexedChainSpec
      => (IndexedHistoricalDecisionRequestEmissionResidual(
            initialContext, node, qc, archive, request)
           ~> (IndexedHistoricalTransport(initialContext)!
                 HistoricalDecisionRequestPacketGoal(
                   node, qc, archive, request)
                \/ IndexedHistoricalDecisionRequestRetransmitArmedResidual(
                     initialContext, node, qc, archive, request)))

IndexedHistoricalDecisionEmissionRunnerSplitResidualProperty ==
  \A initialContext \in AdmissibleContextRecords,
     node, qc, archive, request:
    IndexedChainSpec
      => (IndexedHistoricalDecisionRequestRetransmitArmedResidual(
            initialContext, node, qc, archive, request)
           ~> (IndexedHistoricalTransport(initialContext)!
                 HistoricalDecisionRequestPacketGoal(
                   node, qc, archive, request)
                \/ /\ IndexedHistoricalDecisionRequestRetransmitArmedResidual(
                        initialContext, node, qc, archive, request)
                   /\ IndexedHistoricalRetransmitRunnerSplit(
                        initialContext, node)))

IndexedHistoricalDecisionEmissionRuntimePrefixResidualProperty ==
  \A initialContext \in AdmissibleContextRecords,
     node, qc, archive, request:
    IndexedChainSpec
      => ((/\ IndexedHistoricalDecisionRequestRetransmitArmedResidual(
                 initialContext, node, qc, archive, request)
            /\ IndexedHistoricalRetransmitRunnerSplit(
                 initialContext, node))
           ~> (IndexedHistoricalTransport(initialContext)!
                 HistoricalDecisionRequestPacketGoal(
                   node, qc, archive, request)
                \/ IndexedHistoricalDecisionRequestSendingReadyResidual(
                     initialContext, node, qc, archive, request)))

IndexedHistoricalDecisionEmissionSendingHandoffResidualProperty ==
  \A initialContext \in AdmissibleContextRecords,
     node, qc, archive, request:
    IndexedChainSpec
      => (IndexedHistoricalDecisionRequestSendingReadyResidual(
            initialContext, node, qc, archive, request)
           ~> IndexedHistoricalTransport(initialContext)!
                 HistoricalDecisionRequestPacketGoal(
                   node, qc, archive, request))

IndexedHistoricalDecisionEmissionResidualProperties ==
  /\ IndexedHistoricalDecisionEmissionClockResidualProperty
  /\ IndexedHistoricalDecisionEmissionRunnerSplitResidualProperty
  /\ IndexedHistoricalDecisionEmissionRuntimePrefixResidualProperty
  /\ IndexedHistoricalDecisionEmissionSendingHandoffResidualProperty

THEOREM IndexedHistoricalDecisionEmissionResidualsCloseKernel ==
  IndexedHistoricalDecisionEmissionResidualProperties
    => \A initialContext \in AdmissibleContextRecords:
         IndexedHistoricalTransport(initialContext)!
           HistoricalDecisionRequestPacketEmissionKernelProperty(
             IndexedChainSpec)
BY PTL
   DEF IndexedHistoricalDecisionEmissionResidualProperties,
       IndexedHistoricalDecisionEmissionClockResidualProperty,
       IndexedHistoricalDecisionEmissionRunnerSplitResidualProperty,
       IndexedHistoricalDecisionEmissionRuntimePrefixResidualProperty,
       IndexedHistoricalDecisionEmissionSendingHandoffResidualProperty,
       IndexedHistoricalDecisionRequestEmissionResidual,
       IndexedHistoricalDecisionRequestRetransmitArmedResidual,
       IndexedHistoricalDecisionRequestSendingReadyResidual,
       IndexedHistoricalTransport!
         HistoricalDecisionRequestPacketEmissionKernelProperty

(***************************************************************************
Exact request packet-to-Serve lifecycle residuals.

The head-gate property consumes the fixed packet deadline/source prefix using
the individually fair indexed Admit/runner/I/O actions.  The admission
handoff consumes only the exact `(recipient, source)` Admit action.  The rank
step then consumes one of `PostGstRunNode(archive)`,
`PostGstRunHistoricalServer(archive)`, or
`PostGstServiceIoWorker(archive)` according to the immutable lifecycle owner.
The archive identity is fixed by the route/body alias.  Its exact local
activation and joined membership are a separate residual; no
all-responsive-joined premise is permitted.  A later Serve ticket or Runtime
producer is handled by the shared scheduler ordinal and
`SerializedRuntimePrecedesServeIngressExactFrame`, not by fairness of an
intersection action.
***************************************************************************)

IndexedHistoricalCommitRequestPacketResidual(
    initialContext, target, server, request, packet) ==
  /\ IndexedHistoricalTransport(initialContext)!
       HistoricalCommitRequestPacketOwned(
         target, server, request, packet)
  /\ IndexedHistoricalTransport(initialContext)!
       HistoricalCommitArchiveRouteAvailable(target, server)
  /\ ~IndexedHistoricalTransport(initialContext)!
        HistoricalCommitRequestServeGoal(target, server, request)

IndexedHistoricalCommitRequestPacketAdmissionReady(
    initialContext, target, server, request, packet) ==
  /\ IndexedHistoricalCommitRequestPacketResidual(
       initialContext, target, server, request, packet)
  /\ packet =
       IndexedHistoricalTransport(initialContext)!
         OldestDueSourcePacket(server, request.source)
  /\ ENABLED
       <<IndexedHistoricalTransport(initialContext)!
           PostGstAdmitHiddenPacket(
             server, request.source)>>_(
         IndexedHistoricalTransport(initialContext)!AsyncAllVars)

IndexedHistoricalCommitRequestAdmissionOutcome(
    initialContext, target, server, request) ==
  \/ IndexedHistoricalTransport(initialContext)!
       HistoricalCommitRequestServeGoal(target, server, request)
  \/ IndexedHistoricalTransport(initialContext)!
       HistoricalCommitRequestLifecycleResidual(
         target, server, request)

THEOREM IndexedHistoricalCommitPacketAdmissionHasExactLifecycleOutcome ==
  \A initialContext \in AdmissibleContextRecords,
     target, server, request, packet:
    /\ IndexedCompositionInvariant
    /\ IndexedHistoricalTransport(initialContext)!
         AsyncStrongTypeInvariant
    /\ IndexedHistoricalCommitRequestPacketResidual(
         initialContext, target, server, request, packet)
    /\ packet =
         IndexedHistoricalTransport(initialContext)!
           OldestDueSourcePacket(server, request.source)
    /\ IndexedAdmitPacketStep(
         initialContext, server, request.source)
    => IndexedHistoricalCommitRequestAdmissionOutcome(
         initialContext, target, server, request)'
BY IndexedFairProductStepsProjectExactOccurrences,
   IndexedHistoricalTransport(initialContext)!
     HistoricalCommitPacketAdmissionCreatesExactIngressOwner,
   IndexedHistoricalArchiveRoutePersistsUntilSemanticExit,
   IsaT(300)
   DEF IndexedHistoricalCommitRequestPacketResidual,
       IndexedHistoricalCommitRequestAdmissionOutcome,
       IndexedHistoricalTransport!
         HistoricalCommitRequestLifecycleResidual,
       IndexedHistoricalTransport!HistoricalCommitRequestServeGoal,
       IndexedHistoricalTransport!HistoricalCommitTransportGoal,
       IndexedHistoricalTransport!PostGstAdmitHiddenPacket,
       IndexedHistoricalTransport!AsyncAllVars,
       IndexedAsyncStateAt, IndexedAdmitPacketStep

IndexedHistoricalCommitLifecycleAtRank(
    initialContext, target, server, request, rank) ==
  /\ IndexedHistoricalTransport(initialContext)!
       AsyncStrongTypeInvariant
  /\ IndexedHistoricalTransport(initialContext)!
       HistoricalCommitRequestLifecycleResidual(
         target, server, request)
  /\ rank = IndexedHistoricalTransport(initialContext)!
              HistoricalCommitRequestLifecycleRank(
                target, server, request)
  /\ rank \in IndexedHistoricalTransport(initialContext)!
               HistoricalCommitRequestLifecycleRankCarrier

IndexedHistoricalCommitLifecycleArchiveOwnerReady(
    initialContext, server) ==
  /\ initialContext \in JoinedContexts
  /\ server \in Responsive
  /\ server \in joinedByContext[initialContext]
  /\ server \in IndexedHistoricalTransport(initialContext)!
                 AsyncActiveServiceNodes
  /\ server \in IndexedHistoricalTransport(initialContext)!
                 AsyncVotersAt(initialContext)

IndexedHistoricalCommitLifecycleArchiveActivationResidualProperty ==
  \A initialContext \in AdmissibleContextRecords,
     target, server, request:
    IndexedChainSpec
      => (IndexedHistoricalTransport(initialContext)!
            HistoricalCommitRequestLifecycleResidual(
              target, server, request)
           ~> (IndexedHistoricalTransport(initialContext)!
                 HistoricalCommitRequestServeGoal(
                   target, server, request)
                \/ IndexedHistoricalCommitLifecycleArchiveOwnerReady(
                     initialContext, server)))

THEOREM IndexedHistoricalCommitLifecycleReadyOwnerHasExactFairActions ==
  \A initialContext \in AdmissibleContextRecords, server:
    /\ IndexedChainSpec
    /\ IndexedHistoricalCommitLifecycleArchiveOwnerReady(
         initialContext, server)
    => /\ WF_IndexedChainVars(
              IndexedRunNodeStep(initialContext, server))
       /\ WF_IndexedChainVars(
              IndexedHistoricalServerStep(initialContext, server))
       /\ WF_IndexedChainVars(
              IndexedIoWorkerStep(initialContext, server))
BY Isa
   DEF IndexedChainSpec, IndexedFairness,
       IndexedHistoricalCommitLifecycleArchiveOwnerReady,
       IndexedHistoricalTransport!AsyncVotersAt,
       IndexedAsync!AsyncVotersAt

IndexedHistoricalCommitLifecycleRankGoal(
    initialContext, target, server, request, rank) ==
  \/ IndexedHistoricalTransport(initialContext)!
       HistoricalCommitRequestServeGoal(target, server, request)
  \/ \E lower \in SetLessThan(
       rank,
       IndexedHistoricalTransport(initialContext)!
         HistoricalCommitRequestLifecycleRankOrdering,
       IndexedHistoricalTransport(initialContext)!
         HistoricalCommitRequestLifecycleRankCarrier):
       IndexedHistoricalCommitLifecycleAtRank(
         initialContext, target, server, request, lower)

IndexedHistoricalCommitLifecycleRankStepResidualProperty ==
  \A initialContext \in AdmissibleContextRecords,
     target, server, request:
    IndexedChainSpec
      => \A rank \in
           IndexedHistoricalTransport(initialContext)!
             HistoricalCommitRequestLifecycleRankCarrier:
           IndexedHistoricalCommitLifecycleAtRank(
             initialContext, target, server, request, rank)
             ~> IndexedHistoricalCommitLifecycleRankGoal(
                  initialContext, target, server, request, rank)

THEOREM IndexedHistoricalCommitLifecycleRankStepClosesLifecycle ==
  IndexedHistoricalCommitLifecycleRankStepResidualProperty
    => \A initialContext \in AdmissibleContextRecords,
         target, server, request:
         IndexedChainSpec
           => (IndexedHistoricalTransport(initialContext)!
                 HistoricalCommitRequestLifecycleResidual(
                   target, server, request)
                ~> IndexedHistoricalTransport(initialContext)!
                      HistoricalCommitRequestServeGoal(
                        target, server, request))
PROOF
  <1>1. ASSUME IndexedHistoricalCommitLifecycleRankStepResidualProperty,
                NEW initialContext \in AdmissibleContextRecords,
                NEW target, NEW server, NEW request,
                IndexedChainSpec
         PROVE IndexedHistoricalTransport(initialContext)!
                 HistoricalCommitRequestLifecycleResidual(
                   target, server, request)
                 ~>
               IndexedHistoricalTransport(initialContext)!
                 HistoricalCommitRequestServeGoal(
                   target, server, request)
    <2>1. \A rank \in
                 IndexedHistoricalTransport(initialContext)!
                   HistoricalCommitRequestLifecycleRankCarrier:
             IndexedHistoricalCommitLifecycleAtRank(
               initialContext, target, server, request, rank)
               ~> IndexedHistoricalCommitLifecycleRankGoal(
                    initialContext, target, server, request, rank)
      BY <1>1
         DEF IndexedHistoricalCommitLifecycleRankStepResidualProperty
    <2>2. \A rank \in
                 IndexedHistoricalTransport(initialContext)!
                   HistoricalCommitRequestLifecycleRankCarrier:
             IndexedHistoricalCommitLifecycleAtRank(
               initialContext, target, server, request, rank)
               ~> IndexedHistoricalTransport(initialContext)!
                     HistoricalCommitRequestServeGoal(
                       target, server, request)
      BY <2>1,
         IndexedHistoricalTransport(initialContext)!
           HistoricalCommitRequestLifecycleRankOrderingIsWellFounded,
         WellFoundedLeadsTo
         DEF IndexedHistoricalCommitLifecycleRankGoal
    <2>3. []IndexedHistoricalTransport(initialContext)!
             AsyncStrongTypeInvariant
      BY <1>1, IndexedChainSpecAlwaysHistoricalTemporalSupport, PTL
         DEF IndexedHistoricalTemporalSupportAt
    <2>4. [](IndexedHistoricalTransport(initialContext)!
               HistoricalCommitRequestLifecycleResidual(
                 target, server, request)
              => \E rank \in
                   IndexedHistoricalTransport(initialContext)!
                     HistoricalCommitRequestLifecycleRankCarrier:
                   IndexedHistoricalCommitLifecycleAtRank(
                     initialContext, target, server, request, rank))
      BY <2>3,
         IndexedHistoricalTransport(initialContext)!
           HistoricalCommitRequestLifecycleRankInCarrier,
         PTL
         DEF IndexedHistoricalCommitLifecycleAtRank
    <2> QED BY <2>2, <2>4, PTL
  <1> QED BY <1>1

IndexedHistoricalCommitRequestHeadGateResidualProperty ==
  \A initialContext \in AdmissibleContextRecords,
     target, server, request, packet:
    IndexedChainSpec
      => (IndexedHistoricalCommitRequestPacketResidual(
            initialContext, target, server, request, packet)
           ~> (IndexedHistoricalTransport(initialContext)!
                 HistoricalCommitRequestServeGoal(
                   target, server, request)
                \/ IndexedHistoricalCommitRequestPacketAdmissionReady(
                     initialContext, target, server, request, packet)))

IndexedHistoricalCommitRequestAdmissionHandoffResidualProperty ==
  \A initialContext \in AdmissibleContextRecords,
     target, server, request, packet:
    IndexedChainSpec
      => (IndexedHistoricalCommitRequestPacketAdmissionReady(
            initialContext, target, server, request, packet)
           ~> IndexedHistoricalCommitRequestAdmissionOutcome(
                 initialContext, target, server, request))

IndexedHistoricalCommitRequestIngressResidualProperties ==
  /\ IndexedHistoricalCommitRequestHeadGateResidualProperty
  /\ IndexedHistoricalCommitRequestAdmissionHandoffResidualProperty
  /\ IndexedHistoricalCommitLifecycleArchiveActivationResidualProperty
  /\ IndexedHistoricalCommitLifecycleRankStepResidualProperty

THEOREM IndexedHistoricalCommitIngressResidualsCloseKernel ==
  IndexedHistoricalCommitRequestIngressResidualProperties
    => \A initialContext \in AdmissibleContextRecords:
         IndexedHistoricalTransport(initialContext)!
           HistoricalCommitRequestIngressKernelProperty(
             IndexedChainSpec)
BY IndexedHistoricalCommitLifecycleRankStepClosesLifecycle, PTL
   DEF IndexedHistoricalCommitRequestIngressResidualProperties,
       IndexedHistoricalCommitRequestHeadGateResidualProperty,
       IndexedHistoricalCommitRequestAdmissionHandoffResidualProperty,
       IndexedHistoricalCommitLifecycleArchiveActivationResidualProperty,
       IndexedHistoricalCommitRequestPacketResidual,
       IndexedHistoricalCommitRequestAdmissionOutcome,
       IndexedHistoricalTransport!
         HistoricalCommitRequestIngressKernelProperty,
       IndexedHistoricalTransport!HistoricalCommitRequestServeGoal

IndexedHistoricalDecisionRequestPacketResidual(
    initialContext, node, qc, archive, request, packet) ==
  /\ IndexedHistoricalTransport(initialContext)!
       HistoricalDecisionRequestPacketOwned(
         node, qc, archive, request, packet)
  /\ ~IndexedHistoricalTransport(initialContext)!
        HistoricalDecisionRequestServeGoal(
          node, qc, archive, request)

IndexedHistoricalDecisionRequestPacketAdmissionReady(
    initialContext, node, qc, archive, request, packet) ==
  /\ IndexedHistoricalDecisionRequestPacketResidual(
       initialContext, node, qc, archive, request, packet)
  /\ packet =
       IndexedHistoricalTransport(initialContext)!
         OldestDueSourcePacket(archive, request.source)
  /\ ENABLED
       <<IndexedHistoricalTransport(initialContext)!
           PostGstAdmitHiddenPacket(
             archive, request.source)>>_(
         IndexedHistoricalTransport(initialContext)!AsyncAllVars)

IndexedHistoricalDecisionRequestAdmissionOutcome(
    initialContext, node, qc, archive, request) ==
  \/ IndexedHistoricalTransport(initialContext)!
       HistoricalDecisionRequestServeGoal(
         node, qc, archive, request)
  \/ IndexedHistoricalTransport(initialContext)!
       HistoricalDecisionRequestLifecycleResidual(
         node, qc, archive, request)

THEOREM IndexedHistoricalDecisionPacketAdmissionHasExactLifecycleOutcome ==
  \A initialContext \in AdmissibleContextRecords,
     node, qc, archive, request, packet:
    /\ IndexedHistoricalTransport(initialContext)!
         AsyncStrongTypeInvariant
    /\ IndexedHistoricalDecisionRequestPacketResidual(
         initialContext, node, qc, archive, request, packet)
    /\ packet =
         IndexedHistoricalTransport(initialContext)!
           OldestDueSourcePacket(archive, request.source)
    /\ IndexedAdmitPacketStep(
         initialContext, archive, request.source)
    => IndexedHistoricalDecisionRequestAdmissionOutcome(
         initialContext, node, qc, archive, request)'
BY IndexedFairProductStepsProjectExactOccurrences,
   IndexedHistoricalTransport(initialContext)!
     HistoricalDecisionRequestPacketCreatesIngressOwner,
   IsaT(300)
   DEF IndexedHistoricalDecisionRequestPacketResidual,
       IndexedHistoricalDecisionRequestAdmissionOutcome,
       IndexedHistoricalTransport!
         HistoricalDecisionRequestLifecycleResidual,
       IndexedHistoricalTransport!HistoricalDecisionRequestServeGoal,
       IndexedHistoricalTransport!PostGstAdmitHiddenPacket,
       IndexedHistoricalTransport!AsyncAllVars,
       IndexedAsyncStateAt, IndexedAdmitPacketStep

IndexedHistoricalDecisionLifecycleAtRank(
    initialContext, node, qc, archive, request, rank) ==
  /\ IndexedHistoricalTransport(initialContext)!
       AsyncStrongTypeInvariant
  /\ IndexedHistoricalTransport(initialContext)!
       HistoricalDecisionRequestLifecycleResidual(
         node, qc, archive, request)
  /\ rank = IndexedHistoricalTransport(initialContext)!
              HistoricalDecisionRequestLifecycleRank(
                node, qc, archive, request)
  /\ rank \in IndexedHistoricalTransport(initialContext)!
               HistoricalDecisionRequestLifecycleRankCarrier

IndexedHistoricalDecisionLifecycleArchiveOwnerReady(
    initialContext, archive) ==
  /\ initialContext \in JoinedContexts
  /\ archive \in Responsive
  /\ archive \in joinedByContext[initialContext]
  /\ archive \in IndexedHistoricalTransport(initialContext)!
                  AsyncActiveServiceNodes
  /\ archive \in IndexedHistoricalTransport(initialContext)!
                  AsyncVotersAt(initialContext)

IndexedHistoricalDecisionLifecycleArchiveActivationResidualProperty ==
  \A initialContext \in AdmissibleContextRecords,
     node, qc, archive, request:
    IndexedChainSpec
      => (IndexedHistoricalTransport(initialContext)!
            HistoricalDecisionRequestLifecycleResidual(
              node, qc, archive, request)
           ~> (IndexedHistoricalTransport(initialContext)!
                 HistoricalDecisionRequestServeGoal(
                   node, qc, archive, request)
                \/ IndexedHistoricalDecisionLifecycleArchiveOwnerReady(
                     initialContext, archive)))

THEOREM IndexedHistoricalDecisionLifecycleReadyOwnerHasExactFairActions ==
  \A initialContext \in AdmissibleContextRecords, archive:
    /\ IndexedChainSpec
    /\ IndexedHistoricalDecisionLifecycleArchiveOwnerReady(
         initialContext, archive)
    => /\ WF_IndexedChainVars(
              IndexedRunNodeStep(initialContext, archive))
       /\ WF_IndexedChainVars(
              IndexedHistoricalServerStep(initialContext, archive))
       /\ WF_IndexedChainVars(
              IndexedIoWorkerStep(initialContext, archive))
BY Isa
   DEF IndexedChainSpec, IndexedFairness,
       IndexedHistoricalDecisionLifecycleArchiveOwnerReady,
       IndexedHistoricalTransport!AsyncVotersAt,
       IndexedAsync!AsyncVotersAt

IndexedHistoricalDecisionLifecycleRankGoal(
    initialContext, node, qc, archive, request, rank) ==
  \/ IndexedHistoricalTransport(initialContext)!
       HistoricalDecisionRequestServeGoal(
         node, qc, archive, request)
  \/ \E lower \in SetLessThan(
       rank,
       IndexedHistoricalTransport(initialContext)!
         HistoricalDecisionRequestLifecycleRankOrdering,
       IndexedHistoricalTransport(initialContext)!
         HistoricalDecisionRequestLifecycleRankCarrier):
       IndexedHistoricalDecisionLifecycleAtRank(
         initialContext, node, qc, archive, request, lower)

IndexedHistoricalDecisionLifecycleRankStepResidualProperty ==
  \A initialContext \in AdmissibleContextRecords,
     node, qc, archive, request:
    IndexedChainSpec
      => \A rank \in
           IndexedHistoricalTransport(initialContext)!
             HistoricalDecisionRequestLifecycleRankCarrier:
           IndexedHistoricalDecisionLifecycleAtRank(
             initialContext, node, qc, archive, request, rank)
             ~> IndexedHistoricalDecisionLifecycleRankGoal(
                  initialContext, node, qc, archive, request, rank)

THEOREM IndexedHistoricalDecisionLifecycleRankStepClosesLifecycle ==
  IndexedHistoricalDecisionLifecycleRankStepResidualProperty
    => \A initialContext \in AdmissibleContextRecords,
         node, qc, archive, request:
         IndexedChainSpec
           => (IndexedHistoricalTransport(initialContext)!
                 HistoricalDecisionRequestLifecycleResidual(
                   node, qc, archive, request)
                ~> IndexedHistoricalTransport(initialContext)!
                      HistoricalDecisionRequestServeGoal(
                        node, qc, archive, request))
PROOF
  <1>1. ASSUME IndexedHistoricalDecisionLifecycleRankStepResidualProperty,
                NEW initialContext \in AdmissibleContextRecords,
                NEW node, NEW qc, NEW archive, NEW request,
                IndexedChainSpec
         PROVE IndexedHistoricalTransport(initialContext)!
                 HistoricalDecisionRequestLifecycleResidual(
                   node, qc, archive, request)
                 ~>
               IndexedHistoricalTransport(initialContext)!
                 HistoricalDecisionRequestServeGoal(
                   node, qc, archive, request)
    <2>1. \A rank \in
                 IndexedHistoricalTransport(initialContext)!
                   HistoricalDecisionRequestLifecycleRankCarrier:
             IndexedHistoricalDecisionLifecycleAtRank(
               initialContext, node, qc, archive, request, rank)
               ~> IndexedHistoricalDecisionLifecycleRankGoal(
                    initialContext, node, qc, archive, request, rank)
      BY <1>1
         DEF IndexedHistoricalDecisionLifecycleRankStepResidualProperty
    <2>2. \A rank \in
                 IndexedHistoricalTransport(initialContext)!
                   HistoricalDecisionRequestLifecycleRankCarrier:
             IndexedHistoricalDecisionLifecycleAtRank(
               initialContext, node, qc, archive, request, rank)
               ~> IndexedHistoricalTransport(initialContext)!
                     HistoricalDecisionRequestServeGoal(
                       node, qc, archive, request)
      BY <2>1,
         IndexedHistoricalTransport(initialContext)!
           HistoricalDecisionRequestLifecycleRankOrderingIsWellFounded,
         WellFoundedLeadsTo
         DEF IndexedHistoricalDecisionLifecycleRankGoal
    <2>3. []IndexedHistoricalTransport(initialContext)!
             AsyncStrongTypeInvariant
      BY <1>1, IndexedChainSpecAlwaysHistoricalTemporalSupport, PTL
         DEF IndexedHistoricalTemporalSupportAt
    <2>4. [](IndexedHistoricalTransport(initialContext)!
               HistoricalDecisionRequestLifecycleResidual(
                 node, qc, archive, request)
              => \E rank \in
                   IndexedHistoricalTransport(initialContext)!
                     HistoricalDecisionRequestLifecycleRankCarrier:
                   IndexedHistoricalDecisionLifecycleAtRank(
                     initialContext, node, qc,
                     archive, request, rank))
      BY <2>3,
         IndexedHistoricalTransport(initialContext)!
           HistoricalDecisionRequestLifecycleRankInCarrier,
         PTL
         DEF IndexedHistoricalDecisionLifecycleAtRank
    <2> QED BY <2>2, <2>4, PTL
  <1> QED BY <1>1

IndexedHistoricalDecisionRequestHeadGateResidualProperty ==
  \A initialContext \in AdmissibleContextRecords,
     node, qc, archive, request, packet:
    IndexedChainSpec
      => (IndexedHistoricalDecisionRequestPacketResidual(
            initialContext, node, qc, archive, request, packet)
           ~> (IndexedHistoricalTransport(initialContext)!
                 HistoricalDecisionRequestServeGoal(
                   node, qc, archive, request)
                \/ IndexedHistoricalDecisionRequestPacketAdmissionReady(
                     initialContext, node, qc,
                     archive, request, packet)))

IndexedHistoricalDecisionRequestAdmissionHandoffResidualProperty ==
  \A initialContext \in AdmissibleContextRecords,
     node, qc, archive, request, packet:
    IndexedChainSpec
      => (IndexedHistoricalDecisionRequestPacketAdmissionReady(
            initialContext, node, qc, archive, request, packet)
           ~> IndexedHistoricalDecisionRequestAdmissionOutcome(
                 initialContext, node, qc, archive, request))

IndexedHistoricalDecisionRequestIngressResidualProperties ==
  /\ IndexedHistoricalDecisionRequestHeadGateResidualProperty
  /\ IndexedHistoricalDecisionRequestAdmissionHandoffResidualProperty
  /\ IndexedHistoricalDecisionLifecycleArchiveActivationResidualProperty
  /\ IndexedHistoricalDecisionLifecycleRankStepResidualProperty

THEOREM IndexedHistoricalDecisionIngressResidualsCloseKernel ==
  IndexedHistoricalDecisionRequestIngressResidualProperties
    => \A initialContext \in AdmissibleContextRecords:
         IndexedHistoricalTransport(initialContext)!
           HistoricalDecisionRequestIngressKernelProperty(
             IndexedChainSpec)
BY IndexedHistoricalDecisionLifecycleRankStepClosesLifecycle, PTL
   DEF IndexedHistoricalDecisionRequestIngressResidualProperties,
       IndexedHistoricalDecisionRequestHeadGateResidualProperty,
       IndexedHistoricalDecisionRequestAdmissionHandoffResidualProperty,
       IndexedHistoricalDecisionLifecycleArchiveActivationResidualProperty,
       IndexedHistoricalDecisionRequestPacketResidual,
       IndexedHistoricalDecisionRequestAdmissionOutcome,
       IndexedHistoricalTransport!
         HistoricalDecisionRequestIngressKernelProperty,
       IndexedHistoricalTransport!HistoricalDecisionRequestServeGoal

(***************************************************************************
Exact response admission residuals.

For Commit responses the target packet uses the historical admission action,
then the exact response occurrence must reach the selected fair-ingress slot
of `PostGstRunHistoricalRecoveryNode(target)`.  For Decision responses the
same historical runner consumes a recipient-local route-neutral claim.  The
Decision head property below intentionally includes all three physical
partitions: older outer-source packets, a distinct authenticated claim, and
`TransportCompletionOwnerDebt(response)`.  The missing theorem must descend
those owners under the full historical runner/admission actions; fairness of
the fresh-admission or selected-drain intersection is not available.
***************************************************************************)

IndexedHistoricalCommitResponsePacketResidual(
    initialContext, target, server, request, qc, response, packet) ==
  /\ IndexedHistoricalTransport(initialContext)!
       HistoricalCommitResponsePacketOwned(
         target, server, request, qc, response, packet)
  /\ ~IndexedHistoricalTransport(initialContext)!
        HistoricalCommitTransportGoal(target)

IndexedHistoricalCommitResponseIngressResidual(
    initialContext, target, server, request, qc, response) ==
  /\ IndexedHistoricalTransport(initialContext)!
       HistoricalCommitResponseIngressOwned(
         target, server, request, qc, response)
  /\ ~IndexedHistoricalTransport(initialContext)!
        HistoricalCommitTransportGoal(target)

IndexedHistoricalCommitResponseRunnerOwnerResidual(
    initialContext, target, server, request, qc, response, packet) ==
  \/ IndexedHistoricalCommitResponsePacketResidual(
       initialContext, target, server, request, qc, response, packet)
  \/ IndexedHistoricalCommitResponseIngressResidual(
       initialContext, target, server, request, qc, response)

THEOREM IndexedHistoricalCommitResponseHasJoinedFairRunnerOwner ==
  \A initialContext \in AdmissibleContextRecords,
     target, server, request, qc, response, packet:
    /\ IndexedChainSpec
    /\ IndexedCompositionInvariant
    /\ IndexedHistoricalCommitResponseRunnerOwnerResidual(
         initialContext, target, server, request, qc, response, packet)
    => /\ target \in Responsive
       /\ initialContext \in JoinedContexts
       /\ target \in joinedByContext[initialContext]
       /\ target \in IndexedHistoricalTransport(initialContext)!
                      AsyncActiveServiceNodes
       /\ WF_IndexedChainVars(
            IndexedRunHistoricalRecoveryStep(initialContext, target))
BY IndexedHistoricalRecoveryTargetHasJoinedActiveOwner, IsaT(180)
   DEF IndexedChainSpec, IndexedFairness,
       IndexedHistoricalCommitResponseRunnerOwnerResidual,
       IndexedHistoricalCommitResponsePacketResidual,
       IndexedHistoricalCommitResponseIngressResidual,
       IndexedHistoricalTransport!HistoricalCommitResponsePacketOwned,
       IndexedHistoricalTransport!HistoricalCommitResponseIngressOwned,
       IndexedHistoricalTransport!HistoricalCommitResponseLineage,
       IndexedHistoricalTransport!HistoricalCommitRequestRegistered

THEOREM IndexedHistoricalCommitResponseAdmissionCreatesExactIngressOwner ==
  \A initialContext \in AdmissibleContextRecords,
     target, server, request, qc, response, packet:
    /\ IndexedHistoricalTransport(initialContext)!
         AsyncStrongTypeInvariant
    /\ IndexedHistoricalCommitResponsePacketResidual(
         initialContext, target, server, request,
         qc, response, packet)
    /\ packet = IndexedHistoricalTransport(initialContext)!
                  OldestDueSourcePacket(target, response.source)
    /\ IndexedAdmitHistoricalRecoveryPacketStep(
         initialContext, target, response.source)
    => IndexedHistoricalTransport(initialContext)!
         HistoricalCommitResponseIngressOwned(
           target, server, request, qc, response)'
BY IndexedFairProductStepsProjectExactOccurrences,
   IndexedHistoricalTransport(initialContext)!
     HistoricalCommitResponsePacketAdmissionCreatesIngressOwner,
   IsaT(240)
   DEF IndexedHistoricalCommitResponsePacketResidual,
       IndexedHistoricalTransport!
         PostGstAdmitHistoricalRecoveryPacket,
       IndexedHistoricalTransport!AsyncAllVars,
       IndexedAsyncStateAt,
       IndexedAdmitHistoricalRecoveryPacketStep

THEOREM IndexedHistoricalCommitSelectedResponseDrainCreatesGoal ==
  \A initialContext \in AdmissibleContextRecords,
     target, server, request, qc, response:
    /\ IndexedHistoricalTransport(initialContext)!
         AsyncStrongTypeInvariant
    /\ IndexedHistoricalTransport(initialContext)!
         AsyncProgressOwnershipInvariant
    /\ IndexedHistoricalCommitResponseIngressResidual(
         initialContext, target, server, request, qc, response)
    /\ IndexedHistoricalTransport(initialContext)!
         SelectedIngressItemAt(
           target,
           IndexedHistoricalTransport(initialContext)!
             FirstDrainableIngressIndex(target)) = response
    /\ IndexedHistoricalTransport(initialContext)!
         DrainFairIngressSelected(target)
    => IndexedHistoricalTransport(initialContext)!
         HistoricalCommitTransportGoal(target)'
BY IndexedHistoricalTransport(initialContext)!
     HistoricalCommitResponseIngressCreatesExactDeliverQcOwner,
   Isa
   DEF IndexedHistoricalCommitResponseIngressResidual,
       IndexedHistoricalTransport!HistoricalCommitTransportGoal

IndexedHistoricalCommitResponseHeadGateResidualProperty ==
  \A initialContext \in AdmissibleContextRecords,
     target, server, request, qc, response, packet:
    IndexedChainSpec
      => (IndexedHistoricalCommitResponsePacketResidual(
            initialContext, target, server, request,
            qc, response, packet)
           ~> (IndexedHistoricalTransport(initialContext)!
                 HistoricalCommitTransportGoal(target)
                \/ IndexedHistoricalCommitResponseIngressResidual(
                     initialContext, target, server,
                     request, qc, response)))

IndexedHistoricalCommitResponseIngressRunnerResidualProperty ==
  \A initialContext \in AdmissibleContextRecords,
     target, server, request, qc, response:
    IndexedChainSpec
      => (IndexedHistoricalCommitResponseIngressResidual(
            initialContext, target, server, request, qc, response)
           ~> IndexedHistoricalTransport(initialContext)!
                 HistoricalCommitTransportGoal(target))

IndexedHistoricalCommitResponseAdmissionResidualProperties ==
  /\ IndexedHistoricalCommitResponseHeadGateResidualProperty
  /\ IndexedHistoricalCommitResponseIngressRunnerResidualProperty

THEOREM IndexedHistoricalCommitResponseResidualsCloseKernel ==
  IndexedHistoricalCommitResponseAdmissionResidualProperties
    => \A initialContext \in AdmissibleContextRecords:
         IndexedHistoricalTransport(initialContext)!
           HistoricalCommitResponseAdmissionKernelProperty(
             IndexedChainSpec)
BY PTL
   DEF IndexedHistoricalCommitResponseAdmissionResidualProperties,
       IndexedHistoricalCommitResponseHeadGateResidualProperty,
       IndexedHistoricalCommitResponseIngressRunnerResidualProperty,
       IndexedHistoricalCommitResponsePacketResidual,
       IndexedHistoricalCommitResponseIngressResidual,
       IndexedHistoricalTransport!
         HistoricalCommitResponseAdmissionKernelProperty

IndexedHistoricalDecisionResponsePacketResidual(
    initialContext, node, qc, archive, request, response, packet) ==
  /\ IndexedHistoricalTransport(initialContext)!
       HistoricalDecisionResponsePacketOwned(
         node, qc, archive, request, response, packet)
  /\ ~IndexedHistoricalTransport(initialContext)!
        HistoricalDecisionCertifiedResponseGoal(node, qc)

IndexedHistoricalDecisionResponseClaimResidual(
    initialContext, node, qc, response) ==
  /\ IndexedHistoricalTransport(initialContext)!
       HistoricalDecisionRouteNeutralClaimIngressOwned(
         node, qc, response)
  /\ ~IndexedHistoricalTransport(initialContext)!
        HistoricalDecisionCertifiedResponseGoal(node, qc)

IndexedHistoricalDecisionResponsePhysicalCompletionResidual(
    initialContext, node, qc, archive, request, response, packet) ==
  /\ IndexedHistoricalDecisionResponsePacketResidual(
       initialContext, node, qc, archive, request, response, packet)
  /\ packet = IndexedHistoricalTransport(initialContext)!
                OldestDueSourcePacket(node, response.source)
  /\ IndexedHistoricalTransport(initialContext)!
       CertifiedResponseFreshClaimGateAllows(response)
  /\ ~IndexedHistoricalTransport(initialContext)!
        AsyncTransportCompletionOwnerGateAllows(response)

IndexedHistoricalDecisionResponseRunnerOwnerResidual(
    initialContext, node, qc, archive, request, response, packet) ==
  \/ IndexedHistoricalDecisionResponsePacketResidual(
       initialContext, node, qc, archive, request, response, packet)
  \/ IndexedHistoricalDecisionResponseClaimResidual(
       initialContext, node, qc, response)

THEOREM IndexedHistoricalDecisionResponseHasJoinedFairRunnerOwner ==
  \A initialContext \in AdmissibleContextRecords,
     node, qc, archive, request, response, packet:
    /\ IndexedChainSpec
    /\ IndexedCompositionInvariant
    /\ IndexedHistoricalDecisionResponseRunnerOwnerResidual(
         initialContext, node, qc, archive, request, response, packet)
    => /\ node \in Responsive
       /\ initialContext \in JoinedContexts
       /\ node \in joinedByContext[initialContext]
       /\ node \in IndexedHistoricalTransport(initialContext)!
                    AsyncActiveServiceNodes
       /\ WF_IndexedChainVars(
            IndexedRunHistoricalRecoveryStep(initialContext, node))
BY IndexedHistoricalRecoveryTargetHasJoinedActiveOwner, IsaT(180)
   DEF IndexedChainSpec, IndexedFairness,
       IndexedHistoricalDecisionResponseRunnerOwnerResidual,
       IndexedHistoricalDecisionResponsePacketResidual,
       IndexedHistoricalDecisionResponseClaimResidual,
       IndexedHistoricalTransport!HistoricalDecisionResponsePacketOwned,
       IndexedHistoricalTransport!HistoricalDecisionAuthenticatedResponse,
       IndexedHistoricalTransport!HistoricalDecisionBodyHoldingAlias,
       IndexedHistoricalTransport!
         HistoricalDecisionRouteNeutralClaimIngressOwned,
       IndexedHistoricalTransport!HistoricalExactDecisionServiceSource

THEOREM IndexedHistoricalDecisionResponseAdmissionCreatesRouteNeutralClaim ==
  \A initialContext \in AdmissibleContextRecords,
     node, qc, archive, request, response, packet:
    /\ IndexedHistoricalTransport(initialContext)!
         AsyncStrongTypeInvariant
    /\ IndexedHistoricalDecisionResponsePacketResidual(
         initialContext, node, qc, archive,
         request, response, packet)
    /\ packet = IndexedHistoricalTransport(initialContext)!
                  OldestDueSourcePacket(node, response.source)
    /\ IndexedAdmitHistoricalRecoveryPacketStep(
         initialContext, node, response.source)
    => IndexedHistoricalTransport(initialContext)!
         HistoricalDecisionRouteNeutralClaimIngressOwned(
           node, qc, response)'
BY IndexedFairProductStepsProjectExactOccurrences,
   IndexedHistoricalTransport(initialContext)!
     FreshHistoricalDecisionResponseAcquiresExactIngressOwner,
   IndexedHistoricalTransport(initialContext)!
     CoalescedHistoricalDecisionResponseRetainsRouteNeutralOwner,
   IsaT(300)
   DEF IndexedHistoricalDecisionResponsePacketResidual,
       IndexedHistoricalTransport!
         HistoricalDecisionRouteNeutralClaimIngressOwned,
       IndexedHistoricalTransport!
         HistoricalDecisionClaimedResponseIngressOwned,
       IndexedHistoricalTransport!
         PostGstAdmitHistoricalRecoveryPacket,
       IndexedHistoricalTransport!AdmitIngressPacket,
       IndexedHistoricalTransport!AdmitHiddenPacket,
       IndexedHistoricalTransport!AsyncAllVars,
       IndexedAsyncStateAt,
       IndexedAdmitHistoricalRecoveryPacketStep

THEOREM IndexedHistoricalDecisionRouteNeutralClaimHasExactIngressWitness ==
  \A initialContext \in AdmissibleContextRecords,
     node, qc, response:
    /\ IndexedHistoricalTransport(initialContext)!
         AsyncStrongTypeInvariant
    /\ IndexedHistoricalDecisionResponseClaimResidual(
         initialContext, node, qc, response)
    => \E admitted:
         /\ IndexedHistoricalTransport(initialContext)!
              AsyncCertifiedResponseAuthProjection(admitted)
                = IndexedHistoricalTransport(initialContext)!
                    AsyncCertifiedResponseAuthProjection(response)
         /\ IndexedHistoricalTransport(initialContext)!
              HistoricalDecisionClaimedResponseIngressOwned(
                node, qc, admitted)
BY IndexedHistoricalTransport(initialContext)!
     HistoricalDecisionRouteNeutralOwnerHasExactIngressOccurrence,
   Isa
   DEF IndexedHistoricalDecisionResponseClaimResidual

THEOREM IndexedHistoricalDecisionSelectedClaimDrainCreatesGoal ==
  \A initialContext \in AdmissibleContextRecords,
     node, qc, response, admitted:
    /\ IndexedHistoricalTransport(initialContext)!
         AsyncStrongTypeInvariant
    /\ IndexedHistoricalDecisionResponseClaimResidual(
         initialContext, node, qc, response)
    /\ IndexedHistoricalTransport(initialContext)!
         AsyncCertifiedResponseAuthProjection(admitted)
           = IndexedHistoricalTransport(initialContext)!
               AsyncCertifiedResponseAuthProjection(response)
    /\ IndexedHistoricalTransport(initialContext)!
         HistoricalDecisionClaimedResponseIngressOwned(
           node, qc, admitted)
    /\ IndexedHistoricalTransport(initialContext)!
         SelectedIngressItemAt(
           node,
           IndexedHistoricalTransport(initialContext)!
             FirstDrainableIngressIndex(node)) = admitted
    /\ IndexedHistoricalTransport(initialContext)!
         DrainFairIngressSelected(node)
    => IndexedHistoricalTransport(initialContext)!
         HistoricalDecisionCertifiedResponseGoal(node, qc)'
BY IndexedHistoricalTransport(initialContext)!
     HistoricalDecisionResponseIngressCreatesCertifiedFetch,
   Isa
   DEF IndexedHistoricalDecisionResponseClaimResidual

IndexedHistoricalDecisionResponseNonPhysicalHeadGateResidualProperty ==
  \A initialContext \in AdmissibleContextRecords,
     node, qc, archive, request, response, packet:
    IndexedChainSpec
      => ((/\ IndexedHistoricalDecisionResponsePacketResidual(
                initialContext, node, qc, archive,
                request, response, packet)
           /\ ~IndexedHistoricalDecisionResponsePhysicalCompletionResidual(
                initialContext, node, qc, archive,
                request, response, packet))
           ~> (IndexedHistoricalTransport(initialContext)!
                 HistoricalDecisionCertifiedResponseGoal(node, qc)
                \/ IndexedHistoricalDecisionResponseClaimResidual(
                     initialContext, node, qc, response)))

IndexedHistoricalDecisionResponsePhysicalCompletionResidualProperty ==
  \A initialContext \in AdmissibleContextRecords,
     node, qc, archive, request, response, packet:
    IndexedChainSpec
      => (IndexedHistoricalDecisionResponsePhysicalCompletionResidual(
            initialContext, node, qc, archive,
            request, response, packet)
           ~> (IndexedHistoricalTransport(initialContext)!
                HistoricalDecisionCertifiedResponseGoal(node, qc)
                \/ IndexedHistoricalDecisionResponseClaimResidual(
                     initialContext, node, qc, response)
                \/ /\ IndexedHistoricalDecisionResponsePacketResidual(
                         initialContext, node, qc, archive,
                         request, response, packet)
                   /\ ~IndexedHistoricalDecisionResponsePhysicalCompletionResidual(
                         initialContext, node, qc, archive,
                         request, response, packet)))

IndexedHistoricalDecisionResponseClaimRunnerResidualProperty ==
  \A initialContext \in AdmissibleContextRecords,
     node, qc, response:
    IndexedChainSpec
      => (IndexedHistoricalDecisionResponseClaimResidual(
            initialContext, node, qc, response)
           ~> IndexedHistoricalTransport(initialContext)!
                 HistoricalDecisionCertifiedResponseGoal(node, qc))

IndexedHistoricalDecisionResponseAdmissionResidualProperties ==
  /\ IndexedHistoricalDecisionResponseNonPhysicalHeadGateResidualProperty
  /\ IndexedHistoricalDecisionResponsePhysicalCompletionResidualProperty
  /\ IndexedHistoricalDecisionResponseClaimRunnerResidualProperty

THEOREM IndexedHistoricalDecisionResponseResidualsCloseKernel ==
  IndexedHistoricalDecisionResponseAdmissionResidualProperties
    => \A initialContext \in AdmissibleContextRecords:
         IndexedHistoricalTransport(initialContext)!
           HistoricalDecisionResponseAdmissionKernelProperty(
             IndexedChainSpec)
BY PTL
   DEF IndexedHistoricalDecisionResponseAdmissionResidualProperties,
       IndexedHistoricalDecisionResponseNonPhysicalHeadGateResidualProperty,
       IndexedHistoricalDecisionResponsePhysicalCompletionResidualProperty,
       IndexedHistoricalDecisionResponseClaimRunnerResidualProperty,
       IndexedHistoricalDecisionResponsePacketResidual,
       IndexedHistoricalDecisionResponseClaimResidual,
       IndexedHistoricalTransport!
         HistoricalDecisionResponseAdmissionKernelProperty

IndexedHistoricalPhysicalTransportResidualProperties ==
  /\ IndexedHistoricalCommitEmissionResidualProperties
  /\ IndexedHistoricalCommitRequestIngressResidualProperties
  /\ IndexedHistoricalCommitResponseAdmissionResidualProperties
  /\ IndexedHistoricalDecisionEmissionResidualProperties
  /\ IndexedHistoricalDecisionRequestIngressResidualProperties
  /\ IndexedHistoricalDecisionResponseAdmissionResidualProperties

THEOREM IndexedHistoricalPhysicalResidualsProvideSixKernels ==
  IndexedHistoricalPhysicalTransportResidualProperties
    => /\ \A initialContext \in AdmissibleContextRecords:
              /\ IndexedHistoricalTransport(initialContext)!
                   HistoricalCommitRequestPacketEmissionKernelProperty(
                     IndexedChainSpec)
              /\ IndexedHistoricalTransport(initialContext)!
                   HistoricalCommitRequestIngressKernelProperty(
                     IndexedChainSpec)
              /\ IndexedHistoricalTransport(initialContext)!
                   HistoricalCommitResponseAdmissionKernelProperty(
                     IndexedChainSpec)
       /\ \A initialContext \in AdmissibleContextRecords:
              /\ IndexedHistoricalTransport(initialContext)!
                   HistoricalDecisionRequestPacketEmissionKernelProperty(
                     IndexedChainSpec)
              /\ IndexedHistoricalTransport(initialContext)!
                   HistoricalDecisionRequestIngressKernelProperty(
                     IndexedChainSpec)
              /\ IndexedHistoricalTransport(initialContext)!
                   HistoricalDecisionResponseAdmissionKernelProperty(
                     IndexedChainSpec)
BY IndexedHistoricalCommitEmissionResidualsCloseKernel,
   IndexedHistoricalCommitIngressResidualsCloseKernel,
   IndexedHistoricalCommitResponseResidualsCloseKernel,
   IndexedHistoricalDecisionEmissionResidualsCloseKernel,
   IndexedHistoricalDecisionIngressResidualsCloseKernel,
   IndexedHistoricalDecisionResponseResidualsCloseKernel
   DEF IndexedHistoricalPhysicalTransportResidualProperties

(***************************************************************************
Commit transport reduction.

Request-fanout completeness and exact archive-route availability are now
product invariants, and the ordinary-I/O Serve response kernel is proved
above from its exact FIFO occurrence and action handoff.  The only temporal
input below is therefore the three still-open physical corridors: request
emission, request packet-to-ingress/Serve, and response admission.  This
reduction does not assert any of those residual kernels.
***************************************************************************)

IndexedHistoricalCommitTransportResidualKernelProperties ==
  \A initialContext \in AdmissibleContextRecords:
    /\ IndexedHistoricalTransport(initialContext)!
         HistoricalCommitRequestPacketEmissionKernelProperty(
           IndexedChainSpec)
    /\ IndexedHistoricalTransport(initialContext)!
         HistoricalCommitRequestIngressKernelProperty(IndexedChainSpec)
    /\ IndexedHistoricalTransport(initialContext)!
         HistoricalCommitResponseAdmissionKernelProperty(IndexedChainSpec)

IndexedHistoricalCommitCertificateTransportLeafProperty ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedHistoricalTransport(initialContext)!
      HistoricalCommitCertificateTransportLeaf(IndexedChainSpec)

THEOREM IndexedHistoricalCommitTransportKernelsCloseExactLeaf ==
  /\ IndexedChainSpec
  /\ IndexedHistoricalCommitTransportResidualKernelProperties
  => IndexedHistoricalCommitCertificateTransportLeafProperty
PROOF
  <1>1. ASSUME IndexedChainSpec,
                IndexedHistoricalCommitTransportResidualKernelProperties
         PROVE IndexedHistoricalCommitCertificateTransportLeafProperty
    <2>1. IndexedHistoricalCommitRequestCompletenessProperty
      BY <1>1,
         IndexedChainSpecProvidesHistoricalCommitRequestCompleteness
    <2>2. IndexedHistoricalCommitArchiveRouteAvailabilityProperty
      BY IndexedChainSpecDischargesHistoricalCommitArchiveRouteAvailability
    <2>3. ASSUME NEW initialContext \in AdmissibleContextRecords
           PROVE IndexedHistoricalTransport(initialContext)!
                   HistoricalCommitCertificateTransportLeaf(
                     IndexedChainSpec)
      <3>1. IndexedHistoricalTransport(initialContext)!
               HistoricalCommitRequestCompletenessProperty(IndexedChainSpec)
        BY <2>1 DEF IndexedHistoricalCommitRequestCompletenessProperty
      <3>2. IndexedHistoricalTransport(initialContext)!
               HistoricalCommitArchiveRouteAvailabilityProperty(
                 IndexedChainSpec)
        BY <2>2
           DEF IndexedHistoricalCommitArchiveRouteAvailabilityProperty
      <3>3. IndexedHistoricalTransport(initialContext)!
               HistoricalCommitRequestPacketEmissionKernelProperty(
                 IndexedChainSpec)
        BY <1>1
           DEF IndexedHistoricalCommitTransportResidualKernelProperties
      <3>4. IndexedHistoricalTransport(initialContext)!
               HistoricalCommitRequestIngressKernelProperty(
                 IndexedChainSpec)
        BY <1>1
           DEF IndexedHistoricalCommitTransportResidualKernelProperties
      <3>5. IndexedHistoricalTransport(initialContext)!
               HistoricalCommitServeResponseKernelProperty(
                 IndexedChainSpec)
        BY <1>1,
           IndexedChainSpecClosesHistoricalCommitServeResponseKernel
           DEF IndexedHistoricalTransport!
                 HistoricalCommitServeResponseKernelProperty
      <3>6. IndexedHistoricalTransport(initialContext)!
               HistoricalCommitResponseAdmissionKernelProperty(
                 IndexedChainSpec)
        BY <1>1
           DEF IndexedHistoricalCommitTransportResidualKernelProperties
      <3>7. IndexedHistoricalTransport(initialContext)!
               HistoricalCommitPhysicalTransportKernelProperties(
                 IndexedChainSpec)
        BY <3>3, <3>4, <3>5, <3>6
           DEF IndexedHistoricalTransport!
                 HistoricalCommitPhysicalTransportKernelProperties
      <3> QED BY <3>1, <3>2, <3>7,
           IndexedHistoricalTransport(initialContext)!
             HistoricalCommitTransportKernelsDischargeExactLeaf
    <2> QED BY <2>3
         DEF IndexedHistoricalCommitCertificateTransportLeafProperty
  <1> QED BY <1>1

IndexedHistoricalDecisionTransportResidualKernelProperties ==
  \A initialContext \in AdmissibleContextRecords:
    /\ IndexedHistoricalTransport(initialContext)!
         HistoricalDecisionRequestPacketEmissionKernelProperty(
           IndexedChainSpec)
    /\ IndexedHistoricalTransport(initialContext)!
         HistoricalDecisionRequestIngressKernelProperty(IndexedChainSpec)
    /\ IndexedHistoricalTransport(initialContext)!
         HistoricalDecisionResponseAdmissionKernelProperty(IndexedChainSpec)

IndexedHistoricalDecisionCertifiedBodyTransportLeafProperty ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedHistoricalTransport(initialContext)!
      HistoricalDecisionCertifiedBodyTransportLeaf(IndexedChainSpec)

THEOREM IndexedHistoricalDecisionTransportKernelsCloseExactLeaf ==
  /\ IndexedChainSpec
  /\ IndexedHistoricalDecisionTransportResidualKernelProperties
  => IndexedHistoricalDecisionCertifiedBodyTransportLeafProperty
PROOF
  <1>1. ASSUME IndexedChainSpec,
                IndexedHistoricalDecisionTransportResidualKernelProperties
         PROVE IndexedHistoricalDecisionCertifiedBodyTransportLeafProperty
    <2>1. IndexedHistoricalDecisionRequestCompletenessProperty
      BY <1>1,
         IndexedChainSpecProvidesHistoricalDecisionRequestCompleteness
    <2>2. [](\A initialContext \in AdmissibleContextRecords:
               IndexedHistoricalTemporalSupportAt(initialContext))
      BY <1>1, IndexedChainSpecAlwaysHistoricalTemporalSupport
    <2>3. ASSUME NEW initialContext \in AdmissibleContextRecords
           PROVE IndexedHistoricalTransport(initialContext)!
                   HistoricalDecisionCertifiedBodyTransportLeaf(
                     IndexedChainSpec)
      <3>1. IndexedChainSpec
                 => []IndexedHistoricalTransport(initialContext)!
                       AsyncStrongTypeInvariant
        BY <1>1, <2>2, <2>3, PTL
           DEF IndexedHistoricalTemporalSupportAt
      <3>2. IndexedHistoricalTransport(initialContext)!
               HistoricalDecisionCertifiedRequestCompletenessProperty(
                 IndexedChainSpec)
        BY <2>1
           DEF IndexedHistoricalDecisionRequestCompletenessProperty
      <3>3. IndexedHistoricalTransport(initialContext)!
               HistoricalDecisionRequestPacketEmissionKernelProperty(
                 IndexedChainSpec)
        BY <1>1
           DEF IndexedHistoricalDecisionTransportResidualKernelProperties
      <3>4. IndexedHistoricalTransport(initialContext)!
               HistoricalDecisionRequestIngressKernelProperty(
                 IndexedChainSpec)
        BY <1>1
           DEF IndexedHistoricalDecisionTransportResidualKernelProperties
      <3>5. IndexedHistoricalTransport(initialContext)!
               HistoricalDecisionServeResponseKernelProperty(
                 IndexedChainSpec)
        BY <1>1,
           IndexedChainSpecClosesHistoricalDecisionServeResponseKernel
           DEF IndexedHistoricalTransport!
                 HistoricalDecisionServeResponseKernelProperty
      <3>6. IndexedHistoricalTransport(initialContext)!
               HistoricalDecisionResponseAdmissionKernelProperty(
                 IndexedChainSpec)
        BY <1>1
           DEF IndexedHistoricalDecisionTransportResidualKernelProperties
      <3>7. IndexedHistoricalTransport(initialContext)!
               HistoricalDecisionCertifiedTransportKernelProperties(
                 IndexedChainSpec)
        BY <3>3, <3>4, <3>5, <3>6
           DEF IndexedHistoricalTransport!
                 HistoricalDecisionCertifiedTransportKernelProperties
      <3> QED BY <3>1, <3>2, <3>7,
           IndexedHistoricalTransport(initialContext)!
             HistoricalDecisionTransportKernelsDischargeExactLeaf
    <2> QED BY <2>3
         DEF IndexedHistoricalDecisionCertifiedBodyTransportLeafProperty
  <1> QED BY <1>1

=============================================================================
