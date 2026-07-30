---- MODULE SumeragiV2HistoricalLockedBodyWitnessPreservationProofs ----
EXTENDS SumeragiV2DecisionWitnessPreservationProofs

(***************************************************************************
Proof-only historical locked-body source retention.

The adapter copies a CertifiedResponse from FetchCertifiedBody through
StoreBody and ValidateBody into the causal BeginLockCommit child.  This leaf
keeps that complete authenticated response visible until the broadened lower
BeginLock recovery vocabulary consumes it; it never collapses the response to
a stable Prepare reference.

This module keeps the two authenticated lineages explicit:

  * Qc lineage is one concrete restart-authorized Prepare QcRecord with the
    same production CertificateRef as the historical source; and
  * response lineage is one authenticated CertifiedResponse for the exact
    restart-authorized Prepare QcRecord and recovering requester, paired with
    the immutable append-only signed request required by delayed execution.

The exact signed-request hash binds requester, the full PrepareQC (including
its signer set), and signature nonce.  The archive server independently owns
the response signature but need not be one of the original physical request
routes; reconnect and relay may expose that exact signed request to another
archive.  The cited responder is independently required to be a signer of the
exact frozen QC.  The outer response transport source is intentionally
irrelevant.  SamePrepareRecoveryRef is used only for QcRecord lineage and
logical recovery ownership after the WAL handoff.

No production action is redefined below.  All owner predicates and the
corrected stage are proof-only classifications of existing state.
***************************************************************************)

HistoricalLockedPrepareQcLineage(node, qc, evidence) ==
  /\ evidence \in QcRecordSet
  /\ evidence \in RestartLockedPrepareQCs(node)
  /\ SamePrepareRecoveryRef(evidence, qc)

HistoricalLockedCertifiedResponseLineage(node, qc, item) ==
  /\ qc \in RestartLockedPrepareQCs(node)
  /\ HistoricalCertifiedResponseRecoveryEvidence(node, qc, item)
  /\ CertifiedResponseCapabilityAuthorized(item)

HistoricalLockedBodyEvidenceLineage(node, qc, evidence) ==
  \/ HistoricalLockedPrepareQcLineage(node, qc, evidence)
  \/ HistoricalLockedCertifiedResponseLineage(node, qc, evidence)

HistoricalLockedCertifiedRequestActiveLineaged(node, qc) ==
  /\ qc \in RestartLockedPrepareQCs(node)
  /\ \E request \in asyncActiveRequests:
       /\ request \in CertifiedRequestOutbox(node, qc)
       /\ request.kind = "CertifiedRequest"
       /\ request.source = node
       /\ request.envelope.requester = node
       /\ request.envelope.certificate = qc
       /\ AsyncCertifiedRequestHash(request) =
            AsyncCertifiedRequestHashOf(node, qc, 0)
       /\ request.envelope.recipient \in CertifiedArchiveRoutes(node, qc)
       /\ request.envelope.height = qc.context.height
       /\ request.envelope.view = qc.view
       /\ request.envelope.subject = qc.subject

HistoricalLockedBodyCandidateCoordinates(node, qc, candidate) ==
  /\ candidate \in AsyncCandidateSet
  /\ candidate.class = "Completion"
  /\ candidate.node = node
  /\ candidate.height = qc.context.height
  /\ candidate.view = qc.view
  /\ candidate.subject = qc.subject
  /\ CandidateConsumerCurrent(candidate)
  /\ CandidateScheduled(candidate)

HistoricalLockedBodyFetchCandidate(node, qc, candidate) ==
  /\ HistoricalLockedBodyCandidateCoordinates(node, qc, candidate)
  /\ candidate.kind = "FetchBody"
  /\ HistoricalLockedPrepareQcLineage(node, qc, candidate.evidence)

HistoricalLockedBodyCertifiedFetchCandidate(node, qc, candidate) ==
  /\ HistoricalLockedBodyCandidateCoordinates(node, qc, candidate)
  /\ candidate.kind = "FetchCertifiedBody"
  /\ candidate.item = candidate.evidence
  /\ HistoricalLockedCertifiedResponseLineage(
       node, qc, candidate.evidence)

HistoricalLockedBodyStoreCandidate(node, qc, candidate) ==
  /\ HistoricalLockedBodyCandidateCoordinates(node, qc, candidate)
  /\ candidate.kind = "StoreBody"
  /\ HistoricalLockedBodyEvidenceLineage(node, qc, candidate.evidence)

HistoricalLockedBodyValidateCandidate(node, qc, candidate) ==
  /\ HistoricalLockedBodyCandidateCoordinates(node, qc, candidate)
  /\ candidate.kind = "ValidateBody"
  /\ HistoricalLockedBodyEvidenceLineage(node, qc, candidate.evidence)

HistoricalLockedBodyBeginLockCandidate(node, qc, candidate) ==
  /\ HistoricalLockedBodyCandidateCoordinates(node, qc, candidate)
  /\ candidate.kind = "BeginLockCommit"
  /\ HistoricalLockedBodyEvidenceLineage(node, qc, candidate.evidence)

HistoricalLineagedLockedBodyFetchOwned(node, qc) ==
  \E candidate \in AsyncCandidateSet:
    HistoricalLockedBodyFetchCandidate(node, qc, candidate)

HistoricalLineagedLockedBodyCertifiedFetchOwned(node, qc) ==
  \E candidate \in AsyncCandidateSet:
    HistoricalLockedBodyCertifiedFetchCandidate(node, qc, candidate)

HistoricalLineagedLockedBodyStoreOwned(node, qc) ==
  \E candidate \in AsyncCandidateSet:
    HistoricalLockedBodyStoreCandidate(node, qc, candidate)

HistoricalLineagedLockedBodyValidateOwned(node, qc) ==
  \E candidate \in AsyncCandidateSet:
    HistoricalLockedBodyValidateCandidate(node, qc, candidate)

HistoricalLockedBodyBeginLockOwned(node, qc) ==
  \E candidate \in AsyncCandidateSet:
    HistoricalLockedBodyBeginLockCandidate(node, qc, candidate)

HistoricalLockedBodyValidationHeld(node, qc) ==
  BodyValidatedBy(validatedBodies, node, context, qc.view,
                  generation[node], qc.subject)

(***************************************************************************
The Commit witness admits the same two evidence lineages only at the
proof-only scheduled BeginLock owner.  Durable Commit intent and pending WAL
ownership retain their existing exact/stable-reference definitions.
***************************************************************************)

HistoricalLockedBodyDurableOrPendingCommitWitness(node, qc) ==
  \/ ExactLockedCommitIntents(node, qc.view, qc.subject) # {}
  \/ \E request \in pendingLockCommit:
       /\ request.node = node
       /\ SamePrepareRecoveryRef(request.qc, qc)

HistoricalLockedBodyCommitWitnessLineaged(node, qc) ==
  \/ HistoricalLockedBodyDurableOrPendingCommitWitness(node, qc)
  \/ HistoricalLockedBodyBeginLockOwned(node, qc)

HistoricalLockedBodyCompletionOwnerLineaged(node, qc) ==
  \/ HistoricalLockedBodyCommitWitnessLineaged(node, qc)
  \/ HistoricalLockedCertifiedRequestActiveLineaged(node, qc)
  \/ HistoricalLineagedLockedBodyFetchOwned(node, qc)
  \/ HistoricalLineagedLockedBodyCertifiedFetchOwned(node, qc)
  \/ HistoricalLineagedLockedBodyStoreOwned(node, qc)
  \/ HistoricalLineagedLockedBodyValidateOwned(node, qc)
  \/ HistoricalLockedBodyRecoveryTerminal(node, qc)

(***************************************************************************
No-current-validation partition.

Before current-generation validation, a live owner must be in the exact body
pipeline (or already have Commit ownership).  Once validation exists, a
higher conflicting Prepare is the modeled terminal IrrelevantView result;
otherwise the same-reference Commit pipeline must own the source.
Controller authority remains a separate crash/replay disjunct.
***************************************************************************)

HistoricalLockedBodyRecoveryStageLineagedWithoutAuthority(node, qc) ==
  \/ /\ ~HistoricalLockedBodyValidationHeld(node, qc)
     /\ \/ HistoricalLockedBodyDurableOrPendingCommitWitness(node, qc)
        \/ HistoricalLineagedLockedBodyFetchOwned(node, qc)
        \/ /\ ~BodyHeldBy(durableBodies, node, context,
                           qc.view, qc.subject)
           /\ \/ HistoricalLockedCertifiedRequestActiveLineaged(node, qc)
              \/ HistoricalLineagedLockedBodyCertifiedFetchOwned(node, qc)
              \/ /\ BodyRecord(node, context, qc.view, qc.subject)
                       \in availableBodies
                 /\ HistoricalLineagedLockedBodyStoreOwned(node, qc)
        \/ /\ BodyHeldBy(durableBodies, node, context,
                          qc.view, qc.subject)
           /\ HistoricalLineagedLockedBodyValidateOwned(node, qc)
  \/ /\ BodyHeldBy(durableBodies, node, context, qc.view, qc.subject)
     /\ HistoricalLockedBodyValidationHeld(node, qc)
     /\ IF NoHigherConflictingPrepareKnown(node, qc)
        THEN HistoricalLockedBodyCommitWitnessLineaged(node, qc)
        ELSE HistoricalLockedBodyRecoveryTerminal(node, qc)

HistoricalLockedBodyRecoveryStageLineaged(node, qc) ==
  \/ HistoricalLockedBodyRecoveryAuthority(node, qc)
  \/ HistoricalLockedBodyRecoveryStageLineagedWithoutAuthority(node, qc)

HistoricalLockedBodyLineageSourceRetentionInvariant ==
  \A node \in AsyncCurrentResponsiveVoters, qc \in prepareQCs:
    HistoricalLockedPrepareSource(node, qc)
      => HistoricalLockedBodyRecoveryStageLineaged(node, qc)

ResponsiveReplayLockedBodyLineagedCarrierInvariant ==
  asyncRecoveryPhase = "Replaying"
    => \A qc \in RestartLockedPrepareQCs(asyncRecoveryNode):
         HistoricalLockedBodyRecoveryStageLineagedWithoutAuthority(
           asyncRecoveryNode, qc)

(***************************************************************************
Concrete-owner projections.

Every exact body owner is an existing production pipeline owner.  The lower
BeginLock vocabulary admits either a same-reference QcRecord or the exact
authenticated response, so the response-lined owner also projects directly.
***************************************************************************)

THEOREM LineagedCertifiedRequestProjectsHistoricalRequest ==
  \A node, qc:
    HistoricalLockedCertifiedRequestActiveLineaged(node, qc)
      => HistoricalLockedCertifiedRequestActive(node, qc)
BY Isa
   DEF HistoricalLockedCertifiedRequestActiveLineaged,
       HistoricalLockedCertifiedRequestActive,
       CertifiedRequestOutbox, RestartLockedPrepareQCs

THEOREM LineagedFetchProjectsHistoricalPipeline ==
  \A node, qc, candidate:
    HistoricalLockedBodyFetchCandidate(node, qc, candidate)
      => HistoricalLockedBodyPipelineCandidate(node, qc, candidate)
BY Isa
   DEF HistoricalLockedBodyFetchCandidate,
       HistoricalLockedBodyCandidateCoordinates,
       HistoricalLockedBodyPipelineCandidate

THEOREM LineagedCertifiedFetchProjectsHistoricalPipeline ==
  \A node, qc, candidate:
    HistoricalLockedBodyCertifiedFetchCandidate(node, qc, candidate)
      => HistoricalLockedBodyPipelineCandidate(node, qc, candidate)
BY Isa
   DEF HistoricalLockedBodyCertifiedFetchCandidate,
       HistoricalLockedBodyCandidateCoordinates,
       HistoricalLockedBodyPipelineCandidate

THEOREM LineagedStoreProjectsHistoricalPipeline ==
  \A node, qc, candidate:
    HistoricalLockedBodyStoreCandidate(node, qc, candidate)
      => HistoricalLockedBodyPipelineCandidate(node, qc, candidate)
BY Isa
   DEF HistoricalLockedBodyStoreCandidate,
       HistoricalLockedBodyCandidateCoordinates,
       HistoricalLockedBodyPipelineCandidate

THEOREM LineagedValidateProjectsHistoricalPipeline ==
  \A node, qc, candidate:
    HistoricalLockedBodyValidateCandidate(node, qc, candidate)
      => HistoricalLockedBodyPipelineCandidate(node, qc, candidate)
BY Isa
   DEF HistoricalLockedBodyValidateCandidate,
       HistoricalLockedBodyCandidateCoordinates,
       HistoricalLockedBodyPipelineCandidate

THEOREM LineagedStageProjectsConcreteCompletionOwner ==
  \A node, qc:
    HistoricalLockedBodyRecoveryStageLineaged(node, qc)
      => \/ HistoricalLockedBodyRecoveryAuthority(node, qc)
         \/ HistoricalLockedBodyCompletionOwnerLineaged(node, qc)
BY Isa
   DEF HistoricalLockedBodyRecoveryStageLineaged,
       HistoricalLockedBodyRecoveryStageLineagedWithoutAuthority,
       HistoricalLockedBodyCompletionOwnerLineaged

(***************************************************************************
Direct projection to the release stage.

Both lineaged evidence arms satisfy HistoricalBeginLockRecoveryEvidence.
Consequently a scheduled lineaged BeginLock is already a lower recovery
candidate and therefore a lower Commit witness without an extra assumption.
***************************************************************************)

THEOREM LineagedBeginLockProjectsHistoricalRecoveryCandidate ==
  \A node, qc, candidate:
    HistoricalLockedBodyBeginLockCandidate(node, qc, candidate)
      => HistoricalBeginLockRecoveryCandidate(node, qc, candidate)
BY SameReferenceQcProvidesHistoricalBeginLockRecoveryEvidence,
   AuthenticatedResponseProvidesHistoricalBeginLockRecoveryEvidence, Isa
   DEF HistoricalLockedBodyBeginLockCandidate,
       HistoricalLockedBodyCandidateCoordinates,
       HistoricalLockedBodyEvidenceLineage,
       HistoricalLockedPrepareQcLineage,
       HistoricalLockedCertifiedResponseLineage,
       HistoricalBeginLockRecoveryCandidate,
       RestartLockedPrepareQCs

THEOREM LineagedBeginLockProjectsHistoricalCommitWitness ==
  \A node, qc:
    HistoricalLockedBodyBeginLockOwned(node, qc)
      => HistoricalLockedCommitRecoveryWitness(node, qc)
BY LineagedBeginLockProjectsHistoricalRecoveryCandidate, Isa
   DEF HistoricalLockedBodyBeginLockOwned,
       HistoricalLockedCommitRecoveryWitness

THEOREM LineagedCommitProjectsReleaseWitness ==
  \A node, qc:
    HistoricalLockedBodyCommitWitnessLineaged(node, qc)
      => HistoricalLockedCommitRecoveryWitness(node, qc)
BY LineagedBeginLockProjectsHistoricalCommitWitness, Isa
   DEF HistoricalLockedBodyCommitWitnessLineaged,
       HistoricalLockedBodyDurableOrPendingCommitWitness,
       HistoricalLockedCommitRecoveryWitness

THEOREM LineagedStageProjectsReleaseStage ==
  \A node, qc:
    HistoricalLockedBodyRecoveryStageLineaged(node, qc)
      => HistoricalLockedBodyRecoveryStage(node, qc)
BY LineagedCertifiedRequestProjectsHistoricalRequest,
   LineagedFetchProjectsHistoricalPipeline,
   LineagedCertifiedFetchProjectsHistoricalPipeline,
   LineagedStoreProjectsHistoricalPipeline,
   LineagedValidateProjectsHistoricalPipeline,
   LineagedCommitProjectsReleaseWitness, Isa
   DEF HistoricalLockedBodyRecoveryStageLineaged,
       HistoricalLockedBodyRecoveryStageLineagedWithoutAuthority,
       HistoricalLockedBodyRecoveryStage,
       HistoricalLockedBodyDurableOrPendingCommitWitness,
       HistoricalLineagedLockedBodyFetchOwned,
       HistoricalLineagedLockedBodyCertifiedFetchOwned,
       HistoricalLineagedLockedBodyStoreOwned,
       HistoricalLineagedLockedBodyValidateOwned,
       HistoricalLockedBodyRecoveryTerminal

THEOREM LineagedInvariantProjectsReleaseInvariant ==
  HistoricalLockedBodyLineageSourceRetentionInvariant
    => HistoricalLockedBodyRecoveryStageInvariant
BY LineagedStageProjectsReleaseStage, Isa
   DEF HistoricalLockedBodyLineageSourceRetentionInvariant,
       HistoricalLockedBodyRecoveryStageInvariant

(***************************************************************************
Base case.
***************************************************************************)

THEOREM AsyncInitEstablishesHistoricalLockedBodyLineageSourceRetention ==
  \A initialContext:
    AsyncInitAt(initialContext)
      => HistoricalLockedBodyLineageSourceRetentionInvariant
BY AsyncInitHasNoHistoricalLockedPrepareSource, Isa
   DEF HistoricalLockedBodyLineageSourceRetentionInvariant

(***************************************************************************
Static lineage facts and exact dispatch.
***************************************************************************)

THEOREM QcLineageIsRestartAuthorizedSameReference ==
  \A node, qc, evidence:
    HistoricalLockedPrepareQcLineage(node, qc, evidence)
      => /\ evidence \in QcRecordSet
         /\ evidence.phase = "Prepare"
         /\ LockedPrepareRecoverySource(node, evidence)
         /\ SamePrepareRecoveryRef(evidence, qc)
BY Isa
   DEF HistoricalLockedPrepareQcLineage,
       RestartLockedPrepareQCs

THEOREM RequestLineageBindsExactSignedRequestAndArchiveRoute ==
  \A node, qc:
    HistoricalLockedCertifiedRequestActiveLineaged(node, qc)
      => \E request \in asyncActiveRequests:
           /\ request \in CertifiedRequestOutbox(node, qc)
           /\ request.kind = "CertifiedRequest"
           /\ request.source = node
           /\ request.envelope.requester = node
           /\ request.envelope.certificate = qc
           /\ request.envelope.signatureNonce = 0
           /\ AsyncCertifiedRequestHash(request) =
                AsyncCertifiedRequestHashOf(node, qc, 0)
           /\ request.envelope.recipient
                \in CertifiedArchiveRoutes(node, qc)
           /\ request.envelope.height = qc.context.height
           /\ request.envelope.view = qc.view
           /\ request.envelope.subject = qc.subject
BY Isa
   DEF HistoricalLockedCertifiedRequestActiveLineaged,
       CertifiedRequestOutbox, AsyncCertifiedRequestEnvelope,
       AsyncCertifiedRequestHash

THEOREM ResponseLineageBindsExactRestartIdentities ==
  \A node, qc, item:
    HistoricalLockedCertifiedResponseLineage(node, qc, item)
      => /\ qc \in RestartLockedPrepareQCs(node)
         /\ item.envelope.requestHash =
              AsyncCertifiedRequestHashOf(node, qc, 0)
         /\ item.envelope.signatureOwner =
              item.envelope.archiveServer
         /\ item.envelope.citedResponder \in qc.signers
         /\ CertifiedResponseAuthenticatedOccurrence(item)
         /\ CertifiedResponseCapabilityAuthorized(item)
BY HistoricalCertifiedResponseRecoveryEvidenceBindsExactIdentities, Isa
   DEF HistoricalLockedCertifiedResponseLineage,
       HistoricalCertifiedResponseRecoveryEvidence

THEOREM ResponseLineageIsIndependentOfOuterRelaySource ==
  \A node, qc, item, relay:
    LET relayed ==
          AsyncNetworkItem("CertifiedResponse", relay, item.envelope)
    IN /\ HistoricalLockedCertifiedResponseLineage(node, qc, item)
       /\ relayed \in AsyncNetworkItems
       => HistoricalLockedCertifiedResponseLineage(node, qc, relayed)
BY Isa
   DEF HistoricalLockedCertifiedResponseLineage,
       HistoricalCertifiedResponseRecoveryEvidence,
       CertifiedResponseCapabilityAuthorized,
       MatchingSentCertifiedRequests,
       FrozenCertifiedResponseBinding,
       FrozenCertifiedRequestRegistration,
       CertifiedResponseAuthenticatedOccurrence,
       AsyncCertifiedResponseAuthProjection, AsyncNetworkItem

HistoricalLockedBodyExecutableCandidate(node, qc, candidate) ==
  /\ HistoricalLockedPrepareSource(node, qc)
  /\ HistoricalLockedBodyCandidateCoordinates(node, qc, candidate)
  /\ CASE candidate.kind = "FetchBody" ->
            /\ ~HistoricalLockedBodyValidationHeld(node, qc)
            /\ HistoricalLockedPrepareQcLineage(
                 node, qc, candidate.evidence)
       [] candidate.kind = "FetchCertifiedBody" ->
            /\ ~HistoricalLockedBodyValidationHeld(node, qc)
            /\ ~BodyHeldBy(durableBodies, node, context,
                            qc.view, qc.subject)
            /\ candidate.item = candidate.evidence
            /\ HistoricalLockedCertifiedResponseLineage(
                 node, qc, candidate.evidence)
       [] candidate.kind = "StoreBody" ->
            /\ ~HistoricalLockedBodyValidationHeld(node, qc)
            /\ ~BodyHeldBy(durableBodies, node, context,
                            qc.view, qc.subject)
            /\ BodyRecord(node, context, qc.view, qc.subject)
                 \in availableBodies
            /\ HistoricalLockedBodyEvidenceLineage(
                 node, qc, candidate.evidence)
       [] candidate.kind = "ValidateBody" ->
            /\ ~HistoricalLockedBodyValidationHeld(node, qc)
            /\ BodyHeldBy(durableBodies, node, context,
                           qc.view, qc.subject)
            /\ HistoricalLockedBodyEvidenceLineage(
                 node, qc, candidate.evidence)
       [] candidate.kind = "BeginLockCommit" ->
            /\ BodyHeldBy(durableBodies, node, context,
                           qc.view, qc.subject)
            /\ HistoricalLockedBodyValidationHeld(node, qc)
            /\ HistoricalLockedPrepareForCommit(node, qc)
            /\ NodeIdle(node)
            /\ HistoricalLockedBodyEvidenceLineage(
                 node, qc, candidate.evidence)
       [] OTHER -> FALSE

THEOREM HistoricalLockedBodyExecutableCandidateEnablesExecution ==
  \A node, qc, candidate:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalLockedBodyExecutableCandidate(node, qc, candidate)
    => ENABLED ExecuteCommand(candidate)
BY ExpandENABLED, IsaT(420)
   DEF AsyncStrongTypeInvariant, StrongInductiveInvariant,
       Safety, TypeInvariant, ReducerProvenanceInvariant,
       CertificatesBackedByIntents, HistoricalQcValid,
       HistoricalLockedBodyExecutableCandidate,
       HistoricalLockedBodyCandidateCoordinates,
       HistoricalLockedBodyValidationHeld,
       HistoricalLockedPrepareQcLineage,
       HistoricalLockedCertifiedResponseLineage,
       HistoricalLockedBodyEvidenceLineage,
       RestartLockedPrepareQCs,
       HistoricalLockedPrepareForCommit,
       HistoricalLockedPrepareSource, LockedPrepareRecoverySource,
       HistoricalLockedPrepareRecoveryProvenance,
       InstalledTcSelectsPrepareFor, ExactLockedCommitIntents,
       NoHigherConflictingPrepareKnown,
       ExecuteCommand, ExecuteRegularCommand, RegularCoreCommand,
       ExecuteDecisionFetch, CertifiedRecoveryFetchFrontier,
       LockedPrepareFetchFrontier,
       CertifiedBodyRecoveryAuthority,
       FetchCertifiedBody, AcceptCertifiedResponseCapability,
       InstallCertifiedBodyEffect, StoreBody, ValidateBody,
       ValidateDecidedBody, ValidateLockedBody, RejectBody,
       BeginLockCommit, LockCommitQcValues, ReceivedQcValues,
       CommandMatches, CandidateConsumerCurrent,
       HistoricalCertifiedResponseRecoveryEvidence,
       HistoricalBeginLockRecoveryEvidence,
       CertifiedResponseAuthenticatedOccurrence,
       CertifiedResponseCapabilityAuthorized,
       CertifiedRequestOutbox, CertifiedArchiveRoutes,
       AsyncCertifiedRequestHashOf, AsyncNetworkItem,
       SamePrepareRecoveryRef, SameCertificateRef, CertificateRefOf,
       AsyncAuxVars, vars

THEOREM HistoricalLockedBodyExecutableCandidateIsDispatchable ==
  \A node, qc, candidate:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalLockedBodyExecutableCandidate(node, qc, candidate)
    => CommandDispatchable(candidate)
PROOF
  <1>1. ASSUME NEW node, NEW qc, NEW candidate,
                AsyncStrongTypeInvariant,
                HistoricalLockedBodyExecutableCandidate(
                  node, qc, candidate)
         PROVE CommandDispatchable(candidate)
    <2>1. AsyncCandidateTyped(candidate)
      BY <1>1, Isa
         DEF HistoricalLockedBodyExecutableCandidate,
             HistoricalLockedBodyCandidateCoordinates,
             AsyncCandidateSet, AsyncCandidateTyped
    <2>2. /\ CandidateConsumerCurrent(candidate)
           /\ candidate.class = "Completion"
      BY <1>1
         DEF HistoricalLockedBodyExecutableCandidate,
             HistoricalLockedBodyCandidateCoordinates
    <2>3. ENABLED ExecuteCommand(candidate)
      BY <1>1, HistoricalLockedBodyExecutableCandidateEnablesExecution
    <2>4. CommandExecutionReady(candidate)
      BY <2>3, Isa DEF CommandExecutionReady, ExecuteCommand
    <2> QED BY <2>1, <2>2, <2>4 DEF CommandDispatchable
  <1> QED BY <1>1

THEOREM SelectedExecutableHistoricalFifoOwnerExecutes ==
  \A node \in ValidatorIds:
    \A qc:
      LET command == NextNodeCommand(node)
      IN /\ AsyncStrongTypeInvariant
         /\ HistoricalLockedBodyExecutableCandidate(node, qc, command)
         /\ FifoRuntimeStep(node)
         => /\ ExecuteCommand(command)
            /\ AppendCausalSuccessors(command)
            /\ CommandSuccessorsScheduledAfter(command)
BY HistoricalLockedBodyExecutableCandidateIsDispatchable,
   FifoSuccessfulExecutionSchedulesEverySuccessor, Isa
   DEF FifoRuntimeStep

THEOREM SelectedExecutableHistoricalDeferredOwnerExecutes ==
  \A node \in ValidatorIds:
    \A qc:
      LET command == NextDeferredCommand(node)
      IN /\ AsyncStrongTypeInvariant
         /\ AsyncProgressOwnershipInvariant
         /\ HistoricalLockedBodyExecutableCandidate(node, qc, command)
         /\ DeferredDrainStep(node)
         => /\ ExecuteCommand(command)
            /\ AppendCausalSuccessors(command)
            /\ CommandSuccessorsScheduledAfter(command)
BY HistoricalLockedBodyExecutableCandidateIsDispatchable,
   DeferredSuccessfulExecutionSchedulesEverySuccessor, Isa
   DEF DeferredDrainStep

(***************************************************************************
Semantic handoffs.
***************************************************************************)

THEOREM HistoricalLockedFetchMissingBodyOpensLineagedRequest ==
  \A node, qc, command:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalLockedBodyExecutableCandidate(node, qc, command)
    /\ command.kind = "FetchBody"
    /\ ~BodyHeldBy(durableBodies, node, context, qc.view, qc.subject)
    /\ ExecuteDecisionFetch(command)
    => HistoricalLockedCertifiedRequestActiveLineaged(node, qc)'
BY IsaT(300)
   DEF HistoricalLockedBodyExecutableCandidate,
       HistoricalLockedBodyCandidateCoordinates,
       HistoricalLockedPrepareQcLineage,
       HistoricalLockedCertifiedRequestActiveLineaged,
       ExecuteDecisionFetch, PublishCertifiedRequests,
       CertifiedRequestOutbox, CertifiedArchiveRoutes,
       AsyncCertifiedRequestHash, AsyncCertifiedRequestHashOf,
       AsyncCertifiedRequestEnvelope, RestartLockedPrepareQCs,
       SamePrepareRecoveryRef, SameCertificateRef, CertificateRefOf,
       CommandMatches, AsyncAuxVars, vars

THEOREM HistoricalLockedFetchHeldBodySchedulesLineagedValidation ==
  \A node, qc, command:
    /\ HistoricalLockedBodyExecutableCandidate(node, qc, command)
    /\ command.kind = "FetchBody"
    /\ BodyHeldBy(durableBodies, node, context, qc.view, qc.subject)
    /\ ExecuteDecisionFetch(command)
    /\ CommandSuccessorsScheduledAfter(command)
    => /\ BodyHeldBy(durableBodies', node, context',
                     qc.view, qc.subject)
       /\ ~HistoricalLockedBodyValidationHeld(node, qc)'
       /\ HistoricalLineagedLockedBodyValidateOwned(node, qc)'
BY IsaT(240)
   DEF HistoricalLockedBodyExecutableCandidate,
       HistoricalLockedBodyCandidateCoordinates,
       HistoricalLockedBodyValidationHeld,
       HistoricalLineagedLockedBodyValidateOwned,
       HistoricalLockedBodyValidateCandidate,
       HistoricalLockedBodyEvidenceLineage,
       HistoricalLockedPrepareQcLineage,
       CommandSuccessorsScheduledAfter,
       ExecuteDecisionFetch, CommandSuccessors, CausalCandidate,
       AsyncCandidateFrom, AsyncCandidateWithIdentity,
       CandidateConsumerCurrent, CandidateScheduled

THEOREM HistoricalCertifiedFetchStagesBodyAndSchedulesLineagedStore ==
  \A node, qc, command:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalLockedBodyExecutableCandidate(node, qc, command)
    /\ command.kind = "FetchCertifiedBody"
    /\ ExecuteCommand(command)
    /\ CommandSuccessorsScheduledAfter(command)
    => /\ BodyRecord(node, context', qc.view, qc.subject)
              \in availableBodies'
       /\ ~BodyHeldBy(durableBodies', node, context',
                      qc.view, qc.subject)
       /\ HistoricalLineagedLockedBodyStoreOwned(node, qc)'
BY IsaT(360)
   DEF HistoricalLockedBodyExecutableCandidate,
       HistoricalLockedBodyCandidateCoordinates,
       HistoricalLockedCertifiedResponseLineage,
       HistoricalLockedBodyEvidenceLineage,
       HistoricalLineagedLockedBodyStoreOwned,
       HistoricalLockedBodyStoreCandidate,
       CommandSuccessorsScheduledAfter,
       ExecuteCommand, ExecuteRegularCommand, RegularCoreCommand,
       FetchCertifiedBody, AcceptCertifiedResponseCapability,
       InstallCertifiedBodyEffect, CertifiedBodyRecoveryAuthority,
       LockedPrepareRecoverySource, RestartLockedPrepareQCs,
       CommandMatches, CommandSuccessors, CausalCandidate,
       AsyncCandidateFrom, AsyncCandidateWithIdentity,
       CandidateConsumerCurrent, CandidateScheduled,
       SamePrepareRecoveryRef, SameCertificateRef, CertificateRefOf

THEOREM HistoricalStoreSchedulesLineagedValidation ==
  \A node, qc, command:
    /\ HistoricalLockedBodyExecutableCandidate(node, qc, command)
    /\ command.kind = "StoreBody"
    /\ ExecuteCommand(command)
    /\ CommandSuccessorsScheduledAfter(command)
    => /\ BodyHeldBy(durableBodies', node, context',
                     qc.view, qc.subject)
       /\ ~HistoricalLockedBodyValidationHeld(node, qc)'
       /\ HistoricalLineagedLockedBodyValidateOwned(node, qc)'
BY IsaT(240)
   DEF HistoricalLockedBodyExecutableCandidate,
       HistoricalLockedBodyCandidateCoordinates,
       HistoricalLockedBodyValidationHeld,
       HistoricalLockedBodyEvidenceLineage,
       HistoricalLineagedLockedBodyValidateOwned,
       HistoricalLockedBodyValidateCandidate,
       CommandSuccessorsScheduledAfter,
       ExecuteCommand, ExecuteRegularCommand, RegularCoreCommand,
       StoreBody, CommandSuccessors, CausalCandidate,
       AsyncCandidateFrom, AsyncCandidateWithIdentity,
       CandidateConsumerCurrent, CandidateScheduled

THEOREM HistoricalValidationSchedulesLineagedBeginLockOrTerminal ==
  \A node, qc, command:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalLockedBodyExecutableCandidate(node, qc, command)
    /\ command.kind = "ValidateBody"
    /\ ExecuteCommand(command)
    /\ CommandSuccessorsScheduledAfter(command)
    => /\ BodyHeldBy(durableBodies', node, context',
                     qc.view, qc.subject)
       /\ HistoricalLockedBodyValidationHeld(node, qc)'
       /\ IF NoHigherConflictingPrepareKnown(node, qc)'
          THEN HistoricalLockedBodyBeginLockOwned(node, qc)'
          ELSE HistoricalLockedBodyRecoveryTerminal(node, qc)'
BY ValidationCommandSelectsValidationAction, IsaT(480)
   DEF AsyncStrongTypeInvariant, StrongInductiveInvariant,
       Safety, TypeInvariant, ReducerProvenanceInvariant,
       CertificatesBackedByIntents, HistoricalQcValid,
       HistoricalLockedBodyExecutableCandidate,
       HistoricalLockedBodyCandidateCoordinates,
       HistoricalLockedBodyValidationHeld,
       HistoricalLockedBodyEvidenceLineage,
       HistoricalLockedBodyBeginLockOwned,
       HistoricalLockedBodyBeginLockCandidate,
       HistoricalLockedBodyRecoveryTerminal,
       HistoricalLockedPrepareSource, LockedPrepareRecoverySource,
       HistoricalLockedPrepareRecoveryProvenance,
       HistoricalLockedPrepareForCommit,
       ExactLockedCommitIntents, NoHigherConflictingPrepareKnown,
       CommandSuccessorsScheduledAfter,
       ExecuteCommand, ExecuteRegularCommand, RegularCoreCommand,
       ValidateBody, ValidateDecidedBody, ValidateLockedBody, RejectBody,
       CommandMatches, CommandSuccessors, CausalCandidate,
       AsyncCandidateFrom, AsyncCandidateWithIdentity,
       CandidateConsumerCurrent, CandidateScheduled

(***************************************************************************
Exact BeginLock handoff.

The whole authenticated response remains the candidate evidence through
FetchCertifiedBody, StoreBody, ValidateBody, and the causal BeginLockCommit.
The direct projection above places either evidence arm in the lower recovery
candidate union.  The imported execution theorem then transfers ownership to
a same-reference WAL request.
***************************************************************************)

THEOREM LineagedHistoricalBeginLockExecutionCreatesSameRefPending ==
  \A node \in ValidatorIds, sourceQc \in QcRecordSet,
     command \in AsyncCandidateSet:
    /\ StrongInductiveInvariant
    /\ HistoricalLockedPrepareForCommit(node, sourceQc)
    /\ HistoricalLockedBodyBeginLockCandidate(node, sourceQc, command)
    /\ ExecuteCommand(command)
    => \E request \in pendingLockCommit':
         /\ request.node = node
         /\ SamePrepareRecoveryRef(request.qc, sourceQc)
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                NEW sourceQc \in QcRecordSet,
                NEW command \in AsyncCandidateSet,
                StrongInductiveInvariant,
                HistoricalLockedPrepareForCommit(node, sourceQc),
                HistoricalLockedBodyBeginLockCandidate(
                  node, sourceQc, command),
                ExecuteCommand(command)
         PROVE \E request \in pendingLockCommit':
                 /\ request.node = node
                 /\ SamePrepareRecoveryRef(request.qc, sourceQc)
    <2>1. HistoricalBeginLockRecoveryCandidate(
             node, sourceQc, command)
      BY <1>1, LineagedBeginLockProjectsHistoricalRecoveryCandidate
    <2> QED BY <1>1, <2>1,
         HistoricalBeginLockExecutionCreatesSameRefPending
  <1> QED BY <1>1

THEOREM HistoricalBeginLockCreatesLineagedCommitWitness ==
  \A node \in ValidatorIds, qc \in QcRecordSet,
     command \in AsyncCandidateSet:
    /\ StrongInductiveInvariant
    /\ HistoricalLockedPrepareForCommit(node, qc)
    /\ HistoricalLockedBodyBeginLockCandidate(node, qc, command)
    /\ ExecuteCommand(command)
    => HistoricalLockedBodyCommitWitnessLineaged(node, qc)'
BY LineagedHistoricalBeginLockExecutionCreatesSameRefPending, Isa
   DEF HistoricalLockedBodyCommitWitnessLineaged

THEOREM HistoricalPersistLockCommitCreatesExactCommitWitness ==
  \A node, qc, command:
    /\ HistoricalLockedPrepareSource(node, qc)
    /\ command.kind = "PersistLockCommit"
    /\ \E request \in pendingLockCommit:
         /\ request.node = node
         /\ SamePrepareRecoveryRef(request.qc, qc)
         /\ CommandMatches(command, request.node,
                           request.qc.view, request.qc.subject)
         /\ PersistLockCommit(request)
    => HistoricalLockedBodyCommitWitnessLineaged(node, qc)'
BY IsaT(240)
   DEF HistoricalLockedBodyCommitWitnessLineaged,
       HistoricalLockedPrepareSource, LockedPrepareRecoverySource,
       HistoricalLockedPrepareRecoveryProvenance,
       ExactLockedCommitIntents, PersistLockCommit,
       SamePrepareRecoveryRef, SameCertificateRef, CertificateRefOf,
       CommandMatches

(***************************************************************************
Carrier-neutral retention frame.

The semantic tuple includes every Core/recovery component inspected by the
lineaged stage.  Scheduled owners may move between queue, deferred, causal,
and executor carriers, but may not disappear.  Active request ownership is
tracked separately because authenticated response ingress moves exactly that
owner into FetchCertifiedBody.  Authenticated sent history is append-only,
not unchanged: retaining its old occurrences is sufficient to preserve every
response-lined owner while allowing an unrelated service action to publish.
***************************************************************************)

HistoricalLockedBodyLineageSemanticVars ==
  <<context, nodeView, generation,
    availableBodies, durableBodies, validatedBodies,
    prepareIntents, commitIntents, prepareQCs, installedTCs,
    lockRank, lockSubject, highestRank, highestSubject,
    pendingLockCommit, decisions, applied,
    asyncActiveRequests, AsyncRecoveryVars>>

HistoricalLockedLineagedRequestsRetained ==
  \A node, qc:
    HistoricalLockedCertifiedRequestActiveLineaged(node, qc)
      => HistoricalLockedCertifiedRequestActiveLineaged(node, qc)'

HistoricalLockedLineagedCandidatesRetained ==
  \A node, qc, candidate:
    (\/ HistoricalLockedBodyFetchCandidate(node, qc, candidate)
     \/ HistoricalLockedBodyCertifiedFetchCandidate(node, qc, candidate)
     \/ HistoricalLockedBodyStoreCandidate(node, qc, candidate)
     \/ HistoricalLockedBodyValidateCandidate(node, qc, candidate)
     \/ HistoricalLockedBodyBeginLockCandidate(node, qc, candidate))
      => CandidateScheduled(candidate)'

HistoricalLockedAuthenticatedHistoryRetained ==
  asyncSentItems \subseteq asyncSentItems'

HistoricalLockedBodyLineageRetentionFrame ==
  /\ UNCHANGED HistoricalLockedBodyLineageSemanticVars
  /\ AsyncCurrentResponsiveVoters'
       \subseteq AsyncCurrentResponsiveVoters
  /\ HistoricalLockedAuthenticatedHistoryRetained
  /\ HistoricalLockedLineagedRequestsRetained
  /\ HistoricalLockedLineagedCandidatesRetained

THEOREM HistoricalLockedBodyLineageFramePreservesSourceRetention ==
  /\ HistoricalLockedBodyLineageSourceRetentionInvariant
  /\ HistoricalLockedBodyLineageRetentionFrame
  => HistoricalLockedBodyLineageSourceRetentionInvariant'
BY IsaT(360)
   DEF HistoricalLockedBodyLineageSourceRetentionInvariant,
       HistoricalLockedBodyRecoveryStageLineaged,
       HistoricalLockedBodyRecoveryStageLineagedWithoutAuthority,
       HistoricalLockedBodyRecoveryAuthority,
       HistoricalLockedBodyCompletionOwnerLineaged,
       HistoricalLockedBodyCommitWitnessLineaged,
       HistoricalLockedBodyDurableOrPendingCommitWitness,
       HistoricalLockedCertifiedRequestActiveLineaged,
       HistoricalLineagedLockedBodyFetchOwned,
       HistoricalLineagedLockedBodyCertifiedFetchOwned,
       HistoricalLineagedLockedBodyStoreOwned,
       HistoricalLineagedLockedBodyValidateOwned,
       HistoricalLockedBodyBeginLockOwned,
       HistoricalLockedBodyFetchCandidate,
       HistoricalLockedBodyCertifiedFetchCandidate,
       HistoricalLockedBodyStoreCandidate,
       HistoricalLockedBodyValidateCandidate,
       HistoricalLockedBodyBeginLockCandidate,
       HistoricalLockedBodyCandidateCoordinates,
       HistoricalLockedBodyEvidenceLineage,
       HistoricalLockedPrepareQcLineage,
       HistoricalLockedCertifiedResponseLineage,
       HistoricalCertifiedResponseRecoveryEvidence,
       HistoricalLockedBodyValidationHeld,
       HistoricalLockedBodyRecoveryTerminal,
       HistoricalLockedPrepareSource, LockedPrepareRecoverySource,
       HistoricalLockedPrepareRecoveryProvenance,
       InstalledTcSelectsPrepareFor, ExactLockedCommitIntents,
       NoHigherConflictingPrepareKnown, RestartLockedPrepareQCs,
       HistoricalLockedBodyLineageRetentionFrame,
       HistoricalLockedBodyLineageSemanticVars,
       HistoricalLockedAuthenticatedHistoryRetained,
       HistoricalLockedLineagedRequestsRetained,
       HistoricalLockedLineagedCandidatesRetained,
       CertifiedResponseAuthenticatedOccurrence,
       CertifiedResponseCapabilityAuthorized,
       MatchingSentCertifiedRequests,
       FrozenCertifiedResponseBinding,
       FrozenCertifiedRequestRegistration,
       AsyncCertifiedResponseAuthProjection,
       CertifiedRequestOutbox, CertifiedArchiveRoutes,
       AsyncCertifiedRequestHash, AsyncCertifiedRequestHashOf,
       CandidateConsumerCurrent, CandidateScheduled

(***************************************************************************
Local admission is a pure owner relocation.
***************************************************************************)

THEOREM LocalAdmissionPreservesHistoricalLockedBodyLineageSourceRetention ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalLockedBodyLineageSourceRetentionInvariant
    /\ (LocalAdmissionStep(node)
          \/ SerializedLocalPrecedesServeIngressStep(node))
    => HistoricalLockedBodyLineageSourceRetentionInvariant'
BY LocalAdmissionPreservesScheduledCandidateSet,
   SelectedLocalAdmissionAdvancePreservesScheduledCandidateSet,
   IsaT(300)
   DEF HistoricalLockedBodyLineageSourceRetentionInvariant,
       HistoricalLockedBodyRecoveryStageLineaged,
       HistoricalLockedBodyRecoveryStageLineagedWithoutAuthority,
       HistoricalLockedBodyRecoveryAuthority,
       HistoricalLockedBodyCommitWitnessLineaged,
       HistoricalLockedBodyDurableOrPendingCommitWitness,
       HistoricalLockedCertifiedRequestActiveLineaged,
       HistoricalLineagedLockedBodyFetchOwned,
       HistoricalLineagedLockedBodyCertifiedFetchOwned,
       HistoricalLineagedLockedBodyStoreOwned,
       HistoricalLineagedLockedBodyValidateOwned,
       HistoricalLockedBodyBeginLockOwned,
       HistoricalLockedBodyFetchCandidate,
       HistoricalLockedBodyCertifiedFetchCandidate,
       HistoricalLockedBodyStoreCandidate,
       HistoricalLockedBodyValidateCandidate,
       HistoricalLockedBodyBeginLockCandidate,
       HistoricalLockedBodyCandidateCoordinates,
       HistoricalLockedBodyEvidenceLineage,
       HistoricalLockedPrepareQcLineage,
       HistoricalLockedCertifiedResponseLineage,
       HistoricalLockedBodyValidationHeld,
       HistoricalLockedBodyRecoveryTerminal,
       HistoricalLockedPrepareSource, LockedPrepareRecoverySource,
       HistoricalLockedPrepareRecoveryProvenance,
       InstalledTcSelectsPrepareFor, ExactLockedCommitIntents,
       NoHigherConflictingPrepareKnown, RestartLockedPrepareQCs,
       CertifiedResponseCapabilityAuthorized,
       MatchingSentCertifiedRequests,
       FrozenCertifiedResponseBinding,
       FrozenCertifiedRequestRegistration,
       ScheduledCandidateSet, CandidateScheduled,
       LocalAdmissionStep,
       SerializedLocalPrecedesServeIngressStep,
       SelectedLocalAdmissionAdvance,
       AdmitProducerCompletion, AdmitCausalHead,
       AsyncRecoveryVars, vars

(***************************************************************************
Authenticated CertifiedResponse ingress.

An authorized response retires the exact hash-matched request registration
and installs exactly CertifiedResponseCandidate(item).  The candidate freezes
the complete outer response as both item and evidence; subsequent StoreBody,
ValidateBody, and BeginLockCommit children keep that evidence while replacing
item with NoAsyncItem.
***************************************************************************)

THEOREM AuthorizedLineagedResponseCandidateCarriesExactEvidence ==
  \A node, qc, item:
    /\ HistoricalLockedCertifiedResponseLineage(node, qc, item)
    /\ CertifiedResponseCandidate(item) \in AsyncCandidateSet
    /\ CandidateScheduled(CertifiedResponseCandidate(item))
    => HistoricalLineagedLockedBodyCertifiedFetchOwned(node, qc)
BY Isa
   DEF HistoricalLineagedLockedBodyCertifiedFetchOwned,
       HistoricalLockedBodyCertifiedFetchCandidate,
       HistoricalLockedBodyCandidateCoordinates,
       HistoricalLockedCertifiedResponseLineage,
       CertifiedResponseCandidate, AsyncCandidate,
       AsyncCandidateWithIdentity

THEOREM DrainFairIngressPreservesHistoricalLockedBodyLineageSourceRetention ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ HistoricalLockedBodyLineageSourceRetentionInvariant
    /\ DrainFairIngressSelected(node)
    => HistoricalLockedBodyLineageSourceRetentionInvariant'
BY AuthorizedLineagedResponseCandidateCarriesExactEvidence,
   CertifiedResponseClaimAuthorizationSuppliesFrozenCapability,
   SequenceWithoutIndexRetainsOtherValue,
   SequenceSetAfterAppend, IsaT(720)
   DEF HistoricalLockedBodyLineageSourceRetentionInvariant,
       HistoricalLockedBodyRecoveryStageLineaged,
       HistoricalLockedBodyRecoveryStageLineagedWithoutAuthority,
       HistoricalLockedBodyRecoveryAuthority,
       HistoricalLockedBodyCommitWitnessLineaged,
       HistoricalLockedBodyDurableOrPendingCommitWitness,
       HistoricalLockedCertifiedRequestActiveLineaged,
       HistoricalLineagedLockedBodyFetchOwned,
       HistoricalLineagedLockedBodyCertifiedFetchOwned,
       HistoricalLineagedLockedBodyStoreOwned,
       HistoricalLineagedLockedBodyValidateOwned,
       HistoricalLockedBodyBeginLockOwned,
       HistoricalLockedBodyFetchCandidate,
       HistoricalLockedBodyCertifiedFetchCandidate,
       HistoricalLockedBodyStoreCandidate,
       HistoricalLockedBodyValidateCandidate,
       HistoricalLockedBodyBeginLockCandidate,
       HistoricalLockedBodyCandidateCoordinates,
       HistoricalLockedBodyEvidenceLineage,
       HistoricalLockedPrepareQcLineage,
       HistoricalLockedCertifiedResponseLineage,
       HistoricalLockedBodyValidationHeld,
       HistoricalLockedBodyRecoveryTerminal,
       HistoricalLockedPrepareSource, LockedPrepareRecoverySource,
       HistoricalLockedPrepareRecoveryProvenance,
       InstalledTcSelectsPrepareFor, ExactLockedCommitIntents,
       NoHigherConflictingPrepareKnown, RestartLockedPrepareQCs,
       DrainFairIngressSelected, PopSelectedIngress,
       IngressItemCanDrain, DeliveryCandidate,
       CertifiedResponseAuthorized, MatchingCertifiedRequests,
       CertifiedRequestAuthorized,
       CertifiedBodyRecoveryAuthority,
       CertifiedResponseCandidate, CertifiedRequestOutbox,
       HistoricalCertifiedResponseRecoveryEvidence,
       CertifiedResponseAuthenticatedOccurrence,
       CertifiedResponseCapabilityAuthorized,
       MatchingSentCertifiedRequests,
       FrozenCertifiedResponseBinding,
       FrozenCertifiedRequestRegistration,
       AsyncCertifiedResponseAuthProjection,
       AsyncCertifiedRequestHash, AsyncCertifiedRequestHashOf,
       CertifiedArchiveRoutes,
       AsyncCandidate, AsyncCandidateWithIdentity,
       EnqueueCandidate, CandidateScheduled,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, AsyncProgressOwnershipInvariant,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncIoVars, AsyncRecoveryVars, SequenceSet, vars

THEOREM IngressDrainPreservesHistoricalLockedBodyLineageSourceRetention ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ HistoricalLockedBodyLineageSourceRetentionInvariant
    /\ IngressDrainStep(node)
    => HistoricalLockedBodyLineageSourceRetentionInvariant'
BY DrainFairIngressPreservesHistoricalLockedBodyLineageSourceRetention,
   IsaT(180)
   DEF IngressDrainStep, AsyncRecoveryVars, vars

(***************************************************************************
PersistInstallTC replacement.

Install clears the target node's validation slice and advances its consumer
generation.  Its first causal child is a Qc-backed FetchBody for the resulting
lock.  All QcRecords representing that same resulting Prepare reference share
that one logical owner, even when authenticated signer sets differ.
***************************************************************************)

THEOREM PersistInstallFetchCoversEveryPostHistoricalSource ==
  \A command, qc:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ command \in AsyncCandidateSet
    /\ command.kind = "PersistInstallTC"
    /\ ExecuteCommand(command)
    /\ CommandSuccessorsScheduledAfter(command)
    /\ qc \in prepareQCs'
    /\ HistoricalLockedPrepareSource(command.node, qc)'
    => HistoricalLineagedLockedBodyFetchOwned(command.node, qc)'
BY ExecutedInstallLockedFetchSuccessorIsTypedAndOwned,
   ExecutedInstallLockedFetchSuccessorMatchesPostState,
   PendingInstallTCResultingLockIsCertified,
   IsaT(720)
   DEF AsyncStrongTypeInvariant, StrongInductiveInvariant,
       Safety, TypeInvariant, ReducerProvenanceInvariant,
       CertificatesBackedByIntents, HistoricalQcValid,
       AsyncProgressOwnershipInvariant,
       CommandSuccessorsScheduledAfter,
       ExecuteCommand, ExecutePersistInstall,
       PersistInstallTC, PersistInstalledControlAfterInstall,
       InstallCommandSuccessors, InstallLockedFetchSuccessors,
       InstallLockedFetchSuccessor,
       InstallResultingLockedPrepareQCs, InstallRequests,
       ResultingInstallLockRank, ResultingInstallLockSubject,
       HistoricalLineagedLockedBodyFetchOwned,
       HistoricalLockedBodyFetchCandidate,
       HistoricalLockedBodyCandidateCoordinates,
       HistoricalLockedPrepareQcLineage,
       RestartLockedPrepareQCs,
       HistoricalLockedPrepareSource, LockedPrepareRecoverySource,
       HistoricalLockedPrepareRecoveryProvenance,
       InstalledTcSelectsPrepareFor, ExactLockedCommitIntents,
       NoHigherConflictingPrepareKnown,
       SamePrepareRecoveryRef, SameCertificateRef, CertificateRefOf,
       CandidateConsumerCurrent, CandidateScheduled,
       CommandSuccessors, SequenceSet

THEOREM PersistInstallEstablishesTargetLineagedStage ==
  \A command:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ command \in AsyncCandidateSet
    /\ command.kind = "PersistInstallTC"
    /\ ExecuteCommand(command)
    /\ CommandSuccessorsScheduledAfter(command)
    => \A qc \in prepareQCs':
         HistoricalLockedPrepareSource(command.node, qc)'
           => HistoricalLockedBodyRecoveryStageLineaged(
                command.node, qc)'
BY PersistInstallFetchCoversEveryPostHistoricalSource, IsaT(180)
   DEF HistoricalLockedBodyRecoveryStageLineaged,
       HistoricalLockedBodyRecoveryStageLineagedWithoutAuthority,
       HistoricalLineagedLockedBodyFetchOwned,
       HistoricalLockedBodyValidationHeld,
       ExecuteCommand, ExecutePersistInstall, PersistInstallTC

(***************************************************************************
Selected-owner discard analysis.

Busy FIFO selection defers a Completion owner without loss.  Deferred
selection runs only while NodeIdle.  At that point a current body owner is
executable unless its semantic boundary has already advanced:

  * FetchCertifiedBody/StoreBody advanced to a durable-body Validate owner;
  * ValidateBody advanced to validation plus BeginLock or terminal conflict;
  * BeginLockCommit advanced to a same-reference pending WAL request, an exact
    durable Commit intent, terminal conflict, or a no-longer-relevant source;
  * PersistInstallTC replaced the consumer generation and installed a fresh
    Qc-backed Fetch owner.

The two theorems below state that exact replacement fact for the complete
runtime actions, including removal of the selected sequence occurrence.
***************************************************************************)

HistoricalLockedBodyScheduledCandidate(node, qc, candidate) ==
  \/ HistoricalLockedBodyFetchCandidate(node, qc, candidate)
  \/ HistoricalLockedBodyCertifiedFetchCandidate(node, qc, candidate)
  \/ HistoricalLockedBodyStoreCandidate(node, qc, candidate)
  \/ HistoricalLockedBodyValidateCandidate(node, qc, candidate)
  \/ HistoricalLockedBodyBeginLockCandidate(node, qc, candidate)

THEOREM SelectedHistoricalFifoOwnerPreservesOrReplacesSource ==
  \A node \in ValidatorIds:
    \A qc:
      LET command == NextNodeCommand(node)
      IN /\ AsyncStrongTypeInvariant
         /\ AsyncProgressOwnershipInvariant
         /\ HistoricalLockedBodyLineageSourceRetentionInvariant
         /\ HistoricalLockedPrepareSource(node, qc)
         /\ HistoricalLockedBodyScheduledCandidate(node, qc, command)
         /\ FifoRuntimeStep(node)
         => (HistoricalLockedPrepareSource(node, qc)'
               => HistoricalLockedBodyRecoveryStageLineaged(node, qc)')
BY HistoricalLockedBodyExecutableCandidateIsDispatchable,
   FifoSuccessfulExecutionSchedulesEverySuccessor,
   HistoricalLockedFetchMissingBodyOpensLineagedRequest,
   HistoricalLockedFetchHeldBodySchedulesLineagedValidation,
   HistoricalCertifiedFetchStagesBodyAndSchedulesLineagedStore,
   HistoricalStoreSchedulesLineagedValidation,
   HistoricalValidationSchedulesLineagedBeginLockOrTerminal,
   LineagedHistoricalBeginLockExecutionCreatesSameRefPending,
   PersistInstallEstablishesTargetLineagedStage,
   SequenceWithoutIndexRetainsOtherValue,
   SequenceSetAfterAppend, IsaT(900)
   DEF AsyncStrongTypeInvariant, StrongInductiveInvariant,
       HistoricalLockedBodyLineageSourceRetentionInvariant,
       HistoricalLockedBodyRecoveryStageLineaged,
       HistoricalLockedBodyRecoveryStageLineagedWithoutAuthority,
       HistoricalLockedBodyDurableOrPendingCommitWitness,
       HistoricalLockedBodyCommitWitnessLineaged,
       HistoricalLockedCertifiedRequestActiveLineaged,
       HistoricalLineagedLockedBodyFetchOwned,
       HistoricalLineagedLockedBodyCertifiedFetchOwned,
       HistoricalLineagedLockedBodyStoreOwned,
       HistoricalLineagedLockedBodyValidateOwned,
       HistoricalLockedBodyBeginLockOwned,
       HistoricalLockedBodyScheduledCandidate,
       HistoricalLockedBodyExecutableCandidate,
       HistoricalLockedBodyFetchCandidate,
       HistoricalLockedBodyCertifiedFetchCandidate,
       HistoricalLockedBodyStoreCandidate,
       HistoricalLockedBodyValidateCandidate,
       HistoricalLockedBodyBeginLockCandidate,
       HistoricalLockedBodyCandidateCoordinates,
       HistoricalLockedBodyEvidenceLineage,
       HistoricalLockedPrepareQcLineage,
       HistoricalLockedCertifiedResponseLineage,
       HistoricalLockedBodyValidationHeld,
       HistoricalLockedBodyRecoveryTerminal,
       HistoricalLockedPrepareForCommit,
       HistoricalLockedPrepareSource, LockedPrepareRecoverySource,
       HistoricalLockedPrepareRecoveryProvenance,
       InstalledTcSelectsPrepareFor, ExactLockedCommitIntents,
       NoHigherConflictingPrepareKnown, RestartLockedPrepareQCs,
       CommandSuccessorsScheduledAfter,
       FifoRuntimeStep, RemoveNextNodeCommand,
       DeferCommand, DiscardCommand,
       ExecuteCommand, ExecuteRegularCommand, RegularCoreCommand,
       ExecuteDecisionFetch, ExecutePersistInstall,
       AppendCausalSuccessors, FreshCommandSuccessors,
       FreshCandidateSequence, CommandSuccessors,
       CandidateScheduled, CandidateConsumerCurrent,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, AsyncRecoveryVars, SequenceSet, vars

THEOREM SelectedHistoricalDeferredOwnerPreservesOrReplacesSource ==
  \A node \in ValidatorIds:
    \A qc:
      LET command == NextDeferredCommand(node)
      IN /\ AsyncStrongTypeInvariant
         /\ AsyncProgressOwnershipInvariant
         /\ HistoricalLockedBodyLineageSourceRetentionInvariant
         /\ HistoricalLockedPrepareSource(node, qc)
         /\ HistoricalLockedBodyScheduledCandidate(node, qc, command)
         /\ DeferredDrainStep(node)
         => (HistoricalLockedPrepareSource(node, qc)'
               => HistoricalLockedBodyRecoveryStageLineaged(node, qc)')
BY HistoricalLockedBodyExecutableCandidateIsDispatchable,
   DeferredSuccessfulExecutionSchedulesEverySuccessor,
   HistoricalLockedFetchMissingBodyOpensLineagedRequest,
   HistoricalLockedFetchHeldBodySchedulesLineagedValidation,
   HistoricalCertifiedFetchStagesBodyAndSchedulesLineagedStore,
   HistoricalStoreSchedulesLineagedValidation,
   HistoricalValidationSchedulesLineagedBeginLockOrTerminal,
   LineagedHistoricalBeginLockExecutionCreatesSameRefPending,
   PersistInstallEstablishesTargetLineagedStage,
   TailRetainsNonHeadValue, SequenceSetAfterAppend, IsaT(900)
   DEF AsyncStrongTypeInvariant, StrongInductiveInvariant,
       HistoricalLockedBodyLineageSourceRetentionInvariant,
       HistoricalLockedBodyRecoveryStageLineaged,
       HistoricalLockedBodyRecoveryStageLineagedWithoutAuthority,
       HistoricalLockedBodyDurableOrPendingCommitWitness,
       HistoricalLockedBodyCommitWitnessLineaged,
       HistoricalLockedCertifiedRequestActiveLineaged,
       HistoricalLineagedLockedBodyFetchOwned,
       HistoricalLineagedLockedBodyCertifiedFetchOwned,
       HistoricalLineagedLockedBodyStoreOwned,
       HistoricalLineagedLockedBodyValidateOwned,
       HistoricalLockedBodyBeginLockOwned,
       HistoricalLockedBodyScheduledCandidate,
       HistoricalLockedBodyExecutableCandidate,
       HistoricalLockedBodyFetchCandidate,
       HistoricalLockedBodyCertifiedFetchCandidate,
       HistoricalLockedBodyStoreCandidate,
       HistoricalLockedBodyValidateCandidate,
       HistoricalLockedBodyBeginLockCandidate,
       HistoricalLockedBodyCandidateCoordinates,
       HistoricalLockedBodyEvidenceLineage,
       HistoricalLockedPrepareQcLineage,
       HistoricalLockedCertifiedResponseLineage,
       HistoricalLockedBodyValidationHeld,
       HistoricalLockedBodyRecoveryTerminal,
       HistoricalLockedPrepareForCommit,
       HistoricalLockedPrepareSource, LockedPrepareRecoverySource,
       HistoricalLockedPrepareRecoveryProvenance,
       InstalledTcSelectsPrepareFor, ExactLockedCommitIntents,
       NoHigherConflictingPrepareKnown, RestartLockedPrepareQCs,
       CommandSuccessorsScheduledAfter,
       DeferredDrainStep, RemoveNextDeferredCommand,
       AdvanceNextDeferredClass, DeferredClassQueue,
       DiscardCommand, ExecuteCommand, ExecuteRegularCommand,
       RegularCoreCommand, ExecuteDecisionFetch, ExecutePersistInstall,
       AppendCausalSuccessors, FreshCommandSuccessors,
       FreshCandidateSequence, CommandSuccessors,
       CandidateScheduled, CandidateConsumerCurrent,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, AsyncRecoveryVars, SequenceSet, vars

(***************************************************************************
Serialized runtime and RunNode preservation.

These statements close the existing production dispatch union; they do not
replace it with a proof-only action.  Unselected owners are retained by the
generic sequence-removal lemmas imported from the Decision sibling.  Selected
owners use the exact handoffs above.  PersistInstallTC is the sole
generation-changing runtime arm and is handled by its resulting Qc Fetch.
***************************************************************************)

THEOREM SerializedRuntimePreservesHistoricalLockedBodyLineageSourceRetention ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ HistoricalLockedBodyLineageSourceRetentionInvariant
    /\ SerializedRuntimeStep(node)
    => HistoricalLockedBodyLineageSourceRetentionInvariant'
BY SelectedHistoricalFifoOwnerPreservesOrReplacesSource,
   SelectedHistoricalDeferredOwnerPreservesOrReplacesSource,
   FifoSuccessfulExecutionSchedulesEverySuccessor,
   DeferredSuccessfulExecutionSchedulesEverySuccessor,
   HistoricalLockedFetchMissingBodyOpensLineagedRequest,
   HistoricalLockedFetchHeldBodySchedulesLineagedValidation,
   HistoricalCertifiedFetchStagesBodyAndSchedulesLineagedStore,
   HistoricalStoreSchedulesLineagedValidation,
   HistoricalValidationSchedulesLineagedBeginLockOrTerminal,
   LineagedHistoricalBeginLockExecutionCreatesSameRefPending,
   HistoricalPersistLockCommitCreatesExactCommitWitness,
   PersistInstallEstablishesTargetLineagedStage,
   CertifiedRequestOutboxDecisionSurvivalIsExactTarget,
   PersistDecisionControlRetainsExactlySurvivingRequests,
   SequenceWithoutIndexRetainsOtherValue,
   TailRetainsNonHeadValue, SequenceSetAfterAppend, IsaT(1200)
   DEF AsyncStrongTypeInvariant, StrongInductiveInvariant,
       HistoricalLockedBodyLineageSourceRetentionInvariant,
       HistoricalLockedBodyRecoveryStageLineaged,
       HistoricalLockedBodyRecoveryStageLineagedWithoutAuthority,
       HistoricalLockedBodyRecoveryAuthority,
       HistoricalLockedBodyDurableOrPendingCommitWitness,
       HistoricalLockedBodyCommitWitnessLineaged,
       HistoricalLockedCertifiedRequestActiveLineaged,
       HistoricalLineagedLockedBodyFetchOwned,
       HistoricalLineagedLockedBodyCertifiedFetchOwned,
       HistoricalLineagedLockedBodyStoreOwned,
       HistoricalLineagedLockedBodyValidateOwned,
       HistoricalLockedBodyBeginLockOwned,
       HistoricalLockedBodyScheduledCandidate,
       HistoricalLockedBodyExecutableCandidate,
       HistoricalLockedBodyFetchCandidate,
       HistoricalLockedBodyCertifiedFetchCandidate,
       HistoricalLockedBodyStoreCandidate,
       HistoricalLockedBodyValidateCandidate,
       HistoricalLockedBodyBeginLockCandidate,
       HistoricalLockedBodyCandidateCoordinates,
       HistoricalLockedBodyEvidenceLineage,
       HistoricalLockedPrepareQcLineage,
       HistoricalLockedCertifiedResponseLineage,
       HistoricalLockedBodyValidationHeld,
       HistoricalLockedBodyRecoveryTerminal,
       HistoricalLockedPrepareForCommit,
       HistoricalLockedPrepareSource, LockedPrepareRecoverySource,
       HistoricalLockedPrepareRecoveryProvenance,
       InstalledTcSelectsPrepareFor, ExactLockedCommitIntents,
       NoHigherConflictingPrepareKnown, RestartLockedPrepareQCs,
       CommandSuccessorsScheduledAfter,
       SerializedRuntimeStep, RuntimeStep,
       FifoRuntimeStep, DeferredDrainStep,
       DeferredTagStep, DeferredTimeoutStep,
       DeferredRetransmitStep, DirectTimeoutStep,
       DirectRetransmitStep, IdleRuntimeStep,
       RemoveNextNodeCommand, RemoveNextDeferredCommand,
       DeferCommand, DiscardCommand,
       ExecuteCommand, ExecuteRegularCommand, RegularCoreCommand,
       ExecuteDecisionFetch, ExecutePersistInstall,
       ExecutePersistDecision, PersistDecisionControl,
       CertifiedRequestSurvivesDecision,
       FilterCertifiedResponseAuthority, ExecuteApply,
       AppendCausalSuccessors, AppendHistoricalLockedRetransmitSuccessors,
       HistoricalLockedRetransmitSuccessors,
       FreshCommandSuccessors, FreshCandidateSequence,
       CommandSuccessors, CandidateScheduled, CandidateConsumerCurrent,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, AsyncRecoveryVars, SequenceSet, vars

THEOREM RunNodeWorkPreservesHistoricalLockedBodyLineageSourceRetention ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ HistoricalLockedBodyLineageSourceRetentionInvariant
    /\ RunNodeWork(node)
    => HistoricalLockedBodyLineageSourceRetentionInvariant'
BY LocalAdmissionPreservesHistoricalLockedBodyLineageSourceRetention,
   IngressDrainPreservesHistoricalLockedBodyLineageSourceRetention,
   SerializedRuntimePreservesHistoricalLockedBodyLineageSourceRetention,
   Isa
   DEF RunNodeWork, SerializedLocalPrecedesServeIngressStep

THEOREM RunNodePreservesHistoricalLockedBodyLineageSourceRetention ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ HistoricalLockedBodyLineageSourceRetentionInvariant
    /\ RunNode(node)
    => HistoricalLockedBodyLineageSourceRetentionInvariant'
BY RunNodeWorkPreservesHistoricalLockedBodyLineageSourceRetention, Isa
   DEF RunNode, RunHistoricalRecoveryNode, RunHistoricalServer,
       HistoricalRecoveryTarget, DrainHistoricalIngressSelected,
       HistoricalIdleStep, AsyncRecoveryVars, vars

(***************************************************************************
Explicit remaining frontier -- no theorem follows.

This module closes the corrected lineage vocabulary through base, exact
dispatch, FIFO/deferred carrier mechanics, every body semantic handoff,
same-reference BeginLock WAL transfer, authenticated ingress, target
PersistInstall replacement, LocalAdmission, SerializedRuntimeStep,
RunNodeWork, and RunNode.  In particular, the exact authenticated outer
response survives response -> Store -> Validate -> BeginLock and projects
unconditionally through the broadened lower BeginLock witness into the
release stage and invariant vocabulary.

ResponsiveReplayLockedBodyLineagedCarrierInvariant names the concrete
without-authority carrier required for every RestartLockedPrepareQC while the
controller is Replaying.  Its establishment and preservation across
Crash/Restart/DriveResponsiveReplayHead/FinishResponsiveReplay/Rearm,
ordinary non-runner transport/I/O actions, and the final AsyncNext aggregation
remain outside this file.  No theorem here claims that full aggregation.

The outer response source remains irrelevant.  Archive routing, archive
signature ownership, and the cited signer of the exact frozen QC are explicit
independent identities in every response-lined owner.
***************************************************************************)

=============================================================================
