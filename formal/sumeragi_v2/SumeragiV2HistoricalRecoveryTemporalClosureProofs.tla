---- MODULE SumeragiV2HistoricalRecoveryTemporalClosureProofs ----
EXTENDS SumeragiV2SuccessorActivationRefinementProofs,
        SumeragiV2AsyncHistoricalRecoveryClockTemporalProofs,
        SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs

(***************************************************************************
Proof-bearing exact Decision witness over one indexed Async instance.

`IndexedAsync` deliberately instantiates only the executable network module,
so none of the source-retention theorems from
`SumeragiV2ProgressWitnessFinalClosureProofs` are imported through it.  This
second instance uses the identical state projection and imports only safety
theorems: initialization, bracketed-step preservation, and exact Decision
stage decomposition.  It does not import an AsyncSpecAt fairness projection or
any application, height, or historical-recovery liveness theorem.
***************************************************************************)

IndexedDecisionWitness(initialContext) ==
  INSTANCE SumeragiV2ProgressWitnessFinalClosureProofs
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
       asyncServeProducerTurnReady <-
         IndexedServeProducerTurnDue(initialContext)

(***************************************************************************
Exact Decision-source safety over one indexed Async instance.

This instance has the identical state projection as `IndexedAsync` and
`IndexedDecisionWitness`, but imports only the source-retention and exact
Decision-service vocabulary needed below.  It deliberately stops below the
application-liveness and one-height temporal-closure modules.  Local indexed
runner fairness is supplied by the product providers instead of projecting a
whole `AsyncSpecAt` after an all-responsive join barrier.
***************************************************************************)

IndexedDecisionServiceWitness(initialContext) ==
  INSTANCE SumeragiV2ExactDecisionStageServiceClosureProofs
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
       asyncServeProducerTurnReady <-
         IndexedServeProducerTurnDue(initialContext)

(***************************************************************************
Generic local adequate-leader witness over the same indexed state.

Only the target-local GST-to-Decision interface is consumed below.  The
instance contributes no aggregate Decide, application, one-height, or height
liveness theorem.  Its service kernel is discharged against the indexed
product's exact current-voter action fairness.
***************************************************************************)

IndexedAdequateLeaderWitness(initialContext) ==
  INSTANCE SumeragiV2AdequateLeaderAuthorityDeadlineServiceProofs
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
       asyncServeProducerTurnReady <-
         IndexedServeProducerTurnDue(initialContext)

(***************************************************************************
Exact indexed producer ownership.

All three witnesses above share the product's per-context three-field
producer journal in addition to the 46-field scheduler projection.  Thus a
known/consumed producer episode or retained origin cannot alias another
height/context instance, while the immutable Serve admission and terminal
tombstone state remains in scheduler slots 11..16.  Retried historical or
exact-Decision work therefore stays in one monotone lifecycle instead of
recreating a drained owner through an unindexed witness variable.
***************************************************************************)

(***************************************************************************
Indexed exact-source retention support.

The support conjunction is precisely the safety context consumed by the
proved bracketed final-witness preservation theorem.  Each conjunct has an
independent proved initialization and bracketed-step preservation theorem in
the instantiated module.  Keeping them together here avoids projecting the
full AsyncSpecAt fairness formula, which would require every responsive node
to have joined this context.
***************************************************************************)

IndexedDecisionWitnessSupportAt(initialContext) ==
  /\ IndexedCore(initialContext, 2) = initialContext
  /\ IndexedDecisionWitness(initialContext)!AsyncStrongTypeInvariant
  /\ IndexedDecisionWitness(initialContext)!AsyncProgressOwnershipInvariant
  /\ IndexedDecisionWitness(initialContext)!
       DecisionFrontierUniquenessInvariant
  /\ IndexedDecisionWitness(initialContext)!DecisionTimeoutFrontierInvariant
  /\ IndexedDecisionWitness(initialContext)!
       ResponsiveRecoveryValidationClearedInvariant
  /\ IndexedDecisionWitness(initialContext)!
       FinalProgressWitnessClosureInvariant
  /\ IndexedDecisionWitness(initialContext)!
       CurrentAppliedArchiveBodyRetentionInvariant
  /\ IndexedHistoricalTransport(initialContext)!
       ReachableResponsiveDecisionServiceOwnershipInvariant

IndexedDecisionWitnessSupport ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedDecisionWitnessSupportAt(initialContext)

THEOREM IndexedDecisionWitnessVariablesAreExact ==
  IndexedAsyncStateShape
    => \A initialContext \in AdmissibleContextRecords:
         IndexedDecisionWitness(initialContext)!AsyncAllVars =
           IndexedAsyncStateAt(initialContext)
BY Isa
   DEF IndexedAsyncStateShape, IndexedAsyncStateAt,
       IndexedDecisionWitness!AsyncAllVars,
       IndexedDecisionWitness!AsyncSchedulerVars,
       IndexedDecisionWitness!AsyncRecoveryVars,
       IndexedDecisionWitness!AsyncProducerVars,
       IndexedDecisionWitness!vars,
       IndexedDuplicatedGst, IndexedCore, IndexedScheduler,
       IndexedRecovery, IndexedProducer,
       IndexedFixedCorridorDeadlines,
       IndexedServeProducerTurnDue

THEOREM IndexedAdequateLeaderWitnessVariablesAreExact ==
  IndexedAsyncStateShape
    => \A initialContext \in AdmissibleContextRecords:
         IndexedAdequateLeaderWitness(initialContext)!AsyncAllVars =
           IndexedAsyncStateAt(initialContext)
BY Isa
   DEF IndexedAsyncStateShape, IndexedAsyncStateAt,
       IndexedAdequateLeaderWitness!AsyncAllVars,
       IndexedAdequateLeaderWitness!AsyncSchedulerVars,
       IndexedAdequateLeaderWitness!AsyncRecoveryVars,
       IndexedAdequateLeaderWitness!AsyncProducerVars,
       IndexedAdequateLeaderWitness!vars,
       IndexedDuplicatedGst, IndexedCore, IndexedScheduler,
       IndexedRecovery, IndexedProducer,
       IndexedFixedCorridorDeadlines,
       IndexedServeProducerTurnDue

THEOREM IndexedAsyncLiveSpecProjectsAdequateLeaderWitnessLiveSpec ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedAsync(initialContext)!AsyncLiveSpecAt(initialContext)
      => IndexedAdequateLeaderWitness(initialContext)!
           AsyncLiveSpecAt(initialContext)
BY Isa
   DEF IndexedAsync!AsyncLiveSpecAt,
       IndexedAdequateLeaderWitness!AsyncLiveSpecAt,
       IndexedAsyncStateAt,
       IndexedDuplicatedGst, IndexedCore, IndexedScheduler,
       IndexedRecovery, IndexedProducer,
       IndexedFixedCorridorDeadlines,
       IndexedServeProducerTurnDue

THEOREM IndexedDecisionServiceWitnessVariablesAreExact ==
  IndexedAsyncStateShape
    => \A initialContext \in AdmissibleContextRecords:
         IndexedDecisionServiceWitness(initialContext)!AsyncAllVars =
           IndexedAsyncStateAt(initialContext)
BY Isa
   DEF IndexedAsyncStateShape, IndexedAsyncStateAt,
       IndexedDecisionServiceWitness!AsyncAllVars,
       IndexedDecisionServiceWitness!AsyncSchedulerVars,
       IndexedDecisionServiceWitness!AsyncRecoveryVars,
       IndexedDecisionServiceWitness!AsyncProducerVars,
       IndexedDecisionServiceWitness!vars,
       IndexedDuplicatedGst, IndexedCore, IndexedScheduler,
       IndexedRecovery, IndexedProducer,
       IndexedFixedCorridorDeadlines,
       IndexedServeProducerTurnDue

THEOREM IndexedInitProjectsEveryDecisionWitnessInit ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedChainInit
      => IndexedDecisionWitness(initialContext)!
           AsyncInitAt(initialContext)
BY IndexedInitProjectsEveryAsyncInit
   DEF IndexedDecisionWitness!AsyncInitAt,
       IndexedAsync!AsyncInitAt

THEOREM IndexedStepProjectsEveryDecisionWitnessStep ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedChainNext
      => [IndexedDecisionWitness(initialContext)!AsyncNext]_(
           IndexedDecisionWitness(initialContext)!AsyncAllVars)
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                IndexedChainNext
         PROVE [IndexedDecisionWitness(initialContext)!AsyncNext]_(
                 IndexedDecisionWitness(initialContext)!AsyncAllVars)
    <2>1. IndexedAsyncStateShape
      BY <1>1 DEF IndexedChainNext
    <2>2. IndexedDecisionWitness(initialContext)!AsyncAllVars =
             IndexedAsyncStateAt(initialContext)
      BY <1>1, <2>1, IndexedDecisionWitnessVariablesAreExact
    <2>3. [IndexedAsync(initialContext)!AsyncNext]_(
             IndexedAsyncStateAt(initialContext))
      BY <1>1, IndexedStepProjectsEveryAsyncStep
    <2> QED BY <2>2, <2>3, Isa
         DEF IndexedDecisionWitness!AsyncNext,
             IndexedAsync!AsyncNext
  <1> QED BY <1>1

THEOREM IndexedBracketStepProjectsEveryDecisionWitnessStep ==
  \A initialContext \in AdmissibleContextRecords:
    [IndexedChainNext]_IndexedChainVars
      => [IndexedDecisionWitness(initialContext)!AsyncNext]_(
           IndexedDecisionWitness(initialContext)!AsyncAllVars)
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                [IndexedChainNext]_IndexedChainVars
         PROVE [IndexedDecisionWitness(initialContext)!AsyncNext]_(
                 IndexedDecisionWitness(initialContext)!AsyncAllVars)
    <2>1. CASE IndexedChainNext
      BY <1>1, <2>1, IndexedStepProjectsEveryDecisionWitnessStep
    <2>2. CASE UNCHANGED IndexedChainVars
      <3>1. UNCHANGED indexedAsyncState
        BY <2>2 DEF IndexedChainVars
      <3>2. UNCHANGED
               (IndexedDecisionWitness(initialContext)!AsyncAllVars)
        BY <3>1, Isa
           DEF IndexedDecisionWitness!AsyncAllVars,
               IndexedDecisionWitness!AsyncSchedulerVars,
               IndexedDecisionWitness!AsyncRecoveryVars,
               IndexedDecisionWitness!AsyncProducerVars,
               IndexedDecisionWitness!vars,
               IndexedDuplicatedGst, IndexedCore, IndexedScheduler,
               IndexedRecovery, IndexedProducer,
               IndexedFixedCorridorDeadlines,
               IndexedServeProducerTurnDue
      <3> QED BY <3>2
    <2> QED BY <1>1, <2>1, <2>2
  <1> QED BY <1>1

THEOREM IndexedBracketStepProjectsEveryAdequateLeaderWitnessStep ==
  \A initialContext \in AdmissibleContextRecords:
    [IndexedChainNext]_IndexedChainVars
      => [IndexedAdequateLeaderWitness(initialContext)!AsyncNext]_(
           IndexedAdequateLeaderWitness(initialContext)!AsyncAllVars)
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                [IndexedChainNext]_IndexedChainVars
         PROVE [IndexedAdequateLeaderWitness(initialContext)!AsyncNext]_(
                 IndexedAdequateLeaderWitness(initialContext)!AsyncAllVars)
    <2>1. CASE IndexedChainNext
      <3>1. IndexedAsyncStateShape
        BY <2>1 DEF IndexedChainNext
      <3>2. IndexedAdequateLeaderWitness(initialContext)!AsyncAllVars =
               IndexedAsyncStateAt(initialContext)
        BY <1>1, <3>1, IndexedAdequateLeaderWitnessVariablesAreExact
      <3>3. [IndexedAsync(initialContext)!AsyncNext]_(
               IndexedAsyncStateAt(initialContext))
        BY <2>1, IndexedStepProjectsEveryAsyncStep
      <3> QED BY <3>2, <3>3, Isa
           DEF IndexedAdequateLeaderWitness!AsyncNext,
               IndexedAsync!AsyncNext
    <2>2. CASE UNCHANGED IndexedChainVars
      <3>1. UNCHANGED indexedAsyncState
        BY <2>2 DEF IndexedChainVars
      <3>2. UNCHANGED
               (IndexedAdequateLeaderWitness(initialContext)!AsyncAllVars)
        BY <3>1, Isa
           DEF IndexedAdequateLeaderWitness!AsyncAllVars,
               IndexedAdequateLeaderWitness!AsyncSchedulerVars,
               IndexedAdequateLeaderWitness!AsyncRecoveryVars,
               IndexedAdequateLeaderWitness!AsyncProducerVars,
               IndexedAdequateLeaderWitness!vars,
               IndexedDuplicatedGst, IndexedCore, IndexedScheduler,
               IndexedRecovery, IndexedProducer,
               IndexedFixedCorridorDeadlines,
               IndexedServeProducerTurnDue
      <3> QED BY <3>2
    <2> QED BY <1>1, <2>1, <2>2
  <1> QED BY <1>1

THEOREM IndexedBracketStepProjectsEveryDecisionServiceWitnessStep ==
  \A initialContext \in AdmissibleContextRecords:
    [IndexedChainNext]_IndexedChainVars
      => [IndexedDecisionServiceWitness(initialContext)!AsyncNext]_(
           IndexedDecisionServiceWitness(initialContext)!AsyncAllVars)
BY IndexedBracketStepProjectsEveryDecisionWitnessStep, Isa
   DEF IndexedDecisionServiceWitness!AsyncNext,
       IndexedDecisionWitness!AsyncNext,
       IndexedDecisionServiceWitness!AsyncAllVars,
       IndexedDecisionServiceWitness!AsyncSchedulerVars,
       IndexedDecisionServiceWitness!AsyncRecoveryVars,
       IndexedDecisionServiceWitness!AsyncProducerVars,
       IndexedDecisionServiceWitness!vars,
       IndexedDecisionWitness!AsyncAllVars,
       IndexedDecisionWitness!AsyncSchedulerVars,
       IndexedDecisionWitness!AsyncRecoveryVars,
       IndexedDecisionWitness!AsyncProducerVars,
       IndexedDecisionWitness!vars,
       IndexedDuplicatedGst, IndexedCore, IndexedScheduler,
       IndexedRecovery, IndexedProducer,
       IndexedFixedCorridorDeadlines,
       IndexedServeProducerTurnDue

THEOREM IndexedChainInitEstablishesDecisionWitnessSupport ==
  IndexedChainInit => IndexedDecisionWitnessSupport
PROOF
  <1>1. ASSUME IndexedChainInit,
                NEW initialContext \in AdmissibleContextRecords
         PROVE IndexedDecisionWitnessSupportAt(initialContext)
    <2>1. IndexedDecisionWitness(initialContext)!
             AsyncInitAt(initialContext)
      BY <1>1, IndexedInitProjectsEveryDecisionWitnessInit
    <2>2. IndexedCore(initialContext, 2) = initialContext
      BY <2>1
         DEF IndexedDecisionWitness!AsyncInitAt,
             IndexedDecisionWitness!AsyncBaseInitAt,
             IndexedDecisionWitness!InitAt
    <2>3. IndexedDecisionWitness(initialContext)!
             AsyncStrongTypeInvariant
      BY <2>1,
         IndexedDecisionWitness(initialContext)!
           AsyncInitEstablishesStrongTypeInvariant
    <2>4. IndexedDecisionWitness(initialContext)!
             AsyncProgressOwnershipInvariant
      BY <2>1,
         IndexedDecisionWitness(initialContext)!
           AsyncInitEstablishesProgressOwnership
    <2>5. IndexedDecisionWitness(initialContext)!
             DecisionFrontierUniquenessInvariant
      BY <2>1,
         IndexedDecisionWitness(initialContext)!
           AsyncInitEstablishesDecisionFrontierUniqueness
    <2>6. IndexedDecisionWitness(initialContext)!
             DecisionTimeoutFrontierInvariant
      BY <2>1,
         IndexedDecisionWitness(initialContext)!
           AsyncInitEstablishesDecisionTimeoutFrontier
    <2>7. IndexedDecisionWitness(initialContext)!
             ResponsiveRecoveryValidationClearedInvariant
      BY <2>1,
         IndexedDecisionWitness(initialContext)!
           AsyncInitEstablishesRecoveryValidationClearing
    <2>8. IndexedDecisionWitness(initialContext)!
             FinalProgressWitnessClosureInvariant
      BY <2>1,
         IndexedDecisionWitness(initialContext)!
           AsyncInitEstablishesFinalProgressWitnessClosure
    <2>9. IndexedDecisionWitness(initialContext)!
             CurrentAppliedArchiveBodyRetentionInvariant
      BY <2>1,
         IndexedDecisionWitness(initialContext)!
           AsyncInitEstablishesCurrentAppliedArchiveBodyRetention
    <2>10. IndexedHistoricalTransport(initialContext)!
              ReachableResponsiveDecisionServiceOwnershipInvariant
      BY <1>1, IndexedInitProjectsEveryHistoricalTransportInit,
         IndexedHistoricalTransport(initialContext)!
           AsyncInitEstablishesReachableResponsiveDecisionServiceOwnership
    <2> QED BY <2>2, <2>3, <2>4, <2>5, <2>6, <2>7, <2>8, <2>9,
                 <2>10
         DEF IndexedDecisionWitnessSupportAt
  <1> QED BY <1>1 DEF IndexedDecisionWitnessSupport

THEOREM IndexedBracketStepPreservesDecisionWitnessSupport ==
  /\ IndexedDecisionWitnessSupport
  /\ [IndexedChainNext]_IndexedChainVars
  => IndexedDecisionWitnessSupport'
PROOF
  <1>1. ASSUME IndexedDecisionWitnessSupport,
                [IndexedChainNext]_IndexedChainVars,
                NEW initialContext \in AdmissibleContextRecords
         PROVE IndexedDecisionWitnessSupportAt(initialContext)'
    <2>1. IndexedDecisionWitnessSupportAt(initialContext)
      BY <1>1 DEF IndexedDecisionWitnessSupport
    <2>2. [IndexedDecisionWitness(initialContext)!AsyncNext]_(
             IndexedDecisionWitness(initialContext)!AsyncAllVars)
      BY <1>1, IndexedBracketStepProjectsEveryDecisionWitnessStep
    <2>3. IndexedCore(initialContext, 2)' = initialContext
      BY <2>1, <2>2,
         IndexedDecisionWitness(initialContext)!AsyncBracketStepLeavesContext
         DEF IndexedDecisionWitnessSupportAt
    <2>4. (IndexedDecisionWitness(initialContext)!
             AsyncStrongTypeInvariant)'
      BY <2>1, <2>2,
         IndexedDecisionWitness(initialContext)!
           AsyncBracketNextPreservesStrongTypeInvariant
         DEF IndexedDecisionWitnessSupportAt
    <2>5. (IndexedDecisionWitness(initialContext)!
             AsyncProgressOwnershipInvariant)'
      BY <2>1, <2>2,
         IndexedDecisionWitness(initialContext)!
           AsyncBracketNextPreservesProgressOwnership
         DEF IndexedDecisionWitnessSupportAt
    <2>6. (IndexedDecisionWitness(initialContext)!
             DecisionFrontierUniquenessInvariant)'
      BY <2>1, <2>2,
         IndexedDecisionWitness(initialContext)!
           AsyncBracketPreservesStrongDecisionFrontier
         DEF IndexedDecisionWitnessSupportAt,
             IndexedDecisionWitness!AsyncStrongTypeInvariant
    <2>7. (IndexedDecisionWitness(initialContext)!
             DecisionTimeoutFrontierInvariant)'
      BY <2>1, <2>2,
         IndexedDecisionWitness(initialContext)!
           AsyncBracketPreservesDecisionTimeoutFrontier
         DEF IndexedDecisionWitnessSupportAt
    <2>8. (IndexedDecisionWitness(initialContext)!
             ResponsiveRecoveryValidationClearedInvariant)'
      BY <2>1, <2>2,
         IndexedDecisionWitness(initialContext)!
           AsyncBracketNextPreservesRecoveryValidationClearing
         DEF IndexedDecisionWitnessSupportAt
    <2>9. (IndexedDecisionWitness(initialContext)!
             FinalProgressWitnessClosureInvariant)'
      BY <2>1, <2>2,
         IndexedDecisionWitness(initialContext)!
           AsyncBracketNextPreservesFinalProgressWitnessClosure
         DEF IndexedDecisionWitnessSupportAt
    <2>10. (IndexedDecisionWitness(initialContext)!
              CurrentAppliedArchiveBodyRetentionInvariant)'
      BY <2>1, <2>2,
         IndexedDecisionWitness(initialContext)!
           AsyncBracketPreservesCurrentAppliedArchiveBodyRetention
         DEF IndexedDecisionWitnessSupportAt
    <2>11. (IndexedHistoricalTransport(initialContext)!
              ReachableResponsiveDecisionServiceOwnershipInvariant)'
      BY <2>1,
         IndexedBracketStepProjectsEveryHistoricalTransportStep,
         IndexedHistoricalTransport(initialContext)!
           AsyncBracketPreservesReachableResponsiveDecisionServiceOwnership,
         Isa
         DEF IndexedDecisionWitnessSupportAt,
             IndexedDecisionWitness!AsyncStrongTypeInvariant,
             IndexedHistoricalTransport!AsyncStrongTypeInvariant
    <2> QED BY <2>3, <2>4, <2>5, <2>6, <2>7, <2>8, <2>9,
                 <2>10, <2>11
         DEF IndexedDecisionWitnessSupportAt
  <1> QED BY <1>1 DEF IndexedDecisionWitnessSupport

THEOREM IndexedChainSpecAlwaysDecisionWitnessSupport ==
  IndexedChainSpec => []IndexedDecisionWitnessSupport
PROOF
  <1>1. IndexedChainInit => IndexedDecisionWitnessSupport
    BY IndexedChainInitEstablishesDecisionWitnessSupport
  <1>2. /\ IndexedDecisionWitnessSupport
         /\ [IndexedChainNext]_IndexedChainVars
         => IndexedDecisionWitnessSupport'
    BY IndexedBracketStepPreservesDecisionWitnessSupport
  <1> QED BY <1>1, <1>2, PTL DEF IndexedChainSpec

(***************************************************************************
Exact indexed historical-recovery temporal decomposition.

`IndexedExactHistoricalRecoveryProgress` starts at
`HistoricalRecoveryOutstanding`, which deliberately says only that a
responsive joined node is still located at the frozen context and has not
applied it.  That source is broader than the historical-recovery protocol:
at genesis, and for an ordinary current voter at a newly activated context,
it can hold before an applied archive source or an exact historical target
exists.  The first residual below owns precisely that prefix.  It must be
discharged by source availability or one joined current voter's local
Decision-to-Apply corridor; treating it as a historical packet/service
theorem would be circular.

After exact source authority exists, the remaining predicates name only
production state:

  * `IndexedHistoricalRecoveryOpenable` is the exact chain-owned source and
    target guard for `OpenHistoricalRecovery`;
  * certificate ranks 4..1 are respectively exact target ownership,
    CommitCertificateRequest transit/Serve ownership, the exact published
    CommitCertificateResponse, and recipient-specific CommitQC
    import/delivery/Decision-WAL ownership;
  * Decision ranks 6..1 are respectively FetchBody, one exact active
    CertifiedRequest owner, FetchCertifiedBody, StoreBody, ValidateBody,
    and Apply;
  * responsive archive-route selection and body service after rank 5 remain
    in the separate certified-request rank-progress residual;
  * exact application is handed to the existing chain receipt classifier.

The rank predicates do not place a historical target in
`AsyncCurrentResponsiveVoters`.  Their executor owner is explicitly either
the ordinary current-voter runner or the exact historical target.  Thus an
observer or successor-roster entrant relies on
`PostGstRunHistoricalRecoveryNode`,
`PostGstServiceHistoricalRecoveryIoWorker`, and the historical packet
corridor, not on voter fairness.

The exact Open property, exact Decision-stage ownership exposure, and exact
application receipt handoff are proved here from `IndexedChainSpec`.  The
certificate rank, exact historical-target Decision rank, ordinary-owner
Decision rank, and authority-acquisition operators are closed below from
their exact physical and strict-height providers.  The concluding PTL
reduction still exposes the historical-only boundary after source authority;
the broader chain-level premise is discharged without importing aggregate
application, one-height, or height-liveness conclusions.
***************************************************************************)

IndexedHistoricalExactApplication(initialContext, node) ==
  /\ initialContext \in AdmissibleContextRecords
  /\ node \in Responsive
  /\ IndexedAsync(initialContext)!NodeHasApplication(node)

IndexedHistoricalRecoveryRunnerOwned(initialContext, node) ==
  \/ node \in IndexedAsync(initialContext)!AsyncCurrentResponsiveVoters
  \/ IndexedAsync(initialContext)!HistoricalRecoveryTarget(node)

IndexedHistoricalRecoveryOpenable(initialContext, node) ==
  /\ IndexedCore(initialContext, 7)
  /\ IndexedHistoricalRecoveryTargetReady(initialContext, node)
  /\ \E server \in ValidatorIds,
       source \in Chain!DecisionEvidenceSet:
       IndexedHistoricalRecoverySourceReady(
         initialContext, server, source)

IndexedHistoricalRecoveryTargetOwned(initialContext, node) ==
  /\ HistoricalRecoveryOutstanding(initialContext, node)
  /\ IndexedAsync(initialContext)!HistoricalRecoveryTarget(node)

IndexedHistoricalDecisionOwned(initialContext, node) ==
  /\ HistoricalRecoveryOutstanding(initialContext, node)
  /\ IndexedHistoricalRecoveryRunnerOwned(initialContext, node)
  /\ IndexedAsync(initialContext)!NodeHasDecision(node)

(***************************************************************************
Exact Commit-certificate request/response/import ownership.

The request identity intentionally does not compare its view with the
requester's current `nodeView`: a current-roster recovery target may advance
its pacemaker after publishing the immutable historical request.  Height,
requester, recipient class, and the append-only request object remain exact.
***************************************************************************)

IndexedHistoricalCommitRequestIdentity(
    initialContext, node, request) ==
  /\ request.kind = "CommitCertificateRequest"
  /\ request.source = node
  /\ request.envelope.height = initialContext.height
  /\ request.envelope.recipient
       \in (IndexedAsync(initialContext)!CurrentVoters \ {node})
            \cap IndexedAsync(initialContext)!
                   AsyncResponsiveAppliedArchiveServers

IndexedHistoricalRequestInIngress(
    initialContext, request) ==
  \E source \in IndexedAsync(initialContext)!AsyncIngressSources:
    request \in
      SequenceSet(IndexedScheduler(initialContext, 40)
                    [request.envelope.recipient][source])

IndexedHistoricalRequestInServeQueue(
    initialContext, request) ==
  \E job \in SequenceSet(
       IndexedScheduler(initialContext, 10)
         [request.envelope.recipient]):
    /\ job.class = "Serve"
    /\ job.candidate.item = request

IndexedHistoricalRequestPhysicalOwner(
    initialContext, request) ==
  \/ request \in IndexedScheduler(initialContext, 37)
  \/ \E packet \in IndexedScheduler(initialContext, 39):
       packet.item = request
  \/ IndexedHistoricalRequestInIngress(initialContext, request)
  \/ IndexedHistoricalRequestInServeQueue(initialContext, request)

IndexedHistoricalCommitRequestOwned(initialContext, node) ==
  /\ IndexedHistoricalRecoveryTargetOwned(initialContext, node)
  /\ \E request:
       /\ IndexedHistoricalCommitRequestIdentity(
            initialContext, node, request)
       /\ IndexedHistoricalRequestPhysicalOwner(
            initialContext, request)

IndexedHistoricalCommitResponseIdentity(
    initialContext, node, request, qc, response) ==
  /\ IndexedHistoricalCommitRequestIdentity(
       initialContext, node, request)
  /\ qc.context = initialContext
  /\ qc.phase = "Commit"
  /\ response =
       IndexedAsync(initialContext)!
         CommitCertificateResponseItem(request, qc)
  /\ response.source = request.envelope.recipient
  /\ response.envelope.recipient = node
  /\ response.envelope.request = request

IndexedHistoricalCommitResponsePublished(initialContext, node) ==
  /\ IndexedHistoricalRecoveryTargetOwned(initialContext, node)
  /\ \E request, qc, response:
       /\ response \in IndexedScheduler(initialContext, 35)
       /\ IndexedHistoricalCommitResponseIdentity(
            initialContext, node, request, qc, response)

(***************************************************************************
Recipient-specific import ownership.  Global `commitQCs` or append-only
`qcNetwork` membership is not enough: the serving archive or old Core wire
history can own that QC before the target has any executor.  The predicates
below therefore require the exact target's received QcAt, non-rebroadcast
Decision WAL, or a current protected target command carrying the exact
CommitQC lineage.
***************************************************************************)

IndexedHistoricalCertificateCommandLineage(
    initialContext, node, qc, candidate) ==
  \/ /\ candidate.evidence \in IndexedScheduler(initialContext, 35)
     /\ candidate.evidence.kind = "CommitQC"
     /\ candidate.evidence.envelope =
          IndexedAsync(initialContext)!QcEnvelope(node, qc)
     /\ candidate.causalOrigin =
          IndexedAsync(initialContext)!
            AsyncDeliveryCandidateCausalOriginAt(
              candidate.evidence, initialContext)
     /\ candidate.item =
          IF candidate.kind = "DeliverQC"
          THEN candidate.evidence
          ELSE IndexedAsync(initialContext)!NoAsyncItem
  \/ \E response:
       /\ response \in IndexedScheduler(initialContext, 35)
       /\ response.kind = "CommitCertificateResponse"
       /\ response.source =
            response.envelope.request.envelope.recipient
       /\ response.envelope.recipient = node
       /\ response.envelope.qc = qc
       /\ IndexedAsync(initialContext)!
            CommitCertificateRequestAuthorized(
              response.envelope.request)
       /\ candidate.evidence = response
       /\ candidate.causalOrigin =
            IndexedAsync(initialContext)!
              AsyncCommitCertificateResponseCandidateCausalOriginAt(
                response, initialContext)
       /\ candidate.item =
            IF candidate.kind = "DeliverQC"
            THEN IndexedAsync(initialContext)!
                   DiscoveredCommitQcItem(response)
            ELSE IndexedAsync(initialContext)!NoAsyncItem

IndexedHistoricalCertificateLineageCandidateFor(
    initialContext, node, qc, candidate) ==
  /\ candidate \in
       IndexedAsync(initialContext)!AsyncCandidateSet
  /\ qc \in IndexedCore(initialContext, 23)
  /\ qc.context = initialContext
  /\ qc.phase = "Commit"
  /\ candidate.node = node
  /\ candidate.height = initialContext.height
  /\ candidate.view = qc.view
  /\ candidate.subject = qc.subject
  /\ candidate.kind
       \in {"DeliverQC", "BeginDecision", "PersistDecision"}
  /\ candidate.consumerContext = initialContext
  /\ IndexedAsync(initialContext)!CandidateConsumerCurrent(candidate)
  /\ CASE candidate.kind \in {"DeliverQC", "BeginDecision"} ->
            candidate.class = "Progress"
       [] candidate.kind = "PersistDecision" ->
            candidate.class = "Completion"
       [] OTHER -> FALSE
  /\ IndexedDecisionWitness(initialContext)!
       ProtectedCandidateOwned(candidate)
  /\ IndexedHistoricalCertificateCommandLineage(
       initialContext, node, qc, candidate)

IndexedHistoricalCertificateCommandFor(
    initialContext, node, qc, candidate) ==
  /\ IndexedHistoricalRecoveryTargetOwned(initialContext, node)
  /\ IndexedHistoricalCertificateLineageCandidateFor(
       initialContext, node, qc, candidate)

THEOREM IndexedHistoricalCertificateCommandHasPhysicalOwner ==
  \A node, qc, candidate:
    \A initialContext \in AdmissibleContextRecords:
      IndexedHistoricalCertificateCommandFor(
        initialContext, node, qc, candidate)
        => /\ IndexedHistoricalRecoveryTargetOwned(initialContext, node)
           /\ IndexedAsync(initialContext)!CandidateConsumerCurrent(candidate)
           /\ IndexedDecisionWitness(initialContext)!
                ProtectedCandidateOwned(candidate)
           /\ IndexedHistoricalCertificateCommandLineage(
                initialContext, node, qc, candidate)
BY DEF IndexedHistoricalCertificateCommandFor,
       IndexedHistoricalCertificateLineageCandidateFor

THEOREM IndexedHistoricalCertificateCommandRefinesHistoricalOwner ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    \A qc, candidate:
      /\ IndexedDecisionWitnessSupportAt(initialContext)
      /\ IndexedHistoricalCertificateCommandFor(
           initialContext, node, qc, candidate)
        => IndexedHistoricalTransport(initialContext)!
             HistoricalCommitDecisionCandidateOwned(
               node, candidate.kind)
BY IsaT(300)
   DEF IndexedHistoricalCertificateCommandFor,
       IndexedHistoricalCertificateLineageCandidateFor,
       IndexedHistoricalCertificateCommandLineage,
       IndexedDecisionWitnessSupportAt,
       IndexedHistoricalCommitResponseIdentity,
       IndexedHistoricalCommitRequestIdentity,
       IndexedAsync!AsyncNetworkItem,
       IndexedAsync!CurrentVoters,
       IndexedAsync!CurrentEpoch,
       IndexedAsync!HistoricalRecoveryTarget,
       IndexedAsync!QcEnvelope,
       IndexedAsync!CommitCertificateResponseItem,
       IndexedAsync!AsyncCommitCertificateResponseEnvelope,
       IndexedAsync!DiscoveredCommitQcItem,
       IndexedAsync!AsyncDeliveryCandidateCausalOriginAt,
       IndexedAsync!
         AsyncCommitCertificateResponseCandidateCausalOriginAt,
       IndexedAsync!CommitCertificateRequestAuthorized,
       IndexedDecisionWitness!ProtectedCandidateOwned,
       IndexedDecisionWitness!ProtectedServiceCandidate,
       IndexedDecisionWitness!CandidateScheduled,
       IndexedHistoricalTransport!
         HistoricalCommitDecisionCandidateOwned,
       IndexedHistoricalTransport!
         HistoricalCommitDecisionDirectEvidence,
       IndexedHistoricalTransport!
         HistoricalCommitDecisionResponseEvidence,
       IndexedHistoricalTransport!HistoricalProtectedCandidateOwned,
       IndexedHistoricalTransport!CurrentVoters,
       IndexedHistoricalTransport!CurrentEpoch,
       IndexedHistoricalTransport!HistoricalRecoveryTarget,
       IndexedHistoricalTransport!ProtectedCandidateOwned,
       IndexedHistoricalTransport!ProtectedServiceCandidate,
       IndexedHistoricalTransport!CandidateScheduled,
       IndexedHistoricalTransport!AsyncNetworkItem,
       IndexedHistoricalTransport!QcEnvelope,
       IndexedHistoricalTransport!DiscoveredCommitQcItem,
       IndexedHistoricalTransport!
         AsyncDeliveryCandidateCausalOriginAt,
       IndexedHistoricalTransport!
         AsyncCommitCertificateResponseCandidateCausalOriginAt,
       IndexedHistoricalTransport!CommitCertificateRequestAuthorized,
       IndexedHistoricalRecoveryTargetOwned

(***************************************************************************
Exact target-local import sources.

The received CommitQC pool and the non-rebroadcast Decision WAL are reducer
state, not scheduler owners.  Keep their predicates separate: exposing an
exact causal Candidate from either one needs a source-provenance invariant for
the selected DeliverQC/BeginDecision command.  Structural candidate typing
does not provide that provenance.  The invariant below is instead established
from the production execution guard and retained through causal successors.
Append-only Core qcNetwork history is deliberately not a local source.
***************************************************************************)

IndexedHistoricalCertificateReceivedQcLineageSource(
    initialContext, node, qc) ==
  /\ qc.context = initialContext
  /\ qc.phase = "Commit"
  /\ IndexedAsync(initialContext)!QcAt(node, qc)
       \in IndexedCore(initialContext, 15)

IndexedHistoricalCertificateDecisionWalLineageSource(
    initialContext, node, qc) ==
  /\ qc.context = initialContext
  /\ qc.phase = "Commit"
  /\ IndexedAsync(initialContext)!DecisionWal(node, qc, FALSE)
       \in IndexedCore(initialContext, 39)

IndexedHistoricalCertificateReceivedQcLineageInvariantAt(initialContext) ==
  \A qc:
    \A node \in Responsive:
      IndexedHistoricalCertificateReceivedQcLineageSource(
        initialContext, node, qc)
        => \/ IndexedDecisionWitness(initialContext)!NodeHasDecision(node)
           \/ IndexedDecisionWitness(initialContext)!NodeHasApplication(node)
           \/ \E candidate:
                IndexedHistoricalCertificateLineageCandidateFor(
                  initialContext, node, qc, candidate)

IndexedHistoricalCertificateDecisionWalLineageInvariantAt(initialContext) ==
  \A qc:
    \A node \in Responsive:
      IndexedHistoricalCertificateDecisionWalLineageSource(
        initialContext, node, qc)
        => \/ IndexedDecisionWitness(initialContext)!NodeHasDecision(node)
           \/ IndexedDecisionWitness(initialContext)!NodeHasApplication(node)
           \/ \E candidate:
                IndexedHistoricalCertificateLineageCandidateFor(
                  initialContext, node, qc, candidate)

(***************************************************************************
The execution guard is fail-closed only for unreachable malformed states; it
must not disable a reachable owner.  Cover every current import candidate in
the scheduler carrier, not only the selected FIFO head.  `CandidateScheduled`
ranges over the physical command FIFO, all Completion/Progress/Normal deferred
queues, the causal-successor queue, and outstanding I/O work.  Thus this
invariant supplies exact provenance before either FIFO or deferred execution
selects the candidate, while also covering every transfer between carriers.
***************************************************************************)
IndexedHistoricalCertificateScheduledImportProvenanceInvariantAt(
    initialContext) ==
  \A candidate:
    /\ candidate \in
         IndexedDecisionWitness(initialContext)!AsyncCandidateSet
    /\ IndexedDecisionWitness(initialContext)!
         CandidateConsumerCurrent(candidate)
    /\ IndexedDecisionWitness(initialContext)!CandidateScheduled(candidate)
    /\ IndexedDecisionWitness(initialContext)!
         AsyncCommitImportExecutionNeedsLineage(candidate)
    => IndexedDecisionWitness(initialContext)!
         AsyncCommitImportExecutionProvenance(candidate)

IndexedHistoricalCertificateLocalLineageInvariantAt(initialContext) ==
  /\ IndexedHistoricalCertificateReceivedQcLineageInvariantAt(
       initialContext)
  /\ IndexedHistoricalCertificateDecisionWalLineageInvariantAt(
       initialContext)
  /\ IndexedHistoricalCertificateScheduledImportProvenanceInvariantAt(
       initialContext)

IndexedHistoricalCertificateLocalLineageInvariant ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedHistoricalCertificateLocalLineageInvariantAt(initialContext)

THEOREM IndexedHistoricalCertificateScheduledImportOwnersHaveExactProvenance ==
  \A candidate:
    \A initialContext \in AdmissibleContextRecords:
      /\ IndexedHistoricalCertificateLocalLineageInvariantAt(initialContext)
      /\ candidate \in
           IndexedDecisionWitness(initialContext)!AsyncCandidateSet
      /\ IndexedDecisionWitness(initialContext)!
           CandidateConsumerCurrent(candidate)
      /\ IndexedDecisionWitness(initialContext)!CandidateScheduled(candidate)
      /\ IndexedDecisionWitness(initialContext)!
           AsyncCommitImportExecutionNeedsLineage(candidate)
      => IndexedDecisionWitness(initialContext)!
           AsyncCommitImportExecutionProvenance(candidate)
BY DEF IndexedHistoricalCertificateLocalLineageInvariantAt,
       IndexedHistoricalCertificateScheduledImportProvenanceInvariantAt

THEOREM IndexedAsyncCommitImportLineageRefinesHistoricalCertificateLineage ==
  \A node, qc, candidate:
    \A initialContext \in AdmissibleContextRecords:
      /\ candidate.node = node
      /\ candidate.consumerContext = initialContext
      /\ IndexedAsync(initialContext)!
           AsyncCommitImportCandidateLineage(candidate, qc)
      /\ IndexedAsync(initialContext)!CandidateConsumerCurrent(candidate)
      /\ IndexedDecisionWitness(initialContext)!
           ProtectedCandidateOwned(candidate)
      => IndexedHistoricalCertificateLineageCandidateFor(
           initialContext, node, qc, candidate)
BY Isa
   DEF IndexedHistoricalCertificateLineageCandidateFor,
       IndexedHistoricalCertificateCommandLineage,
       IndexedAsync!AsyncCommitImportCandidateLineage,
       IndexedAsync!AsyncCommitImportDirectEvidence,
       IndexedAsync!AsyncCommitImportResponseEvidence,
       IndexedAsync!QcEnvelope,
       IndexedAsync!DiscoveredCommitQcItem,
       IndexedAsync!CommitCertificateRequestAuthorized,
       IndexedCore, IndexedScheduler

THEOREM IndexedDecisionWitnessInitEstablishesHistoricalCertificateLocalLineage ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedDecisionWitness(initialContext)!AsyncInitAt(initialContext)
      => IndexedHistoricalCertificateLocalLineageInvariantAt(
           initialContext)
BY IsaT(300)
   DEF IndexedHistoricalCertificateLocalLineageInvariantAt,
       IndexedHistoricalCertificateReceivedQcLineageInvariantAt,
       IndexedHistoricalCertificateDecisionWalLineageInvariantAt,
       IndexedHistoricalCertificateScheduledImportProvenanceInvariantAt,
       IndexedHistoricalCertificateReceivedQcLineageSource,
       IndexedHistoricalCertificateDecisionWalLineageSource,
       IndexedDecisionWitness!CandidateScheduled,
       IndexedDecisionWitness!CandidateScheduledIn,
       IndexedDecisionWitness!AsyncCommitImportExecutionNeedsLineage,
       IndexedDecisionWitness!AsyncInitAt,
       IndexedDecisionWitness!AsyncBaseInitAt,
       IndexedDecisionWitness!AsyncRuntimeInit,
       IndexedDecisionWitness!AsyncIoInit,
       IndexedDecisionWitness!AsyncDeferredInit,
       IndexedDecisionWitness!NoItemCandidate,
       IndexedDecisionWitness!AsyncCandidateWithIdentityAndOrigin,
       IndexedDecisionWitness!SequenceSet,
       IndexedDecisionWitness!InitAt,
       IndexedCore, IndexedScheduler

THEOREM IndexedInitEstablishesHistoricalCertificateLocalLineage ==
  IndexedChainInit
    => IndexedHistoricalCertificateLocalLineageInvariant
PROOF
  <1>1. ASSUME IndexedChainInit,
                NEW initialContext \in AdmissibleContextRecords
         PROVE IndexedHistoricalCertificateLocalLineageInvariantAt(
                 initialContext)
    <2>1. IndexedDecisionWitness(initialContext)!
             AsyncInitAt(initialContext)
      BY <1>1, IndexedInitProjectsEveryDecisionWitnessInit
    <2> QED BY <2>1,
         IndexedDecisionWitnessInitEstablishesHistoricalCertificateLocalLineage
  <1> QED BY <1>1
       DEF IndexedHistoricalCertificateLocalLineageInvariant

THEOREM IndexedDecisionWitnessBracketPreservesHistoricalCertificateLocalLineage ==
  \A initialContext \in AdmissibleContextRecords:
    /\ IndexedDecisionWitnessSupportAt(initialContext)
    /\ IndexedHistoricalTemporalSupportAt(initialContext)
    /\ IndexedHistoricalCertificateLocalLineageInvariantAt(initialContext)
    /\ [IndexedDecisionWitness(initialContext)!AsyncNext]_(
         IndexedDecisionWitness(initialContext)!AsyncAllVars)
    => IndexedHistoricalCertificateLocalLineageInvariantAt(
         initialContext)'
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords
         PROVE /\ IndexedDecisionWitnessSupportAt(initialContext)
               /\ IndexedHistoricalTemporalSupportAt(initialContext)
               /\ IndexedHistoricalCertificateLocalLineageInvariantAt(
                    initialContext)
               /\ [IndexedDecisionWitness(initialContext)!AsyncNext]_(
                    IndexedDecisionWitness(initialContext)!AsyncAllVars)
               => IndexedHistoricalCertificateLocalLineageInvariantAt(
                    initialContext)'
    BY IndexedDecisionWitness(initialContext)!
     DirectCommitQcCandidateHasExactImportLineage,
   IndexedDecisionWitness(initialContext)!
     CommitCertificateResponseCandidateHasExactImportLineage,
   IndexedDecisionWitness(initialContext)!
     CommitImportCausalSuccessorRetainsExactLineage,
   IndexedDecisionWitness(initialContext)!
     AsyncCandidateScheduledIdentityDepartureRetiresLifecycleAtGst,
   IndexedDecisionWitness(initialContext)!
     AsyncCandidateCausalAdmissionTransfersSameOwner,
   IndexedDecisionWitness(initialContext)!
     AsyncCandidateIoCompletionTransfersSameOwner,
   IndexedDecisionWitness(initialContext)!
     AsyncCandidateProducerCompletionTransfersSameOwner,
   IndexedDecisionWitness(initialContext)!
     AsyncCandidateBusyDeferralTransfersSameOwner,
   IndexedDecisionWitness(initialContext)!
     AsyncCandidateDeferredHandoffRetainsSameOwner,
   IndexedAsyncCommitImportLineageRefinesHistoricalCertificateLineage,
   IsaT(1800)
   DEF IndexedDecisionWitnessSupportAt,
       IndexedHistoricalTemporalSupportAt,
       IndexedHistoricalCertificateLocalLineageInvariantAt,
       IndexedHistoricalCertificateReceivedQcLineageInvariantAt,
       IndexedHistoricalCertificateDecisionWalLineageInvariantAt,
       IndexedHistoricalCertificateScheduledImportProvenanceInvariantAt,
       IndexedHistoricalCertificateReceivedQcLineageSource,
       IndexedHistoricalCertificateDecisionWalLineageSource,
       IndexedHistoricalCertificateLineageCandidateFor,
       IndexedHistoricalCertificateCommandLineage,
       IndexedHistoricalCommitResponseIdentity,
       IndexedHistoricalCommitRequestIdentity,
       IndexedDecisionWitness!AsyncStrongTypeInvariant,
       IndexedDecisionWitness!AsyncProgressOwnershipInvariant,
       IndexedDecisionWitness!AsyncCandidateServiceTombstoneLifecycleInvariant,
       IndexedHistoricalTransport!
         HistoricalTemporalIdentityLifecycleInvariant,
       IndexedHistoricalTransport!
         AsyncCandidateServiceTombstoneLifecycleInvariant,
       IndexedDecisionWitness!AsyncCommitImportExecutionProvenance,
       IndexedDecisionWitness!AsyncCommitImportExecutionNeedsLineage,
       IndexedDecisionWitness!AsyncCommitImportCandidateLineage,
       IndexedDecisionWitness!AsyncCommitImportDirectEvidence,
       IndexedDecisionWitness!AsyncCommitImportResponseEvidence,
       IndexedDecisionWitness!ProtectedCandidateOwned,
       IndexedDecisionWitness!ProtectedServiceCandidate,
       IndexedDecisionWitness!CandidateConsumerCurrent,
       IndexedDecisionWitness!CandidateScheduled,
       IndexedDecisionWitness!CandidateScheduledIn,
       IndexedDecisionWitness!CandidateScheduledAfter,
       IndexedDecisionWitness!SequenceSet,
       IndexedDecisionWitness!CommandDispatchable,
       IndexedDecisionWitness!CommandSuccessors,
       IndexedDecisionWitness!CausalCandidate,
       IndexedDecisionWitness!CausalCandidateWithEvidence,
       IndexedDecisionWitness!AppendCausalSuccessors,
       IndexedDecisionWitness!FreshCommandSuccessors,
       IndexedDecisionWitness!EnqueueCandidate,
       IndexedDecisionWitness!ExecuteCommand,
       IndexedDecisionWitness!ExecuteRegularCommand,
       IndexedDecisionWitness!ExecuteCoreDelivery,
       IndexedDecisionWitness!ExecutePersistDecision,
       IndexedDecisionWitness!DeliverQC,
       IndexedDecisionWitness!QcDeliveryCreatesReceipt,
       IndexedDecisionWitness!BeginDecision,
       IndexedDecisionWitness!PersistDecision,
       IndexedDecisionWitness!DecisionWal,
       IndexedDecisionWitness!NodeHasDecision,
       IndexedDecisionWitness!NodeHasApplication,
       IndexedDecisionWitness!
         AsyncCandidateSameOriginPhysicalOrDurableOwnerAfter,
       IndexedDecisionWitness!
         AsyncCandidateMonotoneSemanticCoverageAfterIn,
       IndexedDecisionWitness!
         AsyncCandidateReducerStageCoveredAfterIn,
       IndexedDecisionWitness!AsyncCandidateDecisionStageCoveredAfter,
       IndexedDecisionWitness!AsyncCandidateConsumerEpisodeObsoleteAfter,
       IndexedDecisionWitness!AsyncCandidateTerminalTombstoned,
       IndexedDecisionWitness!AsyncNext,
       IndexedDecisionWitness!AsyncNonCrashStep,
       IndexedDecisionWitness!AsyncEnterIndexedServiceActivation,
       IndexedDecisionWitness!AsyncActivateServiceNode,
       IndexedDecisionWitness!AsyncServiceActivationTransition,
       IndexedDecisionWitness!AsyncServiceActivationFrameVars,
       IndexedDecisionWitness!AsyncSchedulerExceptServiceActivation,
       IndexedDecisionWitness!AsyncRunnerStep,
       IndexedDecisionWitness!AsyncNonRunnerStep,
       IndexedDecisionWitness!RunNode,
       IndexedDecisionWitness!RunHistoricalRecoveryNode,
       IndexedDecisionWitness!RunNodeWork,
       IndexedDecisionWitness!SerializedRunnerRuntimeStep,
       IndexedDecisionWitness!SerializedRuntimePrecedesServeIngressStep,
       IndexedDecisionWitness!SerializedLocalPrecedesServeIngressStep,
       IndexedDecisionWitness!AsyncServeIngressTargetOnlyTurn,
       IndexedDecisionWitness!SelectedLocalAdmissionAdvance,
       IndexedDecisionWitness!RunHistoricalServer,
       IndexedDecisionWitness!LocalAdmissionStep,
       IndexedDecisionWitness!IngressDrainStep,
       IndexedDecisionWitness!SerializedRuntimeStep,
       IndexedDecisionWitness!RuntimeStep,
       IndexedDecisionWitness!FifoRuntimeStep,
       IndexedDecisionWitness!DeferredDrainStep,
       IndexedDecisionWitness!ServiceIoWorker,
       IndexedDecisionWitness!ServiceHistoricalRecoveryIoWorker,
       IndexedDecisionWitness!AsyncNetworkStep,
       IndexedDecisionWitness!AsyncFaultStep,
       IndexedDecisionWitness!PreGstCrash,
       IndexedDecisionWitness!PreGstResponsiveCrash,
       IndexedDecisionWitness!PreGstResponsiveRestart,
       IndexedDecisionWitness!PreGstResponsiveReplay,
       IndexedDecisionWitness!ResetNodeSchedulerForRestart,
       IndexedDecisionWitness!AsyncAllVars,
       IndexedDecisionWitness!AsyncProducerVars,
       IndexedAsync!QcAt,
       IndexedAsync!DecisionWal,
       IndexedAsync!QcEnvelope,
       IndexedAsync!DiscoveredCommitQcItem,
       IndexedCore, IndexedScheduler, IndexedProducer
  <1> QED BY <1>1

THEOREM IndexedBracketStepPreservesHistoricalCertificateLocalLineage ==
  /\ IndexedDecisionWitnessSupport
  /\ \A initialContext \in AdmissibleContextRecords:
       IndexedHistoricalTemporalSupportAt(initialContext)
  /\ IndexedHistoricalCertificateLocalLineageInvariant
  /\ [IndexedChainNext]_IndexedChainVars
  => IndexedHistoricalCertificateLocalLineageInvariant'
PROOF
  <1>1. ASSUME IndexedDecisionWitnessSupport,
                \A initialContext \in AdmissibleContextRecords:
                  IndexedHistoricalTemporalSupportAt(initialContext),
                IndexedHistoricalCertificateLocalLineageInvariant,
                [IndexedChainNext]_IndexedChainVars,
                NEW initialContext \in AdmissibleContextRecords
         PROVE IndexedHistoricalCertificateLocalLineageInvariantAt(
                 initialContext)'
    <2>1. IndexedDecisionWitnessSupportAt(initialContext)
      BY <1>1 DEF IndexedDecisionWitnessSupport
    <2>2. IndexedHistoricalCertificateLocalLineageInvariantAt(
             initialContext)
      BY <1>1
         DEF IndexedHistoricalCertificateLocalLineageInvariant
    <2>3. IndexedHistoricalTemporalSupportAt(initialContext)
      BY <1>1
    <2>4. [IndexedDecisionWitness(initialContext)!AsyncNext]_(
             IndexedDecisionWitness(initialContext)!AsyncAllVars)
      BY <1>1, IndexedBracketStepProjectsEveryDecisionWitnessStep
    <2> QED BY <2>1, <2>2, <2>3, <2>4,
         IndexedDecisionWitnessBracketPreservesHistoricalCertificateLocalLineage
  <1> QED BY <1>1
       DEF IndexedHistoricalCertificateLocalLineageInvariant

THEOREM IndexedChainSpecAlwaysHistoricalCertificateLocalLineage ==
  IndexedChainSpec
    => []IndexedHistoricalCertificateLocalLineageInvariant
PROOF
  <1>1. IndexedChainInit
           => IndexedHistoricalCertificateLocalLineageInvariant
    BY IndexedInitEstablishesHistoricalCertificateLocalLineage
  <1>2. IndexedChainSpec => []IndexedDecisionWitnessSupport
    BY IndexedChainSpecAlwaysDecisionWitnessSupport
  <1>3. IndexedChainSpec
           => [](\A initialContext \in AdmissibleContextRecords:
                  IndexedHistoricalTemporalSupportAt(initialContext))
    BY IndexedChainSpecAlwaysHistoricalTemporalSupport
  <1>4. /\ IndexedDecisionWitnessSupport
         /\ \A initialContext \in AdmissibleContextRecords:
              IndexedHistoricalTemporalSupportAt(initialContext)
         /\ IndexedHistoricalCertificateLocalLineageInvariant
         /\ [IndexedChainNext]_IndexedChainVars
         => IndexedHistoricalCertificateLocalLineageInvariant'
    BY IndexedBracketStepPreservesHistoricalCertificateLocalLineage
  <1> QED BY <1>1, <1>2, <1>3, <1>4, PTL DEF IndexedChainSpec

IndexedHistoricalCommitCertificateImported(
    initialContext, node) ==
  /\ IndexedHistoricalRecoveryTargetOwned(initialContext, node)
  /\ \E qc \in IndexedCore(initialContext, 23):
       /\ qc.context = initialContext
       /\ qc.phase = "Commit"
       /\ \/ IndexedAsync(initialContext)!QcAt(node, qc)
               \in IndexedCore(initialContext, 15)
          \/ IndexedAsync(initialContext)!DecisionWal(node, qc, FALSE)
               \in IndexedCore(initialContext, 39)
          \/ \E candidate:
               IndexedHistoricalCertificateCommandFor(
                 initialContext, node, qc, candidate)

(***************************************************************************
Certificate rank:

  4  exact OpenHistoricalRecovery target, before request ownership
  3  exact request registration/packet/ingress/fresh Serve job
  2  exact CommitCertificateResponse published by a serving archive
  1  target-specific CommitQC receipt/Decision-WAL or exact current protected
     command owner

Later owners are excluded from each higher rank.  This makes every temporal
kernel a strict descent and prevents append-only sent history from masquerading
as progress after a recipient-specific import already exists.
***************************************************************************)

IndexedHistoricalCertificateStageAt(
    initialContext, node, rank) ==
  /\ rank \in 1..4
  /\ IndexedHistoricalRecoveryTargetOwned(initialContext, node)
  /\ ~IndexedHistoricalDecisionOwned(initialContext, node)
  /\ CASE rank = 4 ->
            /\ ~IndexedHistoricalCommitRequestOwned(
                  initialContext, node)
            /\ ~IndexedHistoricalCommitResponsePublished(
                  initialContext, node)
            /\ ~IndexedHistoricalCommitCertificateImported(
                  initialContext, node)
       [] rank = 3 ->
            /\ IndexedHistoricalCommitRequestOwned(
                 initialContext, node)
            /\ ~IndexedHistoricalCommitResponsePublished(
                  initialContext, node)
            /\ ~IndexedHistoricalCommitCertificateImported(
                  initialContext, node)
       [] rank = 2 ->
            /\ IndexedHistoricalCommitResponsePublished(
                 initialContext, node)
            /\ ~IndexedHistoricalCommitCertificateImported(
                  initialContext, node)
       [] rank = 1 ->
            IndexedHistoricalCommitCertificateImported(
              initialContext, node)
       [] OTHER -> FALSE

IndexedHistoricalCertificateGoal(initialContext, node) ==
  \/ IndexedHistoricalExactApplication(initialContext, node)
  \/ IndexedHistoricalDecisionOwned(initialContext, node)

THEOREM IndexedHistoricalTargetHasExactCertificateStage ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    IndexedHistoricalRecoveryTargetOwned(initialContext, node)
      => \/ IndexedHistoricalCertificateGoal(initialContext, node)
         \/ \E rank \in 1..4:
              IndexedHistoricalCertificateStageAt(
                initialContext, node, rank)
BY Isa
   DEF IndexedHistoricalCertificateGoal,
       IndexedHistoricalCertificateStageAt

(***************************************************************************
Exact durable-Decision body corridor.

Rank 5 names only the exact active CertifiedRequest for the Decision record.
Responsive archive-route selection, body-holder availability, retransmission,
packet admission, and Serve/I/O service remain obligations of
`IndexedHistoricalDecisionCertifiedRequestResidualProperty`.
***************************************************************************)

IndexedHistoricalDecisionRecord(initialContext, node, qc) ==
  /\ [node |-> node, qc |-> qc]
       \in IndexedCore(initialContext, 48)
  /\ qc.context = initialContext
  /\ qc.phase = "Commit"

IndexedHistoricalDecisionCertifiedRequestActiveExact(
    initialContext, node, qc) ==
  \E request \in IndexedScheduler(initialContext, 37):
    request \in
      IndexedAsync(initialContext)!CertifiedRequestOutbox(node, qc)

IndexedHistoricalDecisionCandidateFor(
    initialContext, node, qc, candidate, commandKind) ==
  /\ candidate \in
       IndexedAsync(initialContext)!AsyncCandidateSet
  /\ candidate.class = "Completion"
  /\ candidate.node = node
  /\ candidate.height = qc.context.height
  /\ candidate.view = qc.view
  /\ candidate.subject = qc.subject
  /\ IndexedAsync(initialContext)!
       CandidateConsumerCurrent(candidate)
  /\ IndexedAsync(initialContext)!CandidateScheduled(candidate)
  /\ candidate.kind = commandKind
  /\ CASE commandKind = "FetchBody" ->
            candidate.evidence = qc
       [] commandKind = "FetchCertifiedBody" ->
            /\ candidate.item.kind = "CertifiedResponse"
            /\ candidate.item.envelope.recipient = node
            /\ candidate.item.envelope.height = initialContext.height
            /\ candidate.item.envelope.view = qc.view
            /\ candidate.item.envelope.subject = qc.subject
            /\ candidate.item.envelope.requestHash =
                 IndexedAsync(initialContext)!
                   AsyncCertifiedRequestHashOf(node, qc, 0)
            /\ candidate.item.envelope.signatureOwner =
                 candidate.item.envelope.responder
            /\ candidate.item.envelope.responder
                 \in IndexedAsync(initialContext)!AsyncArchiveServerIds
            /\ IndexedAsync(initialContext)!
                 CertifiedResponseAuthenticatedOccurrence(
                   candidate.item)
            /\ IndexedAsync(initialContext)!
                 CertifiedResponseCapabilityAuthorized(
                   candidate.item)
            /\ candidate =
                 IndexedAsync(initialContext)!
                   CertifiedResponseCandidate(candidate.item)
       [] commandKind \in
            {"StoreBody", "ValidateBody", "Apply"} -> TRUE
       [] OTHER -> FALSE

(***************************************************************************
Decision rank:

  6  FetchBody
  5  exact active CertifiedRequest owner
  4  FetchCertifiedBody
  3  StoreBody
  2  ValidateBody
  1  Apply

Rank 5 deliberately names only the exact active CertifiedRequest owner.  Route
selection, responsive body-holder availability, retransmission, packet
admission, and Serve/I/O service belong to
`IndexedHistoricalDecisionCertifiedRequestResidualProperty`; they are not
preconditions for exposing the stage owner.
***************************************************************************)

IndexedHistoricalDecisionStageAt(
    initialContext, node, rank) ==
  /\ rank \in 1..6
  /\ IndexedHistoricalDecisionOwned(initialContext, node)
  /\ \E qc:
       /\ IndexedHistoricalDecisionRecord(
            initialContext, node, qc)
       /\ CASE rank = 6 ->
                 \E candidate:
                   IndexedHistoricalDecisionCandidateFor(
                     initialContext, node, qc,
                     candidate, "FetchBody")
            [] rank = 5 ->
                 IndexedHistoricalDecisionCertifiedRequestActiveExact(
                   initialContext, node, qc)
            [] rank = 4 ->
                 \E candidate:
                   IndexedHistoricalDecisionCandidateFor(
                     initialContext, node, qc,
                     candidate, "FetchCertifiedBody")
            [] rank = 3 ->
                 \E candidate:
                   IndexedHistoricalDecisionCandidateFor(
                     initialContext, node, qc,
                     candidate, "StoreBody")
            [] rank = 2 ->
                 \E candidate:
                   IndexedHistoricalDecisionCandidateFor(
                     initialContext, node, qc,
                     candidate, "ValidateBody")
            [] rank = 1 ->
                 \E candidate:
                   IndexedHistoricalDecisionCandidateFor(
                     initialContext, node, qc,
                     candidate, "Apply")
            [] OTHER -> FALSE

IndexedHistoricalDecisionStageGoal(initialContext, node) ==
  \/ IndexedHistoricalExactApplication(initialContext, node)
  \/ \E rank \in 1..6:
       IndexedHistoricalDecisionStageAt(
         initialContext, node, rank)

(***************************************************************************
Narrow residual kernels.

The first residual is intentionally not called a packet or historical-runner
kernel.  It is the part of the existing chain premise that can precede any
historical source and includes ordinary current-voter consensus.
***************************************************************************)

IndexedHistoricalRecoveryEntryGoal(initialContext, node) ==
  \/ IndexedHistoricalExactApplication(initialContext, node)
  \/ IndexedHistoricalDecisionOwned(initialContext, node)
  \/ IndexedHistoricalRecoveryOpenable(initialContext, node)
  \/ IndexedHistoricalRecoveryTargetOwned(initialContext, node)

IndexedHistoricalRecoveryAuthorityAcquisitionResidual(
    initialContext, node) ==
  /\ HistoricalRecoveryOutstanding(initialContext, node)
  /\ ~IndexedHistoricalRecoveryEntryGoal(initialContext, node)

IndexedHistoricalRecoveryAuthorityAcquisitionResidualProperty ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    IndexedHistoricalRecoveryAuthorityAcquisitionResidual(
      initialContext, node)
      ~> IndexedHistoricalRecoveryEntryGoal(initialContext, node)

IndexedHistoricalRecoveryOpenResidual(initialContext, node) ==
  /\ IndexedHistoricalRecoveryOpenable(initialContext, node)
  /\ ~IndexedHistoricalExactApplication(initialContext, node)
  /\ ~IndexedHistoricalDecisionOwned(initialContext, node)
  /\ ~IndexedHistoricalRecoveryTargetOwned(initialContext, node)

IndexedHistoricalRecoveryOpenGoal(initialContext, node) ==
  \/ IndexedHistoricalExactApplication(initialContext, node)
  \/ IndexedHistoricalDecisionOwned(initialContext, node)
  \/ IndexedHistoricalRecoveryTargetOwned(initialContext, node)

IndexedHistoricalRecoveryOpenTargetResidualProperty ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    IndexedHistoricalRecoveryOpenResidual(initialContext, node)
      ~> IndexedHistoricalRecoveryOpenGoal(initialContext, node)

IndexedHistoricalCertificateRankProgressAt(
    initialContext, node, rank) ==
  IndexedHistoricalCertificateStageAt(
    initialContext, node, rank)
    ~> (IndexedHistoricalCertificateGoal(initialContext, node)
         \/ \E lower \in SetLessThan(
              rank, OpToRel(<, Nat), Nat):
              IndexedHistoricalCertificateStageAt(
                initialContext, node, lower))

IndexedHistoricalCertificateDiscoveryRunnerResidualProperty ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    IndexedHistoricalCertificateRankProgressAt(
      initialContext, node, 4)

IndexedHistoricalCertificateRequestServiceResidualProperty ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    IndexedHistoricalCertificateRankProgressAt(
      initialContext, node, 3)

IndexedHistoricalCertificateResponseImportResidualProperty ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    IndexedHistoricalCertificateRankProgressAt(
      initialContext, node, 2)

IndexedHistoricalCertificateImportedDecisionResidualProperty ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    IndexedHistoricalCertificateRankProgressAt(
      initialContext, node, 1)

IndexedHistoricalCertificateRankProgressResidualProperty ==
  /\ IndexedHistoricalCertificateDiscoveryRunnerResidualProperty
  /\ IndexedHistoricalCertificateRequestServiceResidualProperty
  /\ IndexedHistoricalCertificateResponseImportResidualProperty
  /\ IndexedHistoricalCertificateImportedDecisionResidualProperty

(***************************************************************************
Certificate rank 4 is now a derived product theorem.

The fixed-clock proof reaches the direct discovery action, whose dedicated
product fairness publishes the complete canonical request fanout.  The
retained indexed archive-route witness selects one exact responsive applied
server from that fanout.  If Decision or application wins the race, the
certificate goal holds; otherwise a physical registered request rules out
rank 4 and the exact stage partition exposes rank 3, 2, or 1.  No transport
kernel or lower certificate rank is assumed here.
***************************************************************************)

THEOREM IndexedHistoricalDiscoveryOwnedOutcomeDropsCertificateRankFour ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    /\ IndexedCompositionInvariant
    /\ IndexedHistoricalTemporalSupportAt(initialContext)
    /\ IndexedHistoricalCommitArchiveRouteAvailabilityInvariant
    /\ IndexedHistoricalDiscoveryOwnedOutcome(initialContext, node)
    => \/ IndexedHistoricalCertificateGoal(initialContext, node)
       \/ \E lower \in SetLessThan(4, OpToRel(<, Nat), Nat):
            IndexedHistoricalCertificateStageAt(
              initialContext, node, lower)
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW node \in Responsive,
                IndexedCompositionInvariant,
                IndexedHistoricalTemporalSupportAt(initialContext),
                IndexedHistoricalCommitArchiveRouteAvailabilityInvariant,
                IndexedHistoricalDiscoveryOwnedOutcome(
                  initialContext, node)
         PROVE \/ IndexedHistoricalCertificateGoal(
                     initialContext, node)
               \/ \E lower \in SetLessThan(
                    4, OpToRel(<, Nat), Nat):
                    IndexedHistoricalCertificateStageAt(
                      initialContext, node, lower)
    <2>1. CASE IndexedHistoricalTransport(initialContext)!
                  NodeHasApplication(node)
      BY <1>1, <2>1, Isa
         DEF IndexedHistoricalCertificateGoal,
             IndexedHistoricalExactApplication,
             IndexedHistoricalTransport!NodeHasApplication,
             IndexedAsync!NodeHasApplication,
             IndexedCore
    <2>2. CASE ~IndexedHistoricalTransport(initialContext)!
                   NodeHasApplication(node)
      <3>1. /\ IndexedHistoricalTransport(initialContext)!
                  HistoricalRecoveryTarget(node)
             /\ IndexedHistoricalDiscoveryOutcome(
                  initialContext, node)
        BY <1>1, <2>2
           DEF IndexedHistoricalDiscoveryOwnedOutcome
      <3>2. HistoricalRecoveryOutstanding(initialContext, node)
        BY <1>1, <2>2, <3>1, Isa
           DEF HistoricalRecoveryOutstanding,
               IndexedCompositionInvariant,
               IndexedHistoricalRecoveryTargetCoherence,
               IndexedHistoricalTransport!HistoricalRecoveryTarget,
               IndexedAsync!HistoricalRecoveryTarget,
               IndexedHistoricalTransport!NodeHasApplication,
               IndexedAsync!NodeHasApplication,
               IndexedCore, IndexedScheduler
      <3>3. CASE IndexedHistoricalTransport(initialContext)!
                    NodeHasDecision(node)
        <4>1. IndexedHistoricalDecisionOwned(initialContext, node)
          BY <3>1, <3>2, <3>3, Isa
             DEF IndexedHistoricalDecisionOwned,
                 IndexedHistoricalRecoveryRunnerOwned,
                 IndexedHistoricalTransport!NodeHasDecision,
                 IndexedAsync!NodeHasDecision,
                 IndexedHistoricalTransport!HistoricalRecoveryTarget,
                 IndexedAsync!HistoricalRecoveryTarget,
                 IndexedCore, IndexedScheduler
        <4> QED BY <4>1
             DEF IndexedHistoricalCertificateGoal
      <3>4. CASE ~IndexedHistoricalTransport(initialContext)!
                     NodeHasDecision(node)
        <4>1. IndexedHistoricalTransport(initialContext)!
                 ActiveCommitCertificateRequests(node) # {}
          BY <3>1, <3>4
             DEF IndexedHistoricalDiscoveryOutcome,
                 IndexedHistoricalTransport!
                   HistoricalCommitCertificateDiscoveryOutcome
        <4>2. IndexedHistoricalTransport(initialContext)!
                 HistoricalCommitCertificateRequestCompletenessInvariant
          BY <1>1 DEF IndexedHistoricalTemporalSupportAt
        <4>3. IndexedHistoricalTransport(initialContext)!
                 HistoricalCommitArchiveRouteAvailabilityInvariant
          BY <1>1
             DEF IndexedHistoricalCommitArchiveRouteAvailabilityInvariant
        <4>4. \E server, request:
                 /\ IndexedHistoricalTransport(initialContext)!
                      HistoricalCommitArchiveRouteAvailable(
                        node, server)
                 /\ IndexedHistoricalTransport(initialContext)!
                      HistoricalCommitRequestRegistered(
                        node, server, request)
          BY <2>2, <3>1, <3>4, <4>1, <4>2, <4>3,
             IndexedHistoricalTransport(initialContext)!
               CompleteHistoricalCommitFanoutSelectsExactAppliedRoute
        <4>5. PICK server, request:
                 /\ IndexedHistoricalTransport(initialContext)!
                      HistoricalCommitArchiveRouteAvailable(
                        node, server)
                 /\ IndexedHistoricalTransport(initialContext)!
                      HistoricalCommitRequestRegistered(
                        node, server, request)
          BY <4>4
        <4>6. IndexedHistoricalCommitRequestOwned(
                 initialContext, node)
          BY <1>1, <3>1, <4>5, Isa
             DEF IndexedHistoricalCommitRequestOwned,
                 IndexedHistoricalCommitRequestIdentity,
                 IndexedHistoricalRequestPhysicalOwner,
                 IndexedHistoricalRecoveryTargetOwned,
                 IndexedHistoricalTransport!
                   HistoricalCommitArchiveRouteAvailable,
                 IndexedHistoricalTransport!
                   HistoricalCommitRequestRegistered,
                 IndexedHistoricalTransport!
                   HistoricalCommitRequestOccurrence,
                 IndexedHistoricalTransport!
                   HistoricalRecoveryTarget,
                 IndexedAsync!HistoricalRecoveryTarget,
                 IndexedHistoricalTransport!CurrentVoters,
                 IndexedAsync!CurrentVoters,
                 IndexedHistoricalTransport!
                   AsyncResponsiveAppliedArchiveServers,
                 IndexedAsync!AsyncResponsiveAppliedArchiveServers,
                 IndexedScheduler
        <4>7. \/ IndexedHistoricalCertificateGoal(
                       initialContext, node)
                 \/ \E rank \in 1..4:
                      IndexedHistoricalCertificateStageAt(
                        initialContext, node, rank)
          BY <3>1, <3>2,
             IndexedHistoricalTargetHasExactCertificateStage
             DEF IndexedHistoricalRecoveryTargetOwned
        <4> QED BY <4>6, <4>7, Isa
             DEF IndexedHistoricalCertificateStageAt,
                 SetLessThan, OpToRel
      <3> QED BY <3>3, <3>4
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM IndexedChainSpecClosesHistoricalCertificateDiscoveryRank ==
  /\ IndexedChainSpec
  /\ IndexedHistoricalDiscoveryClockProgressProperty
  => IndexedHistoricalCertificateDiscoveryRunnerResidualProperty
PROOF
  <1>1. ASSUME IndexedChainSpec,
                IndexedHistoricalDiscoveryClockProgressProperty
         PROVE IndexedHistoricalCertificateDiscoveryRunnerResidualProperty
    <2>1. []IndexedCompositionInvariant
      BY <1>1, IndexedChainSpecEstablishesCompositionInvariant
    <2>2. [](\A initialContext \in AdmissibleContextRecords:
               IndexedHistoricalTemporalSupportAt(initialContext))
      BY <1>1, IndexedChainSpecAlwaysHistoricalTemporalSupport
    <2>3. []IndexedHistoricalCommitArchiveRouteAvailabilityInvariant
      BY <1>1,
         IndexedChainSpecAlwaysHasHistoricalCommitArchiveRoute
    <2>4. ASSUME NEW initialContext \in AdmissibleContextRecords,
                  NEW node \in Responsive
           PROVE IndexedHistoricalCertificateRankProgressAt(
                   initialContext, node, 4)
      <3>1. [](IndexedHistoricalCertificateStageAt(
                  initialContext, node, 4)
                => /\ IndexedCore(initialContext, 7)
                   /\ IndexedHistoricalTransport(initialContext)!
                        HistoricalRecoveryTarget(node))
        BY <2>2, Isa, PTL
           DEF IndexedHistoricalCertificateStageAt,
               IndexedHistoricalRecoveryTargetOwned,
               IndexedHistoricalTemporalSupportAt,
               IndexedHistoricalTransport!AsyncStrongTypeInvariant,
               IndexedHistoricalTransport!StrongInductiveInvariant,
               IndexedHistoricalTransport!Safety,
               IndexedHistoricalTransport!TypeInvariant,
               IndexedHistoricalTransport!AsyncSchedulerTypeInvariant,
               IndexedHistoricalTransport!
                 AsyncHistoricalRecoveryTypeInvariant,
               IndexedHistoricalTransport!HistoricalRecoveryTarget
      <3>2. (IndexedCore(initialContext, 7)
                /\ IndexedHistoricalTransport(initialContext)!
                     HistoricalRecoveryTarget(node))
               ~>
             IndexedHistoricalDiscoveryOwnedOutcome(
               initialContext, node)
        BY <1>1,
           IndexedChainSpecClosesOwnedHistoricalDiscoveryCorridor
      <3>3. [](IndexedHistoricalDiscoveryOwnedOutcome(
                  initialContext, node)
                => \/ IndexedHistoricalCertificateGoal(
                         initialContext, node)
                   \/ \E lower \in SetLessThan(
                        4, OpToRel(<, Nat), Nat):
                        IndexedHistoricalCertificateStageAt(
                          initialContext, node, lower))
        BY <2>1, <2>2, <2>3,
           IndexedHistoricalDiscoveryOwnedOutcomeDropsCertificateRankFour,
           PTL
      <3> QED BY <3>1, <3>2, <3>3, PTL
           DEF IndexedHistoricalCertificateRankProgressAt
    <2> QED BY <2>4
         DEF IndexedHistoricalCertificateDiscoveryRunnerResidualProperty
  <1> QED BY <1>1

THEOREM IndexedHistoricalFixedClockExactResidualsCloseCertificateDiscoveryRank ==
  /\ IndexedChainSpec
  /\ IndexedHistoricalFixedClockPacketCorridorTemporalResidual
  => IndexedHistoricalCertificateDiscoveryRunnerResidualProperty
PROOF
  <1>1. ASSUME IndexedChainSpec,
                IndexedHistoricalFixedClockPacketCorridorTemporalResidual
         PROVE IndexedHistoricalCertificateDiscoveryRunnerResidualProperty
    <2>1. IndexedHistoricalFixedClockNonPacketServiceProperty
      BY <1>1,
         IndexedChainSpecClosesHistoricalFixedClockNonPacketService
    <2> QED BY <1>1, <2>1,
         IndexedHistoricalFixedClockExactResidualsEstablishPrerequisiteSurface,
         IndexedHistoricalFixedClockPrerequisitesCloseDiscoveryClockProgress,
         IndexedChainSpecClosesHistoricalCertificateDiscoveryRank
  <1> QED BY <1>1

(***************************************************************************
Exact Commit transport discharges certificate ranks 3 and 2.

Both ranks retain an exact active CommitCertificateRequest.  Rank 3 names a
physical request owner; rank 2 additionally records the authenticated
response publication, but the matching active request is not retired until
the response acquires its target-local DeliverQC owner.  Consequently the
same exact transport leaf applies to either rank.  The target is carried
through that leaf by the indexed target-unless-application theorem, so the
transport leaf's `~HistoricalRecoveryTarget` exit cannot be mistaken for
progress without an exact application receipt.
***************************************************************************)

IndexedHistoricalCommitTransportOwnedOutcome(initialContext, node) ==
  \/ IndexedHistoricalTransport(initialContext)!
       NodeHasApplication(node)
  \/ /\ IndexedHistoricalTransport(initialContext)!
          HistoricalRecoveryTarget(node)
     /\ IndexedHistoricalTransport(initialContext)!
          HistoricalCommitTransportGoal(node)

THEOREM IndexedHistoricalCertificateTransportRankHasExactSource ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive,
     rank \in 2..3:
    /\ IndexedCompositionInvariant
    /\ IndexedHistoricalTemporalSupportAt(initialContext)
    /\ IndexedHistoricalCertificateStageAt(
         initialContext, node, rank)
    => /\ IndexedHistoricalTransport(initialContext)!
             HistoricalRecoveryTarget(node)
       /\ IndexedHistoricalTransport(initialContext)!
             HistoricalCommitRequestSource(node)
BY IsaT(600)
   DEF IndexedHistoricalCertificateStageAt,
       IndexedHistoricalRecoveryTargetOwned,
       IndexedHistoricalCommitRequestOwned,
       IndexedHistoricalCommitResponsePublished,
       IndexedHistoricalCommitRequestIdentity,
       IndexedHistoricalRequestPhysicalOwner,
       IndexedHistoricalTransport!HistoricalCommitRequestSource,
       IndexedHistoricalTransport!HistoricalCommitRequestRegistered,
       IndexedHistoricalTransport!HistoricalCommitRequestOccurrence,
       IndexedHistoricalTransport!ActiveCommitCertificateRequests,
       IndexedHistoricalTemporalSupportAt,
       IndexedCompositionInvariant,
       HistoricalRecoveryOutstanding

THEOREM IndexedHistoricalCommitTransportOutcomeDropsCertificateRank ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive,
     rank \in 2..3:
    /\ IndexedCompositionInvariant
    /\ IndexedHistoricalTemporalSupportAt(initialContext)
    /\ IndexedHistoricalCommitTransportOwnedOutcome(
         initialContext, node)
    => \/ IndexedHistoricalCertificateGoal(initialContext, node)
       \/ \E lower \in SetLessThan(rank, OpToRel(<, Nat), Nat):
            IndexedHistoricalCertificateStageAt(
              initialContext, node, lower)
BY IsaT(900)
   DEF IndexedHistoricalCommitTransportOwnedOutcome,
       IndexedHistoricalCertificateGoal,
       IndexedHistoricalCertificateStageAt,
       IndexedHistoricalExactApplication,
       IndexedHistoricalDecisionOwned,
       IndexedHistoricalRecoveryRunnerOwned,
       IndexedHistoricalRecoveryTargetOwned,
       IndexedHistoricalCommitCertificateImported,
       IndexedHistoricalCertificateCommandFor,
       IndexedHistoricalCertificateCommandLineage,
       IndexedHistoricalTransport!HistoricalCommitTransportGoal,
       IndexedHistoricalTransport!HistoricalCommitDeliverQcOwner,
       IndexedHistoricalTransport!HistoricalCommitResponseLineage,
       IndexedHistoricalTransport!HistoricalProtectedCandidateOwned,
       IndexedHistoricalTransport!ProtectedCandidateOwned,
       IndexedHistoricalTransport!CommitCertificateResponseCandidate,
       IndexedHistoricalTransport!DiscoveredCommitQcItem,
       IndexedHistoricalTransport!NodeHasApplication,
       IndexedHistoricalTransport!NodeHasDecision,
       IndexedHistoricalTemporalSupportAt,
       IndexedCompositionInvariant,
       HistoricalRecoveryOutstanding,
       SetLessThan, OpToRel

THEOREM IndexedChainSpecClosesOwnedHistoricalCommitTransport ==
  /\ IndexedChainSpec
  /\ IndexedHistoricalCommitCertificateTransportLeafProperty
  => \A initialContext \in AdmissibleContextRecords,
       node \in Responsive,
       rank \in 2..3:
       IndexedHistoricalCertificateStageAt(
         initialContext, node, rank)
         ~> IndexedHistoricalCommitTransportOwnedOutcome(
              initialContext, node)
PROOF
  <1>1. ASSUME IndexedChainSpec,
                IndexedHistoricalCommitCertificateTransportLeafProperty
         PROVE \A initialContext \in AdmissibleContextRecords,
                  node \in Responsive,
                  rank \in 2..3:
                  IndexedHistoricalCertificateStageAt(
                    initialContext, node, rank)
                    ~> IndexedHistoricalCommitTransportOwnedOutcome(
                         initialContext, node)
    <2>1. []IndexedCompositionInvariant
      BY <1>1, IndexedChainSpecEstablishesCompositionInvariant
    <2>2. [](\A initialContext \in AdmissibleContextRecords:
               IndexedHistoricalTemporalSupportAt(initialContext))
      BY <1>1, IndexedChainSpecAlwaysHistoricalTemporalSupport
    <2>3. [][IndexedChainNext]_IndexedChainVars
      BY <1>1 DEF IndexedChainSpec
    <2>4. ASSUME NEW initialContext \in AdmissibleContextRecords,
                  NEW node \in Responsive,
                  NEW rank \in 2..3
           PROVE IndexedHistoricalCertificateStageAt(
                   initialContext, node, rank)
                   ~> IndexedHistoricalCommitTransportOwnedOutcome(
                        initialContext, node)
      <3>1. [](IndexedHistoricalCertificateStageAt(
                 initialContext, node, rank)
                => /\ IndexedHistoricalTransport(initialContext)!
                         HistoricalRecoveryTarget(node)
                   /\ IndexedHistoricalTransport(initialContext)!
                         HistoricalCommitRequestSource(node))
        BY <2>1, <2>2,
           IndexedHistoricalCertificateTransportRankHasExactSource,
           PTL
      <3>2. IndexedHistoricalTransport(initialContext)!
               HistoricalCommitRequestSource(node)
               ~> IndexedHistoricalTransport(initialContext)!
                     HistoricalCommitTransportGoal(node)
        BY <1>1
           DEF IndexedHistoricalCommitCertificateTransportLeafProperty,
               IndexedHistoricalTransport!
                 HistoricalCommitCertificateTransportLeaf
      <3>3. /\ IndexedHistoricalTemporalSupportAt(initialContext)
              /\ IndexedHistoricalTransport(initialContext)!
                   HistoricalRecoveryTarget(node)
              /\ [IndexedChainNext]_IndexedChainVars
             => \/ IndexedHistoricalTransport(initialContext)!
                      HistoricalRecoveryTarget(node)'
                \/ IndexedHistoricalTransport(initialContext)!
                      NodeHasApplication(node)'
        BY IndexedHistoricalTargetPersistsUntilApplication
      <3> QED BY <2>2, <2>3, <3>1, <3>2, <3>3, PTL
           DEF IndexedHistoricalCommitTransportOwnedOutcome
    <2> QED BY <2>4
  <1> QED BY <1>1

THEOREM IndexedHistoricalCommitTransportLeafClosesCertificateRanksTwoThree ==
  /\ IndexedChainSpec
  /\ IndexedHistoricalCommitCertificateTransportLeafProperty
  => /\ IndexedHistoricalCertificateRequestServiceResidualProperty
     /\ IndexedHistoricalCertificateResponseImportResidualProperty
BY IndexedChainSpecClosesOwnedHistoricalCommitTransport,
   IndexedHistoricalCommitTransportOutcomeDropsCertificateRank,
   PTL
   DEF IndexedHistoricalCertificateRequestServiceResidualProperty,
       IndexedHistoricalCertificateResponseImportResidualProperty,
       IndexedHistoricalCertificateRankProgressAt

IndexedHistoricalDecisionStageOwnershipResidual(
    initialContext, node) ==
  /\ IndexedHistoricalDecisionOwned(initialContext, node)
  /\ ~IndexedHistoricalDecisionStageGoal(initialContext, node)

IndexedHistoricalDecisionStageOwnershipResidualProperty ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    IndexedHistoricalDecisionStageOwnershipResidual(
      initialContext, node)
      ~> IndexedHistoricalDecisionStageGoal(initialContext, node)

(***************************************************************************
The Decision-stage ownership residual is a safety seam, not a scheduler
fairness seam.

The final witness invariant retains an exact stage for every Decision whose
owner is either a current responsive voter or an exact historical target.
The indexed chain product permanently retains the initialized `Eligible`
recovery phase, so the crash/replay authority alternative in that invariant
is impossible.  Exact stage decomposition then maps definitionally to one of
the six indexed body ranks (or exact application).
***************************************************************************)

THEOREM IndexedEligibleRecoveryExcludesDecisionRecoveryAuthority ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedResponsiveRecoveryDormant
      => \A node, qc:
           ~IndexedDecisionWitness(initialContext)!
              DecisionRecoveryAuthority(node, qc)
BY Isa
   DEF IndexedResponsiveRecoveryDormant,
       IndexedDecisionWitness!DecisionRecoveryAuthority,
       IndexedDecisionWitness!DurableDecisionRecoveryAuthority,
       IndexedRecovery

THEOREM IndexedHistoricalDecisionOwnerIsExactWitnessSource ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    IndexedHistoricalDecisionOwned(initialContext, node)
      => /\ IndexedDecisionWitness(initialContext)!NodeHasDecision(node)
         /\ IndexedDecisionWitness(initialContext)!
              DecisionExactSourceOwner(node)
BY Isa
   DEF IndexedHistoricalDecisionOwned,
       IndexedHistoricalRecoveryRunnerOwned,
       IndexedDecisionWitness!NodeHasDecision,
       IndexedDecisionWitness!DecisionExactSourceOwner,
       IndexedDecisionWitness!AsyncCurrentResponsiveVoters,
       IndexedDecisionWitness!HistoricalRecoveryTarget,
       IndexedAsync!NodeHasDecision,
       IndexedAsync!AsyncCurrentResponsiveVoters,
       IndexedAsync!HistoricalRecoveryTarget

THEOREM IndexedHistoricalDecisionOwnerHasExactRecoveryStage ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    /\ IndexedCompositionInvariant
    /\ IndexedDecisionWitnessSupportAt(initialContext)
    /\ IndexedResponsiveRecoveryDormant
    /\ IndexedHistoricalDecisionOwned(initialContext, node)
    => \E qc:
         /\ IndexedHistoricalDecisionRecord(initialContext, node, qc)
         /\ IndexedDecisionWitness(initialContext)!
              DecisionRecoveryStageExact(node, qc)
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW node \in Responsive,
                IndexedCompositionInvariant,
                IndexedDecisionWitnessSupportAt(initialContext),
                IndexedResponsiveRecoveryDormant,
                IndexedHistoricalDecisionOwned(initialContext, node)
         PROVE \E qc:
                 /\ IndexedHistoricalDecisionRecord(
                      initialContext, node, qc)
                 /\ IndexedDecisionWitness(initialContext)!
                      DecisionRecoveryStageExact(node, qc)
    <2>1. /\ IndexedDecisionWitness(initialContext)!NodeHasDecision(node)
           /\ IndexedDecisionWitness(initialContext)!
                DecisionExactSourceOwner(node)
      BY <1>1, IndexedHistoricalDecisionOwnerIsExactWitnessSource
    <2>2. IndexedDecisionWitness(initialContext)!
             DecisionExactSourceRetentionInvariant
      BY <1>1
         DEF IndexedDecisionWitnessSupportAt,
             IndexedDecisionWitness!FinalProgressWitnessClosureInvariant,
             IndexedDecisionWitness!FinalWitnessSourceRetentionInvariant
    <2>3. \E qc:
             /\ IndexedHistoricalDecisionRecord(initialContext, node, qc)
             /\ IndexedDecisionWitness(initialContext)!
                  AsyncDecisionRecoveryStageExact(node, qc)
      BY <1>1, <2>1, <2>2, IsaT(180)
         DEF IndexedDecisionWitness!NodeHasDecision,
             IndexedDecisionWitness!
               DecisionExactSourceRetentionInvariant,
             IndexedDecisionWitness!AsyncStrongTypeInvariant,
             IndexedDecisionWitness!StrongInductiveInvariant,
             IndexedDecisionWitness!Safety,
             IndexedDecisionWitness!TypeInvariant,
             IndexedDecisionWitness!DecisionAgreement,
             IndexedDecisionWitness!ReducerProvenanceInvariant,
             IndexedDecisionWitness!CertificatesBackedByIntents,
             IndexedDecisionWitness!HistoricalQcValid,
             IndexedHistoricalDecisionRecord,
             IndexedCompositionInvariant,
             IndexedTotalReceiptProjection,
             IndexedDecisionReceiptProjection,
             IndexedDecisionEvidence,
             IndexedCurrentDecisions,
             IndexedDecisions,
             Chain!ChainEpochInvariant,
             Chain!ChainEpochTypeInvariant,
             Chain!DecisionEvidenceSet
    <2>4. \A qc:
             ~IndexedDecisionWitness(initialContext)!
                DecisionRecoveryAuthority(node, qc)
      BY <1>1, IndexedEligibleRecoveryExcludesDecisionRecoveryAuthority
    <2> QED BY <2>3, <2>4, Isa
         DEF IndexedDecisionWitness!AsyncDecisionRecoveryStageExact
  <1> QED BY <1>1

THEOREM IndexedExactRecoveryStageProjectsHistoricalDecisionStageGoal ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    \A qc:
      /\ IndexedHistoricalDecisionOwned(initialContext, node)
      /\ IndexedHistoricalDecisionRecord(initialContext, node, qc)
      /\ IndexedDecisionWitness(initialContext)!
           DecisionRecoveryStageExact(node, qc)
      => IndexedHistoricalDecisionStageGoal(initialContext, node)
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW node \in Responsive,
                NEW qc,
                IndexedHistoricalDecisionOwned(initialContext, node),
                IndexedHistoricalDecisionRecord(initialContext, node, qc),
                IndexedDecisionWitness(initialContext)!
                  DecisionRecoveryStageExact(node, qc)
         PROVE IndexedHistoricalDecisionStageGoal(initialContext, node)
    <2>1. \/ IndexedDecisionWitness(initialContext)!
                NodeHasApplication(node)
           \/ IndexedDecisionWitness(initialContext)!
                DecisionCertifiedRequestActiveExact(node, qc)
           \/ \E candidate \in
                  IndexedDecisionWitness(initialContext)!AsyncCandidateSet:
                IndexedDecisionWitness(initialContext)!
                  DecisionExecutableStageOwner(node, qc, candidate)
      BY <1>1,
         IndexedDecisionWitness(initialContext)!ExactDecisionStageDecomposition,
         Isa
         DEF IndexedHistoricalDecisionRecord
    <2> QED BY <1>1, <2>1, Isa
         DEF IndexedHistoricalDecisionStageGoal,
             IndexedHistoricalDecisionStageAt,
             IndexedHistoricalExactApplication,
             IndexedHistoricalDecisionCertifiedRequestActiveExact,
             IndexedHistoricalDecisionCandidateFor,
             IndexedHistoricalDecisionRecord,
             IndexedDecisionWitness!NodeHasApplication,
             IndexedDecisionWitness!DecisionCertifiedRequestActiveExact,
             IndexedDecisionWitness!DecisionExecutableStageOwner,
             IndexedDecisionWitness!DecisionPipelineCandidate,
             IndexedDecisionWitness!
               DecisionCertifiedResponseLineageExact,
             IndexedAsync!NodeHasApplication,
             IndexedAsync!CertifiedRequestOutbox,
             IndexedAsync!CandidateConsumerCurrent,
             IndexedAsync!CandidateScheduled,
             IndexedAsync!CertifiedResponseAuthenticatedOccurrence,
             IndexedAsync!CertifiedResponseCapabilityAuthorized,
             IndexedAsync!CertifiedResponseCandidate
  <1> QED BY <1>1

THEOREM IndexedHistoricalDecisionOwnerHasVisibleExactStage ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    /\ IndexedCompositionInvariant
    /\ IndexedDecisionWitnessSupportAt(initialContext)
    /\ IndexedResponsiveRecoveryDormant
    /\ IndexedHistoricalDecisionOwned(initialContext, node)
    => IndexedHistoricalDecisionStageGoal(initialContext, node)
BY IndexedHistoricalDecisionOwnerHasExactRecoveryStage,
   IndexedExactRecoveryStageProjectsHistoricalDecisionStageGoal

THEOREM IndexedHistoricalDecisionStageOwnershipResidualIsEmpty ==
  /\ IndexedCompositionInvariant
  /\ IndexedDecisionWitnessSupport
  /\ IndexedResponsiveRecoveryDormant
  => \A initialContext \in AdmissibleContextRecords,
        node \in Responsive:
       ~IndexedHistoricalDecisionStageOwnershipResidual(
          initialContext, node)
BY IndexedHistoricalDecisionOwnerHasVisibleExactStage, Isa
   DEF IndexedDecisionWitnessSupport,
       IndexedHistoricalDecisionStageOwnershipResidual

THEOREM IndexedHistoricalDecisionStageOwnershipResidualObligation ==
  IndexedChainSpec
    => IndexedHistoricalDecisionStageOwnershipResidualProperty
PROOF
  <1>1. ASSUME IndexedChainSpec
         PROVE IndexedHistoricalDecisionStageOwnershipResidualProperty
    <2>1. []IndexedDecisionWitnessSupport
      BY <1>1, IndexedChainSpecAlwaysDecisionWitnessSupport
    <2>2. []IndexedResponsiveRecoveryDormant
      BY <1>1, IndexedChainSpecKeepsResponsiveRecoveryDormant
    <2>3. []IndexedCompositionInvariant
      BY <1>1, IndexedChainSpecEstablishesCompositionInvariant
    <2>4. [](\A initialContext \in AdmissibleContextRecords,
                  node \in Responsive:
               ~IndexedHistoricalDecisionStageOwnershipResidual(
                  initialContext, node))
      BY <2>1, <2>2, <2>3,
         IndexedHistoricalDecisionStageOwnershipResidualIsEmpty, PTL
    <2> QED BY <2>4, PTL
         DEF IndexedHistoricalDecisionStageOwnershipResidualProperty
  <1> QED BY <1>1

IndexedHistoricalDecisionRankProgressAt(
    initialContext, node, rank) ==
  IndexedHistoricalDecisionStageAt(initialContext, node, rank)
    ~> (IndexedHistoricalExactApplication(initialContext, node)
         \/ \E lower \in SetLessThan(
              rank, OpToRel(<, Nat), Nat):
              IndexedHistoricalDecisionStageAt(
                initialContext, node, lower))

IndexedHistoricalDecisionFetchBodyResidualProperty ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    IndexedHistoricalDecisionRankProgressAt(
      initialContext, node, 6)

IndexedHistoricalDecisionCertifiedRequestResidualProperty ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    IndexedHistoricalDecisionRankProgressAt(
      initialContext, node, 5)

IndexedHistoricalDecisionFetchCertifiedBodyResidualProperty ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    IndexedHistoricalDecisionRankProgressAt(
      initialContext, node, 4)

IndexedHistoricalDecisionStoreBodyResidualProperty ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    IndexedHistoricalDecisionRankProgressAt(
      initialContext, node, 3)

IndexedHistoricalDecisionValidateBodyResidualProperty ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    IndexedHistoricalDecisionRankProgressAt(
      initialContext, node, 2)

IndexedHistoricalDecisionApplyResidualProperty ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    IndexedHistoricalDecisionRankProgressAt(
      initialContext, node, 1)

IndexedHistoricalDecisionRankProgressResidualProperty ==
  /\ IndexedHistoricalDecisionFetchBodyResidualProperty
  /\ IndexedHistoricalDecisionCertifiedRequestResidualProperty
  /\ IndexedHistoricalDecisionFetchCertifiedBodyResidualProperty
  /\ IndexedHistoricalDecisionStoreBodyResidualProperty
  /\ IndexedHistoricalDecisionValidateBodyResidualProperty
  /\ IndexedHistoricalDecisionApplyResidualProperty

(***************************************************************************
Explicit Decision executor split.

`IndexedHistoricalDecisionOwned` deliberately admits two executor classes:
an ordinary current responsive voter and an exact historical-recovery target.
The historical Candidate/Serve leaves below cover only the second class.  Keep
the two temporal cones separate so an automated prover cannot use a broad
`IsaT` step to discharge the ordinary owner with a target-only premise.

The ordinary branch is closed by the indexed five-leaf exact Decision service
corridor.  The target branch is closed by the historical Candidate ranks plus
the exact CertifiedRequest transport corridor.  Neither property below
assumes the other branch, authority acquisition, aggregate application, or
indexed height liveness.
***************************************************************************)

IndexedHistoricalDecisionOrdinaryRankGoal(
    initialContext, node, rank) ==
  \/ IndexedHistoricalExactApplication(initialContext, node)
  \/ \E lower \in SetLessThan(rank, OpToRel(<, Nat), Nat):
       IndexedHistoricalDecisionStageAt(initialContext, node, lower)

IndexedHistoricalDecisionOrdinaryOwnerRankProgressAt(
    initialContext, node, rank) ==
  ( /\ node \in IndexedAsync(initialContext)!
                 AsyncCurrentResponsiveVoters
    /\ IndexedHistoricalDecisionStageAt(initialContext, node, rank))
    ~> IndexedHistoricalDecisionOrdinaryRankGoal(
         initialContext, node, rank)

IndexedHistoricalDecisionTargetOwnerRankProgressAt(
    initialContext, node, rank) ==
  ( /\ IndexedAsync(initialContext)!HistoricalRecoveryTarget(node)
    /\ IndexedHistoricalDecisionStageAt(initialContext, node, rank))
    ~> (IndexedHistoricalExactApplication(initialContext, node)
         \/ \E lower \in SetLessThan(
              rank, OpToRel(<, Nat), Nat):
              IndexedHistoricalDecisionStageAt(
                initialContext, node, lower))

IndexedHistoricalDecisionOrdinaryOwnerRankProgressProperty ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive,
     rank \in 1..6:
    IndexedHistoricalDecisionOrdinaryOwnerRankProgressAt(
      initialContext, node, rank)

IndexedHistoricalDecisionTargetOwnerRankProgressProperty ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive,
     rank \in 1..6:
    IndexedHistoricalDecisionTargetOwnerRankProgressAt(
      initialContext, node, rank)

IndexedHistoricalDecisionRankProgressAtContext(initialContext) ==
  \A node \in Responsive, rank \in 1..6:
    IndexedHistoricalDecisionRankProgressAt(
      initialContext, node, rank)

THEOREM IndexedHistoricalDecisionStageHasExplicitOwnerClass ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive,
     rank \in 1..6:
    IndexedHistoricalDecisionStageAt(initialContext, node, rank)
      => \/ node \in IndexedAsync(initialContext)!
                      AsyncCurrentResponsiveVoters
         \/ IndexedAsync(initialContext)!HistoricalRecoveryTarget(node)
BY DEF IndexedHistoricalDecisionStageAt,
       IndexedHistoricalDecisionOwned,
       IndexedHistoricalRecoveryRunnerOwned

THEOREM IndexedHistoricalDecisionOrdinaryOwnerHasJoinedCurrentRunner ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive,
     rank \in 1..6:
    /\ node \in IndexedAsync(initialContext)!
                 AsyncCurrentResponsiveVoters
    /\ IndexedHistoricalDecisionStageAt(initialContext, node, rank)
    => /\ node \in joinedByContext[initialContext]
       /\ initialContext \in JoinedContexts
       /\ IndexedNodeCurrentAt(initialContext, node)
BY DEF IndexedHistoricalDecisionStageAt,
       IndexedHistoricalDecisionOwned,
       HistoricalRecoveryOutstanding,
       ExactNodeLocationAt,
       IndexedNodeCurrentAt,
       JoinedContexts

THEOREM IndexedHistoricalDecisionOrdinaryStageHasExactServiceSourceAtGst ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive,
     rank \in 1..6:
    /\ IndexedCompositionInvariant
    /\ IndexedDecisionWitnessSupportAt(initialContext)
    /\ IndexedResponsiveRecoveryDormant
    /\ IndexedCore(initialContext, 7)
    /\ node \in IndexedAsync(initialContext)!
                 AsyncCurrentResponsiveVoters
    /\ IndexedHistoricalDecisionStageAt(initialContext, node, rank)
    => \E qc:
         IndexedDecisionServiceWitness(initialContext)!
           ExactDecisionServiceSource(node, qc)
BY IndexedHistoricalDecisionOwnerHasExactRecoveryStage, Isa
   DEF IndexedHistoricalDecisionStageAt,
       IndexedHistoricalDecisionRecord,
       IndexedDecisionServiceWitness!ExactDecisionServiceSource,
       IndexedDecisionServiceWitness!ExactDecisionRecord,
       IndexedDecisionServiceWitness!DecisionRecoveryStageExact,
       IndexedDecisionWitness!DecisionRecoveryStageExact,
       IndexedAsync!AsyncCurrentResponsiveVoters,
       IndexedDecisionServiceWitness!AsyncCurrentResponsiveVoters,
       IndexedCore, IndexedScheduler, IndexedRecovery

THEOREM IndexedHistoricalDecisionOrdinaryOwnerPersistsOrGoals ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive,
     rank \in 1..6:
    LET owner ==
          /\ node \in IndexedAsync(initialContext)!
                       AsyncCurrentResponsiveVoters
          /\ IndexedHistoricalDecisionStageAt(
               initialContext, node, rank)
    IN /\ IndexedCompositionInvariant
       /\ IndexedDecisionWitnessSupportAt(initialContext)
       /\ IndexedResponsiveRecoveryDormant
       /\ owner
       /\ [IndexedChainNext]_IndexedChainVars
       => \/ owner'
          \/ IndexedHistoricalDecisionOrdinaryRankGoal(
               initialContext, node, rank)'
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW node \in Responsive,
                NEW rank \in 1..6
         PROVE LET owner ==
                     /\ node \in IndexedAsync(initialContext)!
                                  AsyncCurrentResponsiveVoters
                     /\ IndexedHistoricalDecisionStageAt(
                          initialContext, node, rank)
               IN /\ IndexedCompositionInvariant
                  /\ IndexedDecisionWitnessSupportAt(initialContext)
                  /\ IndexedResponsiveRecoveryDormant
                  /\ owner
                  /\ [IndexedChainNext]_IndexedChainVars
                  => \/ owner'
                     \/ IndexedHistoricalDecisionOrdinaryRankGoal(
                          initialContext, node, rank)'
    BY IndexedHistoricalTransport(initialContext)!
         HistoricalDecisionPipelineExactCarrierPersistsOrHandsOff,
       JoinedNonCurrentHasApplicationEvidence, IsaT(1800)
       DEF IndexedHistoricalDecisionOrdinaryRankGoal,
           IndexedHistoricalDecisionStageAt,
           IndexedHistoricalDecisionRecord,
           IndexedHistoricalDecisionOwned,
           IndexedHistoricalRecoveryRunnerOwned,
           IndexedHistoricalExactApplication,
           IndexedNodeCurrentAt,
           IndexedDecisionWitnessSupportAt,
           IndexedCompositionInvariant,
           IndexedResponsiveRecoveryDormant,
           IndexedCore, IndexedScheduler, IndexedRecovery,
           IndexedAsync!AsyncCurrentResponsiveVoters,
           IndexedAsync!NodeHasApplication,
           IndexedHistoricalTransport!AsyncCurrentResponsiveVoters,
           IndexedHistoricalTransport!NodeHasApplication,
           IndexedChainVars
  <1> QED BY <1>1

THEOREM IndexedHistoricalDecisionOwnerClassesCloseRankProgress ==
  /\ IndexedHistoricalDecisionOrdinaryOwnerRankProgressProperty
  /\ IndexedHistoricalDecisionTargetOwnerRankProgressProperty
  => IndexedHistoricalDecisionRankProgressResidualProperty
BY IndexedHistoricalDecisionStageHasExplicitOwnerClass, PTL
   DEF IndexedHistoricalDecisionOrdinaryOwnerRankProgressProperty,
       IndexedHistoricalDecisionTargetOwnerRankProgressProperty,
       IndexedHistoricalDecisionOrdinaryOwnerRankProgressAt,
       IndexedHistoricalDecisionTargetOwnerRankProgressAt,
       IndexedHistoricalDecisionRankProgressResidualProperty,
       IndexedHistoricalDecisionFetchBodyResidualProperty,
       IndexedHistoricalDecisionCertifiedRequestResidualProperty,
       IndexedHistoricalDecisionFetchCertifiedBodyResidualProperty,
       IndexedHistoricalDecisionStoreBodyResidualProperty,
       IndexedHistoricalDecisionValidateBodyResidualProperty,
       IndexedHistoricalDecisionApplyResidualProperty,
       IndexedHistoricalDecisionRankProgressAt

THEOREM IndexedHistoricalDecisionOwnerClassesCloseRankProgressAtContext ==
  \A initialContext \in AdmissibleContextRecords:
    /\ \A node \in Responsive, rank \in 1..6:
         IndexedHistoricalDecisionOrdinaryOwnerRankProgressAt(
           initialContext, node, rank)
    /\ \A node \in Responsive, rank \in 1..6:
         IndexedHistoricalDecisionTargetOwnerRankProgressAt(
           initialContext, node, rank)
    => IndexedHistoricalDecisionRankProgressAtContext(initialContext)
BY IndexedHistoricalDecisionStageHasExplicitOwnerClass, PTL
   DEF IndexedHistoricalDecisionRankProgressAtContext,
       IndexedHistoricalDecisionRankProgressAt,
       IndexedHistoricalDecisionOrdinaryOwnerRankProgressAt,
       IndexedHistoricalDecisionTargetOwnerRankProgressAt

(***************************************************************************
Derived Candidate subkernel of the Decision rank.

The indexed Stage 2..6 leaves close starvation for an exact historical
candidate.  FetchBody may expose the RequestCertifiedBody candidate first, so
the already-closed request-candidate leaf is composed before rank 5.  Ranks
4, 3, 2, and 1 are direct FetchCertifiedBody/StoreBody/ValidateBody/Apply
candidate owners.  Rank 5 is intentionally absent: its owner is an active
CertifiedRequest whose archive route, packet, Serve, ordinary-I/O response,
and target admission form the separate transport corridor.
***************************************************************************)

IndexedHistoricalDecisionCandidateRankProgressResidualProperty ==
  /\ IndexedHistoricalDecisionFetchBodyResidualProperty
  /\ IndexedHistoricalDecisionFetchCertifiedBodyResidualProperty
  /\ IndexedHistoricalDecisionStoreBodyResidualProperty
  /\ IndexedHistoricalDecisionValidateBodyResidualProperty
  /\ IndexedHistoricalDecisionApplyResidualProperty

IndexedHistoricalDecisionTargetCandidateRankProgressResidualProperty ==
  /\ \A initialContext \in AdmissibleContextRecords,
        node \in Responsive:
       IndexedHistoricalDecisionTargetOwnerRankProgressAt(
         initialContext, node, 6)
  /\ \A initialContext \in AdmissibleContextRecords,
        node \in Responsive:
       IndexedHistoricalDecisionTargetOwnerRankProgressAt(
         initialContext, node, 4)
  /\ \A initialContext \in AdmissibleContextRecords,
        node \in Responsive:
       IndexedHistoricalDecisionTargetOwnerRankProgressAt(
         initialContext, node, 3)
  /\ \A initialContext \in AdmissibleContextRecords,
        node \in Responsive:
       IndexedHistoricalDecisionTargetOwnerRankProgressAt(
         initialContext, node, 2)
  /\ \A initialContext \in AdmissibleContextRecords,
        node \in Responsive:
       IndexedHistoricalDecisionTargetOwnerRankProgressAt(
         initialContext, node, 1)

THEOREM IndexedChainSpecClosesHistoricalDecisionTargetCandidateRankResiduals ==
  IndexedChainSpec
    => IndexedHistoricalDecisionTargetCandidateRankProgressResidualProperty
BY IndexedChainSpecProvidesHistoricalFiniteRunnerEpisodeClosure,
   IndexedChainSpecClosesHistoricalDecisionBodyCandidateLeaves,
   IndexedChainSpecClosesHistoricalProtectedCandidateStarvation,
   IsaT(1200), PTL
   DEF IndexedHistoricalDecisionTargetCandidateRankProgressResidualProperty,
       IndexedHistoricalDecisionTargetOwnerRankProgressAt,
       IndexedHistoricalDecisionFetchBodyResidualProperty,
       IndexedHistoricalDecisionFetchCertifiedBodyResidualProperty,
       IndexedHistoricalDecisionStoreBodyResidualProperty,
       IndexedHistoricalDecisionValidateBodyResidualProperty,
       IndexedHistoricalDecisionApplyResidualProperty,
       IndexedHistoricalDecisionRankProgressAt,
       IndexedHistoricalDecisionStageAt,
       IndexedHistoricalExactApplication,
       IndexedHistoricalDecisionOwned,
       IndexedHistoricalDecisionRecord,
       IndexedHistoricalDecisionCertifiedRequestActiveExact,
       IndexedHistoricalDecisionCandidateFor,
       IndexedHistoricalDecisionBodyCandidateProgressLeaves,
       IndexedHistoricalTransport!HistoricalDecisionFetchProgressLeaf,
       IndexedHistoricalTransport!
         HistoricalDecisionRequestBodyProgressLeaf,
       IndexedHistoricalTransport!
         HistoricalDecisionFetchCertifiedProgressLeaf,
       IndexedHistoricalTransport!HistoricalDecisionStoreProgressLeaf,
       IndexedHistoricalTransport!HistoricalDecisionValidateProgressLeaf,
       IndexedHistoricalTransport!HistoricalDecisionApplyProgressLeaf,
       IndexedHistoricalTransport!
         HistoricalProtectedCandidateStarvationProperty,
       IndexedHistoricalTransport!
         HistoricalDecisionPipelineKindOwned,
       IndexedHistoricalTransport!
         HistoricalDecisionCertifiedRequestActive,
       IndexedHistoricalTransport!HistoricalDecisionRecordMatches,
       IndexedHistoricalTransport!DecisionPipelineKindOwned,
       IndexedHistoricalTransport!DecisionCertifiedRequestActive,
       SetLessThan

THEOREM IndexedHistoricalDecisionRankResidualSplitsAtCertifiedRequest ==
  IndexedHistoricalDecisionRankProgressResidualProperty
    <=> /\ IndexedHistoricalDecisionCandidateRankProgressResidualProperty
        /\ IndexedHistoricalDecisionCertifiedRequestResidualProperty
BY DEF IndexedHistoricalDecisionRankProgressResidualProperty,
       IndexedHistoricalDecisionCandidateRankProgressResidualProperty

IndexedHistoricalDecisionTargetCertifiedRequestResidualProperty ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    IndexedHistoricalDecisionTargetOwnerRankProgressAt(
      initialContext, node, 5)

THEOREM IndexedHistoricalDecisionTargetRankResidualSplitsAtCertifiedRequest ==
  IndexedHistoricalDecisionTargetOwnerRankProgressProperty
    <=> /\ IndexedHistoricalDecisionTargetCandidateRankProgressResidualProperty
        /\ IndexedHistoricalDecisionTargetCertifiedRequestResidualProperty
BY DEF IndexedHistoricalDecisionTargetOwnerRankProgressProperty,
       IndexedHistoricalDecisionTargetCandidateRankProgressResidualProperty,
       IndexedHistoricalDecisionTargetCertifiedRequestResidualProperty

(***************************************************************************
The exact historical CertifiedRequest transport leaf closes Decision rank 5.

The source projection below binds the active request to the exact durable
Decision QC and exact historical executor.  The transport result is either an
application receipt or the exact FetchCertifiedBody owner at rank 4.  Target
retirement is carried through the same target-unless-application frame used
by certificate discovery; it is not admitted as an independent liveness
outcome.
***************************************************************************)

IndexedHistoricalDecisionTransportOwnedOutcome(
    initialContext, node, qc) ==
  \/ IndexedHistoricalTransport(initialContext)!
       NodeHasApplication(node)
  \/ /\ IndexedHistoricalTransport(initialContext)!
          HistoricalRecoveryTarget(node)
     /\ IndexedHistoricalTransport(initialContext)!
          HistoricalDecisionCertifiedResponseGoal(node, qc)

THEOREM IndexedHistoricalDecisionRankFiveHasExactTransportSource ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    /\ IndexedCompositionInvariant
    /\ IndexedDecisionWitnessSupportAt(initialContext)
    /\ IndexedResponsiveRecoveryDormant
    /\ IndexedAsync(initialContext)!HistoricalRecoveryTarget(node)
    /\ IndexedHistoricalDecisionStageAt(initialContext, node, 5)
    => \E qc:
         /\ IndexedHistoricalDecisionRecord(initialContext, node, qc)
         /\ IndexedHistoricalTransport(initialContext)!
              HistoricalExactDecisionActiveRequestOwner(node, qc)
BY IndexedHistoricalDecisionOwnerHasExactRecoveryStage,
   IsaT(900)
   DEF IndexedHistoricalDecisionStageAt,
       IndexedHistoricalDecisionRecord,
       IndexedHistoricalDecisionCertifiedRequestActiveExact,
       IndexedHistoricalDecisionOwned,
       IndexedHistoricalRecoveryTargetOwned,
       IndexedHistoricalTransport!
         HistoricalExactDecisionActiveRequestOwner,
       IndexedHistoricalTransport!HistoricalExactDecisionServiceSource,
       IndexedHistoricalTransport!ExactDecisionRecord,
       IndexedHistoricalTransport!DecisionRecoveryStageExact,
       IndexedHistoricalTransport!
         DecisionCertifiedRequestActiveExact,
       IndexedDecisionWitness!DecisionRecoveryStageExact,
       IndexedDecisionWitness!DecisionCertifiedRequestActiveExact,
       IndexedAsync!CertifiedRequestOutbox,
       IndexedHistoricalTransport!CertifiedRequestOutbox,
       HistoricalRecoveryOutstanding,
       IndexedCompositionInvariant

THEOREM IndexedHistoricalDecisionTransportOutcomeDropsRankFive ==
  \A qc:
    \A initialContext \in AdmissibleContextRecords,
       node \in Responsive:
      /\ IndexedCompositionInvariant
      /\ IndexedDecisionWitnessSupportAt(initialContext)
      /\ IndexedHistoricalDecisionTransportOwnedOutcome(
           initialContext, node, qc)
      /\ IndexedHistoricalDecisionRecord(initialContext, node, qc)
      => \/ IndexedHistoricalExactApplication(initialContext, node)
         \/ \E lower \in SetLessThan(5, OpToRel(<, Nat), Nat):
              IndexedHistoricalDecisionStageAt(
                initialContext, node, lower)
BY IsaT(900)
   DEF IndexedHistoricalDecisionTransportOwnedOutcome,
       IndexedHistoricalDecisionStageAt,
       IndexedHistoricalDecisionRecord,
       IndexedHistoricalDecisionCandidateFor,
       IndexedHistoricalDecisionOwned,
       IndexedHistoricalRecoveryRunnerOwned,
       IndexedHistoricalRecoveryTargetOwned,
       IndexedHistoricalExactApplication,
       IndexedHistoricalTransport!
         HistoricalDecisionCertifiedResponseGoal,
       IndexedHistoricalTransport!DecisionCertifiedFetchOwnedExact,
       IndexedHistoricalTransport!DecisionCertifiedResponseLineageExact,
       IndexedHistoricalTransport!CertifiedResponseCandidate,
       IndexedHistoricalTransport!NodeHasApplication,
       IndexedHistoricalTransport!HistoricalRecoveryTarget,
       IndexedAsync!CandidateConsumerCurrent,
       IndexedAsync!CandidateScheduled,
       IndexedAsync!CertifiedResponseAuthenticatedOccurrence,
       IndexedAsync!CertifiedResponseCapabilityAuthorized,
       HistoricalRecoveryOutstanding,
       SetLessThan, OpToRel

THEOREM IndexedChainSpecClosesOwnedHistoricalDecisionTransport ==
  /\ IndexedChainSpec
  /\ IndexedHistoricalDecisionCertifiedBodyTransportLeafProperty
  => \A initialContext \in AdmissibleContextRecords,
       node \in Responsive:
       ( /\ IndexedAsync(initialContext)!HistoricalRecoveryTarget(node)
         /\ IndexedHistoricalDecisionStageAt(initialContext, node, 5))
         ~> \E qc:
              /\ IndexedHistoricalDecisionRecord(
                   initialContext, node, qc)
              /\ IndexedHistoricalDecisionTransportOwnedOutcome(
                   initialContext, node, qc)
PROOF
  <1>1. ASSUME IndexedChainSpec,
                IndexedHistoricalDecisionCertifiedBodyTransportLeafProperty
         PROVE \A initialContext \in AdmissibleContextRecords,
                  node \in Responsive:
                  ( /\ IndexedAsync(initialContext)!
                         HistoricalRecoveryTarget(node)
                    /\ IndexedHistoricalDecisionStageAt(
                         initialContext, node, 5))
                    ~> \E qc:
                         /\ IndexedHistoricalDecisionRecord(
                              initialContext, node, qc)
                         /\ IndexedHistoricalDecisionTransportOwnedOutcome(
                              initialContext, node, qc)
    <2>1. []IndexedCompositionInvariant
      BY <1>1, IndexedChainSpecEstablishesCompositionInvariant
    <2>2. []IndexedDecisionWitnessSupport
      BY <1>1, IndexedChainSpecAlwaysDecisionWitnessSupport
    <2>3. []IndexedResponsiveRecoveryDormant
      BY <1>1, IndexedChainSpecKeepsResponsiveRecoveryDormant
    <2>4. [](\A initialContext \in AdmissibleContextRecords:
               IndexedHistoricalTemporalSupportAt(initialContext))
      BY <1>1, IndexedChainSpecAlwaysHistoricalTemporalSupport
    <2>5. [][IndexedChainNext]_IndexedChainVars
      BY <1>1 DEF IndexedChainSpec
    <2>6. ASSUME NEW initialContext \in AdmissibleContextRecords,
                  NEW node \in Responsive
           PROVE ( /\ IndexedAsync(initialContext)!
                        HistoricalRecoveryTarget(node)
                   /\ IndexedHistoricalDecisionStageAt(
                        initialContext, node, 5))
                   ~> \E qc:
                        /\ IndexedHistoricalDecisionRecord(
                             initialContext, node, qc)
                        /\ IndexedHistoricalDecisionTransportOwnedOutcome(
                             initialContext, node, qc)
      <3>1. \A qc:
                IndexedHistoricalTransport(initialContext)!
                  HistoricalExactDecisionActiveRequestOwner(node, qc)
                  ~> IndexedHistoricalTransport(initialContext)!
                        HistoricalDecisionCertifiedResponseGoal(node, qc)
        BY <1>1
           DEF IndexedHistoricalDecisionCertifiedBodyTransportLeafProperty,
               IndexedHistoricalTransport!
                 HistoricalDecisionCertifiedBodyTransportLeaf
      <3>2. []( /\ IndexedAsync(initialContext)!
                     HistoricalRecoveryTarget(node)
                /\ IndexedHistoricalDecisionStageAt(
                     initialContext, node, 5)
               => \E qc:
                    /\ IndexedHistoricalDecisionRecord(
                         initialContext, node, qc)
                    /\ IndexedHistoricalTransport(initialContext)!
                         HistoricalExactDecisionActiveRequestOwner(
                           node, qc))
        BY <2>1, <2>2, <2>3,
           IndexedHistoricalDecisionRankFiveHasExactTransportSource,
           PTL DEF IndexedDecisionWitnessSupport
      <3>3. /\ IndexedHistoricalTemporalSupportAt(initialContext)
              /\ IndexedHistoricalTransport(initialContext)!
                   HistoricalRecoveryTarget(node)
              /\ [IndexedChainNext]_IndexedChainVars
             => \/ IndexedHistoricalTransport(initialContext)!
                      HistoricalRecoveryTarget(node)'
                \/ IndexedHistoricalTransport(initialContext)!
                      NodeHasApplication(node)'
        BY IndexedHistoricalTargetPersistsUntilApplication
      <3> QED BY <2>4, <2>5, <3>1, <3>2, <3>3, PTL
           DEF IndexedHistoricalDecisionTransportOwnedOutcome
    <2> QED BY <2>6
  <1> QED BY <1>1

THEOREM IndexedHistoricalDecisionTransportLeafClosesTargetRankFive ==
  /\ IndexedChainSpec
  /\ IndexedHistoricalDecisionCertifiedBodyTransportLeafProperty
  => IndexedHistoricalDecisionTargetCertifiedRequestResidualProperty
BY IndexedChainSpecClosesOwnedHistoricalDecisionTransport,
   IndexedHistoricalDecisionTransportOutcomeDropsRankFive,
   PTL
   DEF IndexedHistoricalDecisionTargetCertifiedRequestResidualProperty,
       IndexedHistoricalDecisionTargetOwnerRankProgressAt

(***************************************************************************
Derived exact-Candidate tail of certificate rank 1.

The rank-1 import predicate admits a received-QC pool entry or Decision WAL
before its exact causal successor is selected.  The theorem below therefore
closes only the exact Candidate tail—DeliverQC, BeginDecision,
PersistDecision.  It does not silently promote ranks 4..2, which remain in
the discovery/transport corridor.
***************************************************************************)

IndexedHistoricalCertificateCandidateTailAt(
    initialContext, node, kind) ==
  /\ IndexedHistoricalCertificateStageAt(initialContext, node, 1)
  /\ IndexedHistoricalTransport(initialContext)!
       HistoricalCommitDecisionCandidateOwned(node, kind)

IndexedHistoricalCertificateCandidateTailGoal(
    initialContext, node, kind) ==
  \/ IndexedHistoricalCertificateGoal(initialContext, node)
  \/ CASE kind = "DeliverQC" ->
            IndexedHistoricalTransport(initialContext)!
              HistoricalCommitDecisionCandidateOwned(
                node, "BeginDecision")
       [] kind = "BeginDecision" ->
            IndexedHistoricalTransport(initialContext)!
              HistoricalCommitDecisionCandidateOwned(
                node, "PersistDecision")
       [] kind = "PersistDecision" -> FALSE
       [] OTHER -> FALSE

IndexedHistoricalCertificateCandidateEntryGoal(initialContext, node) ==
  \/ IndexedHistoricalCertificateGoal(initialContext, node)
  \/ \E kind \in
       {"DeliverQC", "BeginDecision", "PersistDecision"}:
       IndexedHistoricalCertificateCandidateTailAt(
         initialContext, node, kind)

THEOREM IndexedHistoricalCertificateReceivedQcLineageExposesCandidateEntry ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    \A qc:
      /\ IndexedHistoricalCertificateStageAt(initialContext, node, 1)
      /\ IndexedDecisionWitnessSupportAt(initialContext)
      /\ IndexedHistoricalCertificateReceivedQcLineageInvariantAt(
           initialContext)
      /\ IndexedHistoricalCertificateReceivedQcLineageSource(
           initialContext, node, qc)
      => IndexedHistoricalCertificateCandidateEntryGoal(
           initialContext, node)
BY IndexedHistoricalCertificateCommandRefinesHistoricalOwner,
   IsaT(600)
   DEF IndexedHistoricalCertificateReceivedQcLineageInvariantAt,
       IndexedHistoricalCertificateReceivedQcLineageSource,
       IndexedHistoricalCertificateCandidateEntryGoal,
       IndexedHistoricalCertificateCandidateTailAt,
       IndexedHistoricalCertificateStageAt,
       IndexedHistoricalCertificateCommandFor,
       IndexedHistoricalCertificateGoal,
       IndexedHistoricalDecisionOwned,
       IndexedHistoricalRecoveryRunnerOwned,
       IndexedHistoricalRecoveryTargetOwned,
       IndexedHistoricalExactApplication,
       IndexedDecisionWitness!NodeHasDecision,
       IndexedDecisionWitness!NodeHasApplication,
       IndexedAsync!NodeHasDecision,
       IndexedAsync!NodeHasApplication

THEOREM IndexedHistoricalCertificateDecisionWalLineageExposesCandidateEntry ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    \A qc:
      /\ IndexedHistoricalCertificateStageAt(initialContext, node, 1)
      /\ IndexedDecisionWitnessSupportAt(initialContext)
      /\ IndexedHistoricalCertificateDecisionWalLineageInvariantAt(
           initialContext)
      /\ IndexedHistoricalCertificateDecisionWalLineageSource(
           initialContext, node, qc)
      => IndexedHistoricalCertificateCandidateEntryGoal(
           initialContext, node)
BY IndexedHistoricalCertificateCommandRefinesHistoricalOwner,
   IsaT(600)
   DEF IndexedHistoricalCertificateDecisionWalLineageInvariantAt,
       IndexedHistoricalCertificateDecisionWalLineageSource,
       IndexedHistoricalCertificateCandidateEntryGoal,
       IndexedHistoricalCertificateCandidateTailAt,
       IndexedHistoricalCertificateStageAt,
       IndexedHistoricalCertificateCommandFor,
       IndexedHistoricalCertificateGoal,
       IndexedHistoricalDecisionOwned,
       IndexedHistoricalRecoveryRunnerOwned,
       IndexedHistoricalRecoveryTargetOwned,
       IndexedHistoricalExactApplication,
       IndexedDecisionWitness!NodeHasDecision,
       IndexedDecisionWitness!NodeHasApplication,
       IndexedAsync!NodeHasDecision,
       IndexedAsync!NodeHasApplication

THEOREM IndexedHistoricalCertificateExactCommandExposesCandidateEntry ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    \A qc, candidate:
      /\ IndexedHistoricalCertificateStageAt(initialContext, node, 1)
      /\ IndexedDecisionWitnessSupportAt(initialContext)
      /\ IndexedHistoricalCertificateCommandFor(
           initialContext, node, qc, candidate)
      => IndexedHistoricalCertificateCandidateEntryGoal(
           initialContext, node)
BY IndexedHistoricalCertificateCommandRefinesHistoricalOwner
   DEF IndexedHistoricalCertificateCandidateEntryGoal,
       IndexedHistoricalCertificateCandidateTailAt

(***************************************************************************
Rank-1 local-import split.

The exact protected-command arm is already a physical scheduler owner and is
closed by the theorem above.  Production causal-origin guards now make the
received-QcAt and non-rebroadcast Decision-WAL arms separate projections of
the proved target-neutral lineage invariant.  Structural `AsyncCandidateTyped`
membership alone is still insufficient; the closure depends on the exact
execution provenance added to `FifoRuntimeStep` and `DeferredDrainStep`.
Bare `qcNetwork` history remains outside rank 1.
***************************************************************************)

IndexedHistoricalCertificateLocalImportAt(initialContext, node) ==
  /\ IndexedHistoricalCertificateStageAt(initialContext, node, 1)
  /\ \E qc \in IndexedCore(initialContext, 23):
       /\ qc.context = initialContext
       /\ qc.phase = "Commit"
       /\ \/ IndexedAsync(initialContext)!QcAt(node, qc)
                \in IndexedCore(initialContext, 15)
          \/ IndexedAsync(initialContext)!DecisionWal(node, qc, FALSE)
                \in IndexedCore(initialContext, 39)
          \/ \E candidate:
               IndexedHistoricalCertificateCommandFor(
                 initialContext, node, qc, candidate)

IndexedHistoricalCertificateReceivedQcLocalImportAt(
    initialContext, node) ==
  /\ IndexedHistoricalCertificateStageAt(initialContext, node, 1)
  /\ \E qc \in IndexedCore(initialContext, 23):
       IndexedHistoricalCertificateReceivedQcLineageSource(
         initialContext, node, qc)

IndexedHistoricalCertificateDecisionWalLocalImportAt(
    initialContext, node) ==
  /\ IndexedHistoricalCertificateStageAt(initialContext, node, 1)
  /\ \E qc \in IndexedCore(initialContext, 23):
       IndexedHistoricalCertificateDecisionWalLineageSource(
         initialContext, node, qc)

IndexedHistoricalCertificateExactCommandLocalImportAt(
    initialContext, node) ==
  /\ IndexedHistoricalCertificateStageAt(initialContext, node, 1)
  /\ \E qc \in IndexedCore(initialContext, 23):
       \E candidate:
         /\ qc.context = initialContext
         /\ qc.phase = "Commit"
         /\ IndexedHistoricalCertificateCommandFor(
              initialContext, node, qc, candidate)

THEOREM IndexedHistoricalCertificateLocalImportSplitsPhysicalSources ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    IndexedHistoricalCertificateLocalImportAt(initialContext, node)
      <=> \/ IndexedHistoricalCertificateReceivedQcLocalImportAt(
               initialContext, node)
          \/ IndexedHistoricalCertificateDecisionWalLocalImportAt(
               initialContext, node)
          \/ IndexedHistoricalCertificateExactCommandLocalImportAt(
               initialContext, node)
BY Isa
   DEF IndexedHistoricalCertificateLocalImportAt,
       IndexedHistoricalCertificateReceivedQcLocalImportAt,
       IndexedHistoricalCertificateDecisionWalLocalImportAt,
       IndexedHistoricalCertificateExactCommandLocalImportAt,
       IndexedHistoricalCertificateReceivedQcLineageSource,
       IndexedHistoricalCertificateDecisionWalLineageSource

THEOREM IndexedHistoricalCertificateRankOneIsLocalImport ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    IndexedHistoricalCertificateStageAt(initialContext, node, 1)
      => IndexedHistoricalCertificateLocalImportAt(
           initialContext, node)
BY Isa
   DEF IndexedHistoricalCertificateStageAt,
       IndexedHistoricalCommitCertificateImported,
       IndexedHistoricalCertificateLocalImportAt

IndexedHistoricalCertificateLocalImportCandidateEntryProperty ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    IndexedHistoricalCertificateLocalImportAt(initialContext, node)
      ~> IndexedHistoricalCertificateCandidateEntryGoal(
           initialContext, node)

IndexedHistoricalCertificateRankOneCandidateEntryProperty ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    IndexedHistoricalCertificateStageAt(initialContext, node, 1)
      ~> IndexedHistoricalCertificateCandidateEntryGoal(
           initialContext, node)

IndexedHistoricalCertificateReceivedQcLocalImportEntryProperty ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    IndexedHistoricalCertificateReceivedQcLocalImportAt(
      initialContext, node)
      ~> IndexedHistoricalCertificateCandidateEntryGoal(
           initialContext, node)

IndexedHistoricalCertificateDecisionWalLocalImportEntryProperty ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    IndexedHistoricalCertificateDecisionWalLocalImportAt(
      initialContext, node)
      ~> IndexedHistoricalCertificateCandidateEntryGoal(
           initialContext, node)

THEOREM IndexedChainSpecClosesHistoricalCertificateExactCommandLocalImport ==
  IndexedChainSpec
    => \A initialContext \in AdmissibleContextRecords,
          node \in Responsive:
         IndexedHistoricalCertificateExactCommandLocalImportAt(
           initialContext, node)
           ~> IndexedHistoricalCertificateCandidateEntryGoal(
                initialContext, node)
PROOF
  <1>1. ASSUME IndexedChainSpec
         PROVE \A initialContext \in AdmissibleContextRecords,
                   node \in Responsive:
                 IndexedHistoricalCertificateExactCommandLocalImportAt(
                   initialContext, node)
                   ~> IndexedHistoricalCertificateCandidateEntryGoal(
                        initialContext, node)
    <2>0. []IndexedDecisionWitnessSupport
      BY <1>1, IndexedChainSpecAlwaysDecisionWitnessSupport
    <2>2. ASSUME NEW initialContext \in AdmissibleContextRecords,
                  NEW node \in Responsive
           PROVE IndexedHistoricalCertificateExactCommandLocalImportAt(
                   initialContext, node)
                   ~> IndexedHistoricalCertificateCandidateEntryGoal(
                        initialContext, node)
      <3>1. [](IndexedHistoricalCertificateExactCommandLocalImportAt(
                 initialContext, node)
                => IndexedHistoricalCertificateCandidateEntryGoal(
                     initialContext, node))
        BY <2>0, IndexedHistoricalCertificateExactCommandExposesCandidateEntry,
           PTL, Isa
           DEF IndexedDecisionWitnessSupport,
               IndexedHistoricalCertificateExactCommandLocalImportAt
      <3> QED BY <3>1, PTL
    <2> QED BY <2>2
  <1> QED BY <1>1

THEOREM IndexedChainSpecClosesHistoricalCertificateReceivedQcLocalImportEntry ==
  IndexedChainSpec
    => IndexedHistoricalCertificateReceivedQcLocalImportEntryProperty
PROOF
  <1>1. ASSUME IndexedChainSpec
         PROVE IndexedHistoricalCertificateReceivedQcLocalImportEntryProperty
    <2>0. []IndexedDecisionWitnessSupport
      BY <1>1, IndexedChainSpecAlwaysDecisionWitnessSupport
    <2>1. []IndexedHistoricalCertificateLocalLineageInvariant
      BY <1>1, IndexedChainSpecAlwaysHistoricalCertificateLocalLineage
    <2>2. ASSUME NEW initialContext \in AdmissibleContextRecords,
                  NEW node \in Responsive
           PROVE IndexedHistoricalCertificateReceivedQcLocalImportAt(
                   initialContext, node)
                   ~> IndexedHistoricalCertificateCandidateEntryGoal(
                        initialContext, node)
      <3>1. [](IndexedHistoricalCertificateReceivedQcLocalImportAt(
                 initialContext, node)
                => IndexedHistoricalCertificateCandidateEntryGoal(
                     initialContext, node))
        BY <2>0, <2>1,
           IndexedHistoricalCertificateReceivedQcLineageExposesCandidateEntry,
           PTL, Isa
           DEF IndexedDecisionWitnessSupport,
               IndexedHistoricalCertificateLocalLineageInvariant,
               IndexedHistoricalCertificateLocalLineageInvariantAt,
               IndexedHistoricalCertificateReceivedQcLocalImportAt
      <3> QED BY <3>1, PTL
    <2> QED BY <2>2
         DEF IndexedHistoricalCertificateReceivedQcLocalImportEntryProperty
  <1> QED BY <1>1

THEOREM IndexedChainSpecClosesHistoricalCertificateDecisionWalLocalImportEntry ==
  IndexedChainSpec
    => IndexedHistoricalCertificateDecisionWalLocalImportEntryProperty
PROOF
  <1>1. ASSUME IndexedChainSpec
         PROVE IndexedHistoricalCertificateDecisionWalLocalImportEntryProperty
    <2>0. []IndexedDecisionWitnessSupport
      BY <1>1, IndexedChainSpecAlwaysDecisionWitnessSupport
    <2>1. []IndexedHistoricalCertificateLocalLineageInvariant
      BY <1>1, IndexedChainSpecAlwaysHistoricalCertificateLocalLineage
    <2>2. ASSUME NEW initialContext \in AdmissibleContextRecords,
                  NEW node \in Responsive
           PROVE IndexedHistoricalCertificateDecisionWalLocalImportAt(
                   initialContext, node)
                   ~> IndexedHistoricalCertificateCandidateEntryGoal(
                        initialContext, node)
      <3>1. [](IndexedHistoricalCertificateDecisionWalLocalImportAt(
                 initialContext, node)
                => IndexedHistoricalCertificateCandidateEntryGoal(
                     initialContext, node))
        BY <2>0, <2>1,
           IndexedHistoricalCertificateDecisionWalLineageExposesCandidateEntry,
           PTL, Isa
           DEF IndexedDecisionWitnessSupport,
               IndexedHistoricalCertificateLocalLineageInvariant,
               IndexedHistoricalCertificateLocalLineageInvariantAt,
               IndexedHistoricalCertificateDecisionWalLocalImportAt
      <3> QED BY <3>1, PTL
    <2> QED BY <2>2
         DEF IndexedHistoricalCertificateDecisionWalLocalImportEntryProperty
  <1> QED BY <1>1

THEOREM IndexedChainSpecClosesHistoricalCertificateLocalImportCandidateEntry ==
  IndexedChainSpec
    => IndexedHistoricalCertificateLocalImportCandidateEntryProperty
PROOF
  <1>1. ASSUME IndexedChainSpec
         PROVE IndexedHistoricalCertificateLocalImportCandidateEntryProperty
    <2>0. IndexedHistoricalCertificateReceivedQcLocalImportEntryProperty
      BY <1>1,
         IndexedChainSpecClosesHistoricalCertificateReceivedQcLocalImportEntry
    <2>1. IndexedHistoricalCertificateDecisionWalLocalImportEntryProperty
      BY <1>1,
         IndexedChainSpecClosesHistoricalCertificateDecisionWalLocalImportEntry
    <2>2. \A initialContext \in AdmissibleContextRecords,
                node \in Responsive:
              IndexedHistoricalCertificateExactCommandLocalImportAt(
                initialContext, node)
                ~> IndexedHistoricalCertificateCandidateEntryGoal(
                     initialContext, node)
      BY <1>1,
         IndexedChainSpecClosesHistoricalCertificateExactCommandLocalImport
    <2>3. ASSUME NEW initialContext \in AdmissibleContextRecords,
                  NEW node \in Responsive
           PROVE IndexedHistoricalCertificateLocalImportAt(
                   initialContext, node)
                   ~> IndexedHistoricalCertificateCandidateEntryGoal(
                        initialContext, node)
      <3>1. IndexedHistoricalCertificateReceivedQcLocalImportAt(
               initialContext, node)
               ~> IndexedHistoricalCertificateCandidateEntryGoal(
                    initialContext, node)
        BY <2>0
           DEF IndexedHistoricalCertificateReceivedQcLocalImportEntryProperty
      <3>2. IndexedHistoricalCertificateDecisionWalLocalImportAt(
               initialContext, node)
               ~> IndexedHistoricalCertificateCandidateEntryGoal(
                    initialContext, node)
        BY <2>1
           DEF IndexedHistoricalCertificateDecisionWalLocalImportEntryProperty
      <3>3. IndexedHistoricalCertificateExactCommandLocalImportAt(
               initialContext, node)
               ~> IndexedHistoricalCertificateCandidateEntryGoal(
                    initialContext, node)
        BY <2>2
      <3>4. [](IndexedHistoricalCertificateLocalImportAt(
                 initialContext, node)
                => \/ IndexedHistoricalCertificateReceivedQcLocalImportAt(
                         initialContext, node)
                   \/ IndexedHistoricalCertificateDecisionWalLocalImportAt(
                         initialContext, node)
                   \/ IndexedHistoricalCertificateExactCommandLocalImportAt(
                         initialContext, node))
        BY IndexedHistoricalCertificateLocalImportSplitsPhysicalSources, PTL
      <3> QED BY <3>1, <3>2, <3>3, <3>4, PTL
    <2> QED BY <2>3
         DEF IndexedHistoricalCertificateLocalImportCandidateEntryProperty
  <1> QED BY <1>1

THEOREM IndexedHistoricalCertificateLocalImportCandidateEntryClosesRankOne ==
  IndexedHistoricalCertificateLocalImportCandidateEntryProperty
    => IndexedHistoricalCertificateRankOneCandidateEntryProperty
PROOF
  <1>1. ASSUME IndexedHistoricalCertificateLocalImportCandidateEntryProperty
         PROVE IndexedHistoricalCertificateRankOneCandidateEntryProperty
    <2>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                  NEW node \in Responsive
           PROVE IndexedHistoricalCertificateStageAt(
                   initialContext, node, 1)
                   ~> IndexedHistoricalCertificateCandidateEntryGoal(
                        initialContext, node)
      <3>1. [](IndexedHistoricalCertificateStageAt(
                 initialContext, node, 1)
                => IndexedHistoricalCertificateLocalImportAt(
                     initialContext, node))
        BY IndexedHistoricalCertificateRankOneIsLocalImport, PTL
      <3>2. IndexedHistoricalCertificateLocalImportAt(
               initialContext, node)
               ~> IndexedHistoricalCertificateCandidateEntryGoal(
                    initialContext, node)
        BY <1>1
           DEF IndexedHistoricalCertificateLocalImportCandidateEntryProperty
      <3> QED BY <3>1, <3>2, PTL
    <2> QED BY <2>1
         DEF IndexedHistoricalCertificateRankOneCandidateEntryProperty
  <1> QED BY <1>1

THEOREM IndexedChainSpecClosesHistoricalCertificateRankOneEntry ==
  IndexedChainSpec
    => IndexedHistoricalCertificateRankOneCandidateEntryProperty
PROOF
  <1>1. ASSUME IndexedChainSpec
         PROVE IndexedHistoricalCertificateRankOneCandidateEntryProperty
    <2>1. IndexedHistoricalCertificateLocalImportCandidateEntryProperty
      BY <1>1,
         IndexedChainSpecClosesHistoricalCertificateLocalImportCandidateEntry
    <2> QED BY <2>1,
         IndexedHistoricalCertificateLocalImportCandidateEntryClosesRankOne
  <1> QED BY <1>1

IndexedHistoricalCertificateCandidateTailProgressProperty ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive,
     kind \in {"DeliverQC", "BeginDecision", "PersistDecision"}:
    IndexedHistoricalCertificateCandidateTailAt(
      initialContext, node, kind)
      ~> IndexedHistoricalCertificateCandidateTailGoal(
           initialContext, node, kind)

THEOREM IndexedChainSpecClosesHistoricalCertificateCandidateTail ==
  IndexedChainSpec
    => IndexedHistoricalCertificateCandidateTailProgressProperty
BY IndexedChainSpecClosesHistoricalDecisionCandidateProgressLeaves,
   IsaT(900), PTL
   DEF IndexedHistoricalCertificateCandidateTailProgressProperty,
       IndexedHistoricalCertificateCandidateTailAt,
       IndexedHistoricalCertificateCandidateTailGoal,
       IndexedHistoricalCertificateStageAt,
       IndexedHistoricalCertificateGoal,
       IndexedHistoricalDecisionOwned,
       IndexedHistoricalRecoveryRunnerOwned,
       IndexedHistoricalRecoveryTargetOwned,
       IndexedHistoricalDecisionCandidateProgressLeaves,
       IndexedHistoricalTransport!HistoricalCommitDeliveryProgressLeaf,
       IndexedHistoricalTransport!HistoricalBeginDecisionProgressLeaf,
       IndexedHistoricalTransport!HistoricalPersistDecisionProgressLeaf,
       IndexedHistoricalTransport!
         HistoricalProtectedCandidateStarvationProperty,
       IndexedHistoricalTransport!
         HistoricalCommitDecisionCandidateOwned

IndexedHistoricalCertificateRemainingCorridorProperty ==
  /\ IndexedHistoricalCertificateDiscoveryRunnerResidualProperty
  /\ IndexedHistoricalCertificateRequestServiceResidualProperty
  /\ IndexedHistoricalCertificateResponseImportResidualProperty

THEOREM IndexedHistoricalCertificateRemainingCorridorClosesRankResidual ==
  /\ IndexedChainSpec
  /\ IndexedHistoricalCertificateRemainingCorridorProperty
  => IndexedHistoricalCertificateRankProgressResidualProperty
PROOF
  <1>1. ASSUME IndexedChainSpec,
                IndexedHistoricalCertificateRemainingCorridorProperty
         PROVE IndexedHistoricalCertificateRankProgressResidualProperty
    <2>1. IndexedHistoricalCertificateCandidateTailProgressProperty
      BY <1>1,
         IndexedChainSpecClosesHistoricalCertificateCandidateTail
    <2>2. IndexedHistoricalCertificateRankOneCandidateEntryProperty
      BY <1>1,
         IndexedChainSpecClosesHistoricalCertificateRankOneEntry
    <2>3. IndexedHistoricalCertificateImportedDecisionResidualProperty
      BY <2>1, <2>2, PTL
         DEF IndexedHistoricalCertificateRankOneCandidateEntryProperty,
             IndexedHistoricalCertificateCandidateEntryGoal,
             IndexedHistoricalCertificateCandidateTailProgressProperty,
             IndexedHistoricalCertificateCandidateTailGoal,
             IndexedHistoricalCertificateRankProgressAt,
             SetLessThan
    <2> QED BY <1>1, <2>3
         DEF IndexedHistoricalCertificateRemainingCorridorProperty,
             IndexedHistoricalCertificateRankProgressResidualProperty
  <1> QED BY <1>1

(***************************************************************************
Exact certificate-rank residual surface.

The fixed-clock prerequisite compositor above is useful for local rank
composition, but it is not the release dependency boundary.  Fixed-kind Serve
service is a theorem of `IndexedChainSpec`; route-neutral Candidate service is
conditional on its explicit cross-instance starvation lift.  The boundary
below therefore names the remaining concrete packet-action service, that
neutral Candidate lift, and the exact Commit transport kernels through the
combined fixed-clock residual.
Fixed-clock non-packet service and the two target-local receipt/WAL entry
handoffs are theorems of `IndexedChainSpec`; archive Serve response is also
proved and is supplied by the transport-kernel reduction.
***************************************************************************)

IndexedHistoricalCertificatePhysicalResidualKernels ==
  /\ IndexedHistoricalFixedClockPacketRemainingTemporalResidual
  /\ IndexedHistoricalCommitTransportResidualKernelProperties

THEOREM IndexedHistoricalCertificatePhysicalKernelsCloseRankResidual ==
  /\ IndexedChainSpec
  /\ IndexedHistoricalCertificatePhysicalResidualKernels
  => IndexedHistoricalCertificateRankProgressResidualProperty
PROOF
  <1>1. ASSUME IndexedChainSpec,
               IndexedHistoricalCertificatePhysicalResidualKernels
         PROVE IndexedHistoricalCertificateRankProgressResidualProperty
    <2>0. IndexedHistoricalFixedClockPacketCorridorTemporalResidual
      BY <1>1,
         IndexedChainSpecAndRemainingPacketResidualProvideFixedClockPacketCorridor
         DEF IndexedHistoricalCertificatePhysicalResidualKernels
    <2>1. IndexedHistoricalCertificateDiscoveryRunnerResidualProperty
      BY <1>1, <2>0,
         IndexedHistoricalFixedClockExactResidualsCloseCertificateDiscoveryRank
         DEF IndexedHistoricalCertificatePhysicalResidualKernels
    <2>2. IndexedHistoricalCommitCertificateTransportLeafProperty
      BY <1>1, IndexedHistoricalCommitTransportKernelsCloseExactLeaf
         DEF IndexedHistoricalCertificatePhysicalResidualKernels
    <2>3. /\ IndexedHistoricalCertificateRequestServiceResidualProperty
           /\ IndexedHistoricalCertificateResponseImportResidualProperty
      BY <1>1, <2>2,
         IndexedHistoricalCommitTransportLeafClosesCertificateRanksTwoThree
    <2>4. IndexedHistoricalCertificateRemainingCorridorProperty
      BY <1>1, <2>1, <2>3
         DEF IndexedHistoricalCertificatePhysicalResidualKernels,
             IndexedHistoricalCertificateRemainingCorridorProperty
    <2> QED BY <1>1, <2>4,
         IndexedHistoricalCertificateRemainingCorridorClosesRankResidual
  <1> QED BY <1>1

THEOREM IndexedHistoricalDecisionTargetCertifiedRequestClosesTargetRank ==
  /\ IndexedChainSpec
  /\ IndexedHistoricalDecisionTargetCertifiedRequestResidualProperty
  => IndexedHistoricalDecisionTargetOwnerRankProgressProperty
BY IndexedChainSpecProvidesHistoricalFiniteRunnerEpisodeClosure,
   IndexedChainSpecClosesHistoricalDecisionTargetCandidateRankResiduals,
   IndexedHistoricalDecisionTargetRankResidualSplitsAtCertifiedRequest

THEOREM IndexedHistoricalDecisionPhysicalKernelsCloseTargetRank ==
  /\ IndexedChainSpec
  /\ IndexedHistoricalDecisionTransportResidualKernelProperties
  => IndexedHistoricalDecisionTargetOwnerRankProgressProperty
BY IndexedChainSpecProvidesHistoricalFiniteRunnerEpisodeClosure,
   IndexedHistoricalDecisionTransportKernelsCloseExactLeaf,
   IndexedHistoricalDecisionTransportLeafClosesTargetRankFive,
   IndexedHistoricalDecisionTargetCertifiedRequestClosesTargetRank

IndexedHistoricalApplicationReceiptHandoffProperty ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    IndexedHistoricalExactApplication(initialContext, node)
      ~> HistoricalRecoveryComplete(initialContext, node)

(***************************************************************************
The nonterminal receipt handoff is already closed.

At MaxHeight exact per-context application is the terminal definition.  Below
the horizon `IndexedApplicationsRespectNodeHeight`, maintained by
`IndexedCompositionInvariant`, says that the same product action which creates
the exact application receipt has already advanced `nodeHeight`.
***************************************************************************)

THEOREM IndexedHistoricalExactApplicationImpliesCompletion ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    /\ IndexedCompositionInvariant
    /\ IndexedHistoricalExactApplication(initialContext, node)
    => HistoricalRecoveryComplete(initialContext, node)
BY Isa
   DEF IndexedHistoricalExactApplication,
       HistoricalRecoveryComplete,
       IndexedCompositionInvariant,
       IndexedApplicationsRespectNodeHeight

THEOREM IndexedChainSpecClosesHistoricalApplicationReceiptHandoff ==
  IndexedChainSpec
    => IndexedHistoricalApplicationReceiptHandoffProperty
BY IndexedChainSpecEstablishesCompositionInvariant,
   IndexedHistoricalExactApplicationImpliesCompletion, PTL
   DEF IndexedHistoricalApplicationReceiptHandoffProperty

(***************************************************************************
Exact Open handoff.

`IndexedHistoricalRecoveryOpenable` includes the exact indexed GST bit, so it
is the whole production guard rather than a pre-GST promise that the guard
will later be reconstructed.  While none of application, Decision, or target
ownership has appeared, its fixed applied-archive witness is durable and the
target guard is stable.  The product enabledness bridge needs only the two
joined owners already named by that witness; it does not assume that every
responsive validator has joined the context.
***************************************************************************)

THEOREM IndexedHistoricalOpenResidualPersistsOrExits ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    /\ IndexedCompositionInvariant
    /\ IndexedHistoricalRecoveryOpenResidual(initialContext, node)
    /\ [IndexedChainNext]_IndexedChainVars
    => \/ IndexedHistoricalRecoveryOpenResidual(
             initialContext, node)'
       \/ IndexedHistoricalRecoveryOpenGoal(initialContext, node)'
BY IndexedStepPreservesCompositionInvariant,
   IndexedBracketStepKeepsNodeHeightsMonotone,
   IndexedNodeJoinIsStable, Isa
   DEF IndexedHistoricalRecoveryOpenResidual,
       IndexedHistoricalRecoveryOpenGoal,
       IndexedHistoricalRecoveryOpenable,
       IndexedHistoricalExactApplication,
       IndexedHistoricalDecisionOwned,
       IndexedHistoricalRecoveryRunnerOwned,
       IndexedHistoricalRecoveryTargetOwned,
       IndexedHistoricalRecoveryTargetReady,
       IndexedHistoricalRecoverySourceReady,
       HistoricalRecoveryOutstanding,
       ExactNodeLocationAt,
       IndexedCurrentDecisions, IndexedCurrentApplications,
       IndexedDecisionEvidence, IndexedApplicationEvidence,
       IndexedCompositionInvariant,
       IndexedEveryInstanceStrongInvariant,
       IndexedTotalReceiptProjection,
       IndexedDecisionReceiptProjection,
       IndexedApplicationReceiptProjection,
       IndexedChainNext, IndexedChainVars,
       IndexedProductActionAt, IndexedReceiptClassification,
       IndexedReceiptFreeChainStutter,
       IndexedDecisionReceiptHandoff,
       IndexedApplicationReceiptHandoff,
       NewIndexedDecisionReceipt, NewIndexedApplicationReceipt,
       NoNewIndexedDurableReceipt,
       IndexedAsync!StrongInductiveInvariant,
       IndexedAsync!Safety, IndexedAsync!TypeInvariant,
       IndexedAsync!AsyncHistoricalRecoveryTypeInvariant,
       IndexedAsync!NodeHasDecision,
       IndexedAsync!NodeHasApplication,
       IndexedAsync!HistoricalRecoveryTarget,
       IndexedAsync!AsyncCurrentResponsiveVoters,
       IndexedAsync!BodyHeldBy,
       IndexedAsync!AsyncNext,
       IndexedAsync!AsyncNonCrashStep,
       IndexedAsync!AsyncSetGST,
       IndexedAsync!PreGstCrash,
       IndexedAsync!PreGstResponsiveCrash,
       IndexedAsync!PreGstResponsiveRestart,
       IndexedAsync!PreGstResponsiveReplay,
       IndexedAsync!Crash,
       IndexedAsync!Restart,
       IndexedAsync!ApplyDecision,
       Chain!RecordCertifiedNext, Chain!RecordKnownDecision,
       Chain!RecordAppliedNext, Chain!RecordKnownApplication

THEOREM IndexedHistoricalOpenResidualEnablesExactOpen ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    /\ IndexedCompositionInvariant
    /\ IndexedHistoricalRecoveryOpenResidual(initialContext, node)
    => ENABLED
         <<IndexedOpenHistoricalRecoveryStep(
             initialContext, node)>>_(IndexedChainVars)
BY IndexedFairActionsRemainEnabledInProduct,
   ExpandENABLED, Isa
   DEF IndexedHistoricalRecoveryOpenResidual,
       IndexedHistoricalRecoveryOpenable,
       IndexedHistoricalRecoveryTargetReady,
       IndexedHistoricalRecoverySourceReady,
       IndexedOpenHistoricalRecoveryStep,
       IndexedOpenHistoricalRecovery,
       IndexedChainNext, IndexedChainVars,
       IndexedAsync!PostGstOpenHistoricalRecovery,
       IndexedAsync!OpenHistoricalRecovery,
       IndexedAsync!HistoricalRecoverySourceReady,
       IndexedAsync!HistoricalRecoveryTarget,
       IndexedAsync!AsyncResponsiveAppliedArchiveServers,
       IndexedAsync!AsyncResponsiveOnlineArchiveServers,
       IndexedAsync!AsyncResponsiveArchiveServers,
       IndexedAsync!NodeHasDecision,
       IndexedAsync!NodeHasApplication,
       IndexedAsync!AsyncNonRunnerOuterFrame,
       IndexedAsync!AsyncNonCrashOuterFrame,
       IndexedAsync!AsyncNonClockVars,
       IndexedAsync!AsyncSchedulerExceptHistoricalRecoveryTargets,
       IndexedAsync!AsyncAllVars,
       IndexedAsync!AsyncSchedulerVars,
       IndexedAsync!AsyncRecoveryVars,
       IndexedAsync!AsyncProducerVars,
       IndexedAsync!vars,
       IndexedProducer

THEOREM IndexedHistoricalOpenStepCreatesExactTarget ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    /\ IndexedHistoricalRecoveryOpenResidual(initialContext, node)
    /\ IndexedOpenHistoricalRecoveryStep(initialContext, node)
    => IndexedHistoricalRecoveryTargetOwned(initialContext, node)'
BY Isa
   DEF IndexedHistoricalRecoveryOpenResidual,
       IndexedHistoricalRecoveryOpenable,
       IndexedHistoricalRecoveryTargetOwned,
       IndexedOpenHistoricalRecoveryStep,
       IndexedOpenHistoricalRecovery,
       IndexedHistoricalRecoveryTargetReady,
       IndexedProductActionAt, IndexedChainNext,
       IndexedJoinedAsyncNext,
       IndexedAsync!OpenHistoricalRecovery,
       IndexedAsync!HistoricalRecoveryTarget,
       IndexedAsync!AsyncSchedulerExceptHistoricalRecoveryTargets,
       HistoricalRecoveryOutstanding,
       ExactNodeLocationAt, IndexedChainVars

THEOREM IndexedChainSpecClosesHistoricalOpenTarget ==
  IndexedChainSpec
    => IndexedHistoricalRecoveryOpenTargetResidualProperty
PROOF
  <1>1. ASSUME IndexedChainSpec,
              NEW initialContext \in AdmissibleContextRecords,
              NEW node \in Responsive
         PROVE IndexedHistoricalRecoveryOpenResidual(
                 initialContext, node)
                 ~> IndexedHistoricalRecoveryOpenGoal(
                      initialContext, node)
    <2>1. []IndexedCompositionInvariant
      BY <1>1, IndexedChainSpecEstablishesCompositionInvariant
    <2>2. [][IndexedChainNext]_IndexedChainVars
      BY <1>1 DEF IndexedChainSpec
    <2>3. WF_IndexedChainVars(
             IndexedOpenHistoricalRecoveryStep(
               initialContext, node))
      BY <1>1 DEF IndexedChainSpec, IndexedFairness
    <2>4. IndexedCompositionInvariant
             /\ IndexedHistoricalRecoveryOpenResidual(
                  initialContext, node)
             /\ [IndexedChainNext]_IndexedChainVars
             => \/ IndexedHistoricalRecoveryOpenResidual(
                      initialContext, node)'
                \/ IndexedHistoricalRecoveryOpenGoal(
                     initialContext, node)'
      BY IndexedHistoricalOpenResidualPersistsOrExits
    <2>5. IndexedCompositionInvariant
             /\ IndexedHistoricalRecoveryOpenResidual(
                  initialContext, node)
             => ENABLED
                  <<IndexedOpenHistoricalRecoveryStep(
                      initialContext, node)>>_(IndexedChainVars)
      BY IndexedHistoricalOpenResidualEnablesExactOpen
    <2>6. IndexedHistoricalRecoveryOpenResidual(
             initialContext, node)
             /\ <<IndexedOpenHistoricalRecoveryStep(
                    initialContext, node)>>_(IndexedChainVars)
             => IndexedHistoricalRecoveryOpenGoal(
                  initialContext, node)'
      BY IndexedHistoricalOpenStepCreatesExactTarget
         DEF IndexedHistoricalRecoveryOpenGoal
    <2> QED BY <2>1, <2>2, <2>3, <2>4, <2>5, <2>6, PTL
  <1> QED BY <1>1
       DEF IndexedHistoricalRecoveryOpenTargetResidualProperty

(***************************************************************************
Well-founded rank reductions.
***************************************************************************)

THEOREM IndexedHistoricalCertificateRankConvergence ==
  IndexedHistoricalCertificateRankProgressResidualProperty
    => \A initialContext \in AdmissibleContextRecords,
          node \in Responsive, rank \in Nat:
         IndexedHistoricalCertificateStageAt(
           initialContext, node, rank)
           ~> IndexedHistoricalCertificateGoal(
                initialContext, node)
PROOF
  <1>1. ASSUME IndexedHistoricalCertificateRankProgressResidualProperty,
                NEW initialContext \in AdmissibleContextRecords,
                NEW node \in Responsive
         PROVE \A rank \in Nat:
                 IndexedHistoricalCertificateStageAt(
                   initialContext, node, rank)
                   ~> IndexedHistoricalCertificateGoal(
                        initialContext, node)
    <2>1. \A rank \in Nat:
             IndexedHistoricalCertificateStageAt(
               initialContext, node, rank)
               ~> (IndexedHistoricalCertificateGoal(
                     initialContext, node)
                    \/ \E lower \in SetLessThan(
                         rank, OpToRel(<, Nat), Nat):
                         IndexedHistoricalCertificateStageAt(
                           initialContext, node, lower))
      BY <1>1
         DEF IndexedHistoricalCertificateRankProgressResidualProperty,
             IndexedHistoricalCertificateDiscoveryRunnerResidualProperty,
             IndexedHistoricalCertificateRequestServiceResidualProperty,
             IndexedHistoricalCertificateResponseImportResidualProperty,
             IndexedHistoricalCertificateImportedDecisionResidualProperty,
             IndexedHistoricalCertificateRankProgressAt,
             IndexedHistoricalCertificateStageAt
    <2> QED BY <2>1, NatLessThanWellFounded, WellFoundedLeadsTo
  <1> QED BY <1>1

THEOREM IndexedHistoricalDecisionRankConvergence ==
  IndexedHistoricalDecisionRankProgressResidualProperty
    => \A initialContext \in AdmissibleContextRecords,
          node \in Responsive, rank \in Nat:
         IndexedHistoricalDecisionStageAt(
           initialContext, node, rank)
           ~> IndexedHistoricalExactApplication(
                initialContext, node)
PROOF
  <1>1. ASSUME IndexedHistoricalDecisionRankProgressResidualProperty,
                NEW initialContext \in AdmissibleContextRecords,
                NEW node \in Responsive
         PROVE \A rank \in Nat:
                 IndexedHistoricalDecisionStageAt(
                   initialContext, node, rank)
                   ~> IndexedHistoricalExactApplication(
                        initialContext, node)
    <2>1. \A rank \in Nat:
             IndexedHistoricalDecisionStageAt(
               initialContext, node, rank)
               ~> (IndexedHistoricalExactApplication(
                     initialContext, node)
                    \/ \E lower \in SetLessThan(
                         rank, OpToRel(<, Nat), Nat):
                         IndexedHistoricalDecisionStageAt(
                           initialContext, node, lower))
      BY <1>1
         DEF IndexedHistoricalDecisionRankProgressResidualProperty,
             IndexedHistoricalDecisionFetchBodyResidualProperty,
             IndexedHistoricalDecisionCertifiedRequestResidualProperty,
             IndexedHistoricalDecisionFetchCertifiedBodyResidualProperty,
             IndexedHistoricalDecisionStoreBodyResidualProperty,
             IndexedHistoricalDecisionValidateBodyResidualProperty,
             IndexedHistoricalDecisionApplyResidualProperty,
             IndexedHistoricalDecisionRankProgressAt,
             IndexedHistoricalDecisionStageAt
    <2> QED BY <2>1, NatLessThanWellFounded, WellFoundedLeadsTo
  <1> QED BY <1>1

THEOREM IndexedHistoricalDecisionRankConvergenceAtContext ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedHistoricalDecisionRankProgressAtContext(initialContext)
      => \A node \in Responsive, rank \in Nat:
           IndexedHistoricalDecisionStageAt(
             initialContext, node, rank)
             ~> IndexedHistoricalExactApplication(
                  initialContext, node)
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
              IndexedHistoricalDecisionRankProgressAtContext(initialContext),
              NEW node \in Responsive
         PROVE \A rank \in Nat:
                 IndexedHistoricalDecisionStageAt(
                   initialContext, node, rank)
                   ~> IndexedHistoricalExactApplication(
                        initialContext, node)
    <2>1. \A rank \in Nat:
             IndexedHistoricalDecisionStageAt(
               initialContext, node, rank)
               ~> (IndexedHistoricalExactApplication(
                     initialContext, node)
                    \/ \E lower \in SetLessThan(
                         rank, OpToRel(<, Nat), Nat):
                         IndexedHistoricalDecisionStageAt(
                           initialContext, node, lower))
      BY <1>1
         DEF IndexedHistoricalDecisionRankProgressAtContext,
             IndexedHistoricalDecisionRankProgressAt,
             IndexedHistoricalDecisionStageAt
    <2> QED BY <2>1, NatLessThanWellFounded, WellFoundedLeadsTo
  <1> QED BY <1>1

(***************************************************************************
Historical-only service boundary.

This property starts after exact authority already exists.  It therefore
contains neither ordinary proposal/vote progress nor the first applied-archive
source acquisition.  A caller may establish either `Openable` or exact target
ownership, then use only the closed Open action and the certificate/body
service kernels below.
***************************************************************************)

IndexedHistoricalRecoveryAuthorityReady(initialContext, node) ==
  \/ IndexedHistoricalRecoveryOpenable(initialContext, node)
  \/ IndexedHistoricalRecoveryTargetOwned(initialContext, node)

IndexedExactHistoricalRecoveryFromAuthorityProgress ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    IndexedHistoricalRecoveryAuthorityReady(initialContext, node)
      ~> HistoricalRecoveryComplete(initialContext, node)

THEOREM IndexedHistoricalServiceKernelsDischargeAuthorityReadyProgress ==
  /\ IndexedChainSpec
  /\ IndexedHistoricalCertificateRankProgressResidualProperty
  /\ IndexedHistoricalDecisionRankProgressResidualProperty
  => IndexedExactHistoricalRecoveryFromAuthorityProgress
PROOF
  <1>1. ASSUME IndexedChainSpec,
              IndexedHistoricalCertificateRankProgressResidualProperty,
              IndexedHistoricalDecisionRankProgressResidualProperty,
              NEW initialContext \in AdmissibleContextRecords,
              NEW node \in Responsive
         PROVE IndexedHistoricalRecoveryAuthorityReady(
                 initialContext, node)
                 ~> HistoricalRecoveryComplete(
                      initialContext, node)
    <2>1. IndexedHistoricalRecoveryOpenTargetResidualProperty
      BY <1>1, IndexedChainSpecClosesHistoricalOpenTarget
    <2>2. IndexedHistoricalApplicationReceiptHandoffProperty
      BY <1>1, IndexedChainSpecClosesHistoricalApplicationReceiptHandoff
    <2>3. \A rank \in Nat:
             IndexedHistoricalCertificateStageAt(
               initialContext, node, rank)
               ~> IndexedHistoricalCertificateGoal(
                    initialContext, node)
      BY <1>1, IndexedHistoricalCertificateRankConvergence
    <2>4. \A rank \in Nat:
             IndexedHistoricalDecisionStageAt(
               initialContext, node, rank)
               ~> IndexedHistoricalExactApplication(
                    initialContext, node)
      BY <1>1, IndexedHistoricalDecisionRankConvergence
    <2>5. IndexedHistoricalRecoveryOpenable(initialContext, node)
             => (IndexedHistoricalRecoveryOpenResidual(
                   initialContext, node)
                  \/ IndexedHistoricalRecoveryOpenGoal(
                       initialContext, node))
      BY DEF IndexedHistoricalRecoveryOpenResidual,
             IndexedHistoricalRecoveryOpenGoal
    <2>6. IndexedHistoricalRecoveryOpenResidual(initialContext, node)
             ~> IndexedHistoricalRecoveryOpenGoal(
                  initialContext, node)
      BY <2>1
         DEF IndexedHistoricalRecoveryOpenTargetResidualProperty
    <2>7. IndexedHistoricalRecoveryTargetOwned(initialContext, node)
             => (IndexedHistoricalCertificateGoal(
                   initialContext, node)
                  \/ \E rank \in 1..4:
                       IndexedHistoricalCertificateStageAt(
                         initialContext, node, rank))
      BY IndexedHistoricalTargetHasExactCertificateStage
    <2>8. (\E rank \in 1..4:
             IndexedHistoricalCertificateStageAt(
               initialContext, node, rank))
             ~> IndexedHistoricalCertificateGoal(
                  initialContext, node)
      BY <2>3, PTL
    <2>9. IndexedHistoricalDecisionOwned(initialContext, node)
             => (IndexedHistoricalDecisionStageGoal(
                   initialContext, node)
                  \/ IndexedHistoricalDecisionStageOwnershipResidual(
                       initialContext, node))
      BY DEF IndexedHistoricalDecisionStageOwnershipResidual
    <2>10. IndexedHistoricalDecisionStageOwnershipResidual(
              initialContext, node)
              ~> IndexedHistoricalDecisionStageGoal(
                   initialContext, node)
      BY <1>1, IndexedHistoricalDecisionStageOwnershipResidualObligation
         DEF IndexedHistoricalDecisionStageOwnershipResidualProperty
    <2>11. (\E rank \in 1..6:
              IndexedHistoricalDecisionStageAt(
                initialContext, node, rank))
              ~> IndexedHistoricalExactApplication(
                   initialContext, node)
      BY <2>4, PTL
    <2>12. IndexedHistoricalDecisionStageGoal(initialContext, node)
              ~> IndexedHistoricalExactApplication(
                   initialContext, node)
      BY <2>11, PTL DEF IndexedHistoricalDecisionStageGoal
    <2>13. IndexedHistoricalCertificateGoal(initialContext, node)
              ~> IndexedHistoricalExactApplication(
                   initialContext, node)
      BY <2>9, <2>10, <2>12, PTL
         DEF IndexedHistoricalCertificateGoal
    <2>14. IndexedHistoricalRecoveryTargetOwned(initialContext, node)
              ~> IndexedHistoricalExactApplication(
                   initialContext, node)
      BY <2>7, <2>8, <2>13, PTL
    <2>15. IndexedHistoricalRecoveryOpenGoal(initialContext, node)
              ~> IndexedHistoricalExactApplication(
                   initialContext, node)
      BY <2>9, <2>10, <2>12, <2>14, PTL
         DEF IndexedHistoricalRecoveryOpenGoal
    <2>16. IndexedHistoricalRecoveryOpenable(initialContext, node)
              ~> IndexedHistoricalExactApplication(
                   initialContext, node)
      BY <2>5, <2>6, <2>15, PTL
    <2>17. IndexedHistoricalRecoveryAuthorityReady(
              initialContext, node)
              ~> IndexedHistoricalExactApplication(
                   initialContext, node)
      BY <2>14, <2>16, PTL
         DEF IndexedHistoricalRecoveryAuthorityReady
    <2>18. IndexedHistoricalExactApplication(initialContext, node)
              ~> HistoricalRecoveryComplete(
                   initialContext, node)
      BY <2>2
         DEF IndexedHistoricalApplicationReceiptHandoffProperty
    <2> QED BY <2>17, <2>18, PTL
  <1> QED BY <1>1
       DEF IndexedExactHistoricalRecoveryFromAuthorityProgress

(***************************************************************************
Exact local archive-authority bridge.

Historical recovery needs one immutable archive owner, not an aggregate
application at every responsive process.  The owner below is chosen once from
the frozen responsive voting roster.  That roster is finite and nonempty by
the dual-quorum configuration, so retries cannot replace the owner or enlarge
the authority universe.

Once the owner is joined, the indexed routing invariant gives the exact
dichotomy used by production:

  * if the owner is still current, the generic adequate-leader kernel reaches
    its durable Decision and the exact Decision service corridor applies it
    using that owner's local runner/packet/I/O fairness; or
  * if the owner is no longer current, advancing past this context already
    implies the exact application receipt.

The receipt projection and body-retention invariant then produce one typed
`IndexedHistoricalRecoverySourceReady` witness.  No all-responsive join,
aggregate Apply, application liveness, one-height closure, or height liveness
appears in this dependency cone.
***************************************************************************)

IndexedHistoricalRecoveryArchiveOwner(initialContext) ==
  CHOOSE server \in
    IndexedAsync(initialContext)!AsyncVotersAt(initialContext):
      TRUE

IndexedHistoricalRecoveryArchiveOwnerJoined(initialContext) ==
  IndexedHistoricalRecoveryArchiveOwner(initialContext)
    \in joinedByContext[initialContext]

IndexedHistoricalRecoveryArchiveOwnerApplied(initialContext) ==
  /\ IndexedCore(initialContext, 7)
  /\ IndexedHistoricalRecoveryArchiveOwner(initialContext)
       \in IndexedAsync(initialContext)!AsyncCurrentResponsiveVoters
  /\ IndexedAsync(initialContext)!NodeHasApplication(
       IndexedHistoricalRecoveryArchiveOwner(initialContext))

IndexedHistoricalRecoveryTypedArchiveAuthority(initialContext) ==
  /\ IndexedCore(initialContext, 7)
  /\ \E server \in ValidatorIds,
       source \in Chain!DecisionEvidenceSet:
       IndexedHistoricalRecoverySourceReady(
         initialContext, server, source)

IndexedLocalAdequateLeaderSemanticKernelProperty ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedAdequateLeaderWitness(initialContext)!
      AdequateLeaderLocalSemanticKernelProperty(IndexedChainSpec)

IndexedLocalAdequateLeaderFreshSelfCorridorExposureProperty ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedAdequateLeaderWitness(initialContext)!
      AdequateLeaderLocalFreshSelfCorridorExposureProperty(
        IndexedChainSpec)

IndexedLocalAdequateLeaderFreshSelfLeaderDecisionProperty ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedAdequateLeaderWitness(initialContext)!
      AdequateLeaderFreshSelfLeaderDecisionProperty(IndexedChainSpec)

IndexedLocalAdequateLeaderTargetProofInvariantsProperty ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedAdequateLeaderWitness(initialContext)!
      AdequateLeaderTargetProofInvariantsProperty(IndexedChainSpec)

IndexedLocalAdequateLeaderFixedDeadlineAndResponsiveDisseminationProperty ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedAdequateLeaderWitness(initialContext)!
      AdequateLeaderFixedDeadlineAndResponsiveDisseminationProperty(
        IndexedChainSpec)

IndexedLocalAdequateLeaderProducerTransportClosureProperty ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedAdequateLeaderWitness(initialContext)!
      AdequateLeaderTargetProducerTransportClosureProperty(
        IndexedChainSpec)

IndexedLocalAdequateLeaderProducerTransportOccurrenceClosureProperty ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedAdequateLeaderWitness(initialContext)!
      AdequateLeaderTargetProducerTransportOccurrenceClosureProperty(
        IndexedChainSpec)

IndexedLocalAdequateLeaderRetainedProducerNonDescentEpisodeStepProperty ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedAdequateLeaderWitness(initialContext)!
      AdequateLeaderRetainedProducerNonDescentEpisodeStepProperty(
        IndexedChainSpec)

IndexedLocalAdequateLeaderRetainedProducerOccurrenceClosureProperty ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedAdequateLeaderWitness(initialContext)!
      AdequateLeaderTargetRetainedProducerOccurrenceClosureProperty(
        IndexedChainSpec)

IndexedLocalAdequateLeaderProductiveEpisodeRankStepProperty ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedAdequateLeaderWitness(initialContext)!
      AdequateLeaderTargetProductiveEpisodeRankStepProperty(
        IndexedChainSpec)

IndexedAdequateLeaderLocalFairBehaviorAt(initialContext) ==
  /\ [][IndexedAdequateLeaderWitness(initialContext)!AsyncNext]_(
       IndexedAdequateLeaderWitness(initialContext)!AsyncAllVars)
  /\ WF_(IndexedAdequateLeaderWitness(initialContext)!AsyncAllVars)(
       IndexedCore(initialContext, 7)
         /\ IndexedAdequateLeaderWitness(initialContext)!AsyncTick)
  /\ \A node \in IndexedAdequateLeaderWitness(initialContext)!
                   AsyncVotersAt(initialContext):
       /\ WF_(IndexedAdequateLeaderWitness(initialContext)!AsyncAllVars)(
            IndexedAdequateLeaderWitness(initialContext)!
              PostGstRunNode(node))
       /\ WF_(IndexedAdequateLeaderWitness(initialContext)!AsyncAllVars)(
            IndexedAdequateLeaderWitness(initialContext)!
              PostGstResolveLocalCandidateProducerContinuation(node))
       /\ WF_(IndexedAdequateLeaderWitness(initialContext)!AsyncAllVars)(
            IndexedAdequateLeaderWitness(initialContext)!
              PostGstServiceConditionalTransportProducerContinuation(node))
       /\ WF_(IndexedAdequateLeaderWitness(initialContext)!AsyncAllVars)(
            IndexedAdequateLeaderWitness(initialContext)!
              PostGstServiceVolatileBodyProducerContinuation(node))
  /\ \A node \in Responsive:
       WF_(IndexedAdequateLeaderWitness(initialContext)!AsyncAllVars)(
         IndexedAdequateLeaderWitness(initialContext)!
           PostGstServiceIoWorker(node))
  /\ \A recipient \in Responsive,
        source \in IndexedAdequateLeaderWitness(initialContext)!
                   AsyncIngressSources:
       WF_(IndexedAdequateLeaderWitness(initialContext)!AsyncAllVars)(
         IndexedAdequateLeaderWitness(initialContext)!
           PostGstAdmitHiddenPacket(recipient, source))
  /\ \A slot \in IndexedAdequateLeaderWitness(initialContext)!
                  AsyncLeaderWireLifecycleSlotSet:
       WF_(IndexedAdequateLeaderWitness(initialContext)!AsyncAllVars)(
         IndexedAdequateLeaderWitness(initialContext)!
           PostGstRetireLeaderWireLifecycleSlot(slot))

IndexedLocalAdequateLeaderDecisionConvergenceProperty ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedAdequateLeaderWitness(initialContext)!
      AdequateLeaderLocalTargetDecisionConvergenceProperty(
        IndexedChainSpec)

IndexedLocalExactDecisionStageServiceProperty ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedDecisionServiceWitness(initialContext)!
      ExactDecisionStageServiceProperty(IndexedChainSpec)

THEOREM IndexedLiveChainSpecProvidesLocalAdequateLeaderProofInvariants ==
  IndexedLiveChainSpec
    => IndexedLocalAdequateLeaderTargetProofInvariantsProperty
PROOF
  <1>1. ASSUME IndexedLiveChainSpec
         PROVE IndexedLocalAdequateLeaderTargetProofInvariantsProperty
    <2>1. IndexedChainSpec
      BY <1>1, IndexedLiveChainSpecProjectsIndexedChainSpec
    <2>2. []IndexedCompositionInvariant
      BY <2>1, IndexedChainSpecEstablishesCompositionInvariant, PTL
    <2>3. \A initialContext \in AdmissibleContextRecords:
             [](IndexedAdequateLeaderWitness(initialContext)!
                  AsyncAllVars = IndexedAsyncStateAt(initialContext))
      BY <2>2, IndexedAdequateLeaderWitnessVariablesAreExact, PTL
         DEF IndexedCompositionInvariant
    <2>4. \A initialContext \in AdmissibleContextRecords:
             []( /\ IndexedAdequateLeaderWitness(initialContext)!
                        AsyncStrongTypeInvariant
                  /\ IndexedAdequateLeaderWitness(initialContext)!
                        AsyncProgressOwnershipInvariant
                  /\ IndexedAdequateLeaderWitness(initialContext)!
                        AsyncCandidateServiceTombstoneLifecycleInvariant)
      BY <2>2, <2>3, Isa, PTL
         DEF IndexedCompositionInvariant,
             IndexedEveryInstanceAsyncStrongTypeInvariant,
             IndexedAdequateLeaderWitness!AsyncStrongTypeInvariant
    <2> QED BY <2>1, <2>4
         DEF IndexedLocalAdequateLeaderTargetProofInvariantsProperty,
             IndexedAdequateLeaderWitness!
               AdequateLeaderTargetProofInvariantsProperty
  <1> QED BY <1>1

THEOREM IndexedLiveChainSpecProvidesAdequateLeaderLocalFairBehavior ==
  IndexedLiveChainSpec
    => \A initialContext \in AdmissibleContextRecords:
         IndexedAdequateLeaderLocalFairBehaviorAt(initialContext)
PROOF
  <1>1. ASSUME IndexedLiveChainSpec,
              NEW initialContext \in AdmissibleContextRecords
         PROVE IndexedAdequateLeaderLocalFairBehaviorAt(initialContext)
    <2>1. IndexedChainSpec
      BY <1>1, IndexedLiveChainSpecProjectsIndexedChainSpec
    <2>2. [][IndexedAdequateLeaderWitness(initialContext)!AsyncNext]_(
             IndexedAdequateLeaderWitness(initialContext)!AsyncAllVars)
      BY <2>1,
         IndexedBracketStepProjectsEveryAdequateLeaderWitnessStep, PTL
    <2>3. IndexedAdequateLeaderWitness(initialContext)!AsyncAllVars =
             IndexedAsyncStateAt(initialContext)
      BY <2>1, IndexedChainSpecEstablishesCompositionInvariant,
         IndexedAdequateLeaderWitnessVariablesAreExact, PTL
         DEF IndexedCompositionInvariant
    <2>4. WF_(IndexedAsyncStateAt(initialContext))(
             IndexedPostGstTick(initialContext))
      BY <2>1, IndexedPostGstTickFairnessTransfersLocally
    <2>5. \A node \in IndexedAsync(initialContext)!
                         AsyncVotersAt(initialContext):
             WF_(IndexedAsyncStateAt(initialContext))(
               IndexedAsync(initialContext)!PostGstRunNode(node))
      BY <2>1, IndexedPostGstRunNodeFairnessTransfersLocally
    <2>6. /\ \A node \in IndexedAsync(initialContext)!
                            AsyncVotersAt(initialContext):
                  /\ WF_(IndexedAsyncStateAt(initialContext))(
                       IndexedAsync(initialContext)!
                         PostGstResolveLocalCandidateProducerContinuation(
                           node))
                  /\ WF_(IndexedAsyncStateAt(initialContext))(
                       IndexedAsync(initialContext)!
                         PostGstServiceConditionalTransportProducerContinuation(
                           node))
                  /\ WF_(IndexedAsyncStateAt(initialContext))(
                       IndexedAsync(initialContext)!
                         PostGstServiceVolatileBodyProducerContinuation(node))
           /\ \A recipient \in Responsive,
                 source \in IndexedAsync(initialContext)!AsyncIngressSources:
                  WF_(IndexedAsyncStateAt(initialContext))(
                    IndexedAsync(initialContext)!
                      PostGstAdmitHiddenPacket(recipient, source))
           /\ \A slot \in IndexedAsync(initialContext)!
                           AsyncLeaderWireLifecycleSlotSet:
                WF_(IndexedAsyncStateAt(initialContext))(
                  IndexedAsync(initialContext)!
                    PostGstRetireLeaderWireLifecycleSlot(slot))
      BY <2>1, IndexedAdequateLeaderNonRunnerFairnessTransfersLocally
    <2>7. \A node \in Responsive:
             WF_(IndexedAsyncStateAt(initialContext))(
               IndexedAsync(initialContext)!PostGstServiceIoWorker(node))
      BY <2>1, IndexedHistoricalNonPacketOwnerFairnessTransfersLocally
    <2> QED BY <2>2, <2>3, <2>4, <2>5, <2>6, <2>7, Isa
         DEF IndexedAdequateLeaderLocalFairBehaviorAt,
             IndexedPostGstTick,
             IndexedAdequateLeaderWitness!AsyncAllVars,
             IndexedAdequateLeaderWitness!AsyncSchedulerVars,
             IndexedAdequateLeaderWitness!AsyncRecoveryVars,
             IndexedAdequateLeaderWitness!AsyncProducerVars,
             IndexedAdequateLeaderWitness!vars,
             IndexedAsyncStateAt, IndexedCore,
             IndexedScheduler, IndexedRecovery, IndexedProducer
  <1> QED BY <1>1

THEOREM IndexedAdequateLeaderLocalSourceJoinsResponsiveRoster ==
  \A initialContext \in AdmissibleContextRecords,
     target \in ValidatorIds:
    /\ IndexedCompositionInvariant
    /\ IndexedAdequateLeaderWitness(initialContext)!
         AdequateLeaderLocalTargetDecisionSource(target)
    => IndexedAllResponsiveJoined(initialContext)
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW target \in ValidatorIds,
                /\ IndexedCompositionInvariant
                /\ IndexedAdequateLeaderWitness(initialContext)!
                     AdequateLeaderLocalTargetDecisionSource(target)
         PROVE IndexedAllResponsiveJoined(initialContext)
    <2>1. IndexedCore(initialContext, 7)
      BY <1>1, Isa
         DEF IndexedAdequateLeaderWitness!
               AdequateLeaderLocalTargetDecisionSource
    <2>2. Responsive \subseteq
             IndexedAsync(initialContext)!AsyncActiveServiceNodes
      BY <1>1, <2>1
         DEF IndexedCompositionInvariant,
             IndexedPostGstResponsiveActiveRosterCoherence
    <2>3. Responsive \subseteq ValidatorIds
      BY <1>1, Isa
         DEF IndexedCompositionInvariant,
             IndexedEveryInstanceAsyncStrongTypeInvariant,
             IndexedAsync!AsyncStrongTypeInvariant,
             IndexedAsync!StrongInductiveInvariant,
             IndexedAsync!Safety, IndexedAsync!TypeInvariant,
             IndexedAsync!ModelConfiguration,
             IndexedAsync!QuorumConfiguration
    <2>4. \A node \in Responsive:
             node \in joinedByContext[initialContext]
      BY <1>1, <2>1, <2>2, <2>3,
         IndexedPostGstActiveServiceOwnerHasJoinedProductInstance, Isa
    <2> QED BY <2>4 DEF IndexedAllResponsiveJoined
  <1> QED BY <1>1

(***************************************************************************
Fresh-self exposure is source-conditioned, not an all-context activation
assumption.  If a local target source never occurs, its leads-to claim is
vacuous.  Otherwise GST plus the product coherence invariant proves that the
complete responsive roster has joined this exact instance; monotone joining
then supplies the existing local AsyncLive activation theorem.  The adequate-
leader witness has the identical state substitution, so its standalone
well-founded view-exposure proof applies without any authority carry or bundle.
***************************************************************************)
THEOREM IndexedLiveChainSpecProvidesLocalAdequateLeaderFreshSelfCorridorExposure ==
  IndexedLiveChainSpec
    => IndexedLocalAdequateLeaderFreshSelfCorridorExposureProperty
PROOF
  <1>1. ASSUME IndexedLiveChainSpec
         PROVE IndexedLocalAdequateLeaderFreshSelfCorridorExposureProperty
    <2>1. IndexedChainSpec
      BY <1>1, IndexedLiveChainSpecProjectsIndexedChainSpec
    <2>2. []IndexedCompositionInvariant
      BY <2>1, IndexedChainSpecEstablishesCompositionInvariant, PTL
    <2>3. ASSUME NEW initialContext \in AdmissibleContextRecords
           PROVE IndexedAdequateLeaderWitness(initialContext)!
                   AdequateLeaderLocalFreshSelfCorridorExposureProperty(
                     IndexedChainSpec)
      <3>1. ASSUME NEW target \in ValidatorIds
             PROVE IndexedAdequateLeaderWitness(initialContext)!
                     AdequateLeaderLocalTargetDecisionSource(target)
                       ~> IndexedAdequateLeaderWitness(initialContext)!
                            AdequateLeaderTargetFreshSelfCorridorGoal(target)
        <4>1. [](IndexedAdequateLeaderWitness(initialContext)!
                    AdequateLeaderLocalTargetDecisionSource(target)
                   => IndexedAllResponsiveJoined(initialContext))
          BY <2>2,
             IndexedAdequateLeaderLocalSourceJoinsResponsiveRoster, PTL
        <4>2. IndexedAllResponsiveJoined(initialContext)
                 /\ [IndexedChainNext]_IndexedChainVars
                 => IndexedAllResponsiveJoined(initialContext)'
          BY <2>3, IndexedAllResponsiveJoinedIsStable
        <4>3. CASE <>IndexedAdequateLeaderWitness(initialContext)!
                         AdequateLeaderLocalTargetDecisionSource(target)
          <5>1. <>IndexedAllResponsiveJoined(initialContext)
            BY <4>1, <4>3, PTL
          <5>2. TRUE ~> IndexedAllResponsiveJoined(initialContext)
            BY <2>1, <4>2, <5>1, PTL DEF IndexedChainSpec
          <5>3. IndexedAsync(initialContext)!
                   AsyncLiveSpecAt(initialContext)
            BY <1>1, <2>3, <5>2,
               IndexedLiveInstanceActivationObligation
          <5>4. IndexedAdequateLeaderWitness(initialContext)!
                   AsyncLiveSpecAt(initialContext)
            BY <2>3, <5>3,
               IndexedAsyncLiveSpecProjectsAdequateLeaderWitnessLiveSpec
          <5>5. IndexedAdequateLeaderWitness(initialContext)!
                   AdequateLeaderLocalFreshSelfCorridorExposureProperty(
                     IndexedAdequateLeaderWitness(initialContext)!
                       AsyncLiveSpecAt(initialContext))
            BY IndexedAdequateLeaderWitness(initialContext)!
                 AsyncLiveProvidesLocalFreshSelfCorridorExposure
          <5> QED BY <2>3, <5>4, <5>5
               DEF IndexedAdequateLeaderWitness!
                     AdequateLeaderLocalFreshSelfCorridorExposureProperty
        <4>4. CASE ~<>IndexedAdequateLeaderWitness(initialContext)!
                         AdequateLeaderLocalTargetDecisionSource(target)
          BY <4>4, PTL
        <4> QED BY <4>3, <4>4
      <3> QED BY <2>1, <3>1
           DEF IndexedAdequateLeaderWitness!
                 AdequateLeaderLocalFreshSelfCorridorExposureProperty
    <2> QED BY <2>3
         DEF IndexedLocalAdequateLeaderFreshSelfCorridorExposureProperty
  <1> QED BY <1>1

(***************************************************************************
Indexed quantitative fixed-deadline lift.

The adequate-leader witness is the exact indexed Async state substitution,
not a second behavior.  Bracketed product steps project to its `AsyncNext`,
the composition invariant supplies the concrete ownership/tombstone facts,
and `IndexedAdequateLeaderLocalFairBehaviorAt` supplies precisely the Tick,
runner, I/O, ingress, retirement, and producer actions selected by the fixed
rank.  A fixed-deadline source which is not already decided is also a local
active-target source, hence all Responsive owners are joined by the existing
source/activation bridge.  The same argument is applied source-by-source to
protected starvation and Decision dissemination; an inactive instance owns
neither source, so its leads-to clause is vacuous.  No indexed height or
historical-recovery conclusion is used here.
***************************************************************************)
THEOREM IndexedAdequateLeaderFixedDeadlineSourceJoinsResponsiveRoster ==
  \A initialContext \in AdmissibleContextRecords,
     target \in ValidatorIds,
     leaderContext \in ContextRecords,
     leaderView \in Views,
     startTime, deadline \in Nat:
    /\ IndexedCompositionInvariant
    /\ IndexedAdequateLeaderWitness(initialContext)!
         AdequateLeaderFixedCorridorDeadlineSource(
           target, leaderContext, leaderView, startTime, deadline)
    /\ ~IndexedAdequateLeaderWitness(initialContext)!NodeHasDecision(target)
      => IndexedAllResponsiveJoined(initialContext)
BY IndexedAdequateLeaderLocalSourceJoinsResponsiveRoster, Isa
   DEF IndexedAdequateLeaderWitness!
         AdequateLeaderFixedCorridorDeadlineSource,
       IndexedAdequateLeaderWitness!
         AdequateLeaderFreshSynchronizedTargetCorridor,
       IndexedAdequateLeaderWitness!
         AdequateLeaderLocalTargetDecisionSource

THEOREM
    IndexedLiveChainSpecProvidesLocalAdequateLeaderFixedDeadlineAndResponsiveDissemination ==
  IndexedLiveChainSpec
    => IndexedLocalAdequateLeaderFixedDeadlineAndResponsiveDisseminationProperty
PROOF
  <1>1. ASSUME IndexedLiveChainSpec
         PROVE
           IndexedLocalAdequateLeaderFixedDeadlineAndResponsiveDisseminationProperty
    <2>1. IndexedChainSpec
      BY <1>1, IndexedLiveChainSpecProjectsIndexedChainSpec
    <2>2. []IndexedCompositionInvariant
      BY <2>1, IndexedChainSpecEstablishesCompositionInvariant, PTL
    <2>3. \A initialContext \in AdmissibleContextRecords:
             [](IndexedAdequateLeaderWitness(initialContext)!
                  AsyncAllVars = IndexedAsyncStateAt(initialContext))
      BY <2>2, IndexedAdequateLeaderWitnessVariablesAreExact, PTL
         DEF IndexedCompositionInvariant
    <2>4. \A initialContext \in AdmissibleContextRecords:
             IndexedAdequateLeaderLocalFairBehaviorAt(initialContext)
      BY <1>1,
         IndexedLiveChainSpecProvidesAdequateLeaderLocalFairBehavior
    <2>5. ASSUME NEW initialContext \in AdmissibleContextRecords
           PROVE IndexedAdequateLeaderWitness(initialContext)!
                   AdequateLeaderFixedDeadlineAndResponsiveDisseminationProperty(
                     IndexedChainSpec)
      <3>1. initialContext
               \in IndexedAdequateLeaderWitness(initialContext)!ContextRecords
        BY <2>2, Isa
           DEF IndexedCompositionInvariant,
               IndexedEveryInstanceAsyncStrongTypeInvariant,
               IndexedAdequateLeaderWitness!AsyncStrongTypeInvariant,
               IndexedAdequateLeaderWitness!StrongInductiveInvariant,
               IndexedAdequateLeaderWitness!Safety,
               IndexedAdequateLeaderWitness!TypeInvariant,
               IndexedAdequateLeaderWitness!ModelConfiguration,
               AdmissibleContextRecords, FrozenContextAdmissible,
               ContextRecords
      <3>2. IndexedAdequateLeaderLocalFairBehaviorAt(initialContext)
        BY <2>4
      <3>3. [](IndexedAdequateLeaderWitness(initialContext)!
                  AsyncAllVars = IndexedAsyncStateAt(initialContext))
        BY <2>3
      <3> QED
        BY <1>1, <2>1, <2>2, <3>1, <3>2, <3>3,
           IndexedLiveChainSpecProvidesLocalAdequateLeaderProofInvariants,
           IndexedAdequateLeaderLocalSourceJoinsResponsiveRoster,
           IndexedAdequateLeaderFixedDeadlineSourceJoinsResponsiveRoster,
           IndexedAllResponsiveJoinedIsStable,
           IndexedLiveInstanceActivationObligation,
           IndexedAsyncLiveSpecProjectsAdequateLeaderWitnessLiveSpec,
           IndexedAdequateLeaderWitness(initialContext)!
             AsyncLiveSpecSuppliesAdequateLeaderFixedDeadlineAndResponsiveDissemination,
           PTL, Isa
           DEF IndexedAdequateLeaderLocalFairBehaviorAt,
               IndexedAdequateLeaderWitness!
                 AdequateLeaderFixedDeadlineAndResponsiveDisseminationProperty,
               IndexedAdequateLeaderWitness!
                 AdequateLeaderFixedCorridorDeadlineServiceProperty,
               IndexedAdequateLeaderWitness!
                 AdequateLeaderFixedCorridorDeadlineSource,
               IndexedAdequateLeaderWitness!
                 AdequateLeaderFreshSynchronizedTargetCorridor,
               IndexedAdequateLeaderWitness!
                 AdequateLeaderLocalTargetDecisionSource,
               IndexedAdequateLeaderWitness!StarvationFreedomProperty,
               IndexedAdequateLeaderWitness!
                 AdequateLeaderResponsiveDecisionDisseminationProperty,
               IndexedAdequateLeaderWitness!AsyncLiveSpecAt,
               IndexedAdequateLeaderWitness!AsyncSpecAt,
               IndexedAdequateLeaderWitness!AsyncFairnessAt,
               IndexedAsyncStateAt
    <2> QED BY <2>5
         DEF
           IndexedLocalAdequateLeaderFixedDeadlineAndResponsiveDisseminationProperty
  <1> QED BY <1>1

THEOREM IndexedAdequateLeaderRetainedProducerStepAndOccurrenceClosureCompose ==
  /\ IndexedLocalAdequateLeaderProducerTransportOccurrenceClosureProperty
  /\ IndexedLocalAdequateLeaderRetainedProducerNonDescentEpisodeStepProperty
    => IndexedLocalAdequateLeaderRetainedProducerOccurrenceClosureProperty
PROOF
  <1>1. ASSUME
          /\ IndexedLocalAdequateLeaderProducerTransportOccurrenceClosureProperty
          /\ IndexedLocalAdequateLeaderRetainedProducerNonDescentEpisodeStepProperty,
        NEW initialContext \in AdmissibleContextRecords
         PROVE IndexedAdequateLeaderWitness(initialContext)!
                 AdequateLeaderTargetRetainedProducerOccurrenceClosureProperty(
                   IndexedChainSpec)
    <2> QED BY <1>1,
         IndexedAdequateLeaderWitness(initialContext)!
           AdequateLeaderRetainedProducerStepAndOccurrenceClosureCompose
         DEF IndexedLocalAdequateLeaderProducerTransportOccurrenceClosureProperty,
             IndexedLocalAdequateLeaderRetainedProducerNonDescentEpisodeStepProperty
  <1> QED BY <1>1
       DEF IndexedLocalAdequateLeaderRetainedProducerOccurrenceClosureProperty

THEOREM IndexedAdequateLeaderCompletedProvidersSupplyLocalSemanticKernel ==
  /\ IndexedLiveChainSpec
  /\ IndexedLocalAdequateLeaderFreshSelfCorridorExposureProperty
  /\ IndexedLocalAdequateLeaderFreshSelfLeaderDecisionProperty
  /\ IndexedLocalAdequateLeaderProducerTransportClosureProperty
  /\ IndexedLocalAdequateLeaderProducerTransportOccurrenceClosureProperty
  /\ IndexedLocalAdequateLeaderRetainedProducerNonDescentEpisodeStepProperty
  /\ IndexedLocalAdequateLeaderProductiveEpisodeRankStepProperty
    => IndexedLocalAdequateLeaderSemanticKernelProperty
PROOF
  <1>1. ASSUME /\ IndexedLiveChainSpec
                /\ IndexedLocalAdequateLeaderFreshSelfCorridorExposureProperty
                /\ IndexedLocalAdequateLeaderFreshSelfLeaderDecisionProperty
                /\ IndexedLocalAdequateLeaderProducerTransportClosureProperty
                /\ IndexedLocalAdequateLeaderProducerTransportOccurrenceClosureProperty
                /\ IndexedLocalAdequateLeaderRetainedProducerNonDescentEpisodeStepProperty
                /\ IndexedLocalAdequateLeaderProductiveEpisodeRankStepProperty,
              NEW initialContext \in AdmissibleContextRecords
         PROVE IndexedAdequateLeaderWitness(initialContext)!
                 AdequateLeaderLocalSemanticKernelProperty(IndexedChainSpec)
    <2>1. IndexedChainSpec
      BY <1>1, IndexedLiveChainSpecProjectsIndexedChainSpec
    <2>2. IndexedAdequateLeaderWitness(initialContext)!
             AdequateLeaderTargetProofInvariantsProperty(IndexedChainSpec)
      BY <1>1,
         IndexedLiveChainSpecProvidesLocalAdequateLeaderProofInvariants
         DEF IndexedLocalAdequateLeaderTargetProofInvariantsProperty
    <2>3. IndexedChainSpec
             => []IndexedAdequateLeaderWitness(initialContext)!
                    AsyncStrongTypeInvariant
      BY <2>2
         DEF IndexedAdequateLeaderWitness!
               AdequateLeaderTargetProofInvariantsProperty
    <2>4. IndexedAdequateLeaderWitness(initialContext)!
             AdequateLeaderTargetRetainedProducerOccurrenceClosureProperty(
               IndexedChainSpec)
      BY <1>1,
         IndexedAdequateLeaderRetainedProducerStepAndOccurrenceClosureCompose
         DEF IndexedLocalAdequateLeaderRetainedProducerOccurrenceClosureProperty
    <2> QED BY <1>1, <2>2, <2>3, <2>4,
         IndexedAdequateLeaderWitness(initialContext)!
           AdequateLeaderCompletedLocalProviderKernelSuppliesSemanticKernel
         DEF IndexedLocalAdequateLeaderFreshSelfCorridorExposureProperty,
             IndexedLocalAdequateLeaderFreshSelfLeaderDecisionProperty,
             IndexedLocalAdequateLeaderProducerTransportClosureProperty,
             IndexedLocalAdequateLeaderRetainedProducerOccurrenceClosureProperty,
             IndexedLocalAdequateLeaderProductiveEpisodeRankStepProperty,
             IndexedAdequateLeaderWitness!
               AdequateLeaderCompletedLocalProviderKernelProperty
  <1> QED BY <1>1
       DEF IndexedLocalAdequateLeaderSemanticKernelProperty

THEOREM IndexedAdequateLeaderFixedDeadlineDisseminationAndExposureSupplyLocalConvergence ==
  /\ IndexedLocalAdequateLeaderFixedDeadlineAndResponsiveDisseminationProperty
  /\ IndexedLocalAdequateLeaderFreshSelfCorridorExposureProperty
    => IndexedLocalAdequateLeaderDecisionConvergenceProperty
PROOF
  <1>1. ASSUME
          /\ IndexedLocalAdequateLeaderFixedDeadlineAndResponsiveDisseminationProperty
          /\ IndexedLocalAdequateLeaderFreshSelfCorridorExposureProperty,
        NEW initialContext \in AdmissibleContextRecords
         PROVE IndexedAdequateLeaderWitness(initialContext)!
                 AdequateLeaderLocalTargetDecisionConvergenceProperty(
                   IndexedChainSpec)
    <2> QED BY <1>1,
         IndexedAdequateLeaderWitness(initialContext)!
           AdequateLeaderFixedDeadlineAndDisseminationSupplyLocalTargetConvergence
         DEF IndexedLocalAdequateLeaderFixedDeadlineAndResponsiveDisseminationProperty,
             IndexedLocalAdequateLeaderFreshSelfCorridorExposureProperty,
             IndexedAdequateLeaderWitness!
               AdequateLeaderFixedDeadlineAndResponsiveDisseminationProperty
  <1> QED BY <1>1
       DEF IndexedLocalAdequateLeaderDecisionConvergenceProperty

THEOREM IndexedHistoricalRecoveryArchiveOwnerIsFrozenResponsiveVoter ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedHistoricalRecoveryArchiveOwner(initialContext)
      \in IndexedAsync(initialContext)!AsyncVotersAt(initialContext)
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords
         PROVE IndexedHistoricalRecoveryArchiveOwner(initialContext)
                 \in IndexedAsync(initialContext)!
                      AsyncVotersAt(initialContext)
    <2> QED BY <1>1, IndexedResponsiveVoterSetIsNonempty, Isa
         DEF IndexedHistoricalRecoveryArchiveOwner
  <1> QED BY <1>1

THEOREM IndexedHistoricalRecoveryArchiveOwnerIsResponsive ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedHistoricalRecoveryArchiveOwner(initialContext) \in Responsive
BY IndexedHistoricalRecoveryArchiveOwnerIsFrozenResponsiveVoter, Isa
   DEF IndexedAsync!AsyncVotersAt

(***************************************************************************
Exact GST enabledness boundary.

Joining one owner is enough to select the product instance, but it is not
enough to enable `AsyncSetGST`: the executable guard also requires every
Responsive service owner to be active.  The roster premise below is therefore
part of the leads-to source.  It is the strongest local statement justified
by the transition relation and prevents the former one-joined-node shortcut
from silently inventing GST.
***************************************************************************)

IndexedResponsiveActiveRosterAt(initialContext) ==
  Responsive \subseteq
    IndexedAsync(initialContext)!AsyncActiveServiceNodes

THEOREM IndexedJoinedResponsiveActiveRosterIsStable ==
  \A initialContext \in AdmissibleContextRecords:
    /\ IndexedCompositionInvariant
    /\ initialContext \in JoinedContexts
    /\ IndexedResponsiveActiveRosterAt(initialContext)
    /\ [IndexedChainNext]_IndexedChainVars
    => /\ initialContext \in JoinedContexts'
       /\ IndexedResponsiveActiveRosterAt(initialContext)'
BY IndexedStepPreservesCompositionInvariant,
   JoinedMembershipIsMonotone, Isa
   DEF IndexedResponsiveActiveRosterAt,
       IndexedCompositionInvariant,
       IndexedServiceActivationCoherence,
       IndexedServiceActivationMembershipCoherenceAt,
       IndexedAsync!AsyncServiceActivationRestricted,
       IndexedAsync!AsyncActiveServiceNodes,
       JoinedContexts, IndexedChainVars

THEOREM IndexedChainSpecResponsiveActiveRosterEventuallySetsExactGst ==
  IndexedChainSpec
    => \A initialContext \in AdmissibleContextRecords:
         /\ initialContext \in JoinedContexts
         /\ IndexedResponsiveActiveRosterAt(initialContext)
           ~> IndexedCore(initialContext, 7)
PROOF
  <1>1. ASSUME IndexedChainSpec,
              NEW initialContext \in AdmissibleContextRecords
         PROVE /\ initialContext \in JoinedContexts
               /\ IndexedResponsiveActiveRosterAt(initialContext)
                 ~> IndexedCore(initialContext, 7)
    <2>1. []IndexedCompositionInvariant
      BY <1>1, IndexedChainSpecEstablishesCompositionInvariant, PTL
    <2>2. []( /\ initialContext \in JoinedContexts
              /\ IndexedResponsiveActiveRosterAt(initialContext)
              /\ ~IndexedCore(initialContext, 7)
                => ENABLED IndexedSetGstStep(initialContext))
      BY <1>1, <2>1, IndexedFairActionsRemainEnabledInProduct,
         ExpandENABLED, Isa, PTL
         DEF IndexedAsync!AsyncSetGST, IndexedAsync!SetGST,
             IndexedResponsiveActiveRosterAt,
             IndexedCore, IndexedAsyncStateAt
    <2>3. WF_IndexedChainVars(IndexedSetGstStep(initialContext))
      BY <1>1 DEF IndexedChainSpec, IndexedFairness
    <2>4. [](IndexedSetGstStep(initialContext)
              => IndexedCore(initialContext, 7)')
      BY IndexedFairProductStepsProjectExactOccurrences, Isa, PTL
         DEF IndexedSetGstStep, IndexedAsync!AsyncSetGST,
             IndexedAsync!SetGST, IndexedCore
    <2>5. []( /\ initialContext \in JoinedContexts
              /\ IndexedResponsiveActiveRosterAt(initialContext)
              /\ ~IndexedCore(initialContext, 7)
                => \/ /\ initialContext \in JoinedContexts'
                         /\ IndexedResponsiveActiveRosterAt(initialContext)'
                   \/ IndexedCore(initialContext, 7)')
      BY <1>1, <2>1, JoinedMembershipIsMonotone, Isa, PTL
         DEF IndexedResponsiveActiveRosterAt,
             IndexedCompositionInvariant,
             IndexedServiceActivationCoherence,
             IndexedServiceActivationMembershipCoherenceAt,
             IndexedAsync!AsyncActiveServiceNodes,
             JoinedContexts, IndexedChainVars
    <2> QED BY <2>2, <2>3, <2>4, <2>5, PTL
  <1> QED BY <1>1

(***************************************************************************
Indexed lift of the five exact Decision leaves.

The standalone theorem is proved from five exact owners.  This lift substitutes
the indexed state tuple and transfers only the actions those owners name:
SetGST/Tick, one current-voter RunNode, the exact packet admission action, and
the exact archive/IO owners exposed by the request lifecycle.  The current
voter finite-runner provider supplies the Candidate ranks.  This is a theorem
of the product relation, not a residual premise.
***************************************************************************)

THEOREM IndexedChainSpecClosesLocalExactDecisionOffSchedulerCorridor ==
  IndexedChainSpec
    => \A initialContext \in AdmissibleContextRecords:
         IndexedDecisionServiceWitness(initialContext)!
           ExactDecisionOffSchedulerResidualConvergenceProperty(
             IndexedChainSpec)
PROOF
  <1>1. ASSUME IndexedChainSpec,
              NEW initialContext \in AdmissibleContextRecords
         PROVE IndexedDecisionServiceWitness(initialContext)!
                 ExactDecisionOffSchedulerResidualConvergenceProperty(
                   IndexedChainSpec)
    <2>1. []IndexedHistoricalTemporalSupportAt(initialContext)
      BY <1>1, IndexedChainSpecAlwaysHistoricalTemporalSupport, PTL
    <2>2. [][IndexedDecisionServiceWitness(initialContext)!AsyncNext]_(
             IndexedDecisionServiceWitness(initialContext)!AsyncAllVars)
      BY <1>1,
         IndexedBracketStepProjectsEveryDecisionServiceWitnessStep, PTL
    <2>3. WF_(IndexedAsyncStateAt(initialContext))(
             IndexedPostGstTick(initialContext))
      BY <1>1, IndexedPostGstTickFairnessTransfersLocally
    <2>4. \A node \in IndexedAsync(initialContext)!
                         AsyncVotersAt(initialContext):
             WF_(IndexedAsyncStateAt(initialContext))(
               IndexedAsync(initialContext)!PostGstRunNode(node))
      BY <1>1, IndexedPostGstRunNodeFairnessTransfersLocally
    <2>5. IndexedHistoricalFixedClockPacketCorridorTemporalResidual
      BY <1>1, IndexedChainSpecClosesHistoricalFixedClockPacketCorridor
    <2>6. IndexedCurrentVoterProtectedCandidateStarvationProperties
      BY <1>1,
         IndexedChainSpecClosesCurrentVoterProtectedCandidateStarvation
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>5, <2>6,
         IndexedDecisionServiceWitness(initialContext)!
           ExactDecisionRequestClockOwnerConvergence,
         IndexedDecisionServiceWitness(initialContext)!
           ExactDecisionRequestRuntimePrefixConvergence,
         IndexedDecisionServiceWitness(initialContext)!
           ExactDecisionRequestHeadGateOwnerConvergence,
         IndexedDecisionServiceWitness(initialContext)!
           ExactDecisionRequestAdmissionCoalescingOutcomeIsDischarged,
         IndexedDecisionServiceWitness(initialContext)!
           ExactDecisionResponseNonPhysicalNonClaimHeadGateOwnerConvergence,
         IndexedFairActionsRemainEnabledInProduct,
         IndexedFairProductStepsProjectExactOccurrences,
         PTL, IsaT(12000)
         DEF IndexedHistoricalTemporalSupportAt,
             IndexedCurrentVoterProtectedCandidateStarvationProperties,
             IndexedDecisionServiceWitness!
               ExactDecisionOffSchedulerResidualConvergenceProperty,
             IndexedDecisionServiceWitness!
               ExactDecisionRequestClockOwnerConvergenceProperty,
             IndexedDecisionServiceWitness!
               ExactDecisionRequestRuntimePrefixConvergenceProperty,
             IndexedDecisionServiceWitness!
               ExactDecisionRequestHeadGateOwnerConvergenceProperty,
             IndexedDecisionServiceWitness!
               ExactDecisionRequestAdmissionCoalescingOutcomeConvergenceProperty,
             IndexedDecisionServiceWitness!
               ExactDecisionResponseNonPhysicalNonClaimHeadGateOwnerConvergenceProperty,
             IndexedDecisionServiceWitness!AsyncAllVars,
             IndexedDecisionServiceWitness!AsyncSchedulerVars,
             IndexedDecisionServiceWitness!AsyncRecoveryVars,
             IndexedDecisionServiceWitness!AsyncProducerVars,
             IndexedDecisionServiceWitness!vars,
             IndexedAsyncStateAt, IndexedCore,
             IndexedScheduler, IndexedRecovery, IndexedProducer
  <1> QED BY <1>1

THEOREM IndexedChainSpecProvidesLocalExactDecisionStageService ==
  IndexedChainSpec => IndexedLocalExactDecisionStageServiceProperty
PROOF
  <1>1. ASSUME IndexedChainSpec,
              NEW initialContext \in AdmissibleContextRecords
         PROVE IndexedDecisionServiceWitness(initialContext)!
                 ExactDecisionStageServiceProperty(IndexedChainSpec)
    <2>1. IndexedDecisionServiceWitness(initialContext)!
             ProtectedServiceFiniteRunnerEpisodeClosureProperty(
               IndexedChainSpec)
      BY <1>1,
         IndexedChainSpecProvidesCurrentVoterFiniteRunnerEpisodeClosure,
         Isa
         DEF IndexedCurrentVoterFiniteRunnerEpisodeClosureProperties,
             IndexedDecisionServiceWitness!
               ProtectedServiceFiniteRunnerEpisodeClosureProperty,
             IndexedDecisionServiceWitness!
               AsyncReadyRunnerEpisodeClosureProperty,
             IndexedDecisionServiceWitness!
               AsyncCapacityRunnerEpisodeClosureProperty,
             IndexedHistoricalTransport!
               AsyncReadyRunnerEpisodeClosureProperty,
             IndexedHistoricalTransport!
               AsyncCapacityRunnerEpisodeClosureProperty,
             IndexedCore, IndexedScheduler, IndexedRecovery
    <2>2. IndexedDecisionServiceWitness(initialContext)!
             ExactDecisionOffSchedulerResidualConvergenceProperty(
               IndexedChainSpec)
      BY <1>1, IndexedChainSpecClosesLocalExactDecisionOffSchedulerCorridor
    <2> QED BY <2>1, <2>2,
         IndexedDecisionServiceWitness(initialContext)!
           ExactDecisionOffSchedulerResidualConvergenceDischargesStageService
  <1> QED BY <1>1
       DEF IndexedLocalExactDecisionStageServiceProperty

(***************************************************************************
Generic adequate-leader product lift.

The sibling module supplies the local semantic reduction.  The product lift
feeds it the exact frozen-owner fairness and the already-proved packet,
runner, timeout, tombstone, and finite-episode providers.  Its conclusion is
GST-to-Decision for one target; it never forms the aggregate Decide mode.
***************************************************************************)

THEOREM IndexedLocalAdequateLeaderSemanticKernelProvidesDecisionConvergence ==
  IndexedLocalAdequateLeaderSemanticKernelProperty
    => IndexedLocalAdequateLeaderDecisionConvergenceProperty
PROOF
  <1>1. ASSUME IndexedLocalAdequateLeaderSemanticKernelProperty,
              NEW initialContext \in AdmissibleContextRecords
         PROVE IndexedAdequateLeaderWitness(initialContext)!
                 AdequateLeaderLocalTargetDecisionConvergenceProperty(
                   IndexedChainSpec)
    <2>1. IndexedAdequateLeaderWitness(initialContext)!
             AdequateLeaderLocalSemanticKernelProperty(IndexedChainSpec)
      BY <1>1 DEF IndexedLocalAdequateLeaderSemanticKernelProperty
    <2> QED BY <2>1,
         IndexedAdequateLeaderWitness(initialContext)!
           AdequateLeaderLocalSemanticKernelSuppliesTargetDecisionConvergence
  <1> QED BY <1>1
       DEF IndexedLocalAdequateLeaderDecisionConvergenceProperty

(***************************************************************************
One exact local application becomes one immutable archive authority.
***************************************************************************)

THEOREM IndexedLocalAppliedVoterSuppliesTypedArchiveAuthority ==
  \A initialContext \in AdmissibleContextRecords,
     server \in ValidatorIds:
    /\ IndexedCompositionInvariant
    /\ IndexedDecisionWitnessSupportAt(initialContext)
    /\ IndexedResponsiveRecoveryDormant
    /\ IndexedCore(initialContext, 7)
    /\ server \in IndexedAsync(initialContext)!
                   AsyncCurrentResponsiveVoters
    /\ server \in joinedByContext[initialContext]
    /\ IndexedAsync(initialContext)!NodeHasApplication(server)
    => IndexedHistoricalRecoveryTypedArchiveAuthority(initialContext)
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
                NEW server \in ValidatorIds,
                IndexedCompositionInvariant,
                IndexedDecisionWitnessSupportAt(initialContext),
                IndexedResponsiveRecoveryDormant,
                IndexedCore(initialContext, 7),
                server \in IndexedAsync(initialContext)!
                            AsyncCurrentResponsiveVoters,
                server \in joinedByContext[initialContext],
                IndexedAsync(initialContext)!NodeHasApplication(server)
         PROVE IndexedHistoricalRecoveryTypedArchiveAuthority(
                   initialContext)
    <2> QED BY <1>1,
         IndexedDecisionWitness(initialContext)!GstResponsiveNodesAreUp,
         IsaT(900)
         DEF IndexedHistoricalRecoveryTypedArchiveAuthority,
             IndexedHistoricalRecoverySourceReady,
             IndexedDecisionWitnessSupportAt,
             IndexedCompositionInvariant,
             IndexedTotalReceiptProjection,
             IndexedDecisionReceiptProjection,
             IndexedApplicationReceiptProjection,
             IndexedCurrentDecisions, IndexedCurrentApplications,
             IndexedDecisions, IndexedApplications,
             JoinedContexts,
             IndexedResponsiveRecoveryDormant,
             IndexedCore, IndexedRecovery,
             Chain!ChainEpochInvariant,
             Chain!ChainEpochTypeInvariant,
             Chain!DurableDecisionEvidenceSound,
             Chain!DurableApplicationEvidenceSound,
             Chain!ApplicationHasRecordedDecision,
             Chain!DecisionBacksCertifiedSlot,
             Chain!CanonicalCommitForSlot,
             Chain!ReceiptOutsideChainHorizon,
             IndexedDecisionWitness!AsyncStrongTypeInvariant,
             IndexedDecisionWitness!StrongInductiveInvariant,
             IndexedDecisionWitness!Safety,
             IndexedDecisionWitness!AppliedRequiresDecision,
             IndexedDecisionWitness!AsyncRecoveryTypeInvariant,
             IndexedDecisionWitness!AsyncGstRecoveryPhaseInvariant,
             IndexedDecisionWitness!
               CurrentAppliedArchiveBodyRetentionInvariant,
             IndexedDecisionWitness!AsyncCurrentResponsiveVoters,
             IndexedDecisionWitness!CurrentVoters,
             IndexedDecisionWitness!CurrentEpoch,
             IndexedDecisionWitness!NodeHasApplication,
             IndexedDecisionWitness!BodyHeldBy,
             IndexedAsync!NodeHasApplication,
             IndexedAsync!AsyncCurrentResponsiveVoters,
             IndexedAsync!BodyHeldBy,
             AdmissibleContextRecords, FrozenContextAdmissible,
             ContextRecords, Heights, ModelConfiguration, ValidatorIds
  <1> QED BY <1>1

THEOREM IndexedChainSpecJoinedArchiveOwnerProducesTypedAuthority ==
  /\ IndexedChainSpec
  /\ IndexedLocalAdequateLeaderDecisionConvergenceProperty
    => \A initialContext \in AdmissibleContextRecords:
         /\ IndexedHistoricalRecoveryArchiveOwnerJoined(initialContext)
         /\ IndexedResponsiveActiveRosterAt(initialContext)
           ~> IndexedHistoricalRecoveryTypedArchiveAuthority(initialContext)
PROOF
  <1>1. ASSUME IndexedChainSpec,
              IndexedLocalAdequateLeaderDecisionConvergenceProperty,
              NEW initialContext \in AdmissibleContextRecords
         PROVE /\ IndexedHistoricalRecoveryArchiveOwnerJoined(initialContext)
               /\ IndexedResponsiveActiveRosterAt(initialContext)
                 ~> IndexedHistoricalRecoveryTypedArchiveAuthority(
                      initialContext)
    <2> DEFINE server ==
          IndexedHistoricalRecoveryArchiveOwner(initialContext)
    <2>1. server \in IndexedAsync(initialContext)!
                    AsyncVotersAt(initialContext)
      BY <1>1, IndexedHistoricalRecoveryArchiveOwnerIsFrozenResponsiveVoter
    <2>2. []IndexedCompositionInvariant
      BY <1>1, IndexedChainSpecEstablishesCompositionInvariant, PTL
    <2>3. []IndexedDecisionWitnessSupport
      BY <1>1, IndexedChainSpecAlwaysDecisionWitnessSupport, PTL
    <2>4. []IndexedResponsiveRecoveryDormant
      BY <1>1, IndexedChainSpecKeepsResponsiveRecoveryDormant, PTL
    <2>5. /\ IndexedHistoricalRecoveryArchiveOwnerJoined(initialContext)
           /\ IndexedResponsiveActiveRosterAt(initialContext)
             => /\ initialContext \in JoinedContexts
                /\ IndexedResponsiveActiveRosterAt(initialContext)
      BY <1>1, <2>1, Isa
         DEF IndexedHistoricalRecoveryArchiveOwnerJoined, JoinedContexts
    <2>6. /\ initialContext \in JoinedContexts
           /\ IndexedResponsiveActiveRosterAt(initialContext)
             ~> IndexedCore(initialContext, 7)
      BY <1>1,
         IndexedChainSpecResponsiveActiveRosterEventuallySetsExactGst
    <2>7. IndexedAdequateLeaderWitness(initialContext)!
             AdequateLeaderLocalTargetDecisionConvergenceProperty(
               IndexedChainSpec)
      BY <1>1 DEF IndexedLocalAdequateLeaderDecisionConvergenceProperty
    <2>8. IndexedDecisionServiceWitness(initialContext)!
             ExactDecisionStageServiceProperty(IndexedChainSpec)
      BY <1>1, IndexedChainSpecProvidesLocalExactDecisionStageService
         DEF IndexedLocalExactDecisionStageServiceProperty
    <2>9. []( /\ IndexedCore(initialContext, 7)
              /\ server \in joinedByContext[initialContext]
              /\ ~IndexedNodeCurrentAt(initialContext, server)
                => IndexedAsync(initialContext)!
                     NodeHasApplication(server))
      BY <1>1, <2>2, JoinedNonCurrentHasApplicationEvidence, PTL
    <2>10. ( /\ IndexedCore(initialContext, 7)
              /\ IndexedNodeCurrentAt(initialContext, server)
              /\ ~IndexedAsync(initialContext)!NodeHasDecision(server))
              ~> IndexedAsync(initialContext)!NodeHasDecision(server)
      BY <1>1, <2>1, <2>7, <2>2, PTL, Isa
         DEF IndexedAdequateLeaderWitness!
               AdequateLeaderLocalTargetDecisionConvergenceProperty,
             IndexedAdequateLeaderWitness!
               AdequateLeaderLocalTargetDecisionSource,
             IndexedAdequateLeaderWitness!
               AsyncCurrentResponsiveVoters,
             IndexedAsync!AsyncVotersAt,
             IndexedAsync!AsyncCurrentResponsiveVoters,
             IndexedNodeCurrentAt
    <2>11. []( /\ IndexedCore(initialContext, 7)
               /\ IndexedNodeCurrentAt(initialContext, server)
               /\ IndexedAsync(initialContext)!NodeHasDecision(server)
                 => \E qc:
                      IndexedDecisionServiceWitness(initialContext)!
                        ExactDecisionServiceSource(server, qc))
      BY <1>1, <2>1, <2>2, <2>3,
         IndexedDecisionServiceWitness(initialContext)!
           PostGstResponsiveDecisionHasExactServiceSource,
         PTL, Isa
         DEF IndexedDecisionWitnessSupport,
             IndexedDecisionWitnessSupportAt,
             IndexedDecisionServiceWitness!AsyncCurrentResponsiveVoters,
             IndexedAsync!AsyncVotersAt,
             IndexedAsync!AsyncCurrentResponsiveVoters,
             IndexedNodeCurrentAt
    <2>12. \A qc:
              IndexedDecisionServiceWitness(initialContext)!
                ExactDecisionServiceSource(server, qc)
                ~> IndexedAsync(initialContext)!NodeHasApplication(server)
      BY <2>8
         DEF IndexedDecisionServiceWitness!
               ExactDecisionStageServiceProperty,
             IndexedDecisionServiceWitness!NodeHasApplication,
             IndexedAsync!NodeHasApplication
    <2>13. ( /\ IndexedCore(initialContext, 7)
              /\ server \in joinedByContext[initialContext])
              ~> IndexedAsync(initialContext)!NodeHasApplication(server)
      BY <2>9, <2>10, <2>11, <2>12, PTL
    <2>14. []( /\ IndexedCore(initialContext, 7)
               /\ server \in joinedByContext[initialContext]
               /\ IndexedAsync(initialContext)!NodeHasApplication(server)
                 => IndexedHistoricalRecoveryTypedArchiveAuthority(
                      initialContext))
      BY <1>1, <2>1, <2>2, <2>3, <2>4,
         IndexedLocalAppliedVoterSuppliesTypedArchiveAuthority,
         PTL DEF IndexedDecisionWitnessSupport
    <2> QED BY <2>5, <2>6, <2>13, <2>14, PTL
         DEF IndexedHistoricalRecoveryArchiveOwnerJoined
  <1> QED BY <1>1

IndexedHistoricalRecoveryActivationPrefixResidualProperty ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    IndexedHistoricalRecoveryAuthorityAcquisitionResidual(
      initialContext, node)
      ~> (IndexedHistoricalRecoveryEntryGoal(initialContext, node)
           \/ /\ IndexedHistoricalRecoveryArchiveOwnerJoined(initialContext)
              /\ IndexedResponsiveActiveRosterAt(initialContext)
              /\ IndexedHistoricalRecoveryAuthorityAcquisitionResidual(
                   initialContext, node))

IndexedHistoricalRecoveryGenesisActivationPrefixResidualProperty ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    /\ initialContext = GenesisContext
    /\ IndexedHistoricalRecoveryAuthorityAcquisitionResidual(
         initialContext, node)
      ~> (IndexedHistoricalRecoveryEntryGoal(initialContext, node)
           \/ /\ IndexedHistoricalRecoveryArchiveOwnerJoined(initialContext)
              /\ IndexedResponsiveActiveRosterAt(initialContext)
              /\ IndexedHistoricalRecoveryAuthorityAcquisitionResidual(
                   initialContext, node))

IndexedHistoricalRecoveryNonGenesisActivationPrefixResidualProperty ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    /\ initialContext # GenesisContext
    /\ IndexedHistoricalRecoveryAuthorityAcquisitionResidual(
         initialContext, node)
      ~> (IndexedHistoricalRecoveryEntryGoal(initialContext, node)
           \/ /\ IndexedHistoricalRecoveryArchiveOwnerJoined(initialContext)
              /\ IndexedResponsiveActiveRosterAt(initialContext)
              /\ IndexedHistoricalRecoveryAuthorityAcquisitionResidual(
                   initialContext, node))

IndexedHistoricalRecoveryPredecessorCatchupResidualProperty ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    /\ initialContext # GenesisContext
    /\ IndexedHistoricalRecoveryAuthorityAcquisitionResidual(
         initialContext, node)
      ~> (IndexedHistoricalRecoveryEntryGoal(initialContext, node)
           \/ /\ IndexedResponsiveHeightReached(initialContext.height)
              /\ IndexedHistoricalRecoveryAuthorityAcquisitionResidual(
                   initialContext, node))

THEOREM IndexedHistoricalRecoveryActivationPrefixSplitsAtGenesis ==
  IndexedHistoricalRecoveryActivationPrefixResidualProperty
    <=> /\ IndexedHistoricalRecoveryGenesisActivationPrefixResidualProperty
        /\ IndexedHistoricalRecoveryNonGenesisActivationPrefixResidualProperty
BY PTL
   DEF IndexedHistoricalRecoveryActivationPrefixResidualProperty,
       IndexedHistoricalRecoveryGenesisActivationPrefixResidualProperty,
       IndexedHistoricalRecoveryNonGenesisActivationPrefixResidualProperty

THEOREM IndexedChainSpecAlwaysJoinsHistoricalGenesisArchiveOwner ==
  IndexedChainSpec
    => []( /\ IndexedHistoricalRecoveryArchiveOwnerJoined(GenesisContext)
           /\ IndexedResponsiveActiveRosterAt(GenesisContext))
PROOF
  <1>1. IndexedChainInit
           => IndexedHistoricalRecoveryArchiveOwnerJoined(GenesisContext)
    BY IndexedHistoricalRecoveryArchiveOwnerIsResponsive, Isa
       DEF IndexedChainInit,
           IndexedHistoricalRecoveryArchiveOwnerJoined,
           GenesisContext, AdmissibleContextRecords,
           FrozenContextAdmissible, ContextRecords, LineagesAt, Heights,
           ModelConfiguration, ValidatorIds
  <1>2. IndexedHistoricalRecoveryArchiveOwnerJoined(GenesisContext)
           /\ [IndexedChainNext]_IndexedChainVars
           => IndexedHistoricalRecoveryArchiveOwnerJoined(GenesisContext)'
    BY IndexedNodeJoinIsStable, Isa
       DEF IndexedHistoricalRecoveryArchiveOwnerJoined,
           AdmissibleContextRecords, FrozenContextAdmissible,
           ContextRecords, LineagesAt, Heights, GenesisContext,
           ModelConfiguration, ValidatorIds
  <1>3. IndexedChainSpec
           => []IndexedResponsiveActiveRosterAt(GenesisContext)
    BY IndexedChainSpecKeepsGenesisResponsiveRosterActive
       DEF IndexedResponsiveActiveRosterAt
  <1> QED BY <1>1, <1>2, <1>3, PTL DEF IndexedChainSpec

THEOREM IndexedLiveChainSpecClosesHistoricalGenesisActivationPrefix ==
  IndexedLiveChainSpec
    => IndexedHistoricalRecoveryGenesisActivationPrefixResidualProperty
BY IndexedLiveChainSpecProjectsIndexedChainSpec,
   IndexedChainSpecAlwaysJoinsHistoricalGenesisArchiveOwner, PTL
   DEF IndexedHistoricalRecoveryGenesisActivationPrefixResidualProperty

IndexedHistoricalRecoveryActivatedArchiveProducerResidualProperty ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    /\ IndexedHistoricalRecoveryArchiveOwnerJoined(initialContext)
    /\ IndexedResponsiveActiveRosterAt(initialContext)
    /\ IndexedHistoricalRecoveryAuthorityAcquisitionResidual(
         initialContext, node)
      ~> (IndexedHistoricalRecoveryEntryGoal(initialContext, node)
           \/ /\ IndexedHistoricalRecoveryTypedArchiveAuthority(
                    initialContext)
              /\ IndexedHistoricalRecoveryAuthorityAcquisitionResidual(
                   initialContext, node))

IndexedHistoricalRecoveryTypedArchiveEntryResidualProperty ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    /\ IndexedHistoricalRecoveryTypedArchiveAuthority(initialContext)
    /\ IndexedHistoricalRecoveryAuthorityAcquisitionResidual(
         initialContext, node)
      ~> IndexedHistoricalRecoveryEntryGoal(initialContext, node)

THEOREM IndexedExactTypedArchiveResidualHasEntry ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    /\ IndexedCompositionInvariant
    /\ IndexedDecisionWitnessSupportAt(initialContext)
    /\ IndexedResponsiveRecoveryDormant
    /\ IndexedHistoricalRecoveryTypedArchiveAuthority(initialContext)
    /\ IndexedHistoricalRecoveryAuthorityAcquisitionResidual(
         initialContext, node)
    => IndexedHistoricalRecoveryEntryGoal(initialContext, node)
BY IsaT(480)
   DEF IndexedHistoricalRecoveryTypedArchiveAuthority,
       IndexedHistoricalRecoveryAuthorityAcquisitionResidual,
       IndexedHistoricalRecoveryEntryGoal,
       IndexedHistoricalRecoveryOpenable,
       IndexedHistoricalRecoveryTargetReady,
       IndexedHistoricalRecoverySourceReady,
       IndexedHistoricalExactApplication,
       IndexedHistoricalDecisionOwned,
       IndexedHistoricalRecoveryRunnerOwned,
       IndexedHistoricalRecoveryTargetOwned,
       HistoricalRecoveryOutstanding,
       ExactNodeLocationAt,
       IndexedProjectedNodeHasApplication,
       HistoricalRecoveryNodeHasApplicationProjection,
       IndexedDecisionWitnessSupportAt,
       IndexedHistoricalTransport!
         ReachableResponsiveDecisionServiceOwnershipInvariant,
       IndexedHistoricalTransport!NodeHasDecision,
       IndexedHistoricalTransport!NodeHasApplication,
       IndexedHistoricalTransport!AsyncCurrentResponsiveVoters,
       IndexedHistoricalTransport!HistoricalRecoveryTarget,
       IndexedResponsiveRecoveryDormant,
       IndexedCore, IndexedRecovery,
       IndexedDecisionWitness!AsyncStrongTypeInvariant,
       IndexedDecisionWitness!StrongInductiveInvariant,
       IndexedDecisionWitness!Safety,
       IndexedDecisionWitness!TypeInvariant,
       IndexedDecisionWitness!AsyncRecoveryTypeInvariant,
       IndexedDecisionWitness!AsyncCurrentResponsiveVoters,
       IndexedDecisionWitness!HistoricalRecoveryTarget,
       IndexedDecisionWitness!NodeHasDecision,
       IndexedDecisionWitness!NodeHasApplication,
       IndexedAsync!AsyncCurrentResponsiveVoters,
       IndexedAsync!HistoricalRecoveryTarget,
       IndexedAsync!NodeHasDecision,
       IndexedAsync!NodeHasApplication,
       AdmissibleContextRecords, FrozenContextAdmissible,
       ContextRecords, Heights, ModelConfiguration, ValidatorIds

THEOREM IndexedHistoricalAuthorityResidualPersistsOrEnters ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    /\ IndexedCompositionInvariant
    /\ IndexedHistoricalRecoveryAuthorityAcquisitionResidual(
         initialContext, node)
    /\ [IndexedChainNext]_IndexedChainVars
    => \/ IndexedHistoricalRecoveryAuthorityAcquisitionResidual(
             initialContext, node)'
       \/ IndexedHistoricalRecoveryEntryGoal(initialContext, node)'
BY IndexedStepPreservesCompositionInvariant,
   IndexedBracketStepKeepsNodeHeightsMonotone,
   IndexedNodeJoinIsStable, Isa
   DEF IndexedHistoricalRecoveryAuthorityAcquisitionResidual,
       IndexedHistoricalRecoveryEntryGoal,
       IndexedHistoricalRecoveryOpenable,
       IndexedHistoricalExactApplication,
       IndexedHistoricalDecisionOwned,
       IndexedHistoricalRecoveryRunnerOwned,
       IndexedHistoricalRecoveryTargetOwned,
       IndexedHistoricalRecoveryTargetReady,
       IndexedHistoricalRecoverySourceReady,
       HistoricalRecoveryOutstanding,
       ExactNodeLocationAt,
       IndexedCurrentDecisions, IndexedCurrentApplications,
       IndexedDecisionEvidence, IndexedApplicationEvidence,
       IndexedCompositionInvariant,
       IndexedEveryInstanceStrongInvariant,
       IndexedTotalReceiptProjection,
       IndexedDecisionReceiptProjection,
       IndexedApplicationReceiptProjection,
       IndexedChainNext, IndexedChainVars,
       IndexedProductActionAt, IndexedReceiptClassification,
       IndexedReceiptFreeChainStutter,
       IndexedDecisionReceiptHandoff,
       IndexedApplicationReceiptHandoff,
       NewIndexedDecisionReceipt, NewIndexedApplicationReceipt,
       NoNewIndexedDurableReceipt,
       IndexedAsync!StrongInductiveInvariant,
       IndexedAsync!Safety, IndexedAsync!TypeInvariant,
       IndexedAsync!AsyncHistoricalRecoveryTypeInvariant,
       IndexedAsync!NodeHasDecision,
       IndexedAsync!NodeHasApplication,
       IndexedAsync!HistoricalRecoveryTarget,
       IndexedAsync!AsyncCurrentResponsiveVoters,
       IndexedAsync!BodyHeldBy,
       IndexedAsync!AsyncNext,
       IndexedAsync!AsyncNonCrashStep,
       IndexedAsync!AsyncSetGST,
       IndexedAsync!PreGstCrash,
       IndexedAsync!PreGstResponsiveCrash,
       IndexedAsync!PreGstResponsiveRestart,
       IndexedAsync!PreGstResponsiveReplay,
       IndexedAsync!Crash,
       IndexedAsync!Restart,
       IndexedAsync!ApplyDecision,
       Chain!RecordCertifiedNext, Chain!RecordKnownDecision,
       Chain!RecordAppliedNext, Chain!RecordKnownApplication

THEOREM IndexedLiveChainSpecClosesActivatedArchiveProducerResidual ==
  /\ IndexedLiveChainSpec
  /\ IndexedLocalAdequateLeaderDecisionConvergenceProperty
    => IndexedHistoricalRecoveryActivatedArchiveProducerResidualProperty
PROOF
  <1>1. ASSUME IndexedLiveChainSpec,
              IndexedLocalAdequateLeaderDecisionConvergenceProperty,
              NEW initialContext \in AdmissibleContextRecords,
              NEW node \in Responsive
         PROVE /\ IndexedHistoricalRecoveryArchiveOwnerJoined(initialContext)
                /\ IndexedResponsiveActiveRosterAt(initialContext)
                /\ IndexedHistoricalRecoveryAuthorityAcquisitionResidual(
                     initialContext, node)
                 ~> (IndexedHistoricalRecoveryEntryGoal(
                       initialContext, node)
                      \/ /\ IndexedHistoricalRecoveryTypedArchiveAuthority(
                               initialContext)
                         /\ IndexedHistoricalRecoveryAuthorityAcquisitionResidual(
                              initialContext, node))
    <2>0. IndexedChainSpec
      BY <1>1, IndexedLiveChainSpecProjectsIndexedChainSpec
    <2>1. []IndexedCompositionInvariant
      BY <2>0, IndexedChainSpecEstablishesCompositionInvariant, PTL
    <2>2. [][IndexedChainNext]_IndexedChainVars
      BY <2>0, PTL DEF IndexedChainSpec
    <2>3. IndexedCompositionInvariant
             /\ IndexedHistoricalRecoveryAuthorityAcquisitionResidual(
                  initialContext, node)
             /\ [IndexedChainNext]_IndexedChainVars
             => \/ IndexedHistoricalRecoveryAuthorityAcquisitionResidual(
                      initialContext, node)'
                \/ IndexedHistoricalRecoveryEntryGoal(
                     initialContext, node)'
      BY <1>1, IndexedHistoricalAuthorityResidualPersistsOrEnters
    <2>4. /\ IndexedHistoricalRecoveryArchiveOwnerJoined(initialContext)
           /\ IndexedResponsiveActiveRosterAt(initialContext)
             ~> IndexedHistoricalRecoveryTypedArchiveAuthority(initialContext)
      BY <1>1, <2>0,
         IndexedChainSpecJoinedArchiveOwnerProducesTypedAuthority
    <2> QED BY <2>1, <2>2, <2>3, <2>4, PTL
  <1> QED BY <1>1
       DEF IndexedHistoricalRecoveryActivatedArchiveProducerResidualProperty

THEOREM IndexedLiveChainSpecClosesTypedArchiveEntryResidual ==
  IndexedLiveChainSpec
    => IndexedHistoricalRecoveryTypedArchiveEntryResidualProperty
PROOF
  <1>1. ASSUME IndexedLiveChainSpec,
              NEW initialContext \in AdmissibleContextRecords,
              NEW node \in Responsive
         PROVE /\ IndexedHistoricalRecoveryTypedArchiveAuthority(
                       initialContext)
                /\ IndexedHistoricalRecoveryAuthorityAcquisitionResidual(
                     initialContext, node)
                 ~> IndexedHistoricalRecoveryEntryGoal(
                      initialContext, node)
    <2>0. IndexedChainSpec
      BY <1>1, IndexedLiveChainSpecProjectsIndexedChainSpec
    <2>1. []IndexedCompositionInvariant
      BY <2>0, IndexedChainSpecEstablishesCompositionInvariant, PTL
    <2>2. []IndexedDecisionWitnessSupport
      BY <2>0, IndexedChainSpecAlwaysDecisionWitnessSupport, PTL
    <2>3. []IndexedResponsiveRecoveryDormant
      BY <2>0, IndexedChainSpecKeepsResponsiveRecoveryDormant, PTL
    <2>4. []( /\ IndexedHistoricalRecoveryTypedArchiveAuthority(
                       initialContext)
              /\ IndexedHistoricalRecoveryAuthorityAcquisitionResidual(
                   initialContext, node)
                => IndexedHistoricalRecoveryEntryGoal(
                     initialContext, node))
      BY <1>1, <2>1, <2>2, <2>3,
         IndexedExactTypedArchiveResidualHasEntry,
         PTL DEF IndexedDecisionWitnessSupport
    <2> QED BY <2>4, PTL
  <1> QED BY <1>1
       DEF IndexedHistoricalRecoveryTypedArchiveEntryResidualProperty


(***************************************************************************
Strict-height recovery/Decision mutual induction.

The target-height activation prefix must not assume recovery at its own
height.  The operators below expose the exact induction interface:

  * joint progress at one context carries authority acquisition, entry
    completion, and context-local Decision-rank progress together;
  * lower authority and entry completion range only over strict ancestors in
    the frozen target's immutable lineage and yield strict-ancestor advance;
  * that advance joins the deterministic frozen-roster archive owner, while
    the exact historical target remains a separate rank premise; and
  * progress through a numeric height is the finite induction accumulator.

The chain refinement's `IndexedStrictAncestorRecoveryAdvance` kernel turns
strict-ancestor completion into responsive target membership using the typed
successor lifecycle.  The current target residual is framed until one of its
entry arms appears, so joining the remaining responsive nodes cannot silently
discard the admitted target.
***************************************************************************)

IndexedHistoricalRecoveryEntryCompletionProperty ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    IndexedHistoricalRecoveryEntryGoal(initialContext, node)
      ~> HistoricalRecoveryComplete(initialContext, node)

IndexedHistoricalRecoveryEntryCompletionAt(initialContext) ==
  \A node \in Responsive:
    IndexedHistoricalRecoveryEntryGoal(initialContext, node)
      ~> HistoricalRecoveryComplete(initialContext, node)

IndexedHistoricalRecoveryEntryCompletionBelow(targetContext) ==
  \A blockHeight \in 0..targetContext.height:
    blockHeight < targetContext.height
      => IndexedHistoricalRecoveryEntryCompletionAt(
           IndexedAncestorContext(targetContext, blockHeight))

IndexedHistoricalRecoveryAuthorityProgressAt(initialContext) ==
  \A node \in Responsive:
    IndexedHistoricalRecoveryAuthorityAcquisitionResidual(
      initialContext, node)
      ~> IndexedHistoricalRecoveryEntryGoal(initialContext, node)

IndexedHistoricalRecoveryJointProgressAt(initialContext) ==
  /\ IndexedHistoricalRecoveryAuthorityProgressAt(initialContext)
  /\ IndexedHistoricalRecoveryEntryCompletionAt(initialContext)
  /\ IndexedHistoricalDecisionRankProgressAtContext(initialContext)

IndexedHistoricalRecoveryJointProgressBelow(targetContext) ==
  \A blockHeight \in 0..targetContext.height:
    blockHeight < targetContext.height
      => IndexedHistoricalRecoveryJointProgressAt(
           IndexedAncestorContext(targetContext, blockHeight))

IndexedHistoricalRecoveryJointProgressThroughHeight(limit) ==
  \A initialContext \in AdmissibleContextRecords:
    initialContext.height <= limit
      => IndexedHistoricalRecoveryJointProgressAt(initialContext)

IndexedHistoricalRecoveryJointProgressProperty ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedHistoricalRecoveryJointProgressAt(initialContext)

IndexedHistoricalRecoveryAuthorityProgressBelow(targetContext) ==
  \A blockHeight \in 0..targetContext.height:
    blockHeight < targetContext.height
      => \A node \in Responsive:
           IndexedHistoricalRecoveryAuthorityAcquisitionResidual(
             IndexedAncestorContext(targetContext, blockHeight), node)
             ~> IndexedHistoricalRecoveryEntryGoal(
                  IndexedAncestorContext(targetContext, blockHeight), node)

THEOREM IndexedHistoricalServiceKernelsDischargeEntryCompletion ==
  /\ IndexedChainSpec
  /\ IndexedHistoricalCertificateRankProgressResidualProperty
  /\ IndexedHistoricalDecisionRankProgressResidualProperty
  => IndexedHistoricalRecoveryEntryCompletionProperty
PROOF
  <1>1. ASSUME IndexedChainSpec,
              IndexedHistoricalCertificateRankProgressResidualProperty,
              IndexedHistoricalDecisionRankProgressResidualProperty,
              NEW initialContext \in AdmissibleContextRecords,
              NEW node \in Responsive
         PROVE IndexedHistoricalRecoveryEntryGoal(initialContext, node)
                 ~> HistoricalRecoveryComplete(initialContext, node)
    <2>1. IndexedHistoricalRecoveryAuthorityReady(initialContext, node)
             ~> HistoricalRecoveryComplete(initialContext, node)
      BY <1>1,
         IndexedHistoricalServiceKernelsDischargeAuthorityReadyProgress
         DEF IndexedExactHistoricalRecoveryFromAuthorityProgress
    <2>2. IndexedHistoricalExactApplication(initialContext, node)
             ~> HistoricalRecoveryComplete(initialContext, node)
      BY <1>1, IndexedChainSpecClosesHistoricalApplicationReceiptHandoff
         DEF IndexedHistoricalApplicationReceiptHandoffProperty
    <2>3. \A rank \in Nat:
             IndexedHistoricalDecisionStageAt(
               initialContext, node, rank)
               ~> IndexedHistoricalExactApplication(
                    initialContext, node)
      BY <1>1, IndexedHistoricalDecisionRankConvergence
    <2>4. IndexedHistoricalDecisionOwned(initialContext, node)
             => (IndexedHistoricalDecisionStageGoal(
                   initialContext, node)
                  \/ IndexedHistoricalDecisionStageOwnershipResidual(
                       initialContext, node))
      BY DEF IndexedHistoricalDecisionStageOwnershipResidual
    <2>5. IndexedHistoricalDecisionStageOwnershipResidual(
             initialContext, node)
             ~> IndexedHistoricalDecisionStageGoal(
                  initialContext, node)
      BY <1>1, IndexedHistoricalDecisionStageOwnershipResidualObligation
         DEF IndexedHistoricalDecisionStageOwnershipResidualProperty
    <2>6. (\E rank \in 1..6:
             IndexedHistoricalDecisionStageAt(
               initialContext, node, rank))
             ~> IndexedHistoricalExactApplication(
                  initialContext, node)
      BY <2>3, PTL
    <2>7. IndexedHistoricalDecisionStageGoal(initialContext, node)
             ~> IndexedHistoricalExactApplication(initialContext, node)
      BY <2>6, PTL DEF IndexedHistoricalDecisionStageGoal
    <2>8. IndexedHistoricalDecisionOwned(initialContext, node)
             ~> IndexedHistoricalExactApplication(initialContext, node)
      BY <2>4, <2>5, <2>7, PTL
    <2>9. IndexedHistoricalRecoveryEntryGoal(initialContext, node)
             ~> (IndexedHistoricalExactApplication(initialContext, node)
                  \/ IndexedHistoricalRecoveryAuthorityReady(
                       initialContext, node))
      BY <2>8, PTL
         DEF IndexedHistoricalRecoveryEntryGoal,
             IndexedHistoricalRecoveryAuthorityReady
    <2> QED BY <2>1, <2>2, <2>9, PTL
  <1> QED BY <1>1
       DEF IndexedHistoricalRecoveryEntryCompletionProperty

THEOREM IndexedHistoricalServiceKernelsDischargeAuthorityReadyProgressAt ==
  /\ IndexedChainSpec
  /\ IndexedHistoricalCertificateRankProgressResidualProperty
  => \A initialContext \in AdmissibleContextRecords:
       IndexedHistoricalDecisionRankProgressAtContext(initialContext)
         => \A node \in Responsive:
              IndexedHistoricalRecoveryAuthorityReady(initialContext, node)
                ~> HistoricalRecoveryComplete(initialContext, node)
PROOF
  <1>1. ASSUME IndexedChainSpec,
              IndexedHistoricalCertificateRankProgressResidualProperty,
              NEW initialContext \in AdmissibleContextRecords,
              IndexedHistoricalDecisionRankProgressAtContext(initialContext),
              NEW node \in Responsive
         PROVE IndexedHistoricalRecoveryAuthorityReady(
                 initialContext, node)
                 ~> HistoricalRecoveryComplete(initialContext, node)
    <2>1. IndexedHistoricalRecoveryOpenTargetResidualProperty
      BY <1>1, IndexedChainSpecClosesHistoricalOpenTarget
    <2>2. IndexedHistoricalApplicationReceiptHandoffProperty
      BY <1>1, IndexedChainSpecClosesHistoricalApplicationReceiptHandoff
    <2>3. \A rank \in Nat:
             IndexedHistoricalCertificateStageAt(
               initialContext, node, rank)
               ~> IndexedHistoricalCertificateGoal(
                    initialContext, node)
      BY <1>1, IndexedHistoricalCertificateRankConvergence
    <2>4. \A rank \in Nat:
             IndexedHistoricalDecisionStageAt(
               initialContext, node, rank)
               ~> IndexedHistoricalExactApplication(
                    initialContext, node)
      BY <1>1, IndexedHistoricalDecisionRankConvergenceAtContext
    <2>5. IndexedHistoricalRecoveryOpenable(initialContext, node)
             => (IndexedHistoricalRecoveryOpenResidual(
                   initialContext, node)
                  \/ IndexedHistoricalRecoveryOpenGoal(
                       initialContext, node))
      BY DEF IndexedHistoricalRecoveryOpenResidual,
             IndexedHistoricalRecoveryOpenGoal
    <2>6. IndexedHistoricalRecoveryOpenResidual(initialContext, node)
             ~> IndexedHistoricalRecoveryOpenGoal(initialContext, node)
      BY <2>1
         DEF IndexedHistoricalRecoveryOpenTargetResidualProperty
    <2>7. IndexedHistoricalRecoveryTargetOwned(initialContext, node)
             => (IndexedHistoricalCertificateGoal(initialContext, node)
                  \/ \E rank \in 1..4:
                       IndexedHistoricalCertificateStageAt(
                         initialContext, node, rank))
      BY IndexedHistoricalTargetHasExactCertificateStage
    <2>8. (\E rank \in 1..4:
             IndexedHistoricalCertificateStageAt(
               initialContext, node, rank))
             ~> IndexedHistoricalCertificateGoal(initialContext, node)
      BY <2>3, PTL
    <2>9. IndexedHistoricalDecisionOwned(initialContext, node)
             => (IndexedHistoricalDecisionStageGoal(initialContext, node)
                  \/ IndexedHistoricalDecisionStageOwnershipResidual(
                       initialContext, node))
      BY DEF IndexedHistoricalDecisionStageOwnershipResidual
    <2>10. IndexedHistoricalDecisionStageOwnershipResidual(
              initialContext, node)
              ~> IndexedHistoricalDecisionStageGoal(initialContext, node)
      BY <1>1, IndexedHistoricalDecisionStageOwnershipResidualObligation
         DEF IndexedHistoricalDecisionStageOwnershipResidualProperty
    <2>11. (\E rank \in 1..6:
              IndexedHistoricalDecisionStageAt(
                initialContext, node, rank))
              ~> IndexedHistoricalExactApplication(initialContext, node)
      BY <2>4, PTL
    <2>12. IndexedHistoricalDecisionStageGoal(initialContext, node)
              ~> IndexedHistoricalExactApplication(initialContext, node)
      BY <2>11, PTL DEF IndexedHistoricalDecisionStageGoal
    <2>13. IndexedHistoricalCertificateGoal(initialContext, node)
              ~> IndexedHistoricalExactApplication(initialContext, node)
      BY <2>9, <2>10, <2>12, PTL
         DEF IndexedHistoricalCertificateGoal
    <2>14. IndexedHistoricalRecoveryTargetOwned(initialContext, node)
              ~> IndexedHistoricalExactApplication(initialContext, node)
      BY <2>7, <2>8, <2>13, PTL
    <2>15. IndexedHistoricalRecoveryOpenGoal(initialContext, node)
              ~> IndexedHistoricalExactApplication(initialContext, node)
      BY <2>9, <2>10, <2>12, <2>14, PTL
         DEF IndexedHistoricalRecoveryOpenGoal
    <2>16. IndexedHistoricalRecoveryOpenable(initialContext, node)
              ~> IndexedHistoricalExactApplication(initialContext, node)
      BY <2>5, <2>6, <2>15, PTL
    <2>17. IndexedHistoricalRecoveryAuthorityReady(initialContext, node)
              ~> IndexedHistoricalExactApplication(initialContext, node)
      BY <2>14, <2>16, PTL
         DEF IndexedHistoricalRecoveryAuthorityReady
    <2>18. IndexedHistoricalExactApplication(initialContext, node)
              ~> HistoricalRecoveryComplete(initialContext, node)
      BY <2>2
         DEF IndexedHistoricalApplicationReceiptHandoffProperty
    <2> QED BY <2>17, <2>18, PTL
  <1> QED BY <1>1

THEOREM IndexedHistoricalServiceKernelsDischargeEntryCompletionAt ==
  /\ IndexedChainSpec
  /\ IndexedHistoricalCertificateRankProgressResidualProperty
  => \A initialContext \in AdmissibleContextRecords:
       IndexedHistoricalDecisionRankProgressAtContext(initialContext)
         => IndexedHistoricalRecoveryEntryCompletionAt(initialContext)
PROOF
  <1>1. ASSUME IndexedChainSpec,
              IndexedHistoricalCertificateRankProgressResidualProperty,
              NEW initialContext \in AdmissibleContextRecords,
              IndexedHistoricalDecisionRankProgressAtContext(initialContext),
              NEW node \in Responsive
         PROVE IndexedHistoricalRecoveryEntryGoal(initialContext, node)
                 ~> HistoricalRecoveryComplete(initialContext, node)
    <2>1. IndexedHistoricalRecoveryAuthorityReady(initialContext, node)
             ~> HistoricalRecoveryComplete(initialContext, node)
      BY <1>1,
         IndexedHistoricalServiceKernelsDischargeAuthorityReadyProgressAt
    <2>2. IndexedHistoricalExactApplication(initialContext, node)
             ~> HistoricalRecoveryComplete(initialContext, node)
      BY <1>1, IndexedChainSpecClosesHistoricalApplicationReceiptHandoff
         DEF IndexedHistoricalApplicationReceiptHandoffProperty
    <2>3. \A rank \in Nat:
             IndexedHistoricalDecisionStageAt(
               initialContext, node, rank)
               ~> IndexedHistoricalExactApplication(initialContext, node)
      BY <1>1, IndexedHistoricalDecisionRankConvergenceAtContext
    <2>4. IndexedHistoricalDecisionOwned(initialContext, node)
             => (IndexedHistoricalDecisionStageGoal(initialContext, node)
                  \/ IndexedHistoricalDecisionStageOwnershipResidual(
                       initialContext, node))
      BY DEF IndexedHistoricalDecisionStageOwnershipResidual
    <2>5. IndexedHistoricalDecisionStageOwnershipResidual(
             initialContext, node)
             ~> IndexedHistoricalDecisionStageGoal(initialContext, node)
      BY <1>1, IndexedHistoricalDecisionStageOwnershipResidualObligation
         DEF IndexedHistoricalDecisionStageOwnershipResidualProperty
    <2>6. (\E rank \in 1..6:
             IndexedHistoricalDecisionStageAt(
               initialContext, node, rank))
             ~> IndexedHistoricalExactApplication(initialContext, node)
      BY <2>3, PTL
    <2>7. IndexedHistoricalDecisionStageGoal(initialContext, node)
             ~> IndexedHistoricalExactApplication(initialContext, node)
      BY <2>6, PTL DEF IndexedHistoricalDecisionStageGoal
    <2>8. IndexedHistoricalDecisionOwned(initialContext, node)
             ~> IndexedHistoricalExactApplication(initialContext, node)
      BY <2>4, <2>5, <2>7, PTL
    <2>9. IndexedHistoricalRecoveryEntryGoal(initialContext, node)
             ~> (IndexedHistoricalExactApplication(initialContext, node)
                  \/ IndexedHistoricalRecoveryAuthorityReady(
                       initialContext, node))
      BY <2>8, PTL
         DEF IndexedHistoricalRecoveryEntryGoal,
             IndexedHistoricalRecoveryAuthorityReady
    <2> QED BY <2>1, <2>2, <2>9, PTL
  <1> QED BY <1>1
       DEF IndexedHistoricalRecoveryEntryCompletionAt

THEOREM IndexedHistoricalLowerAuthorityProgressGivesStrictAncestorAdvance ==
  \A targetContext \in AdmissibleContextRecords:
    /\ IndexedHistoricalRecoveryAuthorityProgressBelow(targetContext)
    /\ IndexedHistoricalRecoveryEntryCompletionBelow(targetContext)
    => IndexedStrictAncestorRecoveryAdvance(targetContext)
PROOF
  <1>1. ASSUME NEW targetContext \in AdmissibleContextRecords,
              IndexedHistoricalRecoveryAuthorityProgressBelow(targetContext),
              IndexedHistoricalRecoveryEntryCompletionBelow(targetContext),
              NEW blockHeight \in 0..targetContext.height,
              blockHeight < targetContext.height,
              NEW node \in Responsive
         PROVE HistoricalRecoveryOutstanding(
                   IndexedAncestorContext(targetContext, blockHeight), node)
                 ~> IndexedNodePastContext(
                      IndexedAncestorContext(targetContext, blockHeight),
                      node)
    <2>1. HistoricalRecoveryOutstanding(
             IndexedAncestorContext(targetContext, blockHeight), node)
             => (IndexedHistoricalRecoveryEntryGoal(
                   IndexedAncestorContext(targetContext, blockHeight), node)
                  \/ IndexedHistoricalRecoveryAuthorityAcquisitionResidual(
                       IndexedAncestorContext(
                         targetContext, blockHeight), node))
      BY DEF IndexedHistoricalRecoveryAuthorityAcquisitionResidual
    <2>2. IndexedHistoricalRecoveryAuthorityAcquisitionResidual(
             IndexedAncestorContext(targetContext, blockHeight), node)
             ~> IndexedHistoricalRecoveryEntryGoal(
                  IndexedAncestorContext(targetContext, blockHeight), node)
      BY <1>1
         DEF IndexedHistoricalRecoveryAuthorityProgressBelow
    <2>3. IndexedAncestorContext(targetContext, blockHeight)
             \in AdmissibleContextRecords
      BY <1>1, IndexedAdmissibleTargetHasAdmissibleAncestors
    <2>4. IndexedHistoricalRecoveryEntryGoal(
             IndexedAncestorContext(targetContext, blockHeight), node)
             ~> HistoricalRecoveryComplete(
                  IndexedAncestorContext(targetContext, blockHeight), node)
      BY <1>1
         DEF IndexedHistoricalRecoveryEntryCompletionBelow
    <2>5. IndexedAncestorContext(targetContext, blockHeight).height
             < MaxHeight
      BY <1>1, <2>3, Isa
         DEF IndexedAncestorContext,
             AdmissibleContextRecords, FrozenContextAdmissible,
             ContextRecords, Heights
    <2>6. HistoricalRecoveryComplete(
             IndexedAncestorContext(targetContext, blockHeight), node)
             => IndexedNodePastContext(
                  IndexedAncestorContext(targetContext, blockHeight), node)
      BY <2>5
         DEF HistoricalRecoveryComplete, IndexedNodePastContext
    <2> QED BY <2>1, <2>2, <2>4, <2>6, PTL
  <1> QED BY <1>1
       DEF IndexedStrictAncestorRecoveryAdvance

THEOREM IndexedHistoricalJointProgressBelowProjectsStrictAncestorInputs ==
  \A targetContext \in AdmissibleContextRecords:
    IndexedHistoricalRecoveryJointProgressBelow(targetContext)
      => /\ IndexedHistoricalRecoveryAuthorityProgressBelow(targetContext)
         /\ IndexedHistoricalRecoveryEntryCompletionBelow(targetContext)
BY DEF IndexedHistoricalRecoveryJointProgressBelow,
       IndexedHistoricalRecoveryJointProgressAt,
       IndexedHistoricalRecoveryAuthorityProgressBelow,
       IndexedHistoricalRecoveryAuthorityProgressAt,
       IndexedHistoricalRecoveryEntryCompletionBelow,
       IndexedHistoricalRecoveryEntryCompletionAt

(***************************************************************************
The successor module is imported in the non-circular direction.  Its proved
starvation result is definitionally the exact chain progress property consumed
by the strict-height induction below; no chain-liveness wrapper is imported.
***************************************************************************)

THEOREM IndexedChainSpecClosesSuccessorActivationForHistoricalInduction ==
  IndexedChainSpec => IndexedSuccessorActivationProgress
PROOF
  <1>1. IndexedChainSpec
           => SuccessorActivationStarvationFreedomProperty
    BY SuccessorActivationStarvationFreedomObligation
  <1>2. SuccessorActivationStarvationFreedomProperty
           <=> IndexedSuccessorActivationProgress
    BY SuccessorActivationStarvationMatchesChainProgress
  <1> QED BY <1>1, <1>2

THEOREM IndexedHistoricalStrictAncestorRecoveryClosesActivationAt ==
  /\ IndexedChainSpec
  /\ IndexedSuccessorActivationProgress
  => \A targetContext \in AdmissibleContextRecords:
       IndexedStrictAncestorRecoveryAdvance(targetContext)
         => \A node \in Responsive:
              IndexedHistoricalRecoveryAuthorityAcquisitionResidual(
                targetContext, node)
                ~> (IndexedHistoricalRecoveryEntryGoal(targetContext, node)
                     \/ /\ IndexedHistoricalRecoveryArchiveOwnerJoined(
                              targetContext)
                        /\ IndexedResponsiveActiveRosterAt(targetContext)
                        /\ IndexedHistoricalRecoveryAuthorityAcquisitionResidual(
                             targetContext, node))
PROOF
  <1>1. ASSUME IndexedChainSpec,
              IndexedSuccessorActivationProgress,
              NEW targetContext \in AdmissibleContextRecords,
              IndexedStrictAncestorRecoveryAdvance(targetContext),
              NEW node \in Responsive
         PROVE IndexedHistoricalRecoveryAuthorityAcquisitionResidual(
                   targetContext, node)
                 ~> (IndexedHistoricalRecoveryEntryGoal(targetContext, node)
                      \/ /\ IndexedHistoricalRecoveryArchiveOwnerJoined(
                               targetContext)
                         /\ IndexedResponsiveActiveRosterAt(targetContext)
                         /\ IndexedHistoricalRecoveryAuthorityAcquisitionResidual(
                              targetContext, node))
    <2>1. []IndexedCompositionInvariant
      BY <1>1, IndexedChainSpecEstablishesCompositionInvariant, PTL
    <2>2. [][IndexedChainNext]_IndexedChainVars
      BY <1>1, PTL DEF IndexedChainSpec
    <2>3. IndexedCompositionInvariant
             /\ IndexedHistoricalRecoveryAuthorityAcquisitionResidual(
                  targetContext, node)
             /\ [IndexedChainNext]_IndexedChainVars
             => \/ IndexedHistoricalRecoveryAuthorityAcquisitionResidual(
                      targetContext, node)'
                \/ IndexedHistoricalRecoveryEntryGoal(
                     targetContext, node)'
      BY <1>1, IndexedHistoricalAuthorityResidualPersistsOrEnters
    <2>4. IndexedHistoricalRecoveryAuthorityAcquisitionResidual(
             targetContext, node)
             => IndexedTargetJoined(targetContext)
      BY Isa
         DEF IndexedHistoricalRecoveryAuthorityAcquisitionResidual,
             HistoricalRecoveryOutstanding,
             IndexedTargetJoined, JoinedContexts
    <2>5. targetContext.height \in 0..targetContext.height
      BY <1>1, Isa
         DEF AdmissibleContextRecords, FrozenContextAdmissible,
             ContextRecords, Heights
    <2>6. IndexedTargetJoined(targetContext)
             ~> IndexedResponsiveHeightReached(targetContext.height)
      BY <1>1, <2>5,
         IndexedJoinedTargetReachesEveryAncestorFromStrictRecovery
    <2>7. IndexedHistoricalRecoveryArchiveOwner(targetContext)
             \in Responsive
      BY <1>1, IndexedHistoricalRecoveryArchiveOwnerIsResponsive
    <2>8. (IndexedTargetJoined(targetContext)
              /\ IndexedResponsiveHeightReached(targetContext.height))
             ~> IndexedHistoricalRecoveryArchiveOwner(targetContext)
                   \in joinedByContext[
                        IndexedAncestorContext(
                          targetContext, targetContext.height)]
      BY <1>1, <2>5, <2>7,
         IndexedReachedAncestorEventuallyJoinsResponsiveNode
    <2>9. IndexedAncestorContext(targetContext, targetContext.height)
             = targetContext
      BY <1>1, Isa
         DEF IndexedAncestorContext, AdmissibleContextRecords,
             FrozenContextAdmissible, ContextRecords, LineagesAt,
             Heights, ContextRecord
    <2>10. IndexedTargetJoined(targetContext)
              /\ [IndexedChainNext]_IndexedChainVars
              => IndexedTargetJoined(targetContext)'
      BY <1>1, IndexedTargetJoinedIsStable
    <2>11. IndexedTargetJoined(targetContext)
              ~> IndexedHistoricalRecoveryArchiveOwnerJoined(targetContext)
      BY <2>6, <2>8, <2>9, <2>10, PTL
         DEF IndexedHistoricalRecoveryArchiveOwnerJoined
    <2>12. IndexedTargetJoined(targetContext)
               ~> IndexedResponsiveActiveRosterAt(targetContext)
      BY <1>1,
         IndexedStrictAncestorRecoveryEventuallyActivatesResponsiveRoster
         DEF IndexedResponsiveActiveRosterAt
    <2>13. []( /\ IndexedHistoricalRecoveryArchiveOwnerJoined(targetContext)
               /\ IndexedResponsiveActiveRosterAt(targetContext)
                 => /\ IndexedHistoricalRecoveryArchiveOwnerJoined(
                          targetContext)'
                    /\ IndexedResponsiveActiveRosterAt(targetContext)')
      BY <1>1, <2>1, IndexedJoinedResponsiveActiveRosterIsStable,
         JoinedMembershipIsMonotone, PTL
         DEF IndexedHistoricalRecoveryArchiveOwnerJoined
    <2>14. IndexedTargetJoined(targetContext)
               ~> /\ IndexedHistoricalRecoveryArchiveOwnerJoined(
                        targetContext)
                   /\ IndexedResponsiveActiveRosterAt(targetContext)
      BY <2>11, <2>12, <2>13, PTL
    <2> QED BY <2>1, <2>2, <2>3, <2>4, <2>14, PTL
  <1> QED BY <1>1

(***************************************************************************
Ordinary Decision owners use one joined current runner after activation.

The exact historical stage already identifies a joined current responsive
voter.  Strict-ancestor recovery activates the frozen Responsive service
roster required by the executable SetGST guard.  Product fairness then sets
GST for that instance, the frozen stage either lowers meanwhile or exposes
the exact Decision service source, and the five-leaf local service corridor
reaches Apply.  No aggregate application theorem or current-height liveness
wrapper occurs in this dependency cone.
***************************************************************************)

THEOREM IndexedLiveStrictAncestorsCloseOrdinaryDecisionOwnerRanks ==
  IndexedChainSpec
    => \A initialContext \in AdmissibleContextRecords:
         IndexedStrictAncestorRecoveryAdvance(initialContext)
           => \A node \in Responsive, rank \in 1..6:
                IndexedHistoricalDecisionOrdinaryOwnerRankProgressAt(
                  initialContext, node, rank)
PROOF
  <1>1. ASSUME IndexedChainSpec,
              NEW initialContext \in AdmissibleContextRecords,
              IndexedStrictAncestorRecoveryAdvance(initialContext),
              NEW node \in Responsive,
              NEW rank \in 1..6
         PROVE IndexedHistoricalDecisionOrdinaryOwnerRankProgressAt(
                 initialContext, node, rank)
    <2> DEFINE owner ==
          /\ node \in IndexedAsync(initialContext)!
                       AsyncCurrentResponsiveVoters
          /\ IndexedHistoricalDecisionStageAt(
               initialContext, node, rank)
    <2> DEFINE goal ==
          IndexedHistoricalDecisionOrdinaryRankGoal(
            initialContext, node, rank)
    <2>1. []IndexedCompositionInvariant
      BY <1>1, IndexedChainSpecEstablishesCompositionInvariant, PTL
    <2>2. []IndexedDecisionWitnessSupport
      BY <1>1, IndexedChainSpecAlwaysDecisionWitnessSupport, PTL
    <2>3. []IndexedResponsiveRecoveryDormant
      BY <1>1, IndexedChainSpecKeepsResponsiveRecoveryDormant, PTL
    <2>4. [][IndexedChainNext]_IndexedChainVars
      BY <1>1, PTL DEF IndexedChainSpec
    <2>5. [](owner
              => /\ initialContext \in JoinedContexts
                 /\ IndexedNodeCurrentAt(initialContext, node))
      BY <1>1, <2>1,
         IndexedHistoricalDecisionOrdinaryOwnerHasJoinedCurrentRunner,
         PTL DEF owner
    <2>6. IndexedSuccessorActivationProgress
      BY <1>1,
         IndexedChainSpecClosesSuccessorActivationForHistoricalInduction
    <2>7. initialContext \in JoinedContexts
             ~> IndexedResponsiveActiveRosterAt(initialContext)
      BY <1>1, <2>6,
         IndexedStrictAncestorRecoveryEventuallyActivatesResponsiveRoster
         DEF IndexedTargetJoined, IndexedResponsiveActiveRosterAt
    <2>8. []( /\ IndexedCompositionInvariant
              /\ IndexedDecisionWitnessSupportAt(initialContext)
              /\ IndexedResponsiveRecoveryDormant
              /\ owner
              /\ [IndexedChainNext]_IndexedChainVars
                => \/ owner'
                   \/ goal')
      BY <1>1, <2>1, <2>2, <2>3, <2>4,
         IndexedHistoricalDecisionOrdinaryOwnerPersistsOrGoals,
         PTL DEF IndexedDecisionWitnessSupport, owner, goal
    <2>9. owner
             ~> (goal
                  \/ /\ IndexedResponsiveActiveRosterAt(initialContext)
                     /\ owner)
      BY <2>5, <2>7, <2>8, PTL
    <2>10. /\ IndexedResponsiveActiveRosterAt(initialContext)
            /\ owner
             ~> (goal
                  \/ /\ IndexedCore(initialContext, 7)
                     /\ owner)
      BY <1>1, <2>5, <2>8,
         IndexedChainSpecResponsiveActiveRosterEventuallySetsExactGst,
         PTL
    <2>11. []( /\ IndexedCompositionInvariant
              /\ IndexedDecisionWitnessSupportAt(initialContext)
              /\ IndexedResponsiveRecoveryDormant
              /\ IndexedCore(initialContext, 7)
              /\ owner
                => \E qc:
                     IndexedDecisionServiceWitness(initialContext)!
                       ExactDecisionServiceSource(node, qc))
      BY <1>1, <2>1, <2>2, <2>3,
         IndexedHistoricalDecisionOrdinaryStageHasExactServiceSourceAtGst,
         PTL DEF IndexedDecisionWitnessSupport, owner
    <2>12. IndexedDecisionServiceWitness(initialContext)!
              ExactDecisionStageServiceProperty(IndexedChainSpec)
      BY <1>1, IndexedChainSpecProvidesLocalExactDecisionStageService
         DEF IndexedLocalExactDecisionStageServiceProperty
    <2>13. \A qc:
              IndexedDecisionServiceWitness(initialContext)!
                ExactDecisionServiceSource(node, qc)
                ~> IndexedAsync(initialContext)!NodeHasApplication(node)
      BY <1>1, <2>12
         DEF IndexedDecisionServiceWitness!ExactDecisionStageServiceProperty,
             IndexedDecisionServiceWitness!NodeHasApplication,
             IndexedAsync!NodeHasApplication
    <2>14. ( /\ IndexedCore(initialContext, 7)
               /\ owner)
               ~> goal
      BY <2>1, <2>2, <2>3, <2>11, <2>13, PTL
         DEF goal, IndexedHistoricalDecisionOrdinaryRankGoal,
             IndexedHistoricalExactApplication
    <2> QED BY <2>9, <2>10, <2>14, PTL
         DEF IndexedHistoricalDecisionOrdinaryOwnerRankProgressAt,
             owner, goal
  <1> QED BY <1>1

THEOREM IndexedHistoricalAuthorityProgressAtHeightFromStrictAncestors ==
  /\ IndexedChainSpec
  /\ IndexedSuccessorActivationProgress
  /\ IndexedHistoricalRecoveryActivatedArchiveProducerResidualProperty
  /\ IndexedHistoricalRecoveryTypedArchiveEntryResidualProperty
  => \A targetContext \in AdmissibleContextRecords:
       /\ IndexedHistoricalRecoveryAuthorityProgressBelow(targetContext)
       /\ IndexedHistoricalRecoveryEntryCompletionBelow(targetContext)
       => IndexedHistoricalRecoveryAuthorityProgressAt(targetContext)
PROOF
  <1>1. ASSUME IndexedChainSpec,
              IndexedSuccessorActivationProgress,
              IndexedHistoricalRecoveryActivatedArchiveProducerResidualProperty,
              IndexedHistoricalRecoveryTypedArchiveEntryResidualProperty,
              NEW targetContext \in AdmissibleContextRecords,
              IndexedHistoricalRecoveryAuthorityProgressBelow(targetContext),
              IndexedHistoricalRecoveryEntryCompletionBelow(targetContext),
              NEW node \in Responsive
         PROVE IndexedHistoricalRecoveryAuthorityAcquisitionResidual(
                   targetContext, node)
                 ~> IndexedHistoricalRecoveryEntryGoal(
                      targetContext, node)
    <2>1. IndexedStrictAncestorRecoveryAdvance(targetContext)
      BY <1>1,
         IndexedHistoricalLowerAuthorityProgressGivesStrictAncestorAdvance
    <2>2. IndexedHistoricalRecoveryAuthorityAcquisitionResidual(
             targetContext, node)
             ~> (IndexedHistoricalRecoveryEntryGoal(targetContext, node)
                  \/ /\ IndexedHistoricalRecoveryArchiveOwnerJoined(
                           targetContext)
                     /\ IndexedResponsiveActiveRosterAt(targetContext)
                     /\ IndexedHistoricalRecoveryAuthorityAcquisitionResidual(
                          targetContext, node))
      BY <1>1, <2>1,
         IndexedHistoricalStrictAncestorRecoveryClosesActivationAt
    <2>3. /\ IndexedHistoricalRecoveryArchiveOwnerJoined(targetContext)
           /\ IndexedResponsiveActiveRosterAt(targetContext)
           /\ IndexedHistoricalRecoveryAuthorityAcquisitionResidual(
                targetContext, node)
             ~> (IndexedHistoricalRecoveryEntryGoal(targetContext, node)
                  \/ /\ IndexedHistoricalRecoveryTypedArchiveAuthority(
                           targetContext)
                     /\ IndexedHistoricalRecoveryAuthorityAcquisitionResidual(
                          targetContext, node))
      BY <1>1
         DEF IndexedHistoricalRecoveryActivatedArchiveProducerResidualProperty
    <2>4. /\ IndexedHistoricalRecoveryTypedArchiveAuthority(targetContext)
           /\ IndexedHistoricalRecoveryAuthorityAcquisitionResidual(
                targetContext, node)
             ~> IndexedHistoricalRecoveryEntryGoal(targetContext, node)
      BY <1>1
         DEF IndexedHistoricalRecoveryTypedArchiveEntryResidualProperty
    <2> QED BY <2>2, <2>3, <2>4, PTL
  <1> QED BY <1>1
       DEF IndexedHistoricalRecoveryAuthorityProgressAt

THEOREM IndexedHistoricalJointProgressAtHeightFromStrictAncestors ==
  /\ IndexedLiveChainSpec
  /\ IndexedLocalAdequateLeaderDecisionConvergenceProperty
  /\ IndexedHistoricalCertificateRankProgressResidualProperty
  /\ IndexedHistoricalDecisionTargetOwnerRankProgressProperty
  => \A targetContext \in AdmissibleContextRecords:
       IndexedHistoricalRecoveryJointProgressBelow(targetContext)
         => IndexedHistoricalRecoveryJointProgressAt(targetContext)
PROOF
  <1>1. ASSUME IndexedLiveChainSpec,
              IndexedLocalAdequateLeaderDecisionConvergenceProperty,
              IndexedHistoricalCertificateRankProgressResidualProperty,
              IndexedHistoricalDecisionTargetOwnerRankProgressProperty,
              NEW targetContext \in AdmissibleContextRecords,
              IndexedHistoricalRecoveryJointProgressBelow(targetContext)
         PROVE IndexedHistoricalRecoveryJointProgressAt(targetContext)
    <2>0. IndexedChainSpec
      BY <1>1, IndexedLiveChainSpecProjectsIndexedChainSpec
    <2>1. IndexedSuccessorActivationProgress
      BY <2>0,
         IndexedChainSpecClosesSuccessorActivationForHistoricalInduction
    <2>2. IndexedHistoricalRecoveryActivatedArchiveProducerResidualProperty
      BY <1>1,
         IndexedLiveChainSpecClosesActivatedArchiveProducerResidual
    <2>3. IndexedHistoricalRecoveryTypedArchiveEntryResidualProperty
      BY <1>1, IndexedLiveChainSpecClosesTypedArchiveEntryResidual
    <2>4. /\ IndexedHistoricalRecoveryAuthorityProgressBelow(targetContext)
           /\ IndexedHistoricalRecoveryEntryCompletionBelow(targetContext)
      BY <1>1,
         IndexedHistoricalJointProgressBelowProjectsStrictAncestorInputs
    <2>5. IndexedStrictAncestorRecoveryAdvance(targetContext)
      BY <1>1, <2>4,
         IndexedHistoricalLowerAuthorityProgressGivesStrictAncestorAdvance
    <2>6. \A node \in Responsive, rank \in 1..6:
             IndexedHistoricalDecisionOrdinaryOwnerRankProgressAt(
               targetContext, node, rank)
      BY <2>0, <2>5,
         IndexedLiveStrictAncestorsCloseOrdinaryDecisionOwnerRanks
    <2>7. \A node \in Responsive, rank \in 1..6:
             IndexedHistoricalDecisionTargetOwnerRankProgressAt(
               targetContext, node, rank)
      BY <1>1
         DEF IndexedHistoricalDecisionTargetOwnerRankProgressProperty
    <2>8. IndexedHistoricalDecisionRankProgressAtContext(targetContext)
      BY <1>1, <2>6, <2>7,
         IndexedHistoricalDecisionOwnerClassesCloseRankProgressAtContext
    <2>9. IndexedHistoricalRecoveryEntryCompletionAt(targetContext)
      BY <1>1, <2>0, <2>8,
         IndexedHistoricalServiceKernelsDischargeEntryCompletionAt
    <2>10. IndexedHistoricalRecoveryAuthorityProgressAt(targetContext)
      BY <1>1, <2>0, <2>1, <2>2, <2>3, <2>4,
         IndexedHistoricalAuthorityProgressAtHeightFromStrictAncestors
    <2> QED BY <2>8, <2>9, <2>10
         DEF IndexedHistoricalRecoveryJointProgressAt
  <1> QED BY <1>1

THEOREM IndexedHistoricalJointProgressStartsAtHeightZero ==
  /\ IndexedLiveChainSpec
  /\ IndexedLocalAdequateLeaderDecisionConvergenceProperty
  /\ IndexedHistoricalCertificateRankProgressResidualProperty
  /\ IndexedHistoricalDecisionTargetOwnerRankProgressProperty
  => IndexedHistoricalRecoveryJointProgressThroughHeight(0)
PROOF
  <1>1. ASSUME IndexedLiveChainSpec,
              IndexedLocalAdequateLeaderDecisionConvergenceProperty,
              IndexedHistoricalCertificateRankProgressResidualProperty,
              IndexedHistoricalDecisionTargetOwnerRankProgressProperty,
              NEW initialContext \in AdmissibleContextRecords,
              initialContext.height <= 0
         PROVE IndexedHistoricalRecoveryJointProgressAt(initialContext)
    <2>1. initialContext.height = 0
      BY <1>1, Isa
         DEF AdmissibleContextRecords, FrozenContextAdmissible,
             ContextRecords, Heights
    <2>2. IndexedHistoricalRecoveryJointProgressBelow(initialContext)
      BY <1>1, <2>1, Isa
         DEF IndexedHistoricalRecoveryJointProgressBelow
    <2> QED BY <1>1, <2>2,
         IndexedHistoricalJointProgressAtHeightFromStrictAncestors
  <1> QED BY <1>1
       DEF IndexedHistoricalRecoveryJointProgressThroughHeight

THEOREM IndexedHistoricalJointProgressAdvancesOneHeight ==
  /\ IndexedLiveChainSpec
  /\ IndexedLocalAdequateLeaderDecisionConvergenceProperty
  /\ IndexedHistoricalCertificateRankProgressResidualProperty
  /\ IndexedHistoricalDecisionTargetOwnerRankProgressProperty
  => \A limit \in Nat:
       IndexedHistoricalRecoveryJointProgressThroughHeight(limit)
         => IndexedHistoricalRecoveryJointProgressThroughHeight(limit + 1)
PROOF
  <1>1. ASSUME IndexedLiveChainSpec,
              IndexedLocalAdequateLeaderDecisionConvergenceProperty,
              IndexedHistoricalCertificateRankProgressResidualProperty,
              IndexedHistoricalDecisionTargetOwnerRankProgressProperty,
              NEW limit \in Nat,
              IndexedHistoricalRecoveryJointProgressThroughHeight(limit),
              NEW initialContext \in AdmissibleContextRecords,
              initialContext.height <= limit + 1
         PROVE IndexedHistoricalRecoveryJointProgressAt(initialContext)
    <2>1. CASE initialContext.height <= limit
      BY <1>1, <2>1
         DEF IndexedHistoricalRecoveryJointProgressThroughHeight
    <2>2. CASE initialContext.height > limit
      <3>1. initialContext.height = limit + 1
        BY <1>1, <2>2, SMT
      <3>2. IndexedHistoricalRecoveryJointProgressBelow(initialContext)
        PROOF
          <4>1. ASSUME NEW blockHeight \in 0..initialContext.height,
                        blockHeight < initialContext.height
                 PROVE IndexedHistoricalRecoveryJointProgressAt(
                           IndexedAncestorContext(
                             initialContext, blockHeight))
            <5>1. IndexedAncestorContext(initialContext, blockHeight)
                     \in AdmissibleContextRecords
              BY <1>1, <4>1,
                 IndexedAdmissibleTargetHasAdmissibleAncestors
            <5>2. IndexedAncestorContext(
                     initialContext, blockHeight).height <= limit
              BY <3>1, <4>1, SMT DEF IndexedAncestorContext
            <5> QED BY <1>1, <5>1, <5>2
                 DEF IndexedHistoricalRecoveryJointProgressThroughHeight
          <4> QED BY <4>1
               DEF IndexedHistoricalRecoveryJointProgressBelow
      <3> QED BY <1>1, <3>2,
           IndexedHistoricalJointProgressAtHeightFromStrictAncestors
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1
       DEF IndexedHistoricalRecoveryJointProgressThroughHeight

THEOREM IndexedHistoricalStrictHeightMutualInductionClosesJointProgress ==
  /\ IndexedLiveChainSpec
  /\ IndexedLocalAdequateLeaderDecisionConvergenceProperty
  /\ IndexedHistoricalCertificateRankProgressResidualProperty
  /\ IndexedHistoricalDecisionTargetOwnerRankProgressProperty
  => IndexedHistoricalRecoveryJointProgressProperty
PROOF
  <1>1. ASSUME IndexedLiveChainSpec,
              IndexedLocalAdequateLeaderDecisionConvergenceProperty,
              IndexedHistoricalCertificateRankProgressResidualProperty,
              IndexedHistoricalDecisionTargetOwnerRankProgressProperty
         PROVE IndexedHistoricalRecoveryJointProgressProperty
    <2> DEFINE P(limit) ==
           IndexedHistoricalRecoveryJointProgressThroughHeight(limit)
    <2>1. P(0)
      BY <1>1, IndexedHistoricalJointProgressStartsAtHeightZero DEF P
    <2>2. \A limit \in Nat: P(limit) => P(limit + 1)
      BY <1>1, IndexedHistoricalJointProgressAdvancesOneHeight DEF P
    <2>3. \A limit \in Nat: P(limit)
      BY <2>1, <2>2, NatInduction
    <2>4. P(MaxHeight)
      BY <2>3, Isa DEF ModelConfiguration
    <2>5. ASSUME NEW initialContext \in AdmissibleContextRecords
           PROVE IndexedHistoricalRecoveryJointProgressAt(initialContext)
      <3>1. initialContext.height <= MaxHeight
        BY <2>5, Isa
           DEF AdmissibleContextRecords, FrozenContextAdmissible,
               ContextRecords, Heights
      <3> QED BY <2>4, <3>1
           DEF P, IndexedHistoricalRecoveryJointProgressThroughHeight
    <2> QED BY <2>5
         DEF IndexedHistoricalRecoveryJointProgressProperty
  <1> QED BY <1>1

THEOREM IndexedHistoricalJointProgressProjectsReleaseProperties ==
  IndexedHistoricalRecoveryJointProgressProperty
    => /\ IndexedHistoricalRecoveryAuthorityAcquisitionResidualProperty
       /\ IndexedHistoricalRecoveryEntryCompletionProperty
       /\ IndexedHistoricalDecisionRankProgressResidualProperty
BY Isa
   DEF IndexedHistoricalRecoveryJointProgressProperty,
       IndexedHistoricalRecoveryJointProgressAt,
       IndexedHistoricalRecoveryAuthorityProgressAt,
       IndexedHistoricalRecoveryEntryCompletionAt,
       IndexedHistoricalDecisionRankProgressAtContext,
       IndexedHistoricalRecoveryAuthorityAcquisitionResidualProperty,
       IndexedHistoricalRecoveryEntryCompletionProperty,
       IndexedHistoricalDecisionRankProgressResidualProperty,
       IndexedHistoricalDecisionFetchBodyResidualProperty,
       IndexedHistoricalDecisionCertifiedRequestResidualProperty,
       IndexedHistoricalDecisionFetchCertifiedBodyResidualProperty,
       IndexedHistoricalDecisionStoreBodyResidualProperty,
       IndexedHistoricalDecisionValidateBodyResidualProperty,
       IndexedHistoricalDecisionApplyResidualProperty

THEOREM IndexedHistoricalStrictHeightServiceCompositionClosesAuthority ==
  /\ IndexedLiveChainSpec
  /\ IndexedLocalAdequateLeaderDecisionConvergenceProperty
  /\ IndexedHistoricalCertificateRankProgressResidualProperty
  /\ IndexedHistoricalDecisionTargetOwnerRankProgressProperty
  => IndexedHistoricalRecoveryAuthorityAcquisitionResidualProperty
BY IndexedHistoricalStrictHeightMutualInductionClosesJointProgress,
   IndexedHistoricalJointProgressProjectsReleaseProperties

THEOREM IndexedHistoricalStrictHeightServiceCompositionClosesDecisionRank ==
  /\ IndexedLiveChainSpec
  /\ IndexedLocalAdequateLeaderDecisionConvergenceProperty
  /\ IndexedHistoricalCertificateRankProgressResidualProperty
  /\ IndexedHistoricalDecisionTargetOwnerRankProgressProperty
  => IndexedHistoricalDecisionRankProgressResidualProperty
BY IndexedHistoricalStrictHeightMutualInductionClosesJointProgress,
   IndexedHistoricalJointProgressProjectsReleaseProperties

THEOREM IndexedHistoricalStrictHeightServiceCompositionClosesActivationPrefix ==
  /\ IndexedLiveChainSpec
  /\ IndexedLocalAdequateLeaderDecisionConvergenceProperty
  /\ IndexedHistoricalCertificateRankProgressResidualProperty
  /\ IndexedHistoricalDecisionTargetOwnerRankProgressProperty
  => IndexedHistoricalRecoveryActivationPrefixResidualProperty
BY IndexedHistoricalStrictHeightServiceCompositionClosesAuthority, PTL
   DEF IndexedHistoricalRecoveryAuthorityAcquisitionResidualProperty,
       IndexedHistoricalRecoveryActivationPrefixResidualProperty

IndexedHistoricalRecoveryOrdinaryAuthorityResidualProperties ==
  /\ IndexedHistoricalRecoveryActivationPrefixResidualProperty
  /\ IndexedHistoricalRecoveryActivatedArchiveProducerResidualProperty
  /\ IndexedHistoricalRecoveryTypedArchiveEntryResidualProperty

THEOREM IndexedHistoricalRecoveryOrdinaryAuthorityResidualReduction ==
  IndexedHistoricalRecoveryOrdinaryAuthorityResidualProperties
    => IndexedHistoricalRecoveryAuthorityAcquisitionResidualProperty
PROOF
  <1>1. ASSUME
          IndexedHistoricalRecoveryOrdinaryAuthorityResidualProperties,
          NEW initialContext \in AdmissibleContextRecords,
          NEW node \in Responsive
         PROVE IndexedHistoricalRecoveryAuthorityAcquisitionResidual(
                 initialContext, node)
                 ~> IndexedHistoricalRecoveryEntryGoal(
                      initialContext, node)
    <2>1. IndexedHistoricalRecoveryAuthorityAcquisitionResidual(
             initialContext, node)
             ~> (IndexedHistoricalRecoveryEntryGoal(
                  initialContext, node)
                  \/ /\ IndexedHistoricalRecoveryArchiveOwnerJoined(
                           initialContext)
                     /\ IndexedHistoricalRecoveryAuthorityAcquisitionResidual(
                          initialContext, node))
      BY <1>1
         DEF IndexedHistoricalRecoveryOrdinaryAuthorityResidualProperties,
             IndexedHistoricalRecoveryActivationPrefixResidualProperty
    <2>2. /\ IndexedHistoricalRecoveryArchiveOwnerJoined(initialContext)
           /\ IndexedHistoricalRecoveryAuthorityAcquisitionResidual(
                initialContext, node)
             ~> (IndexedHistoricalRecoveryEntryGoal(
                   initialContext, node)
                  \/ /\ IndexedHistoricalRecoveryTypedArchiveAuthority(
                           initialContext)
                     /\ IndexedHistoricalRecoveryAuthorityAcquisitionResidual(
                          initialContext, node))
      BY <1>1
         DEF IndexedHistoricalRecoveryOrdinaryAuthorityResidualProperties,
             IndexedHistoricalRecoveryActivatedArchiveProducerResidualProperty
    <2>3. /\ IndexedHistoricalRecoveryTypedArchiveAuthority(initialContext)
           /\ IndexedHistoricalRecoveryAuthorityAcquisitionResidual(
                initialContext, node)
             ~> IndexedHistoricalRecoveryEntryGoal(initialContext, node)
      BY <1>1
         DEF IndexedHistoricalRecoveryOrdinaryAuthorityResidualProperties,
             IndexedHistoricalRecoveryTypedArchiveEntryResidualProperty
    <2> QED BY <2>1, <2>2, <2>3, PTL
  <1> QED BY <1>1
       DEF IndexedHistoricalRecoveryAuthorityAcquisitionResidualProperty

(***************************************************************************
Complete residual inventory and PTL reduction.

The ordinary-consensus authority prefix is a derived strict-height
composition.  It consumes the proved successor-activation theorem, one fixed
responsive archive owner, the generic local adequate-leader Decision kernel,
the five exact Decision service leaves, and both historical service-rank
properties.  No aggregate application, one-height, or height-liveness wrapper
appears in the authority dependency cone.

There are three direct temporal-kernel groups in this leaf:

  1. the packet/Candidate/Serve finite producer episode;
  2. Commit request emission, request ingress, and response admission;
  3. historical Decision request emission, request ingress, and response
     admission.

Exact historical Decision-stage ownership exposure is closed above as an
indexed safety invariant.  It therefore needs no scheduler fairness premise.

The broad first source cannot be closed by historical fairness.  In the
reachable genesis state every responsive node is joined at height zero, while
there is no Decision, application, recovery target, or applied archive:
`HistoricalRecoveryOutstanding` holds and every historical fair action is
disabled.  The sound caller decomposition is therefore one joined current
voter's exact Decision service until an exact durable applied source exists,
followed by the Open theorem and historical-only ranks in this module.
Assuming aggregate application or indexed height liveness here would be
circular.

Exact Open and the application receipt handoff are proved above from
`IndexedChainSpec`.  The certificate residual is split into historical
discovery, request packet/archive Serve/ordinary-I/O service, response packet
import, and target-runner Decision.  Indexed Stage 2..6 service, historical
candidate starvation, and the DeliverQC/BeginDecision/PersistDecision tail are
closed.  Fixed-clock non-packet service is also derived from
`IndexedChainSpec`; the target-local QcAt/Decision-WAL entry handoffs are now
derived separately from the production causal-origin guard and target-neutral
lineage invariant.  Commit request-fanout completeness and exact applied-
archive route availability are product invariants, so the remaining
certificate seam contains the packet-local producer episode and the three
exact Commit transport kernels;
`IndexedHistoricalCertificateRemainingCorridorProperty` names their rank
composition.  For a historical target, Decision ranks 6, 4, 3, 2, and 1 are
closed from the indexed Candidate aggregate and rank 5 is the
certified-request route/body-service corridor.  Exact Decision request-fanout
completeness is also a product invariant, leaving its three physical-owner
kernels.  The rank-source/GST bridge and ordinary-I/O Serve response are
proved above.  The ordinary current-voter branch is
now derived by the strict-height mutual induction above: completed strict
ancestors join one frozen responsive archive owner, and the local exact
Decision corridor then applies that owner's Decision.  No item in this
inventory assumes
`IndexedExactHistoricalRecoveryProgress`, aggregate application liveness, or
the local exact Decision stage service property as a premise.
***************************************************************************)

(***************************************************************************
Exact historical-recovery release declarations.

The physical certificate wrapper and exact historical-target
CertifiedRequest corridor are deductive below.  Their proofs consume the
source-local fixed-clock packet episode and the six exact transport kernels;
the ordinary-owner branch additionally consumes strict-ancestor activation
of the executable Responsive service roster.  Decision-stage ownership, its
executor-class split, the combined Decision-rank property, and authority
acquisition are proof-bearing compositions over those providers.

No declaration in this section assumes indexed height liveness, aggregate
application liveness, or its own conclusion.  Bounded install-generation
exhaustion remains a TLC diagnostic, not a temporal premise.
***************************************************************************)

(***************************************************************************
Certificate-rank non-circularity boundary.

Rank 1 requires an exact target-local QcAt receipt, a non-rebroadcast Decision
WAL, or an exact current-consumer protected owner with production class and
CommitQC evidence/item/origin lineage.  Membership in the `AsyncCandidateSet`
type carrier, a stale scheduled occurrence, or an unrelated same-round command
cannot fabricate imported certificate ownership.

The rank theorem is reduced to
`IndexedHistoricalCertificateRemainingCorridorProperty`:

  * rank 4 uses discovery-clock progress through the overdue-packet corridor.
    Local node/I/O service, Tick, the Candidate/Serve identity bridge, and the
    packet-local finite non-descent episode are all closed from exact product
    fairness and frozen finite ranks;
  * ranks 3 and 2 use exact request/response retention through
    retransmission, historical packet admission, archive Serve/ordinary I/O,
    and response admission; and
  * rank 1 excludes append-only QcEnvelope history.  Its exact command arm is
    closed immediately.  The received-QC and non-rebroadcast Decision-WAL
    arms are closed separately by the target-neutral lineage invariant; once
    either exposes an exact Candidate, the existing DeliverQC, BeginDecision,
    and PersistDecision tail is already closed.

The Stage 2..6 and Candidate tail prerequisites are not assumptions.  The
proof below imports only the source-local fixed-clock and Commit physical
providers, never target-to-Decision or this wrapper itself.
***************************************************************************)

THEOREM IndexedHistoricalCertificateRankProgressResidualObligation ==
  IndexedLiveChainSpec
    => IndexedHistoricalCertificateRankProgressResidualProperty
PROOF
  <1>1. ASSUME IndexedLiveChainSpec
         PROVE IndexedHistoricalCertificateRankProgressResidualProperty
    <2>1. IndexedChainSpec
      BY <1>1, IndexedLiveChainSpecProjectsIndexedChainSpec
    <2>2. IndexedHistoricalFixedClockPacketRemainingTemporalResidual
      BY <2>1,
         IndexedChainSpecClosesHistoricalFixedClockPacketRemainingResidual
    <2>3. IndexedHistoricalCommitTransportResidualKernelProperties
      BY <2>1,
         IndexedChainSpecClosesSixHistoricalPhysicalTransportKernels
         DEF IndexedHistoricalCommitTransportResidualKernelProperties
    <2>4. IndexedHistoricalCertificatePhysicalResidualKernels
      BY <2>2, <2>3
         DEF IndexedHistoricalCertificatePhysicalResidualKernels
    <2> QED BY <2>1, <2>4,
         IndexedHistoricalCertificatePhysicalKernelsCloseRankResidual
  <1> QED BY <1>1

(***************************************************************************
Decision-rank non-circularity boundary.

`IndexedChainSpecClosesHistoricalDecisionTargetCandidateRankResiduals` closes
ranks 6, 4, 3, 2, and 1 only for an exact historical target.  By
`IndexedHistoricalDecisionTargetRankResidualSplitsAtCertifiedRequest`, that
branch reduces to the rank-5 active CertifiedRequest corridor, now supplied
by the exact Decision transport provider below.  The strict-height mutual
induction closes the separate ordinary current-voter branch and recombines it
with the target branch at each context.  Neither branch borrows indexed
height liveness, and the target branch may not treat request replenishment as
progress.
***************************************************************************)

THEOREM IndexedHistoricalDecisionTargetCertifiedRequestResidualObligation ==
  IndexedLiveChainSpec
    => IndexedHistoricalDecisionTargetCertifiedRequestResidualProperty
PROOF
  <1>1. ASSUME IndexedLiveChainSpec
         PROVE IndexedHistoricalDecisionTargetCertifiedRequestResidualProperty
    <2>1. IndexedChainSpec
      BY <1>1, IndexedLiveChainSpecProjectsIndexedChainSpec
    <2>2. IndexedHistoricalDecisionTransportResidualKernelProperties
      BY <2>1,
         IndexedChainSpecClosesSixHistoricalPhysicalTransportKernels
         DEF IndexedHistoricalDecisionTransportResidualKernelProperties
    <2>3. IndexedHistoricalDecisionCertifiedBodyTransportLeafProperty
      BY <2>1, <2>2,
         IndexedHistoricalDecisionTransportKernelsCloseExactLeaf
    <2> QED BY <2>1, <2>3,
         IndexedHistoricalDecisionTransportLeafClosesTargetRankFive
  <1> QED BY <1>1

THEOREM IndexedHistoricalDecisionTargetOwnerRankProgressObligation ==
  IndexedLiveChainSpec
    => IndexedHistoricalDecisionTargetOwnerRankProgressProperty
PROOF
  <1>1. ASSUME IndexedLiveChainSpec
         PROVE IndexedHistoricalDecisionTargetOwnerRankProgressProperty
    <2>1. IndexedChainSpec
      BY <1>1, IndexedLiveChainSpecProjectsIndexedChainSpec
    <2>2. IndexedHistoricalDecisionTargetCertifiedRequestResidualProperty
      BY <1>1,
         IndexedHistoricalDecisionTargetCertifiedRequestResidualObligation
    <2> QED BY <2>1, <2>2,
         IndexedHistoricalDecisionTargetCertifiedRequestClosesTargetRank
  <1> QED BY <1>1

THEOREM IndexedHistoricalDecisionRankProgressResidualObligation ==
  /\ IndexedLiveChainSpec
  /\ IndexedLocalAdequateLeaderDecisionConvergenceProperty
    => IndexedHistoricalDecisionRankProgressResidualProperty
PROOF
  <1>1. ASSUME IndexedLiveChainSpec,
              IndexedLocalAdequateLeaderDecisionConvergenceProperty
         PROVE IndexedHistoricalDecisionRankProgressResidualProperty
    <2>1. IndexedHistoricalCertificateRankProgressResidualProperty
      BY <1>1, IndexedHistoricalCertificateRankProgressResidualObligation
    <2>2. IndexedHistoricalDecisionTargetOwnerRankProgressProperty
      BY <1>1,
         IndexedHistoricalDecisionTargetOwnerRankProgressObligation
    <2> QED BY <1>1, <2>1, <2>2,
         IndexedHistoricalStrictHeightServiceCompositionClosesDecisionRank
  <1> QED BY <1>1

(***************************************************************************
Authority-acquisition closure.

This wrapper is deliberately below the certificate and target-owner rank
compositions.  The strict-height mutual induction derives context-local entry
completion and consumes the already-proved successor-activation theorem.  Its
typed-archive producer joins one deterministic frozen-roster owner, consumes
the generic local adequate-leader Decision kernel, and then uses the five
exact Decision service leaves to reach local Apply.  The dependency cone
imports no aggregate application, one-height, height, or historical-recovery
progress theorem.
***************************************************************************)

THEOREM IndexedHistoricalRecoveryAuthorityAcquisitionResidualObligation ==
  /\ IndexedLiveChainSpec
  /\ IndexedLocalAdequateLeaderDecisionConvergenceProperty
    => IndexedHistoricalRecoveryAuthorityAcquisitionResidualProperty
PROOF
  <1>1. ASSUME IndexedLiveChainSpec,
              IndexedLocalAdequateLeaderDecisionConvergenceProperty
         PROVE IndexedHistoricalRecoveryAuthorityAcquisitionResidualProperty
    <2>1. IndexedHistoricalCertificateRankProgressResidualProperty
      BY <1>1, IndexedHistoricalCertificateRankProgressResidualObligation
    <2>2. IndexedHistoricalDecisionTargetOwnerRankProgressProperty
      BY <1>1,
         IndexedHistoricalDecisionTargetOwnerRankProgressObligation
    <2> QED BY <1>1, <2>1, <2>2,
         IndexedHistoricalStrictHeightServiceCompositionClosesAuthority
  <1> QED BY <1>1

THEOREM IndexedHistoricalReleaseResidualsDischargeExactProgress ==
  /\ IndexedLiveChainSpec
  /\ IndexedLocalAdequateLeaderDecisionConvergenceProperty
    => IndexedExactHistoricalRecoveryProgress
PROOF
  <1>1. ASSUME IndexedLiveChainSpec,
              IndexedLocalAdequateLeaderDecisionConvergenceProperty
         PROVE IndexedExactHistoricalRecoveryProgress
    <2>1. IndexedChainSpec
      BY <1>1, IndexedLiveChainSpecProjectsIndexedChainSpec
    <2>2. IndexedHistoricalCertificateRankProgressResidualProperty
      BY <1>1, IndexedHistoricalCertificateRankProgressResidualObligation
    <2>3. IndexedHistoricalDecisionRankProgressResidualProperty
      BY <1>1, IndexedHistoricalDecisionRankProgressResidualObligation
    <2>4. IndexedHistoricalRecoveryAuthorityAcquisitionResidualProperty
      BY <1>1,
         IndexedHistoricalRecoveryAuthorityAcquisitionResidualObligation
    <2>5. IndexedHistoricalRecoveryEntryCompletionProperty
      BY <2>1, <2>2, <2>3,
         IndexedHistoricalServiceKernelsDischargeEntryCompletion
    <2>6. ASSUME NEW initialContext \in AdmissibleContextRecords,
                  NEW node \in Responsive
           PROVE HistoricalRecoveryOutstanding(initialContext, node)
                   ~> HistoricalRecoveryComplete(initialContext, node)
      <3>1. HistoricalRecoveryOutstanding(initialContext, node)
               => (IndexedHistoricalRecoveryEntryGoal(
                     initialContext, node)
                    \/ IndexedHistoricalRecoveryAuthorityAcquisitionResidual(
                         initialContext, node))
        BY DEF IndexedHistoricalRecoveryAuthorityAcquisitionResidual
      <3>2. IndexedHistoricalRecoveryAuthorityAcquisitionResidual(
               initialContext, node)
               ~> IndexedHistoricalRecoveryEntryGoal(
                    initialContext, node)
        BY <2>4
           DEF IndexedHistoricalRecoveryAuthorityAcquisitionResidualProperty
      <3>3. IndexedHistoricalRecoveryEntryGoal(initialContext, node)
               ~> HistoricalRecoveryComplete(initialContext, node)
        BY <2>5 DEF IndexedHistoricalRecoveryEntryCompletionProperty
      <3> QED BY <3>1, <3>2, <3>3, PTL
    <2> QED BY <2>6 DEF IndexedExactHistoricalRecoveryProgress
  <1> QED BY <1>1

THEOREM IndexedHistoricalFixedDeadlineDisseminationAndExposureDischargeExactProgress ==
  /\ IndexedLiveChainSpec
  /\ IndexedLocalAdequateLeaderFixedDeadlineAndResponsiveDisseminationProperty
  /\ IndexedLocalAdequateLeaderFreshSelfCorridorExposureProperty
    => IndexedExactHistoricalRecoveryProgress
BY IndexedAdequateLeaderFixedDeadlineDisseminationAndExposureSupplyLocalConvergence,
   IndexedHistoricalReleaseResidualsDischargeExactProgress

IndexedHistoricalRecoveryTemporalResidualKernels ==
  /\ IndexedHistoricalFixedClockPacketRemainingTemporalResidual
  /\ IndexedHistoricalCommitTransportResidualKernelProperties
  /\ IndexedHistoricalDecisionTransportResidualKernelProperties

THEOREM IndexedHistoricalRecoveryResidualKernelsDischargeExactProgress ==
  /\ IndexedLiveChainSpec
  /\ IndexedLocalAdequateLeaderDecisionConvergenceProperty
  /\ IndexedHistoricalRecoveryTemporalResidualKernels
    => IndexedExactHistoricalRecoveryProgress
PROOF
  <1>1. ASSUME IndexedLiveChainSpec,
              IndexedLocalAdequateLeaderDecisionConvergenceProperty,
              IndexedHistoricalRecoveryTemporalResidualKernels
         PROVE IndexedExactHistoricalRecoveryProgress
    <2>0. IndexedChainSpec
      BY <1>1, IndexedLiveChainSpecProjectsIndexedChainSpec
    <2>1. IndexedHistoricalCertificatePhysicalResidualKernels
      BY <1>1
         DEF IndexedHistoricalRecoveryTemporalResidualKernels,
             IndexedHistoricalCertificatePhysicalResidualKernels
    <2>2. IndexedHistoricalCertificateRankProgressResidualProperty
      BY <2>0, <2>1,
         IndexedHistoricalCertificatePhysicalKernelsCloseRankResidual
    <2>3. IndexedHistoricalDecisionTargetOwnerRankProgressProperty
      BY <1>1, <2>0,
         IndexedHistoricalDecisionPhysicalKernelsCloseTargetRank
         DEF IndexedHistoricalRecoveryTemporalResidualKernels
    <2>4. IndexedHistoricalRecoveryAuthorityAcquisitionResidualProperty
      BY <1>1, <2>2, <2>3,
         IndexedHistoricalStrictHeightServiceCompositionClosesAuthority
         DEF IndexedHistoricalRecoveryTemporalResidualKernels
    <2>5. IndexedHistoricalRecoveryOpenTargetResidualProperty
      BY <2>0, IndexedChainSpecClosesHistoricalOpenTarget
    <2>6. IndexedHistoricalDecisionStageOwnershipResidualProperty
      BY <2>0, IndexedHistoricalDecisionStageOwnershipResidualObligation
    <2>7. IndexedHistoricalDecisionRankProgressResidualProperty
      BY <1>1, <2>2, <2>3,
         IndexedHistoricalStrictHeightServiceCompositionClosesDecisionRank
         DEF IndexedHistoricalRecoveryTemporalResidualKernels
    <2>8. IndexedHistoricalApplicationReceiptHandoffProperty
      BY <2>0, IndexedChainSpecClosesHistoricalApplicationReceiptHandoff
    <2>9. \A initialContext \in AdmissibleContextRecords,
              node \in Responsive, rank \in Nat:
             IndexedHistoricalCertificateStageAt(
               initialContext, node, rank)
               ~> IndexedHistoricalCertificateGoal(
                    initialContext, node)
      BY <2>2, IndexedHistoricalCertificateRankConvergence
    <2>10. \A initialContext \in AdmissibleContextRecords,
              node \in Responsive, rank \in Nat:
             IndexedHistoricalDecisionStageAt(
               initialContext, node, rank)
               ~> IndexedHistoricalExactApplication(
                    initialContext, node)
      BY <2>7, IndexedHistoricalDecisionRankConvergence
    <2>11. ASSUME NEW initialContext \in AdmissibleContextRecords,
                  NEW node \in Responsive
           PROVE HistoricalRecoveryOutstanding(initialContext, node)
                   ~> HistoricalRecoveryComplete(
                        initialContext, node)
      <3>1. HistoricalRecoveryOutstanding(initialContext, node)
               => (IndexedHistoricalRecoveryEntryGoal(
                     initialContext, node)
                    \/ IndexedHistoricalRecoveryAuthorityAcquisitionResidual(
                         initialContext, node))
        BY DEF IndexedHistoricalRecoveryAuthorityAcquisitionResidual
      <3>2. IndexedHistoricalRecoveryAuthorityAcquisitionResidual(
               initialContext, node)
               ~> IndexedHistoricalRecoveryEntryGoal(
                    initialContext, node)
        BY <2>4
           DEF IndexedHistoricalRecoveryAuthorityAcquisitionResidualProperty
      <3>3. IndexedHistoricalRecoveryOpenable(initialContext, node)
               => (IndexedHistoricalRecoveryOpenResidual(
                     initialContext, node)
                    \/ IndexedHistoricalExactApplication(
                         initialContext, node)
                    \/ IndexedHistoricalDecisionOwned(
                         initialContext, node)
                    \/ IndexedHistoricalRecoveryTargetOwned(
                         initialContext, node))
        BY DEF IndexedHistoricalRecoveryOpenResidual
      <3>4. IndexedHistoricalRecoveryOpenResidual(initialContext, node)
               ~> (IndexedHistoricalExactApplication(
                     initialContext, node)
                    \/ IndexedHistoricalDecisionOwned(
                         initialContext, node)
                    \/ IndexedHistoricalRecoveryTargetOwned(
                         initialContext, node))
        BY <2>5
           DEF IndexedHistoricalRecoveryOpenTargetResidualProperty,
               IndexedHistoricalRecoveryOpenGoal
      <3>5. IndexedHistoricalRecoveryTargetOwned(initialContext, node)
               => (IndexedHistoricalCertificateGoal(
                     initialContext, node)
                    \/ \E rank \in 1..4:
                         IndexedHistoricalCertificateStageAt(
                           initialContext, node, rank))
        BY IndexedHistoricalTargetHasExactCertificateStage
      <3>6. (\E rank \in 1..4:
               IndexedHistoricalCertificateStageAt(
                 initialContext, node, rank))
               ~> IndexedHistoricalCertificateGoal(
                    initialContext, node)
        BY <2>9, PTL
      <3>7. IndexedHistoricalDecisionOwned(initialContext, node)
               => (IndexedHistoricalDecisionStageGoal(
                     initialContext, node)
                    \/ IndexedHistoricalDecisionStageOwnershipResidual(
                         initialContext, node))
        BY DEF IndexedHistoricalDecisionStageOwnershipResidual
      <3>8. IndexedHistoricalDecisionStageOwnershipResidual(
               initialContext, node)
               ~> IndexedHistoricalDecisionStageGoal(
                    initialContext, node)
        BY <2>6
           DEF IndexedHistoricalDecisionStageOwnershipResidualProperty
      <3>9. (\E rank \in 1..6:
               IndexedHistoricalDecisionStageAt(
                 initialContext, node, rank))
               ~> IndexedHistoricalExactApplication(
                    initialContext, node)
        BY <2>10, PTL
      <3>10. IndexedHistoricalDecisionStageGoal(initialContext, node)
                ~> IndexedHistoricalExactApplication(
                     initialContext, node)
        BY <3>9, PTL
           DEF IndexedHistoricalDecisionStageGoal
      <3>11. IndexedHistoricalCertificateGoal(initialContext, node)
                ~> IndexedHistoricalExactApplication(
                     initialContext, node)
        BY <3>7, <3>8, <3>10, PTL
           DEF IndexedHistoricalCertificateGoal
      <3>12. IndexedHistoricalRecoveryTargetOwned(
                initialContext, node)
                ~> IndexedHistoricalExactApplication(
                     initialContext, node)
        BY <3>5, <3>6, <3>11, PTL
      <3>13. IndexedHistoricalRecoveryOpenable(initialContext, node)
                ~> IndexedHistoricalExactApplication(
                     initialContext, node)
        BY <3>3, <3>4, <3>7, <3>8, <3>10, <3>12, PTL
      <3>14. IndexedHistoricalRecoveryEntryGoal(initialContext, node)
                ~> IndexedHistoricalExactApplication(
                     initialContext, node)
        BY <3>7, <3>8, <3>10, <3>12, <3>13, PTL
           DEF IndexedHistoricalRecoveryEntryGoal
      <3>15. IndexedHistoricalExactApplication(initialContext, node)
                ~> HistoricalRecoveryComplete(
                     initialContext, node)
        BY <2>8
           DEF IndexedHistoricalApplicationReceiptHandoffProperty
      <3> QED BY <3>1, <3>2, <3>14, <3>15, PTL
    <2> QED BY <2>11 DEF IndexedExactHistoricalRecoveryProgress
  <1> QED BY <1>1

=============================================================================
