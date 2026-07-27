---- MODULE SumeragiV2HistoricalRecoveryTemporalClosureProofs ----
EXTENDS SumeragiV2ChainEpochRefinement,
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
       asyncOutstandingWork <- IndexedScheduler(initialContext, 11),
       asyncIoReadyCompletions <- IndexedScheduler(initialContext, 12),
       asyncLocalReadyCompletions <- IndexedScheduler(initialContext, 13),
       asyncNextCompletionSource <- IndexedScheduler(initialContext, 14),
       asyncIoControlAvailable <- IndexedScheduler(initialContext, 15),
       asyncDeferredCompletionQueues <- IndexedScheduler(initialContext, 16),
       asyncDeferredProgressQueues <- IndexedScheduler(initialContext, 17),
       asyncDeferredNormalQueues <- IndexedScheduler(initialContext, 18),
       asyncDeferredHandoffs <- IndexedScheduler(initialContext, 19),
       asyncNextDeferredClass <- IndexedScheduler(initialContext, 20),
       asyncDeferredDrainOwed <- IndexedScheduler(initialContext, 21),
       asyncCausalQueues <- IndexedScheduler(initialContext, 22),
       asyncOutstandingTags <- IndexedScheduler(initialContext, 23),
       asyncNodeDeadlines <- IndexedScheduler(initialContext, 24),
       asyncRetransmitDeadlines <- IndexedScheduler(initialContext, 25),
       asyncNodeServiceDeadlines <- IndexedScheduler(initialContext, 26),
       asyncIoServiceDeadlines <- IndexedScheduler(initialContext, 27),
       asyncSentItems <- IndexedScheduler(initialContext, 28),
       asyncRetainedControl <- IndexedScheduler(initialContext, 29),
       asyncActiveRequests <- IndexedScheduler(initialContext, 30),
       asyncCertifiedResponseClaim <- IndexedScheduler(initialContext, 31),
       asyncTransport <- IndexedScheduler(initialContext, 32),
       asyncIngressLanes <- IndexedScheduler(initialContext, 33),
       asyncIngressReady <- IndexedScheduler(initialContext, 34),
       asyncHeldChunks <- IndexedScheduler(initialContext, 35),
       asyncHistoricalRecoveryTargets <- IndexedScheduler(initialContext, 36),
       asyncControlServiceState <- IndexedScheduler(initialContext, 37),
       asyncRecoveryPhase <- IndexedRecovery(initialContext, 1),
       asyncRecoveryNode <- IndexedRecovery(initialContext, 2),
       asyncRecoveryGeneration <- IndexedRecovery(initialContext, 3),
       asyncRecoveryReplayQueue <- IndexedRecovery(initialContext, 4),
       asyncHistoricalLockRestartAuthorities <-
         IndexedRecovery(initialContext, 5)

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
       IndexedDecisionWitness!vars,
       IndexedCore, IndexedScheduler, IndexedRecovery

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
               IndexedDecisionWitness!vars,
               IndexedCore, IndexedScheduler, IndexedRecovery
      <3> QED BY <3>2
    <2> QED BY <1>1, <2>1, <2>2
  <1> QED BY <1>1

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
    <2> QED BY <2>2, <2>3, <2>4, <2>5, <2>6, <2>7, <2>8
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
    <2> QED BY <2>3, <2>4, <2>5, <2>6, <2>7, <2>8, <2>9
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
discharged by source availability or ordinary one-height progress; treating
it as a historical packet/service theorem would be circular.

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
remaining temporal rank and authority operators are not asserted as theorems.
One PTL reduction exposes the historical-only boundary after source authority;
a second keeps the broader chain-level premise conditional on the
ordinary-consensus authority residual.
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
      SequenceSet(IndexedScheduler(initialContext, 33)
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
  \/ request \in IndexedScheduler(initialContext, 30)
  \/ \E packet \in IndexedScheduler(initialContext, 32):
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
  /\ response.source =
       IndexedAsync(initialContext)!AsyncUntrustedSource
  /\ response.envelope.recipient = node
  /\ response.envelope.request = request

IndexedHistoricalCommitResponsePublished(initialContext, node) ==
  /\ IndexedHistoricalRecoveryTargetOwned(initialContext, node)
  /\ \E request, qc, response:
       /\ response \in IndexedScheduler(initialContext, 28)
       /\ IndexedHistoricalCommitResponseIdentity(
            initialContext, node, request, qc, response)

(***************************************************************************
Recipient-specific import ownership.  Global `commitQCs` membership is not
enough: the serving archive already owns that QC before recovery starts.
The predicates below require the exact target's QcEnvelope/QcAt, Decision WAL,
or a current protected target command carrying the exact CommitQC lineage.
***************************************************************************)

IndexedHistoricalCertificateCommandLineage(
    initialContext, node, qc, candidate) ==
  \/ /\ candidate.evidence \in IndexedScheduler(initialContext, 28)
     /\ candidate.evidence.kind = "CommitQC"
     /\ candidate.evidence.envelope =
          IndexedAsync(initialContext)!QcEnvelope(node, qc)
     /\ candidate.item =
          IF candidate.kind = "DeliverQC"
          THEN candidate.evidence
          ELSE IndexedAsync(initialContext)!NoAsyncItem
  \/ \E request, response:
       /\ response \in IndexedScheduler(initialContext, 28)
       /\ IndexedHistoricalCommitResponseIdentity(
            initialContext, node, request, qc, response)
       /\ candidate.evidence = response
       /\ candidate.item =
            IF candidate.kind = "DeliverQC"
            THEN IndexedAsync(initialContext)!
                   DiscoveredCommitQcItem(response)
            ELSE IndexedAsync(initialContext)!NoAsyncItem

IndexedHistoricalCertificateCommandFor(
    initialContext, node, qc, candidate) ==
  /\ candidate \in
       IndexedAsync(initialContext)!AsyncCandidateSet
  /\ IndexedHistoricalRecoveryTargetOwned(initialContext, node)
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

THEOREM IndexedHistoricalCertificateCommandHasPhysicalOwner ==
  \A initialContext \in AdmissibleContextRecords,
     node, qc, candidate:
    IndexedHistoricalCertificateCommandFor(
      initialContext, node, qc, candidate)
      => /\ IndexedHistoricalRecoveryTargetOwned(initialContext, node)
         /\ IndexedAsync(initialContext)!CandidateConsumerCurrent(candidate)
         /\ IndexedDecisionWitness(initialContext)!
              ProtectedCandidateOwned(candidate)
         /\ IndexedHistoricalCertificateCommandLineage(
              initialContext, node, qc, candidate)
BY DEF IndexedHistoricalCertificateCommandFor

IndexedHistoricalCommitCertificateImported(
    initialContext, node) ==
  /\ IndexedHistoricalRecoveryTargetOwned(initialContext, node)
  /\ \E qc \in IndexedCore(initialContext, 23):
       /\ qc.context = initialContext
       /\ qc.phase = "Commit"
       /\ \/ IndexedAsync(initialContext)!QcEnvelope(node, qc)
               \in IndexedCore(initialContext, 42)
          \/ IndexedAsync(initialContext)!QcAt(node, qc)
               \in IndexedCore(initialContext, 15)
          \/ IndexedAsync(initialContext)!DecisionWal(node, qc, FALSE)
               \in IndexedCore(initialContext, 36)
          \/ \E candidate:
               IndexedHistoricalCertificateCommandFor(
                 initialContext, node, qc, candidate)

(***************************************************************************
Certificate rank:

  4  exact OpenHistoricalRecovery target, before request ownership
  3  exact request registration/packet/ingress/fresh Serve job
  2  exact CommitCertificateResponse published by a serving archive
  1  target-specific CommitQC envelope/receipt/Decision-WAL or exact current
     protected command owner

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
       \in IndexedCore(initialContext, 45)
  /\ qc.context = initialContext
  /\ qc.phase = "Commit"

IndexedHistoricalDecisionCertifiedRequestActiveExact(
    initialContext, node, qc) ==
  \E request \in IndexedScheduler(initialContext, 30):
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
                 candidate.item.envelope.archiveServer
            /\ candidate.item.envelope.citedResponder \in qc.signers
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
     node \in Responsive, qc:
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

THEOREM IndexedChainSpecClosesHistoricalDecisionCandidateRankResiduals ==
  IndexedChainSpec
    => IndexedHistoricalDecisionCandidateRankProgressResidualProperty
BY IndexedChainSpecClosesHistoricalDecisionBodyCandidateLeaves,
   IndexedChainSpecClosesHistoricalProtectedCandidateStarvation,
   IsaT(1200), PTL
   DEF IndexedHistoricalDecisionCandidateRankProgressResidualProperty,
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

(***************************************************************************
Derived exact-Candidate tail of certificate rank 1.

The rank-1 import predicate also admits a freshly received QcEnvelope,
received-QC pool entry, or Decision WAL before a scheduled DeliverQC command
is visible.  The theorem below therefore closes only the exact Candidate
tail—DeliverQC, BeginDecision, PersistDecision.  It does not silently promote
all of rank 1, and ranks 4..2 remain in the discovery/transport corridor.
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

IndexedHistoricalCertificateRankOneCandidateEntryProperty ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    IndexedHistoricalCertificateStageAt(initialContext, node, 1)
      ~> (IndexedHistoricalCertificateGoal(initialContext, node)
           \/ \E kind \in
                {"DeliverQC", "BeginDecision", "PersistDecision"}:
                IndexedHistoricalCertificateCandidateTailAt(
                  initialContext, node, kind))

IndexedHistoricalCertificateRemainingCorridorProperty ==
  /\ IndexedHistoricalCertificateDiscoveryRunnerResidualProperty
  /\ IndexedHistoricalCertificateRequestServiceResidualProperty
  /\ IndexedHistoricalCertificateResponseImportResidualProperty
  /\ IndexedHistoricalCertificateRankOneCandidateEntryProperty

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
    <2>2. IndexedHistoricalCertificateImportedDecisionResidualProperty
      BY <1>1, <2>1, PTL
         DEF IndexedHistoricalCertificateRemainingCorridorProperty,
             IndexedHistoricalCertificateRankOneCandidateEntryProperty,
             IndexedHistoricalCertificateCandidateTailProgressProperty,
             IndexedHistoricalCertificateCandidateTailGoal,
             IndexedHistoricalCertificateRankProgressAt,
             SetLessThan
    <2> QED BY <1>1, <2>2
         DEF IndexedHistoricalCertificateRemainingCorridorProperty,
             IndexedHistoricalCertificateRankProgressResidualProperty
  <1> QED BY <1>1

THEOREM IndexedHistoricalDecisionCertifiedRequestClosesRankResidual ==
  /\ IndexedChainSpec
  /\ IndexedHistoricalDecisionCertifiedRequestResidualProperty
  => IndexedHistoricalDecisionRankProgressResidualProperty
BY IndexedChainSpecClosesHistoricalDecisionCandidateRankResiduals,
   IndexedHistoricalDecisionRankResidualSplitsAtCertifiedRequest

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
       IndexedAsync!vars

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
Exact ordinary-authority prefix split.

The authority-acquisition wrapper cannot soundly import
`IndexedLiveInstanceActivationObligation`: its antecedent names one joined
historical target, while that obligation requires eventual join of every
responsive validator in the frozen instance.  The existing arbitrary-context
join theorems consume `IndexedExactHistoricalRecoveryProgress`, so using them
here would be circular.

Even after activation, `AsyncAllResponsiveAppliedAt` supplies only an
application at every current responsive voter.  The indexed Open guard is
strictly stronger: its exact archival record must be owned by a server which
is itself one of that record's CommitQC signers.  No current theorem preserves
the locally formed CommitQC origin through Decision and Apply to expose that
self-signed applied archive.

The three properties below make those boundaries machine-inspectable without
adding fairness or changing the release statement:

  1. reach exact entry or a still-outstanding fully joined instance;
  2. in that instance, reach exact entry or a still-outstanding typed archive;
  3. retain/restore that archive through GST until the existing exact Open
     guard (or another entry arm) appears.

The reduction is pure PTL composition.  It is not a proof of any of the three
producer properties, and none may be discharged with indexed height liveness
or `IndexedExactHistoricalRecoveryProgress`.
***************************************************************************)

IndexedHistoricalRecoveryTypedArchiveAuthority(initialContext) ==
  \E server \in ValidatorIds,
     source \in Chain!DecisionEvidenceSet:
    IndexedHistoricalRecoverySourceReady(
      initialContext, server, source)

IndexedHistoricalRecoveryActivationPrefixResidualProperty ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    IndexedHistoricalRecoveryAuthorityAcquisitionResidual(
      initialContext, node)
      ~> (IndexedHistoricalRecoveryEntryGoal(initialContext, node)
           \/ /\ IndexedAllResponsiveJoined(initialContext)
              /\ IndexedHistoricalRecoveryAuthorityAcquisitionResidual(
                   initialContext, node))

(***************************************************************************
Genesis/non-genesis activation boundary.

Every responsive validator is joined to `GenesisContext` by
`IndexedChainInit`, and joined membership is monotone.  The genesis slice of
the activation prefix is therefore immediate and does not consume one-height
or historical-recovery liveness.

A non-genesis joined target is different.  It proves that one validator
crossed the predecessor, not that every responsive validator did.  New-roster
entrants and responsive observers may still be at an earlier canonical
context.  `IndexedResponsiveHeightReached(initialContext.height)` is the exact
predecessor-catchup boundary after which the existing successor-activation
starvation theorem can join them to `initialContext`; establishing that
boundary requires a well-founded lower-height ordinary/historical composition.
It may not be obtained from `IndexedExactHistoricalRecoveryProgress` here.
***************************************************************************)

IndexedHistoricalRecoveryGenesisActivationPrefixResidualProperty ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    /\ initialContext = GenesisContext
    /\ IndexedHistoricalRecoveryAuthorityAcquisitionResidual(
         initialContext, node)
      ~> (IndexedHistoricalRecoveryEntryGoal(initialContext, node)
           \/ /\ IndexedAllResponsiveJoined(initialContext)
              /\ IndexedHistoricalRecoveryAuthorityAcquisitionResidual(
                   initialContext, node))

IndexedHistoricalRecoveryNonGenesisActivationPrefixResidualProperty ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    /\ initialContext # GenesisContext
    /\ IndexedHistoricalRecoveryAuthorityAcquisitionResidual(
         initialContext, node)
      ~> (IndexedHistoricalRecoveryEntryGoal(initialContext, node)
           \/ /\ IndexedAllResponsiveJoined(initialContext)
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

THEOREM IndexedChainSpecAlwaysActivatesHistoricalGenesisInstance ==
  IndexedChainSpec
    => []IndexedAllResponsiveJoined(GenesisContext)
PROOF
  <1>1. IndexedChainInit
           => \A node \in Responsive:
                node \in joinedByContext[GenesisContext]
    BY Isa
       DEF IndexedChainInit, GenesisContext, AdmissibleContextRecords,
           FrozenContextAdmissible, ContextRecords, LineagesAt, Heights,
           ModelConfiguration, ValidatorIds
  <1>2. \A node \in Responsive:
           node \in joinedByContext[GenesisContext]
             /\ [IndexedChainNext]_IndexedChainVars
             => node \in joinedByContext[GenesisContext]'
    BY IndexedNodeJoinIsStable, Isa
       DEF AdmissibleContextRecords, FrozenContextAdmissible,
           ContextRecords, LineagesAt, Heights, GenesisContext,
           ModelConfiguration, ValidatorIds
  <1> QED BY <1>1, <1>2, PTL
       DEF IndexedChainSpec, IndexedAllResponsiveJoined

THEOREM IndexedLiveChainSpecClosesHistoricalGenesisActivationPrefix ==
  IndexedLiveChainSpec
    => IndexedHistoricalRecoveryGenesisActivationPrefixResidualProperty
BY IndexedLiveChainSpecProjectsIndexedChainSpec,
   IndexedChainSpecAlwaysActivatesHistoricalGenesisInstance, PTL
   DEF IndexedHistoricalRecoveryGenesisActivationPrefixResidualProperty

IndexedHistoricalRecoveryActivatedArchiveProducerResidualProperty ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    /\ IndexedAllResponsiveJoined(initialContext)
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
                  \/ /\ IndexedAllResponsiveJoined(initialContext)
                     /\ IndexedHistoricalRecoveryAuthorityAcquisitionResidual(
                          initialContext, node))
      BY <1>1
         DEF IndexedHistoricalRecoveryOrdinaryAuthorityResidualProperties,
             IndexedHistoricalRecoveryActivationPrefixResidualProperty
    <2>2. /\ IndexedAllResponsiveJoined(initialContext)
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

There are three unproved temporal kernels in this leaf:

  1. ordinary consensus until exact applied-archive authority exists;
  2. the remaining certificate discovery/request/response/import corridor;
  3. the active CertifiedRequest corridor at Decision rank 5.

Exact historical Decision-stage ownership exposure is closed above as an
indexed safety invariant.  It therefore needs no scheduler fairness premise.

The broad first source cannot be closed by historical fairness.  In the
reachable genesis state every responsive node is joined at height zero, while
there is no Decision, application, recovery target, or applied archive:
`HistoricalRecoveryOutstanding` holds and every historical fair action is
disabled.  The sound caller decomposition is therefore ordinary current-voter
one-height progress until an exact durable applied source exists, followed by
the Open theorem and historical-only ranks in this module.  Assuming indexed
height liveness here would be circular.

Exact Open and the application receipt handoff are proved above from
`IndexedChainSpec`.  The certificate residual is split into historical
discovery, request packet/archive Serve/ordinary-I/O service, response packet
import, and target-runner Decision.  Indexed Stage 2..6 service, historical
candidate starvation, and the DeliverQC/BeginDecision/PersistDecision tail are
closed; `IndexedHistoricalCertificateRemainingCorridorProperty` is the exact
remaining certificate seam.  Decision ranks 6, 4, 3, 2, and 1 are closed from
the indexed Candidate aggregate; only the rank-5 certified-request
route/body-service corridor remains.  No item in this inventory assumes
`IndexedExactHistoricalRecoveryProgress`,
`ApplicationLivenessProperty`, or `ExactDecisionStageServiceProperty`.
***************************************************************************)

(***************************************************************************
Exact proof-debt declarations.

Exactly three proofless theorem wrappers make every remaining temporal kernel
visible to the release ledger.  The Decision-stage ownership safety theorem is
proved above from `IndexedChainSpec`; composition derives that property rather
than assuming it as a fourth temporal kernel.

TODO: discharge the three remaining wrappers from the exact ordinary-consensus,
fixed-clock finite-episode, authenticated packet/Serve/I/O, and exact
CertifiedRequest fairness actions under the explicit `IndexedLiveChainSpec`
install-generation premise, without assuming indexed height liveness or
current-voter application liveness.
***************************************************************************)

THEOREM IndexedHistoricalRecoveryAuthorityAcquisitionResidualObligation ==
  IndexedLiveChainSpec
    => IndexedHistoricalRecoveryAuthorityAcquisitionResidualProperty

(***************************************************************************
Certificate-rank non-circularity boundary.

Rank 1 now requires an exact current-consumer protected owner with production
class and CommitQC evidence/item lineage.  Membership in the
`AsyncCandidateSet` type carrier, a stale scheduled occurrence, or an
unrelated same-round command can no longer fabricate imported certificate
ownership.

The remaining rank theorem is reduced to
`IndexedHistoricalCertificateRemainingCorridorProperty`:

  * rank 4 still needs discovery-clock progress.  Weak fairness of
    `IndexedTickStep` is insufficient because overdue packet and local-service
    owners can disable `AsyncTick`.  The Candidate/Serve identity bridge and
    indexed lifecycle invariant are proved, but the separate finite
    non-descent episode leaf remains temporal proof debt;
  * ranks 3 and 2 still need exact request/response retention through
    retransmission, historical packet admission, archive Serve/ordinary I/O,
    and response admission; and
  * rank 1 still needs the short import-to-exact-Candidate entry from a fresh
    QcEnvelope, received-QC entry, or Decision WAL.  Once DeliverQC is visible,
    the indexed historical starvation theorem closes DeliverQC,
    BeginDecision, and PersistDecision.

The Stage 2..6 and Candidate tail prerequisites are no longer assumptions.
Assuming the remaining consequences via target-to-Decision, or assuming this
wrapper itself, would still be circular.

TODO: prove the fixed-clock finite episode, exact authenticated
request/response owner retention, and import-to-Candidate handoff from the
indexed fair actions before adding a proof to this wrapper.
***************************************************************************)

THEOREM IndexedHistoricalCertificateRankProgressResidualObligation ==
  IndexedLiveChainSpec
    => IndexedHistoricalCertificateRankProgressResidualProperty

(***************************************************************************
Decision-rank non-circularity boundary.

`IndexedChainSpecClosesHistoricalDecisionCandidateRankResiduals` closes ranks
6, 4, 3, 2, and 1.  By
`IndexedHistoricalDecisionRankResidualSplitsAtCertifiedRequest`, the wrapper
below is now equivalent, under `IndexedChainSpec`, to the rank-5 active
CertifiedRequest corridor.  That corridor still requires its responsive
archive route, retained exact request, packet admission, Serve reservation,
ordinary I/O response, and authenticated target admission.  It remains
proofless rather than borrowing application liveness or treating request
replenishment as progress.
***************************************************************************)

THEOREM IndexedHistoricalDecisionRankProgressResidualObligation ==
  IndexedLiveChainSpec
    => IndexedHistoricalDecisionRankProgressResidualProperty

IndexedHistoricalRecoveryTemporalResidualKernels ==
  /\ IndexedHistoricalRecoveryAuthorityAcquisitionResidualProperty
  /\ IndexedHistoricalCertificateRankProgressResidualProperty
  /\ IndexedHistoricalDecisionRankProgressResidualProperty

THEOREM IndexedHistoricalRecoveryResidualKernelsDischargeExactProgress ==
  /\ IndexedLiveChainSpec
  /\ IndexedHistoricalRecoveryTemporalResidualKernels
    => IndexedExactHistoricalRecoveryProgress
PROOF
  <1>1. ASSUME IndexedLiveChainSpec,
              IndexedHistoricalRecoveryTemporalResidualKernels
         PROVE IndexedExactHistoricalRecoveryProgress
    <2>0. IndexedChainSpec
      BY <1>1, IndexedLiveChainSpecProjectsIndexedChainSpec
    <2>1. IndexedHistoricalRecoveryAuthorityAcquisitionResidualProperty
      BY <1>1 DEF IndexedHistoricalRecoveryTemporalResidualKernels
    <2>2. IndexedHistoricalRecoveryOpenTargetResidualProperty
      BY <2>0, IndexedChainSpecClosesHistoricalOpenTarget
    <2>3. IndexedHistoricalCertificateRankProgressResidualProperty
      BY <1>1 DEF IndexedHistoricalRecoveryTemporalResidualKernels
    <2>4. IndexedHistoricalDecisionStageOwnershipResidualProperty
      BY <2>0, IndexedHistoricalDecisionStageOwnershipResidualObligation
    <2>5. IndexedHistoricalDecisionRankProgressResidualProperty
      BY <1>1 DEF IndexedHistoricalRecoveryTemporalResidualKernels
    <2>6. IndexedHistoricalApplicationReceiptHandoffProperty
      BY <2>0, IndexedChainSpecClosesHistoricalApplicationReceiptHandoff
    <2>7. \A initialContext \in AdmissibleContextRecords,
              node \in Responsive, rank \in Nat:
             IndexedHistoricalCertificateStageAt(
               initialContext, node, rank)
               ~> IndexedHistoricalCertificateGoal(
                    initialContext, node)
      BY <2>3, IndexedHistoricalCertificateRankConvergence
    <2>8. \A initialContext \in AdmissibleContextRecords,
              node \in Responsive, rank \in Nat:
             IndexedHistoricalDecisionStageAt(
               initialContext, node, rank)
               ~> IndexedHistoricalExactApplication(
                    initialContext, node)
      BY <2>5, IndexedHistoricalDecisionRankConvergence
    <2>9. ASSUME NEW initialContext \in AdmissibleContextRecords,
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
        BY <2>1
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
        BY <2>2
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
        BY <2>7, PTL
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
        BY <2>4
           DEF IndexedHistoricalDecisionStageOwnershipResidualProperty
      <3>9. (\E rank \in 1..6:
               IndexedHistoricalDecisionStageAt(
                 initialContext, node, rank))
               ~> IndexedHistoricalExactApplication(
                    initialContext, node)
        BY <2>8, PTL
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
        BY <2>6
           DEF IndexedHistoricalApplicationReceiptHandoffProperty
      <3> QED BY <3>1, <3>2, <3>14, <3>15, PTL
    <2> QED BY <2>9 DEF IndexedExactHistoricalRecoveryProgress
  <1> QED BY <1>1

=============================================================================
