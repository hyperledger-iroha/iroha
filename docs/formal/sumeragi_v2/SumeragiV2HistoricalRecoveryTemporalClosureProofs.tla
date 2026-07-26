---- MODULE SumeragiV2HistoricalRecoveryTemporalClosureProofs ----
EXTENDS SumeragiV2ChainEpochRefinement

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
  * Decision ranks 6..1 are respectively FetchBody, one exact
    body-holding CertifiedRequest route, FetchCertifiedBody, StoreBody,
    ValidateBody, and Apply;
  * exact application is handed to the existing chain receipt classifier.

The rank predicates do not place a historical target in
`AsyncCurrentResponsiveVoters`.  Their executor owner is explicitly either
the ordinary current-voter runner or the exact historical target.  Thus an
observer or successor-roster entrant relies on
`PostGstRunHistoricalRecoveryNode`,
`PostGstServiceHistoricalRecoveryIoWorker`, and the historical packet
corridor, not on voter fairness.

The exact Open property and exact application receipt handoff are proved here
from `IndexedChainSpec`.  The remaining property operators are not asserted as
theorems.  One PTL reduction exposes the historical-only boundary after source
authority; a second keeps the broader chain-level premise conditional on the
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
      SequenceSet(IndexedScheduler(initialContext, 32)
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
  \/ request \in IndexedScheduler(initialContext, 29)
  \/ \E packet \in IndexedScheduler(initialContext, 31):
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
       /\ response \in IndexedScheduler(initialContext, 27)
       /\ IndexedHistoricalCommitResponseIdentity(
            initialContext, node, request, qc, response)

(***************************************************************************
Recipient-specific import ownership.  Global `commitQCs` membership is not
enough: the serving archive already owns that QC before recovery starts.
The predicates below require the exact target's QcEnvelope/QcAt, Decision WAL,
or a scheduled target command carrying the same round and subject.
***************************************************************************)

IndexedHistoricalCertificateCommandFor(
    initialContext, node, qc, candidate) ==
  /\ candidate \in
       IndexedAsync(initialContext)!AsyncCandidateSet
  /\ candidate.node = node
  /\ candidate.height = initialContext.height
  /\ candidate.view = qc.view
  /\ candidate.subject = qc.subject
  /\ candidate.kind
       \in {"DeliverQC", "BeginDecision", "PersistDecision"}

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
  1  target-specific CommitQC import/delivery/Decision-WAL owner

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

The body request owner requires a responsive addressed archive which already
holds the exact certified body.  This excludes an unresponsive route from
serving as the liveness witness while preserving the route/archive/signer
identity separation of the production protocol.
***************************************************************************)

IndexedHistoricalDecisionRecord(initialContext, node, qc) ==
  /\ [node |-> node, qc |-> qc]
       \in IndexedCore(initialContext, 45)
  /\ qc.context = initialContext
  /\ qc.phase = "Commit"

IndexedHistoricalBodyRequestIdentity(
    initialContext, node, qc, archive, request) ==
  /\ archive \in Responsive
  /\ archive \in IndexedCore(initialContext, 6)
  /\ archive \in joinedByContext[initialContext]
  /\ archive \in
       IndexedAsync(initialContext)!AsyncArchiveIoServiceNodes
  /\ request \in
       IndexedAsync(initialContext)!
         CertifiedRequestOutbox(node, qc)
  /\ request.envelope.recipient = archive
  /\ IndexedAsync(initialContext)!BodyHeldBy(
       IndexedCore(initialContext, 9), archive,
       initialContext, qc.view, qc.subject)

IndexedHistoricalCertifiedBodyRequestOwned(
    initialContext, node, qc) ==
  \E archive, request:
    /\ IndexedHistoricalBodyRequestIdentity(
         initialContext, node, qc, archive, request)
    /\ IndexedHistoricalRequestPhysicalOwner(
         initialContext, request)

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
  5  one responsive exact body-holding CertifiedRequest route
  4  FetchCertifiedBody
  3  StoreBody
  2  ValidateBody
  1  Apply

Rank 5 is deliberately a separate off-scheduler stage.  Its convergence uses
request retransmission, historical packet admission and Serve/I/O fairness;
it is not implied merely by weak fairness of the node runner.
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
                 IndexedHistoricalCertifiedBodyRequestOwned(
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
  /\ IndexedHistoricalDecisionStageOwnershipResidualProperty
  /\ IndexedHistoricalDecisionRankProgressResidualProperty
  => IndexedExactHistoricalRecoveryFromAuthorityProgress
PROOF
  <1>1. ASSUME IndexedChainSpec,
              IndexedHistoricalCertificateRankProgressResidualProperty,
              IndexedHistoricalDecisionStageOwnershipResidualProperty,
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
      BY <1>1
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
Complete residual inventory and PTL reduction.

There are four unproved temporal kernels in this leaf:

  1. ordinary consensus until exact applied-archive authority exists;
  2. strict certificate request/response/import/Decision rank descent;
  3. exact historical Decision-stage ownership exposure; and
  4. strict body Fetch/request/FetchCertified/Store/Validate/Apply descent.

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
import, and target-runner Decision.  The body residual is split into exact
FetchBody, certified-request archive route, FetchCertifiedBody, StoreBody,
ValidateBody, and Apply owners.  No item in this inventory assumes
`IndexedExactHistoricalRecoveryProgress`,
`ApplicationLivenessProperty`, or `ExactDecisionStageServiceProperty`.
***************************************************************************)

IndexedHistoricalRecoveryTemporalResidualKernels ==
  /\ IndexedHistoricalRecoveryAuthorityAcquisitionResidualProperty
  /\ IndexedHistoricalCertificateRankProgressResidualProperty
  /\ IndexedHistoricalDecisionStageOwnershipResidualProperty
  /\ IndexedHistoricalDecisionRankProgressResidualProperty

THEOREM IndexedHistoricalRecoveryResidualKernelsDischargeExactProgress ==
  /\ IndexedChainSpec
  /\ IndexedHistoricalRecoveryTemporalResidualKernels
    => IndexedExactHistoricalRecoveryProgress
PROOF
  <1>1. ASSUME IndexedChainSpec,
              IndexedHistoricalRecoveryTemporalResidualKernels
         PROVE IndexedExactHistoricalRecoveryProgress
    <2>1. IndexedHistoricalRecoveryAuthorityAcquisitionResidualProperty
      BY <1>1 DEF IndexedHistoricalRecoveryTemporalResidualKernels
    <2>2. IndexedHistoricalRecoveryOpenTargetResidualProperty
      BY <1>1, IndexedChainSpecClosesHistoricalOpenTarget
    <2>3. IndexedHistoricalCertificateRankProgressResidualProperty
      BY <1>1 DEF IndexedHistoricalRecoveryTemporalResidualKernels
    <2>4. IndexedHistoricalDecisionStageOwnershipResidualProperty
      BY <1>1 DEF IndexedHistoricalRecoveryTemporalResidualKernels
    <2>5. IndexedHistoricalDecisionRankProgressResidualProperty
      BY <1>1 DEF IndexedHistoricalRecoveryTemporalResidualKernels
    <2>6. IndexedHistoricalApplicationReceiptHandoffProperty
      BY <1>1, IndexedChainSpecClosesHistoricalApplicationReceiptHandoff
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
